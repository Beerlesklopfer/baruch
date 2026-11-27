// SPDX-License-Identifier: AGPL-3.0-or-later
//! Migration 0013 (connections and sources, Round 9) on a rebuilt database in the state of development before this
//! round: store migrations 0001–0012 and library 0101–0105, a freely invented work with template, chapters,
//! proposal, post with reference, idea with keyword, image, request, persona, deleted work and literature with
//! quote and highlight – rows in all tables.
//!
//! Afterwards `Store::open`: every row stays unchanged (through the columns from before), the schema of every
//! existing table as well, new are only `connection`, `work_rag`, `preferred_source` and the index
//! `preferred_source_order`; `MIGRATOR` knows version 13. A deleted work takes its `work_rag` row along. The schema
//! is compared after a migrator up to 0013 (Round 10), `Store::open` afterwards checks the preservation of data
//! with all later migrations.

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::path::Path;
use std::str::FromStr;

use baruch_store::{RagWay, Store, WorkId, title_key};
use sqlx::SqlitePool;
use sqlx::migrate::Migrator;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

const SOURCE: &str = "#set document(title: \"Das Mühlenbuch\", author: \"Lore Kamm\")\n#set heading(numbering: \"1.1\")\n\n\
= Wehr\n\nDas Wasser staut sich.\n\n== Rad\n\nEs dreht sich langsam.\n\n= Mahlgang\n\nKorn wird Mehl.\n";

const NEW_TABLES: [&str; 3] = ["connection", "preferred_source", "work_rag"];

async fn raw_pool(dir: &Path) -> SqlitePool {
    let options = SqliteConnectOptions::from_str("sqlite:")
        .unwrap()
        .filename(dir.join("baruch.sqlite"))
        .create_if_missing(true)
        .foreign_keys(true);
    SqlitePoolOptions::new().max_connections(1).connect_with(options).await.unwrap()
}

/// Store up to 0012, library complete (0101–0105, numbers of its own).
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
    let old: Vec<_> = baruch_store::MIGRATOR.iter().filter(|m| m.version <= 12).cloned().collect();
    assert!(old.iter().any(|m| m.version == 12), "Stand vorher hat 0012");
    let store = Migrator { migrations: Cow::Owned(old), ignore_missing: true, ..Migrator::DEFAULT };
    store.run(pool).await.unwrap();
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../baruch-library/migrations");
    let mut library = Migrator::new(dir).await.unwrap();
    // State before 0013 (like the other migration tests): later library migrations (Round 13: 0106 with initially
    // empty tables) do not belong to it.
    library.migrations = Cow::Owned(library.iter().filter(|m| m.version <= 105).cloned().collect());
    library.ignore_missing = true;
    library.run(pool).await.unwrap();
}

async fn exec(pool: &SqlitePool, sql: &str) {
    sqlx::query(sqlx::AssertSqlSafe(sql)).execute(pool).await.unwrap_or_else(|e| panic!("{sql}: {e}"));
}

