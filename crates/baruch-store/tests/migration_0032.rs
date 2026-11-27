// SPDX-License-Identifier: AGPL-3.0-or-later
//! Migration 0032 (Treffer der Bedeutungssuche als Belege je Persona, Runde 12) auf einer nachgebauten
//! Datenbank im Stand davor (alle Store-Migrationen unter 0032 – im Stand von development bis 0030, nach dem
//! Zusammenführen auch 0031 – und Bibliothek 0101–0105), mit frei erfundenem Werk, Vorschlag, Beitrag,
//! Personas samt eigenem Prompt, Lektor-Wahl, Bibel des Werks und Verbindung.
//!
//! Nach einem Migrator bis 0032: Das Schema jeder alten Tabelle bleibt, neu ist genau `persona_rag`, leer.
//! Danach `Store::open`: jede Zeile jeder alten Tabelle unverändert, keine Persona hat den Schalter an;
//! Setzen geht, Löschen der Persona nimmt die Zeile mit, ein zweites Öffnen ändert nichts.

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::path::Path;
use std::str::FromStr;

use baruch_store::{Store, StoreError, title_key};
use sqlx::SqlitePool;
use sqlx::migrate::Migrator;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

const SOURCE: &str = "#set document(title: \"Das Jahr am Leuchtturm\", author: \"Mara Lind\")\n\n\
= Frühling\n\nDas Licht kreist über dem Wasser.\n\n= Herbst\n\nDer Nebel kommt früh.\n";

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

