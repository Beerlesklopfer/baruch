// SPDX-License-Identifier: AGPL-3.0-or-later
//! Round 39b (ausgaben, contract §2.1): the generated words of `baruch-typst` follow the work language. English works
//! get the English catalog words (numbering of every scheme, page abbreviation, draft, imprint, cover, back matter);
//! German stays as the golden fixtures of `round39b_golden.rs` pin it. Book details freely invented.

use baruch_core::WorkLanguage;
use baruch_core::book_text::{book_keys, book_t};
use baruch_typst::cover::{
    BookInfo, CoverRef, FrameOptions, TitleArea, WrapOptions, book_dict_in, frame_parts, frame_parts_in, imprint_lines,
    imprint_lines_in, wrap_source_with_rules, wrap_source_with_rules_in,
};
use baruch_typst::index::{
    BackMatter, GlossaryEntry, back_matter, builtin_registers, register_title, strip_back_matter,
};
use baruch_typst::scheme::{Numbering, Scheme, cardinal_word_en, export_template};
use baruch_typst::typeset::{
    DEFAULT_DRAFT_TEXT, Draft, GlossaryPosition, IndexPages, TypesetSettings, draft_line_in, draft_rule, draft_rule_in,
    draft_text_in,
};
use baruch_typst::xref::{find_xrefs, xref_snippet, xref_snippet_in};

/// German values of the contract seeds that may never stand in an English output (the start before a placeholder,
/// trimmed). `S.` is left out (it is part of ordinary English text) and checked separately as `, S.~`.
fn german_seeds() -> Vec<String> {
    const SEEDS: [&str; 24] = [
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
        "book.numbering.part",
        "book.numbering.chapter",
        "book.numbering.introduction",
        "book.numbering.conclusion",
        "book.numbering.legal_paragraph",
        "book.numbering.legal_number",
        "book.imprint.dnb_title",
        "book.imprint.dnb_text",
        "book.imprint.typeset_with",
        "book.imprint.cover_credit",
        "book.cover.isbn_placeholder",
        "book.scripture.wording_unchecked",
        "book.claims.not_supported",
        "book.claims.check_reference",
    ];
    SEEDS
        .iter()
        .map(|key| {
            let text = book_t(WorkLanguage::De, key, &[]);
            text.split('{').next().unwrap_or_default().trim().to_owned()
        })
        .collect()
}

/// Does `word` stand in `text` as a whole word (no letter directly before or after)? „Glossar“ is part of the English
/// „Glossary“ and must not count.
pub fn has_word(text: &str, word: &str) -> bool {
    text.match_indices(word).any(|(at, _)| {
        let before = text[..at].chars().next_back();
        let after = text[at + word.len()..].chars().next();
        !before.is_some_and(char::is_alphabetic) && !after.is_some_and(char::is_alphabetic)
    })
}

/// Panics with the offending word if `output` (without Typst comment lines) carries a German seed.
fn assert_no_german(name: &str, output: &str) {
    let visible: String = output.lines().filter(|l| !l.trim_start().starts_with("//")).collect::<Vec<_>>().join("\n");
    for seed in german_seeds() {
        assert!(
            !has_word(&visible, &seed),
            "{name}: an English work must not get the German seed word „{seed}“ (the outputs follow the work \
             language):\n{visible}"
        );
    }
    assert!(!visible.contains(", S.~"), "{name}: the German page abbreviation must not stand in an English work");
}

