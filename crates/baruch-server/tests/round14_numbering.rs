// SPDX-License-Identifier: AGPL-3.0-or-later
//! Runde 14 c „nummerierung“ (Vertrag Abschnitte 3 und 4): eine Nummerierungsquelle für Baum, Satz und
//! Re-Import, Kapitel ohne Nummer als Markierung im Export, Seite „Gliederungsschema“ mit engem Kopf.
//! Echter Datenordner im Temp-Ordner, Gartenbuch aus `tests/data` (Frühling, Aussaat, Tiefe, Sommer).

use crate::common;

use axum::http::StatusCode;
use serde_json::{Value, json};

fn result(answer: &Value) -> &Value {
    assert!(answer.get("error").is_none(), "RPC-Fehler: {answer}");
    &answer["result"]
}

async fn numbers(server: &common::Server, work: i64) -> Vec<String> {
    let loaded = server.rpc("work.get", json!({ "work": work })).await;
    result(&loaded)["work"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n["number"].as_str().unwrap().to_owned())
        .collect()
}

async fn export(server: &common::Server, work: i64) -> String {
    let answer = server.rpc("work.export", json!({ "work": work })).await;
    result(&answer)["typst"].as_str().unwrap().to_owned()
}

/// Ausschnitt `<header class="numbering-head">…</header>` einer Seite.
fn head(html: &str) -> &str {
    let start = html.find("<header class=\"numbering-head\">").unwrap_or_else(|| panic!("Kopf fehlt: {html}"));
    let rest = &html[start..];
    &rest[..rest.find("</header>").expect("Kopf geschlossen")]
}

#[tokio::test]
async fn the_numbering_page_head_is_one_tight_block_on_both_pages() {
    // Backlog „Seite Gliederungsschema: große Abstände oben“: Hinweis, Überschrift und Absätze standen
    // einzeln in der Flex-Spalte und behielten ihre Absatzränder. Jetzt ein Block mit Rand 0 und
    // kleiner Lücke – auf der eigenen Seite und auf Einstellungen → Werk (gleiche Vorlage).
    let server = common::open().await;
    let work = server.import_garden().await.work;
    let (status, page) = server.get(&format!("/werke/{work}/nummerierung?ok=1")).await;
    assert_eq!(status, StatusCode::OK, "{page}");
    let top = head(&page);
    assert!(
        top.contains("<h1 class=\"pane-title\">Gliederungsschema von „Das Gartenbuch“</h1>"),
        "Überschrift im Kopf"
    );
    assert!(top.contains("<p class=\"small\">Jetzt: <strong>Aus der Werkvorlage</strong>"), "Stand im Kopf: {top}");
    assert!(top.contains("<p class=\"muted small\">Das Schema gilt"), "Erklärung im Kopf: {top}");
    assert!(top.contains("numbering: none"), "die Erklärung nennt die Markierung ohne Nummer: {top}");
    let form = page.find("<form class=\"numbering-choice\"").expect("Formular");
    assert!(page.find("<header class=\"numbering-head\">").unwrap() < form, "der Kopf steht vor dem Formular");
    assert!(page.contains(">Übernehmen</button>"), "ohne JavaScript bedienbar: Formular mit Knopf");

    let (_, style) = server.get("/static/app.css").await;
    assert!(
        style.contains(".numbering-head { display: flex; flex-direction: column; gap: 4px; margin: 0; }"),
        "Kopf eng"
    );
    assert!(style.contains(".numbering-head > p, .numbering-head > h1 { margin: 0; }"), "Absätze ohne eigene Ränder");

    let (status, settings) = server.get(&format!("/einstellungen/werk?work={work}")).await;
    assert_eq!(status, StatusCode::OK, "{settings}");
    assert!(
        head(&settings).contains("Gliederungsschema von „Das Gartenbuch“"),
        "auch in den Einstellungen derselbe Kopf"
    );
}

#[tokio::test]
async fn a_work_with_a_word_scheme_exported_and_imported_again_keeps_its_numbers() {
    // Vertrag Runde 14, Abschnitt 3: Die Regel von „Juristisch“ (und den anderen Schemata mit Wörtern)
    // kommt beim Re-Import als genau dieses Schema zurück – bis Runde 13 als „1.1“.
    let server = common::open().await;
    let work = server.import_garden().await.work;
    for scheme in ["legal", "sermon", "book", "parts_chapters", "chapter_words"] {
        result(&server.rpc("numbering.set", json!({ "work": work, "scheme": scheme })).await);
        let expected = numbers(&server, work).await;
        // Eigener Titel je Kopie (Werktitel sind eindeutig); die Nummerierung bleibt davon unberührt.
        let typst = export(&server, work).await.replace("Das Gartenbuch", &format!("Gartenbuch {scheme}"));
        let again = server.import(&typst, &format!("wieder-{scheme}.typ")).await.work;
        assert_eq!(numbers(&server, again).await, expected, "{scheme}: Re-Import hat dieselben Nummern");
        let schemes = server.rpc("numbering.schemes", json!({ "work": again })).await;
        assert_eq!(result(&schemes)["current"]["scheme"], "template", "{scheme}: re-importiert gilt die Werkvorlage");
        assert_eq!(
            baruch_typst::xref::strip_key_labels(&export(&server, again).await),
            baruch_typst::xref::strip_key_labels(&typst),
            "{scheme}: Export → Import → Export byte-gleich (bis auf die Sprungmarken)"
        );
    }
}
