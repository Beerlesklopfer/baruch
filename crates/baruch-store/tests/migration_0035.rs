// SPDX-License-Identifier: AGPL-3.0-or-later
//! Migration 0035 (user question 12: request "create part" with the kind `add`) on a reproduced database in the state
//! of development before it (all store migrations below 0035 and the library), with a freely invented work, chapters
//! and three requests (open, confirmed, declined); the request with the highest id is deleted before the migration
//! (checks that the AUTOINCREMENT counter moves along).
//!
//! After a migrator up to 0035: every row of every table stays (including id), the schema of every other table and
//! every other index stays, `outline_request` differs only in the check of `op` (now with `add`), the index
//! `outline_request_open` is back, no foreign key violations. Before, the database rejects `op = 'add'`, afterwards it
//! accepts it; the next id lies above the deleted one. Afterwards `Store::open` without a change, export
//! byte-identical, and a request through `outline_add_by_assistant` can be filed.

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::path::Path;
use std::str::FromStr;

use baruch_store::{AssistantAdd, IdeaOrigin, NewPoint, Place, Store, WorkId, title_key};
use sqlx::SqlitePool;
use sqlx::migrate::Migrator;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

const SOURCE: &str = "#set document(title: \"Die Mühle am Bach\", author: \"Lene Korn\")\n\n\
= Das Rad\n\nEs dreht sich langsam.\n\n= Der Stein\n\nEr mahlt fein.\n\n= Das Mehl\n\nWeiß und warm.\n";

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

/// Store below 0035 (development before this round ends at 0034) and the whole library.
async fn migrate_before(pool: &SqlitePool) {
    assert!(baruch_store::MIGRATOR.iter().any(|m| m.version == 35), "die neue Migration ist 0035");
    assert!(baruch_store::MIGRATOR.iter().any(|m| m.version == 34), "Stand davor hat 0034");
    migrate_where(pool, |v| v < 35).await;
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../baruch-library/migrations");
    let mut library = Migrator::new(dir).await.unwrap();
    library.ignore_missing = true;
    library.run(pool).await.unwrap();
}

/// Work with three chapters and three requests; the youngest request is deleted afterwards.
async fn insert_rows(pool: &SqlitePool) -> (WorkId, Vec<i64>) {
    let doc = baruch_typst::parse(SOURCE).unwrap();
    let work: WorkId = sqlx::query_scalar(
        "INSERT INTO work (slug, kind, title, author, template, source_name, source_sha256, title_key) \
         VALUES ('die-muehle-am-bach', 'book', 'Die Mühle am Bach', 'Lene Korn', ?, 'muehle.typ', 'erfunden', ?) \
         RETURNING id",
    )
    .bind(&doc.template)
    .bind(title_key("Die Mühle am Bach"))
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
    let mut keys = Vec::new();
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
             VALUES (?, 1, ?, 'human', 'Autor', NULL, 'current', 'Import')",
        )
        .bind(id)
        .bind(&heading.body)
        .execute(pool)
        .await
        .unwrap();
        keys.push(id);
    }
    let rename = format!(
        r#"{{"op":{{"op":"rename","key":{},"title":"Die Achse"}},"expected":{{"title":"Das Rad","level":1}}}}"#,
        keys[0]
    );
    let delete = format!(r#"{{"op":{{"op":"delete","key":{}}},"expected":{{"title":"Das Mehl","level":1}}}}"#, keys[2]);
    for (op, params, decided, decision) in [
        ("rename", rename.as_str(), None, None),
        ("delete", delete.as_str(), Some("2026-11-06T10:00:00.000Z"), Some("rejected")),
        ("rename", rename.as_str(), Some("2026-11-06T11:00:00.000Z"), Some("confirmed")),
        ("delete", delete.as_str(), None, None),
    ] {
        sqlx::query(
            "INSERT INTO outline_request (work_id, op, params, origin_kind, origin_name, note, decided, decision, \
             decided_by) VALUES (?, ?, ?, 'mcp', '@mcp:probe', 'klingt besser', ?, ?, ?)",
        )
        .bind(work)
        .bind(op)
        .bind(params)
        .bind(decided)
        .bind(decision)
        .bind(decided.map(|_| "Autor"))
        .execute(pool)
        .await
        .unwrap();
    }
    // The youngest request (id 4) vanishes: its id must not be assigned anew afterwards.
    sqlx::query("DELETE FROM outline_request WHERE id = 4").execute(pool).await.unwrap();
    (work, keys)
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

async fn insert_add(pool: &SqlitePool, work: WorkId) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(
        "INSERT INTO outline_request (work_id, op, params, origin_kind, origin_name) \
         VALUES (?, 'add', '{}', 'mcp', '@mcp:probe') RETURNING id",
    )
    .bind(work)
    .fetch_one(pool)
    .await
}

