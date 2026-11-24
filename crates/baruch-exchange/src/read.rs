// SPDX-License-Identifier: AGPL-3.0-or-later
//! Lesen und Prüfen einer Austauschdatei (Vertrag R26 F2/F7; Doku `docs/Austauschformat.md` §5).
//!
//! - [`read`] unterscheidet am Anfang: `PK\x03\x04` → ZIP ([`read_baruch`]), sonst XML ([`read_xml`]).
//! - XML ([`dom`]): keine DTD, keine Verarbeitungsanweisung außer der Deklaration, nur die fünf vordefinierten
//!   Entitäten und Zeichenreferenzen, Grenzen aus [`Limits`]; der Leser trimmt nie.
//! - Abbilden ([`map`]): Pflichtelemente und -attribute, unbekannte Elemente und Attribute als Pfad in
//!   [`ExchangeDocument::unknown`]; eine höhere Unterversion (`version` > [`crate::FORMAT_VERSION`]) wird
//!   gelesen, ein anderer Namensraum abgelehnt.
//! - ZIP ([`archive`]): erster Eintrag `mimetype`, nur bekannte Namen, keine Pfade nach außen, keine
//!   Verknüpfung oder Verschlüsselung, nur STORED oder DEFLATE, Entpacken mit `take(Grenze + 1)`, Manifest und
//!   Prüfsummen.
//! - [`validate`] ([`check`]): UUIDs, Doppel, Verweise, genau eine aktuelle Fassung je Punkt, `basis` im selben
//!   Punkt. [`read`], [`read_xml`] und [`read_baruch`] prüfen jedes Dokument damit, bevor sie es zurückgeben.

mod archive;
mod check;
mod dom;
mod map;

use crate::{ExchangeDocument, ExchangeError, ExchangeManifestEntry, Limits};

/// Austauschdatei lesen (ZIP oder XML, am Anfang erkannt).
pub fn read(bytes: &[u8], limits: &Limits) -> Result<ExchangeDocument, ExchangeError> {
    if bytes.len() as u64 > limits.max_file_bytes {
        return Err(ExchangeError::Limit(format!("Datei größer als {} Bytes", limits.max_file_bytes)));
    }
    if bytes.starts_with(b"PK\x03\x04") { read_baruch(bytes, limits) } else { read_xml(bytes, limits) }
}

/// `werk.xml` bzw. `.baruch.xml` lesen. Bilder müssen hier in `<daten>` stehen (ein Verweis `datei` geht nur im
/// Container).
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

/// Container `.baruch` lesen und prüfen.
pub fn read_baruch(zip: &[u8], limits: &Limits) -> Result<ExchangeDocument, ExchangeError> {
    archive::read(zip, limits)
}

/// `MANIFEST.xml` lesen.
pub fn read_manifest(xml: &[u8], limits: &Limits) -> Result<Vec<ExchangeManifestEntry>, ExchangeError> {
    if xml.len() as u64 > limits.max_xml_bytes {
        return Err(ExchangeError::Limit(format!("Manifest größer als {} Bytes", limits.max_xml_bytes)));
    }
    let root = dom::parse(xml, limits)?;
    map::manifest(&root)
}

/// Inhaltliche Prüfung eines gelesenen Dokuments.
pub fn validate(doc: &ExchangeDocument) -> Result<(), ExchangeError> {
    check::validate(doc)
}
