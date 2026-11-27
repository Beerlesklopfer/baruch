// SPDX-License-Identifier: AGPL-3.0-or-later
//! Images and graphics of a work (contract Round 6, section 4f): RPC `asset.*`, the pages
//! „Bilder“ without JavaScript and the forms for uploading and maintaining. Storage is in the store
//! ([`baruch_store::Store::add_asset`] etc., content as Base64 or SVG text); every change is
//! exactly one mirror commit ([`mirror::record_asset`]). Everything is reserved for the human: through MCP
//! there are only the reading tools `list_assets` and `read_asset`, through `/act` nothing.
//!
//! RPC ([`handles`], [`call`]; all take `work?`):
//! - `asset.list` → `{assets: [AssetSummary]}`
//! - `asset.get {name, rev?, content?: bool = true}` → `{asset, revision, content?, encoding}`
//! - `asset.revisions {name}` → `{revisions: [AssetRevisionInfo]}`
//! - `asset.add {file_name, content, encoding: "base64"|"svg", name?, path?, caption?, alt?, note?}` → `{name, rev}`
//! - `asset.replace {name, file_name, content, encoding, note?, rename?: bool = false}` → `{name, rev, renamed_from?}`
//!   (new revision; a different format only with `rename`, then the image is named with the new extension, Round 21c)
//! - `asset.set_current {name, rev}` → `{name, rev}`
//! - `asset.update {name, rev, caption?, alt?, note?, paths?}` → `{name, rev, changed}`
//! - `asset.delete {name, rev?, confirm: true}` → `{name, rev?, deleted}` (without `rev` the whole image)
//! - `asset.path_conflicts` → `{conflicts: [{normal, paths: [{path, asset}]}]}` (legacy data, Round 21c)
//! - `asset.path_resolve {normal, asset}` → `{name, rev, commit?}`: all spellings of `normal` then belong
//!   to the image `asset` (one mirror commit)
//!
//! Writing methods run under [`AppState::write_lock`] and carry `commit` or `mirror`.
//!
//! Pages ([`pages`]): `GET /werke/{id}/bilder`, `/werke/{id}/bilder/{name}[?fassung=n]`,
//! `/werke/{id}/bilder/{name}/datei[?fassung=n]` (bytes with MIME, `nosniff`, CSP with `sandbox`) and
//! `/werke/{id}/bilder/pfad?p=<path>` (current image for the path, otherwise 404). Forms ([`writes`],
//! behind `guard::same_origin`): `POST /werke/bilder/hochladen` (multipart, at most
//! [`UPLOAD_LIMIT`], otherwise 413; field `rename` = „Endung anpassen“) and `POST /werke/bilder/act` (`action` =
//! `set_current`, `update`, `delete`, `remove`, since Round 21c `resolve_path` with `normal` and `name`).

use std::sync::{Arc, OnceLock};

