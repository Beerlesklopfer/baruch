// SPDX-License-Identifier: AGPL-3.0-or-later
//! Engine of the Typst command line (Round 36, ownership `typst_alias`): typeset a Typst file from disk, without
//! a work and without a database. The arguments are read by `baruch_server::typst_cli`; only the typesetting is
//! here. As a child module `cli` may read the private parts of `lib.rs` (fonts, library, German messages).
//!
//! Differences from typesetting a work ([`crate::compile`]): the files come from disk, from the root folder of
//! the job, and only from there – a path that leads out lexically or through a symlink is blocked. Packages
//! (`@preview/…`) are never loaded by the command line either ([`crate::OFFLINE`]); there is no network access.
//! Fonts are those of the server (embedded, then system); with `font_paths` a stock of its own is created for
//! the run, which additionally searches these folders. The source text is not rewritten (no adding of image
//! formats).

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

/// Version of the built-in Typst engine (as in `Cargo.toml` of this crate).
pub const TYPST_VERSION: &str = "0.15.1";

/// Output format of [`compile_file`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Pdf,
    Png,
    Svg,
}

/// A typesetting job of the command line.
#[derive(Debug, Clone)]
pub struct CompileJob {
    /// Input file (`.typ`).
    pub input: PathBuf,
    /// Root of the project; `None` = folder of the input file. Files outside are blocked.
    pub root: Option<PathBuf>,
    /// Folders with fonts, in addition to the embedded and the system fonts.
    pub font_paths: Vec<PathBuf>,
    pub format: OutputFormat,
    /// Resolution, only for [`OutputFormat::Png`]; default of the command line 144.0.
    pub ppi: f32,
}

/// Result of [`compile_file`]: per page the bytes (with [`OutputFormat::Pdf`] exactly one element) and the warnings.
#[derive(Debug, Clone)]
pub struct CompileOutput {
    pub pages: Vec<Vec<u8>>,
    pub warnings: Vec<String>,
}

/// Typesetting or file error; per message "pfad:zeile:spalte: error: text".
#[derive(Debug, Clone)]
pub struct CompileError {
    pub messages: Vec<String>,
}

/// Fonts of a run: the stock of the process or – with `font_paths` – one of its own.
enum FontSet {
    Shared,
    Own(Box<FontStore>),
}

impl FontSet {
    /// Embedded fonts, system fonts and the folders from `font_paths` (in this order as in the
    /// server; the folders are added).
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

/// The world of a run of the command line: files under `root`, read once and then unchanged (Typst
/// demands equal answers during a run).
struct Disk {
    /// Root folder, resolved (without symlinks) – every path that is read is checked against it.
    root: PathBuf,
    /// Root folder as the caller named it – for the paths in messages.
    shown_root: PathBuf,
    /// Input file as the caller named it.
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
        // A symlink in the root folder must not lead out: resolve first, then check against the root.
        let real = std::fs::canonicalize(&path).map_err(|e| FileError::from_io(e, &path))?;
        if !real.starts_with(&self.root) {
            return Err(FileError::AccessDenied);
        }
        if real.is_dir() {
            return Err(FileError::IsDirectory);
        }
        std::fs::read(&real).map(Bytes::new).map_err(|e| FileError::from_io(e, &path))
    }

    /// Path of a file for messages: the input as named, everything else under the named root folder.
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

/// A message of Typst as "pfad:zeile:spalte: error: text" (without a location only "error: text"); hints follow
/// on lines of their own. The location is the message's own, otherwise the first call site with a location.
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

/// Typesets the file of the job. Blocking. Reads only under the root folder, writes nothing and loads nothing
/// from the network; on an error there are no pages. After the errors [`CompileError::messages`] also holds
/// the warnings of the run.
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
    // As after every typesetting run, release older intermediate results of Typst.
    typst::comemo::evict(2);
    match result {
        Ok(pages) => Ok(CompileOutput { pages, warnings }),
        Err(mut messages) => {
            messages.extend(warnings);
            Err(CompileError { messages })
        }
    }
}

/// The output in the wanted format: one PDF or one image per page; `Err` with finished messages.
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

/// Family names of the available fonts (embedded, system, `font_paths`), sorted, without duplicates.
/// Blocking like [`crate::warm_fonts`].
pub fn font_names(font_paths: &[PathBuf]) -> Vec<String> {
    let fonts = FontSet::new(font_paths);
    let mut names: Vec<String> = fonts.store().book().families().map(|(name, _)| name.to_owned()).collect();
    names.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()).then_with(|| a.cmp(b)));
    names.dedup();
    names
}
