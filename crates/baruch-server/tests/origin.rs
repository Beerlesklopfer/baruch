// SPDX-License-Identifier: AGPL-3.0-or-later
//! Schreibende Endpunkte lehnen Anfragen fremder Webseiten ab (CSRF); MCP nimmt ohne Token nur
//! Anfragen vom eigenen Rechner (Loopback) an.

use crate::common;

use std::net::{IpAddr, Ipv6Addr, SocketAddr};

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode, header};
use serde_json::{Value, json};

const HOST: &str = "127.0.0.1:8765";

fn rpc_body(method: &str, params: Value) -> String {
    json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params }).to_string()
}

/// Fester Schlüssel (`key`) des Abschnitts an Stelle `seq`: Schreiben über MCP und Zitieren nehmen
/// nur ihn (Vertrag Runde 5).
async fn key_of(server: &common::Server, work: i64, seq: i64) -> i64 {
    sqlx::query_scalar("SELECT id FROM node WHERE work_id = ? AND seq = ?")
        .bind(work)
        .bind(seq)
        .fetch_one(server.state.store.pool())
        .await
        .unwrap()
}

#[tokio::test]
async fn rpc_from_a_foreign_origin_is_refused_and_changes_nothing() {
    let server = common::open().await;
    let posts = |v: &Value| v["result"]["posts"].as_array().map_or(0, Vec::len);
    let before = posts(&server.rpc("node.get", json!({ "node": 1 })).await);
    let request = Request::post("/rpc")
        .header(header::HOST, HOST)
        .header(header::ORIGIN, "https://evil.example")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(rpc_body("post.add", json!({ "node": 1, "body": "von fremder Seite" }))))
        .unwrap();
    let (status, _, _) = server.send(request).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "fremder Origin bei /rpc → 403");
    let request = Request::post("/rpc")
        .header("sec-fetch-site", "cross-site")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(rpc_body("post.add", json!({ "node": 1, "body": "von fremder Seite" }))))
        .unwrap();
    assert_eq!(server.send(request).await.0, StatusCode::FORBIDDEN, "fremde Seite → 403");
    assert_eq!(posts(&server.rpc("node.get", json!({ "node": 1 })).await), before, "Beitrag trotz Ablehnung");
}

#[tokio::test]
async fn rpc_needs_json_content_type() {
    let server = common::open().await;
    let request = Request::post("/rpc")
        .header(header::CONTENT_TYPE, "text/plain")
        .body(Body::from(rpc_body("work.get", json!({}))))
        .unwrap();
    assert_eq!(server.send(request).await.0, StatusCode::UNSUPPORTED_MEDIA_TYPE, "/rpc verlangt application/json");
}

#[tokio::test]
async fn same_origin_rpc_and_form_pass() {
    let server = common::open().await;
    let request = Request::post("/rpc")
        .header(header::HOST, HOST)
        .header(header::ORIGIN, format!("http://{HOST}"))
        .header("sec-fetch-site", "same-origin")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(rpc_body("work.get", json!({}))))
        .unwrap();
    let (status, _, body) = server.send(request).await;
    assert_eq!(status, StatusCode::OK, "eigene Seite darf /rpc aufrufen: {body}");
    assert!(body.contains("\"result\""), "Antwort mit result statt error: {body}");
    let request = Request::post("/act")
        .header(header::HOST, HOST)
        .header(header::ORIGIN, format!("http://{HOST}"))
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from("method=post.add&node=1&body=hallo"))
        .unwrap();
    assert_eq!(server.send(request).await.0, StatusCode::SEE_OTHER, "Formular der eigenen Seite → Weiterleitung");
}

#[tokio::test]
async fn form_from_a_foreign_origin_is_refused() {
    let server = common::open().await;
    let rev_count = |v: &Value| v["result"]["revisions"].as_array().unwrap().len();
    let before = rev_count(&server.rpc("node.get", json!({ "node": 1 })).await);
    let request = Request::post("/act")
        .header(header::HOST, HOST)
        .header(header::ORIGIN, "https://evil.example")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from("method=revision.save&node=1&body=ueberschrieben"))
        .unwrap();
    assert_eq!(server.send(request).await.0, StatusCode::FORBIDDEN, "fremde Seite → 403");
    assert_eq!(
        rev_count(&server.rpc("node.get", json!({ "node": 1 })).await),
        before,
        "abgelehntes Formular speichert nichts"
    );
}

/// MCP-`ping` mit wählbarer Gegenstelle (`None`: ohne `ConnectInfo`) und Zusatz-Kopfzeilen.
async fn mcp_ping(
    server: &common::Server,
    peer: Option<SocketAddr>,
    extra: &[(header::HeaderName, &str)],
) -> (StatusCode, axum::http::HeaderMap, String) {
    let mut request = Request::post("/mcp").header(header::CONTENT_TYPE, "application/json");
    for (name, value) in extra {
        request = request.header(name, *value);
    }
    let mut request =
        request.body(Body::from(json!({ "jsonrpc": "2.0", "id": 1, "method": "ping" }).to_string())).unwrap();
    if let Some(peer) = peer {
        request.extensions_mut().insert(ConnectInfo(peer));
    }
    server.send(request).await
}

