// SPDX-License-Identifier: AGPL-3.0-or-later
//! Tokens for MCP access from other machines (migration `0016_mcp_token.sql`, Round 10, task "scripture").
//!
//! - A token is `baruch_` + 32 random bytes (`getrandom`, Base64url without padding). The plain text leaves
//!   [`Store::mcp_token_create`] exactly once; only its SHA-256 (hex) and the first 12 characters (`prefix`) for
//!   recognition are stored.
//! - [`McpToken`] carries neither hash nor plain text and may therefore be shown everywhere.
//! - Name: trimmed 1–64 characters without control characters, unique among the tokens that are not revoked (partial
//!   index `mcp_token_active_name`); a revoked name is free again for [`Store::mcp_token_create`].
//! - Round 23: [`Store::mcp_token_create_unique`] (RPC `mcp_token.create` and page „MCP-Zugang“) additionally checks
//!   the handle (`@mcp:<handle>`, the caller passes in the rules): neither the handle nor the legacy handle (rule up to
//!   Round 22) of the new name may equal the handle or the legacy handle of an existing **or revoked** token (their
//!   posts keep carrying it), and none may begin with the prefix of the clients without a token (`lokal-`). Check and
//!   creation in **one** transaction.
//! - Revoking sets `revoked` (idempotent); revoked tokens never apply again, the row stays.
//! - [`Store::mcp_token_verify`] writes `last_used` at most once per minute.
//!
//! Not book text, no commit in the mirror; every change is one transaction (`BEGIN IMMEDIATE`).

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::{Store, StoreError};

/// Beginning of every plain-text token.
pub const MCP_TOKEN_PREFIX: &str = "baruch_";

/// Maximum length of the name (characters, after trimming).
const MAX_NAME_CHARS: usize = 64;

/// Length of the stored beginning (`prefix`), including `baruch_`.
const SHOWN_PREFIX: usize = 12;

/// A token without the secret (no hash, no plain text).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, sqlx::FromRow)]
pub struct McpToken {
    pub id: i64,
    pub name: String,
    /// The first 12 characters of the plain text (`baruch_` + 5), for recognition.
    pub prefix: String,
    pub created: String,
    pub last_used: Option<String>,
    pub revoked: Option<String>,
}

const COLUMNS: &str = "id, name, prefix, created, last_used, revoked";

/// SHA-256 as 64 hex characters.
fn hash(plain: &str) -> String {
    Sha256::digest(plain.as_bytes()).iter().map(|b| format!("{b:02x}")).collect()
}

fn check_name(name: &str) -> Result<&str, StoreError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(StoreError::Refused("Ein Token braucht einen Namen (z. B. den des Rechners)."));
    }
    if name.chars().count() > MAX_NAME_CHARS {
        return Err(StoreError::Refused("Der Name eines Tokens hat höchstens 64 Zeichen."));
    }
    if name.chars().any(char::is_control) {
        return Err(StoreError::Refused("Der Name eines Tokens hat keine Zeilenumbrüche oder Steuerzeichen."));
    }
    Ok(name)
}

/// Message: the handle begins with the prefix of the clients without a token (Round 23).
const HANDLE_RESERVED: &str = "Namen, deren Autor mit „lokal-“ beginnt, sind Clients ohne Token vorbehalten; anderen \
Namen wählen.";
/// Message: the handle (or legacy handle) already belongs to a valid token (Round 23).
const HANDLE_TAKEN: &str = "Dieser Name ergibt denselben Autor (@mcp:…) wie ein vorhandener Token – Groß/klein, \
Umlaute, Sonderzeichen und alles nach 32 Zeichen zählen nicht. Anderen Namen wählen.";
/// Message: the handle (or legacy handle) belonged to a revoked token (Round 23).
const HANDLE_REVOKED: &str = "Dieser Name ergibt denselben Autor (@mcp:…) wie ein widerrufener Token; dessen \
Beiträge tragen ihn weiter. Anderen Namen wählen.";

/// Create a token for the checked name `name` in the transaction of the caller (name unique among the valid ones).
async fn insert(tx: &mut sqlx::SqliteConnection, name: &str) -> Result<(McpToken, String), StoreError> {
    let mut bytes = [0u8; 32];
    getrandom::getrandom(&mut bytes)
        .map_err(|_| StoreError::Refused("Kein Zufall vom Betriebssystem; Token nicht angelegt."))?;
    let plain = format!("{MCP_TOKEN_PREFIX}{}", URL_SAFE_NO_PAD.encode(bytes));
    let prefix: String = plain.chars().take(SHOWN_PREFIX).collect();
    let taken: i64 = sqlx::query_scalar("SELECT count(*) FROM mcp_token WHERE name = ? AND revoked IS NULL")
        .bind(name)
        .fetch_one(&mut *tx)
        .await?;
    if taken > 0 {
        return Err(StoreError::Refused(
            "Es gibt schon einen gültigen Token mit diesem Namen; anderen Namen wählen oder den alten widerrufen.",
        ));
    }
    let token: McpToken = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "INSERT INTO mcp_token (name, hash, prefix) VALUES (?, ?, ?) RETURNING {COLUMNS}"
    )))
    .bind(name)
    .bind(hash(&plain))
    .bind(&prefix)
    .fetch_one(&mut *tx)
    .await?;
    Ok((token, plain))
}

impl Store {
    /// New token with the name `name`; answer: the stored entry and the plain text (only here). Checks only the name
    /// (unique among the valid tokens); user interface and RPC use [`Store::mcp_token_create_unique`].
    pub async fn mcp_token_create(&self, name: &str) -> Result<(McpToken, String), StoreError> {
        let name = check_name(name)?;
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let made = insert(&mut tx, name).await?;
        tx.commit().await?;
        Ok(made)
    }

