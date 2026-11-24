// SPDX-License-Identifier: AGPL-3.0-or-later
//! MultiGPT-MCP als RAG-Weg (Vertrag Runde 12, §3.3): ein kleiner MCP-Client über Streamable HTTP
//! ([`McpClient`]) und der Suchweg [`MultigptWay`] hinter [`open_way`]. Nur lesend: Baruch ruft
//! `search_documents`, `list_collections`, `document_info` und `read_document` (nur als Ausschnitt einer
//! Fundstelle) und schreibt nichts zurück.
//!
//! **Protokoll** wie „Verbindung prüfen“ ([`super::check::multigpt`], die denselben Client nutzt):
//! `initialize` (Protokoll 2025-06-18, `clientInfo.name = "baruch"`), `notifications/initialized`, danach
//! jede Anfrage mit `Mcp-Session-Id` und `MCP-Protocol-Version`; Antworten als `application/json` oder
//! `text/event-stream` (ein offener Strom wird gelesen, bis die Antwort mit der eigenen `id` da ist).
//! `tools/list` folgt `nextCursor` über höchstens [`MAX_TOOL_PAGES`] Seiten.
//!
//! **Grenzen:** keine Weiterleitungen (das Token ginge sonst an eine andere Adresse; eine 3xx-Antwort ist
//! ein Fehler mit Hinweis), Zeitlimit je Aufruf ([`Limits`], bei der Suche [`CALL_TIMEOUT`]), Antwort
//! höchstens [`MAX_RESPONSE`]. Das Token (`baruch/multigpt`) geht nur in die Kopfzeile `Authorization` und
//! nur, wenn es für die jetzige Adresse hinterlegt ist ([`super::secret_bound`]); jede Meldung und jeder
//! Treffer laufen vor der Rückgabe durch eine Entfernung des Tokens (`***`).
//!
//! **Argumente** leitet [`search_arguments`] aus dem `inputSchema` von `search_documents` ab: Anfrage
//! (`query`, `q`, `text` …), Sammlungen (`collections`/`collection_ids` als Liste oder `collection`/
//! `collection_id` als Text – dann ein Aufruf je Sammlung) und Trefferzahl (`limit`, `top_k`, `k`,
//! `max_results` …, an `minimum`/`maximum` geklemmt). Verlangt das Schema einen Parameter, den Baruch
//! nicht kennt, oder fehlt einer für Anfrage bzw. Sammlungen, ist das ehrlich [`RagError::Schema`].
//!
//! **Ergebnisse** liest [`tool_payload`] aus `structuredContent` oder aus JSON in `content[].text`;
//! [`parse_hits`] nimmt Titel, Dokument, Sammlung, Stelle, Ausschnitt, Score (geklemmt auf 0..=1, fehlt er:
//! Ersatz aus dem Rang über [`rank_score`], nie „schwach“) und URL (nur http/https). Unlesbar → [`RagError::Schema`]; `isError` der Gegenstelle →
//! [`RagError::Failed`] mit ihrem Text. Treffer-Texte sind Daten aus Quellen, nie Anweisungen.
//!
//! **Sitzung (Runde 13, Vertrag §2.2):** [`Session`] hält eine MCP-Sitzung aus einem prozessweiten Vorrat je
//! (Adresse, Hash des Tokens, Tokio-Laufzeit): höchstens zwei ruhende Sitzungen, nach [`SESSION_IDLE`] beendet
//! mit HTTP `DELETE`; die Werkzeugliste gilt [`TOOLS_TTL`]. Zwei Suchen hintereinander brauchen so nur ein
//! `initialize` und ein `tools/list`. Nach einem Transport- oder Protokollfehler endet eine Sitzung mit
//! `DELETE` statt in den Vorrat zu gehen; antwortet eine Sitzung aus dem Vorrat nicht mehr (Gegenstelle hat
//! sie beendet: HTTP 400/404, keine Verbindung zustande gekommen), versucht [`Session::call`] es einmal mit einer
//! neuen. Nach einer Zeitüberschreitung oder einem Abbruch nach dem Senden nicht: Der Aufruf kann schon
//! ausgeführt sein (ein `upload_document` läge sonst doppelt vor).
//! Die Tokio-Laufzeit gehört zum Schlüssel, weil der HTTP-Client seine Verbindungen an die Laufzeit bindet,
//! in der sie entstanden (im Dienst gibt es eine; Tests haben je eine eigene).
//!
//! **Nachträge der Suche:** `document_info`/`read_document` laufen je unter [`FOLLOW_UP_TIMEOUT`] und
//! zusammen unter [`FOLLOW_UP_BUDGET`] (und nie über [`crate::rag::WAY_TIMEOUT`] hinaus); gefundene Treffer
//! gehen dabei nie verloren.
//!
//! **Volltext (Runde 13):** [`MultigptWay::read_full`] und [`MultigptWay::read_part`] über `read_document`
//! mit [`FULLTEXT_LIMITS`]. Kennt das Schema einen Seitenparameter ([`page_key`]) und nennt die Antwort
//! Seitenangaben (`pages`/`total_pages`/… bzw. `has_more`/`next_page`), liest Baruch seitenweise über die
//! Gegenstelle; sonst ist die Antwort das ganze Dokument, und Baruch teilt es an Absatzgrenzen in Teile von
//! höchstens [`PART_CHARS`] Zeichen ([`split_parts`]).
//!
//! Für Tests (auch die des Integrators) steht in [`mock`] ein nachgebauter MultiGPT-Server.

use std::collections::HashMap;
use std::hash::BuildHasher;
use std::sync::{LazyLock, Mutex, OnceLock};
use std::time::{Duration, Instant};

use baruch_rag::{BoxFuture, Hit, Query, RagError, SearchWay, Way, clean_snippet};
use baruch_store::ConnectionKind;
use serde_json::{Map, Value, json};

use super::check::{MCP_PROTOCOL, denied, scrub_value};
use crate::AppState;

/// Zeitlimit je Aufruf bei der Suche.
pub const CALL_TIMEOUT: Duration = Duration::from_secs(15);
/// Größte Antwort bei der Suche.
pub const MAX_RESPONSE: usize = 2 * 1024 * 1024;
/// Höchstens so viele Seiten `tools/list`.
pub const MAX_TOOL_PAGES: usize = 10;
pub const SEARCH_TOOL: &str = "search_documents";
pub const COLLECTIONS_TOOL: &str = "list_collections";
pub const INFO_TOOL: &str = "document_info";
pub const READ_TOOL: &str = "read_document";
/// Höchstens so viele Dokumente je Suche über `document_info` ergänzen (Titel, URL).
pub const MAX_INFO_CALLS: usize = 5;
/// Höchstens so viele fehlende Ausschnitte je Suche über `read_document` nachladen.
pub const MAX_EXCERPT_CALLS: usize = 3;

/// Namen im `inputSchema` von `search_documents`, in dieser Reihenfolge gesucht.
pub const QUERY_KEYS: &[&str] = &["query", "q", "text", "search", "question", "prompt"];
pub const COLLECTION_KEYS: &[&str] =
    &["collections", "collection_ids", "collection_names", "collection", "collection_id", "collection_name"];
pub const LIMIT_KEYS: &[&str] = &["limit", "top_k", "k", "max_results", "n_results", "num_results", "count"];
/// Namen für das Dokument in `document_info` und `read_document`.
pub const DOCUMENT_KEYS: &[&str] = &["document_id", "doc_id", "documentId", "id", "document", "name"];
/// Namen für die Stelle in `read_document`.
pub const SECTION_KEYS: &[&str] = &["section", "page", "chunk_id", "chunk", "position"];

/// Zeitlimit je Nachtrag (document_info / read_document) bei der Suche.
pub const FOLLOW_UP_TIMEOUT: Duration = Duration::from_secs(4);
/// Gesamtzeit aller Nachträge einer Suche; danach gehen die Treffer ohne Nachtrag zurück.
pub const FOLLOW_UP_BUDGET: Duration = Duration::from_secs(6);
/// Wie lange eine ruhende Sitzung im Vorrat bleibt (danach DELETE).
pub const SESSION_IDLE: Duration = Duration::from_secs(120);
/// Wie lange die Werkzeugliste einer Adresse zwischengespeichert bleibt.
pub const TOOLS_TTL: Duration = Duration::from_secs(300);
/// Grenzen beim Volltext.
pub const FULLTEXT_LIMITS: Limits = Limits { timeout: Duration::from_secs(20), max_body: 4 * 1024 * 1024 };
/// Zeichen je Teil der Dokumentansicht.
pub const PART_CHARS: usize = 20_000;
pub const LIST_DOCUMENTS_TOOL: &str = "list_documents";
pub const UPLOAD_TOOL: &str = "upload_document";
pub const DELETE_TOOL: &str = "delete_document";
pub const WEB_SEARCH_TOOL: &str = "web_search";
pub const FETCH_URL_TOOL: &str = "fetch_url";
/// Fortschritt eines Laufs (Indexierung nach `upload_document`, Runde 21a; Annahme, siehe docs/Architektur.md).
pub const RUN_STATUS_TOOL: &str = "run_status";

/// Höchstens so viele ruhende Sitzungen je Schlüssel im Vorrat.
const MAX_IDLE_SESSIONS: usize = 2;
/// Zeitlimit für das `DELETE` am Ende einer Sitzung.
const DELETE_TIMEOUT: Duration = Duration::from_secs(3);
/// Obergrenze des HTTP-Clients einer Sitzung; das Zeitlimit je Aufruf setzen die [`Limits`] des Aufrufs.
/// 30 s: Der Abgleich (`sync/`) erlaubt 30 s je Aufruf und teilt sich den Vorrat mit Suche und Websuche.
const SESSION_CEILING: Duration = Duration::from_secs(30);
/// Abstand der Nachträge zum Ende von [`crate::rag::WAY_TIMEOUT`] (Puffer für Rückgabe und Zusammenführen).
const FOLLOW_UP_MARGIN: Duration = Duration::from_secs(1);
/// Höchstens so viele Seiten liest [`MultigptWay::read_full`] bei seitenweiser Gegenstelle.
const MAX_FULLTEXT_PAGES: usize = 500;
/// Namen für den Text eines Dokuments in der Antwort von `read_document`.
const CONTENT_KEYS: &[&str] = &["text", "content", "body", "markdown"];
/// Namen für die Seitenzahl eines Dokuments (Annahme, siehe docs/Architektur.md).
const PAGE_COUNT_KEYS: &[&str] = &["pages", "total_pages", "page_count", "num_pages"];

/// Zeitlimit und Größengrenze eines Clients.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    pub timeout: Duration,
    pub max_body: usize,
}

impl Limits {
    /// Grenzen der Suche (15 s je Aufruf, 2 MiB).
    pub const SEARCH: Limits = Limits { timeout: CALL_TIMEOUT, max_body: MAX_RESPONSE };
}

/// Jedes Vorkommen von `secret` durch `***` ersetzen – über [`baruch_llm::scrub`] (Runde 23: auch Base64-
/// und Prozent-kodiert).
pub fn scrub_text(text: &str, secret: Option<&str>) -> String {
    baruch_llm::scrub(text, secret)
}

/// Meldung eines Fehlers umschreiben (gleiche Art).
fn map_message(e: RagError, f: impl Fn(&str) -> String) -> RagError {
    match e {
        RagError::Invalid(m) => RagError::Invalid(f(&m)),
        RagError::NotConfigured(m) => RagError::NotConfigured(f(&m)),
        RagError::NoEmbedder(m) => RagError::NoEmbedder(f(&m)),
        RagError::ModelMismatch(m) => RagError::ModelMismatch(f(&m)),
        RagError::Unreachable(m) => RagError::Unreachable(f(&m)),
        RagError::Denied(m) => RagError::Denied(f(&m)),
        RagError::Timeout(m) => RagError::Timeout(f(&m)),
        RagError::Schema(m) => RagError::Schema(f(&m)),
        RagError::Failed(m) => RagError::Failed(f(&m)),
    }
}

/// Fehler ohne Geheimnis.
pub fn scrub_error(e: RagError, secret: Option<&str>) -> RagError {
    map_message(e, |m| scrub_text(m, secret))
}

/// Meldung mit Vorsatz („initialize: …“).
fn prefixed(what: &str) -> impl Fn(RagError) -> RagError + '_ {
    move |e| map_message(e, |m| format!("{what}: {m}"))
}

fn is_event_stream(content_type: &str) -> bool {
    content_type.to_ascii_lowercase().starts_with("text/event-stream")
}

/// JSON-RPC-Antwort mit `id` aus einer MCP-Antwort (JSON oder Server-Sent Events); `None`, solange sie
/// (noch) nicht dabei ist. Unlesbar → `Schema`, Fehler der Gegenstelle → `Failed`.
///
/// Ein Ereignisstrom kommt in Stücken; endet der bisherige Puffer mitten in einem Mehrbyte-Zeichen
/// (Umlaut, „“), zählt nur der Teil davor – der Rest kommt mit dem nächsten Stück. Ungültige Bytes mitten im
/// Text bleiben `Schema`.
pub(crate) fn find_answer(content_type: &str, body: &[u8], id: i64) -> Option<Result<Value, RagError>> {
    let text = match std::str::from_utf8(body) {
        Ok(text) => text,
        // `error_len() == None`: nur ein unvollständiges Zeichen am Ende, der Anfang ist gültig.
        Err(e) if e.error_len().is_none() && is_event_stream(content_type) => {
            std::str::from_utf8(&body[..e.valid_up_to()]).expect("bis valid_up_to gültig")
        }
        Err(_) => return Some(Err(RagError::Schema("Antwort ist kein UTF-8-Text.".to_owned()))),
    };
    let mut candidates: Vec<Value> = Vec::new();
    if is_event_stream(content_type) {
        let mut data: Vec<&str> = Vec::new();
        for line in text.lines().chain([""]) {
            if line.is_empty() {
                if !data.is_empty() {
                    if let Ok(value) = serde_json::from_str::<Value>(&data.join("\n")) {
                        candidates.push(value);
                    }
                    data.clear();
                }
            } else if let Some(rest) = line.strip_prefix("data:") {
                data.push(rest.strip_prefix(' ').unwrap_or(rest));
            }
        }
    } else {
        match serde_json::from_str::<Value>(text) {
            Ok(Value::Array(batch)) => candidates.extend(batch),
            Ok(value) => candidates.push(value),
            Err(_) => return Some(Err(RagError::Schema("Antwort ist weder JSON noch text/event-stream.".to_owned()))),
        }
    }
    let answer = candidates.into_iter().find(|v| v.get("id").and_then(Value::as_i64) == Some(id))?;
    if let Some(error) = answer.get("error") {
        let message = error.get("message").and_then(Value::as_str).unwrap_or("ohne Meldung");
        return Some(Err(RagError::Failed(format!("Fehler der Gegenstelle: {message}"))));
    }
    Some(answer.get("result").cloned().ok_or_else(|| RagError::Schema("Antwort ohne result".to_owned())))
}

