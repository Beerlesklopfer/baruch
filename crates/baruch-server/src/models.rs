// SPDX-License-Identifier: AGPL-3.0-or-later
//! Model backends in the server (Round 10, task „backend“; Round 15b „anbieter“): connections of the kind
//! Eliezer, OpenAI-compatible or Ollama ([`baruch_llm::Client`], one HTTP client per endpoint), per persona
//! one connection with „lokal“/„extern“ and a release, queue **per endpoint** ([`status`], exactly one
//! running request, at most eight waiting), model list with a cache, short test, reconciliation of the
//! context length and the RPC methods `model.*` ([`handles`], [`call`]; only for the human – not through
//! `POST /act`, not through MCP).
//!
//! **Connections** (store `model_connection`, migration 0036): the connection „Eliezer“
//! ([`baruch_store::ELIEZER_CONNECTION`]) keeps taking address and binding of the key from the connection
//! `eliezer` (Round 9, Einstellungen → Verbindungen), the key from the account `baruch/eliezer`. Further
//! connections have their own address and the account `baruch/model-<id>`; their key is bound to the target
//! (origin of the address) for which it was stored – if the address points elsewhere, it does not go
//! out ([`LlmError::NotConfigured`]).
//!
//! **External:** a connection „extern“ sends book text out of the house. A job of a persona goes through
//! it only if the persona is released for that (`persona_backend.external_allowed`, default off); otherwise
//! [`route`] refuses with a German sentence **before** a request goes out. Short test and
//! model list send no book text and also work without a release.
//!
//! **Model list:** cached for 60 s per (kind, address) ([`CACHE_FOR`]; `live: false` from the cache;
//! since Round 23 per kind – two connections of different kinds on the same address share neither list nor
//! context length).
//! If the remote side does not answer, [`list_on`] delivers the expired list with `live: false` and a
//! notice (instead of an error) as long as there is one.
//!
//! **Context** ([`context`]): effective length = min(setting of the persona, length reported by the remote
//! side); warning on a deviation. Reporting is read-only (Eliezer `GET /api/v1/models`, Ollama
//! `POST /api/show`, OpenAI nothing); Baruch never requests a context length and never calls
//! `/api/v1/models/load|unload|estimate`. The short test is stored in the database. Since Round 23
//! [`context`] checks the release first: for an external connection without a release of the persona the
//! reconciliation does not go out either (it would carry key and model name), the setting applies.
//!
//! **Deleting** (Round 23): `model.connection_delete` and `model.connection_delete_secret` require
//! `confirm: true` (otherwise -32602); the forms check the mandatory checkbox and then call the same functions.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use baruch_llm::{
    BackendKind, ChatMessage, ChatReply, ChatRequest, Client, Endpoint, Limits, LlmError, ModelInfo, Queue,
    QueueStatus, Role,
};
use baruch_store::{
    ConnectionKind, ELIEZER_CONNECTION, ModelConnection, ModelConnectionEdit, PersonaBackend, PersonaRow,
};
use serde_json::{Map, Value, json};

use crate::AppState;
use crate::assist::port::ContextBudget;
use crate::rpc::{INTERNAL, INVALID_PARAMS, METHOD_NOT_FOUND, REFUSED, RpcError};

/// How long a model list (and a reported context length) is valid.
pub const CACHE_FOR: Duration = Duration::from_secs(60);
/// Fixed prompt of the short test.
pub const PROBE_PROMPT: &str = "Antworte nur mit: OK";
/// `max_tokens` of the short test.
pub const PROBE_MAX_TOKENS: u32 = 16;

const METHODS: &[&str] = &[
    "model.list",
    "model.status",
    "model.probe",
    "model.context",
    "model.connections",
    "model.connection_set",
    "model.connection_delete",
    "model.connection_set_secret",
    "model.connection_delete_secret",
    "model.connection_check",
];

/// Methods that change something (human only; not through `/act`, not through MCP).
pub const WRITES: &[&str] = &[
    "model.connection_set",
    "model.connection_delete",
    "model.connection_set_secret",
    "model.connection_delete_secret",
];

/// Queues per endpoint (address), for the whole process.
static QUEUES: LazyLock<Mutex<HashMap<String, &'static Queue>>> = LazyLock::new(Default::default);
/// HTTP clients per (kind, address, fingerprint of the key).
/// Key of the client pool: kind, address, fingerprint of the key.
type ClientKey = (BackendKind, String, u64);
static CLIENTS: LazyLock<Mutex<HashMap<ClientKey, Client>>> = LazyLock::new(Default::default);

struct Cached {
    at: Instant,
    fetched: String,
    models: Vec<ModelInfo>,
}

/// Model lists per (kind, address) – since Round 23 per kind: an Eliezer and an Ollama connection on
/// the same address report different lists and context lengths.
static CACHE: LazyLock<Mutex<HashMap<(BackendKind, String), Cached>>> = LazyLock::new(Default::default);
/// Reported context lengths per (kind, address, model).
/// (kind, address, model) → (point in time, reported length).
type ContextCache = HashMap<(BackendKind, String, String), (Instant, Option<u32>)>;
static CONTEXTS: LazyLock<Mutex<ContextCache>> = LazyLock::new(Default::default);

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// Model list of a connection.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Listing {
    pub models: Vec<String>,
    /// `true`: just queried; `false`: from the cache.
    pub live: bool,
    /// Point in time of the query (ISO 8601, UTC).
    pub fetched: String,
    /// Notice when the list comes from the expired cache (the remote side does not answer).
    pub message: Option<String>,
    /// Per model the reported context length (as far as the remote side reports it in the list).
    pub infos: Vec<ModelInfo>,
}

/// Now as `2026-11-06T09:30:12.345Z`.
pub fn iso_now() -> String {
    let since = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
    let secs = since.as_secs() as i64;
    let (days, rest) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    // Days since 1970-01-01 → date (H. Hinnant, „civil_from_days“).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{:03}Z",
        rest / 3600,
        rest % 3600 / 60,
        rest % 60,
        since.subsec_millis()
    )
}

/// Origin of an address (scheme, host, port): the target to which a key is bound.
pub fn url_target(url: &str) -> String {
    reqwest::Url::parse(url.trim()).map_or_else(|_| url.trim().to_owned(), |u| u.origin().ascii_serialization())
}

