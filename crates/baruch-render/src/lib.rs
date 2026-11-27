// SPDX-License-Identifier: AGPL-3.0-or-later
//! Typst typesetting for the preview (contract Round 8, section 4a).
//!
//! Pure, without database and without network: [`compile`] gets the work as **one** Typst file
//! (`main`, the byte-identical export including the work template) and the images as virtual files and
//! typesets it into SVG pages with the embedded Typst compiler.
//!
//! - **Files:** `image("…")` searches relative to `main`, which lies in the root folder; `/x`, `./x` and
//!   `x` therefore mean the same file. The search uses the normal form ([`normalize_path`]: without leading
//!   `/` and `./`, without empty parts – the same rule as `baruch_store::normalize_asset_path`).
//!   If a file is missing, Typst reports an error; the message is then in German in
//!   [`Rendered::diagnostics`].
//! - **Fonts:** always the embedded ones from `typst-assets` (Libertinus Serif, New Computer
//!   Modern, DejaVu Sans Mono), behind them the system fonts (searched once per process). If the
//!   template demands a font that does not exist, Typst continues with the next available one; that
//!   yields a warning „Schrift „X“ fehlt; ersetzt durch eingebettete Schrift“, the pages stay.
//! - **Packages:** `#import "@preview/…"` is never loaded (no network); error „Typst-Pakete werden
//!   nicht geladen (offline)“.
//! - **Revision state:** [`stand`] is the SHA-256 over `main` and all files – the key under
//!   which the server caches typeset pages.
//! - **PDF (Round 11):** [`compile_pdf`] typesets the same source in the same world (fonts, files)
//!   and writes it with `typst-pdf` (Apache-2.0) as a PDF; the page count is the same as with
//!   [`compile`]. On an error there are no bytes, never half a file.
//! - **Print check (Round 11):** [`Rendered::print`] or [`Pdf::print`] names every raster image that
//!   has less than [`MIN_DPI`] in its typeset size, with the pixels it would need for 300 dpi
//!   ([`PrintWarning`]). SVG graphics have no resolution and never appear. The warnings are
//!   deliberately not in `diagnostics`, so that error and warning counts stay unchanged.
//! - **Cancellation (Round 18a):** [`compile_cancellable`] typesets like [`compile`], but checks a [`Cancel`].
//!   Typst itself knows no cancellation; so the run is interrupted only at the places where Baruch takes part:
//!   before the run, between typesetting and SVG and before every SVG page. Evaluation (also loops of the
//!   template) and layout continue until Typst is finished by itself; a cancelled run always delivers `None` and
//!   never half pages. A cancellation through the world (refusing files or fonts after the cancellation) does
//!   **not** work: Typst remembers world answers (`comemo`) and aborts with "found differing return values" if
//!   the same question is answered differently in the same run (measured in `tests/cancel.rs`, numbers in
//!   `docs/Architektur.md`, „Runde 18a“).
//! - **Font families (Round 27):** [`font_families`] names every family with its styles and whether it is
//!   embedded; [`has_font_family`] checks a name case-insensitively. The server typesets a missing font of a
//!   cover block with the default and warns (print check `font`).
//! - **Page sizes (Round 27):** [`page_sizes_pt`] reads width and height of every typeset page from the SVG
//!   header – with it the server measures block heights (one page per block with `height: auto`) and the page
//!   size of the template.
//! - **Lines per page (Round 18a):** [`Rendered::page_lines`] names per page the lines of `main` from which
//!   text is on it – with it the server assigns a page shared by two chapters to both.
//! - **Measuring aid (Round 29):** [`page_texts`] names per page the typeset text pieces ([`PlacedText`]: text,
//!   baseline left in pt from the top left, width, size, font) – with it tests measure every typesetting setting
//!   (margins with gutter, page numbers, running heads, line spacing, justification, hyphenation).
//! - **Raster image (Round 21c):** [`render_png`] rasterises a page with `typst-render` as a PNG in at most
//!   the given pixel size (aspect ratio is kept) – for the EPUB cover from the front of the cover.
//! - **Type change (Round 21c):** if the content of an image file does not fit the extension of its path (a PNG
//!   replaces `bilder/haus.svg`, the text still names the old path), `image("<Pfad>"…)` gets the format
//!   explicitly during typesetting (`with_image_formats`); Typst would otherwise take it from the extension and
//!   fail.

pub mod cli;
pub mod notes;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};

use sha2::{Digest, Sha256};
use typst::diag::{FileError, FileResult, SourceDiagnostic};
use typst::foundations::{Bytes, Datetime, NativeElement};
use typst::introspection::Introspector;
use typst::syntax::{DiagSpan, FileId, RootedPath, Source as TypstSource, VirtualPath, VirtualRoot};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Library, LibraryExt, World, WorldExt};
use typst_kit::fonts::FontStore;
use typst_layout::PagedDocument;

/// The work to typeset: the main file and the other files (images) under their path in
/// normal form (e.g. `bilder/laube.svg`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Source {
    pub main: String,
    pub files: Vec<(String, Vec<u8>)>,
}

/// Severity of a message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

