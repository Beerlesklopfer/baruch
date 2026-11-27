// SPDX-License-Identifier: AGPL-3.0-or-later
//! Follow-up fix Round 17: enciphered and BZIP2/XZ modules **from libsword** (not only from Baruch's own
//! fixture). Before, encipherer (fixture) and decipherer (sword-modules) were built according to the same
//! description; an error in the description would thus have stayed undetected.
//!
//! `tests/data/libsword/` contains two modules, generated with `osis2mod` from libsword 1.9.0 (Debian
//! `libsword-utils`) from the freely invented file `erfunden.osis.xml` and the freely invented key [`KEY`]:
//!
//! ```text
//! osis2mod out/bzip2 erfunden.osis.xml -z b -b 3 -c Kiesel-Amsel-1951 -v German
//! osis2mod out/xz    erfunden.osis.xml -z x -b 3 -c Kiesel-Amsel-1951 -v German
//! ```
//!
//! `osis2mod` does not encipher an uncompressed module (RawText) despite `-c` (the entries are in plain text in
//! the file, libsword too reads them as gibberish with a key) – RawText therefore checks only the opposite
//! direction with `diatheke`.
//!
//! Every file is stored packed with `xz -6 --check=crc32` (the index files consist almost only of zeros); the
//! test unpacks them with lzma-rs into a throwaway folder and writes the conf (osis2mod writes none). If
//! `diatheke` is present, libsword conversely reads a module enciphered by Baruch's fixture.

use std::path::{Path, PathBuf};
use std::process::Command;

use baruch_scripture::versification::german;
use baruch_sword::fixture::{Block, Compression, Fixture, Layout, write};
use baruch_sword::{BibleError, open, open_with_key, scan};

/// Freely invented key (the same as when generating).
const KEY: &str = "Kiesel-Amsel-1951";

/// Verses from `erfunden.osis.xml`.
const VERSES: [(&str, u16, u16, &str); 4] = [
    ("Gen", 1, 1, "Der Kiesel lag am Bach, und die Weiden standen still."),
    ("Gen", 1, 2, "Die Amsel sang über dem Tor, bis der Abend kam."),
    ("Ps", 23, 1, "Am Brunnen schöpfte der Hirte Wasser für die müden Schafe."),
    ("John", 3, 16, "Und der Wind ging über das Gras, und niemand wusste, woher."),
];

/// Create the module `kind` (bzip2, xz) under `root`; name "Libsword<Kind>".
fn unpack(root: &Path, kind: &str) -> String {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/libsword").join(kind);
    let name = format!("Libsword{kind}");
    let (group, driver, extra) = match kind {
        "bzip2" => ("ztext", "zText", "CompressType=BZIP2\nBlockType=CHAPTER\n"),
        _ => ("ztext", "zText", "CompressType=XZ\nBlockType=CHAPTER\n"),
    };
    let data_path = format!("./modules/texts/{group}/{}/", name.to_lowercase());
    let dir = root.join(data_path.trim_start_matches("./"));
    std::fs::create_dir_all(&dir).unwrap();
    for entry in std::fs::read_dir(&src).unwrap() {
        let path = entry.unwrap().path();
        let file = path.file_stem().unwrap().to_owned();
        let mut packed = std::io::BufReader::new(std::fs::File::open(&path).unwrap());
        let mut out = Vec::new();
        lzma_rs::xz_decompress(&mut packed, &mut out).unwrap();
        std::fs::write(dir.join(file), out).unwrap();
    }
    std::fs::create_dir_all(root.join("mods.d")).unwrap();
    let conf = format!(
        "[{name}]\nDataPath={data_path}\nModDrv={driver}\n{extra}CipherKey=\nSourceType=OSIS\nEncoding=UTF-8\n\
         Versification=German\nDescription=Erfundene Testbibel (osis2mod)\n"
    );
    std::fs::write(root.join("mods.d").join(format!("{}.conf", name.to_lowercase())), conf).unwrap();
    name
}

fn library() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("sword");
    (dir, root)
}

#[test]
fn modules_enciphered_and_packed_by_libsword_are_read() {
    for kind in ["bzip2", "xz"] {
        let (_dir, root) = library();
        let name = unpack(&root, kind);
        let found = scan(std::slice::from_ref(&root));
        let info = found.find(&name).unwrap_or_else(|| panic!("{kind}: Modul gelistet"));
        assert!(
            info.enciphered && info.locked() && info.supported,
            "{kind}: verschlüsselt, gesperrt, lesbar: {:?}",
            info.problem
        );
        assert!(matches!(open(info), Err(BibleError::Locked(_))), "{kind}: ohne Schlüssel gesperrt");
        let bible = open_with_key(info, Some(KEY)).unwrap();
        assert!(bible.probe().is_ok(), "{kind}: richtiger Schlüssel besteht das Probelesen");
        for (book, c, v, text) in VERSES {
            assert_eq!(
                bible.verse(book, c, v).unwrap().as_deref(),
                Some(text),
                "{kind}: {book} {c},{v} von libsword verschlüsselt und gepackt, von Baruch gelesen"
            );
        }
        let wrong = open_with_key(info, Some("Amsel-Kiesel-1951")).unwrap();
        assert!(wrong.probe().is_err(), "{kind}: falscher Schlüssel fällt auf");
    }
}

/// The opposite direction: libsword (`diatheke`) reads a module that Baruch's fixture has enciphered with
/// Sapphire II and packed with BZIP2. Without `diatheke` (for instance in the CI) the part is skipped with a
/// note.
#[test]
fn libsword_reads_a_module_enciphered_by_the_fixture() {
    if Command::new("diatheke").arg("-b").arg("system").arg("-k").arg("modulelist").output().is_err() {
        eprintln!("diatheke fehlt: Gegenprobe mit libsword übersprungen");
        return;
    }
    for (compression, layout) in [
        (Compression::Bzip2, Layout::ZText(Block::Chapter)),
        (Compression::Xz, Layout::ZText(Block::Book)),
        (Compression::Zip, Layout::RawText),
    ] {
        let (dir, root) = library();
        let name = "BaruchFixtureSiegel";
        let f = Fixture::new(name, layout, german())
            .compression(compression)
            .verse("Gen", 1, 1, "Der Kiesel lag versiegelt am Bach.")
            .verse("Ps", 23, 1, "Die Amsel sang hinter dem Siegel.")
            .enciphered(KEY);
        write(&root, &f).unwrap();
        // diatheke knows no key parameter: key into the conf (only in the throwaway folder).
        let conf = root.join("mods.d/baruchfixturesiegel.conf");
        let text = std::fs::read_to_string(&conf).unwrap();
        std::fs::write(&conf, text.replace("CipherKey=\n", &format!("CipherKey={KEY}\n"))).unwrap();
        let home = dir.path().join("home");
        std::fs::create_dir_all(&home).unwrap();
        let out = Command::new("diatheke")
            .env("SWORD_PATH", &root)
            .env("HOME", &home)
            .args(["-b", name, "-o", "", "-f", "plain", "-k", "Ps 23:1"])
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(
            stdout.contains("Die Amsel sang hinter dem Siegel."),
            "{compression:?} {layout:?}: libsword entschlüsselt Baruchs Sapphire II: {stdout} {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
}
