// SPDX-License-Identifier: AGPL-3.0-or-later
//! Ältere Fassungen ausblenden (Nutzer 2026-11-24, Release 0.9.0; Migration 0051, Tabelle `node_revision_fold`).
//!
//! Je Abschnitt höchstens eine Marke „bis hierhin“ (Nummer einer Fassung); was sie ausblendet, regelt
//! `baruch_core::revision_fold`. Setzen und Aufheben nur der Mensch (RPC und Formular, nicht MCP); Fassungen, Verlauf
//! und Spiegel bleiben unverändert. Protokolliert wird wie bei der Einstellung „Fassungen“ in `updated`/`updated_by`;
//! kein Commit im Spiegel (kein Buchtext), nicht im Austauschformat (Arbeitsweise der Instanz).

use std::collections::HashMap;

use sqlx::SqliteConnection;

use crate::write::{node_id, row};
use crate::{Store, StoreError, WorkId, to_u32};

/// Gespeicherte Marke eines Abschnitts.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct RevisionFold {
    /// Nummer der Fassung, an der die Marke steht (ausgeblendet wird darunter).
    pub below: u32,
    /// Zeitpunkt (UTC) und Name des Menschen, der sie zuletzt gesetzt hat.
    pub updated: String,
    pub updated_by: String,
}

/// Ergebnis von [`Store::fold_revisions_before`]: gesetzte Marken je Abschnittsschlüssel (`node.id`).
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
    /// Marke des Abschnitts `node` (Stelle); `None` = nichts ausgeblendet.
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

    /// Alle Marken des Werks: Abschnittsschlüssel (`Node::key`) → Nummer der Marke.
    pub async fn work_revision_folds(&self, work: WorkId) -> Result<HashMap<i64, u32>, StoreError> {
        let rows: Vec<(i64, i64)> = sqlx::query_as(
            "SELECT f.node_id, f.below FROM node_revision_fold f JOIN node n ON n.id = f.node_id WHERE n.work_id = ?",
        )
        .bind(work)
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter().map(|(key, below)| Ok((key, to_u32(below, "below")?))).collect()
    }

    /// Marke an Fassung `rev` setzen (nur Mensch). Die Fassung muss es geben (`UnknownRevision`); über der aktuellen
    /// Fassung wird abgelehnt (`Refused`), damit die aktuelle nie hinter dem Schalter verschwindet.
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

    /// Marke des Abschnitts aufheben („Wieder alle zeigen“); `true`, wenn es eine gab.
    pub async fn clear_revision_fold(&self, work: WorkId, node: usize) -> Result<bool, StoreError> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let id = node_id(&mut tx, work, node).await?;
        let done = sqlx::query("DELETE FROM node_revision_fold WHERE node_id = ?").bind(id).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(done.rows_affected() > 0)
    }

    /// Alle Marken des Werks aufheben; Zahl der aufgehobenen. Unbekanntes Werk → `UnknownWork`.
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

    /// Werkweit „Ältere Fassungen ausblenden bis …“: für jeden Abschnitt die Marke auf die letzte Fassung, die vor
    /// `before` (UTC-Zeitpunkt im Format von `revision.created`) entstand, höchstens auf die aktuelle. Abschnitte ohne
    /// solche Fassung bleiben unverändert; vorhandene Marken werden ersetzt. Ein Schritt (eine Transaktion).
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
