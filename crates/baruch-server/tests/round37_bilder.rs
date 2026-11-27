// SPDX-License-Identifier: AGPL-3.0-or-later
//! Runde 37 „Hilfe auf der Webseite“, Besitz bilder: Hilfe-Link in der Oberfläche und Bilder der Hilfe.
//!
//! Schnittstelle S1: nur die Adresse der Hilfe. Kein Server, kein Netz, kein Datenordner.

use baruch_server::about;

#[test]
fn the_help_address_is_the_help_section_of_the_homepage() {
    assert_eq!(
        about::HELP_URL,
        format!("{}hilfe/", about::HOMEPAGE),
        "der Eintrag „Hilfe“ führt auf den Abschnitt /hilfe/ der Webseite – zieht die Webseite um, zieht die Hilfe mit"
    );
}
