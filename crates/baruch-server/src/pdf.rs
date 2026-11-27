// SPDX-License-Identifier: AGPL-3.0-or-later
//! Das Buch als PDF (Vertrag Runde 11, Abschnitt 4): dieselbe Quelle wie die Satz-Ansicht
//! ([`crate::typeset::prepare`]: Werkvorlage, aktuelle Fassungen, Bilder, Rahmen mit Umschlag und
//! Titelei), gesetzt mit [`baruch_render::compile_pdf`] – darum dieselbe Seitenzahl.
//!
//! - **Lauf:** in `spawn_blocking`, unter derselben Sperre wie die Vorschau ([`crate::typeset`],
//!   ein Typst-Lauf zur Zeit), mit Zeitlimit [`PDF_TIMEOUT`]. Je Werk höchstens **eine** PDF-Erzeugung
//!   (Buch oder Umschlag); eine zweite gleichzeitige → `REFUSED` „Für dieses Werk entsteht gerade ein
//!   PDF …“. Läuft die Zeit ab, rechnet Typst im Hintergrund zu Ende (hält Sperre und Werk so lange),
//!   das Ergebnis wird verworfen.
//! - **Fehler:** Scheitert der Satz, gibt es **kein** PDF (nie eine halbe Datei), sondern `REFUSED` mit
//!   der ersten Meldung und der Zahl der Fehler; die Seite antwortet 422 mit allen Meldungen und Links zu
//!   den Abschnitten.
//! - **RPC** `work.pdf {work, inline?}` → `{file_name, pages, bytes, sha256, url, print, pdf_base64?}`
//!   ([`call`]); **Seite** `GET /werke/{id}/pdf[?entwurf=1|0]` → `application/pdf` als Anhang `<slug>.pdf` (409 läuft
//!   schon, 422 Satzfehler, 503 Zeit abgelaufen; [`pages`]); **CLI** `baruch-server export <werk> --pdf
//!   DATEI` ([`write_pdf_file`]). Nur lesend: kein Commit.
//! - **Fremde Seiten (Runde 18a):** `GET /werke/{id}/pdf` mit `Sec-Fetch-Site: cross-site|same-site` → 403 ohne
//!   Typst-Lauf ([`crate::guard::cross_site_get`]); [`pdf_runs`] zählt die begonnenen Erzeugungen (für Tests).
//! - **Stichwortzettel (Runde 16c):** Untermodul [`notes`] – RPC `notes.get`/`notes.pdf`, Seiten
//!   `/werke/{id}/stichwortzettel` und `/werke/{id}/stichwortzettel.pdf`, CLI `export … --stichwort DATEI`;
//!   dieselbe Erzeugung (eine je Werk, Sperre, Zeitlimit) wie Buch und Umschlag.
//! - **Entwurf (Runde 22):** `work.pdf {draft?}`, `notes.pdf {draft?}` und `?entwurf=1|0` an beiden Downloads – fehlt
//!   der Schalter, gilt die Satz-Einstellung `draft`; `true`/`1` setzt das Wasserzeichen (gespeicherte Werte, sonst
//!   Vorgabe), `false`/`0` lässt es weg. Die Einstellung ändert sich nie ([`crate::typeset::effective_draft`]).

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
use crate::rpc::{INTERNAL, INVALID_PARAMS, METHOD_NOT_FOUND, REFUSED, RpcError};
use crate::typeset::{self, Compiled, Typeset};
use crate::view::Settings;

/// Begonnene PDF-Erzeugungen im Prozess (Runde 18a).
static PDF_RUNS: AtomicU64 = AtomicU64::new(0);

/// Zahl der begonnenen PDF-Erzeugungen im Prozess (für Tests unter `common::PDF_SERIAL`).
#[doc(hidden)]
pub fn pdf_runs() -> u64 {
    PDF_RUNS.load(Ordering::SeqCst)
}

/// Zeitlimit einer PDF-Erzeugung.
pub const PDF_TIMEOUT: Duration = Duration::from_secs(120);

/// Welches PDF: das Buch oder der Umschlag für die Druckerei (beide über [`source_to_pdf`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PdfKind {
    Book,
    Cover,
    /// Stichwortzettel (Runde 16c, [`notes`]).
    Notes,
    /// Mindmap (Runde 25/34, `crate::idea_map::files`): die Mindmap der Einfälle als eine Seite.
    Map,
}

impl PdfKind {
    fn word(self) -> &'static str {
        match self {
            PdfKind::Book => "Buch",
            PdfKind::Cover => "Umschlag",
            PdfKind::Notes => "Stichwortzettel",
            PdfKind::Map => "Mindmap",
        }
    }
}

/// Ein fertiges PDF.
#[derive(Debug, Clone)]
pub struct PdfOut {
    pub bytes: Vec<u8>,
    pub pages: usize,
    pub stand: String,
    pub print: Vec<PrintWarning>,
    /// Dauer des Typst-Laufs in Millisekunden.
    pub millis: u64,
}

