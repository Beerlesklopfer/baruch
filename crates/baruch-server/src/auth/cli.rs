// SPDX-License-Identifier: AGPL-3.0-or-later
//! Notweg bei vergessenem Passwort (Runde 35, Vertrag §1.7): `baruch-server passwort <name> [--data <Ordner>]`.
//!
//! Wer den Datenordner lesen und schreiben darf, darf ohnehin alles; der Befehl ändert nur das Passwort eines
//! vorhandenen Kontos (legt keines an, hebt keine Sperre des Kontos durch den Autor auf) und beendet dessen
//! Sitzungen. Seit Runde 36 hebt er außerdem die Sperre gegen Durchprobieren für dieses Konto auf
//! ([`super::throttle::account_scope`]): Wer den Autor mit falschen Anmeldungen in der Wartezeit hält, sperrt ihn so
//! nicht aus. Die Sperre je Adresse bleibt. Er öffnet nur die Datenbank, keinen Spiegel und keinen Server, und läuft
//! auch neben einem laufenden Server.

use std::path::Path;

use baruch_store::Store;

use super::{password, throttle};

/// Setzt das Passwort des Kontos `name` im Datenordner `data`, löscht dessen Sitzungen und hebt die Sperre gegen
/// Durchprobieren für dieses Konto auf. Die Fehlermeldung nennt nie das Passwort.
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
    // Ohne Ausnahme: Alle Sitzungen des Kontos enden.
    store.account_set_password(account.id, &phc, None).await.map_err(|e| e.to_string())?;
    // Runde 36: Die Sperre gegen Durchprobieren dieses Kontos fällt weg, damit die Anmeldung mit dem neuen Passwort
    // sofort gelingt. Der Bereich hängt am Namensschlüssel; jede Schreibweise des Namens trifft denselben.
    store.login_reset(&throttle::account_scope(&account.name)).await.map_err(|e| e.to_string())?;
    Ok(())
}
