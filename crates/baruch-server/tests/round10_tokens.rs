// SPDX-License-Identifier: AGPL-3.0-or-later
//! Tokens für den MCP-Zugang von anderen Rechnern (Runde 10, Aufgabe „scripture“): Erzeugen über das
//! Formular und über RPC (Klartext genau einmal, danach nirgends), Widerrufen mit Kästchen, `/mcp` mit
//! Bearer von fremder Adresse, falsche und widerrufene Tokens, Loopback ohne Token wie bisher,
//! `guard::remote_only_mcp` für alle anderen Pfade, fremder Origin weiter 403, nur der Hash in SQLite.
//!
//! RPC `mcp_token.*` verdrahtet der Integrator in `rpc.rs`, die Middleware im Router; die Tests rufen
//! darum `mcp::tokens::call` direkt und legen die Middleware selbst um den Router.

use crate::common;

use std::net::SocketAddr;

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode, header};
use axum::middleware;
use serde_json::{Value, json};
use tower::ServiceExt;

const FOREIGN: ([u8; 4], u16) = ([192, 0, 2, 7], 50000);
const LOOPBACK: ([u8; 4], u16) = ([127, 0, 0, 1], 40000);

/// Klartext-Token aus einem Text: `baruch_` + 43 Zeichen Base64url.
fn secrets_in(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(at) = rest.find("baruch_") {
        let tail = &rest[at + 7..];
        let len = tail.bytes().take_while(|b| b.is_ascii_alphanumeric() || *b == b'-' || *b == b'_').count();
        if len == 43 {
            out.push(rest[at..at + 7 + 43].to_owned());
        }
        rest = &rest[at + 7..];
    }
    out
}

async fn form(server: &common::Server, body: &str) -> (StatusCode, axum::http::HeaderMap, String) {
    let request = Request::post("/einstellungen/mcp/act")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(body.to_owned()))
        .unwrap();
    server.send(request).await
}