use axum::Router;
use axum::extract::multipart::MultipartRejection;
use axum::extract::{DefaultBodyLimit, Form, Multipart, Path, Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use baruch_store::{
    AssetChange, AssetData, AssetEncoding, AssetRevisionInfo, AssetSummary, MAX_ASSET_BYTES, NewAsset, StoreError,
    WorkId,
};
use minijinja::value::Serde;
use minijinja::{Environment, Value, context};
use serde::Deserialize;
use serde_json::json;

use crate::AppState;
use crate::mirror::{self, HUMAN_NAME};
use crate::rpc::{INVALID_PARAMS, METHOD_NOT_FOUND, REFUSED, RpcError};
use crate::view::Settings;

/// Largest request to `POST /werke/bilder/hochladen`: 8 MiB image and 64 KiB for fields and framing.
pub const UPLOAD_LIMIT: usize = MAX_ASSET_BYTES + 64 * 1024;

/// Security header for served images: no script, no embedding of foreign content, sandbox
/// (so even an SVG that is opened directly executes nothing).
const IMAGE_CSP: &str = "default-src 'none'; style-src 'unsafe-inline'; sandbox";

// ---- RPC ---------------------------------------------------------------------------------------

/// Methods of this module.
pub fn handles(method: &str) -> bool {
    matches!(
        method,
        "asset.list"
            | "asset.search"
            | "asset.get"
            | "asset.revisions"
            | "asset.add"
            | "asset.replace"
            | "asset.set_current"
            | "asset.update"
            | "asset.delete"
            | "asset.path_conflicts"
            | "asset.path_resolve"
    )
}

#[derive(Deserialize)]
struct ResolveParams {
    normal: String,
    asset: String,
}

#[derive(Deserialize)]
struct NameParams {
    name: String,
}

#[derive(Deserialize)]
struct SearchParams {
    query: String,
}

#[derive(Deserialize)]
struct GetParams {
    name: String,
    rev: Option<u32>,
    content: Option<bool>,
}

#[derive(Deserialize)]
struct AddParams {
    file_name: String,
    content: String,
    encoding: String,
    name: Option<String>,
    path: Option<String>,
    caption: Option<String>,
    alt: Option<String>,
    note: Option<String>,
}

#[derive(Deserialize)]
struct ReplaceParams {
    name: String,
    file_name: String,
    content: String,
    encoding: String,
    note: Option<String>,
    #[serde(default)]
    rename: bool,
}

#[derive(Deserialize)]
struct RevParams {
    name: String,
    rev: u32,
}

#[derive(Deserialize)]
struct UpdateParams {
    name: String,
    rev: u32,
    caption: Option<String>,
    alt: Option<String>,
    note: Option<String>,
    paths: Option<Vec<String>>,
}

#[derive(Deserialize)]
struct DeleteParams {
    name: String,
    rev: Option<u32>,
}

fn params<T: for<'de> Deserialize<'de>>(value: serde_json::Value) -> Result<T, RpcError> {
    let value = if value.is_null() { json!({}) } else { value };
    serde_json::from_value(value).map_err(|e| RpcError::new(INVALID_PARAMS, e.to_string()))
}

/// Result of the mirror into the response: `commit` for a new commit, `mirror` on error.
fn add_mirror(answer: &mut serde_json::Value, result: Result<Option<String>, String>) {
    match result {
        Ok(Some(commit)) => answer["commit"] = json!(commit),
        Ok(None) => {}
        Err(e) => answer["mirror"] = json!(e),
    }
}

/// Content from RPC: `encoding` = `base64` (standard Base64) or `svg` (SVG text). Result: bytes and
/// whether to store as Base64.
fn content_bytes(content: &str, encoding: &str) -> Result<(Vec<u8>, bool), RpcError> {
    match AssetEncoding::parse(encoding) {
        Some(AssetEncoding::Base64) => Ok((baruch_store::decode_base64(content)?, true)),
        Some(AssetEncoding::Svg) => Ok((content.as_bytes().to_vec(), false)),
        None => {
            Err(RpcError::new(INVALID_PARAMS, format!("encoding: „base64“ oder „svg“ erwartet, nicht „{encoding}“")))
        }
    }
}

/// Mirror the change and answer `{name, rev}` with `commit`/`mirror`.
async fn recorded(state: &AppState, held: &mirror::WriteGuard<'_>, change: &AssetChange) -> serde_json::Value {
    let mut answer = json!({ "name": change.name, "rev": change.rev });
    add_mirror(&mut answer, mirror::record_asset(state, held, change).await);
    answer
}

pub async fn call(
    state: &AppState,
    work: WorkId,
    method: &str,
    params_value: serde_json::Value,
) -> Result<serde_json::Value, RpcError> {
    let store = &state.store;
    match method {
        "asset.list" => Ok(json!({ "assets": store.asset_list(work).await? })),
        "asset.search" => {
            let p: SearchParams = params(params_value)?;
            Ok(json!({ "assets": store.asset_search(work, &p.query).await? }))
        }
        "asset.get" => {
            let p: GetParams = params(params_value)?;
            let (summary, revision) = summary_and_revision(state, work, &p.name, p.rev).await?;
            let mut answer = json!({ "asset": summary, "revision": revision, "encoding": revision.encoding });
            if p.content.unwrap_or(true) {
                answer["content"] = json!(store.asset_data(work, &p.name, Some(revision.number)).await?.content);
            }
            Ok(answer)
        }
        "asset.revisions" => {
            let p: NameParams = params(params_value)?;
            Ok(json!({ "revisions": store.asset_revisions(work, &p.name).await? }))
        }
        "asset.add" => {
            let p: AddParams = params(params_value)?;
            let (bytes, as_base64) = content_bytes(&p.content, &p.encoding)?;
            let new = NewAsset {
                file_name: &p.file_name,
                name: p.name.as_deref(),
                bytes: &bytes,
                as_base64,
                path: p.path.as_deref(),
                caption: p.caption.as_deref().unwrap_or(""),
                alt: p.alt.as_deref().unwrap_or(""),
                note: p.note.as_deref().unwrap_or(""),
            };
            let held = state.write_lock.lock().await;
            let change = store.add_asset(work, &new, HUMAN_NAME).await?;
            Ok(recorded(state, &held, &change).await)
        }
        "asset.replace" => {
            let p: ReplaceParams = params(params_value)?;
            let (bytes, as_base64) = content_bytes(&p.content, &p.encoding)?;
            let held = state.write_lock.lock().await;
            let note = p.note.as_deref().unwrap_or("");
            let change = store
                .replace_asset_renaming(work, &p.name, &p.file_name, &bytes, as_base64, note, HUMAN_NAME, p.rename)
                .await?;
            let mut answer = recorded(state, &held, &change).await;
            if change.name != p.name {
                answer["renamed_from"] = json!(p.name);
            }
            Ok(answer)
        }
        "asset.path_conflicts" => Ok(json!({ "conflicts": store.asset_path_conflicts(work).await? })),
        "asset.path_resolve" => {
            let p: ResolveParams = params(params_value)?;
            let held = state.write_lock.lock().await;
            let change = store.resolve_asset_path(work, &p.normal, p.asset.trim(), HUMAN_NAME).await?;
            Ok(recorded(state, &held, &change).await)
        }
        "asset.set_current" => {
            let p: RevParams = params(params_value)?;
            let held = state.write_lock.lock().await;
            let change = store.set_current_asset(work, &p.name, p.rev).await?;
            Ok(recorded(state, &held, &change).await)
        }
        "asset.update" => {
            let p: UpdateParams = params(params_value)?;
            update(state, work, &p).await
        }
        "asset.delete" => {
            if params_value.get("confirm") != Some(&serde_json::Value::Bool(true)) {
                return Err(RpcError::new(INVALID_PARAMS, "Löschen bitte bestätigen (confirm)."));
            }
            let p: DeleteParams = params(params_value)?;
            delete(state, work, &p.name, p.rev).await
        }
        _ => Err(RpcError::new(METHOD_NOT_FOUND, format!("unbekannte Methode {method}"))),
    }
}

/// Summary of the image and the revision `rev` (missing: the current one).
async fn summary_and_revision(
    state: &AppState,
    work: WorkId,
    name: &str,
    rev: Option<u32>,
) -> Result<(AssetSummary, AssetRevisionInfo), RpcError> {
    let revisions = state.store.asset_revisions(work, name).await?;
    let summary = state
        .store
        .asset_list(work)
        .await?
        .into_iter()
        .find(|a| a.name == name)
        .ok_or_else(|| StoreError::UnknownAsset { work, name: name.to_owned() })?;
    let revision = match rev {
        Some(rev) => revisions.iter().find(|r| r.number == rev).cloned().ok_or_else(|| {
            let numbers: Vec<String> = revisions.iter().map(|r| r.number.to_string()).collect();
            StoreError::Asset(format!("Bild {name}: Fassung {rev} gibt es nicht; vorhanden: {}", numbers.join(", ")))
        })?,
        None => revisions
            .iter()
            .find(|r| r.current)
            .cloned()
            .ok_or_else(|| RpcError::new(crate::rpc::INTERNAL, format!("Bild {name}: keine aktuelle Fassung")))?,
    };
    Ok((summary, revision))
}

/// Paths from a text field or a list: one per line, empty ones dropped, no duplicates, sorted.
fn normalized_paths(paths: &[String]) -> Vec<String> {
    let mut out: Vec<String> =
        paths.iter().flat_map(|p| p.lines()).map(str::trim).filter(|p| !p.is_empty()).map(str::to_owned).collect();
    out.sort();
    out.dedup();
    out
}

/// `asset.update`: fields of the revision and/or paths of the image; what does not change stays
/// untouched. Both together are **one** mirror commit. Nothing changed → `changed: false`.
async fn update(state: &AppState, work: WorkId, p: &UpdateParams) -> Result<serde_json::Value, RpcError> {
    let store = &state.store;
    let held = state.write_lock.lock().await;
    let revisions = store.asset_revisions(work, &p.name).await?;
    let Some(old) = revisions.iter().find(|r| r.number == p.rev) else {
        let numbers: Vec<String> = revisions.iter().map(|r| r.number.to_string()).collect();
        return Err(StoreError::Asset(format!(
            "Bild {}: Fassung {} gibt es nicht; vorhanden: {}",
            p.name,
            p.rev,
            numbers.join(", ")
        ))
        .into());
    };
    let differs = |new: &Option<String>, old: &str| new.as_deref().is_some_and(|n| n.replace("\r\n", "\n") != old);
    let fields = differs(&p.caption, &old.caption) || differs(&p.alt, &old.alt) || differs(&p.note, &old.note);
    let current_paths =
        store.asset_list(work).await?.into_iter().find(|a| a.name == p.name).map(|a| a.paths).unwrap_or_default();
    let paths = p.paths.as_deref().map(normalized_paths).filter(|new| *new != current_paths);
    if !fields && paths.is_none() {
        return Ok(json!({ "name": p.name, "rev": p.rev, "changed": false }));
    }
    let mut change: Option<AssetChange> = None;
    if fields {
        change = Some(
            store
                .update_asset_revision(
                    work,
                    &p.name,
                    p.rev,
                    p.caption.as_deref(),
                    p.alt.as_deref(),
                    p.note.as_deref(),
                    HUMAN_NAME,
                )
                .await?,
        );
    }
    if let Some(paths) = &paths {
        let moved = store.set_asset_paths(work, &p.name, paths).await;
        let moved = match (moved, &change) {
            (Ok(moved), _) => moved,
            // The fields are already stored: then at least log those.
            (Err(e), Some(done)) => {
                let _ = mirror::record_asset(state, &held, done).await;
                return Err(e.into());
            }
            (Err(e), None) => return Err(e.into()),
        };
        change = Some(match change {
            Some(mut first) => {
                first.detail.push('\n');
                first.detail.push_str(&moved.detail);
                first
            }
            None => moved,
        });
    }
    let change = change.expect("mindestens eine Änderung");
    let mut answer = json!({ "name": p.name, "rev": p.rev, "changed": true });
    add_mirror(&mut answer, mirror::record_asset(state, &held, &change).await);
    Ok(answer)
}

/// `asset.delete`: one revision (`rev`) or the whole image.
async fn delete(state: &AppState, work: WorkId, name: &str, rev: Option<u32>) -> Result<serde_json::Value, RpcError> {
    let held = state.write_lock.lock().await;
    // Round 20c: if the whole image is the cover, first „Umschlag entfernt“ (own commit) before deleting.
    let cover = match rev {
        None => crate::cover::release_cover_before_delete(state, &held, work, name).await?,
        Some(_) => None,
    };
    let change = match rev {
        Some(rev) => state.store.delete_asset_revision(work, name, rev, HUMAN_NAME).await?,
        None => state.store.delete_asset(work, name, HUMAN_NAME).await?,
    };
    let mut answer = json!({ "name": change.name, "deleted": true });
    if let Some(result) = cover {
        answer["cover_removed"] = json!(true);
        match result {
            Ok(Some(commit)) => answer["cover_commit"] = json!(commit),
            Ok(None) => {}
            Err(e) => answer["cover_mirror"] = json!(e),
        }
    }
    if let Some(rev) = rev {
        answer["rev"] = json!(rev);
    }
    add_mirror(&mut answer, mirror::record_asset(state, &held, &change).await);
    Ok(answer)
}

// ---- Templates ---------------------------------------------------------------------------------

/// Own template environment of the image pages.
fn templates() -> &'static Environment<'static> {
    static ENV: OnceLock<Environment<'static>> = OnceLock::new();
    ENV.get_or_init(|| {
        let mut env = Environment::new();
        for (name, source) in [
            ("base.html", include_str!("../templates/base.html")),
            ("assets.html", include_str!("../templates/assets.html")),
            ("asset.html", include_str!("../templates/asset.html")),
        ] {
            env.add_template(name, source).expect("Vorlage der Bilder-Seiten fehlerhaft");
        }
        env
    })
}

fn page_settings(headers: &HeaderMap) -> Value {
    let settings = Settings::from_cookie_header(headers.get(header::COOKIE).and_then(|v| v.to_str().ok()));
    Value::from(Serde(settings))
}

fn render(status: StatusCode, name: &str, ctx: Value) -> Response {
    match templates().get_template(name).and_then(|t| t.render(ctx)) {
        Ok(html) => (status, Html(html)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, format!("Fehler: Vorlage {name}: {e}")).into_response(),
    }
}

/// HTTP status for an RPC error: invalid 422, refused 409, otherwise 500.
fn status_of(e: &RpcError) -> StatusCode {
    match e.code {
        INVALID_PARAMS => StatusCode::UNPROCESSABLE_ENTITY,
        REFUSED => StatusCode::CONFLICT,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

/// Self-built address (digits, image names from `[a-z0-9._-]`, percent-encoded values): escape only
/// `&` so that `/` stays readable (minijinja otherwise escapes it as `&#x2f;`).
fn href(url: String) -> Value {
    debug_assert!(!url.contains(['"', '<', '>', '\'', ' ']), "unsichere Adresse {url:?}");
    Value::from_safe_string(url.replace('&', "&amp;"))
}

/// Text for the template, HTML-escaped but with readable `/` (paths like `/bilder/haus.svg`;
/// minijinja otherwise escapes `/` as `&#x2f;`).
pub(crate) fn path_text(text: &str) -> Value {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#x27;"),
            c => out.push(c),
        }
    }
    Value::from_safe_string(out)
}

/// Readable size: „512 Bytes“, „3,2 KiB“, „1,5 MiB“.
fn size_text(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} Bytes")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KiB", bytes as f64 / 1024.0).replace('.', ",")
    } else {
        format!("{:.1} MiB", bytes as f64 / (1024.0 * 1024.0)).replace('.', ",")
    }
}

