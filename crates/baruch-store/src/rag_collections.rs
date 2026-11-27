// SPDX-License-Identifier: AGPL-3.0-or-later
//! Collections per work and remote way (migration `0031_rag_collections.sql`, Round 12, task "pgvector"): which
//! collections of pgvector or MultiGPT the semantic search of a work searches. No row = no collection chosen, the way
//! stays "off". LanceDB has no collections (→ [`StoreError::Refused`]).
//!
//! Order: `position` 1..n without gaps per (work, way); [`Store::set_work_collections`] replaces the whole list in one
//! transaction (`BEGIN IMMEDIATE`). Not book text, therefore no commit in the mirror.

use crate::{RagWay, Store, StoreError, WorkId};

/// At most this many collections per work and way.
const MAX_COLLECTIONS: usize = 100;
/// Longest name of a collection (characters).
const MAX_COLLECTION_CHARS: usize = 200;

/// Only the remote ways have collections.
fn remote(way: RagWay) -> Result<&'static str, StoreError> {
    match way {
        RagWay::Lancedb => Err(StoreError::Refused("LanceDB hat keine Sammlungen (nur pgvector und MultiGPT).")),
        RagWay::Pgvector | RagWay::Multigpt => Ok(way.as_str()),
    }
}

/// Trim the names, remove duplicates (the first one wins); each 1..=200 characters on one line, at most 100 names,
/// otherwise `Refused`.
fn clean_collections(collections: &[String]) -> Result<Vec<String>, StoreError> {
    let mut out: Vec<String> = Vec::new();
    for name in collections {
        let name = name.trim();
        let chars = name.chars().count();
        if chars == 0 || chars > MAX_COLLECTION_CHARS || name.contains(['\n', '\r']) {
            return Err(StoreError::Refused("Sammlung: 1 bis 200 Zeichen in einer Zeile."));
        }
        if !out.iter().any(|seen| seen == name) {
            out.push(name.to_owned());
        }
    }
    if out.len() > MAX_COLLECTIONS {
        return Err(StoreError::Refused("Höchstens 100 Sammlungen je Werk und Weg."));
    }
    Ok(out)
}

impl Store {
    /// Chosen collections of a work for a remote way, in their order (empty = none chosen). `RagWay::Lancedb` →
    /// `Refused`; unknown work → `UnknownWork`.
    pub async fn work_collections(&self, work: WorkId, way: RagWay) -> Result<Vec<String>, StoreError> {
        let way = remote(way)?;
        let mut conn = self.pool.acquire().await?;
        let found: Option<i64> =
            sqlx::query_scalar("SELECT id FROM work WHERE id = ?").bind(work).fetch_optional(&mut *conn).await?;
        found.ok_or(StoreError::UnknownWork(work))?;
        Ok(sqlx::query_scalar(
            "SELECT collection FROM work_rag_collection WHERE work_id = ? AND way = ? ORDER BY position, collection",
        )
        .bind(work)
        .bind(way)
        .fetch_all(&mut *conn)
        .await?)
    }

    /// Replace the collections of a work for a remote way ([`clean_collections`]; empty = none). Returns the stored
    /// list. `RagWay::Lancedb` or invalid names → `Refused` (nothing changed); unknown work → `UnknownWork`.
    pub async fn set_work_collections(
        &self,
        work: WorkId,
        way: RagWay,
        collections: &[String],
    ) -> Result<Vec<String>, StoreError> {
        let way = remote(way)?;
        let list = clean_collections(collections)?;
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let found: Option<i64> =
            sqlx::query_scalar("SELECT id FROM work WHERE id = ?").bind(work).fetch_optional(&mut *tx).await?;
        found.ok_or(StoreError::UnknownWork(work))?;
        sqlx::query("DELETE FROM work_rag_collection WHERE work_id = ? AND way = ?")
            .bind(work)
            .bind(way)
            .execute(&mut *tx)
            .await?;
        for (index, name) in list.iter().enumerate() {
            sqlx::query("INSERT INTO work_rag_collection (work_id, way, collection, position) VALUES (?, ?, ?, ?)")
                .bind(work)
                .bind(way)
                .bind(name)
                .bind(index as i64 + 1)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(list)
    }
}
