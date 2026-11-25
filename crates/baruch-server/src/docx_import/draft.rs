// SPDX-License-Identifier: AGPL-3.0-or-later
//! Entwürfe eines Word-Imports (Runde 32) als Dateien unter `<data>/docx-drafts/<draft>/` (keine Migration):
//! `draft.json` (Kennung, Zeitpunkt, Dateiname, gewünschter Titel, Vorschau) und `source.docx` (die Datei selbst;
//! beim Übernehmen wird sie neu gelesen). Gehalten wie `pdf_import/draft.rs` (Vertrag Runde 32 §2.1):
//!
//! `draft` = 32 Kleinbuchstaben-Hex (zufällig, [`baruch_store::new_uuid`] ohne Bindestriche); jeder andere Name wird
//! abgelehnt (kein Pfad aus Eingaben). Höchstens [`MAX_DRAFTS`] Entwürfe und [`MAX_AGE`]: Beim nächsten `preview`
//! fallen ältere Entwürfe weg, danach die ältesten, bis Platz für einen neuen ist.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use super::DRAFT_DIR;

/// Höchstens so viele Entwürfe liegen gleichzeitig vor.
pub const MAX_DRAFTS: usize = 20;
/// So lange bleibt ein Entwurf.
pub const MAX_AGE: Duration = Duration::from_secs(24 * 60 * 60);

const DRAFT_FILE: &str = "draft.json";
const SOURCE_FILE: &str = "source.docx";

/// Ein Entwurf: die Angaben und die Vorschau, wie `docx_import.preview` sie geliefert hat.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Draft {
    pub id: String,
    /// Sekunden seit 1970 (UTC).
    pub created: u64,
    pub file_name: String,
    /// Titel aus Formular bzw. RPC (`title`), falls angegeben.
    pub wanted_title: Option<String>,
    /// Antwort von `docx_import.preview` (auch für `docx_import.get`).
    pub preview: serde_json::Value,
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
    data.join(DRAFT_DIR)
}

fn folder(data: &Path, id: &str) -> Option<PathBuf> {
    valid_id(id).then(|| root(data).join(id))
}

fn created_of(dir: &Path) -> u64 {
    if let Ok(text) = std::fs::read_to_string(dir.join(DRAFT_FILE))
        && let Ok(value) = serde_json::from_str::<serde_json::Value>(&text)
        && let Some(created) = value.get("created").and_then(serde_json::Value::as_u64)
    {
        return created;
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

/// Neuen Entwurf samt Datei schreiben. Blockiert.
pub fn save(data: &Path, draft: &Draft, bytes: &[u8]) -> Result<(), String> {
    let dir = folder(data, &draft.id).ok_or("ungültiger Entwurf")?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("Entwurf lässt sich nicht anlegen: {e}"))?;
    std::fs::write(dir.join(SOURCE_FILE), bytes).map_err(|e| format!("Entwurf lässt sich nicht schreiben: {e}"))?;
    let text = serde_json::to_vec(draft).map_err(|e| format!("Entwurf: {e}"))?;
    std::fs::write(dir.join(DRAFT_FILE), text).map_err(|e| format!("Entwurf lässt sich nicht schreiben: {e}"))
}

/// Fehler beim Laden eines Entwurfs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadError {
    /// Ungültiger Name oder kein solcher (bzw. abgelaufener) Entwurf.
    Unknown,
    Broken(String),
}

/// Entwurf laden (ohne die Datei). Abgelaufene gelten als unbekannt. Blockiert.
pub fn load(data: &Path, id: &str) -> Result<Draft, LoadError> {
    let dir = folder(data, id).ok_or(LoadError::Unknown)?;
    let text = std::fs::read_to_string(dir.join(DRAFT_FILE)).map_err(|_| LoadError::Unknown)?;
    let draft: Draft = serde_json::from_str(&text).map_err(|e| LoadError::Broken(format!("Entwurf unlesbar: {e}")))?;
    if draft.created < now().saturating_sub(MAX_AGE.as_secs()) {
        return Err(LoadError::Unknown);
    }
    Ok(draft)
}

/// Die Word-Datei eines Entwurfs. Blockiert.
pub fn load_source(data: &Path, id: &str) -> Result<Vec<u8>, String> {
    let dir = folder(data, id).ok_or("ungültiger Entwurf")?;
    std::fs::read(dir.join(SOURCE_FILE)).map_err(|e| format!("Datei des Entwurfs fehlt: {e}"))
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

    #[test]
    fn drafts_are_saved_loaded_cleaned_and_removed() {
        let data = tempfile::tempdir().unwrap();
        let draft = |id: &str, created: u64| Draft {
            id: id.to_owned(),
            created,
            file_name: "deich.docx".into(),
            wanted_title: None,
            preview: serde_json::json!({ "draft": id }),
        };
        let fresh = new_id();
        save(data.path(), &draft(&fresh, now()), b"PK").unwrap();
        assert_eq!(load(data.path(), &fresh).unwrap().file_name, "deich.docx", "ein gespeicherter Entwurf lädt wieder");
        assert_eq!(load_source(data.path(), &fresh).unwrap(), b"PK", "samt seiner Datei");
        let old = new_id();
        save(data.path(), &draft(&old, now() - MAX_AGE.as_secs() - 60), b"PK").unwrap();
        assert_eq!(load(data.path(), &old), Err(LoadError::Unknown), "älter als 24 Stunden gilt als unbekannt");
        assert_eq!(clean(data.path()), 1, "das Aufräumen entfernt genau den abgelaufenen");
        assert_eq!(count(data.path()), 1, "der frische bleibt");
        assert!(save(data.path(), &draft("../x", now()), b"PK").is_err(), "kein Pfad aus einer Eingabe");
        assert!(remove(data.path(), &fresh), "löschen meldet, dass es ihn gab");
        assert!(!remove(data.path(), &fresh), "ein zweites Mal gibt es ihn nicht mehr");
        assert_eq!(count(data.path()), 0, "danach liegt kein Entwurf mehr vor");
    }
}