impl Severity {
    /// `"error"` or `"warning"` (RPC, MCP).
    pub fn as_str(self) -> &'static str {
        match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
        }
    }
}

/// A message of Typst. `line`/`column` are 1-based and refer to `main` if
/// `file` is empty, otherwise to the named file; without a location both `None`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub severity: Severity,
    pub message: String,
    pub hints: Vec<String>,
    pub line: Option<usize>,
    pub column: Option<usize>,
    pub file: Option<String>,
}

/// A heading in `main` (line, 1-based) and the page on which Typst has set it
/// (1-based).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeadingPos {
    pub line: usize,
    pub page: usize,
}

/// Result of [`compile`]: state, pages as SVG, headings and messages. On an error
/// `pages`, `headings` and `print` are empty.
#[derive(Debug, Clone, PartialEq)]
pub struct Rendered {
    pub stand: String,
    pub pages: Vec<String>,
    pub headings: Vec<HeadingPos>,
    pub diagnostics: Vec<Diagnostic>,
    /// Print check: raster images below [`MIN_DPI`] (not in `diagnostics`).
    pub print: Vec<PrintWarning>,
    /// Per page (index 0 = page 1) the lines of `main` (1-based, ascending, without duplicates) from which
    /// typeset text on it comes – also headers and footers and text that the template quotes from headings
    /// (running heads, table of contents). Empty on an error.
    pub page_lines: Vec<Vec<usize>>,
}

/// Cancellation of a run (Round 18a): shared between the one who cancels and the run in
/// [`compile_cancellable`]. `Default` is "not cancelled"; clones share the same state.
#[derive(Debug, Clone, Default)]
pub struct Cancel(Arc<AtomicBool>);

impl Cancel {
    /// New, not cancelled state.
    pub fn new() -> Cancel {
        Cancel::default()
    }

    /// Cancel (affects all clones; harmless if repeated).
    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    /// Was it cancelled?
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

/// Minimum resolution for print (dots per inch in the typeset size).
pub const MIN_DPI: f64 = 300.0;

/// A raster image that would be too coarse in print: path (normal form, e.g. `bilder/umschlag.png`; empty
/// without a known file), page (1-based), pixels of the image, typeset size in mm, achieved
/// resolution (the smaller of both directions) and the pixels for [`MIN_DPI`].
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct PrintWarning {
    pub path: String,
    pub page: usize,
    pub width_px: u32,
    pub height_px: u32,
    pub width_mm: f64,
    pub height_mm: f64,
    pub dpi: f64,
    pub needed_width_px: u32,
    pub needed_height_px: u32,
    /// German, e.g. „Bild „umschlag.png“: 113 dpi bei 18,7 × 27,3 cm – für 300 dpi braucht es
    /// 2209 × 3224 Pixel“.
    pub message: String,
}

/// Result of [`compile_pdf`]: state, the bytes of the PDF (`None` exactly when Typst reported an
/// error), page count, messages and print check.
#[derive(Debug, Clone, PartialEq)]
pub struct Pdf {
    pub stand: String,
    pub bytes: Option<Vec<u8>>,
    pub pages: usize,
    pub diagnostics: Vec<Diagnostic>,
    pub print: Vec<PrintWarning>,
}

impl Pdf {
    /// Was there an error (then without bytes)?
    pub fn has_errors(&self) -> bool {
        self.bytes.is_none() || self.diagnostics.iter().any(|d| d.severity == Severity::Error)
    }
}

impl Rendered {
    /// Was there an error?
    pub fn has_errors(&self) -> bool {
        self.diagnostics.iter().any(|d| d.severity == Severity::Error)
    }
}

/// Message for `@preview` and other package imports.
pub const OFFLINE: &str = "Typst-Pakete werden nicht geladen (offline)";

/// Name of the main file in the virtual root folder.
const MAIN_PATH: &str = "/main.typ";

/// Normal form of a path: without leading `/` and `./`, without empty parts; `..` or empty → `None`.
pub fn normalize_path(path: &str) -> Option<String> {
    let mut parts = Vec::new();
    for part in path.trim().split('/') {
        match part {
            "" | "." => {}
            ".." => return None,
            name => parts.push(name),
        }
    }
    (!parts.is_empty()).then(|| parts.join("/"))
}

/// SHA-256 (hex) over `main` and all files, sorted by path and with lengths in front – every
/// change of a byte, of a path, a new or missing file changes the state, the
/// order of the files in [`Source::files`] does not.
pub fn stand(source: &Source) -> String {
    let mut hasher = Sha256::new();
    let mut feed = |bytes: &[u8]| {
        hasher.update((bytes.len() as u64).to_le_bytes());
        hasher.update(bytes);
    };
    feed(b"baruch-render 1");
    feed(source.main.as_bytes());
    let mut files: Vec<&(String, Vec<u8>)> = source.files.iter().collect();
    files.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
    feed(&(files.len() as u64).to_le_bytes());
    for (path, bytes) in files {
        feed(path.as_bytes());
        feed(bytes);
    }
    let digest = hasher.finalize();
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

/// SHA-256 (hex, lower case) over arbitrary bytes, e.g. over a generated PDF (answer of `work.pdf`).
pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

/// Fonts of a process: the stock and how many of them (at the front) are embedded.
struct Fonts {
    store: FontStore,
    embedded: usize,
}

/// Fonts: embedded ones first, then the system fonts; once per process.
fn font_data() -> &'static Fonts {
    static FONTS: OnceLock<Fonts> = OnceLock::new();
    FONTS.get_or_init(|| {
        let mut store = FontStore::new();
        let embedded: Vec<_> = typst_kit::fonts::embedded().collect();
        let count = embedded.len();
        store.extend(embedded);
        store.extend(typst_kit::fonts::system());
        Fonts { store, embedded: count }
    })
}

fn fonts() -> &'static FontStore {
    &font_data().store
}