#[tokio::test]
async fn migration_0035_keeps_every_request_and_only_allows_the_add_kind() {
    let dir = tempfile::tempdir().unwrap();
    let pool = raw_pool(dir.path()).await;
    migrate_before(&pool).await;
    let (work, keys) = insert_rows(&pool).await;
    let layout = tables(&pool).await;
    let schema_before = schema(&pool).await;
    let before = contents(&pool, &layout).await;
    assert_eq!(before["outline_request"].len(), 3, "drei Anträge vor der Migration (ein vierter ist gelöscht)");
    {
        // Counter-check in a rolled-back transaction: before 0035 the check does not know `add`.
        let mut tx = pool.begin().await.unwrap();
        let refused = sqlx::query("INSERT INTO outline_request (work_id, op, params, origin_kind, origin_name) VALUES (?, 'add', '{}', 'mcp', 'x')")
            .bind(work)
            .execute(&mut *tx)
            .await;
        assert!(refused.is_err(), "vor 0035 lehnt CHECK die Art add ab (deshalb braucht es die Migration)");
        tx.rollback().await.unwrap();
    }

    migrate_where(&pool, |v| v <= 35).await;
    let schema_after = schema(&pool).await;
    assert_eq!(
        schema_after.keys().collect::<Vec<_>>(),
        schema_before.keys().collect::<Vec<_>>(),
        "dieselben Tabellen und Indizes (keine Hilfstabelle bleibt zurück)"
    );
    for (name, sql) in &schema_before {
        if name == "outline_request" {
            continue;
        }
        assert_eq!(schema_after.get(name), Some(sql), "Schema von {name} unverändert");
    }
    let old = schema_before["outline_request"].clone().unwrap();
    let new = schema_after["outline_request"].clone().unwrap();
    assert_eq!(
        new.replace("'set_numbered','add'", "'set_numbered'").replace("\"outline_request\"", "outline_request"),
        old,
        "outline_request unterscheidet sich nur in der Art add (Spalten, Vorgaben und übrige Prüfungen gleich)"
    );
    assert_eq!(tables(&pool).await, layout, "Spalten aller Tabellen in derselben Reihenfolge");
    let after = contents(&pool, &layout).await;
    for (table, rows) in &before {
        if table == "_sqlx_migrations" || table == "sqlite_sequence" {
            continue;
        }
        assert_eq!(after.get(table), Some(rows), "Tabelle {table}: alle Zeilen (samt id) unverändert");
    }
    let added = insert_add(&pool, work).await.expect("nach 0035 nimmt die Datenbank die Art add an");
    assert_eq!(added, 5, "die id des gelöschten Antrags 4 wird nicht neu vergeben (AUTOINCREMENT-Zählung mitgenommen)");
    sqlx::query("DELETE FROM outline_request WHERE id = ?").bind(added).execute(&pool).await.unwrap();
    let broken: i64 =
        sqlx::query_scalar("SELECT count(*) FROM pragma_foreign_key_check").fetch_one(&pool).await.unwrap();
    assert_eq!(broken, 0, "keine Verstöße gegen Fremdschlüssel");
    pool.close().await;

    let store = Store::open(dir.path()).await.expect("Store::open nach 0035");
    let reopened = contents(store.pool(), &layout).await;
    assert_eq!(reopened["outline_request"], before["outline_request"], "Store::open ändert die Anträge nicht");
    assert_eq!(
        baruch_typst::xref::strip_key_labels(&store.export_typst(work).await.unwrap()).into_owned(),
        SOURCE,
        "Export byte-gleich wie vor der Migration"
    );
    let listed = store.outline_requests(work, false).await.unwrap();
    assert_eq!(listed.len(), 3, "die alten Anträge lassen sich lesen: {listed:?}");

    // Afterwards: a part before „Der Stein“ would take over two chapters → request of kind add, nothing created.
    let origin = IdeaOrigin::Mcp { handle: "@mcp:probe".into() };
    let point = NewPoint { title: "Vom Mahlen".into(), level: Some(0) };
    let outcome =
        store.outline_add_by_assistant(work, Some(1), Place::Before, &[point], &origin, "probe", "").await.unwrap();
    let AssistantAdd::Requested(request) = outcome else { panic!("Antrag erwartet: {outcome:?}") };
    assert_eq!(request.expected.adopted, vec![keys[1], keys[2]], "übernommen würden „Der Stein“ und „Das Mehl“");
    assert_eq!(store.load_work(work).await.unwrap().nodes.len(), 3, "nichts angelegt");
}
