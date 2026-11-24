// SPDX-License-Identifier: AGPL-3.0-or-later
//! Typst-Satz für die Vorschau (Vertrag Runde 8, Abschnitt 4a).
//!
//! Rein, ohne Datenbank und ohne Netz: [`compile`] bekommt das Werk als **eine** Typst-Datei
//! (`main`, der byte-gleiche Export samt Werkvorlage) und die Bilder als virtuelle Dateien und setzt
//! es mit dem eingebetteten Typst-Compiler zu SVG-Seiten.
//!
//! - **Dateien:** `image("…")` sucht relativ zu `main`, das im Wurzelordner liegt; `/x`, `./x` und
//!   `x` meinen darum dieselbe Datei. Gesucht wird in Normalform ([`normalize_path`]: ohne führendes
//!   `/` und `./`, ohne leere Teile – dieselbe Regel wie `baruch_store::normalize_asset_path`).
//!   Fehlt eine Datei, meldet Typst einen Fehler; die Meldung steht dann auf Deutsch in
//!   [`Rendered::diagnostics`].
//! - **Schriften:** immer die eingebetteten aus `typst-assets` (Libertinus Serif, New Computer
//!   Modern, DejaVu Sans Mono), dahinter die Systemschriften (einmal je Prozess gesucht). Verlangt die
//!   Vorlage eine Schrift, die es nicht gibt, setzt Typst mit der nächsten verfügbaren weiter; das
//!   ergibt eine Warnung „Schrift „X“ fehlt; ersetzt durch eingebettete Schrift“, die Seiten bleiben.
//! - **Pakete:** `#import "@preview/…"` wird nie geladen (kein Netz); Fehler „Typst-Pakete werden
//!   nicht geladen (offline)“.
//! - **Fassungsstand:** [`stand`] ist der SHA-256 über `main` und alle Dateien – der Schlüssel, unter
//!   dem der Server gesetzte Seiten zwischenspeichert.
//! - **PDF (Runde 11):** [`compile_pdf`] setzt dieselbe Quelle in derselben Welt (Schriften, Dateien)
//!   und schreibt sie mit `typst-pdf` (Apache-2.0) als PDF; die Seitenzahl ist dieselbe wie bei
//!   [`compile`]. Bei einem Fehler gibt es keine Bytes, nie eine halbe Datei.
//! - **Druckprüfung (Runde 11):** [`Rendered::print`] bzw. [`Pdf::print`] nennt jedes Rasterbild, das
//!   in seiner gesetzten Größe weniger als [`MIN_DPI`] hat, mit den Pixeln, die es für 300 dpi bräuchte
//!   ([`PrintWarning`]). SVG-Grafiken haben keine Auflösung und erscheinen nie. Die Warnungen stehen
//!   bewusst nicht in `diagnostics`, damit Fehler- und Warnungszahlen unverändert bleiben.
//! - **Abbruch (Runde 18a):** [`compile_cancellable`] setzt wie [`compile`], prüft aber ein [`Cancel`]. Typst
//!   selbst kennt keinen Abbruch; unterbrochen wird darum nur an den Stellen, an denen Baruch mitspielt: vor dem
//!   Lauf, zwischen Satz und SVG und vor jeder SVG-Seite. Auswertung (auch Schleifen der Vorlage) und Umbruch
//!   laufen weiter, bis Typst von selbst fertig ist; ein abgebrochener Lauf liefert immer `None` und nie halbe
//!   Seiten. Ein Abbruch über die Welt (Dateien oder Schriften nach dem Abbruch verweigern) geht **nicht**: Typst
//!   merkt sich Welt-Antworten (`comemo`) und bricht mit „found differing return values“ ab, wenn dieselbe Frage
//!   im selben Lauf anders beantwortet wird (gemessen in `tests/cancel.rs`, Zahlen in `docs/Architektur.md`,
//!   „Runde 18a“).
//! - **Schriftfamilien (Runde 27):** [`font_families`] nennt jede Familie mit ihren Schnitten und ob sie
//!   eingebettet ist; [`has_font_family`] prüft einen Namen ohne Unterschied von Groß- und Kleinschreibung. Der
//!   Server setzt eine fehlende Schrift eines Umschlag-Blocks mit der Vorgabe und warnt (Druckprüfung `font`).
//! - **Seitengrößen (Runde 27):** [`page_sizes_pt`] liest Breite und Höhe jeder gesetzten Seite aus dem SVG-Kopf –
//!   damit misst der Server Blockhöhen (eine Seite je Block mit `height: auto`) und die Seitengröße der Vorlage.
//! - **Zeilen je Seite (Runde 18a):** [`Rendered::page_lines`] nennt je Seite die Zeilen von `main`, aus denen
//!   Text auf ihr steht – damit ordnet der Server eine Seite, die zwei Kapitel teilen, beiden zu.
//! - **Messhilfe (Runde 29):** [`page_texts`] nennt je Seite die gesetzten Textstücke ([`PlacedText`]: Text,
//!   Grundlinie links in pt ab oben links, Breite, Größe, Schrift) – damit messen Tests jede Satz-Einstellung
//!   (Ränder mit Bundsteg, Seitenzahlen, Kolumnentitel, Zeilenabstand, Blocksatz, Silbentrennung).
//! - **Rasterbild (Runde 21c):** [`render_png`] rastert eine Seite mit `typst-render` als PNG in höchstens
//!   vorgegebener Pixelgröße (Seitenverhältnis bleibt) – für das EPUB-Cover aus der Umschlag-Vorderseite.
//! - **Typwechsel (Runde 21c):** Passt der Inhalt einer Bilddatei nicht zur Endung ihres Pfads (ein PNG ersetzt
//!   `bilder/haus.svg`, der Text nennt weiter den alten Pfad), bekommt `image("<Pfad>"…)` beim Satz das Format
//!   ausdrücklich (`with_image_formats`); Typst nähme es sonst aus der Endung und scheiterte.

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

