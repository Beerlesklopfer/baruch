// SPDX-License-Identifier: AGPL-3.0-or-later
//! RPC der Persona-Aufträge (Vertrag Runde 10, §5.5), aus [`crate::personas::call`]:
//!
//! - `persona.ask {work?, key, persona, question, rev?, scope?}` → `{post: {id, author, body}, question_post,
//!   model, truncated, cut, compressed, url}`; die Frage steht vorher als Beitrag des Menschen im Chat
//! - `persona.review {work?, key, persona?, rev?, question?, scope?}` → dito (Befunde als Beitrag)
//! - `persona.propose {work?, key, persona?, instruction?, rev?, scope?}` → `{rev, status: "proposed", author,
//!   model, truncated, compressed, proposals, url}`
//! - `scope` (Runde 15): `"section"` (Vorgabe) oder `"chapter"` (ganzes Kapitel bzw. ganze Predigt; beim
//!   Vorschlag je Teil ein `proposed`, alle in `proposals`, `rev` ist der erste); `compressed` ist `null` oder
//!   `{message, summarized, chunks}`, wenn der Kontext gestückelt und komprimiert wurde
//! - `persona.chat {work?, key, message, rev?, as_order?}` → `{posts: [..], errors: [..], url, rfc, proposals,
//!   findings, notes}`: Beitrag des Menschen, dann je erwähnter aktiver Persona (`@handle`, höchstens 3, ohne
//!   Doppelte) ihre Antwort; Runde 28: „RFC“ am Anfang bzw. „@alle“ ruft alle aktiven Personas (Lektor zuletzt),
//!   ein Schreibauftrag bzw. `as_order` wird ein Vorschlag (`proposals: [{persona, rev, url}]`), siehe
//!   [`super::chat`]
//!
//! - Runde 22: ask/review tragen `done_proposals`, `done_notes`, `all_done`, wenn die Persona eigene Befunde als erledigt
//!   meldete (Rückfrage „Befund n scheint erledigt?“, [`super::propose_done`]).
//! - Runde 30b: `basis` = `shown` (Vorgabe) | `current` | Fassungsnummer bei ask/review (Prüfgrundlage, siehe
//!   [`super::basis`]); die Antwort trägt `basis: {rev, label, proposal}`. Propose beruht auf der gezeigten Fassung
//!   (`rev`) bzw. der ausdrücklich gewählten (`basis` = `current` | `<n>`), auch auf einem Vorschlag (Fehler 0.9.0).
//!
//! Adressiert wird immer über `key`; `node` wird abgelehnt (-32602). `work` fehlt → Vorgabewerk;
//! `persona` fehlt (review, propose) → der Lektor des Werks. Fehler des Modells → -32000 mit der
//! ehrlichen Meldung, gespeichert ist dann nichts.

use baruch_store::WorkId;
use serde_json::{Value, json};
use tokio::sync::mpsc::UnboundedSender;

pub use super::chat::{chat_json, run_chat};
use super::{Mode, Order, Outcome, Progress, Scope};
use crate::AppState;
use crate::rpc::{INVALID_PARAMS, RpcError};

/// Methoden dieses Moduls.
pub const METHODS: &[&str] = &["persona.ask", "persona.review", "persona.propose", "persona.chat"];

fn invalid(message: impl Into<String>) -> RpcError {
    RpcError::new(INVALID_PARAMS, message.into())
}

/// Text-Parameter (fehlt/`null` → `None`).
pub fn text(params: &Value, name: &str) -> Result<Option<String>, RpcError> {
    match params.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) => Ok(Some(s.clone())),
        Some(_) => Err(invalid(format!("{name}: Text erwartet"))),
    }
}

/// Zahl (oder Ziffern als Text, Formulare); fehlt/leer → `None`.
pub fn number(params: &Value, name: &str) -> Result<Option<i64>, RpcError> {
    let wrong = || invalid(format!("{name}: Zahl erwartet"));
    match params.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Number(n)) => n.as_i64().map(Some).ok_or_else(wrong),
        Some(Value::String(s)) if s.trim().is_empty() => Ok(None),
        Some(Value::String(s)) => s.trim().parse().map(Some).map_err(|_| wrong()),
        Some(_) => Err(wrong()),
    }
}

/// Umfang `scope`: `"section"` (Vorgabe, auch bei fehlendem/leerem Wert) oder `"chapter"`; sonst -32602.
pub fn scope(params: &Value) -> Result<Scope, RpcError> {
    match text(params, "scope")?.as_deref().map(str::trim) {
        None | Some("") | Some("section") => Ok(Scope::Section),
        Some("chapter") => Ok(Scope::Chapter),
        Some(other) => Err(invalid(format!("scope: „section“ oder „chapter“ erwartet, nicht „{other}“"))),
    }
}

/// Werk, Schlüssel und gezeigte Fassung; `node` wird für Schreibendes abgelehnt.
pub async fn address(state: &AppState, params: &Value) -> Result<(WorkId, i64, Option<u32>), RpcError> {
    if params.get("node").is_some_and(|n| !n.is_null()) {
        return Err(invalid("node (Stelle) wird nicht angenommen; key mitgeben (fester Schlüssel des Abschnitts)."));
    }
    let work = match number(params, "work")? {
        Some(work) => work,
        None => state.default_work().await?,
    };
    state.store.work_slug(work).await?;
    let key = number(params, "key")?.ok_or_else(|| invalid("key fehlt (fester Schlüssel des Abschnitts)"))?;
    let rev = match number(params, "rev")? {
        Some(rev) => Some(u32::try_from(rev).map_err(|_| invalid("rev: Fassungsnummer ab 1 erwartet"))?),
        None => None,
    };
    Ok((work, key, rev))
}

