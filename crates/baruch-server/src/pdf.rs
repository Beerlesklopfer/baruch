// SPDX-License-Identifier: AGPL-3.0-or-later
//! The book as PDF (contract Round 11, section 4): the same source as the typeset view
//! ([`crate::typeset::prepare`]: work template, current revisions, images, frame with cover and
//! front matter), typeset with [`baruch_render::compile_pdf`] – hence the same number of pages.
//!
//! - **Run:** in `spawn_blocking`, under the same lock as the preview ([`crate::typeset`],
//!   one Typst run at a time), with the time limit [`PDF_TIMEOUT`]. Per work at most **one** PDF generation
//!   (book or cover); a second simultaneous one → `REFUSED` „Für dieses Werk entsteht gerade ein
//!   PDF …“. If the time runs out, Typst finishes computing in the background (holds lock and work that long),
//!   the result is discarded.
//! - **Errors:** if the typesetting fails, there is **no** PDF (never half a file) but `REFUSED` with
//!   the first message and the number of errors; the page answers 422 with all messages and links to
//!   the sections.
//! - **RPC** `work.pdf {work, inline?}` → `{file_name, pages, bytes, sha256, url, print, pdf_base64?}`
//!   ([`call`]); **page** `GET /werke/{id}/pdf[?entwurf=1|0]` → `application/pdf` as attachment `<slug>.pdf` (409
//!   already running, 422 typesetting error, 503 time ran out; [`pages`]); **CLI** `baruch-server export <werk> --pdf
//!   DATEI` ([`write_pdf_file`]). Read-only: no commit.
//! - **Foreign pages (Round 18a):** `GET /werke/{id}/pdf` with `Sec-Fetch-Site: cross-site|same-site` → 403 without
//!   a Typst run ([`crate::guard::cross_site_get`]); [`pdf_runs`] counts the generations begun (for tests).
//! - **Cue notes (Round 16c):** submodule [`notes`] – RPC `notes.get`/`notes.pdf`, pages
//!   `/werke/{id}/stichwortzettel` and `/werke/{id}/stichwortzettel.pdf`, CLI `export … --stichwort DATEI`;
//!   the same generation (one per work, lock, time limit) as book and cover.
//! - **Draft (Round 22):** `work.pdf {draft?}`, `notes.pdf {draft?}` and `?entwurf=1|0` on both downloads – if the
//!   switch is missing, the typeset setting `draft` applies; `true`/`1` sets the watermark (stored values, otherwise
//!   the default), `false`/`0` leaves it out. The setting never changes ([`crate::typeset::effective_draft`]).

pub mod notes;

use std::collections::BTreeSet;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::Router;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use baruch_render::{Pdf, PrintWarning, Source};
use baruch_store::{StoreError, WorkId};
use minijinja::value::{Serde, Value as JinjaValue};
use serde_json::{Value, json};

use crate::AppState;
use crate::i18n::tc;
use crate::rpc::{INTERNAL, INVALID_PARAMS, METHOD_NOT_FOUND, REFUSED, RpcError};
use crate::typeset::{self, Compiled, Typeset};
use crate::view::Settings;

/// PDF generations begun in the process (Round 18a).
static PDF_RUNS: AtomicU64 = AtomicU64::new(0);

/// Number of PDF generations begun in the process (for tests under `common::PDF_SERIAL`).
#[doc(hidden)]
pub fn pdf_runs() -> u64 {
    PDF_RUNS.load(Ordering::SeqCst)
}

/// Time limit of a PDF generation.
pub const PDF_TIMEOUT: Duration = Duration::from_secs(120);

/// Which PDF: the book or the cover for the print shop (both through [`source_to_pdf`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PdfKind {
    Book,
    Cover,
    /// Cue notes (Round 16c, [`notes`]).
    Notes,
    /// Mind map (Round 25/34, `crate::idea_map::files`): the mind map of the ideas as one page.
    Map,
}

impl PdfKind {
    fn word(self) -> String {
        match self {
            PdfKind::Book => tc("export_msg.pdf.kind_book", &[]),
            PdfKind::Cover => tc("export_msg.pdf.kind_cover", &[]),
            PdfKind::Notes => tc("export_msg.pdf.kind_notes", &[]),
            PdfKind::Map => tc("export_msg.pdf.kind_map", &[]),
        }
    }
}

