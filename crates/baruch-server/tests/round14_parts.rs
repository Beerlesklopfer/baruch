// SPDX-License-Identifier: AGPL-3.0-or-later
//! Runde 14 (R14a, Vertrag Abschnitte 1 und 5): Teil-Markierungen im Server – Import und Export,
//! Satzbild (`wysiwyg.*`, vorgerendertes Satzbild im Editor), Teil-Seite im Satz (`typeset::prepare`,
//! gleiche Quelle für Satz-Ansicht und `work.pdf`). Werke frei erfunden; jedes Werk hat einen eigenen
//! Titel und Text, also einen eigenen Stand im statischen Zwischenspeicher der Vorschau.

use crate::common;

use baruch_core::NodeKind;
use serde_json::{Value, json};

fn result(answer: &Value) -> &Value {
    assert!(answer["error"].is_null(), "RPC sollte gelingen: {answer}");
    &answer["result"]
}

/// Werk mit Werkvorlage, Vorwort ohne Nummer, zwei Teilen und drei Kapiteln (frei erfunden).
fn book(title: &str) -> String {
    format!(
        "#set document(title: \"{title}\", author: \"Erna Erfunden\")\n#set page(width: 10cm, height: 14cm)\n\
         #set heading(numbering: \"1.1\")\n\n\
         #heading(level: 1, numbering: none)[Vorwort] <baruch-unnumbered>\nEin Wort vorab.\n\n\
         #heading(level: 1, numbering: none)[Vom Pflanzen] <baruch-part> // erster Teil\n\n\
         = Die Saat\n#context assert.eq(counter(heading).get(), (1,))\nKörner in die Erde.\n\n\
         = Das Gießen\nWasser am Abend.\n\n\
         #heading(level: 1, numbering: none)[Vom Ernten] <baruch-part>\n\n\
         = Der Korb\n#context assert.eq(counter(heading).get(), (3,), message: \"Kapitel laufen über Teile fort\")\n\
         Voll bis zum Rand.\n"
    )
}

/// (Schlüssel, Art, Titel, Nummer) aller Knoten.
async fn outline(server: &common::Server, work: i64) -> Vec<(i64, NodeKind, String, String)> {
    let loaded = server.state.store.load_work(work).await.unwrap();
    loaded.nodes.iter().map(|n| (n.key, n.kind, n.title.clone(), n.number.clone())).collect()
}

fn key_of(nodes: &[(i64, NodeKind, String, String)], title: &str) -> i64 {
    nodes.iter().find(|n| n.2 == title).unwrap_or_else(|| panic!("{title} fehlt: {nodes:?}")).0
}

#[tokio::test]
async fn import_builds_parts_and_export_gives_the_file_back() {
    let server = common::open().await;
    let source = book("Gartenjahr Import");
    let report = server.import(&source, "gartenjahr.typ").await;
    let nodes = outline(&server, report.work).await;
    let kinds: Vec<(NodeKind, &str, &str)> = nodes.iter().map(|n| (n.1, n.2.as_str(), n.3.as_str())).collect();
    assert_eq!(
        kinds,
        [
            (NodeKind::Chapter, "Vorwort", ""),
            (NodeKind::Part, "Vom Pflanzen", "I"),
            (NodeKind::Chapter, "Die Saat", "1"),
            (NodeKind::Chapter, "Das Gießen", "2"),
            (NodeKind::Part, "Vom Ernten", "II"),
            (NodeKind::Chapter, "Der Korb", "3"),
        ],
        "Teil-Markierung → Teil, „ohne Nummer“ → ohne Nummer; Kapitel laufen über Teile fort"
    );
    let exported =
        baruch_typst::xref::strip_key_labels(&server.state.store.export_typst(report.work).await.unwrap()).into_owned();
    assert_eq!(exported, source, "Export byte-gleich zur Quelle, auch mit Werkvorlage");
}

