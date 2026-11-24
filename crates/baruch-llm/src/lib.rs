// SPDX-License-Identifier: AGPL-3.0-or-later
//! Modell-Backend für Baruch (Runde 10, Stufe B2): ein OpenAI-kompatibler Client für Eliezer.
//!
//! - [`models`]: `GET <basis>/v1/models` → Namen aus `data[].id`.
//! - [`chat_stream`]: `POST <basis>/v1/chat/completions` mit `stream: true` und
//!   `stream_options.include_usage: true`; die Antwort kommt als Server-Sent Events ([`sse`]). Nur
//!   `choices[0].delta.content` zählt (jedes Stück geht sofort an `on_delta`), `reasoning_content`
//!   wird übergangen; `finish_reason` und `usage` kommen aus den letzten Stücken.
//! - [`Queue`]: höchstens **eine** laufende Anfrage (Eliezer lädt sonst ein Modell zweimal) und
//!   höchstens [`MAX_WAITING`] Wartende, sonst [`LlmError::Busy`].
//! - [`alias`]: alte Ollama-Namen der Personas → Namen bei Eliezer.
//! - [`embeddings`] (Runde 12, [`embed`]): `POST <basis>/v1/embeddings` → je Text ein Vektor;
//!   [`embedding_alias`] löst `nomic-embed-text` zu `text-embedding-nomic-embed-text-v1.5` auf.
//! - [`mock`]: kleiner HTTP-Nachbau von Eliezer auf `127.0.0.1:0` für Tests (auch mit Einbettungen; seit
//!   Runde 15b auch als Ollama- bzw. OpenAI-Gegenstelle mit gemeldeter Kontextlänge).
//! - [`Client`] (Runde 15b, [`client`]): ein Client je Endpunkt mit Art ([`BackendKind`]: Eliezer,
//!   OpenAI-kompatibel, Ollama) und einem gemeinsamen `reqwest::Client`; Modellliste mit Kontextlänge
//!   ([`ModelInfo`]) und gemeldete Kontextlänge je Modell. Die freien Funktionen bleiben als Hüllen.
//!
//! Grenzen ([`Limits`]): Verbindungsaufbau 10 s, erstes Stück 180 s (Eliezer lädt dabei ggf. das
//! Modell), Pause zwischen zwei Stücken 60 s, gesamt 600 s, Modellliste gesamt 10 s; Antworttext
//! höchstens 1 MiB; `max_tokens` wird auf `1..=4096` geklemmt. Weiterleitungen folgt der Client nicht
//! (der Schlüssel ginge sonst an eine andere Adresse).
//!
//! Baruch fordert **nie** eine Kontextlänge an und ruft nie `/api/v1/models/load|unload|estimate` auf:
//! Eine andere Kontextlänge ließe Eliezer das Modell neu laden (VRAM-Grenze des Nutzers). Abbruch =
//! Fallenlassen des Futures; die HTTP-Verbindung schließt dann sofort.
//!
//! Der Schlüssel steht nur in der Kopfzeile `Authorization`; jede Meldung läuft durch [`scrub`], bevor
//! sie einen Fehler bildet (seit Runde 23 auch gegen Base64- und Prozent-kodierte Formen; dieselbe
//! Funktion nutzen „Verbindung prüfen“, MultiGPT, pgvector und der Abgleich im Server). `Debug` von [`Endpoint`] zeigt ihn nie.

pub mod client;
pub mod embed;
pub mod mock;
mod queue;
mod secret;
pub mod sse;

use std::fmt;
use std::time::{Duration, Instant};

use futures_util::StreamExt;
use serde::Serialize;
use serde_json::{Value, json};

pub use client::{BackendKind, Client, ModelInfo};
pub use embed::{EmbeddingReply, embedding_alias, embedding_model_name, embeddings, embeddings_with};
pub use queue::{MAX_WAITING, Queue, QueueStatus};
pub use secret::{ENCODED_MIN as SCRUB_ENCODED_MIN, scrub};

/// Rolle einer Nachricht.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
}

/// Eine Nachricht im Gespräch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ChatMessage {
    pub role: Role,
    pub content: String,
}

