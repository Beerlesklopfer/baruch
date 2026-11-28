// SPDX-License-Identifier: AGPL-3.0-or-later
//! Typst file as a new work (contract B1.5, section 4; Round 4): import into the database
//! ([`baruch_store::Store::import_typst_with_assets`]), then the Git mirror of the new work with the
//! commit „Import aus <datei>“. Called by `baruch-server import [--title …]`, by the upload through
//! `POST /werke/import` and RPC `work.import` ([`import_upload`], `crate::works`) and from tests.
//! A title that another work already carries is refused (`DuplicateTitle`, -32000).
//!
//! Images (contract Round 6, section 4d): only the import from the command line ([`import_file_as`])
//! reads the images that template and text name through `image("…")`
//! ([`baruch_typst::images::image_paths`]), relative to the folder of the source file (also paths with a
//! leading `/` or `./`, normal form through [`baruch_store::normalize_asset_path`]). A file
//! outside this folder (through a symlink or `..`), a path the store does not save
//! (`..`, addresses, [`baruch_store::check_asset_path`]), a missing file,
//! one that is too large or not an image file is skipped with a reason ([`SkippedAsset`]); work and images
//! are created in one transaction. Uploading a single file and `work.import` (text only) create no images;
//! ZIP and folder (Round 21b, [`crate::markdown_import`]) bring them along ([`check_image`]).
//! [`export_to_dir`] writes work and images back into a folder (`export --out`) and since Round 21b checks, before
//! the first file, links, files as intermediate folders and file/folder collisions (`check_targets`).
//!
//! Round 21b: every intervention in the text of an export (book end cut, projection removed, rule block moved into
//! the typeset settings) is an [`ImportWarning`] in the report; codes `import-…`.
//!
//! There is no import through MCP (decision 2026-11-05): an import creates current revisions under
//! the name of the human, and an assistant must not do that.

use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};

use baruch_store::{ImportAsset, MAX_ASSET_BYTES, Store, StoreError, WorkId};
use serde::Serialize;

use crate::i18n::tc;
use crate::rpc::{INTERNAL, INVALID_PARAMS, REFUSED, RpcError};
use crate::{AppState, mirror};

/// Largest file the import accepts.
pub const MAX_SOURCE_BYTES: u64 = 16 * 1024 * 1024;

/// An import reads at most this many bytes of images per source file.
pub const MAX_IMPORT_ASSET_BYTES: u64 = 64 * 1024 * 1024;

/// Result of an import.
#[derive(Debug, Clone, Serialize)]
pub struct ImportReport {
    pub work: WorkId,
    pub slug: String,
    pub title: String,
    /// Number of sections (chapters, sections, subsections).
    pub nodes: usize,
    /// Commit id in the mirror of the work.
    pub commit: Option<String>,
    /// Error text if the mirror failed (the work is created nevertheless).
    pub mirror: Option<String>,
    /// Created images (names); only the import from the command line reads images.
    pub assets: Vec<String>,
    /// Skipped image paths with the reason.
    pub skipped: Vec<SkippedAsset>,
    /// Notes of the import (conversion, cut, limit; Round 21): what was not taken over verbatim.
    pub warnings: Vec<ImportWarning>,
}

/// Note of an import (conversion, cut, limit); never silent discarding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ImportWarning {
    pub code: String,
    pub message: String,
    pub line: Option<usize>,
}

/// An image path from the text that was not read during the import.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SkippedAsset {
    pub path: String,
    pub reason: String,
}

/// Hint of the CLI when the title is already taken, in the German wording (the command line runs in German; the
/// page of the PDF/EPUB import recognises it in a message). [`title_hint`] gives it in the interface language.
pub const TITLE_HINT: &str = "Mit --title \"…\" einen anderen Titel wählen.";

/// [`TITLE_HINT`] in the interface language (Round 39c).
pub fn title_hint() -> String {
    tc("markdown_msg.typst.title_hint", &[])
}

/// Import source text. Write lock as for accepting and saving, so that the commit contains exactly
/// the new state. Already imported or title taken → `REFUSED`, no heading →
/// `INVALID_PARAMS`. Note of every revision and subject of the commit: „Import aus <datei>“.
pub async fn import_typst(state: &AppState, source: &str, file_name: &str) -> Result<ImportReport, RpcError> {
    Ok(import_source(state, source, file_name, None, Vec::new(), Vec::new(), Vec::new()).await?)
}

