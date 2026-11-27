// SPDX-License-Identifier: AGPL-3.0-or-later
//! pgvector direkt (Vertrag Runde 12, §3.2 und §5; Aufgabe „pgvector“): „Verbindung prüfen“ ([`check`]) und
//! der Suchweg für die Bedeutungssuche ([`open_way`]), beide nur lesend.
//!
//! - **Zuordnung** ([`Mapping`]): Tabelle (`tabelle` oder `schema.tabelle`) und Spalten kommen aus den
//!   Einstellungen (`table`, `text_column`, `vector_column`, `collection_column`, `document_column`,
//!   wahlweise `section_column`, `title_column`, `url_column`, `model_column`), dazu `embedding_model`.
//!   Die Namen sind vorher geprüft (`config::is_identifier`), werden vor jeder Abfrage gegen
//!   `information_schema.columns` bestätigt ([`resolve`]) und stehen in SQL nur in doppelten
//!   Anführungszeichen. Alles, was von außen kommt (Anfrage, Vektor, Sammlungen, Grenze), ist ein
//!   gebundener Parameter.
//! - **Nur lesend:** jede Sitzung läuft in `BEGIN TRANSACTION READ ONLY` mit `statement_timeout` 10 s
//!   ([`begin_read_only`]); die Verbindung selbst hat ebenfalls 10 s ([`TIMEOUT`]).
//! - **Suche:** die Anfrage wird mit dem Einbetter für `embedding_model` eingebettet (`EmbedderFactory`);
//!   passen Dimension (Spalte bzw. erste Zeile) oder Modell (Antwort des Einbetters, `model_column` der
//!   gewählten Sammlungen) nicht, lehnt der Weg mit [`RagError::ModelMismatch`] ab, ohne die Ähnlichkeit
//!   abzufragen. Sonst Kosinus-Abstand `<=>` gegen `'[…]'::vector`, nur in den gewählten Sammlungen
//!   (`= ANY($n)`), Ähnlichkeit = 1 − Abstand (auf 0..=1 geklemmt).
//! - **Passwort** aus dem Schlüsselbund (`baruch/pgvector`) nur, wenn es für das jetzige Ziel gilt
//!   (`secret_bound`); `PgConnectOptions::new_without_pgpass`, damit nichts aus `PGPASSWORD` oder
//!   `~/.pgpass` des Servers mitgeht. Kein Text enthält das Passwort.
//!
//! Runde 19b (Härtung):
//! - **Gekürzt in SQL:** jedes Feld, das aus der fremden Tabelle kommt, ist schon in der Abfrage per `left(…, n)`
//!   gekürzt – Text ([`SNIPPET_SOURCE_CHARS`]), Dokument ([`MAX_DOCUMENT_CHARS`]), Stelle ([`MAX_SECTION_CHARS`]),
//!   Titel ([`MAX_TITLE_CHARS`]), Adresse ([`MAX_URL_CHARS`]), Sammlung ([`MAX_COLLECTION_CHARS`]) und Modell
//!   ([`MAX_MODEL_CHARS`]), auch die bis zu 1000 Sammlungsnamen der Prüfung. Eine Sammlung mit längerem Namen
//!   bleibt über ihren gekürzten Namen wählbar (die Suche vergleicht beides).
//! - **Bezeichner sicher gequotet** ([`quoted`]): doppelte Anführungszeichen, ein Anführungszeichen im Namen wird
//!   verdoppelt (Regel von PostgreSQL). Das gilt besonders für das Schema, das aus `information_schema` kommt und
//!   nicht vorher geprüft ist; eingestellte Namen bleiben auf [`config::is_identifier`] beschränkt.
//! - **TLS-Hinweis** ([`tls_warning`]): Mit hinterlegtem Passwort und TLS `prefer`, `disable` oder `require`
//!   (verschlüsselt, aber ohne Zertifikatsprüfung) nennen die Prüfung und die Seite „Verbindungen“ das Risiko;
//!   ohne Hinweis bleibt nur `verify-full`.
//! - **Aufgaben-Präfix:** Die Anfrage wird mit `config::embed_prefixes(Pgvector, …).query_text(…)` eingebettet
//!   (Feld `query_prefix`, leer = wie bisher); die Daten in der Tabelle sind schon eingebettet.

use std::sync::Arc;
use std::time::Duration;

use baruch_rag::{
    BoxFuture, EmbedPrefixes, Embedder, EmbedderFactory, Hit, Query, RagError, SearchWay, Way, clean_snippet,
};
use baruch_store::ConnectionKind;
use serde_json::{Map, Value, json};
use sqlx::postgres::{PgConnectOptions, PgConnection, PgSslMode};
use sqlx::{Connection, Postgres, Transaction};

use super::check::Outcome;
use super::config;
use crate::AppState;

/// Zeitlimit für die Verbindung und für jede Anweisung (`statement_timeout`).
pub const TIMEOUT: Duration = Duration::from_secs(10);
/// Höchstens so viele Sammlungen zeigt die Prüfung.
const MAX_LISTED_COLLECTIONS: i64 = 1000;
/// Höchstens so viele verschiedene Modelle zeigt die Prüfung.
const MAX_LISTED_MODELS: i64 = 20;
/// So viele Zeichen des Texts holt die Suche je Treffer (der Ausschnitt kürzt weiter).
pub const SNIPPET_SOURCE_CHARS: i64 = 2000;
/// Höchstlänge des Dokuments (Kennung) je Treffer, in SQL gekürzt (Runde 19b).
pub const MAX_DOCUMENT_CHARS: i64 = 500;
/// Höchstlänge der Stelle je Treffer.
pub const MAX_SECTION_CHARS: i64 = 200;
/// Höchstlänge des Titels je Treffer.
pub const MAX_TITLE_CHARS: i64 = 300;
/// Höchstlänge der Adresse je Treffer (wie `check_url`).
pub const MAX_URL_CHARS: i64 = 2000;
/// Höchstlänge eines Sammlungsnamens (Treffer und Prüfung; wie die Sammlungen im Store, 1 bis 200 Zeichen).
pub const MAX_COLLECTION_CHARS: i64 = 200;
/// Höchstlänge eines Modellnamens aus `model_column` (wie `embedding_model`).
pub const MAX_MODEL_CHARS: i64 = 200;

