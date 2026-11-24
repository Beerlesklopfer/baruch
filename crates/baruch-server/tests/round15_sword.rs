// SPDX-License-Identifier: AGPL-3.0-or-later
//! Runde 15, Punkte 5a–5c (Zweig anbieter): SWORD-Kommentare und -Lexika als Recherchewerkzeug im Server –
//! `bible.modules` mit `kind`, Auswahl je Werk (RPC und Formular ohne JavaScript), `commentary.*`, `lexicon.*`,
//! MCP `commentary_lookup`/`lexicon_lookup`/`lexicon_search` (nur lesend), die HTML-Bausteine für die Literatur
//! (maskiert, `<details>`, Links mit Anfragerest) und `bible_research::research` (Reihenfolge, Lizenz,
//! Grenzen, keine erfundenen Stellen, Schalter).
//!
//! Alle Module schreibt der Test selbst (`baruch_sword::fixture`) mit frei erfundenem Inhalt in einen
//! Temp-Ordner; die echten Modulordner des Rechners werden nie gelesen (jeder Test setzt seine Ordner).

use crate::common;

use std::path::Path;

use baruch_scripture::versification::german;
use baruch_server::bible_research::{NoteKind, research};
use baruch_server::bibles::{commentary_hits_html, commentary_panel_html};
use baruch_sword::fixture::{Block, Fixture, Layout, LexLayout, LexiconFixture, write, write_lexicon};
use serde_json::{Value, json};

const BOOK: &str = "#set document(title: \"Der Turm im Moor\", author: \"Ole Riedel\")\n\n\
= Prüfung\n\nWie Ps 139,23 sagt, steht der Kiesel am Bach. Dazu Mt 5,3 und die Amsel. Ungültig: Ps 151,1.\n";

/// Erfundene Testbibel (deutsche Zählung).
fn bible() -> Fixture {
    Fixture::new("Moorbibel", Layout::ZText(Block::Book), german())
        .conf("Description", "Erfundene Moorbibel")
        .conf("DistributionLicense", "Public Domain")
        .verse("Ps", 139, 23, "Erforsche den Turm im Moor und kenne den Weg.")
        .verse("Matt", 5, 3, "Selig sind die Kiesel am Bach.")
}

/// Erfundener Kommentar; ein Eintrag trägt kodiertes Markup, das nach dem Auflösen wie ein Skript aussieht.
fn commentary() -> Fixture {
    Fixture::new("Moorkommentar", Layout::RawCom, german())
        .conf("Description", "Erfundener Moorkommentar")
        .conf("DistributionLicense", "Copyrighted; Free non-commercial distribution")
        .verse("Ps", 139, 23, "Prüfen heißt: ans Licht halten. &lt;script&gt;alert(1)&lt;/script&gt;")
        .verse("Matt", 5, 3, "Die Kiesel stehen für die Kleinen.")
}

fn lexicon() -> LexiconFixture {
    LexiconFixture::new("Moorlexikon", LexLayout::ZLD)
        .conf("Description", "Erfundenes Moorlexikon")
        .conf("DistributionLicense", "Public Domain")
        .article("Kiesel", "Kleiner Stein am Bach.")
        .article("Amsel", "Schwarzer Vogel, singt am Abend.")
        .article("Turm", "Hohes Bauwerk im Moor.")
}