/// Uploaded file (form `POST /werke/import`, RPC `work.import`): only file names ending in `.typ`
/// (case does not matter), at most 16 MiB, otherwise `INVALID_PARAMS`; `title` (empty = from the
/// file) as for the CLI with `--title`. Folders in the file name are dropped. Images do not come along this way
/// (text only); they can be created afterwards on the page „Bilder“ with their path.
pub async fn import_upload(
    state: &AppState,
    source: &str,
    file_name: &str,
    title: Option<&str>,
) -> Result<ImportReport, RpcError> {
    let file_name = file_name.rsplit(['/', '\\']).next().unwrap_or("").trim();
    if !is_typst_name(file_name) {
        return Err(RpcError::new(INVALID_PARAMS, tc("markdown_msg.typst.only_typ", &[])));
    }
    if source.len() as u64 > MAX_SOURCE_BYTES {
        return Err(RpcError::new(
            INVALID_PARAMS,
            tc("markdown_msg.file.too_large", &[("file", file_name.to_string().into())]),
        ));
    }
    Ok(import_source(state, source, file_name, title, Vec::new(), Vec::new(), Vec::new()).await?)
}

/// Does the file name end in `.typ` (regardless of case) and have a name before it?
pub fn is_typst_name(file_name: &str) -> bool {
    let lower = file_name.to_lowercase();
    lower.len() > 4 && lower.ends_with(".typ")
}

/// Common path of all imports; errors as `StoreError`, so that the CLI recognises a taken title.
/// Work and images in one transaction.
async fn import_source(
    state: &AppState,
    source: &str,
    file_name: &str,
    title: Option<&str>,
    assets: Vec<ImportAsset>,
    skipped: Vec<SkippedAsset>,
    warnings: Vec<ImportWarning>,
) -> Result<ImportReport, StoreError> {
    // Round 31: the book end of an export (index and glossary pages from `// Baruch: Register und Glossar` on) belongs
    // to no section; it is created anew on export from the typeset settings and does not move into the last section.
    // Such an export also carries the additions of the projection (automatic scripture reference marks, linked first
    // mention); they are taken out, otherwise the text would refer to glossary labels that do not exist in the new
    // work.
    // Round 21b: only a generated book end is cut, and every intervention is in the report.
    let mut warnings = warnings;
    let original = source;
    let (source, exported) = baruch_typst::index::strip_back_matter(source);
    if exported {
        warnings.push(ImportWarning {
            code: "import-back-matter-cut".into(),
            message: tc("markdown_msg.typst.back_matter_cut", &[]),
            line: Some(baruch_typst::line_at(original, source.len())),
        });
    } else if baruch_typst::index::has_back_matter_mark(source) {
        warnings.push(ImportWarning {
            code: "import-back-matter-kept".into(),
            message: tc("markdown_msg.typst.back_matter_kept", &[]),
            line: None,
        });
    }
    let unprojected = if exported {
        baruch_typst::index::strip_projection(source)
    } else {
        baruch_typst::index::strip_first_mentions(source)
    };
    if unprojected != source {
        warnings.push(ImportWarning {
            code: "import-projection-removed".into(),
            message: tc("markdown_msg.typst.projection_removed", &[]),
            line: None,
        });
    }
    let source: &str = &unprojected;
    let held = state.write_lock.lock().await;
    let note = format!("Import aus {file_name}");
    let (imported, names) =
        state.store.import_typst_with_assets(source, file_name, mirror::HUMAN_NAME, &note, title, &assets).await?;
    let (commit, mirror) = match mirror::sync_work(state, &held, imported.work, &note).await {
        Ok(commit) => (commit, None),
        Err(e) => (None, Some(e)),
    };
    // Round 21b: the rule block „// Baruch: Satz“ of an export ends up in the typeset settings (store), otherwise it
    // stays in the work template – both are in the report.
    if state.store.work_typeset(imported.work).await.ok().flatten().is_some() {
        warnings.push(ImportWarning {
            code: "import-typeset-adopted".into(),
            message: tc("markdown_msg.typst.typeset_adopted", &[]),
            line: None,
        });
    } else if let Ok(template) = state.store.work_template(imported.work).await
        && template.lines().any(|l| l.trim_end() == baruch_typst::typeset::RULES_MARK)
    {
        warnings.push(ImportWarning {
            code: "import-typeset-kept".into(),
            message: tc("markdown_msg.typst.typeset_kept", &[]),
            line: None,
        });
    }
    Ok(ImportReport {
        work: imported.work,
        slug: imported.slug,
        title: imported.title,
        nodes: imported.nodes,
        commit,
        mirror,
        assets: names,
        skipped,
        warnings,
    })
}

