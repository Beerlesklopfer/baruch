// SPDX-License-Identifier: AGPL-3.0-or-later
//! Migration 0043 (thought map, Round 25) on a reproduced database in the state of development before Round 25 (store
//! up to 0042, library complete) with freely invented rows in all ten tables with uuid and tombstones of all ten kinds.
//!
//! Evidenced: every row of every table and every tombstone stays the same; `uuid_tombstone` differs in the schema only
//! by the two new kinds in the CHECK; the ten tombstone triggers from 0041 are there again literally and keep writing
//! tombstones with the work uuid; new are exactly the objects from 0043; the kinds `idea_link` and `idea_map_pos` are
//! allowed, an invented kind is not; foreign keys stay without violation; 0043 is not in `FOREIGN_KEYS_OFF`;
//! `Store::open` afterwards succeeds.

use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::str::FromStr;

use baruch_store::{Store, title_key};
use sqlx::migrate::Migrator;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};

const BOOK: &str = "#set document(title: \"Das Teichbuch\", author: \"Olga Ufer\")\n\n\
= Seerosen\n\nBlatt an Blatt.\n\n= Frösche\n\nAm Abend quakt es.\n";

/// The ten tombstone kinds from 0041 (value in `uuid_tombstone.kind`) and the table they come from.
const KINDS: [(&str, &str); 10] = [
    ("work", "work"),
    ("node", "node"),
    ("revision", "revision"),
    ("template_revision", "work_template_revision"),
    ("post", "post"),
    ("idea", "idea"),
    ("thread", "chat_thread"),
    ("thread_post", "chat_thread_post"),
    ("asset", "asset"),
    ("asset_revision", "asset_revision"),
];

async fn raw_pool(dir: &Path) -> SqlitePool {
    let options = SqliteConnectOptions::from_str("sqlite:")
        .unwrap()
        .filename(dir.join("baruch.sqlite"))
        .create_if_missing(true)
        .foreign_keys(true);
    SqlitePoolOptions::new().max_connections(1).connect_with(options).await.unwrap()
}

fn store_migrator(keep: impl Fn(i64) -> bool) -> Migrator {
    let list: Vec<_> = baruch_store::MIGRATOR.iter().filter(|m| keep(m.version)).cloned().collect();
    Migrator { migrations: Cow::Owned(list), ignore_missing: true, ..Migrator::DEFAULT }
}

/// Store up to 0042 (0040 as in `baruch_store::migrate` with foreign keys switched off) and the library: the state of
/// development before Round 25.
async fn migrate_before(pool: &SqlitePool) {
    assert!(baruch_store::MIGRATOR.iter().any(|m| m.version == 43), "die neue Migration ist 0043");
    assert!(
        !baruch_store::FOREIGN_KEYS_OFF.contains(&43),
        "0043 baut keine Tabelle um, auf die ein Fremdschlüssel zeigt (Vertrag §1)"
    );
    store_migrator(|v| v < 40).run(pool).await.unwrap();
    {
        let mut conn = pool.acquire().await.unwrap();
        sqlx::query("PRAGMA foreign_keys = OFF").execute(&mut *conn).await.unwrap();
        store_migrator(|v| v == 40).run(&mut *conn).await.unwrap();
        sqlx::query("PRAGMA foreign_keys = ON").execute(&mut *conn).await.unwrap();
    }
    store_migrator(|v| v == 41 || v == 42).run(pool).await.unwrap();
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../baruch-library/migrations");
    let mut library = Migrator::new(dir).await.unwrap();
    library.ignore_missing = true;
    library.run(pool).await.unwrap();
    let highest: i64 = scalar(pool, "SELECT max(version) FROM _sqlx_migrations WHERE version < 100").await;
    assert_eq!(highest, 42, "Stand davor: Store bis 0042");
}

