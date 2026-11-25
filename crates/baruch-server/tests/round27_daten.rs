// SPDX-License-Identifier: AGPL-3.0-or-later
//! Runde 27, Aufgabe umschlag-daten (R27a): Umschlag-Anordnung über RPC und MCP, Anträge, Anschriften und Austausch-Unterversion 3.
//!
//! Werk frei erfunden (`tests/data/gartenbuch.typ`); jede Zusicherung mit Begründung. Geprüft wird über die echten
//! Wege: RPC `cover.layout`, `cover.layout_set`, `cover.layout_reset`, `cover.fonts`, MCP `cover_layout`,
//! `cover_layout_set` mit Bestätigen und Ablehnen über `asset.request_decide`, Spiegel-Commits im Git-Ordner und der
//! Austausch zwischen zwei Instanzen über `work.exchange_export`/`work.exchange_import` (XSD der Unterversion 3).

use std::net::SocketAddr;

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode, header};
use serde_json::{Value, json};

use crate::common;

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

async fn initialize(server: &common::Server) -> String {
    let (headers, answer) = post(
        server,
        None,
        json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize",
                "params": { "protocolVersion": "2025-06-18", "capabilities": {},
                            "clientInfo": { "name": "Umschlagprobe", "version": "1.0" } } }),
    )
    .await;
    assert!(answer["result"].is_object(), "initialize: {answer}");
    headers.get("mcp-session-id").unwrap().to_str().unwrap().to_owned()
}

async fn call(server: &common::Server, session: &str, tool: &str, arguments: Value) -> Value {
    let message = json!({ "jsonrpc": "2.0", "id": 3, "method": "tools/call",
                          "params": { "name": tool, "arguments": arguments } });
    post(server, Some(session), message).await.1
}

async fn ok_tool(server: &common::Server, session: &str, tool: &str, arguments: Value) -> Value {
    let answer = call(server, session, tool, arguments).await;
    assert_eq!(answer["result"]["isError"], false, "{tool} sollte gelingen: {answer}");
    answer["result"]["structuredContent"].clone()
}

fn result(answer: &Value) -> &Value {
    assert!(answer["error"].is_null(), "RPC-Fehler: {answer}");
    &answer["result"]
}

fn commits(server: &common::Server, slug: &str) -> usize {
    common::git(&server.mirror_dir_of(slug), &["rev-list", "--count", "HEAD"]).map_or(0, |n| n.trim().parse().unwrap())
}

fn last_commit(server: &common::Server, slug: &str) -> String {
    common::git(&server.mirror_dir_of(slug), &["log", "-1", "--format=%an <%ae>%n%B"]).unwrap_or_default()
}

async fn garden() -> (common::Server, i64, String) {
    let server = common::open().await;
    let report = server.import_garden().await;
    (server, report.work, report.slug)
}

async fn layout(server: &common::Server, work: i64) -> Value {
    result(&server.rpc("cover.layout", json!({ "work": work })).await).clone()
}

/// Wirksamer Block der Art `kind` aus `cover.layout` (samt `stored` und `height_mm`).
fn block_of(view: &Value, kind: &str) -> Value {
    view["blocks"].as_array().unwrap().iter().find(|b| b["kind"] == kind).cloned().unwrap_or(Value::Null)
}

async fn cover_rows(server: &common::Server, work: i64) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM cover_block WHERE work_id = ?")
        .bind(work)
        .fetch_one(server.state.store.pool())
        .await
        .unwrap()
}

