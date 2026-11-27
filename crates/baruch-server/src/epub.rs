// SPDX-License-Identifier: AGPL-3.0-or-later
//! EPUB eines Werks (Runde 20a, Besitz „epub“): Eingabe sammeln ([`epub_input`]), bauen ([`book_for`]), packen
//! ([`epub_bytes`]), als Datei schreiben ([`write_epub_file`], CLI `export --epub`).
//!
//! - **RPC** ([`handles`], [`call`]): `work.epub {work, inline?, footnotes?}` → `{file_name, bytes, sha256, url,
//!   footnotes, files, warnings, epub_base64?}`; kein Commit; ohne Abschnitte -32000.
//! - **Seite** ([`pages`]): `GET /werke/{id}/epub[?fussnoten=popup|endnoten]` → `application/epub+zip` als Anhang
//!   `<slug>.epub`; Anfrage einer fremden Seite → 403, ohne Abschnitte → 422.
//! - **Entwurf (Runde 22):** `work.epub {draft?}` und `?entwurf=1|0` (dreiwertig, ändert die Einstellung nie); mit
//!   Entwurf trägt die Titelseite den Hinweis und `dc:title` „<Titel> (Entwurf)“ ([`book_for_draft`]).
//!
//! Die Eingabe kommt aus denselben Quellen wie Export und Satz: aktuelle Fassungen und Werkvorlage
//! (`Store::export_map`), Nummern aus dem Gliederungsschema (`Store::numbering_preview`), Bilder
//! (`Store::export_assets`, Alternativtexte aus den Bildangaben), Umschlag und Buchangaben (`Store::book_meta`,
//! [`crate::cover::book_info`] → ISBN geprüft mit `isbn13_check`, Impressum aus `imprint_lines`, Rechte aus
//! `copyright_line`), Satz-Einstellungen ([`crate::cover::typeset_settings`]). Sprache: Satz-Einstellung, sonst
//! `#set text(lang: …)` der Werkvorlage, sonst „de“. `dcterms:modified` ist der jüngste Zeitpunkt aus Werk,
//! Buchangaben, Satz-Einstellungen, Bildern und Fassungen – gleicher Stand, gleiche Bytes.
//!
//! **EPUB-Cover (Runde 21c, [`epub_cover`]):** Das Cover ist ein Rasterbild (PNG, höchstens [`COVER_MAX_WIDTH`] ×
//! [`COVER_MAX_HEIGHT`] Pixel, Seitenverhältnis der Vorderseite) der Umschlag-Vorderseite samt Titel, Untertitel und
//! Autor aus der Umschlag-Anordnung ([`crate::cover::front_cover_source`], gerastert mit
//! [`baruch_render::render_png`]) – auch ohne Umschlagbild (dann auf der dunklen Fläche). Scheitert das Rastern, nimmt
//! das EPUB die rohe Bilddatei des Umschlags (falls es eine gibt) und meldet `COVER_RASTER_FAILED` in `warnings`;
//! nie ein stiller Wegfall. Gleicher Stand → gleiche Bytes (Zwischenspeicher je Stand der Quelle).

use std::collections::BTreeMap;
use std::sync::Arc;

use axum::Router;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use baruch_epub::{EpubBook, EpubError, EpubImage, EpubInput, EpubMeta, EpubSection, EpubWarning, FootnoteMode};
use baruch_store::{StoreError, WorkId};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::AppState;
use crate::rpc::{INTERNAL, INVALID_PARAMS, METHOD_NOT_FOUND, REFUSED, RpcError};

/// Größte Breite des EPUB-Covers in Pixeln (übliche Cover-Größe 1600 × 2560).
pub const COVER_MAX_WIDTH: u32 = 1600;
/// Größte Höhe des EPUB-Covers in Pixeln.
pub const COVER_MAX_HEIGHT: u32 = 2560;
/// Code der Warnung, wenn die Vorderseite nicht gerastert werden konnte.
pub const COVER_RASTER_FAILED: &str = "COVER_RASTER_FAILED";

/// Methoden dieses Moduls.
pub fn handles(method: &str) -> bool {
    method == "work.epub"
}

#[derive(Deserialize)]
struct EpubParams {
    #[serde(default)]
    inline: bool,
    #[serde(default)]
    footnotes: FootnoteMode,
    /// Runde 22: Entwurf dreiwertig (fehlt = nach Satz-Einstellung).
    #[serde(default)]
    draft: Option<bool>,
}

fn epub_error(e: EpubError) -> RpcError {
    match e {
        EpubError::NoSections => RpcError::new(REFUSED, e.to_string()),
        EpubError::Invalid(_) | EpubError::Zip(_) => RpcError::new(INTERNAL, e.to_string()),
    }
}

