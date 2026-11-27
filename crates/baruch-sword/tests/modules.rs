// SPDX-License-Identifier: AGPL-3.0-or-later
//! Read SWORD modules: self-written mini modules (zText in BOOK/CHAPTER/VERSE, RawText) with freely
//! invented text in the versifications German and KJV. The index slots are computed by `fixture::slot` from the
//! numbers of `baruch-scripture`; reading goes through `sword-modules`. Plus conf, DataPath, unsupported
//! versifications and the search.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use baruch_scripture::versification::{GERMAN, KJV, Versification, german, versification};
use baruch_sword::fixture::{Block, Fixture, Key, Layout, slot, slot_count, write};
use baruch_sword::{BibleError, open, scan};

fn kjv() -> &'static Versification {
    versification(KJV).unwrap()
}

/// Entries at all kinds of slots: headers, book introduction, chapter introduction, first and last verses.
fn sample(name: &str, layout: Layout, v11n: &'static Versification) -> Fixture {
    let last_of_mal = if v11n.name == GERMAN { (3, 24) } else { (4, 6) };
    Fixture::new(name, layout, v11n)
        .conf("Description", "Erfundene Testbibel")
        .conf("DistributionLicense", "Public Domain")
        .entry(Key::Module(0), "Kopf AT")
        .entry(Key::Testament(0), "Altes Testament")
        .entry(Key::Module(1), "Kopf NT")
        .entry(Key::Testament(1), "Neues Testament")
        .verse("Gen", 0, 0, "Einleitung zum ersten Buch")
        .verse("Gen", 1, 0, "Einleitung zu Kapitel eins")
        .verse("Gen", 1, 1, "Der Kiesel lag am Bach.")
        .verse("Gen", 1, 31, "Am Abend sang die Amsel.")
        .verse("Gen", 2, 1, "Zweites Kapitel, erster Vers.")
        .verse("Exod", 1, 1, "Ein neues Buch beginnt.")
        .verse("Ps", 119, 176, "Der letzte Vers des langen Liedes.")
        .verse("Mal", last_of_mal.0, last_of_mal.1, "Ende des Alten Testaments.")
        .verse("Matt", 1, 1, "Anfang des Neuen Testaments.")
        .verse("Rev", 22, 21, "Ganz am Ende.")
}

fn written(f: &Fixture) -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("sword");
    write(&root, f).unwrap();
    (dir, root)
}

fn opened(root: &Path, name: &str) -> baruch_sword::Bible {
    let found = scan(&[root.to_path_buf()]);
    let info = found.find(name).unwrap_or_else(|| panic!("{name} gefunden: {found:?}"));
    assert!(info.supported, "{name} lesbar: {:?}", info.problem);
    open(info).unwrap()
}

#[test]
fn the_slot_layout_follows_module_testament_book_and_chapter_entries() {
    let g = german();
    // Slot 0 module header, 1 testament header, 2 book introduction, 3 chapter introduction, 4 first verse.
    assert_eq!(slot(g, &Key::Verse("Gen", 0, 0)), Some((0, 2)), "Bucheinleitung 1. Mose");
    assert_eq!(slot(g, &Key::Verse("Gen", 1, 0)), Some((0, 3)), "Kapiteleinleitung 1. Mose 1");
    assert_eq!(slot(g, &Key::Verse("Gen", 1, 1)), Some((0, 4)), "1. Mose 1,1 an Stelle 4");
    assert_eq!(slot(g, &Key::Verse("Gen", 2, 0)), Some((0, 4 + 31)), "nach 31 Versen die Einleitung von Kapitel 2");
    assert_eq!(slot(g, &Key::Verse("Matt", 1, 1)), Some((1, 4)), "Mt 1,1 ebenfalls an Stelle 4 (eigenes Testament)");
    assert_eq!(slot(g, &Key::Verse("Gen", 1, 32)), None, "1. Mose 1 hat 31 Verse");
    // Number of slots: 2 + books + chapters + verses. Observed on the module GerSch (Versification=German):
    // ot.bzv has 241850 bytes = 24185 slots of 10 bytes, nt.bzv 82460 bytes = 8246 slots.
    assert_eq!(slot_count(g, 0), 2 + 39 + 929 + 23215, "Stellen AT (German)");
    assert_eq!((slot_count(g, 0), slot_count(g, 1)), (24185, 8246), "wie die Indexdateien von GerSch");
    assert_eq!(slot_count(kjv(), 0), 2 + 39 + 929 + 23145, "Stellen AT (KJV)");
}