/// Das zu setzende Werk: die Hauptdatei und die übrigen Dateien (Bilder) unter ihrem Pfad in
/// Normalform (z. B. `bilder/laube.svg`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Source {
    pub main: String,
    pub files: Vec<(String, Vec<u8>)>,
}

/// Schwere einer Meldung.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

impl Severity {
    /// `"error"` bzw. `"warning"` (RPC, MCP).
    pub fn as_str(self) -> &'static str {
        match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
        }
    }
}

/// Eine Meldung von Typst. `line`/`column` sind 1-basiert und beziehen sich auf `main`, wenn
/// `file` leer ist, sonst auf die genannte Datei; ohne Ort beide `None`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub severity: Severity,
    pub message: String,
    pub hints: Vec<String>,
    pub line: Option<usize>,
    pub column: Option<usize>,
    pub file: Option<String>,
}

/// Eine Überschrift in `main` (Zeile, 1-basiert) und die Seite, auf der Typst sie gesetzt hat
/// (1-basiert).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeadingPos {
    pub line: usize,
    pub page: usize,
}

/// Ergebnis von [`compile`]: Stand, Seiten als SVG, Überschriften und Meldungen. Bei einem Fehler
/// sind `pages`, `headings` und `print` leer.
#[derive(Debug, Clone, PartialEq)]
pub struct Rendered {
    pub stand: String,
    pub pages: Vec<String>,
    pub headings: Vec<HeadingPos>,
    pub diagnostics: Vec<Diagnostic>,
    /// Druckprüfung: Rasterbilder unter [`MIN_DPI`] (nicht in `diagnostics`).
    pub print: Vec<PrintWarning>,
    /// Je Seite (Index 0 = Seite 1) die Zeilen von `main` (1-basiert, aufsteigend, ohne Doppel), aus denen
    /// gesetzter Text auf ihr stammt – auch Kopf- und Fußzeilen und Text, den die Vorlage aus Überschriften
    /// zitiert (lebende Kolumnentitel, Inhaltsverzeichnis). Leer bei einem Fehler.
    pub page_lines: Vec<Vec<usize>>,
}

/// Abbruch eines Laufs (Runde 18a): geteilt zwischen dem, der abbricht, und dem Lauf in
/// [`compile_cancellable`]. `Default` ist „nicht abgebrochen“; Klone teilen denselben Zustand.
#[derive(Debug, Clone, Default)]
pub struct Cancel(Arc<AtomicBool>);

impl Cancel {
    /// Neuer, nicht abgebrochener Zustand.
    pub fn new() -> Cancel {
        Cancel::default()
    }

