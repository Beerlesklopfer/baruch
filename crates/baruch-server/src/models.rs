// SPDX-License-Identifier: AGPL-3.0-or-later
//! Modell-Backends im Server (Runde 10, Aufgabe „backend“; Runde 15b „anbieter“): Verbindungen der Art
//! Eliezer, OpenAI-kompatibel oder Ollama ([`baruch_llm::Client`], ein HTTP-Client je Endpunkt), je Persona
//! eine Verbindung mit „lokal“/„extern“ und Freigabe, Warteschlange **je Endpunkt** ([`status`], genau eine
//! laufende Anfrage, höchstens acht Wartende), Modellliste mit Zwischenspeicher, Kurztest, Abgleich der
//! Kontextlänge und die RPC-Methoden `model.*` ([`handles`], [`call`]; nur für den Menschen – nicht über
//! `POST /act`, nicht über MCP).
//!
//! **Verbindungen** (Store `model_connection`, Migration 0036): Die Verbindung „Eliezer“
//! ([`baruch_store::ELIEZER_CONNECTION`]) nimmt Adresse und Bindung des Schlüssels weiter aus der Verbindung
//! `eliezer` (Runde 9, Einstellungen → Verbindungen), den Schlüssel aus dem Konto `baruch/eliezer`. Weitere
//! Verbindungen haben eine eigene Adresse und das Konto `baruch/model-<id>`; ihr Schlüssel ist an das Ziel
//! (Ursprung der Adresse) gebunden, für das er hinterlegt wurde – zeigt die Adresse anderswohin, geht er nicht
//! hinaus ([`LlmError::NotConfigured`]).
//!
//! **Extern:** Eine Verbindung „extern“ schickt Buchtext aus dem Haus. Ein Auftrag einer Persona geht über
//! sie nur, wenn die Persona dafür freigegeben ist (`persona_backend.external_allowed`, Vorgabe aus); sonst
//! lehnt [`route`] mit einem deutschen Satz ab, **bevor** eine Anfrage hinausgeht. Kurztest und
//! Modellliste schicken keinen Buchtext und gehen auch ohne Freigabe.
//!
//! **Modellliste:** 60 s je (Art, Adresse) zwischengespeichert ([`CACHE_FOR`]; `live: false` aus dem Speicher;
//! seit Runde 23 je Art – zwei Verbindungen verschiedener Art auf derselben Adresse teilen weder Liste noch
//! Kontextlänge).
//! Antwortet die Gegenstelle nicht, liefert [`list_on`] die abgelaufene Liste mit `live: false` und einem
//! Hinweis (statt eines Fehlers), solange es eine gibt.
//!
//! **Kontext** ([`context`]): wirksame Länge = min(Einstellung der Persona, von der Gegenstelle gemeldete
//! Länge); Warnung bei Abweichung. Gemeldet wird nur lesend (Eliezer `GET /api/v1/models`, Ollama
//! `POST /api/show`, OpenAI nichts); Baruch fordert nie eine Kontextlänge an und ruft nie
//! `/api/v1/models/load|unload|estimate` auf. Der Kurztest wird in der Datenbank gespeichert. Seit Runde 23
//! prüft [`context`] zuerst die Freigabe: Für eine externe Verbindung ohne Freigabe der Persona geht auch der
//! Abgleich nicht hinaus (er trüge Schlüssel und Modellnamen), es gilt die Einstellung.
//!
//! **Löschen** (Runde 23): `model.connection_delete` und `model.connection_delete_secret` verlangen
//! `confirm: true` (sonst -32602); die Formulare prüfen das Pflicht-Kästchen und rufen dann dieselben Funktionen.

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

/// Wie lange eine Modellliste (und eine gemeldete Kontextlänge) gilt.
pub const CACHE_FOR: Duration = Duration::from_secs(60);
/// Fester Auftrag des Kurztests.
pub const PROBE_PROMPT: &str = "Antworte nur mit: OK";
/// `max_tokens` des Kurztests.
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

