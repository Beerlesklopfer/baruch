// SPDX-License-Identifier: AGPL-3.0-or-later
//! Typst-Datei als neues Werk (Vertrag B1.5, Abschnitt 4; Runde 4): Import in die Datenbank
//! ([`baruch_store::Store::import_typst_with_assets`]), danach Git-Spiegel des neuen Werks mit dem
//! Commit „Import aus <datei>“. Aufgerufen von `baruch-server import [--title …]`, vom Hochladen über
//! `POST /werke/import` und RPC `work.import` ([`import_upload`], `crate::works`) und aus Tests.
//! Ein Titel, den schon ein anderes Werk trägt, wird abgelehnt (`DuplicateTitle`, -32000).
//!
//! Bilder (Vertrag Runde 6, Abschnitt 4d): Nur der Import von der Kommandozeile ([`import_file_as`])
//! liest die Bilder, die Vorlage und Text über `image("…")` nennen
//! ([`baruch_typst::images::image_paths`]), relativ zum Ordner der Quelldatei (auch Pfade mit
//! führendem `/` oder `./`, Normalform über [`baruch_store::normalize_asset_path`]). Eine Datei
//! außerhalb dieses Ordners (über einen Symlink oder `..`), ein Pfad, den der Store nicht speichert
//! (`..`, Adressen, [`baruch_store::check_asset_path`]), eine fehlende,
//! zu große oder keine Bilddatei wird mit Grund übersprungen ([`SkippedAsset`]); Werk und Bilder
//! entstehen in einer Transaktion. Hochladen einer einzelnen Datei und `work.import` (nur Text) legen keine Bilder
//! an; ZIP und Ordner (Runde 21b, [`crate::markdown_import`]) bringen sie mit ([`check_image`]).
//! [`export_to_dir`] schreibt Werk und Bilder wieder in einen Ordner (`export --out`) und prüft seit Runde 21b vor der
//! ersten Datei Verknüpfungen, Dateien als Zwischenordner und Datei/Ordner-Kollisionen (`check_targets`).
//!
//! Runde 21b: Jeder Eingriff in den Text eines Exports (Buchende geschnitten, Projektion entfernt, Regelblock in die
//! Satz-Einstellungen) ist eine [`ImportWarning`] im Bericht; Codes `import-…`.
//!
//! Über MCP gibt es keinen Import (Entscheidung 2026-11-05): Ein Import legt aktuelle Revisionen unter
//! dem Namen des Menschen an, und das darf ein Assistent nicht.

use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};

use baruch_store::{ImportAsset, MAX_ASSET_BYTES, Store, StoreError, WorkId};
use serde::Serialize;

use crate::rpc::{INTERNAL, INVALID_PARAMS, REFUSED, RpcError};
use crate::{AppState, mirror};

/// Größte Datei, die der Import annimmt.
pub const MAX_SOURCE_BYTES: u64 = 16 * 1024 * 1024;

/// Höchstens so viele Bytes Bilder liest ein Import je Quelldatei.
pub const MAX_IMPORT_ASSET_BYTES: u64 = 64 * 1024 * 1024;

/// Ergebnis eines Imports.
#[derive(Debug, Clone, Serialize)]
pub struct ImportReport {
    pub work: WorkId,
    pub slug: String,
    pub title: String,
    /// Anzahl der Abschnitte (Kapitel, Abschnitte, Unterabschnitte).
    pub nodes: usize,
    /// Commit-Id im Spiegel des Werks.
    pub commit: Option<String>,
    /// Fehlertext, falls der Spiegel scheiterte (das Werk ist trotzdem angelegt).
    pub mirror: Option<String>,
    /// Angelegte Bilder (Namen); nur der Import von der Kommandozeile liest Bilder.
    pub assets: Vec<String>,
    /// Übersprungene Bildpfade mit Grund.
    pub skipped: Vec<SkippedAsset>,
    /// Hinweise des Imports (Umwandlung, Schnitt, Grenze; Runde 21): was nicht wörtlich übernommen wurde.
    pub warnings: Vec<ImportWarning>,
}

