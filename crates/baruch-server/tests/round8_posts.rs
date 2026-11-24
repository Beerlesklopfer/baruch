// SPDX-License-Identifier: AGPL-3.0-or-later
//! Abnahme Runde 8, Aufgabe „posts“ (Vertrag §1 und §3): Adressen über den festen Schlüssel,
//! Beiträge löschen, verschieben und in Einfälle umwandeln (RPC, Formular ohne JavaScript, MCP nur
//! eigene), Chat-Abzeichen, MCP-Antworten mit `url`. Echter Datenordner im Temp-Ordner (SQLite,
//! Bibliothek, Git-Spiegel), eigenes Gartenbuch aus `tests/data/gartenbuch.typ`; Testtexte frei erfunden.

use crate::common;

use std::net::SocketAddr;

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode, header};
use serde_json::{Value, json};

/// Gartenbuch: 0 Frühling, 1 Aussaat, 2 Tiefe, 3 Sommer. Gibt Server, Werk, Schlüssel je Stelle und
/// den Spiegelordner zurück.
async fn garden() -> (common::Server, i64, Vec<i64>, std::path::PathBuf) {
    let server = common::open().await;
    let report = server.import_garden().await;
    let keys = keys_of(&server, report.work).await;
    let dir = server.mirror_dir_of(&report.slug);
    (server, report.work, keys, dir)
}

/// Feste Schlüssel nach Stelle.
async fn keys_of(server: &common::Server, work: i64) -> Vec<i64> {
    sqlx::query_scalar("SELECT id FROM node WHERE work_id = ? ORDER BY seq")
        .bind(work)
        .fetch_all(server.state.store.pool())
        .await
        .unwrap()
}

fn result(answer: &Value) -> &Value {
    assert!(answer.get("error").is_none(), "RPC-Fehler: {answer}");
    &answer["result"]
}

/// Beitrag über RPC anlegen; gibt seine id zurück.
async fn add_post(server: &common::Server, work: i64, key: i64, body: &str) -> i64 {
    let added = server.rpc("post.add", json!({ "work": work, "key": key, "body": body })).await;
    result(&added)["id"].as_i64().unwrap_or_else(|| panic!("post.add nennt keine id: {added}"))
}

/// Beiträge eines Abschnitts als `(id, author, body)`.
async fn posts_at(server: &common::Server, work: i64, key: i64) -> Vec<(i64, String, String)> {
    let node = server.rpc("node.get", json!({ "work": work, "key": key })).await;
    result(&node)["posts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| {
            (
                p["id"].as_i64().unwrap(),
                p["author"].as_str().unwrap().to_owned(),
                p["body"].as_str().unwrap().to_owned(),
            )
        })
        .collect()
}

fn commit_count(dir: &std::path::Path) -> Option<u32> {
    common::git(dir, &["rev-list", "--count", "HEAD"]).map(|c| c.trim().parse().unwrap())
}

/// Betreff, Rumpf und Trailer des letzten Commits.
fn last_message(dir: &std::path::Path) -> Option<String> {
    common::git(dir, &["log", "-1", "--format=%an <%ae>%n%B"])
}

/// Alle `href="…"` einer Seite.
fn hrefs(html: &str) -> Vec<&str> {
    html.split("href=\"").skip(1).filter_map(|rest| rest.split('"').next()).collect()
}

// ---- Adressen über den festen Schlüssel (§1) ----------------------------------------------------

#[tokio::test]
async fn a_key_address_opens_its_section_even_after_the_outline_changes() {
    let (server, work, keys, _) = garden().await;
    let (status, html) = server.get(&format!("/?work={work}&key={}", keys[3])).await;
    assert_eq!(status, StatusCode::OK, "{html}");
    assert!(html.contains("Gießen, gießen, gießen."), "key öffnet „Sommer“: {html}");
    // „Sommer“ nach vorn: alle Stellen verschieben sich, der Schlüssel bleibt.
    let moved = server.rpc("outline.move", json!({ "work": work, "node": 3, "target": 0, "place": "before" })).await;
    result(&moved);
    assert_eq!(moved["result"]["key"], keys[3], "die Gliederungsantwort nennt den Schlüssel (Runde 8): {moved}");
    let (_, html) = server.get(&format!("/?work={work}&key={}", keys[3])).await;
    assert!(html.contains("Gießen, gießen, gießen."), "derselbe Link zeigt „Sommer“ an seiner neuen Stelle 0: {html}");
    let (_, html) = server.get(&format!("/?work={work}&key={}&node=2", keys[3])).await;
    assert!(html.contains("Gießen, gießen, gießen."), "der Schlüssel geht vor einer (veralteten) Stelle");
    assert!(!html.contains("missing-key"), "kein Hinweis bei bekanntem Schlüssel");
    // Die alte Form ?node= geht weiter.
    let (status, html) = server.get(&format!("/?work={work}&node=0")).await;
    assert_eq!(status, StatusCode::OK, "{html}");
    assert!(html.contains("Gießen, gießen, gießen."), "/?node=0 ist jetzt „Sommer“");
}