/// Zuordnung von Tabelle und Spalten (geprüft, aber noch nicht gegen die Datenbank bestätigt).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mapping {
    /// Schema aus `schema.tabelle`; `None` = der Suchpfad des Benutzers (`current_schemas`).
    pub schema: Option<String>,
    pub table: String,
    pub text: String,
    pub vector: String,
    pub collection: String,
    pub document: String,
    pub section: Option<String>,
    pub title: Option<String>,
    pub url: Option<String>,
    pub model: Option<String>,
    /// Modell, mit dem die Daten eingebettet sind (und die Anfrage eingebettet wird).
    pub embedding_model: String,
}

impl Mapping {
    /// Aus den Einstellungen (fehlende Felder mit ihrer Vorgabe, `config::effective`).
    pub fn from_config(config: &Value) -> Result<Mapping, String> {
        let config = config::effective(ConnectionKind::Pgvector, config);
        let field = |name: &str| config.get(name).and_then(Value::as_str).unwrap_or("").trim().to_owned();
        let table = field("table");
        let (schema, name) = config::split_table(&table)
            .ok_or_else(|| format!("Tabelle „{table}“ ist kein gültiger Name (tabelle oder schema.tabelle)."))?;
        let required = |name: &str| {
            let column = field(name);
            if config::is_identifier(&column) {
                Ok(column)
            } else {
                Err(format!("{name}: „{column}“ ist kein gültiger Spaltenname."))
            }
        };
        let optional = |name: &str| {
            let column = field(name);
            match column.as_str() {
                "" => Ok(None),
                c if config::is_identifier(c) => Ok(Some(column)),
                _ => Err(format!("{name}: „{column}“ ist kein gültiger Spaltenname.")),
            }
        };
        let embedding_model = field("embedding_model");
        if embedding_model.is_empty() {
            return Err("Kein Einbettungsmodell eingetragen (embedding_model).".into());
        }
        Ok(Mapping {
            schema: schema.map(str::to_owned),
            table: name.to_owned(),
            text: required("text_column")?,
            vector: required("vector_column")?,
            collection: required("collection_column")?,
            document: required("document_column")?,
            section: optional("section_column")?,
            title: optional("title_column")?,
            url: optional("url_column")?,
            model: optional("model_column")?,
            embedding_model,
        })
    }

    /// Rollen und Spalten (nur die vorhandenen), in fester Reihenfolge.
    pub fn columns(&self) -> Vec<(&'static str, &str)> {
        let mut out = vec![
            ("text", self.text.as_str()),
            ("vector", self.vector.as_str()),
            ("collection", self.collection.as_str()),
            ("document", self.document.as_str()),
        ];
        for (role, column) in
            [("section", &self.section), ("title", &self.title), ("url", &self.url), ("model", &self.model)]
        {
            if let Some(column) = column {
                out.push((role, column.as_str()));
            }
        }
        out
    }

    /// `{rolle: spalte}` für die Angaben der Prüfung.
    fn columns_json(&self) -> Value {
        Value::Object(self.columns().into_iter().map(|(role, column)| (role.to_owned(), json!(column))).collect())
    }

    /// Tabelle wie eingestellt (`schema.tabelle` bzw. `tabelle`).
    pub fn table_label(&self) -> String {
        match &self.schema {
            Some(schema) => format!("{schema}.{}", self.table),
            None => self.table.clone(),
        }
    }
}

/// Deutsche Bezeichnung einer Rolle für Meldungen.
fn role_label(role: &str) -> &'static str {
    match role {
        "text" => "Text",
        "vector" => "Vektor",
        "collection" => "Sammlung",
        "document" => "Dokument",
        "section" => "Stelle",
        "title" => "Titel",
        "url" => "Adresse",
        _ => "Modell",
    }
}

/// Ein Bezeichner in doppelten Anführungszeichen für SQL (Runde 19b): Ein Anführungszeichen im Namen wird
/// verdoppelt (`we"ird` → `"we""ird"`), so bleibt jeder Name genau ein Bezeichner – auch ein Schema aus
/// `information_schema`, das Baruch nicht vorher prüfen kann. Eingestellte Namen sind ohnehin auf
/// [`config::is_identifier`] beschränkt.
pub fn quoted(identifier: &str) -> String {
    format!("\"{}\"", identifier.replace('"', "\"\""))
}

