// SPDX-License-Identifier: AGPL-3.0-or-later
//! Runde 35, konten (Vertrag §1): Passwort, Sitzung, Sperre gegen Durchprobieren, Anmelden/Abmelden/Einrichten,
//! `account.*`, Notweg `passwort`.
//!
//! Die Tests gehen den echten Weg (Formulare, Cookie, Schicht `auth::gate`) über den Router; die Gegenstelle
//! (`ConnectInfo`) setzt jeder Test selbst – ohne sie gilt eine Anfrage nie als „eigener Rechner“. Wer die Anfrage
//! stellt, fragen die Tests über `auth::gate::resolve` ab: Das Ergebnis hängt nicht davon ab, was `roles` einer Rolle
//! schon erlaubt. Die Gestaltung der Seiten gehört „oberflaeche_konten“ – geprüft werden nur Status, Kopfzeilen,
//! Cookies und Feldnamen. Zwei Tests starten das echte Programm (Standardfehlerausgabe, Befehl `passwort`); ihre
//! Verbindungen zeigen auf eine tote Adresse, kein Test fragt einen echten Dienst.

use std::io::{BufRead, BufReader, Write};
use std::net::SocketAddr;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{HeaderMap, Request, StatusCode, header};
use baruch_server::auth::{self, AccountKind, Who};
use baruch_server::guard::AllowedHosts;
use serde_json::{Value, json};

use crate::common::{self, Server};

/// Erfundene Passwörter (10 bis 200 Zeichen, nur Zeichen, die im Formular ohne Kodierung stehen dürfen).
const AUTHOR_PW: &str = "muehle-am-hang-26";
const LEKTOR_PW: &str = "wehrgraben-im-mai";
const NEW_PW: &str = "mahlgang-neu-2027";
/// Name, unter dem der Proxy im Test erreichbar ist (`--allow-host`).
const PROXY: &str = "baruch.example.org";

const LOOPBACK: SocketAddr = SocketAddr::new(std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST), 50000);

fn get(uri: &str) -> Request<Body> {
    Request::get(uri).body(Body::empty()).unwrap()
}

fn form(uri: &str, body: &str) -> Request<Body> {
    Request::post(uri)
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(body.to_owned()))
        .unwrap()
}

fn rpc_request(method: &str, params: Value) -> Request<Body> {
    let body = json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params });
    Request::post("/rpc").header(header::CONTENT_TYPE, "application/json").body(Body::from(body.to_string())).unwrap()
}

/// Die Anfrage kommt vom eigenen Rechner: Gegenstelle Loopback, `Host` eine IP-Adresse.
fn local(mut request: Request<Body>) -> Request<Body> {
    request.extensions_mut().insert(ConnectInfo(LOOPBACK));
    request.headers_mut().insert(header::HOST, "127.0.0.1:8765".parse().unwrap());
    request
}

/// Die Anfrage kommt über den Proxy auf demselben Rechner: Gegenstelle Loopback, `Host` der Proxy-Name.
fn proxied(mut request: Request<Body>) -> Request<Body> {
    request.extensions_mut().insert(ConnectInfo(LOOPBACK));
    request.headers_mut().insert(header::HOST, PROXY.parse().unwrap());
    request
}

fn with_cookie(mut request: Request<Body>, token: &str) -> Request<Body> {
    request.headers_mut().insert(header::COOKIE, format!("baruch_work=1; baruch_session={token}").parse().unwrap());
    request
}

fn with_header(mut request: Request<Body>, name: &'static str, value: &'static str) -> Request<Body> {
    request.headers_mut().insert(name, value.parse().unwrap());
    request
}

/// Alle `Set-Cookie` der Antwort, die das Cookie der Sitzung setzen.
fn session_cookies(headers: &HeaderMap) -> Vec<String> {
    headers
        .get_all(header::SET_COOKIE)
        .iter()
        .map(|value| value.to_str().unwrap().to_owned())
        .filter(|cookie| cookie.starts_with("baruch_session="))
        .collect()
}

/// Das Token aus dem `Set-Cookie` der Antwort.
fn token_of(headers: &HeaderMap) -> String {
    let cookies = session_cookies(headers);
    assert_eq!(cookies.len(), 1, "genau ein Set-Cookie der Sitzung erwartet: {cookies:?}");
    cookies[0].trim_start_matches("baruch_session=").split(';').next().unwrap().to_owned()
}

fn location(headers: &HeaderMap) -> &str {
    headers.get(header::LOCATION).map_or("", |value| value.to_str().unwrap())
}

/// Server, der zusätzlich den Proxy-Namen erlaubt (wie `serve --allow-host baruch.example.org`).
async fn open_behind_proxy() -> Server {
    let data = tempfile::tempdir().unwrap();
    let mut state = baruch_server::AppState::open(data.path()).await.expect("Datenordner");
    state.allowed_hosts = AllowedHosts::parse(PROXY).unwrap();
    let state = Arc::new(state);
    Server { app: baruch_server::router(state.clone()), state, data }
}

/// Richtet das erste Konto vom eigenen Rechner ein; liefert das Token der Sitzung des Autors.
async fn setup_author(server: &Server) -> String {
    let body = format!("name=Erika&display=Erika+Beispiel&password={AUTHOR_PW}&password2={AUTHOR_PW}");
    let (status, headers, text) = server.send(local(form("/einrichten", &body))).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Einrichten vom eigenen Rechner gelingt: {text}");
    token_of(&headers)
}

/// Der Autor legt einen Lektor an; liefert dessen id.
async fn create_lektor(server: &Server, author: &str, name: &str, password: &str) -> i64 {
    let body = format!("action=create&name={name}&display=&password={password}&password2={password}");
    let (status, headers, text) = server.send(with_cookie(form("/einstellungen/konten/act", &body), author)).await;
    assert_eq!(
        (status, location(&headers)),
        (StatusCode::SEE_OTHER, "/einstellungen/konten?ok=konto_angelegt"),
        "der Autor legt den Lektor {name} an: {text}"
    );
    sqlx::query_scalar("SELECT id FROM account WHERE name = ?")
        .bind(name)
        .fetch_one(server.state.store.pool())
        .await
        .unwrap()
}

async fn login(server: &Server, name: &str, password: &str) -> (StatusCode, HeaderMap, String) {
    server.send(form("/anmelden", &format!("name={name}&password={password}&next=/"))).await
}

/// Meldet an und liefert das Token.
async fn sign_in(server: &Server, name: &str, password: &str) -> String {
    let (status, headers, text) = login(server, name, password).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Anmelden als {name} gelingt: {text}");
    token_of(&headers)
}

/// Wer stellt diese Anfrage (ohne sie auszuführen)?
async fn who(server: &Server, request: Request<Body>) -> Who {
    let (parts, _) = request.into_parts();
    auth::gate::resolve(&server.state, &parts).await
}

async fn scalar(server: &Server, sql: &str) -> i64 {
    sqlx::query_scalar(sql).fetch_one(server.state.store.pool()).await.unwrap_or_else(|e| panic!("{sql}: {e}"))
}

async fn exec(server: &Server, sql: &str) {
    sqlx::query(sql).execute(server.state.store.pool()).await.unwrap_or_else(|e| panic!("{sql}: {e}"));
}

/// Lässt jede Sperre gegen Durchprobieren ablaufen (die Zählung bleibt).
async fn expire_blocks(server: &Server) {
    exec(server, "UPDATE login_attempt SET blocked_until = strftime('%Y-%m-%dT%H:%M:%fZ','now','-1 seconds')").await;
}

async fn rpc_as(server: &Server, token: &str, method: &str, params: Value) -> Value {
    let (status, _, body) = server.send(with_cookie(rpc_request(method, params), token)).await;
    assert_eq!(status, StatusCode::OK, "{method}: /rpc antwortet einem Angemeldeten mit 200: {body}");
    serde_json::from_str(&body).unwrap()
}

// ---- ohne Konten ---------------------------------------------------------------------------------------------

#[tokio::test]
async fn without_accounts_pages_rpc_and_forms_answer_as_before() {
    let server = common::open().await;
    for with_peer in [false, true] {
        let wrap = |request: Request<Body>| if with_peer { local(request) } else { request };
        let (status, headers, body) = server.send(wrap(get("/"))).await;
        assert_eq!(status, StatusCode::OK, "ohne Konten zeigt GET / die Arbeitsseite (Gegenstelle: {with_peer})");
        assert!(body.contains("<html"), "es ist die Seite, keine Umleitung");
        assert!(session_cookies(&headers).is_empty(), "ohne Konten setzt Baruch kein Cookie der Sitzung");
        assert_ne!(
            headers.get(header::CACHE_CONTROL).map(|v| v.to_str().unwrap()),
            Some("no-store"),
            "ohne Konten hängt die Schicht keine Kopfzeile an"
        );
        let (status, headers, body) = server.send(wrap(rpc_request("work.list", json!({})))).await;
        let answer: Value = serde_json::from_str(&body).unwrap();
        assert_eq!(status, StatusCode::OK, "POST /rpc antwortet ohne Konten wie bisher");
        assert!(
            answer["error"].is_null() && answer["result"]["works"].as_array().is_some_and(|works| works.len() == 1),
            "work.list liefert das Beispielwerk: {answer}"
        );
        assert!(session_cookies(&headers).is_empty(), "auch /rpc setzt kein Cookie");
        let (status, headers, body) = server.send(wrap(form("/act", "method=post.add&node=0&body=Notiz"))).await;
        assert_eq!(status, StatusCode::SEE_OTHER, "POST /act schreibt und leitet zurück wie bisher: {body}");
        assert!(!location(&headers).starts_with("/anmelden"), "keine Umleitung zur Anmeldung: {}", location(&headers));
        assert!(session_cookies(&headers).is_empty(), "auch /act setzt kein Cookie");
        assert_eq!(who(&server, wrap(get("/"))).await, Who::Open, "ohne Konten ist jede Anfrage offen");
    }
    let me = server.rpc("account.me", json!({})).await;
    assert_eq!(
        me["result"],
        json!({ "accounts_exist": false, "signed_in": false, "local_open": false }),
        "account.me ohne Konten: nichts eingerichtet, niemand angemeldet"
    );
    let (status, headers, _) = login(&server, "Erika", AUTHOR_PW).await;
    assert_eq!((status, location(&headers)), (StatusCode::SEE_OTHER, "/"), "ohne Konten führt Anmelden zur Startseite");
    assert!(session_cookies(&headers).is_empty(), "und setzt kein Cookie");
    let (status, headers, _) = server.send(form("/abmelden", "")).await;
    assert_eq!((status, location(&headers)), (StatusCode::SEE_OTHER, "/"), "Abmelden ohne Konten ebenso");
    assert!(session_cookies(&headers).is_empty(), "auch Abmelden setzt ohne Konten kein Cookie");
    assert_eq!(scalar(&server, "SELECT count(*) FROM account").await, 0, "dabei ist kein Konto entstanden");
    assert_eq!(scalar(&server, "SELECT count(*) FROM login_attempt").await, 0, "und kein Fehlversuch gezählt");
}

