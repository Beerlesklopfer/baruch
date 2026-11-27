// SPDX-License-Identifier: AGPL-3.0-or-later
//! Rebuilt MultiGPT **for tests** of the reconciliation (also those of the integrator): a small MCP server
//! over Streamable HTTP on `127.0.0.1:<random>/mcp` with `initialize`, `notifications/initialized`,
//! `tools/list` and `tools/call` for `list_collections`, `list_documents`, `read_document`,
//! `document_info`, `upload_document` and `delete_document`; HTTP `DELETE /mcp` (end of the session) → 200.
//! Every request lands in [`MockDocs::requests`]. With [`MockDocs::deny_write`] the key lacks the right
//! `docs.write`: uploading and deleting answer with HTTP 403 or `isError`. Since Round 19a also: uploading
//! without a document ID, `isError` for empty content, silently capped lists and lost answers. No network except
//! loopback, freely invented test data. Names and shapes of the tools are the assumptions from
//! `docs/Architektur.md`.

use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use axum::Router;
use axum::extract::State;
use axum::http::{HeaderMap, Method, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::any;
use serde_json::{Value, json};

/// Session the mock assigns.
pub const SESSION: &str = "abgleich-sitzung-1";

/// How the mock reports a missing `docs.write`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Deny {
    /// HTTP 403 on the call.
    Forbidden,
    /// `isError: true` with text.
    ToolError,
}

/// A document in the mock.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MockDocument {
    pub id: String,
    pub title: String,
    pub text: String,
    pub url: Option<String>,
    /// Modification time (rises with every change).
    pub version: u64,
}

/// Behaviour of the mock.
#[derive(Debug, Clone)]
pub struct Behavior {
    /// Documents per collection (collections come into being by themselves on uploading).
    pub collections: BTreeMap<String, Vec<MockDocument>>,
    /// `list_documents` reports `updated_at` (otherwise only ID and title).
    pub stamps: bool,
    /// Entries per page of `list_documents` (0 = all).
    pub page_size: usize,
    pub deny: Option<Deny>,
    /// Tools for `tools/list`.
    pub tools: Vec<Value>,
    /// Waiting time before every answer to `tools/call`.
    pub delay: std::time::Duration,
    /// `delete_document` fails (`isError`), uploading still succeeds.
    pub fail_delete: bool,
    /// Greater than 0: `read_document` with `page` delivers pages of this many characters as
    /// `{text, page, pages, has_more}` (the schema must name `page` for that, see [`paged_tools`]).
    pub page_chars: usize,
    /// `upload_document` creates the document but names no document ID (Round 19a).
    pub upload_without_id: bool,
    /// `upload_document` with empty content (only headings or empty lines) → `isError` „Leerer Inhalt …“ (only
    /// this document, Round 19a).
    pub reject_empty: bool,
    /// Greater than 0: `list_documents` delivers at most this many entries and never reports a continuation
    /// (a server that silently caps `limit`; Round 19a).
    pub silent_cap: usize,
    /// `upload_document` creates the document, but the answer is lost (HTTP 502; Round 19a).
    pub lose_upload_answer: bool,
    next_id: u64,
    clock: u64,
}

impl Default for Behavior {
    fn default() -> Self {
        Behavior {
            collections: BTreeMap::new(),
            stamps: true,
            page_size: 0,
            deny: None,
            tools: default_tools(),
            delay: std::time::Duration::ZERO,
            fail_delete: false,
            page_chars: 0,
            upload_without_id: false,
            reject_empty: false,
            silent_cap: 0,
            lose_upload_answer: false,
            next_id: 1,
            clock: 1,
        }
    }
}

/// The six tools with the assumed schemas.
pub fn default_tools() -> Vec<Value> {
    let id = json!({ "type": "object", "required": ["document_id"],
        "properties": { "document_id": { "type": "string" } } });
    vec![
        json!({ "name": "list_collections", "inputSchema": { "type": "object", "properties": {} } }),
        json!({ "name": "list_documents", "inputSchema": { "type": "object", "required": ["collection"],
            "properties": { "collection": { "type": "string" }, "cursor": { "type": "string" },
                "limit": { "type": "integer", "minimum": 1, "maximum": 1000 } } } }),
        json!({ "name": "read_document", "inputSchema": id }),
        json!({ "name": "document_info", "inputSchema": id }),
        json!({ "name": "upload_document", "inputSchema": { "type": "object",
            "required": ["collection", "title", "content"], "properties": {
                "collection": { "type": "string" }, "title": { "type": "string" },
                "content": { "type": "string" }, "mime_type": { "type": "string" } } } }),
        json!({ "name": "delete_document", "inputSchema": id }),
    ]
}

