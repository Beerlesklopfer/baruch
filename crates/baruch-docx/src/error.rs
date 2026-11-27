// SPDX-License-Identifier: AGPL-3.0-or-later
//! Errors when reading and writing a Word file (interface R32-S1, frozen).
//!
//! The server maps [`DocxError::Write`] to an internal error, everything else to invalid parameters
//! (`impl From<DocxError> for RpcError` in `baruch-server/src/rpc.rs`).

/// Errors of this crate; the texts are meant for the human (German, plain language).
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum DocxError {
    /// Not a ZIP, an old .doc file (OLE) or encrypted.
    #[error("Keine Word-Datei (.docx): {0}")]
    NotDocx(String),
    /// Target of a relationship leaves the folder, path with `..` and the like.
    #[error("Unsichere Word-Datei: {0}")]
    Unsafe(String),
    /// A limit from [`crate::Limits`] is exceeded.
    #[error("Word-Datei überschreitet eine Grenze: {0}")]
    Limit(String),
    /// The ZIP archive cannot be read.
    #[error("Archiv der Word-Datei: {0}")]
    Archive(String),
    /// A part is not readable XML.
    #[error("XML der Word-Datei ({part}): {message}")]
    Xml { part: String, message: String },
    /// A required part is missing or does not fit together.
    #[error("Word-Datei unvollständig: {0}")]
    Invalid(String),
    /// Writing failed.
    #[error("Word-Datei schreiben: {0}")]
    Write(String),
}
