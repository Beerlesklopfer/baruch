// SPDX-License-Identifier: AGPL-3.0-or-later
//! Scenes, work assignments and labels (contract Round 6, "satzbild"): `#szene[…]` and `#arbeit[…]` are
//! blocks of their own with individually editable blocks in the body, a label after the heading belongs
//! to it, and `serialize` changes only the edited block. Plus the pasting of several paragraphs as
//! `wysiwyg.ts` sends it. All texts freely invented.

use baruch_wysiwyg::{Block, BlockKind, Edit, Inline, WysiwygError, parse, serialize};

/// Section with heading, label on the next line, scene (three paragraphs, footnote) directly
/// after it, paragraph and work assignment with a list.
const SECTION: &str = "= Die Werkstatt am Fluss
<werkstatt>
#szene[
  Der Morgen begann mit dem Geräusch von Hobeln.

  Jonas legte das Brett auf die Bank.#footnote[Eiche, sagte er, immer Eiche.]

  Draußen rief jemand   seinen Namen.
]

Ein Absatz dazwischen.

#arbeit[
  Überlege dir:

  - Welche Geräusche gehören zu deinem Morgen?
  - Wer ruft deinen Namen?
]
";

fn text(v: &str) -> Inline {
    Inline::Text { v: v.into() }
}

fn para(v: &str) -> BlockKind {
    BlockKind::Paragraph { inlines: vec![text(v)] }
}

fn keep(keep: usize) -> Edit {
    Edit::Keep { keep }
}

fn replace(index: usize, block: BlockKind) -> Edit {
    Edit::Block { replaces: Some(index), block }
}

fn body_of(block: &Block) -> &[Block] {
    match &block.kind {
        BlockKind::Scene { body, .. } | BlockKind::Work { body, .. } => body,
        other => panic!("keine Szene/Arbeit: {other:?}"),
    }
}

#[test]
fn a_section_splits_into_heading_with_label_scene_and_work() {
    let doc = parse(SECTION);
    assert_eq!(doc.blocks.len(), 4, "Überschrift, Szene, Absatz, Arbeit: {:#?}", doc.blocks);
    assert_eq!(
        doc.blocks[0].kind,
        BlockKind::Heading { level: 1, inlines: vec![text("Die Werkstatt am Fluss")], label: Some("werkstatt".into()) },
        "das Label auf der Zeile hinter der Überschrift gehört zu ihr, nicht zu einem Absatz"
    );
    assert_eq!(doc.blocks[0].raw, "= Die Werkstatt am Fluss\n<werkstatt>\n", "Überschrift samt Labelzeile");
    assert!(
        matches!(doc.blocks[1].kind, BlockKind::Scene { args: None, .. }),
        "die Szene direkt danach ist ein eigener Block"
    );
    let scene = body_of(&doc.blocks[1]);
    assert_eq!(scene.len(), 3, "drei Absätze im Rumpf: {scene:#?}");
    assert!(scene.iter().all(|b| matches!(b.kind, BlockKind::Paragraph { .. })), "alle drei bearbeitbar: {scene:#?}");
    assert!(
        matches!(&scene[1].kind, BlockKind::Paragraph { inlines } if inlines.iter().any(|i| matches!(i, Inline::Footnote { .. }))),
        "die Fußnote steht im zweiten Absatz: {:?}",
        scene[1].kind
    );
    assert_eq!(doc.blocks[2].kind, para("Ein Absatz dazwischen."), "gewöhnlicher Absatz zwischen den Kästen");
    let work = body_of(&doc.blocks[3]);
    assert_eq!(work.len(), 2, "Absatz und Liste im Arbeitsauftrag: {work:#?}");
    assert_eq!(work[0].kind, para("Überlege dir:"), "Absatz im Arbeitsauftrag");
    assert!(
        matches!(&work[1].kind, BlockKind::List { ordered: false, items, .. } if items.len() == 2),
        "die eingerückte Liste im Rumpf ist eine Liste, keine Insel: {:?}",
        work[1].kind
    );
    let joined: String = doc.blocks.iter().map(|b| b.raw.as_str()).collect();
    assert_eq!(joined, SECTION, "die raw ergeben den Text byte-gleich");
    let inner: String = scene.iter().map(|b| b.raw.as_str()).collect();
    assert!(SECTION.contains(&inner), "die raw im Rumpf sind Stücke des Originals (mit Einzug): {inner:?}");
}

