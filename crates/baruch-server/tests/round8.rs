// SPDX-License-Identifier: AGPL-3.0-or-later
//! End to end Round 8 (integrator): the three tasks together in one real data directory (SQLite, Git mirror, library) –
//! key addresses from MCP answers after rearranging the outline, proposals of two personas at their place in the
//! reading flow (fold, accept), managing posts (convert = one commit, move, delete, MCP only own ones), an unchanged
//! empty title can be saved, Typst preview with image and Typst error with section. The „Teichbuch“ is freely invented.

use crate::common;

use std::net::SocketAddr;

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode, header};
use serde_json::{Value, json};

/// Invented work: 0 „Der Teich“ (three paragraphs), 1 „Die Brücke“ (with image), 2 without title.
const TEICH: &str = "\
#set document(title: \"Das Teichbuch\", author: \"Test\")

= Der Teich

Seerosen treiben im Wasser.

Ein Frosch sitzt am Rand.

Libellen schwirren umher.

= Die Brücke

Holz, grau vom Regen.

#image(\"bilder/steg.svg\", width: 3cm)

== \n\nOhne Titel aus einem alten Import.
";

const STEG_SVG: &str = "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"30\" height=\"10\" viewBox=\"0 0 30 10\">\
<rect width=\"30\" height=\"10\" fill=\"#a63\"/></svg>";

const LOOPBACK: ([u8; 4], u16) = ([127, 0, 0, 1], 40000);

struct Book {
    server: common::Server,
    work: i64,
    /// Stable keys by position at import.
    keys: Vec<i64>,
    dir: std::path::PathBuf,
}

/// Import the pond book (own final line `tag`, so that the static Typst cache sees a state of its own per test) and
/// create the image of the jetty.
async fn book(tag: &str) -> Book {
    let server = common::open().await;
    let report = server.import(&format!("{TEICH}// {tag}\n"), &format!("{tag}.typ")).await;
    let added = server
        .rpc(
            "asset.add",
            json!({ "work": report.work, "file_name": "steg.svg", "content": STEG_SVG, "encoding": "svg",
                    "path": "bilder/steg.svg", "alt": "Der Steg" }),
        )
        .await;
    assert!(added["result"].is_object(), "das Bild muss sich anlegen lassen: {added}");
    let work = server.state.store.load_work(report.work).await.unwrap();
    let keys = work.nodes.iter().map(|n| n.key).collect();
    let dir = server.mirror_dir_of(&report.slug);
    Book { server, work: report.work, keys, dir }
}

fn result(answer: &Value) -> &Value {
    assert!(answer.get("error").is_none_or(Value::is_null), "RPC sollte gelingen: {answer}");
    &answer["result"]
}

fn commits(dir: &std::path::Path) -> Option<u32> {
    common::git(dir, &["rev-list", "--count", "HEAD"]).map(|c| c.trim().parse().unwrap())
}

async fn get_with_cookie(server: &common::Server, uri: &str, cookie: &str) -> String {
    let request = Request::get(uri).header(header::COOKIE, cookie).body(Body::empty()).unwrap();
    let (status, _, html) = server.send(request).await;
    assert_eq!(status, StatusCode::OK, "{uri}: {html}");
    html
}

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

async fn session(server: &common::Server, name: &str) -> String {
    let (headers, answer) = mcp(
        server,
        None,
        json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize",
                "params": { "protocolVersion": "2025-06-18", "capabilities": {},
                            "clientInfo": { "name": name, "version": "1" } } }),
    )
    .await;
    assert!(answer["result"].is_object(), "initialize: {answer}");
    headers.get("mcp-session-id").unwrap().to_str().unwrap().to_owned()
}

async fn tool(server: &common::Server, session: &str, name: &str, arguments: Value) -> Value {
    let message = json!({ "jsonrpc": "2.0", "id": 7, "method": "tools/call",
                          "params": { "name": name, "arguments": arguments } });
    mcp(server, Some(session), message).await.1["result"].clone()
}

/// Ids of the posts of a section.
async fn post_ids(server: &common::Server, work: i64, key: i64) -> Vec<i64> {
    let node = server.rpc("node.get", json!({ "work": work, "key": key })).await;
    result(&node)["posts"].as_array().unwrap().iter().map(|p| p["id"].as_i64().unwrap()).collect()
}

