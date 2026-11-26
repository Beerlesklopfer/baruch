// SPDX-License-Identifier: AGPL-3.0-or-later
//! RPC `account.*` (Runde 35, Vertrag §1.6). Anmelden, Abmelden und Einrichten gibt es nur als Formular (Cookie);
//! Passwörter stehen nie in einer Antwort.

use serde_json::Value;

use crate::AppState;
use crate::rpc::{REFUSED, RpcError};

pub const METHODS: &[&str] = &[
    "account.me",
    "account.list",
    "account.create",
    "account.set_active",
    "account.set_password",
    "account.set_local_open",
    "account.logout_all",
];

/// `crate::rpc` fragt vor jedem anderen Modul, ob die Methode hierher gehört.
pub fn handles(method: &str) -> bool {
    METHODS.contains(&method)
}

pub async fn call(state: &AppState, method: &str, params: Value) -> Result<Value, RpcError> {
    // STUB: account.me/list/create/set_active/set_password/set_local_open/logout_all (Runde 35)
    let _ = (state, params);
    Err(RpcError::new(REFUSED, format!("{method}: noch nicht umgesetzt (Konten, Runde 35)")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_account_methods_belong_to_this_module() {
        for method in METHODS {
            assert!(handles(method), "{method} gehört der Anmeldung");
            assert!(method.starts_with("account."), "{method}: alle Methoden der Anmeldung heißen account.*");
        }
        assert!(!handles("account.delete"), "Konten werden nie gelöscht – die Methode gibt es nicht");
        assert!(!handles("work.list"), "fremde Methoden gehören nicht hierher");
    }
}
