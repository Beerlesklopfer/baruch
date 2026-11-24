// SPDX-License-Identifier: AGPL-3.0-or-later
//! Migration 0031 (Sammlungen je Werk für pgvector und MultiGPT, Runde 12) auf einer nachgebauten Datenbank
//! im Stand von development davor (alle Store-Migrationen bis 0030 und Bibliothek 0101–0105), mit frei
//! erfundenen Werken, Vorschlag, Beitrag, Personas, Lektor-Wahl, Verbindungen samt Prüfergebnis, RAG-Wegen
//! je Werk, bevorzugter Quelle, Modulordnern und Bibel je Werk.
//!
//! Nach einem Migrator bis 0031: Das Schema jeder alten Tabelle bleibt, neu ist genau `work_rag_collection`,
//! leer. Danach `Store::open`: jede Zeile jeder alten Tabelle unverändert, kein Werk hat Sammlungen;
//! Wählen geht, Löschen des Werks nimmt seine Sammlungen mit (die des anderen Werks bleiben), ein zweites
//! Öffnen ändert nichts.

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::path::Path;
use std::str::FromStr;

use baruch_store::{RagWay, Store, title_key};
use sqlx::SqlitePool;
use sqlx::migrate::Migrator;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

const LIGHTHOUSE: &str = "#set document(title: \"Das Jahr am Leuchtturm\", author: \"Ida Bach\")\n\n\
= Frühling\n\nDie Möwen kehren zurück.\n\n= Herbst\n\nDer Nebel liegt auf dem Wasser.\n";

const MUSTARD: &str = "#set document(title: \"Senfkornbuch\", author: \"Jona Feld\")\n\n= Saat\n\nKlein beginnt es.\n";

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

/// Stand von development vor Runde 12: Store bis 0030 und Bibliothek 0101–0105.
async fn migrate_before(pool: &SqlitePool) {
    assert!(baruch_store::MIGRATOR.iter().any(|m| m.version == 31), "die neue Migration ist 0031");
    assert!(baruch_store::MIGRATOR.iter().any(|m| m.version == 30), "Stand davor hat 0030 (Bibeln)");
    migrate_where(pool, |v| v <= 30).await;
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../baruch-library/migrations");
    let mut library = Migrator::new(dir).await.unwrap();
    library.migrations = Cow::Owned(library.iter().filter(|m| m.version <= 105).cloned().collect());
    library.ignore_missing = true;
    library.run(pool).await.unwrap();
}

async fn exec(pool: &SqlitePool, sql: &str) {
    sqlx::query(sql).execute(pool).await.unwrap_or_else(|e| panic!("{sql}: {e}"));
}

/// Ein Werk aus Typst-Quelltext mit aktuellen Fassungen; Rückgabe: Werk-id und die Knoten-ids.
async fn insert_work(pool: &SqlitePool, slug: &str, title: &str, source: &str) -> (i64, Vec<i64>) {
    let doc = baruch_typst::parse(source).unwrap();
    let work: i64 = sqlx::query_scalar(
        "INSERT INTO work (slug, kind, title, author, template, source_name, source_sha256, title_key, audience) \
         VALUES (?, 'book', ?, 'Ida Bach', ?, 'erfunden.typ', ?, ?, 'Leser') RETURNING id",
    )
    .bind(slug)
    .bind(title)
    .bind(&doc.template)
    .bind(format!("erfunden-{slug}"))
    .bind(title_key(title))
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO work_template_revision (work_id, number, body, author_name, note) VALUES (?, 1, ?, 'Autor', 'Anfang')")
        .bind(work)
        .bind(&doc.template)
        .execute(pool)
        .await
        .unwrap();
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
    (work, ids)
}

