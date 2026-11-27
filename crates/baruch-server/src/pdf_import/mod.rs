// SPDX-License-Identifier: AGPL-3.0-or-later
//! PDF/EPUB import with a preview of the outline (Round 21a; interface R21-S1, owned by pdf-import).
//!
//! A PDF or EPUB file is read, its outline shown as a suggestion and taken over as a work
//! ([`crate::import::import_generated`]) and/or as a source of the library only after explicit confirmation by
//! the human. Never silent: outline always with a preview, warnings instead of discarding, no silent change of way.
//! No import through MCP (decision 2026-11-05): an import creates current revisions under the name of the human.
//!
//! **Ways:** PDF through MultiGPT ([`remote`]: `upload_document` → indexing → `document_info` with
//! table of contents → `read_document` page by page) or locally ([`pdf`]: `pdfinfo`, `pdftotext -bbox-layout`,
//! outline by font size); EPUB always locally ([`epub`]: archive through [`crate::archive::read_zip`], OPF,
//! nav/NCX, XHTML). `way` „auto“: MultiGPT if a connection is set up and a collection is chosen (in the
//! form additionally only with the tick „in MultiGPT hochladen“), otherwise locally – the reason is in
//! `way_reason`. If MultiGPT fails, there is an error with the reason and the offer „lokal lesen“, no automatic
//! change.
//!
//! **Draft:** the preview only creates a draft under `<data>/import-drafts/<draft>/` ([`draft`]); work and
//! source are created only with `source_import.apply` or „Übernehmen“. The outline on accepting is the list
//! confirmed (possibly edited) by the human; a part only if the human confirms it as such (AGENTS.md,
//! addition 2026-11-07); the text of struck items goes to the previous item, never lost.
//!
//! - RPC (global, without a work): [`METHODS`] – see docs/RPC.md „source_import.*“.
//! - Pages: `GET /werke/import/quelle`, forms `POST /werke/import/quelle/vorschau|uebernehmen|verwerfen`
//!   ([`pages`], [`writes`], behind `guard::same_origin`).
//! - CLI: `baruch-server import DATEI.pdf|DATEI.epub [--uebernehmen] [--bibliothek] [--title …]` ([`cli_import`]),
//!   always local, without `--uebernehmen`/`--bibliothek` only a preview (nothing written, no draft either).

pub mod draft;
mod epub;
mod pages;
pub mod pdf;
pub mod remote;
mod typst_out;
mod xml;

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex};

use axum::Router;
use baruch_library::{NewTitle, RemoteRef, TitleText};
use baruch_store::{ConnectionKind, ImportAsset};
use serde::Deserialize;
use serde_json::{Value, json};

pub use pdf::PdfTools;

use crate::AppState;
use crate::import::{ImportReport, ImportWarning, SkippedAsset};
use crate::rpc::{INTERNAL, INVALID_PARAMS, REFUSED, RpcError};
use draft::{Draft, LoadError, Pending, StoredAsset, StoredSkipped, StoredWarning};
use typst_out::{Confirmed, Section};

/// Largest PDF or EPUB file the import accepts.
pub const MAX_SOURCE_FILE_BYTES: u64 = 64 * 1024 * 1024;

/// Limit of the request to `POST /werke/import/quelle/vorschau` (file including form fields).
pub const UPLOAD_LIMIT: usize = MAX_SOURCE_FILE_BYTES as usize + 64 * 1024;

/// Methods of this module.
pub const METHODS: &[&str] =
    &["source_import.preview", "source_import.get", "source_import.apply", "source_import.discard"];

/// Length of the text sample per item of the outline (characters).
pub const SAMPLE_CHARS: usize = 240;

pub fn handles(method: &str) -> bool {
    METHODS.contains(&method)
}

/// Document that was read (common to PDF local, PDF through MultiGPT and EPUB).
#[derive(Debug, Clone)]
pub(crate) struct Extracted {
    pub title: Option<String>,
    pub author: Option<String>,
    pub lang: Option<String>,
    pub pages: Option<usize>,
    pub sections: Vec<Section>,
    /// Images: (path in the work, file name, bytes).
    pub assets: Vec<(String, String, Vec<u8>)>,
    pub skipped: Vec<SkippedAsset>,
    pub warnings: Vec<ImportWarning>,
    /// Full text (library, local way).
    pub plain_text: String,
}

/// Shorten text to `max` characters (with „…“).
pub(crate) fn short(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        text.to_owned()
    } else {
        format!("{}…", text.chars().take(max).collect::<String>())
    }
}

/// Does the file name end (regardless of case) in `.pdf` or `.epub` and have a name before it?
pub fn is_source_name(file_name: &str) -> bool {
    let lower = file_name.to_lowercase();
    [".pdf", ".epub"].iter().any(|suffix| lower.len() > suffix.len() && lower.ends_with(suffix))
}

