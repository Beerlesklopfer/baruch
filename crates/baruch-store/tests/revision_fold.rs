// SPDX-License-Identifier: AGPL-3.0-or-later
//! Ältere Fassungen ausblenden (Nutzer 2026-11-24, Release 0.9.0; Migration 0051 `node_revision_fold`) auf echten
//! SQLite-Dateien im Temp-Ordner mit einem frei erfundenen Werk: Marke setzen, ersetzen, aufheben, Ablehnung über der
//! aktuellen Fassung, werkweit nach Datum, Kaskade mit dem Abschnitt – und 0051 auf einer Datenbank im Stand vor
//! 0.9.0 (ed9d210, Store bis 0050) verlustfrei.

use std::borrow::Cow;
use std::collections::BTreeMap;

use baruch_store::{Store, StoreError};
use sqlx::migrate::Migrator;
use sqlx::{Row, SqlitePool};

const BOOK: &str = "#set document(title: \"Die Glocke von Ostermoor\", author: \"Lene Abt\")\n\n\
= Läuten\n\nAm Sonntag läutete die Glocke über dem Moor, und die Kraniche zogen tief.\n\n\
= Stille\n\nAm Montag blieb der Turm still, nur der Wind ging durch das Schallfenster.\n";

const HUMAN: &str = "Lene";

async fn fresh() -> (tempfile::TempDir, Store, i64) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).await.unwrap();
    let imported = store.import_typst(BOOK, "glocke.typ", HUMAN).await.unwrap();
    (dir, store, imported.work)
}

async fn current_body(store: &Store, work: i64, node: usize) -> String {
    let loaded = store.load_work(work).await.unwrap();
    let n = &loaded.nodes[node];
    n.revisions[n.current_index().unwrap()].body.clone()
}

/// Neue Fassung des Menschen auf Abschnitt 0 (ersetzt die bisherige aktuelle); `word` macht den Text verschieden.
async fn new_revision(store: &Store, work: i64, word: &str) -> u32 {
    let body = current_body(store, work, 0).await.replacen("Glocke", &format!("{word} Glocke"), 1);
    store.save_human(work, 0, &body, HUMAN).await.unwrap().expect("neue Fassung").rev
}

async fn statuses(store: &Store, work: i64) -> Vec<(u32, baruch_core::RevisionStatus)> {
    let loaded = store.load_work(work).await.unwrap();
    loaded.nodes[0].revisions.iter().map(|r| (r.number, r.status)).collect()
}

#[tokio::test]
async fn the_mark_is_set_replaced_and_cleared_without_touching_any_revision() {
    let (_dir, store, work) = fresh().await;
    for word in ["alte", "neue", "helle", "dunkle"] {
        new_revision(&store, work, word).await;
    }
    // Fassungen 1–4 ersetzt, 5 aktuell; dazu ein offener Vorschlag 6.
    let proposal = current_body(&store, work, 0).await.replacen("Moor", "Hochmoor", 1);
    let six = store.add_proposal(work, 0, &proposal, "@lektor", "probe", "Hochmoor").await.unwrap();
    assert_eq!(six, 6);
    let before = statuses(&store, work).await;
    let key = store.load_work(work).await.unwrap().nodes[0].key;

    assert_eq!(store.revision_fold(work, 0).await.unwrap(), None, "ohne Marke nichts ausgeblendet");
    store.set_revision_fold(work, 0, 3, HUMAN).await.unwrap();
    let fold = store.revision_fold(work, 0).await.unwrap().expect("Marke gespeichert");
    assert_eq!((fold.below, fold.updated_by.as_str()), (3, HUMAN), "Nummer und Mensch protokolliert: {fold:?}");
    assert!(fold.updated.ends_with('Z'), "Zeitpunkt in UTC wie revision.created: {}", fold.updated);
    assert_eq!(store.work_revision_folds(work).await.unwrap().get(&key), Some(&3), "werkweit nach Schlüssel");

    // Ersetzen: eine Zeile je Abschnitt, neue Nummer.
    store.set_revision_fold(work, 0, 5, HUMAN).await.unwrap();
    assert_eq!(store.revision_fold(work, 0).await.unwrap().unwrap().below, 5, "Marke ersetzt");
    assert_eq!(store.work_revision_folds(work).await.unwrap().len(), 1, "nur eine Marke je Abschnitt");

    // Über der aktuellen Fassung (Vorschlag 6) abgelehnt, damit die aktuelle nie verschwindet; nichts geändert.
    let refused = store.set_revision_fold(work, 0, 6, HUMAN).await.unwrap_err();
    assert!(matches!(refused, StoreError::Refused(m) if m.contains("aktuellen Fassung")), "{refused}");
    assert_eq!(store.revision_fold(work, 0).await.unwrap().unwrap().below, 5, "abgelehnt: Marke bleibt");
    // Unbekannte Fassung wie bei den übrigen Revisionsmethoden.
    let unknown = store.set_revision_fold(work, 0, 99, HUMAN).await.unwrap_err();
    assert!(matches!(unknown, StoreError::UnknownRevision { rev: 99, .. }), "{unknown}");

    assert!(store.clear_revision_fold(work, 0).await.unwrap(), "aufgehoben");
    assert!(!store.clear_revision_fold(work, 0).await.unwrap(), "zweites Aufheben meldet: keine Marke");
    assert_eq!(store.revision_fold(work, 0).await.unwrap(), None);
    // Nichts gelöscht, kein Status verändert (die Marke ist eine reine Ansichtsmarke).
    assert_eq!(statuses(&store, work).await, before, "Fassungen und Status unverändert");
}