/// Hinweis bei hinterlegtem Passwort und schwachem TLS (`prefer` oder `disable`, auch `prefer` als Vorgabe) sowie
/// – milder – bei `require`: sqlx prüft dort das Zertifikat nicht (`accept_invalid_certs` außer bei
/// `VerifyCa`/`VerifyFull`, Baruch setzt keine Stamm-CA), ein Angreifer im Netz könnte sich also als Server
/// ausgeben. `None` nur bei `verify-full` oder ohne Passwort. Für die Prüfung und die Seite „Verbindungen“
/// (Runde 19b, `require` seit der Nachbesserung zu Runde 19).
pub fn tls_warning(config: &Value, has_secret: bool) -> Option<String> {
    if !has_secret {
        return None;
    }
    let config = config::effective(ConnectionKind::Pgvector, config);
    match config.get("tls").and_then(Value::as_str).unwrap_or("prefer").trim() {
        "verify-full" => None,
        "require" => Some(
            "Hinweis: TLS steht auf „require“ – die Verbindung ist verschlüsselt, das Zertifikat des Servers prüft \
             Baruch dabei aber nicht. Wer sich im Netz als Server ausgibt, könnte Anmeldung, Anfragen und Treffer, \
             je nach Anmeldeverfahren auch das Passwort, mitlesen. Sicher ist nur „verify-full“."
                .to_owned(),
        ),
        "disable" => Some(
            "Achtung: TLS steht auf „disable“ – Anmeldung, Anfragen und Treffer gehen unverschlüsselt über das Netz, \
             je nach Anmeldeverfahren des Servers auch das Passwort. Besser „verify-full“ einstellen."
                .to_owned(),
        ),
        _ => Some(
            "Achtung: TLS steht auf „prefer“ – bietet der Server kein TLS an, verbindet Baruch unverschlüsselt und \
             ohne Prüfung des Zertifikats; Anmeldung, Anfragen und Treffer, je nach Anmeldeverfahren auch das \
             Passwort, gingen dann offen über das Netz. Besser „verify-full“ einstellen."
                .to_owned(),
        ),
    }
}

/// Tabelle nach [`resolve`]: Schema, in dem sie gefunden wurde, und die Dimension der Vektorspalte.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolved {
    pub schema: String,
    /// Feste Dimension der Spalte (`vector(n)`) bzw. die der ersten Zeile; `None` = keine bekannt.
    pub dimension: Option<usize>,
}

impl Resolved {
    /// `"schema"."tabelle"` für SQL.
    fn table_sql(&self, mapping: &Mapping) -> String {
        format!("{}.{}", quoted(&self.schema), quoted(&mapping.table))
    }
}

/// Gleiches Modell? Vergleicht nur Buchstaben und Ziffern ohne Groß-/Kleinschreibung; ein Name darf im
/// anderen enthalten sein (Eliezer antwortet z. B. auf „nomic-embed-text“ mit
/// „text-embedding-nomic-embed-text-v1.5“).
pub fn same_model(a: &str, b: &str) -> bool {
    let norm = |s: &str| s.chars().filter(|c| c.is_alphanumeric()).flat_map(char::to_lowercase).collect::<String>();
    let (a, b) = (norm(a), norm(b));
    !a.is_empty() && !b.is_empty() && (a == b || a.contains(&b) || b.contains(&a))
}

/// Vektor als Text für `$n::vector`, z. B. `[0.1,-0.25]`; nicht endliche Zahlen → `None`.
pub fn vector_literal(vector: &[f32]) -> Option<String> {
    if vector.is_empty() || vector.iter().any(|x| !x.is_finite()) {
        return None;
    }
    Some(format!("[{}]", vector.iter().map(f32::to_string).collect::<Vec<_>>().join(",")))
}

/// Ähnlichkeit aus dem Kosinus-Abstand (0..2): 1 − Abstand, auf 0..=1 geklemmt.
pub fn score_of(distance: f64) -> f32 {
    if distance.is_nan() { 0.0 } else { (1.0 - distance).clamp(0.0, 1.0) as f32 }
}

/// Transaktion `READ ONLY` mit `statement_timeout` (nur für diese Transaktion, `set_config(…, true)`).
/// Jede Abfrage dieses Moduls läuft darin; eine Schreibanweisung scheitert mit SQLSTATE 25006.
pub async fn begin_read_only(
    conn: &mut PgConnection,
    timeout: Duration,
) -> Result<Transaction<'_, Postgres>, sqlx::Error> {
    let mut tx = conn.begin_with("BEGIN TRANSACTION READ ONLY").await?;
    sqlx::query("SELECT set_config('statement_timeout', $1, true)")
        .bind(timeout.as_millis().to_string())
        .execute(&mut *tx)
        .await?;
    Ok(tx)
}

/// Anmeldedaten aus den Einstellungen (Rechner, Port, TLS, Datenbank, Benutzer) und dem Passwort.
/// `None` ohne Rechner.
pub fn connect_options(config: &Value, secret: Option<&str>) -> Option<(PgConnectOptions, String)> {
    let field = |name: &str| config.get(name).and_then(Value::as_str).unwrap_or("").trim().to_owned();
    let host = field("host");
    if host.is_empty() {
        return None;
    }
    let port = config.get("port").and_then(Value::as_u64).and_then(|p| u16::try_from(p).ok()).unwrap_or(5432);
    let ssl = match field("tls").as_str() {
        "disable" => PgSslMode::Disable,
        "require" => PgSslMode::Require,
        "verify-full" => PgSslMode::VerifyFull,
        _ => PgSslMode::Prefer,
    };
    // Ohne pgpass und mit ausdrücklichem (ggf. leerem) Passwort: nichts aus der Umgebung des Servers
    // (PGPASSWORD, ~/.pgpass) gelangt in die Anmeldung.
    let mut options = PgConnectOptions::new_without_pgpass()
        .host(&host)
        .port(port)
        .ssl_mode(ssl)
        .password(secret.unwrap_or(""))
        .application_name("baruch");
    let user = field("user");
    if !user.is_empty() {
        options = options.username(&user);
    }
    let database = field("database");
    if !database.is_empty() {
        options = options.database(&database);
    }
    Some((options, format!("{host}:{port}")))
}

