// SPDX-License-Identifier: AGPL-3.0-or-later
//! Umschlag, Titelei und Buchrücken als Typst-Quelltext (Vertrag Runde 11, Abschnitt 3, Aufgabe cover).
//!
//! Rein, ohne Datenbank: Der Server liefert die Buchangaben ([`BookInfo`]) und die virtuellen Pfade der
//! Bilder ([`VIRTUAL_DIR`]), diese Crate schreibt daraus Typst.
//!
//! - [`frame_parts`]: was vor dem ersten Abschnitt (`front`) und nach dem letzten (`back`) in den
//!   Buchsatz kommt. `front`: Umschlag-Vorderseite (vollflächiges Bild, `margin: 0pt`, Titel und
//!   Untertitel im gewählten Bereich, heller Text auf einem dunklen Verlauf), danach die Titelei
//!   (Schmutztitel, Seite 2 leer bzw. Reihentitel, Haupttitel, Impressum, Widmung auf Wunsch). `back`:
//!   Umschlag-Rückseite (Klappentext, Kurzbiografie mit Foto, ISBN mit Strichcode bzw. Platzhalter,
//!   Verlag und Preis). Beide enthalten keine Überschrift und kein Inhaltsverzeichnis, damit
//!   Seitenzuordnung und Gliederung des Buchs unberührt bleiben; keine dieser Seiten zeigt eine Seitenzahl.
//! - [`wrap_source`]: der ganze Umschlag als **eine** Seite für die Druckerei: Rückseite | Rücken
//!   (Text gedreht) | Vorderseite, rundum mit Beschnitt.
//! - Nutzertexte stehen **nur** als Typst-String-Literale im Quelltext ([`typst_string`]: `\\`, `\"`,
//!   Zeilenumbruch maskiert), nie als Markup: `#`, `*`, `]`, `$` in einem Titel werden wörtlich gesetzt.
//! - [`isbn13`] prüft eine ISBN-13 samt Prüfziffer, [`ean13_svg`] zeichnet ihren Strichcode (EAN-13,
//!   95 Module und Ruhezonen), [`spine_mm`] rechnet die Rückenbreite.
//! - **Runde 27, Anordnung:** Mit `layout` setzen [`frame_parts`] und [`wrap_source`] den Text des Umschlags aus
//!   Blöcken ([`crate::cover_layout`]): `place(top + left, dx, dy, block(width, …))` relativ zur Schnittkante der
//!   Fläche, mit Größe, Zeilenabstand, Farbe, Schrift, Schnitt, Ausrichtung, Schatten bzw. Verlauf; auf dem Rücken
//!   gedreht (`down` 90°, `up` −90°). [`block_measure_source`] setzt jeden Block auf eine eigene Seite mit
//!   `height: auto` – so misst der Server die Höhe mit Typst.
//! - **Runde 27, Titelei aus der Werkvorlage:** [`template_functions`] erkennt `schmutztitel`, `haupttitel`,
//!   `impressum`, `widmung`; [`frame_parts`] schreibt dann einmal [`book_dict`] (`#let baruch_buch = (…)`, nur
//!   String-Literale) und ruft die Funktionen auf eigenen Seiten an fester Stelle auf. [`imprint_template`] ist die
//!   Vorlage für neue Bücher, [`imprint_template_missing`] hängt nur Fehlendes an. Die Grundschrift der
//!   Vorlage liest `baruch_wysiwyg::template_font` (oberste Ebene, letzte Regel gewinnt).
//! - **Runde 20c, Textregeln der Werkvorlage:** [`template_text_rules`] liest die `#set text(…)`- und
//!   `#set par(…)`-Regeln der obersten Ebene (nur Sprache, Silbentrennung, Laufweite, Abstände, Kosten, Blocksatz,
//!   Zeilenabstand, Einzüge – keine Schrift, Größe oder Farbe, die bestimmen die Blöcke bzw. der dunkle Grund) mit
//!   einfachen Werten (Zahlen mit Einheit, Zeichenketten, `true`/`false`/`auto`/`none`, Wörterbücher daraus; Werte
//!   mit Variablen der Vorlage fallen weg, weil der Umschlag die Vorlage nicht einbindet). [`wrap_source_with_rules`]
//!   und [`block_measure_source_with_rules`] setzen sie nach `#set text(lang: "de")`; ohne Regeln sind sie
//!   byte-gleich zu [`wrap_source`] bzw. [`block_measure_source`] (Goldstände R26/R27).