/// A finished PDF.
#[derive(Debug, Clone)]
pub struct PdfOut {
    pub bytes: Vec<u8>,
    pub pages: usize,
    pub stand: String,
    pub print: Vec<PrintWarning>,
    /// Duration of the Typst run in milliseconds.
    pub millis: u64,
}

/// Message when a PDF is already being generated for the work (language of the request, Round 39a).
pub fn busy_text() -> String {
    tc("export_msg.pdf.busy", &[])
}

/// Is `message` the refusal of [`busy_text`] (in German or in the language of the request)?
pub fn is_busy(message: &str) -> bool {
    message == busy_text() || message == crate::i18n::t(crate::i18n::Lang::De, "export_msg.pdf.busy", &[])
}

fn busy_works() -> std::sync::MutexGuard<'static, BTreeSet<WorkId>> {
    static BUSY_WORKS: Mutex<BTreeSet<WorkId>> = Mutex::new(BTreeSet::new());
    BUSY_WORKS.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Marker „für dieses Werk entsteht ein PDF“; goes away when dropped.
#[derive(Debug)]
pub struct PdfBusy(WorkId);

impl Drop for PdfBusy {
    fn drop(&mut self) {
        busy_works().remove(&self.0);
    }
}

/// Mark the work as „PDF entsteht“; `None` if it already is. Public for tests (to reproduce a second
/// simultaneous generation).
#[doc(hidden)]
pub fn claim(work: WorkId) -> Option<PdfBusy> {
    // First insert, release the lock of the set, then build the marker: `then_some(PdfBusy(…))`
    // would build and drop the marker also for „schon belegt“ – its Drop would take the lock a
    // second time (deadlock) and strike the work of the running generation from the set.
    let inserted = busy_works().insert(work);
    inserted.then(|| PdfBusy(work))
}

/// Why no PDF file was created (for status and error page).
enum Failed {
    /// A generation is already running for the work (409).
    Busy(RpcError),
    /// Time limit ran out (503).
    Timeout(RpcError),
    /// Typst reported errors (422); the result carries the messages.
    Typst(Box<Pdf>, RpcError),
    /// Otherwise (lock, crash).
    Other(RpcError),
}

impl Failed {
    fn into_rpc(self) -> RpcError {
        match self {
            Failed::Busy(e) | Failed::Timeout(e) | Failed::Typst(_, e) | Failed::Other(e) => e,
        }
    }
}

/// `REFUSED` with the first error message (with line or file) and the number of errors.
fn typst_refusal(kind: PdfKind, pdf: &Pdf) -> RpcError {
    let errors: Vec<_> = pdf.diagnostics.iter().filter(|d| d.severity == baruch_render::Severity::Error).collect();
    let first = errors.first().map_or_else(
        || tc("export_msg.pdf.no_pdf_from_typst", &[]),
        |d| match (&d.file, d.line) {
            (Some(file), Some(line)) => tc(
                "export_msg.pdf.error_in_file",
                &[("message", d.message.as_str().into()), ("file", file.as_str().into()), ("line", line.into())],
            ),
            (None, Some(line)) => {
                tc("export_msg.pdf.error_in_export", &[("message", d.message.as_str().into()), ("line", line.into())])
            }
            _ => d.message.clone(),
        },
    );
    let count = errors.len().max(1);
    let args = [("kind", kind.word().into()), ("count", count.into()), ("first", first.into())];
    let message = if count == 1 { tc("export_msg.pdf.failed_one", &args) } else { tc("export_msg.pdf.failed", &args) };
    RpcError::new(REFUSED, message)
}

