// SPDX-License-Identifier: AGPL-3.0-or-later
//! `serialize`: unberührte Blöcke bleiben wörtlich, bearbeitete werden kanonisch erzeugt, Inseln nur
//! als Ganzes gelöscht; doppelte und unbekannte Indizes sind Fehler.

use baruch_wysiwyg::{BlockKind, Edit, Inline, WysiwygError, parse, serialize};

const TEXT: &str = "= Der Brunnen <brunnen> // Notiz\n\nErster   Absatz mit  Leerraum,\nden niemand anfasst.\n\n#let zahl = 3\n\nZweiter Absatz.\n";

fn text(v: &str) -> Inline {
    Inline::Text { v: v.into() }
}

fn keep(keep: usize) -> Edit {
    Edit::Keep { keep }
}

fn replace(index: usize, block: BlockKind) -> Edit {
    Edit::Block { replaces: Some(index), block }
}

#[test]
fn only_the_edited_block_changes() {
    let doc = parse(TEXT);
    assert_eq!(doc.blocks.len(), 4, "Überschrift, Absatz, Insel, Absatz: {:#?}", doc.blocks);
    let edits = [keep(0), keep(1), keep(2), replace(3, BlockKind::Paragraph { inlines: vec![text("Neuer Schluss.")] })];
    let out = serialize(&doc, &edits).unwrap();
    assert_eq!(
        out,
        TEXT.replace("Zweiter Absatz.", "Neuer Schluss."),
        "der unberührte Absatz behält seine Leerzeichen und Zeilen, nur der bearbeitete ist neu"
    );
}

#[test]
fn an_edited_heading_keeps_label_and_comment_of_the_original_line() {
    let doc = parse(TEXT);
    let heading = BlockKind::Heading { level: 1, inlines: vec![text("Der alte Brunnen")], label: None };
    let out = serialize(&doc, &[replace(0, heading), keep(1), keep(2), keep(3)]).unwrap();
    assert!(out.starts_with("= Der alte Brunnen <brunnen> // Notiz\n\n"), "Label und Kommentar bleiben: {out}");
    let labelled = BlockKind::Heading {
        level: 1,
        inlines: vec![text("Der alte Brunnen "), Inline::Label { v: "brunnen".into() }],
        label: None,
    };
    let out = serialize(&doc, &[replace(0, labelled), keep(1), keep(2), keep(3)]).unwrap();
    assert!(
        out.starts_with("= Der alte Brunnen <brunnen> // Notiz\n\n"),
        "trägt die neue Überschrift das Label selbst, steht es nicht doppelt da: {out}"
    );
}

#[test]
fn an_island_can_be_deleted_but_not_edited_or_created() {
    let doc = parse(TEXT);
    let out = serialize(&doc, &[keep(0), keep(1), keep(3)]).unwrap();
    assert!(!out.contains("#let"), "Insel als Ganzes gelöscht: {out}");
    assert!(out.contains("anfasst.\n\nZweiter Absatz."), "zwischen den Absätzen bleibt eine Leerzeile: {out}");
    let edited = serialize(&doc, &[replace(2, BlockKind::Paragraph { inlines: vec![text("x")] })]);
    assert_eq!(edited, Err(WysiwygError::IslandEdited(2)), "eine Insel lässt sich nicht in Text verwandeln");
    let as_island = serialize(&doc, &[replace(1, BlockKind::Island)]);
    assert_eq!(as_island, Err(WysiwygError::IslandEdited(1)), "ein Block wird nicht zur Insel");
    let new_island = serialize(&doc, &[Edit::Block { replaces: None, block: BlockKind::Island }]);
    assert_eq!(new_island, Err(WysiwygError::NewIsland), "eine neue Insel gibt es im Satzbild nicht");
}

