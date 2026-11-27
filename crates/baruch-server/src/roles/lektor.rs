// SPDX-License-Identifier: AGPL-3.0-or-later
//! What a Lektor writes (Round 35, contract §2.3): proposal of a revision (`revision.propose`) and finding
//! (`finding.add`), both under his handle and with the origin in the mirror.
//!
//! Both methods exist only for Lektors: the author saves (`revision.save`) and decides on findings; a
//! call with full rights is rejected with a hint (`REFUSED`). The rights themselves (method allowed, work
//! released) have already been checked by [`crate::roles::check_rpc`] before one of the functions here runs; `work`
//! is the released work. Sections are named through `key` (fixed key) or `node` (position) and always searched in
//! the named work – a key of a foreign work hits nothing.

use baruch_core::RevisionStatus;
use baruch_store::{FindingKind, NewFinding, StoreError, WorkId};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::auth::{Identity, Who};
use crate::rpc::{INVALID_PARAMS, REFUSED, RpcError, resolve_keys};
use crate::{AppState, mirror};

/// Largest proposal (bytes) – the same limit as for MCP `propose_revision`.
pub const MAX_PROPOSAL_BYTES: usize = crate::mcp::tools::MAX_MCP_PROPOSAL_BYTES;

/// Longest note on a proposal (characters) – as for MCP.
pub const MAX_NOTE_CHARS: usize = crate::mcp::tools::MAX_MCP_NOTE_CHARS;

fn invalid(message: impl Into<String>) -> RpcError {
    RpcError::new(INVALID_PARAMS, message)
}

/// The Lektor who makes the request; all others get `hint` as the rejection.
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
/// The same path as MCP `propose_revision`: `Store::add_proposal` with the handle as persona and an empty model; the
/// current revision stays what it is. Markdown headings become Typst headings as in `revision.save`.
/// Under the write lock; afterwards [`mirror::side_refs_after_proposal`] updates the side branch
/// `refs/revisions/<key>/<n>` (origin: the handle). Empty text, text equal to the current revision and too large
/// inputs are rejected without writing anything.
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

/// Text from `params[name]`; missing or `null` → `None`, anything other than text → -32602.
fn text(params: &Value, name: &str) -> Result<Option<String>, RpcError> {
    match params.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) => Ok(Some(s.clone())),
        Some(_) => Err(invalid(format!("{name}: Text erwartet"))),
    }
}

/// Fixed key from `params.key` (number or digits as text, forms); missing or empty → `None`.
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
/// A finding under the handle of the Lektor, without a post (`Store::finding_insert`); without `key` an „Anmerkung
/// zum Buch“ (finding without a section, visible on the findings page). The status is „offen“ – the author alone
/// decides on the status. Under the write lock; exactly one commit in the mirror
/// ([`mirror::describe_finding_added`]).
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
