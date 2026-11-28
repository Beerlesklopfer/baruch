// SPDX-License-Identifier: AGPL-3.0-or-later
//! Work language (migration `0055_work_language.sql`, Round 38, interface R38-S1).
//!
//! - `work.language`: `'de'` or `'en'`, default `'de'` (every work from before Round 38 is German).
//! - Not book text and not part of the mirror: no commit (like `work_timing`). Every change is one transaction
//!   (`BEGIN IMMEDIATE`).
//! - Independent of `TypesetSettings.lang` (hyphenation); the two are coupled in Round 39.

use crate::{Store, StoreError, WorkId, WorkLanguage};

impl Store {
    /// Language of the work. Unknown work → `UnknownWork`. A stored value that is no work language (the CHECK
    /// rule of the column forbids it) reads as the default.
    pub async fn work_language(&self, work: WorkId) -> Result<WorkLanguage, StoreError> {
        let code: Option<String> =
            sqlx::query_scalar("SELECT language FROM work WHERE id = ?").bind(work).fetch_optional(&self.pool).await?;
        let code = code.ok_or(StoreError::UnknownWork(work))?;
        Ok(WorkLanguage::parse(&code).unwrap_or_default())
    }

    /// Sets the language of the work. `true` = changed; then `work.updated` is set like in
    /// [`Store::update_work`]. The same language again changes nothing (`false`). Unknown work → `UnknownWork`.
    pub async fn set_work_language(&self, work: WorkId, language: WorkLanguage) -> Result<bool, StoreError> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let old: Option<String> =
            sqlx::query_scalar("SELECT language FROM work WHERE id = ?").bind(work).fetch_optional(&mut *tx).await?;
        let old = old.ok_or(StoreError::UnknownWork(work))?;
        if old == language.as_str() {
            return Ok(false);
        }
        sqlx::query("UPDATE work SET language = ?, updated = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ?")
            .bind(language.as_str())
            .bind(work)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use baruch_core::WorkKind;

    use crate::{NewWork, Preset, Store, StoreError, WorkLanguage};

    #[tokio::test]
    async fn the_work_language_starts_german_and_changes_only_when_set_to_another() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(dir.path()).await.unwrap();
        let new = NewWork {
            title: "The Ferry at Dusk",
            author: "Tilda Rohr",
            kind: WorkKind::Book,
            preset: Preset::Book,
            subtitle: "",
            audience: "",
            bible: "",
            language: Default::default(),
        };
        let work = store.create_work(&new, "Autor").await.unwrap().work;
        assert_eq!(
            store.work_language(work).await.unwrap(),
            WorkLanguage::De,
            "a new work has the default of migration 0055, like every work from before Round 38"
        );
        assert!(
            !store.set_work_language(work, WorkLanguage::De).await.unwrap(),
            "setting the language a work already has changes nothing and says so"
        );
        assert!(store.set_work_language(work, WorkLanguage::En).await.unwrap(), "setting another language is a change");
        assert_eq!(store.work_language(work).await.unwrap(), WorkLanguage::En, "the new language is stored");
        let updated: Option<String> = sqlx::query_scalar("SELECT updated FROM work WHERE id = ?")
            .bind(work)
            .fetch_one(&store.pool)
            .await
            .unwrap();
        assert!(updated.is_some(), "a changed language sets work.updated like update_work");
        assert!(
            matches!(store.work_language(9999).await, Err(StoreError::UnknownWork(9999))),
            "reading the language of a work that does not exist is an error, not the default"
        );
        assert!(
            matches!(store.set_work_language(9999, WorkLanguage::En).await, Err(StoreError::UnknownWork(9999))),
            "setting the language of a work that does not exist is an error"
        );
        store.set_work_language(work, WorkLanguage::De).await.unwrap();
        assert_eq!(
            store.work_language(work).await.unwrap(),
            WorkLanguage::De,
            "the unknown work above did not block the store: the transaction was released"
        );
    }
}
