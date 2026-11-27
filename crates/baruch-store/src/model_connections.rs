// SPDX-License-Identifier: AGPL-3.0-or-later
//! Model connections and the connection per persona (migration `0036_model_connections.sql`, Round 15b).
//!
//! - `model_connection`: connections to model backends of the kind `eliezer`, `openai` (OpenAI-compatible) or `ollama`,
//!   with name, address, "external" (leaves the machine or the own network), account in the keyring, binding of the key
//!   to its target (`secret_target`, as with `connection`), last quick test and last check (JSON each) and order. The
//!   connection **Eliezer** (id [`ELIEZER_CONNECTION`]) always exists: its address and the binding of its key remain in
//!   the connection `eliezer` (table `connection`, Round 9; `url` is `NULL` here, so that it is not kept twice), its
//!   key in the account `eliezer`. It cannot be deleted (trigger).
//! - `persona_backend`: per persona the connection and the approval for external connections (`external_allowed`,
//!   default 0). No row = connection Eliezer, not approved. If a connection is deleted, its assignments are dropped
//!   (that is, back to Eliezer).
//!
//! Secrets are never stored here. Not book text, no commit in the mirror; every change is one transaction (`BEGIN
//! IMMEDIATE`). Addresses and names are checked more closely by the server; the store keeps the limits of the table.

use serde::Serialize;
use serde_json::Value;
use sqlx::SqliteConnection;

use crate::{Store, StoreError};

/// id of the connection Eliezer (from the migration; can never be deleted).
pub const ELIEZER_CONNECTION: i64 = 1;
/// Longest name of a connection (characters).
pub const MAX_CONNECTION_NAME: usize = 80;
/// Longest address (characters).
pub const MAX_CONNECTION_URL: usize = 2000;
/// Key in `store_meta`: the id of a model connection assigned last (ids are never reused).
const LAST_ID: &str = "model_connection_last";
/// Kinds of a model connection (column `kind`).
pub const MODEL_CONNECTION_KINDS: [&str; 3] = ["eliezer", "openai", "ollama"];

/// A model connection.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ModelConnection {
    pub id: i64,
    pub name: String,
    /// `eliezer`, `openai` or `ollama`.
    pub kind: String,
    /// Base address; `None` only for the connection Eliezer (address from the connection `eliezer`).
    pub url: Option<String>,
    /// Leaves the machine or the own network: book text only with an approval per persona.
    pub external: bool,
    /// Account in the keyring (service "baruch"): `eliezer` or `model-<id>`.
    pub account: String,
    /// Target for which the key was stored, if the address has pointed elsewhere since; `None` = applies to the present
    /// target. Always `None` for Eliezer (the binding is in `connection`).
    pub secret_target: Option<String>,
    /// Last quick test (JSON, as `model.probe` returns it).
    pub probe: Option<Value>,
    /// Last check (JSON, as `model.connection_check` returns it).
    pub check: Option<Value>,
    pub position: i64,
    pub updated: String,
}

/// New or changed connection (name and address trimmed).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelConnectionEdit {
    pub name: String,
    pub kind: String,
    /// Ignored for the connection Eliezer (address through the connection `eliezer`).
    pub url: String,
    pub external: bool,
}

/// Connection of a persona.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct PersonaBackend {
    pub connection: i64,
    pub external_allowed: bool,
    /// `false`: no row, default (Eliezer, not approved).
    pub explicit: bool,
}

impl Default for PersonaBackend {
    fn default() -> Self {
        PersonaBackend { connection: ELIEZER_CONNECTION, external_allowed: false, explicit: false }
    }
}

type Tuple =
    (i64, String, String, Option<String>, i64, String, Option<String>, Option<String>, Option<String>, i64, String);

const COLUMNS: &str = "id, name, kind, url, external, account, secret_target, probe, checked, position, updated";

fn json_of(text: Option<String>, what: &str) -> Result<Option<Value>, StoreError> {
    text.map(|t| serde_json::from_str(&t).map_err(|e| crate::broken(format!("model_connection.{what}: {e}"))))
        .transpose()
}