/// A work with everything that exists in the state 0012; plus a deleted work and the library.
async fn insert_everything(pool: &SqlitePool) -> WorkId {
    let doc = baruch_typst::parse(SOURCE).unwrap();
    let work: WorkId = sqlx::query_scalar(
        "INSERT INTO work (slug, kind, title, author, template, source_name, source_sha256, title_key) \
         VALUES ('das-muehlenbuch', 'book', 'Das Mühlenbuch', 'Lore Kamm', ?, 'muehle.typ', 'erfunden', ?) RETURNING id",
    )
    .bind(&doc.template)
    .bind(title_key("Das Mühlenbuch"))
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
    exec(pool, "INSERT INTO work_retired (id, slug, title) VALUES (99, 'alte-scheune', 'Alte Scheune')").await;
    exec(
        pool,
        "INSERT INTO persona (handle, name, role, model, context_length, active, position, color) \
         VALUES ('@lektor', 'Lektor', 'Lektorat', 'probe', 32000, 1, 0, '#1c7ed6')",
    )
    .await;
    let layout = [(None, 0, "1", "chapter"), (Some(0), 0, "1.1", "section"), (None, 1, "2", "chapter")];
    let mut ids = Vec::new();
    for (seq, (heading, (parent, position, number, kind))) in doc.headings.iter().zip(layout).enumerate() {
        let id: i64 = sqlx::query_scalar(
            "INSERT INTO node (work_id, seq, parent_seq, position, number, title, kind) VALUES (?, ?, ?, ?, ?, ?, ?) RETURNING id",
        )
        .bind(work)
        .bind(seq as i64)
        .bind(parent)
        .bind(position)
        .bind(number)
        .bind(&heading.title)
        .bind(kind)
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
         VALUES (?, 2, '== Rad\n\nEs dreht sich schnell.\n', 'assistant', '@lektor', 'probe', 'proposed', 'Tempo', 1)",
    )
    .bind(ids[1])
    .execute(pool)
    .await
    .unwrap();
    let post: i64 = sqlx::query_scalar(
        "INSERT INTO post (node_id, position, author, body, sources) VALUES (?, 0, '@lektor', 'Wie schnell?', '[]') RETURNING id",
    )
    .bind(ids[0])
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO post_ref (post_id, position, node_seq, rev, quote) VALUES (?, 0, 0, 1, 'staut')")
        .bind(post)
        .execute(pool)
        .await
        .unwrap();
    let idea: i64 = sqlx::query_scalar(
        "INSERT INTO idea (work_id, node_id, note, origin_kind, origin_name) VALUES (?, ?, 'Mühlenlied', 'human', 'Autor') RETURNING id",
    )
    .bind(work)
    .bind(ids[2])
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO idea_word (idea_id, position, word, word_key) VALUES (?, 0, 'Lied', 'lied')")
        .bind(idea)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO idea_fts (rowid, words, note, quote, source, bible) VALUES (?, 'Lied', 'Mühlenlied', '', '', '')",
    )
    .bind(idea)
    .execute(pool)
    .await
    .unwrap();
    let asset: i64 = sqlx::query_scalar("INSERT INTO asset (work_id, name) VALUES (?, 'rad.svg') RETURNING id")
        .bind(work)
        .fetch_one(pool)
        .await
        .unwrap();
    let svg = "<svg xmlns=\"http://www.w3.org/2000/svg\"><circle r=\"3\"/></svg>";
    sqlx::query(
        "INSERT INTO asset_revision (asset_id, number, mime, encoding, content, bytes, sha256, caption, alt, author_name, status) \
         VALUES (?, 1, 'image/svg+xml', 'svg', ?, ?, 'erfundene-pruefsumme', 'Rad', 'Ein Wasserrad', 'Autor', 'current')",
    )
    .bind(asset)
    .bind(svg)
    .bind(svg.len() as i64)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO asset_path (work_id, path, asset_id) VALUES (?, 'bilder/rad.svg', ?)")
        .bind(work)
        .bind(asset)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO outline_request (work_id, op, params, origin_kind, origin_name) \
         VALUES (?, 'rename', '{\"op\":{},\"expected\":{}}', 'mcp', '@mcp:probe')",
    )
    .bind(work)
    .execute(pool)
    .await
    .unwrap();
    let title: i64 = sqlx::query_scalar(
        "INSERT INTO library_title (key, kind, title, authors) VALUES ('kamm-muehle', 'book', 'Mühlen am Bach', 'Kamm, Lore') RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    let quote: i64 = sqlx::query_scalar(
        "INSERT INTO library_quote (title_id, position, text, page) VALUES (?, 0, 'Das Rad steht nie still.', 'S. 3') RETURNING id",
    )
    .bind(title)
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO library_highlight (title_id, quote_id, text, kind, author_kind, author_name, status) \
         VALUES (?, ?, 'nie still', 'core', 'human', 'Autor', 'active')",
    )
    .bind(title)
    .bind(quote)
    .execute(pool)
    .await
    .unwrap();
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