fn is_epub(file_name: &str) -> bool {
    file_name.to_lowercase().ends_with(".epub")
}

fn invalid(message: impl Into<String>) -> RpcError {
    RpcError::new(INVALID_PARAMS, message)
}

/// Way that the human chooses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Way {
    Auto,
    Multigpt,
    Local,
}

impl Way {
    pub fn parse(text: Option<&str>) -> Result<Way, RpcError> {
        match text.map(str::trim).filter(|t| !t.is_empty()) {
            None | Some("auto") => Ok(Way::Auto),
            Some("multigpt") => Ok(Way::Multigpt),
            Some("local") | Some("lokal") => Ok(Way::Local),
            Some(other) => Err(invalid(format!("way: „auto“, „multigpt“ oder „local“, nicht „{other}“."))),
        }
    }
}

/// Inputs of a preview.
#[derive(Debug, Clone)]
pub struct PreviewInput {
    pub file_name: String,
    pub bytes: Vec<u8>,
    pub way: Way,
    pub collection: Option<String>,
    pub title: Option<String>,
    /// Uploading to MultiGPT explicitly allowed (form: tick; RPC: always, the choice of way/collection
    /// is the consent).
    pub upload: bool,
}

/// Error of a preview: RPC error, plus the way (if already chosen) and – after an error of MultiGPT – a
/// draft that keeps the same file ready for „lokal lesen“.
#[derive(Debug)]
pub struct PreviewError {
    pub error: RpcError,
    pub way: Option<&'static str>,
    pub retry_local: Option<String>,
}

impl From<RpcError> for PreviewError {
    fn from(error: RpcError) -> Self {
        PreviewError { error, way: None, retry_local: None }
    }
}

/// Local reads with pdftotext per data folder (for tests: „kein pdftotext-Aufruf“ on the MultiGPT way).
static LOCAL_READS: LazyLock<Mutex<HashMap<PathBuf, usize>>> = LazyLock::new(Default::default);

/// How often this data folder has read a PDF locally (pdftotext).
pub fn local_pdf_reads(state: &AppState) -> usize {
    LOCAL_READS.lock().unwrap_or_else(|e| e.into_inner()).get(&state.data).copied().unwrap_or(0)
}

fn count_local_read(state: &AppState) {
    *LOCAL_READS.lock().unwrap_or_else(|e| e.into_inner()).entry(state.data.clone()).or_default() += 1;
}

/// Is a MultiGPT connection set up (address entered)?
pub async fn multigpt_configured(state: &AppState) -> bool {
    state
        .store
        .connection_get(ConnectionKind::Multigpt)
        .await
        .ok()
        .and_then(|row| row.config.get("url").and_then(Value::as_str).map(|u| !u.trim().is_empty()))
        .unwrap_or(false)
}

/// File name without folder.
fn base_name(file_name: &str) -> String {
    file_name.rsplit(['/', '\\']).next().unwrap_or("").trim().to_owned()
}

fn stem(file_name: &str) -> String {
    let name = base_name(file_name);
    name.rsplit_once('.').map_or(name.clone(), |(s, _)| s.to_owned()).trim().to_owned()
}

/// Choose the way: (way, reason) or an error.
async fn choose_way(state: &AppState, input: &PreviewInput) -> Result<(&'static str, String), RpcError> {
    if is_epub(&input.file_name) {
        return Ok((
            "local",
            "EPUB liest Baruch immer selbst: MultiGPT nimmt laut Werkzeugbeschreibung kein EPUB an.".into(),
        ));
    }
    let collection = input.collection.as_deref().map(str::trim).filter(|c| !c.is_empty());
    let configured = multigpt_configured(state).await;
    match input.way {
        Way::Local => {
            Ok(("local", "Lokal gewählt: pdftotext und pdfinfo auf diesem Rechner, nichts verlässt ihn.".into()))
        }
        Way::Multigpt => {
            if !configured {
                return Err(invalid(
                    "Keine MultiGPT-Verbindung eingerichtet (Einstellungen → Verbindungen); lokal lesen geht mit way \
                     „local“.",
                ));
            }
            let Some(collection) = collection else {
                return Err(invalid(
                    "Für den Weg MultiGPT eine Sammlung angeben (collection); dorthin wird die PDF hochgeladen.",
                ));
            };
            if !input.upload {
                return Err(invalid(format!(
                    "Hochladen nach MultiGPT bitte ausdrücklich ankreuzen („in MultiGPT hochladen (Sammlung {collection})“) \
                     – oder lokal lesen."
                )));
            }
            Ok((
                "multigpt",
                format!(
                    "Über MultiGPT gewählt: hochgeladen in die Sammlung „{collection}“, Gliederung aus dem Inhaltsverzeichnis von MultiGPT."
                ),
            ))
        }
        Way::Auto => match (configured, collection) {
            (false, _) => Ok(("local", "Keine MultiGPT-Verbindung eingerichtet – lokal gelesen (pdftotext).".into())),
            (true, None) => Ok((
                "local",
                "Keine Sammlung gewählt – lokal gelesen (pdftotext); zum Lesen über MultiGPT eine Sammlung wählen."
                    .into(),
            )),
            (true, Some(collection)) if !input.upload => Ok((
                "local",
                format!("Hochladen in die Sammlung „{collection}“ nicht angekreuzt – lokal gelesen (pdftotext)."),
            )),
            (true, Some(collection)) => Ok((
                "multigpt",
                format!(
                    "MultiGPT ist eingerichtet und die Sammlung „{collection}“ gewählt: dorthin hochgeladen, Gliederung aus dem Inhaltsverzeichnis von MultiGPT."
                ),
            )),
        },
    }
}

