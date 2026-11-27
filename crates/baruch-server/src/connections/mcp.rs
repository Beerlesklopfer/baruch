// SPDX-License-Identifier: AGPL-3.0-or-later
//! MCP tool `sources_list` (Round 9): read the preferred sources so that an assistant knows
//! where to look first for `@belege`. Read-only; connections, secrets and settings
//! stay closed to MCP (no `settings_read`, the personas come from `list_personas`).

use serde_json::{Value, json};

use crate::AppState;
use crate::mcp::tools::Failure;

/// Description in `tools/list`.
pub const DESCRIPTION: &str = "Bevorzugte Quellen für @belege lesen (nur lesend): Titel, Adresse, Notiz und \
                               Reihenfolge, wie der Mensch sie unter Einstellungen → Quellen pflegt.";

/// Input schema: no arguments.
pub fn schema() -> Value {
    json!({ "type": "object", "properties": {} })
}

/// `sources_list {}` → `{sources: [{id, title, url, note, position}]}`.
pub async fn sources_list(state: &AppState, _arguments: Value) -> Result<Value, Failure> {
    let sources: Vec<Value> = state
        .store
        .sources_list()
        .await?
        .into_iter()
        .map(|s| json!({ "id": s.id, "title": s.title, "url": s.url, "note": s.note, "position": s.position }))
        .collect();
    Ok(json!({ "sources": sources }))
}