#[tokio::test]
async fn without_rows_cover_layout_shows_all_default_blocks_with_geometry() {
    let (server, work, _) = garden().await;
    let view = layout(&server, work).await;
    assert_eq!(view["work"], work, "Werk der Ansicht");
    assert_eq!(view["stored"], false, "ohne Zeile keine eigene Anordnung (Vertrag 1.4)");
    let blocks = view["blocks"].as_array().unwrap();
    assert_eq!(blocks.len(), 15, "alle 15 Arten (Vertrag 1.1)");
    assert!(blocks.iter().all(|b| b["stored"] == false), "alles Vorgabeblöcke");
    // R27b: ein Block ohne Inhalt (etwa der Untertitel eines Werks ohne Untertitel) hat die gemessene Höhe 0.
    assert!(blocks.iter().all(|b| b["height_mm"].as_f64().is_some_and(|h| h >= 0.0)), "jede Höhe gemessen: {view}");
    assert!(
        block_of(&view, "front_title")["height_mm"].as_f64().is_some_and(|h| h > 0.0),
        "der Titel hat Inhalt, also eine gemessene Höhe über 0: {view}"
    );
    for key in ["page_width_mm", "page_height_mm", "spine_mm", "bleed_mm", "safety_mm"] {
        assert!(view["geometry"][key].as_f64().is_some(), "geometry.{key} (JSON eingefroren, Vertrag 4): {view}");
    }
    assert!(view["default_font"].as_str().is_some_and(|f| !f.is_empty()), "Vorgabeschrift genannt");
    assert!(view["warnings"].is_array(), "Druckprüfung als Liste");
    assert_eq!(block_of(&view, "spine_title")["direction"], "down", "Rücken mit Laufrichtung");
    assert!(block_of(&view, "front_title")["direction"].is_null(), "Vorderseite ohne Laufrichtung");
}

#[tokio::test]
async fn layout_set_replaces_named_kinds_keeps_the_others_and_commits_once_with_before_and_after() {
    let (server, work, slug) = garden().await;
    let view = layout(&server, work).await;
    let mut title = block_of(&view, "front_title");
    title["x_mm"] = json!(17.4);
    title["y_mm"] = json!(41.6);
    title["style"] = json!("bold");
    let mut spine = block_of(&view, "spine_author");
    spine["direction"] = json!("up");
    let before = commits(&server, &slug);
    let answer = server.rpc("cover.layout_set", json!({ "work": work, "blocks": [title, spine] })).await;
    let set = result(&answer);
    assert!(set["commit"].is_string(), "ein Commit: {set}");
    assert_eq!(commits(&server, &slug), before + 1, "genau ein Commit für beide Blöcke");
    let message = last_commit(&server, &slug);
    assert!(message.contains("Umschlag: Anordnung geändert"), "Betreff (Vertrag 1.10): {message}");
    assert!(message.contains("Titel (Vorderseite): x "), "Rumpf je Block mit Art und Feld: {message}");
    assert!(message.contains("→ 17,4 mm"), "vorher → nachher mit Einheit: {message}");
    assert!(message.contains("(vorher Vorgabe)"), "erstmals gespeichert: verglichen mit dem Vorgabeblock: {message}");
    assert!(
        message.contains("Autor (Rücken): ") && message.contains("Laufrichtung abwärts → aufwärts"),
        "Rücken: {message}"
    );
    assert!(message.contains("Baruch-Action: cover-layout"), "Trailer: {message}");
    assert!(message.starts_with("Autor <autor@baruch.invalid>"), "der Mensch ist Autor: {message}");
    let shown = &set["layout"];
    assert_eq!(shown["stored"], true, "jetzt eigene Anordnung");
    assert_eq!(block_of(shown, "front_title")["stored"], true, "Titel steht in der Datenbank");
    assert_eq!(block_of(shown, "front_title")["x_mm"], 17.4, "neuer Wert");
    assert_eq!(block_of(shown, "spine_author")["direction"], "up", "Laufrichtung aufwärts");
    assert_eq!(block_of(shown, "front_author")["stored"], false, "ungenannte Arten bleiben Vorgabe");

    // Zweites Setzen nur des Titels: der Rücken-Block bleibt gespeichert, wie er war.
    let mut again = block_of(shown, "front_title");
    again["width_mm"] = json!(90.0);
    let answer = server.rpc("cover.layout_set", json!({ "work": work, "blocks": [again] })).await;
    let after = &result(&answer)["layout"];
    assert_eq!(block_of(after, "spine_author")["stored"], true, "ungenannter Block bleibt (Vertrag 4)");
    assert_eq!(block_of(after, "spine_author")["direction"], "up", "mit seinen Werten");
    assert_eq!(commits(&server, &slug), before + 2, "wieder genau ein Commit");
    // Unverändert noch einmal: kein Commit.
    let same = block_of(after, "front_title");
    let answer = server.rpc("cover.layout_set", json!({ "work": work, "blocks": [same] })).await;
    assert!(result(&answer).get("commit").is_none(), "ohne Änderung kein Commit: {answer}");
    assert_eq!(commits(&server, &slug), before + 2, "Zahl der Commits bleibt");
}

