// SPDX-License-Identifier: AGPL-3.0-or-later
//! Was ein Lektor schreibt (Runde 35, Vertrag §2.3): Vorschlag einer Fassung (`revision.propose`) und Befund
//! (`finding.add`), beide unter seinem Handle und mit Herkunft im Spiegel.
//!
//! Beide Methoden gibt es nur für Lektoren: Der Autor speichert (`revision.save`) und entscheidet über Befunde; ein
//! Aufruf mit vollen Rechten wird mit einem Hinweis abgelehnt (`REFUSED`). Die Rechte selbst (Methode erlaubt, Werk
//! freigegeben) hat [`crate::roles::check_rpc`] schon geprüft, bevor eine der Funktionen hier läuft; `work` ist das
//! freigegebene Werk. Abschnitte werden über `key` (fester Schlüssel) oder `node` (Stelle) genannt und immer im
//! genannten Werk gesucht – ein Schlüssel eines fremden Werks trifft nichts.

use baruch_core::RevisionStatus;
use baruch_store::{FindingKind, NewFinding, StoreError, WorkId};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::auth::{Identity, Who};
use crate::rpc::{INVALID_PARAMS, REFUSED, RpcError, resolve_keys};
use crate::{AppState, mirror};

/// Größter Vorschlag (Bytes) – dieselbe Grenze wie für MCP `propose_revision`.
pub const MAX_PROPOSAL_BYTES: usize = crate::mcp::tools::MAX_MCP_PROPOSAL_BYTES;

/// Längste Notiz zu einem Vorschlag (Zeichen) – wie für MCP.
pub const MAX_NOTE_CHARS: usize = crate::mcp::tools::MAX_MCP_NOTE_CHARS;

fn invalid(message: impl Into<String>) -> RpcError {
    RpcError::new(INVALID_PARAMS, message)
}

/// Der Lektor, der die Anfrage stellt; alle anderen bekommen `hint` als Ablehnung.
fn lektor<'w>(who: &'w Who, hint: &str) -> Result<&'w Identity, RpcError> {
    who.lektor().ok_or_else(|| RpcError::new(REFUSED, hint))
}

#[derive(Deserialize)]
struct ProposeParams {
    node: usize,
    body: String,
    note: Option<String>,
}

/// `revision.propose {work, key|node, body, note?}` → `{node, key, rev, status: "proposal", author}`.
///
/// Derselbe Weg wie MCP `propose_revision`: `Store::add_proposal` mit dem Handle als Persona und leerem Modell; die
/// aktuelle Fassung bleibt, was sie ist. Markdown-Überschriften werden wie bei `revision.save` zu
/// Typst-Überschriften. Unter der Schreibsperre; danach führt [`mirror::side_refs_after_proposal`] den Seitenzweig
/// `refs/revisions/<key>/<n>` nach (Herkunft: der Handle). Leerer Text, Text wie die aktuelle Fassung und zu große
/// Eingaben werden abgelehnt, ohne etwas zu schreiben.
pub async fn propose(state: &AppState, who: &Who, work: WorkId, params: Value) -> Result<Value, RpcError> {
    let identity = lektor(
        who,
        "revision.propose ist der Weg der Lektoren: Der Autor speichert seine Fassung mit revision.save bzw. \
         outline.save.",
    )?;
    let held = state.write_lock.lock().await;
    let p: ProposeParams = serde_json::from_value(resolve_keys(state, work, params).await?)
        .map_err(|e| invalid(format!("revision.propose: {e}")))?;
    if p.body.trim().is_empty() {
        return Err(invalid("Leerer Text: ein Vorschlag braucht den vollständigen Text des Abschnitts."));
    }
    if p.body.len() > MAX_PROPOSAL_BYTES {
        return Err(invalid(format!("Der Vorschlag darf höchstens {MAX_PROPOSAL_BYTES} Bytes haben.")));
    }
    let note = p.note.unwrap_or_default();
    if note.chars().count() > MAX_NOTE_CHARS {
        return Err(invalid(format!("note darf höchstens {MAX_NOTE_CHARS} Zeichen haben.")));
    }
    let body = p.body.replace("\r\n", "\n");
    let body = baruch_typst::markdown_headings(&body).into_owned();
    let loaded = state.store.load_work(work).await?;
    let Some(node) = loaded.nodes.get(p.node) else {
        return Err(StoreError::UnknownNode { work, node: p.node }.into());
    };
    if node.revisions.iter().any(|r| r.status == RevisionStatus::Current && r.body == body) {
        return Err(RpcError::new(REFUSED, "Der Text gleicht der aktuellen Fassung; kein Vorschlag angelegt."));
    }
    let rev = state.store.add_proposal(work, p.node, &body, &identity.handle, "", &note).await?;
    mirror::side_refs_after_proposal(state, &held, work).await;
    Ok(json!({
        "work": work, "node": p.node, "key": node.key, "rev": rev, "status": "proposal", "author": identity.handle,
    }))
}

