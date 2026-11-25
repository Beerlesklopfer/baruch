// SPDX-License-Identifier: AGPL-3.0-or-later
//! Typst eines Abschnitts → Blöcke des Modells (Aufgabe docx_export, Vertrag Runde 32 §3), über
//! `baruch_wysiwyg::parse`. Nichts geht still verloren: Was nur als Insel mitgeht, zählt eine Warnung `docx_insel`.

use crate::{Block, DocxWarning};

/// Ergebnis: Blöcke und Warnungen.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FromTypst {
    pub blocks: Vec<Block>,
    pub warnings: Vec<DocxWarning>,
}

/// Liest einen Abschnitt samt Überschriftszeile (`body`). Der Index eines Pfads in `image_paths` ist der Index des
/// Bildes in `DocxDocument::images`.
pub fn from_typst(body: &str, image_paths: &[String]) -> FromTypst {
    // STUB: Abbildung von Typst fehlt – keine Blöcke, nur die ehrliche Warnung (R32 docx_export)
    let _ = (body, image_paths);
    FromTypst {
        blocks: Vec::new(),
        warnings: vec![DocxWarning {
            code: "docx_nicht_umgesetzt".into(),
            message: "Die Umwandlung von Typst nach Word ist noch nicht umgesetzt (R32 docx_export).".into(),
        }],
    }
}
