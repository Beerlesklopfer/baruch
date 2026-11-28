// SPDX-License-Identifier: AGPL-3.0-or-later
//! EPUB preview per reader (Round 20b, owner “reader”).
//!
//! - **RPC** ([`handles`], [`call`]): `epub.readers {}` → `{readers:[ReaderProfile…]}`;
//!   `epub.preview {work, readers?, file?, footnotes?}` → `{readers:[{slug,name,url}], files, file, warnings}`;
//!   unknown slug or unknown file → -32602.
//! - **Pages** ([`pages`]): `GET /werke/{id}/epub/vorschau[?leser=…&leser=…&datei=…&fussnoten=…&nacht=1]`
//!   (form GET and iframes, usable without JavaScript; default: the mandatory profiles and the first part or
//!   chapter file), `GET /werke/{id}/epub/leser/{slug}/{*file}` (one file of the book; XHTML with inserted
//!   reader CSS from `static/readers/<slug>.css`, everything else unchanged; CSP `sandbox allow-same-origin;
//!   frame-ancestors 'self'`,
//!   `nosniff`; `guard::no_framing` sets `X-Frame-Options: SAMEORIGIN` instead of `DENY` for it).
//! - Below the preview stands the check report of “epub-pruefung” ([`crate::epub_check::preview_panel`], finished
//!   HTML fragment, only embedded here).
//!
//! The files are created anew on every request from the current state ([`crate::epub::book_for`]); there is no
//! cache, so that the preview never shows an old state.
//!
//! **Requests from the own frame:** because of `sandbox` the framed document runs with an opaque
//! origin; its sub-requests (images, `style/book.css`, links between chapters) therefore come with
//! `Sec-Fetch-Site: cross-site`. Such a request is accepted only if `Referer` points to the same `Host`
//! (the browser sets it, a foreign page cannot forge it); otherwise 403 as for every GET of a
//! foreign page. Browsers shorten the `Referer` of such requests to the origin (`http://host/`, Firefox in the
//! visual check of the integrator R20, also with `Referrer-Policy` in the frame); so the shortened
//! `Referer` is accepted or – if a browser sends it unshortened – a file of the same work under
//! `/werke/{id}/epub/leser/`.

use std::sync::{Arc, OnceLock};

use axum::Router;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get;
use baruch_epub::readers::{PROFILES, ReaderProfile, profile, required};
use baruch_epub::{EpubBook, EpubItem, FootnoteMode, ItemKind};
use baruch_store::{StoreError, WorkId};
use minijinja::Environment;
use minijinja::value::{Serde, Value as JinjaValue};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::AppState;
use crate::i18n::tc;
use crate::rpc::{INVALID_PARAMS, METHOD_NOT_FOUND, REFUSED, RpcError};
use crate::view::Settings;

/// CSP of the framed files (contract §0.9).
/// `allow-same-origin` without `allow-scripts`: scripts and forms stay blocked, but images and styles of the
/// reading file load as requests of the same origin. Without this permission the frame has an opaque origin,
/// the browser sends `Sec-Fetch-Site: cross-site`, and the protection against foreign pages answers 403 – the
/// cover image was missing (found by the user 2026-11-09).
pub const FRAME_CSP: &str = "sandbox allow-same-origin; frame-ancestors 'self'";

/// Marker in `static/readers/<slug>.css`: what stands after it applies only in night mode.
pub const NIGHT_MARK: &str = "/* @night */";

const XHTML: &str = "application/xhtml+xml";

/// Methods of this module.
pub fn handles(method: &str) -> bool {
    matches!(method, "epub.readers" | "epub.preview")
}

/// CSS of the reader `slug` (whole file `static/readers/<slug>.css`), unknown → `None`.
pub fn reader_css(slug: &str) -> Option<&'static str> {
    Some(match slug {
        "apple-books-iphone" => include_str!("../static/readers/apple-books-iphone.css"),
        "apple-books-ipad" => include_str!("../static/readers/apple-books-ipad.css"),
        "google-play-books" => include_str!("../static/readers/google-play-books.css"),
        "kindle" => include_str!("../static/readers/kindle.css"),
        "kindle-app" => include_str!("../static/readers/kindle-app.css"),
        "tolino" => include_str!("../static/readers/tolino.css"),
        "kobo" => include_str!("../static/readers/kobo.css"),
        "thorium" => include_str!("../static/readers/thorium.css"),
        "calibre" => include_str!("../static/readers/calibre.css"),
        _ => return None,
    })
}

