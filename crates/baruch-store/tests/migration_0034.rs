// SPDX-License-Identifier: AGPL-3.0-or-later
//! Migration 0034 (Belege der Websuche, Runde 13 „websuche“) auf einer nachgebauten Datenbank im Stand davor
//! (alle Store-Migrationen unter 0034 – im Stand von development bis 0032, nach dem Zusammenführen auch 0033 –
//! und Bibliothek 0101–0105), mit frei erfundenem Werk, Beiträgen samt Quellen und Bezug, Personas und
//! Einstellungen in `store_meta`.
//!
//! Nach einem Migrator bis 0034: Das Schema jeder alten Tabelle bleibt, neu sind genau `web_evidence` und der
//! Index `web_evidence_post`, die Tabelle ist leer. Danach `Store::open`: jede Zeile jeder alten Tabelle
//! unverändert; ein Beleg zu einem Beitrag lässt sich anlegen, das Löschen des Beitrags setzt `post_id` auf
//! NULL (der Beleg bleibt), ein zweites Öffnen ändert nichts.

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::path::Path;
use std::str::FromStr;

use baruch_store::{Store, title_key};
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

/// Store unter 0034 und Bibliothek 0101–0105.
async fn migrate_before(pool: &SqlitePool) {
    assert!(baruch_store::MIGRATOR.iter().any(|m| m.version == 34), "die neue Migration ist 0034");
    assert!(
        baruch_store::MIGRATOR.iter().any(|m| m.version == 32),
        "Stand davor hat mindestens 0032 (development zu Beginn von Runde 13)"
    );
    migrate_where(pool, |v| v < 34).await;
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../baruch-library/migrations");
    let mut library = Migrator::new(dir).await.unwrap();
    library.migrations = Cow::Owned(library.iter().filter(|m| m.version <= 105).cloned().collect());
    library.ignore_missing = true;
    library.run(pool).await.unwrap();
}

async fn exec(pool: &SqlitePool, sql: &str) {
    sqlx::query(sql).execute(pool).await.unwrap_or_else(|e| panic!("{sql}: {e}"));
}

/// Werk mit zwei Kapiteln, drei Beiträge (einer mit Quellen und Bezug), Personas, Einstellungen.
async fn insert_rows(pool: &SqlitePool) -> (i64, Vec<i64>) {
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
         ('@lektor', 'Lektor', 'Lektorat', 'probe-a', 32000, 1, 0, '#1c7ed6', NULL), \
         ('@belege', 'Belege', 'Quellen', 'probe-a', 16000, 1, 1, NULL, 'Eigener Prompt.')",
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
    let mut posts = Vec::new();
    for (node, position, author, body, sources) in [
        (nodes[0], 0, "Autor", "Woher stammt das Bild vom kreisenden Licht?", "[]"),
        (nodes[0], 1, "@belege", "Nicht belegt; bitte prüfen.", "[\"Einfall 3\"]"),
        (nodes[1], 0, "@lektor", "Der Nebel passt.", "[]"),
    ] {
        let id: i64 = sqlx::query_scalar(
            "INSERT INTO post (node_id, position, author, body, sources) VALUES (?, ?, ?, ?, ?) RETURNING id",
        )
        .bind(node)
        .bind(position)
        .bind(author)
        .bind(body)
        .bind(sources)
        .fetch_one(pool)
        .await
        .unwrap();
        posts.push(id);
    }
    sqlx::query("INSERT INTO post_ref (post_id, position, node_seq, rev, quote) VALUES (?, 0, 0, 1, 'kreist')")
        .bind(posts[1])
        .execute(pool)
        .await
        .unwrap();
    exec(
        pool,
        "INSERT INTO preferred_source (title, url, note, position) VALUES ('Lichtkunde', 'https://lichtkunde.example/', '', 1)",
    )
    .await;
    (work, posts)
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

async fn evidence(pool: &SqlitePool, post: i64, verdict: &str) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(
        "INSERT INTO web_evidence (post_id, work_id, node_key, persona, label, url, title, accessed, quote, verdict) \
         VALUES (?, 1, 1, '@belege', 'W1', 'https://lichtkunde.example/a', 'Lichtkunde', '2026-11-07T09:30:05Z', \
         'Das Licht kreist', ?) RETURNING id",
    )
    .bind(post)
    .bind(verdict)
    .fetch_one(pool)
    .await
}