#[tokio::test]
async fn folding_by_date_sets_each_section_to_its_last_revision_before_the_day_at_most_the_current() {
    let (_dir, store, work) = fresh().await;
    new_revision(&store, work, "alte").await;
    new_revision(&store, work, "neue").await;
    // Nach dem Tag (weit in der Zukunft): Marke auf die letzte Fassung, das ist die aktuelle 3; Abschnitt 1 hat nur
    // Fassung 1 (aktuell).
    let done = store.fold_revisions_before(work, "2999-01-01T00:00:00.000Z", HUMAN).await.unwrap();
    let loaded = store.load_work(work).await.unwrap();
    let (k0, k1) = (loaded.nodes[0].key, loaded.nodes[1].key);
    assert_eq!(done.set, vec![(k0, 3), (k1, 1)], "je Abschnitt die letzte Fassung vor dem Tag");
    // Ein Vorschlag über der aktuellen zählt mit, die Marke bleibt aber höchstens auf der aktuellen.
    let proposal = current_body(&store, work, 0).await.replacen("Moor", "Hochmoor", 1);
    store.add_proposal(work, 0, &proposal, "@lektor", "probe", "Hochmoor").await.unwrap();
    let done = store.fold_revisions_before(work, "2999-01-01T00:00:00.000Z", HUMAN).await.unwrap();
    assert_eq!(done.set[0], (k0, 3), "Vorschlag 4 liegt über der aktuellen Fassung 3: Marke höchstens dort");
    // Vor allen Fassungen: nichts zu setzen, vorhandene Marken bleiben.
    let none = store.fold_revisions_before(work, "2000-01-01T00:00:00.000Z", HUMAN).await.unwrap();
    assert!(none.set.is_empty(), "keine Fassung vor 2000: {none:?}");
    assert_eq!(store.revision_fold(work, 0).await.unwrap().unwrap().below, 3, "unverändert");
    assert_eq!(store.clear_work_revision_folds(work).await.unwrap(), 2, "beide Marken aufgehoben");
    assert!(store.work_revision_folds(work).await.unwrap().is_empty());
    assert!(matches!(store.clear_work_revision_folds(9999).await, Err(StoreError::UnknownWork(9999))));
}

#[tokio::test]
async fn the_mark_goes_with_its_section_and_its_work() {
    let (_dir, store, work) = fresh().await;
    new_revision(&store, work, "alte").await;
    store.set_revision_fold(work, 0, 2, HUMAN).await.unwrap();
    let rows: i64 =
        sqlx::query_scalar("SELECT count(*) FROM node_revision_fold").fetch_one(store.pool()).await.unwrap();
    assert_eq!(rows, 1);
    // Werk löschen: Abschnitte gehen, die Marke per ON DELETE CASCADE mit (Fremdschlüssel sind an).
    let title = store.load_work(work).await.unwrap().title;
    store.delete_work(work, &title).await.unwrap();
    let rows: i64 =
        sqlx::query_scalar("SELECT count(*) FROM node_revision_fold").fetch_one(store.pool()).await.unwrap();
    assert_eq!(rows, 0, "Marke geht mit dem Abschnitt");
}

fn store_migrator(keep: impl Fn(i64) -> bool) -> Migrator {
    let list: Vec<_> = baruch_store::MIGRATOR.iter().filter(|m| keep(m.version)).cloned().collect();
    Migrator { migrations: Cow::Owned(list), ignore_missing: true, ..Migrator::DEFAULT }
}

/// Inhalt aller Tabellen (außer `_sqlx_migrations`), je Zeile alle Spalten per `quote()`.
async fn contents(pool: &SqlitePool) -> BTreeMap<String, Vec<String>> {
    let names: Vec<String> = sqlx::query_scalar(
        "SELECT name FROM sqlite_master WHERE type = 'table' AND sql NOT LIKE 'CREATE VIRTUAL%' \
         AND name <> '_sqlx_migrations' ORDER BY name",
    )
    .fetch_all(pool)
    .await
    .unwrap();
    let mut out = BTreeMap::new();
    for table in names {
        let cols: Vec<String> = sqlx::query_scalar("SELECT name FROM pragma_table_info(?) ORDER BY cid")
            .bind(&table)
            .fetch_all(pool)
            .await
            .unwrap();
        let select = cols.iter().map(|c| format!("quote(\"{c}\")")).collect::<Vec<_>>().join(" || char(31) || ");
        let mut rows: Vec<String> =
            sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT {select} FROM \"{table}\"")))
                .fetch_all(pool)
                .await
                .unwrap();
        rows.sort();
        out.insert(table, rows);
    }
    out
}

