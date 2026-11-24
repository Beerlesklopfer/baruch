// SPDX-License-Identifier: AGPL-3.0-or-later
//! Runde 14 (Vertrag Abschnitt 1): Teil und „ohne Nummer“ als kanonische Markierung im Typst-Text.
//! `tests/data/teile.typ` ist frei erfunden (Werkvorlage, zwei Teile, Kapitel ohne Nummer,
//! Abschnitte, Kommentar hinter einer Markierung, Maskierungen mit führendem `-`, `=` und `Ziffern.`).

use baruch_typst::{
    PART_LABEL, UNNUMBERED_LABEL, heading_line, join, parse, part_line, part_page_rule, relevel_line,
    set_numbered_line, unescape_title, unnumbered_line,
};

const PARTS: &str = include_str!("data/teile.typ");

/// (Ebene, Titel, numbered) je Überschrift.
fn outline(source: &str) -> Vec<(u8, String, bool)> {
    parse(source).unwrap().headings.into_iter().map(|h| (h.level, h.title, h.numbered)).collect()
}

fn owned(rows: &[(u8, &str, bool)]) -> Vec<(u8, String, bool)> {
    rows.iter().map(|&(l, t, n)| (l, t.to_owned(), n)).collect()
}

/// Titel der einzigen Überschrift von `line` nach dem Zerlegen (mit einer Schlusszeile davor,
/// damit die Zeile nicht zur Werkvorlage wird, falls sie keine Überschrift ist).
fn single(line: &str) -> Option<(u8, String, bool)> {
    let doc = parse(&format!("= Anfang\n{line}")).ok()?;
    let found: Vec<_> = doc.headings.into_iter().skip(1).map(|h| (h.level, h.title, h.numbered)).collect();
    match found.as_slice() {
        [one] => Some(one.clone()),
        _ => None,
    }
}

#[test]
fn the_parts_book_joins_back_byte_for_byte() {
    let doc = parse(PARTS).unwrap();
    let joined = join(&doc.template, doc.headings.iter().map(|h| h.body.as_str()));
    assert_eq!(joined, PARTS, "join(parse(x)) muss x Byte für Byte ergeben, auch mit Markierungen");
}

#[test]
fn the_parts_book_has_parts_unnumbered_chapters_and_sections() {
    let expected = owned(&[
        (1, "Vorwort", false),
        (0, "Erster Teil: Wasser suchen", true),
        (1, "Die Wünschelrute", true),
        (2, "Am Bach", true),
        (2, "Zwischenruf [ohne Zahl]", false),
        (3, "Tiefer graben", true),
        (0, "- Zweiter Teil # mit Zeichen", true),
        (1, "Der Stein im Weg", true),
        (1, "= Gleich", false),
        (1, "2026. ein Jahr", false),
        (1, "Nachwort", true),
    ]);
    assert_eq!(
        outline(PARTS),
        expected,
        "Teil = Ebene 0 (numbered), „ohne Nummer“ = Ebene N mit numbered=false; Markierungen in Raw, \
         Kommentar und Inhaltsblock sowie das freie #heading mit anderem Label zählen nicht; \
         Titel sind aufgelöst"
    );
}

#[test]
fn a_marker_before_the_first_equals_heading_ends_the_template() {
    let doc = parse(PARTS).unwrap();
    assert!(
        doc.template.ends_with("<anderes-label>\n\n"),
        "Die Werkvorlage endet direkt vor der ersten Markierung „Vorwort“, das freie #heading gehört \
         noch dazu: {:?}",
        doc.template
    );
    assert_eq!(doc.title.as_deref(), Some("Vom Brunnenbauer"), "Titel aus #set document bleibt lesbar");
}

#[test]
fn raw_titles_are_kept_literally_and_bodies_run_to_the_next_marker() {
    let doc = parse(PARTS).unwrap();
    let second_part = &doc.headings[6];
    assert_eq!(second_part.raw_title, r"\- Zweiter Teil \# mit Zeichen", "raw_title ist T wörtlich");
    assert!(
        second_part.body.starts_with("#heading(level: 1, numbering: none)[\\- Zweiter Teil")
            && second_part.body.ends_with("// Kommentar hinter dem Teil\n\n"),
        "Der Text des Teils reicht von seiner Zeile bis vor das nächste Kapitel: {:?}",
        second_part.body
    );
    let stone = &doc.headings[7];
    assert!(
        stone.body.contains("[Im Raw]") && stone.body.contains("[Im Inhaltsblock]"),
        "Markierungen in Raw und Inhaltsblock bleiben Text des Kapitels: {:?}",
        stone.body
    );
}

