// SPDX-License-Identifier: AGPL-3.0-or-later
//! Tokens für den MCP-Zugang von anderen Rechnern (Migration `0016_mcp_token.sql`, Runde 10, Aufgabe
//! „scripture“).
//!
//! - Ein Token ist `baruch_` + 32 Zufallsbytes (`getrandom`, Base64url ohne Füllzeichen). Der Klartext
//!   verlässt [`Store::mcp_token_create`] genau einmal; gespeichert wird nur sein SHA-256 (hex) und die
//!   ersten 12 Zeichen (`prefix`) zum Wiedererkennen.
//! - [`McpToken`] trägt weder Hash noch Klartext und darf darum überall gezeigt werden.
//! - Name: getrimmt 1–64 Zeichen ohne Steuerzeichen, eindeutig unter den nicht widerrufenen Tokens
//!   (Teilindex `mcp_token_active_name`); ein widerrufener Name ist für [`Store::mcp_token_create`] wieder frei.
//! - Runde 23: [`Store::mcp_token_create_unique`] (RPC `mcp_token.create` und Seite „MCP-Zugang“) prüft zusätzlich
//!   den Handle (`@mcp:<handle>`, die Regeln reicht der Aufrufer herein): Weder der Handle noch der Alt-Handle
//!   (Regel bis Runde 22) des neuen Namens darf dem Handle oder dem Alt-Handle eines vorhandenen **oder
//!   widerrufenen** Tokens gleichen (deren Beiträge tragen ihn weiter), und keiner darf mit der Vorsilbe der Clients
//!   ohne Token (`lokal-`) beginnen. Prüfung und Anlegen in **einer** Transaktion.
//! - Widerrufen setzt `revoked` (idempotent); widerrufene Tokens gelten nie mehr, die Zeile bleibt.
//! - [`Store::mcp_token_verify`] schreibt `last_used` höchstens einmal je Minute.
//!
//! Kein Buchtext, kein Commit im Spiegel; jede Änderung ist eine Transaktion (`BEGIN IMMEDIATE`).

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::{Store, StoreError};

/// Anfang jedes Klartext-Tokens.
pub const MCP_TOKEN_PREFIX: &str = "baruch_";

/// Höchstlänge des Namens (Zeichen, nach dem Trimmen).
const MAX_NAME_CHARS: usize = 64;

/// Länge des gespeicherten Anfangs (`prefix`), mit `baruch_`.
const SHOWN_PREFIX: usize = 12;

/// Ein Token ohne Geheimnis (kein Hash, kein Klartext).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, sqlx::FromRow)]
pub struct McpToken {
    pub id: i64,
    pub name: String,
    /// Die ersten 12 Zeichen des Klartexts (`baruch_` + 5), zum Wiedererkennen.
    pub prefix: String,
    pub created: String,
    pub last_used: Option<String>,
    pub revoked: Option<String>,
}

const COLUMNS: &str = "id, name, prefix, created, last_used, revoked";

/// SHA-256 als 64 Hex-Zeichen.
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

/// Meldung: Handle beginnt mit der Vorsilbe der Clients ohne Token (Runde 23).
const HANDLE_RESERVED: &str = "Namen, deren Autor mit „lokal-“ beginnt, sind Clients ohne Token vorbehalten; anderen \
Namen wählen.";
/// Meldung: Handle (oder Alt-Handle) gehört schon einem gültigen Token (Runde 23).
const HANDLE_TAKEN: &str = "Dieser Name ergibt denselben Autor (@mcp:…) wie ein vorhandener Token – Groß/klein, \
Umlaute, Sonderzeichen und alles nach 32 Zeichen zählen nicht. Anderen Namen wählen.";
/// Meldung: Handle (oder Alt-Handle) gehörte einem widerrufenen Token (Runde 23).
const HANDLE_REVOKED: &str = "Dieser Name ergibt denselben Autor (@mcp:…) wie ein widerrufener Token; dessen \
Beiträge tragen ihn weiter. Anderen Namen wählen.";

/// Token für den geprüften Namen `name` in der Transaktion des Aufrufers anlegen (Name eindeutig unter den gültigen).
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
    /// Neuer Token mit Namen `name`; Antwort: der gespeicherte Eintrag und der Klartext (nur hier). Prüft nur den
    /// Namen (eindeutig unter den gültigen Tokens); Oberfläche und RPC nehmen [`Store::mcp_token_create_unique`].
    pub async fn mcp_token_create(&self, name: &str) -> Result<(McpToken, String), StoreError> {
        let name = check_name(name)?;
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let made = insert(&mut tx, name).await?;
        tx.commit().await?;
        Ok(made)
    }

    /// Wie [`Store::mcp_token_create`], dazu eindeutig im Handle (Runde 23): `handle` bildet einen Namen auf den
    /// Namensteil seines Handles ab (ohne `@mcp:`), `legacy` auf den nach der Regel bis Runde 22. Abgelehnt
    /// (`Refused`, nichts angelegt) wird ein Name,
    /// - dessen Handle oder Alt-Handle mit `reserved_prefix` beginnt (`lokal-`: Clients ohne Token vorbehalten),
    /// - dessen Handle oder Alt-Handle dem Handle oder Alt-Handle eines vorhandenen oder widerrufenen Tokens gleicht
    ///   (Groß/klein, Umlaute, Sonderzeichen und alles nach 32 Zeichen fallen zusammen).
    ///
    /// Der Alt-Handle des **neuen** Namens zählt mit, weil ein Token für alte Beiträge auch seinen Alt-Handle besitzt:
    /// sonst erbte „Laptop Küche“ die Beiträge eines Tokens „laptop-k-che“. Prüfung und Anlegen laufen in einer
    /// Transaktion (`BEGIN IMMEDIATE`); von zwei gleichzeitigen Anfragen mit gleichem Handle gewinnt genau eine.
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

    /// Alle Tokens (auch widerrufene), älteste zuerst.
    pub async fn mcp_token_list(&self) -> Result<Vec<McpToken>, StoreError> {
        Ok(sqlx::query_as(sqlx::AssertSqlSafe(format!("SELECT {COLUMNS} FROM mcp_token ORDER BY id")))
            .fetch_all(&self.pool)
            .await?)
    }

    /// Widerruft den Token `id` (setzt `revoked`; ein zweites Mal ändert nichts). Unbekannt → `Refused`.
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

    /// Der gültige (nicht widerrufene) Token zu einem Klartext, sonst `None`. Setzt `last_used`, wenn es
    /// leer oder älter als eine Minute ist.
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
