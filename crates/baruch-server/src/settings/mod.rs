// SPDX-License-Identifier: AGPL-3.0-or-later
//! Settings (Round 9, „Menü und Einstellungen als Popup“, task settings): frame, registration
//! of the seven pages, routes, RPC `settings.*`.
//!
//! **Pages** in a fixed order ([`registry`]): Darstellung (cookies), Werk (SQLite + mirror),
//! Assistenten (SQLite), Verbindungen and Quellen ([`crate::connections`]: SQLite, secrets in the
//! keyring), MCP-Zugang (runtime), Daten (data folder). Every page
//! draws only its content `<section class="settings-page" data-settings-page-body="<slug>">…`
//! ([`PageRender`]); the frame with navigation and „Gespeichert: …“ is put around it by [`frame`].
//!
//! **Without JavaScript:** `GET /einstellungen` → 303 `/einstellungen/darstellung`;
//! `GET /einstellungen/<seite>` → whole page `settings.html`; `?fragment=1` → only the frame (the
//! popup loads it, `web/src/settings.ts`). Forms go by POST to `…/act` (behind
//! `guard::same_origin`) and redirect back with 303; invalid inputs → 422 with the page.
//!
//! **Work of the page:** query `work` > cookie `baruch_work` (if the work exists) > default work >
//! none. An explicitly named, unknown work → 404 (or -32602), so that never silently another
//! work is edited. All other query pairs except `fragment` go unchanged to the
//! page (`ok`, `fehler`, `kind`, `schema`, `muster` …).
//!
//! **Request details:** the contract signature [`PageRender`] knows no headers. What the own
//! pages need of them (cookies for „Darstellung“, `Host` for „MCP-Zugang“, an error message after
//! a rejected form) lies in a `tokio::task_local` during drawing
//! ([`RequestInfo`]); without it defaults apply.

mod access;
mod assistants;
mod data;
mod display;
mod work;
pub mod work_typeset;

use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, OnceLock};

use axum::Router;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use baruch_store::{StoreError, WorkId};
use minijinja::value::Serde;
use minijinja::{Environment, Value, context};
use serde_json::{Value as Json, json};

use crate::AppState;
use crate::rpc::{INTERNAL, INVALID_PARAMS, METHOD_NOT_FOUND, RpcError};
use crate::view::Settings;

pub use data::{Backup, MAX_BACKUPS, RESERVE_BYTES, backup, newest_backup, open_permissions, permissions_warning};

/// Draws the content of a settings page (without the frame). Arguments: state, resolved work
/// (`None` = there is none), other query pairs. `Err` = message without secrets.
pub type PageRender =
    for<'a> fn(
        &'a crate::AppState,
        Option<baruch_store::WorkId>,
        &'a std::collections::BTreeMap<String, String>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String, String>> + Send + 'a>>;

/// A settings page: address `/einstellungen/<slug>`, title in the navigation, where the value lies.
pub struct SettingsPage {
    pub slug: &'static str,
    pub title: &'static str,
    pub storage: &'static str,
    pub render: PageRender,
}

static PAGES: [SettingsPage; 8] = [
    SettingsPage {
        slug: "darstellung",
        title: "Darstellung",
        storage: "Cookie in diesem Browser",
        render: render_display,
    },
    SettingsPage { slug: "werk", title: "Werk", storage: "SQLite, Git-Spiegel des Werks", render: render_work },
    SettingsPage {
        slug: "assistenten",
        title: "Assistenten",
        storage: "SQLite (alle Werke)",
        render: render_assistants,
    },
    SettingsPage {
        slug: "verbindungen",
        title: "Verbindungen",
        storage: "SQLite, Geheimnisse im Schlüsselbund",
        render: crate::connections::render_connections,
    },
    SettingsPage {
        slug: "quellen",
        title: "Quellen",
        storage: "SQLite (alle Werke)",
        render: crate::connections::render_sources,
    },
    SettingsPage {
        slug: "mcp",
        title: "MCP-Zugang",
        storage: "SQLite (nur Hash der Tokens); Clients zur Laufzeit",
        render: render_access,
    },
    SettingsPage { slug: "daten", title: "Daten", storage: "Datenordner", render: render_data },
    // Round 35: accounts (list, create a Lektor, lock, reset password, „Am eigenen Rechner ohne Anmeldung“).
    SettingsPage {
        slug: "konten",
        title: "Konten",
        storage: "SQLite dieser Instanz (Passwort nur als Prüfwert; nicht im Austausch, nicht im Spiegel)",
        render: crate::konten_ui::pages::render_settings,
    },
];

