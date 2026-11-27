// SPDX-License-Identifier: AGPL-3.0-or-later
//! Globally unique identifiers (Round 26, migration 0041; interface S1, afterwards the module belongs to ids).
//!
//! - Work, outline item, revision, template revision, post, idea, thread, thread post, image and image version carry a
//!   column `uuid` (canonical lower case 8-4-4-4-12). New rows get it through the trigger `<T>_uuid_fill` as v4;
//!   numeric ids stay internal. Since Round 25 (migration 0043) likewise links (`idea_link`) and positions
//!   (`idea_map_pos`) of the mind map.
//! - Deleted rows leave a tombstone in `uuid_tombstone` (kinds from 0041 and `idea_link`, `idea_map_pos` from 0043).
//! - Instance: `store_meta.instance_uuid` (from the migration) and `store_meta.instance_name` (default empty).
//! - New rows get their uuid only through the triggers; no write path of the store names the column.
//! - Only `exchange_apply` sets a uuid itself, and only on a row that it created in the same transaction.

use crate::{Store, StoreError, WorkId};

/// Identifier and name of this instance.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Instance {
    pub uuid: String,
    /// Empty = without a name (displayed as „ohne Namen (<first 8 characters>)“).
    pub name: String,
}

impl Instance {
    /// Display name: the name, without a name „ohne Namen (<first 8 characters of the UUID>)“.
    pub fn label(&self) -> String {
        if self.name.is_empty() {
            format!("ohne Namen ({})", self.uuid.chars().take(8).collect::<String>())
        } else {
            self.name.clone()
        }
    }
}

/// Check the instance name: trimmed, at most [`MAX_INSTANCE_NAME_CHARS`] characters, no control characters (no line
/// breaks or tabs in the name either). Empty stays empty (= reset).
pub(crate) fn check_instance_name(name: &str) -> Result<String, StoreError> {
    let name = name.trim();
    let chars = name.chars().count();
    if chars > MAX_INSTANCE_NAME_CHARS {
        return Err(StoreError::Exchange(format!(
            "Der Name der Instanz darf höchstens {MAX_INSTANCE_NAME_CHARS} Zeichen haben (jetzt {chars}); nichts gespeichert."
        )));
    }
    if name.chars().any(char::is_control) {
        return Err(StoreError::Exchange(
            "Der Name der Instanz darf keine Steuerzeichen (Zeilenumbruch, Tab …) enthalten; nichts gespeichert."
                .into(),
        ));
    }
    Ok(name.to_owned())
}

/// Maximum length of the instance name in characters.
pub const MAX_INSTANCE_NAME_CHARS: usize = baruch_exchange::MAX_INSTANCE_NAME_CHARS;

/// New random UUID version 4 (variant 10) in canonical lower case.
pub fn new_uuid() -> String {
    let mut bytes = [0u8; 16];
    getrandom::getrandom(&mut bytes).expect("Kein Zufall vom Betriebssystem für eine UUID");
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    format!("{}-{}-{}-{}-{}", &hex[0..8], &hex[8..12], &hex[12..16], &hex[16..20], &hex[20..32])
}

impl Store {
    /// Identifier and name of this instance.
    pub async fn instance(&self) -> Result<Instance, StoreError> {
        let uuid: Option<String> = sqlx::query_scalar("SELECT value FROM store_meta WHERE key = 'instance_uuid'")
            .fetch_optional(&self.pool)
            .await?;
        let uuid = uuid.ok_or_else(|| StoreError::Exchange("Instanzkennung fehlt (Migration 0041)".into()))?;
        let name: Option<String> = sqlx::query_scalar("SELECT value FROM store_meta WHERE key = 'instance_name'")
            .fetch_optional(&self.pool)
            .await?;
        Ok(Instance { uuid, name: name.unwrap_or_default() })
    }

