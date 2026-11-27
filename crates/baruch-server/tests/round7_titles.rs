// SPDX-License-Identifier: AGPL-3.0-or-later
//! Round 7, „bugs“: a paragraph became the title of an outline item (backlog „Absatz wird Überschrift“). Reproduced
//! over RPC the way the user interface does it: new chapter and a section below it (`outline.add`), write a text with
//! several paragraphs in the typeset view and save it (`wysiwyg.parse` → `wysiwyg.serialize` with the instructions of
//! the widget → `outline.check` → `outline.save`), then change the level (`outline.set_level` and the format selection
//! in the typeset view). Afterwards every outline item is named like its heading line, and no paragraph is a title.
//!
//! Cause: the widget sends a heading that contains a simple line break (Shift+Enter or a `<br>` of the browser after
//! the title, then the typed or pasted paragraph) as **one** heading block. `baruch_wysiwyg::emit` turned it into a
//! single heading line (`Inline::Soft` in headings → space), and `outline.save` took the whole line as the title. Now
//! the heading ends at the first line break, the rest becomes a paragraph of its own; and titles above 200 characters
//! (a whole pasted paragraph) are rejected by the store.
//!
//! Test texts freely invented (garden motifs).

use crate::common;

use serde_json::{Value, json};

/// Outline as (title, level, key, first line of the current revision) in tree order.
async fn outline(server: &common::Server, work: i64) -> Vec<(String, u8, i64, String)> {
    let loaded = server.rpc("work.get", json!({ "work": work })).await;
    let nodes = loaded["result"]["work"]["nodes"].as_array().expect("work.get liefert Knoten").clone();
    let mut levels: Vec<u8> = Vec::new();
    let mut out = Vec::new();
    for node in &nodes {
        let parent = node["parent"].as_u64().map(|p| levels[p as usize]);
        let level = match node["kind"].as_str().unwrap() {
            "part" => 0,
            "chapter" => 1,
            _ if parent.is_some_and(|p| p >= 2) => 3,
            _ => 2,
        };
        levels.push(level);
        let current = node["revisions"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["status"] == "current")
            .expect("jeder Abschnitt hat eine aktuelle Fassung");
        let first = current["body"].as_str().unwrap().lines().next().unwrap_or("").to_owned();
        out.push((node["title"].as_str().unwrap().to_owned(), level, node["key"].as_i64().unwrap(), first));
    }
    out
}

/// Every item (except parts) is named like its heading line, and the level of that line is its level.
fn assert_titles_match_headings(points: &[(String, u8, i64, String)], when: &str) {
    for (title, level, _, first) in points {
        if *level == 0 {
            continue;
        }
        let expected = format!("{} {title}", "=".repeat(usize::from(*level)));
        assert_eq!(first, &expected, "{when}: Überschriftszeile und Gliederung laufen auseinander: {points:?}");
    }
}

fn key_of(points: &[(String, u8, i64, String)], title: &str) -> i64 {
    points.iter().find(|p| p.0 == title).unwrap_or_else(|| panic!("„{title}“ fehlt: {points:?}")).2
}

fn ok(answer: &Value, what: &str) -> Value {
    assert!(answer.get("error").is_none(), "{what} scheitert: {answer}");
    answer["result"].clone()
}

/// Current text of an item through its key.
async fn body_of(server: &common::Server, work: i64, key: i64) -> String {
    let node = server.rpc("node.get", json!({ "work": work, "key": key })).await;
    let node = ok(&node, "node.get");
    let current = node["revisions"].as_array().unwrap().iter().find(|r| r["status"] == "current").unwrap().clone();
    current["body"].as_str().unwrap().to_owned()
}

/// Save like the typeset view: `wysiwyg.serialize` with the instructions of the widget, then `outline.check` and
/// `outline.save` (with confirmation if the check demands splitting).
async fn save_from_sheet(server: &common::Server, work: i64, key: i64, edits: Value) -> Value {
    let body = body_of(server, work, key).await;
    let parsed = ok(&server.rpc("wysiwyg.parse", json!({ "work": work, "body": body })).await, "wysiwyg.parse");
    assert!(parsed["blocks"].is_array(), "wysiwyg.parse liefert Blöcke: {parsed}");
    let serialized = ok(
        &server.rpc("wysiwyg.serialize", json!({ "work": work, "body": body, "edits": edits })).await,
        "wysiwyg.serialize",
    );
    let text = serialized["body"].as_str().unwrap().to_owned();
    let check = ok(&server.rpc("outline.check", json!({ "work": work, "key": key, "body": text })).await, "check");
    let split = !check["split"].as_array().unwrap().is_empty();
    server.rpc("outline.save", json!({ "work": work, "key": key, "body": text, "split": split })).await
}

