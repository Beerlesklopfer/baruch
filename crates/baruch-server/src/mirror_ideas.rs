// SPDX-License-Identifier: AGPL-3.0-or-later
//! Spiegel der Einfälle ohne Werk (`<data>/einfaelle/`) beim Start und nach dem Löschen eines Werks nachführen
//! (Runde 22, Vertrag §2.16; Besitz befund-rueckfrage).
//!
//! Bis Runde 22 folgte der Spiegel nur einer Aktion an einem Einfall ohne Werk ([`crate::mirror::record_idea`]). Macht
//! das Löschen eines Werks dessen Einfälle zu Einfällen ohne Werk (`ON DELETE SET NULL`, Migration 0011) oder hat
//! sich die Datenbank sonst vom Spiegel entfernt, holt [`sync_ideas_without_work`] auf: ein Commit des Menschen mit
//! `einfaelle.md` nach dem Stand der Datenbank, nur wenn sich die Datei ändert (kein leerer Commit). Gibt es weder
//! Einfälle ohne Werk noch den Ordner, entsteht nichts (kein leeres Repo beim ersten Start).
//!
//! Aufrufer: `mirror::sync_all` (Start, Hook aus S1) und `works::delete` (nach dem Protokoll-Commit des Werks). Beide
//! halten die Schreibsperre (`held`).

use baruch_gitmirror::{CommitRequest, Signature};

use crate::AppState;
use crate::mirror::{HUMAN_EMAIL, HUMAN_NAME, IDEAS_DIR};

/// Trailer `Baruch-Action` des Nachführens.
pub const ACTION: &str = "ideas-synced";

/// Führt den Spiegel der Einfälle ohne Werk nach dem Stand der Datenbank nach; `Ok(None)` = nichts zu tun (keine
/// Änderung an `einfaelle.md`, bzw. weder Einfälle ohne Werk noch Ordner). `reason` wird der Betreff des Commits.
pub async fn sync_ideas_without_work(
    state: &AppState,
    held: &crate::mirror::WriteGuard<'_>,
    reason: &str,
) -> Result<Option<String>, String> {
    let _ = held;
    let file = state.store.idea_mirror_file(None).await.map_err(|e| e.to_string())?;
    let dir = state.data.join(IDEAS_DIR);
    if file.is_none() && std::fs::symlink_metadata(&dir).is_err() {
        return Ok(None);
    }
    let human = Signature { name: HUMAN_NAME.to_owned(), email: HUMAN_EMAIL.to_owned() };
    let request = CommitRequest {
        files: Vec::new(),
        author: human.clone(),
        committer: human,
        summary: reason.to_owned(),
        body: "Einfälle ohne Werk nach dem Stand der Datenbank (einfaelle.md).\n".to_owned(),
        trailers: vec![("Baruch-Action".to_owned(), ACTION.to_owned())],
        allow_empty: false,
    };
    let result = crate::mirror::commit_ideas_without_work(state, request).await;
    if let Err(e) = &result {
        eprintln!("Git-Spiegel (Einfälle ohne Werk): {e}");
    }
    result
}
