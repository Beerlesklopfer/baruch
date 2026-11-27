// SPDX-License-Identifier: AGPL-3.0-or-later
//! Exchange: import (Round 26; contract F6/F7, doc `docs/Austauschformat.md` §4/§5).
//!
//! - `GET /austausch/einlesen[?work=W]`: form (multipart, field `datei`, plus `title`), without JavaScript.
//! - `POST /austausch/vorschau`: read and check the file ([`baruch_exchange::read::read`]), compute the preview
//!   ([`baruch_store::Store::exchange_preview`], rolled back); only then the file lies under
//!   `<data>/austausch-eingang/<sha256>` (entries older than 24 h are cleared away in the process). Then the preview
//!   with the form „Übernehmen“.
//! - `POST /austausch/uebernehmen {sha256, mode, title?}`: file from the inbox, checksum as shown, plan anew
//!   and write ([`baruch_store::Store::exchange_apply`], one transaction under `state.write_lock`), one commit
//!   in the mirror ([`crate::mirror::record_exchange`]); result page with links to the deviations
//!   (`/?work=W&key=K&rev=R`) and to the requests (`/gliederung?work=W#antraege`). The inbox file is deleted
//!   afterwards, also on a refusal.
//! - RPC `work.exchange_preview {data: Base64, file_name?, title?}` and `work.exchange_import {data, sha256, mode,
//!   title?}` (size limited by `works::RPC_BODY_LIMIT`; larger files through form or CLI).
//! - CLI `import DATEI.baruch|DATEI.baruch.xml [--abgleichen] [--vorschau] [--title T]`.
//!
//! Importing is an action of the human: never switch, overwrite or delete; outline changes only
//! as a request. Not through MCP. On every error nothing is written.

use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, SystemTime};

use axum::Router;
use axum::extract::multipart::MultipartRejection;
use axum::extract::{DefaultBodyLimit, Form, Multipart, Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use baruch_exchange::{ExchangeDocument, Limits};
use baruch_store::StoreError;
use baruch_store::exchange_apply::{ApplyMode, DeviationAction, ExchangeApplied, ExchangeReport};
use minijinja::Environment;
use minijinja::value::{Serde, Value as JinjaValue};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::rpc::{INTERNAL, INVALID_PARAMS, REFUSED, RpcError};
use crate::view::Settings;
use crate::{AppState, mirror};

/// Largest request to the forms: largest exchange file and 1 MiB for the other fields.
pub const UPLOAD_LIMIT: usize = Limits::DEFAULT.max_file_bytes as usize + 1024 * 1024;

/// Folder of the uploaded files between preview and applying: `<data>/austausch-eingang/<sha256>`.
pub const INBOX_DIR: &str = "austausch-eingang";

/// Entries in the inbox that are older are cleared away by the next preview.
pub const INBOX_MAX_AGE: Duration = Duration::from_secs(24 * 60 * 60);

/// Note of the CLI and the forms if the title is already taken.
pub const TITLE_HINT: &str = "Mit einem eigenen Titel einlesen (Formular: Feld „Titel“, CLI: --title \"…\").";

/// Page of the form (merged through `exchange::pages`).
pub fn pages() -> Router<Arc<AppState>> {
    Router::new().route("/austausch/einlesen", get(import_page))
}

/// Forms (merged through `exchange::writes`, behind `guard::same_origin`).
pub fn writes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/austausch/vorschau", post(preview_form).layer(DefaultBodyLimit::max(UPLOAD_LIMIT)))
        .route("/austausch/uebernehmen", post(import_form).layer(DefaultBodyLimit::max(UPLOAD_LIMIT)))
}

fn templates() -> &'static Environment<'static> {
    static ENV: OnceLock<Environment<'static>> = OnceLock::new();
    ENV.get_or_init(|| {
        let mut env = Environment::new();
        for (name, source) in [
            ("base.html", include_str!("../../templates/base.html")),
            ("exchange_import.html", include_str!("../../templates/exchange_import.html")),
            ("exchange_preview.html", include_str!("../../templates/exchange_preview.html")),
            ("exchange_done.html", include_str!("../../templates/exchange_done.html")),
        ] {
            env.add_template(name, source).expect("Vorlage des Einlesens fehlerhaft");
        }
        env
    })
}

