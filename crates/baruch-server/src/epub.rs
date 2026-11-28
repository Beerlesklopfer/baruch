// SPDX-License-Identifier: AGPL-3.0-or-later
//! EPUB of a work (Round 20a, owner “epub”): collect the input ([`epub_input`]), build ([`book_for`]), pack
//! ([`epub_bytes`]), write as a file ([`write_epub_file`], CLI `export --epub`).
//!
//! - **RPC** ([`handles`], [`call`]): `work.epub {work, inline?, footnotes?}` → `{file_name, bytes, sha256, url,
//!   footnotes, files, warnings, epub_base64?}`; no commit; without sections -32000.
//! - **Page** ([`pages`]): `GET /werke/{id}/epub[?fussnoten=popup|endnoten]` → `application/epub+zip` as attachment
//!   `<slug>.epub`; request of a foreign page → 403, without sections → 422.
//! - **Draft (Round 22):** `work.epub {draft?}` and `?entwurf=1|0` (three-valued, never changes the setting); with
//!   draft the title page carries the note and `dc:title` „<Titel> (Entwurf)“ ([`book_for_draft`]).
//!
//! The input comes from the same sources as export and typesetting: current revisions and work template
//! (`Store::export_map`), numbers from the outline scheme (`Store::numbering_preview`), images
//! (`Store::export_assets`, alternative texts from the image details), cover and book details (`Store::book_meta`,
//! [`crate::cover::book_info`] → ISBN checked with `isbn13_check`, imprint from `imprint_lines`, rights from
//! `copyright_line`), typeset settings ([`crate::cover::typeset_settings`]). Language: typeset setting, otherwise
//! `#set text(lang: …)` of the work template, otherwise „de“. `dcterms:modified` is the latest time from work,
//! book details, typeset settings, images and revisions – same state, same bytes.
//!
//! **EPUB cover (Round 21c, [`epub_cover`]):** the cover is a raster image (PNG, at most [`COVER_MAX_WIDTH`] ×
//! [`COVER_MAX_HEIGHT`] pixels, aspect ratio of the front) of the cover front with title, subtitle and
//! author from the cover layout ([`crate::cover::front_cover_source`], rasterised with
//! [`baruch_render::render_png`]) – also without a cover image (then on the dark area). If rasterising fails, the
//! EPUB takes the raw image file of the cover (if there is one) and reports `COVER_RASTER_FAILED` in `warnings`;
//! never a silent omission. Same state → same bytes (cache per state of the source).

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

/// Maximum width of the EPUB cover in pixels (usual cover size 1600 × 2560).
pub const COVER_MAX_WIDTH: u32 = 1600;
/// Maximum height of the EPUB cover in pixels.
pub const COVER_MAX_HEIGHT: u32 = 2560;
/// Code of the warning if the front could not be rasterised.
pub const COVER_RASTER_FAILED: &str = "COVER_RASTER_FAILED";

/// Methods of this module.
pub fn handles(method: &str) -> bool {
    method == "work.epub"
}

#[derive(Deserialize)]
struct EpubParams {
    #[serde(default)]
    inline: bool,
    #[serde(default)]
    footnotes: FootnoteMode,
    /// Round 22: draft three-valued (missing = by typeset setting).
    #[serde(default)]
    draft: Option<bool>,
}

fn epub_error(e: EpubError) -> RpcError {
    match e {
        EpubError::NoSections => RpcError::new(REFUSED, e.to_string()),
        EpubError::Invalid(_) | EpubError::Zip(_) => RpcError::new(INTERNAL, e.to_string()),
    }
}

/// Word for the footnote kind in answers (`popup` | `endnotes`).
pub fn footnote_word(mode: FootnoteMode) -> &'static str {
    match mode {
        FootnoteMode::Popup => "popup",
        FootnoteMode::Endnotes => "endnotes",
    }
}

/// Files in reading order for answers: `{href, media_type, kind, title, key}`.
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

/// Time of the database (`JJJJ-MM-TTThh:mm:ss[.fff][Z]` or with a space instead of `T`) in the form of
/// `dcterms:modified`; unreadable → `None`.
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

/// Latest time of the state: work, book details, typeset settings, images, revisions.
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

/// Image type by content (PNG, JPEG, GIF, SVG, WebP), only without recognisable content by the file extension:
/// after a type change (PNG replaces `haus.svg`, paths keep the old extension) the extension is no longer right
/// (Round 21, rework).
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

