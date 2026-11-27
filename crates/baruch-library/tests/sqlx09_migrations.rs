// SPDX-License-Identifier: AGPL-3.0-or-later
//! Round 36 (sqlx09): the jump from sqlx 0.8 to 0.9 changes nothing about the migrations of the library.
//!
//! Basis is `tests/fixtures/sqlx08_migrations.tsv`: version, description, `success` and checksum (hex) of the
//! migrations 0101–0108, as **sqlx 0.8.6** embedded them on the state of `development` and wrote them into
//! `_sqlx_migrations` – generated before the conversion with the old state.
//!
//! Proven: every migration file is unchanged (SHA-384 of the file = old checksum) and embedded under 0.9 with the
//! same checksum; a rebuilt database in the state of `development` (core schema from `fixtures/core_schema.sql`,
//! library 0101–0108 as raw SQL, bookkeeping of 0.8, freely invented data including the full-text index) runs
//! through [`baruch_library::migrate`] without `VersionMismatch` and without a changed row; the versions of the
//! store in the same table do not interfere (`ignore_missing`); a changed checksum is refused.

use std::collections::BTreeMap;
use std::path::Path;
use std::str::FromStr;

use baruch_library::{LibraryError, migrate};
use sha2::{Digest, Sha384};
use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

const SQLX08: &str = include_str!("fixtures/sqlx08_migrations.tsv");
const CORE_SCHEMA: &str = include_str!("fixtures/core_schema.sql");

/// The table as sqlx-sqlite 0.8.6 created it (read from a database of the old state).
const SQLX08_TABLE: &str = "CREATE TABLE _sqlx_migrations (
    version BIGINT PRIMARY KEY,
    description TEXT NOT NULL,
    installed_on TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    success BOOLEAN NOT NULL,
    checksum BLOB NOT NULL,
    execution_time BIGINT NOT NULL
)";

/// (version, description, checksum as hex) from the bookkeeping of sqlx 0.8.
fn old_rows() -> Vec<(i64, String, String)> {
    SQLX08
        .lines()
        .map(|line| {
            let cells: Vec<&str> = line.split('\t').collect();
            assert_eq!(cells.len(), 4, "Zeile der Vorlage hat vier Spalten: {line}");
            assert_eq!(cells[2], "1", "Vorlage: jede Migration war erfolgreich ({line})");
            (cells[0].parse().unwrap(), cells[1].to_owned(), cells[3].to_owned())
        })
        .collect()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn unhex(text: &str) -> Vec<u8> {
    (0..text.len()).step_by(2).map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap()).collect()
}

async fn open(dir: &Path) -> SqlitePool {
    let options = SqliteConnectOptions::from_str("sqlite:")
        .unwrap()
        .filename(dir.join("baruch.sqlite"))
        .create_if_missing(true)
        .foreign_keys(true);
    SqlitePoolOptions::new().max_connections(1).connect_with(options).await.unwrap()
}

async fn exec(pool: &SqlitePool, sql: &str) {
    sqlx::query(sqlx::AssertSqlSafe(sql)).execute(pool).await.unwrap_or_else(|e| panic!("{sql}: {e}"));
}

/// State of `development` without the migrator of sqlx 0.9: core schema, bookkeeping table as from 0.8, every
/// migration of the library as raw SQL in a transaction with its row from the template; plus two rows of the store
/// (invented), as they stand in the same table in every real database.
async fn build_development(pool: &SqlitePool) {
    sqlx::raw_sql(CORE_SCHEMA).execute(pool).await.unwrap();
    exec(pool, SQLX08_TABLE).await;
    exec(
        pool,
        "INSERT INTO _sqlx_migrations (version, description, installed_on, success, checksum, execution_time) VALUES \
         (1, 'core', '2026-11-20 08:15:00', TRUE, x'0a0b', 7), (54, 'work member', '2026-11-20 08:15:00', TRUE, x'0c', 7)",
    )
    .await;
    let mut conn = pool.acquire().await.unwrap();
    for (version, description, checksum) in old_rows() {
        let migration = baruch_library::MIGRATOR
            .iter()
            .find(|m| m.version == version)
            .unwrap_or_else(|| panic!("Migration {version} der Vorlage ist eingebunden"));
        sqlx::query("BEGIN").execute(&mut *conn).await.unwrap();
        sqlx::raw_sql(migration.sql.clone())
            .execute(&mut *conn)
            .await
            .unwrap_or_else(|e| panic!("Migration {version}: {e}"));
        sqlx::query(
            "INSERT INTO _sqlx_migrations (version, description, installed_on, success, checksum, execution_time) \
             VALUES (?, ?, '2026-11-20 08:15:02', TRUE, ?, 4711)",
        )
        .bind(version)
        .bind(&description)
        .bind(unhex(&checksum))
        .execute(&mut *conn)
        .await
        .unwrap();
        sqlx::query("COMMIT").execute(&mut *conn).await.unwrap();
    }
}