/// Meldung, wenn für das Werk schon ein PDF entsteht.
pub const BUSY: &str = "Für dieses Werk entsteht gerade ein PDF; bitte warten und dann noch einmal versuchen.";

fn busy_works() -> std::sync::MutexGuard<'static, BTreeSet<WorkId>> {
    static BUSY_WORKS: Mutex<BTreeSet<WorkId>> = Mutex::new(BTreeSet::new());
    BUSY_WORKS.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Markierung „für dieses Werk entsteht ein PDF“; fällt beim Verwerfen weg.
#[derive(Debug)]
pub struct PdfBusy(WorkId);

impl Drop for PdfBusy {
    fn drop(&mut self) {
        busy_works().remove(&self.0);
    }
}

/// Werk als „PDF entsteht“ markieren; `None`, wenn es das schon ist. Für Tests öffentlich (zweite
/// gleichzeitige Erzeugung nachstellen).
#[doc(hidden)]
pub fn claim(work: WorkId) -> Option<PdfBusy> {
    // Erst eintragen, die Sperre der Menge freigeben, dann die Markierung bauen: `then_some(PdfBusy(…))`
    // würde die Markierung auch bei „schon belegt“ bauen und verwerfen – ihr Drop nähme die Sperre ein
    // zweites Mal (Verklemmung) und striche das Werk der laufenden Erzeugung aus der Menge.
    let inserted = busy_works().insert(work);
    inserted.then(|| PdfBusy(work))
}

/// Warum keine PDF-Datei entstand (für Status und Fehlerseite).
enum Failed {
    /// Für das Werk läuft schon eine Erzeugung (409).
    Busy(RpcError),
    /// Zeitlimit abgelaufen (503).
    Timeout(RpcError),
    /// Typst meldete Fehler (422); das Ergebnis trägt die Meldungen.
    Typst(Box<Pdf>, RpcError),
    /// Sonst (Sperre, Absturz).
    Other(RpcError),
}

impl Failed {
    fn into_rpc(self) -> RpcError {
        match self {
            Failed::Busy(e) | Failed::Timeout(e) | Failed::Typst(_, e) | Failed::Other(e) => e,
        }
    }
}

/// `REFUSED` mit der ersten Fehlermeldung (mit Zeile bzw. Datei) und der Zahl der Fehler.
fn typst_refusal(kind: PdfKind, pdf: &Pdf) -> RpcError {
    let errors: Vec<_> = pdf.diagnostics.iter().filter(|d| d.severity == baruch_render::Severity::Error).collect();
    let first = errors.first().map_or_else(
        || "Typst lieferte kein PDF".to_owned(),
        |d| {
            let place = match (&d.file, d.line) {
                (Some(file), Some(line)) => format!(" ({file}, Zeile {line})"),
                (None, Some(line)) => format!(" (Zeile {line} im Export)"),
                _ => String::new(),
            };
            format!("{}{place}", d.message)
        },
    );
    let count = errors.len().max(1);
    let word = if count == 1 { "Fehler" } else { "Fehlern" };
    RpcError::new(
        REFUSED,
        format!("Kein PDF ({}): Der Satz scheitert mit {count} {word}. Erster: {first}", kind.word()),
    )
}

async fn generate(work: WorkId, kind: PdfKind, source: Source, limit: Duration) -> Result<PdfOut, Failed> {
    let busy = claim(work).ok_or_else(|| Failed::Busy(RpcError::new(REFUSED, BUSY)))?;
    let run = async move {
        let permit = typeset::RUNS
            .acquire()
            .await
            .map_err(|e| Failed::Other(RpcError::new(INTERNAL, format!("Typst-Sperre: {e}"))))?;
        let handle = tokio::task::spawn_blocking(move || {
            // Sperre und Werk gehören dem Lauf: Läuft die Zeit ab, rechnet Typst zu Ende und gibt
            // beides erst dann frei.
            let _permit = permit;
            let _busy = busy;
            PDF_RUNS.fetch_add(1, Ordering::SeqCst);
            let started = Instant::now();
            let pdf = baruch_render::compile_pdf(&source);
            (pdf, u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX))
        });
        handle.await.map_err(|e| Failed::Other(RpcError::new(INTERNAL, format!("Typst ist abgestürzt: {e}"))))
    };
    let begun = Instant::now();
    let timeout = || {
        let message = format!("Typst brauchte länger als {}", typeset::seconds(limit));
        Failed::Timeout(RpcError::new(REFUSED, message))
    };
    // `tokio::time::timeout` pollt zuerst den Lauf: Ist er schneller fertig, als der Zeitgeber feuert, kommt
    // `Ok` auch nach Ablauf des Limits (bei 0 immer möglich). Darum zählt zusätzlich die gemessene Zeit.
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

