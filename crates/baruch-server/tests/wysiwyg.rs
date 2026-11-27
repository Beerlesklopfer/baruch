// SPDX-License-Identifier: AGPL-3.0-or-later
//! Typeset view editor in the server: RPC `wysiwyg.*` (directly through `baruch_server::wysiwyg::call`; the entry under
//! `/rpc` is wired by the integrator), editor page without JavaScript and round trip across all revisions of the sample
//! work.

use crate::common;

use axum::http::StatusCode;
use baruch_server::rpc::{INVALID_PARAMS, METHOD_NOT_FOUND};
use baruch_server::wysiwyg;
use baruch_wysiwyg::{Edit, parse, serialize};
use serde_json::{Value, json};

/// Freely invented work with its own template (font, colour, box made up).
const BOOK: &str = r##"#set document(title: "Der Brunnenbauer", author: "Niemand")
#set text(font: "Erfundene Antiqua", size: 10pt, fill: rgb("#223344"))
#set par(first-line-indent: 1em)
#let szene(body) = block(fill: luma(245), inset: 6pt)[#body]

= Am Brunnen

Mira schöpft _frisches_ Wasser.

#szene[Ein Hund läuft vorbei.]
"##;

async fn call(server: &common::Server, work: i64, method: &str, params: Value) -> Result<Value, (i64, String)> {
    wysiwyg::call(&server.state, work, method, params).await.map_err(|e| (e.code, e.message))
}

#[test]
fn the_module_answers_exactly_its_four_methods() {
    for method in ["wysiwyg.parse", "wysiwyg.serialize", "wysiwyg.style", "wysiwyg.diff"] {
        assert!(wysiwyg::handles(method), "{method} gehört dem Satzbild");
    }
    for method in ["wysiwyg.save", "outline.save", "revision.update", "wysiwyg"] {
        assert!(!wysiwyg::handles(method), "{method} gehört nicht dazu (nur lesende Methoden)");
    }
}

#[tokio::test]
async fn parse_returns_blocks_and_html() {
    let server = common::open().await;
    let work = server.import(BOOK, "brunnen.typ").await.work;
    let body = "= Am Brunnen\n\nMira schöpft _frisches_ Wasser.\n\n#let zahl = 3\n";
    let answer = call(&server, work, "wysiwyg.parse", json!({ "body": body })).await.unwrap();
    let blocks = answer["blocks"].as_array().unwrap();
    assert_eq!(blocks.len(), 3, "Überschrift, Absatz, Insel: {answer}");
    assert_eq!(blocks[0]["kind"], "heading", "Überschrift erkannt");
    assert_eq!(blocks[2]["kind"], "island", "#let ist eine Insel");
    let joined: String = blocks.iter().map(|b| b["raw"].as_str().unwrap()).collect();
    assert_eq!(joined, body, "die raw ergeben den Text byte-gleich");
    let html = answer["html"].as_str().unwrap();
    assert!(html.contains("data-b=\"1\" data-kind=\"paragraph\""), "Block mit Index: {html}");
    assert!(html.contains("<em>frisches</em>"), "Hervorhebung als em: {html}");
    assert!(html.contains("contenteditable=\"false\""), "Insel nicht bearbeitbar: {html}");
    let missing = call(&server, work, "wysiwyg.parse", json!({})).await;
    assert_eq!(missing.map_err(|e| e.0), Err(INVALID_PARAMS), "ohne body → -32602");
}