#[tokio::test]
async fn invalid_blocks_give_invalid_params_naming_kind_and_field_and_change_nothing() {
    let (server, work, slug) = garden().await;
    let view = layout(&server, work).await;
    let base = block_of(&view, "front_title");
    let with = |field: &str, value: Value| {
        let mut b = base.clone();
        b[field] = value;
        b
    };
    let mut no_x = base.clone();
    no_x.as_object_mut().unwrap().remove("x_mm");
    let mut blurb = block_of(&view, "back_blurb");
    blurb["direction"] = json!("down");
    let cases: Vec<(Value, &str, &str)> = vec![
        (with("x_mm", json!(2000.0)), "front_title", "x_mm"),
        (with("y_mm", json!(-51)), "front_title", "y_mm"),
        (with("width_mm", json!(0)), "front_title", "width_mm"),
        (with("size_pt", json!(300)), "front_title", "size_pt"),
        (with("leading_em", json!(4)), "front_title", "leading_em"),
        (with("x_mm", json!("zwölf")), "front_title", "x_mm"),
        (no_x, "front_title", "x_mm"),
        (with("kind", json!("front_cloud")), "front_cloud", "kind"),
        (blurb, "back_blurb", "direction"),
        (with("text", json!("Ein eigener Titel")), "front_title", "text"),
        (with("color", json!("rot")), "front_title", "color"),
        (with("align", json!("schief")), "front_title", "align"),
        (with("font", json!("Serif; }")), "front_title", "font"),
        (with("font", json!("A{B")), "front_title", "font"),
        (with("font", json!("<script>")), "front_title", "font"),
        (with("font", json!("A\\B")), "front_title", "font"),
        (with("font", json!("A\u{7}B")), "front_title", "font"),
        (with("font", json!("x".repeat(201))), "front_title", "font"),
        (with("wolke", json!(1)), "front_title", "wolke"),
    ];
    let before = commits(&server, &slug);
    for (bad, kind, field) in cases {
        let answer = server.rpc("cover.layout_set", json!({ "work": work, "blocks": [bad] })).await;
        assert_eq!(answer["error"]["code"], -32602, "{kind}/{field}: ungültige Eingabe (Vertrag 4): {answer}");
        let message = answer["error"]["message"].as_str().unwrap();
        assert!(message.contains(kind), "Meldung nennt die Art {kind}: {message}");
        assert!(message.contains(field), "Meldung nennt das Feld {field}: {message}");
        assert_eq!(cover_rows(&server, work).await, 0, "{kind}/{field}: nichts gespeichert");
    }
    assert_eq!(commits(&server, &slug), before, "kein Commit");
    let answer = server.rpc("cover.layout_set", json!({ "work": work, "blocks": "front_title" })).await;
    assert_eq!(answer["error"]["code"], -32602, "blocks muss eine Liste sein: {answer}");
}