/// Ordner der virtuellen Dateien, die der Server dem Satz beilegt (Umschlag, Foto, Strichcode):
/// `.baruch/<Bildname>`, Strichcode `.baruch/isbn.svg`.
pub const VIRTUAL_DIR: &str = ".baruch";

/// Virtueller Pfad des Strichcodes.
pub const BARCODE_PATH: &str = ".baruch/isbn.svg";

/// Erste Zeile von [`FrameParts::front`]; daran erkennt der Server, dass der Rahmen schon im Satz steht.
pub const FRAME_MARK: &str = "// Baruch: Umschlag und Titelei (Runde 11)";

/// Ruhezone links und rechts des Strichcodes in Modulen (EAN-13: 11 links, 7 rechts).
pub const EAN_QUIET_LEFT: usize = 11;
pub const EAN_QUIET_RIGHT: usize = 7;

/// Wo Titel und Untertitel auf der Vorderseite stehen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TitleArea {
    #[default]
    Top,
    Middle,
    Bottom,
}

/// Angaben des Buchs für Umschlag und Titelei; leere Felder fallen weg.
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
    /// Bibelübersetzung (z. B. „Lutherbibel 2017“).
    pub bible: String,
    /// Rechtehinweis der Bibelübersetzung.
    pub bible_notice: String,
    /// Bildnachweis bzw. Umschlaggestaltung (der Server setzt ohne Angabe die Bildunterschrift des Umschlags ein).
    pub image_credit: String,
    pub dedication: String,
    /// Klappentext der Rückseite.
    pub blurb: String,
    /// Kurzbiografie.
    pub bio: String,
    /// Anschrift des Verlags bzw. bei Selbstverlag des Verfassers oder Herausgebers (Runde 27, mehrzeilig).
    pub publisher_address: String,
    /// Anschrift der Druckerei (Runde 27, mehrzeilig).
    pub printer_address: String,
}

/// Welche Titelei-Funktionen die Werkvorlage definiert (Runde 27, Vertrag 1.6; Typst: `schmutztitel`,
/// `haupttitel`, `impressum`, `widmung`, je `#let name(buch)`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TemplateFunctions {
    pub half_title: bool,
    pub main_title: bool,
    pub imprint: bool,
    pub dedication: bool,
}

/// Umschlagbild: virtueller Pfad (`.baruch/<name>`) und Alternativtext.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoverRef {
    pub path: String,
    pub alt: String,
}

/// Was [`frame_parts`] einfügt.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FrameOptions {
    pub cover: Option<CoverRef>,
    /// Vorder- und Rückseite in den Satz (nur wirksam mit Umschlag).
    pub cover_pages: bool,
    /// Titelei (Schmutztitel, Seite 2, Haupttitel, Impressum, Widmung).
    pub front_matter: bool,
    pub title_area: TitleArea,
    /// Virtueller Pfad der Strichcode-SVG (nur mit gültiger ISBN).
    pub barcode: Option<String>,
    /// Virtueller Pfad des Autorenfotos.
    pub author_photo: Option<String>,
    /// Eigene Anordnung des Umschlags (Runde 27); `None` bzw. leer = bisheriges Bild.
    pub layout: Option<crate::cover_layout::CoverLayout>,
    /// Titelei-Funktionen der Werkvorlage (Runde 27); keine = feste Seiten wie bisher.
    pub template_functions: TemplateFunctions,
}