/// Timestamp `2026-11-05T10:11:12.123Z` (UTC) → `2026-11-05 11:11` in local time.
fn short_time(time: &str) -> String {
    crate::localtime::minutes(time)
}

fn type_label(mime: &str) -> &'static str {
    match mime {
        "image/png" => "PNG",
        "image/jpeg" => "JPEG",
        "image/gif" => "GIF",
        "image/webp" => "WebP",
        "image/svg+xml" => "SVG",
        _ => "Bild",
    }
}

/// Notice after a redirect (`?hinweis=…`).
fn notice(code: Option<&str>) -> Option<&'static str> {
    Some(match code? {
        "hinzugefuegt" => "Das Bild ist hinzugefügt.",
        "ersetzt" => "Neue Fassung hochgeladen; sie ist jetzt aktuell.",
        "umgeschaltet" => "Die gewählte Fassung ist jetzt aktuell.",
        "geaendert" => "Gespeichert.",
        "unveraendert" => "Nichts geändert.",
        "fassung-geloescht" => "Die Fassung ist gelöscht; ihr Inhalt steht im Git-Spiegel.",
        "geloescht" => "Das Bild ist gelöscht; sein Inhalt steht im Git-Spiegel.",
        // Round 11: cover (forms of “cover”) and requests (of “mcp-assets”) return here.
        "umschlag_gesetzt" => "Das Bild ist jetzt der Umschlag des Werks.",
        "umschlag_entfernt" => "Das Werk hat keinen Umschlag mehr.",
        "antrag_bestaetigt" => "Der Antrag ist bestätigt und ausgeführt.",
        "antrag_abgelehnt" => "Der Antrag ist abgelehnt.",
        "antrag_veraltet" => {
            "Der Antrag ist veraltet (das Bild hat sich inzwischen geändert) und wurde nicht ausgeführt."
        }
        "endung_angepasst" => "Neue Fassung hochgeladen; das Bild heißt jetzt mit der neuen Endung, die Pfade blieben.",
        "pfad_zugeordnet" => "Der Pfad gehört jetzt in allen Schreibweisen zum gewählten Bild.",
        _ => return None,
    })
}

// ---- Hooks to other tasks (contract Round 11) ---------------------------------------------------

/// Name of the cover image of the work (`None` = no cover).
async fn hook_cover_name(state: &AppState, work: WorkId) -> Option<String> {
    crate::cover::cover_name(state, work).await.ok().flatten()
}

/// HTML block id="umschlag" for the page „Bilder“ (top).
async fn hook_cover_panel(state: &AppState, work: WorkId) -> String {
    crate::cover::assets_panel(state, work).await.unwrap_or_else(|e| panel_error("Umschlag", &e.message))
}

/// HTML block id="antraege" (requests about images) for the page „Bilder“ (bottom).
async fn hook_requests_panel(state: &AppState, work: WorkId) -> String {
    crate::asset_requests::panel(state, work).await.unwrap_or_else(|e| panel_error("Anträge", &e.message))
}

/// Honest replacement if a block of the images page could not be built (text escaped).
fn panel_error(what: &str, message: &str) -> String {
    let escape = |s: &str| s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;");
    format!("<p class=\"notice error\">{}: {}</p>", escape(what), escape(message))
}

/// Smallest, largest and default longest side of a thumbnail in pixels (`?b=`).
pub const THUMB_MIN: u32 = 64;
pub const THUMB_MAX: u32 = 1024;
pub const THUMB_DEFAULT: u32 = 320;

fn summary_row(work: WorkId, a: &AssetSummary, cover: Option<&str>) -> Value {
    let base = format!("/werke/{work}/bilder/{}", a.name);
    context! {
        name => a.name.as_str(), mime => a.mime.as_str(), kind => type_label(&a.mime),
        encoding => a.encoding.as_str(), size => size_text(a.bytes), current => a.current,
        revisions => a.revisions, caption => a.caption.as_str(), alt => a.alt.as_str(),
        paths => a.paths.iter().map(|p| path_text(p)).collect::<Vec<_>>(), updated => short_time(&a.updated),
        href => href(base.clone()), file_href => href(format!("{base}/datei")),
        thumb_href => href(format!("{base}/vorschau?b={THUMB_DEFAULT}")),
        is_cover => cover == Some(a.name.as_str()),
    }
}

/// Values of the form „Bild hinzufügen“ (to show again after an error).
#[derive(Debug, Default, Clone, serde::Serialize)]
struct AddForm {
    name: String,
    path: String,
    caption: String,
    alt: String,
    note: String,
    as_base64: bool,
}

#[allow(clippy::too_many_arguments)]
async fn list_response(
    state: &AppState,
    headers: &HeaderMap,
    work: WorkId,
    status: StatusCode,
    form: AddForm,
    message: Option<String>,
    hint: Option<&str>,
    query: Option<&str>,
) -> Response {
    let meta = match state.store.work_meta(work).await {
        Ok(meta) => meta,
        Err(StoreError::UnknownWork(_)) => return (StatusCode::NOT_FOUND, "unbekanntes Werk").into_response(),
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, format!("Fehler: {e}")).into_response(),
    };
    let query = query.map(str::trim).filter(|q| !q.is_empty());
    let assets = match state.store.asset_search(work, query.unwrap_or("")).await {
        Ok(assets) => assets,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, format!("Fehler: {e}")).into_response(),
    };
    let total = match query {
        Some(_) => state.store.asset_list(work).await.map(|all| all.len()).unwrap_or(assets.len()),
        None => assets.len(),
    };
    let cover = hook_cover_name(state, work).await;
    let rows: Vec<Value> = assets.iter().map(|a| summary_row(work, a, cover.as_deref())).collect();
    // Round 21c: legacy data with two images for one path (notice and form at the top of the page).
    let found = state.store.asset_path_conflicts(work).await.unwrap_or_default();
    let conflicts: Vec<Value> = found
        .iter()
        .map(|c| {
            let mut choices: Vec<String> = c.paths.iter().map(|p| p.asset.clone()).collect();
            choices.sort_unstable();
            choices.dedup();
            context! {
                normal => c.normal.clone(), normal_text => path_text(&c.normal),
                paths => c.paths.iter().map(|p| context! { path_text => path_text(&p.path), asset => p.asset.clone() }).collect::<Vec<_>>(),
                choices => choices,
            }
        })
        .collect();
    let ctx = context! {
        conflicts => conflicts,
        settings => page_settings(headers), work => context! { id => work, title => meta.title },
        assets => rows, form => Value::from(Serde(form)), error => message, notice => notice(hint),
        max_mib => MAX_ASSET_BYTES / (1024 * 1024), query => query, total => total,
        cover_panel => hook_cover_panel(state, work).await,
        requests_panel => hook_requests_panel(state, work).await,
    };
    render(status, "assets.html", ctx)
}