/// Read the file (UTF-8, at most 16 MiB) and import it; the file name comes from `path`.
/// For the CLI: errors name the reason given by the operating system.
///
/// Before opening, `std::fs::metadata` checks whether `path` is a regular file (also after resolving
/// a link). A FIFO, a device or a folder is refused this way without being opened:
/// `File::open` on a FIFO without a writer would otherwise block. The reading itself runs in
/// `spawn_blocking`, so that no Tokio worker waits for the file system.
pub async fn import_file(state: &AppState, path: &Path) -> Result<ImportReport, RpcError> {
    import_file_as(state, path, None).await
}

/// Like [`import_file`], with its own title (`--title`). If the title is already taken, the
/// message names the existing work and [`TITLE_HINT`] (`REFUSED`). Also reads the images the text
/// names ([`read_assets`]).
pub async fn import_file_as(state: &AppState, path: &Path, title: Option<&str>) -> Result<ImportReport, RpcError> {
    // Only the command line imports a file from a path; its messages stay German (Round 39a).
    crate::i18n::scope(crate::i18n::Lang::De, import_file_as_german(state, path, title)).await
}

async fn import_file_as_german(state: &AppState, path: &Path, title: Option<&str>) -> Result<ImportReport, RpcError> {
    let owned: PathBuf = path.to_owned();
    let (source, file_name, assets, skipped) = tokio::task::spawn_blocking(crate::i18n::carry(move || {
        let (source, file_name) = read_file(&owned)?;
        let (assets, skipped) = read_assets(&owned, &source);
        Ok::<_, RpcError>((source, file_name, assets, skipped))
    }))
    .await
    .map_err(|e| {
        RpcError::new(
            INTERNAL,
            tc(
                "markdown_msg.typst.read_aborted",
                &[("path", path.display().to_string().into()), ("detail", e.to_string().into())],
            ),
        )
    })??;
    import_source(state, &source, &file_name, title, assets, skipped, Vec::new()).await.map_err(map_title_error)
}

/// Create ready-made Typst text as a new work (PDF/EPUB import after confirmation, R21a). Like import_source,
/// taken title → REFUSED with TITLE_HINT, no heading → INVALID_PARAMS.
pub async fn import_generated(
    state: &AppState,
    source: &str,
    file_name: &str,
    title: Option<&str>,
    assets: Vec<ImportAsset>,
    skipped: Vec<SkippedAsset>,
    warnings: Vec<ImportWarning>,
) -> Result<ImportReport, RpcError> {
    import_source(state, source, file_name, title, assets, skipped, warnings).await.map_err(map_title_error)
}

/// Error mapping of the imports with their own title: a taken title names the existing work and [`TITLE_HINT`]
/// (`REFUSED`); everything else like `From<StoreError>` (no heading → `INVALID_PARAMS`).
fn map_title_error(e: StoreError) -> RpcError {
    match e {
        StoreError::DuplicateTitle { .. } => RpcError::new(REFUSED, format!("{e} {}", title_hint())),
        e => e.into(),
    }
}

