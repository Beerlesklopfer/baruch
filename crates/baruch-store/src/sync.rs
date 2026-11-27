// SPDX-License-Identifier: AGPL-3.0-or-later
//! Abgleich Baruch ↔ MultiGPT (Migration 0033, Runde 13 „abgleich“): der Schalter „nach MultiGPT
//! hochladen“ je Werk (`sync_work`, Vorgabe aus), die gewählten Sammlungen zum Holen (`sync_collection`),
//! der zuletzt abgeglichene Stand je Kapitel bzw. Dokument (`sync_state`), die lokalen Kopien geholter
//! Dokumente (`sync_document`) und das Protokoll der Läufe (`sync_run`).
//!
//! Der Store hält nur die Werte; wer was darf (nur der Mensch schaltet und löst aus), prüft der Server.
//! Kein Buchtext, darum kein Commit im Spiegel. Hashes über [`sync_hash`] (SHA-256, hex).

use sha2::{Digest, Sha256};
use sqlx::Row;

use crate::{Store, StoreError, WorkId};

/// Höchstens so viele Zeichen hat der Name einer Sammlung.
pub const SYNC_COLLECTION_CHARS: usize = 200;
/// Höchstens so viele Sammlungen lassen sich zum Holen wählen.
pub const SYNC_MAX_COLLECTIONS: usize = 100;

/// SHA-256 eines Texts als Hex (64 Zeichen): Inhalts-Hash des Abgleichs.
pub fn sync_hash(text: &str) -> String {
    Sha256::digest(text.as_bytes()).iter().map(|b| format!("{b:02x}")).collect()
}

/// Schalter eines Werks (fehlt die Zeile: aus, ohne Sammlung).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct SyncWork {
    pub work: WorkId,
    pub title: String,
    pub upload: bool,
    /// Beim Einschalten festgehaltene Sammlung („Baruch: <Werktitel>“), sonst leer.
    pub collection: String,
    /// Zeitpunkt der Bestätigung beim (letzten) Einschalten.
    pub confirmed: Option<String>,
    pub updated: Option<String>,
}

/// Seite eines Eintrags in `sync_state`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncSide {
    /// Kapitel eines Werks (item `work:<W>/key:<K>`).
    Baruch,
    /// Dokument einer gewählten Sammlung (item = Dokument-ID).
    Multigpt,
}

impl SyncSide {
    pub fn as_str(self) -> &'static str {
        match self {
            SyncSide::Baruch => "baruch",
            SyncSide::Multigpt => "multigpt",
        }
    }

    fn parse(text: &str) -> Result<SyncSide, StoreError> {
        match text {
            "baruch" => Ok(SyncSide::Baruch),
            "multigpt" => Ok(SyncSide::Multigpt),
            other => Err(crate::broken(format!("sync_state: unbekannte Seite {other:?}"))),
        }
    }
}

/// Zuletzt abgeglichener Stand eines Kapitels bzw. Dokuments.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct SyncState {
    pub side: SyncSide,
    pub collection: String,
    pub item: String,
    pub work: Option<WorkId>,
    pub node_key: Option<i64>,
    /// Dokument-ID bei MultiGPT (Seite `baruch`: das hochgeladene Dokument).
    pub remote_id: String,
    pub title: String,
    pub hash: String,
    /// Änderungszeit bzw. Hash, wie MultiGPT ihn in `list_documents` meldet (leer = nicht gemeldet).
    pub remote_stamp: String,
    /// Beim Schreiben setzt der Store die Zeit selbst.
    pub synced: String,
}

/// Lokale Kopie eines geholten Dokuments.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct SyncDocument {
    pub id: i64,
    pub collection: String,
    pub document_id: String,
    pub title: String,
    pub url: String,
    pub text: String,
    pub hash: String,
    pub fetched: String,
}

/// Zahlen eines Laufs.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize)]
pub struct SyncCounts {
    pub uploaded: i64,
    pub replaced: i64,
    pub removed: i64,
    pub downloaded: i64,
    pub unchanged: i64,
    pub failed: i64,
}

/// Ein Lauf (`finished`/`ok` fehlen, solange er läuft oder wenn er abbrach).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct SyncRun {
    pub id: i64,
    pub started: String,
    pub finished: Option<String>,
    pub ok: Option<bool>,
    pub message: String,
    #[serde(flatten)]
    pub counts: SyncCounts,
}