/// Preview: read the file, suggest an outline, create a draft (nothing else).
pub async fn preview(state: &AppState, input: PreviewInput) -> Result<Draft, PreviewError> {
    preview_with(state, input, &PdfTools::default()).await
}

/// Like [`preview`], with own programs for the local PDF way (tests).
pub async fn preview_with(state: &AppState, input: PreviewInput, tools: &PdfTools) -> Result<Draft, PreviewError> {
    let file_name = base_name(&input.file_name);
    if !is_source_name(&file_name) {
        return Err(invalid(format!("„{file_name}“ ist weder PDF noch EPUB (.pdf, .epub).")).into());
    }
    if input.bytes.len() as u64 > MAX_SOURCE_FILE_BYTES {
        return Err(
            invalid(format!("{file_name} ist größer als {} MiB.", MAX_SOURCE_FILE_BYTES / (1024 * 1024))).into()
        );
    }
    if input.bytes.is_empty() {
        return Err(invalid(format!("{file_name} ist leer.")).into());
    }
    let input = PreviewInput { file_name: file_name.clone(), ..input };
    let (way, way_reason) = choose_way(state, &input).await?;
    let data = state.data.clone();
    tokio::task::spawn_blocking(move || draft::clean(&data)).await.ok();
    let id = draft::new_id();
    let epub = is_epub(&file_name);
    let mut document = None;
    let extracted = if epub {
        let bytes = input.bytes.clone();
        tokio::task::spawn_blocking(move || epub::read(&bytes))
            .await
            .map_err(|e| RpcError::new(INTERNAL, format!("Lesen abgebrochen: {e}")))?
            .map_err(|e| PreviewError { error: invalid(e), way: Some("local"), retry_local: None })?
    } else if way == "local" {
        count_local_read(state);
        let dir = draft::root(&state.data).join(&id);
        let file = dir.join("source.pdf");
        let bytes = input.bytes.clone();
        let written = {
            let (dir, file) = (dir.clone(), file.clone());
            tokio::task::spawn_blocking(move || {
                std::fs::create_dir_all(&dir).and_then(|_| std::fs::write(&file, &bytes))
            })
            .await
            .map_err(|e| RpcError::new(INTERNAL, format!("Schreiben abgebrochen: {e}")))?
        };
        written.map_err(|e| RpcError::new(INTERNAL, format!("PDF lässt sich nicht ablegen: {e}")))?;
        let read = pdf::read(&file, tools).await;
        let _ = tokio::fs::remove_file(&file).await;
        match read {
            Ok(read) => read,
            Err(e) => {
                let _ = tokio::fs::remove_dir_all(&dir).await;
                return Err(PreviewError { error: RpcError::new(REFUSED, e), way: Some("local"), retry_local: None });
            }
        }
    } else {
        let collection = input.collection.as_deref().unwrap_or("").trim().to_owned();
        match remote::read(state, &input.bytes, &file_name, &collection).await {
            Ok((read, doc)) => {
                document = Some(doc);
                read
            }
            Err(e) => {
                // No silent change: error with a reason; the file stays ready for „lokal lesen“.
                let pending = Pending {
                    id: id.clone(),
                    created: draft::now(),
                    file_name: file_name.clone(),
                    title: input.title.clone(),
                    collection: Some(collection.clone()),
                };
                let data = state.data.clone();
                let bytes = input.bytes.clone();
                let saved = tokio::task::spawn_blocking(move || draft::save_pending(&data, &pending, &bytes)).await;
                let retry = matches!(saved, Ok(Ok(()))).then(|| id.clone());
                return Err(PreviewError {
                    error: RpcError::new(
                        REFUSED,
                        format!(
                            "MultiGPT (Weg multigpt): {} Nichts angelegt, kein Wechsel auf den lokalen Weg. Angebot: lokal \
                             lesen – dieselbe Datei mit way „local“ senden (pdftotext auf diesem Rechner){}.",
                            e.trim_end_matches('.').to_owned() + ".",
                            retry
                                .as_deref()
                                .map(|d| format!("; im Formular „Lokal lesen“ (Entwurf {d})"))
                                .unwrap_or_default()
                        ),
                    ),
                    way: Some("multigpt"),
                    retry_local: retry,
                });
            }
        }
    };
    let assets = extracted.assets.clone();
    let draft = Draft {
        id: id.clone(),
        created: draft::now(),
        file_name: file_name.clone(),
        kind: if epub { "epub".into() } else { "pdf".into() },
        way: way.into(),
        way_reason,
        title: extracted.title.clone(),
        author: extracted.author.clone(),
        lang: extracted.lang.clone(),
        pages: extracted.pages,
        sections: extracted.sections,
        warnings: extracted.warnings.iter().map(StoredWarning::from).collect(),
        skipped: extracted
            .skipped
            .iter()
            .map(|s| StoredSkipped { path: s.path.clone(), reason: s.reason.clone() })
            .collect(),
        assets: assets
            .iter()
            .map(|(path, file_name, _)| StoredAsset { path: path.clone(), file_name: file_name.clone() })
            .collect(),
        document,
        sha256: baruch_exchange::sha256_hex(&input.bytes),
        mime: if epub { "application/epub+zip".into() } else { "application/pdf".into() },
        plain_text: if way == "local" { extracted.plain_text } else { String::new() },
        wanted_title: input.title.map(|t| t.trim().to_owned()).filter(|t| !t.is_empty()),
    };
    let data = state.data.clone();
    let saved = draft.clone();
    tokio::task::spawn_blocking(move || draft::save(&data, &saved, &assets))
        .await
        .map_err(|e| RpcError::new(INTERNAL, format!("Schreiben abgebrochen: {e}")))?
        .map_err(|e| RpcError::new(INTERNAL, e))?;
    Ok(draft)
}

