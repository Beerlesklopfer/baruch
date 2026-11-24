// SPDX-License-Identifier: AGPL-3.0-or-later
//! Verschlüsselte SWORD-Module (R17c): nur selbst geschriebene Testmodule mit frei erfundenem Text, selbst mit
//! Sapphire II verschlüsselt (`fixture::Fixture::enciphered`) und mit einem frei erfundenen Testschlüssel. Kein
//! echtes oder fremdes Modul, kein geratener Schlüssel.

use std::path::PathBuf;

use baruch_scripture::versification::german;
use baruch_sword::fixture::{Block, Compression, Fixture, Layout, write};
use baruch_sword::{BibleError, open, open_commentary_with_key, open_with_key, scan};

/// Frei erfundener Testschlüssel.
const KEY: &str = "Kiesel-Amsel-1951";

fn sealed(layout: Layout) -> Fixture {
    Fixture::new("Siegel", layout, german())
        .conf("Description", "Erfundene verschlüsselte Testbibel")
        .conf("DistributionLicense", "Copyrighted; Freischaltung beim Herausgeber")
        .verse("Gen", 1, 1, "Der Kiesel lag versiegelt am Bach.")
        .verse("Ps", 23, 1, "Die Amsel sang hinter dem Siegel.")
        .verse("John", 3, 16, "Am Abend öffnete sich das Tor.")
        .enciphered(KEY)
}

fn written(f: &Fixture) -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("sword");
    write(&root, f).unwrap();
    (dir, root)
}

#[test]
fn an_enciphered_module_without_key_is_listed_as_locked_and_gives_no_text() {
    let (_dir, root) = written(&sealed(Layout::ZText(Block::Chapter)));
    let found = scan(std::slice::from_ref(&root));
    let info = found.find("Siegel").expect("verschlüsseltes Modul gelistet");
    assert!(info.enciphered, "CipherKey steht in der conf: verschlüsselt");
    assert!(info.locked(), "CipherKey ist leer: gesperrt, bis der Nutzer den Schlüssel hinterlegt");
    assert!(info.supported, "das Format kennt Baruch; nur der Schlüssel fehlt: {:?}", info.problem);
    match open(info) {
        Err(BibleError::Locked(name)) => assert_eq!(name, "Siegel", "Modulname im Grund"),
        other => panic!("ohne Schlüssel gesperrt erwartet: {other:?}"),
    }
    let why = BibleError::Locked("Siegel".into()).to_string();
    assert!(why.contains("verschlüsselt") && why.contains("Einstellungen"), "ehrlicher Hinweis mit Weg: {why}");
}

#[test]
fn the_right_key_unlocks_every_layout_and_compression_without_touching_the_conf() {
    let layouts = [
        (Layout::ZText(Block::Book), Compression::Zip),
        (Layout::ZText(Block::Chapter), Compression::Bzip2),
        (Layout::ZText(Block::Verse), Compression::Xz),
        (Layout::RawText, Compression::Zip),
    ];
    for (layout, compression) in layouts {
        let (_dir, root) = written(&sealed(layout).compression(compression));
        let conf_file = root.join("mods.d/siegel.conf");
        let before = std::fs::read_to_string(&conf_file).unwrap();
        let found = scan(std::slice::from_ref(&root));
        let info = found.find("Siegel").unwrap();
        let bible = open_with_key(info, Some(KEY)).unwrap_or_else(|e| panic!("{layout:?}: mit Schlüssel offen: {e}"));
        assert!(bible.probe().is_ok(), "{layout:?} {compression:?}: Probelesen gelingt mit dem richtigen Schlüssel");
        assert_eq!(
            bible.verse("Ps", 23, 1).unwrap().as_deref(),
            Some("Die Amsel sang hinter dem Siegel."),
            "{layout:?} {compression:?}: entschlüsselt und entpackt"
        );
        assert_eq!(
            bible.verse("John", 3, 16).unwrap().as_deref(),
            Some("Am Abend öffnete sich das Tor."),
            "{layout:?}: auch im Neuen Testament (eigene Dateien)"
        );
        assert_eq!(std::fs::read_to_string(&conf_file).unwrap(), before, "der Schlüssel steht nie in der conf-Datei");
        assert!(!before.contains(KEY), "Testschlüssel nicht in der conf");
    }
}

#[test]
fn the_stored_bytes_are_really_enciphered() {
    // Ohne Verschlüsselung stünde der Text bei RawText im Klartext in der Datei; mit Schlüssel nicht.
    let (_dir, root) = written(&sealed(Layout::RawText));
    let data = std::fs::read(root.join("modules/texts/rawtext/siegel/ot")).unwrap();
    let plain = "Der Kiesel lag versiegelt am Bach.".as_bytes();
    assert!(!data.windows(plain.len()).any(|w| w == plain), "Verstext nicht im Klartext gespeichert");
    assert_ne!(baruch_sword::fixture::encipher(KEY, b"abc"), b"abc", "Sapphire II ändert die Bytes");
    assert_ne!(
        baruch_sword::fixture::encipher(KEY, b"abc"),
        baruch_sword::fixture::encipher("anderer Schlüssel", b"abc"),
        "anderer Schlüssel, andere Bytes"
    );
}

