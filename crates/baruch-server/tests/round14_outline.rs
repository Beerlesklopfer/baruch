// SPDX-License-Identifier: AGPL-3.0-or-later
//! Round 14 „Teile und Gliederung“ (R14b) in the server: creating parts through RPC `outline.add` with `level`, the
//! form without JavaScript and MCP `outline_add` (author in the mirror), renaming a part, requests with preview (RPC,
//! MCP, page `/gliederung#antraege` without JavaScript) and `idea.process` as an outline item in one transaction. All
//! test texts freely invented (`tests/data/gartenbuch.typ`).

use crate::common;

use std::net::SocketAddr;

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode, header};
use baruch_core::{Author, NodeKind};
use serde_json::{Value, json};

fn result(answer: &Value) -> &Value {
    assert!(answer["error"].is_null(), "RPC-Fehler: {answer}");
    &answer["result"]
}

async fn garden() -> (common::Server, i64, String) {
    let server = common::open().await;
    let report = server.import_garden().await;
    (server, report.work, report.slug)
}

async fn nodes(server: &common::Server, work: i64) -> Vec<Value> {
    let answer = server.rpc("work.get", json!({ "work": work })).await;
    result(&answer)["work"]["nodes"].as_array().unwrap().clone()
}

async fn key_of(server: &common::Server, work: i64, title: &str) -> i64 {
    let all = nodes(server, work).await;
    all.iter().find(|n| n["title"] == title).unwrap_or_else(|| panic!("kein Punkt {title}: {all:?}"))["key"]
        .as_i64()
        .unwrap()
}

async fn titles(server: &common::Server, work: i64) -> Vec<String> {
    nodes(server, work).await.iter().map(|n| n["title"].as_str().unwrap().to_owned()).collect()
}

fn log(server: &common::Server, slug: &str) -> Option<Vec<String>> {
    common::git(&server.mirror_dir_of(slug), &["log", "--format=%s"]).map(|l| l.lines().map(str::to_owned).collect())
}

fn last_commit(server: &common::Server, slug: &str) -> Option<String> {
    common::git(&server.mirror_dir_of(slug), &["log", "-1", "--format=%an <%ae>%n%cn <%ce>%n%B"])
}

/// Current revision (text, author) of the item with the key `key`, directly from the store.
async fn current_of(server: &common::Server, work: i64, key: i64) -> (String, Author, NodeKind) {
    let loaded = server.state.store.load_work(work).await.unwrap();
    let node = loaded.nodes.iter().find(|n| n.key == key).expect("Punkt mit diesem Schlüssel");
    let rev = &node.revisions[node.current_index().unwrap()];
    (rev.body.clone(), rev.author.clone(), node.kind)
}

// ---- MCP helpers (as in outline_mcp.rs) ---------------------------------------------------------

async fn mcp_post(server: &common::Server, session: Option<&str>, message: Value) -> (axum::http::HeaderMap, Value) {
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

async fn mcp_session(server: &common::Server) -> String {
    let (headers, answer) = mcp_post(
        server,
        None,
        json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize",
                "params": { "protocolVersion": "2025-06-18", "capabilities": {},
                            "clientInfo": { "name": "Teilprobe", "version": "1.0" } } }),
    )
    .await;
    assert!(answer["result"].is_object(), "initialize: {answer}");
    headers.get("mcp-session-id").unwrap().to_str().unwrap().to_owned()
}

async fn mcp_call(server: &common::Server, session: &str, tool: &str, arguments: Value) -> Value {
    let message = json!({ "jsonrpc": "2.0", "id": 3, "method": "tools/call",
                          "params": { "name": tool, "arguments": arguments } });
    mcp_post(server, Some(session), message).await.1
}

async fn mcp_ok(server: &common::Server, session: &str, tool: &str, arguments: Value) -> Value {
    let answer = mcp_call(server, session, tool, arguments).await;
    assert_eq!(answer["result"]["isError"], false, "{tool} sollte gelingen: {answer}");
    answer["result"]["structuredContent"].clone()
}

// ---- Creating parts -------------------------------------------------------------------------------

