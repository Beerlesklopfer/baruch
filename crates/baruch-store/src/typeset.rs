// SPDX-License-Identifier: AGPL-3.0-or-later
//! Typesetting settings per work (Round 29, contract §4; interface R29-S1, bodies satz-daten).
//!
//! - Table `work_typeset` (migration 0048): at most one row per work, `settings` as canonical JSON
//!   ([`TypesetSettings::to_json`] of the normalised settings); no row = no typesetting settings (everything from the
//!   work template). Column `uuid` through the fill trigger; a change keeps the row and with it the uuid, a reset
//!   deletes it (tombstone of the kind `work_typeset` with the work uuid, also when the work is deleted).
//! - Only the human changes them (form, or RPC from the user interface); MCP only reads. Every change is a mirror
//!   commit in the server (`typeset_settings`, `mirror::record_typeset`).
//! - Writing: first [`TypesetSettings::normalized`], then [`TypesetSettings::validate`]; invalid →
//!   [`StoreError::Typeset`] with all messages („Rand innen: 5 bis 80 mm; …“), nothing changed. Same values → `changed`
//!   empty, nothing written. Empty settings = reset.
//! - [`Store::adopt_typeset_rule`]: settings and a new revision of the work template in one transaction.
//! - The rules from them ([`baruch_typst::typeset::rules`]) are placed by `snapshot::head` behind the work template,
//!   the scheme rules and the fallback rules ([`typeset_in`]); a work without settings exports byte-identically to
//!   before Round 29.

use baruch_typst::typeset::TypesetSettings;
use sqlx::SqliteConnection;

use crate::{Store, StoreError, WorkId, broken};

/// Stored typesetting settings of a work together with identifier and last change.
#[derive(Debug, Clone, PartialEq)]
pub struct WorkTypeset {
    pub settings: TypesetSettings,
    pub uuid: String,
    pub updated: String,
    pub updated_by: String,
}

/// Result of a change: before, after and the changed field ids (in `FIELDS` order).
#[derive(Debug, Clone, PartialEq)]
pub struct TypesetChange {
    pub before: TypesetSettings,
    pub after: TypesetSettings,
    pub changed: Vec<&'static str>,
}

/// Result of [`Store::adopt_typeset_rule`]: change of the settings and new template revision.
#[derive(Debug, Clone, PartialEq)]
pub struct TypesetAdopted {
    pub change: TypesetChange,
    pub template_rev: u32,
}

/// Maximum length of `updated_by` (CHECK of migration 0048).
const MAX_UPDATED_BY_CHARS: usize = 200;

/// Name of the human for `updated_by`: on one line, at most 200 characters.
pub(crate) fn by(human: &str) -> String {
    human.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(MAX_UPDATED_BY_CHARS).collect()
}

/// Normalises and checks; invalid → `StoreError::Typeset` with all messages, separated by "; ".
pub(crate) fn checked(settings: &TypesetSettings) -> Result<TypesetSettings, StoreError> {
    let normalized = settings.normalized();
    normalized.validate().map_err(|errors| {
        StoreError::Typeset(errors.iter().map(|e| e.message.clone()).collect::<Vec<_>>().join("; "))
    })?;
    Ok(normalized)
}

/// Stored row of the work (without checking whether the work exists); also for `exchange_dump`.
pub(crate) async fn row(conn: &mut SqliteConnection, work: WorkId) -> Result<Option<WorkTypeset>, StoreError> {
    let row: Option<(Option<String>, String, String, String)> =
        sqlx::query_as("SELECT uuid, settings, updated, updated_by FROM work_typeset WHERE work_id = ?")
            .bind(work)
            .fetch_optional(&mut *conn)
            .await?;
    let Some((uuid, json, updated, updated_by)) = row else { return Ok(None) };
    let settings = TypesetSettings::from_json(&json)
        .map_err(|e| broken(format!("Satz-Einstellungen von Werk {work} nicht lesbar: {e}")))?;
    let uuid = uuid.ok_or_else(|| broken(format!("Satz-Einstellungen von Werk {work} ohne uuid")))?;
    Ok(Some(WorkTypeset { settings, uuid, updated, updated_by }))
}

/// Does the work exist? Otherwise `UnknownWork`.
async fn ensure_work(conn: &mut SqliteConnection, work: WorkId) -> Result<(), StoreError> {
    let found: Option<i64> =
        sqlx::query_scalar("SELECT id FROM work WHERE id = ?").bind(work).fetch_optional(&mut *conn).await?;
    found.map(|_| ()).ok_or(StoreError::UnknownWork(work))
}

/// Writes `after` (already normalised and checked) over `before`: empty → delete the row (tombstone), otherwise insert
/// or change (the uuid stays). Same values → nothing.
pub(crate) async fn write(
    conn: &mut SqliteConnection,
    work: WorkId,
    before: TypesetSettings,
    after: TypesetSettings,
    human: &str,
) -> Result<TypesetChange, StoreError> {
    let changed = before.changed_fields(&after);
    if changed.is_empty() {
        return Ok(TypesetChange { before, after, changed });
    }
    if after.is_empty() {
        sqlx::query("DELETE FROM work_typeset WHERE work_id = ?").bind(work).execute(&mut *conn).await?;
    } else {
        sqlx::query(
            "INSERT INTO work_typeset (work_id, settings, updated_by) VALUES (?, ?, ?) \
             ON CONFLICT(work_id) DO UPDATE SET settings = excluded.settings, \
             updated = strftime('%Y-%m-%dT%H:%M:%fZ','now'), updated_by = excluded.updated_by",
        )
        .bind(work)
        .bind(after.to_json())
        .bind(by(human))
        .execute(&mut *conn)
        .await?;
    }
    Ok(TypesetChange { before, after, changed })
}