fn modules_dir(dir: &Path) -> String {
    let root = dir.join("sword");
    write(&root, &bible()).unwrap();
    write(&root, &commentary()).unwrap();
    write_lexicon(&root, &lexicon()).unwrap();
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

/// Server mit eigenem Modulordner und einem Werk; gibt Server, Ordner und Werk.
async fn setup() -> (common::Server, tempfile::TempDir, i64) {
    let server = common::open().await;
    let modules = tempfile::tempdir().unwrap();
    let root = modules_dir(modules.path());
    ok(&server, "bible.set_dirs", json!({ "dirs": [root] })).await;
    let work = server.import(BOOK, "turm.typ").await.work;
    (server, modules, work)
}

/// Werk mit Bibel, Kommentar und Lexikon.
async fn chosen() -> (common::Server, tempfile::TempDir, i64) {
    let (server, modules, work) = setup().await;
    ok(&server, "bible.set_work", json!({ "work": work, "module": "Moorbibel" })).await;
    ok(
        &server,
        "bible.set_work_sources",
        json!({ "work": work, "commentaries": ["Moorkommentar"], "lexicons": ["Moorlexikon"] }),
    )
    .await;
    (server, modules, work)
}

#[tokio::test]
async fn bible_modules_names_the_kind_and_the_bible_choice_stays_with_bibles() {
    let (server, _modules, work) = setup().await;
    let listed = ok(&server, "bible.modules", json!({ "work": work })).await;
    let kinds: Vec<(String, String)> = listed["modules"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| (m["module"].as_str().unwrap().to_owned(), m["kind"].as_str().unwrap().to_owned()))
        .collect();
    assert!(kinds.contains(&("Moorbibel".into(), "bible".into())), "Bibel mit kind bible: {kinds:?}");
    assert!(kinds.contains(&("Moorkommentar".into(), "commentary".into())), "Kommentar mit kind: {kinds:?}");
    assert!(kinds.contains(&("Moorlexikon".into(), "lexicon".into())), "Lexikon mit kind: {kinds:?}");
    let refused = err(&server, "bible.set_work", json!({ "work": work, "module": "Moorkommentar" })).await;
    assert_eq!(refused["code"], -32602, "ein Kommentar ist keine Bibel des Werks: {refused}");
    let (_, html) = server.get(&format!("/einstellungen/quellen?work={work}")).await;
    let select = &html[html.find("name=\"module\"").unwrap()..];
    let select = &select[..select.find("</select>").unwrap()];
    assert!(select.contains("Moorbibel") && !select.contains("Moorkommentar"), "Bibelwahl nur mit Bibeln: {select}");
    assert!(html.contains("data-kind=\"commentary\""), "Tabelle nennt die Art");
}

#[tokio::test]
async fn commentaries_and_lexicons_are_chosen_per_work_without_javascript() {
    let (server, _modules, work) = setup().await;
    let (_, html) = server.get(&format!("/einstellungen/werk?work={work}")).await;
    assert!(html.contains("action=\"/einstellungen/werk/bibeln\""), "Formular der Seite Werk: {html}");
    assert!(html.contains("data-commentary-choice=\"Moorkommentar\""), "Kästchen je Kommentar");
    assert!(html.contains("data-lexicon-choice=\"Moorlexikon\""), "Kästchen je Lexikon");
    let form = format!(
        "method=bible.set_work_sources&work={work}&commentary=Moorkommentar&lexicon=Moorlexikon&lexicon=Moorlexikon"
    );
    let (status, location) = server.post_form("/einstellungen/werk/bibeln", &form).await;
    assert_eq!(status, 303, "Weiterleitung nach dem Speichern");
    assert_eq!(
        location.as_deref(),
        Some(format!("/einstellungen/werk?work={work}&ok=kommentare#kommentare").as_str()),
        "fester Code"
    );
    let sources = ok(&server, "bible.work_sources", json!({ "work": work })).await;
    assert_eq!(sources["commentaries"], json!(["Moorkommentar"]), "gewählt: {sources}");
    assert_eq!(sources["lexicons"], json!(["Moorlexikon"]), "doppelt angehakt zählt einmal: {sources}");
    let (_, html) = server.get(&format!("/einstellungen/werk?work={work}&ok=kommentare")).await;
    assert!(html.contains("value=\"Moorkommentar\" checked"), "angehakt nach dem Speichern");
    let (_, location) = server
        .post_form("/einstellungen/werk/bibeln", &format!("method=bible.set_work_sources&work={work}&commentary=Fehlt"))
        .await;
    assert!(location.unwrap().contains("kommentarfehler=1"), "unbekanntes Modul abgelehnt");
    let refused = err(&server, "bible.set_work_sources", json!({ "work": work, "commentaries": ["Moorbibel"] })).await;
    assert_eq!(refused["code"], -32602, "eine Bibel ist kein Kommentar: {refused}");
    assert_eq!(
        ok(&server, "bible.work_sources", json!({ "work": work })).await["commentaries"],
        json!(["Moorkommentar"]),
        "Auswahl bleibt nach Ablehnung"
    );
    for name in baruch_server::mcp::bibles::NAMES {
        assert!(!name.contains("set"), "über MCP wird nichts gewählt: {name}");
    }
}

#[tokio::test]
async fn rpc_reads_commentaries_and_lexicons_with_their_licence() {
    let (server, _modules, work) = chosen().await;
    let looked = ok(&server, "commentary.lookup", json!({ "work": work, "ref": "Ps 139,23" })).await;
    let first = &looked["commentaries"][0];
    assert_eq!(first["module"], "Moorkommentar", "Kommentar der Werk-Auswahl: {looked}");
    assert_eq!(first["notice"], "Copyrighted; Free non-commercial distribution", "Lizenzvermerk: {first}");
    assert_eq!(first["entries"][0]["ref"], "Ps 139,23", "deutsche Stellenangabe: {first}");
    assert!(first["entries"][0]["text"].as_str().unwrap().starts_with("Prüfen heißt"), "Klartext: {first}");
    let none = ok(&server, "commentary.lookup", json!({ "work": work, "ref": "Ps 1,1" })).await;
    assert!(none["commentaries"][0]["problem"].as_str().unwrap().contains("keinen Eintrag"), "ehrlich leer: {none}");
    let hits = ok(&server, "commentary.search", json!({ "work": work, "query": "Kleinen" })).await;
    assert_eq!(hits["commentaries"][0]["hits"][0]["ref"], "Mt 5,3", "Suche im Kommentar: {hits}");
    let article = ok(&server, "lexicon.lookup", json!({ "work": work, "key": "amsel" })).await;
    assert_eq!(
        (article["found"].as_bool(), article["text"].as_str(), article["notice"].as_str()),
        (Some(true), Some("Schwarzer Vogel, singt am Abend."), Some("Public Domain")),
        "Stichwort → Artikel mit Lizenz: {article}"
    );
    let missing = ok(&server, "lexicon.lookup", json!({ "work": work, "key": "Elefant" })).await;
    assert_eq!(missing["found"], false, "unbekanntes Stichwort: nichts erfunden: {missing}");
    let keys = ok(&server, "lexicon.keys", json!({ "work": work, "prefix": "k" })).await;
    assert_eq!(keys["keys"], json!(["KIESEL"]), "Stichwortliste mit Präfix: {keys}");
    let found = ok(&server, "lexicon.search", json!({ "work": work, "query": "Moor" })).await;
    assert_eq!(found["hits"][0]["key"], "TURM", "Suche im Artikeltext: {found}");
    let other = ok(&server, "commentary.lookup", json!({ "work": work, "ref": "Ps 139,23", "module": "Fehlt" })).await;
    assert!(other["commentaries"][0]["problem"].as_str().unwrap().contains("gefunden"), "fehlendes Modul: {other}");
}

#[tokio::test]
async fn mcp_offers_three_read_only_tools_naming_the_licence() {
    let (server, _modules, work) = chosen().await;
    let call = |name: &'static str, args: Value| {
        let state = server.state.clone();
        async move { baruch_server::mcp::bibles::call(&state, name, args).await }
    };
    let looked = call("commentary_lookup", json!({ "work": work, "ref": "Mt 5,3" })).await.unwrap();
    assert_eq!(looked["commentaries"][0]["entries"][0]["text"], "Die Kiesel stehen für die Kleinen.", "{looked}");
    let article = call("lexicon_lookup", json!({ "work": work, "key": "Turm" })).await.unwrap();
    assert_eq!(article["notice"], "Public Domain", "Lizenzvermerk in der Antwort: {article}");
    let hits = call("lexicon_search", json!({ "work": work, "query": "Bach", "limit": 3 })).await.unwrap();
    assert_eq!(hits["hits"][0]["key"], "KIESEL", "{hits}");
    assert!(call("lexicon_lookup", json!({ "work": work })).await.is_err(), "key ist Pflicht");
    for (name, description) in [
        ("commentary_lookup", baruch_server::mcp::bibles::COMMENTARY_LOOKUP_DESCRIPTION),
        ("lexicon_lookup", baruch_server::mcp::bibles::LEXICON_LOOKUP_DESCRIPTION),
        ("lexicon_search", baruch_server::mcp::bibles::LEXICON_SEARCH_DESCRIPTION),
    ] {
        assert!(description.contains("nur lesend") && description.contains("Lizenzvermerk"), "{name}: {description}");
        assert!(baruch_server::mcp::tools::TOOLS.iter().any(|t| t.name == name), "{name} steht in der Werkzeugliste");
    }
}