#[allow(clippy::too_many_arguments)]
async fn detail_response(
    state: &AppState,
    headers: &HeaderMap,
    work: WorkId,
    name: &str,
    shown: Option<u32>,
    status: StatusCode,
    message: Option<String>,
    hint: Option<&str>,
) -> Response {
    let meta = match state.store.work_meta(work).await {
        Ok(meta) => meta,
        Err(StoreError::UnknownWork(_)) => return (StatusCode::NOT_FOUND, "unbekanntes Werk").into_response(),
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, format!("Fehler: {e}")).into_response(),
    };
    let (summary, revision) = match summary_and_revision(state, work, name, shown).await {
        Ok(found) => found,
        Err(e) if e.code == INVALID_PARAMS => {
            return (StatusCode::NOT_FOUND, format!("Fehler: {}", e.message)).into_response();
        }
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, format!("Fehler: {}", e.message)).into_response(),
    };
    let revisions = match state.store.asset_revisions(work, name).await {
        Ok(revisions) => revisions,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, format!("Fehler: {e}")).into_response(),
    };
    let base = format!("/werke/{work}/bilder/{name}");
    let list: Vec<Value> = revisions
        .iter()
        .rev()
        .map(|r| {
            context! {
                number => r.number, based_on => r.based_on, current => r.current, kind => type_label(&r.mime),
                encoding => r.encoding.as_str(), size => size_text(r.bytes), author => r.author.as_str(),
                note => r.note.as_str(), caption => r.caption.as_str(), created => short_time(&r.created),
                edited => r.edited.as_deref().map(short_time), shown => r.number == revision.number,
                href => href(format!("{base}?fassung={}", r.number)),
            }
        })
        .collect();
    let shown_rev = context! {
        number => revision.number, current => revision.current, kind => type_label(&revision.mime),
        mime => revision.mime.as_str(), encoding => revision.encoding.as_str(), size => size_text(revision.bytes),
        sha256 => revision.sha256.as_str(), caption => revision.caption.as_str(), alt => revision.alt.as_str(),
        note => revision.note.as_str(), author => revision.author.as_str(), created => short_time(&revision.created),
        edited => revision.edited.as_deref().map(short_time),
        file_href => href(format!("{base}/datei?fassung={}", revision.number)),
    };
    let ctx = context! {
        settings => page_settings(headers), work => context! { id => work, title => meta.title },
        asset => summary_row(work, &summary, hook_cover_name(state, work).await.as_deref()), paths_text => path_text(&summary.paths.join("\n")), revision => shown_rev,
        revisions => list, error => message, notice => notice(hint), max_mib => MAX_ASSET_BYTES / (1024 * 1024),
    };
    render(status, "asset.html", ctx)
}

// ---- Pages -------------------------------------------------------------------------------------

/// Pages of the images (read only).
pub fn pages() -> Router<Arc<AppState>> {
    Router::new()
        .route("/werke/{id}/bilder", get(list_page))
        .route("/werke/{id}/bilder/pfad", get(by_path))
        .route("/werke/{id}/bilder/{name}", get(detail_page))
        .route("/werke/{id}/bilder/{name}/datei", get(file))
        .route("/werke/{id}/bilder/{name}/vorschau", get(thumbnail))
}

/// Forms (writing, in the router behind `guard::same_origin`). Only the upload accepts large
/// requests ([`UPLOAD_LIMIT`]).
pub fn writes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/werke/bilder/hochladen", post(upload).layer(DefaultBodyLimit::max(UPLOAD_LIMIT)))
        .route("/werke/bilder/act", post(act))
        .route("/werke/bilder/einfuegen", post(insert))
}

#[derive(Deserialize)]
struct PageQuery {
    fassung: Option<String>,
    hinweis: Option<String>,
    /// Search (Round 11): name, caption, alternative text.
    q: Option<String>,
}

async fn list_page(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<WorkId>,
    Query(q): Query<PageQuery>,
) -> Response {
    let hint = q.hinweis.as_deref();
    list_response(&state, &headers, id, StatusCode::OK, AddForm::default(), None, hint, q.q.as_deref()).await
}

async fn detail_page(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path((id, name)): Path<(WorkId, String)>,
    Query(q): Query<PageQuery>,
) -> Response {
    let shown = q.fassung.as_deref().and_then(|v| v.trim().parse().ok());
    detail_response(&state, &headers, id, &name, shown, StatusCode::OK, None, q.hinweis.as_deref()).await
}

/// Bytes of a revision with its MIME type; `nosniff` and a CSP with `sandbox` so that an SVG never
/// runs scripts or loads foreign content.
fn image_response(data: &AssetData) -> Response {
    let bytes = match data.bytes() {
        Ok(bytes) => bytes,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, format!("Fehler: {e}")).into_response(),
    };
    let mime =
        HeaderValue::from_str(&data.mime).unwrap_or_else(|_| HeaderValue::from_static("application/octet-stream"));
    (
        [
            (header::CONTENT_TYPE, mime),
            (header::X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff")),
            (header::CONTENT_SECURITY_POLICY, HeaderValue::from_static(IMAGE_CSP)),
            (header::CACHE_CONTROL, HeaderValue::from_static("no-cache")),
        ],
        bytes,
    )
        .into_response()
}

fn not_found(e: StoreError) -> Response {
    match e {
        StoreError::UnknownWork(_) | StoreError::UnknownAsset { .. } | StoreError::Asset(_) => {
            (StatusCode::NOT_FOUND, format!("Fehler: {e}")).into_response()
        }
        e => (StatusCode::INTERNAL_SERVER_ERROR, format!("Fehler: {e}")).into_response(),
    }
}

async fn file(
    State(state): State<Arc<AppState>>,
    Path((id, name)): Path<(WorkId, String)>,
    Query(q): Query<PageQuery>,
) -> Response {
    let rev = q.fassung.as_deref().and_then(|v| v.trim().parse().ok());
    match state.store.asset_data(id, &name, rev).await {
        Ok(data) => image_response(&data),
        Err(e) => not_found(e),
    }
}

#[derive(Deserialize)]
struct ThumbQuery {
    b: Option<String>,
    fassung: Option<String>,
}

/// Shrink a raster image (Round 11): longest side at most `bound`, aspect ratio stays, a
/// smaller image keeps its size. JPEG stays JPEG (no alpha), everything else becomes PNG (for GIF the
/// first frame). Result: bytes and MIME type; an unreadable image → `Err` with message.
pub fn thumbnail_bytes(bytes: &[u8], mime: &str, bound: u32) -> Result<(Vec<u8>, &'static str), String> {
    let mut reader = image::ImageReader::new(std::io::Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| format!("Bild nicht lesbar: {e}"))?;
    let mut limits = image::Limits::default();
    // At most 20 000 pixels per side and 512 MiB when unpacking (protection against “pixel bombs”).
    limits.max_image_width = Some(20_000);
    limits.max_image_height = Some(20_000);
    limits.max_alloc = Some(512 * 1024 * 1024);
    reader.limits(limits);
    let picture = reader.decode().map_err(|e| format!("Bild nicht lesbar: {e}"))?;
    let picture = if picture.width().max(picture.height()) > bound { picture.thumbnail(bound, bound) } else { picture };
    let mut out = std::io::Cursor::new(Vec::new());
    let mime = if mime == "image/jpeg" {
        image::DynamicImage::ImageRgb8(picture.to_rgb8())
            .write_to(&mut out, image::ImageFormat::Jpeg)
            .map_err(|e| format!("Vorschau ließ sich nicht schreiben: {e}"))?;
        "image/jpeg"
    } else {
        picture
            .write_to(&mut out, image::ImageFormat::Png)
            .map_err(|e| format!("Vorschau ließ sich nicht schreiben: {e}"))?;
        "image/png"
    };
    Ok((out.into_inner(), mime))
}

