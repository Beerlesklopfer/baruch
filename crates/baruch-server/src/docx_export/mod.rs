// SPDX-License-Identifier: AGPL-3.0-or-later
//! Word-Export (Runde 32; Schnittstelle R32-S1, Besitz docx_export).
//!
//! Das Werk mit den aktuellen Fassungen als .docx für den Verlag ([`baruch_docx::write`]); auf Wunsch liegen die
//! offenen Vorschläge als Änderungsverfolgung darüber ([`DocxExportOptions::proposals`]). Der Export ändert nichts
//! am Werk: Er liest nur (kein Commit, kein Eintrag im Spiegel). Kein MCP-Werkzeug in dieser Runde.
//!
//! - RPC: [`METHODS`] – `work.docx_export {work?, proposals? = false}`, siehe docs/RPC.md „Runde 32 – Word“.
//! - Seiten: `GET /werke/{id}/word` (Erklärung und Wahl), `GET /werke/{id}/word.docx[?vorschlaege=1]` (Datei).
//! - CLI: `baruch-server export ID|SLUG --docx DATEI [--vorschlaege]` ([`cli_export`]).
//!
//! Aufbau der Datei ([`build`]): Titelei aus den Werkangaben (Impressum nur mit eingeschalteter Titelei, wie im
//! EPUB), dann je Gliederungspunkt in Lesereihenfolge die aktuelle Fassung über [`baruch_docx::from_typst`]. Bilder:
//! PNG, JPEG und GIF werden eingebettet; andere Formate (SVG, WebP) bettet Word nicht verlässlich ein – die Abbildung
//! bleibt dann als Typst-Insel mit Warnung `docx_bild_fehlt` stehen. Mit Vorschlägen: je Abschnitt der jüngste offene
//! Vorschlag über [`baruch_docx::diff::track_changes`], Bearbeiter ist das Handle der Persona; weitere offene
//! Vorschläge desselben Abschnitts meldet die Warnung `docx_weitere_vorschlaege`. Ausgeblendete Fassungen (Marke „bis
//! hierhin“, `baruch_core::revision_fold`) zählen nicht.

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
use crate::rpc::{INVALID_PARAMS, METHOD_NOT_FOUND, RpcError};
use crate::view::Settings;

/// Methoden dieses Moduls.
pub const METHODS: &[&str] = &["work.docx_export"];

/// Warnung: Ein Abschnitt hat mehr als einen offenen Vorschlag; in der Datei steht nur der jüngste.
pub const WARN_MORE_PROPOSALS: &str = "docx_weitere_vorschlaege";

pub fn handles(method: &str) -> bool {
    METHODS.contains(&method)
}

/// Wahl des Menschen beim Export.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DocxExportOptions {
    /// Offene Vorschläge als Änderungsverfolgung mitgeben (je Abschnitt der jüngste).
    pub proposals: bool,
}

/// Fertige Datei.
#[derive(Debug, Clone, Default)]
pub struct DocxFile {
    /// `<slug>.docx`.
    pub file_name: String,
    pub bytes: Vec<u8>,
    /// Zahl der Vorschläge, die als Änderungsverfolgung in der Datei stehen.
    pub proposals: usize,
    pub warnings: Vec<baruch_docx::DocxWarning>,
}

/// Baut die Datei aus dem Werk; unbekanntes Werk → -32602.
pub async fn export_file(state: &AppState, work: WorkId, options: &DocxExportOptions) -> Result<DocxFile, RpcError> {
    let slug = state.store.work_slug(work).await?;
    let built = build::document(state, work, options).await?;
    let write_options = baruch_docx::write::WriteOptions {
        created: built.modified,
        generator: format!("Baruch {}", env!("CARGO_PKG_VERSION")),
    };
    // Das Packen ist reine Rechenarbeit: nicht auf dem Laufzeit-Faden der Anfragen.
    let document = built.document;
    let bytes = tokio::task::spawn_blocking(move || baruch_docx::write::write(&document, &write_options))
        .await
        .map_err(|e| RpcError::new(crate::rpc::INTERNAL, format!("Word-Datei schreiben: {e}")))??;
    Ok(DocxFile { file_name: format!("{slug}.docx"), bytes, proposals: built.proposals, warnings: built.warnings })
}

