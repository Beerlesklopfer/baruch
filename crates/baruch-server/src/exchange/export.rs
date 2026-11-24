// SPDX-License-Identifier: AGPL-3.0-or-later
//! Austausch: Ausgeben (Runde 26; Doku `docs/Austauschformat.md`).
//!
//! - `GET /werke/{id}/austausch`: Seite mit Erklärung, Umfang, zwei Download-Links und Link zu „Einlesen“
//!   (`templates/exchange.html`, ohne JavaScript).
//! - `GET /werke/{id}/austausch.baruch`: ZIP (`application/vnd.baruch.werk+zip`), Anhang `<slug>.baruch`.
//! - `GET /werke/{id}/austausch.xml`: nur XML (`application/vnd.baruch.werk+xml`), Anhang `<slug>.baruch.xml`.
//! - RPC `work.exchange_export {work?, format?}` und CLI `export ID|SLUG --baruch DATEI`; MCP
//!   `exchange_export` in [`crate::mcp::exchange`] nutzt [`export_file`].
//!
//! Alles nur lesend: Grundlage ist [`baruch_store::Store::exchange_dump`] (eine lesende Transaktion), dazu
//! Instanz, Zeitpunkt und Programm. Kein Geheimnis, keine Verbindung, keine Persona-Modelle (nur Handles als
//! Herkunft) und keine Anträge stehen in der Datei.

use std::sync::{Arc, OnceLock};

use axum::Router;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get;
use baruch_exchange::write::{ImageMode, write_baruch, write_xml};
use baruch_exchange::{ExchangeDocument, ExchangeInstance, ExchangeNode, FORMAT_VERSION};
use baruch_store::WorkId;
use minijinja::Environment;
use minijinja::value::{Serde, Value as JinjaValue};
use serde_json::{Value, json};

use crate::AppState;
use crate::rpc::{INVALID_PARAMS, RpcError};
use crate::view::Settings;

/// Routen der Seite und der Downloads (gemischt über `exchange::pages`).
pub fn pages() -> Router<Arc<AppState>> {
    Router::new()
        .route("/werke/{id}/austausch", get(exchange_page))
        .route("/werke/{id}/austausch.baruch", get(download_baruch))
        .route("/werke/{id}/austausch.xml", get(download_xml))
}

fn templates() -> &'static Environment<'static> {
    static ENV: OnceLock<Environment<'static>> = OnceLock::new();
    ENV.get_or_init(|| {
        let mut env = Environment::new();
        for (name, source) in [
            ("base.html", include_str!("../../templates/base.html")),
            ("exchange.html", include_str!("../../templates/exchange.html")),
        ] {
            env.add_template(name, source).expect("Vorlage des Austauschs fehlerhaft");
        }
        env
    })
}

fn error_page(status: StatusCode, message: &str) -> Response {
    (
        status,
        Html(format!(
            "<!doctype html><meta charset=\"utf-8\"><title>Austausch</title><p>{}</p>",
            crate::bibles::html_escape(message)
        )),
    )
        .into_response()
}

/// Form der Austauschdatei.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// Container `.baruch` (ZIP) mit Bildern als eigene Dateien.
    Baruch,
    /// Nur XML `.baruch.xml`, Bilder als Base64.
    Xml,
}

impl Format {
    /// Wort in RPC und MCP (`"baruch"` bzw. `"xml"`); unbekannt → `None`.
    pub fn parse(word: &str) -> Option<Format> {
        match word {
            "baruch" => Some(Format::Baruch),
            "xml" => Some(Format::Xml),
            _ => None,
        }
    }

    pub fn word(self) -> &'static str {
        match self {
            Format::Baruch => "baruch",
            Format::Xml => "xml",
        }
    }

    pub fn media_type(self) -> &'static str {
        match self {
            Format::Baruch => baruch_exchange::ZIP_MIMETYPE,
            Format::Xml => baruch_exchange::XML_MEDIA_TYPE,
        }
    }

    pub fn suffix(self) -> &'static str {
        match self {
            Format::Baruch => baruch_exchange::ZIP_SUFFIX,
            Format::Xml => baruch_exchange::XML_SUFFIX,
        }
    }
}

