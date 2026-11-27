// SPDX-License-Identifier: AGPL-3.0-or-later
//! MCP-Werkzeuge über `POST /mcp`: echter Datenordner (SQLite, Bibliothek, Git-Spiegel) im
//! Temp-Ordner, Anfragen als Loopback-Client mit `ConnectInfo`.

use crate::common;

use std::net::SocketAddr;

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{HeaderMap, Request, StatusCode, header};
use serde_json::{Value, json};

const LOOPBACK: ([u8; 4], u16) = ([127, 0, 0, 1], 40000);

/// MCP-Anfrage vom eigenen Rechner, wahlweise mit Sitzung.
async fn post(server: &common::Server, session: Option<&str>, message: Value) -> (StatusCode, HeaderMap, Value) {
    let mut request = Request::post("/mcp").header(header::CONTENT_TYPE, "application/json");
    if let Some(id) = session {
        request = request.header("mcp-session-id", id);
    }
    let mut request = request.body(Body::from(message.to_string())).unwrap();
    request.extensions_mut().insert(ConnectInfo(SocketAddr::from(LOOPBACK)));
    let (status, headers, body) = server.send(request).await;
    let value = if body.is_empty() { Value::Null } else { serde_json::from_str(&body).unwrap() };
    (status, headers, value)
}

/// `initialize` mit `clientInfo`; gibt die neue Sitzungs-Id zurück.
async fn initialize(server: &common::Server, name: &str, version: &str) -> String {
    let (status, headers, answer) = post(
        server,
        None,
        json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": { "protocolVersion": "2025-06-18", "capabilities": {},
                    "clientInfo": { "name": name, "version": version } } }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "initialize muss gelingen: {answer}");
    assert_eq!(answer["result"]["protocolVersion"], "2025-06-18", "Protokollversion des Clients: {answer}");
    let id = headers.get("mcp-session-id").expect("initialize liefert keine Mcp-Session-Id").to_str().unwrap();
    assert!(id.len() >= 16 && id.chars().all(|c| c.is_ascii_hexdigit()), "Sitzungs-Id lesbar und lang genug: {id}");
    id.to_owned()
}

