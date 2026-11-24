// SPDX-License-Identifier: AGPL-3.0-or-later
//! Beiträge bearbeiten (Runde 28, Migration 0046; Schnittstelle S1, danach gehört das Modul chat-auftrag).
//!
//! - Bearbeiten legt die alte Fassung als `post_version` ab (Text, seit wann sie galt, wer ersetzt hat) und setzt
//!   `post.edited` bzw. `chat_thread_post.edited` auf die Zeit der Änderung (NULL = nie bearbeitet).
//! - Wer bearbeiten darf (nur Beiträge des Menschen bzw. über MCP nur eigene), prüft der Server, nicht der Store.
//! - Ein Beitrag wird über seine id **und** sein Werk angesprochen; eine id aus einem anderen Werk ist unbekannt.
//! - Der neue Text wird getrimmt; leer, länger als [`MAX_POST_CHARS`] oder unverändert → [`StoreError::PostEdit`]
//!   (nichts geändert, keine leere Fassung im Verlauf).
//! - Jede Änderung ist genau eine Transaktion (`BEGIN IMMEDIATE`): alte Fassung ablegen und Text samt `edited`
//!   setzen geschehen zusammen oder gar nicht. Den Volltextindex hält der Trigger `post_fts_edit` (Bibliothek 0107)
//!   aktuell; Fadenbeiträge stehen nicht im Index.

use std::collections::HashMap;

use sqlx::SqliteConnection;

use crate::threads::UNKNOWN_THREAD;
use crate::{PostInfo, Store, StoreError, ThreadPost, WorkId};

/// Längster Beitragstext nach dem Bearbeiten (Zeichen; wie eine Chat-Nachricht).
pub const MAX_POST_CHARS: usize = 4000;

/// Neuen Text prüfen: getrimmt, nicht leer, höchstens [`MAX_POST_CHARS`] Zeichen, anders als bisher.
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

/// Alte Fassung ablegen (in der Transaktion des Aufrufers); `post` bzw. `thread_post` ist genau einer gesetzt.
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
    // Die uuid setzt der Fülltrigger nach dem Einfügen; RETURNING sieht sie noch nicht.
    let uuid: Option<String> =
        sqlx::query_scalar("SELECT uuid FROM post_version WHERE id = ?").bind(row.0).fetch_one(&mut *conn).await?;
    Ok(version((row.0, uuid, row.2, row.3, row.4, row.5)))
}

/// Eine alte Fassung eines Beitrags.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct PostVersion {
    pub id: i64,
    pub uuid: String,
    pub body: String,
    /// Seit wann diese Fassung galt (Anlage bzw. vorige Änderung).
    pub valid_from: String,
    /// Wann sie ersetzt wurde.
    pub replaced: String,
    pub edited_by: String,
}

type VersionRow = (i64, Option<String>, String, String, String, String);

fn version((id, uuid, body, valid_from, replaced, edited_by): VersionRow) -> PostVersion {
    PostVersion { id, uuid: uuid.unwrap_or_default(), body, valid_from, replaced, edited_by }
}

/// Gibt es den Beitrag `id` im Werk `work`? Sonst `UnknownPost` bzw. `UnknownWork`.
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

/// Gibt es den Fadenbeitrag `id` im Faden `thread` des Werks `work`? Unbekannter Faden → `Refused(UNKNOWN_THREAD)`,
/// unbekannter Beitrag im Faden → `UnknownPost`, unbekanntes Werk → `UnknownWork`.
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
    /// Beitrag `id` im Abschnitts-Chat bearbeiten: alte Fassung als `post_version`, neuer Text, `edited` = jetzt;
    /// eine Transaktion. Gibt den Beitrag nach der Änderung und die abgelegte alte Fassung zurück.
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

    /// Alte Fassungen des Beitrags `id`, älteste zuerst (leer, wenn nie bearbeitet).
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

    /// Fadenbeitrag `id` im Faden `thread` bearbeiten (wie [`Store::post_edit`]; Fadenbeiträge stehen nie im
    /// Spiegel).
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

    /// Alte Fassungen des Fadenbeitrags `id`, älteste zuerst.
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

    /// Bearbeitete Beiträge des Werks im Abschnitts-Chat: `post.id` → Zeit der letzten Änderung.
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
