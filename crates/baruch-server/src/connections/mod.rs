// SPDX-License-Identifier: AGPL-3.0-or-later
//! Connections and sources (Round 9, task “connections”): the settings pages „Verbindungen“
//! and „Quellen“ as render functions for the frame of the settings ([`render_connections`],
//! [`render_sources`]), their forms (`POST /einstellungen/verbindungen/act`,
//! `POST /einstellungen/quellen/act`, behind `guard::same_origin`), the RPC methods
//! `connection.*` and `sources.*` ([`handles`], [`call`]) and the reading MCP tool `sources_list`
//! ([`mcp`]).
//!
//! Connections: Eliezer (model backend, key `baruch/eliezer`), MultiGPT MCP (token
//! `baruch/multigpt`), pgvector (password `baruch/pgvector`), LanceDB local (no secret); per work
//! the RAG ways. Settings live in SQLite (migration 0013), secrets only in the keyring
//! ([`secrets::SecretStore`]); HTML, RPC and messages name only „hinterlegt“ or „fehlt“.
//! „Verbindung prüfen“ ([`check`]) stores the result in the table `connection`.
//!
//! Everything here is reserved for the human: no path through `POST /act` and none through MCP (except the
//! reading `sources_list`, without secrets). Since Round 10 the personas use Eliezer for their
//! model calls ([`crate::models`]; „Verbindung prüfen“ reconciles the models of the personas there,
//! „Kurztest“ = `model.probe`). Since Round 12 the semantic search (RAG) uses pgvector directly
//! ([`pgvector`]), MultiGPT and LanceDB; per work the human chooses the collections of pgvector and
//! MultiGPT for it (`connection.collections_*`, [`collections`]). The web search of `@belege` queries the sources
//! first (Round 13, [`crate::websearch`], usage text `sources::USAGE`).

pub mod check;
mod collections;
pub mod config;
pub mod mcp;
pub mod multigpt;
mod pages;
pub mod pgvector;
pub mod secrets;
mod sources;

use std::sync::Arc;

use axum::Router;
use axum::routing::post;
use baruch_store::{ConnectionKind, ConnectionRow, RagWay, StoreError, WorkId};
use serde_json::{Value, json};

pub use pages::{render_connections, render_sources};
pub use secrets::{KeyringSecrets, MemorySecrets, SecretStore, SecretsMode, secrets_mode, system_secrets};

use crate::AppState;
use crate::rpc::{INTERNAL, INVALID_PARAMS, METHOD_NOT_FOUND, RpcError};

const METHODS: &[&str] = &[
    "connection.list",
    "connection.get",
    "connection.set",
    "connection.set_secret",
    "connection.delete_secret",
    "connection.check",
    "connection.rag_get",
    "connection.rag_set",
    "connection.collections_get",
    "connection.collections_set",
    "sources.list",
    "sources.create",
    "sources.update",
    "sources.move",
    "sources.delete",
];

/// Does this module serve the RPC method?
pub fn handles(method: &str) -> bool {
    METHODS.contains(&method)
}

/// Why an action fails: invalid input (with a fixed code for the address of the form),
/// keyring or internal. No text contains a secret.
#[derive(Debug)]
pub enum Fault {
    Invalid(&'static str, String),
    Keyring(String),
    Internal(String),
}

impl Fault {
    /// Fixed code for `?fehler=` (see `pages::error_text`).
    pub fn code(&self) -> &'static str {
        match self {
            Fault::Invalid(code, _) => code,
            Fault::Keyring(_) => "schluesselbund",
            Fault::Internal(_) => "intern",
        }
    }
}

impl From<Fault> for RpcError {
    fn from(fault: Fault) -> Self {
        match fault {
            Fault::Invalid(_, message) => RpcError::new(INVALID_PARAMS, message),
            Fault::Keyring(message) => RpcError::new(INTERNAL, format!("Schlüsselbund: {message}")),
            Fault::Internal(message) => RpcError::new(INTERNAL, message),
        }
    }
}

impl From<StoreError> for Fault {
    fn from(e: StoreError) -> Self {
        match e {
            StoreError::UnknownWork(_) => Fault::Invalid("werk", e.to_string()),
            StoreError::Refused(message) => Fault::Invalid("feld", message.to_owned()),
            other => {
                let rpc = RpcError::from(other);
                if rpc.code == INTERNAL { Fault::Internal(rpc.message) } else { Fault::Invalid("feld", rpc.message) }
            }
        }
    }
}

