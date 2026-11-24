// SPDX-License-Identifier: AGPL-3.0-or-later
//! Querverweise im Satzbild (Runde 30a): `#link(<kN>)[…]` samt Seitenzahl-Zusatz ist `Inline::Xref`, `emit`/`parse`
//! ergeben eine Rundreise (auch als Eigenschaftstest mit fester Saat), andere Labels bleiben Inseln. Texte frei
//! erfunden.

use baruch_typst::xref::xref_snippet;
use baruch_wysiwyg::{BlockKind, Inline, emit, parse, render_html};

fn text(v: &str) -> Inline {
    Inline::Text { v: v.into() }
}

/// Inlines des einzigen Absatzes von `typst`.
fn inlines(typst: &str) -> Vec<Inline> {
    let doc = parse(typst);
    assert_eq!(doc.blocks.len(), 1, "ein Block aus {typst:?}");
    match &doc.blocks[0].kind {
        BlockKind::Paragraph { inlines } => inlines.clone(),
        other => panic!("kein Absatz aus {typst:?}: {other:?}"),
    }
}

#[test]
fn both_snippet_forms_read_as_an_xref() {
    let plain = format!("Siehe {}.", xref_snippet(103, "die Einleitung", false));
    assert_eq!(
        inlines(&plain),
        vec![text("Siehe "), Inline::Xref { key: 103, c: vec![text("die Einleitung")], page: false }, text(".")],
        "Form ohne Seitenzahl"
    );
    let paged = format!("Siehe {}.", xref_snippet(7, "das _zweite_ Kapitel", true));
    assert_eq!(
        inlines(&paged),
        vec![
            text("Siehe "),
            Inline::Xref {
                key: 7,
                c: vec![text("das "), Inline::Emph { c: vec![text("zweite")] }, text(" Kapitel")],
                page: true
            },
            text(".")
        ],
        "Form mit Seitenzahl-Zusatz: der Zusatz gehört nicht zum Linktext"
    );
}

#[test]
fn other_labels_and_odd_forms_stay_islands() {
    for typst in [
        "Siehe #link(<hafen>)[den Hafen].",
        "Siehe #link(<k01>)[falsch].",
        "Siehe #link(<k5>).",
        "Siehe #link(<k5>)[].",
        "Siehe #link(<k5>)[Text, S.~#context counter(page).at(<k6>).first()].",
    ] {
        let found = format!("{:?}", parse(typst).blocks);
        assert!(
            !found.contains("Xref"),
            "{typst:?}: kein Querverweis von Baruch (fremdes Label, kein Linktext oder fremder Zusatz): {found:?}"
        );
    }
}

#[test]
fn emit_writes_the_snippet_form_and_html_marks_the_block() {
    let xref = Inline::Xref { key: 12, c: vec![text("den Wachturm")], page: true };
    let kind = BlockKind::Paragraph { inlines: vec![text("Vgl. "), xref.clone()] };
    assert_eq!(emit(&kind), format!("Vgl. {}", xref_snippet(12, "den Wachturm", true)), "Form aus §3.3");
    let html = render_html(&parse(&emit(&kind)));
    assert!(html.contains("class=\"ws-xref\" data-key=\"12\" data-page=\"1\""), "Block mit Schlüssel: {html}");
    assert!(html.contains("ws-xref-page"), "Seitenzahl-Zusatz sichtbar, nicht bearbeitbar: {html}");
    let html = render_html(&parse("#link(<k3>)[Brücke & Turm]"));
    assert!(html.contains("Brücke &amp; "), "Linktext maskiert: {html}");
}

/// Zeichen für zufällige Linktexte, mit Typst-Sonderzeichen.
const ALPHABET: &[char] = &[
    'a', 'n', 'ä', 'ß', 'W', '1', ' ', ' ', '.', ',', ':', '-', '#', '*', '_', '$', '@', '<', '>', '[', ']', '~', '\\',
];

#[test]
fn random_xrefs_read_back_unchanged() {
    let seed = std::env::var("WS_SEED").ok().and_then(|s| s.parse().ok()).unwrap_or(0x30a_c0de);
    let cases = std::env::var("WS_CASES").ok().and_then(|s| s.parse().ok()).unwrap_or(1000);
    let mut rng = fastrand::Rng::with_seed(seed);
    let mut checked = 0;
    for case in 0..cases {
        let len = rng.usize(1..10);
        let mut v: String = (0..len).map(|_| ALPHABET[rng.usize(..ALPHABET.len())]).collect();
        v = v.trim().to_owned();
        if v.is_empty() {
            continue;
        }
        let mut c = vec![text(&v)];
        if rng.bool() {
            c.push(Inline::Strong { c: vec![text("fett")] });
        }
        let xref = Inline::Xref { key: rng.i64(1..100_000), c, page: rng.bool() };
        let kind = BlockKind::Paragraph { inlines: vec![text("Vor "), xref, text(" nach")] };
        let typst = emit(&kind);
        let doc = parse(&typst);
        assert_eq!(doc.blocks.len(), 1, "Fall {case}: ein Block aus {typst:?}");
        assert_eq!(doc.blocks[0].kind, kind, "Fall {case}: Rundreise über {typst:?}");
        checked += 1;
    }
    assert!(checked > cases / 2, "genug Fälle geprüft: {checked}");
}