// ---- Einrichten ----------------------------------------------------------------------------------------------

#[tokio::test]
async fn the_first_account_is_set_up_only_from_the_own_machine() {
    let server = open_behind_proxy().await;
    let good = format!("name=Erika&display=Erika+Beispiel&password={AUTHOR_PW}&password2={AUTHOR_PW}");
    let (status, _, body) = server.send(form("/einrichten", &good)).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "ohne bekannte Gegenstelle gilt die Anfrage nicht als eigener Rechner");
    assert!(!body.contains("<form"), "die Antwort erklärt nur, sie bietet kein Formular an");
    let (status, _, _) = server.send(proxied(form("/einrichten", &good))).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "über den Proxy-Namen lässt sich das erste Konto nicht einrichten");
    for name in ["forwarded", "x-forwarded-for", "x-forwarded-host", "x-forwarded-proto", "x-real-ip", "via"] {
        let (status, _, _) = server.send(with_header(local(form("/einrichten", &good)), name, "203.0.113.9")).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "mit der Proxy-Kopfzeile {name} ist es nicht der eigene Rechner");
    }
    let mut remote = form("/einrichten", &good);
    remote.extensions_mut().insert(ConnectInfo(SocketAddr::from(([192, 0, 2, 7], 50000))));
    let (status, _, _) = server.send(remote).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "von einem anderen Rechner ohnehin nicht");
    assert_eq!(scalar(&server, "SELECT count(*) FROM account").await, 0, "bis hierher ist kein Konto entstanden");

    for (fields, why) in [
        ("name=Erika&password=zu-kurz&password2=zu-kurz".to_owned(), "Passwort unter 10 Zeichen"),
        (format!("name=Erika&password={0}&password2={0}", "a".repeat(201)), "Passwort über 200 Zeichen"),
        (format!("name=Erika&password={AUTHOR_PW}&password2={NEW_PW}"), "Wiederholung weicht ab"),
        (format!("name=&password={AUTHOR_PW}&password2={AUTHOR_PW}"), "leerer Name"),
        (format!("name=unbekannt&password={AUTHOR_PW}&password2={AUTHOR_PW}"), "der Name unbekannt"),
        (format!("name={0}&password={AUTHOR_PW}&password2={AUTHOR_PW}", "n".repeat(41)), "Name über 40 Zeichen"),
    ] {
        let (status, headers, body) = server.send(local(form("/einrichten", &fields))).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{why}: 422 mit dem Formular");
        assert!(body.contains("name=\"password2\""), "{why}: das Formular kommt zurück");
        assert!(!body.contains(AUTHOR_PW) && !body.contains("zu-kurz"), "{why}: kein Passwort wird vorbelegt");
        assert!(session_cookies(&headers).is_empty(), "{why}: keine Sitzung");
    }
    assert_eq!(scalar(&server, "SELECT count(*) FROM account").await, 0, "ungültige Eingaben legen kein Konto an");

    let (status, headers, body) = server.send(local(form("/einrichten", &good))).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "vom eigenen Rechner gelingt das Einrichten: {body}");
    assert_eq!(location(&headers), "/einstellungen/konten?ok=konto_angelegt", "weiter zur Seite der Konten");
    let token = token_of(&headers);
    let stored: (String, String, String, String, i64) =
        sqlx::query_as("SELECT name, slug, display, kind, active FROM account")
            .fetch_one(server.state.store.pool())
            .await
            .unwrap();
    assert_eq!(
        stored,
        ("Erika".to_owned(), "erika".to_owned(), "Erika Beispiel".to_owned(), "author".to_owned(), 1),
        "das erste Konto ist der Autor, aktiv, mit Slug nach der Regel"
    );
    match who(&server, with_cookie(get("/"), &token)).await {
        Who::Account(identity) => {
            assert_eq!(identity.kind, AccountKind::Author, "der Einrichtende ist als Autor angemeldet");
            assert_eq!(identity.handle, "@lektorat:erika", "das Handle folgt dem Slug");
        }
        other => panic!("nach dem Einrichten ist der Autor angemeldet, bekommen: {other:?}"),
    }
    // Ein zweites „erstes“ Konto gibt es nicht – weder für den Autor noch für Unangemeldete.
    let again = format!("name=Jonas&password={AUTHOR_PW}&password2={AUTHOR_PW}");
    let (status, _, _) = server.send(with_cookie(local(form("/einrichten", &again)), &token)).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "mit vorhandenem Konto ist Einrichten abgelehnt");
    let (status, _, _) = server.send(local(form("/einrichten", &again))).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "ohne Anmeldung kommt die Anfrage gar nicht bis zum Einrichten");
    assert_eq!(scalar(&server, "SELECT count(*) FROM account").await, 1, "es bleibt bei einem Konto");
}

#[tokio::test]
async fn two_simultaneous_setups_create_exactly_one_account() {
    let server = common::open().await;
    let body = |name: &str| format!("name={name}&password={AUTHOR_PW}&password2={AUTHOR_PW}");
    let (first, second) = tokio::join!(
        server.send(local(form("/einrichten", &body("Erika")))),
        server.send(local(form("/einrichten", &body("Jonas"))))
    );
    let mut statuses = [first.0, second.0];
    statuses.sort();
    assert_eq!(
        statuses,
        [StatusCode::SEE_OTHER, StatusCode::FORBIDDEN],
        "von zwei gleichzeitigen Einrichtungen gelingt genau eine, die andere ist abgelehnt"
    );
    assert_eq!(scalar(&server, "SELECT count(*) FROM account").await, 1, "es gibt genau ein Konto");
    assert_eq!(scalar(&server, "SELECT count(*) FROM account WHERE kind = 'author'").await, 1, "und es ist der Autor");
    assert_eq!(scalar(&server, "SELECT count(*) FROM account_session").await, 1, "nur der Gewinner ist angemeldet");
    let loser = if first.0 == StatusCode::FORBIDDEN { first } else { second };
    assert!(session_cookies(&loser.1).is_empty(), "die abgelehnte Einrichtung bekommt kein Cookie");
}

// ---- mit Konten, ohne Sitzung --------------------------------------------------------------------------------

#[tokio::test]
async fn without_a_session_every_route_asks_for_the_login() {
    let server = common::open().await;
    setup_author(&server).await;
    let placeholder = regex::Regex::new(r"\{[^}]*\}").unwrap();
    let routes = common::registered_routes();
    let mut checked = 0;
    for (method, pattern) in &routes {
        let path = placeholder.replace_all(pattern, "1").into_owned();
        let public = auth::PUBLIC_PATHS.contains(&path.as_str()) || path.starts_with(auth::PUBLIC_PREFIX);
        if public || path == "/mcp" {
            continue;
        }
        for with_peer in [false, true] {
            let request = match (method.as_str(), path.as_str()) {
                ("GET", _) => get(&format!("{path}?work=1&q=x")),
                ("POST", "/rpc") => rpc_request("work.list", json!({})),
                _ => form(&path, "method=post.add&node=0&body=x&action=create&id=1"),
            };
            let request = if with_peer { local(request) } else { request };
            let (status, headers, body) = server.send(request).await;
            if method == "GET" {
                assert_eq!(status, StatusCode::SEE_OTHER, "GET {pattern} ohne Sitzung leitet zur Anmeldung um");
                let expected = format!("/anmelden?next={path}%3Fwork%3D1%26q%3Dx");
                assert_eq!(location(&headers), expected, "GET {pattern}: Ziel der Umleitung mit next");
            } else {
                assert_eq!(status, StatusCode::UNAUTHORIZED, "POST {pattern} ohne Sitzung ist 401: {body}");
            }
            if path == "/rpc" {
                let answer: Value = serde_json::from_str(&body).unwrap();
                assert_eq!(answer["error"]["code"], -32002, "/rpc: JSON-RPC-Fehler „nicht angemeldet“: {answer}");
                assert!(answer["result"].is_null(), "/rpc liefert ohne Sitzung kein Ergebnis");
            }
            assert_eq!(
                headers.get(header::CACHE_CONTROL).map(|v| v.to_str().unwrap()),
                Some("no-store"),
                "{method} {pattern}: eine Abweisung bleibt in keinem Zwischenspeicher"
            );
            assert!(session_cookies(&headers).is_empty(), "{method} {pattern}: eine Abweisung setzt kein Cookie");
            checked += 1;
        }
    }
    assert!(checked >= 250, "alle Routen des Servers sind geprüft (GET und POST, mit und ohne Gegenstelle): {checked}");
    assert_eq!(scalar(&server, "SELECT count(*) FROM account").await, 1, "keine der Anfragen hat ein Konto angelegt");
    assert_eq!(
        scalar(&server, "SELECT count(*) FROM post").await,
        scalar(&common::open().await, "SELECT count(*) FROM post").await,
        "keine der abgewiesenen Formularanfragen hat einen Beitrag geschrieben"
    );
    // Öffentlich bleibt nur, was die Anmeldeseite braucht.
    for path in auth::PUBLIC_PATHS.iter().filter(|path| path.starts_with("/static/")) {
        assert_eq!(server.send(get(path)).await.0, StatusCode::OK, "{path} lädt die Anmeldeseite ohne Sitzung");
    }
    let (status, _, _) = server.send(get("/static/js/gibt-es-nicht.js")).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "unter /static/js/ antwortet der Server selbst (hier: unbekannt)");
}

