// SPDX-License-Identifier: AGPL-3.0-or-later
//! SWORD Bibles (migration 0030): module folders (global) and the chosen Bible per work. Not book text and therefore no
//! commit in the mirror. The checks of the inputs (length, line break, duplicates) are done by the server; the table
//! additionally secures them with CHECK.

use crate::{Store, StoreError, WorkId};

/// Key in `store_meta`: the module folders are set (otherwise the defaults apply).
const DIRS_SET: &str = "bible_dirs";

impl Store {
    /// Module folders in their order; `None` if they were never set (then the defaults apply).
    pub async fn bible_dirs(&self) -> Result<Option<Vec<String>>, StoreError> {
        let mut conn = self.pool.acquire().await?;
        let set: Option<String> = sqlx::query_scalar("SELECT value FROM store_meta WHERE key = ?")
            .bind(DIRS_SET)
            .fetch_optional(&mut *conn)
            .await?;
        if set.is_none() {
            return Ok(None);
        }
        let dirs: Vec<String> =
            sqlx::query_scalar("SELECT path FROM bible_dir ORDER BY position").fetch_all(&mut *conn).await?;
        Ok(Some(dirs))
    }

    /// Set the module folders (`Some`, also empty) or go back to the defaults (`None`). Duplicate entries are kept only
    /// once (the first one).
    pub async fn set_bible_dirs(&self, dirs: Option<&[String]>) -> Result<(), StoreError> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        sqlx::query("DELETE FROM bible_dir").execute(&mut *tx).await?;
        match dirs {
            None => {
                sqlx::query("DELETE FROM store_meta WHERE key = ?").bind(DIRS_SET).execute(&mut *tx).await?;
            }
            Some(dirs) => {
                let mut seen: Vec<&str> = Vec::new();
                for dir in dirs {
                    if seen.contains(&dir.as_str()) {
                        continue;
                    }
                    seen.push(dir);
                    sqlx::query("INSERT INTO bible_dir (position, path) VALUES (?, ?)")
                        .bind(seen.len() as i64)
                        .bind(dir)
                        .execute(&mut *tx)
                        .await?;
                }
                sqlx::query(
                    "INSERT INTO store_meta (key, value) VALUES (?, '1') ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                )
                .bind(DIRS_SET)
                .execute(&mut *tx)
                .await?;
            }
        }
        tx.commit().await?;
        Ok(())
    }

    /// Chosen Bible (module name) of a work; `None` = none. Unknown work → `UnknownWork`.
    pub async fn work_bible(&self, work: WorkId) -> Result<Option<String>, StoreError> {
        let mut conn = self.pool.acquire().await?;
        let found: Option<i64> =
            sqlx::query_scalar("SELECT id FROM work WHERE id = ?").bind(work).fetch_optional(&mut *conn).await?;
        found.ok_or(StoreError::UnknownWork(work))?;
        Ok(sqlx::query_scalar("SELECT module FROM work_bible WHERE work_id = ?")
            .bind(work)
            .fetch_optional(&mut *conn)
            .await?)
    }

    /// Choose the Bible of a work (`Some(module name)`) or deselect it (`None`).
    pub async fn set_work_bible(&self, work: WorkId, module: Option<&str>) -> Result<(), StoreError> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let found: Option<i64> =
            sqlx::query_scalar("SELECT id FROM work WHERE id = ?").bind(work).fetch_optional(&mut *tx).await?;
        found.ok_or(StoreError::UnknownWork(work))?;
        match module {
            Some(module) => {
                sqlx::query(
                    "INSERT INTO work_bible (work_id, module) VALUES (?, ?) ON CONFLICT(work_id) DO UPDATE SET \
                     module = excluded.module, updated = strftime('%Y-%m-%dT%H:%M:%fZ','now')",
                )
                .bind(work)
                .bind(module)
                .execute(&mut *tx)
                .await?;
            }
            None => {
                sqlx::query("DELETE FROM work_bible WHERE work_id = ?").bind(work).execute(&mut *tx).await?;
            }
        }
        tx.commit().await?;
        Ok(())
    }
}
