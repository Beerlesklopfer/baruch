// SPDX-License-Identifier: AGPL-3.0-or-later
//! MCP-Werkzeug `sources_list` (Runde 9): die bevorzugten Quellen lesen, damit ein Assistent weiß,
//! wo er für `@belege` zuerst nachsehen soll. Nur lesend; Verbindungen, Geheimnisse und Einstellungen
//! bleiben MCP verschlossen (kein `settings_read`, die Personas liefert `list_personas`).

use serde_json::{Value, json};

use crate::AppState;
use crate::mcp::tools::Failure;

/// Beschreibung in `tools/list`.
pub const DESCRIPTION: &str = "Bevorzugte Quellen für @belege lesen (nur lesend): Titel, Adresse, Notiz und \
                               Reihenfolge, wie der Mensch sie unter Einstellungen → Quellen pflegt.";

/// Eingabeschema: keine Argumente.
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