/// Typst vor dem ersten und nach dem letzten Abschnitt.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FrameParts {
    pub front: String,
    pub back: String,
    /// Seiten des Umschlags im Satz ([`COVER_PAGES`] oder 0); die Titelei zählt nicht dazu.
    pub added_cover_pages: usize,
}

/// Seiten, die der Umschlag in den Buchsatz bringt: Vorderseite, leere Innenseite, Rückseite. Die Innenseite
/// hält die Seitenparität (das erste Kapitel beginnt rechts wie ohne Umschlag), und auf ihr beginnt der
/// Seitenzähler neu, damit die Seitenzahlen des Buchblocks nicht vom Umschlag abhängen.
pub const COVER_PAGES: usize = 3;

/// Maße des ganzen Umschlags für [`wrap_source`].
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
    /// Eigene Anordnung des Umschlags (Runde 27); `None` bzw. leer = bisheriges Bild.
    pub layout: Option<crate::cover_layout::CoverLayout>,
}

/// Name des Wörterbuchs, das Baruch nach [`FRAME_MARK`] schreibt, wenn die Werkvorlage Titelei-Funktionen
/// definiert (Runde 27, Vertrag 1.6): `#let baruch_buch = (…)`.
pub const BOOK_DICT: &str = "baruch_buch";

use crate::cover_layout::{
    Backdrop, CoverAlign, CoverBlock, CoverBlockKind, CoverFace, CoverGeometry, CoverLayout, FontStyle, LEADING_EM,
    SIZE_PT, SpineDirection, WIDTH_MM, effective_blocks,
};

/// Farbe von Rückseite und Rücken.
const DARK: &str = "rgb(\"#1f2a33\")";
/// Ränder einer Umschlagseite (ohne Beschnitt) in mm.
const PANEL_MARGIN_MM: f64 = 14.0;

// ---- Typst-Literale ------------------------------------------------------------------------------

/// Typst-String-Literal: `"…"` mit `\\`, `\"`, `\n`, `\t` maskiert; `\r` und andere Steuerzeichen
/// fallen weg. So wird ein Nutzertext nie als Markup oder Code gelesen.
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

/// Länge in mm als Typst-Wert (`12.500mm`), nie in Exponentenschreibweise.
fn mm(value: f64) -> String {
    let value = if value.is_finite() { value.max(0.0) } else { 0.0 };
    format!("{value:.3}mm")
}

/// Mehrzeiliger Text als Absätze (Leerzeile trennt), je Absatz ein String-Literal; einfache
/// Zeilenumbrüche bleiben Zeilenumbrüche.
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

/// Seite ohne Kopf, Fuß und Seitenzahl, Inhalt als Codeblock.
fn plain_page(args: &str, body: &str) -> String {
    format!("#page(header: none, footer: none, numbering: none{args}, {{\n{body}}})\n")
}

// ---- Umschlag-Seiten -------------------------------------------------------------------------------

/// Titel und Untertitel der Vorderseite im gewählten Bereich, heller Text auf dunklem Verlauf.
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

/// Inhalt der Vorderseite (Bild oder dunkle Fläche, darauf der Titel) in einem Block der Größe 100 %.
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

/// Strichcode mit ISBN darunter bzw. ein Platzhalter (ohne gültige ISBN).
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

/// Inhalt der Rückseite: Klappentext, Kurzbiografie (mit Foto), unten Verlag/Preis und Strichcode.
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

// ---- Titelei -----------------------------------------------------------------------------------

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
        // Leere Seite: ein unsichtbarer Inhalt, damit Typst die Seite sicher ausgibt.
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

/// Überschrift des DNB-Hinweises im Impressum.
pub const DNB_TITLE: &str = "Bibliografische Information der Deutschen Nationalbibliothek";
/// Text des DNB-Hinweises im Impressum.
pub const DNB_TEXT: &str = "Die Deutsche Nationalbibliothek verzeichnet diese Publikation in der Deutschen \
     Nationalbibliografie; detaillierte bibliografische Daten sind im Internet über https://dnb.de abrufbar.";