/// Anfrage an das Modell. Bewusst ohne Kontextlänge (siehe Kopf).
#[derive(Debug, Clone, PartialEq)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    /// Wird auf `1..=`[`MAX_TOKENS`] geklemmt.
    pub max_tokens: u32,
    pub temperature: Option<f32>,
}

/// Fertige Antwort.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ChatReply {
    /// Zusammengesetzter Text aller `delta.content`-Stücke.
    pub text: String,
    /// Modell laut Antwort der Gegenstelle (sonst das angefragte).
    pub model: String,
    pub finish_reason: Option<String>,
    pub prompt_tokens: Option<u32>,
    pub completion_tokens: Option<u32>,
}

/// Warum ein Modellaufruf scheitert; dieselben acht Fälle wie `ModelFailure` im Server.
/// [`fmt::Display`] ist ein deutscher Satz und enthält nie ein Geheimnis.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LlmError {
    NotConfigured(String),
    Unreachable(String),
    Denied(String),
    Timeout(String),
    UnknownModel(String),
    Busy(String),
    Cancelled,
    Failed(String),
}

impl fmt::Display for LlmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LlmError::NotConfigured(m) => write!(f, "Modell-Backend nicht bereit: {m}"),
            LlmError::Unreachable(m) => write!(f, "Modell-Backend nicht erreichbar: {m}"),
            LlmError::Denied(m) => write!(f, "Zugang zum Modell-Backend verweigert: {m}"),
            LlmError::Timeout(m) => write!(f, "Zeitüberschreitung beim Modellaufruf: {m}"),
            LlmError::UnknownModel(m) => write!(f, "Unbekanntes Modell: {m}"),
            LlmError::Busy(m) => write!(f, "Modell-Backend beschäftigt: {m}"),
            LlmError::Cancelled => f.write_str("Der Modellaufruf wurde abgebrochen."),
            LlmError::Failed(m) => write!(f, "Modellaufruf gescheitert: {m}"),
        }
    }
}

impl std::error::Error for LlmError {}

/// Größtes `max_tokens`, das Baruch schickt.
pub const MAX_TOKENS: u32 = 4096;
/// Größter Antworttext.
pub const MAX_TEXT: usize = 1024 * 1024;
/// Länge, auf die eine Fehlermeldung der Gegenstelle gekürzt wird (Zeichen).
pub const MAX_SERVER_MESSAGE: usize = 300;

/// Zeitgrenzen und Größen eines Aufrufs; [`Limits::default`] sind die Werte des Betriebs, Tests
/// nehmen kürzere.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// Verbindungsaufbau.
    pub connect: Duration,
    /// Bis zum ersten Stück der Antwort (Kopfzeilen und erstes SSE-Ereignis; Laden des Modells).
    pub first_token: Duration,
    /// Größte Pause zwischen zwei Stücken.
    pub idle: Duration,
    /// Gesamtdauer eines Chat-Aufrufs.
    pub total: Duration,
    /// Gesamtdauer für die Modellliste.
    pub models_total: Duration,
    /// Größter Antworttext in Bytes.
    pub max_text: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Limits {
            connect: Duration::from_secs(10),
            first_token: Duration::from_secs(180),
            idle: Duration::from_secs(60),
            total: Duration::from_secs(600),
            models_total: Duration::from_secs(10),
            max_text: MAX_TEXT,
        }
    }
}

/// Adresse und Schlüssel des Backends. `Debug` zeigt den Schlüssel nie.
#[derive(Clone, PartialEq, Eq)]
pub struct Endpoint {
    /// Grundadresse, z. B. `http://127.0.0.1:1234` oder `…/v1`.
    pub base_url: String,
    /// Bearer-Schlüssel; nur in der Kopfzeile `Authorization`.
    pub secret: Option<String>,
}

impl fmt::Debug for Endpoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Endpoint")
            .field("base_url", &self.base_url)
            .field("secret", &self.secret.as_ref().map(|_| "***"))
            .finish()
    }
}

impl Endpoint {
    pub fn new(base_url: impl Into<String>, secret: Option<String>) -> Endpoint {
        Endpoint { base_url: base_url.into(), secret: secret.filter(|s| !s.is_empty()) }
    }

