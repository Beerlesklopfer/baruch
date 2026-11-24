// SPDX-License-Identifier: AGPL-3.0-or-later
//! MCP-Werkzeug `scripture_map` (Runde 17a): eine Stellenangabe von einer Verszählung in eine andere umrechnen,
//! nur lesend. Dieselbe Antwort wie RPC `scripture.map` ([`crate::scripture_map`]). Eintrag in der Werkzeugliste
//! und Aufruf setzt `tools.rs`.

use serde_json::{Value, json};

use super::tools::Failure;
use crate::AppState;

pub const NAME: &str = "scripture_map";

pub const DESCRIPTION: &str = "Bibelstelle zwischen Verszählungen umrechnen, nur lesend: ref (z. B. „Joel 2,28–32“, \
„Mal 4,1–6“, „Ps 51,1“; Buchnamen deutsch oder OSIS) wird in der Zählung from gelesen und geprüft (Vorgabe KJV) und \
in die Zählung to abgebildet (Vorgabe German, die Zählung der Stellenprüfung). Zählungen: alle 18 von SWORD (German, \
KJV, KJVA, Luther, NRSV, NRSVA, MT, Leningrad, Catholic, Catholic2, Calvin, DarbyFr, LXX, Orthodox, Segond, Synodal, \
SynodalProt, Vulg), Groß-/Kleinschreibung egal. Antwort: from, to, refs (je input, normalized_from, status \
same|moved|unmapped|invalid, target in deutscher Schreibweise oder null, verses [{book, chapter, verse}], rules, \
message) und versifications. Abgebildet wird nur über die geprüfte Regeltabelle (Psalmüberschriften, \
Kapitelgrenzen wie Joel 2/3, Mal 3/4); was keine Regel deckt und in der Kapitellänge abweicht, ist unmapped mit \
Grund („Versifikation passt nicht: …“) – nie geraten. Unbekannte Zählung → Fehler mit der Liste.";

pub fn schema() -> Value {
    json!({ "type": "object", "required": ["ref"], "properties": {
        "ref": { "type": "string", "minLength": 1, "maxLength": 500, "description": "Stellenangabe(n), mehrere mit „;“" },
        "from": { "type": "string", "description": "Zählung der Angabe, Vorgabe KJV" },
        "to": { "type": "string", "description": "Zielzählung, Vorgabe German" } } })
}

pub async fn call(_state: &AppState, arguments: Value) -> Result<Value, Failure> {
    crate::scripture_map::map_json(&arguments).map_err(Failure::from)
}