/// Wort der Fußnotenart in Antworten (`popup` | `endnotes`).
pub fn footnote_word(mode: FootnoteMode) -> &'static str {
    match mode {
        FootnoteMode::Popup => "popup",
        FootnoteMode::Endnotes => "endnotes",
    }
}

/// Dateien in Lesereihenfolge für Antworten: `{href, media_type, kind, title, key}`.
pub fn spine_files(book: &EpubBook) -> Vec<Value> {
    book.spine
        .iter()
        .filter_map(|id| book.items.iter().find(|i| &i.id == id))
        .map(|i| json!({ "href": i.href, "media_type": i.media_type, "kind": i.kind, "title": i.title, "key": i.key }))
        .collect()
}

pub async fn call(state: &AppState, work: WorkId, method: &str, params: Value) -> Result<Value, RpcError> {
    if method != "work.epub" {
        return Err(RpcError::new(METHOD_NOT_FOUND, format!("unknown method {method}")));
    }
    let mut params = if params.is_null() { json!({}) } else { params };
    if let Some(map) = params.as_object_mut() {
        map.remove("work");
    }
    let p: EpubParams = serde_json::from_value(params).map_err(|e| RpcError::new(INVALID_PARAMS, e.to_string()))?;
    let book = book_for_draft(state, work, p.footnotes, p.draft).await?;
    let slug = state.store.work_slug(work).await?;
    let bytes = baruch_epub::package(&book).map_err(epub_error)?;
    let pair = crate::typeset::draft_pair(p.draft);
    let mut answer = json!({
        "file_name": format!("{slug}.epub"),
        "bytes": bytes.len(),
        "sha256": baruch_exchange::sha256_hex(&bytes),
        "url": if pair.is_empty() { format!("/werke/{work}/epub") } else { format!("/werke/{work}/epub?{pair}") },
        "footnotes": footnote_word(p.footnotes),
        "files": spine_files(&book),
        "warnings": book.warnings,
    });
    if p.inline {
        answer["epub_base64"] = json!(baruch_store::encode_base64(&bytes));
    }
    Ok(answer)
}

/// Zeitpunkt der Datenbank (`JJJJ-MM-TTThh:mm:ss[.fff][Z]` oder mit Leerzeichen statt `T`) in der Form von
/// `dcterms:modified`; unlesbar → `None`.
fn normalize_stamp(stamp: &str) -> Option<String> {
    let head: String = stamp.trim().chars().take(19).collect();
    let well_formed = head.len() == 19
        && head.char_indices().all(|(i, c)| match i {
            4 | 7 => c == '-',
            10 => c == 'T' || c == ' ',
            13 | 16 => c == ':',
            _ => c.is_ascii_digit(),
        });
    well_formed.then(|| format!("{}T{}Z", &head[..10], &head[11..]))
}

/// Jüngster Zeitpunkt des Stands: Werk, Buchangaben, Satz-Einstellungen, Bilder, Fassungen.
async fn modified(state: &AppState, work: WorkId, w: &baruch_store::WorkMeta) -> Result<String, StoreError> {
    let mut stamps: Vec<String> = vec![w.created.clone()];
    stamps.extend(w.updated.clone());
    stamps.extend(state.store.book_meta(work).await?.updated);
    stamps.extend(state.store.work_typeset(work).await?.map(|t| t.updated));
    stamps.extend(state.store.asset_list(work).await?.into_iter().map(|a| a.updated));
    for node in state.store.revision_tree(work).await? {
        for rev in node.revisions {
            stamps.push(rev.created);
            stamps.extend(rev.edited);
        }
    }
    Ok(stamps.iter().filter_map(|s| normalize_stamp(s)).max().unwrap_or_else(|| "2000-01-01T00:00:00Z".into()))
}

/// Bildtyp nach dem Inhalt (PNG, JPEG, GIF, SVG, WebP), erst ohne erkennbaren Inhalt nach der Dateiendung: Nach einem
/// Typwechsel (PNG ersetzt `haus.svg`, Pfade behalten die alte Endung) stimmt die Endung nicht mehr (Runde 21,
/// Nachbesserung).
fn media_type(path: &str, bytes: &[u8]) -> &'static str {
    if let Some(mime) = baruch_store::detect_mime(bytes) {
        return mime;
    }
    match path.rsplit_once('.').map(|(_, ext)| ext.to_ascii_lowercase()).as_deref() {
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("svg") => "image/svg+xml",
        Some("webp") => "image/webp",
        _ => "application/octet-stream",
    }
}

