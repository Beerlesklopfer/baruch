// SPDX-License-Identifier: AGPL-3.0-or-later
//! Ideas through MCP (contract Round 7, section 6f): `idea_create`, `idea_read`, `idea_list`, `idea_search`,
//! `idea_update`, `idea_delete` with full CRUD for assistants too (ideas change no book text), origin `persona` or
//! `@mcp:<client>` in database and mirror commit.

use crate::common;

use std::net::SocketAddr;

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode, header};
use serde_json::{Value, json};

/// MCP request from the own machine, optionally with a session; response and headers.
async fn post(server: &common::Server, session: Option<&str>, message: Value) -> (axum::http::HeaderMap, Value) {
    let mut request = Request::post("/mcp").header(header::CONTENT_TYPE, "application/json");
    if let Some(id) = session {
        request = request.header("mcp-session-id", id);
    }
    let mut request = request.body(Body::from(message.to_string())).unwrap();
    request.extensions_mut().insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 40000))));
    let (status, headers, body) = server.send(request).await;
    assert_eq!(status, StatusCode::OK, "MCP antwortet: {body}");
    (headers, serde_json::from_str(&body).unwrap())
}

async fn initialize(server: &common::Server, name: &str) -> String {
    let (headers, answer) = post(
        server,
        None,
        json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize",
                "params": { "protocolVersion": "2025-06-18", "capabilities": {},
                            "clientInfo": { "name": name, "version": "1.0" } } }),
    )
    .await;
    assert!(answer["result"].is_object(), "initialize: {answer}");
    headers.get("mcp-session-id").unwrap().to_str().unwrap().to_owned()
}

async fn call(server: &common::Server, session: Option<&str>, tool: &str, arguments: Value) -> Value {
    post(
        server,
        session,
        json!({ "jsonrpc": "2.0", "id": 3, "method": "tools/call",
                                   "params": { "name": tool, "arguments": arguments } }),
    )
    .await
    .1
}

async fn ok(server: &common::Server, session: Option<&str>, tool: &str, arguments: Value) -> Value {
    let answer = call(server, session, tool, arguments).await;
    assert_eq!(answer["result"]["isError"], false, "{tool} sollte gelingen: {answer}");
    answer["result"]["structuredContent"].clone()
}

async fn refused(server: &common::Server, session: Option<&str>, tool: &str, arguments: Value) -> String {
    let answer = call(server, session, tool, arguments).await;
    assert_eq!(answer["result"]["isError"], true, "{tool} sollte fachlich scheitern: {answer}");
    answer["result"]["content"][0]["text"].as_str().unwrap().to_owned()
}

fn last_commit(dir: &std::path::Path) -> Option<String> {
    common::git(dir, &["log", "-1", "--format=%an <%ae>%n%cn <%ce>%n%B"])
}

