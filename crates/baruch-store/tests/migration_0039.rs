// SPDX-License-Identifier: AGPL-3.0-or-later
//! Migration 0039 (Zeitwächter, Runde 16b) auf einer nachgebauten Datenbank im Stand von development zu Beginn
//! von Runde 16 (Store bis 0038, Bibliothek 0101–0106) mit frei erfundener Predigt: Punkte, Fassungen (aktuell
//! und Vorschlag), Beiträge, Personas, Faden.
//!
//! Nach einem Migrator bis 0039: Das Schema jeder alten Tabelle bleibt, neu ist genau `work_timing` (leer).
//! Danach `Store::open`: jede Zeile jeder alten Tabelle unverändert, Export byte-gleich, ohne Zeile gilt die
//! Vorgabe (110 Wörter je Minute, keine Zielzeit); Einstellungen lassen sich setzen; Löschen des Werks nimmt
//! seine `work_timing`-Zeile mit (ON DELETE CASCADE, ohne dass `Store::delete_work` sie kennt).

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::path::Path;
use std::str::FromStr;

use baruch_store::{Store, TimingSettings, title_key};
use sqlx::SqlitePool;
use sqlx::migrate::Migrator;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

const SOURCE: &str = "#set document(title: \"Die Fähre im Schilf\", author: \"Ivo Brand\")\n\n\
= Abfahrt\n\nDas Seil knarrt am Steg.\n\n= Ankunft\n\nDrüben wartet niemand.\n";

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

/// Store bis 0038 (Stand development zu Beginn von Runde 16) und Bibliothek bis 0106.
async fn migrate_before(pool: &SqlitePool) {
    assert!(baruch_store::MIGRATOR.iter().any(|m| m.version == 39), "die neue Migration ist 0039");
    assert!(
        baruch_store::MIGRATOR.iter().any(|m| m.version == 38),
        "Stand davor hat 0038 (development zu Beginn von Runde 16)"
    );
    migrate_where(pool, |v| v < 39).await;
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../baruch-library/migrations");
    let mut library = Migrator::new(dir).await.unwrap();
    library.migrations = Cow::Owned(library.iter().filter(|m| m.version <= 106).cloned().collect());
    library.ignore_missing = true;
    library.run(pool).await.unwrap();
}

async fn exec(pool: &SqlitePool, sql: &str) {
    sqlx::query(sql).execute(pool).await.unwrap_or_else(|e| panic!("{sql}: {e}"));
}

/// Predigt mit zwei Punkten, Fassungen, zwei Beiträgen (einer mit Quelle), Personas und einem Faden.
async fn insert_rows(pool: &SqlitePool) -> i64 {
    let doc = baruch_typst::parse(SOURCE).unwrap();
    let work: i64 = sqlx::query_scalar(
        "INSERT INTO work (slug, kind, title, author, template, source_name, source_sha256, title_key, audience) \
         VALUES ('die-faehre-im-schilf', 'sermon', 'Die Fähre im Schilf', 'Ivo Brand', ?, 'faehre.typ', \
         'erfunden', ?, 'Flussleute') RETURNING id",
    )
    .bind(&doc.template)
    .bind(title_key("Die Fähre im Schilf"))
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
    exec(pool, "INSERT INTO store_meta (key, value) VALUES ('lector', '@lektor') ON CONFLICT DO NOTHING").await;
    exec(
        pool,
        "INSERT INTO persona (handle, name, role, model, context_length, active, position, color, prompt) VALUES \
         ('@lektor', 'Lektor', 'Lektorat', 'probe-a', 32000, 1, 0, '#1c7ed6', NULL), \
         ('@hoerer', 'Hörer', 'Gemeinde', 'probe-a', 16000, 1, 1, NULL, 'Eigener Prompt.')",
    )
    .await;
    let mut nodes = Vec::new();
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
        nodes.push(id);
    }
    sqlx::query(
        "INSERT INTO revision (node_id, number, body, author_kind, author_name, model, status, note, based_on) \
         VALUES (?, 2, ?, 'assistant', '@lektor', 'probe-a', 'proposed', 'Vorschlag', 1)",
    )
    .bind(nodes[1])
    .bind("= Ankunft\n\nDrüben wartet doch jemand.\n")
    .execute(pool)
    .await
    .unwrap();
    let thread: i64 = sqlx::query_scalar(
        "INSERT INTO chat_thread (work_id, created_by, title) VALUES (?, 'Autor', 'Wie lang darf es sein?') RETURNING id",
    )
    .bind(work)
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO chat_thread_post (thread_id, position, author, body) VALUES (?, 0, 'Autor', 'Zwanzig Minuten.')",
    )
    .bind(thread)
    .execute(pool)
    .await
    .unwrap();
    for (node, position, author, body, sources) in [
        (nodes[0], 0, "Autor", "Knarrt das Seil zu oft?", "[]"),
        (nodes[0], 1, "@lektor", "Einmal genügt.", "[\"Einfall 2\"]"),
    ] {
        sqlx::query("INSERT INTO post (node_id, position, author, body, sources) VALUES (?, ?, ?, ?, ?)")
            .bind(node)
            .bind(position)
            .bind(author)
            .bind(body)
            .bind(sources)
            .execute(pool)
            .await
            .unwrap();
    }
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
            sqlx::query_scalar(&format!("SELECT {select} FROM \"{table}\"")).fetch_all(pool).await.unwrap();
        rows.sort();
        out.insert(table.clone(), rows);
    }
    out
}

