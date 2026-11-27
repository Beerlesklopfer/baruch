// SPDX-License-Identifier: AGPL-3.0-or-later
//! Engine der Typst-Kommandozeile (Runde 36, Schnittstelle R36-S1, Besitz `typst_alias`): eine Typst-Datei von der
//! Platte setzen, ohne Werk und ohne Datenbank. Die Argumente liest `baruch_server::typst_cli`; hier steht nur der
//! Satz. Als Kindmodul darf `cli` die privaten Teile von `lib.rs` lesen (Schriften, Bibliothek, Welt).
//!
//! Stand S1: nur Typen und Signaturen, die Körper sind Stubs.

use std::path::PathBuf;

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

/// Setzt die Datei des Auftrags.
pub fn compile_file(job: &CompileJob) -> Result<CompileOutput, CompileError> {
    // STUB: Typst-Kommandozeile (Runde 36, typst_alias)
    let _ = job;
    Err(CompileError { messages: vec!["noch nicht umgesetzt (Runde 36, typst_alias)".into()] })
}

/// Familiennamen der verfügbaren Schriften (eingebettet, System, `font_paths`), sortiert, ohne Doppelte.
pub fn font_names(font_paths: &[PathBuf]) -> Vec<String> {
    // STUB: Typst-Kommandozeile (Runde 36, typst_alias)
    let _ = font_paths;
    Vec::new()
}
