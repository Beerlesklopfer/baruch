// SPDX-License-Identifier: AGPL-3.0-or-later
//! Metadaten von Beiträgen (Runde 30, Migration 0047; Schnittstelle R30-S1, danach gehört das Modul chat-bedienung).
//!
//! Je Beitrag im Abschnitts-Chat (`post`) bzw. im Faden (`chat_thread_post`): geprüfte Fassung (`basis_rev`,
//! `basis_label`), Antwort auf (`reply_to`), Wiederholung von (`repeat_of`), Relay von (`relay_from`) und der Auftrag
//! (`request`, JSON [`PostRequest`], lokal, nicht im Austauschformat). Verweise zeigen nur auf Beiträge desselben
//! Werks (bzw. desselben Fadens); ein Löschen des Ziels setzt sie auf NULL (ON DELETE SET NULL).
//!
//! Runde 30c (chat-bedienung): Fadenbeiträge mit Metadaten in einer Transaktion ([`Store::thread_add_post_meta`]);
//! `request.scope` ist im Faden `thread`.

use std::collections::HashMap;

use sqlx::SqliteConnection;

use crate::{Store, StoreError, WorkId, to_u32};

/// Höchstens so viele Zeichen hat `basis_label` (CHECK der Migration 0047).
const MAX_BASIS_LABEL_CHARS: usize = 300;
/// Höchstens so viele Zeichen hat `relay_from` (CHECK der Migration 0047).
const MAX_RELAY_FROM_CHARS: usize = 200;

/// Der Auftrag, aus dem ein Persona-Beitrag entstand (für „wiederholen“, Runde 30c).
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct PostRequest {
    pub persona: String,
    /// `ask` | `review` | `propose` | `chat` (`assist::Mode::word`).
    pub mode: String,
    /// `section` | `chapter`.
    pub scope: String,
    pub text: String,
    pub rev: Option<u32>,
    /// Runde 23: Websuche war zugelassen ([`web`](Self::web) = `Order::web`); `None` = Beitrag aus der Zeit davor
    /// (↻ Wiederholen verhält sich dann wie bisher: Websuche zugelassen).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub web: Option<bool>,
    /// Runde 23: wer fragte (`Order::asker`, z. B. `@mcp:<Client>` oder beim Relay die anredende Persona); `None` =
    /// der Mensch bzw. ein Beitrag aus der Zeit davor.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub asker: Option<String>,
    /// Runde 23: Relay-Rahmen des Auftrags als JSON `{persona, post, text, findings}` (die anredende Antwort, die der
    /// Auftrag gerahmt als Daten trug); `None` = kein Relay bzw. ein Beitrag aus der Zeit davor.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relay: Option<String>,
}

/// Metadaten eines Beitrags (leer = alles `None` bzw. leer).
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize)]
pub struct PostMeta {
    pub basis_rev: Option<u32>,
    pub basis_label: String,
    pub reply_to: Option<i64>,
    pub repeat_of: Option<i64>,
    pub relay_from: Option<String>,
    pub request: Option<PostRequest>,
}

impl PostMeta {
    /// Keine Metadaten gesetzt.
    pub fn is_empty(&self) -> bool {
        self == &PostMeta::default()
    }
}

/// Spalten einer Zeile: id, basis_rev, basis_label, reply_to, repeat_of, relay_from, request.
type MetaRow = (i64, Option<i64>, String, Option<i64>, Option<i64>, Option<String>, Option<String>);

const META_COLUMNS: &str = "p.id, p.basis_rev, p.basis_label, p.reply_to, p.repeat_of, p.relay_from, p.request";

fn meta_of(
    (id, basis_rev, basis_label, reply_to, repeat_of, relay_from, request): MetaRow,
) -> Result<(i64, PostMeta), StoreError> {
    let request = request
        .map(|r| {
            serde_json::from_str::<PostRequest>(&r)
                .map_err(|e| crate::broken(format!("Beitrag {id}: request ist kein Auftrag ({e})")))
        })
        .transpose()?;
    let meta = PostMeta {
        basis_rev: basis_rev.map(|r| to_u32(r, "basis_rev")).transpose()?,
        basis_label,
        reply_to,
        repeat_of,
        relay_from,
        request,
    };
    Ok((id, meta))
}

/// Grenzen der Migration 0047 als ehrliche Meldung statt eines CHECK-Fehlers.
fn check(meta: &PostMeta) -> Result<(), StoreError> {
    if meta.basis_rev == Some(0) {
        return Err(StoreError::PostEdit("Die geprüfte Fassung beginnt bei 1.".into()));
    }
    if meta.basis_label.chars().count() > MAX_BASIS_LABEL_CHARS {
        return Err(StoreError::PostEdit(format!(
            "Die Angabe der geprüften Fassung ist zu lang (höchstens {MAX_BASIS_LABEL_CHARS} Zeichen)."
        )));
    }
    if let Some(r) = &meta.relay_from {
        let n = r.chars().count();
        if n == 0 || n > MAX_RELAY_FROM_CHARS {
            return Err(StoreError::PostEdit(format!("„Relay von“ braucht 1 bis {MAX_RELAY_FROM_CHARS} Zeichen.")));
        }
    }
    Ok(())
}