/// Satzvermerk im Impressum.
pub const TYPESET_WITH: &str = "Gesetzt mit Baruch";

/// Die ©-Zeile des Impressums: Jahr, Verfasser (sonst Verlag), Verlag, Ort.
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

/// Mehrzeilige Anschrift in einer Zeile („Weg 1, 12345 Ort“).
fn one_line(address: &str) -> Option<String> {
    let parts: Vec<&str> = address.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
    (!parts.is_empty()).then(|| parts.join(", "))
}

/// Zeilen des Impressums (Text, schon ohne Markup). Die Anschriften (Runde 27) kommen nur dazu, wenn sie gesetzt
/// sind: die des Verlags als eigene Zeile nach der ©-Zeile, die der Druckerei hinter „Druck:“ – ohne Anschriften
/// bleiben die Zeilen wie vor Runde 27.
pub fn imprint_lines(info: &BookInfo) -> Vec<String> {
    let mut lines = vec![DNB_TITLE.to_owned(), DNB_TEXT.to_owned()];
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
        (Some(bible), Some(notice)) => lines.push(format!("Bibelzitate: {bible}. {notice}")),
        (Some(bible), None) => lines.push(format!("Bibelzitate: {bible}.")),
        (None, Some(notice)) => lines.push(notice.to_owned()),
        (None, None) => {}
    }
    if let Some(credit) = filled(&info.image_credit) {
        lines.push(format!("Umschlaggestaltung und Bildnachweis: {credit}"));
    }
    lines.push(TYPESET_WITH.to_owned());
    match (filled(&info.printer), one_line(&info.printer_address)) {
        (Some(printer), Some(address)) => lines.push(format!("Druck: {printer}, {address}")),
        (Some(printer), None) => lines.push(format!("Druck: {printer}")),
        (None, Some(address)) => lines.push(format!("Druck: {address}")),
        (None, None) => {}
    }
    lines
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
    // Danach eine leere Seite, damit das erste Kapitel wieder auf einer rechten Seite beginnt.
    Some(format!("{}{}", plain_page("", &body), plain_page("", "  hide[ ]\n")))
}

/// Typst vor dem ersten und nach dem letzten Abschnitt. Beides leer (und `added_cover_pages` 0),
/// wenn weder ein Umschlag mit `cover_pages` noch die Titelei gewünscht ist.
///
/// Runde 27: Mit einer nicht leeren `opts.layout` setzen Vorder- und Rückseite **nur** die Blöcke dieser
/// Anordnung (sichtbar und mit Inhalt), jeweils relativ zur Schnittkante der Seite (im Buch W × H mit Rand 0).
/// Fehlende Arten ergänzt der Server vorher mit [`crate::cover_layout::effective_blocks`], denn nur er kennt die
/// Seitengröße. Definiert die Werkvorlage Titelei-Funktionen (`opts.template_functions`, nur mit Titelei
/// wirksam), schreibt Baruch nach [`FRAME_MARK`] einmal das Wörterbuch [`BOOK_DICT`] und setzt jede Funktion auf
/// eine eigene Seite an der festen Stelle; fehlt eine Funktion, steht dort die bisherige Seite. Ohne Anordnung und
/// ohne Funktionen ist die Ausgabe byte-gleich zu Runde 26.
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
        // Innenseite: leer; hier Seite 0, damit die erste Seite danach Seite 1 ist (Typst zählt jede Seite,
        // auch ohne Nummer). Physisch Seite 2 – die erste Seite danach liegt rechts wie ohne Umschlag.
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
            // Wie der Rückfall nur mit einer Widmung; danach die leere Seite für die Parität.
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

/// Eine Seite der Titelei aus einer Funktion der Werkvorlage: `name(baruch_buch)` als Inhalt.
fn function_page(name: &str) -> String {
    plain_page("", &format!("  {name}({BOOK_DICT})\n"))
}

