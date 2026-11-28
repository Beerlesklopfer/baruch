// SPDX-License-Identifier: AGPL-3.0-or-later
//! Checking the EPUB (Round 20c, owner “epub-pruefung”): own structure and accessibility check, warnings per
//! reader, EPUBCheck (only at the human's request, never through MCP) and its setting.
//!
//! - **RPC** ([`handles`], [`call`]): `epub.check {work, readers?, epubcheck?, footnotes?}` → `{findings, xml_tool,
//!   epubcheck, summary, readers}`; without a work ([`handles_global`], [`call_global`]): `epub.checker_get {}`,
//!   `epub.checker_set {jar}` → `{jar, found, java}`.
//! - **Pages** ([`pages`]): `GET /werke/{id}/epub/pruefung[?leser=…&fussnoten=…]` – check report without EPUBCheck
//!   with button „EPUBCheck ausführen“; **forms** ([`writes`], behind `guard::same_origin`): `POST /werke/epub/pruefen`
//!   (fields `work`, `leser`*, `fussnoten`) → 200 with the report including EPUBCheck, `POST
//!   /einstellungen/daten/epubcheck` (field `jar`, empty = remove) → 303 to `/einstellungen/daten?ok=epubcheck` or 422.
//! - [`preview_panel`]: check report as an HTML fragment below the EPUB preview (without EPUBCheck).
//!
//! Flow ([`run_check`]): build and pack the EPUB as for the download (`crate::epub::book_for`), then
//! [`baruch_epub::check::check_structure`] (xmllint from PATH, otherwise quick-xml), the warnings of the build
//! (`EpubBook::warnings`, e.g. `ISLAND_OMITTED`, `TEMPLATE_PAGEBREAK_*`) as findings, per reader
//! [`baruch_epub::check::reader_warnings`] and – only on request – EPUBCheck: `epubcheck` from PATH before the
//! configured epubcheck.jar (`java -jar`), at most one run at a time, time limit [`EPUBCHECK_TIMEOUT`], the EPUB
//! in its own temp folder that is removed afterwards (Round 23: random name of 32 hex characters, on Unix
//! mode 0700, never an already existing folder or symlink – see [`TempDir`]). Nothing is downloaded; without both
//! `EpubcheckStatus::NotFound`, and the page says „EPUBCheck nicht gefunden“.

use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use axum::Router;
use axum::extract::{Form, Path as UrlPath, Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use baruch_epub::FootnoteMode;
use baruch_epub::check::{
    CheckReport, EpubcheckStatus, EpubcheckTool, Finding, Severity, XmlTool, check_structure, find_epubcheck,
    find_program, find_xmllint, reader_warnings, run_epubcheck,
};
use baruch_epub::readers::{PROFILES, ReaderProfile, profile};
use baruch_store::{StoreError, WorkId};
use minijinja::value::Serde;
use minijinja::{Environment, Value as Jinja, context};
use serde::Serialize;
use serde_json::{Value, json};

use crate::AppState;
use crate::rpc::{INTERNAL, INVALID_PARAMS, METHOD_NOT_FOUND, REFUSED, RpcError};
use crate::view::Settings;

/// Time limit for one run of EPUBCheck.
pub const EPUBCHECK_TIMEOUT: Duration = Duration::from_secs(180);
/// Maximum length of the path to the epubcheck.jar (bytes).
pub const MAX_JAR_PATH: usize = 4096;

/// Work-related methods of this module.
pub fn handles(method: &str) -> bool {
    method == "epub.check"
}

/// Work-less methods of this module (setting for EPUBCheck).
pub fn handles_global(method: &str) -> bool {
    matches!(method, "epub.checker_get" | "epub.checker_set")
}

fn invalid(message: impl Into<String>) -> RpcError {
    RpcError::new(INVALID_PARAMS, message)
}

/// PATH of the process (read only).
fn process_path() -> Option<OsString> {
    std::env::var_os("PATH")
}

// ---- Check -------------------------------------------------------------------------------------------

/// Number of findings per severity.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct Summary {
    pub errors: usize,
    pub warnings: usize,
    pub infos: usize,
}

impl Summary {
    pub fn of(findings: &[Finding]) -> Summary {
        let count = |s: Severity| findings.iter().filter(|f| f.severity == s).count();
        Summary { errors: count(Severity::Error), warnings: count(Severity::Warning), infos: count(Severity::Info) }
    }
}

