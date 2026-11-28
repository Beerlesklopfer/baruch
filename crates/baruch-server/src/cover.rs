// SPDX-License-Identifier: AGPL-3.0-or-later
//! Cover, front matter and book details per work (contract Round 11, task cover).
//!
//! - **Data:** [`baruch_store::BookMeta`] (migration 0017, table `work_book`) with cover image,
//!   title area, front matter on/off, cover pages in the typeset book, imprint details, blurb, short biography,
//!   author photo, paper thickness and bleed. Subtitle, author and Bible translation stay in `work`.
//! - **Frame in the typeset book** ([`frame`]): inserts cover front and front matter before the first section,
//!   the back after the last ([`baruch_typst::cover::frame_parts`]), shifts the
//!   [`ExportMap`] and adds cover, photo and barcode as virtual files under `.baruch/`.
//!   Without cover (with cover pages) and without front matter, source and map stay byte-identical.
//! - **Cover for the print shop** ([`cover_source`]): one page back | spine | front with
//!   bleed; page size from the first book page, spine = (pages − cover pages) × paper thickness per page
//!   (stored `paper_mm`; since Round 20c UI and RPC name the thickness per sheet `paper_sheet_mm` = 2 ×
//!   `paper_mm`). Round 20c: the cover and the measuring of the blocks take over the text/paragraph rules of the work
//!   template ([`cover_text_rules`]), like the cover pages in the book.
//! - **Typeset settings (Round 29):** the typeset book already carries the rules „// Baruch: Satz“ in the source; so
//!   the first book page has the typeset format, and the page count for the spine includes empty pages (chapter on a
//!   right-hand page, empty back of the part page). The page size of the cover pages in the book ([`frame`],
//!   `book_page_mm`) measures the work template with the rule block ([`page_template`]); cover blocks without a font
//!   take the base font of the book ([`book_font`]: setting, otherwise work template).
//! - **RPC** ([`handles`], [`call`]): `work.set_cover`, `work.meta.get`, `work.meta.set`, `work.cover_pdf`
//!   (Round 22: `draft?` three-valued, also `GET /werke/{id}/umschlag.pdf?entwurf=1|0`; watermark by setting
//!   only with `draft.cover`, the print check then reports `draft`).
//! - **Pages** ([`pages`]): `GET /werke/{id}/umschlag` (preview, forms, print check),
//!   `GET /werke/{id}/umschlag.svg` (preview as SVG), `GET /werke/{id}/umschlag.pdf`.
//!   **Forms** ([`writes`], behind `guard::same_origin`): `POST /werke/umschlag/act`
//!   (`action=set|remove|area`) and `POST /werke/buchangaben/act`; success → 303 to `back` (only a
//!   relative, safe address, default `/einstellungen/werk?work=W`), invalid → 422 with the input.
//! - **Mirror:** per change its own commit (empty allowed, the book details are not in the mirror):
//!   „Buchangaben geändert: <Felder>“ (`Baruch-Action: book-meta`), „Umschlag: <name> gesetzt“
//!   (`cover-set`), „Umschlag entfernt“ (`cover-removed`); body with before/after.
//!
//! - **Cover editor (Round 27c):** above the Typst preview (the same source as the PDF, [`build_wrap_for`]) the
//!   cover page shows the frames of the blocks from `cover.layout` ([`crate::cover_layout::layout_view`]), guides
//!   (bleed, trim edge, safety margin, spine edges, centre) and per block all values as a form
//!   (`b.<kind>.<feld>`, [`BLOCK_FIELDS`]). Without JavaScript: preview through `GET /werke/{id}/umschlag?b.…` (stores
//!   nothing), applying through `POST /werke/umschlag/anordnung` (`action=save|reset|quick`, [`layout_act`]); the
//!   widget `cover-editor` (`web/src/cover_editor.ts`) stores through `cover.layout_set` or `cover.layout_reset`.
//!   Book details also with the addresses of publisher and print shop; button „Impressum-Vorlage in die Werkvorlage
//!   übernehmen“ (`POST /werke/umschlag/impressum`, RPC `cover.imprint_adopt` through `work.update`).
//! - **EPUB (Round 20):** [`book_info`] is `pub(crate)` for `crate::epub`; the cover as `cover-image` is put
//!   into the EPUB by `crate::epub::epub_input` (R20a).

use std::collections::BTreeMap;
use std::sync::{Arc, OnceLock};

