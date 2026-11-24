// SPDX-License-Identifier: AGPL-3.0-or-later
//! SWORD-Bibeln (Migration 0030): Modulordner (global) und gewählte Bibel je Werk. Kein Buchtext und darum
//! kein Commit im Spiegel. Prüfungen der Eingaben (Länge, Zeilenumbruch, Doppel) macht der Server; die
//! Tabelle sichert sie zusätzlich mit CHECK.

use crate::{Store, StoreError, WorkId};

/// Schlüssel in `store_meta`: Die Modulordner sind gesetzt (sonst gelten die Vorgaben).
const DIRS_SET: &str = "bible_dirs";

impl Store {
    /// Modulordner in ihrer Reihenfolge; `None`, wenn sie nie gesetzt wurden (dann gelten die Vorgaben).
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

    /// Modulordner setzen (`Some`, auch leer) oder auf die Vorgaben zurück (`None`). Doppelte Einträge
    /// bleiben nur einmal stehen (der erste).
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

    /// Gewählte Bibel (Modulname) eines Werks; `None` = keine. Unbekanntes Werk → `UnknownWork`.
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

    /// Bibel eines Werks wählen (`Some(Modulname)`) oder abwählen (`None`).
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