/// Fehler der Anmeldung als deutscher Satz (ohne Passwort).
fn connect_message(e: &sqlx::Error, address: &str, has_secret: bool) -> String {
    match e {
        sqlx::Error::Database(db) if matches!(db.code().as_deref(), Some("28P01" | "28000")) => {
            let hint = if has_secret {
                "Passwort baruch/pgvector prüfen"
            } else {
                "kein Passwort hinterlegt (baruch/pgvector)"
            };
            format!("Anmeldung abgelehnt: {} ({hint}).", db.message())
        }
        sqlx::Error::Database(db) => format!("Datenbank {address}: {}", db.message()),
        sqlx::Error::Io(io) => format!("Keine Verbindung zu {address}: {io}"),
        sqlx::Error::Tls(tls) => format!("TLS mit {address}: {tls}"),
        other => format!("Verbindung zu {address}: {other}"),
    }
}

/// sqlx-Fehler → [`RagError`] (Anmeldung, Rechte, Zeitlimit, Netz, sonst „gescheitert“).
fn rag_error(e: &sqlx::Error, address: &str, has_secret: bool) -> RagError {
    let message = connect_message(e, address, has_secret);
    match e {
        sqlx::Error::Database(db) => match db.code().as_deref() {
            Some("28P01" | "28000" | "42501") => RagError::Denied(message),
            Some("57014") => RagError::Timeout(format!(
                "pgvector {address}: Abfrage länger als {} s, abgebrochen (statement_timeout).",
                TIMEOUT.as_secs()
            )),
            Some("42P01" | "42703") => RagError::Schema(message),
            _ => RagError::Failed(message),
        },
        sqlx::Error::Io(_) | sqlx::Error::Tls(_) => RagError::Unreachable(message),
        _ => RagError::Failed(message),
    }
}

/// Tabelle und Spalten gegen `information_schema.columns` bestätigen (Rechte des Benutzers zählen: was er
/// nicht sehen darf, gibt es für Baruch nicht), Vektorspalte vom Typ `vector`, Dimension aus
/// `pg_attribute.atttypmod` bzw. – ohne feste Dimension – `vector_dims` der ersten Zeile. Eine fehlende
/// Tabelle bzw. Spalte ist `Ok(Err(Meldung))` (mit ihrem Namen), ein Datenbankfehler `Err`.
pub async fn resolve(conn: &mut PgConnection, mapping: &Mapping) -> Result<Result<Resolved, String>, sqlx::Error> {
    let rows: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT c.table_schema::text, c.column_name::text, c.udt_name::text \
         FROM information_schema.columns c \
         WHERE c.table_name = $1 AND (($2::text IS NULL AND c.table_schema::name = ANY(current_schemas(false))) \
                                      OR c.table_schema = $2::text) \
         ORDER BY array_position(current_schemas(false), c.table_schema::name), c.table_schema, c.ordinal_position",
    )
    .bind(&mapping.table)
    .bind(mapping.schema.as_deref())
    .fetch_all(&mut *conn)
    .await?;
    let Some(schema) = rows.first().map(|(schema, _, _)| schema.clone()) else {
        let place =
            mapping.schema.as_deref().map_or("im Suchpfad des Benutzers".to_owned(), |s| format!("im Schema {s}"));
        return Ok(Err(format!(
            "Tabelle „{}“ gibt es {place} nicht (oder der Benutzer darf sie nicht lesen).",
            mapping.table
        )));
    };
    let columns: Vec<(&str, &str)> =
        rows.iter().filter(|(s, _, _)| *s == schema).map(|(_, name, udt)| (name.as_str(), udt.as_str())).collect();
    for (role, column) in mapping.columns() {
        let Some((_, udt)) = columns.iter().find(|(name, _)| *name == column) else {
            return Ok(Err(format!(
                "Spalte „{column}“ ({}) fehlt in Tabelle {schema}.{}.",
                role_label(role),
                mapping.table
            )));
        };
        if role == "vector" && *udt != "vector" {
            return Ok(Err(format!("Spalte „{column}“ hat den Typ {udt}, nicht vector (pgvector).")));
        }
    }
    let resolved = Resolved { schema, dimension: None };
    let regclass = resolved.table_sql(mapping);
    let typmod: Option<i32> = sqlx::query_scalar(
        "SELECT a.atttypmod FROM pg_attribute a WHERE a.attrelid = to_regclass($1) AND a.attname = $2 AND NOT a.attisdropped",
    )
    .bind(&regclass)
    .bind(&mapping.vector)
    .fetch_optional(&mut *conn)
    .await?;
    let mut dimension = typmod.filter(|&t| t > 0).map(|t| t as usize);
    if dimension.is_none() {
        let sql = format!(
            "SELECT vector_dims({vector}) FROM {regclass} WHERE {vector} IS NOT NULL LIMIT 1",
            vector = quoted(&mapping.vector)
        );
        let first: Option<i32> =
            sqlx::query_scalar(sqlx::AssertSqlSafe(sql.as_str())).fetch_optional(&mut *conn).await?;
        dimension = first.filter(|&d| d > 0).map(|d| d as usize);
    }
    Ok(Ok(Resolved { dimension, ..resolved }))
}

/// Ergebnis der Prüfung einer Tabelle.
struct TableInfo {
    resolved: Resolved,
    collections: Vec<(String, i64)>,
    chunks: i64,
    models: Vec<String>,
}