#[tokio::test]
async fn an_unknown_key_falls_back_to_node_or_the_default_and_says_so() {
    let (server, work, _, _) = garden().await;
    let (status, html) = server.get(&format!("/?work={work}&key=99999&node=2")).await;
    assert_eq!(status, StatusCode::OK, "kein Fehler, sondern Rückfall: {html}");
    assert!(html.contains("Zwei Zentimeter."), "Rückfall auf node=2 (Tiefe): {html}");
    assert!(html.contains("Abschnitt mit Schlüssel 99999 gibt es nicht (mehr)."), "Hinweis im Editor-Kopf: {html}");
    let (_, html) = server.get(&format!("/?work={work}&key=99999")).await;
    assert!(html.contains("Die Beete werden umgegraben."), "ohne node der Vorgabe-Abschnitt (Frühling): {html}");
    assert!(html.contains("Schlüssel 99999"), "auch dann mit Hinweis");
    let ui = server.rpc("ui.render", json!({ "work": work, "key": 99999, "node": 2 })).await;
    assert!(
        result(&ui)["editor"].as_str().unwrap().contains("Abschnitt mit Schlüssel 99999 gibt es nicht (mehr)."),
        "ui.render (mit JavaScript) zeigt denselben Hinweis: {ui}"
    );
}

#[tokio::test]
async fn ui_render_and_the_outline_pages_take_the_key() {
    let (server, work, keys, _) = garden().await;
    let ui = server.rpc("ui.render", json!({ "work": work, "key": keys[2] })).await;
    assert_eq!(
        result(&ui)["url"],
        format!("/?work={work}&key={}&rev=1", keys[2]),
        "ui.render nimmt key und antwortet mit der Key-Adresse"
    );
    let (status, html) = server.get(&format!("/gliederung?work={work}&key={}", keys[3])).await;
    assert_eq!(status, StatusCode::OK, "{html}");
    assert!(html.contains("<tr class=\"outline-entry level-1 selected\" id=\"punkt-3\""), "Sommer gewählt: {html}");
    let (status, html) = server.get(&format!("/gliederung/bearbeiten?work={work}&key={}", keys[1])).await;
    assert_eq!(status, StatusCode::OK, "{html}");
    assert!(html.contains("value=\"Aussaat\""), "Bearbeiten öffnet „Aussaat“ über den Schlüssel: {html}");
    let (status, html) = server.get(&format!("/literatur?work={work}&key={}", keys[3])).await;
    assert_eq!(status, StatusCode::OK, "{html}");
    assert!(
        html.contains(&format!("href=\"/?work={work}&amp;key={}\" data-back", keys[3])),
        "Literatur nimmt key, der Rückweg trägt ihn: {html}"
    );
}