/// `tools/call`; gibt das ganze JSON-RPC-Antwortobjekt zurück.
async fn call(server: &common::Server, session: Option<&str>, tool: &str, arguments: Value) -> Value {
    let (status, _, answer) = post(
        server,
        session,
        json!({ "jsonrpc": "2.0", "id": 7, "method": "tools/call",
        "params": { "name": tool, "arguments": arguments } }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "tools/call {tool}: {answer}");
    answer
}

/// Erfolgreicher Aufruf: `structuredContent`, dazu dieselben Daten als Text.
async fn ok(server: &common::Server, session: Option<&str>, tool: &str, arguments: Value) -> Value {
    let answer = call(server, session, tool, arguments).await;
    let result = &answer["result"];
    assert_eq!(result["isError"], false, "{tool} sollte gelingen: {answer}");
    let text: Value = serde_json::from_str(result["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(text, result["structuredContent"], "Text und structuredContent müssen übereinstimmen");
    assert!(result["structuredContent"].is_object(), "structuredContent ist laut MCP ein Objekt: {answer}");
    result["structuredContent"].clone()
}

/// Fachlicher Fehler: `isError: true`, Text zurück.
async fn refused(server: &common::Server, session: Option<&str>, tool: &str, arguments: Value) -> String {
    let answer = call(server, session, tool, arguments).await;
    assert_eq!(answer["result"]["isError"], true, "{tool} sollte fachlich scheitern: {answer}");
    answer["result"]["content"][0]["text"].as_str().unwrap().to_owned()
}

/// Fester Schlüssel (`key`) des Abschnitts an Stelle `seq`; schreibende Werkzeuge nehmen nur ihn.
async fn key(server: &common::Server, work: i64, seq: i64) -> i64 {
    sqlx::query_scalar("SELECT id FROM node WHERE work_id = ? AND seq = ?")
        .bind(work)
        .bind(seq)
        .fetch_one(server.state.store.pool())
        .await
        .unwrap()
}

async fn slug(server: &common::Server) -> String {
    let works = ok(server, None, "list_works", json!({})).await;
    works["works"][0]["slug"].as_str().unwrap().to_owned()
}

#[tokio::test]
async fn tools_are_listed_in_contract_order_and_stubs_say_so() {
    let server = common::open().await;
    let (_, _, answer) = post(&server, None, json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" })).await;
    let tools = answer["result"]["tools"].as_array().unwrap();
    let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
    assert_eq!(
        names,
        [
            "list_works",
            "outline",
            "work_timing",
            "read_node",
            "search",
            "typst_check",
            "exchange_export",
            "typeset_settings",
            "epub_export",
            "epub_check",
            "glossary_list",
            "glossary_propose",
            "index_propose",
            "revision_amends",
            "propose_revision",
            "add_post",
            "post_move",
            "post_delete",
            "post_to_idea",
            "outline_add",
            "outline_rename",
            "outline_move",
            "outline_set_level",
            "outline_set_numbered",
            "outline_delete",
            "outline_requests",
            "asset_add",
            "asset_replace",
            "asset_set_cover",
            "asset_delete",
            "asset_requests",
            "cover_layout",
            "cover_layout_set",
            "findings",
            "finding_propose_status",
            "post_edit",
            "text_search",
            "solution_variants",
            "list_personas",
            "list_assets",
            "read_asset",
            "sources_list",
            "idea_create",
            "add_idea",
            "idea_read",
            "idea_list",
            "idea_search",
            "idea_update",
            "idea_delete",
            "idea_move",
            "idea_map",
            "idea_tree",
            "idea_links",
            "idea_link",
            "idea_unlink",
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
        "Reihenfolge laut Vertrag (add_idea = zweiter Name für idea_create, direkt dahinter; Runde 8: \
         post_move, post_delete, post_to_idea nach add_post; typst_check nach search; Runde 26: das lesende exchange_export direkt nach typst_check; Runde 25: die Werkzeuge der Gedankenkarte idea_map, idea_links, idea_link, idea_unlink direkt nach idea_delete; Runde 34 (Mindmap): idea_move direkt nach idea_delete und idea_tree direkt nach idea_map; Wunsch 2026-11-06: \
         die Gliederungswerkzeuge outline_* nach post_to_idea; Runde 9: das lesende sources_list nach read_asset; \
         Runde 10: das lesende scripture_check nach chat; Runde 17a: das lesende scripture_map direkt dahinter; 2026-11-06: die lesenden bible_* danach; Runde 15: die lesenden commentary_lookup, lexicon_lookup, lexicon_search nach bible_search; Runde 12: die lesenden rag_search und rag_status nach bible_search; Runde 13: das lesende source_read nach rag_status; Runde 16b: das lesende work_timing direkt nach outline; Runde 11: die Bild-Werkzeuge asset_* nach \
         outline_requests, damit sources_list direkt hinter read_asset bleibt; Runde 27: cover_layout und \
         cover_layout_set direkt nach asset_requests, denn der Antrag läuft über die Bildanträge; Runde 28: findings, \
         finding_propose_status und post_edit direkt nach cover_layout_set; Runde 30: text_search und solution_variants \
         direkt nach post_edit; Runde 29: das lesende typeset_settings direkt nach exchange_export; Runde 20: die lesenden \
         epub_export und epub_check direkt nach typeset_settings; Runde 31: glossary_list, glossary_propose und \
         index_propose direkt nach epub_check; Runde 22: das lesende revision_amends direkt nach index_propose)"
    );
    for tool in tools {
        assert_eq!(tool["inputSchema"]["type"], "object", "jedes Werkzeug hat ein Objekt-Schema: {tool}");
        let stub = tool["description"].as_str().unwrap().starts_with("STUB");
        // Runde 20/31: epub_* und die Glossar-Werkzeuge sind umgesetzt, die S1-Ausnahmen sind entfallen.
        assert!(!stub, "Runde 10: kein Werkzeug ist mehr als STUB beschrieben (ask_persona und chat gehen): {tool}");
    }
    // Ohne Modell-Backend (AppState::open = NoModel) antworten ask_persona und chat ehrlich mit isError;
    // chat ohne key legt seit Runde 15 einen Faden ohne Abschnitt an (kein Stub mehr).
    let work = server.state.default_work().await.unwrap();
    let key = server.state.store.load_work(work).await.unwrap().nodes[2].key;
    let text = refused(
        &server,
        None,
        "ask_persona",
        json!({ "work": work, "key": key, "persona": "@lektor", "question": "Hallo?" }),
    )
    .await;
    assert!(text.contains("Kein Modell-Backend"), "ehrliche Meldung ohne Backend: {text}");
    let answer = call(&server, None, "chat", json!({ "work": work, "message": "Hallo @lektor" })).await;
    let result = &answer["result"];
    assert_eq!(result["isError"], false, "Faden ohne key: der Beitrag steht im Faden: {answer}");
    let errors = result["structuredContent"]["errors"].as_array().unwrap();
    assert!(
        errors.iter().any(|e| e["message"].as_str().unwrap_or("").contains("Kein Modell-Backend")),
        "die Antwort der Persona scheitert ehrlich ohne Backend: {answer}"
    );
}

#[tokio::test]
async fn unknown_tools_and_unreadable_arguments_are_protocol_errors() {
    let server = common::open().await;
    let answer = call(&server, None, "gibtsnicht", json!({})).await;
    assert_eq!(answer["error"]["code"], -32602, "unbekanntes Werkzeug → -32602: {answer}");
    let answer = call(&server, None, "outline", json!({ "work": [1, 2] })).await;
    assert_eq!(answer["error"]["code"], -32602, "work weder Zahl noch Text → -32602: {answer}");
    let answer = call(&server, None, "read_node", json!({ "work": 1 })).await;
    assert_eq!(answer["error"]["code"], -32602, "node fehlt → -32602: {answer}");
}

#[tokio::test]
async fn notifications_get_202_without_body() {
    let server = common::open().await;
    let (status, _, body) =
        post(&server, None, json!({ "jsonrpc": "2.0", "method": "notifications/initialized" })).await;
    assert_eq!(status, StatusCode::ACCEPTED, "Benachrichtigung ohne Antwort");
    assert!(body.is_null(), "kein Rumpf bei 202: {body}");
    let (_, _, pong) = post(&server, None, json!({ "jsonrpc": "2.0", "id": 9, "method": "ping" })).await;
    assert_eq!(pong["result"], json!({}), "ping antwortet leer: {pong}");
}

#[tokio::test]
async fn the_work_argument_takes_an_id_or_a_slug() {
    let server = common::open().await;
    let works = ok(&server, None, "list_works", json!({})).await;
    let list = works["works"].as_array().unwrap();
    assert_eq!(list.len(), 1, "Beispieldaten haben genau ein Werk: {works}");
    assert_eq!(list[0]["id"], server.state.work, "id des Vorgabewerks");
    assert_eq!(
        list[0]["title"], "Beispielwerk: Leise Abhängigkeit (Auszug)",
        "Titel des gesäten Beispielwerks: {works}"
    );
    let slug = slug(&server).await;
    let by_id = ok(&server, None, "outline", json!({ "work": server.state.work })).await;
    let by_slug = ok(&server, None, "outline", json!({ "work": slug })).await;
    assert_eq!(by_id, by_slug, "id und slug meinen dasselbe Werk");
    let text = refused(&server, None, "outline", json!({ "work": "gibt-es-nicht" })).await;
    assert!(text.contains("Unknown work"), "deutscher Text für unbekannten slug: {text}");
    let text = refused(&server, None, "outline", json!({ "work": 999 })).await;
    assert!(text.contains("Unknown work 999"), "deutscher Text für unbekannte id: {text}");
}

#[tokio::test]
async fn outline_lists_every_node_with_current_revision_and_open_proposals() {
    let server = common::open().await;
    let outline = ok(&server, None, "outline", json!({ "work": server.state.work })).await;
    let nodes = outline["nodes"].as_array().unwrap();
    assert_eq!(nodes.len(), 11, "Beispielwerk hat 11 Abschnitte: {outline}");
    let intro = &nodes[2];
    assert_eq!(intro["title"], "Einleitung", "seq 2 ist die Einleitung: {intro}");
    assert_eq!(intro["number"], "1", "Gliederungsnummer der Einleitung: {intro}");
    assert_eq!(intro["parent"], 1, "unter Teil I");
    assert_eq!(intro["kind"], "chapter", "die Einleitung ist ein Kapitel: {intro}");
    assert_eq!(intro["current_rev"], 2, "Revision 2 ist aktuell (Nummer der Fassung, nicht ihre Stelle)");
    assert_eq!(intro["proposals"], 1, "ein offener Vorschlag des Lektors");
    assert_eq!(nodes[1]["kind"], "part", "Teil I ist ein Teil");
    assert!(nodes[1]["parent"].is_null(), "Teile stehen oben");
}

#[tokio::test]
async fn read_node_names_the_existing_numbers_after_a_deletion() {
    let server = common::open().await;
    let slug = slug(&server).await;
    let work = server.state.store.default_work().await.unwrap();
    // Einleitung (seq 2): 1, 2 (aktuell), 3 (Vorschlag); 4 speichern, dann 3 löschen → 1, 2, 4.
    server.state.store.save_human(work, 2, "= Einleitung\n\nVierte Fassung.\n", "Autor").await.unwrap().unwrap();
    server.state.store.delete_revision(work, 2, 3, "Autor").await.unwrap();
    let text = refused(&server, None, "read_node", json!({ "work": slug, "node": 2, "rev": 3 })).await;
    assert!(
        text.contains("Fassung 3 gibt es nicht; vorhanden: 1, 2, 4"),
        "MCP nennt dieselbe Liste wie RPC, mit der Lücke: {text}"
    );
}

#[tokio::test]
async fn read_node_returns_the_current_text_all_revisions_and_rev_selects_one() {
    let server = common::open().await;
    let slug = slug(&server).await;
    let node = ok(&server, None, "read_node", json!({ "work": slug, "node": 2 })).await;
    assert_eq!(node["rev"], 2, "ohne rev die aktuelle Revision: {node}");
    assert!(node["text"].as_str().unwrap().contains("in Beziehungen"), "Text der Revision 2: {node}");
    let revisions = node["revisions"].as_array().unwrap();
    let statuses: Vec<&str> = revisions.iter().map(|r| r["status"].as_str().unwrap()).collect();
    assert_eq!(statuses, ["superseded", "current", "proposed"], "alle Revisionen mit Status");
    assert_eq!(revisions[0]["author"]["kind"], "human", "Revision 1 vom Menschen: {node}");
    assert!(revisions[0]["author"].get("model").is_none(), "Mensch ohne Modell: {node}");
    assert_eq!(
        revisions[2]["author"],
        json!({ "kind": "assistant", "name": "@lektor", "model": "google/gemma-4-26b-a4b" }),
        "Vorschlag des Lektors mit Modell: {node}"
    );
    assert!(revisions[2]["note"].as_str().unwrap().contains("Zielgruppe"), "Notiz des Vorschlags: {node}");

    let first = ok(&server, None, "read_node", json!({ "work": slug, "node": 2, "rev": 1 })).await;
    assert_eq!(first["rev"], 1, "rev wählt die Revision nach ihrer Nummer: {first}");
    assert!(!first["text"].as_str().unwrap().contains("in Beziehungen"), "rev 1 ist der alte Text: {first}");
    let text = refused(&server, None, "read_node", json!({ "work": slug, "node": 2, "rev": 9 })).await;
    assert!(
        text.contains("Fassung 9 gibt es nicht; vorhanden: 1, 2, 3"),
        "deutscher Text für unbekannte Revision mit den vorhandenen Nummern: {text}"
    );
    let text = refused(&server, None, "read_node", json!({ "work": slug, "node": 99 })).await;
    assert!(text.contains("unbekannter Abschnitt 99"), "deutscher Text für unbekannten Abschnitt: {text}");
}

#[tokio::test]
async fn a_proposal_from_a_session_is_proposed_and_attributed_to_the_client() {
    let server = common::open().await;
    let session = initialize(&server, "Claude Code", "2.1.0").await;
    let before = server.rpc("node.get", json!({ "node": 4 })).await["result"].clone();
    let body = "== Erste Anzeichen\n\nAbhängigkeit beginnt leise.\n";
    let signs = key(&server, server.state.work, 4).await;

    let made = ok(
        &server,
        Some(&session),
        "propose_revision",
        json!({ "work": server.state.work, "key": signs, "body": body, "note": "kürzer" }),
    )
    .await;
    assert_eq!(made["status"], "proposed", "MCP legt nur Vorschläge an: {made}");
    assert_eq!(made["author"], "@mcp:claude-code", "Autor aus clientInfo.name: {made}");
    assert_eq!(made["model"], "Claude Code 2.1.0", "Modell aus Name und Version: {made}");

    let after = server.rpc("node.get", json!({ "node": 4 })).await["result"].clone();
    let revisions = after["revisions"].as_array().unwrap();
    let new = revisions.last().unwrap();
    assert_eq!(new["number"], made["rev"], "neue Revision steht am Ende");
    assert_eq!(new["status"], "proposed", "gespeichert als Vorschlag: {new}");
    assert_eq!(new["body"], body, "Text des Vorschlags unverändert gespeichert");
    assert_eq!(new["note"], "kürzer", "Notiz des Vorschlags gespeichert");
    assert_eq!(
        new["author"],
        json!({ "kind": "assistant", "persona": "@mcp:claude-code", "model": "Claude Code 2.1.0" }),
        "Autor ist der Client als Assistent mit Modell: {new}"
    );
    assert_eq!(common::currents(&after), 1, "genau eine current");
    let current = |v: &Value| v["revisions"].as_array().unwrap().iter().find(|r| r["status"] == "current").cloned();
    assert_eq!(current(&after), current(&before), "die aktuelle Revision bleibt unverändert");

    let made = ok(&server, Some(&session), "propose_revision",
        json!({ "work": server.state.work, "key": signs, "body": "== Erste Anzeichen\n\nAnders.\n", "model": "claude-opus" }))
    .await;
    assert_eq!(made["model"], "claude-opus", "Modell aus dem Argument geht vor: {made}");
}

#[tokio::test]
async fn without_a_session_the_client_is_unknown() {
    let server = common::open().await;
    let signs = key(&server, server.state.work, 4).await;
    let made = ok(
        &server,
        Some("keine-solche-sitzung"),
        "propose_revision",
        json!({ "work": server.state.work, "key": signs, "body": "== Erste Anzeichen\n\nNeu.\n" }),
    )
    .await;
    assert_eq!(made["author"], "@mcp:unbekannt", "unbekannte Sitzung → Client unbekannt: {made}");
    assert_eq!(made["model"], "unbekannt", "ohne clientInfo kein Modell: {made}");
}

#[tokio::test]
async fn a_proposal_with_a_persona_uses_its_handle_and_bad_proposals_are_refused() {
    let server = common::open().await;
    let session = initialize(&server, "claude-code", "1").await;
    let work = server.state.work;
    let signs = key(&server, work, 4).await;
    let made = ok(
        &server,
        Some(&session),
        "propose_revision",
        json!({ "work": work, "key": signs, "body": "== Erste Anzeichen\n\nLektor.\n", "persona": "@lektor" }),
    )
    .await;
    assert_eq!(made["author"], "@lektor", "Persona-Handle als Autor: {made}");
    let node = server.rpc("node.get", json!({ "node": 4 })).await["result"].clone();
    assert_eq!(
        node["revisions"].as_array().unwrap().last().unwrap()["author"]["persona"],
        "@lektor",
        "gespeichert mit der Persona als Autor: {node}"
    );

    let text = refused(
        &server,
        Some(&session),
        "propose_revision",
        json!({ "work": work, "key": signs, "body": "x", "persona": "@niemand" }),
    )
    .await;
    assert!(text.contains("Unknown persona"), "deutscher Text für unbekannte Persona: {text}");
    let text =
        refused(&server, Some(&session), "propose_revision", json!({ "work": "nirgends", "key": signs, "body": "x" }))
            .await;
    assert!(text.contains("Unknown work"), "unbekanntes Werk: {text}");
    let text =
        refused(&server, Some(&session), "propose_revision", json!({ "work": work, "key": 99_999, "body": "x" })).await;
    assert!(text.contains("gibt es nicht mehr"), "unbekannter Schlüssel: {text}");
    let text =
        refused(&server, Some(&session), "propose_revision", json!({ "work": work, "key": signs, "body": "  \n" }))
            .await;
    assert!(text.contains("Empty text"), "leerer Text: {text}");
    let current = server.rpc("node.get", json!({ "node": 4 })).await["result"]["revisions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["status"] == "current")
        .unwrap()["body"]
        .as_str()
        .unwrap()
        .replace('\n', "\r\n");
    let text =
        refused(&server, Some(&session), "propose_revision", json!({ "work": work, "key": signs, "body": current }))
            .await;
    assert!(text.contains("equals the current revision"), "gleicher Text (auch mit CRLF) wird abgelehnt: {text}");
    let count = server.rpc("node.get", json!({ "node": 4 })).await["result"]["revisions"].as_array().unwrap().len();
    assert_eq!(count, 3, "nur der Lektor-Vorschlag kam dazu (2 + 1), keine der Ablehnungen");
}

#[tokio::test]
async fn search_uses_the_library_search_over_all_works_or_one() {
    let server = common::open().await;
    let all = ok(&server, None, "search", json!({ "query": "langmütig" })).await;
    let hits = all["hits"].as_array().unwrap();
    assert!(hits.iter().any(|h| h["kind"] == "revision" && h["node"] == 4), "Treffer im Werk (2.1): {all}");
    assert!(hits.iter().any(|h| h["kind"] == "title" || h["kind"] == "quote"), "Treffer in der Bibliothek: {all}");
    let one = ok(
        &server,
        None,
        "search",
        json!({ "query": "langmütig", "work": slug(&server).await, "kinds": ["revision"] }),
    )
    .await;
    assert!(one["hits"].as_array().unwrap().iter().all(|h| h["kind"] == "revision"), "kinds filtert: {one}");
    assert!(!one["hits"].as_array().unwrap().is_empty(), "mit work weiter Treffer: {one}");
    let text = refused(&server, None, "search", json!({ "query": "  " })).await;
    assert!(!text.is_empty(), "leere Suche ist ein fachlicher Fehler mit Text");
}

#[tokio::test]
async fn add_post_writes_a_post_by_persona_or_client_and_checks_refs() {
    let server = common::open().await;
    let session = initialize(&server, "Claude Code", "2").await;
    let work = server.state.work;
    let posts = |v: &Value| v["result"]["posts"].as_array().unwrap().clone();
    let before = posts(&server.rpc("node.get", json!({ "node": 5 })).await).len();
    let (signs, pious) = (key(&server, work, 4).await, key(&server, work, 5).await);

    let made = ok(
        &server,
        Some(&session),
        "add_post",
        json!({ "work": work, "key": pious, "body": "Siehe 2.1.",
        "refs": [{ "key": signs, "rev": 2, "quote": "Langmut heißt nicht, Unrecht zu dulden." }],
        "sources": ["1. Kor 13,4"] }),
    )
    .await;
    assert_eq!(made["author"], "@mcp:claude-code", "Autor ist der Client: {made}");
    let list = posts(&server.rpc("node.get", json!({ "node": 5 })).await);
    assert_eq!(list.len(), before + 1, "ein Beitrag mehr");
    let post = list.last().unwrap();
    assert_eq!(post["author"], "@mcp:claude-code", "gespeicherter Beitrag nennt den Client: {post}");
    assert_eq!(post["refs"][0]["node"], 4, "Verweis gespeichert: {post}");
    assert_eq!(post["sources"][0], "1. Kor 13,4", "Quelle gespeichert: {post}");

    let made = ok(
        &server,
        Some(&session),
        "add_post",
        json!({ "work": work, "key": pious, "body": "Befund.", "persona": "@nathan" }),
    )
    .await;
    assert_eq!(made["author"], "@nathan", "Persona als Autor: {made}");

    let bad = [
        (json!([{ "key": 99_999, "rev": 1, "quote": "x" }]), "does not exist (any more)"),
        (
            json!([{ "key": signs, "rev": 9, "quote": "x" }]),
            "revision 9 of section 4: it does not exist (existing: 1, 2)",
        ),
        (json!([{ "key": signs, "rev": 1, "quote": "Langmut heißt nicht" }]), "not literally"),
    ];
    for (refs, why) in bad {
        let text = refused(
            &server,
            Some(&session),
            "add_post",
            json!({ "work": work, "key": pious, "body": "x", "refs": refs }),
        )
        .await;
        assert!(text.contains(why), "Verweis geprüft ({why}): {text}");
    }
    let text = refused(&server, Some(&session), "add_post", json!({ "work": work, "key": pious, "body": "   " })).await;
    assert!(text.contains("Leerer Beitrag"), "leerer Beitrag: {text}");
    let text = refused(
        &server,
        Some(&session),
        "add_post",
        json!({ "work": work, "key": pious, "body": "x", "persona": "@niemand" }),
    )
    .await;
    assert!(text.contains("Unknown persona"), "unbekannte Persona: {text}");
    assert_eq!(
        posts(&server.rpc("node.get", json!({ "node": 5 })).await).len(),
        before + 2,
        "Ablehnungen schreiben nichts"
    );
}

#[tokio::test]
async fn list_personas_comes_from_sqlite_and_marks_the_lector() {
    let server = common::open().await;
    sqlx::query("UPDATE persona SET active = 0 WHERE handle = '@lektor'")
        .execute(server.state.store.pool())
        .await
        .unwrap();
    let answer = ok(&server, None, "list_personas", json!({})).await;
    let personas = answer["personas"].as_array().unwrap();
    assert_eq!(personas.len(), 6, "sechs Personas im Beispiel: {answer}");
    assert_eq!(personas[0]["handle"], "@lektor", "Reihenfolge nach position");
    assert_eq!(personas[0]["active"], false, "Änderung in SQLite ist sichtbar: {answer}");
    let lectors: Vec<&Value> = personas.iter().filter(|p| p["lector"] == true).collect();
    assert_eq!(lectors.len(), 1, "genau ein Lektor: {answer}");
    assert_eq!(lectors[0]["active"], true, "Lektor ist eine aktive Persona: {answer}");
    let max = personas.iter().filter(|p| p["active"] == true).map(|p| p["context_length"].as_u64().unwrap()).max();
    assert_eq!(lectors[0]["context_length"].as_u64(), max, "Lektor hat den größten Kontext der aktiven");
    assert_eq!(answer["lector"], lectors[0]["handle"], "lector oben nennt dieselbe Persona wie die Markierung");
}

#[tokio::test]
async fn import_typst_is_no_longer_a_tool_and_imports_nothing() {
    // Ein Import legt aktuelle Revisionen unter dem Namen des Menschen an; das darf ein Assistent
    // nicht (Entscheidung 2026-11-05). Import nur noch über `baruch-server import`.
    let server = common::open().await;
    let (_, _, list) = post(&server, None, json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" })).await;
    let tools = list["result"]["tools"].as_array().unwrap();
    assert!(!tools.iter().any(|t| t["name"] == "import_typst"), "tools/list nennt import_typst nicht mehr: {list}");

    let path = server.path().join("gartenbuch.typ");
    std::fs::write(&path, common::GARDEN).unwrap();
    for arguments in [json!({ "source": common::GARDEN, "name": "gartenbuch.typ" }), json!({ "path": path })] {
        let answer = call(&server, None, "import_typst", arguments).await;
        assert_eq!(answer["error"]["code"], -32602, "import_typst ist ein unbekanntes Werkzeug (-32602): {answer}");
        assert!(
            answer["error"]["message"].as_str().unwrap().contains("unknown tool import_typst"),
            "Meldung nennt das unbekannte Werkzeug: {answer}"
        );
    }
    let works = ok(&server, None, "list_works", json!({})).await;
    assert_eq!(works["works"].as_array().unwrap().len(), 1, "kein Werk angelegt, nur das Beispielwerk: {works}");
    let (_, _, init) = post(
        &server,
        None,
        json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": { "clientInfo": { "name": "x" } } }),
    )
    .await;
    let instructions = init["result"]["instructions"].as_str().unwrap();
    assert!(!instructions.contains("import_typst"), "instructions bieten keinen Import an: {instructions}");
    assert!(
        instructions.contains("the import do not exist over MCP"),
        "instructions sagen es ausdrücklich: {instructions}"
    );
}

/// Anfrage mit fremdem `Origin` vom eigenen Rechner: der guard lehnt vor dem MCP-Endpunkt ab.
async fn post_from_origin(server: &common::Server, origin: &str, message: Value) -> StatusCode {
    let mut request = Request::post("/mcp")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::ORIGIN, origin)
        .body(Body::from(message.to_string()))
        .unwrap();
    request.extensions_mut().insert(ConnectInfo(SocketAddr::from(LOOPBACK)));
    server.send(request).await.0
}

#[tokio::test]
async fn a_client_reads_and_proposes_in_an_imported_work_and_the_accepted_proposal_is_committed() {
    let server = common::open().await;
    // Das Werk kommt wie beim Nutzer aus dem Import (CLI-Weg), nicht über MCP.
    let made = server.import_garden().await;
    let work = json!(made.work);
    let slug = made.slug.clone();
    let dir = server.mirror_dir_of(&slug);
    assert!(made.commit.is_some(), "der Import committet im Spiegel des neuen Werks: {made:?}");

    // Loopback ohne Origin: initialize und tools/list gehen durch.
    let session = initialize(&server, "Claude Code", "2.1.0").await;
    let (status, _, list) =
        post(&server, Some(&session), json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" })).await;
    assert_eq!(status, StatusCode::OK, "tools/list über Loopback ohne Origin: {list}");

    // Fremder Origin auf demselben Rechner (Webseite im Browser): abgelehnt, nichts vorgeschlagen.
    let status = post_from_origin(
        &server,
        "http://evil.example",
        json!({ "jsonrpc": "2.0", "id": 3, "method": "tools/call",
        "params": { "name": "propose_revision", "arguments": { "work": work, "key": key(&server, made.work, 1).await, "body": "== Böse\n" } } }),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "fremder Origin wird vor dem Werkzeug abgelehnt");
    let node = ok(&server, Some(&session), "read_node", json!({ "work": work, "node": 1 })).await;
    assert_eq!(node["revisions"].as_array().unwrap().len(), 1, "der abgelehnte Aufruf legte nichts an: {node}");

    // Gliederung: Unterabschnitt „Tiefe“ hängt unter „Aussaat“, nummeriert 1.1.1.
    let outline = ok(&server, Some(&session), "outline", json!({ "work": slug })).await;
    let nodes = outline["nodes"].as_array().unwrap();
    let titles: Vec<&str> = nodes.iter().map(|n| n["title"].as_str().unwrap()).collect();
    assert_eq!(titles, ["Frühling", "Aussaat", "Tiefe", "Sommer"], "Gliederung in Dateireihenfolge: {outline}");
    assert_eq!(nodes[2]["number"], "1.1.1", "hierarchische Nummer: {outline}");
    assert_eq!(nodes[2]["parent"], 1, "Tiefe steht unter Aussaat: {outline}");
    assert_eq!(nodes[1]["current_rev"], 1, "Revision 1 ist aktuell: {outline}");

    // Lesen: Revision 1 vom Menschen, aktuell, Text ab der Überschriftszeile (ohne Import-Zeile des Spiegels).
    assert_eq!(node["text"], "== Aussaat\n\n#beet[Radieschen & Möhren.]\n\n", "Text des Abschnitts: {node}");
    assert_eq!(node["revisions"][0]["status"], "current", "die importierte Revision ist aktuell: {node}");
    assert_eq!(node["revisions"][0]["author"]["kind"], "human", "der Import schreibt als Mensch: {node}");

    // Vorschlag über MCP: bleibt proposed, bis der Mensch per RPC übernimmt.
    let body = "== Aussaat\n\n#beet[Radieschen, Möhren und Dill.]\n\n";
    let proposed = ok(
        &server,
        Some(&session),
        "propose_revision",
        json!({ "work": slug, "key": nodes[1]["key"], "body": body, "note": "Dill ergänzt" }),
    )
    .await;
    assert_eq!(proposed["status"], "proposed", "MCP-Aufrufer schreiben nur Vorschläge: {proposed}");
    assert_eq!(proposed["rev"], 2, "der Vorschlag bekommt die nächste Nummer: {proposed}");
    let head_before = common::git(&dir, &["rev-parse", "HEAD"]);

    let accepted = server.rpc("revision.accept", json!({ "work": work, "node": 1, "rev": 2 })).await;
    let commit = accepted["result"]["commit"].as_str().unwrap_or_else(|| panic!("kein Commit gemeldet: {accepted}"));
    let node = ok(&server, Some(&session), "read_node", json!({ "work": work, "node": 1 })).await;
    assert_eq!(node["rev"], 2, "der übernommene Vorschlag ist aktuell: {node}");
    let statuses: Vec<&str> =
        node["revisions"].as_array().unwrap().iter().map(|r| r["status"].as_str().unwrap()).collect();
    assert_eq!(statuses, ["superseded", "current"], "genau eine current, die alte superseded");

    // Spiegel des importierten Werks: Datei mit Import der Vorlage und dem neuen Text, Commit mit dem
    // MCP-Client als Autor.
    let file = std::fs::read_dir(dir.join("abschnitte"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .find(|p| p.file_name().unwrap().to_string_lossy().starts_with("001-"))
        .expect("Datei für Abschnitt 1 fehlt im Spiegel");
    assert_eq!(
        baruch_typst::xref::strip_key_labels(&std::fs::read_to_string(file).unwrap()).into_owned(),
        format!("#import \"../vorlage.typ\": *\n{body}"),
        "Spiegel trägt den übernommenen Text hinter der Import-Zeile der Werkvorlage"
    );
    let Some(head_before) = head_before else {
        eprintln!("git fehlt – Commit-Autor nicht geprüft");
        return;
    };
    let head = common::git(&dir, &["rev-parse", "HEAD"]).unwrap();
    assert_eq!(head.trim(), commit, "HEAD ist der gemeldete Commit");
    assert_ne!(head, head_before, "die Übernahme ist ein neuer Commit");
    let log = common::git(&dir, &["log", "-1", "--format=%an <%ae>%n%s"]).unwrap();
    assert!(log.contains("@mcp:claude-code") && log.contains("mcp-claude-code@"), "Autor ist der MCP-Client: {log}");
    assert!(log.contains("Vorschlag 2 übernommen"), "Betreff nennt die Übernahme: {log}");
}

#[tokio::test]
async fn a_proposal_that_would_change_the_outline_is_refused_and_nothing_is_stored() {
    let server = common::open().await;
    let garden = server.import_garden().await;
    let work = json!(garden.work);
    let summer = key(&server, garden.work, 3).await;
    // Befund Runde 4: Sommer (Kapitel) als Abschnitt plus ein neues Kapitel „Herbst“.
    let restructure = "== Sommer\n\nText C.\n\n= Herbst\n\nText D.\n";
    let text =
        refused(&server, None, "propose_revision", json!({ "work": work, "key": summer, "body": restructure })).await;
    assert!(text.contains("„Herbst“"), "die Meldung nennt die zusätzliche Überschrift: {text}");
    let level =
        refused(&server, None, "propose_revision", json!({ "work": work, "key": summer, "body": "== Sommer\n\nC.\n" }))
            .await;
    assert!(level.contains("Ebene 2") && level.contains("Ebene 1"), "die Meldung nennt beide Ebenen: {level}");
    let bare =
        refused(&server, None, "propose_revision", json!({ "work": work, "key": summer, "body": "Nur Text.\n" })).await;
    assert!(bare.contains("keine Überschrift"), "ein Text ohne Überschrift ist kein Vorschlag: {bare}");
    let node = ok(&server, None, "read_node", json!({ "work": work, "node": 3 })).await;
    assert_eq!(node["revisions"].as_array().unwrap().len(), 1, "keiner der Vorschläge wurde angelegt: {node}");
    // Ein neuer Titel derselben Ebene ist erlaubt und benennt beim Übernehmen um.
    let renamed =
        ok(&server, None, "propose_revision", json!({ "work": work, "key": summer, "body": "= Hochsommer\n\nC.\n" }))
            .await;
    let accepted = server.rpc("revision.accept", json!({ "work": work, "node": 3, "rev": renamed["rev"] })).await;
    assert!(accepted.get("error").is_none(), "Übernehmen gelingt: {accepted}");
    let outline = ok(&server, None, "outline", json!({ "work": work })).await;
    assert_eq!(outline["nodes"][3]["title"], "Hochsommer", "gleiche Ebene, neuer Titel → umbenannt: {outline}");
    assert_eq!(outline["nodes"].as_array().unwrap().len(), 4, "die Gliederung behält ihre vier Punkte");
}

#[tokio::test]
async fn a_client_with_an_old_outline_writes_through_the_key_into_the_right_section() {
    let server = common::open().await;
    let work = json!(server.import_garden().await.work);
    let outline = ok(&server, None, "outline", json!({ "work": work })).await;
    let summer = outline["nodes"][3]["key"].as_i64().expect("outline nennt den festen Schlüssel");
    assert_eq!(outline["nodes"][3]["title"], "Sommer", "Testaufbau: an Stelle 3 steht „Sommer“: {outline}");
    // Der Mensch stellt um: „Sommer“ nach vorn, an Stelle 3 steht jetzt „Tiefe“.
    let moved = server.rpc("outline.move", json!({ "work": work, "node": 3, "target": 0, "place": "before" })).await;
    assert!(moved.get("error").is_none(), "Testaufbau: Umstellen gelingt: {moved}");
    let stale = call(
        &server,
        None,
        "propose_revision",
        json!({ "work": work, "node": 3, "key": summer, "body": "= Sommer\n\nGießen am Abend.\n" }),
    )
    .await;
    assert_eq!(stale["error"]["code"], -32602, "die veraltete Stelle wird gar nicht erst angenommen: {stale}");
    let made = ok(
        &server,
        None,
        "propose_revision",
        json!({ "work": work, "key": summer, "body": "= Sommer\n\nGießen am Abend.\n" }),
    )
    .await;
    assert_eq!(made["node"], 0, "der Schlüssel findet „Sommer“ an seiner neuen Stelle 0: {made}");
    let post = ok(
        &server,
        None,
        "add_post",
        json!({ "work": work, "key": summer, "body": "Abends ist besser.",
                "refs": [{ "key": summer, "rev": 1, "quote": "Gießen, gießen" }] }),
    )
    .await;
    assert_eq!(post["node"], 0, "Beitrag über den Schlüssel bei „Sommer“: {post}");
    let deep = ok(&server, None, "read_node", json!({ "work": work, "node": 3 })).await;
    assert_eq!(deep["title"], "Tiefe", "an Stelle 3 steht „Tiefe“");
    assert_eq!(deep["revisions"].as_array().unwrap().len(), 1, "„Tiefe“ bekam keinen Vorschlag: {deep}");
    let summer_now = ok(&server, None, "read_node", json!({ "work": work, "key": summer })).await;
    assert_eq!(summer_now["revisions"][1]["status"], "proposed", "der Vorschlag steht bei „Sommer“: {summer_now}");
    let gone =
        refused(&server, None, "propose_revision", json!({ "work": work, "key": 999_999, "body": "= X\n" })).await;
    assert!(gone.contains("gibt es nicht mehr"), "unbekannter Schlüssel → klare Meldung: {gone}");
}

// ---- key ist Pflicht beim Schreiben (Vertrag Runde 5) -----------------------------------------

#[tokio::test]
async fn writing_tools_need_the_key_and_refuse_the_position() {
    let server = common::open().await;
    let work = server.state.work;
    let (signs, pious) = (key(&server, work, 4).await, key(&server, work, 5).await);
    let body = "== Erste Anzeichen\n\nOhne Schlüssel.\n";
    let cases = [
        ("propose_revision", json!({ "work": work, "node": 4, "body": body }), "node (position) is not accepted"),
        (
            "propose_revision",
            json!({ "work": work, "node": 4, "key": signs, "body": body }),
            "node (position) is not accepted",
        ),
        ("propose_revision", json!({ "work": work, "body": body }), "key is missing"),
        ("add_post", json!({ "work": work, "node": 5, "body": "Beitrag" }), "node (position) is not accepted"),
        ("add_post", json!({ "work": work, "body": "Beitrag" }), "key is missing"),
        (
            "add_post",
            json!({ "work": work, "key": pious, "body": "Beitrag",
                    "refs": [{ "node": 4, "rev": 2, "quote": "Langmut heißt nicht, Unrecht zu dulden." }] }),
            "refs[0]: node (position) is not accepted",
        ),
        (
            "add_post",
            json!({ "work": work, "key": pious, "body": "Beitrag",
                    "refs": [{ "key": signs, "rev": 2, "quote": "Langmut" }, { "rev": 2, "quote": "Langmut" }] }),
            "refs[1]: key is missing",
        ),
    ];
    for (tool, arguments, why) in cases {
        let answer = call(&server, None, tool, arguments.clone()).await;
        assert_eq!(
            answer["error"]["code"], -32602,
            "{tool} {arguments}: JSON-RPC -32602 statt eines Ergebnisses: {answer}"
        );
        let message = answer["error"]["message"].as_str().unwrap_or_default();
        assert!(message.contains(why), "{tool}: deutscher Hinweis „{why}“: {message}");
    }
    let node = |seq: usize| server.rpc("node.get", json!({ "node": seq }));
    let signs_after = node(4).await;
    assert_eq!(
        signs_after["result"]["revisions"].as_array().unwrap().len(),
        2,
        "kein Vorschlag angelegt: {signs_after}"
    );
    let pious_after = node(5).await;
    assert_eq!(pious_after["result"]["posts"].as_array().unwrap().len(), 2, "kein Beitrag angelegt: {pious_after}");
    // Lesen bleibt wie es war: read_node nimmt weiter die Stelle.
    let read = ok(&server, None, "read_node", json!({ "work": work, "node": 4 })).await;
    assert_eq!(read["key"], signs, "read_node über die Stelle nennt den Schlüssel: {read}");
}

#[tokio::test]
async fn the_input_schema_of_writing_tools_requires_the_key_and_has_no_node() {
    let server = common::open().await;
    let (_, _, answer) = post(&server, None, json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" })).await;
    let tools = answer["result"]["tools"].as_array().unwrap();
    let schema = |name: &str| tools.iter().find(|t| t["name"] == name).unwrap()["inputSchema"].clone();
    for name in ["propose_revision", "add_post"] {
        let s = schema(name);
        assert_eq!(s["required"], json!(["work", "key", "body"]), "{name}: work, key und body sind Pflicht: {s}");
        assert!(s["properties"].get("node").is_none(), "{name}: kein node im Schema: {s}");
        assert_eq!(s["properties"]["key"]["type"], "integer", "{name}: key ist eine Zahl: {s}");
    }
    let refs = &schema("add_post")["properties"]["refs"]["items"];
    assert_eq!(refs["required"], json!(["key", "rev", "quote"]), "jeder Verweis braucht key: {refs}");
    assert!(refs["properties"].get("node").is_none(), "Verweise ohne node: {refs}");
    let read = schema("read_node");
    assert!(
        read["properties"].get("node").is_some() && read["properties"].get("key").is_some(),
        "read_node bleibt unverändert (node oder key): {read}"
    );
}
