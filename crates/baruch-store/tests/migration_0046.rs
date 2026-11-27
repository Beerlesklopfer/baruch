// SPDX-License-Identifier: AGPL-3.0-or-later
//! Migration 0046 (findings, finding history, old post versions; Round 28) on a reproduced database in the state of
//! development before Round 28 (store up to 0045, library complete) with freely invented rows: work with chapters,
//! posts including reference, thread with posts, image, ideas including link and position, cover block and tombstones
//! of all thirteen kinds.
//!
//! Evidenced: 0046 is not in `FOREIGN_KEYS_OFF`, and no foreign key points to the rebuilt table `uuid_tombstone`
//! (therefore the implicit DELETE of DROP TABLE triggers no action); every row of every old table stays byte-identical
//! including ids, also all tombstones; `post` and `chat_thread_post` have `edited` as the last column, NULL everywhere;
//! in the schema only the CHECK of `uuid_tombstone` and the columns of `post` and `chat_thread_post` change, the
//! thirteen tombstone triggers are there again literally; new are exactly the objects of `finding`, `finding_status`
//! and `post_version`; the CHECKs reject what the contract forbids; fill triggers and tombstones (with work uuid) fire
//! on single deletion, on deleting a post or thread post including old version and on deleting the work.

use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::str::FromStr;

use baruch_store::{Store, title_key};
use sqlx::migrate::Migrator;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};

const BOOK: &str = "#set document(title: \"Die Laterne am Kanal\", author: \"Mira Docht\")\n\n\
= Abend\n\nDas Wasser trägt das letzte Licht.\n\n= Nacht\n\nDie Laterne brennt bis zum Morgen.\n";

/// The thirteen tombstone kinds before 0046 (value in `uuid_tombstone.kind`) and the name of their tombstone trigger.
const KINDS: [(&str, &str); 13] = [
    ("work", "work_uuid_tombstone"),
    ("node", "node_uuid_tombstone"),
    ("revision", "revision_uuid_tombstone"),
    ("template_revision", "work_template_revision_uuid_tombstone"),
    ("post", "post_uuid_tombstone"),
    ("idea", "idea_uuid_tombstone"),
    ("thread", "chat_thread_uuid_tombstone"),
    ("thread_post", "chat_thread_post_uuid_tombstone"),
    ("asset", "asset_uuid_tombstone"),
    ("asset_revision", "asset_revision_uuid_tombstone"),
    ("idea_link", "idea_link_uuid_tombstone"),
    ("idea_map_pos", "idea_map_pos_uuid_tombstone"),
    ("cover_block", "cover_block_uuid_tombstone"),
];

/// Additions by which the schema of the old tables may differ after 0046.
const TOMBSTONE_KINDS: &str = ",\n                                          'finding','finding_status','post_version'";
const EDITED_COLUMN: &str = ", edited TEXT";