#[test]
fn a_wrong_key_fails_the_probe_instead_of_giving_nonsense() {
    for layout in [Layout::ZText(Block::Chapter), Layout::RawText] {
        let (_dir, root) = written(&sealed(layout));
        let found = scan(std::slice::from_ref(&root));
        let info = found.find("Siegel").unwrap();
        let wrong = open_with_key(info, Some("falsch-geraten")).expect("Öffnen prüft den Schlüssel noch nicht");
        assert!(wrong.probe().is_err(), "{layout:?}: falscher Schlüssel fällt beim Probelesen auf");
        let read = wrong.verse("Gen", 1, 1);
        assert!(
            !matches!(read.as_ref().map(|t| t.as_deref()), Ok(Some("Der Kiesel lag versiegelt am Bach."))),
            "{layout:?}: falscher Schlüssel liest nie den Klartext: {read:?}"
        );
    }
}

/// Nachbesserung Runde 17: RawText in Latin-1 (SWORD-Vorgabe ohne `Encoding`, gelesen als cp1252) kennt kein
/// Ersatzzeichen; vorher prüfte das Probelesen dann nur auf NUL im ersten Eintrag, und ein falscher Schlüssel ging
/// oft als „gültig“ durch. Viele falsche Schlüssel: Keiner darf das Probelesen bestehen.
#[test]
fn a_wrong_key_fails_the_probe_of_an_unpacked_latin1_module() {
    let mut f = Fixture::new("Siegel", Layout::RawText, german()).latin1().enciphered(KEY);
    for (v, text) in [
        "Der Kiesel lag versiegelt am Bach, und die Weiden standen still.",
        "Die Amsel sang über dem Tor, bis der Abend über die Felder kam.",
        "Am Brunnen schöpfte der Hirte Wasser für die müden Schafe.",
        "Und der Wind ging über das Gras, und niemand wusste, woher.",
    ]
    .iter()
    .enumerate()
    {
        f = f.verse("Gen", 1, v as u16 + 1, text);
    }
    let (_dir, root) = written(&f);
    let conf = std::fs::read_to_string(root.join("mods.d/siegel.conf")).unwrap();
    assert!(!conf.contains("Encoding"), "conf ohne Encoding: Latin-1 ist die Vorgabe (Voraussetzung)");
    let found = scan(std::slice::from_ref(&root));
    let info = found.find("Siegel").unwrap();
    let right = open_with_key(info, Some(KEY)).unwrap();
    assert!(right.probe().is_ok(), "richtiger Schlüssel besteht");
    assert_eq!(
        right.verse("Gen", 1, 3).unwrap().as_deref(),
        Some("Am Brunnen schöpfte der Hirte Wasser für die müden Schafe."),
        "Latin-1 richtig gelesen (Umlaute)"
    );
    for n in 0..200 {
        let wrong = format!("falsch-{n}");
        let bible = open_with_key(info, Some(&wrong)).unwrap();
        assert!(bible.probe().is_err(), "falscher Schlüssel {wrong:?} fällt beim Probelesen auf");
    }
}

#[test]
fn an_unenciphered_module_ignores_a_key_and_a_key_in_the_conf_unlocks() {
    let plain = Fixture::new("Offen", Layout::ZText(Block::Chapter), german()).verse("Gen", 1, 1, "Offener Vers.");
    let (_dir, root) = written(&plain);
    let found = scan(std::slice::from_ref(&root));
    let info = found.find("Offen").unwrap();
    assert!(!info.enciphered && !info.locked(), "ohne CipherKey nicht verschlüsselt");
    let bible = open_with_key(info, Some(KEY)).unwrap();
    assert_eq!(bible.verse("Gen", 1, 1).unwrap().as_deref(), Some("Offener Vers."), "Schlüssel übergangen");

    // Hat ein anderes Programm den Schlüssel schon in die conf geschrieben, liest Baruch ohne Schlüsselbund.
    let (_dir2, root2) = written(&sealed(Layout::ZText(Block::Verse)));
    let conf = root2.join("mods.d/siegel.conf");
    let text = std::fs::read_to_string(&conf).unwrap();
    std::fs::write(&conf, text.replace("CipherKey=\n", &format!("CipherKey={KEY}\n"))).unwrap();
    let found = scan(std::slice::from_ref(&root2));
    let info = found.find("Siegel").unwrap();
    assert!(info.enciphered && !info.locked(), "Schlüssel in der conf: verschlüsselt, aber nicht gesperrt");
    assert_eq!(
        open(info).unwrap().verse("Gen", 1, 1).unwrap().as_deref(),
        Some("Der Kiesel lag versiegelt am Bach."),
        "mit dem Schlüssel der conf lesbar"
    );
}

#[test]
fn an_enciphered_commentary_opens_with_the_key() {
    let f = Fixture::new("Deutung", Layout::ZCom(Block::Chapter), german())
        .verse("Ps", 23, 1, "Erfundene Auslegung hinter dem Siegel.")
        .enciphered(KEY);
    let (_dir, root) = written(&f);
    let found = scan(std::slice::from_ref(&root));
    let info = found.find_commentary("Deutung").expect("verschlüsselter Kommentar gelistet");
    assert!(info.enciphered && info.supported, "verschlüsselt, Format lesbar: {:?}", info.problem);
    assert!(matches!(open_commentary_with_key(info, None), Err(BibleError::Locked(_))), "ohne Schlüssel gesperrt");
    let c = open_commentary_with_key(info, Some(KEY)).unwrap();
    let entries = c.entries("Ps", (23, 1), (23, 1)).unwrap();
    assert_eq!(entries.len(), 1, "ein Eintrag: {entries:?}");
    assert_eq!(entries[0].text, "Erfundene Auslegung hinter dem Siegel.", "entschlüsselt");
}
