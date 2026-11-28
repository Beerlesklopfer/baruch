// SPDX-License-Identifier: AGPL-3.0-or-later
//! Round 39b (ausgaben, contract §2.1): an English work gets an English EPUB – navigation, landmarks, guide, cover,
//! title page, notes, glossary, index, figure supplement, back links and the language tag. German stays as the golden
//! fixtures of `round39b_golden.rs` pin it. Texts freely invented (English sample: the German sample with English
//! words and texts).

use baruch_core::WorkLanguage;
use baruch_core::book_text::book_t;
use baruch_epub::template::book_language;
use baruch_epub::{EpubBook, EpubInput, FootnoteMode, build, sample};
use baruch_typst::index::{BackMatter, GlossaryEntry, IndexMark, Projector, builtin_registers, mark_source};
use baruch_typst::typeset::{Draft, GlossaryPosition, IndexPages, TypesetSettings};

/// Does `word` stand in `text` as a whole word? „Glossar“ is part of the English „Glossary“ and must not count.
fn has_word(text: &str, word: &str) -> bool {
    text.match_indices(word).any(|(at, _)| {
        let before = text[..at].chars().next_back();
        let after = text[at + word.len()..].chars().next();
        !before.is_some_and(char::is_alphabetic) && !after.is_some_and(char::is_alphabetic)
    })
}

/// The German book words the EPUB writes (seeds and added keys), start before a placeholder, trimmed.
fn german_words() -> Vec<String> {
    [
        "book.toc.title",
        "book.cover.title",
        "book.cover.alt",
        "book.title_page.title",
        "book.untitled",
        "book.glossary.title",
        "book.index.empty",
        "book.figure.supplement",
        "book.footnote.back",
        "book.draft.watermark",
        "book.front.imprint",
        "book.front.dedication",
        "book.nav.landmarks",
        "book.endnotes.title",
        "book.glossary.variants",
        "book.glossary.bible_refs",
        "book.index.see",
        "book.meta.image_credit",
        "book.a11y.summary_all_alt",
        "book.a11y.summary_some_alt",
        "book.draft.notice",
        "book.index.keyword_register",
        "book.index.person_register",
    ]
    .iter()
    .map(|key| book_t(WorkLanguage::De, key, &[]).split('{').next().unwrap_or_default().trim().to_owned())
    .collect()
}

/// The sample as an English work: English texts, endnotes, a cover without alternative text, no title, index and
/// glossary pages, draft.
fn english_input() -> EpubInput {
    let mut input = sample::input();
    input.work_language = WorkLanguage::En;
    input.meta.language = String::new();
    input.meta.title = String::new();
    input.meta.subtitle = "A building diary".into();
    input.meta.image_credit = "Drawings by Erin Example".into();
    input.template = "#set heading(numbering: \"1.1\")\n".into();
    input.footnotes = FootnoteMode::Endnotes;
    input.imprint = vec!["© 2026 Erin Example".into()];
    input.dedication = "For everyone building by the brook.".into();
    input.typeset.draft = Some(Draft::default());
    if let Some(cover) = &mut input.cover {
        cover.alt = String::new();
    }
    let titles = ["Planning", "The Place", "Soil and Light", "The Foundation", "Concrete & Gravel"];
    for (section, title) in input.sections.iter_mut().zip(titles) {
        section.title = title.into();
    }
    let [part, ch1, s11, ch2, s21] = sample::KEYS;
    input.sections[0].body = format!(
        "#heading(level: 1, numbering: none)[Planning] <baruch-part>\n{}\n",
        baruch_typst::xref::anchor_line(part)
    );
    input.sections[1].body = format!(
        "= The Place <k{ch1}>\n\nAn old willow stands by the brook.#footnote[It flooded twelve years ago.]\n\n\
         #figure(image(\"bilder/weide.png\", alt: \"Sketch of the willow\"), caption: [The willow]) <willow>\n\n\
         See @willow.\n"
    );
    input.sections[2].body = format!("== Soil and Light <k{s11}>\n\nThe soil is loamy.\n");
    input.sections[3].body = format!(
        "= The Foundation <k{ch2}>\n\nAs #link(<k{s11}>)[Soil and Light] says, loam{} carries badly.\n",
        mark_source(&IndexMark::new("stichwort", "Loam"))
    );
    input.sections[4].body = format!("== Concrete & Gravel <k{s21}>\n\nGravel under every pillar.\n");
    let back = BackMatter {
        settings: TypesetSettings {
            index_pages: Some(IndexPages { registers: vec!["stichwort".into(), "personen".into()], columns: 2 }),
            glossary_page: Some(GlossaryPosition::AfterIndexes),
            glossary_first_mention: Some(true),
            ..TypesetSettings::default()
        },
        registers: builtin_registers(),
        glossary: vec![GlossaryEntry {
            uuid: "7b0e5c1d-0000-4000-8000-000000000042".into(),
            term: "Willow".into(),
            variants: vec!["Salix".into()],
            short: "A tree by the water.".into(),
            bible_refs: vec!["Ps 137,2".into()],
            ..GlossaryEntry::default()
        }],
        lang: WorkLanguage::En,
    };
    let mut projector = Projector::new(&back);
    for section in &mut input.sections {
        section.body = projector.project(&section.body).into_owned();
    }
    input.back_matter = back;
    input
}