/// A connection, ready for requests: kind, address, key (only if it applies to the present target).
#[derive(Clone)]
pub struct Target {
    pub connection: ModelConnection,
    pub kind: BackendKind,
    pub endpoint: Endpoint,
}

/// Does the address recognisably lie on this machine or in the own network? Yes for loopback (`127.0.0.0/8`, `::1`,
/// `localhost`, `*.localhost`), private and link-local addresses (`10/8`, `172.16/12`, `192.168/16`, `169.254/16`,
/// `100.64/10`, `fc00::/7`, `fe80::/10`), host names without a dot and the names under `.local`, `.lan`,
/// `.internal`, `.home.arpa`. Everything else (public addresses, ordinary domain names) counts as **not**
/// recognisably local – Baruch asks no DNS. From this follows the default „extern“ when creating and the warning
/// [`locality_warning`].
pub fn looks_local(url: &str) -> bool {
    let Ok(parsed) = reqwest::Url::parse(url.trim()) else { return false };
    let Some(host) = parsed.host_str() else { return false };
    match host.trim_start_matches('[').trim_end_matches(']').parse::<std::net::IpAddr>() {
        Ok(std::net::IpAddr::V4(ip)) => {
            let [a, b, ..] = ip.octets();
            ip.is_loopback() || ip.is_private() || ip.is_link_local() || (a == 100 && (64..128).contains(&b))
        }
        Ok(std::net::IpAddr::V6(ip)) => {
            let first = ip.segments()[0];
            ip.is_loopback() || (first & 0xfe00) == 0xfc00 || (first & 0xffc0) == 0xfe80
        }
        Err(_) => {
            let name = host.trim_end_matches('.').to_ascii_lowercase();
            name == "localhost"
                || !name.contains('.')
                || [".localhost", ".local", ".lan", ".internal", ".home.arpa"].iter().any(|s| name.ends_with(s))
        }
    }
}

/// Warning when a connection kept as local points to an address that is not recognisably local (book text would
/// go out without a release); otherwise `None`.
pub fn locality_warning(external: bool, url: &str) -> Option<String> {
    (!external && !url.trim().is_empty() && !looks_local(url)).then(|| {
        "Die Adresse liegt nicht erkennbar auf diesem Rechner oder im eigenen Netz, die Verbindung ist aber als lokal \
         geführt: Buchtext geht ohne Freigabe je Persona dorthin. Liegt der Dienst außerhalb, „extern“ ankreuzen."
            .to_owned()
    })
}

/// Display „lokal“ or „extern“.
pub fn locality(connection: &ModelConnection) -> &'static str {
    if connection.external { "extern" } else { "lokal" }
}

/// Display name of a kind.
pub fn kind_title(kind: &str) -> &'static str {
    match kind {
        "eliezer" => "Eliezer",
        "openai" => "OpenAI-kompatibel",
        "ollama" => "Ollama",
        _ => "unbekannt",
    }
}

/// Address of the connection Eliezer (from the connection `eliezer`).
async fn eliezer_url(state: &AppState) -> Result<String, LlmError> {
    let row = state
        .store
        .connection_get(ConnectionKind::Eliezer)
        .await
        .map_err(|e| LlmError::NotConfigured(format!("Verbindung eliezer nicht lesbar: {e}")))?;
    Ok(row.config.get("url").and_then(Value::as_str).unwrap_or("").trim().to_owned())
}

/// Effective address of a connection (Eliezer from the connection `eliezer`).
pub async fn effective_url(state: &AppState, connection: &ModelConnection) -> String {
    match &connection.url {
        Some(url) => url.clone(),
        None => eliezer_url(state).await.unwrap_or_default(),
    }
}

/// Resolve the target of a connection: address, key from the keyring (only if it is bound to the present
/// target). Without an address or with a key for another target → `NotConfigured` (nothing sent).
pub async fn resolve(state: &AppState, connection: &ModelConnection) -> Result<Target, LlmError> {
    let kind = BackendKind::parse(&connection.kind)
        .ok_or_else(|| LlmError::NotConfigured(format!("Unbekannte Art „{}“.", connection.kind)))?;
    let (url, bound) = if connection.id == ELIEZER_CONNECTION {
        let row = state
            .store
            .connection_get(ConnectionKind::Eliezer)
            .await
            .map_err(|e| LlmError::NotConfigured(format!("Verbindung eliezer nicht lesbar: {e}")))?;
        let url = row.config.get("url").and_then(Value::as_str).unwrap_or("").trim().to_owned();
        (url, crate::connections::secret_bound(&row))
    } else {
        let url = connection.url.clone().unwrap_or_default();
        let bound = connection.secret_target.as_ref().is_none_or(|t| *t == url_target(&url));
        (url, bound)
    };
    if url.is_empty() {
        return Err(LlmError::NotConfigured(format!(
            "Keine Adresse für {} eingetragen (Einstellungen → Verbindungen).",
            connection.name
        )));
    }
    let secret = crate::connections::secrets::get(&state.secrets, &connection.account)
        .await
        .map_err(|e| LlmError::NotConfigured(format!("Schlüsselbund nicht lesbar ({e}); nichts gesendet.")))?;
    if secret.is_some() && !bound {
        return Err(LlmError::NotConfigured(format!(
            "Schlüssel der Verbindung {} gilt für ein anderes Ziel; nicht gesendet. Für die jetzige Adresse den \
             Schlüssel neu eintragen (Einstellungen → Verbindungen).",
            connection.name
        )));
    }
    Ok(Target { connection: connection.clone(), kind, endpoint: Endpoint::new(url, secret) })
}

/// The shared client of a target (one `reqwest::Client` per kind, address and key).
pub fn client_for(target: &Target) -> Result<Client, LlmError> {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    target.endpoint.secret.hash(&mut hasher);
    let key = (target.kind, target.endpoint.base_url.trim().to_owned(), hasher.finish());
    let mut clients = lock(&CLIENTS);
    if let Some(client) = clients.get(&key) {
        return Ok(client.clone());
    }
    let client = Client::new(target.kind, target.endpoint.clone(), Limits::default())?;
    if clients.len() >= 64 {
        clients.clear();
    }
    clients.insert(key, client.clone());
    Ok(client)
}

/// Number of HTTP clients in the pool (for tests: several calls of the same connection share one).
pub fn client_count() -> usize {
    lock(&CLIENTS).len()
}

