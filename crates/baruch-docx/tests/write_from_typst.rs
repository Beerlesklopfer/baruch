// SPDX-License-Identifier: AGPL-3.0-or-later
//! `from_typst` (Vertrag Runde 32 §3.4): die im Werk üblichen Formen werden Blöcke des Modells; was Word nicht
//! abbilden kann, geht als Typst-Insel mit wörtlichem Quelltext und Warnung mit. Alle Texte sind frei erfunden.

use baruch_docx::from_typst::{FromTypst, WARN_IMAGE_MISSING, WARN_ISLAND, WARN_TABLE_SIMPLIFIED, from_typst};
use baruch_docx::{Block, Inline, Marks};

/// Liste als (geordnet, erste Nummer, Punkte mit Tiefe und Inhalt).
type ListShape = (bool, u64, Vec<(u8, Vec<Inline>)>);

fn text(text: &str) -> Inline {
    Inline::Text { text: text.to_owned(), marks: Marks::default() }
}

fn marked(text: &str, marks: Marks) -> Inline {
    Inline::Text { text: text.to_owned(), marks }
}

fn read(body: &str) -> FromTypst {
    from_typst(body, &["bilder/garten.png".to_owned(), "bilder/zaun.jpg".to_owned()])
}

fn codes(result: &FromTypst) -> Vec<&str> {
    result.warnings.iter().map(|w| w.code.as_str()).collect()
}

#[test]
fn headings_parts_and_unnumbered_lines_become_headings() {
    let body = format!(
        "{}\n= Der Mühlbach\n\nErster Absatz.\n\n== Am Wehr <wehr>\n\n{}\n==== Zwischentitel\n",
        baruch_typst::part_line("Erster Teil"),
        baruch_typst::unnumbered_line(2, "Vorbemerkung"),
    );
    let result = read(&body);
    assert_eq!(
        result.blocks,
        vec![
            Block::Part { content: vec![text("Erster Teil")], mark: None },
            Block::Heading { level: 1, numbered: true, content: vec![text("Der Mühlbach")], mark: None },
            Block::Paragraph { content: vec![text("Erster Absatz.")], mark: None },
            Block::Heading {
                level: 2,
                numbered: true,
                content: vec![text("Am Wehr "), Inline::Typst { raw: "<wehr>".into(), shown: vec![] }],
                mark: None
            },
            Block::Heading { level: 2, numbered: false, content: vec![text("Vorbemerkung")], mark: None },
            Block::Heading { level: 4, numbered: true, content: vec![text("Zwischentitel")], mark: None },
        ],
        "Teil-Markierung → Teil, = → Überschrift, Markierung „ohne Nummer“ → numbered: false; das Label geht unsichtbar mit"
    );
    assert!(result.warnings.is_empty(), "nichts davon ist eine Insel mit Ersatz: {:?}", result.warnings);
}

#[test]
fn inline_forms_bold_italic_footnote_quote_link_and_line_break() {
    let result = read(
        "Im Beet wuchsen *Bohnen* und _Ringel *blumen*_.#footnote[Beides _erfunden_.] Sie sagte #quote[bald] \\\nund \
         schrieb an #link(\"https://example.org/garten\")[die Gärtnerei].\n",
    );
    let bold = Marks { bold: true, ..Marks::default() };
    let italic = Marks { italic: true, ..Marks::default() };
    let both = Marks { bold: true, italic: true, ..Marks::default() };
    assert_eq!(
        result.blocks,
        vec![Block::Paragraph {
            content: vec![
                text("Im Beet wuchsen "),
                marked("Bohnen", bold),
                text(" und "),
                marked("Ringel ", italic),
                marked("blumen", both),
                text("."),
                Inline::Footnote { content: vec![text("Beides "), marked("erfunden", italic), text(".")] },
                text(" Sie sagte "),
                Inline::Quote { content: vec![text("bald")] },
                text(" "),
                Inline::LineBreak,
                text("und schrieb an "),
                Inline::Link { url: "https://example.org/garten".into(), content: vec![text("die Gärtnerei")] },
                text("."),
            ],
            mark: None
        }],
        "fett, kursiv (auch geschachtelt), Fußnote, Zitat im Text, erzwungener Zeilenumbruch und Verweis sind echte \
         Formen des Modells"
    );
    assert!(result.warnings.is_empty(), "dafür braucht es keine Insel: {:?}", result.warnings);
}

