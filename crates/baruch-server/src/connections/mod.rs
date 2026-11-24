// SPDX-License-Identifier: AGPL-3.0-or-later
//! Verbindungen und Quellen (Runde 9, Aufgabe „connections“): die Einstellungsseiten „Verbindungen“
//! und „Quellen“ als Render-Funktionen für den Rahmen der Einstellungen ([`render_connections`],
//! [`render_sources`]), ihre Formulare (`POST /einstellungen/verbindungen/act`,
//! `POST /einstellungen/quellen/act`, hinter `guard::same_origin`), die RPC-Methoden
//! `connection.*` und `sources.*` ([`handles`], [`call`]) und das lesende MCP-Werkzeug `sources_list`
//! ([`mcp`]).
//!
//! Verbindungen: Eliezer (Modell-Backend, Schlüssel `baruch/eliezer`), MultiGPT-MCP (Token
//! `baruch/multigpt`), pgvector (Passwort `baruch/pgvector`), LanceDB lokal (ohne Geheimnis); je Werk
//! die RAG-Wege. Einstellungen liegen in SQLite (Migration 0013), Geheimnisse nur im Schlüsselbund
//! ([`secrets::SecretStore`]); HTML, RPC und Meldungen nennen nur „hinterlegt“ bzw. „fehlt“.
//! „Verbindung prüfen“ ([`check`]) speichert das Ergebnis in der Tabelle `connection`.
//!
//! Alles hier ist dem Menschen vorbehalten: kein Weg über `POST /act` und keiner über MCP (außer dem
//! lesenden `sources_list`, ohne Geheimnisse). Eliezer nutzen die Personas seit Runde 10 für ihre
//! Modellaufrufe ([`crate::models`]; „Verbindung prüfen“ gleicht dabei die Modelle der Personas ab,
//! „Kurztest“ = `model.probe`). Seit Runde 12 nutzt die Bedeutungssuche (RAG) pgvector direkt
//! ([`pgvector`]), MultiGPT und LanceDB; je Werk wählt der Mensch dafür die Sammlungen von pgvector und
//! MultiGPT (`connection.collections_*`, [`collections`]). Die Websuche von `@belege` fragt die Quellen
//! zuerst ab (Runde 13, [`crate::websearch`], Nutzungstext `sources::USAGE`).

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

/// Bedient dieses Modul die RPC-Methode?
pub fn handles(method: &str) -> bool {
    METHODS.contains(&method)
}

/// Warum eine Aktion scheitert: ungültige Eingabe (mit festem Code für die Adresse des Formulars),
/// Schlüsselbund oder intern. Kein Text enthält ein Geheimnis.
#[derive(Debug)]
pub enum Fault {
    Invalid(&'static str, String),
    Keyring(String),
    Internal(String),
}

impl Fault {
    /// Fester Code für `?fehler=` (siehe `pages::error_text`).
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

/// Art aus einem Text; unbekannt → `Invalid("kind")`.
pub fn parse_kind(kind: &str) -> Result<ConnectionKind, Fault> {
    ConnectionKind::ALL
        .into_iter()
        .find(|k| k.as_str() == kind.trim())
        .ok_or_else(|| Fault::Invalid("kind", "kind: eliezer, multigpt, pgvector oder lancedb.".into()))
}

/// Anzeigename.
pub fn title(kind: ConnectionKind) -> &'static str {
    match kind {
        ConnectionKind::Eliezer => "Eliezer (Modell-Backend)",
        ConnectionKind::Multigpt => "MultiGPT (MCP)",
        ConnectionKind::Pgvector => "pgvector (PostgreSQL)",
        ConnectionKind::Lancedb => "LanceDB (lokal)",
    }
}

/// Konto im Schlüsselbund (Dienst „baruch“); LanceDB hat keins.
pub fn account(kind: ConnectionKind) -> Option<&'static str> {
    match kind {
        ConnectionKind::Eliezer => Some("eliezer"),
        ConnectionKind::Multigpt => Some("multigpt"),
        ConnectionKind::Pgvector => Some("pgvector"),
        ConnectionKind::Lancedb => None,
    }
}

/// Wozu Baruch die Verbindung nutzt bzw. was noch nicht geht (ehrlich auf der Seite; ob es ein STUB
/// ist, sagt [`usage_is_stub`]).
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

/// Ist [`usage_stub`] für diese Art noch ein STUB (Nutzung nicht umgesetzt)? Seit Runde 12 für keine.
pub fn usage_is_stub(_kind: ConnectionKind) -> bool {
    false
}

