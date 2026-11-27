// SPDX-License-Identifier: AGPL-3.0-or-later
//! Variants as siblings in the revision tree (Round 30b, migration 0047; interface R30-S1, afterwards the module
//! belongs to chat-loesungen).
//!
//! A variant is a proposal with the same base as its siblings, with a group (uuid, lower case), a label A/B/C, one line
//! of summary and the findings it implements (`revision.variant_*`). Group and label always come together (code rule,
//! see the header of 0047). Accepting stays with the human.

use crate::{Store, StoreError, WorkId, to_u32};

/// Marks of a variant (`revision.variant_*`).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct VariantInfo {
    /// Group: uuid in lower-case letters (36 characters), the same for all siblings.
    pub group: String,
    /// Label "A", "B" or "C".
    pub label: String,
    /// One line of summary (at most 300 characters).
    pub summary: String,
    /// Findings (ids) that this variant implements.
    pub findings: Vec<i64>,
}

/// Columns `variant_group`, `variant_label`, `variant_summary`, `variant_findings` → [`VariantInfo`]; without group or
/// label `None`.
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
    /// Proposal of an assistant on the base `based_on` (atomic): like [`Store::add_proposal`] (headings checked, always
    /// `proposed`), but with the parent `based_on`; unknown base → `UnknownRevision`. `variant` marks it as a variant
    /// (group, label, line, findings).
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

    /// Fixed key of the section to which the variant group `group` belongs (Round 30b: comparison page only with a
    /// group); unknown group → `None`.
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

    /// All revisions of the section `node` (seq) from the variant group `group`, by label, then number. Unknown work or
    /// unknown section → error; unknown group → empty list.
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