/// Title under which work or source would be created: own title, otherwise from the form of the preview,
/// otherwise from the details of the file, otherwise the file name.
fn proposed_title(draft: &Draft) -> String {
    draft
        .wanted_title
        .clone()
        .or_else(|| draft.title.clone().filter(|t| !t.trim().is_empty()))
        .unwrap_or_else(|| stem(&draft.file_name))
}

/// Answer of `source_import.preview` or `.get`.
pub fn preview_json(draft: &Draft) -> Value {
    let outline: Vec<Value> = draft
        .sections
        .iter()
        .enumerate()
        .map(|(index, s)| {
            json!({ "index": index, "level": s.level, "title": s.title, "chars": s.plain.chars().count(),
                "part": s.part, "sample": typst_out::sample(&s.plain, SAMPLE_CHARS) })
        })
        .collect();
    let mut answer = json!({
        "draft": draft.id,
        "file_name": draft.file_name,
        "kind": draft.kind,
        "way": draft.way,
        "way_reason": draft.way_reason,
        "title": proposed_title(draft),
        "author": draft.author,
        "outline": outline,
        "warnings": draft.warnings.iter().map(|w| json!({ "code": w.code, "message": w.message, "line": w.line })).collect::<Vec<_>>(),
        "skipped": draft.skipped.iter().map(|s| json!({ "path": s.path, "reason": s.reason })).collect::<Vec<_>>(),
        "assets": draft.assets.iter().map(|a| a.path.clone()).collect::<Vec<_>>(),
    });
    if let Some(pages) = draft.pages {
        answer["pages"] = json!(pages);
    }
    if let Some(doc) = &draft.document {
        answer["document"] = json!({ "collection": doc.collection, "id": doc.id });
    }
    answer
}

/// Target of an accept.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    Work,
    Library,
    Both,
}

impl Target {
    pub fn parse(text: Option<&str>) -> Result<Target, RpcError> {
        match text.map(str::trim).filter(|t| !t.is_empty()) {
            None | Some("work") => Ok(Target::Work),
            Some("library") => Ok(Target::Library),
            Some("both") => Ok(Target::Both),
            Some(other) => Err(invalid(format!("target: „work“, „library“ oder „both“, nicht „{other}“."))),
        }
    }
    fn work(self) -> bool {
        matches!(self, Target::Work | Target::Both)
    }
    fn library(self) -> bool {
        matches!(self, Target::Library | Target::Both)
    }
}

/// Result of an accept.
#[derive(Debug, Clone)]
pub struct Applied {
    pub report: Option<ImportReport>,
    pub library_title: Option<i64>,
}

/// Drafts that are being accepted right now (twice at once → refused).
static APPLYING: LazyLock<Mutex<HashSet<String>>> = LazyLock::new(Default::default);

struct ApplyingGuard(String);

impl Drop for ApplyingGuard {
    fn drop(&mut self) {
        APPLYING.lock().unwrap_or_else(|e| e.into_inner()).remove(&self.0);
    }
}