/// `GET /werke/{id}/bilder/{name}/vorschau?b=<px>[&fassung=n]` (Round 11): thumbnail for lists.
/// `b` 64–1024 (default 320, otherwise 400); raster shrunk ([`thumbnail_bytes`]), SVG unchanged
/// like `/datei` (CSP `sandbox`). `ETag` from SHA-256 of the revision and `b`; `If-None-Match` → 304.
async fn thumbnail(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path((id, name)): Path<(WorkId, String)>,
    Query(q): Query<ThumbQuery>,
) -> Response {
    let bound = match q.b.as_deref().map(str::trim) {
        None | Some("") => THUMB_DEFAULT,
        Some(text) => match text.parse::<u32>() {
            Ok(b) if (THUMB_MIN..=THUMB_MAX).contains(&b) => b,
            _ => {
                let message = format!("b: längste Seite in Pixeln zwischen {THUMB_MIN} und {THUMB_MAX} angeben.");
                return (StatusCode::BAD_REQUEST, message).into_response();
            }
        },
    };
    let rev = q.fassung.as_deref().and_then(|v| v.trim().parse().ok());
    let data = match state.store.asset_data(id, &name, rev).await {
        Ok(data) => data,
        Err(e) => return not_found(e),
    };
    if data.encoding == AssetEncoding::Svg || data.mime == "image/svg+xml" {
        return image_response(&data);
    }
    let sha = match state.store.asset_revisions(id, &name).await {
        Ok(revisions) => revisions.into_iter().find(|r| r.number == data.rev).map(|r| r.sha256).unwrap_or_default(),
        Err(e) => return not_found(e),
    };
    let etag = format!("\"{sha}-{bound}\"");
    let matches = headers
        .get(header::IF_NONE_MATCH)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.split(',').any(|tag| tag.trim() == etag));
    let etag_value = HeaderValue::from_str(&etag).unwrap_or_else(|_| HeaderValue::from_static("\"\""));
    if matches {
        return (StatusCode::NOT_MODIFIED, [(header::ETAG, etag_value)]).into_response();
    }
    let bytes = match data.bytes() {
        Ok(bytes) => bytes,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, format!("Fehler: {e}")).into_response(),
    };
    let mime = data.mime.clone();
    // At most two thumbnails at once: each unpacks the whole raster image (memory), a list with
    // many images must not flood the machine (review Round 11).
    static THUMB_SLOTS: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(2);
    let Ok(_slot) = THUMB_SLOTS.acquire().await else {
        return (StatusCode::SERVICE_UNAVAILABLE, "Fehler: Vorschaubilder nicht verfügbar").into_response();
    };
    let made = tokio::task::spawn_blocking(move || thumbnail_bytes(&bytes, &mime, bound)).await;
    match made {
        Ok(Ok((bytes, mime))) => (
            [
                (header::CONTENT_TYPE, HeaderValue::from_static(mime)),
                (header::X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff")),
                (header::CONTENT_SECURITY_POLICY, HeaderValue::from_static(IMAGE_CSP)),
                (header::CACHE_CONTROL, HeaderValue::from_static("no-cache")),
                (header::ETAG, etag_value),
            ],
            bytes,
        )
            .into_response(),
        Ok(Err(message)) => (StatusCode::UNPROCESSABLE_ENTITY, format!("Fehler: {message}")).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, format!("Fehler: {e}")).into_response(),
    }
}

#[derive(Deserialize)]
struct PathQuery {
    p: Option<String>,
}

/// `GET /werke/{id}/bilder/pfad?p=…`: the current image for the path from template/text (for the typeset view).
async fn by_path(State(state): State<Arc<AppState>>, Path(id): Path<WorkId>, Query(q): Query<PathQuery>) -> Response {
    let Some(path) = q.p.filter(|p| !p.trim().is_empty()) else {
        return (StatusCode::NOT_FOUND, "Fehler: kein Pfad").into_response();
    };
    match state.store.asset_by_path(id, &path).await {
        Ok(Some(data)) => image_response(&data),
        Ok(None) => (StatusCode::NOT_FOUND, format!("Kein Bild zum Pfad „{path}“.")).into_response(),
        Err(e) => not_found(e),
    }
}

// ---- Forms -------------------------------------------------------------------------------------

fn checked(value: Option<&str>) -> bool {
    matches!(value.map(str::trim), Some("1" | "on" | "true"))
}

/// `POST /werke/bilder/hochladen` (multipart): `work`, `file`, `name` (existing = new revision
/// of this image), `path` (one per line), `caption`, `alt`, `note`, `as_base64`. Success → 303 to the
/// detail page; error → page with message (422, 409, too large 413).
async fn upload(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    multipart: Result<Multipart, MultipartRejection>,
) -> Response {
    let mut form = AddForm::default();
    let mut work: Option<WorkId> = None;
    let mut file: Option<(String, Vec<u8>)> = None;
    let mut as_base64: Option<String> = None;
    let mut rename: Option<String> = None;
    let failed = |work: Option<WorkId>, status: StatusCode, form: AddForm, message: String| {
        let state = state.clone();
        let headers = headers.clone();
        async move {
            match work {
                Some(work) => list_response(&state, &headers, work, status, form, Some(message), None, None).await,
                None => (status, format!("Fehler: {message}")).into_response(),
            }
        }
    };
    let too_large = || format!("Das Bild ist größer als {} MiB.", MAX_ASSET_BYTES / (1024 * 1024));
    let mut multipart = match multipart {
        Ok(multipart) => multipart,
        Err(e) => {
            let message = format!("Das Formular kam nicht als Datei-Upload an ({}).", e.body_text());
            return failed(None, StatusCode::UNPROCESSABLE_ENTITY, form, message).await;
        }
    };
    loop {
        let field = match multipart.next_field().await {
            Ok(Some(field)) => field,
            Ok(None) => break,
            Err(e) => {
                let (status, message) = if e.status() == StatusCode::PAYLOAD_TOO_LARGE {
                    (StatusCode::PAYLOAD_TOO_LARGE, too_large())
                } else {
                    (
                        StatusCode::UNPROCESSABLE_ENTITY,
                        format!("Das Formular ließ sich nicht lesen ({}).", e.body_text()),
                    )
                };
                return failed(work, status, form, message).await;
            }
        };
        let name = field.name().unwrap_or("").to_owned();
        if name == "file" {
            let file_name = field.file_name().unwrap_or("").to_owned();
            match field.bytes().await {
                Ok(bytes) => file = Some((file_name, bytes.to_vec())),
                Err(e) => {
                    let (status, message) = if e.status() == StatusCode::PAYLOAD_TOO_LARGE {
                        (StatusCode::PAYLOAD_TOO_LARGE, too_large())
                    } else {
                        (
                            StatusCode::UNPROCESSABLE_ENTITY,
                            format!("Die Datei ließ sich nicht lesen ({}).", e.body_text()),
                        )
                    };
                    return failed(work, status, form, message).await;
                }
            }
            continue;
        }
        let text = field.text().await.unwrap_or_default();
        match name.as_str() {
            "work" => work = text.trim().parse().ok(),
            "name" => form.name = text.trim().to_owned(),
            "path" => form.path = text,
            "caption" => form.caption = text,
            "alt" => form.alt = text,
            "note" => form.note = text,
            "as_base64" => as_base64 = Some(text),
            "rename" => rename = Some(text),
            _ => {}
        }
    }
    form.as_base64 = checked(as_base64.as_deref());
    let Some(work) = work else {
        return failed(None, StatusCode::UNPROCESSABLE_ENTITY, form, "Werk fehlt.".into()).await;
    };
    let Some((file_name, bytes)) = file.filter(|(name, _)| !name.trim().is_empty()) else {
        return failed(Some(work), StatusCode::UNPROCESSABLE_ENTITY, form, "Bitte eine Bilddatei wählen.".into()).await;
    };
    let base = file_name.rsplit(['/', '\\']).next().unwrap_or("").to_owned();
    if bytes.len() > MAX_ASSET_BYTES {
        return failed(Some(work), StatusCode::PAYLOAD_TOO_LARGE, form, too_large()).await;
    }
    // Existing name → new revision of this image.
    let existing = !form.name.is_empty()
        && state.store.asset_list(work).await.is_ok_and(|list| list.iter().any(|a| a.name == form.name));
    let result = {
        let held = state.write_lock.lock().await;
        let change = if existing {
            let rename = checked(rename.as_deref());
            state
                .store
                .replace_asset_renaming(work, &form.name, &base, &bytes, form.as_base64, &form.note, HUMAN_NAME, rename)
                .await
        } else {
            let new = NewAsset {
                file_name: &base,
                name: Some(form.name.as_str()).filter(|n| !n.is_empty()),
                bytes: &bytes,
                as_base64: form.as_base64,
                path: Some(form.path.as_str()),
                caption: &form.caption,
                alt: &form.alt,
                note: &form.note,
            };
            state.store.add_asset(work, &new, HUMAN_NAME).await
        };
        match change {
            Ok(change) => {
                let _ = mirror::record_asset(&state, &held, &change).await;
                Ok(change)
            }
            Err(e) => Err(RpcError::from(e)),
        }
    };
    match result {
        Ok(change) => {
            let hint = match (existing, change.name == form.name) {
                (true, true) => "ersetzt",
                (true, false) => "endung_angepasst",
                (false, _) => "hinzugefuegt",
            };
            Redirect::to(&format!("/werke/{work}/bilder/{}?hinweis={hint}", change.name)).into_response()
        }
        Err(e) if existing => {
            let name = form.name.clone();
            detail_response(&state, &headers, work, &name, None, status_of(&e), Some(e.message), None).await
        }
        Err(e) => failed(Some(work), status_of(&e), form, e.message).await,
    }
}

