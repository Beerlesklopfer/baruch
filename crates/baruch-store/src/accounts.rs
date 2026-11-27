// SPDX-License-Identifier: AGPL-3.0-or-later
//! Accounts (Round 35, contract §S1.3 and §1; migration `0053_accounts.sql`): who may sign in to Baruch.
//!
//! Instance data: accounts are not in the exchange format and not in the mirror. The store never hashes itself and
//! never sees a plain-text password – the server passes the finished Argon2id PHC text ([`NewAccount::password_phc`]);
//! [`Account`] carries no hash. Without accounts (table empty, [`Store::account_count`] = 0) Baruch stays open as
//! before Round 35.
//!
//! This includes the sessions (`account_session`: only the SHA-256 of the token, never the token), the lock against
//! trying out passwords (`login_attempt`) and the switch „Am eigenen Rechner ohne Anmeldung“ (`store_meta`, key
//! `auth.local_open`). All points in time are computed by SQLite (`strftime('%Y-%m-%dT%H:%M:%fZ','now')`), so that they
//! can be compared as text. Accounts are never deleted: the slug (handle `@lektorat:<slug>`) stays taken.
//!
//! Only the module `auth` of the server calls these methods. Because `lib.rs` hands out only [`Account`],
//! [`NewAccount`] and [`LoginAttempt`] from this module, helpers and messages also hang on [`Store`] as associated
//! functions and constants ([`Store::account_name_key`], [`Store::session_token_hash`], [`Store::ACCOUNT_TAKEN`] …).

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::{Store, StoreError};

/// Columns of [`Account`] (without the hash).
const COLUMNS: &str = "id, name, slug, display, kind, active, created, last_login";
/// Key in `store_meta`: `'1'` = on one's own machine without signing in (default: not set = off).
const LOCAL_OPEN: &str = "auth.local_open";
/// Scope of the lock when `login_attempt` is full.
const ALL_SCOPE: &str = "alle";

/// Outcome of [`Store::login_reserve`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoginAttempt {
    /// A scope is locked: remaining waiting time in seconds. Nothing was counted, nothing is checked.
    Blocked(u64),
    /// The attempt is counted and may be checked. The number is the waiting time in seconds that applies if it fails (0
    /// = none).
    Reserved(u64),
}

/// Body of [`Store::login_wait`] on a connection (also inside a transaction).
async fn wait_in(conn: &mut sqlx::SqliteConnection, scopes: &[&str]) -> Result<Option<u64>, StoreError> {
    let mut longest: Option<u64> = None;
    for scope in scopes.iter().copied().chain([ALL_SCOPE]) {
        let left: Option<f64> = sqlx::query_scalar(
            "SELECT (julianday(blocked_until) - julianday('now')) * 86400.0 FROM login_attempt \
             WHERE scope = ? AND blocked_until IS NOT NULL \
               AND blocked_until > strftime('%Y-%m-%dT%H:%M:%fZ','now')",
        )
        .bind(scope)
        .fetch_optional(&mut *conn)
        .await?;
        if let Some(left) = left {
            // Points in time are in the table rounded to milliseconds: without the small tolerance, freshly set 2
            // seconds would become 3 when rounding up.
            let seconds = ((left - 0.002).ceil() as u64).max(1);
            longest = Some(longest.map_or(seconds, |known| known.max(seconds)));
        }
    }
    Ok(longest)
}

/// Body of [`Store::login_failure`] on a connection; the transaction belongs to the caller.
async fn failure_in(conn: &mut sqlx::SqliteConnection, scope: &str, wait: fn(i64) -> u64) -> Result<u64, StoreError> {
    sqlx::query("DELETE FROM login_attempt WHERE last <= strftime('%Y-%m-%dT%H:%M:%fZ','now','-1 day')")
        .execute(&mut *conn)
        .await?;
    let known: Option<i64> = sqlx::query_scalar("SELECT failures FROM login_attempt WHERE scope = ?")
        .bind(scope)
        .fetch_optional(&mut *conn)
        .await?;
    let others: i64 = sqlx::query_scalar("SELECT count(*) FROM login_attempt WHERE scope <> ?")
        .bind(ALL_SCOPE)
        .fetch_one(&mut *conn)
        .await?;
    let (scope, seconds) = if known.is_none() && others >= Store::LOGIN_ATTEMPT_ROWS - 1 {
        (ALL_SCOPE, Store::LOGIN_FULL_WAIT)
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
    .execute(&mut *conn)
    .await?;
    Ok(seconds)
}

/// An account without the secret (no hash, no plain text). `kind` is `author` or `lektor`; `slug` is the part of the
/// handle after `@lektorat:` (unique, a locked account keeps it as well).
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

/// New account. `password_phc` is the finished Argon2id PHC text (`$argon2id$…`), never a plain-text password –
/// therefore deliberately without `Debug`.
pub struct NewAccount<'a> {
    pub name: &'a str,
    pub slug: &'a str,
    pub display: &'a str,
    pub kind: &'a str,
    pub password_phc: &'a str,
}

