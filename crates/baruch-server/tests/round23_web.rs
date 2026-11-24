// SPDX-License-Identifier: AGPL-3.0-or-later
//! Runde 23 „Härtung“ (Besitz web): Meldecodes, lesende Anfragen fremder Seiten (`guard::own_reads`), Abzeichen ohne
//! JavaScript, Fortschreiben mit Verlaufsschritt beim Ändern über den Baum, Rückfrageseite mit Modus, Umschlagblöcke,
//! Gedankenkarte, Befunde, Verszählung der SWORD-Belege und die Verhaltenstests der Widgets im Node-Nachbau.
//!
//! Werke frei erfunden; kein Eliezer, kein Netz (Modelle nur als Nachbau im Prozess). Jede Zusicherung mit
//! Begründung. Die Tests stehen nach Gebiet in Untermodulen unter `tests/round23_web/*.rs`, eingebunden mit
//! `#[path = "round23_web/x.rs"] mod x;` – `tests/main.rs` bleibt unverändert. Die Szenen der Widget-Tests stehen in
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