/// Was an einer Verbindung noch offen ist (ehrlicher Hinweis auf der Seite, `data-stub`), sonst `None`.
/// Seit Runde 13 ist bei keiner Verbindung etwas offen: Den Volltext über `read_document` zeigt die
/// Literatur, den Abgleich Baruch ↔ MultiGPT die Seite „Abgleichen“.
pub fn open_part(_kind: ConnectionKind) -> Option<&'static str> {
    None
}

/// Zustand des Geheimnisses: `"hinterlegt"`, `"fehlt"` oder `null` (LanceDB bzw. Schlüsselbund nicht
/// lesbar, dann mit Fehlertext).
async fn secret_state(state: &AppState, kind: ConnectionKind) -> (Value, Option<String>) {
    let Some(account) = account(kind) else { return (Value::Null, None) };
    match secrets::get(&state.secrets, account).await {
        Ok(Some(_)) => (json!("hinterlegt"), None),
        Ok(None) => (json!("fehlt"), None),
        Err(e) => (Value::Null, Some(format!("Schlüsselbund: {e}"))),
    }
}

/// Eine Verbindung als JSON für RPC und Seite (nie mit Geheimnis).
pub(crate) async fn describe(state: &AppState, row: &ConnectionRow) -> Value {
    let (secret, secret_error) = secret_state(state, row.kind).await;
    let mut out = json!({
        "kind": row.kind, "title": title(row.kind), "config": row.config, "updated": row.updated,
        "secret": secret, "check": row.check,
    });
    if secret == json!("hinterlegt") && !secret_bound(row) {
        // Ziel, für das das Geheimnis gilt (nur Adressangaben, nie das Geheimnis).
        out["secret_for"] = json!(row.secret_target);
    }
    if let Some(error) = secret_error {
        out["secret_error"] = json!(error);
    }
    out
}

/// Einstellungen prüfen, über die gespeicherten legen, speichern.
///
/// **Bindung des Geheimnisses:** Ändert sich das Ziel ([`config::target`]) und ist ein Geheimnis
/// hinterlegt (oder der Schlüsselbund unlesbar), merkt sich die Verbindung vorher das bisherige Ziel
/// (`secret_target`, sofern sie nicht schon an ein Ziel gebunden ist). [`run_check`] schickt das Geheimnis
/// dann nicht an das neue Ziel; erst ein neu eingetragenes Geheimnis ([`set_secret`]) gilt für es. So
/// zieht `connection.set` mit fremder Adresse plus `connection.check` keinen Schlüssel ab.
pub(crate) async fn set_config(state: &AppState, kind: ConnectionKind, given: &Value) -> Result<ConnectionRow, Fault> {
    let _held = state.write_lock.lock().await;
    let current = state.store.connection_get(kind).await?;
    // Nie gespeichert: `current.config` ist die Vorgabe. Sie kommt über `merged` ohnehin dazu – außer der
    // Zuordnung von pgvector (Runde 12), die erst gespeichert wird, wenn der Mensch sie einträgt.
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
        // Zurück beim gebundenen Ziel: Das Geheimnis gilt wieder für das jetzige.
        state.store.connection_bind_secret(kind, None).await?;
        row.secret_target = None;
    }
    Ok(row)
}

/// Geheimnis prüfen (am Rand beschnitten, nicht leer, höchstens 4096 Zeichen in einer Zeile), ohne
/// etwas zu ändern. Das Formular „Speichern“ prüft so vorher, damit ein ungültiges Geheimnis nicht
/// erst nach dem Speichern der Einstellungen auffällt.
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

/// Geheimnis hinterlegen ([`valid_secret`]); es gilt danach für das jetzige Ziel der Verbindung.
pub(crate) async fn set_secret(state: &AppState, kind: ConnectionKind, secret: &str) -> Result<(), Fault> {
    let secret = valid_secret(kind, secret)?;
    let Some(account) = account(kind) else { unreachable!("valid_secret lässt LanceDB nicht durch") };
    let _held = state.write_lock.lock().await;
    // Erst der Schlüsselbund, dann die Bindung: Scheitert dazwischen etwas, bleibt das neue Geheimnis an
    // das alte Ziel gebunden (wird also nicht gesendet) statt umgekehrt.
    secrets::set(&state.secrets, account, secret).await.map_err(Fault::Keyring)?;
    Ok(state.store.connection_bind_secret(kind, None).await?)
}

