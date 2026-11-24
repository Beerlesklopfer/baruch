// SPDX-License-Identifier: AGPL-3.0-or-later
//! `emit` erzeugt kanonisches Typst, das `parse` genau so zurückliest: `parse(emit(k)) == k` für jede
//! Art außer der Insel (verglichen ohne die `raw` geschachtelter Blöcke). Dazu feste Beispiele und ein
//! Eigenschaftstest mit fester Saat über zufällig erzeugte, kanonische Blöcke.

use baruch_wysiwyg::{Block, BlockKind, Inline, Term, emit, parse};

/// `raw` geschachtelter Blöcke leeren: `emit` schreibt sie nicht, `parse` füllt sie.
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

/// `emit`, dann `parse`: genau ein Block, dessen Art `kind` ist und dessen `raw` der erzeugte Text.
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
        // Runde 18b: geschachtelte Liste, Aufzählung (auch geschachtelt, ab 3), Begriffsliste.
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
        // Runde 18b: erzwungener Umbruch, Link mit und ohne Text, Verweis mit Zusatz und vor Wortzeichen.
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

// ---- Eigenschaftstest --------------------------------------------------------------------------

/// Zeichen für zufällige Texte: Buchstaben, Umlaute, Ziffern, Satz- und alle Typst-Sonderzeichen.
const ALPHABET: &[char] = &[
    'a', 'b', 'e', 'n', 'r', 'x', 'ä', 'ß', 'Ö', '1', '2', '0', ' ', ' ', ' ', '.', ',', ':', ';', '!', '?', '-', '–',
    '=', '+', '/', '\\', '#', '*', '_', '$', '@', '<', '>', '[', ']', '`', '~', '(', ')', '\'', '"', '\u{a0}', '{',
    '}', '%', '&',
];

/// Inseln, die als Inline sicher wörtlich bleiben (keine, die mit einem Bezeichner endet).
const INLINE_ISLANDS: &[&str] = &["$x^2$", "`roh`", "#box[Kasten]", "$a < b$"];
/// Inseln als ganzer Block in einem Rumpf.
const BLOCK_ISLANDS: &[&str] = &["#pagebreak()", "#let zahl = 3", "$ a + b $"];
const LABELS: &[&str] = &["ziel", "abb-1", "kap:2", "a.b"];
/// Ziele von Verweisen (Runde 18b), auch solche, die nur als `#ref(<…>)` gehen.
const TARGETS: &[&str] = &["ziel", "abb-1", "kap:2", "a.b", "ende.", "_unter", "-strich"];
/// Zusätze von Verweisen, wörtlich.
const SUPPLEMENTS: &[&str] = &["S. 3", "Abb.", "_kurz_"];
/// Adressen von Links mit allen Zeichen, die im String maskiert werden müssen.
const URLS: &[&str] = &["https://example.org", "https://example.org/a\"b\\c", "mailto:x@y.de", "datei (1).pdf"];

struct Generator {
    rng: fastrand::Rng,
}

impl Generator {
    fn text(&mut self) -> String {
        let len = self.rng.usize(1..9);
        (0..len).map(|_| ALPHABET[self.rng.usize(..ALPHABET.len())]).collect()
    }

    /// Zufällige Folge; `soft`: Zeilenumbrüche erlaubt, `depth`: Schachtelung.
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
                // Runde 18b: Umbruch, Link, Verweis.
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

    /// Listenpunkte mit gültigen Tiefen (Runde 18b): der erste oben, jeder höchstens eine Ebene tiefer als
    /// der davor; ohne Schachtelung keine Tiefen (so liest `parse` es).
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

    /// Argumente vor dem Rumpf einer Szene/Arbeit (wörtlich), manchmal keine.
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

/// Kanonische Form, wie `parse` sie liefert: keine leeren oder benachbarten Texte, kein Umbruch am
/// Rand oder doppelt, kein Leerraum am Zeilenanfang (`line_start` bzw. nach einem Umbruch) und keiner am
/// Ende des Blocks (`block_end`), keine leeren Auszeichnungen.
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
    // Leerraum am Zeilenanfang und hinter einem erzwungenen Umbruch abschneiden (wie das Lesen: `\ ` schluckt
    // die Leerzeichen dahinter); leer gewordene Texte entfallen.
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
    // Ein Umbruch am Rand oder doppelt, entstanden durch das Abschneiden, entfällt; dann neu prüfen.
    let messy = out.first() == Some(&Inline::Soft)
        || out.last() == Some(&Inline::Soft)
        || out.windows(2).any(|w| w[0] == Inline::Soft && w[1] == Inline::Soft)
        || out.windows(2).any(|w| matches!(w, [Inline::Text { .. }, Inline::Text { .. }]));
    if messy { canonical(out, line_start, block_end) } else { out }
}