/// Typst string literal (`"…"`, `\` and `"` escaped, line breaks as `\n`).
fn typst_string(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => {}
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Text as Typst markup in `[...]`: every character with special meaning gets `\`, line breaks
/// become spaces (a caption is one line).
fn typst_markup(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.split(['\r', '\n']).collect::<Vec<_>>().join(" ").trim().chars() {
        if matches!(
            c,
            '\\' | '[' | ']' | '#' | '*' | '_' | '$' | '<' | '>' | '@' | '`' | '~' | '/' | '"' | '=' | '-' | '+'
        ) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// Figure for the section text: `#figure(image("<path>"[, alt: "…"])[, caption: [...]])`.
pub fn figure_source(path: &str, caption: &str, alt: &str) -> String {
    let mut image = format!("image({}", typst_string(path));
    if !alt.trim().is_empty() {
        image.push_str(&format!(", alt: {}", typst_string(alt.trim())));
    }
    image.push(')');
    let caption = caption.trim();
    if caption.is_empty() {
        format!("#figure({image})")
    } else {
        format!("#figure({image}, caption: [{}])", typst_markup(caption))
    }
}

/// Form „Bild einfügen“ from the editor (Round 11; place and unsaved text since Round 18c).
#[derive(Debug, Default, Deserialize)]
struct InsertForm {
    work: Option<String>,
    key: Option<String>,
    /// Revision the new one builds on (the one shown).
    rev: Option<String>,
    name: Option<String>,
    caption: Option<String>,
    /// Place: `end` (default), `start`, `block:<n>`, `offset:<n>` ([`Place`]).
    place: Option<String>,
    /// Complete, not yet saved text of the editor (Round 18c); then the place applies within it.
    body: Option<String>,
}

/// Place where „Bild einfügen“ inserts the figure as its own block (Round 18c).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Place {
    /// At the end of the section (as since Round 11).
    End,
    /// At the start, directly below the heading of the section.
    Start,
    /// Before the top-level block `n` (counted by `baruch_wysiwyg::parse`); `n` ≥ block count = end.
    Block(usize),
    /// UTF-16 position in the text (like `selectionStart` in the browser); inserted at the next line boundary ≥ `n`.
    Offset(usize),
}

impl Place {
    /// `end` | `start` | `block:<n>` | `offset:<n>`; empty = `end`.
    pub fn parse(value: Option<&str>) -> Option<Place> {
        let value = value.map(str::trim).unwrap_or("");
        match value {
            "" | "end" => Some(Place::End),
            "start" => Some(Place::Start),
            _ => {
                let (kind, n) = value.split_once(':')?;
                let n = n.trim().parse::<usize>().ok()?;
                match kind.trim() {
                    "block" => Some(Place::Block(n)),
                    "offset" => Some(Place::Offset(n)),
                    _ => None,
                }
            }
        }
    }
}

/// Is the block the head of a section (heading, part, unnumbered item)?
fn is_title(block: &baruch_wysiwyg::Block) -> bool {
    matches!(
        block.kind,
        baruch_wysiwyg::BlockKind::Heading { .. }
            | baruch_wysiwyg::BlockKind::Part { .. }
            | baruch_wysiwyg::BlockKind::Unnumbered { .. }
    )
}

/// Byte position for a UTF-16 position (clamped to the end of the text; inside a character → after it).
fn byte_of_utf16(text: &str, units: usize) -> usize {
    let mut seen = 0;
    for (at, c) in text.char_indices() {
        if seen >= units {
            return at;
        }
        seen += c.len_utf16();
    }
    text.len()
}

/// Open brackets (`[`, `(`, `{`) at the end of `text` (Round 21c): characters escaped with `\`, raw text in
/// backticks and strings in code brackets (`(`/`{`) do not count. Roughly like Typst, enough to tell whether
/// a line boundary lies inside a multi-line inline construct (`#footnote[eins⏎zwei]`, `#link(…)[…]`).
fn open_brackets(text: &str) -> usize {
    let mut stack: Vec<char> = Vec::new();
    let mut chars = text.chars();
    let mut in_string = false;
    while let Some(c) = chars.next() {
        if in_string {
            match c {
                '\\' => {
                    chars.next();
                }
                '"' => in_string = false,
                _ => {}
            }
            continue;
        }
        match c {
            '\\' => {
                chars.next();
            }
            '`' => {
                for d in chars.by_ref() {
                    if d == '`' {
                        break;
                    }
                }
            }
            '"' if matches!(stack.last(), Some('(' | '{')) => in_string = true,
            '[' | '(' | '{' => stack.push(c),
            ']' | ')' | '}' => {
                stack.pop();
            }
            _ => {}
        }
    }
    stack.len()
}

/// Text with the figure `figure` as its own block at the place `place` (pure, never fails).
///
/// - Never before or inside the heading of the section (the first block if it is a heading, part or unnumbered
///   item): places before it move directly behind it.
/// - `Offset`: at the next line boundary ≥ the position; if it lies inside a block that is not a paragraph
///   (island, scene, quote, list …), behind that block – so Typst code stays whole. Since Round 21c also in a
///   paragraph: if the line boundary lies in an open multi-line inline construct (`#footnote[eins⏎zwei]`,
///   `#link(…)[…]`, [`open_brackets`]), the figure moves to the next line boundary at which all brackets are
///   closed, otherwise behind the paragraph.
/// - An empty line before it (otherwise the figure would belong to the paragraph before), one after it; at the end
///   as since Round 11 `text without trailing whitespace + "\n\n" + figure + "\n"`.
pub fn insert_figure_at(text: &str, place: Place, figure: &str) -> String {
    let blocks = baruch_wysiwyg::parse(text).blocks;
    let mut starts = Vec::with_capacity(blocks.len() + 1);
    let mut at = 0;
    for block in &blocks {
        starts.push(at);
        at += block.raw.len();
    }
    starts.push(at.min(text.len()));
    let title_end = blocks.first().filter(|b| is_title(b)).map_or(0, |b| b.raw.len().min(text.len()));
    let at = match place {
        Place::End => text.len(),
        Place::Start => title_end,
        Place::Block(n) if n >= blocks.len() => text.len(),
        Place::Block(n) => starts[n],
        Place::Offset(units) => {
            let byte = byte_of_utf16(text, units);
            let mut line = if byte == 0 || text[..byte].ends_with('\n') {
                byte
            } else {
                text[byte..].find('\n').map_or(text.len(), |k| byte + k + 1)
            };
            if let Some(k) = (0..blocks.len()).find(|&k| starts[k] < line && line < starts[k + 1]) {
                if !matches!(blocks[k].kind, baruch_wysiwyg::BlockKind::Paragraph { .. }) {
                    line = starts[k + 1];
                } else if open_brackets(&text[starts[k]..line]) > 0 {
                    // Inside a multi-line inline construct: the next line boundary in the paragraph at which
                    // all brackets are closed, otherwise the end of the paragraph.
                    let end = starts[k + 1].min(text.len());
                    let mut next = end;
                    let mut at = line;
                    while let Some(k2) = text[at..end].find('\n') {
                        let candidate = at + k2 + 1;
                        if open_brackets(&text[starts[k]..candidate]) == 0 {
                            next = candidate;
                            break;
                        }
                        at = candidate;
                    }
                    line = next;
                }
            }
            // Empty lines after a block belong to it: across them to the start of the next block.
            while line < text.len() {
                let end = text[line..].find('\n').map_or(text.len(), |k| line + k + 1);
                if !text[line..end].trim().is_empty() {
                    break;
                }
                line = end;
            }
            line
        }
    }
    .max(title_end);
    if at >= text.len() || text[at..].trim().is_empty() {
        return format!("{}\n\n{figure}\n", text.trim_end());
    }
    let (before, after) = text.split_at(at);
    let gap = if before.is_empty() || before.ends_with("\n\n") {
        ""
    } else if before.ends_with('\n') {
        "\n"
    } else {
        "\n\n"
    };
    format!("{before}{gap}{figure}\n\n{after}")
}