/// Geheimnis entfernen (die Bestätigung prüft der Aufrufer); die Bindung entfällt mit ihm.
pub(crate) async fn delete_secret(state: &AppState, kind: ConnectionKind) -> Result<(), Fault> {
    let Some(account) = account(kind) else {
        return Err(Fault::Invalid("kein_geheimnis", "LanceDB braucht kein Geheimnis.".into()));
    };
    let _held = state.write_lock.lock().await;
    secrets::delete(&state.secrets, account).await.map_err(Fault::Keyring)?;
    Ok(state.store.connection_bind_secret(kind, None).await?)
}

/// Gilt das hinterlegte Geheimnis für das jetzige Ziel? (`false`, wenn die Verbindung an ein anderes
/// Ziel gebunden ist; dann geht es nirgendwohin.)
pub(crate) fn secret_bound(row: &ConnectionRow) -> bool {
    match &row.secret_target {
        None => true,
        Some(bound) => Some(bound) == config::target(row.kind, &row.config).as_ref(),
    }
}

/// Verbindung prüfen und das Ergebnis speichern. Ein Fehlschlag der Gegenstelle ist `ok: false`;
/// ein unlesbarer Schlüsselbund ist ein Fehler. LanceDB prüft [`crate::rag::check_lancedb`] (Ordner, Index,
/// Einbettungsmodell).
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
        // Das Geheimnis gehört zu einem anderen Ziel: nicht senden, nicht verbinden, ehrlich melden.
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
    // Ein gemeinsames Zeitlimit für die ganze Prüfung (MultiGPT schickt drei Anfragen; jede hat zwar
    // ihr eigenes Limit, zusammen sonst aber bis zu 30 s).
    let attempt = async {
        match kind {
            ConnectionKind::Eliezer => check::eliezer(url, secret.as_deref()).await,
            ConnectionKind::Multigpt => check::multigpt(url, secret.as_deref()).await,
            ConnectionKind::Pgvector => pgvector::check(&row.config, secret.as_deref()).await,
            // LanceDB kehrt oben zurück (crate::rag::check_lancedb).
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
        // Modelle der Personas gegen die Liste abgleichen (unbekannt, Alias); die Liste gilt danach
        // auch für `model.list` (Zwischenspeicher).
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

/// RAG-Wege eines Werks.
pub(crate) async fn rag_get(state: &AppState, work: WorkId) -> Result<Value, Fault> {
    let (ways, default) = state.store.work_rag(work).await?;
    Ok(json!({ "work": work, "ways": ways, "default": default }))
}

/// RAG-Wege eines Werks setzen.
pub(crate) async fn rag_set(state: &AppState, work: WorkId, ways: &[RagWay]) -> Result<Value, Fault> {
    let ways = state.store.set_work_rag(work, ways).await?;
    Ok(json!({ "work": work, "ways": ways, "default": false }))
}

/// Werk aus `work` (Zahl); fehlt es, das Vorgabewerk.
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

/// Wege aus einer Liste von Texten; unbekannt → `Invalid("weg")`.
fn ways_param(params: &Value) -> Result<Vec<RagWay>, Fault> {
    let invalid = || Fault::Invalid("weg", "ways: Liste aus lancedb, pgvector, multigpt.".into());
    let list = params.get("ways").and_then(Value::as_array).ok_or_else(invalid)?;
    list.iter()
        .map(|w| w.as_str().and_then(|w| RagWay::ALL.into_iter().find(|r| r.as_str() == w)).ok_or_else(invalid))
        .collect()
}

/// RPC `connection.*` und `sources.*` (aus `rpc::call` vor der Wahl des Werks).
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
            _ => Err(Fault::Internal(format!("unbekannte Methode {method}"))),
        }
    }
    .await;
    match answer {
        Err(Fault::Internal(message)) if message.starts_with("unbekannte Methode") => {
            Err(RpcError::new(METHOD_NOT_FOUND, message))
        }
        other => other.map_err(RpcError::from),
    }
}

/// `POST /einstellungen/verbindungen/act`, `POST /einstellungen/quellen/act` und (Runde 15b)
/// `POST /einstellungen/modelle/act` für die Modell-Verbindungen (im Router der schreibenden Routen, hinter
/// `guard::same_origin`). Die Seiten selbst zeigt der Rahmen der Einstellungen
/// (`GET /einstellungen/verbindungen`, `…/quellen`).
pub fn writes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/einstellungen/verbindungen/act", post(pages::connections_act))
        .route("/einstellungen/quellen/act", post(pages::sources_act))
        .route("/einstellungen/modelle/act", post(pages::models_act))
        // Runde 15: Kommentare und Lexika je Werk (Seite Einstellungen → Werk).
        .route("/einstellungen/werk/bibeln", post(pages::work_bibles_act))
}