/// Citation key from a title: `q-` and lower-case letters/digits/hyphens (at most 40 characters).
fn key_base(title: &str) -> String {
    let mut slug = String::new();
    for c in title.chars().flat_map(char::to_lowercase) {
        let part = match c {
            'ä' => "ae".to_owned(),
            'ö' => "oe".to_owned(),
            'ü' => "ue".to_owned(),
            'ß' => "ss".to_owned(),
            c if c.is_ascii_alphanumeric() => c.to_string(),
            _ => "-".to_owned(),
        };
        if part == "-" && (slug.is_empty() || slug.ends_with('-')) {
            continue;
        }
        slug.push_str(&part);
        if slug.len() >= 36 {
            break;
        }
    }
    let slug = slug.trim_end_matches('-');
    if slug.is_empty() { "q-quelle".to_owned() } else { format!("q-{slug}") }
}

/// Check the confirmed outline: titles, levels, parts.
fn check_confirmed(confirmed: &[Confirmed]) -> Result<(), RpcError> {
    if confirmed.is_empty() {
        return Err(invalid("Die Gliederung ist leer: mindestens ein Punkt (Kapitel) ist nötig."));
    }
    Ok(())
}

/// Accept: create the work (through [`crate::import::import_generated`]) and/or the source of the library. On an
/// error nothing new is created (the library is checked before the work).
pub async fn apply_draft(
    state: &AppState,
    draft: &Draft,
    assets: Vec<(String, String, Vec<u8>)>,
    target: Target,
    title: Option<&str>,
    author: Option<&str>,
    confirmed: Option<Vec<Confirmed>>,
) -> Result<Applied, RpcError> {
    let title = title.map(str::trim).filter(|t| !t.is_empty()).map_or_else(|| proposed_title(draft), str::to_owned);
    let author = author.map(str::trim).map(str::to_owned).or_else(|| draft.author.clone()).filter(|a| !a.is_empty());
    let pool = state.store.pool();
    // Check the library first: if the file is already there as a source, no work is created either.
    if target.library() {
        let existing = match &draft.document {
            Some(doc) => {
                let remote = RemoteRef {
                    way: crate::literature::FULLTEXT_WAY.into(),
                    collection: doc.collection.clone(),
                    document: doc.id.clone(),
                };
                baruch_library::title_for_remote(pool, &remote).await?
            }
            None => baruch_library::title_for_text_sha256(pool, &draft.sha256).await?,
        };
        if let Some(id) = existing {
            return Err(RpcError::new(
                REFUSED,
                format!("Diese Datei steht schon als Quelle in der Bibliothek (Titel {id}); nichts angelegt."),
            ));
        }
    }
    let confirmed = match confirmed {
        Some(list) => list,
        None => draft
            .sections
            .iter()
            .enumerate()
            .map(|(i, s)| Confirmed { level: s.level, title: s.title.clone(), part: false, source: Some(i) })
            .collect(),
    };
    check_confirmed(&confirmed)?;
    let mut report = None;
    if target.work() {
        let (typst, merge_warnings) =
            typst_out::build_typst(&title, author.as_deref(), draft.lang.as_deref(), &draft.sections, &confirmed)
                .map_err(invalid)?;
        let mut warnings = draft.warnings();
        warnings.extend(merge_warnings);
        let import_assets: Vec<ImportAsset> =
            assets.into_iter().map(|(path, file_name, bytes)| ImportAsset { path, file_name, bytes }).collect();
        report = Some(
            crate::import::import_generated(
                state,
                &typst,
                &draft.file_name,
                Some(&title),
                import_assets,
                draft.skipped(),
                warnings,
            )
            .await?,
        );
    }
    let mut library_title = None;
    if target.library() {
        let how = match &draft.document {
            Some(doc) => format!("über MultiGPT gelesen, Sammlung „{}“, Dokument {}", doc.collection, doc.id),
            None => "lokal gelesen, ohne MultiGPT".to_owned(),
        };
        let note = format!(
            "Importiert aus {} ({}, {how}).",
            draft.file_name,
            if draft.kind == "epub" { "EPUB" } else { "PDF" }
        );
        let id = {
            let _held = state.write_lock.lock().await;
            let base = key_base(&title);
            let mut key = base.clone();
            let mut n = 2;
            loop {
                let taken: Option<i64> = sqlx::query_scalar("SELECT id FROM library_title WHERE key = ?1")
                    .bind(&key)
                    .fetch_optional(pool)
                    .await
                    .map_err(|e| RpcError::new(INTERNAL, format!("Bibliothek: {e}")))?;
                if taken.is_none() {
                    break;
                }
                key = format!("{base}-{n}");
                n += 1;
            }
            let new = NewTitle {
                key,
                kind: "book".into(),
                title: title.clone(),
                authors: author.clone().unwrap_or_default(),
                year: None,
                publisher: String::new(),
                place: String::new(),
                url: String::new(),
                note,
            };
            let id = baruch_library::add_title(pool, &new).await?;
            match &draft.document {
                Some(doc) => {
                    let remote = RemoteRef {
                        way: crate::literature::FULLTEXT_WAY.into(),
                        collection: doc.collection.clone(),
                        document: doc.id.clone(),
                    };
                    baruch_library::set_title_remote(pool, id, &remote).await?;
                }
                None => {
                    let text = TitleText {
                        source_name: draft.file_name.clone(),
                        mime: draft.mime.clone(),
                        sha256: draft.sha256.clone(),
                        pages: draft.pages.map(|p| p as i64),
                        text: draft.plain_text.clone(),
                    };
                    baruch_library::set_title_text(pool, id, &text).await?;
                }
            }
            id
        };
        crate::rag::notify(state, None);
        library_title = Some(id);
    }
    Ok(Applied { report, library_title })
}

