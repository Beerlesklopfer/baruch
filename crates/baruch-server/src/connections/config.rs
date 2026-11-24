// SPDX-License-Identifier: AGPL-3.0-or-later
//! Prüfung der Einstellungen einer Verbindung (`connection.set`, Formular „Speichern“).
//!
//! Erlaubte Felder je Art (alles andere → -32602):
//! - `eliezer`, `multigpt`: `url` (http/https, ohne Benutzerinfo, ohne Geheimnis in der Abfrage)
//! - `pgvector`: `host` (leer = nicht eingerichtet), `port` (1–65535), `database`, `user`,
//!   `tls` ∈ `disable` | `prefer` | `require` | `verify-full`; seit Runde 12 die Zuordnung der Tabelle
//!   mit den Einbettungen ([`PG_MAPPING`]): `table` (`tabelle` oder `schema.tabelle`), `text_column`,
//!   `vector_column`, `collection_column`, `document_column` (Pflicht), `section_column`, `title_column`,
//!   `url_column`, `model_column` (leer = Spalte gibt es nicht) und `embedding_model` (Modell, mit dem die
//!   Daten eingebettet sind). Bezeichner nur `^[A-Za-z_][A-Za-z0-9_]{0,62}$`; in SQL kommen sie erst
//!   nach Abgleich mit `information_schema` und in doppelten Anführungszeichen
//!   (`connections::pgvector`). Fehlen diese Felder in einer gespeicherten Einstellung, gilt ihre
//!   Vorgabe ([`effective`]); sie werden erst gespeichert, wenn der Mensch sie einträgt.
//! - `lancedb`: `dir` (relativ zum Datenordner oder absolut), `embedding_model`
//! - Runde 19b, Aufgaben-Präfixe der Einbettung ([`PREFIX_FIELDS`]): `lancedb` `query_prefix` und
//!   `document_prefix`, `pgvector` nur `query_prefix` (seine Daten sind schon eingebettet). Geprüft mit
//!   [`baruch_rag::check_prefix`] (höchstens 64 Zeichen, keine Steuerzeichen), **nicht** beschnitten: Das
//!   Leerzeichen am Ende („search_query: “) gehört zum Präfix. Fehlt das Feld, gilt „kein Präfix“.
//!
//! Felder, deren Name nach einem Geheimnis klingt (`password`, `token`, `key`, `secret` …), lehnt die
//! Prüfung immer ab: Geheimnisse gehören in den Schlüsselbund (`connection.set_secret`). Gegebene
//! Felder ersetzen die gespeicherten, fehlende bleiben (bzw. kommen aus der Vorgabe).

use baruch_store::ConnectionKind;
use serde_json::{Map, Value};

use super::Fault;

/// Erlaubte Felder je Art, in der Reihenfolge der Seite.
pub fn fields(kind: ConnectionKind) -> &'static [&'static str] {
    match kind {
        ConnectionKind::Eliezer | ConnectionKind::Multigpt => &["url"],
        ConnectionKind::Pgvector => &[
            "host",
            "port",
            "database",
            "user",
            "tls",
            "table",
            "text_column",
            "vector_column",
            "collection_column",
            "document_column",
            "section_column",
            "title_column",
            "url_column",
            "model_column",
            "embedding_model",
            "query_prefix",
        ],
        ConnectionKind::Lancedb => &["dir", "embedding_model", "query_prefix", "document_prefix"],
    }
}

/// Vorschlag für die Aufgaben-Präfixe von nomic-embed-text (Runde 19b; nur als Hinweis auf der Seite, keine Vorgabe:
/// ohne Präfix bleibt ein bestehender Index gültig).
pub const NOMIC_PREFIXES: (&str, &str) = ("search_query: ", "search_document: ");

/// Zuordnung von Tabelle und Spalten bei pgvector (Runde 12): fehlen sie in der gespeicherten Einstellung,
/// gilt ihre Vorgabe aus [`ConnectionKind::default_config`] ([`effective`]), ohne sie mitzuspeichern.
pub const PG_MAPPING: [&str; 10] = [
    "table",
    "text_column",
    "vector_column",
    "collection_column",
    "document_column",
    "section_column",
    "title_column",
    "url_column",
    "model_column",
    "embedding_model",
];

