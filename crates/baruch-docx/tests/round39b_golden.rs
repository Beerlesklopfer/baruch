// SPDX-License-Identifier: AGPL-3.0-or-later
//! Round 39b (ausgaben, contract §4.1): golden German Word files. The fixtures under `tests/data/round39b/` were
//! generated with the unchanged S1 code (commit 5c5a821) before any work-language change; every later change must keep
//! them byte-equal, because German works must not change. The writer is deterministic, so the whole archive is
//! compared by size and hash and `document.xml`, `styles.xml` and `docProps/core.xml` as text.
//!
//! Regenerating them is forbidden after the first commit of the round. The environment variable `BARUCH_BLESS_R39B`
//! only exists for that first commit (it writes the files instead of comparing).

use std::io::{Cursor, Read};
use std::path::PathBuf;

use baruch_docx::sample::sample_document;
use baruch_docx::write::{WriteOptions, write};

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

/// FNV-1a, 64 bit: enough to notice a changed archive without storing it.
fn fnv(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, b| (hash ^ u64::from(*b)).wrapping_mul(0x0100_0000_01b3))
}

fn part(bytes: &[u8], name: &str) -> String {
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).expect("the file is a ZIP archive");
    let mut entry = zip.by_name(name).unwrap_or_else(|_| panic!("part {name} is in the archive"));
    let mut text = String::new();
    entry.read_to_string(&mut text).expect("the part is UTF-8");
    text
}

#[test]
fn the_word_file_stays_german_for_a_german_work() {
    let options = WriteOptions { created: "2026-11-25T08:00:00Z".into(), generator: "Baruch Probe".into() };
    let bytes = write(&sample_document(), &options).expect("the sample document is written");
    let mut out = format!("[{} bytes, fnv {:016x}]\n", bytes.len(), fnv(&bytes));
    for name in ["word/document.xml", "word/styles.xml", "docProps/core.xml"] {
        out.push_str(&format!("=== {name}\n{}\n", part(&bytes, name)));
    }
    golden("docx_sample.de.txt", &out);
}
