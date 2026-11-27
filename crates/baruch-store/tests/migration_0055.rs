// SPDX-License-Identifier: AGPL-3.0-or-later
//! Migration 0055 (Round 38, work language) on a rebuilt database in the state of 0054 with invented rows.
//!
//! Shown: the file is the one from interface R38-S1 and only adds a column (not in `FOREIGN_KEYS_OFF`); every row of
//! every old table survives byte for byte (the table `work` apart from the new column); every existing work becomes
//! German; the CHECK rule refuses every value but `de` and `en`; the store opens the migrated database.

use std::borrow::Cow;
use std::path::Path;
use std::str::FromStr;

use baruch_store::{Store, WorkLanguage};
use sqlx::migrate::Migrator;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};

/// The statement literally from the contract (§4.1), without the comment lines of the file.
const CONTRACT_SQL: &str =
    "ALTER TABLE work ADD COLUMN language TEXT NOT NULL DEFAULT 'de' CHECK (language IN ('de', 'en'));";

async fn raw_pool(dir: &Path) -> SqlitePool {
    let options = SqliteConnectOptions::from_str("sqlite:")
        .unwrap()
        .filename(dir.join("baruch.sqlite"))
        .create_if_missing(true)
        .foreign_keys(true);
    SqlitePoolOptions::new().max_connections(1).connect_with(options).await.unwrap()
}

fn store_migrator(keep: impl Fn(i64) -> bool) -> Migrator {
    let list: Vec<_> = baruch_store::MIGRATOR.iter().filter(|m| keep(m.version)).cloned().collect();
    Migrator { migrations: Cow::Owned(list), ignore_missing: true, ..Migrator::DEFAULT }
}

async fn exec(pool: &SqlitePool, sql: &str) {
    sqlx::query(sqlx::AssertSqlSafe(sql)).execute(pool).await.unwrap_or_else(|e| panic!("{sql}: {e}"));
}

async fn rejected(pool: &SqlitePool, sql: &str) -> bool {
    sqlx::query(sqlx::AssertSqlSafe(sql)).execute(pool).await.is_err()
}

/// Store up to 0054 (0040 with foreign keys switched off, as in `baruch_store::migrate`): the state before 0055.
async fn migrate_before(pool: &SqlitePool) {
    assert_eq!(baruch_store::FOREIGN_KEYS_OFF, &[40], "precondition of the rebuild: only 0040 rebuilds tables");
    store_migrator(|v| v < 40).run(pool).await.unwrap();
    {
        let mut conn = pool.acquire().await.unwrap();
        sqlx::query("PRAGMA foreign_keys = OFF").execute(&mut *conn).await.unwrap();
        store_migrator(|v| v == 40).run(&mut *conn).await.unwrap();
        sqlx::query("PRAGMA foreign_keys = ON").execute(&mut *conn).await.unwrap();
    }
    store_migrator(|v| (41..=54).contains(&v)).run(pool).await.unwrap();
    let highest: i64 = sqlx::query_scalar("SELECT max(version) FROM _sqlx_migrations").fetch_one(pool).await.unwrap();
    assert_eq!(highest, 54, "the state before: store up to 0054");
}

async fn insert_work(pool: &SqlitePool, slug: &str, kind: &str, title: &str) -> i64 {
    sqlx::query_scalar(
        "INSERT INTO work (slug, kind, title, author, template, source_name, source_sha256, title_key) \
         VALUES (?, ?, ?, 'Tilda Rohr', '#set text(lang: \"de\")', ?, ?, ?) RETURNING id",
    )
    .bind(slug)
    .bind(kind)
    .bind(title)
    .bind(format!("{slug}.typ"))
    .bind(format!("sha-{slug}"))
    .bind(baruch_store::title_key(title))
    .fetch_one(pool)
    .await
    .unwrap()
}

/// Names of the columns of a table.
async fn columns(pool: &SqlitePool, table: &str) -> Vec<String> {
    sqlx::query(sqlx::AssertSqlSafe(format!("PRAGMA table_info({table})")))
        .fetch_all(pool)
        .await
        .unwrap()
        .iter()
        .map(|row| row.get::<String, _>("name"))
        .collect()
}

/// All rows of a table as text (the given columns through `quote()`), by rowid.
async fn dump(pool: &SqlitePool, table: &str, columns: &[String]) -> Vec<String> {
    let quoted: Vec<String> = columns.iter().map(|c| format!("quote(\"{c}\")")).collect();
    let sql = format!("SELECT {} FROM {table} ORDER BY rowid", quoted.join(" || '|' || "));
    sqlx::query_scalar(sqlx::AssertSqlSafe(sql.as_str())).fetch_all(pool).await.unwrap_or_else(|e| panic!("{sql}: {e}"))
}

/// Names of all ordinary tables (without the bookkeeping of sqlx, full-text indexes and their shadow tables).
async fn tables(pool: &SqlitePool) -> Vec<String> {
    sqlx::query_scalar(
        "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' \
         AND name <> '_sqlx_migrations' AND sql NOT LIKE 'CREATE VIRTUAL%' AND name NOT LIKE '%fts%' ORDER BY name",
    )
    .fetch_all(pool)
    .await
    .unwrap()
}

