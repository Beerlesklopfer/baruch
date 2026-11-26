// SPDX-License-Identifier: AGPL-3.0-or-later
//! Konten (Runde 35, Vertrag §S1.3 und §1; Migration `0053_accounts.sql`): wer sich an Baruch anmelden darf.
//!
//! Instanzdaten: Konten stehen nicht im Austauschformat und nicht im Spiegel. Der Store hasht nie selbst und sieht
//! nie ein Klartext-Passwort – der Server übergibt den fertigen Argon2id-PHC-Text ([`NewAccount::password_phc`]);
//! [`Account`] trägt keinen Hash. Ohne Konten (Tabelle leer, [`Store::account_count`] = 0) bleibt Baruch offen wie
//! vor Runde 35.
//!
//! Schnittstelle R35-S1: Typen und [`Store::account_count`] sind echt, die übrigen Methoden antworten ehrlich mit
//! [`NOT_YET`]; „konten“ setzt sie um und legt die weiteren Methoden (Passwort, Sitzung, Sperre, `auth.local_open`)
//! selbst fest.

use serde::Serialize;

use crate::{Store, StoreError};

/// Meldung der Stubs dieser Runde (Konten und Freigaben).
pub(crate) const NOT_YET: &str = "Konten: noch nicht umgesetzt (Runde 35)";

/// Ein Konto ohne Geheimnis (kein Hash, kein Klartext). `kind` ist `author` oder `lektor`; `slug` ist der Teil des
/// Handles nach `@lektorat:` (eindeutig, bleibt auch einem gesperrten Konto).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, sqlx::FromRow)]
pub struct Account {
    pub id: i64,
    pub name: String,
    pub slug: String,
    pub display: String,
    pub kind: String,
    pub active: bool,
    pub created: String,
    pub last_login: Option<String>,
}

/// Neues Konto. `password_phc` ist der fertige Argon2id-PHC-Text (`$argon2id$…`), nie ein Klartext-Passwort – darum
/// bewusst ohne `Debug`.
pub struct NewAccount<'a> {
    pub name: &'a str,
    pub slug: &'a str,
    pub display: &'a str,
    pub kind: &'a str,
    pub password_phc: &'a str,
}

impl Store {
    /// Anzahl der Konten (auch gesperrte). 0 heißt: Baruch läuft ohne Anmeldung.
    pub async fn account_count(&self) -> Result<i64, StoreError> {
        Ok(sqlx::query_scalar("SELECT count(*) FROM account").fetch_one(self.pool()).await?)
    }

    /// Alle Konten nach id.
    pub async fn account_list(&self) -> Result<Vec<Account>, StoreError> {
        // STUB: Liste der Konten lesen (Runde 35)
        Err(StoreError::Refused(NOT_YET))
    }

    /// Ein Konto; `None`, wenn es die id nicht gibt.
    pub async fn account_get(&self, id: i64) -> Result<Option<Account>, StoreError> {
        // STUB: Konto lesen (Runde 35)
        let _ = id;
        Err(StoreError::Refused(NOT_YET))
    }

    /// Konto anlegen (Name und Handle eindeutig).
    pub async fn account_create(&self, new: &NewAccount<'_>) -> Result<Account, StoreError> {
        // STUB: Konto anlegen (Runde 35)
        let _ = new;
        Err(StoreError::Refused(NOT_YET))
    }

    /// Das erste Konto anlegen: nur wenn die Tabelle leer ist, in einer Transaktion.
    pub async fn account_create_first(&self, new: &NewAccount<'_>) -> Result<Account, StoreError> {
        // STUB: erstes Konto anlegen, nur bei leerer Tabelle (Runde 35)
        let _ = new;
        Err(StoreError::Refused(NOT_YET))
    }
}
