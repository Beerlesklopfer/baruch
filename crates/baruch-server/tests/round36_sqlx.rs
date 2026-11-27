// SPDX-License-Identifier: AGPL-3.0-or-later
//! Runde 36 „Reste“, Besitz sqlx09: Umstieg auf sqlx 0.9 (Verhalten unverändert).
//!
//! Belegt hier, was nur im ganzen Programm zu sehen ist: Das Lock führt genau eine Ausprägung von sqlx (0.9) und ein
//! `libsqlite3-sys` mit behobenem gebündeltem SQLite; die Ausnahme dafür steht nicht mehr in `deny.toml`; der Server
//! behält `postgres` und TLS mit Systemzertifikaten; und ein Datenordner im Stand von `development` – Store **und**
//! Bibliothek in der gemeinsamen Tabelle `_sqlx_migrations`, mit der Buchführung von sqlx 0.8 und frei erfundenen
//! Daten – startet mit `AppState::open` ohne `VersionMismatch` und verlustfrei.
//!
//! Die Vorlagen `sqlx08_migrations.tsv` (Store und Bibliothek) hat der alte Stand mit sqlx 0.8.6 geschrieben; die
//! Einzelprüfungen je Crate stehen in `baruch-store/tests/sqlx09_migrations.rs` und
//! `baruch-library/tests/sqlx09_migrations.rs`. Die Abfragen gegen pgvector unter 0.9 prüfen `round12_*`.

use std::collections::BTreeMap;
use std::path::Path;
use std::str::FromStr;

use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

const STORE_08: &str = include_str!("../../baruch-store/tests/data/sqlx08_migrations.tsv");
const LIBRARY_08: &str = include_str!("../../baruch-library/tests/fixtures/sqlx08_migrations.tsv");
const LOCK: &str = include_str!("../../../Cargo.lock");
const DENY: &str = include_str!("../../../deny.toml");

/// Die Tabelle, wie sqlx-sqlite 0.8.6 sie angelegt hat (aus einer Datenbank des alten Stands gelesen).
const SQLX08_TABLE: &str = "CREATE TABLE _sqlx_migrations (
    version BIGINT PRIMARY KEY,
    description TEXT NOT NULL,
    installed_on TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    success BOOLEAN NOT NULL,
    checksum BLOB NOT NULL,
    execution_time BIGINT NOT NULL
)";

const BOOK: &str = "#set document(title: \"Die Fähre nach Süden\", author: \"Enno Wibben\")\n\n\
= Anleger\n\nDas Tau liegt nass auf den Planken.\n\n= Überfahrt\n\nMitten im Strom dreht der Wind.\n";

/// Versionen eines Pakets im Lock (`[[package]]` mit `name = "<name>"`).
fn locked_versions(name: &str) -> Vec<String> {
    let needle = format!("name = \"{name}\"");
    let mut lines = LOCK.lines();
    let mut out = Vec::new();
    while let Some(line) = lines.next() {
        if line == needle {
            let version = lines.next().and_then(|l| l.strip_prefix("version = \"")).and_then(|l| l.strip_suffix('"'));
            out.push(version.unwrap_or_else(|| panic!("Lock: nach {needle} folgt die Version")).to_owned());
        }
    }
    out
}

fn minor(version: &str) -> (u32, u32) {
    let mut parts = version.split('.').map(|p| p.parse::<u32>().unwrap());
    (parts.next().unwrap(), parts.next().unwrap())
}

#[test]
fn the_lock_carries_exactly_one_sqlx_09_and_a_fixed_bundled_sqlite() {
    for name in ["sqlx", "sqlx-core", "sqlx-sqlite", "sqlx-postgres", "sqlx-macros", "sqlx-macros-core"] {
        let versions = locked_versions(name);
        assert_eq!(versions.len(), 1, "{name}: genau eine Ausprägung im Baum (keine zweite 0.8 daneben): {versions:?}");
        assert_eq!(minor(&versions[0]), (0, 9), "{name}: gehoben auf 0.9 (ist {})", versions[0]);
    }
    let sqlite = locked_versions("libsqlite3-sys");
    assert_eq!(sqlite.len(), 1, "libsqlite3-sys: genau eine Ausprägung (ein gebündeltes SQLite): {sqlite:?}");
    assert!(
        minor(&sqlite[0]) >= (0, 39),
        "libsqlite3-sys {} trägt den Fix zu RUSTSEC-2026-0459 erst ab 0.39 – sqlx 0.9 erlaubt 0.30 bis 0.40, das Lock \
         darf darum nicht auf eine alte Fassung zurückfallen",
        sqlite[0]
    );
}

