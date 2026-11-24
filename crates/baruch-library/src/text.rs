// SPDX-License-Identifier: AGPL-3.0-or-later
//! Lokaler Volltext eines Titels (Runde 21a, Migration `0108_title_text.sql`, Tabelle `library_title_text`): der
//! Text einer als Quelle importierten PDF- oder EPUB-Datei, gelesen ohne MultiGPT. Ein Titel hat höchstens einen
//! Volltext; er verschwindet mit dem Titel. Die Dokumentansicht zeigt ihn in Teilen, und Hervorhebungen ohne Zitat
//! lassen sich gegen ihn prüfen ([`crate::check_range`]) – wie bei einer Verknüpfung mit MultiGPT ([`crate::RemoteRef`]).

use sqlx::SqlitePool;

use crate::LibraryError;

/// Lokaler Volltext eines Titels.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct TitleText {
    /// Dateiname der importierten Datei (ohne Ordner).
    pub source_name: String,
    /// `application/pdf` oder `application/epub+zip`.
    pub mime: String,
    /// SHA-256 der Datei (Kleinbuchstaben-Hex).
    pub sha256: String,
    /// Seitenzahl, falls bekannt (PDF).
    pub pages: Option<i64>,
    pub text: String,
}

/// Volltext setzen bzw. ersetzen. Unbekannter Titel → [`LibraryError::UnknownTitle`].
pub async fn set_title_text(pool: &SqlitePool, title_id: i64, text: &TitleText) -> Result<(), LibraryError> {
    let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
    let exists: Option<i64> = sqlx::query_scalar("SELECT id FROM library_title WHERE id = ?1")
        .bind(title_id)
        .fetch_optional(&mut *tx)
        .await?;
    if exists.is_none() {
        return Err(LibraryError::UnknownTitle(title_id));
    }
    sqlx::query(
        "INSERT INTO library_title_text (title_id, source_name, mime, sha256, pages, text) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6) \
         ON CONFLICT (title_id) DO UPDATE SET source_name = excluded.source_name, mime = excluded.mime, \
         sha256 = excluded.sha256, pages = excluded.pages, text = excluded.text, \
         created = strftime('%Y-%m-%dT%H:%M:%fZ','now')",
    )
    .bind(title_id)
    .bind(&text.source_name)
    .bind(&text.mime)
    .bind(&text.sha256)
    .bind(text.pages)
    .bind(&text.text)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(())
}

/// Volltext eines Titels; `None` ohne (auch bei unbekanntem Titel).
pub async fn title_text(pool: &SqlitePool, title_id: i64) -> Result<Option<TitleText>, LibraryError> {
    let row: Option<(String, String, String, Option<i64>, String)> =
        sqlx::query_as("SELECT source_name, mime, sha256, pages, text FROM library_title_text WHERE title_id = ?1")
            .bind(title_id)
            .fetch_optional(pool)
            .await?;
    Ok(row.map(|(source_name, mime, sha256, pages, text)| TitleText { source_name, mime, sha256, pages, text }))
}

/// Zeitpunkt (UTC, `2026-11-09T10:11:12.123Z`), an dem der Volltext gespeichert wurde; `None` ohne.
pub async fn title_text_created(pool: &SqlitePool, title_id: i64) -> Result<Option<String>, LibraryError> {
    Ok(sqlx::query_scalar("SELECT created FROM library_title_text WHERE title_id = ?1")
        .bind(title_id)
        .fetch_optional(pool)
        .await?)
}

/// Titel, dessen Volltext aus einer Datei mit diesem SHA-256 stammt (kleinste id); `None`, wenn keiner.
pub async fn title_for_text_sha256(pool: &SqlitePool, sha256: &str) -> Result<Option<i64>, LibraryError> {
    Ok(sqlx::query_scalar("SELECT title_id FROM library_title_text WHERE sha256 = ?1 ORDER BY title_id LIMIT 1")
        .bind(sha256)
        .fetch_optional(pool)
        .await?)
}
