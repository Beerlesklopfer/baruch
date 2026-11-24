// SPDX-License-Identifier: AGPL-3.0-or-later
//! Verbindungen, RAG-Wege je Werk und bevorzugte Quellen (Migration `0013_connections.sql`, Runde 9,
//! Aufgabe „connections“).
//!
//! - `connection`: je Art ([`ConnectionKind`]) die Einstellungen als JSON-Objekt und das Ergebnis der
//!   letzten Prüfung. Eine fehlende Zeile ist die Vorgabe ([`ConnectionKind::default_config`]).
//!   **Geheimnisse stehen nie hier**, sie liegen im Schlüsselbund (Server, `connections::secrets`);
//!   welche Schlüssel eine Einstellung haben darf, prüft der Server.
//! - `work_rag`: je Werk die RAG-Wege ([`RagWay`]); fehlt die Zeile, gilt `["lancedb"]`, eine leere
//!   Liste heißt „nur Volltext“. Ein gelöschtes Werk nimmt seine Zeile mit (`ON DELETE CASCADE`).
//! - `preferred_source`: Lesezeichen (Titel, Adresse, Notiz) in einer lückenlosen Reihenfolge
//!   `1..n`, die der Store bei Anlegen, Verschieben und Löschen in derselben Transaktion pflegt.
//!
//! Alles ist global (nicht je Werk außer `work_rag`), kein Buchtext und darum kein Commit im
//! Git-Spiegel. Jede Änderung ist eine Transaktion (`BEGIN IMMEDIATE`). Eingaben prüft der Server
//! genauer (Adressen, Felder); der Store hält nur die Grenzen der Tabellen und lehnt mit
//! [`StoreError::Refused`] ab.

use serde::Serialize;
use serde_json::{Value, json};
use sqlx::SqliteConnection;

use crate::{Store, StoreError, WorkId};

/// Art einer Verbindung (Spalte `connection.kind`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ConnectionKind {
    /// Modell-Backend Eliezer (OpenAI-artige Schnittstelle, `/v1/models`).
    Eliezer,
    /// MultiGPT als MCP-Server.
    Multigpt,
    /// PostgreSQL mit der Erweiterung `vector`.
    Pgvector,
    /// LanceDB im Datenordner (lokal, ohne Geheimnis).
    Lancedb,
}

impl ConnectionKind {
    /// Alle Arten in der Reihenfolge der Einstellungsseite.
    pub const ALL: [ConnectionKind; 4] =
        [ConnectionKind::Eliezer, ConnectionKind::Multigpt, ConnectionKind::Pgvector, ConnectionKind::Lancedb];

    /// Wert in der Tabelle (`eliezer`, `multigpt`, `pgvector`, `lancedb`).
    pub fn as_str(self) -> &'static str {
        match self {
            ConnectionKind::Eliezer => "eliezer",
            ConnectionKind::Multigpt => "multigpt",
            ConnectionKind::Pgvector => "pgvector",
            ConnectionKind::Lancedb => "lancedb",
        }
    }

    /// Vorgabe der Einstellungen, solange keine Zeile gespeichert ist.
    pub fn default_config(self) -> Value {
        match self {
            ConnectionKind::Eliezer => json!({ "url": "http://127.0.0.1:1234" }),
            // Runde 24: keine Adresse als Vorgabe (bis Runde 23 stand hier eine Intranet-Adresse). Eine
            // gespeicherte Zeile bleibt unverändert; ohne Zeile ist MultiGPT „nicht eingerichtet“.
            ConnectionKind::Multigpt => json!({ "url": "" }),
            // Runde 12 (pgvector): Zuordnung von Tabelle und Spalten der Einbettungen und das Modell, mit
            // dem die Daten eingebettet sind; leere Spaltennamen = Spalte nicht vorhanden.
            ConnectionKind::Pgvector => json!({
                "host": "", "port": 5432, "database": "", "user": "", "tls": "prefer",
                "table": "chunks", "text_column": "content", "vector_column": "embedding",
                "collection_column": "collection", "document_column": "document", "section_column": "",
                "title_column": "", "url_column": "", "model_column": "", "embedding_model": "nomic-embed-text",
            }),
            ConnectionKind::Lancedb => json!({ "dir": "lancedb", "embedding_model": "nomic-embed-text" }),
        }
    }
}

