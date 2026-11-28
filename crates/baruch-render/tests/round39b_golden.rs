// SPDX-License-Identifier: AGPL-3.0-or-later
//! Round 39b (ausgaben, contract §4.1): golden German notes sheet (Typst source). The fixture under
//! `tests/data/round39b/` was generated with the unchanged S1 code (commit 5c5a821) before any work-language change;
//! every later change must keep it byte-equal, because German works must not change.
//!
//! Regenerating it is forbidden after the first commit of the round. The environment variable `BARUCH_BLESS_R39B`
//! only exists for that first commit (it writes the file instead of comparing). Texts freely invented.

use std::path::PathBuf;

use baruch_render::notes::{NotesCard, NotesDoc, NotesFormat, notes_source};

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
    assert!(
        expected == actual,
        "German works must not change: {name} differs.\n--- expected\n{expected}\n--- actual\n{actual}"
    );
}

/// A sermon with two cards and a target time.
pub fn doc() -> NotesDoc {
    NotesDoc {
        title: "Das verlorene Schaf".into(),
        total_clock: "18:30".into(),
        target_clock: Some("20:00".into()),
        cards: vec![
            NotesCard {
                number: "Einleitung".into(),
                title: "Hundert Schafe".into(),
                depth: 0,
                start_clock: "0:00".into(),
                clock: "2:10".into(),
                points: vec!["Einer fehlt".into(), "Der Hirte zählt \"genau\"".into()],
                refs: vec!["Lk 15,4–6".into()],
            },
            NotesCard {
                number: "I.".into(),
                title: "Suchen".into(),
                depth: 1,
                start_clock: "2:10".into(),
                clock: "6:00".into(),
                points: Vec::new(),
                refs: Vec::new(),
            },
        ],
    }
}

#[test]
fn the_notes_sheet_stays_german_for_a_german_work() {
    let mut out = String::new();
    for format in [NotesFormat::A5, NotesFormat::Cards] {
        out.push_str(&format!("== {}\n", format.as_str()));
        out.push_str(&notes_source(&doc(), format, format.default_font_pt()));
    }
    out.push_str("== empty\n");
    out.push_str(&notes_source(&NotesDoc { title: "Leer".into(), ..NotesDoc::default() }, NotesFormat::A5, 16));
    golden("notes.de.typ", &out);
}