#[test]
fn every_scheme_numbers_an_english_work_with_english_words() {
    for scheme in Scheme::ALL {
        let custom = Some("I.;A.;1.");
        let english = Numbering::of_scheme_in(scheme, custom, "", WorkLanguage::En).expect("every scheme numbers");
        let german = Numbering::of_scheme(scheme, custom, "").expect("every scheme numbers");
        assert_eq!(
            Numbering::of_scheme_in(scheme, custom, "", WorkLanguage::De),
            Ok(german.clone()),
            "{scheme:?}: of_scheme is exactly of_scheme_in with German"
        );
        let mut report = english.heading_rule();
        for part in 1..=3 {
            report.push_str(&english.part(part));
        }
        for path in [&[1][..], &[2], &[3], &[4], &[2, 1], &[21]] {
            report.push_str(&english.heading(path, 4));
        }
        assert_no_german(&format!("{scheme:?}"), &report);
    }
    let parts = Numbering::of_scheme_in(Scheme::PartsChapters, None, "", WorkLanguage::En).unwrap();
    assert_eq!(parts.part(2), "Part II", "parts and chapters: Roman part number after the English noun");
    assert_eq!(parts.heading(&[3], 3), "Chapter 3", "parts and chapters: chapter number after the English noun");
    let words = Numbering::of_scheme_in(Scheme::ChapterWords, None, "", WorkLanguage::En).unwrap();
    assert_eq!(words.part(1), "Part One", "spelled out: English cardinal word after the noun (documented choice)");
    assert_eq!(words.heading(&[21], 21), "Chapter Twenty-One", "spelled out: compound numbers with a hyphen");
    assert_eq!(words.heading(&[100], 100), "Chapter 100", "from 100 on digits, as in the German scheme");
    let legal = Numbering::of_scheme_in(Scheme::Legal, None, "", WorkLanguage::En).unwrap();
    assert_eq!(
        (legal.heading(&[1], 1), legal.heading(&[1, 2], 1), legal.heading(&[1, 2, 3], 1)),
        ("§ 1".to_owned(), "para. 2".to_owned(), "no. 3".to_owned()),
        "legal: the section sign stays, paragraph and number words are English"
    );
    for scheme in [Scheme::Sermon, Scheme::Book] {
        let sermon = Numbering::of_scheme_in(scheme, None, "", WorkLanguage::En).unwrap();
        assert_eq!(
            (sermon.heading(&[1], 3), sermon.heading(&[2], 3), sermon.heading(&[3], 3)),
            ("Introduction".to_owned(), "I.".to_owned(), "Conclusion".to_owned()),
            "{scheme:?}: introduction and conclusion in English, the Roman numbers in between stay"
        );
    }
    assert_eq!(cardinal_word_en(40), "Forty", "a round ten has no hyphen");
}

#[test]
fn the_template_reader_recognises_the_english_rules_as_english_schemes() {
    for scheme in [Scheme::PartsChapters, Scheme::ChapterWords, Scheme::Legal, Scheme::Sermon, Scheme::Book] {
        let english = Numbering::of_scheme_in(scheme, None, "", WorkLanguage::En).unwrap();
        let exported = export_template("#set text(lang: \"en\")\n", scheme, &english);
        let back = Numbering::from_template(&exported).expect("the exported English rule is readable");
        assert_eq!(back, english, "{scheme:?}: an exported English work reads back as the same English scheme");
        assert_eq!(back.lang(), WorkLanguage::En, "{scheme:?}: the words keep their language on the way back");
        let german = Numbering::of_scheme(scheme, None, "").unwrap();
        let again = Numbering::from_template(&export_template("", scheme, &german)).unwrap();
        assert_eq!(again.lang(), WorkLanguage::De, "{scheme:?}: German rules keep reading as German (regression)");
    }
}

#[test]
fn cross_references_use_the_page_abbreviation_of_the_work_and_are_recognised_in_both() {
    let english = xref_snippet_in(103, "the introduction", true, WorkLanguage::En);
    assert_eq!(
        english, "#link(<k103>)[the introduction, p.~#context counter(page).at(<k103>).first()]",
        "English works write „p.“ before the page number"
    );
    assert_eq!(
        xref_snippet_in(103, "x", true, WorkLanguage::De),
        xref_snippet(103, "x", true),
        "German is exactly the snippet from before Round 39b"
    );
    for (text, language) in [(english.as_str(), "English"), (&xref_snippet(7, "die Einleitung", true), "German")] {
        let found = find_xrefs(text);
        assert_eq!(found.len(), 1, "{language}: one reference found");
        assert!(found[0].page, "{language}: the page addition is recognised in every work language");
        assert!(!found[0].text.contains('~'), "{language}: the addition is not part of the link text");
    }
}

#[test]
fn the_draft_watermark_and_its_date_line_follow_the_work_language() {
    let draft = Draft { date: true, ..Draft::default() };
    assert_eq!(draft.text, DEFAULT_DRAFT_TEXT, "the default draft text is the stored German default");
    assert_eq!(draft_text_in(&draft, WorkLanguage::En), "DRAFT", "the default text becomes the English word");
    let own = Draft { text: "PROBE".into(), ..Draft::default() };
    assert_eq!(draft_text_in(&own, WorkLanguage::En), "PROBE", "a text of the human stays as it is");
    assert_eq!(draft_line_in(&draft, Some("9.11.2026"), WorkLanguage::En), "As of 9.11.2026", "date line in English");
    let rule = draft_rule_in(&draft, Some("9.11.2026"), WorkLanguage::En);
    assert!(rule.contains("DRAFT") && rule.contains("As\u{a0}of"), "the watermark shows the English words: {rule}");
    assert_no_german("draft", &rule);
    assert_eq!(
        draft_rule_in(&draft, Some("9.11.2026"), WorkLanguage::De),
        draft_rule(&draft, Some("9.11.2026")),
        "German is exactly the rule from before Round 39b"
    );
}

