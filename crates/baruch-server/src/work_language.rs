// SPDX-License-Identifier: AGPL-3.0-or-later
//! Work language over RPC (Round 38, interface R38-S1): `work.language` and `work.language_set`.
//!
//! The language itself lives in the store ([`baruch_store::Store::work_language`], migration 0055). Setting it is
//! reserved for the human with full rights: not through `/act`, not through MCP (pattern `work.timing_set`).

use baruch_store::WorkId;
use serde_json::Value;

use crate::AppState;
use crate::rpc::RpcError;

/// Message of the stub answer until task mcp_rpc implements the methods.
pub const NOT_IMPLEMENTED: &str = "work.language: not implemented yet (Round 38, mcp_rpc)";

/// Does this module serve the RPC method? `work.language`, `work.language_set`.
pub fn handles(method: &str) -> bool {
    matches!(method, "work.language" | "work.language_set")
}

/// Serves `work.language` and `work.language_set`.
// STUB: work language RPC (Round 38 mcp_rpc)
pub async fn call(_state: &AppState, _work: WorkId, _method: &str, _params: Value) -> Result<Value, RpcError> {
    Err(RpcError::new(crate::rpc::REFUSED, NOT_IMPLEMENTED))
}

#[cfg(test)]
mod tests {
    use super::handles;

    #[test]
    fn only_the_two_language_methods_are_handled() {
        for method in ["work.language", "work.language_set"] {
            assert!(handles(method), "the dispatcher in rpc.rs sends {method} to this module");
        }
        for method in ["work.languages", "work.timing", "work.get", "language", ""] {
            assert!(!handles(method), "{method:?} belongs to another module and must not be swallowed here");
        }
    }
}
