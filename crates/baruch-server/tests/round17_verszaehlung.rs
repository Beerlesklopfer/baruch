// SPDX-License-Identifier: AGPL-3.0-or-later
//! Runde 17a „verszaehlung“: Stellen zwischen Verszählungen umrechnen – RPC `scripture.map` und
//! `scripture.versifications`, MCP `scripture_map`. Nur lesend, ohne Modell, ohne Netz.

use crate::common;

use std::net::SocketAddr;

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode, header};
use serde_json::{Value, json};

async fn ok(server: &common::Server, method: &str, params: Value) -> Value {
    let answer = server.rpc(method, params).await;
    assert!(answer["error"].is_null(), "{method} sollte gelingen: {answer}");
    answer["result"].clone()
}

async fn err(server: &common::Server, method: &str, params: Value) -> Value {
    let answer = server.rpc(method, params).await;
    assert!(answer["error"].is_object(), "{method} sollte scheitern: {answer}");
    answer["error"].clone()
}

fn verses(entry: &Value) -> Vec<(u64, u64)> {
    entry["verses"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| (v["chapter"].as_u64().unwrap(), v["verse"].as_u64().unwrap()))
        .collect()
}

#[tokio::test]
async fn an_english_reference_is_mapped_to_the_german_versification() {
    let server = common::open().await;
    let joel = ok(&server, "scripture.map", json!({ "ref": "Joel 2,28–32" })).await;
    assert_eq!((joel["from"].as_str(), joel["to"].as_str()), (Some("KJV"), Some("German")), "Vorgaben KJV → German");
    let entry = &joel["refs"][0];
    assert_eq!(entry["status"], "moved", "Joel 2,28–32 liegt deutsch in Kapitel 3: {entry}");
    assert_eq!(entry["target"], "Joel 3,1–5", "deutsche Schreibweise in der Zielzählung: {entry}");
    assert_eq!(verses(entry), (1..=5).map(|v| (3, v)).collect::<Vec<_>>(), "die fünf Verse einzeln: {entry}");
    assert!(entry["rules"][0].as_str().unwrap().contains("Joel"), "Regel genannt: {entry}");
    assert!(entry["message"].as_str().unwrap().contains("Joel 3,1–5"), "Meldung nennt das Ziel: {entry}");
    let names = joel["versifications"].as_array().unwrap();
    assert_eq!((names.len(), names[0].as_str()), (18, Some("German")), "alle 18 Zählungen, German zuerst: {joel}");

    let mal = ok(&server, "scripture.map", json!({ "ref": "Mal 4,1-6; Ps 51,1; Joh 3,16" })).await;
    let refs = mal["refs"].as_array().unwrap();
    assert_eq!(refs.len(), 3, "drei Stellen, je ein Eintrag: {mal}");
    assert_eq!(refs[0]["target"], "Mal 3,19–24", "Mal 4 (KJV) = Mal 3,19–24: {}", refs[0]);
    assert_eq!(refs[1]["target"], "Ps 51,3", "Psalmüberschrift als Verse 1–2: {}", refs[1]);
    assert_eq!(refs[2]["status"], "same", "Joh 3,16 zählt gleich: {}", refs[2]);
    assert_eq!(refs[2]["target"], "Joh 3,16", "gleich bleibt gleich: {}", refs[2]);

    let back = ok(&server, "scripture.map", json!({ "ref": "Joel 3,1-5", "from": "german", "to": "kjv" })).await;
    assert_eq!(back["refs"][0]["target"], "Joel 2,28–32", "und zurück (Groß/Klein egal): {back}");
}

#[tokio::test]
async fn the_reference_is_checked_against_the_source_versification_and_gaps_are_honest() {
    let server = common::open().await;
    // Joel 3,21 gibt es in KJV (21 Verse), in der deutschen Zählung nicht (Joel 3 hat 5) – geprüft wird gegen from.
    let kjv = ok(&server, "scripture.map", json!({ "ref": "Joel 3,21" })).await;
    assert_eq!(kjv["refs"][0]["target"], "Joel 4,21", "gegen KJV gelesen, nicht gegen German: {kjv}");
    let german = ok(&server, "scripture.map", json!({ "ref": "Joel 3,21", "from": "German", "to": "KJV" })).await;
    let entry = &german["refs"][0];
    assert_eq!(entry["status"], "invalid", "in German hat Joel 3 nur 5 Verse: {entry}");
    assert!(entry["message"].as_str().unwrap().contains("German"), "Grund nennt die Zählung: {entry}");
    assert!(entry["target"].is_null() && verses(entry).is_empty(), "kein Ziel bei ungültiger Angabe: {entry}");

    let gap = ok(&server, "scripture.map", json!({ "ref": "2. Kön 15,3", "from": "German", "to": "KJV" })).await;
    let entry = &gap["refs"][0];
    assert_eq!(entry["status"], "unmapped", "2. Kön 15 deckt keine Regel (39 gegen 38 Verse): {entry}");
    assert!(
        entry["message"].as_str().unwrap().starts_with("Versifikation passt nicht: "),
        "ehrlicher Grund statt geratener Stelle: {entry}"
    );
    let unknown = ok(&server, "scripture.map", json!({ "ref": "Hesek 3,1" })).await;
    assert_eq!(unknown["refs"][0]["status"], "invalid", "unbekanntes Buch: {unknown}");
}