use axum::Router;
use axum::extract::{Form, Path, Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use baruch_gitmirror::{CommitRequest, MirrorFile, Signature};
use baruch_render::Source;
use baruch_store::{
    BookEdit, BookMeta, CoverBlock, CoverBlockKind, CoverFace, CoverGeometry, CoverLayout, ExportMap, StoreError,
    TitleArea, WorkEdit, WorkId,
};
use baruch_typst::cover::{self as tc, BookInfo, CoverRef, FrameOptions, WrapOptions};
use baruch_typst::cover_layout::{self as cl, Backdrop, CoverAlign, FontStyle, SpineDirection};
use baruch_typst::typeset::TypesetSettings;
use minijinja::value::Serde;
use minijinja::{Environment, Value as Jinja, context};
use serde_json::{Value, json};

use crate::AppState;
use crate::i18n::tc;
use crate::mirror::{HUMAN_EMAIL, HUMAN_NAME, WriteGuard};
use crate::rpc::{INTERNAL, INVALID_PARAMS, METHOD_NOT_FOUND, REFUSED, RpcError};
use crate::view::Settings;

/// Methods of this module.
pub fn handles(method: &str) -> bool {
    matches!(method, "work.set_cover" | "work.meta.get" | "work.meta.set" | "work.cover_pdf" | "cover.imprint_adopt")
}

fn invalid(message: impl Into<String>) -> RpcError {
    RpcError::new(INVALID_PARAMS, message)
}

/// Error of the store; a rejected value of the book details is an invalid input (-32602).
fn input_error(e: StoreError) -> RpcError {
    match e {
        StoreError::Refused(message) => invalid(message),
        e => e.into(),
    }
}

// ---- Reading -------------------------------------------------------------------------------------

/// Image name of the cover.
pub async fn cover_name(state: &AppState, work: WorkId) -> Result<Option<String>, StoreError> {
    state.store.work_cover(work).await
}

/// Cover for UI and RPC: `{name, caption, alt, url, thumb}` or `None`.
pub async fn cover_info(state: &AppState, work: WorkId) -> Result<Option<Value>, StoreError> {
    let Some(name) = cover_name(state, work).await? else { return Ok(None) };
    Ok(Some(info_of(state, work, &name).await?))
}

async fn info_of(state: &AppState, work: WorkId, name: &str) -> Result<Value, StoreError> {
    let revisions = state.store.asset_revisions(work, name).await?;
    let current = revisions.iter().find(|r| r.current);
    Ok(json!({
        "name": name,
        "caption": current.map_or("", |r| r.caption.as_str()),
        "alt": current.map_or("", |r| r.alt.as_str()),
        "url": format!("/werke/{work}/bilder/{name}/datei"),
        "thumb": format!("/werke/{work}/bilder/{name}/vorschau?b=320"),
    }))
}

/// Details for cover and front matter from work and book details; without an image credit the caption
/// of the cover.
pub(crate) async fn book_info(state: &AppState, work: WorkId, meta: &BookMeta) -> Result<BookInfo, StoreError> {
    let w = state.store.work_meta(work).await?;
    let mut image_credit = meta.image_credit.clone();
    if image_credit.trim().is_empty()
        && let Some(cover) = &meta.cover
    {
        let revisions = state.store.asset_revisions(work, cover).await?;
        if let Some(current) = revisions.iter().find(|r| r.current) {
            image_credit = current.caption.clone();
        }
    }
    Ok(BookInfo {
        title: w.title,
        subtitle: w.subtitle,
        author: w.author,
        publisher: meta.publisher.clone(),
        place: meta.place.clone(),
        year: meta.year.clone(),
        edition: meta.edition.clone(),
        isbn: meta.isbn.clone(),
        price: meta.price.clone(),
        series: meta.series.clone(),
        printer: meta.printer.clone(),
        bible: w.bible,
        bible_notice: meta.bible_notice.clone(),
        image_credit,
        dedication: meta.dedication.clone(),
        blurb: meta.blurb.clone(),
        bio: meta.bio.clone(),
        publisher_address: meta.publisher_address.clone(),
        printer_address: meta.printer_address.clone(),
    })
}

pub(crate) fn title_area(area: TitleArea) -> tc::TitleArea {
    match area {
        TitleArea::Top => tc::TitleArea::Top,
        TitleArea::Middle => tc::TitleArea::Middle,
        TitleArea::Bottom => tc::TitleArea::Bottom,
    }
}

/// Virtual files and references for cover, photo and barcode.
struct Extras {
    files: Vec<(String, Vec<u8>)>,
    cover: Option<CoverRef>,
    photo: Option<String>,
    barcode: Option<String>,
}

async fn extras(state: &AppState, work: WorkId, meta: &BookMeta, with_back: bool) -> Result<Extras, StoreError> {
    let mut out = Extras { files: Vec::new(), cover: None, photo: None, barcode: None };
    if let Some(name) = &meta.cover {
        let data = state.store.asset_data(work, name, None).await?;
        let path = format!("{}/{name}", tc::VIRTUAL_DIR);
        let alt = state
            .store
            .asset_revisions(work, name)
            .await?
            .into_iter()
            .find(|r| r.current)
            .map(|r| r.alt)
            .unwrap_or_default();
        out.files.push((path.clone(), data.bytes()?));
        out.cover = Some(CoverRef { path, alt });
    }
    if !with_back {
        return Ok(out);
    }
    if let Some(name) = &meta.author_photo {
        let data = state.store.asset_data(work, name, None).await?;
        let path = format!("{}/{name}", tc::VIRTUAL_DIR);
        out.files.push((path.clone(), data.bytes()?));
        out.photo = Some(path);
    }
    if let Ok(digits) = tc::isbn13(&meta.isbn) {
        out.files.push((tc::BARCODE_PATH.to_owned(), tc::ean13_svg(&digits).into_bytes()));
        out.barcode = Some(tc::BARCODE_PATH.to_owned());
    }
    Ok(out)
}

// ---- Frame in the typeset book -------------------------------------------------------------------

/// Insert cover and front matter into the typeset book (see module doc). Without cover with
/// cover pages and without front matter: `source` and `map` back unchanged.
///
/// Round 27: if the work has its own layout (rows in `cover_block`), front and back set the blocks
/// (missing kinds as default blocks, measures from the page size of the work template); a missing font falls back
/// to the font of the work template. If the work template defines `schmutztitel`, `haupttitel`, `impressum` or
/// `widmung`, the front matter uses these functions (see [`baruch_typst::cover::frame_parts`]).
pub async fn frame(
    state: &AppState,
    work: WorkId,
    source: Source,
    map: ExportMap,
) -> Result<(Source, ExportMap), StoreError> {
    let meta = state.store.book_meta(work).await?;
    let with_cover = meta.cover.is_some() && meta.cover_pages;
    if !with_cover && !meta.front_matter {
        return Ok((source, map));
    }
    let info = book_info(state, work, &meta).await?;
    let extras = extras(state, work, &meta, with_cover).await?;
    let template = state.store.work_template(work).await?;
    let settings = typeset_settings(state, work).await?;
    let stored = state.store.cover_layout(work).await?;
    let layout = if with_cover && !stored.is_empty() {
        match book_page_mm(&page_template(&template, &settings), &source.files).await {
            Some((width, height)) => {
                // For front and back the spine does not count; the default blocks of the spine are dropped.
                let geometry = CoverGeometry::new(width, height, 0.0, meta.bleed_mm);
                let blocks =
                    baruch_typst::cover_layout::effective_blocks(&stored, title_area(meta.title_area), &geometry)
                        .into_iter()
                        .map(|(block, _)| block)
                        .collect();
                Some(resolve_fonts(CoverLayout { blocks }, None).await)
            }
            None => None,
        }
    } else {
        None
    };
    let opts = FrameOptions {
        cover: extras.cover.clone(),
        cover_pages: meta.cover_pages,
        front_matter: meta.front_matter,
        title_area: title_area(meta.title_area),
        barcode: extras.barcode.clone(),
        author_photo: extras.photo.clone(),
        layout,
        template_functions: tc::template_functions(&template),
    };
    // Round 39b integration: labels of cover pages and front matter in the work language (German byte-equal).
    let lang = crate::work_language::of(state, work).await;
    let parts = tc::frame_parts_in(&info, &opts, lang);
    Ok(insert_parts(source, map, &parts.front, &parts.back, extras.files))
}

/// Typeset settings of the work (Round 29); without a row the default (everything from the work template).
pub(crate) async fn typeset_settings(state: &AppState, work: WorkId) -> Result<TypesetSettings, StoreError> {
    Ok(state.store.work_typeset(work).await?.map(|t| t.settings).unwrap_or_default())
}

/// Typeset settings for the cover sources (Round 39b integration): [`typeset_settings`] plus the book language
/// rule of the typeset book (`baruch_store` `snapshot::rule_settings`): an English work without its own `lang`
/// setting gets `lang: "en"`, so the cover text rules carry `#set text(lang: "en")`. German works unchanged
/// (byte-equal sources and cache states).
pub(crate) async fn cover_settings(state: &AppState, work: WorkId) -> Result<TypesetSettings, StoreError> {
    let mut settings = typeset_settings(state, work).await?;
    let own = settings.lang.as_deref().map(str::trim).is_some_and(|l| !l.is_empty());
    if !own && crate::work_language::of(state, work).await == baruch_core::WorkLanguage::En {
        settings.lang = Some(baruch_core::WorkLanguage::En.as_str().to_owned());
    }
    Ok(settings)
}

/// Template that determines the page size of the book (Round 29): work template and behind it the rule block of
/// the typeset settings (page format, margins) – as in the typeset book (`snapshot::head`). Without settings the
/// work template unchanged (byte-identical to before Round 29, same state in the cache of [`book_page_mm`]).
pub fn page_template(template: &str, settings: &TypesetSettings) -> String {
    let rules = baruch_typst::typeset::rules(settings);
    if rules.is_empty() { template.to_owned() } else { format!("{template}\n{rules}") }
}

/// Base font of the book (Round 29): the typeset setting `font`, otherwise the font of the work template. Fallback
/// for cover blocks without a font of their own (the font that applies in the book anyway).
pub fn book_font(template: &str, settings: &TypesetSettings) -> Option<String> {
    settings.font.clone().or_else(|| baruch_wysiwyg::template_font(template))
}

/// Text and paragraph rules for the print-shop cover and the measuring of the blocks (Round 20c): the rules of the
/// work template with the rule block of the typeset settings ([`page_template`]), filtered by
/// [`baruch_typst::cover::template_text_rules`] (language, hyphenation, tracking, paragraph rules). Without such
/// rules empty – then source and PDF stay byte-identical to the golden states R26/R27.
pub fn cover_text_rules(template: &str, settings: &TypesetSettings) -> String {
    tc::template_text_rules(&page_template(template, settings))
}

/// Time limit for the measuring runs of the cover ([`book_page_mm`], [`block_heights`]): like typesetting the book
/// ([`crate::typeset::COMPILE_TIMEOUT`], 60 s).
pub const MEASURE_TIMEOUT: std::time::Duration = crate::typeset::COMPILE_TIMEOUT;

/// Round 23a: Typst run with time limit. `Ok(Some)` = typeset, `Ok(None)` = time limit exceeded (the run is
/// marked as cancelled and ends at Typst's next checkpoint; its result is discarded), `Err` = the
/// worker thread failed.
pub async fn compile_limited(
    source: Source,
    limit: std::time::Duration,
) -> Result<Option<baruch_render::Rendered>, String> {
    let cancel = baruch_render::Cancel::new();
    let run = cancel.clone();
    let task =
        tokio::task::spawn_blocking(crate::i18n::carry(move || baruch_render::compile_cancellable(&source, &run)));
    match tokio::time::timeout(limit, task).await {
        Ok(Ok(rendered)) => Ok(rendered),
        Ok(Err(e)) => Err(e.to_string()),
        Err(_) => {
            cancel.cancel();
            Ok(None)
        }
    }
}

/// Page size (mm) of the work template: the template typeset with an invisible line, the first page measured.
/// Round 29: the caller passes in [`page_template`] so that the page format of the typeset settings applies.
/// So the frame knows the measures of the cover page in the book (it inherits width and height of the template)
/// without typesetting the book. Typesetting error → `None` (then without layout, as before). Cached per state.
async fn book_page_mm(template: &str, files: &[(String, Vec<u8>)]) -> Option<(f64, f64)> {
    static SIZES: OnceLock<std::sync::Mutex<BTreeMap<String, (f64, f64)>>> = OnceLock::new();
    let source = Source { main: format!("{template}\n#hide[x]\n"), files: files.to_vec() };
    let stand = baruch_render::stand(&source);
    let cache = SIZES.get_or_init(Default::default);
    if let Some(size) = cache.lock().ok().and_then(|c| c.get(&stand).copied()) {
        return Some(size);
    }
    // Round 23a: with the time limit of typesetting; a template that does not finish counts like a typesetting error.
    let rendered = compile_limited(source, MEASURE_TIMEOUT).await.ok()??;
    if rendered.has_errors() {
        return None;
    }
    let (width, height) = baruch_render::page_sizes_pt(&rendered).first().copied()?;
    let size = (width * MM_PER_PT, height * MM_PER_PT);
    if let Ok(mut cache) = cache.lock() {
        if cache.len() > 64 {
            cache.clear();
        }
        cache.insert(stand, size);
    }
    Some(size)
}

/// Resolve the fonts of the blocks: a font that does not exist is dropped (typeset with the default; the
/// print check reports `font`). With `fallback` a block without a font gets this one (print-shop cover and
/// measuring: the font of the work template, which applies in the book anyway), provided it exists.
pub(crate) async fn resolve_fonts(mut layout: CoverLayout, fallback: Option<String>) -> CoverLayout {
    tokio::task::spawn_blocking(crate::i18n::carry(move || {
        let fallback = fallback.filter(|f| baruch_render::has_font_family(f));
        for block in &mut layout.blocks {
            match &block.font {
                Some(font) if !baruch_render::has_font_family(font) => block.font = fallback.clone(),
                Some(_) => {}
                None => block.font = fallback.clone(),
            }
        }
        layout
    }))
    .await
    .unwrap_or_default()
}

/// `front` before the first section (without sections: at the end), `back` at the end; byte ranges and
/// first lines of all sections move along. The own files take precedence over the images of the work.
/// Also for the rule of the part pages (Round 14, `typeset::prepare`).
pub(crate) fn insert_parts(
    source: Source,
    mut map: ExportMap,
    front: &str,
    back: &str,
    extra_files: Vec<(String, Vec<u8>)>,
) -> (Source, ExportMap) {
    let mut typst = map.typst;
    if !front.is_empty() {
        let at = map.sections.first().map_or(typst.len(), |s| s.start);
        let mut front = front.to_owned();
        if at > 0 && !typst[..at].ends_with('\n') {
            front.insert(0, '\n');
        }
        let lines = front.matches('\n').count();
        typst.insert_str(at, &front);
        for section in &mut map.sections {
            section.start += front.len();
            section.end += front.len();
            section.first_line += lines;
        }
    }
    if !back.is_empty() {
        if !typst.is_empty() && !typst.ends_with('\n') {
            typst.push('\n');
        }
        typst.push_str(back);
    }
    let mut files = extra_files;
    files.extend(source.files);
    map.typst = typst.clone();
    (Source { main: typst, files }, map)
}

// ---- Cover for the print shop --------------------------------------------------------------------

/// Width and height (pt) from the head of a Typst SVG page.
fn svg_size_pt(svg: &str) -> Option<(f64, f64)> {
    baruch_render::svg_size_pt(svg)
}

const MM_PER_PT: f64 = 25.4 / 72.0;

/// Built cover and its measures.
pub struct Wrap {
    pub source: Source,
    pub spine_mm: f64,
    pub width_mm: f64,
    pub height_mm: f64,
    pub page_width_mm: f64,
    pub page_height_mm: f64,
    pub pages: usize,
    pub cover_pages: usize,
    pub bleed_mm: f64,
    /// Measures for the cover layout (Round 27).
    pub geometry: CoverGeometry,
}

/// Build the cover with the stored layout (like [`build_wrap_for`] with `None`): typeset the book
/// (cache of the typeset view), page size of the first page, spine from the page count without the
/// cover pages. Typesetting error → `REFUSED`. Without a layout byte-identical to Round 26.
pub async fn build_wrap(state: &AppState, work: WorkId) -> Result<Wrap, RpcError> {
    build_wrap_for(state, work, None).await
}

/// Build the cover with a layout: `layout` = preview (replaces the stored layout), `None` = stored
/// layout. An empty layout typesets the previous picture. Fonts that do not exist fall back to the font
/// of the work template or the embedded default. Typesetting error → `REFUSED` like [`build_wrap`].
pub async fn build_wrap_for(state: &AppState, work: WorkId, layout: Option<&CoverLayout>) -> Result<Wrap, RpcError> {
    let meta = state.store.book_meta(work).await?;
    let typeset = crate::typeset::typeset(state, work).await?;
    let rendered = typeset.rendered();
    if rendered.has_errors() || rendered.pages.is_empty() {
        let errors = rendered.diagnostics.iter().filter(|d| d.severity == baruch_render::Severity::Error).count();
        return Err(RpcError::new(
            REFUSED,
            tc("cover_msg.measure.typeset_errors", &[("errors", errors.into()), ("work", work.into())]),
        ));
    }
    let (width_pt, height_pt) = svg_size_pt(&rendered.pages[0])
        .ok_or_else(|| RpcError::new(INTERNAL, tc("cover_msg.measure.page_size", &[])))?;
    let pages = rendered.pages.len();
    // If the frame is already in the typeset book (after the integration through typeset::prepare), front,
    // inside and back do not count towards the book block.
    let framed = typeset.prepared.source.main.contains(tc::FRAME_MARK);
    let cover_pages = if framed && meta.cover.is_some() && meta.cover_pages { tc::COVER_PAGES } else { 0 };
    let spine = tc::spine_mm(pages.saturating_sub(cover_pages), meta.paper_mm);
    let info = book_info(state, work, &meta).await?;
    let extras = extras(state, work, &meta, true).await?;
    let (page_width_mm, page_height_mm) = (width_pt * MM_PER_PT, height_pt * MM_PER_PT);
    let layout = match layout {
        Some(layout) => layout.clone(),
        None => state.store.cover_layout(work).await?,
    };
    let template = state.store.work_template(work).await?;
    let settings = cover_settings(state, work).await?;
    let rules = cover_text_rules(&template, &settings);
    let layout =
        if layout.is_empty() { None } else { Some(resolve_fonts(layout, book_font(&template, &settings)).await) };
    let opts = WrapOptions {
        page_width_mm,
        page_height_mm,
        spine_mm: spine,
        bleed_mm: meta.bleed_mm,
        title_area: title_area(meta.title_area),
        cover: extras.cover,
        barcode: extras.barcode,
        author_photo: extras.photo,
        layout,
    };
    let lang = crate::work_language::of(state, work).await;
    let main = tc::wrap_source_with_rules_in(&info, &opts, &rules, lang);
    Ok(Wrap {
        source: Source { main, files: extras.files },
        spine_mm: spine,
        width_mm: 2.0 * page_width_mm + spine + 2.0 * meta.bleed_mm,
        height_mm: page_height_mm + 2.0 * meta.bleed_mm,
        page_width_mm,
        page_height_mm,
        pages,
        cover_pages,
        bleed_mm: meta.bleed_mm,
        geometry: CoverGeometry::new(page_width_mm, page_height_mm, spine, meta.bleed_mm),
    })
}

/// Height of every block in mm (same order as `blocks`), typeset with text, font and width of the block:
/// per block one page with `height: auto` ([`baruch_typst::cover::block_measure_source`]) whose height Typst
/// determines. A block without content (no text, no photo) has height 0. For the spine the height is the
/// horizontal extent on the page. Typesetting error of the measuring source → `REFUSED` with the message.
pub async fn block_heights(
    state: &AppState,
    work: WorkId,
    blocks: &[CoverBlock],
    geometry: &CoverGeometry,
) -> Result<Vec<f64>, RpcError> {
    let _ = geometry; // The height depends only on text, font and width, not on the position.
    if blocks.is_empty() {
        return Ok(Vec::new());
    }
    let meta = state.store.book_meta(work).await?;
    let info = book_info(state, work, &meta).await?;
    let extras = extras(state, work, &meta, true).await?;
    let template = state.store.work_template(work).await?;
    let settings = cover_settings(state, work).await?;
    let resolved = resolve_fonts(CoverLayout { blocks: blocks.to_vec() }, book_font(&template, &settings)).await;
    let refs = tc::BlockRefs { photo: extras.photo.as_deref(), barcode: extras.barcode.as_deref() };
    let has_content: Vec<bool> = resolved.blocks.iter().map(|b| tc::block_has_content(&info, b, &refs)).collect();
    let main =
        tc::block_measure_source_with_rules(&info, &resolved.blocks, &refs, &cover_text_rules(&template, &settings));
    let source = Source { main, files: extras.files };
    // Round 23a: with the typesetting time limit (60 s); after it the run is cancelled and the answer honest.
    let rendered = compile_limited(source, MEASURE_TIMEOUT)
        .await
        .map_err(|e| {
            RpcError::new(INTERNAL, tc("cover_msg.measure.block_heights", &[("error", e.to_string().into())]))
        })?
        .ok_or_else(|| {
            RpcError::new(REFUSED, tc("cover_msg.measure.timeout", &[("seconds", MEASURE_TIMEOUT.as_secs().into())]))
        })?;
    if rendered.has_errors() {
        let message = rendered
            .diagnostics
            .iter()
            .find(|d| d.severity == baruch_render::Severity::Error)
            .map_or_else(String::new, |d| d.message.clone());
        return Err(RpcError::new(REFUSED, tc("cover_msg.measure.blocks_failed", &[("message", message.into())])));
    }
    let sizes = baruch_render::page_sizes_pt(&rendered);
    Ok(has_content
        .iter()
        .enumerate()
        .map(|(i, has)| if *has { sizes.get(i).map_or(0.0, |(_, h)| h * MM_PER_PT) } else { 0.0 })
        .collect())
}

/// The cover front as Typst source (one page in the final format of the book, without bleed) for the EPUB cover
/// (Round 21c, [`baruch_typst::cover::front_source_with_rules`]): cover image (if set, otherwise the dark
/// area) and on it title, subtitle and author – from the stored layout (missing kinds as
/// default blocks), without an own layout in the title area as before. The format comes from the work template
/// with typeset settings (like the cover page in the typeset book), without typesetting the book. If the format
/// cannot be measured (typesetting error of the template) → `Err` with reason.
pub async fn front_cover_source(state: &AppState, work: WorkId) -> Result<Source, String> {
    let meta = state.store.book_meta(work).await.map_err(|e| e.to_string())?;
    let info = book_info(state, work, &meta).await.map_err(|e| e.to_string())?;
    let extras = extras(state, work, &meta, false).await.map_err(|e| e.to_string())?;
    let template = state.store.work_template(work).await.map_err(|e| e.to_string())?;
    let settings = cover_settings(state, work).await.map_err(|e| e.to_string())?;
    let assets: Vec<(String, Vec<u8>)> = state
        .store
        .export_assets(work)
        .await
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|f| (f.path, f.bytes))
        .collect();
    let Some((page_width_mm, page_height_mm)) = book_page_mm(&page_template(&template, &settings), &assets).await
    else {
        return Err(tc("cover_msg.measure.page_format", &[]));
    };
    let stored = state.store.cover_layout(work).await.map_err(|e| e.to_string())?;
    let layout =
        if stored.is_empty() { None } else { Some(resolve_fonts(stored, book_font(&template, &settings)).await) };
    let opts = WrapOptions {
        page_width_mm,
        page_height_mm,
        spine_mm: 0.0,
        bleed_mm: meta.bleed_mm,
        title_area: title_area(meta.title_area),
        cover: extras.cover,
        barcode: None,
        author_photo: None,
        layout,
    };
    let lang = crate::work_language::of(state, work).await;
    let main = tc::front_source_with_rules_in(&info, &opts, &cover_text_rules(&template, &settings), lang);
    Ok(Source { main, files: extras.files })
}

