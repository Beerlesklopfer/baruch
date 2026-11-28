// SPDX-License-Identifier: AGPL-3.0-or-later
//! Validation of the settings of a connection (`connection.set`, form „Speichern“).
//!
//! Allowed fields per kind (everything else → -32602):
//! - `eliezer`, `multigpt`: `url` (http/https, no user info, no secret in the query)
//! - `pgvector`: `host` (empty = not set up), `port` (1–65535), `database`, `user`,
//!   `tls` ∈ `disable` | `prefer` | `require` | `verify-full`; since Round 12 the mapping of the table
//!   with the embeddings ([`PG_MAPPING`]): `table` (`tabelle` or `schema.tabelle`), `text_column`,
//!   `vector_column`, `collection_column`, `document_column` (mandatory), `section_column`, `title_column`,
//!   `url_column`, `model_column` (empty = column does not exist) and `embedding_model` (model the
//!   data was embedded with). Identifiers only `^[A-Za-z_][A-Za-z0-9_]{0,62}$`; they get into SQL only
//!   after a comparison with `information_schema` and in double quotes
//!   (`connections::pgvector`). If these fields are missing in a stored setting, their
//!   default applies ([`effective`]); they are stored only when the human enters them.
//! - `lancedb`: `dir` (relative to the data folder or absolute), `embedding_model`
//! - Round 19b, task prefixes of the embedding ([`PREFIX_FIELDS`]): `lancedb` `query_prefix` and
//!   `document_prefix`, `pgvector` only `query_prefix` (its data is already embedded). Checked with
//!   [`baruch_rag::check_prefix`] (at most 64 characters, no control characters), **not** trimmed: the
//!   space at the end („search_query: “) belongs to the prefix. If the field is missing, “no prefix” applies.
//!
//! Fields whose name sounds like a secret (`password`, `token`, `key`, `secret` …) are always rejected by
//! the validation: secrets belong in the keyring (`connection.set_secret`). Given
//! fields replace the stored ones, missing ones stay (or come from the default).

use baruch_store::ConnectionKind;
use serde_json::{Map, Value};

use super::Fault;

/// Allowed fields per kind, in the order of the page.
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

/// Suggestion for the task prefixes of nomic-embed-text (Round 19b; only as a hint on the page, not a default:
/// without a prefix an existing index stays valid).
pub const NOMIC_PREFIXES: (&str, &str) = ("search_query: ", "search_document: ");

/// Mapping of table and columns for pgvector (Round 12): if they are missing in the stored setting,
/// their default from [`ConnectionKind::default_config`] applies ([`effective`]), without storing them.
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

/// Columns that must exist (the others may be empty = not present).
pub const PG_REQUIRED_COLUMNS: [&str; 4] = ["text_column", "vector_column", "collection_column", "document_column"];

/// An identifier for PostgreSQL: `^[A-Za-z_][A-Za-z0-9_]{0,62}$` (no quotes, no
/// dot; at most 63 characters like `NAMEDATALEN - 1`).
pub fn is_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else { return false };
    (first.is_ascii_alphabetic() || first == '_')
        && name.len() <= 63
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Table `tabelle` or `schema.tabelle` → (schema, table); otherwise `None`.
pub fn split_table(table: &str) -> Option<(Option<&str>, &str)> {
    match table.split_once('.') {
        None => is_identifier(table).then_some((None, table)),
        Some((schema, name)) => (is_identifier(schema) && is_identifier(name)).then_some((Some(schema), name)),
    }
}

/// Stored setting over the default: fields that are not (yet) stored come from
/// [`ConnectionKind::default_config`] (for the mapping of pgvector from Round 12 and for the page).
pub fn effective(kind: ConnectionKind, config: &Value) -> Value {
    let mut out = kind.default_config();
    if let (Some(out), Some(given)) = (out.as_object_mut(), config.as_object()) {
        for (name, value) in given {
            out.insert(name.clone(), value.clone());
        }
    }
    out
}

/// TLS modes for pgvector.
pub const TLS_MODES: [&str; 4] = ["disable", "prefer", "require", "verify-full"];

/// Does the name sound like a secret?
pub fn secret_like(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    ["pass", "token", "secret", "geheim", "key", "credential", "auth", "bearer"].iter().any(|word| name.contains(word))
}

fn invalid(code: &'static str, message: impl Into<String>) -> Fault {
    Fault::Invalid(code, message.into())
}

/// Check an address: http/https, host name, no user info, no secret in the query, one
/// line, at most 2000 characters. Returns the trimmed input.
pub fn check_url(input: &str) -> Result<String, Fault> {
    let text = input.trim();
    if text.is_empty() || text.chars().count() > 2000 || text.chars().any(char::is_whitespace) {
        return Err(invalid("url", crate::i18n::tc("connections_msg.field.url", &[])));
    }
    let url = reqwest::Url::parse(text)
        .map_err(|_| invalid("url", crate::i18n::tc("connections_msg.field.url_invalid", &[])))?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(invalid("url_schema", crate::i18n::tc("connections_msg.field.url_scheme", &[])));
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(invalid("url_benutzer", crate::i18n::tc("connections_msg.field.url_user", &[])));
    }
    if url.host_str().is_none_or(str::is_empty) {
        return Err(invalid("url", crate::i18n::tc("connections_msg.field.url_host", &[])));
    }
    if url.query_pairs().any(|(name, _)| secret_like(&name)) {
        return Err(invalid("url_geheimnis", crate::i18n::tc("connections_msg.field.url_secret", &[])));
    }
    Ok(text.to_owned())
}

