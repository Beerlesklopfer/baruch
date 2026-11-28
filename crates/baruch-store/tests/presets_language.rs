// SPDX-License-Identifier: AGPL-3.0-or-later
//! Round 39c: the preset chapters of a new work take their titles in the WORK language (`NewWork::language`,
//! `book.preset.*` of the book catalog), and the work language is written to `work.language` (migration 0055).
//! German presets are the old literals, byte for byte (`SERMON_OUTLINE`, „Erstes Kapitel“). Texts freely invented.

use baruch_core::{WorkKind, WorkLanguage};
use baruch_store::{
    NewWork, Preset, SERMON_OUTLINE, SERMON_TEXT_TITLE, Store, sermon_outline_in, sermon_text_title_in,
};

async fn fresh() -> (tempfile::TempDir, Store) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).await.unwrap();
    (dir, store)
}

fn new_work(title: &str, preset: Preset, language: WorkLanguage) -> NewWork<'_> {
    let kind = if preset == Preset::Sermon { WorkKind::Sermon } else { WorkKind::Book };
    NewWork { title, author: "Tilda Rohr", kind, preset, subtitle: "", audience: "", bible: "", language }
}

async fn titles(store: &Store, work: i64) -> Vec<String> {
    store.load_work(work).await.unwrap().nodes.iter().map(|n| n.title.clone()).collect()
}

#[test]
fn the_german_outline_equals_the_old_constants_word_for_word() {
    let german = sermon_outline_in(WorkLanguage::De);
    let old: Vec<(String, bool)> =
        SERMON_OUTLINE.iter().map(|(title, numbered)| ((*title).to_owned(), *numbered)).collect();
    assert_eq!(german, old, "German works must not change: the catalog words are the former literals");
    assert_eq!(
        sermon_text_title_in(WorkLanguage::De),
        SERMON_TEXT_TITLE,
        "the sermon text item keeps its German title"
    );
}

#[test]
fn the_english_outline_has_the_same_shape_with_english_words() {
    let english = sermon_outline_in(WorkLanguage::En);
    let german = sermon_outline_in(WorkLanguage::De);
    assert_eq!(
        english.iter().map(|(_, numbered)| *numbered).collect::<Vec<_>>(),
        german.iter().map(|(_, numbered)| *numbered).collect::<Vec<_>>(),
        "the language changes the words, not which items carry a number"
    );
    assert_eq!(english.len(), 7, "seven parts as in German");
    assert_eq!(sermon_text_title_in(WorkLanguage::En), english[0].0, "the first item is the sermon text");
    assert!(
        english.iter().all(|(title, _)| title.is_ascii()),
        "no German umlaut in an English preset title: {english:?}"
    );
    assert_ne!(english[0].0, german[0].0, "the English title is not the German one");
}

#[tokio::test]
async fn a_german_book_gets_the_old_first_chapter_and_language_de() {
    let (_dir, store) = fresh().await;
    let created = store.create_work(&new_work("Das Fährhaus", Preset::Book, WorkLanguage::De), "Autor").await.unwrap();
    assert_eq!(titles(&store, created.work).await, ["Erstes Kapitel"], "German book: the old chapter title");
    assert_eq!(store.work_language(created.work).await.unwrap(), WorkLanguage::De, "work.language is written");
    let body = &store.load_work(created.work).await.unwrap().nodes[0].revisions[0].body;
    assert_eq!(body, "= Erstes Kapitel\n\n", "the heading of revision 1 uses the same title");
}

#[tokio::test]
async fn an_english_book_gets_an_english_first_chapter_and_language_en() {
    let (_dir, store) = fresh().await;
    let created =
        store.create_work(&new_work("The Ferry House", Preset::Book, WorkLanguage::En), "Author").await.unwrap();
    assert_eq!(titles(&store, created.work).await, ["First chapter"], "English book: the English chapter title");
    assert_eq!(store.work_language(created.work).await.unwrap(), WorkLanguage::En, "work.language is written");
    let body = &store.load_work(created.work).await.unwrap().nodes[0].revisions[0].body;
    assert_eq!(body, "= First chapter\n\n", "the heading of revision 1 uses the same title");
}

#[tokio::test]
async fn sermons_follow_the_work_language_and_the_sermon_text_lands_in_the_first_item() {
    let (_dir, store) = fresh().await;
    let text = "#quote[A reed shaken by the wind]";
    for language in WorkLanguage::ALL {
        let title = format!("Sermon in {}", language.as_str());
        let created = store
            .create_work_with_text(&new_work(&title, Preset::Sermon, language), "Author", Some(text))
            .await
            .unwrap();
        let expected: Vec<String> = sermon_outline_in(language).into_iter().map(|(t, _)| t).collect();
        assert_eq!(titles(&store, created.work).await, expected, "the outline of a {language:?} sermon");
        let work = store.load_work(created.work).await.unwrap();
        assert_eq!(
            work.nodes[0].revisions[0].body,
            format!("= {}\n\n{text}\n\n", sermon_text_title_in(language)),
            "the sermon text stands under the heading of the first item in {language:?}"
        );
        assert!(
            work.nodes[1].revisions[0].body.starts_with(&format!("= {}\n", expected[1])),
            "the other items keep only their heading"
        );
        assert_eq!(store.work_language(created.work).await.unwrap(), language, "work.language is written");
    }
}