/// The whole cover as Typst source (one page), with the stored layout.
pub async fn cover_source(state: &AppState, work: WorkId) -> Result<Source, RpcError> {
    Ok(build_wrap_for(state, work, None).await?.source)
}

/// Result of the PDF generation for the cover.
struct CoverPdf {
    bytes: Vec<u8>,
    pages: usize,
    /// SHA-256 of the bytes (lower-case hex).
    sha256: String,
}

/// PDF from the cover source.
async fn to_pdf(work: WorkId, source: Source) -> Result<CoverPdf, RpcError> {
    let pdf = crate::pdf::source_to_pdf(work, crate::pdf::PdfKind::Cover, source, crate::pdf::PDF_TIMEOUT).await?;
    let sha256 = baruch_render::sha256_hex(&pdf.bytes);
    Ok(CoverPdf { pages: pdf.pages, sha256, bytes: pdf.bytes })
}

/// Cover PDF with the stored layout. Round 22: `draft` three-valued ([`crate::typeset::cover_draft`]) –
/// by setting only with `draft.cover`, otherwise only explicitly; the watermark stands before the first line of
/// the cover source (the print check then warns, [`crate::cover_print::cover_checks_with`]).
async fn cover_pdf(state: &AppState, work: WorkId, draft: Option<bool>) -> Result<(String, Wrap, CoverPdf), RpcError> {
    let slug = state.store.work_slug(work).await?;
    let mut wrap = build_wrap_for(state, work, None).await?;
    let stored = crate::typeset::stored_draft(state, work).await?;
    if let Some(d) = crate::typeset::cover_draft(stored.as_ref(), draft) {
        let lang = crate::work_language::of(state, work).await;
        wrap.source.main.insert_str(0, &crate::typeset::draft_rule_for_in(&d, lang));
    }
    let pdf = to_pdf(work, wrap.source.clone()).await?;
    Ok((format!("{slug}-umschlag.pdf"), wrap, pdf))
}

// ---- Writing -------------------------------------------------------------------------------------

pub(crate) fn human() -> Signature {
    Signature { name: HUMAN_NAME.to_owned(), email: HUMAN_EMAIL.to_owned() }
}

/// Own log commit (snapshot of the work, empty allowed). Errors only as text.
pub(crate) async fn commit(
    state: &AppState,
    held: &WriteGuard<'_>,
    work: WorkId,
    summary: String,
    body: String,
    action: &str,
) -> Result<Option<String>, String> {
    let request = CommitRequest {
        files: Vec::new(),
        author: human(),
        committer: human(),
        summary,
        body,
        trailers: vec![("Baruch-Action".to_owned(), action.to_owned())],
        allow_empty: true,
    };
    commit_request(state, held, work, request).await
}

/// Commit a finished commit job with the snapshot of the work (Round 20c: also for the one commit
/// of a confirmed cover request). The caller holds the write lock since before the DB write.
pub(crate) async fn commit_request(
    state: &AppState,
    _held: &WriteGuard<'_>,
    work: WorkId,
    mut request: CommitRequest,
) -> Result<Option<String>, String> {
    let files = state.store.mirror_snapshot(work).await.map_err(|e| e.to_string())?;
    request.files = files.into_iter().map(|f| MirrorFile { path: f.path, contents: f.contents }).collect();
    let mirror = state.mirror(work).await?;
    let result = tokio::task::spawn_blocking(crate::i18n::carry(move || mirror.commit(&request)))
        .await
        .map_err(|e| format!("Git: {e}"))?
        .map_err(|e| e.to_string());
    if let Err(e) = &result {
        eprintln!("Git-Spiegel (Werk {work}, Buchangaben): {e}");
        state.forget_mirror(work);
    }
    result
}

fn add_mirror(answer: &mut Value, result: Result<Option<String>, String>) {
    match result {
        Ok(Some(commit)) => answer["commit"] = json!(commit),
        Ok(None) => {}
        Err(e) => answer["mirror"] = json!(e),
    }
}

/// German names of the fields (commit subject, UI).
pub fn field_label(field: &str) -> &'static str {
    match field {
        "subtitle" => "Untertitel",
        "author" => "Autor",
        "bible" => "Bibelübersetzung",
        "publisher" => "Verlag",
        "place" => "Verlagsort",
        "year" => "Jahr",
        "edition" => "Auflage",
        "isbn" => "ISBN",
        "price" => "Preis",
        "series" => "Reihe",
        "printer" => "Druck",
        "bible_notice" => "Rechtehinweis der Bibelübersetzung",
        "image_credit" => "Bildnachweis",
        "dedication" => "Widmung",
        "blurb" => "Klappentext",
        "bio" => "Kurzbiografie",
        "publisher_address" => "Anschrift des Verlags bzw. Herausgebers",
        "printer_address" => "Anschrift der Druckerei",
        "author_photo" => "Autorenfoto",
        "title_area" => "Titelbereich",
        "front_matter" => "Titelei",
        "cover_pages" => "Umschlagseiten im Satz",
        "paper_mm" => "Papierdicke",
        "bleed_mm" => "Beschnitt",
        _ => "Buchangabe",
    }
}

/// Value of a field for the commit body, in one line (empty → „–“).
fn shown(value: &Value) -> String {
    let text = match value {
        Value::Null => String::new(),
        Value::Bool(true) => "an".to_owned(),
        Value::Bool(false) => "aus".to_owned(),
        Value::String(s) => s.split_whitespace().collect::<Vec<_>>().join(" "),
        other => other.to_string(),
    };
    if text.is_empty() { "–".to_owned() } else { text }
}

/// Body „Verlag: – → Gartenverlag“, one line per changed field; the paper thickness additionally per sheet.
fn change_body(changed: &[&str], before: &Value, after: &Value) -> String {
    changed
        .iter()
        .map(|f| {
            let line = format!("{}: {} → {}", field_label(f), shown(&before[*f]), shown(&after[*f]));
            match (*f, before[*f].as_f64(), after[*f].as_f64()) {
                ("paper_mm", Some(b), Some(a)) => {
                    format!(
                        "{line} mm je Seite (je Blatt {} → {} mm)\n",
                        shown(&json!(b * 2.0)),
                        shown(&json!(a * 2.0))
                    )
                }
                _ => format!("{line}\n"),
            }
        })
        .collect()
}

/// Value as text (number or text); `None` = not given.
fn text_param(params: &Value, name: &str) -> Result<Option<String>, RpcError> {
    match params.get(name) {
        None => Ok(None),
        Some(Value::String(s)) => Ok(Some(s.clone())),
        Some(Value::Number(n)) => Ok(Some(n.to_string())),
        Some(Value::Null) => Ok(Some(String::new())),
        Some(_) => Err(invalid(tc("cover_msg.param.text", &[("name", name.into())]))),
    }
}

fn bool_param(params: &Value, name: &str) -> Result<Option<bool>, RpcError> {
    match params.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Bool(b)) => Ok(Some(*b)),
        Some(Value::String(s)) => match s.trim() {
            "1" | "on" | "true" | "ja" => Ok(Some(true)),
            "" | "0" | "off" | "false" | "nein" => Ok(Some(false)),
            _ => Err(invalid(tc("cover_msg.param.switch", &[("name", name.into())]))),
        },
        Some(_) => Err(invalid(tc("cover_msg.param.switch", &[("name", name.into())]))),
    }
}

/// Number in mm, also as text with comma („0,1“).
fn mm_param(params: &Value, name: &str, label: &str) -> Result<Option<f64>, RpcError> {
    let wrong = || invalid(tc("cover_msg.param.mm", &[("label", label.into())]));
    match params.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Number(n)) => n.as_f64().map(Some).ok_or_else(wrong),
        Some(Value::String(s)) if s.trim().is_empty() => Ok(None),
        Some(Value::String(s)) => s.trim().replace(',', ".").parse::<f64>().map(Some).map_err(|_| wrong()),
        Some(_) => Err(wrong()),
    }
}

/// Paper thickness from `paper_mm` (per page, stored) or `paper_sheet_mm` (per sheet = 2 pages, Round 20c; stored
/// halved, no migration). Both together → `INVALID_PARAMS`.
fn paper_param(params: &Value) -> Result<Option<f64>, RpcError> {
    let page = mm_param(params, "paper_mm", &tc("cover_msg.param.paper", &[]))?;
    let sheet = mm_param(params, "paper_sheet_mm", &tc("cover_msg.param.paper_sheet", &[]))?;
    match (page, sheet) {
        (Some(_), Some(_)) => Err(invalid(tc("cover_msg.param.paper_both", &[]))),
        (None, Some(sheet)) if !(sheet.is_finite() && sheet > 0.0 && sheet <= 2.0) => {
            Err(invalid(tc("cover_msg.param.paper_sheet_range", &[])))
        }
        (None, Some(sheet)) => Ok(Some(sheet / 2.0)),
        (page, None) => Ok(page),
    }
}