#[tokio::test]
async fn migration_0034_keeps_every_row_and_only_adds_web_evidence() {
    let dir = tempfile::tempdir().unwrap();
    let pool = raw_pool(dir.path()).await;
    migrate_before(&pool).await;
    let (work, posts) = insert_rows(&pool).await;
    let layout = tables(&pool).await;
    let schema_before = schema(&pool).await;
    let before = contents(&pool, &layout).await;
    assert_eq!(
        before["post"].len(),
        3,
        "drei Beiträge vor der Migration (Begründung: daran hängt der neue Fremdschlüssel)"
    );
    assert_eq!(before["post_ref"].len(), 1, "ein Bezug eines Beitrags");

    migrate_where(&pool, |v| v <= 34).await;
    let schema_after = schema(&pool).await;
    for (name, sql) in &schema_before {
        assert_eq!(schema_after.get(name), Some(sql), "Schema von {name} unverändert (0034 ändert keine alte Tabelle)");
    }
    let added: Vec<&str> =
        schema_after.keys().filter(|n| !schema_before.contains_key(*n)).map(String::as_str).collect();
    assert_eq!(
        added,
        vec!["web_evidence", "web_evidence_post"],
        "genau die neue Tabelle und ihr Index nach post_id (die Belegseite liest je Beitrag)"
    );
    let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM web_evidence").fetch_one(&pool).await.unwrap();
    assert_eq!(rows, 0, "keine Datenänderung: die neue Tabelle ist leer");
    assert!(evidence(&pool, posts[1], "erfunden").await.is_err(), "CHECK lässt nur die drei Urteile zu");
    assert!(evidence(&pool, 99_999, "belegt").await.is_err(), "Fremdschlüssel: nur vorhandene Beiträge");
    pool.close().await;

    let store = Store::open(dir.path()).await.expect("Store::open nach 0034");
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
    let switch: Option<String> = sqlx::query_scalar("SELECT value FROM store_meta WHERE key = 'websearch_belege'")
        .fetch_optional(store.pool())
        .await
        .unwrap();
    assert_eq!(switch, None, "der Schalter der Websuche fehlt nach der Migration (Vorgabe aus)");

    let kept = evidence(store.pool(), posts[1], "belegt").await.expect("Beleg zum Beitrag anlegen");
    let other = evidence(store.pool(), posts[2], "nicht_zitiert").await.unwrap();

    // Zweites Öffnen ändert nichts.
    let layout_now = tables(store.pool()).await;
    let schema_once = schema(store.pool()).await;
    let once = contents(store.pool(), &layout_now).await;
    drop(store);
    let store = Store::open(dir.path()).await.unwrap();
    assert_eq!(schema(store.pool()).await, schema_once, "zweites Öffnen: Schema gleich");
    assert_eq!(contents(store.pool(), &layout_now).await, once, "zweites Öffnen: alle Zeilen gleich");

    // Löschen des Beitrags (über den Store, wie die Oberfläche) setzt post_id auf NULL; der Beleg bleibt.
    let deleted = store.post_delete(work, posts[1]).await.expect("Beitrag löschen");
    assert_eq!(deleted.id, posts[1], "der Beitrag mit Beleg ist gelöscht");
    let post: Option<i64> = sqlx::query_scalar("SELECT post_id FROM web_evidence WHERE id = ?")
        .bind(kept)
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!(post, None, "ON DELETE SET NULL: der Beleg überdauert ohne Beitrag");
    let untouched: Option<i64> = sqlx::query_scalar("SELECT post_id FROM web_evidence WHERE id = ?")
        .bind(other)
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!(untouched, Some(posts[2]), "der Beleg eines anderen Beitrags bleibt verknüpft");
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
