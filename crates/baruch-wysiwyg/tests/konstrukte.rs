// SPDX-License-Identifier: AGPL-3.0-or-later
//! Round 18b ("konstrukte"): more constructs instead of islands – enumerations `1.`, term lists `/ T: E`,
//! nested lists, forced break `\`, `#link`, references `@…` –, islands in place
//! (`Edit::Raw`) and several images in one island. Untouched, every block stays byte-identical, edited it
//! becomes canonical; old JSON forms still deserialise. All texts freely invented.

use baruch_wysiwyg::{
    BlockKind, Doc, Edit, ImageRef, Inline, Term, WysiwygError, emit, image_path, image_paths, parse, render_html,
    render_html_with, serialize,
};
use serde_json::json;

fn text(v: &str) -> Inline {
    Inline::Text { v: v.into() }
}

fn keep_all(doc: &Doc) -> Vec<Edit> {
    (0..doc.blocks.len()).map(|keep| Edit::Keep { keep }).collect()
}

/// A text with all new constructs, freely invented.
const MARKT: &str = "= Markttag\n\n\
1. Wasser holen\n2. Feuer machen\n   und Brot backen\n\n\
/ Krug: Gefäß aus Ton\n/ Elle: altes Maß, etwa 60 cm\n\n\
- Obst\n  - Äpfel\n  - Birnen\n- Gemüse\n\n\
Erste Zeile \\\nzweite Zeile, siehe #link(\"https://example.org/markt\")[Marktordnung] und @stand[S. 2].\n";

#[test]
fn the_new_constructs_are_recognised_as_blocks_and_inlines() {
    let doc = parse(MARKT);
    let kinds: Vec<&BlockKind> = doc.blocks.iter().map(|b| &b.kind).collect();
    assert_eq!(kinds.len(), 5, "Überschrift, Aufzählung, Begriffsliste, Liste, Absatz: {kinds:#?}");
    assert_eq!(
        kinds[1],
        &BlockKind::Enum {
            start: 1,
            items: vec![vec![text("Wasser holen")], vec![text("Feuer machen"), Inline::Soft, text("und Brot backen")]],
            depths: vec![],
        },
        "Aufzählung mit Folgezeile (eingerückt) statt Insel"
    );
    assert_eq!(
        kinds[2],
        &BlockKind::Terms {
            items: vec![
                Term { term: vec![text("Krug")], desc: vec![text("Gefäß aus Ton")] },
                Term { term: vec![text("Elle")], desc: vec![text("altes Maß, etwa 60 cm")] },
            ],
        },
        "Begriffsliste: Begriff bis zum ersten Doppelpunkt, Erklärung dahinter"
    );
    assert_eq!(
        kinds[3],
        &BlockKind::List {
            ordered: false,
            items: vec![vec![text("Obst")], vec![text("Äpfel")], vec![text("Birnen")], vec![text("Gemüse")]],
            depths: vec![0, 1, 1, 0],
        },
        "geschachtelte Liste mit Tiefe je Punkt"
    );
    assert_eq!(
        kinds[4],
        &BlockKind::Paragraph {
            inlines: vec![
                text("Erste Zeile "),
                Inline::Linebreak,
                Inline::Soft,
                text("zweite Zeile, siehe "),
                Inline::Link { url: "https://example.org/markt".into(), c: Some(vec![text("Marktordnung")]) },
                text(" und "),
                Inline::Ref { target: "stand".into(), supplement: Some("S. 2".into()) },
                text("."),
            ],
        },
        "Umbruch, Link und Verweis im Absatz statt einer Insel"
    );
}

