// SPDX-License-Identifier: AGPL-3.0-or-later
//! Migration 0030 (SWORD-Bibeln: Modulordner und Bibel je Werk) auf einer nachgebauten Datenbank im Stand
//! davor (alle Store-Migrationen unter 0030 und Bibliothek 0101–0105), mit einem frei erfundenen Werk,
//! Vorschlag, Beitrag, Personas, Lektor-Wahl und Verbindung.
//!
//! Nach einem Migrator bis 0030: Das Schema jeder alten Tabelle bleibt, neu sind genau `bible_dir` und
//! `work_bible`, beide leer. Danach `Store::open`: jede Zeile jeder alten Tabelle unverändert, keine
//! Modulordner gesetzt (Vorgaben gelten), kein Werk mit Bibel; Wählen und Abwählen gehen, Löschen des Werks
//! nimmt die Wahl mit, ein zweites Öffnen ändert nichts.

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::path::Path;
use std::str::FromStr;

use baruch_store::{Store, title_key};
use sqlx::SqlitePool;
use sqlx::migrate::Migrator;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

const SOURCE: &str = "#set document(title: \"Das Mühlrad\", author: \"Ida Bach\")\n\n\
= Wasser\n\nDas Rad dreht sich langsam.\n\n= Mehl\n\nDer Sack ist voll.\n";

async fn raw_pool(dir: &Path) -> SqlitePool {
    let options = SqliteConnectOptions::from_str("sqlite:")
        .unwrap()
        .filename(dir.join("baruch.sqlite"))
        .create_if_missing(true)
        .foreign_keys(true);
    SqlitePoolOptions::new().max_connections(1).connect_with(options).await.unwrap()
}

async fn migrate_where(pool: &SqlitePool, keep: impl Fn(i64) -> bool) {
    let list: Vec<_> = baruch_store::MIGRATOR.iter().filter(|m| keep(m.version)).cloned().collect();
    let migrator = Migrator { migrations: Cow::Owned(list), ignore_missing: true, ..Migrator::DEFAULT };
    migrator.run(pool).await.unwrap();
}

/// Store unter 0030 und Bibliothek 0101–0105.
async fn migrate_before(pool: &SqlitePool) {
    assert!(baruch_store::MIGRATOR.iter().any(|m| m.version == 30), "die neue Migration ist 0030");
    assert!(baruch_store::MIGRATOR.iter().any(|m| m.version == 16), "Stand davor hat mindestens 0016");
    migrate_where(pool, |v| v < 30).await;
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../baruch-library/migrations");
    let mut library = Migrator::new(dir).await.unwrap();
    library.migrations = Cow::Owned(library.iter().filter(|m| m.version <= 105).cloned().collect());
    library.ignore_missing = true;
    library.run(pool).await.unwrap();
}

async fn exec(pool: &SqlitePool, sql: &str) {
    sqlx::query(sqlx::AssertSqlSafe(sql)).execute(pool).await.unwrap_or_else(|e| panic!("{sql}: {e}"));
}

