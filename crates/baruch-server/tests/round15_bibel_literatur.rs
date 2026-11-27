// SPDX-License-Identifier: AGPL-3.0-or-later
//! Round 15 „auftraege“, user wish 2026-11-07 „Bibeln in der Literatur“: box „Bibeln“ (popup and `/literatur`), page
//! `/literatur/bibel` (paging through book/chapter, opening a passage, verbatim search), hit group „Bibel“ in the
//! shared search field, quotation as a proposal revision with passage reference and „an Persona geben“ – everything
//! without JavaScript.
//!
//! The Bibles are written by the test itself (`baruch_sword::fixture`) with freely invented text into a temp folder,
//! and it sets the module folders through `bible.set_dirs`; the real module folders of the machine are never read.
//! Model: `ScriptedModel` (no network, no Eliezer).

use crate::common;

use std::path::Path;
use std::sync::Arc;

use baruch_scripture::versification::{german, versification};
use baruch_server::assist::mock::{ScriptedModel, Step};
use baruch_sword::fixture::{Block, Fixture, Layout, write};
use serde_json::{Value, json};

const BOOK: &str = "#set document(title: \"Das Kieselbuch\", author: \"Lena Fluss\")\n\n\
= Am Bach\n\nDer Bach rauscht leise.\n\n= Am Ufer\n\nDas Ufer ist still.\n";

/// Invented test Bible in German versification.
fn kiesel() -> Fixture {
    Fixture::new("Kiesel", Layout::ZText(Block::Book), german())
        .conf("Description", "Erfundene Kieselbibel")
        .conf("DistributionLicense", "Public Domain")
        .conf("Copyright", "frei erfunden für Tests")
        .verse("John", 3, 16, "Der Kiesel lag am Bach, und die Amsel sang.")
        .verse("Ps", 139, 1, "Der Bach kennt jeden Stein.")
        .verse("Ps", 139, 23, "Prüfe den Kiesel und sieh, wie er glänzt.")
        .verse("Ps", 139, 24, "Und führe ihn den Bach hinab.")
}

/// Second invented Bible (English versification).
fn englisch() -> Fixture {
    Fixture::new("Englisch", Layout::RawText, versification("KJV").unwrap())
        .conf("Description", "Erfundene Bibel mit KJV-Zählung")
        .verse("John", 3, 16, "Die Amsel sang am Kiesel.")
}

async fn ok(server: &common::Server, method: &str, params: Value) -> Value {
    let answer = server.rpc(method, params).await;
    assert!(answer["error"].is_null(), "{method} sollte gelingen: {answer}");
    answer["result"].clone()
}

struct Shelf {
    server: common::Server,
    model: ScriptedModel,
    _bibles: tempfile::TempDir,
    work: i64,
    key: i64,
}

/// Server with `ScriptedModel`, own test Bibles (without modules: no folders set) and the Kieselbuch.
async fn shelf(modules: &[Fixture], steps: Vec<Step>) -> Shelf {
    let model = ScriptedModel::with(steps);
    let data = tempfile::tempdir().unwrap();
    let state = baruch_server::AppState::open_with(
        data.path(),
        Arc::new(baruch_server::connections::MemorySecrets::default()),
        Arc::new(model.clone()),
    )
    .await
    .unwrap();
    let state = Arc::new(state);
    let server = common::Server { app: baruch_server::router(state.clone()), state, data };
    let bibles = tempfile::tempdir().unwrap();
    if !modules.is_empty() {
        let root: &Path = &bibles.path().join("sword");
        for m in modules {
            write(root, m).unwrap();
        }
        ok(&server, "bible.set_dirs", json!({ "dirs": [root.display().to_string()] })).await;
    }
    let work = server.import(BOOK, "kiesel.typ").await.work;
    let key = server.state.store.load_work(work).await.unwrap().nodes[0].key;
    Shelf { server, model, _bibles: bibles, work, key }
}

#[tokio::test]
async fn the_bible_box_lists_the_bibles_with_the_work_bible_marked_and_first() {
    let s = shelf(&[kiesel(), englisch()], vec![]).await;
    ok(&s.server, "bible.set_work", json!({ "work": s.work, "module": "Kiesel" })).await;
    let (status, page) = s.server.get(&format!("/literatur?work={}&key={}", s.work, s.key)).await;
    assert_eq!(status, 200, "Literatur ohne JavaScript: {page}");
    let box_at = page.find("class=\"literature-bibles\"").unwrap_or_else(|| panic!("Kasten „Bibeln“ fehlt: {page}"));
    let rest = &page[box_at..];
    let kiesel = rest.find("data-module=\"Kiesel\"").expect("Bibel des Werks im Kasten");
    let english = rest.find("data-module=\"Englisch\"").expect("zweite Bibel im Kasten");
    assert!(kiesel < english, "die Bibel des Werks steht zuerst (obwohl „Englisch“ alphabetisch vorn läge)");
    assert!(rest.contains("Bibel des Werks"), "die Bibel des Werks ist markiert: {rest}");
    assert!(
        rest.contains(&format!("href=\"/literatur/bibel?work={}&amp;module=Kiesel&amp;key={}\"", s.work, s.key)),
        "Link auf die Bibelseite mit Werk, Modul und Abschnitt: {rest}"
    );
    let popup = ok(&s.server, "literature.render", json!({ "work": s.work, "key": s.key })).await;
    assert!(popup["html"].as_str().unwrap().contains("data-module=\"Kiesel\""), "derselbe Kasten im Popup: {popup}");
}