#[test]
fn untouched_blocks_stay_byte_equal_and_edited_ones_are_canonical() {
    let doc = parse(MARKT);
    assert_eq!(serialize(&doc, &keep_all(&doc)).unwrap(), MARKT, "lauter keep: byte-gleich");
    // Edit the enumeration (third item added, indentation of the continuation line canonical); the rest stays
    // verbatim.
    let BlockKind::Enum { start, mut items, depths } = doc.blocks[1].kind.clone() else { panic!("Aufzählung") };
    items.push(vec![text("Tisch decken")]);
    let mut edits = keep_all(&doc);
    edits[1] = Edit::Block { replaces: Some(1), block: BlockKind::Enum { start, items, depths } };
    let out = serialize(&doc, &edits).unwrap();
    assert!(
        out.contains("1. Wasser holen\n2. Feuer machen\n  und Brot backen\n3. Tisch decken\n\n/ Krug"),
        "kanonisch: fortlaufende Nummern, Folgezeile mit zwei Leerzeichen, Leerzeile dahinter wie vorher: {out}"
    );
    assert!(
        out.starts_with("= Markttag\n\n") && out.ends_with(&MARKT[MARKT.find("/ Krug").unwrap()..]),
        "Rest wörtlich"
    );
}

#[test]
fn every_new_kind_reads_back_from_its_emitted_typst() {
    let cases = [
        BlockKind::Enum { start: 7, items: vec![vec![text("sieben")], vec![text("acht")]], depths: vec![] },
        BlockKind::List {
            ordered: true,
            items: vec![vec![text("a")], vec![text("b")], vec![text("c")], vec![text("d")]],
            depths: vec![0, 1, 2, 0],
        },
        BlockKind::Terms { items: vec![Term { term: vec![text("Zeit: Ort")], desc: vec![text("beides")] }] },
        BlockKind::Paragraph {
            inlines: vec![
                text("vor"),
                Inline::Linebreak,
                text("nach "),
                Inline::Ref { target: "x".into(), supplement: None },
            ],
        },
    ];
    for kind in cases {
        let typst = emit(&kind);
        let doc = parse(&typst);
        assert_eq!(doc.blocks.len(), 1, "ein Block aus {typst:?}");
        assert_eq!(doc.blocks[0].kind, kind, "parse(emit(k)) == k für {typst:?}");
    }
    assert_eq!(
        emit(&BlockKind::Terms { items: vec![Term { term: vec![text("Zeit: Ort")], desc: vec![] }] }),
        "/ Zeit\\: Ort:",
        "ein Doppelpunkt im Begriff wird maskiert, sonst endete der Begriff dort"
    );
}

#[test]
fn unsafe_lists_stay_islands() {
    for (source, why) in [
        ("- außen\n  + innen\n", "geschachtelt mit anderem Zeichen"),
        ("1. eins\n3. drei\n", "springende Nummer"),
        ("1. eins\n  2. innen beginnt nicht bei 1\n", "geschachtelte Gruppe nicht ab 1"),
        ("/ Außen: a\n  / Innen: b\n", "geschachtelte Begriffsliste"),
        ("/ ohne Doppelpunkt\n", "Begriff ohne Doppelpunkt"),
        ("- a\n    - b\n  - c\n", "Spalte zwischen zwei Ebenen"),
        ("- a\n\t- b\n", "Tabulator im Einzug"),
    ] {
        let doc = parse(source);
        assert_eq!(doc.blocks.len(), 1, "{why}: ein Block");
        assert_eq!(doc.blocks[0].kind, BlockKind::Island, "{why}: nicht sicher abbildbar → Insel");
        assert_eq!(serialize(&doc, &keep_all(&doc)).unwrap(), source, "{why}: byte-gleich");
    }
}