#[tokio::test]
async fn no_generated_link_or_redirect_carries_node_any_more() {
    let (server, work, keys, _) = garden().await;
    // Beitrag mit Verweis (über MCP-Weg nicht nötig: Store direkt), Einfall mit Bezug, Vorschlag.
    let post = baruch_core::Post {
        id: 0,
        created: String::new(),
        author: "@lektor".into(),
        body: "Siehe Aussaat.".into(),
        sources: vec![],
        refs: vec![baruch_core::TextRef { node: 1, rev: 1, quote: "Radieschen".into() }],
    };
    server.state.store.add_post(work, 3, &post).await.unwrap();
    let idea = server.rpc("idea.create", json!({ "work": work, "key": keys[1], "words": "Möhren" })).await;
    let idea_id = result(&idea)["idea"]["id"].as_i64().unwrap();
    let proposal =
        server.state.store.add_proposal(work, 3, "= Sommer\n\nGießen am Abend.\n", "@lektor", "Testmodell", "");
    proposal.await.unwrap();

    let titles = baruch_library::list_titles(server.state.store.pool()).await.unwrap();
    let title = titles.first().expect("Beispielbibliothek hat Titel").title.id;
    let pages = [
        format!("/quelle?title={title}&work={work}&key={}&lq=Radieschen", keys[3]),
        format!("/quelle?title={title}&work={work}&node=3"),
        format!("/?work={work}&key={}", keys[3]),
        format!("/?work={work}&key={}&view=preview&lq=Radieschen", keys[3]),
        format!("/gliederung?work={work}&key={}", keys[1]),
        format!("/gliederung/bearbeiten?work={work}&key={}", keys[1]),
        format!("/literatur?work={work}&key={}&lq=Radieschen", keys[3]),
        format!("/einfaelle?work={work}"),
        format!("/einfaelle/{idea_id}"),
    ];
    for page in &pages {
        let (status, html) = server.get(page).await;
        assert_eq!(status, StatusCode::OK, "{page}: {html}");
        // Links des Arbeitsbereichs (eigene Dateien von posts); Formularfelder `name="node"` sind keine Links.
        let bad: Vec<&str> = hrefs(&html).into_iter().filter(|h| h.contains("node=")).collect();
        assert!(bad.is_empty(), "{page}: Links mit node= (Vertrag Runde 8 §1): {bad:?}");
    }
    let (_, html) = server.get(&format!("/?work={work}&key={}", keys[3])).await;
    let k3 = keys[3];
    assert!(
        html.contains(&format!("href=\"/?work={work}&key={k3}&rev=1&view=preview\"")),
        "Vorschau-Umschalter in Key-Form: {html}"
    );
    assert!(html.contains(&format!("href=\"/werke/{work}/satz?key={k3}\"")), "Link „Satz“ (Runde 8): {html}");
    assert!(html.contains(">Satz</a>"), "Knopf heißt „Satz“");
    assert!(
        html.contains(&format!("href=\"&#x2f;?work={work}&amp;key={k3}&amp;rev=2\"")),
        "Fassungsbaum „zeigen“ (Vorschlag 2) in Key-Form: {html}"
    );
    assert!(
        html.contains(&format!("href=\"&#x2f;?work={work}&amp;key={}&amp;rev=1\"", keys[1])),
        "Chat-Verweis auf Aussaat in Key-Form: {html}"
    );
    let (_, html) = server.get(&format!("/einfaelle/{idea_id}")).await;
    assert!(
        html.contains(&format!("href=\"/?work={work}&amp;key={}\"", keys[1])),
        "Einfall-Bezug (und „Zum Werk“) führt auf den Abschnitt in Key-Form: {html}"
    );
    let (_, html) = server.get(&format!("/literatur?work={work}&key={}&lq=Radieschen", keys[3])).await;
    assert!(
        html.contains(&format!("href=\"/?work={work}&amp;key={}&amp;rev=1&amp;lq=Radieschen#fundstelle\"", keys[1])),
        "Treffer im Werk über node_key: {html}"
    );

    // Weiterleitungen nach Formularen.
    let redirects = [
        ("/act", format!("method=post.add&work={work}&node=3&key={k3}&body=Gut")),
        ("/gliederung/act", format!("method=outline.rename&work={work}&node=1&key={}&title=Saat", keys[1])),
        ("/literatur/act", format!("action=similar&work={work}&key={k3}&text=Radieschen+und+M%C3%B6hren")),
        (
            "/einfaelle/act",
            format!("method=idea.create&work={work}&key={k3}&words=Hitze&back=%2F%3Fwork%3D{work}%26key%3D{k3}"),
        ),
    ];
    for (route, form) in &redirects {
        let (status, location) = server.post_form(route, form).await;
        assert_eq!(status, StatusCode::SEE_OTHER, "{route}: {form}");
        let location = location.unwrap();
        assert!(!location.contains("node="), "{route}: Weiterleitung ohne node=: {location}");
        if *route != "/literatur/act" {
            assert!(location.contains("key="), "{route}: Weiterleitung trägt den Schlüssel: {location}");
        }
    }
    let id = add_post(&server, work, k3, "Zum Löschen.").await;
    let (status, location) =
        server.post_form("/beitraege/act", &format!("method=post.delete&work={work}&key={k3}&id={id}&confirm=1")).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "/beitraege/act leitet weiter");
    assert_eq!(location, Some(format!("/?work={work}&key={k3}")), "zurück zum gezeigten Abschnitt in Key-Form");
}

// ---- MCP-Antworten mit url (§1) -----------------------------------------------------------------

const LOOPBACK: ([u8; 4], u16) = ([127, 0, 0, 1], 40000);

/// MCP-Anfrage vom eigenen Rechner, wahlweise mit Sitzung; gibt Kopfzeilen und Antwort zurück.
async fn mcp(server: &common::Server, session: Option<&str>, message: Value) -> (axum::http::HeaderMap, Value) {
    let mut request = Request::post("/mcp").header(header::CONTENT_TYPE, "application/json");
    if let Some(id) = session {
        request = request.header("mcp-session-id", id);
    }
    let mut request = request.body(Body::from(message.to_string())).unwrap();
    request.extensions_mut().insert(ConnectInfo(SocketAddr::from(LOOPBACK)));
    let (status, headers, body) = server.send(request).await;
    assert_eq!(status, StatusCode::OK, "MCP antwortet: {body}");
    let value = if body.is_empty() { Value::Null } else { serde_json::from_str(&body).unwrap() };
    (headers, value)
}

/// Sitzung als Client `name` (Handle `@mcp:<name>`).
async fn session(server: &common::Server, name: &str) -> String {
    let (headers, answer) = mcp(
        server,
        None,
        json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": { "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": { "name": name, "version": "1" } } }),
    )
    .await;
    assert!(answer["result"].is_object(), "initialize: {answer}");
    headers.get("mcp-session-id").unwrap().to_str().unwrap().to_owned()
}

