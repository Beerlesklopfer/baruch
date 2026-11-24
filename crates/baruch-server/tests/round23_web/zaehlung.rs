// SPDX-License-Identifier: AGPL-3.0-or-later
//! Runde 23a (Codereview quellen-1, quellen-2, ki-1): Belege aus SWORD und Kommentare lesen eine Stelle in deutscher
//! Zählung über die Abbildung in die Zählung des Moduls; `source.read` begrenzt die Suchanfrage zum Markieren; Anträge
//! zur Gliederung jenseits der ersten zwanzig bekommen ihre Vorschau auf Anforderung. Module, Texte und Werke frei
//! erfunden (`baruch_sword::fixture`).

use axum::http::StatusCode;
use baruch_scripture::versification::versification;
use baruch_server::bible_research::{NoteKind, research};
use baruch_store::{IdeaOrigin, OutlineOp};
use baruch_sword::fixture::{Block, Fixture, Layout, write};
use serde_json::{Value, json};

use crate::common;

/// Erfundener Text mit zwei Stellen, deren Kapitel in der KJV-Zählung anders liegen: Joel 3,1 (deutsch) ist KJV
/// Joel 2,28; Mal 3,20 (deutsch) ist KJV Mal 4,2.
const BOOK: &str = "#set document(title: \"Der Regen im Tal\", author: \"Hanne Voss\")\n\n\
= Zusage\n\nWie Joel 3,1 sagt, kommt der Regen. Und Mal 3,20 nennt die Sonne.\n";

async fn ok(server: &common::Server, method: &str, params: Value) -> Value {
    let answer = server.rpc(method, params).await;
    assert!(answer["error"].is_null(), "{method} sollte gelingen: {answer}");
    answer["result"].clone()
}

/// Server mit einer Bibel und einem Kommentar in KJV-Zählung (frei erfundener Inhalt), beides für das Werk gewählt.
async fn kjv_work() -> (common::Server, tempfile::TempDir, i64) {
    let kjv = versification("KJV").expect("KJV ist eingebettet");
    let bible = Fixture::new("Talbibel", Layout::ZText(Block::Book), kjv)
        .conf("Description", "Erfundene Talbibel in KJV-Zählung")
        .conf("DistributionLicense", "Public Domain")
        .verse("Joel", 2, 28, "RICHTIG: Der Regen fällt auf alle Täler.")
        .verse("Joel", 3, 1, "FALSCH: Das wäre deutsch Joel 4,1.")
        .verse("Mal", 4, 2, "RICHTIG: Die Sonne geht über dem Tal auf.");
    let commentary = Fixture::new("Talkommentar", Layout::RawCom, kjv)
        .conf("Description", "Erfundener Talkommentar in KJV-Zählung")
        .conf("DistributionLicense", "Public Domain")
        .verse("Joel", 2, 28, "RICHTIG: Regen meint hier Fülle.")
        .verse("Joel", 3, 1, "FALSCH: Kommentar zu deutsch Joel 4,1.")
        .verse("Mal", 4, 2, "RICHTIG: Die Sonne meint Heilung.");
    let modules = tempfile::tempdir().unwrap();
    let root = modules.path().join("sword");
    write(&root, &bible).unwrap();
    write(&root, &commentary).unwrap();
    let server = common::open().await;
    ok(&server, "bible.set_dirs", json!({ "dirs": [root.display().to_string()] })).await;
    let work = server.import(BOOK, "regen-23.typ").await.work;
    ok(&server, "bible.set_work", json!({ "work": work, "module": "Talbibel" })).await;
    ok(&server, "bible.set_work_sources", json!({ "work": work, "commentaries": ["Talkommentar"], "lexicons": [] }))
        .await;
    (server, modules, work)
}

#[tokio::test]
async fn sword_evidence_reads_german_references_through_the_mapping_of_the_module() {
    let (server, _modules, work) = kjv_work().await;
    let found = research(&server.state, work, "@belege", BOOK, 10_000).await.expect("@belege bekommt Belege");
    let note =
        |kind: NoteKind, reference: &str| {
            found.notes.iter().find(|n| n.kind == kind && n.reference == reference).unwrap_or_else(|| {
                panic!("Beleg {kind:?} {reference} fehlt: {:#?} / {:?}", found.notes, found.problems)
            })
        };
    assert_eq!(
        note(NoteKind::Bible, "Joel 3,1").text,
        "RICHTIG: Der Regen fällt auf alle Täler.",
        "deutsch Joel 3,1 ist im KJV-Modul Joel 2,28 – gelesen wird der abgebildete Vers, nicht KJV 3,1"
    );
    assert_eq!(
        note(NoteKind::Bible, "Mal 3,20").text,
        "RICHTIG: Die Sonne geht über dem Tal auf.",
        "deutsch Mal 3,20 ist KJV Mal 4,2 (vorher: „Modul hat dort keinen Text“)"
    );
    assert_eq!(
        note(NoteKind::Commentary, "Joel 3,1").text,
        "RICHTIG: Regen meint hier Fülle.",
        "auch der Kommentar wird in seiner Zählung an der abgebildeten Stelle nachgeschlagen"
    );
    assert_eq!(note(NoteKind::Commentary, "Mal 3,20").text, "RICHTIG: Die Sonne meint Heilung.", "ebenso Mal 4,2");
    assert!(
        !found.notes.iter().any(|n| n.text.contains("FALSCH")),
        "kein Beleg trägt den Text der unverschobenen Stelle: {:#?}",
        found.notes
    );
}

