// SPDX-License-Identifier: AGPL-3.0-or-later
//! Schicht der Anmeldung (Runde 35, Vertrag §S1.4): bestimmt, wer die Anfrage stellt, weist Unangemeldete ab und
//! fragt [`crate::roles::check_request`], ob die Rolle die Route darf. Liegt im Router direkt außerhalb von
//! `consent::inject` und innerhalb von `guard::remote_only_mcp`.

use std::sync::Arc;

use axum::Json;
use axum::extract::{Request, State};
use axum::http::request::Parts;
use axum::http::{Method, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Redirect, Response};
use serde_json::json;

use super::{PUBLIC_PATHS, PUBLIC_PREFIX, Who, scope};
use crate::AppState;
use crate::roles::{self, Verdict};

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
    let who = match parts.extensions.get::<Who>() {
        Some(who) => who.clone(),
        None => resolve(&state, &parts).await,
    };
    let path = parts.uri.path();
    if who == Who::Anonymous {
        if PUBLIC_PATHS.contains(&path) || path.starts_with(PUBLIC_PREFIX) {
            return scope(who, next.run(Request::from_parts(parts, body))).await;
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
    match roles::check_request(&state, &who, &parts.method, path, parts.uri.query()).await {
        Verdict::Allow => {}
        Verdict::Redirect(url) => return Redirect::to(&url).into_response(),
        Verdict::Deny(reason) => {
            return (StatusCode::FORBIDDEN, [("x-baruch-abgelehnt", "rolle")], format!("Abgelehnt: {reason}"))
                .into_response();
        }
    }
    scope(who, next.run(Request::from_parts(parts, body))).await
}

/// Wer stellt die Anfrage? Ohne Konten [`Who::Open`]; sonst Sitzung, Ausnahme am eigenen Rechner oder
/// [`Who::Anonymous`] (Vertrag §1.4).
///
/// Nimmt den Kopf der Anfrage (Methode, Adresse, Kopfzeilen, Extensions mit `ConnectInfo`), nicht die ganze Anfrage:
/// `&Request` ließe sich nicht über ein `await` halten (der Rumpf ist nicht `Sync`, die Schicht wäre nicht `Send`).
pub async fn resolve(state: &AppState, request: &Parts) -> Who {
    // STUB: Sitzung (Cookie), Ausnahme „Am eigenen Rechner ohne Anmeldung“ und Anonymous auflösen (Runde 35) –
    // bis dahin gibt es keine Konten (account_create ist ebenfalls Stub), also immer offen.
    let _ = (state, request);
    Who::Open
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
    use super::percent_encode;

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
}
