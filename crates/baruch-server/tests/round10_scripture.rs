// SPDX-License-Identifier: AGPL-3.0-or-later
//! Passage check in the server (Round 10, task „scripture“): RPC `scripture.check` (the dispatch is set by the
//! integrator, here through `scripture::call`), MCP tool `scripture_check` (function directly), hooks `facts_for` and
//! `mark_answer` for the personas. Without a chosen Bible the wording is always „ungeprüft“ (with a Bible:
//! `sword_bibles.rs`).

use crate::common;

use baruch_server::mcp::tools::Failure;
use baruch_server::scripture::{self, FACTS_END, MARK_OTHERS, MARK_SCRIPTURE};
use serde_json::{Value, json};

async fn check(server: &common::Server, text: &str) -> Value {
    scripture::call(&server.state, "scripture.check", json!({ "text": text })).await.expect("scripture.check")
}

#[tokio::test]
async fn scripture_check_reports_each_reference_and_never_confirms_a_wording() {
    let server = common::open().await;
    assert!(scripture::handles("scripture.check") && !scripture::handles("scripture.quote"), "nur check");
    let result = check(&server, "1. Kön 3,16–28; Ps 151,1; Ps 139,25; Henoch 1,9").await;
    let refs = result["refs"].as_array().unwrap();
    let status: Vec<&str> = refs.iter().map(|r| r["status"].as_str().unwrap()).collect();
    assert_eq!(
        status,
        ["valid", "no_such_chapter", "no_such_verse", "unknown_book"],
        "je Angabe ein Ergebnis: {result}"
    );
    assert_eq!(refs[0]["normalized"], "1. Kön 3,16–28", "einheitliche Schreibweise: {result}");
    assert_eq!(refs[0]["book"], "1. Könige", "Buchname nach Schlachter: {result}");
    assert!(refs[1]["message"].as_str().unwrap().contains("150 Kapitel"), "Maximum genannt: {result}");
    assert!(refs[2]["message"].as_str().unwrap().contains("24 Verse"), "Maximum genannt: {result}");
    assert_eq!(refs[3]["book"], Value::Null, "unbekanntes Buch ohne Namen: {result}");
    assert!(refs.iter().all(|r| r["wording"] == "ungeprüft"), "Wortlaut immer ungeprüft: {result}");
    let source = result["source"].as_str().unwrap();
    assert!(source.contains("SWORD") && source.contains("Schlachter"), "Quelle genannt: {source}");

    let prose =
        check(&server, "Im Gleichnis (2. Sam 12,1–12) hält Nathan dem König den Spiegel vor;\nvgl. Gal 6,4.").await;
    let found: Vec<&str> =
        prose["refs"].as_array().unwrap().iter().map(|r| r["normalized"].as_str().unwrap()).collect();
    assert_eq!(found, ["2. Sam 12,1–12", "Gal 6,4"], "Fließtext wird nach Stellen durchsucht: {prose}");
    let span = &prose["refs"][0]["span"];
    let text = "Im Gleichnis (2. Sam 12,1–12) hält Nathan dem König den Spiegel vor;\nvgl. Gal 6,4.";
    let (a, b) = (span[0].as_u64().unwrap() as usize, span[1].as_u64().unwrap() as usize);
    assert_eq!(&text[a..b], "2. Sam 12,1–12", "Byte-Bereich trifft die Angabe");

    let missing = scripture::call(&server.state, "scripture.check", json!({})).await;
    assert!(missing.is_err_and(|e| e.code == baruch_server::rpc::INVALID_PARAMS), "ohne text → -32602");
}

