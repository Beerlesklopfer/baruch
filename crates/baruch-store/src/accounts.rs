// SPDX-License-Identifier: AGPL-3.0-or-later
//! Konten (Runde 35, Vertrag §S1.3 und §1; Migration `0053_accounts.sql`): wer sich an Baruch anmelden darf.
//!
//! Instanzdaten: Konten stehen nicht im Austauschformat und nicht im Spiegel. Der Store hasht nie selbst und sieht
//! nie ein Klartext-Passwort – der Server übergibt den fertigen Argon2id-PHC-Text ([`NewAccount::password_phc`]);
//! [`Account`] trägt keinen Hash. Ohne Konten (Tabelle leer, [`Store::account_count`] = 0) bleibt Baruch offen wie
//! vor Runde 35.
//!
//! Dazu gehören die Sitzungen (`account_session`: nur der SHA-256 des Tokens, nie das Token), die Sperre gegen
//! Durchprobieren (`login_attempt`) und der Schalter „Am eigenen Rechner ohne Anmeldung“ (`store_meta`, Schlüssel
//! `auth.local_open`). Alle Zeitpunkte rechnet SQLite (`strftime('%Y-%m-%dT%H:%M:%fZ','now')`), damit sie sich als
//! Text vergleichen lassen. Konten werden nie gelöscht: Der Slug (Handle `@lektorat:<slug>`) bleibt vergeben.
//!
//! Diese Methoden ruft nur das Modul `auth` des Servers. Weil `lib.rs` aus diesem Modul nur [`Account`] und
//! [`NewAccount`] herausreicht, hängen auch Hilfen und Meldungen als zugehörige Funktionen und Konstanten an
//! [`Store`] ([`Store::account_name_key`], [`Store::session_token_hash`], [`Store::ACCOUNT_TAKEN`] …).

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::{Store, StoreError};

/// Meldung der Stubs der Freigaben (`members`), bis „rollen“ sie umsetzt; die Konten selbst sind umgesetzt.
#[allow(dead_code)]
pub(crate) const NOT_YET: &str = "Konten: noch nicht umgesetzt (Runde 35)";

/// Spalten von [`Account`] (ohne Hash).
const COLUMNS: &str = "id, name, slug, display, kind, active, created, last_login";
/// Schlüssel in `store_meta`: `'1'` = am eigenen Rechner ohne Anmeldung (Vorgabe: nicht gesetzt = aus).
const LOCAL_OPEN: &str = "auth.local_open";
/// Bereich der Sperre, wenn `login_attempt` voll ist.
const ALL_SCOPE: &str = "alle";

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

/// Verletzte Eindeutigkeit (Name oder Slug) als fachliche Ablehnung.
fn unique(e: sqlx::Error) -> StoreError {
    match &e {
        sqlx::Error::Database(db) if db.is_unique_violation() => StoreError::Refused(Store::ACCOUNT_TAKEN),
        _ => StoreError::Db(e),
    }
}

impl Store {
    /// Meldung: Name oder Handle (Slug) gehört schon einem Konto.
    pub const ACCOUNT_TAKEN: &'static str = "Diesen Namen gibt es schon.";
    /// Meldung von [`Store::account_create_first`]: Es gibt schon ein Konto.
    pub const ACCOUNT_NOT_FIRST: &'static str = "Es gibt schon ein Konto.";
    /// Meldung von [`Store::account_create`]: ohne Autor-Konto kein Lektor.
    pub const ACCOUNT_NEEDS_AUTHOR: &'static str = "Das erste Konto gehört dem Autor (Einrichten).";
    /// Meldung von [`Store::account_set_active`]: Das letzte aktive Autor-Konto bleibt.
    pub const ACCOUNT_LAST_AUTHOR: &'static str = "Das letzte aktive Autor-Konto lässt sich nicht sperren.";
    /// Höchstzahl der Zeilen in `login_attempt` (samt dem Bereich `alle`).
    pub const LOGIN_ATTEMPT_ROWS: i64 = 1000;
    /// Wartezeit in Sekunden für alle, solange `login_attempt` voll ist.
    pub const LOGIN_FULL_WAIT: u64 = 2;

