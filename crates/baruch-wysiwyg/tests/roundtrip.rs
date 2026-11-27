// SPDX-License-Identifier: AGPL-3.0-or-later
//! Round trip byte-identical: for every test file of all crates (`crates/*/tests/data/*.typ`, only read)
//! and the own `tests/data/satzbild.typ` it holds – for the whole file and for every section according to
//! `baruch_typst::parse` –: the concatenation of all `raw` is the text, and `serialize` with nothing but `Keep`
//! yields it again. The revisions of the sample work are checked by `baruch-server/tests/wysiwyg.rs`.

use std::path::PathBuf;

use baruch_wysiwyg::{BlockKind, Doc, Edit, Inline, parse, serialize};

const SATZBILD: &str = include_str!("data/satzbild.typ");
const SZENEN: &str = include_str!("data/szenen.typ");

/// All section texts and the whole file.
fn pieces(source: &str) -> Vec<String> {
    let mut out = vec![source.to_owned()];
    if let Ok(doc) = baruch_typst::parse(source) {
        out.push(doc.template.clone());
        out.extend(doc.headings.into_iter().map(|h| h.body));
    }
    out
}

fn keep_all(doc: &Doc) -> Vec<Edit> {
    (0..doc.blocks.len()).map(|keep| Edit::Keep { keep }).collect()
}

fn assert_byte_equal(text: &str, origin: &str) {
    let doc = parse(text);
    let joined: String = doc.blocks.iter().map(|b| b.raw.as_str()).collect();
    assert_eq!(joined, text, "{origin}: Verkettung der raw ist der Text");
    let again = serialize(&doc, &keep_all(&doc)).expect("lauter Keep ist gültig");
    assert_eq!(again, text, "{origin}: serialize mit lauter Keep ist byte-gleich");
}

/// Test files of all crates: `crates/*/tests/data/*.typ`.
fn data_files() -> Vec<PathBuf> {
    let crates = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut files = Vec::new();
    for entry in std::fs::read_dir(&crates).unwrap() {
        let data = entry.unwrap().path().join("tests/data");
        let Ok(list) = std::fs::read_dir(&data) else { continue };
        for file in list {
            let path = file.unwrap().path();
            if path.extension().is_some_and(|e| e == "typ") {
                files.push(path);
            }
        }
    }
    files.sort();
    files
}

#[test]
fn every_test_file_of_every_crate_comes_back_byte_for_byte() {
    let files = data_files();
    assert!(files.len() >= 5, "Testdateien von typst, store, server und die eigene gefunden: {files:?}");
    assert!(files.iter().any(|f| f.ends_with("satzbild.typ")), "die eigene Datei ist dabei");
    assert!(files.iter().any(|f| f.ends_with("szenen.typ")), "die Datei mit Szenen und Bild ist dabei");
    for file in files {
        let source = std::fs::read_to_string(&file).unwrap();
        for (k, piece) in pieces(&source).iter().enumerate() {
            assert_byte_equal(piece, &format!("{} Teil {k}", file.display()));
        }
    }
}

#[test]
fn unusual_texts_come_back_byte_for_byte() {
    let texts = [
        "",
        "\n",
        "\n\n= Titel\n",
        "ohne Zeilenende",
        "= Titel\r\nmit CRLF\r\n",
        "Absatz\n\n\n\n",
        "  eingerückt\n",
        "\u{feff}= Mit BOM\n",
        "#szene[ nicht geschlossen\n\nText\n",
        "Text ] mit Klammer\n",
        "\\",
        "_",
        "#",
        "$offen",
        "/* offen",
        "`offen",
        "- \n+ \n",
        "= \n",
        "#quote(block: true)[a\n\n#quote(block: false)[b]]\n",
    ];
    for text in texts {
        assert_byte_equal(text, &format!("{text:?}"));
    }
}

