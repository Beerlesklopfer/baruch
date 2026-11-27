// SPDX-License-Identifier: AGPL-3.0-or-later
//! Round 14: part and "without number" in the typeset view. The marker lines from `baruch_typst::marker` are
//! blocks of their own ([`BlockKind::Part`], [`BlockKind::Unnumbered`]); untouched they stay byte-identical,
//! changed, `serialize` builds them anew (comment stays). All texts freely invented.

use baruch_wysiwyg::{BlockKind, Edit, WysiwygError, parse, render_html, serialize};

/// Part with comment, chapter, section without number and paragraphs.
const TEXT: &str = "#heading(level: 1, numbering: none)[Erster Teil \\# Wasser] <baruch-part> // Notiz
Ein Satz direkt unter dem Teil.

= Die Quelle
Am Hang sprudelt es.

#heading(level: 2, numbering: none)[Am Rand] <baruch-unnumbered>

Letzter Absatz.
";

fn keep(keep: usize) -> Edit {
    Edit::Keep { keep }
}

fn keep_all(count: usize) -> Vec<Edit> {
    (0..count).map(keep).collect()
}

#[test]
fn markers_become_their_own_blocks() {
    let doc = parse(TEXT);
    let kinds: Vec<&BlockKind> = doc.blocks.iter().map(|b| &b.kind).collect();
    assert_eq!(
        kinds[0],
        &BlockKind::Part { title: "Erster Teil # Wasser".into() },
        "Die Teil-Markierung ist ein Teil-Block mit aufgelöstem Titel (ohne Kommentar)"
    );
    assert!(
        matches!(kinds[1], BlockKind::Paragraph { .. }),
        "Die Zeile direkt darunter ist ein eigener Absatz, wie unter einer Überschrift: {kinds:?}"
    );
    assert_eq!(kinds[4], &BlockKind::Unnumbered { level: 2, title: "Am Rand".into() }, "„ohne Nummer“ mit Ebene");
    assert_eq!(doc.blocks.len(), 6, "Teil, Absatz, Überschrift, Absatz, ohne Nummer, Absatz: {kinds:?}");
    let joined: String = doc.blocks.iter().map(|b| b.raw.as_str()).collect();
    assert_eq!(joined, TEXT, "Die Verkettung aller raw ist der Text, Byte für Byte");
}

#[test]
fn keeping_every_block_gives_back_the_text() {
    let doc = parse(TEXT);
    assert_eq!(serialize(&doc, &keep_all(doc.blocks.len())).unwrap(), TEXT, "Keep übernimmt Markierungen wörtlich");
}

#[test]
fn renaming_a_part_builds_the_part_line_and_keeps_the_comment() {
    let doc = parse(TEXT);
    let mut edits = keep_all(doc.blocks.len());
    edits[0] = Edit::Block { replaces: Some(0), block: BlockKind::Part { title: "- Neuer [Teil]".into() } };
    let out = serialize(&doc, &edits).unwrap();
    assert!(
        out.starts_with("#heading(level: 1, numbering: none)[\\- Neuer \\[Teil\\]] <baruch-part> // Notiz\nEin Satz"),
        "Neue Teil-Zeile über part_line (führendes - und Klammern maskiert), Kommentar und Rest bleiben: {out:?}"
    );
    let reread = baruch_typst::parse(&out).unwrap();
    assert_eq!(
        (reread.headings[0].level, reread.headings[0].title.as_str()),
        (0, "- Neuer [Teil]"),
        "baruch_typst liest wieder einen Teil mit demselben Titel"
    );
    assert_eq!(
        &out[out.find("Ein Satz").unwrap()..],
        &TEXT[TEXT.find("Ein Satz").unwrap()..],
        "nur die eine Zeile ändert sich"
    );
}

#[test]
fn renaming_an_unnumbered_point_keeps_the_marker() {
    let doc = parse(TEXT);
    let mut edits = keep_all(doc.blocks.len());
    edits[4] =
        Edit::Block { replaces: Some(4), block: BlockKind::Unnumbered { level: 2, title: "Ganz am Rand".into() } };
    let out = serialize(&doc, &edits).unwrap();
    assert!(
        out.contains("\n#heading(level: 2, numbering: none)[Ganz am Rand] <baruch-unnumbered>\n\nLetzter Absatz.\n"),
        "„ohne Nummer“ bleibt Markierung, Leerzeile dahinter bleibt: {out:?}"
    );
}