/// Der ganze Umschlag auf einer Seite: Rückseite | Rücken | Vorderseite, mit Beschnitt rundum.
/// Breite = 2 × Seitenbreite + Rücken + 2 × Beschnitt, Höhe = Seitenhöhe + 2 × Beschnitt.
///
/// Runde 27: Mit einer nicht leeren `opts.layout` bleiben die Flächen (Bild bzw. dunkle Fläche) wie bisher, Text
/// kommt aber nur aus den Blöcken; fehlende Arten ergänzt [`crate::cover_layout::effective_blocks`]. Jeder Block
/// steht relativ zur Schnittkante seiner Fläche ([`CoverGeometry::face_origin`]) und darf in den Beschnitt ragen.
pub fn wrap_source(info: &BookInfo, opts: &WrapOptions) -> String {
    wrap_source_with_rules(info, opts, "")
}

/// Wie [`wrap_source`], mit den Textregeln der Werkvorlage (`rules` aus [`template_text_rules`], leer = keine)
/// direkt nach `#set text(lang: "de")` (Runde 20c): So bricht ein Klappentext auf dem Umschlag für die Druckerei wie
/// auf der Umschlagseite im Buch um.
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
    // Rückseite
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
    // Rücken: ab 6 mm mit Titel und Autor, gedreht (von oben nach unten zu lesen).
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
    // Vorderseite
    let front = front_body(info, opts.cover.as_ref(), opts.title_area, "  ");
    out.push_str(&format!(
        "#place(top + left, dx: {}, block(width: {}, height: {}, clip: true, {{\n{front}}}))\n",
        mm(panel + spine),
        mm(panel),
        mm(height)
    ));
    out
}

/// Nur die **Vorderseite** des Umschlags auf einer Seite im Endformat (`page_width_mm` × `page_height_mm`, ohne
/// Beschnitt) – die Vorlage des EPUB-Covers (Runde 21c, der Server rastert sie mit `baruch_render::render_png`).
/// Inhalt wie die Vorderseite in [`wrap_source_with_rules`]: das Umschlagbild füllend (`fit: "cover"`) bzw. ohne
/// Bild die dunkle Fläche, darauf mit einer nicht leeren `opts.layout` die sichtbaren Blöcke der Vorderseite
/// (Titel, Untertitel, Autor, Verlag, Freitext; fehlende Arten als Vorgabeblöcke), sonst Titel, Untertitel und Autor
/// im Titelbereich wie bisher. `spine_mm`, `barcode` und `author_photo` spielen keine Rolle; `bleed_mm` nur für die
/// Vorgabeblöcke (wie im Buchsatz). Nutzertexte nur als String-Literale.
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

// ---- Textregeln der Werkvorlage (Runde 20c) ---------------------------------------------------------

/// Argumente von `#set text(…)`, die der Umschlag übernimmt (keine Schrift, Größe, Farbe).
const TEXT_RULE_ARGS: [&str; 10] =
    ["lang", "region", "script", "dir", "hyphenate", "tracking", "spacing", "costs", "kerning", "ligatures"];
/// Argumente von `#set par(…)`, die der Umschlag übernimmt.
const PAR_RULE_ARGS: [&str; 6] = ["justify", "leading", "spacing", "first-line-indent", "hanging-indent", "linebreaks"];
/// Wörter, die in einem übernommenen Wert stehen dürfen (sonst wäre es eine Variable der Vorlage).
const RULE_WORDS: [&str; 4] = ["true", "false", "auto", "none"];

/// Kommentare (`// …`, `/* … */`) durch Leerzeichen ersetzen, Zeichenketten bleiben (Bytestellen gleich).
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
    // Mehrbyte-Zeichen in Kommentaren werden byteweise zu Leerzeichen; der Rest bleibt gültiges UTF-8.
    String::from_utf8_lossy(&out).into_owned()
}

/// Ende (exklusiv) der Klammer, die bei `open` beginnt; Zeichenketten zählen nicht.
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

/// Argumente auf Klammertiefe 0 trennen.
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