fn queue_for(url: &str) -> &'static Queue {
    let mut queues = lock(&QUEUES);
    queues.entry(url.trim().to_owned()).or_insert_with(|| Box::leak(Box::new(Queue::default())))
}

/// Old Ollama name → name at Eliezer (`llama3.2:3b`, `qwen3:4b`); otherwise `None`.
pub fn alias(name: &str) -> Option<&'static str> {
    baruch_llm::alias(name)
}

/// State of the queues: label of a running job and number of all waiting.
pub fn status() -> QueueStatus {
    let queues = lock(&QUEUES);
    let mut out = QueueStatus { running: None, waiting: 0 };
    for queue in queues.values() {
        let s = queue.status();
        if out.running.is_none() {
            out.running = s.running;
        }
        out.waiting += s.waiting;
    }
    out
}

/// Environment variable with an upper limit for `max_tokens` of every model call (e.g. `300` for a
/// manual check with short answers); if it is missing or not a number ≥ 1, no limit of its own applies.
pub const MAX_TOKENS_ENV: &str = "BARUCH_MAX_TOKENS";

/// Upper limit from the value of [`MAX_TOKENS_ENV`].
pub fn max_tokens_cap(value: Option<&str>) -> Option<u32> {
    value.and_then(|v| v.trim().parse::<u32>().ok()).filter(|&n| n >= 1)
}

static MAX_TOKENS_CAP: LazyLock<Option<u32>> =
    LazyLock::new(|| max_tokens_cap(std::env::var(MAX_TOKENS_ENV).ok().as_deref()));

/// Connection and assignment of a persona (empty handle or unknown persona: Eliezer, not released).
pub async fn connection_of(state: &AppState, persona: &str) -> Result<(ModelConnection, PersonaBackend), LlmError> {
    let backend = if persona.trim().is_empty() {
        PersonaBackend::default()
    } else {
        match state.store.persona_backend(persona).await {
            Ok(b) => b,
            Err(baruch_store::StoreError::UnknownPersona(_)) => PersonaBackend::default(),
            Err(e) => return Err(LlmError::NotConfigured(format!("Verbindung der Persona nicht lesbar: {e}"))),
        }
    };
    let connection = state
        .store
        .model_connection_get(backend.connection)
        .await
        .map_err(|e| LlmError::NotConfigured(format!("Modell-Verbindung nicht lesbar: {e}")))?;
    Ok((connection, backend))
}

/// Sentence when an external connection is not released for the persona.
pub fn not_allowed(connection: &ModelConnection, persona: &str) -> String {
    let who = if persona.trim().is_empty() { "diesen Auftrag ohne Persona".to_owned() } else { persona.to_owned() };
    format!(
        "Die Verbindung „{}“ ist extern; für {who} ist sie nicht freigegeben (Einstellungen → Assistenten). Es \
         wurde nichts gesendet.",
        connection.name
    )
}

/// Path of a job of the persona `persona`: its connection, resolved. If the connection is external and the
/// persona not released, → `NotConfigured` with [`not_allowed`], and **no** request goes out.
pub async fn route(state: &AppState, persona: &str) -> Result<Target, LlmError> {
    let (connection, backend) = connection_of(state, persona).await?;
    if connection.external && !backend.external_allowed {
        return Err(LlmError::NotConfigured(not_allowed(&connection, persona)));
    }
    resolve(state, &connection).await
}

/// Message with the right account (the error texts of `baruch_llm` name `baruch/eliezer`).
fn with_account(error: LlmError, target: &Target) -> LlmError {
    if target.connection.id == ELIEZER_CONNECTION {
        return error;
    }
    let account = format!("baruch/{}", target.connection.account);
    let fix = |m: String| m.replace("baruch/eliezer", &account);
    match error {
        LlmError::Denied(m) => LlmError::Denied(fix(m)),
        LlmError::Failed(m) => LlmError::Failed(fix(m)),
        other => other,
    }
}

/// A model call through the target `target` through its queue. Aliases apply only to Eliezer.
pub async fn chat_on(
    target: &Target,
    request: ChatRequest,
    label: &str,
    deltas: tokio::sync::mpsc::UnboundedSender<String>,
) -> Result<ChatReply, LlmError> {
    let mut request = request;
    if target.kind == BackendKind::Eliezer
        && let Some(name) = alias(&request.model)
    {
        request.model = name.to_owned();
    }
    if let Some(cap) = *MAX_TOKENS_CAP {
        request.max_tokens = request.max_tokens.min(cap);
    }
    let client = client_for(target)?;
    let result = queue_for(&target.endpoint.base_url)
        .run(label, async {
            client
                .chat_stream(&request, move |piece| {
                    let _ = deltas.send(piece.to_owned());
                })
                .await
        })
        .await;
    result.map_err(|e| with_account(e, target))
}

/// A model call through the connection Eliezer (without a persona, as before Round 15); every piece of text goes
/// to `deltas`. If Eliezer is marked as external, it is refused (no persona, no release).
pub async fn chat(
    state: &AppState,
    request: ChatRequest,
    label: &str,
    deltas: tokio::sync::mpsc::UnboundedSender<String>,
) -> Result<ChatReply, LlmError> {
    let target = route(state, "").await?;
    chat_on(&target, request, label, deltas).await
}

/// Put the list of the connection Eliezer into the cache (from „Verbindung prüfen“ of the connection
/// `eliezer`; hence kind Eliezer).
pub(crate) fn remember(url: &str, models: &[String]) {
    let kind = BackendKind::Eliezer;
    let infos: Vec<ModelInfo> = {
        let cache = lock(&CACHE);
        let old = cache.get(&(kind, url.trim().to_owned()));
        models
            .iter()
            .map(|id| ModelInfo {
                id: id.clone(),
                context_length: old.and_then(|c| c.models.iter().find(|m| &m.id == id)).and_then(|m| m.context_length),
            })
            .collect()
    };
    remember_infos(kind, url, infos);
}

fn remember_infos(kind: BackendKind, url: &str, models: Vec<ModelInfo>) {
    lock(&CACHE).insert((kind, url.trim().to_owned()), Cached { at: Instant::now(), fetched: iso_now(), models });
}