#[tokio::test]
async fn a_key_url_from_mcp_opens_the_right_section_after_the_outline_is_rearranged() {
    let b = book("e2e-adresse").await;
    let s = session(&b.server, "teich").await;
    let added =
        tool(&b.server, &s, "add_post", json!({ "work": b.work, "key": b.keys[1], "body": "Brücke streichen?" })).await
            ["structuredContent"]
            .clone();
    let url = added["url"].as_str().unwrap_or_else(|| panic!("add_post nennt eine url: {added}")).to_owned();
    assert_eq!(url, format!("/?work={}&key={}", b.work, b.keys[1]), "kanonische Key-Adresse ohne rev");

    // „Die Brücke“ before „Der Teich“: all positions shift, the address from MCP stays valid.
    let moved =
        b.server.rpc("outline.move", json!({ "work": b.work, "node": 1, "target": 0, "place": "before" })).await;
    result(&moved);
    let (status, html) = b.server.get(&url).await;
    assert_eq!(status, StatusCode::OK, "{html}");
    assert!(
        html.contains("Holz, grau vom Regen.") && html.contains("Brücke streichen?"),
        "die MCP-url öffnet nach dem Umstellen weiter „Die Brücke“ samt Beitrag: {html}"
    );
    assert!(!html.contains("missing-key"), "bekannter Schlüssel: kein Hinweis");
    let bad: Vec<&str> =
        html.split("href=\"").skip(1).filter_map(|r| r.split('"').next()).filter(|h| h.contains("node=")).collect();
    assert!(bad.is_empty(), "alle erzeugten Links nutzen key, keiner node= (Vertrag §1): {bad:?}");
    let read = tool(&b.server, &s, "read_node", json!({ "work": b.work, "key": b.keys[0] })).await;
    assert_eq!(
        read["structuredContent"]["url"],
        format!("/?work={}&key={}", b.work, b.keys[0]),
        "auch read_node trägt die url, unabhängig von der neuen Stelle"
    );
}

#[tokio::test]
async fn two_persona_proposals_stand_at_their_place_fold_and_one_is_accepted_by_form() {
    let b = book("e2e-vorschlag").await;
    let store = &b.server.state.store;
    let teich = b.keys[0];
    let frog = "= Der Teich\n\nSeerosen treiben im Wasser.\n\nEin Frosch quakt am Rand.\n\nLibellen schwirren umher.\n";
    let fly =
        "= Der Teich\n\nSeerosen treiben im Wasser.\n\nEin Frosch sitzt am Rand.\n\nBlaue Libellen jagen Mücken.\n";
    assert_eq!(store.add_proposal(b.work, 0, frog, "@lektor", "probe-1", "").await.unwrap(), 2, "Fassung 2");
    assert_eq!(store.add_proposal(b.work, 0, fly, "@nathan", "probe-1", "").await.unwrap(), 3, "Fassung 3");

    let html = b.server.get(&format!("/?work={}&key={teich}", b.work)).await.1;
    let flow_at = html.find("<section class=\"pv-flow\"").unwrap_or_else(|| panic!("Lesefluss fehlt: {html}"));
    let flow = &html[flow_at..flow_at + html[flow_at..].find("</section>").unwrap()];
    let block = |i: usize| flow.find(&format!("<pre class=\"pv-src\" data-b=\"{i}\">")).expect("Block im Lesefluss");
    let lektor = flow.find(&format!("data-pv=\"{teich}-2\"")).expect("Vorschlag des Lektors im Lesefluss");
    let nathan = flow.find(&format!("data-pv=\"{teich}-3\"")).expect("Vorschlag Nathans im Lesefluss");
    // Blocks: 0 heading, 1 Seerosen, 2 Frosch, 3 Libellen.
    assert!(
        block(2) < lektor && lektor < block(3),
        "der Frosch-Vorschlag steht direkt nach dem Frosch-Absatz, vor den Libellen"
    );
    assert!(block(3) < nathan, "der Libellen-Vorschlag steht nach dem Libellen-Absatz, nicht davor");
    let details = |at: usize| {
        let start = flow[..at].rfind("<details").unwrap();
        &flow[start..at + flow[at..].find("</details>").unwrap()]
    };
    for (at, handle) in [(lektor, "@lektor"), (nathan, "@nathan")] {
        let d = details(at);
        assert_eq!(d.matches("class=\"pv-line\"").count(), 1, "genau eine Kopfzeile je Stück: {d}");
        assert!(d.contains(&format!("({handle})")), "die Kopfzeile nennt die Persona: {d}");
        assert!(d.contains("--pc-line:") && d.contains("--pc-line-dark:"), "Linienfarbe je Thema: {d}");
        assert!(!d.contains("unbekannt"), "kein zweiter Kopf „Lektor … unbekannt“ mehr: {d}");
        assert!(d.starts_with("<details") && d[..d.find('>').unwrap()].ends_with(" open"), "aufgeklappt: {d}");
    }

    // Fold (cookie): only the lector proposal becomes a line at its place.
    let folded =
        get_with_cookie(&b.server, &format!("/?work={}&key={teich}", b.work), &format!("baruch_folded={teich}-2"))
            .await;
    let tag = |rev: u32| {
        let at = folded.find(&format!("data-pv=\"{teich}-{rev}\"")).unwrap();
        let start = folded[..at].rfind("<details").unwrap();
        folded[start..at + folded[at..].find('>').unwrap() + 1].to_owned()
    };
    assert!(!tag(2).ends_with(" open>"), "eingeklappt = nur die farbige Linie: {}", tag(2));
    assert!(tag(3).ends_with(" open>"), "der andere bleibt offen: {}", tag(3));

    // Accept via form without JavaScript: one commit, back to the key address.
    let before = commits(&b.dir);
    let (status, location) = b
        .server
        .post_form("/act", &format!("method=revision.accept&work={}&node=0&key={teich}&rev=3&view=edit", b.work))
        .await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Übernehmen per Formular");
    assert_eq!(location.as_deref(), Some(format!("/?work={}&key={teich}&rev=3", b.work).as_str()), "Key-Redirect");
    if let (Some(before), Some(after)) = (before, commits(&b.dir)) {
        assert_eq!(after, before + 1, "genau ein Spiegel-Commit für die Übernahme");
    }
    let html = b.server.get(&format!("/?work={}&key={teich}", b.work)).await.1;
    assert!(html.contains("Blaue Libellen jagen Mücken."), "Nathans Fassung ist jetzt aktuell");
    assert!(!html.contains(&format!("data-pv=\"{teich}-3\"")), "der übernommene ist kein offener Vorschlag mehr");
    assert!(html.contains(&format!("data-pv=\"{teich}-2\"")), "der Lektor-Vorschlag bleibt offen");
}

