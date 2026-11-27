// SPDX-License-Identifier: AGPL-3.0-or-later
//! Migration 0108 (Round 21a, „pdf-import“: local full text of an imported source) on a rebuilt database in the
//! state before it: core schema from `tests/fixtures/core_schema.sql` and the library up to 0107, with freely
//! invented titles, quotes, highlights, a link with MultiGPT, a check and the full-text index.
//!
//! Afterwards: every row of every old table unchanged (also the FTS index), the schema of the old tables and
//! triggers the same, new is exactly `library_title_text`, empty. The table can be filled afterwards (text only,
//! CHECK), and its row disappears with the title. (Against the real core schema of development
//! `baruch-server/tests/round21_pdf_import.rs` checks the same migration once more.)

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::str::FromStr;

use baruch_library::{TitleText, migrate, set_title_text, title_for_text_sha256, title_text};
use sqlx::SqlitePool;
use sqlx::migrate::Migrator;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

const CORE_SCHEMA: &str = include_str!("fixtures/core_schema.sql");

async fn open(dir: &std::path::Path) -> SqlitePool {
    let options = SqliteConnectOptions::from_str("sqlite:")
        .unwrap()
        .filename(dir.join("baruch.sqlite"))
        .create_if_missing(true)
        .foreign_keys(true);
    SqlitePoolOptions::new().max_connections(1).connect_with(options).await.unwrap()
}

async fn migrate_upto(pool: &SqlitePool, version: i64) {
    let list: Vec<_> = baruch_library::MIGRATOR.iter().filter(|m| m.version <= version).cloned().collect();
    Migrator { migrations: Cow::Owned(list), ignore_missing: true, ..Migrator::DEFAULT }.run(pool).await.unwrap();
}

async fn exec(pool: &SqlitePool, sql: &str) {
    sqlx::query(sqlx::AssertSqlSafe(sql)).execute(pool).await.unwrap_or_else(|e| panic!("{sql}: {e}"));
}

/// Freely invented data in the state 0107.
async fn insert_rows(pool: &SqlitePool) {
    exec(pool, "INSERT INTO work (id, slug, kind, title, author) VALUES (1, 'deich', 'book', 'Am Deich', 'Ina Moor')")
        .await;
    exec(
        pool,
        "INSERT INTO library_title (id, key, kind, title, authors, year, publisher, place, url, note) VALUES \
         (1, 'moor2020', 'book', 'Wind über dem Watt', 'Moor, Ina', 2020, 'Deichverlag', 'Husum', '', 'Erfunden'), \
         (2, 'mg-ebbe', 'other', 'Ebbe und Flut', '', NULL, '', '', 'https://bib.example/ebbe', '')",
    )
    .await;
    exec(
        pool,
        "INSERT INTO library_quote (id, title_id, position, text, page, note) VALUES \
         (1, 1, 1, 'Der Wind kommt von Westen.', 'S. 3', '')",
    )
    .await;
    exec(
        pool,
        "INSERT INTO library_highlight (id, title_id, quote_id, locator, start_char, end_char, text, kind, note, \
           author_kind, author_name, model, status) VALUES \
         (1, 1, 1, 'S. 3', 4, 8, 'Wind', 'core', '', 'human', 'Autor', NULL, 'active'), \
         (2, 2, NULL, '', 10, 20, 'Flut steigt', 'quote', '', 'human', 'Autor', NULL, 'active')",
    )
    .await;
    exec(
        pool,
        "INSERT INTO library_title_remote (title_id, way, collection, document) VALUES (2, 'multigpt', 'Watt', 'eb-1')",
    )
    .await;
    exec(pool, "INSERT INTO library_highlight_check (highlight_id, result, message) VALUES (2, 'found', '')").await;
}