/// Violated uniqueness (name or slug) as a domain rejection.
fn unique(e: sqlx::Error) -> StoreError {
    match &e {
        sqlx::Error::Database(db) if db.is_unique_violation() => StoreError::Refused(Store::ACCOUNT_TAKEN),
        _ => StoreError::Db(e),
    }
}

impl Store {
    /// Message: the name or handle (slug) already belongs to an account.
    pub const ACCOUNT_TAKEN: &'static str = "Diesen Namen gibt es schon.";
    /// Message of [`Store::account_create_first`]: there already is an account.
    pub const ACCOUNT_NOT_FIRST: &'static str = "Es gibt schon ein Konto.";
    /// Message of [`Store::account_create`]: no Lektor without an author account.
    pub const ACCOUNT_NEEDS_AUTHOR: &'static str = "Das erste Konto gehört dem Autor (Einrichten).";
    /// Message of [`Store::account_set_active`]: the last active author account stays.
    pub const ACCOUNT_LAST_AUTHOR: &'static str = "Das letzte aktive Autor-Konto lässt sich nicht sperren.";
    /// Maximum number of rows in `login_attempt` (including the scope `alle`).
    pub const LOGIN_ATTEMPT_ROWS: i64 = 1000;
    /// Waiting time in seconds for everyone as long as `login_attempt` is full.
    pub const LOGIN_FULL_WAIT: u64 = 2;

    /// Number of accounts (also locked ones). 0 means: Baruch runs without signing in.
    pub async fn account_count(&self) -> Result<i64, StoreError> {
        Ok(sqlx::query_scalar("SELECT count(*) FROM account").fetch_one(self.pool()).await?)
    }

    /// All accounts by id.
    pub async fn account_list(&self) -> Result<Vec<Account>, StoreError> {
        Ok(sqlx::query_as(sqlx::AssertSqlSafe(format!("SELECT {COLUMNS} FROM account ORDER BY id")))
            .fetch_all(self.pool())
            .await?)
    }

    /// An account; `None` if the id does not exist.
    pub async fn account_get(&self, id: i64) -> Result<Option<Account>, StoreError> {
        Ok(sqlx::query_as(sqlx::AssertSqlSafe(format!("SELECT {COLUMNS} FROM account WHERE id = ?")))
            .bind(id)
            .fetch_optional(self.pool())
            .await?)
    }

