// SPDX-License-Identifier: AGPL-3.0-or-later
//! Arguments and answers of the MultiGPT tools of the reconciliation (`list_documents`, `read_document`,
//! `upload_document`, `delete_document`), derived from the `inputSchema` like `search_arguments`
//! (contract Round 13, §2.4). The names are **assumptions** (unchecked against the real MultiGPT, see
//! `docs/Architektur.md`, section „Abgleich“): if a schema requires a parameter Baruch does not
//! know, that is honestly [`RagError::Schema`].

use baruch_rag::RagError;
use serde_json::{Map, Value, json};

use crate::connections::multigpt::{COLLECTION_KEYS, DOCUMENT_KEYS, clean_line, content_text, tool_payload, web_url};

/// Title of a document (uploading and listing).
pub(crate) const TITLE_KEYS: &[&str] = &["title", "name", "filename"];
/// Text of a document (uploading and reading).
pub(crate) const CONTENT_KEYS: &[&str] = &["content", "text", "body", "markdown"];
/// Continuation of a list.
pub(crate) const CURSOR_KEYS: &[&str] = &["cursor", "offset", "page"];
/// Kind of the content when uploading (if the schema asks for it).
pub(crate) const MIME_KEYS: &[&str] = &["mime_type", "content_type", "mimetype"];
/// Maximum number per page of `list_documents` (if the schema knows one).
pub(crate) const PAGE_LIMIT_KEYS: &[&str] = &["limit", "page_size", "per_page", "max_results"];
/// Document ID in answers (`upload_document`, `list_documents`).
pub(crate) const ID_KEYS: &[&str] = &["document_id", "doc_id", "documentId", "id"];
/// Modification time or content hash in `list_documents` (searched in this order).
pub(crate) const STAMP_KEYS: &[&str] = &[
    "updated_at",
    "modified_at",
    "updated",
    "modified",
    "last_modified",
    "lastModified",
    "content_hash",
    "hash",
    "sha256",
    "checksum",
    "etag",
    "version",
];
/// Lists in answers of `list_documents`.
const LIST_KEYS: &[&str] = &["documents", "items", "results", "data", "files"];
/// Continuation in answers of `list_documents`.
const NEXT_KEYS: &[&str] = &["next_cursor", "nextCursor", "next_page_token", "next"];
const URL_KEYS: &[&str] = &["url", "link", "source_url", "uri", "href"];

/// Parameters and required entries of an `inputSchema`.
pub(crate) struct Filler {
    tool: &'static str,
    props: Map<String, Value>,
    required: Vec<String>,
    args: Map<String, Value>,
}

/// May a parameter be a list?
fn allows_array(spec: &Value) -> bool {
    let typed = |s: &Value| match s.get("type") {
        Some(Value::String(t)) => t == "array",
        Some(Value::Array(types)) => types.iter().any(|t| t == "array"),
        _ => false,
    };
    typed(spec)
        || ["anyOf", "oneOf"].iter().filter_map(|k| spec.get(*k).and_then(Value::as_array)).any(|a| a.iter().any(typed))
}

/// Value matching the schema: a number if it requires `integer` and the text is one.
fn typed(spec: &Value, text: &str) -> Value {
    let integer = match spec.get("type") {
        Some(Value::String(t)) => t == "integer",
        Some(Value::Array(types)) => types.iter().any(|t| t == "integer") && !types.iter().any(|t| t == "string"),
        _ => false,
    };
    match text.trim().parse::<i64>() {
        Ok(n) if integer => json!(n),
        _ => json!(text),
    }
}

impl Filler {
    pub(crate) fn new(tool: &'static str, schema: &Value) -> Filler {
        let props = schema.get("properties").and_then(Value::as_object).cloned().unwrap_or_default();
        let required = schema
            .get("required")
            .and_then(Value::as_array)
            .map(|r| r.iter().filter_map(Value::as_str).map(str::to_owned).collect())
            .unwrap_or_default();
        Filler { tool, props, required, args: Map::new() }
    }