/// Inserted reader CSS: the part before [`NIGHT_MARK`], with `night` (and night mode in the profile) the whole file.
pub fn reader_style(reader: &ReaderProfile, night: bool) -> String {
    let css = reader_css(reader.slug).unwrap_or("");
    match css.split_once(NIGHT_MARK) {
        Some((day, _)) if !(night && reader.supports.night_mode) => day.to_owned(),
        _ => css.to_owned(),
    }
}

/// `file` is a harmless relative path (no `..`, no `/` at the start, no empty parts, no `\`).
fn safe_href(file: &str) -> bool {
    !file.is_empty()
        && file.len() <= 1024
        && !file.contains('\\')
        && !file.chars().any(char::is_control)
        && file.split('/').all(|part| !part.is_empty() && part != "." && part != "..")
}

/// File `file` (relative to OEBPS/) of the book as the reader `reader` gets it: (Content-Type, bytes). XHTML
/// gets the reader CSS as `<style id="baruch-reader">` before `</head>` (so after `style/book.css`), all other
/// files stay unchanged. Unknown or unsafe paths → `None`.
pub fn reader_file(book: &EpubBook, reader: &ReaderProfile, file: &str, night: bool) -> Option<(String, Vec<u8>)> {
    if !safe_href(file) {
        return None;
    }
    let item = book.item(file)?;
    if item.media_type != XHTML {
        return Some((item.media_type.clone(), item.bytes.clone()));
    }
    let text = String::from_utf8_lossy(&item.bytes);
    let style = format!(
        "<style id=\"baruch-reader\" type=\"text/css\" data-reader=\"{}\">\n{}</style>\n",
        reader.slug,
        reader_style(reader, night)
    );
    let out = match text.find("</head>") {
        Some(at) => format!("{}{style}{}", &text[..at], &text[at..]),
        None => text.into_owned(),
    };
    Some((format!("{XHTML}; charset=utf-8"), out.into_bytes()))
}

/// Files with text in reading order (spine without tables of contents).
pub fn text_files(book: &EpubBook) -> Vec<&EpubItem> {
    book.spine
        .iter()
        .filter_map(|id| book.items.iter().find(|i| &i.id == id))
        .filter(|i| i.media_type == XHTML && !matches!(i.kind, ItemKind::Nav | ItemKind::Ncx))
        .collect()
}

/// Default file: the first part or chapter file, otherwise the first file with text.
fn default_file(book: &EpubBook) -> Option<String> {
    let files = text_files(book);
    files
        .iter()
        .find(|i| matches!(i.kind, ItemKind::Part | ItemKind::Chapter))
        .or(files.first())
        .map(|i| i.href.clone())
}

/// Label of the file kind in the language of the request (list of chapter boundaries).
pub fn kind_label(kind: ItemKind) -> String {
    match kind {
        ItemKind::Cover => tc("export_msg.epub_kind.cover", &[]),
        ItemKind::Front => tc("export_msg.epub_kind.front", &[]),
        ItemKind::Part => tc("export_msg.epub_kind.part", &[]),
        ItemKind::Chapter => tc("export_msg.epub_kind.chapter", &[]),
        ItemKind::Notes => tc("export_msg.epub_kind.notes", &[]),
        ItemKind::Nav => tc("export_msg.epub_kind.nav", &[]),
        ItemKind::Ncx => tc("export_msg.epub_kind.ncx", &[]),
        ItemKind::Style => tc("export_msg.epub_kind.style", &[]),
        ItemKind::Image => tc("export_msg.epub_kind.image", &[]),
        ItemKind::CoverImage => tc("export_msg.epub_kind.cover_image", &[]),
        ItemKind::Glossary => tc("export_msg.epub_kind.glossary", &[]),
        ItemKind::Index => tc("export_msg.epub_kind.index", &[]),
    }
}

