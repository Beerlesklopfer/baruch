// SPDX-License-Identifier: AGPL-3.0-or-later
//! Round 26 „ids“: identifiers (migration 0041), tombstones, instance name (RPC `instance.*`, Einstellungen → Daten).
//!
//! Proven through the real router (`tower::ServiceExt::oneshot`) on a temp data directory: `instance.get` and
//! `instance.set_name` according to the contract (trim, at most 80 characters, no control characters, empty = reset),
//! no MCP tool for it, `work.meta` returns the uuid, „Werk bearbeiten“ shows it read-only, Einstellungen → Daten shows
//! identifier and name and saves the name without JavaScript; foreign Origin → 403.

use std::net::SocketAddr;

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{HeaderMap, Request, StatusCode, header};
use baruch_server::rpc::INVALID_PARAMS;
use serde_json::{Value, json};

use crate::common;

fn result(answer: &Value) -> &Value {
    assert!(answer["error"].is_null(), "RPC-Fehler: {answer}");
    &answer["result"]
}

fn is_v4(uuid: &str) -> bool {
    baruch_exchange::is_uuid(uuid) && &uuid[14..15] == "4" && matches!(&uuid[19..20], "8" | "9" | "a" | "b")
}

/// Percent-encode a form value.
fn enc(value: &str) -> String {
    value
        .bytes()
        .map(|b| if b.is_ascii_alphanumeric() { (b as char).to_string() } else { format!("%{b:02X}") })
        .collect()
}

async fn form(server: &common::Server, uri: &str, body: &str, origin: Option<&str>) -> (StatusCode, HeaderMap, String) {
    let mut request = Request::post(uri).header(header::CONTENT_TYPE, "application/x-www-form-urlencoded");
    if let Some(origin) = origin {
        request = request.header(header::HOST, "127.0.0.1:8765").header(header::ORIGIN, origin);
    }
    server.send(request.body(Body::from(body.to_owned())).unwrap()).await
}

#[tokio::test]
async fn instance_rpc_reads_and_sets_the_name_by_the_contract() {
    let server = common::open().await;
    let got = server.rpc("instance.get", json!({})).await;
    let uuid = result(&got)["uuid"].as_str().expect("uuid als Text").to_owned();
    assert!(is_v4(&uuid), "Instanzkennung kanonisch v4 (Migration 0041): {uuid}");
    assert_eq!(result(&got)["name"], "", "Name anfangs leer");
    assert_eq!(result(&got).as_object().unwrap().len(), 2, "Antwort genau {{uuid, name}}: {got}");
    assert_eq!(
        result(&server.rpc("instance.get", Value::Null).await)["uuid"],
        uuid.as_str(),
        "ohne Parameter geht auch"
    );

    let set = server.rpc("instance.set_name", json!({ "name": "  Schreibstube  " })).await;
    assert_eq!(result(&set), &json!({ "uuid": uuid, "name": "Schreibstube" }), "getrimmt gespeichert, uuid bleibt");
    assert_eq!(result(&server.rpc("instance.get", json!({})).await)["name"], "Schreibstube", "gelesen wie gesetzt");

    let too_long = server.rpc("instance.set_name", json!({ "name": "x".repeat(81) })).await;
    assert_eq!(too_long["error"]["code"], INVALID_PARAMS, "81 Zeichen → INVALID_PARAMS: {too_long}");
    assert!(too_long["error"]["message"].as_str().unwrap().contains("höchstens 80 Zeichen"), "Meldung: {too_long}");
    let control = server.rpc("instance.set_name", json!({ "name": "Zeile\nzwei" })).await;
    assert_eq!(control["error"]["code"], INVALID_PARAMS, "Steuerzeichen → INVALID_PARAMS: {control}");
    assert!(control["error"]["message"].as_str().unwrap().contains("Steuerzeichen"), "Meldung: {control}");
    let wrong = server.rpc("instance.set_name", json!({ "name": 7 })).await;
    assert_eq!(wrong["error"]["code"], INVALID_PARAMS, "Zahl statt Text → INVALID_PARAMS: {wrong}");
    assert_eq!(
        result(&server.rpc("instance.get", json!({})).await)["name"],
        "Schreibstube",
        "abgelehnte Namen ändern nichts"
    );

    let reset = server.rpc("instance.set_name", json!({ "name": "" })).await;
    assert_eq!(result(&reset)["name"], "", "leer = zurücksetzen");
    assert_eq!(result(&reset)["uuid"], uuid.as_str(), "Zurücksetzen lässt die Kennung");
}

#[tokio::test]
async fn mcp_has_no_tool_for_the_instance() {
    // The instance name is a setting of the human; no tool reads or sets it over MCP.
    let server = common::open().await;
    let mut request = Request::post("/mcp")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" }).to_string()))
        .unwrap();
    request.extensions_mut().insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 40000))));
    let (_, _, body) = server.send(request).await;
    let list: Value = serde_json::from_str(&body).unwrap();
    let tools = list["result"]["tools"].as_array().expect("Werkzeugliste");
    assert!(tools.len() > 10, "die Liste ist die echte Werkzeugliste: {list}");
    assert!(
        !tools.iter().any(|t| t["name"].as_str().is_some_and(|n| n.contains("instance") || n.contains("instanz"))),
        "kein MCP-Werkzeug für die Instanz: {list}"
    );
}