#[test]
fn deny_toml_no_longer_ignores_the_sqlite_advisory() {
    assert!(
        !DENY.contains("RUSTSEC-2026-0459"),
        "die Ausnahme für das gebündelte SQLite samt Kommentar ist aus deny.toml entfernt"
    );
    assert!(
        DENY.contains("[advisories]") && DENY.contains("ignore = ["),
        "der Abschnitt der Sicherheitsmeldungen besteht weiter (die Prüfung ist nicht abgeschaltet)"
    );
    assert!(
        !DENY.lines().any(|l| l.trim_start().starts_with("{ id =") && l.contains("sqlite")),
        "keine andere Ausnahme zu SQLite ist an die Stelle getreten"
    );
}

#[test]
fn the_three_crates_ask_for_sqlx_09_and_the_server_keeps_postgres_with_tls() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let line_of = |krate: &str| {
        let manifest = std::fs::read_to_string(root.join(krate).join("Cargo.toml")).unwrap();
        manifest
            .lines()
            .find(|l| l.starts_with("sqlx = "))
            .map(str::to_owned)
            .unwrap_or_else(|| panic!("{krate}: sqlx"))
    };
    for krate in ["baruch-store", "baruch-library", "baruch-server"] {
        let line = line_of(krate);
        assert!(line.contains("version = \"0.9\""), "{krate} verlangt sqlx 0.9: {line}");
        for feature in ["\"runtime-tokio\"", "\"sqlite\"", "\"migrate\"", "\"macros\""] {
            assert!(
                line.contains(feature),
                "{krate} behält das Feature {feature} (nur eine Ausprägung von sqlx): {line}"
            );
        }
        assert!(line.contains("default-features = false"), "{krate}: weiter ohne die Vorgabe-Features (kein any/json)");
    }
    let server = line_of("baruch-server");
    assert!(server.contains("\"postgres\""), "der Server behält Postgres (pgvector): {server}");
    assert!(
        server.contains("\"tls-rustls-ring-native-roots\""),
        "TLS zu pgvector bleibt rustls mit ring und den Zertifikaten des Systems (Hauszertifizierungsstelle): {server}"
    );
    // Dass die Features wirklich wirken, zeigt der Typ: Ohne `postgres` gäbe es ihn nicht, und `PgSslMode::VerifyFull`
    // lässt sich nur mit einem TLS-Feature sinnvoll einstellen.
    let options = sqlx::postgres::PgConnectOptions::new().ssl_mode(sqlx::postgres::PgSslMode::VerifyFull);
    assert!(
        matches!(options.get_ssl_mode(), sqlx::postgres::PgSslMode::VerifyFull),
        "Postgres-Optionen von sqlx 0.9 mit geprüftem TLS lassen sich bilden"
    );
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