/// `work.meta.set`: book details and (through `Store::update_work`) subtitle, author, Bible translation;
/// one commit „Buchangaben geändert: …“ if something changes.
async fn meta_set(state: &AppState, work: WorkId, params: &Value) -> Result<Value, RpcError> {
    let mut edit = BookEdit::default();
    for (name, slot) in [
        ("publisher", &mut edit.publisher),
        ("place", &mut edit.place),
        ("year", &mut edit.year),
        ("edition", &mut edit.edition),
        ("isbn", &mut edit.isbn),
        ("price", &mut edit.price),
        ("series", &mut edit.series),
        ("printer", &mut edit.printer),
        ("bible_notice", &mut edit.bible_notice),
        ("image_credit", &mut edit.image_credit),
        ("dedication", &mut edit.dedication),
        ("blurb", &mut edit.blurb),
        ("bio", &mut edit.bio),
        ("publisher_address", &mut edit.publisher_address),
        ("printer_address", &mut edit.printer_address),
    ] {
        *slot = text_param(params, name)?;
    }
    if let Some(isbn) = edit.isbn.as_deref().filter(|i| !i.trim().is_empty()) {
        // Detailed message (the store knows only fixed texts).
        tc::isbn13(isbn).map_err(invalid)?;
    }
    edit.author_photo = match params.get("author_photo") {
        None => None,
        Some(Value::Null) => Some(None),
        Some(Value::String(s)) => Some(Some(s.trim().to_owned()).filter(|s| !s.is_empty())),
        Some(_) => return Err(invalid(tc("cover_msg.param.author_photo", &[]))),
    };
    edit.title_area = match text_param(params, "title_area")? {
        None => None,
        Some(area) => Some(
            TitleArea::parse(&area)
                .ok_or_else(|| invalid(tc("cover_msg.param.title_area", &[("area", area.as_str().into())])))?,
        ),
    };
    edit.front_matter = bool_param(params, "front_matter")?;
    edit.cover_pages = bool_param(params, "cover_pages")?;
    edit.paper_mm = paper_param(params)?;
    edit.bleed_mm = mm_param(params, "bleed_mm", &tc("cover_msg.param.bleed", &[]))?;
    let subtitle = text_param(params, "subtitle")?;
    let author = text_param(params, "author")?;
    let bible = text_param(params, "bible")?;

    let held = state.write_lock.lock().await;
    let work_before = state.store.work_meta(work).await?;
    // First the book details (they check everything), then the work fields: so a rejected input changes nothing.
    let book = state.store.set_book_meta(work, &edit).await.map_err(input_error)?;
    let mut changed: Vec<&'static str> = Vec::new();
    if subtitle.is_some() || author.is_some() || bible.is_some() {
        let work_edit = WorkEdit {
            subtitle: subtitle.as_deref(),
            author: author.as_deref(),
            bible: bible.as_deref(),
            ..WorkEdit::default()
        };
        let update = state.store.update_work(work, &work_edit, HUMAN_NAME).await?;
        changed.extend(update.changed);
    }
    let work_after = state.store.work_meta(work).await?;
    let mut before = json!(book.before);
    let mut after = json!(book.after);
    for (name, b, a) in [
        ("subtitle", &work_before.subtitle, &work_after.subtitle),
        ("author", &work_before.author, &work_after.author),
        ("bible", &work_before.bible, &work_after.bible),
    ] {
        before[name] = json!(b);
        after[name] = json!(a);
    }
    changed.extend(book.changed.iter().copied());
    let mut answer = json!({ "work": work, "changed": changed });
    if !changed.is_empty() {
        let labels: Vec<&str> = changed.iter().map(|f| field_label(f)).collect();
        let summary = format!("Buchangaben geändert: {}", labels.join(", "));
        let body = change_body(&changed, &before, &after);
        add_mirror(&mut answer, commit(state, &held, work, summary, body, "book-meta").await);
    }
    Ok(answer)
}

/// `work.set_cover`: set or remove the cover, commit only on a change.
async fn set_cover(state: &AppState, work: WorkId, name: Option<&str>) -> Result<Value, RpcError> {
    let held = state.write_lock.lock().await;
    let change = state.store.set_work_cover(work, name).await?;
    let mut answer = json!({ "work": work, "cover": cover_info(state, work).await? });
    if change.before != change.after {
        add_mirror(&mut answer, commit_cover_change(state, &held, work, &change).await);
    }
    Ok(answer)
}

/// Body of a cover commit: „Umschlag vorher: … / Umschlag nachher: …“.
pub(crate) fn cover_change_body(change: &baruch_store::CoverChange) -> String {
    format!(
        "Umschlag vorher: {}\nUmschlag nachher: {}\n",
        change.before.as_deref().unwrap_or("–"),
        change.after.as_deref().unwrap_or("–")
    )
}

/// Commit for a cover change (subject „Umschlag: <name> gesetzt“ or „Umschlag entfernt“).
async fn commit_cover_change(
    state: &AppState,
    held: &WriteGuard<'_>,
    work: WorkId,
    change: &baruch_store::CoverChange,
) -> Result<Option<String>, String> {
    let (summary, action) = match &change.after {
        Some(name) => (format!("Umschlag: {name} gesetzt"), "cover-set"),
        None => ("Umschlag entfernt".to_owned(), "cover-removed"),
    };
    commit(state, held, work, summary, cover_change_body(change), action).await
}

/// Before deleting the whole image `name` (Round 20c): if it is the cover, it is first removed through
/// `Store::set_work_cover` and mirrored as „Umschlag entfernt“ – otherwise `ON DELETE SET NULL` would take the
/// cover away without a commit. `None` = the image was not the cover; otherwise the result of the commit. The
/// caller holds the write lock (as for the deletion itself).
pub(crate) async fn release_cover_before_delete(
    state: &AppState,
    held: &WriteGuard<'_>,
    work: WorkId,
    name: &str,
) -> Result<Option<Result<Option<String>, String>>, RpcError> {
    if state.store.work_cover(work).await?.as_deref() != Some(name) {
        return Ok(None);
    }
    let change = state.store.set_work_cover(work, None).await?;
    Ok(Some(commit_cover_change(state, held, work, &change).await))
}

/// `work.meta.get`: book details with title, subtitle, author, Bible translation and the cover.
async fn meta_get(state: &AppState, work: WorkId) -> Result<Value, RpcError> {
    let book = state.store.book_meta(work).await?;
    let w = state.store.work_meta(work).await?;
    let mut meta = json!(book);
    // Round 20c: thickness per sheet (= 2 pages), as print shops state it; stored stays `paper_mm` per page.
    meta["paper_sheet_mm"] = json!(book.paper_mm * 2.0);
    meta["title"] = json!(w.title);
    meta["subtitle"] = json!(w.subtitle);
    meta["author"] = json!(w.author);
    meta["bible"] = json!(w.bible);
    Ok(json!({ "work": work, "meta": meta, "cover": cover_info(state, work).await? }))
}

/// `cover.imprint_adopt` (Round 27c): append the missing front-matter functions of the imprint template
/// ([`tc::imprint_template_missing`]) to the work template – as a new template revision by the human through
/// `work.update` (lock and commit there). If the template already has all four functions: `REFUSED`, no change.
async fn imprint_adopt(state: &AppState, work: WorkId) -> Result<Value, RpcError> {
    let template = state.store.work_template(work).await?;
    let adopted = tc::imprint_template_missing(&template).map_err(|why| {
        RpcError::new(REFUSED, tc("cover_msg.imprint.not_adopted", &[("why", why.to_string().into())]))
    })?;
    if adopted == template {
        return Err(RpcError::new(REFUSED, tc("cover_msg.imprint.complete", &[])));
    }
    let updated = crate::works::call(state, "work.update", json!({ "work": work, "template": adopted })).await?;
    let mut answer = json!({ "work": work, "template_rev": updated["template_rev"] });
    for key in ["commit", "mirror"] {
        if let Some(value) = updated.get(key) {
            answer[key] = value.clone();
        }
    }
    Ok(answer)
}

/// RPC of this module (`docs/runde11/cover.md`).
pub async fn call(state: &AppState, work: WorkId, method: &str, params: Value) -> Result<Value, RpcError> {
    let params = if params.is_null() { json!({}) } else { params };
    match method {
        "work.set_cover" => {
            let name = match params.get("name") {
                Some(Value::Null) => None,
                Some(Value::String(s)) => Some(s.as_str()),
                Some(_) => return Err(invalid(tc("cover_msg.param.name", &[]))),
                None => return Err(invalid(tc("cover_msg.param.name_missing", &[]))),
            };
            set_cover(state, work, name).await
        }
        "work.meta.get" => meta_get(state, work).await,
        "work.meta.set" => meta_set(state, work, &params).await,
        "work.cover_pdf" => {
            let inline = params.get("inline").and_then(Value::as_bool).unwrap_or(false);
            let draft = crate::typeset::draft_param(&params)?;
            let (file_name, wrap, pdf) = cover_pdf(state, work, draft).await?;
            let pair = crate::typeset::draft_pair(draft);
            let url = if pair.is_empty() {
                format!("/werke/{work}/umschlag.pdf")
            } else {
                format!("/werke/{work}/umschlag.pdf?{pair}")
            };
            let mut answer = json!({
                "file_name": file_name, "pages": pdf.pages, "bytes": pdf.bytes.len(), "sha256": pdf.sha256,
                "url": url, "spine_mm": wrap.spine_mm,
                "draft": wrap.source.main.contains(baruch_typst::typeset::DRAFT_MARK),
            });
            if inline {
                answer["pdf_base64"] = json!(baruch_store::encode_base64(&pdf.bytes));
            }
            // Round 27c: print check of the cover (stored layout) and of the mandatory details.
            answer["checks"] = json!(crate::cover_print::cover_checks_with(state, work, None, draft).await?);
            Ok(answer)
        }
        "cover.imprint_adopt" => imprint_adopt(state, work).await,
        _ => Err(RpcError::new(METHOD_NOT_FOUND, format!("unknown method {method}"))),
    }
}

// ---- Templates -----------------------------------------------------------------------------------

fn templates() -> &'static Environment<'static> {
    static ENV: OnceLock<Environment<'static>> = OnceLock::new();
    ENV.get_or_init(|| {
        let mut env = Environment::new();
        crate::i18n::install(&mut env);
        for (name, source) in [
            ("base.html", include_str!("../templates/base.html")),
            ("cover.html", include_str!("../templates/cover.html")),
            ("fragments/cover_panel.html", include_str!("../templates/fragments/cover_panel.html")),
        ] {
            env.add_template(name, source).expect("Vorlage des Umschlags fehlerhaft");
        }
        env
    })
}

fn render_template(name: &str, ctx: Jinja) -> Result<String, String> {
    templates()
        .get_template(name)
        .and_then(|t| t.render(ctx))
        .map_err(|e| tc("cover_msg.page.template_error", &[("name", name.into()), ("error", e.to_string().into())]))
}

/// A rejected form: message and the entered values (field name → text).
#[derive(Debug, Clone, Default)]
pub struct Retry {
    pub error: String,
    pub values: BTreeMap<String, String>,
}

/// Form values of the book details: stored, after an error the entered ones.
fn book_values(meta: &BookMeta, retry: Option<&Retry>) -> BTreeMap<String, String> {
    let mut values = BTreeMap::new();
    let saved = json!(meta);
    if let Value::Object(map) = saved {
        for (key, value) in map {
            let text = match value {
                Value::String(s) => s,
                Value::Null => String::new(),
                Value::Bool(b) => if b { "1" } else { "" }.to_owned(),
                Value::Number(n) => n.to_string().replace('.', ","),
                other => other.to_string(),
            };
            values.insert(key, text);
        }
    }
    // Round 20c: the form asks for the thickness per sheet (= 2 pages).
    values.insert("paper_sheet_mm".into(), paper_text(meta.paper_mm * 2.0));
    if let Some(retry) = retry {
        if retry.values.contains_key("flags") {
            // Checkboxes are missing in the form when they are off.
            values.insert("front_matter".into(), String::new());
            values.insert("cover_pages".into(), String::new());
        }
        for (key, value) in &retry.values {
            values.insert(key.clone(), value.clone());
        }
    }
    values
}

/// Context of `fragments/cover_panel.html`.
async fn panel_context(state: &AppState, work: WorkId, back: &str, retry: Option<&Retry>) -> Result<Jinja, StoreError> {
    let meta = state.store.book_meta(work).await?;
    let assets = state.store.asset_list(work).await?;
    let images: Vec<Jinja> = assets
        .iter()
        .map(|a| context! { name => a.name.as_str(), caption => a.caption.as_str(), selected => meta.cover.as_deref() == Some(a.name.as_str()), photo => meta.author_photo.as_deref() == Some(a.name.as_str()) })
        .collect();
    let cover = match &meta.cover {
        Some(name) => Some(Jinja::from(Serde(info_of(state, work, name).await?))),
        None => None,
    };
    // Round 27c: front-matter functions the work template already has (button „Impressum-Vorlage übernehmen“).
    let template = state.store.work_template(work).await?;
    let functions = tc::template_functions(&template);
    let present: Vec<&str> = [
        (functions.half_title, "schmutztitel"),
        (functions.main_title, "haupttitel"),
        (functions.imprint, "impressum"),
        (functions.dedication, "widmung"),
    ]
    .into_iter()
    .filter_map(|(has, name)| has.then_some(name))
    .collect();
    Ok(context! {
        work => context! { id => work }, cover => cover, images => images,
        values => Jinja::from(Serde(book_values(&meta, retry))), area => meta.title_area.as_str(),
        back => back, error => retry.map(|r| r.error.clone()),
        imprint => context! { present => present.join(", "), complete => present.len() == 4 },
    })
}

/// HTML block `id="umschlag"` for the page „Bilder“: preview, selection, removal, title area,
/// link to the cover page.
pub async fn assets_panel(state: &AppState, work: WorkId) -> Result<String, RpcError> {
    let ctx = panel_context(state, work, &format!("/werke/{work}/bilder"), None).await?;
    render_template("fragments/cover_panel.html", context! { show_cover => true, ..ctx })
        .map_err(|e| RpcError::new(INTERNAL, e))
}