/// Wie [`find_answer`], aber „nicht gefunden“ ist ein Fehler.
pub(crate) fn rpc_answer(content_type: &str, body: &[u8], id: i64) -> Result<Value, RagError> {
    find_answer(content_type, body, id)
        .unwrap_or_else(|| Err(RagError::Schema(format!("keine JSON-RPC-Antwort mit id {id} gefunden"))))
}

fn mib(bytes: usize) -> usize {
    (bytes / (1024 * 1024)).max(1)
}

/// Ergebnis von `tools/list` über alle gelesenen Seiten.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolList {
    pub tools: Vec<Value>,
    pub pages: usize,
    /// Nach [`MAX_TOOL_PAGES`] Seiten nannte die Gegenstelle noch einen `nextCursor`.
    pub more: bool,
}

impl ToolList {
    pub fn names(&self) -> Vec<String> {
        self.tools.iter().filter_map(|t| t.get("name").and_then(Value::as_str)).map(str::to_owned).collect()
    }

    pub fn find(&self, name: &str) -> Option<&Value> {
        self.tools.iter().find(|t| t.get("name").and_then(Value::as_str) == Some(name))
    }
}

/// MCP-Client über Streamable HTTP (eine Sitzung).
pub struct McpClient {
    http: reqwest::Client,
    url: String,
    secret: Option<String>,
    session: Option<String>,
    next_id: i64,
    limits: Limits,
    /// HTTP-Status der letzten Antwort (für die Wiederholung mit neuer Sitzung).
    last_status: Option<u16>,
    /// Die letzte Anfrage ist nachweislich nie bei der Gegenstelle angekommen (Verbindungsaufbau gescheitert).
    unsent: bool,
}

impl McpClient {
    /// Verbinden: `initialize` und `notifications/initialized`. Gibt den Client und `serverInfo` zurück.
    pub async fn connect(url: &str, secret: Option<&str>, limits: Limits) -> Result<(McpClient, Value), RagError> {
        McpClient::connect_with(url, secret, limits, limits.timeout).await
    }

