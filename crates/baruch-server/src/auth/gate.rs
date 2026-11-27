// SPDX-License-Identifier: AGPL-3.0-or-later
//! Login layer (Round 35, contract §S1.4 and §1.4): determines who makes the request, turns away the
//! unauthenticated and asks [`crate::roles::check_request`] whether the role may use the route. Sits in the router
//! directly outside of `consent::inject` and inside of `guard::remote_only_mcp`.
//!
//! Without accounts the layer changes nothing in the response (no header, no cookie). With accounts every
//! response gets `Cache-Control: no-store` if it carries no directive itself – neither a page of a logged-in user
//! nor a rejection should remain in a cache.

use std::net::SocketAddr;
use std::sync::Arc;

use axum::Json;
use axum::extract::{ConnectInfo, Request, State};
use axum::http::request::Parts;
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Redirect, Response};
use baruch_store::Store;
use serde_json::json;

use super::{PUBLIC_PATHS, PUBLIC_PREFIX, SESSION_COOKIE, Who, scope, session};
use crate::AppState;
use crate::roles::{self, Verdict};

/// What [`identify`] finds out about a request.
struct Seen {
    who: Who,
    /// Token hash of the session the request runs under.
    session: Option<String>,
    /// `Set-Cookie` that renews the lifetime of the cookie in the browser (first use after a longer pause).
    renew: Option<String>,
}

/// Flow: (1) `/mcp` stays with tokens and passes as [`Who::Open`]. (2) [`Who`] from the extensions of the
/// request if present (settable only in-process: tests), otherwise [`resolve`]. (3) [`Who::Anonymous`]: public
/// paths pass; GET/HEAD → 303 to `/anmelden?next=…`; `POST /rpc` → 401 with JSON-RPC error
/// [`crate::rpc::UNAUTHENTICATED`]; otherwise 401 text. (4) [`roles::check_request`]: allowed, redirect or 403
/// „Abgelehnt: …“ with header `x-baruch-abgelehnt: rolle`. (5) The request runs in [`scope`].
pub async fn gate(State(state): State<Arc<AppState>>, request: Request, next: Next) -> Response {
    if request.uri().path() == "/mcp" {
        return scope(Who::Open, next.run(request)).await;
    }
    // The body of a request is not `Sync`: so across an `await` only the head (`Parts`) is borrowed.
    let (parts, body) = request.into_parts();
    let seen = match parts.extensions.get::<Who>() {
        Some(who) => Seen { who: who.clone(), session: None, renew: None },
        None => identify(&state, &parts).await,
    };
    let Seen { who, session, renew } = seen;
    // Without accounts the response stays byte for byte as it was before Round 35.
    let private = who != Who::Open;
    let mut response = answer(&state, who, session, parts, body, next).await;
    if private && !response.headers().contains_key(header::CACHE_CONTROL) {
        response.headers_mut().insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    }
    if let Some(cookie) = renew
        && !sets_session_cookie(response.headers())
    {
        session::set_cookie(&mut response, &cookie);
    }
    response
}

/// Steps (3) to (5) of [`gate`].
async fn answer(
    state: &AppState,
    who: Who,
    session: Option<String>,
    mut parts: Parts,
    body: axum::body::Body,
    next: Next,
) -> Response {
    let path = parts.uri.path();
    if who == Who::Anonymous {
        if PUBLIC_PATHS.contains(&path) || path.starts_with(PUBLIC_PREFIX) {
            return scope(who, session::scope(None, next.run(Request::from_parts(parts, body)))).await;
        }
        if parts.method == Method::GET || parts.method == Method::HEAD {
            let target = parts.uri.path_and_query().map_or(path, |pq| pq.as_str());
            return Redirect::to(&format!("/anmelden?next={}", percent_encode(target))).into_response();
        }
        if parts.method == Method::POST && path == "/rpc" {
            let body = json!({ "jsonrpc": "2.0", "id": null,
                "error": { "code": crate::rpc::UNAUTHENTICATED, "message": "Login required." } });
            return (StatusCode::UNAUTHORIZED, Json(body)).into_response();
        }
        return (StatusCode::UNAUTHORIZED, "Anmeldung erforderlich.").into_response();
    }
    let preferred = last_work(&parts.headers);
    match roles::check_request_preferring(state, &who, &parts.method, path, parts.uri.query(), preferred).await {
        Verdict::Allow => {}
        Verdict::Redirect(url) => return Redirect::to(&url).into_response(),
        Verdict::Deny(reason) => {
            return (StatusCode::FORBIDDEN, [("x-baruch-abgelehnt", "rolle")], format!("Abgelehnt: {reason}"))
                .into_response();
        }
    }
    if who.lektor().is_some() {
        pin_last_work(state, &who, preferred, &mut parts.headers).await;
    }
    scope(who, session::scope(session, next.run(Request::from_parts(parts, body)))).await
}

/// For a Lektor the cookie `baruch_work` that the pages see always points to a shared work: the
/// last opened one if it is shared, otherwise the smallest shared one; without a share it is missing. Pages without
/// a work of their own (`/werke`, `/konto`, `/ueber`) take their work for „Zurück zum Werk“ and the ribbon from this
/// cookie or – without a cookie – the default work of the instance; both could be a foreign work for a Lektor
/// (visual check Round 35: the links ended with 403). Only the header of this one request is changed, never
/// the cookie in the browser. A database error takes the cookie away (never a foreign work).
async fn pin_last_work(state: &AppState, who: &Who, preferred: Option<baruch_store::WorkId>, headers: &mut HeaderMap) {
    let granted = roles::granted_works(state, who).await.unwrap_or_else(|e| {
        eprintln!("Rollen: Freigaben nicht lesbar: {e}");
        Vec::new()
    });
    let work = preferred.filter(|work| granted.contains(work)).or(granted.first().copied());
    let mut pairs: Vec<String> = headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|line| line.split(';'))
        .map(|pair| pair.trim().to_owned())
        .filter(|pair| !pair.is_empty() && pair.split('=').next().map(str::trim) != Some("baruch_work"))
        .collect();
    if let Some(work) = work {
        pairs.push(format!("baruch_work={work}"));
    }
    headers.remove(header::COOKIE);
    if let Ok(value) = HeaderValue::from_str(&pairs.join("; "))
        && !pairs.is_empty()
    {
        headers.insert(header::COOKIE, value);
    }
}

