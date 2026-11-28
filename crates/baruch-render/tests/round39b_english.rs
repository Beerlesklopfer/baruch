// SPDX-License-Identifier: AGPL-3.0-or-later
//! Round 39b (ausgaben, contract §2.1): the notes sheet of an English sermon has English words and `lang: "en"`, and
//! the numbering words of an English work typeset (Typst run). German stays as `round39b_golden.rs` pins it. Texts
//! freely invented.

use baruch_core::WorkLanguage;
use baruch_render::notes::{NotesCard, NotesDoc, NotesFormat, notes_source};
use baruch_render::{Source, compile};
use baruch_typst::scheme::{Numbering, Scheme, export_template};

fn english_sermon() -> NotesDoc {
    NotesDoc {
        title: "The Lost Sheep".into(),
        total_clock: "18:30".into(),
        target_clock: Some("20:00".into()),
        cards: vec![NotesCard {
            number: "Introduction".into(),
            title: "A hundred sheep".into(),
            start_clock: "0:00".into(),
            clock: "2:10".into(),
            points: vec!["One is missing".into()],
            refs: vec!["Lk 15,4–6".into()],
            ..NotesCard::default()
        }],
        lang: WorkLanguage::En,
    }
}

#[test]
fn the_notes_sheet_of_an_english_sermon_has_english_words() {
    let source = notes_source(&english_sermon(), NotesFormat::A5, 16);
    for expected in ["\"Cue sheet – The Lost Sheep\"", "lang: \"en\"", "\"Total 18:30 · Target 20:00\""] {
        assert!(source.contains(expected), "the English sheet carries {expected}: {source}");
    }
    for german in ["Stichwortzettel – The", "Gesamt", "Ziel", "lang: \"de\""] {
        assert!(!source.contains(german), "no German word „{german}“ in the sheet of an English work: {source}");
    }
    let empty =
        notes_source(&NotesDoc { title: "Empty".into(), cards: Vec::new(), ..english_sermon() }, NotesFormat::A5, 16);
    assert!(empty.contains("\"No sections.\""), "an empty English sheet says so in English: {empty}");
}

#[test]
fn the_numbering_rules_of_an_english_work_typeset_with_english_words() {
    for (scheme, expected) in [
        (Scheme::PartsChapters, Some("Chapter 2")),
        (Scheme::ChapterWords, Some("Chapter Two")),
        (Scheme::Sermon, None),
    ] {
        let numbering = Numbering::of_scheme_in(scheme, None, "", WorkLanguage::En).unwrap();
        // Typst itself checks the number of the second chapter (the sermon rule returns content, it only has to set).
        let check = expected.map_or(String::new(), |number| {
            format!("#context assert.eq(numbering(heading.numbering, ..counter(heading).get()), \"{number}\")\n")
        });
        let main = format!(
            "{}= First\n\nText.\n\n= Second\n\n{check}Text.\n\n= Third\n\nText.\n",
            export_template("#set text(lang: \"en\")\n", scheme, &numbering)
        );
        let rendered = compile(&Source { main: main.clone(), files: Vec::new() });
        assert!(
            !rendered.has_errors(),
            "{scheme:?}: the English rule sets the expected words without errors: {:?}\n{main}",
            rendered.diagnostics
        );
    }
}
