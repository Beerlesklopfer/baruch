// SPDX-License-Identifier: AGPL-3.0-or-later
//! Fäden ohne Abschnitt (Runde 15, Migration 0038): ein Gespräch je Werk, das an keinem Abschnitt hängt.
//!
//! - MCP `chat` ohne `key` legt einen Faden an (erster Beitrag = Absender) bzw. setzt einen fort (`thread`);
//!   der Mensch liest und antwortet auf den Seiten `/faeden` und `/faden` (Server `threads.rs`).
//! - Ein Faden ist kein Buchtext: Beiträge stehen in `chat_thread_post`, nie in `post`, und kommen nicht in den
//!   Spiegel. Jede Änderung ist genau eine Transaktion (`BEGIN IMMEDIATE`).
//! - Angesprochen wird ein Faden über seine id **und** sein Werk; eine id aus einem anderen Werk gilt als
//!   unbekannt ([`UNKNOWN_THREAD`]).
//! - Runde 30c (chat-bedienung): Fadenbeiträge lassen sich löschen ([`Store::thread_post_delete`]); Metadaten
//!   (Antwort auf, Wiederholung, Relay, Auftrag) schreibt `post_meta.rs` (`thread_add_post_meta`).

use crate::{Store, StoreError, WorkId};

/// Meldung für einen Faden, den es im Werk nicht gibt.
pub const UNKNOWN_THREAD: &str = "Diesen Faden gibt es in diesem Werk nicht.";
/// Höchstens so viele Zeichen hat der Titel eines Fadens (aus dem Anfang der ersten Nachricht).
pub const MAX_THREAD_TITLE_CHARS: usize = 80;

/// Ein Faden in der Übersicht.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ThreadSummary {
    pub id: i64,
    pub work: WorkId,
    pub title: String,
    /// Wer ihn begonnen hat („Autor“, `@handle`, `@mcp:<name>`).
    pub created_by: String,
    pub created: String,
    /// Zeit des letzten Beitrags.
    pub updated: String,
    pub posts: i64,
    /// Autor des letzten Beitrags (leer ohne Beiträge).
    pub last_author: String,
}

/// Ein Beitrag in einem Faden.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ThreadPost {
    pub id: i64,
    pub thread: i64,
    pub position: i64,
    pub author: String,
    pub body: String,
    pub sources: Vec<String>,
    pub created: String,
    /// Zeit der letzten Änderung (Runde 28, `chat_thread_post.edited`); `None` = nie bearbeitet.
    pub edited: Option<String>,
}

/// Ein Faden mit allen Beiträgen (älteste zuerst).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Thread {
    #[serde(flatten)]
    pub summary: ThreadSummary,
    pub entries: Vec<ThreadPost>,
}

/// Titel aus dem Anfang einer Nachricht: eine Zeile, höchstens [`MAX_THREAD_TITLE_CHARS`] Zeichen.
pub fn thread_title(message: &str) -> String {
    let flat = message.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.chars().count() <= MAX_THREAD_TITLE_CHARS {
        flat
    } else {
        let mut cut: String = flat.chars().take(MAX_THREAD_TITLE_CHARS - 1).collect();
        cut.push('…');
        cut
    }
}

type SummaryRow = (i64, i64, String, String, String, String, i64, Option<String>);
/// Zeile eines Fadenbeitrags: id, Stelle, Autor, Text, Quellen (JSON), angelegt, bearbeitet.
type PostRow = (i64, i64, String, String, String, String, Option<String>);

const SUMMARY_SELECT: &str = "SELECT t.id, t.work_id, t.title, t.created_by, t.created, t.updated, \
     (SELECT count(*) FROM chat_thread_post p WHERE p.thread_id = t.id), \
     (SELECT p.author FROM chat_thread_post p WHERE p.thread_id = t.id ORDER BY p.position DESC LIMIT 1) \
     FROM chat_thread t";

fn summary((id, work, title, created_by, created, updated, posts, last): SummaryRow) -> ThreadSummary {
    ThreadSummary { id, work, title, created_by, created, updated, posts, last_author: last.unwrap_or_default() }
}

impl Store {
    async fn check_work(&self, work: WorkId) -> Result<(), StoreError> {
        let found: Option<i64> =
            sqlx::query_scalar("SELECT id FROM work WHERE id = ?").bind(work).fetch_optional(&self.pool).await?;
        found.map(|_| ()).ok_or(StoreError::UnknownWork(work))
    }