/// Short excerpt of a block for choosing the place (first non-empty line, at most 40 characters).
fn excerpt(raw: &str) -> String {
    let line = raw.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("");
    let mut out: String = line.chars().take(40).collect();
    if line.chars().count() > 40 {
        out.push('…');
    }
    out
}

/// Places for the form „Bild einfügen“ without JavaScript (Round 18c): `(value of place, label)` –
/// end, start and “before” every top-level block of the revision after the heading.
pub fn image_places(body: &str) -> Vec<(String, String)> {
    let blocks = baruch_wysiwyg::parse(body).blocks;
    let mut out = vec![
        ("end".to_owned(), "am Ende".to_owned()),
        ("start".to_owned(), "am Anfang (unter der Überschrift)".to_owned()),
    ];
    let first = usize::from(blocks.first().is_some_and(is_title));
    for (n, block) in blocks.iter().enumerate().skip(first) {
        if block.raw.trim().is_empty() {
            continue;
        }
        out.push((format!("block:{n}"), format!("vor „{}“", excerpt(&block.raw))));
    }
    out
}

/// First line (heading line) of a section text, without `\r`.
fn heading_line(text: &str) -> &str {
    text.split('\n').next().unwrap_or("").trim_end_matches('\r')
}

/// `POST /werke/bilder/einfuegen` (Round 11, behind `guard::same_origin`; place since Round 18c): `work`, `key`,
/// `rev`, `name`, `caption?`, `place` (`end` | `start` | `block:<n>` | `offset:<n>`, [`Place`]), `body?`
/// (complete unsaved text of the editor; then the place applies within it, and its heading line must be that
/// of the revision `rev`, otherwise 422 „erst speichern“). Creates **as the human** a new revision on `rev` that
/// carries `#figure(image("<path>"), caption: [...])` as its own block at the place ([`insert_figure_at`];
/// empty caption → that of the image, alternative text of the image as `alt:`). If the image has no path, it gets
/// `bilder/<name>` – in the **same transaction** as the revision
/// ([`baruch_store::Store::save_human_with_asset_path`]): if saving fails, no path remains. **One**
/// mirror commit (the snapshot also contains the path). Success → 303 to the section; error → 422 (input)
/// or 409 (refused) as text.
async fn insert(State(state): State<Arc<AppState>>, Form(form): Form<InsertForm>) -> Response {
    let number = |v: &Option<String>| v.as_deref().and_then(|v| v.trim().parse::<i64>().ok());
    let (Some(work), Some(key), Some(rev)) = (number(&form.work), number(&form.key), number(&form.rev)) else {
        return (StatusCode::UNPROCESSABLE_ENTITY, "Fehler: Werk, Abschnitt (key) und Fassung (rev) angeben.")
            .into_response();
    };
    let Ok(rev) = u32::try_from(rev) else {
        return (StatusCode::UNPROCESSABLE_ENTITY, "Fehler: Fassung ungültig.").into_response();
    };
    let Some(place) = Place::parse(form.place.as_deref()) else {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            "Fehler: Stelle ungültig (end, start, block:<Nummer> oder offset:<Stelle>).",
        )
            .into_response();
    };
    let name = form.name.as_deref().map(str::trim).unwrap_or("").to_owned();
    if name.is_empty() {
        return (StatusCode::UNPROCESSABLE_ENTITY, "Fehler: Bild wählen.").into_response();
    }
    // Forms send line breaks as CRLF; the position (UTF-16) counts in the browser's text with LF.
    let unsaved = form.body.as_deref().map(|b| b.replace("\r\n", "\n"));
    let result: Result<i64, RpcError> = async {
        let held = state.write_lock.lock().await;
        let store = &state.store;
        let node = store.node_seq(work, key).await?;
        let asset = store
            .asset_list(work)
            .await?
            .into_iter()
            .find(|a| a.name == name)
            .ok_or_else(|| StoreError::UnknownAsset { work, name: name.clone() })?;
        let loaded = store.load_work(work).await?;
        let section = loaded.nodes.get(node).ok_or(StoreError::UnknownNode { work, node })?;
        let base = section.revision(rev).ok_or_else(|| StoreError::UnknownRevision {
            node,
            rev,
            existing: section.revisions.iter().map(|r| r.number).collect(),
        })?;
        let text = match &unsaved {
            Some(body) if heading_line(body) != heading_line(&base.body) => {
                return Err(RpcError::new(
                    INVALID_PARAMS,
                    "Die Überschriftszeile ist geändert – erst speichern, dann das Bild einfügen.",
                ));
            }
            Some(body) => body.as_str(),
            None => base.body.as_str(),
        };
        // The path the image has or gets; it is assigned only together with the revision.
        let path = asset.paths.first().cloned().unwrap_or_else(|| format!("bilder/{}", asset.name));
        let caption = form.caption.as_deref().map(str::trim).filter(|c| !c.is_empty()).unwrap_or(&asset.caption);
        let figure = figure_source(&path, caption, &asset.alt);
        let body = insert_figure_at(text, place, &figure);
        let (change, _) =
            store.save_human_with_asset_path(work, node, &body, HUMAN_NAME, rev, &asset.name, &path).await?;
        if let Some(change) = &change {
            let _ = mirror::record(&state, &held, change).await;
        }
        Ok(key)
    }
    .await;
    match result {
        Ok(key) => Redirect::to(&format!("/?work={work}&key={key}")).into_response(),
        Err(e) => (status_of(&e), format!("Fehler: {}", e.message)).into_response(),
    }
}

/// Form without JavaScript: `action` = `set_current`, `update`, `delete` (one revision) or
/// `remove` (the whole image); deleting only with the checkbox `confirm`.
#[derive(Debug, Default, Deserialize)]
struct ActForm {
    action: String,
    work: Option<String>,
    name: Option<String>,
    rev: Option<String>,
    caption: Option<String>,
    alt: Option<String>,
    paths: Option<String>,
    note: Option<String>,
    confirm: Option<String>,
    /// Round 21c (`resolve_path`): the path in normal form whose collision is resolved.
    normal: Option<String>,
}

