// SPDX-License-Identifier: AGPL-3.0-or-later
//! Baruch-Server: liefert Gerüst, Vorlagen und Widgets der Oberfläche aus und bedient den Kern
//! über JSON-RPC (`POST /rpc`). Geöffnet wird die Oberfläche im Browser.
//!
//! Daten: SQLite unter `<data>/baruch.sqlite` ist die Quelle ([`baruch_store`]), dazu die
//! Autorenbibliothek mit Volltextsuche ([`baruch_library`]) in derselben Datei. Jede Übernahme und
//! jedes Speichern wird danach als Commit in den Typst-Ordner `<data>/works/<slug>/` gespiegelt
//! ([`baruch_gitmirror`]); ein Fehler im Spiegel hält die Arbeit nicht auf, er wird gemeldet.
//!
//! Mehrere Werke: Jede Seite und jede RPC-Methode nimmt ein Werk (`?work=<id>` bzw. Parameter
//! `work`); ohne Angabe gilt das Vorgabewerk [`AppState::default_work`] (das mit der kleinsten id,
//! nach dem Löschen also das nächste), die Seite `/` nimmt vorher das zuletzt geöffnete Werk aus dem
//! Cookie `baruch_work`. Ohne Werke leitet `/` auf `/werke`. Jedes Werk hat seinen eigenen Spiegel,
//! geöffnet beim ersten Gebrauch ([`AppState::mirror`]). Werke verwaltet der Mensch über [`works`]
//! (Anlegen, Hochladen, Ändern, Löschen, Export); Typst-Dateien kommen über [`import`] hinzu.

pub mod about;
pub mod archive;
pub mod asset_requests;
pub mod assets;
pub mod assist;
pub mod band;
pub mod bible_research;
pub mod bibles;
pub mod chat_live;
pub mod chat_render;
pub mod connections;
pub mod consent;
pub mod cover;
pub mod cover_layout;
pub mod cover_print;
pub mod docx_export;
pub mod docx_import;
pub mod epub;
pub mod epub_check;
pub mod epub_preview;
pub mod exchange;
pub mod findings;
pub mod glossary;
pub mod glossary_page;
pub mod guard;
pub mod idea_map;
pub mod ideas;
pub mod import;
pub mod index_marks;
pub mod index_page;
pub mod instance;
pub mod laptop;
pub mod literature;
pub mod localtime;
pub mod markdown_import;
pub mod mcp;
pub mod menu;
pub mod mirror;
mod mirror_ideas;
pub mod models;
pub mod numbering;
pub mod outline;
pub mod pdf;
pub mod pdf_import;
pub mod personas;
pub mod posts;
pub mod quotes;
pub mod rag;
mod revision_fold;
mod revisions;
pub mod ribbon;
pub mod rpc;
pub mod scripture;
pub mod scripture_check;
pub mod scripture_map;
pub mod search;
pub mod search_hooks;
pub mod settings;
pub mod sheet;
pub mod shutdown;
pub mod sync;
pub mod threads;
pub mod timing;
pub mod typeset;
pub mod typeset_settings;
pub mod view;
pub mod websearch;
pub mod works;
pub mod wysiwyg;
pub mod xref;

use std::collections::HashMap;
use std::path::{Path as FsPath, PathBuf};
use std::sync::Arc;