async fn exec(pool: &SqlitePool, sql: &str) {
    sqlx::query(sqlx::AssertSqlSafe(sql)).execute(pool).await.unwrap_or_else(|e| panic!("{sql}: {e}"));
}

async fn scalar(pool: &SqlitePool, sql: &str) -> i64 {
    sqlx::query_scalar(sqlx::AssertSqlSafe(sql)).fetch_one(pool).await.unwrap_or_else(|e| panic!("{sql}: {e}"))
}

/// Work with two chapters (current revision), template revision, post, thread, image and ideas directly in SQLite; plus
/// per kind one tombstone from an earlier deletion. Returns the work id.
async fn insert_rows(pool: &SqlitePool) -> i64 {
    exec(pool, "INSERT INTO store_meta (key, value) VALUES ('seeded', '1') ON CONFLICT DO NOTHING").await;
    let doc = baruch_typst::parse(BOOK).unwrap();
    let work: i64 = sqlx::query_scalar(
        "INSERT INTO work (slug, kind, title, author, template, source_name, source_sha256, title_key) \
         VALUES ('das-teichbuch', 'book', 'Das Teichbuch', 'Olga Ufer', ?, 'teich.typ', 'sha-teich', ?) RETURNING id",
    )
    .bind(&doc.template)
    .bind(title_key("Das Teichbuch"))
    .fetch_one(pool)
    .await
    .unwrap();
    exec(
        pool,
        &format!(
            "INSERT INTO work_template_revision (work_id, number, body, author_name, note) VALUES ({work}, 1, '// Vorlage\n', 'Olga', 'Anfang')"
        ),
    )
    .await;
    let mut nodes = Vec::new();
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
             VALUES (?, 1, ?, 'human', 'Olga', NULL, 'current', 'Import')",
        )
        .bind(id)
        .bind(&heading.body)
        .execute(pool)
        .await
        .unwrap();
        nodes.push(id);
    }
    let post: i64 = sqlx::query_scalar(
        "INSERT INTO post (node_id, position, author, body, sources) VALUES (?, 0, '@lektor', 'Wie tief?', '[]') RETURNING id",
    )
    .bind(nodes[0])
    .fetch_one(pool)
    .await
    .unwrap();
    let thread: i64 = sqlx::query_scalar(
        "INSERT INTO chat_thread (work_id, created_by, title) VALUES (?, 'Olga', 'Libellen') RETURNING id",
    )
    .bind(work)
    .fetch_one(pool)
    .await
    .unwrap();
    let asset: i64 = sqlx::query_scalar("INSERT INTO asset (work_id, name) VALUES (?, 'teich.svg') RETURNING id")
        .bind(work)
        .fetch_one(pool)
        .await
        .unwrap();
    let svg = "<svg xmlns=\"http://www.w3.org/2000/svg\"><circle r=\"1\"/></svg>";
    for sql in [
        format!("INSERT INTO post_ref (post_id, position, node_seq, rev, quote) VALUES ({post}, 0, 0, 1, 'Blatt')"),
        format!("INSERT INTO chat_thread_post (thread_id, position, author, body) VALUES ({thread}, 0, 'Olga', 'Im Juli.')"),
        format!(
            "INSERT INTO asset_revision (asset_id, number, mime, encoding, content, bytes, sha256, caption, alt, author_name, status) \
             VALUES ({asset}, 1, 'image/svg+xml', 'svg', '{svg}', {}, 'abc', 'Teich', 'Teich', 'Olga', 'current')",
            svg.len()
        ),
        format!(
            "INSERT INTO idea (work_id, node_id, note, origin_kind, origin_name) VALUES ({work}, {}, 'Libelle als Bild', 'human', 'Olga')",
            nodes[1]
        ),
        format!(
            "INSERT INTO idea (work_id, node_id, note, origin_kind, origin_name) VALUES ({work}, NULL, 'Schilf rauscht', 'persona', '@lektor')"
        ),
        "INSERT INTO idea (work_id, node_id, note, origin_kind, origin_name, deleted) VALUES (NULL, NULL, 'ohne Werk', 'mcp', '@mcp:probe', '2026-11-01T10:00:00.000Z')".to_owned(),
    ] {
        exec(pool, &sql).await;
    }
    // Tombstones of all ten kinds (as after earlier deletions; invented uuids).
    for (n, (kind, _)) in KINDS.iter().enumerate() {
        exec(
            pool,
            &format!(
                "INSERT INTO uuid_tombstone (uuid, kind, work_uuid, deleted) VALUES \
                 ('00000000-0000-4000-8000-0000000000{n:02}', '{kind}', (SELECT uuid FROM work WHERE id = {work}), '2026-11-0{}T08:00:00.000Z')",
                n % 9 + 1
            ),
        )
        .await;
    }
    for (_, table) in KINDS {
        assert!(scalar(pool, &format!("SELECT count(*) FROM {table}")).await > 0, "{table} hat Zeilen vor 0043");
    }
    work
}

