// SPDX-License-Identifier: AGPL-3.0-or-later
//! `emit` generates canonical Typst that `parse` reads back exactly the same: `parse(emit(k)) == k` for every
//! kind except the island (compared without the `raw` of nested blocks). Plus fixed examples and a
//! property test with a fixed seed over randomly generated, canonical blocks.

use baruch_wysiwyg::{Block, BlockKind, Inline, Term, emit, parse};

/// Empty the `raw` of nested blocks: `emit` does not write them, `parse` fills them.
fn without_raw(kind: &BlockKind) -> BlockKind {
    let strip = |body: &[Block]| -> Vec<Block> {
        body.iter().map(|b| Block { kind: without_raw(&b.kind), raw: String::new() }).collect()
    };
    match kind {
        BlockKind::Quote { block, attribution, body } => {
            BlockKind::Quote { block: *block, attribution: attribution.clone(), body: strip(body) }
        }
        BlockKind::Scene { args, body } => BlockKind::Scene { args: args.clone(), body: strip(body) },
        BlockKind::Work { args, body } => BlockKind::Work { args: args.clone(), body: strip(body) },
        other => other.clone(),
    }
}

/// `emit`, then `parse`: exactly one block whose kind is `kind` and whose `raw` is the generated text.
fn assert_round_trip(kind: &BlockKind, case: &str) {
    let typst = emit(kind);
    let doc = parse(&typst);
    assert_eq!(doc.blocks.len(), 1, "{case}: genau ein Block aus {typst:?}, gelesen {:#?}", doc.blocks);
    assert_eq!(doc.blocks[0].raw, typst, "{case}: raw ist der ganze Text");
    assert_eq!(without_raw(&doc.blocks[0].kind), without_raw(kind), "{case}: dieselbe Art aus {typst:?}");
}

fn text(v: &str) -> Inline {
    Inline::Text { v: v.into() }
}

fn para(inlines: Vec<Inline>) -> BlockKind {
    BlockKind::Paragraph { inlines }
}

#[test]
fn every_kind_reads_back_from_its_emitted_typst() {
    let cases = [
        BlockKind::Heading {
            level: 2,
            inlines: vec![text("Am "), Inline::Emph { c: vec![text("Brunnen")] }],
            label: None,
        },
        BlockKind::Heading {
            level: 1,
            inlines: vec![text("Mit Label "), Inline::Label { v: "kap-1".into() }],
            label: None,
        },
        BlockKind::Heading { level: 3, inlines: vec![text("Label darunter")], label: Some("kap-2".into()) },
        para(vec![
            text("Ein "),
            Inline::Strong { c: vec![text("fester")] },
            text(" Satz"),
            Inline::Footnote { c: vec![text("Anmerkung")] },
            text("."),
            Inline::Soft,
            text("- kein Listenpunkt, sondern Text"),
        ]),
        para(vec![Inline::Quote { c: vec![text("Wer schöpft?")] }]),
        BlockKind::List {
            ordered: false,
            items: vec![vec![text("eins")], vec![text("zwei"), Inline::Soft, text("weiter")]],
            depths: vec![],
        },
        BlockKind::List { ordered: true, items: vec![vec![text("erstens")], vec![text("zweitens")]], depths: vec![] },
        // Round 18b: nested list, enumeration (also nested, from 3), term list.
        BlockKind::List {
            ordered: false,
            items: vec![vec![text("außen")], vec![text("innen"), Inline::Soft, text("weiter")], vec![text("wieder")]],
            depths: vec![0, 1, 0],
        },
        BlockKind::Enum {
            start: 1,
            items: vec![vec![text("Wasser holen")], vec![text("Feuer machen")]],
            depths: vec![],
        },
        BlockKind::Enum {
            start: 3,
            items: vec![vec![text("drei")], vec![text("eins innen")], vec![text("zwei innen")], vec![text("vier")]],
            depths: vec![0, 1, 1, 0],
        },
        BlockKind::Terms {
            items: vec![
                Term { term: vec![text("Krug")], desc: vec![text("Gefäß aus Ton")] },
                Term { term: vec![text("Uhr: Rathaus")], desc: vec![] },
            ],
        },
        // Round 18b: forced break, link with and without text, reference with addition and before a word character.
        para(vec![
            text("Ein Umbruch"),
            Inline::Linebreak,
            text("mitten"),
            Inline::Linebreak,
            Inline::Soft,
            text("im Satz"),
        ]),
        para(vec![
            text("Siehe "),
            Inline::Link { url: "https://example.org/a\"b".into(), c: Some(vec![text("Beispiel")]) },
            text(" und "),
            Inline::Link { url: "mailto:mira@example.org".into(), c: None },
            text("."),
        ]),
        para(vec![
            text("Vgl. "),
            Inline::Ref { target: "brunnen".into(), supplement: Some("S. 3".into()) },
            text(", "),
            Inline::Ref { target: "abb".into(), supplement: None },
            text("x und "),
            Inline::Ref { target: "kap.".into(), supplement: None },
        ]),
        BlockKind::Quote {
            block: true,
            attribution: Some(vec![text("Inschrift")]),
            body: vec![Block { kind: para(vec![text("Wer hier schöpft.")]), raw: String::new() }],
        },
        BlockKind::Quote { block: false, attribution: None, body: vec![] },
        BlockKind::Scene {
            args: None,
            body: vec![
                Block { kind: para(vec![text("Mira setzt sich.")]), raw: String::new() },
                Block { kind: BlockKind::Island, raw: "#let zahl = 3".into() },
                Block { kind: para(vec![text("Ein Hund läuft vorbei.")]), raw: String::new() },
            ],
        },
        BlockKind::Work {
            args: None,
            body: vec![Block { kind: para(vec![text("Wann zuletzt?")]), raw: String::new() }],
        },
        BlockKind::Scene {
            args: Some("ort: [Küche], zeit: \"Abend\"".into()),
            body: vec![
                Block { kind: para(vec![text("Erster Absatz.")]), raw: String::new() },
                Block { kind: para(vec![text("Zweiter Absatz.")]), raw: String::new() },
            ],
        },
    ];
    for (k, kind) in cases.iter().enumerate() {
        assert_round_trip(kind, &format!("Beispiel {k}"));
    }
}