use axum::extract::{Form, Path, Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::middleware;
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use baruch_gitmirror::Mirror;
use baruch_store::{Store, StoreError, WorkId};
use minijinja::Environment;
use serde::Deserialize;
use serde_json::{Value, json};

use view::{Location, Settings, Shelf};

fn settings(headers: &HeaderMap) -> Settings {
    Settings::from_cookie_header(headers.get(header::COOKIE).and_then(|v| v.to_str().ok()))
}

/// Übersetzte TypeScript-Widgets aus `build.rs`: `(Dateiname, Inhalt)`.
pub const SCRIPTS: &[(&str, &str)] = include!(concat!(env!("OUT_DIR"), "/scripts.rs"));
const STYLE: &str = concat!(
    include_str!("../static/app.css"),
    include_str!("../static/library.css"),
    include_str!("../static/works.css"),
    include_str!("../static/outline.css"),
    include_str!("../static/wysiwyg.css"),
    include_str!("../static/sheet.css"),
    include_str!("../static/numbering.css"),
    include_str!("../static/assets.css"),
    include_str!("../static/proposals.css"),
    include_str!("../static/ideas.css"),
    include_str!("../static/typeset.css"),
    include_str!("../static/ribbon.css"),
    include_str!("../static/menu.css"),
    include_str!("../static/settings.css"),
    include_str!("../static/connections.css"),
    include_str!("../static/assist.css"),
    include_str!("../static/cover.css"),
    include_str!("../static/consent.css"),
    include_str!("../static/commentaries.css"),
    include_str!("../static/toast.css"),
    include_str!("../static/literature_bibles.css"),
    include_str!("../static/threads.css"),
    include_str!("../static/predigt.css"),
    include_str!("../static/pruefung.css"),
    include_str!("../static/karte.css"),
    include_str!("../static/karte_widget.css"),
    include_str!("../static/karte_gliederung.css"),
    include_str!("../static/befunde.css"),
    include_str!("../static/chat.css"),
    include_str!("../static/search.css"),
    include_str!("../static/solutions.css"),
    include_str!("../static/typeset_settings.css"),
    // Runde 20: EPUB-Vorschau (reader) und Prüfung (epub-pruefung).
    include_str!("../static/epub.css"),
    include_str!("../static/epub_check.css"),
    // Runde 31: Glossar/Register-Seiten und Kontextmenü (kontextmenue).
    include_str!("../static/glossary.css"),
    include_str!("../static/context_menu.css"),
    // Runde 21 (Schnittstelle R21-S1): PDF/EPUB-Import mit Vorschau der Gliederung (pdf-import).
    include_str!("../static/source_import.css"),
    // Runde 22 (Schnittstelle R22-S1): Fortschreiben, Verlauf und Einstellung „Fassungen“ (fassungen).
    include_str!("../static/revisions.css"),
    // Runde 32 (Schnittstelle R32-S1): Word-Import, Word-Export und die Seite „Baruch beenden“.
    include_str!("../static/docx_import.css"),
    include_str!("../static/docx_export.css"),
    include_str!("../static/shutdown.css"),
    // Runde 33 (Schnittstelle R33-S1): Menüband (menueband) und Laptop-Modus (laptop); laptop.css zuletzt.
    include_str!("../static/band.css"),
    include_str!("../static/laptop.css")
);
const ICON: &str = include_str!("../static/favicon.svg");

pub struct AppState {
    pub store: Store,
    /// Datenordner (`baruch.sqlite`, `works/`).
    pub data: PathBuf,
    /// Vorgabe beim Start: das Werk mit der kleinsten id (0, wenn es keins gibt). Seit Werke gelöscht
    /// werden können, gilt zur Laufzeit [`AppState::default_work`].
    pub work: WorkId,
    /// Git-Spiegel je Werk unter `<data>/works/<slug>/`, geöffnet beim ersten Gebrauch.
    mirrors: std::sync::Mutex<HashMap<WorkId, Arc<Mirror>>>,
    /// Reiht Änderungen, die einen Spiegel berühren, hintereinander: DB-Schreibvorgang, Schnappschuss
    /// und Commit laufen unter dieser Sperre, damit jeder Commit genau seine Änderung enthält und
    /// ihr zugeschrieben ist (siehe [`mirror::record`]).
    pub write_lock: tokio::sync::Mutex<()>,
    pub templates: Environment<'static>,
    /// Geheimnisse der Verbindungen (Runde 9): im Betrieb der Schlüsselbund des Systems
    /// ([`connections::system_secrets`]), sonst [`connections::MemorySecrets`].
    pub secrets: Arc<dyn connections::SecretStore>,
    /// Modell-Backend der Personas (Runde 10): im Dienst Eliezer, in Tests `assist::mock::ScriptedModel`,
    /// sonst [`assist::NoModel`] (ehrliche Meldung statt Modellaufruf).
    pub model: Arc<dyn assist::ModelPort>,
    /// Zusätzlich erlaubte Rechnernamen (Runde 24, `--allow-host`): Betrieb hinter einem Reverse-Proxy. Nach
    /// dem Öffnen leer; `serve` setzt sie vor [`app`].
    pub allowed_hosts: guard::AllowedHosts,
    /// Auslöser für das Herunterfahren (Runde 32): `POST /beenden` verlangt es, [`shutdown::serve`] wartet darauf.
    pub shutdown: shutdown::Shutdown,
}

/// Fehler beim Start (Datenordner, Datenbank).
#[derive(Debug)]
pub struct StartError(pub String);

impl std::fmt::Display for StartError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for StartError {}

fn start_error<E: std::fmt::Display>(what: &'static str) -> impl FnOnce(E) -> StartError {
    move |e| StartError(format!("{what}: {e}"))
}

impl AppState {
    /// Öffnet den Datenordner in der festen Reihenfolge: `Store::open` (Kernschema, Beispielwerk),
    /// `baruch_library::migrate` (Literatur, FTS5), `baruch_library::seed_if_empty`; danach werden
    /// die Git-Spiegel aller Werke geöffnet und auf den Stand der Datenbank nachgeführt. Ein Fehler
    /// im Spiegel verhindert den Start nicht, er steht auf stderr.
    ///
    /// Geheimnisse der Verbindungen liegen dabei nur im Speicher ([`connections::MemorySecrets`]):
    /// Tests und Hilfsbefehle berühren so nie den echten Schlüsselbund; `serve` nimmt
    /// [`AppState::open_with_secrets`] mit dem Schlüsselbund des Systems. Ein Modell-Backend gibt es dabei
    /// nicht ([`assist::NoModel`]); Tests mit Personas nehmen [`AppState::open_with`].
    pub async fn open(data: &FsPath) -> Result<AppState, StartError> {
        Self::open_with_secrets(data, Arc::new(connections::MemorySecrets::default())).await
    }

    /// Wie [`AppState::open`], mit eigener Ablage für die Geheimnisse der Verbindungen (ohne
    /// Modell-Backend, [`assist::NoModel`]).
    pub async fn open_with_secrets(
        data: &FsPath,
        secrets: Arc<dyn connections::SecretStore>,
    ) -> Result<AppState, StartError> {
        Self::open_with(data, secrets, Arc::new(assist::NoModel)).await
    }

    /// Wie [`AppState::open_with_secrets`], mit einem Modell-Backend für die Personas (Runde 10).
    pub async fn open_with(
        data: &FsPath,
        secrets: Arc<dyn connections::SecretStore>,
        model: Arc<dyn assist::ModelPort>,
    ) -> Result<AppState, StartError> {
        let store = Store::open(data).await.map_err(start_error("Speicher"))?;
        baruch_library::migrate(store.pool()).await.map_err(start_error("Bibliothek"))?;
        baruch_library::seed_if_empty(store.pool()).await.map_err(start_error("Bibliothek"))?;
        let work = match store.default_work().await {
            Ok(work) => work,
            Err(StoreError::UnknownWork(_)) => 0,
            Err(e) => return Err(start_error("Speicher")(e)),
        };
        let state = AppState {
            store,
            data: data.to_owned(),
            work,
            mirrors: std::sync::Mutex::new(HashMap::new()),
            write_lock: tokio::sync::Mutex::new(()),
            templates: view::environment(),
            secrets,
            model,
            allowed_hosts: guard::AllowedHosts::default(),
            shutdown: shutdown::Shutdown::new(),
        };
        mirror::sync_all(&state).await;
        mirror::convert_legacy_parts(&state).await;
        Ok(state)
    }

    /// Vorgabewerk, wenn Seite oder RPC-Aufruf kein Werk nennen: das mit der kleinsten id, jetzt
    /// (nach dem Löschen eines Werks also das nächste). Ohne Werke → `UnknownWork(0)`.
    pub async fn default_work(&self) -> Result<WorkId, StoreError> {
        self.store.default_work().await
    }

    /// Git-Spiegel des Werks; beim ersten Gebrauch geöffnet bzw. angelegt (`<data>/works/<slug>/`).
    pub async fn mirror(&self, work: WorkId) -> Result<Arc<Mirror>, String> {
        if let Some(mirror) = self.mirror_cache().get(&work) {
            return Ok(mirror.clone());
        }
        let slug = self.store.work_slug(work).await.map_err(|e| e.to_string())?;
        let dir = mirror_dir(&self.data, &slug)?;
        let shown = dir.display().to_string();
        let mirror = tokio::task::spawn_blocking(move || Mirror::open_or_init(&dir))
            .await
            .map_err(|e| format!("Git: {e}"))?
            .map_err(|e| format!("Git-Spiegel {shown}: {e}"))?;
        Ok(self.mirror_cache().entry(work).or_insert_with(|| Arc::new(mirror)).clone())
    }

    /// Spiegel vergessen, damit der nächste Gebrauch ihn neu öffnet (nach einem Fehler, nach dem
    /// Löschen eines Werks).
    pub(crate) fn forget_mirror(&self, work: WorkId) {
        self.mirror_cache().remove(&work);
    }

    fn mirror_cache(&self) -> std::sync::MutexGuard<'_, HashMap<WorkId, Arc<Mirror>>> {
        self.mirrors.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

/// Ordner des Typst-Spiegels eines Werks: `<data>/works/<slug>`. Der Ordnername muss
/// `^[a-z0-9]+(-[a-z0-9]+)*$` sein und höchstens 48 Zeichen haben; `<data>/works` und
/// `<data>/works/<slug>` dürfen keine symbolischen Verknüpfungen sein.
pub fn mirror_dir(data: &FsPath, slug: &str) -> Result<PathBuf, String> {
    let valid = !slug.is_empty()
        && slug.len() <= 48
        && slug
            .split('-')
            .all(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit()));
    if !valid {
        return Err(format!("ungültiger Ordnername für den Git-Spiegel: {slug:?}"));
    }
    let works = data.join("works");
    let dir = works.join(slug);
    for path in [&works, &dir] {
        if std::fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink()) {
            return Err(format!(
                "{} ist eine symbolische Verknüpfung; der Git-Spiegel schreibt dort nicht",
                path.display()
            ));
        }
    }
    Ok(dir)
}

