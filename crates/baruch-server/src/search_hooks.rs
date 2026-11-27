// SPDX-License-Identifier: AGPL-3.0-or-later
//! Meaning search for literature, ideas and the settings page „Verbindungen“ (contract Round 12,
//! §2.2). The user interface (switch wording/meaning, status of the ways, „Index neu aufbauen“) calls only
//! these functions; they forward to `crate::rag` (wired up by the integrator in Round 12).

use baruch_rag::{Answer, RagError, SearchRequest, Status};
use baruch_store::WorkId;

use crate::AppState;

/// Meaning search over the ways of the work (answer with all three ways).
pub async fn rag_search(state: &AppState, request: &SearchRequest) -> Result<Answer, RagError> {
    crate::rag::search(state, request).await
}

/// State of the ways and of the local index of a work (without network).
pub async fn rag_status(state: &AppState, work: WorkId) -> Result<Status, RagError> {
    crate::rag::status(state, work).await
}

/// Discard the index of the work and rebuild it; returned: outstanding jobs.
pub async fn rag_reindex(state: &AppState, work: WorkId) -> Result<usize, RagError> {
    crate::rag::reindex(state, work).await
}

/// A highlight of the literature has changed; never blocking. Highlights and library lie
/// in the shared scope of the index (without a work), therefore `notify(None)` brings them up to date; the work
/// is enqueued in addition, because a quotation can change a section there.
pub fn changed(state: &AppState, work: WorkId) {
    crate::rag::notify(state, None);
    crate::rag::notify(state, Some(work));
}
