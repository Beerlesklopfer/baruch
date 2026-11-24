// SPDX-License-Identifier: AGPL-3.0-or-later
//! Satz-Einstellungen je Werk (Runde 29, Vertrag §4; Schnittstelle R29-S1, Körper satz-daten).
//!
//! - Tabelle `work_typeset` (Migration 0048): höchstens eine Zeile je Werk, `settings` als kanonisches JSON
//!   ([`TypesetSettings::to_json`] der normalisierten Einstellungen); keine Zeile = keine Satz-Einstellungen (alles
//!   aus der Werkvorlage). Spalte `uuid` über den Fülltrigger; ein Ändern behält die Zeile und damit die uuid,
//!   Zurücksetzen löscht sie (Grabstein der Art `work_typeset` mit der Werk-uuid, auch beim Löschen des Werks).
//! - Ändern nur durch den Menschen (Formular bzw. RPC aus der Oberfläche); MCP liest nur. Jede Änderung ist im Server
//!   ein Spiegel-Commit (`typeset_settings`, `mirror::record_typeset`).
//! - Schreiben: erst [`TypesetSettings::normalized`], dann [`TypesetSettings::validate`]; ungültig →
//!   [`StoreError::Typeset`] mit allen Meldungen („Rand innen: 5 bis 80 mm; …“), nichts geändert. Gleiche Werte →
//!   `changed` leer, nichts geschrieben. Leere Einstellungen = Zurücksetzen.
//! - [`Store::adopt_typeset_rule`]: Einstellungen und neue Fassung der Werkvorlage in einer Transaktion.
//! - Die Regeln daraus ([`baruch_typst::typeset::rules`]) setzt `snapshot::head` hinter Werkvorlage, Schema- und
//!   Rückfall-Regeln ([`typeset_in`]); ein Werk ohne Einstellungen exportiert byte-gleich wie vor Runde 29.

use baruch_typst::typeset::TypesetSettings;
use sqlx::SqliteConnection;

use crate::{Store, StoreError, WorkId, broken};

/// Gespeicherte Satz-Einstellungen eines Werks samt Kennung und letzter Änderung.
#[derive(Debug, Clone, PartialEq)]
pub struct WorkTypeset {
    pub settings: TypesetSettings,
    pub uuid: String,
    pub updated: String,
    pub updated_by: String,
}

/// Ergebnis einer Änderung: vorher, nachher und die geänderten Feld-ids (in `FIELDS`-Reihenfolge).
#[derive(Debug, Clone, PartialEq)]
pub struct TypesetChange {
    pub before: TypesetSettings,
    pub after: TypesetSettings,
    pub changed: Vec<&'static str>,
}

/// Ergebnis von [`Store::adopt_typeset_rule`]: Änderung der Einstellungen und neue Vorlagenfassung.
#[derive(Debug, Clone, PartialEq)]
pub struct TypesetAdopted {
    pub change: TypesetChange,
    pub template_rev: u32,
}

/// Höchstlänge von `updated_by` (CHECK der Migration 0048).
const MAX_UPDATED_BY_CHARS: usize = 200;

/// Name des Menschen für `updated_by`: in einer Zeile, höchstens 200 Zeichen.
pub(crate) fn by(human: &str) -> String {
    human.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(MAX_UPDATED_BY_CHARS).collect()
}

/// Normalisiert und prüft; ungültig → `StoreError::Typeset` mit allen Meldungen, durch „; “ getrennt.
pub(crate) fn checked(settings: &TypesetSettings) -> Result<TypesetSettings, StoreError> {
    let normalized = settings.normalized();
    normalized.validate().map_err(|errors| {
        StoreError::Typeset(errors.iter().map(|e| e.message.clone()).collect::<Vec<_>>().join("; "))
    })?;
    Ok(normalized)
}

/// Gespeicherte Zeile des Werks (ohne Prüfung, ob es das Werk gibt); auch für `exchange_dump`.
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

/// Gibt es das Werk? Sonst `UnknownWork`.
async fn ensure_work(conn: &mut SqliteConnection, work: WorkId) -> Result<(), StoreError> {
    let found: Option<i64> =
        sqlx::query_scalar("SELECT id FROM work WHERE id = ?").bind(work).fetch_optional(&mut *conn).await?;
    found.map(|_| ()).ok_or(StoreError::UnknownWork(work))
}

/// Schreibt `after` (schon normalisiert und geprüft) über `before`: leer → Zeile löschen (Grabstein), sonst
/// einfügen bzw. ändern (uuid bleibt). Gleiche Werte → nichts.
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
    /// Gespeicherte Satz-Einstellungen; keine Zeile → `None`. Unbekanntes Werk → `UnknownWork`.
    pub async fn work_typeset(&self, work: WorkId) -> Result<Option<WorkTypeset>, StoreError> {
        let mut tx = self.pool.begin().await?;
        ensure_work(&mut tx, work).await?;
        let stored = row(&mut tx, work).await?;
        tx.commit().await?;
        Ok(stored)
    }

    /// Ersetzt die Einstellungen ganz (normalized + validate, sonst `StoreError::Typeset` mit allen Meldungen, nichts
    /// geändert). Leere Einstellungen = reset. Gleiche Werte → `changed` leer, nichts geschrieben. uuid bleibt bei
    /// Änderungen gleich.
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

    /// Löscht die Zeile (Grabstein). Keine Zeile → `changed` leer.
    pub async fn reset_work_typeset(&self, work: WorkId, human: &str) -> Result<TypesetChange, StoreError> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        ensure_work(&mut tx, work).await?;
        let before = row(&mut tx, work).await?.map(|s| s.settings).unwrap_or_default();
        let change = write(&mut tx, work, before, TypesetSettings::default(), human).await?;
        tx.commit().await?;
        Ok(change)
    }

    /// Eine Transaktion: Einstellungen setzen UND neue Vorlagenfassung `template` (Notiz `note`, Autor `human`) –
    /// beides oder nichts. Ungültige Einstellungen → `Typeset`; eine Vorlage mit Überschrift (`=` am Zeilenanfang)
    /// oder ohne Unterschied zur jetzigen → `Refused`. Die Vorlage wird wie in `update_work` gespeichert (CRLF → LF,
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

/// Satz-Einstellungen für `snapshot::head`; keine Zeile → Vorgabe (leer).
pub(crate) async fn typeset_in(conn: &mut SqliteConnection, work: WorkId) -> Result<TypesetSettings, StoreError> {
    Ok(row(conn, work).await?.map(|s| s.settings).unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_name_of_the_human_fits_the_column() {
        // CHECK (length(updated_by) <= 200) der Migration 0048: längere Namen würden das Speichern scheitern lassen.
        assert_eq!(by("  Mira \n Docht "), "Mira Docht", "in einer Zeile, getrimmt");
        assert_eq!(by(&"ä".repeat(300)).chars().count(), 200, "auf 200 Zeichen gekürzt");
    }

    #[test]
    fn invalid_settings_name_every_field() {
        // Vertrag §4: alle Meldungen, nicht nur die erste.
        let bad = TypesetSettings { font_size_pt: Some(99.0), leading_em: Some(9.0), ..Default::default() };
        let Err(StoreError::Typeset(message)) = checked(&bad) else { panic!("ungültig erwartet") };
        assert!(message.contains("Schriftgröße") && message.contains("Zeilenabstand"), "beide Felder: {message}");
        assert!(message.contains("; "), "durch „; “ getrennt: {message}");
    }
}
