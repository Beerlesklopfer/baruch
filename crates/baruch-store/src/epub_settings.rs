// SPDX-License-Identifier: AGPL-3.0-or-later
//! Setting for EPUBCheck (Round 20, ownership "epub-pruefung"): path to an epubcheck.jar in `store_meta` (key
//! [`EPUBCHECK_JAR`]), global, without a migration. Not book text and therefore no commit in the mirror. The checks of
//! the path (absolute, `.jar`, existing file, length) are done by the server.

use crate::{Store, StoreError};

/// Key in `store_meta`: path to the epubcheck.jar.
const EPUBCHECK_JAR: &str = "epubcheck_jar";

impl Store {
    /// Configured path to the epubcheck.jar; `None` if none is set.
    pub async fn epubcheck_jar(&self) -> Result<Option<String>, StoreError> {
        Ok(sqlx::query_scalar("SELECT value FROM store_meta WHERE key = ?")
            .bind(EPUBCHECK_JAR)
            .fetch_optional(&self.pool)
            .await?)
    }

    /// Set the path (`Some`) or remove it (`None`).
    pub async fn set_epubcheck_jar(&self, path: Option<&str>) -> Result<(), StoreError> {
        match path {
            Some(path) => {
                sqlx::query(
                    "INSERT INTO store_meta (key, value) VALUES (?, ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                )
                .bind(EPUBCHECK_JAR)
                .bind(path)
                .execute(&self.pool)
                .await?;
            }
            None => {
                sqlx::query("DELETE FROM store_meta WHERE key = ?").bind(EPUBCHECK_JAR).execute(&self.pool).await?;
            }
        }
        Ok(())
    }
}