/// Hinweis eines Imports (Umwandlung, Schnitt, Grenze); nie stilles Verwerfen.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ImportWarning {
    pub code: String,
    pub message: String,
    pub line: Option<usize>,
}

/// Ein Bildpfad aus dem Text, der beim Import nicht gelesen wurde.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SkippedAsset {
    pub path: String,
    pub reason: String,
}

/// Hinweis der CLI, wenn der Titel schon vergeben ist.
pub const TITLE_HINT: &str = "Mit --title \"…\" einen anderen Titel wählen.";

/// Quelltext importieren. Schreibsperre wie bei Übernehmen und Speichern, damit der Commit genau
/// den neuen Stand enthält. Schon importiert oder Titel vergeben → `REFUSED`, keine Überschrift →
/// `INVALID_PARAMS`. Notiz jeder Revision und Betreff des Commits: „Import aus <datei>“.
pub async fn import_typst(state: &AppState, source: &str, file_name: &str) -> Result<ImportReport, RpcError> {
    Ok(import_source(state, source, file_name, None, Vec::new(), Vec::new(), Vec::new()).await?)
}

/// Hochgeladene Datei (Formular `POST /werke/import`, RPC `work.import`): nur Dateinamen auf `.typ`
/// (Groß-/Kleinschreibung egal), höchstens 16 MiB, sonst `INVALID_PARAMS`; `title` (leer = aus der
/// Datei) wie bei der CLI mit `--title`. Ordner im Dateinamen fallen weg. Bilder kommen so nicht mit
/// (nur Text); sie lassen sich danach auf der Seite „Bilder“ mit ihrem Pfad anlegen.
pub async fn import_upload(
    state: &AppState,
    source: &str,
    file_name: &str,
    title: Option<&str>,
) -> Result<ImportReport, RpcError> {
    let file_name = file_name.rsplit(['/', '\\']).next().unwrap_or("").trim();
    if !is_typst_name(file_name) {
        return Err(RpcError::new(INVALID_PARAMS, "Nur Typst-Dateien (.typ) lassen sich importieren."));
    }
    if source.len() as u64 > MAX_SOURCE_BYTES {
        return Err(RpcError::new(INVALID_PARAMS, format!("{file_name} ist größer als 16 MiB")));
    }
    Ok(import_source(state, source, file_name, title, Vec::new(), Vec::new(), Vec::new()).await?)
}

/// Endet der Dateiname auf `.typ` (ohne Rücksicht auf Groß-/Kleinschreibung) und hat davor einen Namen?
pub fn is_typst_name(file_name: &str) -> bool {
    let lower = file_name.to_lowercase();
    lower.len() > 4 && lower.ends_with(".typ")
}