/// Methoden, die etwas ändern (nur Mensch; nicht über `/act`, nicht über MCP).
pub const WRITES: &[&str] = &[
    "model.connection_set",
    "model.connection_delete",
    "model.connection_set_secret",
    "model.connection_delete_secret",
];

/// Warteschlangen je Endpunkt (Adresse), für den ganzen Prozess.
static QUEUES: LazyLock<Mutex<HashMap<String, &'static Queue>>> = LazyLock::new(Default::default);
/// HTTP-Clients je (Art, Adresse, Fingerabdruck des Schlüssels).
/// Schlüssel des Client-Vorrats: Art, Adresse, Fingerabdruck des Schlüssels.
type ClientKey = (BackendKind, String, u64);
static CLIENTS: LazyLock<Mutex<HashMap<ClientKey, Client>>> = LazyLock::new(Default::default);

struct Cached {
    at: Instant,
    fetched: String,
    models: Vec<ModelInfo>,
}

/// Modelllisten je (Art, Adresse) – seit Runde 23 je Art: Eine Eliezer- und eine Ollama-Verbindung auf
/// derselben Adresse melden verschiedene Listen und Kontextlängen.
static CACHE: LazyLock<Mutex<HashMap<(BackendKind, String), Cached>>> = LazyLock::new(Default::default);
/// Gemeldete Kontextlängen je (Art, Adresse, Modell).
/// (Art, Adresse, Modell) → (Zeitpunkt, gemeldete Länge).
type ContextCache = HashMap<(BackendKind, String, String), (Instant, Option<u32>)>;
static CONTEXTS: LazyLock<Mutex<ContextCache>> = LazyLock::new(Default::default);

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// Modellliste einer Verbindung.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Listing {
    pub models: Vec<String>,
    /// `true`: eben abgefragt; `false`: aus dem Zwischenspeicher.
    pub live: bool,
    /// Zeitpunkt der Abfrage (ISO 8601, UTC).
    pub fetched: String,
    /// Hinweis, wenn die Liste aus dem abgelaufenen Zwischenspeicher kommt (Gegenstelle antwortet nicht).
    pub message: Option<String>,
    /// Je Modell die gemeldete Kontextlänge (soweit die Gegenstelle sie in der Liste meldet).
    pub infos: Vec<ModelInfo>,
}

/// Jetzt als `2026-11-06T09:30:12.345Z`.
pub fn iso_now() -> String {
    let since = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
    let secs = since.as_secs() as i64;
    let (days, rest) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    // Tage seit 1970-01-01 → Datum (H. Hinnant, „civil_from_days“).
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

/// Ursprung einer Adresse (Schema, Rechner, Port): das Ziel, an das ein Schlüssel gebunden ist.
pub fn url_target(url: &str) -> String {
    reqwest::Url::parse(url.trim()).map_or_else(|_| url.trim().to_owned(), |u| u.origin().ascii_serialization())
}

/// Eine Verbindung, bereit für Anfragen: Art, Adresse, Schlüssel (nur, wenn er dem jetzigen Ziel gilt).
#[derive(Clone)]
pub struct Target {
    pub connection: ModelConnection,
    pub kind: BackendKind,
    pub endpoint: Endpoint,
}

/// Liegt die Adresse erkennbar auf diesem Rechner oder im eigenen Netz? Ja bei Loopback (`127.0.0.0/8`, `::1`,
/// `localhost`, `*.localhost`), privaten und Link-local-Adressen (`10/8`, `172.16/12`, `192.168/16`, `169.254/16`,
/// `100.64/10`, `fc00::/7`, `fe80::/10`), Rechnernamen ohne Punkt und den Namen unter `.local`, `.lan`,
/// `.internal`, `.home.arpa`. Alles andere (öffentliche Adressen, gewöhnliche Domänennamen) gilt als **nicht**
/// erkennbar lokal – Baruch fragt kein DNS. Daraus folgt die Vorgabe „extern“ beim Anlegen und die Warnung
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

/// Warnung, wenn eine als lokal geführte Verbindung auf eine nicht erkennbar lokale Adresse zeigt (Buchtext ginge
/// ohne Freigabe hinaus); sonst `None`.
pub fn locality_warning(external: bool, url: &str) -> Option<String> {
    (!external && !url.trim().is_empty() && !looks_local(url)).then(|| {
        "Die Adresse liegt nicht erkennbar auf diesem Rechner oder im eigenen Netz, die Verbindung ist aber als lokal \
         geführt: Buchtext geht ohne Freigabe je Persona dorthin. Liegt der Dienst außerhalb, „extern“ ankreuzen."
            .to_owned()
    })
}

/// Anzeige „lokal“ bzw. „extern“.
pub fn locality(connection: &ModelConnection) -> &'static str {
    if connection.external { "extern" } else { "lokal" }
}

