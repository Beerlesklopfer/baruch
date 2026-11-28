// SPDX-License-Identifier: AGPL-3.0-or-later
//! Cover, front matter and spine as Typst source (contract Round 11, section 3, task cover).
//!
//! Pure, without database: the server delivers the book details ([`BookInfo`]) and the virtual paths of the images
//! ([`VIRTUAL_DIR`]), this crate writes Typst from them.
//!
//! - [`frame_parts`]: what goes into the typeset book before the first section (`front`) and after the last
//!   (`back`). `front`: cover front (full-bleed image, `margin: 0pt`, title and subtitle in the chosen area, light
//!   text on a dark gradient), then the front matter (half title, page 2 empty or series title, main title,
//!   imprint, dedication on request). `back`: cover back (blurb, short biography with photo, ISBN with barcode or
//!   placeholder, publisher and price). Both contain no heading and no table of contents, so that page mapping and
//!   outline of the book stay untouched; none of these pages shows a page number.
//! - [`wrap_source`]: the whole cover as **one** page for the print shop: back | spine (text rotated) | front, with
//!   bleed all around.
//! - User texts stand **only** as Typst string literals in the source ([`typst_string`]: `\\`, `\"`, line break
//!   escaped), never as markup: `#`, `*`, `]`, `$` in a title are set literally.
//! - [`isbn13`] checks an ISBN-13 including check digit, [`ean13_svg`] draws its barcode (EAN-13, 95 modules and
//!   quiet zones), [`spine_mm`] calculates the spine width.
//! - **Round 27, layout:** with `layout`, [`frame_parts`] and [`wrap_source`] set the text of the cover from blocks
//!   ([`crate::cover_layout`]): `place(top + left, dx, dy, block(width, …))` relative to the trim edge of the face,
//!   with size, leading, colour, font, weight, alignment, shadow or gradient; rotated on the spine (`down` 90°,
//!   `up` −90°). [`block_measure_source`] sets every block on a page of its own with `height: auto` – this is how
//!   the server measures the height with Typst.
//! - **Round 27, front matter from the work template:** [`template_functions`] recognises `schmutztitel`,
//!   `haupttitel`, `impressum`, `widmung`; [`frame_parts`] then writes [`book_dict`] once (`#let baruch_buch =
//!   (…)`, only string literals) and calls the functions on pages of their own at a fixed place.
//!   [`imprint_template`] is the template for new books, [`imprint_template_missing`] appends only what is missing.
//!   The base font of the template is read by `baruch_wysiwyg::template_font` (top level, last rule wins).
//! - **Round 20c, text rules of the work template:** [`template_text_rules`] reads the `#set text(…)` and `#set
//!   par(…)` rules of the top level (only language, hyphenation, tracking, spacings, costs, justification, leading,
//!   indents – no font, size or colour, those are determined by the blocks or the dark ground) with simple values
//!   (numbers with unit, strings, `true`/`false`/`auto`/`none`, dictionaries of those; values with variables of the
//!   template are dropped, because the cover does not include the template). [`wrap_source_with_rules`] and
//!   [`block_measure_source_with_rules`] set them after `#set text(lang: "de")`; without rules they are
//!   byte-identical to [`wrap_source`] and [`block_measure_source`] (golden files R26/R27).

/// Folder of the virtual files the server adds to the typesetting (cover, photo, barcode): `.baruch/<image name>`,
/// barcode `.baruch/isbn.svg`.
pub const VIRTUAL_DIR: &str = ".baruch";

/// Virtual path of the barcode.
pub const BARCODE_PATH: &str = ".baruch/isbn.svg";

/// First line of [`FrameParts::front`]; by it the server recognises that the frame already stands in the typesetting.
pub const FRAME_MARK: &str = "// Baruch: Umschlag und Titelei (Runde 11)";

/// Quiet zone left and right of the barcode in modules (EAN-13: 11 left, 7 right).
pub const EAN_QUIET_LEFT: usize = 11;
pub const EAN_QUIET_RIGHT: usize = 7;

/// Where title and subtitle stand on the front.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TitleArea {
    #[default]
    Top,
    Middle,
    Bottom,
}

/// Details of the book for cover and front matter; empty fields are dropped.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BookInfo {
    pub title: String,
    pub subtitle: String,
    pub author: String,
    pub publisher: String,
    pub place: String,
    pub year: String,
    pub edition: String,
    pub isbn: String,
    pub price: String,
    pub series: String,
    pub printer: String,
    /// Bible translation (e.g. „Lutherbibel 2017“).
    pub bible: String,
    /// Rights notice of the Bible translation.
    pub bible_notice: String,
    /// Image credit or cover design (without a value the server inserts the caption of the cover image).
    pub image_credit: String,
    pub dedication: String,
    /// Blurb of the back.
    pub blurb: String,
    /// Short biography.
    pub bio: String,
    /// Address of the publisher or, with self-publishing, of the author or editor (Round 27, multi-line).
    pub publisher_address: String,
    /// Address of the print shop (Round 27, multi-line).
    pub printer_address: String,
}

/// Which front-matter functions the work template defines (Round 27, contract 1.6; Typst: `schmutztitel`,
/// `haupttitel`, `impressum`, `widmung`, each `#let name(buch)`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TemplateFunctions {
    pub half_title: bool,
    pub main_title: bool,
    pub imprint: bool,
    pub dedication: bool,
}

/// Cover image: virtual path (`.baruch/<name>`) and alternative text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoverRef {
    pub path: String,
    pub alt: String,
}

/// What [`frame_parts`] inserts.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FrameOptions {
    pub cover: Option<CoverRef>,
    /// Front and back into the typesetting (only effective with a cover).
    pub cover_pages: bool,
    /// Front matter (half title, page 2, main title, imprint, dedication).
    pub front_matter: bool,
    pub title_area: TitleArea,
    /// Virtual path of the barcode SVG (only with a valid ISBN).
    pub barcode: Option<String>,
    /// Virtual path of the author photo.
    pub author_photo: Option<String>,
    /// Own layout of the cover (Round 27); `None` or empty = previous appearance.
    pub layout: Option<crate::cover_layout::CoverLayout>,
    /// Front-matter functions of the work template (Round 27); none = fixed pages as before.
    pub template_functions: TemplateFunctions,
}

/// Typst before the first and after the last section.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FrameParts {
    pub front: String,
    pub back: String,
    /// Pages of the cover in the typesetting ([`COVER_PAGES`] or 0); the front matter does not count.
    pub added_cover_pages: usize,
}

/// Pages the cover brings into the typeset book: front, empty inside page, back. The inside page keeps the page
/// parity (the first chapter starts on the right as without a cover), and on it the page counter starts anew, so
/// that the page numbers of the book block do not depend on the cover.
pub const COVER_PAGES: usize = 3;

/// Dimensions of the whole cover for [`wrap_source`].
#[derive(Debug, Clone, PartialEq)]
pub struct WrapOptions {
    pub page_width_mm: f64,
    pub page_height_mm: f64,
    pub spine_mm: f64,
    pub bleed_mm: f64,
    pub title_area: TitleArea,
    pub cover: Option<CoverRef>,
    pub barcode: Option<String>,
    pub author_photo: Option<String>,
    /// Own layout of the cover (Round 27); `None` or empty = previous appearance.
    pub layout: Option<crate::cover_layout::CoverLayout>,
}

/// Name of the dictionary Baruch writes after [`FRAME_MARK`] if the work template defines front-matter functions
/// (Round 27, contract 1.6): `#let baruch_buch = (…)`.
pub const BOOK_DICT: &str = "baruch_buch";

use baruch_core::WorkLanguage;
use baruch_core::book_text::book_t;

use crate::cover_layout::{
    Backdrop, CoverAlign, CoverBlock, CoverBlockKind, CoverFace, CoverGeometry, CoverLayout, FontStyle, LEADING_EM,
    SIZE_PT, SpineDirection, WIDTH_MM, effective_blocks,
};

/// Colour of back and spine.
const DARK: &str = "rgb(\"#1f2a33\")";
/// Margins of a cover page (without bleed) in mm.
const PANEL_MARGIN_MM: f64 = 14.0;

// ---- Typst literals ------------------------------------------------------------------------------