    /// Anzahl der Konten (auch gesperrte). 0 heißt: Baruch läuft ohne Anmeldung.
    pub async fn account_count(&self) -> Result<i64, StoreError> {
        Ok(sqlx::query_scalar("SELECT count(*) FROM account").fetch_one(self.pool()).await?)
    }

    /// Alle Konten nach id.
    pub async fn account_list(&self) -> Result<Vec<Account>, StoreError> {
        Ok(sqlx::query_as(&format!("SELECT {COLUMNS} FROM account ORDER BY id")).fetch_all(self.pool()).await?)
    }

    /// Ein Konto; `None`, wenn es die id nicht gibt.
    pub async fn account_get(&self, id: i64) -> Result<Option<Account>, StoreError> {
        Ok(sqlx::query_as(&format!("SELECT {COLUMNS} FROM account WHERE id = ?"))
            .bind(id)
            .fetch_optional(self.pool())
            .await?)
    }

    /// Konto anlegen (Name und Handle eindeutig, sonst [`Store::ACCOUNT_TAKEN`]). Ein Lektor-Konto entsteht nur,
    /// wenn es schon ein Autor-Konto gibt ([`Store::ACCOUNT_NEEDS_AUTHOR`]) – sonst gäbe es Konten, aber niemanden
    /// mit vollen Rechten. Das erste Konto legt [`Store::account_create_first`] an.
    pub async fn account_create(&self, new: &NewAccount<'_>) -> Result<Account, StoreError> {
        let created: Option<Account> = sqlx::query_as(&format!(
            "INSERT INTO account (name, name_key, slug, display, kind, password) SELECT ?1, ?2, ?3, ?4, ?5, ?6 \
             WHERE ?5 = 'author' OR EXISTS (SELECT 1 FROM account WHERE kind = 'author') RETURNING {COLUMNS}"
        ))
        .bind(new.name.trim())
        .bind(Self::account_name_key(new.name))
        .bind(new.slug)
        .bind(new.display.trim())
        .bind(new.kind)
        .bind(new.password_phc)
        .fetch_optional(self.pool())
        .await
        .map_err(unique)?;
        created.ok_or(StoreError::Refused(Self::ACCOUNT_NEEDS_AUTHOR))
    }

    /// Das erste Konto anlegen: nur wenn die Tabelle leer ist, sonst [`Store::ACCOUNT_NOT_FIRST`]. Prüfung und
    /// Einfügen sind eine Anweisung (eine Transaktion): Von zwei gleichzeitigen Aufrufen gewinnt genau einer.
    pub async fn account_create_first(&self, new: &NewAccount<'_>) -> Result<Account, StoreError> {
        let created: Option<Account> = sqlx::query_as(&format!(
            "INSERT INTO account (name, name_key, slug, display, kind, password) SELECT ?, ?, ?, ?, ?, ? \
             WHERE NOT EXISTS (SELECT 1 FROM account) RETURNING {COLUMNS}"
        ))
        .bind(new.name.trim())
        .bind(Self::account_name_key(new.name))
        .bind(new.slug)
        .bind(new.display.trim())
        .bind(new.kind)
        .bind(new.password_phc)
        .fetch_optional(self.pool())
        .await
        .map_err(unique)?;
        created.ok_or(StoreError::Refused(Self::ACCOUNT_NOT_FIRST))
    }

    /// Vergleichsschlüssel eines Kontonamens: ohne Rand, Leerraum zusammengezogen, klein. Zwei Namen mit demselben
    /// Schlüssel sind derselbe Name („Erika“ = „ erika “).
    pub fn account_name_key(name: &str) -> String {
        name.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
    }

