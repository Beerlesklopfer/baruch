// SPDX-License-Identifier: AGPL-3.0-or-later
//! Maskierungen in Überschriften, Nummerierungsmuster und neue Überschriftszeilen (Vertrag Runde 4,
//! Abschnitte 2 und 3a). Eigene Testdatei `tests/data/maskiert.typ`.

use baruch_typst::{
    escape_title, format_number, heading_line, heading_numbering, is_numbering_pattern, join, parse, unescape_title,
};

const MASKED: &str = include_str!("data/maskiert.typ");

#[test]
fn unescaping_resolves_typst_escapes_in_titles() {
    assert_eq!(unescape_title("Anna\\'s Laube"), "Anna's Laube", "\\' wird zum Apostroph");
    assert_eq!(unescape_title("\\#1 \\*fett\\* \\_x\\_"), "#1 *fett* _x_", "Markup-Zeichen werden wörtlich");
    assert_eq!(unescape_title("a \\\\ b"), "a \\ b", "\\\\ wird ein Backslash");
    assert_eq!(unescape_title("\\\\'"), "\\'", "erst \\\\, dann der Rest: kein zweites Auflösen");
    assert_eq!(unescape_title("Zeile\\nweiter \\u{41}"), "Zeile\\nweiter \\u{41}", "andere Backslashes bleiben");
    assert_eq!(unescape_title("Ende\\"), "Ende\\", "ein Backslash am Ende bleibt");
}

#[test]
fn escaping_and_unescaping_are_inverse_for_markup_characters() {
    for title in [
        "#Hashtag *fett* _kursiv_",
        "$5 für @alle",
        "a < b > c",
        "[Klammer] und `Code`",
        "Pfad C:\\Daten\\",
        "~ und // kein Kommentar /* auch nicht */",
        "Anna's Laube",
        "Ganz normal",
    ] {
        let escaped = escape_title(title);
        assert_eq!(unescape_title(&escaped), title, "Rundreise über escape/unescape: {escaped:?}");
        // Die neue Überschriftszeile ergibt beim Zerlegen wieder genau diesen Titel.
        let line = heading_line(2, title, None);
        let doc = parse(&line).unwrap();
        assert_eq!(doc.headings.len(), 1, "eine Überschrift aus {line:?}");
        assert_eq!(doc.headings[0].title, title, "Titel überlebt die Überschriftszeile {line:?}");
        assert_eq!(doc.headings[0].level, 2, "Ebene aus der Zahl der =");
    }
    assert_eq!(escape_title("  zwei\nZeilen "), "zwei Zeilen", "Zeilenende wird Leerzeichen, Ränder getrimmt");
    // Typst bricht auch bei diesen Zeichen um; ein Titel aus dem Baum darf keine zweite Zeile beginnen.
    for brk in ['\x0B', '\x0C', '\u{85}', '\u{2028}', '\u{2029}'] {
        assert!(baruch_typst::is_newline(brk), "{brk:?} ist ein Zeilenende im Sinn von Typst");
        let line = heading_line(2, &format!("Titel{brk}= Neu"), None);
        assert_eq!(line, "== Titel = Neu\n", "{brk:?} wird Leerzeichen, keine zweite Überschrift");
    }
}

#[test]
fn parse_gives_the_resolved_and_the_literal_title() {
    let doc = parse(MASKED).unwrap();
    let pairs: Vec<(&str, &str)> = doc.headings.iter().map(|h| (h.title.as_str(), h.raw_title.as_str())).collect();
    assert_eq!(
        pairs,
        vec![
            ("Anna's Laube", "Anna\\'s Laube"),
            ("#1 der Liste", "\\#1 der Liste"),
            ("Preis: $5 *netto*", "Preis: \\$5 \\*netto\\*"),
            ("Ohne Maskierung", "Ohne Maskierung"),
        ],
        "title aufgelöst, raw_title wörtlich (ohne Label und Kommentar)"
    );
    let joined = join(&doc.template, doc.headings.iter().map(|h| h.body.as_str()));
    assert_eq!(joined.as_bytes(), MASKED.as_bytes(), "join bleibt byte-gleich, der Text behält die Maskierung");
}

#[test]
fn heading_numbering_reads_the_last_top_level_rule_outside_comments() {
    let template = "// #set heading(numbering: \"a)\")\n/* #set heading(numbering: \"A\") */\n\
                    #set heading(numbering: \"I.1\")\n#set heading(numbering: \"1\")\n#set heading(outlined: true)\n";
    assert_eq!(
        heading_numbering(template).as_deref(),
        Some("1"),
        "Kommentare zählen nicht, eine Regel ohne numbering ändert nichts, die letzte mit numbering gilt (wie Typst)"
    );
    assert_eq!(heading_numbering("#set heading(numbering: none)\n").as_deref(), Some(""), "none → keine Nummern");
    assert_eq!(heading_numbering("#set text(lang: \"de\")\n"), None, "ohne Regel kein Muster");
    assert_eq!(
        heading_numbering("#set heading(numbering: (..n) => n.pos().map(str).join(\".\"))\n"),
        None,
        "eine Funktion ist nicht auswertbar"
    );
    assert_eq!(heading_numbering("```\n#set heading(numbering: \"1.\")\n```\n"), None, "Raw-Text zählt nicht");
}

