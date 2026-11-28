// SPDX-License-Identifier: AGPL-3.0-or-later
//! Language of the user interface (Round 39a, interface R39a-S1, contract §2.5): the chosen language of the
//! interface in `store_meta`, without a migration. Not book text and therefore no commit in the mirror; the work
//! language (`work.language`, Round 38) is a different setting.
//!
//! Keys: [`INSTANCE_KEY`] for the instance (no accounts, or the author on the own machine) and
//! `ui.language.account.<id>` per account (authors and Lektors alike). The value is a two-letter code; which codes
//! the interface knows is decided by the server (`crate::i18n` of `baruch-server`).

use crate::{Store, StoreError};

/// Key in `store_meta`: language of the interface for the instance.
const INSTANCE_KEY: &str = "ui.language";

/// Refusal for a code that is not two lower-case ASCII letters.
pub const UI_LANGUAGE_INVALID: &str = "Sprache der Oberfläche: zwei Kleinbuchstaben (z. B. de oder en).";

fn key(account: Option<i64>) -> String {
    match account {
        Some(id) => format!("{INSTANCE_KEY}.account.{id}"),
        None => INSTANCE_KEY.to_owned(),
    }
}

impl Store {
    /// Stored language of the interface: of the account (`Some(id)`) or of the instance (`None`); `None` if none
    /// is set.
    pub async fn ui_language(&self, account: Option<i64>) -> Result<Option<String>, StoreError> {
        Ok(sqlx::query_scalar("SELECT value FROM store_meta WHERE key = ?")
            .bind(key(account))
            .fetch_optional(&self.pool)
            .await?)
    }

    /// Set the language of the interface (`Some(code)`) or remove it (`None`, then the browser decides again).
    /// A code other than two lower-case ASCII letters is refused and changes nothing.
    pub async fn set_ui_language(&self, account: Option<i64>, code: Option<&str>) -> Result<(), StoreError> {
        match code {
            Some(code) => {
                if code.len() != 2 || !code.bytes().all(|b| b.is_ascii_lowercase()) {
                    return Err(StoreError::Refused(UI_LANGUAGE_INVALID));
                }
                sqlx::query(
                    "INSERT INTO store_meta (key, value) VALUES (?, ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                )
                .bind(key(account))
                .bind(code)
                .execute(&self.pool)
                .await?;
            }
            None => {
                sqlx::query("DELETE FROM store_meta WHERE key = ?").bind(key(account)).execute(&self.pool).await?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::{Store, StoreError};

    #[tokio::test]
    async fn instance_and_account_languages_are_kept_apart_and_checked() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(dir.path()).await.unwrap();
        assert_eq!(store.ui_language(None).await.unwrap(), None, "a new data folder has no interface language");
        store.set_ui_language(None, Some("en")).await.unwrap();
        store.set_ui_language(Some(7), Some("de")).await.unwrap();
        assert_eq!(store.ui_language(None).await.unwrap().as_deref(), Some("en"), "the instance keeps its own value");
        assert_eq!(
            store.ui_language(Some(7)).await.unwrap().as_deref(),
            Some("de"),
            "an account has its own key and does not overwrite the instance"
        );
        assert_eq!(store.ui_language(Some(8)).await.unwrap(), None, "another account has nothing set");
        for bad in ["EN", "deu", "d", "", "d1", "é"] {
            assert!(
                matches!(store.set_ui_language(None, Some(bad)).await, Err(StoreError::Refused(_))),
                "{bad:?} is not two lower-case ASCII letters and must be refused"
            );
        }
        assert_eq!(store.ui_language(None).await.unwrap().as_deref(), Some("en"), "a refused code changes nothing");
        store.set_ui_language(None, None).await.unwrap();
        assert_eq!(store.ui_language(None).await.unwrap(), None, "None removes the instance setting");
        assert_eq!(store.ui_language(Some(7)).await.unwrap().as_deref(), Some("de"), "removing one keeps the other");
    }
}
