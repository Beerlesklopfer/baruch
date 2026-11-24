// SPDX-License-Identifier: AGPL-3.0-or-later
//! Modell-Verbindungen und Verbindung je Persona (Migration `0036_model_connections.sql`, Runde 15b).
//!
//! - `model_connection`: Verbindungen zu Modell-Backends der Art `eliezer`, `openai` (OpenAI-kompatibel)
//!   oder `ollama`, mit Name, Adresse, „extern“ (verlässt den Rechner bzw. das eigene Netz), Konto im
//!   Schlüsselbund, Bindung des Schlüssels an sein Ziel (`secret_target`, wie bei `connection`), letztem
//!   Kurztest und letzter Prüfung (je JSON) und Reihenfolge. Die Verbindung **Eliezer** (id
//!   [`ELIEZER_CONNECTION`]) gibt es immer: Ihre Adresse und die Bindung ihres Schlüssels stehen weiter in
//!   der Verbindung `eliezer` (Tabelle `connection`, Runde 9; `url` ist hier `NULL`, damit sie nicht doppelt
//!   geführt wird), ihr Schlüssel im Konto `eliezer`. Sie lässt sich nicht löschen (Trigger).
//! - `persona_backend`: je Persona die Verbindung und die Freigabe für externe Verbindungen
//!   (`external_allowed`, Vorgabe 0). Keine Zeile = Verbindung Eliezer, nicht freigegeben. Wird eine
//!   Verbindung gelöscht, fallen ihre Zuordnungen weg (also zurück auf Eliezer).
//!
//! Geheimnisse stehen nie hier. Kein Buchtext, kein Commit im Spiegel; jede Änderung eine Transaktion
//! (`BEGIN IMMEDIATE`). Adressen und Namen prüft der Server genauer; der Store hält die Grenzen der Tabelle.

use serde::Serialize;
use serde_json::Value;
use sqlx::SqliteConnection;

use crate::{Store, StoreError};

/// id der Verbindung Eliezer (aus der Migration; nie löschbar).
pub const ELIEZER_CONNECTION: i64 = 1;
/// Längster Name einer Verbindung (Zeichen).
pub const MAX_CONNECTION_NAME: usize = 80;
/// Längste Adresse (Zeichen).
pub const MAX_CONNECTION_URL: usize = 2000;
/// Schlüssel in `store_meta`: zuletzt vergebene id einer Modell-Verbindung (ids werden nie wiederverwendet).
const LAST_ID: &str = "model_connection_last";
/// Arten einer Modell-Verbindung (Spalte `kind`).
pub const MODEL_CONNECTION_KINDS: [&str; 3] = ["eliezer", "openai", "ollama"];

/// Eine Modell-Verbindung.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ModelConnection {
    pub id: i64,
    pub name: String,
    /// `eliezer`, `openai` oder `ollama`.
    pub kind: String,
    /// Grundadresse; `None` nur bei der Verbindung Eliezer (Adresse aus der Verbindung `eliezer`).
    pub url: Option<String>,
    /// Verlässt den Rechner bzw. das eigene Netz: Buchtext nur mit Freigabe je Persona.
    pub external: bool,
    /// Konto im Schlüsselbund (Dienst „baruch“): `eliezer` bzw. `model-<id>`.
    pub account: String,
    /// Ziel, für das der Schlüssel hinterlegt wurde, wenn die Adresse seitdem anderswohin zeigt; `None` =
    /// gilt für das jetzige Ziel. Bei Eliezer immer `None` (die Bindung steht in `connection`).
    pub secret_target: Option<String>,
    /// Letzter Kurztest (JSON, wie `model.probe` ihn liefert).
    pub probe: Option<Value>,
    /// Letzte Prüfung (JSON, wie `model.connection_check` sie liefert).
    pub check: Option<Value>,
    pub position: i64,
    pub updated: String,
}

/// Neue bzw. geänderte Verbindung (Name und Adresse getrimmt).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelConnectionEdit {
    pub name: String,
    pub kind: String,
    /// Bei der Verbindung Eliezer ignoriert (Adresse über die Verbindung `eliezer`).
    pub url: String,
    pub external: bool,
}

/// Verbindung einer Persona.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct PersonaBackend {
    pub connection: i64,
    pub external_allowed: bool,
    /// `false`: keine Zeile, Vorgabe (Eliezer, nicht freigegeben).
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
    let row: Option<Tuple> = sqlx::query_as(&format!("SELECT {COLUMNS} FROM model_connection WHERE id = ?"))
        .bind(id)
        .fetch_optional(&mut *conn)
        .await?;
    of(row.ok_or(StoreError::Refused("Diese Modell-Verbindung gibt es nicht (mehr)."))?)
}

/// Name und Art prüfen; Adresse nur für andere Verbindungen als Eliezer.
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

/// id der Persona (Handle mit oder ohne `@`).
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
    /// Alle Modell-Verbindungen in ihrer Reihenfolge (Eliezer zuerst, solange nicht verschoben).
    pub async fn model_connection_list(&self) -> Result<Vec<ModelConnection>, StoreError> {
        let mut conn = self.pool.acquire().await?;
        let rows: Vec<Tuple> = sqlx::query_as(&format!("SELECT {COLUMNS} FROM model_connection ORDER BY position, id"))
            .fetch_all(&mut *conn)
            .await?;
        rows.into_iter().map(of).collect()
    }

    /// Eine Modell-Verbindung; unbekannt → `Refused`.
    pub async fn model_connection_get(&self, id: i64) -> Result<ModelConnection, StoreError> {
        let mut conn = self.pool.acquire().await?;
        one(&mut conn, id).await
    }

    /// Neue Verbindung (Konto `model-<id>`, ans Ende der Liste). Doppelter Name → `Refused`.
    pub async fn model_connection_create(&self, edit: &ModelConnectionEdit) -> Result<ModelConnection, StoreError> {
        let (name, kind, url) = check(edit, false)?;
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let position: i64 = sqlx::query_scalar("SELECT COALESCE(MAX(position), 0) + 1 FROM model_connection")
            .fetch_one(&mut *tx)
            .await?;
        // ids nie doppelt (sonst erbte eine neue Verbindung das Konto `model-<id>` einer gelöschten): größer als
        // jede vorhandene und als die zuletzt vergebene (`store_meta`).
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

    /// Verbindung ändern. Ändert sich Art oder Adresse, gelten letzter Kurztest und letzte Prüfung nicht mehr
    /// (geleert). Die Bindung des Schlüssels setzt der Server vorher mit [`Store::model_connection_bind_secret`].
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

    /// Verbindung löschen (nicht Eliezer); Personas, die sie nutzten, fallen auf Eliezer zurück (nicht
    /// freigegeben). Liefert die Handles dieser Personas.
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
        // Lückenlose Reihenfolge.
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

    /// Ziel, an das der Schlüssel gebunden ist (`None` = gilt für das jetzige). Bei Eliezer nichts (die
    /// Bindung steht in `connection`).
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

    /// Letzten Kurztest speichern (überlebt einen Neustart).
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

    /// Letzte Prüfung speichern.
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

    /// Verbindung und Freigabe einer Persona; keine Zeile = Vorgabe. Unbekannte Persona → `UnknownPersona`.
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

    /// Verbindungen aller Personas (Handle → Zuordnung), Personas ohne Zeile mit der Vorgabe.
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

    /// Verbindung und Freigabe einer Persona setzen; unbekannte Verbindung → `Refused`.
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
