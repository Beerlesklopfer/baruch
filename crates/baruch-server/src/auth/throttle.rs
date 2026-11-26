// SPDX-License-Identifier: AGPL-3.0-or-later
//! Sperre gegen Durchprobieren (Runde 35, Vertrag §1.3, Tabelle `login_attempt`): ab dem vierten Fehlversuch
//! wachsende Wartezeit je Konto und je Adresse.
//!
//! Bereiche: `konto:<Namensschlüssel>` – immer, auch für Namen, die es nicht gibt (sonst verriete die Sperre, welche
//! Konten bestehen) – und `adresse:<IP>`, wenn die Gegenstelle bekannt ist und die Anfrage nicht über den Proxy kam
//! (hinter dem Proxy trügen alle dieselbe Adresse). Während der Sperre wird kein Passwort geprüft. Die Tabelle
//! wächst nicht über 1000 Zeilen: Ist sie voll, bekommt ein neuer Bereich keine Zeile, stattdessen wartet der Bereich
//! `alle` zwei Sekunden (`baruch_store::Store::login_failure`). Einträge, die älter als 24 Stunden sind, fallen
//! beim nächsten Fehlversuch weg.

use std::net::SocketAddr;

use axum::http::HeaderMap;
use baruch_store::{Store, StoreError};

use crate::AppState;

/// So viele Fehlversuche bleiben ohne Wartezeit.
pub const FREE_FAILURES: i64 = 3;
/// Längste Wartezeit in Sekunden (15 Minuten).
pub const MAX_WAIT_SECONDS: u64 = 900;
/// Ein Kontoname hat höchstens 40 Zeichen; längere Eingaben teilen sich den Bereich ihres Anfangs.
const NAME_KEY_CHARS: usize = 40;

/// Wartezeit nach dem `failures`-ten Fehlversuch in Folge: bis zum dritten keine, ab dem vierten 2^(n−3) Sekunden
/// (2, 4, 8 …), höchstens [`MAX_WAIT_SECONDS`].
pub fn wait_seconds(failures: i64) -> u64 {
    if failures <= FREE_FAILURES {
        return 0;
    }
    // 2^10 liegt schon über der Obergrenze; so läuft die Verschiebung nie über.
    let exponent = (failures - FREE_FAILURES).min(10) as u32;
    (1u64 << exponent).min(MAX_WAIT_SECONDS)
}

/// Bereich des Kontos zu einem eingegebenen Namen.
pub fn account_scope(name: &str) -> String {
    let key: String = Store::account_name_key(name).chars().take(NAME_KEY_CHARS).collect();
    format!("konto:{key}")
}

/// Die Bereiche eines Anmeldeversuchs: immer das Konto, dazu die Adresse der Gegenstelle, wenn sie bekannt ist und
/// die Anfrage nicht über den Proxy kam.
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

/// Verbleibende Wartezeit in Sekunden, wenn einer der Bereiche (oder `alle`) gesperrt ist.
pub async fn blocked(state: &AppState, scopes: &[String]) -> Result<Option<u64>, StoreError> {
    state.store.login_wait(&borrowed(scopes)).await
}

/// Zählt den Fehlversuch in jedem Bereich; liefert die längste Wartezeit, die daraus folgt (0 = keine).
pub async fn failed(state: &AppState, scopes: &[String]) -> Result<u64, StoreError> {
    let mut longest = 0;
    for scope in scopes {
        longest = longest.max(state.store.login_failure(scope, wait_seconds).await?);
    }
    Ok(longest)
}

/// Erfolg setzt die Bereiche zurück.
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