/// Spielt die Migrationen einer Vorlage als rohes SQL ein (ohne den Migrator von sqlx 0.9) und schreibt je Migration
/// die Zeile, die sqlx 0.8 geschrieben hat; `off` nennt die Versionen mit ausgeschalteten Fremdschlüsseln.
async fn replay(pool: &SqlitePool, old: &str, migrator: &sqlx::migrate::Migrator, off: &[i64]) -> usize {
    let mut conn = pool.acquire().await.unwrap();
    let mut count = 0;
    for line in old.lines() {
        let cells: Vec<&str> = line.split('\t').collect();
        assert_eq!(cells.len(), 4, "Zeile der Vorlage hat vier Spalten: {line}");
        let version: i64 = cells[0].parse().unwrap();
        let migration = migrator
            .iter()
            .find(|m| m.version == version)
            .unwrap_or_else(|| panic!("Migration {version} der Vorlage ist eingebunden"));
        if off.contains(&version) {
            sqlx::query("PRAGMA foreign_keys = OFF").execute(&mut *conn).await.unwrap();
        }
        sqlx::query("BEGIN").execute(&mut *conn).await.unwrap();
        sqlx::raw_sql(migration.sql.clone())
            .execute(&mut *conn)
            .await
            .unwrap_or_else(|e| panic!("Migration {version}: {e}"));
        sqlx::query(
            "INSERT INTO _sqlx_migrations (version, description, installed_on, success, checksum, execution_time) \
             VALUES (?, ?, '2026-11-20 08:15:00', TRUE, ?, 4711)",
        )
        .bind(version)
        .bind(cells[1])
        .bind(unhex(cells[3]))
        .execute(&mut *conn)
        .await
        .unwrap();
        sqlx::query("COMMIT").execute(&mut *conn).await.unwrap();
        if off.contains(&version) {
            sqlx::query("PRAGMA foreign_keys = ON").execute(&mut *conn).await.unwrap();
        }
        count += 1;
    }
    count
}

