// SPDX-License-Identifier: AGPL-3.0-or-later
//! Migration 0017 (book details and cover, Round 11, task cover) on a reproduced database in the state of development
//! before it: store migrations 0001–0016 and library 0101 ff., a freely invented work with chapters, personas, posts,
//! ideas, connections, an MCP token and two images.
//!
//! Afterwards `Store::open`: every row of every table stays unchanged, the schema of every old table likewise; only the
//! empty table `work_book` with its triggers is added. A cover from a foreign work is rejected, deleting the cover
//! image sets it to NULL, and a work with a new row can be deleted. Later migrations of other tasks (e.g. 0018) run
//! along on opening; therefore only what concerns 0017 is compared (no assumption "last migration = 17").

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::path::Path;
use std::str::FromStr;

use baruch_store::{Store, title_key};
use sqlx::SqlitePool;
use sqlx::migrate::Migrator;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

const SOURCE: &str = "#set document(title: \"Die Laterne am Kai\", author: \"Ida Morgenstern\")\n\n\
= Abend\n\nDie Fähre legt an.\n\n= Nacht\n\nNur die Laterne brennt noch.\n";

async fn raw_pool(dir: &Path) -> SqlitePool {
    let options = SqliteConnectOptions::from_str("sqlite:")
        .unwrap()
        .filename(dir.join("baruch.sqlite"))
        .create_if_missing(true)
        .foreign_keys(true);
    SqlitePoolOptions::new().max_connections(1).connect_with(options).await.unwrap()
}

/// Store up to 0016 (state of development before Round 11) and the library.
async fn migrate_before(pool: &SqlitePool) {
    let old: Vec<_> = baruch_store::MIGRATOR.iter().filter(|m| m.version <= 16).cloned().collect();
    assert!(old.iter().any(|m| m.version == 16), "Stand vorher hat 0016");
    let store = Migrator { migrations: Cow::Owned(old), ignore_missing: true, ..Migrator::DEFAULT };
    store.run(pool).await.unwrap();
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../baruch-library/migrations");
    let mut library = Migrator::new(dir).await.unwrap();
    library.ignore_missing = true;
    library.run(pool).await.unwrap();
}

async fn exec(pool: &SqlitePool, sql: &str) {
    sqlx::query(sqlx::AssertSqlSafe(sql)).execute(pool).await.unwrap_or_else(|e| panic!("{sql}: {e}"));
}

async fn insert_rows(pool: &SqlitePool) -> i64 {
    let doc = baruch_typst::parse(SOURCE).unwrap();
    let work: i64 = sqlx::query_scalar(
        "INSERT INTO work (slug, kind, title, author, template, source_name, source_sha256, title_key) \
         VALUES ('die-laterne-am-kai', 'book', 'Die Laterne am Kai', 'Ida Morgenstern', ?, 'kai.typ', 'erfunden', ?) RETURNING id",
    )
    .bind(&doc.template)
    .bind(title_key("Die Laterne am Kai"))
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
    let mut first_node = 0;
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
        if seq == 0 {
            first_node = id;
        }
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
    exec(
        pool,
        "INSERT INTO persona (handle, name, role, model, context_length, active, position, color) VALUES \
         ('@lektor', 'Lektor', 'Lektorat', 'erfundenes-modell', 32768, 1, 0, '#336699'), \
         ('@hoerer', 'Hörer', 'Gemeinde', 'erfundenes-modell', 8192, 0, 1, NULL)",
    )
    .await;
    sqlx::query("INSERT INTO post (node_id, position, author, body) VALUES (?, 0, 'Autor', 'Brennt die Laterne zu lange?'), (?, 1, '@lektor', 'Das Bild trägt.')")
        .bind(first_node)
        .bind(first_node)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO idea (work_id, node_id, note, bible, origin_kind, origin_name) VALUES (?, ?, 'Möwen am Morgen', 'Ps 139,9', 'human', 'Autor')",
    )
    .bind(work)
    .bind(first_node)
    .execute(pool)
    .await
    .unwrap();
    exec(pool, "INSERT INTO idea (note, origin_kind, origin_name) VALUES ('ohne Werk', 'mcp', '@mcp:probe')").await;
    exec(pool, "INSERT INTO connection (kind, config) VALUES ('eliezer', '{\"url\":\"http://127.0.0.1:4321\"}')").await;
    exec(
        pool,
        "INSERT INTO preferred_source (title, url, note, position) VALUES ('Hafenkunde', 'https://hafen.example/', 'erfunden', 1)",
    )
    .await;
    exec(pool, "INSERT INTO mcp_token (name, hash, prefix) VALUES ('Laptop', 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa', 'bm_erf')").await;
    for name in ["kai.svg", "laterne.svg"] {
        let asset: i64 = sqlx::query_scalar("INSERT INTO asset (work_id, name) VALUES (?, ?) RETURNING id")
            .bind(work)
            .bind(name)
            .fetch_one(pool)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO asset_revision (asset_id, number, mime, encoding, content, bytes, sha256, caption, alt, author_name, status) \
             VALUES (?, 1, 'image/svg+xml', 'svg', '<svg xmlns=\"http://www.w3.org/2000/svg\"/>', 40, 'erfunden', 'Am Kai', 'Kai', 'Autor', 'current')",
        )
        .bind(asset)
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

