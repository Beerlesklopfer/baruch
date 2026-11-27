// SPDX-License-Identifier: AGPL-3.0-or-later
//! Migration 0036 (model connections and connection per persona, Round 15b „anbieter“) on a reproduced database in the
//! state of development (all store migrations up to 0035, library 0101–0105), with a freely invented work, personas and
//! a stored connection `eliezer` including the binding of the key to a former target (migration 0014).
//!
//! After a migrator up to 0036: the schema of every old table stays, new are exactly `model_connection`,
//! `persona_backend` and the trigger that protects Eliezer from deletion; there is exactly one connection „Eliezer“
//! without an address of its own (it stays in `connection`) with the account `eliezer`, and no persona has a row in
//! `persona_backend` (= Eliezer, not cleared). Afterwards `Store::open`: every old row unchanged, the store methods
//! work on the new tables.

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::path::Path;
use std::str::FromStr;

use baruch_store::{ConnectionKind, ELIEZER_CONNECTION, ModelConnectionEdit, Store, title_key};
use sqlx::SqlitePool;
use sqlx::migrate::Migrator;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

const SOURCE: &str = "#set document(title: \"Die Glocken von Arnheide\", author: \"Lene Brook\")\n\n\
= Morgen\n\nDie erste Glocke ruft über den Acker.\n\n= Abend\n\nDie letzte schweigt.\n";

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

/// Store up to 0035 (state of development at the beginning of Round 15) and library 0101–0105.
async fn migrate_before(pool: &SqlitePool) {
    assert!(baruch_store::MIGRATOR.iter().any(|m| m.version == 36), "die neue Migration ist 0036");
    assert!(baruch_store::MIGRATOR.iter().any(|m| m.version == 35), "Stand davor hat 0035 (development, ca04c58)");
    migrate_where(pool, |v| v < 36).await;
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../baruch-library/migrations");
    let mut library = Migrator::new(dir).await.unwrap();
    library.migrations = Cow::Owned(library.iter().filter(|m| m.version <= 105).cloned().collect());
    library.ignore_missing = true;
    library.run(pool).await.unwrap();
}

async fn exec(pool: &SqlitePool, sql: &str) {
    sqlx::query(sqlx::AssertSqlSafe(sql)).execute(pool).await.unwrap_or_else(|e| panic!("{sql}: {e}"));
}