#[test]
fn a_new_part_block_is_refused_in_the_satzbild() {
    // Round 18b (backlog „Nach Runde 14 offen“: a client could write a part block into a chapter
    // text): parts arise in the outline (`outline.add` with `level: 0`); the typeset view does not offer
    // them, and `serialize` rejects a new part block instead of silently writing a part marker.
    let doc = parse("Absatz.\n");
    let edits = vec![Edit::Block { replaces: None, block: BlockKind::Part { title: "Neu".into() } }, keep(0)];
    assert_eq!(serialize(&doc, &edits), Err(WysiwygError::NewPart), "neuer Teil im Satzbild → Fehler, nichts erzeugt");
    assert_eq!(
        WysiwygError::NewPart.to_string(),
        "Einen Teil legt man in der Gliederung an, nicht im Satzbild.",
        "deutsche Meldung mit dem Weg, der geht"
    );
}

#[test]
fn a_heading_turned_into_a_part_is_refused() {
    // Converting another block into a part would also write a part marker into the text; the
    // outline rejected it as the first heading anyway (TO_PART), as a further one it silently created a part.
    let doc = parse("= Alt // bleibt\nText.\n");
    let edits = vec![Edit::Block { replaces: Some(0), block: BlockKind::Part { title: "Teil".into() } }, keep(1)];
    assert_eq!(serialize(&doc, &edits), Err(WysiwygError::NewPart), "Überschrift → Teil im Satzbild abgelehnt");
    let doc = parse("#heading(level: 1, numbering: none)[Alt] <baruch-part> // bleibt\n");
    let edits = vec![Edit::Block { replaces: Some(0), block: BlockKind::Part { title: "Neu".into() } }];
    assert_eq!(
        serialize(&doc, &edits).unwrap(),
        "#heading(level: 1, numbering: none)[Neu] <baruch-part> // bleibt\n",
        "einen vorhandenen Teil umbenennen geht weiter (Kommentar bleibt)"
    );
}

#[test]
fn deviating_markers_and_markers_in_a_scene_are_not_parts() {
    for text in [
        "#heading(level: 1, numbering: none)[T]  <baruch-part>\n",
        " #heading(level: 1, numbering: none)[T] <baruch-part>\n",
        "#heading(level: 2, numbering: none)[T] <baruch-part>\n",
        "#heading(level: 1, numbering: none)[T] <anderes>\n",
    ] {
        let doc = parse(text);
        assert!(
            doc.blocks.iter().all(|b| !matches!(b.kind, BlockKind::Part { .. } | BlockKind::Unnumbered { .. })),
            "Nur die kanonische Form ist eine Markierung (wie baruch_typst::parse): {text:?}"
        );
        let joined: String = doc.blocks.iter().map(|b| b.raw.as_str()).collect();
        assert_eq!(joined, text, "und der Text bleibt byte-gleich");
    }
    let scene = "#szene[\n#heading(level: 1, numbering: none)[T] <baruch-part>\n]\n";
    let doc = parse(scene);
    let BlockKind::Scene { body, .. } = &doc.blocks[0].kind else { panic!("Szene erwartet: {:?}", doc.blocks) };
    assert!(
        body.iter().all(|b| !matches!(b.kind, BlockKind::Part { .. })),
        "Im Rumpf einer Szene ist die Zeile auch für baruch_typst kein Teil: {body:?}"
    );
}

#[test]
fn render_html_shows_the_part_as_its_own_block() {
    let html = render_html(&parse(TEXT));
    assert!(
        html.starts_with(
            "<div class=\"ws-block ws-part\" data-b=\"0\" data-kind=\"part\" data-level=\"0\" role=\"heading\" aria-level=\"1\""
        ),
        "Teil als eigener Block mit Rolle Überschrift: {html}"
    );
    assert!(html.contains(">Erster Teil # Wasser</div>"), "Titel als Klartext, ohne Kommentar und Markierung: {html}");
    assert!(
        html.contains("data-kind=\"unnumbered\" data-level=\"2\" role=\"heading\" aria-level=\"2\""),
        "„ohne Nummer“ mit Ebene: {html}"
    );
    let hostile = render_html(&parse("#heading(level: 1, numbering: none)[\\<b\\> \\& x] <baruch-part>\n"));
    assert!(hostile.contains("&lt;b&gt;"), "Der Titel ist maskiert: {hostile}");
    assert!(!hostile.contains("<b>"), "kein HTML aus dem Titel: {hostile}");
}

#[test]
fn the_json_form_of_the_new_kinds() {
    let part = serde_json::to_value(BlockKind::Part { title: "T".into() }).unwrap();
    assert_eq!(part, serde_json::json!({"kind": "part", "title": "T"}), "so schickt wysiwyg.ts einen Teil");
    let off: BlockKind =
        serde_json::from_value(serde_json::json!({"kind": "unnumbered", "level": 3, "title": "U"})).unwrap();
    assert_eq!(off, BlockKind::Unnumbered { level: 3, title: "U".into() }, "und „ohne Nummer“");
}