/// Eine fertige Austauschdatei.
#[derive(Debug, Clone)]
pub struct ExportFile {
    pub work: WorkId,
    /// UUID des Werks.
    pub uuid: String,
    pub title: String,
    /// `<slug>.baruch` bzw. `<slug>.baruch.xml`.
    pub file_name: String,
    pub format: Format,
    pub bytes: Vec<u8>,
    /// SHA-256 der Datei (Kleinbuchstaben-Hex).
    pub sha256: String,
}

/// Zeitpunkt der Ausgabe im Format der Datenbank (`JJJJ-MM-TTTHH:MM:SS.sssZ`, UTC).
fn now() -> String {
    chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()
}

/// Das Werk als Austauschdokument (mit Instanz, Zeitpunkt und Programm).
pub async fn document(state: &AppState, work: WorkId) -> Result<ExchangeDocument, RpcError> {
    let dumped = state.store.exchange_dump(work).await?;
    let instance = state.store.instance().await?;
    Ok(ExchangeDocument {
        version: FORMAT_VERSION,
        exported: now(),
        instance: ExchangeInstance { uuid: instance.uuid, name: instance.name },
        generator: format!("baruch {}", env!("CARGO_PKG_VERSION")),
        work: dumped,
        unknown: Vec::new(),
    })
}

/// Werk als Datei in der gewünschten Form. Unbekanntes Werk → `INVALID_PARAMS` (aus `UnknownWork`).
pub async fn export_file(state: &AppState, work: WorkId, format: Format) -> Result<ExportFile, RpcError> {
    let slug = state.store.work_slug(work).await?;
    let doc = document(state, work).await?;
    let bytes = match format {
        Format::Baruch => write_baruch(&doc)?,
        Format::Xml => write_xml(&doc, ImageMode::Inline)?.into_bytes(),
    };
    Ok(ExportFile {
        work,
        uuid: doc.work.uuid.clone(),
        title: doc.work.title.clone(),
        file_name: format!("{slug}{}", format.suffix()),
        format,
        sha256: baruch_exchange::sha256_hex(&bytes),
        bytes,
    })
}

/// Umfang des Werks für die Seite: Punkte, Fassungen, Beiträge, Einfälle, Fäden, Bilder, Bildfassungen, die
/// Gedankenkarte (Verbindungen, davon gelöscht, Lagen; Runde 25), seit Runde 28 Befunde (davon offen) und alte
/// Fassungen bearbeiteter Beiträge, seit Runde 29 `typeset` (Satz-Einstellungen vorhanden) und `typeset_fields`
/// (Bezeichnungen der gesetzten Felder), seit Runde 31 `glossary_terms` (davon `proposed_terms` vorgeschlagen) und
/// `registers` (eigene Register).
fn counts(doc: &ExchangeDocument) -> Value {
    fn walk(nodes: &[ExchangeNode], n: &mut [usize; 4]) {
        for node in nodes {
            n[0] += 1;
            n[1] += node.revisions.len();
            n[2] += node.posts.len();
            n[3] += node.posts.iter().map(|p| p.versions.len()).sum::<usize>();
            walk(&node.children, n);
        }
    }
    let mut n = [0usize; 4];
    walk(&doc.work.outline, &mut n);
    let work = &doc.work;
    json!({
        "nodes": n[0],
        "revisions": n[1],
        "posts": n[2],
        "template_revisions": work.template.revisions.len(),
        "ideas": work.ideas.len(),
        "deleted_ideas": work.ideas.iter().filter(|i| i.deleted.is_some()).count(),
        "threads": work.threads.len(),
        "thread_posts": work.threads.iter().map(|t| t.posts.len()).sum::<usize>(),
        "assets": work.assets.len(),
        "asset_revisions": work.assets.iter().map(|a| a.revisions.len()).sum::<usize>(),
        "tombstones": work.deleted.len(),
        "links": work.map.links.len(),
        "deleted_links": work.map.links.iter().filter(|l| l.deleted.is_some()).count(),
        "positions": work.map.positions.len(),
        "findings": work.findings.len(),
        "open_findings": work
            .findings
            .iter()
            .filter(|f| {
                // Zählregel 1.5 (Runde 28): offen, nicht angehängt, kein Lob, mit Abschnitt.
                f.status == baruch_exchange::ExchangeFindingStatus::Open
                    && f.similar_to.is_none()
                    && f.kind != baruch_exchange::ExchangeFindingKind::Praise
                    && f.node.is_some()
            })
            .count(),
        "post_versions": n[3]
            + work.threads.iter().flat_map(|t| &t.posts).map(|p| p.versions.len()).sum::<usize>(),
        "typeset": work.typeset.is_some(),
        "typeset_fields": work
            .typeset
            .as_ref()
            .map(|t| baruch_typst::typeset::TypesetSettings::default().changed_fields(&t.settings))
            .unwrap_or_default()
            .into_iter()
            .map(baruch_typst::typeset::field_label)
            .collect::<Vec<_>>(),
        "glossary_terms": work.glossary.len(),
        "proposed_terms": work
            .glossary
            .iter()
            .filter(|t| t.status == baruch_exchange::ExchangeGlossaryStatus::Proposed)
            .count(),
        "registers": work.registers.len(),
    })
}

