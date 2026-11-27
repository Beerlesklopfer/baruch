// SPDX-License-Identifier: AGPL-3.0-or-later
//! Freely invented sample document for tests of reader, writer and round trip (interface R32-S1; after S1 only
//! the integrator changes it, so that both tasks check against the same state).

use crate::model::{Block, Change, ChangeKind, Comment, DocxDocument, DocxImage, DocxMeta, Inline, ListItem, Marks};

/// Editor of the insertion, the deletion and the comment in the sample.
pub const SAMPLE_EDITOR: &str = "Erika Beispiel";
/// Time of the changes in the sample.
pub const SAMPLE_CHANGE_DATE: &str = "2026-11-20T09:30:00Z";

/// A valid PNG with a single pixel (70 bytes).
pub const SAMPLE_PNG: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52, 0x00, 0x00, 0x00,
    0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f, 0x15, 0xc4, 0x89, 0x00, 0x00, 0x00, 0x0d, 0x49,
    0x44, 0x41, 0x54, 0x78, 0xda, 0x63, 0x64, 0x60, 0xf8, 0x5f, 0x0f, 0x00, 0x02, 0x87, 0x01, 0x80, 0xeb, 0x47, 0xba,
    0x92, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
];

fn text(text: &str) -> Inline {
    Inline::Text { text: text.to_owned(), marks: Marks::default() }
}

fn tracked(kind: ChangeKind, content: &str) -> Inline {
    Inline::Tracked {
        change: Change { kind, author: SAMPLE_EDITOR.to_owned(), date: SAMPLE_CHANGE_DATE.to_owned() },
        content: vec![text(content)],
    }
}

