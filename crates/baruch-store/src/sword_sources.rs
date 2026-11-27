// SPDX-License-Identifier: AGPL-3.0-or-later
//! Commentaries and lexicons per work as well as the switch "Bibel-Belege" (Bible evidence) per persona (migration
//! `0037_sword_sources.sql`, Round 15, items 5a/5c).
//!
//! - `work_sword_source`: per work several SWORD modules of the kind `commentary` or `lexicon` in an order (`position`
//!   1..n per kind). A deleted work takes its rows along.
//! - `persona_bible`: switch per persona (`enabled` 0/1); no row = default, which the server defines (`@belege` on,
//!   lector off). Who may use the switch is checked by the server.
//!
//! Not book text, no commit in the mirror; every change is one transaction (`BEGIN IMMEDIATE`).

use sqlx::SqliteConnection;

use crate::{Store, StoreError, WorkId};

/// Maximum number of modules per kind and work.
pub const MAX_WORK_SOURCES: usize = 20;
/// Longest module name (characters).
pub const MAX_MODULE_NAME: usize = 200;

/// Chosen commentaries and lexicons of a work (each in its order).
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
pub struct WorkSources {
    pub commentaries: Vec<String>,
    pub lexicons: Vec<String>,
}

async fn work_exists(conn: &mut SqliteConnection, work: WorkId) -> Result<(), StoreError> {
    let found: Option<i64> =
        sqlx::query_scalar("SELECT id FROM work WHERE id = ?").bind(work).fetch_optional(&mut *conn).await?;
    found.map(|_| ()).ok_or(StoreError::UnknownWork(work))
}

async fn persona_id(conn: &mut SqliteConnection, handle: &str) -> Result<i64, StoreError> {
    let wanted = handle.trim();
    let bare = wanted.trim_start_matches('@');
    for candidate in [wanted.to_owned(), format!("@{bare}")] {
        if candidate.is_empty() || candidate == "@" {
            continue;
        }
        let id: Option<i64> = sqlx::query_scalar("SELECT id FROM persona WHERE handle = ?")
            .bind(&candidate)
            .fetch_optional(&mut *conn)
            .await?;
        if let Some(id) = id {
            return Ok(id);
        }
    }
    Err(StoreError::UnknownPersona(wanted.to_owned()))
}

/// Names trimmed, without empty and duplicate ones (case-insensitive), at most [`MAX_WORK_SOURCES`].
fn cleaned(names: &[String]) -> Result<Vec<String>, StoreError> {
    let mut out: Vec<String> = Vec::new();
    for name in names {
        let name = name.trim();
        if name.is_empty() {
            continue;
        }
        if name.chars().count() > MAX_MODULE_NAME || name.chars().any(char::is_control) {
            return Err(StoreError::Refused("Modulname: höchstens 200 Zeichen in einer Zeile."));
        }
        if !out.iter().any(|o| o.eq_ignore_ascii_case(name)) {
            out.push(name.to_owned());
        }
    }
    if out.len() > MAX_WORK_SOURCES {
        return Err(StoreError::Refused("Höchstens 20 Kommentare bzw. Lexika je Werk."));
    }
    Ok(out)
}

impl Store {
    /// Commentaries and lexicons of a work; unknown work → `UnknownWork`.
    pub async fn work_sword_sources(&self, work: WorkId) -> Result<WorkSources, StoreError> {
        let mut conn = self.pool.acquire().await?;
        work_exists(&mut conn, work).await?;
        let rows: Vec<(String, String)> = sqlx::query_as(
            "SELECT kind, module FROM work_sword_source WHERE work_id = ? ORDER BY kind, position, module",
        )
        .bind(work)
        .fetch_all(&mut *conn)
        .await?;
        let mut out = WorkSources::default();
        for (kind, module) in rows {
            match kind.as_str() {
                "commentary" => out.commentaries.push(module),
                _ => out.lexicons.push(module),
            }
        }
        Ok(out)
    }

    /// Replace the commentaries and lexicons of a work completely (order as passed, empty and duplicate names are
    /// dropped). Whether the modules exist is checked by the server.
    pub async fn set_work_sword_sources(
        &self,
        work: WorkId,
        commentaries: &[String],
        lexicons: &[String],
    ) -> Result<WorkSources, StoreError> {
        let commentaries = cleaned(commentaries)?;
        let lexicons = cleaned(lexicons)?;
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        work_exists(&mut tx, work).await?;
        sqlx::query("DELETE FROM work_sword_source WHERE work_id = ?").bind(work).execute(&mut *tx).await?;
        for (kind, list) in [("commentary", &commentaries), ("lexicon", &lexicons)] {
            for (index, module) in list.iter().enumerate() {
                sqlx::query("INSERT INTO work_sword_source (work_id, kind, module, position) VALUES (?, ?, ?, ?)")
                    .bind(work)
                    .bind(kind)
                    .bind(module)
                    .bind(index as i64 + 1)
                    .execute(&mut *tx)
                    .await?;
            }
        }
        tx.commit().await?;
        Ok(WorkSources { commentaries, lexicons })
    }

    /// Switch "Bibel-Belege" of a persona; `None` = no row (default of the server). Unknown persona → `UnknownPersona`.
    pub async fn persona_bible(&self, handle: &str) -> Result<Option<bool>, StoreError> {
        let mut conn = self.pool.acquire().await?;
        let id = persona_id(&mut conn, handle).await?;
        let enabled: Option<i64> = sqlx::query_scalar("SELECT enabled FROM persona_bible WHERE persona_id = ?")
            .bind(id)
            .fetch_optional(&mut *conn)
            .await?;
        Ok(enabled.map(|e| e == 1))
    }

    /// Set the switch; unknown persona → `UnknownPersona`.
    pub async fn set_persona_bible(&self, handle: &str, on: bool) -> Result<bool, StoreError> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let id = persona_id(&mut tx, handle).await?;
        sqlx::query(
            "INSERT INTO persona_bible (persona_id, enabled) VALUES (?, ?) ON CONFLICT(persona_id) DO UPDATE SET \
             enabled = excluded.enabled, updated = strftime('%Y-%m-%dT%H:%M:%fZ','now')",
        )
        .bind(id)
        .bind(i64::from(on))
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(on)
    }
}