/// Content of a regular file as UTF-8 (at most 16 MiB) and its file name. Blocks; call only in
/// `spawn_blocking`.
fn read_file(path: &Path) -> Result<(String, String), RpcError> {
    let shown = path.display();
    let unreadable = |e: std::io::Error| {
        RpcError::new(
            INVALID_PARAMS,
            tc("markdown_msg.file.unreadable", &[("file", shown.to_string().into()), ("detail", e.to_string().into())]),
        )
    };
    if !std::fs::metadata(path).map_err(unreadable)?.is_file() {
        return Err(RpcError::new(
            INVALID_PARAMS,
            tc("markdown_msg.file.not_file", &[("file", shown.to_string().into())]),
        ));
    }
    let file = std::fs::File::open(path).map_err(unreadable)?;
    let mut bytes = Vec::new();
    file.take(MAX_SOURCE_BYTES + 1).read_to_end(&mut bytes).map_err(unreadable)?;
    if bytes.len() as u64 > MAX_SOURCE_BYTES {
        return Err(RpcError::new(
            INVALID_PARAMS,
            tc("markdown_msg.file.too_large", &[("file", shown.to_string().into())]),
        ));
    }
    let source = String::from_utf8(bytes).map_err(|_| {
        RpcError::new(INVALID_PARAMS, tc("markdown_msg.file.not_utf8", &[("file", shown.to_string().into())]))
    })?;
    let file_name = path.file_name().and_then(|n| n.to_str()).ok_or_else(|| {
        RpcError::new(INVALID_PARAMS, tc("markdown_msg.file.no_name", &[("file", shown.to_string().into())]))
    })?;
    Ok((source, file_name.to_owned()))
}

/// Read the images that `source` names through `image("…")`, relative to the folder of the source file `typ`.
/// Paths with a leading `/` also count as relative to this folder (it is the project root).
/// After `canonicalize` the file must lie in the canonical folder (no symlink and no `..`
/// out of it), be a regular file, at most [`MAX_ASSET_BYTES`] large and a supported image;
/// all together at most [`MAX_IMPORT_ASSET_BYTES`]. Otherwise skipped with a reason. Blocks; call only
/// in `spawn_blocking`.
fn read_assets(typ: &Path, source: &str) -> (Vec<ImportAsset>, Vec<SkippedAsset>) {
    let folder = typ.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
    read_assets_in(folder, source)
}

/// Like [`read_assets`], with the folder of the source file instead of its path (Markdown import from the command
/// line, Round 21b: `source` is then the converted Typst text). Blocks; call only in `spawn_blocking`.
pub(crate) fn read_assets_in(folder: &Path, source: &str) -> (Vec<ImportAsset>, Vec<SkippedAsset>) {
    let mut assets = Vec::new();
    let mut skipped = Vec::new();
    let paths = baruch_typst::images::image_paths(source);
    if paths.is_empty() {
        return (assets, skipped);
    }
    let root = match std::fs::canonicalize(folder) {
        Ok(root) => root,
        Err(e) => {
            let reason = tc("markdown_msg.asset.folder", &[("detail", e.to_string().into())]);
            skipped.extend(paths.into_iter().map(|path| SkippedAsset { path, reason: reason.clone() }));
            return (assets, skipped);
        }
    };
    let mut total: u64 = 0;
    for path in paths {
        match read_asset(&root, &path, &mut total) {
            Ok((file_name, bytes)) => assets.push(ImportAsset { path, file_name, bytes }),
            Err(reason) => skipped.push(SkippedAsset { path, reason }),
        }
    }
    (assets, skipped)
}

/// Read one image file below `root`; errors as a reason (interface language).
fn read_asset(root: &Path, path: &str, total: &mut u64) -> Result<(String, Vec<u8>), String> {
    let relative = path.trim().trim_start_matches('/');
    if relative.is_empty() || relative.contains('\\') || relative.contains("://") {
        return Err(tc("markdown_msg.asset.not_path", &[]));
    }
    let real = std::fs::canonicalize(root.join(relative))
        .map_err(|e| tc("markdown_msg.asset.not_found", &[("detail", e.to_string().into())]))?;
    if !real.starts_with(root) {
        return Err(tc("markdown_msg.asset.outside", &[]));
    }
    // Before reading, the path rule of the store (`..`, control characters …): otherwise the import would read the
    // file, and the store would refuse the path and discard the whole import including the text. `./bilder/a.png`,
    // `bilder//a.png` and `/bilder/a.png` are allowed and count as `bilder/a.png`
    // ([`baruch_store::normalize_asset_path`], Round 7).
    if let Err(e) = baruch_store::check_asset_path(path) {
        return Err(e.to_string());
    }
    let unreadable = |e: std::io::Error| tc("markdown_msg.asset.unreadable", &[("detail", e.to_string().into())]);
    let meta = std::fs::metadata(&real).map_err(unreadable)?;
    if !meta.is_file() {
        return Err(tc("markdown_msg.asset.not_file", &[]));
    }
    if meta.len() > MAX_ASSET_BYTES as u64 {
        return Err(tc("markdown_msg.asset.too_large", &[]));
    }
    if *total + meta.len() > MAX_IMPORT_ASSET_BYTES {
        return Err(tc("markdown_msg.asset.total", &[]));
    }
    let file = std::fs::File::open(&real).map_err(unreadable)?;
    let mut bytes = Vec::new();
    file.take(MAX_ASSET_BYTES as u64 + 1).read_to_end(&mut bytes).map_err(unreadable)?;
    if bytes.len() > MAX_ASSET_BYTES {
        return Err(tc("markdown_msg.asset.too_large", &[]));
    }
    if baruch_store::detect_mime(&bytes).is_none() {
        return Err(tc("markdown_msg.asset.not_image", &[]));
    }
    *total += bytes.len() as u64;
    let file_name = real.file_name().and_then(|n| n.to_str()).unwrap_or("bild").to_owned();
    Ok((file_name, bytes))
}

