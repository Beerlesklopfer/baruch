// SPDX-License-Identifier: AGPL-3.0-or-later
//! Editing posts (Round 28, migration 0046; interface S1, afterwards the module belongs to chat-auftrag).
//!
//! - Editing stores the old version as `post_version` (text, since when it applied, who replaced it) and sets
//!   `post.edited` or `chat_thread_post.edited` to the time of the change (NULL = never edited).
//! - Who may edit (only posts of the human, or through MCP only one's own) is checked by the server, not the store.
//! - A post is addressed through its id **and** its work; an id from another work is unknown.
//! - The new text is trimmed; empty, longer than [`MAX_POST_CHARS`] or unchanged → [`StoreError::PostEdit`] (nothing
//!   changed, no empty version in the history).
//! - Every change is exactly one transaction (`BEGIN IMMEDIATE`): storing the old version and setting the text together
//!   with `edited` happen together or not at all. The full-text index is kept up to date by the trigger `post_fts_edit`
//!   (library 0107); thread posts are not in the index.

use std::collections::HashMap;

use sqlx::SqliteConnection;

use crate::threads::UNKNOWN_THREAD;
use crate::{PostInfo, Store, StoreError, ThreadPost, WorkId};

/// Longest post text after editing (characters; like a chat message).
pub const MAX_POST_CHARS: usize = 4000;

/// Check the new text: trimmed, not empty, at most [`MAX_POST_CHARS`] characters, different from before.
fn checked_body<'a>(body: &'a str, before: &str) -> Result<&'a str, StoreError> {
    let body = body.trim();
    if body.is_empty() {
        return Err(StoreError::PostEdit("Der Beitrag darf nicht leer sein; nichts geändert.".into()));
    }
    if body.chars().count() > MAX_POST_CHARS {
        return Err(StoreError::PostEdit(format!(
            "Ein Beitrag darf höchstens {MAX_POST_CHARS} Zeichen haben; nichts geändert."
        )));
    }
    if body == before.trim() {
        return Err(StoreError::PostEdit("Der Text ist unverändert; nichts geändert.".into()));
    }
    Ok(body)
}

/// Store the old version (in the transaction of the caller); exactly one of `post` and `thread_post` is set.
async fn archive(
    conn: &mut SqliteConnection,
    post: Option<i64>,
    thread_post: Option<i64>,
    body: &str,
    valid_from: &str,
    by: &str,
) -> Result<PostVersion, StoreError> {
    let row: VersionRow = sqlx::query_as(
        "INSERT INTO post_version (post_id, thread_post_id, body, valid_from, edited_by) VALUES (?, ?, ?, ?, ?) \
         RETURNING id, uuid, body, valid_from, replaced, edited_by",
    )
    .bind(post)
    .bind(thread_post)
    .bind(body)
    .bind(valid_from)
    .bind(by)
    .fetch_one(&mut *conn)
    .await?;
    // The fill trigger sets the uuid after the insert; RETURNING does not see it yet.
    let uuid: Option<String> =
        sqlx::query_scalar("SELECT uuid FROM post_version WHERE id = ?").bind(row.0).fetch_one(&mut *conn).await?;
    Ok(version((row.0, uuid, row.2, row.3, row.4, row.5)))
}

/// An old version of a post.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct PostVersion {
    pub id: i64,
    pub uuid: String,
    pub body: String,
    /// Since when this version applied (creation or previous change).
    pub valid_from: String,
    /// When it was replaced.
    pub replaced: String,
    pub edited_by: String,
}

type VersionRow = (i64, Option<String>, String, String, String, String);

fn version((id, uuid, body, valid_from, replaced, edited_by): VersionRow) -> PostVersion {
    PostVersion { id, uuid: uuid.unwrap_or_default(), body, valid_from, replaced, edited_by }
}

/// Does the post `id` exist in the work `work`? Otherwise `UnknownPost` or `UnknownWork`.
async fn check_post(conn: &mut SqliteConnection, work: WorkId, id: i64) -> Result<(), StoreError> {
    let found: Option<i64> =
        sqlx::query_scalar("SELECT p.id FROM post p JOIN node n ON n.id = p.node_id WHERE p.id = ? AND n.work_id = ?")
            .bind(id)
            .bind(work)
            .fetch_optional(&mut *conn)
            .await?;
    if found.is_some() {
        return Ok(());
    }
    let exists: Option<i64> =
        sqlx::query_scalar("SELECT id FROM work WHERE id = ?").bind(work).fetch_optional(&mut *conn).await?;
    Err(match exists {
        Some(_) => StoreError::UnknownPost { work, post: id },
        None => StoreError::UnknownWork(work),
    })
}

