// SPDX-License-Identifier: AGPL-3.0-or-later
//! MCP tool `scripture_map` (Round 17a): convert a reference from one verse numbering into another,
//! read-only. The same answer as RPC `scripture.map` ([`crate::scripture_map`]). The entry in the tool list
//! and the call are set by `tools.rs`.

use serde_json::{Value, json};

use super::tools::Failure;
use crate::AppState;

pub const NAME: &str = "scripture_map";

pub const DESCRIPTION: &str = "Convert a Bible reference between versifications, read-only: ref (e.g. \"Joel 2,28–32\", \
\"Mal 4,1–6\", \"Ps 51,1\"; book names German or OSIS) is read and checked in the versification from (default KJV) and \
mapped into the versification to (default German, the versification of the reference check). Versifications: all 18 \
of SWORD (German, KJV, KJVA, Luther, NRSV, NRSVA, MT, Leningrad, Catholic, Catholic2, Calvin, DarbyFr, LXX, Orthodox, \
Segond, Synodal, SynodalProt, Vulg), case does not matter. Answer: from, to, refs (per input, normalized_from, status \
same|moved|unmapped|invalid, target in German notation or null, verses [{book, chapter, verse}], rules, message) and \
versifications. Mapping uses only the checked rule table (psalm titles, chapter boundaries like Joel 2/3, Mal 3/4); \
what no rule covers and differs in chapter length is unmapped with a reason (message starting \"Versifikation passt \
nicht\") – never guessed. Unknown versification → error with the list.";

pub fn schema() -> Value {
    json!({ "type": "object", "required": ["ref"], "properties": {
        "ref": { "type": "string", "minLength": 1, "maxLength": 500, "description": "reference(s), several with \";\"" },
        "from": { "type": "string", "description": "versification of the reference, default KJV" },
        "to": { "type": "string", "description": "target versification, default German" } } })
}

pub async fn call(_state: &AppState, arguments: Value) -> Result<Value, Failure> {
    crate::scripture_map::map_json(&arguments).map_err(Failure::from)
}