async fn tool(server: &common::Server, session: &str, name: &str, arguments: Value) -> Value {
    let (_, answer) = mcp(
        server,
        Some(session),
        json!({ "jsonrpc": "2.0", "id": 7, "method": "tools/call", "params": { "name": name, "arguments": arguments } }),
    )
    .await;
    answer
}

async fn tool_ok(server: &common::Server, session: &str, name: &str, arguments: Value) -> Value {
    let answer = tool(server, session, name, arguments).await;
    assert_eq!(answer["result"]["isError"], false, "{name} sollte gelingen: {answer}");
    answer["result"]["structuredContent"].clone()
}

async fn tool_refused(server: &common::Server, session: &str, name: &str, arguments: Value) -> String {
    let answer = tool(server, session, name, arguments).await;
    assert_eq!(answer["result"]["isError"], true, "{name} sollte fachlich scheitern: {answer}");
    answer["result"]["content"][0]["text"].as_str().unwrap().to_owned()
}

#[tokio::test]
async fn mcp_answers_carry_a_key_url_for_every_section_hit_and_idea() {
    let (server, work, keys, _) = garden().await;
    let s = session(&server, "probe").await;
    let added = tool_ok(&server, &s, "add_post", json!({ "work": work, "key": keys[3], "body": "Gießkanne?" })).await;
    assert!(added["id"].as_i64().is_some_and(|id| id > 0), "add_post nennt die id des Beitrags: {added}");
    assert_eq!(added["url"], format!("/?work={work}&key={}", keys[3]), "add_post: url ohne rev: {added}");

    let read = tool_ok(&server, &s, "read_node", json!({ "work": work, "key": keys[1] })).await;
    assert_eq!(read["url"], format!("/?work={work}&key={}", keys[1]), "read_node: url: {read}");
    let outline = tool_ok(&server, &s, "outline", json!({ "work": work })).await;
    for node in outline["nodes"].as_array().unwrap() {
        assert_eq!(node["url"], format!("/?work={work}&key={}", node["key"]), "outline: url je Knoten: {node}");
    }
    let read = tool_ok(&server, &s, "read_node", json!({ "work": work, "key": keys[3] })).await;
    let posted = read["posts"].as_array().unwrap();
    assert!(
        posted.iter().any(|p| p["id"] == added["id"] && p["author"] == "@mcp:probe"),
        "read_node listet Beiträge mit id und Autor (eigene lassen sich so finden): {read}"
    );

    let found = tool_ok(&server, &s, "search", json!({ "query": "Radieschen", "work": work })).await;
    let hit = found["hits"].as_array().unwrap().iter().find(|h| h["kind"] == "revision").expect("Revisionstreffer");
    assert_eq!(hit["node_key"], keys[1], "Treffer tragen node_key: {hit}");
    assert_eq!(hit["url"], format!("/?work={work}&key={}&rev=1", keys[1]), "search: url mit rev: {hit}");
    let library = tool_ok(&server, &s, "search", json!({ "query": "langmütig" })).await;
    let quote = library["hits"].as_array().unwrap().iter().find(|h| h["kind"] == "quote").expect("Bibliothekstreffer");
    assert!(quote["url"].is_null(), "Treffer ohne Abschnitt: url null: {quote}");

    let with_key = tool_ok(&server, &s, "idea_create", json!({ "work": work, "key": keys[2], "words": "Tiefe" })).await;
    assert_eq!(with_key["idea"]["url"], format!("/?work={work}&key={}", keys[2]), "Einfall mit Bezug: {with_key}");
    let only_work = tool_ok(&server, &s, "add_idea", json!({ "work": work, "words": "Werk" })).await;
    assert_eq!(only_work["idea"]["url"], format!("/?work={work}"), "nur Werk: {only_work}");
    let none = tool_ok(&server, &s, "idea_create", json!({ "words": "frei" })).await;
    assert!(none["idea"]["url"].is_null(), "ohne Werk: url null: {none}");
    let id = with_key["idea"]["id"].clone();
    let read = tool_ok(&server, &s, "idea_read", json!({ "id": id })).await;
    assert_eq!(read["idea"]["url"], with_key["idea"]["url"], "idea_read: url");
    let list = tool_ok(&server, &s, "idea_list", json!({ "work": work })).await;
    assert!(list["ideas"].as_array().unwrap().iter().all(|i| i.get("url").is_some()), "idea_list: url je Einfall");
    let search = tool_ok(&server, &s, "idea_search", json!({ "query": "Tiefe" })).await;
    assert_eq!(search["ideas"][0]["url"], with_key["idea"]["url"], "idea_search: url: {search}");
    let updated = tool_ok(&server, &s, "idea_update", json!({ "id": id, "note": "tiefer säen" })).await;
    assert_eq!(updated["idea"]["url"], with_key["idea"]["url"], "idea_update: url: {updated}");
}

