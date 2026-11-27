// SPDX-License-Identifier: AGPL-3.0-or-later
//! Runde 37 „Hilfe auf der Webseite“, Besitz hilfe_text: die Hilfeseiten unter `site/content/hilfe/` und der
//! Einstieg `docs/Bedienung.md`.
//!
//! Schnittstelle S1: nur die Übernahme der alten Adresse. Kein Server, kein Netz, kein Datenordner.

use std::path::PathBuf;

fn root() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
}

#[test]
fn the_help_section_takes_over_the_old_address() {
    let index = std::fs::read_to_string(root().join("site/content/hilfe/_index.md"))
        .unwrap_or_else(|e| panic!("site/content/hilfe/_index.md: {e}"));
    assert!(
        index.contains("aliases: [\"/bedienung/\"]"),
        "alte Verweise auf /bedienung/ müssen weiter ankommen: die Hilfe leitet die Adresse per aliases um"
    );
    assert!(
        !root().join("site/content/bedienung.md").exists(),
        "neben dem Alias gäbe eine Seite bedienung.md zwei Ausgaben auf demselben Pfad – die alte Seite ist gelöscht"
    );
}
