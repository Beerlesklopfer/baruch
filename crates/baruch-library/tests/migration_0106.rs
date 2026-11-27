// SPDX-License-Identifier: AGPL-3.0-or-later
//! Migration 0106 (Runde 13, „quellen“: Verknüpfung eines Titels mit MultiGPT, Prüfung von Hervorhebungen
//! gegen den Volltext) auf einer nachgebauten Datenbank im Stand davor: Kernschema aus
//! `tests/fixtures/core_schema.sql` und die Bibliothek bis 0105, mit frei erfundenen Titeln, Zitaten,
//! Hervorhebungen (Mensch und Persona, mit und ohne Bereich) und dem Volltextindex.
//!
//! Danach: jede Zeile jeder alten Tabelle unverändert (auch der FTS-Index), das Schema der alten Tabellen
//! gleich, neu sind genau `library_title_remote` und `library_highlight_check`, beide leer. Die neuen
//! Tabellen lassen sich danach füllen, und ihre Zeilen verschwinden mit Titel bzw. Hervorhebung.

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::str::FromStr;

use baruch_library::{
    FulltextCheck, RemoteRef, highlight_checks, migrate, set_highlight_check, set_title_remote, title_remote,
};
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

/// Bibliothek bis einschließlich 0105 (Stand von development vor Runde 13).
async fn migrate_before(pool: &SqlitePool) {
    assert!(baruch_library::MIGRATOR.iter().any(|m| m.version == 106), "die neue Migration ist 0106");
    assert!(baruch_library::MIGRATOR.iter().any(|m| m.version == 105), "der Stand davor hat 0105");
    let list: Vec<_> = baruch_library::MIGRATOR.iter().filter(|m| m.version <= 105).cloned().collect();
    let migrator = Migrator { migrations: Cow::Owned(list), ignore_missing: true, ..Migrator::DEFAULT };
    migrator.run(pool).await.unwrap();
}

async fn exec(pool: &SqlitePool, sql: &str) {
    sqlx::query(sqlx::AssertSqlSafe(sql)).execute(pool).await.unwrap_or_else(|e| panic!("{sql}: {e}"));
}

/// Frei erfundene Daten: ein Werk mit Abschnitt und Revision (für den Index), drei Titel, Zitate,
/// Hervorhebungen aller Arten.
async fn insert_rows(pool: &SqlitePool) {
    exec(pool, "INSERT INTO work (id, slug, kind, title, author) VALUES (1, 'leuchtturm', 'book', 'Das Jahr am Leuchtturm', 'Mara Lind')").await;
    exec(pool, "INSERT INTO node (id, work_id, seq, parent_seq, position, number, title, kind) VALUES (1, 1, 0, NULL, 0, '1', 'Frühling', 'chapter')").await;
    exec(
        pool,
        "INSERT INTO library_title (id, key, kind, title, authors, year, publisher, place, url, note) VALUES \
         (1, 'lind2019', 'book', 'Licht über dem Wasser', 'Lind, Mara', 2019, 'Küstenverlag', 'Kiel', '', 'Erfunden'), \
         (2, 'senfkorn', 'article', 'Vom Senfkorn', 'Beet, Ilse', NULL, '', '', 'https://bib.example/sk', ''), \
         (3, 'web-moewe', 'web', 'Möwen am Kai', '', 2024, '', '', 'https://moewe.example/', 'Abgerufen am 01.02.2026')",
    )
    .await;
    exec(
        pool,
        "INSERT INTO library_quote (id, title_id, position, text, page, note) VALUES \
         (1, 1, 1, 'Das Licht kreist über dem Wasser.', 'S. 12', ''), \
         (2, 1, 2, 'Der Nebel kommt früh.', 'S. 40', 'Herbst'), \
         (3, 2, 1, 'Ein kleines Korn wird ein Baum.', '', '')",
    )
    .await;
    exec(
        pool,
        "INSERT INTO library_highlight (id, title_id, quote_id, locator, start_char, end_char, text, kind, note, \
           author_kind, author_name, model, status) VALUES \
         (1, 1, 1, 'S. 12', 4, 9, 'Licht', 'core', '', 'human', 'Autor', NULL, 'active'), \
         (2, 1, NULL, 'Kap. 2', 120, 160, 'Ein Satz im Volltext ohne Zitat', 'quote', 'Volltext', 'human', 'Autor', NULL, 'active'), \
         (3, 2, 3, '', NULL, NULL, 'Baum', 'check', '', 'assistant', '@belege', 'probe-a', 'proposed'), \
         (4, 3, NULL, '', NULL, NULL, 'Möwen schreien', 'contradiction', 'abgelehnt', 'human', 'Autor', NULL, 'rejected')",
    )
    .await;
}

