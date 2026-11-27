// SPDX-License-Identifier: AGPL-3.0-or-later
//! Migration 0044 (typeset-view settings per work, Round 18c) on a reproduced database in the state of development
//! before Round 18 (store up to 0043, library complete) with a freely invented work: chapters with current revision and
//! proposal, template revision, post, image, time setting.
//!
//! Evidenced: 0044 is not in `FOREIGN_KEYS_OFF` (creates only a new table); every row of every old table and the schema
//! of every old table stay the same; new is exactly `work_sheet` (empty, no index, no trigger); the CHECK rules and the
//! foreign key take effect in the table already; `Store::open` afterwards succeeds, the export is byte-identical,
//! without a row nothing is set; settings can be set; deleting the work takes the row along (ON DELETE CASCADE, without
//! `Store::delete_work` knowing it); no foreign key violations.

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::path::Path;
use std::str::FromStr;

use baruch_store::{SheetSettings, Store, title_key};
use sqlx::SqlitePool;
use sqlx::migrate::Migrator;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

const BOOK: &str = "#set document(title: \"Die Mühle am Bach\", author: \"Rita Korn\")\n\n\
= Mahlen\n\nDas Rad dreht sich.\n\n= Rasten\n\nAm Sonntag steht es still.\n";

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

async fn scalar(pool: &SqlitePool, sql: &str) -> i64 {
    sqlx::query_scalar(sqlx::AssertSqlSafe(sql)).fetch_one(pool).await.unwrap_or_else(|e| panic!("{sql}: {e}"))
}

async fn exec(pool: &SqlitePool, sql: &str) {
    sqlx::query(sqlx::AssertSqlSafe(sql)).execute(pool).await.unwrap_or_else(|e| panic!("{sql}: {e}"));
}

/// Store up to 0043 (0040 as in `baruch_store::migrate` with foreign keys switched off) and the library: the state of
/// development before Round 18.
async fn migrate_upto_0043(pool: &SqlitePool) {
    assert!(baruch_store::MIGRATOR.iter().any(|m| m.version == 44), "die neue Migration ist 0044");
    assert!(
        !baruch_store::FOREIGN_KEYS_OFF.contains(&44),
        "0044 legt nur eine neue Tabelle an und baut keine um (Vertrag R18 §0)"
    );
    store_migrator(|v| v < 40).run(pool).await.unwrap();
    {
        let mut conn = pool.acquire().await.unwrap();
        sqlx::query("PRAGMA foreign_keys = OFF").execute(&mut *conn).await.unwrap();
        store_migrator(|v| v == 40).run(&mut *conn).await.unwrap();
        sqlx::query("PRAGMA foreign_keys = ON").execute(&mut *conn).await.unwrap();
    }
    store_migrator(|v| (41..=43).contains(&v)).run(pool).await.unwrap();
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../baruch-library/migrations");
    let mut library = Migrator::new(dir).await.unwrap();
    library.ignore_missing = true;
    library.run(pool).await.unwrap();
    let highest = scalar(pool, "SELECT max(version) FROM _sqlx_migrations WHERE version < 100").await;
    assert_eq!(highest, 43, "Stand davor: Store bis 0043");
}

