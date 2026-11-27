// SPDX-License-Identifier: AGPL-3.0-or-later
//! Migration 0015 (eigener Rollen-Prompt je Persona, Runde 10 „personas“) auf einer nachgebauten
//! Datenbank im Stand von development davor: Store-Migrationen 0001–0014 und Bibliothek 0101–0105, ein
//! frei erfundenes Werk mit Kapiteln, Vorschlag und Beiträgen, Personas (eine mit eigener Farbe),
//! Einfälle, gespeicherte Verbindungen.
//!
//! Nach einem Migrator bis 0015: Das Schema jeder Tabelle außer `persona` bleibt, `persona` hat genau die
//! neue Spalte `prompt` am Ende, überall NULL (= Vorgabe aus dem Code). Danach `Store::open` mit allen
//! Migrationen: jede Zeile jeder Tabelle in allen alten Spalten unverändert, keine Zeile `lector` in
//! `store_meta` (die Wahl ohne Zeile ist fest `@lektor`), ein zweites Öffnen ändert nichts.

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::path::Path;
use std::str::FromStr;

use baruch_core::LectorChoice;
use baruch_store::{Store, title_key};
use sqlx::SqlitePool;
use sqlx::migrate::Migrator;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

const SOURCE: &str = "#set document(title: \"Der Bienenstock\", author: \"Mara Wabe\")\n\n\
= Schwarm\n\nIm Mai zieht das Volk aus.\n\n= Honig\n\nDer Rahmen ist schwer.\n";

async fn raw_pool(dir: &Path) -> SqlitePool {
    let options = SqliteConnectOptions::from_str("sqlite:")
        .unwrap()
        .filename(dir.join("baruch.sqlite"))
        .create_if_missing(true)
        .foreign_keys(true);
    SqlitePoolOptions::new().max_connections(1).connect_with(options).await.unwrap()
}

/// Store bis 0014 (Stand development) und Bibliothek 0101–0105.
async fn migrate_before(pool: &SqlitePool) {
    let old: Vec<_> = baruch_store::MIGRATOR.iter().filter(|m| m.version <= 14).cloned().collect();
    assert!(old.iter().any(|m| m.version == 14), "Stand development hat 0014");
    assert!(baruch_store::MIGRATOR.iter().any(|m| m.version == 15), "die neue Migration ist 0015");
    let store = Migrator { migrations: Cow::Owned(old), ignore_missing: true, ..Migrator::DEFAULT };
    store.run(pool).await.unwrap();
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../baruch-library/migrations");
    let mut library = Migrator::new(dir).await.unwrap();
    let versions: Vec<i64> = library.iter().map(|m| m.version).collect();
    assert!(versions.starts_with(&[101, 102, 103, 104, 105]), "Bibliothek im Stand 0101–0105: {versions:?}");
    library.migrations = Cow::Owned(library.iter().filter(|m| m.version <= 105).cloned().collect());
    library.ignore_missing = true;
    library.run(pool).await.unwrap();
}

/// Store-Migrationen bis einschließlich `version`.
async fn migrate_upto(pool: &SqlitePool, version: i64) {
    let upto: Vec<_> = baruch_store::MIGRATOR.iter().filter(|m| m.version <= version).cloned().collect();
    let migrator = Migrator { migrations: Cow::Owned(upto), ignore_missing: true, ..Migrator::DEFAULT };
    migrator.run(pool).await.unwrap();
}

async fn exec(pool: &SqlitePool, sql: &str) {
    sqlx::query(sqlx::AssertSqlSafe(sql)).execute(pool).await.unwrap_or_else(|e| panic!("{sql}: {e}"));
}

/// Werk mit zwei Kapiteln, Vorschlag, Beiträgen, Einfällen, Personas und Verbindungen.
async fn insert_rows(pool: &SqlitePool) -> i64 {
    let doc = baruch_typst::parse(SOURCE).unwrap();
    let work: i64 = sqlx::query_scalar(
        "INSERT INTO work (slug, kind, title, author, template, source_name, source_sha256, title_key, audience) \
         VALUES ('der-bienenstock', 'book', 'Der Bienenstock', 'Mara Wabe', ?, 'bienen.typ', 'erfunden', ?, 'Imker') RETURNING id",
    )
    .bind(&doc.template)
    .bind(title_key("Der Bienenstock"))
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
        "INSERT INTO persona (handle, name, role, model, context_length, active, position, color) VALUES \
         ('@lektor', 'Lektor', 'Lektorat', 'probe-a', 32000, 1, 0, '#1c7ed6'), \
         ('@nathan', 'Nathan', 'Auditor', 'probe-b', 64000, 1, 1, NULL), \
         ('@hoerer', 'Hörer', 'Zielgruppe', 'probe-a', 8000, 0, 2, NULL)",
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
         VALUES (?, 2, '= Honig\n\nDer Rahmen ist voll und schwer.\n', 'assistant', '@lektor', 'probe-a', 'proposed', 'Bild', 1)",
    )
    .bind(ids[1])
    .execute(pool)
    .await
    .unwrap();
    for (position, author, body) in
        [(0, "Autor", "Wann schwärmen sie?"), (1, "@nathan", "Kritisch: „im Mai“ ist zu eng.")]
    {
        sqlx::query("INSERT INTO post (node_id, position, author, body, sources) VALUES (?, ?, ?, ?, '[]')")
            .bind(ids[0])
            .bind(position)
            .bind(author)
            .bind(body)
            .execute(pool)
            .await
            .unwrap();
    }
    let idea: i64 = sqlx::query_scalar(
        "INSERT INTO idea (work_id, node_id, note, origin_kind, origin_name) VALUES (?, ?, 'Tanz der Bienen', 'human', 'Autor') RETURNING id",
    )
    .bind(work)
    .bind(ids[0])
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO idea_word (idea_id, position, word, word_key) VALUES (?, 0, 'Tanz', 'tanz')")
        .bind(idea)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO idea_fts (rowid, words, note, quote, source, bible) VALUES (?, 'Tanz', 'Tanz der Bienen', '', '', '')",
    )
    .bind(idea)
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