#[test]
fn old_json_forms_still_deserialize() {
    // Before Round 18b a list had no `depths`; the widget of older pages still sends this form.
    let old: BlockKind = serde_json::from_value(json!({
        "kind": "list", "ordered": false, "items": [[{"t": "text", "v": "eins"}]]
    }))
    .unwrap();
    assert_eq!(
        old,
        BlockKind::List { ordered: false, items: vec![vec![text("eins")]], depths: vec![] },
        "alte Liste ohne depths: alle Punkte oben"
    );
    assert_eq!(
        serde_json::to_value(&old).unwrap(),
        json!({"kind": "list", "ordered": false, "items": [[{"t": "text", "v": "eins"}]]}),
        "ohne Schachtelung wird depths nicht geschrieben (Antwort wie bisher)"
    );
    let edits: Vec<Edit> = serde_json::from_value(json!([
        {"keep": 0},
        {"replaces": 1, "block": {"kind": "paragraph", "inlines": []}},
        {"replaces": 2, "body": [{"keep": 0}]},
        {"replaces": 3, "raw": "#let x = 2"},
        {"raw": "#neu()"}
    ]))
    .unwrap();
    assert!(matches!(edits[0], Edit::Keep { keep: 0 }), "keep wie bisher");
    assert!(matches!(edits[1], Edit::Block { replaces: Some(1), .. }), "Block wie bisher");
    assert!(matches!(edits[2], Edit::Body { replaces: 2, .. }), "Rumpf wie bisher");
    assert_eq!(edits[3], Edit::Raw { replaces: Some(3), raw: "#let x = 2".into() }, "neu: Quelltext eines Blocks");
    assert_eq!(edits[4], Edit::Raw { replaces: None, raw: "#neu()".into() }, "ohne replaces: neue Insel (abgelehnt)");
    let json =
        serde_json::to_value(BlockKind::Enum { start: 1, items: vec![vec![text("a")]], depths: vec![] }).unwrap();
    assert_eq!(
        json,
        json!({"kind": "enum", "start": 1, "items": [[{"t": "text", "v": "a"}]]}),
        "JSON-Form der Aufzählung"
    );
    let enum_default: BlockKind =
        serde_json::from_value(json!({"kind": "enum", "items": [[{"t": "text", "v": "a"}]]})).unwrap();
    assert!(matches!(enum_default, BlockKind::Enum { start: 1, .. }), "ohne start beginnt die Aufzählung bei 1");
    let inline = serde_json::to_value(Inline::Ref { target: "x".into(), supplement: None }).unwrap();
    assert_eq!(inline, json!({"t": "ref", "target": "x"}), "JSON-Form des Verweises");
    let inline = serde_json::to_value(Inline::Link { url: "u".into(), c: None }).unwrap();
    assert_eq!(inline, json!({"t": "link", "url": "u"}), "JSON-Form des Links ohne Text");
    assert_eq!(serde_json::to_value(Inline::Linebreak).unwrap(), json!({"t": "linebreak"}), "JSON-Form des Umbruchs");
}

#[test]
fn an_island_is_edited_in_place_with_raw() {
    let source = "Davor.\n\n#let zahl = 3\n\nDanach.\n";
    let doc = parse(source);
    assert_eq!(doc.blocks[1].kind, BlockKind::Island, "Code-Zeile ist eine Insel");
    let edits = vec![
        Edit::Keep { keep: 0 },
        Edit::Raw { replaces: Some(1), raw: "#let zahl = 4\n".into() },
        Edit::Keep { keep: 2 },
    ];
    assert_eq!(
        serialize(&doc, &edits).unwrap(),
        "Davor.\n\n#let zahl = 4\n\nDanach.\n",
        "nur die Insel ändert sich, wörtlich wie getippt, Leerzeile dahinter bleibt"
    );
    let gone =
        vec![Edit::Keep { keep: 0 }, Edit::Raw { replaces: Some(1), raw: "  \n".into() }, Edit::Keep { keep: 2 }];
    assert_eq!(serialize(&doc, &gone).unwrap(), "Davor.\n\nDanach.\n", "leerer Quelltext löscht die Insel");
    assert_eq!(
        serialize(&doc, &[Edit::Raw { replaces: None, raw: "#neu()".into() }]),
        Err(WysiwygError::NewIsland),
        "neue Inseln anlegen bleibt verboten"
    );
    assert_eq!(
        serialize(&doc, &[Edit::Raw { replaces: Some(9), raw: "x".into() }]),
        Err(WysiwygError::UnknownBlock { index: 9, count: 3 }),
        "unbekannter Index → Fehler"
    );
    assert_eq!(
        serialize(&doc, &[Edit::Keep { keep: 1 }, Edit::Raw { replaces: Some(1), raw: "x".into() }]),
        Err(WysiwygError::DuplicateBlock(1)),
        "ein Index nur einmal"
    );
}