/// Zahl und letzter Abgleich (Kapitel eines Werks bzw. Dokumente einer Sammlung).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct SyncTally {
    pub documents: i64,
    pub synced: Option<String>,
}

/// Name einer Sammlung: beschnitten, 1..=200 Zeichen in einer Zeile.
fn clean_collection(name: &str) -> Result<String, StoreError> {
    let name = name.trim();
    let chars = name.chars().count();
    if chars == 0 || chars > SYNC_COLLECTION_CHARS || name.contains(['\n', '\r']) {
        return Err(StoreError::Refused("Sammlung: 1 bis 200 Zeichen in einer Zeile."));
    }
    Ok(name.to_owned())
}

fn state_of(row: &sqlx::sqlite::SqliteRow) -> Result<SyncState, StoreError> {
    Ok(SyncState {
        side: SyncSide::parse(&row.try_get::<String, _>("side")?)?,
        collection: row.try_get("collection")?,
        item: row.try_get("item")?,
        work: row.try_get("work_id")?,
        node_key: row.try_get("node_key")?,
        remote_id: row.try_get("remote_id")?,
        title: row.try_get("title")?,
        hash: row.try_get("hash")?,
        remote_stamp: row.try_get("remote_stamp")?,
        synced: row.try_get("synced")?,
    })
}

fn document_of(row: &sqlx::sqlite::SqliteRow) -> Result<SyncDocument, StoreError> {
    Ok(SyncDocument {
        id: row.try_get("id")?,
        collection: row.try_get("collection")?,
        document_id: row.try_get("document_id")?,
        title: row.try_get("title")?,
        url: row.try_get("url")?,
        text: row.try_get("text")?,
        hash: row.try_get("hash")?,
        fetched: row.try_get("fetched")?,
    })
}

fn run_of(row: &sqlx::sqlite::SqliteRow) -> Result<SyncRun, StoreError> {
    let ok: Option<i64> = row.try_get("ok")?;
    Ok(SyncRun {
        id: row.try_get("id")?,
        started: row.try_get("started")?,
        finished: row.try_get("finished")?,
        ok: ok.map(|v| v == 1),
        message: row.try_get("message")?,
        counts: SyncCounts {
            uploaded: row.try_get("uploaded")?,
            replaced: row.try_get("replaced")?,
            removed: row.try_get("removed")?,
            downloaded: row.try_get("downloaded")?,
            unchanged: row.try_get("unchanged")?,
            failed: row.try_get("failed")?,
        },
    })
}

const WORK_SELECT: &str = "SELECT w.id AS id, w.title AS title, coalesce(s.upload, 0) AS upload, \
     coalesce(s.collection, '') AS collection, s.confirmed AS confirmed, s.updated AS updated \
     FROM work w LEFT JOIN sync_work s ON s.work_id = w.id";

fn work_of(row: &sqlx::sqlite::SqliteRow) -> Result<SyncWork, StoreError> {
    let upload: i64 = row.try_get("upload")?;
    Ok(SyncWork {
        work: row.try_get("id")?,
        title: row.try_get("title")?,
        upload: upload == 1,
        collection: row.try_get("collection")?,
        confirmed: row.try_get("confirmed")?,
        updated: row.try_get("updated")?,
    })
}

const STATE_COLUMNS: &str = "side, collection, item, work_id, node_key, remote_id, title, hash, remote_stamp, synced";
const DOCUMENT_COLUMNS: &str = "id, collection, document_id, title, url, text, hash, fetched";

impl Store {
    /// Schalter eines Werks; unbekanntes Werk → `UnknownWork`.
    pub async fn sync_work(&self, work: WorkId) -> Result<SyncWork, StoreError> {
        let row = sqlx::query(sqlx::AssertSqlSafe(format!("{WORK_SELECT} WHERE w.id = ?")))
            .bind(work)
            .fetch_optional(&self.pool)
            .await?
            .ok_or(StoreError::UnknownWork(work))?;
        work_of(&row)
    }

    /// Schalter aller Werke (nach id).
    pub async fn sync_works(&self) -> Result<Vec<SyncWork>, StoreError> {
        let rows =
            sqlx::query(sqlx::AssertSqlSafe(format!("{WORK_SELECT} ORDER BY w.id"))).fetch_all(&self.pool).await?;
        rows.iter().map(work_of).collect()
    }