/// Anzeigename einer Art.
pub fn kind_title(kind: &str) -> &'static str {
    match kind {
        "eliezer" => "Eliezer",
        "openai" => "OpenAI-kompatibel",
        "ollama" => "Ollama",
        _ => "unbekannt",
    }
}

/// Adresse der Verbindung Eliezer (aus der Verbindung `eliezer`).
async fn eliezer_url(state: &AppState) -> Result<String, LlmError> {
    let row = state
        .store
        .connection_get(ConnectionKind::Eliezer)
        .await
        .map_err(|e| LlmError::NotConfigured(format!("Verbindung eliezer nicht lesbar: {e}")))?;
    Ok(row.config.get("url").and_then(Value::as_str).unwrap_or("").trim().to_owned())
}

/// Wirksame Adresse einer Verbindung (Eliezer aus der Verbindung `eliezer`).
pub async fn effective_url(state: &AppState, connection: &ModelConnection) -> String {
    match &connection.url {
        Some(url) => url.clone(),
        None => eliezer_url(state).await.unwrap_or_default(),
    }
}

/// Ziel einer Verbindung auflösen: Adresse, Schlüssel aus dem Schlüsselbund (nur, wenn er an das jetzige Ziel
/// gebunden ist). Ohne Adresse oder mit Schlüssel für ein anderes Ziel → `NotConfigured` (nichts gesendet).
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

/// Der gemeinsame Client eines Ziels (ein `reqwest::Client` je Art, Adresse und Schlüssel).
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

/// Zahl der HTTP-Clients im Vorrat (für Tests: mehrere Aufrufe derselben Verbindung teilen einen).
pub fn client_count() -> usize {
    lock(&CLIENTS).len()
}

fn queue_for(url: &str) -> &'static Queue {
    let mut queues = lock(&QUEUES);
    queues.entry(url.trim().to_owned()).or_insert_with(|| Box::leak(Box::new(Queue::default())))
}

/// Alter Ollama-Name → Name bei Eliezer (`llama3.2:3b`, `qwen3:4b`); sonst `None`.
pub fn alias(name: &str) -> Option<&'static str> {
    baruch_llm::alias(name)
}

/// Zustand der Warteschlangen: Bezeichnung eines laufenden Auftrags und Zahl aller Wartenden.
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

/// Umgebungsvariable mit einer Obergrenze für `max_tokens` jedes Modellaufrufs (z. B. `300` für eine
/// Handprüfung mit kurzen Antworten); fehlt sie oder ist sie keine Zahl ≥ 1, gilt keine eigene Grenze.
pub const MAX_TOKENS_ENV: &str = "BARUCH_MAX_TOKENS";

/// Obergrenze aus dem Wert von [`MAX_TOKENS_ENV`].
pub fn max_tokens_cap(value: Option<&str>) -> Option<u32> {
    value.and_then(|v| v.trim().parse::<u32>().ok()).filter(|&n| n >= 1)
}

static MAX_TOKENS_CAP: LazyLock<Option<u32>> =
    LazyLock::new(|| max_tokens_cap(std::env::var(MAX_TOKENS_ENV).ok().as_deref()));

/// Verbindung und Zuordnung einer Persona (leerer Handle oder unbekannte Persona: Eliezer, nicht freigegeben).
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

