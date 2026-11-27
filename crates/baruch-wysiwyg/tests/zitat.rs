// SPDX-License-Identifier: AGPL-3.0-or-later
//! Round 23a (finding of the user, backlog „Zitat-Werkzeug“): a quote block `#quote(block: true, …)[…]` with
//! the attribution in the next line is a quote block in the typeset view and after it a paragraph of its own –
//! as in Typst, where a block ends the paragraph –, not a code island. Untouched, the text goes back
//! byte-identical. All texts (quote, reference, paragraphs) are freely invented.

use baruch_wysiwyg::{BlockKind, Doc, Edit, Inline, parse, render_html, serialize};

fn text(v: &str) -> Inline {
    Inline::Text { v: v.into() }
}

fn keep_all(doc: &Doc) -> Vec<Edit> {
    (0..doc.blocks.len()).map(|keep| Edit::Keep { keep }).collect()
}

/// Invented text: paragraph, quote, attribution directly in the next line, ordinary paragraph.
const MUEHLE: &str = "Der Müller schrieb es an die Tür.\n\n\
#quote(block: true)[Wer mahlt, hört den Bach auch im Schlaf.]\n\
– Mühlenbuch 7,3\n\n\
Dann schloss er das Wehr.\n";

#[test]
fn a_block_quote_with_the_reference_on_the_next_line_is_a_quote_and_a_paragraph_not_an_island() {
    let doc = parse(MUEHLE);
    let kinds: Vec<&BlockKind> = doc.blocks.iter().map(|b| &b.kind).collect();
    assert_eq!(kinds.len(), 4, "Absatz, Zitat, Stellenangabe, Absatz sind vier Blöcke: {kinds:#?}");
    assert!(
        matches!(kinds[1], BlockKind::Quote { block: true, attribution: None, body } if body.len() == 1),
        "das Zitat ist ein Zitatblock mit einem Absatz im Rumpf, keine Insel: {:#?}",
        kinds[1]
    );
    assert_eq!(
        kinds[2],
        &BlockKind::Paragraph { inlines: vec![text("– Mühlenbuch 7,3")] },
        "die Zeile direkt nach `]` ist ein eigener Absatz, wie Typst sie setzt"
    );
    assert!(!kinds.iter().any(|k| **k == BlockKind::Island), "kein Stück des Texts ist eine Code-Insel: {kinds:#?}");
    assert_eq!(
        doc.blocks[1].raw, "#quote(block: true)[Wer mahlt, hört den Bach auch im Schlaf.]\n",
        "das Zitat endet mit seiner Zeile; die Stellenangabe gehört nicht zu seinem Rohtext"
    );
    assert_eq!(
        serialize(&doc, &keep_all(&doc)).unwrap(),
        MUEHLE,
        "unberührt geht der Text byte-gleich zurück (keine neue Leerzeile zwischen Zitat und Angabe)"
    );
    let html = render_html(&doc);
    assert!(html.contains("data-kind=\"quote\""), "das Satzbild zeigt einen Zitatblock: {html}");
    assert!(html.contains("– Mühlenbuch 7,3"), "die Stellenangabe steht als Text im Satzbild: {html}");
    assert!(!html.contains("ws-raw"), "nichts wird als roher Typst-Code gezeigt: {html}");
}

#[test]
fn what_the_ribbon_inserts_is_a_quote_block_also_while_still_empty() {
    // This is what the button „Zitat“ inserts (crates/baruch-server/src/ribbon.rs), here in the middle of invented
    // text.
    let fresh = "Vorher.\n\n#quote(block: true)[]\n– \n\nNachher.\n";
    let doc = parse(fresh);
    assert!(
        matches!(&doc.blocks[1].kind, BlockKind::Quote { block: true, attribution: None, .. }),
        "auch der frisch eingefügte, noch leere Block ist ein Zitatblock: {:#?}",
        doc.blocks
    );
    assert_eq!(serialize(&doc, &keep_all(&doc)).unwrap(), fresh, "byte-gleich zurück, auch die Zeile für die Angabe");

    let with_attribution = "Vorher.\n\n#quote(block: true, attribution: [Gartenregel 3])[Wer sät, wartet.]\nNachher.\n";
    let doc = parse(with_attribution);
    assert_eq!(doc.blocks.len(), 3, "Absatz, Zitat, Absatz: {:#?}", doc.blocks);
    assert!(
        matches!(&doc.blocks[1].kind, BlockKind::Quote { block: true, attribution: Some(a), .. } if a == &vec![text("Gartenregel 3")]),
        "eine Angabe in `attribution` bleibt Teil des Zitatblocks: {:#?}",
        doc.blocks[1].kind
    );
    assert_eq!(serialize(&doc, &keep_all(&doc)).unwrap(), with_attribution, "byte-gleich zurück");
}