#[test]
fn an_island_is_never_emitted() {
    assert_eq!(emit(&BlockKind::Island), "", "eine Insel hat keinen eigenen Typst-Text; serialize lehnt sie ab");
}

// ---- Property test -----------------------------------------------------------------------------

/// Characters for random texts: letters, umlauts, digits, punctuation and all Typst special characters.
const ALPHABET: &[char] = &[
    'a', 'b', 'e', 'n', 'r', 'x', 'ä', 'ß', 'Ö', '1', '2', '0', ' ', ' ', ' ', '.', ',', ':', ';', '!', '?', '-', '–',
    '=', '+', '/', '\\', '#', '*', '_', '$', '@', '<', '>', '[', ']', '`', '~', '(', ')', '\'', '"', '\u{a0}', '{',
    '}', '%', '&',
];

/// Islands that safely stay verbatim as an inline (none that ends with an identifier).
const INLINE_ISLANDS: &[&str] = &["$x^2$", "`roh`", "#box[Kasten]", "$a < b$"];
/// Islands as a whole block in a body.
const BLOCK_ISLANDS: &[&str] = &["#pagebreak()", "#let zahl = 3", "$ a + b $"];
const LABELS: &[&str] = &["ziel", "abb-1", "kap:2", "a.b"];
/// Targets of references (Round 18b), also those that only work as `#ref(<…>)`.
const TARGETS: &[&str] = &["ziel", "abb-1", "kap:2", "a.b", "ende.", "_unter", "-strich"];
/// Additions of references, verbatim.
const SUPPLEMENTS: &[&str] = &["S. 3", "Abb.", "_kurz_"];
/// Addresses of links with all characters that must be escaped in the string.
const URLS: &[&str] = &["https://example.org", "https://example.org/a\"b\\c", "mailto:x@y.de", "datei (1).pdf"];

struct Generator {
    rng: fastrand::Rng,
}

impl Generator {
    fn text(&mut self) -> String {
        let len = self.rng.usize(1..9);
        (0..len).map(|_| ALPHABET[self.rng.usize(..ALPHABET.len())]).collect()
    }

    /// Random sequence; `soft`: line breaks allowed, `depth`: nesting.
    fn inlines(&mut self, soft: bool, depth: usize) -> Vec<Inline> {
        let count = self.rng.usize(1..6);
        (0..count)
            .map(|_| match self.rng.usize(0..20) {
                0..=8 => Inline::Text { v: self.text() },
                9 | 10 if soft => Inline::Soft,
                11 if depth < 3 => Inline::Emph { c: self.inlines(soft, depth + 1) },
                12 if depth < 3 => Inline::Strong { c: self.inlines(soft, depth + 1) },
                13 if depth < 3 => Inline::Footnote { c: self.inlines(true, depth + 1) },
                14 if depth < 3 => Inline::Quote { c: self.inlines(true, depth + 1) },
                15 => Inline::Label { v: LABELS[self.rng.usize(..LABELS.len())].into() },
                16 => Inline::Island { raw: INLINE_ISLANDS[self.rng.usize(..INLINE_ISLANDS.len())].into() },
                // Round 18b: break, link, reference.
                17 => Inline::Linebreak,
                18 if depth < 3 => Inline::Link {
                    url: URLS[self.rng.usize(..URLS.len())].into(),
                    c: self.rng.bool().then(|| self.inlines(soft, depth + 1)),
                },
                19 => Inline::Ref {
                    target: TARGETS[self.rng.usize(..TARGETS.len())].into(),
                    supplement: self.rng.bool().then(|| SUPPLEMENTS[self.rng.usize(..SUPPLEMENTS.len())].into()),
                },
                _ => Inline::Text { v: self.text() },
            })
            .collect()
    }

