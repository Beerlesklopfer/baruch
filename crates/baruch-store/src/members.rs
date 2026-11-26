// SPDX-License-Identifier: AGPL-3.0-or-later
//! Freigaben (Runde 35, Vertrag §S1.3 und §2; Migration `0054_work_member.sql`): welches Lektor-Konto welches Werk
//! sehen darf.
//!
//! Die Kontoart ist global, die Rolle gilt je Werk: Ein Autor-Konto ist Autor in jedem Werk und braucht keine Zeile;
//! ein Lektor-Konto ist Lektor nur in Werken mit Zeile in `work_member`. Instanzdaten: nicht im Austauschformat,
//! kein Commit im Spiegel (der Zeitpunkt steht in `granted`).
//!
//! Ein gesperrtes Konto (`account.active = 0`) behält seine Freigaben (nach dem Entsperren gelten sie wieder), hat
//! aber keine wirksame: [`Store::member_works`] liefert für es nichts. So wirkt eine Sperre bei der nächsten Anfrage,
//! auch wenn eine Sitzung noch bestünde.

use serde::Serialize;

use crate::{Store, StoreError, WorkId};

/// Eine Freigabe: Konto `account` hat im Werk `work` die Rolle `role` (in dieser Runde nur `lektor`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, sqlx::FromRow)]
pub struct WorkMember {
    pub work: WorkId,
    pub account: i64,
    pub role: String,
    pub granted: String,
}

/// Die einzige Rolle, die sich in dieser Runde freigeben lässt (der Autor braucht keine Zeile).
const ROLE_LEKTOR: &str = "lektor";

impl Store {
    /// Freigaben eines Werks, nach Konto aufsteigend (auch die gesperrter Konten). Unbekanntes Werk →
    /// [`StoreError::UnknownWork`].
    pub async fn member_list(&self, work: WorkId) -> Result<Vec<WorkMember>, StoreError> {
        self.work_slug(work).await?;
        let rows = sqlx::query_as::<_, WorkMember>(
            "SELECT work_id AS work, account_id AS account, role, granted FROM work_member WHERE work_id = ? \
             ORDER BY account_id",
        )
        .bind(work)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Werk für ein Konto freigeben; nur Konten der Art `lektor`, eine vorhandene Freigabe bleibt samt ihrem
    /// Zeitpunkt (idempotent). Unbekanntes Werk → [`StoreError::UnknownWork`]; unbekanntes Konto, ein Autor-Konto
    /// oder eine andere Rolle als `lektor` → [`StoreError::Refused`], nichts geschrieben.
    pub async fn member_grant(&self, work: WorkId, account: i64, role: &str) -> Result<(), StoreError> {
        if role != ROLE_LEKTOR {
            return Err(StoreError::Refused("Freigeben lässt sich nur die Rolle „lektor“."));
        }
        self.work_slug(work).await?;
        let mut tx = self.pool.begin().await?;
        let kind: Option<String> =
            sqlx::query_scalar("SELECT kind FROM account WHERE id = ?").bind(account).fetch_optional(&mut *tx).await?;
        match kind.as_deref() {
            None => return Err(StoreError::Refused("Unbekanntes Konto: nichts freigegeben.")),
            Some("lektor") => {}
            Some(_) => {
                return Err(StoreError::Refused(
                    "Nur Lektor-Konten bekommen eine Freigabe; der Autor ist Autor in jedem Werk.",
                ));
            }
        }
        sqlx::query("INSERT INTO work_member (work_id, account_id, role) VALUES (?, ?, ?) ON CONFLICT DO NOTHING")
            .bind(work)
            .bind(account)
            .bind(role)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }

    /// Freigabe entziehen; `false`, wenn es keine gab.
    pub async fn member_revoke(&self, work: WorkId, account: i64) -> Result<bool, StoreError> {
        let done = sqlx::query("DELETE FROM work_member WHERE work_id = ? AND account_id = ?")
            .bind(work)
            .bind(account)
            .execute(&self.pool)
            .await?;
        Ok(done.rows_affected() > 0)
    }

    /// Die für ein Konto wirksam freigegebenen Werke, aufsteigend nach id: nur für ein aktives Konto der Art
    /// `lektor`; für ein gesperrtes, ein unbekanntes oder ein Autor-Konto leer.
    pub async fn member_works(&self, account: i64) -> Result<Vec<WorkId>, StoreError> {
        let works = sqlx::query_scalar::<_, WorkId>(
            "SELECT m.work_id FROM work_member m JOIN account a ON a.id = m.account_id \
             WHERE m.account_id = ? AND a.active = 1 AND a.kind = 'lektor' AND m.role = 'lektor' ORDER BY m.work_id",
        )
        .bind(account)
        .fetch_all(&self.pool)
        .await?;
        Ok(works)
    }
}