async fn table_info(conn: &mut PgConnection, mapping: &Mapping) -> Result<Result<TableInfo, String>, sqlx::Error> {
    let resolved = match resolve(conn, mapping).await? {
        Ok(resolved) => resolved,
        Err(message) => return Ok(Err(message)),
    };
    let from = resolved.table_sql(mapping);
    let collection = quoted(&mapping.collection);
    // Runde 19b: Die Namen kommen schon gekürzt aus der Datenbank (gezählt wird je vollem Namen).
    let collections: Vec<(String, i64)> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT left(name, {MAX_COLLECTION_CHARS}), n FROM \
           (SELECT coalesce({collection}::text, '') AS name, count(*) AS n FROM {from} \
            GROUP BY 1 ORDER BY 1 LIMIT {MAX_LISTED_COLLECTIONS}) AS listed \
         ORDER BY name"
    )))
    .fetch_all(&mut *conn)
    .await?;
    let chunks: i64 =
        sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT count(*) FROM {from}"))).fetch_one(&mut *conn).await?;
    let models: Vec<String> = match &mapping.model {
        Some(model) => {
            let model = quoted(model);
            sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
                "SELECT DISTINCT left({model}::text, {MAX_MODEL_CHARS}) FROM {from} WHERE {model} IS NOT NULL \
                 ORDER BY 1 LIMIT {MAX_LISTED_MODELS}"
            )))
            .fetch_all(&mut *conn)
            .await?
        }
        None => Vec::new(),
    };
    Ok(Ok(TableInfo { resolved, collections, chunks, models }))
}

fn failed(message: impl Into<String>, details: Value) -> Outcome {
    Outcome { ok: false, message: message.into(), details }
}

/// „Verbindung prüfen“ für pgvector: anmelden, Serverversion und Erweiterung `vector`, dann Tabelle und
/// Spalten der Zuordnung, Sammlungen mit Zahl der Abschnitte, Dimension und (mit `model_column`) die
/// vermerkten Modelle – alles in einer Transaktion `READ ONLY`. Angaben:
/// `{server_version, vector, table, columns: {rolle: spalte}, collections: [{name, count}], chunks,
/// dimension, models}`. `ok` = verbunden ∧ `vector` da ∧ Tabelle/Spalten passen ∧ Dimension bekannt.
pub async fn check(config: &Value, secret: Option<&str>) -> Outcome {
    let Some((options, address)) = connect_options(config, secret) else {
        return failed("Kein Rechner (Host) eingetragen.", json!({}));
    };
    let mapping = match Mapping::from_config(config) {
        Ok(mapping) => mapping,
        Err(message) => return failed(format!("Zuordnung ungültig: {message}"), json!({})),
    };
    let attempt = async {
        let mut conn = PgConnection::connect_with(&options).await?;
        let version: String = sqlx::query_scalar("SELECT version()").fetch_one(&mut conn).await?;
        let vector: Option<String> = sqlx::query_scalar("SELECT extversion FROM pg_extension WHERE extname = 'vector'")
            .fetch_optional(&mut conn)
            .await?;
        let table = match &vector {
            Some(_) => {
                let mut tx = begin_read_only(&mut conn, TIMEOUT).await?;
                let info = table_info(&mut tx, &mapping).await;
                let _ = tx.rollback().await;
                Some(info)
            }
            None => None,
        };
        let _ = conn.close().await;
        Ok::<_, sqlx::Error>((version, vector, table))
    };
    let (version, vector, table) = match tokio::time::timeout(TIMEOUT, attempt).await {
        Err(_) => {
            return failed(format!("Keine Antwort von {address} innerhalb von {} s.", TIMEOUT.as_secs()), json!({}));
        }
        Ok(Err(e)) => return failed(connect_message(&e, &address, secret.is_some()), json!({})),
        Ok(Ok(result)) => result,
    };
    let Some(vector) = vector else {
        return failed(
            "Verbindung steht, aber die Erweiterung vector fehlt (CREATE EXTENSION vector).",
            json!({ "server_version": version, "vector": null }),
        );
    };
    let mut details = Map::new();
    details.insert("server_version".into(), json!(version));
    details.insert("vector".into(), json!(vector));
    details.insert("table".into(), json!(mapping.table_label()));
    details.insert("columns".into(), mapping.columns_json());
    let info = match table {
        Some(Ok(Ok(info))) => info,
        Some(Ok(Err(message))) => {
            return failed(format!("Verbindung steht (vector {vector}), aber: {message}"), Value::Object(details));
        }
        Some(Err(e)) => {
            let why = rag_error(&e, &address, secret.is_some());
            return failed(format!("Verbindung steht (vector {vector}), aber: {why}"), Value::Object(details));
        }
        None => unreachable!("mit vector wird die Tabelle geprüft"),
    };
    details.insert("table".into(), json!(format!("{}.{}", info.resolved.schema, mapping.table)));
    details.insert(
        "collections".into(),
        Value::Array(info.collections.iter().map(|(name, count)| json!({ "name": name, "count": count })).collect()),
    );
    details.insert("chunks".into(), json!(info.chunks));
    details.insert("dimension".into(), json!(info.resolved.dimension));
    details.insert("models".into(), json!(info.models));
    let Some(dimension) = info.resolved.dimension else {
        return failed(
            format!(
                "Verbindung steht (vector {vector}), aber die Vektorspalte „{}“ hat keine feste Dimension und noch keine Daten.",
                mapping.vector
            ),
            Value::Object(details),
        );
    };
    let count = info.collections.len();
    let word = if count == 1 { "Sammlung" } else { "Sammlungen" };
    let warning = tls_warning(config, secret.is_some());
    let mut message = format!(
        "Verbindung steht, Erweiterung vector {vector}: {count} {word}, {} Abschnitte, Dimension {dimension}",
        info.chunks
    );
    if info.models.is_empty() {
        message.push_str(&format!("; eingestellt ist das Modell {}.", mapping.embedding_model));
    } else {
        message.push_str(&format!("; Modelle in den Daten: {}.", info.models.join(", ")));
        let strangers: Vec<&str> =
            info.models.iter().map(String::as_str).filter(|m| !same_model(m, &mapping.embedding_model)).collect();
        if !strangers.is_empty() {
            message.push_str(&format!(
                " Achtung: Eingestellt ist {}; Sammlungen mit {} lehnt die Suche ab (anderes Modell).",
                mapping.embedding_model,
                strangers.join(", ")
            ));
        }
    }
    if let Some(warning) = warning {
        message.push(' ');
        message.push_str(&warning);
        details.insert("tls_warning".into(), json!(warning));
    }
    Outcome { ok: true, message, details: Value::Object(details) }
}