/// Result of [`run_check`].
#[derive(Debug, Clone)]
pub struct Checked {
    pub report: CheckReport,
    pub readers: Vec<&'static ReaderProfile>,
    pub summary: Summary,
}

impl Checked {
    /// Answer of `epub.check` or MCP `epub_check`.
    pub fn to_json(&self) -> Value {
        json!({
            "findings": self.report.findings, "xml_tool": self.report.xml_tool, "epubcheck": self.report.epubcheck,
            "summary": self.summary, "readers": self.readers.iter().map(|r| r.slug).collect::<Vec<_>>(),
        })
    }
}

/// Mandatory profiles (default if no readers are named).
pub fn default_readers() -> Vec<&'static ReaderProfile> {
    PROFILES.iter().filter(|p| p.required).collect()
}

/// Readers by slug; unknown → `INVALID_PARAMS`; empty → mandatory profiles.
pub fn readers_of(slugs: &[String]) -> Result<Vec<&'static ReaderProfile>, RpcError> {
    if slugs.is_empty() {
        return Ok(default_readers());
    }
    let mut out: Vec<&'static ReaderProfile> = Vec::new();
    for slug in slugs {
        let p = profile(slug.trim()).ok_or_else(|| {
            let known: Vec<&str> = PROFILES.iter().map(|p| p.slug).collect();
            invalid(format!("Unbekannter Reader „{slug}“ (bekannt: {}).", known.join(", ")))
        })?;
        if !out.iter().any(|o| o.slug == p.slug) {
            out.push(p);
        }
    }
    Ok(out)
}

/// Warning of the build as a finding: `…_MAPPED` (e.g. `TEMPLATE_PAGEBREAK_MAPPED`) is a note, everything else a
/// warning.
fn build_warning(w: &baruch_epub::EpubWarning) -> Finding {
    let severity = if w.code.ends_with("_MAPPED") { Severity::Info } else { Severity::Warning };
    Finding {
        code: w.code.clone(),
        severity,
        message: w.message.clone(),
        path: w.key.map(|k| format!("Abschnitt {k}")),
        reader: None,
    }
}

/// Prefix of the temp folder for EPUBCheck; behind it stand [`TEMP_NAME_HEX`] random hex characters.
const TEMP_PREFIX: &str = "baruch-epubcheck-";
/// Random hex characters in the name of the temp folder (16 bytes from the system's random generator).
const TEMP_NAME_HEX: usize = 32;

/// Own temp folder for the run of EPUBCheck; removed when dropped.
///
/// Round 23 (review R20): the name was `baruch-epubcheck-<pid>-<n>` – predictable and with the permissions of the
/// umask. On a machine with several users another one could read the unpublished EPUB or create the
/// folder in advance. Now: random name ([`TEMP_NAME_HEX`] hex characters), on Unix mode 0700 right at
/// creation, and it is created without `recursive` – if something is already there (folder, file, symlink), that
/// is an error and it is not used (and not touched when cleaning up).
struct TempDir(PathBuf);

impl TempDir {
    fn new() -> std::io::Result<TempDir> {
        TempDir::create(&std::env::temp_dir().join(TempDir::random_name()?))
    }

    /// `baruch-epubcheck-` + 32 random hex characters.
    fn random_name() -> std::io::Result<String> {
        let mut bytes = [0u8; TEMP_NAME_HEX / 2];
        getrandom::getrandom(&mut bytes).map_err(|e| std::io::Error::other(format!("Zufallszahlen: {e}")))?;
        let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
        Ok(format!("{TEMP_PREFIX}{hex}"))
    }