#[test]
fn every_layout_reads_every_kind_of_slot_in_both_versifications() {
    let layouts =
        [Layout::ZText(Block::Book), Layout::ZText(Block::Chapter), Layout::ZText(Block::Verse), Layout::RawText];
    for v11n in [german(), kjv()] {
        for layout in layouts {
            let f = sample("Probe", layout, v11n);
            let (_dir, root) = written(&f);
            let bible = opened(&root, "Probe");
            let what = format!("{layout:?} {}", v11n.name);
            assert_eq!(bible.v11n.name, v11n.name, "{what}: Zählung aus der conf");
            for (key, text) in &f.entries {
                let read = match key {
                    Key::Module(t) => bible.slot_text(*t, 0).unwrap(),
                    Key::Testament(t) => bible.slot_text(*t, 1).unwrap(),
                    Key::Verse(b, c, 0) => bible.intro(b, *c).unwrap(),
                    Key::Verse(b, c, v) => bible.verse(b, *c, *v).unwrap(),
                };
                assert_eq!(read.as_deref(), Some(text.as_str()), "{what}: {key:?} an der berechneten Stelle");
            }
            assert_eq!(bible.verse("Gen", 1, 2).unwrap(), None, "{what}: leere Stelle hat keinen Text");
            assert_eq!(bible.verse("Gen", 1, 32).unwrap(), None, "{what}: Vers außerhalb der Zählung");
        }
    }
}

#[test]
fn german_and_kjv_differ_where_the_counting_differs() {
    // Joel 3,1 (KJV) is Joel 4,1 (German): same numbers, another slot in the index.
    let g =
        Fixture::new("DeuProbe", Layout::ZText(Block::Chapter), german()).verse("Joel", 4, 1, "Erfundener Joel-Vers.");
    let (_d, root) = written(&g);
    let bible = opened(&root, "DeuProbe");
    assert_eq!(bible.verse("Joel", 4, 1).unwrap().as_deref(), Some("Erfundener Joel-Vers."), "Joel 4 gibt es deutsch");
    let k = Fixture::new("EngProbe", Layout::ZText(Block::Chapter), kjv()).verse("Joel", 3, 1, "Erfundener Vers.");
    let (_d2, root2) = written(&k);
    let bible = opened(&root2, "EngProbe");
    assert_eq!(bible.verse("Joel", 4, 1).unwrap(), None, "Joel hat in KJV nur 3 Kapitel");
    assert_eq!(bible.verse("Joel", 3, 1).unwrap().as_deref(), Some("Erfundener Vers."), "KJV Joel 3,1");
}

#[test]
fn the_tables_of_sword_modules_equal_ours_for_german_and_kjv() {
    // The reader computes the slots with its own tables; they must agree with ours verse by verse,
    // otherwise every verse would lie at a wrong slot.
    for ours in [german(), kjv()] {
        let theirs = sword_modules::Versification::get(ours.name).expect("Zählung bekannt");
        let mut books = 0;
        for (id, chapters) in ours.books() {
            let key = sword_modules::VerseKey::from_osis(id, 1, 1).unwrap_or_else(|| panic!("OSIS {id} bekannt"));
            assert_eq!(
                theirs.chapter_count(key.book).map(u16::from),
                Some(chapters.len() as u16),
                "{} {id}: Kapitel",
                ours.name
            );
            for (c, &n) in chapters.iter().enumerate() {
                assert_eq!(
                    theirs.verse_count(key.book, (c + 1) as u8).map(u16::from),
                    Some(n),
                    "{} {id} {}: Verse",
                    ours.name,
                    c + 1
                );
            }
            assert_eq!(key.osis_book(), Some(id), "OSIS-Kürzel {id} unverändert zurück");
            books += 1;
        }
        assert_eq!((books, theirs.books().len()), (66, 66), "{}: 66 Bücher", ours.name);
    }
}