#[tokio::test]
async fn the_six_idea_tools_follow_read_asset_and_say_they_change_no_book_text() {
    let server = common::open().await;
    let (_, answer) = post(&server, None, json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" })).await;
    let tools = answer["result"]["tools"].as_array().unwrap();
    let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
    let at = names.iter().position(|n| *n == "read_asset").unwrap();
    // Round 9: the reading sources_list is directly after read_asset, the ideas after it.
    assert_eq!(names[at + 1], "sources_list", "sources_list direkt nach read_asset: {names:?}");
    let at = at + 1;
    assert_eq!(
        &names[at + 1..at + 8],
        ["idea_create", "add_idea", "idea_read", "idea_list", "idea_search", "idea_update", "idea_delete"],
        "nach read_asset, vor ask_persona; add_idea ist der zweite Name für idea_create: {names:?}"
    );
    assert_eq!(
        &names[at + 8..at + 14],
        ["idea_move", "idea_map", "idea_tree", "idea_links", "idea_link", "idea_unlink"],
        "Runde 34: idea_move direkt nach idea_delete (es gehört zum CRUD der Einfälle); Runde 25/34: die Werkzeuge der \
         Mindmap danach, idea_tree (Baum) direkt hinter idea_map (flache Sicht): {names:?}"
    );
    assert_eq!(names[at + 14], "ask_persona", "dahinter die Persona-Werkzeuge (seit Runde 10 keine Stubs mehr)");
    assert_eq!(
        &names[at + 14..],
        [
            "ask_persona",
            "chat",
            "scripture_check",
            "scripture_map",
            "bible_modules",
            "bible_lookup",
            "bible_search",
            "commentary_lookup",
            "lexicon_lookup",
            "lexicon_search",
            "rag_search",
            "rag_status",
            "source_read"
        ],
        "Runde 10: nach den Persona-Werkzeugen nur noch das lesende scripture_check (seit Runde 17a mit dem lesenden \
         scripture_map dahinter), seit 2026-11-06 dahinter die \
         lesenden Bibel-Werkzeuge (seit Runde 15 mit Kommentaren und Lexika), seit Runde 12 die lesenden RAG-Werkzeuge, seit Runde 13 das lesende source_read: {names:?}"
    );
    for tool in &tools[at + 1..at + 8] {
        let text = tool["description"].as_str().unwrap();
        assert!(!text.starts_with("STUB"), "kein Stub: {text}");
        assert_eq!(tool["inputSchema"]["type"], "object", "Objekt-Schema: {tool}");
    }
    for name in ["idea_create", "add_idea", "idea_update", "idea_delete"] {
        let tool = tools.iter().find(|t| t["name"] == name).unwrap();
        assert!(
            tool["description"].as_str().unwrap().contains("hanges no book text"),
            "{name} nennt die Ausnahme: {tool}"
        );
    }
}

#[tokio::test]
async fn a_client_and_a_persona_create_change_and_delete_ideas_with_their_origin() {
    let server = common::open().await;
    let session = initialize(&server, "Probe Client").await;
    let s = Some(session.as_str());
    let dir = server.mirror_dir();
    let works = ok(&server, s, "list_works", json!({})).await;
    let slug = works["works"][0]["slug"].as_str().unwrap().to_owned();

    let created = ok(
        &server,
        s,
        "idea_create",
        json!({ "work": slug, "words": ["Laube", "Wein"], "note": "Reben an der Laube" }),
    )
    .await;
    let id = created["idea"]["id"].as_i64().unwrap();
    assert_eq!(
        created["idea"]["origin"],
        json!({ "kind": "mcp", "handle": "@mcp:probe-client" }),
        "Client als Herkunft"
    );
    assert!(created["commit"].is_string(), "ein Commit: {created}");
    if let Some(log) = last_commit(&dir) {
        assert!(
            log.starts_with("MCP-Client (@mcp:probe-client) <mcp-probe-client@persona.baruch.invalid>\nAutor <"),
            "{log}"
        );
        for trailer in ["Baruch-Origin: mcp", "Persona: @mcp:probe-client", "Baruch-Action: idea-create"] {
            assert!(log.contains(trailer), "{trailer} fehlt: {log}");
        }
    }

    let by_persona = ok(
        &server,
        s,
        "idea_update",
        json!({ "id": id, "note": "Reben und Rosen", "open": false, "persona": "lektor" }),
    )
    .await;
    assert_eq!(by_persona["idea"]["status"], "done", "open: false → verarbeitet");
    assert_eq!(by_persona["idea"]["origin"]["kind"], "mcp", "die Herkunft des Einfalls bleibt der Client");
    if let Some(log) = last_commit(&dir) {
        assert!(log.contains("(@lektor) <lektor@persona.baruch.invalid>"), "Autor des Commits ist die Persona: {log}");
        assert!(log.contains("Baruch-Origin: persona") && log.contains("Persona: @lektor"), "{log}");
    }
    let text = refused(&server, s, "idea_update", json!({ "id": id, "note": "x", "persona": "@niemand" })).await;
    assert!(text.contains("Unknown persona"), "unbekannte Persona → isError: {text}");
    assert_eq!(
        ok(&server, s, "idea_read", json!({ "id": id })).await["idea"]["note"],
        "Reben und Rosen",
        "unverändert"
    );

    let listed = ok(&server, s, "idea_list", json!({ "work": slug, "open": false })).await;
    assert_eq!(listed["ideas"].as_array().unwrap().len(), 1, "verarbeitete im Werk: {listed}");
    assert!(
        ok(&server, s, "idea_list", json!({ "work": "none" })).await["ideas"].as_array().unwrap().is_empty(),
        "ohne Werk: keine"
    );
    let found = ok(&server, s, "idea_search", json!({ "query": "rosen" })).await;
    assert_eq!(found["hits"][0]["id"], id, "Volltext: {found}");
    let text = refused(&server, s, "idea_search", json!({ "query": "  " })).await;
    assert!(text.contains("Search term"), "{text}");

    let unlinked = ok(&server, s, "idea_update", json!({ "id": id, "work": null })).await;
    assert!(unlinked["idea"]["work"].is_null(), "work: null → ohne Werk");
    let deleted = ok(&server, s, "idea_delete", json!({ "id": id })).await;
    assert_eq!(deleted["deleted"], true, "ohne confirm gelöscht (wiederherstellbar): {deleted}");
    let loose = server.path().join("einfaelle");
    if let Some(log) = last_commit(&loose) {
        assert!(log.contains("Baruch-Action: idea-delete") && log.contains("- Gedanke: Reben und Rosen"), "{log}");
    }
    let read = ok(&server, s, "idea_read", json!({ "id": id })).await;
    assert!(read["idea"]["deleted"].is_string(), "gelöscht, aber lesbar: {read}");
    let restored = server.rpc("idea.restore", json!({ "id": id })).await;
    assert!(restored["result"]["idea"]["deleted"].is_null(), "über RPC wiederhergestellt: {restored}");

    let text = refused(&server, s, "idea_read", json!({ "id": 4711 })).await;
    assert!(text.contains("4711"), "unbekannter Einfall → isError: {text}");
    let text = refused(&server, s, "idea_create", json!({ "words": "Rose\nDorn" })).await;
    assert!(text.contains("Zeilenumbruch"), "Grenzen gelten auch hier: {text}");
    let text = refused(&server, s, "idea_create", json!({ "words": ["Rose"], "persona": "@gibtsnicht" })).await;
    assert!(text.contains("Unknown persona"), "{text}");
    let answer = call(&server, s, "idea_create", json!({ "note": "ohne Schlagwörter" })).await;
    assert_eq!(answer["error"]["code"], -32602, "words fehlt → -32602: {answer}");
    let all = server.rpc("idea.list", json!({})).await;
    assert_eq!(all["result"]["ideas"].as_array().unwrap().len(), 1, "abgelehnte Aufrufe legen nichts an");
}

#[tokio::test]
async fn add_idea_is_a_second_name_for_idea_create_with_the_same_effect() {
    let server = common::open().await;
    let session = initialize(&server, "Zweitname").await;
    let s = Some(session.as_str());
    let works = ok(&server, s, "list_works", json!({})).await;
    let slug = works["works"][0]["slug"].as_str().unwrap().to_owned();
    let created =
        ok(&server, s, "add_idea", json!({ "work": slug, "words": "Brunnen, Kelle", "note": "Wasser holen" })).await;
    let id = created["idea"]["id"].as_i64().unwrap();
    assert_eq!(
        created["idea"]["words"],
        json!(["Brunnen", "Kelle"]),
        "add_idea nimmt Schlagwörter wie idea_create: {created}"
    );
    assert_eq!(
        created["idea"]["origin"],
        json!({ "kind": "mcp", "handle": "@mcp:zweitname" }),
        "Herkunft wie bei idea_create: der Client"
    );
    assert!(created["commit"].is_string(), "auch add_idea schreibt einen Commit in den Spiegel: {created}");
    let read = ok(&server, s, "idea_read", json!({ "id": id })).await;
    assert_eq!(read["idea"]["note"], "Wasser holen", "der über add_idea angelegte Einfall ist lesbar: {read}");
    let text = refused(&server, s, "add_idea", json!({ "work": slug, "words": [] })).await;
    assert!(!text.is_empty(), "ohne Schlagwörter lehnt add_idea ab wie idea_create: {text}");
}