/// Address of the file `file` in the reader `slug` (digits, slug and `href` from the book; only `&` and spaces
/// do not occur).
pub fn leser_url(work: WorkId, slug: &str, file: &str, footnotes: FootnoteMode, night: bool) -> String {
    let mut url = format!("/werke/{work}/epub/leser/{slug}/{file}");
    let mut sep = '?';
    if footnotes == FootnoteMode::Endnotes {
        url.push_str("?fussnoten=endnoten");
        sep = '&';
    }
    if night {
        url.push(sep);
        url.push_str("nacht=1");
    }
    url
}

fn footnote_word(mode: FootnoteMode) -> &'static str {
    match mode {
        FootnoteMode::Popup => "popup",
        FootnoteMode::Endnotes => "endnotes",
    }
}

#[derive(Deserialize)]
struct PreviewParams {
    #[serde(default)]
    readers: Option<Vec<String>>,
    #[serde(default)]
    file: Option<String>,
    #[serde(default)]
    footnotes: FootnoteMode,
}

/// Profiles for `slugs` (display order, duplicates once); empty → mandatory profiles. Unknown → `Err(slug)`.
fn chosen_profiles(slugs: &[String]) -> Result<Vec<&'static ReaderProfile>, String> {
    for slug in slugs {
        if profile(slug).is_none() {
            return Err(slug.clone());
        }
    }
    if slugs.is_empty() {
        return Ok(required().collect());
    }
    Ok(PROFILES.iter().filter(|p| slugs.iter().any(|s| s == p.slug)).collect())
}

fn files_json(book: &EpubBook) -> Vec<Value> {
    text_files(book)
        .into_iter()
        .map(|i| json!({ "href": i.href, "title": i.title, "kind": i.kind, "key": i.key }))
        .collect()
}

pub async fn call(state: &AppState, work: WorkId, method: &str, params: Value) -> Result<Value, RpcError> {
    match method {
        "epub.readers" => Ok(json!({ "readers": PROFILES })),
        "epub.preview" => {
            let mut params = if params.is_null() { json!({}) } else { params };
            if let Some(map) = params.as_object_mut() {
                map.remove("work");
            }
            let p: PreviewParams =
                serde_json::from_value(params).map_err(|e| RpcError::new(INVALID_PARAMS, e.to_string()))?;
            let readers = chosen_profiles(p.readers.as_deref().unwrap_or_default()).map_err(|slug| {
                RpcError::new(INVALID_PARAMS, tc("export_msg.epub_preview.unknown_reader", &[("slug", slug.into())]))
            })?;
            let book = crate::epub::book_for(state, work, p.footnotes).await?;
            let file = match p.file.map(|f| f.trim().to_owned()).filter(|f| !f.is_empty()) {
                Some(file) => {
                    if !safe_href(&file) || !text_files(&book).iter().any(|i| i.href == file) {
                        return Err(RpcError::new(
                            INVALID_PARAMS,
                            tc("export_msg.epub_preview.no_text_file", &[("file", file.as_str().into())]),
                        ));
                    }
                    file
                }
                None => default_file(&book)
                    .ok_or_else(|| RpcError::new(REFUSED, tc("export_msg.epub_preview.no_text", &[])))?,
            };
            let readers: Vec<Value> = readers
                .iter()
                .map(|r| json!({ "slug": r.slug, "name": r.name, "url": leser_url(work, r.slug, &file, p.footnotes, false) }))
                .collect();
            Ok(json!({
                "readers": readers,
                "files": files_json(&book),
                "file": file,
                "footnotes": footnote_word(p.footnotes),
                "warnings": book.warnings,
            }))
        }
        _ => Err(RpcError::new(METHOD_NOT_FOUND, format!("unknown method {method}"))),
    }
}

/// Pages of the preview.
pub fn pages() -> Router<Arc<AppState>> {
    Router::new()
        .route("/werke/{id}/epub/vorschau", get(preview_page))
        .route("/werke/{id}/epub/leser/{slug}/{*file}", get(reader_page))
}