#[tokio::test]
async fn serialize_keeps_untouched_blocks_and_refuses_nonsense() {
    let server = common::open().await;
    let work = default_work(&server).await;
    let body = "= Titel\n\nErster   Absatz.\n\nZweiter Absatz.\n";
    let edits = json!([
        { "keep": 0 },
        { "keep": 1 },
        { "replaces": 2, "block": { "kind": "paragraph", "inlines": [{ "t": "text", "v": "Neu #1" }] } }
    ]);
    let answer = call(&server, work, "wysiwyg.serialize", json!({ "body": body, "edits": edits })).await.unwrap();
    assert_eq!(
        answer["body"], "= Titel\n\nErster   Absatz.\n\nNeu \\#1\n",
        "unberührt wörtlich, der bearbeitete Block maskiert neu erzeugt"
    );
    for (edits, why) in [
        (json!([{ "keep": 7 }]), "unbekannter Index"),
        (json!([{ "keep": 0 }, { "keep": 0 }]), "doppelter Index"),
        (json!([{ "replaces": null, "block": { "kind": "island" } }]), "neue Insel"),
        (json!([{ "unsinn": true }]), "keine Anweisung"),
        (json!("kein Array"), "falscher Typ"),
    ] {
        let refused = call(&server, work, "wysiwyg.serialize", json!({ "body": body, "edits": edits })).await;
        assert_eq!(refused.as_ref().map_err(|e| e.0), Err(INVALID_PARAMS), "{why} → -32602: {refused:?}");
    }
    let unknown = call(&server, work, "wysiwyg.save", json!({})).await;
    assert_eq!(unknown.map_err(|e| e.0), Err(METHOD_NOT_FOUND), "unbekannte Methode");
}

#[tokio::test]
async fn style_comes_from_the_work_template() {
    let server = common::open().await;
    let work = server.import(BOOK, "brunnen.typ").await.work;
    let answer = call(&server, work, "wysiwyg.style", json!({})).await.unwrap();
    assert_eq!(answer["style"]["font"], "\"Erfundene Antiqua\"", "Schrift aus #set text: {answer}");
    assert_eq!(answer["style"]["size"], "10pt", "Größe: {answer}");
    assert_eq!(answer["style"]["fill"], "#223344", "Farbe: {answer}");
    assert_eq!(answer["style"]["indent"], "1em", "Einzug: {answer}");
    assert_eq!(answer["style"]["scene"]["fill"], "rgb(245, 245, 245)", "Kasten von #szene: {answer}");
    let css = answer["css"].as_str().unwrap();
    assert!(css.contains("--ws-font: \"Erfundene Antiqua\";"), "CSS-Variable gesetzt: {css}");
    let sample = default_work(&server).await;
    let plain = call(&server, sample, "wysiwyg.style", json!({})).await.unwrap();
    assert!(plain["style"]["font"].is_null(), "Beispielwerk ohne Schriftregel: Vorgabe aus wysiwyg.css: {plain}");
    let unknown = call(&server, 999, "wysiwyg.style", json!({})).await;
    assert_eq!(unknown.map_err(|e| e.0), Err(INVALID_PARAMS), "unbekanntes Werk");
}

#[tokio::test]
async fn diff_compares_a_proposal_with_its_parent() {
    let server = common::open().await;
    let work = default_work(&server).await;
    // Sample work: the introduction (position 2) has revision 3 as a proposal.
    let tree = server.rpc("revision.tree", json!({ "work": work, "node": 2 })).await;
    let revisions = tree["result"]["revisions"].as_array().unwrap();
    let proposal = revisions.iter().find(|r| r["status"] == "proposed").expect("Vorschlag im Beispielwerk");
    let rev = proposal["number"].as_u64().unwrap();
    let parent = proposal["based_on"].as_u64().expect("der Vorschlag hat ein Elternteil");
    let answer = call(&server, work, "wysiwyg.diff", json!({ "node": 2, "rev": rev })).await.unwrap();
    assert_eq!(answer["base"], parent, "ohne base gilt based_on des Vorschlags: {answer}");
    let html = answer["html"].as_str().unwrap();
    assert!(html.contains("class=\"ws-ins\""), "neuer Text markiert: {html}");
    assert!(html.contains("class=\"ws-del\""), "alter Text markiert: {html}");
    assert!(html.contains("class=\"ws-same\""), "die Überschrift ist gleich: {html}");
    let first = call(&server, work, "wysiwyg.diff", json!({ "node": 2, "rev": rev, "base": 1 })).await.unwrap();
    assert_eq!(first["base"], 1, "base lässt sich wählen");
    let node = server.rpc("node.get", json!({ "work": work, "node": 2 })).await;
    let key = node["result"]["key"].as_i64().unwrap();
    let by_key = call(&server, work, "wysiwyg.diff", json!({ "key": key, "node": 0, "rev": rev })).await.unwrap();
    assert_eq!(by_key["html"], answer["html"], "der Schlüssel geht vor der Stelle");
    let gone = call(&server, work, "wysiwyg.diff", json!({ "node": 2, "rev": 99 })).await;
    assert_eq!(gone.map_err(|e| e.0), Err(INVALID_PARAMS), "unbekannte Fassung");
}

