// SPDX-License-Identifier: AGPL-3.0-or-later
//! Runde 36 „Reste“ (Schnittstelle R36-S1, Besitz `reste`): was das Löschen eines Gliederungspunkts mitlöscht
//! (store-5), damit der Server es vor [`Store::outline_delete`] liest und im Spiegel-Commit protokolliert.
//!
//! Stand S1: nur Typen und Signatur, der Körper ist ein Stub ohne Aufrufer.

use crate::{Store, StoreError, WorkId};

/// Was [`Store::outline_delete`] außer der aktuellen Fassung mitlöscht.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OutlineLosses {
    pub revisions: Vec<LostRevision>,
    pub posts: Vec<LostPost>,
}

/// Eine mitgelöschte Fassung (Vorschläge mit `proposal = true`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LostRevision {
    pub number: u32,
    pub author: String,
    pub proposal: bool,
    pub body: String,
}

/// Ein mitgelöschter Beitrag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LostPost {
    pub id: i64,
    pub author: String,
    pub body: String,
}

impl Store {
    /// Was `outline_delete` außer der aktuellen Fassung mitlöscht: alle übrigen Fassungen
    /// (Vorschläge mit proposal = true) und alle Beiträge des Punkts. Nur lesend.
    pub async fn outline_delete_losses(&self, work: WorkId, node: usize) -> Result<OutlineLosses, StoreError> {
        // STUB: outline_delete_losses (Runde 36, reste)
        let _ = (work, node);
        Err(StoreError::Refused("outline_delete_losses: noch nicht umgesetzt (Runde 36, reste)"))
    }
}