#[test]
fn the_conf_is_read_and_data_path_is_relative_to_the_module_folder() {
    let f = sample("Kiesel", Layout::ZText(Block::Book), german())
        .conf("About", "Erste Zeile\\par Zweite Zeile")
        .conf("Lang", "de")
        .conf("Copyright", "frei erfunden")
        .conf("Version", "1.0");
    let (_dir, root) = written(&f);
    let found = scan(std::slice::from_ref(&root));
    let info = found.find("kiesel").expect("Name ohne Rücksicht auf Groß/Klein");
    assert_eq!(info.description, "Erfundene Testbibel", "Description");
    assert!(
        info.about.contains("Erste Zeile") && info.about.contains("Zweite Zeile"),
        "About als Text: {}",
        info.about
    );
    assert!(!info.about.contains("\\par"), "RTF-Marke entfernt: {}", info.about);
    assert_eq!((info.lang.as_str(), info.versification.as_str()), ("de", GERMAN), "Lang und Versification");
    assert_eq!(
        (info.driver.as_str(), info.compress.as_str(), info.block_type.as_str()),
        ("zText", "ZIP", "BOOK"),
        "Speicherart"
    );
    assert_eq!(info.notice, "Public Domain; frei erfunden", "Lizenzvermerk aus DistributionLicense und Copyright");
    assert_eq!(info.dir, root, "Modulordner");
    // The test runs in the folder of the crate; reading nevertheless happens from the module folder.
    assert!(!Path::new("modules").exists(), "kein Ordner modules im aktuellen Verzeichnis");
    let bible = open(info).unwrap();
    assert_eq!(bible.verse("Gen", 1, 1).unwrap().as_deref(), Some("Der Kiesel lag am Bach."), "Text über DataPath");
}

#[test]
fn a_data_path_outside_the_module_folder_is_refused_and_a_leading_slash_stays_inside() {
    let f = sample("Ausbruch", Layout::RawText, german());
    let (_dir, root) = written(&f);
    let conf = root.join("mods.d/ausbruch.conf");
    let text = std::fs::read_to_string(&conf).unwrap();
    std::fs::write(&conf, text.replace("DataPath=./modules/", "DataPath=../modules/")).unwrap();
    let info = scan(std::slice::from_ref(&root)).find("Ausbruch").cloned().unwrap();
    assert!(!info.supported, "„..“ verlässt den Modulordner");
    assert!(info.problem.as_deref().unwrap().contains("verlässt den Modulordner"), "Grund: {:?}", info.problem);
    assert!(matches!(open(&info), Err(BibleError::Unsupported(_))), "nicht zu öffnen");

    std::fs::write(&conf, text.replace("DataPath=./modules/", "DataPath=/modules/")).unwrap();
    let bible = opened(&root, "Ausbruch");
    assert_eq!(
        bible.verse("Rev", 22, 21).unwrap().as_deref(),
        Some("Ganz am Ende."),
        "„/modules/…“ im Modulordner gelesen"
    );
}

