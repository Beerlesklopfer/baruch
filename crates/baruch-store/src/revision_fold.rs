// SPDX-License-Identifier: AGPL-3.0-or-later
//! Hiding older revisions (user 2026-11-24, release 0.9.0; migration 0051, table `node_revision_fold`).
//!
//! Per section at most one mark "up to here" (number of a revision); what it hides is governed by
//! `baruch_core::revision_fold`. Only the human sets and clears it (RPC and form, not MCP); revisions, history and
//! mirror stay unchanged. It is logged like the setting "Fassungen" in `updated`/`updated_by`; no commit in the mirror
//! (not book text), not in the exchange format (working style of the instance).

use std::collections::HashMap;

use sqlx::SqliteConnection;

use crate::write::{node_id, row};
use crate::{Store, StoreError, WorkId, to_u32};

/// Stored mark of a section.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct RevisionFold {
    /// Number of the revision at which the mark stands (what lies below it is hidden).
    pub below: u32,
    /// Point in time (UTC) and name of the human who last set it.
    pub updated: String,
    pub updated_by: String,
}

/// Result of [`Store::fold_revisions_before`]: marks set per section key (`node.id`).
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
pub struct FoldBefore {
    pub set: Vec<(i64, u32)>,
}

async fn current_number(conn: &mut SqliteConnection, node_id: i64) -> Result<Option<u32>, StoreError> {
    let current: Option<i64> =
        sqlx::query_scalar("SELECT number FROM revision WHERE node_id = ? AND status = 'current'")
            .bind(node_id)
            .fetch_optional(&mut *conn)
            .await?;
    current.map(|n| to_u32(n, "Revisionsnummer")).transpose()
}

async fn upsert(conn: &mut SqliteConnection, node_id: i64, below: u32, human: &str) -> Result<(), StoreError> {
    sqlx::query(
        "INSERT INTO node_revision_fold (node_id, below, updated_by) VALUES (?, ?, ?) \
         ON CONFLICT(node_id) DO UPDATE SET below = excluded.below, \
         updated = strftime('%Y-%m-%dT%H:%M:%fZ','now'), updated_by = excluded.updated_by",
    )
    .bind(node_id)
    .bind(i64::from(below))
    .bind(human.chars().take(200).collect::<String>())
    .execute(&mut *conn)
    .await?;
    Ok(())
}

impl Store {
    /// Mark of the section `node` (position); `None` = nothing hidden.
    pub async fn revision_fold(&self, work: WorkId, node: usize) -> Result<Option<RevisionFold>, StoreError> {
        let mut conn = self.pool.acquire().await?;
        let id = node_id(&mut conn, work, node).await?;
        let found: Option<(i64, String, String)> =
            sqlx::query_as("SELECT below, updated, updated_by FROM node_revision_fold WHERE node_id = ?")
                .bind(id)
                .fetch_optional(&mut *conn)
                .await?;
        found
            .map(|(below, updated, updated_by)| {
                Ok(RevisionFold { below: to_u32(below, "below")?, updated, updated_by })
            })
            .transpose()
    }

    /// All marks of the work: section key (`Node::key`) → number of the mark.
    pub async fn work_revision_folds(&self, work: WorkId) -> Result<HashMap<i64, u32>, StoreError> {
        let rows: Vec<(i64, i64)> = sqlx::query_as(
            "SELECT f.node_id, f.below FROM node_revision_fold f JOIN node n ON n.id = f.node_id WHERE n.work_id = ?",
        )
        .bind(work)
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter().map(|(key, below)| Ok((key, to_u32(below, "below")?))).collect()
    }

    /// Set the mark at revision `rev` (human only). The revision must exist (`UnknownRevision`); above the current
    /// revision it is rejected (`Refused`), so that the current one never disappears behind the switch.
    pub async fn set_revision_fold(&self, work: WorkId, node: usize, rev: u32, human: &str) -> Result<(), StoreError> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let id = node_id(&mut tx, work, node).await?;
        row(&mut tx, id, node, rev).await?;
        if current_number(&mut tx, id).await?.is_some_and(|current| rev > current) {
            return Err(StoreError::Refused(
                "Über der aktuellen Fassung lässt sich nicht ausblenden; die Marke höchstens an die aktuelle Fassung setzen.",
            ));
        }
        upsert(&mut tx, id, rev, human).await?;
        tx.commit().await?;
        Ok(())
    }

    /// Clear the mark of the section („Wieder alle zeigen“); `true` if there was one.
    pub async fn clear_revision_fold(&self, work: WorkId, node: usize) -> Result<bool, StoreError> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let id = node_id(&mut tx, work, node).await?;
        let done = sqlx::query("DELETE FROM node_revision_fold WHERE node_id = ?").bind(id).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(done.rows_affected() > 0)
    }

    /// Clear all marks of the work; number of marks cleared. Unknown work → `UnknownWork`.
    pub async fn clear_work_revision_folds(&self, work: WorkId) -> Result<u64, StoreError> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let exists: Option<i64> =
            sqlx::query_scalar("SELECT id FROM work WHERE id = ?").bind(work).fetch_optional(&mut *tx).await?;
        if exists.is_none() {
            return Err(StoreError::UnknownWork(work));
        }
        let done =
            sqlx::query("DELETE FROM node_revision_fold WHERE node_id IN (SELECT id FROM node WHERE work_id = ?)")
                .bind(work)
                .execute(&mut *tx)
                .await?;
        tx.commit().await?;
        Ok(done.rows_affected())
    }

    /// Work-wide „Ältere Fassungen ausblenden bis …“: for every section the mark goes to the last revision that was
    /// created before `before` (UTC point in time in the format of `revision.created`), at most to the current one.
    /// Sections without such a revision stay unchanged; existing marks are replaced. One step (one transaction).
    pub async fn fold_revisions_before(
        &self,
        work: WorkId,
        before: &str,
        human: &str,
    ) -> Result<FoldBefore, StoreError> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let exists: Option<i64> =
            sqlx::query_scalar("SELECT id FROM work WHERE id = ?").bind(work).fetch_optional(&mut *tx).await?;
        if exists.is_none() {
            return Err(StoreError::UnknownWork(work));
        }
        let rows: Vec<(i64, i64, Option<i64>)> = sqlx::query_as(
            "SELECT n.id, max(r.number), \
             (SELECT c.number FROM revision c WHERE c.node_id = n.id AND c.status = 'current') \
             FROM node n JOIN revision r ON r.node_id = n.id \
             WHERE n.work_id = ? AND r.created < ? GROUP BY n.id ORDER BY n.seq",
        )
        .bind(work)
        .bind(before)
        .fetch_all(&mut *tx)
        .await?;
        let mut set = Vec::new();
        for (id, last, current) in rows {
            let last = to_u32(last, "Revisionsnummer")?;
            let below = match current {
                Some(c) => last.min(to_u32(c, "Revisionsnummer")?),
                None => last,
            };
            upsert(&mut tx, id, below, human).await?;
            set.push((id, below));
        }
        tx.commit().await?;
        Ok(FoldBefore { set })
    }
}