fn library() -> &'static LazyHash<Library> {
    static LIBRARY: OnceLock<LazyHash<Library>> = OnceLock::new();
    LIBRARY.get_or_init(|| LazyHash::new(Library::default()))
}

/// Font search in advance (Round 18a): load embedded and system fonts once per process, so that the first
/// typesetting run does not wait for it. Blocking (noticeable with many system fonts) – in the server in
/// `spawn_blocking`. Returns the number of fonts.
pub fn warm_fonts() -> usize {
    fonts().book().families().count()
}

/// A font family for the selection in the cover editor (Round 27, `cover.fonts`): name, available styles
/// (`normal`, `bold`, `italic`) and origin.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct FontFamily {
    pub name: String,
    /// Available styles (`normal`, `bold`, `italic` …).
    pub styles: Vec<&'static str>,
    /// Embedded with Baruch (instead of a system font).
    pub embedded: bool,
}

/// All font families (embedded and system fonts), sorted by name and without duplicates (Typst treats
/// families case-insensitively). Per family the styles that really exist: `normal` (upright, below semi-bold),
/// `bold` (upright, from semi-bold 600), `italic` (italic or oblique); plus whether it is embedded with Baruch
/// (`embedded`, e.g. Libertinus Serif) or comes from the system. Blocking like [`warm_fonts`] – in the server
/// in `spawn_blocking`.
pub fn font_families() -> Vec<FontFamily> {
    use typst::text::FontStyle as Style;
    let data = font_data();
    let book = data.store.book();
    let mut out: Vec<FontFamily> = book
        .families()
        .map(|(name, ids)| {
            let (mut normal, mut bold, mut italic, mut embedded) = (false, false, false, false);
            for id in ids {
                embedded |= id < data.embedded;
                let Some(info) = book.info(id) else { continue };
                let heavy = info.variant.weight.to_number() >= 600;
                match info.variant.style {
                    Style::Normal if heavy => bold = true,
                    Style::Normal => normal = true,
                    Style::Italic | Style::Oblique => italic = true,
                }
            }
            let styles = [(normal, "normal"), (bold, "bold"), (italic, "italic")]
                .into_iter()
                .filter_map(|(has, word)| has.then_some(word))
                .collect();
            FontFamily { name: name.to_owned(), styles, embedded }
        })
        .collect();
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()).then_with(|| a.name.cmp(&b.name)));
    out.dedup_by(|a, b| a.name.to_lowercase() == b.name.to_lowercase());
    out
}

/// Does the font family `name` exist (case-insensitive)? Blocking like
/// [`warm_fonts`].
pub fn has_font_family(name: &str) -> bool {
    let wanted = name.trim().to_lowercase();
    !wanted.is_empty() && fonts().book().families().any(|(family, _)| family.to_lowercase() == wanted)
}

/// Width and height (pt) of a typeset SVG page from its header (`width="…pt" height="…pt"`).
pub fn svg_size_pt(svg: &str) -> Option<(f64, f64)> {
    let head = &svg[..svg.find('>')?];
    let attr = |name: &str| -> Option<f64> {
        let at = head.find(&format!(" {name}=\""))? + name.len() + 3;
        let rest = &head[at..];
        let value = &rest[..rest.find('"')?];
        value.strip_suffix("pt")?.parse().ok()
    };
    Some((attr("width")?, attr("height")?))
}

/// Width and height (pt) of every page of a typesetting run (same order as [`Rendered::pages`]); an unreadable
/// page yields (0, 0).
pub fn page_sizes_pt(rendered: &Rendered) -> Vec<(f64, f64)> {
    rendered.pages.iter().map(|page| svg_size_pt(page).unwrap_or((0.0, 0.0))).collect()
}

/// The world of a run: `main` and the files in normal form. Its answers stay the same during a run (Typst
/// checks that, see cancellation in the module header).
struct Baruch {
    main: TypstSource,
    files: HashMap<String, Bytes>,
}

/// Format of an image file by its first bytes (Typst word: `png`, `jpg`, `gif`, `webp`, `svg`); otherwise `None`.
fn sniff_format(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("png")
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("jpg")
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some("gif")
    } else if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some("webp")
    } else {
        let head = std::str::from_utf8(&bytes[..bytes.len().min(4096)]).ok().or_else(|| {
            // Cut off in the middle of a character: up to the last complete position.
            let cut = &bytes[..bytes.len().min(4096)];
            std::str::from_utf8(&cut[..cut.len().saturating_sub(3)]).ok()
        })?;
        let head = head.trim_start_matches('\u{feff}').trim_start();
        (head.starts_with('<') && head.contains("<svg")).then_some("svg")
    }
}