#[test]
fn only_the_canonical_form_is_a_marker() {
    // (Zeile, Grund) – keine davon darf eine Überschrift ergeben.
    let deviations = [
        ("#heading(level: 1, numbering: none)[T]  <baruch-part>\n", "zwei Leerzeichen vor dem Label"),
        ("#heading(level: 1, numbering: none)[T]<baruch-part>\n", "kein Leerzeichen vor dem Label"),
        ("#heading(level:1, numbering: none)[T] <baruch-part>\n", "fehlendes Leerzeichen nach level:"),
        ("#heading(level: 1,  numbering: none)[T] <baruch-part>\n", "doppeltes Leerzeichen"),
        ("#heading(numbering: none, level: 1)[T] <baruch-part>\n", "andere Reihenfolge"),
        ("#heading(level: 1, numbering: none)[T] <baruch-teil>\n", "anderes Label"),
        ("#heading(level: 1, numbering: none)[T] <baruch-part>x\n", "Text hinter dem Label"),
        ("#heading(level: 1, numbering: none)[T] <baruch-part> \n", "Leerzeichen am Ende ohne Kommentar"),
        ("#heading(level: 1, numbering: none)[T] <baruch-part> /* x */\n", "Blockkommentar statt //"),
        ("#heading(level: 2, numbering: none)[T] <baruch-part>\n", "Teil mit Ziffer 2"),
        ("#heading(level: 4, numbering: none)[T] <baruch-unnumbered>\n", "Ebene 4 gibt es nicht"),
        ("#heading(level: 0, numbering: none)[T] <baruch-unnumbered>\n", "Ebene 0 ist kein „ohne Nummer“"),
        (" #heading(level: 1, numbering: none)[T] <baruch-part>\n", "eingerückt"),
        ("#heading(level: 1, numbering: \"1\")[T] <baruch-part>\n", "mit Nummer"),
        ("#heading(level: 1, numbering: none)[a [b] c] <baruch-part>\n", "unmaskierte Klammern im Titel"),
        ("#heading(level: 1, numbering: none)[a // b] <baruch-part>\n", "Kommentar im Titel"),
        ("#heading(level: 1, numbering: none)[a `] <baruch-part>\n", "offenes Inline-Raw im Titel"),
        ("#heading(level: 1, numbering: none)[T\\] <baruch-part>\n", "maskiertes ] schließt nicht"),
    ];
    for (line, why) in deviations {
        let doc = parse(&format!("= Anfang\n{line}")).unwrap();
        assert_eq!(doc.headings.len(), 1, "{why}: {line:?} darf keine Markierung sein");
    }
    for (line, why) in deviations {
        // Vor der ersten `=` bleibt die Zeile Werkvorlage; manche Abweichungen lassen den Rest sogar
        // offen (Klammer, Raw) – dann gibt es gar keine Überschrift, aber nie eine Markierung.
        let only_plain = parse(&format!("{line}= Anfang\n"))
            .map_or(true, |doc| doc.template == line && doc.headings.iter().all(|h| h.level == 1 && h.numbered));
        assert!(only_plain, "{why}: {line:?} vor der ersten = bleibt Werkvorlage");
    }
}

#[test]
fn markers_inside_raw_comment_content_or_string_stay_text() {
    let cases = [
        ("```\n#heading(level: 1, numbering: none)[T] <baruch-part>\n```\n", "Raw-Block"),
        ("/*\n#heading(level: 1, numbering: none)[T] <baruch-part>\n*/\n", "Blockkommentar"),
        ("#block[\n#heading(level: 1, numbering: none)[T] <baruch-part>\n]\n", "Inhaltsblock"),
        ("#let s = \"\n#heading(level: 1, numbering: none)[T] <baruch-part>\n\"\n", "mehrzeiliger String"),
        ("`\n#heading(level: 1, numbering: none)[T] <baruch-part>\n`\n", "mehrzeiliges Inline-Raw"),
    ];
    for (text, why) in cases {
        let doc = parse(&format!("= Anfang\n{text}= Ende\n")).unwrap();
        let levels: Vec<u8> = doc.headings.iter().map(|h| h.level).collect();
        assert_eq!(levels, vec![1, 1], "{why}: die Markierung darin ist Text");
    }
}

