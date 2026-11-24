// SPDX-License-Identifier: AGPL-3.0-or-later
//! Nachbau von Eliezer für Tests (nur Tests; nie im Betrieb): ein kleiner HTTP/1.1-Server auf
//! `127.0.0.1:0` mit `GET /v1/models` und gestreamtem `POST /v1/chat/completions`.
//!
//! - Antworten je Chat-Aufruf der Reihe nach aus [`MockEliezer::push`] (danach [`Reply::text`] mit „OK“):
//!   SSE-Stücke mit Pausen ([`Reply::Stream`]), Fehlercode ([`Reply::Status`]), Weiterleitung
//!   ([`Reply::Redirect`]), ganze JSON-Antwort ([`Reply::Json`]) oder „hängt bis Abbruch“ ([`Reply::Hang`]).
//! - Prüft den Bearer ([`MockEliezer::require_bearer`]); bei falschem Schlüssel antwortet er 401 und
//!   **spiegelt den vorgelegten Schlüssel** in der Meldung – Baruch darf ihn nicht weitergeben.
//! - Zeichnet jede Anfrage auf ([`MockEliezer::requests`]), zählt gleichzeitige Chat-Aufrufe
//!   ([`MockEliezer::max_active`]) und Abbrüche durch den Client ([`MockEliezer::cancelled`]).
//!
//! - Runde 12: `POST /v1/embeddings` antwortet deterministisch ([`mock_vector`]: Wörter in Fächer
//!   gestreut, normiert – Texte mit gemeinsamen Wörtern sind sich ähnlich) für die Modelle aus
//!   [`MockEliezer::set_embedding_models`] (Vorgabe `text-embedding-nomic-embed-text-v1.5`, 512
//!   Dimensionen), sonst 404 „model not found“. Die Antwort kommt absichtlich in umgekehrter Reihenfolge
//!   (mit `index`). Gezählt werden gleichzeitige Einbettungsanfragen ([`MockEliezer::embed_max_active`],
//!   getrennt von den Chat-Aufrufen) und alle eingebetteten Texte ([`MockEliezer::embedding_inputs`]).
//!
//! - Runde 15b: Art der Gegenstelle ([`MockEliezer::set_flavor`], [`Flavor`]): Eliezer meldet die
//!   Kontextlänge über `GET /api/v1/models` (Verwaltungsliste wie in Eliezers `docs/wiki/API.md`), Ollama über
//!   `POST /api/show` (`parameters` mit `num_ctx`, ohne gesetzte Länge keins; `GET /api/ps` meldet kein geladenes
//!   Modell), OpenAI-kompatibel gar nicht (404). Die Längen setzt
//!   [`MockEliezer::set_context_length`]. Mit [`MockEliezer::set_keep_alive`] bleiben Verbindungen nach
//!   JSON-Antworten offen (HTTP/1.1 keep-alive); [`MockEliezer::connections`] zählt angenommene
//!   TCP-Verbindungen – so lässt sich prüfen, ob ein Client seinen HTTP-Client wiederverwendet.
//!   [`MockEliezer::fail_models`] lässt `GET /v1/models` scheitern.
//!
//! Ein Chat-Aufruf gilt als „aktiv“ vom Eingang der Anfrage bis **vor** dem letzten geschriebenen
//! Stück: Wer das letzte Stück (`[DONE]`) gelesen hat, ist beim Nachbau schon nicht mehr aktiv.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Notify;
use tokio::task::JoinHandle;

/// Ein Schritt einer gestreamten Antwort.
#[derive(Debug, Clone)]
pub enum Step {
    /// Rohe Bytes schreiben (ein TCP-Paket, `nodelay`).
    Send(Vec<u8>),
    /// Warten (bricht der Client dabei ab, zählt das als Abbruch).
    Pause(Duration),
}

/// Antwort auf einen Chat-Aufruf.
#[derive(Debug, Clone)]
pub enum Reply {
    /// `200 text/event-stream` und die Schritte.
    Stream(Vec<Step>),
    /// Fehlercode mit Rumpf (JSON oder Text).
    Status(u16, String),
    /// `302` mit `Location`.
    Redirect(String),
    /// `200 application/json` mit ganzem Rumpf (Gegenstelle ohne Strom).
    Json(Value),
    /// Keine Antwort, bis der Client die Verbindung schließt.
    Hang,
}