    /// Abbrechen (wirkt auf alle Klone; mehrfach harmlos).
    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    /// Wurde abgebrochen?
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

/// Mindestauflösung für den Druck (Punkte je Zoll in der gesetzten Größe).
pub const MIN_DPI: f64 = 300.0;

/// Ein Rasterbild, das gedruckt zu grob wäre: Pfad (Normalform, z. B. `bilder/umschlag.png`; ohne
/// bekannte Datei leer), Seite (1-basiert), Pixel des Bilds, gesetzte Größe in mm, erreichte
/// Auflösung (die kleinere beider Richtungen) und die Pixel für [`MIN_DPI`].
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
    /// Deutsch, z. B. „Bild „umschlag.png“: 113 dpi bei 18,7 × 27,3 cm – für 300 dpi braucht es
    /// 2209 × 3224 Pixel“.
    pub message: String,
}

/// Ergebnis von [`compile_pdf`]: Stand, die Bytes des PDF (`None` genau dann, wenn Typst einen Fehler
/// meldete), Seitenzahl, Meldungen und Druckprüfung.
#[derive(Debug, Clone, PartialEq)]
pub struct Pdf {
    pub stand: String,
    pub bytes: Option<Vec<u8>>,
    pub pages: usize,
    pub diagnostics: Vec<Diagnostic>,
    pub print: Vec<PrintWarning>,
}

impl Pdf {
    /// Gab es einen Fehler (dann ohne Bytes)?
    pub fn has_errors(&self) -> bool {
        self.bytes.is_none() || self.diagnostics.iter().any(|d| d.severity == Severity::Error)
    }
}

impl Rendered {
    /// Gab es einen Fehler?
    pub fn has_errors(&self) -> bool {
        self.diagnostics.iter().any(|d| d.severity == Severity::Error)
    }
}

/// Meldung bei `@preview`- und anderen Paket-Importen.
pub const OFFLINE: &str = "Typst-Pakete werden nicht geladen (offline)";

/// Name der Hauptdatei im virtuellen Wurzelordner.
const MAIN_PATH: &str = "/main.typ";

/// Normalform eines Pfads: ohne führendes `/` und `./`, ohne leere Teile; `..` oder leer → `None`.
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

/// SHA-256 (hex) über `main` und alle Dateien, nach Pfad sortiert und mit Längen davor – jede
/// Änderung eines Bytes, eines Pfads, eine neue oder fehlende Datei ändert den Stand, die
/// Reihenfolge der Dateien in [`Source::files`] nicht.
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

/// SHA-256 (hex, klein) über beliebige Bytes, z. B. über ein erzeugtes PDF (Antwort von `work.pdf`).
pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

/// Schriften eines Prozesses: der Vorrat und wie viele davon (vorn) eingebettet sind.
struct Fonts {
    store: FontStore,
    embedded: usize,
}

/// Schriften: eingebettete zuerst, dann die Systemschriften; einmal je Prozess.
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

/// Schriftsuche vorab (Runde 18a): eingebettete und Systemschriften einmal je Prozess laden, damit der erste
/// Satz nicht darauf wartet. Blockierend (bei vielen Systemschriften spürbar) – im Server in `spawn_blocking`.
/// Gibt die Zahl der Schriften zurück.
pub fn warm_fonts() -> usize {
    fonts().book().families().count()
}

/// Eine Schriftfamilie für die Auswahl im Umschlag-Editor (Runde 27, `cover.fonts`): Name, vorhandene Schnitte
/// (`normal`, `bold`, `italic`) und Herkunft.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct FontFamily {
    pub name: String,
    /// Vorhandene Schnitte (`normal`, `bold`, `italic` …).
    pub styles: Vec<&'static str>,
    /// Mit Baruch eingebettet (statt Systemschrift).
    pub embedded: bool,
}

/// Alle Schriftfamilien (eingebettete und Systemschriften), nach Namen sortiert und ohne Doppel (Typst führt
/// Familien ohne Unterschied von Groß- und Kleinschreibung). Je Familie die Schnitte, die es wirklich gibt:
/// `normal` (aufrecht, unter Halbfett), `bold` (aufrecht, ab Halbfett 600), `italic` (kursiv oder schräg); dazu, ob
/// sie mit Baruch eingebettet ist (`embedded`, z. B. Libertinus Serif) oder vom System kommt. Blockierend wie
/// [`warm_fonts`] – im Server in `spawn_blocking`.
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

/// Gibt es die Schriftfamilie `name` (ohne Unterschied von Groß- und Kleinschreibung)? Blockierend wie
/// [`warm_fonts`].
pub fn has_font_family(name: &str) -> bool {
    let wanted = name.trim().to_lowercase();
    !wanted.is_empty() && fonts().book().families().any(|(family, _)| family.to_lowercase() == wanted)
}

