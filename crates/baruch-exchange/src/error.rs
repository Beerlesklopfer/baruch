// SPDX-License-Identifier: AGPL-3.0-or-later
//! Errors of the exchange format (contract R26, S1 (5)). On every error nothing is written.

#[derive(Debug, thiserror::Error)]
pub enum ExchangeError {
    /// Neither a ZIP with matching `mimetype` nor XML with `<werk>` or `<manifest>`.
    #[error("Keine Austauschdatei: {0}")]
    NotExchange(String),
    /// Wrong namespace or unreadable version.
    #[error("Version {found} wird nicht unterstützt (Namensraum {namespace})")]
    Version { found: String, namespace: String },
    /// DTD, processing instruction, unknown entity.
    #[error("Unsicheres XML: {0}")]
    Unsafe(String),
    /// A limit from [`crate::Limits`] is exceeded.
    #[error("Grenze überschritten: {0}")]
    Limit(String),
    /// ZIP error: name, order, link, encryption, compression, manifest.
    #[error("Archiv: {0}")]
    Archive(String),
    #[error("Prüfsumme stimmt nicht: {path}")]
    Checksum { path: String },
    /// XML not well-formed or a mandatory element is missing.
    #[error("XML: {0}")]
    Xml(String),
    /// Invalid in content (UUIDs, duplicates, references, current revision).
    #[error("Ungültig: {0}")]
    Invalid(String),
    /// On writing, e.g. a forbidden character in an attribute (with the name of the field).
    #[error("Ausgabe: {0}")]
    Write(String),
}
