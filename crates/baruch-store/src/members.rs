// SPDX-License-Identifier: AGPL-3.0-or-later
//! Freigaben (Runde 35, Vertrag §S1.3 und §2; Migration `0054_work_member.sql`): welches Lektor-Konto welches Werk
//! sehen darf.
//!
//! Die Kontoart ist global, die Rolle gilt je Werk: Ein Autor-Konto ist Autor in jedem Werk und braucht keine Zeile;
//! ein Lektor-Konto ist Lektor nur in Werken mit Zeile in `work_member`. Instanzdaten: nicht im Austauschformat,
//! kein Commit im Spiegel (der Zeitpunkt steht in `granted`).
//!
//! Schnittstelle R35-S1: Der Typ ist echt, die Methoden antworten ehrlich mit „noch nicht umgesetzt“; „rollen“ setzt
//! sie um.

use serde::Serialize;

use crate::accounts::NOT_YET;
use crate::{Store, StoreError, WorkId};

/// Eine Freigabe: Konto `account` hat im Werk `work` die Rolle `role` (in dieser Runde nur `lektor`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, sqlx::FromRow)]
pub struct WorkMember {
    pub work: WorkId,
    pub account: i64,
    pub role: String,
    pub granted: String,
}

impl Store {
    /// Freigaben eines Werks.
    pub async fn member_list(&self, work: WorkId) -> Result<Vec<WorkMember>, StoreError> {
        // STUB: Freigaben eines Werks lesen (Runde 35)
        let _ = work;
        Err(StoreError::Refused(NOT_YET))
    }

    /// Werk für ein Konto freigeben; nur Konten der Art `lektor`, eine vorhandene Freigabe bleibt (idempotent).
    pub async fn member_grant(&self, work: WorkId, account: i64, role: &str) -> Result<(), StoreError> {
        // STUB: Freigabe anlegen (Runde 35)
        let _ = (work, account, role);
        Err(StoreError::Refused(NOT_YET))
    }

    /// Freigabe entziehen; `false`, wenn es keine gab.
    pub async fn member_revoke(&self, work: WorkId, account: i64) -> Result<bool, StoreError> {
        // STUB: Freigabe entziehen (Runde 35)
        let _ = (work, account);
        Err(StoreError::Refused(NOT_YET))
    }

    /// Die für ein Konto freigegebenen Werke, aufsteigend nach id.
    pub async fn member_works(&self, account: i64) -> Result<Vec<WorkId>, StoreError> {
        // STUB: freigegebene Werke eines Kontos lesen (Runde 35)
        let _ = account;
        Err(StoreError::Refused(NOT_YET))
    }
}