/// Ein SSE-Ereignis `data: <json>\n\n`.
pub fn sse(value: &Value) -> Vec<u8> {
    format!("data: {value}\n\n").into_bytes()
}

/// Ein Textstück als Ereignis.
pub fn delta(model: &str, text: &str) -> Vec<u8> {
    sse(&json!({ "id": "c1", "object": "chat.completion.chunk", "model": model,
                 "choices": [{ "index": 0, "delta": { "content": text }, "finish_reason": null }] }))
}

/// Schluss: `finish_reason`, `usage`, `[DONE]`.
pub fn finish(model: &str, reason: &str, prompt: u32, completion: u32) -> Vec<Vec<u8>> {
    vec![
        sse(&json!({ "id": "c1", "model": model, "choices": [{ "index": 0, "delta": {}, "finish_reason": reason }] })),
        sse(&json!({ "id": "c1", "model": model, "choices": [],
                     "usage": { "prompt_tokens": prompt, "completion_tokens": completion, "total_tokens": prompt + completion } })),
        b"data: [DONE]\n\n".to_vec(),
    ]
}

impl Reply {
    /// Text in Stücken (Modell `mock-modell`), dann `stop`, usage (11/Zahl der Stücke) und `[DONE]`;
    /// zwischen den Stücken 15 ms Pause, damit sie als eigene Pakete ankommen.
    pub fn text(pieces: &[&str]) -> Reply {
        Reply::text_from("mock-modell", pieces, Duration::from_millis(15))
    }

    /// Wie [`Reply::text`] mit Modell und Pause.
    pub fn text_from(model: &str, pieces: &[&str], pause: Duration) -> Reply {
        let mut steps = Vec::new();
        for piece in pieces {
            steps.push(Step::Send(delta(model, piece)));
            if !pause.is_zero() {
                steps.push(Step::Pause(pause));
            }
        }
        for end in finish(model, "stop", 11, pieces.len() as u32) {
            steps.push(Step::Send(end));
        }
        Reply::Stream(steps)
    }
}

/// Eine aufgezeichnete Anfrage.
#[derive(Debug, Clone, PartialEq)]
pub struct Seen {
    pub method: String,
    pub path: String,
    /// Kopfzeile `Authorization` (in Tests nur mit Test-Schlüsseln).
    pub authorization: Option<String>,
    /// Rumpf als JSON (falls einer kam).
    pub body: Option<Value>,
}

/// Art der nachgebauten Gegenstelle (Runde 15b).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Flavor {
    /// Eliezer: `GET /api/v1/models` mit Kontextlängen.
    #[default]
    Eliezer,
    /// Ollama: `POST /api/show` mit `num_ctx`.
    Ollama,
    /// OpenAI-kompatibel: keine Verwaltungs-Schnittstelle.
    Openai,
}

/// Vorgabe der Dimension der Einbettungen des Nachbaus.
pub const MOCK_DIMENSION: usize = 512;
/// Vorgabe-Einbettungsmodell des Nachbaus (wie bei Eliezer).
pub const MOCK_EMBEDDING_MODEL: &str = "text-embedding-nomic-embed-text-v1.5";

/// Deterministischer Vektor eines Textes: jedes Wort (klein, nur Buchstaben und Ziffern) erhöht ein Fach
/// (FNV-1a), danach auf Länge 1 normiert; ein Text ohne Wörter zeigt auf das erste Fach.
pub fn mock_vector(text: &str, dimension: usize) -> Vec<f32> {
    let dimension = dimension.max(1);
    let mut v = vec![0f32; dimension];
    for word in text.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()) {
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
        for byte in word.to_lowercase().bytes() {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x0100_0000_01b3);
        }
        v[(hash % dimension as u64) as usize] += 1.0;
    }
    let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm == 0.0 {
        v[0] = 1.0;
    } else {
        v.iter_mut().for_each(|x| *x /= norm);
    }
    v
}

struct Embedding {
    models: Vec<String>,
    dimension: usize,
    delay: Duration,
    failure: Option<(u16, String)>,
}

