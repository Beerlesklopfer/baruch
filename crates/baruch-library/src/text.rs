// SPDX-License-Identifier: AGPL-3.0-or-later
//! Local full text of a title (Round 21a, migration `0108_title_text.sql`, table `library_title_text`): the text of
//! a PDF or EPUB file imported as a source, read without MultiGPT. A title has at most one full text; it disappears
//! with the title. The document view shows it in parts, and highlights without a quote can be checked against it
//! ([`crate::check_range`]) – as with a link with MultiGPT ([`crate::RemoteRef`]).

use sqlx::SqlitePool;

use crate::LibraryError;

/// Local full text of a title.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct TitleText {
    /// File name of the imported file (without folder).
    pub source_name: String,
    /// `application/pdf` or `application/epub+zip`.
    pub mime: String,
    /// SHA-256 of the file (lower-case hex).
    pub sha256: String,
    /// Number of pages, if known (PDF).
    pub pages: Option<i64>,
    pub text: String,
}

/// Set or replace the full text. Unknown title → [`LibraryError::UnknownTitle`].
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

/// Full text of a title; `None` without one (also for an unknown title).
pub async fn title_text(pool: &SqlitePool, title_id: i64) -> Result<Option<TitleText>, LibraryError> {
    let row: Option<(String, String, String, Option<i64>, String)> =
        sqlx::query_as("SELECT source_name, mime, sha256, pages, text FROM library_title_text WHERE title_id = ?1")
            .bind(title_id)
            .fetch_optional(pool)
            .await?;
    Ok(row.map(|(source_name, mime, sha256, pages, text)| TitleText { source_name, mime, sha256, pages, text }))
}

/// Time (UTC, `2026-11-09T10:11:12.123Z`) at which the full text was stored; `None` without one.
pub async fn title_text_created(pool: &SqlitePool, title_id: i64) -> Result<Option<String>, LibraryError> {
    Ok(sqlx::query_scalar("SELECT created FROM library_title_text WHERE title_id = ?1")
        .bind(title_id)
        .fetch_optional(pool)
        .await?)
}

/// Title whose full text comes from a file with this SHA-256 (smallest id); `None` if there is none.
pub async fn title_for_text_sha256(pool: &SqlitePool, sha256: &str) -> Result<Option<i64>, LibraryError> {
    Ok(sqlx::query_scalar("SELECT title_id FROM library_title_text WHERE sha256 = ?1 ORDER BY title_id LIMIT 1")
        .bind(sha256)
        .fetch_optional(pool)
        .await?)
}