/// Check image bytes from an archive or folder upload (Round 21b), with the same reasons as [`read_asset`]:
/// at most [`MAX_ASSET_BYTES`], all together at most [`MAX_IMPORT_ASSET_BYTES`], a supported image. Counts
/// `total` up only on success.
pub(crate) fn check_image(bytes: &[u8], total: &mut u64) -> Result<(), String> {
    if bytes.len() > MAX_ASSET_BYTES {
        return Err(tc("markdown_msg.asset.too_large", &[]));
    }
    if *total + bytes.len() as u64 > MAX_IMPORT_ASSET_BYTES {
        return Err(tc("markdown_msg.asset.total", &[]));
    }
    if baruch_store::detect_mime(bytes).is_none() {
        return Err(tc("markdown_msg.asset.not_image", &[]));
    }
    *total += bytes.len() as u64;
    Ok(())
}

/// A relative path made of names only (no `..`, `.`, no root, no drive)?
fn plain_relative(path: &str) -> bool {
    !path.is_empty() && Path::new(path).components().all(|c| matches!(c, Component::Normal(_)))
}

/// Write work and images into the folder `out` (`baruch-server export … --out`): the Typst file
/// as `<source_name>` (only the file name; without an import name `<slug>.typ`), text like
/// [`Store::export_typst`], and every file from [`Store::export_assets`] at its path. Only relative
/// paths without `..`; missing folders are created. If a target file already exists, nothing is
/// written (error). Result: the files written.
///
/// Only the command line calls it (`export --out`); like the imports of the command line it runs in German (Round 39a,
/// 39c), because `main.rs` prints its own lines in German.
pub async fn export_to_dir(store: &Store, work: WorkId, out: &Path) -> Result<Vec<PathBuf>, String> {
    crate::i18n::scope(crate::i18n::Lang::De, export_to_dir_in_scope(store, work, out)).await
}

async fn export_to_dir_in_scope(store: &Store, work: WorkId, out: &Path) -> Result<Vec<PathBuf>, String> {
    let meta = store.work_meta(work).await.map_err(|e| e.to_string())?;
    let typst = store.export_typst(work).await.map_err(|e| e.to_string())?;
    let assets = store.export_assets(work).await.map_err(|e| e.to_string())?;
    let typ_name = meta
        .source_name
        .as_deref()
        .and_then(|n| n.rsplit(['/', '\\']).next())
        .filter(|n| plain_relative(n) && is_typst_name(n))
        .map_or_else(|| format!("{}.typ", meta.slug), str::to_owned);
    let mut files: Vec<(String, Vec<u8>)> = vec![(typ_name, typst.into_bytes())];
    for file in assets {
        if !plain_relative(&file.path) {
            return Err(tc("markdown_msg.export.asset_path", &[("path", file.path.as_str().into())]));
        }
        // Normal form of the components (`bilder//a.png` = `bilder/a.png`), so that duplicates and collisions show up.
        let path = components(&file.path).join("/");
        if files.iter().any(|(p, _)| *p == path) {
            return Err(tc("markdown_msg.export.twice", &[("path", file.path.as_str().into())]));
        }
        files.push((path, file.bytes));
    }
    let out = out.to_owned();
    tokio::task::spawn_blocking(crate::i18n::carry(move || write_all(&out, &files)))
        .await
        .map_err(|e| tc("markdown_msg.export.aborted", &[("detail", e.to_string().into())]))?
}