#[tokio::test]
async fn mcp_stays_with_tokens_and_ignores_accounts() {
    let open = common::open().await;
    let closed = common::open().await;
    setup_author(&closed).await;
    let initialize = || {
        let body = json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
            "protocolVersion": "2025-03-26", "capabilities": {},
            "clientInfo": { "name": "r35-konten-probe", "version": "1" } } });
        Request::post("/mcp")
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::ACCEPT, "application/json, text/event-stream")
            .body(Body::from(body.to_string()))
            .unwrap()
    };
    let remote = |mut request: Request<Body>| {
        request.extensions_mut().insert(ConnectInfo(SocketAddr::from(([192, 0, 2, 7], 50000))));
        request
    };
    for (label, wrap) in [
        ("eigener Rechner", &(|r| local(r)) as &dyn Fn(Request<Body>) -> Request<Body>),
        ("anderer Rechner ohne Token", &remote),
    ] {
        let (status_open, _, body_open) = open.send(wrap(initialize())).await;
        let (status_closed, headers, body_closed) = closed.send(wrap(initialize())).await;
        assert_eq!(status_closed, status_open, "/mcp ({label}) antwortet mit Konten wie ohne: {body_closed}");
        assert_ne!(status_closed, StatusCode::SEE_OTHER, "/mcp ({label}) leitet nie zur Anmeldung um");
        assert!(!body_closed.contains("Anmeldung erforderlich"), "/mcp ({label}) fragt nach Token, nicht nach Konto");
        assert!(session_cookies(&headers).is_empty(), "/mcp ({label}) setzt kein Cookie der Sitzung");
        let is_result = |body: &str| body.contains("\"result\"");
        assert_eq!(is_result(&body_closed), is_result(&body_open), "/mcp ({label}): gleiche Art der Antwort");
    }
    let (status, _, body) = closed.send(local(initialize())).await;
    assert_eq!(status, StatusCode::OK, "vom eigenen Rechner gelingt initialize ohne Anmeldung: {body}");
}

// ---- Anmelden ------------------------------------------------------------------------------------------------

#[tokio::test]
async fn the_right_password_sets_the_cookie_and_wrong_or_unknown_look_the_same() {
    let server = open_behind_proxy().await;
    setup_author(&server).await;

    let direct = local(form("/anmelden", &format!("name=Erika&password={AUTHOR_PW}&next=/")));
    let (status, headers, body) = server.send(direct).await;
    assert_eq!(
        (status, location(&headers)),
        (StatusCode::SEE_OTHER, "/"),
        "richtiges Passwort: weiter zum Ziel: {body}"
    );
    let cookies = session_cookies(&headers);
    let token = token_of(&headers);
    assert_eq!(
        cookies[0],
        format!("baruch_session={token}; Path=/; HttpOnly; SameSite=Strict; Max-Age=2592000"),
        "Cookie wörtlich nach Vertrag §1.2 (direkt am Rechner ohne Secure)"
    );
    assert_eq!(token.len(), 43, "das Token sind 32 Zufallsbytes als Base64url");
    let (status, headers, _) =
        server.send(proxied(form("/anmelden", &format!("name=Erika&password={AUTHOR_PW}&next=/werke")))).await;
    assert_eq!((status, location(&headers)), (StatusCode::SEE_OTHER, "/werke"), "über den Proxy ebenso");
    let proxy_cookie = session_cookies(&headers).remove(0);
    assert!(
        proxy_cookie.ends_with("; HttpOnly; SameSite=Strict; Max-Age=2592000; Secure"),
        "unter dem Proxy-Namen trägt das Cookie zusätzlich Secure: {proxy_cookie}"
    );
    let (_, headers, _) = login(&server, "Erika", AUTHOR_PW).await;
    assert!(
        session_cookies(&headers)[0].ends_with("; Secure"),
        "sind Proxy-Namen erlaubt, gilt auch eine Anfrage ohne Host als weitergereicht: Secure"
    );
    let (status, headers, _) = login(&server, "  eRiKa ", AUTHOR_PW).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "der Name gilt ohne Rücksicht auf Rand und Großschreibung");
    assert!(!session_cookies(&headers).is_empty(), "auch dann mit Sitzung");

    // next wird nur als Pfad dieses Servers befolgt.
    for (next, target) in [
        ("%2F%2Ffremd.example%2Fx", "/"),
        ("https%3A%2F%2Ffremd.example%2F", "/"),
        ("%2F%5Cfremd.example", "/"),
        ("werke", "/"),
        ("", "/"),
        ("%2Fwerke%3Fx%3D1%26y%3D2", "/werke?x=1&y=2"),
        ("%2Fwerke%2F1%2Fbefunde", "/werke/1/befunde"),
    ] {
        let body = format!("name=Erika&password={AUTHOR_PW}&next={next}");
        let (status, headers, _) = server.send(form("/anmelden", &body)).await;
        assert_eq!(status, StatusCode::SEE_OTHER, "next={next}: die Anmeldung gelingt");
        assert_eq!(location(&headers), target, "next={next}: nur ein lokaler Pfad wird befolgt");
    }

    // Falsches Passwort, unbekannter Name, überlanges Passwort: gleicher Status, gleiche Seite, kein Cookie.
    let (wrong_status, wrong_headers, wrong_body) = login(&server, "Erika", "falsches-passwort").await;
    let (unknown_status, unknown_headers, unknown_body) = login(&server, "Nemo", "falsches-passwort").await;
    assert_eq!(wrong_status, StatusCode::UNAUTHORIZED, "falsches Passwort: 401");
    assert_eq!(unknown_status, wrong_status, "ein unbekannter Name bekommt denselben Status");
    assert_eq!(
        unknown_body.replace("Nemo", "NAME"),
        wrong_body.replace("Erika", "NAME"),
        "und bis auf den eingegebenen Namen dieselbe Seite – sie verrät nicht, ob es das Konto gibt"
    );
    assert!(wrong_body.contains("Name oder Passwort stimmt nicht."), "die Meldung des Vertrags: {wrong_body}");
    assert!(!wrong_body.contains("falsches-passwort"), "das eingegebene Passwort steht nicht in der Antwort");
    assert!(wrong_body.contains("name=\"password\""), "das Formular kommt zurück");
    assert!(session_cookies(&wrong_headers).is_empty(), "ein Fehlversuch setzt kein Cookie");
    assert!(session_cookies(&unknown_headers).is_empty(), "auch nicht für einen unbekannten Namen");
    let (status, _, _) = login(&server, "Zoe", &"a".repeat(201)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "ein überlanges Passwort ist schlicht falsch");
    let (status, _, _) = server.send(form("/anmelden", "")).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "ein leeres Formular ist ein Fehlversuch, kein Serverfehler");

    // Mit Sitzung zeigt GET / die Seite, ohne Zwischenspeicher.
    let (status, headers, _) = server.send(with_cookie(get("/"), &token)).await;
    assert_eq!(status, StatusCode::OK, "mit dem Cookie zeigt GET / die Arbeitsseite");
    assert_eq!(
        headers.get(header::CACHE_CONTROL).map(|v| v.to_str().unwrap()),
        Some("no-store"),
        "die Seite eines Angemeldeten bleibt in keinem Zwischenspeicher"
    );
    assert!(session_cookies(&headers).is_empty(), "eine gewöhnliche Anfrage setzt das Cookie nicht neu");
    let (status, headers, _) = server.send(with_cookie(get("/static/app.css"), &token)).await;
    assert_eq!(status, StatusCode::OK, "das Stylesheet lädt");
    assert_eq!(
        headers.get(header::CACHE_CONTROL).map(|v| v.to_str().unwrap()),
        Some("no-cache"),
        "eine Antwort mit eigener Angabe behält sie"
    );
    let me = rpc_as(&server, &token, "account.me", json!({})).await;
    assert_eq!(
        me["result"],
        json!({ "accounts_exist": true, "signed_in": true, "local_open": false, "id": 1, "name": "Erika",
            "display": "Erika Beispiel", "kind": "author", "handle": "@lektorat:erika" }),
        "account.me nennt das angemeldete Konto"
    );
    let last_login: Option<String> =
        sqlx::query_scalar("SELECT last_login FROM account").fetch_one(server.state.store.pool()).await.unwrap();
    assert!(last_login.is_some(), "die Anmeldung ist am Konto vermerkt");
}

// ---- Sperre gegen Durchprobieren -----------------------------------------------------------------------------

#[tokio::test]
async fn repeated_failures_block_the_account_with_a_growing_wait() {
    let server = common::open().await;
    setup_author(&server).await;
    for attempt in 1..=3 {
        let (status, headers, _) = login(&server, "Erika", "falsch-geraten-1").await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "Fehlversuch {attempt}: noch ohne Wartezeit");
        assert!(headers.get(header::RETRY_AFTER).is_none(), "Fehlversuch {attempt}: kein Retry-After");
    }
    let (status, headers, body) = login(&server, "Erika", "falsch-geraten-1").await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS, "der vierte Fehlversuch: 429");
    assert_eq!(headers.get(header::RETRY_AFTER).unwrap(), "2", "mit Retry-After 2 Sekunden");
    assert!(body.contains("name=\"password\""), "die Anmeldeseite kommt zurück");
    let (status, headers, _) = login(&server, "Erika", AUTHOR_PW).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS, "während der Sperre hilft auch das richtige Passwort nicht");
    let retry: u64 = headers.get(header::RETRY_AFTER).unwrap().to_str().unwrap().parse().unwrap();
    assert!((1..=2).contains(&retry), "Retry-After nennt die verbleibende Wartezeit: {retry}");
    assert!(session_cookies(&headers).is_empty(), "während der Sperre entsteht keine Sitzung");
    assert_eq!(
        scalar(&server, "SELECT failures FROM login_attempt WHERE scope = 'konto:erika'").await,
        4,
        "ein Versuch während der Sperre wird nicht geprüft und nicht gezählt"
    );
    let (status, _, _) = login(&server, "eRIKA ", AUTHOR_PW).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS, "eine andere Schreibweise des Namens umgeht die Sperre nicht");

    // Wachsende Wartezeit: 4, 8 … bis höchstens 900 Sekunden.
    for expected in ["4", "8", "16"] {
        expire_blocks(&server).await;
        let (status, headers, _) = login(&server, "Erika", "falsch-geraten-2").await;
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS, "nach Ablauf zählt der nächste Fehlversuch weiter");
        assert_eq!(headers.get(header::RETRY_AFTER).unwrap(), expected, "die Wartezeit verdoppelt sich");
    }
    exec(&server, "UPDATE login_attempt SET failures = 40").await;
    expire_blocks(&server).await;
    let (_, headers, _) = login(&server, "Erika", "falsch-geraten-3").await;
    assert_eq!(headers.get(header::RETRY_AFTER).unwrap(), "900", "höchstens 900 Sekunden, auch nach 41 Versuchen");

    // Erfolg nach Ablauf setzt zurück.
    expire_blocks(&server).await;
    let (status, headers, _) = login(&server, "Erika", AUTHOR_PW).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "nach Ablauf der Sperre gelingt die Anmeldung");
    assert!(!session_cookies(&headers).is_empty(), "mit Sitzung");
    assert_eq!(scalar(&server, "SELECT count(*) FROM login_attempt").await, 0, "der Erfolg setzt den Bereich zurück");
    let (status, _, _) = login(&server, "Erika", "falsch-geraten-4").await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "danach zählt es wieder von vorn (401 ohne Wartezeit)");
}

