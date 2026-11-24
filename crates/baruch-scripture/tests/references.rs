// SPDX-License-Identifier: AGPL-3.0-or-later
//! Stellenangaben lesen und prüfen (Runde 10, Aufgabe „scripture“): Pflichtbeispiele, Bereiche, Listen,
//! übertragenes Buch, Fehlerfälle mit genanntem Maximum, Fließtext mit Byte-Bereich, Wortlaut ungeprüft.

use baruch_scripture::{Found, NoBible, RefStatus, VerseText, Wording, find_book, parse, scan};

fn one(reference: &str) -> Found {
    let mut found = parse(reference);
    assert_eq!(found.len(), 1, "genau eine Stelle in „{reference}“: {found:?}");
    found.pop().unwrap()
}

fn valid(reference: &str, normalized: &str, start: (u16, u16), end: (u16, u16)) {
    let f = one(reference);
    assert_eq!(f.status, RefStatus::Valid, "„{reference}“ gibt es in der Verszählung: {f:?}");
    assert_eq!(f.normalized.as_deref(), Some(normalized), "einheitliche Schreibweise von „{reference}“");
    let p = f.passage.expect("Stelle gelesen");
    assert_eq!((p.start, p.end), (start, end), "Anfang und Ende von „{reference}“");
    assert_eq!(f.input, reference.trim(), "input ist der Text der Angabe");
}

#[test]
fn the_required_examples_are_valid() {
    valid("1. Kön 3,16–28", "1. Kön 3,16–28", (3, 16), (3, 28));
    valid("Ps 139,23-24", "Ps 139,23–24", (139, 23), (139, 24));
    valid("Joh 3,16", "Joh 3,16", (3, 16), (3, 16));
    valid("2. Sam 12,1–12", "2. Sam 12,1–12", (12, 1), (12, 12));
    valid("Gal 6,4", "Gal 6,4", (6, 4), (6, 4));
    assert_eq!(one("1. Kön 3,16–28").passage.unwrap().book.id, "1Kgs", "OSIS-Kürzel des Buchs");
}

#[test]
fn numbered_books_accept_the_usual_spellings() {
    for spelling in ["1. Kön", "1 Kön", "1.Kön", "1. Könige", "1Kön", "1. kön"] {
        assert_eq!(find_book(spelling).map(|b| b.id), Some("1Kgs"), "„{spelling}“ ist das erste Buch der Könige");
    }
    assert_eq!(find_book("Psalm").map(|b| b.id), Some("Ps"), "Psalm (Einzahl)");
    assert_eq!(find_book("Röm").map(|b| b.name), Some("Römer"), "Röm");
    assert_eq!(find_book("2. Sam").map(|b| b.abbr), Some("2. Sam"), "2. Sam");
}

#[test]
fn john_and_first_john_are_different_books() {
    assert_eq!(find_book("Joh").map(|b| b.id), Some("John"), "Joh ist das Evangelium");
    assert_eq!(find_book("1. Joh").map(|b| b.id), Some("1John"), "1. Joh ist der erste Brief");
    let gospel = one("Joh 5,1");
    let letter = one("1. Joh 5,1");
    assert_eq!(gospel.passage.unwrap().book.id, "John", "Evangelium: {gospel:?}");
    assert_eq!(letter.passage.unwrap().book.id, "1John", "Brief: {letter:?}");
    // 1. Joh hat 5 Kapitel, das Evangelium 21: Kapitel 6 gibt es nur im Evangelium.
    assert_eq!(one("1. Joh 6,1").status, RefStatus::NoSuchChapter { max: 5 }, "der Brief hat 5 Kapitel");
    assert_eq!(one("Joh 6,1").status, RefStatus::Valid, "das Evangelium hat Kapitel 6");
}

#[test]
fn several_references_with_semicolons_carry_the_book() {
    let found = parse("Joh 3,16; 4,1; Röm 8,28; 12,1-2");
    let normalized: Vec<_> = found.iter().map(|f| f.normalized.clone().unwrap_or_default()).collect();
    assert_eq!(normalized, ["Joh 3,16", "Joh 4,1", "Röm 8,28", "Röm 12,1–2"], "Buch wird übertragen: {found:?}");
    assert!(found.iter().all(|f| f.status == RefStatus::Valid), "alle vorhanden: {found:?}");
    assert_eq!(found[1].input, "4,1", "input der zweiten Angabe ohne Buch");
    let text = "Joh 3,16; 4,1; Röm 8,28; 12,1-2";
    for f in &found {
        assert_eq!(&text[f.span.0..f.span.1], f.input, "span zeigt auf den Text der Angabe");
    }
}