fn templates() -> &'static Environment<'static> {
    static ENV: OnceLock<Environment<'static>> = OnceLock::new();
    ENV.get_or_init(|| {
        let mut env = Environment::new();
        crate::i18n::install(&mut env);
        for (name, source) in [
            ("base.html", include_str!("../templates/base.html")),
            ("epub_preview.html", include_str!("../templates/epub_preview.html")),
        ] {
            env.add_template(name, source).expect("Vorlage der EPUB-Vorschau fehlerhaft");
        }
        env
    })
}

fn text_response(status: StatusCode, message: &str) -> Response {
    (status, [(header::CONTENT_TYPE, "text/plain; charset=utf-8")], message.to_owned()).into_response()
}

/// `Referer` points to the same `Host`: shortened to the origin (`/`) or to a file of the same work in the
/// preview (see module head).
pub fn from_own_frame(headers: &HeaderMap, work: WorkId) -> bool {
    let Some(host) = headers.get(header::HOST).and_then(|h| h.to_str().ok()) else { return false };
    let Some(referer) = headers.get(header::REFERER).and_then(|r| r.to_str().ok()) else { return false };
    let Some(rest) = referer.strip_prefix("http://").or_else(|| referer.strip_prefix("https://")) else {
        return false;
    };
    let (authority, path) = rest.split_at(rest.find('/').unwrap_or(rest.len()));
    authority == host && (path.is_empty() || path == "/" || path.starts_with(&format!("/werke/{work}/epub/leser/")))
}

fn footnotes_of(word: Option<&str>) -> Result<Option<FootnoteMode>, ()> {
    match word.map(str::trim) {
        None | Some("") => Ok(None),
        Some("popup") => Ok(Some(FootnoteMode::Popup)),
        Some("endnoten" | "endnotes") => Ok(Some(FootnoteMode::Endnotes)),
        Some(_) => Err(()),
    }
}

#[derive(Deserialize)]
struct ReaderQuery {
    fussnoten: Option<String>,
    nacht: Option<String>,
}

/// `GET /werke/{id}/epub/leser/{slug}/{*file}`.
async fn reader_page(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path((id, slug, file)): Path<(WorkId, String, String)>,
    Query(q): Query<ReaderQuery>,
) -> Response {
    if crate::guard::cross_site_get(&headers) && !from_own_frame(&headers, id) {
        return text_response(StatusCode::FORBIDDEN, &tc("export_msg.epub_preview.foreign", &[]));
    }
    let Some(reader) = profile(&slug) else {
        return text_response(StatusCode::NOT_FOUND, &tc("export_msg.epub_preview.unknown_profile", &[]));
    };
    if !safe_href(&file) {
        return text_response(StatusCode::NOT_FOUND, &tc("export_msg.epub_preview.unknown_file", &[]));
    }
    let Ok(footnotes) = footnotes_of(q.fussnoten.as_deref()) else {
        return text_response(StatusCode::BAD_REQUEST, &tc("export_msg.epub.footnotes", &[]));
    };
    // Links from a chapter file to the notes lose the query: notes.xhtml exists only with endnotes.
    let footnotes =
        footnotes.unwrap_or(if file == "text/notes.xhtml" { FootnoteMode::Endnotes } else { FootnoteMode::Popup });
    let night = q.nacht.as_deref() == Some("1");
    match state.store.work_slug(id).await {
        Ok(_) => {}
        Err(StoreError::UnknownWork(_)) => {
            return text_response(StatusCode::NOT_FOUND, &tc("export_msg.epub_preview.unknown_work", &[]));
        }
        Err(e) => {
            return text_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                &tc("export_msg.page.error", &[("error", e.to_string().into())]),
            );
        }
    }
    let book = match crate::epub::book_for(&state, id, footnotes).await {
        Ok(book) => book,
        Err(e) if e.code == REFUSED => return text_response(StatusCode::UNPROCESSABLE_ENTITY, &e.message),
        Err(e) => {
            return text_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                &tc("export_msg.page.error", &[("error", e.message.as_str().into())]),
            );
        }
    };
    let Some((media_type, bytes)) = reader_file(&book, reader, &file, night) else {
        return text_response(StatusCode::NOT_FOUND, &tc("export_msg.epub_preview.unknown_file", &[]));
    };
    let content_type =
        HeaderValue::from_str(&media_type).unwrap_or(HeaderValue::from_static("application/octet-stream"));
    (
        [
            (header::CONTENT_TYPE, content_type),
            (header::CONTENT_SECURITY_POLICY, HeaderValue::from_static(FRAME_CSP)),
            (header::X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff")),
            (header::CACHE_CONTROL, HeaderValue::from_static("no-cache")),
        ],
        bytes,
    )
        .into_response()
}

