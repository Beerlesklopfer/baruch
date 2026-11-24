// SPDX-License-Identifier: AGPL-3.0-or-later
//! Abbildung zwischen Verszählungen (Runde 17a): Regeltabelle `data/versification-map.tsv` gegen die Verszahlen
//! beider Zählungen, Psalmüberschriften, Joel 2/3, Mal 3/4, Grenzen und ehrliches „Versifikation passt nicht“.

use std::collections::HashSet;

use baruch_scripture::versification::mapping::{MISMATCH, Mapped, TABLE, VerseRef, map_range, map_verse, rules};
use baruch_scripture::versification::{GERMAN, KJV, Versification, german, names, versification};

fn v11n(name: &str) -> &'static Versification {
    versification(name).unwrap_or_else(|| panic!("Zählung {name} bekannt"))
}

fn verse(book: &'static str, chapter: u16, verse: u16) -> VerseRef {
    VerseRef { book, chapter, verse }
}

fn pairs(range: &[VerseRef]) -> Vec<(u16, u16)> {
    range.iter().map(|v| (v.chapter, v.verse)).collect()
}

#[test]
fn a_psalm_title_counted_as_verse_moves_the_verses() {
    let (g, k) = (german(), v11n(KJV));
    match map_verse(k, g, verse("Ps", 51, 1)) {
        Mapped::Moved { to, rule } => {
            assert_eq!((to.chapter, to.verse), (51, 3), "KJV Ps 51,1 ist deutsch 51,3 (Überschrift = Verse 1–2)");
            assert!(rule.starts_with("Psalmüberschrift"), "Regel nennt die Überschrift: {rule}");
        }
        other => panic!("verschoben erwartet: {other:?}"),
    }
    assert_eq!(map_verse(g, k, verse("Ps", 51, 3)).target(), Some(verse("Ps", 51, 1)), "und zurück");
    match map_verse(g, k, verse("Ps", 51, 1)) {
        Mapped::Unmapped { reason } => {
            assert!(reason.starts_with(MISMATCH), "Grund beginnt mit „Versifikation passt nicht“: {reason}");
            assert!(reason.contains("German") && reason.contains("KJV"), "beide Zählungen genannt: {reason}");
        }
        other => panic!("die Überschrift ist in KJV kein Vers: {other:?}"),
    }
    assert_eq!(map_verse(k, g, verse("Ps", 3, 1)).target(), Some(verse("Ps", 3, 2)), "Ps 3: eine Überschrift");
    assert_eq!(
        map_verse(k, g, verse("Ps", 13, 1)).target(),
        Some(verse("Ps", 13, 2)),
        "Ps 13 hat gleich viele Verse und ist trotzdem verschoben (Regel, nicht Verszahl)"
    );
}

#[test]
fn joel_and_malachi_follow_their_chapter_boundaries_in_both_directions() {
    let (g, k) = (german(), v11n(KJV));
    let joel = map_range(k, g, "Joel", (2, 28), (2, 32), 100).unwrap();
    assert_eq!(pairs(&joel.verses), (1..=5).map(|v| (3, v)).collect::<Vec<_>>(), "Joel 2,28–32 (KJV) = Joel 3,1–5");
    assert_eq!(joel.rules.len(), 1, "eine Regel: {:?}", joel.rules);
    let back = map_range(g, k, "Joel", (3, 1), (3, 5), 100).unwrap();
    assert_eq!(pairs(&back.verses), (28..=32).map(|v| (2, v)).collect::<Vec<_>>(), "und zurück");
    let joel4 = map_range(g, k, "Joel", (4, 0), (4, 0), 100).unwrap();
    assert_eq!(pairs(&joel4.verses), (1..=21).map(|v| (3, v)).collect::<Vec<_>>(), "Joel 4 (deutsch) = Joel 3 (KJV)");
    let mal = map_range(k, g, "Mal", (4, 1), (4, 6), 100).unwrap();
    assert_eq!(pairs(&mal.verses), (19..=24).map(|v| (3, v)).collect::<Vec<_>>(), "Mal 4,1–6 (KJV) = Mal 3,19–24");
    let back = map_range(g, k, "Mal", (3, 19), (3, 24), 100).unwrap();
    assert_eq!(pairs(&back.verses), (1..=6).map(|v| (4, v)).collect::<Vec<_>>(), "und zurück");
    assert_eq!(
        map_verse(k, g, verse("Mal", 3, 18)),
        Mapped::Same { to: verse("Mal", 3, 18) },
        "Mal 3,1–18 bleibt gleich (Regel mit gleichem Ziel)"
    );
}