/// Der Suchweg pgvector (eine Verbindung je Suche, nur lesend).
struct PgvectorWay {
    options: PgConnectOptions,
    address: String,
    /// Nur zum Schwärzen von Meldungen (`***`), nie ausgegeben.
    secret: Option<String>,
    mapping: Mapping,
    embedder: Arc<dyn Embedder>,
    /// Aufgaben-Präfix der Anfrage (Runde 19b; `document` ist bei pgvector immer leer).
    prefixes: EmbedPrefixes,
}

impl PgvectorWay {
    /// Meldung ohne Passwort (Runde 23: über `multigpt::scrub_error`, also [`baruch_llm::scrub`] – auch
    /// Base64- und Prozent-kodiert; die Fehlerart bleibt).
    fn scrub(&self, error: RagError) -> RagError {
        super::multigpt::scrub_error(error, self.secret.as_deref())
    }

    fn db_error(&self, e: &sqlx::Error) -> RagError {
        rag_error(e, &self.address, self.secret.is_some())
    }

    async fn run(&self, query: &Query) -> Result<Vec<Hit>, RagError> {
        if query.collections.is_empty() {
            return Err(RagError::NotConfigured("Keine Sammlung gewählt (Einstellungen → Verbindungen).".into()));
        }
        let mapping = &self.mapping;
        let mut conn = match tokio::time::timeout(TIMEOUT, PgConnection::connect_with(&self.options)).await {
            Err(_) => {
                return Err(RagError::Timeout(format!(
                    "Keine Antwort von {} innerhalb von {} s.",
                    self.address,
                    TIMEOUT.as_secs()
                )));
            }
            Ok(Err(e)) => return Err(self.db_error(&e)),
            Ok(Ok(conn)) => conn,
        };
        let result = self.query_in(&mut conn, query, mapping).await;
        let _ = conn.close().await;
        result
    }