/// Anzeigename einer Instanz: Name oder „ohne Namen (<erste 8 Zeichen>)“ (Vertrag F3).
pub fn instance_label(uuid: &str, name: &str) -> String {
    if name.trim().is_empty() { format!("ohne Namen ({})", uuid.get(..8).unwrap_or(uuid)) } else { name.to_owned() }
}

/// `GET /werke/{id}/austausch`.
async fn exchange_page(State(state): State<Arc<AppState>>, headers: HeaderMap, Path(id): Path<WorkId>) -> Response {
    let (slug, title) = match state.store.works().await {
        Ok(list) => match list.into_iter().find(|(w, _, _)| *w == id) {
            Some((_, slug, title)) => (slug, title),
            None => return error_page(StatusCode::NOT_FOUND, "Unbekanntes Werk."),
        },
        Err(e) => return error_page(StatusCode::INTERNAL_SERVER_ERROR, &format!("Fehler: {e}")),
    };
    let doc = match document(&state, id).await {
        Ok(doc) => doc,
        Err(e) => return error_page(StatusCode::INTERNAL_SERVER_ERROR, &format!("Fehler: {}", e.message)),
    };
    let settings = Settings::from_cookie_header(headers.get(header::COOKIE).and_then(|v| v.to_str().ok()));
    let ctx = minijinja::context! {
        settings => JinjaValue::from(Serde(settings)),
        work => minijinja::context! { id => id, title => title, slug => slug.clone(), uuid => doc.work.uuid.clone() },
        instance => minijinja::context! {
            uuid => doc.instance.uuid.clone(),
            label => instance_label(&doc.instance.uuid, &doc.instance.name),
            named => !doc.instance.name.trim().is_empty(),
        },
        counts => JinjaValue::from(Serde(counts(&doc))),
        back => format!("/?work={id}"),
        baruch_name => format!("{slug}{}", baruch_exchange::ZIP_SUFFIX),
        xml_name => format!("{slug}{}", baruch_exchange::XML_SUFFIX),
        version => FORMAT_VERSION,
        namespace => baruch_exchange::NAMESPACE,
    };
    match templates().get_template("exchange.html").and_then(|t| t.render(ctx)) {
        Ok(html) => Html(html).into_response(),
        Err(e) => error_page(StatusCode::INTERNAL_SERVER_ERROR, &format!("Vorlage exchange.html: {e}")),
    }
}

/// Download als Anhang; unbekanntes Werk → 404.
async fn download(state: &AppState, id: WorkId, format: Format) -> Response {
    match state.store.works().await {
        Ok(list) if list.iter().any(|(w, _, _)| *w == id) => {}
        Ok(_) => return error_page(StatusCode::NOT_FOUND, "Unbekanntes Werk."),
        Err(e) => return error_page(StatusCode::INTERNAL_SERVER_ERROR, &format!("Fehler: {e}")),
    }
    let file = match export_file(state, id, format).await {
        Ok(file) => file,
        Err(e) => return error_page(StatusCode::INTERNAL_SERVER_ERROR, &format!("Austausch: {}", e.message)),
    };
    let disposition = format!("attachment; filename=\"{}\"", file.file_name.replace(['"', '\\'], "_"));
    let mut response = file.bytes.into_response();
    let headers = response.headers_mut();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static(format.media_type()));
    match HeaderValue::from_str(&disposition) {
        Ok(value) => headers.insert(header::CONTENT_DISPOSITION, value),
        Err(_) => headers.insert(header::CONTENT_DISPOSITION, HeaderValue::from_static("attachment")),
    };
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

/// `GET /werke/{id}/austausch.baruch`.
async fn download_baruch(State(state): State<Arc<AppState>>, Path(id): Path<WorkId>) -> Response {
    download(&state, id, Format::Baruch).await
}