#[tokio::test]
async fn a_quick_choice_resets_only_the_front_with_the_area_and_a_full_reset_leaves_tombstones() {
    let (server, work, slug) = garden().await;
    let view = layout(&server, work).await;
    let blocks: Vec<Value> =
        ["front_title", "front_author", "spine_title"].iter().map(|k| block_of(&view, k)).collect();
    result(&server.rpc("cover.layout_set", json!({ "work": work, "blocks": blocks })).await);
    assert_eq!(cover_rows(&server, work).await, 3, "Vorbedingung: drei Zeilen");
    let before = commits(&server, &slug);

    let answer = server.rpc("cover.layout_reset", json!({ "work": work, "faces": ["front"], "area": "bottom" })).await;
    let reset = result(&answer);
    assert!(reset["commit"].is_string(), "ein Commit: {reset}");
    assert_eq!(commits(&server, &slug), before + 1, "Bereich und Löschen in einem Commit (Vertrag 1.5)");
    let message = last_commit(&server, &slug);
    assert!(message.contains("Umschlag: Schnellwahl unten"), "Betreff der Schnellwahl: {message}");
    assert!(message.contains("Titelbereich: oben → unten"), "Bereich im Rumpf: {message}");
    assert!(message.contains("Baruch-Action: cover-layout-reset"), "Trailer: {message}");
    assert_eq!(reset["layout"]["title_area"], "bottom", "title_area gesetzt");
    assert_eq!(block_of(&reset["layout"], "front_title")["stored"], false, "Vorderseite wieder Vorgabe");
    assert_eq!(block_of(&reset["layout"], "spine_title")["stored"], true, "Rücken bleibt");
    let meta = result(&server.rpc("work.meta.get", json!({ "work": work })).await).clone();
    assert_eq!(meta["meta"]["title_area"], "bottom", "Buchangaben kennen den neuen Bereich");

    let answer = server.rpc("cover.layout_reset", json!({ "work": work })).await;
    let all = result(&answer);
    assert_eq!(all["layout"]["stored"], false, "ohne faces: alle Zeilen weg (Vertrag 4)");
    assert!(last_commit(&server, &slug).contains("Umschlag: Anordnung zurückgesetzt"), "Betreff des Zurücksetzens");
    assert_eq!(commits(&server, &slug), before + 2, "ein Commit");
    let stones: i64 = sqlx::query_scalar("SELECT count(*) FROM uuid_tombstone WHERE kind = 'cover_block'")
        .fetch_one(server.state.store.pool())
        .await
        .unwrap();
    assert_eq!(stones, 3, "je gelöschter Zeile ein Grabstein 'cover_block'");
    // Nichts mehr zurückzusetzen: kein Commit.
    result(&server.rpc("cover.layout_reset", json!({ "work": work })).await);
    assert_eq!(commits(&server, &slug), before + 2, "ohne Änderung kein Commit");
    let wrong = server.rpc("cover.layout_reset", json!({ "work": work, "faces": ["inside"] })).await;
    assert_eq!(wrong["error"]["code"], -32602, "unbekannte Fläche: {wrong}");
    let wrong = server.rpc("cover.layout_reset", json!({ "work": work, "area": "links" })).await;
    assert_eq!(wrong["error"]["code"], -32602, "unbekannter Bereich: {wrong}");
}

#[tokio::test]
async fn cover_fonts_lists_families_and_the_default() {
    let (server, work, _) = garden().await;
    let fonts = result(&server.rpc("cover.fonts", json!({ "work": work })).await).clone();
    let families = fonts["families"].as_array().unwrap();
    assert!(!families.is_empty(), "Schriften des Satzes: {fonts}");
    assert!(
        families.iter().all(|f| f["name"].is_string() && f["styles"].is_array()),
        "Form {{name, styles, embedded}}"
    );
    assert!(families.iter().any(|f| f["name"] == "Libertinus Serif"), "eingebettete Vorgabeschrift dabei");
    assert!(fonts["default"].as_str().is_some_and(|d| !d.is_empty()), "Vorgabe genannt");
}

#[tokio::test]
async fn addresses_are_kept_multiline_and_appear_in_work_meta_get() {
    let (server, work, _) = garden().await;
    let meta = result(&server.rpc("work.meta.get", json!({ "work": work })).await).clone();
    assert_eq!(meta["meta"]["publisher_address"], "", "anfangs leer (Migration 0045)");
    let edit = baruch_store::BookEdit {
        publisher_address: Some("Gartenverlag\nAm Beet 4\n12345 Blumenstadt".into()),
        printer_address: Some("Druckerei Probe\nPresse 1".into()),
        ..Default::default()
    };
    server.state.store.set_book_meta(work, &edit).await.unwrap();
    let meta = result(&server.rpc("work.meta.get", json!({ "work": work })).await).clone();
    assert_eq!(
        meta["meta"]["publisher_address"], "Gartenverlag\nAm Beet 4\n12345 Blumenstadt",
        "mehrzeilig gespeichert"
    );
    assert_eq!(meta["meta"]["printer_address"], "Druckerei Probe\nPresse 1", "Anschrift der Druckerei");
    let long = baruch_store::BookEdit { publisher_address: Some("x".repeat(501)), ..Default::default() };
    let refused = server.state.store.set_book_meta(work, &long).await.unwrap_err().to_string();
    assert!(refused.contains("höchstens 500 Zeichen"), "deutsche Meldung mit Grenze: {refused}");
}