/// The eight pages in a fixed order.
pub fn registry() -> &'static [SettingsPage] {
    &PAGES
}

/// Page for a slug.
pub fn page(slug: &str) -> Option<&'static SettingsPage> {
    registry().iter().find(|p| p.slug == slug)
}

fn render_display<'a>(
    state: &'a AppState,
    work: Option<WorkId>,
    query: &'a BTreeMap<String, String>,
) -> Pin<Box<dyn Future<Output = Result<String, String>> + Send + 'a>> {
    Box::pin(display::render(state, work, query))
}

fn render_work<'a>(
    state: &'a AppState,
    work: Option<WorkId>,
    query: &'a BTreeMap<String, String>,
) -> Pin<Box<dyn Future<Output = Result<String, String>> + Send + 'a>> {
    Box::pin(work::render(state, work, query))
}

fn render_assistants<'a>(
    state: &'a AppState,
    work: Option<WorkId>,
    query: &'a BTreeMap<String, String>,
) -> Pin<Box<dyn Future<Output = Result<String, String>> + Send + 'a>> {
    Box::pin(assistants::render(state, work, query))
}

fn render_access<'a>(
    state: &'a AppState,
    work: Option<WorkId>,
    query: &'a BTreeMap<String, String>,
) -> Pin<Box<dyn Future<Output = Result<String, String>> + Send + 'a>> {
    Box::pin(access::render(state, work, query))
}

fn render_data<'a>(
    state: &'a AppState,
    work: Option<WorkId>,
    query: &'a BTreeMap<String, String>,
) -> Pin<Box<dyn Future<Output = Result<String, String>> + Send + 'a>> {
    Box::pin(data::render(state, work, query))
}

// ---- Request details ------------------------------------------------------------------------------

/// What the own pages need from the request (see the module documentation).
#[derive(Debug, Clone, Default)]
pub(crate) struct RequestInfo {
    /// Cookies of the request (`baruch_*`).
    pub settings: Settings,
    /// `Host` header (already checked by `guard::local_host` for IP address/localhost); if it is missing
    /// (RPC), „MCP-Zugang“ shows only the path.
    pub host: Option<String>,
    /// Message of a rejected form (422).
    pub error: Option<String>,
    /// Rejected form of the page „Werk“ with its input (metadata/work template or scheme).
    pub work_retry: Option<WorkRetry>,
}

/// A form rejected on the page „Werk“: the page shows it again with the input and the message
/// instead of losing the input.
#[derive(Debug, Clone)]
pub(crate) enum WorkRetry {
    /// `POST /werke/act` `action=update`.
    Edit { form: crate::works::UpdateParams, error: String },
    /// `POST /werke/nummerierung/act` (the error is in `Shown::error`).
    Numbering(crate::numbering::Shown),
    /// `POST /werke/buchangaben/act` or `/werke/umschlag/act` (Round 11, groups „Buchangaben“ and „Umschlag“).
    Book(crate::cover::Retry),
    /// `POST /einstellungen/werk/satz` or `…/satz/vorlage` (Round 29c, block „Satz“): rejected input or
    /// preview (`action=preview`, status 200) with the unsaved values.
    Typeset(crate::settings::work_typeset::Retry),
}

tokio::task_local! {
    static REQUEST: RequestInfo;
}

