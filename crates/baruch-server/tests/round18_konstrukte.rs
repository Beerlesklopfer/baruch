// SPDX-License-Identifier: AGPL-3.0-or-later
//! Runde 18b (konstrukte): neue Konstrukte im Satzbild (Aufzählungen, Begriffslisten, geschachtelte Listen,
//! Umbruch, Links, Verweise), Inseln an Ort und Stelle (`{replaces, raw}`), mehrere Bilder in einem Block, Bildadresse
//! in Normalform, Teil-Block im Kapiteltext, Markdown bei `revision.update`, `guardHeadingMerge` über Vorschläge und
//! die Befehle `baruch:ws-command`/`baruch:ws-caret` im Widget. Abläufe über RPC so, wie das Widget sie schickt (SKILLS
//! „Fehler aus der Arbeit nachstellen“); was nur im Browser geschieht, prüft der Test an den Stellen im übersetzten
//! Widget und sagt das. Alle Texte frei erfunden, kein Netz, kein Modell.

use crate::common;

use serde_json::{Value, json};

fn ok(answer: &Value, what: &str) -> Value {
    assert!(answer["error"].is_null(), "{what} sollte gelingen: {answer}");
    answer["result"].clone()
}

fn error(answer: &Value, what: &str) -> (i64, String) {
    assert!(answer["result"].is_null(), "{what} sollte scheitern: {answer}");
    (answer["error"]["code"].as_i64().unwrap(), answer["error"]["message"].as_str().unwrap_or("").to_owned())
}

fn script(name: &str) -> &'static str {
    baruch_server::SCRIPTS.iter().find(|(n, _)| *n == name).map(|(_, js)| *js).expect("übersetztes Widget")
}

/// Aktueller Text eines Abschnitts (über den Schlüssel).
async fn body_of(server: &common::Server, work: i64, key: i64) -> String {
    let loaded = server.state.store.load_work(work).await.unwrap();
    let node = loaded.nodes.iter().find(|n| n.key == key).unwrap();
    node.revisions[node.current_index().unwrap()].body.clone()
}

async fn keys_of(server: &common::Server, work: i64) -> Vec<(String, i64)> {
    server.state.store.load_work(work).await.unwrap().nodes.iter().map(|n| (n.title.clone(), n.key)).collect()
}

const MARKT: &str = "#set document(title: \"Der Markttag\", author: \"Ida Feld\")\n#set heading(numbering: \"1.\")\n\n\
= Markttag\n\nAm Morgen öffnen die Stände.\n\n\
1. Wasser holen\n2. Feuer machen\n\n\
/ Krug: Gefäß aus Ton\n/ Elle: altes Maß\n\n\
- Obst\n  - Äpfel\n  - Birnen\n- Gemüse\n\n\
Erste Zeile \\\nzweite Zeile, siehe #link(\"https://example.org/markt\")[Marktordnung] und @stand.\n\n\
#let preis = 3\n\n\
= Abend\n\nDie Stände schließen.\n";

