// SPDX-License-Identifier: AGPL-3.0-or-later
//! Migration 0037 (Kommentare und Lexika je Werk, Schalter Bibel-Belege je Persona; Runde 15 Punkt 5a/5c) auf
//! einer nachgebauten Datenbank im Stand von development (Store bis 0035, Bibliothek 0101–0105) samt 0036 aus
//! diesem Zweig, mit frei erfundenem Werk, Personas und gewählter Bibel.
//!
//! Nach einem Migrator bis 0037: Das Schema jeder alten Tabelle bleibt, neu sind genau `work_sword_source` und
//! `persona_bible`, beide leer (Vorgaben des Servers gelten). Danach `Store::open`: jede alte Zeile unverändert;
//! Auswahl je Werk und Schalter je Persona lassen sich setzen, ein gelöschtes Werk nimmt seine Auswahl mit.

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::path::Path;
use std::str::FromStr;

use baruch_store::{Store, title_key};
use sqlx::SqlitePool;
use sqlx::migrate::Migrator;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

const SOURCE: &str = "#set document(title: \"Der Brunnen im Hof\", author: \"Tilde Marsch\")\n\n\
= Wasser\n\nDer Eimer schlägt an die Steine.\n\n= Stille\n\nNiemand schöpft mehr.\n";

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

/// Store bis 0036 (development bis 0035 und 0036 dieses Zweigs) und Bibliothek 0101–0105.
async fn migrate_before(pool: &SqlitePool) {
    assert!(baruch_store::MIGRATOR.iter().any(|m| m.version == 37), "die neue Migration ist 0037");
    assert!(baruch_store::MIGRATOR.iter().any(|m| m.version == 35), "Stand davor hat 0035 (development, ca04c58)");
    migrate_where(pool, |v| v < 37).await;
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../baruch-library/migrations");
    let mut library = Migrator::new(dir).await.unwrap();
    library.migrations = Cow::Owned(library.iter().filter(|m| m.version <= 105).cloned().collect());
    library.ignore_missing = true;
    library.run(pool).await.unwrap();
}

async fn exec(pool: &SqlitePool, sql: &str) {
    sqlx::query(sql).execute(pool).await.unwrap_or_else(|e| panic!("{sql}: {e}"));
}

/// Werk mit zwei Kapiteln, Personas und gewählter Bibel.
async fn insert_rows(pool: &SqlitePool) -> i64 {
    let doc = baruch_typst::parse(SOURCE).unwrap();
    let work: i64 = sqlx::query_scalar(
        "INSERT INTO work (slug, kind, title, author, template, source_name, source_sha256, title_key, audience) \
         VALUES ('der-brunnen-im-hof', 'book', 'Der Brunnen im Hof', 'Tilde Marsch', ?, 'brunnen.typ', \
         'erfunden', ?, 'Hofleute') RETURNING id",
    )
    .bind(&doc.template)
    .bind(title_key("Der Brunnen im Hof"))
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
         ('@belege', 'Belege', 'Quellen', 'probe-a', 16000, 1, 1, NULL, NULL)",
    )
    .await;
    sqlx::query("INSERT INTO work_bible (work_id, module) VALUES (?, 'ErfundeneBibel')")
        .bind(work)
        .execute(pool)
        .await
        .unwrap();
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
            sqlx::query_scalar(&format!("SELECT {select} FROM \"{table}\"")).fetch_all(pool).await.unwrap();
        rows.sort();
        out.insert(table.clone(), rows);
    }
    out
}

#[tokio::test]
async fn migration_0037_adds_empty_tables_for_work_sources_and_the_bible_switch() {
    let dir = tempfile::tempdir().unwrap();
    let pool = raw_pool(dir.path()).await;
    migrate_before(&pool).await;
    let work = insert_rows(&pool).await;
    let layout = tables(&pool).await;
    let schema_before = schema(&pool).await;
    let before = contents(&pool, &layout).await;
    assert_eq!(before["work_bible"].len(), 1, "die gewählte Bibel des Werks steht vor der Migration");

    migrate_where(&pool, |v| v <= 37).await;
    let schema_after = schema(&pool).await;
    for (name, sql) in &schema_before {
        assert_eq!(schema_after.get(name), Some(sql), "Schema von {name} unverändert (0037 legt nur Neues an)");
    }
    let added: Vec<&str> =
        schema_after.keys().filter(|n| !schema_before.contains_key(*n)).map(String::as_str).collect();
    assert_eq!(added, vec!["persona_bible", "work_sword_source"], "genau die zwei neuen Tabellen");
    for table in ["persona_bible", "work_sword_source"] {
        let n: i64 = sqlx::query_scalar(&format!("SELECT count(*) FROM {table}")).fetch_one(&pool).await.unwrap();
        assert_eq!(n, 0, "{table} ist leer: es gelten die Vorgaben (keine Auswahl, Schalter nach Persona)");
    }
    let bad =
        sqlx::query("INSERT INTO work_sword_source (work_id, kind, module, position) VALUES (?, 'bible', 'X', 1)")
            .bind(work)
            .execute(&pool)
            .await;
    assert!(bad.is_err(), "CHECK: nur commentary und lexicon (die Bibel des Werks bleibt in work_bible)");
    pool.close().await;

    let store = Store::open(dir.path()).await.expect("Store::open nach 0037");
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
    assert_eq!(store.work_bible(work).await.unwrap().as_deref(), Some("ErfundeneBibel"), "Bibel des Werks bleibt");
    assert_eq!(store.persona_bible("@belege").await.unwrap(), None, "keine Zeile: Vorgabe des Servers");

    let set = store
        .set_work_sword_sources(work, &["KomA".into(), " koma ".into(), "KomB".into()], &["LexA".into()])
        .await
        .unwrap();
    assert_eq!(set.commentaries, vec!["KomA".to_owned(), "KomB".to_owned()], "Reihenfolge bleibt, Doppel fallen weg");
    assert_eq!(store.work_sword_sources(work).await.unwrap(), set, "gelesen wie gesetzt");
    assert!(store.set_persona_bible("@lektor", true).await.unwrap(), "Schalter an");
    assert_eq!(store.persona_bible("lektor").await.unwrap(), Some(true), "Handle ohne @ geht auch");
    assert!(store.persona_bible("@niemand").await.is_err(), "unbekannte Persona wird abgelehnt");
    store.delete_work(work, "Der Brunnen im Hof").await.expect("Werk löschen");
    let left: i64 = sqlx::query_scalar("SELECT count(*) FROM work_sword_source").fetch_one(store.pool()).await.unwrap();
    assert_eq!(left, 0, "ON DELETE CASCADE: ein gelöschtes Werk nimmt seine Auswahl mit");
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
