// SPDX-License-Identifier: AGPL-3.0-or-later
//! Vorschlag als Änderungsverfolgung über die aktuelle Fassung legen (Aufgabe docx_export, Vertrag Runde 32 §3).
//!
//! Zusage (Vertrag): Der Stand „Original“ des Ergebnisses entspricht `current`, der Stand „Final“ `proposed`.

use crate::{Block, Change};

/// Richtet die Blöcke von `current` und `proposed` aus und markiert die Unterschiede mit `change` (Bearbeiter und
/// Datum; die Art je Stelle setzt die Funktion selbst).
pub fn track_changes(current: &[Block], proposed: &[Block], change: &Change) -> Vec<Block> {
    // STUB: Vergleich fehlt – liefert keine Blöcke (R32 docx_export)
    let _ = (current, proposed, change);
    Vec::new()
}
