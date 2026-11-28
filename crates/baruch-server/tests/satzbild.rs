// SPDX-License-Identifier: AGPL-3.0-or-later
//! Typeset view polish (contract Round 6, „satzbild“) in the server: scenes and labels over RPC, images in islands with
//! the address of the images page, presentation only from rules of the top level, style and widget without an empty
//! area in source text mode, default typeset view after every redraw, without JavaScript only the text field. All texts
//! freely invented.

use crate::common;

use axum::http::StatusCode;
use baruch_server::wysiwyg;
use serde_json::{Value, json};

/// Freely invented work with scene, label at the heading and figure.
const BOOK: &str = r##"#set text(size: 10.5pt)
#show footnote: set text(size: 8.5pt)
#let f() = { set par(first-line-indent: 0pt) }
#set par(first-line-indent: 1.1em)
#let szene(body) = block(fill: luma(245), inset: 6pt)[#body]

= Die Werkstatt
<werkstatt>
#szene[
  Der Morgen begann mit Hobeln.

  Jonas strich über das Brett.#footnote[Eiche.]
]

#figure(image("/bilder/erfunden.svg"), caption: [Ein erfundener Grundriss.])
"##;

async fn call(server: &common::Server, work: i64, method: &str, params: Value) -> Result<Value, (i64, String)> {
    wysiwyg::call(&server.state, work, method, params).await.map_err(|e| (e.code, e.message))
}

/// Create an invented image for the path (since Round 11 the typeset view shows „Bild fehlt“ without the image in the
/// work).
async fn add_image(server: &common::Server, work: i64, file_name: &str, path: &str) {
    let svg =
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"4\" height=\"4\"><rect width=\"4\" height=\"4\"/></svg>\n";
    let answer = server
        .rpc(
            "asset.add",
            json!({ "work": work, "file_name": file_name, "content": svg, "encoding": "svg", "path": path }),
        )
        .await;
    assert!(answer["error"].is_null(), "Bild angelegt: {answer}");
}

fn script(name: &str) -> &'static str {
    baruch_server::SCRIPTS.iter().find(|(n, _)| *n == name).unwrap_or_else(|| panic!("{name} eingebettet")).1
}

#[tokio::test]
async fn parse_shows_scene_label_and_the_image_with_the_address_of_the_image_page() {
    let server = common::open().await;
    let work = server.import(BOOK, "werkstatt.typ").await.work;
    let body = &BOOK[BOOK.find("= Die Werkstatt").unwrap()..];
    let missing = call(&server, work, "wysiwyg.parse", json!({ "body": body })).await.unwrap();
    assert!(
        missing["html"].as_str().unwrap().contains("Bild fehlt: <code>/bilder/erfunden.svg</code>"),
        "Runde 11: ohne Bild zum Pfad meldet das Satzbild es deutlich: {missing}"
    );
    add_image(&server, work, "erfunden.svg", "/bilder/erfunden.svg").await;
    let answer = call(&server, work, "wysiwyg.parse", json!({ "body": body })).await.unwrap();
    let blocks = answer["blocks"].as_array().unwrap();
    assert_eq!(blocks.len(), 3, "Überschrift, Szene, Abbildung: {answer}");
    assert_eq!(blocks[0]["label"], "werkstatt", "das Label gehört zur Überschrift: {answer}");
    assert_eq!(blocks[1]["kind"], "scene", "die Szene direkt nach der Überschrift: {answer}");
    assert_eq!(blocks[1]["body"].as_array().map(Vec::len), Some(2), "zwei Absätze im Rumpf: {answer}");
    assert_eq!(blocks[2]["kind"], "island", "die Abbildung bleibt eine Insel: {answer}");
    let html = answer["html"].as_str().unwrap();
    let src = format!("src=\"/werke/{work}/bilder/pfad?p=bilder%2Ferfunden.svg\"");
    assert!(
        html.contains(&format!("<img class=\"ws-image\" {src} alt=\"\" loading=\"eager\">")),
        "Bild mit Adresse der Bilderseite: {html}"
    );
    // Round 7: the figure appears as an image with caption, not as raw text.
    assert!(
        html.contains("<div class=\"ws-block ws-figure\" data-b=\"2\" data-kind=\"island\" data-image=\"1\" contenteditable=\"false\""),
        "die Bildinsel bleibt nicht bearbeitbar und ist als Abbildung markiert: {html}"
    );
    assert!(!html.contains("<code class=\"ws-raw\">#figure"), "kein sichtbarer Rohtext der Abbildung: {html}");
    assert!(html.contains("<span class=\"ws-caption\">Ein erfundener Grundriss.</span>"), "Unterschrift: {html}");
    assert!(html.contains("data-heading-label=\"1\" data-v=\"werkstatt\""), "Label am Überschriftsblock: {html}");
}