    /// SHA-256 (hex, 64 Zeichen) eines Sitzungs-Tokens: Nur das steht in der Datenbank.
    pub fn session_token_hash(token: &str) -> String {
        Sha256::digest(token.as_bytes()).iter().map(|byte| format!("{byte:02x}")).collect()
    }

    /// Konto zum Namen samt Argon2id-PHC-Text – nur für die Prüfung beim Anmelden (auch gesperrte Konten).
    pub async fn account_for_login(&self, name: &str) -> Result<Option<(Account, String)>, StoreError> {
        let found: Option<(i64, String)> = sqlx::query_as("SELECT id, password FROM account WHERE name_key = ?")
            .bind(Self::account_name_key(name))
            .fetch_optional(self.pool())
            .await?;
        let Some((id, phc)) = found else { return Ok(None) };
        Ok(self.account_get(id).await?.map(|account| (account, phc)))
    }

    /// Argon2id-PHC-Text eines Kontos (Prüfung des bisherigen Passworts); `None`, wenn es die id nicht gibt.
    pub async fn account_password(&self, id: i64) -> Result<Option<String>, StoreError> {
        Ok(sqlx::query_scalar("SELECT password FROM account WHERE id = ?").bind(id).fetch_optional(self.pool()).await?)
    }

    /// Neues Passwort (fertiger PHC-Text) setzen und in derselben Transaktion alle Sitzungen des Kontos löschen –
    /// außer der mit dem Token-Hash `keep` (die Sitzung, die das Passwort gerade ändert). `false`: unbekanntes Konto.
    pub async fn account_set_password(&self, id: i64, phc: &str, keep: Option<&str>) -> Result<bool, StoreError> {
        let mut tx = self.pool().begin().await?;
        let changed = sqlx::query(
            "UPDATE account SET password = ?, password_changed = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ?",
        )
        .bind(phc)
        .bind(id)
        .execute(&mut *tx)
        .await?
        .rows_affected();
        if changed == 0 {
            return Ok(false);
        }
        sqlx::query("DELETE FROM account_session WHERE account_id = ? AND token_hash <> ?")
            .bind(id)
            .bind(keep.unwrap_or(""))
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(true)
    }

    /// Konto sperren (`active = false`, löscht alle seine Sitzungen) oder entsperren. `Ok(false)`: unbekanntes
    /// Konto. Das letzte aktive Autor-Konto lässt sich nicht sperren ([`Store::ACCOUNT_LAST_AUTHOR`]) – Prüfung und
    /// Änderung sind eine Anweisung.
    pub async fn account_set_active(&self, id: i64, active: bool) -> Result<bool, StoreError> {
        let mut tx = self.pool().begin().await?;
        let changed = sqlx::query(
            "UPDATE account SET active = ?1 WHERE id = ?2 AND (?1 = 1 OR kind <> 'author' OR active = 0 OR \
             (SELECT count(*) FROM account WHERE kind = 'author' AND active = 1 AND id <> ?2) > 0)",
        )
        .bind(i64::from(active))
        .bind(id)
        .execute(&mut *tx)
        .await?
        .rows_affected();
        if changed == 0 {
            let known: Option<i64> =
                sqlx::query_scalar("SELECT id FROM account WHERE id = ?").bind(id).fetch_optional(&mut *tx).await?;
            return match known {
                Some(_) => Err(StoreError::Refused(Self::ACCOUNT_LAST_AUTHOR)),
                None => Ok(false),
            };
        }
        if !active {
            sqlx::query("DELETE FROM account_session WHERE account_id = ?").bind(id).execute(&mut *tx).await?;
        }
        tx.commit().await?;
        Ok(true)
    }