#[tokio::test]
async fn rpc_outline_add_with_level_zero_puts_a_first_part_before_the_first_chapter_of_a_template_work() {
    let (server, work, slug) = garden().await;
    let spring = key_of(&server, work, "Frühling").await;
    let summer = key_of(&server, work, "Sommer").await;
    let before = nodes(&server, work).await;
    let answer = server
        .rpc(
            "outline.add",
            json!({ "work": work, "target_key": spring, "place": "before", "title": "Erster Teil", "level": 0 }),
        )
        .await;
    let added = result(&answer);
    // The Gartenbuch has a work template: up to Round 13 Baruch rejected here, now no longer.
    assert_eq!(added["action"], "added", "angelegt: {added}");
    assert_eq!(added["node"], 0, "der Teil steht vor dem ersten Kapitel");
    assert_eq!(added["adopted"], json!([spring, summer]), "adopted = die Kapitel, die jetzt unter dem Teil stehen");
    let new_key = added["key"].as_i64().expect("Antwort trägt den Schlüssel des neuen Teils");
    let (body, author, kind) = current_of(&server, work, new_key).await;
    assert_eq!(kind, NodeKind::Part, "der neue Punkt ist ein Teil");
    assert_eq!(body, format!("{}\n", baruch_typst::heading_line(0, "Erster Teil", None)), "Text = Teil-Markierung");
    assert_eq!(author, Author::Human { name: "Autor".into() }, "Revision 1 vom Menschen");
    // Numbers of the chapters stay (chapters count across parts).
    let after = nodes(&server, work).await;
    for old in before {
        let now = after.iter().find(|n| n["key"] == old["key"]).unwrap();
        assert_eq!(now["number"], old["number"], "{}: Nummer unverändert", old["title"]);
    }
    if let Some(commit) = last_commit(&server, &slug) {
        assert!(commit.contains("Baruch-Action: outline-add"), "ein Gliederungs-Commit: {commit}");
        assert!(commit.starts_with("Autor <autor@baruch.invalid>"), "Autor des Commits ist der Mensch: {commit}");
    }

    // `into` with level 0 → -32000 (contract 5), nothing created.
    let count = after.len();
    let answer = server
        .rpc(
            "outline.add",
            json!({ "work": work, "target_key": summer, "place": "into", "title": "Falsch", "level": 0 }),
        )
        .await;
    assert_eq!(answer["error"]["code"], -32000, "into mit Ebene 0 abgelehnt: {answer}");
    let answer = server
        .rpc("outline.add", json!({ "work": work, "target_key": summer, "place": "after", "title": "X", "level": 9 }))
        .await;
    assert!(answer["error"].is_object(), "Ebene 9 abgelehnt: {answer}");
    assert_eq!(nodes(&server, work).await.len(), count, "nichts angelegt");
    // Without `level` as before: level of the place, `adopted` empty.
    let answer = server
        .rpc("outline.add", json!({ "work": work, "target_key": summer, "place": "after", "title": "Herbst" }))
        .await;
    assert_eq!(result(&answer)["adopted"], json!([]), "ein Kapitel übernimmt nichts");
}

#[tokio::test]
async fn the_form_without_javascript_creates_a_part_with_the_level_field() {
    let (server, work, _) = garden().await;
    let summer = key_of(&server, work, "Sommer").await;
    // The edit page of a chapter offers „Ebene: Teil“ in the form „Neuer Gliederungspunkt“.
    let (status, page) = server.get(&format!("/gliederung/bearbeiten?work={work}&key={summer}")).await;
    assert_eq!(status, StatusCode::OK, "Seite lädt");
    assert!(page.contains(r#"<select name="level">"#) && page.contains(r#"<option value="0">"#), "Feld level mit Teil");
    let form = format!(
        "method=outline.add&work={work}&target_key={summer}&place=after&title=Zweiter+Teil&level=0&view=gliederung"
    );
    let (status, location) = server.post_form("/gliederung/act", &form).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "ohne JavaScript: 303 zurück zur Gliederung");
    assert!(location.unwrap().starts_with(&format!("/gliederung?work={work}")), "zurück auf /gliederung");
    let all = nodes(&server, work).await;
    let part = all.iter().find(|n| n["title"] == "Zweiter Teil").expect("der Teil steht im Werk");
    assert_eq!(part["kind"], "part", "Art Teil");
    // Empty field level = level of the place (as before Round 14).
    let form = format!("method=outline.add&work={work}&target_key={summer}&place=after&title=Nachsommer&level=");
    let (status, _) = server.post_form("/gliederung/act", &form).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "leeres level wird angenommen");
    let all = nodes(&server, work).await;
    assert_eq!(all.iter().find(|n| n["title"] == "Nachsommer").unwrap()["kind"], "chapter", "ohne level ein Kapitel");
}