#[tokio::test]
async fn commentary_lookup_maps_the_german_reference_and_names_the_counting_of_the_module() {
    let (server, _modules, work) = kjv_work().await;
    let looked = ok(&server, "commentary.lookup", json!({ "work": work, "ref": "Joel 3,1" })).await;
    let entry = &looked["commentaries"][0]["entries"][0];
    assert_eq!(entry["text"], "RICHTIG: Regen meint hier Fülle.", "der Eintrag zur abgebildeten Stelle: {looked}");
    assert_eq!(
        entry["ref"], "Joel 2,28 (KJV)",
        "die Beschriftung nennt die Stelle des Kommentars mit dem Namen seiner Zählung – niemand hält sie für die \
         deutsche: {looked}"
    );
    let malachi = ok(&server, "commentary.lookup", json!({ "work": work, "ref": "Mal 3,20" })).await;
    assert_eq!(
        malachi["commentaries"][0]["entries"][0]["text"], "RICHTIG: Die Sonne meint Heilung.",
        "Mal 3,20 (deutsch) findet den Kommentar zu KJV Mal 4,2 (vorher: „keinen Eintrag“): {malachi}"
    );
    assert_eq!(malachi["commentaries"][0]["entries"][0]["ref"], "Mal 4,2 (KJV)", "mit Modulstelle: {malachi}");
}

#[tokio::test]
async fn source_read_refuses_an_overlong_marking_query() {
    let server = common::open().await;
    let limit = baruch_server::literature::MAX_MARK_QUERY_CHARS;
    let long = "zqx ".repeat(limit / 4 + 1);
    let refused = server.rpc("source.read", json!({ "document": "doc-erfunden", "q": long })).await;
    assert_eq!(refused["error"]["code"], -32602, "überlange Anfrage → -32602, bevor etwas gelesen wird: {refused}");
    assert!(
        refused["error"]["message"].as_str().unwrap().contains(&format!("höchstens {limit} Zeichen")),
        "die Meldung nennt die Grenze: {refused}"
    );

    // Die Markierung selbst sucht höchstens zwanzig verschiedene Wörter und bleibt damit schnell.
    let text = "Das Licht kreist über dem Wasser. ".repeat(400);
    let query: String = (0..5000).map(|n| format!("wort{n} ")).collect();
    let started = std::time::Instant::now();
    let marks = baruch_server::literature::marks(&text, Some(&query));
    assert!(marks.is_empty(), "keines der erfundenen Wörter kommt vor");
    assert!(
        started.elapsed() < std::time::Duration::from_secs(2),
        "5000 nicht vorkommende Wörter kosten keine Sekunden mehr ({:?}): gesucht werden höchstens {}",
        started.elapsed(),
        baruch_server::literature::MAX_MARK_WORDS
    );
    assert_eq!(
        baruch_server::literature::marks("Am Ufer steht ein Kran.", Some("Kran Ufer")),
        vec![[3, 7], [18, 22]],
        "eine gewöhnliche Anfrage markiert wie bisher ihre Wörter"
    );
}

#[tokio::test]
async fn only_the_first_requests_get_their_preview_at_once_the_rest_on_demand() {
    let server = common::open().await;
    let work = server
        .import("= Am Kai\n\nDie Fähre legt an.\n\n= Im Lager\n\nKisten bis zur Decke.\n", "kai-23.typ")
        .await
        .work;
    let key = server.state.store.load_work(work).await.unwrap().nodes[0].key;
    let immediate = baruch_server::outline::MAX_IMMEDIATE_PREVIEWS;
    let mcp = IdeaOrigin::Mcp { handle: "@mcp:probe".into() };
    let mut ids = Vec::new();
    for n in 0..immediate + 2 {
        let op = OutlineOp::Rename { key, title: format!("Am Kai {n}") };
        ids.push(server.state.store.outline_request(work, op, &mcp, "").await.unwrap().id);
    }
    let (status, page) = server.get(&format!("/gliederung?work={work}")).await;
    assert_eq!(status, StatusCode::OK, "die Seite lädt");
    assert_eq!(
        page.matches("<details class=\"request-preview\"").count(),
        immediate,
        "nur die ersten {immediate} offenen Anträge haben ihre Vorschau sofort"
    );
    assert_eq!(
        page.matches("data-preview-later").count(),
        2,
        "die übrigen zwei tragen einen Link „Vorschau anzeigen“ (ohne JavaScript bedienbar)"
    );
    let last = *ids.last().unwrap();
    let link = format!("/gliederung/antraege/{last}/vorschau?work={work}");
    assert!(
        page.contains(&format!("/gliederung/antraege/{last}/vorschau?work={work}").replace('/', "&#x2f;"))
            || page.contains(&link),
        "der Link zeigt auf die Vorschau dieses Antrags: {page}"
    );

    let (status, shown) = server.get(&link).await;
    assert_eq!(status, StatusCode::OK, "die Vorschau auf Anforderung lädt (nur lesend)");
    let row = &shown[shown.find(&format!("id=\"antrag-{last}\"")).expect("Zeile des Antrags")..];
    let row = &row[..row.find("</tr>").unwrap()];
    assert!(
        row.contains("<details class=\"request-preview\" open>") && row.contains(&format!("Am Kai {}", immediate + 1)),
        "die verlangte Vorschau ist gerechnet und aufgeklappt, mit dem neuen Titel: {row}"
    );
    assert!(row.contains("umbenannt"), "sie zeigt die Wirkung des Antrags: {row}");
    let open = server.state.store.open_outline_requests(work).await.unwrap();
    assert_eq!(open, immediate + 2, "die Vorschau hat nichts entschieden und nichts geändert");
    let title = server.state.store.load_work(work).await.unwrap().nodes[0].title.clone();
    assert_eq!(title, "Am Kai", "der Titel ist unverändert");
}