async fn snapshot(pool: &SqlitePool) -> BTreeMap<String, Vec<String>> {
    let tables: Vec<String> = sqlx::query_scalar(
        "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' \
           AND name <> '_sqlx_migrations' ORDER BY name",
    )
    .fetch_all(pool)
    .await
    .unwrap();
    let mut out = BTreeMap::new();
    for table in tables {
        let columns: Vec<String> = sqlx::query_scalar("SELECT name FROM pragma_table_info(?1) ORDER BY cid")
            .bind(&table)
            .fetch_all(pool)
            .await
            .unwrap();
        if columns.is_empty() {
            continue;
        }
        let row = columns.iter().map(|c| format!("quote(\"{c}\")")).collect::<Vec<_>>().join(" || '|' || ");
        let mut rows: Vec<String> = sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT {row} FROM \"{table}\"")))
            .fetch_all(pool)
            .await
            .unwrap();
        rows.sort();
        out.insert(table, rows);
    }
    out
}

async fn schema(pool: &SqlitePool) -> BTreeMap<String, String> {
    let rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT name, coalesce(sql, '') FROM sqlite_master WHERE name NOT LIKE 'sqlite_%' ORDER BY name",
    )
    .fetch_all(pool)
    .await
    .unwrap();
    rows.into_iter().collect()
}

#[tokio::test]
async fn migration_0108_keeps_every_row_and_adds_one_empty_table() {
    assert!(baruch_library::MIGRATOR.iter().any(|m| m.version == 108), "die neue Migration ist 0108");
    let dir = tempfile::tempdir().unwrap();
    let pool = open(dir.path()).await;
    sqlx::raw_sql(CORE_SCHEMA).execute(&pool).await.unwrap();
    migrate_upto(&pool, 107).await;
    insert_rows(&pool).await;
    let before = snapshot(&pool).await;
    let schema_before = schema(&pool).await;
    assert!(
        before["library_title"].len() == 2 && before["library_title_remote"].len() == 1,
        "die nachgebaute Datenbank hat Daten, auch eine Verknüpfung: {before:?}"
    );
    assert!(!before.contains_key("library_title_text"), "vor 0108 gibt es die Tabelle noch nicht");

    migrate_upto(&pool, 108).await;
    let after = snapshot(&pool).await;
    for (table, rows) in &before {
        assert_eq!(after.get(table), Some(rows), "{table}: jede Zeile unverändert (auch der FTS-Index)");
    }
    let schema_after = schema(&pool).await;
    for (name, sql) in &schema_before {
        assert_eq!(schema_after.get(name), Some(sql), "{name}: Schema der alten Tabelle bzw. des Triggers gleich");
    }
    let new: Vec<&String> = after.keys().filter(|t| !before.contains_key(*t)).collect();
    assert_eq!(new, ["library_title_text"], "genau die eine neue Tabelle, nichts umgebaut");
    assert!(after["library_title_text"].is_empty(), "anfangs leer: kein Titel bekommt einen erfundenen Volltext");

    // Usable afterwards: set, read, find through the hash; text only (CHECK); disappears with the title.
    let text = TitleText {
        source_name: "watt.pdf".into(),
        mime: "application/pdf".into(),
        sha256: "ab".repeat(32),
        pages: Some(3),
        text: "Der Wind kommt von Westen.\n\nDie Flut steigt.".into(),
    };
    set_title_text(&pool, 1, &text).await.expect("Volltext setzen");
    assert_eq!(title_text(&pool, 1).await.unwrap(), Some(text.clone()), "Volltext gelesen, wie gespeichert");
    assert_eq!(title_text(&pool, 2).await.unwrap(), None, "andere Titel haben keinen Volltext");
    assert_eq!(title_for_text_sha256(&pool, &text.sha256).await.unwrap(), Some(1), "über den Hash der Datei gefunden");
    assert!(set_title_text(&pool, 99, &text).await.is_err(), "unbekannter Titel abgelehnt, nichts angelegt");
    let blob = sqlx::query(
        "INSERT INTO library_title_text (title_id, source_name, mime, sha256, text) VALUES (2, 'x', 'y', 'z', x'00')",
    )
    .execute(&pool)
    .await;
    assert!(blob.is_err(), "Bytes statt Text verletzen den CHECK: der Volltext ist immer Text");
    exec(&pool, "DELETE FROM library_title WHERE id = 1").await;
    assert_eq!(title_text(&pool, 1).await.unwrap(), None, "Volltext verschwindet mit dem Titel (ON DELETE CASCADE)");

    migrate(&pool).await.expect("voller Lauf danach ändert nichts und scheitert nicht");
}
