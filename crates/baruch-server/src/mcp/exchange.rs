// SPDX-License-Identifier: AGPL-3.0-or-later
//! MCP-Werkzeug `exchange_export` (Runde 26): das ganze Werk als Austauschdatei, nur lesend.
//!
//! Antwort wie RPC `work.exchange_export` ([`crate::exchange::export::answer`]): `xml` als Text (Vorgabe) bzw.
//! `data` als Base64 (`format: "baruch"`). Über [`MCP_EXPORT_LIMIT`] Bytes Datei ein ehrlicher Fehler mit Hinweis
//! auf Formular und Kommandozeile. Ein Einlesen gibt es über MCP nicht: Das bleibt beim Menschen (Formular,
//! RPC, Kommandozeile).

use serde::Deserialize;
use serde_json::{Value, json};

use super::tools::{Failure, WorkRef, args, resolve, tool_error, work_schema};
use crate::AppState;
use crate::exchange::export::{Format, answer, export_file};

pub const NAME: &str = "exchange_export";

/// Höchstgröße der Datei (vor Base64) für eine MCP-Antwort: 4 MiB. Die Antwort trägt den Inhalt zweimal
/// (`structuredContent` und Text), als JSON maskiert; größere Werke über Formular oder Kommandozeile.
pub const MCP_EXPORT_LIMIT: usize = 4 * 1024 * 1024;

pub const DESCRIPTION: &str = "Return a whole work as an exchange file for another Baruch instance, read-only: \
changes nothing. Content: data, book data, work language, work template, outline with all revisions, posts, ideas, \
mind map (links between ideas and positions), threads, images and tombstones, every piece with a UUID (format \
urn:baruch:werk:1, current sub-version in docs/Austauschformat.md); without keys, connections to services, persona \
models and requests. format xml (default) returns xml as text (images as Base64), format baruch the ZIP container \
as Base64 in data; plus work, uuid, file_name, media_type, bytes, sha256. At most 4 MiB, larger works through the \
page /werke/{id}/austausch or the command line. Reading in does not exist over MCP (only the human through form, \
RPC or command line).";

pub fn schema() -> Value {
    json!({ "type": "object", "required": ["work"], "properties": {
        "work": work_schema(),
        "format": { "enum": ["xml", "baruch"],
                    "description": "xml (default): one XML file as text; baruch: ZIP container as Base64" } } })
}

#[derive(Deserialize)]
struct ExportArgs {
    work: WorkRef,
    format: Option<String>,
}

pub async fn call(state: &AppState, arguments: Value) -> Result<Value, Failure> {
    let a: ExportArgs = args(arguments)?;
    let (work, slug, _) = resolve(&state.store, &a.work).await?;
    let format = match a.format.as_deref() {
        None => Format::Xml,
        Some(word) => Format::parse(word)
            .ok_or_else(|| Failure::Args(format!("format: \"xml\" or \"baruch\" expected, not \"{word}\"")))?,
    };
    let file = export_file(state, work, format).await?;
    if file.bytes.len() > MCP_EXPORT_LIMIT {
        return Err(tool_error(format!(
            "The exchange file has {} bytes, over MCP at most {} bytes (4 MiB) work. Please export through the page \
             /werke/{work}/austausch (Werk ▾ → Austausch: Ausgeben) or the command line \
             \"baruch-server export {slug} --baruch {}\".",
            file.bytes.len(),
            MCP_EXPORT_LIMIT,
            file.file_name
        )));
    }
    Ok(answer(file))
}