/// Persona aus `persona` oder der Lektor des Werks.
async fn persona_or_lector(state: &AppState, params: &Value, work: WorkId) -> Result<String, RpcError> {
    if let Some(p) = text(params, "persona")?.filter(|p| !p.trim().is_empty()) {
        return Ok(p);
    }
    let loaded = state.store.load_work(work).await?;
    loaded.lector().map(|p| p.handle.clone()).ok_or_else(|| invalid("Keine Persona ist aktiv (kein Lektor)."))
}

/// Auftrag aus RPC-Parametern (ask/review/propose).
pub async fn order(state: &AppState, mode: Mode, params: &Value) -> Result<Order, RpcError> {
    let (work, key, rev) = address(state, params).await?;
    let persona = match mode {
        Mode::Ask => text(params, "persona")?
            .filter(|p| !p.trim().is_empty())
            .ok_or_else(|| invalid("persona fehlt (z. B. „@lektor“)"))?,
        _ => persona_or_lector(state, params, work).await?,
    };
    let field = match mode {
        Mode::Propose => "instruction",
        Mode::Chat => "message",
        _ => "question",
    };
    // Prüfen nimmt den Hinweis auch als `instruction` (ein Textfeld für Prüfen und Vorschlag im Formular).
    let text = match text(params, field)? {
        Some(t) if !t.trim().is_empty() => t,
        _ if mode == Mode::Review => text(params, "instruction")?.unwrap_or_default(),
        other => other.unwrap_or_default(),
    };
    // Runde 30b: Prüfgrundlage `basis` (shown | current | <n>) für Fragen und Prüfen; beim Vorschlag die Basis.
    let (rev, basis) = if mode == Mode::Propose {
        // Fehler 0.9.0 (Kritik-Zyklus): eine ausdrücklich gewählte Prüfgrundlage (`current` bzw. Fassung n, etwa
        // „Vorschlag 2 von @lektor“) ist die Basis des neuen Vorschlags; ohne Wahl die gezeigte Fassung.
        match super::basis::BasisChoice::from_param(params)? {
            super::basis::BasisChoice::Shown => (rev, None),
            choice => (Some(super::basis::choose(state, work, key, rev, choice).await?.rev), None),
        }
    } else {
        let choice = super::basis::BasisChoice::from_param(params)?;
        let chosen = super::basis::choose(state, work, key, rev, choice).await?;
        // Ohne gezeigte Fassung bleibt die Adresse ohne `rev`, solange die aktuelle Fassung geprüft wird.
        let rev = if rev.is_some() || chosen.proposal || choice != super::basis::BasisChoice::Shown {
            Some(chosen.rev)
        } else {
            rev
        };
        (rev, Some(chosen))
    };
    Ok(Order {
        work,
        key,
        persona,
        mode,
        text,
        rev,
        asker: None,
        post_question: mode == Mode::Ask,
        sources: vec![],
        web: true,
        scope: if mode == Mode::Chat { Scope::Section } else { scope(params)? },
        frame: Default::default(),
        links: Default::default(),
        basis,
    })
}

/// Antwort eines Auftrags als JSON (ask/review: Beitrag; propose: Vorschlag).
pub fn outcome_json(outcome: &Outcome) -> Value {
    match outcome.mode {
        Mode::Propose => json!({
            "rev": outcome.rev, "status": "proposed", "author": outcome.persona, "model": outcome.model,
            "truncated": outcome.truncated, "url": outcome.url, "key": outcome.key, "work": outcome.work,
            "compressed": outcome.compressed, "proposals": outcome.proposals,
            // Runde 36: die Fassung, auf der der Vorschlag (beim Kapitel: der des eigenen Teils) beruht.
            "basis": outcome.basis,
        }),
        _ => {
            let mut value = json!({
                "post": outcome.post, "question_post": outcome.question, "model": outcome.model,
                "truncated": outcome.truncated, "cut": outcome.cut, "url": outcome.url, "key": outcome.key,
                "work": outcome.work, "persona": outcome.persona, "compressed": outcome.compressed,
                "basis": outcome.basis,
            });
            // Runde 22: Rückfragen „Befund n scheint erledigt?“ aus der Antwort (nur, wenn es welche gibt).
            if !outcome.done_proposals.is_empty() || !outcome.done_notes.is_empty() || outcome.all_done {
                value["done_proposals"] = json!(outcome.done_proposals);
                value["done_notes"] = json!(outcome.done_notes);
                value["all_done"] = json!(outcome.all_done);
            }
            value
        }
    }
}

/// Eine Methode aus [`METHODS`], wahlweise mit Fortschritt (SSE).
pub async fn call_with(
    state: &AppState,
    method: &str,
    params: &Value,
    progress: Option<&UnboundedSender<Progress>>,
) -> Result<Value, RpcError> {
    let mode = match method {
        "persona.ask" => Mode::Ask,
        "persona.review" => Mode::Review,
        "persona.propose" => Mode::Propose,
        "persona.chat" => return Ok(chat_json(&run_chat(state, params, progress).await?)),
        _ => return Err(RpcError::new(crate::rpc::METHOD_NOT_FOUND, format!("unknown method {method}"))),
    };
    let order = order(state, mode, params).await?;
    Ok(outcome_json(&super::run(state, &order, progress).await?))
}

pub async fn call(state: &AppState, method: &str, params: Value) -> Result<Value, RpcError> {
    let params = if params.is_null() { json!({}) } else { params };
    call_with(state, method, &params, None).await
}