pub fn app(state: AppState) -> Router {
    router(Arc::new(state))
}

/// Größte Anfrage an `/mcp` (Runde 23, Codereview ki-2): ein Bild mit 8 MiB (`baruch_store::MAX_ASSET_BYTES`) als
/// Base64 (≈ 10,7 MiB) samt Rahmen; alles andere über MCP ist durch eigene Längengrenzen viel kleiner. Darüber 413.
pub const MCP_BODY_LIMIT: usize = 12 * 1024 * 1024;

/// Wie [`app`], mit geteiltem Zustand (Tests greifen so auf [`AppState::write_lock`] zu).
pub fn router(state: Arc<AppState>) -> Router {
    let writes = Router::new()
        .route("/rpc", post(rpc_endpoint).layer(axum::extract::DefaultBodyLimit::max(works::RPC_BODY_LIMIT)))
        .route("/act", post(act))
        // Runde 11: `asset_add` nimmt Bilder bis 8 MiB (Base64 größer). Runde 23 (Codereview ki-2): eigene,
        // kleinere Grenze [`MCP_BODY_LIMIT`] statt der von `/rpc`.
        .route("/mcp", post(mcp::endpoint).layer(axum::extract::DefaultBodyLimit::max(MCP_BODY_LIMIT)))
        .merge(literature::writes())
        .merge(works::writes())
        .merge(outline::writes())
        .merge(numbering::writes())
        .merge(assets::writes())
        .merge(asset_requests::writes())
        .merge(personas::writes())
        .merge(ideas::writes())
        .merge(posts::writes())
        .merge(settings::writes())
        .merge(connections::writes())
        .merge(cover::writes())
        .merge(websearch::writes())
        .merge(sync::writes())
        .merge(literature::bibles::writes())
        .merge(threads::writes())
        .merge(exchange::writes())
        .merge(idea_map::files::writes())
        .merge(typeset::writes())
        .merge(findings::page::writes())
        // Runde 30 (Schnittstelle R30-S1): Suchen/Ersetzen, Querverweise, Lösungen und Varianten.
        .merge(search::writes())
        .merge(xref::writes())
        .merge(assist::solutions::writes())
        // Runde 20 (Schnittstelle R20-S1): EPUB prüfen und Einstellung für EPUBCheck.
        .merge(epub_check::writes())
        // Runde 31 (Schnittstelle R31-S1): Glossar und Register.
        .merge(glossary_page::writes())
        .merge(index_page::writes())
        // Runde 21 (Schnittstelle R21-S1): PDF/EPUB-Import (Vorschau, Übernehmen, Verwerfen).
        .merge(pdf_import::writes())
        // Runde 22 (Schnittstelle R22-S1): Verlauf der Fortschreibungen und Einstellung „Fassungen“.
        .merge(revisions::writes())
        // 0.9.0 (Nutzer 2026-11-24): ältere Fassungen werkweit ausblenden.
        .merge(revision_fold::writes())
        // Runde 32 (Schnittstelle R32-S1): Word-Import (Vorschau, Übernehmen, Verwerfen) und „Baruch beenden“.
        .merge(docx_import::writes())
        .merge(shutdown::writes())
        .route_layer(middleware::from_fn(guard::same_origin));
    Router::new()
        .route("/", get(index))
        .merge(literature::pages())
        .merge(works::pages())
        .merge(outline::pages())
        .merge(numbering::pages())
        .merge(assets::pages())
        .merge(personas::pages())
        .merge(ideas::pages())
        .merge(typeset::pages())
        .merge(pdf::pages())
        .merge(settings::pages())
        .merge(cover::pages())
        .merge(websearch::pages())
        .merge(sync::pages())
        .merge(literature::bibles::pages())
        .merge(threads::pages())
        .merge(scripture_check::pages())
        .merge(exchange::pages())
        .merge(idea_map::bild::pages())
        .merge(idea_map::files::pages())
        .merge(findings::page::pages())
        .merge(posts::pages())
        .merge(search::pages())
        .merge(xref::pages())
        .merge(assist::solutions::pages())
        // Runde 20 (Schnittstelle R20-S1): EPUB herunterladen, Vorschau je Reader, Prüfung.
        .merge(epub::pages())
        .merge(epub_preview::pages())
        .merge(epub_check::pages())
        .merge(glossary_page::pages())
        .merge(index_page::pages())
        .merge(pdf_import::pages())
        .merge(revisions::pages())
        // Runde 24 (Schnittstelle R24-S1): Über Baruch.
        .merge(about::pages())
        // Runde 32 (Schnittstelle R32-S1): Word-Import, Word-Export, Bestätigungsseite „Baruch beenden“.
        .merge(docx_import::pages())
        .merge(docx_export::pages())
        .merge(shutdown::pages())
        .merge(writes)
        .route("/static/app.css", get(|| async { asset("text/css; charset=utf-8", STYLE) }))
        .route("/static/favicon.svg", get(|| async { asset("image/svg+xml", ICON) }))
        .route("/static/js/{name}", get(script))
        // Runde 10: Von anderen Rechnern nur `/mcp` (mit Token, geprüft in `mcp::endpoint`).
        // Nutzerfrage 12: Hinweisleiste und Dialog für Anträge, die Zustimmung brauchen, auf jeder Seite.
        .layer(middleware::from_fn_with_state(state.clone(), consent::inject))
        .layer(middleware::from_fn(guard::remote_only_mcp))
        .layer(middleware::from_fn(guard::own_reads))
        .layer(middleware::from_fn_with_state(state.clone(), guard::local_host))
        // Clickjacking: keine Seite und keine Antwort in einem fremden Rahmen (auch nicht 403/421).
        .layer(middleware::from_fn(guard::no_framing))
        .with_state(state)
}