/// Last opened work of the browser (cookie `baruch_work`, read as everywhere through
/// [`crate::view::Settings`]); only a wish for the redirect of a Lektor, never a right.
fn last_work(headers: &HeaderMap) -> Option<baruch_store::WorkId> {
    let cookies = headers.get(header::COOKIE).and_then(|value| value.to_str().ok());
    crate::view::Settings::from_cookie_header(cookies).work
}

/// Does the response already set the session cookie itself (login, logout)? Then it is not renewed.
fn sets_session_cookie(headers: &HeaderMap) -> bool {
    headers
        .get_all(header::SET_COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .any(|cookie| cookie.strip_prefix(SESSION_COOKIE).is_some_and(|rest| rest.starts_with('=')))
}

/// Remote side of the connection (`ConnectInfo`); without it (tests without a value, Unix socket) `None`.
pub fn peer(parts: &Parts) -> Option<SocketAddr> {
    parts.extensions.get::<ConnectInfo<SocketAddr>>().map(|info| info.0)
}

/// Does the request come from the own machine in the sense of decision 0.2? Loopback, not through a proxy name
/// ([`crate::shutdown::own_machine`]) and without any header a proxy appends ([`crate::guard::forwarded`]).
/// Without a known remote side: no.
pub fn own_machine(state: &AppState, parts: &Parts) -> bool {
    crate::shutdown::own_machine(&state.allowed_hosts, peer(parts), &parts.headers)
        && !crate::guard::forwarded(&parts.headers)
}

/// Who makes the request? Without accounts [`Who::Open`]; otherwise session, exception on the own machine or
/// [`Who::Anonymous`] (contract §1.4).
///
/// Takes the head of the request (method, address, headers, extensions with `ConnectInfo`), not the whole request:
/// `&Request` could not be held across an `await` (the body is not `Sync`, the layer would not be `Send`).
pub async fn resolve(state: &AppState, request: &Parts) -> Who {
    identify(state, request).await.who
}

/// [`resolve`] including session and due renewal of the cookie. A database error never leads to more rights: it
/// goes to stderr, and the request counts as not logged in.
async fn identify(state: &AppState, request: &Parts) -> Seen {
    if !super::accounts_exist(state).await {
        return Seen { who: Who::Open, session: None, renew: None };
    }
    if let Some(token) = session::token_from(&request.headers) {
        let hash = Store::session_token_hash(token);
        match state.store.session_account(&hash).await {
            Ok(Some((account, idle))) => {
                let renew = (idle >= session::RENEW_AFTER_IDLE_SECONDS)
                    .then(|| session::cookie(token, crate::guard::via_proxy(&state.allowed_hosts, &request.headers)));
                return Seen { who: Who::Account(super::identity_of(&account)), session: Some(hash), renew };
            }
            Ok(None) => {}
            Err(e) => eprintln!("Konten: Sitzung nicht lesbar: {e}"),
        }
    }
    if own_machine(state, request) {
        match state.store.auth_local_open().await {
            Ok(true) => return Seen { who: Who::LocalAuthor, session: None, renew: None },
            Ok(false) => {}
            Err(e) => eprintln!("Konten: Schalter „Am eigenen Rechner ohne Anmeldung“ nicht lesbar: {e}"),
        }
    }
    Seen { who: Who::Anonymous, session: None, renew: None }
}

/// Percent-encoding for the value of `next`: everything except letters, digits and `-._~/` becomes `%XX`.
fn percent_encode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' => out.push(byte as char),
            other => out.push_str(&format!("%{other:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn next_is_encoded_so_that_query_and_fragment_survive_as_one_value() {
        assert_eq!(percent_encode("/werke/3/befunde"), "/werke/3/befunde", "ein schlichter Pfad bleibt lesbar");
        assert_eq!(
            percent_encode("/?work=2&key=7#x"),
            "/%3Fwork%3D2%26key%3D7%23x",
            "?, =, & und # dürfen die Adresse der Anmeldeseite nicht zerteilen"
        );
        assert_eq!(percent_encode("/suche?q=ä b"), "/suche%3Fq%3D%C3%A4%20b", "Umlaute und Leerzeichen byteweise");
    }

    #[test]
    fn only_a_cookie_named_like_the_session_counts_as_set() {
        let mut headers = HeaderMap::new();
        headers.append(header::SET_COOKIE, HeaderValue::from_static("baruch_work=2; Path=/"));
        assert!(!sets_session_cookie(&headers), "ein anderes Cookie ist nicht das der Sitzung");
        headers.append(header::SET_COOKIE, HeaderValue::from_static("baruch_session_alt=x"));
        assert!(!sets_session_cookie(&headers), "ein ähnlicher Name zählt nicht");
        headers.append(header::SET_COOKIE, HeaderValue::from_static("baruch_session=; Max-Age=0"));
        assert!(sets_session_cookie(&headers), "das Löschen beim Abmelden darf kein Erneuern überschreiben");
    }
}