    /// Schalter „nach MultiGPT hochladen“ setzen (eine Transaktion). Beim Einschalten wird die Sammlung
    /// festgehalten: eine schon festgehaltene bleibt (sonst verlören hochgeladene Dokumente ihren Ort),
    /// sonst gilt `collection` (1..=200 Zeichen, eine Zeile); `confirmed` = jetzt. Ausschalten behält die
    /// Sammlung. Unbekanntes Werk → `UnknownWork`, ungültige Sammlung → `Refused` (nichts geändert).
    pub async fn sync_set_upload(&self, work: WorkId, on: bool, collection: &str) -> Result<SyncWork, StoreError> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let found: Option<i64> =
            sqlx::query_scalar("SELECT id FROM work WHERE id = ?").bind(work).fetch_optional(&mut *tx).await?;
        found.ok_or(StoreError::UnknownWork(work))?;
        let kept: Option<String> = sqlx::query_scalar("SELECT collection FROM sync_work WHERE work_id = ?")
            .bind(work)
            .fetch_optional(&mut *tx)
            .await?;
        let kept = kept.unwrap_or_default();
        if on {
            let chosen = if kept.is_empty() { clean_collection(collection)? } else { kept };
            sqlx::query(
                "INSERT INTO sync_work (work_id, upload, collection, confirmed) VALUES (?, 1, ?, \
                 strftime('%Y-%m-%dT%H:%M:%fZ','now')) ON CONFLICT(work_id) DO UPDATE SET upload = 1, \
                 collection = excluded.collection, confirmed = excluded.confirmed, \
                 updated = strftime('%Y-%m-%dT%H:%M:%fZ','now')",
            )
            .bind(work)
            .bind(&chosen)
            .execute(&mut *tx)
            .await?;
        } else {
            sqlx::query(
                "INSERT INTO sync_work (work_id, upload, collection) VALUES (?, 0, ?) ON CONFLICT(work_id) DO UPDATE \
                 SET upload = 0, updated = strftime('%Y-%m-%dT%H:%M:%fZ','now')",
            )
            .bind(work)
            .bind(&kept)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        self.sync_work(work).await
    }

    /// Zum Holen gewählte Sammlungen in ihrer Reihenfolge.
    pub async fn sync_collections(&self) -> Result<Vec<String>, StoreError> {
        Ok(sqlx::query_scalar("SELECT collection FROM sync_collection ORDER BY position, collection")
            .fetch_all(&self.pool)
            .await?)
    }

