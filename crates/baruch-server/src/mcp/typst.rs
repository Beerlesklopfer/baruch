// SPDX-License-Identifier: AGPL-3.0-or-later
//! MCP-Werkzeug `typst_check` (Vertrag Runde 8, Abschnitt 4d): das Werk mit Typst setzen und
//! Fehler und Warnungen mit Abschnitt melden. Nur lesend – es ändert nichts und schreibt keinen
//! Commit; der Lauf ist derselbe wie für `preview.typst` ([`crate::typeset`]: Zwischenspeicher,
//! eine Sperre, Zeitlimit 60 s).

use serde::Deserialize;
use serde_json::{Value, json};

use super::tools::{Failure, WorkRef, args, resolve, work_schema};
use crate::AppState;
use crate::typeset;

pub const DESCRIPTION: &str = "Werk mit Typst setzen (Werkvorlage, Gliederungsschema, Bilder) und prüfen, nur lesend: \
ändert nichts. Antwort: ok (keine Fehler), stand (Fassungsstand), pages (Seitenzahl), errors, warnings, \
diagnostics (je severity, message, hints, line/column im Export, key und section des Abschnitts bzw. \
„Werkvorlage“, section_line), print (Druckprüfung: Rasterbilder unter 300 dpi in ihrer gesetzten Größe mit \
path, page, Pixeln, mm, dpi, nötigen Pixeln und message), missing (Bildpfade ohne Datei im Werk) und url der \
Satz-Seite. Ohne work das Vorgabewerk; key setzt den Fokus der url \
auf das Kapitel des Abschnitts. Pakete (@preview) werden nicht geladen.";

pub fn check_schema() -> Value {
    json!({ "type": "object", "properties": {
        "work": work_schema(),
        "key": { "type": "integer",
                 "description": "fester Schlüssel eines Abschnitts (key aus outline): url zeigt sein Kapitel" } } })
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
