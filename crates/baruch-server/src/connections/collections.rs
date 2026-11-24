// SPDX-License-Identifier: AGPL-3.0-or-later
//! Sammlungen je Werk für pgvector und MultiGPT (Vertrag Runde 12, §6; Aufgabe „pgvector“): RPC
//! `connection.collections_get` und `connection.collections_set` (aus `connections::call`, nur Mensch –
//! kein MCP-Werkzeug, nicht über `POST /act`).
//!
//! - `connection.collections_get {work?, way}` → `{work, way, collections, available: [{name, count}],
//!   available_at}`: `available` sind die Sammlungen aus der letzten Prüfung der Verbindung
//!   (`details.collections`), `available_at` deren Zeitpunkt (`null` = nie geprüft).
//! - `connection.collections_set {work?, way, collections: [..]}` → `{work, way, collections}`: ersetzt die
//!   Wahl (beschnitten, ohne Doppel, höchstens 100, je 1–200 Zeichen in einer Zeile; sonst -32602, nichts
//!   geändert).
//!
//! `way` ist `pgvector` oder `multigpt`; `lancedb` (keine Sammlungen) und alles andere → -32602.

use baruch_store::{ConnectionKind, RagWay, StoreError, WorkId};
use serde_json::{Value, json};

use super::Fault;
use crate::AppState;

/// Remote-Weg aus einem Text (`pgvector`, `multigpt`); sonst `Invalid("weg")`.
pub(crate) fn remote_way(way: &str) -> Result<RagWay, Fault> {
    match way.trim() {
        "pgvector" => Ok(RagWay::Pgvector),
        "multigpt" => Ok(RagWay::Multigpt),
        _ => Err(Fault::Invalid("weg", "way: pgvector oder multigpt (nur diese Wege haben Sammlungen).".into())),
    }
}

/// Verbindung, deren Prüfung die Sammlungen meldet.
pub(crate) fn kind_of(way: RagWay) -> ConnectionKind {
    match way {
        RagWay::Multigpt => ConnectionKind::Multigpt,
        _ => ConnectionKind::Pgvector,
    }
}

/// Sammlungen aus Angaben einer Prüfung: `details.collections = [{name, count}]` (gemeinsames Format von
/// pgvector und MultiGPT, Vertrag §5); fehlerhafte Einträge fallen weg, `count` ist eine Zahl oder `null`.
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

/// Gewählte und gemeldete Sammlungen eines Werks.
pub(crate) async fn get(state: &AppState, work: WorkId, way: RagWay) -> Result<Value, Fault> {
    let collections = state.store.work_collections(work, way).await?;
    let row = state.store.connection_get(kind_of(way)).await?;
    let (available, at) = match &row.check {
        Some(check) => (available_of(&check.details), Some(check.at.clone())),
        None => (Vec::new(), None),
    };
    Ok(json!({ "work": work, "way": way, "collections": collections, "available": available, "available_at": at }))
}

/// Wahl ersetzen; ungültige Namen → `Invalid("sammlung")`.
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

/// RPC `connection.collections_*`; `None` = nicht diese Methode.
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
