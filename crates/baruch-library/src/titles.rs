// SPDX-License-Identifier: AGPL-3.0-or-later
//! Titel und Zitate der Bibliothek.

use sqlx::SqlitePool;

use crate::LibraryError;

#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow)]
pub struct Title {
    pub id: i64,
    pub key: String,
    pub kind: String,
    pub title: String,
    pub authors: String,
    pub year: Option<i64>,
    pub publisher: String,
    pub place: String,
    pub url: String,
    pub note: String,
}

#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow)]
pub struct Quote {
    pub id: i64,
    pub title_id: i64,
    pub position: i64,
    pub text: String,
    pub page: String,
    pub note: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct TitleEntry {
    #[serde(flatten)]
    pub title: Title,
    pub quotes: Vec<Quote>,
}

/// Neuer Titel; `kind` ist eines von book, article, chapter, bible, web, other (prüft die DB).
#[derive(Debug, Clone)]
pub struct NewTitle {
    pub key: String,
    pub kind: String,
    pub title: String,
    pub authors: String,
    pub year: Option<i64>,
    pub publisher: String,
    pub place: String,
    pub url: String,
    pub note: String,
}

const TITLE_COLUMNS: &str = "id, key, kind, title, authors, year, publisher, place, url, note";
const QUOTE_COLUMNS: &str = "id, title_id, position, text, page, note";

/// Alle Titel mit ihren Zitaten, sortiert nach Autoren, dann Titel.
pub async fn list_titles(pool: &SqlitePool) -> Result<Vec<TitleEntry>, LibraryError> {
    let mut conn = pool.acquire().await?;
    let titles: Vec<Title> = sqlx::query_as(&format!(
        "SELECT {TITLE_COLUMNS} FROM library_title \
         ORDER BY authors COLLATE NOCASE, title COLLATE NOCASE, id"
    ))
    .fetch_all(&mut *conn)
    .await?;
    let quotes: Vec<Quote> =
        sqlx::query_as(&format!("SELECT {QUOTE_COLUMNS} FROM library_quote ORDER BY title_id, position"))
            .fetch_all(&mut *conn)
            .await?;
    let mut by_title: std::collections::HashMap<i64, Vec<Quote>> = std::collections::HashMap::new();
    for quote in quotes {
        by_title.entry(quote.title_id).or_default().push(quote);
    }
    Ok(titles
        .into_iter()
        .map(|title| {
            let quotes = by_title.remove(&title.id).unwrap_or_default();
            TitleEntry { title, quotes }
        })
        .collect())
}

/// Ein Titel mit Zitaten; unbekannte id → [`LibraryError::UnknownTitle`].
pub async fn get_title(pool: &SqlitePool, id: i64) -> Result<TitleEntry, LibraryError> {
    let mut conn = pool.acquire().await?;
    let title: Option<Title> = sqlx::query_as(&format!("SELECT {TITLE_COLUMNS} FROM library_title WHERE id = ?1"))
        .bind(id)
        .fetch_optional(&mut *conn)
        .await?;
    let title = title.ok_or(LibraryError::UnknownTitle(id))?;
    let quotes: Vec<Quote> =
        sqlx::query_as(&format!("SELECT {QUOTE_COLUMNS} FROM library_quote WHERE title_id = ?1 ORDER BY position"))
            .bind(id)
            .fetch_all(&mut *conn)
            .await?;
    Ok(TitleEntry { title, quotes })
}

/// Legt einen Titel an und gibt seine id zurück (doppelter Schlüssel → Datenbankfehler).
pub async fn add_title(pool: &SqlitePool, title: &NewTitle) -> Result<i64, LibraryError> {
    let mut conn = pool.acquire().await?;
    insert_title(&mut conn, title).await
}

pub(crate) async fn insert_title(conn: &mut sqlx::SqliteConnection, title: &NewTitle) -> Result<i64, LibraryError> {
    let id = sqlx::query_scalar(
        "INSERT INTO library_title (key, kind, title, authors, year, publisher, place, url, note) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9) RETURNING id",
    )
    .bind(&title.key)
    .bind(&title.kind)
    .bind(&title.title)
    .bind(&title.authors)
    .bind(title.year)
    .bind(&title.publisher)
    .bind(&title.place)
    .bind(&title.url)
    .bind(&title.note)
    .fetch_one(&mut *conn)
    .await?;
    Ok(id)
}

/// Hängt ein Zitat an den Titel an (Position = bisher höchste + 1) und gibt seine id zurück.
pub async fn add_quote(
    pool: &SqlitePool,
    title_id: i64,
    text: &str,
    page: &str,
    note: &str,
) -> Result<i64, LibraryError> {
    let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
    let id = insert_quote(&mut tx, title_id, text, page, note).await?;
    tx.commit().await?;
    Ok(id)
}

pub(crate) async fn insert_quote(
    conn: &mut sqlx::SqliteConnection,
    title_id: i64,
    text: &str,
    page: &str,
    note: &str,
) -> Result<i64, LibraryError> {
    let exists: Option<i64> = sqlx::query_scalar("SELECT id FROM library_title WHERE id = ?1")
        .bind(title_id)
        .fetch_optional(&mut *conn)
        .await?;
    if exists.is_none() {
        return Err(LibraryError::UnknownTitle(title_id));
    }
    let id = sqlx::query_scalar(
        "INSERT INTO library_quote (title_id, position, text, page, note) \
         SELECT ?1, coalesce(max(position), 0) + 1, ?2, ?3, ?4 FROM library_quote WHERE title_id = ?1 \
         RETURNING id",
    )
    .bind(title_id)
    .bind(text)
    .bind(page)
    .bind(note)
    .fetch_one(&mut *conn)
    .await?;
    Ok(id)
}
