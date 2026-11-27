// SPDX-License-Identifier: AGPL-3.0-or-later
//! Runde 36 (sqlx09): Der Sprung von sqlx 0.8 auf 0.9 ändert nichts an den Migrationen des Stores.
//!
//! Grundlage ist `tests/data/sqlx08_migrations.tsv`: die Zeilen von `_sqlx_migrations` (Version, Beschreibung,
//! `success`, Prüfsumme als Hex), wie **sqlx 0.8.6** sie auf dem Stand von `development` (0001–0054) geschrieben hat –
//! erzeugt vor dem Umbau mit dem alten Stand (`baruch_store::migrate` auf einer leeren Datenbank).
//!
//! Belegt: Jede Migrationsdatei ist unverändert (SHA-384 der Datei = alte Prüfsumme); der eingebettete Migrator von
//! 0.9 trägt dieselben Versionen, Beschreibungen und Prüfsummen; eine nachgebaute Datenbank im Stand von
//! `development` mit der Buchführung von 0.8 und frei erfundenen Daten öffnet mit [`Store::open`] ohne
//! `VersionMismatch`, ohne dass eine Migration noch einmal läuft, und verlustfrei; eine wirklich geänderte Prüfsumme
//! lehnt der Store weiter ab; fremde Versionen in der gemeinsamen Tabelle (Bibliothek) stören nicht (`ignore_missing`).

use std::collections::BTreeMap;
use std::path::Path;
use std::str::FromStr;

use baruch_store::{Store, StoreError};
use sha2::{Digest, Sha384};
use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

const SQLX08: &str = include_str!("data/sqlx08_migrations.tsv");

/// Die Tabelle, wie sqlx-sqlite 0.8.6 sie angelegt hat (aus einer Datenbank des alten Stands gelesen).
const SQLX08_TABLE: &str = "CREATE TABLE _sqlx_migrations (
    version BIGINT PRIMARY KEY,
    description TEXT NOT NULL,
    installed_on TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    success BOOLEAN NOT NULL,
    checksum BLOB NOT NULL,
    execution_time BIGINT NOT NULL
)";

const BOOK: &str = "#set document(title: \"Der Speicher am Fleet\", author: \"Jorinde Haack\")\n\n\
= Luke\n\nDer Kran hängt seit dem Herbst still.\n\n= Kontor\n\nIm Kontor riecht es nach Tinte und Tee.\n";

struct Old {
    version: i64,
    description: String,
    checksum: String,
}

/// Die Zeilen der Buchführung von sqlx 0.8.
fn old_rows() -> Vec<Old> {
    SQLX08
        .lines()
        .map(|line| {
            let cells: Vec<&str> = line.split('\t').collect();
            assert_eq!(cells.len(), 4, "Zeile der Vorlage hat vier Spalten: {line}");
            assert_eq!(cells[2], "1", "Vorlage: jede Migration war erfolgreich ({line})");
            Old { version: cells[0].parse().unwrap(), description: cells[1].to_owned(), checksum: cells[3].to_owned() }
        })
        .collect()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn unhex(text: &str) -> Vec<u8> {
    (0..text.len()).step_by(2).map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap()).collect()
}

