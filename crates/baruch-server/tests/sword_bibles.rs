// SPDX-License-Identifier: AGPL-3.0-or-later
//! SWORD Bibles in the server (wish 2026-11-06): module directory and Bible per work (RPC and form without JavaScript),
//! `bible.lookup`/`bible.search`/`bible.check`, wording check in `scripture.check`, fact block and hint for @schrift,
//! MCP `bible_*` and `scripture_check {key}`. The test writes the Bibles itself (`baruch_sword::fixture`) with freely
//! invented text into a temp directory; the real module directories of the machine are never read (every test sets its
//! directories).

use crate::common;

use std::path::Path;

use baruch_scripture::versification::{german, versification};
use baruch_server::scripture::{FACTS_END_BIBLE, MARK_BIBLE, MARK_SCRIPTURE};
use baruch_sword::fixture::{Block, Fixture, Layout, write};
use serde_json::{Value, json};

const BOOK: &str = "#set document(title: \"Das Kieselbuch\", author: \"Lena Fluss\")\n\n\
= Am Bach\n\n\
Wie es heißt: „Der Kiesel lag am Bach, und die Amsel sang.“ (Joh 3,16)\n\n\
Nur halb: „und die Amsel sang“ (Joh 3,16). Falsch erinnert: „Der Stein lag am Bach“ (Joh 3,16).\n\n\
Ohne Zitat Ps 23,1. Die Stelle Ps 23,2 fehlt im Modul. Ungültig: Ps 151,1.\n\n\
#quote(block: true, attribution: [Ps 23,1])[Ein Lied vom stillen Wasser.]\n";

/// Invented test Bible in German versification (zText, blocks per book).
fn kiesel() -> Fixture {
    Fixture::new("Kiesel", Layout::ZText(Block::Book), german())
        .conf("Description", "Erfundene Kieselbibel")
        .conf("DistributionLicense", "Public Domain")
        .conf("Copyright", "frei erfunden für Tests")
        .conf("Lang", "de")
        .verse(
            "John",
            3,
            16,
            "Der Kiesel lag am Bach, und die <divineName>Amsel</divineName> sang.<note>Fußnote</note>",
        )
        .verse("Ps", 23, 1, "Ein Lied vom stillen Wasser.")
        .verse("Mal", 3, 24, "Ende des erfundenen Alten Testaments.")
}

/// Invented test Bible in English versification (RawText).
fn english() -> Fixture {
    Fixture::new("Englisch", Layout::RawText, versification("KJV").unwrap())
        .conf("Description", "Erfundene Bibel mit KJV-Zählung")
        .verse("John", 3, 16, "Der Kiesel lag am Bach, und die Amsel sang.")
        .verse("Mal", 4, 6, "Ende in englischer Zählung.")
}

fn modules_dir(dir: &Path, modules: &[Fixture]) -> String {
    let root = dir.join("sword");
    for m in modules {
        write(&root, m).unwrap();
    }
    root.display().to_string()
}

async fn ok(server: &common::Server, method: &str, params: Value) -> Value {
    let answer = server.rpc(method, params).await;
    assert!(answer["error"].is_null(), "{method} sollte gelingen: {answer}");
    answer["result"].clone()
}

async fn err(server: &common::Server, method: &str, params: Value) -> Value {
    let answer = server.rpc(method, params).await;
    assert!(answer["error"].is_object(), "{method} sollte scheitern: {answer}");
    answer["error"].clone()
}

/// Server with its own module directory and the pebble book; returns server, work and the key of the section.
async fn setup(modules: &[Fixture]) -> (common::Server, tempfile::TempDir, i64, i64) {
    let server = common::open().await;
    let bibles = tempfile::tempdir().unwrap();
    let root = modules_dir(bibles.path(), modules);
    ok(&server, "bible.set_dirs", json!({ "dirs": [root] })).await;
    let work = server.import(BOOK, "kiesel.typ").await.work;
    let loaded = ok(&server, "work.get", json!({ "work": work })).await;
    let key = loaded["work"]["nodes"][0]["key"].as_i64().unwrap();
    (server, bibles, work, key)
}

