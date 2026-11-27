// SPDX-License-Identifier: AGPL-3.0-or-later
//! RPC of the findings (Round 28, task befunde). All methods take `work` (number or digits; missing → default work).
//!
//! - `finding.list {work, key?, keys?, subtree?, post?, status?, kind?, persona?, since?, until?, include_praise?,
//!   include_attached?, limit?}` → `{findings, total}`; `keys`, `status`, `kind` as a list or text „a,b“;
//!   `subtree: true` adds all sub-items to `key` or `keys`.
//! - `finding.get {work, id}` → `{finding, history, attached, summary_of}`
//! - `finding.set_status {work, id, status, reason?, rev?, target_key?, target_idea?}` (human only) →
//!   `{finding, before, commit?|mirror?}`
//! - `finding.decide {work, entry, confirm}` (human only) → `{finding, entry, commit?|mirror?}`
//!
//! Write paths ([`set_status`], [`propose`], [`decide`]) hold [`AppState::write_lock`] from writing to the commit
//! and yield exactly one commit in the mirror ([`mirror::record_finding`], trailer `Baruch-Finding`, `Baruch-Action`
//! `finding-status|finding-propose|finding-decide`, `Baruch-Node`, for proposals `Persona`). Through RPC the status
//! is always set by the human; assistants (persona, MCP) reach only [`propose`].

use std::collections::HashSet;

use baruch_store::{FindingFilter, IdeaOrigin, WorkId};
use serde_json::{Value, json};

use super::{DeferTarget, FindingKind, FindingStatus, StatusChange, finding_json};
use crate::rpc::{INVALID_PARAMS, METHOD_NOT_FOUND, RpcError};
use crate::{AppState, mirror};

pub const METHODS: &[&str] = &["finding.list", "finding.get", "finding.set_status", "finding.decide"];

/// Does this module serve the RPC method?
pub fn handles(method: &str) -> bool {
    METHODS.contains(&method)
}

fn invalid(message: impl Into<String>) -> RpcError {
    RpcError::new(INVALID_PARAMS, message)
}

/// Integer from `params[name]` (number or digits as text from forms); missing, `null` or empty → `None`.
fn int(params: &Value, name: &str) -> Result<Option<i64>, RpcError> {
    let wrong = || invalid(format!("{name}: Zahl erwartet"));
    match params.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Number(n)) => n.as_i64().map(Some).ok_or_else(wrong),
        Some(Value::String(s)) if s.trim().is_empty() => Ok(None),
        Some(Value::String(s)) => s.trim().parse().map(Some).map_err(|_| wrong()),
        Some(_) => Err(wrong()),
    }
}

fn required(params: &Value, name: &str, what: &str) -> Result<i64, RpcError> {
    int(params, name)?.ok_or_else(|| invalid(format!("{name} fehlt ({what})")))
}

/// Boolean (true/false or from forms „1“, „true“, „on“, „0“, „false“); missing → `None`.
fn flag(params: &Value, name: &str) -> Result<Option<bool>, RpcError> {
    match params.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Bool(b)) => Ok(Some(*b)),
        Some(Value::String(s)) => match s.trim() {
            "1" | "true" | "on" | "ja" => Ok(Some(true)),
            "0" | "false" | "off" | "nein" | "" => Ok(Some(false)),
            _ => Err(invalid(format!("{name}: true oder false erwartet"))),
        },
        Some(_) => Err(invalid(format!("{name}: true oder false erwartet"))),
    }
}

fn text(params: &Value, name: &str) -> Result<Option<String>, RpcError> {
    match params.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) => Ok(Some(s.clone())),
        Some(_) => Err(invalid(format!("{name}: Text erwartet"))),
    }
}

/// List from `params[name]`: JSON list of texts or numbers, or text „a,b“.
fn words(params: &Value, name: &str) -> Result<Vec<String>, RpcError> {
    match params.get(name) {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(Value::String(s)) => {
            Ok(s.split(',').map(str::trim).filter(|w| !w.is_empty()).map(str::to_owned).collect())
        }
        Some(Value::Array(items)) => items
            .iter()
            .map(|v| match v {
                Value::String(s) => Ok(s.trim().to_owned()),
                Value::Number(n) => Ok(n.to_string()),
                _ => Err(invalid(format!("{name}: Liste von Texten erwartet"))),
            })
            .collect(),
        Some(Value::Number(n)) => Ok(vec![n.to_string()]),
        Some(_) => Err(invalid(format!("{name}: Liste oder Text „a,b“ erwartet"))),
    }
}

