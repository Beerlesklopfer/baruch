// SPDX-License-Identifier: AGPL-3.0-or-later
//! Migration 0012 (Anträge zur Gliederung, Wunsch 2026-11-06) auf einer nachgebauten Datenbank im
//! Stand vor dieser Runde: Store-Migrationen 0001–0011 und alle Bibliotheks-Migrationen, ein frei
//! erfundenes Werk mit Vorlage, Kapiteln, Abschnitt, Vorschlag, Beitrag und Einfall.
//!
//! Danach `Store::open`: Alle Zeilen und Spalten aller vorhandenen Tabellen bleiben (verglichen über
//! die Spalten von vorher), neu ist nur `outline_request`, keine Verstöße gegen Fremdschlüssel, Export
//! byte-gleich. Danach lässt sich ein Antrag stellen und bestätigen; ein gelöschtes Werk nimmt seine
//! Anträge mit. Die neuen Tabellen werden nach einem Migrator bis 0012 bestimmt (Runde 10), nicht nach
//! `Store::open` mit allen späteren Migrationen.

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::path::Path;
use std::str::FromStr;

use baruch_store::{IdeaOrigin, OutlineOp, Store, WorkId, title_key};
use sqlx::SqlitePool;
use sqlx::migrate::Migrator;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

const SOURCE: &str = "#set document(title: \"Das Brunnenbuch\", author: \"Ida Feder\")\n#set heading(numbering: \"1.1\")\n\n\
= Quelle\n\nDas Wasser steigt.\n\n== Eimer\n\nAus Eiche.\n\n= Rinne\n\nEs fließt ab.\n";

async fn raw_pool(dir: &Path) -> SqlitePool {
    let options = SqliteConnectOptions::from_str("sqlite:")
        .unwrap()
        .filename(dir.join("baruch.sqlite"))
        .create_if_missing(true)
        .foreign_keys(true);
    SqlitePoolOptions::new().max_connections(1).connect_with(options).await.unwrap()
}

/// Store-Migrationen bis einschließlich `version` auf der nachgebauten Datenbank (Runde 10): Das Schema
/// wird nach diesem Migrator verglichen, damit spätere Migrationen (z. B. 0015 an `persona`) den Vergleich
/// nicht brechen; `Store::open` danach prüft nur noch den Datenerhalt.
async fn migrate_upto(pool: &SqlitePool, version: i64) {
    let upto: Vec<_> = baruch_store::MIGRATOR.iter().filter(|m| m.version <= version).cloned().collect();
    assert!(upto.iter().any(|m| m.version == version), "MIGRATOR enthält {version}");
    let migrator = Migrator { migrations: Cow::Owned(upto), ignore_missing: true, ..Migrator::DEFAULT };
    migrator.run(pool).await.unwrap();
}

/// Store bis 0011, Bibliothek vollständig (sie hat eigene Nummern ab 0101).
async fn migrate_before(pool: &SqlitePool) {
    let old: Vec<_> = baruch_store::MIGRATOR.iter().filter(|m| m.version <= 11).cloned().collect();
    assert!(old.iter().any(|m| m.version == 11), "Stand vorher hat 0011");
    assert!(baruch_store::MIGRATOR.iter().any(|m| m.version == 12), "die neue Migration ist 0012");
    let store = Migrator { migrations: Cow::Owned(old), ignore_missing: true, ..Migrator::DEFAULT };
    store.run(pool).await.unwrap();
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../baruch-library/migrations");
    let mut library = Migrator::new(dir).await.unwrap();
    library.ignore_missing = true;
    library.run(pool).await.unwrap();
}

async fn insert_book(pool: &SqlitePool) -> WorkId {
    let doc = baruch_typst::parse(SOURCE).unwrap();
    let work: WorkId = sqlx::query_scalar(
        "INSERT INTO work (slug, kind, title, author, template, source_name, source_sha256, title_key) \
         VALUES ('das-brunnenbuch', 'book', 'Das Brunnenbuch', 'Ida Feder', ?, 'brunnen.typ', 'erfunden', ?) RETURNING id",
    )
    .bind(&doc.template)
    .bind(title_key("Das Brunnenbuch"))
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO work_template_revision (work_id, number, body, author_name, note) VALUES (?, 1, ?, 'Autor', 'Anfang')")
        .bind(work)
        .bind(&doc.template)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO store_meta (key, value) VALUES ('seeded', '1') ON CONFLICT DO NOTHING")
        .execute(pool)
        .await
        .unwrap();
    // Quelle (0), Eimer (1, unter 0), Rinne (2).
    let layout = [(None, 0, "1", "chapter"), (Some(0), 0, "1.1", "section"), (None, 1, "2", "chapter")];
    let mut ids = Vec::new();
    for (seq, (heading, (parent, position, number, kind))) in doc.headings.iter().zip(layout).enumerate() {
        let id: i64 = sqlx::query_scalar(
            "INSERT INTO node (work_id, seq, parent_seq, position, number, title, kind) VALUES (?, ?, ?, ?, ?, ?, ?) RETURNING id",
        )
        .bind(work)
        .bind(seq as i64)
        .bind(parent)
        .bind(position)
        .bind(number)
        .bind(&heading.title)
        .bind(kind)
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
         VALUES (?, 2, '== Eimer\n\nAus Buche.\n', 'assistant', '@lektor', 'probe', 'proposed', 'Holz', 1)",
    )
    .bind(ids[1])
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO post (node_id, position, author, body, sources) VALUES (?, 0, '@nathan', 'Wie tief?', '[]')",
    )
    .bind(ids[0])
    .execute(pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO idea (work_id, node_id, note, origin_kind, origin_name) VALUES (?, ?, 'Brunnenlied', 'human', 'Autor')")
        .bind(work)
        .bind(ids[2])
        .execute(pool)
        .await
        .unwrap();
    work
}

