// SPDX-License-Identifier: AGPL-3.0-or-later
//! Round 18, acceptance of the integrator: a freely invented work with all constructs of the round (scenes, work
//! orders, footnotes, quotations, bullet lists, term lists, nested lists, line break `\`, `#link`, references, two
//! images in one block, an island) runs end to end through all three parts: ribbon (18a), constructs (18b) and typeset
//! view per work (18c). Every step goes the way the user interface goes – without JavaScript through forms, with
//! JavaScript through the same RPC calls as the widgets. No network, no model; the typesetting runs only where it is
//! checked.

use crate::common;

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::{Value, json};

fn ok(answer: &Value, what: &str) -> Value {
    assert!(answer["error"].is_null(), "{what} sollte gelingen: {answer}");
    answer["result"].clone()
}

/// Work template with `#szene` and `#arbeit`; one chapter with every construct in canonical spelling (the way `emit`
/// writes it), a second chapter as neighbour. Own closing line: own state in the typesetting cache. Canonical also
/// means: no reference directly before a full stop – `@kai.` is written by `emit` cautiously as `#ref(<kai>).`
/// (equivalent in Typst, but not byte-identical; concerns only an edited block).
const HAFEN: &str = "#let szene(body) = block(fill: luma(240), inset: 6pt, body)\n\
#let arbeit(body) = rect(inset: 6pt, body)\n\n\
= Hafen\n\n\
Die Möwen kreisen über dem Kai.#footnote[Beobachtet im Frühjahr.]\n\n\
Sie rufen #quote[Heimkehr!] über das Wasser.\n\n\
#szene[\nEin Fischer flickt sein Netz.\n]\n\n\
#arbeit[\nBeschreibe den Geruch des Hafens.\n]\n\n\
1. Netze prüfen\n2. Boote zählen\n\n\
/ Kai: befestigtes Ufer\n/ Poller: Pfosten zum Festmachen\n\n\
- Fische\n  - Hering\n  - Dorsch\n- Muscheln\n\n\
Erste Zeile \\\nzweite Zeile mit #link(\"https://example.org/hafen\")[Hafenordnung], siehe @kai am Ufer.\n\n\
#grid(columns: 2, image(\"bilder/boot.svg\"), image(\"bilder/netz.svg\"))\n\n\
#let tiefe = 7\n\n\
= Abend\n\nDie Laternen gehen an.\n\n// Test: round18-abnahme\n";

async fn add_image(server: &common::Server, work: i64, file: &str, path: &str) {
    let svg =
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"8\" height=\"8\"><rect width=\"8\" height=\"8\"/></svg>\n";
    let answer = server
        .rpc(
            "asset.add",
            json!({ "work": work, "file_name": file, "content": svg, "encoding": "svg", "path": path,
                    "caption": "Ein Boot", "alt": "Boot am Kai" }),
        )
        .await;
    ok(&answer, "asset.add");
}

/// (revision number, text) of the current revision.
async fn current(server: &common::Server, work: i64, key: i64) -> (u64, String) {
    let got = ok(&server.rpc("work.get", json!({ "work": work })).await, "work.get");
    let nodes = got["work"]["nodes"].as_array().unwrap().clone();
    let node = nodes.iter().find(|n| n["key"] == key).unwrap();
    let rev = node["revisions"].as_array().unwrap().iter().find(|r| r["status"] == "current").unwrap();
    (rev["number"].as_u64().unwrap(), rev["body"].as_str().unwrap().to_owned())
}

async fn hafen(server: &common::Server) -> (i64, i64) {
    let work = server.import(HAFEN, "hafen.typ").await.work;
    let loaded = server.state.store.load_work(work).await.unwrap();
    let key = loaded.nodes.iter().find(|n| n.title == "Hafen").expect("Kapitel Hafen").key;
    (work, key)
}

