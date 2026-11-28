// SPDX-License-Identifier: AGPL-3.0-or-later
//! Round 39c „Developer documentation on the website, user help gaps, rest of the interface literals“, task reste_1,
//! owner reste_1: still-German server modules in the interface catalogs, draft date in the work language.
//!
//! Every converted module answers in the interface language of the request (cookie `baruch_lang`), the German text is
//! the wording from before (contract §4.5). The draft date follows the work language: German `%-d.%-m.%Y` (as
//! before), English ISO 8601.

use crate::common;

use axum::body::Body;
use axum::http::{Request, header};
use baruch_core::WorkLanguage;
use baruch_server::connections::multigpt;
use baruch_server::i18n::{self, Lang};
use baruch_server::{rag, typeset};
use baruch_typst::typeset::Draft;
use regex::Regex;
use serde_json::{Value, json};

/// RPC call in the interface language `lang` (cookie, which beats the German pin of the test router).
async fn rpc_in(server: &common::Server, lang: &str, method: &str, params: Value) -> Value {
    let body = json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params });
    let request = Request::post("/rpc")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::COOKIE, format!("{}={lang}", i18n::COOKIE))
        .body(Body::from(body.to_string()))
        .unwrap();
    let (_, _, text) = server.send(request).await;
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("RPC answer is JSON ({e}): {text}"))
}

/// Page in the interface language `lang`.
async fn page_in(server: &common::Server, lang: &str, uri: &str) -> String {
    let request = Request::get(uri)
        .header(header::HOST, "127.0.0.1:8765")
        .header(header::COOKIE, format!("{}={lang}", i18n::COOKIE))
        .body(Body::empty())
        .unwrap();
    let (status, _, text) = server.send(request).await;
    assert!(status.is_success(), "{uri} answers ({status}): {text}");
    text
}

fn error_message(answer: &Value) -> String {
    answer["error"]["message"].as_str().unwrap_or_else(|| panic!("an error is expected: {answer}")).to_owned()
}

fn way_message(status: &Value, way: &str) -> String {
    let ways = status["result"]["ways"].as_array().unwrap_or_else(|| panic!("ways in rag.status: {status}"));
    let found = ways.iter().find(|w| w["way"] == way).unwrap_or_else(|| panic!("way {way} in {status}"));
    found["message"].as_str().unwrap().to_owned()
}

#[tokio::test]
async fn the_semantic_search_speaks_the_interface_language() {
    let server = common::open().await;
    let work = server.state.default_work().await.unwrap();
    // Reason: pgvector is not chosen for the sample work, so rag.status names the sentence of rag/mod.rs.
    let en = rpc_in(&server, "en", "rag.status", json!({ "work": work })).await;
    let de = rpc_in(&server, "de", "rag.status", json!({ "work": work })).await;
    assert_eq!(
        way_message(&en, "pgvector"),
        "Not chosen for this work (Settings → Connections).",
        "English interface: English reason of the path"
    );
    assert_eq!(
        way_message(&de, "pgvector"),
        "Für dieses Werk nicht gewählt (Einstellungen → Verbindungen).",
        "German interface: the wording from before Round 39c"
    );
    // Reason: tests run without an embedder, so the index problem is the sentence of rag/embed.rs.
    assert!(
        en["result"]["index"]["problem"].as_str().unwrap().starts_with("No embedding model connected"),
        "English problem of the index: {en}"
    );
    assert!(
        de["result"]["index"]["problem"].as_str().unwrap().starts_with("Kein Einbettungsmodell angebunden"),
        "German problem of the index: {de}"
    );
    // Reason: a refused RPC parameter of rag.search comes from rag/mod.rs as well.
    let en = rpc_in(&server, "en", "rag.search", json!({ "work": work, "query": "x", "limit": "5" })).await;
    let de = rpc_in(&server, "de", "rag.search", json!({ "work": work, "query": "x", "limit": "5" })).await;
    assert_eq!(error_message(&en), "limit: whole number from 1 to 50.", "English refusal: {en}");
    assert_eq!(error_message(&de), "limit: ganze Zahl von 1 bis 50.", "German refusal verbatim: {de}");
}

