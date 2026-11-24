// SPDX-License-Identifier: AGPL-3.0-or-later
//! Sammlungen je Werk und Remote-Weg (Migration `0031_rag_collections.sql`, Runde 12, Aufgabe „pgvector“):
//! welche Sammlungen von pgvector bzw. MultiGPT die Bedeutungssuche eines Werks durchsucht. Keine Zeile =
//! keine Sammlung gewählt, der Weg bleibt „aus“. LanceDB hat keine Sammlungen (→ [`StoreError::Refused`]).
//!
//! Reihenfolge: `position` 1..n lückenlos je (Werk, Weg); [`Store::set_work_collections`] ersetzt die ganze
//! Liste in einer Transaktion (`BEGIN IMMEDIATE`). Kein Buchtext, darum kein Commit im Spiegel.

use crate::{RagWay, Store, StoreError, WorkId};

/// Höchstens so viele Sammlungen je Werk und Weg.
const MAX_COLLECTIONS: usize = 100;
/// Längster Name einer Sammlung (Zeichen).
const MAX_COLLECTION_CHARS: usize = 200;

/// Nur die Remote-Wege haben Sammlungen.
fn remote(way: RagWay) -> Result<&'static str, StoreError> {
    match way {
        RagWay::Lancedb => Err(StoreError::Refused("LanceDB hat keine Sammlungen (nur pgvector und MultiGPT).")),
        RagWay::Pgvector | RagWay::Multigpt => Ok(way.as_str()),
    }
}

/// Namen beschneiden, Doppelte entfernen (der erste gewinnt); jeder 1..=200 Zeichen in einer Zeile,
/// höchstens 100 Namen, sonst `Refused`.
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
    /// Gewählte Sammlungen eines Werks für einen Remote-Weg, in ihrer Reihenfolge (leer = keine gewählt).
    /// `RagWay::Lancedb` → `Refused`; unbekanntes Werk → `UnknownWork`.
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

    /// Sammlungen eines Werks für einen Remote-Weg ersetzen ([`clean_collections`]; leer = keine). Liefert
    /// die gespeicherte Liste. `RagWay::Lancedb` oder ungültige Namen → `Refused` (nichts geändert);
    /// unbekanntes Werk → `UnknownWork`.
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