/// Eingabe für [`baruch_epub::build`] aus den aktuellen Fassungen (wie Export und Satz).
pub async fn epub_input(state: &AppState, work: WorkId, footnotes: FootnoteMode) -> Result<EpubInput, RpcError> {
    let w = state.store.work_meta(work).await?;
    let map = state.store.export_map(work).await?;
    let first = map.sections.iter().map(|s| s.start).next().unwrap_or(map.typst.len());
    let template = map.typst[..first].to_owned();

    // Angezeigte Nummern aus dem eingestellten Gliederungsschema (wie in der Gliederung).
    let numbering = state.store.outline_numbering(work).await?;
    let numbers: BTreeMap<i64, String> = state
        .store
        .numbering_preview(work, &numbering.scheme, numbering.custom.as_deref())
        .await
        .map(|rows| rows.into_iter().map(|r| (r.key, r.number)).collect())
        .unwrap_or_default();

    let mut sections = Vec::new();
    for s in map.sections.iter().filter(|s| s.start < s.end) {
        let body = &map.typst[s.start..s.end];
        // Ohne lesbare Überschrift bleibt der Text erhalten (Ebene 2, in der Datei des Kapitels davor).
        let (level, title) = match baruch_typst::parse(body).ok().and_then(|d| d.headings.into_iter().next()) {
            Some(h) => (h.level, h.title),
            None => (2, String::new()),
        };
        sections.push(EpubSection {
            key: s.key,
            level,
            number: numbers.get(&s.key).map(|n| n.trim().to_owned()).filter(|n| !n.is_empty()),
            title,
            body: body.to_owned(),
        });
    }

    let typeset = crate::cover::typeset_settings(state, work).await?;
    let language = typeset
        .lang
        .clone()
        .filter(|l| baruch_epub::template::is_language_tag(l.trim()))
        .or_else(|| baruch_epub::template::template_lang(&template))
        .unwrap_or_else(|| "de".into());

    let book = state.store.book_meta(work).await?;
    let info = crate::cover::book_info(state, work, &book).await?;
    let isbn = baruch_typst::cover::isbn13_check(&info.isbn).ok();

    // Bilder: aktuelle Fassung je Pfad, Alternativtext aus den Bildangaben.
    let mut alts: BTreeMap<String, String> = BTreeMap::new();
    for asset in state.store.asset_list(work).await? {
        for path in &asset.paths {
            if let Some(normal) = baruch_store::normalize_asset_path(path) {
                alts.insert(normal, asset.alt.clone());
            }
        }
    }
    let images = state
        .store
        .export_assets(work)
        .await?
        .into_iter()
        .map(|f| EpubImage {
            media_type: media_type(&f.path, &f.bytes).to_owned(),
            alt: alts.get(&f.path).cloned().unwrap_or_default(),
            path: f.path,
            bytes: f.bytes,
        })
        .collect();

    let (cover, _) = epub_cover(state, work).await?;

    let stamp = modified(state, work, &w).await?;
    Ok(EpubInput {
        meta: EpubMeta {
            identifier: format!("urn:uuid:{}", w.uuid),
            title: w.title,
            subtitle: w.subtitle,
            author: w.author,
            language,
            isbn,
            publisher: info.publisher.clone(),
            year: info.year.clone(),
            rights: baruch_typst::cover::copyright_line(&info),
            image_credit: info.image_credit.clone(),
            description: info.blurb.clone(),
            modified: stamp,
        },
        template,
        sections,
        typeset,
        images,
        cover,
        front_matter: book.front_matter,
        imprint: baruch_typst::cover::imprint_lines(&info),
        dedication: info.dedication.clone(),
        footnotes,
        // R31: Register und Glossar (glossary.xhtml, index-{slug}.xhtml); inaktiv ohne Wirkung.
        back_matter: state.store.back_matter(work).await?,
    })
}

/// Rohe Bilddatei des Umschlags (aktuelle Fassung, Alternativtext), falls das Werk einen Umschlag hat.
async fn raw_cover(state: &AppState, work: WorkId) -> Result<Option<EpubImage>, RpcError> {
    let Some(name) = crate::cover::cover_name(state, work).await? else { return Ok(None) };
    let data = state.store.asset_data(work, &name, None).await?;
    let revisions = state.store.asset_revisions(work, &name).await?;
    let alt = revisions.iter().find(|r| r.current).map(|r| r.alt.clone()).unwrap_or_default();
    Ok(Some(EpubImage { path: name.clone(), media_type: data.mime.clone(), bytes: data.bytes()?, alt }))
}