/// Mark cached lists and context lengths of the address `url` as expired (the lists stay
/// as a fallback). For tests and „Verbindung prüfen“; affects only this address, but every kind there.
pub fn expire_list(url: &str) {
    let old = Instant::now().checked_sub(CACHE_FOR + Duration::from_secs(1)).unwrap_or_else(Instant::now);
    for ((_, u), cached) in lock(&CACHE).iter_mut() {
        if u == url.trim() {
            cached.at = old;
        }
    }
    lock(&CONTEXTS).retain(|(_, u, _), _| u != url.trim());
}

fn listing_of(cached: &Cached, live: bool, message: Option<String>) -> Listing {
    Listing {
        models: cached.models.iter().map(|m| m.id.clone()).collect(),
        live,
        fetched: cached.fetched.clone(),
        message,
        infos: cached.models.clone(),
    }
}

/// Model list of the connection Eliezer.
pub async fn list(state: &AppState) -> Result<Listing, LlmError> {
    let connection = state
        .store
        .model_connection_get(ELIEZER_CONNECTION)
        .await
        .map_err(|e| LlmError::NotConfigured(format!("Modell-Verbindung nicht lesbar: {e}")))?;
    list_on(state, &connection).await
}

/// Model list of a connection: queried anew at most every 60 s (not through the queue; the list
/// loads no model). If the remote side does not answer and there is an expired list, that one comes with
/// `live: false` and a notice.
pub async fn list_on(state: &AppState, connection: &ModelConnection) -> Result<Listing, LlmError> {
    let target = resolve(state, connection).await?;
    let key = (target.kind, target.endpoint.base_url.trim().to_owned());
    if let Some(hit) = lock(&CACHE).get(&key).filter(|c| c.at.elapsed() < CACHE_FOR) {
        return Ok(listing_of(hit, false, None));
    }
    let client = client_for(&target)?;
    match client.models().await {
        Ok(models) => {
            remember_infos(key.0, &key.1, models);
            let cache = lock(&CACHE);
            let hit = cache.get(&key).expect("eben gespeichert");
            Ok(listing_of(hit, true, None))
        }
        Err(e) => {
            let e = with_account(e, &target);
            match lock(&CACHE).get(&key) {
                Some(stale) => {
                    let message = format!(
                        "{} antwortet nicht ({e}); Liste aus dem abgelaufenen Zwischenspeicher (Stand {}).",
                        connection.name, stale.fetched
                    );
                    Ok(listing_of(stale, false, Some(message)))
                }
                None => Err(e),
            }
        }
    }
}

/// Sentence for a deviating context length.
fn context_warning(setting: u32, name: &str, reported: u32) -> Option<String> {
    (reported != setting).then(|| {
        if reported < setting {
            format!("Einstellung {setting}, {name} meldet {reported}; wirksam sind {reported} Tokens.")
        } else {
            format!("Einstellung {setting}, {name} meldet {reported}; wirksam bleibt die Einstellung.")
        }
    })
}

fn budget(setting: u32, name: &str, reported: Option<u32>) -> ContextBudget {
    match reported {
        None => ContextBudget { tokens: setting, reported: None, warning: None },
        Some(r) => {
            ContextBudget { tokens: setting.min(r), reported: Some(r), warning: context_warning(setting, name, r) }
        }
    }
}

/// Model name as it goes to the connection (alias only for Eliezer).
fn sent_model(kind: &str, model: &str) -> String {
    if kind == "eliezer" {
        alias(model).map_or_else(|| model.trim().to_owned(), str::to_owned)
    } else {
        model.trim().to_owned()
    }
}

/// Known context length without a network query (from the caches; expired entries count too).
fn cached_context(kind: BackendKind, url: &str, model: &str) -> Option<Option<u32>> {
    if let Some((_, reported)) = lock(&CONTEXTS).get(&(kind, url.trim().to_owned(), model.to_owned())) {
        return Some(*reported);
    }
    let cache = lock(&CACHE);
    let hit = cache.get(&(kind, url.trim().to_owned()))?;
    hit.models.iter().find(|m| m.id == model).and_then(|m| m.context_length).map(Some)
}

/// Context without a network query (for `persona.list`): what the caches know, otherwise the setting.
pub async fn context_cached(state: &AppState, persona: &PersonaRow, connection: &ModelConnection) -> ContextBudget {
    // OpenAI-compatible reports no length (contract §3) – not through a cache of the same
    // address either that another kind (Eliezer) filled (integrator Round 15).
    if connection.kind == "openai" {
        return budget(persona.context_length, &connection.name, None);
    }
    // Round 23: per kind – an unknown kind has no cache.
    let Some(kind) = BackendKind::parse(&connection.kind) else {
        return budget(persona.context_length, &connection.name, None);
    };
    let url = effective_url(state, connection).await;
    let model = sent_model(&connection.kind, &persona.model);
    budget(persona.context_length, &connection.name, cached_context(kind, &url, &model).flatten())
}

/// Context without a network query for a persona through its connection (Round 17a, for pages and lists): like
/// [`context_cached`]; without a readable connection the setting.
pub async fn context_cached_for(state: &AppState, persona: &PersonaRow) -> ContextBudget {
    match connection_of(state, &persona.handle).await {
        Ok((connection, _)) => context_cached(state, persona, &connection).await,
        Err(_) => budget(persona.context_length, "", None),
    }
}

/// Model list of a connection without a network query (Round 17a): the cache, also expired; `None` if
/// never queried yet.
pub async fn cached_list_on(state: &AppState, connection: &ModelConnection) -> Option<Listing> {
    let kind = BackendKind::parse(&connection.kind)?;
    let url = effective_url(state, connection).await;
    lock(&CACHE).get(&(kind, url.trim().to_owned())).map(|hit| listing_of(hit, false, None))
}