/// Runde 35, Codereview: Die Sperre zählte erst nach der Prüfung des Passworts; eine Salve gleichzeitiger Versuche
/// lief deshalb vollständig durch die Prüfung, bevor der erste Fehlversuch eingetragen war.
#[tokio::test]
async fn a_salvo_of_simultaneous_guesses_is_checked_no_further_than_guesses_in_a_row() {
    let server = common::open().await;
    let author = setup_author(&server).await;
    let guesses: Vec<String> = (0..12).map(|i| format!("name=Erika&password=geraten-nummer-{i:02}")).collect();
    let mut salvo = tokio::task::JoinSet::new();
    for guess in &guesses {
        let app = server.app.clone();
        let request = form("/anmelden", guess);
        salvo.spawn(async move { tower::ServiceExt::oneshot(app, request).await.unwrap().status() });
    }
    let mut statuses = Vec::new();
    while let Some(status) = salvo.join_next().await {
        statuses.push(status.unwrap());
    }
    assert_eq!(
        statuses.iter().filter(|status| **status == StatusCode::UNAUTHORIZED).count(),
        3,
        "nur drei der zwölf gleichzeitigen Versuche sind frei (401): {statuses:?}"
    );
    assert_eq!(
        statuses.iter().filter(|status| **status == StatusCode::TOO_MANY_REQUESTS).count(),
        9,
        "der vierte löst die Sperre aus, die übrigen acht sind abgewiesen (429): {statuses:?}"
    );
    assert_eq!(
        scalar(&server, "SELECT failures FROM login_attempt WHERE scope = 'konto:erika'").await,
        4,
        "nur vier Passwörter kamen bis zur Prüfung – wer während der Sperre ankommt, wird nicht geprüft und nicht \
         gezählt, auch wenn die Prüfung der ersten noch läuft"
    );
    let (status, _, _) = login(&server, "Erika", AUTHOR_PW).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS, "nach der Salve ist das Konto gesperrt");

    // Dasselbe beim bisherigen Passwort des Passwortwechsels (Bereich des Kontos).
    exec(&server, "DELETE FROM login_attempt").await;
    let mut salvo = tokio::task::JoinSet::new();
    for i in 0..8 {
        let app = server.app.clone();
        let fields = format!("current=geraten-nummer-{i:02}&password={NEW_PW}&password2={NEW_PW}");
        let request = with_cookie(form("/konto/passwort", &fields), &author);
        salvo.spawn(async move { tower::ServiceExt::oneshot(app, request).await.unwrap().status() });
    }
    while let Some(status) = salvo.join_next().await {
        assert_eq!(status.unwrap(), StatusCode::SEE_OTHER, "jede Änderung antwortet mit der Umleitung auf /konto");
    }
    assert_eq!(
        scalar(&server, "SELECT failures FROM login_attempt WHERE scope = 'konto:erika'").await,
        4,
        "auch beim Passwortwechsel kommen von acht gleichzeitigen Versuchen nur vier bis zur Prüfung"
    );
    expire_blocks(&server).await;
    let (status, _, _) = login(&server, "Erika", AUTHOR_PW).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "kein geratenes Passwort hat das Konto geändert: das alte gilt noch");
    assert_eq!(
        scalar(&server, "SELECT count(*) FROM login_attempt WHERE scope = 'konto:erika'").await,
        0,
        "die Reservierung des gelungenen Versuchs ist zurückgenommen"
    );
}

#[tokio::test]
async fn unknown_names_and_addresses_are_counted_too() {
    let server = open_behind_proxy().await;
    setup_author(&server).await;
    // Unbekannter Name: dieselbe Sperre wie für ein vorhandenes Konto.
    for attempt in 1..=3 {
        let (status, _, _) = login(&server, "Niemand", "falsch-geraten-1").await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "unbekannter Name, Fehlversuch {attempt}: 401");
    }
    let (status, headers, _) = login(&server, "Niemand", "falsch-geraten-1").await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS, "auch ein unbekannter Name ist ab dem vierten Versuch gesperrt");
    assert_eq!(headers.get(header::RETRY_AFTER).unwrap(), "2", "mit derselben Wartezeit – kein Unterschied zum Konto");
    assert_eq!(
        scalar(&server, "SELECT count(*) FROM login_attempt WHERE scope = 'konto:niemand'").await,
        1,
        "der Bereich des unbekannten Namens zählt mit"
    );
    assert_eq!(
        scalar(&server, "SELECT count(*) FROM login_attempt WHERE scope LIKE 'adresse:%'").await,
        0,
        "ohne bekannte Gegenstelle gibt es keinen Bereich der Adresse"
    );
    let (status, _, _) = login(&server, "Erika", AUTHOR_PW).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "ein anderes Konto ist davon nicht gesperrt");

    // Adresse: vier Fehlversuche mit vier Namen von derselben Gegenstelle sperren die Adresse.
    exec(&server, "DELETE FROM login_attempt").await;
    for name in ["Ada", "Bela", "Cleo", "Dana"] {
        let body = format!("name={name}&password=falsch-geraten-2");
        server.send(local(form("/anmelden", &body))).await;
    }
    assert_eq!(
        scalar(&server, "SELECT failures FROM login_attempt WHERE scope = 'adresse:127.0.0.1'").await,
        4,
        "die Adresse der Gegenstelle zählt über alle Namen"
    );
    let body = format!("name=Erika&password={AUTHOR_PW}");
    let (status, _, _) = server.send(local(form("/anmelden", &body))).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS, "von der gesperrten Adresse geht auch ein anderes Konto nicht");
    let (status, _, _) = server.send(proxied(form("/anmelden", &body))).await;
    assert_eq!(
        status,
        StatusCode::SEE_OTHER,
        "über den Proxy zählt die Adresse nicht (dort trügen alle die Adresse des Proxys) – das Konto ist frei"
    );
    exec(&server, "DELETE FROM login_attempt").await;
    for name in ["Ada", "Bela", "Cleo", "Dana", "Egon"] {
        let body = format!("name={name}&password=falsch-geraten-3");
        let (status, _, _) = server.send(proxied(form("/anmelden", &body))).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "über den Proxy sperren fünf Namen einander nicht ({name})");
    }
    assert_eq!(
        scalar(&server, "SELECT count(*) FROM login_attempt WHERE scope LIKE 'adresse:%'").await,
        0,
        "über den Proxy entsteht kein Bereich der Adresse"
    );

    // Volle Tabelle: Sie wächst nicht über 1000 Zeilen, neue Namen warten gemeinsam.
    exec(&server, "DELETE FROM login_attempt").await;
    exec(
        &server,
        "WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM n WHERE i < 999) \
         INSERT INTO login_attempt (scope, failures) SELECT 'konto:flut' || i, 1 FROM n",
    )
    .await;
    let (status, headers, _) = login(&server, "Frisch", "falsch-geraten-4").await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS, "bei voller Tabelle wartet ein neuer Name sofort");
    assert_eq!(headers.get(header::RETRY_AFTER).unwrap(), "2", "fest zwei Sekunden");
    let (status, _, _) = login(&server, "Nochmal", "falsch-geraten-4").await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS, "der Bereich `alle` sperrt in dieser Zeit jeden Versuch");
    assert_eq!(scalar(&server, "SELECT count(*) FROM login_attempt").await, 1000, "höchstens 1000 Zeilen");
    assert_eq!(
        scalar(&server, "SELECT count(*) FROM login_attempt WHERE scope IN ('konto:frisch', 'konto:nochmal')").await,
        0,
        "die neuen Namen bekamen keine eigene Zeile"
    );
}

// ---- Sitzung -------------------------------------------------------------------------------------------------

#[tokio::test]
async fn the_database_keeps_only_the_hash_and_logout_ends_the_session() {
    let server = common::open().await;
    let token = setup_author(&server).await;
    let rows: Vec<(String, i64)> = sqlx::query_as("SELECT token_hash, account_id FROM account_session")
        .fetch_all(server.state.store.pool())
        .await
        .unwrap();
    assert_eq!(rows.len(), 1, "eine Sitzung");
    assert_eq!(rows[0].0, baruch_store::Store::session_token_hash(&token), "gespeichert ist der SHA-256 des Tokens");
    assert_eq!(rows[0].0.len(), 64, "als 64 Hex-Zeichen");
    assert_ne!(rows[0].0, token, "nie das Token selbst");
    let dump: Vec<String> = sqlx::query_scalar(
        "SELECT name || '|' || password || '|' || slug FROM account \
         UNION ALL SELECT token_hash FROM account_session UNION ALL SELECT key || '=' || value FROM store_meta",
    )
    .fetch_all(server.state.store.pool())
    .await
    .unwrap();
    for row in &dump {
        assert!(!row.contains(&token), "das Token steht in keiner Zeile der Datenbank");
        assert!(!row.contains(AUTHOR_PW), "das Passwort steht in keiner Zeile der Datenbank");
    }
    let phc: String =
        sqlx::query_scalar("SELECT password FROM account").fetch_one(server.state.store.pool()).await.unwrap();
    assert!(
        phc.starts_with("$argon2id$v=19$m=19456,t=2,p=1$"),
        "gespeichert ist ein Argon2id-PHC-Text mit m = 19456, t = 2, p = 1"
    );
    assert!(auth::password::verify(AUTHOR_PW, &phc), "er passt zum Passwort");

    assert!(matches!(who(&server, with_cookie(get("/"), &token)).await, Who::Account(_)), "die Sitzung gilt");
    assert_eq!(who(&server, get("/")).await, Who::Anonymous, "ohne Cookie ist niemand angemeldet");
    let forged = "A".repeat(43);
    assert_eq!(who(&server, with_cookie(get("/"), &forged)).await, Who::Anonymous, "ein erfundenes Token gilt nicht");
    let hash = baruch_store::Store::session_token_hash(&token);
    assert_eq!(
        who(&server, with_cookie(get("/"), &hash)).await,
        Who::Anonymous,
        "wer nur die Datenbank kennt (den Hash), kann sich damit nicht anmelden"
    );

    let (status, headers, _) = server.send(with_cookie(form("/abmelden", ""), &token)).await;
    assert_eq!(
        (status, location(&headers)),
        (StatusCode::SEE_OTHER, "/anmelden?ok=abgemeldet"),
        "Abmelden führt zur Anmeldeseite"
    );
    assert_eq!(
        session_cookies(&headers),
        ["baruch_session=; Path=/; HttpOnly; SameSite=Strict; Max-Age=0"],
        "und löscht das Cookie im Browser"
    );
    assert_eq!(scalar(&server, "SELECT count(*) FROM account_session").await, 0, "die Sitzung ist gelöscht");
    assert_eq!(
        who(&server, with_cookie(get("/"), &token)).await,
        Who::Anonymous,
        "das alte Cookie gilt sofort nicht mehr"
    );
    let (status, headers, _) = server.send(with_cookie(get("/werke"), &token)).await;
    assert_eq!(
        (status, location(&headers)),
        (StatusCode::SEE_OTHER, "/anmelden?next=/werke"),
        "eine Seite mit dem alten Cookie leitet zur Anmeldung um"
    );
}