#[test]
fn duplicate_and_unknown_indices_are_refused() {
    let doc = parse(TEXT);
    assert_eq!(serialize(&doc, &[keep(0), keep(0)]), Err(WysiwygError::DuplicateBlock(0)), "Keep zweimal");
    let twice = [keep(1), replace(1, BlockKind::Paragraph { inlines: vec![text("x")] })];
    assert_eq!(serialize(&doc, &twice), Err(WysiwygError::DuplicateBlock(1)), "Keep und Ersatz desselben Blocks");
    assert_eq!(
        serialize(&doc, &[keep(9)]),
        Err(WysiwygError::UnknownBlock { index: 9, count: 4 }),
        "unbekannter Index"
    );
    let message = WysiwygError::UnknownBlock { index: 9, count: 4 }.to_string();
    assert!(message.contains("Block 9 gibt es nicht"), "deutsche Meldung: {message}");
}

#[test]
fn inserted_and_moved_blocks_are_separated_by_a_blank_line() {
    let doc = parse("Eins\nZwei\n= Titel\nDrei");
    assert_eq!(doc.blocks.len(), 3, "Absatz, Überschrift, Absatz ohne Leerzeilen: {:#?}", doc.blocks);
    let inserted = Edit::Block { replaces: None, block: BlockKind::Paragraph { inlines: vec![text("Neu")] } };
    let out = serialize(&doc, &[keep(0), keep(1), keep(2), inserted]).unwrap();
    assert_eq!(out, "Eins\nZwei\n= Titel\nDrei\n\nNeu\n\n", "der neue Absatz verschmilzt nicht mit „Drei“");
    let moved = serialize(&doc, &[keep(2), keep(0)]).unwrap();
    assert_eq!(moved, "Drei\n\nEins\nZwei\n", "umgestellt: Leerzeile dazwischen, sonst wörtlich");
    let emptied = serialize(&doc, &[replace(0, BlockKind::Paragraph { inlines: vec![] }), keep(1), keep(2)]).unwrap();
    assert_eq!(emptied, "= Titel\nDrei", "ein leer gewordener Block entfällt");
}