fn request_json(meta: &PostMeta) -> Option<String> {
    meta.request.as_ref().map(|r| serde_json::to_string(r).expect("Auftrag ist immer JSON"))
}

/// Gehört der Beitrag `post` zum Werk? Sonst `UnknownPost`.
async fn check_post(conn: &mut SqliteConnection, work: WorkId, post: i64) -> Result<(), StoreError> {
    let found: Option<i64> =
        sqlx::query_scalar("SELECT p.id FROM post p JOIN node n ON n.id = p.node_id WHERE p.id = ? AND n.work_id = ?")
            .bind(post)
            .bind(work)
            .fetch_optional(&mut *conn)
            .await?;
    found.map(|_| ()).ok_or(StoreError::UnknownPost { work, post })
}

/// Gehört der Fadenbeitrag `post` zum Werk? Sonst `UnknownPost`.
async fn check_thread_post(conn: &mut SqliteConnection, work: WorkId, post: i64) -> Result<(), StoreError> {
    let found: Option<i64> = sqlx::query_scalar(
        "SELECT p.id FROM chat_thread_post p JOIN chat_thread t ON t.id = p.thread_id WHERE p.id = ? AND t.work_id = ?",
    )
    .bind(post)
    .bind(work)
    .fetch_optional(&mut *conn)
    .await?;
    found.map(|_| ()).ok_or(StoreError::UnknownPost { work, post })
}

/// Metadaten eines Beitrags im Abschnitts-Chat schreiben (in der Transaktion des Aufrufers); Verweise nur auf Beiträge
/// desselben Werks.
async fn write_post_meta(
    conn: &mut SqliteConnection,
    work: WorkId,
    post: i64,
    meta: &PostMeta,
) -> Result<(), StoreError> {
    check(meta)?;
    check_post(conn, work, post).await?;
    for target in [meta.reply_to, meta.repeat_of].into_iter().flatten() {
        check_post(conn, work, target).await?;
    }
    sqlx::query(
        "UPDATE post SET basis_rev = ?, basis_label = ?, reply_to = ?, repeat_of = ?, relay_from = ?, request = ? \
         WHERE id = ?",
    )
    .bind(meta.basis_rev.map(i64::from))
    .bind(&meta.basis_label)
    .bind(meta.reply_to)
    .bind(meta.repeat_of)
    .bind(&meta.relay_from)
    .bind(request_json(meta))
    .bind(post)
    .execute(&mut *conn)
    .await?;
    Ok(())
}

/// Metadaten eines Fadenbeitrags schreiben (in der Transaktion des Aufrufers); Verweise nur auf Beiträge desselben
/// Fadens.
async fn write_thread_post_meta(
    conn: &mut SqliteConnection,
    work: WorkId,
    thread_post: i64,
    meta: &PostMeta,
) -> Result<(), StoreError> {
    check(meta)?;
    check_thread_post(conn, work, thread_post).await?;
    for target in [meta.reply_to, meta.repeat_of].into_iter().flatten() {
        let same: Option<i64> = sqlx::query_scalar(
            "SELECT b.id FROM chat_thread_post a JOIN chat_thread_post b ON b.thread_id = a.thread_id \
             WHERE a.id = ? AND b.id = ?",
        )
        .bind(thread_post)
        .bind(target)
        .fetch_optional(&mut *conn)
        .await?;
        same.ok_or(StoreError::UnknownPost { work, post: target })?;
    }
    sqlx::query(
        "UPDATE chat_thread_post SET basis_rev = ?, basis_label = ?, reply_to = ?, repeat_of = ?, relay_from = ?, \
         request = ? WHERE id = ?",
    )
    .bind(meta.basis_rev.map(i64::from))
    .bind(&meta.basis_label)
    .bind(meta.reply_to)
    .bind(meta.repeat_of)
    .bind(&meta.relay_from)
    .bind(request_json(meta))
    .bind(thread_post)
    .execute(&mut *conn)
    .await?;
    Ok(())
}

/// Ids als Platzhalterliste „?, ?, ?“.
fn placeholders(n: usize) -> String {
    vec!["?"; n].join(", ")
}