/// Details of the running request; outside of a frame the defaults.
pub(crate) fn request() -> RequestInfo {
    REQUEST.try_with(Clone::clone).unwrap_or_default()
}

fn request_info(headers: &HeaderMap, error: Option<String>) -> RequestInfo {
    RequestInfo {
        settings: Settings::from_cookie_header(headers.get(header::COOKIE).and_then(|v| v.to_str().ok())),
        host: headers.get(header::HOST).and_then(|h| h.to_str().ok()).map(str::to_owned),
        error,
        work_retry: None,
    }
}

// ---- Templates ------------------------------------------------------------------------------------

/// Own template environment (autoescape by `.html`). Contains `work_edit.html` and
/// `work_numbering.html`, to show their `{% block content %}` on the page „Werk“.
fn templates() -> &'static Environment<'static> {
    static ENV: OnceLock<Environment<'static>> = OnceLock::new();
    ENV.get_or_init(|| {
        let mut env = Environment::new();
        crate::i18n::install(&mut env);
        for (name, source) in [
            ("base.html", include_str!("../../templates/base.html")),
            ("settings.html", include_str!("../../templates/settings.html")),
            ("settings/frame.html", include_str!("../../templates/settings/frame.html")),
            ("settings/darstellung.html", include_str!("../../templates/settings/darstellung.html")),
            ("settings/werk.html", include_str!("../../templates/settings/werk.html")),
            ("settings/werk_satz.html", include_str!("../../templates/settings/werk_satz.html")),
            ("settings/assistenten.html", include_str!("../../templates/settings/assistenten.html")),
            ("settings/mcp.html", include_str!("../../templates/settings/mcp.html")),
            ("settings/daten.html", include_str!("../../templates/settings/daten.html")),
            ("settings/konten.html", include_str!("../../templates/settings/konten.html")),
            ("work_edit.html", include_str!("../../templates/work_edit.html")),
            ("work_numbering.html", include_str!("../../templates/work_numbering.html")),
        ] {
            env.add_template(name, source).expect("Vorlage der Einstellungen fehlerhaft");
        }
        env
    })
}

pub(crate) fn render_template(name: &str, ctx: Value) -> Result<String, String> {
    templates().get_template(name).and_then(|t| t.render(ctx)).map_err(|e| format!("Vorlage {name}: {e}"))
}

/// Only `{% block content %}` of a template (form part of `work_edit.html`, `work_numbering.html`).
pub(crate) fn render_content_block(name: &str, ctx: Value) -> Result<String, String> {
    let template = templates().get_template(name).map_err(|e| format!("Vorlage {name}: {e}"))?;
    // The template inherits from base.html; render_captured draws it once as a whole, base.html needs
    // `settings` for that (only the theme in the <html> tag, which is not output here).
    let ctx = context! { settings => Value::from(Serde(Settings::default())), ..ctx };
    let mut captured = template.render_captured(ctx).map_err(|e| format!("Vorlage {name}: {e}"))?;
    captured.with_state_mut(|state| state.render_block("content")).map_err(|e| format!("Vorlage {name}: {e}"))
}

// ---- Frame ----------------------------------------------------------------------------------------

/// Error while drawing the frame.
#[derive(Debug)]
pub(crate) enum FrameError {
    /// Unknown page or unknown work (404 or -32602).
    NotFound(String),
    /// Internal error (500 or -32603).
    Internal(String),
}

impl From<FrameError> for RpcError {
    fn from(e: FrameError) -> Self {
        match e {
            FrameError::NotFound(m) => RpcError::new(INVALID_PARAMS, m),
            FrameError::Internal(m) => RpcError::new(INTERNAL, m),
        }
    }
}

