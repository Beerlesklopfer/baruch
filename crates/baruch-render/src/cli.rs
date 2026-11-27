// SPDX-License-Identifier: AGPL-3.0-or-later
//! Engine der Typst-Kommandozeile (Runde 36, Besitz `typst_alias`): eine Typst-Datei von der Platte setzen, ohne
//! Werk und ohne Datenbank. Die Argumente liest `baruch_server::typst_cli`; hier steht nur der Satz. Als Kindmodul
//! darf `cli` die privaten Teile von `lib.rs` lesen (Schriften, Bibliothek, deutsche Meldungen).
//!
//! Unterschiede zum Satz eines Werks ([`crate::compile`]): Die Dateien kommen von der Platte, aus dem Wurzelordner
//! des Auftrags, und nur von dort – ein Pfad, der lexikalisch oder über einen Symlink hinausführt, ist gesperrt.
//! Pakete (`@preview/…`) lädt auch die Kommandozeile nie ([`crate::OFFLINE`]); es gibt keinen Netzzugriff. Schriften
//! sind die des Servers (eingebettet, dann System); mit `font_paths` entsteht für den Lauf ein eigener Vorrat, der
//! zusätzlich diese Ordner durchsucht. Der Quelltext wird nicht umgeschrieben (kein Nachtragen von Bildformaten).

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

use typst::diag::{FileError, FileResult, SourceDiagnostic};
use typst::foundations::{Bytes, Datetime};
use typst::syntax::{DiagSpan, FileId, RootedPath, Source as TypstSource, VirtualPath, VirtualRoot};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Library, World, WorldExt};
use typst_kit::fonts::FontStore;
use typst_layout::PagedDocument;

/// Version der eingebauten Typst-Engine (wie in `Cargo.toml` dieses Crates).
pub const TYPST_VERSION: &str = "0.15.1";

/// Ausgabeformat von [`compile_file`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Pdf,
    Png,
    Svg,
}

/// Ein Satzauftrag der Kommandozeile.
#[derive(Debug, Clone)]
pub struct CompileJob {
    /// Eingabedatei (`.typ`).
    pub input: PathBuf,
    /// Wurzel des Projekts; `None` = Ordner der Eingabedatei. Dateien außerhalb sind gesperrt.
    pub root: Option<PathBuf>,
    /// Ordner mit Schriften, zusätzlich zu den eingebetteten und den Systemschriften.
    pub font_paths: Vec<PathBuf>,
    pub format: OutputFormat,
    /// Auflösung, nur für [`OutputFormat::Png`]; Vorgabe der Kommandozeile 144.0.
    pub ppi: f32,
}

/// Ergebnis von [`compile_file`]: je Seite die Bytes (bei [`OutputFormat::Pdf`] genau ein Element) und die Warnungen.
#[derive(Debug, Clone)]
pub struct CompileOutput {
    pub pages: Vec<Vec<u8>>,
    pub warnings: Vec<String>,
}

/// Satz- oder Dateifehler; je Meldung „pfad:zeile:spalte: error: text“.
#[derive(Debug, Clone)]
pub struct CompileError {
    pub messages: Vec<String>,
}

/// Schriften eines Laufs: der Vorrat des Prozesses oder – mit `font_paths` – ein eigener.
enum FontSet {
    Shared,
    Own(Box<FontStore>),
}

impl FontSet {
    /// Eingebettete Schriften, Systemschriften und die Ordner aus `font_paths` (in dieser Reihenfolge wie im
    /// Server; die Ordner kommen hinzu).
    fn new(font_paths: &[PathBuf]) -> FontSet {
        if font_paths.is_empty() {
            return FontSet::Shared;
        }
        let mut store = FontStore::new();
        store.extend(typst_kit::fonts::embedded());
        store.extend(typst_kit::fonts::system());
        for path in font_paths {
            store.extend(typst_kit::fonts::scan(path));
        }
        FontSet::Own(Box::new(store))
    }

    fn store(&self) -> &FontStore {
        match self {
            FontSet::Shared => super::fonts(),
            FontSet::Own(store) => store,
        }
    }
}

/// Die Welt eines Laufs der Kommandozeile: Dateien unter `root`, einmal gelesen und dann unverändert (Typst
/// verlangt gleiche Antworten während eines Laufs).
struct Disk {
    /// Wurzelordner, aufgelöst (ohne Symlinks) – gegen ihn wird jeder gelesene Pfad geprüft.
    root: PathBuf,
    /// Wurzelordner, wie ihn der Aufrufer nannte – für die Pfade in Meldungen.
    shown_root: PathBuf,
    /// Eingabedatei, wie sie der Aufrufer nannte.
    shown_input: PathBuf,
    main: FileId,
    fonts: FontSet,
    files: Mutex<HashMap<FileId, FileResult<Bytes>>>,
}