impl Store {
    /// Stored typesetting settings; no row → `None`. Unknown work → `UnknownWork`.
    pub async fn work_typeset(&self, work: WorkId) -> Result<Option<WorkTypeset>, StoreError> {
        let mut tx = self.pool.begin().await?;
        ensure_work(&mut tx, work).await?;
        let stored = row(&mut tx, work).await?;
        tx.commit().await?;
        Ok(stored)
    }

    /// Replaces the settings completely (normalized + validate, otherwise `StoreError::Typeset` with all messages,
    /// nothing changed). Empty settings = reset. Same values → `changed` empty, nothing written. The uuid stays the
    /// same across changes.
    pub async fn set_work_typeset(
        &self,
        work: WorkId,
        settings: &TypesetSettings,
        human: &str,
    ) -> Result<TypesetChange, StoreError> {
        let after = checked(settings)?;
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        ensure_work(&mut tx, work).await?;
        let before = row(&mut tx, work).await?.map(|s| s.settings).unwrap_or_default();
        let change = write(&mut tx, work, before, after, human).await?;
        tx.commit().await?;
        Ok(change)
    }

    /// Deletes the row (tombstone). No row → `changed` empty.
    pub async fn reset_work_typeset(&self, work: WorkId, human: &str) -> Result<TypesetChange, StoreError> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        ensure_work(&mut tx, work).await?;
        let before = row(&mut tx, work).await?.map(|s| s.settings).unwrap_or_default();
        let change = write(&mut tx, work, before, TypesetSettings::default(), human).await?;
        tx.commit().await?;
        Ok(change)
    }

    /// One transaction: set the settings AND a new template revision `template` (note `note`, author `human`) – both or
    /// nothing. Invalid settings → `Typeset`; a template with a heading (`=` at the start of a line) or without a
    /// difference to the present one → `Refused`. The template is stored as in `update_work` (CRLF → LF,
    /// `work.template`, `work.updated`).
    pub async fn adopt_typeset_rule(
        &self,
        work: WorkId,
        settings: &TypesetSettings,
        template: &str,
        note: &str,
        human: &str,
    ) -> Result<TypesetAdopted, StoreError> {
        let after = checked(settings)?;
        let template = template.replace("\r\n", "\n");
        if baruch_typst::parse(&template).is_ok() {
            return Err(StoreError::Refused(
                "Die Werkvorlage darf keine Überschrift (= am Zeilenanfang) enthalten; Überschriften gehören zur Gliederung.",
            ));
        }
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let current: Option<String> =
            sqlx::query_scalar("SELECT template FROM work WHERE id = ?").bind(work).fetch_optional(&mut *tx).await?;
        let current = current.ok_or(StoreError::UnknownWork(work))?;
        if current == template {
            return Err(StoreError::Refused("Die Werkvorlage wäre unverändert; nichts geändert."));
        }
        let before = row(&mut tx, work).await?.map(|s| s.settings).unwrap_or_default();
        let change = write(&mut tx, work, before, after, human).await?;
        let last: i64 =
            sqlx::query_scalar("SELECT coalesce(max(number), 0) FROM work_template_revision WHERE work_id = ?")
                .bind(work)
                .fetch_one(&mut *tx)
                .await?;
        let number = crate::to_u32(last, "Fassung der Werkvorlage")? + 1;
        let note = note.split_whitespace().collect::<Vec<_>>().join(" ");
        crate::works::insert_template_revision(&mut tx, work, number, &template, human, &note).await?;
        sqlx::query("UPDATE work SET template = ?, updated = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ?")
            .bind(&template)
            .bind(work)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(TypesetAdopted { change, template_rev: number })
    }
}

/// Typesetting settings for `snapshot::head`; no row → default (empty).
pub(crate) async fn typeset_in(conn: &mut SqliteConnection, work: WorkId) -> Result<TypesetSettings, StoreError> {
    Ok(row(conn, work).await?.map(|s| s.settings).unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_name_of_the_human_fits_the_column() {
        // CHECK (length(updated_by) <= 200) of migration 0048: longer names would make the saving fail.
        assert_eq!(by("  Mira \n Docht "), "Mira Docht", "in einer Zeile, getrimmt");
        assert_eq!(by(&"ä".repeat(300)).chars().count(), 200, "auf 200 Zeichen gekürzt");
    }

    #[test]
    fn invalid_settings_name_every_field() {
        // Contract §4: all messages, not only the first one.
        let bad = TypesetSettings { font_size_pt: Some(99.0), leading_em: Some(9.0), ..Default::default() };
        let Err(StoreError::Typeset(message)) = checked(&bad) else { panic!("ungültig erwartet") };
        assert!(message.contains("Schriftgröße") && message.contains("Zeilenabstand"), "beide Felder: {message}");
        assert!(message.contains("; "), "durch „; “ getrennt: {message}");
    }
}
