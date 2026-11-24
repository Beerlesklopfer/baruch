// SPDX-License-Identifier: AGPL-3.0-or-later
//! Zerlegen und Zusammensetzen kleiner, eigener Typst-Dateien (`tests/data/kleines-buch.typ`,
//! `tests/data/grenzen.typ`) und einzelner Grenzfälle. Jeder Fall prüft auch, dass `join` die Eingabe
//! Byte für Byte zurückgibt.

use baruch_typst::{Document, ParseError, join, parse};

const BOOK: &str = include_str!("data/kleines-buch.typ");
const EDGES: &str = include_str!("data/grenzen.typ");

fn levels_and_titles(source: &str) -> Vec<(u8, String)> {
    parse(source).unwrap().headings.into_iter().map(|h| (h.level, h.title)).collect()
}

/// Zerlegen und prüfen, dass `join` Byte für Byte die Eingabe ergibt.
fn round_trip(source: &str) -> Document {
    let doc = parse(source).unwrap_or_else(|e| panic!("{e}: {source:?}"));
    let joined = join(&doc.template, doc.headings.iter().map(|h| h.body.as_str()));
    assert_eq!(joined.as_bytes(), source.as_bytes(), "join(parse(x)) muss x Byte für Byte ergeben: {source:?}");
    doc
}

fn titles(doc: &Document) -> Vec<&str> {
    doc.headings.iter().map(|h| h.title.as_str()).collect()
}

#[test]
fn joining_the_parsed_book_gives_back_the_source_byte_for_byte() {
    assert!(!BOOK.ends_with('\n'), "Testdatei soll ohne Schluss-Zeilenende enden");
    round_trip(BOOK);
}

#[test]
fn headings_of_level_one_to_three_become_the_outline() {
    let found = levels_and_titles(BOOK);
    let expected = [
        (1, "Vorwort"),
        (1, "Erstes Kapitel"),
        (2, "Ein Abschnitt"),
        (3, "Ein Unterabschnitt"),
        (2, "Zweiter Abschnitt"),
    ];
    let expected: Vec<(u8, String)> = expected.iter().map(|&(l, t)| (l, t.to_owned())).collect();
    assert_eq!(
        found, expected,
        "Kommentar, Raw-Block, '===='-Zeile und eingerückte '= '-Zeile dürfen keine Überschrift werden; Labels fallen weg"
    );
}

#[test]
fn the_template_is_everything_before_the_first_heading() {
    let doc = parse(BOOK).unwrap();
    assert!(doc.template.starts_with("// Kleines Testbuch"), "Vorlage beginnt am Dateianfang");
    assert!(doc.template.contains("#let szene(body)"), "Definitionen gehören zur Vorlage");
    assert!(doc.template.contains("= Kein Kapitel, nur ein Kommentar"), "Kommentar bleibt in der Vorlage");
    assert!(doc.template.ends_with("*/\n\n"), "Vorlage endet direkt vor der ersten Überschrift: {:?}", doc.template);
}

#[test]
fn each_body_runs_from_its_heading_line_to_the_next_heading() {
    let doc = parse(BOOK).unwrap();
    let first = &doc.headings[0];
    assert!(first.body.starts_with("= Vorwort <vorwort>\n"), "Text beginnt mit der Überschriftszeile samt Label");
    assert!(first.body.ends_with("„Anführung“.\n\n"), "Leerzeilen vor der nächsten Überschrift gehören dazu");
    let section = &doc.headings[2];
    assert!(section.body.contains("= Das steht in einem Raw-Block"), "Raw-Block bleibt im Abschnitt");
    assert!(section.body.contains("==== Vierte Ebene bleibt Text"), "vierte Ebene bleibt im Abschnitt");
    let chapter = &doc.headings[1];
    assert!(chapter.body.contains(" = eingerückt"), "eingerückte Zeile bleibt im Kapitel");
    assert_eq!(
        doc.headings[4].body, "== Zweiter Abschnitt\n\nText ohne Schluss-Zeilenende.",
        "letzter Text ohne Zeilenende"
    );
}

