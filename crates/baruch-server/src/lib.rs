// SPDX-License-Identifier: AGPL-3.0-or-later
//! Baruch server: serves the frame, templates and widgets of the user interface and serves the core
//! through JSON-RPC (`POST /rpc`). The user interface is opened in the browser.
//!
//! Data: SQLite under `<data>/baruch.sqlite` is the source ([`baruch_store`]), plus the
//! author's library with full-text search ([`baruch_library`]) in the same file. Every accept and
//! every save is afterwards mirrored as a commit into the Typst folder `<data>/works/<slug>/`
//! ([`baruch_gitmirror`]); an error in the mirror does not hold up the work, it is reported.
//!
//! Several works: every page and every RPC method takes a work (`?work=<id>` or parameter
//! `work`); without it the default work [`AppState::default_work`] applies (the one with the smallest id,
//! after deleting therefore the next one), the page `/` first takes the work opened last from the
//! cookie `baruch_work`. Without works `/` redirects to `/werke`. Every work has its own mirror,
//! opened on first use ([`AppState::mirror`]). The human manages works through [`works`]
//! (create, upload, change, delete, export); Typst files are added through [`import`].

pub mod about;
pub mod archive;
pub mod asset_requests;
pub mod assets;
pub mod assist;
pub mod auth;
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
pub mod konten_ui;
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
pub mod roles;
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
pub mod typst_cli;
pub mod view;
pub mod websearch;
pub mod work_language;
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

/// Compiled TypeScript widgets from `build.rs`: `(file name, content)`.
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
    // Round 20: EPUB preview (reader) and check (epub-pruefung).
    include_str!("../static/epub.css"),
    include_str!("../static/epub_check.css"),
    // Round 31: glossary/index pages and context menu (kontextmenue).
    include_str!("../static/glossary.css"),
    include_str!("../static/context_menu.css"),
    // Round 21 (interface R21-S1): PDF/EPUB import with a preview of the outline (pdf-import).
    include_str!("../static/source_import.css"),
    // Round 22 (interface R22-S1): amending, history and setting „Fassungen“ (fassungen).
    include_str!("../static/revisions.css"),
    // Round 32 (interface R32-S1): Word import, Word export and the page „Baruch beenden“.
    include_str!("../static/docx_import.css"),
    include_str!("../static/docx_export.css"),
    include_str!("../static/shutdown.css"),
    // Round 33 (interface R33-S1): ribbon (menueband) and laptop mode (laptop); laptop.css last.
    include_str!("../static/band.css"),
    include_str!("../static/laptop.css"),
    // Round 35 (interface R35-S1): sign-in, accounts, contributors and the rules `nur-autor`/`nur-lektor`.
    include_str!("../static/konten.css")
);
const ICON: &str = include_str!("../static/favicon.svg");

pub struct AppState {
    pub store: Store,
    /// Data folder (`baruch.sqlite`, `works/`).
    pub data: PathBuf,
    /// Default at start: the work with the smallest id (0 if there is none). Since works can be
    /// deleted, [`AppState::default_work`] applies at run time.
    pub work: WorkId,
    /// Git mirror per work under `<data>/works/<slug>/`, opened on first use.
    mirrors: std::sync::Mutex<HashMap<WorkId, Arc<Mirror>>>,
    /// Queues changes that touch a mirror one after another: database write, snapshot
    /// and commit run under this lock, so that every commit contains exactly its change and is
    /// attributed to it (see [`mirror::record`]).
    pub write_lock: tokio::sync::Mutex<()>,
    pub templates: Environment<'static>,
    /// Secrets of the connections (Round 9): in operation the keyring of the system
    /// ([`connections::system_secrets`]), otherwise [`connections::MemorySecrets`].
    pub secrets: Arc<dyn connections::SecretStore>,
    /// Model backend of the personas (Round 10): in the service Eliezer, in tests `assist::mock::ScriptedModel`,
    /// otherwise [`assist::NoModel`] (honest message instead of a model call).
    pub model: Arc<dyn assist::ModelPort>,
    /// Additionally allowed host names (Round 24, `--allow-host`): operation behind a reverse proxy. Empty
    /// after opening; `serve` sets them before [`app`].
    pub allowed_hosts: guard::AllowedHosts,
    /// Trigger for shutting down (Round 32): `POST /beenden` requests it, [`shutdown::serve`] waits for it.
    pub shutdown: shutdown::Shutdown,
    /// State of the sign-in (Round 35): per `AppState`, never process-wide; the fields are defined by [`auth`].
    pub auth: auth::AuthState,
}

