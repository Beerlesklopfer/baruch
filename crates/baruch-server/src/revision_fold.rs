// SPDX-License-Identifier: AGPL-3.0-or-later
//! Hiding older revisions (user 2026-11-24, release 0.9.0): mark „bis hierhin“ per section.
//!
//! - RPC (human only; MCP has no tool for it): `revision.fold_get {key|node}` → `{node, key, below, cutoff,
//!   hidden, updated?, updated_by?}`; `revision.fold_set {key|node, rev}` → `{node, key, rev, below, hidden}`;
//!   `revision.fold_clear {key|node, rev?}` → `{node, key, rev, cleared}`; work-wide `revision.fold_before {work?,
//!   date}` (day `JJJJ-MM-TT` in local time, mark per section on the last revision before this day, at most the
//!   current one) → `{work, sections}`; `revision.fold_clear_all {work?}` → `{work, cleared}`.
//! - Forms: `fold_set`/`fold_clear` also through `POST /act` (back to the revision); work-wide
//!   `POST /einstellungen/werk/fassungen/ausblenden` (`work`, `date`, `do=fold|clear`) → 303 to
//!   `/einstellungen/werk?work=W&ok=ausgeblendet#fassungen` or `ok=eingeblendet`, rejected `ausblendenfehler=1`.
//! - What is hidden is governed by `baruch_core::revision_fold` (never the current revision, never open proposals, a
//!   mark above the current one only holds up to below the current one). Nothing is deleted, no commit in the
//!   mirror (no book text); logged in the database (`node_revision_fold.updated`/`updated_by`) like the setting
//!   „Fassungen“.

use std::sync::Arc;

use axum::Router;
use axum::extract::{Form, State};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::post;
use baruch_core::revision_fold;
use baruch_store::{StoreError, WorkId};
use chrono::{Local, NaiveDate, TimeZone, Utc};
use serde_json::{Value, json};

use crate::i18n::tc;
use crate::rpc::{INVALID_PARAMS, RpcError, resolve_keys};
use crate::{AppState, mirror};

/// RPC methods of this module.
pub fn handles(method: &str) -> bool {
    matches!(
        method,
        "revision.fold_get"
            | "revision.fold_set"
            | "revision.fold_clear"
            | "revision.fold_before"
            | "revision.fold_clear_all"
    )
}

fn invalid(message: impl Into<String>) -> RpcError {
    RpcError::new(INVALID_PARAMS, message)
}

fn number(params: &Value, key: &str) -> Result<Option<u64>, RpcError> {
    let wrong = || invalid(tc("outline_msg.param.number", &[("name", key.into())]));
    match params.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Number(n)) => n.as_u64().map(Some).ok_or_else(wrong),
        Some(Value::String(s)) if s.trim().is_empty() => Ok(None),
        Some(Value::String(s)) => s.trim().parse().map(Some).map_err(|_| wrong()),
        Some(_) => Err(wrong()),
    }
}

fn node_of(params: &Value) -> Result<usize, RpcError> {
    let value = number(params, "node")?.ok_or_else(|| invalid(tc("outline_msg.revision.node_missing", &[])))?;
    usize::try_from(value).map_err(|_| invalid(tc("outline_msg.param.too_large", &[("name", "node".into())])))
}

/// Day `JJJJ-MM-TT` (local time, midnight) → UTC instant in the format of `revision.created`.
pub fn day_start_utc(date: &str) -> Result<String, RpcError> {
    let day = NaiveDate::parse_from_str(date.trim(), "%Y-%m-%d")
        .map_err(|_| invalid(tc("outline_msg.fold.date_format", &[])))?;
    let midnight = day.and_hms_opt(0, 0, 0).ok_or_else(|| invalid(tc("outline_msg.fold.date_invalid", &[])))?;
    let local = Local
        .from_local_datetime(&midnight)
        .earliest()
        .ok_or_else(|| invalid(tc("outline_msg.fold.date_invalid", &[])))?;
    Ok(local.with_timezone(&Utc).format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string())
}

/// State of the mark of a section as JSON.
async fn fold_state(state: &AppState, work: WorkId, node: usize) -> Result<Value, RpcError> {
    let store = &state.store;
    let fold = store.revision_fold(work, node).await?;
    let loaded = store.load_work(work).await?;
    let n = loaded.nodes.get(node).ok_or(StoreError::UnknownNode { work, node })?;
    let mark = fold.as_ref().map(|f| f.below);
    let mut answer = json!({
        "node": node,
        "key": n.key,
        "below": mark,
        "cutoff": revision_fold::cutoff(n, mark),
        "hidden": revision_fold::hidden_count(n, mark),
    });
    if let Some(f) = fold {
        answer["updated"] = json!(f.updated);
        answer["updated_by"] = json!(f.updated_by);
    }
    Ok(answer)
}