#[test]
fn editing_one_paragraph_of_a_scene_changes_only_that_paragraph() {
    let doc = parse(SECTION);
    let edits = [
        keep(0),
        Edit::Body { replaces: 1, body: vec![keep(0), replace(1, para("Jonas legte das Brett hin.")), keep(2)] },
        keep(2),
        keep(3),
    ];
    let out = serialize(&doc, &edits).unwrap();
    let expected = SECTION.replace(
        "Jonas legte das Brett auf die Bank.#footnote[Eiche, sagte er, immer Eiche.]",
        "Jonas legte das Brett hin.",
    );
    assert_eq!(
        out, expected,
        "nur der bearbeitete Absatz ist neu; Einzug, Leerraum der Nachbarn („jemand   seinen“) und Rahmen bleiben"
    );
    let all_keep = [keep(0), Edit::Body { replaces: 1, body: vec![keep(0), keep(1), keep(2)] }, keep(2), keep(3)];
    assert_eq!(serialize(&doc, &all_keep).unwrap(), SECTION, "ein Rumpf aus lauter Keep ist byte-gleich");
}

#[test]
fn editing_the_list_of_a_work_keeps_its_indentation() {
    let doc = parse(SECTION);
    let list = BlockKind::List {
        ordered: false,
        items: vec![vec![text("Was hörst du?")], vec![text("Wer ruft?")]],
        depths: vec![],
    };
    let edits = [keep(0), keep(1), keep(2), Edit::Body { replaces: 3, body: vec![keep(0), replace(1, list)] }];
    let out = serialize(&doc, &edits).unwrap();
    assert!(
        out.ends_with("#arbeit[\n  Überlege dir:\n\n  - Was hörst du?\n  - Wer ruft?\n]\n"),
        "die neue Liste steht mit dem Einzug des Rumpfs da: {out}"
    );
    assert!(out.starts_with(&SECTION[..SECTION.find("#arbeit").unwrap()]), "alles davor unverändert: {out}");
    let reparsed = parse(&out);
    assert!(
        matches!(&body_of(&reparsed.blocks[3])[1].kind, BlockKind::List { items, .. } if items.len() == 2),
        "die neue Liste liest sich wieder als Liste: {:#?}",
        reparsed.blocks[3]
    );
}

#[test]
fn a_new_paragraph_in_a_scene_gets_the_indentation_and_a_blank_line() {
    let doc = parse(SECTION);
    let insert = Edit::Block { replaces: None, block: para("Ein neuer Absatz.") };
    let edits = [keep(0), Edit::Body { replaces: 1, body: vec![keep(0), insert, keep(1), keep(2)] }, keep(2), keep(3)];
    let out = serialize(&doc, &edits).unwrap();
    assert!(
        out.contains("Geräusch von Hobeln.\n\n  Ein neuer Absatz.\n\n  Jonas legte"),
        "neuer Absatz mit Einzug und Leerzeilen zwischen den Nachbarn: {out}"
    );
    let deleted = [keep(0), Edit::Body { replaces: 1, body: vec![keep(0), keep(2)] }, keep(2), keep(3)];
    let out = serialize(&doc, &deleted).unwrap();
    assert!(!out.contains("Jonas"), "der gelöschte Absatz ist weg: {out}");
    assert!(out.contains("Hobeln.\n\n  Draußen"), "zwischen den übrigen bleibt eine Leerzeile: {out}");
}

/// Scene that starts with a list: the indentation of its first line is in the frame before `body_blocks`,
/// that of its second line in the `raw`.
const LIST_FIRST: &str = "#szene[\n  - Hammer\n  - Zange\n\n  Der Rest lag im Regal.\n]\n";

/// List in the body: two items on the same level, otherwise the assertion fails with `what`.
fn assert_flat_list(out: &str, index: usize, what: &str) {
    let reparsed = parse(out);
    let body = body_of(&reparsed.blocks[0]);
    assert!(
        matches!(&body[index].kind, BlockKind::List { items, .. } if items.len() == 2),
        "{what}: die Liste bleibt eine Liste mit zwei Punkten auf einer Ebene, keine Insel: {out}\n{body:#?}"
    );
}