fn warnings_json(warnings: &[baruch_docx::DocxWarning]) -> Value {
    warnings.iter().map(|w| json!({ "code": w.code, "message": w.message })).collect()
}

/// Werk aus `work`: Zahl (id), Text (Ordnername oder id als Ziffern) oder – ohne Angabe – das Vorgabewerk.
async fn work_of(state: &AppState, params: &Value) -> Result<WorkId, RpcError> {
    let unknown = |e: StoreError| match e {
        StoreError::UnknownWork(_) => RpcError::new(INVALID_PARAMS, "work: unbekanntes Werk"),
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

/// RPC-Aufruf einer Methode aus [`METHODS`].
pub async fn call(state: &AppState, method: &str, params: Value) -> Result<Value, RpcError> {
    if method != "work.docx_export" {
        return Err(RpcError::new(METHOD_NOT_FOUND, format!("unbekannte Methode {method}")));
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

/// `baruch-server export ID|SLUG --docx DATEI [--vorschlaege]`: `Ok(text)` → Standardausgabe, Exit 0; `Err(text)` →
/// Fehlerausgabe, Exit 1. Eine vorhandene Datei wird nie überschrieben.
pub async fn cli_export(
    state: &AppState,
    work: WorkId,
    path: &std::path::Path,
    options: &DocxExportOptions,
) -> Result<String, String> {
    // Auch ein toter Verweis zählt als vorhanden: nichts überschreiben, nichts hinter einem Verweis anlegen.
    if std::fs::symlink_metadata(path).is_ok() {
        return Err(format!("Export: {} gibt es schon; nichts überschrieben", path.display()));
    }
    let file = export_file(state, work, options).await.map_err(|e| format!("Export: {}", e.message))?;
    let failed = |e: std::io::Error| format!("Export: {}: {e}", path.display());
    // `create_new` schließt die Lücke zwischen Prüfung und Anlegen.
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
        for (name, source) in [
            ("base.html", include_str!("../../templates/base.html")),
            ("docx_export.html", include_str!("../../templates/docx_export.html")),
        ] {
            env.add_template(name, source).expect("Vorlage des Word-Exports fehlerhaft");
        }
        env
    })
}

/// Antwort für ein Werk, das es nicht gibt (404), oder einen anderen Fehler des Speichers (500).
fn store_failure(e: StoreError) -> Response {
    match e {
        StoreError::UnknownWork(_) => (StatusCode::NOT_FOUND, "unbekanntes Werk").into_response(),
        e => (StatusCode::INTERNAL_SERVER_ERROR, format!("Fehler: {e}")).into_response(),
    }
}

/// Seite mit Erklärung und Wahl; das Formular schickt per GET an die Datei (geht ohne JavaScript).
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
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, format!("Fehler: Vorlage docx_export.html: {e}")).into_response(),
    }
}

#[derive(Deserialize)]
struct DownloadQuery {
    /// `1` = offene Vorschläge als Änderungsverfolgung; fehlt oder `0` = ohne.
    vorschlaege: Option<String>,
}

/// Die Datei selbst.
async fn download(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<WorkId>,
    Query(query): Query<DownloadQuery>,
) -> Response {
    // Wie PDF und EPUB: Die Datei entsteht nicht auf Anfrage einer fremden Seite (zusätzlich zu `guard::own_reads`).
    if crate::guard::cross_site_get(&headers) {
        return (StatusCode::FORBIDDEN, "Abgelehnt: Die Word-Datei entsteht nicht auf Anfrage einer fremden Seite.")
            .into_response();
    }
    let proposals = match query.vorschlaege.as_deref().map(str::trim) {
        None | Some("" | "0") => false,
        Some("1") => true,
        Some(_) => return (StatusCode::BAD_REQUEST, "vorschlaege: 1 oder 0").into_response(),
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
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, format!("Fehler: {}", e.message)).into_response(),
    }
}

/// Seiten (GET), gemischt in `lib.rs` bei den Seiten: `GET /werke/{id}/word` und `GET /werke/{id}/word.docx`.
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