/// Satz, wenn eine externe Verbindung für die Persona nicht freigegeben ist.
pub fn not_allowed(connection: &ModelConnection, persona: &str) -> String {
    let who = if persona.trim().is_empty() { "diesen Auftrag ohne Persona".to_owned() } else { persona.to_owned() };
    format!(
        "Die Verbindung „{}“ ist extern; für {who} ist sie nicht freigegeben (Einstellungen → Assistenten). Es \
         wurde nichts gesendet.",
        connection.name
    )
}

/// Weg eines Auftrags der Persona `persona`: ihre Verbindung, aufgelöst. Ist die Verbindung extern und die
/// Persona nicht freigegeben, → `NotConfigured` mit [`not_allowed`], und es geht **keine** Anfrage hinaus.
pub async fn route(state: &AppState, persona: &str) -> Result<Target, LlmError> {
    let (connection, backend) = connection_of(state, persona).await?;
    if connection.external && !backend.external_allowed {
        return Err(LlmError::NotConfigured(not_allowed(&connection, persona)));
    }
    resolve(state, &connection).await
}

/// Meldung mit dem richtigen Konto (die Fehlertexte von `baruch_llm` nennen `baruch/eliezer`).
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

/// Ein Modellaufruf über das Ziel `target` durch dessen Warteschlange. Aliasse gelten nur bei Eliezer.
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

/// Ein Modellaufruf über die Verbindung Eliezer (ohne Persona, wie vor Runde 15); jedes Textstück geht an
/// `deltas`. Ist Eliezer als extern markiert, wird abgelehnt (keine Persona, keine Freigabe).
pub async fn chat(
    state: &AppState,
    request: ChatRequest,
    label: &str,
    deltas: tokio::sync::mpsc::UnboundedSender<String>,
) -> Result<ChatReply, LlmError> {
    let target = route(state, "").await?;
    chat_on(&target, request, label, deltas).await
}

/// Liste der Verbindung Eliezer in den Zwischenspeicher legen (aus „Verbindung prüfen“ der Verbindung
/// `eliezer`; darum Art Eliezer).
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

/// Zwischengespeicherte Listen und Kontextlängen der Adresse `url` als abgelaufen markieren (die Listen bleiben
/// als Rückfall). Für Tests und „Verbindung prüfen“; betrifft nur diese Adresse, dort aber jede Art.
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

/// Modellliste der Verbindung Eliezer.
pub async fn list(state: &AppState) -> Result<Listing, LlmError> {
    let connection = state
        .store
        .model_connection_get(ELIEZER_CONNECTION)
        .await
        .map_err(|e| LlmError::NotConfigured(format!("Modell-Verbindung nicht lesbar: {e}")))?;
    list_on(state, &connection).await
}

/// Modellliste einer Verbindung: höchstens alle 60 s neu abgefragt (nicht durch die Warteschlange; die Liste
/// lädt kein Modell). Antwortet die Gegenstelle nicht und gibt es eine abgelaufene Liste, kommt diese mit
/// `live: false` und Hinweis.
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

/// Satz bei abweichender Kontextlänge.
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

/// Modellname, wie er an die Verbindung geht (Alias nur bei Eliezer).
fn sent_model(kind: &str, model: &str) -> String {
    if kind == "eliezer" {
        alias(model).map_or_else(|| model.trim().to_owned(), str::to_owned)
    } else {
        model.trim().to_owned()
    }
}

/// Bekannte Kontextlänge ohne Netzabfrage (aus den Zwischenspeichern; abgelaufene Einträge zählen auch).
fn cached_context(kind: BackendKind, url: &str, model: &str) -> Option<Option<u32>> {
    if let Some((_, reported)) = lock(&CONTEXTS).get(&(kind, url.trim().to_owned(), model.to_owned())) {
        return Some(*reported);
    }
    let cache = lock(&CACHE);
    let hit = cache.get(&(kind, url.trim().to_owned()))?;
    hit.models.iter().find(|m| m.id == model).and_then(|m| m.context_length).map(Some)
}