/// Breite und Höhe (pt) einer gesetzten SVG-Seite aus ihrem Kopf (`width="…pt" height="…pt"`).
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

/// Breite und Höhe (pt) jeder Seite eines Satzes (gleiche Reihenfolge wie [`Rendered::pages`]); eine unlesbare
/// Seite ergibt (0, 0).
pub fn page_sizes_pt(rendered: &Rendered) -> Vec<(f64, f64)> {
    rendered.pages.iter().map(|page| svg_size_pt(page).unwrap_or((0.0, 0.0))).collect()
}

/// Die Welt eines Laufs: `main` und die Dateien in Normalform. Ihre Antworten bleiben während eines Laufs gleich
/// (Typst prüft das, siehe Abbruch im Modulkopf).
struct Baruch {
    main: TypstSource,
    files: HashMap<String, Bytes>,
}

/// Format einer Bilddatei an ihren ersten Bytes (Typst-Wort: `png`, `jpg`, `gif`, `webp`, `svg`); sonst `None`.
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
            // Abgeschnitten mitten in einem Zeichen: bis zur letzten vollständigen Stelle.
            let cut = &bytes[..bytes.len().min(4096)];
            std::str::from_utf8(&cut[..cut.len().saturating_sub(3)]).ok()
        })?;
        let head = head.trim_start_matches('\u{feff}').trim_start();
        (head.starts_with('<') && head.contains("<svg")).then_some("svg")
    }
}

/// Format, das Typst aus der Endung eines Pfads ableitet (wie `typst_library::visualize::image`).
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

/// Typwechsel (Runde 21c): Ein Bild kann seinen Typ wechseln (PNG ersetzt SVG), während Vorlage und Text es weiter
/// unter dem alten Pfad mit der alten Endung nennen. Typst nimmt das Format aus der Endung und scheitert dann
/// („failed to parse SVG“). Für jede Datei, deren Inhalt nicht zur Endung passt, bekommt jeder Aufruf
/// `image("<Pfad>"…)` mit einem Zeichenketten-Literal als erstem Argument das Format ausdrücklich (`format: "png"`),
/// direkt hinter dem Literal und ohne neue Zeile (Zeilennummern der Meldungen bleiben). Aufrufe, die schon `format:`
/// nennen, und berechnete Pfade (Variablen) bleiben unverändert. Nichts zu tun → `None`.
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
        // Nur der Bezeichner `image` (nicht `myimage(`), danach optional Leerraum und ein Literal.
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
        // Rest der Argumente bis zur schließenden Klammer: schon ein `format:` darin? Dann nichts ändern.
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

    /// Pfad (Normalform) der Datei mit genau diesen Bytes; unbekannt → leer.
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

/// Tage seit 1970-01-01 → (Jahr, Monat, Tag) im gregorianischen Kalender (Algorithmus von
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

/// Setzt das Werk. Blockierend (Sekunden bei großen Werken) – im Server nur in `spawn_blocking`.
/// Scheitert nie: Fehler stehen in [`Rendered::diagnostics`], die Seiten sind dann leer.
pub fn compile(source: &Source) -> Rendered {
    run(&Baruch::new(source), source, &Cancel::new()).expect("ohne Abbruch gibt es immer ein Ergebnis")
}

/// Wie [`compile`], aber abbrechbar über `cancel` (Runde 18a): `None` = abgebrochen (vor, während oder nach dem
/// Satz; nie halbe Seiten). Was Typst wirklich unterbricht, steht im Modulkopf.
pub fn compile_cancellable(source: &Source, cancel: &Cancel) -> Option<Rendered> {
    if cancel.is_cancelled() {
        return None;
    }
    run(&Baruch::new(source), source, cancel)
}