/// Spalten, die es geben muss (die übrigen dürfen leer sein = nicht vorhanden).
pub const PG_REQUIRED_COLUMNS: [&str; 4] = ["text_column", "vector_column", "collection_column", "document_column"];

/// Ein Bezeichner für PostgreSQL: `^[A-Za-z_][A-Za-z0-9_]{0,62}$` (ohne Anführungszeichen, ohne
/// Punkt; höchstens 63 Zeichen wie `NAMEDATALEN - 1`).
pub fn is_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else { return false };
    (first.is_ascii_alphabetic() || first == '_')
        && name.len() <= 63
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Tabelle `tabelle` oder `schema.tabelle` → (Schema, Tabelle); sonst `None`.
pub fn split_table(table: &str) -> Option<(Option<&str>, &str)> {
    match table.split_once('.') {
        None => is_identifier(table).then_some((None, table)),
        Some((schema, name)) => (is_identifier(schema) && is_identifier(name)).then_some((Some(schema), name)),
    }
}

/// Gespeicherte Einstellung über der Vorgabe: Felder, die (noch) nicht gespeichert sind, kommen aus
/// [`ConnectionKind::default_config`] (für die Zuordnung von pgvector aus Runde 12 und für die Seite).
pub fn effective(kind: ConnectionKind, config: &Value) -> Value {
    let mut out = kind.default_config();
    if let (Some(out), Some(given)) = (out.as_object_mut(), config.as_object()) {
        for (name, value) in given {
            out.insert(name.clone(), value.clone());
        }
    }
    out
}

/// TLS-Modi für pgvector.
pub const TLS_MODES: [&str; 4] = ["disable", "prefer", "require", "verify-full"];

/// Klingt der Name nach einem Geheimnis?
pub fn secret_like(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    ["pass", "token", "secret", "geheim", "key", "credential", "auth", "bearer"].iter().any(|word| name.contains(word))
}

fn invalid(code: &'static str, message: impl Into<String>) -> Fault {
    Fault::Invalid(code, message.into())
}

/// Adresse prüfen: http/https, Rechnername, keine Benutzerinfo, kein Geheimnis in der Abfrage, eine
/// Zeile, höchstens 2000 Zeichen. Liefert die beschnittene Eingabe.
pub fn check_url(input: &str) -> Result<String, Fault> {
    let text = input.trim();
    if text.is_empty() || text.chars().count() > 2000 || text.chars().any(char::is_whitespace) {
        return Err(invalid("url", "Adresse fehlt oder ist ungültig (eine Zeile, höchstens 2000 Zeichen)."));
    }
    let url = reqwest::Url::parse(text).map_err(|_| invalid("url", "Adresse ist ungültig."))?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(invalid("url_schema", "Adresse muss mit http:// oder https:// beginnen."));
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(invalid(
            "url_benutzer",
            "Adresse ohne Benutzer und Passwort (Geheimnisse gehören in den Schlüsselbund).",
        ));
    }
    if url.host_str().is_none_or(str::is_empty) {
        return Err(invalid("url", "Adresse ohne Rechnernamen."));
    }
    if url.query_pairs().any(|(name, _)| secret_like(&name)) {
        return Err(invalid(
            "url_geheimnis",
            "Adresse ohne Schlüssel oder Token in der Abfrage (dafür gibt es den Schlüsselbund).",
        ));
    }
    Ok(text.to_owned())
}

fn text<'a>(name: &str, value: &'a Value) -> Result<&'a str, Fault> {
    value.as_str().ok_or_else(|| invalid("feld", format!("{name}: Text erwartet.")))
}

fn one_line(value: &str, max: usize) -> bool {
    value.chars().count() <= max && !value.chars().any(char::is_control)
}