/// Ein Wert aus Zahlen (mit Einheit), Zeichenketten, `true`/`false`/`auto`/`none` und Wörterbüchern bzw. Feldern
/// daraus – ohne Variablen, Funktionsaufrufe oder Inhaltsblöcke.
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

/// Die Text- und Absatzregeln der obersten Ebene der Werkvorlage als Typst-Zeilen (`#set text(…)`,
/// `#set par(…)`), nur mit den übernommenen Argumenten und einfachen Werten (siehe Moduldoku); keine Regel → "".
/// Die Reihenfolge der Vorlage bleibt (die letzte Regel gewinnt wie im Buch). Bedingte Regeln (`… if …`) und
/// eingerückte Regeln (in Funktionen, Blöcken) zählen nicht.
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
            // Die Sprache, die ohnehin gilt (anfangs „de“), nicht noch einmal setzen: So bleiben Werke mit
            // `#set text(lang: "de")` byte-gleich zu den Goldständen.
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

// ---- Blöcke der Anordnung (Runde 27) ---------------------------------------------------------------

/// Virtuelle Pfade für die Blöcke mit Bild (Kurzbiografie mit Foto, Strichcode).
#[derive(Debug, Clone, Copy, Default)]
pub struct BlockRefs<'a> {
    pub photo: Option<&'a str>,
    pub barcode: Option<&'a str>,
}

/// Länge in mm mit Vorzeichen (Blöcke dürfen in den Beschnitt ragen), nie in Exponentenschreibweise.
fn signed_mm(value: f64) -> String {
    let value = if value.is_finite() { value } else { 0.0 };
    let text = format!("{value:.3}mm");
    if text == "-0.000mm" { "0.000mm".to_owned() } else { text }
}

/// `#rrggbb` mit sechs Hex-Ziffern?
pub fn is_hex_color(color: &str) -> bool {
    color.len() == 7 && color.starts_with('#') && color[1..].chars().all(|c| c.is_ascii_hexdigit())
}

/// Typst-Farbe eines Blocks: `light` weiß, `dark` wie Rückseite und Rücken, `#rrggbb` als `rgb("…")`; alles
/// andere weiß.
pub fn block_color(color: &str) -> String {
    match color.trim() {
        "dark" => DARK.to_owned(),
        hex if is_hex_color(hex) => format!("rgb({})", typst_string(&hex.to_ascii_lowercase())),
        _ => "white".to_owned(),
    }
}

/// Hintergrund der Vorderseite: Bild (füllend, `fit: "cover"`) oder dunkle Fläche.
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

/// Inhalt eines Blocks (Typst-Code ohne Rahmen) oder `None`, wenn er nichts zu zeigen hat (leerer Text). Der
/// Strichcode zeigt ohne gültige ISBN den Platzhalter. Nutzertexte nur als String-Literale.
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

/// Der Block ohne Lage: `block(width: …, {…})` mit Schrift, Absatz, Ausrichtung und auf Wunsch Hintergrund.
/// `None` wie [`block_content`].
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

/// Die sichtbaren Blöcke der Fläche `face` als `place(…)`-Zeilen; `origin` ist die Schnittkante der Fläche auf der
/// Seite (mm). Rücken: gedreht (`down` 90°, `up` −90°, `reflow`), die linke obere Ecke des gedrehten Rahmens steht
/// bei (x, y). `indent` steht vor jeder Zeile (im Umschlag für die Druckerei `#`).
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

/// Quelle zum Messen der Blockhöhen: je Block eine Seite (`width` = Blockbreite, `height: auto`, Rand 0) mit dem
/// Block ohne Hintergrund (der Hintergrund ragt nur über `outset` hinaus und ändert die Höhe nicht). Ein Block ohne
/// Inhalt bekommt eine leere Seite. Die Höhe der Seite `i` ist die Höhe des Blocks `i` in Leserichtung (beim
/// Rücken die waagrechte Ausdehnung auf der Seite).
pub fn block_measure_source(info: &BookInfo, blocks: &[CoverBlock], refs: &BlockRefs<'_>) -> String {
    block_measure_source_with_rules(info, blocks, refs, "")
}