async fn generate(work: WorkId, kind: PdfKind, source: Source, limit: Duration) -> Result<PdfOut, Failed> {
    let busy = claim(work).ok_or_else(|| Failed::Busy(RpcError::new(REFUSED, busy_text())))?;
    let run = async move {
        let permit = typeset::RUNS.acquire().await.map_err(|e| {
            Failed::Other(RpcError::new(INTERNAL, tc("export_msg.typeset.lock", &[("error", e.to_string().into())])))
        })?;
        let handle = tokio::task::spawn_blocking(crate::i18n::carry(move || {
            // Lock and work belong to the run: if the time runs out, Typst finishes computing and releases
            // both only then.
            let _permit = permit;
            let _busy = busy;
            PDF_RUNS.fetch_add(1, Ordering::SeqCst);
            let started = Instant::now();
            let pdf = baruch_render::compile_pdf(&source);
            (pdf, u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX))
        }));
        handle.await.map_err(|e| {
            Failed::Other(RpcError::new(INTERNAL, tc("export_msg.typeset.crashed", &[("error", e.to_string().into())])))
        })
    };
    let begun = Instant::now();
    let timeout = || Failed::Timeout(RpcError::new(REFUSED, typeset::timeout_text(limit)));
    // `tokio::time::timeout` polls the run first: if it finishes faster than the timer fires, `Ok` comes
    // even after the limit has passed (always possible at 0). Therefore the measured time counts in addition.
    let (pdf, millis) = match tokio::time::timeout(limit, run).await {
        Ok(_) if begun.elapsed() >= limit => return Err(timeout()),
        Ok(result) => result?,
        Err(_) => return Err(timeout()),
    };
    match pdf.bytes {
        Some(ref bytes) if !pdf.has_errors() => {
            Ok(PdfOut { bytes: bytes.clone(), pages: pdf.pages, stand: pdf.stand, print: pdf.print, millis })
        }
        _ => {
            let error = typst_refusal(kind, &pdf);
            Err(Failed::Typst(Box::new(pdf), error))
        }
    }
}

/// Typeset the source as PDF (book or cover): at most one generation per work, the same lock as
/// the preview, time limit `limit`. Errors: `REFUSED` (already running, typesetting error with first message and
/// number, „Typst brauchte länger als …“); never half the bytes.
pub async fn source_to_pdf(work: WorkId, kind: PdfKind, source: Source, limit: Duration) -> Result<PdfOut, RpcError> {
    generate(work, kind, source, limit).await.map_err(Failed::into_rpc)
}

/// The book as PDF: [`typeset::prepare`] (with frame) and [`source_to_pdf`].
pub async fn book_pdf(state: &AppState, work: WorkId) -> Result<PdfOut, RpcError> {
    book_pdf_with_limit(state, work, PDF_TIMEOUT).await
}

/// Like [`book_pdf`] with its own time limit (tests with a short limit).
#[doc(hidden)]
pub async fn book_pdf_with_limit(state: &AppState, work: WorkId, limit: Duration) -> Result<PdfOut, RpcError> {
    book_pdf_draft(state, work, limit, None).await
}

/// The book as PDF with the three-valued draft switch (Round 22, [`typeset::prepare_with`]); the setting
/// stays unchanged.
pub async fn book_pdf_draft(
    state: &AppState,
    work: WorkId,
    limit: Duration,
    draft: Option<bool>,
) -> Result<PdfOut, RpcError> {
    let prepared = typeset::prepare_with(state, work, draft).await?;
    source_to_pdf(work, PdfKind::Book, prepared.source, limit).await
}

// ---- RPC ---------------------------------------------------------------------------------------

pub fn handles(method: &str) -> bool {
    method == "work.pdf" || notes::handles(method)
}

fn flag(params: &Value, name: &str) -> Result<bool, RpcError> {
    match params.get(name) {
        None | Some(Value::Null) => Ok(false),
        Some(Value::Bool(b)) => Ok(*b),
        Some(_) => Err(RpcError::new(INVALID_PARAMS, tc("export_msg.pdf.flag", &[("name", name.into())]))),
    }
}

/// Answer to `work.pdf` (without the bytes themselves, except with `inline`).
pub fn answer(slug: &str, work: WorkId, pdf: &PdfOut, inline: bool) -> Value {
    let mut answer = json!({
        "file_name": format!("{slug}.pdf"),
        "pages": pdf.pages,
        "bytes": pdf.bytes.len(),
        "sha256": baruch_render::sha256_hex(&pdf.bytes),
        "url": format!("/werke/{work}/pdf"),
        "print": pdf.print,
        "stand": pdf.stand,
        "millis": pdf.millis,
    });
    if inline {
        answer["pdf_base64"] = json!(baruch_store::encode_base64(&pdf.bytes));
    }
    answer
}