/// Error at start (data folder, database).
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
    /// Opens the data folder in the fixed order: `Store::open` (core schema, sample work),
    /// `baruch_library::migrate` (literature, FTS5), `baruch_library::seed_if_empty`; afterwards
    /// the Git mirrors of all works are opened and brought up to the state of the database. An error
    /// in the mirror does not prevent the start, it is written to stderr.
    ///
    /// Secrets of the connections live only in memory here ([`connections::MemorySecrets`]):
    /// tests and helper commands thus never touch the real keyring; `serve` takes
    /// [`AppState::open_with_secrets`] with the keyring of the system. There is no model backend here
    /// ([`assist::NoModel`]); tests with personas take [`AppState::open_with`].
    pub async fn open(data: &FsPath) -> Result<AppState, StartError> {
        Self::open_with_secrets(data, Arc::new(connections::MemorySecrets::default())).await
    }

    /// Like [`AppState::open`], with its own storage for the secrets of the connections (without a
    /// model backend, [`assist::NoModel`]).
    pub async fn open_with_secrets(
        data: &FsPath,
        secrets: Arc<dyn connections::SecretStore>,
    ) -> Result<AppState, StartError> {
        Self::open_with(data, secrets, Arc::new(assist::NoModel)).await
    }

    /// Like [`AppState::open_with_secrets`], with a model backend for the personas (Round 10).
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
            auth: auth::AuthState::default(),
        };
        mirror::sync_all(&state).await;
        mirror::convert_legacy_parts(&state).await;
        Ok(state)
    }

    /// Default work when page or RPC call name no work: the one with the smallest id, now
    /// (after deleting a work therefore the next one). Without works → `UnknownWork(0)`.
    ///
    /// Round 35: for a Lektor ([`auth::current`]) it is the smallest **shared** work, without a share
    /// `UnknownWork(0)` – a page without `work` thus never shows him a foreign work and links to none
    /// („Zurück zum Werk“). Outside of a request (spawned tasks, start) the smallest id applies.
    pub async fn default_work(&self) -> Result<WorkId, StoreError> {
        let who = auth::current();
        if who.lektor().is_some() {
            let granted = roles::granted_works(self, &who).await?;
            return granted.first().copied().ok_or(StoreError::UnknownWork(0));
        }
        self.store.default_work().await
    }

    /// Git mirror of the work; opened or created on first use (`<data>/works/<slug>/`).
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

    /// Forget the mirror so that the next use reopens it (after an error, after
    /// deleting a work).
    pub(crate) fn forget_mirror(&self, work: WorkId) {
        self.mirror_cache().remove(&work);
    }

    fn mirror_cache(&self) -> std::sync::MutexGuard<'_, HashMap<WorkId, Arc<Mirror>>> {
        self.mirrors.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

/// Folder of the Typst mirror of a work: `<data>/works/<slug>`. The folder name must be
/// `^[a-z0-9]+(-[a-z0-9]+)*$` and have at most 48 characters; `<data>/works` and
/// `<data>/works/<slug>` must not be symbolic links.
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

/// Largest request to `/mcp` (Round 23, code review ki-2): an image of 8 MiB (`baruch_store::MAX_ASSET_BYTES`) as
/// Base64 (≈ 10.7 MiB) with its frame; everything else through MCP is much smaller by its own length limits. Above
/// that 413.
pub const MCP_BODY_LIMIT: usize = 12 * 1024 * 1024;