/// Typst string literal: `"…"` with `\\`, `\"`, `\n`, `\t` escaped; `\r` and other control characters are dropped.
/// This way a user text is never read as markup or code.
pub fn typst_string(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Length in mm as a Typst value (`12.500mm`), never in exponent notation.
fn mm(value: f64) -> String {
    let value = if value.is_finite() { value.max(0.0) } else { 0.0 };
    format!("{value:.3}mm")
}

/// Multi-line text as paragraphs (an empty line separates), one string literal per paragraph; single line breaks
/// stay line breaks.
fn paragraphs(text: &str, indent: &str) -> String {
    let normalized = text.replace("\r\n", "\n");
    let mut out = String::new();
    for paragraph in normalized.split("\n\n").map(str::trim).filter(|p| !p.is_empty()) {
        out.push_str(&format!("{indent}par({})\n", typst_string(paragraph)));
    }
    out
}

fn filled(value: &str) -> Option<&str> {
    let value = value.trim();
    (!value.is_empty()).then_some(value)
}

/// Page without header, footer and page number, content as a code block.
fn plain_page(args: &str, body: &str) -> String {
    format!("#page(header: none, footer: none, numbering: none{args}, {{\n{body}}})\n")
}

// ---- Cover pages -----------------------------------------------------------------------------------

/// Title and subtitle of the front in the chosen area, light text on a dark gradient.
fn title_block(info: &BookInfo, area: TitleArea, indent: &str) -> String {
    let (align, fill) = match area {
        TitleArea::Top => {
            ("top + center", "gradient.linear(black.transparentize(30%), black.transparentize(100%), angle: 90deg)")
        }
        TitleArea::Middle => ("horizon + center", "black.transparentize(50%)"),
        TitleArea::Bottom => {
            ("bottom + center", "gradient.linear(black.transparentize(100%), black.transparentize(30%), angle: 90deg)")
        }
    };
    let mut body = String::new();
    if let Some(author) = filled(&info.author) {
        body.push_str(&format!("{indent}    text(size: 13pt, fill: white, {})\n", typst_string(author)));
        body.push_str(&format!("{indent}    v(0.8em)\n"));
    }
    body.push_str(&format!(
        "{indent}    text(size: 30pt, weight: \"bold\", fill: white, {})\n",
        typst_string(info.title.trim())
    ));
    if let Some(subtitle) = filled(&info.subtitle) {
        body.push_str(&format!("{indent}    v(0.5em)\n"));
        body.push_str(&format!("{indent}    text(size: 15pt, fill: white, {})\n", typst_string(subtitle)));
    }
    format!(
        "{indent}place({align}, block(width: 100%, inset: (x: 10%, y: 9%), fill: {fill}, {{\n\
         {indent}    set align(center)\n\
         {indent}    set par(leading: 0.5em)\n\
         {body}{indent}}}))\n"
    )
}

/// Content of the front (image or dark area, on it the title) in a block of size 100 %.
fn front_body(info: &BookInfo, cover: Option<&CoverRef>, area: TitleArea, indent: &str) -> String {
    let mut out = String::new();
    match cover {
        Some(cover) => out.push_str(&format!(
            "{indent}place(top + left, image({}, alt: {}, width: 100%, height: 100%, fit: \"cover\"))\n",
            typst_string(&cover.path),
            typst_string(&cover.alt)
        )),
        None => out.push_str(&format!("{indent}place(top + left, rect(width: 100%, height: 100%, fill: {DARK}))\n")),
    }
    out.push_str(&title_block(info, area, indent));
    out
}

/// Barcode with the ISBN below it, or a placeholder (without a valid ISBN).
fn barcode_block(info: &BookInfo, barcode: Option<&str>) -> String {
    match (barcode, filled(&info.isbn)) {
        (Some(path), Some(isbn)) => format!(
            "box(fill: white, inset: 3pt, {{ image({}, width: 36mm); align(center, text(size: 7.5pt, fill: black, {})) }})",
            typst_string(path),
            typst_string(&format!("ISBN {isbn}"))
        ),
        _ => "box(width: 36mm, height: 22mm, stroke: 0.5pt + white, align(center + horizon, text(size: 7.5pt, \"ISBN und Strichcode\")))".to_owned(),
    }
}

/// Content of the back: blurb, short biography (with photo), at the bottom publisher/price and barcode.
fn back_body(info: &BookInfo, photo: Option<&str>, barcode: Option<&str>, indent: &str) -> String {
    let mut out = format!("{indent}set text(fill: white, size: 10.5pt)\n{indent}set par(justify: true)\n");
    out.push_str(&paragraphs(&info.blurb, indent));
    out.push_str(&format!("{indent}v(1fr)\n"));
    if filled(&info.bio).is_some() || photo.is_some() {
        let photo = match photo {
            Some(path) => format!("image({}, width: 22mm)", typst_string(path)),
            None => "[]".to_owned(),
        };
        let bio: Vec<String> = paragraphs(&info.bio, "").lines().map(str::to_owned).collect();
        let bio = if bio.is_empty() { "[]".to_owned() } else { bio.join("; ") };
        out.push_str(&format!(
            "{indent}grid(columns: (auto, 1fr), column-gutter: 4mm, {photo}, {{ set text(size: 9pt); {bio} }})\n"
        ));
        out.push_str(&format!("{indent}v(6mm)\n"));
    }
    let mut left = Vec::new();
    if let Some(publisher) = filled(&info.publisher) {
        left.push(format!("text(size: 9pt, {})", typst_string(publisher)));
    }
    if let Some(price) = filled(&info.price) {
        left.push(format!("text(size: 9pt, {})", typst_string(price)));
    }
    let left = if left.is_empty() { "[]".to_owned() } else { format!("{{ {} }}", left.join("; linebreak(); ")) };
    out.push_str(&format!(
        "{indent}grid(columns: (1fr, auto), align: (left + bottom, right + bottom), {left}, {})\n",
        barcode_block(info, barcode)
    ));
    out
}

// ---- Front matter ------------------------------------------------------------------------------

fn half_title(info: &BookInfo) -> String {
    plain_page("", &format!("  set align(center)\n  v(22%)\n  text(size: 16pt, {})\n", typst_string(info.title.trim())))
}

fn second_page(info: &BookInfo) -> String {
    match filled(&info.series) {
        Some(series) => plain_page(
            "",
            &format!(
                "  set align(center)\n  v(22%)\n  text(size: 11pt, style: \"italic\", {})\n",
                typst_string(series)
            ),
        ),
        // Empty page: an invisible content, so that Typst reliably emits the page.
        None => plain_page("", "  hide[ ]\n"),
    }
}

fn main_title(info: &BookInfo) -> String {
    let mut body = String::from("  set align(center)\n  v(12%)\n");
    if let Some(author) = filled(&info.author) {
        body.push_str(&format!("  text(size: 13pt, {})\n  v(2em)\n", typst_string(author)));
    }
    body.push_str(&format!("  text(size: 26pt, weight: \"bold\", {})\n", typst_string(info.title.trim())));
    if let Some(subtitle) = filled(&info.subtitle) {
        body.push_str(&format!("  v(1em)\n  text(size: 14pt, {})\n", typst_string(subtitle)));
    }
    body.push_str("  v(1fr)\n");
    if let Some(publisher) = filled(&info.publisher) {
        body.push_str(&format!("  text(size: 11pt, {})\n", typst_string(publisher)));
    }
    if let Some(place) = filled(&info.place) {
        body.push_str(&format!("  linebreak()\n  text(size: 10pt, {})\n", typst_string(place)));
    }
    plain_page("", &body)
}

/// Heading of the DNB notice in the imprint.
pub const DNB_TITLE: &str = "Bibliografische Information der Deutschen Nationalbibliothek";
/// Text of the DNB notice in the imprint.
pub const DNB_TEXT: &str = "Die Deutsche Nationalbibliothek verzeichnet diese Publikation in der Deutschen \
     Nationalbibliografie; detaillierte bibliografische Daten sind im Internet über https://dnb.de abrufbar.";
/// Typesetting note in the imprint.
pub const TYPESET_WITH: &str = "Gesetzt mit Baruch";

/// The © line of the imprint: year, author (otherwise publisher), publisher, place.
pub fn copyright_line(info: &BookInfo) -> String {
    let holder = filled(&info.author).or(filled(&info.publisher)).unwrap_or("");
    let mut copyright = match (filled(&info.year), holder) {
        (Some(year), "") => format!("© {year}"),
        (Some(year), holder) => format!("© {year} {holder}"),
        (None, "") => "©".to_owned(),
        (None, holder) => format!("© {holder}"),
    };
    if let (Some(publisher), true) = (filled(&info.publisher), filled(&info.author).is_some()) {
        copyright.push_str(&format!(", {publisher}"));
    }
    if let Some(place) = filled(&info.place) {
        copyright.push_str(&format!(", {place}"));
    }
    copyright
}

/// Multi-line address in one line („Weg 1, 12345 Ort“).
fn one_line(address: &str) -> Option<String> {
    let parts: Vec<&str> = address.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
    (!parts.is_empty()).then(|| parts.join(", "))
}

/// Lines of the imprint (text, already without markup). The addresses (Round 27) are only added if they are set:
/// that of the publisher as a line of its own after the © line, that of the print shop behind „Druck:“ – without
/// addresses the lines stay as before Round 27.
pub fn imprint_lines(info: &BookInfo) -> Vec<String> {
    imprint_lines_in(info, WorkLanguage::De)
}

/// Like [`imprint_lines`] in the work language `lang` (Round 39b): DNB notice, „Bibelzitate“, cover credit, „Gesetzt
/// mit Baruch“ and „Druck“ from the book catalog; German exactly as before.
pub fn imprint_lines_in(info: &BookInfo, lang: WorkLanguage) -> Vec<String> {
    let t = |key: &str, args: &[(&str, &str)]| book_t(lang, key, args);
    let mut lines = vec![t("book.imprint.dnb_title", &[]), t("book.imprint.dnb_text", &[])];
    lines.push(copyright_line(info));
    if let Some(address) = one_line(&info.publisher_address) {
        lines.push(address);
    }
    if let Some(edition) = filled(&info.edition) {
        lines.push(edition.to_owned());
    }
    if let Some(isbn) = filled(&info.isbn) {
        lines.push(format!("ISBN {isbn}"));
    }
    match (filled(&info.bible), filled(&info.bible_notice)) {
        (Some(bible), Some(notice)) => lines.push(format!("{} {notice}", t("book.imprint.bible", &[("bible", bible)]))),
        (Some(bible), None) => lines.push(t("book.imprint.bible", &[("bible", bible)])),
        (None, Some(notice)) => lines.push(notice.to_owned()),
        (None, None) => {}
    }
    if let Some(credit) = filled(&info.image_credit) {
        lines.push(t("book.imprint.cover_credit", &[("credit", credit)]));
    }
    lines.push(t("book.imprint.typeset_with", &[]));
    match (filled(&info.printer), one_line(&info.printer_address)) {
        (Some(printer), Some(address)) => {
            lines.push(t("book.imprint.printer", &[("printer", &format!("{printer}, {address}"))]))
        }
        (Some(printer), None) => lines.push(t("book.imprint.printer", &[("printer", printer)])),
        (None, Some(address)) => lines.push(t("book.imprint.printer", &[("printer", &address)])),
        (None, None) => {}
    }
    lines
}

/// The cover and front-matter Typst written for a German work, with the generated words in the work language
/// `lang` (Round 39b): language rule, DNB notice, „Gesetzt mit Baruch“, the starts of the cover credit, Bible and
/// print-shop lines and the ISBN placeholder. The German words only ever stand there as whole string literals
/// written by this module (user texts never start a literal with them in practice), so replacing those literals is
/// exact. German: unchanged.
fn localized(source: String, lang: WorkLanguage) -> String {
    if lang == WorkLanguage::De {
        return source;
    }
    let both = |key: &str| (typst_string(&book_t(WorkLanguage::De, key, &[])), typst_string(&book_t(lang, key, &[])));
    // Literal start up to the placeholder: `"Druck: ` for `Druck: { $printer }`.
    let start = |key: &str, name: &str| {
        let slot = "\u{1}";
        let cut = |language| {
            let text = book_t(language, key, &[(name, slot)]);
            let before = text.split_once(slot).map_or(text.as_str(), |(before, _)| before).to_owned();
            let literal = typst_string(&before);
            literal[..literal.len() - 1].to_owned()
        };
        (cut(WorkLanguage::De), cut(lang))
    };
    let pairs = [
        ("#set text(lang: \"de\")".to_owned(), format!("#set text(lang: \"{}\")", lang.as_str())),
        both("book.imprint.dnb_title"),
        both("book.imprint.dnb_text"),
        both("book.imprint.typeset_with"),
        both("book.cover.isbn_placeholder"),
        start("book.imprint.cover_credit", "credit"),
        start("book.imprint.bible", "bible"),
        start("book.imprint.printer", "printer"),
    ];
    let mut out = source;
    for (german, other) in pairs {
        out = out.replace(&german, &other);
    }
    out
}

fn imprint(info: &BookInfo) -> String {
    let mut body = String::from("  set text(size: 8.5pt)\n  set par(justify: false)\n  v(1fr)\n");
    for (i, line) in imprint_lines(info).iter().enumerate() {
        let weight = if i == 0 { ", weight: \"bold\"" } else { "" };
        body.push_str(&format!("  block(spacing: 0.8em, text({}{weight}))\n", typst_string(line)));
    }
    plain_page("", &body)
}

fn dedication(info: &BookInfo) -> Option<String> {
    let text = filled(&info.dedication)?;
    let mut body = String::from("  set align(center)\n  v(25%)\n  set text(style: \"italic\")\n");
    body.push_str(&paragraphs(text, "  "));
    // Afterwards an empty page, so that the first chapter starts on a right-hand page again.
    Some(format!("{}{}", plain_page("", &body), plain_page("", "  hide[ ]\n")))
}

/// Typst before the first and after the last section. Both empty (and `added_cover_pages` 0) if neither a cover
/// with `cover_pages` nor the front matter is wanted.
///
/// Round 27: with a non-empty `opts.layout` front and back set **only** the blocks of this layout (visible and with
/// content), each relative to the trim edge of the page (in the book W × H with margin 0). The server adds missing
/// kinds beforehand with [`crate::cover_layout::effective_blocks`], because only it knows the page size. If the
/// work template defines front-matter functions (`opts.template_functions`, only effective with front matter),
/// Baruch writes the dictionary [`BOOK_DICT`] once after [`FRAME_MARK`] and sets every function on a page of its
/// own at the fixed place; if a function is missing, the previous page stands there. Without a layout and without
/// functions the output is byte-identical to Round 26.
///
/// Round 39b: the generated words in the work language `lang` (German: byte-identical to the function without
/// `_in`).
pub fn frame_parts_in(info: &BookInfo, opts: &FrameOptions, lang: WorkLanguage) -> FrameParts {
    let parts = frame_parts(info, opts);
    FrameParts { front: localized(parts.front, lang), back: localized(parts.back, lang), ..parts }
}

/// Frame of a German work (exactly as before Round 39b); [`frame_parts_in`] for the work language.
pub fn frame_parts(info: &BookInfo, opts: &FrameOptions) -> FrameParts {
    let cover = opts.cover.as_ref().filter(|_| opts.cover_pages);
    if cover.is_none() && !opts.front_matter {
        return FrameParts::default();
    }
    let layout = opts.layout.as_ref().filter(|l| !l.is_empty());
    let functions = if opts.front_matter { opts.template_functions } else { TemplateFunctions::default() };
    let uses_functions = functions.half_title || functions.main_title || functions.imprint || functions.dedication;
    let mut front = format!("{FRAME_MARK}\n");
    if uses_functions {
        front.push_str(&book_dict(info));
    }
    let mut back = String::new();
    if let Some(cover) = cover {
        front.push_str("// Umschlag, Vorderseite\n");
        let body = match layout {
            None => front_body(info, Some(cover), opts.title_area, "  "),
            Some(layout) => {
                let mut body = cover_background(Some(cover), "  ");
                body.push_str(&placed_blocks(info, layout, CoverFace::Front, (0.0, 0.0), &BlockRefs::default(), "  "));
                body
            }
        };
        front.push_str(&plain_page(", margin: 0pt", &body));
        // Inside page: empty; page 0 here, so that the first page after it is page 1 (Typst counts every page, also
        // without a number). Physically page 2 – the first page after it lies on the right as without a cover.
        front.push_str("// Umschlag, Innenseite (leer)\n");
        front.push_str(&plain_page("", "  counter(page).update(0)\n  hide[ ]\n"));
        back.push_str("// Baruch: Umschlag, Rückseite (Runde 11)\n");
        match layout {
            None => {
                let body = back_body(info, opts.author_photo.as_deref(), opts.barcode.as_deref(), "    ");
                back.push_str(&plain_page(
                    &format!(", margin: 0pt, fill: {DARK}"),
                    &format!(
                        "  pad(x: {}, y: {}, block(width: 100%, height: 100%, {{\n{body}  }}))\n",
                        mm(PANEL_MARGIN_MM),
                        mm(PANEL_MARGIN_MM)
                    ),
                ));
            }
            Some(layout) => {
                let refs = BlockRefs { photo: opts.author_photo.as_deref(), barcode: opts.barcode.as_deref() };
                let body = placed_blocks(info, layout, CoverFace::Back, (0.0, 0.0), &refs, "  ");
                let body = if body.is_empty() { "  hide[ ]\n".to_owned() } else { body };
                back.push_str(&plain_page(&format!(", margin: 0pt, fill: {DARK}"), &body));
            }
        }
    }
    if opts.front_matter {
        front.push_str("// Titelei\n");
        front.push_str(&if functions.half_title { function_page("schmutztitel") } else { half_title(info) });
        front.push_str(&second_page(info));
        front.push_str(&if functions.main_title { function_page("haupttitel") } else { main_title(info) });
        front.push_str(&if functions.imprint { function_page("impressum") } else { imprint(info) });
        if functions.dedication {
            // Like the fallback only with a dedication; afterwards the empty page for the parity.
            if filled(&info.dedication).is_some() {
                front.push_str(&function_page("widmung"));
                front.push_str(&plain_page("", "  hide[ ]\n"));
            }
        } else if let Some(page) = dedication(info) {
            front.push_str(&page);
        }
    }
    FrameParts { front, back, added_cover_pages: if cover.is_some() { COVER_PAGES } else { 0 } }
}

/// One page of the front matter from a function of the work template: `name(baruch_buch)` as content.
fn function_page(name: &str) -> String {
    plain_page("", &format!("  {name}({BOOK_DICT})\n"))
}

/// The whole cover on one page: back | spine | front, with bleed all around. Width = 2 × page width + spine + 2 ×
/// bleed, height = page height + 2 × bleed.
///
/// Round 27: with a non-empty `opts.layout` the faces (image or dark area) stay as before, but text only comes from
/// the blocks; missing kinds are added by [`crate::cover_layout::effective_blocks`]. Every block stands relative to
/// the trim edge of its face ([`CoverGeometry::face_origin`]) and may reach into the bleed.
pub fn wrap_source(info: &BookInfo, opts: &WrapOptions) -> String {
    wrap_source_with_rules(info, opts, "")
}

/// Like [`wrap_source`], with the text rules of the work template (`rules` from [`template_text_rules`], empty =
/// none) directly after `#set text(lang: "de")` (Round 20c): this way a blurb on the cover for the print shop
/// breaks like on the cover page in the book.
///
/// Round 39b: the generated words in the work language `lang` (German: byte-identical to the function without
/// `_in`).
pub fn wrap_source_with_rules_in(info: &BookInfo, opts: &WrapOptions, rules: &str, lang: WorkLanguage) -> String {
    localized(wrap_source_with_rules(info, opts, rules), lang)
}

/// Cover for the print shop of a German work (exactly as before Round 39b); [`wrap_source_with_rules_in`] for the
/// work language.
pub fn wrap_source_with_rules(info: &BookInfo, opts: &WrapOptions, rules: &str) -> String {
    let bleed = opts.bleed_mm.max(0.0);
    let panel = opts.page_width_mm.max(1.0) + bleed;
    let spine = opts.spine_mm.max(0.0);
    let width = 2.0 * panel + spine;
    let height = opts.page_height_mm.max(1.0) + 2.0 * bleed;
    let inner = PANEL_MARGIN_MM + bleed;
    let mut out = format!(
        "// Baruch: Umschlag für die Druckerei (Runde 11) – Rückseite | Rücken | Vorderseite, Beschnitt {}\n\
         #set page(width: {}, height: {}, margin: 0pt)\n#set text(lang: \"de\")\n",
        mm(bleed),
        mm(width),
        mm(height)
    );
    out.push_str(rules);
    if let Some(layout) = opts.layout.as_ref().filter(|l| !l.is_empty()) {
        let geometry = CoverGeometry::new(opts.page_width_mm.max(1.0), opts.page_height_mm.max(1.0), spine, bleed);
        let blocks: Vec<CoverBlock> =
            effective_blocks(layout, opts.title_area, &geometry).into_iter().map(|(b, _)| b).collect();
        let full = CoverLayout { blocks };
        out.push_str(&format!(
            "#place(top + left, block(width: {}, height: {}, fill: {DARK}))\n",
            mm(panel),
            mm(height)
        ));
        out.push_str(&format!(
            "#place(top + left, dx: {}, block(width: {}, height: {}, fill: {DARK}))\n",
            mm(panel),
            mm(spine),
            mm(height)
        ));
        let front = cover_background(opts.cover.as_ref(), "  ");
        out.push_str(&format!(
            "#place(top + left, dx: {}, block(width: {}, height: {}, clip: true, {{\n{front}}}))\n",
            mm(panel + spine),
            mm(panel),
            mm(height)
        ));
        let refs = BlockRefs { photo: opts.author_photo.as_deref(), barcode: opts.barcode.as_deref() };
        for face in [CoverFace::Back, CoverFace::Spine, CoverFace::Front] {
            let placed = placed_blocks(info, &full, face, geometry.face_origin(face), &refs, "#");
            out.push_str(&placed);
        }
        return out;
    }
    // Back
    let back = back_body(info, opts.author_photo.as_deref(), opts.barcode.as_deref(), "  ");
    out.push_str(&format!(
        "#place(top + left, block(width: {}, height: {}, fill: {DARK}, clip: true, \
         inset: (left: {}, right: {}, top: {}, bottom: {}), {{\n{back}}}))\n",
        mm(panel),
        mm(height),
        mm(inner),
        mm(PANEL_MARGIN_MM),
        mm(inner),
        mm(inner)
    ));
    // Spine: from 6 mm on with title and author, rotated (to be read from top to bottom).
    let spine_text = if spine >= 6.0 {
        let label = match filled(&info.author) {
            Some(author) => format!("{} · {}", author, info.title.trim()),
            None => info.title.trim().to_owned(),
        };
        let size = (spine * 0.45 * 2.835).min(11.0);
        format!(
            "align(center + horizon, rotate(90deg, reflow: true, text(size: {size:.2}pt, fill: white, {})))",
            typst_string(&label)
        )
    } else {
        "[]".to_owned()
    };
    out.push_str(&format!(
        "#place(top + left, dx: {}, block(width: {}, height: {}, fill: {DARK}, clip: true, {spine_text}))\n",
        mm(panel),
        mm(spine),
        mm(height)
    ));
    // Front
    let front = front_body(info, opts.cover.as_ref(), opts.title_area, "  ");
    out.push_str(&format!(
        "#place(top + left, dx: {}, block(width: {}, height: {}, clip: true, {{\n{front}}}))\n",
        mm(panel + spine),
        mm(panel),
        mm(height)
    ));
    out
}

/// Only the **front** of the cover on one page in the final format (`page_width_mm` × `page_height_mm`, without
/// bleed) – the template of the EPUB cover (Round 21c, the server rasterises it with `baruch_render::render_png`).
/// Content like the front in [`wrap_source_with_rules`]: the cover image filling (`fit: "cover"`) or, without an
/// image, the dark area, on it with a non-empty `opts.layout` the visible blocks of the front (title, subtitle,
/// author, publisher, free text; missing kinds as default blocks), otherwise title, subtitle and author in the
/// title area as before. `spine_mm`, `barcode` and `author_photo` play no role; `bleed_mm` only for the default
/// blocks (as in the typeset book). User texts only as string literals.
///
/// Round 39b: the generated words in the work language `lang` (German: byte-identical to the function without
/// `_in`).
pub fn front_source_with_rules_in(info: &BookInfo, opts: &WrapOptions, rules: &str, lang: WorkLanguage) -> String {
    localized(front_source_with_rules(info, opts, rules), lang)
}

/// Cover front of a German work (exactly as before Round 39b); [`front_source_with_rules_in`] for the work language.
pub fn front_source_with_rules(info: &BookInfo, opts: &WrapOptions, rules: &str) -> String {
    let width = opts.page_width_mm.max(1.0);
    let height = opts.page_height_mm.max(1.0);
    let mut out = format!(
        "// Baruch: Umschlag-Vorderseite als EPUB-Cover (Runde 21c)\n\
         #set page(width: {}, height: {}, margin: 0pt)\n#set text(lang: \"de\")\n",
        mm(width),
        mm(height)
    );
    out.push_str(rules);
    match opts.layout.as_ref().filter(|l| !l.is_empty()) {
        Some(layout) => {
            let geometry = CoverGeometry::new(width, height, 0.0, opts.bleed_mm.max(0.0));
            let blocks: Vec<CoverBlock> =
                effective_blocks(layout, opts.title_area, &geometry).into_iter().map(|(b, _)| b).collect();
            let full = CoverLayout { blocks };
            let front = cover_background(opts.cover.as_ref(), "  ");
            out.push_str(&format!("#place(top + left, block(width: 100%, height: 100%, clip: true, {{\n{front}}}))\n"));
            out.push_str(&placed_blocks(info, &full, CoverFace::Front, (0.0, 0.0), &BlockRefs::default(), "#"));
        }
        None => {
            let front = front_body(info, opts.cover.as_ref(), opts.title_area, "  ");
            out.push_str(&format!("#place(top + left, block(width: 100%, height: 100%, clip: true, {{\n{front}}}))\n"));
        }
    }
    out
}

// ---- Text rules of the work template (Round 20c) ----------------------------------------------------

/// Arguments of `#set text(…)` the cover takes over (no font, size, colour).
const TEXT_RULE_ARGS: [&str; 10] =
    ["lang", "region", "script", "dir", "hyphenate", "tracking", "spacing", "costs", "kerning", "ligatures"];
/// Arguments of `#set par(…)` the cover takes over.
const PAR_RULE_ARGS: [&str; 6] = ["justify", "leading", "spacing", "first-line-indent", "hanging-indent", "linebreaks"];
/// Words that may stand in a taken-over value (otherwise it would be a variable of the template).
const RULE_WORDS: [&str; 4] = ["true", "false", "auto", "none"];

/// Replace comments (`// …`, `/* … */`) by spaces, strings stay (byte positions the same).
fn blank_comments(template: &str) -> String {
    let bytes = template.as_bytes();
    let mut out = bytes.to_vec();
    let mut i = 0;
    let mut in_string = false;
    while i < bytes.len() {
        let b = bytes[i];
        if in_string {
            match b {
                b'\\' => i += 1,
                b'"' | b'\n' => in_string = false,
                _ => {}
            }
            i += 1;
            continue;
        }
        match (b, bytes.get(i + 1)) {
            (b'"', _) => in_string = true,
            (b'/', Some(b'/')) => {
                while i < bytes.len() && bytes[i] != b'\n' {
                    out[i] = b' ';
                    i += 1;
                }
                continue;
            }
            (b'/', Some(b'*')) => {
                let mut depth = 0usize;
                while i < bytes.len() {
                    if bytes[i] == b'/' && bytes.get(i + 1) == Some(&b'*') {
                        depth += 1;
                        out[i] = b' ';
                        out[i + 1] = b' ';
                        i += 2;
                        continue;
                    }
                    if bytes[i] == b'*' && bytes.get(i + 1) == Some(&b'/') {
                        depth -= 1;
                        out[i] = b' ';
                        out[i + 1] = b' ';
                        i += 2;
                        if depth == 0 {
                            break;
                        }
                        continue;
                    }
                    if bytes[i] != b'\n' {
                        out[i] = b' ';
                    }
                    i += 1;
                }
                continue;
            }
            _ => {}
        }
        i += 1;
    }
    // Multi-byte characters in comments become spaces byte by byte; the rest stays valid UTF-8.
    String::from_utf8_lossy(&out).into_owned()
}

/// End (exclusive) of the parenthesis that starts at `open`; strings do not count.
fn closing_paren(text: &str, open: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut depth = 0usize;
    let mut in_string = false;
    let mut i = open;
    while i < bytes.len() {
        let b = bytes[i];
        if in_string {
            match b {
                b'\\' => i += 1,
                b'"' => in_string = false,
                _ => {}
            }
        } else {
            match b {
                b'"' => in_string = true,
                b'(' | b'[' | b'{' => depth += 1,
                b')' | b']' | b'}' => {
                    depth = depth.checked_sub(1)?;
                    if depth == 0 {
                        return Some(i + 1);
                    }
                }
                _ => {}
            }
        }
        i += 1;
    }
    None
}

/// Split arguments at parenthesis depth 0.
fn split_args(inner: &str) -> Vec<&str> {
    let bytes = inner.as_bytes();
    let (mut depth, mut in_string, mut start) = (0usize, false, 0usize);
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if in_string {
            match b {
                b'\\' => i += 1,
                b'"' => in_string = false,
                _ => {}
            }
        } else {
            match b {
                b'"' => in_string = true,
                b'(' | b'[' | b'{' => depth += 1,
                b')' | b']' | b'}' => depth = depth.saturating_sub(1),
                b',' if depth == 0 => {
                    out.push(&inner[start..i]);
                    start = i + 1;
                }
                _ => {}
            }
        }
        i += 1;
    }
    out.push(&inner[start..]);
    out.into_iter().map(str::trim).filter(|a| !a.is_empty()).collect()
}