const PARAGRAPH: &str = "Anna's Laube steht im Herbst voller Kürbisse, und der Wind trägt das Laub über den Weg.";

#[tokio::test]
async fn a_paragraph_typed_behind_the_heading_never_becomes_the_title() {
    let server = common::open().await;
    let work = server.import_garden().await.work;
    let points = outline(&server, work).await;
    let summer = key_of(&points, "Sommer");

    // New chapter after „Sommer“, in it a section.
    let added = server
        .rpc("outline.add", json!({ "work": work, "target_key": summer, "place": "after", "title": "Herbst" }))
        .await;
    ok(&added, "neues Kapitel");
    let autumn = key_of(&outline(&server, work).await, "Herbst");
    let added = server
        .rpc("outline.add", json!({ "work": work, "target_key": autumn, "place": "into", "title": "Laub" }))
        .await;
    ok(&added, "neuer Abschnitt darunter");
    let points = outline(&server, work).await;
    let leaves = key_of(&points, "Laub");
    assert_titles_match_headings(&points, "nach dem Anlegen");
    assert_eq!(body_of(&server, work, leaves).await, "== Laub\n\n", "Fassung 1 = Überschrift und Leerzeile");

    // In the typeset view: after the title Shift+Enter (or a <br> of the browser), then the paragraph, Enter, a second
    // paragraph. This is how the widget sent it before the fix: the paragraph in the heading.
    let edits = json!([
        { "replaces": 0, "block": { "kind": "heading", "level": 2, "inlines": [
            { "t": "text", "v": "Laub" }, { "t": "soft" }, { "t": "text", "v": PARAGRAPH } ] } },
        { "replaces": null, "block": { "kind": "paragraph", "inlines": [ { "t": "text", "v": "Dann wird geharkt." } ] } }
    ]);
    let saved = save_from_sheet(&server, work, leaves, edits).await;
    ok(&saved, "Speichern aus dem Satzbild");
    let points = outline(&server, work).await;
    assert_titles_match_headings(&points, "nach dem Speichern im Satzbild");
    assert!(points.iter().any(|p| p.0 == "Laub"), "der Abschnitt heißt weiter „Laub“: {points:?}");
    assert!(
        points.iter().all(|p| !p.0.contains("Kürbisse")),
        "kein Absatz ist Titel geworden (Ursache des Fehlers): {points:?}"
    );
    let body = body_of(&server, work, leaves).await;
    assert_eq!(
        body,
        format!("== Laub\n\n{PARAGRAPH}\n\nDann wird geharkt.\n\n"),
        "der Absatz steht unter der Überschrift, nicht in ihr"
    );

    // Change the level through the outline: Laub becomes a chapter and a section again.
    ok(&server.rpc("outline.set_level", json!({ "work": work, "key": leaves, "level": 1 })).await, "Ebene 2 → 1");
    let points = outline(&server, work).await;
    assert_titles_match_headings(&points, "nach outline.set_level");
    assert_eq!(
        body_of(&server, work, leaves).await,
        format!("= Laub\n\n{PARAGRAPH}\n\nDann wird geharkt.\n\n"),
        "Ebene 1: nur das Gleichheitszeichen ändert sich, der Absatz bleibt Absatz"
    );

    // Change the level through the format selection in the typeset view („Überschrift 2“ with the cursor in the
    // heading); the remaining blocks are sent by the widget as keep.
    let edits = json!([
        { "replaces": 0, "block": { "kind": "heading", "level": 2, "inlines": [ { "t": "text", "v": "Laub" } ] } },
        { "keep": 1 }, { "keep": 2 }
    ]);
    ok(&save_from_sheet(&server, work, leaves, edits).await, "Format-Auswahl Überschrift 2");
    let points = outline(&server, work).await;
    assert_titles_match_headings(&points, "nach der Format-Auswahl");
    let titles: Vec<&str> = points.iter().map(|p| p.0.as_str()).collect();
    assert_eq!(
        titles,
        ["Frühling", "Aussaat", "Tiefe", "Sommer", "Herbst", "Laub"],
        "alle Titel unverändert, kein neuer Punkt aus einem Absatz"
    );
    assert_eq!(points[5].1, 2, "Laub ist wieder Abschnitt unter Herbst: {points:?}");
}