/// MCP-Nachricht an `/mcp` mit Gegenstelle, optionalem Token und Zusatz-Kopfzeilen.
async fn mcp(
    server: &common::Server,
    peer: Option<([u8; 4], u16)>,
    token: Option<&str>,
    extra: &[(header::HeaderName, &str)],
    message: Value,
) -> (StatusCode, axum::http::HeaderMap, String) {
    let mut request = Request::post("/mcp").header(header::CONTENT_TYPE, "application/json");
    if let Some(token) = token {
        request = request.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    for (name, value) in extra {
        request = request.header(name, *value);
    }
    let mut request = request.body(Body::from(message.to_string())).unwrap();
    if let Some(peer) = peer {
        request.extensions_mut().insert(ConnectInfo(SocketAddr::from(peer)));
    }
    server.send(request).await
}

fn ping() -> Value {
    json!({ "jsonrpc": "2.0", "id": 1, "method": "ping" })
}

async fn create_by_rpc(server: &common::Server, name: &str) -> (Value, String) {
    let made = baruch_server::mcp::tokens::call(&server.state, "mcp_token.create", json!({ "name": name }))
        .await
        .expect("Token erzeugen");
    let secret = made["secret"].as_str().unwrap().to_owned();
    (made["token"].clone(), secret)
}

#[tokio::test]
async fn the_form_shows_a_new_token_exactly_once_and_never_again() {
    let server = common::open().await;
    let (status, headers, html) = form(&server, "action=create&name=Arbeitszimmer").await;
    assert_eq!(status, StatusCode::OK, "Erzeugen antwortet direkt mit der Seite (keine Weiterleitung): {html}");
    assert!(headers.get(header::LOCATION).is_none(), "keine Weiterleitung, also kein Klartext in einer Adresse");
    assert_eq!(
        headers.get(header::CACHE_CONTROL).and_then(|v| v.to_str().ok()),
        Some("no-store"),
        "die Seite mit dem Klartext wird nicht zwischengespeichert"
    );
    assert!(headers.get(header::SET_COOKIE).is_none(), "kein Cookie mit dem Token");
    let secrets = secrets_in(&html);
    assert_eq!(secrets.len(), 1, "der Klartext steht genau einmal in der Antwort: {secrets:?}");
    let secret = secrets[0].clone();
    assert!(html.contains("nur jetzt sichtbar"), "Hinweis, dass er nur jetzt zu sehen ist");
    assert!(html.contains("@mcp:arbeitszimmer"), "Autor der Beiträge dieses Zugangs genannt");

    let (status, later) = server.get("/einstellungen/mcp").await;
    assert_eq!(status, StatusCode::OK, "Seite lädt wieder");
    assert!(!later.contains(&secret), "später zeigt die Seite den Klartext nicht mehr");
    assert!(later.contains("<td>Arbeitszimmer</td>"), "der Token steht mit Namen in der Liste");
    assert!(later.contains(&format!("<code>{}…</code>", &secret[..12])), "nur der Anfang wird gezeigt");
    let (_, fragment) = server.get("/einstellungen/mcp?fragment=1").await;
    assert!(!fragment.contains(&secret), "auch das Popup-Fragment zeigt ihn nicht");

    let listed = baruch_server::mcp::tokens::call(&server.state, "mcp_token.list", json!({})).await.unwrap();
    assert!(!listed.to_string().contains(&secret), "die Liste über RPC trägt keinen Klartext: {listed}");
    assert!(listed["tokens"][0].get("hash").is_none(), "und keinen Hash: {listed}");
    let values = server.rpc("settings.get", json!({ "page": "mcp" })).await;
    assert_eq!(values["result"]["loopback_without_token"], true, "ohne Token nur Loopback: {values}");
    assert_eq!(values["result"]["tokens"][0]["name"], "Arbeitszimmer", "Tokens in settings.get: {values}");
    assert!(!values.to_string().contains(&secret), "settings.get ohne Klartext");

    sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)").execute(server.state.store.pool()).await.unwrap();
    let mut bytes = std::fs::read(server.path().join("baruch.sqlite")).unwrap();
    bytes.extend(std::fs::read(server.path().join("baruch.sqlite-wal")).unwrap_or_default());
    let file = String::from_utf8_lossy(&bytes);
    assert!(!file.contains(&secret), "kein Klartext in der Datenbankdatei");
    let hash: String =
        sqlx::query_scalar("SELECT hash FROM mcp_token").fetch_one(server.state.store.pool()).await.unwrap();
    assert!(hash.len() == 64 && file.contains(&hash), "in SQLite steht der SHA-256-Hash");
}

#[tokio::test]
async fn refused_forms_keep_everything_and_show_no_secret() {
    let server = common::open().await;
    let (status, _, html) = form(&server, "action=create&name=++").await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "leerer Name abgelehnt: {html}");
    assert!(html.contains("braucht einen Namen"), "Meldung auf Deutsch");
    let (_, secret) = create_by_rpc(&server, "Laptop").await;
    let (status, _, html) = form(&server, "action=create&name=Laptop").await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "gleicher Name unter gültigen abgelehnt");
    assert!(secrets_in(&html).is_empty(), "eine Ablehnung zeigt keinen Token: {html}");
    assert!(!html.contains(&secret), "auch nicht den vorhandenen");

    let id =
        baruch_server::mcp::tokens::call(&server.state, "mcp_token.list", json!({})).await.unwrap()["tokens"][0]["id"]
            .as_i64()
            .unwrap();
    let (status, _, html) = form(&server, &format!("action=revoke&id={id}")).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "Widerrufen ohne Kästchen abgelehnt (der Server prüft)");
    assert!(html.contains("bestätigen"), "Meldung nennt die Bestätigung");
    assert_eq!(mcp(&server, Some(FOREIGN), Some(&secret), &[], ping()).await.0, StatusCode::OK, "Token gilt weiter");

    let (status, headers, _) = form(&server, &format!("action=revoke&id={id}&confirm=1")).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Widerrufen leitet zurück");
    let location = headers.get(header::LOCATION).unwrap().to_str().unwrap();
    assert_eq!(location, "/einstellungen/mcp?ok=widerrufen", "Rücksprung mit Meldung, ohne Token");
    let (_, html) = server.get(location).await;
    assert!(html.contains("Token widerrufen"), "Meldung nach dem Widerruf");
    assert!(html.contains("widerrufen 20"), "Liste zeigt den Zeitpunkt des Widerrufs");
    let rpc = baruch_server::mcp::tokens::call(&server.state, "mcp_token.revoke", json!({ "id": id })).await;
    assert!(rpc.is_err_and(|e| e.message.contains("bestätigen")), "RPC verlangt confirm: true");
}