#[test]
fn chapter_ranges_verse_lists_and_following_verses() {
    valid("Joh 3,16–4,2", "Joh 3,16–4,2", (3, 16), (4, 2));
    valid("Joh 3,16f", "Joh 3,16–17", (3, 16), (3, 17));
    // „ff“ reicht bis zum Kapitelende: Joh 3 hat 36 Verse (Verszählung der Quelle).
    valid("Joh 3,16ff", "Joh 3,16ff", (3, 16), (3, 36));
    let list = parse("Joh 3,16.18");
    let normalized: Vec<_> = list.iter().map(|f| f.normalized.clone().unwrap()).collect();
    assert_eq!(normalized, ["Joh 3,16", "Joh 3,18"], "Versliste: je Eintrag eine Stelle: {list:?}");
    assert_eq!(list[1].input, "18", "zweiter Eintrag zeigt auf seine Zahl");
}

#[test]
fn a_whole_chapter_has_verse_zero() {
    valid("Ps 23", "Ps 23", (23, 0), (23, 0));
    valid("Ps 23–24", "Ps 23–24", (23, 0), (24, 0));
    // Bücher mit einem Kapitel: die Zahl ist der Vers.
    valid("Jud 3", "Jud 1,3", (1, 3), (1, 3));
}

#[test]
fn unknown_books_and_numbers_beyond_the_maximum_are_reported() {
    let unknown = one("Henoch 1,9");
    assert_eq!(unknown.status, RefStatus::UnknownBook, "Henoch gehört nicht zu den 66 Büchern: {unknown:?}");
    assert!(unknown.passage.is_none() && unknown.normalized.is_none(), "keine Stelle ohne Buch");
    assert!(unknown.message().contains("Buch unbekannt"), "Meldung: {}", unknown.message());

    let chapter = one("Ps 151,1");
    assert_eq!(chapter.status, RefStatus::NoSuchChapter { max: 150 }, "der Psalter hat 150 Psalmen");
    assert!(chapter.message().contains("150 Kapitel"), "Maximum genannt: {}", chapter.message());

    let verse = one("Ps 139,25");
    assert_eq!(verse.status, RefStatus::NoSuchVerse { chapter: 139, max: 24 }, "Psalm 139 hat 24 Verse");
    assert!(verse.message().contains("24 Verse"), "Maximum genannt: {}", verse.message());

    let end = one("1. Kön 3,16–29");
    assert_eq!(end.status, RefStatus::NoSuchVerse { chapter: 3, max: 28 }, "auch das Ende wird geprüft");
}

#[test]
fn malformed_references_say_why_and_never_panic() {
    for text in ["Joh", "Joh 3,", "3,16", "Joh 3,0", "Joh 4,2–3,1", "Joh 3,16 und so", "", ";;", "Joh 3,16;"] {
        let found = parse(text);
        for f in &found {
            if let RefStatus::Malformed(why) = &f.status {
                assert!(!why.is_empty(), "Grund genannt für „{text}“");
            }
        }
        if ["Joh", "Joh 3,", "3,16", "Joh 3,0", "Joh 4,2–3,1", "Joh 3,16 und so"].contains(&text) {
            assert!(
                found.iter().any(|f| matches!(f.status, RefStatus::Malformed(_))),
                "„{text}“ ist keine lesbare Stelle: {found:?}"
            );
        }
    }
    assert!(parse("").is_empty() && parse(";;").is_empty(), "leere Stücke ergeben nichts");
    assert_eq!(parse("Joh 3,16;").len(), 1, "Semikolon am Ende stört nicht");
}