#[test]
fn block_quote_with_source_and_lists() {
    let result = read(
        "#quote(block: true, attribution: [Spruch der Müllerin])[Wer sät, braucht Geduld.]\n\n- Bohnen stecken\n  - \
         erst nach den Eisheiligen\n- Ringelblumen säen\n\n+ gießen\n+ warten\n\n3. dritter Schritt\n4. vierter Schritt\n",
    );
    let Block::Quote { blocks, attribution } = &result.blocks[0] else {
        panic!("Blockzitat erwartet, gefunden: {:?}", result.blocks[0]);
    };
    assert_eq!(attribution, &vec![text("Spruch der Müllerin")], "die Quelle des Zitats");
    assert_eq!(
        blocks,
        &vec![Block::Paragraph { content: vec![text("Wer sät, braucht Geduld.")], mark: None }],
        "der Wortlaut des Zitats als Absatz"
    );
    let lists: Vec<ListShape> = result.blocks[1..]
        .iter()
        .map(|block| match block {
            Block::List { ordered, start, items } => {
                (*ordered, *start, items.iter().map(|i| (i.depth, i.content.clone())).collect())
            }
            other => panic!("Liste erwartet, gefunden: {other:?}"),
        })
        .collect();
    assert_eq!(
        lists,
        vec![
            (
                false,
                1,
                vec![
                    (0, vec![text("Bohnen stecken")]),
                    (1, vec![text("erst nach den Eisheiligen")]),
                    (0, vec![text("Ringelblumen säen")])
                ]
            ),
            (true, 1, vec![(0, vec![text("gießen")]), (0, vec![text("warten")])]),
            (true, 3, vec![(0, vec![text("dritter Schritt")]), (0, vec![text("vierter Schritt")])]),
        ],
        "ungeordnete Liste mit Tiefe, geordnete Liste (+) und Aufzählung mit erster Nummer 3"
    );
    assert!(result.warnings.is_empty(), "Zitat und Listen kennt das Modell: {:?}", result.warnings);
}

#[test]
fn figures_keep_their_source_and_find_their_image() {
    let source =
        "#figure(image(\"/bilder/zaun.jpg\", alt: \"Ein grüner Zaun\", width: 60%), caption: [Der _alte_ Zaun])";
    let result = read(&format!("{source}\n\n#image(\"bilder/garten.png\")\n\n#figure(image(\"bilder/fehlt.png\"))\n"));
    assert_eq!(
        result.blocks[0],
        Block::Figure {
            image: 1,
            alt: "Ein grüner Zaun".into(),
            caption: vec![text("Der "), marked("alte", Marks { italic: true, ..Marks::default() }), text(" Zaun")],
            raw: source.into(),
        },
        "Abbildung: Index des Bildes nach dem Pfad (führendes / zählt nicht), Alternativtext, Unterschrift – und der \
         wörtliche Quelltext, damit Breite und Label nicht verloren gehen"
    );
    assert!(
        matches!(&result.blocks[1], Block::Figure { image: 0, caption, .. } if caption.is_empty()),
        "ein bloßes #image ist eine Abbildung ohne Unterschrift: {:?}",
        result.blocks[1]
    );
    let Block::Typst { raw, shown } = &result.blocks[2] else {
        panic!("ein Bild, das es nicht gibt, bleibt eine Insel: {:?}", result.blocks[2]);
    };
    assert_eq!(raw, "#figure(image(\"bilder/fehlt.png\"))", "mit wörtlichem Quelltext");
    assert_eq!(
        shown,
        &vec![Block::Paragraph { content: vec![text("[Bild: bilder/fehlt.png]")], mark: None }],
        "und einem sichtbaren Hinweis an der Stelle des Bildes"
    );
    assert_eq!(codes(&result), vec![WARN_IMAGE_MISSING], "das fehlende Bild wird gemeldet, nicht still übergangen");
}

#[test]
fn simple_tables_become_tables_and_page_breaks_page_breaks() {
    let result = read(
        "#table(\n  columns: 2,\n  align: left,\n  table.header([*Sorte*], [Zeit]),\n  [Bohne], [Mai],\n)\n\n#pagebreak()\n\n\
         #table(columns: 2, table.cell(colspan: 2)[breit])\n",
    );
    let Block::Table { header_rows, rows } = &result.blocks[0] else {
        panic!("Tabelle erwartet, gefunden: {:?}", result.blocks[0]);
    };
    assert_eq!(*header_rows, 1, "table.header ist die Kopfzeile");
    assert_eq!(rows.len(), 2, "Kopf und eine Zeile");
    assert_eq!(
        rows[0][0],
        vec![Block::Paragraph { content: vec![marked("Sorte", Marks { bold: true, ..Marks::default() })], mark: None }],
        "der Inhalt einer Zelle wird wie Fließtext gelesen (hier fett)"
    );
    assert_eq!(rows[1][1], vec![Block::Paragraph { content: vec![text("Mai")], mark: None }], "letzte Zelle");
    assert_eq!(result.blocks[1], Block::PageBreak, "#pagebreak() ist ein Seitenumbruch");
    assert!(
        matches!(&result.blocks[2], Block::Typst { raw, .. } if raw.contains("colspan")),
        "eine Tabelle mit verbundenen Zellen versteht der Export nicht – sie bleibt wörtlich als Insel: {:?}",
        result.blocks[2]
    );
    assert_eq!(
        codes(&result),
        vec![WARN_TABLE_SIMPLIFIED, WARN_ISLAND],
        "die entfallene Ausrichtung und die Insel werden gemeldet"
    );
}