/// Text aus `params[name]`; fehlt oder `null` → `None`, anderes als Text → -32602.
fn text(params: &Value, name: &str) -> Result<Option<String>, RpcError> {
    match params.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) => Ok(Some(s.clone())),
        Some(_) => Err(invalid(format!("{name}: Text erwartet"))),
    }
}

/// Fester Schlüssel aus `params.key` (Zahl oder Ziffern als Text, Formulare); fehlt oder leer → `None`.
fn key(params: &Value) -> Result<Option<i64>, RpcError> {
    let wrong = || invalid("key: fester Schlüssel (Zahl) erwartet");
    match params.get("key") {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Number(n)) => n.as_i64().map(Some).ok_or_else(wrong),
        Some(Value::String(s)) if s.trim().is_empty() => Ok(None),
        Some(Value::String(s)) => s.trim().parse().map(Some).map_err(|_| wrong()),
        Some(_) => Err(wrong()),
    }
}

/// `finding.add {work, key?, kind: "hint"|"critical"|"praise", body, quote?}` → `{finding, commit?|mirror?}`.
///
/// Ein Befund unter dem Handle des Lektors, ohne Beitrag (`Store::finding_insert`); ohne `key` eine „Anmerkung zum
/// Buch“ (Befund ohne Abschnitt, sichtbar auf der Befundseite). Status ist „offen“ – über den Status entscheidet
/// allein der Autor. Unter der Schreibsperre; genau ein Commit im Spiegel ([`mirror::describe_finding_added`]).
pub async fn add_finding(state: &AppState, who: &Who, work: WorkId, params: Value) -> Result<Value, RpcError> {
    let identity = lektor(
        who,
        "finding.add ist der Weg der Lektoren: Der Autor hält eine Anmerkung als Einfall oder Beitrag fest und \
         entscheidet über Befunde mit finding.set_status.",
    )?;
    let kind = match text(&params, "kind")?.as_deref().map(str::trim) {
        None | Some("") => return Err(invalid("kind fehlt (hint, critical oder praise)")),
        Some(kind) => FindingKind::from_db(kind)
            .ok_or_else(|| invalid(format!("kind: hint, critical oder praise erwartet, nicht „{kind}“")))?,
    };
    let body = text(&params, "body")?.ok_or_else(|| invalid("body fehlt (Text des Befunds)"))?;
    let quote = text(&params, "quote")?.unwrap_or_default();
    let key = key(&params)?;
    let held = state.write_lock.lock().await;
    let finding = state
        .store
        .finding_insert(&NewFinding {
            work,
            key,
            post: None,
            thread_post: None,
            position: 0,
            persona: &identity.handle,
            kind,
            body: body.trim(),
            quote: quote.trim(),
            quote_check: "",
            similar_to: None,
            summary_of: &[],
        })
        .await?;
    let mut answer = json!({ "finding": crate::findings::finding_json(work, &finding) });
    let result = match state.store.load_work(work).await {
        Ok(loaded) => {
            let request = mirror::describe_finding_added(&loaded, &finding, &identity.handle, &identity.display);
            mirror::record_finding(state, &held, work, request).await
        }
        Err(e) => Err(e.to_string()),
    };
    crate::rpc::add_mirror(&mut answer, result);
    Ok(answer)
}
