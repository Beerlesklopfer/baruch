// SPDX-License-Identifier: AGPL-3.0-or-later
//! Time guard per work (migration `0039_work_timing.sql`, Round 16b, interface R16-S1).
//!
//! - `work_timing`: at most one row per work with words per minute (60–250) and target time in seconds (60–14400 or
//!   none). Without a row [`TimingSettings::default`] applies (110 words per minute, no target time). A deleted work
//!   takes its row along (`ON DELETE CASCADE`).
//! - Not book text, no commit in the mirror (like the colours of the personas); every change is one transaction (`BEGIN
//!   IMMEDIATE`). Values outside the limits are rejected before writing.

use baruch_typst::timing::{DEFAULT_WORDS_PER_MINUTE, MAX_TARGET_MINUTES, MAX_WORDS_PER_MINUTE, MIN_WORDS_PER_MINUTE};
use sqlx::SqliteConnection;

use crate::{Store, StoreError, WorkId};

/// Speaking rate and target time of a work.
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

/// Check the limits (the same as the CHECK rules of the table).
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
    /// Settings of the work; without a row the default. Unknown work → `UnknownWork`.
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

    /// Set the settings of the work. Outside the limits (60–250 words per minute, target time 60–14400 s) → `Refused`
    /// with a German message, nothing changed; unknown work → `UnknownWork`.
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
