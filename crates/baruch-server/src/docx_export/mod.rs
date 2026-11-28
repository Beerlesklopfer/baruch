// SPDX-License-Identifier: AGPL-3.0-or-later
//! Word export (Round 32; interface R32-S1, owner docx_export).
//!
//! The work with the current revisions as .docx for the publisher ([`baruch_docx::write`]); on request the
//! open proposals lie on top as tracked changes ([`DocxExportOptions::proposals`]). The export changes nothing
//! in the work: it only reads (no commit, no entry in the mirror). No MCP tool in this round.
//!
//! - RPC: [`METHODS`] – `work.docx_export {work?, proposals? = false}`, see docs/RPC.md „Runde 32 – Word“.
//! - Pages: `GET /werke/{id}/word` (explanation and choice), `GET /werke/{id}/word.docx[?vorschlaege=1]` (file).
//! - CLI: `baruch-server export ID|SLUG --docx DATEI [--vorschlaege]` ([`cli_export`]).
//!
//! Structure of the file ([`build`]): front matter from the work details (imprint only with front matter switched
//! on, as in the EPUB), then per outline item in reading order the current revision through
//! [`baruch_docx::from_typst`]. Images: PNG, JPEG and GIF are embedded; Word does not embed other formats (SVG,
//! WebP) reliably – the figure then stays as a Typst island with warning `docx_bild_fehlt`. With proposals: per
//! section the newest open proposal through [`baruch_docx::diff::track_changes`], editor is the handle of the
//! persona; further open proposals of the same section are reported by the warning `docx_weitere_vorschlaege`.
//! Hidden revisions (marker „bis hierhin“, `baruch_core::revision_fold`) do not count.

mod build;

use std::sync::{Arc, OnceLock};

use axum::Router;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get;
use baruch_store::{StoreError, WorkId};
use minijinja::value::Serde;
use minijinja::{Environment, Value as Jinja, context};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::AppState;
use crate::i18n::tc;
use crate::rpc::{INVALID_PARAMS, METHOD_NOT_FOUND, RpcError};
use crate::view::Settings;

/// Methods of this module.
pub const METHODS: &[&str] = &["work.docx_export"];

/// Warning: a section has more than one open proposal; the file contains only the newest.
pub const WARN_MORE_PROPOSALS: &str = "docx_weitere_vorschlaege";

pub fn handles(method: &str) -> bool {
    METHODS.contains(&method)
}

/// Choice of the human on export.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DocxExportOptions {
    /// Include open proposals as tracked changes (per section the newest).
    pub proposals: bool,
}

/// Finished file.
#[derive(Debug, Clone, Default)]
pub struct DocxFile {
    /// `<slug>.docx`.
    pub file_name: String,
    pub bytes: Vec<u8>,
    /// Number of proposals that stand in the file as tracked changes.
    pub proposals: usize,
    pub warnings: Vec<baruch_docx::DocxWarning>,
}

/// Builds the file from the work; unknown work → -32602.
pub async fn export_file(state: &AppState, work: WorkId, options: &DocxExportOptions) -> Result<DocxFile, RpcError> {
    let slug = state.store.work_slug(work).await?;
    let built = build::document(state, work, options).await?;
    let write_options = baruch_docx::write::WriteOptions {
        created: built.modified,
        generator: format!("Baruch {}", env!("CARGO_PKG_VERSION")),
    };
    // Packing is pure computation: not on the runtime thread of the requests.
    let document = built.document;
    let bytes =
        tokio::task::spawn_blocking(crate::i18n::carry(move || baruch_docx::write::write(&document, &write_options)))
            .await
            .map_err(|e| {
                RpcError::new(crate::rpc::INTERNAL, tc("export_msg.docx.write", &[("error", e.to_string().into())]))
            })??;
    Ok(DocxFile { file_name: format!("{slug}.docx"), bytes, proposals: built.proposals, warnings: built.warnings })
}

fn warnings_json(warnings: &[baruch_docx::DocxWarning]) -> Value {
    warnings.iter().map(|w| json!({ "code": w.code, "message": w.message })).collect()
}

