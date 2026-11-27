// SPDX-License-Identifier: AGPL-3.0-or-later
//! Bring the mirror of the ideas without a work (`<data>/einfaelle/`) up to date at start and after deleting a work
//! (Round 22, contract §2.16; owned by befund-rueckfrage).
//!
//! Until Round 22 the mirror only followed an action on an idea without a work ([`crate::mirror::record_idea`]).
//! If deleting a work turns its ideas into ideas without a work (`ON DELETE SET NULL`, migration 0011) or if
//! the database has otherwise drifted away from the mirror, [`sync_ideas_without_work`] catches up: one commit of
//! the human with `einfaelle.md` according to the state of the database, only if the file changes (no empty
//! commit). If there are neither ideas without a work nor the folder, nothing is created (no empty repo at the
//! first start).
//!
//! Callers: `mirror::sync_all` (start, hook from S1) and `works::delete` (after the log commit of the work). Both
//! hold the write lock (`held`).

use baruch_gitmirror::{CommitRequest, Signature};

use crate::AppState;
use crate::mirror::{HUMAN_EMAIL, HUMAN_NAME, IDEAS_DIR};

/// Trailer `Baruch-Action` of the catching up.
pub const ACTION: &str = "ideas-synced";

/// Brings the mirror of the ideas without a work up to the state of the database; `Ok(None)` = nothing to do (no
/// change to `einfaelle.md`, or neither ideas without a work nor a folder). `reason` becomes the commit subject.
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
