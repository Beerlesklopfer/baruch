// SPDX-License-Identifier: AGPL-3.0-or-later
//! Verknüpfung eines Titels der Bibliothek mit einem Dokument außerhalb (Runde 13, Migration
//! `0106_source_remote.sql`, Tabelle `library_title_remote`): heute nur MultiGPT (`way = 'multigpt'`).
//! Über die Verknüpfung zeigt die Dokumentansicht den Volltext, und Hervorhebungen ohne Zitat lassen sich
//! gegen ihn prüfen. Ein Titel hat höchstens eine Verknüpfung; sie verschwindet mit dem Titel.

use sqlx::SqlitePool;

use crate::LibraryError;

/// Ort eines Dokuments außerhalb der Bibliothek.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct RemoteRef {
    /// Heute nur `"multigpt"` (prüft die Datenbank).
    pub way: String,
    /// Sammlung in MultiGPT (leer = unbekannt).
    pub collection: String,
    /// Dokument-ID der Gegenstelle (1–500 Zeichen).
    pub document: String,
}

/// Verknüpfung setzen bzw. ersetzen. Unbekannter Titel → [`LibraryError::UnknownTitle`]; ungültiger Weg
/// oder leeres/zu langes Dokument → Datenbankfehler (CHECK).
pub async fn set_title_remote(pool: &SqlitePool, title_id: i64, remote: &RemoteRef) -> Result<(), LibraryError> {
    let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
    let exists: Option<i64> = sqlx::query_scalar("SELECT id FROM library_title WHERE id = ?1")
        .bind(title_id)
        .fetch_optional(&mut *tx)
        .await?;
    if exists.is_none() {
        return Err(LibraryError::UnknownTitle(title_id));
    }
    sqlx::query(
        "INSERT INTO library_title_remote (title_id, way, collection, document) VALUES (?1, ?2, ?3, ?4) \
         ON CONFLICT (title_id) DO UPDATE SET way = excluded.way, collection = excluded.collection, \
         document = excluded.document",
    )
    .bind(title_id)
    .bind(&remote.way)
    .bind(&remote.collection)
    .bind(&remote.document)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(())
}

/// Verknüpfung eines Titels; `None` ohne Verknüpfung (auch bei unbekanntem Titel).
pub async fn title_remote(pool: &SqlitePool, title_id: i64) -> Result<Option<RemoteRef>, LibraryError> {
    let row: Option<(String, String, String)> =
        sqlx::query_as("SELECT way, collection, document FROM library_title_remote WHERE title_id = ?1")
            .bind(title_id)
            .fetch_optional(pool)
            .await?;
    Ok(row.map(|(way, collection, document)| RemoteRef { way, collection, document }))
}

/// Titel, der mit diesem Dokument verknüpft ist (Weg, Sammlung, Dokument gleich); `None`, wenn keiner.
/// Eine leere Sammlung passt zu jeder (ältere Verknüpfungen ohne Sammlung).
pub async fn title_for_remote(pool: &SqlitePool, remote: &RemoteRef) -> Result<Option<i64>, LibraryError> {
    let id: Option<i64> = sqlx::query_scalar(
        "SELECT title_id FROM library_title_remote WHERE way = ?1 AND document = ?3 \
           AND (collection = ?2 OR collection = '' OR ?2 = '') ORDER BY collection = ?2 DESC, title_id LIMIT 1",
    )
    .bind(&remote.way)
    .bind(&remote.collection)
    .bind(&remote.document)
    .fetch_optional(pool)
    .await?;
    Ok(id)
}
