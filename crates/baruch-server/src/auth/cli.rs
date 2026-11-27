// SPDX-License-Identifier: AGPL-3.0-or-later
//! Emergency path for a forgotten password (Round 35, contract §1.7): `baruch-server passwort <name> [--data <dir>]`.
//!
//! Whoever may read and write the data folder may do everything anyway; the command only changes the password of an
//! existing account (creates none, does not lift a lock of the account set by the author) and ends its
//! sessions. Since Round 36 it also lifts the brute-force lock for this account
//! ([`super::throttle::account_scope`]): whoever keeps the author in the waiting time with wrong logins does not
//! lock him out this way. The lock per address stays. It opens only the database, no mirror and no server, and also
//! runs next to a running server.

use std::path::Path;

use baruch_store::Store;

use super::{password, throttle};

/// Sets the password of the account `name` in the data folder `data`, deletes its sessions and lifts the
/// brute-force lock for this account. The error message never names the password.
pub async fn reset_password(data: &Path, name: &str, password: &str) -> Result<(), String> {
    match password::policy(password) {
        Ok(()) => {}
        Err("passwort_kurz") => {
            return Err(format!("Das Passwort braucht mindestens {} Zeichen.", password::MIN_PASSWORD_CHARS));
        }
        Err(_) => return Err(format!("Das Passwort darf höchstens {} Zeichen haben.", password::MAX_PASSWORD_CHARS)),
    }
    if !data.join("baruch.sqlite").is_file() {
        return Err(format!("In {} liegt keine Datenbank (baruch.sqlite).", data.display()));
    }
    let store = Store::open(data).await.map_err(|e| format!("Datenordner {}: {e}", data.display()))?;
    let result = set(&store, name, password).await;
    store.pool().close().await;
    result
}

async fn set(store: &Store, name: &str, password: &str) -> Result<(), String> {
    let found = store.account_for_login(name).await.map_err(|e| e.to_string())?;
    let Some((account, _)) = found else {
        let known = store.account_list().await.map_err(|e| e.to_string())?;
        return Err(if known.is_empty() {
            "Es gibt noch kein Konto. Das erste Konto wird im Browser unter /einrichten angelegt.".to_owned()
        } else {
            let names: Vec<&str> = known.iter().map(|account| account.name.as_str()).collect();
            format!("Das Konto „{name}“ gibt es nicht. Vorhanden: {}.", names.join(", "))
        });
    };
    let phc = password::hash_blocking(password.to_owned()).await?;
    // Without exception: all sessions of the account end.
    store.account_set_password(account.id, &phc, None).await.map_err(|e| e.to_string())?;
    // Round 36: the brute-force lock of this account is dropped so that the login with the new password
    // succeeds immediately. The scope hangs on the name key; every spelling of the name hits the same one.
    store.login_reset(&throttle::account_scope(&account.name)).await.map_err(|e| e.to_string())?;
    Ok(())
}