/// All text files of the book and the package document as one text; the style sheet is left out (its German comments
/// such as `/* Baruch: Register und Glossar */` are no book text and no reader shows them).
fn all_text(book: &EpubBook) -> String {
    let mut out = book.opf.clone();
    for item in book.items.iter().filter(|i| !i.media_type.starts_with("image/") && i.media_type != "text/css") {
        out.push_str(&String::from_utf8_lossy(&item.bytes));
    }
    for entry in &book.toc {
        out.push_str(&entry.title);
    }
    out
}

fn text(book: &EpubBook, href: &str) -> String {
    let item = book.items.iter().find(|i| i.href == href).unwrap_or_else(|| panic!("{href} is in the book"));
    String::from_utf8_lossy(&item.bytes).into_owned()
}

#[test]
fn an_english_work_gets_an_english_epub_without_german_book_words() {
    let book = build(&english_input()).expect("the English sample builds");
    let all = all_text(&book);
    for word in german_words() {
        assert!(!has_word(&all, &word), "an English EPUB must not carry the German book word „{word}“:\n{all}");
    }
    let nav = text(&book, "nav.xhtml");
    for expected in ["<h1>Contents</h1>", "<h2>Guide</h2>", ">Cover</a>", ">Title page</a>", ">Text</a>"] {
        assert!(nav.contains(expected), "navigation and landmarks in English ({expected}): {nav}");
    }
    for expected in [
        "<dc:language>en</dc:language>",
        "xml:lang=\"en\"",
        "title=\"Cover\"",
        "title=\"Title page\"",
        "title=\"Contents\"",
        "Untitled (Draft)",
        "<dc:rights>Image credit: Drawings by Erin Example</dc:rights>",
    ] {
        assert!(book.opf.contains(expected), "package document in English ({expected}): {}", book.opf);
    }
    assert!(text(&book, "text/cover.xhtml").contains("alt=\"Cover: Untitled\""), "cover alternative text in English");
    assert!(text(&book, "text/title.xhtml").contains("Draft: DRAFT"), "draft notice with the English default text");
    let notes = text(&book, "text/notes.xhtml");
    assert!(notes.contains("<h1>Notes</h1>") && notes.contains("title=\"back to the text\""), "endnotes: {notes}");
    let glossary = text(&book, "text/glossary.xhtml");
    assert!(
        glossary.contains("<h1>Glossary</h1>")
            && glossary.contains("Also: Salix")
            && glossary.contains("Bible references: Ps 137,2"),
        "glossary in English: {glossary}"
    );
    let persons = book.items.iter().find(|i| i.title.as_deref() == Some("Index of persons"));
    let persons = persons.map(|i| String::from_utf8_lossy(&i.bytes).into_owned()).unwrap_or_default();
    assert!(persons.contains("No entries."), "an empty built-in index in English: {persons}");
    assert!(all.contains("Figure\u{a0}1"), "a reference to a figure shows the English supplement: {all}");
}

#[test]
fn pop_up_notes_of_an_english_work_have_an_english_back_link() {
    let mut input = english_input();
    input.footnotes = FootnoteMode::Popup;
    let book = build(&input).expect("builds");
    let chapter = text(&book, "text/k11.xhtml");
    assert!(chapter.contains("title=\"back to the text\""), "pop-up back link in English: {chapter}");
}

#[test]
fn the_book_language_follows_the_typesetting_setting_then_the_work_language() {
    let template = "#set text(lang: \"de\")\n";
    assert_eq!(book_language(Some("de-CH"), template, WorkLanguage::En), "de-CH", "the typesetting setting wins");
    assert_eq!(book_language(None, template, WorkLanguage::De), "de", "a German work: the template as before");
    assert_eq!(book_language(None, "", WorkLanguage::De), "de", "a German work without template: de as before");
    assert_eq!(
        book_language(None, "#set text(lang: \"fr\")\n", WorkLanguage::De),
        "fr",
        "a German work with another template language keeps it (byte-equal German behaviour)"
    );
    assert_eq!(book_language(None, template, WorkLanguage::En), "en", "an English work without setting: en");
    assert_eq!(book_language(Some("x y"), "", WorkLanguage::En), "en", "an invalid setting does not count");
}