/// `work.pdf {work, inline?: bool = false, draft?: bool}` → `{file_name, pages, bytes, sha256, url, print, stand,
/// millis, pdf_base64?}`. Downloading goes through `url`; `inline: true` also delivers the PDF as Base64. Round 22:
/// `draft` missing = watermark by typeset setting, `true` = with, `false` = without (the setting stays).
pub async fn call(state: &AppState, work: WorkId, method: &str, params: Value) -> Result<Value, RpcError> {
    if notes::handles(method) {
        return notes::call(state, work, method, params).await;
    }
    if !handles(method) {
        return Err(RpcError::new(METHOD_NOT_FOUND, format!("unknown method {method}")));
    }
    let inline = flag(&params, "inline")?;
    let draft = typeset::draft_param(&params)?;
    let slug = state.store.work_slug(work).await?;
    let pdf = book_pdf_draft(state, work, PDF_TIMEOUT, draft).await?;
    let mut answer = answer(&slug, work, &pdf, inline);
    if draft.is_some() {
        answer["url"] = json!(format!("/werke/{work}/pdf?{}", typeset::draft_pair(draft)));
    }
    Ok(answer)
}

// ---- Page --------------------------------------------------------------------------------------

/// `GET /werke/{id}/pdf`, `GET /werke/{id}/stichwortzettel` and `GET /werke/{id}/stichwortzettel.pdf` (read
/// only).
pub fn pages() -> Router<Arc<AppState>> {
    Router::new()
        .route("/werke/{id}/pdf", get(download))
        .route("/werke/{id}/stichwortzettel", get(notes::notes_page))
        .route("/werke/{id}/stichwortzettel.pdf", get(notes::notes_download))
}

/// PDF as an attachment: `Content-Disposition: attachment; filename="<name>"`. The name consists of the slug
/// (`[a-z0-9-]`) and may therefore go into the header without escaping.
pub fn attachment(file_name: &str, bytes: Vec<u8>) -> Response {
    let disposition = HeaderValue::from_str(&format!("attachment; filename=\"{file_name}\""))
        .unwrap_or_else(|_| HeaderValue::from_static("attachment"));
    (
        [
            (header::CONTENT_TYPE, HeaderValue::from_static("application/pdf")),
            (header::CONTENT_DISPOSITION, disposition),
            (header::X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff")),
            (header::CACHE_CONTROL, HeaderValue::from_static("no-cache")),
        ],
        bytes,
    )
        .into_response()
}

#[derive(serde::Deserialize)]
struct DownloadQuery {
    /// Round 22: watermark three-valued (`1` with, `0` without, missing = by setting).
    entwurf: Option<String>,
}

async fn download(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<WorkId>,
    Query(q): Query<DownloadQuery>,
) -> Response {
    if crate::guard::cross_site_get(&headers) {
        return typeset::foreign_get();
    }
    let draft = match typeset::draft_query(q.entwurf.as_deref()) {
        Ok(draft) => draft,
        Err(message) => return (StatusCode::BAD_REQUEST, message).into_response(),
    };
    let (slug, title) = match state.store.works().await {
        Ok(works) => match works.into_iter().find(|(w, _, _)| *w == id) {
            Some((_, slug, title)) => (slug, title),
            None => return (StatusCode::NOT_FOUND, tc("export_msg.page.unknown_work", &[])).into_response(),
        },
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                tc("export_msg.page.error", &[("error", e.to_string().into())]),
            )
                .into_response();
        }
    };
    let prepared = match typeset::prepare_with(&state, id, draft).await {
        Ok(prepared) => prepared,
        Err(StoreError::UnknownWork(_)) => {
            return (StatusCode::NOT_FOUND, tc("export_msg.page.unknown_work", &[])).into_response();
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                tc("export_msg.page.error", &[("error", e.to_string().into())]),
            )
                .into_response();
        }
    };
    let source = prepared.source.clone();
    let failed = match generate(id, PdfKind::Book, source, PDF_TIMEOUT).await {
        Ok(pdf) => return attachment(&format!("{slug}.pdf"), pdf.bytes),
        Err(failed) => failed,
    };
    let settings = Settings::from_cookie_header(headers.get(header::COOKIE).and_then(|v| v.to_str().ok()));
    let (status, error, diagnostics) = match failed {
        Failed::Busy(e) => (StatusCode::CONFLICT, e.message, Vec::new()),
        Failed::Timeout(e) => (StatusCode::SERVICE_UNAVAILABLE, e.message, Vec::new()),
        Failed::Other(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.message, Vec::new()),
        Failed::Typst(pdf, e) => {
            // Trace messages back to the sections through the map (as in the typeset view).
            let rendered = baruch_render::Rendered {
                stand: pdf.stand.clone(),
                pages: Vec::new(),
                headings: Vec::new(),
                diagnostics: pdf.diagnostics.clone(),
                print: Vec::new(),
                page_lines: Vec::new(),
            };
            let typeset = Typeset { prepared, compiled: Arc::new(Compiled { rendered, millis: 0 }), cached: false };
            (StatusCode::UNPROCESSABLE_ENTITY, e.message, typeset::diagnostic_links(id, typeset.diagnostics()))
        }
    };
    let ctx = minijinja::context! {
        settings => JinjaValue::from(Serde(settings)),
        work => typeset::sv(json!({ "id": id, "title": title })),
        back => typeset::href(format!("/werke/{id}/satz")),
        error,
        pdf_error => true,
        diagnostics => typeset::sv(diagnostics),
    };
    typeset::render(status, ctx)
}

