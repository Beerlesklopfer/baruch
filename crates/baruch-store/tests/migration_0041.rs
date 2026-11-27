// SPDX-License-Identifier: AGPL-3.0-or-later
//! Migration 0041 (Kennungen, Runde 26) und ihre Nachbesserung 0042 auf einer nachgebauten Datenbank im Stand
//! von development vor Runde 26 (Store bis 0040, Bibliothek bis 0106) mit frei erfundenen Zeilen in allen zehn
//! Tabellen, die eine uuid bekommen.
//!
//! Belegt: Jede Zeile jeder Tabelle bleibt bis auf die neue Spalte gleich; alle uuid sind gefüllt, eindeutig
//! und kanonisch v4; das Schema aller anderen Objekte bleibt unverändert, neu sind genau die Objekte aus
//! 0041/0042; Fremdschlüssel sind danach an und greifen; die Spalte hält nur kanonische, eindeutige Werte;
//! `store_meta.instance_uuid` ist da; ein zweites `Store::open` ist wiederholbar und vergibt keine neue uuid.

use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::str::FromStr;

use baruch_store::{Store, title_key};
use sqlx::migrate::Migrator;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};

const BOOK: &str = "#set document(title: \"Das Wiesenbuch\", author: \"Hanne Heu\")\n\n\
= Gräser\n\nHalm an Halm.\n\n= Blumen\n\nKlee und Salbei.\n\n= Mahd\n\nIm Juni.\n";

const SERMON: &str = "#set document(title: \"Vom Senfkorn\", author: \"Hanne Heu\")\n\n\
= Hinführung\n\nKlein fängt es an.\n\n= Ausblick\n\nVögel nisten darin.\n";

/// Die zehn Tabellen mit Werkinhalt, die 0041 um `uuid` erweitert.
const TABLES: [&str; 10] = [
    "work",
    "node",
    "revision",
    "work_template_revision",
    "post",
    "idea",
    "chat_thread",
    "chat_thread_post",
    "asset",
    "asset_revision",
];

/// Zusätze aus 0045 (Runde 27), die `Store::open` mit ausführt: die vierte Antragsart in `asset_request` und die
/// zwei Anschriften in `work_book` (geprüft in migration_0045.rs).
const OP_COVER_LAYOUT: &str = ",'cover_layout'";
const ADDRESS_COLUMNS: &str = ", publisher_address TEXT NOT NULL DEFAULT '', printer_address TEXT NOT NULL DEFAULT ''";

fn without_0045(sql: &str) -> String {
    sql.replace(OP_COVER_LAYOUT, "").replace(ADDRESS_COLUMNS, "")
}

/// Runde 28 (0046): `edited` an `post` und `chat_thread_post` (andere Tabellen wie `revision` haben eine Spalte
/// `edited` schon vorher und bleiben wörtlich).
fn without_0046(name: &str, sql: &str) -> String {
    if name == "post" || name == "chat_thread_post" { sql.replace(", edited TEXT", "") } else { sql.to_owned() }
}

const UUID_COLUMN: &str = ", uuid TEXT CHECK (uuid IS NULL OR (length(uuid) = 36 AND uuid = lower(uuid)))";

/// Runde 34 (0052) hängt Titel, Elternknoten, Reihenfolge und Hauptthema an `idea` an (der gespeicherte CREATE-Text
/// wächst um genau diesen Zusatz, direkt hinter der Spalte uuid; geprüft in migration_0052.rs); der Vergleich des
/// alten Schemas nimmt ihn heraus.
const R34_IDEA_COLUMNS: &str = ", title TEXT NOT NULL DEFAULT '' CHECK (length(title) <= 200), parent_id INTEGER REFERENCES idea(id) ON DELETE SET NULL, position INTEGER NOT NULL DEFAULT 0, main_topic INTEGER NOT NULL DEFAULT 0 CHECK (main_topic IN (0, 1))";
/// Round 38 (0055): column appended to `work` after that (as SQLite stores it in the definition of the table).
const R38_WORK_COLUMN: &str = ", language TEXT NOT NULL DEFAULT 'de' CHECK (language IN ('de', 'en'))";

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