#[test]
fn heading_numbering_finds_the_pattern_inside_a_numbering_function() {
    // Frei erfundene Vorlagen: eine Funktion direkt in der Regel, die Kapitel römisch zählt.
    let inline = "#set heading(numbering: (..n) => numbering(\"I\", ..n))\n";
    assert_eq!(
        heading_numbering(inline).as_deref(),
        Some("I"),
        "eine Funktion, die numbering(\"I\", ..n) aufruft, liefert ihr Muster"
    );
    let branching = "#set heading(numbering: (..zahlen) => {\n  let z = zahlen.pos()\n  \
                     if z.len() == 1 { numbering(\"I\", ..z) } else { numbering(\"1.1\", ..z.slice(1)) }\n})\n";
    assert_eq!(
        heading_numbering(branching).as_deref(),
        Some("I"),
        "mehrere Aufrufe über Zeilen: das erste Muster gilt (Kapitel)"
    );
    assert_eq!(
        heading_numbering("#set heading(numbering: (..n) => my-numbering(\"a\", ..n))\n"),
        None,
        "nur der Aufruf von numbering selbst zählt, nicht eine Funktion mit ähnlichem Namen"
    );
}

#[test]
fn heading_numbering_follows_the_name_of_a_let_function() {
    let template = "#let leise(body) = text(size: 9pt, body)\n\
                    #let kapitelzahl(..teile) = numbering(\"I\", ..teile)\n\
                    #set text(lang: \"de\")\n\
                    #set heading(numbering: kapitelzahl)\n";
    assert_eq!(
        heading_numbering(template).as_deref(),
        Some("I"),
        "der Name einer #let-Funktion, deren Rumpf numbering(\"I\", …) aufruft, liefert das Muster"
    );
    let block = "#let zaehlung = (..n) => {\n  numbering(\"A.1\", ..n)\n}\n#set heading(numbering: zaehlung)\n";
    assert_eq!(
        heading_numbering(block).as_deref(),
        Some("A.1"),
        "auch #let name = (..n) => {{ … }} über mehrere Zeilen"
    );
    let commented = "// #let alt(..n) = numbering(\"i\", ..n)\n#let alt(..n) = n.pos().map(str).join(\"-\")\n\
                     #set heading(numbering: alt)\n";
    assert_eq!(
        heading_numbering(commented),
        None,
        "ein auskommentierter Rumpf zählt nicht, der echte ruft numbering nicht auf"
    );
    assert_eq!(heading_numbering("#set heading(numbering: unbekannt)\n"), None, "ohne #let-Definition kein Muster");
}

#[test]
fn rules_inside_show_rules_content_blocks_and_functions_do_not_count() {
    // Frei erfundene Vorlage: nur die Regel ganz oben gilt, die anderen stehen in Rümpfen.
    let template = "#set heading(numbering: \"I.1\")\n\
                    #show heading.where(level: 3): it => [#set heading(numbering: \"a\"); #it]\n\
                    #show: rest => [\n  #set heading(numbering: \"A\")\n  #rest\n]\n\
                    #let anhang(body) = {\n  set heading(numbering: \"i\")\n  body\n}\n\
                    #let kasten(body) = block[#set heading(numbering: \"*\") #body]\n";
    assert_eq!(
        heading_numbering(template).as_deref(),
        Some("I.1"),
        "#set heading in #show-Rümpfen, Inhaltsblöcken und Funktionen zählt nicht, nur die oberste Regel"
    );
}

#[test]
fn a_depth_condition_numbers_only_the_upper_levels() {
    let roman = "#set heading(numbering: (..n) => if n.pos().len() == 1 { numbering(\"I\", ..n) } else { \"\" })\n";
    assert_eq!(heading_numbering(roman).as_deref(), Some("I|1"), "== 1 mit leerem Sonst-Zweig: nur Kapitel römisch");
    let numbering = baruch_typst::scheme::Numbering::parse_spec("I|1").unwrap();
    assert_eq!(numbering.heading(&[2], 3), "II", "Kapitel 2 römisch");
    assert_eq!(numbering.heading(&[2, 1], 3), "", "Ebene 2 hat keine Nummer");
    assert_eq!(numbering.heading(&[2, 1, 4], 3), "", "Ebene 3 hat keine Nummer");

    for (condition, expected) in [("<= 2", "1.a|2"), ("< 3", "1.a|2"), ("< 2", "1.a|1")] {
        let template = format!(
            "#set heading(numbering: (..n) => if n.pos().len() {condition} {{ numbering(\"1.a\", ..n) }} else {{ none }})\n"
        );
        assert_eq!(
            heading_numbering(&template).as_deref(),
            Some(expected),
            "len() {condition} mit none im Sonst-Zweig"
        );
    }
    let second = "#set heading(numbering: (..n) => if n.pos().len() == 2 { numbering(\"1.1\", ..n) } else { [] })\n";
    assert_eq!(
        heading_numbering(second).as_deref(),
        Some(";1.1"),
        "== 2: Liste nur mit Position 2, Kapitel ohne Nummer"
    );
    let without_else =
        "#set heading(numbering: (..n) => { let p = n.pos(); if p.len() <= 1 { numbering(\"A.\", ..p) } })\n";
    assert_eq!(
        heading_numbering(without_else).as_deref(),
        Some("A.|1"),
        "fehlendes else = ohne Nummer, auch über let p = n.pos()"
    );
}