/// Frei erfundene Daten direkt in SQLite: ein Werk mit zwei Kapiteln (je eine aktuelle Fassung), ein Beitrag, ein
/// Einfall, ein Titel der Bibliothek mit Zitat.
async fn insert_rows(pool: &SqlitePool) {
    let doc = baruch_typst::parse(BOOK).unwrap();
    let work: i64 = sqlx::query_scalar(
        "INSERT INTO work (slug, kind, title, author, template, source_name, source_sha256, title_key) \
         VALUES ('die-faehre-nach-sueden', 'book', 'Die Fähre nach Süden', 'Enno Wibben', ?, 'faehre.typ', \
                 'sha-faehre', ?) RETURNING id",
    )
    .bind(&doc.template)
    .bind(baruch_store::title_key("Die Fähre nach Süden"))
    .fetch_one(pool)
    .await
    .unwrap();
    for (seq, (heading, body)) in
        doc.headings.iter().zip(["Das Tau liegt nass auf den Planken.", "Mitten im Strom dreht der Wind."]).enumerate()
    {
        let node: i64 = sqlx::query_scalar(
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
        sqlx::query(
            "INSERT INTO revision (node_id, number, body, author_kind, author_name, model, status, note) \
             VALUES (?, 1, ?, 'human', 'Autor', NULL, 'current', 'Import')",
        )
        .bind(node)
        .bind(format!("= {}\n\n{body}\n", heading.title))
        .execute(pool)
        .await
        .unwrap();
        if seq == 0 {
            sqlx::query("INSERT INTO post (node_id, position, author, body) VALUES (?, 0, 'Autor', 'Das Tau knarrt.')")
                .bind(node)
                .execute(pool)
                .await
                .unwrap();
        }
    }
    sqlx::query(
        "INSERT INTO idea (work_id, node_id, note, origin_kind, origin_name, status, created, updated, deleted) \
         VALUES (?, NULL, 'Ein Fährmann, der nicht schwimmen kann.', 'human', 'Autor', 'open', \
                 '2026-06-01T08:00:00.000Z', '2026-06-01T08:00:00.000Z', NULL)",
    )
    .bind(work)
    .execute(pool)
    .await
    .unwrap();
    exec(
        pool,
        "INSERT INTO library_title (id, key, kind, title, authors, year, publisher, place, url, note) VALUES \
         (1, 'wibben2021', 'book', 'Strom und Gegenstrom', 'Wibben, Enno', 2021, 'Deichverlag', 'Leer', '', 'Erfunden')",
    )
    .await;
    exec(
        pool,
        "INSERT INTO library_quote (id, title_id, position, text, page, note) VALUES \
         (1, 1, 1, 'Wer übersetzt, kommt nie am selben Ufer an.', 'S. 7', '')",
    )
    .await;
    exec(pool, "INSERT OR IGNORE INTO store_meta (key, value) VALUES ('seeded', '1')").await;
}

/// Jede Zeile der genannten Tabellen als Text (jede Spalte über `quote()`), sortiert.
async fn dump(pool: &SqlitePool, tables: &[&str]) -> BTreeMap<String, Vec<String>> {
    let mut out = BTreeMap::new();
    for table in tables {
        let columns: Vec<String> = sqlx::query_scalar("SELECT name FROM pragma_table_info(?) ORDER BY cid")
            .bind(table)
            .fetch_all(pool)
            .await
            .unwrap();
        assert!(!columns.is_empty(), "Tabelle {table} gibt es");
        let cells: Vec<String> = columns.iter().map(|c| format!("quote(\"{c}\")")).collect();
        let sql = format!("SELECT {} FROM \"{table}\"", cells.join(" || '|' || "));
        let mut rows: Vec<String> = sqlx::query_scalar(sqlx::AssertSqlSafe(sql.as_str()))
            .fetch_all(pool)
            .await
            .unwrap_or_else(|e| panic!("{sql}: {e}"));
        rows.sort();
        out.insert((*table).to_owned(), rows);
    }
    out
}

/// Tabellen mit dem Inhalt des Autors; `AppState::open` darf darin nichts ändern.
const CONTENT: &[&str] = &[
    "_sqlx_migrations",
    "work",
    "node",
    "revision",
    "post",
    "idea",
    "library_title",
    "library_quote",
    "account",
    "work_member",
];

#[tokio::test]
async fn a_data_folder_of_development_starts_without_version_mismatch_and_losslessly() {
    let data = tempfile::tempdir().unwrap();
    let pool = raw_pool(data.path()).await;
    exec(&pool, SQLX08_TABLE).await;
    assert_eq!(baruch_store::FOREIGN_KEYS_OFF, &[40], "Vorbedingung des Nachbaus: nur 0040 baut Tabellen um");
    let store = replay(&pool, STORE_08, &baruch_store::MIGRATOR, baruch_store::FOREIGN_KEYS_OFF).await;
    let library = replay(&pool, LIBRARY_08, &baruch_library::MIGRATOR, &[]).await;
    assert_eq!((store, library), (43, 8), "Vorbedingung: Store 0001–0054 (43 Dateien) und Bibliothek 0101–0108");
    insert_rows(&pool).await;
    let before = dump(&pool, CONTENT).await;
    assert_eq!(before["_sqlx_migrations"].len(), 51, "Vorbedingung: beide Migratoren stehen in der einen Tabelle");
    for (table, rows) in [("work", 1), ("node", 2), ("revision", 2), ("post", 1), ("idea", 1), ("library_title", 1)] {
        assert_eq!(before[table].len(), rows, "Vorbedingung: die erfundenen Zeilen in {table} liegen im alten Ordner");
    }
    pool.close().await;

    let state = baruch_server::AppState::open(data.path())
        .await
        .unwrap_or_else(|e| panic!("der Server mit sqlx 0.9 startet auf dem Datenordner des alten Stands: {e}"));

    let after = dump(state.store.pool(), CONTENT).await;
    assert_eq!(
        after["_sqlx_migrations"], before["_sqlx_migrations"],
        "die Buchführung beider Migratoren ist unberührt: keine Migration lief noch einmal, keine Prüfsumme wurde \
         umgeschrieben (sonst käme der alte Stand mit dem Ordner nicht mehr zurecht)"
    );
    for table in CONTENT {
        assert_eq!(after[*table], before[*table], "{table}: jede Zeile bleibt byte-gleich, nichts kam dazu");
    }
    let works = state.store.works().await.unwrap();
    assert_eq!(
        works.iter().map(|(_, slug, title)| (slug.as_str(), title.as_str())).collect::<Vec<_>>(),
        [("die-faehre-nach-sueden", "Die Fähre nach Süden")],
        "der Server sieht das alte Werk, und kein Beispielwerk kam dazu"
    );
    let exported = state.store.export_typst(works[0].0).await.unwrap();
    assert!(
        exported.contains("Das Tau liegt nass auf den Planken.")
            && exported.contains("Mitten im Strom dreht der Wind."),
        "der Text beider Kapitel ist über den Store lesbar: {exported}"
    );
    let titles = baruch_library::list_titles(state.store.pool()).await.unwrap();
    assert_eq!(titles.len(), 1, "die Bibliothek zeigt den alten Titel und sät keine Beispiele dazu");
}
