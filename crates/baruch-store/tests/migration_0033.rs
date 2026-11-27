// SPDX-License-Identifier: AGPL-3.0-or-later
//! Migration 0033 (sync Baruch ↔ MultiGPT, Round 13 „abgleich“) on a reproduced database in the state of development
//! (all store migrations below 0033, that is up to 0032, and library 0101–0105), with a freely invented work, revisions
//! (current revisions and a proposal), post, personas, Bible of the work and connection.
//!
//! After a migrator up to 0033: the schema of every old table stays, new are exactly `sync_work`, `sync_collection`,
//! `sync_state`, `sync_document` and `sync_run` with the columns of the contract, all empty. Afterwards `Store::open`:
//! every row of every old table unchanged; the switch is off for every work; deleting the work takes its `sync_work`
//! row along, `sync_state` (without foreign key) stays for the next sync.

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::path::Path;
use std::str::FromStr;

use baruch_store::{Store, StoreError, SyncCounts, SyncSide, SyncState, title_key};
use sqlx::SqlitePool;
use sqlx::migrate::Migrator;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

const SOURCE: &str = "#set document(title: \"Das Jahr am Leuchtturm\", author: \"Mara Lind\")\n\n\
= Frühling\n\nDas Licht kreist über dem Wasser.\n\n= Herbst\n\nDer Nebel kommt früh.\n";

async fn raw_pool(dir: &Path) -> SqlitePool {
    let options = SqliteConnectOptions::from_str("sqlite:")
        .unwrap()
        .filename(dir.join("baruch.sqlite"))
        .create_if_missing(true)
        .foreign_keys(true);
    SqlitePoolOptions::new().max_connections(1).connect_with(options).await.unwrap()
}

async fn migrate_where(pool: &SqlitePool, keep: impl Fn(i64) -> bool) {
    let list: Vec<_> = baruch_store::MIGRATOR.iter().filter(|m| keep(m.version)).cloned().collect();
    let migrator = Migrator { migrations: Cow::Owned(list), ignore_missing: true, ..Migrator::DEFAULT };
    migrator.run(pool).await.unwrap();
}

/// Store below 0033 (state of development: up to 0032) and library 0101–0105.
async fn migrate_before(pool: &SqlitePool) {
    assert!(baruch_store::MIGRATOR.iter().any(|m| m.version == 33), "die neue Migration ist 0033");
    assert!(baruch_store::MIGRATOR.iter().any(|m| m.version == 32), "Stand davor hat 0032 (development)");
    migrate_where(pool, |v| v < 33).await;
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../baruch-library/migrations");
    let mut library = Migrator::new(dir).await.unwrap();
    library.migrations = Cow::Owned(library.iter().filter(|m| m.version <= 105).cloned().collect());
    library.ignore_missing = true;
    library.run(pool).await.unwrap();
}

async fn exec(pool: &SqlitePool, sql: &str) {
    sqlx::query(sqlx::AssertSqlSafe(sql)).execute(pool).await.unwrap_or_else(|e| panic!("{sql}: {e}"));
}