/// Gemeinsamer Weg aller Importe; Fehler als `StoreError`, damit die CLI einen vergebenen Titel
/// erkennt. Werk und Bilder in einer Transaktion.
async fn import_source(
    state: &AppState,
    source: &str,
    file_name: &str,
    title: Option<&str>,
    assets: Vec<ImportAsset>,
    skipped: Vec<SkippedAsset>,
    warnings: Vec<ImportWarning>,
) -> Result<ImportReport, StoreError> {
    // Runde 31: Das Buchende eines Exports (Register- und Glossarseiten ab `// Baruch: Register und Glossar`) gehört zu
    // keinem Abschnitt; es entsteht beim Export aus den Satz-Einstellungen neu und wandert nicht in den letzten Abschnitt.
    // Ein solcher Export trägt auch die Zusätze der Projektion (automatische Bibelstellen-Marken, verlinkte erste
    // Erwähnung); sie kommen heraus, sonst verwiese der Text auf Glossar-Labels, die es im neuen Werk nicht gibt.
    // Runde 21b: geschnitten wird nur ein erzeugtes Buchende, und jeder Eingriff steht im Bericht.
    let mut warnings = warnings;
    let original = source;
    let (source, exported) = baruch_typst::index::strip_back_matter(source);
    if exported {
        warnings.push(ImportWarning {
            code: "import-back-matter-cut".into(),
            message: "Das erzeugte Buchende eines Exports (Register- und Glossarseiten ab „// Baruch: Register und \
                      Glossar“) ist abgeschnitten; es entsteht beim Export aus den Satz-Einstellungen neu."
                .into(),
            line: Some(baruch_typst::line_at(original, source.len())),
        });
    } else if baruch_typst::index::has_back_matter_mark(source) {
        warnings.push(ImportWarning {
            code: "import-back-matter-kept".into(),
            message: "Die Zeile „// Baruch: Register und Glossar“ steht im Text, aber ohne das erzeugte Buchende \
                      dahinter; nichts abgeschnitten."
                .into(),
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
            message: "Zusätze der Projektion eines Exports (automatische Bibelstellen-Marken, Auszeichnung der ersten \
                      Erwähnung) sind entfernt; sie entstehen beim Export aus den Einstellungen neu."
                .into(),
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
    // Runde 21b: Der Regelblock „// Baruch: Satz“ eines Exports landet in den Satz-Einstellungen (Store), sonst bleibt er
    // in der Werkvorlage – beides steht im Bericht.
    if state.store.work_typeset(imported.work).await.ok().flatten().is_some() {
        warnings.push(ImportWarning {
            code: "import-typeset-adopted".into(),
            message: "Der Regelblock „// Baruch: Satz“ ist in die Satz-Einstellungen des Werks übernommen \
                      (Einstellungen → Werk → Satz), nicht in die Werkvorlage."
                .into(),
            line: None,
        });
    } else if let Ok(template) = state.store.work_template(imported.work).await
        && template.lines().any(|l| l.trim_end() == baruch_typst::typeset::RULES_MARK)
    {
        warnings.push(ImportWarning {
            code: "import-typeset-kept".into(),
            message:
                "Der Regelblock „// Baruch: Satz“ ließ sich nicht genau in Satz-Einstellungen zurücklesen (von Hand \
                      geändert?) und steht weiter in der Werkvorlage."
                    .into(),
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

/// Datei lesen (UTF-8, höchstens 16 MiB) und importieren; der Dateiname kommt aus `path`.
/// Für die CLI: Fehler nennen den Grund des Betriebssystems.
///
/// Vor dem Öffnen prüft `std::fs::metadata`, ob `path` eine reguläre Datei ist (auch nach Auflösen
/// einer Verknüpfung). Eine FIFO, ein Gerät oder ein Ordner wird so abgelehnt, ohne geöffnet zu
/// werden: `File::open` auf eine FIFO ohne Schreiber würde sonst blockieren. Das Lesen selbst läuft in
/// `spawn_blocking`, damit kein Tokio-Worker auf das Dateisystem wartet.
pub async fn import_file(state: &AppState, path: &Path) -> Result<ImportReport, RpcError> {
    import_file_as(state, path, None).await
}

/// Wie [`import_file`], mit eigenem Titel (`--title`). Ist der Titel schon vergeben, nennt die
/// Meldung das vorhandene Werk und [`TITLE_HINT`] (`REFUSED`). Liest auch die Bilder, die der Text
/// nennt ([`read_assets`]).
pub async fn import_file_as(state: &AppState, path: &Path, title: Option<&str>) -> Result<ImportReport, RpcError> {
    let owned: PathBuf = path.to_owned();
    let (source, file_name, assets, skipped) = tokio::task::spawn_blocking(move || {
        let (source, file_name) = read_file(&owned)?;
        let (assets, skipped) = read_assets(&owned, &source);
        Ok::<_, RpcError>((source, file_name, assets, skipped))
    })
    .await
    .map_err(|e| RpcError::new(INTERNAL, format!("Lesen von {} abgebrochen: {e}", path.display())))??;
    import_source(state, &source, &file_name, title, assets, skipped, Vec::new()).await.map_err(map_title_error)
}

/// Fertig erzeugten Typst-Text als neues Werk anlegen (PDF/EPUB-Import nach Bestätigung, R21a). Wie import_source,
/// vergebener Titel → REFUSED mit TITLE_HINT, keine Überschrift → INVALID_PARAMS.
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

/// Fehlerabbildung der Importe mit eigenem Titel: Ein vergebener Titel nennt das vorhandene Werk und [`TITLE_HINT`]
/// (`REFUSED`); alles andere wie `From<StoreError>` (keine Überschrift → `INVALID_PARAMS`).
fn map_title_error(e: StoreError) -> RpcError {
    match e {
        StoreError::DuplicateTitle { .. } => RpcError::new(REFUSED, format!("{e} {TITLE_HINT}")),
        e => e.into(),
    }
}

/// Inhalt einer regulären Datei als UTF-8 (höchstens 16 MiB) und ihr Dateiname. Blockiert; nur in
/// `spawn_blocking` aufrufen.
fn read_file(path: &Path) -> Result<(String, String), RpcError> {
    let shown = path.display();
    let unreadable = |e: std::io::Error| RpcError::new(INVALID_PARAMS, format!("{shown} lässt sich nicht lesen: {e}"));
    if !std::fs::metadata(path).map_err(unreadable)?.is_file() {
        return Err(RpcError::new(INVALID_PARAMS, format!("{shown} ist keine Datei")));
    }
    let file = std::fs::File::open(path).map_err(unreadable)?;
    let mut bytes = Vec::new();
    file.take(MAX_SOURCE_BYTES + 1).read_to_end(&mut bytes).map_err(unreadable)?;
    if bytes.len() as u64 > MAX_SOURCE_BYTES {
        return Err(RpcError::new(INVALID_PARAMS, format!("{shown} ist größer als 16 MiB")));
    }
    let source =
        String::from_utf8(bytes).map_err(|_| RpcError::new(INVALID_PARAMS, format!("{shown} ist kein UTF-8-Text")))?;
    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| RpcError::new(INVALID_PARAMS, format!("{shown}: Dateiname fehlt oder ist kein UTF-8")))?;
    Ok((source, file_name.to_owned()))
}

/// Bilder, die `source` über `image("…")` nennt, relativ zum Ordner der Quelldatei `typ` lesen.
/// Pfade mit führendem `/` gelten ebenfalls relativ zu diesem Ordner (er ist die Projektwurzel).
/// Nach `canonicalize` muss die Datei im kanonischen Ordner liegen (kein Symlink und kein `..`
/// hinaus), eine reguläre Datei sein, höchstens [`MAX_ASSET_BYTES`] groß und ein unterstütztes Bild;
/// alle zusammen höchstens [`MAX_IMPORT_ASSET_BYTES`]. Sonst übersprungen mit Grund. Blockiert; nur
/// in `spawn_blocking` aufrufen.
fn read_assets(typ: &Path, source: &str) -> (Vec<ImportAsset>, Vec<SkippedAsset>) {
    let folder = typ.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
    read_assets_in(folder, source)
}

/// Wie [`read_assets`], mit dem Ordner der Quelldatei statt ihres Pfads (Markdown-Import von der Kommandozeile,
/// Runde 21b: `source` ist dann der umgewandelte Typst-Text). Blockiert; nur in `spawn_blocking` aufrufen.
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
            let reason = format!("Ordner der Quelldatei nicht lesbar ({e})");
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

/// Eine Bilddatei unter `root` lesen; Fehler als Grund (Deutsch).
fn read_asset(root: &Path, path: &str, total: &mut u64) -> Result<(String, Vec<u8>), String> {
    let relative = path.trim().trim_start_matches('/');
    if relative.is_empty() || relative.contains('\\') || relative.contains("://") {
        return Err("kein Dateipfad".to_owned());
    }
    let real = std::fs::canonicalize(root.join(relative)).map_err(|e| format!("nicht gefunden ({e})"))?;
    if !real.starts_with(root) {
        return Err("liegt außerhalb des Ordners der Quelldatei".to_owned());
    }
    // Vor dem Lesen die Pfadregel des Stores (`..`, Steuerzeichen …): Sonst läse der Import die Datei,
    // und der Store lehnte den Pfad ab und verwürfe den ganzen Import samt Text. `./bilder/a.png`,
    // `bilder//a.png` und `/bilder/a.png` sind erlaubt und gelten als `bilder/a.png`
    // ([`baruch_store::normalize_asset_path`], Runde 7).
    if let Err(e) = baruch_store::check_asset_path(path) {
        return Err(e.to_string());
    }
    let meta = std::fs::metadata(&real).map_err(|e| format!("nicht lesbar ({e})"))?;
    if !meta.is_file() {
        return Err("keine Datei".to_owned());
    }
    if meta.len() > MAX_ASSET_BYTES as u64 {
        return Err("größer als 8 MiB".to_owned());
    }
    if *total + meta.len() > MAX_IMPORT_ASSET_BYTES {
        return Err("die Bilder dieser Datei sind zusammen größer als 64 MiB".to_owned());
    }
    let file = std::fs::File::open(&real).map_err(|e| format!("nicht lesbar ({e})"))?;
    let mut bytes = Vec::new();
    file.take(MAX_ASSET_BYTES as u64 + 1).read_to_end(&mut bytes).map_err(|e| format!("nicht lesbar ({e})"))?;
    if bytes.len() > MAX_ASSET_BYTES {
        return Err("größer als 8 MiB".to_owned());
    }
    if baruch_store::detect_mime(&bytes).is_none() {
        return Err("kein unterstütztes Bild (PNG, JPEG, GIF, WebP, SVG)".to_owned());
    }
    *total += bytes.len() as u64;
    let file_name = real.file_name().and_then(|n| n.to_str()).unwrap_or("bild").to_owned();
    Ok((file_name, bytes))
}

/// Bildbytes aus einem Archiv oder Ordner-Upload prüfen (Runde 21b), mit denselben Gründen wie [`read_asset`]:
/// höchstens [`MAX_ASSET_BYTES`], alle zusammen höchstens [`MAX_IMPORT_ASSET_BYTES`], ein unterstütztes Bild. Zählt
/// `total` nur bei Erfolg hoch.
pub(crate) fn check_image(bytes: &[u8], total: &mut u64) -> Result<(), String> {
    if bytes.len() > MAX_ASSET_BYTES {
        return Err("größer als 8 MiB".to_owned());
    }
    if *total + bytes.len() as u64 > MAX_IMPORT_ASSET_BYTES {
        return Err("die Bilder dieser Datei sind zusammen größer als 64 MiB".to_owned());
    }
    if baruch_store::detect_mime(bytes).is_none() {
        return Err("kein unterstütztes Bild (PNG, JPEG, GIF, WebP, SVG)".to_owned());
    }
    *total += bytes.len() as u64;
    Ok(())
}

/// Ein relativer Pfad nur aus Namen (kein `..`, `.`, keine Wurzel, kein Laufwerk)?
fn plain_relative(path: &str) -> bool {
    !path.is_empty() && Path::new(path).components().all(|c| matches!(c, Component::Normal(_)))
}

/// Werk und Bilder in den Ordner `out` schreiben (`baruch-server export … --out`): die Typst-Datei
/// als `<source_name>` (nur der Dateiname; ohne Importnamen `<slug>.typ`), Text wie
/// [`Store::export_typst`], und jede Datei aus [`Store::export_assets`] an ihren Pfad. Nur relative
/// Pfade ohne `..`; fehlende Ordner werden angelegt. Gibt es eine Zieldatei schon, wird nichts
/// geschrieben (Fehler). Ergebnis: die geschriebenen Dateien.
pub async fn export_to_dir(store: &Store, work: WorkId, out: &Path) -> Result<Vec<PathBuf>, String> {
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
            return Err(format!("Bildpfad {} ist kein relativer Pfad ohne ..; nichts geschrieben", file.path));
        }
        // Normalform der Komponenten (`bilder//a.png` = `bilder/a.png`), damit Doppel und Kollisionen auffallen.
        let path = components(&file.path).join("/");
        if files.iter().any(|(p, _)| *p == path) {
            return Err(format!("{} käme zweimal vor; nichts geschrieben", file.path));
        }
        files.push((path, file.bytes));
    }
    let out = out.to_owned();
    tokio::task::spawn_blocking(move || write_all(&out, &files))
        .await
        .map_err(|e| format!("Schreiben abgebrochen: {e}"))?
}

/// Namen eines relativen Pfads (nur `Component::Normal`).
fn components(path: &str) -> Vec<String> {
    Path::new(path)
        .components()
        .filter_map(|c| match c {
            Component::Normal(name) => Some(name.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect()
}

/// Alles prüfen, bevor die erste Datei entsteht (Runde 21b): `out` ist (falls vorhanden) ein Ordner; keine Zieldatei gibt
/// es schon; kein Pfad ist zugleich Datei und Ordner eines anderen (`a.png` und `a.png/b.svg`); kein schon vorhandener
/// Zwischenordner ist eine Verknüpfung oder eine Datei (sonst landeten Bilder außerhalb von `out`). Blockiert.
fn check_targets(out: &Path, files: &[(String, Vec<u8>)]) -> Result<(), String> {
    match std::fs::symlink_metadata(out) {
        Ok(meta) if meta.file_type().is_symlink() => {
            if !std::fs::metadata(out).is_ok_and(|m| m.is_dir()) {
                return Err(format!("{} ist kein Ordner; nichts geschrieben", out.display()));
            }
        }
        Ok(meta) if !meta.is_dir() => return Err(format!("{} ist kein Ordner; nichts geschrieben", out.display())),
        _ => {}
    }
    let paths: Vec<Vec<String>> = files.iter().map(|(p, _)| components(p)).collect();
    for (i, a) in paths.iter().enumerate() {
        for (j, b) in paths.iter().enumerate() {
            if i != j && b.len() > a.len() && b.starts_with(a) {
                return Err(format!(
                    "{} wäre zugleich Datei und Ordner (für {}); nichts geschrieben",
                    a.join("/"),
                    b.join("/")
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
                Err(_) => break, // fehlt: wird angelegt, darunter kann nichts vorhanden sein
                Ok(_) if last => return Err(format!("{} gibt es schon; nichts überschrieben", at.display())),
                Ok(meta) if meta.file_type().is_symlink() => {
                    return Err(format!(
                        "{} ist eine Verknüpfung; Bilder könnten außerhalb von {} landen; nichts geschrieben",
                        at.display(),
                        out.display()
                    ));
                }
                Ok(meta) if !meta.is_dir() => {
                    return Err(format!("{} ist eine Datei, kein Ordner; nichts geschrieben", at.display()));
                }
                Ok(_) => {}
            }
        }
    }
    Ok(())
}

/// Alle Dateien schreiben; vorher alles prüfen ([`check_targets`]), dann nichts überschreiben und jeden Ordner nach dem
/// Anlegen noch einmal unter `out` verorten (`canonicalize`). Blockiert.
fn write_all(out: &Path, files: &[(String, Vec<u8>)]) -> Result<Vec<PathBuf>, String> {
    check_targets(out, files)?;
    std::fs::create_dir_all(out).map_err(|e| format!("{} lässt sich nicht anlegen: {e}", out.display()))?;
    let root = std::fs::canonicalize(out).map_err(|e| format!("{} lässt sich nicht lesen: {e}", out.display()))?;
    let mut written = Vec::with_capacity(files.len());
    for (path, bytes) in files {
        let target = out.join(path);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("{} lässt sich nicht anlegen: {e}", parent.display()))?;
            let real = std::fs::canonicalize(parent)
                .map_err(|e| format!("{} lässt sich nicht lesen: {e}", parent.display()))?;
            if !real.starts_with(&root) {
                return Err(format!("{} liegt außerhalb von {}; abgebrochen", parent.display(), out.display()));
            }
        }
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&target)
            .map_err(|e| format!("{} lässt sich nicht schreiben: {e}", target.display()))?;
        file.write_all(bytes).map_err(|e| format!("{} lässt sich nicht schreiben: {e}", target.display()))?;
        written.push(target);
    }
    Ok(written)
}