#[tokio::test]
async fn a_valid_bearer_token_opens_mcp_from_another_host_under_the_token_name() {
    let server = common::open().await;
    let (_, secret) = create_by_rpc(&server, "Mein Laptop").await;
    let init = json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": { "protocolVersion": "2025-06-18", "capabilities": {},
                    "clientInfo": { "name": "Anderer Name", "version": "3.2" } } });
    let (status, headers, body) = mcp(&server, Some(FOREIGN), Some(&secret), &[], init).await;
    assert_eq!(status, StatusCode::OK, "gültiger Token von 192.0.2.7: {body}");
    let session = headers.get("mcp-session-id").unwrap().to_str().unwrap().to_owned();
    let seen = baruch_server::mcp::session::clients();
    let client = seen.iter().find(|c| c.handle == "@mcp:mein-laptop").expect("Client heißt wie der Token");
    assert_eq!(client.version, "3.2", "aus clientInfo gilt nur die Version");
    assert!(!seen.iter().any(|c| c.handle == "@mcp:anderer-name"), "der Name aus clientInfo zählt nicht");

    let work = server.state.default_work().await.unwrap();
    let key: i64 = sqlx::query_scalar("SELECT id FROM node WHERE work_id = ? ORDER BY seq LIMIT 1")
        .bind(work)
        .fetch_one(server.state.store.pool())
        .await
        .unwrap();
    let call = json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/call",
        "params": { "name": "add_post", "arguments": { "work": work, "key": key, "body": "Gruß vom Laptop." } } });
    let session_header = [(header::HeaderName::from_static("mcp-session-id"), session.as_str())];
    let (status, _, body) = mcp(&server, Some(FOREIGN), Some(&secret), &session_header, call).await;
    assert_eq!(status, StatusCode::OK, "Werkzeug über den Token: {body}");
    let answer: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(answer["result"]["isError"], false, "Beitrag angelegt: {body}");
    assert_eq!(answer["result"]["structuredContent"]["author"], "@mcp:mein-laptop", "Autor @mcp:<tokenname>: {body}");

    // Ohne Sitzung: auch dann heißt der Client wie der Token.
    let call = json!({ "jsonrpc": "2.0", "id": 3, "method": "tools/call",
        "params": { "name": "add_post", "arguments": { "work": work, "key": key, "body": "Noch ein Gruß." } } });
    let (_, _, body) = mcp(&server, None, Some(&secret), &[], call).await;
    let answer: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(answer["result"]["structuredContent"]["author"], "@mcp:mein-laptop", "ohne Sitzung: {body}");

    let used = baruch_server::mcp::tokens::call(&server.state, "mcp_token.list", json!({})).await.unwrap();
    assert!(used["tokens"][0]["last_used"].is_string(), "zuletzt benutzt ist gesetzt: {used}");
}