/// A value of numbers (with unit), strings, `true`/`false`/`auto`/`none` and dictionaries or arrays of those –
/// without variables, function calls or content blocks.
fn simple_value(value: &str) -> bool {
    let bytes = value.as_bytes();
    if value.trim().is_empty() || value.contains(['[', ']', '{', '}', '#', '\n']) {
        return false;
    }
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'"' => {
                i += 1;
                while i < bytes.len() && bytes[i] != b'"' {
                    if bytes[i] == b'\\' {
                        i += 1;
                    }
                    i += 1;
                }
                if i >= bytes.len() {
                    return false;
                }
                i += 1;
            }
            b'0'..=b'9' | b'.' => {
                while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'.' || bytes[i] == b'%') {
                    i += 1;
                }
            }
            b'a'..=b'z' | b'A'..=b'Z' | b'_' => {
                let start = i;
                while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || matches!(bytes[i], b'_' | b'-')) {
                    i += 1;
                }
                let word = &value[start..i];
                let key = value[i..].trim_start().starts_with(':');
                if !key && !RULE_WORDS.contains(&word) {
                    return false;
                }
            }
            b' ' | b'\t' | b'(' | b')' | b',' | b':' | b'+' | b'-' | b'*' | b'/' => i += 1,
            _ => return false,
        }
    }
    true
}