/// Ein Feld prüfen und in Normalform bringen.
fn check_field(name: &str, value: &Value) -> Result<Value, Fault> {
    Ok(match name {
        "url" => Value::from(check_url(text(name, value)?)?),
        "host" => {
            let host = text(name, value)?.trim();
            let bad = !one_line(host, 253) || host.contains(|c: char| c.is_whitespace() || "/@?#".contains(c));
            if bad {
                return Err(invalid("host", "Rechner: Name oder IP-Adresse ohne Leerzeichen, / oder @."));
            }
            Value::from(host)
        }
        "port" => {
            let port = match value {
                Value::Number(n) => n.as_u64(),
                Value::String(s) => s.trim().parse::<u64>().ok(),
                _ => None,
            };
            match port {
                Some(port @ 1..=65535) => Value::from(port),
                _ => return Err(invalid("port", "Port: ganze Zahl von 1 bis 65535.")),
            }
        }
        "database" | "user" => {
            let value = text(name, value)?.trim();
            if !one_line(value, 63) {
                return Err(invalid("feld", format!("{name}: höchstens 63 Zeichen in einer Zeile.")));
            }
            Value::from(value)
        }
        "tls" => {
            let mode = text(name, value)?.trim();
            if !TLS_MODES.contains(&mode) {
                return Err(invalid("tls", "TLS: disable, prefer, require oder verify-full."));
            }
            Value::from(mode)
        }
        "dir" => {
            let dir = text(name, value)?.trim();
            if dir.is_empty() || !one_line(dir, 4096) {
                return Err(invalid("ordner", "Ordner: nicht leer, eine Zeile."));
            }
            Value::from(dir)
        }
        "table" => {
            let table = text(name, value)?.trim();
            if split_table(table).is_none() {
                return Err(invalid(
                    "tabelle",
                    "Tabelle: Name aus Buchstaben, Ziffern und _ (nicht mit einer Ziffer beginnend, höchstens 63 Zeichen), wahlweise schema.tabelle.",
                ));
            }
            Value::from(table)
        }
        "text_column" | "vector_column" | "collection_column" | "document_column" | "section_column"
        | "title_column" | "url_column" | "model_column" => {
            let column = text(name, value)?.trim();
            let optional = !PG_REQUIRED_COLUMNS.contains(&name);
            if !(is_identifier(column) || optional && column.is_empty()) {
                return Err(invalid(
                    "spalte",
                    format!(
                        "{name}: Spaltenname aus Buchstaben, Ziffern und _ (nicht mit einer Ziffer beginnend, höchstens 63 Zeichen){}.",
                        if optional { "; leer = Spalte gibt es nicht" } else { "" }
                    ),
                ));
            }
            Value::from(column)
        }
        "embedding_model" => {
            let model = text(name, value)?.trim();
            if model.is_empty() || !one_line(model, 200) {
                return Err(invalid("modell", "Einbettungsmodell: 1 bis 200 Zeichen in einer Zeile."));
            }
            Value::from(model)
        }
        // Runde 19b: nicht beschnitten – das Leerzeichen am Ende gehört zum Präfix.
        "query_prefix" | "document_prefix" => {
            let prefix = text(name, value)?;
            let checked = baruch_rag::check_prefix(name, prefix).map_err(|e| invalid("praefix", e.message()))?;
            Value::from(checked)
        }
        _ => return Err(invalid("feld", format!("unbekanntes Feld {name}"))),
    })
}

/// Ziel einer Verbindung, an das ihr Geheimnis gebunden ist: bei Eliezer und MultiGPT der Ursprung der
/// Adresse (Schema, Rechner, Port – ein anderer Pfad beim selben Server ist dasselbe Ziel), bei pgvector
/// Rechner, Port und TLS-Modus (ein schwächeres TLS schickte das Passwort anders über das Netz);
/// LanceDB hat kein Geheimnis (`None`). Enthält nie ein Geheimnis.
pub fn target(kind: ConnectionKind, config: &Value) -> Option<String> {
    let field = |name: &str| match config.get(name) {
        Some(Value::String(s)) => s.trim().to_owned(),
        Some(Value::Number(n)) => n.to_string(),
        _ => String::new(),
    };
    match kind {
        ConnectionKind::Eliezer | ConnectionKind::Multigpt => {
            let url = field("url");
            Some(reqwest::Url::parse(&url).map_or(url, |u| u.origin().ascii_serialization()))
        }
        ConnectionKind::Pgvector => {
            Some(format!("postgres://{}:{} (TLS {})", field("host"), field("port"), field("tls")))
        }
        ConnectionKind::Lancedb => None,
    }
}