    /// Like [`Store::mcp_token_create`], in addition unique in the handle (Round 23): `handle` maps a name to the name
    /// part of its handle (without `@mcp:`), `legacy` to the one after the rule up to Round 22. Rejected (`Refused`,
    /// nothing created) is a name
    /// - whose handle or legacy handle begins with `reserved_prefix` (`lokal-`: reserved for clients without a token),
    /// - whose handle or legacy handle equals the handle or legacy handle of an existing or revoked token (upper/lower
    ///   case, umlauts, special characters and everything after 32 characters coincide).
    ///
    /// The legacy handle of the **new** name counts as well, because a token also owns its legacy handle for old posts:
    /// otherwise „Laptop Küche“ would inherit the posts of a token „laptop-k-che“. Check and creation run in one
    /// transaction (`BEGIN IMMEDIATE`); of two simultaneous requests with the same handle exactly one wins.
    pub async fn mcp_token_create_unique(
        &self,
        name: &str,
        handle: impl Fn(&str) -> String,
        legacy: impl Fn(&str) -> String,
        reserved_prefix: &str,
    ) -> Result<(McpToken, String), StoreError> {
        let name = check_name(name)?;
        let wanted = [handle(name), legacy(name)];
        if !reserved_prefix.is_empty() && wanted.iter().any(|w| w.starts_with(reserved_prefix)) {
            return Err(StoreError::Refused(HANDLE_RESERVED));
        }
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let known: Vec<(String, Option<String>)> =
            sqlx::query_as("SELECT name, revoked FROM mcp_token ORDER BY id").fetch_all(&mut *tx).await?;
        for (other, revoked) in &known {
            if [handle(other), legacy(other)].iter().any(|h| wanted.contains(h)) {
                return Err(StoreError::Refused(if revoked.is_some() { HANDLE_REVOKED } else { HANDLE_TAKEN }));
            }
        }
        let made = insert(&mut tx, name).await?;
        tx.commit().await?;
        Ok(made)
    }

    /// All tokens (also revoked ones), oldest first.
    pub async fn mcp_token_list(&self) -> Result<Vec<McpToken>, StoreError> {
        Ok(sqlx::query_as(sqlx::AssertSqlSafe(format!("SELECT {COLUMNS} FROM mcp_token ORDER BY id")))
            .fetch_all(&self.pool)
            .await?)
    }

    /// Revokes the token `id` (sets `revoked`; a second time changes nothing). Unknown → `Refused`.
    pub async fn mcp_token_revoke(&self, id: i64) -> Result<McpToken, StoreError> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        sqlx::query(
            "UPDATE mcp_token SET revoked = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ? AND revoked IS NULL",
        )
        .bind(id)
        .execute(&mut *tx)
        .await?;
        let token: Option<McpToken> =
            sqlx::query_as(sqlx::AssertSqlSafe(format!("SELECT {COLUMNS} FROM mcp_token WHERE id = ?")))
                .bind(id)
                .fetch_optional(&mut *tx)
                .await?;
        let token = token.ok_or(StoreError::Refused("Diesen Token gibt es nicht."))?;
        tx.commit().await?;
        Ok(token)
    }

    /// The valid (not revoked) token for a plain text, otherwise `None`. Sets `last_used` if it is empty or older than
    /// one minute.
    pub async fn mcp_token_verify(&self, plain: &str) -> Result<Option<McpToken>, StoreError> {
        if !plain.starts_with(MCP_TOKEN_PREFIX) || plain.len() > 200 {
            return Ok(None);
        }
        let digest = hash(plain);
        let found: Option<McpToken> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
            "SELECT {COLUMNS} FROM mcp_token WHERE hash = ? AND revoked IS NULL"
        )))
        .bind(&digest)
        .fetch_optional(&self.pool)
        .await?;
        let Some(token) = found else { return Ok(None) };
        let stale = sqlx::query(
            "UPDATE mcp_token SET last_used = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ? AND revoked IS NULL \
             AND (last_used IS NULL OR last_used < strftime('%Y-%m-%dT%H:%M:%fZ','now','-60 seconds'))",
        )
        .bind(token.id)
        .execute(&self.pool)
        .await?;
        if stale.rows_affected() == 0 {
            return Ok(Some(token));
        }
        let fresh: Option<McpToken> =
            sqlx::query_as(sqlx::AssertSqlSafe(format!("SELECT {COLUMNS} FROM mcp_token WHERE id = ?")))
                .bind(token.id)
                .fetch_optional(&self.pool)
                .await?;
        Ok(fresh.or(Some(token)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_trimmed_and_bounded() {
        assert_eq!(check_name("  Laptop  ").unwrap(), "Laptop", "getrimmt");
        assert!(check_name("   ").is_err(), "leer");
        assert!(check_name(&"x".repeat(64)).is_ok(), "64 Zeichen gehen");
        assert!(check_name(&"ä".repeat(65)).is_err(), "65 Zeichen nicht (Zeichen, nicht Bytes)");
        assert!(check_name("a\nb").is_err(), "kein Zeilenumbruch");
    }

    #[test]
    fn the_hash_is_64_hex_characters() {
        let h = hash("baruch_probe");
        assert_eq!(h.len(), 64, "SHA-256 hex");
        assert!(h.bytes().all(|b| b.is_ascii_hexdigit()), "nur Hex");
    }
}
