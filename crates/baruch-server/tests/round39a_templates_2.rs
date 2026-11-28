// SPDX-License-Identifier: AGPL-3.0-or-later
//! Round 39a (task templates_2): the pages of the works, ideas and mind map, findings, cover, images and the
//! export/import pages render in English and in German. Each page is fetched twice (Accept-Language en and de):
//! `<html lang>` follows, no raw key of the six stems (works, ideas, findings, cover, assets, export) is left, a
//! sample of the former German wording appears only in German and its English text only in English.

use crate::common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use baruch_server::i18n::{self, Arg, CATALOGS, Lang};
use baruch_store::{FindingKind, NewFinding};
use serde_json::{Value, json};

/// The stems owned by templates_2 (contract §4).
const STEMS: [&str; 6] = ["works", "ideas", "findings", "cover", "assets", "export"];

const HOUSE: &str =
    "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"8\" height=\"8\"><path d=\"M0 8 L4 0 L8 8 Z\"/></svg>\n";

fn result(answer: &Value) -> &Value {
    assert!(answer["error"].is_null(), "RPC error: {answer}");
    &answer["result"]
}

/// All English keys of the six stems.
fn own_keys() -> Vec<String> {
    let mut keys = Vec::new();
    for (code, stem, source) in CATALOGS.iter() {
        if *code == "en" && STEMS.contains(stem) {
            keys.extend(i18n::catalog_entries(source).into_iter().map(|(key, _)| key));
        }
    }
    assert!(keys.len() > 600, "the six area files carry the texts of 29 templates, found only {} keys", keys.len());
    keys
}

async fn page(server: &common::Server, uri: &str, lang: &str) -> String {
    let request = common::with_lang(Request::get(uri).body(Body::empty()).unwrap(), lang);
    let (status, _, html) = server.send(request).await;
    assert_eq!(status, StatusCode::OK, "{uri} in {lang} must load: {html}");
    html
}

/// One page in both languages with a German sample and its English counterpart.
struct Case {
    uri: String,
    de: &'static str,
    en: &'static str,
}

fn case(uri: impl Into<String>, de: &'static str, en: &'static str) -> Case {
    Case { uri: uri.into(), de, en }
}

/// Garden book with an image, an idea and a finding: every page of templates_2 has something to show.
async fn book(server: &common::Server) -> (i64, String, i64, i64) {
    let work = server.import_garden().await.work;
    let nodes = server.rpc("work.get", json!({ "work": work })).await;
    let key = result(&nodes)["work"]["nodes"][1]["key"].as_i64().unwrap();
    let added = server
        .rpc(
            "asset.add",
            json!({ "work": work, "file_name": "Haus.svg", "content": HOUSE, "encoding": "svg",
                    "path": "/bilder/haus.svg", "caption": "Das Haus", "alt": "Ein Haus mit Dach" }),
        )
        .await;
    let asset = result(&added)["name"].as_str().unwrap().to_owned();
    let idea = server.rpc("idea.create", json!({ "work": work, "key": key, "words": "Laube", "note": "Rosen." })).await;
    let idea = result(&idea)["idea"]["id"].as_i64().unwrap();
    let finding = NewFinding {
        work,
        key: Some(key),
        post: None,
        thread_post: None,
        position: 0,
        persona: "@lektor",
        kind: FindingKind::Critical,
        body: "Der Satz ist zu lang.",
        quote: "",
        quote_check: "",
        similar_to: None,
        summary_of: &[],
    };
    let finding = server.state.store.finding_insert(&finding).await.expect("a finding can be created").id;
    (work, asset, idea, finding)
}

