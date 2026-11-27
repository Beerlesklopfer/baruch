// SPDX-License-Identifier: AGPL-3.0-or-later
//! Brute-force lock (Round 35, contract §1.3, table `login_attempt`): from the fourth failed attempt on a
//! growing waiting time per account and per address.
//!
//! Scopes: `konto:<name key>` – always, also for names that do not exist (otherwise the lock would reveal which
//! accounts exist) – and `adresse:<IP>` if the remote side is known and the request did not come through the proxy
//! (behind the proxy all would carry the same address). During the lock no password is checked. Every attempt is
//! counted before the check ([`reserve`], one transaction with the check of the lock) and taken back on success:
//! simultaneous attempts thus get no further than attempts one after another. The table
//! does not grow beyond 1000 rows: if it is full, a new scope gets no row, instead the scope
//! `alle` waits two seconds (`baruch_store::Store::login_failure`). Entries older than 24 hours are dropped
//! at the next failed attempt.

use std::net::SocketAddr;

use axum::http::HeaderMap;
use baruch_store::{LoginAttempt, Store, StoreError};

use crate::AppState;

/// This many failed attempts stay without waiting time.
pub const FREE_FAILURES: i64 = 3;
/// Longest waiting time in seconds (15 minutes).
pub const MAX_WAIT_SECONDS: u64 = 900;
/// An account name has at most 40 characters; longer inputs share the scope of their beginning.
const NAME_KEY_CHARS: usize = 40;

/// Waiting time after the `failures`-th failed attempt in a row: none up to the third, from the fourth 2^(n−3)
/// seconds (2, 4, 8 …), at most [`MAX_WAIT_SECONDS`].
pub fn wait_seconds(failures: i64) -> u64 {
    if failures <= FREE_FAILURES {
        return 0;
    }
    // 2^10 is already above the upper limit; so the shift never overflows.
    let exponent = (failures - FREE_FAILURES).min(10) as u32;
    (1u64 << exponent).min(MAX_WAIT_SECONDS)
}

/// Scope of the account for an entered name.
pub fn account_scope(name: &str) -> String {
    let key: String = Store::account_name_key(name).chars().take(NAME_KEY_CHARS).collect();
    format!("konto:{key}")
}

/// The scopes of a login attempt: always the account, plus the address of the remote side if it is known and
/// the request did not come through the proxy.
pub fn scopes(state: &AppState, name: &str, peer: Option<SocketAddr>, headers: &HeaderMap) -> Vec<String> {
    let mut scopes = vec![account_scope(name)];
    if let Some(peer) = peer
        && !crate::guard::via_proxy(&state.allowed_hosts, headers)
    {
        scopes.push(format!("adresse:{}", peer.ip().to_canonical()));
    }
    scopes
}

fn borrowed(scopes: &[String]) -> Vec<&str> {
    scopes.iter().map(String::as_str).collect()
}

/// Reserves the attempt **before** the password check, in one transaction: if one of the scopes (or
/// `alle`) is locked, [`LoginAttempt::Blocked`] comes with the remaining waiting time and nothing is counted; else
/// the attempt immediately counts as failed ([`LoginAttempt::Reserved`] with the waiting time that applies on a
/// failure; 0 = none). After a success [`succeeded`] takes the count back. Count first, then check: otherwise
/// a salvo of simultaneous attempts would get past the lock while the first check (Argon2id) is still running.
pub async fn reserve(state: &AppState, scopes: &[String]) -> Result<LoginAttempt, StoreError> {
    state.store.login_reserve(&borrowed(scopes), wait_seconds).await
}

/// Success resets the scopes.
pub async fn succeeded(state: &AppState, scopes: &[String]) -> Result<(), StoreError> {
    for scope in scopes {
        state.store.login_reset(scope).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_wait_doubles_from_the_fourth_failure_up_to_fifteen_minutes() {
        for failures in 0..=3 {
            assert_eq!(wait_seconds(failures), 0, "bis zum dritten Fehlversuch keine Wartezeit ({failures})");
        }
        let expected = [(4, 2), (5, 4), (6, 8), (7, 16), (8, 32), (9, 64), (10, 128), (11, 256), (12, 512)];
        for (failures, seconds) in expected {
            assert_eq!(wait_seconds(failures), seconds, "nach dem {failures}. Fehlversuch 2^(n−3) Sekunden");
        }
        assert_eq!(wait_seconds(13), 900, "1024 Sekunden wären über der Obergrenze: 900");
        assert_eq!(wait_seconds(1_000_000), 900, "auch nach sehr vielen Versuchen höchstens 900 Sekunden");
        assert_eq!(wait_seconds(i64::MAX), 900, "kein Überlauf bei absurden Zahlen");
    }

    #[test]
    fn names_that_are_the_same_account_share_a_scope() {
        assert_eq!(account_scope(" Erika  Beispiel "), "konto:erika beispiel", "Rand, Leerraum und Großschreibung");
        assert_eq!(account_scope("ERIKA BEISPIEL"), account_scope("erika beispiel"), "derselbe Schlüssel");
        let long = account_scope(&"x".repeat(5000));
        assert_eq!(long.chars().count(), 46, "ein überlanger Name sprengt die Spalte scope (200 Zeichen) nicht");
    }
}