#[tokio::test]
async fn module_folders_list_the_bibles_with_description_versification_and_licence() {
    let server = common::open().await;
    let bibles = tempfile::tempdir().unwrap();
    let root = modules_dir(bibles.path(), &[kiesel(), english()]);
    let missing = bibles.path().join("gibt-es-nicht").display().to_string();
    let listed = ok(&server, "bible.set_dirs", json!({ "dirs": [missing, root] })).await;
    assert_eq!(listed["default_dirs"], false, "eigene Liste statt Vorgabe: {listed}");
    let modules = listed["modules"].as_array().unwrap();
    let names: Vec<&str> = modules.iter().map(|m| m["module"].as_str().unwrap()).collect();
    assert_eq!(names, ["Englisch", "Kiesel"], "beide Bibeln, nach conf-Datei sortiert: {listed}");
    let k = &modules[1];
    assert_eq!(k["description"], "Erfundene Kieselbibel", "Beschreibung aus der conf: {k}");
    assert_eq!(k["versification"], "German", "Versifikation aus der conf: {k}");
    assert_eq!(k["license"], "Public Domain; frei erfunden für Tests", "Lizenzvermerk aus der conf: {k}");
    assert_eq!(modules[0]["versification"], "KJV", "zweite Zählung: {listed}");
    assert!(
        listed["problems"].as_array().unwrap().iter().any(|p| p.as_str().unwrap().contains("kein Ordner mods.d")),
        "fehlender Ordner gemeldet: {listed}"
    );
    let refused = err(&server, "bible.set_dirs", json!({ "dirs": ["relativ/ordner"] })).await;
    assert_eq!(refused["code"], -32602, "relativer Ordner abgelehnt: {refused}");
    let reset = ok(&server, "bible.set_dirs", json!({ "dirs": null })).await;
    assert_eq!(reset["default_dirs"], true, "null stellt die Vorgabe wieder her: {reset}");
}

#[tokio::test]
async fn the_bible_of_a_work_is_chosen_without_javascript_on_the_sources_page() {
    let (server, _bibles, work, _key) = setup(&[kiesel()]).await;
    let (status, html) = server.get(&format!("/einstellungen/quellen?work={work}")).await;
    assert_eq!(status, 200, "Seite Quellen: {html}");
    assert!(html.contains("Bibeln (SWORD)"), "Abschnitt Bibeln: {html}");
    assert!(html.contains("<code>Kiesel</code>") && html.contains("Erfundene Kieselbibel"), "Modul mit Beschreibung");
    assert!(html.contains("Public Domain; frei erfunden für Tests"), "Lizenzvermerk sichtbar");
    assert!(html.contains("value=\"bible.set_work\"") && html.contains("value=\"bible.set_dirs\""), "zwei Formulare");
    assert!(
        html.contains("keine (Wortlaut ungeprüft)\" selected") || html.contains("value=\"\" selected"),
        "noch keine Bibel"
    );

    let (status, location) = server
        .post_form("/einstellungen/quellen/act", &format!("method=bible.set_work&work={work}&module=Kiesel"))
        .await;
    assert_eq!(status, 303, "Formular antwortet mit Weiterleitung");
    assert_eq!(
        location.as_deref(),
        Some(format!("/einstellungen/quellen?ok=bibel_gewaehlt&work={work}#bibeln").as_str()),
        "fester Code"
    );
    let chosen = ok(&server, "bible.modules", json!({ "work": work })).await;
    assert_eq!(chosen["work_bible"], "Kiesel", "Wahl gespeichert: {chosen}");
    let (_, html) = server.get(&format!("/einstellungen/quellen?work={work}&ok=bibel_gewaehlt")).await;
    assert!(html.contains("value=\"Kiesel\" selected"), "gewählte Bibel vorausgewählt: {html}");
    assert!(html.contains("prüft jetzt den Wortlaut"), "Erfolgsmeldung: {html}");

    let (_, location) = server
        .post_form("/einstellungen/quellen/act", &format!("method=bible.set_work&work={work}&module=Fehlt"))
        .await;
    assert!(location.unwrap().contains("fehler=bibel_modul"), "unbekanntes Modul wird abgelehnt");
    assert_eq!(ok(&server, "bible.modules", json!({ "work": work })).await["work_bible"], "Kiesel", "Wahl bleibt");
    let (_, location) =
        server.post_form("/einstellungen/quellen/act", "method=bible.set_dirs&dirs=relativ%2Fordner").await;
    assert!(location.unwrap().contains("fehler=bibel_ordner"), "relativer Ordner im Formular abgelehnt");
    let (_, location) =
        server.post_form("/einstellungen/quellen/act", &format!("method=bible.set_work&work={work}&module=")).await;
    assert!(location.unwrap().contains("ok=bibel_keine"), "abwählen");
    assert_eq!(ok(&server, "bible.modules", json!({ "work": work })).await["work_bible"], Value::Null, "keine Bibel");
}