#[tokio::test]
async fn posts_are_converted_moved_and_deleted_and_mcp_may_not_touch_foreign_ones() {
    let b = book("e2e-beitraege").await;
    let (teich, bruecke) = (b.keys[0], b.keys[1]);
    let s = session(&b.server, "teich").await;
    let own = tool(&b.server, &s, "add_post", json!({ "work": b.work, "key": teich, "body": "Mehr über Kröten." }))
        .await["structuredContent"]["id"]
        .as_i64()
        .unwrap();
    let human =
        result(&b.server.rpc("post.add", json!({ "work": b.work, "key": teich, "body": "Vom Menschen." })).await)["id"]
            .as_i64()
            .unwrap();

    // MCP: foreign post → isError, nothing changed, no commit.
    let before = (post_ids(&b.server, b.work, teich).await, commits(&b.dir));
    for (name, args) in [
        ("post_delete", json!({ "work": b.work, "id": human })),
        ("post_move", json!({ "work": b.work, "id": human, "key": bruecke })),
        ("post_to_idea", json!({ "work": b.work, "id": human, "words": "Mensch" })),
    ] {
        let answer = tool(&b.server, &s, name, args).await;
        assert_eq!(answer["isError"], true, "{name} an einem fremden Beitrag ist verboten: {answer}");
        // Round 39a: MCP answers in English.
        assert!(answer["content"][0]["text"].as_str().unwrap().starts_with("Only your own posts"), "{answer}");
    }
    assert_eq!((post_ids(&b.server, b.work, teich).await, commits(&b.dir)), before, "nichts geändert");

    // Convert an own post into an idea over MCP: exactly one commit with einfaelle.md.
    let converted =
        tool(&b.server, &s, "post_to_idea", json!({ "work": b.work, "id": own, "words": ["Kröten"] })).await;
    assert_eq!(converted["isError"], false, "eigener Beitrag: {converted}");
    let idea = converted["structuredContent"]["idea"].clone();
    assert_eq!(idea["url"], format!("/?work={}&key={teich}", b.work), "Einfall mit Bezug und url: {idea}");
    if let (Some(before), Some(after)) = (before.1, commits(&b.dir)) {
        assert_eq!(after, before + 1, "Umwandeln = EIN Commit (Einfall anlegen und Beitrag löschen)");
        let files = common::git(&b.dir, &["show", "--name-only", "--format=", "HEAD"]).unwrap();
        assert!(files.lines().any(|f| f == "einfaelle.md"), "der Einfall steht im selben Commit: {files}");
    }
    assert_eq!(post_ids(&b.server, b.work, teich).await, vec![human], "der umgewandelte Beitrag ist fort");

    // The human moves and deletes through forms without JavaScript.
    let (status, location) = b
        .server
        .post_form(
            "/beitraege/act",
            &format!("method=post.move&work={}&key={teich}&id={human}&target_key={bruecke}", b.work),
        )
        .await;
    assert_eq!(status, StatusCode::SEE_OTHER, "verschieben per Formular");
    assert_eq!(location.as_deref(), Some(format!("/?work={}&key={teich}", b.work).as_str()), "zurück zum gezeigten");
    assert_eq!(post_ids(&b.server, b.work, bruecke).await, vec![human], "der Beitrag steht jetzt bei der Brücke");
    let (status, _) = b
        .server
        .post_form("/beitraege/act", &format!("method=post.delete&work={}&key={bruecke}&id={human}", b.work))
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "Löschen ohne Bestätigung wird abgelehnt");
    let (status, _) = b
        .server
        .post_form("/beitraege/act", &format!("method=post.delete&work={}&key={bruecke}&id={human}&confirm=1", b.work))
        .await;
    assert_eq!(status, StatusCode::SEE_OTHER, "mit Bestätigung gelöscht");
    assert!(post_ids(&b.server, b.work, bruecke).await.is_empty(), "kein Beitrag mehr");
    if let Some(message) = common::git(&b.dir, &["log", "-1", "--format=%B"]) {
        assert!(message.contains("Vom Menschen."), "der Lösch-Commit hält den Beitrag fest: {message}");
    }
}