/// Work of the page: `asked` (from query or parameter) > cookie > default work > none.
pub(crate) async fn resolve_work(
    state: &AppState,
    asked: Option<WorkId>,
    cookie: Option<WorkId>,
) -> Result<Option<WorkId>, FrameError> {
    if let Some(work) = asked {
        return match state.store.work_slug(work).await {
            Ok(_) => Ok(Some(work)),
            Err(StoreError::UnknownWork(_)) => Err(FrameError::NotFound(format!("unbekanntes Werk {work}"))),
            Err(e) => Err(FrameError::Internal(e.to_string())),
        };
    }
    if let Some(work) = cookie {
        // Round 36: the cookie never grants a right. For a Lektor it counts only if the work is released
        // (`roles::role_in`); otherwise the frame of a page without `work` showed the foreign work opened last.
        let who = crate::auth::current();
        let visible = match crate::roles::role_in(state, &who, work).await {
            Ok(role) => role.is_some() || who.lektor().is_none(),
            Err(e) => return Err(FrameError::Internal(e.to_string())),
        };
        if visible {
            match state.store.work_slug(work).await {
                Ok(_) => return Ok(Some(work)),
                Err(StoreError::UnknownWork(_)) => {}
                Err(e) => return Err(FrameError::Internal(e.to_string())),
            }
        }
    }
    match state.default_work().await {
        Ok(work) => Ok(Some(work)),
        Err(StoreError::UnknownWork(_)) => Ok(None),
        Err(e) => Err(FrameError::Internal(e.to_string())),
    }
}

/// A drawn frame.
pub(crate) struct Framed {
    pub page: &'static SettingsPage,
    pub work: Option<WorkId>,
    /// `<div class="settings-frame" data-settings-frame …>…</div>`
    pub html: String,
}

/// Frame of a page: navigation (one link per page with `data-settings-page`, the shown one with
/// `aria-current="page"`), storage location and the content of the page. `info` applies during drawing.
pub(crate) async fn frame(
    state: &AppState,
    slug: &str,
    asked: Option<WorkId>,
    query: BTreeMap<String, String>,
    info: RequestInfo,
) -> Result<Framed, FrameError> {
    let page = page(slug).ok_or_else(|| FrameError::NotFound(format!("unbekannte Einstellungsseite „{slug}“")))?;
    let work = resolve_work(state, asked, info.settings.work).await?;
    let body = REQUEST.scope(info, (page.render)(state, work, &query)).await.map_err(FrameError::Internal)?;
    // Round 35: a Lektor reaches only „Darstellung“ (cookies in the own browser); the other pages are not in
    // the navigation for him (the lock itself lies in `roles`).
    let lektor = crate::auth::current().lektor().is_some();
    let pages: Vec<Value> = registry()
        .iter()
        .filter(|p| !lektor || p.slug == "darstellung")
        .map(|p| context! { slug => p.slug, title => p.title, current => p.slug == page.slug })
        .collect();
    let html = render_template(
        "settings/frame.html",
        context! { slug => page.slug, storage => page.storage, pages => pages, work => work, body => body },
    )
    .map_err(FrameError::Internal)?;
    Ok(Framed { page, work, html })
}

/// Query → (work, other pairs without `work` and `fragment`, `fragment=1`?). A `work` that is not a
/// number counts as not named.
fn split_query(pairs: Vec<(String, String)>) -> (Option<WorkId>, BTreeMap<String, String>, bool) {
    let mut work = None;
    let mut fragment = false;
    let mut rest = BTreeMap::new();
    for (key, value) in pairs {
        match key.as_str() {
            "work" => work = value.trim().parse::<WorkId>().ok().filter(|&w| w > 0),
            "fragment" => fragment = value == "1",
            _ => {
                rest.insert(key, value);
            }
        }
    }
    (work, rest, fragment)
}