async fn load_draft(state: &AppState, id: &str) -> Result<Draft, RpcError> {
    let data = state.data.clone();
    let owned = id.to_owned();
    let loaded = tokio::task::spawn_blocking(move || draft::load(&data, &owned))
        .await
        .map_err(|e| RpcError::new(INTERNAL, format!("Lesen abgebrochen: {e}")))?;
    loaded.map_err(|e| match e {
        LoadError::Unknown => invalid(format!(
            "Entwurf „{id}“ gibt es nicht (verworfen, schon übernommen oder älter als 24 Stunden); die Datei bitte neu lesen."
        )),
        LoadError::Pending => invalid(format!(
            "Entwurf „{id}“ hat keine Vorschau: MultiGPT scheiterte; die Datei lokal lesen (way „local“)."
        )),
        LoadError::Broken(m) => RpcError::new(INTERNAL, m),
    })
}

/// Accept a stored draft; afterwards the draft is gone.
pub async fn apply(
    state: &AppState,
    id: &str,
    target: Target,
    title: Option<&str>,
    author: Option<&str>,
    confirmed: Option<Vec<Confirmed>>,
) -> Result<Applied, RpcError> {
    let draft = load_draft(state, id).await?;
    {
        let mut applying = APPLYING.lock().unwrap_or_else(|e| e.into_inner());
        if !applying.insert(id.to_owned()) {
            return Err(RpcError::new(REFUSED, format!("Entwurf „{id}“ wird gerade übernommen.")));
        }
    }
    let _guard = ApplyingGuard(id.to_owned());
    let data = state.data.clone();
    let loaded = draft.clone();
    let assets = tokio::task::spawn_blocking(move || draft::load_assets(&data, &loaded))
        .await
        .map_err(|e| RpcError::new(INTERNAL, format!("Lesen abgebrochen: {e}")))?
        .map_err(|e| RpcError::new(INTERNAL, e))?;
    let applied = apply_draft(state, &draft, assets, target, title, author, confirmed).await?;
    let data = state.data.clone();
    let owned = id.to_owned();
    let _ = tokio::task::spawn_blocking(move || draft::remove(&data, &owned)).await;
    Ok(applied)
}

/// Discard a draft.
pub async fn discard(state: &AppState, id: &str) -> Result<bool, RpcError> {
    if !draft::valid_id(id) {
        return Err(invalid("draft: 32 Kleinbuchstaben-Hex (aus source_import.preview)."));
    }
    let data = state.data.clone();
    let owned = id.to_owned();
    tokio::task::spawn_blocking(move || draft::remove(&data, &owned))
        .await
        .map_err(|e| RpcError::new(INTERNAL, format!("Löschen abgebrochen: {e}")))
}

/// Number of drafts in the data folder (tests, display).
pub fn draft_count(state: &AppState) -> usize {
    draft::count(&state.data)
}

// ---- RPC -------------------------------------------------------------------------------------------------------

fn params<T: for<'de> Deserialize<'de>>(value: Value) -> Result<T, RpcError> {
    let value = if value.is_null() { json!({}) } else { value };
    serde_json::from_value(value).map_err(|e| invalid(e.to_string()))
}

#[derive(Deserialize)]
struct PreviewParams {
    file_name: String,
    content: String,
    way: Option<String>,
    collection: Option<String>,
    title: Option<String>,
}

#[derive(Deserialize)]
struct DraftParams {
    draft: String,
}

#[derive(Deserialize)]
struct OutlineItem {
    level: u8,
    title: String,
    #[serde(default)]
    part: bool,
    source: Option<usize>,
}

#[derive(Deserialize)]
struct ApplyParams {
    draft: String,
    target: Option<String>,
    title: Option<String>,
    author: Option<String>,
    outline: Option<Vec<OutlineItem>>,
}

/// Answer of an accept (RPC).
pub fn applied_json(applied: &Applied) -> Value {
    let mut answer = json!({});
    if let Some(report) = &applied.report {
        answer["report"] = serde_json::to_value(report).unwrap_or(Value::Null);
    }
    if let Some(id) = applied.library_title {
        answer["library_title"] = json!(id);
    }
    answer
}

