// SPDX-License-Identifier: AGPL-3.0-or-later
//! Word-Export (Runde 32; Schnittstelle R32-S1, Besitz docx_export).
//!
//! Das Werk mit den aktuellen Fassungen als .docx für den Verlag ([`baruch_docx::write`]); auf Wunsch liegen die
//! offenen Vorschläge als Änderungsverfolgung darüber ([`DocxExportOptions::proposals`]). Der Export ändert nichts
//! am Werk. Kein MCP-Werkzeug in dieser Runde.
//!
//! - RPC: [`METHODS`] – `work.docx_export {work?, proposals? = false}`, siehe docs/RPC.md „Runde 32 – Word“.
//! - Seiten: `GET /werke/{id}/word` (Erklärung und Wahl), `GET /werke/{id}/word.docx[?vorschlaege=1]` (Datei).
//! - CLI: `baruch-server export ID|SLUG --docx DATEI [--vorschlaege]` ([`cli_export`]).

use std::sync::Arc;

use axum::Router;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use baruch_store::WorkId;
use serde_json::Value;

use crate::AppState;
use crate::rpc::{REFUSED, RpcError};

/// Methoden dieses Moduls.
pub const METHODS: &[&str] = &["work.docx_export"];

/// Meldung aller Wege dieses Moduls, solange sie Stub sind.
const NOT_YET: &str = "Der Word-Export ist noch nicht umgesetzt (Runde 32).";

pub fn handles(method: &str) -> bool {
    METHODS.contains(&method)
}

/// Wahl des Menschen beim Export.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DocxExportOptions {
    /// Offene Vorschläge als Änderungsverfolgung mitgeben (je Abschnitt der jüngste).
    pub proposals: bool,
}

/// Fertige Datei.
#[derive(Debug, Clone, Default)]
pub struct DocxFile {
    /// `<slug>.docx`.
    pub file_name: String,
    pub bytes: Vec<u8>,
    /// Zahl der Vorschläge, die als Änderungsverfolgung in der Datei stehen.
    pub proposals: usize,
    pub warnings: Vec<baruch_docx::DocxWarning>,
}

/// Baut die Datei aus dem Werk; unbekanntes Werk → -32602.
pub async fn export_file(state: &AppState, work: WorkId, options: &DocxExportOptions) -> Result<DocxFile, RpcError> {
    // STUB: Export fehlt – Werk lesen, Typst nach Blöcken, Vorschläge als Änderungen, schreiben (R32 docx_export)
    let _ = (state, work, options);
    Err(RpcError::new(REFUSED, NOT_YET))
}

/// RPC-Aufruf einer Methode aus [`METHODS`].
pub async fn call(state: &AppState, method: &str, params: Value) -> Result<Value, RpcError> {
    // STUB: work.docx_export fehlt (R32 docx_export)
    let _ = (state, method, params);
    Err(RpcError::new(REFUSED, NOT_YET))
}

/// `baruch-server export ID|SLUG --docx DATEI [--vorschlaege]`: `Ok(text)` → Standardausgabe, Exit 0; `Err(text)` →
/// Fehlerausgabe, Exit 1. Eine vorhandene Datei wird nie überschrieben.
pub async fn cli_export(
    state: &AppState,
    work: WorkId,
    path: &std::path::Path,
    options: &DocxExportOptions,
) -> Result<String, String> {
    // STUB: Export über die Kommandozeile fehlt – nichts wird geschrieben (R32 docx_export)
    let _ = (state, work, options);
    Err(format!("{}: {NOT_YET}", path.display()))
}

async fn not_yet() -> Response {
    // STUB: Seite und Datei fehlen (R32 docx_export)
    (StatusCode::NOT_IMPLEMENTED, NOT_YET).into_response()
}

/// Seiten (GET), gemischt in `lib.rs` bei den Seiten: `GET /werke/{id}/word` und `GET /werke/{id}/word.docx`.
pub fn pages() -> Router<Arc<AppState>> {
    Router::new().route("/werke/{id}/word", get(not_yet)).route("/werke/{id}/word.docx", get(not_yet))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_export_method_belongs_to_this_module() {
        assert!(handles("work.docx_export"), "die eine Methode des Exports");
        assert!(!handles("docx_import.preview"), "der Import gehört docx_import");
        assert!(!handles("work.export"), "der Typst-Export bleibt bei works");
        assert!(!DocxExportOptions::default().proposals, "Vorgabe: ohne Vorschläge (Vertrag: proposals? = false)");
    }
}