#[test]
fn an_uncovered_difference_is_never_guessed() {
    let (g, k) = (german(), v11n(KJV));
    // 2. Kön 15 hat in German 39, in KJV 38 Verse, und keine Regel deckt das ab.
    match map_verse(g, k, verse("2Kgs", 15, 1)) {
        Mapped::Unmapped { reason } => {
            assert!(reason.starts_with(MISMATCH), "ehrlicher Grund: {reason}");
            assert!(reason.contains("39") && reason.contains("38"), "Verszahlen genannt: {reason}");
        }
        other => panic!("nicht abbildbar erwartet: {other:?}"),
    }
    let err = map_range(g, k, "2Kgs", (15, 1), (15, 3), 10).unwrap_err();
    assert!(err.starts_with(MISMATCH) && err.contains("German") && err.contains("KJV"), "Stelle scheitert: {err}");
    let err = map_range(k, g, "Joel", (4, 1), (4, 1), 10).unwrap_err();
    assert!(err.starts_with(MISMATCH) && err.contains("KJV"), "Joel 4 gibt es in KJV nicht: {err}");
    let err = map_range(g, v11n("MT"), "John", (3, 16), (3, 16), 10).unwrap_err();
    assert!(err.contains("MT"), "MT hat kein Neues Testament: {err}");
    match map_verse(g, v11n("Vulg"), verse("Ps", 23, 1)) {
        Mapped::Unmapped { reason } => assert!(reason.contains("Psalmen"), "griechische Psalmzählung: {reason}"),
        other => panic!("Psalmen der Vulgata sind nicht abgebildet: {other:?}"),
    }
}

#[test]
fn the_limit_counts_verses_of_the_source() {
    let (g, k) = (german(), v11n(KJV));
    let exact = map_range(g, k, "Ps", (23, 0), (23, 0), 6).unwrap();
    assert_eq!((exact.verses.len(), exact.truncated), (6, false), "genau limit Verse ⇒ nicht gekürzt");
    let over = map_range(g, k, "Ps", (23, 0), (23, 0), 5).unwrap();
    assert_eq!((over.verses.len(), over.truncated), (5, true), "limit + 1 Verse ⇒ gekürzt");
}

#[test]
fn a_whole_psalm_leaves_out_title_verses_but_a_title_verse_alone_fails() {
    let (g, k) = (german(), v11n(KJV));
    let whole = map_range(g, k, "Ps", (51, 0), (51, 0), 100).unwrap();
    assert_eq!(pairs(&whole.verses), (1..=19).map(|v| (51, v)).collect::<Vec<_>>(), "Ps 51 ganz = KJV 51,1–19");
    assert!(whole.rules.iter().any(|r| r.contains("ausgelassen")), "Auslassung steht in den Regeln: {:?}", whole.rules);
    assert!(map_range(g, k, "Ps", (51, 1), (51, 3), 100).is_err(), "Verse 1–3 enthalten die Überschrift");
}

#[test]
fn identical_versifications_map_every_verse_to_itself() {
    let (g, l) = (german(), v11n("Luther"));
    for (book, chapters) in g.books() {
        for (c, &n) in chapters.iter().enumerate() {
            let c = c as u16 + 1;
            for v in [1, n] {
                assert_eq!(
                    map_verse(g, l, verse(book, c, v)),
                    Mapped::Same { to: verse(book, c, v) },
                    "German = Luther"
                );
            }
        }
    }
}