impl std::fmt::Display for ConnectionKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for ConnectionKind {
    type Err = StoreError;

    /// Tabellenwert → Art; alles andere → `Refused`.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        ConnectionKind::ALL
            .into_iter()
            .find(|k| k.as_str() == s)
            .ok_or(StoreError::Refused("Unbekannte Verbindung (eliezer, multigpt, pgvector oder lancedb)."))
    }
}

/// Ergebnis der letzten Prüfung einer Verbindung.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ConnectionCheck {
    /// Zeitpunkt (UTC, `YYYY-MM-DDTHH:MM:SS.sssZ`).
    pub at: String,
    pub ok: bool,
    pub message: String,
    /// Angaben der Gegenstelle (Modelle, Werkzeuge, Versionen); nie Geheimnisse.
    pub details: Value,
}

/// Eine Verbindung mit Einstellungen und letzter Prüfung.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ConnectionRow {
    pub kind: ConnectionKind,
    /// Gespeicherte Einstellungen bzw. [`ConnectionKind::default_config`].
    pub config: Value,
    /// Letzte Änderung; `None` = nie gespeichert (Vorgabe).
    pub updated: Option<String>,
    pub check: Option<ConnectionCheck>,
    /// Ziel, für das das Geheimnis hinterlegt wurde, wenn die Einstellungen seitdem auf ein anderes
    /// zeigen (Migration 0014); `None` = das Geheimnis gilt für das jetzige Ziel. Nie ein Geheimnis.
    pub secret_target: Option<String>,
}

/// Ein RAG-Weg eines Werks (Teilmenge in `work_rag.ways`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum RagWay {
    Lancedb,
    Pgvector,
    Multigpt,
}

impl RagWay {
    /// Alle Wege in fester Reihenfolge (so werden sie gespeichert).
    pub const ALL: [RagWay; 3] = [RagWay::Lancedb, RagWay::Pgvector, RagWay::Multigpt];

    pub fn as_str(self) -> &'static str {
        match self {
            RagWay::Lancedb => "lancedb",
            RagWay::Pgvector => "pgvector",
            RagWay::Multigpt => "multigpt",
        }
    }
}

impl std::fmt::Display for RagWay {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for RagWay {
    type Err = StoreError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        RagWay::ALL
            .into_iter()
            .find(|w| w.as_str() == s)
            .ok_or(StoreError::Refused("Unbekannter RAG-Weg (lancedb, pgvector oder multigpt)."))
    }
}

/// Ein Lesezeichen der Liste bevorzugter Quellen.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PreferredSource {
    pub id: i64,
    pub title: String,
    pub url: String,
    pub note: String,
    /// Stelle in der Liste, 1..n lückenlos.
    pub position: i64,
    pub created: String,
    pub updated: String,
}

/// Neue Quelle; der Titel wird am Rand beschnitten.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NewSource {
    pub title: String,
    pub url: String,
    pub note: String,
}

/// Änderung einer Quelle; `None` = unverändert.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SourceEdit {
    pub title: Option<String>,
    pub url: Option<String>,
    pub note: Option<String>,
}

const MAX_SOURCE_TITLE: usize = 200;
const MAX_SOURCE_URL: usize = 2000;

/// Titel getrimmt, 1–200 Zeichen, eine Zeile.
fn source_title(title: &str) -> Result<String, StoreError> {
    let title = title.trim();
    if title.is_empty() || title.chars().count() > MAX_SOURCE_TITLE || title.contains(['\n', '\r']) {
        return Err(StoreError::Refused("Titel einer Quelle: 1 bis 200 Zeichen in einer Zeile."));
    }
    Ok(title.to_owned())
}