/// Follow-up fix: the break is inside an emphasis of the heading (drop or a `<br>` of the browser in `<b>`). Before, it
/// became a space in the heading line, and the paragraph was the title again (below 200 characters the title rule did
/// not take hold either).
#[tokio::test]
async fn a_paragraph_after_a_break_inside_bold_never_becomes_the_title() {
    let server = common::open().await;
    let work = server.import_garden().await.work;
    let summer = key_of(&outline(&server, work).await, "Sommer");
    let added = server
        .rpc("outline.add", json!({ "work": work, "target_key": summer, "place": "after", "title": "Laub" }))
        .await;
    ok(&added, "neues Kapitel");
    let leaves = key_of(&outline(&server, work).await, "Laub");
    let edits = json!([
        { "replaces": 0, "block": { "kind": "heading", "level": 1, "inlines": [
            { "t": "strong", "c": [ { "t": "text", "v": "Laub" }, { "t": "soft" }, { "t": "text", "v": PARAGRAPH } ] } ] } }
    ]);
    ok(&save_from_sheet(&server, work, leaves, edits).await, "Speichern aus dem Satzbild");
    let points = outline(&server, work).await;
    assert_titles_match_headings(&points, "nach dem Speichern mit Umbruch in *…*");
    // Cause of the bug: title „Laub Anna's Laube …“. Now the title stays „Laub“.
    assert!(points.iter().all(|p| !p.0.contains("Kürbisse")), "kein Absatz ist Titel geworden: {points:?}");
    // The title is the heading line including markup, as with every emphasised heading.
    assert!(points.iter().any(|p| p.0 == "*Laub*"), "das Kapitel heißt weiter „*Laub*“: {points:?}");
    // The paragraph is kept and emphasised, only now below the heading.
    assert_eq!(body_of(&server, work, leaves).await, format!("= *Laub*\n\n*{PARAGRAPH}*\n\n"));
}

#[tokio::test]
async fn a_whole_paragraph_pasted_into_the_heading_is_refused_as_title() {
    let server = common::open().await;
    let work = server.import_garden().await.work;
    let points = outline(&server, work).await;
    let summer = key_of(&points, "Sommer");
    let before = body_of(&server, work, summer).await;
    // A whole pasted paragraph in the heading (without a line break): more than 200 characters.
    let long = format!("{PARAGRAPH} {PARAGRAPH} {PARAGRAPH}");
    assert!(long.chars().count() > 200, "Testtext länger als die Titelgrenze");
    let edits = json!([
        { "replaces": 0, "block": { "kind": "heading", "level": 1, "inlines": [ { "t": "text", "v": long } ] } },
        { "keep": 1 }
    ]);
    let saved = save_from_sheet(&server, work, summer, edits).await;
    assert_eq!(saved["error"]["code"], -32602, "zu langer Titel → INVALID_PARAMS: {saved}");
    let message = saved["error"]["message"].as_str().unwrap();
    assert!(message.starts_with("Der Titel ist zu lang (höchstens 200 Zeichen, hier "), "deutsche Meldung: {message}");
    assert_eq!(body_of(&server, work, summer).await, before, "nichts gespeichert");
    assert_titles_match_headings(&outline(&server, work).await, "nach der Ablehnung");
}

const EMPTY: &str = "Ein Gliederungspunkt braucht einen Titel.";
const NEWLINE: &str = "Der Titel darf keinen Zeilenumbruch enthalten.";

/// Rejected with -32602 and exactly this message (or its beginning).
fn refused_with(answer: &Value, message: &str, what: &str) {
    assert_eq!(answer["error"]["code"], -32602, "{what}: Titelregel → INVALID_PARAMS: {answer}");
    let text = answer["error"]["message"].as_str().unwrap_or_default();
    assert!(text.starts_with(message), "{what}: Meldung „{message}“ erwartet: {answer}");
}

/// MCP call from the own machine without a session (author `@mcp:unbekannt`).
async fn mcp_call(server: &common::Server, tool: &str, arguments: Value) -> Value {
    use axum::body::Body;
    use axum::extract::ConnectInfo;
    use axum::http::{Request, header};
    let message = json!({ "jsonrpc": "2.0", "id": 7, "method": "tools/call",
        "params": { "name": tool, "arguments": arguments } });
    let mut request = Request::post("/mcp")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(message.to_string()))
        .unwrap();
    request.extensions_mut().insert(ConnectInfo(std::net::SocketAddr::from(([127, 0, 0, 1], 40000))));
    let (status, _, body) = server.send(request).await;
    assert_eq!(status, axum::http::StatusCode::OK, "tools/call {tool}: {body}");
    serde_json::from_str(&body).unwrap()
}