/// Work with two chapters, proposal, template revision, post, image and time setting directly in SQLite.
async fn insert_rows(pool: &SqlitePool) -> i64 {
    exec(pool, "INSERT INTO store_meta (key, value) VALUES ('seeded', '1') ON CONFLICT DO NOTHING").await;
    let doc = baruch_typst::parse(BOOK).unwrap();
    let work: i64 = sqlx::query_scalar(
        "INSERT INTO work (slug, kind, title, author, template, source_name, source_sha256, title_key) \
         VALUES ('die-muehle-am-bach', 'book', 'Die Mühle am Bach', 'Rita Korn', ?, 'muehle.typ', 'sha-muehle', ?) \
         RETURNING id",
    )
    .bind(&doc.template)
    .bind(title_key("Die Mühle am Bach"))
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO work_template_revision (work_id, number, body, author_name, note) VALUES (?, 1, ?, 'Rita', 'Anfang')",
    )
    .bind(work)
    .bind(&doc.template)
    .execute(pool)
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
        sqlx::query(
            "INSERT INTO revision (node_id, number, body, author_kind, author_name, model, status, note) \
             VALUES (?, 1, ?, 'human', 'Rita', NULL, 'current', 'Import')",
        )
        .bind(id)
        .bind(&heading.body)
        .execute(pool)
        .await
        .unwrap();
        nodes.push(id);
    }
    sqlx::query(
        "INSERT INTO revision (node_id, number, body, author_kind, author_name, model, status, note, based_on) \
         VALUES (?, 2, ?, 'assistant', '@lektor', 'probe-a', 'proposed', 'Vorschlag', 1)",
    )
    .bind(nodes[1])
    .bind("= Rasten\n\nAm Sonntag ruht das Rad.\n")
    .execute(pool)
    .await
    .unwrap();
    exec(
        pool,
        &format!(
            "INSERT INTO post (node_id, position, author, body, sources) VALUES ({}, 0, '@lektor', 'Welcher Bach?', '[]')",
            nodes[0]
        ),
    )
    .await;
    let asset: i64 = sqlx::query_scalar("INSERT INTO asset (work_id, name) VALUES (?, 'rad.svg') RETURNING id")
        .bind(work)
        .fetch_one(pool)
        .await
        .unwrap();
    let svg = "<svg xmlns=\"http://www.w3.org/2000/svg\"><circle r=\"2\"/></svg>";
    exec(
        pool,
        &format!(
            "INSERT INTO asset_revision (asset_id, number, mime, encoding, content, bytes, sha256, caption, alt, author_name, status) \
             VALUES ({asset}, 1, 'image/svg+xml', 'svg', '{svg}', {}, 'abc', 'Rad', 'Ein Mühlrad', 'Rita', 'current')",
            svg.len()
        ),
    )
    .await;
    exec(pool, &format!("INSERT INTO work_timing (work_id, words_per_minute) VALUES ({work}, 120)")).await;
    work
}

