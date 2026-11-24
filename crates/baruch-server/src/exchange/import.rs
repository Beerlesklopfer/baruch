// SPDX-License-Identifier: AGPL-3.0-or-later
//! Austausch: Einlesen (Runde 26; Vertrag F6/F7, Doku `docs/Austauschformat.md` §4/§5).
//!
//! - `GET /austausch/einlesen[?work=W]`: Formular (multipart, Feld `datei`, dazu `title`), ohne JavaScript.
//! - `POST /austausch/vorschau`: Datei lesen und prüfen ([`baruch_exchange::read::read`]), Vorschau rechnen
//!   ([`baruch_store::Store::exchange_preview`], zurückgerollt); erst dann liegt die Datei unter
//!   `<data>/austausch-eingang/<sha256>` (Einträge älter als 24 h werden dabei weggeräumt). Danach die Vorschau
//!   mit dem Formular „Übernehmen“.
//! - `POST /austausch/uebernehmen {sha256, mode, title?}`: Datei aus dem Eingang, Prüfsumme wie gezeigt, neu planen
//!   und schreiben ([`baruch_store::Store::exchange_apply`], eine Transaktion unter `state.write_lock`), ein Commit
//!   im Spiegel ([`crate::mirror::record_exchange`]); Ergebnisseite mit Links zu den Abweichungen
//!   (`/?work=W&key=K&rev=R`) und zu den Anträgen (`/gliederung?work=W#antraege`). Die Eingangsdatei wird danach
//!   gelöscht, auch bei einer Ablehnung.
//! - RPC `work.exchange_preview {data: Base64, file_name?, title?}` und `work.exchange_import {data, sha256, mode,
//!   title?}` (Größe begrenzt durch `works::RPC_BODY_LIMIT`; größere Dateien über Formular oder CLI).
//! - CLI `import DATEI.baruch|DATEI.baruch.xml [--abgleichen] [--vorschau] [--title T]`.
//!
//! Einlesen ist eine Aktion des Menschen: nie umschalten, überschreiben oder löschen; Gliederungsänderungen nur
//! als Antrag. Nicht über MCP. Bei jedem Fehler wird nichts geschrieben.

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

/// Größte Anfrage an die Formulare: größte Austauschdatei und 1 MiB für die übrigen Felder.
pub const UPLOAD_LIMIT: usize = Limits::DEFAULT.max_file_bytes as usize + 1024 * 1024;

/// Ordner der hochgeladenen Dateien zwischen Vorschau und Übernehmen: `<data>/austausch-eingang/<sha256>`.
pub const INBOX_DIR: &str = "austausch-eingang";

/// Einträge im Eingang, die älter sind, räumt die nächste Vorschau weg.
pub const INBOX_MAX_AGE: Duration = Duration::from_secs(24 * 60 * 60);

/// Hinweis der CLI und der Formulare, wenn der Titel schon vergeben ist.
pub const TITLE_HINT: &str = "Mit einem eigenen Titel einlesen (Formular: Feld „Titel“, CLI: --title \"…\").";

/// Seite des Formulars (gemischt über `exchange::pages`).
pub fn pages() -> Router<Arc<AppState>> {
    Router::new().route("/austausch/einlesen", get(import_page))
}

/// Formulare (gemischt über `exchange::writes`, hinter `guard::same_origin`).
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

/// Abfrage der Seite: `work` (Werk, aus dem das Menü kam; nur für die Kopfzeile).
#[derive(Debug, Default, serde::Deserialize)]
pub struct ImportQuery {
    pub work: Option<String>,
}

/// Formularseite (auch nach einem Fehler, dann mit Meldung).
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

// ---- Gemeinsamer Weg -------------------------------------------------------------------------------

/// Datei lesen und prüfen (blockiert nicht den Tokio-Worker).
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

/// Vorschau einer Datei (nichts bleibt geschrieben). Unter der Schreibsperre, damit die zurückgerollte Planung
/// keinen anderen Schreibvorgang kreuzt.
pub async fn preview_bytes(state: &AppState, bytes: Vec<u8>, title: Option<&str>) -> Result<ExchangeReport, RpcError> {
    let sha = baruch_exchange::sha256_hex(&bytes);
    let doc = read_document(Arc::new(bytes)).await?;
    let _held = state.write_lock.lock().await;
    state.store.exchange_preview(&doc, &sha, title).await.map_err(store_error)
}