    /// Neue Sitzung: nur der Hash des Tokens ([`Store::session_token_hash`]). Merkt die Anmeldung am Konto
    /// (`last_login`) und räumt abgelaufene Sitzungen aller Konten weg.
    pub async fn session_create(&self, token_hash: &str, account: i64) -> Result<(), StoreError> {
        let mut tx = self.pool().begin().await?;
        sqlx::query(
            "DELETE FROM account_session WHERE last_seen <= strftime('%Y-%m-%dT%H:%M:%fZ','now','-30 days') \
             OR created <= strftime('%Y-%m-%dT%H:%M:%fZ','now','-90 days')",
        )
        .execute(&mut *tx)
        .await?;
        sqlx::query("INSERT INTO account_session (token_hash, account_id) VALUES (?, ?)")
            .bind(token_hash)
            .bind(account)
            .execute(&mut *tx)
            .await?;
        sqlx::query("UPDATE account SET last_login = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ?")
            .bind(account)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }

    /// Konto zur Sitzung, wenn sie gilt: aktives Konto, in den letzten 30 Tagen benutzt, höchstens 90 Tage alt.
    /// Schreibt `last_seen` höchstens einmal je Minute. Dazu die Sekunden seit der vorigen Benutzung (der Server
    /// erneuert danach das Cookie).
    pub async fn session_account(&self, token_hash: &str) -> Result<Option<(Account, i64)>, StoreError> {
        let found: Option<(i64, i64)> = sqlx::query_as(
            "SELECT s.account_id, CAST((julianday('now') - julianday(s.last_seen)) * 86400 AS INTEGER) \
             FROM account_session s JOIN account a ON a.id = s.account_id \
             WHERE s.token_hash = ? AND a.active = 1 \
               AND s.last_seen > strftime('%Y-%m-%dT%H:%M:%fZ','now','-30 days') \
               AND s.created > strftime('%Y-%m-%dT%H:%M:%fZ','now','-90 days')",
        )
        .bind(token_hash)
        .fetch_optional(self.pool())
        .await?;
        let Some((account, idle)) = found else { return Ok(None) };
        if idle >= 60 {
            sqlx::query(
                "UPDATE account_session SET last_seen = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE token_hash = ?",
            )
            .bind(token_hash)
            .execute(self.pool())
            .await?;
        }
        Ok(self.account_get(account).await?.map(|account| (account, idle)))
    }

    /// Eine Sitzung löschen (Abmelden). `true`, wenn es sie gab.
    pub async fn session_delete(&self, token_hash: &str) -> Result<bool, StoreError> {
        let gone = sqlx::query("DELETE FROM account_session WHERE token_hash = ?")
            .bind(token_hash)
            .execute(self.pool())
            .await?
            .rows_affected();
        Ok(gone > 0)
    }

    /// Alle Sitzungen eines Kontos löschen, außer der mit dem Token-Hash `keep`. Liefert die Anzahl.
    pub async fn session_delete_all(&self, account: i64, keep: Option<&str>) -> Result<u64, StoreError> {
        Ok(sqlx::query("DELETE FROM account_session WHERE account_id = ? AND token_hash <> ?")
            .bind(account)
            .bind(keep.unwrap_or(""))
            .execute(self.pool())
            .await?
            .rows_affected())
    }

    /// Längste verbleibende Wartezeit (aufgerundete Sekunden) der genannten Bereiche und des Bereichs `alle`;
    /// `None`, wenn keiner gesperrt ist.
    pub async fn login_wait(&self, scopes: &[&str]) -> Result<Option<u64>, StoreError> {
        let mut longest: Option<u64> = None;
        for scope in scopes.iter().copied().chain([ALL_SCOPE]) {
            let left: Option<f64> = sqlx::query_scalar(
                "SELECT (julianday(blocked_until) - julianday('now')) * 86400.0 FROM login_attempt \
                 WHERE scope = ? AND blocked_until IS NOT NULL \
                   AND blocked_until > strftime('%Y-%m-%dT%H:%M:%fZ','now')",
            )
            .bind(scope)
            .fetch_optional(self.pool())
            .await?;
            if let Some(left) = left {
                // Zeitpunkte stehen auf Millisekunden gerundet in der Tabelle: Ohne die kleine Toleranz würden aus
                // frisch gesetzten 2 Sekunden beim Aufrunden 3.
                let seconds = ((left - 0.002).ceil() as u64).max(1);
                longest = Some(longest.map_or(seconds, |known| known.max(seconds)));
            }
        }
        Ok(longest)
    }