/// A profile on the preview page.
#[derive(serde::Serialize)]
struct FrameView {
    slug: &'static str,
    name: &'static str,
    device: &'static str,
    width: u32,
    height: u32,
    via_conversion: bool,
    note: &'static str,
    night: bool,
    src: JinjaValue,
    open: JinjaValue,
    lacks: Vec<String>,
}

/// Missing capabilities of a profile in words (note below the frame).
fn lacks(reader: &ReaderProfile) -> Vec<String> {
    let s = &reader.supports;
    let mut out = Vec::new();
    if !s.popup_footnotes {
        out.push(tc("export_msg.epub_preview.lacks_popup", &[]));
    }
    if !s.css_backgrounds {
        out.push(tc("export_msg.epub_preview.lacks_backgrounds", &[]));
    }
    if !s.svg_images {
        out.push(tc("export_msg.epub_preview.lacks_svg", &[]));
    }
    if !s.embedded_fonts {
        out.push(tc("export_msg.epub_preview.lacks_fonts", &[]));
    }
    if s.overrides_publisher_font {
        out.push(tc("export_msg.epub_preview.overrides_font", &[]));
    }
    if !s.hyphenation {
        out.push(tc("export_msg.epub_preview.lacks_hyphenation", &[]));
    }
    if !s.night_mode {
        out.push(tc("export_msg.epub_preview.lacks_night", &[]));
    }
    out
}

/// Path from the book unescaped (minijinja would otherwise escape `/`) as long as it has only harmless characters.
fn plain(path: &str) -> JinjaValue {
    if path.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '/')) {
        JinjaValue::from_safe_string(path.to_owned())
    } else {
        JinjaValue::from(path)
    }
}

#[derive(serde::Serialize)]
struct FileView {
    href: JinjaValue,
    title: String,
    kind: String,
    boundary: bool,
    current: bool,
    link: JinjaValue,
}

#[derive(serde::Serialize)]
struct ChoiceView {
    slug: &'static str,
    name: &'static str,
    required: bool,
    checked: bool,
}

/// Query of the preview as pairs (`leser` may come several times).
struct PreviewQuery {
    readers: Vec<String>,
    file: Option<String>,
    footnotes: Option<String>,
    night: bool,
}

impl PreviewQuery {
    fn from_pairs(pairs: Vec<(String, String)>) -> Self {
        let mut q = PreviewQuery { readers: Vec::new(), file: None, footnotes: None, night: false };
        for (name, value) in pairs {
            match name.as_str() {
                "leser" if !value.trim().is_empty() => q.readers.push(value.trim().to_owned()),
                "datei" if !value.trim().is_empty() => q.file = Some(value.trim().to_owned()),
                "fussnoten" => q.footnotes = Some(value),
                "nacht" => q.night = value == "1",
                _ => {}
            }
        }
        q
    }
}

