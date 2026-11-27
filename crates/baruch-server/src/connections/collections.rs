// SPDX-License-Identifier: AGPL-3.0-or-later
//! Collections per work for pgvector and MultiGPT (contract Round 12, §6; task “pgvector”): RPC
//! `connection.collections_get` and `connection.collections_set` (from `connections::call`, human only –
//! no MCP tool, not through `POST /act`).
//!
//! - `connection.collections_get {work?, way}` → `{work, way, collections, available: [{name, count}],
//!   available_at}`: `available` are the collections from the last check of the connection
//!   (`details.collections`), `available_at` its time (`null` = never checked).
//! - `connection.collections_set {work?, way, collections: [..]}` → `{work, way, collections}`: replaces the
//!   choice (trimmed, no duplicates, at most 100, 1–200 characters each in one line; otherwise -32602, nothing
//!   changed).
//!
//! `way` is `pgvector` or `multigpt`; `lancedb` (no collections) and everything else → -32602.

use baruch_store::{ConnectionKind, RagWay, StoreError, WorkId};
use serde_json::{Value, json};

use super::Fault;
use crate::AppState;

/// Remote way from a text (`pgvector`, `multigpt`); otherwise `Invalid("weg")`.
pub(crate) fn remote_way(way: &str) -> Result<RagWay, Fault> {
    match way.trim() {
        "pgvector" => Ok(RagWay::Pgvector),
        "multigpt" => Ok(RagWay::Multigpt),
        _ => Err(Fault::Invalid("weg", "way: pgvector oder multigpt (nur diese Wege haben Sammlungen).".into())),
    }
}

/// Connection whose check reports the collections.
pub(crate) fn kind_of(way: RagWay) -> ConnectionKind {
    match way {
        RagWay::Multigpt => ConnectionKind::Multigpt,
        _ => ConnectionKind::Pgvector,
    }
}

/// Collections from details of a check: `details.collections = [{name, count}]` (shared format of
/// pgvector and MultiGPT, contract §5); faulty entries are dropped, `count` is a number or `null`.
pub fn available_of(details: &Value) -> Vec<Value> {
    details
        .get("collections")
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .filter_map(|entry| {
                    let name = entry.get("name").and_then(Value::as_str)?.trim();
                    if name.is_empty() {
                        return None;
                    }
                    let count = entry.get("count").and_then(Value::as_i64);
                    Some(json!({ "name": name, "count": count }))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Chosen and reported collections of a work.
pub(crate) async fn get(state: &AppState, work: WorkId, way: RagWay) -> Result<Value, Fault> {
    let collections = state.store.work_collections(work, way).await?;
    let row = state.store.connection_get(kind_of(way)).await?;
    let (available, at) = match &row.check {
        Some(check) => (available_of(&check.details), Some(check.at.clone())),
        None => (Vec::new(), None),
    };
    Ok(json!({ "work": work, "way": way, "collections": collections, "available": available, "available_at": at }))
}

/// Replace the choice; invalid names → `Invalid("sammlung")`.
pub(crate) async fn set(state: &AppState, work: WorkId, way: RagWay, collections: &[String]) -> Result<Value, Fault> {
    match state.store.set_work_collections(work, way, collections).await {
        Ok(saved) => Ok(json!({ "work": work, "way": way, "collections": saved })),
        Err(StoreError::Refused(message)) => Err(Fault::Invalid("sammlung", message.to_owned())),
        Err(e) => Err(e.into()),
    }
}

fn way_param(params: &Value) -> Result<RagWay, Fault> {
    match params.get("way").and_then(Value::as_str) {
        Some(way) => remote_way(way),
        None => Err(Fault::Invalid("weg", "way fehlt (pgvector oder multigpt).".into())),
    }
}

fn collections_param(params: &Value) -> Result<Vec<String>, Fault> {
    let invalid = || Fault::Invalid("sammlung", "collections: Liste von Texten erwartet.".into());
    let list = params.get("collections").and_then(Value::as_array).ok_or_else(invalid)?;
    list.iter().map(|c| c.as_str().map(str::to_owned).ok_or_else(invalid)).collect()
}

/// RPC `connection.collections_*`; `None` = not this method.
pub(crate) async fn call(state: &AppState, method: &str, params: &Value) -> Option<Result<Value, Fault>> {
    match method {
        "connection.collections_get" => Some(
            async {
                let way = way_param(params)?;
                let work = super::work_param(state, params).await?;
                get(state, work, way).await
            }
            .await,
        ),
        "connection.collections_set" => Some(
            async {
                let way = way_param(params)?;
                let collections = collections_param(params)?;
                let work = super::work_param(state, params).await?;
                set(state, work, way, &collections).await
            }
            .await,
        ),
        _ => None,
    }
}