/// Whole page `settings.html` around a frame.
async fn full_page(state: &AppState, headers: &HeaderMap, status: StatusCode, framed: Framed) -> Response {
    let settings = Settings::from_cookie_header(headers.get(header::COOKIE).and_then(|v| v.to_str().ok()));
    let work = match framed.work {
        Some(id) => match state.store.work_meta(id).await {
            Ok(meta) => Some(context! { id => id, title => meta.title, author => meta.author }),
            Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, format!("Fehler: {e}")).into_response(),
        },
        None => None,
    };
    let ctx = context! {
        settings => Value::from(Serde(settings)), work => work, frame => framed.html, title => framed.page.title,
    };
    match render_template("settings.html", ctx) {
        Ok(html) => (status, Html(html)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, format!("Fehler: {e}")).into_response(),
    }
}

fn frame_error(e: FrameError) -> Response {
    match e {
        FrameError::NotFound(m) => (StatusCode::NOT_FOUND, m).into_response(),
        FrameError::Internal(m) => (StatusCode::INTERNAL_SERVER_ERROR, format!("Fehler: {m}")).into_response(),
    }
}

/// Answer of a page: whole or as a fragment, with status.
#[allow(clippy::too_many_arguments)]
async fn respond(
    state: &AppState,
    headers: &HeaderMap,
    status: StatusCode,
    slug: &str,
    asked: Option<WorkId>,
    query: BTreeMap<String, String>,
    fragment: bool,
    error: Option<String>,
) -> Response {
    let framed = match frame(state, slug, asked, query, request_info(headers, error)).await {
        Ok(framed) => framed,
        Err(e) => return frame_error(e),
    };
    if fragment {
        (status, [(header::CACHE_CONTROL, "no-store")], Html(framed.html)).into_response()
    } else {
        full_page(state, headers, status, framed).await
    }
}

/// 422 with the whole page `slug` and the message (rejected form without JavaScript; the popup
/// takes the frame from it).
pub(crate) async fn error_page(
    state: &AppState,
    headers: &HeaderMap,
    slug: &str,
    work: Option<WorkId>,
    message: &str,
) -> Response {
    let status = StatusCode::UNPROCESSABLE_ENTITY;
    respond(state, headers, status, slug, work, BTreeMap::new(), false, Some(message.to_owned())).await
}

/// Rejected form of the page „Werk“ (from `POST /werke/act` or `/werke/nummerierung/act` with
/// `back=/einstellungen/werk…`): whole page „Werk“ with the status `status`, message and the input. The
/// popup takes `[data-settings-frame]` from it; without JavaScript the settings page stands there.
pub(crate) async fn work_retry_page(
    state: &AppState,
    headers: &HeaderMap,
    status: StatusCode,
    work: WorkId,
    retry: WorkRetry,
) -> Response {
    let info = RequestInfo { work_retry: Some(retry), ..request_info(headers, None) };
    match frame(state, "werk", Some(work), BTreeMap::new(), info).await {
        Ok(framed) => full_page(state, headers, status, framed).await,
        Err(e) => frame_error(e),
    }
}

/// Work suffix `&work=W` for returns.
pub(crate) fn work_suffix(work: Option<WorkId>) -> String {
    work.map_or(String::new(), |w| format!("&work={w}"))
}

// ---- Routes ---------------------------------------------------------------------------------------

/// `GET /einstellungen` → 303 `/einstellungen/darstellung[?work=W]`.
async fn index(Query(pairs): Query<Vec<(String, String)>>) -> Response {
    let (work, _, _) = split_query(pairs);
    let to = work.map_or("/einstellungen/darstellung".to_owned(), |w| format!("/einstellungen/darstellung?work={w}"));
    Redirect::to(&to).into_response()
}

/// `GET /einstellungen/{seite}[?work=W][&fragment=1][&…]`.
async fn show(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(slug): Path<String>,
    Query(pairs): Query<Vec<(String, String)>>,
) -> Response {
    let (work, query, fragment) = split_query(pairs);
    respond(&state, &headers, StatusCode::OK, &slug, work, query, fragment, None).await
}