/// Work from `work` (number or digits); missing → default work; unknown → -32602.
pub(crate) async fn work_of(state: &AppState, params: &Value) -> Result<WorkId, RpcError> {
    match int(params, "work")? {
        None => Ok(state.default_work().await?),
        Some(work) => {
            state.store.work_slug(work).await?;
            Ok(work)
        }
    }
}

/// RPC `finding.*` (the human).
pub async fn call(state: &AppState, method: &str, params: Value) -> Result<Value, RpcError> {
    let params = if params.is_null() { json!({}) } else { params };
    let work = work_of(state, &params).await?;
    match method {
        "finding.list" => list(state, work, &params).await,
        "finding.get" => get(state, work, required(&params, "id", "Befund")?).await,
        "finding.set_status" => {
            let id = required(&params, "id", "Befund")?;
            let change = parse_change(&params)?;
            set_status(state, work, id, &change).await
        }
        "finding.decide" => {
            let entry = required(&params, "entry", "Vorschlag im Verlauf")?;
            let confirm = flag(&params, "confirm")?
                .ok_or_else(|| invalid("confirm fehlt (true = bestätigen, false = ablehnen)"))?;
            decide(state, work, entry, confirm).await
        }
        other => Err(RpcError::new(METHOD_NOT_FOUND, format!("unknown method {other}"))),
    }
}

/// Selection from the parameters of `finding.list`.
async fn filter_of(state: &AppState, work: WorkId, params: &Value) -> Result<FindingFilter, RpcError> {
    let mut keys: Vec<i64> = Vec::new();
    if let Some(key) = int(params, "key")? {
        keys.push(key);
    }
    for key in words(params, "keys")? {
        keys.push(key.parse().map_err(|_| invalid(format!("keys: fester Schlüssel (Zahl) erwartet, nicht „{key}“")))?);
    }
    if flag(params, "subtree")?.unwrap_or(false) && !keys.is_empty() {
        let loaded = state.store.load_work(work).await?;
        let mut wanted: HashSet<i64> = keys.iter().copied().collect();
        // Tree order: parents stand before their children, one pass is enough.
        let mut seqs: HashSet<usize> = loaded.nodes.iter().filter(|n| wanted.contains(&n.key)).map(|n| n.id).collect();
        for node in &loaded.nodes {
            if node.parent.is_some_and(|p| seqs.contains(&p)) {
                seqs.insert(node.id);
                wanted.insert(node.key);
            }
        }
        keys = loaded.nodes.iter().filter(|n| wanted.contains(&n.key)).map(|n| n.key).collect();
    }
    let statuses = words(params, "status")?
        .iter()
        .map(|s| FindingStatus::from_db(s).ok_or_else(|| invalid(format!("status: unbekannter Status „{s}“"))))
        .collect::<Result<Vec<_>, _>>()?;
    let kinds = words(params, "kind")?
        .iter()
        .map(|s| FindingKind::from_db(s).ok_or_else(|| invalid(format!("kind: unbekannte Art „{s}“"))))
        .collect::<Result<Vec<_>, _>>()?;
    let limit = match int(params, "limit")? {
        None => None,
        Some(n) if (1..=2000).contains(&n) => Some(n as u32),
        Some(_) => return Err(invalid("limit: 1 bis 2000")),
    };
    Ok(FindingFilter {
        keys,
        post: int(params, "post")?,
        statuses,
        kinds,
        persona: text(params, "persona")?.filter(|p| !p.trim().is_empty()),
        since: text(params, "since")?.filter(|p| !p.trim().is_empty()),
        until: text(params, "until")?.filter(|p| !p.trim().is_empty()),
        include_praise: flag(params, "include_praise")?.unwrap_or(false),
        include_attached: flag(params, "include_attached")?.unwrap_or(false),
        limit,
    })
}