/// Feldnamen der Aufgaben-Präfixe (Runde 19b): LanceDB beide, pgvector nur `query_prefix` (seine Daten sind schon eingebettet).
pub const PREFIX_FIELDS: [&str; 2] = ["query_prefix", "document_prefix"];

/// Aufgaben-Präfixe aus einer gespeicherten Einstellung: Lancedb → query_prefix/document_prefix, Pgvector → nur query_prefix (document = ""), Eliezer/Multigpt → leer; fehlend oder kein Text = "".
pub fn embed_prefixes(kind: ConnectionKind, config: &Value) -> baruch_rag::EmbedPrefixes {
    let text = |name: &str| match config.get(name) {
        Some(Value::String(s)) => s.clone(),
        _ => String::new(),
    };
    match kind {
        ConnectionKind::Lancedb => {
            baruch_rag::EmbedPrefixes { query: text(PREFIX_FIELDS[0]), document: text(PREFIX_FIELDS[1]) }
        }
        ConnectionKind::Pgvector => {
            baruch_rag::EmbedPrefixes { query: text(PREFIX_FIELDS[0]), document: String::new() }
        }
        ConnectionKind::Eliezer | ConnectionKind::Multigpt => baruch_rag::EmbedPrefixes::default(),
    }
}

/// Gegebene Felder über die gespeicherten (bzw. die Vorgabe) legen und alles prüfen.
pub fn merged(kind: ConnectionKind, current: &Value, given: &Value) -> Result<Value, Fault> {
    let Some(given) = given.as_object() else {
        return Err(invalid("feld", "config: JSON-Objekt erwartet."));
    };
    let allowed = fields(kind);
    for name in given.keys() {
        if secret_like(name) {
            return Err(invalid(
                "geheimnis_feld",
                "Geheimnisse gehören nicht in die Einstellungen, sondern in den Schlüsselbund (connection.set_secret).",
            ));
        }
        if !allowed.contains(&name.as_str()) {
            return Err(invalid("feld", format!("{kind}: unbekanntes Feld (erlaubt: {}).", allowed.join(", "))));
        }
    }
    let default = kind.default_config();
    let mut out = Map::new();
    for name in allowed {
        // Die Zuordnung von pgvector (Runde 12) bleibt ungespeichert, bis der Mensch sie einträgt; bis
        // dahin gilt ihre Vorgabe über [`effective`]. So ändert ein Speichern der alten Felder nichts daran.
        let from_default = !(kind == ConnectionKind::Pgvector && PG_MAPPING.contains(name));
        let value = given
            .get(*name)
            .or_else(|| current.get(*name))
            .or_else(|| if from_default { default.get(*name) } else { None });
        if let Some(value) = value {
            out.insert((*name).to_owned(), check_field(name, value)?);
        }
    }
    Ok(Value::Object(out))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn code(result: Result<impl std::fmt::Debug, Fault>) -> &'static str {
        match result {
            Err(Fault::Invalid(code, _)) => code,
            other => panic!("Ablehnung erwartet, nicht {other:?}"),
        }
    }

    #[test]
    fn urls_need_http_a_host_and_no_credentials() {
        assert_eq!(check_url(" http://127.0.0.1:1234 ").unwrap(), "http://127.0.0.1:1234", "beschnitten");
        assert_eq!(code(check_url("ftp://example.org/")), "url_schema", "nur http/https");
        assert_eq!(code(check_url("file:///etc/passwd")), "url_schema", "kein Dateizugriff");
        assert_eq!(code(check_url("https://ich:pw@example.org/")), "url_benutzer", "keine Benutzerinfo");
        assert_eq!(code(check_url("https://ich@example.org/")), "url_benutzer", "auch ohne Passwort");
        assert_eq!(code(check_url("https://example.org/?api_key=x")), "url_geheimnis", "kein Schlüssel in der Abfrage");
        assert_eq!(code(check_url("example.org")), "url", "ohne Schema keine Adresse");
        assert_eq!(code(check_url("https://exa mple.org")), "url", "kein Leerraum");
    }

    #[test]
    fn the_target_names_where_the_secret_goes_and_nothing_else() {
        let eliezer = |url: &str| target(ConnectionKind::Eliezer, &json!({ "url": url })).unwrap();
        assert_eq!(eliezer("http://127.0.0.1:1234/v1"), "http://127.0.0.1:1234", "Ursprung ohne Pfad");
        assert_eq!(eliezer("http://127.0.0.1:1234"), eliezer("http://127.0.0.1:1234/anders/"), "Pfad egal");
        assert_ne!(eliezer("http://127.0.0.1:1234"), eliezer("http://127.0.0.1:1235"), "anderer Port");
        assert_ne!(eliezer("https://a.example/"), eliezer("http://a.example/"), "anderes Schema");
        assert_ne!(eliezer("https://a.example/"), eliezer("https://b.example/"), "anderer Rechner");
        assert_eq!(eliezer("https://a.example/"), eliezer("https://a.example:443/"), "Vorgabe-Port gleich");
        let pg = |host: &str, port: u16, tls: &str| {
            let config = json!({ "host": host, "port": port, "database": "rag", "user": "u", "tls": tls });
            target(ConnectionKind::Pgvector, &config).unwrap()
        };
        assert_eq!(pg("db.example", 5432, "require"), "postgres://db.example:5432 (TLS require)", "Rechner, Port, TLS");
        assert_ne!(pg("db.example", 5432, "require"), pg("db.example", 5433, "require"), "anderer Port");
        assert_ne!(pg("db.example", 5432, "require"), pg("db.example", 5432, "disable"), "schwächeres TLS");
        assert_eq!(target(ConnectionKind::Lancedb, &json!({ "dir": "x" })), None, "LanceDB ohne Geheimnis");
    }

    #[test]
    fn merging_keeps_stored_fields_and_rejects_secrets_and_unknown_keys() {
        let kind = ConnectionKind::Pgvector;
        let current =
            json!({ "host": "db.example", "port": 5433, "database": "rag", "user": "baruch", "tls": "require" });
        let merged = merged(kind, &current, &json!({ "port": "6543" })).unwrap();
        assert_eq!(
            merged,
            json!({ "host": "db.example", "port": 6543, "database": "rag", "user": "baruch", "tls": "require" }),
            "nur der Port neu (als Zahl), der Rest bleibt"
        );
        assert_eq!(code(super::merged(kind, &current, &json!({ "password": "x" }))), "geheimnis_feld", "Passwort");
        assert_eq!(code(super::merged(kind, &current, &json!({ "api_token": "x" }))), "geheimnis_feld", "Token");
        assert_eq!(code(super::merged(kind, &current, &json!({ "schema": "x" }))), "feld", "unbekanntes Feld");
        assert_eq!(code(super::merged(kind, &current, &json!({ "port": 0 }))), "port", "Port 0");
        assert_eq!(code(super::merged(kind, &current, &json!({ "port": 70000 }))), "port", "Port zu groß");
        assert_eq!(code(super::merged(kind, &current, &json!({ "tls": "allow" }))), "tls", "nur vier TLS-Modi");
        assert_eq!(code(super::merged(kind, &current, &json!({ "host": "a b" }))), "host", "Rechner ohne Leerraum");
        assert_eq!(code(super::merged(kind, &current, &json!(["host"]))), "feld", "kein Objekt");
        let lance = super::merged(ConnectionKind::Lancedb, &json!({}), &json!({})).unwrap();
        assert_eq!(lance, ConnectionKind::Lancedb.default_config(), "leer: die Vorgabe");
        assert!(!secret_like("embedding_model") && !secret_like("database"), "harmlose Felder");
    }

    #[test]
    fn no_new_pgvector_field_sounds_like_a_secret() {
        // Sonst lehnte `merged` die Zuordnung als „Geheimnis in den Einstellungen“ ab; besonders
        // `model_column` und `text_column` enthalten keins der Geheimwörter (pass, token, key, auth …).
        for name in fields(ConnectionKind::Pgvector) {
            assert!(!secret_like(name), "{name} ist ein gewöhnliches Feld, kein Geheimnis");
        }
        for name in PG_MAPPING {
            assert!(fields(ConnectionKind::Pgvector).contains(&name), "{name} ist auf der Seite erlaubt");
        }
    }

    #[test]
    fn identifiers_follow_the_postgres_rule_and_tables_may_name_a_schema() {
        assert!(
            is_identifier("chunks") && is_identifier("_x9") && is_identifier("Embedding"),
            "Buchstaben, _, Ziffern"
        );
        assert!(is_identifier(&"a".repeat(63)), "63 Zeichen gehen (NAMEDATALEN - 1)");
        assert!(!is_identifier(&"a".repeat(64)), "64 Zeichen nicht");
        for bad in ["", "9chunks", "chunks;", "a b", "\"x\"", "x-y", "ümlaut", "a.b"] {
            assert!(!is_identifier(bad), "{bad:?} ist kein erlaubter Bezeichner");
        }
        assert_eq!(split_table("rag.chunks"), Some((Some("rag"), "chunks")), "schema.tabelle");
        assert_eq!(split_table("chunks"), Some((None, "chunks")), "nur die Tabelle");
        assert_eq!(split_table("a.b.c"), None, "höchstens ein Punkt");
        assert_eq!(split_table(".chunks"), None, "leeres Schema");
        assert_eq!(split_table("x.chunks; DROP TABLE y"), None, "keine SQL-Reste");
    }

    #[test]
    fn the_pgvector_mapping_is_checked_and_empty_optional_columns_mean_absent() {
        let kind = ConnectionKind::Pgvector;
        let current = json!({ "host": "db.example", "port": 5432, "database": "rag", "user": "u", "tls": "prefer" });
        let given =
            json!({ "table": " rag.chunks ", "section_column": "", "title_column": "title", "model_column": "model" });
        let merged = merged(kind, &current, &given).unwrap();
        assert_eq!(merged["table"], "rag.chunks", "schema.tabelle beschnitten");
        assert_eq!(merged["section_column"], "", "leer = Spalte gibt es nicht");
        assert_eq!(merged["model_column"], "model", "Modellspalte gesetzt");
        assert!(merged.get("text_column").is_none(), "nicht eingetragene Zuordnung bleibt ungespeichert");
        assert_eq!(effective(kind, &merged)["text_column"], "content", "… und gilt mit ihrer Vorgabe");
        assert_eq!(effective(kind, &merged)["embedding_model"], "nomic-embed-text", "Vorgabe des Modells");
        assert_eq!(effective(kind, &merged)["table"], "rag.chunks", "Gespeichertes geht vor");
        for (field, value, code_wanted) in [
            ("table", "", "tabelle"),
            ("table", "chunks; DROP TABLE x", "tabelle"),
            ("table", "a.b.c", "tabelle"),
            ("text_column", "", "spalte"),
            ("vector_column", "", "spalte"),
            ("collection_column", "", "spalte"),
            ("document_column", "", "spalte"),
            ("text_column", "con tent", "spalte"),
            ("url_column", "\"url\"", "spalte"),
            ("model_column", "1model", "spalte"),
            ("embedding_model", "", "modell"),
        ] {
            let result = super::merged(kind, &current, &json!({ field: value }));
            assert_eq!(code(result), code_wanted, "{field} = {value:?} wird abgelehnt");
        }
        for field in ["section_column", "title_column", "url_column", "model_column"] {
            assert!(super::merged(kind, &current, &json!({ field: "" })).is_ok(), "{field} darf leer sein");
        }
        assert_eq!(
            code(super::merged(kind, &current, &json!({ "password": "x" }))),
            "geheimnis_feld",
            "weiter kein Passwort"
        );
        assert_eq!(
            code(super::merged(kind, &current, &json!({ "auth_column": "x" }))),
            "geheimnis_feld",
            "Geheimwort im Namen"
        );
        assert_eq!(
            target(kind, &merged),
            target(kind, &current),
            "die Zuordnung ändert das Ziel des Passworts nicht (Rechner, Port, TLS)"
        );
    }

    #[test]
    fn prefixes_are_allowed_per_way_kept_with_their_trailing_space_and_checked() {
        // Runde 19b: LanceDB beide, pgvector nur das Anfrage-Präfix (seine Daten sind schon eingebettet).
        let lance = merged(
            ConnectionKind::Lancedb,
            &json!({}),
            &json!({ "query_prefix": "search_query: ", "document_prefix": "search_document: " }),
        )
        .unwrap();
        assert_eq!(lance["query_prefix"], "search_query: ", "das Leerzeichen am Ende bleibt (nicht beschnitten)");
        assert_eq!(lance["document_prefix"], "search_document: ", "Dokument-Präfix bei LanceDB erlaubt");
        assert_eq!(
            embed_prefixes(ConnectionKind::Lancedb, &lance).document_text("Text"),
            "search_document: Text",
            "gespeichert, wie embed_prefixes es liest"
        );
        let pg = merged(ConnectionKind::Pgvector, &json!({}), &json!({ "query_prefix": "search_query: " })).unwrap();
        assert_eq!(pg["query_prefix"], "search_query: ", "Anfrage-Präfix bei pgvector erlaubt");
        assert_eq!(
            code(merged(ConnectionKind::Pgvector, &json!({}), &json!({ "document_prefix": "x" }))),
            "feld",
            "pgvector bettet keine Dokumente ein: document_prefix ist dort ein unbekanntes Feld"
        );
        assert_eq!(
            code(merged(ConnectionKind::Lancedb, &json!({}), &json!({ "query_prefix": "a".repeat(65) }))),
            "praefix",
            "höchstens 64 Zeichen (baruch_rag::MAX_PREFIX_CHARS)"
        );
        assert_eq!(
            code(merged(ConnectionKind::Lancedb, &json!({}), &json!({ "query_prefix": "a\nb" }))),
            "praefix",
            "kein Zeilenumbruch (Steuerzeichen)"
        );
        assert_eq!(
            code(merged(ConnectionKind::Eliezer, &json!({}), &json!({ "query_prefix": "x" }))),
            "feld",
            "Eliezer hat keine Präfix-Felder"
        );
        assert!(
            merged(ConnectionKind::Lancedb, &json!({}), &json!({})).unwrap().get("query_prefix").is_none(),
            "ohne Eingabe kein Präfix gespeichert (Vorgabe leer = Verhalten wie bisher)"
        );
        for name in PREFIX_FIELDS {
            assert!(!secret_like(name), "{name} klingt nicht nach einem Geheimnis");
        }
    }

    #[test]
    fn embed_prefixes_follow_the_kind_and_missing_or_non_text_fields_mean_none() {
        let config = json!({ "query_prefix": "search_query: ", "document_prefix": "search_document: " });
        let lance = embed_prefixes(ConnectionKind::Lancedb, &config);
        assert_eq!(lance.query, "search_query: ", "LanceDB liest das Anfrage-Präfix samt Leerzeichen am Ende");
        assert_eq!(lance.document, "search_document: ", "LanceDB liest auch das Dokument-Präfix");
        let pg = embed_prefixes(ConnectionKind::Pgvector, &config);
        assert_eq!(pg.query, "search_query: ", "pgvector liest das Anfrage-Präfix");
        assert_eq!(pg.document, "", "pgvector bettet keine Dokumente ein, also kein Dokument-Präfix");
        for kind in [ConnectionKind::Eliezer, ConnectionKind::Multigpt] {
            assert_eq!(
                embed_prefixes(kind, &config),
                baruch_rag::EmbedPrefixes::default(),
                "{kind:?} hat keine Präfixe"
            );
        }
        assert_eq!(
            embed_prefixes(ConnectionKind::Lancedb, &json!({ "query_prefix": 7 })),
            baruch_rag::EmbedPrefixes::default(),
            "fehlend oder kein Text = leer (Verhalten wie bisher)"
        );
    }
}
