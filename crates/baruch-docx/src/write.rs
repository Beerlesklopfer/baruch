// SPDX-License-Identifier: AGPL-3.0-or-later
//! Schreiber: [`DocxDocument`] → .docx (Aufgabe docx_export, Vertrag Runde 32 §3). Untermodule unter `src/write/`
//! deklariert der Besitzer hier.
//!
//! Zusagen (Vertrag): gültiges OOXML nach [`crate::ooxml`], deterministisch (feste ZIP-Zeiten, feste Reihenfolge:
//! gleiche Eingabe → gleiche Bytes), XML-verbotene Zeichen werden ersetzt statt ein kaputtes Dokument zu schreiben.

use crate::{DocxDocument, DocxError};

/// Angaben, die nicht aus dem Dokument kommen.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WriteOptions {
    /// Zeitpunkt für `docProps/core.xml` (ISO 8601 UTC); leer = ohne Angabe.
    pub created: String,
    /// Name des erzeugenden Programms für `docProps/app.xml` (z. B. „Baruch 0.9.0“).
    pub generator: String,
}

/// Schreibt das Dokument als .docx.
pub fn write(document: &DocxDocument, options: &WriteOptions) -> Result<Vec<u8>, DocxError> {
    // STUB: Schreiber fehlt ganz – Archiv, styles/numbering/footnotes/comments, Bilder, Änderungen (R32 docx_export)
    let _ = (document, options);
    Err(DocxError::Invalid("Das Schreiben von Word-Dateien ist noch nicht umgesetzt (R32 docx_export).".into()))
}