#[tokio::test]
async fn work_meta_and_the_edit_page_show_the_uuid_read_only() {
    let server = common::open().await;
    let garden = server.import_garden().await;
    let uuid = server.state.store.work_uuid(garden.work).await.unwrap();
    assert!(is_v4(&uuid), "Werk-uuid kanonisch v4: {uuid}");

    let meta = server.rpc("work.meta", json!({ "work": garden.work })).await;
    assert_eq!(result(&meta)["work"]["uuid"], uuid.as_str(), "work.meta liefert die uuid des Werks: {meta}");

    let (status, page) = server.get(&format!("/werke/{}/bearbeiten", garden.work)).await;
    assert_eq!(status, StatusCode::OK, "Seite „Werk ändern“");
    assert!(page.contains(&format!("<code>{uuid}</code>")), "die uuid steht auf der Seite: {page}");
    assert!(page.contains("Kennung des Werks (UUID"), "mit deutscher Erklärung");
    // Read-only: no form field carries the uuid, and work.update knows no field for it.
    assert!(!page.contains("name=\"uuid\""), "kein Eingabefeld für die uuid");
    let form_part = &page[page.find("<form class=\"work-form").unwrap()..];
    let form_part = &form_part[..form_part.find("</form>").unwrap()];
    assert!(!form_part.contains(&uuid), "die uuid steht nicht im Formular");
    let update =
        server.rpc("work.update", json!({ "work": garden.work, "uuid": "00000000-0000-4000-8000-000000000000" })).await;
    assert_eq!(
        server.state.store.work_uuid(garden.work).await.unwrap(),
        uuid,
        "work.update ändert die uuid nicht: {update}"
    );

    // The same display on the settings page „Werk“ (same form part).
    let (_, settings) = server.get(&format!("/einstellungen/werk?work={}", garden.work)).await;
    assert!(settings.contains(&format!("<code>{uuid}</code>")), "Einstellungen → Werk zeigt die uuid");
}

#[tokio::test]
async fn the_data_page_shows_the_instance_and_saves_its_name_without_javascript() {
    let server = common::open().await;
    let instance = server.state.store.instance().await.unwrap();
    let (status, page) = server.get("/einstellungen/daten").await;
    assert_eq!(status, StatusCode::OK, "Einstellungen → Daten");
    assert!(page.contains(&format!("<code data-instance-uuid>{}</code>", instance.uuid)), "Kennung sichtbar: {page}");
    assert!(page.contains(&format!("ohne Namen ({})", &instance.uuid[..8])), "ohne Namen mit uuid-Anfang");
    assert!(page.contains("action=\"/einstellungen/daten/instanz\""), "Formular ohne JavaScript");
    assert!(page.contains("name=\"name\""), "Feld name");

    // Save: 303 back to the page (with work), there the notice and the name.
    let (status, headers, _) =
        form(&server, "/einstellungen/daten/instanz", &format!("work=1&name={}", enc(" Gartenhaus <&> ")), None).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Speichern leitet weiter");
    assert_eq!(
        headers.get(header::LOCATION).and_then(|v| v.to_str().ok()),
        Some("/einstellungen/daten?ok=instanz&work=1"),
        "zurück auf die Seite Daten mit Werk"
    );
    assert_eq!(server.state.store.instance().await.unwrap().name, "Gartenhaus <&>", "getrimmt gespeichert");
    let (_, page) = server.get("/einstellungen/daten?ok=instanz").await;
    assert!(page.contains("Name der Instanz gespeichert: Gartenhaus &lt;&amp;&gt;."), "Hinweis, maskiert: {page}");
    assert!(page.contains("<dd data-instance-name>Gartenhaus &lt;&amp;&gt;</dd>"), "Name angezeigt, maskiert");
    assert!(page.contains("value=\"Gartenhaus &lt;&amp;&gt;\""), "Feld vorbelegt, maskiert");
    let values = server.rpc("settings.get", json!({ "page": "daten" })).await;
    assert_eq!(
        result(&values)["instance"],
        json!({ "uuid": instance.uuid, "name": "Gartenhaus <&>" }),
        "settings.get daten trägt die Instanz"
    );

    // Invalid: 422 with the page and the message, nothing saved.
    let (status, _, page) =
        form(&server, "/einstellungen/daten/instanz", &format!("name={}", "y".repeat(81)), None).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "zu langer Name → 422");
    assert!(page.contains("höchstens 80 Zeichen"), "Meldung auf der Seite: {page}");
    assert!(page.contains("role=\"alert\""), "als Fehlermeldung");
    let (status, _, page) = form(&server, "/einstellungen/daten/instanz", "name=Tab%09drin", None).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "Steuerzeichen → 422");
    assert!(page.contains("Steuerzeichen"), "Meldung nennt Steuerzeichen: {page}");
    assert_eq!(server.state.store.instance().await.unwrap().name, "Gartenhaus <&>", "nichts gespeichert");

    // Foreign Origin: 403 from the guard, nothing saved.
    let (status, _, _) =
        form(&server, "/einstellungen/daten/instanz", "name=Fremd", Some("https://evil.example")).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "fremder Origin → 403");
    assert_eq!(server.state.store.instance().await.unwrap().name, "Gartenhaus <&>", "fremder Origin ändert nichts");

    // Empty = reset.
    let (status, _, _) = form(&server, "/einstellungen/daten/instanz", "name=", Some("http://127.0.0.1:8765")).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "gleicher Origin geht durch");
    assert_eq!(server.state.store.instance().await.unwrap().name, "", "leer setzt zurück");
}