/// Format that Typst derives from the extension of a path (like `typst_library::visualize::image`).
fn extension_format(path: &str) -> Option<&'static str> {
    match path.rsplit_once('.')?.1.to_ascii_lowercase().as_str() {
        "png" => Some("png"),
        "jpg" | "jpeg" => Some("jpg"),
        "gif" => Some("gif"),
        "webp" => Some("webp"),
        "svg" | "svgz" => Some("svg"),
        _ => None,
    }
}

/// Type change (Round 21c): an image can change its type (PNG replaces SVG) while template and text still name
/// it under the old path with the old extension. Typst takes the format from the extension and then fails
/// ("failed to parse SVG"). For every file whose content does not fit the extension, every call
/// `image("<Pfad>"…)` with a string literal as the first argument gets the format explicitly (`format: "png"`),
/// directly after the literal and without a new line (line numbers of the messages stay). Calls that already
/// name `format:` and computed paths (variables) stay unchanged. Nothing to do → `None`.
fn with_image_formats(main: &str, files: &HashMap<String, Bytes>) -> Option<String> {
    let mismatched: HashMap<&str, &'static str> = files
        .iter()
        .filter_map(|(path, bytes)| {
            let actual = sniff_format(bytes.as_slice())?;
            (extension_format(path) != Some(actual)).then_some((path.as_str(), actual))
        })
        .collect();
    if mismatched.is_empty() || !main.contains("image(") {
        return None;
    }
    let mut out = String::with_capacity(main.len() + 64);
    let mut rest = main;
    let mut changed = false;
    while let Some(at) = rest.find("image(") {
        // Only the identifier `image` (not `myimage(`), then optional whitespace and a literal.
        let ident_ok = rest[..at].chars().next_back().is_none_or(|c| !(c.is_alphanumeric() || c == '_' || c == '-'));
        let open = at + "image(".len();
        out.push_str(&rest[..open]);
        rest = &rest[open..];
        if !ident_ok {
            continue;
        }
        let lead = rest.len() - rest.trim_start().len();
        let after_ws = &rest[lead..];
        let Some(literal) = after_ws.strip_prefix('"') else { continue };
        let Some(end) = literal.find(['"', '\\', '\n']).filter(|&e| literal.as_bytes()[e] == b'"') else { continue };
        let path = &literal[..end];
        let Some(format) = normalize_path(path).and_then(|p| mismatched.get(p.as_str()).copied()) else { continue };
        // Rest of the arguments up to the closing bracket: already a `format:` in it? Then change nothing.
        let tail = &literal[end + 1..];
        let mut depth = 1usize;
        let mut close = tail.len();
        let mut in_string = false;
        let mut escaped = false;
        for (i, c) in tail.char_indices() {
            if in_string {
                match (escaped, c) {
                    (true, _) => escaped = false,
                    (false, '\\') => escaped = true,
                    (false, '"') => in_string = false,
                    _ => {}
                }
                continue;
            }
            match c {
                '"' => in_string = true,
                '(' | '[' | '{' => depth += 1,
                ')' | ']' | '}' => {
                    depth -= 1;
                    if depth == 0 {
                        close = i;
                        break;
                    }
                }
                _ => {}
            }
        }
        if tail[..close].contains("format:") {
            continue;
        }
        let literal_len = lead + 1 + end + 1;
        out.push_str(&rest[..literal_len]);
        out.push_str(&format!(", format: \"{format}\""));
        rest = &rest[literal_len..];
        changed = true;
    }
    out.push_str(rest);
    changed.then_some(out)
}

impl Baruch {
    fn new(source: &Source) -> Baruch {
        let id = FileId::new(RootedPath::new(VirtualRoot::Project, VirtualPath::new(MAIN_PATH).expect("fester Pfad")));
        let mut files = HashMap::new();
        for (path, bytes) in &source.files {
            if let Some(path) = normalize_path(path) {
                files.entry(path).or_insert_with(|| Bytes::new(bytes.clone()));
            }
        }
        let main = with_image_formats(&source.main, &files).unwrap_or_else(|| source.main.clone());
        Baruch { main: TypstSource::new(id, main), files }
    }

    /// Path (normal form) of the file with exactly these bytes; unknown → empty.
    fn path_of(&self, bytes: &[u8]) -> String {
        let mut paths: Vec<&String> =
            self.files.iter().filter(|(_, b)| b.as_slice() == bytes).map(|(path, _)| path).collect();
        paths.sort();
        paths.first().map(|p| (*p).clone()).unwrap_or_default()
    }

    fn lookup(&self, id: FileId) -> FileResult<&Bytes> {
        if let VirtualRoot::Package(_) = id.root() {
            return Err(FileError::Other(Some(OFFLINE.into())));
        }
        let path = id.vpath().get_without_slash();
        normalize_path(path)
            .and_then(|path| self.files.get(&path))
            .ok_or_else(|| FileError::NotFound(PathBuf::from(path)))
    }
}