#[test]
fn a_line_before_a_block_quote_ends_its_paragraph() {
    let before = "Der Fischer sagte:\n#quote(block: true)[Morgen kommt Wind.]\n";
    let doc = parse(before);
    assert_eq!(doc.blocks.len(), 2, "Absatz und Zitatblock sind getrennt, wie Typst setzt: {:#?}", doc.blocks);
    assert_eq!(
        doc.blocks[0].kind,
        BlockKind::Paragraph { inlines: vec![text("Der Fischer sagte:")] },
        "die Zeile davor ist ein vollständiger Absatz"
    );
    assert!(matches!(doc.blocks[1].kind, BlockKind::Quote { block: true, .. }), "Zitatblock: {:#?}", doc.blocks);
    assert_eq!(serialize(&doc, &keep_all(&doc)).unwrap(), before, "byte-gleich zurück");

    let multi = "#quote(block: true)[\n  Erste Zeile des Lieds.\n\n  Zweite Strophe.\n]\n– Liederheft 12\n";
    let doc = parse(multi);
    assert!(
        matches!(&doc.blocks[0].kind, BlockKind::Quote { block: true, body, .. } if body.len() == 2),
        "ein mehrzeiliger Zitatblock mit zwei Absätzen im Rumpf bleibt ein Block: {:#?}",
        doc.blocks
    );
    assert_eq!(doc.blocks.len(), 2, "die Angabe danach ist ein eigener Absatz: {:#?}", doc.blocks);
    assert_eq!(serialize(&doc, &keep_all(&doc)).unwrap(), multi, "byte-gleich zurück");
}

#[test]
fn inline_quotes_and_block_false_stay_inside_their_paragraph() {
    let inline = "#quote[ein gutes Zeichen]\nnannte er es und ging.\n";
    let doc = parse(inline);
    assert_eq!(doc.blocks.len(), 1, "`#quote[…]` am Zeilenanfang bleibt Teil seines Absatzes: {:#?}", doc.blocks);
    assert!(
        matches!(&doc.blocks[0].kind, BlockKind::Paragraph { inlines } if matches!(inlines[0], Inline::Quote { .. })),
        "als Zitat im Fließtext gelesen: {:#?}",
        doc.blocks[0].kind
    );
    assert_eq!(serialize(&doc, &keep_all(&doc)).unwrap(), inline, "byte-gleich zurück");

    let not_block = "#quote(block: false)[leise]\nweiter im Satz.\n";
    let doc = parse(not_block);
    assert_eq!(doc.blocks.len(), 1, "`block: false` trennt keinen Absatz ab (Typst setzt es im Fließtext)");
    assert_eq!(serialize(&doc, &keep_all(&doc)).unwrap(), not_block, "byte-gleich zurück");

    let trailing = "#quote(block: true)[Kurz.] und gleich weiter.\n";
    assert_eq!(
        parse(trailing).blocks.len(),
        1,
        "steht hinter `]` noch Text in derselben Zeile, ist die Zeile kein alleinstehender Block (unverändert)"
    );
}

#[test]
fn editing_the_reference_paragraph_keeps_the_quote_byte_identical() {
    let doc = parse(MUEHLE);
    let mut edits = keep_all(&doc);
    edits[2] =
        Edit::Block { block: BlockKind::Paragraph { inlines: vec![text("– Mühlenbuch 7,4")] }, replaces: Some(2) };
    let out = serialize(&doc, &edits).unwrap();
    assert!(
        out.contains("#quote(block: true)[Wer mahlt, hört den Bach auch im Schlaf.]\n– Mühlenbuch 7,4\n\n"),
        "nur die Angabe ändert sich; Zitat und der einfache Zeilenumbruch dazwischen bleiben: {out:?}"
    );
    let again = parse(&out);
    assert!(matches!(again.blocks[1].kind, BlockKind::Quote { .. }), "wieder gelesen bleibt das Zitat ein Zitat");
}