    /// First key from `keys` the schema knows; without `properties` `fallback` applies.
    fn key(&self, keys: &[&'static str], fallback: Option<&'static str>) -> Option<&'static str> {
        if self.props.is_empty() {
            return fallback;
        }
        keys.iter().find(|k| self.props.contains_key(**k)).copied()
    }

    /// Text under the first known key (a number if the schema requires `integer`). Without
    /// `properties` `fallback` applies. Returns whether the value was set.
    pub(crate) fn text(&mut self, keys: &[&'static str], fallback: Option<&'static str>, value: &str) -> bool {
        let Some(key) = self.key(keys, fallback) else { return false };
        let spec = self.props.get(key).cloned().unwrap_or(json!({}));
        self.args.insert(key.to_owned(), typed(&spec, value));
        true
    }

    /// Number under the first known key (clamped to `minimum`/`maximum`). Returns the value that was
    /// set (`None` = no key in the schema).
    pub(crate) fn number(&mut self, keys: &[&'static str], value: u64) -> Option<u64> {
        let key = self.key(keys, None)?;
        let spec = self.props.get(key).cloned().unwrap_or(json!({}));
        let mut n = value;
        if let Some(max) = spec.get("maximum").and_then(Value::as_u64) {
            n = n.min(max);
        }
        if let Some(min) = spec.get("minimum").and_then(Value::as_u64) {
            n = n.max(min);
        }
        self.args.insert(key.to_owned(), json!(n));
        Some(n)
    }

    /// Collection: as a list if the schema allows one (`collections: [c]`), otherwise as text. Without
    /// `properties` `collection` applies.
    pub(crate) fn collection(&mut self, name: &str) -> bool {
        let Some(key) = self.key(COLLECTION_KEYS, Some("collection")) else { return false };
        let spec = self.props.get(key).cloned().unwrap_or(json!({}));
        let value = if allows_array(&spec) {
            json!([typed(&spec.get("items").cloned().unwrap_or(json!({})), name)])
        } else {
            typed(&spec, name)
        };
        self.args.insert(key.to_owned(), value);
        true
    }

    /// Document ID (keys as in `document_arguments`, without `properties`: `document_id`).
    pub(crate) fn document(&mut self, id: &str) -> bool {
        self.text(DOCUMENT_KEYS, Some("document_id"), id)
    }

    /// Finished arguments; a required entry Baruch does not fill → [`RagError::Schema`].
    pub(crate) fn finish(self) -> Result<Value, RagError> {
        if let Some(unknown) = self.required.iter().find(|r| !self.args.contains_key(*r)) {
            return Err(RagError::Schema(format!(
                "{} verlangt den Parameter „{unknown}“, den Baruch nicht kennt.",
                self.tool
            )));
        }
        Ok(Value::Object(self.args))
    }
}

/// Text (or number as text) under one of the keys.
fn text_in(map: &Map<String, Value>, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|k| match map.get(*k) {
        Some(Value::String(s)) if !s.trim().is_empty() => Some(s.trim().to_owned()),
        Some(Value::Number(n)) => Some(n.to_string()),
        _ => None,
    })
}

/// Objects in which an entry can stand: the data themselves and `document`, `result`, `data`, `metadata`.
fn places(payload: &Value) -> Vec<&Map<String, Value>> {
    let Some(map) = payload.as_object() else { return Vec::new() };
    let mut out = vec![map];
    for nested in ["document", "result", "data", "metadata"] {
        if let Some(Value::Object(inner)) = map.get(nested) {
            out.push(inner);
        }
    }
    out
}

/// Document ID from the answer of `upload_document`: from the data (`document_id`, `id` …, also under
/// `document`/`result`/`data`), else `None`.
pub(crate) fn uploaded_id(result: &Value) -> Option<String> {
    let payload = tool_payload(result)?;
    places(&payload).into_iter().find_map(|m| text_in(m, ID_KEYS)).map(|id| clean_line(&id, 500))
}

/// A document from `list_documents`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Listed {
    pub id: String,
    pub title: String,
    pub url: String,
    /// `<key>=<value>` of the first reported modification time or of the hash, else empty.
    pub stamp: String,
}