#[test]
fn a_comment_after_a_marker_is_allowed_and_not_part_of_the_title() {
    for line in [
        "#heading(level: 1, numbering: none)[Mitte] <baruch-part> // Notiz\n",
        "#heading(level: 1, numbering: none)[Mitte] <baruch-part>\t// Notiz\n",
        "#heading(level: 2, numbering: none)[Mitte] <baruch-unnumbered>   // Notiz\n",
    ] {
        let (_, title, _) = single(line).expect("Markierung mit Kommentar");
        assert_eq!(title, "Mitte", "Der Kommentar gehört nicht zum Titel: {line:?}");
    }
    assert_eq!(
        single("#heading(level: 1, numbering: none)[A] <baruch-part>\r\n").map(|h| h.0),
        Some(0),
        "CRLF ist ein Zeilenende"
    );
    assert_eq!(
        single("#heading(level: 1, numbering: none)[A] <baruch-part>").map(|h| h.0),
        Some(0),
        "letzte Zeile ohne Zeilenende"
    );
}

/// Titel, die beim Bauen maskiert werden müssen.
const HARD_TITLES: &[&str] = &[
    "Einfach",
    "Klammern ] und [ gemischt",
    "#hash und \\ Backslash",
    "- führender Strich",
    "= führendes Gleich",
    "+ führendes Plus",
    "/ führender Schrägstrich",
    "// zwei Schrägstriche",
    "12. Jahrhundert",
    "3.14 keine Liste",
    "Anna's \"Laube\" mit *Stern* und _Strich_ und $ und `Raw`",
    "https://beispiel.invalid/pfad",
    "",
];

#[test]
fn built_markers_parse_back_to_the_same_title() {
    for &title in HARD_TITLES {
        let part = part_line(title);
        assert!(part.ends_with(&format!("] <{PART_LABEL}>\n")), "Teil-Markierung mit Label und Zeilenende: {part:?}");
        assert_eq!(
            single(&part),
            Some((0, title.to_owned(), true)),
            "part_line({title:?}) = {part:?} muss als Teil mit demselben Titel zurückkommen"
        );
        for level in 1..=3 {
            let line = unnumbered_line(level, title);
            assert!(line.contains(&format!("<{UNNUMBERED_LABEL}>")), "Label „ohne Nummer“: {line:?}");
            assert_eq!(
                single(&line),
                Some((level, title.to_owned(), false)),
                "unnumbered_line({level}, {title:?}) = {line:?}"
            );
        }
    }
    assert_eq!(single(&unnumbered_line(0, "Null")), Some((1, "Null".into(), false)), "Ebene 0 wird auf 1 geklemmt");
    assert_eq!(single(&unnumbered_line(7, "Sieben")), Some((3, "Sieben".into(), false)), "Ebene 7 wird auf 3 geklemmt");
}

#[test]
fn leading_structure_characters_get_a_backslash() {
    let table = [
        ("- Strich", r"\- Strich"),
        ("= Gleich", r"\= Gleich"),
        ("+ Plus", r"\+ Plus"),
        ("/ Schräg", r"\/ Schräg"),
        ("12. Jahrhundert", r"12\. Jahrhundert"),
        ("Mitte - nicht", "Mitte - nicht"),
        ("3,5 kein Punkt", "3,5 kein Punkt"),
    ];
    for (title, t) in table {
        assert_eq!(
            part_line(title),
            format!("#heading(level: 1, numbering: none)[{t}] <baruch-part>\n"),
            "Nur ein führendes Strukturzeichen braucht im Inhaltsblock einen Backslash"
        );
        assert_eq!(unescape_title(t), title, "unescape_title(T) ergibt den Titel");
    }
}

