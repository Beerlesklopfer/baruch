// SPDX-License-Identifier: AGPL-3.0-or-later
//! Rundreise im Crate (Runde 32, Integrator): Was der Schreiber aus dem gemeinsamen Beispiel schreibt, liest der
//! Leser wieder als dasselbe Modell. Beide Seiten sind getrennt entstanden und kennen nur `model.rs` und `ooxml.rs`;
//! dieser Test ist ihre Nahtstelle.

use baruch_docx::read::read;
use baruch_docx::sample::sample_document;
use baruch_docx::write::{WriteOptions, write};
use baruch_docx::{Block, DocxDocument, DocxImage, Inline, Limits, ListItem, Marks};

fn options() -> WriteOptions {
    WriteOptions { created: "2026-11-25T08:00:00Z".to_owned(), generator: "Baruch Test".to_owned() }
}

fn there_and_back(document: &DocxDocument) -> baruch_docx::read::ReadDocx {
    let bytes = write(document, &options()).expect("der Schreiber schreibt das Dokument");
    read(&bytes, &Limits::DEFAULT).expect("der Leser liest die Datei des Schreibers")
}

fn text(t: &str) -> Inline {
    Inline::Text { text: t.to_owned(), marks: Marks::default() }
}

#[test]
fn the_sample_document_comes_back_as_the_same_model() {
    let sample = sample_document();
    let back = there_and_back(&sample);
    assert_eq!(back.warnings, Vec::new(), "die eigene Datei braucht beim Lesen keinen Hinweis");
    assert_eq!(back.document.meta, sample.meta, "Titel, Untertitel, Autor, Sprache und Impressum kommen zurück");
    assert_eq!(
        back.document.blocks, sample.blocks,
        "die Blöcke kommen unverändert zurück: Überschriften, Fußnote, Kommentarbereich, Löschung und Einfügung mit \
         Bearbeiter und Datum, Blockzitat mit Quelle, Abbildung mit Alternativtext und Unterschrift, Liste mit Ebenen"
    );
    assert_eq!(back.document.comments, sample.comments, "der Kommentar kommt mit Name, Kürzel, Datum und Text zurück");
    assert_eq!(
        back.document.images.iter().map(|i| (&i.path, &i.media_type, &i.bytes)).collect::<Vec<_>>(),
        sample.images.iter().map(|i| (&i.path, &i.media_type, &i.bytes)).collect::<Vec<_>>(),
        "das Bild kommt mit seinem Pfad im Werk (aus wp:docPr name), Medientyp und denselben Bytes zurück"
    );
}

#[test]
fn islands_ordered_lists_tables_and_marks_come_back() {
    let all = Marks { bold: true, italic: true, ..Marks::default() };
    let figure_raw = "#figure(image(\"karten/ufer.png\", width: 60%), caption: [Das Ufer]) <ufer>";
    let document = DocxDocument {
        blocks: vec![
            Block::Part { content: vec![text("Erster Teil")], mark: None },
            Block::Heading { level: 2, numbered: false, content: vec![text("Vorrede")], mark: None },
            Block::Paragraph {
                content: vec![
                    Inline::Text { text: "Beides".to_owned(), marks: all },
                    text(" und "),
                    Inline::Quote { content: vec![text("ein Zitat im Text")] },
                    text(", dazu "),
                    Inline::Link { url: "https://example.org/weiher".to_owned(), content: vec![text("ein Verweis")] },
                    text(" und "),
                    Inline::Typst { raw: "#rahmen[kurz]".to_owned(), shown: vec![text("kurz")] },
                    text("."),
                    Inline::LineBreak,
                    text("Neue Zeile."),
                ],
                mark: None,
            },
            Block::List {
                ordered: true,
                start: 1,
                items: vec![ListItem { depth: 0, content: vec![text("eins")], mark: None }],
            },
            Block::Paragraph { content: vec![text("Dazwischen.")], mark: None },
            Block::List {
                ordered: true,
                start: 4,
                items: vec![
                    ListItem { depth: 0, content: vec![text("vier")], mark: None },
                    ListItem { depth: 0, content: vec![text("fünf")], mark: None },
                ],
            },
            Block::Table {
                header_rows: 1,
                rows: vec![
                    vec![
                        vec![Block::Paragraph { content: vec![text("Ort")], mark: None }],
                        vec![Block::Paragraph { content: vec![text("Tiefe")], mark: None }],
                    ],
                    vec![
                        vec![Block::Paragraph { content: vec![text("Weiher")], mark: None }],
                        vec![Block::Paragraph { content: vec![text("2 m")], mark: None }],
                    ],
                ],
            },
            Block::Figure {
                image: 0,
                alt: "Ein Ufer".to_owned(),
                caption: vec![text("Das Ufer")],
                raw: figure_raw.to_owned(),
            },
            Block::Typst {
                raw: "#let rahmen(inhalt) = box(stroke: 1pt)[#inhalt]\n#set par(justify: true)".to_owned(),
                shown: vec![Block::Paragraph { content: vec![text("(Einstellung)")], mark: None }],
            },
            Block::PageBreak,
            Block::Paragraph { content: vec![text("Schluss.")], mark: None },
        ],
        images: vec![DocxImage {
            path: "karten/ufer.png".to_owned(),
            file_name: "ufer.png".to_owned(),
            media_type: "image/png".to_owned(),
            bytes: baruch_docx::sample::SAMPLE_PNG.to_vec(),
        }],
        ..DocxDocument::default()
    };
    let back = there_and_back(&document);
    assert_eq!(back.warnings, Vec::new(), "die eigene Datei braucht beim Lesen keinen Hinweis");
    // Vom Sichtbaren einer Insel behält der Leser nichts (es steckt im Quelltext); eine Abbildung mit Quelltext ist
    // für ihn eine Insel.
    let expected: Vec<Block> = document
        .blocks
        .iter()
        .map(|block| match block {
            Block::Figure { raw, .. } if !raw.is_empty() => Block::Typst { raw: raw.clone(), shown: Vec::new() },
            Block::Typst { raw, .. } => Block::Typst { raw: raw.clone(), shown: Vec::new() },
            Block::Paragraph { content, mark } => Block::Paragraph {
                content: content
                    .iter()
                    .map(|inline| match inline {
                        Inline::Typst { raw, .. } => Inline::Typst { raw: raw.clone(), shown: Vec::new() },
                        other => other.clone(),
                    })
                    .collect(),
                mark: mark.clone(),
            },
            other => other.clone(),
        })
        .collect();
    assert_eq!(
        back.document.blocks, expected,
        "Teil, Überschrift ohne Nummer, Zeichenformate, Zitat im Text, Verweis, Zeilenumbruch, zwei getrennte \
         Aufzählungen (die zweite ab 4), Tabelle mit Kopfzeile, Seitenumbruch und die Inseln mit wörtlichem Quelltext \
         kommen zurück"
    );
    assert_eq!(
        back.document.images.iter().map(|i| i.path.as_str()).collect::<Vec<_>>(),
        vec!["karten/ufer.png"],
        "auch das Bild einer Abbildung, die als Insel in der Datei steht, kommt unter seinem Pfad ins Werk – ihr \
         Quelltext nennt es"
    );
}