/// One page of `list_documents`: documents and the continuation (`next_cursor` …), if reported,
/// as well as `has_more`. No list → [`RagError::Schema`].
pub(crate) fn listed(result: &Value) -> Result<(Vec<Listed>, Option<String>, bool), RagError> {
    let payload = tool_payload(result).ok_or_else(|| {
        RagError::Schema("list_documents: Antwort nicht lesbar (weder structuredContent noch JSON in content).".into())
    })?;
    let list = match &payload {
        Value::Array(list) => Some(list),
        Value::Object(map) => LIST_KEYS.iter().find_map(|k| map.get(*k).and_then(Value::as_array)).or_else(|| {
            map.values().filter_map(Value::as_object).find_map(|m| LIST_KEYS.iter().find_map(|k| m.get(*k)?.as_array()))
        }),
        _ => None,
    }
    .ok_or_else(|| {
        RagError::Schema("list_documents: keine Dokumentliste in der Antwort (documents, items …).".into())
    })?;
    let mut out = Vec::new();
    for item in list {
        let (id, title, url, stamp) = match item {
            Value::String(id) => (id.trim().to_owned(), String::new(), None, String::new()),
            Value::Object(map) => {
                let id = text_in(map, ID_KEYS).unwrap_or_default();
                let title = text_in(map, &["title", "document_title", "name", "filename"]).unwrap_or_default();
                let url = web_url(text_in(map, URL_KEYS));
                let stamp =
                    STAMP_KEYS.iter().find_map(|k| text_in(map, &[k]).map(|v| format!("{k}={v}"))).unwrap_or_default();
                (id, title, url, stamp)
            }
            _ => continue,
        };
        if id.is_empty() {
            continue;
        }
        let id = clean_line(&id, 500);
        out.push(Listed {
            title: clean_line(if title.is_empty() { &id } else { &title }, 300),
            id,
            url: url.unwrap_or_default(),
            stamp: clean_line(&stamp, 200),
        });
    }
    if out.is_empty() && !list.is_empty() {
        return Err(RagError::Schema(format!(
            "list_documents: {} Einträge ohne Dokument-ID; Format nicht lesbar.",
            list.len()
        )));
    }
    let map = payload.as_object();
    let next = map.and_then(|m| text_in(m, NEXT_KEYS));
    let more = map.and_then(|m| m.get("has_more").or_else(|| m.get("hasMore"))).and_then(Value::as_bool) == Some(true);
    Ok((out, next, more))
}

