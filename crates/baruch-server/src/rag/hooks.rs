// SPDX-License-Identifier: AGPL-3.0-or-later
//! Connection to the remote ways (contract Round 12, §2.1): collections of the work from the store, pgvector
//! through `connections::pgvector`, MultiGPT through `connections::multigpt`. Wired up by the integrator in
//! Round 12.

use std::sync::Arc;

use baruch_rag::{EmbedderFactory, RagError, SearchWay, Way};
use baruch_store::WorkId;

use crate::AppState;

/// Chosen collections of the work for a remote way (empty = none chosen, way "off").
pub async fn collections(state: &AppState, work: WorkId, way: Way) -> Result<Vec<String>, RagError> {
    state
        .store
        .work_collections(work, super::rag_way(way))
        .await
        .map_err(|e| RagError::Failed(crate::i18n::tc("rag_msg.collections", &[("detail", e.to_string().into())])))
}

/// Open pgvector as a search way.
pub async fn open_pgvector(
    state: &AppState,
    embedders: Arc<dyn EmbedderFactory>,
) -> Result<Box<dyn SearchWay>, RagError> {
    crate::connections::pgvector::open_way(state, embedders).await
}

/// Open MultiGPT as a search way.
pub async fn open_multigpt(state: &AppState) -> Result<Box<dyn SearchWay>, RagError> {
    crate::connections::multigpt::open_way(state).await
}
