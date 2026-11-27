// SPDX-License-Identifier: AGPL-3.0-or-later
//! MCP tool `typst_check` (contract Round 8, section 4d): typeset the work with Typst and
//! report errors and warnings with their section. Read-only – it changes nothing and writes no
//! commit; the run is the same as for `preview.typst` ([`crate::typeset`]: cache,
//! one lock, time limit 60 s).

use serde::Deserialize;
use serde_json::{Value, json};

use super::tools::{Failure, WorkRef, args, resolve, work_schema};
use crate::AppState;
use crate::typeset;

pub const DESCRIPTION: &str = "Typeset the work with Typst (work template, outline scheme, images) and check it, \
read-only: changes nothing. Answer: ok (no errors), stand (revision state), pages (page count), errors, warnings, \
diagnostics (each severity, message, hints, line/column in the export, key and section of the section or \
\"Werkvorlage\", section_line), print (print check: raster images below 300 dpi at their typeset size with path, \
page, pixels, mm, dpi, pixels needed and message), missing (image paths without a file in the work) and url of the \
typesetting page. Without work the default work; key sets the focus of the url on the chapter of the section. \
Packages (@preview) are not loaded.";

pub fn check_schema() -> Value {
    json!({ "type": "object", "properties": {
        "work": work_schema(),
        "key": { "type": "integer",
                 "description": "stable key of a section (key from outline): url shows its chapter" } } })
}

#[derive(Deserialize)]
struct CheckArgs {
    work: Option<WorkRef>,
    key: Option<i64>,
}

pub async fn call(state: &AppState, arguments: Value) -> Result<Value, Failure> {
    let a: CheckArgs = args(arguments)?;
    let work = match &a.work {
        Some(work) => resolve(&state.store, work).await?.0,
        None => state.default_work().await.map_err(Failure::from)?,
    };
    let typeset = typeset::typeset_at(state, work, a.key).await?;
    let (errors, warnings) = typeset.counts();
    let url = match a.key {
        Some(key) => format!("/werke/{work}/satz?key={key}"),
        None => format!("/werke/{work}/satz"),
    };
    Ok(json!({
        "ok": errors == 0,
        "stand": typeset.prepared.stand,
        "pages": typeset.page_count(),
        "errors": errors,
        "warnings": warnings,
        "diagnostics": typeset.diagnostics(),
        "print": typeset.rendered().print,
        "missing": typeset::missing_images(&typeset.prepared.source),
        "url": url,
    }))
}