#[tokio::test]
async fn serialize_edits_one_paragraph_inside_a_scene() {
    let server = common::open().await;
    let work = server.state.default_work().await.unwrap();
    let body =
        "= Die Werkstatt\n<werkstatt>\n#szene[\n  Der Morgen begann mit Hobeln.\n\n  Jonas strich über das Brett.\n]\n";
    let edits = json!([
        { "keep": 0 },
        { "replaces": 1, "body": [
            { "keep": 0 },
            { "replaces": 1, "block": { "kind": "paragraph", "inlines": [{ "t": "text", "v": "Jonas ging." }] } }
        ] }
    ]);
    let answer = call(&server, work, "wysiwyg.serialize", json!({ "body": body, "edits": edits })).await.unwrap();
    assert_eq!(
        answer["body"],
        body.replace("Jonas strich über das Brett.", "Jonas ging."),
        "nur der bearbeitete Absatz in der Szene ändert sich: {answer}"
    );
    let wrong = json!([{ "replaces": 0, "body": [] }]);
    let refused = call(&server, work, "wysiwyg.serialize", json!({ "body": body, "edits": wrong })).await;
    assert_eq!(
        refused.map_err(|e| e.0),
        Err(baruch_server::rpc::INVALID_PARAMS),
        "Überschrift hat keinen Rumpf → -32602"
    );
}

#[tokio::test]
async fn the_style_reads_only_top_level_rules_of_the_template() {
    let server = common::open().await;
    let work = server.import(BOOK, "werkstatt.typ").await.work;
    let answer = call(&server, work, "wysiwyg.style", json!({})).await.unwrap();
    assert_eq!(answer["style"]["size"], "10.5pt", "#show footnote: set text(size:) zählt nicht: {answer}");
    assert_eq!(answer["style"]["indent"], "1.1em", "die Regel im Funktionsrumpf zählt nicht: {answer}");
}

#[tokio::test]
async fn the_diff_shows_images_in_islands() {
    let server = common::open().await;
    let work = server.import("= Die Werkstatt\n\nText.\n", "werkstatt.typ").await.work;
    add_image(&server, work, "neu.svg", "/bilder/neu.png").await;
    let body = "= Die Werkstatt\n\nText.\n\n#image(\"/bilder/neu.png\")\n";
    let rev = server.state.store.add_proposal(work, 0, body, "Lektor", "erfunden", "Bild").await.unwrap();
    let answer = call(&server, work, "wysiwyg.diff", json!({ "node": 0, "rev": rev })).await.unwrap();
    let html = answer["html"].as_str().unwrap();
    assert!(html.contains("class=\"ws-ins\""), "die neue Bildinsel ist markiert: {html}");
    assert!(
        html.contains(&format!("<img class=\"ws-image\" src=\"/werke/{work}/bilder/pfad?p=bilder%2Fneu.png\"")),
        "Bild im Satzbild-Diff mit Adresse der Bilderseite: {html}"
    );
}