#[tokio::test]
async fn the_commentary_panel_is_escaped_html_that_folds_without_javascript() {
    let (server, _modules, work) = chosen().await;
    let html = commentary_panel_html(&server.state, work, "Ps 139,23").await;
    assert!(
        html.starts_with("<details class=\"commentaries\" data-widget=\"commentaries\">")
            && html.ends_with("</details>"),
        "ein-/ausklappbar ohne JavaScript: {html}"
    );
    assert!(html.contains("&lt;script&gt;alert(1)&lt;/script&gt;"), "Modultext maskiert: {html}");
    assert!(!html.contains("<script"), "kein ausführbares Skript aus dem Modul: {html}");
    assert!(html.contains("Moorkommentar") && html.contains("Lizenz: Copyrighted"), "Modulname und Lizenz: {html}");
    assert!(commentary_panel_html(&server.state, work, "Ps 1,1").await.is_empty(), "ohne Eintrag leer");
    let (bare, _modules2, other) = setup().await;
    assert!(commentary_panel_html(&bare.state, other, "Ps 139,23").await.is_empty(), "ohne Auswahl leer");
}

#[tokio::test]
async fn commentary_hits_link_to_the_bible_page_with_the_given_tail() {
    let (server, _modules, work) = chosen().await;
    let started = std::time::Instant::now();
    let html = commentary_hits_html(&server.state, work, "Licht", "&key=7&lq=Licht").await;
    assert!(started.elapsed() < std::time::Duration::from_secs(4), "Zeitgrenze 3 s (plus Rand) eingehalten");
    assert!(html.contains("data-hit-group=\"commentary\"") && html.contains("Kommentar · Moorkommentar"), "{html}");
    assert!(
        html.contains(&format!("href=\"/literatur/bibel?work={work}&amp;ref=Ps%20139%2C23&amp;kom=1&key=7&lq=Licht\"")),
        "Link nach /literatur/bibel mit Stelle, kom=1 und Anfragerest: {html}"
    );
    assert!(!html.contains("<script"), "auch der Ausschnitt ist maskiert: {html}");
    assert!(commentary_hits_html(&server.state, work, "Elefant", "").await.is_empty(), "ohne Treffer leer");
}