/// Adresse getrimmt, `http://` oder `https://`, höchstens 2000 Zeichen, ohne Leerraum.
fn source_url(url: &str) -> Result<String, StoreError> {
    let url = url.trim();
    let lower = url.to_ascii_lowercase();
    let scheme = lower.starts_with("http://") || lower.starts_with("https://");
    if !scheme || url.chars().count() > MAX_SOURCE_URL || url.chars().any(char::is_whitespace) {
        return Err(StoreError::Refused("Adresse einer Quelle: http:// oder https://, höchstens 2000 Zeichen."));
    }
    Ok(url.to_owned())
}

type SourceTuple = (i64, String, String, String, i64, String, String);

fn source_of((id, title, url, note, position, created, updated): SourceTuple) -> PreferredSource {
    PreferredSource { id, title, url, note, position, created, updated }
}

const SOURCE_COLUMNS: &str = "id, title, url, note, position, created, updated";

async fn sources(conn: &mut SqliteConnection) -> Result<Vec<PreferredSource>, StoreError> {
    let rows: Vec<SourceTuple> =
        sqlx::query_as(&format!("SELECT {SOURCE_COLUMNS} FROM preferred_source ORDER BY position, id"))
            .fetch_all(&mut *conn)
            .await?;
    Ok(rows.into_iter().map(source_of).collect())
}

async fn source(conn: &mut SqliteConnection, id: i64) -> Result<PreferredSource, StoreError> {
    let row: Option<SourceTuple> =
        sqlx::query_as(&format!("SELECT {SOURCE_COLUMNS} FROM preferred_source WHERE id = ?"))
            .bind(id)
            .fetch_optional(&mut *conn)
            .await?;
    row.map(source_of).ok_or(StoreError::Refused("Diese Quelle gibt es nicht (mehr)."))
}

/// Stellen `1..n` in der Reihenfolge `ids` vergeben (nur geänderte Zeilen werden geschrieben).
async fn renumber(conn: &mut SqliteConnection, ids: &[i64]) -> Result<(), StoreError> {
    for (index, id) in ids.iter().enumerate() {
        sqlx::query("UPDATE preferred_source SET position = ? WHERE id = ? AND position <> ?")
            .bind(index as i64 + 1)
            .bind(id)
            .bind(index as i64 + 1)
            .execute(&mut *conn)
            .await?;
    }
    Ok(())
}

type ConnectionTuple = (String, String, Option<String>, Option<i64>, Option<String>, Option<String>, Option<String>);

async fn connection_row(conn: &mut SqliteConnection, kind: ConnectionKind) -> Result<ConnectionRow, StoreError> {
    let row: Option<ConnectionTuple> = sqlx::query_as(
        "SELECT config, updated, checked, check_ok, check_message, check_details, secret_target \
         FROM connection WHERE kind = ?",
    )
    .bind(kind.as_str())
    .fetch_optional(&mut *conn)
    .await?;
    let Some(row) = row else {
        return Ok(ConnectionRow {
            kind,
            config: kind.default_config(),
            updated: None,
            check: None,
            secret_target: None,
        });
    };
    row_of(kind, row)
}

fn row_of(kind: ConnectionKind, row: ConnectionTuple) -> Result<ConnectionRow, StoreError> {
    let (config, updated, checked, check_ok, message, details, secret_target) = row;
    let config: Value = serde_json::from_str(&config)
        .map_err(|e| crate::broken(format!("connection.config von {kind} ist kein JSON: {e}")))?;
    let check = match (checked, check_ok) {
        (Some(at), Some(ok)) => Some(ConnectionCheck {
            at,
            ok: ok == 1,
            message: message.unwrap_or_default(),
            details: match details {
                Some(text) => serde_json::from_str(&text)
                    .map_err(|e| crate::broken(format!("connection.check_details von {kind}: {e}")))?,
                None => Value::Null,
            },
        }),
        _ => None,
    };
    Ok(ConnectionRow { kind, config, updated: Some(updated), check, secret_target })
}

/// Werk vorhanden? Sonst `UnknownWork`.
async fn work_exists(conn: &mut SqliteConnection, work: WorkId) -> Result<(), StoreError> {
    let found: Option<i64> =
        sqlx::query_scalar("SELECT id FROM work WHERE id = ?").bind(work).fetch_optional(&mut *conn).await?;
    found.map(|_| ()).ok_or(StoreError::UnknownWork(work))
}