    /// Create an account (name and handle unique, otherwise [`Store::ACCOUNT_TAKEN`]). A Lektor account is only created
    /// if there already is an author account ([`Store::ACCOUNT_NEEDS_AUTHOR`]) – otherwise there would be accounts but
    /// nobody with full rights. The first account is created by [`Store::account_create_first`].
    pub async fn account_create(&self, new: &NewAccount<'_>) -> Result<Account, StoreError> {
        let created: Option<Account> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
            "INSERT INTO account (name, name_key, slug, display, kind, password) SELECT ?1, ?2, ?3, ?4, ?5, ?6 \
             WHERE ?5 = 'author' OR EXISTS (SELECT 1 FROM account WHERE kind = 'author') RETURNING {COLUMNS}"
        )))
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

    /// Create the first account: only if the table is empty, otherwise [`Store::ACCOUNT_NOT_FIRST`]. Check and insert
    /// are one statement (one transaction): of two simultaneous calls exactly one wins.
    pub async fn account_create_first(&self, new: &NewAccount<'_>) -> Result<Account, StoreError> {
        let created: Option<Account> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
            "INSERT INTO account (name, name_key, slug, display, kind, password) SELECT ?, ?, ?, ?, ?, ? \
             WHERE NOT EXISTS (SELECT 1 FROM account) RETURNING {COLUMNS}"
        )))
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

    /// Comparison key of an account name: without edges, whitespace collapsed, lower case. Two names with the same key
    /// are the same name ("Erika" = " erika ").
    pub fn account_name_key(name: &str) -> String {
        name.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
    }

    /// SHA-256 (hex, 64 characters) of a session token: only that is in the database.
    pub fn session_token_hash(token: &str) -> String {
        Sha256::digest(token.as_bytes()).iter().map(|byte| format!("{byte:02x}")).collect()
    }

    /// Account for the name together with the Argon2id PHC text – only for the check when signing in (also locked
    /// accounts).
    pub async fn account_for_login(&self, name: &str) -> Result<Option<(Account, String)>, StoreError> {
        let found: Option<(i64, String)> = sqlx::query_as("SELECT id, password FROM account WHERE name_key = ?")
            .bind(Self::account_name_key(name))
            .fetch_optional(self.pool())
            .await?;
        let Some((id, phc)) = found else { return Ok(None) };
        Ok(self.account_get(id).await?.map(|account| (account, phc)))
    }

    /// Argon2id PHC text of an account (check of the previous password); `None` if the id does not exist.
    pub async fn account_password(&self, id: i64) -> Result<Option<String>, StoreError> {
        Ok(sqlx::query_scalar("SELECT password FROM account WHERE id = ?").bind(id).fetch_optional(self.pool()).await?)
    }

    /// Set a new password (finished PHC text) and delete all sessions of the account in the same transaction – except
    /// the one with the token hash `keep` (the session that is changing the password right now). `false`: unknown
    /// account.
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

    /// Lock an account (`active = false`, deletes all its sessions) or unlock it. `Ok(false)`: unknown account. The
    /// last active author account cannot be locked ([`Store::ACCOUNT_LAST_AUTHOR`]) – check and change are one
    /// statement.
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

    /// New session: only the hash of the token ([`Store::session_token_hash`]). Records the sign-in on the account
    /// (`last_login`) and clears away expired sessions of all accounts.
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

    /// Account for the session if it is valid: active account, used in the last 30 days, at most 90 days old. Writes
    /// `last_seen` at most once per minute. In addition the seconds since the previous use (the server renews the
    /// cookie after that).
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

    /// Delete a session (sign out). `true` if it existed.
    pub async fn session_delete(&self, token_hash: &str) -> Result<bool, StoreError> {
        let gone = sqlx::query("DELETE FROM account_session WHERE token_hash = ?")
            .bind(token_hash)
            .execute(self.pool())
            .await?
            .rows_affected();
        Ok(gone > 0)
    }

    /// Delete all sessions of an account, except the one with the token hash `keep`. Returns the number.
    pub async fn session_delete_all(&self, account: i64, keep: Option<&str>) -> Result<u64, StoreError> {
        Ok(sqlx::query("DELETE FROM account_session WHERE account_id = ? AND token_hash <> ?")
            .bind(account)
            .bind(keep.unwrap_or(""))
            .execute(self.pool())
            .await?
            .rows_affected())
    }

    /// Longest remaining waiting time (seconds rounded up) of the named scopes and of the scope `alle`; `None` if none
    /// is locked.
    pub async fn login_wait(&self, scopes: &[&str]) -> Result<Option<u64>, StoreError> {
        let mut conn = self.pool().acquire().await?;
        wait_in(&mut conn, scopes).await
    }

    /// Count a failed attempt in the scope and set the waiting time that `wait` computes from the new number of failed
    /// attempts (0 = none). Before that, entries older than 24 hours are dropped. If the table is full
    /// ([`Store::LOGIN_ATTEMPT_ROWS`]) and the scope is new, no row is created for it; instead the scope `alle` waits
    /// [`Store::LOGIN_FULL_WAIT`] seconds. Returns the waiting time in seconds.
    pub async fn login_failure(&self, scope: &str, wait: fn(i64) -> u64) -> Result<u64, StoreError> {
        // `BEGIN IMMEDIATE`: reading and counting are one step; a deferred transaction that first reads and then writes
        // fails at once under concurrent writers (stale read state).
        let mut tx = self.pool().begin_with("BEGIN IMMEDIATE").await?;
        let seconds = failure_in(&mut tx, scope, wait).await?;
        tx.commit().await?;
        Ok(seconds)
    }

    /// Reserve a sign-in attempt **before** the check of the password – check of the lock and counting in **one**
    /// transaction (`BEGIN IMMEDIATE`): if one of the scopes (or `alle`) is locked, nothing changes
    /// ([`LoginAttempt::Blocked`]); otherwise the attempt counts at once as a failed attempt in every scope
    /// ([`LoginAttempt::Reserved`] with the longest waiting time that follows from it; rules as in
    /// [`Store::login_failure`]). If the password is right, [`Store::login_reset`] takes the counting back. That way,
    /// of any number of simultaneous attempts only as many get to the check as would one after another – whoever counts
    /// only after the check lets a whole volley through before the first failed attempt is entered.
    pub async fn login_reserve(&self, scopes: &[&str], wait: fn(i64) -> u64) -> Result<LoginAttempt, StoreError> {
        let mut tx = self.pool().begin_with("BEGIN IMMEDIATE").await?;
        if let Some(left) = wait_in(&mut tx, scopes).await? {
            return Ok(LoginAttempt::Blocked(left));
        }
        let mut longest = 0;
        for scope in scopes {
            longest = longest.max(failure_in(&mut tx, scope, wait).await?);
        }
        tx.commit().await?;
        Ok(LoginAttempt::Reserved(longest))
    }

    /// Reset a scope after a successful sign-in.
    pub async fn login_reset(&self, scope: &str) -> Result<(), StoreError> {
        sqlx::query("DELETE FROM login_attempt WHERE scope = ?").bind(scope).execute(self.pool()).await?;
        Ok(())
    }

    /// Switch „Am eigenen Rechner ohne Anmeldung“ (`store_meta` `auth.local_open` = `'1'`); default off.
    pub async fn auth_local_open(&self) -> Result<bool, StoreError> {
        let value: Option<String> = sqlx::query_scalar("SELECT value FROM store_meta WHERE key = ?")
            .bind(LOCAL_OPEN)
            .fetch_optional(self.pool())
            .await?;
        Ok(value.as_deref() == Some("1"))
    }

    /// Set the switch; "off" deletes the key (default).
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