    fn api(&self, path: &str) -> String {
        let base = self.base_url.trim().trim_end_matches('/');
        if base.ends_with("/v1") { format!("{base}/{path}") } else { format!("{base}/v1/{path}") }
    }

    /// `<basis>/v1/models` (endet die Basis schon auf `/v1`, nur `/models`).
    pub fn models_url(&self) -> String {
        self.api("models")
    }

    /// `<basis>/v1/chat/completions`.
    pub fn chat_url(&self) -> String {
        self.api("chat/completions")
    }

    /// `<basis>/v1/embeddings` (Runde 12).
    pub fn embeddings_url(&self) -> String {
        self.api("embeddings")
    }

    /// Jedes Vorkommen des Schlüssels durch `***` ersetzen (auch Base64- und Prozent-kodiert, siehe [`scrub`]).
    pub fn scrub(&self, text: &str) -> String {
        scrub(text, self.secret.as_deref())
    }
}

/// Alter Ollama-Name → Name bei Eliezer (Backlog „Modelle der Personas über Eliezer“).
pub fn alias(name: &str) -> Option<&'static str> {
    match name.trim() {
        "llama3.2:3b" => Some("llama-3.2-3b-instruct-q4-k-m"),
        "qwen3:4b" => Some("qwen3-4b-q4-k-m"),
        _ => None,
    }
}

/// Alle Aliasse `(alt, neu)`.
pub const ALIASES: &[(&str, &str)] =
    &[("llama3.2:3b", "llama-3.2-3b-instruct-q4-k-m"), ("qwen3:4b", "qwen3-4b-q4-k-m")];