/// Freely invented data in the state 0108.
async fn insert_rows(pool: &SqlitePool) {
    exec(
        pool,
        "INSERT INTO work (id, slug, kind, title, author) VALUES (1, 'hallig', 'book', 'Auf der Hallig', 'Sönke Peters')",
    )
    .await;
    exec(
        pool,
        "INSERT INTO library_title (id, key, kind, title, authors, year, publisher, place, url, note) VALUES \
         (1, 'peters2019', 'book', 'Salz im Gras', 'Peters, Sönke', 2019, 'Warftverlag', 'Hooge', '', 'Erfunden'), \
         (2, 'mg-priel', 'other', 'Der Priel bei Nacht', '', NULL, '', '', 'https://bib.example/priel', '')",
    )
    .await;
    exec(
        pool,
        "INSERT INTO library_quote (id, title_id, position, text, page, note) VALUES \
         (1, 1, 1, 'Das Gras schmeckt nach dem Meer.', 'S. 12', '')",
    )
    .await;
    exec(
        pool,
        "INSERT INTO library_highlight (id, title_id, quote_id, locator, start_char, end_char, text, kind, note, \
           author_kind, author_name, model, status) VALUES \
         (1, 1, 1, 'S. 12', 4, 8, 'Gras', 'core', '', 'human', 'Autor', NULL, 'active'), \
         (2, 2, NULL, '', 4, 9, 'Priel', 'quote', '', 'human', 'Autor', NULL, 'active')",
    )
    .await;
    exec(
        pool,
        "INSERT INTO library_title_remote (title_id, way, collection, document) VALUES (2, 'multigpt', 'Watt', 'pr-1')",
    )
    .await;
    exec(pool, "INSERT INTO library_highlight_check (highlight_id, result, message) VALUES (2, 'found', '')").await;
    exec(
        pool,
        "INSERT INTO library_title_text (title_id, source_name, mime, sha256, pages, text, created) VALUES \
         (2, 'priel.pdf', 'application/pdf', 'sha-priel', 1, 'Bei Nacht steht das Wasser still im Priel.', \
          '2026-11-20T08:20:00.000Z')",
    )
    .await;
}