/// `GET /werke/{id}/austausch.xml`.
async fn download_xml(State(state): State<Arc<AppState>>, Path(id): Path<WorkId>) -> Response {
    download(&state, id, Format::Xml).await
}

/// RPC `work.exchange_export {work?, format?: "baruch"|"xml"}` → `{work, uuid, file_name, media_type, bytes,
/// sha256, data?: Base64 (baruch), xml?: Text (xml)}`.
pub async fn rpc_export(state: &AppState, params: serde_json::Value) -> Result<serde_json::Value, RpcError> {
    let work = crate::rpc::work_param(state, &params).await?;
    let format = match params.get("format") {
        None | Some(Value::Null) => Format::Baruch,
        Some(Value::String(word)) => Format::parse(word).ok_or_else(|| {
            RpcError::new(INVALID_PARAMS, format!("format: „baruch“ oder „xml“ erwartet, nicht „{word}“"))
        })?,
        Some(_) => return Err(RpcError::new(INVALID_PARAMS, "format: „baruch“ oder „xml“ erwartet")),
    };
    let file = export_file(state, work, format).await?;
    Ok(answer(file))
}

/// Antwort von RPC und MCP für eine Datei.
pub fn answer(file: ExportFile) -> Value {
    let mut answer = json!({
        "work": file.work,
        "uuid": file.uuid,
        "format": file.format.word(),
        "file_name": file.file_name,
        "media_type": file.format.media_type(),
        "bytes": file.bytes.len(),
        "sha256": file.sha256,
    });
    match file.format {
        Format::Baruch => answer["data"] = json!(baruch_store::encode_base64(&file.bytes)),
        Format::Xml => answer["xml"] = json!(String::from_utf8(file.bytes).unwrap_or_default()),
    }
    answer
}

/// CLI `export ID|SLUG --baruch DATEI`: endet DATEI auf `.xml`, nur XML, sonst ZIP; vorhandene Datei → Fehler.
pub async fn cli_export(state: &AppState, work: WorkId, path: &std::path::Path) -> Result<String, String> {
    use std::io::Write as _;
    let xml = path.extension().is_some_and(|e| e.eq_ignore_ascii_case("xml"));
    let format = if xml { Format::Xml } else { Format::Baruch };
    if path.exists() || path.symlink_metadata().is_ok() {
        return Err(format!("{} gibt es schon; nichts überschrieben.", path.display()));
    }
    let file = export_file(state, work, format).await.map_err(|e| e.message)?;
    // create_new: Entsteht die Datei zwischen Prüfen und Schreiben doch, wird sie nicht überschrieben.
    let mut out = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    if let Err(e) = out.write_all(&file.bytes).and_then(|()| out.sync_all()) {
        let _ = std::fs::remove_file(path);
        return Err(format!("{}: {e}", path.display()));
    }
    let what = match format {
        Format::Baruch => "Austauschdatei (ZIP)",
        Format::Xml => "Austauschdatei (nur XML)",
    };
    Ok(format!(
        "{what} geschrieben: {}\nWerk {} „{}“, uuid {}\n{} Bytes, sha256 {}",
        path.display(),
        file.work,
        file.title,
        file.uuid,
        file.bytes.len(),
        file.sha256
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_have_words_types_and_suffixes() {
        // RPC und MCP nehmen genau die Wörter des Vertrags; unbekannte lehnt rpc_export ab.
        assert_eq!(Format::parse("baruch"), Some(Format::Baruch), "baruch");
        assert_eq!(Format::parse("xml"), Some(Format::Xml), "xml");
        assert_eq!(Format::parse("zip"), None, "zip ist kein Wort des Vertrags");
        assert_eq!(Format::Baruch.media_type(), "application/vnd.baruch.werk+zip", "Medientyp ZIP");
        assert_eq!(Format::Xml.suffix(), ".baruch.xml", "Endung nur XML");
    }

    #[test]
    fn instance_without_name_shows_the_uuid_prefix() {
        // Vertrag F3: „ohne Namen (<erste 8 Zeichen>)“.
        assert_eq!(instance_label("0f8c2a4e-1b3d-4c5e-9f60-718293a4b5c6", ""), "ohne Namen (0f8c2a4e)", "ohne Namen");
        assert_eq!(instance_label("0f8c2a4e-1b3d-4c5e-9f60-718293a4b5c6", "Laptop"), "Laptop", "mit Namen");
    }
}
