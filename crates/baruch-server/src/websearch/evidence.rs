// SPDX-License-Identifier: AGPL-3.0-or-later
//! Belege der Websuche in `web_evidence` (Migration 0034): [`record`] nach dem Speichern des Beitrags (eine
//! Zeile je Zitat bzw. genannter Seite, mit `post_id`), [`for_post`] und [`get`] zum Lesen und [`adopt`] für
//! die Übernahme in die Autorenbibliothek – nur bei `belegt`, nur durch den Menschen (RPC bzw. Formular, kein
//! MCP). Übernommen werden über die vorhandene API von `baruch-library` ein Titel der Art `web` (mit URL und
//! der Notiz „Abgerufen am <Datum> (Websuche @belege)“; eine schon übernommene Seite mit derselben Adresse
//! bekommt keinen zweiten Titel), das Zitat als `library_quote` und eine Hervorhebung der Art `quote` vom
//! Menschen. Danach meldet sich die Bibliothek beim gemeinsamen Index ([`crate::rag::notify`]).

use baruch_library::{HighlightAuthor, HighlightKind, NewHighlight, NewTitle};
use baruch_store::WorkId;
use serde::Serialize;
use serde_json::{Value, json};
use sqlx::SqlitePool;

use super::search::host_of;
use super::verify::Draft;
use super::{Research, day_of};
use crate::rpc::{INVALID_PARAMS, RpcError};
use crate::{AppState, mirror};

/// Eine Zeile aus `web_evidence`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, sqlx::FromRow)]
pub struct Evidence {
    pub id: i64,
    pub post: Option<i64>,
    pub work: Option<i64>,
    pub key: Option<i64>,
    pub persona: String,
    /// „W1“ …
    pub label: String,
    pub url: String,
    pub title: String,
    pub preferred: bool,
    /// Zeitpunkt des Abrufs (UTC, ISO).
    pub accessed: String,
    pub quote: String,
    /// `belegt` | `nicht_belegt` | `nicht_zitiert`.
    pub verdict: String,
    /// Titel in der Autorenbibliothek nach der Übernahme.
    pub library_title: Option<i64>,
    pub created: String,
}

const COLUMNS: &str = "id, post_id AS post, work_id AS work, node_key AS key, persona, label, url, title, \
    preferred, accessed, quote, verdict, library_title_id AS library_title, created";