    fn block(&mut self, depth: usize) -> BlockKind {
        loop {
            let kind = match self.rng.usize(0..10) {
                0 | 1 => {
                    let inlines = canonical_heading(self.inlines(false, 0));
                    let label = (self.rng.usize(0..4) == 0).then(|| LABELS[self.rng.usize(..LABELS.len())].into());
                    BlockKind::Heading { level: self.rng.u8(1..=6), inlines, label }
                }
                2 => {
                    let (items, depths) = self.items();
                    BlockKind::List { ordered: self.rng.bool(), items, depths }
                }
                3 => {
                    let (items, depths) = self.items();
                    BlockKind::Enum { start: self.rng.u64(0..30), items, depths }
                }
                7 => {
                    let count = self.rng.usize(1..4);
                    let items = (0..count)
                        .map(|_| Term {
                            term: canonical(self.inlines(false, 1), true, true),
                            desc: canonical(self.inlines(true, 0), true, true),
                        })
                        .collect();
                    BlockKind::Terms { items }
                }
                4 if depth < 2 => {
                    let attribution = self.rng.bool().then(|| canonical(self.inlines(true, 1), false, false));
                    BlockKind::Quote { block: self.rng.bool(), attribution, body: self.body(depth + 1) }
                }
                5 if depth < 2 => BlockKind::Scene { args: self.args(), body: self.body(depth + 1) },
                6 if depth < 2 => BlockKind::Work { args: self.args(), body: self.body(depth + 1) },
                _ => BlockKind::Paragraph { inlines: canonical(self.inlines(true, 0), true, true) },
            };
            if valid(&kind) {
                return kind;
            }
        }
    }

    /// List items with valid depths (Round 18b): the first at the top, each at most one level deeper than
    /// the one before; without nesting no depths (that is how `parse` reads it).
    fn items(&mut self) -> (Vec<Vec<Inline>>, Vec<u8>) {
        let count = self.rng.usize(1..5);
        let items = (0..count).map(|_| canonical(self.inlines(true, 0), true, true)).collect();
        let mut depths: Vec<u8> = Vec::with_capacity(count);
        for k in 0..count {
            let limit = if k == 0 { 0 } else { depths[k - 1] + 1 };
            depths.push(self.rng.u8(0..=limit.min(3)));
        }
        if depths.iter().all(|&d| d == 0) {
            depths.clear();
        }
        (items, depths)
    }

    /// Arguments before the body of a scene/work (verbatim), sometimes none.
    fn args(&mut self) -> Option<String> {
        const ARGS: &[&str] = &["", "ort: [Küche]", "titel: \"Am (Abend)\", nummer: 2", "farbe: luma(240)"];
        (self.rng.usize(0..3) == 0).then(|| ARGS[self.rng.usize(..ARGS.len())].into())
    }

    fn body(&mut self, depth: usize) -> Vec<Block> {
        let count = self.rng.usize(0..4);
        (0..count)
            .map(|_| {
                if self.rng.usize(0..5) == 0 {
                    let raw = BLOCK_ISLANDS[self.rng.usize(..BLOCK_ISLANDS.len())];
                    Block { kind: BlockKind::Island, raw: raw.into() }
                } else {
                    Block { kind: self.block(depth), raw: String::new() }
                }
            })
            .collect()
    }
}

