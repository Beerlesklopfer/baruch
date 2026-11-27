// SPDX-License-Identifier: AGPL-3.0-or-later
//! Round 9, integrator: end to end through the whole router after menu, settings and connections have been merged. The
//! teams have checked their parts each for themselves (connections the pages directly through the render functions);
//! here everything runs through the frame of the settings, the header from `base.html` and the forms without JavaScript
//! – the way the popup (`settings.ts`) and the browser without script see them. All test texts freely invented.

use crate::common;

use std::net::SocketAddr;

use axum::Router;
use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{HeaderMap, Request, StatusCode, header};
use axum::response::IntoResponse;
use axum::routing::get;
use serde_json::{Value, json};
use tokio::net::TcpListener;

/// Unique test secret (in-memory keyring `MemorySecrets` from `AppState::open`).
const SECRET: &str = "Qx7-integrator-geheim-41b2-Probe";

fn form_escape(text: &str) -> String {
    text.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// Send a form: status, headers, text.
async fn form(server: &common::Server, uri: &str, body: &str) -> (StatusCode, HeaderMap, String) {
    let request = Request::post(uri)
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(body.to_owned()))
        .unwrap();
    server.send(request).await
}

fn location(headers: &HeaderMap) -> String {
    headers.get(header::LOCATION).and_then(|v| v.to_str().ok()).expect("Weiterleitung mit Location").to_owned()
}

/// Target of a redirect without the fragment identifier, as a fragment of the popup (`settings.ts` appends
/// `fragment=1`).
fn as_fragment(location: &str) -> String {
    let path = location.split('#').next().unwrap();
    if path.contains('?') { format!("{path}&fragment=1") } else { format!("{path}?fragment=1") }
}

/// Eliezer as a mock on 127.0.0.1:0: `/v1/models` only with the right bearer.
async fn fake_eliezer() -> String {
    let app = Router::new().route(
        "/v1/models",
        get(|headers: HeaderMap| async move {
            let presented = headers.get(header::AUTHORIZATION).and_then(|v| v.to_str().ok()).unwrap_or("");
            if presented != format!("Bearer {SECRET}") {
                return StatusCode::UNAUTHORIZED.into_response();
            }
            axum::Json(json!({ "object": "list", "data": [{ "id": "probe-modell-integration" }] })).into_response()
        }),
    );
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    format!("http://{address}")
}

async fn mcp_call(server: &common::Server, tool: &str, arguments: Value) -> Value {
    let message = json!({ "jsonrpc": "2.0", "id": 3, "method": "tools/call",
        "params": { "name": tool, "arguments": arguments } });
    let mut request = Request::post("/mcp")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(message.to_string()))
        .unwrap();
    request.extensions_mut().insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 40001))));
    let (status, _, body) = server.send(request).await;
    assert_eq!(status, StatusCode::OK, "tools/call {tool}: {body}");
    serde_json::from_str(&body).unwrap()
}

#[tokio::test]
async fn every_settings_page_loads_as_popup_fragment_and_as_page_without_javascript() {
    let server = common::open().await;
    let work = server.import_garden().await.work;
    let pages = server.rpc("settings.pages", json!({})).await;
    let pages = pages["result"]["pages"].as_array().expect("settings.pages liefert die Liste").clone();
    assert_eq!(
        pages.len(),
        8,
        "acht Seiten nach Vertrag (seit Runde 35 mit Konten), Verbindungen und Quellen eingehängt"
    );
    for page in &pages {
        let slug = page["slug"].as_str().unwrap();
        let body = format!("data-settings-page-body=\"{slug}\"");
        let (status, html) = server.get(&format!("/einstellungen/{slug}?work={work}")).await;
        assert_eq!(status, StatusCode::OK, "{slug}: Seite ohne JavaScript lädt: {html}");
        assert_eq!(
            html.matches("<header class=\"band\"").count(), // Round 33: the header is the ribbon (header.band instead of header.topbar)
            1,
            "{slug}: genau die gemeinsame Kopfzeile aus base.html"
        );
        assert!(html.contains(&body), "{slug}: die Seite zeigt ihren eigenen Inhalt, keinen Platzhalter");
        let (status, fragment) = server.get(&format!("/einstellungen/{slug}?work={work}&fragment=1")).await;
        assert_eq!(status, StatusCode::OK, "{slug}: Fragment fürs Popup lädt");
        assert!(
            fragment.starts_with("<div class=\"settings-frame\"") && fragment.contains(&body),
            "{slug}: das Popup bekommt denselben Rahmen mit demselben Inhalt: {fragment}"
        );
        assert!(!fragment.contains("<header class=\"band\""), "{slug}: im Fragment keine zweite Kopfzeile"); // Round 33: the header is the ribbon (header.band, not header.topbar)
        let rendered = server.rpc("settings.render", json!({ "page": slug, "work": work })).await;
        assert!(
            rendered["result"]["html"].as_str().is_some_and(|h| h.contains(&body)),
            "{slug}: settings.render liefert dasselbe Fragment über RPC: {rendered}"
        );
    }
}

