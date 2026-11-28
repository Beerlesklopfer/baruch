// SPDX-License-Identifier: AGPL-3.0-or-later
//! Cross-references in the typeset view (Round 30a): `#link(<kN>)[…]` including page number addition is
//! `Inline::Xref`, `emit`/`parse` yield a round trip (also as a property test with a fixed seed), other labels
//! stay islands. Texts freely invented.

use baruch_typst::xref::{xref_snippet, xref_snippet_in};
use baruch_wysiwyg::{
    BlockKind, Edit, Inline, WorkLanguage, diff_html_in, emit, emit_in, parse, render_html, render_html_in, serialize,
    serialize_in,
};

fn text(v: &str) -> Inline {
    Inline::Text { v: v.into() }
}

/// Inlines of the only paragraph of `typst`.
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

/// Characters for random link texts, with Typst special characters.
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

// ---- Work language (Round 39b) --------------------------------------------------------------------------------------

/// English page number addition as `xref.ts` writes it in the source editor of an English work.
const ENGLISH_SUFFIX: &str = ", p.~#context counter(page).at(<k12>).first()";

#[test]
fn emit_in_an_english_work_writes_the_english_page_abbreviation() {
    let xref = Inline::Xref { key: 12, c: vec![text("the watchtower")], page: true };
    let kind = BlockKind::Paragraph { inlines: vec![text("See "), xref] };
    let english = emit_in(&kind, WorkLanguage::En);
    // The book PDF of an English work printed "S. 12" before Round 39b; the addition must be the English one.
    assert_eq!(english, format!("See #link(<k12>)[the watchtower{ENGLISH_SUFFIX}]"), "English form: {english}");
    assert_eq!(
        english,
        format!("See {}", xref_snippet_in(12, "the watchtower", true, WorkLanguage::En)),
        "the same form as the snippet of baruch_typst (one source for book and editor)"
    );
    // German works must not change: emit stays the German form, emit_in with De is identical.
    assert_eq!(emit_in(&kind, WorkLanguage::De), emit(&kind), "emit is emit_in for a German work");
    assert!(emit(&kind).contains(", S.~#context"), "German works keep S.: {}", emit(&kind));
    // Nested places (footnote, list item, quote body, heading) carry the language as well.
    let nested = BlockKind::List {
        ordered: false,
        items: vec![vec![Inline::Footnote {
            c: vec![Inline::Xref { key: 12, c: vec![text("the watchtower")], page: true }],
        }]],
        depths: vec![],
    };
    let typst = emit_in(&nested, WorkLanguage::En);
    assert!(typst.contains(ENGLISH_SUFFIX) && !typst.contains("S.~"), "nested xref in English: {typst}");
}

#[test]
fn the_english_page_addition_reads_back_as_a_page_addition() {
    let typst = format!("See #link(<k12>)[the watchtower{ENGLISH_SUFFIX}].");
    assert_eq!(
        inlines(&typst),
        vec![text("See "), Inline::Xref { key: 12, c: vec![text("the watchtower")], page: true }, text(".")],
        "the English addition from xref.ts is a page addition, not part of the link text"
    );
    // German additions keep being read (German works, and a German text pasted into an English work).
    let german = format!("Siehe {}.", xref_snippet(12, "den Wachturm", true));
    assert!(matches!(&inlines(&german)[1], Inline::Xref { page: true, .. }), "German addition: {german}");
    // An addition naming another key stays foreign in every language: no cross reference of Baruch (island).
    let foreign = "See #link(<k12>)[the watchtower, p.~#context counter(page).at(<k13>).first()].";
    let found = format!("{:?}", parse(foreign).blocks);
    assert!(!found.contains("Xref"), "an English addition with another key is not Baruch's page addition: {found}");
}

#[test]
fn serialize_in_an_english_work_keeps_the_english_form_through_an_edit() {
    // The typeset view (default editor) sends the edited paragraph as blocks; the server writes it with serialize_in.
    let body = format!("= The Tower\n\nSee #link(<k12>)[the watchtower{ENGLISH_SUFFIX}].\n");
    let doc = parse(&body);
    let BlockKind::Paragraph { inlines } = &doc.blocks[1].kind else { panic!("paragraph: {:?}", doc.blocks) };
    let mut edited = inlines.clone();
    edited.push(text(" Now edited."));
    let edits = vec![
        Edit::Keep { keep: 0 },
        Edit::Block { replaces: Some(1), block: BlockKind::Paragraph { inlines: edited } },
    ];
    let english = serialize_in(&doc, &edits, WorkLanguage::En).unwrap();
    assert_eq!(
        english,
        format!("= The Tower\n\nSee #link(<k12>)[the watchtower{ENGLISH_SUFFIX}]. Now edited.\n"),
        "the edited block keeps p. in an English work"
    );
    assert_eq!(
        serialize(&doc, &edits).unwrap(),
        serialize_in(&doc, &edits, WorkLanguage::De).unwrap(),
        "serialize is serialize_in for a German work"
    );
}

#[test]
fn the_typeset_view_shows_the_page_abbreviation_of_the_work_language() {
    let doc = parse(&format!("See #link(<k12>)[the watchtower{ENGLISH_SUFFIX}]."));
    let english = render_html_in(&doc, &|_| None, WorkLanguage::En);
    assert!(english.contains(">, p.\u{a0}…</span>"), "English addition in the typeset view: {english}");
    assert!(!english.contains("S.\u{a0}"), "no German abbreviation in an English work: {english}");
    let german = render_html(&doc);
    assert!(german.contains(">, S.\u{a0}…</span>"), "German works keep S.: {german}");
    assert_eq!(render_html_in(&doc, &|_| None, WorkLanguage::De), german, "render_html is the German form");
    let diff = diff_html_in("", &format!("See #link(<k12>)[x{ENGLISH_SUFFIX}]."), &|_| None, WorkLanguage::En);
    assert!(diff.contains(">, p.\u{a0}…</span>"), "the difference view follows the work language too: {diff}");
}

#[test]
fn random_english_xrefs_read_back_unchanged() {
    let mut rng = fastrand::Rng::with_seed(0x39b_c0de);
    for case in 0..500 {
        let v: String = (0..rng.usize(1..8)).map(|_| ALPHABET[rng.usize(..ALPHABET.len())]).collect();
        let v = v.trim().to_owned();
        if v.is_empty() {
            continue;
        }
        let xref = Inline::Xref { key: rng.i64(1..100_000), c: vec![text(&v)], page: rng.bool() };
        let kind = BlockKind::Paragraph { inlines: vec![text("Before "), xref, text(" after")] };
        let typst = emit_in(&kind, WorkLanguage::En);
        let doc = parse(&typst);
        assert_eq!(doc.blocks.len(), 1, "case {case}: one block from {typst:?}");
        assert_eq!(doc.blocks[0].kind, kind, "case {case}: round trip through {typst:?}");
    }
}