async fn list(state: &AppState, work: WorkId, params: &Value) -> Result<Value, RpcError> {
    let filter = filter_of(state, work, params).await?;
    let findings = state.store.finding_list(work, &filter).await?;
    // `total` = all matching (without limit); `truncated` if `findings` are only the newest of them.
    let total = state.store.finding_count(work, &filter).await?;
    Ok(json!({ "findings": findings.iter().map(|f| finding_json(work, f)).collect::<Vec<_>>(), "total": total,
               "truncated": total > findings.len() }))
}

async fn get(state: &AppState, work: WorkId, id: i64) -> Result<Value, RpcError> {
    let finding = state.store.finding_get(work, id).await?;
    let history = state.store.finding_history(work, id).await?;
    // Loaded specifically (without limit): a work list would lose attached ones and origins from 2000 findings on.
    let attached: Vec<Value> =
        state.store.finding_attached_to(work, id).await?.iter().map(|f| finding_json(work, f)).collect();
    let summary_of: Vec<Value> =
        state.store.findings_by_uuid(work, &finding.summary_of).await?.iter().map(|f| finding_json(work, f)).collect();
    Ok(json!({
        "finding": finding_json(work, &finding),
        "history": history,
        "attached": attached,
        "summary_of": summary_of,
    }))
}

/// Read the wanted status from the parameters: `status` = open|adopted|rejected|deferred|done; adopted needs
/// `rev` (revision number, from 1), rejected `reason` (not empty), deferred exactly one of `target_key` (section)
/// or `target_idea` (idea). `reason` is otherwise optional. Whether revision and target exist the store checks.
pub fn parse_change(params: &Value) -> Result<StatusChange, RpcError> {
    let status =
        text(params, "status")?.ok_or_else(|| invalid("status fehlt (open, adopted, rejected, deferred, done)"))?;
    let reason = text(params, "reason")?.unwrap_or_default();
    match status.trim() {
        "open" => Ok(StatusChange::Open),
        "adopted" => {
            let rev = int(params, "rev")?.ok_or_else(|| invalid("„übernommen“ braucht die Fassung (rev)."))?;
            let rev = u32::try_from(rev).ok().filter(|r| *r >= 1).ok_or_else(|| invalid("rev: Fassungsnummer ab 1"))?;
            Ok(StatusChange::Adopted { rev })
        }
        "rejected" => {
            if reason.trim().is_empty() {
                return Err(invalid("Verwerfen braucht eine Begründung (reason)."));
            }
            Ok(StatusChange::Rejected { reason })
        }
        "deferred" => match (int(params, "target_key")?, int(params, "target_idea")?) {
            (Some(key), None) => Ok(StatusChange::Deferred { target: DeferTarget::Node { key }, reason }),
            (None, Some(id)) => Ok(StatusChange::Deferred { target: DeferTarget::Idea { id }, reason }),
            (None, None) => {
                Err(invalid("„verschoben“ braucht ein Ziel: target_key (Abschnitt) oder target_idea (Einfall)."))
            }
            (Some(_), Some(_)) => {
                Err(invalid("„verschoben“ braucht genau ein Ziel: target_key oder target_idea, nicht beide."))
            }
        },
        "done" => Ok(StatusChange::Done { reason }),
        other => {
            Err(invalid(format!("status: unbekannter Status „{other}“ (open, adopted, rejected, deferred, done)")))
        }
    }
}

/// Result of the mirror into the answer: `commit` for a new commit, `mirror` on error.
fn add_mirror(answer: &mut Value, result: Result<Option<String>, String>) {
    match result {
        Ok(Some(commit)) => answer["commit"] = json!(commit),
        Ok(None) => {}
        Err(e) => answer["mirror"] = json!(e),
    }
}

/// Commit for an action on a finding (under the held lock).
async fn record(
    state: &AppState,
    held: &mirror::WriteGuard<'_>,
    work: WorkId,
    finding: &baruch_store::Finding,
    entry: &baruch_store::FindingStatusEntry,
    action: mirror::FindingAction,
) -> Result<Option<String>, String> {
    match state.store.load_work(work).await {
        Ok(loaded) => {
            let request = mirror::describe_finding(&loaded, finding, entry, action);
            mirror::record_finding(state, held, work, request).await
        }
        Err(e) => Err(e.to_string()),
    }
}