#[tokio::test]
async fn expired_sessions_do_not_count_and_an_idle_one_gets_a_fresh_cookie() {
    let server = open_behind_proxy().await;
    let token = setup_author(&server).await;
    let set = |column: &str, modifier: &str| {
        format!("UPDATE account_session SET {column} = strftime('%Y-%m-%dT%H:%M:%fZ','now','{modifier}')")
    };
    let valid = |server: &Server| {
        let request = with_cookie(get("/"), &token);
        let state = server.state.clone();
        async move {
            let (parts, _) = request.into_parts();
            matches!(auth::gate::resolve(&state, &parts).await, Who::Account(_))
        }
    };
    exec(&server, &set("last_seen", "-29 days")).await;
    assert!(valid(&server).await, "29 Tage ohne Benutzung: die Sitzung gilt noch");
    exec(&server, &set("last_seen", "-31 days")).await;
    assert!(!valid(&server).await, "31 Tage ohne Benutzung: abgelaufen");
    exec(&server, &set("last_seen", "-1 minutes")).await;
    exec(&server, &set("created", "-89 days")).await;
    assert!(valid(&server).await, "89 Tage alt, kürzlich benutzt: gilt noch");
    exec(&server, &set("created", "-91 days")).await;
    assert!(!valid(&server).await, "91 Tage alt: abgelaufen, auch bei täglicher Benutzung");
    let (status, _, _) = server.send(with_cookie(get("/"), &token)).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "mit abgelaufener Sitzung leitet die Seite zur Anmeldung um");

    // Erneuern: Nach einer Pause von mehr als einer Stunde bekommt der Browser das Cookie mit frischer Frist.
    exec(&server, &set("created", "-10 days")).await;
    exec(&server, &set("last_seen", "-2 hours")).await;
    let (status, headers, _) = server.send(with_cookie(local(get("/")), &token)).await;
    assert_eq!(status, StatusCode::OK, "die Sitzung gilt");
    assert_eq!(
        session_cookies(&headers),
        [format!("baruch_session={token}; Path=/; HttpOnly; SameSite=Strict; Max-Age=2592000")],
        "nach längerer Pause kommt dasselbe Token mit neuer Lebensdauer zurück"
    );
    let (_, headers, _) = server.send(with_cookie(local(get("/")), &token)).await;
    assert!(session_cookies(&headers).is_empty(), "gleich danach nicht noch einmal");
    exec(&server, &set("last_seen", "-2 hours")).await;
    let (_, headers, _) = server.send(with_cookie(proxied(get("/")), &token)).await;
    assert!(
        session_cookies(&headers).first().is_some_and(|cookie| cookie.ends_with("; Secure")),
        "über den Proxy wird auch das erneuerte Cookie mit Secure gesetzt: {:?}",
        session_cookies(&headers)
    );
    exec(&server, &set("last_seen", "-2 hours")).await;
    let (_, headers, _) = server.send(with_cookie(local(form("/abmelden", "")), &token)).await;
    assert_eq!(
        session_cookies(&headers),
        ["baruch_session=; Path=/; HttpOnly; SameSite=Strict; Max-Age=0"],
        "Abmelden nach einer Pause löscht das Cookie – das Erneuern setzt es nicht wieder"
    );
}

#[tokio::test]
async fn a_password_change_ends_the_other_sessions_and_a_lock_ends_all() {
    let server = common::open().await;
    let first = setup_author(&server).await;
    let second = sign_in(&server, "Erika", AUTHOR_PW).await;
    let valid = |token: &str| {
        let request = with_cookie(get("/"), token);
        let server = &server;
        async move { matches!(who(server, request).await, Who::Account(_)) }
    };
    assert!(valid(&first).await && valid(&second).await, "Vorbedingung: zwei Sitzungen des Autors");

    // Abgelehnte Änderungen lassen alles, wie es ist.
    for (fields, code) in [
        (format!("current=falsches-passwort&password={NEW_PW}&password2={NEW_PW}"), "passwort_falsch"),
        (format!("current={AUTHOR_PW}&password=zu-kurz&password2=zu-kurz"), "passwort_kurz"),
        (format!("current={AUTHOR_PW}&password={0}&password2={0}", "b".repeat(201)), "passwort_lang"),
        (format!("current={AUTHOR_PW}&password={NEW_PW}&password2={LEKTOR_PW}"), "passwort_ungleich"),
    ] {
        let (status, headers, _) = server.send(with_cookie(form("/konto/passwort", &fields), &first)).await;
        assert_eq!(
            (status, location(&headers)),
            (StatusCode::SEE_OTHER, format!("/konto?fehler={code}").as_str()),
            "abgelehnt mit dem Code {code}"
        );
        assert!(valid(&second).await, "{code}: eine abgelehnte Änderung beendet keine Sitzung");
    }
    exec(&server, "DELETE FROM login_attempt").await;
    let fields = format!("current={AUTHOR_PW}&password={NEW_PW}&password2={NEW_PW}");
    let (status, headers, _) = server.send(with_cookie(form("/konto/passwort", &fields), &first)).await;
    assert_eq!(
        (status, location(&headers)),
        (StatusCode::SEE_OTHER, "/konto?ok=passwort_geaendert"),
        "mit dem bisherigen Passwort gelingt die Änderung"
    );
    assert!(valid(&first).await, "die Sitzung, die das Passwort geändert hat, bleibt");
    assert!(!valid(&second).await, "jede andere Sitzung des Kontos endet sofort");
    assert_eq!(
        login(&server, "Erika", AUTHOR_PW).await.0,
        StatusCode::UNAUTHORIZED,
        "das alte Passwort gilt nicht mehr"
    );
    let author = sign_in(&server, "Erika", NEW_PW).await;
    let (status, headers, _) = server
        .send(form("/konto/passwort", &format!("current={NEW_PW}&password={AUTHOR_PW}&password2={AUTHOR_PW}")))
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "ohne Sitzung lässt sich kein Passwort ändern");
    assert!(location(&headers).is_empty(), "und es gibt keine Umleitung mit Erfolgsmeldung");

    // Sperren beendet alle Sitzungen des Kontos; Entsperren bringt sie nicht zurück.
    let lea = create_lektor(&server, &author, "Lea", LEKTOR_PW).await;
    let lektor = sign_in(&server, "Lea", LEKTOR_PW).await;
    match who(&server, with_cookie(get("/"), &lektor)).await {
        Who::Account(identity) => {
            assert_eq!(identity.kind, AccountKind::Lektor, "ein weiteres Konto ist Lektor");
            assert_eq!(identity.handle, "@lektorat:lea", "mit dem Handle seines Namens");
        }
        other => panic!("der Lektor ist angemeldet, bekommen: {other:?}"),
    }
    let (status, headers, _) = server
        .send(with_cookie(form("/einstellungen/konten/act", &format!("action=deactivate&id={lea}")), &author))
        .await;
    assert_eq!(
        (status, location(&headers)),
        (StatusCode::SEE_OTHER, "/einstellungen/konten?ok=konto_gesperrt"),
        "der Autor sperrt den Lektor"
    );
    assert!(!valid(&lektor).await, "die Sitzung des gesperrten Kontos gilt bei der nächsten Anfrage nicht mehr");
    let (status, _, body) = login(&server, "Lea", LEKTOR_PW).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "ein gesperrtes Konto kann sich nicht anmelden");
    assert!(
        body.contains("Name oder Passwort stimmt nicht."),
        "mit derselben Meldung wie bei falschem Passwort – die Antwort verrät die Sperre nicht"
    );
    let (_, headers, _) = server
        .send(with_cookie(form("/einstellungen/konten/act", &format!("action=activate&id={lea}")), &author))
        .await;
    assert_eq!(location(&headers), "/einstellungen/konten?ok=konto_entsperrt", "der Autor entsperrt den Lektor");
    assert!(!valid(&lektor).await, "die alte Sitzung bleibt beendet");
    exec(&server, "DELETE FROM login_attempt").await;
    assert_eq!(login(&server, "Lea", LEKTOR_PW).await.0, StatusCode::SEE_OTHER, "neu anmelden geht wieder");
}

// ---- Ausnahme „Am eigenen Rechner ohne Anmeldung“ ---------------------------------------------------------------