#[tokio::test]
async fn wrong_or_revoked_tokens_are_refused_even_from_loopback() {
    let server = common::open().await;
    let (token, secret) = create_by_rpc(&server, "Laptop").await;
    let wrong = format!("{}x", &secret[..secret.len() - 1]);
    for (peer, token, why) in [
        (Some(LOOPBACK), wrong.as_str(), "falscher Token über Loopback"),
        (Some(FOREIGN), wrong.as_str(), "falscher Token von fremder Adresse"),
        (Some(LOOPBACK), "irgendwas", "kein Baruch-Token"),
    ] {
        let (status, headers, body) = mcp(&server, peer, Some(token), &[], ping()).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{why}: {body}");
        let challenge = headers.get(header::WWW_AUTHENTICATE).unwrap().to_str().unwrap();
        assert!(challenge.contains("invalid_token"), "{why}: 401 nennt den Grund im Verfahren: {challenge}");
        assert!(body.contains("ungültig oder widerrufen"), "{why}: deutscher Text: {body}");
        assert!(!body.contains(&secret), "{why}: kein Token in der Meldung");
    }
    let (status, _, body) =
        mcp(&server, Some(LOOPBACK), None, &[(header::AUTHORIZATION, "Basic YWI6Y2Q=")], ping()).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "andere Verfahren als Bearer zählen nicht als Token: {body}");

    let (status, ..) = mcp(&server, Some(FOREIGN), None, &[], ping()).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "ohne Token von fremder Adresse");
    let (status, _, body) = mcp(&server, Some(LOOPBACK), None, &[], ping()).await;
    assert_eq!(status, StatusCode::OK, "Loopback ohne Token weiter ok: {body}");

    baruch_server::mcp::tokens::call(&server.state, "mcp_token.revoke", json!({ "id": token["id"], "confirm": true }))
        .await
        .unwrap();
    for peer in [LOOPBACK, FOREIGN] {
        let (status, _, body) = mcp(&server, Some(peer), Some(&secret), &[], ping()).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "widerrufener Token gilt nirgends mehr ({peer:?}): {body}");
    }
}

#[tokio::test]
async fn other_hosts_reach_only_mcp_and_foreign_origins_stay_out() {
    let server = common::open().await;
    let (_, secret) = create_by_rpc(&server, "Laptop").await;
    let app =
        baruch_server::router(server.state.clone()).layer(middleware::from_fn(baruch_server::guard::remote_only_mcp));
    let send = |request: Request<Body>| {
        let app = app.clone();
        async move { app.oneshot(request).await.unwrap().status() }
    };
    let with_peer = |mut request: Request<Body>, peer: ([u8; 4], u16)| {
        request.extensions_mut().insert(ConnectInfo(SocketAddr::from(peer)));
        request
    };

    let page = with_peer(Request::get("/").body(Body::empty()).unwrap(), FOREIGN);
    assert_eq!(send(page).await, StatusCode::FORBIDDEN, "Oberfläche nicht von fremder Adresse");
    let rpc = with_peer(
        Request::post("/rpc")
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::AUTHORIZATION, format!("Bearer {secret}"))
            .body(Body::from(json!({ "jsonrpc": "2.0", "id": 1, "method": "work.list" }).to_string()))
            .unwrap(),
        FOREIGN,
    );
    assert_eq!(send(rpc).await, StatusCode::FORBIDDEN, "/rpc nicht von fremder Adresse, auch nicht mit Token");
    let settings = with_peer(Request::get("/einstellungen/mcp").body(Body::empty()).unwrap(), FOREIGN);
    assert_eq!(send(settings).await, StatusCode::FORBIDDEN, "Token-Verwaltung nicht von fremder Adresse");

    let mcp_ok = with_peer(
        Request::post("/mcp")
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::AUTHORIZATION, format!("Bearer {secret}"))
            .body(Body::from(ping().to_string()))
            .unwrap(),
        FOREIGN,
    );
    assert_eq!(send(mcp_ok).await, StatusCode::OK, "/mcp mit Token geht durch die Middleware");
    let local = with_peer(Request::get("/einstellungen/mcp").body(Body::empty()).unwrap(), LOOPBACK);
    assert_eq!(send(local).await, StatusCode::OK, "vom eigenen Rechner bleibt alles erreichbar");
    let unknown = Request::get("/einstellungen/mcp").body(Body::empty()).unwrap();
    assert_eq!(send(unknown).await, StatusCode::OK, "ohne ConnectInfo (Tests, Unix) geht die Anfrage durch");

    let evil = with_peer(
        Request::post("/mcp")
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::AUTHORIZATION, format!("Bearer {secret}"))
            .header(header::HOST, "192.0.2.1:8765")
            .header(header::ORIGIN, "https://evil.example")
            .body(Body::from(ping().to_string()))
            .unwrap(),
        FOREIGN,
    );
    assert_eq!(send(evil).await, StatusCode::FORBIDDEN, "fremder Origin bleibt 403, auch mit gültigem Token");
}