// ---- Command line ------------------------------------------------------------------------------

/// `baruch-server export <werk> --pdf DATEI`: the book as PDF to `target`. If the file already exists,
/// nothing is written; if the typesetting fails, no file is created. Result: the PDF (for the
/// output of the CLI).
/// The command line stays German (Round 39a, contract §5: CLI untouched): the whole run in a German scope.
pub async fn write_pdf_file(state: &AppState, work: WorkId, target: &std::path::Path) -> Result<PdfOut, String> {
    crate::i18n::scope(crate::i18n::Lang::De, write_pdf_file_german(state, work, target)).await
}

async fn write_pdf_file_german(state: &AppState, work: WorkId, target: &std::path::Path) -> Result<PdfOut, String> {
    if std::fs::symlink_metadata(target).is_ok() {
        return Err(format!("{} gibt es schon; nichts überschrieben", target.display()));
    }
    let pdf = book_pdf(state, work).await.map_err(|e| e.message)?;
    write_new_file(target, pdf.bytes.clone()).await?;
    Ok(pdf)
}

/// Write `bytes` into a new file (`create_new`: an existing one stays untouched); if the writing
/// fails, the half file is removed.
async fn write_new_file(target: &std::path::Path, bytes: Vec<u8>) -> Result<(), String> {
    let path = target.to_owned();
    tokio::task::spawn_blocking(crate::i18n::carry(move || -> Result<(), String> {
        use std::io::Write;
        let shown = path.display().to_string();
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|e| format!("{shown} lässt sich nicht schreiben: {e}"))?;
        if let Err(e) = file.write_all(&bytes).and_then(|()| file.sync_all()) {
            // Leave no half file lying around.
            drop(file);
            let _ = std::fs::remove_file(&path);
            return Err(format!("{shown} lässt sich nicht schreiben: {e}"));
        }
        Ok(())
    }))
    .await
    .map_err(|e| format!("Schreiben abgebrochen: {e}"))?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_work_can_be_claimed_only_once_at_a_time() {
        let first = claim(-7).expect("frei");
        assert!(claim(-7).is_none(), "zweite Erzeugung für dasselbe Werk wird abgelehnt");
        assert!(claim(-8).is_some(), "ein anderes Werk ist frei");
        drop(first);
        assert!(claim(-7).is_some(), "nach dem Ende ist das Werk wieder frei");
    }

    #[test]
    fn a_typst_refusal_names_the_first_error_and_the_count() {
        crate::i18n::sync_scope(crate::i18n::Lang::De, || {
            let diag = |message: &str, line| baruch_render::Diagnostic {
                severity: baruch_render::Severity::Error,
                message: message.into(),
                hints: vec![],
                line: Some(line),
                column: Some(1),
                file: None,
            };
            let pdf = Pdf {
                stand: String::new(),
                bytes: None,
                pages: 0,
                diagnostics: vec![diag("unknown variable: x", 4), diag("unknown variable: y", 9)],
                print: vec![],
            };
            let e = typst_refusal(PdfKind::Book, &pdf);
            assert_eq!(e.code, REFUSED, "Satzfehler sind fachlich (REFUSED)");
            assert_eq!(
                e.message,
                "Kein PDF (Buch): Der Satz scheitert mit 2 Fehlern. Erster: unknown variable: x (Zeile 4 im Export)",
                "erste Meldung mit Zeile und die Zahl"
            );
        })
    }
}