/// Set the status (the human): under the write lock, exactly one commit (`Baruch-Action: finding-status`).
pub async fn set_status(state: &AppState, work: WorkId, id: i64, change: &StatusChange) -> Result<Value, RpcError> {
    let held = state.write_lock.lock().await;
    let (before, after) = state.store.finding_set_status(work, id, change, mirror::HUMAN_NAME).await?;
    let history = state.store.finding_history(work, id).await?;
    let mut answer = json!({ "finding": finding_json(work, &after), "before": finding_json(work, &before) });
    let result = match history.last() {
        Some(entry) => record(state, &held, work, &after, entry, mirror::FindingAction::Status).await,
        None => Err("Verlauf des Befunds fehlt nach dem Setzen".to_owned()),
    };
    add_mirror(&mut answer, result);
    Ok(answer)
}

/// Status proposal of an assistant (persona or MCP client): under the write lock, exactly one commit
/// (`Baruch-Action: finding-propose`, author and trailer `Persona` = origin). The store refuses rejected and open.
pub async fn propose(
    state: &AppState,
    work: WorkId,
    id: i64,
    change: &StatusChange,
    by: &IdeaOrigin,
    note: &str,
) -> Result<Value, RpcError> {
    let held = state.write_lock.lock().await;
    propose_held(state, &held, work, id, change, by, note).await
}

/// Like [`propose`], the caller already holds the write lock (Round 22: feedback „Erledigt: Befund n“ when
/// storing a persona post in `assist::save`).
pub async fn propose_held(
    state: &AppState,
    held: &mirror::WriteGuard<'_>,
    work: WorkId,
    id: i64,
    change: &StatusChange,
    by: &IdeaOrigin,
    note: &str,
) -> Result<Value, RpcError> {
    let entry = state.store.finding_propose_status(work, id, change, by, note).await?;
    let finding = state.store.finding_get(work, id).await?;
    let mut answer = json!({ "finding": finding_json(work, &finding), "entry": entry });
    let result = record(state, held, work, &finding, &entry, mirror::FindingAction::Proposed).await;
    add_mirror(&mut answer, result);
    Ok(answer)
}

/// Decide a proposal (the human): under the write lock, exactly one commit
/// (`Baruch-Action: finding-decide`). A stale proposal becomes `stale`, the finding stays.
pub async fn decide(state: &AppState, work: WorkId, entry: i64, confirm: bool) -> Result<Value, RpcError> {
    let held = state.write_lock.lock().await;
    let (finding, entry) = state.store.finding_decide(work, entry, confirm, mirror::HUMAN_NAME).await?;
    let mut answer = json!({ "finding": finding_json(work, &finding), "entry": entry });
    let result = record(state, &held, work, &finding, &entry, mirror::FindingAction::Decided).await;
    add_mirror(&mut answer, result);
    Ok(answer)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_change_requires_the_fields_of_each_status() {
        let ok = |v: Value| parse_change(&v).unwrap();
        let bad = |v: Value| parse_change(&v).unwrap_err().code;
        assert_eq!(ok(json!({"status": "adopted", "rev": "3"})), StatusChange::Adopted { rev: 3 }, "rev aus Formular");
        assert_eq!(bad(json!({"status": "adopted"})), INVALID_PARAMS, "übernommen ohne Fassung");
        assert_eq!(bad(json!({"status": "rejected", "reason": "  "})), INVALID_PARAMS, "verworfen ohne Begründung");
        assert_eq!(bad(json!({"status": "deferred"})), INVALID_PARAMS, "verschoben ohne Ziel");
        assert_eq!(
            bad(json!({"status": "deferred", "target_key": 1, "target_idea": 2})),
            INVALID_PARAMS,
            "verschoben mit zwei Zielen"
        );
        assert_eq!(
            ok(json!({"status": "deferred", "target_idea": 2, "reason": "später"})),
            StatusChange::Deferred { target: DeferTarget::Idea { id: 2 }, reason: "später".into() },
            "Einfall als Ziel"
        );
        assert_eq!(bad(json!({"status": "weg"})), INVALID_PARAMS, "unbekannter Status");
        assert_eq!(ok(json!({"status": "done"})), StatusChange::Done { reason: String::new() }, "Begründung wahlweise");
    }
}