#[tokio::test]
async fn an_unknown_versification_or_a_missing_reference_is_an_invalid_parameter() {
    let server = common::open().await;
    let error = err(&server, "scripture.map", json!({ "ref": "Joh 3,16", "from": "Phantasie" })).await;
    assert_eq!(error["code"], -32602, "ungültiger Parameter: {error}");
    let message = error["message"].as_str().unwrap();
    assert!(
        message.contains("Phantasie") && message.contains("Luther") && message.contains("NRSVA"),
        "mit Liste: {message}"
    );
    let error = err(&server, "scripture.map", json!({ "ref": "" })).await;
    assert_eq!(error["code"], -32602, "leere Angabe: {error}");
    let error = err(&server, "scripture.map", json!({ "ref": "Joh 3,16 ".repeat(60) })).await;
    assert!(error["message"].as_str().unwrap().contains("500"), "höchstens 500 Zeichen: {error}");
}

#[tokio::test]
async fn the_list_of_versifications_names_books_chapters_and_verses() {
    let server = common::open().await;
    let list = ok(&server, "scripture.versifications", json!({})).await;
    let all = list["versifications"].as_array().unwrap();
    assert_eq!(all.len(), 18, "alle Zählungen von sword-modules: {list}");
    let german = &all[0];
    assert_eq!(
        (german["name"].as_str(), german["books"].as_u64(), german["chapters"].as_u64(), german["verses"].as_u64()),
        (Some("German"), Some(66), Some(1189), Some(31172)),
        "German zuerst, mit den Zahlen der Stellenprüfung: {german}"
    );
    let mt = all.iter().find(|v| v["name"] == "MT").unwrap();
    assert_eq!(mt["books"], 39, "MT hat kein Neues Testament (ehrlich ohne die 27 Bücher): {mt}");
}

#[tokio::test]
async fn the_mcp_tool_answers_like_the_rpc_method_and_refuses_an_unknown_versification() {
    let server = common::open().await;
    let request = |message: Value| {
        let mut request = Request::post("/mcp")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(message.to_string()))
            .unwrap();
        request.extensions_mut().insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 40000))));
        request
    };
    let (status, headers, _) = server
        .send(request(json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize",
            "params": { "protocolVersion": "2025-06-18", "capabilities": {},
                        "clientInfo": { "name": "r17-test", "version": "1" } } })))
        .await;
    assert_eq!(status, StatusCode::OK, "initialize");
    let session = headers.get("mcp-session-id").unwrap().to_str().unwrap().to_owned();
    let call = |arguments: Value| {
        let mut r = request(json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/call",
            "params": { "name": "scripture_map", "arguments": arguments } }));
        r.headers_mut().insert("mcp-session-id", session.parse().unwrap());
        r
    };
    let (_, _, body) = server.send(call(json!({ "ref": "Mal 4,5" }))).await;
    let answer: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(answer["result"]["isError"], false, "scripture_map gelingt: {answer}");
    let content = &answer["result"]["structuredContent"];
    let rpc = ok(&server, "scripture.map", json!({ "ref": "Mal 4,5" })).await;
    assert_eq!(content, &rpc, "MCP antwortet wie RPC scripture.map");
    assert_eq!(content["refs"][0]["target"], "Mal 3,23", "Mal 4,5 (KJV) = Mal 3,23: {content}");
    let (_, _, body) = server.send(call(json!({ "ref": "Mal 4,5", "to": "Klingonisch" }))).await;
    let answer: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(answer["result"]["isError"], true, "unbekannte Zählung ist ein Fehler: {answer}");
    let text = answer["result"]["content"][0]["text"].as_str().unwrap();
    assert!(text.contains("Klingonisch") && text.contains("German"), "Fehler nennt die Liste: {text}");
}
