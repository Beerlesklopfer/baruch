// SPDX-License-Identifier: AGPL-3.0-or-later
//! RPC of the persona orders (contract Round 10, §5.5), from [`crate::personas::call`]:
//!
//! - `persona.ask {work?, key, persona, question, rev?, scope?}` → `{post: {id, author, body}, question_post,
//!   model, truncated, cut, compressed, url}`; the question stands in the chat beforehand as a post of the human
//! - `persona.review {work?, key, persona?, rev?, question?, scope?}` → ditto (findings as a post)
//! - `persona.propose {work?, key, persona?, instruction?, rev?, scope?}` → `{rev, status: "proposed", author,
//!   model, truncated, compressed, proposals, url}`
//! - `scope` (Round 15): `"section"` (default) or `"chapter"` (whole chapter or whole sermon; for a
//!   proposal one `proposed` per part, all in `proposals`, `rev` is the first); `compressed` is `null` or
//!   `{message, summarized, chunks}` if the context was chunked and compressed
//! - `persona.chat {work?, key, message, rev?, as_order?}` → `{posts: [..], errors: [..], url, rfc, proposals,
//!   findings, notes}`: post of the human, then per mentioned active persona (`@handle`, at most 3, no
//!   duplicates) its answer; Round 28: „RFC“ at the start or „@alle“ calls all active personas (lector last),
//!   a write order or `as_order` becomes a proposal (`proposals: [{persona, rev, url}]`), see
//!   [`super::chat`]
//!
//! - Round 22: ask/review carry `done_proposals`, `done_notes`, `all_done` if the persona reported own findings as
//!   resolved (query „Befund n scheint erledigt?“, [`super::propose_done`]).
//! - Round 30b: `basis` = `shown` (default) | `current` | revision number for ask/review (review basis, see
//!   [`super::basis`]); the answer carries `basis: {rev, label, proposal}`. Propose is based on the shown revision
//!   (`rev`) or the explicitly chosen one (`basis` = `current` | `<n>`), also on a proposal (bug 0.9.0).
//!
//! Addressing is always through `key`; `node` is rejected (-32602). `work` missing → default work;
//! `persona` missing (review, propose) → the lector of the work. Error of the model → -32000 with the
//! honest message, nothing is stored then.

use baruch_store::WorkId;
use serde_json::{Value, json};
use tokio::sync::mpsc::UnboundedSender;

pub use super::chat::{chat_json, run_chat};
use super::{Mode, Order, Outcome, Progress, Scope};
use crate::AppState;
use crate::rpc::{INVALID_PARAMS, RpcError};

/// Methods of this module.
pub const METHODS: &[&str] = &["persona.ask", "persona.review", "persona.propose", "persona.chat"];

fn invalid(message: impl Into<String>) -> RpcError {
    RpcError::new(INVALID_PARAMS, message.into())
}

/// Text parameter (missing/`null` → `None`).
pub fn text(params: &Value, name: &str) -> Result<Option<String>, RpcError> {
    match params.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) => Ok(Some(s.clone())),
        Some(_) => Err(invalid(format!("{name}: Text erwartet"))),
    }
}

/// Number (or digits as text, forms); missing/empty → `None`.
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

/// Scope `scope`: `"section"` (default, also for a missing/empty value) or `"chapter"`; otherwise -32602.
pub fn scope(params: &Value) -> Result<Scope, RpcError> {
    match text(params, "scope")?.as_deref().map(str::trim) {
        None | Some("") | Some("section") => Ok(Scope::Section),
        Some("chapter") => Ok(Scope::Chapter),
        Some(other) => Err(invalid(format!("scope: „section“ oder „chapter“ erwartet, nicht „{other}“"))),
    }
}

/// Work, key and shown revision; `node` is rejected for anything that writes.
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

/// Persona from `persona` or the lector of the work.
async fn persona_or_lector(state: &AppState, params: &Value, work: WorkId) -> Result<String, RpcError> {
    if let Some(p) = text(params, "persona")?.filter(|p| !p.trim().is_empty()) {
        return Ok(p);
    }
    let loaded = state.store.load_work(work).await?;
    loaded.lector().map(|p| p.handle.clone()).ok_or_else(|| invalid("Keine Persona ist aktiv (kein Lektor)."))
}

/// Order from RPC parameters (ask/review/propose).
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
    // Review also takes the note as `instruction` (one text field for review and proposal in the form).
    let text = match text(params, field)? {
        Some(t) if !t.trim().is_empty() => t,
        _ if mode == Mode::Review => text(params, "instruction")?.unwrap_or_default(),
        other => other.unwrap_or_default(),
    };
    // Round 30b: review basis `basis` (shown | current | <n>) for ask and review; for a proposal the base.
    let (rev, basis) = if mode == Mode::Propose {
        // Bug 0.9.0 (critique cycle): an explicitly chosen review basis (`current` or revision n, e.g.
        // „Vorschlag 2 von @lektor“) is the base of the new proposal; without a choice the shown revision.
        match super::basis::BasisChoice::from_param(params)? {
            super::basis::BasisChoice::Shown => (rev, None),
            choice => (Some(super::basis::choose(state, work, key, rev, choice).await?.rev), None),
        }
    } else {
        let choice = super::basis::BasisChoice::from_param(params)?;
        let chosen = super::basis::choose(state, work, key, rev, choice).await?;
        // Without a shown revision the address stays without `rev` as long as the current revision is reviewed.
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

/// Answer of an order as JSON (ask/review: post; propose: proposal).
pub fn outcome_json(outcome: &Outcome) -> Value {
    match outcome.mode {
        Mode::Propose => json!({
            "rev": outcome.rev, "status": "proposed", "author": outcome.persona, "model": outcome.model,
            "truncated": outcome.truncated, "url": outcome.url, "key": outcome.key, "work": outcome.work,
            "compressed": outcome.compressed, "proposals": outcome.proposals,
            // Round 36: the revision the proposal (for a chapter: that of the own part) is based on.
            "basis": outcome.basis,
        }),
        _ => {
            let mut value = json!({
                "post": outcome.post, "question_post": outcome.question, "model": outcome.model,
                "truncated": outcome.truncated, "cut": outcome.cut, "url": outcome.url, "key": outcome.key,
                "work": outcome.work, "persona": outcome.persona, "compressed": outcome.compressed,
                "basis": outcome.basis,
            });
            // Round 22: queries „Befund n scheint erledigt?“ from the answer (only if there are any).
            if !outcome.done_proposals.is_empty() || !outcome.done_notes.is_empty() || outcome.all_done {
                value["done_proposals"] = json!(outcome.done_proposals);
                value["done_notes"] = json!(outcome.done_notes);
                value["all_done"] = json!(outcome.all_done);
            }
            value
        }
    }
}

/// A method from [`METHODS`], optionally with progress (SSE).
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
