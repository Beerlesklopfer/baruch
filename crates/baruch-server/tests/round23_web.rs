// SPDX-License-Identifier: AGPL-3.0-or-later
//! Round 23 „Härtung“ (ownership web): message codes, reading requests of foreign pages (`guard::own_reads`), badges
//! without JavaScript, amending with a history step when changing through the tree, confirmation page with mode, cover
//! blocks, thought map, findings, versification of the SWORD evidence and the behaviour tests of the widgets in the
//! Node mock.
//!
//! Works freely invented; no Eliezer, no network (models only as a mock in the process). Every assertion with a reason.
//! The tests are by area in submodules under `tests/round23_web/*.rs`, included with
//! `#[path = "round23_web/x.rs"] mod x;` – `tests/main.rs` stays unchanged. The scenes of the widget tests are in
//! `tests/round23_web/widgets.mjs`.

#[path = "round23_web/befunde.rs"]
mod befunde;
#[path = "round23_web/fassungen.rs"]
mod fassungen;
#[path = "round23_web/schutz.rs"]
mod schutz;
#[path = "round23_web/umschlag_karte.rs"]
mod umschlag_karte;
#[path = "round23_web/widgets.rs"]
mod widgets;
#[path = "round23_web/zaehlung.rs"]
mod zaehlung;