#[test]
fn a_depth_condition_is_found_through_let_names() {
    let direct = "#let kapitel(..n) = if n.pos().len() == 1 { numbering(\"I\", ..n) } else { \"\" }\n\
                  #set text(lang: \"de\")\n#set heading(numbering: kapitel)\n";
    assert_eq!(heading_numbering(direct).as_deref(), Some("I|1"), "#let name(..n) = if … über den Namen");
    let lambda = "#let zaehlung = (..zahlen) => {\n  let z = zahlen.pos()\n  if z.len() < 3 {\n    numbering(\"1.1\", ..z)\n  } else {\n    \"\"\n  }\n}\n\
                  #set heading(numbering: zaehlung)\n";
    assert_eq!(heading_numbering(lambda).as_deref(), Some("1.1|2"), "#let name = (..n) => {{ … }} über mehrere Zeilen");
    let redefined = "#let k(..n) = numbering(\"a\", ..n)\n#let k(..n) = if n.pos().len() == 1 { numbering(\"I\", ..n) }\n\
                     #set heading(numbering: k)\n";
    assert_eq!(heading_numbering(redefined).as_deref(), Some("I|1"), "die letzte Definition vor der Regel gilt");
}

#[test]
fn a_chain_of_levels_gives_a_pattern_per_level() {
    let chain = "#set heading(numbering: (..n) => { let p = n.pos(); if p.len() == 1 { numbering(\"I.\", p.at(0)) } \
                 else if p.len() == 2 { numbering(\"A.\", p.at(1)) } else if p.len() == 3 { numbering(\"1.\", p.at(2)) } \
                 else { \"\" } })\n";
    assert_eq!(heading_numbering(chain).as_deref(), Some("I.;A.;1."), "je Ebene die eigene Zahl mit eigenem Muster");
    let doubled = "#set heading(numbering: (..n) => { let p = n.pos(); if p.len() == 1 { numbering(\"a\", p.at(0)) * 2 + \")\" } \
                   else { \"\" } })\n";
    assert_eq!(
        heading_numbering(doubled).as_deref(),
        Some("aa)|1"),
        "numbering(\"a\", x) * 2 ist der verdoppelte Buchstabe"
    );
}

#[test]
fn format_number_follows_typst_rules() {
    assert_eq!(format_number("I.1", &[2, 3]), "II.3", "römisches Kapitel, arabischer Abschnitt");
    assert_eq!(format_number("I.1", &[4]), "IV", "Ebene 1 nur das erste Zählzeichen");
    assert_eq!(format_number("1.1", &[1, 2, 3]), "1.2.3", "das letzte Zählzeichen wiederholt sich mit Präfix");
    assert_eq!(format_number("1.", &[2]), "2.", "Suffix steht am Ende");
    assert_eq!(format_number("1.", &[2, 1]), "2.1.", "ohne Präfix trennt das Suffix die wiederholten Zahlen");
    assert_eq!(format_number("(1)", &[3]), "(3)", "Präfix und Suffix");
    assert_eq!(
        format_number("§ A:", &[2]),
        "§ B:",
        "Großbuchstabe zählt, anderer Text bleibt (wie in Typst ist jedes a, i, 1 … ein Zählzeichen)"
    );
    assert_eq!(format_number("a", &[27]), "aa", "Buchstaben bijektiv zur Basis 26");
    assert_eq!(format_number("i", &[14]), "xiv", "kleine römische Zahl");
    assert_eq!(format_number("*", &[2]), "†", "Symbole");
    assert_eq!(format_number("1.1", &[]), "", "ohne Zahlen keine Nummer");
    assert_eq!(format_number("Ohne", &[1]), "", "ohne Zählzeichen keine Nummer");
    assert!(is_numbering_pattern("I.1") && !is_numbering_pattern("Ohne"), "Zählzeichen erkannt");
}

#[test]
fn a_new_heading_line_keeps_label_and_comment_of_the_old_one() {
    assert_eq!(heading_line(1, "Neu", None), "= Neu\n", "einfache Zeile mit Zeilenende");
    assert_eq!(
        heading_line(2, "Neu", Some("= Alt <kap:alt> // Notiz\n")),
        "== Neu <kap:alt> // Notiz\n",
        "Label und Kommentar bleiben, Ebene neu"
    );
    assert_eq!(heading_line(3, "A*B", Some("== Alt\n")), "=== A\\*B\n", "Titel maskiert, ohne Label kein Anhang");
}