#[tokio::test]
async fn bad_titles_are_refused_on_every_way_in_and_change_nothing() {
    let server = common::open().await;
    let work = server.import_garden().await.work;
    let points = outline(&server, work).await;
    let seed = key_of(&points, "Aussaat");
    let export = |server: &common::Server| {
        let server = server.app.clone();
        async move { common::rpc(server, "work.export", json!({ "work": work })).await["result"]["typst"].clone() }
    };
    let before = export(&server).await;
    let long = "Kürbis".repeat(34); // 204 characters

    // Tree: rename and create over RPC.
    for (title, message) in [("   ", EMPTY), ("Saat\nund Ernte", NEWLINE), (long.as_str(), "Der Titel ist zu lang")] {
        let renamed = server.rpc("outline.rename", json!({ "work": work, "key": seed, "title": title })).await;
        refused_with(&renamed, message, "outline.rename");
        let added = server
            .rpc("outline.add", json!({ "work": work, "target_key": seed, "place": "after", "title": title }))
            .await;
        refused_with(&added, message, "outline.add");
    }
    // Tree without JavaScript: form to /gliederung/act → 422.
    let form = format!("method=outline.rename&work={work}&key={seed}&node=1&title=+%09+");
    let (status, _) = server.post_form("/gliederung/act", &form).await;
    assert_eq!(status, axum::http::StatusCode::UNPROCESSABLE_ENTITY, "leerer Titel im Formular → 422");
    let form = format!("method=outline.add&work={work}&target_key={seed}&target=1&place=after&title=Saat%0AErnte");
    let (status, _) = server.post_form("/gliederung/act", &form).await;
    assert_eq!(status, axum::http::StatusCode::UNPROCESSABLE_ENTITY, "Zeilenumbruch im Formular → 422");

    // Editor and revisions: save, revision.save, change a revision, check.
    let empty_heading = "== \n\nRadieschen & Möhren.\n";
    refused_with(
        &server.rpc("outline.save", json!({ "work": work, "key": seed, "body": empty_heading })).await,
        EMPTY,
        "outline.save",
    );
    let check =
        ok(&server.rpc("outline.check", json!({ "work": work, "key": seed, "body": empty_heading })).await, "c");
    assert_eq!((check["valid"].as_bool(), check["message"].as_str()), (Some(false), Some(EMPTY)), "check: {check}");
    refused_with(
        &server.rpc("revision.save", json!({ "work": work, "key": seed, "body": empty_heading })).await,
        EMPTY,
        "revision.save",
    );
    refused_with(
        &server.rpc("revision.update", json!({ "work": work, "key": seed, "rev": 1, "body": empty_heading })).await,
        EMPTY,
        "revision.update der aktuellen Fassung",
    );

    // MCP: propose_revision goes through the store (title check in the proposal) → isError.
    let answer =
        mcp_call(&server, "propose_revision", json!({ "work": work, "key": seed, "body": empty_heading })).await;
    assert_eq!(answer["result"]["isError"], true, "MCP lehnt den Vorschlag ab: {answer}");
    assert_eq!(answer["result"]["content"][0]["text"], EMPTY, "mit der deutschen Meldung: {answer}");
    let with_long = format!("== {long}\n\nText.\n");
    let answer = mcp_call(&server, "propose_revision", json!({ "work": work, "key": seed, "body": with_long })).await;
    assert_eq!(answer["result"]["isError"], true, "auch ein zu langer Titel: {answer}");
    assert_eq!(export(&server).await, before, "keine Ablehnung hat etwas geändert");

    // Switching: changing revision 1 (old) is allowed, switching to it is not.
    let saved = server.rpc("revision.save", json!({ "work": work, "key": seed, "body": "== Aussaat\n\nNeu.\n" })).await;
    ok(&saved, "gültige neue Fassung");
    let edited =
        server.rpc("revision.update", json!({ "work": work, "key": seed, "rev": 1, "body": empty_heading })).await;
    ok(&edited, "eine alte Fassung ändern prüft keine Überschrift");
    let after_edit = export(&server).await;
    refused_with(
        &server.rpc("revision.set_current", json!({ "work": work, "key": seed, "rev": 1 })).await,
        EMPTY,
        "revision.set_current",
    );
    assert_eq!(export(&server).await, after_edit, "Umschalten abgelehnt, nichts geändert");
    assert_ne!(before, after_edit, "die gültige Fassung 2 ist gespeichert");
    assert_titles_match_headings(&outline(&server, work).await, "nach allen Ablehnungen");
    let titles: Vec<String> = outline(&server, work).await.into_iter().map(|p| p.0).collect();
    assert_eq!(titles, ["Frühling", "Aussaat", "Tiefe", "Sommer"], "keine Ablehnung hat einen Titel geändert");
}