#[tokio::test]
async fn without_modules_the_box_and_the_page_say_so_and_link_to_the_settings() {
    let s = shelf(&[], vec![]).await;
    let (_, page) = s.server.get(&format!("/literatur?work={}", s.work)).await;
    assert!(page.contains("data-bibles=\"none\""), "ehrlicher Hinweis statt einer Liste: {page}");
    assert!(page.contains("href=\"/einstellungen/quellen\""), "Link zu Einstellungen → Quellen: {page}");
    let (status, bible) = s.server.get(&format!("/literatur/bibel?work={}", s.work)).await;
    assert_eq!(status, 200, "die Bibelseite antwortet auch ohne Bibel: {bible}");
    assert!(bible.contains("data-bible=\"none\"") && bible.contains("Keine SWORD-Bibel"), "Hinweis: {bible}");
    assert!(bible.contains("href=\"/einstellungen/quellen\""), "Link zu den Einstellungen: {bible}");
}

#[tokio::test]
async fn the_bible_page_browses_looks_up_and_searches_without_javascript() {
    let s = shelf(&[kiesel()], vec![]).await;
    let base = format!("/literatur/bibel?work={}&key={}&module=Kiesel", s.work, s.key);
    let (status, books) = s.server.get(&base).await;
    assert_eq!(status, 200, "Bibel öffnen: {books}");
    assert!(
        books.contains("Erfundene Kieselbibel") && books.contains("Public Domain"),
        "Beschreibung und Lizenz: {books}"
    );
    assert!(books.contains("<option value=\"Ps\">"), "Bücherwahl als Formular ohne JavaScript: {books}");

    let (_, chapter) = s.server.get(&format!("{base}&book=Ps&chapter=139")).await;
    assert!(chapter.contains("Der Bach kennt jeden Stein."), "Kapitel aufgeschlagen: {chapter}");
    assert!(
        chapter.contains("data-chapter-link=\"prev\"") && chapter.contains("chapter=138"),
        "voriges Kapitel: {chapter}"
    );
    assert!(
        chapter.contains("data-chapter-link=\"next\"") && chapter.contains("chapter=140"),
        "nächstes Kapitel: {chapter}"
    );

    let (_, verses) = s.server.get(&format!("{base}&ref=Ps+139%2C23-24")).await;
    assert!(verses.contains("Prüfe den Kiesel und sieh, wie er glänzt."), "Vers 23: {verses}");
    assert!(verses.contains("Und führe ihn den Bach hinab."), "Vers 24: {verses}");
    assert!(!verses.contains("Der Bach kennt jeden Stein."), "nur die Stelle, nicht das ganze Kapitel");
    assert!(verses.contains("value=\"Ps 139,23–24\""), "die Stelle steht normalisiert im Formular: {verses}");
    assert!(verses.contains("action=\"/literatur/bibel/act\""), "Verse verwenden ohne JavaScript: {verses}");

    let (_, found) = s.server.get(&format!("{base}&q=Amsel")).await;
    assert!(found.contains("Joh 3,16"), "Wortlautsuche findet die Stelle: {found}");
    assert!(found.contains("ref=Joh%203%2C16"), "Treffer verlinkt die Stelle: {found}");

    let (_, bad) = s.server.get(&format!("{base}&ref=Ps+151%2C1")).await;
    assert_eq!(bad.matches("class=\"bible-verse\"").count(), 0, "eine ungültige Stelle zeigt keine erfundenen Verse");
    assert!(bad.contains("151"), "die Stelle wird als ungültig gemeldet: {bad}");
    let template = include_str!("../templates/literature_bible.html");
    assert!(template.contains("{{ commentaries }}"), "Platz für den Kommentar-Baustein neben der Stelle");
}

#[tokio::test]
async fn the_shared_search_shows_a_bible_group_with_the_module_name_only_in_wording_mode() {
    let s = shelf(&[kiesel()], vec![]).await;
    let (_, page) = s.server.get(&format!("/literatur?work={}&key={}&lq=Amsel", s.work, s.key)).await;
    assert!(page.contains("data-group=\"bibel\" data-module=\"Kiesel\""), "Gruppe „Bibel“ mit Modulname: {page}");
    assert!(page.contains("Der Kiesel lag am Bach, und die Amsel sang."), "Bibeltext des Treffers: {page}");
    let popup = ok(&s.server, "literature.render", json!({ "work": s.work, "key": s.key, "lq": "Amsel" })).await;
    assert!(popup["html"].as_str().unwrap().contains("data-group=\"bibel\""), "auch im Popup: {popup}");
    let (_, rag) = s.server.get(&format!("/literatur?work={}&key={}&lq=Amsel&lmode=rag", s.work, s.key)).await;
    assert!(!rag.contains("data-group=\"bibel\""), "Bedeutungssuche: keine Wortlaut-Gruppe der Bibel");
}