#[tokio::test]
async fn the_editor_works_without_javascript_and_carries_the_satzbild_hidden() {
    let server = common::open().await;
    let work = default_work(&server).await;
    let (status, html) = server.get(&format!("/?work={work}&node=2&rev=3")).await;
    assert_eq!(status, StatusCode::OK, "Seite mit Vorschlag");
    assert!(
        html.contains("<form class=\"editor-form\" id=\"editor-form\" method=\"post\" action=\"/gliederung/act\" data-rpc data-widget=\"editor\" data-outline-save>"),
        "das Editor-Formular bleibt unverändert und geht ohne JavaScript an /gliederung/act: {html}"
    );
    assert!(html.contains("<textarea name=\"body\""), "das Textfeld ist da (Quelle beim Absenden): {html}");
    for field in [
        "method\" value=\"outline.save\"",
        "name=\"work\"",
        "name=\"node\"",
        "name=\"key\"",
        "name=\"based_on\" value=\"3\"",
    ] {
        assert!(html.contains(field), "verstecktes Feld {field}: {html}");
    }
    let start = html.find("data-widget=\"wysiwyg\"").expect("Satzbild-Bereich vorhanden");
    let tag = &html[html[..start].rfind('<').unwrap()..start + html[start..].find('>').unwrap()];
    assert!(tag.starts_with("<div class=\"wysiwyg\""), "Bereich ist ein div.wysiwyg: {tag}");
    assert!(!tag.contains(" hidden"), "der Bereich umfasst auch Textfeld und Statuszeile und bleibt sichtbar: {tag}");
    assert!(html.contains("data-edit-toolbar hidden>"), "ohne JavaScript ist die Werkzeugleiste verborgen");
    for attribute in
        [format!("data-work=\"{work}\""), "data-node=\"2\"".into(), "data-rev=\"3\"".into(), "data-key=".into()]
    {
        assert!(tag.contains(&attribute), "{attribute} am Bereich: {tag}");
    }
    let form = html.find("<form class=\"editor-form\"").unwrap();
    let area = form + html[form..].find("<textarea name=\"body\"").expect("Textfeld im Editor-Formular");
    let wrap = html.find("<div class=\"ws-wrap\" data-ws-wrap hidden>").expect("Satzbild ohne JavaScript verborgen");
    assert!(start < form && form < wrap && wrap < area, "das Satzbild steht im Formular vor dem Textfeld");
    assert!(!html[form..area].contains("</form>"), "zwischen Formularbeginn und Textfeld endet das Formular nicht");
    let save = html.find("form=\"editor-form\" data-saved=\"Gespeichert\"").expect("Speichern in der Statuszeile");
    assert!(area < save, "„Speichern“ steht in der Statuszeile unter dem Blatt und gehört über form= zum Formular");
    // Round 7: the proposal stands as a variant in the reading flow; the typeset view diff comes from the server
    // (pv-satz, visible via CSS only with the typeset view open), the line diff stays the fallback.
    assert!(html.contains("<pre class=\"pv-quelltext diff\">"), "der Quelltext-Diff bleibt als Rückfall: {html}");
    assert!(
        html.contains("<div class=\"pv-satz ws-sheet\"><div class=\"ws-diff-doc\" data-kind=\"changed\">"),
        "Satzbild des Stücks fertig gezeichnet im Vorschlag (Runde 8: mit Art): {html}"
    );
    assert!(html.contains("Unterschied zu Fassung 2 (aktuell)"), "Diff gegen die Basis 2: {html}");
    assert!(!html.contains("data-widget=\"wysiwyg-diff\""), "das alte Diff-Widget ist aus editor.html entfernt");
    assert!(
        html.contains("data-ws-mode=\"satz\"") && html.contains("data-ws-mode=\"quelltext\""),
        "Umschalter Satzbild | Quelltext"
    );
    let (_, plain) = server.get(&format!("/?work={work}&node=2&rev=2")).await;
    assert!(
        plain.contains("data-pv=\"") && plain.contains("<details class=\"pv pc\""),
        "auch bei der aktuellen Fassung stehen die offenen Vorschläge im Lesefluss: {plain}"
    );
    assert!(!plain.contains("pv-shown"), "gezeigt ist hier kein Vorschlag: {plain}");
    let (_, preview) = server.get(&format!("/?work={work}&node=2&view=preview")).await;
    assert!(!preview.contains("data-widget=\"wysiwyg\""), "die Vorschau hat keinen Editor");
    let (_, css) = server.get("/static/app.css").await;
    assert!(css.contains(".ws-sheet"), "wysiwyg.css steckt im Stil");
    let script = baruch_server::SCRIPTS.iter().find(|(name, _)| *name == "wysiwyg.js");
    assert!(script.is_some_and(|(_, body)| body.contains("baruch:before-save")), "Widget eingebettet");
    let main = baruch_server::SCRIPTS.iter().find(|(name, _)| *name == "main.js").unwrap().1;
    assert!(main.contains("./wysiwyg.js"), "main.js lädt das Widget");
}