    /// Fäden eines Werks, zuletzt beschriebene zuerst.
    pub async fn thread_list(&self, work: WorkId) -> Result<Vec<ThreadSummary>, StoreError> {
        self.check_work(work).await?;
        let rows: Vec<SummaryRow> =
            sqlx::query_as(&format!("{SUMMARY_SELECT} WHERE t.work_id = ? ORDER BY t.updated DESC, t.id DESC"))
                .bind(work)
                .fetch_all(&self.pool)
                .await?;
        Ok(rows.into_iter().map(summary).collect())
    }

    /// Faden `id` im Werk `work` mit allen Beiträgen; unbekannt (auch in einem anderen Werk) →
    /// `Refused(UNKNOWN_THREAD)`.
    pub async fn thread_get(&self, work: WorkId, id: i64) -> Result<Thread, StoreError> {
        self.check_work(work).await?;
        let row: Option<SummaryRow> = sqlx::query_as(&format!("{SUMMARY_SELECT} WHERE t.id = ? AND t.work_id = ?"))
            .bind(id)
            .bind(work)
            .fetch_optional(&self.pool)
            .await?;
        let summary = summary(row.ok_or(StoreError::Refused(UNKNOWN_THREAD))?);
        let rows: Vec<PostRow> = sqlx::query_as(
            "SELECT id, position, author, body, sources, created, edited FROM chat_thread_post WHERE thread_id = ? \
             ORDER BY position",
        )
        .bind(id)
        .fetch_all(&self.pool)
        .await?;
        let entries = rows
            .into_iter()
            .map(|(post, position, author, body, sources, created, edited)| {
                let sources: Vec<String> = serde_json::from_str(&sources)
                    .map_err(|e| crate::broken(format!("Fadenbeitrag {post}: Quellen sind kein JSON ({e})")))?;
                Ok(ThreadPost { id: post, thread: id, position, author, body, sources, created, edited })
            })
            .collect::<Result<Vec<_>, StoreError>>()?;
        Ok(Thread { summary, entries })
    }

    /// Neuen Faden mit erstem Beitrag anlegen (eine Transaktion); Titel aus der Nachricht ([`thread_title`]).
    pub async fn thread_create(
        &self,
        work: WorkId,
        author: &str,
        message: &str,
    ) -> Result<(ThreadSummary, ThreadPost), StoreError> {
        if message.trim().is_empty() {
            return Err(StoreError::EmptyPost);
        }
        self.check_work(work).await?;
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let id: i64 =
            sqlx::query_scalar("INSERT INTO chat_thread (work_id, title, created_by) VALUES (?, ?, ?) RETURNING id")
                .bind(work)
                .bind(thread_title(message))
                .bind(author)
                .fetch_one(&mut *tx)
                .await?;
        let post = insert_post(&mut tx, id, author, message, &[]).await?;
        tx.commit().await?;
        let thread = self.thread_get(work, id).await?;
        Ok((thread.summary, post))
    }

    /// Beitrag an den Faden `id` anhängen (Stelle = bisher größte + 1, eine Transaktion).
    pub async fn thread_add_post(
        &self,
        work: WorkId,
        id: i64,
        author: &str,
        body: &str,
        sources: &[String],
    ) -> Result<ThreadPost, StoreError> {
        if body.trim().is_empty() {
            return Err(StoreError::EmptyPost);
        }
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let post = add_post_in(&mut tx, work, id, author, body, sources).await?;
        tx.commit().await?;
        Ok(post)
    }

    /// Runde 30c: Fadenbeitrag `id` im Faden `thread` löschen (eine Transaktion; Verlauf und Befund-Verweise folgen
    /// den Fremdschlüsseln, Grabstein über den Trigger). Gibt den gelöschten Beitrag zurück. Unbekannter Faden →
    /// `Refused(UNKNOWN_THREAD)`, unbekannter Beitrag → `UnknownPost`. Wer löschen darf, prüft der Server.
    pub async fn thread_post_delete(&self, work: WorkId, thread: i64, id: i64) -> Result<ThreadPost, StoreError> {
        self.check_work(work).await?;
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        check_thread(&mut tx, work, thread).await?;
        let post = post_get(&mut tx, work, thread, id).await?;
        let done = sqlx::query("DELETE FROM chat_thread_post WHERE id = ? AND thread_id = ?")
            .bind(id)
            .bind(thread)
            .execute(&mut *tx)
            .await?;
        if done.rows_affected() != 1 {
            return Err(crate::broken(format!("Fadenbeitrag {id} nicht gelöscht")));
        }
        tx.commit().await?;
        Ok(post)
    }
}