#[tokio::test]
async fn a_secret_entered_in_the_dialog_shows_only_as_deposited_and_the_check_reaches_the_test_server() {
    let server = common::open().await;
    let work = server.import_garden().await.work;
    let eliezer = fake_eliezer().await;
    let body = format!(
        "method=connection.set&kind=eliezer&work={work}&url={}&secret={}",
        form_escape(&eliezer),
        form_escape(SECRET)
    );
    let (status, headers, _) = form(&server, "/einstellungen/verbindungen/act", &body).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Formular ohne JavaScript leitet zurück");
    let target = location(&headers);
    assert!(!target.contains(SECRET), "das Geheimnis steht nie in der Adresse: {target}");
    let (status, page) = server.get(&as_fragment(&target)).await;
    assert_eq!(status, StatusCode::OK, "das Popup lädt die Antwortseite im Rahmen");
    assert!(page.contains("hinterlegt"), "die Seite nennt nur den Zustand „hinterlegt“");
    assert!(!page.contains(SECRET), "und nie das Geheimnis selbst: {page}");

    let (status, headers, _) =
        form(&server, "/einstellungen/verbindungen/act", &format!("method=connection.check&kind=eliezer&work={work}"))
            .await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Prüfen leitet zurück");
    let (_, page) = server.get(&as_fragment(&location(&headers))).await;
    assert!(
        page.contains("probe-modell-integration"),
        "die Prüfung mit dem Bearer aus dem Speicher-Schlüsselbund hat den Test-Server erreicht: {page}"
    );
    assert!(!page.contains(SECRET), "auch das Prüfergebnis zeigt kein Geheimnis");

    let listed = server.rpc("connection.list", json!({})).await;
    assert!(!listed.to_string().contains(SECRET), "RPC connection.list zeigt kein Geheimnis: {listed}");
    let eliezer_row = listed["result"]["connections"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["kind"] == "eliezer")
        .cloned()
        .expect("Eliezer in der Liste");
    assert_eq!(eliezer_row["secret"], "hinterlegt", "RPC nennt nur den Zustand");
    assert_eq!(eliezer_row["check"]["ok"], true, "die Prüfung ist gespeichert und gelungen");
    let full = server.get(&format!("/einstellungen/verbindungen?work={work}")).await.1;
    assert!(!full.contains(SECRET), "auch die ganze Seite ohne JavaScript nicht");
}

#[tokio::test]
async fn sources_created_in_the_dialog_appear_on_the_page_and_for_assistants() {
    let server = common::open().await;
    let body = format!(
        "method=sources.create&title={}&url={}&note={}",
        form_escape("Erfundenes Wörterbuch"),
        form_escape("https://woerter.example/"),
        form_escape("nur zum Testen")
    );
    let (status, headers, _) = form(&server, "/einstellungen/quellen/act", &body).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Anlegen leitet zurück");
    let (_, page) = server.get(&as_fragment(&location(&headers))).await;
    assert!(page.contains("Erfundenes Wörterbuch"), "die neue Quelle steht im Rahmen des Popups: {page}");
    let answer = mcp_call(&server, "sources_list", json!({})).await;
    assert_eq!(
        answer["result"]["structuredContent"]["sources"][0]["title"], "Erfundenes Wörterbuch",
        "Assistenten lesen dieselbe Liste über MCP: {answer}"
    );
    let id = answer["result"]["structuredContent"]["sources"][0]["id"].as_i64().unwrap();
    let (status, _, _) =
        form(&server, "/einstellungen/quellen/act", &format!("method=sources.delete&id={id}&confirm=1")).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Löschen mit Kästchen leitet zurück");
    let answer = mcp_call(&server, "sources_list", json!({})).await;
    assert_eq!(
        answer["result"]["structuredContent"]["sources"].as_array().map(Vec::len),
        Some(0),
        "nach dem Löschen ist die Liste leer"
    );
}

#[tokio::test]
async fn saving_in_display_and_assistants_takes_effect_on_the_next_page() {
    let server = common::open().await;
    let work = server.import_garden().await.work;
    let (status, headers, _) = form(&server, "/einstellungen/darstellung/act", "theme=dark&editor=quelltext").await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Darstellung speichert und leitet zurück");
    let cookies: Vec<String> = headers
        .get_all(header::SET_COOKIE)
        .iter()
        .map(|v| v.to_str().unwrap().split(';').next().unwrap().to_owned())
        .collect();
    assert!(cookies.contains(&"baruch_theme=dark".to_owned()), "das Thema liegt im Cookie: {cookies:?}");
    let request =
        Request::get(format!("/?work={work}")).header(header::COOKIE, cookies.join("; ")).body(Body::empty()).unwrap();
    let (_, _, html) = server.send(request).await;
    assert!(html.contains("data-theme=\"dark\""), "die Arbeitsseite zeichnet das gespeicherte Thema");

    // Round 10: the default is fixed @lektor; the lector follows the context only with „automatisch“.
    server.rpc("persona.set_lector", json!({ "mode": "auto" })).await;
    let (status, _, _) = form(
        &server,
        "/einstellungen/personas/act",
        &format!("method=persona.update&handle=%40nathan&role=Probe&model=probe-m&context_length=9000000&active=1&work={work}"),
    )
    .await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Assistenten speichert und leitet zurück");
    let listed = server.rpc("persona.list", json!({})).await;
    assert!(
        listed["result"]["personas"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p["handle"] == "@nathan" && p["lector"] == true && p["model"] == "probe-m"),
        "mit dem größten Kontext ist @nathan jetzt Lektor (SQLite): {listed}"
    );
}
