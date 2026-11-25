// SPDX-License-Identifier: AGPL-3.0-or-later
//! Word-Dateien (.docx, OOXML WordprocessingML) lesen und schreiben (Runde 32, Schnittstelle R32-S1).
//!
//! Rein: keine Datenbank, kein Netz. Drehpunkt ist das Blockmodell in [`model`] ([`DocxDocument`]): Der Leser
//! ([`read`]) macht aus einer Datei das Modell samt Änderungsverfolgung und Kommentaren, [`resolve`] löst die
//! Änderungen zu einem Stand auf, [`to_typst`] schreibt daraus Typst je Abschnitt. In der Gegenrichtung liest
//! [`from_typst`] einen Abschnitt in Blöcke, [`diff`] legt einen Vorschlag als Änderungsverfolgung über die aktuelle
//! Fassung, und [`write`] schreibt das Modell als .docx.
//!
//! Besitz (Vertrag Runde 32, §5): `lib.rs`, [`model`], [`error`], [`limits`], [`ooxml`] und [`sample`] sind die
//! Schnittstelle und gehören dem Integrator (nach S1 eingefroren); [`read`], [`resolve`], [`to_typst`] gehören der
//! Aufgabe docx_import, [`write`], [`from_typst`], [`diff`] der Aufgabe docx_export.

pub mod diff;
pub mod error;
pub mod from_typst;
pub mod limits;
pub mod model;
pub mod ooxml;
pub mod read;
pub mod resolve;
pub mod sample;
pub mod to_typst;
pub mod write;

pub use error::DocxError;
pub use limits::Limits;
pub use model::*;