/// `tools/call` über `/mcp` mit Gegenstelle und optionalem Token; gibt `result` zurück.
async fn call_tool(
    server: &common::Server,
    peer: ([u8; 4], u16),
    token: Option<&str>,
    name: &str,
    args: Value,
) -> Value {
    let call = json!({ "jsonrpc": "2.0", "id": 9, "method": "tools/call",
        "params": { "name": name, "arguments": args } });
    let (status, _, body) = mcp(server, Some(peer), token, &[], call).await;
    assert_eq!(status, StatusCode::OK, "{name}: {body}");
    serde_json::from_str::<Value>(&body).unwrap()["result"].clone()
}

#[tokio::test]
async fn a_token_client_cannot_act_as_a_persona_but_loopback_still_can() {
    // Mit Token kommt ein anderer Rechner herein. Dürfte er `persona` nennen, könnte er Beiträge als Werk des
    // Lektors ausgeben und dessen echte (vom Modell erzeugte) Beiträge löschen; über Loopback ohne Token
    // bleibt die Regel „Eigentum nach Angabe des Clients“ (eigener Rechner).
    let server = common::open().await;
    let (_, secret) = create_by_rpc(&server, "Laptop").await;
    let work = server.state.default_work().await.unwrap();
    let key: i64 = sqlx::query_scalar("SELECT id FROM node WHERE work_id = ? ORDER BY seq LIMIT 1")
        .bind(work)
        .fetch_one(server.state.store.pool())
        .await
        .unwrap();
    let real = call_tool(
        &server,
        LOOPBACK,
        None,
        "add_post",
        json!({ "work": work, "key": key, "body": "Echter Befund.", "persona": "@lektor" }),
    )
    .await;
    assert_eq!(real["isError"], false, "über Loopback ohne Token als Persona: {real}");
    assert_eq!(real["structuredContent"]["author"], "@lektor", "Autor die Persona");
    let post_id = real["structuredContent"]["id"].as_i64().expect("Beitrags-Id");

    let lector_posts = || async {
        let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM post WHERE author = '@lektor'")
            .fetch_one(server.state.store.pool())
            .await
            .unwrap();
        n
    };
    let before = lector_posts().await;
    let attempts = [
        ("add_post", json!({ "work": work, "key": key, "body": "Gefälscht.", "persona": "@lektor" })),
        ("propose_revision", json!({ "work": work, "key": key, "body": "= Neu\n\nText.\n", "persona": "lektor" })),
        ("post_delete", json!({ "work": work, "id": post_id, "persona": "@lektor" })),
        ("post_move", json!({ "work": work, "id": post_id, "key": key, "persona": "@lektor" })),
        ("post_to_idea", json!({ "work": work, "id": post_id, "words": ["Wabe"], "persona": "@lektor" })),
        ("idea_create", json!({ "work": work, "words": ["Wabe"], "persona": "@lektor" })),
        ("outline_add", json!({ "work": work, "key": key, "title": "Neu", "persona": "@lektor" })),
    ];
    for (tool, args) in attempts {
        let answer = call_tool(&server, FOREIGN, Some(&secret), tool, args).await;
        assert_eq!(answer["isError"], true, "{tool} mit Token und persona → isError: {answer}");
        let text = answer["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("nur im eigenen Namen"), "{tool}: Meldung nennt den Grund: {text}");
    }
    let still: Option<String> = sqlx::query_scalar("SELECT author FROM post WHERE id = ?")
        .bind(post_id)
        .fetch_optional(server.state.store.pool())
        .await
        .unwrap();
    assert_eq!(still.as_deref(), Some("@lektor"), "der echte Beitrag steht noch (weder gelöscht noch umgewandelt)");
    assert_eq!(lector_posts().await, before, "kein gefälschter Beitrag des Lektors dazu");

    let own =
        call_tool(&server, FOREIGN, Some(&secret), "add_post", json!({ "work": work, "key": key, "body": "Eigenes." }))
            .await;
    assert_eq!(own["structuredContent"]["author"], "@mcp:laptop", "ohne persona im eigenen Namen: {own}");
}
