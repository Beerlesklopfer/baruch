// SPDX-License-Identifier: AGPL-3.0-or-later
//! [`DocxDocument`] → Typst je Abschnitt (Aufgabe docx_import, Vertrag Runde 32 §2); bevorzugt über das
//! Blockmodell von `baruch-wysiwyg`, damit Maskierung und Rundreise zum Satzbild passen.

use crate::{DocxDocument, DocxWarning};

/// Anker eines Kommentars im Abschnitt: der kommentierte Wortlaut.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommentAnchor {
    pub comment: u32,
    pub quote: String,
}

/// Ein Abschnitt des Werks: `level` 0 = Teil, 1–3 = Gliederung; `body` ohne die Überschriftszeile.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TypstSection {
    pub level: u8,
    pub numbered: bool,
    pub title: String,
    pub body: String,
    pub anchors: Vec<CommentAnchor>,
}

/// Ergebnis: Abschnitte in Lesereihenfolge und Warnungen.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TypstOut {
    pub sections: Vec<TypstSection>,
    pub warnings: Vec<DocxWarning>,
}

/// Schreibt ein aufgelöstes Dokument (ohne [`crate::Inline::Tracked`]) als Typst je Abschnitt.
pub fn to_typst(document: &DocxDocument) -> TypstOut {
    // STUB: Typst-Ausgabe fehlt – keine Abschnitte, nur die ehrliche Warnung (R32 docx_import)
    let _ = document;
    TypstOut {
        sections: Vec::new(),
        warnings: vec![DocxWarning {
            code: "docx_nicht_umgesetzt".into(),
            message: "Die Umwandlung von Word nach Typst ist noch nicht umgesetzt (R32 docx_import).".into(),
        }],
    }
}
