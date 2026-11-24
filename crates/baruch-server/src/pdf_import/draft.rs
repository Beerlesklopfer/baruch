// SPDX-License-Identifier: AGPL-3.0-or-later
//! Entwürfe eines PDF/EPUB-Imports (Runde 21a) als Dateien unter `<data>/import-drafts/<draft>/` (keine Migration):
//! `draft.json` (Vorschau samt Text der Abschnitte) und `asset-<n>.bin` (Bilder) bzw. – wenn MultiGPT scheiterte und
//! der Mensch lokal lesen kann, ohne die Datei erneut zu wählen – `pending.json` und `source.bin`.
//!
//! `draft` = 32 Kleinbuchstaben-Hex (zufällig, [`baruch_store::new_uuid`] ohne Bindestriche); jeder andere Name wird
//! abgelehnt (kein Pfad aus Eingaben). Höchstens [`MAX_DRAFTS`] Entwürfe und [`MAX_AGE`]: Beim nächsten `preview`
//! fallen ältere Entwürfe weg, danach die ältesten, bis Platz für einen neuen ist.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use super::remote::RemoteDoc;
use super::typst_out::Section;
use crate::import::{ImportWarning, SkippedAsset};

/// Höchstens so viele Entwürfe liegen gleichzeitig vor.
pub const MAX_DRAFTS: usize = 20;
/// So lange bleibt ein Entwurf.
pub const MAX_AGE: Duration = Duration::from_secs(24 * 60 * 60);
/// Ordner der Entwürfe im Datenordner.
pub const DIR: &str = "import-drafts";

/// Hinweis (wie [`ImportWarning`], speicherbar).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredWarning {
    pub code: String,
    pub message: String,
    pub line: Option<usize>,
}

impl From<&ImportWarning> for StoredWarning {
    fn from(w: &ImportWarning) -> Self {
        StoredWarning { code: w.code.clone(), message: w.message.clone(), line: w.line }
    }
}

impl From<&StoredWarning> for ImportWarning {
    fn from(w: &StoredWarning) -> Self {
        ImportWarning { code: w.code.clone(), message: w.message.clone(), line: w.line }
    }
}

/// Übersprungenes Bild (wie [`SkippedAsset`], speicherbar).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredSkipped {
    pub path: String,
    pub reason: String,
}

/// Bild eines Entwurfs (Bytes in `asset-<n>.bin`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredAsset {
    pub path: String,
    pub file_name: String,
}

/// Ein fertig gelesener Entwurf.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Draft {
    pub id: String,
    /// Sekunden seit 1970 (UTC).
    pub created: u64,
    pub file_name: String,
    /// `pdf` oder `epub`.
    pub kind: String,
    /// `multigpt` oder `local`.
    pub way: String,
    pub way_reason: String,
    pub title: Option<String>,
    pub author: Option<String>,
    pub lang: Option<String>,
    pub pages: Option<usize>,
    pub sections: Vec<Section>,
    pub warnings: Vec<StoredWarning>,
    pub skipped: Vec<StoredSkipped>,
    pub assets: Vec<StoredAsset>,
    pub document: Option<RemoteDoc>,
    pub sha256: String,
    pub mime: String,
    /// Volltext für die Bibliothek (nur lokaler Weg; über MultiGPT gilt die Verknüpfung).
    pub plain_text: String,
    /// Vorgeschlagener Titel aus dem Formular bzw. RPC (`title`), falls angegeben.
    pub wanted_title: Option<String>,
}

impl Draft {
    pub fn warnings(&self) -> Vec<ImportWarning> {
        self.warnings.iter().map(ImportWarning::from).collect()
    }

    pub fn skipped(&self) -> Vec<SkippedAsset> {
        self.skipped.iter().map(|s| SkippedAsset { path: s.path.clone(), reason: s.reason.clone() }).collect()
    }
}

/// Datei, deren Lesen über MultiGPT scheiterte; bereit für „lokal lesen“.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pending {
    pub id: String,
    pub created: u64,
    pub file_name: String,
    pub title: Option<String>,
    pub collection: Option<String>,
}

pub fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// Ist `id` ein gültiger Name eines Entwurfs (32 Kleinbuchstaben-Hex)?
pub fn valid_id(id: &str) -> bool {
    id.len() == 32 && id.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

pub fn new_id() -> String {
    baruch_store::new_uuid().replace('-', "")
}

pub fn root(data: &Path) -> PathBuf {
    data.join(DIR)
}

fn folder(data: &Path, id: &str) -> Option<PathBuf> {
    valid_id(id).then(|| root(data).join(id))
}

fn created_of(dir: &Path) -> u64 {
    for name in ["draft.json", "pending.json"] {
        if let Ok(text) = std::fs::read_to_string(dir.join(name))
            && let Ok(value) = serde_json::from_str::<serde_json::Value>(&text)
            && let Some(created) = value.get("created").and_then(serde_json::Value::as_u64)
        {
            return created;
        }
    }
    std::fs::metadata(dir)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_secs())
}

/// Aufräumen vor einem neuen Entwurf: älter als [`MAX_AGE`] weg, dann die ältesten, bis höchstens
/// `MAX_DRAFTS - 1` bleiben. Blockiert. Ergebnis: Zahl der entfernten Entwürfe.
pub fn clean(data: &Path) -> usize {
    let Ok(entries) = std::fs::read_dir(root(data)) else { return 0 };
    let mut drafts: Vec<(u64, PathBuf)> = entries
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .filter(|e| e.file_name().to_str().is_some_and(valid_id))
        .map(|e| (created_of(&e.path()), e.path()))
        .collect();
    drafts.sort();
    let limit = now().saturating_sub(MAX_AGE.as_secs());
    let mut removed = 0;
    let mut kept = Vec::new();
    for (created, path) in drafts {
        if created < limit {
            if std::fs::remove_dir_all(&path).is_ok() {
                removed += 1;
            }
        } else {
            kept.push(path);
        }
    }
    while kept.len() >= MAX_DRAFTS {
        let path = kept.remove(0);
        if std::fs::remove_dir_all(&path).is_ok() {
            removed += 1;
        }
    }
    removed
}