#[test]
fn heading_line_builds_parts_and_keeps_unnumbered_and_comments() {
    assert_eq!(heading_line(0, "Teil A", None), part_line("Teil A"), "Ebene 0 ohne Vorgänger: Teil-Markierung");
    assert_eq!(
        heading_line(0, "Teil B", Some("#heading(level: 1, numbering: none)[Teil A] <baruch-part> // Notiz\n")),
        "#heading(level: 1, numbering: none)[Teil B] <baruch-part> // Notiz\n",
        "Umbenennen eines Teils: Kommentar bleibt"
    );
    assert_eq!(
        heading_line(0, "Teil", Some("= Alt <eigenes> // Notiz\n")),
        "#heading(level: 1, numbering: none)[Teil] <baruch-part> // Notiz\n",
        "Kapitel → Teil: Kommentar bleibt, das eigene Label nicht (der Teil hat sein Label)"
    );
    assert_eq!(
        heading_line(2, "Neu", Some("#heading(level: 1, numbering: none)[Alt] <baruch-unnumbered>\t// n\n")),
        "#heading(level: 2, numbering: none)[Neu] <baruch-unnumbered> // n\n",
        "„ohne Nummer“ bleibt beim Umbenennen eine Markierung, Kommentar bleibt"
    );
    assert_eq!(heading_line(2, "Neu", Some("== Alt // n\n")), "== Neu // n\n", "=-Zeile wie bisher");
    assert_eq!(heading_line(1, "Neu", None), "= Neu\n", "ohne Vorgänger wie bisher");
    assert_eq!(
        heading_line(1, "Neu", Some("#heading(level: 1, numbering: none)[Alt] <baruch-part>\n")),
        "= Neu\n",
        "Teil → Kapitel: wie bisher eine =-Zeile"
    );
}

#[test]
fn relevel_line_changes_only_the_level() {
    let table: &[(&str, u8, Option<&str>)] = &[
        ("= Titel // n\n", 2, Some("== Titel // n\n")),
        ("=== T <l>\n", 1, Some("= T <l>\n")),
        ("== T", 3, Some("=== T")),
        (
            "#heading(level: 1, numbering: none)[T] <baruch-unnumbered> // n\n",
            3,
            Some("#heading(level: 3, numbering: none)[T] <baruch-unnumbered> // n\n"),
        ),
        (
            "#heading(level: 2, numbering: none)[T] <baruch-unnumbered>\r\n",
            1,
            Some("#heading(level: 1, numbering: none)[T] <baruch-unnumbered>\r\n"),
        ),
        ("#heading(level: 1, numbering: none)[T] <baruch-part>\n", 1, None),
        ("= T\n", 0, None),
        ("= T\n", 4, None),
        ("Fließtext\n", 1, None),
        ("==== T\n", 1, None),
        ("#heading(level: 1, numbering: none)[T] <anderes>\n", 2, None),
    ];
    for &(line, level, expected) in table {
        assert_eq!(
            relevel_line(line, level).as_deref(),
            expected,
            "relevel_line({line:?}, {level}): Titel und Zeilenende wörtlich; Teil, Ebene 0/4 und Nicht-Überschriften → None"
        );
    }
}

#[test]
fn set_numbered_line_switches_only_the_first_line() {
    let body = "= Vorwort // n\nText mit = Zeichen\n== Unter\n";
    let off = set_numbered_line(body, false).unwrap();
    assert_eq!(
        off, "#heading(level: 1, numbering: none)[Vorwort] <baruch-unnumbered> // n\nText mit = Zeichen\n== Unter\n",
        "Nur die erste Überschriftszeile wird Markierung, Kommentar bleibt, Rest byte-gleich"
    );
    assert_eq!(set_numbered_line(&off, true).as_deref(), Some(body), "Zurück ergibt die =-Zeile byte-gleich");
    assert_eq!(set_numbered_line(&off, false), None, "schon ohne Nummer → None");
    assert_eq!(set_numbered_line(body, true), None, "schon mit Nummer → None");
    assert_eq!(
        set_numbered_line("\n\n== Zwei\nx", false).as_deref(),
        Some("\n\n#heading(level: 2, numbering: none)[Zwei] <baruch-unnumbered>\nx"),
        "Leerzeilen davor bleiben"
    );
    assert_eq!(set_numbered_line(&part_line("Teil"), false), None, "Teil → None");
    assert_eq!(set_numbered_line(&part_line("Teil"), true), None, "Teil → None");
    assert_eq!(
        set_numbered_line("= Mit <eigen>\n", false),
        None,
        "=-Zeile mit eigenem Label → None (Show-Regel bleibt)"
    );
    assert_eq!(set_numbered_line("Kein Kopf\n= Später\n", false), None, "keine Überschrift am Anfang → None");
    assert_eq!(set_numbered_line("  = Eingerückt\n", false), None, "eingerückt ist keine Überschrift");
    assert_eq!(set_numbered_line("", false), None, "leerer Text → None");
}