impl World for Baruch {
    fn library(&self) -> &LazyHash<Library> {
        library()
    }

    fn book(&self) -> &LazyHash<FontBook> {
        fonts().book()
    }

    fn main(&self) -> FileId {
        self.main.id()
    }

    fn source(&self, id: FileId) -> FileResult<TypstSource> {
        if id == self.main.id() {
            return Ok(self.main.clone());
        }
        let bytes = self.lookup(id)?;
        let text = std::str::from_utf8(bytes.as_slice()).map_err(|_| FileError::InvalidUtf8)?;
        Ok(TypstSource::new(id, text.to_owned()))
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        if id == self.main.id() {
            return Ok(Bytes::from_string(self.main.text().to_owned()));
        }
        self.lookup(id).cloned()
    }

    fn font(&self, index: usize) -> Option<Font> {
        fonts().font(index)
    }

    fn today(&self, offset: Option<typst::foundations::Duration>) -> Option<Datetime> {
        let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).ok()?.as_secs() as i64;
        let hours = offset.map(|d| d.seconds() as i64).unwrap_or(0);
        let days = (secs + hours).div_euclid(86_400);
        let (y, m, d) = civil_from_days(days);
        Datetime::from_ymd(y, m, d)
    }
}

/// Days since 1970-01-01 → (year, month, day) in the Gregorian calendar (algorithm by
/// Howard Hinnant).
fn civil_from_days(days: i64) -> (i32, u8, u8) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    ((if m <= 2 { y + 1 } else { y }) as i32, m as u8, d as u8)
}

/// Typesets the work. Blocking (seconds for large works) – in the server only in `spawn_blocking`.
/// Never fails: errors are in [`Rendered::diagnostics`], the pages are then empty.
pub fn compile(source: &Source) -> Rendered {
    run(&Baruch::new(source), source, &Cancel::new()).expect("ohne Abbruch gibt es immer ein Ergebnis")
}

/// Like [`compile`], but cancellable through `cancel` (Round 18a): `None` = cancelled (before, during or after
/// typesetting; never half pages). What Typst really interrupts is in the module header.
pub fn compile_cancellable(source: &Source, cancel: &Cancel) -> Option<Rendered> {
    if cancel.is_cancelled() {
        return None;
    }
    run(&Baruch::new(source), source, cancel)
}

/// A run in the world `world`; `None` only if `cancel` was cancelled.
fn run(world: &Baruch, source: &Source, cancel: &Cancel) -> Option<Rendered> {
    let warned = typst::compile::<PagedDocument>(world);
    let cancelled = || {
        if cancel.is_cancelled() {
            // As after every run, release older intermediate results of Typst.
            typst::comemo::evict(2);
            true
        } else {
            false
        }
    };
    if cancelled() {
        return None;
    }
    let mut diagnostics: Vec<Diagnostic> = Vec::new();
    let (pages, headings, print, page_lines) = match warned.output {
        Ok(document) => {
            let options = typst_svg::SvgOptions::default();
            let mut pages = Vec::with_capacity(document.pages().len());
            for page in document.pages() {
                if cancelled() {
                    return None;
                }
                pages.push(typst_svg::svg(page, &options));
            }
            let lines = page_lines(world, &document);
            (pages, headings(world, &document), print_warnings(world, &document), lines)
        }
        Err(errors) => {
            diagnostics.extend(errors.iter().map(|d| convert(world, d)));
            (Vec::new(), Vec::new(), Vec::new(), Vec::new())
        }
    };
    diagnostics.extend(warned.warnings.iter().map(|d| convert(world, d)));
    // Typst remembers intermediate results process-wide; release those older than two runs again, so that
    // the memory of a long-lived server does not grow with every work.
    typst::comemo::evict(2);
    if cancelled() {
        return None;
    }
    Some(Rendered { stand: stand(source), pages, headings, diagnostics, print, page_lines })
}

/// Per page the lines of `main` from which text on it comes (first glyph of every text piece).
fn page_lines(world: &Baruch, document: &PagedDocument) -> Vec<Vec<usize>> {
    fn walk(world: &Baruch, frame: &typst::layout::Frame, out: &mut Vec<usize>) {
        use typst::layout::FrameItem;
        for (_, item) in frame.items() {
            match item {
                FrameItem::Group(group) => walk(world, &group.frame, out),
                FrameItem::Text(text) => {
                    let Some(glyph) = text.glyphs.iter().find(|g| !g.span.0.is_detached()) else { continue };
                    if let Some((line, _)) = main_position(world, DiagSpan::from(glyph.span.0)) {
                        out.push(line);
                    }
                }
                _ => {}
            }
        }
    }
    document
        .pages()
        .iter()
        .map(|page| {
            let mut lines = Vec::new();
            walk(world, &page.frame, &mut lines);
            lines.sort_unstable();
            lines.dedup();
            lines
        })
        .collect()
}