/// Objects that 0046 newly creates.
const NEW_OBJECTS: [&str; 22] = [
    "finding",
    "finding_uuid",
    "finding_uuid_fill",
    "finding_uuid_tombstone",
    "finding_post_position",
    "finding_thread_post_position",
    "finding_work_node_status",
    "finding_status",
    "finding_status_uuid",
    "finding_status_uuid_fill",
    "finding_status_uuid_tombstone",
    "finding_status_finding",
    "post_version",
    "post_version_uuid",
    "post_version_uuid_fill",
    "post_version_uuid_tombstone",
    "post_version_post",
    "post_version_thread_post",
    "work_finding_cascade_uuid_tombstone",
    "finding_status_cascade_uuid_tombstone",
    "post_version_cascade_uuid_tombstone",
    "thread_post_version_cascade_uuid_tombstone",
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

async fn exec(pool: &SqlitePool, sql: &str) {
    sqlx::query(sqlx::AssertSqlSafe(sql)).execute(pool).await.unwrap_or_else(|e| panic!("{sql}: {e}"));
}

async fn scalar(pool: &SqlitePool, sql: &str) -> i64 {
    sqlx::query_scalar(sqlx::AssertSqlSafe(sql)).fetch_one(pool).await.unwrap_or_else(|e| panic!("{sql}: {e}"))
}

async fn text(pool: &SqlitePool, sql: &str) -> String {
    sqlx::query_scalar(sqlx::AssertSqlSafe(sql)).fetch_one(pool).await.unwrap_or_else(|e| panic!("{sql}: {e}"))
}

/// Store up to 0045 (0040 as in `baruch_store::migrate` with foreign keys switched off) and the library: the state of
/// development before Round 28.
async fn migrate_before(pool: &SqlitePool) {
    assert!(baruch_store::MIGRATOR.iter().any(|m| m.version == 46), "die neue Migration ist 0046");
    assert!(
        !baruch_store::FOREIGN_KEYS_OFF.contains(&46),
        "0046 baut keine Tabelle um, auf die ein Fremdschlüssel zeigt (Vertrag R28 §2)"
    );
    store_migrator(|v| v < 40).run(pool).await.unwrap();
    {
        let mut conn = pool.acquire().await.unwrap();
        sqlx::query("PRAGMA foreign_keys = OFF").execute(&mut *conn).await.unwrap();
        store_migrator(|v| v == 40).run(&mut *conn).await.unwrap();
        sqlx::query("PRAGMA foreign_keys = ON").execute(&mut *conn).await.unwrap();
    }
    store_migrator(|v| (41..=45).contains(&v)).run(pool).await.unwrap();
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../baruch-library/migrations");
    let mut library = Migrator::new(dir).await.unwrap();
    library.ignore_missing = true;
    library.run(pool).await.unwrap();
    let highest = scalar(pool, "SELECT max(version) FROM _sqlx_migrations WHERE version < 100").await;
    assert_eq!(highest, 45, "Stand davor: Store bis 0045");
}

/// Ids of the test data.
struct Rows {
    work: i64,
    nodes: Vec<i64>,
    posts: Vec<i64>,
    thread_posts: Vec<i64>,
}

/// Work with contents in all tables with uuid and tombstones of all thirteen kinds directly in SQLite.
async fn insert_rows(pool: &SqlitePool) -> Rows {
    exec(pool, "INSERT INTO store_meta (key, value) VALUES ('seeded', '1') ON CONFLICT DO NOTHING").await;
    let doc = baruch_typst::parse(BOOK).unwrap();
    let work: i64 = sqlx::query_scalar(
        "INSERT INTO work (slug, kind, title, author, template, source_name, source_sha256, title_key) \
         VALUES ('die-laterne-am-kanal', 'book', 'Die Laterne am Kanal', 'Mira Docht', ?, 'laterne.typ', 'sha-laterne', ?) \
         RETURNING id",
    )
    .bind(&doc.template)
    .bind(title_key("Die Laterne am Kanal"))
    .fetch_one(pool)
    .await
    .unwrap();
    exec(
        pool,
        &format!(
            "INSERT INTO work_template_revision (work_id, number, body, author_name, note) \
             VALUES ({work}, 1, '// Vorlage\n', 'Mira', 'Anfang')"
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
             VALUES (?, 1, ?, 'human', 'Mira', NULL, 'current', 'Import')",
        )
        .bind(id)
        .bind(&heading.body)
        .execute(pool)
        .await
        .unwrap();
        nodes.push(id);
    }
    let mut posts = Vec::new();
    for (position, (author, body)) in [
        ("Autor", "Wie hell ist die Laterne?"),
        ("@lektor", "Was trägt: das Bild.\nKritisch: „letzte Licht“ ist matt."),
    ]
    .iter()
    .enumerate()
    {
        let id: i64 = sqlx::query_scalar(
            "INSERT INTO post (node_id, position, author, body, sources) VALUES (?, ?, ?, ?, '[]') RETURNING id",
        )
        .bind(nodes[0])
        .bind(position as i64)
        .bind(author)
        .bind(body)
        .fetch_one(pool)
        .await
        .unwrap();
        posts.push(id);
    }
    let thread: i64 = sqlx::query_scalar(
        "INSERT INTO chat_thread (work_id, created_by, title) VALUES (?, 'Autor', 'Brücken') RETURNING id",
    )
    .bind(work)
    .fetch_one(pool)
    .await
    .unwrap();
    let mut thread_posts = Vec::new();
    for (position, (author, body)) in
        [("Autor", "Welche Brücke zuerst?"), ("@lektor", "Hinweis: die steinerne.")].iter().enumerate()
    {
        let id: i64 = sqlx::query_scalar(
            "INSERT INTO chat_thread_post (thread_id, position, author, body) VALUES (?, ?, ?, ?) RETURNING id",
        )
        .bind(thread)
        .bind(position as i64)
        .bind(author)
        .bind(body)
        .fetch_one(pool)
        .await
        .unwrap();
        thread_posts.push(id);
    }
    let asset: i64 = sqlx::query_scalar("INSERT INTO asset (work_id, name) VALUES (?, 'laterne.svg') RETURNING id")
        .bind(work)
        .fetch_one(pool)
        .await
        .unwrap();
    let svg = "<svg xmlns=\"http://www.w3.org/2000/svg\"><rect width=\"2\" height=\"5\"/></svg>";
    for sql in [
        format!(
            "INSERT INTO post_ref (post_id, position, node_seq, rev, quote) VALUES ({}, 0, 0, 1, 'Licht')",
            posts[1]
        ),
        format!(
            "INSERT INTO asset_revision (asset_id, number, mime, encoding, content, bytes, sha256, caption, alt, \
             author_name, status) \
             VALUES ({asset}, 1, 'image/svg+xml', 'svg', '{svg}', {}, 'abc', 'Laterne', 'Laterne', 'Mira', 'current')",
            svg.len()
        ),
        format!(
            "INSERT INTO idea (work_id, node_id, note, origin_kind, origin_name) \
             VALUES ({work}, {}, 'Spiegelung im Wasser', 'human', 'Mira')",
            nodes[1]
        ),
        format!(
            "INSERT INTO idea (work_id, node_id, note, origin_kind, origin_name) \
             VALUES ({work}, NULL, 'Geruch von Teer', 'persona', '@lektor')"
        ),
        format!(
            "INSERT INTO cover_block (work_id, kind, x_mm, y_mm, width_mm, size_pt) \
             VALUES ({work}, 'front_title', 10, 20, 100, 24)"
        ),
    ] {
        exec(pool, &sql).await;
    }
    let ideas: Vec<i64> = sqlx::query_scalar("SELECT id FROM idea WHERE work_id = ? ORDER BY id")
        .bind(work)
        .fetch_all(pool)
        .await
        .unwrap();
    exec(
        pool,
        &format!(
            "INSERT INTO idea_link (from_id, to_id, kind, origin_kind, origin_name) \
             VALUES ({}, {}, 'leads_to', 'human', 'Mira')",
            ideas[0], ideas[1]
        ),
    )
    .await;
    exec(pool, &format!("INSERT INTO idea_map_pos (work_id, idea_id, x, y) VALUES ({work}, {}, 4, 2.5)", ideas[1]))
        .await;
    // Tombstones of all thirteen kinds (as after earlier deletions; invented uuids).
    for (n, (kind, _)) in KINDS.iter().enumerate() {
        exec(
            pool,
            &format!(
                "INSERT INTO uuid_tombstone (uuid, kind, work_uuid, deleted) VALUES \
                 ('00000000-0000-4000-8000-0000000000{n:02}', '{kind}', (SELECT uuid FROM work WHERE id = {work}), \
                 '2026-11-0{}T08:00:00.000Z')",
                n % 9 + 1
            ),
        )
        .await;
    }
    Rows { work, nodes, posts, thread_posts }
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

/// Content of every table in the columns from `layout`, values through `quote()` (type and bytes visible).
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

const FINDING_COLUMNS: &str = "work_id, node_id, post_id, thread_post_id, position, persona, kind, body, status, \
     reason, adopted_revision, deferred_node, deferred_idea, summary_of";

/// A valid, open finding on the lector post at position `position` as SQL values in [`FINDING_COLUMNS`].
fn finding_values(rows: &Rows, position: u32) -> String {
    format!(
        "{}, {}, {}, NULL, {position}, '@lektor', 'critical', '„letzte Licht“ ist matt.', 'open', '', NULL, NULL, \
         NULL, '[]'",
        rows.work, rows.nodes[0], rows.posts[1]
    )
}

const STATUS_COLUMNS: &str = "finding_id, status, reason, adopted_revision, deferred_node, deferred_idea, actor_kind, \
     actor, proposed, decision, decided";

#[tokio::test]
async fn migration_0046_keeps_every_row_and_adds_findings_and_post_versions() {
    let dir = tempfile::tempdir().unwrap();
    let pool = raw_pool(dir.path()).await;
    migrate_before(&pool).await;
    let rows = insert_rows(&pool).await;
    let layout = tables(&pool).await;
    let schema_before = schema(&pool).await;
    let before = contents(&pool, &layout).await;
    assert_eq!(scalar(&pool, "SELECT count(DISTINCT kind) FROM uuid_tombstone").await, 13, "alle dreizehn Arten");
    let refused = sqlx::query("INSERT INTO uuid_tombstone (uuid, kind) VALUES ('x', 'finding')").execute(&pool).await;
    assert!(refused.is_err(), "vor 0046 kennt der CHECK die neuen Arten nicht (sonst prüfte dieser Test nichts)");
    // No foreign key points to the rebuilt table: reason why 0046 gets by without FOREIGN_KEYS_OFF.
    let pointing = scalar(
        &pool,
        "SELECT count(*) FROM sqlite_master m, pragma_foreign_key_list(m.name) f \
         WHERE m.type = 'table' AND f.\"table\" = 'uuid_tombstone'",
    )
    .await;
    assert_eq!(pointing, 0, "kein Fremdschlüssel auf uuid_tombstone");

    // Only 0046, as `baruch_store::migrate` runs it (not in FOREIGN_KEYS_OFF: foreign keys stay on).
    store_migrator(|v| v == 46).run(&pool).await.expect("0046 läuft auf dem Stand von development");

    // Data: every row of every old table byte-identical in the old columns (ids, tombstones, posts).
    let after = contents(&pool, &layout).await;
    for (table, old) in &before {
        if table != "_sqlx_migrations" {
            assert_eq!(after.get(table), Some(old), "Tabelle {table}: alle Zeilen unverändert");
        }
    }
    let layout_after = tables(&pool).await;
    for (table, cols) in &layout {
        let mut now = layout_after[table].clone();
        if table == "post" || table == "chat_thread_post" {
            let added = now.split_off(cols.len());
            assert_eq!(added, ["edited"], "{table}: edited als letzte Spalte");
        }
        assert_eq!(&now, cols, "{table}: alte Spalten in alter Reihenfolge");
    }
    assert_eq!(
        scalar(&pool, "SELECT count(*) FROM post WHERE edited IS NOT NULL").await
            + scalar(&pool, "SELECT count(*) FROM chat_thread_post WHERE edited IS NOT NULL").await,
        0,
        "vorhandene Beiträge gelten als nie bearbeitet (edited NULL)"
    );

    // Schema: old objects literal except for the allowed additions, new exactly the objects of the three tables.
    let schema_after = schema(&pool).await;
    let added: BTreeSet<String> = schema_after.keys().filter(|k| !schema_before.contains_key(*k)).cloned().collect();
    let expected: BTreeSet<String> = NEW_OBJECTS.iter().map(|s| s.to_string()).collect();
    assert_eq!(
        added, expected,
        "neu sind genau Tabellen, Indizes und Trigger von finding, finding_status, post_version"
    );
    for (name, (kind, table, sql)) in &schema_before {
        let (kind_now, table_now, sql_now) = schema_after.get(name).unwrap_or_else(|| panic!("{name} verschwunden"));
        assert_eq!((kind_now, table_now), (kind, table), "{name}: Typ und Tabelle unverändert");
        let now = sql_now.clone().unwrap_or_default();
        let expected_addition = match name.as_str() {
            "uuid_tombstone" => Some(TOMBSTONE_KINDS),
            "post" | "chat_thread_post" => Some(EDITED_COLUMN),
            _ => None,
        };
        let now = match expected_addition {
            Some(addition) => {
                assert!(now.contains(addition), "{name}: Zusatz aus 0046 ({addition:?}) steht im Schema: {now}");
                now.replacen(addition, "", 1)
            }
            None => now,
        };
        // The thirteen tombstone triggers too: literally as before.
        assert_eq!(Some(now), sql.clone().or(Some(String::new())), "Schema von {name} unverändert");
    }
    for (_, trigger) in KINDS {
        assert!(schema_after.contains_key(trigger), "Grabstein-Trigger {trigger} steht wieder da");
    }
    assert!(schema_after.contains_key("uuid_tombstone_work"), "Index der Grabsteine neu angelegt");

    // CHECK of uuid_tombstone: new kinds allowed, invented ones still not.
    for kind in ["finding", "finding_status", "post_version"] {
        exec(&pool, &format!("INSERT INTO uuid_tombstone (uuid, kind) VALUES ('probe-{kind}', '{kind}')")).await;
    }
    let invented =
        sqlx::query("INSERT INTO uuid_tombstone (uuid, kind) VALUES ('probe-x', 'befund')").execute(&pool).await;
    assert!(invented.is_err(), "CHECK von uuid_tombstone lehnt eine erfundene Art weiter ab");
    exec(&pool, "DELETE FROM uuid_tombstone WHERE uuid LIKE 'probe-%'").await;

    // finding: valid findings work, every forbidden deviation does not.
    let revision_uuid = text(&pool, &format!("SELECT uuid FROM revision WHERE node_id = {}", rows.nodes[0])).await;
    let node_uuid = text(&pool, &format!("SELECT uuid FROM node WHERE id = {}", rows.nodes[1])).await;
    let idea_uuid = text(&pool, "SELECT uuid FROM idea ORDER BY id LIMIT 1").await;
    exec(&pool, &format!("INSERT INTO finding ({FINDING_COLUMNS}) VALUES ({})", finding_values(&rows, 0))).await;
    let valid = finding_values(&rows, 1);
    let adopted = valid.replace("'open', '', NULL,", &format!("'adopted', '', '{revision_uuid}',"));
    let rejected = valid.replace("'open', '',", "'rejected', 'Absicht: die Mattigkeit ist gewollt.',");
    let deferred = valid.replace("'open', '', NULL, NULL,", &format!("'deferred', '', NULL, '{node_uuid}',"));
    let done = valid.replace("'open', '',", "'done', '',");
    let summary = valid.replace("'[]'", "'[\"00000000-0000-4000-8000-000000000001\"]'").replace("'critical'", "'hint'");
    for (n, good) in [&adopted, &rejected, &deferred, &done, &summary].iter().enumerate() {
        let good = good.replace(", 1, '@lektor'", &format!(", {}, '@lektor'", n + 1));
        exec(&pool, &format!("INSERT INTO finding ({FINDING_COLUMNS}) VALUES ({good})")).await;
    }
    let thread_finding = format!(
        "{}, NULL, NULL, {}, 0, '@lektor', 'hint', 'die steinerne', 'open', '', NULL, NULL, NULL, '[]'",
        rows.work, rows.thread_posts[1]
    );
    exec(&pool, &format!("INSERT INTO finding ({FINDING_COLUMNS}) VALUES ({thread_finding})")).await;
    let fresh = finding_values(&rows, 9);
    for (bad, why) in [
        (fresh.replace("'critical'", "'lob'"), "unbekannte Art"),
        (fresh.replace("'open'", "'erledigt'"), "unbekannter Status"),
        (fresh.replace("'„letzte Licht“ ist matt.'", "'   '"), "Text leer bzw. nur Leerraum"),
        (fresh.replace("'„letzte Licht“ ist matt.'", &format!("'{}'", "x".repeat(4001))), "Text über 4000 Zeichen"),
        (fresh.replace("'@lektor'", "''"), "Persona leer"),
        (fresh.replace(", 9, '@lektor'", ", -1, '@lektor'"), "Stelle negativ"),
        (fresh.replace("'open', '',", "'rejected', '  ',"), "verworfen ohne Begründung"),
        (fresh.replace("'open', '',", "'adopted', '',"), "übernommen ohne Fassung"),
        (fresh.replace("'open', '', NULL, NULL, NULL,", "'deferred', '', NULL, NULL, NULL,"), "verschoben ohne Ziel"),
        (
            fresh.replace(
                "'open', '', NULL, NULL, NULL,",
                &format!("'deferred', '', NULL, '{node_uuid}', '{idea_uuid}',"),
            ),
            "verschoben mit zwei Zielen",
        ),
        (fresh.replace("'[]'", "'{}'"), "summary_of keine Liste"),
        (fresh.replace("'[]'", "'kaputt'"), "summary_of kein JSON"),
        (
            fresh.replace(
                &format!("{}, NULL, 9,", rows.posts[1]),
                &format!("{}, {}, 9,", rows.posts[1], rows.thread_posts[1]),
            ),
            "zugleich Beitrag und Fadenbeitrag",
        ),
        (finding_values(&rows, 0), "derselbe Beitrag mit derselben Stelle zweimal"),
        (
            fresh.replace(&format!("{}, {},", rows.work, rows.nodes[0]), &format!("99999, {},", rows.nodes[0])),
            "Werk muss es geben",
        ),
    ] {
        let result =
            sqlx::query(sqlx::AssertSqlSafe(format!("INSERT INTO finding ({FINDING_COLUMNS}) VALUES ({bad})")))
                .execute(&pool)
                .await;
        assert!(result.is_err(), "finding lehnt ab: {why}");
    }
    let duplicate_thread =
        sqlx::query(sqlx::AssertSqlSafe(format!("INSERT INTO finding ({FINDING_COLUMNS}) VALUES ({thread_finding})")))
            .execute(&pool)
            .await;
    assert!(duplicate_thread.is_err(), "finding lehnt ab: derselbe Fadenbeitrag mit derselben Stelle zweimal");
    let uuids: Vec<Option<String>> = sqlx::query_scalar("SELECT uuid FROM finding").fetch_all(&pool).await.unwrap();
    assert_eq!(uuids.len(), 7, "sieben gültige Befunde");
    for uuid in &uuids {
        let uuid = uuid.as_deref().expect("Fülltrigger setzt die uuid");
        assert!(uuid.len() == 36 && uuid.as_bytes()[14] == b'4', "kanonische v4-uuid: {uuid}");
    }
    let defaults: (String, String, String) =
        sqlx::query_as("SELECT quote, quote_check, status FROM finding WHERE position = 0 AND post_id IS NOT NULL")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(defaults, (String::new(), String::new(), "open".into()), "Vorgaben: kein Zitat, kein Vermerk, offen");

    // finding_status: the human sets, the assistant proposes; every forbidden deviation does not work.
    let first: i64 = scalar(&pool, "SELECT id FROM finding WHERE position = 0 AND post_id IS NOT NULL").await;
    let human = format!("{first}, 'done', '', NULL, NULL, NULL, 'human', 'Autor', 0, NULL, NULL");
    let proposal = format!("{first}, 'done', '', NULL, NULL, NULL, 'persona', '@lektor', 1, NULL, NULL");
    let decided = format!(
        "{first}, 'done', '', NULL, NULL, NULL, 'mcp', '@mcp:probe', 1, 'confirmed', '2026-11-09T10:00:00.000Z'"
    );
    for good in [&human, &proposal, &decided] {
        exec(&pool, &format!("INSERT INTO finding_status ({STATUS_COLUMNS}) VALUES ({good})")).await;
    }
    for (bad, why) in [
        (
            human.replace("'human', 'Autor', 0", "'persona', '@lektor', 0"),
            "Status setzen ohne Vorschlag nur der Mensch",
        ),
        (
            human.replace("0, NULL, NULL", "0, 'confirmed', '2026-11-09T10:00:00.000Z'"),
            "Entscheidung nur bei Vorschlägen",
        ),
        (proposal.replace("1, NULL, NULL", "1, 'confirmed', NULL"), "Entscheidung ohne Zeit"),
        (proposal.replace("1, NULL, NULL", "1, NULL, '2026-11-09T10:00:00.000Z'"), "Zeit ohne Entscheidung"),
        (proposal.replace("1, NULL, NULL", "1, 'vielleicht', '2026-11-09T10:00:00.000Z'"), "unbekannte Entscheidung"),
        (proposal.replace(", 1, NULL", ", 2, NULL"), "proposed nur 0 oder 1"),
        (proposal.replace("'persona'", "'robot'"), "unbekannte Herkunft"),
        (human.replace("'done', ''", "'rejected', ''"), "verworfen ohne Begründung"),
        (human.replace("'done', ''", "'adopted', ''"), "übernommen ohne Fassung"),
        (
            human.replace(
                "'done', '', NULL, NULL, NULL",
                &format!("'deferred', '', NULL, '{node_uuid}', '{idea_uuid}'"),
            ),
            "verschoben mit zwei Zielen",
        ),
        (human.replace(&format!("{first}, "), "99999, "), "Befund muss es geben"),
    ] {
        let result =
            sqlx::query(sqlx::AssertSqlSafe(format!("INSERT INTO finding_status ({STATUS_COLUMNS}) VALUES ({bad})")))
                .execute(&pool)
                .await;
        assert!(result.is_err(), "finding_status lehnt ab: {why}");
    }

    // post_version: exactly one post or thread post.
    for (post, thread_post) in
        [(rows.posts[0].to_string(), "NULL".to_owned()), ("NULL".to_owned(), rows.thread_posts[0].to_string())]
    {
        exec(
            &pool,
            &format!(
                "INSERT INTO post_version (post_id, thread_post_id, body, valid_from, edited_by) \
                 VALUES ({post}, {thread_post}, 'Wie hell?', '2026-11-08T09:00:00.000Z', 'Autor')"
            ),
        )
        .await;
    }
    for (post, thread_post, why) in [
        ("NULL".to_owned(), "NULL".to_owned(), "weder Beitrag noch Fadenbeitrag"),
        (rows.posts[0].to_string(), rows.thread_posts[0].to_string(), "zugleich Beitrag und Fadenbeitrag"),
        ("99999".to_owned(), "NULL".to_owned(), "Beitrag muss es geben"),
    ] {
        let result = sqlx::query(sqlx::AssertSqlSafe(format!(
            "INSERT INTO post_version (post_id, thread_post_id, body, valid_from, edited_by) \
             VALUES ({post}, {thread_post}, 'x', '2026-11-08T09:00:00.000Z', 'Autor')"
        )))
        .execute(&pool)
        .await;
        assert!(result.is_err(), "post_version lehnt ab: {why}");
    }
    let replaced: String = text(&pool, "SELECT replaced FROM post_version ORDER BY id LIMIT 1").await;
    assert!(replaced.ends_with('Z') && replaced.len() == 24, "replaced bekommt die Zeit des Ersetzens: {replaced}");

    // Tombstones on single deletion, each with the work uuid.
    let work_uuid = text(&pool, &format!("SELECT uuid FROM work WHERE id = {}", rows.work)).await;
    let grave = |uuid: String| {
        let pool = pool.clone();
        async move {
            sqlx::query_as::<_, (String, Option<String>)>("SELECT kind, work_uuid FROM uuid_tombstone WHERE uuid = ?")
                .bind(&uuid)
                .fetch_optional(&pool)
                .await
                .unwrap()
        }
    };
    let status_uuid = text(&pool, "SELECT uuid FROM finding_status ORDER BY id LIMIT 1").await;
    exec(&pool, "DELETE FROM finding_status WHERE id = (SELECT min(id) FROM finding_status)").await;
    assert_eq!(grave(status_uuid).await, Some(("finding_status".into(), Some(work_uuid.clone()))), "Verlaufseintrag");
    let thread_finding_uuid = text(&pool, "SELECT uuid FROM finding WHERE thread_post_id IS NOT NULL").await;
    exec(&pool, "DELETE FROM finding WHERE thread_post_id IS NOT NULL").await;
    assert_eq!(grave(thread_finding_uuid).await, Some(("finding".into(), Some(work_uuid.clone()))), "Befund");
    // A finding with history takes the history along (BEFORE DELETE trigger, tombstones with work uuid).
    let history: Vec<String> = sqlx::query_scalar("SELECT uuid FROM finding_status WHERE finding_id = ?")
        .bind(first)
        .fetch_all(&pool)
        .await
        .unwrap();
    assert_eq!(history.len(), 2, "zwei Verlaufseinträge übrig");
    exec(&pool, &format!("DELETE FROM finding WHERE id = {first}")).await;
    for uuid in history {
        assert_eq!(grave(uuid).await, Some(("finding_status".into(), Some(work_uuid.clone()))), "Verlauf mit Befund");
    }
    // Deleting a post takes its old version along (tombstone with work uuid); the finding on it only loses the
    // reference.
    let version_uuid = text(&pool, &format!("SELECT uuid FROM post_version WHERE post_id = {}", rows.posts[0])).await;
    exec(&pool, &format!("DELETE FROM post WHERE id = {}", rows.posts[0])).await;
    assert_eq!(grave(version_uuid).await, Some(("post_version".into(), Some(work_uuid.clone()))), "Beitragsfassung");
    let thread_version =
        text(&pool, &format!("SELECT uuid FROM post_version WHERE thread_post_id = {}", rows.thread_posts[0])).await;
    exec(&pool, &format!("DELETE FROM chat_thread_post WHERE id = {}", rows.thread_posts[0])).await;
    assert_eq!(grave(thread_version).await, Some(("post_version".into(), Some(work_uuid.clone()))), "Fadenfassung");
    assert_eq!(scalar(&pool, "SELECT count(*) FROM post_version").await, 0, "beide alten Fassungen fort");
    let lektor_findings =
        scalar(&pool, &format!("SELECT count(*) FROM finding WHERE post_id = {}", rows.posts[1])).await;
    exec(&pool, &format!("DELETE FROM post_ref WHERE post_id = {}", rows.posts[1])).await;
    exec(&pool, &format!("DELETE FROM post WHERE id = {}", rows.posts[1])).await;
    assert_eq!(
        scalar(&pool, "SELECT count(*) FROM finding WHERE post_id IS NULL AND thread_post_id IS NULL").await,
        lektor_findings,
        "Befunde eines gelöschten Beitrags bleiben (post_id NULL, ON DELETE SET NULL)"
    );

    assert_eq!(scalar(&pool, "SELECT count(*) FROM pragma_foreign_key_check").await, 0, "foreign_key_check leer");
    let integrity: String = sqlx::query_scalar("PRAGMA integrity_check").fetch_one(&pool).await.unwrap();
    assert_eq!(integrity, "ok", "integrity_check");
    pool.close().await;

    // Store::open afterwards; deleting the work writes the tombstones of the findings, of their history and of the old
    // versions (with work uuid), and the old triggers keep writing.
    let store = Store::open(dir.path()).await.expect("Store::open nach 0046");
    let pool = store.pool();
    let finding: i64 = scalar(pool, "SELECT min(id) FROM finding").await;
    exec(
        pool,
        &format!(
            "INSERT INTO finding_status ({STATUS_COLUMNS}) VALUES \
             ({finding}, 'done', '', NULL, NULL, NULL, 'human', 'Autor', 0, NULL, NULL)"
        ),
    )
    .await;
    let thread_post: i64 = scalar(pool, "SELECT min(id) FROM chat_thread_post").await;
    exec(
        pool,
        &format!(
            "INSERT INTO post_version (thread_post_id, body, valid_from, edited_by) \
             VALUES ({thread_post}, 'früher', '2026-11-08T09:00:00.000Z', 'Autor')"
        ),
    )
    .await;
    let mine: BTreeSet<String> = sqlx::query_scalar(
        "SELECT uuid FROM finding UNION ALL SELECT uuid FROM finding_status UNION ALL SELECT uuid FROM post_version",
    )
    .fetch_all(pool)
    .await
    .unwrap()
    .into_iter()
    .collect();
    assert!(mine.len() >= 7, "Befunde, Verlauf und Fassung vor dem Löschen des Werks: {mine:?}");
    exec(pool, "DELETE FROM idea WHERE note = 'Spiegelung im Wasser'").await; // kinds idea, idea_link (test only)
    store.delete_work(rows.work, "Die Laterne am Kanal").await.expect("Werk löschen schreibt Grabsteine");
    for table in ["finding", "finding_status", "post_version"] {
        assert_eq!(scalar(pool, &format!("SELECT count(*) FROM {table}")).await, 0, "{table} geht mit dem Werk");
    }
    let fresh: Vec<(String, String, Option<String>)> = sqlx::query_as(
        "SELECT uuid, kind, work_uuid FROM uuid_tombstone WHERE uuid NOT LIKE '00000000-0000-4000-8000-%' ORDER BY kind",
    )
    .fetch_all(pool)
    .await
    .unwrap();
    for uuid in &mine {
        let found = fresh.iter().find(|(u, ..)| u == uuid).unwrap_or_else(|| panic!("Grabstein für {uuid}"));
        assert_eq!(found.2.as_deref(), Some(work_uuid.as_str()), "Grabstein {} {uuid} mit Werk-uuid", found.1);
    }
    let kinds: BTreeSet<&str> = fresh.iter().map(|(_, k, _)| k.as_str()).collect();
    for kind in KINDS.iter().map(|(k, _)| *k).chain(["finding", "finding_status", "post_version"]) {
        assert!(kinds.contains(kind), "nach 0046 schreibt der Trigger der Art {kind}: {kinds:?}");
    }
    for (uuid, kind, grave) in &fresh {
        assert_eq!(grave.as_deref(), Some(work_uuid.as_str()), "Grabstein {kind} {uuid} trägt die Werk-uuid");
    }
}