#[tokio::test]
async fn mcp_outline_add_creates_parts_as_the_assistant_with_adopted_chapters() {
    let (server, work, slug) = garden().await;
    let s = mcp_session(&server).await;
    let spring = key_of(&server, work, "Frühling").await;
    let summer = key_of(&server, work, "Sommer").await;
    // Single item with level 0 before the first chapter: would take over both chapters → request (user question 12).
    let answer = mcp_ok(
        &server,
        &s,
        "outline_add",
        json!({ "work": work, "key": spring, "place": "before", "title": "Teil A", "level": 0 }),
    )
    .await;
    assert_eq!(answer["action"], "requested", "Teil mit Folgekapiteln → Antrag: {answer}");
    assert_eq!(answer["adopted"], json!([spring, summer]), "adopted in der MCP-Antwort");
    let id = answer["request"]["id"].as_i64().unwrap();
    result(&server.rpc("outline.request_decide", json!({ "work": work, "id": id, "decision": "confirm" })).await);
    let part_key = key_of(&server, work, "Teil A").await;
    let (body, author, _) = current_of(&server, work, part_key).await;
    assert!(body.starts_with(&baruch_typst::part_line("Teil A")), "Teil-Markierung als Text");
    assert!(
        matches!(&author, Author::Assistant { persona, .. } if persona == "@mcp:teilprobe"),
        "Revision 1 trägt den Client, auch nach der Zustimmung des Menschen: {author:?}"
    );
    if let Some(commit) = last_commit(&server, &slug) {
        assert!(commit.contains("Baruch-Origin: mcp"), "Herkunft MCP im Commit: {commit}");
        assert!(commit.contains("Baruch-Requested-By: @mcp:teilprobe"), "Antragsteller im Commit: {commit}");
        assert!(commit.starts_with("Autor <"), "Autor des Commits ist der zustimmende Mensch: {commit}");
    }
    // items with a second part before „Sommer“ (level of the place 1, part allowed as the first item).
    let items = json!([{ "title": "Teil B", "level": 0 }]);
    let answer =
        mcp_ok(&server, &s, "outline_add", json!({ "work": work, "key": summer, "place": "before", "items": items }))
            .await;
    assert_eq!(answer["action"], "requested", "auch als items ein Antrag: {answer}");
    assert_eq!(answer["adopted"], json!([summer]), "nur „Sommer“ würde zum neuen Teil wechseln");
    // A part at the end of the work takes over nothing: created at once, author of the commit is the client.
    let answer = mcp_ok(
        &server,
        &s,
        "outline_add",
        json!({ "work": work, "key": summer, "place": "after", "title": "Teil C", "level": 0 }),
    )
    .await;
    assert_eq!((answer["action"].as_str(), answer["adopted"].clone()), (Some("added"), json!([])), "{answer}");
    if let Some(commit) = last_commit(&server, &slug) {
        assert!(commit.starts_with("MCP-Client (@mcp:teilprobe)"), "Autor des Commits ist der Client: {commit}");
    }
    // level next to items is contradictory.
    let answer =
        mcp_call(&server, &s, "outline_add", json!({ "work": work, "key": summer, "level": 0, "items": items })).await;
    assert_eq!(answer["error"]["code"], -32602, "level neben items abgelehnt: {answer}");
    // Changing the level to a part stays rejected as a request too.
    let answer = mcp_call(&server, &s, "outline_set_level", json!({ "work": work, "key": summer, "level": 0 })).await;
    assert_eq!(answer["result"]["isError"], true, "outline_set_level auf 0 abgelehnt: {answer}");
    // The descriptions name level 0, adopted and preview (contract 5).
    let (_, list) = mcp_post(&server, Some(&s), json!({ "jsonrpc": "2.0", "id": 9, "method": "tools/list" })).await;
    let tools = list["result"]["tools"].as_array().unwrap();
    let describe =
        |name: &str| tools.iter().find(|t| t["name"] == name).unwrap()["description"].as_str().unwrap().to_owned();
    assert!(describe("outline_add").contains("level 0") && describe("outline_add").contains("adopted"), "outline_add");
    assert!(describe("outline_requests").contains("preview"), "outline_requests nennt preview");
}