impl Default for Embedding {
    fn default() -> Self {
        Embedding {
            models: vec![MOCK_EMBEDDING_MODEL.to_owned()],
            dimension: MOCK_DIMENSION,
            delay: Duration::ZERO,
            failure: None,
        }
    }
}

#[derive(Default)]
struct Shared {
    embedding: Mutex<Embedding>,
    embed_active: AtomicUsize,
    embed_max_active: AtomicUsize,
    models: Mutex<Vec<String>>,
    bearer: Mutex<Option<String>>,
    replies: Mutex<VecDeque<Reply>>,
    requests: Mutex<Vec<Seen>>,
    active: AtomicUsize,
    max_active: AtomicUsize,
    cancelled: AtomicUsize,
    changed: Notify,
    flavor: Mutex<Flavor>,
    contexts: Mutex<Vec<(String, u32)>>,
    keep_alive: std::sync::atomic::AtomicBool,
    connections: AtomicUsize,
    models_failure: Mutex<Option<(u16, String)>>,
}

/// Der Nachbau; beim Fallenlassen endet er.
pub struct MockEliezer {
    address: std::net::SocketAddr,
    shared: Arc<Shared>,
    task: JoinHandle<()>,
}

impl Drop for MockEliezer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

impl MockEliezer {
    /// Startet auf `127.0.0.1:0` mit den Modellen `mock-modell` und `google/gemma-4-26b-a4b`, ohne
    /// Bearer-Pflicht.
    pub async fn start() -> MockEliezer {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("Nachbau: Port");
        let address = listener.local_addr().expect("Nachbau: Adresse");
        let shared = Arc::new(Shared::default());
        *lock(&shared.models) = vec!["mock-modell".into(), "google/gemma-4-26b-a4b".into()];
        let accept = shared.clone();
        let task = tokio::spawn(async move {
            while let Ok((stream, _)) = listener.accept().await {
                accept.connections.fetch_add(1, Ordering::SeqCst);
                let shared = accept.clone();
                tokio::spawn(async move {
                    let _ = handle(stream, shared).await;
                });
            }
        });
        MockEliezer { address, shared, task }
    }

    /// `http://127.0.0.1:PORT`.
    pub fn url(&self) -> String {
        format!("http://{}", self.address)
    }

    pub fn set_models(&self, models: &[&str]) {
        *lock(&self.shared.models) = models.iter().map(|m| (*m).to_owned()).collect();
    }

    /// Art der Gegenstelle (Vorgabe Eliezer).
    pub fn set_flavor(&self, flavor: Flavor) {
        *lock(&self.shared.flavor) = flavor;
    }

    /// Gemeldete Kontextlänge eines Modells (Eliezer: `max_context_length`, Ollama: `num_ctx`).
    pub fn set_context_length(&self, model: &str, tokens: u32) {
        let mut contexts = lock(&self.shared.contexts);
        contexts.retain(|(m, _)| m != model);
        contexts.push((model.to_owned(), tokens));
    }

    /// Verbindungen nach JSON-Antworten offen halten (keep-alive) statt sie zu schließen.
    pub fn set_keep_alive(&self, on: bool) {
        self.shared.keep_alive.store(on, Ordering::SeqCst);
    }

    /// Zahl der angenommenen TCP-Verbindungen seit dem Start.
    pub fn connections(&self) -> usize {
        self.shared.connections.load(Ordering::SeqCst)
    }

    /// `GET /v1/models` mit diesem Status und Rumpf beantworten (`None` = normal).
    pub fn fail_models(&self, failure: Option<(u16, &str)>) {
        *lock(&self.shared.models_failure) = failure.map(|(code, body)| (code, body.to_owned()));
    }

    /// Pfade aller Anfragen in ihrer Reihenfolge.
    pub fn paths(&self) -> Vec<String> {
        self.requests().into_iter().map(|s| s.path).collect()
    }

    /// Nur mit `Authorization: Bearer <secret>` antworten (`None` = jeder).
    pub fn require_bearer(&self, secret: Option<&str>) {
        *lock(&self.shared.bearer) = secret.map(str::to_owned);
    }

