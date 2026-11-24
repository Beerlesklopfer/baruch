// SPDX-License-Identifier: AGPL-3.0-or-later
//! Kommentare und Lexika lesen (Runde 15, Punkt 5a): selbst geschriebene Mini-Module mit frei erfundenem Inhalt
//! (RawCom, zCom; RawLD, RawLD4, zLD), gelesen über `sword-modules`. Kommentar zu einer Stelle (auch über mehrere
//! Verse), Stichwort → Artikel, Stichwortliste, Suche, Klartext ohne Auszeichnung, Größengrenzen und nicht
//! lesbare Arten mit Grund.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use baruch_scripture::versification::german;
use baruch_sword::fixture::{Block, Fixture, Layout, LexLayout, LexiconFixture, write, write_lexicon};
use baruch_sword::{MAX_ENTRY_CHARS, SourceKind, open_commentary, open_lexicon, scan};

fn commentary(name: &str, layout: Layout) -> Fixture {
    Fixture::new(name, layout, german())
        .conf("Description", "Erfundener Testkommentar")
        .conf("DistributionLicense", "Public Domain")
        .verse("Gen", 1, 1, "<p>Der <hi type=\"bold\">Kiesel</hi> steht für Geduld.</p><note>nur Fußnote</note>")
        // Ein Eintrag für zwei Verse (gleicher Text an beiden Stellen).
        .verse("Ps", 139, 23, "Prüfen heißt hier: ans Licht halten.")
        .verse("Ps", 139, 24, "Prüfen heißt hier: ans Licht halten.")
        .verse("Matt", 5, 3, "Die Amsel singt auch im Regen.")
}

fn lexicon(name: &str, layout: LexLayout) -> LexiconFixture {
    LexiconFixture::new(name, layout)
        .conf("Description", "Erfundenes Testlexikon")
        .conf("DistributionLicense", "Copyrighted; Free non-commercial distribution")
        .article("Kiesel", "<p>Ein <b>kleiner</b> Stein am Bach &amp; im Feld.</p>")
        .article("Amsel", "Schwarzer Vogel, singt in der Dämmerung.")
        .article("Abend", "Zeit nach dem Tag; die Amsel singt.")
        .article("Bach", "Fließendes Wasser, kleiner als ein Fluss.")
        .article("Kieselstein", "@LINK Kiesel")
}

fn root() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("sword");
    (dir, root)
}

#[test]
fn commentaries_of_both_layouts_are_read_in_their_versification() {
    for (name, layout) in [("KomRaw", Layout::RawCom), ("KomZip", Layout::ZCom(Block::Chapter))] {
        let (_dir, root) = root();
        write(&root, &commentary(name, layout)).unwrap();
        let found = scan(std::slice::from_ref(&root));
        let info = found.find_commentary(name).unwrap_or_else(|| panic!("{name} gefunden: {found:?}"));
        assert_eq!(info.kind, SourceKind::Commentary, "{name} ist ein Kommentar");
        assert!(found.find(name).is_none(), "{name} steht nicht unter den Bibeln (Bibelwahl bleibt sauber)");
        assert!(info.supported, "{name} lesbar: {:?}", info.problem);
        assert_eq!(info.notice, "Public Domain", "Lizenzvermerk aus der conf");
        let c = open_commentary(info).unwrap();
        let entries = c.entries("Gen", (1, 1), (1, 1)).unwrap();
        assert_eq!(entries.len(), 1, "ein Eintrag zu 1. Mose 1,1: {entries:?}");
        assert_eq!(entries[0].text, "Der Kiesel steht für Geduld.", "Klartext ohne Auszeichnung und ohne Fußnote");
        let both = c.entries("Ps", (139, 23), (139, 24)).unwrap();
        assert_eq!(both.len(), 1, "gleicher Eintrag zweier Verse wird ein Bereich: {both:?}");
        assert_eq!((both[0].start, both[0].end), ((139, 23), (139, 24)), "Bereich Ps 139,23–24");
        let chapter = c.entries("Matt", (5, 0), (5, 0)).unwrap();
        assert_eq!(chapter.len(), 1, "ganzes Kapitel (Vers 0): nur Verse mit Eintrag");
        assert!(c.entries("Gen", (2, 1), (2, 3)).unwrap().is_empty(), "ohne Eintrag: leer, nichts erfunden");
        let (hits, cut) = c.search("ans licht", 10, Instant::now() + Duration::from_secs(5)).unwrap();
        assert_eq!(hits.len(), 1, "Suche findet den geteilten Eintrag einmal: {hits:?}");
        assert_eq!((hits[0].book.as_str(), hits[0].chapter, hits[0].verse), ("Ps", 139, 23), "erster Vers");
        assert!(!cut, "nicht abgeschnitten");
    }
}

