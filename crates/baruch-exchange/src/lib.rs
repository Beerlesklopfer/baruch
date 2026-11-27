// SPDX-License-Identifier: AGPL-3.0-or-later
//! Exchange format between Baruch instances (Round 26, contract sections 1–4; documentation
//! `docs/Austauschformat.md`).
//!
//! - Container `.baruch` (ZIP): `mimetype` (STORED, `application/vnd.baruch.werk+zip`), `werk.xml`,
//!   `bilder/<fassungs-uuid>.<ext>`, last `MANIFEST.xml` with SHA-256 and byte count per entry.
//! - "XML only" `.baruch.xml`: the same `werk.xml`, images as Base64 in `<daten>`.
//! - Namespace [`NAMESPACE`] (`urn:baruch:werk:1`), sub-version [`FORMAT_VERSION`]; XSD under
//!   `docs/schema/baruch-werk-1-u7.xsd` (sub-version 7 with own indexes and glossary, Round 31;
//!   `baruch-werk-1-u6.xsd` stays for 6 with typesetting settings `<satz>`, `baruch-werk-1-u5.xsd` for 5 with
//!   solutions, variants and post metadata, `baruch-werk-1-u4.xsd` for 4 with findings and old post versions,
//!   `baruch-werk-1-u3.xsd` for 3 with cover layout, addresses and typeset view, `baruch-werk-1-u2.xsd` for 2 with
//!   `<karte>`, `baruch-werk-1.xsd` for 1).
//!
//! Modules: [`model`] (types, frozen after S1), [`error`], [`limits`] (limits when reading), [`sample`]
//! (invented sample work for tests), [`write`] (writing, R26b), [`read`] (reading and checking, R26c) and
//! [`typeset`] (words of the typesetting settings, Round 29).
//! The crate knows no database; mapping onto the store is done by `baruch_store::exchange_dump` and
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
