// SPDX-License-Identifier: AGPL-3.0-or-later
//! Round 39b (ausgaben, contract §4.1): golden German EPUB files. The fixtures under `tests/data/round39b/` were
//! generated with the unchanged S1 code (commit 5c5a821) before any work-language change; every later change must keep
//! them byte-equal, because German works must not change.
//!
//! Regenerating them is forbidden after the first commit of the round. The environment variable `BARUCH_BLESS_R39B`
//! only exists for that first commit (it writes the files instead of comparing).
//!
//! Two inputs: the sample work as it is (pop-up notes, cover with alternative text), and the sample with endnotes,
//! a cover without alternative text, an untitled book and index and glossary pages. Text files are compared whole,
//! images by size and an FNV-1a hash (no binary files in the repository).

use std::path::PathBuf;

use baruch_epub::{EpubInput, FootnoteMode, build, package, sample};
use baruch_typst::index::{BackMatter, GlossaryEntry, IndexMark, Projector, builtin_registers, mark_source};
use baruch_typst::typeset::{GlossaryPosition, IndexPages, TypesetSettings};

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

/// FNV-1a, 64 bit: enough to notice a changed image without storing it.
fn fnv(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, b| (hash ^ u64::from(*b)).wrapping_mul(0x0100_0000_01b3))
}

/// Every file of the book (text whole, binary by size and hash), the spine, the table of contents and the warnings,
/// plus size and hash of the packaged archive.
pub fn dump(input: &EpubInput) -> String {
    let book = build(input).expect("the golden input builds");
    let mut out = String::new();
    for item in &book.items {
        out.push_str(&format!(
            "=== {} ({}, {:?}, id {}, properties {:?}, title {:?}, key {:?})\n",
            item.href, item.media_type, item.kind, item.id, item.properties, item.title, item.key
        ));
        match std::str::from_utf8(&item.bytes) {
            Ok(text) if !item.media_type.starts_with("image/") => {
                out.push_str(text);
                out.push('\n');
            }
            _ => out.push_str(&format!("[{} bytes, fnv {:016x}]\n", item.bytes.len(), fnv(&item.bytes))),
        }
    }
    out.push_str(&format!("=== content.opf\n{}\n", book.opf));
    out.push_str(&format!("=== spine\n{:?}\n=== toc\n", book.spine));
    for entry in &book.toc {
        out.push_str(&format!("{entry:?}\n"));
    }
    out.push_str("=== warnings\n");
    for warning in &book.warnings {
        out.push_str(&format!("{warning:?}\n"));
    }
    let zip = package(&book).expect("the golden book packages");
    out.push_str(&format!("=== package\n[{} bytes, fnv {:016x}]\n", zip.len(), fnv(&zip)));
    out
}

/// The sample with endnotes, a cover without alternative text, no title and index and glossary pages.
pub fn full_input() -> EpubInput {
    let mut input = sample::input();
    input.footnotes = FootnoteMode::Endnotes;
    input.meta.title = String::new();
    if let Some(cover) = &mut input.cover {
        cover.alt = String::new();
    }
    let back = BackMatter {
        settings: TypesetSettings {
            index_pages: Some(IndexPages { registers: vec!["stichwort".into(), "personen".into()], columns: 2 }),
            glossary_page: Some(GlossaryPosition::AfterIndexes),
            glossary_first_mention: Some(true),
            ..TypesetSettings::default()
        },
        registers: builtin_registers(),
        glossary: vec![GlossaryEntry {
            uuid: "7b0e5c1d-0000-4000-8000-000000000039".into(),
            term: "Weide".into(),
            variants: vec!["Salix".into()],
            short: "Baum am Wasser.".into(),
            long: "Sie wächst schnell.".into(),
            bible_refs: vec!["Ps 137,2".into()],
        }],
    };
    let lehm = mark_source(&IndexMark::new("stichwort", "Lehm"));
    input.sections[3].body = input.sections[3].body.replacen("Lehm schlecht", &format!("Lehm{lehm} schlecht"), 1);
    let mut projector = Projector::new(&back);
    for section in &mut input.sections {
        section.body = projector.project(&section.body).into_owned();
    }
    input.back_matter = back;
    input
}

#[test]
fn the_sample_epub_stays_german_for_a_german_work() {
    golden("epub_sample.de.txt", &dump(&sample::input()));
}

#[test]
fn the_epub_with_endnotes_index_and_glossary_stays_german_for_a_german_work() {
    golden("epub_full.de.txt", &dump(&full_input()));
}