/// Every row of every table (also `_sqlx_migrations` and the shadow tables of the full-text index) as text.
async fn snapshot(pool: &SqlitePool) -> BTreeMap<String, Vec<String>> {
    let tables: Vec<String> = sqlx::query_scalar(
        "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
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

#[test]
fn no_migration_file_changed_and_the_embedded_checksums_are_those_of_sqlx_08() {
    let old = old_rows();
    assert_eq!(
        old.iter().map(|(v, ..)| *v).collect::<Vec<_>>(),
        (101..=108).collect::<Vec<i64>>(),
        "Vorlage: die Bibliothek trägt auf development 0101 bis 0108"
    );
    let folder = Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations");
    let files: Vec<String> =
        std::fs::read_dir(&folder).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    for (version, description, checksum) in &old {
        let prefix = format!("{version:04}_");
        let name = files
            .iter()
            .find(|f| f.starts_with(&prefix) && f.ends_with(".sql"))
            .unwrap_or_else(|| panic!("die Datei der Migration {prefix} liegt noch im Ordner"));
        let bytes = std::fs::read(folder.join(name)).unwrap();
        assert_eq!(
            &hex(&Sha384::digest(&bytes)),
            checksum,
            "{name}: Die Datei ist Byte für Byte die, über die sqlx 0.8 die Prüfsumme gebildet hat (SHA-384) – \
             keine vorhandene Migration ist geändert"
        );
        let migration = baruch_library::MIGRATOR
            .iter()
            .find(|m| m.version == *version)
            .unwrap_or_else(|| panic!("{name} ist auch unter sqlx 0.9 eingebettet"));
        assert_eq!(
            &hex(&migration.checksum),
            checksum,
            "{name}: sqlx 0.9 bettet dieselbe Prüfsumme ein wie 0.8 – sonst meldete jede vorhandene Datenbank \
             VersionMismatch"
        );
        assert_eq!(&migration.description, description, "{name}: die Beschreibung leitet 0.9 wie 0.8 ab");
        assert_eq!(migration.sql.as_str().as_bytes(), bytes, "{name}: eingebettet ist genau der Inhalt der Datei");
    }
    assert!(baruch_library::MIGRATOR.ignore_missing, "der Migrator duldet weiter die Versionen des Stores");
    assert_eq!(
        baruch_library::MIGRATOR.table_name,
        "_sqlx_migrations",
        "Bibliothek und Store teilen sich weiter die eine Tabelle der Buchführung"
    );
}

#[tokio::test]
async fn a_database_of_development_migrates_without_version_mismatch_and_losslessly() {
    let dir = tempfile::tempdir().unwrap();
    let pool = open(dir.path()).await;
    build_development(&pool).await;
    insert_rows(&pool).await;
    let before = snapshot(&pool).await;
    let schema_before = schema(&pool).await;
    assert_eq!(before["_sqlx_migrations"].len(), 10, "Vorbedingung: acht eigene Versionen und zwei des Stores");
    assert!(
        before["library_title"].len() == 2
            && before["library_quote"].len() == 1
            && before["library_highlight"].len() == 2,
        "Vorbedingung: die erfundenen Titel, Zitate und Hervorhebungen liegen in der alten Datenbank: {before:?}"
    );
    let hits: i64 = sqlx::query_scalar("SELECT count(*) FROM search_fts WHERE search_fts MATCH 'Gras'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(hits >= 1, "Vorbedingung: der Volltextindex der alten Datenbank findet das erfundene Zitat");

    migrate(&pool)
        .await
        .expect("die Bibliothek mit sqlx 0.9 nimmt die Datenbank des alten Stands (kein VersionMismatch)");

    let after = snapshot(&pool).await;
    assert_eq!(
        after.get("_sqlx_migrations"),
        before.get("_sqlx_migrations"),
        "die Buchführung ist unberührt: nichts lief noch einmal, keine Prüfsumme wurde umgeschrieben, die Versionen \
         des Stores stehen noch da"
    );
    assert_eq!(
        after, before,
        "jede Zeile jeder Tabelle bleibt gleich (auch der Volltextindex), keine Tabelle kam dazu"
    );
    assert_eq!(schema(&pool).await, schema_before, "kein Schemaobjekt ist geändert");
    let again: i64 = sqlx::query_scalar("SELECT count(*) FROM search_fts WHERE search_fts MATCH 'Gras'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(again, hits, "die Volltextsuche (FTS5 des neuen gebündelten SQLite) findet dasselbe wie vorher");
}

#[tokio::test]
async fn a_really_changed_checksum_is_still_refused() {
    let dir = tempfile::tempdir().unwrap();
    let pool = open(dir.path()).await;
    build_development(&pool).await;
    exec(&pool, "UPDATE _sqlx_migrations SET checksum = x'00' || substr(checksum, 2) WHERE version = 104").await;
    let error = migrate(&pool).await.expect_err("eine fremde Prüfsumme lehnt die Bibliothek ab");
    assert!(
        matches!(&error, LibraryError::Migrate(sqlx::migrate::MigrateError::VersionMismatch(104))),
        "die Prüfung der Prüfsummen ist unter 0.9 nicht abgeschaltet (Gegenprobe zum Test davor): {error}"
    );
}