#[tokio::test]
async fn research_gives_bible_then_commentary_per_reference_and_lexicons_last() {
    let (server, _modules, work) = chosen().await;
    let found = research(&server.state, work, "@belege", BOOK, 10_000).await.expect("@belege bekommt Belege");
    let order: Vec<(NoteKind, &str)> = found.notes.iter().map(|n| (n.kind, n.reference.as_str())).collect();
    assert_eq!(
        order,
        vec![
            (NoteKind::Bible, "Ps 139,23"),
            (NoteKind::Commentary, "Ps 139,23"),
            (NoteKind::Bible, "Mt 5,3"),
            (NoteKind::Commentary, "Mt 5,3"),
            (NoteKind::Lexicon, "TURM"),
            (NoteKind::Lexicon, "KIESEL"),
            (NoteKind::Lexicon, "AMSEL"),
        ],
        "Stellen in Textfolge, je Stelle Bibel vor Kommentar, Lexika zuletzt (Stichwörter in Textfolge)"
    );
    assert_eq!(found.notes[0].text, "Erforsche den Turm im Moor und kenne den Weg.", "Wortlaut der Werk-Bibel");
    assert_eq!(found.notes[0].notice, "Public Domain", "Lizenzvermerk am Beleg");
    assert_eq!(found.notes[1].module, "Moorkommentar", "Modul am Beleg");
    assert!(
        found.problems.iter().any(|p| p.contains("Ps 151,1")),
        "ungültige Stelle als Hinweis: {:?}",
        found.problems
    );
    assert!(!found.notes.iter().any(|n| n.reference.contains("151")), "für die ungültige Stelle kein Text");

    let small = research(&server.state, work, "@belege", BOOK, 120).await.unwrap();
    let total: usize = small.notes.iter().map(|n| n.text.chars().count()).sum();
    assert!(total <= 120, "max_chars eingehalten: {total}");
    assert!(
        small.problems.iter().any(|p| p.contains("Grenze erreicht")),
        "Grenze ehrlich genannt: {:?}",
        small.problems
    );

    assert_eq!(research(&server.state, work, "@lektor", BOOK, 10_000).await, None, "Lektor: Vorgabe aus");
    ok(&server, "persona.bible_set", json!({ "persona": "@lektor", "on": true })).await;
    assert!(research(&server.state, work, "@lektor", BOOK, 10_000).await.is_some(), "eingeschaltet: Belege");
    assert_eq!(research(&server.state, work, "@hoerer", BOOK, 10_000).await, None, "andere Personas nie");
    ok(&server, "persona.bible_set", json!({ "persona": "@belege", "on": false })).await;
    assert_eq!(research(&server.state, work, "@belege", BOOK, 10_000).await, None, "Schalter aus: kein Block");
    let (bare, _m, other) = setup().await;
    assert_eq!(research(&bare.state, other, "@belege", BOOK, 10_000).await, None, "nichts gewählt: kein Block");
}