/// Kanonische Überschrift (Runde 18b, Lücke aus dem Backlog „Nach Runde 14 offen“): wie [`canonical`], dazu wie
/// `split_heading` beim Erzeugen auch Leerraum am Anfang einer vorn stehenden Hervorhebung (beliebig tief)
/// entfernen; eine dadurch leere Hervorhebung entfällt. Vorher blieb `_ Brunnen_` stehen, `emit` schrieb aber
/// `_Brunnen_` – ein Fehlschlag des Generators, nicht des Produkts.
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
    // Das Abschneiden kann zwei Texte nebeneinander stellen oder eine Hervorhebung leeren: bis es stillsteht.
    let again = canonical(trim_front(once.clone()), true, true);
    if again == once { once } else { canonical_heading(again) }
}

/// Erzeugte Blöcke, die keine sinnvolle Eingabe sind, werden verworfen: leere Überschriften und
/// Listenpunkte, Absätze ohne eigenen Text (die liest `parse` als Insel), Text nach einer Insel, der
/// mit `@` verschmölze.
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
    // Feste Saat: derselbe Lauf auf jedem Rechner; ein Fehlschlag nennt den Fall.
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
    // Runde 14: Teil und „ohne Nummer“ mit zufälligen Titeln aus allen Typst-Sonderzeichen. Eigener
    // Generator mit eigener Saat, damit die Fälle des Tests oben unverändert bleiben.
    let mut generator = Generator { rng: fastrand::Rng::with_seed(0x007e_1114) };
    for case in 0..2000 {
        let title = generator.text().trim().to_owned();
        let kind = if generator.rng.bool() {
            BlockKind::Part { title }
        } else {
            BlockKind::Unnumbered { level: generator.rng.u8(1..=3), title }
        };
        if !valid(&kind) {
            continue; // leerer Titel: kein Gliederungspunkt
        }
        assert_round_trip(&kind, &format!("Markierung {case}: {kind:?}"));
    }
}

#[test]
fn space_at_the_start_of_an_emphasis_in_a_heading_is_trimmed_by_the_generator() {
    // Fall aus dem Backlog („Nach Runde 14 offen“, Testgenerator): Eine Überschrift, deren Titel mit einer
    // Hervorhebung beginnt, in der vorn Leerraum steht. `emit` schreibt sie wie `split_heading` ohne diesen
    // Leerraum; die alte `canonical` ließ ihn stehen, und der Vergleich scheiterte (z. B. Fall 1168 mit anderer
    // Saat).
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
    // Eine Hervorhebung, die durch das Kürzen leer wird, entfällt ganz (wie `trim_front` beim Erzeugen).
    assert_eq!(
        canonical_heading(vec![Inline::Strong { c: vec![text("  ")] }, text(" Titel")]),
        vec![text("Titel")],
        "leere Hervorhebung vorn entfällt, der Text dahinter beginnt ohne Leerraum"
    );
}

#[test]
fn a_reference_before_a_word_character_is_written_as_ref_call() {
    // `@abb` direkt vor einem Buchstaben läse Typst als längeres Ziel (`@abbx`); `emit` nimmt dann `#ref(<…>)`.
    let kind = para(vec![Inline::Ref { target: "abb".into(), supplement: None }, text("x")]);
    assert_eq!(emit(&kind), "#ref(<abb>)x", "Verweis vor Wortzeichen als Funktionsaufruf");
    assert_round_trip(&kind, "Verweis vor Wortzeichen");
    let kind = para(vec![text("siehe "), Inline::Ref { target: "abb".into(), supplement: Some("S. 2".into()) }]);
    assert_eq!(emit(&kind), "siehe @abb[S. 2]", "sonst die Kurzform mit Zusatz in Klammern");
}

#[test]
fn a_forced_line_break_keeps_a_space_before_closing_characters() {
    // `\]` bzw. `\_` wären Maskierungen; darum steht hinter dem Umbruch in einem Inhaltsblock ein Leerzeichen.
    let kind = para(vec![text("Satz"), Inline::Footnote { c: vec![text("Notiz"), Inline::Linebreak] }]);
    assert_eq!(emit(&kind), "Satz#footnote[Notiz\\ ]", "Leerzeichen vor der schließenden Klammer");
    assert_round_trip(&kind, "Umbruch am Ende einer Fußnote");
    let kind = para(vec![text("Ende"), Inline::Linebreak]);
    assert_eq!(emit(&kind), "Ende\\", "am Blockende steht der Umbruch allein");
}
