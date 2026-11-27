// SPDX-License-Identifier: AGPL-3.0-or-later
//! Round 36 "Reste" (leftovers; interface R36-S1, ownership `reste`): what deleting an outline item deletes along with
//! it (store-5), so that the server reads it before [`Store::outline_delete`] and logs it in the mirror commit.
//!
//! Round 36 (store-5 from the code review): [`Store::outline_delete`] removes the outline item together with all
//! revisions and posts; the mirror so far held only the text of the current revision. [`Store::outline_delete_losses`]
//! reads – without changing anything – what else is lost. The caller holds the write lock of the server and reads
//! immediately before deleting. Fixed SQL only (string literals, values bound).

use crate::{Store, StoreError, WorkId};

/// What [`Store::outline_delete`] deletes besides the current revision.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OutlineLosses {
    pub revisions: Vec<LostRevision>,
    pub posts: Vec<LostPost>,
}

/// A revision deleted along with it (proposals with `proposal = true`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LostRevision {
    pub number: u32,
    pub author: String,
    pub proposal: bool,
    pub body: String,
}

/// A post deleted along with it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LostPost {
    pub id: i64,
    pub author: String,
    pub body: String,
}

impl Store {
    /// What `outline_delete` deletes besides the current revision: all other revisions (proposals with proposal = true)
    /// and all posts of the item. Read-only.
    pub async fn outline_delete_losses(&self, work: WorkId, node: usize) -> Result<OutlineLosses, StoreError> {
        let seq = i64::try_from(node).map_err(|_| StoreError::UnknownNode { work, node })?;
        let id: Option<i64> = sqlx::query_scalar("SELECT id FROM node WHERE work_id = ? AND seq = ?")
            .bind(work)
            .bind(seq)
            .fetch_optional(&self.pool)
            .await?;
        let Some(id) = id else {
            // An unknown work and an unknown item are reported as in the other methods.
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
