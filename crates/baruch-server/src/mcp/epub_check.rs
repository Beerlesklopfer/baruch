// SPDX-License-Identifier: AGPL-3.0-or-later
//! MCP-Werkzeug `epub_check` (Runde 20c, Besitz „epub-pruefung“): eigene Strukturprüfung des EPUB und Warnungen je
//! Reader, nur lesend. Startet NIE EPUBCheck (das darf nur der Mensch über RPC/Formular): `run_check` bekommt hier
//! fest `epubcheck = false`, die Antwort trägt `epubcheck: {status: "not_requested"}`. Eintrag in der Werkzeugliste
//! und Aufruf setzt `tools.rs`.

use serde::Deserialize;
use serde_json::{Value, json};

use super::tools::{Failure, WorkRef, args, work_or_default, work_schema};
use crate::AppState;

pub const NAME: &str = "epub_check";

pub const DESCRIPTION: &str = "Check the EPUB of the work, read-only: own structure and accessibility check \
(mimetype, container.xml, manifest and spine, well-formedness with xmllint or quick-xml, navigation, language, alt \
texts, reading order) and warnings per reader (slugs as in epub.readers; without the argument the mandatory readers). \
Never starts EPUBCheck – only the human does that on the check page. Answer: findings [{code, severity, message, \
path, reader}], xml_tool, epubcheck (always not_requested), summary {errors, warnings, infos}, readers.";

pub fn schema() -> Value {
    json!({ "type": "object", "properties": {
        "work": work_schema(),
        "readers": { "type": "array", "items": { "type": "string" },
                     "description": "slugs of the reader profiles (without the argument: the mandatory readers)" },
        "footnotes": { "type": "string", "enum": ["popup", "endnotes"], "description": "footnotes in the EPUB (default popup)" } } })
}

#[derive(Deserialize)]
struct Args {
    work: Option<WorkRef>,
    #[serde(default)]
    readers: Vec<String>,
    #[serde(default)]
    footnotes: baruch_epub::FootnoteMode,
}

pub async fn call(state: &AppState, arguments: Value) -> Result<Value, Failure> {
    let a: Args = args(arguments)?;
    let work = work_or_default(state, a.work.as_ref()).await?;
    let readers = crate::epub_check::readers_of(&a.readers)?;
    let path = std::env::var_os("PATH");
    // Nie EPUBCheck: fest `false`.
    let checked = crate::epub_check::run_check(state, work, &readers, a.footnotes, false, path.as_deref()).await?;
    let mut answer = checked.to_json();
    answer["work"] = json!(work);
    answer["url"] = json!(format!("/werke/{work}/epub/pruefung"));
    Ok(answer)
}