/// Text from the answer of `read_document`: `content`/`text`/`body`/`markdown` of the data, otherwise the text
/// in `content[]`; `\r\n` → `\n`.
pub(crate) fn document_text(result: &Value) -> String {
    let from_payload = tool_payload(result).and_then(|p| {
        places(&p)
            .into_iter()
            .find_map(|m| CONTENT_KEYS.iter().find_map(|k| m.get(*k).and_then(Value::as_str).map(str::to_owned)))
    });
    from_payload.unwrap_or_else(|| content_text(result)).replace("\r\n", "\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arguments_follow_the_schema_and_unknown_required_parameters_are_honest() {
        let schema = json!({ "required": ["collection_name", "title", "content"], "properties": {
            "collection_name": { "type": "string" }, "title": { "type": "string" },
            "content": { "type": "string" }, "mime_type": { "type": "string" } } });
        let mut f = Filler::new("upload_document", &schema);
        assert!(
            f.collection("Baruch: Gartenbuch") && f.text(TITLE_KEYS, Some("title"), "1 Frühling"),
            "Sammlung und Titel haben je einen Schlüssel im Schema"
        );
        assert!(
            f.text(CONTENT_KEYS, Some("content"), "Text") && f.text(MIME_KEYS, None, "text/plain"),
            "Inhalt und freiwilliger mime_type haben je einen Schlüssel im Schema"
        );
        assert_eq!(
            f.finish().unwrap(),
            json!({ "collection_name": "Baruch: Gartenbuch", "title": "1 Frühling", "content": "Text",
                    "mime_type": "text/plain" }),
            "Namen aus dem Schema, Sammlung als Text"
        );
        let list = json!({ "properties": { "collections": { "type": "array", "items": { "type": "string" } } } });
        let mut f = Filler::new("list_documents", &list);
        f.collection("Bibeln");
        assert_eq!(f.finish().unwrap(), json!({ "collections": ["Bibeln"] }), "Liste, wenn das Schema eine erlaubt");
        let mut f = Filler::new("upload_document", &json!({}));
        f.collection("C");
        f.text(TITLE_KEYS, Some("title"), "T");
        f.text(MIME_KEYS, None, "text/plain");
        assert_eq!(f.finish().unwrap(), json!({ "collection": "C", "title": "T" }), "ohne properties: Vorgaben");
        let strange = json!({ "required": ["folder_uuid"], "properties": { "folder_uuid": {} } });
        let e = Filler::new("upload_document", &strange).finish().unwrap_err();
        assert_eq!(e.code(), "schema", "unbekannte Pflichtangabe → Schema: {e}");
        assert!(e.message().contains("folder_uuid"), "der Satz nennt den Parameter: {e}");
    }

    #[test]
    fn listings_read_ids_titles_stamps_and_the_continuation() {
        let result = json!({ "structuredContent": { "documents": [
            { "document_id": "d1", "title": "Psalm", "updated_at": "2026-11-01T10:00:00Z", "url": "https://x.example/1" },
            { "id": 7, "name": "Sprüche", "hash": "abc" },
            { "title": "ohne ID" },
            "d3"
        ], "next_cursor": "c-2" } });
        let (docs, next, more) = listed(&result).unwrap();
        assert_eq!(docs.len(), 3, "Einträge ohne ID fallen weg: {docs:?}");
        assert_eq!(docs[0].stamp, "updated_at=2026-11-01T10:00:00Z", "Änderungszeit mit Schlüssel");
        assert_eq!(docs[0].url, "https://x.example/1", "Adresse übernommen");
        assert_eq!((docs[1].id.as_str(), docs[1].title.as_str()), ("7", "Sprüche"), "Zahl als ID, name als Titel");
        assert_eq!(docs[1].stamp, "hash=abc", "Hash statt Zeit");
        assert_eq!((docs[2].id.as_str(), docs[2].title.as_str(), docs[2].stamp.as_str()), ("d3", "d3", ""), "nur ID");
        assert_eq!((next.as_deref(), more), (Some("c-2"), false), "Fortsetzung gelesen");
        let unreadable = json!({ "content": [{ "type": "text", "text": "kein JSON" }] });
        assert_eq!(listed(&unreadable).unwrap_err().code(), "schema", "unlesbar → Schema");
    }

    #[test]
    fn document_texts_and_upload_ids_come_from_data_or_plain_text() {
        let plain = json!({ "content": [{ "type": "text", "text": "Zeile eins\r\nZeile zwei" }] });
        assert_eq!(document_text(&plain), "Zeile eins\nZeile zwei", "Klartext, CRLF → LF");
        let structured = json!({ "structuredContent": { "document": { "content": "Inhalt" } } });
        assert_eq!(document_text(&structured), "Inhalt", "Text unter document.content");
        let uploaded = json!({ "structuredContent": { "result": { "document_id": "neu-1" } } });
        assert_eq!(uploaded_id(&uploaded).as_deref(), Some("neu-1"), "ID eine Ebene tiefer");
        assert_eq!(uploaded_id(&plain), None, "ohne Daten keine ID (ehrlich, nichts erfunden)");
    }
}