/// Reading routes: `/einstellungen`, `/einstellungen/{seite}`, preview images of the block „Satz“ (Round 29c).
pub fn pages() -> Router<Arc<AppState>> {
    Router::new()
        .route("/einstellungen", get(index))
        .route("/einstellungen/{seite}", get(show))
        // Round 29c: preview images of the block „Satz“.
        .route("/einstellungen/werk/satz/vorschau/{stand}/{file}", get(work_typeset::preview_svg))
}

/// Writing routes (behind `guard::same_origin`): display (cookies), backup, name of the instance
/// (Round 26), MCP tokens, sermon time of the work (Round 16b), typeset view (Round 18c), typesetting (Round 29c).
pub fn writes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/einstellungen/darstellung/act", post(display::act))
        .route("/einstellungen/daten/sicherung", post(data::act))
        .route("/einstellungen/daten/instanz", post(data::instance_act))
        .route("/einstellungen/mcp/act", post(access::act))
        .route("/einstellungen/werk/zeit", post(work::timing_act))
        .route("/einstellungen/werk/sprache", post(work::language_act))
        .route("/einstellungen/werk/satzbild", post(work::sheet_act))
        // Round 29c: block „Satz“ (save, preview, reset; take over the template rule).
        .route("/einstellungen/werk/satz", post(work_typeset::act))
        .route("/einstellungen/werk/satz/vorlage", post(work_typeset::adopt_act))
}

// ---- RPC ------------------------------------------------------------------------------------------

const METHODS: &[&str] = &["settings.pages", "settings.render", "settings.get", "settings.set", "settings.backup"];

/// Does this module serve the RPC method?
pub fn handles(method: &str) -> bool {
    METHODS.contains(&method)
}

fn invalid(message: impl Into<String>) -> RpcError {
    RpcError::new(INVALID_PARAMS, message)
}

fn page_param(params: &Json) -> Result<&'static SettingsPage, RpcError> {
    let slug = params.get("page").and_then(Json::as_str).ok_or_else(|| invalid("page fehlt (z. B. „darstellung“)"))?;
    page(slug).ok_or_else(|| invalid(format!("unbekannte Einstellungsseite „{slug}“")))
}

fn work_param(params: &Json) -> Result<Option<WorkId>, RpcError> {
    match params.get("work") {
        None | Some(Json::Null) => Ok(None),
        Some(value) => {
            value.as_i64().filter(|&w| w > 0).map(Some).ok_or_else(|| invalid("work: Werk-id (Zahl) erwartet"))
        }
    }
}