#[test]
fn an_island_in_a_scene_is_edited_in_place_and_keeps_the_indent() {
    let source =
        "#szene[\n  Mira setzt sich.\n\n  #let ort = \"Hof\"\n  #let zeit = 3\n\n  Ein Hund läuft vorbei.\n]\n";
    let doc = parse(source);
    let BlockKind::Scene { body, .. } = &doc.blocks[0].kind else { panic!("Szene") };
    assert_eq!(body[1].kind, BlockKind::Island, "Code im Rumpf ist eine Insel");
    // The widget sends the source text as it stood in data-raw (with the indentation of the continuation lines).
    let edits = vec![Edit::Body {
        replaces: 0,
        body: vec![
            Edit::Keep { keep: 0 },
            Edit::Raw { replaces: Some(1), raw: "#let ort = \"Garten\"\n  #let zeit = 4".into() },
            Edit::Keep { keep: 2 },
        ],
    }];
    assert_eq!(
        serialize(&doc, &edits).unwrap(),
        "#szene[\n  Mira setzt sich.\n\n  #let ort = \"Garten\"\n  #let zeit = 4\n\n  Ein Hund läuft vorbei.\n]\n",
        "die Insel im Rumpf mit dem Einzug des Rumpfs, die Absätze byte-gleich"
    );
}

/// Scene with a raw block island (```` ``` ````) in the indented body (finding after Round 18).
const SCENE_WITH_RAW_BLOCK: &str =
    "#szene[\n  Absatz eins.\n\n  ```rust\n  fn main() {\n      x();\n  }\n  ```\n\n  Absatz zwei.\n]\n";

fn edit_island_in_scene(raw: &str) -> String {
    let doc = parse(SCENE_WITH_RAW_BLOCK);
    let BlockKind::Scene { body, .. } = &doc.blocks[0].kind else { panic!("Szene") };
    assert_eq!(body[1].kind, BlockKind::Island, "der Rohblock im Rumpf ist eine Insel");
    let edits = vec![Edit::Body {
        replaces: 0,
        body: vec![Edit::Keep { keep: 0 }, Edit::Raw { replaces: Some(1), raw: raw.into() }, Edit::Keep { keep: 2 }],
    }];
    serialize(&doc, &edits).unwrap()
}

#[test]
fn an_unchanged_raw_block_island_in_a_scene_stays_byte_equal() {
    // As the widget sends data-raw: first line without, continuation lines with the indentation of the body.
    let raw = "```rust\n  fn main() {\n      x();\n  }\n  ```";
    assert_eq!(
        edit_island_in_scene(raw),
        SCENE_WITH_RAW_BLOCK,
        "unverändert zurückgeschickt bleibt die Szene byte-gleich: der Einzug des Rumpfs wird auch mit ``` gesetzt"
    );
    // Also with indentation in the first line (both are allowed).
    assert_eq!(
        edit_island_in_scene(&format!("  {raw}")),
        SCENE_WITH_RAW_BLOCK,
        "mit Einzug in der ersten Zeile dasselbe Ergebnis"
    );
}

#[test]
fn a_raw_block_island_sent_without_indent_keeps_its_content() {
    // Sent through RPC without the indentation of the body: the code line has 4 spaces of indentation of its own.
    let raw = "```rust\nfn main() {\n    x();\n}\n```";
    assert_eq!(
        edit_island_in_scene(raw),
        SCENE_WITH_RAW_BLOCK,
        "nicht jede Folgezeile trägt den Einzug, also wird nichts entfernt: aus 4 Leerzeichen vor x() werden \
         nicht 2, und der Rumpf-Einzug kommt einheitlich davor"
    );
}