/// Ein Lauf in der Welt `world`; `None` nur, wenn `cancel` abgebrochen wurde.
fn run(world: &Baruch, source: &Source, cancel: &Cancel) -> Option<Rendered> {
    let warned = typst::compile::<PagedDocument>(world);
    let cancelled = || {
        if cancel.is_cancelled() {
            // Wie nach jedem Lauf ältere Zwischenergebnisse von Typst freigeben.
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
    // Typst merkt sich Zwischenergebnisse prozessweit; ältere als zwei Läufe wieder freigeben, damit
    // der Speicher eines langlebigen Servers nicht mit jedem Werk wächst.
    typst::comemo::evict(2);
    if cancelled() {
        return None;
    }
    Some(Rendered { stand: stand(source), pages, headings, diagnostics, print, page_lines })
}

/// Je Seite die Zeilen von `main`, aus denen Text auf ihr stammt (erste Glyphe jedes Textstücks).
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

/// Setzt das Werk wie [`compile`] (dieselbe Welt, dieselben Schriften und Dateien) und schreibt es
/// als PDF (`typst-pdf`, getaggt, ohne Zeitstempel). Blockierend. Scheitert nie: Bei einem Fehler
/// (Satz oder PDF-Ausgabe) sind `bytes` `None` und `pages` 0, die Meldungen stehen in `diagnostics`.
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

// ---- Rasterbild einer Seite (Runde 21c) --------------------------------------------------------

/// Eine Seite als Rasterbild ([`render_png`]): PNG-Bytes und Größe in Pixeln.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Raster {
    pub png: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

/// Setzt `source` wie [`compile`] und rastert die Seite `page` (0-basiert) mit `typst-render` (Apache-2.0) als PNG –
/// so groß wie möglich, aber höchstens `max_width` × `max_height` Pixel, im Seitenverhältnis der Seite (gerundet auf
/// ganze Pixel). Blockierend. Ein Satzfehler (erste Fehlermeldung), eine fehlende Seite, eine Grenze 0 oder ein
/// Fehler beim Schreiben des PNG → `Err` mit deutscher Meldung, nie ein halbes Bild. Für das EPUB-Cover aus der
/// Vorderseite des Umschlags (Runde 21c, Cover-Größe höchstens 1600 × 2560).
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
                    // Größter Maßstab, bei dem die gerundete Pixelgröße in die Grenzen passt (typst-render rundet).
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

// ---- Messhilfe (Runde 29) ----------------------------------------------------------------------

/// Ein gesetztes Textstück (ein Lauf gleicher Schrift, meist ein Wort oder eine Zeile) für Messungen (Runde 29,
/// Vertrag §5.7): Text, Anfang der Grundlinie links in pt ab der oberen linken Seitenecke, Breite in pt, Schriftgröße in
/// pt (mit Vergrößerung der umgebenden Gruppen) und Familienname der Schrift.
#[derive(Debug, Clone, PartialEq)]
pub struct PlacedText {
    pub text: String,
    pub x_pt: f64,
    pub y_pt: f64,
    pub width_pt: f64,
    pub size_pt: f64,
    pub font: String,
}

/// Je Seite (Index 0 = Seite 1) die gesetzten Textstücke in der Reihenfolge des Rahmens (Kopfzeile, Inhalt,
/// Fußzeile). `None` bei einem Satzfehler. Blockierend wie [`compile`]. Für Tests und die Abnahme der
/// Satz-Einstellungen: Ränder, Seitenzahlen, Kolumnentitel, Schrift und Zeilenabstand lassen sich so in pt messen.
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
                    // Waagrechte Vergrößerung (Länge der transformierten x-Achse) für Breite und Größe.
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

// ---- Druckprüfung ------------------------------------------------------------------------------

/// Alle Rasterbilder unter [`MIN_DPI`], je Seite in der Reihenfolge des Satzes; dasselbe Bild in
/// derselben Größe auf derselben Seite nur einmal.
fn print_warnings(world: &Baruch, document: &PagedDocument) -> Vec<PrintWarning> {
    let mut out: Vec<PrintWarning> = Vec::new();
    for (index, page) in document.pages().iter().enumerate() {
        walk_frame(world, &page.frame, (1.0, 1.0), index + 1, &mut out);
    }
    out
}

/// Rahmen durchgehen; `scale` ist die Vergrößerung der umgebenden Gruppen (x, y).
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

/// Warnung für ein Rasterbild von `width_px` × `height_px` Pixeln, gesetzt in `width_mm` × `height_mm`;
/// `None`, wenn es [`MIN_DPI`] erreicht (oder keine Fläche hat).
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

/// Überschriften aus `main` mit ihrer Seite, nach Zeile sortiert.
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

/// Zeile und Spalte (1-basiert) eines Orts in `main`.
fn main_position(world: &Baruch, span: DiagSpan) -> Option<(usize, usize)> {
    if span.id()? != world.main.id() {
        return None;
    }
    let range = world.range(span)?;
    let (line, column) = world.main.lines().byte_to_line_column(range.start)?;
    Some((line + 1, column + 1))
}

/// Ort einer Meldung: eigener Ort, sonst der erste Aufrufpunkt mit Ort (Datei, Zeile, Spalte).
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

/// Bekannte Meldungen auf Deutsch; die englische bleibt als Hinweis stehen.
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