#[tokio::test]
async fn the_own_machine_needs_the_switch_and_no_proxy_header() {
    let server = open_behind_proxy().await;
    let author = setup_author(&server).await;
    assert_eq!(who(&server, local(get("/"))).await, Who::Anonymous, "Vorgabe: auch am eigenen Rechner anmelden");
    let (status, headers, _) = server.send(local(get("/"))).await;
    assert_eq!(
        (status, location(&headers)),
        (StatusCode::SEE_OTHER, "/anmelden?next=/"),
        "ohne den Schalter leitet auch der eigene Rechner zur Anmeldung um"
    );

    let (status, headers, _) =
        server.send(with_cookie(form("/einstellungen/konten/act", "action=local_open&open=1"), &author)).await;
    assert_eq!(
        (status, location(&headers)),
        (StatusCode::SEE_OTHER, "/einstellungen/konten?ok=lokal_offen"),
        "der Autor schaltet die Ausnahme ein"
    );
    assert_eq!(
        who(&server, local(get("/"))).await,
        Who::LocalAuthor,
        "eingeschaltet: Loopback ohne Proxy ist der Autor"
    );
    assert!(Who::LocalAuthor.full_rights(), "mit vollen Rechten");
    let localhost = with_header(local(get("/")), "host", "localhost:8765");
    assert_eq!(who(&server, localhost).await, Who::LocalAuthor, "auch unter dem Namen localhost");
    let (status, headers, _) = server.send(local(get("/"))).await;
    assert_eq!(status, StatusCode::OK, "die Seite kommt ohne Anmeldung");
    assert!(session_cookies(&headers).is_empty(), "die Ausnahme setzt kein Cookie");
    assert_eq!(
        headers.get(header::CACHE_CONTROL).map(|v| v.to_str().unwrap()),
        Some("no-store"),
        "auch diese Seite bleibt in keinem Zwischenspeicher"
    );

    for name in ["forwarded", "x-forwarded-for", "x-forwarded-host", "x-forwarded-proto", "x-real-ip", "via"] {
        for value in ["127.0.0.1", ""] {
            let request = with_header(local(get("/")), name, value);
            assert_eq!(
                who(&server, request).await,
                Who::Anonymous,
                "mit der Kopfzeile {name} (Wert {value:?}) gilt die Ausnahme nicht – das war ein Proxy"
            );
        }
        let (status, _, _) = server.send(with_header(local(get("/")), name, "127.0.0.1")).await;
        assert_eq!(status, StatusCode::SEE_OTHER, "{name}: die Seite verlangt dann die Anmeldung");
    }
    assert_eq!(who(&server, proxied(get("/"))).await, Who::Anonymous, "unter dem Proxy-Namen gilt sie nicht");
    assert_eq!(who(&server, get("/")).await, Who::Anonymous, "ohne bekannte Gegenstelle gilt sie nicht");
    let mut no_host = get("/");
    no_host.extensions_mut().insert(ConnectInfo(LOOPBACK));
    assert_eq!(
        who(&server, no_host).await,
        Who::Anonymous,
        "sind Proxy-Namen erlaubt, gilt auch eine Anfrage ohne Host nicht als eigener Rechner"
    );
    let mut remote = get("/");
    remote.extensions_mut().insert(ConnectInfo(SocketAddr::from(([192, 0, 2, 7], 50000))));
    assert_eq!(who(&server, remote).await, Who::Anonymous, "von einem anderen Rechner gilt sie nicht");
    let mut mapped = with_header(get("/"), "host", "127.0.0.1:8765");
    mapped.extensions_mut().insert(ConnectInfo("[::ffff:127.0.0.1]:50000".parse::<SocketAddr>().unwrap()));
    assert_eq!(who(&server, mapped).await, Who::LocalAuthor, "Loopback über einen Socket für beide Familien zählt");
    assert!(
        matches!(who(&server, with_cookie(proxied(get("/")), &author)).await, Who::Account(_)),
        "eine Sitzung gilt weiter – über den Proxy meldet sich der Autor an"
    );
    assert!(
        matches!(who(&server, with_cookie(local(get("/")), &author)).await, Who::Account(_)),
        "mit Sitzung zählt das Konto, nicht die Ausnahme"
    );

    let me = server.send(local(rpc_request("account.me", json!({})))).await;
    let me: Value = serde_json::from_str(&me.2).unwrap();
    assert_eq!(
        me["result"],
        json!({ "accounts_exist": true, "signed_in": false, "local_open": true }),
        "account.me am eigenen Rechner: ohne Konto, aber offen"
    );
    let (_, headers, _) = server.send(local(form("/einstellungen/konten/act", "action=local_open&open=0"))).await;
    assert_eq!(location(&headers), "/einstellungen/konten?ok=lokal_anmelden", "wieder ausschalten (hier ohne Sitzung)");
    assert_eq!(who(&server, local(get("/"))).await, Who::Anonymous, "ausgeschaltet gilt sofort wieder die Anmeldung");
}

// ---- Konten verwalten ----------------------------------------------------------------------------------------

#[tokio::test]
async fn further_accounts_are_always_lektor_and_the_last_author_stays() {
    let server = common::open().await;
    let author = setup_author(&server).await;
    let act = |fields: String| {
        let request = with_cookie(form("/einstellungen/konten/act", &fields), &author);
        let server = &server;
        async move {
            let (status, headers, body) = server.send(request).await;
            assert_eq!(status, StatusCode::SEE_OTHER, "die Verwaltung antwortet mit einer Umleitung: {body}");
            location(&headers).trim_start_matches("/einstellungen/konten?").to_owned()
        }
    };
    assert_eq!(act("action=deactivate&id=1".into()).await, "fehler=letzter_autor", "der letzte Autor bleibt aktiv");
    assert_eq!(scalar(&server, "SELECT active FROM account WHERE id = 1").await, 1, "das Konto ist weiter aktiv");
    assert!(
        matches!(who(&server, with_cookie(get("/"), &author)).await, Who::Account(_)),
        "und seine Sitzung gilt weiter"
    );

    let create = |name: &str| format!("action=create&name={name}&password={LEKTOR_PW}&password2={LEKTOR_PW}");
    assert_eq!(act(format!("{}&kind=author", create("Lea"))).await, "ok=konto_angelegt", "ein weiteres Konto");
    assert_eq!(
        scalar(&server, "SELECT count(*) FROM account WHERE name = 'Lea' AND kind = 'lektor' AND slug = 'lea'").await,
        1,
        "es ist Lektor – auch wenn das Formular kind=author mitschickt"
    );
    assert_eq!(act(create("LEA")).await, "fehler=name_vergeben", "derselbe Name in anderer Schreibweise");
    assert_eq!(
        act(create("Lea%21")).await,
        "fehler=name_vergeben",
        "ein anderer Name mit demselben Handle (Lea! → lea)"
    );
    assert_eq!(act(create("Unbekannt")).await, "fehler=name_ungueltig", "unbekannt ist als Kontoname abgelehnt");
    assert_eq!(act(create("")).await, "fehler=name_ungueltig", "ein leerer Name");
    assert_eq!(
        act("action=create&name=Mia&password=zu-kurz&password2=zu-kurz".into()).await,
        "fehler=passwort_kurz",
        "auch für Lektoren gilt die Mindestlänge"
    );
    assert_eq!(
        act(format!("action=create&name=Mia&password={LEKTOR_PW}&password2={NEW_PW}")).await,
        "fehler=passwort_ungleich",
        "die Wiederholung muss stimmen"
    );
    assert_eq!(scalar(&server, "SELECT count(*) FROM account").await, 2, "abgelehnte Eingaben legen nichts an");
    assert_eq!(act("action=deactivate&id=99".into()).await, "fehler=unbekanntes_konto", "ein Konto, das es nicht gibt");
    assert_eq!(act("action=deactivate".into()).await, "fehler=unbekanntes_konto", "ohne id");

    // Passwort eines Lektors zurücksetzen: seine Sitzungen enden, das neue Passwort gilt.
    let lektor = sign_in(&server, "Lea", LEKTOR_PW).await;
    assert_eq!(
        act(format!("action=reset_password&id=2&password={NEW_PW}&password2={NEW_PW}")).await,
        "ok=passwort_geaendert",
        "der Autor setzt das Passwort des Lektors"
    );
    assert_eq!(who(&server, with_cookie(get("/"), &lektor)).await, Who::Anonymous, "die Sitzung des Lektors endet");
    assert_eq!(login(&server, "Lea", LEKTOR_PW).await.0, StatusCode::UNAUTHORIZED, "das alte Passwort gilt nicht mehr");
    exec(&server, "DELETE FROM login_attempt").await;
    let lektor = sign_in(&server, "Lea", NEW_PW).await;
    assert_eq!(
        act(format!("action=reset_password&id=1&password={NEW_PW}&password2={NEW_PW}")).await,
        "ok=passwort_geaendert",
        "der Autor setzt sein eigenes Passwort zurück"
    );
    assert!(
        matches!(who(&server, with_cookie(get("/"), &author)).await, Who::Account(_)),
        "dabei bleibt die Sitzung, in der er es tut"
    );

    // Ein Lektor verwaltet keine Konten: weder über das Formular noch über RPC.
    let (status, _, _) =
        server.send(with_cookie(form("/einstellungen/konten/act", "action=deactivate&id=1"), &lektor)).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "das Formular der Konten ist für einen Lektor gesperrt");
    let Who::Account(identity) = who(&server, with_cookie(get("/"), &lektor)).await else {
        panic!("der Lektor ist angemeldet");
    };
    let as_lektor = Who::Account(identity);
    for (method, params) in [
        ("account.list", json!({})),
        ("account.create", json!({ "name": "Mia", "password": LEKTOR_PW })),
        ("account.set_active", json!({ "id": 1, "active": false })),
        ("account.set_local_open", json!({ "open": true })),
        ("account.set_password", json!({ "id": 1, "password": LEKTOR_PW })),
    ] {
        let result = auth::scope(as_lektor.clone(), auth::rpc::call(&server.state, method, params)).await;
        let error = result.expect_err("ein Lektor darf das nicht");
        assert_eq!(error.code, baruch_server::rpc::FORBIDDEN, "{method}: -32003 für einen Lektor");
        assert!(error.message.contains("nur der Autor"), "{method}: die Meldung nennt den Grund: {}", error.message);
    }
    assert_eq!(scalar(&server, "SELECT count(*) FROM account WHERE active = 1").await, 2, "nichts davon hat gewirkt");
    assert!(!server.state.store.auth_local_open().await.unwrap(), "der Schalter steht weiter auf aus");
    let me = auth::scope(as_lektor.clone(), auth::rpc::call(&server.state, "account.me", json!({}))).await.unwrap();
    assert_eq!((&me["kind"], &me["handle"]), (&json!("lektor"), &json!("@lektorat:lea")), "account.me darf jeder");
    // Ein Lektor ändert sein eigenes Passwort (mit dem bisherigen) und beendet seine Sitzungen.
    let wrong = json!({ "current": "falsches-passwort", "password": LEKTOR_PW });
    let error = auth::scope(as_lektor.clone(), auth::rpc::call(&server.state, "account.set_password", wrong))
        .await
        .expect_err("mit falschem bisherigem Passwort geht es nicht");
    assert_eq!(error.code, baruch_server::rpc::REFUSED, "falsches bisheriges Passwort: fachlich abgelehnt");
    let right = json!({ "current": NEW_PW, "password": LEKTOR_PW });
    auth::scope(as_lektor.clone(), auth::rpc::call(&server.state, "account.set_password", right))
        .await
        .expect("mit dem bisherigen Passwort ändert ein Lektor sein eigenes");
    exec(&server, "DELETE FROM login_attempt").await;
    let lektor = sign_in(&server, "Lea", LEKTOR_PW).await;
    let ended = auth::scope(as_lektor, auth::rpc::call(&server.state, "account.logout_all", json!({}))).await.unwrap();
    assert_eq!(ended["sessions"], 1, "account.logout_all beendet alle Sitzungen des eigenen Kontos");
    assert_eq!(who(&server, with_cookie(get("/"), &lektor)).await, Who::Anonymous, "auch die gerade benutzte");
}

