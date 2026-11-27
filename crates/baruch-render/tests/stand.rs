// SPDX-License-Identifier: AGPL-3.0-or-later
//! `baruch_render::stand`: changes with every byte of `main` or of the files and only then.

use baruch_render::{Source, stand};

fn sample() -> Source {
    Source {
        main: "= Kapitel\nÄpfel\n".into(),
        files: vec![("bilder/a.svg".into(), b"<svg/>".to_vec()), ("b.png".into(), vec![0, 1, 2])],
    }
}

#[test]
fn the_same_source_always_has_the_same_stand() {
    assert_eq!(stand(&sample()), stand(&sample()), "gleiche Eingabe → gleicher Cache-Schlüssel");
    assert_eq!(stand(&sample()).len(), 64, "SHA-256 als Hex hat 64 Zeichen");
    let mut swapped = sample();
    swapped.files.reverse();
    assert_eq!(stand(&swapped), stand(&sample()), "die Reihenfolge der Dateien ist kein Inhalt");
}

#[test]
fn every_byte_of_main_changes_the_stand() {
    let base = stand(&sample());
    let bytes = sample().main.into_bytes();
    for i in 0..bytes.len() {
        let mut changed = bytes.clone();
        changed[i] ^= 0x01;
        // No longer valid UTF-8? Then another change at the same place.
        let main = String::from_utf8(changed).unwrap_or_else(|_| {
            let mut s = sample().main;
            s.insert(i, 'x');
            s
        });
        let source = Source { main, ..sample() };
        assert_ne!(stand(&source), base, "Byte {i} von main geändert, der Stand muss sich ändern");
    }
}

#[test]
fn every_byte_of_a_file_its_path_and_the_file_list_change_the_stand() {
    let base = stand(&sample());
    for (f, (_, bytes)) in sample().files.iter().enumerate() {
        for i in 0..bytes.len() {
            let mut source = sample();
            source.files[f].1[i] ^= 0x80;
            assert_ne!(stand(&source), base, "Byte {i} von Datei {f} geändert, der Stand muss sich ändern");
        }
    }
    let mut renamed = sample();
    renamed.files[0].0 = "bilder/c.svg".into();
    assert_ne!(stand(&renamed), base, "ein anderer Pfad ist ein anderes Werk");
    let mut fewer = sample();
    fewer.files.pop();
    assert_ne!(stand(&fewer), base, "eine fehlende Datei ändert den Stand");
    // Shifting boundaries between fields (length in front): "ab"+"c" is not "a"+"bc".
    let a = Source { main: String::new(), files: vec![("ab".into(), b"c".to_vec())] };
    let b = Source { main: String::new(), files: vec![("a".into(), b"bc".to_vec())] };
    assert_ne!(stand(&a), stand(&b), "Pfad und Inhalt dürfen nicht ineinander verschwimmen");
}