async fn tables(pool: &SqlitePool) -> BTreeMap<String, Vec<String>> {
    let names: Vec<String> = sqlx::query_scalar(
        "SELECT name FROM sqlite_master WHERE type = 'table' AND sql NOT LIKE 'CREATE VIRTUAL%' ORDER BY name",
    )
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

/// All schema objects: name → (type, table, SQL).
async fn schema(pool: &SqlitePool) -> BTreeMap<String, (String, String, Option<String>)> {
    let rows =
        sqlx::query("SELECT name, type, tbl_name, sql FROM sqlite_master ORDER BY name").fetch_all(pool).await.unwrap();
    rows.into_iter().map(|r| (r.get(0), (r.get(1), r.get(2), r.get(3)))).collect()
}

/// Content of every table in the columns from `layout`.
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

/// Objects that 0043 newly creates.
fn new_objects() -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    for table in ["idea_link", "idea_map_pos"] {
        for suffix in ["", "_uuid", "_uuid_fill", "_uuid_tombstone"] {
            names.insert(format!("{table}{suffix}"));
        }
        names.insert(format!("sqlite_autoindex_{table}_1"));
    }
    names.insert("idea_link_to".into());
    names.insert("work_map_pos_cascade_uuid_tombstone".into());
    names.insert("idea_link_cascade_uuid_tombstone".into());
    names
}