fn asset(kind: &'static str, body: &'static str) -> Response {
    ([(header::CONTENT_TYPE, kind), (header::CACHE_CONTROL, "no-cache")], body).into_response()
}

async fn script(Path(name): Path<String>) -> Response {
    match SCRIPTS.iter().find(|(file, _)| *file == name) {
        Some((_, body)) => asset("text/javascript; charset=utf-8", body),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

fn internal(e: impl std::fmt::Display) -> Response {
    (StatusCode::INTERNAL_SERVER_ERROR, format!("Fehler: {e}")).into_response()
}

/// Werk und Werkwahl für Seite und `ui.render`. Unbekanntes Werk → `UnknownWork`; ein Werk ohne
/// Abschnitte lässt sich nicht zeigen (`Refused`).
pub(crate) async fn shelf(state: &AppState, work: WorkId) -> Result<(baruch_core::Work, Shelf), StoreError> {
    let loaded = state.store.load_work(work).await?;
    if loaded.nodes.is_empty() {
        return Err(StoreError::Refused("Das Werk hat keine Abschnitte."));
    }
    let slug = state.store.work_slug(work).await?;
    let works = state.store.work_list().await?;
    Ok((loaded, Shelf { id: work, slug, works }))
}

/// Werk der Seite `/` ohne `?work=`: das aus dem Cookie `baruch_work`, wenn es das Werk noch gibt,
/// sonst das Vorgabewerk; `None`, wenn es gar kein Werk gibt.
async fn chosen_work(state: &AppState, remembered: Option<WorkId>) -> Result<Option<WorkId>, StoreError> {
    if let Some(work) = remembered {
        match state.store.work_slug(work).await {
            Ok(_) => return Ok(Some(work)),
            Err(StoreError::UnknownWork(_)) => {}
            Err(e) => return Err(e),
        }
    }
    match state.default_work().await {
        Ok(work) => Ok(Some(work)),
        Err(StoreError::UnknownWork(_)) => Ok(None),
        Err(e) => Err(e),
    }
}

/// Cookie, das sich das gezeigte Werk merkt (ein Jahr, nur dieser Server).
fn work_cookie(work: WorkId) -> header::HeaderValue {
    header::HeaderValue::from_str(&format!("baruch_work={work}; Path=/; Max-Age=31536000; SameSite=Lax"))
        .expect("Ziffern sind ein gültiger Kopfzeilenwert")
}

async fn index(State(state): State<Arc<AppState>>, headers: HeaderMap, Query(at): Query<Location>) -> Response {
    let remembered = settings(&headers).work;
    let work_id = match at.work {
        Some(work) => work,
        None => match chosen_work(&state, remembered).await {
            Ok(Some(work)) => work,
            Ok(None) => return Redirect::to("/werke").into_response(),
            Err(e) => return internal(e),
        },
    };
    let (work, shelf) = match shelf(&state, work_id).await {
        Ok(found) => found,
        Err(StoreError::UnknownWork(_)) => return (StatusCode::NOT_FOUND, "unbekanntes Werk").into_response(),
        Err(e) => return internal(e),
    };
    let library = match rpc::library_view(&state, work_id, at.lq.as_deref()).await {
        Ok(library) => library,
        Err(e) => return internal(e.message),
    };
    let resolved = at.resolve(&work);
    match view::page(&state.templates, &work, &shelf, resolved, &settings(&headers), &library) {
        Ok(html) => {
            let mut response = Html(html).into_response();
            response.headers_mut().insert(header::SET_COOKIE, work_cookie(work_id));
            response
        }
        Err(e) => internal(e),
    }
}

async fn rpc_endpoint(State(state): State<Arc<AppState>>, headers: HeaderMap, body: String) -> Response {
    if !guard::is_json(&headers) {
        return guard::need_json();
    }
    let settings = settings(&headers);
    let response = match serde_json::from_str::<Value>(&body) {
        Ok(Value::Array(batch)) if !batch.is_empty() => {
            let mut answers = Vec::with_capacity(batch.len());
            for request in batch {
                answers.push(rpc::handle(&state, &settings, request).await);
            }
            Value::Array(answers)
        }
        Ok(request @ Value::Object(_)) => rpc::handle(&state, &settings, request).await,
        Ok(_) => json!({ "jsonrpc": "2.0", "id": null,
            "error": { "code": rpc::INVALID_REQUEST, "message": "Objekt oder Liste erwartet" } }),
        Err(e) => json!({ "jsonrpc": "2.0", "id": null,
            "error": { "code": rpc::PARSE_ERROR, "message": e.to_string() } }),
    };
    Json(response).into_response()
}

/// Formular ohne JavaScript: dieselbe RPC-Methode, danach zurück zur Seite.
#[derive(Deserialize)]
struct ActForm {
    method: String,
    /// Werk; fehlt es, das Vorgabewerk.
    work: Option<WorkId>,
    /// Stelle (Anzeige, Rückfall); fehlt sie, zählt nur `key`.
    node: Option<usize>,
    /// Fester Schlüssel des Abschnitts (`Node::key`); geht vor `node` (siehe `rpc::resolve_keys`).
    key: Option<String>,
    rev: Option<usize>,
    body: Option<String>,
    note: Option<String>,
    based_on: Option<u32>,
    /// Runde 23: Stand der Fortschreibungen, auf dem das Formular „Fassung ändern“ beruht (`revision.update`).
    based_on_amend: Option<u32>,
    view: Option<String>,
    /// Suche im Literatur-Popup; bleibt nach der Weiterleitung erhalten.
    lq: Option<String>,
    /// Kästchen „wirklich löschen“ (`1` oder `on`); Pflicht bei `revision.delete`.
    confirm: Option<String>,
    /// Runde 22: Speichermodus von `revision.save` („auto“ | „new“); fehlt er, gilt „new“.
    mode: Option<String>,
}

async fn act(State(state): State<Arc<AppState>>, Form(form): Form<ActForm>) -> Response {
    let work = match form.work {
        Some(work) => work,
        None => match state.default_work().await {
            Ok(work) => work,
            Err(e) => return (StatusCode::UNPROCESSABLE_ENTITY, e.to_string()).into_response(),
        },
    };
    let mut params = json!({ "work": work });
    if let Some(node) = form.node {
        params["node"] = json!(node);
    }
    if let Some(key) = &form.key {
        params["key"] = json!(key);
    }
    if let Some(rev) = form.rev {
        params["rev"] = json!(rev);
    }
    if let Some(body) = &form.body {
        params["body"] = json!(body);
    }
    if let Some(note) = &form.note {
        params["note"] = json!(note);
    }
    if let Some(based_on) = form.based_on {
        params["based_on"] = json!(based_on);
    }
    if let Some(amend) = form.based_on_amend {
        params["based_on_amend"] = json!(amend);
    }
    if let Some(mode) = &form.mode {
        params["mode"] = json!(mode);
    }
    if form.method == "revision.delete" {
        // Der Server prüft das Kästchen selbst; `required` im HTML allein hält niemanden auf.
        if !matches!(form.confirm.as_deref(), Some("1" | "on")) {
            return (StatusCode::UNPROCESSABLE_ENTITY, "Löschen bitte bestätigen.").into_response();
        }
        params["confirm"] = json!(true);
    }
    let allowed = [
        "revision.accept",
        "revision.reject",
        "revision.save",
        "post.add",
        "revision.set_current",
        "revision.update",
        "revision.delete",
        // 0.9.0: ältere Fassungen „bis hierhin“ ausblenden bzw. wieder alle zeigen (nur Mensch, ohne JavaScript).
        "revision.fold_set",
        "revision.fold_clear",
    ];
    if !allowed.contains(&form.method.as_str()) {
        return (StatusCode::BAD_REQUEST, "unbekannte Aktion").into_response();
    }
    let (node, rev) = match rpc::call(&state, &Settings::default(), &form.method, params).await {
        Ok(result) => (
            result["node"].as_u64().map(|n| n as usize).or(form.node),
            result["rev"].as_u64().and_then(|r| u32::try_from(r).ok()).or(form.rev.and_then(|r| u32::try_from(r).ok())),
        ),
        Err(e) => return (StatusCode::UNPROCESSABLE_ENTITY, e.message).into_response(),
    };
    // Zurück in Key-Form (Vertrag Runde 8, §1): der Schlüssel des Abschnitts, den die Antwort nennt
    // (nach Umschalten mit anderer Ebene bleibt er derselbe, die Stelle kann sich ändern).
    let key = match node {
        Some(node) => state.store.node_key(work, node).await.ok(),
        None => None,
    };
    let key = key.or_else(|| form.key.as_deref().and_then(|k| k.trim().parse().ok()));
    let url = match key {
        Some(key) => view::node_url(
            work,
            key,
            rev,
            form.view.as_deref() == Some("preview"),
            "",
            &view::lq_param(form.lq.as_deref()),
        ),
        None => format!("/?work={work}{}", view::lq_param(form.lq.as_deref())),
    };
    Redirect::to(&url).into_response()
}