    /// Wie [`McpClient::connect`]; der HTTP-Client erlaubt Anfragen bis `ceiling` (für spätere Aufrufe mit
    /// eigenen Grenzen, [`McpClient::set_limits`]), das Verbinden selbst hat `limits.timeout`.
    async fn connect_with(
        url: &str,
        secret: Option<&str>,
        limits: Limits,
        ceiling: Duration,
    ) -> Result<(McpClient, Value), RagError> {
        let http = reqwest::Client::builder()
            .timeout(ceiling.max(limits.timeout))
            .connect_timeout(limits.timeout)
            .redirect(reqwest::redirect::Policy::none())
            .user_agent(concat!("baruch/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|e| RagError::Failed(format!("HTTP-Client: {e}")))?;
        let mut client = McpClient {
            http,
            url: url.to_owned(),
            secret: secret.filter(|s| !s.is_empty()).map(str::to_owned),
            session: None,
            next_id: 1,
            limits,
            last_status: None,
            unsent: false,
        };
        let init = json!({ "protocolVersion": MCP_PROTOCOL, "capabilities": {},
            "clientInfo": { "name": "baruch", "version": env!("CARGO_PKG_VERSION") } });
        let result = client.request("initialize", init).await.map_err(prefixed("initialize"))?;
        let server = result.get("serverInfo").cloned().unwrap_or(json!({}));
        let initialized = json!({ "jsonrpc": "2.0", "method": "notifications/initialized" });
        client.send(initialized, None).await.map_err(prefixed("notifications/initialized"))?;
        let server = scrub_value(server, client.secret.as_deref());
        Ok((client, server))
    }

    fn scrub(&self, e: RagError) -> RagError {
        scrub_error(e, self.secret.as_deref())
    }

    /// Grenzen der folgenden Anfragen (Zeitlimit höchstens bis zur Obergrenze des HTTP-Clients).
    fn set_limits(&mut self, limits: Limits) {
        self.limits = limits;
    }

    /// Sitzung bei der Gegenstelle beenden: HTTP `DELETE` mit `Mcp-Session-Id` (höchstens
    /// [`DELETE_TIMEOUT`]); Fehler werden ignoriert. Ohne Sitzung geschieht nichts.
    pub(crate) async fn delete(&self) {
        let Some(session) = &self.session else { return };
        let mut request =
            self.http.delete(&self.url).header("Mcp-Session-Id", session).header("MCP-Protocol-Version", MCP_PROTOCOL);
        if let Some(secret) = &self.secret {
            request = request.bearer_auth(secret);
        }
        let _ = tokio::time::timeout(DELETE_TIMEOUT, request.send()).await;
    }

    /// Eine Anfrage mit neuer `id`; Antwort `result`.
    pub async fn request(&mut self, method: &str, params: Value) -> Result<Value, RagError> {
        let id = self.next_id;
        self.next_id += 1;
        let mut message = json!({ "jsonrpc": "2.0", "id": id, "method": method });
        if !params.is_null() {
            message["params"] = params;
        }
        self.send(message, Some(id)).await
    }

    /// Eine Nachricht schicken (mit eigenem Zeitlimit um alles); mit `id` die Antwort lesen.
    async fn send(&mut self, message: Value, id: Option<i64>) -> Result<Value, RagError> {
        let limit = self.limits.timeout;
        let outcome = match tokio::time::timeout(limit, self.exchange(message, id)).await {
            Ok(outcome) => outcome,
            Err(_) => Err(RagError::Timeout(format!(
                "Keine vollständige Antwort von {} innerhalb von {} s.",
                self.url,
                limit.as_secs()
            ))),
        };
        outcome.map_err(|e| self.scrub(e))
    }

    fn http_error(&self, e: &reqwest::Error) -> RagError {
        if e.is_timeout() {
            return RagError::Timeout(format!(
                "Keine Antwort von {} innerhalb von {} s.",
                self.url,
                self.limits.timeout.as_secs()
            ));
        }
        let mut causes = Vec::new();
        let mut source = std::error::Error::source(e);
        while let Some(cause) = source {
            causes.push(cause.to_string());
            source = cause.source();
        }
        let why = if causes.is_empty() { e.to_string() } else { causes.join(": ") };
        if e.is_connect() {
            RagError::Unreachable(format!("Keine Verbindung zu {}: {why}", self.url))
        } else {
            RagError::Unreachable(format!("Anfrage an {} gescheitert: {why}", self.url))
        }
    }

    fn too_big(&self) -> RagError {
        RagError::Failed(format!(
            "Antwort von {} ist größer als {} MiB; abgebrochen.",
            self.url,
            mib(self.limits.max_body)
        ))
    }

    async fn exchange(&mut self, message: Value, id: Option<i64>) -> Result<Value, RagError> {
        let mut request = self
            .http
            .post(&self.url)
            .header(reqwest::header::ACCEPT, "application/json, text/event-stream")
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body(message.to_string());
        if let Some(secret) = &self.secret {
            request = request.bearer_auth(secret);
        }
        if let Some(session) = &self.session {
            request = request.header("Mcp-Session-Id", session).header("MCP-Protocol-Version", MCP_PROTOCOL);
        }
        self.last_status = None;
        self.unsent = false;
        let response = request.send().await.map_err(|e| {
            // Nur ein gescheiterter Verbindungsaufbau beweist, dass nichts gesendet wurde; eine Zeitüberschreitung
            // oder ein Abbruch danach kann die Anfrage schon ausgeführt haben.
            self.unsent = e.is_connect();
            self.http_error(&e)
        })?;
        let status = response.status();
        self.last_status = Some(status.as_u16());
        if matches!(status.as_u16(), 401 | 403) {
            return Err(RagError::Denied(denied(status, "multigpt", self.secret.is_some())));
        }
        if status.is_redirection() {
            return Err(RagError::Failed(format!(
                "{} leitet weiter (HTTP {}); Baruch folgt Weiterleitungen nicht, damit das Token nicht an eine \
                 andere Adresse geht. Die Zieladresse unter Einstellungen → Verbindungen eintragen.",
                self.url,
                status.as_u16()
            )));
        }
        if !status.is_success() {
            return Err(RagError::Failed(format!("{} antwortet mit HTTP {}.", self.url, status.as_u16())));
        }
        if let Some(session) = response.headers().get("mcp-session-id").and_then(|v| v.to_str().ok()) {
            self.session = Some(session.to_owned());
        }
        let Some(id) = id else { return Ok(Value::Null) };
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_owned();
        if response.content_length().is_some_and(|n| n > self.limits.max_body as u64) {
            return Err(self.too_big());
        }
        let stream = is_event_stream(&content_type);
        let mut response = response;
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(|e| self.http_error(&e))? {
            if bytes.len() + chunk.len() > self.limits.max_body {
                return Err(self.too_big());
            }
            bytes.extend_from_slice(&chunk);
            // Ein Ereignisstrom darf offen bleiben: lesen, bis die Antwort mit `id` da ist.
            if stream && let Some(answer) = find_answer(&content_type, &bytes, id) {
                return answer;
            }
        }
        rpc_answer(&content_type, &bytes, id)
    }

    /// `tools/list` über `nextCursor`, höchstens `max_pages` Seiten.
    pub async fn list_tools(&mut self, max_pages: usize) -> Result<ToolList, RagError> {
        let mut cursor: Option<String> = None;
        let mut tools = Vec::new();
        let mut pages = 0;
        loop {
            let params = cursor.as_ref().map_or(Value::Null, |c| json!({ "cursor": c }));
            let result = self.request("tools/list", params).await.map_err(prefixed("tools/list"))?;
            pages += 1;
            if let Some(list) = result.get("tools").and_then(Value::as_array) {
                tools.extend(list.iter().cloned());
            }
            let next = result.get("nextCursor").and_then(Value::as_str).filter(|c| !c.is_empty()).map(str::to_owned);
            match next {
                None => return Ok(ToolList { tools, pages, more: false }),
                Some(_) if pages >= max_pages => return Ok(ToolList { tools, pages, more: true }),
                next => cursor = next,
            }
        }
    }

    /// `tools/call`; `isError` der Gegenstelle → [`RagError::Failed`] mit ihrem Text.
    pub async fn call_tool(&mut self, name: &str, arguments: Value) -> Result<Value, RagError> {
        let result = self
            .request("tools/call", json!({ "name": name, "arguments": arguments }))
            .await
            .map_err(prefixed(name))?;
        if result.get("isError") == Some(&Value::Bool(true)) {
            let text = clean_line(&content_text(&result), 300);
            let text = if text.is_empty() { "ohne Meldung".to_owned() } else { text };
            return Err(self.scrub(RagError::Failed(format!("{name} meldet einen Fehler: {text}"))));
        }
        Ok(result)
    }

    /// Daten ohne das Token (für Angaben der Gegenstelle).
    pub fn clean(&self, value: Value) -> Value {
        scrub_value(value, self.secret.as_deref())
    }
}

/// Alle `content[].text` eines Werkzeug-Ergebnisses, mit Zeilenumbruch verbunden.
pub fn content_text(result: &Value) -> String {
    result
        .get("content")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter(|i| i.get("type").and_then(Value::as_str).is_none_or(|t| t == "text"))
                .filter_map(|i| i.get("text").and_then(Value::as_str))
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default()
}

/// Eine Zeile: Steuerzeichen und Leerraum → ein Leerzeichen, höchstens `max` Zeichen (sonst „…“).
pub fn clean_line(text: &str, max: usize) -> String {
    let flat: String = text
        .split(|c: char| c.is_whitespace() || c.is_control())
        .filter(|w| !w.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    if flat.chars().count() <= max {
        flat
    } else {
        let mut cut: String = flat.chars().take(max.saturating_sub(1)).collect();
        cut.push('…');
        cut
    }
}

/// Daten eines Werkzeug-Ergebnisses: `structuredContent` (Objekt oder Liste), sonst das erste JSON-Objekt
/// bzw. die erste Liste in `content[].text`; sonst `None`.
pub fn tool_payload(result: &Value) -> Option<Value> {
    if let Some(structured) = result.get("structuredContent").filter(|v| v.is_object() || v.is_array()) {
        return Some(structured.clone());
    }
    let items = result.get("content").and_then(Value::as_array)?;
    items
        .iter()
        .filter_map(|i| i.get("text").and_then(Value::as_str))
        .filter_map(|t| serde_json::from_str::<Value>(t.trim()).ok())
        .find(|v| v.is_object() || v.is_array())
}

/// Eine Liste in den Daten: die Daten selbst, oder unter einem der Schlüssel `keys` (auch eine Ebene tiefer,
/// etwa `{"result": {"results": [..]}}`).
fn list_in<'a>(payload: &'a Value, keys: &[&str], depth: usize) -> Option<&'a Vec<Value>> {
    match payload {
        Value::Array(list) => Some(list),
        Value::Object(map) => {
            let direct = keys.iter().filter_map(|k| map.get(*k)).find_map(Value::as_array);
            direct.or_else(|| {
                (depth > 0).then(|| map.values().filter(|v| v.is_object()).find_map(|v| list_in(v, keys, depth - 1)))?
            })
        }
        _ => None,
    }
}

/// Schlüssel einer Trefferliste.
const HIT_LIST_KEYS: &[&str] = &["results", "hits", "documents", "matches", "items", "data", "chunks", "result"];
const COLLECTION_LIST_KEYS: &[&str] = &["collections", "items", "data", "results", "result"];

/// Erster Text (oder Zahl als Text) unter einem der Schlüssel, im Treffer selbst oder in `metadata`,
/// `meta`, `document`, `source`.
fn text_of(item: &Map<String, Value>, keys: &[&str]) -> Option<String> {
    let mut places = vec![item];
    for nested in ["metadata", "meta", "document", "source"] {
        if let Some(Value::Object(inner)) = item.get(nested) {
            places.push(inner);
        }
    }
    places.into_iter().find_map(|place| {
        keys.iter().find_map(|k| match place.get(*k) {
            Some(Value::String(s)) if !s.trim().is_empty() => Some(s.trim().to_owned()),
            Some(Value::Number(n)) => Some(n.to_string()),
            _ => None,
        })
    })
}

fn number_of(item: &Map<String, Value>, keys: &[&str]) -> Option<f64> {
    let mut places = vec![item];
    if let Some(Value::Object(inner)) = item.get("metadata") {
        places.push(inner);
    }
    places
        .into_iter()
        .find_map(|place| keys.iter().find_map(|k| place.get(*k).and_then(Value::as_f64)))
        .filter(|n| n.is_finite())
}

/// Nur http(s)-Adressen gelten.
pub fn web_url(url: Option<String>) -> Option<String> {
    url.filter(|u| {
        let lower = u.to_ascii_lowercase();
        (lower.starts_with("http://") || lower.starts_with("https://")) && !u.chars().any(char::is_whitespace)
    })
}

/// Ersatz-Score eines Treffers ohne `score` und `distance` (Runde 21a, Gutachten R19): Die Ähnlichkeit ist dann
/// unbekannt, nur die Reihenfolge zählt. `WEAK_SCORE + (1 − WEAK_SCORE) / Rang` fällt mit dem Rang (Rang 1 = 1,0) und
/// bleibt immer bei oder über [`baruch_rag::WEAK_SCORE`]: Ein Treffer „aus Rang“ gilt nie als „schwach“ (früher 1/Rang:
/// jeder Treffer ab Rang 3 war schwach, ohne dass MultiGPT etwas über seine Ähnlichkeit gesagt hätte). `meta.score`
/// bleibt `"aus Rang"`, damit Anzeigen die Zahl als Rang-Ersatz erkennen können.
pub fn rank_score(rank: usize) -> f64 {
    let weak = f64::from(baruch_rag::WEAK_SCORE);
    weak + (1.0 - weak) / rank.max(1) as f64
}

/// Treffer aus den Daten von `search_documents` (Rang = Stelle in der Liste, ab 1). `collection` gilt für
/// Treffer ohne eigene Sammlung (ein Aufruf je Sammlung). Keine Liste oder kein lesbarer Treffer in einer
/// nicht leeren Liste → [`RagError::Schema`].
pub fn parse_hits(payload: &Value, collection: Option<&str>) -> Result<Vec<Hit>, RagError> {
    let list = list_in(payload, HIT_LIST_KEYS, 1).ok_or_else(|| {
        RagError::Schema(format!("{SEARCH_TOOL}: keine Trefferliste in der Antwort (results, hits, documents …)."))
    })?;
    let mut hits = Vec::new();
    for (index, item) in list.iter().enumerate() {
        let rank = index + 1;
        let Some(item) = item.as_object() else { continue };
        let document = text_of(
            item,
            &[
                "document_id",
                "doc_id",
                "documentId",
                "document",
                "source_id",
                "id",
                "source",
                "filename",
                "file_name",
                "file",
            ],
        );
        let title = text_of(item, &["title", "document_title", "name", "filename", "file_name"]);
        let (document, title) = match (document, title) {
            (Some(d), Some(t)) => (d, t),
            (Some(d), None) => (d.clone(), d),
            (None, Some(t)) => (t.clone(), t),
            (None, None) => continue,
        };
        let reported = number_of(item, &["score", "similarity", "relevance", "relevance_score", "_score"]);
        let distance = number_of(item, &["distance"]);
        let (score, origin) = match (reported, distance) {
            (Some(s), _) => (s.clamp(0.0, 1.0), "gemeldet"),
            (None, Some(d)) => ((1.0 - d).clamp(0.0, 1.0), "aus Abstand"),
            (None, None) => (rank_score(rank), "aus Rang"),
        };
        let found_in = text_of(item, &["collection", "collection_name", "collection_id", "collectionId"]);
        hits.push(Hit {
            way: Way::Multigpt,
            collection: clean_line(&found_in.or_else(|| collection.map(str::to_owned)).unwrap_or_default(), 200),
            document: clean_line(&document, 500),
            section: text_of(
                item,
                &["section", "chunk_id", "chunk", "chunk_index", "page", "location", "anchor", "position"],
            )
            .map(|s| clean_line(&s, 200)),
            title: clean_line(&title, 300),
            snippet: clean_snippet(
                &text_of(item, &["snippet", "text", "content", "chunk_text", "excerpt", "passage", "highlight"])
                    .unwrap_or_default(),
            ),
            score: score as f32,
            url: web_url(text_of(item, &["url", "link", "source_url", "uri", "href"])),
            meta: json!({ "rank": rank, "score": origin }),
        });
    }
    if hits.is_empty() && !list.is_empty() {
        return Err(RagError::Schema(format!(
            "{SEARCH_TOOL}: {} Treffer ohne Dokument und Titel; Format nicht lesbar.",
            list.len()
        )));
    }
    Ok(hits)
}

/// Eine Sammlung, wie die Einstellungsseite sie zeigt (`count` = Dokumente, falls gemeldet).
pub fn parse_collections(payload: &Value) -> Vec<Value> {
    let Some(list) = list_in(payload, COLLECTION_LIST_KEYS, 1) else { return Vec::new() };
    let mut seen: Vec<String> = Vec::new();
    let mut out = Vec::new();
    for item in list {
        let (name, count) = match item {
            Value::String(s) => (clean_line(s, 200), None),
            Value::Object(map) => {
                let name = text_of(map, &["name", "collection", "collection_name", "id", "title"]).unwrap_or_default();
                let count =
                    ["count", "documents", "document_count", "documents_count", "num_documents", "doc_count", "size"]
                        .iter()
                        .find_map(|k| map.get(*k).and_then(Value::as_u64));
                (clean_line(&name, 200), count)
            }
            _ => continue,
        };
        if name.is_empty() || seen.contains(&name) {
            continue;
        }
        seen.push(name.clone());
        out.push(json!({ "name": name, "count": count }));
    }
    out
}

/// Darf ein Parameter eine Liste sein?
fn allows_array(schema: &Value) -> bool {
    let typed = |s: &Value| match s.get("type") {
        Some(Value::String(t)) => t == "array",
        Some(Value::Array(types)) => types.iter().any(|t| t == "array"),
        _ => false,
    };
    typed(schema)
        || ["anyOf", "oneOf"]
            .iter()
            .filter_map(|k| schema.get(*k).and_then(Value::as_array))
            .any(|alts| alts.iter().any(typed))
}

/// Wert passend zum Schema: Zahl, wenn das Schema `integer` verlangt und der Text eine ist.
fn typed_value(schema: &Value, text: &str) -> Value {
    let integer = match schema.get("type") {
        Some(Value::String(t)) => t == "integer",
        Some(Value::Array(types)) => types.iter().any(|t| t == "integer") && !types.iter().any(|t| t == "string"),
        _ => false,
    };
    match text.trim().parse::<i64>() {
        Ok(n) if integer => json!(n),
        _ => json!(text),
    }
}

/// Argumente für `search_documents` aus dessen `inputSchema`: je Aufruf die Argumente und die Sammlung,
/// falls das Schema nur eine Sammlung je Aufruf nimmt. Ohne `properties` gelten `query`, `collections`
/// (Liste) und `limit`.
pub fn search_arguments(
    schema: &Value,
    text: &str,
    collections: &[String],
    limit: usize,
) -> Result<Vec<(Value, Option<String>)>, RagError> {
    let empty = Map::new();
    let props = schema.get("properties").and_then(Value::as_object).unwrap_or(&empty);
    let pick = |keys: &[&'static str]| keys.iter().find(|k| props.contains_key(**k)).copied();
    let (query_key, collection_key, limit_key) = if props.is_empty() {
        ("query", "collections", Some("limit"))
    } else {
        let query_key = pick(QUERY_KEYS).ok_or_else(|| {
            RagError::Schema(format!(
                "{SEARCH_TOOL} nennt im inputSchema keinen Parameter für die Anfrage ({}).",
                QUERY_KEYS.join(", ")
            ))
        })?;
        let collection_key = pick(COLLECTION_KEYS).ok_or_else(|| {
            RagError::Schema(format!(
                "{SEARCH_TOOL} nennt im inputSchema keinen Parameter für Sammlungen ({}); ohne ihn suchte \
                 MultiGPT in allen Sammlungen, darum fragt Baruch nicht.",
                COLLECTION_KEYS.join(", ")
            ))
        })?;
        (query_key, collection_key, pick(LIMIT_KEYS))
    };
    let required: Vec<&str> = schema
        .get("required")
        .and_then(Value::as_array)
        .map(|r| r.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    if let Some(unknown) = required.iter().find(|r| ![query_key, collection_key].contains(r) && Some(**r) != limit_key)
    {
        return Err(RagError::Schema(format!(
            "{SEARCH_TOOL} verlangt den Parameter „{unknown}“, den Baruch nicht kennt."
        )));
    }
    let mut base = Map::new();
    base.insert(query_key.to_owned(), json!(text));
    if let Some(key) = limit_key {
        let spec = props.get(key).cloned().unwrap_or(json!({}));
        let mut n = limit as u64;
        if let Some(max) = spec.get("maximum").and_then(Value::as_u64) {
            n = n.min(max);
        }
        if let Some(min) = spec.get("minimum").and_then(Value::as_u64) {
            n = n.max(min);
        }
        base.insert(key.to_owned(), json!(n));
    }
    let spec = props.get(collection_key).cloned().unwrap_or(json!({ "type": "array" }));
    if props.is_empty() || allows_array(&spec) {
        let items = spec.get("items").cloned().unwrap_or(json!({}));
        let mut args = base;
        args.insert(
            collection_key.to_owned(),
            Value::Array(collections.iter().map(|c| typed_value(&items, c)).collect()),
        );
        return Ok(vec![(Value::Object(args), None)]);
    }
    Ok(collections
        .iter()
        .map(|c| {
            let mut args = base.clone();
            args.insert(collection_key.to_owned(), typed_value(&spec, c));
            (Value::Object(args), Some(c.clone()))
        })
        .collect())
}

/// Argumente für `document_info` bzw. `read_document`: das Dokument (und die Stelle, falls das Schema eine
/// kennt); `None`, wenn das Schema keinen Parameter für das Dokument nennt.
pub fn document_arguments(schema: &Value, document: &str, section: Option<&str>) -> Option<Value> {
    let props = schema.get("properties").and_then(Value::as_object)?;
    let key = DOCUMENT_KEYS.iter().find(|k| props.contains_key(**k))?;
    let mut args = Map::new();
    args.insert((*key).to_owned(), typed_value(&props[*key], document));
    if let (Some(section), Some(skey)) = (section, SECTION_KEYS.iter().find(|k| props.contains_key(**k))) {
        args.insert((*skey).to_owned(), typed_value(&props[*skey], section));
    }
    Some(Value::Object(args))
}

/// Ausschnitt aus einer Antwort von `read_document`: Text aus den Daten (`text`, `content`, `excerpt` …)
/// oder der Text in `content[]`, gesäubert und auf [`baruch_rag::MAX_SNIPPET_CHARS`] gekürzt (nie der Volltext).
pub fn excerpt_of(result: &Value) -> Option<String> {
    let from_payload = tool_payload(result)
        .and_then(|p| p.as_object().and_then(|m| text_of(m, &["excerpt", "snippet", "text", "content", "body"])));
    let text = from_payload.unwrap_or_else(|| content_text(result));
    let excerpt = clean_snippet(&text);
    (!excerpt.is_empty()).then_some(excerpt)
}

// ---- Sitzung und Vorrat (Runde 13, Vertrag §2.2) ---------------------------------------------------------

/// Schlüssel des Vorrats: Adresse, Hash des Tokens (nie das Token selbst), Tokio-Laufzeit.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct PoolKey {
    url: String,
    token: u64,
    runtime: String,
}

impl PoolKey {
    fn new(url: &str, secret: Option<&str>) -> PoolKey {
        // Zufälliger Schlüssel je Prozess: Der Hash taugt nur zum Unterscheiden, nicht zum Zurückrechnen.
        static HASHER: OnceLock<std::collections::hash_map::RandomState> = OnceLock::new();
        let token = match secret.filter(|s| !s.is_empty()) {
            None => 0,
            Some(secret) => HASHER.get_or_init(Default::default).hash_one(secret),
        };
        let runtime = tokio::runtime::Handle::try_current().map(|h| h.id().to_string()).unwrap_or_default();
        PoolKey { url: url.to_owned(), token, runtime }
    }
}

/// Eine ruhende Sitzung im Vorrat.
struct Idle {
    client: McpClient,
    since: Instant,
}

static POOL: LazyLock<Mutex<HashMap<PoolKey, Vec<Idle>>>> = LazyLock::new(|| Mutex::new(HashMap::new()));
static TOOL_CACHE: LazyLock<Mutex<HashMap<PoolKey, (Instant, ToolList)>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

fn locked<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// Sitzung im Hintergrund beenden (`DELETE`), falls eine Tokio-Laufzeit läuft.
fn spawn_delete(client: McpClient) {
    if let Ok(handle) = tokio::runtime::Handle::try_current() {
        handle.spawn(async move { client.delete().await });
    }
}

/// Ruhende Sitzungen, die länger als `older_than` warten, aus dem Vorrat nehmen (nur die der Adresse `url`,
/// falls angegeben).
fn take_expired(older_than: Duration, url: Option<&str>) -> Vec<McpClient> {
    let mut pool = locked(&POOL);
    let mut out = Vec::new();
    for (key, idle) in pool.iter_mut() {
        if url.is_some_and(|u| u != key.url) {
            continue;
        }
        let (old, keep): (Vec<Idle>, Vec<Idle>) =
            std::mem::take(idle).into_iter().partition(|i| i.since.elapsed() >= older_than);
        *idle = keep;
        out.extend(old.into_iter().map(|i| i.client));
    }
    pool.retain(|_, idle| !idle.is_empty());
    out
}

/// Eine ruhende Sitzung für `key` (die zuletzt zurückgegebene); abgelaufene werden dabei beendet.
fn take_idle(key: &PoolKey) -> Option<McpClient> {
    for client in take_expired(SESSION_IDLE, None) {
        spawn_delete(client);
    }
    let mut pool = locked(&POOL);
    let client = pool.get_mut(key).and_then(Vec::pop).map(|i| i.client);
    pool.retain(|_, idle| !idle.is_empty());
    client
}

/// Sitzung in den Vorrat legen (höchstens [`MAX_IDLE_SESSIONS`] je Schlüssel, sonst beenden) und nach
/// [`SESSION_IDLE`] nachsehen, ob sie noch ruht.
fn put_idle(key: PoolKey, client: McpClient) {
    let surplus = {
        let mut pool = locked(&POOL);
        let idle = pool.entry(key).or_default();
        if idle.len() < MAX_IDLE_SESSIONS {
            idle.push(Idle { client, since: Instant::now() });
            None
        } else {
            Some(client)
        }
    };
    if let Some(client) = surplus {
        spawn_delete(client);
    }
    if let Ok(handle) = tokio::runtime::Handle::try_current() {
        handle.spawn(async {
            tokio::time::sleep(SESSION_IDLE).await;
            for client in take_expired(SESSION_IDLE, None) {
                client.delete().await;
            }
        });
    }
}

/// Ruhende Sitzungen der Adresse `url`, die mindestens `older_than` warten, mit `DELETE` beenden; gibt ihre
/// Zahl zurück. Der Dienst tut das selbst nach [`SESSION_IDLE`]; Tests rufen es mit kurzer Frist auf.
pub async fn expire_idle(url: &str, older_than: Duration) -> usize {
    let expired = take_expired(older_than, Some(url));
    let count = expired.len();
    for client in expired {
        client.delete().await;
    }
    count
}

fn cached_tools(key: &PoolKey) -> Option<ToolList> {
    let cache = locked(&TOOL_CACHE);
    cache.get(key).filter(|(at, _)| at.elapsed() < TOOLS_TTL).map(|(_, tools)| tools.clone())
}

fn store_tools(key: &PoolKey, tools: &ToolList) {
    let mut cache = locked(&TOOL_CACHE);
    cache.retain(|_, (at, _)| at.elapsed() < TOOLS_TTL);
    cache.insert(key.clone(), (Instant::now(), tools.clone()));
}

/// Adresse und Token aus Einstellungen → Verbindungen (Regeln wie [`open_way`]).
async fn settings(state: &AppState) -> Result<(String, Option<String>), RagError> {
    let row = state
        .store
        .connection_get(ConnectionKind::Multigpt)
        .await
        .map_err(|e| RagError::Failed(format!("Einstellungen der Verbindung MultiGPT: {e}")))?;
    let url = row.config.get("url").and_then(Value::as_str).unwrap_or("").trim().to_owned();
    if url.is_empty() {
        return Err(RagError::NotConfigured(
            "Keine Adresse für MultiGPT eingetragen (Einstellungen → Verbindungen).".into(),
        ));
    }
    let secret = super::secrets::get(&state.secrets, "multigpt")
        .await
        .map_err(|e| RagError::Failed(format!("Schlüsselbund: {e}")))?;
    if secret.is_some() && !super::secret_bound(&row) {
        return Err(RagError::NotConfigured(
            "Das Token baruch/multigpt ist für eine andere Adresse hinterlegt; Baruch schickt es nicht an die \
             jetzige. Für die neue Adresse das Token neu eintragen (Einstellungen → Verbindungen)."
                .into(),
        ));
    }
    Ok((url, secret))
}

/// Eine MCP-Sitzung zu MultiGPT. Prozessweiter Vorrat je (Adresse, Token-Hash): höchstens zwei ruhende
/// Sitzungen, die Werkzeugliste gilt TOOLS_TTL. Nach Gebrauch immer `release` oder `close`.
///
/// Fällt eine Sitzung ohne `release`/`close` weg (etwa weil ein Zeitlimit die Suche abbricht), beendet sie
/// sich im Hintergrund mit `DELETE`.
pub struct Session {
    client: Option<McpClient>,
    key: PoolKey,
    url: String,
    secret: Option<String>,
    tools: ToolList,
    limits: Limits,
    /// Aus dem Vorrat und noch ohne Antwort: ein Fehler kann heißen, dass die Gegenstelle sie beendet hat.
    reused: bool,
    /// Transport- oder Protokollfehler: nicht zurück in den Vorrat.
    broken: bool,
}

impl Session {
    /// Adresse und Token aus Einstellungen → Verbindungen, Regeln wie `open_way`: ohne Adresse NotConfigured,
    /// das Token nur bei passender Bindung (`secret_bound`), sonst NotConfigured mit Hinweis.
    pub async fn open(state: &AppState, limits: Limits) -> Result<Session, RagError> {
        let (url, secret) = settings(state).await?;
        Session::connect(&url, secret.as_deref(), limits).await
    }

    /// Direkt aus Adresse und Token (für Tests und `MultigptWay`).
    pub async fn connect(url: &str, secret: Option<&str>, limits: Limits) -> Result<Session, RagError> {
        let secret = secret.filter(|s| !s.is_empty());
        let key = PoolKey::new(url, secret);
        if let Some(mut client) = take_idle(&key) {
            client.set_limits(limits);
            let tools = match cached_tools(&key) {
                Some(tools) => Some(tools),
                None => match client.list_tools(MAX_TOOL_PAGES).await {
                    Ok(tools) => {
                        store_tools(&key, &tools);
                        Some(tools)
                    }
                    Err(_) => None,
                },
            };
            match tools {
                Some(tools) => {
                    return Ok(Session {
                        client: Some(client),
                        key,
                        url: url.to_owned(),
                        secret: secret.map(str::to_owned),
                        tools,
                        limits,
                        reused: true,
                        broken: false,
                    });
                }
                // Die ruhende Sitzung antwortet nicht mehr: beenden und neu verbinden.
                None => spawn_delete(client),
            }
        }
        let (mut client, _) = McpClient::connect_with(url, secret, limits, SESSION_CEILING).await?;
        let tools = match cached_tools(&key) {
            Some(tools) => tools,
            None => match client.list_tools(MAX_TOOL_PAGES).await {
                Ok(tools) => {
                    store_tools(&key, &tools);
                    tools
                }
                Err(e) => {
                    spawn_delete(client);
                    return Err(e);
                }
            },
        };
        Ok(Session {
            client: Some(client),
            key,
            url: url.to_owned(),
            secret: secret.map(str::to_owned),
            tools,
            limits,
            reused: false,
            broken: false,
        })
    }

    /// Werkzeugliste (zwischengespeichert).
    /// Ist die Sitzung gestört (Transport- oder Protokollfehler beim letzten Aufruf)? Ein `isError` der Gegenstelle
    /// stört sie nicht: Die Antwort kam vollständig an (Abgleich, Runde 19a: blockiert nur dieses Dokument).
    pub fn broken(&self) -> bool {
        self.broken
    }

    pub fn tools(&self) -> &ToolList {
        &self.tools
    }

    /// `inputSchema` eines Werkzeugs; `None`, wenn MultiGPT es nicht anbietet.
    pub fn schema(&self, tool: &str) -> Option<Value> {
        self.tools.find(tool).map(|t| t.get("inputSchema").cloned().unwrap_or_else(|| json!({})))
    }

    /// `tools/call` mit eigenen Grenzen für diesen Aufruf (`None` = die der Sitzung). Bei `isError` gibt es
    /// RagError::Failed mit dem Text der Gegenstelle, ohne Token.
    pub async fn call(&mut self, tool: &str, arguments: Value, limits: Option<Limits>) -> Result<Value, RagError> {
        let limits = limits.unwrap_or(self.limits);
        let first = self.call_once(tool, arguments.clone(), limits).await;
        let retry = self.reused && first.is_err() && self.gone();
        self.reused = false;
        if !retry {
            return first;
        }
        // Die Sitzung aus dem Vorrat hat die Gegenstelle schon beendet: einmal mit einer neuen.
        if let Some(old) = self.client.take() {
            spawn_delete(old);
        }
        let (client, _) = McpClient::connect_with(&self.url, self.secret.as_deref(), limits, SESSION_CEILING)
            .await
            .inspect_err(|_| self.broken = true)?;
        self.client = Some(client);
        self.broken = false;
        self.call_once(tool, arguments, limits).await
    }

    /// Hat die Gegenstelle die Sitzung offenbar beendet, ohne den Aufruf auszuführen (HTTP 400/404 oder keine
    /// Verbindung zustande gekommen)? Eine Zeitüberschreitung oder ein Abbruch nach dem Senden zählt nicht: Der
    /// Aufruf kann dann schon ausgeführt sein, und eine Wiederholung legte etwa ein `upload_document` doppelt an.
    fn gone(&self) -> bool {
        self.broken && self.client.as_ref().is_some_and(|c| c.unsent || matches!(c.last_status, Some(400 | 404)))
    }

    async fn call_once(&mut self, tool: &str, arguments: Value, limits: Limits) -> Result<Value, RagError> {
        let Some(client) = self.client.as_mut() else {
            self.broken = true;
            return Err(RagError::Failed("Sitzung zu MultiGPT ist schon beendet.".into()));
        };
        let previous = client.limits;
        client.set_limits(limits);
        let outcome =
            client.request("tools/call", json!({ "name": tool, "arguments": arguments })).await.map_err(prefixed(tool));
        client.set_limits(previous);
        let result = match outcome {
            Ok(result) => result,
            Err(e) => {
                self.broken = true;
                return Err(e);
            }
        };
        if result.get("isError") == Some(&Value::Bool(true)) {
            let text = clean_line(&content_text(&result), 300);
            let text = if text.is_empty() { "ohne Meldung".to_owned() } else { text };
            return Err(scrub_error(
                RagError::Failed(format!("{tool} meldet einen Fehler: {text}")),
                self.secret.as_deref(),
            ));
        }
        // Auch Daten gehen nur ohne Token zurück (Websuche und Abgleich zeigen sie an).
        Ok(self.clean(result))
    }

    /// Text ohne Token.
    pub fn scrub(&self, text: &str) -> String {
        scrub_text(text, self.secret.as_deref())
    }

    /// Daten ohne Token.
    fn clean(&self, value: Value) -> Value {
        scrub_value(value, self.secret.as_deref())
    }

    /// Zurück in den Vorrat. Nach einem Transport- oder Protokollfehler beendet sie sich stattdessen mit DELETE.
    pub async fn release(mut self) {
        let Some(mut client) = self.client.take() else { return };
        if self.broken {
            // Im Hintergrund: Die Suche wartet nicht auf eine Gegenstelle, die gerade nicht antwortet.
            spawn_delete(client);
        } else {
            client.set_limits(self.limits);
            put_idle(self.key.clone(), client);
        }
    }

    /// Beenden mit HTTP DELETE (Mcp-Session-Id); Fehler beim DELETE werden ignoriert.
    pub async fn close(mut self) {
        if let Some(client) = self.client.take() {
            client.delete().await;
        }
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        if let Some(client) = self.client.take() {
            spawn_delete(client);
        }
    }
}

// ---- Volltext (Runde 13) ------------------------------------------------------------------------------

/// Ein Teil der Dokumentansicht (Vertrag §2.2).
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct FullTextPart {
    pub document: String,
    pub part: usize,
    /// `None`, wenn die Gegenstelle seitenweise liefert und das Ende noch unbekannt ist.
    pub parts: Option<usize>,
    pub more: bool,
    pub text: String,
    /// Kennt das Schema einen Seitenparameter (SECTION_KEYS), liest quellen seitenweise über die Gegenstelle;
    /// sonst teilt Baruch den Volltext an Absatzgrenzen in Teile von höchstens PART_CHARS Zeichen.
    pub paged_by_remote: bool,
}

/// Angaben eines Dokuments aus `document_info` (falls angeboten); fehlt etwas, bleibt es `None`.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize)]
pub struct DocumentInfo {
    pub title: Option<String>,
    pub url: Option<String>,
}

/// Seitenparameter im Schema von `read_document`: ein Name aus [`SECTION_KEYS`], der eine Zahl nimmt
/// (`page` immer); Namen wie `chunk_id` mit Text sind keine Seiten.
pub fn page_key(schema: &Value) -> Option<&'static str> {
    let props = schema.get("properties").and_then(Value::as_object)?;
    let numeric = |spec: &Value| match spec.get("type") {
        Some(Value::String(t)) => t == "integer" || t == "number",
        Some(Value::Array(types)) => types.iter().any(|t| t == "integer" || t == "number"),
        _ => false,
    };
    SECTION_KEYS.iter().copied().find(|k| props.get(*k).is_some_and(|spec| *k == "page" || numeric(spec)))
}

/// Teilt `text` an Absatzgrenzen (Leerzeilen bleiben am Absatz davor) in Teile von höchstens `max` Zeichen;
/// ein längerer Absatz wird am letzten Leerraum vor der Grenze geteilt, sonst hart. Die Teile hintereinander
/// ergeben genau `text` (so bleiben Zeichenbereiche im Volltext gültig). Leerer Text → ein leerer Teil.
pub fn split_parts(text: &str, max: usize) -> Vec<String> {
    let max = max.max(1);
    let mut paragraphs: Vec<&str> = Vec::new();
    let mut start = 0;
    while let Some(found) = text[start..].find("\n\n") {
        let mut end = start + found + 2;
        while text[end..].starts_with('\n') {
            end += 1;
        }
        paragraphs.push(&text[start..end]);
        start = end;
    }
    if start < text.len() {
        paragraphs.push(&text[start..]);
    }
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut current_len = 0;
    for paragraph in paragraphs {
        let len = paragraph.chars().count();
        if current_len + len <= max {
            current.push_str(paragraph);
            current_len += len;
            continue;
        }
        if !current.is_empty() {
            parts.push(std::mem::take(&mut current));
        }
        let mut rest = paragraph;
        while rest.chars().count() > max {
            let limit = rest.char_indices().nth(max).map_or(rest.len(), |(i, _)| i);
            let cut = rest[..limit]
                .char_indices()
                .rev()
                .find(|(_, c)| c.is_whitespace())
                .map(|(i, c)| i + c.len_utf8())
                .filter(|&i| i > 0)
                .unwrap_or(limit);
            parts.push(rest[..cut].to_owned());
            rest = &rest[cut..];
        }
        current = rest.to_owned();
        current_len = rest.chars().count();
    }
    if !current.is_empty() || parts.is_empty() {
        parts.push(current);
    }
    parts
}

/// Antwort von `read_document` (ein Dokument oder eine Seite).
struct PageAnswer {
    text: String,
    total: Option<usize>,
    more: Option<bool>,
}

impl PageAnswer {
    fn paged(&self) -> bool {
        self.total.is_some() || self.more.is_some()
    }
}

/// Text (ohne Beschneiden, `\r\n` → `\n`) und Seitenangaben aus einer Antwort von `read_document`: aus den
/// Daten (`text`, `content`, `body`, `markdown`, auch unter `document`/`result`/`data`), sonst der Text in
/// `content[]`. Daten ohne Text → [`RagError::Schema`].
fn page_answer(result: &Value) -> Result<PageAnswer, RagError> {
    let Some(payload) = tool_payload(result) else {
        return Ok(PageAnswer { text: content_text(result).replace("\r\n", "\n"), total: None, more: None });
    };
    let Some(map) = payload.as_object() else {
        return Err(RagError::Schema(format!("{READ_TOOL}: Antwort ist eine Liste, kein Dokument.")));
    };
    let mut places = vec![map];
    for nested in ["document", "result", "data"] {
        if let Some(Value::Object(inner)) = map.get(nested) {
            places.push(inner);
        }
    }
    let text = places
        .iter()
        .find_map(|place| CONTENT_KEYS.iter().find_map(|k| place.get(*k).and_then(Value::as_str)))
        .ok_or_else(|| {
            RagError::Schema(format!("{READ_TOOL}: kein Text in der Antwort ({}).", CONTENT_KEYS.join(", ")))
        })?;
    let total = places.iter().find_map(|place| {
        PAGE_COUNT_KEYS.iter().find_map(|k| place.get(*k).and_then(Value::as_u64)).map(|n| n as usize)
    });
    let more = places.iter().find_map(|place| {
        ["has_more", "more"].iter().find_map(|k| place.get(*k).and_then(Value::as_bool)).or_else(|| {
            ["next_page", "next_cursor", "nextCursor"].iter().find_map(|k| match place.get(*k) {
                Some(Value::Null) => Some(false),
                Some(Value::String(s)) => Some(!s.is_empty()),
                Some(Value::Number(_)) => Some(true),
                _ => None,
            })
        })
    });
    Ok(PageAnswer { text: text.replace("\r\n", "\n"), total, more })
}

/// Teil `part` (ab 1) eines ganzen Dokuments, von Baruch geteilt ([`split_parts`]), und die Stelle seines
/// ersten Zeichens im Volltext (Unicode-Zeichen ab 0). Gibt es den Teil nicht → [`RagError::Invalid`].
pub fn local_part(document: &str, text: &str, part: usize) -> Result<(FullTextPart, usize), RagError> {
    let parts = split_parts(text, PART_CHARS);
    let count = parts.len();
    if part == 0 || part > count {
        return Err(RagError::Invalid(format!("Teil {part} gibt es nicht; das Dokument hat {count} Teile.")));
    }
    let offset = parts[..part - 1].iter().map(|p| p.chars().count()).sum();
    let chosen = parts.into_iter().nth(part - 1).unwrap_or_default();
    let found = FullTextPart {
        document: document.to_owned(),
        part,
        parts: Some(count),
        more: part < count,
        text: chosen,
        paged_by_remote: false,
    };
    Ok((found, offset))
}

/// Ein gelesener Teil mit den Angaben des Dokuments und – wenn Baruch selbst geteilt hat – der Stelle seines
/// ersten Zeichens im Volltext.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct PartReading {
    pub part: FullTextPart,
    pub info: DocumentInfo,
    pub offset: Option<usize>,
}