#[test]
fn every_verse_mapped_to_kjv_maps_back_to_itself() {
    let (g, k) = (german(), v11n(KJV));
    let mut mapped = 0;
    for (from, to) in [(g, k), (k, g)] {
        for (book, chapters) in from.books() {
            for (c, &n) in chapters.iter().enumerate() {
                for v in 1..=n {
                    let source = verse(book, c as u16 + 1, v);
                    let Some(target) = map_verse(from, to, source).target() else { continue };
                    assert_eq!(
                        map_verse(to, from, target).target(),
                        Some(source),
                        "{} {} → {} → zurück muss derselbe Vers sein",
                        from.name,
                        source.label(),
                        target.label()
                    );
                    mapped += 1;
                }
            }
        }
    }
    assert!(mapped > 60_000, "fast alle Verse beider Richtungen abbildbar: {mapped}");
}

#[test]
fn every_rule_is_checked_against_both_versifications() {
    let k = v11n(KJV);
    let systems: HashSet<&str> = rules().iter().flat_map(|r| r.systems.iter().copied()).collect();
    for system in systems {
        let x = versification(system).unwrap_or_else(|| panic!("Regel nennt unbekannte Zählung {system}"));
        let mut sources = HashSet::new();
        let mut targets = HashSet::new();
        let mut touched = HashSet::new();
        for rule in rules().iter().filter(|r| r.systems.contains(&system)) {
            let Some(chapter) = rule.chapter else { continue };
            for (i, v) in (rule.first..=rule.last).enumerate() {
                let n = x.verses_in(rule.book, chapter);
                assert!(n.is_some_and(|n| v >= 1 && v <= n), "{system}: Quelle {} {chapter},{v} existiert", rule.book);
                assert!(sources.insert((rule.book, chapter, v)), "{system}: {} {chapter},{v} nur einmal", rule.book);
                touched.insert((rule.book, chapter));
                if let Some((tc, tv)) = rule.target {
                    let t = tv + i as u16;
                    let m = k.verses_in(rule.book, tc);
                    assert!(
                        m.is_some_and(|m| t <= m),
                        "{system}: Ziel {} {tc},{t} existiert in KJV ({})",
                        rule.book,
                        rule.name
                    );
                    assert!(
                        targets.insert((rule.book, tc, t)),
                        "{system}: Ziel {} {tc},{t} injektiv ({})",
                        rule.book,
                        rule.name
                    );
                }
            }
        }
        for (book, chapter) in touched {
            for v in 1..=x.verses_in(book, chapter).unwrap() {
                assert!(
                    sources.contains(&(book, chapter, v)),
                    "{system}: berührtes Kapitel {book} {chapter} ganz abgedeckt, {v} fehlt"
                );
            }
        }
    }
}

#[test]
fn the_table_holds_only_numbers_and_rule_names_and_names_its_origin() {
    let head: String = TABLE.lines().take_while(|l| l.starts_with('#')).collect::<Vec<_>>().join("\n");
    for needle in ["sword-modules", "KJV", "Psalmüberschrift", "Joel", "Mal 3/4", "Röm 16", "Offb 12", "kein"] {
        assert!(head.contains(needle), "Kopf nennt {needle}");
    }
    for (n, line) in TABLE.lines().enumerate().filter(|(_, l)| !l.starts_with('#')) {
        let cols: Vec<&str> = line.split('\t').collect();
        assert_eq!(cols.len(), 8, "Zeile {}: acht Spalten", n + 1);
        for col in &cols[2..7] {
            assert!(
                col.chars().all(|c| c.is_ascii_digit()) || *col == "-" || *col == "*",
                "Zeile {}: nur Zahlen",
                n + 1
            );
        }
        assert!(cols[7].chars().count() <= 80, "Zeile {}: kurzer Regelname, kein Bibeltext", n + 1);
    }
}

