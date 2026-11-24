// SPDX-License-Identifier: AGPL-3.0-or-later
//! Varianten als Geschwister im Fassungsbaum (Runde 30b, Migration 0047; Schnittstelle R30-S1, danach gehört das Modul
//! chat-loesungen).
//!
//! Eine Variante ist ein Vorschlag mit gleicher Basis wie ihre Geschwister, mit Gruppe (uuid, klein), Etikett A/B/C,
//! einer Zeile Zusammenfassung und den umgesetzten Befunden (`revision.variant_*`). Gruppe und Etikett stehen immer
//! zusammen (Code-Regel, siehe Kopf von 0047). Übernehmen bleibt beim Menschen.

use crate::{Store, StoreError, WorkId, to_u32};

/// Kennzeichen einer Variante (`revision.variant_*`).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct VariantInfo {
    /// Gruppe: uuid in Kleinbuchstaben (36 Zeichen), gleich für alle Geschwister.
    pub group: String,
    /// Etikett „A“, „B“ oder „C“.
    pub label: String,
    /// Eine Zeile Zusammenfassung (höchstens 300 Zeichen).
    pub summary: String,
    /// Befunde (ids), die diese Variante umsetzt.
    pub findings: Vec<i64>,
}

/// Spalten `variant_group`, `variant_label`, `variant_summary`, `variant_findings` → [`VariantInfo`]; ohne Gruppe oder
/// Etikett `None`.
pub(crate) fn from_columns(
    group: Option<String>,
    label: Option<String>,
    summary: String,
    findings: &str,
) -> Result<Option<VariantInfo>, StoreError> {
    let (Some(group), Some(label)) = (group, label) else {
        return Ok(None);
    };
    let findings: Vec<i64> = serde_json::from_str(findings).map_err(|e| {
        crate::broken(format!("Variante {group}/{label}: variant_findings ist keine Zahlenliste ({e})"))
    })?;
    Ok(Some(VariantInfo { group, label, summary, findings }))
}

impl Store {
    /// Vorschlag eines Assistenten auf der Basis `based_on` (atomar): wie [`Store::add_proposal`] (Überschriften
    /// geprüft, immer `proposed`), aber mit Elternteil `based_on`; unbekannte Basis → `UnknownRevision`. `variant`
    /// kennzeichnet ihn als Variante (Gruppe, Etikett, Zeile, Befunde).
    #[allow(clippy::too_many_arguments)]
    pub async fn add_proposal_on(
        &self,
        work: WorkId,
        node: usize,
        body: &str,
        persona: &str,
        model: &str,
        note: &str,
        based_on: u32,
        variant: Option<&VariantInfo>,
    ) -> Result<u32, StoreError> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        crate::outline::check_proposal_heading(&mut tx, work, node, body).await?;
        let number =
            crate::write::add_proposal_on(&mut tx, work, node, body, persona, model, note, based_on, variant).await?;
        tx.commit().await?;
        Ok(number)
    }

    /// Fester Schlüssel des Abschnitts, zu dem die Variantengruppe `group` gehört (Runde 30b: Vergleichsseite nur mit
    /// Gruppe); unbekannte Gruppe → `None`.
    pub async fn variant_group_key(&self, work: WorkId, group: &str) -> Result<Option<i64>, StoreError> {
        let key: Option<i64> = sqlx::query_scalar(
            "SELECT n.id FROM revision r JOIN node n ON n.id = r.node_id \
             WHERE n.work_id = ? AND r.variant_group = ? LIMIT 1",
        )
        .bind(work)
        .bind(group)
        .fetch_optional(&self.pool)
        .await?;
        Ok(key)
    }

    /// Alle Fassungen des Abschnitts `node` (seq) aus der Variantengruppe `group`, nach Etikett, dann Nummer.
    /// Unbekanntes Werk oder unbekannter Abschnitt → Fehler; unbekannte Gruppe → leere Liste.
    pub async fn variant_group(
        &self,
        work: WorkId,
        node: usize,
        group: &str,
    ) -> Result<Vec<(u32, VariantInfo)>, StoreError> {
        let mut conn = self.pool.acquire().await?;
        let node_id = crate::write::node_id(&mut conn, work, node).await?;
        type VariantRow = (i64, Option<String>, Option<String>, String, String);
        let rows: Vec<VariantRow> = sqlx::query_as(
            "SELECT number, variant_group, variant_label, variant_summary, variant_findings FROM revision \
             WHERE node_id = ? AND variant_group = ? ORDER BY variant_label, number",
        )
        .bind(node_id)
        .bind(group)
        .fetch_all(&mut *conn)
        .await?;
        let mut out = Vec::new();
        for (number, group, label, summary, findings) in rows {
            if let Some(info) = from_columns(group, label, summary, &findings)? {
                out.push((to_u32(number, "Revisionsnummer")?, info));
            }
        }
        Ok(out)
    }
}