// ---- RPC post.* (§3d) ---------------------------------------------------------------------------

#[tokio::test]
async fn deleting_by_rpc_needs_confirm_and_records_the_whole_post_in_one_commit() {
    let (server, work, keys, dir) = garden().await;
    let id = add_post(&server, work, keys[1], "Möhren erst im April säen.\nZweite Zeile.").await;
    let before = commit_count(&dir);
    let refused = server.rpc("post.delete", json!({ "work": work, "id": id })).await;
    assert_eq!(refused["error"]["code"], -32602, "ohne confirm: ungültige Parameter: {refused}");
    assert_eq!(posts_at(&server, work, keys[1]).await.len(), 1, "nichts gelöscht");
    assert_eq!(commit_count(&dir), before, "kein Commit ohne Bestätigung");

    let done = server.rpc("post.delete", json!({ "work": work, "id": id, "confirm": true })).await;
    let done = result(&done);
    assert_eq!(
        (done["id"].as_i64(), done["node"].as_u64(), done["key"].as_i64()),
        (Some(id), Some(1), Some(keys[1])),
        "die Antwort nennt den gelöschten Beitrag, seine Stelle und den festen Schlüssel des Abschnitts: {done}"
    );
    assert_eq!(done["deleted"]["author"], "Autor", "der gelöschte Beitrag: {done}");
    assert_eq!(done["deleted"]["body"], "Möhren erst im April säen.\nZweite Zeile.", "mit ganzem Text: {done}");
    assert!(done["commit"].is_string(), "Antwort trägt den Commit: {done}");
    assert!(posts_at(&server, work, keys[1]).await.is_empty(), "der Beitrag ist fort");
    if let (Some(before), Some(after)) = (before, commit_count(&dir)) {
        assert_eq!(after, before + 1, "genau ein Commit");
    }
    if let Some(message) = last_message(&dir) {
        assert!(message.contains(&format!("Beitrag {id}: gelöscht")), "Betreff: {message}");
        assert!(message.contains(&format!("Baruch-Post: {id}")), "Trailer Baruch-Post: {message}");
        assert!(message.contains("Baruch-Action: post-delete"), "Trailer Baruch-Action: {message}");
        assert!(message.contains(&format!("Baruch-Node: {}", keys[1])), "Trailer Baruch-Node (Schlüssel): {message}");
        assert!(
            message.contains("Möhren erst im April säen.\nZweite Zeile."),
            "der ganze Beitrag steht im Commit (wiederherstellbar): {message}"
        );
        assert!(message.starts_with("Autor <autor@baruch.invalid>"), "Autor ist der Mensch: {message}");
    }
    let again = server.rpc("post.delete", json!({ "work": work, "id": id, "confirm": true })).await;
    assert_eq!(again["error"]["code"], -32602, "unbekannter Beitrag: {again}");
    assert!(again["error"]["message"].as_str().unwrap().contains(&format!("Beitrag {id} gibt es")), "{again}");
}

#[tokio::test]
async fn moving_by_rpc_appends_to_the_target_with_one_commit() {
    let (server, work, keys, dir) = garden().await;
    let id = add_post(&server, work, keys[0], "Gehört eigentlich zum Sommer.").await;
    let before = commit_count(&dir);
    let moved = server.rpc("post.move", json!({ "work": work, "id": id, "target_key": keys[3] })).await;
    let moved = result(&moved);
    assert_eq!(
        (moved["node"].as_u64(), moved["key"].as_i64(), moved["from_node"].as_u64(), moved["from_key"].as_i64()),
        (Some(3), Some(keys[3]), Some(0), Some(keys[0])),
        "Antwort nennt Ziel und Herkunft: {moved}"
    );
    assert_eq!(posts_at(&server, work, keys[3]).await.last().map(|p| p.0), Some(id), "am Ende des Sommers");
    assert!(posts_at(&server, work, keys[0]).await.is_empty(), "aus dem Frühling fort");
    if let (Some(before), Some(after)) = (before, commit_count(&dir)) {
        assert_eq!(after, before + 1, "genau ein Commit");
    }
    if let Some(message) = last_message(&dir) {
        assert!(message.contains(&format!("Beitrag {id}: verschoben nach 2 Sommer")), "Betreff: {message}");
        assert!(message.contains("Baruch-Action: post-move"), "{message}");
        assert!(message.contains(&format!("Baruch-Node: {}", keys[3])), "{message}");
    }
    let same = server.rpc("post.move", json!({ "work": work, "id": id, "target_key": keys[3] })).await;
    assert_eq!(same["error"]["code"], -32000, "gleiches Ziel fachlich abgelehnt: {same}");
    let no_target = server.rpc("post.move", json!({ "work": work, "id": id })).await;
    assert_eq!(no_target["error"]["code"], -32602, "ohne Ziel: {no_target}");
    let by_seq = server.rpc("post.move", json!({ "work": work, "id": id, "target": 1 })).await;
    assert_eq!(result(&by_seq)["key"], keys[1], "Rückfall über die Stelle target");
}