    /// Create exactly this folder anew (0700 on Unix); an existing entry of the same name is an error.
    fn create(dir: &Path) -> std::io::Result<TempDir> {
        let mut builder = std::fs::DirBuilder::new();
        builder.recursive(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder.create(dir)?;
        Ok(TempDir(dir.to_path_buf()))
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Run EPUBCheck with `bytes` (at most one run at a time).
async fn epubcheck(tool: EpubcheckTool, bytes: Vec<u8>, timeout: Duration) -> (EpubcheckStatus, Vec<Finding>) {
    static ONE: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(1);
    let Ok(_turn) = ONE.acquire().await else {
        return (EpubcheckStatus::Failed { message: "EPUBCheck: Warteschlange geschlossen".into() }, Vec::new());
    };
    let run_tool = tool.clone();
    let result = tokio::task::spawn_blocking(crate::i18n::carry(move || {
        let dir = TempDir::new().map_err(|e| format!("Temp-Ordner für EPUBCheck: {e}"))?;
        let file = dir.0.join("buch.epub");
        std::fs::write(&file, &bytes).map_err(|e| format!("EPUB für EPUBCheck schreiben: {e}"))?;
        run_epubcheck(&run_tool, &file, timeout)
    }))
    .await
    .unwrap_or_else(|e| Err(format!("EPUBCheck: {e}")));
    match result {
        Ok((findings, version, ok)) => (EpubcheckStatus::Ran { tool, version, ok }, findings),
        Err(message) => (EpubcheckStatus::Failed { message }, Vec::new()),
    }
}

/// Checks the EPUB of the work (see module doc). `epubcheck = false` never starts EPUBCheck (`NotRequested`);
/// `path_env` is the PATH content for finding xmllint and EPUBCheck (RPC and pages: that of the process,
/// tests: own folders). Without sections → `REFUSED`.
pub async fn run_check(
    state: &AppState,
    work: WorkId,
    readers: &[&'static ReaderProfile],
    footnotes: FootnoteMode,
    epubcheck_wanted: bool,
    path_env: Option<&OsStr>,
) -> Result<Checked, RpcError> {
    let book = crate::epub::book_for(state, work, footnotes).await?;
    let bytes = baruch_epub::package(&book).map_err(|e| RpcError::new(INTERNAL, e.to_string()))?;
    let xmllint = find_xmllint(path_env);
    let structure_bytes = bytes.clone();
    let (mut findings, xml_tool) =
        tokio::task::spawn_blocking(crate::i18n::carry(move || check_structure(&structure_bytes, xmllint.as_deref())))
            .await
            .map_err(|e| RpcError::new(INTERNAL, format!("Prüfung abgebrochen: {e}")))?;
    findings.extend(book.warnings.iter().map(build_warning));
    for reader in readers {
        findings.extend(reader_warnings(&book, reader));
    }
    let status = if epubcheck_wanted {
        let jar = state.store.epubcheck_jar().await?;
        match find_epubcheck(path_env, jar.as_deref().map(Path::new)) {
            None => EpubcheckStatus::NotFound,
            Some(tool) => {
                let (status, more) = epubcheck(tool, bytes, EPUBCHECK_TIMEOUT).await;
                findings.extend(more);
                status
            }
        }
    } else {
        EpubcheckStatus::NotRequested
    };
    let summary = Summary::of(&findings);
    Ok(Checked { report: CheckReport { findings, xml_tool, epubcheck: status }, readers: readers.to_vec(), summary })
}

fn slugs_param(params: &Value) -> Result<Vec<String>, RpcError> {
    match params.get("readers") {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(Value::Array(list)) => list
            .iter()
            .map(|v| v.as_str().map(str::to_owned).ok_or_else(|| invalid("readers: Liste von Slugs erwartet")))
            .collect(),
        Some(_) => Err(invalid("readers: Liste von Slugs erwartet")),
    }
}

fn footnotes_param(params: &Value) -> Result<FootnoteMode, RpcError> {
    match params.get("footnotes") {
        None | Some(Value::Null) => Ok(FootnoteMode::Popup),
        Some(value) => {
            serde_json::from_value(value.clone()).map_err(|_| invalid("footnotes: „popup“ oder „endnotes“ erwartet"))
        }
    }
}

pub async fn call(state: &AppState, work: WorkId, method: &str, params: Value) -> Result<Value, RpcError> {
    if !handles(method) {
        return Err(RpcError::new(METHOD_NOT_FOUND, format!("unknown method {method}")));
    }
    let params = if params.is_null() { json!({}) } else { params };
    let readers = readers_of(&slugs_param(&params)?)?;
    let footnotes = footnotes_param(&params)?;
    let wanted = match params.get("epubcheck") {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(_) => return Err(invalid("epubcheck: true oder false erwartet")),
    };
    let path = process_path();
    Ok(run_check(state, work, &readers, footnotes, wanted, path.as_deref()).await?.to_json())
}

// ---- Setting -----------------------------------------------------------------------------------------

/// Checks a path to the epubcheck.jar: absolute, ends in `.jar`, existing regular file, no control characters,
/// at most [`MAX_JAR_PATH`] bytes. Empty (after trimming) → `Ok(None)` = remove.
pub fn validate_jar(raw: &str) -> Result<Option<String>, String> {
    let path = raw.trim();
    if path.is_empty() {
        return Ok(None);
    }
    if path.len() > MAX_JAR_PATH {
        return Err(format!("EPUBCheck: Der Pfad ist zu lang (höchstens {MAX_JAR_PATH} Bytes)."));
    }
    if path.chars().any(char::is_control) {
        return Err("EPUBCheck: Der Pfad enthält Steuerzeichen.".into());
    }
    let p = Path::new(path);
    if !p.is_absolute() {
        return Err(format!("EPUBCheck: „{path}“ ist kein absoluter Pfad (z. B. /opt/epubcheck/epubcheck.jar)."));
    }
    if !p.extension().is_some_and(|e| e.eq_ignore_ascii_case("jar")) {
        return Err(format!("EPUBCheck: „{path}“ endet nicht auf .jar."));
    }
    match std::fs::metadata(p) {
        Ok(m) if m.is_file() => Ok(Some(path.to_owned())),
        Ok(_) => Err(format!("EPUBCheck: „{path}“ ist keine Datei.")),
        Err(_) => Err(format!("EPUBCheck: „{path}“ gibt es nicht.")),
    }
}

/// State of the setting: configured jar, tool found (PATH before jar), `java` in PATH.
pub async fn checker_state(state: &AppState, path_env: Option<&OsStr>) -> Result<Value, RpcError> {
    let jar = state.store.epubcheck_jar().await?;
    let found = find_epubcheck(path_env, jar.as_deref().map(Path::new));
    let java = find_program("java", path_env).is_some();
    Ok(json!({ "jar": jar, "found": found, "java": java }))
}

pub async fn call_global(state: &AppState, method: &str, params: Value) -> Result<Value, RpcError> {
    if !handles_global(method) {
        return Err(RpcError::new(METHOD_NOT_FOUND, format!("unknown method {method}")));
    }
    let path = process_path();
    if method == "epub.checker_set" {
        let jar = match params.get("jar") {
            None => return Err(invalid("jar fehlt: Pfad zum epubcheck.jar oder null zum Entfernen")),
            Some(Value::Null) => None,
            Some(Value::String(s)) => validate_jar(s).map_err(invalid)?,
            Some(_) => return Err(invalid("jar: Pfad (Text) oder null erwartet")),
        };
        state.store.set_epubcheck_jar(jar.as_deref()).await?;
    }
    checker_state(state, path.as_deref()).await
}

// ---- UI ----------------------------------------------------------------------------------------------

fn templates() -> &'static Environment<'static> {
    static ENV: OnceLock<Environment<'static>> = OnceLock::new();
    ENV.get_or_init(|| {
        let mut env = Environment::new();
        crate::i18n::install(&mut env);
        for (name, source) in [
            ("base.html", include_str!("../templates/base.html")),
            ("epub_check.html", include_str!("../templates/epub_check.html")),
            ("fragments/epub_check_panel.html", include_str!("../templates/fragments/epub_check_panel.html")),
        ] {
            env.add_template(name, source).expect("Vorlage der EPUB-Prüfung fehlerhaft");
        }
        env
    })
}

fn render(name: &str, ctx: Jinja) -> Result<String, String> {
    templates().get_template(name).and_then(|t| t.render(ctx)).map_err(|e| format!("Vorlage {name}: {e}"))
}

fn severity_label(s: Severity) -> &'static str {
    match s {
        Severity::Error => "Fehler",
        Severity::Warning => "Warnung",
        Severity::Info => "Hinweis",
    }
}

fn severity_class(s: Severity) -> &'static str {
    match s {
        Severity::Error => "error",
        Severity::Warning => "warning",
        Severity::Info => "info",
    }
}

fn xml_tool_label(t: &XmlTool) -> &'static str {
    match t {
        XmlTool::Xmllint => "xmllint (--noout --nonet)",
        XmlTool::QuickXml => "quick-xml (eingebaut; xmllint nicht gefunden)",
    }
}

fn tool_label(tool: &EpubcheckTool) -> String {
    match tool {
        EpubcheckTool::Binary { path } => format!("Programm {}", path.display()),
        EpubcheckTool::Jar { path } => format!("java -jar {}", path.display()),
    }
}

/// State of EPUBCheck as text for the page: (short form, explanation, class).
fn epubcheck_view(status: &EpubcheckStatus, available: Option<&EpubcheckTool>) -> Jinja {
    match status {
        EpubcheckStatus::NotRequested => match available {
            Some(tool) => context! {
                kind => "ready", title => "EPUBCheck gefunden",
                text => format!("{} – startet nur auf Knopfdruck.", tool_label(tool)),
            },
            None => context! {
                kind => "missing", title => "EPUBCheck nicht gefunden",
                text => "Weder „epubcheck“ im PATH noch ein eingestelltes epubcheck.jar (Einstellungen → Daten). \
                         Oben steht die eigene Prüfung von Baruch; EPUBCheck wird nie heruntergeladen.",
            },
        },
        EpubcheckStatus::NotFound => context! {
            kind => "missing", title => "EPUBCheck nicht gefunden",
            text => "Weder „epubcheck“ im PATH noch ein eingestelltes epubcheck.jar (Einstellungen → Daten). Oben \
                     steht die eigene Prüfung von Baruch; EPUBCheck wird nie heruntergeladen.",
        },
        EpubcheckStatus::Ran { tool, version, ok } => context! {
            kind => if *ok { "ok" } else { "failed" },
            title => if *ok { "EPUBCheck: bestanden" } else { "EPUBCheck: nicht bestanden" },
            text => format!(
                "{}{} – die Meldungen stehen mit „EPUBCheck:“ in der Liste.",
                tool_label(tool),
                version.as_deref().map(|v| format!(", Version {v}")).unwrap_or_default()
            ),
        },
        EpubcheckStatus::Failed { message } => context! {
            kind => "failed", title => "EPUBCheck lief nicht durch", text => message.as_str(),
        },
    }
}

/// Context of `fragments/epub_check_panel.html`.
fn panel_context(work: WorkId, checked: &Checked) -> Jinja {
    let names: BTreeMap<&str, &str> = PROFILES.iter().map(|p| (p.slug, p.name)).collect();
    let rows: Vec<Jinja> = checked
        .report
        .findings
        .iter()
        .map(|f| {
            context! {
                severity => severity_label(f.severity), class => severity_class(f.severity), code => f.code.as_str(),
                message => f.message.as_str(), path => f.path.as_deref(),
                reader => f.reader.as_deref().map(|r| names.get(r).copied().unwrap_or(r)),
            }
        })
        .collect();
    context! {
        work => work, summary => Jinja::from(Serde(checked.summary)), findings => rows,
        xml_tool => xml_tool_label(&checked.report.xml_tool),
        readers => checked.readers.iter().map(|r| r.name).collect::<Vec<_>>(),
    }
}

/// Check report as a finished, escaped HTML fragment for the EPUB preview.
pub async fn preview_panel(state: &AppState, work: WorkId, readers: &[&'static ReaderProfile]) -> String {
    let path = process_path();
    let ctx = match run_check(state, work, readers, FootnoteMode::Popup, false, path.as_deref()).await {
        Ok(checked) => panel_context(work, &checked),
        Err(e) => context! { work => work, error => e.message },
    };
    render("fragments/epub_check_panel.html", context! { panel => true, ..ctx }).unwrap_or_else(|e| {
        let text = minijinja::HtmlEscape(&e).to_string();
        format!("<p class=\"notice error\" role=\"alert\">Prüfbericht: {text}</p>")
    })
}

/// Page of the check report.
pub fn pages() -> Router<Arc<AppState>> {
    Router::new().route("/werke/{id}/epub/pruefung", get(check_page))
}

/// Forms (behind `guard::same_origin`).
pub fn writes() -> Router<Arc<AppState>> {
    Router::new().route("/werke/epub/pruefen", post(check_act)).route("/einstellungen/daten/epubcheck", post(jar_act))
}

/// Fields `leser` (repeated) and `fussnoten` from query or form.
fn form_choice(pairs: &[(String, String)]) -> Result<(Vec<&'static ReaderProfile>, FootnoteMode), String> {
    let slugs: Vec<String> =
        pairs.iter().filter(|(k, v)| k == "leser" && !v.trim().is_empty()).map(|(_, v)| v.clone()).collect();
    let readers = readers_of(&slugs).map_err(|e| e.message)?;
    let footnotes = match pairs.iter().find(|(k, _)| k == "fussnoten").map(|(_, v)| v.trim()) {
        None | Some("") | Some("popup") => FootnoteMode::Popup,
        Some("endnoten") => FootnoteMode::Endnotes,
        Some(other) => return Err(format!("fussnoten: „popup“ oder „endnoten“, nicht „{other}“.")),
    };
    Ok((readers, footnotes))
}

async fn page(
    state: &AppState,
    headers: &HeaderMap,
    work: WorkId,
    pairs: &[(String, String)],
    epubcheck_wanted: bool,
) -> Response {
    let title = match state.store.work_meta(work).await {
        Ok(meta) => meta.title,
        Err(StoreError::UnknownWork(_)) => return (StatusCode::NOT_FOUND, "unbekanntes Werk").into_response(),
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, format!("Fehler: {e}")).into_response(),
    };
    let settings = Settings::from_cookie_header(headers.get(header::COOKIE).and_then(|v| v.to_str().ok()));
    let (readers, footnotes, choice_error) = match form_choice(pairs) {
        Ok((readers, footnotes)) => (readers, footnotes, None),
        Err(message) => (default_readers(), FootnoteMode::Popup, Some(message)),
    };
    let path = process_path();
    let jar = state.store.epubcheck_jar().await.ok().flatten();
    let available = find_epubcheck(path.as_deref(), jar.as_deref().map(Path::new));
    let (status, panel, epubcheck, error) = match &choice_error {
        Some(message) => (StatusCode::BAD_REQUEST, None, None, Some(message.clone())),
        None => match run_check(state, work, &readers, footnotes, epubcheck_wanted, path.as_deref()).await {
            Ok(checked) => {
                let panel = render("fragments/epub_check_panel.html", panel_context(work, &checked));
                let view = epubcheck_view(&checked.report.epubcheck, available.as_ref());
                match panel {
                    Ok(html) => (StatusCode::OK, Some(html), Some(view), None),
                    Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, format!("Fehler: {e}")).into_response(),
                }
            }
            Err(e) if e.code == REFUSED => (StatusCode::UNPROCESSABLE_ENTITY, None, None, Some(e.message)),
            Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, format!("Fehler: {}", e.message)).into_response(),
        },
    };
    let choices: Vec<Jinja> = PROFILES
        .iter()
        .map(|p| {
            context! {
                slug => p.slug, name => p.name, required => p.required,
                checked => readers.iter().any(|r| r.slug == p.slug),
            }
        })
        .collect();
    let ctx = context! {
        settings => Jinja::from(Serde(settings)), work => context! { id => work, title => title },
        panel => panel, epubcheck => epubcheck, error => error, readers => choices,
        endnotes => footnotes == FootnoteMode::Endnotes, ran => epubcheck_wanted,
    };
    match render("epub_check.html", ctx) {
        Ok(html) => (status, Html(html)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, format!("Fehler: {e}")).into_response(),
    }
}