/// Inhalt im Stand vor 0031; Rückgabe: (Leuchtturm, Senfkorn).
async fn insert_rows(pool: &SqlitePool) -> (i64, i64) {
    let (lighthouse, ids) = insert_work(pool, "das-jahr-am-leuchtturm", "Das Jahr am Leuchtturm", LIGHTHOUSE).await;
    let (mustard, _) = insert_work(pool, "senfkornbuch", "Senfkornbuch", MUSTARD).await;
    exec(pool, "INSERT INTO store_meta (key, value) VALUES ('seeded', '1') ON CONFLICT DO NOTHING").await;
    exec(pool, "INSERT INTO store_meta (key, value) VALUES ('lector', 'auto') ON CONFLICT DO NOTHING").await;
    exec(
        pool,
        "INSERT INTO persona (handle, name, role, model, context_length, active, position, color) VALUES \
         ('@lektor', 'Lektor', 'Lektorat', 'probe-a', 32000, 1, 0, '#1c7ed6'), \
         ('@belege', 'Belege', 'Quellen', 'probe-a', 32000, 1, 1, NULL)",
    )
    .await;
    sqlx::query(
        "INSERT INTO revision (node_id, number, body, author_kind, author_name, model, status, note, based_on) \
         VALUES (?, 2, '= Herbst\n\nDer Nebel liegt dicht auf dem Wasser.\n', 'assistant', '@lektor', 'probe-a', 'proposed', 'Bild', 1)",
    )
    .bind(ids[1])
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO post (node_id, position, author, body, sources) VALUES (?, 0, '@belege', 'Keine Quelle.', '[]')",
    )
    .bind(ids[0])
    .execute(pool)
    .await
    .unwrap();
    exec(pool, "INSERT INTO connection (kind, config) VALUES ('eliezer', '{\"url\":\"http://127.0.0.1:4321\"}')").await;
    exec(
        pool,
        "INSERT INTO connection (kind, config, checked, check_ok, check_message, check_details) VALUES ('pgvector', \
         '{\"host\":\"db.example\",\"port\":5432,\"database\":\"rag\",\"user\":\"leser\",\"tls\":\"require\"}', \
         '2026-11-06T10:00:00.000Z', 1, 'Verbindung steht', '{\"server_version\":\"PostgreSQL 17\",\"vector\":\"0.8.0\"}')",
    )
    .await;
    sqlx::query("INSERT INTO work_rag (work_id, ways) VALUES (?, '[\"lancedb\",\"pgvector\"]')")
        .bind(lighthouse)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO work_rag (work_id, ways) VALUES (?, '[]')").bind(mustard).execute(pool).await.unwrap();
    exec(pool, "INSERT INTO preferred_source (title, url, note, position) VALUES ('Leuchtfeuer', 'https://leuchtfeuer.example/', '', 1)").await;
    exec(pool, "INSERT INTO bible_dir (position, path) VALUES (1, '/erfunden/sword')").await;
    exec(pool, "INSERT INTO store_meta (key, value) VALUES ('bible_dirs', '1')").await;
    sqlx::query("INSERT INTO work_bible (work_id, module) VALUES (?, 'Erfunden')")
        .bind(lighthouse)
        .execute(pool)
        .await
        .unwrap();
    (lighthouse, mustard)
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
            sqlx::query_scalar(&format!("SELECT {select} FROM \"{table}\"")).fetch_all(pool).await.unwrap();
        rows.sort();
        out.insert(table.clone(), rows);
    }
    out
}

async fn collection_rows(store: &Store) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM work_rag_collection").fetch_one(store.pool()).await.unwrap()
}