/// `CREATE` text of all tables, indexes and triggers.
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
async fn migration_0013_keeps_every_row_and_adds_only_the_three_tables_and_the_index() {
    assert!(baruch_store::MIGRATOR.iter().any(|m| m.version == 13), "MIGRATOR enthält die neue Migration 0013");
    let dir = tempfile::tempdir().unwrap();
    let pool = raw_pool(dir.path()).await;
    migrate_before(&pool).await;
    let work = insert_everything(&pool).await;
    let layout = tables(&pool).await;
    let schema_before = schema(&pool).await;
    let before = contents(&pool, &layout).await;
    for table in NEW_TABLES {
        assert!(!layout.contains_key(table), "vorher gibt es {table} nicht");
    }
    for (table, rows) in &before {
        // SQLite fills FTS shadow tables itself; sqlite_sequence keeps AUTOINCREMENT.
        let shadow = table.starts_with("search_fts_") || table.starts_with("idea_fts_");
        assert!(shadow || !rows.is_empty(), "Zeilen in jeder Tabelle vor der Migration, auch in {table}");
    }
    migrate_upto(&pool, 13).await;
    let schema_after = schema(&pool).await;
    pool.close().await;

    let store = Store::open(dir.path()).await.expect("Store::open führt 0013 aus");
    let pool = store.pool();
    let after = without_instance_row(contents(pool, &layout).await);
    for (table, rows) in &before {
        if table == "_sqlx_migrations" {
            continue;
        }
        assert_eq!(after.get(table), Some(rows), "Tabelle {table}: alle Zeilen und Spalten unverändert");
    }
    let versions: Vec<i64> =
        sqlx::query_scalar("SELECT version FROM _sqlx_migrations ORDER BY version").fetch_all(pool).await.unwrap();
    assert!(versions.contains(&13) && versions.contains(&105), "0013 eingetragen, Bibliothek bleibt: {versions:?}");

    for (name, sql) in &schema_before {
        assert_eq!(schema_after.get(name), Some(sql), "Schema von {name} unverändert (0013 ändert keine Tabelle)");
    }
    let added: Vec<&str> =
        schema_after.keys().map(String::as_str).filter(|name| !schema_before.contains_key(*name)).collect();
    // Later migrations may bring further tables; 0013 itself brings exactly these four entries.
    for name in ["connection", "preferred_source", "preferred_source_order", "work_rag"] {
        assert!(added.contains(&name), "0013 legt {name} an: {added:?}");
    }
    let from_0013: Vec<&str> = added
        .iter()
        .copied()
        .filter(|name| {
            let sql = schema_after[*name].as_deref().unwrap_or("");
            NEW_TABLES.iter().any(|t| sql.contains(&format!("TABLE {t} ")) || sql.contains(&format!("ON {t}(")))
        })
        .collect();
    assert_eq!(
        from_0013,
        ["connection", "preferred_source", "preferred_source_order", "work_rag"],
        "nur drei Tabellen und ein Index gehören zu den neuen Tabellen (keine Trigger)"
    );
    let rag_sql = schema_after["work_rag"].as_deref().unwrap();
    assert!(rag_sql.contains("REFERENCES work(id) ON DELETE CASCADE"), "work_rag hängt am Werk: {rag_sql}");
    let empty: i64 = sqlx::query_scalar(
        "SELECT (SELECT count(*) FROM connection) + (SELECT count(*) FROM work_rag) + (SELECT count(*) FROM preferred_source)",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(empty, 0, "die neuen Tabellen beginnen leer (fehlende Zeile = Vorgabe)");
    let broken: i64 =
        sqlx::query_scalar("SELECT count(*) FROM pragma_foreign_key_check").fetch_one(pool).await.unwrap();
    assert_eq!(broken, 0, "keine Verstöße gegen Fremdschlüssel");
    assert_eq!(
        baruch_typst::xref::strip_key_labels(&store.export_typst(work).await.unwrap()).into_owned(),
        SOURCE,
        "Export byte-gleich wie vor der Migration"
    );

    // Afterwards: set RAG ways; a deleted work takes its row along (foreign_keys is on).
    assert_eq!(store.work_rag(work).await.unwrap(), (vec![RagWay::Lancedb], true), "ohne Zeile die Vorgabe");
    store.set_work_rag(work, &[RagWay::Pgvector]).await.unwrap();
    let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM work_rag").fetch_one(pool).await.unwrap();
    assert_eq!(rows, 1, "eine Zeile für das Werk");
    store.delete_work(work, "Das Mühlenbuch").await.unwrap();
    let left: i64 = sqlx::query_scalar("SELECT count(*) FROM work_rag").fetch_one(pool).await.unwrap();
    assert_eq!(left, 0, "ein gelöschtes Werk nimmt seine RAG-Wege mit (ON DELETE CASCADE)");
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