impl Disk {
    fn read(&self, id: FileId) -> FileResult<Bytes> {
        if let VirtualRoot::Package(_) = id.root() {
            return Err(FileError::Other(Some(super::OFFLINE.into())));
        }
        let mut files = self.files.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        files.entry(id).or_insert_with(|| self.read_from_disk(id)).clone()
    }

    fn read_from_disk(&self, id: FileId) -> FileResult<Bytes> {
        let path = id.vpath().realize(&self.root).map_err(FileError::Realize)?;
        // Ein Symlink im Wurzelordner darf nicht hinausführen: erst auflösen, dann gegen die Wurzel prüfen.
        let real = std::fs::canonicalize(&path).map_err(|e| FileError::from_io(e, &path))?;
        if !real.starts_with(&self.root) {
            return Err(FileError::AccessDenied);
        }
        if real.is_dir() {
            return Err(FileError::IsDirectory);
        }
        std::fs::read(&real).map(Bytes::new).map_err(|e| FileError::from_io(e, &path))
    }

    /// Pfad einer Datei für Meldungen: die Eingabe wie genannt, alles andere unter dem genannten Wurzelordner.
    fn shown_path(&self, id: FileId) -> String {
        if id == self.main {
            return self.shown_input.display().to_string();
        }
        if let VirtualRoot::Package(_) = id.root() {
            return id.vpath().get_without_slash().to_owned();
        }
        match id.vpath().realize(&self.shown_root) {
            Ok(path) => path.display().to_string(),
            Err(_) => id.vpath().get_without_slash().to_owned(),
        }
    }
}

impl World for Disk {
    fn library(&self) -> &LazyHash<Library> {
        super::library()
    }

    fn book(&self) -> &LazyHash<FontBook> {
        self.fonts.store().book()
    }

    fn main(&self) -> FileId {
        self.main
    }

    fn source(&self, id: FileId) -> FileResult<TypstSource> {
        let bytes = self.read(id)?;
        let text = std::str::from_utf8(bytes.as_slice()).map_err(|_| FileError::InvalidUtf8)?;
        Ok(TypstSource::new(id, text.to_owned()))
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        self.read(id)
    }

    fn font(&self, index: usize) -> Option<Font> {
        self.fonts.store().font(index)
    }

    fn today(&self, offset: Option<typst::foundations::Duration>) -> Option<Datetime> {
        let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).ok()?.as_secs() as i64;
        let shift = offset.map(|d| d.seconds() as i64).unwrap_or(0);
        let (y, m, d) = super::civil_from_days((secs + shift).div_euclid(86_400));
        Datetime::from_ymd(y, m, d)
    }
}

/// Eine Meldung von Typst als „pfad:zeile:spalte: error: text“ (ohne Ort nur „error: text“); Hinweise folgen auf
/// eigenen Zeilen. Der Ort ist der eigene der Meldung, sonst der erste Aufrufpunkt mit Ort.
fn message(world: &Disk, diagnostic: &SourceDiagnostic) -> String {
    let kind = match diagnostic.severity {
        typst::diag::Severity::Error => "error",
        typst::diag::Severity::Warning => "warning",
    };
    let place = std::iter::once(diagnostic.span)
        .chain(diagnostic.trace.iter().map(|point| DiagSpan::from(point.span)))
        .filter(|span| !span.is_detached())
        .find_map(|span| {
            let id = span.id()?;
            let path = world.shown_path(id);
            let position = world.source(id).ok().zip(world.range(span)).and_then(|(source, range)| {
                source.lines().byte_to_line_column(range.start).map(|(line, column)| (line + 1, column + 1))
            });
            Some(match position {
                Some((line, column)) => format!("{path}:{line}:{column}: "),
                None => format!("{path}: "),
            })
        })
        .unwrap_or_default();
    let mut hints: Vec<String> = diagnostic.hints.iter().map(|hint| hint.v.to_string()).collect();
    let text = super::german(&diagnostic.message, &mut hints);
    let mut out = format!("{place}{kind}: {text}");
    for hint in hints {
        out.push_str("\n  Hinweis: ");
        out.push_str(&hint);
    }
    out
}