#[test]
fn inserting_before_the_first_block_of_a_body_keeps_its_indentation() {
    let doc = parse(LIST_FIRST);
    let insert = Edit::Block { replaces: None, block: para("Auf der Bank lag:") };
    let out = serialize(&doc, &[Edit::Body { replaces: 0, body: vec![insert, keep(0), keep(1)] }]).unwrap();
    assert_eq!(
        out, "#szene[\n  Auf der Bank lag:\n\n  - Hammer\n  - Zange\n\n  Der Rest lag im Regal.\n]\n",
        "der neue Absatz bekommt den Einzug des Rumpfs, und die Liste behält den Einzug ihrer ersten Zeile"
    );
    assert_flat_list(&out, 1, "nach dem Einfügen davor");
    let unchanged = serialize(&doc, &[Edit::Body { replaces: 0, body: vec![keep(0), keep(1)] }]).unwrap();
    assert_eq!(unchanged, LIST_FIRST, "ohne Änderung byte-gleich, der Einzug steht nicht doppelt da");
}

#[test]
fn moving_the_first_block_of_a_body_keeps_its_indentation() {
    let doc = parse(LIST_FIRST);
    let out = serialize(&doc, &[Edit::Body { replaces: 0, body: vec![keep(1), keep(0)] }]).unwrap();
    assert_eq!(
        out, "#szene[\n  Der Rest lag im Regal.\n\n  - Hammer\n  - Zange\n]\n",
        "beide Blöcke stehen umgestellt mit dem Einzug des Rumpfs da, ohne doppelten Einzug vorn"
    );
    assert_flat_list(&out, 1, "nach dem Umstellen");
    let deleted = serialize(&doc, &[Edit::Body { replaces: 0, body: vec![keep(1)] }]).unwrap();
    assert_eq!(deleted, "#szene[\n  Der Rest lag im Regal.\n]\n", "ohne den ersten Block: der zweite rückt nach vorn");
    let edited = serialize(&doc, &[Edit::Body { replaces: 0, body: vec![keep(1), replace(0, para("Nichts."))] }]);
    assert_eq!(
        edited.unwrap(),
        "#szene[\n  Der Rest lag im Regal.\n\n  Nichts.\n]\n",
        "ein ersetzter erster Block nach hinten bekommt den Einzug wie jeder neue"
    );
}

#[test]
fn a_body_starting_on_the_bracket_line_moves_without_indentation() {
    let source = "#arbeit[Erst lesen.\n\nDann schreiben.]\n";
    let doc = parse(source);
    let out = serialize(&doc, &[Edit::Body { replaces: 0, body: vec![keep(1), keep(0)] }]).unwrap();
    assert_eq!(
        out, "#arbeit[Dann schreiben.\n\nErst lesen.]\n",
        "ohne eigene Zeile gibt es keinen Einzug, der wandern müsste"
    );
}

#[test]
fn scene_arguments_stay_and_only_the_body_is_edited() {
    let source = "#szene(ort: [Hof], zeit: \"Abend\")[\n  Die Kundin stand am Tor.\n\n  Sie wartete.\n]\n";
    let doc = parse(source);
    assert_eq!(doc.blocks.len(), 1, "ein Block: {:#?}", doc.blocks);
    let BlockKind::Scene { args, body } = &doc.blocks[0].kind else { panic!("keine Szene: {:?}", doc.blocks[0]) };
    assert_eq!(args.as_deref(), Some("ort: [Hof], zeit: \"Abend\""), "Argumente vor dem Rumpf wörtlich");
    assert_eq!(body.len(), 2, "zwei Absätze im Rumpf");
    let out =
        serialize(&doc, &[Edit::Body { replaces: 0, body: vec![keep(0), replace(1, para("Sie ging."))] }]).unwrap();
    assert_eq!(
        out,
        source.replace("Sie wartete.", "Sie ging."),
        "Argumente und Rahmen bleiben, nur der Absatz ist neu"
    );
}

#[test]
fn body_edits_need_a_scene_or_work() {
    let doc = parse(SECTION);
    let edits = [Edit::Body { replaces: 2, body: vec![] }];
    assert_eq!(serialize(&doc, &edits), Err(WysiwygError::NotContainer(2)), "ein Absatz hat keinen Rumpf");
    let unknown = [Edit::Body { replaces: 1, body: vec![keep(7)] }];
    assert_eq!(
        serialize(&doc, &unknown),
        Err(WysiwygError::UnknownBlock { index: 7, count: 3 }),
        "Indizes im Rumpf beziehen sich auf die Blöcke des Rumpfs"
    );
}

