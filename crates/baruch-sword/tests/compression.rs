// SPDX-License-Identifier: AGPL-3.0-or-later
//! Komprimierte SWORD-Module (R17c): BZIP2 und XZ neben ZIP, je Blockart, mit selbst geschriebenen
//! Testmodulen (`fixture::Fixture::compression`) und frei erfundenem Text.

use std::path::PathBuf;

use baruch_scripture::versification::german;
use baruch_sword::fixture::{Block, Compression, Fixture, Layout, write};
use baruch_sword::{open, open_commentary, scan};

fn written(f: &Fixture) -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("sword");
    write(&root, f).unwrap();
    (dir, root)
}

#[test]
fn bzip2_and_xz_modules_are_read_in_every_block_type() {
    for compression in [Compression::Zip, Compression::Bzip2, Compression::Xz] {
        for block in [Block::Book, Block::Chapter, Block::Verse] {
            let f = Fixture::new("Gepackt", Layout::ZText(block), german())
                .compression(compression)
                .verse("Gen", 1, 1, "Der Kiesel lag am Bach.")
                .verse("Gen", 1, 2, "Die Amsel sang dazu.")
                .verse("Gen", 2, 1, "Ein zweites Kapitel.")
                .verse("Rev", 22, 21, "Ganz am Ende.");
            let (_dir, root) = written(&f);
            let found = scan(std::slice::from_ref(&root));
            let info = found.find("Gepackt").unwrap();
            let what = format!("{compression:?} {block:?}");
            let expected = match compression {
                Compression::Zip => "ZIP",
                Compression::Bzip2 => "BZIP2",
                Compression::Xz => "XZ",
            };
            assert_eq!(info.compress, expected, "{what}: CompressType aus der conf");
            assert!(info.supported, "{what}: lesbar: {:?}", info.problem);
            let bible = open(info).unwrap();
            for (book, c, v, text) in [
                ("Gen", 1, 1, "Der Kiesel lag am Bach."),
                ("Gen", 1, 2, "Die Amsel sang dazu."),
                ("Gen", 2, 1, "Ein zweites Kapitel."),
                ("Rev", 22, 21, "Ganz am Ende."),
            ] {
                assert_eq!(bible.verse(book, c, v).unwrap().as_deref(), Some(text), "{what}: {book} {c},{v} entpackt");
            }
        }
    }
}

#[test]
fn a_compressed_commentary_is_read_too() {
    let f = Fixture::new("Gepresst", Layout::ZCom(Block::Verse), german()).compression(Compression::Xz).verse(
        "Ps",
        23,
        1,
        "Erfundene Auslegung, gepackt mit XZ.",
    );
    let (_dir, root) = written(&f);
    let found = scan(std::slice::from_ref(&root));
    let c = open_commentary(found.find_commentary("Gepresst").unwrap()).unwrap();
    let entries = c.entries("Ps", (23, 1), (23, 1)).unwrap();
    assert_eq!(entries.first().map(|e| e.text.as_str()), Some("Erfundene Auslegung, gepackt mit XZ."), "entpackt");
}

#[test]
fn an_unknown_compression_is_reported_with_its_name() {
    let f = Fixture::new("Fremd", Layout::ZText(Block::Chapter), german()).verse("Gen", 1, 1, "x");
    let (_dir, root) = written(&f);
    let conf = root.join("mods.d/fremd.conf");
    let text = std::fs::read_to_string(&conf).unwrap();
    std::fs::write(&conf, text.replace("CompressType=ZIP", "CompressType=ZSTD")).unwrap();
    let info = scan(std::slice::from_ref(&root)).find("Fremd").cloned().unwrap();
    assert!(!info.supported, "ZSTD kennt Baruch nicht");
    let why = info.problem.unwrap_or_default();
    assert!(
        why.contains("ZSTD") && why.contains("BZIP2") && why.contains("XZ"),
        "Grund nennt Art und Bekanntes: {why}"
    );
}

/// Nachbesserung Runde 17: Ein manipuliertes Modul (etwa aus einem fremden Modul-Repository) darf beim Entpacken
/// eines Blocks nicht beliebig viel Speicher belegen. sword-modules ist dafür in `vendor/` mit einer Obergrenze von
/// 64 MiB je Block eingebettet; ein Block, der zu 65 MiB Nullen entpackt (wenige Hundert Byte bzw. KiB gepackt),
/// gibt einen Fehler statt den Prozess per OOM zu beenden. Die Größe im Index (100 Byte) lügt dabei – geprüft wird
/// das Entpackte.
#[test]
fn a_decompression_bomb_is_refused_instead_of_filling_memory() {
    let bomb = vec![0u8; 65 << 20];
    for compression in [Compression::Zip, Compression::Bzip2, Compression::Xz] {
        let f = Fixture::new("Bombe", Layout::ZText(Block::Book), german()).compression(compression).verse(
            "Gen",
            1,
            1,
            "Der Kiesel lag am Bach.",
        );
        let (_dir, root) = written(&f);
        let dir = root.join("modules/texts/ztext/bombe");
        // lzma-rs packt XZ nur mit Literalen (kein Gewinn bei Nullen): dafür eine mit `xz -6 --check=crc32` aus
        // 65 MiB Nullen erzeugte Datei (10 KiB, kein Text).
        let packed = match compression {
            Compression::Xz => include_bytes!("data/nullen-65mib.xz").to_vec(),
            other => other.pack(&bomb).unwrap(),
        };
        assert!(packed.len() < 1 << 20, "{compression:?}: klein gepackt ({} Byte)", packed.len());
        let mut sizes = Vec::new();
        for v in [0u32, packed.len() as u32, 100] {
            sizes.extend_from_slice(&v.to_le_bytes());
        }
        std::fs::write(dir.join("ot.bzs"), sizes).unwrap();
        std::fs::write(dir.join("ot.bzz"), &packed).unwrap();
        let found = scan(std::slice::from_ref(&root));
        let bible = open(found.find("Bombe").unwrap()).unwrap();
        let read = bible.verse("Gen", 1, 1);
        let err = read.expect_err("Block über der Obergrenze: Fehler statt Speicher füllen").to_string();
        assert!(err.contains("64 MiB"), "{compression:?}: Grund nennt die Grenze: {err}");
        assert!(bible.probe().is_err(), "{compression:?}: auch das Probelesen scheitert sauber");
    }
}
