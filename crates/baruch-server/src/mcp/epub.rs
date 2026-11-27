// SPDX-License-Identifier: AGPL-3.0-or-later
//! MCP tool `epub_export` (Round 20a, owned by „epub“): the work as EPUB, read-only – metadata, file list,
//! warnings and address; the bytes (Base64) only with `include_bytes` and at most [`MCP_EPUB_LIMIT`]. The entry in
//! the tool list and the call are set by `tools.rs`.

use serde::Deserialize;
use serde_json::{Value, json};

use super::tools::{Failure, WorkRef, args, tool_error, work_or_default, work_schema};
use crate::AppState;
use baruch_epub::FootnoteMode;

pub const NAME: &str = "epub_export";

/// Largest EPUB file that `epub_export` delivers as Base64 with `include_bytes` (8 MiB).
pub const MCP_EPUB_LIMIT: usize = 8 * 1024 * 1024;

pub const DESCRIPTION: &str = "The work as EPUB 3 (with EPUB 2 fallback), read-only: metadata (title, author, \
language, ISBN, date), files in reading order (one file per part and chapter), warnings (e.g. omitted Typst islands, \
images without alternative text, page break rules of the work template), size, SHA-256 and the address \
/werke/{id}/epub for downloading. footnotes: popup (default) or endnotes. The file itself (Base64) only with \
include_bytes: true and at most 8 MiB. draft: true = as a draft (note on the title page, \"(Entwurf)\" in the title \
metadata), false = without, omitted = by the typesetting setting; the setting stays unchanged. Changes nothing in \
the work.";

pub fn schema() -> Value {
    json!({ "type": "object", "properties": {
        "work": work_schema(),
        "footnotes": { "enum": ["popup", "endnotes"], "description": "footnotes as popup (default) or endnotes" },
        "include_bytes": { "type": "boolean", "description": "also return the EPUB as Base64 (at most 8 MiB)" },
        "draft": { "type": "boolean",
            "description": "true = as a draft, false = without draft, omitted = by the typesetting setting" } } })
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
            "The EPUB has {} bytes and is larger than 8 MiB; without include_bytes there are metadata and file list, the \
             file itself under {url} or with baruch-server export {slug} --epub FILE.",
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