#[tokio::test]
async fn mcp_reads_without_change_and_a_confirmed_request_is_executed_with_requester_and_confirmer() {
    let (server, work, slug) = garden().await;
    let session = initialize(&server).await;
    let before = commits(&server, &slug);
    let read = ok_tool(&server, &session, "cover_layout", json!({ "work": work })).await;
    assert_eq!(read["blocks"].as_array().map(Vec::len), Some(15), "cover_layout wie cover.layout: {read}");
    assert_eq!(read["stored"], false, "ohne eigene Anordnung");
    assert!(read["geometry"].is_object(), "mit Seitenmaß");

    let filed = ok_tool(
        &server,
        &session,
        "cover_layout_set",
        json!({ "work": work, "blocks": [{ "kind": "front_title", "y_mm": 55.5 }], "note": "Titel tiefer setzen" }),
    )
    .await;
    assert_eq!(filed["status"], "open", "Antrag offen: {filed}");
    let request = &filed["request"];
    assert_eq!(request["op"], "cover_layout", "Bildantrag mit op cover_layout");
    assert_eq!(request["blocks"][0]["y_mm"], 55.5, "Antrag zeigt die Blöcke");
    assert_eq!(
        request["blocks"][0]["x_mm"],
        block_of(&read, "front_title")["x_mm"],
        "fehlende Felder aus dem wirksamen Block"
    );
    assert_eq!(request["expected_layout"].as_str().map(str::len), Some(64), "Fingerabdruck SHA-256");
    assert_eq!(cover_rows(&server, work).await, 0, "der Antrag ändert die Anordnung nicht");
    assert_eq!(commits(&server, &slug), before, "kein Commit für Lesen und Antrag");
    let twin = call(
        &server,
        &session,
        "cover_layout_set",
        json!({ "work": work, "blocks": [{ "kind": "front_title", "y_mm": 55.5 }] }),
    )
    .await;
    assert_eq!(twin["result"]["isError"], true, "gleicher offener Antrag nicht doppelt: {twin}");
    let listed = result(&server.rpc("asset.request_list", json!({ "work": work })).await).clone();
    let requests = listed["requests"].as_array().unwrap();
    assert_eq!(requests.len(), 1, "genau ein offener Antrag");
    assert!(
        requests[0]["blocks"].is_array() && requests[0]["expected_layout"].is_string(),
        "Liste zeigt blocks und expected_layout"
    );
    // Die Seite „Bilder“ zeigt den Antrag ohne JavaScript mit Bestätigen/Ablehnen und je Block eine Zeile.
    let (status, page) = server.get(&format!("/werke/{work}/bilder")).await;
    assert_eq!(status, StatusCode::OK, "Seite Bilder");
    assert!(page.contains("Umschlag-Anordnung ändern (1 Block)"), "Zusammenfassung in der Liste");
    assert!(page.contains("Titel (Vorderseite): x "), "Zeile des Blocks: {page}");
    assert!(page.contains("Umschlag ansehen"), "Verweis auf die Umschlagseite");

    let id = request["id"].as_i64().unwrap();
    let decided =
        result(&server.rpc("asset.request_decide", json!({ "work": work, "id": id, "decision": "confirm" })).await)
            .clone();
    assert_eq!(decided["request"]["decision"], "confirmed", "bestätigt: {decided}");
    assert!(decided["commit"].is_string(), "ein Commit: {decided}");
    let view = layout(&server, work).await;
    assert_eq!(block_of(&view, "front_title")["y_mm"], 55.5, "Antrag ausgeführt");
    assert_eq!(block_of(&view, "front_title")["stored"], true, "jetzt gespeichert");
    let message = last_commit(&server, &slug);
    assert!(message.contains(&format!("Umschlag: Anordnung geändert (Antrag {id})")), "Betreff: {message}");
    assert!(message.contains("bestätigt von Autor"), "Bestätiger im Rumpf: {message}");
    assert!(message.contains("Begründung: Titel tiefer setzen"), "Begründung: {message}");
    assert!(message.contains("Baruch-Requested-By: @mcp:"), "Antragsteller als Trailer: {message}");
    assert!(message.contains("Baruch-Confirmed-By: Autor"), "Bestätiger als Trailer: {message}");
    assert!(message.contains("Titel (Vorderseite): y "), "Rumpf mit vorher → nachher: {message}");
}