fn failure(text: String) -> CompileError {
    CompileError { messages: vec![text] }
}

/// Setzt die Datei des Auftrags. Blockierend. Liest nur unter dem Wurzelordner, schreibt nichts und lädt nichts
/// aus dem Netz; bei einem Fehler gibt es keine Seiten. Nach den Fehlern stehen in [`CompileError::messages`] auch
/// die Warnungen des Laufs.
pub fn compile_file(job: &CompileJob) -> Result<CompileOutput, CompileError> {
    let shown_input = job.input.clone();
    let input = std::fs::canonicalize(&job.input)
        .map_err(|e| failure(format!("{}: error: Eingabedatei lässt sich nicht lesen ({e})", shown_input.display())))?;
    if !input.is_file() {
        return Err(failure(format!("{}: error: Eingabe ist keine Datei", shown_input.display())));
    }
    let shown_root = match &job.root {
        Some(root) => root.clone(),
        None => match job.input.parent() {
            Some(parent) if !parent.as_os_str().is_empty() => parent.to_path_buf(),
            _ => PathBuf::from("."),
        },
    };
    let root = std::fs::canonicalize(&shown_root)
        .ok()
        .filter(|root| root.is_dir())
        .ok_or_else(|| failure(format!("{}: error: Wurzelordner (--root) gibt es nicht", shown_root.display())))?;
    let vpath = VirtualPath::virtualize(&root, &input).map_err(|_| {
        failure(format!(
            "{}: error: Eingabedatei liegt nicht im Wurzelordner {} (--root)",
            shown_input.display(),
            shown_root.display()
        ))
    })?;
    if !(job.ppi.is_finite() && job.ppi > 0.0) {
        return Err(failure("error: Auflösung (--ppi) muss größer als 0 sein".into()));
    }
    let world = Disk {
        root,
        shown_root,
        shown_input,
        main: FileId::new(RootedPath::new(VirtualRoot::Project, vpath)),
        fonts: FontSet::new(&job.font_paths),
        files: Mutex::new(HashMap::new()),
    };
    let warned = typst::compile::<PagedDocument>(&world);
    let warnings: Vec<String> = warned.warnings.iter().map(|d| message(&world, d)).collect();
    let result = match warned.output {
        Ok(document) => pages(&world, &document, job),
        Err(errors) => Err(errors.iter().map(|d| message(&world, d)).collect()),
    };
    // Wie nach jedem Satz ältere Zwischenergebnisse von Typst freigeben.
    typst::comemo::evict(2);
    match result {
        Ok(pages) => Ok(CompileOutput { pages, warnings }),
        Err(mut messages) => {
            messages.extend(warnings);
            Err(CompileError { messages })
        }
    }
}

/// Die Ausgabe im gewünschten Format: ein PDF oder je Seite ein Bild; `Err` mit fertigen Meldungen.
fn pages(world: &Disk, document: &PagedDocument, job: &CompileJob) -> Result<Vec<Vec<u8>>, Vec<String>> {
    match job.format {
        OutputFormat::Pdf => typst_pdf::pdf(document, &typst_pdf::PdfOptions::default())
            .map(|bytes| vec![bytes])
            .map_err(|errors| errors.iter().map(|d| message(world, d)).collect()),
        OutputFormat::Svg => {
            let options = typst_svg::SvgOptions::default();
            Ok(document.pages().iter().map(|page| typst_svg::svg(page, &options).into_bytes()).collect())
        }
        OutputFormat::Png => {
            let options = typst_render::RenderOptions {
                pixel_per_pt: typst::utils::Scalar::new(f64::from(job.ppi) / 72.0),
                render_bleed: false,
            };
            document
                .pages()
                .iter()
                .map(|page| {
                    typst_render::render(page, &options)
                        .encode_png()
                        .map_err(|e| vec![format!("error: PNG ließ sich nicht schreiben ({e})")])
                })
                .collect()
        }
    }
}

/// Familiennamen der verfügbaren Schriften (eingebettet, System, `font_paths`), sortiert, ohne Doppelte.
/// Blockierend wie [`crate::warm_fonts`].
pub fn font_names(font_paths: &[PathBuf]) -> Vec<String> {
    let fonts = FontSet::new(font_paths);
    let mut names: Vec<String> = fonts.store().book().families().map(|(name, _)| name.to_owned()).collect();
    names.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()).then_with(|| a.cmp(b)));
    names.dedup();
    names
}