/// Canonical form as `parse` delivers it: no empty or neighbouring texts, no break at the
/// edge or doubled, no whitespace at the beginning of a line (`line_start` or after a break) and none at the
/// end of the block (`block_end`), no empty markup.
fn canonical(inlines: Vec<Inline>, line_start: bool, block_end: bool) -> Vec<Inline> {
    let mut out: Vec<Inline> = Vec::new();
    for inline in inlines {
        let inline = match inline {
            Inline::Emph { c } => Inline::Emph { c: canonical(c, false, false) },
            Inline::Strong { c } => Inline::Strong { c: canonical(c, false, false) },
            Inline::Footnote { c } => Inline::Footnote { c: canonical(c, false, false) },
            Inline::Quote { c } => Inline::Quote { c: canonical(c, false, false) },
            Inline::Link { url, c } => Inline::Link { url, c: c.map(|c| canonical(c, false, false)) },
            other => other,
        };
        match (&inline, out.last_mut()) {
            (Inline::Emph { c } | Inline::Strong { c } | Inline::Footnote { c } | Inline::Quote { c }, _)
                if c.is_empty() => {}
            (Inline::Text { v }, Some(Inline::Text { v: last })) => last.push_str(v),
            (Inline::Soft, None | Some(Inline::Soft)) => {}
            _ => out.push(inline),
        }
    }
    while matches!(out.last(), Some(Inline::Soft)) {
        out.pop();
    }
    // Cut off whitespace at the beginning of a line and after a forced break (like the reading: `\ ` swallows
    // the spaces after it); texts that became empty are dropped.
    let mut k = 0;
    while k < out.len() {
        let at_start = (k == 0 && line_start) || (k > 0 && matches!(out[k - 1], Inline::Soft | Inline::Linebreak));
        if at_start && let Inline::Text { v } = &mut out[k] {
            *v = v.trim_start_matches([' ', '\t']).to_owned();
            if v.is_empty() {
                out.remove(k);
                continue;
            }
        }
        k += 1;
    }
    if block_end {
        while let Some(Inline::Text { v }) = out.last_mut() {
            let trimmed = v.trim_end().to_owned();
            if trimmed.is_empty() {
                out.pop();
            } else {
                *v = trimmed;
                break;
            }
        }
    }
    // A break at the edge or doubled, arisen through the cutting, is dropped; then check anew.
    let messy = out.first() == Some(&Inline::Soft)
        || out.last() == Some(&Inline::Soft)
        || out.windows(2).any(|w| w[0] == Inline::Soft && w[1] == Inline::Soft)
        || out.windows(2).any(|w| matches!(w, [Inline::Text { .. }, Inline::Text { .. }]));
    if messy { canonical(out, line_start, block_end) } else { out }
}

/// Canonical heading (Round 18b, gap from the backlog „Nach Runde 14 offen“): like [`canonical`], plus, like
/// `split_heading` when generating, also remove whitespace at the beginning of an emphasis standing at the front
/// (arbitrarily deep); an emphasis emptied by that is dropped. Before, `_ Brunnen_` stayed, but `emit` wrote
/// `_Brunnen_` – a failure of the generator, not of the product.
fn canonical_heading(inlines: Vec<Inline>) -> Vec<Inline> {
    fn trim_front(mut inlines: Vec<Inline>) -> Vec<Inline> {
        loop {
            match inlines.first_mut() {
                Some(Inline::Text { v }) => {
                    let trimmed = v.trim_start_matches([' ', '\t']).to_owned();
                    if trimmed.is_empty() {
                        inlines.remove(0);
                    } else {
                        *v = trimmed;
                        return inlines;
                    }
                }
                Some(Inline::Emph { c } | Inline::Strong { c }) => {
                    *c = trim_front(std::mem::take(c));
                    if c.is_empty() {
                        inlines.remove(0);
                    } else {
                        return inlines;
                    }
                }
                _ => return inlines,
            }
        }
    }
    let once = canonical(trim_front(canonical(inlines, true, true)), true, true);
    // The cutting can put two texts next to each other or empty an emphasis: until it stands still.
    let again = canonical(trim_front(once.clone()), true, true);
    if again == once { once } else { canonical_heading(again) }
}

/// Generated blocks that are no sensible input are discarded: empty headings and
/// list items, paragraphs without text of their own (`parse` reads those as an island), text after an island
/// that would merge with `@`.
fn valid(kind: &BlockKind) -> bool {
    let substantive = |inlines: &[Inline]| {
        inlines.iter().any(|i| match i {
            Inline::Text { v } => !v.trim().is_empty(),
            Inline::Soft | Inline::Island { .. } | Inline::Linebreak => false,
            _ => true,
        })
    };
    match kind {
        BlockKind::Heading { inlines, .. } => !inlines.is_empty(),
        BlockKind::Paragraph { inlines } => substantive(inlines),
        BlockKind::List { items, .. } | BlockKind::Enum { items, .. } => items.iter().all(|item| !item.is_empty()),
        BlockKind::Terms { items } => items.iter().all(|t| substantive(&t.term)),
        BlockKind::Part { title } | BlockKind::Unnumbered { title, .. } => !title.is_empty(),
        _ => true,
    }
}