#[tokio::test]
async fn citing_verses_creates_a_proposal_with_the_reference_and_the_current_stays() {
    let s = shelf(&[kiesel()], vec![]).await;
    let form = format!("action=cite&work={}&key={}&module=Kiesel&ref=Ps+139%2C23-24", s.work, s.key);
    let (status, location) = s.server.post_form("/literatur/bibel/act", &form).await;
    assert_eq!(status, 303, "Zitat angelegt, Weiterleitung zum Abschnitt");
    assert_eq!(location.as_deref(), Some(format!("/?work={}&key={}&rev=2", s.work, s.key).as_str()), "zum Vorschlag");
    let node = s.server.state.store.load_work(s.work).await.unwrap().nodes[0].clone();
    assert_eq!(node.current_number(), Some(1), "die aktuelle Fassung bleibt (Einfügen nur durch den Menschen)");
    let proposal = node.revision(2).expect("Vorschlag 2");
    assert_eq!(proposal.status, baruch_core::RevisionStatus::Proposed, "nur ein Vorschlag");
    match &proposal.author {
        baruch_core::Author::Assistant { persona, model } => {
            assert_eq!(
                (persona.as_str(), model.as_str()),
                ("@literatur", "baruch-zitat"),
                "Herkunft wie highlight.cite"
            )
        }
        other => panic!("Autor @literatur erwartet: {other:?}"),
    }
    assert!(
        proposal.body.contains("#quote(block: true, attribution: [Ps 139,23–24 (Kiesel)])["),
        "Zitat mit Stellenangabe und Modul: {}",
        proposal.body
    );
    assert!(
        proposal.body.contains("Prüfe den Kiesel") && proposal.body.contains("den Bach hinab"),
        "Wortlaut aus dem Modul"
    );
    assert!(proposal.body.starts_with("= Am Bach"), "der bisherige Text bleibt vorn: {}", proposal.body);
}

#[tokio::test]
async fn giving_verses_to_a_persona_posts_the_answer_with_the_bible_as_source() {
    let s = shelf(&[kiesel()], vec![Step::words("Kritisch: Der Kiesel passt zum Bach.")]).await;
    let form = format!(
        "action=persona&work={}&key={}&module=Kiesel&ref=Ps+139%2C23&persona=%40lektor&question=Passt+das%3F",
        s.work, s.key
    );
    let (status, location) = s.server.post_form("/literatur/bibel/act", &form).await;
    assert_eq!(status, 303, "Persona hat geantwortet, Weiterleitung");
    assert_eq!(location.as_deref(), Some(format!("/?work={}&key={}", s.work, s.key).as_str()), "zurück zum Abschnitt");
    let call = s.model.calls().pop().expect("ein Modellaufruf");
    let asked: String = call.messages.iter().map(|m| m.content.clone()).collect();
    assert!(
        asked.contains("Prüfe den Kiesel und sieh, wie er glänzt."),
        "der Verstext aus dem Modul geht mit: {asked}"
    );
    let node = s.server.state.store.load_work(s.work).await.unwrap().nodes[0].clone();
    assert_eq!(node.posts.len(), 2, "Frage des Menschen und Antwort der Persona");
    assert_eq!(node.posts[1].author, "@lektor", "Antwort der gewählten Persona");
    assert_eq!(node.posts[1].sources, ["Bibel Kiesel, Ps 139,23"], "Quelle: Bibel und Stelle");
    assert_eq!(node.current_number(), Some(1), "nichts am Buchtext");
}

#[tokio::test]
async fn bible_actions_without_a_section_or_with_highlight_show_an_honest_notice_page() {
    let s = shelf(&[kiesel()], vec![]).await;
    let request = |form: String| {
        axum::http::Request::post("/literatur/bibel/act")
            .header(axum::http::header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(axum::body::Body::from(form))
            .unwrap()
    };
    let (status, _, page) =
        s.server.send(request(format!("action=cite&work={}&module=Kiesel&ref=Ps+139%2C23", s.work))).await;
    assert_eq!(status, 200, "Hinweisseite statt Fehler: {page}");
    assert!(page.contains("Abschnitt wählen"), "ohne Abschnitt der Hinweis „Abschnitt wählen“: {page}");
    let (_, _, page) = s
        .server
        .send(request(format!("action=highlight&work={}&key={}&module=Kiesel&ref=Ps+139%2C23", s.work, s.key)))
        .await;
    assert!(page.contains("Hervorheben geht nur bei Titeln der Bibliothek"), "Hervorheben ehrlich vermerkt: {page}");
    let node = s.server.state.store.load_work(s.work).await.unwrap().nodes[0].clone();
    assert_eq!(node.revisions.len(), 1, "nichts angelegt");
}