#[test]
fn set_numbered_line_keeps_raw_titles_and_guards_structure() {
    let table = [
        ("= Die *starke* Hand\n", "Die *starke* Hand"),
        ("= a [b] c\n", "a \\[b\\] c"),
        ("= - Strich\n", "\\- Strich"),
        ("= 7. Tag\n", "7\\. Tag"),
        ("= Anna\\'s `[x]` Laube\n", "Anna\\'s `[x]` Laube"),
        ("= Siehe https://beispiel.invalid\n", "Siehe https://beispiel.invalid"),
    ];
    for (line, t) in table {
        let off = set_numbered_line(line, false).unwrap_or_else(|| panic!("{line:?}"));
        assert_eq!(
            off,
            format!("#heading(level: 1, numbering: none)[{t}] <baruch-unnumbered>\n"),
            "T aus dem wörtlichen Titel"
        );
        let before = parse(line).unwrap().headings[0].title.clone();
        let after = parse(&off).unwrap().headings[0].clone();
        assert_eq!((after.title, after.numbered), (before, false), "Der aufgelöste Titel bleibt gleich: {line:?}");
    }
}

#[test]
fn titles_with_code_are_not_projected_to_a_marker() {
    // Befund Gutachten Runde 14: Aus `= Das #emph[Jahr]` wurde `…[Das #emph\[Jahr\]]`; Typst setzte
    // dann den Funktionswert als Raw-Text „emph“ und „[Jahr]“ wörtlich. Mit Code im Titel bleibt die
    // =-Zeile (None), der Export nimmt den Rückfall über die Show-Regel.
    for line in [
        "= Das #emph[Jahr]\n",
        "= Ein Wort#footnote[Erfunden.]\n",
        "= Siehe #link(\"https://beispiel.invalid\")[hier]\n",
        "= Am #strong[Ende] [Zusatz]\n",
    ] {
        assert_eq!(set_numbered_line(line, false), None, "Code im Titel (unmaskiertes #) → None: {line:?}");
    }
    let table = [("= Psalm \\#3\n", "Psalm \\#3"), ("= Das `#emph[x]` im Code\n", "Das `#emph[x]` im Code")];
    for (line, t) in table {
        assert_eq!(
            set_numbered_line(line, false),
            Some(format!("#heading(level: 1, numbering: none)[{t}] <baruch-unnumbered>\n")),
            "maskiertes # und # in Inline-Raw sind kein Code: weiter als Markierung"
        );
    }
}

#[test]
fn the_part_page_rule_is_typst_text_ending_with_a_line_end() {
    let rule = part_page_rule(&["Teil I".to_owned(), String::new(), "Teil \"3\"".to_owned()]);
    assert!(rule.starts_with("#show <baruch-part>:"), "Regel greift über das Label: {rule}");
    assert!(
        rule.ends_with("\n") && rule.matches('\n').count() == 1,
        "eine Zeile mit Zeilenende (verschiebt Zeilen nur um eins)"
    );
    assert!(rule.contains(r#"("Teil I", "", "Teil \"3\"")"#), "Nummern als maskierte String-Literale: {rule}");
    assert!(part_page_rule(&["I".to_owned()]).contains(r#"("I",)"#), "ein Element braucht das Komma, sonst kein Array");
    assert!(part_page_rule(&[]).contains("()"), "ohne Nummern ein leeres Array");
}