#[test]
fn random_canonical_blocks_read_back_unchanged() {
    // Fixed seed: the same run on every machine; a failure names the case.
    let mut generator = Generator {
        rng: fastrand::Rng::with_seed(
            std::env::var("WS_SEED").ok().and_then(|s| s.parse().ok()).unwrap_or(0x5a7_2b11d),
        ),
    };
    let cases = std::env::var("WS_CASES").ok().and_then(|s| s.parse().ok()).unwrap_or(2000);
    for case in 0..cases {
        let kind = generator.block(0);
        assert_round_trip(&kind, &format!("Fall {case} von {cases}: {kind:?}"));
    }
}

#[test]
fn random_markers_read_back_unchanged() {
    // Round 14: part and "without number" with random titles from all Typst special characters. Own
    // generator with its own seed, so that the cases of the test above stay unchanged.
    let mut generator = Generator { rng: fastrand::Rng::with_seed(0x007e_1114) };
    for case in 0..2000 {
        let title = generator.text().trim().to_owned();
        let kind = if generator.rng.bool() {
            BlockKind::Part { title }
        } else {
            BlockKind::Unnumbered { level: generator.rng.u8(1..=3), title }
        };
        if !valid(&kind) {
            continue; // empty title: no outline item
        }
        assert_round_trip(&kind, &format!("Markierung {case}: {kind:?}"));
    }
}

#[test]
fn space_at_the_start_of_an_emphasis_in_a_heading_is_trimmed_by_the_generator() {
    // Case from the backlog („Nach Runde 14 offen“, test generator): a heading whose title starts with an
    // emphasis with whitespace at its front. `emit` writes it like `split_heading` without this
    // whitespace; the old `canonical` left it, and the comparison failed (e.g. case 1168 with another
    // seed).
    let raw = vec![Inline::Emph { c: vec![text(" Brunnen")] }, text(" am Markt")];
    let untrimmed = BlockKind::Heading { level: 1, inlines: raw.clone(), label: None };
    assert_eq!(emit(&untrimmed), "= _Brunnen_ am Markt", "emit entfernt den Leerraum in der Hervorhebung");
    assert_ne!(
        without_raw(&parse(&emit(&untrimmed)).blocks[0].kind),
        untrimmed,
        "ungekürzt ist die Überschrift nicht kanonisch – genau das war die Lücke im Generator"
    );
    let fixed = canonical_heading(raw);
    assert_eq!(
        fixed,
        vec![Inline::Emph { c: vec![text("Brunnen")] }, text(" am Markt")],
        "canonical_heading kürzt auch in der vorn stehenden Hervorhebung"
    );
    assert_round_trip(&BlockKind::Heading { level: 1, inlines: fixed, label: None }, "Fall aus dem Backlog");
    // An emphasis that becomes empty through the shortening is dropped entirely (like `trim_front` when generating).
    assert_eq!(
        canonical_heading(vec![Inline::Strong { c: vec![text("  ")] }, text(" Titel")]),
        vec![text("Titel")],
        "leere Hervorhebung vorn entfällt, der Text dahinter beginnt ohne Leerraum"
    );
}

#[test]
fn a_reference_before_a_word_character_is_written_as_ref_call() {
    // Typst would read `@abb` directly before a letter as a longer target (`@abbx`); `emit` then takes `#ref(<…>)`.
    let kind = para(vec![Inline::Ref { target: "abb".into(), supplement: None }, text("x")]);
    assert_eq!(emit(&kind), "#ref(<abb>)x", "Verweis vor Wortzeichen als Funktionsaufruf");
    assert_round_trip(&kind, "Verweis vor Wortzeichen");
    let kind = para(vec![text("siehe "), Inline::Ref { target: "abb".into(), supplement: Some("S. 2".into()) }]);
    assert_eq!(emit(&kind), "siehe @abb[S. 2]", "sonst die Kurzform mit Zusatz in Klammern");
}

#[test]
fn a_forced_line_break_keeps_a_space_before_closing_characters() {
    // `\]` or `\_` would be escapes; therefore a space stands after the break in a content block.
    let kind = para(vec![text("Satz"), Inline::Footnote { c: vec![text("Notiz"), Inline::Linebreak] }]);
    assert_eq!(emit(&kind), "Satz#footnote[Notiz\\ ]", "Leerzeichen vor der schließenden Klammer");
    assert_round_trip(&kind, "Umbruch am Ende einer Fußnote");
    let kind = para(vec![text("Ende"), Inline::Linebreak]);
    assert_eq!(emit(&kind), "Ende\\", "am Blockende steht der Umbruch allein");
}