/// Zwischenspeicher der gerasterten Vorderseiten: (Stand der Quelle, PNG).
type RasterCache = std::sync::Mutex<Vec<(String, Vec<u8>)>>;

/// Gerasterte Vorderseiten je Stand der Quelle (höchstens acht, älteste zuerst weg).
fn raster_cache() -> &'static RasterCache {
    static CACHE: std::sync::OnceLock<RasterCache> = std::sync::OnceLock::new();
    CACHE.get_or_init(Default::default)
}

/// Die Umschlag-Vorderseite als PNG (höchstens [`COVER_MAX_WIDTH`] × [`COVER_MAX_HEIGHT`]); Fehler mit Grund.
pub async fn front_cover_png(state: &AppState, work: WorkId) -> Result<Vec<u8>, String> {
    let source = crate::cover::front_cover_source(state, work).await?;
    let stand = baruch_render::stand(&source);
    if let Some(png) =
        raster_cache().lock().ok().and_then(|c| c.iter().find(|(s, _)| *s == stand).map(|(_, png)| png.clone()))
    {
        return Ok(png);
    }
    let raster =
        tokio::task::spawn_blocking(move || baruch_render::render_png(&source, 0, COVER_MAX_WIDTH, COVER_MAX_HEIGHT))
            .await
            .map_err(|e| format!("Rasterbild: {e}"))??;
    if let Ok(mut cache) = raster_cache().lock() {
        if cache.len() >= 8 {
            cache.remove(0);
        }
        cache.push((stand, raster.png.clone()));
    }
    Ok(raster.png)
}

/// EPUB-Cover (Runde 21c, siehe Moduldoku): das Rasterbild der Vorderseite mit Titel, sonst die rohe Bilddatei des
/// Umschlags mit Warnung [`COVER_RASTER_FAILED`] (ohne Umschlagbild dann kein Cover, ebenfalls mit Warnung).
pub async fn epub_cover(state: &AppState, work: WorkId) -> Result<(Option<EpubImage>, Vec<EpubWarning>), RpcError> {
    let raw = raw_cover(state, work).await?;
    match front_cover_png(state, work).await {
        Ok(png) => {
            let alt = raw.as_ref().map(|r| r.alt.clone()).unwrap_or_default();
            Ok((
                Some(EpubImage { path: "cover.png".into(), media_type: "image/png".into(), bytes: png, alt }),
                Vec::new(),
            ))
        }
        Err(reason) => {
            let message = match &raw {
                Some(image) => format!(
                    "EPUB-Cover: Die Umschlag-Vorderseite ließ sich nicht als Bild setzen ({reason}); statt dessen steht \
                     die Bilddatei des Umschlags „{}“ ohne Titel im EPUB.",
                    image.path
                ),
                None => format!(
                    "EPUB-Cover: Die Umschlag-Vorderseite ließ sich nicht als Bild setzen ({reason}); das EPUB hat kein \
                     Cover."
                ),
            };
            Ok((raw, vec![EpubWarning { code: COVER_RASTER_FAILED.into(), message, key: None }]))
        }
    }
}

/// Gebautes Buch zum aktuellen Stand; Warnungen des Covers ([`epub_cover`]) stehen vorn in `warnings`.
pub async fn book_for(state: &AppState, work: WorkId, footnotes: FootnoteMode) -> Result<Arc<EpubBook>, RpcError> {
    book_for_draft(state, work, footnotes, None).await
}

/// Runde 22: Entwurf für das EPUB – wirksam nach dem dreiwertigen Schalter ([`crate::typeset::effective_draft`]);
/// das Datum (nur mit `draft.date`, Ortszeit) schreibt der Server vorn in den Zusatz, damit baruch-epub ohne Uhr
/// bleibt.
pub fn epub_draft(
    stored: Option<&baruch_typst::typeset::Draft>,
    choice: Option<bool>,
) -> Option<baruch_typst::typeset::Draft> {
    crate::typeset::effective_draft(stored, choice).map(|d| {
        let date = d.date.then(crate::typeset::draft_today);
        let note = baruch_typst::typeset::draft_line(&d, date.as_deref());
        baruch_typst::typeset::Draft { note, date: false, ..d }
    })
}