#[test]
fn other_versifications_drivers_and_ciphers_are_reported_as_unsupported() {
    let f = sample("Luthertest", Layout::RawText, german());
    let (_dir, root) = written(&f);
    let conf = root.join("mods.d/luthertest.conf");
    let text = std::fs::read_to_string(&conf).unwrap();
    std::fs::write(&conf, text.replace("Versification=German", "Versification=Phantasie")).unwrap();
    let info = scan(std::slice::from_ref(&root)).find("Luthertest").cloned().unwrap();
    assert!(!info.supported, "Phantasie gibt es nicht");
    assert!(info.problem.as_deref().unwrap().contains("Phantasie"), "Grund nennt die Zählung: {:?}", info.problem);
    match open(&info) {
        Err(BibleError::Unsupported(why)) => assert!(why.contains("kennt Baruch nicht"), "ehrliche Meldung: {why}"),
        other => panic!("nicht unterstützt erwartet: {other:?}"),
    }
    std::fs::write(&conf, text.replace("Versification=German", "Versification=German\nCipherKey=")).unwrap();
    let info = scan(std::slice::from_ref(&root)).find("Luthertest").cloned().unwrap();
    assert!(info.enciphered && info.supported, "verschlüsselt (R17c): gelistet, Format lesbar: {:?}", info.problem);
    assert!(matches!(open(&info), Err(BibleError::Locked(_))), "ohne Schlüssel gesperrt statt Unsinn zu lesen");
    std::fs::write(&conf, text.replace("ModDrv=RawText", "ModDrv=zText\nCompressType=LZMA")).unwrap();
    let info = scan(std::slice::from_ref(&root)).find("Luthertest").cloned().unwrap();
    assert!(info.problem.as_deref().unwrap().contains("LZMA"), "unbekannte Kompression: {:?}", info.problem);
}

#[test]
fn scanning_skips_missing_folders_commentaries_and_duplicate_names() {
    let (_d1, root1) = written(&sample("Doppelt", Layout::RawText, german()));
    let (_d2, root2) = written(&sample("Doppelt", Layout::ZText(Block::Verse), kjv()));
    std::fs::write(root1.join("mods.d/kommentar.conf"), "[Notizen]\nDataPath=./modules/comments/x/\nModDrv=zCom\n")
        .unwrap();
    let missing = root1.join("gibt-es-nicht");
    let found = scan(&[missing.clone(), root1.clone(), root2.clone()]);
    assert_eq!(found.modules.len(), 1, "eine Bibel (Kommentar übergangen, Name doppelt): {found:?}");
    assert_eq!(found.modules[0].versification, GERMAN, "der erste Ordner gewinnt");
    assert!(
        found.problems.iter().any(|p| p.contains("kein Ordner mods.d")),
        "fehlender Ordner gemeldet: {:?}",
        found.problems
    );
    assert!(
        found.problems.iter().any(|p| p.contains("früheren Ordner")),
        "doppelter Name gemeldet: {:?}",
        found.problems
    );
}

#[test]
fn osis_markup_becomes_plain_text_in_verses() {
    let f = Fixture::new("Auszeichnung", Layout::ZText(Block::Chapter), german()).verse(
        "Ps",
        23,
        1,
        "<title canonical=\"true\">Ein Lied.</title> Der <divineName>Hirte</divineName> wacht\
         <note type=\"x-footnote\">Fußnote</note>.",
    );
    let (_dir, root) = written(&f);
    let bible = opened(&root, "Auszeichnung");
    assert_eq!(
        bible.verse("Ps", 23, 1).unwrap().as_deref(),
        Some("Ein Lied. Der Hirte wacht."),
        "Klartext ohne Fußnote"
    );
}

#[test]
fn a_passage_lists_every_verse_and_marks_missing_ones() {
    let (_dir, root) = written(&sample("Bereich", Layout::ZText(Block::Book), german()));
    let bible = opened(&root, "Bereich");
    let verses = bible.passage("Gen", (1, 31), (2, 1)).unwrap();
    let got: Vec<(u16, u16, bool)> = verses.iter().map(|v| (v.chapter, v.verse, v.text.is_some())).collect();
    assert_eq!(got, vec![(1, 31, true), (2, 1, true)], "über die Kapitelgrenze");
    let chapter = bible.passage("Gen", (2, 0), (2, 0)).unwrap();
    assert_eq!(chapter.len(), 25, "ganzes Kapitel 2 hat 25 Verse");
    assert_eq!(chapter.iter().filter(|v| v.text.is_some()).count(), 1, "nur 2,1 hat Text");
    assert!(matches!(bible.passage("Gen", (51, 1), (51, 1)), Err(BibleError::Unsupported(_))), "Kapitel 51 fehlt");
}

