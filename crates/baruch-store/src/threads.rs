// SPDX-License-Identifier: AGPL-3.0-or-later
//! Threads without a section (Round 15, migration 0038): a conversation per work that hangs on no section.
//!
//! - MCP `chat` without `key` creates a thread (first post = sender) or continues one (`thread`); the human reads and
//!   answers on the pages `/faeden` and `/faden` (server `threads.rs`).
//! - A thread is not book text: posts are in `chat_thread_post`, never in `post`, and do not get into the mirror. Every
//!   change is exactly one transaction (`BEGIN IMMEDIATE`).
//! - A thread is addressed through its id **and** its work; an id from another work counts as unknown
//!   ([`UNKNOWN_THREAD`]).
//! - Round 30c (chat-bedienung): thread posts can be deleted ([`Store::thread_post_delete`]); metadata (reply to,
//!   repetition, relay, job) is written by `post_meta.rs` (`thread_add_post_meta`).

use crate::{Store, StoreError, WorkId};

/// Message for a thread that does not exist in the work.
pub const UNKNOWN_THREAD: &str = "Diesen Faden gibt es in diesem Werk nicht.";
/// The title of a thread has at most this many characters (from the beginning of the first message).
pub const MAX_THREAD_TITLE_CHARS: usize = 80;

/// A thread in the overview.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ThreadSummary {
    pub id: i64,
    pub work: WorkId,
    pub title: String,
    /// Who started it („Autor“, `@handle`, `@mcp:<name>`).
    pub created_by: String,
    pub created: String,
    /// Time of the last post.
    pub updated: String,
    pub posts: i64,
    /// Author of the last post (empty without posts).
    pub last_author: String,
}

/// A post in a thread.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ThreadPost {
    pub id: i64,
    pub thread: i64,
    pub position: i64,
    pub author: String,
    pub body: String,
    pub sources: Vec<String>,
    pub created: String,
    /// Time of the last change (Round 28, `chat_thread_post.edited`); `None` = never edited.
    pub edited: Option<String>,
}

/// A thread with all posts (oldest first).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Thread {
    #[serde(flatten)]
    pub summary: ThreadSummary,
    pub entries: Vec<ThreadPost>,
}

/// Title from the beginning of a message: one line, at most [`MAX_THREAD_TITLE_CHARS`] characters.
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
/// Row of a thread post: id, position, author, text, sources (JSON), created, edited.
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

    /// Threads of a work, most recently written first.
    pub async fn thread_list(&self, work: WorkId) -> Result<Vec<ThreadSummary>, StoreError> {
        self.check_work(work).await?;
        let rows: Vec<SummaryRow> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
            "{SUMMARY_SELECT} WHERE t.work_id = ? ORDER BY t.updated DESC, t.id DESC"
        )))
        .bind(work)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(summary).collect())
    }

    /// Thread `id` in the work `work` with all posts; unknown (also in another work) → `Refused(UNKNOWN_THREAD)`.
    pub async fn thread_get(&self, work: WorkId, id: i64) -> Result<Thread, StoreError> {
        self.check_work(work).await?;
        let row: Option<SummaryRow> =
            sqlx::query_as(sqlx::AssertSqlSafe(format!("{SUMMARY_SELECT} WHERE t.id = ? AND t.work_id = ?")))
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

    /// Create a new thread with a first post (one transaction); title from the message ([`thread_title`]).
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

    /// Append a post to the thread `id` (position = largest so far + 1, one transaction).
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

    /// Round 30c: delete the thread post `id` in the thread `thread` (one transaction; history and finding references
    /// follow the foreign keys, tombstone through the trigger). Returns the deleted post. Unknown thread →
    /// `Refused(UNKNOWN_THREAD)`, unknown post → `UnknownPost`. Who may delete is checked by the server.
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

/// Does the thread `thread` exist in the work `work`? Otherwise `Refused(UNKNOWN_THREAD)`.
async fn check_thread(conn: &mut sqlx::SqliteConnection, work: WorkId, thread: i64) -> Result<(), StoreError> {
    let found: Option<i64> = sqlx::query_scalar("SELECT id FROM chat_thread WHERE id = ? AND work_id = ?")
        .bind(thread)
        .bind(work)
        .fetch_optional(&mut *conn)
        .await?;
    found.map(|_| ()).ok_or(StoreError::Refused(UNKNOWN_THREAD))
}

/// Append a post to the thread `thread` of the work `work` (in the transaction of the caller; Round 30c also for
/// `thread_add_post_meta`). Unknown work → `UnknownWork`, unknown thread → `Refused(UNKNOWN_THREAD)`.
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

/// Thread post `id` in the thread `thread`, read in the connection or transaction of the caller (Round 28: editing).
/// Unknown → `UnknownPost` (the caller checks thread and work).
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
