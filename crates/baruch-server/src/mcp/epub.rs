// SPDX-License-Identifier: AGPL-3.0-or-later
//! MCP-Werkzeug `epub_export` (Runde 20a, Besitz „epub“): das Werk als EPUB, nur lesend – Metadaten, Dateiliste,
//! Warnungen und Adresse; die Bytes (Base64) nur mit `include_bytes` und höchstens [`MCP_EPUB_LIMIT`]. Eintrag in der
//! Werkzeugliste und Aufruf setzt `tools.rs`.

use serde::Deserialize;
use serde_json::{Value, json};

use super::tools::{Failure, WorkRef, args, tool_error, work_or_default, work_schema};
use crate::AppState;
use baruch_epub::FootnoteMode;

pub const NAME: &str = "epub_export";

/// Größte EPUB-Datei, die `epub_export` mit `include_bytes` als Base64 mitliefert (8 MiB).
pub const MCP_EPUB_LIMIT: usize = 8 * 1024 * 1024;

pub const DESCRIPTION: &str = "Das Werk als EPUB 3 (mit EPUB-2-Rückfall), nur lesend: Metadaten (Titel, Autor, \
Sprache, ISBN, Stand), Dateien in Lesereihenfolge (je Teil und Kapitel eine Datei), Warnungen (z. B. ausgelassene \
Typst-Inseln, Bilder ohne Alternativtext, Seitenumbruch-Regeln der Werkvorlage), Größe, SHA-256 und die Adresse \
/werke/{id}/epub zum Herunterladen. footnotes: popup (Vorgabe) oder endnotes. Die Datei selbst (Base64) nur mit \
include_bytes: true und höchstens 8 MiB. draft: true = als Entwurf (Hinweis auf der Titelseite, „(Entwurf)“ im \
Titel-Metadatum), false = ohne, weglassen = nach Satz-Einstellung; die Einstellung bleibt unverändert. Ändert nichts \
am Werk.";

pub fn schema() -> Value {
    json!({ "type": "object", "properties": {
        "work": work_schema(),
        "footnotes": { "enum": ["popup", "endnotes"], "description": "Fußnoten als Popup (Vorgabe) oder Endnoten" },
        "include_bytes": { "type": "boolean", "description": "EPUB als Base64 mitliefern (höchstens 8 MiB)" },
        "draft": { "type": "boolean",
            "description": "true = als Entwurf, false = ohne Entwurf, weglassen = nach Satz-Einstellung" } } })
}

#[derive(Deserialize)]
struct Args {
    work: Option<WorkRef>,
    #[serde(default)]
    footnotes: FootnoteMode,
    #[serde(default)]
    include_bytes: bool,
    #[serde(default)]
    draft: Option<bool>,
}

pub async fn call(state: &AppState, arguments: Value) -> Result<Value, Failure> {
    let a: Args = args(arguments)?;
    let work = work_or_default(state, a.work.as_ref()).await?;
    let book = crate::epub::book_for_draft(state, work, a.footnotes, a.draft).await?;
    let slug = state.store.work_slug(work).await?;
    let bytes = baruch_epub::package(&book).map_err(|e| Failure::Internal(e.to_string()))?;
    let pair = crate::typeset::draft_pair(a.draft);
    let url = if pair.is_empty() { format!("/werke/{work}/epub") } else { format!("/werke/{work}/epub?{pair}") };
    if a.include_bytes && bytes.len() > MCP_EPUB_LIMIT {
        return Err(tool_error(format!(
            "Das EPUB hat {} Bytes und ist größer als 8 MiB; ohne include_bytes gibt es Angaben und Dateiliste, die \
             Datei selbst unter {url} oder mit baruch-server export {slug} --epub DATEI.",
            bytes.len()
        )));
    }
    let meta = &book.meta;
    let mut answer = json!({
        "work": work,
        "file_name": format!("{slug}.epub"),
        "url": url,
        "bytes": bytes.len(),
        "sha256": baruch_exchange::sha256_hex(&bytes),
        "footnotes": crate::epub::footnote_word(a.footnotes),
        "meta": {
            "identifier": meta.identifier, "title": meta.title, "subtitle": meta.subtitle, "author": meta.author,
            "language": meta.language, "isbn": meta.isbn, "publisher": meta.publisher, "year": meta.year,
            "modified": meta.modified,
        },
        "files": crate::epub::spine_files(&book),
        "toc": book.toc,
        "warnings": book.warnings,
    });
    if a.include_bytes {
        answer["epub_base64"] = json!(baruch_store::encode_base64(&bytes));
    }
    Ok(answer)
}