#[tokio::test]
async fn converting_by_rpc_is_one_commit_with_the_idea_in_einfaelle_md() {
    let (server, work, keys, dir) = garden().await;
    let id = add_post(&server, work, keys[2], "Tiefer säen, wenn es trocken ist.").await;
    let before = commit_count(&dir);
    let empty = server.rpc("post.to_idea", json!({ "work": work, "id": id, "words": " , " })).await;
    assert_eq!(empty["error"]["code"], -32602, "ohne Schlagwort: {empty}");
    assert_eq!(posts_at(&server, work, keys[2]).await.len(), 1, "der Beitrag bleibt, wenn der Einfall scheitert");
    assert_eq!(commit_count(&dir), before, "kein Commit");

    let done = server.rpc("post.to_idea", json!({ "work": work, "id": id, "words": "Trockenheit, Saattiefe" })).await;
    let done = result(&done);
    let idea = done["idea"]["id"].as_i64().unwrap_or_else(|| panic!("kein Einfall: {done}"));
    assert_eq!(done["idea"]["key"], keys[2], "Bezug = Abschnitt des Beitrags: {done}");
    assert_eq!(done["idea"]["note"], "Tiefer säen, wenn es trocken ist.", "Gedanke = Text: {done}");
    assert_eq!(done["idea"]["words"], json!(["Trockenheit", "Saattiefe"]), "Schlagwörter: {done}");
    assert_eq!(done["key"], keys[2], "{done}");
    assert!(posts_at(&server, work, keys[2]).await.is_empty(), "der Beitrag ist fort");
    if let (Some(before), Some(after)) = (before, commit_count(&dir)) {
        assert_eq!(after, before + 1, "genau EIN Commit für Einfall und Löschen");
    }
    if let Some(message) = last_message(&dir) {
        assert!(message.contains(&format!("Beitrag {id}: in Einfall {idea} umgewandelt")), "Betreff: {message}");
        assert!(message.contains("Baruch-Action: post-to-idea"), "{message}");
        assert!(message.contains(&format!("Baruch-Idea: {idea}")), "Trailer Baruch-Idea: {message}");
        assert!(message.contains("Tiefer säen, wenn es trocken ist."), "Beitrag im Rumpf: {message}");
        let files = common::git(&dir, &["show", "--name-only", "--format=", "HEAD"]).unwrap();
        assert!(files.lines().any(|f| f == "einfaelle.md"), "einfaelle.md im selben Commit: {files}");
        let md = common::git(&dir, &["show", "HEAD:einfaelle.md"]).unwrap();
        assert!(md.contains(&format!("## Einfall {idea}")), "der neue Einfall steht in einfaelle.md: {md}");
    }
}

#[tokio::test]
async fn post_add_answers_with_id_and_key() {
    let (server, work, keys, _) = garden().await;
    let added = server.rpc("post.add", json!({ "work": work, "node": 3, "body": "Hallo" })).await;
    let added = result(&added);
    assert_eq!(added["key"], keys[3], "post.add antwortet mit dem Schlüssel: {added}");
    let id = added["id"].as_i64().unwrap();
    let node = server.rpc("node.get", json!({ "work": work, "key": keys[3] })).await;
    let post = &result(&node)["posts"][0];
    assert_eq!(post["id"], id, "dieselbe id in node.get: {post}");
    assert!(post["created"].as_str().is_some_and(|c| c.ends_with('Z')), "Zeitpunkt in node.get: {post}");
}

// ---- MCP nur eigene Beiträge (§3e) --------------------------------------------------------------