async fn insert_rows(pool: &SqlitePool) -> i64 {
    let doc = baruch_typst::parse(SOURCE).unwrap();
    let work: i64 = sqlx::query_scalar(
        "INSERT INTO work (slug, kind, title, author, template, source_name, source_sha256, title_key, audience) \
         VALUES ('das-jahr-am-leuchtturm', 'book', 'Das Jahr am Leuchtturm', 'Mara Lind', ?, 'leuchtturm.typ', \
         'erfunden', ?, 'Küstenleute') RETURNING id",
    )
    .bind(&doc.template)
    .bind(title_key("Das Jahr am Leuchtturm"))
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO work_template_revision (work_id, number, body, author_name, note) VALUES (?, 1, ?, 'Autor', 'Anfang')")
        .bind(work)
        .bind(&doc.template)
        .execute(pool)
        .await
        .unwrap();
    exec(pool, "INSERT INTO store_meta (key, value) VALUES ('seeded', '1') ON CONFLICT DO NOTHING").await;
    exec(pool, "INSERT INTO store_meta (key, value) VALUES ('lector', '@lektor') ON CONFLICT DO NOTHING").await;
    exec(
        pool,
        "INSERT INTO persona (handle, name, role, model, context_length, active, position, color, prompt) VALUES \
         ('@lektor', 'Lektor', 'Lektorat', 'probe-a', 32000, 1, 0, '#1c7ed6', 'Eigener Prompt.'), \
         ('@belege', 'Belege', 'Quellen', 'probe-a', 16000, 1, 1, NULL, NULL), \
         ('@hoerer', 'Hörer', 'Zielgruppe', 'probe-a', 8000, 0, 2, NULL, NULL)",
    )
    .await;
    let mut ids = Vec::new();
    for (seq, heading) in doc.headings.iter().enumerate() {
        let id: i64 = sqlx::query_scalar(
            "INSERT INTO node (work_id, seq, parent_seq, position, number, title, kind) VALUES (?, ?, NULL, ?, ?, ?, 'chapter') RETURNING id",
        )
        .bind(work)
        .bind(seq as i64)
        .bind(seq as i64)
        .bind((seq + 1).to_string())
        .bind(&heading.title)
        .fetch_one(pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO revision (node_id, number, body, author_kind, author_name, model, status, note) \
             VALUES (?, 1, ?, 'human', 'Autor', NULL, 'current', 'Import')",
        )
        .bind(id)
        .bind(&heading.body)
        .execute(pool)
        .await
        .unwrap();
        ids.push(id);
    }
    sqlx::query(
        "INSERT INTO revision (node_id, number, body, author_kind, author_name, model, status, note, based_on) \
         VALUES (?, 2, '= Herbst\n\nDer Nebel kommt früh und bleibt.\n', 'assistant', '@lektor', 'probe-a', 'proposed', 'Bild', 1)",
    )
    .bind(ids[1])
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO post (node_id, position, author, body, sources) VALUES (?, 0, '@belege', 'Nicht belegt.', '[]')",
    )
    .bind(ids[0])
    .execute(pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO work_bible (work_id, module) VALUES (?, 'Erfunden')")
        .bind(work)
        .execute(pool)
        .await
        .unwrap();
    exec(pool, "INSERT INTO connection (kind, config) VALUES ('multigpt', '{\"url\":\"http://127.0.0.1:4321/mcp\"}')")
        .await;
    work
}

async fn tables(pool: &SqlitePool) -> BTreeMap<String, Vec<String>> {
    let names: Vec<String> = sqlx::query_scalar("SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name")
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
        out.insert(table, cols);
    }
    out
}

async fn schema(pool: &SqlitePool) -> BTreeMap<String, Option<String>> {
    let rows: Vec<(String, Option<String>)> =
        sqlx::query_as("SELECT name, sql FROM sqlite_master WHERE name NOT LIKE 'sqlite_%' ORDER BY name")
            .fetch_all(pool)
            .await
            .unwrap();
    rows.into_iter().collect()
}

async fn contents(pool: &SqlitePool, layout: &BTreeMap<String, Vec<String>>) -> BTreeMap<String, Vec<String>> {
    let mut out = BTreeMap::new();
    for (table, cols) in layout {
        let select = cols.iter().map(|c| format!("quote(\"{c}\")")).collect::<Vec<_>>().join(" || char(31) || ");
        let mut rows: Vec<String> =
            sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT {select} FROM \"{table}\"")))
                .fetch_all(pool)
                .await
                .unwrap();
        rows.sort();
        out.insert(table.clone(), rows);
    }
    out
}

/// Columns of the new tables, literally according to contract §3.
fn expected_columns() -> BTreeMap<&'static str, Vec<&'static str>> {
    BTreeMap::from([
        ("sync_work", vec!["work_id", "upload", "collection", "confirmed", "updated"]),
        ("sync_collection", vec!["collection", "position", "added"]),
        (
            "sync_state",
            vec![
                "side",
                "collection",
                "item",
                "work_id",
                "node_key",
                "remote_id",
                "title",
                "hash",
                "remote_stamp",
                "synced",
            ],
        ),
        ("sync_document", vec!["id", "collection", "document_id", "title", "url", "text", "hash", "fetched"]),
        (
            "sync_run",
            vec![
                "id",
                "started",
                "finished",
                "ok",
                "message",
                "uploaded",
                "replaced",
                "removed",
                "downloaded",
                "unchanged",
                "failed",
            ],
        ),
    ])
}