#[test]
fn edits_have_the_json_shape_of_the_contract() {
    let edits: Vec<Edit> = serde_json::from_str(
        r#"[{"keep": 0}, {"replaces": 1, "block": {"kind": "paragraph", "inlines": [{"t": "text", "v": "Hallo"}, {"t": "soft"}, {"t": "emph", "c": [{"t": "text", "v": "Welt"}]}]}}, {"block": {"kind": "heading", "level": 2, "inlines": [{"t": "text", "v": "Neu"}]}}]"#,
    )
    .expect("Vertragsform lässt sich lesen");
    assert_eq!(edits[0], keep(0), "Keep als {{keep}}");
    assert!(matches!(&edits[1], Edit::Block { replaces: Some(1), .. }), "Ersatz mit replaces");
    assert!(matches!(&edits[2], Edit::Block { replaces: None, .. }), "fehlendes replaces = eingefügt");
    let doc = parse("= Titel\n\nAlt\n");
    let blocks = serde_json::to_value(&doc).unwrap();
    assert_eq!(blocks["blocks"][0]["kind"], "heading", "Art steht flach im Block");
    assert_eq!(blocks["blocks"][0]["raw"], "= Titel\n\n", "raw mit den folgenden Leerzeilen");
    assert_eq!(blocks["blocks"][0]["inlines"][0]["t"], "text", "Inline mit Kennung t");
    let island: baruch_wysiwyg::Block = serde_json::from_str(r#"{"kind": "island"}"#).unwrap();
    assert_eq!(island.raw, "", "raw ist mit #[serde(default)] optional");
}

/// Jeden Block unverändert als neu erzeugten Block schicken (wie das Satzbild nach einem Tastendruck).
fn regenerate_all(source: &str) -> String {
    let doc = parse(source);
    let edits: Vec<Edit> = doc.blocks.iter().enumerate().map(|(i, b)| replace(i, b.kind.clone())).collect();
    serialize(&doc, &edits).unwrap()
}

#[test]
fn regenerated_blocks_keep_escapes_that_would_otherwise_become_shorthands() {
    // Befund nach Runde 5: parse löste `\'` zu `'` auf, emit maskierte es nicht wieder, und Typst
    // machte daraus ein typografisches Anführungszeichen (ebenso `--`, `...`, `-?`).
    let heading = regenerate_all("= Anna\\'s Laube <laube>\n");
    assert_eq!(heading, "= Anna\\'s Laube <laube>\n", "der gerade Apostroph bleibt maskiert");
    let shorthands = concat!(r#"a\--b und \"zitat\" und \.\.\. und Wort\-?teil und -\?x und -\1"#, "\n");
    // Von `\.\.\.` muss der erste Punkt nicht maskiert bleiben: hinter ihm stehen nur maskierte Punkte,
    // `...` entsteht so nicht. Kanonisch ist darum `.\.\.`, gleichbedeutend mit dem Original.
    assert_eq!(
        regenerate_all(shorthands),
        shorthands.replace(r"\.\.\.", r".\.\."),
        "keine Maskierung geht verloren, die einen Halbgeviertstrich, Anführungszeichen, Auslassungspunkte, \
         ein weiches Trennzeichen oder ein Minus verhinderte"
    );
    let scene = regenerate_all(concat!("#szene[\n  Er sagte ", r#"\"nein\""#, ".\n\n  Zweiter Absatz.\n]\n"));
    assert!(scene.contains(r#"\"nein\""#), "auch im neu erzeugten Rumpf von #szene bleiben die Maskierungen: {scene}");
}

#[test]
fn harmless_escapes_stay_editable_text() {
    // Nur Maskierungen, die als Text etwas anderes bedeuteten, werden Inseln; ein einzelner `\.` hinter
    // einer Zahl (kein `...`) bleibt Text, damit das Satzbild ihn bearbeiten kann.
    let doc = parse("Am 1\\. Mai, \\#2.\n");
    let BlockKind::Paragraph { inlines } = &doc.blocks[0].kind else {
        panic!("ein Absatz: {:#?}", doc.blocks);
    };
    assert_eq!(inlines, &[text("Am 1. Mai, #2.")], "kein Inline wird zur Insel");
    let doc = parse("Anna\\'s\n");
    let BlockKind::Paragraph { inlines } = &doc.blocks[0].kind else {
        panic!("ein Absatz: {:#?}", doc.blocks);
    };
    assert_eq!(
        inlines,
        &[text("Anna"), Inline::Island { raw: "\\'".into() }, text("s")],
        "der maskierte Apostroph ist eine Insel, der Text rundherum bleibt bearbeitbar"
    );
}

#[test]
fn every_typst_line_break_in_text_becomes_a_space() {
    // Typst bricht auch bei U+2028, U+2029, U+0085, \\x0B und \\x0C um; dahinter wäre `= ` eine Überschrift.
    for (brk, marker) in [('\u{2028}', "= "), ('\u{2029}', "- "), ('\u{85}', "+ "), ('\x0B', "1. "), ('\x0C', "/ ")] {
        let para = BlockKind::Paragraph { inlines: vec![text(&format!("Satz{brk}{marker}weiter"))] };
        let doc = parse("Alt.\n");
        let out = serialize(&doc, &[replace(0, para)]).unwrap();
        assert!(!out.contains(brk), "kein Zeilenende {brk:?} bleibt im Text: {out:?}");
        let reparsed = parse(&out);
        assert_eq!(reparsed.blocks.len(), 1, "der Absatz zerfällt nicht in Struktur ({marker:?}): {out:?}");
        assert!(
            matches!(reparsed.blocks[0].kind, BlockKind::Paragraph { .. }),
            "es bleibt ein Absatz ({marker:?}): {out:?}"
        );
        let heading =
            BlockKind::Heading { level: 2, inlines: vec![text(&format!("Titel{brk}= Zweiter"))], label: None };
        let out = serialize(&parse("== Alt\n"), &[replace(0, heading)]).unwrap();
        // Runde 7: Die Überschrift endet am Zeilenende; der Rest wird ein eigener Absatz (maskiert),
        // statt als „Titel = Zweiter“ in der Überschriftszeile zu landen.
        assert_eq!(out, "== Titel\n\n\\= Zweiter\n", "Überschrift bis zum Zeilenende, dann ein Absatz ({brk:?})");
        assert!(!out.contains(brk), "auch nicht in der Überschrift: {out:?}");
        let reparsed = parse(&out);
        assert!(
            matches!(&reparsed.blocks[0].kind, BlockKind::Heading { inlines, .. } if inlines == &[text("Titel")]),
            "der Titel ist nur die erste Zeile ({brk:?}): {out:?}"
        );
    }
}

/// Runde 7, „Absatz wird Titel“: So schickte das Widget eine Überschrift, hinter deren Titel
/// Umschalt+Enter (bzw. ein `<br>` des Browsers) und dann ein Absatz stand.
fn heading_with_paragraph() -> Vec<Inline> {
    vec![
        text("Laub"),
        Inline::Soft,
        text("Anna's Laube steht im Herbst voller Kürbisse."),
        Inline::Soft,
        text("Und Äpfel."),
    ]
}

#[test]
fn a_line_break_in_an_edited_heading_ends_the_heading_and_starts_a_paragraph() {
    let doc = parse("== Laub <laub> // Notiz\n\nDann wird geharkt.\n");
    let heading = BlockKind::Heading { level: 2, inlines: heading_with_paragraph(), label: None };
    let out = serialize(&doc, &[replace(0, heading), keep(1)]).unwrap();
    assert_eq!(
        out,
        "== Laub <laub> // Notiz\n\nAnna's Laube steht im Herbst voller Kürbisse.\nUnd Äpfel.\n\nDann wird geharkt.\n",
        "nur „Laub“ ist Überschrift (mit Label und Kommentar der Zeile), der Rest ein Absatz davor"
    );
    let headings = baruch_typst::parse(&out).unwrap().headings;
    assert_eq!(headings.len(), 1, "genau eine echte Überschriftszeile: {out:?}");
    assert_eq!(headings[0].title, "Laub", "der Titel ist kein Absatz: {out:?}");
}

#[test]
fn a_new_heading_with_a_line_break_and_leading_breaks_is_split_too() {
    let doc = parse("= Herbst\n");
    let mut inlines = vec![Inline::Soft, text("  ")];
    inlines.extend(heading_with_paragraph());
    let fresh = Edit::Block { replaces: None, block: BlockKind::Heading { level: 2, inlines, label: None } };
    let out = serialize(&doc, &[keep(0), fresh]).unwrap();
    assert_eq!(
        out, "= Herbst\n\n== Laub\n\nAnna's Laube steht im Herbst voller Kürbisse.\nUnd Äpfel.\n\n",
        "Umbrüche am Anfang zählen nicht; die neue Überschrift endet am ersten Umbruch"
    );
    let only_break = BlockKind::Heading { level: 1, inlines: vec![text("Herbst "), Inline::Soft], label: None };
    let out = serialize(&doc, &[replace(0, only_break)]).unwrap();
    assert_eq!(out, "= Herbst\n", "ein Umbruch am Ende ergibt keinen leeren Absatz");
    let paragraph = BlockKind::Paragraph { inlines: heading_with_paragraph() };
    let out = serialize(&parse("Alt.\n"), &[replace(0, paragraph)]).unwrap();
    assert_eq!(
        out, "Laub\nAnna's Laube steht im Herbst voller Kürbisse.\nUnd Äpfel.\n",
        "in einem Absatz bleiben Umbrüche, wie sie sind"
    );
}

/// Runde 7, Nachbesserung: Ein Umbruch in `*…*` bzw. `_…_` einer Überschrift (Drop, `<br>` des
/// Browsers in `<b>`) wurde in der Überschriftszeile ein Leerzeichen, der Absatz landete wieder im
/// Titel. Jetzt teilt er die Hervorhebung: vorn der Titel, hinten der Absatz, beide hervorgehoben.
#[test]
fn a_line_break_inside_bold_or_italic_ends_the_heading_too() {
    let strong = |c: Vec<Inline>| Inline::Strong { c };
    let emph = |c: Vec<Inline>| Inline::Emph { c };
    let cases = [
        (
            "Soft in *…*",
            vec![strong(vec![text("Laub"), Inline::Soft, text("Ein ganzer Absatz.")])],
            "== *Laub*\n\n*Ein ganzer Absatz.*\n",
        ),
        (
            "Zeilenende im Text in *…*",
            vec![strong(vec![text("Laub\nEin ganzer Absatz.")])],
            "== *Laub*\n\n*Ein ganzer Absatz.*\n",
        ),
        (
            "Soft tief in _*…*_, Leerraum an der Schnittstelle",
            vec![
                text("Im "),
                emph(vec![
                    text("späten "),
                    strong(vec![text("Laub "), Inline::Soft, text(" Absatz")]),
                    text(" weiter"),
                ]),
            ],
            "== Im _späten *Laub*_\n\n_*Absatz* weiter_\n",
        ),
        (
            "Umbruch ganz vorn in *…* zählt nicht",
            vec![strong(vec![Inline::Soft, text(" ")]), text("Laub"), Inline::Soft, text("Absatz")],
            "== Laub\n\nAbsatz\n",
        ),
        (
            "Umbruch am Ende in *…* ergibt keinen leeren Absatz",
            vec![text("Laub "), strong(vec![text("fällt"), Inline::Soft])],
            "== Laub *fällt*\n",
        ),
    ];
    for (case, inlines, expected) in cases {
        let heading = BlockKind::Heading { level: 2, inlines, label: None };
        let out = serialize(&parse("== Alt\n"), &[replace(0, heading)]).unwrap();
        // Die genaue Ausgabe zeigt, wo geteilt wurde und dass nichts verloren geht.
        assert_eq!(out, expected, "{case}");
        // Ursache des Fehlers war eine Titelzeile mit dem Absatz: Typst liest genau einen Titel ohne ihn.
        let headings = baruch_typst::parse(&out).unwrap().headings;
        assert_eq!(headings.len(), 1, "{case}: genau eine Überschriftszeile: {out:?}");
        assert!(!headings[0].title.contains("Absatz"), "{case}: kein Absatz im Titel: {:?}", headings[0].title);
    }
    // Auch bei einer neu eingefügten Überschrift (eigener Weg in `serialize`, nur über `emit`).
    let fresh = Edit::Block {
        replaces: None,
        block: BlockKind::Heading {
            level: 2,
            inlines: vec![strong(vec![text("Laub"), Inline::Soft, text("Ein ganzer Absatz.")])],
            label: None,
        },
    };
    let out = serialize(&parse("= Herbst\n"), &[keep(0), fresh]).unwrap();
    assert_eq!(out, "= Herbst\n\n== *Laub*\n\n*Ein ganzer Absatz.*\n\n", "neue Überschrift wird ebenso geteilt");
    // Fußnoten sind ein eigener Inhaltsblock: ihr Umbruch bleibt in ihr, die Überschrift ganz.
    let footnote = BlockKind::Heading {
        level: 2,
        inlines: vec![text("Laub"), Inline::Footnote { c: vec![text("eins"), Inline::Soft, text("zwei")] }],
        label: None,
    };
    let out = serialize(&parse("== Alt\n"), &[replace(0, footnote)]).unwrap();
    assert_eq!(baruch_typst::parse(&out).unwrap().headings.len(), 1, "Fußnote teilt nicht: {out:?}");
    assert!(out.starts_with("== Laub#footnote[eins\nzwei]"), "Fußnote bleibt ganz in der Überschrift: {out:?}");
}
