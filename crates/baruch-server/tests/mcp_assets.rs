// SPDX-License-Identifier: AGPL-3.0-or-later
//! Bilder über MCP (Vertrag Runde 6, Abschnitt 4f): lesend (`list_assets`, `read_asset`); seit Runde 11
//! dazu `asset_add` (sofort) und die Anträge `asset_replace`, `asset_set_cover`, `asset_delete`,
//! `asset_requests` (Ende-zu-Ende in `round11_mcp_assets.rs`). Die Namen der Runde 6 für schreibende
//! Werkzeuge gibt es weiterhin nicht. Anfragen als Loopback-Client mit `ConnectInfo`.

use crate::common;

use std::net::SocketAddr;

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode, header};
use serde_json::{Value, json};

const LOOPBACK: ([u8; 4], u16) = ([127, 0, 0, 1], 40000);
const HOUSE: &str = "<svg xmlns=\"http://www.w3.org/2000/svg\"><rect width=\"4\" height=\"3\"/></svg>\n";

async fn mcp(server: &common::Server, method: &str, params: Value) -> Value {
    let message = json!({ "jsonrpc": "2.0", "id": 3, "method": method, "params": params });
    let mut request = Request::post("/mcp")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(message.to_string()))
        .unwrap();
    request.extensions_mut().insert(ConnectInfo(SocketAddr::from(LOOPBACK)));
    let (status, _, body) = server.send(request).await;
    assert_eq!(status, StatusCode::OK, "MCP {method}: {body}");
    serde_json::from_str(&body).unwrap()
}

/// Erfolgreicher Werkzeugaufruf: `structuredContent`.
async fn tool(server: &common::Server, name: &str, arguments: Value) -> Value {
    let answer = mcp(server, "tools/call", json!({ "name": name, "arguments": arguments })).await;
    assert_eq!(answer["result"]["isError"], false, "{name} sollte gelingen: {answer}");
    answer["result"]["structuredContent"].clone()
}

async fn with_house(server: &common::Server) -> i64 {
    let work = server.state.default_work().await.unwrap();
    let added = server
        .rpc(
            "asset.add",
            json!({ "work": work, "file_name": "haus.svg", "content": HOUSE, "encoding": "svg",
                    "path": "/bilder/haus.svg", "alt": "Ein Haus" }),
        )
        .await;
    assert!(added["result"]["commit"].is_string(), "Bild angelegt: {added}");
    work
}

#[tokio::test]
async fn tools_list_names_the_reading_image_tools_and_only_the_round_11_writing_ones() {
    let server = common::open().await;
    let answer = mcp(&server, "tools/list", json!({})).await;
    let names: Vec<&str> =
        answer["result"]["tools"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap()).collect();
    assert!(names.contains(&"list_assets") && names.contains(&"read_asset"), "beide Bild-Werkzeuge: {names:?}");
    let writing: Vec<&&str> = names
        .iter()
        .filter(|n| {
            (n.contains("asset") || n.contains("image") || n.contains("bild"))
                && !matches!(**n, "list_assets" | "read_asset")
        })
        .collect();
    assert_eq!(
        writing,
        [&"asset_add", &"asset_replace", &"asset_set_cover", &"asset_delete", &"asset_requests"],
        "Runde 11: nur asset_add legt an, die übrigen stellen Anträge bzw. lesen sie; sonst kein Bild-Werkzeug"
    );
    for name in ["add_asset", "replace_asset", "update_asset", "delete_asset", "set_current_asset"] {
        let call = mcp(&server, "tools/call", json!({ "name": name, "arguments": { "name": "haus.svg" } })).await;
        assert_eq!(call["error"]["code"], -32602, "{name} gibt es über MCP nicht: {call}");
    }
}

#[tokio::test]
async fn list_assets_and_read_asset_only_read() {
    let server = common::open().await;
    let work = with_house(&server).await;
    let dir = server.mirror_dir();
    let before = common::git(&dir, &["rev-list", "--count", "HEAD"]);

    let list = tool(&server, "list_assets", json!({})).await;
    assert_eq!(list["work"], work, "ohne work das Vorgabewerk: {list}");
    assert_eq!(list["assets"][0]["name"], "haus.svg", "das Bild steht in der Liste: {list}");
    assert_eq!(list["assets"][0]["paths"], json!(["/bilder/haus.svg"]), "mit Pfaden: {list}");
    let slug = server.state.store.work_slug(work).await.unwrap();
    let by_slug = tool(&server, "list_assets", json!({ "work": slug })).await;
    assert_eq!(by_slug["assets"], list["assets"], "Werk auch über den slug");

    let meta = tool(&server, "read_asset", json!({ "name": "haus.svg" })).await;
    assert_eq!(meta["revision"]["number"], 1, "aktuelle Fassung: {meta}");
    assert_eq!(meta["revision"]["alt"], "Ein Haus", "Alternativtext: {meta}");
    assert_eq!(meta["encoding"], "svg", "Kodierung genannt: {meta}");
    assert!(meta.get("content").is_none(), "ohne content: true kein Inhalt: {meta}");
    let full =
        tool(&server, "read_asset", json!({ "work": work, "name": "haus.svg", "rev": 1, "content": true })).await;
    assert_eq!(full["content"], HOUSE, "mit content: true der SVG-Text: {full}");

    let missing =
        mcp(&server, "tools/call", json!({ "name": "read_asset", "arguments": { "name": "fehlt.svg" } })).await;
    assert_eq!(missing["result"]["isError"], true, "unbekanntes Bild ist ein fachlicher Fehler: {missing}");
    let gap =
        mcp(&server, "tools/call", json!({ "name": "read_asset", "arguments": { "name": "haus.svg", "rev": 5 } }))
            .await;
    assert_eq!(gap["result"]["isError"], true, "unbekannte Fassung: {gap}");

    assert_eq!(common::git(&dir, &["rev-list", "--count", "HEAD"]), before, "Lesen über MCP erzeugt keinen Commit");
    let revisions = server.state.store.asset_revisions(work, "haus.svg").await.unwrap();
    assert_eq!(revisions.len(), 1, "und ändert nichts an den Fassungen");
}
