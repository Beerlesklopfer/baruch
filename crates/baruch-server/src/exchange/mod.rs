// SPDX-License-Identifier: AGPL-3.0-or-later
//! Exchange between instances (Round 26, doc `docs/Austauschformat.md`; after S1 integrator only).
//!
//! - [`export`]: page `/werke/{id}/austausch`, downloads `.baruch`/`.baruch.xml`, RPC `work.exchange_export`,
//!   CLI `export … --baruch DATEI` (R26b).
//! - [`import`]: page `/austausch/einlesen`, forms `POST /austausch/vorschau` and
//!   `POST /austausch/uebernehmen`, RPC `work.exchange_preview`/`work.exchange_import`, CLI `import DATEI.baruch`
//!   (R26c). Importing only by the human, not through MCP.
//!
//! RPC without default work: [`crate::rpc::call`] asks [`handles`] before `works::handles`.

pub mod export;
pub mod import;

use std::sync::Arc;

use axum::Router;
use baruch_exchange::ExchangeError;

use crate::AppState;
use crate::rpc::{INTERNAL, INVALID_PARAMS, METHOD_NOT_FOUND, RpcError};

/// Methods of this module.
pub const METHODS: &[&str] = &["work.exchange_export", "work.exchange_preview", "work.exchange_import"];

pub fn handles(method: &str) -> bool {
    METHODS.contains(&method)
}

/// RPC of the exchange; forwards to [`export`] or [`import`].
pub async fn call(state: &AppState, method: &str, params: serde_json::Value) -> Result<serde_json::Value, RpcError> {
    match method {
        "work.exchange_export" => export::rpc_export(state, params).await,
        "work.exchange_preview" => import::rpc_preview(state, params).await,
        "work.exchange_import" => import::rpc_import(state, params).await,
        other => Err(RpcError::new(METHOD_NOT_FOUND, format!("unknown method: {other}"))),
    }
}

/// Pages (GET), merged in `lib.rs` with the pages.
pub fn pages() -> Router<Arc<AppState>> {
    export::pages().merge(import::pages())
}

/// Forms (POST), merged in `lib.rs` in the writes router behind `guard::same_origin`.
pub fn writes() -> Router<Arc<AppState>> {
    import::writes()
}

/// `true` if the file name ends (ignoring case) in `.baruch` or `.baruch.xml`.
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
        // Case-insensitive: files from other systems are sometimes called WERK.BARUCH.
        assert!(is_exchange_name("garten.baruch"), ".baruch");
        assert!(is_exchange_name("Garten.BARUCH.XML"), ".baruch.xml groß");
        // Typst files and bare XML go on to the Typst import or are rejected.
        assert!(!is_exchange_name("garten.typ"), ".typ ist Typst");
        assert!(!is_exchange_name("garten.xml"), "bloßes .xml ist keine Austauschdatei");
        assert!(!is_exchange_name("baruch"), "ohne Punkt keine Endung");
    }
}