/// Alle Zeilen über die Spalten von `layout`, sortiert.
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
async fn migration_0015_keeps_every_row_and_only_adds_the_persona_prompt() {
    let dir = tempfile::tempdir().unwrap();
    let pool = raw_pool(dir.path()).await;
    migrate_before(&pool).await;
    let work = insert_rows(&pool).await;
    let layout = tables(&pool).await;
    let schema_before = schema(&pool).await;
    let before = contents(&pool, &layout).await;
    assert_eq!(before["persona"].len(), 3, "drei Personas vor der Migration");
    assert!(before["post"].len() == 2 && before["idea"].len() == 1, "Beiträge und Einfall vorhanden");
    assert!(!layout["persona"].contains(&"prompt".to_owned()), "vorher keine Spalte prompt");

    migrate_upto(&pool, 15).await;
    let schema_after = schema(&pool).await;
    for (name, sql) in &schema_before {
        if name == "persona" {
            continue;
        }
        assert_eq!(schema_after.get(name), Some(sql), "Schema von {name} unverändert (0015 ändert nur persona)");
    }
    let added: Vec<&String> = schema_after.keys().filter(|n| !schema_before.contains_key(*n)).collect();
    assert!(added.is_empty(), "0015 legt keine Tabelle und keinen Index an: {added:?}");
    let mut expected = layout["persona"].clone();
    expected.push("prompt".to_owned());
    assert_eq!(tables(&pool).await["persona"], expected, "persona: genau eine neue Spalte am Ende");
    let persona_sql = schema_after["persona"].as_deref().unwrap();
    assert!(
        persona_sql.contains("prompt TEXT CHECK (prompt IS NULL OR length(prompt) BETWEEN 1 AND 8000)"),
        "Grenze 1–8000 Zeichen als CHECK: {persona_sql}"
    );
    let filled: i64 =
        sqlx::query_scalar("SELECT count(*) FROM persona WHERE prompt IS NOT NULL").fetch_one(&pool).await.unwrap();
    assert_eq!(filled, 0, "alte Personas: NULL = Vorgabe aus dem Code");
    for bad in ["''".to_owned(), format!("'{}'", "x".repeat(8001))] {
        let refused =
            sqlx::query(sqlx::AssertSqlSafe(format!("UPDATE persona SET prompt = {bad} WHERE handle = '@lektor'")))
                .execute(&pool)
                .await;
        assert!(refused.is_err(), "CHECK lehnt einen leeren bzw. zu langen Prompt ab");
    }
    pool.close().await;

    let store = Store::open(dir.path()).await.expect("Store::open nach 0015");
    let pool = store.pool();
    let after = without_instance_row(contents(pool, &layout).await);
    for (table, rows) in &before {
        if table == "_sqlx_migrations" {
            continue;
        }
        assert_eq!(after.get(table), Some(rows), "Tabelle {table}: alle Zeilen in den alten Spalten unverändert");
    }
    let versions: Vec<i64> =
        sqlx::query_scalar("SELECT version FROM _sqlx_migrations ORDER BY version").fetch_all(pool).await.unwrap();
    assert!(versions.contains(&15) && versions.contains(&105), "0015 eingetragen, Bibliothek bleibt: {versions:?}");
    let lector_rows: i64 =
        sqlx::query_scalar("SELECT count(*) FROM store_meta WHERE key = 'lector'").fetch_one(pool).await.unwrap();
    assert_eq!(lector_rows, 0, "keine Datenänderung: keine Zeile für die Lektor-Wahl");
    assert_eq!(store.lector_choice().await.unwrap(), LectorChoice::Fixed("@lektor".into()), "ohne Zeile fest @lektor");
    let loaded = store.load_work(work).await.unwrap();
    assert_eq!(loaded.lector().unwrap().handle, "@lektor", "fest @lektor, obwohl @nathan mehr Kontext hat");
    assert!(store.persona_list().await.unwrap().iter().all(|p| p.prompt.is_none()), "alle Prompts = Vorgabe");
    let broken: i64 =
        sqlx::query_scalar("SELECT count(*) FROM pragma_foreign_key_check").fetch_one(pool).await.unwrap();
    assert_eq!(broken, 0, "keine Verstöße gegen Fremdschlüssel");
    assert_eq!(
        baruch_typst::xref::strip_key_labels(&store.export_typst(work).await.unwrap()).into_owned(),
        SOURCE,
        "Export byte-gleich wie vor der Migration"
    );

    // Zweites Öffnen ändert nichts.
    let layout_now = tables(pool).await;
    let schema_once = schema(pool).await;
    let once = contents(pool, &layout_now).await;
    drop(store);
    let store = Store::open(dir.path()).await.unwrap();
    assert_eq!(schema(store.pool()).await, schema_once, "zweites Öffnen: Schema gleich");
    assert_eq!(contents(store.pool(), &layout_now).await, once, "zweites Öffnen: alle Zeilen gleich");
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
