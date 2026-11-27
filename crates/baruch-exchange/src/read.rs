// SPDX-License-Identifier: AGPL-3.0-or-later
//! Reading and checking an exchange file (contract R26 F2/F7; documentation `docs/Austauschformat.md` §5).
//!
//! - [`read`] distinguishes at the beginning: `PK\x03\x04` → ZIP ([`read_baruch`]), otherwise XML ([`read_xml`]).
//! - XML ([`dom`]): no DTD, no processing instruction except the declaration, only the five predefined
//!   entities and character references, limits from [`Limits`]; the reader never trims.
//! - Mapping ([`map`]): mandatory elements and attributes, unknown elements and attributes as a path in
//!   [`ExchangeDocument::unknown`]; a higher sub-version (`version` > [`crate::FORMAT_VERSION`]) is
//!   read, another namespace rejected.
//! - ZIP ([`archive`]): first entry `mimetype`, only known names, no paths to the outside, no
//!   link or encryption, only STORED or DEFLATE, unpacking with `take(limit + 1)`, manifest and
//!   checksums.
//! - [`validate`] ([`check`]): UUIDs, duplicates, references, exactly one current revision per item, `basis` in
//!   the same item. [`read`], [`read_xml`] and [`read_baruch`] check every document with it before they return it.

mod archive;
mod check;
mod dom;
mod map;

use crate::{ExchangeDocument, ExchangeError, ExchangeManifestEntry, Limits};

/// Read an exchange file (ZIP or XML, recognised at the beginning).
pub fn read(bytes: &[u8], limits: &Limits) -> Result<ExchangeDocument, ExchangeError> {
    if bytes.len() as u64 > limits.max_file_bytes {
        return Err(ExchangeError::Limit(format!("Datei größer als {} Bytes", limits.max_file_bytes)));
    }
    if bytes.starts_with(b"PK\x03\x04") { read_baruch(bytes, limits) } else { read_xml(bytes, limits) }
}

/// Read `werk.xml` or `.baruch.xml`. Images must be in `<daten>` here (a reference `datei` only works in the
/// container).
pub fn read_xml(xml: &[u8], limits: &Limits) -> Result<ExchangeDocument, ExchangeError> {
    if xml.len() as u64 > limits.max_xml_bytes {
        return Err(ExchangeError::Limit(format!("XML größer als {} Bytes", limits.max_xml_bytes)));
    }
    let trimmed = xml.strip_prefix(b"\xef\xbb\xbf").unwrap_or(xml);
    let first = trimmed.iter().find(|b| !b.is_ascii_whitespace());
    if first != Some(&b'<') {
        return Err(ExchangeError::NotExchange("weder ZIP noch XML".into()));
    }
    let root = dom::parse(xml, limits)?;
    let (doc, refs) = map::document(&root)?;
    if let Some(first) = refs.first() {
        let revision = &doc.work.assets[first.asset].revisions[first.revision];
        return Err(ExchangeError::Invalid(format!(
            "Bildfassung {} verweist auf „{}“; in einer .baruch.xml stehen Bilder in <daten>",
            revision.uuid, first.entry
        )));
    }
    validate(&doc)?;
    Ok(doc)
}

/// Read and check the container `.baruch`.
pub fn read_baruch(zip: &[u8], limits: &Limits) -> Result<ExchangeDocument, ExchangeError> {
    archive::read(zip, limits)
}

/// Read `MANIFEST.xml`.
pub fn read_manifest(xml: &[u8], limits: &Limits) -> Result<Vec<ExchangeManifestEntry>, ExchangeError> {
    if xml.len() as u64 > limits.max_xml_bytes {
        return Err(ExchangeError::Limit(format!("Manifest größer als {} Bytes", limits.max_xml_bytes)));
    }
    let root = dom::parse(xml, limits)?;
    map::manifest(&root)
}

/// Content check of a read document.
pub fn validate(doc: &ExchangeDocument) -> Result<(), ExchangeError> {
    check::validate(doc)
}