fn info() -> BookInfo {
    BookInfo {
        title: "The Garden House".into(),
        subtitle: "A Building Diary".into(),
        author: "Erin Example".into(),
        publisher: "Example Press".into(),
        isbn: "978-3-16-148410-0".into(),
        bible: "Invented Version".into(),
        image_credit: "Drawings by Erin Example".into(),
        printer: "Example Print".into(),
        dedication: "For everyone building by the brook.".into(),
        blurb: "How a willow became a garden house.".into(),
        ..BookInfo::default()
    }
}

#[test]
fn imprint_cover_and_front_matter_of_an_english_work_have_no_german_words() {
    let lines = imprint_lines_in(&info(), WorkLanguage::En);
    assert_eq!(
        lines.first().map(String::as_str),
        Some("Bibliographic information published by the Deutsche Nationalbibliothek"),
        "the DNB notice heading in English"
    );
    for expected in [
        "Bible quotations: Invented Version.",
        "Cover design and image credit: Drawings by Erin Example",
        "Typeset with Baruch",
        "Printed by: Example Print",
    ] {
        assert!(lines.iter().any(|l| l == expected), "imprint line „{expected}“ in English: {lines:?}");
    }
    assert_no_german("imprint", &lines.join("\n"));
    assert_eq!(imprint_lines_in(&info(), WorkLanguage::De), imprint_lines(&info()), "German: as before");

    let opts = FrameOptions {
        cover: Some(CoverRef { path: ".baruch/cover.png".into(), alt: "Willow".into() }),
        cover_pages: true,
        front_matter: true,
        title_area: TitleArea::Top,
        barcode: None,
        author_photo: None,
        layout: None,
        template_functions: Default::default(),
    };
    let frame = frame_parts_in(&info(), &opts, WorkLanguage::En);
    assert_no_german("frame", &format!("{}\n{}", frame.front, frame.back));
    assert!(frame.back.contains("ISBN and barcode"), "without a barcode the English placeholder: {}", frame.back);
    assert_eq!(frame_parts_in(&info(), &opts, WorkLanguage::De), frame_parts(&info(), &opts), "German: as before");
    let wrap = WrapOptions {
        page_width_mm: 120.0,
        page_height_mm: 180.0,
        spine_mm: 9.0,
        bleed_mm: 3.0,
        title_area: TitleArea::Middle,
        cover: None,
        barcode: None,
        author_photo: None,
        layout: None,
    };
    let source = wrap_source_with_rules_in(&info(), &wrap, "", WorkLanguage::En);
    assert!(source.contains("#set text(lang: \"en\")"), "the cover source is set in English: {source}");
    assert_no_german("wrap", &source);
    assert_eq!(
        wrap_source_with_rules_in(&info(), &wrap, "", WorkLanguage::De),
        wrap_source_with_rules(&info(), &wrap, ""),
        "German: as before"
    );
    assert_no_german("book dict", &book_dict_in(&info(), WorkLanguage::En));
}

#[test]
fn the_back_matter_of_an_english_work_has_english_headings_and_words() {
    let back = BackMatter {
        settings: TypesetSettings {
            index_pages: Some(IndexPages {
                registers: vec!["stichwort".into(), "personen".into(), "bibel".into()],
                columns: 2,
            }),
            glossary_page: Some(GlossaryPosition::AfterIndexes),
            ..TypesetSettings::default()
        },
        registers: builtin_registers(),
        glossary: vec![GlossaryEntry {
            uuid: "7b0e5c1d-0000-4000-8000-000000000041".into(),
            term: "Willow".into(),
            short: "A tree by the water.".into(),
            ..GlossaryEntry::default()
        }],
        lang: WorkLanguage::En,
    };
    let source = back_matter(&back, &["= The Place\n\nA willow stands by the brook.\n"]);
    for word in ["\"Glossary\"", "\"No entries.\"", "\"Index\"", "\"Index of persons\"", "#emph[see also]"] {
        assert!(source.contains(word), "the English back matter carries {word}: {source}");
    }
    assert_no_german("back matter", &source);
    let (_, cut) = strip_back_matter(&format!("= The Place\n\n{source}"));
    assert!(cut, "the import recognises an English back matter as generated, like a German one");
    let custom = baruch_typst::index::Register {
        slug: "orte".into(),
        title: "Places".into(),
        kind: baruch_typst::index::RegisterKind::Custom,
    };
    assert_eq!(register_title(&custom, WorkLanguage::En), "Places", "an own index title stays as the human wrote it");
}

#[test]
fn every_english_catalog_text_is_free_of_the_german_seed_words() {
    for key in book_keys() {
        let text = book_t(WorkLanguage::En, key, &[]);
        for seed in german_seeds() {
            assert!(
                !has_word(&text, &seed),
                "English catalog text of `{key}` carries the German word „{seed}“: {text}"
            );
        }
    }
}