    /// Gewählte Sammlungen ersetzen: beschnitten, Doppelte entfernt (die erste gewinnt), je 1..=200
    /// Zeichen in einer Zeile, höchstens 100; sonst `Refused` und nichts geändert. Liefert die Liste.
    pub async fn sync_set_collections(&self, collections: &[String]) -> Result<Vec<String>, StoreError> {
        let mut list: Vec<String> = Vec::new();
        for name in collections {
            let name = clean_collection(name)?;
            if !list.contains(&name) {
                list.push(name);
            }
        }
        if list.len() > SYNC_MAX_COLLECTIONS {
            return Err(StoreError::Refused("Höchstens 100 Sammlungen zum Holen."));
        }
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let before: Vec<(String, String)> =
            sqlx::query_as("SELECT collection, added FROM sync_collection").fetch_all(&mut *tx).await?;
        sqlx::query("DELETE FROM sync_collection").execute(&mut *tx).await?;
        for (index, name) in list.iter().enumerate() {
            // Eine schon gewählte Sammlung behält ihren Zeitpunkt.
            let added = before.iter().find(|(c, _)| c == name).map(|(_, a)| a.clone());
            sqlx::query(
                "INSERT INTO sync_collection (collection, position, added) VALUES (?, ?, \
                 coalesce(?, strftime('%Y-%m-%dT%H:%M:%fZ','now')))",
            )
            .bind(name)
            .bind(index as i64 + 1)
            .bind(added)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(list)
    }

    /// Stände einer Seite, wahlweise nur einer Sammlung (nach Sammlung und item).
    pub async fn sync_states(&self, side: SyncSide, collection: Option<&str>) -> Result<Vec<SyncState>, StoreError> {
        let rows = match collection {
            Some(collection) => {
                sqlx::query(sqlx::AssertSqlSafe(format!(
                    "SELECT {STATE_COLUMNS} FROM sync_state WHERE side = ? AND collection = ? ORDER BY item"
                )))
                .bind(side.as_str())
                .bind(collection)
                .fetch_all(&self.pool)
                .await?
            }
            None => {
                sqlx::query(sqlx::AssertSqlSafe(format!(
                    "SELECT {STATE_COLUMNS} FROM sync_state WHERE side = ? ORDER BY collection, item"
                )))
                .bind(side.as_str())
                .fetch_all(&self.pool)
                .await?
            }
        };
        rows.iter().map(state_of).collect()
    }

    /// Stand schreiben (ersetzt einen vorhandenen; `synced` = jetzt).
    pub async fn sync_state_put(&self, state: &SyncState) -> Result<(), StoreError> {
        sqlx::query(
            "INSERT INTO sync_state (side, collection, item, work_id, node_key, remote_id, title, hash, remote_stamp) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?) ON CONFLICT(side, collection, item) DO UPDATE SET \
             work_id = excluded.work_id, node_key = excluded.node_key, remote_id = excluded.remote_id, \
             title = excluded.title, hash = excluded.hash, remote_stamp = excluded.remote_stamp, \
             synced = strftime('%Y-%m-%dT%H:%M:%fZ','now')",
        )
        .bind(state.side.as_str())
        .bind(&state.collection)
        .bind(&state.item)
        .bind(state.work)
        .bind(state.node_key)
        .bind(&state.remote_id)
        .bind(&state.title)
        .bind(&state.hash)
        .bind(&state.remote_stamp)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Stand entfernen (fehlt er, geschieht nichts).
    pub async fn sync_state_delete(&self, side: SyncSide, collection: &str, item: &str) -> Result<(), StoreError> {
        sqlx::query("DELETE FROM sync_state WHERE side = ? AND collection = ? AND item = ?")
            .bind(side.as_str())
            .bind(collection)
            .bind(item)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Lokale Kopie eines Dokuments.
    pub async fn sync_document(&self, collection: &str, document_id: &str) -> Result<Option<SyncDocument>, StoreError> {
        let row = sqlx::query(sqlx::AssertSqlSafe(format!(
            "SELECT {DOCUMENT_COLUMNS} FROM sync_document WHERE collection = ? AND document_id = ?"
        )))
        .bind(collection)
        .bind(document_id)
        .fetch_optional(&self.pool)
        .await?;
        row.as_ref().map(document_of).transpose()
    }

    /// Alle lokalen Kopien (nach id), für den lokalen Index.
    pub async fn sync_documents(&self) -> Result<Vec<SyncDocument>, StoreError> {
        let rows =
            sqlx::query(sqlx::AssertSqlSafe(format!("SELECT {DOCUMENT_COLUMNS} FROM sync_document ORDER BY id")))
                .fetch_all(&self.pool)
                .await?;
        rows.iter().map(document_of).collect()
    }

    /// Sammlungen mit lokalen Kopien, je mit den Dokument-IDs.
    pub async fn sync_document_ids(&self) -> Result<Vec<(String, String)>, StoreError> {
        Ok(sqlx::query_as("SELECT collection, document_id FROM sync_document ORDER BY collection, document_id")
            .fetch_all(&self.pool)
            .await?)
    }

    /// Lokale Kopie schreiben. Gleicher Hash wie gespeichert → nichts ändert sich (`false`); sonst neuer
    /// Text, Titel, Adresse und Abrufzeit bei gleicher id (`true`). Liefert (id, geändert).
    pub async fn sync_document_put(
        &self,
        collection: &str,
        document_id: &str,
        title: &str,
        url: &str,
        text: &str,
    ) -> Result<(i64, bool), StoreError> {
        let hash = sync_hash(text);
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let known: Option<(i64, String, String, String)> =
            sqlx::query_as("SELECT id, hash, title, url FROM sync_document WHERE collection = ? AND document_id = ?")
                .bind(collection)
                .bind(document_id)
                .fetch_optional(&mut *tx)
                .await?;
        let outcome = match known {
            Some((id, old, old_title, old_url)) if old == hash && old_title == title && old_url == url => (id, false),
            Some((id, ..)) => {
                sqlx::query(
                    "UPDATE sync_document SET title = ?, url = ?, text = ?, hash = ?, \
                     fetched = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ?",
                )
                .bind(title)
                .bind(url)
                .bind(text)
                .bind(&hash)
                .bind(id)
                .execute(&mut *tx)
                .await?;
                (id, true)
            }
            None => {
                let id: i64 = sqlx::query_scalar(
                    "INSERT INTO sync_document (collection, document_id, title, url, text, hash) VALUES (?, ?, ?, ?, ?, ?) \
                     RETURNING id",
                )
                .bind(collection)
                .bind(document_id)
                .bind(title)
                .bind(url)
                .bind(text)
                .bind(&hash)
                .fetch_one(&mut *tx)
                .await?;
                (id, true)
            }
        };
        tx.commit().await?;
        Ok(outcome)
    }

    /// Lokale Kopie entfernen (fehlt sie, geschieht nichts).
    pub async fn sync_document_delete(&self, collection: &str, document_id: &str) -> Result<(), StoreError> {
        sqlx::query("DELETE FROM sync_document WHERE collection = ? AND document_id = ?")
            .bind(collection)
            .bind(document_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Zahl der hochgeladenen Kapitel und letzter Abgleich eines Werks.
    pub async fn sync_work_tally(&self, work: WorkId) -> Result<SyncTally, StoreError> {
        let (documents, synced): (i64, Option<String>) =
            sqlx::query_as("SELECT count(*), max(synced) FROM sync_state WHERE side = 'baruch' AND work_id = ?")
                .bind(work)
                .fetch_one(&self.pool)
                .await?;
        Ok(SyncTally { documents, synced })
    }

    /// Zahl der lokalen Kopien und letzter Abruf einer Sammlung.
    pub async fn sync_collection_tally(&self, collection: &str) -> Result<SyncTally, StoreError> {
        let (documents, synced): (i64, Option<String>) =
            sqlx::query_as("SELECT count(*), max(fetched) FROM sync_document WHERE collection = ?")
                .bind(collection)
                .fetch_one(&self.pool)
                .await?;
        Ok(SyncTally { documents, synced })
    }

    /// Neuer Lauf (Beginn = jetzt); liefert die id.
    pub async fn sync_run_start(&self) -> Result<i64, StoreError> {
        Ok(sqlx::query_scalar(
            "INSERT INTO sync_run (started) VALUES (strftime('%Y-%m-%dT%H:%M:%fZ','now')) RETURNING id",
        )
        .fetch_one(&self.pool)
        .await?)
    }

    /// Lauf abschließen (Ende = jetzt).
    pub async fn sync_run_finish(
        &self,
        id: i64,
        ok: bool,
        message: &str,
        counts: &SyncCounts,
    ) -> Result<(), StoreError> {
        sqlx::query(
            "UPDATE sync_run SET finished = strftime('%Y-%m-%dT%H:%M:%fZ','now'), ok = ?, message = ?, uploaded = ?, \
             replaced = ?, removed = ?, downloaded = ?, unchanged = ?, failed = ? WHERE id = ?",
        )
        .bind(i64::from(ok))
        .bind(message)
        .bind(counts.uploaded)
        .bind(counts.replaced)
        .bind(counts.removed)
        .bind(counts.downloaded)
        .bind(counts.unchanged)
        .bind(counts.failed)
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Jüngster Lauf.
    pub async fn sync_run_last(&self) -> Result<Option<SyncRun>, StoreError> {
        let row = sqlx::query(
            "SELECT id, started, finished, ok, message, uploaded, replaced, removed, downloaded, unchanged, failed \
             FROM sync_run ORDER BY id DESC LIMIT 1",
        )
        .fetch_optional(&self.pool)
        .await?;
        row.as_ref().map(run_of).transpose()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sync_hash_is_full_sha256_hex() {
        let hash = sync_hash("Leuchtturm");
        assert_eq!(hash.len(), 64, "SHA-256 als Hex hat 64 Zeichen: {hash}");
        assert_eq!(hash, sync_hash("Leuchtturm"), "gleicher Text, gleicher Hash");
        assert_ne!(hash, sync_hash("Leuchtturm."), "ein Zeichen mehr ändert den Hash");
        assert_eq!(
            sync_hash(""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
            "bekannter SHA-256 des leeren Texts"
        );
    }

    #[test]
    fn collection_names_are_one_line_with_at_most_200_characters() {
        assert_eq!(clean_collection("  Baruch: Gartenbuch ").unwrap(), "Baruch: Gartenbuch", "beschnitten");
        assert!(clean_collection("").is_err(), "leer abgelehnt");
        assert!(clean_collection("a\nb").is_err(), "Zeilenumbruch abgelehnt");
        assert!(clean_collection(&"x".repeat(201)).is_err(), "201 Zeichen abgelehnt");
        assert!(clean_collection(&"ä".repeat(200)).is_ok(), "200 Zeichen (Umlaute zählen als eins) gehen");
    }
}