/// Wege in fester Reihenfolge ([`RagWay::ALL`]), ohne Doppel.
fn canonical(ways: &[RagWay]) -> Vec<RagWay> {
    RagWay::ALL.into_iter().filter(|w| ways.contains(w)).collect()
}

impl Store {
    /// Eine Verbindung; ohne gespeicherte Zeile die Vorgabe (`updated` und `check` sind dann `None`).
    pub async fn connection_get(&self, kind: ConnectionKind) -> Result<ConnectionRow, StoreError> {
        let mut conn = self.pool.acquire().await?;
        connection_row(&mut conn, kind).await
    }

    /// Alle vier Verbindungen in der Reihenfolge [`ConnectionKind::ALL`].
    pub async fn connection_list(&self) -> Result<Vec<ConnectionRow>, StoreError> {
        let mut conn = self.pool.acquire().await?;
        let mut rows = Vec::with_capacity(ConnectionKind::ALL.len());
        for kind in ConnectionKind::ALL {
            rows.push(connection_row(&mut conn, kind).await?);
        }
        Ok(rows)
    }

    /// Einstellungen speichern (ganz ersetzen; der Server füllt fehlende Felder vorher auf). `config`
    /// muss ein JSON-Objekt sein, sonst `Refused`. Ändern sich die Einstellungen, gilt die letzte
    /// Prüfung nicht mehr und wird geleert (sie galt der alten Adresse).
    pub async fn connection_set(&self, kind: ConnectionKind, config: &Value) -> Result<ConnectionRow, StoreError> {
        if !config.is_object() {
            return Err(StoreError::Refused("Einstellungen einer Verbindung sind ein JSON-Objekt."));
        }
        let text = config.to_string();
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let before = connection_row(&mut tx, kind).await?;
        let changed = before.updated.is_none() || &before.config != config;
        if changed {
            sqlx::query(
                "INSERT INTO connection (kind, config) VALUES (?, ?) ON CONFLICT(kind) DO UPDATE SET \
                 config = excluded.config, updated = strftime('%Y-%m-%dT%H:%M:%fZ','now'), \
                 checked = NULL, check_ok = NULL, check_message = NULL, check_details = NULL",
            )
            .bind(kind.as_str())
            .bind(&text)
            .execute(&mut *tx)
            .await?;
        }
        let row = connection_row(&mut tx, kind).await?;
        tx.commit().await?;
        Ok(row)
    }