async fn tables(pool: &SqlitePool) -> BTreeMap<String, Vec<String>> {
    let names: Vec<String> = sqlx::query_scalar(
        "SELECT name FROM sqlite_master WHERE type = 'table' AND sql NOT LIKE 'CREATE VIRTUAL%' ORDER BY name",
    )
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
async fn migration_0044_keeps_every_row_and_only_adds_the_sheet_table() {
    let dir = tempfile::tempdir().unwrap();
    let pool = raw_pool(dir.path()).await;
    migrate_upto_0043(&pool).await;
    let work = insert_rows(&pool).await;
    let layout = tables(&pool).await;
    let schema_before = schema(&pool).await;
    let before = contents(&pool, &layout).await;
    assert_eq!(before["revision"].len(), 3, "drei Fassungen vor der Migration (zwei aktuelle, ein Vorschlag)");
    assert!(
        !schema_before.contains_key("work_sheet"),
        "vor 0044 gibt es work_sheet nicht (sonst prüfte der Test nichts)"
    );

    // Only 0044, as `baruch_store::migrate` runs it (not in FOREIGN_KEYS_OFF: foreign keys stay on).
    store_migrator(|v| v == 44).run(&pool).await.expect("0044 läuft auf dem Stand von development");

    let after = contents(&pool, &layout).await;
    for (table, rows) in &before {
        if table != "_sqlx_migrations" {
            assert_eq!(after.get(table), Some(rows), "Tabelle {table}: alle Zeilen unverändert");
        }
    }
    let schema_after = schema(&pool).await;
    for (name, sql) in &schema_before {
        assert_eq!(schema_after.get(name), Some(sql), "Schema von {name} unverändert (0044 ändert keine alte Tabelle)");
    }
    let added: Vec<&str> =
        schema_after.keys().filter(|n| !schema_before.contains_key(*n)).map(String::as_str).collect();
    assert_eq!(added, vec!["work_sheet"], "genau die neue Tabelle work_sheet, kein Index, kein Trigger");
    assert_eq!(scalar(&pool, "SELECT count(*) FROM work_sheet").await, 0, "work_sheet ist leer: alles aus der Vorlage");
    let cols = tables(&pool).await.remove("work_sheet").unwrap();
    assert_eq!(
        cols,
        ["work_id", "font", "heading_font", "size", "fill", "heading_fill", "background", "width", "updated"],
        "Spalten wie im Vertrag (gemeinsame Namen §2)"
    );

    // The table itself sets the outer limits.
    let orphan = sqlx::query("INSERT INTO work_sheet (work_id, size) VALUES (99999, '11pt')").execute(&pool).await;
    assert!(orphan.is_err(), "Fremdschlüssel: eine Zeile braucht ein vorhandenes Werk");
    for (column, value, why) in [
        ("fill", "rot", "eine Farbe hat genau sieben Zeichen und beginnt mit #"),
        ("background", "#1234567", "acht Zeichen sind keine Farbe"),
        ("size", "1", "eine Länge hat mindestens drei Zeichen (Zahl und Einheit)"),
        ("width", "12345678901234567cm", "eine Länge hat höchstens 16 Zeichen"),
        ("font", "", "eine leere Schriftliste ist NULL, kein leerer Text"),
    ] {
        let refused =
            sqlx::query(sqlx::AssertSqlSafe(format!("INSERT INTO work_sheet (work_id, {column}) VALUES (?, ?)")))
                .bind(work)
                .bind(value)
                .execute(&pool)
                .await;
        assert!(refused.is_err(), "CHECK auf {column} = {value:?}: {why}");
    }
    let long_font = "a".repeat(201);
    let refused = sqlx::query("INSERT INTO work_sheet (work_id, font) VALUES (?, ?)")
        .bind(work)
        .bind(&long_font)
        .execute(&pool)
        .await;
    assert!(refused.is_err(), "CHECK: höchstens 200 Zeichen Schriftliste");
    assert_eq!(scalar(&pool, "SELECT count(*) FROM work_sheet").await, 0, "keine der abgelehnten Zeilen steht da");
    pool.close().await;

    let store = Store::open(dir.path()).await.expect("Store::open nach 0044");
    assert_eq!(
        baruch_typst::xref::strip_key_labels(&store.export_typst(work).await.unwrap()).into_owned(),
        BOOK,
        "Export byte-gleich wie vor der Migration"
    );
    assert!(
        store.sheet_settings(work).await.unwrap().is_empty(),
        "ohne Zeile ist nichts gesetzt (alles aus der Vorlage)"
    );
    let set =
        SheetSettings { width: Some("17cm".into()), background: Some("#fffaf0".into()), ..SheetSettings::default() };
    store.set_sheet_settings(work, &set).await.expect("Einstellungen setzen nach der Migration");
    assert_eq!(store.sheet_settings(work).await.unwrap(), set, "gesetzte Werte werden gelesen");

    store.delete_work(work, "Die Mühle am Bach").await.expect("Werk löschen");
    let left: i64 = sqlx::query_scalar("SELECT count(*) FROM work_sheet").fetch_one(store.pool()).await.unwrap();
    assert_eq!(left, 0, "mit dem Werk ist seine Satzbild-Einstellung fort (ON DELETE CASCADE)");
    let broken: i64 =
        sqlx::query_scalar("SELECT count(*) FROM pragma_foreign_key_check").fetch_one(store.pool()).await.unwrap();
    assert_eq!(broken, 0, "keine Verstöße gegen Fremdschlüssel");
}
