// SPDX-License-Identifier: AGPL-3.0-or-later
//! Read and write Word files (.docx, OOXML WordprocessingML) (Round 32, interface R32-S1).
//!
//! Pure: no database, no network. The pivot is the block model in [`model`] ([`DocxDocument`]): the reader
//! ([`read`]) turns a file into the model including tracked changes and comments, [`resolve`] resolves the
//! changes into one state, [`to_typst`] writes Typst per section from it. In the opposite direction
//! [`from_typst`] reads a section into blocks, [`diff`] lays a proposal as tracked changes over the current
//! revision, and [`write`] writes the model as .docx.
//!
//! Ownership (contract Round 32, §5): `lib.rs`, [`model`], [`error`], [`limits`], [`ooxml`] and [`sample`] are the
//! interface and belong to the integrator (frozen after S1); [`read`], [`resolve`], [`to_typst`] belong to the
//! task docx_import, [`write`], [`from_typst`], [`diff`] to the task docx_export.

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