/// Concatenating the raw and `serialize` with nothing but Keep yield the text.
fn assert_round_trip(body: &str, origin: &str) {
    let doc = parse(body);
    let joined: String = doc.blocks.iter().map(|b| b.raw.as_str()).collect();
    assert_eq!(joined, body, "{origin}: Verkettung der raw");
    let keep: Vec<Edit> = (0..doc.blocks.len()).map(|keep| Edit::Keep { keep }).collect();
    assert_eq!(serialize(&doc, &keep).unwrap(), body, "{origin}: serialize mit lauter Keep");
}

#[tokio::test]
async fn every_revision_of_the_sample_work_comes_back_byte_for_byte() {
    // Sample data in the source …
    let sample = baruch_core::sample::work();
    let mut count = 0;
    for node in &sample.nodes {
        for rev in &node.revisions {
            assert_round_trip(&rev.body, &format!("Beispiel {} Fassung {}", node.title, rev.number));
            count += 1;
        }
    }
    assert!(count > sample.nodes.len(), "auch ältere Fassungen und Vorschläge geprüft ({count})");
    // … and as they are in the database (seeded), including work template and export.
    let server = common::open().await;
    let work = default_work(&server).await;
    let loaded = server.rpc("work.get", json!({ "work": work })).await;
    for node in loaded["result"]["work"]["nodes"].as_array().unwrap() {
        for rev in node["revisions"].as_array().unwrap() {
            assert_round_trip(
                rev["body"].as_str().unwrap(),
                &format!("gesät {} Fassung {}", node["title"], rev["number"]),
            );
        }
    }
    let export = server.rpc("work.export", json!({ "work": work })).await;
    assert_round_trip(export["result"]["typst"].as_str().unwrap(), "Export des Beispielwerks");
    let garden = server.import_garden().await.work;
    let export = server.rpc("work.export", json!({ "work": garden })).await;
    assert_round_trip(export["result"]["typst"].as_str().unwrap(), "Export des Gartenbuchs");
}

/// Default work (the seeded sample work).
async fn default_work(server: &common::Server) -> i64 {
    server.state.default_work().await.unwrap()
}