/// `GET /werke/{id}/epub/pruefung[?leser=…&fussnoten=…]`: report without EPUBCheck.
async fn check_page(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    UrlPath(work): UrlPath<WorkId>,
    Query(pairs): Query<Vec<(String, String)>>,
) -> Response {
    // The check builds and packs the whole EPUB and starts xmllint per file: not for foreign pages.
    if crate::guard::cross_site_get(&headers) {
        return (StatusCode::FORBIDDEN, "Abgelehnt: Die Prüfung entsteht nicht auf Anfrage einer fremden Seite.")
            .into_response();
    }
    page(&state, &headers, work, &pairs, false).await
}

/// `POST /werke/epub/pruefen` (fields `work`, `leser`*, `fussnoten`): report with EPUBCheck.
async fn check_act(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Form(pairs): Form<Vec<(String, String)>>,
) -> Response {
    let Some(work) = pairs.iter().find(|(k, _)| k == "work").and_then(|(_, v)| v.trim().parse::<WorkId>().ok()) else {
        return (StatusCode::UNPROCESSABLE_ENTITY, "Fehler: Werk fehlt.").into_response();
    };
    page(&state, &headers, work, &pairs, true).await
}

/// `POST /einstellungen/daten/epubcheck` (field `jar`, empty = remove; `work` optional): 303 to
/// `/einstellungen/daten?ok=epubcheck`, invalid path → 422 with the page and the message, nothing stored.
async fn jar_act(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Form(pairs): Form<Vec<(String, String)>>,
) -> Response {
    let field = |name: &str| pairs.iter().find(|(k, _)| k == name).map(|(_, v)| v.as_str());
    let work = field("work").and_then(|w| w.trim().parse::<WorkId>().ok()).filter(|&w| w > 0);
    let jar = match validate_jar(field("jar").unwrap_or("")) {
        Ok(jar) => jar,
        Err(message) => return crate::settings::error_page(&state, &headers, "daten", work, &message).await,
    };
    match state.store.set_epubcheck_jar(jar.as_deref()).await {
        Ok(()) => {
            Redirect::to(&format!("/einstellungen/daten?ok=epubcheck{}#epubcheck", crate::settings::work_suffix(work)))
                .into_response()
        }
        Err(e) => crate::settings::error_page(&state, &headers, "daten", work, &e.to_string()).await,
    }
}