/// RPC `source_import.*`.
pub async fn call(state: &AppState, method: &str, params_value: Value) -> Result<Value, RpcError> {
    match method {
        "source_import.preview" => {
            let p: PreviewParams = params(params_value)?;
            let max_base64 = MAX_SOURCE_FILE_BYTES.div_ceil(3) * 4 + 4;
            if p.content.len() as u64 > max_base64 {
                return Err(invalid(format!(
                    "content: Datei größer als {} MiB.",
                    MAX_SOURCE_FILE_BYTES / (1024 * 1024)
                )));
            }
            let bytes = baruch_store::decode_base64(p.content.trim()).map_err(|e| invalid(format!("content: {e}")))?;
            let input = PreviewInput {
                file_name: p.file_name,
                bytes,
                way: Way::parse(p.way.as_deref())?,
                collection: p.collection,
                title: p.title,
                upload: true,
            };
            let draft = preview(state, input).await.map_err(|e| e.error)?;
            Ok(preview_json(&draft))
        }
        "source_import.get" => {
            let p: DraftParams = params(params_value)?;
            Ok(preview_json(&load_draft(state, p.draft.trim()).await?))
        }
        "source_import.apply" => {
            let p: ApplyParams = params(params_value)?;
            let target = Target::parse(p.target.as_deref())?;
            let confirmed = p.outline.map(|items| {
                items
                    .into_iter()
                    .map(|i| Confirmed { level: i.level, title: i.title, part: i.part, source: i.source })
                    .collect::<Vec<_>>()
            });
            let applied =
                apply(state, p.draft.trim(), target, p.title.as_deref(), p.author.as_deref(), confirmed).await?;
            Ok(applied_json(&applied))
        }
        "source_import.discard" => {
            let p: DraftParams = params(params_value)?;
            Ok(json!({ "discarded": discard(state, p.draft.trim()).await? }))
        }
        other => Err(RpcError::new(crate::rpc::METHOD_NOT_FOUND, format!("unknown method {other}"))),
    }
}

// ---- CLI -------------------------------------------------------------------------------------------------------

/// Options of the CLI: `--title`, `--uebernehmen` (`apply`), `--bibliothek` (`library`).
#[derive(Debug, Clone, Default)]
pub struct CliSourceImport {
    pub title: Option<String>,
    pub apply: bool,
    pub library: bool,
}

/// `import DATEI.pdf|DATEI.epub`: `Ok(text)` → standard output (exit 0), `Err(text)` → standard error (exit 1).
/// Always local (without MultiGPT). Without `--uebernehmen` and `--bibliothek` only a preview: outline and
/// warnings, nothing written (no draft either). `--uebernehmen` creates the work with exactly the outline shown,
/// `--bibliothek` the source (with `--uebernehmen` in addition to the work).
pub async fn cli_import(state: &AppState, file: &Path, options: &CliSourceImport) -> Result<String, String> {
    cli_import_with(state, file, options, &PdfTools::default()).await
}