#[tokio::test]
async fn the_author_manages_accounts_over_rpc_and_no_answer_carries_a_secret() {
    let server = common::open().await;
    let author = setup_author(&server).await;
    let mut answers: Vec<String> = Vec::new();
    let mut call = async |method: &str, params: Value| {
        let answer = rpc_as(&server, &author, method, params).await;
        answers.push(answer.to_string());
        answer
    };

    let created = call("account.create", json!({ "name": "Lea", "display": "Lea L.", "password": LEKTOR_PW })).await;
    assert_eq!(
        (&created["result"]["kind"], &created["result"]["handle"], &created["result"]["active"]),
        (&json!("lektor"), &json!("@lektorat:lea"), &json!(true)),
        "account.create legt einen Lektor an: {created}"
    );
    let lea = created["result"]["id"].as_i64().unwrap();
    let list = call("account.list", json!({})).await;
    let names: Vec<&str> =
        list["result"]["accounts"].as_array().unwrap().iter().map(|a| a["name"].as_str().unwrap()).collect();
    assert_eq!(names, ["Erika", "Lea"], "account.list nennt die Konten nach id");
    for (params, code, why) in [
        (json!({ "name": "Lea", "password": LEKTOR_PW }), -32000, "Name vergeben"),
        (json!({ "name": "Mia", "password": "zu-kurz" }), -32000, "Passwort zu kurz"),
        (json!({ "name": "unbekannt", "password": LEKTOR_PW }), -32000, "Name unbekannt"),
        (json!({ "name": "Mia" }), -32602, "Passwort fehlt"),
        (json!({ "name": "Mia", "password": ["falscher-typ-geheim"] }), -32602, "Passwort ist kein Text"),
    ] {
        let answer = call("account.create", params).await;
        assert_eq!(answer["error"]["code"], code, "account.create, {why}: {answer}");
    }
    let locked = call("account.set_active", json!({ "id": lea, "active": false })).await;
    assert_eq!(locked["result"]["active"], false, "account.set_active sperrt: {locked}");
    let last = call("account.set_active", json!({ "id": 1, "active": false })).await;
    assert_eq!(last["error"]["code"], -32000, "das letzte aktive Autor-Konto bleibt: {last}");
    let missing = call("account.set_active", json!({ "id": 99, "active": false })).await;
    assert_eq!(missing["error"]["code"], -32000, "unbekanntes Konto: {missing}");
    call("account.set_active", json!({ "id": lea, "active": true })).await;
    let open = call("account.set_local_open", json!({ "open": true })).await;
    assert_eq!(open["result"]["local_open"], true, "account.set_local_open schaltet ein: {open}");
    assert!(server.state.store.auth_local_open().await.unwrap(), "der Schalter steht in der Datenbank");
    call("account.set_local_open", json!({ "open": false })).await;

    // Fremdes Passwort setzen (Autor), eigenes nur mit dem bisherigen.
    let lektor = sign_in(&server, "Lea", LEKTOR_PW).await;
    let reset = call("account.set_password", json!({ "id": lea, "password": NEW_PW })).await;
    assert_eq!(reset["result"], json!({ "id": lea, "changed": true }), "der Autor setzt das Passwort des Lektors");
    assert_eq!(who(&server, with_cookie(get("/"), &lektor)).await, Who::Anonymous, "dessen Sitzung endet");
    let without = call("account.set_password", json!({ "password": NEW_PW })).await;
    assert_eq!(without["error"]["code"], -32602, "eigenes Passwort ohne current: falsche Parameter");
    let own_id = call("account.set_password", json!({ "id": 1, "password": NEW_PW })).await;
    assert_eq!(own_id["error"]["code"], -32602, "auch mit der eigenen id braucht es das bisherige Passwort");
    let wrong = call("account.set_password", json!({ "current": "falsches-passwort", "password": NEW_PW })).await;
    assert_eq!(wrong["error"]["code"], -32000, "falsches bisheriges Passwort: abgelehnt");
    let second = sign_in(&server, "Erika", AUTHOR_PW).await;
    let own = call("account.set_password", json!({ "current": AUTHOR_PW, "password": NEW_PW })).await;
    assert_eq!(own["result"], json!({ "id": 1, "changed": true }), "mit dem bisherigen Passwort gelingt es: {own}");
    assert!(
        matches!(who(&server, with_cookie(get("/"), &author)).await, Who::Account(_)),
        "die Sitzung, die über /rpc das Passwort ändert, bleibt"
    );
    assert_eq!(who(&server, with_cookie(get("/"), &second)).await, Who::Anonymous, "die andere Sitzung endet");
    let me = call("account.me", json!({})).await;
    assert_eq!(me["result"]["signed_in"], true, "account.me: angemeldet");
    let third = sign_in(&server, "Erika", NEW_PW).await;
    let ended = call("account.logout_all", json!({})).await;
    assert_eq!(ended["result"]["sessions"], 2, "account.logout_all beendet beide Sitzungen des Autors: {ended}");
    assert_eq!(who(&server, with_cookie(get("/"), &third)).await, Who::Anonymous, "danach gilt keine mehr");

    // Kein Geheimnis in irgendeiner Antwort von account.*.
    let hashes: Vec<String> =
        sqlx::query_scalar("SELECT password FROM account").fetch_all(server.state.store.pool()).await.unwrap();
    assert_eq!(answers.len(), 20, "alle Antworten von account.* sind gesammelt");
    for answer in &answers {
        for secret in [AUTHOR_PW, LEKTOR_PW, NEW_PW, "zu-kurz", "falsches-passwort", "falscher-typ-geheim"] {
            assert!(!answer.contains(secret), "eine Antwort von account.* nennt ein Passwort: {answer}");
        }
        for token in [&author, &lektor, &second, &third] {
            assert!(!answer.contains(token.as_str()), "eine Antwort von account.* nennt ein Token");
        }
        assert!(!answer.contains("$argon2"), "eine Antwort von account.* nennt einen Hash: {answer}");
        for hash in &hashes {
            assert!(!answer.contains(hash.as_str()), "eine Antwort von account.* nennt einen gespeicherten Hash");
        }
        assert!(!answer.contains("\"password"), "kein Feld namens password steht in einer Antwort: {answer}");
    }
    // Debug-Ausgaben der Typen, die durch die Anfrage wandern.
    let Who::Account(identity) = who(&server, with_cookie(get("/"), &sign_in(&server, "Erika", NEW_PW).await)).await
    else {
        panic!("der Autor ist angemeldet");
    };
    let accounts = server.state.store.account_list().await.unwrap();
    let debug = format!("{:?} {:?} {accounts:?}", Who::Account(identity.clone()), identity);
    assert!(debug.contains("Erika"), "die Debug-Ausgabe nennt das Konto: {debug}");
    assert!(
        !debug.contains("$argon2") && !debug.contains(NEW_PW) && !debug.to_lowercase().contains("token"),
        "Debug-Ausgaben von Who, Identity und Account tragen weder Hash noch Passwort noch Token: {debug}"
    );
}

// ---- Notweg `passwort` und das echte Programm ----------------------------------------------------------------

#[tokio::test]
async fn the_command_line_resets_a_password_and_ends_the_sessions() {
    let server = common::open().await;
    let author = setup_author(&server).await;
    let reset = |name: &'static str, password: &'static str| auth::cli::reset_password(server.path(), name, password);

    let error = reset("Nemo", NEW_PW).await.expect_err("ein unbekanntes Konto");
    assert!(error.contains("Nemo") && error.contains("Erika"), "nennt den Namen und die vorhandenen Konten: {error}");
    let error = reset("Erika", "zu-kurz").await.expect_err("ein zu kurzes Passwort");
    assert!(error.contains("mindestens 10 Zeichen") && !error.contains("zu-kurz"), "ohne das Passwort: {error}");
    let long: &'static str = Box::leak("c".repeat(201).into_boxed_str());
    let error = reset("Erika", long).await.expect_err("ein zu langes Passwort");
    assert!(error.contains("höchstens 200 Zeichen"), "nennt die Grenze: {error}");
    assert!(
        matches!(who(&server, with_cookie(get("/"), &author)).await, Who::Account(_)),
        "abgelehnte Aufrufe beenden keine Sitzung"
    );
    assert_eq!(login(&server, "Erika", AUTHOR_PW).await.0, StatusCode::SEE_OTHER, "und das alte Passwort gilt weiter");

    reset("erika", NEW_PW).await.expect("das Passwort wird gesetzt (Name ohne Rücksicht auf Großschreibung)");
    assert_eq!(scalar(&server, "SELECT count(*) FROM account_session").await, 0, "alle Sitzungen des Kontos enden");
    assert_eq!(who(&server, with_cookie(get("/"), &author)).await, Who::Anonymous, "das alte Cookie gilt nicht mehr");
    assert_eq!(
        login(&server, "Erika", AUTHOR_PW).await.0,
        StatusCode::UNAUTHORIZED,
        "das alte Passwort gilt nicht mehr"
    );
    exec(&server, "DELETE FROM login_attempt").await;
    assert_eq!(login(&server, "Erika", NEW_PW).await.0, StatusCode::SEE_OTHER, "das neue gilt");
    assert_eq!(scalar(&server, "SELECT count(*) FROM account").await, 1, "der Notweg legt kein Konto an");

    let empty = tempfile::tempdir().unwrap();
    let error = auth::cli::reset_password(empty.path(), "Erika", NEW_PW).await.expect_err("ohne Datenbank");
    assert!(error.contains("keine Datenbank"), "ein Ordner ohne Datenbank wird gemeldet: {error}");
    assert!(!empty.path().join("baruch.sqlite").exists(), "und dabei wird keine angelegt");
    let fresh = common::open().await;
    let error = auth::cli::reset_password(fresh.path(), "Erika", NEW_PW).await.expect_err("ohne Konten");
    assert!(error.contains("/einrichten"), "ohne Konten verweist der Notweg auf das Einrichten: {error}");
}