/// Input for [`baruch_epub::build`] from the current revisions (like export and typesetting).
pub async fn epub_input(state: &AppState, work: WorkId, footnotes: FootnoteMode) -> Result<EpubInput, RpcError> {
    let w = state.store.work_meta(work).await?;
    let map = state.store.export_map(work).await?;
    let first = map.sections.iter().map(|s| s.start).next().unwrap_or(map.typst.len());
    let template = map.typst[..first].to_owned();

    // Displayed numbers from the configured outline scheme (as in the outline).
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
        // Without a readable heading the text is kept (level 2, in the file of the chapter before).
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

    // Images: current revision per path, alternative text from the image details.
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
        // R31: index and glossary (glossary.xhtml, index-{slug}.xhtml); inactive without effect.
        back_matter: state.store.back_matter(work).await?,
    })
}

/// Raw image file of the cover (current revision, alternative text) if the work has a cover.
async fn raw_cover(state: &AppState, work: WorkId) -> Result<Option<EpubImage>, RpcError> {
    let Some(name) = crate::cover::cover_name(state, work).await? else { return Ok(None) };
    let data = state.store.asset_data(work, &name, None).await?;
    let revisions = state.store.asset_revisions(work, &name).await?;
    let alt = revisions.iter().find(|r| r.current).map(|r| r.alt.clone()).unwrap_or_default();
    Ok(Some(EpubImage { path: name.clone(), media_type: data.mime.clone(), bytes: data.bytes()?, alt }))
}

/// Cache of the rasterised fronts: (state of the source, PNG).
type RasterCache = std::sync::Mutex<Vec<(String, Vec<u8>)>>;

/// Rasterised fronts per state of the source (at most eight, oldest dropped first).
fn raster_cache() -> &'static RasterCache {
    static CACHE: std::sync::OnceLock<RasterCache> = std::sync::OnceLock::new();
    CACHE.get_or_init(Default::default)
}

/// The cover front as PNG (at most [`COVER_MAX_WIDTH`] × [`COVER_MAX_HEIGHT`]); errors with reason.
pub async fn front_cover_png(state: &AppState, work: WorkId) -> Result<Vec<u8>, String> {
    let source = crate::cover::front_cover_source(state, work).await?;
    let stand = baruch_render::stand(&source);
    if let Some(png) =
        raster_cache().lock().ok().and_then(|c| c.iter().find(|(s, _)| *s == stand).map(|(_, png)| png.clone()))
    {
        return Ok(png);
    }
    let raster = tokio::task::spawn_blocking(crate::i18n::carry(move || {
        baruch_render::render_png(&source, 0, COVER_MAX_WIDTH, COVER_MAX_HEIGHT)
    }))
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

/// EPUB cover (Round 21c, see module doc): the raster image of the front with title, otherwise the raw image file of
/// the cover with warning [`COVER_RASTER_FAILED`] (without a cover image then no cover, also with a warning).
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

/// Built book for the current state; warnings of the cover ([`epub_cover`]) stand in front in `warnings`.
pub async fn book_for(state: &AppState, work: WorkId, footnotes: FootnoteMode) -> Result<Arc<EpubBook>, RpcError> {
    book_for_draft(state, work, footnotes, None).await
}

/// Round 22: draft for the EPUB – effective by the three-valued switch ([`crate::typeset::effective_draft`]);
/// the date (only with `draft.date`, local time) is written by the server at the front of the addition so that
/// baruch-epub stays without a clock.
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

/// Like [`book_for`] with the draft switch (Round 22): with draft a note on the title page and „(Entwurf)“ in the
/// title metadata; without draft unchanged.
pub async fn book_for_draft(
    state: &AppState,
    work: WorkId,
    footnotes: FootnoteMode,
    draft: Option<bool>,
) -> Result<Arc<EpubBook>, RpcError> {
    let mut input = epub_input(state, work, footnotes).await?;
    input.typeset.draft = epub_draft(input.typeset.draft.as_ref(), draft);
    let mut book = baruch_epub::build(&input).map_err(epub_error)?;
    // The raster image lies in the cache; a second call costs only the query of the warning.
    let (_, mut warnings) = epub_cover(state, work).await?;
    warnings.append(&mut book.warnings);
    book.warnings = warnings;
    Ok(Arc::new(book))
}

/// (File name `<slug>.epub`, ZIP bytes).
pub async fn epub_bytes(
    state: &AppState,
    work: WorkId,
    footnotes: FootnoteMode,
) -> Result<(String, Vec<u8>), RpcError> {
    epub_bytes_draft(state, work, footnotes, None).await
}

/// Like [`epub_bytes`] with the draft switch (Round 22).
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

/// Write the EPUB to `target`: (bytes, warnings). An existing file stays untouched (error).
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
    /// Round 22: `1` with draft, `0` without, missing = by setting.
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