#[test]
fn search_finds_a_phrase_ignoring_case_and_punctuation_and_stops_at_the_limit() {
    let (_dir, root) = written(&sample("Suche", Layout::ZText(Block::Chapter), german()));
    let bible = opened(&root, "Suche");
    let far = Instant::now() + Duration::from_secs(10);
    let found = bible.search("kiesel LAG", 10, far).unwrap();
    assert_eq!(found.hits.len(), 1, "ein Treffer: {found:?}");
    assert_eq!((found.hits[0].book.as_str(), found.hits[0].chapter, found.hits[0].verse), ("Gen", 1, 1), "1. Mose 1,1");
    assert!(!found.truncated, "nicht abgeschnitten");
    let many = bible.search("am", 1, far).unwrap();
    assert_eq!(many.hits.len(), 1, "höchstens ein Treffer");
    assert!(many.truncated, "Grenze erreicht");
    assert!(bible.search("…", 5, far).unwrap().hits.is_empty(), "leere Suche findet nichts");
}

#[test]
fn strong_numbers_of_a_verse_come_in_text_order_and_the_conf_says_whether_there_are_any() {
    let f = Fixture::new("Starke", Layout::ZText(Block::Chapter), german())
        .conf("GlobalOptionFilter", "OSISStrongs")
        .verse(
            "Gen",
            1,
            1,
            "<w lemma=\"strong:H07225\">Im Anfang</w> <w lemma=\"strong:H0430\">schuf Gott</w> \
             <w lemma=\"strong:H7225\">wieder</w>.",
        )
        .verse("John", 3, 16, "<w lemma=\"strong:G0026\">Liebe</w> <w lemma=\"strong:G2316 strong:G3588\">Gott</w>")
        .verse("Gen", 1, 2, "Ohne Auszeichnung.");
    let (_dir, root) = written(&f);
    let bible = opened(&root, "Starke");
    assert!(bible.has_strongs(), "GlobalOptionFilter=OSISStrongs: das Modul trägt Strong-Nummern");
    assert_eq!(bible.verse_strongs("Gen", 1, 1).unwrap(), ["H7225", "H430"], "ohne Nullen, Doppel nur einmal");
    assert_eq!(bible.verse_strongs("John", 3, 16).unwrap(), ["G26", "G2316", "G3588"], "Reihenfolge des Textes");
    assert!(bible.verse_strongs("Gen", 1, 2).unwrap().is_empty(), "ohne Auszeichnung leer");
    assert!(bible.verse_strongs("Gen", 1, 3).unwrap().is_empty(), "ohne Text leer");
    assert!(bible.verse_strongs("Gen", 1, 99).unwrap().is_empty(), "außerhalb der Zählung leer");

    let plain = Fixture::new("Schlicht", Layout::RawText, german()).verse("Gen", 1, 1, "Ohne Nummern.");
    let (_d2, root2) = written(&plain);
    assert!(!opened(&root2, "Schlicht").has_strongs(), "ohne Filter und Feature keine Strong-Nummern");
    let feature = Fixture::new("Merkmal", Layout::RawText, german()).conf("Feature", "StrongsNumbers").verse(
        "Gen",
        1,
        1,
        "Wort<WH0430>",
    );
    let (_d3, root3) = written(&feature);
    let bible = opened(&root3, "Merkmal");
    assert!(bible.has_strongs(), "Feature=StrongsNumbers zählt auch");
    assert_eq!(
        bible.verse_strongs("Gen", 1, 1).unwrap(),
        ["H430"],
        "GBF-Marke <WH0430> im Modul gelesen (Marken werden unabhängig von SourceType erkannt)"
    );
}