#[tokio::test]
async fn migration_0033_keeps_every_row_and_only_adds_the_five_sync_tables() {
    let dir = tempfile::tempdir().unwrap();
    let pool = raw_pool(dir.path()).await;
    migrate_before(&pool).await;
    let work = insert_rows(&pool).await;
    let layout = tables(&pool).await;
    let schema_before = schema(&pool).await;
    let before = contents(&pool, &layout).await;
    assert_eq!(before["revision"].len(), 3, "zwei aktuelle Fassungen und ein Vorschlag vor der Migration");
    assert_eq!(before["work"].len(), 1, "ein Werk vor der Migration");

    migrate_where(&pool, |v| v <= 33).await;
    let schema_after = schema(&pool).await;
    for (name, sql) in &schema_before {
        assert_eq!(schema_after.get(name), Some(sql), "Schema von {name} unverändert (0033 ändert keine alte Tabelle)");
    }
    let added: Vec<&str> =
        schema_after.keys().filter(|n| !schema_before.contains_key(*n)).map(String::as_str).collect();
    assert_eq!(
        added,
        vec!["sync_collection", "sync_document", "sync_run", "sync_state", "sync_work",],
        "genau die fünf neuen Tabellen (ohne zusätzliche Indizes oder Trigger)"
    );
    let now = tables(&pool).await;
    for (table, columns) in expected_columns() {
        assert_eq!(
            now[table].iter().map(String::as_str).collect::<Vec<_>>(),
            columns,
            "{table}: genau die Spalten aus Vertrag §3 in dieser Reihenfolge"
        );
        let rows: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT count(*) FROM {table}")))
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(rows, 0, "{table}: keine Datenänderung, die Tabelle ist leer");
    }
    for (sql, why) in [
        ("INSERT INTO sync_work (work_id, upload) VALUES ((SELECT id FROM work), 2)", "upload nur 0 oder 1"),
        ("INSERT INTO sync_work (work_id) VALUES (99999)", "sync_work nur für vorhandene Werke (Fremdschlüssel)"),
        ("INSERT INTO sync_collection (collection, position) VALUES ('', 1)", "leere Sammlung abgelehnt"),
        ("INSERT INTO sync_collection (collection, position) VALUES ('a' || char(10) || 'b', 1)", "kein Zeilenumbruch"),
        ("INSERT INTO sync_collection (collection, position) VALUES ('Bibeln', 0)", "position ab 1"),
        (
            "INSERT INTO sync_state (side, collection, item, hash) VALUES ('fremd', 'c', 'i', 'h')",
            "side nur baruch|multigpt",
        ),
        ("INSERT INTO sync_run (started, ok) VALUES ('x', 3)", "ok nur 0 oder 1"),
    ] {
        assert!(sqlx::query(sql).execute(&pool).await.is_err(), "{why}: {sql}");
    }
    let refused = sqlx::query("INSERT INTO sync_collection (collection, position) VALUES (?, 1)")
        .bind("ä".repeat(201))
        .execute(&pool)
        .await;
    assert!(refused.is_err(), "201 Zeichen (length() zählt Zeichen, nicht Bytes) abgelehnt");
    let fine = sqlx::query("INSERT INTO sync_collection (collection, position) VALUES (?, 1)")
        .bind("ä".repeat(200))
        .execute(&pool)
        .await;
    assert!(fine.is_ok(), "200 Umlaute gehen: {fine:?}");
    exec(&pool, "DELETE FROM sync_collection").await;
    // Without foreign key: a state may name a (deleted) work that does not exist.
    exec(
        &pool,
        "INSERT INTO sync_state (side, collection, item, work_id, hash) VALUES ('baruch', 'c', 'work:99999/key:1', \
         99999, 'h')",
    )
    .await;
    exec(&pool, "DELETE FROM sync_state").await;
    pool.close().await;

    let store = Store::open(dir.path()).await.expect("Store::open nach 0033");
    let after = without_instance_row(contents(store.pool(), &layout).await);
    for (table, rows) in &before {
        if table == "_sqlx_migrations" {
            continue;
        }
        assert_eq!(after.get(table), Some(rows), "Tabelle {table}: alle Zeilen unverändert");
    }
    assert_eq!(
        baruch_typst::xref::strip_key_labels(&store.export_typst(work).await.unwrap()).into_owned(),
        SOURCE,
        "Export byte-gleich wie vor der Migration"
    );
    let switch = store.sync_work(work).await.unwrap();
    assert!(!switch.upload && switch.collection.is_empty(), "Vorgabe: Hochladen aus, keine Sammlung: {switch:?}");
    assert!(store.sync_run_last().await.unwrap().is_none(), "noch kein Lauf");
    assert!(store.sync_collections().await.unwrap().is_empty(), "keine Sammlung gewählt");

    // Switching on fixes the collection; a second switching on with a different name keeps it.
    let on = store.sync_set_upload(work, true, "Baruch: Das Jahr am Leuchtturm").await.unwrap();
    assert!(on.upload && on.confirmed.is_some(), "eingeschaltet mit Zeitpunkt der Bestätigung: {on:?}");
    assert_eq!(on.collection, "Baruch: Das Jahr am Leuchtturm", "Sammlung festgehalten");
    let off = store.sync_set_upload(work, false, "").await.unwrap();
    assert!(!off.upload && off.collection == on.collection, "aus behält die Sammlung: {off:?}");
    let again = store.sync_set_upload(work, true, "Baruch: Anderer Titel").await.unwrap();
    assert_eq!(
        again.collection, on.collection,
        "die festgehaltene Sammlung bleibt (hochgeladene Dokumente verlieren ihren Ort nicht)"
    );
    assert!(
        matches!(store.sync_set_upload(99999, true, "x").await, Err(StoreError::UnknownWork(99999))),
        "unbekanntes Werk"
    );
    assert!(
        matches!(store.sync_set_collections(&["a\nb".to_owned()]).await, Err(StoreError::Refused(_))),
        "ungültige Sammlung abgelehnt"
    );
    let chosen = store
        .sync_set_collections(&[" Bibeln ".to_owned(), "Kommentare".to_owned(), "Bibeln".to_owned()])
        .await
        .unwrap();
    assert_eq!(chosen, vec!["Bibeln", "Kommentare"], "beschnitten, Doppelte entfernt, Reihenfolge bleibt");
    let (id, changed) = store.sync_document_put("Bibeln", "d1", "Psalm", "", "Ein erfundener Text.").await.unwrap();
    assert!(changed, "neu angelegt");
    let (same, changed) = store.sync_document_put("Bibeln", "d1", "Psalm", "", "Ein erfundener Text.").await.unwrap();
    assert!(!changed && same == id, "gleicher Hash: nichts geändert, gleiche id");
    let (same, changed) = store.sync_document_put("Bibeln", "d1", "Psalm", "", "Ein anderer Text.").await.unwrap();
    assert!(changed && same == id, "neuer Text: geändert, gleiche id (der Index behält seine Zeilen-ids)");
    let run = store.sync_run_start().await.unwrap();
    let counts = SyncCounts { uploaded: 2, unchanged: 1, ..SyncCounts::default() };
    store.sync_run_finish(run, true, "Alles gut.", &counts).await.unwrap();
    let last = store.sync_run_last().await.unwrap().expect("Lauf gespeichert");
    assert_eq!((last.ok, last.counts), (Some(true), counts), "Ergebnis und Zahlen des Laufs");
    store
        .sync_state_put(&SyncState {
            side: SyncSide::Baruch,
            collection: on.collection.clone(),
            item: format!("work:{work}/key:1"),
            work: Some(work),
            node_key: Some(1),
            remote_id: "doc-1".into(),
            title: "1 Frühling".into(),
            hash: baruch_store::sync_hash("x"),
            remote_stamp: String::new(),
            synced: String::new(),
        })
        .await
        .unwrap();

    // A second opening changes nothing.
    let layout_now = tables(store.pool()).await;
    let schema_once = schema(store.pool()).await;
    let once = contents(store.pool(), &layout_now).await;
    drop(store);
    let store = Store::open(dir.path()).await.unwrap();
    assert_eq!(schema(store.pool()).await, schema_once, "zweites Öffnen: Schema gleich");
    assert_eq!(contents(store.pool(), &layout_now).await, once, "zweites Öffnen: alle Zeilen gleich");

    // Deleting the work takes its sync_work row along (ON DELETE CASCADE); sync_state stays for the next sync (which
    // clears away the documents of the deleted work at MultiGPT).
    store.delete_work(work, "Das Jahr am Leuchtturm").await.expect("Werk gelöscht");
    let left: i64 = sqlx::query_scalar("SELECT count(*) FROM sync_work").fetch_one(store.pool()).await.unwrap();
    assert_eq!(left, 0, "die sync_work-Zeile ist mit dem Werk gegangen");
    let states = store.sync_states(SyncSide::Baruch, None).await.unwrap();
    assert_eq!(states.len(), 1, "sync_state hat absichtlich keinen Fremdschlüssel: {states:?}");
    let broken: i64 =
        sqlx::query_scalar("SELECT count(*) FROM pragma_foreign_key_check").fetch_one(store.pool()).await.unwrap();
    assert_eq!(broken, 0, "keine Verstöße gegen Fremdschlüssel");
}

/// Migration 0041 (Round 26) enters the instance identifier as a new row in `store_meta` at start; that adds one row
/// and changes no old one. The comparison of the old rows takes out exactly this one row.
fn without_instance_row(mut contents: BTreeMap<String, Vec<String>>) -> BTreeMap<String, Vec<String>> {
    if let Some(meta) = contents.get_mut("store_meta") {
        let count = meta.len();
        meta.retain(|row| !row.starts_with("'instance_uuid'\u{1f}"));
        assert_eq!(count - meta.len(), 1, "genau eine neue Zeile instance_uuid in store_meta (Migration 0041)");
    }
    contents
}