/// Gibt es den Faden `thread` im Werk `work`? Sonst `Refused(UNKNOWN_THREAD)`.
async fn check_thread(conn: &mut sqlx::SqliteConnection, work: WorkId, thread: i64) -> Result<(), StoreError> {
    let found: Option<i64> = sqlx::query_scalar("SELECT id FROM chat_thread WHERE id = ? AND work_id = ?")
        .bind(thread)
        .bind(work)
        .fetch_optional(&mut *conn)
        .await?;
    found.map(|_| ()).ok_or(StoreError::Refused(UNKNOWN_THREAD))
}

/// Beitrag an den Faden `thread` des Werks `work` anhängen (in der Transaktion des Aufrufers; Runde 30c auch für
/// `thread_add_post_meta`). Unbekanntes Werk → `UnknownWork`, unbekannter Faden → `Refused(UNKNOWN_THREAD)`.
pub(crate) async fn add_post_in(
    conn: &mut sqlx::SqliteConnection,
    work: WorkId,
    thread: i64,
    author: &str,
    body: &str,
    sources: &[String],
) -> Result<ThreadPost, StoreError> {
    let exists: Option<i64> =
        sqlx::query_scalar("SELECT id FROM work WHERE id = ?").bind(work).fetch_optional(&mut *conn).await?;
    exists.ok_or(StoreError::UnknownWork(work))?;
    check_thread(conn, work, thread).await?;
    insert_post(conn, thread, author, body, sources).await
}

/// Fadenbeitrag `id` im Faden `thread`, gelesen in der Verbindung bzw. Transaktion des Aufrufers (Runde 28:
/// Bearbeiten). Unbekannt → `UnknownPost` (Faden und Werk prüft der Aufrufer).
pub(crate) async fn post_get(
    conn: &mut sqlx::SqliteConnection,
    work: WorkId,
    thread: i64,
    id: i64,
) -> Result<ThreadPost, StoreError> {
    let row: Option<(i64, String, String, String, String, Option<String>)> = sqlx::query_as(
        "SELECT position, author, body, sources, created, edited FROM chat_thread_post WHERE id = ? AND thread_id = ?",
    )
    .bind(id)
    .bind(thread)
    .fetch_optional(&mut *conn)
    .await?;
    let (position, author, body, sources, created, edited) = row.ok_or(StoreError::UnknownPost { work, post: id })?;
    let sources: Vec<String> = serde_json::from_str(&sources)
        .map_err(|e| crate::broken(format!("Fadenbeitrag {id}: Quellen sind kein JSON ({e})")))?;
    Ok(ThreadPost { id, thread, position, author, body, sources, created, edited })
}

async fn insert_post(
    conn: &mut sqlx::SqliteConnection,
    thread: i64,
    author: &str,
    body: &str,
    sources: &[String],
) -> Result<ThreadPost, StoreError> {
    let sources_json = serde_json::to_string(sources).map_err(|e| crate::broken(format!("Quellen: {e}")))?;
    let (id, position, created): (i64, i64, String) = sqlx::query_as(
        "INSERT INTO chat_thread_post (thread_id, position, author, body, sources) VALUES (?, \
         (SELECT coalesce(max(position), -1) + 1 FROM chat_thread_post WHERE thread_id = ?), ?, ?, ?) \
         RETURNING id, position, created",
    )
    .bind(thread)
    .bind(thread)
    .bind(author)
    .bind(body)
    .bind(&sources_json)
    .fetch_one(&mut *conn)
    .await?;
    sqlx::query("UPDATE chat_thread SET updated = ? WHERE id = ?")
        .bind(&created)
        .bind(thread)
        .execute(&mut *conn)
        .await?;
    Ok(ThreadPost {
        id,
        thread,
        position,
        author: author.to_owned(),
        body: body.to_owned(),
        sources: sources.to_vec(),
        created,
        edited: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_thread_title_is_one_short_line() {
        assert_eq!(thread_title("  Wie\nweiter?  "), "Wie weiter?", "Zeilenumbrüche und Leerraum zusammengefasst");
        let long = "a".repeat(200);
        let title = thread_title(&long);
        assert_eq!(title.chars().count(), MAX_THREAD_TITLE_CHARS, "höchstens 80 Zeichen samt Auslassungszeichen");
        assert!(title.ends_with('…'), "gekürzt erkennbar: {title}");
    }
}