/// The text and paragraph rules of the top level of the work template as Typst lines (`#set text(…)`, `#set
/// par(…)`), only with the taken-over arguments and simple values (see module documentation); no rule → "". The
/// order of the template stays (the last rule wins as in the book). Conditional rules (`… if …`) and indented rules
/// (in functions, blocks) do not count.
pub fn template_text_rules(template: &str) -> String {
    let clean = blank_comments(template);
    let mut out = String::new();
    let mut line_start = 0usize;
    let mut lang = "\"de\"".to_owned();
    for line in clean.split_inclusive('\n') {
        let start = line_start;
        line_start += line.len();
        let (element, allowed): (&str, &[&str]) = if line.starts_with("#set text(") {
            ("text", &TEXT_RULE_ARGS)
        } else if line.starts_with("#set par(") {
            ("par", &PAR_RULE_ARGS)
        } else {
            continue;
        };
        let open = start + "#set ".len() + element.len();
        let Some(close) = closing_paren(&clean, open) else { continue };
        if !clean[close..].split('\n').next().unwrap_or("").trim().is_empty() {
            continue;
        }
        let mut kept: Vec<String> = Vec::new();
        for arg in split_args(&clean[open + 1..close - 1]) {
            let Some((name, value)) = arg.split_once(':') else { continue };
            let (name, value) = (name.trim(), value.trim());
            if !allowed.contains(&name) || !simple_value(value) {
                continue;
            }
            // Do not set the language that applies anyway (initially "de") once more: this way works with `#set
            // text(lang: "de")` stay byte-identical to the golden files.
            if element == "text" && name == "lang" {
                if value == lang {
                    continue;
                }
                lang = value.to_owned();
            }
            kept.push(format!("{name}: {value}"));
        }
        if !kept.is_empty() {
            out.push_str(&format!("#set {element}({})\n", kept.join(", ")));
        }
    }
    out
}