async fn schema(pool: &SqlitePool) -> BTreeMap<String, Option<String>> {
    let rows = sqlx::query("SELECT name, sql FROM sqlite_master ORDER BY name").fetch_all(pool).await.unwrap();
    rows.into_iter().map(|r| (r.get(0), r.get(1))).collect()
}

#[tokio::test]
async fn migration_0051_on_a_database_from_before_0_9_0_keeps_every_row_and_adds_only_the_fold_table() {
    // Stand ed9d210 nachbauen: eine echte Datenbank über die Store-API füllen (Werk, ersetzte Fassungen, Vorschlag,
    // Beitrag) und 0051 wieder herausnehmen. 0051 legt nur die Tabelle an, darum ist das genau der Stand mit Store bis
    // 0050 (geprüft: danach fehlen Tabelle und Eintrag in _sqlx_migrations, höchste Version 0050).
    let (dir, store, work) = fresh().await;
    new_revision(&store, work, "alte").await;
    new_revision(&store, work, "neue").await;
    let proposal = current_body(&store, work, 0).await.replacen("Moor", "Hochmoor", 1);
    store.add_proposal(work, 0, &proposal, "@lektor", "probe", "Hochmoor").await.unwrap();
    let pool = store.pool().clone();
    sqlx::query("DROP TABLE node_revision_fold").execute(&pool).await.unwrap();
    sqlx::query("DELETE FROM _sqlx_migrations WHERE version = 51").execute(&pool).await.unwrap();
    // Runde 34: 0052 (Spalten an `idea`) liegt danach und bleibt eingespielt; sie berührt die Fassungen nicht.
    let highest: i64 = sqlx::query_scalar("SELECT max(version) FROM _sqlx_migrations WHERE version < 52")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(highest, 50, "Stand davor: Store bis 0050 (ed9d210)");
    assert!(!baruch_store::FOREIGN_KEYS_OFF.contains(&51), "0051 legt nur eine Tabelle an, Fremdschlüssel bleiben an");
    let before = contents(&pool).await;
    let schema_before = schema(&pool).await;
    assert!(
        before["revision"].len() >= 5,
        "Fassungen vorhanden (sonst prüfte der Test wenig): {}",
        before["revision"].len()
    );

    store_migrator(|v| v == 51).run(&pool).await.expect("0051 läuft auf dem Stand vor 0.9.0");

    let after = contents(&pool).await;
    for (table, rows) in &before {
        assert_eq!(after.get(table), Some(rows), "Tabelle {table}: alle Zeilen byte-gleich");
    }
    assert_eq!(after.get("node_revision_fold"), Some(&Vec::new()), "neue Tabelle leer: zunächst nichts ausgeblendet");
    let schema_after = schema(&pool).await;
    let added: Vec<&String> = schema_after.keys().filter(|k| !schema_before.contains_key(*k)).collect();
    assert_eq!(added, vec!["node_revision_fold"], "genau ein neues Schemaobjekt");
    for (name, sql) in &schema_before {
        assert_eq!(schema_after.get(name), Some(sql), "Schema von {name} unverändert");
    }
    // CHECKs: Marke ab 1, Abschnitt muss es geben (Fremdschlüssel an).
    let node: i64 = sqlx::query_scalar("SELECT id FROM node WHERE work_id = ? AND seq = 0")
        .bind(work)
        .fetch_one(&pool)
        .await
        .unwrap();
    let zero =
        sqlx::query("INSERT INTO node_revision_fold (node_id, below) VALUES (?, 0)").bind(node).execute(&pool).await;
    assert!(zero.is_err(), "Marke 0 abgelehnt");
    let orphan = sqlx::query("INSERT INTO node_revision_fold (node_id, below) VALUES (987654, 1)").execute(&pool).await;
    assert!(orphan.is_err(), "Marke ohne Abschnitt abgelehnt");
    let integrity: String = sqlx::query_scalar("PRAGMA integrity_check").fetch_one(&pool).await.unwrap();
    assert_eq!(integrity, "ok");
    drop(store);

    // Store::open danach: Marke über die Store-API auf den alten Daten.
    let reopened = Store::open(dir.path()).await.expect("Store::open nach 0051");
    reopened.set_revision_fold(work, 0, 3, HUMAN).await.unwrap();
    assert_eq!(reopened.revision_fold(work, 0).await.unwrap().unwrap().below, 3);
}
