// SPDX-License-Identifier: AGPL-3.0-or-later
//! Austausch zwischen Instanzen (Runde 26, Doku `docs/Austauschformat.md`; nach S1 nur Integrator).
//!
//! - [`export`]: Seite `/werke/{id}/austausch`, Downloads `.baruch`/`.baruch.xml`, RPC `work.exchange_export`,
//!   CLI `export … --baruch DATEI` (R26b).
//! - [`import`]: Seite `/austausch/einlesen`, Formulare `POST /austausch/vorschau` und
//!   `POST /austausch/uebernehmen`, RPC `work.exchange_preview`/`work.exchange_import`, CLI `import DATEI.baruch`
//!   (R26c). Einlesen nur durch den Menschen, nicht über MCP.
//!
//! RPC ohne Vorgabewerk: [`crate::rpc::call`] fragt [`handles`] vor `works::handles`.

pub mod export;
pub mod import;

use std::sync::Arc;

use axum::Router;
use baruch_exchange::ExchangeError;

use crate::AppState;
use crate::rpc::{INTERNAL, INVALID_PARAMS, METHOD_NOT_FOUND, RpcError};

/// Methoden dieses Moduls.
pub const METHODS: &[&str] = &["work.exchange_export", "work.exchange_preview", "work.exchange_import"];

pub fn handles(method: &str) -> bool {
    METHODS.contains(&method)
}

/// RPC des Austauschs; leitet an [`export`] bzw. [`import`] weiter.
pub async fn call(state: &AppState, method: &str, params: serde_json::Value) -> Result<serde_json::Value, RpcError> {
    match method {
        "work.exchange_export" => export::rpc_export(state, params).await,
        "work.exchange_preview" => import::rpc_preview(state, params).await,
        "work.exchange_import" => import::rpc_import(state, params).await,
        other => Err(RpcError::new(METHOD_NOT_FOUND, format!("unknown method: {other}"))),
    }
}

/// Seiten (GET), gemischt in `lib.rs` bei den Seiten.
pub fn pages() -> Router<Arc<AppState>> {
    export::pages().merge(import::pages())
}

/// Formulare (POST), gemischt in `lib.rs` im writes-Router hinter `guard::same_origin`.
pub fn writes() -> Router<Arc<AppState>> {
    import::writes()
}

/// `true`, wenn der Dateiname (ohne Rücksicht auf Groß/klein) auf `.baruch` oder `.baruch.xml` endet.
pub fn is_exchange_name(file_name: &str) -> bool {
    let lower = file_name.to_lowercase();
    lower.ends_with(baruch_exchange::ZIP_SUFFIX) || lower.ends_with(baruch_exchange::XML_SUFFIX)
}

impl From<ExchangeError> for RpcError {
    fn from(e: ExchangeError) -> Self {
        let code = match e {
            ExchangeError::Write(_) => INTERNAL,
            ExchangeError::NotExchange(_)
            | ExchangeError::Version { .. }
            | ExchangeError::Unsafe(_)
            | ExchangeError::Limit(_)
            | ExchangeError::Archive(_)
            | ExchangeError::Checksum { .. }
            | ExchangeError::Xml(_)
            | ExchangeError::Invalid(_) => INVALID_PARAMS,
        };
        RpcError::new(code, e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exchange_names_end_in_baruch_or_baruch_xml() {
        // Groß/klein egal: Dateien von anderen Systemen heißen auch einmal WERK.BARUCH.
        assert!(is_exchange_name("garten.baruch"), ".baruch");
        assert!(is_exchange_name("Garten.BARUCH.XML"), ".baruch.xml groß");
        // Typst-Dateien und bloßes XML gehen weiter zum Typst-Import bzw. werden abgelehnt.
        assert!(!is_exchange_name("garten.typ"), ".typ ist Typst");
        assert!(!is_exchange_name("garten.xml"), "bloßes .xml ist keine Austauschdatei");
        assert!(!is_exchange_name("baruch"), "ohne Punkt keine Endung");
    }
}