async fn act(State(state): State<Arc<AppState>>, headers: HeaderMap, Form(form): Form<ActForm>) -> Response {
    let work: Option<WorkId> = form.work.as_deref().and_then(|w| w.trim().parse().ok());
    let Some(work) = work else {
        return (StatusCode::UNPROCESSABLE_ENTITY, "Fehler: Werk fehlt.").into_response();
    };
    let name = form.name.as_deref().map(str::trim).unwrap_or("").to_owned();
    if form.action == "resolve_path" {
        // Round 21c: legacy data – the human picks the image that keeps the path (one mirror commit).
        let normal = form.normal.as_deref().map(str::trim).unwrap_or("");
        let result = if normal.is_empty() || name.is_empty() {
            Err(RpcError::new(INVALID_PARAMS, "Pfad und Bild wählen."))
        } else {
            call(&state, work, "asset.path_resolve", json!({ "normal": normal, "asset": name })).await
        };
        return match result {
            Ok(_) => Redirect::to(&format!("/werke/{work}/bilder?hinweis=pfad_zugeordnet")).into_response(),
            Err(e) => {
                list_response(&state, &headers, work, status_of(&e), AddForm::default(), Some(e.message), None, None)
                    .await
            }
        };
    }
    if name.is_empty() {
        return (StatusCode::UNPROCESSABLE_ENTITY, "Fehler: Bild fehlt.").into_response();
    }
    let rev: Option<u32> = form.rev.as_deref().and_then(|r| r.trim().parse().ok());
    let confirmed = checked(form.confirm.as_deref());
    let need_rev = || RpcError::new(INVALID_PARAMS, "Fassung fehlt.");
    let unconfirmed = || RpcError::new(INVALID_PARAMS, "Löschen bitte mit dem Kästchen bestätigen.");
    let result: Result<(&str, Option<u32>), RpcError> = match form.action.as_str() {
        "set_current" => match rev {
            Some(rev) => call(&state, work, "asset.set_current", json!({ "name": name, "rev": rev }))
                .await
                .map(|_| ("umgeschaltet", Some(rev))),
            None => Err(need_rev()),
        },
        "update" => match rev {
            Some(rev) => {
                let mut params = json!({ "name": name, "rev": rev });
                for (key, value) in [("caption", &form.caption), ("alt", &form.alt), ("note", &form.note)] {
                    if let Some(value) = value {
                        params[key] = json!(value);
                    }
                }
                if let Some(paths) = &form.paths {
                    params["paths"] = json!([paths]);
                }
                call(&state, work, "asset.update", params)
                    .await
                    .map(|answer| (if answer["changed"] == true { "geaendert" } else { "unveraendert" }, Some(rev)))
            }
            None => Err(need_rev()),
        },
        "delete" => match (rev, confirmed) {
            (None, _) => Err(need_rev()),
            (Some(_), false) => Err(unconfirmed()),
            (Some(rev), true) => {
                call(&state, work, "asset.delete", json!({ "name": name, "rev": rev, "confirm": true }))
                    .await
                    .map(|_| ("fassung-geloescht", None))
            }
        },
        "remove" if !confirmed => Err(unconfirmed()),
        "remove" => call(&state, work, "asset.delete", json!({ "name": name, "confirm": true }))
            .await
            .map(|_| ("geloescht", None)),
        _ => return (StatusCode::BAD_REQUEST, "unbekannte Aktion").into_response(),
    };
    match result {
        Ok(("geloescht", _)) => Redirect::to(&format!("/werke/{work}/bilder?hinweis=geloescht")).into_response(),
        Ok((hint, Some(rev))) => {
            Redirect::to(&format!("/werke/{work}/bilder/{name}?fassung={rev}&hinweis={hint}")).into_response()
        }
        Ok((hint, None)) => Redirect::to(&format!("/werke/{work}/bilder/{name}?hinweis={hint}")).into_response(),
        Err(e) => detail_response(&state, &headers, work, &name, rev, status_of(&e), Some(e.message), None).await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_asset_methods_are_handled_here() {
        for method in [
            "asset.list",
            "asset.search",
            "asset.get",
            "asset.revisions",
            "asset.add",
            "asset.replace",
            "asset.set_current",
            "asset.update",
            "asset.delete",
            "asset.path_conflicts",
            "asset.path_resolve",
        ] {
            assert!(handles(method), "{method} gehört zu den Bildern");
        }
        for method in ["asset.import", "work.list", "assets.list"] {
            assert!(!handles(method), "{method} ist keine Bild-Methode");
        }
    }

    #[test]
    fn paths_from_a_text_field_are_one_per_line_without_doubles() {
        let paths = normalized_paths(&[" /b.svg\r\n\n/a.png\n/b.svg ".to_owned()]);
        assert_eq!(paths, ["/a.png", "/b.svg"], "getrimmt, leer weg, sortiert, ohne Doppelte");
    }

    #[test]
    fn sizes_are_shown_in_german() {
        assert_eq!(size_text(512), "512 Bytes", "unter 1 KiB in Bytes");
        assert_eq!(size_text(3 * 1024 + 200), "3,2 KiB", "Komma statt Punkt");
        assert_eq!(size_text(MAX_ASSET_BYTES as u64), "8,0 MiB", "Grenze");
    }

    #[test]
    fn a_figure_never_splits_an_island_and_never_lands_before_the_heading() {
        let text = "= Hof\n\n#szene[\n  Anna fegt.\n  Der Hund schläft.\n]\n\nDer Abend kommt.\n";
        let inside = "= Hof\n\n#szene[\n  Anna".encode_utf16().count();
        assert_eq!(
            insert_figure_at(text, Place::Offset(inside), "#figure()"),
            "= Hof\n\n#szene[\n  Anna fegt.\n  Der Hund schläft.\n]\n\n#figure()\n\nDer Abend kommt.\n",
            "eine Stelle in der Szene rückt hinter die ganze Szene (Typst-Code bleibt ganz)"
        );
        assert_eq!(
            insert_figure_at(text, Place::Offset(0), "#figure()"),
            "= Hof\n\n#figure()\n\n#szene[\n  Anna fegt.\n  Der Hund schläft.\n]\n\nDer Abend kommt.\n",
            "eine Stelle in der Überschrift rückt dahinter"
        );
        assert_eq!(
            insert_figure_at("= Hof", Place::Start, "#figure()"),
            "= Hof\n\n#figure()\n",
            "nur eine Überschrift ohne Zeilenende: wie am Ende"
        );
        assert_eq!(
            insert_figure_at(text, Place::Offset(10_000), "#figure()"),
            "= Hof\n\n#szene[\n  Anna fegt.\n  Der Hund schläft.\n]\n\nDer Abend kommt.\n\n#figure()\n",
            "eine Stelle hinter dem Text: am Ende"
        );
        assert_eq!(Place::parse(Some("block:3")), Some(Place::Block(3)), "block:<n>");
        // Round 21c: a position in a multi-line footnote of a paragraph puts the figure behind the
        // construct, not into it (otherwise #figure would sit in the footnote).
        let note = "= Hof\n\nAnna fegt#footnote[eins\nzwei] den Hof.\nDer Hund schläft.\n\nDer Abend kommt.\n";
        let in_note = "= Hof\n\nAnna fegt#footnote[ei".encode_utf16().count();
        assert_eq!(
            insert_figure_at(note, Place::Offset(in_note), "#figure()"),
            "= Hof\n\nAnna fegt#footnote[eins\nzwei] den Hof.\n\n#figure()\n\nDer Hund schläft.\n\nDer Abend kommt.\n",
            "die nächste Zeilengrenze mit geschlossenen Klammern liegt hinter „zwei] den Hof.“"
        );
        let link = "= Hof\n\nSiehe #link(\"https://example.org/a)b\")[die\nSeite]\n\nEnde.\n";
        let in_link = "= Hof\n\nSiehe #link(".encode_utf16().count();
        assert_eq!(
            insert_figure_at(link, Place::Offset(in_link), "#figure()"),
            "= Hof\n\nSiehe #link(\"https://example.org/a)b\")[die\nSeite]\n\n#figure()\n\nEnde.\n",
            "eine Klammer in der Zeichenkette zählt nicht; das Konstrukt endet erst nach „Seite]“"
        );
        assert_eq!(open_brackets("a \\[ b"), 0, "eine maskierte Klammer öffnet nichts");
        assert_eq!(open_brackets("`[` und #f["), 1, "Rohtext zählt nicht, die offene Klammer danach schon");
        assert_eq!(Place::parse(None), Some(Place::End), "ohne Angabe am Ende");
        assert_eq!(Place::parse(Some("offset:-1")), None, "keine negative Stelle");
        assert_eq!(Place::parse(Some("mitte")), None, "unbekannte Stelle");
    }
}