/// Wie [`block_measure_source`], mit den Textregeln der Werkvorlage (Runde 20c, siehe [`wrap_source_with_rules`]):
/// Die gemessene Höhe gehört zum Umbruch, den der Umschlag später auch hat.
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

/// Hat der Block etwas zu zeigen (Text bzw. Foto/Strichcode)?
pub fn block_has_content(info: &BookInfo, block: &CoverBlock, refs: &BlockRefs<'_>) -> bool {
    block_content(info, block, refs).is_some()
}

// ---- Wörterbuch für die Titelei-Funktionen (Runde 27) ----------------------------------------------

/// `#let baruch_buch = (…)`: alle Angaben als Typst-Zeichenketten (nie Markup oder Code), leer = `""`, dazu
/// `impressum_zeilen` als Array der Zeilen von [`imprint_lines`].
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

// ---- Titelei aus der Werkvorlage (Runde 27) --------------------------------------------------------

/// Namen der Titelei-Funktionen in der Werkvorlage, in der Reihenfolge der Seiten.
pub const TEMPLATE_FUNCTION_NAMES: [&str; 4] = ["schmutztitel", "haupttitel", "impressum", "widmung"];

/// Namen der Funktionen, die `#let name(` bzw. `#let name =` am Zeilenanfang definiert (Erkennung wie
/// `ribbon::defined_functions` im Server, eigene Kopie).
fn defined_functions(template: &str) -> Vec<String> {
    let mut names = Vec::new();
    for line in template.lines() {
        let Some(rest) = line.trim_start().strip_prefix("#let") else { continue };
        let rest = rest.trim_start();
        if rest.len() == line.trim_start().len() - 4 {
            continue; // „#letter…“: kein Leerraum nach #let
        }
        let name: String = rest.chars().take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '-').collect();
        let after = rest[name.len()..].trim_start();
        if !name.is_empty() && (after.starts_with('(') || after.starts_with('=')) && !names.contains(&name) {
            names.push(name);
        }
    }
    names
}

/// Welche der Funktionen `schmutztitel`, `haupttitel`, `impressum`, `widmung` die Werkvorlage definiert
/// (Erkennung wie `ribbon::defined_functions`, eigene Kopie). Gezählt wird nur eine Funktion mit Parameter
/// (`#let impressum(buch) = …`), nicht eine gleichnamige Variable.
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

/// Kopf der Impressum-Vorlage.
const IMPRINT_HEAD: &str = "// Baruch: Titelei aus der Werkvorlage (Runde 27). Jede Funktion liefert den Inhalt einer Seite; Baruch\n\
// setzt sie an die feste Stelle (Schmutztitel, Haupttitel, Impressum auf dessen Rückseite, Widmung). `buch` ist\n\
// ein Wörterbuch aus Zeichenketten: titel, untertitel, autor, verlag, verlag_anschrift, ort, jahr, auflage, isbn,\n\
// preis, reihe, druck, druck_anschrift, bibel, bibel_hinweis, bildnachweis, widmung, klappentext, vita, copyright,\n\
// dnb_titel, dnb_text, gesetzt und impressum_zeilen (Array). Frei änderbar; fehlt eine Funktion, setzt Baruch\n\
// die eingebaute Seite.\n";

/// Die vier Funktionen der Impressum-Vorlage (Name, Typst). Sie setzen dasselbe wie die eingebauten Seiten.
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

/// Impressum-Vorlage (die vier Titelei-Funktionen), die neue Bücher an ihre Werkvorlage gehängt bekommen.
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

/// Meldung, wenn die Werkvorlage schon alle vier Titelei-Funktionen hat.
pub const IMPRINT_COMPLETE: &str =
    "Die Werkvorlage definiert schmutztitel, haupttitel, impressum und widmung schon – es fehlt nichts.";