#[test]
fn an_edited_heading_keeps_its_label_line() {
    let doc = parse(SECTION);
    let heading = BlockKind::Heading { level: 1, inlines: vec![text("Die Werkstatt")], label: None };
    let out = serialize(&doc, &[replace(0, heading), keep(1), keep(2), keep(3)]).unwrap();
    assert!(out.starts_with("= Die Werkstatt\n<werkstatt>\n#szene["), "Labelzeile bleibt unverändert: {out}");
    let same = BlockKind::Heading { level: 2, inlines: vec![text("Werkstatt")], label: Some("werkstatt".into()) };
    let out = serialize(&doc, &[replace(0, same), keep(1), keep(2), keep(3)]).unwrap();
    assert!(out.starts_with("== Werkstatt\n<werkstatt>\n#szene["), "gleiches Label: die Zeile bleibt wörtlich: {out}");
    let renamed = BlockKind::Heading { level: 1, inlines: vec![text("Werkstatt")], label: Some("ort".into()) };
    let out = serialize(&doc, &[replace(0, renamed), keep(1), keep(2), keep(3)]).unwrap();
    assert!(out.starts_with("= Werkstatt\n<ort>\n#szene["), "ein anderes Label ersetzt das alte: {out}");
}

#[test]
fn a_label_line_after_a_heading_is_not_a_paragraph_and_other_labels_stay_where_they_are() {
    let doc = parse("== Der Auftrag <auftrag>\n\nText davor\n<marke>\n\n<frei>\n");
    assert_eq!(
        doc.blocks[0].kind,
        BlockKind::Heading {
            level: 2,
            inlines: vec![text("Der Auftrag "), Inline::Label { v: "auftrag".into() }],
            label: None
        },
        "ein Label am Zeilenende bleibt in der Überschrift"
    );
    assert_eq!(
        doc.blocks[1].kind,
        BlockKind::Paragraph { inlines: vec![text("Text davor"), Inline::Soft, Inline::Label { v: "marke".into() }] },
        "hinter einem Absatz gehört das Label zum Absatz (wie bisher)"
    );
    assert_eq!(doc.blocks.len(), 3, "ein Label nach einer Leerzeile ist ein eigener Block: {:#?}", doc.blocks);
}

#[test]
fn pasting_text_with_blank_lines_makes_several_paragraphs() {
    // wysiwyg.ts: pasting „Eins⏎⏎Zwei⏎⏎Drei“ in the middle of „Anfang Ende“ splits the paragraph: the old
    // block (data-b=0, edited) ends with the first piece, the further pieces are new blocks
    // without an index, the last carries the rest of the old paragraph.
    let original = "Anfang Ende\n\nDanach.\n";
    let doc = parse(original);
    let edits = [
        replace(0, para("Anfang Eins")),
        Edit::Block { replaces: None, block: para("Zwei") },
        Edit::Block { replaces: None, block: para("Drei Ende") },
        keep(1),
    ];
    let out = serialize(&doc, &edits).unwrap();
    assert_eq!(out, "Anfang Eins\n\nZwei\n\nDrei Ende\n\nDanach.\n", "drei Absätze statt einem langen");
    let kinds = parse(&out).blocks.into_iter().filter(|b| matches!(b.kind, BlockKind::Paragraph { .. })).count();
    assert_eq!(kinds, 4, "vier Absatz-Blöcke beim erneuten Lesen");
}

#[test]
fn edits_and_blocks_read_from_the_json_the_widget_sends() {
    let edits: Vec<Edit> = serde_json::from_str(
        r#"[{"keep": 0}, {"replaces": 1, "body": [{"keep": 0}, {"replaces": 1, "block": {"kind": "paragraph", "inlines": [{"t": "text", "v": "Neu."}]}}]}]"#,
    )
    .unwrap();
    assert!(
        matches!(&edits[1], Edit::Body { replaces: 1, body } if body.len() == 2),
        "Rumpf-Anweisung erkannt: {edits:?}"
    );
    let heading: BlockKind =
        serde_json::from_str(r#"{"kind": "heading", "level": 1, "inlines": [{"t": "text", "v": "T"}]}"#).unwrap();
    assert_eq!(
        heading,
        BlockKind::Heading { level: 1, inlines: vec![text("T")], label: None },
        "ohne label (ältere Widgets) gilt keins"
    );
    let json = serde_json::to_string(&parse("= T\n").blocks[0]).unwrap();
    assert!(!json.contains("label"), "ohne Label steht kein Feld im JSON: {json}");
    let scene = serde_json::to_string(&parse("#szene[x]\n").blocks[0]).unwrap();
    assert!(!scene.contains("args"), "ohne Argumente steht kein Feld im JSON: {scene}");
}