    /// Antwort für den nächsten Chat-Aufruf anhängen.
    pub fn push(&self, reply: Reply) {
        lock(&self.shared.replies).push_back(reply);
    }

    pub fn requests(&self) -> Vec<Seen> {
        lock(&self.shared.requests).clone()
    }

    /// Rümpfe aller Chat-Aufrufe.
    pub fn chat_bodies(&self) -> Vec<Value> {
        self.requests().into_iter().filter(|s| s.path.ends_with("/chat/completions")).filter_map(|s| s.body).collect()
    }

    /// Aufgerufene Pfade, die Baruch nie aufrufen darf (`/api/v1/models/load|unload|estimate`).
    pub fn forbidden_paths(&self) -> Vec<String> {
        self.requests()
            .into_iter()
            .map(|s| s.path)
            .filter(|p| ["/models/load", "/models/unload", "/models/estimate"].iter().any(|f| p.contains(f)))
            .collect()
    }

    /// Rümpfe, die eine Kontextlänge nennen (`context_length`, `num_ctx`, `n_ctx`, `contextLength` …).
    pub fn context_fields(&self) -> Vec<Value> {
        self.chat_bodies()
            .into_iter()
            .filter(|b| {
                let text = b.to_string().to_lowercase();
                text.contains("\"context") || text.contains("ctx\"") || text.contains("contextlength")
            })
            .collect()
    }

    /// Laufende Chat-Aufrufe.
    pub fn active(&self) -> usize {
        self.shared.active.load(Ordering::SeqCst)
    }

    /// Höchstzahl gleichzeitiger Chat-Aufrufe seit dem Start.
    pub fn max_active(&self) -> usize {
        self.shared.max_active.load(Ordering::SeqCst)
    }

    /// Vom Client abgebrochene Antworten.
    pub fn cancelled(&self) -> usize {
        self.shared.cancelled.load(Ordering::SeqCst)
    }

    /// Einbettungsmodelle, die der Nachbau kennt (andere → 404 „model not found“).
    pub fn set_embedding_models(&self, models: &[&str]) {
        lock(&self.shared.embedding).models = models.iter().map(|m| (*m).to_owned()).collect();
    }

    /// Dimension der Vektoren.
    pub fn set_embedding_dimension(&self, dimension: usize) {
        lock(&self.shared.embedding).dimension = dimension.max(1);
    }

    /// Pause vor jeder Einbettungsantwort (macht Überschneidungen sichtbar).
    pub fn set_embedding_delay(&self, delay: Duration) {
        lock(&self.shared.embedding).delay = delay;
    }

    /// Jede Einbettungsanfrage mit diesem Status und Rumpf beantworten (`None` = normal).
    pub fn fail_embeddings(&self, failure: Option<(u16, &str)>) {
        lock(&self.shared.embedding).failure = failure.map(|(code, body)| (code, body.to_owned()));
    }

    /// Rümpfe aller Einbettungsanfragen.
    pub fn embedding_bodies(&self) -> Vec<Value> {
        self.requests().into_iter().filter(|s| s.path.ends_with("/embeddings")).filter_map(|s| s.body).collect()
    }

    /// Alle eingebetteten Texte in der Reihenfolge der Anfragen.
    pub fn embedding_inputs(&self) -> Vec<String> {
        self.embedding_bodies()
            .iter()
            .flat_map(|b| b.get("input").and_then(Value::as_array).cloned().unwrap_or_default())
            .filter_map(|v| v.as_str().map(str::to_owned))
            .collect()
    }

    /// Höchstzahl gleichzeitiger Einbettungsanfragen seit dem Start (getrennt von [`MockEliezer::max_active`]).
    pub fn embed_max_active(&self) -> usize {
        self.shared.embed_max_active.load(Ordering::SeqCst)
    }

    /// Wartet höchstens `limit`, bis `test` gilt.
    pub async fn wait_for(&self, limit: Duration, test: impl Fn(&MockEliezer) -> bool) -> bool {
        let deadline = tokio::time::Instant::now() + limit;
        loop {
            let notified = self.shared.changed.notified();
            if test(self) {
                return true;
            }
            if tokio::time::timeout_at(deadline, notified).await.is_err() {
                return test(self);
            }
        }
    }
}