fn text<'a>(name: &str, value: &'a Value) -> Result<&'a str, Fault> {
    value
        .as_str()
        .ok_or_else(|| invalid("feld", crate::i18n::tc("connections_msg.field.text", &[("name", name.into())])))
}

fn one_line(value: &str, max: usize) -> bool {
    value.chars().count() <= max && !value.chars().any(char::is_control)
}

/// Check a field and bring it into normal form.
fn check_field(name: &str, value: &Value) -> Result<Value, Fault> {
    Ok(match name {
        "url" => Value::from(check_url(text(name, value)?)?),
        "host" => {
            let host = text(name, value)?.trim();
            let bad = !one_line(host, 253) || host.contains(|c: char| c.is_whitespace() || "/@?#".contains(c));
            if bad {
                return Err(invalid("host", crate::i18n::tc("connections_msg.field.host", &[])));
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
                _ => return Err(invalid("port", crate::i18n::tc("connections_msg.field.port", &[]))),
            }
        }
        "database" | "user" => {
            let value = text(name, value)?.trim();
            if !one_line(value, 63) {
                return Err(invalid("feld", crate::i18n::tc("connections_msg.field.short", &[("name", name.into())])));
            }
            Value::from(value)
        }
        "tls" => {
            let mode = text(name, value)?.trim();
            if !TLS_MODES.contains(&mode) {
                return Err(invalid("tls", crate::i18n::tc("connections_msg.field.tls", &[])));
            }
            Value::from(mode)
        }
        "dir" => {
            let dir = text(name, value)?.trim();
            if dir.is_empty() || !one_line(dir, 4096) {
                return Err(invalid("ordner", crate::i18n::tc("connections_msg.field.dir", &[])));
            }
            Value::from(dir)
        }
        "table" => {
            let table = text(name, value)?.trim();
            if split_table(table).is_none() {
                return Err(invalid("tabelle", crate::i18n::tc("connections_msg.field.table", &[])));
            }
            Value::from(table)
        }
        "text_column" | "vector_column" | "collection_column" | "document_column" | "section_column"
        | "title_column" | "url_column" | "model_column" => {
            let column = text(name, value)?.trim();
            let optional = !PG_REQUIRED_COLUMNS.contains(&name);
            if !(is_identifier(column) || optional && column.is_empty()) {
                let key =
                    if optional { "connections_msg.field.column_optional" } else { "connections_msg.field.column" };
                return Err(invalid("spalte", crate::i18n::tc(key, &[("name", name.into())])));
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
        // Round 19b: not trimmed – the space at the end belongs to the prefix.
        "query_prefix" | "document_prefix" => {
            let prefix = text(name, value)?;
            let checked = baruch_rag::check_prefix(name, prefix).map_err(|e| invalid("praefix", e.message()))?;
            Value::from(checked)
        }
        _ => return Err(invalid("feld", format!("unbekanntes Feld {name}"))),
    })
}

/// Target of a connection to which its secret is bound: for Eliezer and MultiGPT the origin of the
/// address (scheme, host, port – another path on the same server is the same target), for pgvector
/// host, port and TLS mode (a weaker TLS would send the password differently over the network);
/// LanceDB has no secret (`None`). Never contains a secret.
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

/// Field names of the task prefixes (Round 19b): LanceDB both, pgvector only `query_prefix` (data already embedded).
pub const PREFIX_FIELDS: [&str; 2] = ["query_prefix", "document_prefix"];

/// Task prefixes from a stored setting: Lancedb → query_prefix/document_prefix, Pgvector → only query_prefix
/// (document = ""), Eliezer/Multigpt → empty; missing or not a text = "".
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

/// Lay given fields over the stored ones (or the default) and check everything.
pub fn merged(kind: ConnectionKind, current: &Value, given: &Value) -> Result<Value, Fault> {
    let Some(given) = given.as_object() else {
        return Err(invalid("feld", crate::i18n::tc("connections_msg.field.config_object", &[])));
    };
    let allowed = fields(kind);
    for name in given.keys() {
        if secret_like(name) {
            return Err(invalid("geheimnis_feld", crate::i18n::tc("connections_msg.field.secret_in_config", &[])));
        }
        if !allowed.contains(&name.as_str()) {
            return Err(invalid("feld", format!("{kind}: unbekanntes Feld (erlaubt: {}).", allowed.join(", "))));
        }
    }
    let default = kind.default_config();
    let mut out = Map::new();
    for name in allowed {
        // The mapping of pgvector (Round 12) stays unstored until the human enters it; until
        // then its default applies through [`effective`]. So saving the old fields changes nothing about it.
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
        // Otherwise `merged` would reject the mapping as “secret in the settings”; in particular
        // `model_column` and `text_column` contain none of the secret words (pass, token, key, auth …).
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
        // Round 19b: LanceDB both, pgvector only the query prefix (its data is already embedded).
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
