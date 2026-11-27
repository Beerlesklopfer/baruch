// SPDX-License-Identifier: AGPL-3.0-or-later
//! Runde 36 „Reste“ (Schnittstelle R36-S1, Besitz `reste`): was das Löschen eines Gliederungspunkts mitlöscht
//! (store-5), damit der Server es vor [`Store::outline_delete`] liest und im Spiegel-Commit protokolliert.
//!
//! Runde 36 (store-5 aus dem Codereview): [`Store::outline_delete`] entfernt den Gliederungspunkt samt allen
//! Fassungen und Beiträgen; im Spiegel stand bisher nur der Text der aktuellen Fassung. [`Store::outline_delete_losses`]
//! liest – ohne etwas zu ändern – was sonst noch verloren geht. Der Aufrufer hält die Schreibsperre des Servers
//! und liest unmittelbar vor dem Löschen. Nur festes SQL (Zeichenketten-Literale, Werte gebunden).

use crate::{Store, StoreError, WorkId};

/// Was [`Store::outline_delete`] außer der aktuellen Fassung mitlöscht.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OutlineLosses {
    pub revisions: Vec<LostRevision>,
    pub posts: Vec<LostPost>,
}

/// Eine mitgelöschte Fassung (Vorschläge mit `proposal = true`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LostRevision {
    pub number: u32,
    pub author: String,
    pub proposal: bool,
    pub body: String,
}

/// Ein mitgelöschter Beitrag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LostPost {
    pub id: i64,
    pub author: String,
    pub body: String,
}

impl Store {
    /// Was `outline_delete` außer der aktuellen Fassung mitlöscht: alle übrigen Fassungen
    /// (Vorschläge mit proposal = true) und alle Beiträge des Punkts. Nur lesend.
    pub async fn outline_delete_losses(&self, work: WorkId, node: usize) -> Result<OutlineLosses, StoreError> {
        let seq = i64::try_from(node).map_err(|_| StoreError::UnknownNode { work, node })?;
        let id: Option<i64> = sqlx::query_scalar("SELECT id FROM node WHERE work_id = ? AND seq = ?")
            .bind(work)
            .bind(seq)
            .fetch_optional(&self.pool)
            .await?;
        let Some(id) = id else {
            // Unbekanntes Werk und unbekannter Punkt melden sich wie bei den übrigen Methoden.
            self.work_slug(work).await?;
            return Err(StoreError::UnknownNode { work, node });
        };
        let revisions: Vec<(i64, String, String, String)> = sqlx::query_as(
            "SELECT number, author_name, status, body FROM revision \
             WHERE node_id = ? AND status <> 'current' ORDER BY number",
        )
        .bind(id)
        .fetch_all(&self.pool)
        .await?;
        let posts: Vec<(i64, String, String)> =
            sqlx::query_as("SELECT id, author, body FROM post WHERE node_id = ? ORDER BY position, id")
                .bind(id)
                .fetch_all(&self.pool)
                .await?;
        Ok(OutlineLosses {
            revisions: revisions
                .into_iter()
                .map(|(number, author, status, body)| LostRevision {
                    number: u32::try_from(number).unwrap_or_default(),
                    author,
                    proposal: status == "proposed",
                    body,
                })
                .collect(),
            posts: posts.into_iter().map(|(id, author, body)| LostPost { id, author, body }).collect(),
        })
    }
}