fn urlencoding(text: &str) -> String {
    text.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// Content of `<style id="ws-style" data-work="W">…</style>`.
fn served_style(page: &str, work: i64) -> Option<String> {
    let open = format!("<style id=\"ws-style\" data-work=\"{work}\">");
    let at = page.find(&open)? + open.len();
    Some(page[at..at + page[at..].find("</style>")?].to_owned())
}

#[tokio::test]
async fn every_construct_of_the_round_goes_typst_to_satzbild_to_typst_byte_equal() {
    let server = common::open().await;
    let (work, key) = hafen(&server).await;
    add_image(&server, work, "boot.svg", "bilder/boot.svg").await;
    add_image(&server, work, "netz.svg", "bilder/netz.svg").await;
    let (_, body) = current(&server, work, key).await;
    let parsed = ok(&server.rpc("wysiwyg.parse", json!({ "work": work, "body": body })).await, "wysiwyg.parse");
    let blocks = parsed["blocks"].as_array().unwrap().clone();
    let kinds: Vec<&str> = blocks.iter().map(|b| b["kind"].as_str().unwrap()).collect();
    assert_eq!(
        kinds,
        [
            "heading",
            "paragraph",
            "paragraph",
            "scene",
            "work",
            "enum",
            "terms",
            "list",
            "paragraph",
            "island",
            "island"
        ],
        "jedes Konstrukt wird ein Block seiner Art, nur Bilderraster und #let bleiben Inseln: {parsed}"
    );
    let raws: String = blocks.iter().map(|b| b["raw"].as_str().unwrap()).collect();
    assert_eq!(raws, body, "Verkettung der raw = Text, byte-gleich");

    let html = parsed["html"].as_str().unwrap();
    for (piece, why) in [
        ("ws-footnote", "Fußnote"),
        ("q class=\"ws-quote\"", "Zitat im Satz"),
        ("<ol start=\"1\">", "Aufzählung"),
        ("<dl><dt>Kai</dt>", "Begriffsliste"),
        ("data-linebreak=\"1\"", "erzwungener Umbruch"),
        ("class=\"ws-link\"", "Link"),
        ("class=\"ws-ref\"", "Verweis als Schaltfläche"),
    ] {
        assert!(html.contains(piece), "{why} im Satzbild ({piece}): {html}");
    }
    assert_eq!(html.matches("<img class=\"ws-image\"").count(), 2, "beide Bilder des Rasters sichtbar: {html}");
    assert!(
        html.contains(&format!("src=\"/werke/{work}/bilder/pfad?p=bilder%2Fnetz.svg\"")),
        "Bildadresse in Normalform: {html}"
    );

    // Everything untouched (keep): byte-identical.
    let keep: Vec<Value> = (0..blocks.len()).map(|i| json!({ "keep": i })).collect();
    let same = ok(
        &server.rpc("wysiwyg.serialize", json!({ "work": work, "body": body, "edits": keep })).await,
        "wysiwyg.serialize keep",
    );
    assert_eq!(same["body"], body, "lauter keep: byte-gleich");

    // Every block the way the widget sends it after an edit (`replaces` + block or, for islands, `raw`): because the
    // text is written canonically, `emit` writes it back byte-identical – for every construct.
    for (i, block) in blocks.iter().enumerate().skip(1) {
        let mut edits: Vec<Value> = (0..blocks.len()).map(|j| json!({ "keep": j })).collect();
        edits[i] = if block["kind"] == "island" {
            json!({ "replaces": i, "raw": block["raw"] })
        } else {
            json!({ "replaces": i, "block": block })
        };
        let again = server.rpc("wysiwyg.serialize", json!({ "work": work, "body": body, "edits": edits })).await;
        let again = ok(&again, &format!("wysiwyg.serialize Block {i} ({})", block["kind"]));
        assert_eq!(again["body"], body, "Block {i} ({}) neu geschrieben bleibt byte-gleich: {block}", block["kind"]);
    }

    // Island in place: only it changes, and exactly that is stored.
    let island = kinds.iter().rposition(|k| *k == "island").unwrap();
    let mut edits: Vec<Value> = (0..blocks.len()).map(|j| json!({ "keep": j })).collect();
    edits[island] = json!({ "replaces": island, "raw": "#let tiefe = 9\n" });
    let text = ok(
        &server.rpc("wysiwyg.serialize", json!({ "work": work, "body": body, "edits": edits })).await,
        "Insel an Ort und Stelle",
    )["body"]
        .as_str()
        .unwrap()
        .to_owned();
    assert_eq!(text, body.replace("#let tiefe = 7", "#let tiefe = 9"), "nur die Insel ändert sich");
    ok(&server.rpc("outline.save", json!({ "work": work, "key": key, "body": text })).await, "outline.save");
    assert_eq!(current(&server, work, key).await.1, text, "gespeichert, wie das Satzbild es schickte");
}

#[tokio::test]
async fn the_ribbon_offers_the_template_formats_and_a_snippet_saved_without_javascript_becomes_a_scene() {
    let server = common::open().await;
    let (work, key) = hafen(&server).await;
    let formats = ok(&server.rpc("ribbon.formats", json!({ "work": work })).await, "ribbon.formats");
    let list = formats["formats"].as_array().unwrap();
    let cmds: Vec<&str> = list.iter().map(|f| f["cmd"].as_str().unwrap()).collect();
    assert_eq!(
        cmds,
        ["scene", "work", "footnote", "quote", "list", "enum", "terms", "break", "link", "xref"],
        "Szene und Arbeit aus der Vorlage, dann die eingebauten Formate: {formats}"
    );

    let (status, page) = server.get(&format!("/?work={work}&key={key}")).await;
    assert_eq!(status, StatusCode::OK, "Editor lädt");
    for cmd in &cmds {
        assert!(page.contains(&format!("data-ribbon-cmd=\"{cmd}\"")), "Knopf {cmd} im Menüband: {page}");
    }
    assert!(page.contains("data-ribbon-image"), "Knopf „Bild“ im Menüband");
    assert!(page.contains("data-image-insert"), "sein Ziel „Bild einfügen …“ steht im Editor");
    assert!(page.contains("<textarea"), "ohne JavaScript bleibt der Quelltext das Eingabefeld");

    // Without JavaScript one types the Typst text of the button oneself; with JavaScript ribbon.ts inserts exactly this
    // text. Both ways end in the same stored text, and the typeset view recognises the scene.
    let snippet = list[0]["snippet"].as_str().unwrap().replace("\n\n", "\nDie Fähre legt an.\n");
    let (rev, body) = current(&server, work, key).await;
    let text = format!("{}\n\n{snippet}", body.trim_end());
    let form = format!("method=revision.save&work={work}&key={key}&based_on={rev}&body={}", urlencoding(&text));
    let (status, _) = server.post_form("/act", &form).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Speichern über das Formular ohne JavaScript");
    let (_, saved) = current(&server, work, key).await;
    assert_eq!(saved, text, "der Text des Knopfs ist gespeichert");
    let parsed = ok(&server.rpc("wysiwyg.parse", json!({ "work": work, "body": saved })).await, "wysiwyg.parse");
    let last = parsed["blocks"].as_array().unwrap().last().unwrap().clone();
    assert_eq!(last["kind"], "scene", "der eingefügte Text ist eine Szene im Satzbild: {last}");

    // With JavaScript: the compiled widget sends the command while the typeset view is open and otherwise inserts the
    // text.
    let (status, js) = server.get("/static/js/ribbon.js").await;
    assert_eq!(status, StatusCode::OK, "ribbon.js wird ausgeliefert");
    assert!(js.contains("sendWsCommand") && js.contains("data-ribbon-snippet"), "ribbon.js nutzt beide Wege");
}

#[tokio::test]
async fn work_sheet_settings_reach_the_html_before_any_script_and_survive_a_restart() {
    let server = common::open().await;
    let (work, key) = hafen(&server).await;
    let (_, before) = server.get(&format!("/?work={work}&key={key}")).await;
    let style_before = served_style(&before, work).expect("Stil des Satzbilds steht schon im HTML");
    assert!(!style_before.contains("Probe Antiqua"), "vorher nur die Vorlage: {style_before}");

    let form = format!(
        "method=work.sheet_set&work={work}&font=%22Probe+Antiqua%22%2C+serif&heading_font=&size=12pt&fill=%23223344&heading_fill=&background=%23fdf6e3&width=14cm"
    );
    let (status, location) = server.post_form("/einstellungen/werk/satzbild", &form).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Formular ohne JavaScript");
    assert_eq!(location.as_deref(), Some(format!("/einstellungen/werk?work={work}&ok=satzbild#satzbild").as_str()));

    let check = |page: &str, why: &str| {
        let style = served_style(page, work).unwrap_or_else(|| panic!("{why}: Stil fehlt im HTML"));
        for piece in ["Probe Antiqua", "12pt", "#223344", "#fdf6e3", "14cm"] {
            assert!(style.contains(piece), "{why}: {piece} steht im Stil (kein Flackern, kein RPC nötig): {style}");
        }
        style
    };
    let (_, after) = server.get(&format!("/?work={work}&key={key}")).await;
    let style = check(&after, "nach dem Speichern");
    let rpc = ok(&server.rpc("wysiwyg.style", json!({ "work": work })).await, "wysiwyg.style");
    assert_eq!(rpc["css"].as_str().unwrap(), style, "Widget und HTML haben dasselbe CSS");

    // Restart: new state on the same data folder.
    let common::Server { data, state, app } = server;
    drop(app);
    drop(state);
    let state = Arc::new(baruch_server::AppState::open(data.path()).await.expect("Datenordner öffnet wieder"));
    let restarted = common::Server { app: baruch_server::router(state.clone()), state, data };
    let (_, page) = restarted.get(&format!("/?work={work}&key={key}")).await;
    check(&page, "nach dem Neustart");

    // Typeset-view settings do not change the typesetting: the Typst source stays that of the work.
    let source = baruch_server::typeset::prepare(&restarted.state, work).await.unwrap();
    assert!(!source.source.main.contains("Probe Antiqua"), "Typst bleibt unberührt");
}

#[tokio::test]
async fn an_image_lands_at_the_caret_with_its_path_in_the_same_revision() {
    let server = common::open().await;
    let (work, key) = hafen(&server).await;
    add_image(&server, work, "leuchtturm.svg", "bilder/leuchtturm.svg").await;
    let (rev, body) = current(&server, work, key).await;
    // Cursor in the middle of the sentence about the seagulls: the image moves to the next line boundary, as a block of
    // its own.
    let offset = body.find("kreisen").map(|b| body[..b].encode_utf16().count()).unwrap();
    let form = format!("work={work}&key={key}&rev={rev}&name=leuchtturm.svg&place=offset:{offset}");
    let (status, _) = server.post_form("/werke/bilder/einfuegen", &form).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "eingefügt (Formular, auch ohne JavaScript)");
    let (new_rev, text) = current(&server, work, key).await;
    assert!(new_rev > rev, "eine neue Fassung");
    let after_gulls =
        text.find("#footnote[Beobachtet im Frühjahr.]\n").unwrap() + "#footnote[Beobachtet im Frühjahr.]\n".len();
    assert!(
        text[after_gulls..].starts_with("\n#figure(image(\"bilder/leuchtturm.svg\""),
        "das Bild steht direkt hinter dem Absatz am Cursor: {text}"
    );
    let parsed = ok(&server.rpc("wysiwyg.parse", json!({ "work": work, "body": text })).await, "wysiwyg.parse");
    let kinds: Vec<&str> = parsed["blocks"].as_array().unwrap().iter().map(|b| b["kind"].as_str().unwrap()).collect();
    assert_eq!(kinds[..4], ["heading", "paragraph", "island", "paragraph"], "eigener Block, Rest unberührt: {kinds:?}");
    let assets = ok(&server.rpc("asset.list", json!({ "work": work })).await, "asset.list");
    assert!(assets.to_string().contains("bilder/leuchtturm.svg"), "der Pfad des Bilds ist vergeben: {assets}");
}