#[tokio::test]
async fn migration_0031_keeps_every_row_and_only_adds_the_collection_table() {
    let dir = tempfile::tempdir().unwrap();
    let pool = raw_pool(dir.path()).await;
    migrate_before(&pool).await;
    let (lighthouse, mustard) = insert_rows(&pool).await;
    let layout = tables(&pool).await;
    let schema_before = schema(&pool).await;
    let before = contents(&pool, &layout).await;
    assert_eq!(before["revision"].len(), 4, "drei aktuelle Fassungen und ein Vorschlag vor der Migration");
    assert_eq!(before["work_rag"].len(), 2, "beide Werke mit RAG-Wegen vor der Migration");

    migrate_where(&pool, |v| v <= 31).await;
    let schema_after = schema(&pool).await;
    for (name, sql) in &schema_before {
        assert_eq!(schema_after.get(name), Some(sql), "Schema von {name} unverändert (0031 ändert keine alte Tabelle)");
    }
    let added: Vec<&str> =
        schema_after.keys().filter(|n| !schema_before.contains_key(*n)).map(String::as_str).collect();
    assert_eq!(
        added,
        vec!["work_rag_collection"],
        "genau eine neue Tabelle (ihr Primärschlüssel-Index heißt sqlite_autoindex_…, der ist ausgefiltert)"
    );
    let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM work_rag_collection").fetch_one(&pool).await.unwrap();
    assert_eq!(rows, 0, "keine Datenänderung: die neue Tabelle ist leer");
    pool.close().await;

    let store = Store::open(dir.path()).await.expect("Store::open nach 0031");
    let after = without_instance_row(contents(store.pool(), &layout).await);
    for (table, rows) in &before {
        if table == "_sqlx_migrations" {
            continue;
        }
        assert_eq!(after.get(table), Some(rows), "Tabelle {table}: alle Zeilen unverändert");
    }
    for work in [lighthouse, mustard] {
        for way in [RagWay::Pgvector, RagWay::Multigpt] {
            assert!(store.work_collections(work, way).await.unwrap().is_empty(), "Werk {work}, {way}: keine Sammlung");
        }
    }
    assert_eq!(
        store.work_rag(lighthouse).await.unwrap(),
        (vec![RagWay::Lancedb, RagWay::Pgvector], false),
        "die RAG-Wege des Werks bleiben"
    );
    assert_eq!(
        baruch_typst::xref::strip_key_labels(&store.export_typst(lighthouse).await.unwrap()).into_owned(),
        LIGHTHOUSE,
        "Export byte-gleich wie vor der Migration"
    );

    store.set_work_collections(lighthouse, RagWay::Pgvector, &["Leuchtfeuer".into(), "Gezeiten".into()]).await.unwrap();
    store.set_work_collections(lighthouse, RagWay::Multigpt, &["Seekarten".into()]).await.unwrap();
    store.set_work_collections(mustard, RagWay::Multigpt, &["Saatgut".into()]).await.unwrap();
    assert_eq!(collection_rows(&store).await, 4, "vier gewählte Sammlungen");
    let broken: i64 =
        sqlx::query_scalar("SELECT count(*) FROM pragma_foreign_key_check").fetch_one(store.pool()).await.unwrap();
    assert_eq!(broken, 0, "keine Verstöße gegen Fremdschlüssel");

    // Zweites Öffnen ändert nichts.
    let layout_now = tables(store.pool()).await;
    let schema_once = schema(store.pool()).await;
    let once = contents(store.pool(), &layout_now).await;
    drop(store);
    let store = Store::open(dir.path()).await.unwrap();
    assert_eq!(schema(store.pool()).await, schema_once, "zweites Öffnen: Schema gleich");
    assert_eq!(contents(store.pool(), &layout_now).await, once, "zweites Öffnen: alle Zeilen gleich");

    // Löschen eines Werks nimmt seine Sammlungen mit, die des anderen bleiben.
    store.delete_work(lighthouse, "Das Jahr am Leuchtturm").await.unwrap();
    assert_eq!(collection_rows(&store).await, 1, "ON DELETE CASCADE: nur die Sammlung des Senfkornbuchs bleibt");
    assert_eq!(
        store.work_collections(mustard, RagWay::Multigpt).await.unwrap(),
        vec!["Saatgut".to_owned()],
        "das andere Werk behält seine Wahl"
    );
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