/// All rows over the columns of `layout` (before), sorted.
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
async fn migration_0017_keeps_every_row_and_only_adds_the_book_table() {
    assert!(baruch_store::MIGRATOR.iter().any(|m| m.version == 17), "MIGRATOR enthält 0017");
    let dir = tempfile::tempdir().unwrap();
    let pool = raw_pool(dir.path()).await;
    migrate_before(&pool).await;
    let work = insert_rows(&pool).await;
    let layout = tables(&pool).await;
    let schema_before = schema(&pool).await;
    let before = contents(&pool, &layout).await;
    assert!(!layout.contains_key("work_book"), "vorher keine Tabelle work_book");
    for (table, rows) in [("persona", 2), ("post", 2), ("idea", 2), ("node", 2), ("asset", 2), ("mcp_token", 1)] {
        assert_eq!(before[table].len(), rows, "nachgebaute Datenbank hat {rows} Zeilen in {table}");
    }
    pool.close().await;

    let store = Store::open(dir.path()).await.expect("Store::open führt 0017 aus");
    let pool = store.pool();
    let after = without_instance_row(contents(pool, &layout).await);
    for (table, rows) in &before {
        if table == "_sqlx_migrations" {
            continue;
        }
        assert_eq!(after.get(table), Some(rows), "Tabelle {table}: alle Zeilen in den alten Spalten unverändert");
    }
    let schema_after = schema(pool).await;
    for (name, sql) in &schema_before {
        // `Store::open` runs all migrations; the later 0035 (user question 12) recreates `outline_request` with the
        // request kind `add` (checked in migration_0035.rs), 0040 `work` with the scheme `book` (checked in
        // migration_0040.rs).
        if name == "outline_request" || name == "work" {
            continue;
        }
        assert_eq!(
            schema_after.get(name).map(|s| s
                .as_deref()
                .map(|sql| without_r30(name, &without_r28_edited(name, &without_uuid_column(sql))))),
            Some(sql.clone()),
            "Schema von {name} unverändert (bis auf die Spalte uuid aus 0041)"
        );
    }
    let table = schema_after.get("work_book").cloned().flatten().expect("neue Tabelle work_book");
    assert!(
        table.contains("ON DELETE CASCADE") && table.contains("ON DELETE SET NULL"),
        "Fremdschlüssel nach Vertrag: {table}"
    );
    for trigger in ["work_book_same_work_insert", "work_book_same_work_update"] {
        assert!(schema_after.contains_key(trigger), "Trigger {trigger} angelegt");
    }
    let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM work_book").fetch_one(pool).await.unwrap();
    assert_eq!(rows, 0, "keine Zeile = Vorgaben; die Migration legt nichts an");
    let meta = store.book_meta(work).await.unwrap();
    assert_eq!((meta.cover, meta.paper_mm, meta.bleed_mm), (None, 0.1, 3.0), "Vorgaben ohne Zeile");
    assert_eq!(
        baruch_typst::xref::strip_key_labels(&store.export_typst(work).await.unwrap()).into_owned(),
        SOURCE,
        "Export byte-gleich wie vor der Migration"
    );
    let full_layout = tables(pool).await;
    let full_schema = schema_after.clone();
    let full = contents(pool, &full_layout).await;
    store.pool().close().await;
    drop(store);

    let again = Store::open(dir.path()).await.expect("zweites Öffnen");
    assert_eq!(schema(again.pool()).await, full_schema, "zweites Öffnen ändert das Schema nicht");
    assert_eq!(contents(again.pool(), &full_layout).await, full, "zweites Öffnen ändert keine Zeile");

    // New rows: foreign image rejected, deleting the image → NULL, deleting the work with a row works.
    let pool = again.pool();
    let other: i64 = sqlx::query_scalar(
        "INSERT INTO work (slug, kind, title, author, template, title_key) VALUES ('fremd', 'book', 'Fremd', '', '', 'fremd') RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    let kai: i64 = sqlx::query_scalar("SELECT id FROM asset WHERE name = 'kai.svg'").fetch_one(pool).await.unwrap();
    let foreign = sqlx::query("INSERT INTO work_book (work_id, cover_asset_id) VALUES (?, ?)")
        .bind(other)
        .bind(kai)
        .execute(pool)
        .await;
    assert!(foreign.is_err(), "Umschlag aus einem fremden Werk → Trigger bricht ab");
    again.set_work_cover(work, Some("kai.svg")).await.unwrap();
    again.delete_asset(work, "kai.svg", "Autor").await.unwrap();
    assert_eq!(again.work_cover(work).await.unwrap(), None, "Umschlagbild gelöscht → Umschlag NULL");
    again.set_work_cover(work, Some("laterne.svg")).await.unwrap();
    again.delete_work(work, "Die Laterne am Kai").await.expect("Werk mit work_book-Zeile lässt sich löschen");
    let left: i64 = sqlx::query_scalar("SELECT count(*) FROM work_book").fetch_one(pool).await.unwrap();
    assert_eq!(left, 0, "Zeile mit dem Werk gelöscht");
    let broken: i64 =
        sqlx::query_scalar("SELECT count(*) FROM pragma_foreign_key_check").fetch_one(pool).await.unwrap();
    assert_eq!(broken, 0, "keine Verstöße gegen Fremdschlüssel");
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

/// Migration 0041 (Round 26) appends the column `uuid` to tables with work content (`ALTER TABLE … ADD COLUMN` extends
/// the stored CREATE text by exactly this addition); the comparison of the old schema takes it out. Round 28 (0046)
/// appends `edited` to `post` and `chat_thread_post` (checked in migration_0046.rs); otherwise the schema stays
/// literal.
fn without_r28_edited(name: &str, sql: &str) -> String {
    if name == "post" || name == "chat_thread_post" { sql.replace(", edited TEXT", "") } else { sql.to_owned() }
}

fn without_uuid_column(sql: &str) -> String {
    sql.replace(R34_IDEA_COLUMNS, "").replace(UUID_COLUMN, "")
}

/// Round 34 (0052) appends title, parent node, order and main topic to `idea` (`ALTER TABLE … ADD COLUMN` extends the
/// stored CREATE text by exactly this addition, directly after the column uuid; checked in migration_0052.rs); the
/// comparison of the old schema takes it out.
const R34_IDEA_COLUMNS: &str = ", title TEXT NOT NULL DEFAULT '' CHECK (length(title) <= 200), parent_id INTEGER REFERENCES idea(id) ON DELETE SET NULL, position INTEGER NOT NULL DEFAULT 0, main_topic INTEGER NOT NULL DEFAULT 0 CHECK (main_topic IN (0, 1))";

const UUID_COLUMN: &str = ", uuid TEXT CHECK (uuid IS NULL OR (length(uuid) = 36 AND uuid = lower(uuid)))";

/// Round 30 (0047) appends columns to `post`, `chat_thread_post`, `finding` and `revision` (`ALTER TABLE … ADD COLUMN`
/// extends the stored CREATE text by exactly these additions; checked in migration_0047.rs); the comparison of the old
/// schema takes them out.
fn without_r30(name: &str, sql: &str) -> String {
    let sql: &str = &without_r22(name, sql);
    const POST: &str = ", basis_rev INTEGER CHECK (basis_rev IS NULL OR basis_rev >= 1), basis_label TEXT NOT NULL \
        DEFAULT '' CHECK (length(basis_label) <= 300), reply_to INTEGER REFERENCES post(id) ON DELETE SET NULL, \
        repeat_of INTEGER REFERENCES post(id) ON DELETE SET NULL, relay_from TEXT CHECK (relay_from IS NULL OR \
        length(relay_from) BETWEEN 1 AND 200), request TEXT CHECK (request IS NULL OR json_valid(request))";
    const FINDING: &str =
        ", solutions TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(solutions) AND json_type(solutions) = 'array')";
    const REVISION: &str = ", variant_group TEXT CHECK (variant_group IS NULL OR (length(variant_group) = 36 AND \
        variant_group = lower(variant_group))), variant_label TEXT CHECK (variant_label IS NULL OR variant_label IN \
        ('A','B','C')), variant_summary TEXT NOT NULL DEFAULT '' CHECK (length(variant_summary) <= 300), \
        variant_findings TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(variant_findings) AND \
        json_type(variant_findings) = 'array')";
    let added = match name {
        "post" => POST.to_owned(),
        "chat_thread_post" => POST.replace("post(id)", "chat_thread_post(id)"),
        "finding" => FINDING.to_owned(),
        "revision" => REVISION.to_owned(),
        _ => return sql.to_owned(),
    };
    sql.replace(&added, "")
}

/// Objects that 0047 (Round 30) newly creates.
#[allow(dead_code)]
const R30_OBJECTS: [&str; 3] = ["replace_batch", "replace_batch_work", "revision_variant_group"];

/// Round 22 (0050) appends `amend_seq`/`based_on_amend` to `revision` and `adopted_amend` to `finding` and
/// `finding_status` (`ALTER TABLE … ADD COLUMN`, checked in migration_0050.rs); the comparison of the old schema takes
/// them out.
fn without_r22(name: &str, sql: &str) -> String {
    const REVISION: &str = ", amend_seq INTEGER NOT NULL DEFAULT 0 CHECK (amend_seq >= 0), based_on_amend INTEGER \
        CHECK (based_on_amend IS NULL OR based_on_amend >= 0)";
    const FINDING: &str = ", adopted_amend INTEGER CHECK (adopted_amend IS NULL OR adopted_amend >= 0)";
    match name {
        "revision" => sql.replace(REVISION, ""),
        "finding" | "finding_status" => sql.replace(FINDING, ""),
        _ => sql.to_owned(),
    }
}