/// Belege eines gespeicherten Beitrags anlegen; gibt die Zahl der Zeilen zurück.
pub async fn record(
    pool: &SqlitePool,
    post: i64,
    work: WorkId,
    key: i64,
    persona: &str,
    research: &Research,
    drafts: &[Draft],
) -> Result<usize, sqlx::Error> {
    let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
    let mut count = 0;
    for draft in drafts {
        let Some(page) = research.page(draft.label) else { continue };
        sqlx::query(
            "INSERT INTO web_evidence (post_id, work_id, node_key, persona, label, url, title, preferred, accessed, \
             quote, verdict, page_hash) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(post)
        .bind(work)
        .bind(key)
        .bind(persona)
        .bind(format!("W{}", page.label))
        .bind(&page.url)
        .bind(&page.title)
        .bind(page.preferred)
        .bind(&page.accessed)
        .bind(&draft.quote)
        .bind(draft.verdict.as_str())
        // Fingerabdruck der geprüften Seite (SHA-256 aus baruch-store), damit eine spätere Prüfung erkennt,
        // ob sich die Seite seit dem Abruf geändert hat.
        .bind(baruch_store::sync_hash(&page.text))
        .execute(&mut *tx)
        .await?;
        count += 1;
    }
    tx.commit().await?;
    Ok(count)
}

/// Belege eines Beitrags in der Reihenfolge des Anlegens.
pub async fn for_post(pool: &SqlitePool, post: i64) -> Result<Vec<Evidence>, sqlx::Error> {
    sqlx::query_as(sqlx::AssertSqlSafe(format!("SELECT {COLUMNS} FROM web_evidence WHERE post_id = ? ORDER BY id")))
        .bind(post)
        .fetch_all(pool)
        .await
}

/// Ein Beleg.
pub async fn get(pool: &SqlitePool, id: i64) -> Result<Option<Evidence>, sqlx::Error> {
    sqlx::query_as(sqlx::AssertSqlSafe(format!("SELECT {COLUMNS} FROM web_evidence WHERE id = ?")))
        .bind(id)
        .fetch_optional(pool)
        .await
}

fn invalid(message: impl Into<String>) -> RpcError {
    RpcError::new(INVALID_PARAMS, message)
}

/// Freier Zitierschlüssel `web-<rechner>-<JJJJMMTT>[-n]` (nur `a-z0-9-`).
async fn free_key(pool: &SqlitePool, url: &str, accessed: &str) -> Result<String, sqlx::Error> {
    let host: String = host_of(url)
        .unwrap_or_else(|| "seite".into())
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    let date: String = accessed.chars().take(10).filter(char::is_ascii_digit).collect();
    let base = format!("web-{}-{date}", host.trim_matches('-'));
    let mut key = base.clone();
    let mut n = 1;
    loop {
        let taken: Option<i64> =
            sqlx::query_scalar("SELECT id FROM library_title WHERE key = ?").bind(&key).fetch_optional(pool).await?;
        if taken.is_none() {
            return Ok(key);
        }
        n += 1;
        key = format!("{base}-{n}");
    }
}

/// Beleg in die Autorenbibliothek übernehmen (siehe Moduldoku): `{title: TitleEntry, quote, highlight}`.
/// Unbekannt, nicht `belegt` oder schon übernommen → -32602, dann ist nichts angelegt.
pub async fn adopt(state: &AppState, id: i64) -> Result<Value, RpcError> {
    let pool = state.store.pool();
    let db = |e: sqlx::Error| RpcError::new(crate::rpc::INTERNAL, format!("Speicher: {e}"));
    let _held = state.write_lock.lock().await;
    let found = get(pool, id).await.map_err(db)?.ok_or_else(|| invalid(format!("Beleg {id} gibt es nicht.")))?;
    if found.verdict != "belegt" {
        return Err(invalid(format!(
            "Beleg {id} ist {}; in die Bibliothek kommen nur Zitate, deren Wortlaut auf der abgerufenen Seite steht.",
            if found.verdict == "nicht_belegt" { "nicht belegt" } else { "ohne Zitat" }
        )));
    }
    if let Some(title) = found.library_title {
        return Err(invalid(format!("Beleg {id} ist schon übernommen (Titel {title}).")));
    }
    if found.quote.trim().is_empty() {
        return Err(invalid(format!("Beleg {id} hat kein Zitat.")));
    }
    let existing: Option<i64> =
        sqlx::query_scalar("SELECT id FROM library_title WHERE kind = 'web' AND url = ? ORDER BY id LIMIT 1")
            .bind(&found.url)
            .fetch_optional(pool)
            .await
            .map_err(db)?;
    let title_id = match existing {
        Some(title) => title,
        None => {
            let new = NewTitle {
                key: free_key(pool, &found.url, &found.accessed).await.map_err(db)?,
                kind: "web".into(),
                title: if found.title.trim().is_empty() { found.url.clone() } else { found.title.clone() },
                authors: String::new(),
                year: None,
                publisher: host_of(&found.url).unwrap_or_default(),
                place: String::new(),
                url: found.url.clone(),
                note: format!("Abgerufen am {} (Websuche @belege)", day_of(&found.accessed)),
            };
            baruch_library::add_title(pool, &new).await?
        }
    };
    let note = format!("Websuche @belege, abgerufen am {}", day_of(&found.accessed));
    let quote_id = baruch_library::add_quote(pool, title_id, found.quote.trim(), "", &note).await?;
    let highlight = NewHighlight {
        title_id,
        quote_id: Some(quote_id),
        locator: String::new(),
        start: None,
        end: None,
        text: found.quote.trim().to_owned(),
        kind: HighlightKind::Quote,
        note: String::new(),
        author: HighlightAuthor::Human { name: mirror::HUMAN_NAME.to_owned() },
    };
    let highlight_id = baruch_library::add_highlight(pool, &highlight).await?;
    sqlx::query("UPDATE web_evidence SET library_title_id = ? WHERE id = ?")
        .bind(title_id)
        .bind(id)
        .execute(pool)
        .await
        .map_err(db)?;
    let title = baruch_library::get_title(pool, title_id).await?;
    let quote = title.quotes.iter().find(|q| q.id == quote_id).cloned();
    let highlight = baruch_library::get_highlight(pool, highlight_id).await?;
    crate::rag::notify(state, None);
    Ok(json!({ "title": title, "quote": quote, "highlight": highlight, "evidence": id }))
}