#[test]
fn title_and_author_come_from_set_document() {
    let doc = parse(BOOK).unwrap();
    assert_eq!(doc.title.as_deref(), Some("Größe und Übermaß"), "Titel aus dem String-Literal");
    assert_eq!(doc.author.as_deref(), Some("Jörg Müller, Anna Weiß"), "Autoren aus dem Tupel, mit \", \" verbunden");
}

#[test]
fn a_file_starting_with_a_heading_has_an_empty_template_and_no_metadata() {
    let doc = round_trip("= Eins\nText\n== Zwei\n");
    assert_eq!(doc.template, "", "keine Präambel");
    assert_eq!((doc.title, doc.author), (None, None), "ohne #set document kein Titel und Autor");
}

#[test]
fn a_file_without_headings_is_refused() {
    let err = parse("#set text(lang: \"de\")\nNur Text.\n").unwrap_err();
    assert!(matches!(err, ParseError::NoHeadings), "ohne Überschrift kein Werk");
    assert_eq!(err.to_string(), "keine Überschrift (= …) gefunden", "deutsche Meldung");
}

#[test]
fn crlf_is_left_alone_and_still_round_trips() {
    let doc = round_trip("#set document(title: [Zeilen])\r\n= Eins\r\nText\r\n");
    assert_eq!(doc.headings[0].title, "Eins", "\\r gehört nicht zum Titel");
    assert_eq!(doc.title.as_deref(), Some("Zeilen"), "Titel aus Inhaltsblock");
}

#[test]
fn a_byte_order_mark_does_not_swallow_the_first_heading() {
    // Windows-Editoren schreiben gern U+FEFF an den Anfang; die erste Überschrift darf deshalb
    // nicht in der Werkvorlage verschwinden.
    let doc = round_trip("\u{feff}= Erstes\nText\n= Zweites\nMehr\n");
    assert_eq!(titles(&doc), ["Erstes", "Zweites"], "beide Kapitel erkannt, auch direkt hinter dem BOM");
    assert_eq!(doc.template, "\u{feff}", "der BOM bleibt Werkvorlage, damit die Rundreise byte-gleich bleibt");

    let single = parse("\u{feff}= Nur eins\n").expect("eine einzige Überschrift hinter dem BOM ist kein NoHeadings");
    assert_eq!(single.headings.len(), 1, "genau ein Kapitel");
}

#[test]
fn headings_inside_a_multi_line_content_block_are_not_outline() {
    let doc = round_trip("= Eins\n#block(inset: 4pt)[\n= Kein Kapitel\n  [innen]\n]\n= Zwei\n");
    assert_eq!(titles(&doc), ["Eins", "Zwei"], "eine Überschriftszeile im #block[…] bleibt Text des Kapitels");
    assert!(doc.headings[0].body.contains("= Kein Kapitel"), "der Block bleibt ganz im ersten Kapitel");

    let doc = round_trip("= Eins\n#szene[\n== Nicht hier\n]\n== Hier\n");
    assert_eq!(titles(&doc), ["Eins", "Hier"], "#name[…] ohne Argumente zählt wie #block(…)[…]");

    let doc = round_trip("= Eins\n#figure(\n  caption: [\n= Nein\n  ],\n)[Bild]\n= Zwei\n");
    assert_eq!(
        titles(&doc),
        ["Eins", "Zwei"],
        "[…] in einer mehrzeiligen Argumentliste ist kein Markup der obersten Ebene"
    );

    let doc = round_trip("= Eins\n#grid[a][\n= Nein\n]\n= Zwei\n");
    assert_eq!(titles(&doc), ["Eins", "Zwei"], "auch ein zweiter, angehängter Inhaltsblock zählt");
}

#[test]
fn headings_inside_content_blocks_of_let_and_show_rules_are_not_outline() {
    let doc = round_trip("#let kasten = [\n= Kein Kapitel\n]\n#show heading: it => [\n= Auch nicht\n#it\n]\n= Eins\n");
    assert_eq!(titles(&doc), ["Eins"], "Inhaltsblöcke in #let und #show gehören zur Vorlage");
    assert!(doc.template.contains("= Auch nicht"), "die Show-Regel bleibt ganz in der Vorlage");
}