/// Kontext ohne Netzabfrage (für `persona.list`): was die Zwischenspeicher wissen, sonst die Einstellung.
pub async fn context_cached(state: &AppState, persona: &PersonaRow, connection: &ModelConnection) -> ContextBudget {
    // OpenAI-kompatibel meldet keine Länge (Vertrag §3) – auch nicht über einen Zwischenspeicher derselben
    // Adresse, den eine andere Art (Eliezer) gefüllt hat (Integrator Runde 15).
    if connection.kind == "openai" {
        return budget(persona.context_length, &connection.name, None);
    }
    // Runde 23: je Art – eine unbekannte Art hat keinen Zwischenspeicher.
    let Some(kind) = BackendKind::parse(&connection.kind) else {
        return budget(persona.context_length, &connection.name, None);
    };
    let url = effective_url(state, connection).await;
    let model = sent_model(&connection.kind, &persona.model);
    budget(persona.context_length, &connection.name, cached_context(kind, &url, &model).flatten())
}

/// Kontext ohne Netzabfrage für eine Persona über ihre Verbindung (Runde 17a, für Seiten und Listen): wie
/// [`context_cached`]; ohne lesbare Verbindung die Einstellung.
pub async fn context_cached_for(state: &AppState, persona: &PersonaRow) -> ContextBudget {
    match connection_of(state, &persona.handle).await {
        Ok((connection, _)) => context_cached(state, persona, &connection).await,
        Err(_) => budget(persona.context_length, "", None),
    }
}

/// Modellliste einer Verbindung ohne Netzabfrage (Runde 17a): der Zwischenspeicher, auch abgelaufen; `None`, wenn
/// noch nie abgefragt.
pub async fn cached_list_on(state: &AppState, connection: &ModelConnection) -> Option<Listing> {
    let kind = BackendKind::parse(&connection.kind)?;
    let url = effective_url(state, connection).await;
    lock(&CACHE).get(&(kind, url.trim().to_owned())).map(|hit| listing_of(hit, false, None))
}

/// Kontext für Aufträge an `persona`: Einstellung abgeglichen mit der gemeldeten Länge der Gegenstelle.
/// Wartet höchstens so lange wie die Modellliste (Zwischenspeicher 60 s); scheitert nie (ohne Meldung gilt
/// die Einstellung).
///
/// Runde 23: Die Freigabe wird **vor** jeder Anfrage geprüft wie in [`route`]. Ist die Verbindung extern und die
/// Persona nicht freigegeben, geht nichts hinaus (auch kein `/api/show` bzw. `/api/v1/models` – beide trügen
/// Schlüssel und Modellnamen) und es gilt die Einstellung; der Auftrag selbst wird danach ohnehin abgelehnt.
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

/// Modell einer Persona gegen die Liste: `{handle, model, active, state: "bekannt" | "alias" |
/// "unbekannt", target?}`. „alias“: der alte Name hat einen Alias, den Eliezer kennt (`target`);
/// kennt Eliezer auch den Alias nicht, ist die Persona „unbekannt“ (mit `target` als Hinweis).
pub fn persona_states(personas: &[PersonaRow], models: &[String]) -> Vec<Value> {
    states_of(personas.iter().map(|p| (p.handle.as_str(), p.model.as_str(), p.active)), models)
}

/// Wie [`persona_states`] für `(handle, model, active)`.
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

/// Satz zum Abgleich (für „Verbindung prüfen“): unbekannte Modelle und Aliasse; leer, wenn alle bekannt.
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

/// Aliasse als Objekt `{alt: neu}`.
fn aliases() -> Value {
    Value::Object(baruch_llm::ALIASES.iter().map(|(old, new)| ((*old).to_owned(), json!(new))).collect::<Map<_, _>>())
}

/// Kurztest auf der Verbindung Eliezer ([`probe_on`]).
pub async fn probe(state: &AppState, model: &str) -> Value {
    probe_on(state, ELIEZER_CONNECTION, model).await
}

/// Kurztest: fester Auftrag [`PROBE_PROMPT`], `max_tokens` 16, durch die Warteschlange der Verbindung. Ein
/// Fehlschlag ist ein Ergebnis (`ok: false` mit Meldung), kein Fehler. Das Ergebnis steht danach in der
/// Datenbank (überlebt einen Neustart). Kein Buchtext: geht auch über eine externe Verbindung ohne Freigabe.
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