pub(crate) fn client(limits: &Limits) -> Result<reqwest::Client, LlmError> {
    reqwest::Client::builder()
        .connect_timeout(limits.connect)
        .redirect(reqwest::redirect::Policy::none())
        .user_agent(concat!("baruch/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| LlmError::Failed(format!("HTTP-Client: {e}")))
}

pub(crate) fn whole_secs(d: Duration) -> String {
    if d.as_millis().is_multiple_of(1000) { format!("{} s", d.as_secs()) } else { format!("{} ms", d.as_millis()) }
}

/// Fehler von reqwest als Satz mit Ursachenkette (ohne Kopfzeilen, also ohne Schlüssel).
pub(crate) fn transport_error(e: &reqwest::Error, url: &str, endpoint: &Endpoint, limits: &Limits) -> LlmError {
    if e.is_timeout() {
        return LlmError::Timeout(format!("Keine Verbindung zu {url} innerhalb von {}.", whole_secs(limits.connect)));
    }
    let mut causes = Vec::new();
    let mut source = std::error::Error::source(e);
    while let Some(cause) = source {
        causes.push(cause.to_string());
        source = cause.source();
    }
    let why = if causes.is_empty() { e.to_string() } else { causes.join(": ") };
    let why = endpoint.scrub(&why);
    if e.is_connect() {
        LlmError::Unreachable(format!("Keine Verbindung zu {url}: {why}"))
    } else {
        LlmError::Failed(format!("Anfrage an {url} gescheitert: {why}"))
    }
}

/// Meldung aus einer Fehlerantwort: `error.message`, `error` (Text), `message` oder der Text selbst;
/// Geheimnis entfernt, eine Zeile, höchstens [`MAX_SERVER_MESSAGE`] Zeichen.
pub fn server_message(body: &[u8], endpoint: &Endpoint) -> String {
    let text = String::from_utf8_lossy(body);
    let message = match serde_json::from_str::<Value>(&text) {
        Ok(v) => v
            .pointer("/error/message")
            .or_else(|| v.get("error").filter(|e| e.is_string()))
            .or_else(|| v.get("message"))
            .and_then(Value::as_str)
            .map(str::to_owned)
            .unwrap_or_else(|| text.to_string()),
        Err(_) => text.to_string(),
    };
    let clean: String = endpoint.scrub(&message).split_whitespace().collect::<Vec<_>>().join(" ");
    shorten(&clean, MAX_SERVER_MESSAGE)
}

fn shorten(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_owned();
    }
    let mut out: String = text.chars().take(max.saturating_sub(1)).collect();
    out.push('…');
    out
}

fn mentions_unknown_model(message: &str) -> bool {
    let lower = message.to_lowercase();
    lower.contains("model not found") || lower.contains("model_not_found") || lower.contains("no such model")
}

/// HTTP-Status ≠ 2xx → Fehlerart. `chat`: 404 heißt dort „Modell unbekannt“.
pub(crate) fn status_error(status: u16, body: &[u8], url: &str, endpoint: &Endpoint, chat: bool) -> LlmError {
    let message = server_message(body, endpoint);
    let shown = if message.is_empty() { String::new() } else { format!(": {message}") };
    match status {
        401 | 403 => {
            let hint = if endpoint.secret.is_some() {
                "Schlüssel baruch/eliezer im Schlüsselbund prüfen"
            } else {
                "kein Schlüssel hinterlegt (baruch/eliezer)"
            };
            LlmError::Denied(format!("HTTP {status} von {url}; {hint}."))
        }
        // Runde 23: Der Client folgt keiner Weiterleitung (`redirect::Policy::none()`); das Ziel (`Location`)
        // wird nicht genannt, es käme ungeprüft von der Gegenstelle.
        300..=399 => LlmError::Failed(format!(
            "HTTP {status} von {url}: Weiterleitung. Baruch folgt ihr nicht (der Schlüssel ginge sonst an eine \
             andere Adresse); bitte die endgültige Adresse eintragen."
        )),
        404 if chat => LlmError::UnknownModel(format!("HTTP 404 von {url}{shown}")),
        _ if mentions_unknown_model(&message) => LlmError::UnknownModel(format!("HTTP {status}{shown}")),
        409 | 429 | 503 => LlmError::Busy(format!("HTTP {status} von {url}{shown}")),
        _ => LlmError::Failed(format!("HTTP {status} von {url}{shown}")),
    }
}

/// Text einer Antwort, höchstens `max` Bytes (mehr wird abgeschnitten, nicht gelesen).
pub(crate) async fn read_limited(response: reqwest::Response, max: usize) -> Vec<u8> {
    let mut out = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(Ok(chunk)) = stream.next().await {
        let room = max.saturating_sub(out.len());
        out.extend_from_slice(&chunk[..chunk.len().min(room)]);
        if out.len() >= max {
            break;
        }
    }
    out
}

/// Modellliste mit den Grenzen des Betriebs.
pub async fn models(endpoint: &Endpoint) -> Result<Vec<String>, LlmError> {
    models_with(endpoint, &Limits::default()).await
}

/// Modellliste: `GET <basis>/v1/models` → `data[].id`, gesamt höchstens `limits.models_total` (eigener
/// HTTP-Client für diesen Aufruf; [`Client::models`] teilt einen).
pub async fn models_with(endpoint: &Endpoint, limits: &Limits) -> Result<Vec<String>, LlmError> {
    let http = client(limits)?;
    models_using(&http, endpoint, limits).await
}

/// Wie [`models_with`] über einen vorhandenen HTTP-Client.
pub(crate) async fn models_using(
    client: &reqwest::Client,
    endpoint: &Endpoint,
    limits: &Limits,
) -> Result<Vec<String>, LlmError> {
    let url = endpoint.models_url();
    let attempt = async {
        let mut request = client.get(&url).header(reqwest::header::ACCEPT, "application/json");
        if let Some(secret) = &endpoint.secret {
            request = request.bearer_auth(secret);
        }
        let response = request.send().await.map_err(|e| transport_error(&e, &url, endpoint, limits))?;
        let status = response.status().as_u16();
        if !(200..300).contains(&status) {
            let body = read_limited(response, 64 * 1024).await;
            return Err(status_error(status, &body, &url, endpoint, false));
        }
        let body = read_limited(response, limits.max_text + 1).await;
        if body.len() > limits.max_text {
            return Err(LlmError::Failed(format!("Antwort von {url} ist größer als 1 MiB; abgebrochen.")));
        }
        let parsed: Value =
            serde_json::from_slice(&body).map_err(|_| LlmError::Failed(format!("{url} liefert kein JSON.")))?;
        let data = parsed
            .get("data")
            .and_then(Value::as_array)
            .ok_or_else(|| LlmError::Failed(format!("{url} liefert keine Modellliste (Feld data fehlt).")))?;
        Ok(data.iter().filter_map(|m| m.get("id").and_then(Value::as_str)).map(|id| endpoint.scrub(id)).collect())
    };
    tokio::time::timeout(limits.models_total, attempt).await.unwrap_or_else(|_| {
        Err(LlmError::Timeout(format!(
            "Keine Modellliste von {url} innerhalb von {}.",
            whole_secs(limits.models_total)
        )))
    })
}

/// Rumpf von `POST /v1/chat/completions` (öffentlich für Tests): Modell, Nachrichten, geklemmtes
/// `max_tokens`, wahlweise `temperature`, `stream: true`, `stream_options.include_usage: true`.
/// Bewusst ohne Kontextlänge.
pub fn chat_body(request: &ChatRequest) -> Value {
    let mut body = json!({
        "model": request.model,
        "messages": request.messages,
        "max_tokens": request.max_tokens.clamp(1, MAX_TOKENS),
        "stream": true,
        "stream_options": { "include_usage": true },
    });
    if let Some(t) = request.temperature.filter(|t| t.is_finite()) {
        body["temperature"] = json!(t);
    }
    body
}

/// Gestreamter Chat mit den Grenzen des Betriebs.
pub async fn chat_stream(
    endpoint: &Endpoint,
    request: &ChatRequest,
    on_delta: impl FnMut(&str) + Send,
) -> Result<ChatReply, LlmError> {
    chat_stream_with(endpoint, request, &Limits::default(), on_delta).await
}

/// Zwischenstand beim Lesen der Ereignisse.
struct Collect<'a> {
    endpoint: &'a Endpoint,
    limits: &'a Limits,
    text: String,
    model: Option<String>,
    finish_reason: Option<String>,
    prompt_tokens: Option<u32>,
    completion_tokens: Option<u32>,
}