/// Like [`cli_import`], with own programs for PDF (tests: „pdftotext fehlt“).
pub async fn cli_import_with(
    state: &AppState,
    file: &Path,
    options: &CliSourceImport,
    tools: &PdfTools,
) -> Result<String, String> {
    let shown = file.display().to_string();
    let meta = tokio::fs::metadata(file).await.map_err(|e| format!("{shown} lässt sich nicht lesen: {e}"))?;
    if !meta.is_file() {
        return Err(format!("{shown} ist keine Datei"));
    }
    if meta.len() > MAX_SOURCE_FILE_BYTES {
        return Err(format!("{shown} ist größer als {} MiB", MAX_SOURCE_FILE_BYTES / (1024 * 1024)));
    }
    let file_name = file
        .file_name()
        .and_then(|n| n.to_str())
        .map(str::to_owned)
        .ok_or_else(|| format!("{shown}: Dateiname fehlt oder ist kein UTF-8"))?;
    let bytes = tokio::fs::read(file).await.map_err(|e| format!("{shown} lässt sich nicht lesen: {e}"))?;
    let epub = is_epub(&file_name);
    let extracted = if epub {
        let owned = bytes.clone();
        tokio::task::spawn_blocking(move || epub::read(&owned))
            .await
            .map_err(|e| format!("Lesen abgebrochen: {e}"))??
    } else {
        count_local_read(state);
        pdf::read(file, tools).await?
    };
    let draft = Draft {
        id: draft::new_id(),
        created: draft::now(),
        file_name: file_name.clone(),
        kind: if epub { "epub".into() } else { "pdf".into() },
        way: "local".into(),
        way_reason: "Die Kommandozeile liest immer lokal, ohne MultiGPT.".into(),
        title: extracted.title.clone(),
        author: extracted.author.clone(),
        lang: extracted.lang.clone(),
        pages: extracted.pages,
        sections: extracted.sections.clone(),
        warnings: extracted.warnings.iter().map(StoredWarning::from).collect(),
        skipped: extracted
            .skipped
            .iter()
            .map(|s| StoredSkipped { path: s.path.clone(), reason: s.reason.clone() })
            .collect(),
        assets: extracted
            .assets
            .iter()
            .map(|(p, f, _)| StoredAsset { path: p.clone(), file_name: f.clone() })
            .collect(),
        document: None,
        sha256: baruch_exchange::sha256_hex(&bytes),
        mime: if epub { "application/epub+zip".into() } else { "application/pdf".into() },
        plain_text: extracted.plain_text.clone(),
        wanted_title: options.title.clone().filter(|t| !t.trim().is_empty()),
    };
    let mut out = String::new();
    let kind = if epub { "EPUB" } else { "PDF" };
    out.push_str(&format!("{kind} lokal gelesen: {file_name}"));
    if let Some(pages) = draft.pages {
        out.push_str(&format!(" ({pages} {})", if pages == 1 { "Seite" } else { "Seiten" }));
    }
    out.push('\n');
    out.push_str(&format!("Titel: {}\n", proposed_title(&draft)));
    if let Some(author) = &draft.author {
        out.push_str(&format!("Autor: {author}\n"));
    }
    out.push_str("Gliederung:\n");
    for s in &draft.sections {
        out.push_str(&format!(
            "  {}{} {}  ({} Zeichen){}\n",
            "  ".repeat(usize::from(s.level.saturating_sub(1))),
            "=".repeat(usize::from(s.level)),
            s.title,
            s.plain.chars().count(),
            if s.part { " – möglicher Teil (als Teil nur über die Oberfläche bestätigen)" } else { "" }
        ));
    }
    for w in &draft.warnings {
        out.push_str(&format!("Hinweis ({}): {}\n", w.code, w.message));
    }
    for s in &draft.skipped {
        out.push_str(&format!("Bild übersprungen: {} ({})\n", s.path, s.reason));
    }
    let target = match (options.apply, options.library) {
        (false, false) => {
            out.push_str(
                "Nichts angelegt (nur Vorschau). Mit --uebernehmen als Werk anlegen, mit --bibliothek als Quelle der \
                 Bibliothek.\n",
            );
            return Ok(out);
        }
        (true, false) => Target::Work,
        (false, true) => Target::Library,
        (true, true) => Target::Both,
    };
    let applied = apply_draft(state, &draft, extracted.assets, target, None, None, None)
        .await
        .map_err(|e| format!("{out}Import von {shown} gescheitert: {}", e.message))?;
    if let Some(report) = &applied.report {
        out.push_str(&format!(
            "Werk {} angelegt: „{}“ (slug {}), {} {}, {} {}\n",
            report.work,
            report.title,
            report.slug,
            report.nodes,
            if report.nodes == 1 { "Abschnitt" } else { "Abschnitte" },
            report.assets.len(),
            if report.assets.len() == 1 { "Bild" } else { "Bilder" }
        ));
        match (&report.commit, &report.mirror) {
            (Some(commit), _) => out.push_str(&format!("Spiegel-Commit: {commit}\n")),
            (None, Some(e)) => out.push_str(&format!("Spiegel: {e}\n")),
            (None, None) => {}
        }
    }
    if let Some(id) = applied.library_title {
        out.push_str(&format!("Quelle {id} in der Bibliothek angelegt (Volltext lokal, ohne MultiGPT)\n"));
    }
    Ok(out)
}

// ---- Pages -----------------------------------------------------------------------------------------------------

/// Pages (GET), merged in `lib.rs` with the pages.
pub fn pages() -> Router<Arc<AppState>> {
    pages::pages()
}

/// Forms (POST), merged in `lib.rs` into the writes router behind `guard::same_origin`.
pub fn writes() -> Router<Arc<AppState>> {
    pages::writes()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_names_end_in_pdf_or_epub_with_a_name_before() {
        assert!(is_source_name("buch.pdf"), ".pdf");
        assert!(is_source_name("BUCH.EPUB"), "Groß/klein egal: Dateien anderer Systeme heißen auch einmal so");
        assert!(!is_source_name(".pdf"), "ohne Namen davor ist es keine Datei, die sich benennen ließe");
        assert!(!is_source_name("buch.pdf.typ"), "entscheidend ist die letzte Endung");
        assert!(!is_source_name("buch.md"), "Markdown gehört zu markdown_import");
    }

    #[test]
    fn library_keys_are_plain_ascii() {
        assert_eq!(key_base("Möwen über dem Kai!"), "q-moewen-ueber-dem-kai", "Umlaute umschrieben, Satzzeichen weg");
        assert_eq!(key_base("¿?"), "q-quelle", "ohne Buchstaben ein fester Name");
    }
}
