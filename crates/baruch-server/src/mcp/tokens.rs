// SPDX-License-Identifier: AGPL-3.0-or-later
//! Tokens für den MCP-Zugang von anderen Rechnern (Runde 10, Aufgabe „scripture“): RPC
//! `mcp_token.list`, `mcp_token.create`, `mcp_token.revoke`. Gespeichert wird nur der SHA-256-Hash
//! ([`baruch_store::Store::mcp_token_create`]); den Klartext trägt genau eine Antwort, die von
//! `mcp_token.create`. Nur der Mensch verwaltet Tokens: nicht über `POST /act` und nicht über MCP.
//! Ohne JavaScript dasselbe über die Einstellungsseite „MCP-Zugang“ (`POST /einstellungen/mcp/act`).
//!
//! Runde 23: RPC und Formular legen über [`create`] an. Ein Name, dessen Handle ([`super::session::handle_name`]) oder
//! Alt-Handle ([`super::session::legacy_handle_name`]) dem eines vorhandenen oder widerrufenen Tokens gleicht oder mit
//! `lokal-` beginnt, wird abgelehnt (`StoreError::Refused`, RPC -32000, Formular 422); nichts angelegt.

use serde_json::{Value, json};

use crate::AppState;
use crate::rpc::{INVALID_PARAMS, METHOD_NOT_FOUND, RpcError};

/// Token anlegen, eindeutig im Handle (Runde 23, siehe Moduldoku); der einzige Weg von RPC und Formular.
pub async fn create(
    store: &baruch_store::Store,
    name: &str,
) -> Result<(baruch_store::McpToken, String), baruch_store::StoreError> {
    use super::session::{LOCAL_PREFIX, handle_name, legacy_handle_name};
    store.mcp_token_create_unique(name, handle_name, legacy_handle_name, LOCAL_PREFIX).await
}

const METHODS: &[&str] = &["mcp_token.list", "mcp_token.create", "mcp_token.revoke"];

/// Bedient dieses Modul die RPC-Methode?
pub fn handles(method: &str) -> bool {
    METHODS.contains(&method)
}

/// RPC `mcp_token.*` (`docs/RPC.md`, „MCP-Tokens“).
pub async fn call(state: &AppState, method: &str, params: Value) -> Result<Value, RpcError> {
    match method {
        "mcp_token.list" => Ok(json!({ "tokens": state.store.mcp_token_list().await? })),
        "mcp_token.create" => {
            let name = params
                .get("name")
                .and_then(Value::as_str)
                .ok_or_else(|| RpcError::new(INVALID_PARAMS, "name fehlt (z. B. der Name des Rechners)"))?;
            let (token, secret) = create(&state.store, name).await?;
            Ok(json!({ "token": token, "secret": secret }))
        }
        "mcp_token.revoke" => {
            if params.get("confirm") != Some(&Value::Bool(true)) {
                return Err(RpcError::new(INVALID_PARAMS, "Widerrufen bitte bestätigen (confirm: true)."));
            }
            let id = params
                .get("id")
                .and_then(Value::as_i64)
                .ok_or_else(|| RpcError::new(INVALID_PARAMS, "id fehlt (Zahl aus mcp_token.list)"))?;
            Ok(json!({ "token": state.store.mcp_token_revoke(id).await? }))
        }
        _ => Err(RpcError::new(METHOD_NOT_FOUND, format!("unbekannte Methode {method}"))),
    }
}