fn write_json(dir: &Path, name: &str, value: &impl Serialize) -> Result<(), String> {
    let text = serde_json::to_vec(value).map_err(|e| format!("Entwurf: {e}"))?;
    std::fs::write(dir.join(name), text).map_err(|e| format!("Entwurf lässt sich nicht schreiben: {e}"))
}

/// Neuen Entwurf schreiben (Bilder als Dateien). Blockiert.
pub fn save(data: &Path, draft: &Draft, assets: &[(String, String, Vec<u8>)]) -> Result<(), String> {
    let dir = folder(data, &draft.id).ok_or("ungültiger Entwurf")?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("Entwurf lässt sich nicht anlegen: {e}"))?;
    for (n, (_, _, bytes)) in assets.iter().enumerate() {
        std::fs::write(dir.join(format!("asset-{n}.bin")), bytes)
            .map_err(|e| format!("Bild des Entwurfs lässt sich nicht schreiben: {e}"))?;
    }
    write_json(&dir, "draft.json", draft)
}

/// Datei für „lokal lesen“ nach einem Fehler von MultiGPT ablegen. Blockiert.
pub fn save_pending(data: &Path, pending: &Pending, bytes: &[u8]) -> Result<(), String> {
    let dir = folder(data, &pending.id).ok_or("ungültiger Entwurf")?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("Entwurf lässt sich nicht anlegen: {e}"))?;
    std::fs::write(dir.join("source.bin"), bytes).map_err(|e| format!("Entwurf lässt sich nicht schreiben: {e}"))?;
    write_json(&dir, "pending.json", pending)
}

/// Fehler beim Laden eines Entwurfs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadError {
    /// Ungültiger Name oder kein solcher (bzw. abgelaufener) Entwurf.
    Unknown,
    /// Der Entwurf wartet auf „lokal lesen“ (keine Vorschau).
    Pending,
    Broken(String),
}

/// Entwurf laden (ohne Bilder). Abgelaufene gelten als unbekannt. Blockiert.
pub fn load(data: &Path, id: &str) -> Result<Draft, LoadError> {
    let dir = folder(data, id).ok_or(LoadError::Unknown)?;
    let text = match std::fs::read_to_string(dir.join("draft.json")) {
        Ok(text) => text,
        Err(_) if dir.join("pending.json").is_file() => return Err(LoadError::Pending),
        Err(_) => return Err(LoadError::Unknown),
    };
    let draft: Draft = serde_json::from_str(&text).map_err(|e| LoadError::Broken(format!("Entwurf unlesbar: {e}")))?;
    if draft.created < now().saturating_sub(MAX_AGE.as_secs()) {
        return Err(LoadError::Unknown);
    }
    Ok(draft)
}

/// Bilder eines Entwurfs. Blockiert.
pub fn load_assets(data: &Path, draft: &Draft) -> Result<Vec<(String, String, Vec<u8>)>, String> {
    let dir = folder(data, &draft.id).ok_or("ungültiger Entwurf")?;
    draft
        .assets
        .iter()
        .enumerate()
        .map(|(n, a)| {
            std::fs::read(dir.join(format!("asset-{n}.bin")))
                .map(|bytes| (a.path.clone(), a.file_name.clone(), bytes))
                .map_err(|e| format!("Bild {} des Entwurfs fehlt: {e}", a.path))
        })
        .collect()
}

/// Wartende Datei laden. Blockiert.
pub fn load_pending(data: &Path, id: &str) -> Result<(Pending, Vec<u8>), LoadError> {
    let dir = folder(data, id).ok_or(LoadError::Unknown)?;
    let text = std::fs::read_to_string(dir.join("pending.json")).map_err(|_| LoadError::Unknown)?;
    let pending: Pending =
        serde_json::from_str(&text).map_err(|e| LoadError::Broken(format!("Entwurf unlesbar: {e}")))?;
    if pending.created < now().saturating_sub(MAX_AGE.as_secs()) {
        return Err(LoadError::Unknown);
    }
    let bytes = std::fs::read(dir.join("source.bin"))
        .map_err(|e| LoadError::Broken(format!("Datei des Entwurfs fehlt: {e}")))?;
    Ok((pending, bytes))
}

/// Entwurf löschen; `true`, wenn es ihn gab. Blockiert.
pub fn remove(data: &Path, id: &str) -> bool {
    match folder(data, id) {
        Some(dir) if dir.is_dir() => std::fs::remove_dir_all(dir).is_ok(),
        _ => false,
    }
}

/// Zahl der vorhandenen Entwürfe (für Tests und Anzeige). Blockiert.
pub fn count(data: &Path) -> usize {
    std::fs::read_dir(root(data))
        .map(|entries| {
            entries.flatten().filter(|e| e.file_name().to_str().is_some_and(valid_id) && e.path().is_dir()).count()
        })
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn draft_names_are_32_lowercase_hex_and_nothing_else() {
        assert!(valid_id(&new_id()), "ein neuer Name ist gültig");
        assert_ne!(new_id(), new_id(), "zufällig");
        assert!(!valid_id("../../etc/passwd"), "kein Pfad");
        assert!(!valid_id(&"A".repeat(32)), "nur Kleinbuchstaben");
        assert!(!valid_id(&"a".repeat(31)), "genau 32 Zeichen");
    }
}
