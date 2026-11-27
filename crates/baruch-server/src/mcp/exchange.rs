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

pub const DESCRIPTION: &str = "Ganzes Werk als Austauschdatei für eine andere Baruch-Instanz ausgeben, nur lesend: \
ändert nichts. Inhalt: Angaben, Buchangaben, Werkvorlage, Gliederung mit allen Fassungen, Beiträge, Einfälle, \
Gedankenkarte (Verbindungen zwischen Einfällen und Lagen), Fäden, Bilder und Grabsteine, jedes Stück mit UUID \
(Format urn:baruch:werk:1, aktuelle Unterversion in docs/Austauschformat.md); ohne Schlüssel, Verbindungen zu Diensten, \
Persona-Modelle und Anträge. format xml (Vorgabe) liefert xml als Text (Bilder als \
Base64), format baruch den ZIP-Container als Base64 in data; dazu work, uuid, file_name, media_type, bytes, \
sha256. Höchstens 4 MiB, größere Werke über die Seite /werke/{id}/austausch oder die Kommandozeile. Einlesen \
gibt es über MCP nicht (nur der Mensch über Formular, RPC oder Kommandozeile).";

pub fn schema() -> Value {
    json!({ "type": "object", "required": ["work"], "properties": {
        "work": work_schema(),
        "format": { "enum": ["xml", "baruch"],
                    "description": "xml (Vorgabe): eine XML-Datei als Text; baruch: ZIP-Container als Base64" } } })
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
            .ok_or_else(|| Failure::Args(format!("format: „xml“ oder „baruch“ erwartet, nicht „{word}“")))?,
    };
    let file = export_file(state, work, format).await?;
    if file.bytes.len() > MCP_EXPORT_LIMIT {
        return Err(tool_error(format!(
            "Die Austauschdatei ist {} Bytes groß, über MCP gehen höchstens {} Bytes (4 MiB). Bitte über die Seite \
             /werke/{work}/austausch (Werk ▾ → Austausch: Ausgeben) oder die Kommandozeile \
             „baruch-server export {slug} --baruch {}“ ausgeben.",
            file.bytes.len(),
            MCP_EXPORT_LIMIT,
            file.file_name
        )));
    }
    Ok(answer(file))
}