/// Kind from a text; unknown → `Invalid("kind")`.
pub fn parse_kind(kind: &str) -> Result<ConnectionKind, Fault> {
    ConnectionKind::ALL
        .into_iter()
        .find(|k| k.as_str() == kind.trim())
        .ok_or_else(|| Fault::Invalid("kind", "kind: eliezer, multigpt, pgvector oder lancedb.".into()))
}

/// Display name.
pub fn title(kind: ConnectionKind) -> &'static str {
    match kind {
        ConnectionKind::Eliezer => "Eliezer (Modell-Backend)",
        ConnectionKind::Multigpt => "MultiGPT (MCP)",
        ConnectionKind::Pgvector => "pgvector (PostgreSQL)",
        ConnectionKind::Lancedb => "LanceDB (lokal)",
    }
}

/// Account in the keyring (service “baruch”); LanceDB has none.
pub fn account(kind: ConnectionKind) -> Option<&'static str> {
    match kind {
        ConnectionKind::Eliezer => Some("eliezer"),
        ConnectionKind::Multigpt => Some("multigpt"),
        ConnectionKind::Pgvector => Some("pgvector"),
        ConnectionKind::Lancedb => None,
    }
}

/// What Baruch uses the connection for or what does not work yet (honest on the page; whether it is a STUB
/// is told by [`usage_is_stub`]).
pub fn usage_stub(kind: ConnectionKind) -> &'static str {
    match kind {
        ConnectionKind::Eliezer => {
            "Personas nutzen Eliezer für ihre Antworten, Prüfungen und Vorschläge – immer nur eine Anfrage zur Zeit; das Modell je Persona steht unter Einstellungen → Assistenten. Dazu bettet Eliezer die Texte für die Bedeutungssuche ein (LanceDB und pgvector, eigene Warteschlange, höchstens 32 Texte je Anfrage); ohne Eliezer bleibt die Wortlaut-Suche."
        }
        ConnectionKind::Multigpt => {
            "Suche über MultiGPT (search_documents) in den gewählten Sammlungen des Werks, Treffer mit Quellenangabe; read_document als Ausschnitt einer Fundstelle und als Volltext in der Dokumentansicht der Literatur (teilweise, ohne JavaScript); Hervorhebungen ohne Zitat werden gegen den Volltext geprüft."
        }
        ConnectionKind::Pgvector => {
            "Bedeutungssuche direkt in pgvector (Kosinus-Abstand, nur lesend) in den gewählten Sammlungen des Werks; die Anfrage wird mit demselben Einbettungsmodell eingebettet wie die Daten."
        }
        ConnectionKind::Lancedb => {
            "Haupt-RAG im Datenordner: Abschnitte (aktuelle Fassung), Einfälle, Hervorhebungen und die Autorenbibliothek, eingebettet über Eliezer; ohne Einbettungsmodell bleibt die Wortlaut-Suche (FTS)."
        }
    }
}

/// Is [`usage_stub`] still a STUB for this kind (use not implemented)? Since Round 12 for none.
pub fn usage_is_stub(_kind: ConnectionKind) -> bool {
    false
}

/// What is still open on a connection (honest note on the page, `data-stub`), otherwise `None`.
/// Since Round 13 nothing is open on any connection: the full text through `read_document` is shown by the
/// literature, the reconciliation Baruch ↔ MultiGPT by the page „Abgleichen“.
pub fn open_part(_kind: ConnectionKind) -> Option<&'static str> {
    None
}

/// State of the secret: `"hinterlegt"`, `"fehlt"` or `null` (LanceDB, or keyring not
/// readable, then with error text).
async fn secret_state(state: &AppState, kind: ConnectionKind) -> (Value, Option<String>) {
    let Some(account) = account(kind) else { return (Value::Null, None) };
    match secrets::get(&state.secrets, account).await {
        Ok(Some(_)) => (json!("hinterlegt"), None),
        Ok(None) => (json!("fehlt"), None),
        Err(e) => (Value::Null, Some(format!("Schlüsselbund: {e}"))),
    }
}

/// A connection as JSON for RPC and page (never with secret).
pub(crate) async fn describe(state: &AppState, row: &ConnectionRow) -> Value {
    let (secret, secret_error) = secret_state(state, row.kind).await;
    let mut out = json!({
        "kind": row.kind, "title": title(row.kind), "config": row.config, "updated": row.updated,
        "secret": secret, "check": row.check,
    });
    if secret == json!("hinterlegt") && !secret_bound(row) {
        // Target for which the secret is valid (address details only, never the secret).
        out["secret_for"] = json!(row.secret_target);
    }
    if let Some(error) = secret_error {
        out["secret_error"] = json!(error);
    }
    out
}