/// Answers a method from [`handles`].
pub async fn call(state: &AppState, work: WorkId, method: &str, params: Value) -> Result<Value, RpcError> {
    let store = &state.store;
    match method {
        "revision.fold_get" => {
            let params = resolve_keys(state, work, params).await?;
            fold_state(state, work, node_of(&params)?).await
        }
        "revision.fold_set" => {
            let _held = state.write_lock.lock().await;
            let params = resolve_keys(state, work, params).await?;
            let node = node_of(&params)?;
            let rev = number(&params, "rev")?
                .ok_or_else(|| invalid(tc("outline_msg.param.missing", &[("name", "rev".into())])))?;
            let rev = u32::try_from(rev)
                .map_err(|_| invalid(tc("outline_msg.param.too_large", &[("name", "rev".into())])))?;
            store.set_revision_fold(work, node, rev, mirror::HUMAN_NAME).await?;
            let mut answer = fold_state(state, work, node).await?;
            answer["rev"] = json!(rev);
            Ok(answer)
        }
        "revision.fold_clear" => {
            let _held = state.write_lock.lock().await;
            let params = resolve_keys(state, work, params).await?;
            let node = node_of(&params)?;
            let cleared = store.clear_revision_fold(work, node).await?;
            let mut answer = fold_state(state, work, node).await?;
            answer["cleared"] = json!(cleared);
            // Back to the shown revision (form through `/act`).
            if let Some(rev) = number(&params, "rev")? {
                answer["rev"] = json!(rev);
            }
            Ok(answer)
        }
        "revision.fold_before" => {
            let _held = state.write_lock.lock().await;
            let date = params
                .get("date")
                .and_then(Value::as_str)
                .ok_or_else(|| invalid(tc("outline_msg.fold.date_missing", &[])))?;
            let before = day_start_utc(date)?;
            let done = store.fold_revisions_before(work, &before, mirror::HUMAN_NAME).await?;
            let sections: Vec<Value> =
                done.set.iter().map(|(key, below)| json!({ "key": key, "below": below })).collect();
            Ok(json!({ "work": work, "date": date.trim(), "sections": sections }))
        }
        "revision.fold_clear_all" => {
            let _held = state.write_lock.lock().await;
            let cleared = store.clear_work_revision_folds(work).await?;
            Ok(json!({ "work": work, "cleared": cleared }))
        }
        _ => Err(RpcError::new(crate::rpc::METHOD_NOT_FOUND, format!("unknown method {method}"))),
    }
}

fn pair<'a>(pairs: &'a [(String, String)], name: &str) -> Option<&'a str> {
    pairs.iter().find(|(k, _)| k == name).map(|(_, v)| v.trim()).filter(|v| !v.is_empty())
}

/// `POST /einstellungen/werk/fassungen/ausblenden` (behind `guard::same_origin`): `work`, `date` (JJJJ-MM-TT) and
/// `do=fold` (default) or `do=clear` (lift all marks of the work).
async fn settings_act(State(state): State<Arc<AppState>>, Form(pairs): Form<Vec<(String, String)>>) -> Response {
    let work = match pair(&pairs, "work").and_then(|w| w.parse::<WorkId>().ok()).filter(|&w| w > 0) {
        Some(w) => match state.store.work_slug(w).await {
            Ok(_) => w,
            Err(_) => return Redirect::to("/einstellungen/werk").into_response(),
        },
        None => match state.default_work().await {
            Ok(w) => w,
            Err(_) => return Redirect::to("/einstellungen/werk").into_response(),
        },
    };
    let failed = format!("/einstellungen/werk?work={work}&ausblendenfehler=1#fassungen");
    let result = match pair(&pairs, "do") {
        Some("clear") => {
            call(&state, work, "revision.fold_clear_all", json!({ "work": work })).await.map(|_| "eingeblendet")
        }
        None | Some("fold") => {
            let date = pair(&pairs, "date").unwrap_or("");
            call(&state, work, "revision.fold_before", json!({ "work": work, "date": date }))
                .await
                .map(|_| "ausgeblendet")
        }
        Some(_) => return Redirect::to(&failed).into_response(),
    };
    match result {
        Ok(ok) => Redirect::to(&format!("/einstellungen/werk?work={work}&ok={ok}#fassungen")).into_response(),
        Err(_) => Redirect::to(&failed).into_response(),
    }
}

/// Writing forms (POST): work-wide hiding in the settings.
pub fn writes() -> Router<Arc<AppState>> {
    Router::new().route("/einstellungen/werk/fassungen/ausblenden", post(settings_act))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_day_becomes_local_midnight_in_utc() {
        // Format like revision.created, so that the text comparison in the store is right.
        let stamp = day_start_utc("2026-11-24").unwrap();
        assert_eq!(stamp.len(), "2026-11-24T00:00:00.000Z".len(), "{stamp}");
        assert!(stamp.ends_with('Z'), "{stamp}");
        assert!(day_start_utc("24.11.2026").is_err(), "nur JJJJ-MM-TT");
        assert!(day_start_utc("").is_err(), "leer abgelehnt");
    }
}