/// RPC `settings.*` (`docs/RPC.md`, „Einstellungen“). `settings` are the cookies of the request.
pub async fn call(state: &AppState, settings: &Settings, method: &str, params: Json) -> Result<Json, RpcError> {
    let params = if params.is_null() { json!({}) } else { params };
    match method {
        "settings.pages" => {
            let pages: Vec<Json> = registry()
                .iter()
                .map(|p| json!({ "slug": p.slug, "title": p.title, "storage": p.storage, "url": format!("/einstellungen/{}", p.slug) }))
                .collect();
            Ok(json!({ "pages": pages }))
        }
        "settings.render" => {
            let page = page_param(&params)?;
            let mut query = BTreeMap::new();
            match params.get("query") {
                None | Some(Json::Null) => {}
                Some(Json::Object(map)) => {
                    for (key, value) in map {
                        let value = match value {
                            Json::String(s) => s.clone(),
                            Json::Number(n) => n.to_string(),
                            Json::Bool(b) => b.to_string(),
                            _ => return Err(invalid(format!("query.{key}: Text erwartet"))),
                        };
                        if key != "work" && key != "fragment" {
                            query.insert(key.clone(), value);
                        }
                    }
                }
                Some(_) => return Err(invalid("query: Objekt erwartet")),
            }
            let info = RequestInfo { settings: settings.clone(), host: None, error: None, work_retry: None };
            let framed = frame(state, page.slug, work_param(&params)?, query, info).await?;
            Ok(json!({ "page": page.slug, "title": page.title, "storage": page.storage, "html": framed.html }))
        }
        "settings.get" => {
            let page = page_param(&params)?;
            let work = resolve_work(state, work_param(&params)?, settings.work).await?;
            match page.slug {
                "darstellung" => Ok(display::values(settings)),
                "werk" => work::values(state, work).await,
                "assistenten" => assistants::values(state).await,
                "mcp" => access::values(state).await,
                "daten" => data::values(state).await,
                "verbindungen" | "quellen" => {
                    Err(invalid("Verbindungen und Quellen liest man über connection.list bzw. sources.list."))
                }
                other => Err(invalid(format!("unbekannte Einstellungsseite „{other}“"))),
            }
        }
        "settings.set" => {
            let page = page_param(&params)?;
            let values = match params.get("values") {
                Some(Json::Object(map)) => Json::Object(map.clone()),
                _ => return Err(invalid("values: Objekt mit den Feldern der Seite erwartet")),
            };
            match page.slug {
                "werk" => {
                    let work = resolve_work(state, work_param(&params)?, settings.work).await?;
                    work::set(state, work, values).await
                }
                "assistenten" => assistants::set(state, values).await,
                "darstellung" => Err(invalid(
                    "Darstellung liegt in Cookies dieses Browsers: Cookies setzt das Formular POST /einstellungen/darstellung/act.",
                )),
                "verbindungen" | "quellen" => {
                    Err(invalid("Verbindungen und Quellen schreibt man über connection.* bzw. sources.*."))
                }
                other => Err(invalid(format!("Die Seite „{other}“ hat nichts zu speichern."))),
            }
        }
        "settings.backup" => {
            if params.get("confirm") != Some(&Json::Bool(true)) {
                return Err(invalid("Sicherung bitte bestätigen (confirm: true)."));
            }
            let made = backup(state).await.map_err(|e| RpcError::new(INTERNAL, e))?;
            Ok(json!({ "dir": made.dir.display().to_string(), "size_bytes": made.size_bytes, "skipped": made.skipped }))
        }
        _ => Err(RpcError::new(METHOD_NOT_FOUND, format!("unknown method {method}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_registry_lists_eight_pages_in_the_agreed_order() {
        let slugs: Vec<&str> = registry().iter().map(|p| p.slug).collect();
        assert_eq!(
            slugs,
            ["darstellung", "werk", "assistenten", "verbindungen", "quellen", "mcp", "daten", "konten"],
            "Reihenfolge nach Vertrag Runde 9, §3"
        );
        assert!(registry().iter().all(|p| !p.title.is_empty() && !p.storage.is_empty()), "Titel und Speicherort");
    }

    #[test]
    fn the_query_keeps_every_pair_except_work_and_fragment() {
        let parse = |uri: &str| {
            let uri: axum::http::Uri = uri.parse().unwrap();
            Query::<Vec<(String, String)>>::try_from_uri(&uri).unwrap().0
        };
        let (work, rest, fragment) = split_query(parse("/e?work=3&fragment=1&ok=farbe&muster=I.%3B1.&x=a+b&leer="));
        assert_eq!((work, fragment), (Some(3), true), "work und fragment gelesen");
        assert_eq!(rest.get("ok").map(String::as_str), Some("farbe"), "ok bleibt");
        assert_eq!(rest.get("muster").map(String::as_str), Some("I.;1."), "Prozent-Kodierung gelöst");
        assert_eq!(rest.get("x").map(String::as_str), Some("a b"), "+ ist ein Leerzeichen");
        assert_eq!(rest.get("leer").map(String::as_str), Some(""), "Schlüssel ohne Wert");
        assert!(!rest.contains_key("work") && !rest.contains_key("fragment"), "beide nicht weitergereicht");
        let (bad, _, no) = split_query(parse("/e?work=abc&fragment=0"));
        assert_eq!((bad, no), (None, false), "work ohne Zahl zählt nicht, fragment nur bei 1");
        assert!(handles("settings.render") && !handles("settings.delete"), "nur die fünf Methoden");
    }
}