/// Ergebnis des Übernehmens samt Spiegel.
#[derive(Debug, Clone)]
pub struct ImportOutcome {
    pub applied: ExchangeApplied,
    pub commit: Option<String>,
    pub mirror: Option<String>,
}

/// Übernehmen: Prüfsumme muss zur gezeigten Datei passen (sonst `INVALID_PARAMS`), dann neu planen und in einer
/// Transaktion schreiben (Modus passt nicht → `REFUSED`), danach ein Commit im Spiegel (außer beim Abgleich ohne
/// jede Änderung).
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

/// RPC `work.exchange_preview {data: Base64, file_name?, title?}` → Vorschau (Form des Vertrags R26 §5).
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

/// Antwort von `work.exchange_import`.
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

// ---- Eingang -------------------------------------------------------------------------------------

/// SHA-256 als 64 Kleinbuchstaben-Hex? Nur solche Namen kommen in den Eingang (kein Pfad von außen).
fn is_sha256(text: &str) -> bool {
    text.len() == 64 && text.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// `<data>/austausch-eingang`; keine symbolische Verknüpfung.
fn inbox(data: &Path) -> Result<PathBuf, String> {
    let dir = data.join(INBOX_DIR);
    if std::fs::symlink_metadata(&dir).is_ok_and(|m| m.file_type().is_symlink()) {
        return Err(format!("{} ist eine symbolische Verknüpfung; dort wird nichts abgelegt", dir.display()));
    }
    Ok(dir)
}

/// Einträge älter als [`INBOX_MAX_AGE`] wegräumen (nur Dateien mit SHA-256-Namen).
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

/// Datei in den Eingang legen (zuerst alte wegräumen); schreibt über eine Hilfsdatei und benennt um.
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

// ---- Formulare -----------------------------------------------------------------------------------

/// Felder der Vorschau aus dem multipart-Formular: Datei (`datei`), Titel, Werk der Kopfzeile.
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

/// Kurzbeschreibung einer Vorschau für Vorlagen: Zahlen als Liste (Art, Zahl) ohne Nullen, die Art in Einzahl
/// bei genau 1 („1 Fassung“, „2 Fassungen“).
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
        // Runde 22: Fortschreibungen (auch beim Vorspulen einer bekannten Fassung).
        ("Fortschreibung", "Fortschreibungen", add.amends),
    ]
    .into_iter()
    .filter(|(_, _, n)| *n > 0)
    .map(|(one, many, n)| ((if n == 1 { one } else { many }).to_owned(), n))
    .collect()
}

/// Abweichungen für Vorlagen, je mit Ziel-Adresse (`/?work=W&key=K[&rev=R]`) und Wort der Aktion.
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

/// Felder von `POST /austausch/uebernehmen`.
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
    // Danach (auch bei einer Ablehnung) bleibt keine Eingangsdatei liegen.
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

/// Angaben der Kommandozeile für `import DATEI.baruch|DATEI.baruch.xml`.
#[derive(Debug, Clone, Default)]
pub struct CliImport {
    /// `--abgleichen`: das Werk ist hier bekannt und wird abgeglichen.
    pub merge: bool,
    /// `--vorschau`: nichts schreiben, nur die Vorschau ausgeben.
    pub preview_only: bool,
    /// `--title`: eigener Titel beim Einlesen als neues Werk.
    pub title: Option<String>,
}

/// Datei für die CLI lesen: nur eine reguläre Datei (keine FIFO, kein Ordner), höchstens die Grenze der Datei.
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

/// Vorschau bzw. Ergebnis als Text für die Kommandozeile.
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

/// CLI `import DATEI.baruch|DATEI.baruch.xml [--abgleichen] [--vorschau] [--title T]`: ohne Flag und unbekannt →
/// neu; ohne Flag und bekannt → Fehler mit Hinweis auf `--abgleichen`; `--abgleichen` bei unbekanntem Werk →
/// Fehler; `--vorschau` schreibt nichts.
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
        // Der Name der Eingangsdatei kommt aus dem Formular: nur 64 Kleinbuchstaben-Hex, sonst ginge ein Pfad hinaus.
        assert!(is_sha256(&"a".repeat(64)), "64 Hex");
        assert!(!is_sha256("../../etc/passwd"), "Pfad");
        assert!(!is_sha256(&"A".repeat(64)), "groß nicht");
        assert!(!is_sha256(&"a".repeat(63)), "zu kurz");
    }
}
