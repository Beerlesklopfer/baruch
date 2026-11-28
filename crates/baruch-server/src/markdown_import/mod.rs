// SPDX-License-Identifier: AGPL-3.0-or-later
//! Markdown and archive import (Round 21b; interface R21-S1, afterwards owned by markdown-import).
//!
//! Markdown (`.md`, `.markdown`) is converted to Typst ([`convert::to_typst`]; every form that is not mapped becomes
//! a warning, [`crate::import::ImportWarning`]) and imported like a Typst file
//! ([`crate::import::import_generated`]). A ZIP archive ([`crate::archive::read_zip`] with
//! [`crate::archive::IMPORT_LIMITS`]) or an uploaded folder (several files with relative paths) brings exactly
//! one main file (`.typ` or `.md`) and images relative to it ([`Bundle`]). Forms and RPC are in
//! [`crate::works`] (`POST /werke/import`, `work.import`, `work.import_archive`), the CLI calls [`cli_import`].
//!
//! **Main file:** if there is exactly one `.typ`/`.md`/`.markdown` file, that is it; otherwise the one `index.md`,
//! `main.typ` or `README.md` (regardless of case) on the flattest level; otherwise an error that
//! names the candidates. **Images** (`image("…")` in the Typst text or `![…](…)` in the Markdown) count as relative
//! to the folder of the main file, a leading `/` as well (project root as in the Typst import). What is missing, too
//! large or not an image is in `skipped` with a reason; files that nothing names are in the report as the note
//! `archive-unused`.
//!
//! There is no import through MCP (decision 2026-11-05, not for Markdown either).

pub mod convert;

use std::collections::{BTreeMap, HashSet};
use std::path::Path;

use baruch_store::ImportAsset;

use crate::AppState;
use crate::archive::{self, ArchiveError};
use crate::i18n::tc;
use crate::import::{self, ImportReport, ImportWarning, MAX_SOURCE_BYTES, SkippedAsset};
use crate::rpc::{INVALID_PARAMS, RpcError};

/// Does the file name end (regardless of case) in `.md` or `.markdown` and have a name before it?
pub fn is_markdown_name(file_name: &str) -> bool {
    has_suffix(file_name, &[".md", ".markdown"])
}

/// Does the file name end (regardless of case) in `.zip` and have a name before it?
pub fn is_archive_name(file_name: &str) -> bool {
    has_suffix(file_name, &[".zip"])
}

fn has_suffix(file_name: &str, suffixes: &[&str]) -> bool {
    let lower = file_name.to_lowercase();
    suffixes.iter().any(|suffix| lower.len() > suffix.len() && lower.ends_with(suffix))
}

/// File name without folder and extension (title of the chapter for text before the first heading).
fn stem(file_name: &str) -> String {
    let base = file_name.rsplit(['/', '\\']).next().unwrap_or(file_name);
    Path::new(base).file_stem().and_then(|s| s.to_str()).unwrap_or(base).trim().to_owned()
}

fn invalid(message: impl Into<String>) -> RpcError {
    RpcError::new(INVALID_PARAMS, message)
}

/// Markdown text as a new work (upload of a single file, RPC `work.import`): without images – every image the
/// text names is in `skipped` with a reason (only the text arrived). Larger than 16 MiB → `INVALID_PARAMS`.
pub async fn import_text(
    state: &AppState,
    source: &str,
    file_name: &str,
    title: Option<&str>,
) -> Result<ImportReport, RpcError> {
    let file_name = file_name.rsplit(['/', '\\']).next().unwrap_or("").trim();
    if !is_markdown_name(file_name) {
        return Err(invalid(tc("markdown_msg.import.only_markdown", &[])));
    }
    if source.len() as u64 > MAX_SOURCE_BYTES {
        return Err(invalid(tc("markdown_msg.file.too_large", &[("file", file_name.into())])));
    }
    let converted = convert::to_typst(source, &stem(file_name));
    let skipped = baruch_typst::images::image_paths(&converted.typst)
        .into_iter()
        .map(|path| SkippedAsset { path, reason: tc("markdown_msg.import.not_sent", &[]) })
        .collect();
    import::import_generated(state, &converted.typst, file_name, title, Vec::new(), skipped, converted.warnings).await
}