impl Collect<'_> {
    /// Ein `data:`-Ereignis (JSON) verarbeiten; liefert das neue Textstück.
    fn event(&mut self, data: &str) -> Result<Option<String>, LlmError> {
        let Ok(value) = serde_json::from_str::<Value>(data) else {
            return Err(LlmError::Failed("Ereignis im Antwortstrom ist kein JSON.".into()));
        };
        if let Some(error) = value.get("error") {
            let message = server_message(error.to_string().as_bytes(), self.endpoint);
            let message = match error.get("message").and_then(Value::as_str) {
                Some(m) => shorten(&self.endpoint.scrub(m), MAX_SERVER_MESSAGE),
                None => message,
            };
            return Err(if mentions_unknown_model(&message) {
                LlmError::UnknownModel(message)
            } else {
                LlmError::Failed(format!("Fehler im Antwortstrom: {message}"))
            });
        }
        if self.model.is_none() {
            self.model = value.get("model").and_then(Value::as_str).filter(|m| !m.is_empty()).map(str::to_owned);
        }
        if let Some(usage) = value.get("usage").filter(|u| u.is_object()) {
            let number = |name: &str| usage.get(name).and_then(Value::as_u64).and_then(|n| u32::try_from(n).ok());
            self.prompt_tokens = number("prompt_tokens").or(self.prompt_tokens);
            self.completion_tokens = number("completion_tokens").or(self.completion_tokens);
        }
        let Some(choice) = value.get("choices").and_then(Value::as_array).and_then(|c| c.first()) else {
            return Ok(None);
        };
        if let Some(reason) = choice.get("finish_reason").and_then(Value::as_str) {
            self.finish_reason = Some(reason.to_owned());
        }
        // Nur der eigentliche Inhalt; `delta.reasoning_content` (Denkspur) wird übergangen. Ohne
        // Strom (Gegenstelle ignoriert `stream`) steht der Text in `message.content`.
        let piece = choice
            .pointer("/delta/content")
            .or_else(|| choice.pointer("/message/content"))
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty());
        let Some(piece) = piece else { return Ok(None) };
        if self.text.len() + piece.len() > self.limits.max_text {
            return Err(LlmError::Failed("Die Antwort ist größer als 1 MiB; abgebrochen.".into()));
        }
        self.text.push_str(piece);
        Ok(Some(piece.to_owned()))
    }
}