/// Context for jobs to `persona`: setting reconciled with the reported length of the remote side.
/// Waits at most as long as the model list (cache 60 s); never fails (without a report
/// the setting applies).
///
/// Round 23: the release is checked **before** every request as in [`route`]. If the connection is external and
/// the persona not released, nothing goes out (no `/api/show` or `/api/v1/models` either – both would carry
/// key and model name) and the setting applies; the job itself is refused afterwards anyway.
pub async fn context(state: &AppState, persona: &PersonaRow) -> ContextBudget {
    let setting = persona.context_length;
    let Ok((connection, backend)) = connection_of(state, &persona.handle).await else {
        return budget(setting, "", None);
    };
    if connection.external && !backend.external_allowed {
        return budget(setting, &connection.name, None);
    }
    if connection.kind == "openai" {
        return budget(setting, &connection.name, None);
    }
    let Ok(target) = resolve(state, &connection).await else { return budget(setting, &connection.name, None) };
    let url = target.endpoint.base_url.trim().to_owned();
    let model = sent_model(&connection.kind, &persona.model);
    let key = (target.kind, url.clone(), model.clone());
    if let Some((at, reported)) = lock(&CONTEXTS).get(&key).copied()
        && at.elapsed() < CACHE_FOR
    {
        return budget(setting, &connection.name, reported);
    }
    if let Some(Some(reported)) = lock(&CACHE)
        .get(&(target.kind, url.clone()))
        .filter(|c| c.at.elapsed() < CACHE_FOR)
        .map(|c| c.models.iter().find(|m| m.id == model).and_then(|m| m.context_length))
    {
        lock(&CONTEXTS).insert(key, (Instant::now(), Some(reported)));
        return budget(setting, &connection.name, Some(reported));
    }
    let reported = match client_for(&target) {
        Ok(client) => {
            let wait = client.limits().models_total;
            tokio::time::timeout(wait, client.context_length(&model)).await.ok().and_then(Result::ok).flatten()
        }
        Err(_) => None,
    };
    lock(&CONTEXTS).insert(key, (Instant::now(), reported));
    budget(setting, &connection.name, reported)
}

/// Model of a persona against the list: `{handle, model, active, state: "bekannt" | "alias" |
/// "unbekannt", target?}`. „alias“: the old name has an alias that Eliezer knows (`target`);
/// if Eliezer does not know the alias either, the persona is „unbekannt“ (with `target` as a hint).
pub fn persona_states(personas: &[PersonaRow], models: &[String]) -> Vec<Value> {
    states_of(personas.iter().map(|p| (p.handle.as_str(), p.model.as_str(), p.active)), models)
}

/// Like [`persona_states`] for `(handle, model, active)`.
fn states_of<'a>(personas: impl Iterator<Item = (&'a str, &'a str, bool)>, models: &[String]) -> Vec<Value> {
    personas
        .map(|(handle, model, active)| {
            let mut out = json!({ "handle": handle, "model": model, "active": active });
            if models.iter().any(|m| m == model) {
                out["state"] = json!("bekannt");
            } else if let Some(target) = alias(model) {
                let known = models.iter().any(|m| m == target);
                out["state"] = json!(if known { "alias" } else { "unbekannt" });
                out["target"] = json!(target);
            } else {
                out["state"] = json!("unbekannt");
            }
            out
        })
        .collect()
}

/// Sentence for the reconciliation (for „Verbindung prüfen“): unknown models and aliases; empty if all are known.
pub fn persona_summary(states: &[Value]) -> String {
    let field = |v: &Value, name: &str| v.get(name).and_then(Value::as_str).unwrap_or("").to_owned();
    let unknown: Vec<String> = states
        .iter()
        .filter(|s| field(s, "state") == "unbekannt")
        .map(|s| format!("{} ({})", field(s, "handle"), field(s, "model")))
        .collect();
    let aliased: Vec<String> = states
        .iter()
        .filter(|s| field(s, "state") == "alias")
        .map(|s| format!("{} {} → {}", field(s, "handle"), field(s, "model"), field(s, "target")))
        .collect();
    let mut parts = Vec::new();
    if !unknown.is_empty() {
        parts.push(format!("Unbekannt bei Eliezer: {}.", unknown.join(", ")));
    }
    if !aliased.is_empty() {
        parts.push(format!("Alias: {} (Baruch löst den alten Namen beim Senden auf).", aliased.join(", ")));
    }
    parts.join(" ")
}

/// Aliases as an object `{alt: neu}`.
fn aliases() -> Value {
    Value::Object(baruch_llm::ALIASES.iter().map(|(old, new)| ((*old).to_owned(), json!(new))).collect::<Map<_, _>>())
}

/// Short test on the connection Eliezer ([`probe_on`]).
pub async fn probe(state: &AppState, model: &str) -> Value {
    probe_on(state, ELIEZER_CONNECTION, model).await
}

/// Short test: fixed prompt [`PROBE_PROMPT`], `max_tokens` 16, through the queue of the connection. A
/// failure is a result (`ok: false` with a message), not an error. The result is in the
/// database afterwards (survives a restart). No book text: also works through an external connection without a
/// release.
pub async fn probe_on(state: &AppState, connection: i64, model: &str) -> Value {
    let model = model.trim();
    let started = Instant::now();
    let request = ChatRequest {
        model: model.to_owned(),
        messages: vec![ChatMessage { role: Role::User, content: PROBE_PROMPT.to_owned() }],
        max_tokens: PROBE_MAX_TOKENS,
        temperature: Some(0.0),
    };
    let (deltas, _ignored) = tokio::sync::mpsc::unbounded_channel();
    let result = match state.store.model_connection_get(connection).await {
        Ok(row) => match resolve(state, &row).await {
            Ok(target) => chat_on(&target, request, &format!("Kurztest {model}"), deltas).await,
            Err(e) => Err(e),
        },
        Err(e) => Err(LlmError::NotConfigured(e.to_string())),
    };
    let ms = started.elapsed().as_millis() as u64;
    let mut out = match result {
        Ok(reply) => {
            let text: String = reply.text.trim().chars().take(200).collect();
            json!({ "ok": true, "model": reply.model, "requested": model, "reply": text, "ms": ms,
                    "finish_reason": reply.finish_reason, "completion_tokens": reply.completion_tokens })
        }
        Err(e) => json!({ "ok": false, "model": model, "requested": model, "message": e.to_string(), "ms": ms }),
    };
    out["at"] = json!(iso_now());
    out["connection"] = json!(connection);
    let _ = state.store.model_connection_record_probe(connection, &out).await;
    out
}

/// Last short test of the connection Eliezer (from the database).
pub async fn last_probe(state: &AppState) -> Option<Value> {
    state.store.model_connection_get(ELIEZER_CONNECTION).await.ok().and_then(|c| c.probe)
}

/// Does this module serve the RPC method?
pub fn handles(method: &str) -> bool {
    METHODS.contains(&method)
}

fn failure(e: LlmError) -> RpcError {
    RpcError::new(REFUSED, e.to_string())
}

