// SPDX-License-Identifier: AGPL-3.0-or-later
//! Migration 0038 (threads without section, Round 15 „auftraege“) on a reproduced database in the state before it (all
//! store migrations below 0038 – in the state of development up to 0035, after the merge also 0036 and 0037 – and
//! library 0101–0106), with a freely invented work, posts including source, personas and settings.
//!
//! After a migrator up to 0038: the schema of every old table stays, new are exactly `chat_thread`, `chat_thread_post`
//! and the index `chat_thread_work`, both tables are empty. Afterwards `Store::open`: every row of every old table
//! unchanged; a thread can be created and continued, a thread of another work counts as unknown, deleting the work
//! takes its threads along, a second opening changes nothing.

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::path::Path;
use std::str::FromStr;

use baruch_store::{Store, StoreError, UNKNOWN_THREAD, title_key};
use sqlx::SqlitePool;
use sqlx::migrate::Migrator;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

const SOURCE: &str = "#set document(title: \"Die Fähre im Schilf\", author: \"Ivo Brand\")\n\n\
= Abfahrt\n\nDas Seil knarrt am Steg.\n\n= Ankunft\n\nDrüben wartet niemand.\n";

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

/// Store below 0038 and library up to 0106.
async fn migrate_before(pool: &SqlitePool) {
    assert!(baruch_store::MIGRATOR.iter().any(|m| m.version == 38), "die neue Migration ist 0038");
    assert!(
        baruch_store::MIGRATOR.iter().any(|m| m.version == 35),
        "Stand davor hat mindestens 0035 (development zu Beginn von Runde 15)"
    );
    migrate_where(pool, |v| v < 38).await;
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../baruch-library/migrations");
    let mut library = Migrator::new(dir).await.unwrap();
    library.migrations = Cow::Owned(library.iter().filter(|m| m.version <= 106).cloned().collect());
    library.ignore_missing = true;
    library.run(pool).await.unwrap();
}

async fn exec(pool: &SqlitePool, sql: &str) {
    sqlx::query(sqlx::AssertSqlSafe(sql)).execute(pool).await.unwrap_or_else(|e| panic!("{sql}: {e}"));
}

/// Work with two chapters, two posts (one with source), personas, settings.
async fn insert_rows(pool: &SqlitePool) -> i64 {
    let doc = baruch_typst::parse(SOURCE).unwrap();
    let work: i64 = sqlx::query_scalar(
        "INSERT INTO work (slug, kind, title, author, template, source_name, source_sha256, title_key, audience) \
         VALUES ('die-faehre-im-schilf', 'book', 'Die Fähre im Schilf', 'Ivo Brand', ?, 'faehre.typ', \
         'erfunden', ?, 'Flussleute') RETURNING id",
    )
    .bind(&doc.template)
    .bind(title_key("Die Fähre im Schilf"))
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
         ('@lektor', 'Lektor', 'Lektorat', 'probe-a', 32000, 1, 0, '#1c7ed6', NULL), \
         ('@hoerer', 'Hörer', 'Gemeinde', 'probe-a', 16000, 1, 1, NULL, 'Eigener Prompt.')",
    )
    .await;
    let mut nodes = Vec::new();
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
        nodes.push(id);
    }
    for (node, position, author, body, sources) in [
        (nodes[0], 0, "Autor", "Knarrt das Seil zu oft?", "[]"),
        (nodes[0], 1, "@lektor", "Einmal genügt.", "[\"Einfall 2\"]"),
    ] {
        sqlx::query("INSERT INTO post (node_id, position, author, body, sources) VALUES (?, ?, ?, ?, ?)")
            .bind(node)
            .bind(position)
            .bind(author)
            .bind(body)
            .bind(sources)
            .execute(pool)
            .await
            .unwrap();
    }
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