fn kinds(doc: &Doc) -> Vec<&'static str> {
    doc.blocks
        .iter()
        .map(|b| match &b.kind {
            BlockKind::Heading { .. } => "heading",
            BlockKind::Paragraph { .. } => "paragraph",
            BlockKind::Quote { .. } => "quote",
            BlockKind::List { ordered: false, .. } => "list",
            BlockKind::List { ordered: true, .. } => "olist",
            BlockKind::Enum { .. } => "enum",
            BlockKind::Terms { .. } => "terms",
            BlockKind::Scene { .. } => "scene",
            BlockKind::Work { .. } => "work",
            BlockKind::Part { .. } => "part",
            BlockKind::Unnumbered { .. } => "unnumbered",
            BlockKind::Island => "island",
        })
        .collect()
}

fn all_inlines(doc: &Doc) -> Vec<Inline> {
    fn walk(inlines: &[Inline], out: &mut Vec<Inline>) {
        for inline in inlines {
            out.push(inline.clone());
            if let Inline::Emph { c } | Inline::Strong { c } | Inline::Footnote { c } | Inline::Quote { c } = inline {
                walk(c, out);
            }
            if let Inline::Link { c: Some(c), .. } = inline {
                walk(c, out);
            }
        }
    }
    let mut out = Vec::new();
    for block in &doc.blocks {
        match &block.kind {
            BlockKind::Heading { inlines, .. } | BlockKind::Paragraph { inlines } => walk(inlines, &mut out),
            BlockKind::List { items, .. } | BlockKind::Enum { items, .. } => {
                items.iter().for_each(|i| walk(i, &mut out))
            }
            BlockKind::Terms { items } => items.iter().for_each(|t| {
                walk(&t.term, &mut out);
                walk(&t.desc, &mut out);
            }),
            _ => {}
        }
    }
    out
}

#[test]
fn the_satzbild_file_contains_every_construct_and_island() {
    let doc = parse(SATZBILD);
    let found = kinds(&doc);
    for kind in ["heading", "paragraph", "quote", "list", "olist", "enum", "terms", "scene", "work", "island"] {
        assert!(found.contains(&kind), "Art {kind} kommt vor: {found:?}");
    }
    let inlines = all_inlines(&doc);
    let has = |f: fn(&Inline) -> bool| inlines.iter().any(f);
    assert!(has(|i| matches!(i, Inline::Emph { .. })), "Hervorhebung erkannt");
    assert!(has(|i| matches!(i, Inline::Strong { .. })), "Fett erkannt");
    assert!(has(|i| matches!(i, Inline::Footnote { .. })), "Fußnote erkannt");
    assert!(has(|i| matches!(i, Inline::Quote { .. })), "Zitat im Satz erkannt");
    assert!(has(|i| matches!(i, Inline::Label { .. })), "Label erkannt");
    assert!(has(|i| matches!(i, Inline::Soft)), "Zeilenumbruch erkannt");
    assert!(has(|i| matches!(i, Inline::Island { raw } if raw.starts_with('$'))), "Formel als Insel im Satz");
    // Round 18b: references, links and forced breaks are no longer islands.
    assert!(
        has(|i| matches!(i, Inline::Ref { target, supplement: None } if target == "brunnen")),
        "Verweis @brunnen als Verweis (Schaltfläche), nicht als Insel"
    );
    assert!(
        has(|i| matches!(i, Inline::Link { url, c: Some(_) } if url == "https://example.org")),
        "#link(…)[…] als Link mit bearbeitbarem Text"
    );
    assert!(has(|i| matches!(i, Inline::Linebreak)), "erzwungener Zeilenumbruch \\ erkannt");
    assert!(has(|i| matches!(i, Inline::Text { v } if v.contains('\u{a0}'))), "~ als geschütztes Leerzeichen");
    assert!(has(|i| matches!(i, Inline::Text { v } if v.contains("C:\\Daten"))), "Maskierungen aufgelöst");
    // The code lines at the beginning and at the end, raw block, open _ and (Round 18b) lists with mixed
    // markers or indented term lists and jumping numbers stay islands.
    let islands: Vec<&str> =
        doc.blocks.iter().filter(|b| b.kind == BlockKind::Island).map(|b| b.raw.trim_end()).collect();
    for start in [
        "#set text",
        "#pagebreak()",
        "$ sum",
        "```rust",
        "#show heading",
        "Zum Schluss",
        "- gemischt",
        "/ Außen",
        "3. springt",
    ] {
        assert!(islands.iter().any(|raw| raw.starts_with(start)), "Insel beginnt mit {start:?}: {islands:#?}");
    }
    // Round 18b: enumeration, term list, nested list and the break with \ are constructs.
    let block = |start: &str| {
        doc.blocks.iter().find(|b| b.raw.starts_with(start)).unwrap_or_else(|| panic!("Block {start:?} fehlt"))
    };
    assert!(
        matches!(&block("1. Aufz").kind, BlockKind::Enum { start: 1, items, depths } if items.len() == 2 && depths.is_empty()),
        "Aufzählung 1./2. als Enum: {:?}",
        block("1. Aufz").kind
    );
    assert!(
        matches!(&block("/ Begriff").kind, BlockKind::Terms { items } if items.len() == 1),
        "Begriffsliste als Terms: {:?}",
        block("/ Begriff").kind
    );
    assert!(
        matches!(&block("- äußerer").kind, BlockKind::List { depths, .. } if depths == &[0, 1]),
        "geschachtelte Liste mit Tiefen 0 und 1: {:?}",
        block("- äußerer").kind
    );
    assert!(
        matches!(&block("Ein Umbruch").kind, BlockKind::Paragraph { inlines } if inlines.contains(&Inline::Linebreak)),
        "Absatz mit erzwungenem Umbruch bleibt bearbeitbar: {:?}",
        block("Ein Umbruch").kind
    );
}