impl Store {
    /// Beitrag wie `add_post`, dazu die Metadaten (eine Transaktion). Verweise (`reply_to`, `repeat_of`) auf Beiträge
    /// eines anderen Werks → `UnknownPost`, nichts angelegt.
    pub async fn add_post_meta(
        &self,
        work: WorkId,
        node: usize,
        post: &baruch_core::Post,
        meta: &PostMeta,
    ) -> Result<i64, StoreError> {
        check(meta)?;
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let id = crate::write::add_post(&mut tx, work, node, post).await?;
        if !meta.is_empty() {
            write_post_meta(&mut tx, work, id, meta).await?;
        }
        tx.commit().await?;
        Ok(id)
    }

    /// Metadaten des Beitrags `post` ersetzen (eine Transaktion). Fremder oder unbekannter Beitrag → `UnknownPost`.
    pub async fn post_meta_set(&self, work: WorkId, post: i64, meta: &PostMeta) -> Result<(), StoreError> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        write_post_meta(&mut tx, work, post, meta).await?;
        tx.commit().await?;
        Ok(())
    }

    /// Metadaten der Beiträge `ids` des Werks (fremde und unbekannte ids fehlen in der Antwort, leere Metadaten auch
    /// nicht: jede gefundene id steht darin).
    pub async fn post_meta(&self, work: WorkId, ids: &[i64]) -> Result<HashMap<i64, PostMeta>, StoreError> {
        if ids.is_empty() {
            return Ok(HashMap::new());
        }
        let sql = format!(
            "SELECT {META_COLUMNS} FROM post p JOIN node n ON n.id = p.node_id WHERE n.work_id = ? AND p.id IN ({})",
            placeholders(ids.len())
        );
        let mut query = sqlx::query_as::<_, MetaRow>(sqlx::AssertSqlSafe(sql.as_str())).bind(work);
        for id in ids {
            query = query.bind(id);
        }
        query.fetch_all(&self.pool).await?.into_iter().map(meta_of).collect()
    }

    /// Alle Beiträge des Werks mit nicht leeren Metadaten.
    pub async fn work_post_meta(&self, work: WorkId) -> Result<HashMap<i64, PostMeta>, StoreError> {
        let sql = format!(
            "SELECT {META_COLUMNS} FROM post p JOIN node n ON n.id = p.node_id WHERE n.work_id = ? AND \
             (p.basis_rev IS NOT NULL OR p.basis_label <> '' OR p.reply_to IS NOT NULL OR p.repeat_of IS NOT NULL \
              OR p.relay_from IS NOT NULL OR p.request IS NOT NULL)"
        );
        let rows: Vec<MetaRow> =
            sqlx::query_as(sqlx::AssertSqlSafe(sql.as_str())).bind(work).fetch_all(&self.pool).await?;
        rows.into_iter().map(meta_of).collect()
    }

    /// Metadaten des Fadenbeitrags `thread_post` ersetzen (eine Transaktion). Verweise nur auf Beiträge desselben
    /// Fadens; fremder oder unbekannter Beitrag → `UnknownPost`.
    pub async fn thread_post_meta_set(
        &self,
        work: WorkId,
        thread_post: i64,
        meta: &PostMeta,
    ) -> Result<(), StoreError> {
        check(meta)?;
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        write_thread_post_meta(&mut tx, work, thread_post, meta).await?;
        tx.commit().await?;
        Ok(())
    }

    /// Runde 30c: Beitrag an den Faden `thread` anhängen wie `thread_add_post`, dazu die Metadaten (eine
    /// Transaktion). Verweise auf Beiträge eines anderen Fadens → `UnknownPost`, nichts angelegt.
    pub async fn thread_add_post_meta(
        &self,
        work: WorkId,
        thread: i64,
        author: &str,
        body: &str,
        sources: &[String],
        meta: &PostMeta,
    ) -> Result<crate::ThreadPost, StoreError> {
        check(meta)?;
        if body.trim().is_empty() {
            return Err(StoreError::EmptyPost);
        }
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let post = crate::threads::add_post_in(&mut tx, work, thread, author, body, sources).await?;
        if !meta.is_empty() {
            write_thread_post_meta(&mut tx, work, post.id, meta).await?;
        }
        tx.commit().await?;
        Ok(post)
    }

    /// Metadaten der Fadenbeiträge `ids` des Werks (fremde und unbekannte ids fehlen).
    pub async fn thread_post_meta(&self, work: WorkId, ids: &[i64]) -> Result<HashMap<i64, PostMeta>, StoreError> {
        if ids.is_empty() {
            return Ok(HashMap::new());
        }
        let sql = format!(
            "SELECT {META_COLUMNS} FROM chat_thread_post p JOIN chat_thread t ON t.id = p.thread_id \
             WHERE t.work_id = ? AND p.id IN ({})",
            placeholders(ids.len())
        );
        let mut query = sqlx::query_as::<_, MetaRow>(sqlx::AssertSqlSafe(sql.as_str())).bind(work);
        for id in ids {
            query = query.bind(id);
        }
        query.fetch_all(&self.pool).await?.into_iter().map(meta_of).collect()
    }
}
