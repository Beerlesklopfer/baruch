// SPDX-License-Identifier: AGPL-3.0-or-later
//! Migration 0014 (binding of the secret to the target of a connection) on a rebuilt database in the state of
//! development before it: store migrations 0001–0013 and library, a freely invented work with chapters, stored
//! connections (one with a check), RAG ways and one preferred source.
//!
//! Afterwards `Store::open`: every row of every table stays unchanged in all old columns, the schema of all tables
//! except `connection` as well; `connection` only has the new column `secret_target`, which is NULL in all old rows
//! ("applies to the present target" – so as before). Schema and columns are compared after a migrator up to 0014
//! (Round 10), `Store::open` afterwards checks the preservation of data.

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::path::Path;
use std::str::FromStr;

use baruch_store::{ConnectionKind, Store, title_key};
use sqlx::SqlitePool;
use sqlx::migrate::Migrator;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

const SOURCE: &str = "#set document(title: \"Der Taubenschlag\", author: \"Edda Firn\")\n\n\
= Gurren\n\nAm Morgen ist es laut.\n\n= Flug\n\nSie kreisen über dem Hof.\n";

async fn raw_pool(dir: &Path) -> SqlitePool {
    let options = SqliteConnectOptions::from_str("sqlite:")
        .unwrap()
        .filename(dir.join("baruch.sqlite"))
        .create_if_missing(true)
        .foreign_keys(true);
    SqlitePoolOptions::new().max_connections(1).connect_with(options).await.unwrap()
}

/// Store migrations up to and including `version` on the rebuilt database (Round 10): the schema is compared after
/// this migrator, so that later migrations (e.g. 0015 on `persona`) do not break the comparison; `Store::open`
/// afterwards only checks the preservation of data.
async fn migrate_upto(pool: &SqlitePool, version: i64) {
    let upto: Vec<_> = baruch_store::MIGRATOR.iter().filter(|m| m.version <= version).cloned().collect();
    assert!(upto.iter().any(|m| m.version == version), "MIGRATOR enthält {version}");
    let migrator = Migrator { migrations: Cow::Owned(upto), ignore_missing: true, ..Migrator::DEFAULT };
    migrator.run(pool).await.unwrap();
}

async fn migrate_before(pool: &SqlitePool) {
    let old: Vec<_> = baruch_store::MIGRATOR.iter().filter(|m| m.version <= 13).cloned().collect();
    assert!(old.iter().any(|m| m.version == 13), "Stand vorher hat 0013");
    let store = Migrator { migrations: Cow::Owned(old), ignore_missing: true, ..Migrator::DEFAULT };
    store.run(pool).await.unwrap();
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../baruch-library/migrations");
    let mut library = Migrator::new(dir).await.unwrap();
    library.ignore_missing = true;
    library.run(pool).await.unwrap();
}

async fn exec(pool: &SqlitePool, sql: &str) {
    sqlx::query(sqlx::AssertSqlSafe(sql)).execute(pool).await.unwrap_or_else(|e| panic!("{sql}: {e}"));
}

async fn insert_rows(pool: &SqlitePool) -> i64 {
    let doc = baruch_typst::parse(SOURCE).unwrap();
    let work: i64 = sqlx::query_scalar(
        "INSERT INTO work (slug, kind, title, author, template, source_name, source_sha256, title_key) \
         VALUES ('der-taubenschlag', 'book', 'Der Taubenschlag', 'Edda Firn', ?, 'tauben.typ', 'erfunden', ?) RETURNING id",
    )
    .bind(&doc.template)
    .bind(title_key("Der Taubenschlag"))
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
    }
    exec(pool, "INSERT INTO connection (kind, config) VALUES ('eliezer', '{\"url\":\"http://127.0.0.1:4321\"}')").await;
    exec(
        pool,
        "INSERT INTO connection (kind, config, checked, check_ok, check_message, check_details) VALUES \
         ('pgvector', '{\"host\":\"db.example\",\"port\":5433,\"database\":\"rag\",\"user\":\"tauben\",\"tls\":\"require\"}', \
          '2026-11-01T08:00:00.000Z', 1, 'Verbindung steht', '{\"server_version\":\"16\"}')",
    )
    .await;
    sqlx::query("INSERT INTO work_rag (work_id, ways) VALUES (?, '[\"pgvector\"]')")
        .bind(work)
        .execute(pool)
        .await
        .unwrap();
    exec(
        pool,
        "INSERT INTO preferred_source (title, url, note, position) VALUES ('Vogelkunde', 'https://voegel.example/', 'erfunden', 1)",
    )
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