#[tokio::test]
async fn the_check_of_lancedb_and_the_nearness_of_ideas_follow_the_language() {
    let server = common::open().await;
    let work = server.state.default_work().await.unwrap();
    // Reason: without an embedder the probe of rag/check.rs fails with the sentence of rag/embed.rs.
    let en = rpc_in(&server, "en", "connection.check", json!({ "kind": "lancedb" })).await;
    let de = rpc_in(&server, "de", "connection.check", json!({ "kind": "lancedb" })).await;
    assert!(en["result"]["message"].as_str().unwrap().contains("No embedding model connected"), "English check: {en}");
    assert!(
        de["result"]["message"].as_str().unwrap().contains("Kein Einbettungsmodell angebunden"),
        "German check: {de}"
    );
    // Reason: rag/neighbors.rs answers in the language of the scope it runs in (the mind map runs in the request).
    let english = i18n::scope(Lang::En, rag::neighbors::idea_vectors(&server.state, work)).await;
    let german = i18n::scope(Lang::De, rag::neighbors::idea_vectors(&server.state, work)).await;
    assert_eq!(
        english,
        rag::neighbors::IdeaVectors::NoEmbedder(
            "No embedding connected (Settings → Connections); nearness only through shared keywords.".into()
        ),
        "English nearness note"
    );
    assert_eq!(
        german,
        rag::neighbors::IdeaVectors::NoEmbedder(
            "Keine Einbettung angebunden (Einstellungen → Verbindungen); Nähe nur über gleiche Schlagwörter.".into()
        ),
        "German nearness note verbatim"
    );
    // Reason: the sentence of rag/index.rs about a changed prefix is the beginning of the mismatch message.
    assert_eq!(i18n::sync_scope(Lang::En, rag::index::prefix_changed), "Task prefix changed; rebuild the index.");
    assert_eq!(i18n::sync_scope(Lang::De, rag::index::prefix_changed), "Aufgaben-Präfix geändert; Index neu aufbauen.");
}

#[tokio::test]
async fn pgvector_and_multigpt_messages_follow_the_language() {
    let server = common::open().await;
    // Reason: a new data folder has no pgvector host, so the check of connections/pgvector.rs refuses at once (no
    // connection is attempted).
    let en = rpc_in(&server, "en", "connection.check", json!({ "kind": "pgvector" })).await;
    let de = rpc_in(&server, "de", "connection.check", json!({ "kind": "pgvector" })).await;
    assert_eq!(en["result"]["message"], "No computer (host) entered.", "English check of pgvector: {en}");
    assert_eq!(de["result"]["message"], "Kein Rechner (Host) eingetragen.", "German check verbatim: {de}");
    // Reason: connections/multigpt.rs reads the answer of search_documents; an answer without a hit list is refused
    // with a sentence in the interface language of the calling request.
    let refuse = || multigpt::parse_hits(&json!({ "note": "no list" }), None).unwrap_err().message().to_owned();
    assert_eq!(
        i18n::sync_scope(Lang::En, refuse),
        "search_documents: no hit list in the answer (results, hits, documents …).",
        "English refusal of the MultiGPT path"
    );
    assert_eq!(
        i18n::sync_scope(Lang::De, refuse),
        "search_documents: keine Trefferliste in der Antwort (results, hits, documents …).",
        "German refusal verbatim"
    );
}

#[tokio::test]
async fn typst_markdown_and_archive_imports_follow_the_language() {
    let server = common::open().await;
    // import.rs: a file that is not Typst.
    let params = json!({ "file_name": "notes.txt", "source": "= A\n" });
    let en = rpc_in(&server, "en", "work.import", params.clone()).await;
    let de = rpc_in(&server, "de", "work.import", params).await;
    assert_eq!(error_message(&en), "Only Typst files (.typ) can be imported.", "English refusal of import.rs");
    assert_eq!(error_message(&de), "Nur Typst-Dateien (.typ) lassen sich importieren.", "German refusal verbatim");
    // markdown_import: text before the first heading becomes a chapter, with a note in the report.
    let source = "Opening words.\n\n# Spring\n\nThe beds are dug.\n";
    let en = rpc_in(
        &server,
        "en",
        "work.import",
        json!({ "file_name": "spring-en.md", "source": source, "title": "Spring EN" }),
    )
    .await;
    let de = rpc_in(
        &server,
        "de",
        "work.import",
        json!({ "file_name": "spring-de.md", "source": source, "title": "Spring DE" }),
    )
    .await;
    let note = |answer: &Value| {
        let warnings = answer["result"]["warnings"].as_array().unwrap_or_else(|| panic!("report: {answer}"));
        let found = warnings.iter().find(|w| w["code"] == "md-preamble-heading").expect("the note of the preamble");
        found["message"].as_str().unwrap().to_owned()
    };
    assert!(note(&en).starts_with("Text before the first heading is now in the chapter"), "English note: {en}");
    assert!(note(&de).starts_with("Text vor der ersten Überschrift steht jetzt im Kapitel"), "German note: {de}");
    // archive.rs: bytes that are not a ZIP archive ("not a zip" in Base64).
    let params = json!({ "file_name": "bundle.zip", "content": "bm90IGEgemlw" });
    let en = rpc_in(&server, "en", "work.import_archive", params.clone()).await;
    let de = rpc_in(&server, "de", "work.import_archive", params).await;
    assert_eq!(
        error_message(&en),
        "bundle.zip: not a ZIP archive: the beginning “PK” of a ZIP archive is missing",
        "English refusal of archive.rs"
    );
    assert_eq!(
        error_message(&de),
        "bundle.zip: kein ZIP-Archiv: der Anfang „PK“ eines ZIP-Archivs fehlt",
        "German refusal verbatim"
    );
}