#[tokio::test]
async fn without_javascript_only_the_text_field_is_there_and_the_area_stays_hidden() {
    let server = common::open().await;
    let work = server.state.default_work().await.unwrap();
    let (status, html) = server.get(&format!("/?work={work}&node=2&rev=2")).await;
    assert_eq!(status, StatusCode::OK, "Editor-Seite");
    let start = html.find("data-widget=\"wysiwyg\"").expect("Satzbild-Bereich vorhanden");
    let tag = &html[html[..start].rfind('<').unwrap()..start + html[start..].find('>').unwrap()];
    assert!(!tag.contains("data-ws-open"), "ohne JavaScript nie „offen“: {tag}");
    // Wish 2026-11-06: the area encloses toolbar, sheet and status line; hidden without JavaScript are toolbar, typeset
    // view and its hint – the text field and „Speichern“ stay.
    for hidden in ["data-edit-toolbar hidden>", "<div class=\"ws-wrap\" data-ws-wrap hidden>", "data-ws-hint hidden>"] {
        assert!(html.contains(hidden), "ohne JavaScript verborgen: {hidden}");
    }
    assert!(html.contains("<textarea name=\"body\""), "das Textfeld ist da: {html}");
    // Redraw via ui.render (click in the outline): the same area, hidden again until the widget opens it; no source
    // text state comes from the server.
    let rendered = server.rpc("ui.render", json!({ "work": work, "node": 3 })).await;
    let editor =
        rendered["result"]["editor"].as_str().unwrap_or_else(|| panic!("ui.render liefert den Editor: {rendered}"));
    let at = editor.find("data-widget=\"wysiwyg\"").expect("Satzbild-Bereich nach dem Neuzeichnen");
    let tag = &editor[editor[..at].rfind('<').unwrap()..at + editor[at..].find('>').unwrap()];
    assert!(
        !tag.contains("data-ws-open") && editor.contains("<div class=\"ws-wrap\" data-ws-wrap hidden>"),
        "nach ui.render wie beim Laden: Satzbild verborgen, bis das Widget es öffnet: {tag}"
    );
    let (_, css) = server.get("/static/app.css").await;
    let rule = |selector: &str| -> String {
        let at = css.find(&format!("{selector} {{")).unwrap_or_else(|| panic!("Regel {selector} fehlt: {css}"));
        css[at..at + css[at..].find('}').unwrap()].to_owned()
    };
    // In source text mode the typeset view is gone entirely (no empty area above the text field), the text field fills
    // the sheet; when open the typeset view fills the sheet.
    let wrap = rule(".ws-wrap[hidden]");
    assert!(wrap.contains("display: none"), "verborgenes Satzbild nimmt keinen Platz: {wrap}");
    let area = rule(".editor-form textarea");
    assert!(area.contains("flex: 1 0 auto"), "das Textfeld füllt das Blatt: {area}");
    let open = rule(".ws-wrap");
    assert!(open.contains("flex: 1 0 auto"), "das offene Satzbild füllt das Blatt: {open}");
}

#[test]
fn the_widget_opens_the_satzbild_by_default_and_only_an_explicit_choice_keeps_the_source() {
    let js = script("wysiwyg.js");
    assert!(
        js.contains("store.get(COOKIE) === \"quelltext\""),
        "Quelltext nur, wenn das Cookie ausdrücklich „quelltext“ sagt; sonst Satzbild: {js}"
    );
    assert!(!js.contains("?? \"satz\""), "kein Rückfall auf einen anderen Cookie-Wert als Vorgabe");
    assert!(
        js.contains("let fundstelle = window.location.hash === \"#fundstelle\"") && js.contains("fundstelle = false"),
        "#fundstelle gilt nur für den Editor beim Laden, nicht nach einem Neuzeichnen (ui.render, baruch:navigate)"
    );
    assert_eq!(js.matches("store.set(COOKIE").count(), 1, "das Cookie setzt nur der Umschalter (ausdrückliche Wahl)");
    assert!(js.contains("toggleAttribute(\"data-ws-open\""), "nur das offene Satzbild trägt data-ws-open");
    assert!(js.contains("numberBodies"), "Rumpf-Blöcke einer Szene werden durchnummeriert");
    assert!(js.contains("pastedParagraphs"), "Einfügen mit Leerzeilen ergibt mehrere Absätze");
    assert!(!js.contains("./outline.js"), "wysiwyg.ts importiert nichts aus outline.ts");
    assert!(!script("outline.js").contains("./wysiwyg.js"), "outline.ts importiert nichts aus wysiwyg.ts");
}