#[test]
fn scan_finds_references_in_running_text_with_byte_spans() {
    let text = "Die Liebe ist langmütig (1. Kor 13,4). Später lesen wir Ps 139,23-24 und Joh 3,16; 4,1. Am Ende.";
    let found = scan(text);
    let normalized: Vec<_> = found.iter().map(|f| f.normalized.clone().unwrap()).collect();
    assert_eq!(normalized, ["1. Kor 13,4", "Ps 139,23–24", "Joh 3,16", "Joh 4,1"], "Stellen im Fließtext: {found:?}");
    for f in &found {
        assert_eq!(&text[f.span.0..f.span.1], f.input, "Byte-Bereich trifft die Angabe: {f:?}");
    }
    assert_eq!(&text[found[0].span.0..found[0].span.1], "1. Kor 13,4", "ohne Klammern und ohne Punkt");
}

#[test]
fn scan_ignores_ordinary_words_and_ambiguous_abbreviations_without_verse() {
    let text = "Am 3. Tag kam er zum ersten Mal 3 Stunden zu spät; die Johannisbeere 3,5 Euro.";
    assert!(scan(text).is_empty(), "keine Stelle in gewöhnlichem Text: {:?}", scan(text));
    let with_verse = scan("So steht es in Am 3,7 geschrieben.");
    assert_eq!(with_verse.len(), 1, "mit Vers ist Am der Prophet Amos: {with_verse:?}");
    assert_eq!(with_verse[0].passage.unwrap().book.id, "Amos", "Amos");
    assert!(scan("wie joh 3,16 sagt").is_empty(), "im Fließtext nur mit großem Anfangsbuchstaben");
    assert_eq!(scan("Lies Ps 23.").len(), 1, "ganzes Kapitel mit eindeutigem Buch");
}

#[test]
fn no_bible_never_confirms_a_wording() {
    let passage = one("Joh 3,16").passage.unwrap();
    assert_eq!(NoBible.wording(&passage), Wording::Unchecked, "ohne lokale Bibel bleibt der Wortlaut ungeprüft");
    // Seit 2026-11-06 liefert ein SWORD-Modul den Text (Crate baruch-sword); NoBible heißt „keine Bibel
    // gewählt“ und ist kein STUB mehr.
}

#[test]
fn found_serializes_for_interfaces() {
    let value = serde_json::to_value(one("Ps 139,25")).unwrap();
    assert_eq!(value["status"]["kind"], "no_such_verse", "Status als Name: {value}");
    assert_eq!(value["status"]["detail"]["max"], 24, "Maximum im Status: {value}");
    assert_eq!(value["passage"]["book"]["name"], "Psalmen", "Buch mit Namen, ohne Verstabelle: {value}");
    assert!(value["passage"]["book"].get("verses").is_none(), "Verstabelle nicht in der Ausgabe: {value}");
}

#[test]
fn german_numbering_differs_from_the_english_and_is_used() {
    // Deutsche Bibeln (auch Schlachter 1951) zählen die Psalmüberschrift als Vers und gliedern Joel,
    // Maleachi, Nehemia 3/4, 1. Kön 4/5, Daniel 3/4 und einige NT-Stellen anders als die englische
    // (KJV-)Zählung. Jede Zeile ist eine Stelle, die es nur in der deutschen Zählung gibt.
    for reference in [
        "Ps 51,21",
        "Ps 3,9",
        "Ps 60,14",
        "Neh 3,38",
        "1. Kön 5,32",
        "Dan 3,33",
        "Joel 4,1",
        "Hos 2,25",
        "Jona 2,11",
        "2. Kor 13,13",
        "3. Joh 15",
        "Offb 12,18",
    ] {
        let f = one(reference);
        assert_eq!(f.status, RefStatus::Valid, "„{reference}“ gibt es in der deutschen Zählung: {f:?}");
    }
    // Umgekehrt: nur englisch vorhanden.
    assert_eq!(one("Mal 4,1").status, RefStatus::NoSuchChapter { max: 3 }, "Maleachi hat 3 Kapitel");
    assert_eq!(one("Mal 3,24").status, RefStatus::Valid, "Mal 3,24 ist der letzte Vers");
    assert_eq!(one("Dan 4,37").status, RefStatus::NoSuchVerse { chapter: 4, max: 34 }, "Dan 4 hat 34 Verse");
    assert_eq!(one("2. Kor 13,14").status, RefStatus::NoSuchVerse { chapter: 13, max: 13 }, "2. Kor 13 hat 13 Verse");
}

