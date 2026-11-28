// SPDX-License-Identifier: AGPL-3.0-or-later
//! Active consent to requests (user question 12, decision of the user 2026-11-07: „es braucht eine
//! aktive Zustimmung (Popup)“).
//!
//! If an assistant creates a part in such a way that existing chapters would move under it, a request arises
//! instead of the part ([`baruch_store::Store::outline_add_by_assistant`]). As long as such a request is open,
//! every page that extends `base.html` shows a notice bar with a link to the request (usable without
//! JavaScript: a form there with „Zustimmen“/„Ablehnen“) and a dialog with preview and
//! the same buttons, which the widget `consent` (`web/src/consent.ts`) opens modally. Nothing happens without
//! a click: no timeout and no closing of the dialog counts as consent.
//!
//! Implementation: `base.html` carries the marker [`MARKER`]; the layer [`inject`] replaces it in every
//! HTML response with [`banner`] (empty if nothing is open; since Round 35 also empty for Lektors – only
//! someone with full rights can consent). So none of the many
//! template environments needs own data for it. The query runs only if the marker stands in the response.

use std::sync::{Arc, OnceLock};

use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::{HeaderValue, header};
use axum::middleware::Next;
use axum::response::Response;
use minijinja::value::Serde;
use minijinja::{Environment, context};
use serde_json::{Value as Json, json};

use crate::AppState;
use crate::i18n::tc;

/// Placeholder in `base.html` (directly below the header).
pub const MARKER: &str = "<!--baruch-consent-->";

/// Largest HTML response into which the notice bar is inserted (larger ones stay unchanged).
const MAX_PAGE: usize = 32 * 1024 * 1024;

fn templates() -> &'static Environment<'static> {
    static ENV: OnceLock<Environment<'static>> = OnceLock::new();
    ENV.get_or_init(|| {
        let mut env = Environment::new();
        crate::i18n::install(&mut env);
        env.add_template("consent.html", include_str!("../templates/consent.html"))
            .expect("Vorlage der Zustimmung fehlerhaft");
        env
    })
}

/// Notice bar and dialog for all open requests that need consent; empty if none is open.
/// Errors (database, template) go to stderr; the page then comes without the bar – the requests
/// stay visible on `/gliederung`.
pub async fn banner(state: &AppState) -> String {
    match build(state).await {
        Ok(html) => html,
        Err(e) => {
            eprintln!("Zustimmung: {e}");
            String::new()
        }
    }
}

async fn build(state: &AppState) -> Result<String, String> {
    let requests = state.store.outline_requests_needing_consent().await.map_err(|e| e.to_string())?;
    if requests.is_empty() {
        return Ok(String::new());
    }
    let titles = state.store.works().await.map_err(|e| e.to_string())?;
    let mut shown: Vec<Json> = Vec::new();
    for request in &requests {
        let mut value = crate::outline::request_json(request);
        crate::outline::add_preview(state, request.work, request, &mut value).await.map_err(|e| e.message)?;
        let title = titles.iter().find(|(id, _, _)| *id == request.work).map_or("", |(_, _, t)| t.as_str());
        value["work_title"] = json!(title);
        shown.push(value);
    }
    let first = &requests[0];
    let ctx = context! {
        requests => minijinja::Value::from(Serde(&shown)),
        // Only digits and fixed characters: as a safe value so that `/` is not escaped.
        first_url => minijinja::Value::from_safe_string(format!("/gliederung?work={}#antrag-{}", first.work, first.id)),
    };
    templates()
        .get_template("consent.html")
        .and_then(|t| t.render(ctx))
        .map_err(|e| tc("view_msg.consent.template_error", &[("error", e.to_string().into())]))
}

/// Layer above all routes: inserts the notice bar into HTML responses with [`MARKER`] (or removes
/// the marker) and fills the badges of the header (Round 23a, `menu::fill_page`); since Round 33 it also puts
/// the state of ribbon and laptop mode in there (`band::apply`) and remembers it as a cookie. Other responses
/// (JSON, PDF, images, fragments without marker) pass through unchanged.
pub async fn inject(State(state): State<Arc<AppState>>, request: Request, next: Next) -> Response {
    // Round 33 (interface R33-S1): state of ribbon and laptop mode from cookie and address, captured
    // before the request is passed on.
    let cookies = request.headers().get(header::COOKIE).and_then(|v| v.to_str().ok()).map(str::to_owned);
    let query = request.uri().query().map(str::to_owned);
    let band = crate::band::BandState::from_request(cookies.as_deref(), query.as_deref());
    let laptop = crate::laptop::LaptopState::from_request(cookies.as_deref(), query.as_deref());
    let response = next.run(request).await;
    let html = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.starts_with("text/html"));
    if !html {
        return response;
    }
    let (mut parts, body) = response.into_parts();
    let bytes = match axum::body::to_bytes(body, MAX_PAGE).await {
        Ok(bytes) => bytes,
        Err(e) => {
            eprintln!("Zustimmung: Seite nicht lesbar: {e}");
            parts.headers.remove(header::CONTENT_LENGTH);
            return Response::from_parts(parts, Body::from(tc("view_msg.consent.page_too_large", &[])));
        }
    };
    let Some(text) = std::str::from_utf8(&bytes).ok().filter(|t| t.contains(MARKER)) else {
        // Round 36: the standalone account pages (`/anmelden`, `/einrichten`, without ribbon and without marker)
        // follow the cookie `baruch_theme` – without JavaScript, without a cookie unchanged.
        let theme = crate::view::Settings::from_cookie_header(cookies.as_deref()).theme;
        if let Some(page) = std::str::from_utf8(&bytes).ok().and_then(|t| crate::konten_ui::themed(t, theme)) {
            parts.headers.remove(header::CONTENT_LENGTH);
            return Response::from_parts(parts, Body::from(page));
        }
        return Response::from_parts(parts, Body::from(bytes));
    };
    // Round 35: only the author can consent – so notice bar and dialog go only to someone with full rights
    // (without accounts everybody, as before). For Lektors only the marker disappears.
    let who = crate::auth::current();
    let consent = if who.full_rights() { banner(&state).await } else { String::new() };
    let page = text.replacen(MARKER, &consent, 1);
    // Round 23a: fill the badges of the header outside the work page on the server (also without JavaScript).
    let page = crate::menu::fill_page(&state, page).await;
    // Round 33: active tab, addresses of the tabs and laptop state in the finished HTML; remember valid keys of the
    // address as a cookie – append, not replace (`/` already sets `baruch_work`).
    let page = crate::band::apply(&page, &band, &laptop);
    // Round 35 (interface R35-S1): account entry in the menu „Baruch ▾“ and role on <body>; the query runs only
    // if the marker stands in the page.
    let page = if page.contains(crate::konten_ui::KONTO_MARK) {
        let accounts_exist = crate::auth::accounts_exist(&state).await;
        crate::konten_ui::apply(&page, &who, accounts_exist)
    } else {
        page
    };
    // Round 36: display name for the origin `@lektorat:<slug>` (only with accounts, only in origin elements).
    let page = crate::konten_ui::names::show_for(&state, page).await;
    for cookie in
        crate::band::set_cookies(query.as_deref()).into_iter().chain(crate::laptop::set_cookies(query.as_deref()))
    {
        match HeaderValue::from_str(&cookie) {
            Ok(value) => {
                parts.headers.append(header::SET_COOKIE, value);
            }
            Err(e) => eprintln!("Menüband: Cookie nicht setzbar: {e}"),
        }
    }
    parts.headers.remove(header::CONTENT_LENGTH);
    Response::from_parts(parts, Body::from(page))
}