// ---- Blocks of the layout (Round 27) ---------------------------------------------------------------

/// Virtual paths for the blocks with an image (short biography with photo, barcode).
#[derive(Debug, Clone, Copy, Default)]
pub struct BlockRefs<'a> {
    pub photo: Option<&'a str>,
    pub barcode: Option<&'a str>,
}

/// Length in mm with sign (blocks may reach into the bleed), never in exponent notation.
fn signed_mm(value: f64) -> String {
    let value = if value.is_finite() { value } else { 0.0 };
    let text = format!("{value:.3}mm");
    if text == "-0.000mm" { "0.000mm".to_owned() } else { text }
}

/// `#rrggbb` with six hex digits?
pub fn is_hex_color(color: &str) -> bool {
    color.len() == 7 && color.starts_with('#') && color[1..].chars().all(|c| c.is_ascii_hexdigit())
}

/// Typst colour of a block: `light` white, `dark` like back and spine, `#rrggbb` as `rgb("…")`; everything else white.
pub fn block_color(color: &str) -> String {
    match color.trim() {
        "dark" => DARK.to_owned(),
        hex if is_hex_color(hex) => format!("rgb({})", typst_string(&hex.to_ascii_lowercase())),
        _ => "white".to_owned(),
    }
}

/// Background of the front: image (filling, `fit: "cover"`) or dark area.
fn cover_background(cover: Option<&CoverRef>, indent: &str) -> String {
    match cover {
        Some(cover) => format!(
            "{indent}place(top + left, image({}, alt: {}, width: 100%, height: 100%, fit: \"cover\"))\n",
            typst_string(&cover.path),
            typst_string(&cover.alt)
        ),
        None => format!("{indent}place(top + left, rect(width: 100%, height: 100%, fill: {DARK}))\n"),
    }
}

