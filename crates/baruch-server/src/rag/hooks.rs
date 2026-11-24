// SPDX-License-Identifier: AGPL-3.0-or-later
//! Verbindung zu den Remote-Wegen (Vertrag Runde 12, §2.1): Sammlungen des Werks aus dem Store, pgvector
//! über `connections::pgvector`, MultiGPT über `connections::multigpt`. In Runde 12 vom Integrator
//! verdrahtet.

use std::sync::Arc;

use baruch_rag::{EmbedderFactory, RagError, SearchWay, Way};
use baruch_store::WorkId;

use crate::AppState;

/// Gewählte Sammlungen des Werks für einen Remote-Weg (leer = keine gewählt, Weg „aus“).
pub async fn collections(state: &AppState, work: WorkId, way: Way) -> Result<Vec<String>, RagError> {
    state
        .store
        .work_collections(work, super::rag_way(way))
        .await
        .map_err(|e| RagError::Failed(format!("Sammlungen des Werks nicht lesbar: {e}")))
}

/// pgvector als Suchweg öffnen.
pub async fn open_pgvector(
    state: &AppState,
    embedders: Arc<dyn EmbedderFactory>,
) -> Result<Box<dyn SearchWay>, RagError> {
    crate::connections::pgvector::open_way(state, embedders).await
}

/// MultiGPT als Suchweg öffnen.
pub async fn open_multigpt(state: &AppState) -> Result<Box<dyn SearchWay>, RagError> {
    crate::connections::multigpt::open_way(state).await
}
