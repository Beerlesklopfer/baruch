// SPDX-License-Identifier: AGPL-3.0-or-later
//! Shares (Round 35, contract §S1.3 and §2; migration `0054_work_member.sql`): which Lektor account may see which work.
//!
//! The account kind is global, the role applies per work: an author account is author in every work and needs no row; a
//! Lektor account is Lektor only in works with a row in `work_member`. Instance data: not in the exchange format, no
//! commit in the mirror (the point in time is in `granted`).
//!
//! A locked account (`account.active = 0`) keeps its shares (after unlocking they apply again) but has no effective
//! one: [`Store::member_works`] returns nothing for it. That way a lock takes effect with the next request, even if a
//! session still existed.

use serde::Serialize;

use crate::{Store, StoreError, WorkId};

/// A share: account `account` has the role `role` in the work `work` (in this round only `lektor`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, sqlx::FromRow)]
pub struct WorkMember {
    pub work: WorkId,
    pub account: i64,
    pub role: String,
    pub granted: String,
}

/// The only role that can be shared in this round (the author needs no row).
const ROLE_LEKTOR: &str = "lektor";

impl Store {
    /// Shares of a work, ascending by account (also those of locked accounts). Unknown work →
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

    /// Share a work with an account; only accounts of the kind `lektor`, an existing share stays together with its
    /// point in time (idempotent). Unknown work → [`StoreError::UnknownWork`]; an unknown account, an author account or
    /// a role other than `lektor` → [`StoreError::Refused`], nothing written.
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

    /// Withdraw a share; `false` if there was none.
    pub async fn member_revoke(&self, work: WorkId, account: i64) -> Result<bool, StoreError> {
        let done = sqlx::query("DELETE FROM work_member WHERE work_id = ? AND account_id = ?")
            .bind(work)
            .bind(account)
            .execute(&self.pool)
            .await?;
        Ok(done.rows_affected() > 0)
    }

    /// The works effectively shared with an account, ascending by id: only for an active account of the kind `lektor`;
    /// empty for a locked, an unknown or an author account.
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