#[tokio::test]
async fn a_foreign_page_cannot_start_a_typeset_and_cancelling_without_a_run_says_so() {
    let server = common::open().await;
    let (work, key) = hafen(&server).await;
    for uri in [format!("/werke/{work}/satz"), format!("/werke/{work}/pdf")] {
        let request = Request::get(&uri).header("sec-fetch-site", "cross-site").body(Body::empty()).unwrap();
        let (status, _, _) = server.send(request).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{uri} von einer fremden Seite → 403 ohne Typst-Lauf");
    }
    let foreign = Request::post(format!("/werke/{work}/satz/abbrechen"))
        .header("origin", "https://fremd.example")
        .header("content-type", "application/x-www-form-urlencoded")
        .body(Body::empty())
        .unwrap();
    let (status, _, _) = server.send(foreign).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "Abbrechen nur von der eigenen Seite");
    let (status, location) = server.post_form(&format!("/werke/{work}/satz/abbrechen"), &format!("key={key}")).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Abbrechen ohne JavaScript");
    assert!(
        location.as_deref().is_some_and(|l| l.starts_with(&format!("/werke/{work}/satz?abgebrochen=0"))),
        "ohne laufenden Satz ehrlich abgebrochen=0: {location:?}"
    );
    let cancel = ok(&server.rpc("preview.cancel", json!({ "work": work })).await, "preview.cancel");
    assert_eq!(cancel["cancelled"], false, "über RPC ebenso: {cancel}");
}