/// Groups „Buchangaben“ and „Umschlag“ of the settings page „Werk“ (finished HTML).
pub(crate) async fn settings_blocks(
    state: &AppState,
    work: WorkId,
    retry: Option<&Retry>,
) -> Result<(String, String), String> {
    let back = format!("/einstellungen/werk?work={work}");
    // The message stands at the group whose form was rejected (book details carry inputs).
    let (book_retry, cover_retry) = match retry {
        Some(r) if r.values.is_empty() => (None, Some(r)),
        other => (other, None),
    };
    let book_ctx = panel_context(state, work, &back, book_retry).await.map_err(|e| e.to_string())?;
    let cover_ctx = panel_context(state, work, &back, cover_retry).await.map_err(|e| e.to_string())?;
    let book = render_template("fragments/cover_panel.html", context! { show_book => true, ..book_ctx })?;
    let cover = render_template("fragments/cover_panel.html", context! { show_cover => true, ..cover_ctx })?;
    Ok((book, cover))
}

// ---- Pages ---------------------------------------------------------------------------------------

/// Reading pages: cover page (editor), preview as SVG, cover PDF.
pub fn pages() -> Router<Arc<AppState>> {
    Router::new()
        .route("/werke/{id}/umschlag", get(cover_page))
        .route("/werke/{id}/umschlag.svg", get(cover_svg))
        .route("/werke/{id}/umschlag.pdf", get(cover_pdf_download))
}

/// Forms (in the router behind `guard::same_origin`).
pub fn writes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/werke/umschlag/act", post(cover_act))
        .route("/werke/buchangaben/act", post(book_act))
        .route("/werke/umschlag/anordnung", post(layout_act))
        .route("/werke/umschlag/impressum", post(imprint_act))
}

fn page_settings(headers: &HeaderMap) -> Jinja {
    let settings = Settings::from_cookie_header(headers.get(header::COOKIE).and_then(|v| v.to_str().ok()));
    Jinja::from(Serde(settings))
}

/// Notices after a redirect (`hinweis=…` or `ok=…`).
fn notice(code: Option<&str>) -> Option<String> {
    match code? {
        "umschlag_gesetzt" => Some(tc("cover_msg.notice.cover_set", &[])),
        "umschlag_entfernt" => Some(tc("cover_msg.notice.cover_removed", &[])),
        "buchangaben" => Some(tc("cover_msg.notice.book_details", &[])),
        "anordnung" => Some(tc("cover_msg.notice.layout", &[])),
        "impressum" => Some(tc("cover_msg.notice.imprint", &[])),
        _ => None,
    }
}

/// What a cover page should show besides the work.
#[derive(Default)]
struct PageInput<'a> {
    /// Notice after a redirect.
    hint: Option<&'a str>,
    /// Rejected form of the groups „Umschlag“/„Buchangaben“.
    retry: Option<&'a Retry>,
    /// Fields `b.<kind>.<feld>` (preview or rejected input of the editor).
    fields: &'a [(String, String)],
    /// Message for the editor (rejected form „Anordnung“).
    editor_error: Option<String>,
}

async fn page_response(
    state: &AppState,
    headers: &HeaderMap,
    work: WorkId,
    status: StatusCode,
    input: PageInput<'_>,
) -> Response {
    let title = match state.store.work_meta(work).await {
        Ok(meta) => meta.title,
        Err(StoreError::UnknownWork(_)) => return (StatusCode::NOT_FOUND, unknown_work()).into_response(),
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, error_text(e)).into_response(),
    };
    let back = format!("/werke/{work}/umschlag");
    let panel = match panel_context(state, work, &back, input.retry).await {
        Ok(ctx) => {
            render_template("fragments/cover_panel.html", context! { show_cover => true, show_book => true, ..ctx })
        }
        Err(e) => Err(e.to_string()),
    };
    let panel = match panel {
        Ok(html) => html,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, error_text(e)).into_response(),
    };
    // Editor: layout with the preview from the fields (without storing).
    let fields: Vec<(String, String)> = input.fields.iter().filter(|(k, _)| k.starts_with("b.")).cloned().collect();
    let editor = editor_state(state, work, &fields, input.editor_error).await;
    let status =
        if editor.invalid_preview && status == StatusCode::OK { StatusCode::UNPROCESSABLE_ENTITY } else { status };
    // Preview: typeset the cover (cache of the typeset view by state), with the preview layout.
    let (preview, facts, diagnostics, print, preview_error) =
        match build_wrap_for(state, work, editor.shown.as_ref()).await {
            Ok(wrap) => {
                let stand = baruch_render::stand(&wrap.source);
                // Round 20c: spine = book pages / 2 sheets × thickness per sheet (cover pages in the book do not count).
                let paper_mm = state.store.book_meta(work).await.map_or(0.1, |m| m.paper_mm);
                let book_pages = wrap.pages.saturating_sub(wrap.cover_pages);
                let facts = context! {
                    width => format_mm(wrap.width_mm), height => format_mm(wrap.height_mm),
                    spine => format_mm(wrap.spine_mm), bleed => format_mm(wrap.bleed_mm),
                    page_width => format_mm(wrap.page_width_mm), page_height => format_mm(wrap.page_height_mm),
                    pages => wrap.pages, cover_pages => wrap.cover_pages, book_pages => book_pages,
                    sheets => ui_decimal(book_pages as f64 / 2.0, 2),
                    sheet_mm => ui_decimal(paper_mm * 2.0, 3),
                };
                match crate::typeset::compile(wrap.source, &stand, crate::typeset::COMPILE_TIMEOUT).await {
                    Ok((compiled, _)) => {
                        let diagnostics: Vec<Jinja> = compiled
                            .rendered
                            .diagnostics
                            .iter()
                            .map(|d| {
                                context! {
                                    label => if d.severity == baruch_render::Severity::Error {
                                        tc("cover.page.diagnostic_error", &[])
                                    } else {
                                        tc("cover.page.diagnostic_warning", &[])
                                    },
                                    message => d.message.as_str(), file => d.file.as_deref(), line => d.line,
                                }
                            })
                            .collect();
                        let image = (!compiled.rendered.pages.is_empty()).then(|| stand[..16].to_owned());
                        let print = print_warnings(&compiled.rendered.print);
                        (image, Some(facts), diagnostics, print, None)
                    }
                    Err(e) => (None, Some(facts), Vec::new(), Vec::new(), Some(e.message)),
                }
            }
            Err(e) => (None, None, Vec::new(), Vec::new(), Some(e.message)),
        };
    let fonts = font_names().await;
    let ctx = context! {
        settings => page_settings(headers), work => context! { id => work, title => title },
        notice => notice(input.hint), preview => preview, facts => facts, diagnostics => diagnostics,
        print_warnings => print, preview_error => preview_error,
        preview_query => editor.query.clone(), editor => editor.context(work, &back, &fonts),
        panel => panel,
    };
    match render_template("cover.html", ctx) {
        Ok(html) => (status, Html(html)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, error_text(e)).into_response(),
    }
}

/// „Fehler: …“ in the language of the request.
fn error_text(error: impl std::fmt::Display) -> String {
    tc("cover_msg.page.error", &[("error", error.to_string().into())])
}

/// „unbekanntes Werk“ in the language of the request.
fn unknown_work() -> String {
    tc("cover_msg.page.unknown_work", &[])
}

/// Print check of the images (resolution) for the cover page.
fn print_warnings(print: &[baruch_render::PrintWarning]) -> Vec<String> {
    print.iter().map(|w| w.message.clone()).collect()
}

/// „12,3 mm“ (German) or „12.3 mm“ (English): one decimal in the request language (Round 39b, `i18n::number`).
fn format_mm(value: f64) -> String {
    crate::i18n::number(crate::i18n::current(), value, 1)
}

/// A number with at most `decimals` places and no trailing zeros, in the request language through
/// `i18n::number` (Round 39b): sheets „12,5“ (German) or „12.5“ (English), „12“ for a whole number.
fn ui_decimal(value: f64, decimals: usize) -> String {
    let plain = format!("{value:.decimals$}");
    let needed = plain.split_once('.').map_or(0, |(_, fraction)| fraction.trim_end_matches('0').len());
    crate::i18n::number(crate::i18n::current(), value, needed)
}

/// Fields of a query or a form with notice (`hinweis` or `ok`).
fn hint_of(query: &[(String, String)]) -> Option<&str> {
    query.iter().rev().find(|(k, _)| k == "hinweis" || k == "ok").map(|(_, v)| v.as_str())
}

async fn cover_page(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<WorkId>,
    Query(query): Query<Vec<(String, String)>>,
) -> Response {
    if crate::guard::cross_site_get(&headers) {
        return crate::typeset::foreign_get();
    }
    let input = PageInput { hint: hint_of(&query), fields: &query, ..PageInput::default() };
    page_response(&state, &headers, id, StatusCode::OK, input).await
}