#[tokio::test]
async fn lookup_and_search_name_module_versification_and_licence() {
    let (server, _bibles, work, _key) = setup(&[kiesel()]).await;
    ok(&server, "bible.set_work", json!({ "work": work, "module": "Kiesel" })).await;
    let found = ok(&server, "bible.lookup", json!({ "work": work, "ref": "Joh 3,16; Ps 23,1–2" })).await;
    assert_eq!(found["bible"]["module"], "Kiesel", "Modul genannt: {found}");
    assert_eq!(found["bible"]["versification"], "German", "Versifikation genannt: {found}");
    assert_eq!(found["bible"]["license"], "Public Domain; frei erfunden für Tests", "Lizenz genannt: {found}");
    assert_eq!(
        found["refs"][0]["verses"][0]["text"], "Der Kiesel lag am Bach, und die Amsel sang.",
        "Klartext ohne Fußnote, divineName als Text: {found}"
    );
    assert_eq!(found["refs"][1]["verses"].as_array().unwrap().len(), 2, "zwei Verse: {found}");
    assert_eq!(found["refs"][1]["verses"][1]["text"], Value::Null, "Ps 23,2 fehlt im Modul: {found}");
    assert_eq!(found["refs"][1]["missing"], 1, "ein Vers fehlt: {found}");

    let hits = ok(&server, "bible.search", json!({ "work": work, "query": "amsel SANG" })).await;
    assert_eq!(hits["hits"][0]["ref"], "Joh 3,16", "Treffer in deutscher Schreibweise: {hits}");
    assert_eq!(hits["bible"]["module"], "Kiesel", "Suche nennt das Modul: {hits}");
    let other = ok(&server, "bible.lookup", json!({ "work": work, "module": "Kiesel", "ref": "Mal 3,24" })).await;
    assert_eq!(other["refs"][0]["verses"][0]["text"], "Ende des erfundenen Alten Testaments.", "module angegeben");

    ok(&server, "bible.set_work", json!({ "work": work, "module": null })).await;
    let refused = err(&server, "bible.lookup", json!({ "work": work, "ref": "Joh 3,16" })).await;
    assert!(refused["message"].as_str().unwrap().contains("Keine Bibel gewählt"), "ehrliche Meldung: {refused}");
}

#[tokio::test]
async fn the_reference_check_compares_quotes_with_the_chosen_bible() {
    let (server, _bibles, work, key) = setup(&[kiesel()]).await;
    let unchecked = ok(&server, "bible.check", json!({ "work": work, "key": key })).await;
    assert!(
        unchecked["refs"].as_array().unwrap().iter().all(|r| r["wording"] == "ungeprüft"),
        "ohne Bibel bleibt alles ungeprüft: {unchecked}"
    );
    assert_eq!(unchecked["bible"], Value::Null, "keine Bibel: {unchecked}");

    ok(&server, "bible.set_work", json!({ "work": work, "module": "Kiesel" })).await;
    let checked = ok(&server, "bible.check", json!({ "work": work, "key": key })).await;
    let refs = checked["refs"].as_array().unwrap();
    let codes: Vec<(&str, &str)> =
        refs.iter().map(|r| (r["normalized"].as_str().unwrap_or("?"), r["wording_code"].as_str().unwrap())).collect();
    assert_eq!(
        codes,
        [
            ("Joh 3,16", "match"),
            ("Joh 3,16", "match"),
            ("Joh 3,16", "differs"),
            ("Ps 23,1", "no_quote"),
            ("Ps 23,2", "missing_verse"),
            ("Ps 151,1", "invalid_ref"),
            ("Ps 23,1", "match"),
        ],
        "je Stelle das Ergebnis (zuletzt das Blockzitat mit Zuschreibung): {checked}"
    );
    assert_eq!(refs[0]["wording"], "Wortlaut stimmt", "deutsch: {checked}");
    assert_eq!(refs[1]["wording"], "Wortlaut stimmt (Teilzitat)", "Teilzitat: {checked}");
    assert_eq!(refs[2]["wording"], "Wortlaut weicht ab", "Abweichung: {checked}");
    assert_eq!(refs[2]["diff_line"], "Der [−Kiesel] [+Stein] lag am Bach", "Wort-Diff: {checked}");
    assert_eq!(refs[2]["quote"], "Der Stein lag am Bach", "das Zitat: {checked}");
    assert!(refs[4]["wording"].as_str().unwrap().contains("Ps 23,2"), "fehlender Vers genannt: {checked}");
    assert_eq!(checked["bible"]["module"], "Kiesel", "Modul genannt: {checked}");
    assert_eq!(checked["title"], "Am Bach", "Abschnitt genannt: {checked}");

    // scripture.check with text takes the Bible of the work; MCP scripture_check with key checks the section.
    let text =
        ok(&server, "scripture.check", json!({ "work": work, "text": "„Der Kiesel lag am Bach“ (Joh 3,16)" })).await;
    assert_eq!(text["refs"][0]["wording_code"], "match", "Teilzitat im Text: {text}");
    let tool = baruch_server::mcp::scripture::call(&server.state, json!({ "work": work, "key": key })).await.unwrap();
    assert_eq!(tool["refs"], checked["refs"], "MCP mit key gibt dasselbe wie bible.check");
}