#[tokio::test]
async fn the_mcp_tool_answers_like_the_rpc_method_and_only_reads() {
    let server = common::open().await;
    assert_eq!(baruch_server::mcp::scripture::NAME, "scripture_check", "Werkzeugname nach Vertrag");
    assert!(baruch_server::mcp::scripture::DESCRIPTION.contains("unchecked"), "Beschreibung nennt den Wortlaut");
    assert!(!baruch_server::mcp::scripture::DESCRIPTION.starts_with("STUB"), "kein Stub");
    let schema = baruch_server::mcp::scripture::schema();
    // Since 2026-11-06 key (section of the work) works instead of text too; one of the two is mandatory.
    assert!(
        schema["properties"]["text"].is_object() && schema["properties"]["key"].is_object(),
        "text oder key: {schema}"
    );
    let before = server.state.store.load_work(server.state.default_work().await.unwrap()).await.unwrap();

    let tool = baruch_server::mcp::scripture::call(&server.state, json!({ "text": "Joh 3,16; 4,1" })).await.unwrap();
    let rpc = check(&server, "Joh 3,16; 4,1").await;
    assert_eq!(tool, rpc, "MCP und RPC geben dasselbe");
    assert_eq!(tool["refs"][1]["normalized"], "Joh 4,1", "Buch übertragen: {tool}");
    assert!(tool["refs"].as_array().unwrap().iter().all(|r| r["wording"] == "ungeprüft"), "ungeprüft: {tool}");
    let without = baruch_server::mcp::scripture::call(&server.state, json!({})).await;
    assert!(matches!(without, Err(Failure::Args(_))), "ohne text → Fehler in den Argumenten: {without:?}");

    let after = server.state.store.load_work(server.state.default_work().await.unwrap()).await.unwrap();
    assert_eq!(format!("{before:?}"), format!("{after:?}"), "die Prüfung ändert nichts am Werk");
}

#[test]
fn facts_are_given_only_to_schrift_and_belege() {
    let section = "== Weisheit\n\nSalomo urteilt klug (1. Kön 3,16–28), wie Ps 151,1 nie sagt.\n";
    let facts = scripture::facts_for("@schrift", section).expect("@schrift bekommt Fakten");
    assert!(
        facts.starts_with("Stellenprüfung (Baruch, Verszählung Schlachter 1951 – kein Wortlaut):"),
        "Kopf nach Vertrag: {facts}"
    );
    assert!(facts.contains("„1. Kön 3,16–28“ → 1. Kön 3,16–28: vorhanden"), "gültige Stelle: {facts}");
    assert!(facts.contains("Psalmen hat nur 150 Kapitel"), "Fehler mit Maximum: {facts}");
    assert!(facts.ends_with(FACTS_END), "Schlusssatz zum Wortlaut: {facts}");
    assert!(FACTS_END.contains("Bestätige keinen Wortlaut"), "ausdrückliches Verbot");
    assert!(scripture::facts_for("belege", section).is_some(), "@belege auch ohne @");
    for other in ["@lektor", "@exeget", "@nathan", "@hoerer", ""] {
        assert_eq!(scripture::facts_for(other, section), None, "{other} bekommt keinen Faktenblock");
    }
    let none = scripture::facts_for("@schrift", "== Leer\n\nKein Bezug auf die Schrift.\n").unwrap();
    assert!(none.contains("keine Stellenangabe gefunden"), "ohne Stelle ehrlich: {none}");
    assert!(none.ends_with(FACTS_END), "auch dann der Schlusssatz");
}

#[test]
fn answers_get_the_wording_note_after_the_contract() {
    let with_ref = "== Abschnitt\n\nVgl. Joh 3,16.\n";
    let plain = "== Abschnitt\n\nOhne Stelle.\n";

    let marked = scripture::mark_answer("@schrift", with_ref, "Die Zuordnung stimmt.".to_owned());
    assert!(marked.ends_with(MARK_SCRIPTURE), "@schrift mit Stelle im Abschnitt: {marked}");
    assert!(marked.starts_with("Die Zuordnung stimmt."), "Antwort bleibt vorn");
    let marked = scripture::mark_answer("@belege", plain, "Siehe Röm 8,28.".to_owned());
    assert!(marked.ends_with(MARK_SCRIPTURE), "@belege mit Stelle in der Antwort: {marked}");
    let twice = scripture::mark_answer("@schrift", with_ref, marked.clone());
    assert_eq!(twice.matches(MARK_SCRIPTURE).count(), 1, "die Zeile kommt nur einmal");
    assert_eq!(
        scripture::mark_answer("@schrift", plain, "Keine Stelle.".to_owned()),
        "Keine Stelle.",
        "@schrift ohne Stelle: unverändert"
    );

    let other = scripture::mark_answer("@lektor", plain, "Kritisch: vgl. Gal 6,4.".to_owned());
    assert!(other.ends_with(MARK_OTHERS), "andere Persona nennt eine Stelle: {other}");
    assert_eq!(
        scripture::mark_answer("@lektor", with_ref, "Kritisch: „Vgl.“ ist knapp.".to_owned()),
        "Kritisch: „Vgl.“ ist knapp.",
        "andere Persona ohne Stelle in der Antwort: unverändert (die Stelle im Abschnitt zählt nicht)"
    );
}