#[test]
fn a_lone_bracket_in_running_text_does_not_hide_later_headings() {
    let doc = round_trip("= Eins\nText mit [ offener Klammer und ] einzelner.\n= Zwei\n");
    assert_eq!(titles(&doc), ["Eins", "Zwei"], "[ im Fließtext der obersten Ebene ist Text, keine Klammer");
}

#[test]
fn headings_inside_multi_line_inline_raw_are_not_outline() {
    let doc = round_trip("= Eins\nCode: `erste\n= Kein Kapitel\n` weiter.\n= Zwei\n");
    assert_eq!(titles(&doc), ["Eins", "Zwei"], "Inline-Raw über Zeilen schützt die Überschriftszeile");

    let doc = round_trip("= Eins\nDoppelt: ``a\n= Nein\n`` und `x`.\n= Zwei\n");
    assert_eq!(titles(&doc), ["Eins", "Zwei"], "Inline-Raw mit zwei Backticks endet erst an zwei Backticks");
}

#[test]
fn headings_inside_multi_line_strings_are_not_outline() {
    let doc = round_trip("#let hinweis = \"\n= Kein Kapitel\n\"\n= Eins\n#let b = (\"a\", \"\n== Nein\n\")\n== Ja\n");
    assert_eq!(titles(&doc), ["Eins", "Ja"], "Überschriftszeilen in Strings im Code zählen nicht");
    let doc = round_trip("= Eins\nEr sagte \"Hallo\n= Zwei\n");
    assert_eq!(titles(&doc), ["Eins", "Zwei"], "ein Anführungszeichen im Fließtext öffnet keinen String");
}

#[test]
fn document_meta_ignores_commented_out_set_document() {
    let doc = round_trip("/*\n#set document(title: \"Alt\")\n*/\n#set document(title: \"Neu\")\n= Eins\n");
    assert_eq!(doc.title.as_deref(), Some("Neu"), "#set document in /* … */ zählt nicht");
    let doc = round_trip("/* #set document(title: \"Alt\") */\n= Eins\n");
    assert_eq!(doc.title, None, "nur auskommentiert → kein Titel");
    let doc = round_trip("#set text(lang: \"de\") // #set document(title: \"Hinten\")\n= Eins\n");
    assert_eq!(doc.title, None, "auch ein //-Kommentar hinter Code zählt nicht");
}

#[test]
fn a_line_comment_behind_a_heading_is_not_part_of_the_title() {
    let doc = round_trip("== Titel // Kommentar\nText.\n= Link https://example.org/x\n");
    assert_eq!(titles(&doc), ["Titel", "Link https://example.org/x"], "Kommentar weg, Link bleibt");
    assert_eq!(doc.headings[0].body, "== Titel // Kommentar\nText.\n", "der Text behält die Zeile samt Kommentar");
}

#[test]
fn the_edge_case_file_parses_and_round_trips() {
    let doc = round_trip(EDGES);
    assert_eq!(
        titles(&doc),
        ["Erstes Kapitel", "Abschnitt mit Link https://example.org/a", "Zweites Kapitel"],
        "nur die drei echten Überschriften; Strings, Show-Regel, Kasten, Raw und Bildunterschrift zählen nicht"
    );
    assert_eq!(doc.title.as_deref(), Some("Grenzfälle"), "Titel aus dem einzigen nicht auskommentierten #set document");
    assert_eq!(doc.author.as_deref(), Some("Test"), "Autor aus demselben #set document");
    assert!(doc.template.contains("= Kein Kapitel aus einer Show-Regel"), "Show-Regel bleibt Vorlage");
}

#[test]
fn titles_without_escapes_are_the_same_resolved_and_literal() {
    let doc = round_trip(BOOK);
    for heading in &doc.headings {
        assert_eq!(heading.title, heading.raw_title, "ohne Maskierung sind beide Titel gleich: {heading:?}");
    }
}