/// Files of an archive or folder upload: path (with `/`, checked) → content, order as delivered.
#[derive(Debug, Clone, Default)]
pub struct Bundle {
    files: Vec<(String, Vec<u8>)>,
}

impl Bundle {
    /// From the entries of a ZIP archive ([`archive::read_zip`] has already checked paths, duplicates and limits).
    pub fn from_archive(bytes: &[u8]) -> Result<Bundle, ArchiveError> {
        let entries = archive::read_zip(bytes, &archive::IMPORT_LIMITS)?;
        Ok(Bundle { files: entries.into_iter().map(|e| (e.path, e.bytes)).collect() })
    }

    /// From uploaded files with relative paths (folder upload, `webkitdirectory`). The same rules as in the
    /// archive: paths with `/`, not absolute, without `..`, `\`, drive and control characters, no duplicates (not even
    /// ones differing only in case), at most [`archive::IMPORT_LIMITS`] entries and bytes.
    pub fn from_files(files: Vec<(String, Vec<u8>)>) -> Result<Bundle, ArchiveError> {
        let limits = archive::IMPORT_LIMITS;
        if files.len() > limits.max_entries {
            return Err(ArchiveError::TooLarge(tc(
                "markdown_msg.import.too_many_files",
                &[("max", limits.max_entries.to_string().into())],
            )));
        }
        let mut seen = HashSet::new();
        let mut total: u64 = 0;
        for (path, bytes) in &files {
            check_upload_path(path)?;
            if !seen.insert(path.to_lowercase()) {
                return Err(ArchiveError::Unsafe {
                    path: path.clone(),
                    reason: tc("markdown_msg.import.duplicate", &[]),
                });
            }
            if bytes.len() as u64 > limits.max_entry_bytes {
                return Err(ArchiveError::TooLarge(tc("markdown_msg.import.file_too_large", &[("path", path.into())])));
            }
            total += bytes.len() as u64;
            if total > limits.max_total_bytes {
                return Err(ArchiveError::TooLarge(tc("markdown_msg.import.total_too_large", &[])));
            }
        }
        Ok(Bundle { files })
    }

    /// The main file (see the module doc); errors name the candidates.
    fn main_file(&self) -> Result<&str, String> {
        let sources: Vec<&str> = self
            .files
            .iter()
            .map(|(p, _)| p.as_str())
            .filter(|p| import::is_typst_name(base(p)) || is_markdown_name(base(p)))
            .collect();
        match sources.as_slice() {
            [] => return Err(tc("markdown_msg.import.no_main", &[])),
            [one] => return Ok(one),
            _ => {}
        }
        let named: Vec<&str> = sources
            .iter()
            .copied()
            .filter(|p| matches!(base(p).to_lowercase().as_str(), "index.md" | "main.typ" | "readme.md"))
            .collect();
        let depth = |p: &str| p.matches('/').count();
        let flattest = named.iter().map(|p| depth(p)).min();
        let top: Vec<&str> = named.iter().copied().filter(|p| Some(depth(p)) == flattest).collect();
        match top.as_slice() {
            [one] => Ok(one),
            _ => {
                let shown: Vec<&str> = sources.iter().copied().take(10).collect();
                let more = if sources.len() > shown.len() { ", …" } else { "" };
                Err(tc("markdown_msg.import.several_main", &[("files", format!("{}{more}", shown.join(", ")).into())]))
            }
        }
    }
}