/// Gestreamter Chat: jedes Textstück geht sofort an `on_delta`; Ergebnis ist der ganze Text.
///
/// Zeitgrenzen: bis zum ersten SSE-Ereignis `limits.first_token` (ab dem Absenden), danach je Stück
/// höchstens `limits.idle`, insgesamt `limits.total`. `[DONE]` beendet das Lesen, auch wenn die
/// Gegenstelle die Verbindung offen hält.
pub async fn chat_stream_with(
    endpoint: &Endpoint,
    request: &ChatRequest,
    limits: &Limits,
    on_delta: impl FnMut(&str) + Send,
) -> Result<ChatReply, LlmError> {
    let http = client(limits)?;
    chat_stream_using(&http, endpoint, request, limits, on_delta).await
}

/// Wie [`chat_stream_with`] über einen vorhandenen HTTP-Client ([`Client::chat_stream`]).
pub(crate) async fn chat_stream_using(
    client: &reqwest::Client,
    endpoint: &Endpoint,
    request: &ChatRequest,
    limits: &Limits,
    mut on_delta: impl FnMut(&str) + Send,
) -> Result<ChatReply, LlmError> {
    let url = endpoint.chat_url();
    let body = chat_body(request);
    let started = Instant::now();
    let first_deadline = started + limits.first_token;
    let first_timeout = || {
        LlmError::Timeout(format!(
            "Keine Antwort von {url} innerhalb von {} (lädt das Modell noch?).",
            whole_secs(limits.first_token)
        ))
    };
    let attempt = async {
        let mut call = client
            .post(&url)
            .header(reqwest::header::ACCEPT, "text/event-stream")
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body(body.to_string());
        if let Some(secret) = &endpoint.secret {
            call = call.bearer_auth(secret);
        }
        let response = match tokio::time::timeout(limits.first_token, call.send()).await {
            Err(_) => return Err(first_timeout()),
            Ok(Err(e)) => return Err(transport_error(&e, &url, endpoint, limits)),
            Ok(Ok(response)) => response,
        };
        let status = response.status().as_u16();
        if !(200..300).contains(&status) {
            let body = tokio::time::timeout(limits.idle, read_limited(response, 64 * 1024)).await.unwrap_or_default();
            return Err(status_error(status, &body, &url, endpoint, true));
        }
        let event_stream = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .is_none_or(|v| v.to_ascii_lowercase().starts_with("text/event-stream"));
        let mut collect = Collect {
            endpoint,
            limits,
            text: String::new(),
            model: None,
            finish_reason: None,
            prompt_tokens: None,
            completion_tokens: None,
        };
        let mut parser = sse::Parser::new(limits.max_text + 64 * 1024);
        let mut whole = Vec::new();
        let mut stream = response.bytes_stream();
        let mut alive = false;
        'read: loop {
            let wait = if alive { limits.idle } else { first_deadline.saturating_duration_since(Instant::now()) };
            let next = match tokio::time::timeout(wait, stream.next()).await {
                Ok(next) => next,
                Err(_) if alive => {
                    return Err(LlmError::Timeout(format!(
                        "Die Antwort von {url} stockt länger als {}.",
                        whole_secs(limits.idle)
                    )));
                }
                Err(_) => return Err(first_timeout()),
            };
            let chunk = match next {
                None => break,
                Some(Err(e)) => return Err(transport_error(&e, &url, endpoint, limits)),
                Some(Ok(chunk)) => chunk,
            };
            if !event_stream {
                // Gegenstelle ohne Strom: ganze JSON-Antwort sammeln.
                if whole.len() + chunk.len() > limits.max_text {
                    return Err(LlmError::Failed("Die Antwort ist größer als 1 MiB; abgebrochen.".into()));
                }
                whole.extend_from_slice(&chunk);
                continue;
            }
            for event in parser.push(&chunk)? {
                alive = true;
                match event {
                    sse::Event::Done => break 'read,
                    sse::Event::Data(data) => {
                        if let Some(piece) = collect.event(&data)? {
                            on_delta(&piece);
                        }
                    }
                }
            }
        }
        if event_stream {
            for event in parser.finish()? {
                if let sse::Event::Data(data) = event
                    && let Some(piece) = collect.event(&data)?
                {
                    on_delta(&piece);
                }
            }
        } else {
            let text = String::from_utf8_lossy(&whole).to_string();
            if let Some(piece) = collect.event(&text)? {
                on_delta(&piece);
            }
        }
        Ok(ChatReply {
            text: collect.text,
            model: collect.model.unwrap_or_else(|| request.model.clone()),
            finish_reason: collect.finish_reason,
            prompt_tokens: collect.prompt_tokens,
            completion_tokens: collect.completion_tokens,
        })
    };
    tokio::time::timeout(limits.total, attempt).await.unwrap_or_else(|_| {
        Err(LlmError::Timeout(format!("Der Modellaufruf dauerte länger als {}.", whole_secs(limits.total))))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn addresses_are_built_from_the_base_with_or_without_v1() {
        let e = Endpoint::new("http://h:1/", None);
        assert_eq!(e.models_url(), "http://h:1/v1/models", "Grundadresse mit Schrägstrich");
        assert_eq!(e.chat_url(), "http://h:1/v1/chat/completions", "Chat-Adresse");
        let e = Endpoint::new("http://h:1/v1", None);
        assert_eq!(e.models_url(), "http://h:1/v1/models", "Basis endet schon auf /v1");
        assert_eq!(e.chat_url(), "http://h:1/v1/chat/completions", "kein doppeltes /v1");
        assert_eq!(e.embeddings_url(), "http://h:1/v1/embeddings", "Einbettungen ebenso");
    }

    #[test]
    fn debug_never_shows_the_secret() {
        let e = Endpoint::new("http://h", Some("Qq-geheim-123".into()));
        let shown = format!("{e:?}");
        assert!(!shown.contains("Qq-geheim-123") && shown.contains("***"), "Debug ohne Schlüssel: {shown}");
    }

    #[test]
    fn the_body_clamps_max_tokens_and_never_names_a_context_length() {
        let mut request = ChatRequest { model: "m".into(), messages: vec![], max_tokens: 0, temperature: Some(0.4) };
        assert_eq!(chat_body(&request)["max_tokens"], 1, "0 → 1");
        request.max_tokens = 1_000_000;
        let body = chat_body(&request);
        assert_eq!(body["max_tokens"], MAX_TOKENS, "nach oben geklemmt");
        assert_eq!(body["stream"], true, "immer gestreamt");
        assert_eq!(body["stream_options"]["include_usage"], true, "usage im letzten Stück");
        let text = body.to_string().to_lowercase();
        assert!(!text.contains("ctx") && !text.contains("context"), "keine Kontextlänge: {text}");
    }

    #[test]
    fn server_messages_are_one_short_line_without_the_secret() {
        let e = Endpoint::new("http://h", Some("Ss-77".into()));
        let long = format!(r#"{{"error":{{"message":"schlecht Ss-77\n{}"}}}}"#, "x".repeat(1000));
        let m = server_message(long.as_bytes(), &e);
        assert!(!m.contains("Ss-77") && !m.contains('\n'), "ohne Schlüssel, eine Zeile: {m}");
        assert_eq!(m.chars().count(), MAX_SERVER_MESSAGE, "auf 300 Zeichen gekürzt");
        assert_eq!(server_message(b"plain text", &e), "plain text", "Text ohne JSON bleibt");
    }

    #[test]
    fn old_ollama_names_have_aliases() {
        assert_eq!(alias("llama3.2:3b"), Some("llama-3.2-3b-instruct-q4-k-m"), "Llama");
        assert_eq!(alias("qwen3:4b"), Some("qwen3-4b-q4-k-m"), "Qwen");
        assert_eq!(alias("google/gemma-4-26b-a4b"), None, "Eliezer-Namen bleiben");
        for (old, new) in ALIASES {
            assert_eq!(alias(old), Some(*new), "Liste und Funktion stimmen überein");
        }
    }
}