#[test]
fn the_widgets_check_titles_like_the_server_and_never_pull_a_paragraph_into_a_heading() {
    // Round 7: the same messages as `baruch_store::check_title`; the server stays authoritative.
    for name in ["wysiwyg.js", "outline.js"] {
        let js = script(name);
        // Round 39a: the widgets name catalog keys; the German catalog text is the server's message.
        let module = name.trim_end_matches(".js");
        for (key, message) in [
            ("title_empty", "Ein Gliederungspunkt braucht einen Titel."),
            ("title_newline", "Der Titel darf keinen Zeilenumbruch enthalten."),
            ("title_long", "Der Titel ist zu lang (höchstens"),
        ] {
            let key = format!("widget.{module}.{key}");
            let text = baruch_server::i18n::t(baruch_server::i18n::Lang::De, &key, &[]);
            assert!(
                js.contains(&format!("\"{key}\"")) && text.starts_with(message),
                "{name} prüft vorab mit der Meldung des Servers: {message}"
            );
        }
    }
    let js = script("wysiwyg.js");
    assert!(js.contains("splitHeading"), "eine Überschrift endet am ersten Umbruch, der Rest wird ein Absatz");
    assert!(js.contains("cutAtBreak"), "auch ein Umbruch in emph/strong beendet die Überschrift (wie split_heading)");
    assert!(js.contains("guardHeadingMerge"), "Rückschritt/Entf ziehen keinen Absatz in die Überschrift");
    assert!(js.contains("headingProblem(state.sheet)"), "vor dem Übertragen ins Textfeld werden Titel geprüft");
}

#[tokio::test]
async fn a_figure_shown_as_an_image_is_saved_byte_for_byte() {
    // Round 7: the figure appears as an image (ws-figure); it stays stored character for character.
    let server = common::open().await;
    let work = server.import(BOOK, "werkstatt.typ").await.work;
    let loaded = server.rpc("work.get", json!({ "work": work })).await;
    let node = loaded["result"]["work"]["nodes"][0].clone();
    assert_eq!(node["title"], "Die Werkstatt", "das Kapitel: {node}");
    let key = node["key"].as_i64().unwrap();
    let body = node["revisions"][0]["body"].as_str().unwrap().to_owned();
    let parsed = call(&server, work, "wysiwyg.parse", json!({ "body": body })).await.unwrap();
    assert!(parsed["html"].as_str().unwrap().contains("ws-figure"), "die Abbildung als Bild: {parsed}");
    let count = parsed["blocks"].as_array().unwrap().len();

    let keep_all: Vec<Value> = (0..count).map(|i| json!({ "keep": i })).collect();
    let same = call(&server, work, "wysiwyg.serialize", json!({ "body": body, "edits": keep_all })).await.unwrap();
    assert_eq!(same["body"], body.as_str(), "ohne Änderung byte-gleich");

    // A new paragraph after the figure; the island goes along as keep.
    let mut edits: Vec<Value> = (0..count).map(|i| json!({ "keep": i })).collect();
    edits.push(
        json!({ "replaces": null, "block": { "kind": "paragraph", "inlines": [{ "t": "text", "v": "Späne." }] } }),
    );
    let changed = call(&server, work, "wysiwyg.serialize", json!({ "body": body, "edits": edits })).await.unwrap();
    let new_body = changed["body"].as_str().unwrap().to_owned();
    let figure = "#figure(image(\"/bilder/erfunden.svg\"), caption: [Ein erfundener Grundriss.])\n";
    assert!(new_body.contains(figure), "die Abbildung wörtlich: {new_body:?}");
    let saved = server.rpc("outline.save", json!({ "work": work, "key": key, "body": new_body })).await;
    assert!(saved.get("error").is_none(), "Speichern gelingt: {saved}");
    let export =
        baruch_typst::xref::strip_key_labels(&server.state.store.export_typst(work).await.unwrap()).into_owned();
    assert_eq!(export, BOOK.replacen(&body, &new_body, 1), "Export: nur der neue Absatz kommt dazu");
    assert!(export.contains(figure), "die Abbildung steht byte-gleich im Export");
}
