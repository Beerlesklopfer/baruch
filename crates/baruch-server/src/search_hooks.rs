// SPDX-License-Identifier: AGPL-3.0-or-later
//! Bedeutungssuche für Literatur, Einfälle und die Einstellungsseite „Verbindungen“ (Vertrag Runde 12,
//! §2.2). Die Oberfläche (Umschalter Wortlaut/Bedeutung, Status der Wege, „Index neu aufbauen“) ruft nur
//! diese Funktionen; sie leiten an `crate::rag` weiter (in Runde 12 vom Integrator verdrahtet).

use baruch_rag::{Answer, RagError, SearchRequest, Status};
use baruch_store::WorkId;

use crate::AppState;

/// Bedeutungssuche über die Wege des Werks (Antwort mit allen drei Wegen).
pub async fn rag_search(state: &AppState, request: &SearchRequest) -> Result<Answer, RagError> {
    crate::rag::search(state, request).await
}

/// Zustand der Wege und des lokalen Index eines Werks (ohne Netz).
pub async fn rag_status(state: &AppState, work: WorkId) -> Result<Status, RagError> {
    crate::rag::status(state, work).await
}

/// Index des Werks verwerfen und neu aufbauen; Rückgabe: ausstehende Aufträge.
pub async fn rag_reindex(state: &AppState, work: WorkId) -> Result<usize, RagError> {
    crate::rag::reindex(state, work).await
}

/// Eine Hervorhebung der Literatur hat sich geändert; nie blockierend. Hervorhebungen und Bibliothek liegen
/// im gemeinsamen Bereich des Index (ohne Werk), darum zieht `notify(None)` sie nach; das Werk wird
/// zusätzlich eingereiht, weil ein Zitat dort einen Abschnitt ändern kann.
pub fn changed(state: &AppState, work: WorkId) {
    crate::rag::notify(state, None);
    crate::rag::notify(state, Some(work));
}