/// Check settings, lay them over the stored ones, store.
///
/// **Binding of the secret:** if the target changes ([`config::target`]) and a secret is
/// stored (or the keyring unreadable), the connection first remembers the previous target
/// (`secret_target`, unless it is already bound to a target). [`run_check`] then does not send the secret
/// to the new target; only a newly entered secret ([`set_secret`]) is valid for it. So
/// `connection.set` with a foreign address plus `connection.check` extracts no key.
pub(crate) async fn set_config(state: &AppState, kind: ConnectionKind, given: &Value) -> Result<ConnectionRow, Fault> {
    let _held = state.write_lock.lock().await;
    let current = state.store.connection_get(kind).await?;
    // Never stored: `current.config` is the default. It is added through `merged` anyway – except the
    // mapping of pgvector (Round 12), which is stored only when the human enters it.
    let stored = if current.updated.is_some() { current.config.clone() } else { json!({}) };
    let config = config::merged(kind, &stored, given)?;
    let (before, after) = (config::target(kind, &current.config), config::target(kind, &config));
    if before != after && current.secret_target.is_none() {
        let held_secret = match account(kind) {
            Some(account) => !matches!(secrets::get(&state.secrets, account).await, Ok(None)),
            None => false,
        };
        if held_secret {
            state.store.connection_bind_secret(kind, before.as_deref()).await?;
        }
    }
    let mut row = state.store.connection_set(kind, &config).await?;
    if row.secret_target.is_some() && row.secret_target == after {
        // Back at the bound target: the secret is valid again for the present one.
        state.store.connection_bind_secret(kind, None).await?;
        row.secret_target = None;
    }
    Ok(row)
}

/// Check a secret (trimmed at the edges, not empty, at most 4096 characters in one line) without
/// changing anything. The form „Speichern“ checks this way beforehand so that an invalid secret does not
/// show up only after the settings are stored.
pub(crate) fn valid_secret(kind: ConnectionKind, secret: &str) -> Result<&str, Fault> {
    if account(kind).is_none() {
        return Err(Fault::Invalid("kein_geheimnis", "LanceDB braucht kein Geheimnis.".into()));
    }
    let secret = secret.trim();
    if secret.is_empty() {
        return Err(Fault::Invalid("geheimnis_leer", "secret: leer; nichts geändert.".into()));
    }
    if secret.chars().count() > 4096 || secret.chars().any(char::is_control) {
        return Err(Fault::Invalid("geheimnis_ungueltig", "secret: höchstens 4096 Zeichen in einer Zeile.".into()));
    }
    Ok(secret)
}

/// Store a secret ([`valid_secret`]); afterwards it is valid for the present target of the connection.
pub(crate) async fn set_secret(state: &AppState, kind: ConnectionKind, secret: &str) -> Result<(), Fault> {
    let secret = valid_secret(kind, secret)?;
    let Some(account) = account(kind) else { unreachable!("valid_secret lässt LanceDB nicht durch") };
    let _held = state.write_lock.lock().await;
    // First the keyring, then the binding: if something fails in between, the new secret stays bound to
    // the old target (so is not sent) instead of the other way round.
    secrets::set(&state.secrets, account, secret).await.map_err(Fault::Keyring)?;
    Ok(state.store.connection_bind_secret(kind, None).await?)
}

/// Remove a secret (the caller checks the confirmation); the binding goes with it.
pub(crate) async fn delete_secret(state: &AppState, kind: ConnectionKind) -> Result<(), Fault> {
    let Some(account) = account(kind) else {
        return Err(Fault::Invalid("kein_geheimnis", "LanceDB braucht kein Geheimnis.".into()));
    };
    let _held = state.write_lock.lock().await;
    secrets::delete(&state.secrets, account).await.map_err(Fault::Keyring)?;
    Ok(state.store.connection_bind_secret(kind, None).await?)
}

/// Is the stored secret valid for the present target? (`false` if the connection is bound to another
/// target; then it goes nowhere.)
pub(crate) fn secret_bound(row: &ConnectionRow) -> bool {
    match &row.secret_target {
        None => true,
        Some(bound) => Some(bound) == config::target(row.kind, &row.config).as_ref(),
    }
}