#[tokio::test]
async fn the_new_constructs_are_shown_and_saved_byte_equal_or_canonical() {
    let server = common::open().await;
    let work = server.import(MARKT, "markt.typ").await.work;
    let keys = keys_of(&server, work).await;
    let key = keys.iter().find(|(t, _)| t == "Markttag").unwrap().1;
    let body = body_of(&server, work, key).await;
    let parsed = ok(&server.rpc("wysiwyg.parse", json!({ "work": work, "body": body })).await, "wysiwyg.parse");
    let kinds: Vec<&str> = parsed["blocks"].as_array().unwrap().iter().map(|b| b["kind"].as_str().unwrap()).collect();
    assert_eq!(
        kinds,
        ["heading", "paragraph", "enum", "terms", "list", "paragraph", "island"],
        "Aufzählung, Begriffsliste und geschachtelte Liste sind Blöcke, nur #let bleibt Insel: {parsed}"
    );
    assert_eq!(parsed["blocks"][4]["depths"], json!([0, 1, 1, 0]), "Tiefen der geschachtelten Liste im JSON");
    let inlines = &parsed["blocks"][5]["inlines"];
    let ts: Vec<&str> = inlines.as_array().unwrap().iter().map(|i| i["t"].as_str().unwrap()).collect();
    assert!(
        ts.contains(&"linebreak") && ts.contains(&"link") && ts.contains(&"ref"),
        "Umbruch, Link, Verweis: {inlines}"
    );
    let html = parsed["html"].as_str().unwrap();
    assert!(
        html.contains("<ol start=\"1\">") && html.contains("<dl><dt>Krug</dt>"),
        "Aufzählung und Begriffsliste: {html}"
    );
    assert!(html.contains("class=\"ws-ref\"") && html.contains("type=\"button\""), "Verweis als Schaltfläche: {html}");

    let count = parsed["blocks"].as_array().unwrap().len();
    let keep: Vec<Value> = (0..count).map(|i| json!({ "keep": i })).collect();
    let same = ok(
        &server.rpc("wysiwyg.serialize", json!({ "work": work, "body": body, "edits": keep })).await,
        "wysiwyg.serialize",
    );
    assert_eq!(same["body"], body, "lauter keep: byte-gleich");

    // Wie das Widget: die Aufzählung bekommt einen Punkt dazu (data-dirty), der Rest ist keep.
    let mut edits: Vec<Value> = (0..count).map(|i| json!({ "keep": i })).collect();
    edits[2] = json!({ "replaces": 2, "block": { "kind": "enum", "start": 1, "items": [
        [{ "t": "text", "v": "Wasser holen" }], [{ "t": "text", "v": "Feuer machen" }], [{ "t": "text", "v": "Brot backen" }]
    ] } });
    let text = ok(
        &server.rpc("wysiwyg.serialize", json!({ "work": work, "body": body, "edits": edits })).await,
        "wysiwyg.serialize",
    )["body"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(
        text.contains("1. Wasser holen\n2. Feuer machen\n3. Brot backen\n\n/ Krug"),
        "kanonisch fortlaufend: {text}"
    );
    assert_eq!(text.replace("3. Brot backen\n", ""), body, "sonst byte-gleich");
    let saved = server.rpc("outline.save", json!({ "work": work, "key": key, "body": text })).await;
    ok(&saved, "outline.save");
    assert_eq!(body_of(&server, work, key).await, text, "gespeichert, wie das Satzbild es schickte");
}

#[tokio::test]
async fn an_island_is_edited_in_place_and_invalid_raw_edits_are_refused() {
    let server = common::open().await;
    let work = server.import(MARKT, "markt.typ").await.work;
    let key = keys_of(&server, work).await.into_iter().find(|(t, _)| t == "Markttag").unwrap().1;
    let body = body_of(&server, work, key).await;
    let parsed = ok(&server.rpc("wysiwyg.parse", json!({ "work": work, "body": body })).await, "wysiwyg.parse");
    let blocks = parsed["blocks"].as_array().unwrap();
    let island = blocks.iter().position(|b| b["kind"] == "island").expect("die #let-Zeile ist eine Insel");
    let mut edits: Vec<Value> = (0..blocks.len()).map(|i| json!({ "keep": i })).collect();
    edits[island] = json!({ "replaces": island, "raw": "#let preis = 4\n" });
    let text = ok(
        &server.rpc("wysiwyg.serialize", json!({ "work": work, "body": body, "edits": edits })).await,
        "Insel an Ort und Stelle",
    )["body"]
        .as_str()
        .unwrap()
        .to_owned();
    assert_eq!(text, body.replace("#let preis = 3", "#let preis = 4"), "nur die Insel ändert sich, wörtlich");

    let (code, message) = error(
        &server
            .rpc("wysiwyg.serialize", json!({ "work": work, "body": body, "edits": [{ "replaces": 99, "raw": "x" }] }))
            .await,
        "unbekannter Index",
    );
    assert_eq!(code, -32602, "ungültiger Index → INVALID_PARAMS: {message}");
    assert!(message.contains("Block 99 gibt es nicht"), "deutsche Meldung: {message}");
    let (code, message) = error(
        &server.rpc("wysiwyg.serialize", json!({ "work": work, "body": body, "edits": [{ "raw": "#neu()" }] })).await,
        "neue Insel",
    );
    assert_eq!(code, -32602, "neue Inseln anlegen bleibt verboten: {message}");
    assert!(message.contains("Typst-Insel lässt sich im Satzbild nicht anlegen"), "NewIsland: {message}");
}

/// Erfundenes Bild zum Pfad anlegen.
async fn add_image(server: &common::Server, work: i64, file_name: &str, path: &str) {
    let svg =
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"4\" height=\"4\"><rect width=\"4\" height=\"4\"/></svg>\n";
    let answer = server
        .rpc(
            "asset.add",
            json!({ "work": work, "file_name": file_name, "content": svg, "encoding": "svg", "path": path }),
        )
        .await;
    ok(&answer, "Bild angelegt");
}

#[tokio::test]
async fn a_block_with_two_images_shows_both_and_the_address_has_no_leading_slash() {
    let server = common::open().await;
    let work = server.import("= Galerie\n\nText.\n", "galerie.typ").await.work;
    add_image(&server, work, "links.svg", "/bilder/links.svg").await;
    add_image(&server, work, "rechts.svg", "bilder/rechts.svg").await;
    let body = "= Galerie\n\n#grid(columns: 2, image(\"/bilder/links.svg\"), image(\"./bilder/rechts.svg\"))\n";
    let parsed = ok(&server.rpc("wysiwyg.parse", json!({ "work": work, "body": body })).await, "wysiwyg.parse");
    let html = parsed["html"].as_str().unwrap();
    assert_eq!(
        html.matches("<img class=\"ws-image\"").count(),
        2,
        "beide Bilder des Blocks (vorher nur das erste): {html}"
    );
    assert!(
        html.contains(&format!("src=\"/werke/{work}/bilder/pfad?p=bilder%2Flinks.svg\"")),
        "Adresse in Normalform ohne führendes / (vorher p=%2Fbilder%2F…): {html}"
    );
    assert!(html.contains(&format!("src=\"/werke/{work}/bilder/pfad?p=bilder%2Frechts.svg\"")), "./ entfällt: {html}");
    let (status, svg) = server.get(&format!("/werke/{work}/bilder/pfad?p=bilder%2Flinks.svg")).await;
    assert_eq!(status, axum::http::StatusCode::OK, "die Adresse ohne / liefert das mit / angelegte Bild");
    assert!(svg.starts_with("<svg"), "das Bild selbst: {svg}");
}

#[tokio::test]
async fn revision_update_converts_markdown_headings_like_revision_save() {
    // Backlog „Nach Runde 14 offen“ (Nachgang Markdown-Überschriften): revision.save wandelte `# Titel` um,
    // revision.update (Fassung ändern) nicht – dort blieb die Raute als Text stehen.
    let server = common::open().await;
    let work = server.import("= Hafen\n\nDie Fähre.\n", "hafen.typ").await.work;
    let key = keys_of(&server, work).await[0].1;
    let saved = ok(
        &server
            .rpc("revision.save", json!({ "work": work, "key": key, "body": "= Hafen\n\nDie Fähre.\n\nNotiz.\n" }))
            .await,
        "revision.save",
    );
    let rev = saved["rev"].as_u64().unwrap() as u32;
    let body = "# Hafen\n\nDie Fähre legt an.\n\nNotiz.\n";
    let updated = ok(
        &server.rpc("revision.update", json!({ "work": work, "key": key, "rev": rev, "body": body })).await,
        "revision.update",
    );
    assert_eq!(updated["changed"], true, "die Fassung ist geändert: {updated}");
    let loaded = server.state.store.load_work(work).await.unwrap();
    let text = loaded.nodes.iter().find(|n| n.key == key).unwrap().revision(rev).unwrap().body.clone();
    assert_eq!(text, "= Hafen\n\nDie Fähre legt an.\n\nNotiz.\n", "`# Hafen` wird `= Hafen` wie bei revision.save");
    // Eine weitere Markdown-Überschrift wird wie eine getippte `==`-Zeile behandelt: Fassung ändern stellt die
    // Gliederung nicht um und lehnt ehrlich ab (vorher blieb `## Der Steg` still als Text stehen).
    let more = "= Hafen\n\nDie Fähre legt an.\n\n## Der Steg\nNotiz.\n";
    let refused = server.rpc("revision.update", json!({ "work": work, "key": key, "rev": rev, "body": more })).await;
    let message = refused["error"]["message"].as_str().unwrap_or("");
    assert!(message.contains("„Der Steg“"), "abgelehnt wie `== Der Steg`, mit dem Titel in der Meldung: {refused}");
}

#[tokio::test]
async fn a_part_block_in_a_chapter_text_is_refused_in_the_satzbild_and_needs_confirmation_in_outline_save() {
    // Backlog „Nach Runde 14 offen“: Ein Client konnte einen Teil-Block in einen Kapiteltext schreiben. Festgelegt
    // (Runde 18b): Das Satzbild bietet Teile nicht an, und wysiwyg.serialize lehnt einen neuen Teil-Block ab.
    let server = common::open().await;
    let work = server.import(MARKT, "markt.typ").await.work;
    let keys = keys_of(&server, work).await;
    let key = keys.iter().find(|(t, _)| t == "Abend").unwrap().1;
    let body = body_of(&server, work, key).await;
    let edits = json!([{ "keep": 0 }, { "replaces": null, "block": { "kind": "part", "title": "Zweiter Teil" } }, { "keep": 1 }]);
    let (code, message) = error(
        &server.rpc("wysiwyg.serialize", json!({ "work": work, "body": body, "edits": edits })).await,
        "Teil-Block",
    );
    assert_eq!(code, -32602, "INVALID_PARAMS: {message}");
    assert_eq!(message, "Einen Teil legt man in der Gliederung an, nicht im Satzbild.", "Meldung nennt den Weg");

    // Im Quelltext (Mensch) als weitere Überschrift: outline.check verlangt die Rückfrage zum Teilen; ohne
    // Bestätigung wird nichts gespeichert, mit Bestätigung entsteht der Teil (Aktion des Menschen).
    let text = format!(
        "{}\n#heading(level: 1, numbering: none)[Zweiter Teil] <baruch-part>\n\nNach dem Teil.\n",
        body.trim_end()
    );
    let text = text.replacen("Die Stände schließen.\n#heading", "Die Stände schließen.\n\n#heading", 1);
    let check =
        ok(&server.rpc("outline.check", json!({ "work": work, "key": key, "body": text })).await, "outline.check");
    let split = check["split"].as_array().cloned().unwrap_or_default();
    assert_eq!(split.len(), 1, "die Teil-Markierung ist ein neuer Gliederungspunkt, der Rückfrage braucht: {check}");
    let refused = server.rpc("outline.save", json!({ "work": work, "key": key, "body": text })).await;
    assert!(refused["error"].is_object(), "ohne Bestätigung (split) nichts gespeichert: {refused}");
    assert_eq!(body_of(&server, work, key).await, body, "Text unverändert");
    let saved = server.rpc("outline.save", json!({ "work": work, "key": key, "body": text, "split": true })).await;
    ok(&saved, "outline.save mit Bestätigung");
    let loaded = server.state.store.load_work(work).await.unwrap();
    let part = loaded.nodes.iter().find(|n| n.title == "Zweiter Teil").expect("der Teil ist angelegt");
    assert_eq!(part.kind, baruch_core::NodeKind::Part, "als Teil (Ebene 0), nicht als Kapitel: {:?}", part.kind);
}

#[tokio::test]
async fn a_proposal_between_heading_and_paragraph_does_not_disarm_the_merge_guard() {
    // Gutachten R8 (Backlog): Stand ein Vorschlag (`details.pv`) zwischen Überschrift und Absatz, griff
    // `guardHeadingMerge` nicht (Nachbar war das details), und Rückschritt zog den Absatz in die Überschrift.
    let server = common::open().await;
    let work = server.import(MARKT, "markt.typ").await.work;
    let key = keys_of(&server, work).await.into_iter().find(|(t, _)| t == "Abend").unwrap().1;
    let body = body_of(&server, work, key).await;
    // Ein Vorschlag, der vor dem Absatz (Block 1) einfügt: das Widget setzt sein Stück vor [data-b=1], also genau
    // zwischen Überschrift (0) und Absatz (1).
    let proposal = body.replace("Die Stände schließen.", "Ein Händler zählt.\n\nDie Stände schließen.");
    server.state.store.add_proposal(work, 1, &proposal, "@lektor", "erfunden", "Satz davor").await.unwrap();
    let (_, page) = server.get(&format!("/?work={work}&key={key}")).await;
    assert!(
        page.contains("data-pv-gap=\"1\""),
        "das Stück steht in der Lücke vor Block 1 (zwischen Überschrift und Absatz)"
    );
    // Was ohne Schutz geschähe (Nachbau über RPC): der Absatz in der Überschrift wird ihr Titel.
    let merged = json!([{ "replaces": 0, "block": { "kind": "heading", "level": 1, "inlines": [
        { "t": "text", "v": "AbendDie Stände schließen." }] } }]);
    let text = ok(
        &server.rpc("wysiwyg.serialize", json!({ "work": work, "body": body, "edits": merged })).await,
        "Nachbau",
    )["body"]
        .as_str()
        .unwrap()
        .to_owned();
    let check =
        ok(&server.rpc("outline.check", json!({ "work": work, "key": key, "body": text })).await, "outline.check");
    assert!(check.to_string().contains("AbendDie Stände schließen."), "verschmolzen würde der Absatz Titel: {check}");
    // Der Schutz im Widget überspringt jetzt Vorschläge beim Nachbarn (nur im Browser wirksam; geprüft an der Stelle).
    let js = script("wysiwyg.js");
    assert!(js.contains("function blockNeighbour"), "eigene Nachbarsuche im Widget");
    assert!(
        js.contains("while (node && isProposal(node))"),
        "die Nachbarsuche überspringt details.pv, guardHeadingMerge nutzt sie"
    );
    assert!(
        js.contains("const neighbour = blockNeighbour(block, backward);"),
        "guardHeadingMerge fragt die Nachbarsuche"
    );
}

#[tokio::test]
async fn the_caret_offset_for_images_comes_from_the_same_edits_as_the_full_text() {
    // Ablauf von `baruch:ws-caret` über RPC: wysiwyg.ts serialisiert alle Blöcke (Text) und die Blöcke bis zum
    // Block am Cursor (Präfix); die Stelle ist die Länge des Präfixes – ein Zeilenanfang im Text.
    let server = common::open().await;
    let work = server.import(MARKT, "markt.typ").await.work;
    let key = keys_of(&server, work).await.into_iter().find(|(t, _)| t == "Markttag").unwrap().1;
    let body = body_of(&server, work, key).await;
    let parsed = ok(&server.rpc("wysiwyg.parse", json!({ "work": work, "body": body })).await, "wysiwyg.parse");
    let count = parsed["blocks"].as_array().unwrap().len();
    // Ungespeichert: im Absatz (Block 1) getippt; Cursor in der Aufzählung (Block 2).
    let mut edits: Vec<Value> = (0..count).map(|i| json!({ "keep": i })).collect();
    edits[1] = json!({ "replaces": 1, "block": { "kind": "paragraph", "inlines": [{ "t": "text", "v": "Am Morgen öffnen die Stände früh." }] } });
    let full = ok(
        &server.rpc("wysiwyg.serialize", json!({ "work": work, "body": body, "edits": edits })).await,
        "Text",
    )["body"]
        .as_str()
        .unwrap()
        .to_owned();
    let prefix = ok(
        &server.rpc("wysiwyg.serialize", json!({ "work": work, "body": body, "edits": edits[..3] })).await,
        "Präfix",
    )["body"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(full.starts_with(&prefix), "das Präfix ist der Anfang des Texts (Anweisungen in derselben Reihenfolge)");
    assert!(prefix.ends_with('\n'), "die Stelle ist ein Zeilenanfang: {prefix:?}");
    let offset = prefix.encode_utf16().count();
    let at: String = full.encode_utf16().take(offset).map(|_| ' ').collect();
    assert_eq!(at.len(), offset, "UTF-16-Stelle, wie JavaScript zählt");
    let rest = &full[prefix.len()..];
    assert!(rest.starts_with("/ Krug"), "dahinter beginnt der nächste Block (die Begriffsliste): {rest:?}");
    let js = script("wysiwyg.js");
    assert!(js.contains("baruch:ws-caret") || js.contains("WS_CARET"), "das Widget hört auf baruch:ws-caret");
    assert!(
        js.contains("editsOf(state.sheet, false, block)"),
        "Präfix aus denselben Anweisungen bis zum Block am Cursor"
    );
}

#[tokio::test]
async fn the_widget_handles_every_ribbon_command_and_adopts_the_server_style() {
    // Nur im Browser wirksam (Firefox läuft hier nicht): geprüft an den Stellen im übersetzten Widget.
    let js = script("wysiwyg.js");
    let frozen = script("ws_command.js");
    assert!(frozen.contains("export const WS_COMMAND = \"baruch:ws-command\""), "Vertragsdatei unverändert übersetzt");
    assert!(js.contains("from \"./ws_command.js\""), "wysiwyg.ts nutzt die eingefrorene Vertragsdatei");
    for cmd in ["scene", "work", "footnote", "quote", "list", "enum", "terms", "break", "link"] {
        assert!(js.contains(&format!("case \"{cmd}\":")), "Befehl {cmd} hat einen Zweig in runCommand");
    }
    assert!(js.contains("detail.handled = true;"), "handled nur bei offenem Satzbild (sonst bleibt es false)");
    assert!(js.contains("if (!detail || !state.open"), "geschlossenes Satzbild: Befehl bleibt unerledigt");
    assert!(
        js.contains("style#ws-style[data-work=\"${work}\"]"),
        "ein vom Server mitgeliefertes #ws-style mit gleichem data-work wird übernommen"
    );
    let adopt = js.find("if (own)").expect("Übernahme-Zweig");
    let rpc = js.find("call(\"wysiwyg.style\"").expect("RPC-Zweig");
    assert!(adopt < rpc, "erst übernehmen, nur ohne Element über RPC holen");
}