/// Alle Tabellen außer `_sqlx_migrations` mit ihren Zeilen (jede Spalte über `quote()`), sortiert.
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
async fn migration_0106_keeps_every_row_and_adds_two_empty_tables() {
    let dir = tempfile::tempdir().unwrap();
    let pool = open(dir.path()).await;
    sqlx::raw_sql(CORE_SCHEMA).execute(&pool).await.unwrap();
    migrate_before(&pool).await;
    insert_rows(&pool).await;
    let before = snapshot(&pool).await;
    let schema_before = schema(&pool).await;
    assert!(
        before["library_highlight"].len() == 4 && before["library_title"].len() == 3,
        "die nachgebaute Datenbank hat Daten: {before:?}"
    );
    assert!(!before.contains_key("library_title_remote"), "vor 0106 gibt es die Tabelle noch nicht");

    // Genau bis 0106: Spätere Migrationen (0108 seit Runde 21a) legen eigene Tabellen an und würden die Liste
    // „genau die zwei neuen Tabellen“ verfälschen; danach läuft unten der volle Migrator.
    let upto: Vec<_> = baruch_library::MIGRATOR.iter().filter(|m| m.version <= 106).cloned().collect();
    Migrator { migrations: Cow::Owned(upto), ignore_missing: true, ..Migrator::DEFAULT }
        .run(&pool)
        .await
        .expect("Migration bis 0106");
    let after = snapshot(&pool).await;
    for (table, rows) in &before {
        assert_eq!(after.get(table), Some(rows), "{table}: jede Zeile unverändert (auch der FTS-Index)");
    }
    let schema_after = schema(&pool).await;
    for (name, sql) in &schema_before {
        assert_eq!(schema_after.get(name), Some(sql), "{name}: Schema der alten Tabelle bzw. des Triggers gleich");
    }
    let new: Vec<&String> = after.keys().filter(|t| !before.contains_key(*t)).collect();
    assert_eq!(new, ["library_highlight_check", "library_title_remote"], "genau die zwei neuen Tabellen");
    assert!(after["library_title_remote"].is_empty(), "Verknüpfungen anfangs leer: kein Titel gilt als verknüpft");
    assert!(after["library_highlight_check"].is_empty(), "Prüfungen anfangs leer: nichts wird als geprüft erfunden");

    // Danach benutzbar: Verknüpfung setzen und lesen, Prüfung speichern; beides verschwindet mit dem Besitzer.
    let remote = RemoteRef { way: "multigpt".into(), collection: "Leuchtturm".into(), document: "lt-1".into() };
    set_title_remote(&pool, 1, &remote).await.expect("Verknüpfung setzen");
    assert_eq!(title_remote(&pool, 1).await.unwrap(), Some(remote), "Verknüpfung gelesen");
    assert_eq!(title_remote(&pool, 2).await.unwrap(), None, "andere Titel bleiben unverknüpft");
    let wrong = RemoteRef { way: "pgvector".into(), collection: String::new(), document: "x".into() };
    assert!(set_title_remote(&pool, 2, &wrong).await.is_err(), "nur der Weg multigpt (CHECK)");
    let empty = RemoteRef { way: "multigpt".into(), collection: String::new(), document: String::new() };
    assert!(set_title_remote(&pool, 2, &empty).await.is_err(), "leeres Dokument abgelehnt (CHECK)");
    set_highlight_check(&pool, 2, FulltextCheck::Found, "").await.expect("Prüfung speichern");
    let checks = highlight_checks(&pool, Some(1)).await.unwrap();
    assert_eq!(checks.get(&2), Some(&(FulltextCheck::Found, String::new())), "Prüfung gelesen: {checks:?}");
    exec(&pool, "DELETE FROM library_highlight WHERE id = 2").await;
    assert!(highlight_checks(&pool, None).await.unwrap().is_empty(), "Prüfung verschwindet mit der Hervorhebung");
    exec(&pool, "DELETE FROM library_title WHERE id = 1").await;
    assert_eq!(title_remote(&pool, 1).await.unwrap(), None, "Verknüpfung verschwindet mit dem Titel");

    migrate(&pool).await.expect("zweiter Lauf ändert nichts");
}