/// Content of a block (Typst code without frame) or `None` if it has nothing to show (empty text). The barcode
/// shows the placeholder without a valid ISBN. User texts only as string literals.
fn block_content(info: &BookInfo, block: &CoverBlock, refs: &BlockRefs<'_>) -> Option<String> {
    let single = |text: &str| filled(text).map(|t| format!("text({})", typst_string(t)));
    let paras = |text: &str| {
        let out: Vec<String> = paragraphs(text, "").lines().map(str::to_owned).collect();
        (!out.is_empty()).then(|| out.join("; "))
    };
    match block.kind {
        CoverBlockKind::FrontTitle | CoverBlockKind::SpineTitle => single(&info.title),
        CoverBlockKind::FrontSubtitle => single(&info.subtitle),
        CoverBlockKind::FrontAuthor | CoverBlockKind::SpineAuthor => single(&info.author),
        CoverBlockKind::FrontPublisher | CoverBlockKind::SpinePublisher => single(&info.publisher),
        CoverBlockKind::FrontFree
        | CoverBlockKind::SpineFree
        | CoverBlockKind::BackFree
        | CoverBlockKind::BackMotto => paras(block.text.as_deref().unwrap_or("")),
        CoverBlockKind::BackBlurb => paras(&info.blurb),
        CoverBlockKind::BackBio => {
            let bio = paras(&info.bio);
            match (refs.photo, bio) {
                (None, None) => None,
                (None, Some(bio)) => Some(bio),
                (Some(path), bio) => Some(format!(
                    "grid(columns: (auto, 1fr), column-gutter: 4mm, image({}, width: 22mm), {{ {} }})",
                    typst_string(path),
                    bio.unwrap_or_else(|| "[]".to_owned())
                )),
            }
        }
        CoverBlockKind::BackBarcode => Some(barcode_block(info, refs.barcode)),
        CoverBlockKind::BackPublisherPrice => {
            let parts: Vec<String> = [filled(&info.publisher), filled(&info.price)]
                .into_iter()
                .flatten()
                .map(|t| format!("text({})", typst_string(t)))
                .collect();
            (!parts.is_empty()).then(|| parts.join("; linebreak(); "))
        }
    }
}

/// The block without position: `block(width: …, {…})` with font, paragraph, alignment and on request background.
/// `None` like [`block_content`].
fn block_body(info: &BookInfo, block: &CoverBlock, refs: &BlockRefs<'_>, with_backdrop: bool) -> Option<String> {
    let content = block_content(info, block, refs)?;
    let mut text =
        format!("size: {:.2}pt, fill: {}", block.size_pt.clamp(SIZE_PT.0, SIZE_PT.1), block_color(&block.color));
    match block.style {
        FontStyle::Normal => {}
        FontStyle::Bold => text.push_str(", weight: \"bold\""),
        FontStyle::Italic => text.push_str(", style: \"italic\""),
    }
    if let Some(font) = block.font.as_deref().map(str::trim).filter(|f| !f.is_empty()) {
        text.push_str(&format!(", font: {}", typst_string(font)));
    }
    let align = match block.align {
        CoverAlign::Left | CoverAlign::Justify => "left",
        CoverAlign::Center => "center",
        CoverAlign::Right => "right",
    };
    let justify = block.align == CoverAlign::Justify;
    let leading = if block.leading_em.is_finite() { block.leading_em.clamp(LEADING_EM.0, LEADING_EM.1) } else { 0.65 };
    let backdrop = match (with_backdrop, block.backdrop) {
        (false, _) | (_, Backdrop::None) => "",
        (true, Backdrop::Shadow) => ", fill: black.transparentize(50%), outset: (x: 3mm, y: 2mm), radius: 1mm",
        (true, Backdrop::Gradient) => {
            ", fill: gradient.linear(black.transparentize(100%), black.transparentize(35%), \
             black.transparentize(100%), angle: 90deg), outset: (x: 6mm, y: 6mm)"
        }
    };
    Some(format!(
        "block(width: {}{backdrop}, {{ set text({text}); set par(leading: {leading:.3}em, justify: {justify}, \
         first-line-indent: 0pt, hanging-indent: 0pt, spacing: 0.8em); set align({align}); {content} }})",
        mm(block.width_mm.clamp(WIDTH_MM.0, WIDTH_MM.1)),
    ))
}

/// The visible blocks of the face `face` as `place(…)` lines; `origin` is the trim edge of the face on the page
/// (mm). Spine: rotated (`down` 90°, `up` −90°, `reflow`), the top left corner of the rotated frame stands at (x,
/// y). `indent` stands before every line (in the cover for the print shop `#`).
fn placed_blocks(
    info: &BookInfo,
    layout: &CoverLayout,
    face: CoverFace,
    origin: (f64, f64),
    refs: &BlockRefs<'_>,
    indent: &str,
) -> String {
    let mut out = String::new();
    for kind in CoverBlockKind::ALL.into_iter().filter(|k| k.face() == face) {
        let Some(block) = layout.get(kind).filter(|b| b.visible) else { continue };
        let Some(body) = block_body(info, block, refs, true) else { continue };
        let body = if face == CoverFace::Spine {
            let angle = match block.direction.unwrap_or(SpineDirection::Down) {
                SpineDirection::Down => "90deg",
                SpineDirection::Up => "-90deg",
            };
            format!("rotate({angle}, reflow: true, {body})")
        } else {
            body
        };
        out.push_str(&format!(
            "{indent}place(top + left, dx: {}, dy: {}, {body}) // {}\n",
            signed_mm(origin.0 + block.x_mm),
            signed_mm(origin.1 + block.y_mm),
            kind.as_str()
        ));
    }
    out
}

/// Source for measuring the block heights: one page per block (`width` = block width, `height: auto`, margin 0)
/// with the block without background (the background only reaches out through `outset` and does not change the
/// height). A block without content gets an empty page. The height of page `i` is the height of block `i` in
/// reading direction (on the spine the horizontal extent on the page).
pub fn block_measure_source(info: &BookInfo, blocks: &[CoverBlock], refs: &BlockRefs<'_>) -> String {
    block_measure_source_with_rules(info, blocks, refs, "")
}

/// Like [`block_measure_source`], with the text rules of the work template (Round 20c, see
/// [`wrap_source_with_rules`]): the measured height belongs to the line breaking the cover will also have later.
pub fn block_measure_source_with_rules(
    info: &BookInfo,
    blocks: &[CoverBlock],
    refs: &BlockRefs<'_>,
    rules: &str,
) -> String {
    let mut out = String::from("// Baruch: Höhen der Umschlag-Blöcke (Runde 27)\n#set text(lang: \"de\")\n");
    out.push_str(rules);
    for block in blocks {
        let width = mm(block.width_mm.clamp(WIDTH_MM.0, WIDTH_MM.1));
        let body = block_body(info, block, refs, false).unwrap_or_else(|| "block(height: 0pt)".to_owned());
        out.push_str(&format!("#page(width: {width}, height: auto, margin: 0pt, {body})\n"));
    }
    out
}