#[tokio::test]
async fn migration_0043_keeps_every_row_and_tombstone_and_widens_the_tombstone_kinds() {
    let dir = tempfile::tempdir().unwrap();
    let pool = raw_pool(dir.path()).await;
    migrate_before(&pool).await;
    let work = insert_rows(&pool).await;
    let layout = tables(&pool).await;
    let schema_before = schema(&pool).await;
    let before = contents(&pool, &layout).await;
    assert_eq!(
        scalar(&pool, "SELECT count(DISTINCT kind) FROM uuid_tombstone").await,
        10,
        "Grabsteine aller zehn Arten"
    );
    let refused = sqlx::query("INSERT INTO uuid_tombstone (uuid, kind) VALUES ('x', 'idea_link')").execute(&pool).await;
    assert!(refused.is_err(), "vor 0043 kennt der CHECK die neue Art nicht (sonst prüfte dieser Test nichts)");

    // Only 0043, as `baruch_store::migrate` runs it (not in FOREIGN_KEYS_OFF: foreign keys stay on).
    store_migrator(|v| v == 43).run(&pool).await.expect("0043 läuft auf dem Stand von development");

    // Data: every row of every old table unchanged, also all tombstones.
    let after = contents(&pool, &layout).await;
    for (table, rows) in &before {
        if table != "_sqlx_migrations" {
            assert_eq!(after.get(table), Some(rows), "Tabelle {table}: alle Zeilen unverändert");
        }
    }
    let layout_after = tables(&pool).await;
    for (table, cols) in &layout {
        assert_eq!(layout_after.get(table), Some(cols), "{table}: Spalten unverändert");
    }

    // Schema: old objects literal (uuid_tombstone only with the two new kinds), new exactly those from 0043.
    let schema_after = schema(&pool).await;
    let added: BTreeSet<String> = schema_after.keys().filter(|k| !schema_before.contains_key(*k)).cloned().collect();
    assert_eq!(added, new_objects(), "neu sind genau Tabellen, Indizes und Trigger aus 0043");
    for (name, (kind, table, sql)) in &schema_before {
        let (kind_now, table_now, sql_now) = schema_after.get(name).unwrap_or_else(|| panic!("{name} verschwunden"));
        assert_eq!((kind_now, table_now), (kind, table), "{name}: Typ und Tabelle unverändert");
        let sql_now = if name == "uuid_tombstone" {
            let now = sql_now.clone().unwrap();
            assert!(now.contains("'asset_revision','idea_link','idea_map_pos'"), "neue Arten im CHECK: {now}");
            Some(now.replace(",'idea_link','idea_map_pos'", ""))
        } else {
            sql_now.clone()
        };
        // The ten tombstone triggers too: after the recreation literally as from 0041 (no RENAME with quotation marks).
        assert_eq!(&sql_now, sql, "Schema von {name} unverändert");
    }
    for (kind, table) in KINDS {
        let trigger = if kind == "template_revision" {
            "work_template_revision_uuid_tombstone".to_owned()
        } else {
            format!("{table}_uuid_tombstone")
        };
        assert!(schema_after.contains_key(&trigger), "Grabstein-Trigger {trigger} steht wieder da");
    }

    // New kinds allowed, an invented one still not.
    for kind in ["idea_link", "idea_map_pos"] {
        exec(&pool, &format!("INSERT INTO uuid_tombstone (uuid, kind) VALUES ('probe-{kind}', '{kind}')")).await;
    }
    let invented =
        sqlx::query("INSERT INTO uuid_tombstone (uuid, kind) VALUES ('probe-x', 'gedanke')").execute(&pool).await;
    assert!(invented.is_err(), "CHECK lehnt eine erfundene Art weiter ab");
    exec(&pool, "DELETE FROM uuid_tombstone WHERE uuid LIKE 'probe-%'").await;

    assert_eq!(scalar(&pool, "SELECT count(*) FROM pragma_foreign_key_check").await, 0, "foreign_key_check leer");
    let integrity: String = sqlx::query_scalar("PRAGMA integrity_check").fetch_one(&pool).await.unwrap();
    assert_eq!(integrity, "ok", "integrity_check");
    pool.close().await;

    // Store::open afterwards (runs only later migrations any more) and the old triggers keep writing.
    let store = Store::open(dir.path()).await.expect("Store::open nach 0043");
    let pool = store.pool();
    let work_uuid: String =
        sqlx::query_scalar("SELECT uuid FROM work WHERE id = ?").bind(work).fetch_one(pool).await.unwrap();
    let old_graves = scalar(pool, "SELECT count(*) FROM uuid_tombstone").await;
    exec(pool, "DELETE FROM idea WHERE note = 'Schilf rauscht'").await; // kind idea (hard, only in the test)
    store.delete_work(work, "Das Teichbuch").await.expect("Werk löschen schreibt Grabsteine");
    let fresh: Vec<(String, Option<String>)> = sqlx::query_as(
        "SELECT kind, work_uuid FROM uuid_tombstone WHERE uuid NOT LIKE '00000000-0000-4000-8000-%' ORDER BY kind",
    )
    .fetch_all(pool)
    .await
    .unwrap();
    let kinds: BTreeSet<&str> = fresh.iter().map(|(k, _)| k.as_str()).collect();
    for (kind, _) in KINDS {
        assert!(kinds.contains(kind), "nach 0043 schreibt der Trigger der Art {kind} weiter: {fresh:?}");
    }
    for (kind, grave) in &fresh {
        assert_eq!(grave.as_deref(), Some(work_uuid.as_str()), "Grabstein {kind} trägt die Werk-uuid");
    }
    assert!(scalar(pool, "SELECT count(*) FROM uuid_tombstone").await > old_graves, "neue Grabsteine dazu");
}