fn of(row: Tuple) -> Result<ModelConnection, StoreError> {
    let (id, name, kind, url, external, account, secret_target, probe, check, position, updated) = row;
    Ok(ModelConnection {
        id,
        name,
        kind,
        url,
        external: external != 0,
        account,
        secret_target,
        probe: json_of(probe, "probe")?,
        check: json_of(check, "checked")?,
        position,
        updated,
    })
}

async fn one(conn: &mut SqliteConnection, id: i64) -> Result<ModelConnection, StoreError> {
    let row: Option<Tuple> =
        sqlx::query_as(sqlx::AssertSqlSafe(format!("SELECT {COLUMNS} FROM model_connection WHERE id = ?")))
            .bind(id)
            .fetch_optional(&mut *conn)
            .await?;
    of(row.ok_or(StoreError::Refused("Diese Modell-Verbindung gibt es nicht (mehr)."))?)
}

/// Check name and kind; the address only for connections other than Eliezer.
fn check(edit: &ModelConnectionEdit, eliezer: bool) -> Result<(String, String, Option<String>), StoreError> {
    let name = edit.name.trim();
    if name.is_empty() || name.chars().count() > MAX_CONNECTION_NAME || name.chars().any(char::is_control) {
        return Err(StoreError::Refused("Name einer Modell-Verbindung: 1 bis 80 Zeichen in einer Zeile."));
    }
    let kind = edit.kind.trim().to_lowercase();
    if !MODEL_CONNECTION_KINDS.contains(&kind.as_str()) {
        return Err(StoreError::Refused("Art einer Modell-Verbindung: eliezer, openai oder ollama."));
    }
    if eliezer {
        return Ok((name.to_owned(), kind, None));
    }
    let url = edit.url.trim();
    let lower = url.to_ascii_lowercase();
    if !(lower.starts_with("http://") || lower.starts_with("https://"))
        || url.chars().count() > MAX_CONNECTION_URL
        || url.chars().any(char::is_whitespace)
    {
        return Err(StoreError::Refused("Adresse einer Modell-Verbindung: http:// oder https://, ohne Leerraum."));
    }
    Ok((name.to_owned(), kind, Some(url.to_owned())))
}

/// id of the persona (handle with or without `@`).
async fn persona_id(conn: &mut SqliteConnection, handle: &str) -> Result<i64, StoreError> {
    let wanted = handle.trim();
    let bare = wanted.trim_start_matches('@');
    for candidate in [wanted.to_owned(), format!("@{bare}")] {
        if candidate.is_empty() || candidate == "@" {
            continue;
        }
        let id: Option<i64> = sqlx::query_scalar("SELECT id FROM persona WHERE handle = ?")
            .bind(&candidate)
            .fetch_optional(&mut *conn)
            .await?;
        if let Some(id) = id {
            return Ok(id);
        }
    }
    Err(StoreError::UnknownPersona(wanted.to_owned()))
}

fn unique_violation(e: &sqlx::Error) -> bool {
    e.as_database_error().is_some_and(|d| d.message().contains("UNIQUE"))
}