async fn raw_pool(dir: &Path) -> SqlitePool {
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

async fn scalar(pool: &SqlitePool, sql: &str) -> i64 {
    sqlx::query_scalar(sqlx::AssertSqlSafe(sql)).fetch_one(pool).await.unwrap_or_else(|e| panic!("{sql}: {e}"))
}

/// Baut die Datenbank im Stand von `development` nach, **ohne** den Migrator von sqlx 0.9: Tabelle der Buchführung
/// wie von 0.8 angelegt, jede Migration 0001–0054 als rohes SQL in einer Transaktion (0040 wie in
/// `baruch_store::migrate` mit ausgeschalteten Fremdschlüsseln), dazu je Migration die Zeile aus der Vorlage.
async fn build_development(pool: &SqlitePool) {
    assert_eq!(baruch_store::FOREIGN_KEYS_OFF, &[40], "Vorbedingung des Nachbaus: nur 0040 baut Tabellen um");
    exec(pool, SQLX08_TABLE).await;
    let mut conn = pool.acquire().await.unwrap();
    for old in old_rows() {
        let migration = baruch_store::MIGRATOR
            .iter()
            .find(|m| m.version == old.version)
            .unwrap_or_else(|| panic!("Migration {} der Vorlage ist eingebunden", old.version));
        let off = baruch_store::FOREIGN_KEYS_OFF.contains(&old.version);
        if off {
            sqlx::query("PRAGMA foreign_keys = OFF").execute(&mut *conn).await.unwrap();
        }
        sqlx::query("BEGIN").execute(&mut *conn).await.unwrap();
        sqlx::raw_sql(migration.sql.clone())
            .execute(&mut *conn)
            .await
            .unwrap_or_else(|e| panic!("Migration {}: {e}", old.version));
        sqlx::query(
            "INSERT INTO _sqlx_migrations (version, description, installed_on, success, checksum, execution_time) \
             VALUES (?, ?, '2026-11-20 08:15:00', TRUE, ?, 4711)",
        )
        .bind(old.version)
        .bind(&old.description)
        .bind(unhex(&old.checksum))
        .execute(&mut *conn)
        .await
        .unwrap();
        sqlx::query("COMMIT").execute(&mut *conn).await.unwrap();
        if off {
            sqlx::query("PRAGMA foreign_keys = ON").execute(&mut *conn).await.unwrap();
        }
    }
}

/// Frei erfundene Daten direkt in SQLite: ein Werk mit zwei Kapiteln, zwei Beiträge, drei Einfälle (einer gelöscht,
/// einer ohne Werk), zwei Konten mit Sitzung, Fehlversuch und Freigabe, eigene Einträge in `store_meta`.
async fn insert_rows(pool: &SqlitePool) {
    let doc = baruch_typst::parse(BOOK).unwrap();
    let work: i64 = sqlx::query_scalar(
        "INSERT INTO work (slug, kind, title, author, template, source_name, source_sha256, title_key) \
         VALUES ('der-speicher-am-fleet', 'book', 'Der Speicher am Fleet', 'Jorinde Haack', ?, 'speicher.typ', \
                 'sha-speicher', ?) RETURNING id",
    )
    .bind(&doc.template)
    .bind(baruch_store::title_key("Der Speicher am Fleet"))
    .fetch_one(pool)
    .await
    .unwrap();
    let mut nodes = Vec::new();
    for (seq, heading) in doc.headings.iter().enumerate() {
        let id: i64 = sqlx::query_scalar(
            "INSERT INTO node (work_id, seq, parent_seq, position, number, title, kind) \
             VALUES (?, ?, NULL, ?, ?, ?, 'chapter') RETURNING id",
        )
        .bind(work)
        .bind(seq as i64)
        .bind(seq as i64)
        .bind((seq + 1).to_string())
        .bind(&heading.title)
        .fetch_one(pool)
        .await
        .unwrap();
        nodes.push(id);
    }
    assert_eq!(nodes.len(), 2, "Vorbedingung: zwei Kapitel");
    for (position, (author, body)) in
        [("Autor", "Die Luke braucht noch ein Geräusch."), ("@lektor", "Der Tee trägt den Absatz.")].iter().enumerate()
    {
        sqlx::query("INSERT INTO post (node_id, position, author, body) VALUES (?, ?, ?, ?)")
            .bind(nodes[0])
            .bind(position as i64)
            .bind(author)
            .bind(body)
            .execute(pool)
            .await
            .unwrap();
    }
    for (work_id, node, note, deleted) in [
        (Some(work), Some(nodes[0]), "Möwen über dem Dach.", None),
        (Some(work), None, "Verworfen: der Zöllner.", Some("2026-05-02T09:00:00.000Z")),
        (None, None, "Ohne Werk: ein Leuchtfeuer.", None),
    ] {
        sqlx::query(
            "INSERT INTO idea (work_id, node_id, note, origin_kind, origin_name, status, created, updated, deleted) \
             VALUES (?, ?, ?, 'human', 'Autor', 'open', '2026-05-01T08:00:00.000Z', '2026-05-01T08:00:00.000Z', ?)",
        )
        .bind(work_id)
        .bind(node)
        .bind(note)
        .bind(deleted)
        .execute(pool)
        .await
        .unwrap();
    }
    exec(
        pool,
        "INSERT INTO account (name, name_key, slug, display, kind, password, active) VALUES \
         ('Jorinde', 'jorinde', 'jorinde', 'Jorinde Haack', 'author', '$argon2id$test', 1), \
         ('Malte', 'malte', 'malte', 'Malte Brodersen', 'lektor', '$argon2id$test', 1)",
    )
    .await;
    let token = "c".repeat(64);
    exec(pool, &format!("INSERT INTO account_session (token_hash, account_id) VALUES ('{token}', 2)")).await;
    exec(pool, "INSERT INTO login_attempt (scope, failures) VALUES ('konto:malte', 3)").await;
    exec(pool, &format!("INSERT INTO work_member (work_id, account_id, role) VALUES ({work}, 2, 'lektor')")).await;
    // Eine Datenbank im Betrieb hat der Store schon einmal geöffnet: Die Markierung „gesät“ steht.
    exec(pool, "INSERT OR IGNORE INTO store_meta (key, value) VALUES ('seeded', '1')").await;
    exec(pool, "INSERT INTO store_meta (key, value) VALUES ('probe.r36', 'bleibt')").await;
}

/// Jede Zeile jeder gewöhnlichen Tabelle als Text (jede Spalte über `quote()`), sortiert – auch die Buchführung
/// `_sqlx_migrations` und die Schattentabellen der Volltextindizes.
async fn dump_all(pool: &SqlitePool) -> BTreeMap<String, Vec<String>> {
    let names: Vec<String> = sqlx::query_scalar(
        "SELECT name FROM sqlite_master WHERE type = 'table' AND sql NOT LIKE 'CREATE VIRTUAL%' ORDER BY name",
    )
    .fetch_all(pool)
    .await
    .unwrap();
    let mut out = BTreeMap::new();
    for table in names {
        let columns: Vec<String> = sqlx::query_scalar("SELECT name FROM pragma_table_info(?) ORDER BY cid")
            .bind(&table)
            .fetch_all(pool)
            .await
            .unwrap();
        let cells: Vec<String> = columns.iter().map(|c| format!("quote(\"{c}\")")).collect();
        let sql = format!("SELECT {} FROM \"{table}\"", cells.join(" || '|' || "));
        let mut rows: Vec<String> = sqlx::query_scalar(sqlx::AssertSqlSafe(sql.as_str()))
            .fetch_all(pool)
            .await
            .unwrap_or_else(|e| panic!("{sql}: {e}"));
        rows.sort();
        out.insert(table, rows);
    }
    out
}

/// Alle Schemaobjekte als „typ name: sql“.
async fn schema(pool: &SqlitePool) -> Vec<String> {
    sqlx::query_scalar(
        "SELECT type || ' ' || name || ': ' || coalesce(sql, '') FROM sqlite_master \
         WHERE name NOT LIKE 'sqlite_%' ORDER BY type, name",
    )
    .fetch_all(pool)
    .await
    .unwrap()
}

#[test]
fn no_migration_file_changed_and_the_embedded_checksums_are_those_of_sqlx_08() {
    let old = old_rows();
    assert_eq!(old.len(), 43, "Vorlage: der Stand von development hat 43 Store-Migrationen");
    assert_eq!(
        (old.first().unwrap().version, old.last().unwrap().version),
        (1, 54),
        "Vorlage: 0001 bis 0054, aufsteigend"
    );
    let folder = Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations");
    let mut files: Vec<String> =
        std::fs::read_dir(&folder).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    files.sort();
    for row in &old {
        let prefix = format!("{:04}_", row.version);
        let name = files
            .iter()
            .find(|f| f.starts_with(&prefix) && f.ends_with(".sql"))
            .unwrap_or_else(|| panic!("die Datei der Migration {prefix} liegt noch im Ordner"));
        let bytes = std::fs::read(folder.join(name)).unwrap();
        assert_eq!(
            hex(&Sha384::digest(&bytes)),
            row.checksum,
            "{name}: Die Datei ist Byte für Byte die, über die sqlx 0.8 die Prüfsumme gebildet hat (SHA-384) – \
             keine vorhandene Migration ist geändert"
        );
        let migration = baruch_store::MIGRATOR
            .iter()
            .find(|m| m.version == row.version)
            .unwrap_or_else(|| panic!("{name} ist auch unter sqlx 0.9 eingebettet"));
        assert_eq!(
            hex(&migration.checksum),
            row.checksum,
            "{name}: sqlx 0.9 bettet dieselbe Prüfsumme ein wie 0.8 – sonst meldete jede vorhandene Datenbank \
             VersionMismatch"
        );
        assert_eq!(
            migration.description, row.description,
            "{name}: die Beschreibung (aus dem Dateinamen) leitet 0.9 wie 0.8 ab"
        );
        assert_eq!(migration.sql.as_str().as_bytes(), bytes, "{name}: eingebettet ist genau der Inhalt der Datei");
        assert!(!migration.no_tx, "{name}: läuft wie bisher in einer Transaktion (kein `-- no-transaction`)");
    }
    let versions: Vec<i64> = baruch_store::MIGRATOR.iter().map(|m| m.version).filter(|v| *v <= 54).collect();
    let expected: Vec<i64> = old.iter().map(|r| r.version).collect();
    assert_eq!(versions, expected, "bis 0054 trägt der Migrator genau die Versionen des alten Stands, aufsteigend");
    assert!(baruch_store::MIGRATOR.ignore_missing, "der Migrator duldet weiter fremde Versionen (Bibliothek)");
    assert!(baruch_store::MIGRATOR.locking, "die Sperre während der Migration bleibt wie in der Vorgabe an");
    assert_eq!(
        baruch_store::MIGRATOR.table_name,
        "_sqlx_migrations",
        "die Buchführung bleibt in der Tabelle, die vorhandene Datenbanken schon haben (0.9 könnte sie umbenennen)"
    );
    assert!(baruch_store::MIGRATOR.create_schemas.is_empty(), "kein Schema wird zusätzlich angelegt");
}

#[tokio::test]
async fn a_database_of_development_opens_without_version_mismatch_and_losslessly() {
    let dir = tempfile::tempdir().unwrap();
    let pool = raw_pool(dir.path()).await;
    build_development(&pool).await;
    insert_rows(&pool).await;
    // Die Bibliothek teilt sich die Tabelle: fremde Versionen (hier erfunden, 0101 und 0102) stehen in jeder
    // wirklichen Datenbank und dürfen den Store nicht stören.
    for version in [101, 102] {
        exec(
            &pool,
            &format!(
                "INSERT INTO _sqlx_migrations (version, description, installed_on, success, checksum, execution_time) \
                 VALUES ({version}, 'fremd', '2026-11-20 08:15:01', TRUE, x'00ff', 1)"
            ),
        )
        .await;
    }
    assert_eq!(scalar(&pool, "SELECT count(*) FROM _sqlx_migrations").await, 45, "Vorbedingung: 43 eigene, 2 fremde");
    let rows_before = dump_all(&pool).await;
    let schema_before = schema(&pool).await;
    for (table, expected) in [
        ("work", 1),
        ("node", 2),
        ("post", 2),
        ("idea", 3),
        ("account", 2),
        ("account_session", 1),
        ("login_attempt", 1),
        ("work_member", 1),
        ("_sqlx_migrations", 45),
    ] {
        assert_eq!(
            rows_before.get(table).map(Vec::len),
            Some(expected),
            "Vorbedingung: die erfundenen Zeilen in {table} liegen in der alten Datenbank"
        );
    }
    pool.close().await;

    let store = Store::open(dir.path())
        .await
        .expect("der Store mit sqlx 0.9 öffnet die Datenbank des alten Stands (kein VersionMismatch)");

    let rows_after = dump_all(store.pool()).await;
    assert_eq!(
        rows_after.get("_sqlx_migrations"),
        rows_before.get("_sqlx_migrations"),
        "die Buchführung ist unberührt: keine Migration lief noch einmal (installed_on, execution_time gleich), \
         keine Prüfsumme wurde umgeschrieben, die fremden Versionen stehen noch da"
    );
    for (table, before) in &rows_before {
        assert_eq!(rows_after.get(table), Some(before), "jede Zeile von {table} bleibt byte-gleich");
    }
    assert_eq!(
        rows_after.keys().collect::<Vec<_>>(),
        rows_before.keys().collect::<Vec<_>>(),
        "keine Tabelle kam dazu oder fiel weg"
    );
    assert_eq!(
        schema(store.pool()).await,
        schema_before,
        "kein Schemaobjekt ist geändert – auch die Tabelle _sqlx_migrations behält die Gestalt von 0.8"
    );
    let works = store.works().await.unwrap();
    assert_eq!(
        works.iter().map(|(_, slug, title)| (slug.as_str(), title.as_str())).collect::<Vec<_>>(),
        [("der-speicher-am-fleet", "Der Speicher am Fleet")],
        "der Store liest das alte Werk, und kein Beispielwerk kam dazu"
    );
    assert_eq!(store.account_count().await.unwrap(), 2, "die Konten sind über den Store lesbar");

    // Ein zweites Öffnen ändert ebenfalls nichts (wiederholbar wie bisher).
    drop(store);
    let again = Store::open(dir.path()).await.expect("zweites Öffnen");
    assert_eq!(dump_all(again.pool()).await, rows_after, "auch das zweite Öffnen lässt jede Zeile stehen");
}

#[tokio::test]
async fn a_really_changed_checksum_is_still_refused() {
    let dir = tempfile::tempdir().unwrap();
    let pool = raw_pool(dir.path()).await;
    build_development(&pool).await;
    insert_rows(&pool).await;
    exec(&pool, "UPDATE _sqlx_migrations SET checksum = x'00' || substr(checksum, 2) WHERE version = 12").await;
    let rows_before = dump_all(&pool).await;
    pool.close().await;

    let error = Store::open(dir.path()).await.err().expect("eine fremde Prüfsumme lehnt der Store ab");
    assert!(
        matches!(&error, StoreError::Migrate(sqlx::migrate::MigrateError::VersionMismatch(12))),
        "die Prüfung der Prüfsummen ist unter 0.9 nicht abgeschaltet (Gegenprobe zum Test davor): {error}"
    );
    let pool = raw_pool(dir.path()).await;
    assert_eq!(dump_all(&pool).await, rows_before, "nach der Ablehnung ist nichts geschrieben");
}

#[tokio::test]
async fn a_fresh_database_gets_the_same_bookkeeping_as_under_sqlx_08() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).await.unwrap();
    let rows: Vec<(i64, String, bool, Vec<u8>)> = sqlx::query_as(
        "SELECT version, description, success, checksum FROM _sqlx_migrations WHERE version <= 54 ORDER BY version",
    )
    .fetch_all(store.pool())
    .await
    .unwrap();
    let old = old_rows();
    assert_eq!(rows.len(), old.len(), "eine neue Datenbank trägt bis 0054 dieselben Versionen wie unter 0.8");
    for ((version, description, success, checksum), row) in rows.iter().zip(&old) {
        assert_eq!(
            (*version, description.as_str(), *success, hex(checksum).as_str()),
            (row.version, row.description.as_str(), true, row.checksum.as_str()),
            "Zeile {version}: was 0.9 schreibt, ist das, was 0.8 schrieb – eine neue Datenbank ließe sich auch mit \
             dem alten Stand wieder öffnen"
        );
    }
    let table: String = sqlx::query_scalar("SELECT sql FROM sqlite_master WHERE name = '_sqlx_migrations'")
        .fetch_one(store.pool())
        .await
        .unwrap();
    let squeeze = |s: &str| s.split_whitespace().collect::<Vec<_>>().join(" ");
    assert_eq!(
        squeeze(&table).replace("IF NOT EXISTS ", ""),
        squeeze(SQLX08_TABLE),
        "auch die Tabelle der Buchführung legt 0.9 mit denselben Spalten an"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("PRAGMA foreign_keys").fetch_one(store.pool()).await.unwrap(),
        1,
        "nach 0040 (FOREIGN_KEYS_OFF) sind die Fremdschlüssel wieder eingeschaltet"
    );
}