    /// Set the name of the instance (trimmed, at most [`MAX_INSTANCE_NAME_CHARS`] characters, no control characters,
    /// empty = reset).
    /// Invalid → `StoreError::Exchange` with a German message (RPC: `INVALID_PARAMS`), nothing stored.
    pub async fn set_instance_name(&self, name: &str) -> Result<Instance, StoreError> {
        let name = check_instance_name(name)?;
        if name.is_empty() {
            sqlx::query("DELETE FROM store_meta WHERE key = 'instance_name'").execute(&self.pool).await?;
        } else {
            sqlx::query(
                "INSERT INTO store_meta (key, value) VALUES ('instance_name', ?) \
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            )
            .bind(&name)
            .execute(&self.pool)
            .await?;
        }
        self.instance().await
    }

    /// UUID of a work; unknown → `UnknownWork`.
    pub async fn work_uuid(&self, work: WorkId) -> Result<String, StoreError> {
        sqlx::query_scalar("SELECT uuid FROM work WHERE id = ?")
            .bind(work)
            .fetch_optional(&self.pool)
            .await?
            .ok_or(StoreError::UnknownWork(work))
    }

    /// Work for the UUID, if there is one here.
    pub async fn work_by_uuid(&self, uuid: &str) -> Result<Option<WorkId>, StoreError> {
        Ok(sqlx::query_scalar("SELECT id FROM work WHERE uuid = ?").bind(uuid).fetch_optional(&self.pool).await?)
    }

    /// Fixed key (`key` = `node.id`) of the outline item with this UUID in the work.
    pub async fn node_key_by_uuid(&self, work: WorkId, uuid: &str) -> Result<Option<i64>, StoreError> {
        Ok(sqlx::query_scalar("SELECT id FROM node WHERE work_id = ? AND uuid = ?")
            .bind(work)
            .bind(uuid)
            .fetch_optional(&self.pool)
            .await?)
    }

    /// UUID of the outline item with the fixed key `key`; not (any longer) in the work → `GoneNode`.
    pub async fn node_uuid(&self, work: WorkId, key: i64) -> Result<String, StoreError> {
        sqlx::query_scalar("SELECT uuid FROM node WHERE work_id = ? AND id = ?")
            .bind(work)
            .bind(key)
            .fetch_optional(&self.pool)
            .await?
            .ok_or(StoreError::GoneNode { work, key })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_uuids_are_canonical_v4() {
        let a = new_uuid();
        let b = new_uuid();
        // Canonical lower case 8-4-4-4-12, otherwise the CHECK of the column or the reader would reject it.
        assert!(baruch_exchange::is_uuid(&a), "kanonisch: {a}");
        // Version 4 at position 14, variant 10 (8, 9, a or b) at position 19 (RFC 9562).
        assert_eq!(&a[14..15], "4", "Version 4: {a}");
        assert!(matches!(&a[19..20], "8" | "9" | "a" | "b"), "Variante 10: {a}");
        // Randomness: two calls yield different identifiers.
        assert_ne!(a, b, "zwei neue Kennungen sind verschieden");
    }

    #[tokio::test]
    async fn migration_0041_fills_uuids_and_leaves_tombstones() {
        let data = tempfile::tempdir().unwrap();
        // New database: seeding with the sample work, afterwards the triggers `<T>_uuid_fill` run for every row.
        let store = Store::open(data.path()).await.unwrap();
        let instance = store.instance().await.unwrap();
        assert!(baruch_exchange::is_uuid(&instance.uuid), "Instanzkennung aus der Migration: {}", instance.uuid);
        assert_eq!(instance.name, "", "Name der Instanz ist anfangs leer");
        let (work, slug, title) = store.works().await.unwrap().remove(0);
        let _ = slug;
        let uuid = store.work_uuid(work).await.unwrap();
        assert!(baruch_exchange::is_uuid(&uuid), "Werk-UUID kanonisch: {uuid}");
        assert_eq!(store.work_by_uuid(&uuid).await.unwrap(), Some(work), "Werk über seine UUID");
        // Every row of every table with work content has a UUID (fill trigger).
        for table in ["work", "node", "revision", "work_template_revision", "post"] {
            let missing: i64 =
                sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT count(*) FROM {table} WHERE uuid IS NULL")))
                    .fetch_one(&store.pool)
                    .await
                    .unwrap();
            assert_eq!(missing, 0, "{table}: keine Zeile ohne UUID");
        }
        let key: i64 = sqlx::query_scalar("SELECT id FROM node WHERE work_id = ? ORDER BY seq LIMIT 1")
            .bind(work)
            .fetch_one(&store.pool)
            .await
            .unwrap();
        let node = store.node_uuid(work, key).await.unwrap();
        assert_eq!(store.node_key_by_uuid(work, &node).await.unwrap(), Some(key), "Punkt über seine UUID");
        assert!(
            matches!(store.node_uuid(work, -1).await, Err(StoreError::GoneNode { .. })),
            "unbekannter Schlüssel → GoneNode"
        );
        // Deleting the whole work leaves tombstones for work and items (trigger `<T>_uuid_tombstone`).
        store.delete_work(work, &title).await.unwrap();
        let kinds: Vec<(String, Option<String>)> =
            sqlx::query_as("SELECT kind, work_uuid FROM uuid_tombstone WHERE uuid IN (?, ?) ORDER BY kind")
                .bind(&uuid)
                .bind(&node)
                .fetch_all(&store.pool)
                .await
                .unwrap();
        assert_eq!(
            kinds,
            vec![("node".to_owned(), Some(uuid.clone())), ("work".to_owned(), Some(uuid.clone()))],
            "Grabsteine für Werk und Punkt, beide mit der Werk-UUID"
        );
        let named = store.set_instance_name("  Probe  ").await.unwrap();
        assert_eq!(named.name, "Probe", "Instanzname getrimmt gespeichert");
    }
}
