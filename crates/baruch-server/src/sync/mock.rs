// SPDX-License-Identifier: AGPL-3.0-or-later
//! Nachgebautes MultiGPT **für Tests** des Abgleichs (auch die des Integrators): ein kleiner MCP-Server
//! über Streamable HTTP auf `127.0.0.1:<zufällig>/mcp` mit `initialize`, `notifications/initialized`,
//! `tools/list` und `tools/call` für `list_collections`, `list_documents`, `read_document`,
//! `document_info`, `upload_document` und `delete_document`; HTTP `DELETE /mcp` (Ende der Sitzung) → 200.
//! Jede Anfrage landet in [`MockDocs::requests`]. Mit [`MockDocs::deny_write`] fehlt dem Schlüssel das Recht
//! `docs.write`: Hochladen und Löschen antworten mit HTTP 403 bzw. `isError`. Seit Runde 19a auch: Hochladen
//! ohne Dokument-ID, `isError` bei leerem Inhalt, still gekappte Listen und verlorene Antworten. Kein Netz außer Loopback,
//! frei erfundene Testdaten. Namen und Formen der Werkzeuge sind die Annahmen aus `docs/Architektur.md`.

use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use axum::Router;
use axum::extract::State;
use axum::http::{HeaderMap, Method, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::any;
use serde_json::{Value, json};

/// Sitzung, die der Nachbau vergibt.
pub const SESSION: &str = "abgleich-sitzung-1";

/// Wie der Nachbau ein fehlendes `docs.write` meldet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Deny {
    /// HTTP 403 auf den Aufruf.
    Forbidden,
    /// `isError: true` mit Text.
    ToolError,
}

/// Ein Dokument im Nachbau.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MockDocument {
    pub id: String,
    pub title: String,
    pub text: String,
    pub url: Option<String>,
    /// Änderungszeit (steigt bei jeder Änderung).
    pub version: u64,
}

/// Verhalten des Nachbaus.
#[derive(Debug, Clone)]
pub struct Behavior {
    /// Dokumente je Sammlung (Sammlungen entstehen beim Hochladen von selbst).
    pub collections: BTreeMap<String, Vec<MockDocument>>,
    /// `list_documents` meldet `updated_at` (sonst nur ID und Titel).
    pub stamps: bool,
    /// Einträge je Seite von `list_documents` (0 = alle).
    pub page_size: usize,
    pub deny: Option<Deny>,
    /// Werkzeuge für `tools/list`.
    pub tools: Vec<Value>,
    /// Wartezeit vor jeder Antwort auf `tools/call`.
    pub delay: std::time::Duration,
    /// `delete_document` scheitert (`isError`), Hochladen gelingt weiter.
    pub fail_delete: bool,
    /// Größer als 0: `read_document` mit `page` liefert Seiten zu so vielen Zeichen als `{text, page, pages, has_more}`
    /// (das Schema muss dazu `page` nennen, siehe [`paged_tools`]).
    pub page_chars: usize,
    /// `upload_document` legt das Dokument an, nennt aber keine Dokument-ID (Runde 19a).
    pub upload_without_id: bool,
    /// `upload_document` mit leerem Inhalt (nur Überschriften bzw. Leerzeilen) → `isError` „Leerer Inhalt …“ (nur
    /// dieses Dokument, Runde 19a).
    pub reject_empty: bool,
    /// Größer als 0: `list_documents` liefert höchstens so viele Einträge und meldet nie eine Fortsetzung
    /// (ein Server, der `limit` still kappt; Runde 19a).
    pub silent_cap: usize,
    /// `upload_document` legt das Dokument an, die Antwort geht aber verloren (HTTP 502; Runde 19a).
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

/// Die sechs Werkzeuge mit den angenommenen Schemas.
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

/// Wie [`default_tools`], `read_document` mit dem Seitenparameter `page` (ganze Zahl ab 1).
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

/// Laufender Nachbau; endet mit dem Fallenlassen.
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
    /// Auf `127.0.0.1:0` starten.
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

    /// Dokument anlegen bzw. ersetzen (neue Änderungszeit).
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

    /// Adresse eines Dokuments setzen.
    pub fn set_url(&self, id: &str, url: &str) {
        self.configure(|b| {
            for doc in b.collections.values_mut().flatten().filter(|d| d.id == id) {
                doc.url = Some(url.to_owned());
            }
        });
    }

    /// Dokument entfernen (wie in MultiGPT gelöscht).
    pub fn remove_document(&self, id: &str) {
        self.configure(|b| b.collections.values_mut().for_each(|list| list.retain(|d| d.id != id)));
    }

    /// Dokumente einer Sammlung.
    pub fn documents(&self, collection: &str) -> Vec<MockDocument> {
        self.shared.behavior.lock().expect("Nachbau").collections.get(collection).cloned().unwrap_or_default()
    }

    /// `docs.write` fehlt (`Some`) bzw. ist da (`None`).
    pub fn deny_write(&self, deny: Option<Deny>) {
        self.configure(|b| b.deny = deny);
    }

    /// Alle Anfragen: `{http, method, params, session}` (`http` = HTTP-Methode, `method` = JSON-RPC-Methode).
    pub fn requests(&self) -> Vec<Value> {
        self.shared.requests.lock().expect("Nachbau").clone()
    }

    /// Argumente aller Aufrufe eines Werkzeugs.
    pub fn tool_calls(&self, name: &str) -> Vec<Value> {
        self.requests()
            .into_iter()
            .filter(|r| r["method"] == "tools/call" && r["params"]["name"] == name)
            .map(|r| r["params"]["arguments"].clone())
            .collect()
    }

    /// Anfragen vergessen (für Zählungen je Lauf).
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

/// Ein Werkzeugaufruf; `Err(())` = HTTP 403.
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
            // Volltext als Klartext (wie das echte read_document vermutlich): kein JSON.
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
                "error": { "code": -32601, "message": format!("unbekannte Methode {other}") } });
            return axum::Json(error).into_response();
        }
    };
    answer(result).into_response()
}