/// Check a connection and store the result. A failure of the remote side is `ok: false`;
/// an unreadable keyring is an error. LanceDB is checked by [`crate::rag::check_lancedb`] (folder, index,
/// embedding model).
pub(crate) async fn run_check(state: &AppState, kind: ConnectionKind) -> Result<Value, Fault> {
    if kind == ConnectionKind::Lancedb {
        return crate::rag::check_lancedb(state).await;
    }
    let row = state.store.connection_get(kind).await?;
    let secret = match account(kind) {
        Some(account) => secrets::get(&state.secrets, account).await.map_err(Fault::Keyring)?,
        None => None,
    };
    if secret.is_some() && !secret_bound(&row) {
        // The secret belongs to another target: do not send, do not connect, report honestly.
        let bound = row.secret_target.clone().unwrap_or_default();
        let message = format!(
            "Nicht geprüft: Das Geheimnis ist für {bound} hinterlegt, die Adresse zeigt jetzt anderswohin. \
             Baruch schickt es nicht an ein anderes Ziel. Für die neue Adresse das Geheimnis neu eintragen \
             (oder die alte Adresse wieder einstellen)."
        );
        let details = json!({ "secret_withheld": true, "secret_target": bound });
        let saved = state.store.connection_record_check(kind, false, &message, &details).await?;
        let at = saved.check.map(|c| c.at);
        return Ok(json!({ "kind": kind, "ok": false, "message": message, "details": details, "at": at }));
    }
    let url = row.config.get("url").and_then(Value::as_str).unwrap_or("");
    // One shared time limit for the whole check (MultiGPT sends three requests; each has
    // its own limit, but together otherwise up to 30 s).
    let attempt = async {
        match kind {
            ConnectionKind::Eliezer => check::eliezer(url, secret.as_deref()).await,
            ConnectionKind::Multigpt => check::multigpt(url, secret.as_deref()).await,
            ConnectionKind::Pgvector => pgvector::check(&row.config, secret.as_deref()).await,
            // LanceDB returns above (crate::rag::check_lancedb).
            ConnectionKind::Lancedb => unreachable!("LanceDB wird oben behandelt"),
        }
    };
    let mut outcome = tokio::time::timeout(check::TIMEOUT, attempt).await.unwrap_or_else(|_| check::Outcome {
        ok: false,
        message: format!(
            "Keine vollständige Antwort innerhalb von {} s; Prüfung abgebrochen.",
            check::TIMEOUT.as_secs()
        ),
        details: json!({}),
    });
    if kind == ConnectionKind::Eliezer && outcome.ok {
        // Reconcile the models of the personas with the list (unknown, alias); afterwards the list also
        // serves `model.list` (cache).
        let models: Vec<String> = outcome
            .details
            .get("models")
            .and_then(Value::as_array)
            .map(|list| list.iter().filter_map(Value::as_str).map(str::to_owned).collect())
            .unwrap_or_default();
        crate::models::remember(url, &models);
        let personas = state.store.persona_list().await?;
        let states = crate::models::persona_states(&personas, &models);
        let summary = crate::models::persona_summary(&states);
        if !summary.is_empty() {
            outcome.message = format!("{} {summary}", outcome.message);
        }
        outcome.details["personas"] = json!(states);
    }
    let outcome = check::scrub(outcome, secret.as_deref());
    let saved = state.store.connection_record_check(kind, outcome.ok, &outcome.message, &outcome.details).await?;
    let at = saved.check.map(|c| c.at);
    Ok(json!({ "kind": kind, "ok": outcome.ok, "message": outcome.message, "details": outcome.details, "at": at }))
}

/// RAG ways of a work.
pub(crate) async fn rag_get(state: &AppState, work: WorkId) -> Result<Value, Fault> {
    let (ways, default) = state.store.work_rag(work).await?;
    Ok(json!({ "work": work, "ways": ways, "default": default }))
}

/// Set the RAG ways of a work.
pub(crate) async fn rag_set(state: &AppState, work: WorkId, ways: &[RagWay]) -> Result<Value, Fault> {
    let ways = state.store.set_work_rag(work, ways).await?;
    Ok(json!({ "work": work, "ways": ways, "default": false }))
}

/// Work from `work` (number); if missing, the default work.
async fn work_param(state: &AppState, params: &Value) -> Result<WorkId, Fault> {
    match params.get("work") {
        None | Some(Value::Null) => Ok(state.default_work().await?),
        Some(work) => {
            let work = work.as_i64().ok_or_else(|| Fault::Invalid("werk", "work: Werk-id (Zahl) erwartet.".into()))?;
            state.store.work_slug(work).await?;
            Ok(work)
        }
    }
}