#[tokio::test]
async fn the_new_tables_fill_uuids_and_leave_tombstones_of_the_new_kinds() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).await.unwrap();
    let pool = store.pool();
    let work = store.default_work().await.unwrap();
    for note in ["eins", "zwei"] {
        exec(
            pool,
            &format!(
                "INSERT INTO idea (work_id, note, origin_kind, origin_name) VALUES ({work}, '{note}', 'human', 'Autor')"
            ),
        )
        .await;
    }
    let ids: Vec<i64> = sqlx::query_scalar("SELECT id FROM idea WHERE note IN ('eins', 'zwei') ORDER BY id")
        .fetch_all(pool)
        .await
        .unwrap();
    let (a, b) = (ids[0], ids[1]);
    exec(pool, &format!("INSERT INTO idea_link (from_id, to_id, kind, origin_kind, origin_name) VALUES ({a}, {b}, 'leads_to', 'human', 'Autor')")).await;
    exec(pool, &format!("INSERT INTO idea_map_pos (work_id, idea_id, x, y) VALUES ({work}, {a}, 1.5, -2)")).await;
    for table in ["idea_link", "idea_map_pos"] {
        let uuid: String =
            sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT uuid FROM {table}"))).fetch_one(pool).await.unwrap();
        assert!(baruch_exchange::is_uuid(&uuid) && &uuid[14..15] == "4", "{table}: Fülltrigger vergibt v4: {uuid}");
    }
    // CHECKs of the new tables: self-reference, „widerspricht“ only normalised, coordinates limited.
    let self_link = format!(
        "INSERT INTO idea_link (from_id, to_id, kind, origin_kind, origin_name) VALUES ({a}, {a}, 'leads_to', 'human', 'X')"
    );
    assert!(
        sqlx::query(sqlx::AssertSqlSafe(self_link.as_str())).execute(pool).await.is_err(),
        "CHECK from_id <> to_id"
    );
    let reversed = format!(
        "INSERT INTO idea_link (from_id, to_id, kind, origin_kind, origin_name) VALUES ({b}, {a}, 'contradicts', 'human', 'X')"
    );
    assert!(
        sqlx::query(sqlx::AssertSqlSafe(reversed.as_str())).execute(pool).await.is_err(),
        "CHECK contradicts nur mit from_id < to_id"
    );
    let far = format!("INSERT INTO idea_map_pos (work_id, idea_id, x, y) VALUES ({work}, {b}, 1000001, 0)");
    assert!(
        sqlx::query(sqlx::AssertSqlSafe(far.as_str())).execute(pool).await.is_err(),
        "CHECK x BETWEEN -1e6 AND 1e6"
    );

    let work_uuid: String =
        sqlx::query_scalar("SELECT uuid FROM work WHERE id = ?").bind(work).fetch_one(pool).await.unwrap();
    exec(pool, "DELETE FROM idea_link").await;
    exec(pool, "DELETE FROM idea_map_pos").await;
    let graves: Vec<(String, Option<String>)> =
        sqlx::query_as("SELECT kind, work_uuid FROM uuid_tombstone ORDER BY kind").fetch_all(pool).await.unwrap();
    assert_eq!(
        graves,
        vec![("idea_link".to_owned(), Some(work_uuid.clone())), ("idea_map_pos".to_owned(), Some(work_uuid))],
        "Grabstein der Verbindung mit dem Werk des Einfalls from_id, der Lage mit ihrem Werk"
    );
}