#[test]
fn lexicons_of_all_three_layouts_give_articles_keys_and_hits() {
    for (name, layout) in [("LexRaw", LexLayout::RawLD), ("LexRaw4", LexLayout::RawLD4), ("LexZip", LexLayout::ZLD)] {
        let (_dir, root) = root();
        write_lexicon(&root, &lexicon(name, layout)).unwrap();
        let found = scan(std::slice::from_ref(&root));
        let info = found.find_lexicon(name).unwrap_or_else(|| panic!("{name} gefunden: {found:?}"));
        assert_eq!(info.kind, SourceKind::Lexicon, "{name} ist ein Lexikon");
        assert!(info.supported, "{name} lesbar: {:?}", info.problem);
        assert!(info.versification.is_empty(), "Lexika haben keine Verszählung");
        let lex = open_lexicon(info).unwrap();
        assert_eq!(lex.len(), 5, "fünf Stichwörter");
        let kiesel = lex.lookup("kiesel").unwrap().expect("Stichwort ohne Rücksicht auf Groß-/Kleinschreibung");
        assert_eq!(kiesel.text, "Ein kleiner Stein am Bach & im Feld.", "{name}: Klartext, Entität aufgelöst");
        assert_eq!(kiesel.key, "KIESEL", "Stichwort, wie es im Modul steht");
        let linked = lex.lookup("Kieselstein").unwrap().expect("@LINK gefolgt");
        assert_eq!(linked.text, kiesel.text, "{name}: Verweis zeigt den Artikel des Ziels");
        assert!(lex.lookup("Elefant").unwrap().is_none(), "unbekanntes Stichwort: nichts erfunden");
        let keys = lex.keys("", 100).unwrap();
        assert_eq!(keys.keys, vec!["ABEND", "AMSEL", "BACH", "KIESEL", "KIESELSTEIN"], "{name}: sortiert");
        let prefixed = lex.keys("ki", 1).unwrap();
        assert_eq!((prefixed.keys.len(), prefixed.truncated), (1, true), "Präfix mit Grenze: {prefixed:?}");
        assert_eq!(prefixed.keys[0], "KIESEL", "erstes Stichwort mit Präfix");
        let (hits, _) = lex.search("amsel", 10, Instant::now() + Duration::from_secs(5)).unwrap();
        assert_eq!(
            hits.first().map(|h| (h.key.as_str(), h.in_key)),
            Some(("AMSEL", true)),
            "Stichwort zuerst: {hits:?}"
        );
        assert!(hits.iter().any(|h| h.key == "ABEND" && !h.in_key), "Treffer im Text des Artikels auch: {hits:?}");
    }
}

#[test]
fn long_entries_are_cut_and_say_so() {
    let (_dir, root) = root();
    let long = "Wort ".repeat(MAX_ENTRY_CHARS / 4);
    write_lexicon(&root, &LexiconFixture::new("LexLang", LexLayout::RawLD4).article("Lang", &long)).unwrap();
    let found = scan(std::slice::from_ref(&root));
    let lex = open_lexicon(found.find_lexicon("LexLang").unwrap()).unwrap();
    let article = lex.lookup("Lang").unwrap().unwrap();
    assert!(article.truncated, "länger als die Grenze: gekürzt markiert");
    assert_eq!(article.text.chars().count(), MAX_ENTRY_CHARS, "genau auf die Grenze gekürzt (mit …)");
}

#[test]
fn unreadable_kinds_are_listed_with_a_reason() {
    let (_dir, root) = root();
    std::fs::create_dir_all(root.join("mods.d")).unwrap();
    std::fs::write(
        root.join("mods.d/notizen.conf"),
        "[Notizen]\nDataPath=./modules/comments/rawfiles/notizen/\nModDrv=RawFiles\nDescription=Persönliche Notizen\n",
    )
    .unwrap();
    std::fs::write(
        root.join("mods.d/verweise.conf"),
        "[Verweise]\nDataPath=./modules/comments/hrefcom/verweise/\nModDrv=HREFCom\n",
    )
    .unwrap();
    std::fs::write(
        root.join("mods.d/fremdzaehlung.conf"),
        "[Fremdzaehlung]\nDataPath=./modules/comments/rawcom/f/\nModDrv=RawCom\nVersification=Phantasie\n",
    )
    .unwrap();
    std::fs::write(
        root.join("mods.d/geheim.conf"),
        "[Geheim]\nDataPath=./modules/lexdict/rawld/geheim/geheim\nModDrv=RawLD\nCipherKey=abc\n",
    )
    .unwrap();
    let found = scan(std::slice::from_ref(&root));
    for (name, why) in [("Notizen", "RawFiles"), ("Verweise", "HREFCom"), ("Fremdzaehlung", "Verszählung")] {
        let info = found.find_commentary(name).unwrap_or_else(|| panic!("{name} gelistet: {found:?}"));
        assert!(!info.supported, "{name} nicht lesbar");
        assert!(
            info.problem.as_deref().is_some_and(|p| p.contains(why)),
            "{name}: Grund nennt {why}: {:?}",
            info.problem
        );
        assert!(open_commentary(info).is_err(), "{name}: Öffnen scheitert ehrlich statt Unsinn zu lesen");
    }
    let secret = found.find_lexicon("Geheim").expect("verschlüsseltes Lexikon gelistet");
    // Seit R17c lehnt Baruch verschlüsselte Module nicht mehr ab: Die conf trägt hier schon einen Schlüssel.
    assert!(secret.enciphered, "CipherKey in der conf: verschlüsselt");
    assert!(secret.supported && !secret.locked(), "Schlüssel steht in der conf: lesbar: {:?}", secret.problem);
}
