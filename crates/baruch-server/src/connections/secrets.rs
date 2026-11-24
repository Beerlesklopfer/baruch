// SPDX-License-Identifier: AGPL-3.0-or-later
//! Geheimnisse der Verbindungen (Schlüssel, Token, Passwort) – nur im Schlüsselbund, nie in SQLite,
//! HTML, RPC-Antworten, Fehlermeldungen, Prüfergebnissen oder Protokollen.
//!
//! [`SecretStore`] ist die einzige Tür: im Betrieb [`KeyringSecrets`] (Schlüsselbund des Systems,
//! unter Linux der Secret Service über D-Bus, unter macOS der Schlüsselbund (Keychain), unter Windows die
//! Anmeldeinformationsverwaltung – je Zielsystem ein Feature der Crate `keyring`, siehe `Cargo.toml`; Dienst
//! [`SERVICE`], Konten `eliezer`, `multigpt`, `pgvector`), in Tests [`MemorySecrets`] (`AppState::open` nimmt es, damit kein Test den echten
//! Schlüsselbund berührt). Die Methoden sind blockierend; der Server ruft sie nur über [`get`],
//! [`set`] und [`delete`] auf, die jeden Aufruf in `spawn_blocking` legen.
//!
//! Fehlertexte nennen nie das Geheimnis: `keyring::Error` beschreibt nur die Art des Fehlers (die
//! Bytes bei `BadEncoding` werden nicht ausgegeben).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Dienstname im Schlüsselbund.
pub const SERVICE: &str = "baruch";

/// Ablage für Geheimnisse je Konto (`eliezer`, `multigpt`, `pgvector`) unter dem Dienst „baruch“.
pub trait SecretStore: Send + Sync {
    /// Geheimnis lesen; `Ok(None)`, wenn keins hinterlegt ist.
    fn get(&self, account: &str) -> Result<Option<String>, String>;
    /// Geheimnis hinterlegen (ersetzt ein vorhandenes).
    fn set(&self, account: &str, secret: &str) -> Result<(), String>;
    /// Geheimnis entfernen; fehlt es schon, ist das kein Fehler.
    fn delete(&self, account: &str) -> Result<(), String>;
}

/// Der Schlüsselbund des Systems über die Crate `keyring` (Linux: Secret Service, macOS: Schlüsselbund,
/// Windows: Anmeldeinformationsverwaltung). Die Zugänge für macOS und Windows sind seit Runde 24 gebaut,
/// aber auf keinem echten Gerät erprobt. Auf einem System ohne eigenen Zugang (z. B. FreeBSD) kennt `keyring`
/// nur einen Speicher im Prozess: Geheimnisse überleben dort den Neustart nicht.
#[derive(Debug, Default, Clone, Copy)]
pub struct KeyringSecrets;

/// Fehlertext ohne Inhalt: nur die Art des Fehlers.
fn keyring_error(e: &keyring::Error) -> String {
    match e {
        keyring::Error::NoStorageAccess(_) => {
            "kein Zugriff auf den Schlüsselbund (gesperrt oder nicht erreichbar)".into()
        }
        keyring::Error::PlatformFailure(inner) => format!("Fehler des Schlüsselbunds: {inner}"),
        keyring::Error::BadEncoding(_) => "der hinterlegte Wert ist kein UTF-8-Text".into(),
        keyring::Error::Ambiguous(found) => {
            format!("{} Einträge passen; bitte im Schlüsselbund aufräumen", found.len())
        }
        other => other.to_string(),
    }
}

impl KeyringSecrets {
    fn entry(account: &str) -> Result<keyring::Entry, String> {
        keyring::Entry::new(SERVICE, account).map_err(|e| keyring_error(&e))
    }
}

impl SecretStore for KeyringSecrets {
    fn get(&self, account: &str) -> Result<Option<String>, String> {
        match Self::entry(account)?.get_password() {
            Ok(secret) => Ok(Some(secret)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(keyring_error(&e)),
        }
    }

    fn set(&self, account: &str, secret: &str) -> Result<(), String> {
        Self::entry(account)?.set_password(secret).map_err(|e| keyring_error(&e))
    }

    fn delete(&self, account: &str) -> Result<(), String> {
        match Self::entry(account)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(keyring_error(&e)),
        }
    }
}

/// Geheimnisse im Speicher (Tests, Wegwerf-Server): nichts verlässt den Prozess.
#[derive(Debug, Default)]
pub struct MemorySecrets {
    secrets: Mutex<HashMap<String, String>>,
}

impl MemorySecrets {
    /// Mit Paaren `(Konto, Geheimnis)` vorbelegt.
    pub fn with<'a>(pairs: impl IntoIterator<Item = (&'a str, &'a str)>) -> MemorySecrets {
        let secrets = pairs.into_iter().map(|(a, s)| (a.to_owned(), s.to_owned())).collect();
        MemorySecrets { secrets: Mutex::new(secrets) }
    }

    fn map(&self) -> std::sync::MutexGuard<'_, HashMap<String, String>> {
        self.secrets.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

impl SecretStore for MemorySecrets {
    fn get(&self, account: &str) -> Result<Option<String>, String> {
        Ok(self.map().get(account).cloned())
    }

    fn set(&self, account: &str, secret: &str) -> Result<(), String> {
        self.map().insert(account.to_owned(), secret.to_owned());
        Ok(())
    }

    fn delete(&self, account: &str) -> Result<(), String> {
        self.map().remove(account);
        Ok(())
    }
}