/// Store bis 0040 (0040 wie in `baruch_store::migrate` mit ausgeschalteten Fremdschlüsseln) und Bibliothek
/// bis 0106: der Stand von development vor Runde 26.
async fn migrate_before(pool: &SqlitePool) {
    assert!(baruch_store::MIGRATOR.iter().any(|m| m.version == 41), "die neue Migration ist 0041");
    assert!(baruch_store::MIGRATOR.iter().any(|m| m.version == 42), "Nachbesserung 0042 gibt es");
    assert!(!baruch_store::FOREIGN_KEYS_OFF.contains(&41), "0041 baut keine Tabelle um");
    assert!(!baruch_store::FOREIGN_KEYS_OFF.contains(&42), "0042 baut keine Tabelle um");
    store_migrator(|v| v < 40).run(pool).await.unwrap();
    {
        let mut conn = pool.acquire().await.unwrap();
        sqlx::query("PRAGMA foreign_keys = OFF").execute(&mut *conn).await.unwrap();
        store_migrator(|v| v == 40).run(&mut *conn).await.unwrap();
        sqlx::query("PRAGMA foreign_keys = ON").execute(&mut *conn).await.unwrap();
    }
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../baruch-library/migrations");
    let mut library = Migrator::new(dir).await.unwrap();
    library.migrations = Cow::Owned(library.iter().filter(|m| m.version <= 106).cloned().collect());
    library.ignore_missing = true;
    library.run(pool).await.unwrap();
    let highest: i64 = scalar(pool, "SELECT max(version) FROM _sqlx_migrations WHERE version < 100").await;
    assert_eq!(highest, 40, "Stand davor: Store bis 0040");
}

async fn exec(pool: &SqlitePool, sql: &str) {
    sqlx::query(sqlx::AssertSqlSafe(sql)).execute(pool).await.unwrap_or_else(|e| panic!("{sql}: {e}"));
}

async fn scalar(pool: &SqlitePool, sql: &str) -> i64 {
    sqlx::query_scalar(sqlx::AssertSqlSafe(sql)).fetch_one(pool).await.unwrap_or_else(|e| panic!("{sql}: {e}"))
}

/// Werk mit Gliederung (je Überschrift ein Kapitel mit aktueller Fassung) und Vorlagenfassung direkt in SQLite.
async fn insert_work(pool: &SqlitePool, slug: &str, kind: &str, source: &str) -> (i64, Vec<i64>) {
    let doc = baruch_typst::parse(source).unwrap();
    let title = slug.replace('-', " ");
    let work: i64 = sqlx::query_scalar(
        "INSERT INTO work (slug, kind, title, author, template, source_name, source_sha256, title_key, subtitle) \
         VALUES (?, ?, ?, 'Hanne Heu', ?, ?, ?, ?, 'Unterzeile') RETURNING id",
    )
    .bind(slug)
    .bind(kind)
    .bind(&title)
    .bind(&doc.template)
    .bind(format!("{slug}.typ"))
    .bind(format!("sha-{slug}"))
    .bind(title_key(&title))
    .fetch_one(pool)
    .await
    .unwrap();
    for (number, note) in [(1, "Anfang"), (2, "Sprache")] {
        sqlx::query(
            "INSERT INTO work_template_revision (work_id, number, body, author_name, note) VALUES (?, ?, ?, 'Hanne', ?)",
        )
        .bind(work)
        .bind(number)
        .bind(format!("{}// Fassung {number}\n", doc.template))
        .bind(note)
        .execute(pool)
        .await
        .unwrap();
    }
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
             VALUES (?, 1, ?, 'human', 'Hanne', NULL, 'current', 'Import')",
        )
        .bind(id)
        .bind(&heading.body)
        .execute(pool)
        .await
        .unwrap();
        nodes.push(id);
    }
    (work, nodes)
}