/// Die Teile der Impressum-Vorlage, die `template` noch fehlen, an `template` gehängt; sind alle vier Funktionen
/// schon da, eine ehrliche Ablehnung ([`IMPRINT_COMPLETE`]).
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

// ---- ISBN und Strichcode ---------------------------------------------------------------------------

/// Warum eine ISBN-13 nicht gilt ([`isbn13_check`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IsbnError {
    /// Ein Zeichen außer Ziffern, Bindestrichen und Leerzeichen.
    Character(char),
    /// Nicht 13 Ziffern (gefunden: so viele).
    Length(usize),
    /// Prüfziffer falsch (erwartet, angegeben).
    CheckDigit(u8, u8),
    /// Präfix (die ersten drei Ziffern) weder 978 noch 979 (Runde 20c).
    Prefix(u16),
}

impl IsbnError {
    /// Kurze feste Meldung (für Fehlerarten mit `&'static str`).
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

/// Vorsatz „ISBN-13“, „ISBN 13“, „ISBN13“ bzw. „ISBN“ in beliebiger Schreibung (auch „isbn-13“) abschneiden, dazu
/// einen Doppelpunkt danach.
fn strip_isbn_label(text: &str) -> &str {
    let has_label = text.get(..4).is_some_and(|head| head.eq_ignore_ascii_case("isbn"));
    if !has_label {
        return text;
    }
    let mut rest = &text[4..];
    for thirteen in ["-13", " 13", "13", "\u{2010}13", "\u{2011}13"] {
        if let Some(after) = rest.strip_prefix(thirteen) {
            // „ISBN 13…“ nur als Vorsatz, wenn danach keine Ziffer folgt (sonst gehört die 13 nicht zum Vorsatz,
            // eine ISBN beginnt aber ohnehin mit 978/979).
            if !after.starts_with(|c: char| c.is_ascii_digit()) {
                rest = after;
            }
            break;
        }
    }
    rest.trim_start().trim_start_matches(':').trim()
}

/// Wie [`isbn13`], mit der Fehlerart. Runde 20c: Vorsatz in jeder Schreibung („isbn-13 …“), Präfix 978/979.
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

/// Prüft eine ISBN-13 (mit oder ohne Bindestriche/Leerzeichen, wahlweise mit „ISBN“ davor) und gibt
/// die 13 Ziffern zurück; falsche Zeichen, Länge oder Prüfziffer → deutsche Meldung.
pub fn isbn13(input: &str) -> Result<String, String> {
    isbn13_check(input).map_err(|e| e.to_string())
}

/// Prüfziffer einer EAN-13 aus den ersten 12 Ziffern (Gewichte 1, 3, 1, 3 …).
fn ean_check_digit(first12: &str) -> u8 {
    let sum: u32 = first12.bytes().enumerate().map(|(i, b)| u32::from(b - b'0') * if i % 2 == 0 { 1 } else { 3 }).sum();
    ((10 - sum % 10) % 10) as u8
}

const EAN_L: [&str; 10] =
    ["0001101", "0011001", "0010011", "0111101", "0100011", "0110001", "0101111", "0111011", "0110111", "0001011"];
const EAN_PARITY: [&str; 10] =
    ["LLLLLL", "LLGLGG", "LLGGLG", "LLGGGL", "LGLLGG", "LGGLLG", "LGGGLL", "LGLGLG", "LGLGGL", "LGGLGL"];

/// Module einer EAN-13 als `0`/`1` (95 Zeichen, ohne Ruhezonen); keine 13 Ziffern → `None`.
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
            // G-Code: das Spiegelbild des R-Codes (= Komplement des L-Codes).
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

/// Strichcode als SVG: ein Modul = eine Einheit, Ruhezonen [`EAN_QUIET_LEFT`]/[`EAN_QUIET_RIGHT`],
/// Randbalken länger; ein `<rect>` je Balken. Keine gültigen 13 Ziffern → leerer Text.
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

/// Rückenbreite in mm: Seiten × Papierdicke je Seite.
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