/// State of the key of a connection: `hinterlegt`, `fehlt` or `null` (keyring not readable).
async fn secret_state(state: &AppState, connection: &ModelConnection) -> Value {
    match crate::connections::secrets::get(&state.secrets, &connection.account).await {
        Ok(Some(_)) => json!("hinterlegt"),
        Ok(None) => json!("fehlt"),
        Err(_) => Value::Null,
    }
}

/// A connection as JSON (never with the key): kind, address, local/external, key stored/missing,
/// last short test and last check, personas that use it.
pub async fn describe(state: &AppState, connection: &ModelConnection) -> Value {
    let url = effective_url(state, connection).await;
    let secret = secret_state(state, connection).await;
    let bound = if connection.id == ELIEZER_CONNECTION {
        state
            .store
            .connection_get(ConnectionKind::Eliezer)
            .await
            .map(|r| crate::connections::secret_bound(&r))
            .unwrap_or(true)
    } else {
        connection.secret_target.as_ref().is_none_or(|t| *t == url_target(&url))
    };
    let personas: Vec<String> = state
        .store
        .persona_backends()
        .await
        .unwrap_or_default()
        .into_iter()
        .filter(|(_, b)| b.connection == connection.id)
        .map(|(h, _)| h)
        .collect();
    let mut out = json!({
        "id": connection.id, "name": connection.name, "kind": connection.kind,
        "kind_title": kind_title(&connection.kind), "url": url, "external": connection.external,
        "locality": locality(connection), "account": connection.account, "secret": secret,
        "probe": connection.probe, "check": connection.check, "position": connection.position,
        "eliezer": connection.id == ELIEZER_CONNECTION, "personas": personas,
    });
    if let Some(warning) = locality_warning(connection.external, &url) {
        out["locality_warning"] = json!(warning);
    }
    if secret == json!("hinterlegt") && !bound {
        out["secret_for"] = json!(connection.secret_target);
        out["secret_withheld"] = json!(true);
    }
    out
}

fn id_param(params: &Value, name: &str) -> Result<i64, RpcError> {
    match params.get(name) {
        Some(Value::Number(n)) => {
            n.as_i64().ok_or_else(|| RpcError::new(INVALID_PARAMS, format!("{name}: Zahl erwartet")))
        }
        Some(Value::String(s)) => {
            s.trim().parse().map_err(|_| RpcError::new(INVALID_PARAMS, format!("{name}: Zahl erwartet")))
        }
        _ => Err(RpcError::new(INVALID_PARAMS, format!("{name} fehlt (id der Modell-Verbindung)"))),
    }
}

/// Connection from `connection` (id) or the persona `persona`; without both Eliezer.
async fn connection_param(state: &AppState, params: &Value) -> Result<ModelConnection, RpcError> {
    if params.get("connection").is_some_and(|c| !c.is_null()) {
        return Ok(state.store.model_connection_get(id_param(params, "connection")?).await?);
    }
    if let Some(persona) = params.get("persona").and_then(Value::as_str).filter(|p| !p.trim().is_empty()) {
        let backend = state.store.persona_backend(persona).await?;
        return Ok(state.store.model_connection_get(backend.connection).await?);
    }
    Ok(state.store.model_connection_get(ELIEZER_CONNECTION).await?)
}

/// Check the fields of `model.connection_set` (name, kind, address through `connections::config::check_url`).
fn edit_param(
    params: &Value,
    eliezer: bool,
    current: Option<&ModelConnection>,
) -> Result<ModelConnectionEdit, RpcError> {
    let text = |name: &str| params.get(name).and_then(Value::as_str).map(str::trim).map(str::to_owned);
    let name = text("name").or_else(|| current.map(|c| c.name.clone())).unwrap_or_default();
    let kind = text("kind").or_else(|| current.map(|c| c.kind.clone())).unwrap_or_default();
    if BackendKind::parse(&kind).is_none() {
        return Err(RpcError::new(INVALID_PARAMS, "kind: eliezer, openai oder ollama."));
    }
    let url = match text("url") {
        Some(url) => crate::connections::config::check_url(&url).map_err(RpcError::from)?,
        None if eliezer => String::new(),
        None => current.and_then(|c| c.url.clone()).ok_or_else(|| RpcError::new(INVALID_PARAMS, "url fehlt."))?,
    };
    // Without a value: when creating by the address (not recognisably local → external); when changing „extern“ stays,
    // and a new address that is not recognisably local makes the connection external – a forgotten tick never opens
    // the lock. Whoever runs a service in the own network under an ordinary name sets `false` explicitly.
    let auto = |url: &str| !url.is_empty() && !looks_local(url);
    let external = match params.get("external") {
        Some(Value::Bool(b)) => *b,
        None | Some(Value::Null) => match current {
            Some(c) => c.external || (text("url").is_some() && auto(&url)),
            None => auto(&url),
        },
        Some(_) => return Err(RpcError::new(INVALID_PARAMS, "external: true oder false.")),
    };
    Ok(ModelConnectionEdit { name, kind: kind.to_lowercase(), url, external })
}

/// `model.connection_set`: create (without `id`) or change. For Eliezer the address goes to the connection
/// `eliezer` (with its binding of the key); for others a new target binds the key to the old one.
pub async fn set_connection(state: &AppState, params: &Value) -> Result<ModelConnection, RpcError> {
    let id = match params.get("id") {
        None | Some(Value::Null) => None,
        Some(Value::String(s)) if s.trim().is_empty() => None,
        Some(_) => Some(id_param(params, "id")?),
    };
    let Some(id) = id else {
        let edit = edit_param(params, false, None)?;
        return Ok(state.store.model_connection_create(&edit).await?);
    };
    let current = state.store.model_connection_get(id).await?;
    let eliezer = id == ELIEZER_CONNECTION;
    let edit = edit_param(params, eliezer, Some(&current))?;
    if eliezer {
        if !edit.url.is_empty() {
            crate::connections::set_config(state, ConnectionKind::Eliezer, &json!({ "url": edit.url }))
                .await
                .map_err(RpcError::from)?;
        }
        return Ok(state.store.model_connection_update(id, &edit).await?);
    }
    let _held = state.write_lock.lock().await;
    let before = url_target(current.url.as_deref().unwrap_or(""));
    let after = url_target(&edit.url);
    if before != after && current.secret_target.is_none() {
        let held = !matches!(crate::connections::secrets::get(&state.secrets, &current.account).await, Ok(None));
        if held {
            state.store.model_connection_bind_secret(id, Some(&before)).await?;
        }
    }
    let mut row = state.store.model_connection_update(id, &edit).await?;
    if row.secret_target.as_deref() == Some(after.as_str()) {
        state.store.model_connection_bind_secret(id, None).await?;
        row.secret_target = None;
    }
    Ok(row)
}

