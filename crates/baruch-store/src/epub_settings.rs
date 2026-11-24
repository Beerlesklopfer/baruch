// SPDX-License-Identifier: AGPL-3.0-or-later
//! Einstellung für EPUBCheck (Runde 20, Besitz „epub-pruefung“): Pfad zu einem epubcheck.jar in `store_meta`
//! (Schlüssel [`EPUBCHECK_JAR`]), global, ohne Migration. Kein Buchtext und darum kein Commit im Spiegel. Prüfungen
//! des Pfads (absolut, `.jar`, vorhandene Datei, Länge) macht der Server.

use crate::{Store, StoreError};

/// Schlüssel in `store_meta`: Pfad zum epubcheck.jar.
const EPUBCHECK_JAR: &str = "epubcheck_jar";

impl Store {
    /// Eingestellter Pfad zum epubcheck.jar; `None`, wenn keiner gesetzt ist.
    pub async fn epubcheck_jar(&self) -> Result<Option<String>, StoreError> {
        Ok(sqlx::query_scalar("SELECT value FROM store_meta WHERE key = ?")
            .bind(EPUBCHECK_JAR)
            .fetch_optional(&self.pool)
            .await?)
    }

    /// Pfad setzen (`Some`) oder entfernen (`None`).
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