#[test]
fn a_heading_keeps_its_label_and_the_comment_stays_out_of_the_title() {
    let doc = parse("== Ein Gespräch <ort> // Notiz\n");
    assert_eq!(
        doc.blocks[0].kind,
        BlockKind::Heading {
            level: 2,
            inlines: vec![Inline::Text { v: "Ein Gespräch ".into() }, Inline::Label { v: "ort".into() }],
            label: None,
        },
        "Label gehört zur Überschrift, der Kommentar nicht (ihn behält serialize aus der Zeile)"
    );
}

#[test]
fn the_scenes_file_has_labelled_headings_scenes_works_and_an_image_island() {
    let doc = parse(SZENEN);
    assert_byte_equal(SZENEN, "szenen.typ");
    assert!(
        doc.blocks.iter().any(|b| matches!(&b.kind, BlockKind::Heading { label: Some(l), .. } if l == "werkstatt")),
        "Überschrift mit Label auf der nächsten Zeile"
    );
    let scenes: Vec<&BlockKind> =
        doc.blocks.iter().map(|b| &b.kind).filter(|k| matches!(k, BlockKind::Scene { .. })).collect();
    assert_eq!(scenes.len(), 2, "zwei Szenen: {scenes:#?}");
    assert!(
        matches!(scenes[0], BlockKind::Scene { args: None, body } if body.len() == 3
            && body.iter().all(|b| matches!(b.kind, BlockKind::Paragraph { .. }))),
        "die erste Szene hat drei bearbeitbare Absätze: {:#?}",
        scenes[0]
    );
    assert!(
        matches!(scenes[1], BlockKind::Scene { args: Some(a), body } if a == "ort: [Hof]"
            && matches!(body[1].kind, BlockKind::Quote { .. })),
        "die zweite Szene trägt Argumente und ein Zitat im Rumpf: {:#?}",
        scenes[1]
    );
    assert!(
        doc.blocks.iter().any(|b| matches!(&b.kind, BlockKind::Work { body, .. }
            if body.iter().any(|x| matches!(x.kind, BlockKind::List { .. })))),
        "ein Arbeitsauftrag mit Liste"
    );
    let image = doc.blocks.iter().find(|b| b.raw.starts_with("#figure")).expect("Abbildung vorhanden");
    assert_eq!(image.kind, BlockKind::Island, "die Abbildung ist eine Insel");
    assert_eq!(baruch_wysiwyg::image_path(&image.raw).as_deref(), Some("/bilder/erfunden.svg"), "Bildpfad erkannt");
    let footnotes = all_inlines(&doc).iter().filter(|i| matches!(i, Inline::Footnote { .. })).count();
    assert!(footnotes >= 2, "Fußnoten im Fließtext: {footnotes}");
}