/// Like [`default_tools`], `read_document` with the page parameter `page` (integer from 1).
pub fn paged_tools() -> Vec<Value> {
    let mut tools = default_tools();
    for tool in tools.iter_mut().filter(|t| t["name"] == "read_document") {
        tool["inputSchema"]["properties"]["page"] = json!({ "type": "integer", "minimum": 1 });
    }
    tools
}

#[derive(Default)]
struct Shared {
    behavior: Mutex<Behavior>,
    requests: Mutex<Vec<Value>>,
}

/// Running mock; ends when dropped.
pub struct MockDocs {
    addr: SocketAddr,
    shared: Arc<Shared>,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for MockDocs {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl MockDocs {
    /// Start on `127.0.0.1:0`.
    pub async fn start() -> MockDocs {
        let shared = Arc::new(Shared::default());
        let app = Router::new().route("/mcp", any(handle)).with_state(shared.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("Nachbau: kein Port");
        let addr = listener.local_addr().expect("Nachbau: Adresse");
        let task = tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        MockDocs { addr, shared, task }
    }

    /// `http://127.0.0.1:<port>/mcp`
    pub fn url(&self) -> String {
        format!("http://{}/mcp", self.addr)
    }

    pub fn configure(&self, change: impl FnOnce(&mut Behavior)) {
        change(&mut self.shared.behavior.lock().expect("Nachbau"));
    }

    /// Create or replace a document (new modification time).
    pub fn put_document(&self, collection: &str, id: &str, title: &str, text: &str) {
        self.configure(|b| {
            b.clock += 1;
            let version = b.clock;
            let list = b.collections.entry(collection.to_owned()).or_default();
            let doc = MockDocument { id: id.into(), title: title.into(), text: text.into(), url: None, version };
            match list.iter_mut().find(|d| d.id == id) {
                Some(old) => *old = MockDocument { url: old.url.clone(), ..doc },
                None => list.push(doc),
            }
        });
    }

    /// Set the address of a document.
    pub fn set_url(&self, id: &str, url: &str) {
        self.configure(|b| {
            for doc in b.collections.values_mut().flatten().filter(|d| d.id == id) {
                doc.url = Some(url.to_owned());
            }
        });
    }

    /// Remove a document (as if deleted in MultiGPT).
    pub fn remove_document(&self, id: &str) {
        self.configure(|b| b.collections.values_mut().for_each(|list| list.retain(|d| d.id != id)));
    }

    /// Documents of a collection.
    pub fn documents(&self, collection: &str) -> Vec<MockDocument> {
        self.shared.behavior.lock().expect("Nachbau").collections.get(collection).cloned().unwrap_or_default()
    }

    /// `docs.write` is missing (`Some`) or is there (`None`).
    pub fn deny_write(&self, deny: Option<Deny>) {
        self.configure(|b| b.deny = deny);
    }

    /// All requests: `{http, method, params, session}` (`http` = HTTP method, `method` = JSON-RPC method).
    pub fn requests(&self) -> Vec<Value> {
        self.shared.requests.lock().expect("Nachbau").clone()
    }

    /// Arguments of all calls of a tool.
    pub fn tool_calls(&self, name: &str) -> Vec<Value> {
        self.requests()
            .into_iter()
            .filter(|r| r["method"] == "tools/call" && r["params"]["name"] == name)
            .map(|r| r["params"]["arguments"].clone())
            .collect()
    }

    /// Forget the requests (for counts per run).
    pub fn clear_requests(&self) {
        self.shared.requests.lock().expect("Nachbau").clear();
    }
}

fn tool_text(value: &Value) -> Value {
    json!({ "content": [{ "type": "text", "text": value.to_string() }], "structuredContent": value, "isError": false })
}

fn tool_error(text: &str) -> Value {
    json!({ "content": [{ "type": "text", "text": text }], "isError": true })
}

fn text_arg(args: &Value, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|k| match args.get(*k) {
        Some(Value::String(s)) => Some(s.clone()),
        Some(Value::Array(items)) => items.first().and_then(Value::as_str).map(str::to_owned),
        _ => None,
    })
}

fn find<'a>(behavior: &'a Behavior, id: &str) -> Option<(&'a String, &'a MockDocument)> {
    behavior.collections.iter().find_map(|(c, list)| list.iter().find(|d| d.id == id).map(|d| (c, d)))
}

/// A tool call; `Err(())` = HTTP 403.
fn call(behavior: &mut Behavior, name: &str, args: &Value) -> Result<Value, ()> {
    let writes = matches!(name, "upload_document" | "delete_document");
    match (writes, behavior.deny) {
        (true, Some(Deny::Forbidden)) => return Err(()),
        (true, Some(Deny::ToolError)) => {
            return Ok(tool_error("Fehlende Berechtigung: Der API-Schlüssel hat kein docs.write."));
        }
        _ => {}
    }
    let document = text_arg(args, &["document_id"]);
    Ok(match name {
        "list_collections" => {
            let list: Vec<Value> =
                behavior.collections.iter().map(|(c, docs)| json!({ "name": c, "count": docs.len() })).collect();
            tool_text(&json!({ "collections": list }))
        }
        "list_documents" => {
            let Some(collection) = text_arg(args, &["collection"]) else {
                return Ok(tool_error("collection fehlt"));
            };
            let docs = behavior.collections.get(&collection).cloned().unwrap_or_default();
            let start: usize =
                text_arg(args, &["cursor"]).and_then(|c| c.strip_prefix("c-")?.parse().ok()).unwrap_or(0);
            let mut size = if behavior.page_size == 0 { docs.len().max(1) } else { behavior.page_size };
            if behavior.silent_cap > 0 {
                size = size.min(behavior.silent_cap);
            }
            let page: Vec<Value> = docs
                .iter()
                .skip(start)
                .take(size)
                .map(|d| {
                    let mut item = json!({ "document_id": d.id, "title": d.title });
                    if let Some(url) = &d.url {
                        item["url"] = json!(url);
                    }
                    if behavior.stamps {
                        item["updated_at"] = json!(format!("2026-11-07T00:00:00.{:06}Z", d.version));
                    }
                    item
                })
                .collect();
            let mut out = json!({ "documents": page });
            if start + size < docs.len() && behavior.silent_cap == 0 {
                out["next_cursor"] = json!(format!("c-{}", start + size));
            }
            tool_text(&out)
        }
        "read_document" => match document.as_deref().and_then(|d| find(behavior, d)) {
            Some((_, doc)) if behavior.page_chars > 0 && args.get("page").is_some() => {
                let chars: Vec<char> = doc.text.chars().collect();
                let pages = chars.len().div_ceil(behavior.page_chars).max(1);
                let page = args["page"].as_u64().unwrap_or(1).max(1) as usize;
                if page > pages {
                    return Ok(tool_error("Seite gibt es nicht"));
                }
                let start = (page - 1) * behavior.page_chars;
                let text: String = chars[start..chars.len().min(start + behavior.page_chars)].iter().collect();
                tool_text(&json!({ "text": text, "page": page, "pages": pages, "has_more": page < pages }))
            }
            // Full text as plain text (as the real read_document presumably does): no JSON.
            Some((_, doc)) => json!({ "content": [{ "type": "text", "text": doc.text }], "isError": false }),
            None => tool_error("Dokument unbekannt"),
        },
        "document_info" => match document.as_deref().and_then(|d| find(behavior, d)) {
            Some((collection, doc)) => tool_text(
                &json!({ "document_id": doc.id, "title": doc.title, "collection": collection, "url": doc.url }),
            ),
            None => tool_error("Dokument unbekannt"),
        },
        "upload_document" => {
            let (Some(collection), Some(title), Some(text)) =
                (text_arg(args, &["collection"]), text_arg(args, &["title"]), text_arg(args, &["content"]))
            else {
                return Ok(tool_error("collection, title und content sind Pflicht"));
            };
            if behavior.reject_empty && text.lines().all(|l| l.trim().is_empty() || l.trim_start().starts_with('=')) {
                return Ok(tool_error("Leerer Inhalt lässt sich nicht indexieren."));
            }
            let id = format!("doc-{}", behavior.next_id);
            behavior.next_id += 1;
            behavior.clock += 1;
            let doc = MockDocument { id: id.clone(), title, text, url: None, version: behavior.clock };
            behavior.collections.entry(collection.clone()).or_default().push(doc);
            if behavior.upload_without_id {
                return Ok(tool_text(&json!({ "collection": collection, "status": "indexed" })));
            }
            tool_text(&json!({ "document_id": id, "collection": collection, "status": "indexed" }))
        }
        "delete_document" => {
            if behavior.fail_delete {
                return Ok(tool_error("Löschen gerade nicht möglich (Wartung)."));
            }
            let Some(id) = document else { return Ok(tool_error("document_id fehlt")) };
            if find(behavior, &id).is_none() {
                return Ok(tool_error("Dokument unbekannt"));
            }
            behavior.collections.values_mut().for_each(|list| list.retain(|d| d.id != id));
            tool_text(&json!({ "deleted": id }))
        }
        other => tool_error(&format!("Unbekanntes Werkzeug {other}")),
    })
}

async fn handle(State(shared): State<Arc<Shared>>, method: Method, headers: HeaderMap, body: String) -> Response {
    let message: Value = serde_json::from_str(&body).unwrap_or(Value::Null);
    let session = headers.get("mcp-session-id").and_then(|v| v.to_str().ok()).map(str::to_owned);
    shared.requests.lock().expect("Nachbau").push(json!({
        "http": method.as_str(), "method": message["method"], "params": message["params"], "session": session,
    }));
    if method == Method::DELETE {
        return StatusCode::OK.into_response();
    }
    if method != Method::POST {
        return StatusCode::METHOD_NOT_ALLOWED.into_response();
    }
    let rpc = message["method"].as_str().unwrap_or("").to_owned();
    let id = message.get("id").cloned();
    let answer = |result: Value| axum::Json(json!({ "jsonrpc": "2.0", "id": id, "result": result }));
    if rpc == "initialize" {
        let result = json!({ "protocolVersion": "2025-06-18", "capabilities": { "tools": {} },
            "serverInfo": { "name": "multigpt-abgleich-nachbau", "version": "0.1" } });
        let mut response = answer(result).into_response();
        response.headers_mut().insert("mcp-session-id", SESSION.parse().expect("Kopfzeile"));
        return response;
    }
    if session.as_deref() != Some(SESSION) {
        return (StatusCode::BAD_REQUEST, "Mcp-Session-Id fehlt").into_response();
    }
    if id.is_none() {
        return StatusCode::ACCEPTED.into_response();
    }
    let delay = shared.behavior.lock().expect("Nachbau").delay;
    if rpc == "tools/call" && !delay.is_zero() {
        tokio::time::sleep(delay).await;
    }
    let mut behavior = shared.behavior.lock().expect("Nachbau");
    let result = match rpc.as_str() {
        "tools/list" => json!({ "tools": behavior.tools }),
        "tools/call" => {
            let name = message["params"]["name"].as_str().unwrap_or("").to_owned();
            match call(&mut behavior, &name, &message["params"]["arguments"]) {
                Ok(_) if name == "upload_document" && behavior.lose_upload_answer => {
                    return (StatusCode::BAD_GATEWAY, "Antwort verloren").into_response();
                }
                Ok(result) => result,
                Err(()) => {
                    return (StatusCode::FORBIDDEN, [(header::WWW_AUTHENTICATE, "Bearer")], "docs.write fehlt")
                        .into_response();
                }
            }
        }
        other => {
            let error = json!({ "jsonrpc": "2.0", "id": id,
                "error": { "code": -32601, "message": format!("unknown method {other}") } });
            return axum::Json(error).into_response();
        }
    };
    answer(result).into_response()
}