async fn columns(pool: &SqlitePool) -> BTreeMap<String, Vec<String>> {
    let tables: Vec<String> = sqlx::query_scalar("SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name")
        .fetch_all(pool)
        .await
        .unwrap();
    let mut out = BTreeMap::new();
    for table in tables {
        let names: Vec<String> = sqlx::query_scalar("SELECT name FROM pragma_table_info(?) ORDER BY cid")
            .bind(&table)
            .fetch_all(pool)
            .await
            .unwrap();
        out.insert(table, names);
    }
    out
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
async fn migration_0012_keeps_every_row_and_adds_only_the_request_table() {
    let dir = tempfile::tempdir().unwrap();
    let pool = raw_pool(dir.path()).await;
    migrate_before(&pool).await;
    let work = insert_book(&pool).await;
    let layout = columns(&pool).await;
    let before = contents(&pool, &layout).await;
    assert!(!layout.contains_key("outline_request"), "vorher gibt es outline_request nicht");
    assert!(before["revision"].len() == 4 && before["idea"].len() == 1, "Daten vorhanden: {:?}", before["revision"]);
    migrate_upto(&pool, 12).await;
    let new_tables: Vec<String> = columns(&pool).await.into_keys().filter(|t| !layout.contains_key(t)).collect();
    assert_eq!(new_tables, ["outline_request"], "nur die Antragstabelle ist neu (nach 0012)");
    pool.close().await;

    let store = Store::open(dir.path()).await.expect("Store::open führt 0012 aus");
    let pool = store.pool();
    let after = without_instance_row(contents(pool, &layout).await);
    for (table, rows) in &before {
        if table == "_sqlx_migrations" {
            continue;
        }
        assert_eq!(after.get(table), Some(rows), "Tabelle {table}: alle Zeilen und Spalten unverändert");
    }
    assert!(columns(pool).await.contains_key("outline_request"), "nach Store::open gibt es outline_request");
    let broken: i64 =
        sqlx::query_scalar("SELECT count(*) FROM pragma_foreign_key_check").fetch_one(pool).await.unwrap();
    assert_eq!(broken, 0, "keine Verstöße gegen Fremdschlüssel");
    assert_eq!(
        baruch_typst::xref::strip_key_labels(&store.export_typst(work).await.unwrap()).into_owned(),
        SOURCE,
        "Export byte-gleich wie vor der Migration"
    );

    // Danach: Antrag stellen, bestätigen; Werk löschen nimmt die Anträge mit.
    let key: i64 = sqlx::query_scalar("SELECT id FROM node WHERE work_id = ? AND seq = 2")
        .bind(work)
        .fetch_one(pool)
        .await
        .unwrap();
    let origin = IdeaOrigin::Mcp { handle: "@mcp:probe".into() };
    let request =
        store.outline_request(work, OutlineOp::Rename { key, title: "Graben".into() }, &origin, "").await.unwrap();
    assert!(request.is_open(), "neuer Antrag ist offen");
    let decided = store.outline_request_decide(work, request.id, true, "Autor").await.unwrap();
    assert!(decided.change.is_some(), "bestätigt und ausgeführt: {decided:?}");
    let titles: Vec<String> = store.load_work(work).await.unwrap().nodes.iter().map(|n| n.title.clone()).collect();
    assert_eq!(titles, ["Quelle", "Eimer", "Graben"], "umbenannt");
    store.outline_request(work, OutlineOp::Delete { key }, &origin, "").await.unwrap();
    store.delete_work(work, "Das Brunnenbuch").await.unwrap();
    let left: i64 = sqlx::query_scalar("SELECT count(*) FROM outline_request").fetch_one(pool).await.unwrap();
    assert_eq!(left, 0, "ein gelöschtes Werk nimmt seine Anträge mit (ON DELETE CASCADE)");
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