    /// Ziel festhalten, für das das Geheimnis hinterlegt ist (`None` = „gilt für das jetzige Ziel“).
    /// Ohne gespeicherte Zeile und mit `Some` wird sie mit der Vorgabe angelegt; ohne Zeile und mit
    /// `None` ändert sich nichts (keine Zeile heißt schon „jetziges Ziel“). Der Server ruft das vor
    /// [`Store::connection_set`], wenn sich das Ziel ändert, und nach dem Hinterlegen eines Geheimnisses.
    pub async fn connection_bind_secret(&self, kind: ConnectionKind, target: Option<&str>) -> Result<(), StoreError> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        if target.is_some() {
            sqlx::query("INSERT INTO connection (kind, config) VALUES (?, ?) ON CONFLICT(kind) DO NOTHING")
                .bind(kind.as_str())
                .bind(kind.default_config().to_string())
                .execute(&mut *tx)
                .await?;
        }
        sqlx::query("UPDATE connection SET secret_target = ? WHERE kind = ?")
            .bind(target)
            .bind(kind.as_str())
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }

    /// Ergebnis einer Prüfung festhalten (Zeitpunkt jetzt). Ohne gespeicherte Zeile wird sie mit der
    /// Vorgabe angelegt, damit die Prüfung der Vorgabe-Adresse sichtbar bleibt.
    pub async fn connection_record_check(
        &self,
        kind: ConnectionKind,
        ok: bool,
        message: &str,
        details: &Value,
    ) -> Result<ConnectionRow, StoreError> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        sqlx::query("INSERT INTO connection (kind, config) VALUES (?, ?) ON CONFLICT(kind) DO NOTHING")
            .bind(kind.as_str())
            .bind(kind.default_config().to_string())
            .execute(&mut *tx)
            .await?;
        sqlx::query(
            "UPDATE connection SET checked = strftime('%Y-%m-%dT%H:%M:%fZ','now'), check_ok = ?, \
             check_message = ?, check_details = ? WHERE kind = ?",
        )
        .bind(i64::from(ok))
        .bind(message)
        .bind(details.to_string())
        .bind(kind.as_str())
        .execute(&mut *tx)
        .await?;
        let row = connection_row(&mut tx, kind).await?;
        tx.commit().await?;
        Ok(row)
    }

    /// RAG-Wege eines Werks und ob es die Vorgabe (`["lancedb"]`, keine Zeile) ist. Unbekanntes Werk →
    /// `UnknownWork`.
    pub async fn work_rag(&self, work: WorkId) -> Result<(Vec<RagWay>, bool), StoreError> {
        let mut conn = self.pool.acquire().await?;
        work_exists(&mut conn, work).await?;
        let ways: Option<String> = sqlx::query_scalar("SELECT ways FROM work_rag WHERE work_id = ?")
            .bind(work)
            .fetch_optional(&mut *conn)
            .await?;
        let Some(ways) = ways else {
            return Ok((vec![RagWay::Lancedb], true));
        };
        let names: Vec<String> = serde_json::from_str(&ways)
            .map_err(|e| crate::broken(format!("work_rag.ways von Werk {work} ist keine Liste von Texten: {e}")))?;
        let parsed: Vec<RagWay> = names.iter().filter_map(|n| n.parse().ok()).collect();
        Ok((canonical(&parsed), false))
    }

    /// RAG-Wege eines Werks setzen (leer = nur Volltext); gespeichert in fester Reihenfolge ohne Doppel.
    pub async fn set_work_rag(&self, work: WorkId, ways: &[RagWay]) -> Result<Vec<RagWay>, StoreError> {
        let ways = canonical(ways);
        let names: Vec<&str> = ways.iter().map(|w| w.as_str()).collect();
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        work_exists(&mut tx, work).await?;
        sqlx::query(
            "INSERT INTO work_rag (work_id, ways) VALUES (?, ?) ON CONFLICT(work_id) DO UPDATE SET \
             ways = excluded.ways, updated = strftime('%Y-%m-%dT%H:%M:%fZ','now')",
        )
        .bind(work)
        .bind(serde_json::to_string(&names).map_err(|e| crate::broken(e.to_string()))?)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(ways)
    }

    /// Alle Quellen in ihrer Reihenfolge.
    pub async fn sources_list(&self) -> Result<Vec<PreferredSource>, StoreError> {
        let mut conn = self.pool.acquire().await?;
        sources(&mut conn).await
    }

    /// Neue Quelle am Ende der Liste.
    pub async fn source_create(&self, new: &NewSource) -> Result<PreferredSource, StoreError> {
        let title = source_title(&new.title)?;
        let url = source_url(&new.url)?;
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let next: i64 = sqlx::query_scalar("SELECT coalesce(max(position), 0) + 1 FROM preferred_source")
            .fetch_one(&mut *tx)
            .await?;
        let id: i64 = sqlx::query_scalar(
            "INSERT INTO preferred_source (title, url, note, position) VALUES (?, ?, ?, ?) RETURNING id",
        )
        .bind(&title)
        .bind(&url)
        .bind(&new.note)
        .bind(next)
        .fetch_one(&mut *tx)
        .await?;
        let created = source(&mut tx, id).await?;
        tx.commit().await?;
        Ok(created)
    }

    /// Titel, Adresse und/oder Notiz einer Quelle ändern; unbekannte id → `Refused`.
    pub async fn source_update(&self, id: i64, edit: &SourceEdit) -> Result<PreferredSource, StoreError> {
        let title = edit.title.as_deref().map(source_title).transpose()?;
        let url = edit.url.as_deref().map(source_url).transpose()?;
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let before = source(&mut tx, id).await?;
        let title = title.unwrap_or(before.title.clone());
        let url = url.unwrap_or(before.url.clone());
        let note = edit.note.clone().unwrap_or(before.note.clone());
        if (&title, &url, &note) != (&before.title, &before.url, &before.note) {
            sqlx::query(
                "UPDATE preferred_source SET title = ?, url = ?, note = ?, \
                 updated = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ?",
            )
            .bind(&title)
            .bind(&url)
            .bind(&note)
            .bind(id)
            .execute(&mut *tx)
            .await?;
        }
        let after = source(&mut tx, id).await?;
        tx.commit().await?;
        Ok(after)
    }

    /// Quelle an die Stelle `position` (1..n, außerhalb an den Rand) setzen; die übrigen rücken nach.
    /// Liefert die ganze Liste.
    pub async fn source_move(&self, id: i64, position: i64) -> Result<Vec<PreferredSource>, StoreError> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        source(&mut tx, id).await?;
        let mut ids: Vec<i64> = sources(&mut tx).await?.into_iter().map(|s| s.id).filter(|&s| s != id).collect();
        let at = usize::try_from(position.clamp(1, ids.len() as i64 + 1) - 1).unwrap_or(0);
        ids.insert(at, id);
        renumber(&mut tx, &ids).await?;
        let list = sources(&mut tx).await?;
        tx.commit().await?;
        Ok(list)
    }

    /// Quelle löschen (die Bestätigung prüft der Server); die übrigen rücken zusammen. Liefert die
    /// gelöschte Quelle.
    pub async fn source_delete(&self, id: i64) -> Result<PreferredSource, StoreError> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let gone = source(&mut tx, id).await?;
        sqlx::query("DELETE FROM preferred_source WHERE id = ?").bind(id).execute(&mut *tx).await?;
        let ids: Vec<i64> = sources(&mut tx).await?.into_iter().map(|s| s.id).collect();
        renumber(&mut tx, &ids).await?;
        tx.commit().await?;
        Ok(gone)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds_and_ways_round_trip_through_their_table_values() {
        for kind in ConnectionKind::ALL {
            assert_eq!(kind.as_str().parse::<ConnectionKind>().ok(), Some(kind), "{kind} hin und zurück");
            assert!(kind.default_config().is_object(), "Vorgabe von {kind} ist ein Objekt");
        }
        for way in RagWay::ALL {
            assert_eq!(way.as_str().parse::<RagWay>().ok(), Some(way), "{way} hin und zurück");
        }
        assert!("Eliezer".parse::<ConnectionKind>().is_err(), "Tabellenwerte sind klein geschrieben");
        assert!("volltext".parse::<RagWay>().is_err(), "Volltext ist kein Weg, sondern die leere Liste");
    }

    #[test]
    fn titles_and_addresses_of_sources_are_checked() {
        assert_eq!(source_title("  Lexikon  ").unwrap(), "Lexikon", "Titel wird beschnitten");
        assert!(source_title("   ").is_err(), "leerer Titel");
        assert!(source_title("a\nb").is_err(), "Titel in einer Zeile");
        assert!(source_title(&"x".repeat(201)).is_err(), "über 200 Zeichen");
        assert!(source_title(&"ä".repeat(200)).is_ok(), "200 Zeichen (nicht Bytes) gehen");
        assert_eq!(
            source_url(" https://example.org/a ").unwrap(),
            "https://example.org/a",
            "Leerraum am Rand wird abgeschnitten (aus Formularen eingefügt), die Adresse selbst bleibt unverändert – \
             die Websuche vergleicht später ihren Rechner"
        );
        assert!(source_url("HTTP://example.org").is_ok(), "Schema ohne Rücksicht auf Groß/klein");
        for bad in ["ftp://example.org", "javascript:alert(1)", "example.org", "https://a b"] {
            assert!(source_url(bad).is_err(), "{bad:?} ist keine http(s)-Adresse ohne Leerraum");
        }
    }
}