#[tokio::test]
async fn mcp_from_loopback_without_origin_and_token_is_answered() {
    let server = common::open().await;
    let v4 = SocketAddr::from(([127, 0, 0, 1], 40000));
    let v6 = SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), 40000);
    let mapped = SocketAddr::new(IpAddr::V6(Ipv6Addr::new(0, 0, 0, 0, 0, 0xffff, 0x7f00, 1)), 40000);
    for peer in [v4, v6, mapped] {
        let (status, _, body) = mcp_ping(&server, Some(peer), &[]).await;
        assert_eq!(status, StatusCode::OK, "Loopback {peer} ohne Origin und Token muss durchgehen: {body}");
        let answer: Value = serde_json::from_str(&body).unwrap();
        assert_eq!(answer["result"], json!({}), "ping antwortet leer: {body}");
    }
}

#[tokio::test]
async fn mcp_from_another_host_or_without_peer_needs_a_token() {
    let server = common::open().await;
    let foreign = SocketAddr::from(([192, 0, 2, 1], 40000));
    for peer in [Some(foreign), None] {
        let (status, headers, body) = mcp_ping(&server, peer, &[]).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{peer:?} ist kein Loopback: {body}");
        let challenge = headers.get(header::WWW_AUTHENTICATE).map(|v| v.to_str().unwrap().to_owned());
        assert_eq!(challenge.as_deref(), Some(r#"Bearer realm="baruch""#), "401 nennt das Verfahren");
        assert!(body.contains("eigenen Rechner"), "deutscher Text erklärt den Zugang: {body}");
    }
    let (status, _, _) = mcp_ping(&server, Some(foreign), &[(header::AUTHORIZATION, "Bearer irgendwas")]).await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "ein unbekanntes Token gilt nicht (Runde 10: nur erzeugte, nicht widerrufene Tokens öffnen /mcp)"
    );
}

#[tokio::test]
async fn mcp_from_a_foreign_origin_is_refused_before_the_loopback_check() {
    let server = common::open().await;
    let loopback = SocketAddr::from(([127, 0, 0, 1], 40000));
    let extra = [(header::HOST, HOST), (header::ORIGIN, "https://evil.example")];
    let (status, _, body) = mcp_ping(&server, Some(loopback), &extra).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "fremde Webseite über den eigenen Browser bleibt draußen: {body}");
    let (status, _, _) = mcp_ping(&server, None, &extra).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "guard läuft vor der Zugangsprüfung");
}

#[tokio::test]
async fn mcp_needs_json_content_type_also_from_loopback() {
    let server = common::open().await;
    let mut request = Request::post("/mcp")
        .header(header::CONTENT_TYPE, "text/plain")
        .body(Body::from(json!({ "jsonrpc": "2.0", "id": 1, "method": "ping" }).to_string()))
        .unwrap();
    request.extensions_mut().insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 40000))));
    assert_eq!(server.send(request).await.0, StatusCode::UNSUPPORTED_MEDIA_TYPE, "text/plain braucht keinen Preflight");
}

#[tokio::test]
async fn a_foreign_host_name_is_refused_even_from_loopback_with_matching_origin() {
    // DNS-Rebinding: rebind.example zeigt auf 127.0.0.1, Origin und Host passen zusammen.
    let server = common::open().await;
    let revisions = |v: &Value| v["result"]["revisions"].as_array().map_or(0, Vec::len);
    let before = revisions(&server.rpc("node.get", json!({ "node": 4 })).await);
    let host = "rebind.example:8765";
    let message = json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/call",
        "params": { "name": "propose_revision",
                    "arguments": { "work": server.state.work, "key": key_of(&server, server.state.work, 4).await,
                                   "body": "== Fremd\n" } } });
    let mut request = Request::post("/mcp")
        .header(header::HOST, host)
        .header(header::ORIGIN, format!("http://{host}"))
        .header("sec-fetch-site", "same-origin")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(message.to_string()))
        .unwrap();
    request.extensions_mut().insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 40000))));
    assert_eq!(server.send(request).await.0, StatusCode::MISDIRECTED_REQUEST, "MCP unter fremdem Namen");
    assert_eq!(revisions(&server.rpc("node.get", json!({ "node": 4 })).await), before, "kein Vorschlag angelegt");

    let request = Request::post("/rpc")
        .header(header::HOST, host)
        .header(header::ORIGIN, format!("http://{host}"))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(rpc_body("work.export", json!({}))))
        .unwrap();
    assert_eq!(server.send(request).await.0, StatusCode::MISDIRECTED_REQUEST, "RPC unter fremdem Namen");

    let request = Request::get("/").header(header::HOST, host).body(Body::empty()).unwrap();
    let (status, _, body) = server.send(request).await;
    assert_eq!(status, StatusCode::MISDIRECTED_REQUEST, "auch die lesende Seite gibt keinen Text heraus");
    assert!(body.contains("DNS-Rebinding"), "Meldung nennt den Grund: {body}");

    let request = Request::get("/").header(header::HOST, "localhost:8765").body(Body::empty()).unwrap();
    assert_eq!(server.send(request).await.0, StatusCode::OK, "localhost bleibt erlaubt");
}