    async fn query_in(&self, conn: &mut PgConnection, query: &Query, mapping: &Mapping) -> Result<Vec<Hit>, RagError> {
        let mut tx = begin_read_only(conn, TIMEOUT).await.map_err(|e| self.db_error(&e))?;
        let resolved = match resolve(&mut tx, mapping).await.map_err(|e| self.db_error(&e))? {
            Ok(resolved) => resolved,
            Err(message) => return Err(RagError::Schema(format!("pgvector: {message}"))),
        };
        let from = resolved.table_sql(mapping);
        let collection = quoted(&mapping.collection);
        // Runde 19b: gewählt ist der volle Name oder (bei überlangen Namen) der gekürzte aus der Prüfung.
        let chosen = |n: u8| {
            format!("({collection}::text = ANY(${n}) OR left({collection}::text, {MAX_COLLECTION_CHARS}) = ANY(${n}))")
        };
        if let Some(model) = &mapping.model {
            let model = quoted(model);
            let found: Vec<String> = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
                "SELECT DISTINCT left({model}::text, {MAX_MODEL_CHARS}) FROM {from} WHERE {} AND {model} IS NOT NULL \
                 ORDER BY 1 LIMIT {MAX_LISTED_MODELS}",
                chosen(1)
            )))
            .bind(&query.collections)
            .fetch_all(&mut *tx)
            .await
            .map_err(|e| self.db_error(&e))?;
            let strangers: Vec<&str> =
                found.iter().map(String::as_str).filter(|m| !same_model(m, &mapping.embedding_model)).collect();
            if !strangers.is_empty() {
                return Err(RagError::ModelMismatch(format!(
                    "pgvector: Die gewählten Sammlungen sind mit {} eingebettet, eingestellt ist {}; Treffer wären wertlos.",
                    strangers.join(", "),
                    mapping.embedding_model
                )));
            }
        }
        let Some(dimension) = resolved.dimension else {
            return Err(RagError::Schema(format!(
                "pgvector: Die Vektorspalte „{}“ hat keine feste Dimension und noch keine Daten.",
                mapping.vector
            )));
        };
        let text = self.prefixes.query_text(&query.text);
        let embeddings = self.embedder.embed(std::slice::from_ref(&text)).await?;
        if !same_model(&embeddings.model, &mapping.embedding_model) {
            return Err(RagError::ModelMismatch(format!(
                "pgvector: Die Anfrage wurde mit {} eingebettet, die Daten mit {}; Treffer wären wertlos.",
                embeddings.model, mapping.embedding_model
            )));
        }
        let vector = match embeddings.vectors.as_slice() {
            [vector] => vector,
            _ => return Err(RagError::Failed("pgvector: Der Einbetter lieferte nicht genau einen Vektor.".into())),
        };
        if vector.len() != dimension || embeddings.dimension != dimension {
            return Err(RagError::ModelMismatch(format!(
                "pgvector: Die Anfrage hat {} Dimensionen, die Spalte „{}“ {dimension}; anderes Einbettungsmodell?",
                vector.len(),
                mapping.vector
            )));
        }
        let literal = vector_literal(vector)
            .ok_or_else(|| RagError::Failed("pgvector: Der Einbetter lieferte keine endlichen Zahlen.".into()))?;
        // Runde 19b: jedes Feld schon in SQL gekürzt (wie der Text), damit eine fremde Tabelle mit riesigen Werten
        // weder Speicher noch Antwort sprengt.
        let optional = |column: &Option<String>, max: i64| {
            column.as_deref().map_or("NULL::text".to_owned(), |c| format!("left({}::text, {max})", quoted(c)))
        };
        let sql = format!(
            "SELECT left(coalesce({document}::text, ''), {MAX_DOCUMENT_CHARS}), {section}, {title}, {url}, \
                    left(coalesce({collection}::text, ''), {MAX_COLLECTION_CHARS}), \
                    left(coalesce({text}::text, ''), {SNIPPET_SOURCE_CHARS}), \
                    ({vector} <=> $1::vector)::float8 \
             FROM {from} \
             WHERE {chosen} AND {vector} IS NOT NULL \
             ORDER BY {vector} <=> $1::vector, 1, 2 \
             LIMIT $3",
            chosen = chosen(2),
            document = quoted(&mapping.document),
            section = optional(&mapping.section, MAX_SECTION_CHARS),
            title = optional(&mapping.title, MAX_TITLE_CHARS),
            url = optional(&mapping.url, MAX_URL_CHARS),
            text = quoted(&mapping.text),
            vector = quoted(&mapping.vector),
        );
        type Row = (String, Option<String>, Option<String>, Option<String>, String, String, f64);
        let rows: Vec<Row> = sqlx::query_as(sqlx::AssertSqlSafe(sql.as_str()))
            .bind(&literal)
            .bind(&query.collections)
            .bind(query.limit.clamp(1, baruch_rag::MAX_LIMIT) as i64)
            .fetch_all(&mut *tx)
            .await
            .map_err(|e| self.db_error(&e))?;
        let _ = tx.rollback().await;
        let table = format!("{}.{}", resolved.schema, mapping.table);
        Ok(rows
            .into_iter()
            .map(|(document, section, title, url, collection, text, distance)| {
                let section = section.map(|s| s.trim().to_owned()).filter(|s| !s.is_empty());
                let title = title.map(|t| t.trim().to_owned()).filter(|t| !t.is_empty());
                let url = url.map(|u| u.trim().to_owned()).filter(|u| {
                    let lower = u.to_ascii_lowercase();
                    (lower.starts_with("https://") || lower.starts_with("http://")) && !u.contains(char::is_whitespace)
                });
                Hit {
                    way: Way::Pgvector,
                    collection,
                    title: title.unwrap_or_else(|| document.clone()),
                    document,
                    section,
                    snippet: clean_snippet(&text),
                    score: score_of(distance),
                    url,
                    meta: json!({ "distance": distance, "table": table }),
                }
            })
            .collect())
    }
}

impl SearchWay for PgvectorWay {
    fn way(&self) -> Way {
        Way::Pgvector
    }

    fn search<'a>(&'a self, query: &'a Query) -> BoxFuture<'a, Result<Vec<Hit>, RagError>> {
        Box::pin(async move { self.run(query).await.map_err(|e| self.scrub(e)) })
    }
}

/// Suchweg aus Einstellungen und Passwort (für [`open_way`] und Tests). Ohne Rechner → `NotConfigured`.
pub fn way_from_config(
    config: &Value,
    secret: Option<&str>,
    embedders: Arc<dyn EmbedderFactory>,
) -> Result<Box<dyn SearchWay>, RagError> {
    let config = config::effective(ConnectionKind::Pgvector, config);
    let Some((options, address)) = connect_options(&config, secret) else {
        return Err(RagError::NotConfigured(
            "pgvector: Kein Rechner eingetragen (Einstellungen → Verbindungen).".into(),
        ));
    };
    let mapping = Mapping::from_config(&config).map_err(|m| RagError::Schema(format!("pgvector: {m}")))?;
    let embedder = embedders.for_model(&mapping.embedding_model);
    let prefixes = config::embed_prefixes(ConnectionKind::Pgvector, &config);
    Ok(Box::new(PgvectorWay { options, address, secret: secret.map(str::to_owned), mapping, embedder, prefixes }))
}