/// Does the block have something to show (text or photo/barcode)?
pub fn block_has_content(info: &BookInfo, block: &CoverBlock, refs: &BlockRefs<'_>) -> bool {
    block_content(info, block, refs).is_some()
}

// ---- Dictionary for the front-matter functions (Round 27) ------------------------------------------

/// `#let baruch_buch = (…)`: all details as Typst strings (never markup or code), empty = `""`, plus
/// `impressum_zeilen` as an array of the lines of [`imprint_lines`].
///
/// Round 39b: the generated words in the work language `lang` (German: byte-identical to the function without
/// `_in`).
pub fn book_dict_in(info: &BookInfo, lang: WorkLanguage) -> String {
    localized(book_dict(info), lang)
}

/// Dictionary of a German work (exactly as before Round 39b); [`book_dict_in`] for the work language.
pub fn book_dict(info: &BookInfo) -> String {
    let value = |text: &str| typst_string(text.trim());
    let entries: [(&str, String); 23] = [
        ("titel", value(&info.title)),
        ("untertitel", value(&info.subtitle)),
        ("autor", value(&info.author)),
        ("verlag", value(&info.publisher)),
        ("verlag_anschrift", value(&info.publisher_address)),
        ("ort", value(&info.place)),
        ("jahr", value(&info.year)),
        ("auflage", value(&info.edition)),
        ("isbn", value(&info.isbn)),
        ("preis", value(&info.price)),
        ("reihe", value(&info.series)),
        ("druck", value(&info.printer)),
        ("druck_anschrift", value(&info.printer_address)),
        ("bibel", value(&info.bible)),
        ("bibel_hinweis", value(&info.bible_notice)),
        ("bildnachweis", value(&info.image_credit)),
        ("widmung", value(&info.dedication)),
        ("klappentext", value(&info.blurb)),
        ("vita", value(&info.bio)),
        ("copyright", typst_string(&copyright_line(info))),
        ("dnb_titel", typst_string(DNB_TITLE)),
        ("dnb_text", typst_string(DNB_TEXT)),
        ("gesetzt", typst_string(TYPESET_WITH)),
    ];
    let mut out = format!("#let {BOOK_DICT} = (\n");
    for (key, value) in entries {
        out.push_str(&format!("  {key}: {value},\n"));
    }
    out.push_str("  impressum_zeilen: (");
    for line in imprint_lines(info) {
        out.push_str(&typst_string(&line));
        out.push_str(", ");
    }
    out.push_str("),\n)\n");
    out
}

// ---- Front matter from the work template (Round 27) ------------------------------------------------

/// Names of the front-matter functions in the work template, in the order of the pages.
pub const TEMPLATE_FUNCTION_NAMES: [&str; 4] = ["schmutztitel", "haupttitel", "impressum", "widmung"];

/// Names of the functions that `#let name(` or `#let name =` defines at the start of a line (detection like
/// `ribbon::defined_functions` in the server, own copy).
fn defined_functions(template: &str) -> Vec<String> {
    let mut names = Vec::new();
    for line in template.lines() {
        let Some(rest) = line.trim_start().strip_prefix("#let") else { continue };
        let rest = rest.trim_start();
        if rest.len() == line.trim_start().len() - 4 {
            continue; // „#letter…“: no whitespace after #let
        }
        let name: String = rest.chars().take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '-').collect();
        let after = rest[name.len()..].trim_start();
        if !name.is_empty() && (after.starts_with('(') || after.starts_with('=')) && !names.contains(&name) {
            names.push(name);
        }
    }
    names
}

/// Which of the functions `schmutztitel`, `haupttitel`, `impressum`, `widmung` the work template defines (detection
/// like `ribbon::defined_functions`, own copy). Only a function with a parameter counts (`#let impressum(buch) =
/// …`), not a variable of the same name.
pub fn template_functions(template: &str) -> TemplateFunctions {
    let defined = defined_functions(template);
    let has = |name: &str| {
        defined.iter().any(|n| n == name)
            && template.lines().any(|line| {
                line.trim_start()
                    .strip_prefix("#let")
                    .map(str::trim_start)
                    .and_then(|rest| rest.strip_prefix(name))
                    .is_some_and(|after| after.trim_start().starts_with('('))
            })
    };
    TemplateFunctions {
        half_title: has("schmutztitel"),
        main_title: has("haupttitel"),
        imprint: has("impressum"),
        dedication: has("widmung"),
    }
}

/// Head of the imprint template.
const IMPRINT_HEAD: &str = "// Baruch: Titelei aus der Werkvorlage (Runde 27). Jede Funktion liefert den Inhalt einer Seite; Baruch\n\
// setzt sie an die feste Stelle (Schmutztitel, Haupttitel, Impressum auf dessen Rückseite, Widmung). `buch` ist\n\
// ein Wörterbuch aus Zeichenketten: titel, untertitel, autor, verlag, verlag_anschrift, ort, jahr, auflage, isbn,\n\
// preis, reihe, druck, druck_anschrift, bibel, bibel_hinweis, bildnachweis, widmung, klappentext, vita, copyright,\n\
// dnb_titel, dnb_text, gesetzt und impressum_zeilen (Array). Frei änderbar; fehlt eine Funktion, setzt Baruch\n\
// die eingebaute Seite.\n";

/// The four functions of the imprint template (name, Typst). They set the same as the built-in pages.
const IMPRINT_FUNCTIONS: [(&str, &str); 4] = [
    ("schmutztitel", "#let schmutztitel(buch) = {\n  set align(center)\n  v(22%)\n  text(size: 16pt, buch.titel)\n}\n"),
    (
        "haupttitel",
        "#let haupttitel(buch) = {\n  set align(center)\n  v(12%)\n  if buch.autor != \"\" {\n    text(size: 13pt, buch.autor)\n    v(2em)\n  }\n  text(size: 26pt, weight: \"bold\", buch.titel)\n  if buch.untertitel != \"\" {\n    v(1em)\n    text(size: 14pt, buch.untertitel)\n  }\n  v(1fr)\n  if buch.verlag != \"\" { text(size: 11pt, buch.verlag) }\n  if buch.ort != \"\" {\n    linebreak()\n    text(size: 10pt, buch.ort)\n  }\n}\n",
    ),
    (
        "impressum",
        "#let impressum(buch) = {\n  set text(size: 8.5pt)\n  set par(justify: false)\n  v(1fr)\n  for (i, zeile) in buch.impressum_zeilen.enumerate() {\n    if i == 0 { block(spacing: 0.8em, text(weight: \"bold\", zeile)) } else { block(spacing: 0.8em, text(zeile)) }\n  }\n}\n",
    ),
    (
        "widmung",
        "#let widmung(buch) = {\n  set align(center)\n  v(25%)\n  set text(style: \"italic\")\n  for absatz in buch.widmung.split(\"\\n\\n\") {\n    if absatz.trim() != \"\" { par(absatz.trim()) }\n  }\n}\n",
    ),
];

/// Imprint template (the four front-matter functions) that new books get appended to their work template.
pub fn imprint_template() -> &'static str {
    static TEMPLATE: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    TEMPLATE.get_or_init(|| {
        let mut out = IMPRINT_HEAD.to_owned();
        for (_, code) in IMPRINT_FUNCTIONS {
            out.push_str(code);
        }
        out
    })
}

/// Message if the work template already has all four front-matter functions.
pub const IMPRINT_COMPLETE: &str =
    "Die Werkvorlage definiert schmutztitel, haupttitel, impressum und widmung schon – es fehlt nichts.";

/// The parts of the imprint template that `template` still lacks, appended to `template`; if all four functions are
/// already there, an honest refusal ([`IMPRINT_COMPLETE`]).
pub fn imprint_template_missing(template: &str) -> Result<String, &'static str> {
    let present = template_functions(template);
    let missing: Vec<&str> = IMPRINT_FUNCTIONS
        .iter()
        .filter(|(name, _)| match *name {
            "schmutztitel" => !present.half_title,
            "haupttitel" => !present.main_title,
            "impressum" => !present.imprint,
            _ => !present.dedication,
        })
        .map(|(_, code)| *code)
        .collect();
    if missing.is_empty() {
        return Err(IMPRINT_COMPLETE);
    }
    let mut out = template.to_owned();
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    out.push('\n');
    out.push_str(IMPRINT_HEAD);
    for code in missing {
        out.push_str(code);
    }
    Ok(out)
}

