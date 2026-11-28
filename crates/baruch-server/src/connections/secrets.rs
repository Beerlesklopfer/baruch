// SPDX-License-Identifier: AGPL-3.0-or-later
//! Secrets of the connections (key, token, password) – only in the keyring, never in SQLite,
//! HTML, RPC responses, error messages, check results or logs.
//!
//! [`SecretStore`] is the only door: in operation [`KeyringSecrets`] (keyring of the system,
//! on Linux the Secret Service over D-Bus, on macOS the Keychain, on Windows the
//! Credential Manager – per target system a feature of the crate `keyring`, see `Cargo.toml`; service
//! [`SERVICE`], accounts `eliezer`, `multigpt`, `pgvector`), in tests [`MemorySecrets`] (`AppState::open` takes it
//! so that no test touches the real
//! keyring). The methods are blocking; the server calls them only through [`get`],
//! [`set`] and [`delete`], which put every call into `spawn_blocking`.
//!
//! Error texts never name the secret: `keyring::Error` describes only the kind of the error (the
//! bytes of `BadEncoding` are not output).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Service name in the keyring.
pub const SERVICE: &str = "baruch";

/// Storage for secrets per account (`eliezer`, `multigpt`, `pgvector`) under the service “baruch”.
pub trait SecretStore: Send + Sync {
    /// Read a secret; `Ok(None)` if none is stored.
    fn get(&self, account: &str) -> Result<Option<String>, String>;
    /// Store a secret (replaces an existing one).
    fn set(&self, account: &str, secret: &str) -> Result<(), String>;
    /// Remove a secret; if it is already missing, that is no error.
    fn delete(&self, account: &str) -> Result<(), String>;
}

/// The keyring of the system through the crate `keyring` (Linux: Secret Service, macOS: Keychain,
/// Windows: Credential Manager). The backends for macOS and Windows have been built since Round 24,
/// but tried on no real device. On a system without a backend of its own (e.g. FreeBSD) `keyring` knows
/// only an in-process store: secrets do not survive the restart there.
#[derive(Debug, Default, Clone, Copy)]
pub struct KeyringSecrets;

/// Error text without content: only the kind of the error.
fn keyring_error(e: &keyring::Error) -> String {
    match e {
        keyring::Error::NoStorageAccess(_) => crate::i18n::tc("connections_msg.keyring.no_access", &[]),
        keyring::Error::PlatformFailure(inner) => {
            crate::i18n::tc("connections_msg.keyring.failure", &[("detail", inner.to_string().into())])
        }
        keyring::Error::BadEncoding(_) => crate::i18n::tc("connections_msg.keyring.encoding", &[]),
        keyring::Error::Ambiguous(found) => {
            crate::i18n::tc("connections_msg.keyring.ambiguous", &[("count", found.len().into())])
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

/// Secrets in memory (tests, throwaway servers): nothing leaves the process.
#[derive(Debug, Default)]
pub struct MemorySecrets {
    secrets: Mutex<HashMap<String, String>>,
}

impl MemorySecrets {
    /// Pre-filled with pairs `(account, secret)`.
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

/// Where `serve` keeps the secrets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecretsMode {
    /// Keyring of the system ([`KeyringSecrets`]).
    Keyring,
    /// Only in the memory of the process ([`MemorySecrets`]); gone on exit.
    Memory,
}

impl SecretsMode {
    /// Line for the start-up output.
    pub fn describe(self) -> &'static str {
        match self {
            SecretsMode::Keyring => "Schlüsselbund des Systems (Dienst „baruch“)",
            SecretsMode::Memory => {
                "nur im Speicher dieses Prozesses (anderer Datenordner als die Vorgabe; --secrets keyring für den Schlüsselbund)"
            }
        }
    }
}

/// The same folder? Both exist: compared canonically, otherwise literally.
fn same_dir(a: &std::path::Path, b: &std::path::Path) -> bool {
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => a == b,
    }
}

/// Storage of the secrets for `serve`: explicitly `keyring` or `memory` (`--secrets`,
/// `BARUCH_SECRETS`), otherwise the keyring only if `data` is the default data folder `default`.
/// The entries in the keyring hang only on service “baruch” and account, not on the data folder; a
/// throwaway server with its own `--data` would otherwise share them with the actual service.
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

/// The keyring of the system for `serve`.
pub fn system_secrets() -> Arc<dyn SecretStore> {
    Arc::new(KeyringSecrets)
}

/// [`SecretStore::get`] outside the async runtime thread (D-Bus blocks).
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
        // A throwaway or test server with its own --data must not touch the entries of the service.
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