async fn insert_rows(pool: &SqlitePool) -> i64 {
    let doc = baruch_typst::parse(SOURCE).unwrap();
    let work: i64 = sqlx::query_scalar(
        "INSERT INTO work (slug, kind, title, author, template, source_name, source_sha256, title_key, audience) \
         VALUES ('das-muehlrad', 'book', 'Das Mühlrad', 'Ida Bach', ?, 'muehle.typ', 'erfunden', ?, 'Müller') RETURNING id",
    )
    .bind(&doc.template)
    .bind(title_key("Das Mühlrad"))
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
    exec(pool, "INSERT INTO store_meta (key, value) VALUES ('lector', 'auto') ON CONFLICT DO NOTHING").await;
    exec(
        pool,
        "INSERT INTO persona (handle, name, role, model, context_length, active, position, color) VALUES \
         ('@lektor', 'Lektor', 'Lektorat', 'probe-a', 32000, 1, 0, '#1c7ed6'), \
         ('@schrift', 'Schrift', 'Bibelstellen', 'probe-a', 32000, 1, 1, NULL)",
    )
    .await;
    let mut ids = Vec::new();
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
        ids.push(id);
    }
    sqlx::query(
        "INSERT INTO revision (node_id, number, body, author_kind, author_name, model, status, note, based_on) \
         VALUES (?, 2, '= Mehl\n\nDer Sack ist prall gefüllt.\n', 'assistant', '@lektor', 'probe-a', 'proposed', 'Bild', 1)",
    )
    .bind(ids[1])
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO post (node_id, position, author, body, sources) VALUES (?, 0, '@schrift', 'Keine Stelle.', '[]')",
    )
    .bind(ids[0])
    .execute(pool)
    .await
    .unwrap();
    exec(pool, "INSERT INTO connection (kind, config) VALUES ('eliezer', '{\"url\":\"http://127.0.0.1:4321\"}')").await;
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
async fn migration_0030_keeps_every_row_and_only_adds_the_bible_tables() {
    let dir = tempfile::tempdir().unwrap();
    let pool = raw_pool(dir.path()).await;
    migrate_before(&pool).await;
    let work = insert_rows(&pool).await;
    let layout = tables(&pool).await;
    let schema_before = schema(&pool).await;
    let before = contents(&pool, &layout).await;
    assert_eq!(before["revision"].len(), 3, "zwei aktuelle Fassungen und ein Vorschlag vor der Migration");

    migrate_where(&pool, |v| v <= 30).await;
    let schema_after = schema(&pool).await;
    for (name, sql) in &schema_before {
        assert_eq!(schema_after.get(name), Some(sql), "Schema von {name} unverändert (0030 ändert keine alte Tabelle)");
    }
    let added: Vec<&str> =
        schema_after.keys().filter(|n| !schema_before.contains_key(*n)).map(String::as_str).collect();
    assert_eq!(added, vec!["bible_dir", "work_bible"], "genau zwei neue Tabellen (ohne eigene Indizes)");
    let rows: i64 = sqlx::query_scalar("SELECT (SELECT count(*) FROM bible_dir) + (SELECT count(*) FROM work_bible)")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(rows, 0, "keine Datenänderung: beide Tabellen leer");
    let refused =
        sqlx::query("INSERT INTO work_bible (work_id, module) VALUES (?, '')").bind(work).execute(&pool).await;
    assert!(refused.is_err(), "CHECK lehnt einen leeren Modulnamen ab");
    pool.close().await;

    let store = Store::open(dir.path()).await.expect("Store::open nach 0030");
    let after = without_instance_row(contents(store.pool(), &layout).await);
    for (table, rows) in &before {
        if table == "_sqlx_migrations" {
            continue;
        }
        assert_eq!(after.get(table), Some(rows), "Tabelle {table}: alle Zeilen unverändert");
    }
    assert_eq!(store.bible_dirs().await.unwrap(), None, "Modulordner nie gesetzt: Vorgaben gelten");
    assert_eq!(store.work_bible(work).await.unwrap(), None, "das Werk hat noch keine Bibel");
    assert_eq!(
        baruch_typst::xref::strip_key_labels(&store.export_typst(work).await.unwrap()).into_owned(),
        SOURCE,
        "Export byte-gleich wie vor der Migration"
    );

    store.set_bible_dirs(Some(&["/a/sword".to_owned(), "/b".to_owned(), "/a/sword".to_owned()])).await.unwrap();
    assert_eq!(
        store.bible_dirs().await.unwrap(),
        Some(vec!["/a/sword".into(), "/b".into()]),
        "Reihenfolge, ohne Doppel"
    );
    store.set_bible_dirs(Some(&[])).await.unwrap();
    assert_eq!(store.bible_dirs().await.unwrap(), Some(vec![]), "leere Liste ist etwas anderes als die Vorgabe");
    store.set_bible_dirs(None).await.unwrap();
    assert_eq!(store.bible_dirs().await.unwrap(), None, "zurück auf die Vorgaben");
    store.set_work_bible(work, Some("Erfunden")).await.unwrap();
    assert_eq!(store.work_bible(work).await.unwrap().as_deref(), Some("Erfunden"), "Bibel gewählt");
    assert!(store.work_bible(work + 99).await.is_err(), "unbekanntes Werk wird gemeldet");

    let broken: i64 =
        sqlx::query_scalar("SELECT count(*) FROM pragma_foreign_key_check").fetch_one(store.pool()).await.unwrap();
    assert_eq!(broken, 0, "keine Verstöße gegen Fremdschlüssel");
    store.set_work_bible(work, None).await.unwrap();
    assert_eq!(store.work_bible(work).await.unwrap(), None, "abgewählt");

    // Zweites Öffnen ändert nichts.
    let layout_now = tables(store.pool()).await;
    let schema_once = schema(store.pool()).await;
    let once = contents(store.pool(), &layout_now).await;
    drop(store);
    let store = Store::open(dir.path()).await.unwrap();
    assert_eq!(schema(store.pool()).await, schema_once, "zweites Öffnen: Schema gleich");
    assert_eq!(contents(store.pool(), &layout_now).await, once, "zweites Öffnen: alle Zeilen gleich");
}

#[tokio::test]
async fn deleting_a_work_removes_its_bible_choice() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).await.unwrap();
    let first = &store.work_list().await.unwrap()[0];
    let (work, title) = (first.id, first.title.clone());
    store.set_work_bible(work, Some("Erfunden")).await.unwrap();
    store.delete_work(work, &title).await.unwrap();
    let left: i64 = sqlx::query_scalar("SELECT count(*) FROM work_bible").fetch_one(store.pool()).await.unwrap();
    assert_eq!(left, 0, "ON DELETE CASCADE nimmt die Wahl mit");
}

/// Migration 0041 (Runde 26) trägt beim Start die Instanzkennung als neue Zeile in `store_meta` ein; das ergänzt
/// eine Zeile und ändert keine alte. Der Vergleich der alten Zeilen nimmt genau diese eine Zeile heraus.
fn without_instance_row(mut contents: BTreeMap<String, Vec<String>>) -> BTreeMap<String, Vec<String>> {
    if let Some(meta) = contents.get_mut("store_meta") {
        let count = meta.len();
        meta.retain(|row| !row.starts_with("'instance_uuid'\u{1f}"));
        assert_eq!(count - meta.len(), 1, "genau eine neue Zeile instance_uuid in store_meta (Migration 0041)");
    }
    contents
}