#[tokio::test]
async fn migration_0039_keeps_every_row_and_only_adds_the_timing_table() {
    let dir = tempfile::tempdir().unwrap();
    let pool = raw_pool(dir.path()).await;
    migrate_before(&pool).await;
    let work = insert_rows(&pool).await;
    let layout = tables(&pool).await;
    let schema_before = schema(&pool).await;
    let before = contents(&pool, &layout).await;
    assert_eq!(before["revision"].len(), 3, "drei Fassungen vor der Migration (zwei aktuelle, ein Vorschlag)");
    assert_eq!(before["chat_thread_post"].len(), 1, "ein Fadenbeitrag vor der Migration");

    migrate_where(&pool, |v| v <= 39).await;
    let schema_after = schema(&pool).await;
    for (name, sql) in &schema_before {
        assert_eq!(schema_after.get(name), Some(sql), "Schema von {name} unverändert (0039 ändert keine alte Tabelle)");
    }
    let added: Vec<&str> =
        schema_after.keys().filter(|n| !schema_before.contains_key(*n)).map(String::as_str).collect();
    assert_eq!(added, vec!["work_timing"], "genau die neue Tabelle work_timing, kein Index, kein Trigger");
    let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM work_timing").fetch_one(&pool).await.unwrap();
    assert_eq!(rows, 0, "keine Datenänderung: work_timing ist leer, es gilt die Vorgabe");
    let orphan =
        sqlx::query("INSERT INTO work_timing (work_id, words_per_minute) VALUES (99999, 110)").execute(&pool).await;
    assert!(orphan.is_err(), "Fremdschlüssel: eine Zeile braucht ein vorhandenes Werk");
    let too_fast = sqlx::query("INSERT INTO work_timing (work_id, words_per_minute) VALUES (?, 251)")
        .bind(work)
        .execute(&pool)
        .await;
    assert!(too_fast.is_err(), "CHECK: mehr als 250 Wörter je Minute weist schon die Tabelle ab");
    pool.close().await;

    let store = Store::open(dir.path()).await.expect("Store::open nach 0039");
    let after = without_instance_row(contents(store.pool(), &layout).await);
    for (table, rows) in &before {
        if table == "_sqlx_migrations" {
            continue;
        }
        assert_eq!(after.get(table), Some(rows), "Tabelle {table}: alle Zeilen unverändert");
    }
    assert_eq!(
        baruch_typst::xref::strip_key_labels(&store.export_typst(work).await.unwrap()).into_owned(),
        SOURCE,
        "Export byte-gleich wie vor der Migration"
    );
    assert_eq!(
        store.timing_settings(work).await.unwrap(),
        TimingSettings { words_per_minute: 110, target_seconds: None },
        "ohne Zeile gilt die Vorgabe: 110 Wörter je Minute, keine Zielzeit"
    );
    let set = TimingSettings { words_per_minute: 120, target_seconds: Some(1200) };
    store.set_timing_settings(work, set).await.expect("Einstellungen setzen nach der Migration");
    assert_eq!(store.timing_settings(work).await.unwrap(), set, "gesetzte Werte werden gelesen");

    // Werk löschen nimmt seine Zeile mit (ON DELETE CASCADE).
    store.delete_work(work, "Die Fähre im Schilf").await.expect("Werk löschen");
    let left: i64 = sqlx::query_scalar("SELECT count(*) FROM work_timing").fetch_one(store.pool()).await.unwrap();
    assert_eq!(left, 0, "mit dem Werk ist seine Zeit-Einstellung fort");
    let broken: i64 =
        sqlx::query_scalar("SELECT count(*) FROM pragma_foreign_key_check").fetch_one(store.pool()).await.unwrap();
    assert_eq!(broken, 0, "keine Verstöße gegen Fremdschlüssel");
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