#[tokio::test]
async fn migration_0038_keeps_every_row_and_only_adds_the_thread_tables() {
    let dir = tempfile::tempdir().unwrap();
    let pool = raw_pool(dir.path()).await;
    migrate_before(&pool).await;
    let work = insert_rows(&pool).await;
    let layout = tables(&pool).await;
    let schema_before = schema(&pool).await;
    let before = contents(&pool, &layout).await;
    assert_eq!(before["post"].len(), 2, "zwei Abschnitts-Beiträge vor der Migration (sie dürfen nicht wandern)");

    migrate_where(&pool, |v| v <= 38).await;
    let schema_after = schema(&pool).await;
    for (name, sql) in &schema_before {
        assert_eq!(schema_after.get(name), Some(sql), "Schema von {name} unverändert (0038 ändert keine alte Tabelle)");
    }
    let added: Vec<&str> =
        schema_after.keys().filter(|n| !schema_before.contains_key(*n)).map(String::as_str).collect();
    assert_eq!(
        added,
        vec!["chat_thread", "chat_thread_post", "chat_thread_work"],
        "genau die zwei neuen Tabellen und der Index je Werk (die Übersicht liest je Werk nach Zeit)"
    );
    for table in ["chat_thread", "chat_thread_post"] {
        let rows: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT count(*) FROM {table}")))
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(rows, 0, "keine Datenänderung: {table} ist leer");
    }
    let orphan =
        sqlx::query("INSERT INTO chat_thread (work_id, created_by) VALUES (99999, 'Autor')").execute(&pool).await;
    assert!(orphan.is_err(), "Fremdschlüssel: ein Faden braucht ein vorhandenes Werk");
    pool.close().await;

    let store = Store::open(dir.path()).await.expect("Store::open nach 0038");
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

    // Create and continue a thread.
    let (thread, first) = store.thread_create(work, "@mcp:probe", "Wie endet die Überfahrt?").await.unwrap();
    assert_eq!(first.position, 0, "der erste Beitrag steht an Stelle 0");
    assert_eq!(thread.title, "Wie endet die Überfahrt?", "Titel aus der ersten Nachricht");
    let second = store.thread_add_post(work, thread.id, "@lektor", "Offen.", &["Faden".to_owned()]).await.unwrap();
    assert_eq!(second.position, 1, "fortgesetzt an der nächsten Stelle");
    let read = store.thread_get(work, thread.id).await.unwrap();
    assert_eq!(read.entries.len(), 2, "beide Beiträge im Faden");
    assert_eq!(read.summary.last_author, "@lektor", "der letzte Autor steht in der Übersicht");
    assert_eq!(read.entries[1].sources, vec!["Faden".to_owned()], "Quellen als JSON gespeichert und gelesen");
    let posts: i64 = sqlx::query_scalar("SELECT count(*) FROM post").fetch_one(store.pool()).await.unwrap();
    assert_eq!(posts, 2, "Fadenbeiträge stehen nicht im Abschnitts-Chat (Tabelle post unverändert)");

    // A thread of another work is unknown.
    let new = baruch_store::NewWork {
        title: "Ein anderes Ufer",
        author: "Ivo Brand",
        kind: baruch_core::WorkKind::Book,
        preset: baruch_store::Preset::default(),
        subtitle: "",
        audience: "",
        bible: "",
        language: Default::default(),
    };
    let other = store.create_work(&new, "Autor").await.expect("zweites Werk");
    let foreign = store.thread_get(other.work, thread.id).await;
    assert!(
        matches!(foreign, Err(StoreError::Refused(m)) if m == UNKNOWN_THREAD),
        "id aus fremdem Werk wird nicht gefunden: {foreign:?}"
    );

    // A second opening changes nothing.
    let layout_now = tables(store.pool()).await;
    let schema_once = schema(store.pool()).await;
    let once = contents(store.pool(), &layout_now).await;
    drop(store);
    let store = Store::open(dir.path()).await.unwrap();
    assert_eq!(schema(store.pool()).await, schema_once, "zweites Öffnen: Schema gleich");
    assert_eq!(contents(store.pool(), &layout_now).await, once, "zweites Öffnen: alle Zeilen gleich");

    // Deleting the work takes its threads along (ON DELETE CASCADE).
    store.delete_work(work, "Die Fähre im Schilf").await.expect("Werk löschen");
    let left: i64 = sqlx::query_scalar("SELECT count(*) FROM chat_thread_post").fetch_one(store.pool()).await.unwrap();
    assert_eq!(left, 0, "mit dem Werk sind seine Fadenbeiträge fort");
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