fn secret_text(params: &Value) -> Result<String, RpcError> {
    let secret = params.get("secret").and_then(Value::as_str).unwrap_or("").trim();
    if secret.is_empty() {
        return Err(RpcError::new(INVALID_PARAMS, "secret: leer; nichts geändert."));
    }
    if secret.chars().count() > 4096 || secret.chars().any(char::is_control) {
        return Err(RpcError::new(INVALID_PARAMS, "secret: höchstens 4096 Zeichen in einer Zeile."));
    }
    Ok(secret.to_owned())
}

/// Store the key of a connection; afterwards it applies to its present target. Eliezer: account `eliezer` with the
/// binding of the connection `eliezer`.
pub async fn set_secret(state: &AppState, id: i64, secret: &str) -> Result<(), RpcError> {
    if id == ELIEZER_CONNECTION {
        return crate::connections::set_secret(state, ConnectionKind::Eliezer, secret).await.map_err(RpcError::from);
    }
    let connection = state.store.model_connection_get(id).await?;
    let _held = state.write_lock.lock().await;
    crate::connections::secrets::set(&state.secrets, &connection.account, secret)
        .await
        .map_err(|e| RpcError::new(INTERNAL, format!("Schlüsselbund: {e}")))?;
    state.store.model_connection_bind_secret(id, None).await?;
    Ok(())
}

/// Remove the key of a connection.
pub async fn delete_secret(state: &AppState, id: i64) -> Result<(), RpcError> {
    if id == ELIEZER_CONNECTION {
        return crate::connections::delete_secret(state, ConnectionKind::Eliezer).await.map_err(RpcError::from);
    }
    let connection = state.store.model_connection_get(id).await?;
    let _held = state.write_lock.lock().await;
    crate::connections::secrets::delete(&state.secrets, &connection.account)
        .await
        .map_err(|e| RpcError::new(INTERNAL, format!("Schlüsselbund: {e}")))?;
    state.store.model_connection_bind_secret(id, None).await?;
    Ok(())
}

/// Delete a connection (not Eliezer); its key is removed with it, its personas fall back to Eliezer.
pub async fn delete_connection(state: &AppState, id: i64) -> Result<Vec<String>, RpcError> {
    let connection = state.store.model_connection_get(id).await?;
    let handles = state.store.model_connection_delete(id).await?;
    let _ = crate::connections::secrets::delete(&state.secrets, &connection.account).await;
    Ok(handles)
}

/// Check a connection: query the model list live (with context lengths as far as reported), store the result.
/// Eliezer: like „Verbindung prüfen“ of the connection `eliezer` (reconciles the models of the personas).
pub async fn check_connection(state: &AppState, id: i64) -> Result<Value, RpcError> {
    let connection = state.store.model_connection_get(id).await?;
    let mut out = if id == ELIEZER_CONNECTION {
        crate::connections::run_check(state, ConnectionKind::Eliezer).await.map_err(RpcError::from)?
    } else {
        let outcome = match resolve(state, &connection).await {
            Err(e) => json!({ "ok": false, "message": e.to_string(), "details": {} }),
            Ok(target) => match client_for(&target) {
                Err(e) => json!({ "ok": false, "message": e.to_string(), "details": {} }),
                Ok(client) => match client.models().await {
                    Ok(models) => {
                        let key = target.endpoint.base_url.trim().to_owned();
                        remember_infos(target.kind, &key, models.clone());
                        let word = if models.len() == 1 { "Modell" } else { "Modelle" };
                        json!({ "ok": true, "message": format!("Verbindung steht: {} {word}.", models.len()),
                                "details": { "models": models.iter().map(|m| m.id.clone()).collect::<Vec<_>>(),
                                             "contexts": models } })
                    }
                    Err(e) => {
                        let e = with_account(e, &target);
                        json!({ "ok": false, "message": e.to_string(), "details": {} })
                    }
                },
            },
        };
        let mut outcome = outcome;
        if let Some(secret) = crate::connections::secrets::get(&state.secrets, &connection.account).await.ok().flatten()
        {
            outcome = crate::connections::check::scrub_value(outcome, Some(&secret));
        }
        outcome
    };
    out["at"] = json!(iso_now());
    out["connection"] = json!(id);
    state.store.model_connection_record_check(id, &out).await?;
    Ok(out)
}

/// Round 23: deleting (connection, key) only with `confirm: true` – like `work.delete` and the forms with
/// their mandatory checkbox. Otherwise -32602, and nothing is deleted.
fn confirmed(params: &Value) -> Result<(), RpcError> {
    if params.get("confirm").and_then(Value::as_bool) == Some(true) {
        Ok(())
    } else {
        Err(RpcError::new(INVALID_PARAMS, "confirm: true erwartet"))
    }
}