/// Does the thread post `id` exist in the thread `thread` of the work `work`? Unknown thread →
/// `Refused(UNKNOWN_THREAD)`, unknown post in the thread → `UnknownPost`, unknown work → `UnknownWork`.
async fn check_thread_post(conn: &mut SqliteConnection, work: WorkId, thread: i64, id: i64) -> Result<(), StoreError> {
    let exists: Option<i64> =
        sqlx::query_scalar("SELECT id FROM work WHERE id = ?").bind(work).fetch_optional(&mut *conn).await?;
    exists.ok_or(StoreError::UnknownWork(work))?;
    let found: Option<i64> = sqlx::query_scalar("SELECT id FROM chat_thread WHERE id = ? AND work_id = ?")
        .bind(thread)
        .bind(work)
        .fetch_optional(&mut *conn)
        .await?;
    found.ok_or(StoreError::Refused(UNKNOWN_THREAD))?;
    let post: Option<i64> = sqlx::query_scalar("SELECT id FROM chat_thread_post WHERE id = ? AND thread_id = ?")
        .bind(id)
        .bind(thread)
        .fetch_optional(&mut *conn)
        .await?;
    post.map(|_| ()).ok_or(StoreError::UnknownPost { work, post: id })
}

impl Store {
    /// Edit the post `id` in the section chat: old version as `post_version`, new text, `edited` = now; one
    /// transaction. Returns the post after the change and the old version that was stored.
    pub async fn post_edit(
        &self,
        work: WorkId,
        id: i64,
        body: &str,
        by: &str,
    ) -> Result<(PostInfo, PostVersion), StoreError> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let before = crate::posts::get(&mut tx, work, id).await?;
        let body = checked_body(body, &before.body)?;
        let valid_from = before.edited.clone().unwrap_or_else(|| before.created.clone());
        let old = archive(&mut tx, Some(id), None, &before.body, &valid_from, by).await?;
        sqlx::query("UPDATE post SET body = ?, edited = ? WHERE id = ?")
            .bind(body)
            .bind(&old.replaced)
            .bind(id)
            .execute(&mut *tx)
            .await?;
        let after = crate::posts::get(&mut tx, work, id).await?;
        tx.commit().await?;
        Ok((after, old))
    }

    /// Old versions of the post `id`, oldest first (empty if never edited).
    pub async fn post_versions(&self, work: WorkId, id: i64) -> Result<Vec<PostVersion>, StoreError> {
        let mut conn = self.pool.acquire().await?;
        check_post(&mut conn, work, id).await?;
        let rows: Vec<VersionRow> = sqlx::query_as(
            "SELECT id, uuid, body, valid_from, replaced, edited_by FROM post_version WHERE post_id = ? ORDER BY id",
        )
        .bind(id)
        .fetch_all(&mut *conn)
        .await?;
        Ok(rows.into_iter().map(version).collect())
    }

    /// Edit the thread post `id` in the thread `thread` (like [`Store::post_edit`]; thread posts are never in the
    /// mirror).
    pub async fn thread_post_edit(
        &self,
        work: WorkId,
        thread: i64,
        id: i64,
        body: &str,
        by: &str,
    ) -> Result<(ThreadPost, PostVersion), StoreError> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        check_thread_post(&mut tx, work, thread, id).await?;
        let before = crate::threads::post_get(&mut tx, work, thread, id).await?;
        let body = checked_body(body, &before.body)?;
        let valid_from = before.edited.clone().unwrap_or_else(|| before.created.clone());
        let old = archive(&mut tx, None, Some(id), &before.body, &valid_from, by).await?;
        sqlx::query("UPDATE chat_thread_post SET body = ?, edited = ? WHERE id = ?")
            .bind(body)
            .bind(&old.replaced)
            .bind(id)
            .execute(&mut *tx)
            .await?;
        let after = crate::threads::post_get(&mut tx, work, thread, id).await?;
        tx.commit().await?;
        Ok((after, old))
    }

    /// Old versions of the thread post `id`, oldest first.
    pub async fn thread_post_versions(
        &self,
        work: WorkId,
        thread: i64,
        id: i64,
    ) -> Result<Vec<PostVersion>, StoreError> {
        let mut conn = self.pool.acquire().await?;
        check_thread_post(&mut conn, work, thread, id).await?;
        let rows: Vec<VersionRow> = sqlx::query_as(
            "SELECT id, uuid, body, valid_from, replaced, edited_by FROM post_version WHERE thread_post_id = ? \
             ORDER BY id",
        )
        .bind(id)
        .fetch_all(&mut *conn)
        .await?;
        Ok(rows.into_iter().map(version).collect())
    }

    /// Edited posts of the work in the section chat: `post.id` → time of the last change.
    pub async fn post_edited_map(&self, work: WorkId) -> Result<HashMap<i64, String>, StoreError> {
        let rows: Vec<(i64, String)> = sqlx::query_as(
            "SELECT p.id, p.edited FROM post p JOIN node n ON n.id = p.node_id \
             WHERE n.work_id = ? AND p.edited IS NOT NULL",
        )
        .bind(work)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().collect())
    }
}