/// Work with two chapters, personas, connection `eliezer` with address, check and bound key.
async fn insert_rows(pool: &SqlitePool) -> i64 {
    let doc = baruch_typst::parse(SOURCE).unwrap();
    let work: i64 = sqlx::query_scalar(
        "INSERT INTO work (slug, kind, title, author, template, source_name, source_sha256, title_key, audience) \
         VALUES ('die-glocken-von-arnheide', 'book', 'Die Glocken von Arnheide', 'Lene Brook', ?, 'glocken.typ', \
         'erfunden', ?, 'Dorfleute') RETURNING id",
    )
    .bind(&doc.template)
    .bind(title_key("Die Glocken von Arnheide"))
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
    exec(
        pool,
        "INSERT INTO persona (handle, name, role, model, context_length, active, position, color, prompt) VALUES \
         ('@lektor', 'Lektor', 'Lektorat', 'probe-a', 32768, 1, 0, '#1c7ed6', NULL), \
         ('@belege', 'Belege', 'Quellen', 'probe-a', 16000, 1, 1, NULL, 'Eigener Prompt.')",
    )
    .await;
    exec(
        pool,
        "INSERT INTO connection (kind, config, checked, check_ok, check_message, check_details, secret_target) VALUES \
         ('eliezer', '{\"url\":\"http://127.0.0.1:9\"}', '2026-11-07T08:00:00.000Z', 1, 'Verbindung steht: 1 Modell.', \
          '{\"models\":[\"probe-a\"]}', 'http://127.0.0.1:1234')",
    )
    .await;
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
async fn migration_0036_adds_the_eliezer_connection_without_touching_old_rows() {
    let dir = tempfile::tempdir().unwrap();
    let pool = raw_pool(dir.path()).await;
    migrate_before(&pool).await;
    let work = insert_rows(&pool).await;
    let layout = tables(&pool).await;
    let schema_before = schema(&pool).await;
    let before = contents(&pool, &layout).await;
    assert_eq!(before["connection"].len(), 1, "eine gespeicherte Verbindung eliezer vor der Migration");

    migrate_where(&pool, |v| v <= 36).await;
    let schema_after = schema(&pool).await;
    for (name, sql) in &schema_before {
        assert_eq!(schema_after.get(name), Some(sql), "Schema von {name} unverändert (0036 legt nur Neues an)");
    }
    let added: Vec<&str> =
        schema_after.keys().filter(|n| !schema_before.contains_key(*n)).map(String::as_str).collect();
    assert_eq!(
        added,
        vec!["model_connection", "model_connection_keep_eliezer", "persona_backend"],
        "genau die zwei neuen Tabellen und der Schutz der Verbindung Eliezer"
    );
    let rows: Vec<(i64, String, String, Option<String>, i64, String)> =
        sqlx::query_as("SELECT id, name, kind, url, external, account FROM model_connection")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(
        rows,
        vec![(1, "Eliezer".to_owned(), "eliezer".to_owned(), None, 0, "eliezer".to_owned())],
        "genau eine Verbindung Eliezer: Adresse nicht doppelt (bleibt in connection), Konto eliezer, lokal"
    );
    let backends: i64 = sqlx::query_scalar("SELECT count(*) FROM persona_backend").fetch_one(&pool).await.unwrap();
    assert_eq!(backends, 0, "keine Zeile je Persona: alle nutzen Eliezer, nicht extern freigegeben");
    let refused = sqlx::query("DELETE FROM model_connection WHERE id = 1").execute(&pool).await;
    assert!(refused.is_err(), "der Trigger schützt die Verbindung Eliezer vor dem Löschen");
    pool.close().await;

    let store = Store::open(dir.path()).await.expect("Store::open nach 0036");
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
    let eliezer = store.connection_get(ConnectionKind::Eliezer).await.unwrap();
    assert_eq!(
        (eliezer.config["url"].as_str(), eliezer.secret_target.as_deref()),
        (Some("http://127.0.0.1:9"), Some("http://127.0.0.1:1234")),
        "Adresse und Bindung des Schlüssels der Verbindung eliezer bleiben nutzbar (Embeddings, Personas)"
    );

    // Store methods on the new tables.
    let backend = store.persona_backend("@lektor").await.unwrap();
    assert_eq!(
        (backend.connection, backend.external_allowed, backend.explicit),
        (ELIEZER_CONNECTION, false, false),
        "Vorgabe je Persona: Eliezer, nicht freigegeben"
    );
    let ollama = store
        .model_connection_create(&ModelConnectionEdit {
            name: "Ollama im Keller".into(),
            kind: "ollama".into(),
            url: "http://127.0.0.1:11434".into(),
            external: false,
        })
        .await
        .unwrap();
    assert_eq!(ollama.account, format!("model-{}", ollama.id), "neues Konto baruch/model-<id>");
    let twice = store
        .model_connection_create(&ModelConnectionEdit {
            name: "ollama im keller".into(),
            kind: "ollama".into(),
            url: "http://127.0.0.1:11435".into(),
            external: false,
        })
        .await;
    assert!(twice.is_err(), "Name eindeutig ohne Rücksicht auf Groß-/Kleinschreibung");
    store.set_persona_backend("belege", ollama.id, true).await.unwrap();
    assert_eq!(store.persona_backend("@belege").await.unwrap().connection, ollama.id, "Handle ohne @ geht auch");
    let moved = store.model_connection_delete(ollama.id).await.unwrap();
    assert_eq!(moved, vec!["@belege".to_owned()], "Löschen nennt die Personas, die zurückfallen");
    assert_eq!(
        store.persona_backend("@belege").await.unwrap().connection,
        ELIEZER_CONNECTION,
        "nach dem Löschen nutzt @belege wieder Eliezer"
    );
    assert!(store.model_connection_delete(ELIEZER_CONNECTION).await.is_err(), "Eliezer bleibt");
    let broken: i64 =
        sqlx::query_scalar("SELECT count(*) FROM pragma_foreign_key_check").fetch_one(store.pool()).await.unwrap();
    assert_eq!(broken, 0, "keine Verstöße gegen Fremdschlüssel");

    // A second opening changes nothing.
    let layout_now = tables(store.pool()).await;
    let once = contents(store.pool(), &layout_now).await;
    drop(store);
    let store = Store::open(dir.path()).await.unwrap();
    assert_eq!(contents(store.pool(), &layout_now).await, once, "zweites Öffnen: alle Zeilen gleich");
}

/// Migration 0041 (Round 26) enters the instance identifier as a new row in `store_meta` at start; that adds one row
/// and changes no old one. The comparison of the old rows takes out exactly this one row.
fn without_instance_row(mut contents: BTreeMap<String, Vec<String>>) -> BTreeMap<String, Vec<String>> {
    if let Some(meta) = contents.get_mut("store_meta") {
        let count = meta.len();
        meta.retain(|row| !row.starts_with("'instance_uuid'\u{1f}"));
        assert_eq!(count - meta.len(), 1, "genau eine neue Zeile instance_uuid in store_meta (Migration 0041)");
    }
    contents
}