/// HTTP status for an error of the PDF generation (like `/werke/{id}/pdf`).
fn pdf_status(e: &RpcError) -> StatusCode {
    match e.code {
        INVALID_PARAMS => StatusCode::NOT_FOUND,
        REFUSED if crate::pdf::is_busy(&e.message) => StatusCode::CONFLICT,
        REFUSED if crate::typeset::is_timeout(&e.message) => StatusCode::SERVICE_UNAVAILABLE,
        REFUSED => StatusCode::UNPROCESSABLE_ENTITY,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

/// `GET /werke/{id}/umschlag.svg`: the cover as one SVG page (preview); with fields `b.<kind>.<feld>` the
/// preview of this layout (through [`build_wrap_for`], without storing).
async fn cover_svg(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<WorkId>,
    Query(query): Query<Vec<(String, String)>>,
) -> Response {
    if crate::guard::cross_site_get(&headers) {
        return crate::typeset::foreign_get();
    }
    let fields: Vec<(String, String)> = query.into_iter().filter(|(k, _)| k.starts_with("b.")).collect();
    let shown = if fields.is_empty() {
        None
    } else {
        match preview_layout(&state, id, &fields).await {
            Ok(layout) => Some(layout),
            Err(PreviewError::Invalid(message)) => return (StatusCode::UNPROCESSABLE_ENTITY, message).into_response(),
            Err(PreviewError::Rpc(e)) => return (pdf_status(&e), e.message).into_response(),
        }
    };
    let wrap = match build_wrap_for(&state, id, shown.as_ref()).await {
        Ok(wrap) => wrap,
        Err(e) => return (pdf_status(&e), e.message).into_response(),
    };
    let stand = baruch_render::stand(&wrap.source);
    let compiled = match crate::typeset::compile(wrap.source, &stand, crate::typeset::COMPILE_TIMEOUT).await {
        Ok((compiled, _)) => compiled,
        Err(e) => return (pdf_status(&e), e.message).into_response(),
    };
    let Some(page) = compiled.rendered.pages.first() else {
        return (StatusCode::UNPROCESSABLE_ENTITY, tc("cover_msg.page.not_typeset", &[])).into_response();
    };
    (
        [
            (header::CONTENT_TYPE, HeaderValue::from_static("image/svg+xml")),
            (header::X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff")),
            (header::CONTENT_SECURITY_POLICY, HeaderValue::from_static(crate::typeset::SVG_CSP)),
            (header::CACHE_CONTROL, HeaderValue::from_static("no-cache")),
        ],
        page.clone(),
    )
        .into_response()
}

// ---- Cover editor (Round 27c) --------------------------------------------------------------------

/// Fields of a block in the form and in the preview address (`b.<kind>.<feld>`), contract section 4.
pub const BLOCK_FIELDS: [&str; 13] = [
    "x_mm",
    "y_mm",
    "width_mm",
    "align",
    "size_pt",
    "leading_em",
    "color",
    "font",
    "style",
    "backdrop",
    "visible",
    "direction",
    "text",
];

/// Name of a block with face in the language of the request: „Titel (Vorderseite)“, „Title (Front)“ (Round 39b).
pub fn block_label(kind: CoverBlockKind) -> String {
    crate::cover_layout::kind_label_in(crate::i18n::current(), kind)
}

/// Name of a block field in the language of the request (messages, form).
pub fn block_field_label(field: &str) -> String {
    match field {
        "x_mm" => "x".to_owned(),
        "y_mm" => "y".to_owned(),
        "width_mm" => tc("cover_msg.block_field.width", &[]),
        "align" => tc("cover_msg.block_field.align", &[]),
        "size_pt" => tc("cover_msg.block_field.size", &[]),
        "leading_em" => tc("cover_msg.block_field.leading", &[]),
        "color" => tc("cover_msg.block_field.color", &[]),
        "font" => tc("cover_msg.block_field.font", &[]),
        "style" => tc("cover_msg.block_field.style", &[]),
        "backdrop" => tc("cover_msg.block_field.backdrop", &[]),
        "visible" => tc("cover_msg.block_field.visible", &[]),
        "direction" => tc("cover_msg.block_field.direction", &[]),
        "text" => tc("cover_msg.block_field.text", &[]),
        _ => tc("cover_msg.block_field.other", &[]),
    }
}

/// A decimal number of [`number_value`] with the decimal separator of the request language (de comma, en point).
fn local_decimal(value: f64) -> String {
    let text = number_value(value);
    if crate::i18n::current() == crate::i18n::Lang::De { text.replace('.', ",") } else { text }
}

/// Number from a field (dot or comma); finite values only.
fn number_field(raw: &str) -> Option<f64> {
    raw.trim().replace(',', ".").parse::<f64>().ok().filter(|v| v.is_finite())
}

/// Number for a number field (`type=number` demands the dot): at most two decimals, without trailing zeros.
/// Paper thickness for form and display: up to three decimals, comma, without trailing zeros („0,2“, „0,125“).
fn paper_text(value: f64) -> String {
    let text = format!("{value:.3}");
    text.trim_end_matches('0').trim_end_matches('.').replace('.', ",")
}

fn number_value(value: f64) -> String {
    let text = format!("{value:.2}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    if text == "-0" { "0".to_owned() } else { text.to_owned() }
}

/// Limit for messages: „-50 und 1000“.
fn bounds(range: (f64, f64)) -> String {
    tc("cover_msg.block.bounds", &[("low", local_decimal(range.0).into()), ("high", local_decimal(range.1).into())])
}

/// Apply a field to a block; errors as a German message with kind and field.
fn apply_block_field(block: &mut CoverBlock, field: &str, raw: &str) -> Result<(), String> {
    let wrong = |why: String| format!("{}, {}: {why}", block_label(block.kind), block_field_label(field));
    let ranged = |range: (f64, f64), unit: &str| -> Result<f64, String> {
        match number_field(raw) {
            Some(v) if v >= range.0 && v <= range.1 => Ok(v),
            _ => Err(wrong(tc(
                "cover_msg.block.number_range",
                &[("bounds", bounds(range).into()), ("unit", unit.into()), ("raw", raw.trim().into())],
            ))),
        }
    };
    match field {
        "x_mm" => block.x_mm = ranged(cl::POS_MM, "mm")?,
        "y_mm" => block.y_mm = ranged(cl::POS_MM, "mm")?,
        "width_mm" => block.width_mm = ranged(cl::WIDTH_MM, "mm")?,
        "size_pt" => block.size_pt = ranged(cl::SIZE_PT, "pt")?,
        "leading_em" => block.leading_em = ranged(cl::LEADING_EM, "em")?,
        "align" => {
            block.align = CoverAlign::parse(raw)
                .ok_or_else(|| wrong(tc("cover_msg.block.align", &[("raw", raw.trim().into())])))?
        }
        "style" => {
            block.style = FontStyle::parse(raw)
                .ok_or_else(|| wrong(tc("cover_msg.block.style", &[("raw", raw.trim().into())])))?
        }
        "backdrop" => {
            block.backdrop = Backdrop::parse(raw)
                .ok_or_else(|| wrong(tc("cover_msg.block.backdrop", &[("raw", raw.trim().into())])))?
        }
        "color" => {
            let color = raw.trim().to_ascii_lowercase();
            let hex = color.len() == 7 && color.starts_with('#') && color[1..].bytes().all(|b| b.is_ascii_hexdigit());
            if !(color == "light" || color == "dark" || hex) {
                return Err(wrong(tc("cover_msg.block.color", &[("raw", raw.trim().into())])));
            }
            block.color = color;
        }
        "font" => {
            let font = raw.trim();
            if font.is_empty() {
                block.font = None;
            } else {
                let allowed = font.chars().all(|c| c.is_alphanumeric() || matches!(c, ' ' | '-' | '_' | '.'));
                if !allowed || font.chars().count() > cl::MAX_FONT_CHARS {
                    return Err(wrong(tc("cover_msg.block.font", &[("max", cl::MAX_FONT_CHARS.into())])));
                }
                block.font = Some(font.to_owned());
            }
        }
        "visible" => {
            block.visible = match raw.trim() {
                "1" | "on" | "true" | "ja" => true,
                "" | "0" | "off" | "false" | "nein" => false,
                other => return Err(wrong(tc("cover_msg.block.switch", &[("raw", other.into())]))),
            }
        }
        "direction" => {
            if block.kind.face() == CoverFace::Spine {
                block.direction = Some(
                    SpineDirection::parse(raw)
                        .ok_or_else(|| wrong(tc("cover_msg.block.direction", &[("raw", raw.trim().into())])))?,
                );
            } else if !raw.trim().is_empty() {
                return Err(wrong(tc("cover_msg.block.spine_only", &[])));
            }
        }
        "text" => {
            let text = raw.replace("\r\n", "\n");
            if block.kind.own_text() {
                if text.chars().count() > cl::MAX_TEXT_CHARS {
                    return Err(wrong(tc("cover_msg.block.text_too_long", &[("max", cl::MAX_TEXT_CHARS.into())])));
                }
                block.text = Some(text);
            } else if !text.trim().is_empty() {
                return Err(wrong(tc("cover_msg.block.text_from_work", &[])));
            }
        }
        other => {
            return Err(tc(
                "cover_msg.block.unknown_field",
                &[("block", block_label(block.kind).into()), ("field", other.into())],
            ));
        }
    }
    Ok(())
}

/// Blocks from fields `b.<kind>.<feld>`: every kind with at least one field, starting from its block in `base`
/// (stored or default); a field named several times counts last. Checked as in the store
/// ([`baruch_store::check_cover_block`]).
pub fn blocks_from_fields(fields: &[(String, String)], base: &[CoverBlock]) -> Result<Vec<CoverBlock>, String> {
    for (name, _) in fields {
        if let Some(rest) = name.strip_prefix("b.") {
            let kind = rest.split_once('.').map_or(rest, |(kind, _)| kind);
            if CoverBlockKind::parse(kind).is_none() {
                return Err(tc(
                    "cover_msg.block.unknown_kind",
                    &[("kind", kind.into()), ("name", name.as_str().into())],
                ));
            }
        }
    }
    let mut out = Vec::new();
    for kind in CoverBlockKind::ALL {
        let prefix = format!("b.{}.", kind.as_str());
        let given: Vec<(&str, &str)> =
            fields.iter().filter_map(|(k, v)| k.strip_prefix(&prefix).map(|f| (f, v.as_str()))).collect();
        if given.is_empty() {
            continue;
        }
        let mut block = base
            .iter()
            .find(|b| b.kind == kind)
            .cloned()
            .ok_or_else(|| tc("cover_msg.block.no_base", &[("block", block_label(kind).into())]))?;
        for (field, raw) in given {
            apply_block_field(&mut block, field, raw)?;
        }
        let block = baruch_store::check_cover_block(&block).map_err(|e| format!("{}: {e}", block_label(kind)))?;
        out.push(block);
    }
    Ok(out)
}

/// Values of a block for the form (field → text).
fn block_values(block: &CoverBlock) -> BTreeMap<&'static str, String> {
    BTreeMap::from([
        ("x_mm", number_value(block.x_mm)),
        ("y_mm", number_value(block.y_mm)),
        ("width_mm", number_value(block.width_mm)),
        ("align", block.align.as_str().to_owned()),
        ("size_pt", number_value(block.size_pt)),
        ("leading_em", number_value(block.leading_em)),
        ("color", block.color.clone()),
        ("font", block.font.clone().unwrap_or_default()),
        ("style", block.style.as_str().to_owned()),
        ("backdrop", block.backdrop.as_str().to_owned()),
        ("visible", if block.visible { "1" } else { "0" }.to_owned()),
        ("direction", block.direction.map_or("", |d| d.as_str()).to_owned()),
        ("text", block.text.clone().unwrap_or_default()),
    ])
}

/// Stored layout, replaced per kind by the preview (as in `cover_layout::layout_view`).
fn merged_layout(stored: &CoverLayout, preview: &CoverLayout) -> CoverLayout {
    let mut merged = stored.clone();
    for block in &preview.blocks {
        merged.blocks.retain(|b| b.kind != block.kind);
        merged.blocks.push(block.clone());
    }
    merged
}

enum PreviewError {
    Invalid(String),
    Rpc(RpcError),
}

/// Layout for a preview from fields: stored layout, the named kinds from the fields (starting
/// from the effective block). For the SVG preview.
async fn preview_layout(
    state: &AppState,
    work: WorkId,
    fields: &[(String, String)],
) -> Result<CoverLayout, PreviewError> {
    let stored = state.store.cover_layout(work).await.map_err(|e| PreviewError::Rpc(e.into()))?;
    let wrap = build_wrap_for(state, work, None).await.map_err(PreviewError::Rpc)?;
    let meta = state.store.book_meta(work).await.map_err(|e| PreviewError::Rpc(e.into()))?;
    let base: Vec<CoverBlock> = cl::effective_blocks(&stored, title_area(meta.title_area), &wrap.geometry)
        .into_iter()
        .map(|(b, _)| b)
        .collect();
    let blocks = blocks_from_fields(fields, &base).map_err(PreviewError::Invalid)?;
    Ok(merged_layout(&stored, &CoverLayout { blocks }))
}

/// Font families for the selection list (blocking in `spawn_blocking`); empty on an error.
async fn font_names() -> Vec<String> {
    tokio::task::spawn_blocking(baruch_render::font_families)
        .await
        .map(|families| families.into_iter().map(|f| f.name).collect())
        .unwrap_or_default()
}

/// State of the editor for a page.
struct EditorState {
    /// View from `cover.layout` (with preview), or the message why there is none.
    view: Result<crate::cover_layout::LayoutView, String>,
    /// Layout for typesetting the preview (stored + preview), `None` = stored.
    shown: Option<CoverLayout>,
    /// Entered values per field name (after an error or in the preview).
    raw: BTreeMap<String, String>,
    /// Query for the SVG preview (`b.…=…&…`), empty without preview.
    query: String,
    error: Option<String>,
    invalid_preview: bool,
    preview: bool,
}

/// Load the view; with fields the preview on top (without storing). Invalid fields: message, input stays.
async fn editor_state(
    state: &AppState,
    work: WorkId,
    fields: &[(String, String)],
    error: Option<String>,
) -> EditorState {
    let raw: BTreeMap<String, String> = fields.iter().cloned().collect();
    let mut out = EditorState {
        view: Err(String::new()),
        shown: None,
        raw,
        query: String::new(),
        invalid_preview: false,
        preview: false,
        error,
    };
    let base = match crate::cover_layout::layout_view(state, work, None).await {
        Ok(view) => view,
        Err(e) => {
            out.view = Err(e.message);
            return out;
        }
    };
    if fields.is_empty() {
        out.view = Ok(base);
        return out;
    }
    let base_blocks: Vec<CoverBlock> = base.blocks.iter().map(|b| b.block.clone()).collect();
    match blocks_from_fields(fields, &base_blocks) {
        Ok(blocks) => {
            let preview = CoverLayout { blocks };
            match crate::cover_layout::layout_view(state, work, Some(&preview)).await {
                Ok(view) => {
                    let stored = state.store.cover_layout(work).await.unwrap_or_default();
                    out.shown = Some(merged_layout(&stored, &preview));
                    out.query = fields
                        .iter()
                        .map(|(k, v)| format!("{}={}", query_escape(k), query_escape(v)))
                        .collect::<Vec<_>>()
                        .join("&");
                    out.preview = out.error.is_none();
                    out.view = Ok(view);
                }
                Err(e) => {
                    out.error.get_or_insert(e.message);
                    out.view = Ok(base);
                }
            }
        }
        Err(message) => {
            out.invalid_preview = out.error.is_none();
            out.error.get_or_insert(message);
            out.view = Ok(base);
        }
    }
    out
}

/// Percent-encoding for a query (everything except `A-Za-z0-9-_.~`).
fn query_escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for b in text.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// Percent of a length (for `left`/`top`/`width`/`height`), with dot.
fn percent(part: f64, whole: f64) -> String {
    if whole <= 0.0 { "0%".to_owned() } else { format!("{:.4}%", part / whole * 100.0) }
}

/// Position in the whole cover (mm): left, top, width, height of the frame of a block. On the spine the text
/// runs rotated: `width_mm` is the vertical, the height the horizontal extent (contract 1.2).
pub fn block_box(block: &CoverBlock, height_mm: f64, geometry: &CoverGeometry) -> (f64, f64, f64, f64) {
    let face = block.kind.face();
    let (ox, oy) = geometry.face_origin(face);
    if face == CoverFace::Spine {
        (ox + block.x_mm, oy + block.y_mm, height_mm, block.width_mm)
    } else {
        (ox + block.x_mm, oy + block.y_mm, block.width_mm, height_mm)
    }
}

/// Guides as rectangles in the whole cover (mm): (kind, label, left, top, width, height); a line has
/// width or height 0. Kinds: `bleed` (outer edge of the bleed), `trim` (trim edge), `safety`
/// (safety margin per face), `spine` (spine edges), `center` (centre of the pages and of the height).
pub fn guides(geometry: &CoverGeometry) -> Vec<(&'static str, String, f64, f64, f64, f64)> {
    let (wrap_w, wrap_h) = geometry.wrap_size();
    let (b, w, h, s) = (geometry.bleed_mm, geometry.page_width_mm, geometry.page_height_mm, geometry.spine_mm);
    let safety = geometry.safety_mm;
    let mut out = vec![
        ("bleed", tc("cover_msg.guide.bleed", &[]), 0.0, 0.0, wrap_w, wrap_h),
        ("trim", tc("cover_msg.guide.trim", &[]), b, b, 2.0 * w + s, h),
    ];
    for face in [CoverFace::Back, CoverFace::Spine, CoverFace::Front] {
        let (ox, oy) = geometry.face_origin(face);
        let (fw, fh) = geometry.face_size(face);
        if fw > 2.0 * safety && fh > 2.0 * safety {
            out.push((
                "safety",
                tc(
                    "cover_msg.guide.safety",
                    &[
                        ("mm", local_decimal(safety).into()),
                        ("face", crate::cover_layout::face_label_in(crate::i18n::current(), face).into()),
                    ],
                ),
                ox + safety,
                oy + safety,
                fw - 2.0 * safety,
                fh - 2.0 * safety,
            ));
        }
    }
    out.push(("spine", tc("cover_msg.guide.spine_left", &[]), b + w, b, 0.0, h));
    out.push(("spine", tc("cover_msg.guide.spine_right", &[]), b + w + s, b, 0.0, h));
    out.push(("center", tc("cover_msg.guide.middle_back", &[]), b + w / 2.0, b, 0.0, h));
    out.push(("center", tc("cover_msg.guide.middle_front", &[]), b + w + s + w / 2.0, b, 0.0, h));
    out.push(("center", tc("cover_msg.guide.middle_height", &[]), b, b + h / 2.0, 2.0 * w + s, 0.0));
    out
}

impl EditorState {
    /// Context `editor` of `cover.html`.
    fn context(&self, work: WorkId, back: &str, fonts: &[String]) -> Jinja {
        let view = match &self.view {
            Ok(view) => view,
            Err(message) => {
                return context! { available => false, reason => message.as_str(), error => self.error.as_deref() };
            }
        };
        let geometry = view.geometry;
        let (wrap_w, wrap_h) = geometry.wrap_size();
        let blocks: Vec<Jinja> = view
            .blocks
            .iter()
            .map(|vb| {
                let block = &vb.block;
                let kind = block.kind;
                let (left, top, width, height) = block_box(block, vb.height_mm, &geometry);
                let mut values: BTreeMap<String, String> =
                    block_values(block).into_iter().map(|(k, v)| (k.to_owned(), v)).collect();
                for field in BLOCK_FIELDS {
                    if let Some(raw) = self.raw.get(&format!("b.{}.{field}", kind.as_str())) {
                        // Number fields (`type=number`) need the dot; unreadable input stays as it was entered.
                        let numeric = matches!(field, "x_mm" | "y_mm" | "width_mm" | "size_pt" | "leading_em");
                        let shown = match number_field(raw) {
                            Some(value) if numeric => number_value(value),
                            _ => raw.clone(),
                        };
                        values.insert(field.to_owned(), shown);
                    }
                }
                let label = block_label(kind);
                let args = [
                    ("label", label.as_str().into()),
                    ("x", format_mm(block.x_mm).into()),
                    ("y", format_mm(block.y_mm).into()),
                    ("width", format_mm(block.width_mm).into()),
                ];
                let aria = if block.visible {
                    tc("cover_msg.editor.block_aria", &args)
                } else {
                    tc("cover_msg.editor.block_aria_hidden", &args)
                };
                context! {
                    kind => kind.as_str(), label => label,
                    name => crate::cover_layout::block_name_in(crate::i18n::current(), kind),
                    face => kind.face().as_str(),
                    face_label => crate::cover_layout::face_label_in(crate::i18n::current(), kind.face()),
                    aria => aria, stored => vb.stored,
                    own_text => kind.own_text(), spine => kind.face() == CoverFace::Spine, visible => block.visible,
                    left => percent(left, wrap_w), top => percent(top, wrap_h),
                    width => percent(width, wrap_w), height => percent(height, wrap_h),
                    values => Jinja::from(Serde(values)),
                }
            })
            .collect();
        let guides: Vec<Jinja> = guides(&geometry)
            .into_iter()
            .map(|(kind, label, left, top, width, height)| {
                context! {
                    kind => kind, label => label, left => percent(left, wrap_w), top => percent(top, wrap_h),
                    width => percent(width, wrap_w), height => percent(height, wrap_h),
                }
            })
            .collect();
        let warnings: Vec<Jinja> = view
            .warnings
            .iter()
            .map(|w| {
                context! {
                    code => w.code, block => w.block.map(block_label), kind => w.block.map(CoverBlockKind::as_str),
                    message => w.message.as_str(),
                }
            })
            .collect();
        let faces: Vec<Jinja> = CoverFace::ALL
            .into_iter()
            .map(|f| {
                context! { name => f.as_str(), label => crate::cover_layout::face_label_in(crate::i18n::current(), f) }
            })
            .collect();
        context! {
            available => true, error => self.error.as_deref(), preview => self.preview,
            layout_json => serde_json::to_string(view).unwrap_or_default(),
            stored => view.stored, area => view.title_area.as_str(), default_font => view.default_font.as_str(),
            ratio => format!("{} / {}", number_value(wrap_w), number_value(wrap_h)),
            blocks => blocks, guides => guides, warnings => warnings, faces => faces, fonts => fonts.to_vec(),
            work => work, back => back,
        }
    }
}

/// `GET /werke/{id}/umschlag.pdf`: cover as PDF for download; errors as HTML without PDF.
async fn cover_pdf_download(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<WorkId>,
    Query(query): Query<Vec<(String, String)>>,
) -> Response {
    if crate::guard::cross_site_get(&headers) {
        return crate::typeset::foreign_get();
    }
    // Round 22: `?entwurf=1|0` (three-valued, does not change the setting).
    let entwurf = query.iter().rev().find(|(k, _)| k == "entwurf").map(|(_, v)| v.as_str());
    let draft = match crate::typeset::draft_query(entwurf) {
        Ok(draft) => draft,
        Err(message) => return (StatusCode::BAD_REQUEST, message).into_response(),
    };
    match cover_pdf(&state, id, draft).await {
        Ok((file_name, _, pdf)) => {
            let disposition = HeaderValue::from_str(&format!("attachment; filename=\"{file_name}\""))
                .unwrap_or_else(|_| HeaderValue::from_static("attachment"));
            (
                [
                    (header::CONTENT_TYPE, HeaderValue::from_static("application/pdf")),
                    (header::CONTENT_DISPOSITION, disposition),
                    (header::X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff")),
                    (header::CACHE_CONTROL, HeaderValue::from_static("no-cache")),
                ],
                pdf.bytes,
            )
                .into_response()
        }
        Err(e) => {
            let status = pdf_status(&e);
            let retry = Retry {
                error: tc("cover_msg.page.no_pdf", &[("error", e.message.as_str().into())]),
                values: BTreeMap::new(),
            };
            if status == StatusCode::NOT_FOUND {
                return (status, unknown_work()).into_response();
            }
            page_response(&state, &headers, id, status, PageInput { retry: Some(&retry), ..PageInput::default() }).await
        }
    }
}

// ---- Forms ---------------------------------------------------------------------------------------

/// `back` only as a relative, safe address (`/…`, not `//`, only `[A-Za-z0-9/_.=&%?#-]`).
pub(crate) fn safe_back(back: Option<&str>) -> Option<String> {
    let back = back?.trim();
    let ok = back.starts_with('/')
        && !back.starts_with("//")
        && back.bytes().all(|b| b.is_ascii_alphanumeric() || b"/_.=&%?#-".contains(&b));
    ok.then(|| back.to_owned())
}

/// Target after success: settings → `ok=<settings_code>`, otherwise `hinweis=<hint>` (before a `#…`).
fn success_target(back: &str, settings_code: &str, hint: &str) -> String {
    if back.starts_with("/einstellungen/") {
        return crate::works::with_ok(back, settings_code);
    }
    let (path, fragment) = match back.split_once('#') {
        Some((path, fragment)) => (path, Some(fragment)),
        None => (back, None),
    };
    let sep = if path.contains('?') { '&' } else { '?' };
    match fragment {
        Some(fragment) => format!("{path}{sep}hinweis={hint}#{fragment}"),
        None => format!("{path}{sep}hinweis={hint}"),
    }
}

fn form_status(e: &RpcError) -> StatusCode {
    match e.code {
        INVALID_PARAMS => StatusCode::UNPROCESSABLE_ENTITY,
        REFUSED => StatusCode::CONFLICT,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

/// Rejected form: back to the settings page „Werk“ or the cover page, with message
/// and input.
async fn failed(
    state: &AppState,
    headers: &HeaderMap,
    work: WorkId,
    back: &str,
    status: StatusCode,
    retry: Retry,
) -> Response {
    if back.starts_with("/einstellungen/") {
        return crate::settings::work_retry_page(state, headers, status, work, crate::settings::WorkRetry::Book(retry))
            .await;
    }
    page_response(state, headers, work, status, PageInput { retry: Some(&retry), ..PageInput::default() }).await
}

/// Work and `back` from a form.
fn form_work(fields: &BTreeMap<String, String>) -> Option<WorkId> {
    fields.get("work").and_then(|w| w.trim().parse().ok())
}

/// `POST /werke/umschlag/act`: `action=set` (field `name`), `remove`, `area` (field `title_area`).
async fn cover_act(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Form(pairs): Form<Vec<(String, String)>>,
) -> Response {
    let fields: BTreeMap<String, String> = pairs.into_iter().collect();
    let Some(work) = form_work(&fields) else {
        return (StatusCode::UNPROCESSABLE_ENTITY, tc("cover_msg.form.work_missing", &[])).into_response();
    };
    let back =
        safe_back(fields.get("back").map(String::as_str)).unwrap_or_else(|| format!("/einstellungen/werk?work={work}"));
    let action = fields.get("action").map(String::as_str).unwrap_or("");
    let result = match action {
        "set" => {
            let name = fields.get("name").map(|n| n.trim()).unwrap_or("");
            if name.is_empty() {
                Err(invalid(tc("cover_msg.form.choose_image", &[])))
            } else {
                set_cover(&state, work, Some(name)).await.map(|_| "umschlag_gesetzt")
            }
        }
        "remove" => set_cover(&state, work, None).await.map(|_| "umschlag_entfernt"),
        "area" => {
            let area = fields.get("title_area").cloned().unwrap_or_default();
            meta_set(&state, work, &json!({ "title_area": area })).await.map(|_| "buchangaben")
        }
        _ => return (StatusCode::BAD_REQUEST, tc("cover_msg.form.unknown_action", &[])).into_response(),
    };
    match result {
        Ok(hint) => Redirect::to(&success_target(&back, "umschlag", hint)).into_response(),
        Err(e) if e.code == INTERNAL => (StatusCode::INTERNAL_SERVER_ERROR, error_text(&e.message)).into_response(),
        Err(e) => {
            let status = form_status(&e);
            failed(&state, &headers, work, &back, status, Retry { error: e.message, values: BTreeMap::new() }).await
        }
    }
}

/// Field names of the form „Buchangaben“ (checkboxes and `flags` separately).
const BOOK_FIELDS: [&str; 22] = [
    "publisher",
    "place",
    "year",
    "edition",
    "isbn",
    "price",
    "series",
    "printer",
    "bible_notice",
    "image_credit",
    "dedication",
    "blurb",
    "bio",
    "publisher_address",
    "printer_address",
    "author_photo",
    "title_area",
    "paper_mm",
    "paper_sheet_mm",
    "bleed_mm",
    "subtitle",
    "author",
];

/// `POST /werke/buchangaben/act`: all book details; checkboxes `front_matter`, `cover_pages` (with
/// `flags=1`: missing = off), `paper_mm`/`bleed_mm` with comma or dot.
async fn book_act(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Form(pairs): Form<Vec<(String, String)>>,
) -> Response {
    let fields: BTreeMap<String, String> = pairs.into_iter().collect();
    let Some(work) = form_work(&fields) else {
        return (StatusCode::UNPROCESSABLE_ENTITY, tc("cover_msg.form.work_missing", &[])).into_response();
    };
    let back =
        safe_back(fields.get("back").map(String::as_str)).unwrap_or_else(|| format!("/einstellungen/werk?work={work}"));
    let mut params = json!({});
    for name in BOOK_FIELDS.iter().chain(["bible"].iter()) {
        if let Some(value) = fields.get(*name) {
            params[*name] = json!(value);
        }
    }
    if params.get("author_photo").and_then(Value::as_str).is_some_and(|p| p.trim().is_empty()) {
        params["author_photo"] = Value::Null;
    }
    let flags = fields.contains_key("flags");
    for name in ["front_matter", "cover_pages"] {
        match fields.get(name) {
            Some(value) => params[name] = json!(value),
            None if flags => params[name] = json!(false),
            None => {}
        }
    }
    match meta_set(&state, work, &params).await {
        Ok(_) => Redirect::to(&success_target(&back, "buchangaben", "buchangaben")).into_response(),
        Err(e) if e.code == INTERNAL => (StatusCode::INTERNAL_SERVER_ERROR, error_text(&e.message)).into_response(),
        Err(e) => {
            let mut values: BTreeMap<String, String> = fields
                .iter()
                .filter(|(k, _)| !matches!(k.as_str(), "work" | "back"))
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            for name in ["front_matter", "cover_pages"] {
                if let Some(v) = values.get_mut(name) {
                    *v = "1".into();
                }
            }
            let status = form_status(&e);
            failed(&state, &headers, work, &back, status, Retry { error: e.message, values }).await
        }
    }
}

/// `back` with `ok=<code>` (before a `#…`), for the forms of the editor and of the imprint template.
fn with_code(back: &str, code: &str) -> String {
    let (path, fragment) = match back.split_once('#') {
        Some((path, fragment)) => (path, Some(fragment)),
        None => (back, None),
    };
    let sep = if path.contains('?') { '&' } else { '?' };
    match fragment {
        Some(fragment) => format!("{path}{sep}ok={code}#{fragment}"),
        None => format!("{path}{sep}ok={code}"),
    }
}

/// `POST /werke/umschlag/anordnung` (Round 27c, behind `guard::same_origin`): fields `work`, `back`,
/// `action=save|reset|quick`; `save` passes all blocks from the fields `b.<kind>.<feld>` to
/// [`crate::cover_layout::layout_set`], `quick` sets the title area with `area` (top/centre/bottom) and deletes the
/// rows of the front, `reset` deletes the rows of the faces `faces` (repeated; none = all) – both through
/// [`crate::cover_layout::layout_reset`]. Success → 303 to `back` with `ok=anordnung`; invalid → 422 with input and
/// message on the cover page.
async fn layout_act(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Form(pairs): Form<Vec<(String, String)>>,
) -> Response {
    let fields: BTreeMap<String, String> = pairs.iter().cloned().collect();
    let Some(work) = form_work(&fields) else {
        return (StatusCode::UNPROCESSABLE_ENTITY, tc("cover_msg.form.work_missing", &[])).into_response();
    };
    let back = safe_back(fields.get("back").map(String::as_str)).unwrap_or_else(|| format!("/werke/{work}/umschlag"));
    let action = fields.get("action").map(|a| a.trim()).unwrap_or("save");
    let result = match action {
        "save" => match crate::cover_layout::layout_view(&state, work, None).await {
            Ok(base) => {
                let base: Vec<CoverBlock> = base.blocks.into_iter().map(|b| b.block).collect();
                match blocks_from_fields(&pairs, &base) {
                    Ok(blocks) if blocks.is_empty() => {
                        Err(invalid(tc("cover_msg.form.no_blocks", &[("lt", "<".into()), ("gt", ">".into())])))
                    }
                    Ok(blocks) => crate::cover_layout::layout_set(&state, work, blocks).await,
                    Err(message) => Err(invalid(message)),
                }
            }
            Err(e) => Err(e),
        },
        "quick" => {
            let area = fields.get("area").map(String::as_str).unwrap_or("");
            match TitleArea::parse(area) {
                Some(area) => crate::cover_layout::layout_reset(&state, work, &[CoverFace::Front], Some(area)).await,
                None => Err(invalid(tc("cover_msg.form.quick_area", &[("area", area.into())]))),
            }
        }
        "reset" => {
            let mut faces = Vec::new();
            let mut wrong = None;
            for (_, value) in pairs.iter().filter(|(k, _)| k == "faces") {
                match CoverFace::parse(value) {
                    Some(face) if !faces.contains(&face) => faces.push(face),
                    Some(_) => {}
                    None => wrong = Some(value.clone()),
                }
            }
            match wrong {
                Some(word) => Err(invalid(tc("cover_msg.form.faces", &[("word", word.into())]))),
                None => crate::cover_layout::layout_reset(&state, work, &faces, None).await,
            }
        }
        _ => return (StatusCode::BAD_REQUEST, tc("cover_msg.form.unknown_action", &[])).into_response(),
    };
    match result {
        Ok(_) => Redirect::to(&with_code(&back, "anordnung")).into_response(),
        Err(e) if e.code == INTERNAL => (StatusCode::INTERNAL_SERVER_ERROR, error_text(&e.message)).into_response(),
        Err(e) => {
            let status = form_status(&e);
            // The input stays (only when saving; quick choice and reset have no values).
            let kept: &[(String, String)] = if action == "save" { &pairs } else { &[] };
            let input = PageInput { fields: kept, editor_error: Some(e.message), ..PageInput::default() };
            page_response(&state, &headers, work, status, input).await
        }
    }
}

/// `POST /werke/umschlag/impressum` (Round 27c, behind `guard::same_origin`): fields `work`, `back`; appends the
/// missing front-matter functions of the imprint template to the work template (`cover.imprint_adopt`). Success →
/// 303 to `back` with `ok=impressum`; refusal (everything already there) → 422 with message, nothing changed.
async fn imprint_act(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Form(pairs): Form<Vec<(String, String)>>,
) -> Response {
    let fields: BTreeMap<String, String> = pairs.into_iter().collect();
    let Some(work) = form_work(&fields) else {
        return (StatusCode::UNPROCESSABLE_ENTITY, tc("cover_msg.form.work_missing", &[])).into_response();
    };
    let back = safe_back(fields.get("back").map(String::as_str)).unwrap_or_else(|| format!("/werke/{work}/umschlag"));
    match imprint_adopt(&state, work).await {
        Ok(_) => Redirect::to(&with_code(&back, "impressum")).into_response(),
        Err(e) if e.code == INTERNAL => (StatusCode::INTERNAL_SERVER_ERROR, error_text(&e.message)).into_response(),
        Err(e) => {
            let retry = Retry { error: e.message, values: BTreeMap::new() };
            failed(&state, &headers, work, &back, StatusCode::UNPROCESSABLE_ENTITY, retry).await
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(kind: CoverBlockKind) -> CoverBlock {
        let g = CoverGeometry::new(148.0, 210.0, 12.0, 3.0);
        cl::default_blocks(tc::TitleArea::Top, &g).into_iter().find(|b| b.kind == kind).unwrap()
    }

    fn pairs(list: &[(&str, &str)]) -> Vec<(String, String)> {
        list.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    #[test]
    fn sheets_and_millimetres_follow_the_request_language() {
        use crate::i18n::{Lang, sync_scope};
        sync_scope(Lang::De, || {
            assert_eq!(ui_decimal(12.5, 2), "12,5", "German sheets keep the decimal comma (as before)");
            assert_eq!(ui_decimal(12.0, 2), "12", "a whole number has no decimals");
            assert_eq!(ui_decimal(0.25, 3), "0,25", "paper per sheet without trailing zeros");
            assert_eq!(format_mm(12.34), "12,3", "German millimetres with one decimal and a comma");
        });
        sync_scope(Lang::En, || {
            assert_eq!(ui_decimal(12.5, 2), "12.5", "English sheets with a decimal point (finding R39a: was 12,5)");
            assert_eq!(format_mm(12.34), "12.3", "English millimetres with a point");
        });
    }

    #[test]
    fn fields_override_only_their_kind_and_accept_a_comma() {
        let base = vec![sample(CoverBlockKind::FrontTitle), sample(CoverBlockKind::SpineTitle)];
        let fields = pairs(&[
            ("work", "3"),
            ("b.front_title.y_mm", "40,5"),
            ("b.front_title.color", "#A0B0C0"),
            ("b.front_title.font", ""),
            ("b.spine_title.direction", "up"),
        ]);
        let blocks = blocks_from_fields(&fields, &base).unwrap();
        assert_eq!(blocks.len(), 2, "nur die genannten Arten: {blocks:?}");
        assert_eq!(blocks[0].y_mm, 40.5, "Komma als Dezimalzeichen");
        assert_eq!(blocks[0].x_mm, base[0].x_mm, "ungenannte Felder bleiben vom Ausgangsblock");
        assert_eq!(blocks[0].color, "#a0b0c0", "Farbe in Kleinbuchstaben");
        assert_eq!(blocks[0].font, None, "leere Schrift = Schrift der Werkvorlage");
        assert_eq!(blocks[1].direction, Some(SpineDirection::Up), "Laufrichtung auf dem Rücken");
    }

    #[test]
    fn wrong_fields_name_kind_and_field() {
        crate::i18n::sync_scope(crate::i18n::Lang::De, || {
            let base = vec![sample(CoverBlockKind::FrontTitle), sample(CoverBlockKind::BackBlurb)];
            let cases = [
                (("b.front_title.x_mm", "1001"), "Titel (Vorderseite), x"),
                (("b.front_title.size_pt", "drei"), "Titel (Vorderseite), Schriftgröße"),
                (("b.front_title.direction", "up"), "nur auf dem Rücken"),
                (("b.back_blurb.text", "eigener Text"), "Klappentext (Rückseite), Text"),
                (("b.front_title.font", "Schrift;rot"), "Titel (Vorderseite), Schrift"),
                (("b.front_title.color", "rot"), "Titel (Vorderseite), Farbe"),
                (("b.front_title.unsinn", "1"), "unbekanntes Feld"),
                (("b.rueckenwind.x_mm", "1"), "Unbekannte Blockart"),
            ];
            for ((name, value), needle) in cases {
                let error = blocks_from_fields(&pairs(&[(name, value)]), &base).unwrap_err();
                assert!(error.contains(needle), "{name}={value}: Meldung nennt „{needle}“: {error}");
            }
        })
    }

    #[test]
    fn a_spine_frame_stands_upright_on_the_page() {
        let g = CoverGeometry::new(148.0, 210.0, 12.0, 3.0);
        let mut spine = sample(CoverBlockKind::SpineTitle);
        spine.x_mm = 2.0;
        spine.y_mm = 20.0;
        spine.width_mm = 150.0;
        let (left, top, width, height) = block_box(&spine, 6.0, &g);
        assert_eq!((left, top), (3.0 + 148.0 + 2.0, 23.0), "ab der Schnittkante des Rückens (b + W, b)");
        assert_eq!((width, height), (6.0, 150.0), "Breite in Leserichtung = senkrechte Ausdehnung (Vertrag 1.2)");
    }

    #[test]
    fn guides_cover_bleed_trim_safety_spine_and_middle() {
        let g = CoverGeometry::new(148.0, 210.0, 12.0, 3.0);
        let kinds: Vec<&str> = guides(&g).iter().map(|g| g.0).collect();
        for kind in ["bleed", "trim", "safety", "spine", "center"] {
            assert!(kinds.contains(&kind), "Hilfslinie {kind} fehlt: {kinds:?}");
        }
        let spine: Vec<f64> = guides(&g).iter().filter(|g| g.0 == "spine").map(|g| g.2).collect();
        assert_eq!(spine, [151.0, 163.0], "Rückenkanten bei b + W und b + W + S");
        let safety = kinds.iter().filter(|k| **k == "safety").count();
        assert_eq!(safety, 3, "Sicherheitsabstand je Fläche (Rücken 12 mm > 2 × 5 mm)");
    }

    #[test]
    fn the_ok_code_goes_before_the_fragment() {
        assert_eq!(with_code("/werke/3/umschlag#anordnung", "anordnung"), "/werke/3/umschlag?ok=anordnung#anordnung");
        assert_eq!(with_code("/einstellungen/werk?work=3", "impressum"), "/einstellungen/werk?work=3&ok=impressum");
    }

    #[test]
    fn back_must_be_a_relative_safe_address() {
        assert_eq!(safe_back(Some("/werke/3/bilder")).as_deref(), Some("/werke/3/bilder"), "relativ: angenommen");
        assert_eq!(safe_back(Some("//boese.example/x")), None, "Protokoll-relativ führt hinaus");
        assert_eq!(safe_back(Some("https://boese.example/")), None, "absolute Adresse");
        assert_eq!(safe_back(Some("/x\"onload")), None, "Anführungszeichen");
        assert_eq!(safe_back(Some("/x\\y")), None, "Rückstrich");
    }

    #[test]
    fn the_hint_goes_before_the_fragment() {
        assert_eq!(
            success_target("/werke/3/bilder#umschlag", "umschlag", "umschlag_gesetzt"),
            "/werke/3/bilder?hinweis=umschlag_gesetzt#umschlag",
            "Abfrage vor dem Anker"
        );
        assert_eq!(
            success_target("/einstellungen/werk?work=3", "umschlag", "umschlag_gesetzt"),
            "/einstellungen/werk?work=3&ok=umschlag",
            "Einstellungen bekommen ok="
        );
    }

    #[test]
    fn svg_size_is_read_from_the_root_element() {
        let svg = "<svg class=\"typst-doc\" viewBox=\"0 0 100 200\" width=\"100pt\" height=\"200.5pt\" xmlns=\"x\"><g/></svg>";
        assert_eq!(svg_size_pt(svg), Some((100.0, 200.5)), "Breite und Höhe in pt");
    }
}
