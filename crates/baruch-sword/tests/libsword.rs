// SPDX-License-Identifier: AGPL-3.0-or-later
//! Nachbesserung Runde 17: verschlüsselte und BZIP2/XZ-Module **von libsword** (nicht nur von Baruchs eigener
//! Fixture). Vorher waren Verschlüssler (Fixture) und Entschlüssler (sword-modules) nach derselben Beschreibung
//! gebaut; ein Fehler in der Beschreibung wäre so unentdeckt geblieben.
//!
//! `tests/data/libsword/` enthält zwei Module, erzeugt mit `osis2mod` aus libsword 1.9.0 (Debian
//! `libsword-utils`) aus der frei erfundenen Datei `erfunden.osis.xml` und dem frei erfundenen Schlüssel [`KEY`]:
//!
//! ```text
//! osis2mod out/bzip2 erfunden.osis.xml -z b -b 3 -c Kiesel-Amsel-1951 -v German
//! osis2mod out/xz    erfunden.osis.xml -z x -b 3 -c Kiesel-Amsel-1951 -v German
//! ```
//!
//! Ein unkomprimiertes Modul (RawText) verschlüsselt `osis2mod` trotz `-c` nicht (die Einträge stehen im Klartext
//! in der Datei, auch libsword liest sie mit Schlüssel als Kauderwelsch) – RawText prüft deshalb nur die
//! Gegenrichtung mit `diatheke`.
//!
//! Jede Datei ist mit `xz -6 --check=crc32` gepackt abgelegt (die Indexdateien bestehen fast nur aus Nullen); der
//! Test entpackt sie mit lzma-rs in einen Wegwerf-Ordner und schreibt die conf (osis2mod schreibt keine). Ist
//! `diatheke` vorhanden, liest umgekehrt libsword ein von Baruchs Fixture verschlüsseltes Modul.

use std::path::{Path, PathBuf};
use std::process::Command;

use baruch_scripture::versification::german;
use baruch_sword::fixture::{Block, Compression, Fixture, Layout, write};
use baruch_sword::{BibleError, open, open_with_key, scan};

/// Frei erfundener Schlüssel (derselbe wie beim Erzeugen).
const KEY: &str = "Kiesel-Amsel-1951";

/// Verse aus `erfunden.osis.xml`.
const VERSES: [(&str, u16, u16, &str); 4] = [
    ("Gen", 1, 1, "Der Kiesel lag am Bach, und die Weiden standen still."),
    ("Gen", 1, 2, "Die Amsel sang über dem Tor, bis der Abend kam."),
    ("Ps", 23, 1, "Am Brunnen schöpfte der Hirte Wasser für die müden Schafe."),
    ("John", 3, 16, "Und der Wind ging über das Gras, und niemand wusste, woher."),
];

/// Modul `kind` (bzip2, xz) unter `root` anlegen; Name „Libsword<Kind>“.
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

/// Die Gegenrichtung: libsword (`diatheke`) liest ein Modul, das Baruchs Fixture mit Sapphire II verschlüsselt und
/// mit BZIP2 gepackt hat. Ohne `diatheke` (etwa in der CI) entfällt der Teil mit Hinweis.
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
        // diatheke kennt keinen Schlüssel-Parameter: Schlüssel in die conf (nur im Wegwerf-Ordner).
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