#[test]
fn unknown_things_become_islands_with_literal_source_and_a_warning() {
    let body = "#let hof(inhalt) = box(stroke: 1pt)[#inhalt]\n\nDer Bach ist $3 dot 4$ Ellen breit, siehe @wehr. // geprüft\n\n\
                #hof[Im Hof]\n\n/ Wehr: Stauanlage im Bach\n\n// nur eine Notiz\n";
    let result = read(body);
    let mut raws = Vec::new();
    for block in &result.blocks {
        match block {
            Block::Typst { raw, .. } => raws.push(raw.clone()),
            Block::Paragraph { content, .. } => {
                raws.extend(content.iter().filter_map(|i| match i {
                    Inline::Typst { raw, .. } => Some(raw.clone()),
                    _ => None,
                }));
            }
            other => panic!("nur Inseln und ein Absatz erwartet, gefunden: {other:?}"),
        }
    }
    for wanted in [
        "#let hof(inhalt) = box(stroke: 1pt)[#inhalt]",
        "$3 dot 4$",
        "@wehr",
        "#hof[Im Hof]",
        "/ Wehr: Stauanlage im Bach",
        "// nur eine Notiz",
    ] {
        assert!(
            raws.iter().any(|raw| raw == wanted),
            "„{wanted}“ steht wörtlich als Quelltext einer Insel da: {raws:?}"
        );
    }
    assert!(
        raws.iter().any(|raw| raw.contains("// geprüft")),
        "auch der Kommentar im Absatz geht nicht verloren: {raws:?}"
    );
    let islands = codes(&result).iter().filter(|c| **c == WARN_ISLAND).count();
    assert_eq!(
        islands, 5,
        "gemeldet werden die fünf Inseln mit sichtbarem Ersatz (#let, Formel, Verweis, eigene Funktion, Begriffsliste); \
         Kommentare sind im Satz unsichtbar und brauchen keine Warnung: {:?}",
        result.warnings
    );
    assert!(
        result.warnings.iter().all(|w| w.message.contains("Typst-Insel")),
        "die Meldung sagt dem Menschen, worum es geht"
    );
    let Some(Block::Typst { shown, .. }) =
        result.blocks.iter().find(|b| matches!(b, Block::Typst { raw, .. } if raw.starts_with("/ Wehr")))
    else {
        panic!("Begriffsliste als Insel erwartet");
    };
    assert_eq!(
        shown,
        &vec![Block::Paragraph {
            content: vec![marked("Wehr", Marks { bold: true, ..Marks::default() }), text(": Stauanlage im Bach")],
            mark: None
        }],
        "sichtbar bleibt bei der Begriffsliste der Begriff (fett) mit seiner Erklärung"
    );
}

#[test]
fn text_is_never_lost() {
    // Jedes Wort des Abschnitts findet sich im Ergebnis wieder: als Text, als Sichtbares oder im Quelltext einer Insel.
    let body = "= Ernte\n\nKürbis und *Mangold*#footnote[Winterhart.] wachsen.\n\n#szene[Abend im Garten.\n\nDie Amsel singt.]\n\n\
                - Lauch\n\n#quote(block: true)[Geduld.]\n\n#table(columns: 1, [Rübe])\n\n#unbekannt(wert: \"Sellerie\")\n";
    let result = read(body);
    let mut all = String::new();
    fn inlines(items: &[Inline], out: &mut String) {
        for item in items {
            match item {
                Inline::Text { text, .. } => out.push_str(text),
                Inline::Footnote { content } | Inline::Quote { content } | Inline::Link { content, .. } => {
                    inlines(content, out)
                }
                Inline::Typst { raw, shown } => {
                    out.push_str(raw);
                    inlines(shown, out);
                }
                _ => {}
            }
            out.push(' ');
        }
    }
    fn blocks(items: &[Block], out: &mut String) {
        for item in items {
            match item {
                Block::Part { content, .. } | Block::Heading { content, .. } | Block::Paragraph { content, .. } => {
                    inlines(content, out)
                }
                Block::Quote { blocks: inner, attribution } => {
                    blocks(inner, out);
                    inlines(attribution, out);
                }
                Block::List { items, .. } => items.iter().for_each(|i| inlines(&i.content, out)),
                Block::Table { rows, .. } => rows.iter().flatten().for_each(|cell| blocks(cell, out)),
                Block::Figure { caption, raw, .. } => {
                    out.push_str(raw);
                    inlines(caption, out);
                }
                Block::Typst { raw, shown } => {
                    out.push_str(raw);
                    blocks(shown, out);
                }
                Block::PageBreak => {}
            }
        }
    }
    blocks(&result.blocks, &mut all);
    for word in
        ["Ernte", "Kürbis", "Mangold", "Winterhart", "wachsen", "Abend", "Amsel", "Lauch", "Geduld", "Rübe", "Sellerie"]
    {
        assert!(
            all.contains(word),
            "„{word}“ aus dem Abschnitt fehlt im Ergebnis – nichts darf still verloren gehen: {all}"
        );
    }
    let scene = result.blocks.iter().find_map(|b| match b {
        Block::Typst { raw, shown } if raw.starts_with("#szene") => Some(shown),
        _ => None,
    });
    assert_eq!(
        scene.map(Vec::len),
        Some(2),
        "eine Szene bleibt Insel (ihre Gestaltung kennt Word nicht), zeigt aber ihre beiden Absätze lesbar"
    );
}
