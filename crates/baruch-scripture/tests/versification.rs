// SPDX-License-Identifier: AGPL-3.0-or-later
//! Die eingebettete Verszählung: 66 Bücher in deutscher Zählung, nur Zahlen, Quelle im Kopf und in `SOURCE`.

use baruch_scripture::{SOURCE, books};

const TABLE: &str = include_str!("../data/versification.tsv");

#[test]
fn the_table_has_66_books_and_the_counts_of_the_source() {
    let all = books();
    assert_eq!(all.len(), 66, "39 + 27 Bücher");
    let chapters: usize = all.iter().map(|b| b.verses.len()).sum();
    let verses: u32 = all.iter().flat_map(|b| b.verses.iter()).map(|&v| u32::from(v)).sum();
    // Zahlen der Quelle (SWORD „German“, ausgezählt am 2026-11-06): 1189 Kapitel, 31172 Verse. Die
    // englische Zählung (KJV) hätte 31102 Verse; daran ist eine verwechselte Quelle zu erkennen.
    assert_eq!(chapters, 1189, "Kapitel insgesamt");
    assert_eq!(verses, 31172, "Verse insgesamt (deutsche Zählung)");
    let chapters_of = |id: &str| all.iter().find(|b| b.id == id).unwrap().chapters();
    assert_eq!((chapters_of("Joel"), chapters_of("Mal")), (4, 3), "Joel 4, Maleachi 3 Kapitel (deutsch)");
    assert_eq!((all[0].id, all[0].name), ("Gen", "1. Mose"), "erstes Buch");
    assert_eq!((all[65].id, all[65].name), ("Rev", "Offenbarung"), "letztes Buch");
    let psalms = all.iter().find(|b| b.id == "Ps").unwrap();
    assert_eq!((psalms.chapters(), psalms.verses_in(119)), (150, Some(176)), "Psalm 119 hat 176 Verse");
    assert_eq!(psalms.verses_in(0), None, "Kapitel 0 gibt es nicht");
}

#[test]
fn the_table_contains_only_numbers_and_names_its_source_and_licence() {
    for (n, line) in TABLE.lines().enumerate() {
        if line.starts_with('#') {
            continue;
        }
        let cols: Vec<&str> = line.split('\t').collect();
        assert_eq!(cols.len(), 3, "Zeile {}: Buch, Kapitel, Verse", n + 1);
        assert!(cols[0].chars().all(|c| c.is_ascii_alphanumeric()), "Zeile {}: OSIS-Kürzel", n + 1);
        assert!(cols[1..].iter().all(|c| c.chars().all(|d| d.is_ascii_digit())), "Zeile {}: nur Zahlen", n + 1);
    }
    let head: String = TABLE.lines().take_while(|l| l.starts_with('#')).collect::<Vec<_>>().join("\n");
    for needle in ["CrossWire", "canon_german.h", "Versification=German", "SHA-256", "abgerufen am 2026-11-06"] {
        assert!(head.contains(needle), "Kopf nennt {needle}");
    }
    for needle in ["SWORD", "German", "2026-11-06", "kein Bibeltext"] {
        assert!(SOURCE.contains(needle), "SOURCE nennt {needle}: {SOURCE}");
    }
}

/// Runde 17a: Die übrigen Zählungen kommen aus sword-modules; German und KJV aus den eingebetteten Tabellen. Beide
/// Quellen müssen für German und KJV übereinstimmen – sonst bildete Baruch mit anderen Zahlen ab, als es prüft.
#[test]
fn sword_modules_has_the_same_numbers_as_the_tables_for_german_and_kjv() {
    use baruch_scripture::versification::versification;
    for name in ["German", "KJV"] {
        let ours = versification(name).unwrap();
        let theirs = sword_modules::Versification::get(name).unwrap();
        let mut differences = Vec::new();
        for (book, chapters) in ours.books() {
            let def = theirs.book_by_osis(book).unwrap_or_else(|| panic!("{name}: Buch {book} fehlt in sword-modules"));
            if usize::from(def.chapters) != chapters.len() {
                differences.push(format!("{book}: {} statt {} Kapitel", def.chapters, chapters.len()));
            }
            for (c, &n) in chapters.iter().enumerate() {
                let other = theirs.verse_count(def.id, c as u8 + 1).map(u16::from);
                if other != Some(n) {
                    differences.push(format!("{book} {}: {other:?} statt {n}", c + 1));
                }
            }
        }
        assert!(differences.is_empty(), "{name}: sword-modules weicht von der Tabelle ab: {differences:?}");
    }
}