#[tokio::test]
async fn mcp_manages_only_its_own_posts() {
    let (server, work, keys, dir) = garden().await;
    let s = session(&server, "probe").await;
    let own =
        tool_ok(&server, &s, "add_post", json!({ "work": work, "key": keys[1], "body": "Eigener Gedanke." })).await;
    let own = own["id"].as_i64().unwrap();
    let human = add_post(&server, work, keys[1], "Vom Menschen.").await;
    let lektor = tool_ok(
        &server,
        &s,
        "add_post",
        json!({ "work": work, "key": keys[1], "body": "Vom Lektor.", "persona": "@lektor" }),
    )
    .await["id"]
        .as_i64()
        .unwrap();
    let other = session(&server, "fremd").await;
    let before = (posts_at(&server, work, keys[1]).await, commit_count(&dir));

    // Fremde Beiträge: isError, nichts geändert, kein Commit.
    for (name, args) in [
        ("post_delete", json!({ "work": work, "id": human })),
        ("post_move", json!({ "work": work, "id": human, "key": keys[3] })),
        ("post_to_idea", json!({ "work": work, "id": human, "words": "Mensch" })),
        ("post_delete", json!({ "work": work, "id": lektor })),
    ] {
        let text = tool_refused(&server, &s, name, args).await;
        assert!(text.starts_with("Nur eigene Beiträge"), "{name}: verständliche Ablehnung: {text}");
    }
    let text = tool_refused(&server, &other, "post_delete", json!({ "work": work, "id": own })).await;
    assert!(text.contains("@mcp:probe"), "ein anderer Client darf den Beitrag von probe nicht löschen: {text}");
    // Eigentum nach Angabe des Clients (docs/MCP.md): Keine Angabe trifft den Menschen – weder eine
    // Persona noch ein Client, der sich „Autor“ nennt (er wird @mcp:Autor, nicht „Autor“).
    let impostor = session(&server, "Autor").await;
    for (who, args) in [
        (&s, json!({ "work": work, "id": human, "persona": "@lektor" })),
        (&impostor, json!({ "work": work, "id": human })),
    ] {
        let text = tool_refused(&server, who, "post_delete", args).await;
        assert!(text.starts_with("Nur eigene Beiträge"), "der Beitrag des Menschen bleibt geschützt: {text}");
    }
    assert_eq!((posts_at(&server, work, keys[1]).await, commit_count(&dir)), before, "nichts geändert, kein Commit");

    // node statt key wird abgelehnt (Argumentfehler).
    let (_, answer) = mcp(
        &server,
        Some(&s),
        json!({ "jsonrpc": "2.0", "id": 9, "method": "tools/call",
                "params": { "name": "post_move", "arguments": { "work": work, "id": own, "node": 3 } } }),
    )
    .await;
    assert_eq!(answer["error"]["code"], -32602, "post_move nimmt nur key: {answer}");

    // Eigene: erlaubt, mit url und je einem Commit; Persona über `persona`.
    let moved = tool_ok(&server, &s, "post_move", json!({ "work": work, "id": own, "key": keys[3] })).await;
    assert_eq!(moved["url"], format!("/?work={work}&key={}", keys[3]), "post_move: url des Ziels: {moved}");
    if let Some(message) = last_message(&dir) {
        assert!(message.starts_with("MCP-Client (@mcp:probe)"), "Autor ist der Client: {message}");
        assert!(message.contains("Persona: @mcp:probe"), "{message}");
    }
    let converted =
        tool_ok(&server, &s, "post_to_idea", json!({ "work": work, "id": own, "words": ["Gedanke"] })).await;
    assert_eq!(converted["idea"]["origin"]["kind"], "mcp", "Herkunft des Einfalls: der Client: {converted}");
    assert_eq!(converted["idea"]["url"], format!("/?work={work}&key={}", keys[3]), "Einfall mit url: {converted}");
    let deleted =
        tool_ok(&server, &s, "post_delete", json!({ "work": work, "id": lektor, "persona": "@lektor" })).await;
    assert_eq!(deleted["deleted"]["author"], "@lektor", "die Persona löscht ihren eigenen Beitrag: {deleted}");
    if let Some(message) = last_message(&dir) {
        assert!(message.contains("(@lektor)"), "Autor des Commits ist die Persona: {message}");
    }
    let left: Vec<i64> = posts_at(&server, work, keys[1]).await.into_iter().map(|p| p.0).collect();
    assert_eq!(left, vec![human], "nur der Beitrag des Menschen bleibt");
}

// ---- Formulare ohne JavaScript (§3d, §3f) -------------------------------------------------------