#[tokio::test]
async fn a_request_goes_stale_after_a_human_change_and_rejecting_changes_nothing() {
    let (server, work, slug) = garden().await;
    let session = initialize(&server).await;
    let filed = ok_tool(
        &server,
        &session,
        "cover_layout_set",
        json!({ "work": work, "blocks": [{ "kind": "back_blurb", "width_mm": 80 }] }),
    )
    .await;
    let id = filed["request"]["id"].as_i64().unwrap();
    // Der Mensch ändert inzwischen direkt.
    let view = layout(&server, work).await;
    let mut author = block_of(&view, "front_author");
    author["y_mm"] = json!(150.0);
    result(&server.rpc("cover.layout_set", json!({ "work": work, "blocks": [author] })).await);
    let before = commits(&server, &slug);
    let decided =
        result(&server.rpc("asset.request_decide", json!({ "work": work, "id": id, "decision": "confirm" })).await)
            .clone();
    assert_eq!(decided["request"]["decision"], "stale", "veraltet (Vertrag 1.9): {decided}");
    assert!(decided["request"]["message"].as_str().unwrap().contains("inzwischen geändert"), "mit Grund: {decided}");
    assert_eq!(block_of(&layout(&server, work).await, "back_blurb")["stored"], false, "Antrag nicht ausgeführt");
    assert_eq!(commits(&server, &slug), before, "kein Commit für einen veralteten Antrag");

    let filed = ok_tool(
        &server,
        &session,
        "cover_layout_set",
        json!({ "work": work, "blocks": [{ "kind": "back_blurb", "width_mm": 70 }] }),
    )
    .await;
    let id = filed["request"]["id"].as_i64().unwrap();
    let rows = cover_rows(&server, work).await;
    let decided =
        result(&server.rpc("asset.request_decide", json!({ "work": work, "id": id, "decision": "reject" })).await)
            .clone();
    assert_eq!(decided["request"]["decision"], "rejected", "abgelehnt");
    assert_eq!(cover_rows(&server, work).await, rows, "Ablehnen ändert nichts");
    assert_eq!(commits(&server, &slug), before, "kein Commit");
}