/// Typesets the work like [`compile`] (the same world, the same fonts and files) and writes it
/// as a PDF (`typst-pdf`, tagged, without timestamp). Blocking. Never fails: on an error
/// (typesetting or PDF output) `bytes` are `None` and `pages` 0, the messages are in `diagnostics`.
pub fn compile_pdf(source: &Source) -> Pdf {
    let world = Baruch::new(source);
    let warned = typst::compile::<PagedDocument>(&world);
    let mut diagnostics: Vec<Diagnostic> = Vec::new();
    let (bytes, pages, print) = match warned.output {
        Ok(document) => match typst_pdf::pdf(&document, &typst_pdf::PdfOptions::default()) {
            Ok(bytes) => (Some(bytes), document.pages().len(), print_warnings(&world, &document)),
            Err(errors) => {
                diagnostics.extend(errors.iter().map(|d| convert(&world, d)));
                (None, 0, Vec::new())
            }
        },
        Err(errors) => {
            diagnostics.extend(errors.iter().map(|d| convert(&world, d)));
            (None, 0, Vec::new())
        }
    };
    diagnostics.extend(warned.warnings.iter().map(|d| convert(&world, d)));
    typst::comemo::evict(2);
    Pdf { stand: stand(source), bytes, pages, diagnostics, print }
}

// ---- Raster image of a page (Round 21c) --------------------------------------------------------

/// A page as a raster image ([`render_png`]): PNG bytes and size in pixels.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Raster {
    pub png: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

/// Typesets `source` like [`compile`] and rasterises the page `page` (0-based) with `typst-render` (Apache-2.0)
/// as a PNG – as large as possible, but at most `max_width` × `max_height` pixels, in the aspect ratio of the
/// page (rounded to whole pixels). Blocking. A typesetting error (first error message), a missing page, a limit
/// of 0 or an error when writing the PNG → `Err` with a German message, never half an image. For the EPUB cover
/// from the front of the cover (Round 21c, cover size at most 1600 × 2560).
pub fn render_png(source: &Source, page: usize, max_width: u32, max_height: u32) -> Result<Raster, String> {
    if max_width == 0 || max_height == 0 {
        return Err("Rasterbild: Breite und Höhe müssen mindestens 1 Pixel sein.".to_owned());
    }
    let world = Baruch::new(source);
    let warned = typst::compile::<PagedDocument>(&world);
    let result = match warned.output {
        Ok(document) => match document.pages().get(page) {
            Some(page_ref) => {
                let size = page_ref.frame.size();
                let (w_pt, h_pt) = (size.x.to_pt(), size.y.to_pt());
                if !(w_pt > 0.0 && h_pt > 0.0) {
                    Err("Rasterbild: die Seite hat keine Fläche.".to_owned())
                } else {
                    // Largest scale at which the rounded pixel size fits into the limits (typst-render rounds).
                    let mut scale = (f64::from(max_width) / w_pt).min(f64::from(max_height) / h_pt);
                    while (scale * w_pt).round() > f64::from(max_width)
                        || (scale * h_pt).round() > f64::from(max_height)
                    {
                        scale *= 0.999;
                    }
                    let options = typst_render::RenderOptions {
                        pixel_per_pt: typst::utils::Scalar::new(scale),
                        render_bleed: false,
                    };
                    let pixmap = typst_render::render(page_ref, &options);
                    pixmap
                        .encode_png()
                        .map(|png| Raster { png, width: pixmap.width(), height: pixmap.height() })
                        .map_err(|e| format!("Rasterbild: PNG ließ sich nicht schreiben ({e})."))
                }
            }
            None => Err(format!(
                "Rasterbild: Seite {} gibt es nicht (gesetzt: {} Seiten).",
                page + 1,
                document.pages().len()
            )),
        },
        Err(errors) => {
            let first = errors.iter().next().map(|d| convert(&world, d).message).unwrap_or_default();
            Err(format!("Rasterbild: Satzfehler: {first}"))
        }
    };
    typst::comemo::evict(2);
    result
}

// ---- Measuring aid (Round 29) ------------------------------------------------------------------

/// A typeset text piece (a run of the same font, mostly a word or a line) for measurements (Round 29, contract
/// §5.7): text, start of the baseline at the left in pt from the top left corner of the page, width in pt, font
/// size in pt (with the scaling of the surrounding groups) and family name of the font.
#[derive(Debug, Clone, PartialEq)]
pub struct PlacedText {
    pub text: String,
    pub x_pt: f64,
    pub y_pt: f64,
    pub width_pt: f64,
    pub size_pt: f64,
    pub font: String,
}