impl Store {
    /// All model connections in their order (Eliezer first, as long as it is not moved).
    pub async fn model_connection_list(&self) -> Result<Vec<ModelConnection>, StoreError> {
        let mut conn = self.pool.acquire().await?;
        let rows: Vec<Tuple> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
            "SELECT {COLUMNS} FROM model_connection ORDER BY position, id"
        )))
        .fetch_all(&mut *conn)
        .await?;
        rows.into_iter().map(of).collect()
    }

    /// A model connection; unknown → `Refused`.
    pub async fn model_connection_get(&self, id: i64) -> Result<ModelConnection, StoreError> {
        let mut conn = self.pool.acquire().await?;
        one(&mut conn, id).await
    }

    /// New connection (account `model-<id>`, at the end of the list). Duplicate name → `Refused`.
    pub async fn model_connection_create(&self, edit: &ModelConnectionEdit) -> Result<ModelConnection, StoreError> {
        let (name, kind, url) = check(edit, false)?;
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let position: i64 = sqlx::query_scalar("SELECT COALESCE(MAX(position), 0) + 1 FROM model_connection")
            .fetch_one(&mut *tx)
            .await?;
        // ids never twice (otherwise a new connection would inherit the account `model-<id>` of a deleted one): larger
        // than every existing one and than the one assigned last (`store_meta`).
        let last: Option<String> = sqlx::query_scalar("SELECT value FROM store_meta WHERE key = ?")
            .bind(LAST_ID)
            .fetch_optional(&mut *tx)
            .await?;
        let max: i64 =
            sqlx::query_scalar("SELECT COALESCE(MAX(id), 0) FROM model_connection").fetch_one(&mut *tx).await?;
        let id = max.max(last.and_then(|l| l.parse().ok()).unwrap_or(0)) + 1;
        let inserted = sqlx::query(
            "INSERT INTO model_connection (id, name, kind, url, external, account, position) VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(id)
        .bind(&name)
        .bind(&kind)
        .bind(&url)
        .bind(i64::from(edit.external))
        .bind(format!("model-{id}"))
        .bind(position)
        .execute(&mut *tx)
        .await;
        match inserted {
            Ok(_) => {}
            Err(e) if unique_violation(&e) => {
                return Err(StoreError::Refused("Eine Modell-Verbindung mit diesem Namen gibt es schon."));
            }
            Err(e) => return Err(e.into()),
        }
        sqlx::query(
            "INSERT INTO store_meta (key, value) VALUES (?, ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        )
        .bind(LAST_ID)
        .bind(id.to_string())
        .execute(&mut *tx)
        .await?;
        let row = one(&mut tx, id).await?;
        tx.commit().await?;
        Ok(row)
    }

    /// Change a connection. If kind or address changes, the last quick test and the last check no longer apply
    /// (cleared). The server sets the binding of the key beforehand with [`Store::model_connection_bind_secret`].
    pub async fn model_connection_update(
        &self,
        id: i64,
        edit: &ModelConnectionEdit,
    ) -> Result<ModelConnection, StoreError> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let before = one(&mut tx, id).await?;
        let (name, kind, url) = check(edit, id == ELIEZER_CONNECTION)?;
        if id == ELIEZER_CONNECTION && kind != "eliezer" {
            return Err(StoreError::Refused("Die Verbindung Eliezer bleibt von der Art eliezer."));
        }
        let moved = before.kind != kind || before.url != url;
        let result = sqlx::query(
            "UPDATE model_connection SET name = ?, kind = ?, url = ?, external = ?, \
             probe = CASE WHEN ? THEN NULL ELSE probe END, checked = CASE WHEN ? THEN NULL ELSE checked END, \
             updated = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ?",
        )
        .bind(&name)
        .bind(&kind)
        .bind(&url)
        .bind(i64::from(edit.external))
        .bind(moved)
        .bind(moved)
        .bind(id)
        .execute(&mut *tx)
        .await;
        match result {
            Ok(_) => {}
            Err(e) if unique_violation(&e) => {
                return Err(StoreError::Refused("Eine Modell-Verbindung mit diesem Namen gibt es schon."));
            }
            Err(e) => return Err(e.into()),
        }
        let row = one(&mut tx, id).await?;
        tx.commit().await?;
        Ok(row)
    }

    /// Delete a connection (not Eliezer); personas that used it fall back to Eliezer (not approved). Returns the
    /// handles of these personas.
    pub async fn model_connection_delete(&self, id: i64) -> Result<Vec<String>, StoreError> {
        if id == ELIEZER_CONNECTION {
            return Err(StoreError::Refused("Die Verbindung Eliezer lässt sich nicht löschen."));
        }
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        one(&mut tx, id).await?;
        let handles: Vec<String> = sqlx::query_scalar(
            "SELECT p.handle FROM persona_backend b JOIN persona p ON p.id = b.persona_id WHERE b.connection_id = ? \
             ORDER BY p.position, p.id",
        )
        .bind(id)
        .fetch_all(&mut *tx)
        .await?;
        sqlx::query("DELETE FROM persona_backend WHERE connection_id = ?").bind(id).execute(&mut *tx).await?;
        sqlx::query("DELETE FROM model_connection WHERE id = ?").bind(id).execute(&mut *tx).await?;
        // Order without gaps.
        let ids: Vec<i64> =
            sqlx::query_scalar("SELECT id FROM model_connection ORDER BY position, id").fetch_all(&mut *tx).await?;
        for (index, other) in ids.iter().enumerate() {
            sqlx::query("UPDATE model_connection SET position = ? WHERE id = ?")
                .bind(index as i64 + 1)
                .bind(other)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(handles)
    }

    /// Target to which the key is bound (`None` = applies to the present one). Nothing for Eliezer (the binding is in
    /// `connection`).
    pub async fn model_connection_bind_secret(&self, id: i64, target: Option<&str>) -> Result<(), StoreError> {
        if id == ELIEZER_CONNECTION {
            return Ok(());
        }
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        one(&mut tx, id).await?;
        sqlx::query("UPDATE model_connection SET secret_target = ? WHERE id = ?")
            .bind(target)
            .bind(id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }

    /// Store the last quick test (survives a restart).
    pub async fn model_connection_record_probe(&self, id: i64, probe: &Value) -> Result<(), StoreError> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        one(&mut tx, id).await?;
        sqlx::query("UPDATE model_connection SET probe = ? WHERE id = ?")
            .bind(probe.to_string())
            .bind(id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }

    /// Store the last check.
    pub async fn model_connection_record_check(&self, id: i64, check: &Value) -> Result<(), StoreError> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        one(&mut tx, id).await?;
        sqlx::query("UPDATE model_connection SET checked = ? WHERE id = ?")
            .bind(check.to_string())
            .bind(id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }

    /// Connection and approval of a persona; no row = default. Unknown persona → `UnknownPersona`.
    pub async fn persona_backend(&self, handle: &str) -> Result<PersonaBackend, StoreError> {
        let mut conn = self.pool.acquire().await?;
        let id = persona_id(&mut conn, handle).await?;
        let row: Option<(i64, i64)> =
            sqlx::query_as("SELECT connection_id, external_allowed FROM persona_backend WHERE persona_id = ?")
                .bind(id)
                .fetch_optional(&mut *conn)
                .await?;
        Ok(row.map_or_else(PersonaBackend::default, |(connection, allowed)| PersonaBackend {
            connection,
            external_allowed: allowed != 0,
            explicit: true,
        }))
    }

    /// Connections of all personas (handle → assignment), personas without a row with the default.
    pub async fn persona_backends(&self) -> Result<Vec<(String, PersonaBackend)>, StoreError> {
        let mut conn = self.pool.acquire().await?;
        let rows: Vec<(String, Option<i64>, Option<i64>)> = sqlx::query_as(
            "SELECT p.handle, b.connection_id, b.external_allowed FROM persona p \
             LEFT JOIN persona_backend b ON b.persona_id = p.id ORDER BY p.position, p.id",
        )
        .fetch_all(&mut *conn)
        .await?;
        Ok(rows
            .into_iter()
            .map(|(handle, connection, allowed)| {
                let backend = match connection {
                    Some(connection) => {
                        PersonaBackend { connection, external_allowed: allowed == Some(1), explicit: true }
                    }
                    None => PersonaBackend::default(),
                };
                (handle, backend)
            })
            .collect())
    }

    /// Set connection and approval of a persona; unknown connection → `Refused`.
    pub async fn set_persona_backend(
        &self,
        handle: &str,
        connection: i64,
        external_allowed: bool,
    ) -> Result<PersonaBackend, StoreError> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let id = persona_id(&mut tx, handle).await?;
        one(&mut tx, connection).await?;
        sqlx::query(
            "INSERT INTO persona_backend (persona_id, connection_id, external_allowed) VALUES (?, ?, ?) \
             ON CONFLICT(persona_id) DO UPDATE SET connection_id = excluded.connection_id, \
             external_allowed = excluded.external_allowed, updated = strftime('%Y-%m-%dT%H:%M:%fZ','now')",
        )
        .bind(id)
        .bind(connection)
        .bind(i64::from(external_allowed))
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(PersonaBackend { connection, external_allowed, explicit: true })
    }
}