/// Query part of the preview for links of the chapter boundary list (`datei` follows after it).
fn preview_query(readers: &[&'static ReaderProfile], footnotes: FootnoteMode, night: bool) -> String {
    let mut parts: Vec<String> = readers.iter().map(|r| format!("leser={}", r.slug)).collect();
    if footnotes == FootnoteMode::Endnotes {
        parts.push("fussnoten=endnoten".into());
    }
    if night {
        parts.push("nacht=1".into());
    }
    parts.join("&")
}

/// `GET /werke/{id}/epub/vorschau`.
async fn preview_page(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<WorkId>,
    Query(pairs): Query<Vec<(String, String)>>,
) -> Response {
    if crate::guard::cross_site_get(&headers) {
        return text_response(StatusCode::FORBIDDEN, &tc("export_msg.epub_preview.foreign", &[]));
    }
    let title = match state.store.works().await {
        Ok(works) => match works.into_iter().find(|(w, _, _)| *w == id) {
            Some((_, _, title)) => title,
            None => return text_response(StatusCode::NOT_FOUND, &tc("export_msg.epub_preview.unknown_work", &[])),
        },
        Err(e) => {
            return text_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                &tc("export_msg.page.error", &[("error", e.to_string().into())]),
            );
        }
    };
    let q = PreviewQuery::from_pairs(pairs);
    let mut status = StatusCode::OK;
    let mut notices: Vec<String> = Vec::new();
    // Unknown slugs: note and 404, the known ones stay chosen (otherwise the mandatory profiles).
    let known: Vec<String> = q.readers.iter().filter(|s| profile(s).is_some()).cloned().collect();
    for slug in q.readers.iter().filter(|s| profile(s).is_none()) {
        status = StatusCode::NOT_FOUND;
        notices.push(tc("export_msg.epub_preview.profile_not_shown", &[("slug", slug.as_str().into())]));
    }
    let readers = chosen_profiles(&known).unwrap_or_else(|_| required().collect());
    let footnotes = match footnotes_of(q.footnotes.as_deref()) {
        Ok(mode) => mode.unwrap_or_default(),
        Err(()) => {
            status = StatusCode::BAD_REQUEST;
            notices.push(tc("export_msg.epub_preview.footnotes_default", &[]));
            FootnoteMode::Popup
        }
    };
    let settings = Settings::from_cookie_header(headers.get(header::COOKIE).and_then(|v| v.to_str().ok()));
    let book = match crate::epub::book_for(&state, id, footnotes).await {
        Ok(book) => Some(book),
        Err(e) => {
            status =
                if e.code == REFUSED { StatusCode::UNPROCESSABLE_ENTITY } else { StatusCode::INTERNAL_SERVER_ERROR };
            notices.push(e.message);
            None
        }
    };
    let mut file = None;
    if let Some(book) = &book {
        file = match &q.file {
            Some(f) if safe_href(f) && text_files(book).iter().any(|i| &i.href == f) => Some(f.clone()),
            Some(f) => {
                status = StatusCode::NOT_FOUND;
                notices.push(tc("export_msg.epub_preview.file_missing", &[("file", f.as_str().into())]));
                default_file(book)
            }
            None => default_file(book),
        };
    }
    let query = preview_query(&readers, footnotes, q.night);
    let files: Vec<FileView> = book
        .as_deref()
        .map(|book| {
            text_files(book)
                .into_iter()
                .map(|i| FileView {
                    href: plain(&i.href),
                    title: i.title.clone().unwrap_or_else(|| i.href.clone()),
                    kind: kind_label(i.kind),
                    boundary: matches!(i.kind, ItemKind::Part | ItemKind::Chapter),
                    current: file.as_deref() == Some(i.href.as_str()),
                    link: crate::typeset::href(format!("/werke/{id}/epub/vorschau?{query}&datei={}", i.href)),
                })
                .collect()
        })
        .unwrap_or_default();
    let frames: Vec<FrameView> = match &file {
        Some(file) => readers
            .iter()
            .map(|r| {
                let src = leser_url(id, r.slug, file, footnotes, q.night);
                FrameView {
                    slug: r.slug,
                    name: r.name,
                    device: r.device,
                    width: r.width_px,
                    height: r.height_px,
                    via_conversion: r.via_conversion,
                    note: r.note,
                    night: q.night && r.supports.night_mode,
                    src: crate::typeset::href(src.clone()),
                    open: crate::typeset::href(src),
                    lacks: lacks(r),
                }
            })
            .collect(),
        None => Vec::new(),
    };
    let choices: Vec<ChoiceView> = PROFILES
        .iter()
        .map(|p| ChoiceView {
            slug: p.slug,
            name: p.name,
            required: p.required,
            checked: readers.iter().any(|r| r.slug == p.slug),
        })
        .collect();
    let panel = crate::epub_check::preview_panel(&state, id, &readers).await;
    let download = match footnotes {
        FootnoteMode::Popup => format!("/werke/{id}/epub"),
        FootnoteMode::Endnotes => format!("/werke/{id}/epub?fussnoten=endnoten"),
    };
    let ctx = minijinja::context! {
        settings => JinjaValue::from(Serde(settings)),
        work => minijinja::context! { id => id, title => title },
        notices => notices,
        choices => JinjaValue::from(Serde(choices)),
        files => JinjaValue::from(Serde(files)),
        file => file,
        frames => JinjaValue::from(Serde(frames)),
        endnotes => footnotes == FootnoteMode::Endnotes,
        night => q.night,
        warnings => JinjaValue::from(Serde(book.as_deref().map(|b| b.warnings.clone()).unwrap_or_default())),
        panel => JinjaValue::from_safe_string(panel),
        download => crate::typeset::href(download),
        check_href => crate::typeset::href(format!("/werke/{id}/epub/pruefung")),
        action => crate::typeset::href(format!("/werke/{id}/epub/vorschau")),
    };
    match templates().get_template("epub_preview.html").and_then(|t| t.render(ctx)) {
        Ok(html) => (status, Html(html)).into_response(),
        Err(e) => text_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            &tc(
                "export_msg.typeset.template_error",
                &[("name", "epub_preview.html".into()), ("error", e.to_string().into())],
            ),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_harmless_relative_paths_are_served() {
        for ok in ["text/k12.xhtml", "style/book.css", "images/bilder/a.png", "nav.xhtml"] {
            assert!(safe_href(ok), "{ok} ist ein gewöhnlicher Pfad im Buch");
        }
        for bad in ["", "../x", "text/../../etc/passwd", "/etc/passwd", "text//k1.xhtml", "a\\b", "./nav.xhtml", "a\nb"]
        {
            assert!(!safe_href(bad), "{bad:?} darf nie eine Datei liefern (Pfad hinaus oder unlesbar)");
        }
    }

    #[test]
    fn the_night_part_comes_only_with_night_mode() {
        let kindle = profile("kindle").unwrap();
        let day = reader_style(kindle, false);
        let night = reader_style(kindle, true);
        assert!(!day.contains(NIGHT_MARK), "ohne Nachtmodus fehlt der Nachtteil: {day}");
        assert!(night.contains(NIGHT_MARK) && night.len() > day.len(), "mit Nachtmodus die ganze Datei");
    }

    #[test]
    fn the_own_frame_is_recognised_only_on_the_same_host_and_work() {
        // Shortened Referer (only the origin) as Firefox sends it for sub-requests from the sandbox frame.
        let headers = |host: &str, referer: &str| {
            let mut h = HeaderMap::new();
            h.insert(header::HOST, host.parse().unwrap());
            h.insert(header::REFERER, referer.parse().unwrap());
            h
        };
        assert!(from_own_frame(
            &headers("127.0.0.1:8765", "http://127.0.0.1:8765/werke/3/epub/leser/kindle/text/k1.xhtml"),
            3
        ));
        let trimmed = headers("127.0.0.1:8765", "http://127.0.0.1:8765/");
        assert!(from_own_frame(&trimmed, 3), "auf die Herkunft gekürzter Referer desselben Hosts");
        assert!(!from_own_frame(&headers("127.0.0.1:8765", "http://evil.example/"), 3), "fremde Herkunft gekürzt");
        assert!(
            !from_own_frame(&headers("127.0.0.1:8765", "http://evil.example/werke/3/epub/leser/kindle/x"), 3),
            "fremder Host im Referer"
        );
        assert!(
            !from_own_frame(&headers("127.0.0.1:8765", "http://127.0.0.1:8765/werke/4/epub/leser/kindle/x"), 3),
            "anderes Werk"
        );
        assert!(
            !from_own_frame(&headers("127.0.0.1:8765", "http://127.0.0.1:8765/werke/3/epub/vorschau"), 3),
            "nicht aus dem Rahmen"
        );
    }
}