/// Kopf lesen (bis `\r\n\r\n`, höchstens 64 KiB), Rumpf nach `Content-Length`.
async fn read_request(
    stream: &mut TcpStream,
) -> std::io::Result<Option<(String, String, Vec<(String, String)>, Vec<u8>)>> {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    let head_end = loop {
        if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break i;
        }
        if buf.len() > 64 * 1024 {
            return Ok(None);
        }
        let n = stream.read(&mut chunk).await?;
        if n == 0 {
            return Ok(None);
        }
        buf.extend_from_slice(&chunk[..n]);
    };
    let head = String::from_utf8_lossy(&buf[..head_end]).to_string();
    let mut lines = head.split("\r\n");
    let mut first = lines.next().unwrap_or("").split(' ');
    let method = first.next().unwrap_or("").to_owned();
    let path = first.next().unwrap_or("").to_owned();
    let headers: Vec<(String, String)> = lines
        .filter_map(|l| l.split_once(':'))
        .map(|(k, v)| (k.trim().to_ascii_lowercase(), v.trim().to_owned()))
        .collect();
    let length = headers.iter().find(|(k, _)| k == "content-length").and_then(|(_, v)| v.parse().ok()).unwrap_or(0);
    let mut body = buf[head_end + 4..].to_vec();
    while body.len() < length {
        let n = stream.read(&mut chunk).await?;
        if n == 0 {
            break;
        }
        body.extend_from_slice(&chunk[..n]);
    }
    Ok(Some((method, path, headers, body)))
}