/// Führt `baruch-server passwort <name> --data <ordner>` aus; liefert Exit-Code, Standardausgabe und Fehlerausgabe.
fn run_password_command(data: &std::path::Path, name: &str, input: &str) -> (Option<i32>, String, String) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_baruch-server"))
        .args(["passwort", name, "--data"])
        .arg(data)
        .env_remove("BARUCH_DATA")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("baruch-server startet");
    child.stdin.take().unwrap().write_all(input.as_bytes()).unwrap();
    let output = child.wait_with_output().unwrap();
    (
        output.status.code(),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

#[tokio::test]
async fn the_program_reads_the_new_password_from_stdin_and_never_prints_it() {
    let server = common::open().await;
    setup_author(&server).await;
    let data = server.path().to_owned();
    let run = |name: &'static str, input: String| {
        let data = data.clone();
        tokio::task::spawn_blocking(move || run_password_command(&data, name, &input))
    };

    let (code, out, err) = run("Erika", format!("{NEW_PW}\nzweite-zeile-zaehlt-nicht\n")).await.unwrap();
    assert_eq!(code, Some(0), "passwort endet mit Exit 0: {err}");
    assert!(out.contains("Erika") && out.contains("gesetzt"), "die Ausgabe nennt das Konto: {out}");
    assert!(!out.contains(NEW_PW) && !err.contains(NEW_PW), "das Passwort steht weder auf stdout noch auf stderr");
    assert!(!out.contains("$argon2") && !err.contains("$argon2"), "auch kein Hash");
    assert_eq!(login(&server, "Erika", NEW_PW).await.0, StatusCode::SEE_OTHER, "die erste Zeile ist das neue Passwort");
    assert_eq!(scalar(&server, "SELECT count(*) FROM account_session").await, 1, "die alten Sitzungen sind beendet");

    let (code, out, err) = run("Erika", "zu-kurz\n".to_owned()).await.unwrap();
    assert_eq!(code, Some(1), "ein zu kurzes Passwort: Exit 1");
    assert!(err.contains("mindestens 10 Zeichen"), "mit Begründung auf stderr: {err}");
    assert!(
        !out.contains("zu-kurz") && !err.contains("zu-kurz"),
        "auch ein abgelehntes Passwort wird nicht ausgegeben"
    );
    let (code, _, err) = run("Nemo", format!("{AUTHOR_PW}\n")).await.unwrap();
    assert_eq!(code, Some(1), "ein unbekanntes Konto: Exit 1");
    assert!(err.contains("Nemo") && !err.contains(AUTHOR_PW), "nennt den Namen, nicht das Passwort: {err}");
    let missing = Command::new(env!("CARGO_BIN_EXE_baruch-server")).arg("passwort").output().unwrap();
    assert_eq!(missing.status.code(), Some(2), "ohne Namen: Aufruffehler (Exit 2)");
    let help = Command::new(env!("CARGO_BIN_EXE_baruch-server")).arg("--help").output().unwrap();
    assert!(
        String::from_utf8_lossy(&help.stdout).contains("baruch-server passwort NAME"),
        "der Hilfetext nennt den Notweg"
    );
}

/// Der echte Server als Prozess, mit Standardausgabe und Fehlerausgabe in Dateien.
struct Process {
    child: std::process::Child,
    url: String,
    data: tempfile::TempDir,
    logs: tempfile::TempDir,
}

impl Drop for Process {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Process {
    /// Datenordner vorbereiten (Verbindungen auf eine tote Adresse: keine Anfrage an echte Dienste), Server starten,
    /// auf „Baruch läuft“ warten.
    async fn start() -> Process {
        let data = tempfile::tempdir().unwrap();
        {
            let state = Arc::new(baruch_server::AppState::open(data.path()).await.expect("Datenordner"));
            let app = baruch_server::router(state.clone());
            for kind in ["eliezer", "multigpt"] {
                let set = json!({ "kind": kind, "config": { "url": "http://127.0.0.1:9" } });
                let answer = common::rpc(app.clone(), "connection.set", set).await;
                assert!(answer["error"].is_null(), "Verbindung {kind} auf eine tote Adresse gestellt: {answer}");
            }
            state.store.pool().close().await;
        }
        let logs = tempfile::tempdir().unwrap();
        let stderr = std::fs::File::create(logs.path().join("stderr.log")).unwrap();
        let mut child = Command::new(env!("CARGO_BIN_EXE_baruch-server"))
            .args(["serve", "--listen", "127.0.0.1:0", "--secrets", "memory", "--data"])
            .arg(data.path())
            .env_remove("BARUCH_ALLOWED_HOSTS")
            .env_remove("BARUCH_LISTEN")
            .env_remove("BARUCH_DATA")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(stderr)
            .spawn()
            .expect("baruch-server startet");
        let stdout = BufReader::new(child.stdout.take().unwrap());
        let (send, lines) = std::sync::mpsc::channel();
        let copy = logs.path().join("stdout.log");
        std::thread::spawn(move || {
            let mut file = std::fs::File::create(copy).unwrap();
            for line in stdout.lines().map_while(Result::ok) {
                let _ = writeln!(file, "{line}");
                let _ = send.send(line);
            }
        });
        let mut process = Process { child, url: String::new(), data, logs };
        let limit = Instant::now() + Duration::from_secs(120);
        while process.url.is_empty() {
            match lines.recv_timeout(limit.saturating_duration_since(Instant::now())) {
                Ok(line) => {
                    if let Some(url) = line.strip_prefix("Baruch läuft: ") {
                        process.url = url.trim_end_matches('/').to_owned();
                    }
                }
                Err(e) => panic!("der Server meldet sich nicht mit „Baruch läuft“: {e}"),
            }
        }
        process
    }

    /// Beendet den Prozess und liefert alles, was er ausgegeben hat (stdout und stderr).
    fn stop(mut self) -> String {
        let _ = self.child.kill();
        let _ = self.child.wait();
        // Der Faden, der stdout mitschreibt, endet mit dem Prozess; kurz warten, bis die Datei vollständig ist.
        std::thread::sleep(Duration::from_millis(200));
        let out = std::fs::read_to_string(self.logs.path().join("stdout.log")).unwrap_or_default();
        let err = std::fs::read_to_string(self.logs.path().join("stderr.log")).unwrap_or_default();
        format!("{out}\n{err}")
    }
}

#[tokio::test]
async fn the_running_server_logs_no_password_token_or_hash() {
    let process = Process::start().await;
    let client = reqwest::Client::builder().no_proxy().redirect(reqwest::redirect::Policy::none()).build().unwrap();
    let url = |path: &str| format!("{}{path}", process.url);
    let cookie_of = |response: &reqwest::Response| -> Option<String> {
        response
            .headers()
            .get_all(reqwest::header::SET_COOKIE)
            .iter()
            .filter_map(|value| value.to_str().ok())
            .find_map(|cookie| cookie.strip_prefix("baruch_session="))
            .map(|rest| rest.split(';').next().unwrap().to_owned())
    };

    // Ohne Konten ist der Server offen wie bisher.
    let start = client.get(url("/")).send().await.unwrap();
    assert_eq!(start.status().as_u16(), 200, "ohne Konten zeigt der laufende Server die Arbeitsseite");
    assert!(cookie_of(&start).is_none(), "ohne Cookie der Sitzung");

    // Einrichten über die echte Verbindung (Gegenstelle Loopback aus dem Betriebssystem, nicht aus dem Test).
    let setup = [("name", "Erika"), ("display", "Erika Beispiel"), ("password", AUTHOR_PW), ("password2", AUTHOR_PW)];
    let forwarded =
        client.post(url("/einrichten")).header("x-forwarded-for", "203.0.113.9").form(&setup).send().await.unwrap();
    assert_eq!(
        forwarded.status().as_u16(),
        403,
        "mit X-Forwarded-For ist es auch am echten Server nicht der eigene Rechner"
    );
    let created = client.post(url("/einrichten")).form(&setup).send().await.unwrap();
    assert_eq!(created.status().as_u16(), 303, "Einrichten am laufenden Server vom eigenen Rechner");
    let first = cookie_of(&created).expect("mit Sitzung");

    let start = client.get(url("/werke?x=1")).send().await.unwrap();
    assert_eq!(start.status().as_u16(), 303, "jetzt verlangt auch der eigene Rechner die Anmeldung");
    assert_eq!(start.headers().get("location").unwrap(), "/anmelden?next=/werke%3Fx%3D1", "mit dem Ziel in next");
    let rpc = json!({ "jsonrpc": "2.0", "id": 1, "method": "account.me", "params": {} });
    let anonymous = client.post(url("/rpc")).json(&rpc).send().await.unwrap();
    assert_eq!(anonymous.status().as_u16(), 401, "/rpc ohne Sitzung: 401");
    let answer: Value = anonymous.json().await.unwrap();
    assert_eq!(answer["error"]["code"], -32002, "mit dem Fehlercode „nicht angemeldet“");

    for attempt in 1..=2 {
        let wrong = [("name", "Erika"), ("password", "falsch-geraten-9"), ("next", "/")];
        let failed = client.post(url("/anmelden")).form(&wrong).send().await.unwrap();
        assert_eq!(failed.status().as_u16(), 401, "Fehlversuch {attempt} am laufenden Server");
    }
    let unknown = [("name", "Nemo"), ("password", "falsch-geraten-8"), ("next", "/")];
    assert_eq!(client.post(url("/anmelden")).form(&unknown).send().await.unwrap().status().as_u16(), 401, "unbekannt");
    let right = [("name", "Erika"), ("password", AUTHOR_PW), ("next", "/werke")];
    let signed = client.post(url("/anmelden")).form(&right).send().await.unwrap();
    assert_eq!(signed.status().as_u16(), 303, "die Anmeldung am laufenden Server");
    let second = cookie_of(&signed).expect("mit Sitzung");
    let me: Value = client
        .post(url("/rpc"))
        .header("cookie", format!("baruch_session={second}"))
        .json(&rpc)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(me["result"]["name"], "Erika", "mit dem Cookie ist der Autor angemeldet: {me}");
    let change = [("current", AUTHOR_PW), ("password", NEW_PW), ("password2", NEW_PW)];
    let changed = client
        .post(url("/konto/passwort"))
        .header("cookie", format!("baruch_session={second}"))
        .form(&change)
        .send()
        .await
        .unwrap();
    assert_eq!(changed.headers().get("location").unwrap(), "/konto?ok=passwort_geaendert", "Passwort geändert");
    let short = [("current", NEW_PW), ("password", "zu-kurz"), ("password2", "zu-kurz")];
    client
        .post(url("/konto/passwort"))
        .header("cookie", format!("baruch_session={second}"))
        .form(&short)
        .send()
        .await
        .unwrap();

    let hashes: Vec<String> = {
        let state = baruch_server::AppState::open(process.data.path()).await.expect("Datenordner lesen");
        let hashes = sqlx::query_scalar("SELECT password FROM account").fetch_all(state.store.pool()).await.unwrap();
        state.store.pool().close().await;
        hashes
    };
    assert_eq!(hashes.len(), 1, "ein Konto mit einem Hash");
    let output = process.stop();
    assert!(output.contains("Baruch läuft"), "die Ausgabe des Servers ist erfasst: {output}");
    assert!(output.contains("Anmeldung: aus"), "beim Start nennt der Server den Zustand der Anmeldung: {output}");
    for secret in
        [AUTHOR_PW, NEW_PW, "falsch-geraten-9", "falsch-geraten-8", "zu-kurz", first.as_str(), second.as_str()]
    {
        assert!(!output.contains(secret), "weder Passwort noch Token steht in der Ausgabe des Servers");
    }
    assert!(!output.contains("$argon2") && !output.contains(hashes[0].as_str()), "auch kein Hash");
}