#[test]
fn the_migration_file_is_the_one_from_the_interface_and_only_adds_a_column() {
    let migration = baruch_store::MIGRATOR.iter().find(|m| m.version == 55).expect("migration 0055 is bundled");
    let statements: String = migration
        .sql
        .as_str()
        .lines()
        .filter(|line| !line.trim_start().starts_with("--") && !line.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(statements, CONTRACT_SQL, "the statement is literally the one from the contract (§4.1)");
    assert_eq!(migration.description, "work language", "file name 0055_work_language.sql as in the contract");
    assert!(
        !baruch_store::FOREIGN_KEYS_OFF.contains(&55),
        "ADD COLUMN rebuilds no table, so 0055 must run with foreign keys switched on"
    );
    assert!(
        baruch_store::MIGRATOR.iter().all(|m| m.version != 56),
        "0056 is reserved for the integrator and expected to stay unused in this branch"
    );
}

#[tokio::test]
async fn existing_rows_survive_and_every_work_becomes_german() {
    let dir = tempfile::tempdir().unwrap();
    let pool = raw_pool(dir.path()).await;
    migrate_before(&pool).await;

    // Invented rows in the state of 0054: two works (book and sermon), outline, revisions, a post and an idea.
    let ferry = insert_work(&pool, "the-ferry-at-dusk", "book", "The Ferry at Dusk").await;
    let salt = insert_work(&pool, "salz-im-brot", "sermon", "Salz im Brot").await;
    exec(&pool, &format!("UPDATE work SET subtitle = 'Notes from the pier', audience = 'ferrymen' WHERE id = {ferry}"))
        .await;
    for (work, title) in [(ferry, "Low Tide"), (salt, "Eine Prise")] {
        exec(
            &pool,
            &format!(
                "INSERT INTO node (work_id, seq, parent_seq, position, number, title, kind) \
                 VALUES ({work}, 0, NULL, 0, '1', '{title}', 'chapter')"
            ),
        )
        .await;
        exec(
            &pool,
            &format!(
                "INSERT INTO revision (node_id, number, body, author_kind, author_name, model, status, note) \
                 SELECT id, 1, '= {title}\n\nThe rope creaks.\n', 'human', 'Tilda Rohr', NULL, 'current', 'Import' \
                 FROM node WHERE work_id = {work}"
            ),
        )
        .await;
    }
    exec(
        &pool,
        &format!(
            "INSERT INTO post (node_id, position, author, body) \
             SELECT id, 0, '@lektor', 'Kritisch: \"creaks\" twice.' FROM node WHERE work_id = {ferry}"
        ),
    )
    .await;
    assert!(
        !columns(&pool, "work").await.contains(&"language".to_owned()),
        "precondition: before 0055 the table work has no column language"
    );

    let names = tables(&pool).await;
    let mut before = Vec::new();
    for table in &names {
        let cols = columns(&pool, table).await;
        before.push((table.clone(), cols.clone(), dump(&pool, table, &cols).await));
    }
    assert!(
        before.iter().any(|(t, _, rows)| t == "revision" && rows.len() == 2),
        "the rebuilt database really holds the invented rows"
    );

    store_migrator(|v| v == 55).run(&pool).await.unwrap();

    assert_eq!(tables(&pool).await, names, "0055 creates and drops no table");
    for (table, cols, rows) in &before {
        assert_eq!(
            &dump(&pool, table, cols).await,
            rows,
            "table {table}: every old column of every row is byte for byte the same after 0055"
        );
        let now = columns(&pool, table).await;
        if table == "work" {
            let mut expected = cols.clone();
            expected.push("language".to_owned());
            assert_eq!(now, expected, "work gets exactly one new column at the end");
        } else {
            assert_eq!(&now, cols, "table {table} keeps its columns");
        }
    }
    let languages: Vec<String> =
        sqlx::query_scalar("SELECT language FROM work ORDER BY id").fetch_all(&pool).await.unwrap();
    assert_eq!(languages, ["de", "de"], "every work from before Round 38 is a German work");

    // The CHECK rule: only the two work languages.
    assert!(
        rejected(&pool, &format!("UPDATE work SET language = 'fr' WHERE id = {ferry}")).await,
        "CHECK refuses a language Baruch has no markers for"
    );
    for bad in ["", "DE", "En", "de "] {
        assert!(
            rejected(&pool, &format!("UPDATE work SET language = '{bad}' WHERE id = {ferry}")).await,
            "CHECK refuses {bad:?}: the codes are exactly de and en, lower case, untrimmed"
        );
    }
    assert!(
        rejected(&pool, &format!("UPDATE work SET language = NULL WHERE id = {ferry}")).await,
        "NOT NULL: a work always has a language"
    );
    exec(&pool, &format!("UPDATE work SET language = 'en' WHERE id = {ferry}")).await;
    let fresh = insert_work(&pool, "neues-werk", "book", "Neues Werk").await;
    let languages: Vec<(i64, String)> =
        sqlx::query_as("SELECT id, language FROM work ORDER BY id").fetch_all(&pool).await.unwrap();
    assert_eq!(
        languages,
        [(ferry, "en".to_owned()), (salt, "de".to_owned()), (fresh, "de".to_owned())],
        "en is accepted, the other work stays as it was, and a row inserted without the column gets the default de"
    );
    pool.close().await;

    // The store opens the database (later migrations, if any, run) and reads the languages.
    let store = Store::open(dir.path()).await.unwrap();
    assert_eq!(store.work_language(ferry).await.unwrap(), WorkLanguage::En, "the store reads the changed language");
    assert_eq!(store.work_language(salt).await.unwrap(), WorkLanguage::De, "and the default of the old work");
    let listed: Vec<(i64, WorkLanguage)> =
        store.work_list().await.unwrap().iter().map(|w| (w.id, w.language)).collect();
    assert!(
        listed.contains(&(ferry, WorkLanguage::En)) && listed.contains(&(salt, WorkLanguage::De)),
        "work_list carries the language of every work: {listed:?}"
    );
}