/// Suchweg pgvector nach den gespeicherten Einstellungen (Vertrag §3.2). Kein Rechner → `NotConfigured`;
/// das Passwort `baruch/pgvector` nur, wenn es für das jetzige Ziel gilt, sonst `NotConfigured` mit
/// Hinweis (es geht dann nirgendwohin).
pub async fn open_way(state: &AppState, embedders: Arc<dyn EmbedderFactory>) -> Result<Box<dyn SearchWay>, RagError> {
    let row = state
        .store
        .connection_get(ConnectionKind::Pgvector)
        .await
        .map_err(|e| RagError::Failed(format!("Einstellungen von pgvector: {e}")))?;
    let effective = config::effective(ConnectionKind::Pgvector, &row.config);
    if connect_options(&effective, None).is_none() {
        return Err(RagError::NotConfigured(
            "pgvector: Kein Rechner eingetragen (Einstellungen → Verbindungen).".into(),
        ));
    }
    let secret = super::secrets::get(&state.secrets, "pgvector")
        .await
        .map_err(|e| RagError::Failed(format!("Schlüsselbund: {e}")))?;
    if secret.is_some() && !super::secret_bound(&row) {
        return Err(RagError::NotConfigured(
            "pgvector: Das Passwort baruch/pgvector ist für ein anderes Ziel hinterlegt; Baruch schickt es nicht an \
             die jetzige Adresse. Für sie das Passwort neu eintragen (Einstellungen → Verbindungen)."
                .into(),
        ));
    }
    way_from_config(&effective, secret.as_deref(), embedders)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn models_match_loosely_but_not_across_families() {
        assert!(same_model("nomic-embed-text", "text-embedding-nomic-embed-text-v1.5"), "Alias von Eliezer");
        assert!(same_model("Nomic-Embed-Text", "nomic_embed_text"), "Schreibweise egal");
        assert!(!same_model("nomic-embed-text", "bge-m3"), "anderes Modell");
        assert!(!same_model("", "bge-m3"), "leer passt zu nichts");
    }

    #[test]
    fn vectors_become_text_literals_and_distances_become_scores() {
        assert_eq!(vector_literal(&[0.5, -0.25, 1.0]).as_deref(), Some("[0.5,-0.25,1]"), "Textform für ::vector");
        assert_eq!(vector_literal(&[f32::NAN]), None, "NaN wird nicht gesendet");
        assert_eq!(vector_literal(&[]), None, "leerer Vektor wird nicht gesendet");
        assert_eq!(score_of(0.0), 1.0, "Abstand 0 = gleich");
        assert!((score_of(0.25) - 0.75).abs() < 1e-6, "1 − Abstand");
        assert_eq!(score_of(1.5), 0.0, "gegenläufig: auf 0 geklemmt");
        assert_eq!(score_of(f64::NAN), 0.0, "NaN → 0");
    }

    #[test]
    fn the_mapping_comes_from_the_settings_with_defaults_for_missing_fields() {
        let mapping = Mapping::from_config(&json!({ "host": "h" })).unwrap();
        assert_eq!(
            (mapping.schema.as_deref(), mapping.table.as_str(), mapping.text.as_str(), mapping.vector.as_str()),
            (None, "chunks", "content", "embedding"),
            "Vorgaben aus Vertrag §5"
        );
        assert_eq!(mapping.section, None, "Stelle: Vorgabe leer = keine Spalte");
        let mapping =
            Mapping::from_config(&json!({ "table": "rag.stuecke", "title_column": "titel", "model_column": "" }))
                .unwrap();
        assert_eq!((mapping.schema.as_deref(), mapping.table.as_str()), (Some("rag"), "stuecke"), "schema.tabelle");
        assert_eq!(mapping.title.as_deref(), Some("titel"), "Titelspalte");
        let roles: Vec<&str> = mapping.columns().iter().map(|(role, _)| *role).collect();
        assert_eq!(roles, ["text", "vector", "collection", "document", "title"], "nur vorhandene Spalten");
        assert!(Mapping::from_config(&json!({ "text_column": "x\"; DROP" })).is_err(), "kein SQL im Namen");
        assert!(Mapping::from_config(&json!({ "table": "" })).is_err(), "Tabelle Pflicht");
        assert_eq!(quoted("chunks"), "\"chunks\"", "doppelte Anführungszeichen");
    }

    #[test]
    fn identifiers_with_quotes_stay_one_identifier() {
        // Runde 19b: Ein Schema aus information_schema kann ein Anführungszeichen enthalten; verdoppelt bleibt es
        // Teil des Namens, statt den Bezeichner zu beenden (sonst ließe sich SQL anhängen).
        assert_eq!(quoted("we\"ird"), "\"we\"\"ird\"", "Anführungszeichen verdoppelt");
        assert_eq!(quoted("x\"; DROP TABLE y; --"), "\"x\"\"; DROP TABLE y; --\"", "kein Ausbruch aus dem Namen");
        let inner = quoted("a\"\"b");
        assert_eq!(inner.matches('"').count(), 6, "zwei Zeichen werden vier, dazu die beiden äußeren: {inner}");
    }

    #[test]
    fn the_tls_warning_needs_a_password_and_a_weak_mode() {
        // Vertrag R19b: Hinweis nur mit Passwort und prefer/disable (auch prefer als Vorgabe ohne Feld).
        assert!(tls_warning(&json!({ "tls": "prefer" }), true).is_some_and(|w| w.contains("„prefer“")), "prefer");
        assert!(tls_warning(&json!({ "tls": "disable" }), true).is_some_and(|w| w.contains("„disable“")), "disable");
        assert!(tls_warning(&json!({}), true).is_some(), "Vorgabe ist prefer, also mit Hinweis");
        // Begründung: sqlx prüft bei PgSslMode::Require das Zertifikat nicht (accept_invalid_certs außer bei
        // VerifyCa/VerifyFull) – verschlüsselt, aber nicht gegen einen falschen Server geschützt.
        let require =
            tls_warning(&json!({ "tls": "require" }), true).expect("require ohne Zertifikatsprüfung: Hinweis");
        assert!(require.contains("„require“") && require.contains("Zertifikat"), "nennt Modus und Grund: {require}");
        assert!(require.contains("„verify-full“"), "und den Ausweg: {require}");
        for weak in ["prefer", "disable", "require"] {
            let text = tls_warning(&json!({ "tls": weak }), true).unwrap();
            assert!(!text.contains("Besser „require“"), "{weak}: empfiehlt nicht mehr require: {text}");
        }
        assert_eq!(
            tls_warning(&json!({ "tls": "verify-full" }), true),
            None,
            "verify-full verlangt TLS und Zertifikat"
        );
        assert_eq!(tls_warning(&json!({ "tls": "disable" }), false), None, "ohne Passwort kein Hinweis");
    }
}
