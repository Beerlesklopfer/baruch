// SPDX-License-Identifier: AGPL-3.0-or-later
//! Schicht der Anmeldung (Runde 35, Vertrag §S1.4 und §1.4): bestimmt, wer die Anfrage stellt, weist Unangemeldete
//! ab und fragt [`crate::roles::check_request`], ob die Rolle die Route darf. Liegt im Router direkt außerhalb von
//! `consent::inject` und innerhalb von `guard::remote_only_mcp`.
//!
//! Ohne Konten ändert die Schicht nichts an der Antwort (keine Kopfzeile, kein Cookie). Mit Konten bekommt jede
//! Antwort `Cache-Control: no-store`, wenn sie selbst keine Angabe trägt – weder eine Seite eines Angemeldeten noch
//! eine Abweisung soll in einem Zwischenspeicher liegen bleiben.

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

/// Was [`identify`] über eine Anfrage herausfindet.
struct Seen {
    who: Who,
    /// Token-Hash der Sitzung, unter der die Anfrage läuft.
    session: Option<String>,
    /// `Set-Cookie`, das die Lebensdauer des Cookies im Browser erneuert (erste Benutzung nach längerer Pause).
    renew: Option<String>,
}

/// Ablauf: (1) `/mcp` bleibt bei Tokens und läuft als [`Who::Open`] durch. (2) [`Who`] aus den Extensions der
/// Anfrage, falls vorhanden (nur im Prozess setzbar: Tests), sonst [`resolve`]. (3) [`Who::Anonymous`]: öffentliche
/// Pfade laufen durch; GET/HEAD → 303 auf `/anmelden?next=…`; `POST /rpc` → 401 mit JSON-RPC-Fehler
/// [`crate::rpc::UNAUTHENTICATED`]; sonst 401 Text. (4) [`roles::check_request`]: erlaubt, Umleitung oder 403
/// „Abgelehnt: …“ mit Kopfzeile `x-baruch-abgelehnt: rolle`. (5) Die Anfrage läuft in [`scope`].
pub async fn gate(State(state): State<Arc<AppState>>, request: Request, next: Next) -> Response {
    if request.uri().path() == "/mcp" {
        return scope(Who::Open, next.run(request)).await;
    }
    // Der Rumpf einer Anfrage ist nicht `Sync`: Über ein `await` hinweg wird darum nur der Kopf (`Parts`) verliehen.
    let (parts, body) = request.into_parts();
    let seen = match parts.extensions.get::<Who>() {
        Some(who) => Seen { who: who.clone(), session: None, renew: None },
        None => identify(&state, &parts).await,
    };
    let Seen { who, session, renew } = seen;
    // Ohne Konten bleibt die Antwort Byte für Byte, wie sie vor Runde 35 war.
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

/// Schritte (3) bis (5) von [`gate`].
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
                "error": { "code": crate::rpc::UNAUTHENTICATED, "message": "Anmeldung erforderlich." } });
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

/// Für einen Lektor zeigt das Cookie `baruch_work`, das die Seiten sehen, immer auf ein freigegebenes Werk: das
/// zuletzt geöffnete, wenn es freigegeben ist, sonst das kleinste freigegebene; ohne Freigabe fehlt es. Seiten ohne
/// eigenes Werk (`/werke`, `/konto`, `/ueber`) nehmen ihr Werk für „Zurück zum Werk“ und das Menüband aus diesem
/// Cookie bzw. – ohne Cookie – das Vorgabewerk der Instanz; beides könnte für einen Lektor ein fremdes Werk sein
/// (Sichtprüfung Runde 35: die Links endeten mit 403). Geändert wird nur die Kopfzeile dieser einen Anfrage, nie
/// das Cookie im Browser. Ein Datenbankfehler nimmt das Cookie weg (nie ein fremdes Werk).
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

/// Zuletzt geöffnetes Werk des Browsers (Cookie `baruch_work`, gelesen wie überall über
/// [`crate::view::Settings`]); nur ein Wunsch für die Umleitung eines Lektors, nie ein Recht.
fn last_work(headers: &HeaderMap) -> Option<baruch_store::WorkId> {
    let cookies = headers.get(header::COOKIE).and_then(|value| value.to_str().ok());
    crate::view::Settings::from_cookie_header(cookies).work
}

/// Setzt die Antwort das Cookie der Sitzung schon selbst (Anmelden, Abmelden)? Dann wird es nicht erneuert.
fn sets_session_cookie(headers: &HeaderMap) -> bool {
    headers
        .get_all(header::SET_COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .any(|cookie| cookie.strip_prefix(SESSION_COOKIE).is_some_and(|rest| rest.starts_with('=')))
}

/// Gegenstelle der Verbindung (`ConnectInfo`); ohne sie (Tests ohne Angabe, Unix-Socket) `None`.
pub fn peer(parts: &Parts) -> Option<SocketAddr> {
    parts.extensions.get::<ConnectInfo<SocketAddr>>().map(|info| info.0)
}

/// Kommt die Anfrage vom eigenen Rechner im Sinn der Entscheidung 0.2? Loopback, nicht über einen Proxy-Namen
/// ([`crate::shutdown::own_machine`]) und ohne jede Kopfzeile, die ein Proxy anhängt ([`crate::guard::forwarded`]).
/// Ohne bekannte Gegenstelle: nein.
pub fn own_machine(state: &AppState, parts: &Parts) -> bool {
    crate::shutdown::own_machine(&state.allowed_hosts, peer(parts), &parts.headers)
        && !crate::guard::forwarded(&parts.headers)
}

/// Wer stellt die Anfrage? Ohne Konten [`Who::Open`]; sonst Sitzung, Ausnahme am eigenen Rechner oder
/// [`Who::Anonymous`] (Vertrag §1.4).
///
/// Nimmt den Kopf der Anfrage (Methode, Adresse, Kopfzeilen, Extensions mit `ConnectInfo`), nicht die ganze Anfrage:
/// `&Request` ließe sich nicht über ein `await` halten (der Rumpf ist nicht `Sync`, die Schicht wäre nicht `Send`).
pub async fn resolve(state: &AppState, request: &Parts) -> Who {
    identify(state, request).await.who
}

/// [`resolve`] samt Sitzung und fälligem Erneuern des Cookies. Ein Datenbankfehler führt nie zu mehr Rechten: Er
/// steht auf stderr, und die Anfrage gilt als nicht angemeldet.
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

/// Prozentkodierung für den Wert von `next`: alles außer Buchstaben, Ziffern und `-._~/` wird `%XX`.
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