#[tokio::test]
async fn no_mcp_tool_changes_the_layout_directly() {
    let (server, work, _) = garden().await;
    let session = initialize(&server).await;
    let (_, list) = post(&server, Some(&session), json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" })).await;
    let tools = list["result"]["tools"].as_array().unwrap();
    let cover: Vec<&str> = tools.iter().filter_map(|t| t["name"].as_str()).filter(|n| n.starts_with("cover")).collect();
    assert_eq!(cover, ["cover_layout", "cover_layout_set"], "nur Lesen und Antrag (Vertrag 1.9)");
    let set = tools.iter().find(|t| t["name"] == "cover_layout_set").unwrap();
    assert!(set["description"].as_str().unwrap().starts_with("Antrag"), "Beschreibung nennt den Antrag zuerst");
    assert!(
        tools
            .iter()
            .filter(|t| t["name"].as_str().is_some_and(|n| n.starts_with("cover")))
            .all(|t| !t["description"].as_str().unwrap().contains("STUB")),
        "kein Stub mehr (R27a umgesetzt)"
    );
    // Auch ein vollständiger Block über MCP ändert nichts sofort.
    let read = ok_tool(&server, &session, "cover_layout", json!({ "work": work })).await;
    let mut full = block_of(&read, "front_title");
    full["x_mm"] = json!(9.0);
    ok_tool(&server, &session, "cover_layout_set", json!({ "work": work, "blocks": [full] })).await;
    assert_eq!(cover_rows(&server, work).await, 0, "nur ein Antrag, keine Zeile");
}

#[tokio::test]
async fn without_a_page_size_rpc_refuses_but_mcp_reads_the_stored_blocks_and_saving_still_works() {
    // Ein Buch mit Satzfehler hat kein Seitenmaß: cover.layout lehnt ab wie build_wrap (Vertrag 4), MCP liest mit
    // geometry: null und geometry_error, und Speichern geht trotzdem (Antwort layout: null, layout_error).
    let server = common::open().await;
    let report = server.import("= Kaputt\n#gibt-es-nicht-im-garten()\n", "kaputt.typ").await;
    let work = report.work;
    let refused = server.rpc("cover.layout", json!({ "work": work })).await;
    assert_eq!(refused["error"]["code"], -32000, "Satzfehler → REFUSED: {refused}");
    let block = json!({ "kind": "front_title", "x_mm": 10, "y_mm": 20, "width_mm": 100, "align": "center",
                        "size_pt": 20, "leading_em": 0.65, "color": "dark", "style": "normal", "backdrop": "none",
                        "visible": true });
    let saved = result(&server.rpc("cover.layout_set", json!({ "work": work, "blocks": [block] })).await).clone();
    assert!(
        saved["layout"].is_null() && saved["layout_error"].is_string(),
        "gespeichert, Ansicht ehrlich ohne: {saved}"
    );
    assert!(saved["commit"].is_string(), "Commit trotzdem: {saved}");
    assert!(
        last_commit(&server, &report.slug).contains("Titel (Vorderseite): neu (x 10,0 mm"),
        "ohne Vorgabe alle Werte"
    );
    let session = initialize(&server).await;
    let read = ok_tool(&server, &session, "cover_layout", json!({ "work": work })).await;
    assert!(read["geometry"].is_null(), "geometry: null (Vertrag 4): {read}");
    assert!(read["geometry_error"].as_str().is_some_and(|e| e.contains("Fehler")), "geometry_error mit Grund: {read}");
    assert_eq!(read["stored"], true, "die gespeicherte Anordnung");
    assert_eq!(read["blocks"][0]["color"], "dark", "gespeicherter Block gelesen");
}

async fn export_xml(server: &common::Server, work: i64) -> String {
    let answer = server.rpc("work.exchange_export", json!({ "work": work, "format": "xml" })).await;
    result(&answer)["xml"].as_str().unwrap().to_owned()
}

async fn import(server: &common::Server, xml: &str, mode: &str) -> Value {
    let answer = server
        .rpc(
            "work.exchange_import",
            json!({ "data": baruch_store::encode_base64(xml.as_bytes()), "sha256": baruch_exchange::sha256_hex(xml.as_bytes()), "mode": mode }),
        )
        .await;
    result(&answer).clone()
}

fn xmllint_u3(xml: &str) {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("werk.baruch.xml");
    std::fs::write(&file, xml).unwrap();
    let schema = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/schema/baruch-werk-1-u8.xsd");
    match std::process::Command::new("xmllint")
        .args(["--noout", "--nonet", "--schema"])
        .arg(&schema)
        .arg(&file)
        .output()
    {
        Ok(output) => assert!(
            output.status.success(),
            "XSD u8 (Runde 30: `schluessel` an den Punkten, darum nicht mehr u3): {}",
            String::from_utf8_lossy(&output.stderr)
        ),
        Err(e) => eprintln!("ÜBERSPRUNGEN: xmllint nicht gefunden ({e})"),
    }
}

#[tokio::test]
async fn two_instances_carry_layout_addresses_and_sheet_and_merge_after_f6() {
    let (a, work_a, _) = garden().await;
    let view = layout(&a, work_a).await;
    let mut title = block_of(&view, "front_title");
    title["font"] = json!("Libertinus Serif");
    let mut spine = block_of(&view, "spine_author");
    spine["direction"] = json!("up");
    let mut free = block_of(&view, "back_free");
    free["text"] = json!("Ein erfundener Satz & <Klammern>\nzweite Zeile");
    free["visible"] = json!(true);
    result(&a.rpc("cover.layout_set", json!({ "work": work_a, "blocks": [title, spine, free] })).await);
    let edit = baruch_store::BookEdit {
        publisher_address: Some("Gartenverlag\nAm Beet 4".into()),
        printer_address: Some("Druckerei Probe\nPresse 1".into()),
        ..Default::default()
    };
    a.state.store.set_book_meta(work_a, &edit).await.unwrap();
    result(&a.rpc("work.sheet_set", json!({ "work": work_a, "size": "12pt", "background": "#fbf8f1" })).await);

    let xml = export_xml(&a, work_a).await;
    // Runde 34 (mindmap_daten): Der Schreiber schreibt jetzt Unterversion 9 (Baum der Einfälle).
    assert!(xml.contains("version=\"9\""), "Ausgabe in Unterversion 9 (seit Runde 34)");
    assert!(
        xml.contains("<umschlag>") && xml.contains("art=\"ruecken-autor\"") && xml.contains("<satzbild "),
        "neue Teile"
    );
    xmllint_u3(&xml);

    // B liest neu ein: Anordnung samt Kennungen, Anschriften und Satzbild kommen mit.
    let b = common::open().await;
    let work_b = import(&b, &xml, "new").await["work"].as_i64().unwrap();
    assert_eq!(
        b.state.store.cover_layout(work_b).await.unwrap(),
        a.state.store.cover_layout(work_a).await.unwrap(),
        "dieselbe Anordnung auf B"
    );
    let uuids = |server: &common::Server, work: i64| {
        let pool = server.state.store.pool().clone();
        async move {
            sqlx::query_scalar::<_, String>("SELECT uuid FROM cover_block WHERE work_id = ? ORDER BY uuid")
                .bind(work)
                .fetch_all(&pool)
                .await
                .unwrap()
        }
    };
    assert_eq!(uuids(&b, work_b).await, uuids(&a, work_a).await, "Blöcke behalten ihre Kennungen");
    let meta_b = b.state.store.book_meta(work_b).await.unwrap();
    assert_eq!(
        (meta_b.publisher_address.as_str(), meta_b.printer_address.as_str()),
        ("Gartenverlag\nAm Beet 4", "Druckerei Probe\nPresse 1"),
        "Anschriften"
    );
    assert_eq!(
        b.state.store.sheet_settings(work_b).await.unwrap(),
        a.state.store.sheet_settings(work_a).await.unwrap(),
        "Satzbild"
    );
    assert_eq!(
        export_xml(&b, work_b).await.split("<angaben>").nth(1),
        xml.split("<angaben>").nth(1),
        "Rückweg byte-gleich ab <angaben>"
    );

    // F6: A setzt den Rücken zurück (Grabstein); B setzt den Titel zurück (Grabstein) und verschiebt den Freitext.
    result(&a.rpc("cover.layout_reset", json!({ "work": work_a, "faces": ["spine"] })).await);
    result(&b.rpc("cover.layout_reset", json!({ "work": work_b, "faces": ["front"] })).await);
    let mut moved = block_of(&layout(&b, work_b).await, "back_free");
    moved["x_mm"] = json!(20.0);
    result(&b.rpc("cover.layout_set", json!({ "work": work_b, "blocks": [moved] })).await);
    let before_b = b.state.store.cover_layout(work_b).await.unwrap();
    let merged = import(&b, &export_xml(&a, work_a).await, "merge").await;
    assert_eq!(merged["added"]["cover_blocks"], 0, "nichts kommt hinzu: {merged}");
    let deleted: Vec<&str> =
        merged["deleted_here"].as_array().unwrap().iter().filter_map(|d| d["kind"].as_str()).collect();
    assert!(deleted.contains(&"umschlag-block"), "der hier zurückgesetzte Titel kommt nicht wieder: {merged}");
    assert_eq!(b.state.store.cover_layout(work_b).await.unwrap(), before_b, "B bleibt, wie es ist (nie überschreiben)");
    let deviations = merged["deviations"].as_array().unwrap();
    assert!(
        deviations.iter().any(|d| d["kind"] == "umschlag-block" && d["label"].as_str().unwrap().contains("weicht ab")),
        "verschobener Freitext: nur Hinweis: {merged}"
    );
    assert!(
        deviations.iter().any(|d| d["kind"] == "umschlag-block" && d["label"].as_str().unwrap().contains("gelöscht")),
        "der auf A gelöschte Rücken lebt auf B weiter (Hinweis): {merged}"
    );
}
