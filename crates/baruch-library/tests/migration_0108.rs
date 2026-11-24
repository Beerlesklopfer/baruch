// SPDX-License-Identifier: AGPL-3.0-or-later
//! Migration 0108 (Runde 21a, „pdf-import“: lokaler Volltext einer importierten Quelle) auf einer nachgebauten
//! Datenbank im Stand davor: Kernschema aus `tests/fixtures/core_schema.sql` und die Bibliothek bis 0107, mit frei
//! erfundenen Titeln, Zitaten, Hervorhebungen, einer Verknüpfung mit MultiGPT, einer Prüfung und dem Volltextindex.
//!
//! Danach: jede Zeile jeder alten Tabelle unverändert (auch der FTS-Index), das Schema der alten Tabellen und Trigger
//! gleich, neu ist genau `library_title_text`, leer. Die Tabelle lässt sich danach füllen (nur Text, CHECK), und ihre
//! Zeile verschwindet mit dem Titel. (Gegen das echte Kernschema von development prüft
//! `baruch-server/tests/round21_pdf_import.rs` dieselbe Migration noch einmal.)

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
    sqlx::query(sql).execute(pool).await.unwrap_or_else(|e| panic!("{sql}: {e}"));
}

/// Frei erfundene Daten im Stand 0107.
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
        let mut rows: Vec<String> =
            sqlx::query_scalar(&format!("SELECT {row} FROM \"{table}\"")).fetch_all(pool).await.unwrap();
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

    // Danach benutzbar: setzen, lesen, über den Hash finden; nur Text (CHECK); verschwindet mit dem Titel.
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
