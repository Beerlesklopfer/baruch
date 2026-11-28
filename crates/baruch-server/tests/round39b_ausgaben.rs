// SPDX-License-Identifier: AGPL-3.0-or-later
//! Round 39b „User interface German and English, outputs and help“, owner ausgaben: book output in the work language
//! (EPUB, Typst/PDF, DOCX, notes sheet, read-back words), golden German fixtures and the book catalog completeness
//! check (contract §2.1, §4.1, §4.2).
//!
//! Golden fixtures (`tests/data/round39b/`): generated with the unchanged S1 code (commit 5c5a821) from a fixed German
//! work before any work-language change. Every later change must keep them byte-equal, because German works must not
//! change. Regenerating them is forbidden after the first commit of the round; `BARUCH_BLESS_R39B` only exists for
//! that first commit. Texts freely invented.

use std::path::PathBuf;

use crate::common;

/// Compares `actual` with the fixture `name` (or writes it while blessing the first commit).
fn golden(name: &str, actual: &str) {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/round39b").join(name);
    if std::env::var_os("BARUCH_BLESS_R39B").is_some() {
        std::fs::create_dir_all(path.parent().expect("fixture folder")).expect("fixture folder can be created");
        std::fs::write(&path, actual).expect("fixture can be written");
        return;
    }
    let expected = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("golden fixture {} is missing ({e}); German works must not change", path.display()));
    if expected != actual {
        let line = expected.lines().zip(actual.lines()).position(|(a, b)| a != b).map_or(0, |i| i + 1);
        panic!(
            "German works must not change: {name} differs from the golden fixture (first differing line {line}).\n\
             --- expected\n{expected}\n--- actual\n{actual}"
        );
    }
}

/// The fixed test work: a part, three chapters (one with a section), a footnote, a cross-reference and a Bible
/// reference.
pub fn source(title: &str) -> String {
    format!(
        "#set document(title: \"{title}\", author: \"Erika Beispiel\")\n\
         #set text(lang: \"de\")\n\n\
         {part}\n\
         = Die Weide\n\nAm Bach steht eine alte Weide.#footnote[Erfunden.] Wie Ps 46,4 sagt.\n\n\
         == Boden und Licht\n\nDer Boden ist lehmig.\n\n\
         = Das Fundament\n\nSechs Punktfundamente tragen das Haus.\n\n\
         = Das Dach\n\nZum Schluss kommt das Dach.\n",
        part = baruch_typst::part_line("Planen"),
    )
}

/// Every output that does not depend on the clock or the random work id, for every scheme with words.
async fn outputs(server: &common::Server, work: i64) -> String {
    let store = &server.state.store;
    let mut out = String::new();
    for scheme in ["parts_chapters", "chapter_words", "legal", "sermon", "book"] {
        store.set_numbering_scheme(work, scheme, None).await.expect("the scheme is valid");
        out.push_str(&format!("==== scheme {scheme}\n"));
        out.push_str(&store.export_typst(work).await.expect("the work exports"));
        out.push_str("\n==== notes\n");
        let notes = baruch_server::pdf::notes::notes_of(&server.state, work).await.expect("notes of the work");
        out.push_str(&baruch_render::notes::notes_source(&notes.doc(), baruch_render::notes::NotesFormat::A5, 16));
    }
    let mut input = baruch_server::epub::epub_input(&server.state, work, baruch_epub::FootnoteMode::Endnotes)
        .await
        .expect("the EPUB input of the work");
    input.meta.identifier = "urn:uuid:00000000-0000-4000-8000-000000000039".into();
    input.meta.modified = "2026-11-28T12:00:00Z".into();
    let book = baruch_epub::build(&input).expect("the EPUB builds");
    for item in &book.items {
        out.push_str(&format!("==== {} ({:?})\n", item.href, item.kind));
        out.push_str(&String::from_utf8_lossy(&item.bytes));
        out.push('\n');
    }
    out.push_str(&format!("==== opf\n{}\n", book.opf));
    out
}

#[tokio::test]
async fn a_german_work_keeps_its_export_notes_and_epub() {
    let server = common::open().await;
    let store = &server.state.store;
    let work = store.import_typst(&source("Das Gartenhaus"), "gartenhaus.typ", "Erika").await.expect("import").work;
    let settings = baruch_typst::typeset::TypesetSettings {
        index_pages: Some(baruch_typst::typeset::IndexPages { registers: vec!["personen".into()], columns: 2 }),
        ..Default::default()
    };
    store.set_work_typeset(work, &settings, "Erika").await.expect("typeset settings are valid");
    golden("server_work.de.txt", &outputs(&server, work).await);
}