#[test]
fn a_numbered_book_after_a_comma_starts_a_new_reference() {
    // „Joh 3, 1. Joh 4“: Nach dem ganzen Kapitel Joh 3 beginnt mit „1. Joh“ ein neues Buch – „1“ ist kein Vers.
    let found = scan("vgl. Joh 3, 1. Joh 4 und Ps 23, 2. Sam 7,14.");
    let normalized: Vec<_> = found.iter().map(|f| f.normalized.clone().unwrap()).collect();
    assert_eq!(normalized, ["Joh 3", "1. Joh 4", "Ps 23", "2. Sam 7,14"], "je Buch eine Stelle: {found:?}");
    assert_eq!(found[1].passage.unwrap().book.id, "1John", "der Brief, nicht das Evangelium");
    // Ohne Leerraum nach dem Komma bleibt es ein Vers, ebenso eine Zahl ohne folgendes Buch.
    valid("Joh 3,1", "Joh 3,1", (3, 1), (3, 1));
    valid("Joh 3, 16", "Joh 3,16", (3, 16), (3, 16));
}

// ---- Runde 17b: Verslisten mit Komma, Zahlwörter, Uhrzeiten, „Vers …“ im selben Satz ---------------------

fn normalized(found: &[Found]) -> Vec<String> {
    found.iter().map(|f| f.normalized.clone().unwrap_or_default()).collect()
}

#[test]
fn a_verse_list_with_commas_gives_one_reference_per_entry() {
    let list = parse("Joh 3,16, 18");
    assert_eq!(
        normalized(&list),
        ["Joh 3,16", "Joh 3,18"],
        "Komma-Liste wie Punkt-Liste: je Vers eine Stelle: {list:?}"
    );
    assert_eq!(list[1].input, "18", "der zweite Eintrag zeigt nur auf seine Zahl (wie bei „3,16.18“)");
    assert!(list.iter().all(|f| f.status == RefStatus::Valid), "beide Verse gibt es: {list:?}");
    let range = parse("Joh 3,16, 18–20");
    assert_eq!(normalized(&range), ["Joh 3,16", "Joh 3,18–20"], "auch ein Bereich nach dem Komma: {range:?}");
    assert_eq!(range[1].input, "18–20", "input des Bereichs");
    let text = "Lies Joh 3,16, 18 und dann weiter.";
    let found = scan(text);
    assert_eq!(normalized(&found), ["Joh 3,16", "Joh 3,18"], "im Fließtext ebenso: {found:?}");
    for f in &found {
        assert_eq!(&text[f.span.0..f.span.1], f.input, "Byte-Bereich trifft die Angabe: {f:?}");
    }
    // Was nach dem Komma steht, ist kein Vers, wenn es ein neues Buch, eine Ordnungszahl oder ein Zahlwort ist.
    let mixed = scan("vgl. Joh 3, 1. Joh 4 und Ps 139,23-24, 2. Sam 7,14 sowie Joh 3,16, 18 Mal.");
    assert_eq!(
        normalized(&mixed),
        ["Joh 3", "1. Joh 4", "Ps 139,23–24", "2. Sam 7,14", "Joh 3,16"],
        "„Joh 3, 1. Joh 4“ bleibt zwei Stellen, „, 2. Sam“ beginnt ein Buch, „, 18 Mal“ ist eine Anzahl: {mixed:?}"
    );
}

#[test]
fn numbers_before_count_words_are_not_references_in_running_text() {
    for text in ["Er hat es Johannes 3 Mal gesagt.", "Psalm 23 Jahre lang", "Johannes 3x gefragt", "Markus 4 Stück"] {
        assert!(
            scan(text).is_empty(),
            "„{text}“: eine Zahl vor einem Zahlwort ist eine Anzahl, keine Stelle: {:?}",
            scan(text)
        );
    }
    assert_eq!(
        normalized(&scan("Lies Johannes 3 mit Bedacht.")),
        ["Joh 3"],
        "ohne Zahlwort bleibt das Kapitel eine Stelle"
    );
    assert_eq!(
        normalized(&scan("Johannes 3 malt ein Bild")),
        ["Joh 3"],
        "„malt“ ist kein Zahlwort (ganzes Wort zählt)"
    );
}