#[tokio::test]
async fn renaming_a_part_over_rpc_rewrites_its_marker_line() {
    let (server, work, _) = garden().await;
    let spring = key_of(&server, work, "Frühling").await;
    let answer = server
        .rpc(
            "outline.add",
            json!({ "work": work, "target_key": spring, "place": "before", "title": "Alt", "level": 0 }),
        )
        .await;
    let part = result(&answer)["key"].as_i64().unwrap();
    let answer = server.rpc("outline.rename", json!({ "work": work, "key": part, "title": "Neu" })).await;
    let renamed = result(&answer);
    assert_eq!(renamed["revisions"].as_array().unwrap().len(), 1, "genau eine neue Fassung");
    let (body, author, _) = current_of(&server, work, part).await;
    assert_eq!(body, format!("{}\n", baruch_typst::part_line("Neu")), "nur die Markierungszeile ist neu");
    assert_eq!(author, Author::Human { name: "Autor".into() }, "vom Menschen");
}

// ---- Requests with preview --------------------------------------------------------------------------

#[tokio::test]
async fn requests_show_a_preview_over_rpc_mcp_and_on_the_page_without_javascript() {
    let (server, work, slug) = garden().await;
    let s = mcp_session(&server).await;
    let summer = key_of(&server, work, "Sommer").await;
    let request = mcp_ok(
        &server,
        &s,
        "outline_rename",
        json!({ "work": work, "key": summer, "title": "Hochsommer", "note": "klarer" }),
    )
    .await;
    let id = request["request"]["id"].as_i64().unwrap();
    let shown = titles(&server, work).await;
    let commits = log(&server, &slug);

    // RPC without preview: no field, as before.
    let plain = server.rpc("outline.requests", json!({ "work": work })).await;
    assert!(result(&plain)["requests"][0].get("preview").is_none(), "ohne preview keine Vorschau");
    // RPC with preview: outline afterwards, the renamed item marked.
    let answer = server.rpc("outline.requests", json!({ "work": work, "preview": true })).await;
    let preview = result(&answer)["requests"][0]["preview"].as_array().expect("preview je offenem Antrag").clone();
    let point = preview.iter().find(|p| p["key"] == summer).unwrap();
    assert_eq!(point["title"], "Hochsommer", "Titel nach dem Bestätigen");
    assert_eq!(point["mark"], "renamed", "Marke umbenannt");
    assert_eq!(preview.len(), shown.len(), "alle Punkte der Gliederung");
    // MCP with preview: the same.
    let listed = mcp_ok(&server, &s, "outline_requests", json!({ "work": work, "preview": true })).await;
    assert_eq!(listed["requests"][0]["preview"], json!(preview), "MCP liefert dieselbe Vorschau wie RPC");
    // Page without JavaScript: <details> with the preview at the request.
    let (status, page) = server.get(&format!("/gliederung?work={work}")).await;
    assert_eq!(status, StatusCode::OK, "Gliederungsseite lädt");
    let row = page.split(&format!(r#"id="antrag-{id}""#)).nth(1).expect("Zeile des Antrags");
    let row = row.split("</tr>").next().unwrap();
    assert!(row.contains("<details class=\"request-preview\">"), "Vorschau als <details> (ohne JavaScript)");
    assert!(row.contains("Hochsommer") && row.contains("umbenannt"), "neuer Titel und Marke sichtbar: {row}");
    // Nothing changed: outline and mirror as before.
    assert_eq!(titles(&server, work).await, shown, "die Vorschau ändert die Gliederung nicht");
    assert_eq!(log(&server, &slug), commits, "kein Commit durch die Vorschau");

    // Confirming yields exactly the preview (titles, numbers, order).
    server.rpc("outline.request_decide", json!({ "work": work, "id": id, "decision": "confirm" })).await;
    let after = nodes(&server, work).await;
    let rows: Vec<(Value, Value, Value)> =
        after.iter().map(|n| (n["key"].clone(), n["number"].clone(), n["title"].clone())).collect();
    let expected: Vec<(Value, Value, Value)> =
        preview.iter().map(|p| (p["key"].clone(), p["number"].clone(), p["title"].clone())).collect();
    assert_eq!(rows, expected, "Vorschau = Gliederung nach dem Bestätigen");
}

#[tokio::test]
async fn stale_and_decided_requests_carry_a_preview_error() {
    let (server, work, _) = garden().await;
    let s = mcp_session(&server).await;
    let summer = key_of(&server, work, "Sommer").await;
    let spring = key_of(&server, work, "Frühling").await;
    mcp_ok(&server, &s, "outline_rename", json!({ "work": work, "key": summer, "title": "Hochsommer" })).await;
    let rejected = mcp_ok(&server, &s, "outline_rename", json!({ "work": work, "key": spring, "title": "Lenz" })).await;
    let rejected = rejected["request"]["id"].as_i64().unwrap();
    server.rpc("outline.request_decide", json!({ "work": work, "id": rejected, "decision": "reject" })).await;
    // The human renames „Sommer“ himself: the open request is outdated.
    server.rpc("outline.rename", json!({ "work": work, "key": summer, "title": "Spätsommer" })).await;
    let answer = server.rpc("outline.requests", json!({ "work": work, "open": false, "preview": true })).await;
    let requests = result(&answer)["requests"].as_array().unwrap().clone();
    let stale = requests.iter().find(|r| r["decision"].is_null()).expect("der offene Antrag");
    assert!(stale.get("preview").is_none(), "veraltet: keine Vorschau");
    let text = stale["preview_error"].as_str().expect("preview_error");
    assert!(text.contains("Veraltet") && text.contains("Spätsommer"), "Grund genannt: {text}");
    let decided = requests.iter().find(|r| r["id"] == rejected).unwrap();
    assert!(decided["preview_error"].as_str().unwrap().contains("schon entschieden"), "entschieden: {decided}");
    // The page shows the reason instead of the preview.
    let (_, page) = server.get(&format!("/gliederung?work={work}")).await;
    assert!(page.contains("Keine Vorschau: Veraltet"), "Seite nennt den Grund ohne JavaScript");
}

// ---- idea.process as an outline item --------------------------------------------------------------

#[tokio::test]
async fn idea_process_as_outline_is_one_transaction_without_a_commit_on_failure() {
    let (server, work, slug) = garden().await;
    let summer = key_of(&server, work, "Sommer").await;
    let created = server
        .rpc("idea.create", json!({ "work": work, "key": summer, "words": ["Laub"], "note": "Ein Herbstkapitel." }))
        .await;
    let id = result(&created)["idea"]["id"].as_i64().unwrap();
    let shown = titles(&server, work).await;
    let commits = log(&server, &slug);
    // The second step fails by force (trigger on UPDATE idea): nothing may stay.
    sqlx::query("CREATE TRIGGER test_idea_fails BEFORE UPDATE ON idea BEGIN SELECT RAISE(ABORT, 'Probe'); END")
        .execute(server.state.store.pool())
        .await
        .unwrap();
    let failed =
        server.rpc("idea.process", json!({ "id": id, "as": "outline", "key": summer, "title": "Herbst" })).await;
    assert!(failed["error"].is_object(), "der Trigger lässt idea.process scheitern: {failed}");
    sqlx::query("DROP TRIGGER test_idea_fails").execute(server.state.store.pool()).await.unwrap();
    assert_eq!(titles(&server, work).await, shown, "kein neuer Gliederungspunkt");
    assert_eq!(log(&server, &slug), commits, "kein Commit im Spiegel");
    let idea = server.rpc("idea.get", json!({ "id": id })).await;
    assert_eq!(result(&idea)["idea"]["status"], "open", "der Einfall bleibt offen");

    // Without the trigger: item and „verarbeitet“ together, done_ref names the key.
    let done = server.rpc("idea.process", json!({ "id": id, "as": "outline", "key": summer, "title": "Herbst" })).await;
    let done = result(&done);
    let key = done["key"].as_i64().expect("Schlüssel des neuen Punkts");
    assert_eq!(done["idea"]["status"], "done", "verarbeitet");
    assert_eq!(done["idea"]["done_ref"], format!("key {key}"), "done_ref = key des Punkts");
    assert_eq!(key_of(&server, work, "Herbst").await, key, "der Punkt steht im Werk");
    if let Some(after) = log(&server, &slug) {
        assert!(after.len() > commits.unwrap().len(), "jetzt gibt es Commits");
    }
}