/// Zeilen in allen zehn Tabellen (und den Nachbartabellen, die auf sie verweisen).
async fn insert_rows(pool: &SqlitePool) {
    exec(pool, "INSERT INTO store_meta (key, value) VALUES ('seeded', '1') ON CONFLICT DO NOTHING").await;
    let (book, nodes) = insert_work(pool, "das-wiesenbuch", "book", BOOK).await;
    let (sermon, sermon_nodes) = insert_work(pool, "vom-senfkorn", "sermon", SERMON).await;
    sqlx::query(
        "INSERT INTO revision (node_id, number, body, author_kind, author_name, model, status, note, based_on) \
         VALUES (?, 2, '= Blumen\n\nKlee, Salbei, Mohn.\n', 'assistant', '@lektor', 'probe-a', 'proposed', 'Vorschlag', 1)",
    )
    .bind(nodes[1])
    .execute(pool)
    .await
    .unwrap();
    let post: i64 = sqlx::query_scalar(
        "INSERT INTO post (node_id, position, author, body, sources) VALUES (?, 0, '@lektor', 'Mohn passt?', '[\"Feldbuch\"]') RETURNING id",
    )
    .bind(nodes[1])
    .fetch_one(pool)
    .await
    .unwrap();
    let thread: i64 = sqlx::query_scalar(
        "INSERT INTO chat_thread (work_id, created_by, title) VALUES (?, 'Hanne', 'Wann mähen?') RETURNING id",
    )
    .bind(book)
    .fetch_one(pool)
    .await
    .unwrap();
    let asset: i64 = sqlx::query_scalar("INSERT INTO asset (work_id, name) VALUES (?, 'wiese.svg') RETURNING id")
        .bind(book)
        .fetch_one(pool)
        .await
        .unwrap();
    let svg = "<svg xmlns=\"http://www.w3.org/2000/svg\"><rect width=\"2\" height=\"1\"/></svg>";
    for sql in [
        format!("INSERT INTO post_ref (post_id, position, node_seq, rev, quote) VALUES ({post}, 0, 1, 1, 'Klee')"),
        format!("INSERT INTO chat_thread_post (thread_id, position, author, body) VALUES ({thread}, 0, 'Hanne', 'Im Juni.')"),
        format!("INSERT INTO chat_thread_post (thread_id, position, author, body) VALUES ({thread}, 1, '@lektor', 'Nach der Blüte.')"),
        format!(
            "INSERT INTO asset_revision (asset_id, number, mime, encoding, content, bytes, sha256, caption, alt, author_name, status) \
             VALUES ({asset}, 1, 'image/svg+xml', 'svg', '{svg}', {}, 'abc', 'Wiese', 'Wiese', 'Hanne', 'current')",
            svg.len()
        ),
        format!("INSERT INTO asset_path (work_id, path, asset_id) VALUES ({book}, 'bilder/wiese.svg', {asset})"),
        format!("INSERT INTO work_book (work_id, cover_asset_id, publisher) VALUES ({book}, {asset}, 'Heuverlag')"),
        format!(
            "INSERT INTO idea (work_id, node_id, note, origin_kind, origin_name) VALUES ({book}, {}, 'Mohn als Bild', 'human', 'Hanne')",
            nodes[2]
        ),
        format!(
            "INSERT INTO idea (work_id, node_id, note, origin_kind, origin_name) VALUES ({sermon}, {}, 'Senf schmeckt scharf', 'persona', '@lektor')",
            sermon_nodes[0]
        ),
        "INSERT INTO idea (work_id, node_id, note, origin_kind, origin_name, deleted) VALUES (NULL, NULL, 'ohne Werk', 'mcp', '@mcp:probe', '2026-11-01T10:00:00.000Z')".to_owned(),
        format!("INSERT INTO work_timing (work_id, words_per_minute, target_seconds) VALUES ({sermon}, 120, 900)"),
    ] {
        exec(pool, &sql).await;
    }
    for table in TABLES {
        assert!(
            scalar(pool, &format!("SELECT count(*) FROM {table}")).await > 0,
            "{table} hat Zeilen vor der Migration"
        );
    }
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

/// Alle Schemaobjekte: Name → (Typ, Tabelle, SQL).
async fn schema(pool: &SqlitePool) -> BTreeMap<String, (String, String, Option<String>)> {
    let rows =
        sqlx::query("SELECT name, type, tbl_name, sql FROM sqlite_master ORDER BY name").fetch_all(pool).await.unwrap();
    rows.into_iter().map(|r| (r.get(0), (r.get(1), r.get(2), r.get(3)))).collect()
}

/// Inhalt jeder Tabelle in den Spalten aus `layout` (alte Spalten; die neue Spalte uuid fehlt darin).
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

/// (Tabelle, id) → uuid über alle zehn Tabellen.
async fn uuids(pool: &SqlitePool) -> BTreeMap<(String, i64), Option<String>> {
    let mut out = BTreeMap::new();
    for table in TABLES {
        let rows: Vec<(i64, Option<String>)> =
            sqlx::query_as(sqlx::AssertSqlSafe(format!("SELECT id, uuid FROM {table}"))).fetch_all(pool).await.unwrap();
        for (id, uuid) in rows {
            out.insert((table.to_owned(), id), uuid);
        }
    }
    out
}

fn is_v4(uuid: &str) -> bool {
    baruch_exchange::is_uuid(uuid) && &uuid[14..15] == "4" && matches!(&uuid[19..20], "8" | "9" | "a" | "b")
}

/// Objekte, die 0041 und 0042 neu anlegen (Namen), dazu die aus 0043, die `Store::open` mit ausführt.
fn new_objects() -> BTreeSet<String> {
    let mut names: BTreeSet<String> = ["uuid_tombstone", "uuid_tombstone_work", "sqlite_autoindex_uuid_tombstone_1"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    for table in TABLES {
        for suffix in ["_uuid", "_uuid_fill", "_uuid_tombstone"] {
            names.insert(format!("{table}{suffix}"));
        }
    }
    names.insert("work_cascade_uuid_tombstone".into());
    names.insert("chat_thread_cascade_uuid_tombstone".into());
    // Runde 25 (0043): `Store::open` legt auch die Tabellen der Gedankenkarte samt Indizes und Triggern an; die
    // zehn Grabstein-Trigger und uuid_tombstone baut 0043 nur um (oben schon enthalten).
    for table in ["idea_link", "idea_map_pos"] {
        for suffix in ["", "_uuid", "_uuid_fill", "_uuid_tombstone"] {
            names.insert(format!("{table}{suffix}"));
        }
        names.insert(format!("sqlite_autoindex_{table}_1"));
    }
    names.insert("idea_link_to".into());
    names.insert("work_map_pos_cascade_uuid_tombstone".into());
    names.insert("idea_link_cascade_uuid_tombstone".into());
    // Runde 18c (0044): `Store::open` legt auch die Tabelle der Satzbild-Einstellungen an (ohne Index und Trigger).
    names.insert("work_sheet".into());
    // Runde 27 (0045): `Store::open` legt auch die Blöcke der Umschlag-Anordnung an; uuid_tombstone, die zwölf
    // Grabstein-Trigger und asset_request baut 0045 nur um.
    for suffix in ["", "_uuid", "_uuid_fill", "_uuid_tombstone"] {
        names.insert(format!("cover_block{suffix}"));
    }
    names.insert("sqlite_autoindex_cover_block_1".into());
    names.insert("work_cover_block_cascade_uuid_tombstone".into());
    // Runde 28 (0046): Befunde, ihr Verlauf und alte Beitragsfassungen samt Indizes und Triggern; uuid_tombstone und
    // die dreizehn Grabstein-Trigger baut 0046 nur um (geprüft in migration_0046.rs).
    names.extend(R28_OBJECTS.iter().map(|s| s.to_string()));
    // Runde 30 (0047): Protokoll des Ersetzens samt Index, Teilindex der Varianten (geprüft in migration_0047.rs).
    names.extend(R30_OBJECTS.iter().map(|s| s.to_string()));
    // Runde 29 (0048): Satz-Einstellungen je Werk samt Index, Fülltrigger, Grabstein und Kaskade; uuid_tombstone und
    // die sechzehn Grabstein-Trigger baut 0048 nur um (geprüft in migration_0048.rs).
    names.extend(R29_OBJECTS.iter().map(|s| s.to_string()));
    // Runde 31 (0049): Glossar, eigene Register und Registervorschläge samt Indizes und Triggern; uuid_tombstone und die
    // siebzehn Grabstein-Trigger baut 0049 nur um (geprüft in migration_0049.rs).
    names.extend(R31_OBJECTS.iter().map(|s| s.to_string()));
    // Runde 22 (0050): Verlauf der Fortschreibungen samt Index und Triggern, Einstellung „Fassungen“; uuid_tombstone
    // und die neunzehn Grabstein-Trigger baut 0050 nur um (geprüft in migration_0050.rs).
    names.extend(R22_OBJECTS.iter().map(|s| s.to_string()));
    // 0.9.0 (0051): Marke „ältere Fassungen ausblenden“ je Abschnitt (geprüft in revision_fold.rs).
    names.insert("node_revision_fold".into());
    // Runde 34 (0052): Index über Elternknoten und Reihenfolge der Einfälle.
    names.insert("idea_parent".into());
    // Runde 35 (0053, 0054): Konten, Sitzungen, Sperre gegen Durchprobieren und Freigaben samt Indizes – Instanzdaten
    // ohne uuid, nicht im Austausch (geprüft in migration_0053.rs und migration_0054.rs).
    names.extend(R35_OBJECTS.iter().map(|s| s.to_string()));
    names
}

/// Objekte, die 0050 (Runde 22) neu anlegt.
const R35_OBJECTS: [&str; 11] = [
    "account",
    "account_session",
    "account_session_account",
    "login_attempt",
    "sqlite_autoindex_account_1",
    "sqlite_autoindex_account_2",
    "sqlite_autoindex_account_session_1",
    "sqlite_autoindex_login_attempt_1",
    "sqlite_autoindex_work_member_1",
    "work_member",
    "work_member_account",
];

const R22_OBJECTS: [&str; 7] = [
    "revision_amend",
    "revision_amend_uuid",
    "revision_amend_uuid_fill",
    "revision_amend_uuid_tombstone",
    "revision_amend_before_revision_delete",
    "sqlite_autoindex_revision_amend_1",
    "work_revision_policy",
];

/// Objekte, die 0049 (Runde 31) neu anlegt.
const R31_OBJECTS: [&str; 18] = [
    "glossary_term",
    "glossary_term_uuid",
    "glossary_term_accepted_key",
    "glossary_term_work",
    "glossary_term_uuid_fill",
    "glossary_term_uuid_tombstone",
    "glossary_term_cascade_uuid_tombstone",
    "glossary_variant",
    "sqlite_autoindex_glossary_variant_1",
    "glossary_variant_form_key",
    "index_register",
    "sqlite_autoindex_index_register_1",
    "index_register_uuid",
    "index_register_uuid_fill",
    "index_register_uuid_tombstone",
    "index_register_cascade_uuid_tombstone",
    "index_proposal",
    "index_proposal_work_status",
];

/// Objekte, die 0048 (Runde 29) neu anlegt.
const R29_OBJECTS: [&str; 5] = [
    "work_typeset",
    "work_typeset_uuid",
    "work_typeset_uuid_fill",
    "work_typeset_uuid_tombstone",
    "work_typeset_cascade_uuid_tombstone",
];

/// Objekte, die 0046 (Runde 28) neu anlegt.
const R28_OBJECTS: [&str; 22] = [
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

#[tokio::test]
async fn migration_0041_adds_canonical_uuids_and_keeps_every_row_and_object() {
    let dir = tempfile::tempdir().unwrap();
    let pool = raw_pool(dir.path()).await;
    migrate_before(&pool).await;
    insert_rows(&pool).await;
    let layout = tables(&pool).await;
    let schema_before = schema(&pool).await;
    let before = contents(&pool, &layout).await;
    for table in TABLES {
        assert!(!layout[table].contains(&"uuid".to_owned()), "{table} hat vor 0041 keine Spalte uuid");
    }
    pool.close().await;

    let store = Store::open(dir.path()).await.expect("Store::open führt 0041 und 0042 aus");
    let pool = store.pool();
    for version in [41, 42] {
        let ran =
            scalar(pool, &format!("SELECT count(*) FROM _sqlx_migrations WHERE version = {version} AND success")).await;
        assert_eq!(ran, 1, "{version:04} lief");
    }

    // Daten: jede alte Zeile jeder Tabelle in den alten Spalten unverändert; neu ist nur die Zeile instance_uuid.
    let mut after = contents(pool, &layout).await;
    let meta = after.get_mut("store_meta").unwrap();
    let count = meta.len();
    meta.retain(|row| !row.starts_with("'instance_uuid'\u{1f}"));
    assert_eq!(count - meta.len(), 1, "genau eine neue Zeile instance_uuid in store_meta");
    for (table, rows) in &before {
        if table != "_sqlx_migrations" {
            assert_eq!(after.get(table), Some(rows), "Tabelle {table}: alle Zeilen unverändert (alte Spalten)");
        }
    }

    // Spalte uuid: zuletzt angehängt, jede Zeile gefüllt, kanonisch v4, eindeutig auch über Tabellen hinweg.
    let layout_after = tables(pool).await;
    for table in TABLES {
        let mut cols = layout_after[table].clone();
        // Runde 30 (0047) hängt danach Metadaten an Beiträge und Fadenbeiträge und Varianten an Fassungen an.
        let r30 = match table {
            "post" | "chat_thread_post" => 6,
            // Runde 22 (0050) hängt danach amend_seq und based_on_amend an Fassungen an.
            "revision" => 6,
            // Runde 34 (0052) hängt danach title, parent_id, position und main_topic an Einfälle an.
            "idea" => 4,
            // Round 38 (0055) appends language to works after that.
            "work" => 1,
            _ => 0,
        };
        cols.truncate(cols.len() - r30);
        // Runde 28 (0046) hängt danach `edited` an Beiträge und Fadenbeiträge an.
        if ["post", "chat_thread_post"].contains(&table) {
            assert_eq!(cols.pop().as_deref(), Some("edited"), "{table}: edited aus 0046 zuletzt");
        }
        assert_eq!(cols.pop().as_deref(), Some("uuid"), "{table}: uuid als letzte Spalte");
        assert_eq!(cols, layout[table], "{table}: alte Spalten in alter Reihenfolge");
    }
    let ids = uuids(pool).await;
    let mut seen = BTreeSet::new();
    for ((table, id), uuid) in &ids {
        let uuid = uuid.as_deref().unwrap_or_else(|| panic!("{table} {id} ohne uuid"));
        assert!(is_v4(uuid), "{table} {id}: kanonische v4-uuid, war {uuid}");
        assert!(seen.insert(uuid.to_owned()), "{table} {id}: uuid {uuid} doppelt");
    }
    let instance = store.instance().await.unwrap();
    assert!(is_v4(&instance.uuid), "Instanzkennung kanonisch v4: {}", instance.uuid);
    assert_eq!(instance.name, "", "Name der Instanz leer");
    assert_eq!(scalar(pool, "SELECT count(*) FROM uuid_tombstone").await, 0, "die Migration löscht nichts");

    // Schema: jedes alte Objekt unverändert (bis auf den Spaltenzusatz im CREATE-Text der zehn Tabellen), neu
    // sind genau die Objekte aus 0041/0042.
    let schema_after = schema(pool).await;
    let added: BTreeSet<String> = schema_after.keys().filter(|k| !schema_before.contains_key(*k)).cloned().collect();
    assert_eq!(added, new_objects(), "neu sind genau Tabelle uuid_tombstone, Indizes und Trigger aus 0041/0042");
    for (name, (kind, table, sql)) in &schema_before {
        let (kind_now, table_now, sql_now) = schema_after.get(name).unwrap_or_else(|| panic!("{name} verschwunden"));
        assert_eq!((kind_now, table_now), (kind, table), "{name}: Typ und Tabelle unverändert");
        let sql_now = sql_now
            .as_ref()
            .map(|s| s.replace(R34_IDEA_COLUMNS, "").replace(R38_WORK_COLUMN, ""))
            .map(|s| without_r30(name, &without_0046(name, &without_0045(&s.replace(UUID_COLUMN, "")))));
        assert_eq!(&sql_now, sql, "Schema von {name} unverändert (bis auf die Spalte uuid)");
        if TABLES.contains(&name.as_str()) {
            assert!(schema_after[name].2.as_ref().unwrap().contains(UUID_COLUMN), "{name}: Spaltenzusatz");
        }
    }

    // Fremdschlüssel an, ohne Verstöße, und sie greifen.
    assert_eq!(scalar(pool, "SELECT count(*) FROM pragma_foreign_key_check").await, 0, "foreign_key_check leer");
    let integrity: String = sqlx::query_scalar("PRAGMA integrity_check").fetch_one(pool).await.unwrap();
    assert_eq!(integrity, "ok", "integrity_check");
    let mut held = Vec::new();
    for _ in 0..4 {
        held.push(pool.acquire().await.unwrap());
    }
    for conn in &mut held {
        let on: i64 = sqlx::query_scalar("PRAGMA foreign_keys").fetch_one(&mut **conn).await.unwrap();
        assert_eq!(on, 1, "Fremdschlüssel auf jeder Verbindung an");
    }
    drop(held);
    let orphan = sqlx::query(
        "INSERT INTO node (work_id, seq, position, number, title, kind) VALUES (99999, 0, 0, '1', 'x', 'chapter')",
    )
    .execute(pool)
    .await;
    assert!(orphan.is_err(), "Fremdschlüssel greifen: Punkt braucht ein vorhandenes Werk");

    // Die Spalte nimmt nur kanonische, eindeutige Werte (CHECK und eindeutiger Index).
    let (some_id, some_uuid): (i64, String) =
        sqlx::query_as("SELECT id, uuid FROM node ORDER BY id LIMIT 1").fetch_one(pool).await.unwrap();
    let upper = sqlx::query("UPDATE node SET uuid = upper(uuid) WHERE id = ?").bind(some_id).execute(pool).await;
    assert!(upper.is_err(), "Großschreibung abgelehnt (CHECK uuid = lower(uuid))");
    let short = sqlx::query("UPDATE node SET uuid = 'abc' WHERE id = ?").bind(some_id).execute(pool).await;
    assert!(short.is_err(), "falsche Länge abgelehnt (CHECK length = 36)");
    let twice = sqlx::query("UPDATE node SET uuid = ? WHERE id <> ? AND id = (SELECT max(id) FROM node)")
        .bind(&some_uuid)
        .bind(some_id)
        .execute(pool)
        .await;
    assert!(twice.is_err(), "doppelte uuid abgelehnt (UNIQUE INDEX node_uuid)");

    // Wiederholbar: zweites Öffnen vergibt keine neue uuid, ändert Instanz, Zeilen und Schema nicht.
    let full_layout = tables(pool).await;
    let full = contents(pool, &full_layout).await;
    drop(store);
    let again = Store::open(dir.path()).await.expect("zweites Öffnen");
    assert_eq!(uuids(again.pool()).await, ids, "keine neue uuid beim zweiten Öffnen");
    assert_eq!(again.instance().await.unwrap(), instance, "Instanzkennung bleibt");
    assert_eq!(schema(again.pool()).await, schema_after, "Schema unverändert");
    assert_eq!(contents(again.pool(), &full_layout).await, full, "keine Zeile geändert");
}

#[tokio::test]
async fn new_rows_get_a_uuid_from_the_trigger_and_deleted_rows_a_tombstone() {
    // Die Trigger arbeiten auch für Zeilen, die nicht über den Store entstehen (z. B. nachgebaute Datenbanken).
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).await.unwrap();
    let pool = store.pool();
    let work: i64 = sqlx::query_scalar(
        "INSERT INTO work (slug, kind, title, author, title_key) VALUES ('roh', 'book', 'Roh', 'X', 'roh') RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    let uuid: String =
        sqlx::query_scalar("SELECT uuid FROM work WHERE id = ?").bind(work).fetch_one(pool).await.unwrap();
    assert!(is_v4(&uuid), "Fülltrigger vergibt v4: {uuid}");
    // Eine mitgegebene uuid überschreibt der Trigger nicht (WHEN NEW.uuid IS NULL): so legt exchange_apply an.
    let given = "0f0e0d0c-0b0a-4908-8706-050403020100";
    let other: i64 = sqlx::query_scalar(
        "INSERT INTO work (slug, kind, title, author, title_key, uuid) VALUES ('gegeben', 'book', 'Gegeben', 'X', 'gegeben', ?) RETURNING id",
    )
    .bind(given)
    .fetch_one(pool)
    .await
    .unwrap();
    let kept: String =
        sqlx::query_scalar("SELECT uuid FROM work WHERE id = ?").bind(other).fetch_one(pool).await.unwrap();
    assert_eq!(kept, given, "mitgegebene uuid bleibt");
    exec(pool, &format!("DELETE FROM work WHERE id = {other}")).await;
    let grave: (String, Option<String>) = sqlx::query_as("SELECT kind, work_uuid FROM uuid_tombstone WHERE uuid = ?")
        .bind(given)
        .fetch_one(pool)
        .await
        .unwrap();
    assert_eq!(
        grave,
        ("work".to_owned(), Some(given.to_owned())),
        "Grabstein des Werks trägt die eigene uuid als Werk"
    );
}

/// Runde 30 (0047) hängt Spalten an `post`, `chat_thread_post`, `finding` und `revision` an (`ALTER TABLE … ADD
/// COLUMN` erweitert den gespeicherten CREATE-Text um genau diese Zusätze; geprüft in migration_0047.rs); der
/// Vergleich des alten Schemas nimmt sie heraus.
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

/// Objekte, die 0047 (Runde 30) neu anlegt.
#[allow(dead_code)]
const R30_OBJECTS: [&str; 3] = ["replace_batch", "replace_batch_work", "revision_variant_group"];

/// Runde 22 (0050) hängt `amend_seq`/`based_on_amend` an `revision` und `adopted_amend` an `finding` und
/// `finding_status` an (`ALTER TABLE … ADD COLUMN`, geprüft in migration_0050.rs); der Vergleich des alten Schemas
/// nimmt sie heraus.
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