#[test]
fn clock_times_after_weekdays_are_not_references_but_strict_parse_stays_micah() {
    for text in ["Treffen Mi 19:30 im Saal.", "Probe Fr 9:15 Uhr", "Mi 7:18 geht der Zug"] {
        assert!(
            scan(text).is_empty(),
            "„{text}“ ist eine Uhrzeit nach einem Wochentag, keine Stelle: {:?}",
            scan(text)
        );
    }
    let strict = one("Mi 7:18");
    assert_eq!(
        strict.passage.unwrap().book.id,
        "Mic",
        "das strenge parse liest „Mi 7:18“ als Micha (Eingabe als Stelle)"
    );
    assert_eq!(strict.status, RefStatus::Valid, "Micha 7,18 gibt es: {strict:?}");
    assert_eq!(normalized(&scan("So steht es in Mi 7,18 geschrieben.")), ["Mi 7,18"], "mit Komma bleibt es Micha");
    assert_eq!(
        normalized(&scan("Treffen Mi 19:75 im Saal.")),
        ["Mi 19,75"],
        "keine gültige Uhrzeit (Minute 75): wie bisher eine (ungültige) Stelle"
    );
}

#[test]
fn a_verse_named_later_in_the_same_sentence_completes_the_chapter() {
    for (text, label, input) in [
        ("Im Psalm 139, besonders Vers 23, betet David.", "Ps 139,23", "Psalm 139, besonders Vers 23"),
        ("Psalm 139 – dort die Verse 23–24 – ist ein Gebet.", "Ps 139,23–24", "Psalm 139 – dort die Verse 23–24"),
        ("Psalm 139, vgl. V. 23.", "Ps 139,23", "Psalm 139, vgl. V. 23"),
    ] {
        let found = scan(text);
        assert_eq!(
            normalized(&found),
            [label],
            "„{text}“: Kapitel und Vers im selben Satz sind eine Stelle: {found:?}"
        );
        assert_eq!(found[0].input, input, "Bereich vom Buch bis zur Versnummer");
        assert_eq!(&text[found[0].span.0..found[0].span.1], input, "span trifft den Bereich");
        assert_eq!(found[0].status, RefStatus::Valid, "Ps 139 hat 24 Verse: {found:?}");
    }
    let other_sentence = scan("Lies Psalm 139. Vers 23 steht später.");
    assert_eq!(normalized(&other_sentence), ["Ps 139"], "ein Satzende trennt Kapitel und Vers: {other_sentence:?}");
    let other_ref = scan("Psalm 23 und Joh 3 Vers 16.");
    assert_eq!(normalized(&other_ref), ["Ps 23", "Joh 3,16"], "der Vers gehört zur nächsten Stelle: {other_ref:?}");
    let far = format!("Psalm 139, {} Vers 23.", "sehr ".repeat(30));
    assert_eq!(normalized(&scan(&far)), ["Ps 139"], "mehr als 120 Zeichen dazwischen: kein Bezug");
    let beyond = scan("Psalm 139, besonders Vers 25.");
    assert_eq!(beyond[0].status, RefStatus::NoSuchVerse { chapter: 139, max: 24 }, "der Vers wird geprüft: {beyond:?}");
}

#[test]
fn the_fourth_and_fifth_book_of_moses_are_found_in_running_text() {
    // Integrator Runde 17: Der Fließtext-Scanner begann eine Stelle nur bei einer Ziffer 1–3; „4. Mose“ und
    // „5. Mose“ blieben so unentdeckt (aufgefallen am Ende-zu-Ende-Test der Abbildung mit 4. Mo 25,19).
    let found = scan("Vgl. 4. Mo 25,19 und 5. Mose 6,4 sowie 4Mo 1,1.");
    assert_eq!(
        normalized(&found),
        ["4. Mo 25,19", "5. Mo 6,4", "4. Mo 1,1"],
        "auch das vierte und fünfte Buch Mose sind Stellen im Fließtext: {found:?}"
    );
    assert!(scan("Es kamen 4 Männer und 5 Frauen.").is_empty(), "eine bloße Zahl ist weiterhin keine Stelle");
}