#[tokio::test]
async fn the_chat_shows_a_badge_and_forms_that_work_without_javascript() {
    let (server, work, keys, _) = garden().await;
    let k1 = keys[1];
    let first = add_post(&server, work, k1, "Erster.").await;
    let second = add_post(&server, work, k1, "Zweiter.").await;
    let (_, html) = server.get(&format!("/?work={work}&key={k1}")).await;
    let pane = &html[html.find("id=\"pane-assistants\"").unwrap()..];
    assert!(
        pane.contains("<span class=\"badge chat-count\" data-chat-count=\"2\""),
        "Abzeichen mit der Zahl im Akkordeon-Kopf, auch eingeklappt (im summary): {pane}"
    );
    let summary = &pane[pane.find("data-section=\"chat\"").unwrap()..];
    let summary = &summary[..summary.find("</summary>").unwrap()];
    assert!(summary.contains("chat-count"), "das Abzeichen steht im summary: {summary}");
    assert!(pane.contains(&format!("data-post-id=\"{first}\" data-post-created=\"20")), "data-post-*: {pane}");
    assert!(pane.contains("data-widget=\"chat\""), "Widget chat am Chat");
    for method in ["post.delete", "post.move", "post.to_idea"] {
        assert!(pane.contains(&format!("name=\"method\" value=\"{method}\"")), "Formular {method}: {pane}");
    }
    assert!(pane.contains("action=\"/beitraege/act\""), "Formulare gehen ohne JavaScript an /beitraege/act");
    assert!(pane.contains("name=\"confirm\" value=\"1\" required"), "Löschen mit Kästchen");
    assert!(pane.contains("<summary class=\"small\">Verschieben…</summary>"), "Verschieben… als details");
    assert!(pane.contains(&format!("<option value=\"{}\">", keys[3])), "Ziele als festen Schlüssel: {pane}");
    assert!(!pane.contains(&format!("<option value=\"{k1}\">")), "der gezeigte Abschnitt ist kein Ziel");
    assert!(pane.contains("name=\"words\" required"), "Umwandeln braucht Schlagwörter");

    // Löschen ohne Kästchen: 422, mit: 303.
    let (status, _) =
        server.post_form("/beitraege/act", &format!("method=post.delete&work={work}&key={k1}&id={first}")).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "ohne Kästchen abgelehnt");
    let (status, location) = server
        .post_form(
            "/beitraege/act",
            &format!("method=post.delete&work={work}&key={k1}&node=1&id={first}&rev=1&view=preview&lq=Saat&confirm=on"),
        )
        .await;
    assert_eq!(status, StatusCode::SEE_OTHER, "gelöscht");
    assert_eq!(location, Some(format!("/?work={work}&key={k1}&rev=1&view=preview&lq=Saat")), "zurück, alles bleibt");
    // Verschieben über select target_key.
    let (status, location) = server
        .post_form(
            "/beitraege/act",
            &format!("method=post.move&work={work}&key={k1}&id={second}&target_key={}", keys[3]),
        )
        .await;
    assert_eq!(status, StatusCode::SEE_OTHER, "verschoben");
    assert_eq!(location, Some(format!("/?work={work}&key={k1}")), "zurück zum gezeigten Abschnitt");
    assert_eq!(posts_at(&server, work, keys[3]).await.last().map(|p| p.0), Some(second), "im Sommer");
    // Umwandeln: Schlagwörter Pflicht.
    let (status, _) = server
        .post_form("/beitraege/act", &format!("method=post.to_idea&work={work}&key={}&id={second}&words=", keys[3]))
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "ohne Schlagwörter abgelehnt");
    let (status, location) = server
        .post_form(
            "/beitraege/act",
            &format!("method=post.to_idea&work={work}&key={}&id={second}&words=Sommer%2C+Hitze", keys[3]),
        )
        .await;
    assert_eq!(status, StatusCode::SEE_OTHER, "umgewandelt");
    assert_eq!(location, Some(format!("/?work={work}&key={}", keys[3])), "zurück");
    assert!(posts_at(&server, work, keys[3]).await.is_empty(), "Beitrag fort");
    let ideas = server.rpc("idea.list", json!({ "work": work })).await;
    assert!(
        result(&ideas)["ideas"].as_array().unwrap().iter().any(|i| i["note"] == "Zweiter."),
        "der Einfall ist da: {ideas}"
    );
    let (status, _) = server.post_form("/beitraege/act", "method=post.purge&id=1").await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "nur post.delete, post.move, post.to_idea");
}

#[tokio::test]
async fn the_post_form_from_a_foreign_origin_is_refused() {
    let (server, work, keys, _) = garden().await;
    let id = add_post(&server, work, keys[1], "Bleibt.").await;
    let request = Request::post("/beitraege/act")
        .header(header::HOST, "127.0.0.1:8765")
        .header(header::ORIGIN, "https://evil.example")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(format!("method=post.delete&work={work}&key={}&id={id}&confirm=1", keys[1])))
        .unwrap();
    assert_eq!(server.send(request).await.0, StatusCode::FORBIDDEN, "fremde Seite → 403");
    assert_eq!(posts_at(&server, work, keys[1]).await.len(), 1, "der Beitrag bleibt");
}

#[test]
fn the_chat_widget_remembers_the_last_visit_in_local_storage_safely() {
    let script = include_str!("../web/src/chat.ts");
    assert!(script.contains("const STORAGE = \"baruch_chat_seen\";"), "Schlüssel im LocalStorage laut Vertrag");
    assert!(script.contains("register(\"chat\""), "Widget chat");
    assert!(script.contains("post.classList.add(\"post-new\")"), "Klasse post-new für neue Beiträge");
    assert!(script.contains("` · ${fresh} neu`"), "„k neu“ am Abzeichen");
    let reads = script.matches("window.localStorage").count();
    let guarded = script.matches("try {").count();
    assert!(reads >= 2 && guarded >= 2, "jeder LocalStorage-Zugriff in try/catch: {reads} Zugriffe, {guarded} try");
    assert!(include_str!("../web/src/main.ts").contains("import \"./chat.js\";"), "main.ts lädt chat.ts");
    assert!(!include_str!("../web/src/panes.ts").contains("register(\"chat\""), "chat nur noch in chat.ts");
}
