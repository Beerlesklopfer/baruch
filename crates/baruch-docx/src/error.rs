// SPDX-License-Identifier: AGPL-3.0-or-later
//! Fehler beim Lesen und Schreiben einer Word-Datei (Schnittstelle R32-S1, eingefroren).
//!
//! Der Server bildet [`DocxError::Write`] auf einen internen Fehler ab, alles andere auf ungültige Parameter
//! (`impl From<DocxError> for RpcError` in `baruch-server/src/rpc.rs`).

/// Fehler dieses Crates; die Texte sind für den Menschen gedacht (Deutsch, Klartext).
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum DocxError {
    /// Kein ZIP, eine alte .doc-Datei (OLE) oder verschlüsselt.
    #[error("Keine Word-Datei (.docx): {0}")]
    NotDocx(String),
    /// Ziel einer Beziehung verlässt den Ordner, Pfad mit `..` und Ähnliches.
    #[error("Unsichere Word-Datei: {0}")]
    Unsafe(String),
    /// Eine Grenze aus [`crate::Limits`] ist überschritten.
    #[error("Word-Datei überschreitet eine Grenze: {0}")]
    Limit(String),
    /// Das ZIP-Archiv lässt sich nicht lesen.
    #[error("Archiv der Word-Datei: {0}")]
    Archive(String),
    /// Ein Teil ist kein lesbares XML.
    #[error("XML der Word-Datei ({part}): {message}")]
    Xml { part: String, message: String },
    /// Ein nötiger Teil fehlt oder passt nicht zusammen.
    #[error("Word-Datei unvollständig: {0}")]
    Invalid(String),
    /// Das Schreiben ist gescheitert.
    #[error("Word-Datei schreiben: {0}")]
    Write(String),
}