/// Names of a relative path (only `Component::Normal`).
fn components(path: &str) -> Vec<String> {
    Path::new(path)
        .components()
        .filter_map(|c| match c {
            Component::Normal(name) => Some(name.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect()
}

/// Check everything before the first file is created (Round 21b): `out` is (if present) a folder; no target file
/// exists yet; no path is at once a file and a folder of another one (`a.png` and `a.png/b.svg`); no intermediate
/// folder that already exists is a link or a file (otherwise images would land outside of `out`). Blocks.
fn check_targets(out: &Path, files: &[(String, Vec<u8>)]) -> Result<(), String> {
    match std::fs::symlink_metadata(out) {
        Ok(meta) if meta.file_type().is_symlink() => {
            if !std::fs::metadata(out).is_ok_and(|m| m.is_dir()) {
                return Err(at_path("markdown_msg.export.not_folder", out));
            }
        }
        Ok(meta) if !meta.is_dir() => return Err(at_path("markdown_msg.export.not_folder", out)),
        _ => {}
    }
    let paths: Vec<Vec<String>> = files.iter().map(|(p, _)| components(p)).collect();
    for (i, a) in paths.iter().enumerate() {
        for (j, b) in paths.iter().enumerate() {
            if i != j && b.len() > a.len() && b.starts_with(a) {
                return Err(tc(
                    "markdown_msg.export.file_and_folder",
                    &[("path", a.join("/").into()), ("other", b.join("/").into())],
                ));
            }
        }
    }
    for parts in &paths {
        let mut at = out.to_path_buf();
        for (k, part) in parts.iter().enumerate() {
            at.push(part);
            let last = k + 1 == parts.len();
            match std::fs::symlink_metadata(&at) {
                Err(_) => break, // missing: is created, nothing can exist below it
                Ok(_) if last => return Err(at_path("markdown_msg.export.exists", &at)),
                Ok(meta) if meta.file_type().is_symlink() => {
                    return Err(tc(
                        "markdown_msg.export.link",
                        &[("path", at.display().to_string().into()), ("out", out.display().to_string().into())],
                    ));
                }
                Ok(meta) if !meta.is_dir() => {
                    return Err(at_path("markdown_msg.export.is_file", &at));
                }
                Ok(_) => {}
            }
        }
    }
    Ok(())
}

/// Message of the export with a path (`{ $path }`).
fn at_path(key: &str, path: &Path) -> String {
    tc(key, &[("path", path.display().to_string().into())])
}

/// Message of the export with a path and the error of the operating system.
fn io_at(key: &str, path: &Path, e: &std::io::Error) -> String {
    tc(key, &[("path", path.display().to_string().into()), ("detail", e.to_string().into())])
}

/// Write all files; check everything first ([`check_targets`]), then overwrite nothing and locate every folder
/// below `out` once more after creating it (`canonicalize`). Blocks.
fn write_all(out: &Path, files: &[(String, Vec<u8>)]) -> Result<Vec<PathBuf>, String> {
    check_targets(out, files)?;
    std::fs::create_dir_all(out).map_err(|e| io_at("markdown_msg.export.create", out, &e))?;
    let root = std::fs::canonicalize(out).map_err(|e| io_at("markdown_msg.export.read", out, &e))?;
    let mut written = Vec::with_capacity(files.len());
    for (path, bytes) in files {
        let target = out.join(path);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|e| io_at("markdown_msg.export.create", parent, &e))?;
            let real = std::fs::canonicalize(parent).map_err(|e| io_at("markdown_msg.export.read", parent, &e))?;
            if !real.starts_with(&root) {
                return Err(tc(
                    "markdown_msg.export.outside",
                    &[("path", parent.display().to_string().into()), ("out", out.display().to_string().into())],
                ));
            }
        }
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&target)
            .map_err(|e| io_at("markdown_msg.export.write", &target, &e))?;
        file.write_all(bytes).map_err(|e| io_at("markdown_msg.export.write", &target, &e))?;
        written.push(target);
    }
    Ok(written)
}