/// Work from `work`: number (id), text (folder name or id as digits) or – without a value – the default work.
async fn work_of(state: &AppState, params: &Value) -> Result<WorkId, RpcError> {
    let unknown = |e: StoreError| match e {
        StoreError::UnknownWork(_) => RpcError::new(INVALID_PARAMS, tc("export_msg.docx.unknown_work", &[])),
        other => other.into(),
    };
    match params.get("work") {
        Some(Value::String(text)) => match text.trim().parse::<WorkId>() {
            Ok(id) => state.store.work_slug(id).await.map(|_| id).map_err(unknown),
            Err(_) => state.store.work_by_slug(text.trim()).await.map_err(unknown),
        },
        _ => crate::rpc::work_param(state, params).await,
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExportParams {
    #[serde(default)]
    proposals: bool,
}

/// RPC call of a method from [`METHODS`].
pub async fn call(state: &AppState, method: &str, params: Value) -> Result<Value, RpcError> {
    if method != "work.docx_export" {
        return Err(RpcError::new(METHOD_NOT_FOUND, format!("unknown method {method}")));
    }
    let mut params = if params.is_null() { json!({}) } else { params };
    let work = work_of(state, &params).await?;
    if let Some(map) = params.as_object_mut() {
        map.remove("work");
    }
    let p: ExportParams = serde_json::from_value(params).map_err(|e| RpcError::new(INVALID_PARAMS, e.to_string()))?;
    let file = export_file(state, work, &DocxExportOptions { proposals: p.proposals }).await?;
    Ok(json!({
        "file_name": file.file_name,
        "data_base64": baruch_store::encode_base64(&file.bytes),
        "bytes": file.bytes.len(),
        "proposals": file.proposals,
        "warnings": warnings_json(&file.warnings),
    }))
}

/// `baruch-server export ID|SLUG --docx DATEI [--vorschlaege]`: `Ok(text)` → standard output, exit 0; `Err(text)` →
/// error output, exit 1. An existing file is never overwritten.
pub async fn cli_export(
    state: &AppState,
    work: WorkId,
    path: &std::path::Path,
    options: &DocxExportOptions,
) -> Result<String, String> {
    // The command line stays German (Round 39a, contract §5: CLI untouched).
    crate::i18n::scope(crate::i18n::Lang::De, cli_export_german(state, work, path, options)).await
}

async fn cli_export_german(
    state: &AppState,
    work: WorkId,
    path: &std::path::Path,
    options: &DocxExportOptions,
) -> Result<String, String> {
    // A dead link counts as existing too: overwrite nothing, create nothing behind a link.
    if std::fs::symlink_metadata(path).is_ok() {
        return Err(format!("Export: {} gibt es schon; nichts überschrieben", path.display()));
    }
    let file = export_file(state, work, options).await.map_err(|e| format!("Export: {}", e.message))?;
    let failed = |e: std::io::Error| format!("Export: {}: {e}", path.display());
    // `create_new` closes the gap between check and creation.
    let mut target = std::fs::OpenOptions::new().write(true).create_new(true).open(path).map_err(failed)?;
    std::io::Write::write_all(&mut target, &file.bytes).map_err(failed)?;
    let mut text = format!("Word-Datei geschrieben: {} ({} Bytes)", path.display(), file.bytes.len());
    if options.proposals {
        text.push_str(&format!("\nVorschläge als Änderungsverfolgung: {}", file.proposals));
    }
    for warning in &file.warnings {
        text.push_str(&format!("\nHinweis ({}): {}", warning.code, warning.message));
    }
    Ok(text)
}

fn templates() -> &'static Environment<'static> {
    static ENV: OnceLock<Environment<'static>> = OnceLock::new();
    ENV.get_or_init(|| {
        let mut env = Environment::new();
        crate::i18n::install(&mut env);
        for (name, source) in [
            ("base.html", include_str!("../../templates/base.html")),
            ("docx_export.html", include_str!("../../templates/docx_export.html")),
        ] {
            env.add_template(name, source).expect("Vorlage des Word-Exports fehlerhaft");
        }
        env
    })
}

/// Answer for a work that does not exist (404), or another error of the store (500).
fn store_failure(e: StoreError) -> Response {
    match e {
        StoreError::UnknownWork(_) => (StatusCode::NOT_FOUND, tc("export_msg.page.unknown_work", &[])).into_response(),
        e => (StatusCode::INTERNAL_SERVER_ERROR, tc("export_msg.page.error", &[("error", e.to_string().into())]))
            .into_response(),
    }
}

/// Page with explanation and choice; the form sends by GET to the file (works without JavaScript).
async fn page(State(state): State<Arc<AppState>>, headers: HeaderMap, Path(id): Path<WorkId>) -> Response {
    let work = match state.store.work_meta(id).await {
        Ok(work) => work,
        Err(e) => return store_failure(e),
    };
    let open = match build::open_proposals(&state, id).await {
        Ok(open) => open,
        Err(e) => return store_failure(e),
    };
    let settings = Settings::from_cookie_header(headers.get(header::COOKIE).and_then(|v| v.to_str().ok()));
    let ctx = context! {
        settings => Jinja::from(Serde(settings)),
        work => context! { id => work.id, title => work.title, slug => work.slug },
        sections_with_proposals => open.sections,
        open_proposals => open.total,
    };
    match templates().get_template("docx_export.html").and_then(|t| t.render(ctx)) {
        Ok(html) => Html(html).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            tc(
                "export_msg.typeset.template_error",
                &[("name", "docx_export.html".into()), ("error", e.to_string().into())],
            ),
        )
            .into_response(),
    }
}

#[derive(Deserialize)]
struct DownloadQuery {
    /// `1` = open proposals as tracked changes; missing or `0` = without.
    vorschlaege: Option<String>,
}

/// The file itself.
async fn download(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<WorkId>,
    Query(query): Query<DownloadQuery>,
) -> Response {
    // Like PDF and EPUB: the file is not created on request of a foreign page (in addition to `guard::own_reads`).
    if crate::guard::cross_site_get(&headers) {
        return (StatusCode::FORBIDDEN, tc("export_msg.docx.foreign", &[])).into_response();
    }
    let proposals = match query.vorschlaege.as_deref().map(str::trim) {
        None | Some("" | "0") => false,
        Some("1") => true,
        Some(_) => return (StatusCode::BAD_REQUEST, tc("export_msg.docx.proposals_flag", &[])).into_response(),
    };
    if let Err(e) = state.store.work_slug(id).await {
        return store_failure(e);
    }
    match export_file(&state, id, &DocxExportOptions { proposals }).await {
        Ok(file) => {
            let disposition = HeaderValue::from_str(&format!("attachment; filename=\"{}\"", file.file_name))
                .unwrap_or_else(|_| HeaderValue::from_static("attachment"));
            (
                [
                    (header::CONTENT_TYPE, HeaderValue::from_static(baruch_docx::ooxml::MEDIA_TYPE_DOCX)),
                    (header::CONTENT_DISPOSITION, disposition),
                    (header::X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff")),
                    (header::CACHE_CONTROL, HeaderValue::from_static("no-cache")),
                ],
                file.bytes,
            )
                .into_response()
        }
        Err(e) if e.code == INVALID_PARAMS => (StatusCode::UNPROCESSABLE_ENTITY, e.message).into_response(),
        Err(e) => {
            (StatusCode::INTERNAL_SERVER_ERROR, tc("export_msg.page.error", &[("error", e.message.as_str().into())]))
                .into_response()
        }
    }
}

/// Pages (GET), merged in `lib.rs` with the pages: `GET /werke/{id}/word` and `GET /werke/{id}/word.docx`.
pub fn pages() -> Router<Arc<AppState>> {
    Router::new().route("/werke/{id}/word", get(page)).route("/werke/{id}/word.docx", get(download))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_export_method_belongs_to_this_module() {
        assert!(handles("work.docx_export"), "die eine Methode des Exports");
        assert!(!handles("docx_import.preview"), "der Import gehört docx_import");
        assert!(!handles("work.export"), "der Typst-Export bleibt bei works");
        assert!(!DocxExportOptions::default().proposals, "Vorgabe: ohne Vorschläge (Vertrag: proposals? = false)");
    }
}
