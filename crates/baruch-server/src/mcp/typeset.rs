// SPDX-License-Identifier: AGPL-3.0-or-later
//! MCP tool `typeset_settings` (Round 29a, task satz-daten): read the
//! typeset settings of the work, like RPC `work.typeset_get` without `fields` ([`crate::typeset_settings`]).
//! Read-only – there is no writing MCP tool; only the human can change them (Einstellungen → Werk → Satz).
//! The entry in the tool list and the call are set by `tools.rs`.

use serde::Deserialize;
use serde_json::{Value, json};

use super::tools::{Failure, WorkRef, args, work_or_default, work_schema};
use crate::AppState;
use crate::typeset_settings;

pub const NAME: &str = "typeset_settings";

pub const DESCRIPTION: &str = "Read the typesetting settings of the work (chapter start, page format, margins, font, \
page numbers …), plus the generated Typst rules \"// Baruch: Satz\" and hints where the work template sets the same \
thing itself. Read-only; only the human can change them (Einstellungen → Werk → Satz). Answer: work, set, uuid, \
updated, updated_by, settings (only fields that are set; if a field is missing the work template applies), rules, \
conflicts [{field, label, line, text, effect, message}], template_rule {line, text, chapter_start} or null, warnings.";

pub fn schema() -> Value {
    json!({ "type": "object", "properties": { "work": work_schema() } })
}

#[derive(Deserialize)]
struct Args {
    work: Option<WorkRef>,
}

pub async fn call(state: &AppState, arguments: Value) -> Result<Value, Failure> {
    let a: Args = args(arguments)?;
    let work = work_or_default(state, a.work.as_ref()).await?;
    let view = typeset_settings::load(state, work).await?;
    Ok(typeset_settings::view_json(&view))
}