    /// Einen Fehlversuch im Bereich zählen und die Wartezeit setzen, die `wait` aus der neuen Zahl der Fehlversuche
    /// berechnet (0 = keine). Vorher fallen Einträge weg, die älter als 24 Stunden sind. Ist die Tabelle voll
    /// ([`Store::LOGIN_ATTEMPT_ROWS`]) und der Bereich neu, entsteht keine Zeile für ihn; stattdessen wartet der
    /// Bereich `alle` [`Store::LOGIN_FULL_WAIT`] Sekunden. Liefert die Wartezeit in Sekunden.
    pub async fn login_failure(&self, scope: &str, wait: fn(i64) -> u64) -> Result<u64, StoreError> {
        let mut tx = self.pool().begin().await?;
        sqlx::query("DELETE FROM login_attempt WHERE last <= strftime('%Y-%m-%dT%H:%M:%fZ','now','-1 day')")
            .execute(&mut *tx)
            .await?;
        let known: Option<i64> = sqlx::query_scalar("SELECT failures FROM login_attempt WHERE scope = ?")
            .bind(scope)
            .fetch_optional(&mut *tx)
            .await?;
        let others: i64 = sqlx::query_scalar("SELECT count(*) FROM login_attempt WHERE scope <> ?")
            .bind(ALL_SCOPE)
            .fetch_one(&mut *tx)
            .await?;
        let (scope, seconds) = if known.is_none() && others >= Self::LOGIN_ATTEMPT_ROWS - 1 {
            (ALL_SCOPE, Self::LOGIN_FULL_WAIT)
        } else {
            (scope, wait(known.unwrap_or(0) + 1))
        };
        sqlx::query(
            "INSERT INTO login_attempt (scope, failures, blocked_until) \
             VALUES (?1, 1, CASE WHEN ?2 > 0 THEN strftime('%Y-%m-%dT%H:%M:%fZ','now', ?3) END) \
             ON CONFLICT(scope) DO UPDATE SET failures = failures + 1, \
             last = strftime('%Y-%m-%dT%H:%M:%fZ','now'), blocked_until = excluded.blocked_until",
        )
        .bind(scope)
        .bind(seconds as i64)
        .bind(format!("+{seconds} seconds"))
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(seconds)
    }

    /// Bereich nach einer erfolgreichen Anmeldung zurücksetzen.
    pub async fn login_reset(&self, scope: &str) -> Result<(), StoreError> {
        sqlx::query("DELETE FROM login_attempt WHERE scope = ?").bind(scope).execute(self.pool()).await?;
        Ok(())
    }

    /// Schalter „Am eigenen Rechner ohne Anmeldung“ (`store_meta` `auth.local_open` = `'1'`); Vorgabe aus.
    pub async fn auth_local_open(&self) -> Result<bool, StoreError> {
        let value: Option<String> = sqlx::query_scalar("SELECT value FROM store_meta WHERE key = ?")
            .bind(LOCAL_OPEN)
            .fetch_optional(self.pool())
            .await?;
        Ok(value.as_deref() == Some("1"))
    }

    /// Schalter setzen; „aus“ löscht den Schlüssel (Vorgabe).
    pub async fn set_auth_local_open(&self, open: bool) -> Result<(), StoreError> {
        if open {
            sqlx::query(
                "INSERT INTO store_meta (key, value) VALUES (?, '1') \
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            )
            .bind(LOCAL_OPEN)
            .execute(self.pool())
            .await?;
        } else {
            sqlx::query("DELETE FROM store_meta WHERE key = ?").bind(LOCAL_OPEN).execute(self.pool()).await?;
        }
        Ok(())
    }
}