/// Per page (index 0 = page 1) the typeset text pieces in the order of the frame (header, content, footer).
/// `None` on a typesetting error. Blocking like [`compile`]. For tests and the acceptance of the typesetting
/// settings: margins, page numbers, running heads, font and line spacing can thus be measured in pt.
pub fn page_texts(source: &Source) -> Option<Vec<Vec<PlacedText>>> {
    use typst::layout::{Frame, FrameItem, Point, Transform};
    fn walk(frame: &Frame, ts: Transform, out: &mut Vec<PlacedText>) {
        for (pos, item) in frame.items() {
            match item {
                FrameItem::Group(group) => {
                    let inner = ts.pre_concat(Transform::translate(pos.x, pos.y)).pre_concat(group.transform);
                    walk(&group.frame, inner, out);
                }
                FrameItem::Text(text) => {
                    let at = Point::new(pos.x, pos.y).transform(ts);
                    // Horizontal scaling (length of the transformed x axis) for width and size.
                    let scale = ts.sx.get().hypot(ts.ky.get());
                    out.push(PlacedText {
                        text: text.text.to_string(),
                        x_pt: at.x.to_pt(),
                        y_pt: at.y.to_pt(),
                        width_pt: text.width().to_pt() * scale,
                        size_pt: text.size.to_pt() * scale,
                        font: text.font.font().info().family.clone(),
                    });
                }
                _ => {}
            }
        }
    }
    let world = Baruch::new(source);
    let warned = typst::compile::<PagedDocument>(&world);
    let result = warned.output.ok().map(|document| {
        document
            .pages()
            .iter()
            .map(|page| {
                let mut out = Vec::new();
                walk(&page.frame, Transform::identity(), &mut out);
                out
            })
            .collect()
    });
    typst::comemo::evict(2);
    result
}

// ---- Print check -------------------------------------------------------------------------------

/// All raster images below [`MIN_DPI`], per page in the order of the typeset result; the same image in
/// the same size on the same page only once.
fn print_warnings(world: &Baruch, document: &PagedDocument) -> Vec<PrintWarning> {
    let mut out: Vec<PrintWarning> = Vec::new();
    for (index, page) in document.pages().iter().enumerate() {
        walk_frame(world, &page.frame, (1.0, 1.0), index + 1, &mut out);
    }
    out
}

/// Walk through the frame; `scale` is the scaling of the surrounding groups (x, y).
fn walk_frame(
    world: &Baruch,
    frame: &typst::layout::Frame,
    scale: (f64, f64),
    page: usize,
    out: &mut Vec<PrintWarning>,
) {
    use typst::layout::FrameItem;
    for (_, item) in frame.items() {
        match item {
            FrameItem::Group(group) => {
                let t = group.transform;
                let sx = t.sx.get().hypot(t.ky.get());
                let sy = t.kx.get().hypot(t.sy.get());
                walk_frame(world, &group.frame, (scale.0 * sx, scale.1 * sy), page, out);
            }
            FrameItem::Image(image, size, _) => {
                let typst::visualize::ImageKind::Raster(raster) = image.kind() else { continue };
                let width_mm = size.x.to_mm() * scale.0;
                let height_mm = size.y.to_mm() * scale.1;
                let path = world.path_of(raster.data().as_slice());
                if let Some(warning) = check_raster(path, page, raster.width(), raster.height(), width_mm, height_mm)
                    && !out.contains(&warning)
                {
                    out.push(warning);
                }
            }
            _ => {}
        }
    }
}

/// Warning for a raster image of `width_px` × `height_px` pixels, typeset in `width_mm` × `height_mm`;
/// `None` if it reaches [`MIN_DPI`] (or has no area).
pub fn check_raster(
    path: String,
    page: usize,
    width_px: u32,
    height_px: u32,
    width_mm: f64,
    height_mm: f64,
) -> Option<PrintWarning> {
    if !(width_mm > 0.0 && height_mm > 0.0) {
        return None;
    }
    let inch = |mm: f64| mm / 25.4;
    let dpi = (f64::from(width_px) / inch(width_mm)).min(f64::from(height_px) / inch(height_mm));
    if dpi >= MIN_DPI {
        return None;
    }
    let needed = |mm: f64| (inch(mm) * MIN_DPI - 1e-9).ceil().max(1.0) as u32;
    let (needed_width_px, needed_height_px) = (needed(width_mm), needed(height_mm));
    let shown = path.rsplit('/').next().filter(|n| !n.is_empty()).unwrap_or("(eingebettet)");
    let cm = |mm: f64| format!("{:.1}", mm / 10.0).replace('.', ",");
    let message = format!(
        "Bild „{shown}“: {} dpi bei {} × {} cm – für {} dpi braucht es {needed_width_px} × {needed_height_px} Pixel",
        dpi.floor() as u64,
        cm(width_mm),
        cm(height_mm),
        MIN_DPI as u64,
    );
    Some(PrintWarning {
        path,
        page,
        width_px,
        height_px,
        width_mm,
        height_mm,
        dpi,
        needed_width_px,
        needed_height_px,
        message,
    })
}

/// Headings from `main` with their page, sorted by line.
fn headings(world: &Baruch, document: &PagedDocument) -> Vec<HeadingPos> {
    let introspector = document.introspector();
    let mut out = Vec::new();
    for heading in introspector.query(&typst::model::HeadingElem::ELEM.select()) {
        let (Some(location), span) = (heading.location(), heading.span()) else { continue };
        let Some(page) = introspector.page(location) else { continue };
        let Some((line, _)) = main_position(world, span.into()) else { continue };
        out.push(HeadingPos { line, page: page.get() });
    }
    out.sort_by_key(|h| (h.line, h.page));
    out.dedup_by_key(|h| h.line);
    out
}

/// Line and column (1-based) of a location in `main`.
fn main_position(world: &Baruch, span: DiagSpan) -> Option<(usize, usize)> {
    if span.id()? != world.main.id() {
        return None;
    }
    let range = world.range(span)?;
    let (line, column) = world.main.lines().byte_to_line_column(range.start)?;
    Some((line + 1, column + 1))
}