/// Wo `serve` die Geheimnisse hält.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecretsMode {
    /// Schlüsselbund des Systems ([`KeyringSecrets`]).
    Keyring,
    /// Nur im Speicher des Prozesses ([`MemorySecrets`]); weg beim Beenden.
    Memory,
}

impl SecretsMode {
    /// Zeile für die Startausgabe.
    pub fn describe(self) -> &'static str {
        match self {
            SecretsMode::Keyring => "Schlüsselbund des Systems (Dienst „baruch“)",
            SecretsMode::Memory => {
                "nur im Speicher dieses Prozesses (anderer Datenordner als die Vorgabe; --secrets keyring für den Schlüsselbund)"
            }
        }
    }
}

/// Derselbe Ordner? Beide vorhanden: kanonisch verglichen, sonst wörtlich.
fn same_dir(a: &std::path::Path, b: &std::path::Path) -> bool {
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => a == b,
    }
}

/// Ablage der Geheimnisse für `serve`: ausdrücklich `keyring` oder `memory` (`--secrets`,
/// `BARUCH_SECRETS`), sonst der Schlüsselbund nur, wenn `data` der Vorgabe-Datenordner `default` ist.
/// Die Einträge im Schlüsselbund hängen nur an Dienst „baruch“ und Konto, nicht am Datenordner; ein
/// Wegwerf-Server mit eigenem `--data` teilte sie sonst mit dem eigentlichen Dienst.
pub fn secrets_mode(
    given: Option<&str>,
    data: &std::path::Path,
    default: &std::path::Path,
) -> Result<SecretsMode, String> {
    match given.map(str::trim) {
        Some("keyring") => Ok(SecretsMode::Keyring),
        Some("memory") => Ok(SecretsMode::Memory),
        Some(other) => Err(format!("--secrets: keyring oder memory, nicht „{other}“")),
        None if same_dir(data, default) => Ok(SecretsMode::Keyring),
        None => Ok(SecretsMode::Memory),
    }
}

/// Der Schlüsselbund des Systems für `serve`.
pub fn system_secrets() -> Arc<dyn SecretStore> {
    Arc::new(KeyringSecrets)
}

/// [`SecretStore::get`] außerhalb des asynchronen Laufzeit-Threads (D-Bus blockiert).
pub async fn get(store: &Arc<dyn SecretStore>, account: &str) -> Result<Option<String>, String> {
    let (store, account) = (store.clone(), account.to_owned());
    tokio::task::spawn_blocking(move || store.get(&account)).await.map_err(|e| format!("Aufgabe abgebrochen: {e}"))?
}

/// [`SecretStore::set`] in `spawn_blocking`.
pub async fn set(store: &Arc<dyn SecretStore>, account: &str, secret: &str) -> Result<(), String> {
    let (store, account, secret) = (store.clone(), account.to_owned(), secret.to_owned());
    tokio::task::spawn_blocking(move || store.set(&account, &secret))
        .await
        .map_err(|e| format!("Aufgabe abgebrochen: {e}"))?
}

/// [`SecretStore::delete`] in `spawn_blocking`.
pub async fn delete(store: &Arc<dyn SecretStore>, account: &str) -> Result<(), String> {
    let (store, account) = (store.clone(), account.to_owned());
    tokio::task::spawn_blocking(move || store.delete(&account))
        .await
        .map_err(|e| format!("Aufgabe abgebrochen: {e}"))?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_default_data_folder_reaches_the_system_keyring() {
        let root = tempfile::tempdir().unwrap();
        let default = root.path().join("baruch");
        std::fs::create_dir(&default).unwrap();
        let other = root.path().join("wegwerf");
        std::fs::create_dir(&other).unwrap();
        // Ein Wegwerf- oder Prüfserver mit eigenem --data darf die Einträge des Dienstes nicht berühren.
        assert_eq!(secrets_mode(None, &other, &default), Ok(SecretsMode::Memory), "anderer Ordner → Speicher");
        assert_eq!(secrets_mode(None, &default, &default), Ok(SecretsMode::Keyring), "Vorgabe → Schlüsselbund");
        let same = root.path().join(".").join("baruch");
        assert_eq!(
            secrets_mode(None, &same, &default),
            Ok(SecretsMode::Keyring),
            "derselbe Ordner, anders geschrieben"
        );
        let missing = root.path().join("fehlt");
        assert_eq!(
            secrets_mode(None, &missing, &default),
            Ok(SecretsMode::Memory),
            "noch nicht angelegt, anderer Pfad"
        );
        assert_eq!(secrets_mode(Some("keyring"), &other, &default), Ok(SecretsMode::Keyring), "ausdrücklich");
        assert_eq!(secrets_mode(Some(" memory "), &default, &default), Ok(SecretsMode::Memory), "ausdrücklich");
        assert!(secrets_mode(Some("datei"), &default, &default).is_err(), "unbekannter Wert → Fehler");
    }

    #[test]
    fn memory_secrets_set_replace_and_delete() {
        let store = MemorySecrets::with([("eliezer", "eins")]);
        assert_eq!(store.get("eliezer").unwrap().as_deref(), Some("eins"), "vorbelegt");
        store.set("eliezer", "zwei").unwrap();
        assert_eq!(store.get("eliezer").unwrap().as_deref(), Some("zwei"), "ersetzt");
        store.delete("eliezer").unwrap();
        store.delete("eliezer").unwrap();
        assert_eq!(store.get("eliezer").unwrap(), None, "entfernt; zweimal entfernen ist kein Fehler");
    }
}