#[tokio::test]
async fn a_bible_with_english_counting_reports_where_the_versification_does_not_fit() {
    let (server, _bibles, work, _key) = setup(&[english()]).await;
    ok(&server, "bible.set_work", json!({ "work": work, "module": "Englisch" })).await;
    let text = "„Der Kiesel lag am Bach“ (Joh 3,16) und „Ende“ (Mal 3,24).";
    let checked = ok(&server, "scripture.check", json!({ "work": work, "text": text })).await;
    assert_eq!(checked["refs"][0]["wording_code"], "match", "Joh 3 hat in beiden Zählungen 36 Verse: {checked}");
    // Since R17c Baruch reads through the mapping (R17a: Mal 3,19–24 German = Mal 4,1–6 KJV). Without a rule for the
    // reference it stays „Versifikation passt nicht“ with a reason; the expectation follows the mapping itself.
    let kjv = versification("KJV").unwrap();
    match baruch_scripture::versification::mapping::map_range(german(), kjv, "Mal", (3, 24), (3, 24), 400) {
        Ok(_) => {
            assert_eq!(checked["refs"][1]["module_ref"], "Mal 4,6 (KJV)", "gelesen als Mal 4,6: {checked}");
            assert_eq!(checked["refs"][1]["wording_code"], "match", "„Ende“ steht in Mal 4,6 des Moduls: {checked}");
        }
        Err(_) => {
            assert_eq!(
                checked["refs"][1]["wording_code"], "versification_mismatch",
                "Mal 3 unterscheidet sich: {checked}"
            );
            let label = checked["refs"][1]["wording"].as_str().unwrap();
            assert!(label.contains("KJV") && label.contains("Mal 3"), "Meldung nennt Zählung und Kapitel: {label}");
        }
    }
    let looked = ok(&server, "bible.lookup", json!({ "work": work, "ref": "Joh 3,16" })).await;
    assert_eq!(looked["bible"]["versification"], "KJV", "Versifikation genannt: {looked}");
    assert_eq!(looked["bible"]["license"], "keine Lizenzangabe in der conf", "ehrlich ohne Angabe: {looked}");
}

#[tokio::test]
async fn an_unsupported_module_is_listed_but_cannot_be_chosen() {
    let server = common::open().await;
    let bibles = tempfile::tempdir().unwrap();
    let root = modules_dir(bibles.path(), &[kiesel()]);
    let conf = Path::new(&root).join("mods.d/kiesel.conf");
    let text = std::fs::read_to_string(&conf).unwrap();
    std::fs::write(&conf, text.replace("Versification=German", "Versification=Phantasie")).unwrap();
    let listed = ok(&server, "bible.set_dirs", json!({ "dirs": [root] })).await;
    assert_eq!(listed["modules"][0]["supported"], false, "nicht lesbar: {listed}");
    assert!(listed["modules"][0]["problem"].as_str().unwrap().contains("Phantasie"), "Grund genannt: {listed}");
    let work = server.state.default_work().await.unwrap();
    let refused = err(&server, "bible.set_work", json!({ "work": work, "module": "Kiesel" })).await;
    assert_eq!(refused["code"], -32602, "nicht wählbar: {refused}");
}

