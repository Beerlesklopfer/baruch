// SPDX-License-Identifier: AGPL-3.0-or-later
//! Leser: .docx → [`DocxDocument`] samt Änderungsverfolgung, Kommentaren und Bildern (Aufgabe docx_import, Vertrag
//! Runde 32 §2). Untermodule unter `src/read/` deklariert der Besitzer hier.
//!
//! Zusagen (Vertrag): ZIP nur innerhalb der [`Limits`] (den Größen im Kopf wird nicht getraut), keine
//! Entity-Auflösung, Ziele von Beziehungen verlassen den Ordner nicht ([`DocxError::Unsafe`]), externe Bilder
//! werden nie abgerufen; was nicht übernommen wird, steht als Warnung im Ergebnis.

use crate::{DocxDocument, DocxError, DocxWarning, Limits};

/// Ergebnis des Lesers: das Dokument und alles, was dabei auffiel.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ReadDocx {
    pub document: DocxDocument,
    pub warnings: Vec<DocxWarning>,
}

/// Liest eine .docx-Datei aus `bytes`.
pub fn read(bytes: &[u8], limits: &Limits) -> Result<ReadDocx, DocxError> {
    // STUB: Leser fehlt ganz – ZIP mit Grenzen, styles/numbering/footnotes/comments, Änderungsverfolgung, Bilder
    // (R32 docx_import)
    let _ = (bytes, limits);
    Err(DocxError::Invalid("Das Lesen von Word-Dateien ist noch nicht umgesetzt (R32 docx_import).".into()))
}