/// File name without folder.
fn base(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// Check the path of an uploaded file (rules as in [`archive::read_zip`]).
fn check_upload_path(path: &str) -> Result<(), ArchiveError> {
    // The reasons are those of the archive reader (`archive_msg.reason.*`), plus the empty part of a folder path.
    let bad = |reason: String| ArchiveError::Unsafe { path: path.escape_debug().to_string(), reason };
    if path.is_empty() || path.ends_with('/') {
        return Err(bad(tc("archive_msg.reason.empty_name", &[])));
    }
    if path.chars().any(char::is_control) {
        return Err(bad(tc("archive_msg.reason.control", &[])));
    }
    if path.starts_with('/') {
        return Err(bad(tc("archive_msg.reason.absolute", &[])));
    }
    if path.contains('\\') {
        return Err(bad(tc("archive_msg.reason.backslash", &[])));
    }
    if path.len() >= 2 && path.as_bytes()[1] == b':' && path.as_bytes()[0].is_ascii_alphabetic() {
        return Err(bad(tc("archive_msg.reason.drive", &[])));
    }
    if path.split('/').any(|part| part == ".." || part == "." || part.is_empty()) {
        return Err(bad(tc("markdown_msg.import.bad_part", &[])));
    }
    Ok(())
}

/// An archive or folder as a new work: choose the main file, convert Markdown, read images relative to the main
/// file from the bundle, report what is unused. `origin` is the name of the archive or folder (for title and note).
pub async fn import_bundle(
    state: &AppState,
    bundle: Bundle,
    origin: &str,
    title: Option<&str>,
) -> Result<ImportReport, RpcError> {
    let main = bundle.main_file().map_err(invalid)?.to_owned();
    let bytes = bundle.files.iter().find(|(p, _)| *p == main).map(|(_, b)| b.clone()).unwrap_or_default();
    if bytes.len() as u64 > MAX_SOURCE_BYTES {
        return Err(invalid(tc("markdown_msg.file.too_large", &[("file", main.as_str().into())])));
    }
    let text = String::from_utf8(bytes)
        .map_err(|_| invalid(tc("markdown_msg.file.not_utf8", &[("file", main.as_str().into())])))?;
    let main_base = base(&main).to_owned();
    // index.md/main.typ/README.md say nothing about the work: title and name then come from the archive or folder.
    let generic = matches!(main_base.to_lowercase().as_str(), "index.md" | "main.typ" | "readme.md");
    let origin_stem = stem(origin);
    let extension = main_base.rsplit('.').next().unwrap_or("typ").to_owned();
    let file_name =
        if generic && !origin_stem.is_empty() { format!("{origin_stem}.{extension}") } else { main_base.clone() };
    let (typst, mut warnings) = if is_markdown_name(&main_base) {
        let converted = convert::to_typst(&text, &stem(&file_name));
        (converted.typst, converted.warnings)
    } else {
        (text, Vec::new())
    };
    let folder = main.rsplit_once('/').map_or("", |(dir, _)| dir).to_owned();
    let files: BTreeMap<String, &Vec<u8>> = bundle.files.iter().map(|(p, b)| (p.clone(), b)).collect();
    let mut used: HashSet<String> = HashSet::from([main.clone()]);
    let mut assets = Vec::new();
    let mut skipped = Vec::new();
    let mut total: u64 = 0;
    for path in baruch_typst::images::image_paths(&typst) {
        match bundle_asset(&files, &folder, &path, &mut total) {
            Ok((key, file_name, bytes)) => {
                used.insert(key);
                assets.push(ImportAsset { path, file_name, bytes });
            }
            Err((key, reason)) => {
                if let Some(key) = key {
                    used.insert(key);
                }
                skipped.push(SkippedAsset { path, reason });
            }
        }
    }
    let unused: Vec<&str> = bundle.files.iter().map(|(p, _)| p.as_str()).filter(|p| !used.contains(*p)).collect();
    if !unused.is_empty() {
        let shown: Vec<&str> = unused.iter().copied().take(10).collect();
        warnings.push(ImportWarning {
            code: "archive-unused".into(),
            message: tc(
                "markdown_msg.import.unused",
                &[
                    ("count", unused.len().to_string().into()),
                    ("main", main.as_str().into()),
                    (
                        "files",
                        format!("{}{}", shown.join(", "), if unused.len() > shown.len() { ", …" } else { "" }).into(),
                    ),
                ],
            ),
            line: None,
        });
    }
    import::import_generated(state, &typst, &file_name, title, assets, skipped, warnings).await
}

/// An image from the bundle: path from the text relative to the folder of the main file (leading `/` = this folder).
/// Result: key in the bundle, file name, bytes; error with the key (if found) and the reason.
type Found = (String, String, Vec<u8>);
fn bundle_asset(
    files: &BTreeMap<String, &Vec<u8>>,
    folder: &str,
    path: &str,
    total: &mut u64,
) -> Result<Found, (Option<String>, String)> {
    if path.contains("://") {
        return Err((None, tc("markdown_msg.import.address", &[])));
    }
    baruch_store::check_asset_path(path).map_err(|e| (None, e.to_string()))?;
    let normal =
        baruch_store::normalize_asset_path(path).ok_or_else(|| (None, tc("markdown_msg.import.no_file_name", &[])))?;
    let key = if folder.is_empty() { normal } else { format!("{folder}/{normal}") };
    let Some(bytes) = files.get(&key) else {
        return Err((None, tc("markdown_msg.import.not_in_bundle", &[])));
    };
    import::check_image(bytes, total).map_err(|reason| (Some(key.clone()), reason))?;
    let file_name = base(&key).to_owned();
    Ok((key, file_name, (*bytes).clone()))
}

/// Read the archive (ZIP bytes) and create it as a new work; errors of the archive → `INVALID_PARAMS` with a reason
/// in the interface language.
pub async fn import_archive(
    state: &AppState,
    bytes: &[u8],
    file_name: &str,
    title: Option<&str>,
) -> Result<ImportReport, RpcError> {
    let bundle = Bundle::from_archive(bytes).map_err(|e| invalid(format!("{file_name}: {e}")))?;
    import_bundle(state, bundle, file_name, title).await
}

/// `import DATEI.md|DATEI.markdown|DATEI.zip [--title …]`: `Ok(text)` → standard output (exit 0), `Err(text)` →
/// standard error (exit 1). Markdown reads the images relative to the folder of the file (like the Typst import: not
/// out of it, no link to the outside), a ZIP from the archive. The notes are in the report (`Hinweis (<code>): …`).
/// Like the other imports of the command line (Round 39a) it runs in the German interface language: `main.rs` prints
/// its own lines in German, and the report must not mix languages.
pub async fn cli_import(state: &AppState, file: &Path, title: Option<&str>) -> Result<String, String> {
    crate::i18n::scope(crate::i18n::Lang::De, cli_import_in_scope(state, file, title)).await
}

async fn cli_import_in_scope(state: &AppState, file: &Path, title: Option<&str>) -> Result<String, String> {
    let shown = file.display().to_string();
    let name = file
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| tc("markdown_msg.file.no_name", &[("file", shown.as_str().into())]))?;
    let unreadable = |e: std::io::Error| {
        tc("markdown_msg.file.unreadable", &[("file", shown.as_str().into()), ("detail", e.to_string().into())])
    };
    let meta = std::fs::metadata(file).map_err(unreadable)?;
    if !meta.is_file() {
        return Err(tc("markdown_msg.file.not_file", &[("file", shown.as_str().into())]));
    }
    let limit = if is_archive_name(name) { archive::IMPORT_LIMITS.max_total_bytes } else { MAX_SOURCE_BYTES };
    if meta.len() > limit {
        return Err(tc(
            "markdown_msg.file.too_large_mib",
            &[("file", shown.as_str().into()), ("mib", (limit / (1024 * 1024)).to_string().into())],
        ));
    }
    let owned = file.to_owned();
    let bytes = tokio::task::spawn_blocking(crate::i18n::carry(move || std::fs::read(owned)))
        .await
        .map_err(|e| {
            tc("markdown_msg.typst.read_aborted", &[("path", shown.as_str().into()), ("detail", e.to_string().into())])
        })?
        .map_err(unreadable)?;
    let result = if is_archive_name(name) {
        import_archive(state, &bytes, name, title).await
    } else {
        let text = String::from_utf8(bytes)
            .map_err(|_| tc("markdown_msg.file.not_utf8", &[("file", shown.as_str().into())]))?;
        let converted = convert::to_typst(&text, &stem(name));
        let folder = file.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new(".")).to_owned();
        let typst = converted.typst.clone();
        let (assets, skipped) =
            tokio::task::spawn_blocking(crate::i18n::carry(move || import::read_assets_in(&folder, &typst)))
                .await
                .map_err(|e| e.to_string())?;
        import::import_generated(state, &converted.typst, name, title, assets, skipped, converted.warnings).await
    };
    let report = result.map_err(|e| {
        tc("markdown_msg.import.failed", &[("file", shown.as_str().into()), ("detail", e.message.into())])
    })?;
    Ok(report_text(&report))
}

