// SPDX-License-Identifier: AGPL-3.0-or-later
//! Hits of the semantic search as evidence per persona (migration 0032, contract Round 12 §4): one row in `persona_rag`
//! with `enabled` 0/1, no row = off. Who may use the switch (only `@belege` and the lector) is checked by the server;
//! the store only holds the value. Not book text, therefore no commit in the mirror. `PersonaRow` stays unchanged.
//! Handles as in the other persona functions, with or without `@`.

use sqlx::SqliteConnection;

use crate::{Store, StoreError};

/// Message for an unknown persona.
const UNKNOWN: &str = "Diese Persona gibt es nicht.";

/// id of the persona (handle as entered or with a leading `@`).
async fn persona_id(conn: &mut SqliteConnection, handle: &str) -> Result<Option<i64>, StoreError> {
    let wanted = handle.trim();
    let mut candidates = vec![wanted.to_owned()];
    if !wanted.starts_with('@') {
        candidates.push(format!("@{wanted}"));
    }
    for candidate in candidates.iter().filter(|c| !c.is_empty()) {
        let id: Option<i64> = sqlx::query_scalar("SELECT id FROM persona WHERE handle = ?")
            .bind(candidate)
            .fetch_optional(&mut *conn)
            .await?;
        if id.is_some() {
            return Ok(id);
        }
    }
    Ok(None)
}

impl Store {
    /// Does the persona get hits of the semantic search as evidence? No row = `false`; unknown persona → `Refused`.
    pub async fn persona_rag(&self, handle: &str) -> Result<bool, StoreError> {
        let mut conn = self.pool.acquire().await?;
        let id = persona_id(&mut conn, handle).await?.ok_or(StoreError::Refused(UNKNOWN))?;
        let enabled: Option<i64> = sqlx::query_scalar("SELECT enabled FROM persona_rag WHERE persona_id = ?")
            .bind(id)
            .fetch_optional(&mut *conn)
            .await?;
        Ok(enabled == Some(1))
    }

    /// Set the switch (one transaction, `BEGIN IMMEDIATE`) and return the new value; unknown persona → `Refused`, then
    /// everything stays as it is.
    pub async fn set_persona_rag(&self, handle: &str, on: bool) -> Result<bool, StoreError> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let id = persona_id(&mut tx, handle).await?.ok_or(StoreError::Refused(UNKNOWN))?;
        sqlx::query(
            "INSERT INTO persona_rag (persona_id, enabled) VALUES (?, ?) ON CONFLICT(persona_id) DO UPDATE SET \
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
