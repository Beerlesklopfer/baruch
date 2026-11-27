// SPDX-License-Identifier: AGPL-3.0-or-later
//! Tokens for MCP access from other machines (Round 10, task „scripture“): RPC
//! `mcp_token.list`, `mcp_token.create`, `mcp_token.revoke`. Only the SHA-256 hash is stored
//! ([`baruch_store::Store::mcp_token_create`]); exactly one answer carries the plain text, that of
//! `mcp_token.create`. Only the human manages tokens: not through `POST /act` and not through MCP.
//! Without JavaScript the same through the settings page „MCP-Zugang“ (`POST /einstellungen/mcp/act`).
//!
//! Round 23: RPC and form create through [`create`]. A name whose handle ([`super::session::handle_name`]) or
//! legacy handle ([`super::session::legacy_handle_name`]) equals that of an existing or revoked token or begins
//! with `lokal-` is refused (`StoreError::Refused`, RPC -32000, form 422); nothing created.

use serde_json::{Value, json};

use crate::AppState;
use crate::rpc::{INVALID_PARAMS, METHOD_NOT_FOUND, RpcError};

/// Create a token, unique in its handle (Round 23, see the module doc); the only path of RPC and form.
pub async fn create(
    store: &baruch_store::Store,
    name: &str,
) -> Result<(baruch_store::McpToken, String), baruch_store::StoreError> {
    use super::session::{LOCAL_PREFIX, handle_name, legacy_handle_name};
    store.mcp_token_create_unique(name, handle_name, legacy_handle_name, LOCAL_PREFIX).await
}

const METHODS: &[&str] = &["mcp_token.list", "mcp_token.create", "mcp_token.revoke"];

/// Does this module serve the RPC method?
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
        _ => Err(RpcError::new(METHOD_NOT_FOUND, format!("unknown method {method}"))),
    }
}