/// Quelle als PDF setzen (Buch oder Umschlag): höchstens eine Erzeugung je Werk, dieselbe Sperre wie
/// die Vorschau, Zeitlimit `limit`. Fehler: `REFUSED` (läuft schon, Satzfehler mit erster Meldung und
/// Zahl, „Typst brauchte länger als …“); nie halbe Bytes.
pub async fn source_to_pdf(work: WorkId, kind: PdfKind, source: Source, limit: Duration) -> Result<PdfOut, RpcError> {
    generate(work, kind, source, limit).await.map_err(Failed::into_rpc)
}

/// Das Buch als PDF: [`typeset::prepare`] (mit Rahmen) und [`source_to_pdf`].
pub async fn book_pdf(state: &AppState, work: WorkId) -> Result<PdfOut, RpcError> {
    book_pdf_with_limit(state, work, PDF_TIMEOUT).await
}

/// Wie [`book_pdf`] mit eigenem Zeitlimit (Tests mit kurzem Limit).
#[doc(hidden)]
pub async fn book_pdf_with_limit(state: &AppState, work: WorkId, limit: Duration) -> Result<PdfOut, RpcError> {
    book_pdf_draft(state, work, limit, None).await
}

/// Das Buch als PDF mit dem dreiwertigen Entwurf-Schalter (Runde 22, [`typeset::prepare_with`]); die Einstellung
/// bleibt unverändert.
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
        Some(_) => Err(RpcError::new(INVALID_PARAMS, format!("{name}: true oder false erwartet"))),
    }
}

/// Antwort auf `work.pdf` (ohne die Bytes selbst, außer mit `inline`).
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
/// millis, pdf_base64?}`. Herunterladen geht über `url`; `inline: true` liefert das PDF als Base64 mit. Runde 22:
/// `draft` fehlt = Wasserzeichen nach Satz-Einstellung, `true` = mit, `false` = ohne (die Einstellung bleibt).
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

// ---- Seite -------------------------------------------------------------------------------------

/// `GET /werke/{id}/pdf`, `GET /werke/{id}/stichwortzettel` und `GET /werke/{id}/stichwortzettel.pdf` (nur
/// lesen).
pub fn pages() -> Router<Arc<AppState>> {
    Router::new()
        .route("/werke/{id}/pdf", get(download))
        .route("/werke/{id}/stichwortzettel", get(notes::notes_page))
        .route("/werke/{id}/stichwortzettel.pdf", get(notes::notes_download))
}

/// PDF als Anhang: `Content-Disposition: attachment; filename="<name>"`. Der Name besteht aus dem slug
/// (`[a-z0-9-]`) und darf darum ohne Maskierung in die Kopfzeile.
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
    /// Runde 22: Wasserzeichen dreiwertig (`1` mit, `0` ohne, fehlt = nach Einstellung).
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
            None => return (StatusCode::NOT_FOUND, "unbekanntes Werk").into_response(),
        },
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, format!("Fehler: {e}")).into_response(),
    };
    let prepared = match typeset::prepare_with(&state, id, draft).await {
        Ok(prepared) => prepared,
        Err(StoreError::UnknownWork(_)) => return (StatusCode::NOT_FOUND, "unbekanntes Werk").into_response(),
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, format!("Fehler: {e}")).into_response(),
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
            // Meldungen über die Karte auf die Abschnitte zurückführen (wie in der Satz-Ansicht).
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

// ---- Kommandozeile -----------------------------------------------------------------------------

/// `baruch-server export <werk> --pdf DATEI`: das Buch als PDF nach `target`. Gibt es die Datei schon,
/// wird nichts geschrieben; scheitert der Satz, entsteht keine Datei. Ergebnis: das PDF (für die
/// Ausgabe der CLI).
pub async fn write_pdf_file(state: &AppState, work: WorkId, target: &std::path::Path) -> Result<PdfOut, String> {
    if std::fs::symlink_metadata(target).is_ok() {
        return Err(format!("{} gibt es schon; nichts überschrieben", target.display()));
    }
    let pdf = book_pdf(state, work).await.map_err(|e| e.message)?;
    write_new_file(target, pdf.bytes.clone()).await?;
    Ok(pdf)
}

/// `bytes` in eine neue Datei schreiben (`create_new`: eine vorhandene bleibt unberührt); scheitert das
/// Schreiben, wird die halbe Datei entfernt.
async fn write_new_file(target: &std::path::Path, bytes: Vec<u8>) -> Result<(), String> {
    let path = target.to_owned();
    tokio::task::spawn_blocking(move || -> Result<(), String> {
        use std::io::Write;
        let shown = path.display().to_string();
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|e| format!("{shown} lässt sich nicht schreiben: {e}"))?;
        if let Err(e) = file.write_all(&bytes).and_then(|()| file.sync_all()) {
            // Keine halbe Datei liegen lassen.
            drop(file);
            let _ = std::fs::remove_file(&path);
            return Err(format!("{shown} lässt sich nicht schreiben: {e}"));
        }
        Ok(())
    })
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
    }
}