/// Location of a message: own location, otherwise the first call site with a location (file, line, column).
fn locate(world: &Baruch, diagnostic: &SourceDiagnostic) -> (Option<String>, Option<usize>, Option<usize>) {
    let spans = std::iter::once(diagnostic.span)
        .chain(diagnostic.trace.iter().map(|point| DiagSpan::from(point.span)))
        .filter(|span| !span.is_detached());
    for span in spans {
        if let Some((line, column)) = main_position(world, span) {
            return (None, Some(line), Some(column));
        }
        if let Some(id) = span.id() {
            let file = Some(id.vpath().get_without_slash().to_owned());
            let position = world.source(id).ok().zip(world.range(span)).and_then(|(source, range)| {
                source.lines().byte_to_line_column(range.start).map(|(l, c)| (l + 1, c + 1))
            });
            return (file, position.map(|p| p.0), position.map(|p| p.1));
        }
    }
    (None, None, None)
}

fn convert(world: &Baruch, diagnostic: &SourceDiagnostic) -> Diagnostic {
    let severity = match diagnostic.severity {
        typst::diag::Severity::Error => Severity::Error,
        typst::diag::Severity::Warning => Severity::Warning,
    };
    let (file, line, column) = locate(world, diagnostic);
    let mut hints: Vec<String> = diagnostic.hints.iter().map(|hint| hint.v.to_string()).collect();
    let message = german(&diagnostic.message, &mut hints);
    Diagnostic { severity, message, hints, line, column, file }
}

/// Known messages in German; the English one stays as a hint.
fn german(message: &str, hints: &mut Vec<String>) -> String {
    if message.contains(OFFLINE) {
        hints.insert(0, "Funktionen aus Paketen in die Werkvorlage übernehmen oder den Import entfernen.".into());
        return OFFLINE.to_owned();
    }
    if let Some(family) = message.strip_prefix("unknown font family: ") {
        return format!("Schrift „{family}“ fehlt; ersetzt durch eingebettete Schrift");
    }
    if let Some(rest) = message.strip_prefix("file not found (searched at ") {
        let path = rest.strip_suffix(')').unwrap_or(rest);
        hints.push(message.to_owned());
        return format!("Datei „{path}“ fehlt im Werk (Bilder lädt man auf der Seite „Bilder“ hoch)");
    }
    message.to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn civil_dates_follow_the_gregorian_calendar() {
        assert_eq!(civil_from_days(0), (1970, 1, 1), "Tag 0 ist der 1. Januar 1970");
        assert_eq!(civil_from_days(19_782), (2024, 2, 29), "Schalttag 2024 muss stimmen");
        assert_eq!(civil_from_days(-1), (1969, 12, 31), "Tage vor 1970 zählen rückwärts");
    }

    #[test]
    fn a_picture_whose_content_does_not_fit_its_extension_gets_an_explicit_format() {
        let png = b"\x89PNG\r\n\x1a\n....".to_vec();
        let files: HashMap<String, Bytes> = [
            ("bilder/haus.svg".to_owned(), Bytes::new(png)),
            ("bilder/ok.svg".to_owned(), Bytes::new(b"<svg xmlns=\"x\"/>".to_vec())),
        ]
        .into_iter()
        .collect();
        let main = "#figure(image(\"/bilder/haus.svg\", width: 4cm))\n#image(\"bilder/ok.svg\")\n\
                    #image( \"bilder/haus.svg\", format: \"svg\")\n#myimage(\"bilder/haus.svg\")\n";
        let out = with_image_formats(main, &files).expect("eine Datei passt nicht zur Endung");
        assert_eq!(
            out,
            "#figure(image(\"/bilder/haus.svg\", format: \"png\", width: 4cm))\n#image(\"bilder/ok.svg\")\n\
             #image( \"bilder/haus.svg\", format: \"svg\")\n#myimage(\"bilder/haus.svg\")\n",
            "nur der passende Aufruf ohne eigenes format bekommt es; andere Bezeichner und passende Dateien bleiben"
        );
        assert_eq!(out.lines().count(), main.lines().count(), "keine neue Zeile (Meldungen behalten ihre Zeile)");
        let fitting: HashMap<String, Bytes> =
            [("a.png".to_owned(), Bytes::new(b"\x89PNG\r\n\x1a\n".to_vec()))].into_iter().collect();
        assert_eq!(with_image_formats("#image(\"a.png\")", &fitting), None, "alles passt: Quelle unverändert");
    }

    #[test]
    fn paths_are_compared_in_normal_form() {
        assert_eq!(normalize_path("/bilder/x.svg").as_deref(), Some("bilder/x.svg"), "führendes / fällt weg");
        assert_eq!(normalize_path("./bilder//x.svg").as_deref(), Some("bilder/x.svg"), "./ und // fallen weg");
        assert_eq!(normalize_path("../x.svg"), None, "aus dem Werk hinaus gibt es nicht");
        assert_eq!(normalize_path(" "), None, "leerer Pfad hat keine Normalform");
    }
}