fn kind_param(params: &Value) -> Result<ConnectionKind, Fault> {
    match params.get("kind").and_then(Value::as_str) {
        Some(kind) => parse_kind(kind),
        None => Err(Fault::Invalid("kind", "kind fehlt (eliezer, multigpt, pgvector oder lancedb).".into())),
    }
}

fn confirmed(params: &Value) -> Result<(), Fault> {
    if params.get("confirm") == Some(&Value::Bool(true)) {
        Ok(())
    } else {
        Err(Fault::Invalid("bestaetigen", "confirm: true fehlt; nichts geändert.".into()))
    }
}

/// Ways from a list of texts; unknown → `Invalid("weg")`.
fn ways_param(params: &Value) -> Result<Vec<RagWay>, Fault> {
    let invalid = || Fault::Invalid("weg", "ways: Liste aus lancedb, pgvector, multigpt.".into());
    let list = params.get("ways").and_then(Value::as_array).ok_or_else(invalid)?;
    list.iter()
        .map(|w| w.as_str().and_then(|w| RagWay::ALL.into_iter().find(|r| r.as_str() == w)).ok_or_else(invalid))
        .collect()
}

/// RPC `connection.*` and `sources.*` (from `rpc::call` before the work is chosen).
pub async fn call(state: &AppState, method: &str, params: Value) -> Result<Value, RpcError> {
    let params = if params.is_null() { json!({}) } else { params };
    if !params.is_object() {
        return Err(RpcError::new(INVALID_PARAMS, "Parameter als Objekt erwartet"));
    }
    if let Some(answer) = sources::call(state, method, &params).await {
        return answer.map_err(RpcError::from);
    }
    if let Some(answer) = collections::call(state, method, &params).await {
        return answer.map_err(RpcError::from);
    }
    let answer: Result<Value, Fault> = async {
        match method {
            "connection.list" => {
                let mut list = Vec::new();
                for row in state.store.connection_list().await? {
                    list.push(describe(state, &row).await);
                }
                Ok(json!({ "connections": list }))
            }
            "connection.get" => {
                let row = state.store.connection_get(kind_param(&params)?).await?;
                Ok(describe(state, &row).await)
            }
            "connection.set" => {
                let kind = kind_param(&params)?;
                let config = params.get("config").cloned().unwrap_or(Value::Null);
                let row = set_config(state, kind, &config).await?;
                Ok(describe(state, &row).await)
            }
            "connection.set_secret" => {
                let kind = kind_param(&params)?;
                let secret = params
                    .get("secret")
                    .and_then(Value::as_str)
                    .ok_or_else(|| Fault::Invalid("geheimnis_leer", "secret: Text erwartet.".into()))?;
                set_secret(state, kind, secret).await?;
                Ok(json!({ "kind": kind, "secret": "hinterlegt" }))
            }
            "connection.delete_secret" => {
                let kind = kind_param(&params)?;
                confirmed(&params)?;
                delete_secret(state, kind).await?;
                Ok(json!({ "kind": kind, "secret": "fehlt" }))
            }
            "connection.check" => run_check(state, kind_param(&params)?).await,
            "connection.rag_get" => rag_get(state, work_param(state, &params).await?).await,
            "connection.rag_set" => {
                let work = work_param(state, &params).await?;
                rag_set(state, work, &ways_param(&params)?).await
            }
            _ => Err(Fault::Internal(format!("unknown method {method}"))),
        }
    }
    .await;
    match answer {
        Err(Fault::Internal(message)) if message.starts_with("unknown method") => {
            Err(RpcError::new(METHOD_NOT_FOUND, message))
        }
        other => other.map_err(RpcError::from),
    }
}

/// `POST /einstellungen/verbindungen/act`, `POST /einstellungen/quellen/act` and (Round 15b)
/// `POST /einstellungen/modelle/act` for the model connections (in the router of the writing routes, behind
/// `guard::same_origin`). The pages themselves are shown by the frame of the settings
/// (`GET /einstellungen/verbindungen`, `…/quellen`).
pub fn writes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/einstellungen/verbindungen/act", post(pages::connections_act))
        .route("/einstellungen/quellen/act", post(pages::sources_act))
        .route("/einstellungen/modelle/act", post(pages::models_act))
        // Round 15: commentaries and lexicons per work (page Einstellungen → Werk).
        .route("/einstellungen/werk/bibeln", post(pages::work_bibles_act))
}