/// Like [`app`], with shared state (this way tests reach [`AppState::write_lock`]).
pub fn router(state: Arc<AppState>) -> Router {
    let writes = Router::new()
        .route("/rpc", post(rpc_endpoint).layer(axum::extract::DefaultBodyLimit::max(works::RPC_BODY_LIMIT)))
        .route("/act", post(act))
        // Round 11: `asset_add` takes images up to 8 MiB (Base64 larger). Round 23 (code review ki-2): own,
        // smaller limit [`MCP_BODY_LIMIT`] instead of that of `/rpc`.
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
        // Round 30 (interface R30-S1): search/replace, cross references, solutions and variants.
        .merge(search::writes())
        .merge(xref::writes())
        .merge(assist::solutions::writes())
        // Round 20 (interface R20-S1): check EPUB and setting for EPUBCheck.
        .merge(epub_check::writes())
        // Round 31 (interface R31-S1): glossary and index.
        .merge(glossary_page::writes())
        .merge(index_page::writes())
        // Round 21 (interface R21-S1): PDF/EPUB import (preview, accept, discard).
        .merge(pdf_import::writes())
        // Round 22 (interface R22-S1): history of the amendments and setting „Fassungen“.
        .merge(revisions::writes())
        // 0.9.0 (user 2026-11-24): hide older revisions work-wide.
        .merge(revision_fold::writes())
        // Round 32 (interface R32-S1): Word import (preview, accept, discard) and „Baruch beenden“.
        .merge(docx_import::writes())
        .merge(shutdown::writes())
        // Round 35 (interface R35-S1): sign in/sign out/set up/accounts and the forms of the roles.
        .merge(auth::routes::writes())
        .merge(roles::routes::writes())
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
        // Round 20 (interface R20-S1): download EPUB, preview per reader, check.
        .merge(epub::pages())
        .merge(epub_preview::pages())
        .merge(epub_check::pages())
        .merge(glossary_page::pages())
        .merge(index_page::pages())
        .merge(pdf_import::pages())
        .merge(revisions::pages())
        // Round 24 (interface R24-S1): about Baruch.
        .merge(about::pages())
        // Round 32 (interface R32-S1): Word import, Word export, confirmation page „Baruch beenden“.
        .merge(docx_import::pages())
        .merge(docx_export::pages())
        .merge(shutdown::pages())
        // Round 35 (interface R35-S1): sign-in page, set up, Mein Konto, Mitwirkende.
        .merge(konten_ui::pages())
        .merge(writes)
        .route("/static/app.css", get(|| async { asset("text/css; charset=utf-8", STYLE) }))
        .route("/static/favicon.svg", get(|| async { asset("image/svg+xml", ICON) }))
        .route("/static/js/{name}", get(script))
        // Round 10: from other machines only `/mcp` (with a token, checked in `mcp::endpoint`).
        // User question 12: notice bar and dialog for requests that need consent, on every page.
        .layer(middleware::from_fn_with_state(state.clone(), consent::inject))
        // Round 35: sign-in and roles – outside of `inject` (page and bar are already created under the
        // established identity), inside of `remote_only_mcp` (foreign machines do not get this far at all).
        .layer(middleware::from_fn_with_state(state.clone(), auth::gate))
        .layer(middleware::from_fn(guard::remote_only_mcp))
        .layer(middleware::from_fn(guard::own_reads))
        .layer(middleware::from_fn_with_state(state.clone(), guard::local_host))
        // Clickjacking: no page and no response in a foreign frame (not even 403/421).
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

/// Work and work selection for page and `ui.render`. Unknown work → `UnknownWork`; a work without
/// sections cannot be shown (`Refused`).
pub(crate) async fn shelf(state: &AppState, work: WorkId) -> Result<(baruch_core::Work, Shelf), StoreError> {
    let loaded = state.store.load_work(work).await?;
    if loaded.nodes.is_empty() {
        return Err(StoreError::Refused("Das Werk hat keine Abschnitte."));
    }
    let slug = state.store.work_slug(work).await?;
    // Round 35: the work selection shows only what the role may see.
    let works = roles::visible_works(state, &auth::current()).await?;
    Ok((loaded, Shelf { id: work, slug, works }))
}

/// Work of the page `/` without `?work=`: the one from the cookie `baruch_work` if the work still exists,
/// otherwise the default work; `None` if there is no work at all.
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

/// Cookie that remembers the work shown (one year, this server only).
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
            "error": { "code": rpc::INVALID_REQUEST, "message": "object or list expected" } }),
        Err(e) => json!({ "jsonrpc": "2.0", "id": null,
            "error": { "code": rpc::PARSE_ERROR, "message": e.to_string() } }),
    };
    Json(response).into_response()
}

/// Form without JavaScript: the same RPC method, then back to the page.
#[derive(Deserialize)]
struct ActForm {
    method: String,
    /// Work; if missing, the default work.
    work: Option<WorkId>,
    /// Position (display, fallback); if missing, only `key` counts.
    node: Option<usize>,
    /// Fixed key of the section (`Node::key`); takes precedence over `node` (see `rpc::resolve_keys`).
    key: Option<String>,
    rev: Option<usize>,
    body: Option<String>,
    note: Option<String>,
    based_on: Option<u32>,
    /// Round 23: state of the amendments on which the form „Fassung ändern“ is based (`revision.update`).
    based_on_amend: Option<u32>,
    view: Option<String>,
    /// Search in the literature popup; is kept after the redirect.
    lq: Option<String>,
    /// Checkbox „wirklich löschen“ (`1` or `on`); mandatory for `revision.delete`.
    confirm: Option<String>,
    /// Round 22: save mode of `revision.save` („auto“ | „new“); if missing, „new“ applies.
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
        // The server checks the checkbox itself; `required` in the HTML alone stops nobody.
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
        // 0.9.0: hide older revisions „bis hierhin“ or show all again (human only, without JavaScript).
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
    // Back in key form (contract Round 8, §1): the key of the section that the answer names
    // (after switching with a different level it stays the same, the position can change).
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
