// SPDX-License-Identifier: AGPL-3.0-or-later
//! Round 39a, task templates_1: the layout, the workspace (editor, tree, revisions, assistants), the outline pages,
//! posts and threads, search and the reference pages (glossary, index, cross references, note cards, marking) speak
//! English and German from the catalogs `locales/{en,de}/{layout,editor,outline,posts,search,reference}.ftl`.
//!
//! Every page is rendered twice through the raw router (`Accept-Language: en` and `de`): `<html lang>` follows the
//! language, no raw key of the six stems is left in the page, the English page has none of a sample of the German
//! wording and the German page still has it (the German texts are the wording before Round 39a).

use crate::common;

use std::collections::BTreeSet;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use baruch_server::i18n::{self, CATALOGS, Lang};
use regex::Regex;

/// The stems of this task (contract §4).
const STEMS: [&str; 6] = ["layout", "editor", "outline", "posts", "search", "reference"];

/// A work with a post on a section and a thread without a section; returns the server, the work and the section key.
async fn fixture() -> (common::Server, i64, i64, i64, i64) {
    let server = common::open().await;
    let report = server.import_garden().await;
    let work = report.work;
    let loaded = server.state.store.load_work(work).await.unwrap();
    let key = loaded.nodes[1].key;
    let seq = server.state.store.node_seq(work, key).await.unwrap();
    let post = baruch_core::Post {
        id: 0,
        created: String::new(),
        author: "Autor".into(),
        body: "Ein Beitrag zum Garten.".into(),
        sources: vec![],
        refs: vec![],
    };
    let post = server.state.store.add_post(work, seq, &post).await.unwrap();
    let (thread, _) = server.state.store.thread_create(work, "Autor", "Ein Faden ohne Abschnitt.").await.unwrap();
    (server, work, key, post, thread.id)
}

/// Page `uri` in `lang` through the raw router (without the German pin of the test harness).
async fn page(server: &common::Server, uri: &str, lang: &str) -> String {
    let raw = baruch_server::router(server.state.clone());
    let request = common::with_lang(Request::get(uri).body(Body::empty()).unwrap(), lang);
    let response = tower::ServiceExt::oneshot(raw, request).await.unwrap();
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let html = String::from_utf8(bytes.to_vec()).unwrap();
    assert_eq!(status, StatusCode::OK, "{uri} in {lang} must render: {html}");
    html
}

/// Raw keys of the six stems (always three or more segments; RPC method names like `outline.move` have two).
fn raw_keys(html: &str) -> Vec<String> {
    let re = Regex::new(r"\b(?:layout|editor|outline|posts|search|reference)\.[a-z0-9_]+\.[a-z0-9_.]+").unwrap();
    re.find_iter(html).map(|m| m.as_str().to_owned()).collect::<BTreeSet<_>>().into_iter().collect()
}

/// Pages of the task with German samples (wording before Round 39a) and their English counterparts.
fn pages(work: i64, key: i64, post: i64, thread: i64) -> Vec<(String, Vec<(&'static str, &'static str)>)> {
    vec![
        (
            format!("/?work={work}&key={key}"),
            vec![
                ("Alle Werke", "All works"),
                ("Als neue Fassung speichern", "Save as new revision"),
                ("Menüband für das Bearbeiten", "Ribbon for editing"),
                ("Fäden ohne Abschnitt", "Threads without a section"),
            ],
        ),
        (
            format!("/gliederung?work={work}"),
            vec![("Anträge der Assistenten", "Requests of the assistants"), ("Höher stufen", "Promote")],
        ),
        (
            format!("/beitraege?work={work}&key={key}"),
            vec![("Zurück zum Abschnitt", "Back to the section"), ("Abschnitts-Chat:", "Section chat:")],
        ),
        (
            format!("/beitrag?work={work}&id={post}"),
            vec![("Text (markieren und kopieren)", "Text (mark and copy)"), ("Nie bearbeitet.", "Never edited.")],
        ),
        (format!("/faeden?work={work}"), vec![("Fäden ohne Abschnitt", "Threads without a section")]),
        (format!("/faden?work={work}&thread={thread}"), vec![("Persona antworten lassen", "Let a persona answer")]),
        (
            format!("/suche?work={work}&q=Garten"),
            vec![("Im Werk suchen und ersetzen", "Search and replace in the work"), ("Textarten", "Kinds of text")],
        ),
        (
            format!("/werke/{work}/glossar"),
            vec![("Kurze Erklärung", "Short explanation"), ("Neuer Begriff", "New term")],
        ),
        (
            format!("/werke/{work}/register"),
            vec![("Register verwalten", "Manage indexes"), ("Register anlegen", "Create index")],
        ),
        (format!("/verweise?work={work}"), vec![("Mögliche Querverweise", "Possible cross references")]),
        (format!("/werke/{work}/stichwortzettel"), vec![("Schriftgröße (Punkt)", "Font size (points)")]),
        (
            format!("/werke/{work}/markierung?key={key}"),
            vec![("Markiertes Wort", "Marked word"), ("Aktionen an der Markierung", "Actions on the selection")],
        ),
        ("/werke/neu".to_owned(), vec![("Neues Werk anlegen", "Create a new work")]),
    ]
}

