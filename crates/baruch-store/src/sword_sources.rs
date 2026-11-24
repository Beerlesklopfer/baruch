// SPDX-License-Identifier: AGPL-3.0-or-later
//! Kommentare und Lexika je Werk sowie der Schalter „Bibel-Belege“ je Persona (Migration
//! `0037_sword_sources.sql`, Runde 15, Punkte 5a/5c).
//!
//! - `work_sword_source`: je Werk mehrere SWORD-Module der Art `commentary` oder `lexicon` in einer
//!   Reihenfolge (`position` 1..n je Art). Ein gelöschtes Werk nimmt seine Zeilen mit.
//! - `persona_bible`: Schalter je Persona (`enabled` 0/1); keine Zeile = Vorgabe, die der Server festlegt
//!   (`@belege` an, Lektor aus). Wer den Schalter nutzen darf, prüft der Server.
//!
//! Kein Buchtext, kein Commit im Spiegel; jede Änderung eine Transaktion (`BEGIN IMMEDIATE`).

use sqlx::SqliteConnection;

use crate::{Store, StoreError, WorkId};

/// Höchstzahl der Module je Art und Werk.
pub const MAX_WORK_SOURCES: usize = 20;
/// Längster Modulname (Zeichen).
pub const MAX_MODULE_NAME: usize = 200;

/// Gewählte Kommentare und Lexika eines Werks (je in ihrer Reihenfolge).
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

/// Namen getrimmt, ohne leere und doppelte (Groß-/Kleinschreibung egal), höchstens [`MAX_WORK_SOURCES`].
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
    /// Kommentare und Lexika eines Werks; unbekanntes Werk → `UnknownWork`.
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

    /// Kommentare und Lexika eines Werks ganz ersetzen (Reihenfolge wie übergeben, leere und doppelte
    /// Namen fallen weg). Ob es die Module gibt, prüft der Server.
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

    /// Schalter „Bibel-Belege“ einer Persona; `None` = keine Zeile (Vorgabe des Servers). Unbekannte Persona
    /// → `UnknownPersona`.
    pub async fn persona_bible(&self, handle: &str) -> Result<Option<bool>, StoreError> {
        let mut conn = self.pool.acquire().await?;
        let id = persona_id(&mut conn, handle).await?;
        let enabled: Option<i64> = sqlx::query_scalar("SELECT enabled FROM persona_bible WHERE persona_id = ?")
            .bind(id)
            .fetch_optional(&mut *conn)
            .await?;
        Ok(enabled.map(|e| e == 1))
    }

    /// Schalter setzen; unbekannte Persona → `UnknownPersona`.
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