/// Store unter 0032 und Bibliothek 0101–0105.
async fn migrate_before(pool: &SqlitePool) {
    assert!(baruch_store::MIGRATOR.iter().any(|m| m.version == 32), "die neue Migration ist 0032");
    assert!(baruch_store::MIGRATOR.iter().any(|m| m.version == 30), "Stand davor hat mindestens 0030 (development)");
    migrate_where(pool, |v| v < 32).await;
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
         VALUES ('das-jahr-am-leuchtturm', 'book', 'Das Jahr am Leuchtturm', 'Mara Lind', ?, 'leuchtturm.typ', \
         'erfunden', ?, 'Küstenleute') RETURNING id",
    )
    .bind(&doc.template)
    .bind(title_key("Das Jahr am Leuchtturm"))
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
         ('@lektor', 'Lektor', 'Lektorat', 'probe-a', 32000, 1, 0, '#1c7ed6', 'Eigener Prompt.'), \
         ('@belege', 'Belege', 'Quellen', 'probe-a', 16000, 1, 1, NULL, NULL), \
         ('@hoerer', 'Hörer', 'Zielgruppe', 'probe-a', 8000, 0, 2, NULL, NULL)",
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
         VALUES (?, 2, '= Herbst\n\nDer Nebel kommt früh und bleibt.\n', 'assistant', '@lektor', 'probe-a', 'proposed', 'Bild', 1)",
    )
    .bind(ids[1])
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO post (node_id, position, author, body, sources) VALUES (?, 0, '@belege', 'Nicht belegt.', '[]')",
    )
    .bind(ids[0])
    .execute(pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO work_bible (work_id, module) VALUES (?, 'Erfunden')")
        .bind(work)
        .execute(pool)
        .await
        .unwrap();
    exec(pool, "INSERT INTO connection (kind, config) VALUES ('multigpt', '{\"url\":\"http://127.0.0.1:4321/mcp\"}')")
        .await;
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
async fn migration_0032_keeps_every_row_and_only_adds_persona_rag() {
    let dir = tempfile::tempdir().unwrap();
    let pool = raw_pool(dir.path()).await;
    migrate_before(&pool).await;
    let work = insert_rows(&pool).await;
    let layout = tables(&pool).await;
    let schema_before = schema(&pool).await;
    let before = contents(&pool, &layout).await;
    assert_eq!(before["persona"].len(), 3, "drei Personas vor der Migration");
    assert_eq!(before["revision"].len(), 3, "zwei aktuelle Fassungen und ein Vorschlag");

    migrate_where(&pool, |v| v <= 32).await;
    let schema_after = schema(&pool).await;
    for (name, sql) in &schema_before {
        assert_eq!(schema_after.get(name), Some(sql), "Schema von {name} unverändert (0032 ändert keine alte Tabelle)");
    }
    let added: Vec<&str> =
        schema_after.keys().filter(|n| !schema_before.contains_key(*n)).map(String::as_str).collect();
    assert_eq!(added, vec!["persona_rag"], "genau eine neue Tabelle (PersonaRow und persona bleiben)");
    let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM persona_rag").fetch_one(&pool).await.unwrap();
    assert_eq!(rows, 0, "keine Datenänderung: keine Zeile = aus");
    let refused =
        sqlx::query("INSERT INTO persona_rag (persona_id, enabled) VALUES ((SELECT id FROM persona LIMIT 1), 2)")
            .execute(&pool)
            .await;
    assert!(refused.is_err(), "CHECK lässt nur 0 und 1 zu");
    let orphan = sqlx::query("INSERT INTO persona_rag (persona_id, enabled) VALUES (99999, 1)").execute(&pool).await;
    assert!(orphan.is_err(), "Fremdschlüssel: nur vorhandene Personas");
    pool.close().await;

    let store = Store::open(dir.path()).await.expect("Store::open nach 0032");
    let after = without_instance_row(contents(store.pool(), &layout).await);
    for (table, rows) in &before {
        if table == "_sqlx_migrations" {
            continue;
        }
        assert_eq!(after.get(table), Some(rows), "Tabelle {table}: alle Zeilen unverändert");
    }
    for handle in ["@lektor", "belege", "@hoerer"] {
        assert!(!store.persona_rag(handle).await.unwrap(), "{handle}: nach der Migration aus (keine Zeile)");
    }
    assert_eq!(
        baruch_typst::xref::strip_key_labels(&store.export_typst(work).await.unwrap()).into_owned(),
        SOURCE,
        "Export byte-gleich wie vor der Migration"
    );
    assert!(store.set_persona_rag("belege", true).await.unwrap(), "Handle ohne @ gefunden, eingeschaltet");
    assert!(store.persona_rag("@belege").await.unwrap(), "gespeichert");
    assert!(!store.set_persona_rag("@belege", false).await.unwrap(), "wieder aus");
    assert!(!store.persona_rag("@belege").await.unwrap(), "aus ist eine Zeile mit 0");
    assert!(
        matches!(store.set_persona_rag("@gibtsnicht", true).await, Err(StoreError::Refused(_))),
        "unbekannte Persona → Refused"
    );
    assert!(matches!(store.persona_rag("@gibtsnicht").await, Err(StoreError::Refused(_))), "auch beim Lesen");
    let personas = store.persona_list().await.unwrap();
    assert_eq!(personas.len(), 3, "PersonaRow unverändert lesbar, keine Persona dazugekommen");
    store.set_persona_rag("@lektor", true).await.unwrap();

    // Zweites Öffnen ändert nichts.
    let layout_now = tables(store.pool()).await;
    let schema_once = schema(store.pool()).await;
    let once = contents(store.pool(), &layout_now).await;
    drop(store);
    let store = Store::open(dir.path()).await.unwrap();
    assert_eq!(schema(store.pool()).await, schema_once, "zweites Öffnen: Schema gleich");
    assert_eq!(contents(store.pool(), &layout_now).await, once, "zweites Öffnen: alle Zeilen gleich");
    assert!(store.persona_rag("@lektor").await.unwrap(), "der Schalter überdauert das Öffnen");

    // Löschen der Persona nimmt ihre Zeile mit (ON DELETE CASCADE).
    sqlx::query("DELETE FROM persona WHERE handle = '@lektor'").execute(store.pool()).await.unwrap();
    let left: i64 = sqlx::query_scalar("SELECT count(*) FROM persona_rag").fetch_one(store.pool()).await.unwrap();
    assert_eq!(left, 1, "nur die Zeile von @belege bleibt (@lektor ist mit seiner Persona gegangen)");
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