/// Antwort, die die Verbindung offen lässt (keep-alive).
async fn respond_kept(stream: &mut TcpStream, status: u16, content_type: &str, body: &str) -> std::io::Result<()> {
    let text = format!(
        "HTTP/1.1 {status} X\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: keep-alive\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(text.as_bytes()).await
}

async fn respond(stream: &mut TcpStream, status: u16, content_type: &str, body: &str) -> std::io::Result<()> {
    let text = format!(
        "HTTP/1.1 {status} X\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(text.as_bytes()).await?;
    stream.shutdown().await
}

/// Wartet `pause`; `true`, wenn der Client in der Zeit die Verbindung geschlossen hat.
async fn closed_during(stream: &mut TcpStream, pause: Option<Duration>) -> bool {
    let mut probe = [0u8; 64];
    let read = async {
        loop {
            match stream.read(&mut probe).await {
                Ok(0) | Err(_) => return,
                Ok(_) => continue,
            }
        }
    };
    match pause {
        Some(pause) => tokio::time::timeout(pause, read).await.is_ok(),
        None => {
            read.await;
            true
        }
    }
}

struct Active(Arc<Shared>, bool);

impl Active {
    fn start(shared: &Arc<Shared>) -> Active {
        let now = shared.active.fetch_add(1, Ordering::SeqCst) + 1;
        shared.max_active.fetch_max(now, Ordering::SeqCst);
        shared.changed.notify_waiters();
        Active(shared.clone(), true)
    }

    fn end(&mut self) {
        if std::mem::take(&mut self.1) {
            self.0.active.fetch_sub(1, Ordering::SeqCst);
            self.0.changed.notify_waiters();
        }
    }

    fn cancelled(&mut self) {
        self.0.cancelled.fetch_add(1, Ordering::SeqCst);
        self.end();
    }
}

impl Drop for Active {
    fn drop(&mut self) {
        self.end();
    }
}

async fn handle(mut stream: TcpStream, shared: Arc<Shared>) -> std::io::Result<()> {
    stream.set_nodelay(true)?;
    while handle_one(&mut stream, &shared).await? {}
    Ok(())
}

/// JSON-Antwort: bei keep-alive offen lassen (`Ok(true)`), sonst schließen.
async fn json_reply(stream: &mut TcpStream, shared: &Arc<Shared>, status: u16, body: &str) -> std::io::Result<bool> {
    if shared.keep_alive.load(Ordering::SeqCst) {
        respond_kept(stream, status, "application/json", body).await?;
        Ok(true)
    } else {
        respond(stream, status, "application/json", body).await?;
        Ok(false)
    }
}

/// Eine Anfrage; `Ok(true)`, wenn die Verbindung für die nächste offen bleibt.
async fn handle_one(stream: &mut TcpStream, shared: &Arc<Shared>) -> std::io::Result<bool> {
    let Some((method, path, headers, body)) = read_request(stream).await? else { return Ok(false) };
    let authorization = headers.iter().find(|(k, _)| k == "authorization").map(|(_, v)| v.clone());
    let body_json = serde_json::from_slice::<Value>(&body).ok();
    lock(&shared.requests).push(Seen {
        method: method.clone(),
        path: path.clone(),
        authorization: authorization.clone(),
        body: body_json,
    });
    shared.changed.notify_waiters();
    let required = lock(&shared.bearer).clone();
    if let Some(secret) = required
        && authorization.as_deref() != Some(&format!("Bearer {secret}"))
    {
        let shown = authorization.unwrap_or_default();
        let body = json!({ "error": { "message": format!("Invalid API key: {shown}"), "type": "auth" } });
        respond(stream, 401, "application/json", &body.to_string()).await?;
        return Ok(false);
    }
    let flavor = *lock(&shared.flavor);
    let context_of = |model: &str| lock(&shared.contexts).iter().find(|(m, _)| m == model).map(|(_, n)| *n);
    match (method.as_str(), path.as_str()) {
        ("GET", "/v1/models") => {
            let failure = lock(&shared.models_failure).clone();
            if let Some((code, text)) = failure {
                respond(stream, code, "application/json", &text).await?;
                return Ok(false);
            }
            let models = lock(&shared.models).clone();
            let data: Vec<Value> = models.iter().map(|m| json!({ "id": m, "object": "model" })).collect();
            json_reply(stream, shared, 200, &json!({ "object": "list", "data": data }).to_string()).await
        }
        ("GET", "/api/v1/models") if flavor == Flavor::Eliezer => {
            let models = lock(&shared.models).clone();
            let list: Vec<Value> = models
                .iter()
                .map(|m| {
                    let mut entry = json!({ "type": "llm", "key": m, "display_name": m, "loaded_instances": [] });
                    if let Some(n) = context_of(m) {
                        entry["max_context_length"] = json!(n);
                    }
                    entry
                })
                .collect();
            json_reply(stream, shared, 200, &json!({ "models": list }).to_string()).await
        }
        ("POST", "/api/show") if flavor == Flavor::Ollama => {
            let model = serde_json::from_slice::<Value>(&body)
                .ok()
                .and_then(|b| b.get("model").and_then(Value::as_str).map(str::to_owned))
                .unwrap_or_default();
            if !lock(&shared.models).contains(&model) {
                let text = json!({ "error": format!("model '{model}' not found") }).to_string();
                respond(stream, 404, "application/json", &text).await?;
                return Ok(false);
            }
            let mut answer = json!({ "modelfile": "",
                "model_info": { "general.architecture": "llama", "llama.context_length": 131072 } });
            if let Some(n) = context_of(&model) {
                answer["parameters"] = json!(format!("num_ctx {n}"));
            }
            json_reply(stream, shared, 200, &answer.to_string()).await
        }
        ("GET", "/api/ps") if flavor == Flavor::Ollama => {
            // Kein Modell geladen: Baruch muss dann die Vorgabe von Ollama annehmen (nie den Höchstwert).
            json_reply(stream, shared, 200, &json!({ "models": [] }).to_string()).await
        }
        ("POST", "/v1/embeddings") => {
            embed(stream, shared, serde_json::from_slice::<Value>(&body).ok()).await?;
            Ok(false)
        }
        ("POST", "/v1/chat/completions") => {
            chat(stream, shared).await?;
            Ok(false)
        }
        _ => {
            respond(stream, 404, "application/json", NOT_FOUND).await?;
            Ok(false)
        }
    }
}

const NOT_FOUND: &str = "{\"error\":{\"message\":\"not found\"}}";

/// Antwort auf einen Chat-Aufruf (schließt die Verbindung).
async fn chat(stream: &mut TcpStream, shared: &Arc<Shared>) -> std::io::Result<()> {
    let reply = lock(&shared.replies).pop_front().unwrap_or_else(|| Reply::text(&["OK"]));
    let mut active = Active::start(shared);
    match reply {
        Reply::Status(code, body) => {
            active.end();
            let kind = if body.trim_start().starts_with('{') { "application/json" } else { "text/plain" };
            respond(stream, code, kind, &body).await
        }
        Reply::Redirect(location) => {
            active.end();
            let text =
                format!("HTTP/1.1 302 Found\r\nLocation: {location}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
            stream.write_all(text.as_bytes()).await?;
            stream.shutdown().await
        }
        Reply::Json(value) => {
            active.end();
            respond(stream, 200, "application/json", &value.to_string()).await
        }
        Reply::Hang => {
            if closed_during(stream, None).await {
                active.cancelled();
            }
            Ok(())
        }
        Reply::Stream(steps) => {
            let head = "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-cache\r\nConnection: close\r\n\r\n";
            if stream.write_all(head.as_bytes()).await.is_err() {
                active.cancelled();
                return Ok(());
            }
            let last_send = steps.iter().rposition(|s| matches!(s, Step::Send(_)));
            for (i, step) in steps.into_iter().enumerate() {
                match step {
                    Step::Send(bytes) => {
                        if Some(i) == last_send {
                            active.end();
                        }
                        if stream.write_all(&bytes).await.is_err() {
                            active.cancelled();
                            return Ok(());
                        }
                    }
                    Step::Pause(pause) => {
                        if closed_during(stream, Some(pause)).await {
                            active.cancelled();
                            return Ok(());
                        }
                    }
                }
            }
            active.end();
            let _ = stream.shutdown().await;
            Ok(())
        }
    }
}

/// Zählt eine laufende Einbettungsanfrage.
struct EmbedActive(Arc<Shared>);

impl EmbedActive {
    fn start(shared: &Arc<Shared>) -> EmbedActive {
        let now = shared.embed_active.fetch_add(1, Ordering::SeqCst) + 1;
        shared.embed_max_active.fetch_max(now, Ordering::SeqCst);
        EmbedActive(shared.clone())
    }
}

impl Drop for EmbedActive {
    fn drop(&mut self) {
        self.0.embed_active.fetch_sub(1, Ordering::SeqCst);
        self.0.changed.notify_waiters();
    }
}

async fn embed(stream: &mut TcpStream, shared: &Arc<Shared>, body: Option<Value>) -> std::io::Result<()> {
    let active = EmbedActive::start(shared);
    let (models, dimension, delay, failure) = {
        let e = lock(&shared.embedding);
        (e.models.clone(), e.dimension, e.delay, e.failure.clone())
    };
    if !delay.is_zero() {
        tokio::time::sleep(delay).await;
    }
    if let Some((code, text)) = failure {
        drop(active);
        let kind = if text.trim_start().starts_with('{') { "application/json" } else { "text/plain" };
        return respond(stream, code, kind, &text).await;
    }
    let body = body.unwrap_or(Value::Null);
    let model = body.get("model").and_then(Value::as_str).unwrap_or("").to_owned();
    if !models.contains(&model) {
        drop(active);
        let text = json!({ "error": { "message": format!("model not found: {model}") } });
        return respond(stream, 404, "application/json", &text.to_string()).await;
    }
    let inputs: Vec<String> = match body.get("input") {
        Some(Value::Array(list)) => list.iter().map(|v| v.as_str().unwrap_or("").to_owned()).collect(),
        Some(Value::String(one)) => vec![one.clone()],
        _ => Vec::new(),
    };
    // Absichtlich rückwärts: Der Client muss nach `index` ordnen.
    let data: Vec<Value> = inputs
        .iter()
        .enumerate()
        .rev()
        .map(|(i, text)| json!({ "object": "embedding", "index": i, "embedding": mock_vector(text, dimension) }))
        .collect();
    let answer = json!({ "object": "list", "model": model, "data": data,
                         "usage": { "prompt_tokens": inputs.len(), "total_tokens": inputs.len() } });
    drop(active);
    respond(stream, 200, "application/json", &answer.to_string()).await
}
