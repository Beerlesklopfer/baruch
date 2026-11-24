// SPDX-License-Identifier: AGPL-3.0-or-later
//! Fehler des Austauschformats (Vertrag R26, S1 (5)). Bei jedem Fehler wird nichts geschrieben.

#[derive(Debug, thiserror::Error)]
pub enum ExchangeError {
    /// Weder ZIP mit passendem `mimetype` noch XML mit `<werk>` bzw. `<manifest>`.
    #[error("Keine Austauschdatei: {0}")]
    NotExchange(String),
    /// Falscher Namensraum oder unlesbare Versionsangabe.
    #[error("Version {found} wird nicht unterstützt (Namensraum {namespace})")]
    Version { found: String, namespace: String },
    /// DTD, Verarbeitungsanweisung, unbekannte Entität.
    #[error("Unsicheres XML: {0}")]
    Unsafe(String),
    /// Eine Grenze aus [`crate::Limits`] ist überschritten.
    #[error("Grenze überschritten: {0}")]
    Limit(String),
    /// ZIP-Fehler: Name, Reihenfolge, Verknüpfung, Verschlüsselung, Kompression, Manifest.
    #[error("Archiv: {0}")]
    Archive(String),
    #[error("Prüfsumme stimmt nicht: {path}")]
    Checksum { path: String },
    /// XML nicht wohlgeformt oder Pflichtelement fehlt.
    #[error("XML: {0}")]
    Xml(String),
    /// Inhaltlich ungültig (UUIDs, Doppel, Verweise, aktuelle Fassung).
    #[error("Ungültig: {0}")]
    Invalid(String),
    /// Beim Schreiben, z. B. ein verbotenes Zeichen in einem Attribut (mit dem Namen des Felds).
    #[error("Ausgabe: {0}")]
    Write(String),
}
