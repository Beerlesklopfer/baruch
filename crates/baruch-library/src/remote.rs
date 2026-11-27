// SPDX-License-Identifier: AGPL-3.0-or-later
//! Link of a title of the library with a document outside (Round 13, migration `0106_source_remote.sql`, table
//! `library_title_remote`): today only MultiGPT (`way = 'multigpt'`). Through the link the document view shows the
//! full text, and highlights without a quote can be checked against it. A title has at most one link; it disappears
//! with the title.

use sqlx::SqlitePool;

use crate::LibraryError;

/// Place of a document outside the library.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct RemoteRef {
    /// Today only `"multigpt"` (checked by the database).
    pub way: String,
    /// Collection in MultiGPT (empty = unknown).
    pub collection: String,
    /// Document ID of the remote side (1–500 characters).
    pub document: String,
}

/// Set or replace the link. Unknown title → [`LibraryError::UnknownTitle`]; invalid way or empty/too long document
/// → database error (CHECK).
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

/// Link of a title; `None` without a link (also for an unknown title).
pub async fn title_remote(pool: &SqlitePool, title_id: i64) -> Result<Option<RemoteRef>, LibraryError> {
    let row: Option<(String, String, String)> =
        sqlx::query_as("SELECT way, collection, document FROM library_title_remote WHERE title_id = ?1")
            .bind(title_id)
            .fetch_optional(pool)
            .await?;
    Ok(row.map(|(way, collection, document)| RemoteRef { way, collection, document }))
}

/// Title that is linked with this document (way, collection, document equal); `None` if there is none. An empty
/// collection matches any (older links without a collection).
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