/// Letzter Kurztest der Verbindung Eliezer (aus der Datenbank).
pub async fn last_probe(state: &AppState) -> Option<Value> {
    state.store.model_connection_get(ELIEZER_CONNECTION).await.ok().and_then(|c| c.probe)
}

/// Bedient dieses Modul die RPC-Methode?
pub fn handles(method: &str) -> bool {
    METHODS.contains(&method)
}

fn failure(e: LlmError) -> RpcError {
    RpcError::new(REFUSED, e.to_string())
}

/// Zustand des Schlüssels einer Verbindung: `hinterlegt`, `fehlt` oder `null` (Schlüsselbund nicht lesbar).
async fn secret_state(state: &AppState, connection: &ModelConnection) -> Value {
    match crate::connections::secrets::get(&state.secrets, &connection.account).await {
        Ok(Some(_)) => json!("hinterlegt"),
        Ok(None) => json!("fehlt"),
        Err(_) => Value::Null,
    }
}

/// Eine Verbindung als JSON (nie mit Schlüssel): Art, Adresse, lokal/extern, Schlüssel hinterlegt/fehlt,
/// letzter Kurztest und letzte Prüfung, Personas, die sie nutzen.
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

/// Verbindung aus `connection` (id) bzw. der Persona `persona`; ohne beides Eliezer.
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

/// Felder von `model.connection_set` prüfen (Name, Art, Adresse über `connections::config::check_url`).
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
    // Ohne Angabe: beim Anlegen nach der Adresse (nicht erkennbar lokal → extern); beim Ändern bleibt „extern“,
    // und eine neue, nicht erkennbar lokale Adresse macht die Verbindung extern – ein vergessener Haken öffnet die
    // Sperre nie. Wer einen Dienst im eigenen Netz unter einem gewöhnlichen Namen betreibt, setzt `false` ausdrücklich.
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

/// `model.connection_set`: anlegen (ohne `id`) oder ändern. Bei Eliezer geht die Adresse an die Verbindung
/// `eliezer` (mit deren Bindung des Schlüssels); bei anderen bindet ein neues Ziel den Schlüssel an das alte.
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

/// Schlüssel einer Verbindung hinterlegen; gilt danach für ihr jetziges Ziel. Eliezer: Konto `eliezer` mit der
/// Bindung der Verbindung `eliezer`.
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

/// Schlüssel einer Verbindung entfernen.
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

/// Verbindung löschen (nicht Eliezer); ihr Schlüssel wird mit entfernt, ihre Personas fallen auf Eliezer zurück.
pub async fn delete_connection(state: &AppState, id: i64) -> Result<Vec<String>, RpcError> {
    let connection = state.store.model_connection_get(id).await?;
    let handles = state.store.model_connection_delete(id).await?;
    let _ = crate::connections::secrets::delete(&state.secrets, &connection.account).await;
    Ok(handles)
}

/// Verbindung prüfen: Modellliste live abfragen (mit Kontextlängen, soweit gemeldet), Ergebnis speichern.
/// Eliezer: wie „Verbindung prüfen“ der Verbindung `eliezer` (gleicht die Modelle der Personas ab).
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

/// Runde 23: Löschen (Verbindung, Schlüssel) nur mit `confirm: true` – wie `work.delete` und die Formulare mit
/// ihrem Pflicht-Kästchen. Sonst -32602, und nichts ist gelöscht.
fn confirmed(params: &Value) -> Result<(), RpcError> {
    if params.get("confirm").and_then(Value::as_bool) == Some(true) {
        Ok(())
    } else {
        Err(RpcError::new(INVALID_PARAMS, "confirm: true erwartet"))
    }
}

/// RPC `model.*` (`docs/RPC.md`, „Modelle“). Fehler des Backends bei `model.list` ohne Zwischenspeicher →
/// -32000 mit deutschem Satz.
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
        _ => Err(RpcError::new(METHOD_NOT_FOUND, format!("unbekannte Methode {method}"))),
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
        // Keine PersonaRow-Literale: die Struktur wächst (Runde 10, personas).
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