/// Eine Antwort von `read_document` (mit Seitennummer, falls `page` gesetzt) unter `limits`.
async fn read_page_with(
    session: &mut Session,
    args: &Value,
    page: Option<(&'static str, usize)>,
    limits: Option<Limits>,
) -> Result<PageAnswer, RagError> {
    let mut args = args.clone();
    if let (Some((key, number)), Some(map)) = (page, args.as_object_mut()) {
        map.insert(key.to_owned(), json!(number));
    }
    let result = session.call(READ_TOOL, args, limits).await?;
    page_answer(&result)
}

/// Ganzer Text eines Dokuments über `read_document` mit den fertigen Argumenten `args`: Kennt das Schema einen
/// Seitenparameter (`page`, siehe [`page_key`]) und nennt die Antwort Seitenangaben, werden alle Seiten gelesen
/// und mit Leerzeile verbunden (höchstens [`MAX_FULLTEXT_PAGES`] Seiten und `max_bytes` Bytes, sonst Fehler);
/// sonst ist die eine Antwort das ganze Dokument. Gemeinsam für Volltext (quellen) und Abgleich, damit die
/// lokale Kopie genau dem Volltext entspricht.
pub(crate) async fn read_all_pages(
    session: &mut Session,
    args: &Value,
    page: Option<&'static str>,
    limits: Option<Limits>,
    max_bytes: usize,
) -> Result<String, RagError> {
    let too_big =
        || RagError::Failed(format!("{READ_TOOL}: Volltext ist größer als {} MiB; abgebrochen.", mib(max_bytes)));
    let Some(key) = page else {
        let text = read_page_with(session, args, None, limits).await?.text;
        return if text.len() > max_bytes { Err(too_big()) } else { Ok(text) };
    };
    let first = read_page_with(session, args, Some((key, 1)), limits).await?;
    if !first.paged() {
        return if first.text.len() > max_bytes { Err(too_big()) } else { Ok(first.text) };
    }
    let mut more = first.more.unwrap_or_else(|| first.total.is_some_and(|total| total > 1));
    let mut bytes = first.text.len();
    let mut pages = vec![first.text];
    while more {
        let number = pages.len() + 1;
        if number > MAX_FULLTEXT_PAGES {
            return Err(RagError::Failed(format!(
                "{READ_TOOL}: mehr als {MAX_FULLTEXT_PAGES} Seiten; Volltext abgebrochen."
            )));
        }
        let answer = read_page_with(session, args, Some((key, number)), limits).await?;
        bytes += answer.text.len() + 2;
        if bytes > max_bytes {
            return Err(too_big());
        }
        more = answer.more.unwrap_or_else(|| answer.total.is_some_and(|total| number < total));
        pages.push(answer.text);
    }
    Ok(pages.join("\n\n"))
}

/// Höchstens so viele Dokumente aus Treffern merkt sich der Prozess für [`MultigptWay::found_in_chosen`].
const MAX_FOUND: usize = 4096;

/// Ein Dokument, das eine Suche in gewählten Sammlungen gefunden hat.
struct Found {
    url: String,
    collection: String,
    document: String,
}

/// Dokumente aus Treffern von Suchen in gewählten Sammlungen (älteste fallen zuerst heraus).
static FOUND: LazyLock<Mutex<std::collections::VecDeque<Found>>> = LazyLock::new(Default::default);

/// Treffer einer Suche merken, deren Sammlung zu den gesuchten (vom Menschen gewählten) gehört. Ein Treffer
/// aus einer anderen Sammlung (die Gegenstelle hielte sich nicht an den Filter) zählt nicht.
fn remember_hits(url: &str, collections: &[String], hits: &[Hit]) {
    let mut found = locked(&FOUND);
    for hit in hits.iter().filter(|h| !h.document.is_empty() && collections.contains(&h.collection)) {
        let (collection, document) = (hit.collection.trim(), hit.document.trim());
        if found.iter().any(|f| f.url == url && f.collection == collection && f.document == document) {
            continue;
        }
        if found.len() >= MAX_FOUND {
            found.pop_front();
        }
        found.push_back(Found {
            url: url.to_owned(),
            collection: collection.to_owned(),
            document: document.to_owned(),
        });
    }
}

/// Suchweg MultiGPT.
pub struct MultigptWay {
    url: String,
    secret: Option<String>,
    limits: Limits,
}

impl MultigptWay {
    /// Für Tests und den Integrator: Weg direkt aus Adresse und Token (ohne Einstellungen).
    pub fn new(url: &str, secret: Option<&str>, limits: Limits) -> MultigptWay {
        MultigptWay { url: url.to_owned(), secret: secret.map(str::to_owned), limits }
    }

    /// Weg aus den Einstellungen (Regeln wie [`open_way`]), als konkreter Typ für Volltext und Ausschnitt.
    pub async fn from_settings(state: &AppState, limits: Limits) -> Result<MultigptWay, RagError> {
        let (url, secret) = settings(state).await?;
        Ok(MultigptWay::new(&url, secret.as_deref(), limits))
    }

    async fn session(&self) -> Result<Session, RagError> {
        Session::connect(&self.url, self.secret.as_deref(), self.limits).await
    }

    async fn run(&self, query: &Query) -> Result<Vec<Hit>, RagError> {
        if query.collections.is_empty() {
            return Err(RagError::NotConfigured("Keine Sammlung gewählt (Einstellungen → Verbindungen).".into()));
        }
        let started = Instant::now();
        let mut session = self.session().await?;
        let outcome = self.search_in(&mut session, query, started).await;
        session.release().await;
        outcome
    }

    async fn search_in(&self, session: &mut Session, query: &Query, started: Instant) -> Result<Vec<Hit>, RagError> {
        let schema = session.schema(SEARCH_TOOL).ok_or_else(|| {
            RagError::Schema(format!("MultiGPT bietet das Werkzeug {SEARCH_TOOL} nicht an (tools/list)."))
        })?;
        let calls = search_arguments(&schema, &query.text, &query.collections, query.limit)?;
        let mut hits = Vec::new();
        for (arguments, collection) in calls {
            let result = session.call(SEARCH_TOOL, arguments, None).await?;
            let payload = tool_payload(&result).ok_or_else(|| {
                RagError::Schema(format!(
                    "{SEARCH_TOOL}: Antwort nicht lesbar (weder structuredContent noch JSON in content)."
                ))
            })?;
            let mut found = parse_hits(&payload, collection.as_deref())?;
            if query.collections.len() == 1 {
                for hit in found.iter_mut().filter(|h| h.collection.is_empty()) {
                    hit.collection.clone_from(&query.collections[0]);
                }
            }
            hits.extend(found);
        }
        // Gleicher Score: die frühere Stelle der Gegenstelle bleibt vorn (stabil sortiert).
        hits.sort_by(|a, b| b.score.total_cmp(&a.score));
        hits.truncate(query.limit);
        let deadline = started + crate::rag::WAY_TIMEOUT.saturating_sub(FOLLOW_UP_MARGIN);
        self.complete(session, &mut hits, deadline).await;
        let secret = self.secret.as_deref();
        for hit in &mut hits {
            for text in [&mut hit.collection, &mut hit.document, &mut hit.title, &mut hit.snippet] {
                *text = scrub_text(text, secret);
            }
            hit.section = hit.section.as_deref().map(|s| scrub_text(s, secret));
            hit.url = hit.url.as_deref().map(|u| scrub_text(u, secret));
            hit.meta = scrub_value(hit.meta.take(), secret);
        }
        remember_hits(&self.url, &query.collections, &hits);
        Ok(hits)
    }

    /// Kam `document` (in `collection`, leer = gleich welcher) unter dieser Adresse in einem Treffer einer Suche
    /// in vom Menschen gewählten Sammlungen vor ([`remember_hits`])? Grundlage für MCP `source_read`.
    pub fn found_in_chosen(&self, collection: &str, document: &str) -> bool {
        let (collection, document) = (collection.trim(), document.trim());
        locked(&FOUND).iter().any(|f| {
            f.url == self.url && f.document == document && (collection.is_empty() || f.collection == collection)
        })
    }

    /// Fehlende Titel/URLs über `document_info`, fehlende Ausschnitte über `read_document` ergänzen (jeweils
    /// nur, wenn die Gegenstelle das Werkzeug anbietet; Fehler lassen den Treffer, wie er ist). Jeder Nachtrag
    /// hat höchstens [`FOLLOW_UP_TIMEOUT`], alle zusammen [`FOLLOW_UP_BUDGET`] und nie mehr als bis `deadline`;
    /// nach einem Transportfehler (auch Zeitlimit) gibt es keine weiteren Nachträge.
    async fn complete(&self, session: &mut Session, hits: &mut [Hit], deadline: Instant) {
        let budget_end = (Instant::now() + FOLLOW_UP_BUDGET).min(deadline);
        let max_body = self.limits.max_body;
        let next_limits = |session: &Session| -> Option<Limits> {
            let left = budget_end.saturating_duration_since(Instant::now());
            (!session.broken && left >= Duration::from_millis(100))
                .then(|| Limits { timeout: left.min(FOLLOW_UP_TIMEOUT), max_body })
        };
        if let Some(schema) = session.schema(INFO_TOOL) {
            let mut asked: Vec<String> = Vec::new();
            for index in 0..hits.len() {
                let needs = hits[index].url.is_none() || hits[index].title == hits[index].document;
                let document = hits[index].document.clone();
                if !needs || asked.contains(&document) || asked.len() >= MAX_INFO_CALLS {
                    continue;
                }
                asked.push(document.clone());
                let Some(args) = document_arguments(&schema, &document, None) else { break };
                let Some(limits) = next_limits(session) else { return };
                let Ok(result) = session.call(INFO_TOOL, args, Some(limits)).await else { continue };
                let Some(info) = tool_payload(&result).and_then(|p| p.as_object().cloned()) else { continue };
                let title = text_of(&info, &["title", "document_title", "name"]).map(|t| clean_line(&t, 300));
                let url = web_url(text_of(&info, &["url", "link", "source_url", "uri", "href"]));
                for hit in hits.iter_mut().filter(|h| h.document == document) {
                    if let Some(title) = title.as_ref().filter(|_| hit.title == hit.document) {
                        hit.title.clone_from(title);
                    }
                    if hit.url.is_none() {
                        hit.url.clone_from(&url);
                    }
                }
            }
        }
        if let Some(schema) = session.schema(READ_TOOL) {
            for hit in hits.iter_mut().filter(|h| h.snippet.is_empty()).take(MAX_EXCERPT_CALLS) {
                let Some(args) = document_arguments(&schema, &hit.document, hit.section.as_deref()) else { break };
                let Some(limits) = next_limits(session) else { return };
                if let Ok(result) = session.call(READ_TOOL, args, Some(limits)).await
                    && let Some(excerpt) = excerpt_of(&result)
                {
                    hit.snippet = excerpt;
                    hit.meta["excerpt"] = json!(READ_TOOL);
                }
            }
        }
    }

    /// Ausschnitt einer Fundstelle über `read_document` (höchstens [`baruch_rag::MAX_SNIPPET_CHARS`] Zeichen).
    pub async fn excerpt(&self, document: &str, section: Option<&str>) -> Result<String, RagError> {
        let mut session = self.session().await?;
        let outcome = async {
            let schema = session
                .schema(READ_TOOL)
                .ok_or_else(|| RagError::Schema(format!("MultiGPT bietet das Werkzeug {READ_TOOL} nicht an.")))?;
            let args = document_arguments(&schema, document, section).ok_or_else(|| {
                RagError::Schema(format!("{READ_TOOL} nennt im inputSchema keinen Parameter für das Dokument."))
            })?;
            let result = session.call(READ_TOOL, args, None).await?;
            excerpt_of(&result).ok_or_else(|| RagError::Schema(format!("{READ_TOOL}: kein Text in der Antwort.")))
        }
        .await;
        session.release().await;
        outcome.map(|excerpt| scrub_text(&excerpt, self.secret.as_deref()))
    }

    /// Schema, Argumente für das Dokument und ggf. der Seitenparameter von `read_document`. Verlangt das Schema
    /// etwas, das Baruch nicht kennt, ist das ehrlich [`RagError::Schema`].
    fn read_setup(session: &Session, document: &str) -> Result<(Value, Option<&'static str>), RagError> {
        let schema = session
            .schema(READ_TOOL)
            .ok_or_else(|| RagError::Schema(format!("MultiGPT bietet das Werkzeug {READ_TOOL} nicht an.")))?;
        let args = document_arguments(&schema, document, None).ok_or_else(|| {
            RagError::Schema(format!(
                "{READ_TOOL} nennt im inputSchema keinen Parameter für das Dokument ({}).",
                DOCUMENT_KEYS.join(", ")
            ))
        })?;
        let page = page_key(&schema);
        let known: Vec<&str> = args.as_object().map(|m| m.keys().map(String::as_str).collect()).unwrap_or_default();
        let required = schema.get("required").and_then(Value::as_array).cloned().unwrap_or_default();
        if let Some(unknown) =
            required.iter().filter_map(Value::as_str).find(|r| !known.contains(r) && Some(*r) != page)
        {
            return Err(RagError::Schema(format!(
                "{READ_TOOL} verlangt den Parameter „{unknown}“, den Baruch nicht kennt."
            )));
        }
        Ok((args, page))
    }

    async fn read_page(
        session: &mut Session,
        args: &Value,
        page: Option<(&'static str, usize)>,
    ) -> Result<PageAnswer, RagError> {
        read_page_with(session, args, page, Some(FULLTEXT_LIMITS)).await
    }

    async fn read_part_in(
        &self,
        session: &mut Session,
        document: &str,
        part: usize,
    ) -> Result<(FullTextPart, Option<usize>), RagError> {
        let (args, page) = MultigptWay::read_setup(session, document)?;
        let Some(key) = page else {
            let answer = MultigptWay::read_page(session, &args, None).await?;
            return local_part(document, &answer.text, part).map(|(found, offset)| (found, Some(offset)));
        };
        let answer = MultigptWay::read_page(session, &args, Some((key, part))).await?;
        if !answer.paged() {
            // Keine Seitenangaben: Die Gegenstelle lieferte das ganze Dokument – Baruch teilt es selbst.
            return local_part(document, &answer.text, part).map(|(found, offset)| (found, Some(offset)));
        }
        if let Some(total) = answer.total
            && part > total
        {
            return Err(RagError::Invalid(format!("Teil {part} gibt es nicht; das Dokument hat {total} Seiten.")));
        }
        let more = answer.more.unwrap_or_else(|| answer.total.is_some_and(|total| part < total));
        let found = FullTextPart {
            document: document.to_owned(),
            part,
            parts: answer.total,
            more,
            text: answer.text,
            paged_by_remote: true,
        };
        Ok((found, None))
    }

    async fn read_full_in(&self, session: &mut Session, document: &str) -> Result<String, RagError> {
        let (args, page) = MultigptWay::read_setup(session, document)?;
        read_all_pages(session, &args, page, Some(FULLTEXT_LIMITS), FULLTEXT_LIMITS.max_body).await
    }

    /// Angaben über `document_info`, falls angeboten; Fehler ergeben leere Angaben.
    async fn info_in(&self, session: &mut Session, document: &str) -> DocumentInfo {
        let Some(schema) = session.schema(INFO_TOOL) else { return DocumentInfo::default() };
        let Some(args) = document_arguments(&schema, document, None) else { return DocumentInfo::default() };
        let Ok(result) = session.call(INFO_TOOL, args, Some(Limits::SEARCH)).await else {
            return DocumentInfo::default();
        };
        let Some(info) = tool_payload(&result).map(|p| session.clean(p)) else { return DocumentInfo::default() };
        let Some(info) = info.as_object() else { return DocumentInfo::default() };
        DocumentInfo {
            title: text_of(info, &["title", "document_title", "name", "filename"]).map(|t| clean_line(&t, 300)),
            url: web_url(text_of(info, &["url", "link", "source_url", "uri", "href"])),
        }
    }

    /// Volltext eines Dokuments (höchstens FULLTEXT_LIMITS.max_body, `\r\n` → `\n`).
    pub async fn read_full(&self, document: &str) -> Result<String, RagError> {
        let mut session = self.session().await?;
        let outcome = self.read_full_in(&mut session, document).await;
        session.release().await;
        outcome
            .map(|text| scrub_text(&text, self.secret.as_deref()))
            .map_err(|e| scrub_error(e, self.secret.as_deref()))
    }

    /// Teil `part` (ab 1) eines Dokuments.
    pub async fn read_part(&self, document: &str, part: usize) -> Result<FullTextPart, RagError> {
        Ok(self.read_part_with_info(document, part).await?.part)
    }

    /// Wie [`MultigptWay::read_part`], dazu Titel und Adresse aus `document_info` (in derselben Sitzung) und
    /// die Stelle des Teils im Volltext.
    pub async fn read_part_with_info(&self, document: &str, part: usize) -> Result<PartReading, RagError> {
        if part == 0 {
            return Err(RagError::Invalid("part: Teile zählen ab 1.".into()));
        }
        let mut session = self.session().await?;
        let outcome = match self.read_part_in(&mut session, document, part).await {
            Ok((found, offset)) => {
                Ok(PartReading { part: found, info: self.info_in(&mut session, document).await, offset })
            }
            Err(e) => Err(e),
        };
        session.release().await;
        let secret = self.secret.as_deref();
        outcome
            .map(|mut reading| {
                reading.part.text = scrub_text(&reading.part.text, secret);
                reading
            })
            .map_err(|e| scrub_error(e, secret))
    }

    /// Angaben eines Dokuments (Titel, Adresse) über `document_info`; bietet MultiGPT es nicht an, leer.
    pub async fn info(&self, document: &str) -> Result<DocumentInfo, RagError> {
        let mut session = self.session().await?;
        let info = self.info_in(&mut session, document).await;
        session.release().await;
        Ok(info)
    }
}

impl SearchWay for MultigptWay {
    fn way(&self) -> Way {
        Way::Multigpt
    }

    fn search<'a>(&'a self, query: &'a Query) -> BoxFuture<'a, Result<Vec<Hit>, RagError>> {
        Box::pin(async move { self.run(query).await.map_err(|e| scrub_error(e, self.secret.as_deref())) })
    }
}

/// Suchweg aus den Einstellungen (Vertrag §3.3): keine Adresse → `NotConfigured`; das Token
/// `baruch/multigpt` nur, wenn es für die jetzige Adresse hinterlegt ist, sonst `NotConfigured` mit
/// Hinweis (es geht dann nirgendwohin).
pub async fn open_way(state: &AppState) -> Result<Box<dyn SearchWay>, RagError> {
    Ok(Box::new(MultigptWay::from_settings(state, Limits::SEARCH).await?))
}

/// Nachgebauter MultiGPT-MCP-Server **für Tests** (auch die des Integrators): Streamable HTTP auf
/// `127.0.0.1:<zufällig>/mcp` mit `initialize`, `notifications/initialized`, `tools/list` (Seiten über
/// `nextCursor`), `tools/call` für `search_documents`, `list_collections`, `document_info`,
/// `read_document`. Prüft Token (wenn verlangt) und `Mcp-Session-Id`; jede Anfrage landet in
/// [`MockMultigpt::requests`]. Kein Netz außer Loopback. Frei erfundene Testdaten.
///
/// Runde 13: `DELETE /mcp` beendet die Sitzung (200, in `requests()` mit `method: "DELETE"`),
/// `read_document` liefert seitenweise aus `Behavior::pages` (`{text, page, pages}`), und
/// `Behavior::tool_delays` verzögert einzelne Werkzeuge.
pub mod mock {
    use std::collections::HashMap;
    use std::net::SocketAddr;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use axum::Router;
    use axum::extract::State;
    use axum::http::{HeaderMap, StatusCode, header};
    use axum::response::{IntoResponse, Response};
    use axum::routing::post;
    use serde_json::{Value, json};

    /// Sitzung, die der Nachbau vergibt.
    pub const SESSION: &str = "nachbau-sitzung-1";

    /// Form der Antwort von `search_documents`.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum ResultForm {
        /// `structuredContent: {results: [..]}` und dasselbe als JSON-Text.
        Structured,
        /// Nur JSON-Text in `content` (`{"hits": [..]}`).
        TextJson,
        /// Ein Satz ohne JSON (unlesbar).
        Unreadable,
    }

    /// Verhalten des Nachbaus (über [`MockMultigpt::configure`] änderbar).
    #[derive(Debug, Clone)]
    pub struct Behavior {
        pub server_name: String,
        /// Werkzeuge für `tools/list` (mit `inputSchema`).
        pub tools: Vec<Value>,
        /// Werkzeuge je Seite (0 = alle auf einer).
        pub page_size: usize,
        /// Einträge für `list_collections`.
        pub collections: Vec<Value>,
        /// Treffer für `search_documents` (gefiltert nach den Sammlungen der Anfrage, falls ein Treffer
        /// `collection` nennt, und gekürzt auf die Trefferzahl).
        pub hits: Vec<Value>,
        /// Verlangtes Token (`Authorization: Bearer …`); `None` = keins.
        pub bearer: Option<String>,
        pub result: ResultForm,
        /// `search_documents` antwortet mit `isError` und diesem Text.
        pub tool_error: Option<String>,
        /// Wartezeit vor jeder Antwort auf `tools/call`.
        pub delay: Duration,
        /// Jede Anfrage → 307 mit dieser Adresse.
        pub redirect: Option<String>,
        /// Antworten als `text/event-stream`.
        pub event_stream: bool,
        /// Ereignisstrom in zwei Stücken mit kurzer Pause, getrennt mitten im ersten Mehrbyte-Zeichen.
        pub split_utf8: bool,
        /// `document_info` je Dokument.
        pub documents: HashMap<String, Value>,
        /// `read_document` je Dokument (Volltext).
        pub texts: HashMap<String, String>,
        /// `read_document` mit `page` (ab 1) je Dokument: Seiten, Antwort `{document_id, text, page, pages}`;
        /// eine Seite hinter dem Ende → `isError`. Geht vor `texts`.
        pub pages: HashMap<String, Vec<String>>,
        /// Zusätzliche Wartezeit je Werkzeug (nach `delay`).
        pub tool_delays: HashMap<String, Duration>,
        /// Sitzung, die `initialize` vergibt und die jede weitere Anfrage tragen muss (Vorgabe [`SESSION`]);
        /// ändern = die Gegenstelle „vergisst“ ältere Sitzungen (400).
        pub session_id: String,
        /// Runde 21a: Dokumente mit Seiten für `read_document {page_from, page_to}` (höchstens [`MAX_RANGE_PAGES`]
        /// Seiten und [`MAX_READ_CHARS`] Zeichen je Aufruf, Antwort `{document_id, text, page_from, page_to, pages,
        /// truncated}`); ohne Bereich der ganze Text. `upload_document` legt hier an.
        pub ranged: HashMap<String, Vec<String>>,
        /// Seiten, die ein hochgeladenes Dokument nach dem Indexieren hat (der Nachbau liest keine PDF; der Test
        /// sagt, was MultiGPT „erkannt“ hätte).
        pub upload_pages: Vec<String>,
        /// Inhaltsverzeichnis, das `document_info` für ein hochgeladenes Dokument nennt (`toc`), z. B.
        /// `{"section": "1", "title": "…", "page": 1}`.
        pub upload_toc: Vec<Value>,
        /// Titel und Autor, die `document_info` für ein hochgeladenes Dokument nennt.
        pub upload_title: Option<String>,
        pub upload_author: Option<String>,
        /// `upload_document` antwortet mit `isError` und diesem Text.
        pub upload_error: Option<String>,
        /// So oft meldet `run_status` (bzw. `read_document` ohne `run_status`) „läuft noch“, bevor das Dokument
        /// bereit ist.
        pub index_polls: usize,
        /// Antworten von `web_search` (`{"results": …}`).
        pub web_results: Vec<Value>,
        /// Gezählte Abfragen je Lauf bzw. Dokument (für `index_polls`).
        polls: HashMap<String, usize>,
        /// Laufende Nummer der hochgeladenen Dokumente.
        uploaded: usize,
        /// Hochgeladene Dokumente: (Sammlung, Dokument-ID, Dateiname) – für `list_documents`, `delete_document`.
        listed: Vec<(String, String, String)>,
    }

    /// Höchstens so viele Seiten liest `read_document` je Aufruf mit `page_from`/`page_to` (Annahme laut
    /// Werkzeugbeschreibung, Stand 2026-11-09).
    pub const MAX_RANGE_PAGES: usize = 10;
    /// Höchstens so viele Zeichen liefert `read_document` je Aufruf (Annahme laut Werkzeugbeschreibung).
    pub const MAX_READ_CHARS: usize = 12_000;

    impl Default for Behavior {
        fn default() -> Self {
            Behavior {
                server_name: "multigpt-nachbau".into(),
                tools: default_tools(),
                page_size: 0,
                collections: vec![
                    json!({ "name": "Leuchtturm", "count": 3 }),
                    json!({ "name": "Senfkorn", "count": 2 }),
                ],
                hits: Vec::new(),
                bearer: None,
                result: ResultForm::Structured,
                tool_error: None,
                delay: Duration::ZERO,
                redirect: None,
                event_stream: false,
                split_utf8: false,
                documents: HashMap::new(),
                texts: HashMap::new(),
                pages: HashMap::new(),
                tool_delays: HashMap::new(),
                session_id: SESSION.into(),
                ranged: HashMap::new(),
                upload_pages: Vec::new(),
                upload_toc: Vec::new(),
                upload_title: None,
                upload_author: None,
                upload_error: None,
                index_polls: 0,
                web_results: Vec::new(),
                polls: HashMap::new(),
                uploaded: 0,
                listed: Vec::new(),
            }
        }
    }

    /// Alle Werkzeuge der MultiGPT-Werkzeugbeschreibung (Stand 2026-11-09, Runde 21a): die neun Werkzeuge
    /// `search_documents`, `read_document` (mit `page_from`/`page_to`), `list_collections`, `document_info`,
    /// `list_documents`, `upload_document` (`collection`, `content_base64`, `filename`, `url?`), `delete_document`,
    /// `web_search`, `fetch_url` und dazu `run_status` (Fortschritt der Indexierung). `document_id` nimmt Text oder
    /// Zahl. [`default_tools`] bleibt unverändert (ältere Tests hängen an ihr).
    pub fn full_tools() -> Vec<Value> {
        let document = json!({ "type": ["string", "integer"] });
        vec![
            json!({ "name": "search_documents", "description": "Bedeutungssuche in Sammlungen",
                "inputSchema": { "type": "object", "required": ["query"], "properties": {
                    "query": { "type": "string" },
                    "collections": { "type": "array", "items": { "type": "string" } },
                    "limit": { "type": "integer", "minimum": 1, "maximum": 50 } } } }),
            json!({ "name": "read_document", "description": "Text eines Dokuments, höchstens 12000 Zeichen bzw. 10 Seiten je Aufruf",
                "inputSchema": { "type": "object", "required": ["document_id"], "properties": {
                    "document_id": document, "page_from": { "type": "integer", "minimum": 1 },
                    "page_to": { "type": "integer", "minimum": 1 }, "paragraph_from": { "type": "integer" },
                    "paragraph_to": { "type": "integer" }, "section": { "type": "string" } } } }),
            json!({ "name": "list_collections", "inputSchema": { "type": "object", "properties": {} } }),
            json!({ "name": "document_info", "description": "Angaben und erkanntes Inhaltsverzeichnis eines Dokuments",
                "inputSchema": { "type": "object", "required": ["document_id"],
                "properties": { "document_id": document } } }),
            json!({ "name": "list_documents", "inputSchema": { "type": "object", "required": ["collection"],
                "properties": { "collection": { "type": "string" }, "limit": { "type": "integer" },
                    "cursor": { "type": "string" } } } }),
            json!({ "name": "upload_document", "description": "Datei hochladen (PDF, DOCX, TXT, MD, Bilder); Indexierung läuft nach",
                "inputSchema": { "type": "object", "required": ["collection", "content_base64", "filename"],
                "properties": { "collection": { "type": "string", "description": "ID oder Name der Sammlung" },
                    "content_base64": { "type": "string" }, "filename": { "type": "string" },
                    "url": { "type": "string" } } } }),
            json!({ "name": "delete_document", "inputSchema": { "type": "object", "required": ["document_id"],
                "properties": { "document_id": document } } }),
            json!({ "name": "web_search", "inputSchema": { "type": "object", "required": ["query"],
                "properties": { "query": { "type": "string" }, "limit": { "type": "integer" } } } }),
            json!({ "name": "fetch_url", "inputSchema": { "type": "object", "required": ["url"],
                "properties": { "url": { "type": "string" } } } }),
            json!({ "name": "run_status", "description": "Fortschritt eines Laufs",
                "inputSchema": { "type": "object", "required": ["run_id"],
                "properties": { "run_id": { "type": "string" } } } }),
        ]
    }

    /// Die vier Werkzeuge wie bei MultiGPT (Schema-Variante „query/collections/limit“).
    pub fn default_tools() -> Vec<Value> {
        vec![
            json!({ "name": "search_documents", "description": "Bedeutungssuche in Sammlungen",
                "inputSchema": { "type": "object", "required": ["query"], "properties": {
                    "query": { "type": "string" },
                    "collections": { "type": "array", "items": { "type": "string" } },
                    "limit": { "type": "integer", "minimum": 1, "maximum": 50 } } } }),
            json!({ "name": "list_collections", "inputSchema": { "type": "object", "properties": {} } }),
            json!({ "name": "document_info", "inputSchema": { "type": "object", "required": ["document_id"],
                "properties": { "document_id": { "type": "string" } } } }),
            json!({ "name": "read_document", "inputSchema": { "type": "object", "required": ["document_id"],
                "properties": { "document_id": { "type": "string" }, "page": { "type": "integer" } } } }),
        ]
    }

    #[derive(Default)]
    struct Shared {
        behavior: Mutex<Behavior>,
        requests: Mutex<Vec<Value>>,
    }

    /// Laufender Nachbau; endet mit dem Fallenlassen.
    pub struct MockMultigpt {
        addr: SocketAddr,
        shared: Arc<Shared>,
        task: tokio::task::JoinHandle<()>,
    }

    impl Drop for MockMultigpt {
        fn drop(&mut self) {
            self.task.abort();
        }
    }

    impl MockMultigpt {
        /// Auf `127.0.0.1:0` starten.
        pub async fn start() -> MockMultigpt {
            let shared = Arc::new(Shared::default());
            let app = Router::new().route("/mcp", post(handle).delete(end_session)).with_state(shared.clone());
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("Nachbau: kein Port");
            let addr = listener.local_addr().expect("Nachbau: Adresse");
            let task = tokio::spawn(async move {
                let _ = axum::serve(listener, app).await;
            });
            MockMultigpt { addr, shared, task }
        }

        /// `http://127.0.0.1:<port>/mcp`
        pub fn url(&self) -> String {
            format!("http://{}/mcp", self.addr)
        }

        pub fn configure(&self, change: impl FnOnce(&mut Behavior)) {
            change(&mut self.shared.behavior.lock().expect("Nachbau"));
        }

        pub fn set_tools(&self, tools: Vec<Value>) {
            self.configure(|b| b.tools = tools);
        }

        pub fn set_collections(&self, collections: Vec<Value>) {
            self.configure(|b| b.collections = collections);
        }

        pub fn set_hits(&self, hits: Vec<Value>) {
            self.configure(|b| b.hits = hits);
        }

        pub fn require_bearer(&self, token: Option<&str>) {
            self.configure(|b| b.bearer = token.map(str::to_owned));
        }

        /// Alle Anfragen: `{method, params, session, protocol, authorization}` (`authorization` = ob eine
        /// Kopfzeile kam und ob sie passte: `"keine"`, `"passt"`, `"falsch"`).
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
    }

    fn answer(id: &Value, result: Value) -> Value {
        json!({ "jsonrpc": "2.0", "id": id, "result": result })
    }

    fn tool_text(value: &Value, structured: bool) -> Value {
        let mut out = json!({ "content": [{ "type": "text", "text": value.to_string() }], "isError": false });
        if structured {
            out["structuredContent"] = value.clone();
        }
        out
    }

    fn tool_error(text: &str) -> Value {
        json!({ "content": [{ "type": "text", "text": text }], "isError": true })
    }

    fn string_list(value: &Value) -> Option<Vec<String>> {
        match value {
            Value::String(s) => Some(vec![s.clone()]),
            Value::Number(n) => Some(vec![n.to_string()]),
            Value::Array(items) => {
                Some(items.iter().map(|v| v.as_str().map_or_else(|| v.to_string(), str::to_owned)).collect())
            }
            _ => None,
        }
    }

    /// Text eines Arguments (Zahl als Text), sonst `None`.
    fn arg_text(args: &Value, key: &str) -> Option<String> {
        match args.get(key)? {
            Value::String(s) => Some(s.clone()),
            Value::Number(n) => Some(n.to_string()),
            _ => None,
        }
    }

    /// Werkzeuge mit Zustand (Runde 21a): Hochladen, Lauf, seitenweises Lesen, Liste, Löschen, Websuche, Abruf.
    /// `None` = kein solches Werkzeug bzw. kein solches Dokument (dann antwortet [`call`]).
    fn call_stateful(behavior: &mut Behavior, name: &str, args: &Value) -> Option<Value> {
        match name {
            "upload_document" => {
                if let Some(text) = &behavior.upload_error {
                    return Some(tool_error(text));
                }
                let (Some(collection), Some(content), Some(filename)) =
                    (arg_text(args, "collection"), arg_text(args, "content_base64"), arg_text(args, "filename"))
                else {
                    return Some(tool_error("collection, content_base64 und filename sind Pflicht"));
                };
                if baruch_store::decode_base64(&content).is_err() {
                    return Some(tool_error("content_base64 ist kein gültiges Base64"));
                }
                let lower = filename.to_lowercase();
                if lower.ends_with(".epub") {
                    return Some(tool_error("Dateityp nicht unterstützt (PDF, DOCX, TXT, MD, Bilder)"));
                }
                behavior.uploaded += 1;
                let id = format!("up-{}", behavior.uploaded);
                let run = format!("lauf-{}", behavior.uploaded);
                behavior.ranged.insert(id.clone(), behavior.upload_pages.clone());
                let mut info = json!({ "document_id": id, "filename": filename, "collection": collection,
                    "pages": behavior.upload_pages.len(), "toc": behavior.upload_toc });
                if let Some(title) = &behavior.upload_title {
                    info["title"] = json!(title);
                }
                if let Some(author) = &behavior.upload_author {
                    info["author"] = json!(author);
                }
                behavior.documents.insert(id.clone(), info);
                behavior.listed.push((collection, id.clone(), filename));
                Some(tool_text(&json!({ "document_id": id, "run_id": run, "status": "queued" }), true))
            }
            "run_status" => {
                let run = arg_text(args, "run_id")?;
                let seen = behavior.polls.entry(run.clone()).or_insert(0);
                *seen += 1;
                let done = *seen > behavior.index_polls;
                Some(tool_text(
                    &json!({ "run_id": run, "status": if done { "done" } else { "running" },
                        "progress": if done { 100 } else { 50 } }),
                    true,
                ))
            }
            "read_document" => {
                let document = arg_text(args, "document_id")?;
                let pages = behavior.ranged.get(&document)?.clone();
                // Ohne run_status zählt das Lesen als Abfrage, ob die Indexierung fertig ist.
                let has_run_status = behavior.tools.iter().any(|t| t["name"] == "run_status");
                if !has_run_status {
                    let seen = behavior.polls.entry(document.clone()).or_insert(0);
                    *seen += 1;
                    if *seen <= behavior.index_polls {
                        return Some(tool_error("Dokument wird noch indexiert"));
                    }
                }
                let from = args.get("page_from").and_then(Value::as_u64).map(|n| n as usize);
                let to = args.get("page_to").and_then(Value::as_u64).map(|n| n as usize);
                let (from, to) = match (from, to) {
                    (None, None) => (1, pages.len().max(1)),
                    (Some(f), None) => (f, f),
                    (None, Some(t)) => (1, t),
                    (Some(f), Some(t)) => (f, t),
                };
                if from == 0 || to < from || (from > pages.len() && !pages.is_empty()) {
                    return Some(tool_error(&format!("Seiten {from}–{to} gibt es nicht ({} Seiten)", pages.len())));
                }
                let ranged = args.get("page_from").is_some() || args.get("page_to").is_some();
                if ranged && to - from + 1 > MAX_RANGE_PAGES {
                    return Some(tool_error(&format!("höchstens {MAX_RANGE_PAGES} Seiten je Aufruf")));
                }
                let to = to.min(pages.len());
                let joined = pages.get(from.saturating_sub(1)..to).map(|p| p.join("\n\n")).unwrap_or_default();
                let truncated = joined.chars().count() > MAX_READ_CHARS;
                let text: String = joined.chars().take(MAX_READ_CHARS).collect();
                Some(tool_text(
                    &json!({ "document_id": document, "text": text, "page_from": from, "page_to": to,
                        "pages": pages.len(), "truncated": truncated }),
                    true,
                ))
            }
            "list_documents" => {
                let collection = arg_text(args, "collection").unwrap_or_default();
                let documents: Vec<Value> = behavior
                    .listed
                    .iter()
                    .filter(|(c, ..)| *c == collection)
                    .map(|(c, id, name)| json!({ "document_id": id, "title": name, "collection": c }))
                    .collect();
                Some(tool_text(&json!({ "documents": documents, "has_more": false }), true))
            }
            "delete_document" => {
                let document = arg_text(args, "document_id")?;
                let before = behavior.listed.len();
                behavior.listed.retain(|(_, id, _)| *id != document);
                behavior.ranged.remove(&document);
                behavior.documents.remove(&document);
                Some(if behavior.listed.len() < before {
                    tool_text(&json!({ "deleted": true, "document_id": document }), true)
                } else {
                    tool_error("Dokument unbekannt")
                })
            }
            "web_search" => Some(tool_text(&json!({ "results": behavior.web_results }), true)),
            // Der Nachbau ruft nie eine Adresse ab (kein Internet in Tests).
            "fetch_url" => Some(tool_error("Der Nachbau ruft keine Adressen ab.")),
            _ => None,
        }
    }

    fn call(behavior: &mut Behavior, name: &str, args: &Value) -> Value {
        if let Some(answer) = call_stateful(behavior, name, args) {
            return answer;
        }
        let arg = |keys: &[&str]| keys.iter().find_map(|k| args.get(*k)).cloned().unwrap_or(Value::Null);
        match name {
            "search_documents" => {
                if let Some(text) = &behavior.tool_error {
                    return tool_error(text);
                }
                let wanted = string_list(&arg(super::COLLECTION_KEYS));
                let limit = arg(super::LIMIT_KEYS).as_u64().map_or(usize::MAX, |n| n as usize);
                let hits: Vec<Value> = behavior
                    .hits
                    .iter()
                    .filter(|h| match (h.get("collection").and_then(Value::as_str), &wanted) {
                        (Some(c), Some(wanted)) => wanted.iter().any(|w| w == c),
                        _ => true,
                    })
                    .take(limit)
                    .cloned()
                    .collect();
                match behavior.result {
                    ResultForm::Structured => tool_text(&json!({ "results": hits }), true),
                    ResultForm::TextJson => tool_text(&json!({ "hits": hits }), false),
                    ResultForm::Unreadable => json!({ "content": [{ "type": "text",
                        "text": "Es gibt ein paar Fundstellen, aber hier steht kein JSON." }], "isError": false }),
                }
            }
            "list_collections" => tool_text(&json!({ "collections": behavior.collections }), true),
            "document_info" => {
                let document = string_list(&arg(super::DOCUMENT_KEYS)).and_then(|d| d.into_iter().next());
                match document.and_then(|d| behavior.documents.get(&d)) {
                    Some(info) => tool_text(info, true),
                    None => tool_error("Dokument unbekannt"),
                }
            }
            "read_document" => {
                let document = string_list(&arg(super::DOCUMENT_KEYS)).and_then(|d| d.into_iter().next());
                if let Some((id, pages)) = document.as_ref().and_then(|d| behavior.pages.get_key_value(d)) {
                    let page = args.get("page").and_then(Value::as_u64).unwrap_or(1) as usize;
                    return match pages.get(page.wrapping_sub(1)) {
                        Some(text) => tool_text(
                            &json!({ "document_id": id, "text": text, "page": page, "pages": pages.len() }),
                            true,
                        ),
                        None => tool_error(&format!("Seite {page} gibt es nicht")),
                    };
                }
                match document.and_then(|d| behavior.texts.get(&d)) {
                    Some(text) => json!({ "content": [{ "type": "text", "text": text }], "isError": false }),
                    None => tool_error("Dokument unbekannt"),
                }
            }
            other => tool_error(&format!("Unbekanntes Werkzeug {other}")),
        }
    }

    fn respond(behavior: &Behavior, body: Value, session: bool) -> Response {
        let mut response = if behavior.event_stream {
            let text = format!(
                "event: message\ndata: {{\"jsonrpc\":\"2.0\",\"method\":\"notifications/message\",\"params\":{{}}}}\n\n\
                 event: message\ndata: {body}\n\n"
            );
            if behavior.split_utf8 {
                split_stream(text.into_bytes())
            } else {
                ([(header::CONTENT_TYPE, "text/event-stream")], text).into_response()
            }
        } else {
            axum::Json(body).into_response()
        };
        if session {
            response.headers_mut().insert("mcp-session-id", behavior.session_id.parse().expect("Kopfzeile"));
        }
        response
    }

    /// Rumpf in zwei Stücken: das erste endet nach dem ersten Byte des ersten Mehrbyte-Zeichens (sonst in
    /// der Mitte), das zweite folgt nach 50 ms – so kommt es beim Client sicher getrennt an.
    fn split_stream(bytes: Vec<u8>) -> Response {
        let cut = bytes.iter().position(|b| *b >= 0x80).map_or(bytes.len() / 2, |i| i + 1);
        let (tx, rx) = tokio::sync::mpsc::channel::<Result<axum::body::Bytes, std::convert::Infallible>>(2);
        let second = bytes[cut..].to_vec();
        let first = bytes[..cut].to_vec();
        tokio::spawn(async move {
            let _ = tx.send(Ok(first.into())).await;
            tokio::time::sleep(Duration::from_millis(50)).await;
            let _ = tx.send(Ok(second.into())).await;
        });
        let body = axum::body::Body::from_stream(tokio_stream::wrappers::ReceiverStream::new(rx));
        ([(header::CONTENT_TYPE, "text/event-stream")], body).into_response()
    }

    /// `DELETE /mcp`: Sitzung beenden (immer 200), aufgezeichnet wie jede Anfrage.
    async fn end_session(State(shared): State<Arc<Shared>>, headers: HeaderMap) -> Response {
        let text = |name: &str| headers.get(name).and_then(|v| v.to_str().ok()).map(str::to_owned);
        let behavior = shared.behavior.lock().expect("Nachbau").clone();
        let authorization = match (text("authorization"), &behavior.bearer) {
            (None, _) => "keine",
            (Some(g), Some(t)) if g == format!("Bearer {t}") => "passt",
            (Some(_), None) => "passt",
            (Some(_), Some(_)) => "falsch",
        };
        shared.requests.lock().expect("Nachbau").push(json!({
            "method": "DELETE", "params": null, "session": text("mcp-session-id"),
            "protocol": text("mcp-protocol-version"), "authorization": authorization,
        }));
        StatusCode::OK.into_response()
    }

    async fn handle(State(shared): State<Arc<Shared>>, headers: HeaderMap, body: String) -> Response {
        let behavior = shared.behavior.lock().expect("Nachbau").clone();
        let message: Value = serde_json::from_str(&body).unwrap_or(Value::Null);
        let given = headers.get(header::AUTHORIZATION).and_then(|v| v.to_str().ok()).map(str::to_owned);
        let authorization = match (&given, &behavior.bearer) {
            (None, _) => "keine",
            (Some(g), Some(t)) if *g == format!("Bearer {t}") => "passt",
            (Some(_), None) => "passt",
            (Some(_), Some(_)) => "falsch",
        };
        let text = |name: &str| headers.get(name).and_then(|v| v.to_str().ok()).map(str::to_owned);
        shared.requests.lock().expect("Nachbau").push(json!({
            "method": message["method"], "params": message["params"], "session": text("mcp-session-id"),
            "protocol": text("mcp-protocol-version"), "authorization": authorization,
        }));
        if let Some(to) = &behavior.redirect {
            return (StatusCode::TEMPORARY_REDIRECT, [(header::LOCATION, to.clone())]).into_response();
        }
        let authorized = match &behavior.bearer {
            None => true,
            Some(_) => authorization == "passt",
        };
        if !authorized {
            return (StatusCode::UNAUTHORIZED, [(header::WWW_AUTHENTICATE, "Bearer")], "Token fehlt oder falsch")
                .into_response();
        }
        let method = message["method"].as_str().unwrap_or("").to_owned();
        let id = message.get("id").cloned();
        if method == "initialize" {
            let result = json!({ "protocolVersion": "2025-06-18", "capabilities": { "tools": {} },
                "serverInfo": { "name": behavior.server_name, "version": "0.1" } });
            return respond(&behavior, answer(&id.unwrap_or(Value::Null), result), true);
        }
        if text("mcp-session-id").as_deref() != Some(behavior.session_id.as_str()) {
            return (StatusCode::BAD_REQUEST, "Mcp-Session-Id fehlt").into_response();
        }
        let Some(id) = id else { return StatusCode::ACCEPTED.into_response() };
        let result = match method.as_str() {
            "tools/list" => {
                let size = if behavior.page_size == 0 { behavior.tools.len().max(1) } else { behavior.page_size };
                let page: usize = message["params"]["cursor"]
                    .as_str()
                    .and_then(|c| c.strip_prefix("seite-"))
                    .and_then(|n| n.parse().ok())
                    .unwrap_or(0);
                let tools: Vec<Value> = behavior.tools.iter().skip(page * size).take(size).cloned().collect();
                let mut result = json!({ "tools": tools });
                if (page + 1) * size < behavior.tools.len() {
                    result["nextCursor"] = json!(format!("seite-{}", page + 1));
                }
                result
            }
            "tools/call" => {
                if !behavior.delay.is_zero() {
                    tokio::time::sleep(behavior.delay).await;
                }
                let name = message["params"]["name"].as_str().unwrap_or("");
                if let Some(extra) = behavior.tool_delays.get(name) {
                    tokio::time::sleep(*extra).await;
                }
                // Werkzeuge mit Zustand (Hochladen, Lauf …) ändern das gemeinsame Verhalten.
                let mut shared_behavior = shared.behavior.lock().expect("Nachbau");
                call(&mut shared_behavior, name, &message["params"]["arguments"])
            }
            other => {
                let error = json!({ "jsonrpc": "2.0", "id": id,
                    "error": { "code": -32601, "message": format!("unbekannte Methode {other}") } });
                return respond(&behavior, error, false);
            }
        };
        respond(&behavior, answer(&id, result), false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arguments_follow_the_input_schema() {
        let collections = vec!["Leuchtturm".to_owned(), "Senfkorn".to_owned()];
        let calls = search_arguments(&json!({}), "Licht", &collections, 5).unwrap();
        assert_eq!(
            calls,
            vec![(json!({ "query": "Licht", "collections": ["Leuchtturm", "Senfkorn"], "limit": 5 }), None)],
            "ohne properties gelten die Vorgaben query/collections/limit"
        );
        let schema = json!({ "properties": { "q": { "type": "string" }, "collection_id": { "type": "integer" },
            "top_k": { "type": "integer", "maximum": 3 } } });
        let calls = search_arguments(&schema, "Licht", &["7".to_owned(), "Senfkorn".to_owned()], 5).unwrap();
        assert_eq!(
            calls,
            vec![
                (json!({ "q": "Licht", "collection_id": 7, "top_k": 3 }), Some("7".to_owned())),
                (json!({ "q": "Licht", "collection_id": "Senfkorn", "top_k": 3 }), Some("Senfkorn".to_owned())),
            ],
            "eine Sammlung je Aufruf, Zahl wo das Schema integer verlangt, Trefferzahl an maximum geklemmt"
        );
        let missing = search_arguments(&json!({ "properties": { "query": {} } }), "x", &collections, 5);
        assert_eq!(missing.unwrap_err().code(), "schema", "ohne Sammlungs-Parameter wird nicht gesucht");
        let unknown = json!({ "properties": { "query": {}, "collections": { "type": "array" }, "user": {} },
            "required": ["query", "user"] });
        assert_eq!(
            search_arguments(&unknown, "x", &collections, 5).unwrap_err().code(),
            "schema",
            "unbekannte Pflicht"
        );
    }

    #[test]
    fn hits_carry_source_details_and_a_clamped_or_ranked_score() {
        let payload = json!({ "results": [
            { "title": "Das Jahr am Leuchtturm", "document_id": "d1", "collection": "Leuchtturm", "page": 4,
              "text": "Das Licht\u{7} kreist.", "score": 1.7, "url": "https://bib.example/d1" },
            { "metadata": { "filename": "senfkorn.pdf" }, "content": "Klein", "url": "javascript:alert(1)" },
            { "nichts": true },
        ] });
        let hits = parse_hits(&payload, Some("Senfkorn")).unwrap();
        assert_eq!(hits.len(), 2, "der Eintrag ohne Dokument und Titel fällt heraus");
        assert_eq!(
            (hits[0].title.as_str(), hits[0].document.as_str(), hits[0].section.as_deref(), hits[0].score),
            ("Das Jahr am Leuchtturm", "d1", Some("4"), 1.0),
            "Titel, Dokument, Stelle; Score über 1 geklemmt"
        );
        assert_eq!(hits[0].snippet, "Das Licht  kreist.", "Steuerzeichen im Ausschnitt ersetzt");
        assert_eq!(hits[0].url.as_deref(), Some("https://bib.example/d1"), "http(s)-Adresse bleibt");
        assert_eq!(
            (hits[1].document.as_str(), hits[1].collection.as_str(), hits[1].score, hits[1].url.as_deref()),
            ("senfkorn.pdf", "Senfkorn", rank_score(2) as f32, None),
            "Dateiname aus metadata als Dokument, Sammlung des Aufrufs, fehlender Score = Ersatz aus dem Rang, keine \
             javascript:-Adresse"
        );
        assert_eq!(hits[1].meta["score"], "aus Rang", "der Ersatz bleibt als solcher erkennbar");
        assert_eq!(parse_hits(&json!({ "x": 1 }), None).unwrap_err().code(), "schema", "keine Liste → Schema");
        assert_eq!(parse_hits(&json!([{ "a": 1 }]), None).unwrap_err().code(), "schema", "nur Unlesbares → Schema");
        assert_eq!(parse_hits(&json!({ "results": [] }), None).unwrap(), vec![], "leere Liste ist eine Antwort");
    }

    #[test]
    fn hits_without_score_or_distance_are_never_weak_by_rank() {
        // Gutachten R19: Ohne Score galt jeder Treffer ab Rang 3 als „schwach“ (1/Rang < 0,45) – eine Aussage über
        // die Ähnlichkeit, die MultiGPT nie gemacht hat.
        let payload = json!({ "results": [
            { "document_id": "a", "title": "Erster" },
            { "document_id": "b", "title": "Zweiter" },
            { "document_id": "c", "title": "Dritter" },
        ] });
        let hits = parse_hits(&payload, Some("Watt")).unwrap();
        assert_eq!(hits.len(), 3, "drei Treffer ohne Score");
        for hit in &hits {
            assert!(!baruch_rag::is_weak(hit.score), "Rang-Ersatz ist nie schwach: {} hat {}", hit.document, hit.score);
            assert_eq!(hit.meta["score"], "aus Rang", "Herkunft der Zahl bleibt sichtbar: {}", hit.document);
        }
        assert!(
            hits[0].score > hits[1].score && hits[1].score > hits[2].score,
            "die Reihenfolge der Gegenstelle bleibt in den Zahlen erhalten: {:?}",
            hits.iter().map(|h| h.score).collect::<Vec<_>>()
        );
        assert_eq!(hits[0].score, 1.0, "Rang 1 bekommt die volle Zahl");
        assert!(!baruch_rag::is_weak(rank_score(10_000) as f32), "auch ein sehr später Rang bleibt über der Schwelle");
    }

    #[test]
    fn payloads_and_collections_are_read_from_structured_content_or_text() {
        let structured = json!({ "structuredContent": { "results": [] }, "content": [] });
        assert_eq!(tool_payload(&structured), Some(json!({ "results": [] })), "structuredContent zuerst");
        let text =
            json!({ "content": [{ "type": "text", "text": "Vorrede" }, { "type": "text", "text": " [1, 2] " }] });
        assert_eq!(tool_payload(&text), Some(json!([1, 2])), "erstes JSON in content");
        assert_eq!(tool_payload(&json!({ "content": [{ "type": "text", "text": "kein JSON" }] })), None, "unlesbar");
        let list = json!({ "collections": ["A", { "name": "B", "documents": 4 }, { "id": 7 }, "A"] });
        assert_eq!(
            parse_collections(&list),
            vec![
                json!({ "name": "A", "count": null }),
                json!({ "name": "B", "count": 4 }),
                json!({ "name": "7", "count": null })
            ],
            "Namen und Zahlen in gemeinsamer Form, ohne Doppelte"
        );
    }

    #[test]
    fn a_stream_cut_inside_a_multibyte_character_is_not_an_error() {
        let full = "event: message\ndata: {\"jsonrpc\":\"2.0\",\"id\":3,\"result\":{\"titel\":\"Möwen\"}}\n\n";
        let bytes = full.as_bytes();
        let cut = full.find('ö').unwrap() + 1;
        // Begründung: Bis hierher fehlt das zweite Byte von „ö“ – das heißt „noch nicht da“, nicht „kaputt“.
        assert!(find_answer("text/event-stream", &bytes[..cut], 3).is_none(), "halbes Zeichen am Ende: weiterlesen");
        assert_eq!(
            find_answer("text/event-stream", bytes, 3).unwrap().unwrap(),
            json!({ "titel": "Möwen" }),
            "mit dem nächsten Stück ist die Antwort vollständig"
        );
        let broken = [b"data: {\"id\":3,\"result\":1}\n\n".as_slice(), &[0xFF, b'x']].concat();
        assert_eq!(
            find_answer("text/event-stream", &broken, 3).unwrap().unwrap_err().code(),
            "schema",
            "ungültige Bytes mitten im Text bleiben ein Fehler"
        );
        assert_eq!(
            find_answer("application/json", &bytes[..cut], 3).unwrap().unwrap_err().code(),
            "schema",
            "eine fertige JSON-Antwort mit halbem Zeichen ist kaputt"
        );
    }

    #[test]
    fn pages_are_recognised_only_with_a_numeric_parameter_and_page_details_in_the_answer() {
        let paged = json!({ "properties": { "document_id": {}, "page": { "type": "integer" } } });
        assert_eq!(page_key(&paged), Some("page"), "page gilt immer als Seitenparameter");
        let chunk = json!({ "properties": { "document_id": {}, "chunk_id": { "type": "string" } } });
        assert_eq!(page_key(&chunk), None, "eine Text-ID (chunk_id) ist keine Seite");
        let numbered = json!({ "properties": { "id": {}, "position": { "type": ["integer", "null"] } } });
        assert_eq!(page_key(&numbered), Some("position"), "Name aus SECTION_KEYS mit Zahl");

        let whole = json!({ "content": [{ "type": "text", "text": "Ganzer Text.\r\nZweite Zeile." }] });
        let answer = page_answer(&whole).unwrap();
        assert_eq!(answer.text, "Ganzer Text.\nZweite Zeile.", "Text aus content, \\r\\n vereinheitlicht");
        assert!(!answer.paged(), "ohne Seitenangaben ist die Antwort das ganze Dokument");
        let page = json!({ "structuredContent": { "document": { "text": " Seite 2 ", "total_pages": 3 } } });
        let answer = page_answer(&page).unwrap();
        assert_eq!((answer.text.as_str(), answer.total), (" Seite 2 ", Some(3)), "Text unbeschnitten, Seitenzahl");
        let last = json!({ "structuredContent": { "content": "Ende", "next_page": null } });
        assert_eq!(page_answer(&last).unwrap().more, Some(false), "next_page: null heißt letzte Seite");
        let no_text = json!({ "structuredContent": { "title": "nur Angaben" } });
        assert_eq!(page_answer(&no_text).err().map(|e| e.code()), Some("schema"), "Daten ohne Text → Schema");
    }

    #[test]
    fn the_secret_is_removed_from_errors() {
        let e = scrub_error(RagError::Denied("Token geheim-123 abgelehnt".into()), Some("geheim-123"));
        assert_eq!(e, RagError::Denied("Token *** abgelehnt".into()), "gleiche Art, Token ersetzt");
    }
}