#[tokio::test]
async fn the_pages_of_the_task_speak_english_and_german_without_raw_keys() {
    let (server, work, key, post, thread) = fixture().await;
    for (uri, samples) in pages(work, key, post, thread) {
        let en = page(&server, &uri, "en").await;
        let de = page(&server, &uri, "de").await;
        assert!(en.contains("<html lang=\"en\""), "{uri}: the English page says so in <html lang>");
        assert!(de.contains("<html lang=\"de\""), "{uri}: the German page says so in <html lang>");
        assert!(raw_keys(&en).is_empty(), "{uri}: no raw catalog key in the English page: {:?}", raw_keys(&en));
        assert!(raw_keys(&de).is_empty(), "{uri}: no raw catalog key in the German page: {:?}", raw_keys(&de));
        for (german, english) in samples {
            assert!(de.contains(german), "{uri}: the German page keeps the wording before Round 39a: {german:?}");
            assert!(!en.contains(german), "{uri}: the English page has no German {german:?}");
            assert!(en.contains(english), "{uri}: the English page has the English text {english:?}");
        }
    }
}

#[tokio::test]
async fn the_badges_keep_their_literal_shape_in_both_languages() {
    let (server, work, key, _, _) = fixture().await;
    // menu::fill_badges replaces the zero directly after the tag; the number must come first in every language.
    for (lang, word) in [("en", "open"), ("de", "offen")] {
        let html = page(&server, &format!("/?work={work}&key={key}"), lang).await;
        assert!(
            html.contains("data-badge=\"open_findings\" title=") && html.contains(&format!(">0 {word}</span>")),
            "{lang}: the findings badge stays „<number> {word}“ so that the layer can fill it"
        );
    }
}

#[test]
fn the_six_catalogs_exist_in_both_languages_with_the_task_keys_only() {
    for stem in STEMS {
        for lang in ["en", "de"] {
            assert!(
                CATALOGS.iter().any(|(code, s, _)| *code == lang && *s == stem),
                "locales/{lang}/{stem}.ftl belongs to templates_1 and must exist"
            );
        }
        let (_, _, source) = CATALOGS.iter().find(|(code, s, _)| *code == "en" && *s == stem).unwrap();
        for (key, _) in i18n::catalog_entries(source) {
            assert!(key.starts_with(&format!("{stem}.")), "{key}: the first segment equals the file stem {stem}");
        }
    }
}

#[test]
fn german_texts_are_the_previous_wording_and_english_is_not_german() {
    // Spot checks of moved texts (byte for byte, with the typographic characters of the templates).
    for (key, german, english) in [
        ("layout.band.new_work_html", "Neues Werk / Importieren…", "New work / Import…"),
        ("editor.page.save_new", "Als neue Fassung speichern", "Save as new revision"),
        (
            "outline.confirm.back_note",
            "„Zurück“ verwirft den Text; er ist dann nicht gespeichert.",
            "“Back” discards the text; it is then not saved.",
        ),
        ("posts.thread.mentioned", "– nur Erwähnte (@…) –", "– only those mentioned (@…) –"),
        ("search.page.hits_other", "Treffer", "matches"),
        ("reference.glossary.lookup_found", "Im Glossar: „{ $word }“", "In the glossary: “{ $word }”"),
    ] {
        let (_, _, de) = CATALOGS.iter().find(|(c, s, _)| *c == "de" && key.starts_with(&format!("{s}."))).unwrap();
        let value = i18n::catalog_entries(de).into_iter().find(|(k, _)| k == key).map(|(_, v)| v);
        assert_eq!(value.as_deref(), Some(german), "{key}: the German catalog keeps the old wording verbatim");
        let (_, _, en) = CATALOGS.iter().find(|(c, s, _)| *c == "en" && key.starts_with(&format!("{s}."))).unwrap();
        let value = i18n::catalog_entries(en).into_iter().find(|(k, _)| k == key).map(|(_, v)| v);
        assert_eq!(value.as_deref(), Some(english), "{key}: the English base catalog has natural English");
    }
    // A plural kept as two keys renders the number in both languages.
    assert_eq!(i18n::t(Lang::De, "posts.chat.count_one", &[("count", 1.into())]), "1 Beitrag", "German singular");
    assert_eq!(i18n::t(Lang::En, "posts.chat.count_other", &[("count", 3.into())]), "3 posts", "English plural");
    // No German umlaut or sharp s in the English values of the six stems (proper names and quotations aside).
    let umlaut = Regex::new("[äöüÄÖÜß„]").unwrap();
    for (code, stem, source) in CATALOGS {
        if *code != "en" || !STEMS.contains(stem) {
            continue;
        }
        for (key, value) in i18n::catalog_entries(source) {
            assert!(!umlaut.is_match(&value), "{key}: the English text must not contain German letters: {value:?}");
        }
    }
}