#[tokio::test]
async fn the_command_line_markdown_import_stays_german() {
    // Reason: main.rs prints its own lines in German and the other CLI imports run in German (Round 39a); the
    // Markdown import of the command line runs in German as well, even outside a request.
    let server = common::open().await;
    let folder = tempfile::tempdir().unwrap();
    let file = folder.path().join("harbour.md");
    std::fs::write(&file, "# Harbour\n\nBoats.\n").unwrap();
    let out =
        baruch_server::markdown_import::cli_import(&server.state, &file, None).await.expect("the import succeeds");
    assert!(out.starts_with("Werk: "), "German report of the command line: {out}");
    assert!(out.contains("\nAbschnitte: 1\n"), "German report line: {out}");
}

#[tokio::test]
async fn the_cloud_of_the_mind_map_speaks_the_interface_language() {
    let server = common::open().await;
    let work = server.state.default_work().await.unwrap();
    // Reason: the sample work has no idea in the cloud, so idea_map/bild/mindmap.rs draws the empty cloud.
    let en = page_in(&server, "en", &format!("/werke/{work}/karte")).await;
    let de = page_in(&server, "de", &format!("/werke/{work}/karte")).await;
    assert!(en.contains("Cloud – empty. Ideas without a place in the mind map lie here."), "English cloud head");
    assert!(!en.contains("Wolke – leer"), "no German cloud head on the English page");
    assert!(de.contains("Wolke – leer. Einfälle ohne Platz in der Mindmap liegen hier."), "German cloud head verbatim");
}

#[test]
fn the_draft_date_follows_the_work_language() {
    // Reason: the German form is the one from before Round 39c (day.month.year without leading zeros), the English
    // form is ISO 8601 without words.
    let german = typeset::draft_today_in(WorkLanguage::De);
    let english = typeset::draft_today_in(WorkLanguage::En);
    assert!(Regex::new(r"^\d{1,2}\.\d{1,2}\.\d{4}$").unwrap().is_match(&german), "German date: {german}");
    assert!(Regex::new(r"^\d{4}-\d{2}-\d{2}$").unwrap().is_match(&english), "English date: {english}");
    assert_eq!(typeset::draft_today(), german, "draft_today keeps the German form (byte-equal)");
    // Reason: the rule of the watermark of a book PDF, a cover or a preview carries the date of its work language.
    let draft = Draft { date: true, ..Draft::default() };
    let rule_en = typeset::draft_rule_for_in(&draft, WorkLanguage::En);
    let rule_de = typeset::draft_rule_for_in(&draft, WorkLanguage::De);
    assert!(
        rule_en.contains(&typeset::draft_today_in(WorkLanguage::En)),
        "English work: ISO date in the rule: {rule_en}"
    );
    assert!(
        rule_de.contains(&typeset::draft_today_in(WorkLanguage::De)),
        "German work: German date in the rule: {rule_de}"
    );
    assert_eq!(rule_de, typeset::draft_rule_for(&draft), "the German rule stays byte-equal to draft_rule_for");
    // Reason: the EPUB writes the date into the note of its draft addition.
    let epub_en = baruch_server::epub::epub_draft_in(Some(&draft), Some(true), WorkLanguage::En).expect("draft");
    let epub_de = baruch_server::epub::epub_draft_in(Some(&draft), Some(true), WorkLanguage::De).expect("draft");
    assert!(epub_en.note.contains(&typeset::draft_today_in(WorkLanguage::En)), "English EPUB note: {}", epub_en.note);
    assert!(epub_de.note.contains(&typeset::draft_today_in(WorkLanguage::De)), "German EPUB note: {}", epub_de.note);
}