/// The sample „Der Garten hinter der Mühle“: two chapters, a footnote, a block quote with source, a figure
/// (1×1 PNG), a list with two levels, a deletion („schmal“) and an insertion („breit“) by [`SAMPLE_EDITOR`] and
/// a comment on the wording „Der Zaun war grün gestrichen“.
pub fn sample_document() -> DocxDocument {
    let bold = Marks { bold: true, ..Marks::default() };
    let italic = Marks { italic: true, ..Marks::default() };
    DocxDocument {
        meta: DocxMeta {
            title: "Der Garten hinter der Mühle".to_owned(),
            subtitle: "Eine erfundene Erzählung".to_owned(),
            author: "Hanna Probst".to_owned(),
            lang: "de".to_owned(),
            imprint: vec!["© 2026 Hanna Probst".to_owned(), "Erfundenes Beispiel für die Tests von Baruch".to_owned()],
        },
        blocks: vec![
            Block::Heading { level: 1, numbered: true, content: vec![text("Der Mühlbach")], mark: None },
            Block::Paragraph {
                content: vec![
                    text("Hinter der Mühle lag ein Garten, den niemand mehr pflegte."),
                    Inline::Footnote { content: vec![text("Die Mühle und ihr Garten sind erfunden.")] },
                    text(" "),
                    Inline::CommentStart { id: 0 },
                    text("Der Zaun war grün gestrichen"),
                    Inline::CommentEnd { id: 0 },
                    text(", und der Bach dahinter war "),
                    tracked(ChangeKind::Delete, "schmal"),
                    tracked(ChangeKind::Insert, "breit"),
                    text(" und kalt."),
                ],
                mark: None,
            },
            Block::Quote {
                blocks: vec![Block::Paragraph { content: vec![text("Wer sät, braucht Geduld.")], mark: None }],
                attribution: vec![text("Spruch der Müllerin")],
            },
            Block::Figure {
                image: 0,
                alt: "Ein einzelner grauer Bildpunkt".to_owned(),
                caption: vec![text("Der Garten im Frühjahr")],
                raw: String::new(),
            },
            Block::Heading { level: 1, numbered: true, content: vec![text("Das Beet")], mark: None },
            Block::Paragraph {
                content: vec![
                    text("Im Beet wuchsen "),
                    Inline::Text { text: "Bohnen".to_owned(), marks: bold },
                    text(" und "),
                    Inline::Text { text: "Ringelblumen".to_owned(), marks: italic },
                    text("."),
                ],
                mark: None,
            },
            Block::List {
                ordered: false,
                start: 1,
                items: vec![
                    ListItem { depth: 0, content: vec![text("Bohnen stecken")], mark: None },
                    ListItem { depth: 1, content: vec![text("erst nach den Eisheiligen")], mark: None },
                    ListItem { depth: 0, content: vec![text("Ringelblumen säen")], mark: None },
                ],
            },
        ],
        comments: vec![Comment {
            id: 0,
            author: SAMPLE_EDITOR.to_owned(),
            initials: "EB".to_owned(),
            date: SAMPLE_CHANGE_DATE.to_owned(),
            text: "Ist die Farbe des Zauns für die Geschichte wichtig?".to_owned(),
            reply_to: None,
        }],
        images: vec![DocxImage {
            path: "bilder/garten.png".to_owned(),
            file_name: "garten.png".to_owned(),
            media_type: "image/png".to_owned(),
            bytes: SAMPLE_PNG.to_vec(),
        }],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inlines(document: &DocxDocument) -> Vec<&Inline> {
        document
            .blocks
            .iter()
            .filter_map(|block| match block {
                Block::Paragraph { content, .. } => Some(content.iter()),
                _ => None,
            })
            .flatten()
            .collect()
    }

    #[test]
    fn the_sample_holds_everything_the_contract_names() {
        let document = sample_document();
        assert_eq!(document.meta.title, "Der Garten hinter der Mühle", "Titel des Beispiels laut Vertrag R32 §1.2");
        let chapters = document.blocks.iter().filter(|b| matches!(b, Block::Heading { level: 1, .. })).count();
        assert_eq!(chapters, 2, "zwei Kapitel, damit die Zuordnung der Abschnitte geprüft werden kann");
        let inlines = inlines(&document);
        assert!(inlines.iter().any(|i| matches!(i, Inline::Footnote { .. })), "eine Fußnote steht inline im Absatz");
        assert!(document.blocks.iter().any(|b| matches!(b, Block::Quote { .. })), "ein Blockzitat mit Quelle");
        assert!(document.blocks.iter().any(|b| matches!(b, Block::List { .. })), "eine Liste");
        for kind in [ChangeKind::Insert, ChangeKind::Delete] {
            assert!(
                inlines.iter().any(
                    |i| matches!(i, Inline::Tracked { change, .. } if change.kind == kind && change.author == SAMPLE_EDITOR)
                ),
                "je eine Einfügung und eine Löschung von {SAMPLE_EDITOR}: {kind:?} fehlt"
            );
        }
        assert_eq!(document.comments.len(), 1, "genau ein Kommentar");
        let id = document.comments[0].id;
        assert!(
            inlines.iter().any(|i| matches!(i, Inline::CommentStart { id: start } if *start == id))
                && inlines.iter().any(|i| matches!(i, Inline::CommentEnd { id: end } if *end == id)),
            "der Kommentar ist im Text mit Anfang und Ende verankert, sonst ginge er als „ohne Anker“ durch"
        );
    }

    #[test]
    fn the_sample_image_is_a_real_png_and_every_reference_points_to_it() {
        let document = sample_document();
        assert_eq!(document.images.len(), 1, "ein Bild");
        let image = &document.images[0];
        assert!(image.bytes.starts_with(b"\x89PNG\r\n\x1a\n"), "die Bytes beginnen mit der PNG-Signatur");
        assert!(
            image.bytes.ends_with(&[0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82]),
            "und enden mit dem IEND-Block"
        );
        assert!(image.path.starts_with("bilder/"), "der Pfad im Werk liegt unter bilder/ wie bei jedem Import");
        for block in &document.blocks {
            if let Block::Figure { image, .. } = block {
                assert!(
                    *image < document.images.len(),
                    "jede Abbildung zeigt auf ein vorhandenes Bild (Index {image})"
                );
            }
        }
    }
}