#[tokio::test]
async fn an_unchanged_empty_title_can_be_saved_but_not_a_new_empty_one() {
    let b = book("e2e-titel").await;
    let untitled = b.keys[2];
    let work = b.server.state.store.load_work(b.work).await.unwrap();
    let node = work.nodes.iter().find(|n| n.key == untitled).unwrap();
    assert_eq!(node.title, "", "Voraussetzung: der Abschnitt hat aus dem Import keinen Titel");
    let body = node.revision(node.current_number().unwrap()).unwrap().body.clone();
    let saved = b
        .server
        .rpc("revision.save", json!({ "work": b.work, "key": untitled, "body": format!("{body}\nNeu dazu.\n") }))
        .await;
    assert!(result(&saved)["rev"].as_u64().is_some(), "unveränderte leere Überschrift: speicherbar: {saved}");
    let refused =
        b.server.rpc("revision.save", json!({ "work": b.work, "key": b.keys[1], "body": "= \n\nHolz.\n" })).await;
    assert!(refused["error"].is_object(), "ein neu geleerter Titel wird weiter abgelehnt: {refused}");
}

#[tokio::test]
async fn the_typst_preview_sets_pages_with_the_image_and_reports_an_error_with_its_section() {
    let b = book("e2e-satz").await;
    let (status, html) = b.server.get(&format!("/werke/{}/satz?key={}", b.work, b.keys[1])).await;
    assert_eq!(status, StatusCode::OK, "{html}");
    let src = html
        .split("<img src=\"")
        .nth(1)
        .and_then(|r| r.split('"').next())
        .unwrap_or_else(|| panic!("die Satz-Seite zeigt eine Seite als Bild: {html}"))
        .to_owned();
    assert!(src.starts_with(&format!("/werke/{}/satz/", b.work)) && src.ends_with(".svg"), "{src}");
    let (status, headers, svg) = b.server.send(Request::get(&src).body(Body::empty()).unwrap()).await;
    assert_eq!(status, StatusCode::OK, "{src}");
    assert_eq!(headers[header::CONTENT_TYPE], "image/svg+xml", "als SVG ausgeliefert");
    assert!(
        svg.starts_with("<svg") && svg.contains("data:image/svg+xml"),
        "die Brücken-Seite trägt das Bild des Werks"
    );

    // A Typst error in „Die Brücke“: no pages, message with section and key link.
    let saved = b
        .server
        .rpc(
            "revision.save",
            json!({ "work": b.work, "key": b.keys[1], "body": "= Die Brücke\n\nZahl: #calc.pow(\"zwei\", 2)\n" }),
        )
        .await;
    result(&saved);
    let answer = b.server.rpc("preview.typst", json!({ "work": b.work })).await;
    let r = result(&answer);
    assert_eq!(r["pages"], 0, "bei einem Fehler gibt es keine Seiten: {r}");
    let error = r["diagnostics"].as_array().unwrap().iter().find(|d| d["severity"] == "error").expect("ein Fehler");
    assert_eq!(error["key"], b.keys[1], "der Fehler gehört zu „Die Brücke“ (fester Schlüssel): {error}");
    assert_eq!(error["section"], "2 Die Brücke", "Abschnitt mit Nummer und Titel");
    let (_, html) = b.server.get(&format!("/werke/{}/satz", b.work)).await;
    assert!(
        html.contains(&format!("href=\"/?work={}&amp;key={}\"", b.work, b.keys[1])),
        "die Meldung führt in den Editor des Abschnitts (Key-Form): {html}"
    );
    let s = session(&b.server, "satz").await;
    let check = tool(&b.server, &s, "typst_check", json!({ "work": b.work })).await["structuredContent"].clone();
    assert_eq!(
        (check["ok"].clone(), check["errors"].clone()),
        (json!(false), json!(1)),
        "typst_check meldet ihn: {check}"
    );
}
