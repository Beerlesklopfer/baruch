// SPDX-License-Identifier: AGPL-3.0-or-later
//! Austauschformat zwischen Baruch-Instanzen (Runde 26, Vertrag Abschnitte 1–4; Doku `docs/Austauschformat.md`).
//!
//! - Container `.baruch` (ZIP): `mimetype` (STORED, `application/vnd.baruch.werk+zip`), `werk.xml`,
//!   `bilder/<fassungs-uuid>.<ext>`, zuletzt `MANIFEST.xml` mit SHA-256 und Bytezahl je Eintrag.
//! - „Nur XML“ `.baruch.xml`: dasselbe `werk.xml`, Bilder als Base64 in `<daten>`.
//! - Namensraum [`NAMESPACE`] (`urn:baruch:werk:1`), Unterversion [`FORMAT_VERSION`]; XSD unter
//!   `docs/schema/baruch-werk-1-u7.xsd` (Unterversion 7 mit eigenen Registern und Glossar, Runde 31;
//!   `baruch-werk-1-u6.xsd` bleibt für 6 mit Satz-Einstellungen `<satz>`, `baruch-werk-1-u5.xsd` für 5 mit Lösungen, Varianten und Beitragsmetadaten, `baruch-werk-1-u4.xsd` für 4
//!   mit Befunden und alten Beitragsfassungen, `baruch-werk-1-u3.xsd` für 3 mit Umschlag-Anordnung, Anschriften und
//!   Satzbild, `baruch-werk-1-u2.xsd` für 2 mit `<karte>`, `baruch-werk-1.xsd` für 1).
//!
//! Module: [`model`] (Typen, eingefroren nach S1), [`error`], [`limits`] (Grenzen beim Lesen), [`sample`]
//! (erfundenes Beispielwerk für Tests), [`write`] (Schreiben, R26b), [`read`] (Lesen und Prüfen, R26c) und
//! [`typeset`] (Wörter der Satz-Einstellungen, Runde 29).
//! Die Crate kennt keine Datenbank; das Abbilden auf den Store machen `baruch_store::exchange_dump` und
//! `baruch_store::exchange_apply`.

pub mod error;
pub mod limits;
pub mod model;
pub mod read;
pub mod sample;
pub mod typeset;
pub mod write;

pub use error::ExchangeError;
pub use limits::Limits;
pub use model::*;
