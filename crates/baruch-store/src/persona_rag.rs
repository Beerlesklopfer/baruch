// SPDX-License-Identifier: AGPL-3.0-or-later
//! Treffer der Bedeutungssuche als Belege je Persona (Migration 0032, Vertrag Runde 12 §4): eine Zeile in
//! `persona_rag` mit `enabled` 0/1, keine Zeile = aus. Wer den Schalter nutzen darf (nur `@belege` und der
//! Lektor), prüft der Server; der Store hält nur den Wert. Kein Buchtext, darum kein Commit im Spiegel.
//! `PersonaRow` bleibt unverändert. Handles wie bei den übrigen Personen-Funktionen mit oder ohne `@`.

use sqlx::SqliteConnection;

use crate::{Store, StoreError};

/// Meldung bei unbekannter Persona.
const UNKNOWN: &str = "Diese Persona gibt es nicht.";

/// id der Persona (Handle wie eingegeben oder mit vorangestelltem `@`).
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
    /// Bekommt die Persona Treffer der Bedeutungssuche als Belege? Keine Zeile = `false`; unbekannte
    /// Persona → `Refused`.
    pub async fn persona_rag(&self, handle: &str) -> Result<bool, StoreError> {
        let mut conn = self.pool.acquire().await?;
        let id = persona_id(&mut conn, handle).await?.ok_or(StoreError::Refused(UNKNOWN))?;
        let enabled: Option<i64> = sqlx::query_scalar("SELECT enabled FROM persona_rag WHERE persona_id = ?")
            .bind(id)
            .fetch_optional(&mut *conn)
            .await?;
        Ok(enabled == Some(1))
    }

    /// Schalter setzen (eine Transaktion, `BEGIN IMMEDIATE`) und den neuen Wert zurückgeben; unbekannte
    /// Persona → `Refused`, dann bleibt alles, wie es ist.
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