/// Wie [`book_for`] mit dem Entwurf-Schalter (Runde 22): mit Entwurf Hinweis auf der Titelseite und „(Entwurf)“ im
/// Titel-Metadatum; ohne Entwurf unverändert.
pub async fn book_for_draft(
    state: &AppState,
    work: WorkId,
    footnotes: FootnoteMode,
    draft: Option<bool>,
) -> Result<Arc<EpubBook>, RpcError> {
    let mut input = epub_input(state, work, footnotes).await?;
    input.typeset.draft = epub_draft(input.typeset.draft.as_ref(), draft);
    let mut book = baruch_epub::build(&input).map_err(epub_error)?;
    // Das Rasterbild liegt im Zwischenspeicher; ein zweiter Aufruf kostet nur die Abfrage der Warnung.
    let (_, mut warnings) = epub_cover(state, work).await?;
    warnings.append(&mut book.warnings);
    book.warnings = warnings;
    Ok(Arc::new(book))
}

/// (Dateiname `<slug>.epub`, ZIP-Bytes).
pub async fn epub_bytes(
    state: &AppState,
    work: WorkId,
    footnotes: FootnoteMode,
) -> Result<(String, Vec<u8>), RpcError> {
    epub_bytes_draft(state, work, footnotes, None).await
}

/// Wie [`epub_bytes`] mit dem Entwurf-Schalter (Runde 22).
pub async fn epub_bytes_draft(
    state: &AppState,
    work: WorkId,
    footnotes: FootnoteMode,
    draft: Option<bool>,
) -> Result<(String, Vec<u8>), RpcError> {
    let book = book_for_draft(state, work, footnotes, draft).await?;
    let slug = state.store.work_slug(work).await?;
    Ok((format!("{slug}.epub"), baruch_epub::package(&book).map_err(epub_error)?))
}

/// EPUB nach `target` schreiben: (Bytes, Warnungen). Eine vorhandene Datei bleibt unangetastet (Fehler).
pub async fn write_epub_file(
    state: &AppState,
    work: WorkId,
    target: &std::path::Path,
) -> Result<(usize, Vec<EpubWarning>), String> {
    if std::fs::symlink_metadata(target).is_ok() {
        return Err(format!("{} gibt es schon; nichts überschrieben", target.display()));
    }
    let book = book_for(state, work, FootnoteMode::Popup).await.map_err(|e| e.message)?;
    let bytes = baruch_epub::package(&book).map_err(|e| e.to_string())?;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(target)
        .map_err(|e| format!("{}: {e}", target.display()))?;
    std::io::Write::write_all(&mut file, &bytes).map_err(|e| format!("{}: {e}", target.display()))?;
    Ok((bytes.len(), book.warnings.clone()))
}

pub fn pages() -> Router<Arc<AppState>> {
    Router::new().route("/werke/{id}/epub", get(download))
}

#[derive(Deserialize)]
struct DownloadQuery {
    fussnoten: Option<String>,
    /// Runde 22: `1` mit Entwurf, `0` ohne, fehlt = nach Einstellung.
    entwurf: Option<String>,
}

async fn download(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<WorkId>,
    Query(q): Query<DownloadQuery>,
) -> Response {
    if crate::guard::cross_site_get(&headers) {
        return (StatusCode::FORBIDDEN, "Abgelehnt: Das EPUB entsteht nicht auf Anfrage einer fremden Seite.")
            .into_response();
    }
    let footnotes = match q.fussnoten.as_deref() {
        None | Some("") | Some("popup") => FootnoteMode::Popup,
        Some("endnoten") => FootnoteMode::Endnotes,
        Some(_) => return (StatusCode::BAD_REQUEST, "fussnoten: popup oder endnoten").into_response(),
    };
    let draft = match crate::typeset::draft_query(q.entwurf.as_deref()) {
        Ok(draft) => draft,
        Err(message) => return (StatusCode::BAD_REQUEST, message).into_response(),
    };
    if let Err(e) = state.store.work_slug(id).await {
        return match e {
            StoreError::UnknownWork(_) => (StatusCode::NOT_FOUND, "unbekanntes Werk").into_response(),
            e => (StatusCode::INTERNAL_SERVER_ERROR, format!("Fehler: {e}")).into_response(),
        };
    }
    match epub_bytes_draft(&state, id, footnotes, draft).await {
        Ok((file_name, bytes)) => {
            let disposition = HeaderValue::from_str(&format!("attachment; filename=\"{file_name}\""))
                .unwrap_or_else(|_| HeaderValue::from_static("attachment"));
            (
                [
                    (header::CONTENT_TYPE, HeaderValue::from_static(baruch_epub::MIMETYPE)),
                    (header::CONTENT_DISPOSITION, disposition),
                    (header::X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff")),
                    (header::CACHE_CONTROL, HeaderValue::from_static("no-cache")),
                ],
                bytes,
            )
                .into_response()
        }
        Err(e) if e.code == REFUSED => (StatusCode::UNPROCESSABLE_ENTITY, e.message).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, format!("Fehler: {}", e.message)).into_response(),
    }
}
