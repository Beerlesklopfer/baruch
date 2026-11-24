// SPDX-License-Identifier: AGPL-3.0-or-later
//! Zeitwächter je Werk (Migration `0039_work_timing.sql`, Runde 16b, Schnittstelle R16-S1).
//!
//! - `work_timing`: höchstens eine Zeile je Werk mit Wörtern je Minute (60–250) und Zielzeit in Sekunden
//!   (60–14400 oder keine). Ohne Zeile gilt [`TimingSettings::default`] (110 Wörter je Minute, keine
//!   Zielzeit). Ein gelöschtes Werk nimmt seine Zeile mit (`ON DELETE CASCADE`).
//! - Kein Buchtext, kein Commit im Spiegel (wie die Farben der Personas); jede Änderung eine Transaktion
//!   (`BEGIN IMMEDIATE`). Werte außerhalb der Grenzen werden vor dem Schreiben abgelehnt.

use baruch_typst::timing::{DEFAULT_WORDS_PER_MINUTE, MAX_TARGET_MINUTES, MAX_WORDS_PER_MINUTE, MIN_WORDS_PER_MINUTE};
use sqlx::SqliteConnection;

use crate::{Store, StoreError, WorkId};

/// Sprechgeschwindigkeit und Zielzeit eines Werks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct TimingSettings {
    pub words_per_minute: u32,
    pub target_seconds: Option<u32>,
}

impl Default for TimingSettings {
    fn default() -> Self {
        TimingSettings { words_per_minute: DEFAULT_WORDS_PER_MINUTE, target_seconds: None }
    }
}

async fn work_exists(conn: &mut SqliteConnection, work: WorkId) -> Result<(), StoreError> {
    let found: Option<i64> =
        sqlx::query_scalar("SELECT id FROM work WHERE id = ?").bind(work).fetch_optional(&mut *conn).await?;
    found.map(|_| ()).ok_or(StoreError::UnknownWork(work))
}

/// Grenzen prüfen (dieselben wie die CHECK-Regeln der Tabelle).
fn check(settings: TimingSettings) -> Result<(), StoreError> {
    if !(MIN_WORDS_PER_MINUTE..=MAX_WORDS_PER_MINUTE).contains(&settings.words_per_minute) {
        return Err(StoreError::Refused("Wörter je Minute: erlaubt sind 60 bis 250."));
    }
    if let Some(target) = settings.target_seconds
        && !(60..=MAX_TARGET_MINUTES * 60).contains(&target)
    {
        return Err(StoreError::Refused("Zielzeit: erlaubt sind 1 bis 240 Minuten (oder keine Zielzeit)."));
    }
    Ok(())
}

impl Store {
    /// Einstellungen des Werks; ohne Zeile die Vorgabe. Unbekanntes Werk → `UnknownWork`.
    pub async fn timing_settings(&self, work: WorkId) -> Result<TimingSettings, StoreError> {
        let mut conn = self.pool.acquire().await?;
        work_exists(&mut conn, work).await?;
        let row: Option<(i64, Option<i64>)> =
            sqlx::query_as("SELECT words_per_minute, target_seconds FROM work_timing WHERE work_id = ?")
                .bind(work)
                .fetch_optional(&mut *conn)
                .await?;
        Ok(match row {
            Some((wpm, target)) => TimingSettings {
                words_per_minute: u32::try_from(wpm).unwrap_or(DEFAULT_WORDS_PER_MINUTE),
                target_seconds: target.and_then(|t| u32::try_from(t).ok()),
            },
            None => TimingSettings::default(),
        })
    }

    /// Einstellungen des Werks setzen. Außerhalb der Grenzen (60–250 Wörter je Minute, Zielzeit 60–14400 s)
    /// → `Refused` mit deutscher Meldung, nichts geändert; unbekanntes Werk → `UnknownWork`.
    pub async fn set_timing_settings(&self, work: WorkId, settings: TimingSettings) -> Result<(), StoreError> {
        check(settings)?;
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        work_exists(&mut tx, work).await?;
        sqlx::query(
            "INSERT INTO work_timing (work_id, words_per_minute, target_seconds) VALUES (?, ?, ?) \
             ON CONFLICT(work_id) DO UPDATE SET words_per_minute = excluded.words_per_minute, \
             target_seconds = excluded.target_seconds, updated = strftime('%Y-%m-%dT%H:%M:%SZ','now')",
        )
        .bind(work)
        .bind(i64::from(settings.words_per_minute))
        .bind(settings.target_seconds.map(i64::from))
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }
}