// ---- ISBN and barcode ------------------------------------------------------------------------------

/// Why an ISBN-13 is not valid ([`isbn13_check`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IsbnError {
    /// A character other than digits, hyphens and spaces.
    Character(char),
    /// Not 13 digits (found: this many).
    Length(usize),
    /// Check digit wrong (expected, given).
    CheckDigit(u8, u8),
    /// Prefix (the first three digits) neither 978 nor 979 (Round 20c).
    Prefix(u16),
}

impl IsbnError {
    /// Short fixed message (for error kinds with `&'static str`).
    pub fn message(self) -> &'static str {
        match self {
            IsbnError::Character(_) => "ISBN: nur Ziffern, Bindestriche und Leerzeichen sind erlaubt.",
            IsbnError::Length(_) => "ISBN: 13 Ziffern erwartet (ISBN-13).",
            IsbnError::CheckDigit(..) => "ISBN: Die Prüfziffer stimmt nicht.",
            IsbnError::Prefix(_) => "ISBN: Eine ISBN-13 beginnt mit 978 oder 979.",
        }
    }
}

impl std::fmt::Display for IsbnError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            IsbnError::Character(c) => {
                write!(f, "ISBN: „{c}“ ist nicht erlaubt (nur Ziffern, Bindestriche und Leerzeichen).")
            }
            IsbnError::Length(n) => write!(f, "ISBN: 13 Ziffern erwartet (ISBN-13), gefunden {n}."),
            IsbnError::CheckDigit(expected, given) => {
                write!(f, "ISBN: Die Prüfziffer stimmt nicht (erwartet {expected}, angegeben {given}).")
            }
            IsbnError::Prefix(prefix) => write!(
                f,
                "ISBN: Eine ISBN-13 beginnt mit 978 oder 979, nicht mit {prefix:03} (eine EAN, aber keine ISBN)."
            ),
        }
    }
}

/// Cut off the prefix „ISBN-13“, „ISBN 13“, „ISBN13“ or „ISBN“ in any spelling (also „isbn-13“), plus a colon after it.
fn strip_isbn_label(text: &str) -> &str {
    let has_label = text.get(..4).is_some_and(|head| head.eq_ignore_ascii_case("isbn"));
    if !has_label {
        return text;
    }
    let mut rest = &text[4..];
    for thirteen in ["-13", " 13", "13", "\u{2010}13", "\u{2011}13"] {
        if let Some(after) = rest.strip_prefix(thirteen) {
            // „ISBN 13…“ only as a prefix if no digit follows after it (otherwise the 13 does not belong to the
            // prefix, but an ISBN starts with 978/979 anyway).
            if !after.starts_with(|c: char| c.is_ascii_digit()) {
                rest = after;
            }
            break;
        }
    }
    rest.trim_start().trim_start_matches(':').trim()
}

/// Like [`isbn13`], with the kind of error. Round 20c: prefix in any spelling („isbn-13 …“), prefix 978/979.
pub fn isbn13_check(input: &str) -> Result<String, IsbnError> {
    let rest = strip_isbn_label(input.trim());
    let mut digits = String::new();
    for c in rest.chars() {
        match c {
            '0'..='9' => digits.push(c),
            '-' | ' ' | '\u{2010}' | '\u{2011}' => {}
            other => return Err(IsbnError::Character(other)),
        }
    }
    if digits.len() != 13 {
        return Err(IsbnError::Length(digits.len()));
    }
    let prefix: u16 = digits[..3].parse().unwrap_or(0);
    if prefix != 978 && prefix != 979 {
        return Err(IsbnError::Prefix(prefix));
    }
    let check = ean_check_digit(&digits[..12]);
    let last = digits.as_bytes()[12] - b'0';
    if check != last {
        return Err(IsbnError::CheckDigit(check, last));
    }
    Ok(digits)
}

/// Checks an ISBN-13 (with or without hyphens/spaces, optionally with „ISBN“ in front) and returns the 13 digits;
/// wrong characters, length or check digit → German message.
pub fn isbn13(input: &str) -> Result<String, String> {
    isbn13_check(input).map_err(|e| e.to_string())
}

/// Check digit of an EAN-13 from the first 12 digits (weights 1, 3, 1, 3 …).
fn ean_check_digit(first12: &str) -> u8 {
    let sum: u32 = first12.bytes().enumerate().map(|(i, b)| u32::from(b - b'0') * if i % 2 == 0 { 1 } else { 3 }).sum();
    ((10 - sum % 10) % 10) as u8
}

const EAN_L: [&str; 10] =
    ["0001101", "0011001", "0010011", "0111101", "0100011", "0110001", "0101111", "0111011", "0110111", "0001011"];
const EAN_PARITY: [&str; 10] =
    ["LLLLLL", "LLGLGG", "LLGGLG", "LLGGGL", "LGLLGG", "LGGLLG", "LGGGLL", "LGLGLG", "LGLGGL", "LGGLGL"];

/// Modules of an EAN-13 as `0`/`1` (95 characters, without quiet zones); not 13 digits → `None`.
pub fn ean13_modules(digits: &str) -> Option<String> {
    if digits.len() != 13 || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let d: Vec<usize> = digits.bytes().map(|b| usize::from(b - b'0')).collect();
    let complement = |code: &str| code.chars().map(|c| if c == '0' { '1' } else { '0' }).collect::<String>();
    let mut out = String::from("101");
    for (i, &digit) in d[1..7].iter().enumerate() {
        let l = EAN_L[digit];
        if EAN_PARITY[d[0]].as_bytes()[i] == b'L' {
            out.push_str(l);
        } else {
            // G code: the mirror image of the R code (= complement of the L code).
            out.extend(complement(l).chars().rev());
        }
    }
    out.push_str("01010");
    for &digit in &d[7..] {
        out.push_str(&complement(EAN_L[digit]));
    }
    out.push_str("101");
    Some(out)
}

/// Barcode as SVG: one module = one unit, quiet zones [`EAN_QUIET_LEFT`]/[`EAN_QUIET_RIGHT`], guard bars longer;
/// one `<rect>` per bar. No valid 13 digits → empty text.
pub fn ean13_svg(digits: &str) -> String {
    let Some(modules) = ean13_modules(digits) else { return String::new() };
    let total = EAN_QUIET_LEFT + modules.len() + EAN_QUIET_RIGHT;
    let height = 60;
    let mut out = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {total} {height}\" width=\"{total}\" height=\"{height}\" shape-rendering=\"crispEdges\">\
         <rect width=\"{total}\" height=\"{height}\" fill=\"#ffffff\"/>"
    );
    let bytes = modules.as_bytes();
    let guard = |at: usize| at < 3 || (45..50).contains(&at) || at >= 92;
    let mut at = 0;
    while at < bytes.len() {
        if bytes[at] == b'1' {
            let start = at;
            while at < bytes.len() && bytes[at] == b'1' {
                at += 1;
            }
            let h = if guard(start) { height } else { height - 6 };
            out.push_str(&format!(
                "<rect x=\"{}\" y=\"0\" width=\"{}\" height=\"{h}\" fill=\"#000000\"/>",
                EAN_QUIET_LEFT + start,
                at - start
            ));
        } else {
            at += 1;
        }
    }
    out.push_str("</svg>");
    out
}

/// Spine width in mm: pages × paper thickness per page.
pub fn spine_mm(pages: usize, paper_mm: f64) -> f64 {
    pages as f64 * paper_mm
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literals_escape_quotes_backslashes_and_newlines() {
        assert_eq!(typst_string("a\"b\\c\nd"), "\"a\\\"b\\\\c\\nd\"", "Anführung, Rückstrich, Zeilenumbruch maskiert");
        assert_eq!(typst_string("#*]$"), "\"#*]$\"", "Markup-Zeichen bleiben in einem String harmlos");
    }

    #[test]
    fn lengths_never_use_exponents() {
        assert_eq!(mm(0.000_01), "0.000mm", "winzig → 0.000mm, nicht 1e-5");
        assert_eq!(mm(f64::NAN), "0.000mm", "NaN → 0");
    }
}