#[tokio::test]
async fn schrift_gets_the_results_as_facts_and_the_answer_names_the_module() {
    let (server, _bibles, work, _key) = setup(&[kiesel()]).await;
    let section = BOOK.split_once("= Am Bach").unwrap().1;
    let without = baruch_server::scripture::facts_for_work(&server.state, work, "@schrift", section).await.unwrap();
    assert!(without.contains("Wortlaut ungeprüft"), "ohne Bibel ungeprüft: {without}");

    ok(&server, "bible.set_work", json!({ "work": work, "module": "Kiesel" })).await;
    let facts = baruch_server::scripture::facts_for_work(&server.state, work, "@schrift", section).await.unwrap();
    assert!(
        facts.contains("SWORD-Modul Kiesel") && facts.contains("Versifikation German"),
        "Modul und Zählung: {facts}"
    );
    assert!(facts.contains("Lizenz laut conf: Public Domain"), "Lizenzvermerk: {facts}");
    assert!(facts.contains("Joh 3,16: vorhanden; Wortlaut stimmt"), "Ergebnis als Fakt: {facts}");
    assert!(
        facts.contains(
            "Wortlaut weicht ab – Zitat „Der Stein lag am Bach“, Bibel „Der Kiesel lag am Bach, und die Amsel \
                        sang.“, Unterschiede: Der [−Kiesel] [+Stein] lag am Bach"
        ),
        "Abweichung mit geprüftem Vers und Wort-Diff: {facts}"
    );
    assert!(facts.contains("Ps 23,2: vorhanden; Vers fehlt im Modul Kiesel"), "fehlender Vers als Fakt: {facts}");
    assert!(facts.ends_with(FACTS_END_BIBLE), "Schlusssatz verbietet eigenes Bestätigen: {facts}");
    assert!(FACTS_END_BIBLE.contains("nie selbst"), "ausdrückliches Verbot");
    // The complete Bible text appears only for a deviation (the checked verse), otherwise not.
    assert!(!facts.contains("Ein Lied vom stillen Wasser.“, Unterschiede"), "keine Abweichung bei Ps 23,1: {facts}");
    assert_eq!(
        baruch_server::scripture::facts_for_work(&server.state, work, "@lektor", section).await,
        None,
        "nur @schrift/@belege"
    );

    let answer = "- Joh 3,16: Stelle gibt es.".to_owned();
    let marked = baruch_server::scripture::mark_answer_for_work(&server.state, work, "@schrift", section, answer).await;
    assert!(marked.contains(MARK_BIBLE) && marked.contains("Modul Kiesel"), "Hinweis von Baruch: {marked}");
    assert!(marked.contains("Joh 3,16 stimmt"), "Ergebnis je Stelle: {marked}");
    assert!(!marked.contains(MARK_SCRIPTURE), "nicht mehr „keine lokale Bibel“: {marked}");
}

#[tokio::test]
async fn the_mcp_tools_read_bibles_and_name_the_licence() {
    let (server, _bibles, work, _key) = setup(&[kiesel()]).await;
    ok(&server, "bible.set_work", json!({ "work": work, "module": "Kiesel" })).await;
    let call = |name: &'static str, args: Value| {
        let state = server.state.clone();
        async move { baruch_server::mcp::bibles::call(&state, name, args).await }
    };
    let listed = call("bible_modules", json!({ "work": work })).await.unwrap();
    assert_eq!(listed["work_bible"], "Kiesel", "Wahl des Werks: {listed}");
    let looked = call("bible_lookup", json!({ "work": work, "ref": "Joh 3,16" })).await.unwrap();
    assert_eq!(looked["bible"]["license"], "Public Domain; frei erfunden für Tests", "Lizenz: {looked}");
    let hits = call("bible_search", json!({ "work": work, "query": "stillen Wasser", "limit": 5 })).await.unwrap();
    assert_eq!(hits["hits"][0]["ref"], "Ps 23,1", "Suche: {hits}");
    assert!(call("bible_lookup", json!({ "work": work })).await.is_err(), "ref ist Pflicht");
    for name in baruch_server::mcp::bibles::NAMES {
        assert!(!name.contains("set"), "über MCP wird nichts gewählt: {name}");
    }
}