/// Context of the block „EPUBCheck“ on Einstellungen → Daten: configured path, tool found, `java`.
pub(crate) async fn settings_context(state: &AppState, saved: bool) -> Jinja {
    let path = process_path();
    let jar = state.store.epubcheck_jar().await.ok().flatten();
    let found = find_epubcheck(path.as_deref(), jar.as_deref().map(Path::new));
    let java = find_program("java", path.as_deref()).is_some();
    context! {
        jar => jar.clone().unwrap_or_default(), has_jar => jar.is_some(),
        jar_missing => jar.as_deref().is_some_and(|j| !baruch_epub::check::is_jar(Path::new(j))),
        found => found.as_ref().map(tool_label), java => java, saved => saved, max => MAX_JAR_PATH,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jar_paths_must_be_absolute_existing_jars_without_control_characters() {
        let dir = std::env::temp_dir();
        assert_eq!(validate_jar("  "), Ok(None), "leer = entfernen");
        assert!(validate_jar("epubcheck.jar").unwrap_err().contains("absolut"), "relativ abgelehnt");
        assert!(validate_jar("/opt/x/epubcheck.zip").unwrap_err().contains(".jar"), "andere Endung abgelehnt");
        assert!(validate_jar("/opt/gibt-es-nicht/epubcheck.jar").unwrap_err().contains("gibt es nicht"), "fehlt");
        assert!(validate_jar("/opt/a\u{7}b.jar").unwrap_err().contains("Steuerzeichen"), "Steuerzeichen");
        let long = format!("/{}.jar", "a".repeat(MAX_JAR_PATH));
        assert!(validate_jar(&long).unwrap_err().contains("zu lang"), "zu lang");
        let folder = dir.join(format!("baruch-unit-{}.jar", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        assert!(validate_jar(folder.to_str().unwrap()).unwrap_err().contains("keine Datei"), "Ordner ist keine Datei");
        let _ = std::fs::remove_dir(&folder);
    }

    #[test]
    fn the_temp_folder_has_a_random_name_and_is_private() {
        let first = TempDir::random_name().expect("Zufallsname");
        let second = TempDir::random_name().expect("Zufallsname");
        let random = first.strip_prefix(TEMP_PREFIX).expect("Name beginnt mit der Vorsilbe");
        assert!(
            random.len() >= 16 && random.chars().all(|c| c.is_ascii_hexdigit()),
            "mindestens 16 zufällige Hex-Zeichen hinter der Vorsilbe (nicht erratbar): {first}"
        );
        assert_ne!(first, second, "zwei Namen unterscheiden sich (128 Bit Zufall) – kein Zähler, keine Prozessnummer");
        let dir = TempDir::new().expect("Temp-Ordner anlegen");
        let path = dir.0.clone();
        assert!(path.is_dir(), "der Ordner ist angelegt: {}", path.display());
        assert_eq!(path.parent(), Some(std::env::temp_dir().as_path()), "direkt im Temp-Ordner des Systems");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o700, "nur der eigene Benutzer darf hinein (das EPUB ist unveröffentlicht)");
        }
        std::fs::write(path.join("buch.epub"), b"x").expect("im eigenen Ordner schreiben");
        drop(dir);
        assert!(!path.exists(), "nach dem Lauf ist der Ordner samt Inhalt entfernt");
    }

    #[test]
    fn a_folder_or_symlink_that_already_exists_is_never_used() {
        // Own practice folder (private), in it the test plays the “attacker”.
        let playground = TempDir::new().expect("Übungsordner");
        let planted = playground.0.join(TempDir::random_name().unwrap());
        std::fs::create_dir(&planted).unwrap();
        std::fs::write(planted.join("fremd.txt"), b"vorab angelegt").unwrap();
        let error = TempDir::create(&planted).err().expect("ein vorhandener Ordner wird nicht benutzt");
        assert_eq!(
            error.kind(),
            std::io::ErrorKind::AlreadyExists,
            "ohne recursive meldet das Anlegen den vorhandenen Ordner als Fehler: {error}"
        );
        assert!(
            planted.join("fremd.txt").exists(),
            "der fremde Ordner bleibt unangetastet – kein TempDir, also auch kein Aufräumen darin"
        );
        #[cfg(unix)]
        {
            let elsewhere = playground.0.join("ziel");
            std::fs::create_dir(&elsewhere).unwrap();
            let link = playground.0.join(TempDir::random_name().unwrap());
            std::os::unix::fs::symlink(&elsewhere, &link).unwrap();
            let error = TempDir::create(&link).err().expect("einem Symlink wird nicht gefolgt");
            assert_eq!(
                error.kind(),
                std::io::ErrorKind::AlreadyExists,
                "Symlink gleichen Namens ist ein Fehler: {error}"
            );
            assert_eq!(
                std::fs::read_dir(&elsewhere).unwrap().count(),
                0,
                "im Ziel des Symlinks ist nichts entstanden (das EPUB wäre sonst dort gelandet)"
            );
            // A symlink into nothing (target missing) is not “filled in” either.
            let dangling = playground.0.join(TempDir::random_name().unwrap());
            std::os::unix::fs::symlink(playground.0.join("gibt-es-nicht"), &dangling).unwrap();
            assert!(TempDir::create(&dangling).is_err(), "Symlink ohne Ziel wird nicht benutzt");
            assert!(!playground.0.join("gibt-es-nicht").exists(), "und sein Ziel wird nicht angelegt");
        }
        // TempDir does not create multi-level paths (no `recursive`): the parent folder must already exist.
        let deep = playground.0.join("a").join("b");
        assert!(TempDir::create(&deep).is_err(), "fehlender Elternordner wird nicht mit angelegt");
    }

    #[test]
    fn the_panel_escapes_messages_paths_and_codes() {
        let finding = Finding {
            code: "X<1>".into(),
            severity: Severity::Error,
            message: "<script>alert(1)</script> & mehr".into(),
            path: Some("OEBPS/\"a\".xhtml".into()),
            reader: None,
        };
        let checked = Checked {
            summary: Summary::of(std::slice::from_ref(&finding)),
            report: CheckReport {
                findings: vec![finding],
                xml_tool: XmlTool::QuickXml,
                epubcheck: EpubcheckStatus::NotRequested,
            },
            readers: default_readers(),
        };
        let html = crate::i18n::sync_scope(crate::i18n::Lang::De, || {
            render("fragments/epub_check_panel.html", context! { panel => true, ..panel_context(7, &checked) })
        })
        .unwrap();
        assert!(!html.contains("<script>"), "Meldungen werden maskiert: {html}");
        assert!(html.contains("&lt;script&gt;alert(1)&lt;&#x2f;script&gt; &amp; mehr"), "maskierter Text: {html}");
        assert!(html.contains("X&lt;1&gt;") && html.contains("&quot;a&quot;"), "Code und Pfad maskiert: {html}");
        assert!(html.contains("<strong>1 Fehler</strong>"), "Zusammenfassung: {html}");
        assert!(html.contains("/werke/7/epub/pruefung"), "Link zur Prüfseite: {html}");
    }

    #[test]
    fn mapped_template_breaks_are_hints_and_everything_else_from_the_build_is_a_warning() {
        let w = |code: &str| baruch_epub::EpubWarning { code: code.into(), message: "m".into(), key: Some(3) };
        assert_eq!(build_warning(&w("TEMPLATE_PAGEBREAK_MAPPED")).severity, Severity::Info, "abgebildet = Hinweis");
        assert_eq!(build_warning(&w("ISLAND_OMITTED")).severity, Severity::Warning, "ausgelassen = Warnung");
        assert_eq!(build_warning(&w("ISLAND_OMITTED")).path.as_deref(), Some("Abschnitt 3"), "Abschnitt genannt");
    }
}