/// RPC `model.*` (`docs/RPC.md`, „Modelle“). Errors of the backend for `model.list` without a cache →
/// -32000 with a German sentence.
pub async fn call(state: &AppState, method: &str, params: Value) -> Result<Value, RpcError> {
    let params = if params.is_null() { json!({}) } else { params };
    if !params.is_object() {
        return Err(RpcError::new(INVALID_PARAMS, "Parameter als Objekt erwartet"));
    }
    match method {
        "model.list" => {
            let connection = connection_param(state, &params).await?;
            let listing = list_on(state, &connection).await.map_err(failure)?;
            let personas = state.store.persona_list().await.map_err(RpcError::from)?;
            let backends = state.store.persona_backends().await?;
            let on_this: Vec<PersonaRow> = personas
                .into_iter()
                .filter(|p| {
                    backends.iter().find(|(h, _)| *h == p.handle).map_or(ELIEZER_CONNECTION, |(_, b)| b.connection)
                        == connection.id
                })
                .collect();
            Ok(json!({
                "models": listing.models, "live": listing.live, "fetched": listing.fetched,
                "message": listing.message, "contexts": listing.infos,
                "aliases": aliases(), "personas": persona_states(&on_this, &listing.models),
                "connection": describe(state, &connection).await,
            }))
        }
        "model.status" => Ok(json!(status())),
        "model.probe" => {
            let model = params.get("model").and_then(Value::as_str).map(str::trim).unwrap_or("");
            if model.is_empty() || model.chars().count() > 200 || model.chars().any(char::is_control) {
                return Err(RpcError::new(INVALID_PARAMS, "model: Modellname (1–200 Zeichen, eine Zeile) erwartet."));
            }
            let connection = connection_param(state, &params).await?;
            Ok(probe_on(state, connection.id, model).await)
        }
        "model.context" => {
            let persona = params
                .get("persona")
                .and_then(Value::as_str)
                .filter(|p| !p.trim().is_empty())
                .ok_or_else(|| RpcError::new(INVALID_PARAMS, "persona fehlt (z. B. „@lektor“)"))?;
            let want = persona.trim().trim_start_matches('@');
            let row = state
                .store
                .persona_list()
                .await?
                .into_iter()
                .find(|p| p.handle.trim_start_matches('@') == want)
                .ok_or_else(|| RpcError::from(baruch_store::StoreError::UnknownPersona(persona.trim().to_owned())))?;
            let budget = state.model.context(state, &row).await;
            let mut out = json!(budget);
            out["persona"] = json!(row.handle);
            out["setting"] = json!(row.context_length);
            Ok(out)
        }
        "model.connections" => {
            let mut list = Vec::new();
            for connection in state.store.model_connection_list().await? {
                list.push(describe(state, &connection).await);
            }
            Ok(json!({ "connections": list, "kinds": BackendKind::ALL.iter().map(|k| json!({
                "kind": k.as_str(), "title": kind_title(k.as_str()) })).collect::<Vec<_>>() }))
        }
        "model.connection_set" => {
            let row = set_connection(state, &params).await?;
            Ok(describe(state, &row).await)
        }
        "model.connection_delete" => {
            let id = id_param(&params, "id")?;
            confirmed(&params)?;
            let personas = delete_connection(state, id).await?;
            Ok(json!({ "id": id, "deleted": true, "personas": personas }))
        }
        "model.connection_set_secret" => {
            let id = id_param(&params, "id")?;
            let secret = secret_text(&params)?;
            set_secret(state, id, &secret).await?;
            Ok(json!({ "id": id, "secret": "hinterlegt" }))
        }
        "model.connection_delete_secret" => {
            let id = id_param(&params, "id")?;
            confirmed(&params)?;
            delete_secret(state, id).await?;
            Ok(json!({ "id": id, "secret": "fehlt" }))
        }
        "model.connection_check" => check_connection(state, id_param(&params, "id")?).await,
        _ => Err(RpcError::new(METHOD_NOT_FOUND, format!("unknown method {method}"))),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_max_tokens_cap_takes_only_positive_numbers() {
        assert_eq!(super::max_tokens_cap(Some(" 300 ")), Some(300), "Zahl mit Leerraum gilt");
        assert_eq!(super::max_tokens_cap(Some("0")), None, "0 wäre keine Antwort: keine Grenze");
        assert_eq!(super::max_tokens_cap(Some("viel")), None, "keine Zahl: keine Grenze");
        assert_eq!(super::max_tokens_cap(None), None, "ohne Variable: keine Grenze");
    }

    use super::*;

    #[test]
    fn the_time_is_iso_8601_in_utc() {
        let now = iso_now();
        assert_eq!(now.len(), 24, "Form 2026-11-06T09:30:12.345Z: {now}");
        assert!(now.ends_with('Z') && &now[4..5] == "-" && &now[10..11] == "T", "ISO mit Z: {now}");
        assert!(now.as_str() > "2026-01-01", "plausibles Datum: {now}");
    }

    #[test]
    fn the_effective_context_is_the_smaller_length_with_a_warning() {
        let b = budget(65536, "Eliezer", Some(32768));
        assert_eq!(b.tokens, 32768, "Minimum aus Einstellung und Meldung");
        assert!(
            b.warning.as_deref().is_some_and(|w| w.contains("Einstellung 65536, Eliezer meldet 32768")),
            "Warnung: {b:?}"
        );
        let b = budget(8192, "Ollama", Some(8192));
        assert_eq!((b.tokens, b.warning), (8192, None), "gleich: keine Warnung");
        let b = budget(4096, "OpenAI", None);
        assert_eq!((b.tokens, b.reported, b.warning), (4096, None, None), "ohne Meldung gilt die Einstellung");
        let b = budget(4096, "X", Some(131072));
        assert_eq!(b.tokens, 4096, "größere Meldung ändert die Einstellung nicht");
        assert!(b.warning.is_some(), "aber Hinweis auf die Abweichung");
    }

    #[test]
    fn a_secret_is_bound_to_the_origin_of_the_address() {
        assert_eq!(url_target("http://h:11434/v1"), "http://h:11434", "Pfad zählt nicht");
        assert_ne!(url_target("http://h:11434"), url_target("http://h:11435"), "anderer Port, anderes Ziel");
    }

    #[test]
    fn persona_models_are_compared_with_aliases() {
        // No PersonaRow literals: the structure grows (Round 10, personas).
        let rows = [("@a", "m1", true), ("@b", "llama3.2:3b", true), ("@c", "qwen3:4b", false), ("@d", "x", true)];
        let models = ["m1".to_owned(), "llama-3.2-3b-instruct-q4-k-m".to_owned()];
        let states = states_of(rows.into_iter(), &models);
        let state = |i: usize| states[i]["state"].as_str().unwrap().to_owned();
        assert_eq!(state(0), "bekannt", "Name steht in der Liste");
        assert_eq!((state(1), &states[1]["target"]), ("alias".into(), &json!("llama-3.2-3b-instruct-q4-k-m")), "Alias");
        assert_eq!(
            (state(2), &states[2]["target"]),
            ("unbekannt".into(), &json!("qwen3-4b-q4-k-m")),
            "Alias unbekannt"
        );
        assert_eq!(state(3), "unbekannt", "ganz unbekannt");
        let summary = persona_summary(&states);
        assert!(summary.contains("@c (qwen3:4b)") && summary.contains("@d (x)"), "Unbekannte genannt: {summary}");
        assert!(summary.contains("@b llama3.2:3b → llama-3.2-3b-instruct-q4-k-m"), "Alias-Hinweis: {summary}");
    }
}
