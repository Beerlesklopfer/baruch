// SPDX-License-Identifier: AGPL-3.0-or-later
//! Weltweit eindeutige Kennungen (Runde 26, Migration 0041; Schnittstelle S1, danach gehört das Modul ids).
//!
//! - Werk, Gliederungspunkt, Fassung, Vorlagenfassung, Beitrag, Einfall, Faden, Fadenbeitrag, Bild und
//!   Bildfassung tragen eine Spalte `uuid` (kanonisch klein 8-4-4-4-12). Neue Zeilen bekommen sie über den
//!   Trigger `<T>_uuid_fill` als v4; Zahlen-ids bleiben intern.
//!   Seit Runde 25 (Migration 0043) ebenso Verbindungen (`idea_link`) und Lagen (`idea_map_pos`) der
//!   Gedankenkarte.
//! - Gelöschte Zeilen hinterlassen einen Grabstein in `uuid_tombstone` (Arten aus 0041 und `idea_link`,
//!   `idea_map_pos` aus 0043).
//! - Instanz: `store_meta.instance_uuid` (aus der Migration) und `store_meta.instance_name` (Vorgabe leer).
//! - Neue Zeilen bekommen ihre uuid nur über die Trigger; kein Schreibweg des Stores nennt die Spalte.
//! - Nur `exchange_apply` setzt eine uuid selbst, und nur auf einer Zeile, die es in derselben Transaktion
//!   angelegt hat.

use crate::{Store, StoreError, WorkId};

/// Kennung und Name dieser Instanz.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Instance {
    pub uuid: String,
    /// Leer = ohne Namen (angezeigt als „ohne Namen (<erste 8 Zeichen>)“).
    pub name: String,
}

impl Instance {
    /// Anzeigename: der Name, ohne Namen „ohne Namen (<erste 8 Zeichen der UUID>)“.
    pub fn label(&self) -> String {
        if self.name.is_empty() {
            format!("ohne Namen ({})", self.uuid.chars().take(8).collect::<String>())
        } else {
            self.name.clone()
        }
    }
}

/// Instanzname prüfen: getrimmt, höchstens [`MAX_INSTANCE_NAME_CHARS`] Zeichen, keine Steuerzeichen (auch
/// keine Zeilenumbrüche oder Tabs im Namen). Leer bleibt leer (= zurücksetzen).
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

/// Höchstlänge des Instanznamens in Zeichen.
pub const MAX_INSTANCE_NAME_CHARS: usize = baruch_exchange::MAX_INSTANCE_NAME_CHARS;

/// Neue zufällige UUID Version 4 (Variante 10) in kanonischer Kleinschreibung.
pub fn new_uuid() -> String {
    let mut bytes = [0u8; 16];
    getrandom::getrandom(&mut bytes).expect("Kein Zufall vom Betriebssystem für eine UUID");
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    format!("{}-{}-{}-{}-{}", &hex[0..8], &hex[8..12], &hex[12..16], &hex[16..20], &hex[20..32])
}

impl Store {
    /// Kennung und Name dieser Instanz.
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

    /// Namen der Instanz setzen (getrimmt, höchstens [`MAX_INSTANCE_NAME_CHARS`] Zeichen, keine Steuerzeichen,
    /// leer = zurücksetzen).
    /// Ungültig → `StoreError::Exchange` mit deutscher Meldung (RPC: `INVALID_PARAMS`), nichts gespeichert.
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

    /// UUID eines Werks; unbekannt → `UnknownWork`.
    pub async fn work_uuid(&self, work: WorkId) -> Result<String, StoreError> {
        sqlx::query_scalar("SELECT uuid FROM work WHERE id = ?")
            .bind(work)
            .fetch_optional(&self.pool)
            .await?
            .ok_or(StoreError::UnknownWork(work))
    }

    /// Werk zur UUID, falls es hier eines gibt.
    pub async fn work_by_uuid(&self, uuid: &str) -> Result<Option<WorkId>, StoreError> {
        Ok(sqlx::query_scalar("SELECT id FROM work WHERE uuid = ?").bind(uuid).fetch_optional(&self.pool).await?)
    }

    /// Fester Schlüssel (`key` = `node.id`) des Gliederungspunkts mit dieser UUID im Werk.
    pub async fn node_key_by_uuid(&self, work: WorkId, uuid: &str) -> Result<Option<i64>, StoreError> {
        Ok(sqlx::query_scalar("SELECT id FROM node WHERE work_id = ? AND uuid = ?")
            .bind(work)
            .bind(uuid)
            .fetch_optional(&self.pool)
            .await?)
    }

    /// UUID des Gliederungspunkts mit dem festen Schlüssel `key`; nicht (mehr) im Werk → `GoneNode`.
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
        // Kanonisch klein 8-4-4-4-12, sonst lehnte der CHECK der Spalte bzw. der Leser sie ab.
        assert!(baruch_exchange::is_uuid(&a), "kanonisch: {a}");
        // Version 4 an Stelle 14, Variante 10 (8, 9, a oder b) an Stelle 19 (RFC 9562).
        assert_eq!(&a[14..15], "4", "Version 4: {a}");
        assert!(matches!(&a[19..20], "8" | "9" | "a" | "b"), "Variante 10: {a}");
        // Zufall: zwei Aufrufe ergeben verschiedene Kennungen.
        assert_ne!(a, b, "zwei neue Kennungen sind verschieden");
    }

    #[tokio::test]
    async fn migration_0041_fills_uuids_and_leaves_tombstones() {
        let data = tempfile::tempdir().unwrap();
        // Neue Datenbank: Saat mit dem Beispielwerk, danach laufen die Trigger `<T>_uuid_fill` für jede Zeile.
        let store = Store::open(data.path()).await.unwrap();
        let instance = store.instance().await.unwrap();
        assert!(baruch_exchange::is_uuid(&instance.uuid), "Instanzkennung aus der Migration: {}", instance.uuid);
        assert_eq!(instance.name, "", "Name der Instanz ist anfangs leer");
        let (work, slug, title) = store.works().await.unwrap().remove(0);
        let _ = slug;
        let uuid = store.work_uuid(work).await.unwrap();
        assert!(baruch_exchange::is_uuid(&uuid), "Werk-UUID kanonisch: {uuid}");
        assert_eq!(store.work_by_uuid(&uuid).await.unwrap(), Some(work), "Werk über seine UUID");
        // Jede Zeile jeder Tabelle mit Werkinhalt hat eine UUID (Fülltrigger).
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
        // Löschen des ganzen Werks hinterlässt Grabsteine für Werk und Punkte (Trigger `<T>_uuid_tombstone`).
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