#[tokio::test]
async fn the_pages_of_works_ideas_findings_cover_images_and_exports_speak_english_and_german() {
    // The cover and EPUB pages typeset the work: the mark „PDF entsteht“ is process-wide (SKILLS.md, tests).
    let _serial = common::PDF_SERIAL.lock().await;
    let server = common::open().await;
    let (work, asset, idea, finding) = book(&server).await;
    let keys = own_keys();
    let cases = [
        case("/werke", "Neues Werk oder Import…", "New work or import…"),
        case("/werke/archiv", "Archiv der gelöschten Werke", "Archive of deleted works"),
        case(format!("/werke/{work}/bearbeiten"), "Fassungen der Werkvorlage", "Revisions of the work template"),
        case(format!("/werke/{work}/loeschen"), "Endgültig löschen", "Delete for good"),
        case(format!("/werke/{work}/nummerierung"), "Mustersprache:", "Pattern language:"),
        case(format!("/einfaelle?work={work}"), "Neuer Einfall", "New idea"),
        case(format!("/einfaelle/{idea}"), "Platz in der Mindmap", "Place in the mind map"),
        case(format!("/werke/{work}/karte"), "Legende der Beziehungen", "Legend of the relations"),
        case(format!("/werke/{work}/karte/gliederung"), "Gelöschte Einfälle", "Deleted ideas"),
        case(format!("/werke/{work}/karte/einlesen"), "Übernommen werden Titel", "Titles, notes, keywords"),
        case(format!("/werke/{work}/befunde"), "Matrix: Kapitel × Persona", "Matrix: chapter × persona"),
        case(format!("/werke/{work}/befunde?befund={finding}"), "Status setzen …", "Set status …"),
        case(format!("/werke/{work}/umschlag"), "Druckprüfung", "Print check"),
        case(format!("/werke/{work}/bilder"), "Bild hinzufügen", "Add image"),
        case(format!("/werke/{work}/bilder/{asset}"), "Ersetzen (neue Fassung)", "Replace (new revision)"),
        case(
            format!("/werke/{work}/austausch"),
            "Nicht in der Datei: Schlüssel und Token",
            "Not in the file: keys and tokens",
        ),
        case("/austausch/einlesen", "Austauschdatei wählen", "Choose an exchange file"),
        case(format!("/werke/{work}/word"), "Word-Datei herunterladen", "Download Word file"),
        case("/werke/import/word", "Typst oder Markdown?", "Typst or Markdown?"),
        case(format!("/werke/{work}/epub/vorschau"), "EPUB-Vorschau je Reader", "EPUB preview per reader"),
        case(format!("/werke/{work}/epub/pruefung"), "EPUBCheck ausführen", "Run EPUBCheck"),
    ];
    for c in &cases {
        let en = page(&server, &c.uri, "en").await;
        let de = page(&server, &c.uri, "de").await;
        assert!(en.contains("<html lang=\"en\""), "{}: the English page says lang=\"en\"", c.uri);
        assert!(de.contains("<html lang=\"de\""), "{}: the German page says lang=\"de\"", c.uri);
        assert!(de.contains(c.de), "{}: the German wording {:?} stays as before: {de}", c.uri, c.de);
        assert!(!en.contains(c.de), "{}: the German wording {:?} must not appear in English: {en}", c.uri, c.de);
        assert!(en.contains(c.en), "{}: the English text {:?} replaces it: {en}", c.uri, c.en);
        assert!(!de.contains(c.en), "{}: the English text {:?} must not appear in German", c.uri, c.en);
        for key in &keys {
            assert!(!en.contains(key.as_str()), "{}: raw key {key} in the English page (missing text?)", c.uri);
            assert!(!de.contains(key.as_str()), "{}: raw key {key} in the German page (missing text?)", c.uri);
        }
    }
}

#[tokio::test]
async fn the_findings_panel_of_the_work_page_follows_the_language() {
    let server = common::open().await;
    let (work, _, _, _) = book(&server).await;
    let nodes = server.rpc("work.get", json!({ "work": work })).await;
    let key = result(&nodes)["work"]["nodes"][1]["key"].as_i64().unwrap();
    let uri = format!("/?work={work}&key={key}");
    let en = page(&server, &uri, "en").await;
    let de = page(&server, &uri, "de").await;
    assert!(
        de.contains("Alle Befunde dieses Abschnitts") && de.contains("Lösungen umsetzen"),
        "the accordion „Befunde“ keeps its German wording: {de}"
    );
    assert!(
        en.contains("All findings of this section") && en.contains("Apply solutions"),
        "the accordion speaks English on an English page: {en}"
    );
    assert!(!en.contains("Alle Befunde dieses Abschnitts"), "no German accordion text in English");
}

#[test]
fn plural_texts_choose_the_form_per_language() {
    let args = |nodes: i64, images: i64| vec![("nodes", Arg::from(nodes)), ("images", Arg::from(images))];
    assert_eq!(
        i18n::t(Lang::En, "works.import.taken_over", &args(1, 2)),
        "1 section, 2 images taken over.",
        "English uses the singular for one and the plural for two"
    );
    assert_eq!(
        i18n::t(Lang::De, "works.import.taken_over", &args(3, 1)),
        "3 Abschnitte, 1 Bild übernommen.",
        "German keeps the wording of the former template conditionals"
    );
    assert_eq!(
        i18n::t(Lang::De, "export.typeset.page_count", &[("count", Arg::from(1))]),
        "1 Seite",
        "one typeset page is „Seite“"
    );
    assert_eq!(
        i18n::t(Lang::En, "export.typeset.page_count", &[("count", Arg::from(12))]),
        "12 pages",
        "twelve pages are plural in English"
    );
}

#[test]
fn german_texts_keep_the_former_wording_verbatim() {
    // Spot checks of wording that existing tests and the user know (contract §4: de byte for byte).
    for (key, german) in [
        ("works.list.new", "Neues Werk oder Import…"),
        ("works.delete.submit", "Endgültig löschen"),
        (
            "ideas.idea.delete_confirm",
            "Ja, diesen Einfall löschen (wiederherstellbar unter „Gelöschte“ und über den Git-Spiegel)",
        ),
        ("findings.card.question_default", "Befund { $id } scheint erledigt – als erledigt markieren?"),
        ("cover.book.paper", "Papierdicke je Blatt in mm (1 Blatt = 2 Seiten)"),
        ("assets.requests.confirm", "Bestätigen"),
        ("export.exchange.heading", "Austausch: Ausgeben"),
    ] {
        let raw = CATALOGS
            .iter()
            .filter(|(code, _, _)| *code == "de")
            .flat_map(|(_, _, source)| i18n::catalog_entries(source))
            .find(|(k, _)| k == key)
            .map(|(_, value)| value);
        assert_eq!(raw.as_deref(), Some(german), "{key}: the German catalog keeps the former wording");
    }
}