#[test]
fn a_changed_raw_block_island_in_a_scene_gets_the_body_indent() {
    let out = edit_island_in_scene("```rust\nfn main() {\n    y();\n}\n```");
    assert_eq!(
        out, "#szene[\n  Absatz eins.\n\n  ```rust\n  fn main() {\n      y();\n  }\n  ```\n\n  Absatz zwei.\n]\n",
        "geänderter Rohblock: jede Zeile mit dem Einzug des Rumpfs, der eigene Einzug im Code bleibt"
    );
    let reparsed = parse(&out);
    let BlockKind::Scene { body, .. } = &reparsed.blocks[0].kind else { panic!("Szene") };
    assert_eq!(
        body.len(),
        3,
        "der Rumpf zerfällt wieder in Absatz, Insel, Absatz – nichts ist aus ihm herausgerutscht"
    );
}

/// Images for the test: every path under /bilder/ has an image.
fn images(path: &str) -> Option<ImageRef> {
    path.starts_with("/bilder/").then(|| ImageRef {
        src: format!("/b?p={path}"),
        alt: format!("Alt {path}"),
        missing: false,
    })
}

#[test]
fn a_block_with_several_images_shows_all_of_them() {
    let raw = "#grid(columns: 2, image(\"/bilder/a.png\"), image(\"/bilder/b.png\"))\n";
    assert_eq!(image_path(raw).as_deref(), Some("/bilder/a.png"), "image_path bleibt beim ersten Bild");
    assert_eq!(image_paths(raw), vec!["/bilder/a.png".to_owned(), "/bilder/b.png".to_owned()], "image_paths: alle");
    let html = render_html_with(&parse(raw), &images);
    assert_eq!(html.matches("<img").count(), 2, "beide Bilder im Satzbild (vorher nur das erste): {html}");
    assert!(html.contains("src=\"/b?p=/bilder/b.png\""), "auch das zweite: {html}");
    let inline =
        render_html_with(&parse("Text #box[#image(\"/bilder/c.png\") #image(\"/bilder/d.png\")] mehr\n"), &images);
    assert_eq!(inline.matches("<img").count(), 2, "auch eine Insel im Satz zeigt alle Bilder: {inline}");
}

#[test]
fn the_html_shows_the_new_constructs() {
    let html = render_html(&parse(MARKT));
    assert!(
        html.contains("data-kind=\"enum\" data-start=\"1\"><ol start=\"1\"><li>Wasser holen</li>"),
        "Aufzählung: {html}"
    );
    assert!(
        html.contains("data-kind=\"terms\"><dl><dt>Krug</dt><dd>Gefäß aus Ton</dd>"),
        "Begriffsliste als dl/dt/dd: {html}"
    );
    assert!(
        html.contains("<ul><li>Obst<ul><li>Äpfel</li><li>Birnen</li></ul></li><li>Gemüse</li></ul>"),
        "geschachtelte Liste als Liste im Punkt: {html}"
    );
    assert!(
        html.contains("<span class=\"ws-linebreak\" contenteditable=\"false\" data-linebreak=\"1\""),
        "Umbruch: {html}"
    );
    assert!(
        html.contains("<a class=\"ws-link\" data-url=\"https://example.org/markt\" href=\"https://example.org/markt\""),
        "Link mit Adresse (bearbeitbarer Text): {html}"
    );
    assert!(
        html.contains("<button type=\"button\" class=\"ws-ref\" contenteditable=\"false\" tabindex=\"-1\" data-target=\"stand\" data-supplement=\"S. 2\""),
        "Verweis als nicht bearbeitbare Schaltfläche (type=button: schickt das Formular nicht ab): {html}"
    );
    let unsafe_link = render_html(&parse("Ein #link(\"javascript:alert(1)\")[Klick].\n"));
    assert!(!unsafe_link.contains("href="), "kein href für fremde Schemata: {unsafe_link}");
}