fn render(name: &str, status: StatusCode, ctx: JinjaValue) -> Response {
    match templates().get_template(name).and_then(|t| t.render(ctx)) {
        Ok(html) => (status, Html(html)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, format!("Vorlage {name}: {e}")).into_response(),
    }
}

fn settings_of(headers: &HeaderMap) -> Settings {
    Settings::from_cookie_header(headers.get(header::COOKIE).and_then(|v| v.to_str().ok()))
}

fn status_of(e: &RpcError) -> StatusCode {
    match e.code {
        INVALID_PARAMS => StatusCode::UNPROCESSABLE_ENTITY,
        REFUSED => StatusCode::CONFLICT,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

/// Query of the page: `work` (work the menu came from; only for the header).
#[derive(Debug, Default, serde::Deserialize)]
pub struct ImportQuery {
    pub work: Option<String>,
}

/// Form page (also after an error, then with message).
fn form_page(
    headers: &HeaderMap,
    status: StatusCode,
    work: Option<i64>,
    error: Option<String>,
    title: &str,
) -> Response {
    let ctx = minijinja::context! {
        settings => JinjaValue::from(Serde(settings_of(headers))),
        work => work.map(|id| minijinja::context! { id => id }),
        error => error,
        title_value => title,
        limit_mib => Limits::DEFAULT.max_file_bytes / (1024 * 1024),
    };
    render("exchange_import.html", status, ctx)
}

/// `GET /austausch/einlesen[?work=W]`.
async fn import_page(headers: HeaderMap, Query(query): Query<ImportQuery>) -> Response {
    let work = query.work.as_deref().and_then(|w| w.trim().parse::<i64>().ok());
    form_page(&headers, StatusCode::OK, work, None, "")
}

// ---- Shared path -----------------------------------------------------------------------------------

/// Read and check the file (does not block the Tokio worker).
async fn read_document(bytes: Arc<Vec<u8>>) -> Result<ExchangeDocument, RpcError> {
    tokio::task::spawn_blocking(move || baruch_exchange::read::read(&bytes, &Limits::DEFAULT))
        .await
        .map_err(|e| RpcError::new(INTERNAL, format!("Lesen abgebrochen: {e}")))?
        .map_err(RpcError::from)
}

fn store_error(e: StoreError) -> RpcError {
    match e {
        StoreError::DuplicateTitle { .. } => RpcError::new(REFUSED, format!("{e} {TITLE_HINT}")),
        e => e.into(),
    }
}

/// Preview of a file (nothing stays written). Under the write lock so that the rolled-back planning
/// crosses no other write.
pub async fn preview_bytes(state: &AppState, bytes: Vec<u8>, title: Option<&str>) -> Result<ExchangeReport, RpcError> {
    let sha = baruch_exchange::sha256_hex(&bytes);
    let doc = read_document(Arc::new(bytes)).await?;
    let _held = state.write_lock.lock().await;
    state.store.exchange_preview(&doc, &sha, title).await.map_err(store_error)
}

/// Result of applying including the mirror.
#[derive(Debug, Clone)]
pub struct ImportOutcome {
    pub applied: ExchangeApplied,
    pub commit: Option<String>,
    pub mirror: Option<String>,
}

/// Applying: the checksum must match the file shown (otherwise `INVALID_PARAMS`), then plan anew and write in one
/// transaction (mode does not fit → `REFUSED`), then one commit in the mirror (except for a merge without
/// any change).
pub async fn import_bytes(
    state: &AppState,
    bytes: Vec<u8>,
    sha256: &str,
    mode: ApplyMode,
    title: Option<&str>,
) -> Result<ImportOutcome, RpcError> {
    let sha = baruch_exchange::sha256_hex(&bytes);
    if !sha.eq_ignore_ascii_case(sha256.trim()) {
        return Err(RpcError::new(
            INVALID_PARAMS,
            "Die Prüfsumme passt nicht zur gezeigten Datei; bitte die Vorschau neu holen.",
        ));
    }
    let doc = read_document(Arc::new(bytes)).await?;
    let held = state.write_lock.lock().await;
    let applied = state.store.exchange_apply(&doc, &sha, mode, title).await.map_err(store_error)?;
    let changed = applied.mode == ApplyMode::New || applied.report.add.total() > 0 || !applied.requests.is_empty();
    let (commit, mirror) = if changed {
        match mirror::record_exchange(state, &held, &applied).await {
            Ok(commit) => (commit, None),
            Err(e) => (None, Some(e)),
        }
    } else {
        (None, None)
    };
    Ok(ImportOutcome { applied, commit, mirror })
}

// ---- RPC -----------------------------------------------------------------------------------------

#[derive(Deserialize)]
struct PreviewParams {
    data: String,
    #[serde(default)]
    file_name: Option<String>,
    #[serde(default)]
    title: Option<String>,
}

#[derive(Deserialize)]
struct ImportParams {
    data: String,
    sha256: String,
    mode: String,
    #[serde(default)]
    title: Option<String>,
}

fn params<T: serde::de::DeserializeOwned>(value: Value) -> Result<T, RpcError> {
    serde_json::from_value(value).map_err(|e| RpcError::new(INVALID_PARAMS, format!("Parameter: {e}")))
}

fn decode(data: &str) -> Result<Vec<u8>, RpcError> {
    let compact: String = data.chars().filter(|c| !c.is_ascii_whitespace()).collect();
    baruch_store::decode_base64(&compact).map_err(|_| RpcError::new(INVALID_PARAMS, "data ist kein gültiges Base64."))
}

fn title_of(title: &Option<String>) -> Option<&str> {
    title.as_deref().map(str::trim).filter(|t| !t.is_empty())
}

/// RPC `work.exchange_preview {data: Base64, file_name?, title?}` → preview (form of the contract R26 §5).
pub async fn rpc_preview(state: &AppState, params_value: Value) -> Result<Value, RpcError> {
    let p: PreviewParams = params(params_value)?;
    let _ = p.file_name;
    let report = preview_bytes(state, decode(&p.data)?, title_of(&p.title)).await?;
    serde_json::to_value(&report).map_err(|e| RpcError::new(INTERNAL, e.to_string()))
}

/// RPC `work.exchange_import {data: Base64, sha256, mode: "new"|"merge", title?}`.
pub async fn rpc_import(state: &AppState, params_value: Value) -> Result<Value, RpcError> {
    let p: ImportParams = params(params_value)?;
    let mode =
        ApplyMode::parse(p.mode.trim()).ok_or_else(|| RpcError::new(INVALID_PARAMS, "mode ist „new“ oder „merge“."))?;
    let outcome = import_bytes(state, decode(&p.data)?, &p.sha256, mode, title_of(&p.title)).await?;
    Ok(import_answer(&outcome))
}

/// Answer of `work.exchange_import`.
pub fn import_answer(outcome: &ImportOutcome) -> Value {
    let applied = &outcome.applied;
    let mut answer = json!({
        "work": applied.work,
        "slug": applied.slug,
        "title": applied.title,
        "mode": applied.mode.as_str(),
        "added": applied.report.add,
        "requests": applied.requests,
        "deviations": applied.report.deviations,
        "deleted_here": applied.report.deleted_here,
        "refusals": applied.report.refusals,
        "unknown": applied.report.unknown,
    });
    if let Some(commit) = &outcome.commit {
        answer["commit"] = json!(commit);
    }
    if let Some(mirror) = &outcome.mirror {
        answer["mirror"] = json!(mirror);
    }
    answer
}

// ---- Inbox ---------------------------------------------------------------------------------------

/// SHA-256 as 64 lower-case hex? Only such names get into the inbox (no path from outside).
fn is_sha256(text: &str) -> bool {
    text.len() == 64 && text.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// `<data>/austausch-eingang`; no symbolic link.
fn inbox(data: &Path) -> Result<PathBuf, String> {
    let dir = data.join(INBOX_DIR);
    if std::fs::symlink_metadata(&dir).is_ok_and(|m| m.file_type().is_symlink()) {
        return Err(format!("{} ist eine symbolische Verknüpfung; dort wird nichts abgelegt", dir.display()));
    }
    Ok(dir)
}

/// Clear away entries older than [`INBOX_MAX_AGE`] (only files with SHA-256 names).
fn sweep(dir: &Path) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    let now = SystemTime::now();
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        if !is_sha256(name) {
            continue;
        }
        let old = entry
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| now.duration_since(t).ok())
            .is_some_and(|age| age > INBOX_MAX_AGE);
        if old {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

/// Put a file into the inbox (first clear away old ones); writes through a helper file and renames.
fn put_inbox(data: &Path, sha: &str, bytes: &[u8]) -> Result<(), String> {
    let dir = inbox(data)?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("{} lässt sich nicht anlegen: {e}", dir.display()))?;
    sweep(&dir);
    let target = dir.join(sha);
    let temp = dir.join(format!(".{sha}.{}", std::process::id()));
    std::fs::write(&temp, bytes).map_err(|e| format!("{} lässt sich nicht schreiben: {e}", temp.display()))?;
    std::fs::rename(&temp, &target).map_err(|e| format!("{} lässt sich nicht ablegen: {e}", target.display()))
}

fn take_inbox_path(data: &Path, sha: &str) -> Result<PathBuf, String> {
    Ok(inbox(data)?.join(sha))
}

// ---- Forms ---------------------------------------------------------------------------------------

/// Fields of the preview from the multipart form: file (`datei`), title, work of the header.
#[derive(Default)]
struct Upload {
    file: Option<(String, Vec<u8>)>,
    title: String,
    work: Option<i64>,
}

async fn read_upload(multipart: Result<Multipart, MultipartRejection>) -> Result<Upload, (StatusCode, String)> {
    let too_large = || {
        (
            StatusCode::PAYLOAD_TOO_LARGE,
            format!(
                "Die Datei ist größer als {} MiB; größere Werke über die Kommandozeile einlesen.",
                Limits::DEFAULT.max_file_bytes / (1024 * 1024)
            ),
        )
    };
    let mut multipart = multipart.map_err(|e| {
        (StatusCode::UNPROCESSABLE_ENTITY, format!("Das Formular kam nicht als Datei-Upload an ({}).", e.body_text()))
    })?;
    let mut upload = Upload::default();
    loop {
        let field = match multipart.next_field().await {
            Ok(Some(field)) => field,
            Ok(None) => break,
            Err(e) if e.status() == StatusCode::PAYLOAD_TOO_LARGE => return Err(too_large()),
            Err(e) => {
                return Err((
                    StatusCode::UNPROCESSABLE_ENTITY,
                    format!("Das Formular ließ sich nicht lesen ({}).", e.body_text()),
                ));
            }
        };
        match field.name() {
            Some("datei") => {
                let name = field.file_name().unwrap_or("").to_owned();
                match field.bytes().await {
                    Ok(bytes) => upload.file = Some((name, bytes.to_vec())),
                    Err(e) if e.status() == StatusCode::PAYLOAD_TOO_LARGE => return Err(too_large()),
                    Err(e) => {
                        return Err((
                            StatusCode::UNPROCESSABLE_ENTITY,
                            format!("Die Datei ließ sich nicht lesen ({}).", e.body_text()),
                        ));
                    }
                }
            }
            Some("title") => upload.title = field.text().await.unwrap_or_default(),
            Some("work") => upload.work = field.text().await.ok().and_then(|w| w.trim().parse().ok()),
            _ => {}
        }
    }
    if let Some((_, bytes)) = &upload.file
        && bytes.len() as u64 > Limits::DEFAULT.max_file_bytes
    {
        return Err(too_large());
    }
    Ok(upload)
}

/// Short description of a preview for templates: numbers as a list (kind, number) without zeros, the kind in the
/// singular for exactly 1 („1 Fassung“, „2 Fassungen“).
fn add_list(add: &baruch_store::exchange_apply::AddCounts) -> Vec<(String, usize)> {
    [
        ("Gliederungspunkt", "Gliederungspunkte", add.nodes),
        ("Fassung", "Fassungen", add.revisions),
        ("Vorlagenfassung", "Vorlagenfassungen", add.template_revisions),
        ("Einfall", "Einfälle", add.ideas),
        ("Beitrag", "Beiträge", add.posts),
        ("Faden", "Fäden", add.threads),
        ("Fadenbeitrag", "Fadenbeiträge", add.thread_posts),
        ("Bild", "Bilder", add.assets),
        ("Bildfassung", "Bildfassungen", add.asset_revisions),
        ("Verbindung der Gedankenkarte", "Verbindungen der Gedankenkarte", add.links),
        ("Lage auf der Gedankenkarte", "Lagen auf der Gedankenkarte", add.positions),
        ("Block der Umschlag-Anordnung", "Blöcke der Umschlag-Anordnung", add.cover_blocks),
        ("Befund", "Befunde", add.findings),
        ("Eintrag im Verlauf der Befunde", "Einträge im Verlauf der Befunde", add.finding_entries),
        ("alte Fassung eines Beitrags", "alte Fassungen von Beiträgen", add.post_versions),
        ("Satz-Einstellungen", "Satz-Einstellungen", add.typeset),
        ("eigenes Register", "eigene Register", add.registers),
        ("Glossarbegriff", "Glossarbegriffe", add.glossary_terms),
        // Round 22: continuations (also when fast-forwarding a known revision).
        ("Fortschreibung", "Fortschreibungen", add.amends),
    ]
    .into_iter()
    .filter(|(_, _, n)| *n > 0)
    .map(|(one, many, n)| ((if n == 1 { one } else { many }).to_owned(), n))
    .collect()
}

/// Deviations for templates, each with target address (`/?work=W&key=K[&rev=R]`) and word of the action.
fn deviation_rows(work: Option<i64>, report: &ExchangeReport) -> Vec<Value> {
    report
        .deviations
        .iter()
        .map(|d| {
            let href = match (work, d.key) {
                (Some(w), Some(k)) => Some(match d.rev {
                    Some(r) => format!("/?work={w}&key={k}&rev={r}"),
                    None => format!("/?work={w}&key={k}"),
                }),
                _ => None,
            };
            json!({
                "kind": d.kind,
                "label": d.label,
                "here": d.here,
                "there": d.there,
                "action": d.action.label(),
                "is_request": d.action == DeviationAction::Antrag,
                "href": href,
            })
        })
        .collect()
}

fn instance_text(report: &ExchangeReport) -> String {
    if report.instance.name.trim().is_empty() {
        format!("ohne Namen ({})", report.instance.uuid.chars().take(8).collect::<String>())
    } else {
        report.instance.name.trim().to_owned()
    }
}

/// `POST /austausch/vorschau` (multipart: `datei`, `title`, `work`).
async fn preview_form(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    multipart: Result<Multipart, MultipartRejection>,
) -> Response {
    let upload = match read_upload(multipart).await {
        Ok(upload) => upload,
        Err((status, message)) => return form_page(&headers, status, None, Some(message), ""),
    };
    let Some((file_name, bytes)) = upload.file.filter(|(name, bytes)| !name.trim().is_empty() || !bytes.is_empty())
    else {
        return form_page(
            &headers,
            StatusCode::UNPROCESSABLE_ENTITY,
            upload.work,
            Some("Bitte eine Austauschdatei (.baruch oder .baruch.xml) wählen.".into()),
            &upload.title,
        );
    };
    let title = Some(upload.title.trim()).filter(|t| !t.is_empty()).map(str::to_owned);
    let sha = baruch_exchange::sha256_hex(&bytes);
    let report = match preview_bytes(&state, bytes.clone(), title.as_deref()).await {
        Ok(report) => report,
        Err(e) => return form_page(&headers, status_of(&e), upload.work, Some(e.message), &upload.title),
    };
    let data = state.data.clone();
    let sha_for_inbox = sha.clone();
    let stored = tokio::task::spawn_blocking(move || put_inbox(&data, &sha_for_inbox, &bytes)).await;
    match stored {
        Ok(Ok(())) => {}
        Ok(Err(e)) => {
            return form_page(&headers, StatusCode::INTERNAL_SERVER_ERROR, upload.work, Some(e), &upload.title);
        }
        Err(e) => {
            return form_page(
                &headers,
                StatusCode::INTERNAL_SERVER_ERROR,
                upload.work,
                Some(e.to_string()),
                &upload.title,
            );
        }
    }
    let work = report.known_work.or(upload.work);
    let base = file_name.rsplit(['/', '\\']).next().unwrap_or("").to_owned();
    let ctx = minijinja::context! {
        settings => JinjaValue::from(Serde(settings_of(&headers))),
        work => work.map(|id| minijinja::context! { id => id }),
        file_name => base,
        sha256 => sha,
        mode => report.mode.as_str(),
        mode_label => report.mode.label(),
        instance => instance_text(&report),
        add => add_list(&report.add),
        add_total => report.add.total(),
        deviations => JinjaValue::from(Serde(deviation_rows(report.known_work, &report))),
        report => JinjaValue::from(Serde(&report)),
        newer => report.version > baruch_exchange::FORMAT_VERSION,
        title_value => title.unwrap_or_default(),
    };
    render("exchange_preview.html", StatusCode::OK, ctx)
}

/// Fields of `POST /austausch/uebernehmen`.
#[derive(Debug, Default, Deserialize)]
struct ImportForm {
    #[serde(default)]
    sha256: String,
    #[serde(default)]
    mode: String,
    #[serde(default)]
    title: String,
}

/// `POST /austausch/uebernehmen` (`sha256`, `mode`, `title`).
async fn import_form(State(state): State<Arc<AppState>>, headers: HeaderMap, Form(form): Form<ImportForm>) -> Response {
    let sha = form.sha256.trim().to_ascii_lowercase();
    if !is_sha256(&sha) {
        return form_page(
            &headers,
            StatusCode::UNPROCESSABLE_ENTITY,
            None,
            Some("Ungültige Prüfsumme; bitte die Datei neu wählen.".into()),
            "",
        );
    }
    let Some(mode) = ApplyMode::parse(form.mode.trim()) else {
        return form_page(
            &headers,
            StatusCode::UNPROCESSABLE_ENTITY,
            None,
            Some("Unbekannter Modus (neu oder abgleichen).".into()),
            "",
        );
    };
    let path = match take_inbox_path(&state.data, &sha) {
        Ok(path) => path,
        Err(e) => return form_page(&headers, StatusCode::INTERNAL_SERVER_ERROR, None, Some(e), ""),
    };
    let read_path = path.clone();
    let bytes = match tokio::task::spawn_blocking(move || std::fs::read(&read_path)).await {
        Ok(Ok(bytes)) => bytes,
        _ => {
            return form_page(
                &headers,
                StatusCode::GONE,
                None,
                Some("Die Datei der Vorschau ist nicht mehr da (nach 24 Stunden weggeräumt oder schon übernommen); bitte neu wählen.".into()),
                &form.title,
            );
        }
    };
    let title = Some(form.title.trim()).filter(|t| !t.is_empty()).map(str::to_owned);
    let outcome = import_bytes(&state, bytes, &sha, mode, title.as_deref()).await;
    // Afterwards (also on a refusal) no inbox file is left behind.
    let _ = tokio::task::spawn_blocking(move || std::fs::remove_file(&path)).await;
    let outcome = match outcome {
        Ok(outcome) => outcome,
        Err(e) => {
            let mut message = e.message.clone();
            message.push_str(" Die Datei bitte noch einmal wählen.");
            return form_page(&headers, status_of(&e), None, Some(message), &form.title);
        }
    };
    let applied = &outcome.applied;
    let report = &applied.report;
    let ctx = minijinja::context! {
        settings => JinjaValue::from(Serde(settings_of(&headers))),
        work => minijinja::context! { id => applied.work, title => applied.title.clone(), slug => applied.slug.clone() },
        mode_label => applied.mode.label(),
        instance => instance_text(report),
        add => add_list(&report.add),
        add_total => report.add.total(),
        requests => applied.requests.len(),
        deviations => JinjaValue::from(Serde(deviation_rows(Some(applied.work), report))),
        report => JinjaValue::from(Serde(report)),
        commit => outcome.commit.clone(),
        mirror => outcome.mirror.clone(),
    };
    render("exchange_done.html", StatusCode::OK, ctx)
}

// ---- CLI -----------------------------------------------------------------------------------------

/// Details of the command line for `import DATEI.baruch|DATEI.baruch.xml`.
#[derive(Debug, Clone, Default)]
pub struct CliImport {
    /// `--abgleichen`: the work is known here and is merged.
    pub merge: bool,
    /// `--vorschau`: write nothing, only output the preview.
    pub preview_only: bool,
    /// `--title`: own title when importing as a new work.
    pub title: Option<String>,
}

/// Read a file for the CLI: only a regular file (no FIFO, no folder), at most the limit of the file.
fn read_cli_file(path: &Path) -> Result<Vec<u8>, String> {
    use std::io::Read;
    let shown = path.display();
    let meta = std::fs::metadata(path).map_err(|e| format!("{shown} lässt sich nicht lesen: {e}"))?;
    if !meta.is_file() {
        return Err(format!("{shown} ist keine Datei"));
    }
    let limit = Limits::DEFAULT.max_file_bytes;
    let file = std::fs::File::open(path).map_err(|e| format!("{shown} lässt sich nicht lesen: {e}"))?;
    let mut bytes = Vec::new();
    file.take(limit + 1).read_to_end(&mut bytes).map_err(|e| format!("{shown} lässt sich nicht lesen: {e}"))?;
    if bytes.len() as u64 > limit {
        return Err(format!("{shown} ist größer als {} MiB", limit / (1024 * 1024)));
    }
    Ok(bytes)
}

/// Preview or result as text for the command line.
pub fn report_text(report: &ExchangeReport) -> String {
    let mut out = format!(
        "Austausch aus {} (ausgegeben {}, Unterversion {}), Datei {}\nModus: {} – Werk „{}“\n",
        instance_text(report),
        report.exported,
        report.version,
        report.sha256,
        report.mode.label(),
        report.title
    );
    let add = add_list(&report.add);
    if add.is_empty() {
        out.push_str("Hinzu: nichts\n");
    } else {
        let parts: Vec<String> = add.iter().map(|(name, n)| format!("{n} {name}")).collect();
        out.push_str(&format!("Hinzu: {}\n", parts.join(", ")));
    }
    if report.truncated {
        out.push_str(&format!("(Liste gekürzt auf {} Einträge)\n", report.items.len()));
    }
    if !report.deviations.is_empty() {
        out.push_str(&format!("Abweichungen ({}):\n", report.deviations.len()));
        for d in &report.deviations {
            let place = match (d.key, d.rev) {
                (Some(k), Some(r)) => format!(" [key {k}, Fassung {r}]"),
                (Some(k), None) => format!(" [key {k}]"),
                _ => String::new(),
            };
            out.push_str(&format!("  - {}: {}{place}\n", d.action.label(), d.label));
        }
    }
    if !report.requests.is_empty() {
        out.push_str(&format!(
            "Anträge ({}), entscheidet der Mensch unter Gliederung → Anträge:\n",
            report.requests.len()
        ));
        for r in &report.requests {
            out.push_str(&format!("  - {}\n", r.label));
        }
    }
    if !report.deleted_here.is_empty() {
        out.push_str(&format!("Hier gelöscht, kommt nicht wieder ({}):\n", report.deleted_here.len()));
        for d in &report.deleted_here {
            out.push_str(&format!("  - {}\n", d.label));
        }
    }
    if !report.unknown.is_empty() {
        out.push_str(&format!("Unbekannt, nicht übernommen: {}\n", report.unknown.join(", ")));
    }
    for refusal in &report.refusals {
        out.push_str(&format!("Nicht übernommen: {refusal}\n"));
    }
    out
}

/// CLI `import DATEI.baruch|DATEI.baruch.xml [--abgleichen] [--vorschau] [--title T]`: without flag and unknown →
/// new; without flag and known → error with a pointer to `--abgleichen`; `--abgleichen` for an unknown work →
/// error; `--vorschau` writes nothing.
pub async fn cli_import(state: &AppState, path: &std::path::Path, options: &CliImport) -> Result<String, String> {
    let owned = path.to_owned();
    let bytes = tokio::task::spawn_blocking(move || read_cli_file(&owned))
        .await
        .map_err(|e| format!("Lesen abgebrochen: {e}"))??;
    let sha = baruch_exchange::sha256_hex(&bytes);
    let title = options.title.as_deref().map(str::trim).filter(|t| !t.is_empty());
    let doc = read_document(Arc::new(bytes.clone())).await.map_err(|e| e.message)?;
    let known = state.store.work_by_uuid(&doc.work.uuid).await.map_err(|e| e.to_string())?;
    if options.preview_only {
        let report = preview_bytes(state, bytes, title).await.map_err(|e| e.message)?;
        return Ok(format!("Vorschau (nichts geschrieben)\n{}", report_text(&report)));
    }
    let mode = match (options.merge, known) {
        (false, None) => ApplyMode::New,
        (true, Some(_)) => ApplyMode::Merge,
        (false, Some(work)) => {
            let slug = state.store.work_slug(work).await.unwrap_or_default();
            return Err(format!(
                "Werk schon vorhanden: „{}“ ({slug}, id {work}). Mit --abgleichen abgleichen.",
                doc.work.title
            ));
        }
        (true, None) => {
            return Err(format!(
                "Das Werk „{}“ gibt es hier nicht; abgleichen geht nicht. Ohne --abgleichen als neues Werk einlesen.",
                doc.work.title
            ));
        }
    };
    let outcome = import_bytes(state, bytes, &sha, mode, title).await.map_err(|e| e.message)?;
    let applied = &outcome.applied;
    let mut text =
        format!("Werk {} ({}) „{}“\n{}", applied.work, applied.slug, applied.title, report_text(&applied.report));
    if !applied.requests.is_empty() {
        let ids: Vec<String> = applied.requests.iter().map(i64::to_string).collect();
        text.push_str(&format!("Angelegte Anträge: {}\n", ids.join(", ")));
    }
    match (&outcome.commit, &outcome.mirror) {
        (Some(commit), _) => text.push_str(&format!("Spiegel: Commit {commit}\n")),
        (None, Some(error)) => text.push_str(&format!("Spiegel: Fehler {error} (das Werk ist trotzdem übernommen)\n")),
        (None, None) => text.push_str("Spiegel: kein Commit (nichts geändert)\n"),
    }
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_sha256_names_reach_the_inbox() {
        // The name of the inbox file comes from the form: only 64 lower-case hex, otherwise a path would get out.
        assert!(is_sha256(&"a".repeat(64)), "64 Hex");
        assert!(!is_sha256("../../etc/passwd"), "Pfad");
        assert!(!is_sha256(&"A".repeat(64)), "groß nicht");
        assert!(!is_sha256(&"a".repeat(63)), "zu kurz");
    }
}