#[tokio::test]
async fn the_typeset_puts_every_part_on_its_own_right_page() {
    let server = common::open().await;
    let report = server.import(&book("Gartenjahr Satz"), "satz.typ").await;
    let work = report.work;
    let nodes = outline(&server, work).await;

    let prepared = baruch_server::typeset::prepare(&server.state, work).await.unwrap();
    let rule = baruch_typst::part_page_rule(&["I".to_owned(), "II".to_owned()]);
    assert_eq!(
        prepared.source.main.matches(&rule).count(),
        1,
        "Regel der Teil-Seite genau einmal, mit den Nummern I und II"
    );
    let first = prepared.map.sections.first().unwrap();
    assert!(
        prepared.source.main[..first.start].ends_with(&rule),
        "Die Regel steht hinter der Werkvorlage direkt vor dem ersten Abschnitt"
    );
    let map = server.state.store.export_map(work).await.unwrap();
    for (moved, original) in prepared.map.sections.iter().zip(&map.sections) {
        assert_eq!(
            &prepared.source.main[moved.start..moved.end],
            // Runde 30a: im Satz steht die Ankerzeile eines Teils vor der Teil-Zeile (sonst nach dem Umbruch).
            &*baruch_typst::xref::part_anchors_for_typesetting(&map.typst[original.start..original.end]),
            "Die Bytebereiche der Abschnitte rücken mit (Meldungen finden ihren Abschnitt)"
        );
        assert_eq!(moved.first_line, original.first_line + 1, "die Regel ist genau eine Zeile");
    }

    let typeset = baruch_server::typeset::typeset(&server.state, work).await.unwrap();
    let errors: Vec<_> = typeset
        .rendered()
        .diagnostics
        .iter()
        .filter(|d| d.severity == baruch_render::Severity::Error)
        .map(|d| d.message.clone())
        .collect();
    assert!(errors.is_empty(), "Werk mit zwei Teilen setzt fehlerfrei (Zähler-asserts im Text gelten): {errors:?}");
    let page = |title: &str| typeset.page_of(Some(key_of(&nodes, title)));
    let (part1, part2) = (page("Vom Pflanzen"), page("Vom Ernten"));
    assert!(part1 % 2 == 1 && part2 % 2 == 1, "Teil-Seiten sind rechte Seiten: {part1}, {part2}");
    assert!(part1 > page("Vorwort"), "der erste Teil beginnt nach dem Vorwort eine neue Seite");
    assert!(page("Die Saat") > part1, "das Kapitel nach dem Teil beginnt auf einer späteren Seite");
    assert!(part2 > page("Das Gießen") && page("Der Korb") > part2, "auch beim zweiten Teil");

    let preview = server.rpc("preview.typst", json!({ "work": work })).await;
    assert_eq!(
        result(&preview)["pages"].as_u64().unwrap() as usize,
        typeset.page_count(),
        "Satz-Ansicht (RPC) und typeset nutzen dasselbe prepare"
    );
}

#[tokio::test]
async fn without_part_markers_the_prepared_source_is_the_export() {
    let server = common::open().await;
    let source = "#set document(title: \"Ohne Teile Satz\")\n\n= Teil I: nur ein Kapitel\nText.\n\n= Zwei\nMehr.\n";
    let report = server.import(source, "ohne.typ").await;
    let prepared = baruch_server::typeset::prepare(&server.state, report.work).await.unwrap();
    assert_eq!(
        prepared.source.main,
        server.state.store.export_map(report.work).await.unwrap().typst,
        "Ohne Teil-Markierung (auch mit „= Teil …“ als Kapitel) ist die vorbereitete Quelle byte-gleich"
    );
    assert!(!prepared.source.main.contains("<baruch-part>"), "keine Regel");
}

#[tokio::test]
async fn work_pdf_has_the_pages_of_the_typeset_with_part_pages() {
    let _serial = common::PDF_SERIAL.lock().await;
    let server = common::open().await;
    let report = server.import(&book("Gartenjahr Druck"), "druck.typ").await;
    let preview = server.rpc("preview.typst", json!({ "work": report.work })).await;
    let pages = result(&preview)["pages"].as_u64().unwrap();
    let pdf = server.rpc("work.pdf", json!({ "work": report.work })).await;
    assert_eq!(result(&pdf)["pages"].as_u64().unwrap(), pages, "PDF mit Teil-Seiten hat die Seiten der Satz-Ansicht");
}

#[tokio::test]
async fn the_satzbild_shows_the_part_and_renames_it_over_part_line() {
    let server = common::open().await;
    let report = server.import(&book("Gartenjahr Satzbild"), "satzbild.typ").await;
    let work = report.work;
    let loaded = server.state.store.load_work(work).await.unwrap();
    let part = loaded.nodes.iter().find(|n| n.title == "Vom Pflanzen").unwrap();
    let body = part.revisions[0].body.clone();

    let parsed = server.rpc("wysiwyg.parse", json!({ "work": work, "body": body })).await;
    let parsed = result(&parsed);
    assert_eq!(parsed["blocks"][0]["kind"], "part", "Blockmodell: Teil als eigene Art: {parsed}");
    assert_eq!(parsed["blocks"][0]["title"], "Vom Pflanzen", "mit Titel");
    assert!(parsed["html"].as_str().unwrap().contains("data-kind=\"part\""), "HTML mit Teil-Block");

    let edits = json!([{ "replaces": 0, "block": { "kind": "part", "title": "Vom Säen und Pflanzen" } }]);
    let saved = server.rpc("wysiwyg.serialize", json!({ "work": work, "body": body, "edits": edits })).await;
    assert_eq!(
        result(&saved)["body"],
        "#heading(level: 1, numbering: none)[Vom Säen und Pflanzen] <baruch-part> // erster Teil\n\n",
        "Titeländerung im Satzbild erzeugt part_line, Kommentar und Leerzeile bleiben"
    );

    let (status, html) = server.get(&format!("/?work={work}&key={}", part.key)).await;
    assert_eq!(status, axum::http::StatusCode::OK, "Editor des Teils");
    assert!(html.contains("data-kind=\"part\""), "Das vorgerenderte Satzbild zeigt den Teil als eigenen Block");
    assert!(
        html.contains("&lt;baruch-part&gt; &#x2f;&#x2f; erster Teil")
            || html.contains("&lt;baruch-part&gt; // erster Teil"),
        "Ohne JavaScript steht die Markierung wörtlich im Textfeld (Speichern wie bisher)"
    );
}