#[test]
fn every_name_of_sword_modules_is_known_without_case() {
    let all = names();
    assert_eq!(all.len(), 18, "18 Zählungen: {all:?}");
    assert_eq!(all[0], GERMAN, "German zuerst");
    for name in [
        "calvin",
        "CATHOLIC",
        "Catholic2",
        "darbyfr",
        "kjv",
        "kjva",
        "lxx",
        "leningrad",
        "luther",
        "mt",
        "nrsv",
        "nrsva",
        "orthodox",
        "segond",
        "synodal",
        "synodalprot",
        "vulg",
    ] {
        assert!(versification(name).is_some(), "{name} bekannt (Groß/Klein egal)");
    }
    assert!(versification("Phantasie").is_none(), "Erfundenes bleibt unbekannt");
}

/// Nachbesserung Runde 17: Catholic, Catholic2, Segond und Calvin zählen die Psalmüberschrift wie German als Vers 1
/// (Ps 3 = 9, Ps 51 = 21 Verse), haben aber keine einzige Regel in der Tabelle. Ein Psalm mit zufällig gleich
/// vielen Versen wie in KJV (Ps 13: 6 = 6) darf deshalb nicht als „gleich“ gelten.
#[test]
fn a_versification_without_rules_never_guesses_a_differing_book() {
    let (g, k) = (german(), v11n(KJV));
    for name in ["Catholic", "Catholic2", "Segond", "Calvin"] {
        let x = v11n(name);
        assert!(rules().iter().all(|r| !r.systems.contains(&name)), "{name} hat keine Regel (Voraussetzung)");
        assert_eq!(
            (x.verses_in("Ps", 3), x.verses_in("Ps", 13), k.verses_in("Ps", 13)),
            (Some(9), Some(6), Some(6)),
            "{name}: Überschrift als Vers 1 (Ps 3 = 9), Ps 13 gleich lang wie in KJV (Voraussetzung)"
        );
        for (from, to) in [(x, k), (k, x), (g, x), (x, g)] {
            match map_verse(from, to, verse("Ps", 13, 2)) {
                Mapped::Unmapped { reason } => {
                    assert!(reason.starts_with(MISMATCH), "ehrlicher Grund: {reason}");
                    assert!(
                        reason.contains(name) && reason.contains("keine geprüften Regeln"),
                        "Grund nennt {name}: {reason}"
                    );
                }
                other => panic!("{} → {}: Ps 13,2 darf nicht geraten werden: {other:?}", from.name, to.name),
            }
        }
        assert_eq!(
            map_verse(x, k, verse("John", 3, 16)),
            Mapped::Same { to: verse("John", 3, 16) },
            "{name}: Ein Buch mit denselben Verszahlen wie KJV bleibt abbildbar"
        );
    }
    assert_eq!(
        map_verse(v11n("Catholic"), v11n("Catholic2"), verse("Ps", 13, 2)),
        Mapped::Same { to: verse("Ps", 13, 2) },
        "gleiche Verszahlen im ganzen Buch und dieselben (keine) Regeln: Vers bleibt, wo er ist"
    );
    assert_eq!(
        map_verse(k, g, verse("Ps", 13, 1)).target(),
        Some(verse("Ps", 13, 2)),
        "Zählungen mit Regeln bleiben, wie sie waren"
    );
}

/// Kein Kapitel einer Zählung ohne Regel für das Buch wird über die Kapitellänge geraten: Für jede Zählung, jedes
/// Buch ohne Regel und jedes Kapitel gilt „gleich“ nur, wenn das ganze Buch mit KJV übereinstimmt.
#[test]
fn every_book_without_rules_maps_only_when_it_equals_kjv() {
    let k = v11n(KJV);
    for x in baruch_scripture::versification::all() {
        if x.name == KJV {
            continue;
        }
        for (book, chapters) in x.books() {
            if rules().iter().any(|r| r.book == book && r.systems.contains(&x.name)) {
                continue;
            }
            let equal = k.chapters_of(book) == Some(chapters);
            for (c, &n) in chapters.iter().enumerate() {
                if n == 0 {
                    continue;
                }
                let mapped = map_verse(x, k, verse(book, c as u16 + 1, 1));
                assert_eq!(
                    mapped.target().is_some(),
                    equal,
                    "{} {book} {}: abbildbar nur bei gleichem Buch wie KJV ({mapped:?})",
                    x.name,
                    c + 1
                );
            }
        }
    }
}
