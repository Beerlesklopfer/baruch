// SPDX-License-Identifier: AGPL-3.0-or-later
//! MCP-Werkzeug `solution_variants` (Runde 30b, chat-loesungen): der Lektor (nach Einstellung) schreibt zwei oder drei
//! Varianten eines Abschnitts als Geschwister-Vorschläge mit gleicher Basis (Etiketten A/B/C, je eine Zeile und die
//! umgesetzten Befunde), über [`crate::assist::variants::run`]. Legt nur Vorschläge an – übernehmen und die übrigen
//! verwerfen kann nur der Mensch – und zählt zur MCP-Quote (zwei Aufträge je Client; die Quote nimmt `mcp/tools.rs`
//! vor dem Aufruf).

use serde::Deserialize;
use serde_json::{Value, json};

use super::session::Client;
use super::tools::{Failure, WorkRef, args, resolve, tool_error, work_schema};
use crate::AppState;

pub const NAME: &str = "solution_variants";

pub const DESCRIPTION: &str = "Varianten eines Abschnitts als Vorschläge: der Lektor (nach Einstellung) setzt Befunde \
und ihre Lösungen in 2 oder 3 Varianten (A/B/C) mit gleicher Basis rev um, je mit einer Zeile, was sie anders macht, \
und den umgesetzten Befunden (findings, ids aus [B<id>]; ohne Angabe die offenen Befunde des Abschnitts). Lange \
Abschnitte bekommen Teilvorschläge (nur die betroffenen Absätze). Legt nur Vorschläge an – übernehmen und die übrigen \
verwerfen kann nur der Mensch; Vergleich unter url. Zählt zur Grenze von zwei Modellaufträgen je Client.";

pub fn schema() -> Value {
    json!({ "type": "object", "required": ["work", "key"], "properties": {
        "work": work_schema(),
        "key": { "type": "integer", "description": "fester Schlüssel des Abschnitts (key aus outline)" },
        "rev": { "type": "integer", "minimum": 1, "description": "Basis (Vorgabe: die aktuelle Fassung)" },
        "findings": { "type": "array", "items": { "type": "integer" }, "description": "Befunde, die umgesetzt werden sollen ([B<id>])" },
        "count": { "type": "integer", "enum": [2, 3], "description": "Zahl der Varianten (Vorgabe 2)" } } })
}

#[derive(Deserialize)]
struct Args {
    work: WorkRef,
    key: i64,
    rev: Option<u32>,
    #[serde(default)]
    findings: Vec<i64>,
    count: Option<u8>,
}

pub async fn call_tool(state: &AppState, client: &Client, arguments: Value) -> Result<Value, Failure> {
    let a: Args = args(arguments)?;
    let (work, _, _) = resolve(&state.store, &a.work).await?;
    if let Some(count) = a.count
        && !(2..=crate::assist::variants::MAX_VARIANTS).contains(&count)
    {
        return Err(tool_error(format!("count: 2 oder 3 Varianten, nicht {count}.")));
    }
    if a.rev == Some(0) {
        return Err(Failure::Args("arguments: rev ab 1".into()));
    }
    // Unbekannter Abschnitt → isError statt eines Modellaufrufs.
    state.store.node_seq(work, a.key).await?;
    let order = crate::assist::variants::VariantOrder {
        work,
        key: a.key,
        rev: a.rev,
        findings: a.findings,
        count: a.count,
        asker: client.handle(),
        from_post: None,
    };
    let outcome = crate::assist::variants::run(state, &order, None).await?;
    let mut answer = serde_json::to_value(&outcome).map_err(|e| Failure::Internal(e.to_string()))?;
    answer["work"] = json!(work);
    answer["key"] = json!(a.key);
    answer["url"] = json!(format!("/varianten?work={work}&key={}&group={}", a.key, outcome.group));
    answer["status"] = json!("proposed");
    Ok(answer)
}