/// Report for the command line (interface language, one item per line; [`cli_import`] runs it in German).
pub fn report_text(report: &ImportReport) -> String {
    let mut lines = vec![
        tc(
            "markdown_msg.report.work",
            &[("title", report.title.as_str().into()), ("id", report.work.to_string().into())],
        ),
        tc("markdown_msg.report.folder", &[("slug", report.slug.as_str().into())]),
        tc("markdown_msg.report.sections", &[("count", report.nodes.to_string().into())]),
        tc("markdown_msg.report.images", &[("count", report.assets.len().to_string().into())]),
    ];
    for skipped in &report.skipped {
        lines.push(tc(
            "markdown_msg.report.skipped",
            &[("path", skipped.path.as_str().into()), ("reason", skipped.reason.as_str().into())],
        ));
    }
    for warning in &report.warnings {
        let (code, message) = (warning.code.as_str(), warning.message.as_str());
        match warning.line {
            Some(line) => lines.push(tc(
                "markdown_msg.report.note_line",
                &[("code", code.into()), ("line", line.to_string().into()), ("message", message.into())],
            )),
            None => lines.push(tc("markdown_msg.report.note", &[("code", code.into()), ("message", message.into())])),
        }
    }
    match (&report.commit, &report.mirror) {
        (Some(commit), _) => lines.push(tc("markdown_msg.report.commit", &[("commit", commit.as_str().into())])),
        (None, Some(error)) => {
            lines.push(tc("markdown_msg.report.mirror_failed", &[("error", error.as_str().into())]));
        }
        (None, None) => lines.push(tc("markdown_msg.report.no_commit", &[])),
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markdown_and_archive_names_need_a_name_before_the_suffix() {
        assert!(is_markdown_name("kapitel.md"), ".md");
        assert!(is_markdown_name("Kapitel.MarkDown"), "Groß/klein egal");
        assert!(!is_markdown_name(".md"), "ohne Namen davor keine Datei");
        assert!(!is_markdown_name("kapitel.mdx"), "MDX ist kein Markdown, das der Import kennt");
        assert!(is_archive_name("buch.ZIP"), ".zip, Groß/klein egal");
        assert!(!is_archive_name(".zip"), "ohne Namen davor keine Datei");
        assert!(!is_archive_name("buch.baruch"), "eine Austauschdatei ist zwar ein ZIP, gehört aber zum Austausch");
    }

    fn bundle(paths: &[&str]) -> Bundle {
        Bundle::from_files(paths.iter().map(|p| ((*p).to_owned(), b"x".to_vec())).collect()).expect("gültige Pfade")
    }

    #[test]
    fn the_main_file_is_the_only_source_or_the_flattest_index() {
        assert_eq!(
            bundle(&["buch/kapitel.md", "buch/bilder/a.png"]).main_file(),
            Ok("buch/kapitel.md"),
            "einzige Quelle"
        );
        assert_eq!(
            bundle(&["buch/index.md", "buch/teil/index.md", "buch/notiz.md"]).main_file(),
            Ok("buch/index.md"),
            "flachste index.md gewinnt"
        );
        let both = bundle(&["a.md", "b.md"]).main_file().unwrap_err();
        assert!(both.contains("a.md") && both.contains("b.md"), "Fehler nennt die Kandidaten: {both}");
        assert!(bundle(&["bild.png"]).main_file().is_err(), "ohne Quelle ein Fehler");
    }

    #[test]
    fn uploaded_paths_follow_the_archive_rules() {
        for bad in ["/abs.md", "../flucht.md", "a\\b.md", "C:/x.md", "a/./b.md", "a//b.md", "zeile\n.md"] {
            assert!(
                Bundle::from_files(vec![(bad.to_owned(), b"x".to_vec())]).is_err(),
                "{bad:?} muss abgelehnt werden (wie im Archiv)"
            );
        }
        let twice = Bundle::from_files(vec![("A.md".into(), b"x".to_vec()), ("a.md".into(), b"y".to_vec())]);
        assert!(matches!(twice, Err(ArchiveError::Unsafe { .. })), "Doppel nur in Groß/klein: {twice:?}");
    }
}
