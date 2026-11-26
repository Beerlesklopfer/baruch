// SPDX-License-Identifier: AGPL-3.0-or-later
//! RPC der Rollen (Runde 35, Vertrag §2.3): Freigaben (`member.*`, nur volle Rechte) und die Schreibwege eines
//! Lektors (`revision.propose`, `finding.add`).

use serde_json::Value;

use crate::AppState;
use crate::rpc::{REFUSED, RpcError};

pub const METHODS: &[&str] = &["member.list", "member.grant", "member.revoke", "revision.propose", "finding.add"];

/// `crate::rpc` fragt vor den Fachmodulen, ob die Methode hierher gehört.
pub fn handles(method: &str) -> bool {
    METHODS.contains(&method)
}

pub async fn call(state: &AppState, method: &str, params: Value) -> Result<Value, RpcError> {
    // STUB: member.list/grant/revoke, revision.propose, finding.add (Runde 35)
    let _ = (state, params);
    Err(RpcError::new(REFUSED, format!("{method}: noch nicht umgesetzt (Rollen, Runde 35)")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_five_methods_belong_to_this_module() {
        for method in METHODS {
            assert!(handles(method), "{method} gehört den Rollen");
        }
        assert!(!handles("finding.list"), "Befunde lesen bleibt bei findings::rpc");
        assert!(!handles("revision.save"), "Speichern bleibt dem Autor und bei rpc.rs");
    }
}