/// All rows through the columns of `layout` (before), sorted.
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
async fn migration_0014_keeps_every_row_and_only_adds_secret_target() {
    assert!(baruch_store::MIGRATOR.iter().any(|m| m.version == 14), "MIGRATOR enthält 0014");
    let dir = tempfile::tempdir().unwrap();
    let pool = raw_pool(dir.path()).await;
    migrate_before(&pool).await;
    let work = insert_rows(&pool).await;
    let layout = tables(&pool).await;
    let schema_before = schema(&pool).await;
    let before = contents(&pool, &layout).await;
    assert_eq!(before["connection"].len(), 2, "zwei gespeicherte Verbindungen vor der Migration");
    assert!(!layout["connection"].contains(&"secret_target".to_owned()), "vorher keine Spalte secret_target");
    migrate_upto(&pool, 14).await;
    let schema_after = schema(&pool).await;
    let columns = tables(&pool).await;
    pool.close().await;

    let store = Store::open(dir.path()).await.expect("Store::open führt 0014 aus");
    let pool = store.pool();
    let after = without_instance_row(contents(pool, &layout).await);
    for (table, rows) in &before {
        if table == "_sqlx_migrations" {
            continue;
        }
        assert_eq!(after.get(table), Some(rows), "Tabelle {table}: alle Zeilen in den alten Spalten unverändert");
    }
    for (name, sql) in &schema_before {
        if name == "connection" {
            continue;
        }
        assert_eq!(schema_after.get(name), Some(sql), "Schema von {name} unverändert");
    }
    let mut expected = layout["connection"].clone();
    expected.push("secret_target".to_owned());
    assert_eq!(columns["connection"], expected, "connection: genau eine neue Spalte am Ende");
    let bound: i64 = sqlx::query_scalar("SELECT count(*) FROM connection WHERE secret_target IS NOT NULL")
        .fetch_one(pool)
        .await
        .unwrap();
    assert_eq!(bound, 0, "alte Zeilen: NULL = das Geheimnis gilt für das jetzige Ziel (wie bisher)");

    let pg = store.connection_get(ConnectionKind::Pgvector).await.unwrap();
    assert_eq!(pg.config["host"], "db.example", "Einstellungen lesbar");
    assert!(pg.check.is_some_and(|c| c.ok && c.message == "Verbindung steht"), "Prüfung bleibt");
    assert_eq!(pg.secret_target, None, "ohne Bindung");
    let broken: i64 =
        sqlx::query_scalar("SELECT count(*) FROM pragma_foreign_key_check").fetch_one(pool).await.unwrap();
    assert_eq!(broken, 0, "keine Verstöße gegen Fremdschlüssel");
    assert_eq!(
        baruch_typst::xref::strip_key_labels(&store.export_typst(work).await.unwrap()).into_owned(),
        SOURCE,
        "Export byte-gleich wie vor der Migration"
    );
}

/// Migration 0041 (Round 26) enters the instance identifier as a new row into `store_meta` at start; that adds one
/// row and changes no old one. The comparison of the old rows takes out exactly this one row.
fn without_instance_row(mut contents: BTreeMap<String, Vec<String>>) -> BTreeMap<String, Vec<String>> {
    if let Some(meta) = contents.get_mut("store_meta") {
        let count = meta.len();
        meta.retain(|row| !row.starts_with("'instance_uuid'\u{1f}"));
        assert_eq!(count - meta.len(), 1, "genau eine neue Zeile instance_uuid in store_meta (Migration 0041)");
    }
    contents
}
