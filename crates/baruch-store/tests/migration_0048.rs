// SPDX-License-Identifier: AGPL-3.0-or-later
//! Migration 0048 (Satz-Einstellungen je Werk; Runde 29a, Vertrag §3) auf einer nachgebauten Datenbank im Stand von
//! development vor Runde 29 (Store bis 0047, Bibliothek vollständig) mit frei erfundenen Zeilen in allen Tabellen mit
//! uuid (Werk, Kapitel, Fassungen, Vorlagenfassung, Beiträge samt Verweis und alter Fassung, Faden, Bild,
//! Einfälle samt Verbindung und Lage, Umschlag-Block, Befunde samt Verlauf, Protokoll des Ersetzens) und Grabsteinen
//! aller sechzehn Arten.
//!
//! Belegt: 0048 ist die einzige neue Store-Migration (0049–0051 unbenutzt) und steht nicht in `FOREIGN_KEYS_OFF`; auf
//! die umgebaute Tabelle `uuid_tombstone` zeigt kein Fremdschlüssel (darum löst das implizite DELETE von DROP TABLE
//! keine Aktion aus); jede Zeile jeder alten Tabelle bleibt byte-gleich samt ids, auch alle Grabsteine; im Schema
//! ändert sich nur der CHECK von `uuid_tombstone`, die sechzehn Grabstein-Trigger stehen wörtlich wieder da; neu sind
//! genau die Objekte von `work_typeset`; die CHECKs lehnen ab, was der Vertrag verbietet; Fülltrigger und Grabstein
//! (mit Werk-uuid) feuern beim Einzellöschen und beim Löschen des Werks, und alle alten Grabstein-Trigger schreiben
//! danach weiter.

use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::str::FromStr;

use baruch_store::{Store, title_key};
use sqlx::migrate::Migrator;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};

const BOOK: &str = "#set document(title: \"Der Kahn im Schilf\", author: \"Ole Ried\")\n\n\
= Morgen\n\nDer Kahn liegt still im Schilf.\n\n= Abend\n\nDie Reiher kehren heim.\n";

/// Die sechzehn Grabstein-Arten vor 0048 (Wert in `uuid_tombstone.kind`) und der Name ihres Grabstein-Triggers.
const KINDS: [(&str, &str); 16] = [
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
    ("finding", "finding_uuid_tombstone"),
    ("finding_status", "finding_status_uuid_tombstone"),
    ("post_version", "post_version_uuid_tombstone"),
];

/// Zusatz, um den sich der CHECK von `uuid_tombstone` nach 0048 unterscheiden darf.
const TOMBSTONE_KIND: &str = ",\n                                          'work_typeset'";

/// Objekte, die 0048 neu anlegt.
const NEW_OBJECTS: [&str; 5] = [
    "work_typeset",
    "work_typeset_uuid",
    "work_typeset_uuid_fill",
    "work_typeset_uuid_tombstone",
    "work_typeset_cascade_uuid_tombstone",
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
    sqlx::query(sql).execute(pool).await.unwrap_or_else(|e| panic!("{sql}: {e}"));
}

async fn scalar(pool: &SqlitePool, sql: &str) -> i64 {
    sqlx::query_scalar(sql).fetch_one(pool).await.unwrap_or_else(|e| panic!("{sql}: {e}"))
}

async fn text(pool: &SqlitePool, sql: &str) -> String {
    sqlx::query_scalar(sql).fetch_one(pool).await.unwrap_or_else(|e| panic!("{sql}: {e}"))
}

async fn rejected(pool: &SqlitePool, sql: &str) -> bool {
    sqlx::query(sql).execute(pool).await.is_err()
}

/// Store bis 0047 (0040 wie in `baruch_store::migrate` mit ausgeschalteten Fremdschlüsseln) und die Bibliothek:
/// der Stand von development vor Runde 29.
async fn migrate_before(pool: &SqlitePool) {
    let versions: Vec<i64> = baruch_store::MIGRATOR.iter().map(|m| m.version).filter(|v| *v < 100).collect();
    // Runde 31 (0049) kam danach; 0048 ist die einzige neue Migration ihrer Runde (0049ff. prüft migration_0049.rs).
    assert_eq!(
        versions.iter().copied().find(|v| *v > 47),
        Some(48),
        "0048 ist die erste Migration nach 0047 (0049–0051 der Runde 29 unbenutzt)"
    );
    assert!(!baruch_store::FOREIGN_KEYS_OFF.contains(&48), "0048 baut keine Tabelle mit Fremdschlüsseln um");
    store_migrator(|v| v < 40).run(pool).await.unwrap();
    {
        let mut conn = pool.acquire().await.unwrap();
        sqlx::query("PRAGMA foreign_keys = OFF").execute(&mut *conn).await.unwrap();
        store_migrator(|v| v == 40).run(&mut *conn).await.unwrap();
        sqlx::query("PRAGMA foreign_keys = ON").execute(&mut *conn).await.unwrap();
    }
    store_migrator(|v| (41..=47).contains(&v)).run(pool).await.unwrap();
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../baruch-library/migrations");
    let mut library = Migrator::new(dir).await.unwrap();
    library.ignore_missing = true;
    library.run(pool).await.unwrap();
    let highest = scalar(pool, "SELECT max(version) FROM _sqlx_migrations WHERE version < 100").await;
    assert_eq!(highest, 47, "Stand davor: Store bis 0047");
    let library_highest = scalar(pool, "SELECT max(version) FROM _sqlx_migrations WHERE version >= 100").await;
    assert!(library_highest >= 107, "Bibliothek mindestens bis 0107 (0108 kam in Runde 21a dazu): {library_highest}");
}

/// Werk mit Inhalten in allen Tabellen mit uuid und Grabsteinen aller sechzehn Arten direkt in SQLite; Rückgabe: id
/// des Werks.
async fn insert_rows(pool: &SqlitePool) -> i64 {
    exec(pool, "INSERT INTO store_meta (key, value) VALUES ('seeded', '1') ON CONFLICT DO NOTHING").await;
    let doc = baruch_typst::parse(BOOK).unwrap();
    let work: i64 = sqlx::query_scalar(
        "INSERT INTO work (slug, kind, title, author, template, source_name, source_sha256, title_key) \
         VALUES ('der-kahn-im-schilf', 'book', 'Der Kahn im Schilf', 'Ole Ried', ?, 'kahn.typ', 'sha-kahn', ?) \
         RETURNING id",
    )
    .bind(&doc.template)
    .bind(title_key("Der Kahn im Schilf"))
    .fetch_one(pool)
    .await
    .unwrap();
    exec(
        pool,
        &format!(
            "INSERT INTO work_template_revision (work_id, number, body, author_name, note) \
             VALUES ({work}, 1, '// Vorlage\n', 'Ole', 'Anfang')"
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
             VALUES (?, 1, ?, 'human', 'Ole', NULL, 'current', 'Import')",
        )
        .bind(id)
        .bind(&heading.body)
        .execute(pool)
        .await
        .unwrap();
        nodes.push(id);
    }
    let mut posts = Vec::new();
    for (position, (author, body)) in
        [("Autor", "Wie dicht ist das Schilf?"), ("@lektor", "Kritisch: „liegt still“ ist matt.")].iter().enumerate()
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
        "INSERT INTO chat_thread (work_id, created_by, title) VALUES (?, 'Autor', 'Vögel') RETURNING id",
    )
    .bind(work)
    .fetch_one(pool)
    .await
    .unwrap();
    let thread_post: i64 = sqlx::query_scalar(
        "INSERT INTO chat_thread_post (thread_id, position, author, body) VALUES (?, 0, 'Autor', 'Reiher?') RETURNING id",
    )
    .bind(thread)
    .fetch_one(pool)
    .await
    .unwrap();
    let asset: i64 = sqlx::query_scalar("INSERT INTO asset (work_id, name) VALUES (?, 'kahn.svg') RETURNING id")
        .bind(work)
        .fetch_one(pool)
        .await
        .unwrap();
    let svg = "<svg xmlns=\"http://www.w3.org/2000/svg\"><rect width=\"6\" height=\"1\"/></svg>";
    for sql in [
        format!(
            "INSERT INTO post_ref (post_id, position, node_seq, rev, quote) VALUES ({}, 0, 0, 1, 'Kahn')",
            posts[1]
        ),
        format!(
            "INSERT INTO post_version (post_id, body, valid_from, edited_by) \
             VALUES ({}, 'Wie dicht?', '2026-11-08T09:00:00.000Z', 'Autor')",
            posts[0]
        ),
        format!(
            "INSERT INTO asset_revision (asset_id, number, mime, encoding, content, bytes, sha256, caption, alt, \
             author_name, status) \
             VALUES ({asset}, 1, 'image/svg+xml', 'svg', '{svg}', {}, 'abc', 'Kahn', 'Kahn', 'Ole', 'current')",
            svg.len()
        ),
        format!(
            "INSERT INTO idea (work_id, node_id, note, origin_kind, origin_name) \
             VALUES ({work}, {}, 'Nebel über dem Wasser', 'human', 'Ole')",
            nodes[1]
        ),
        format!(
            "INSERT INTO idea (work_id, node_id, note, origin_kind, origin_name) \
             VALUES ({work}, NULL, 'Geruch von Moor', 'persona', '@lektor')"
        ),
        format!(
            "INSERT INTO cover_block (work_id, kind, x_mm, y_mm, width_mm, size_pt) \
             VALUES ({work}, 'front_title', 10, 20, 100, 24)"
        ),
        format!(
            "INSERT INTO finding (work_id, node_id, post_id, position, persona, kind, body, quote) \
             VALUES ({work}, {}, {}, 0, '@lektor', 'critical', '„liegt still“ ist matt.', 'liegt still')",
            nodes[0], posts[1]
        ),
        format!("INSERT INTO replace_batch (work_id, query, replacement, items) VALUES ({work}, 'Kahn', 'Boot', '[]')"),
    ] {
        exec(pool, &sql).await;
    }
    exec(
        pool,
        "INSERT INTO finding_status (finding_id, status, actor_kind, actor) \
         VALUES ((SELECT min(id) FROM finding), 'done', 'human', 'Autor')",
    )
    .await;
    let ideas: Vec<i64> = sqlx::query_scalar("SELECT id FROM idea WHERE work_id = ? ORDER BY id")
        .bind(work)
        .fetch_all(pool)
        .await
        .unwrap();
    exec(
        pool,
        &format!(
            "INSERT INTO idea_link (from_id, to_id, kind, origin_kind, origin_name) \
             VALUES ({}, {}, 'leads_to', 'human', 'Ole')",
            ideas[0], ideas[1]
        ),
    )
    .await;
    exec(pool, &format!("INSERT INTO idea_map_pos (work_id, idea_id, x, y) VALUES ({work}, {}, 4, 2.5)", ideas[1]))
        .await;
    assert!(thread_post > 0, "Fadenbeitrag angelegt");
    // Grabsteine aller sechzehn Arten (wie nach früheren Löschungen; erfundene uuids).
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

/// Alle Schemaobjekte: Name → (Typ, Tabelle, SQL).
async fn schema(pool: &SqlitePool) -> BTreeMap<String, (String, String, Option<String>)> {
    let rows =
        sqlx::query("SELECT name, type, tbl_name, sql FROM sqlite_master ORDER BY name").fetch_all(pool).await.unwrap();
    rows.into_iter().map(|r| (r.get(0), (r.get(1), r.get(2), r.get(3)))).collect()
}

/// Inhalt jeder Tabelle in den Spalten aus `layout`, Werte per `quote()` (Typ und Bytes sichtbar).
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
async fn migration_0048_keeps_every_row_and_tombstone_and_adds_work_typeset() {
    let dir = tempfile::tempdir().unwrap();
    let pool = raw_pool(dir.path()).await;
    migrate_before(&pool).await;
    let work = insert_rows(&pool).await;
    let layout = tables(&pool).await;
    let schema_before = schema(&pool).await;
    let before = contents(&pool, &layout).await;
    assert_eq!(scalar(&pool, "SELECT count(DISTINCT kind) FROM uuid_tombstone").await, 16, "alle sechzehn Arten");
    let refused =
        sqlx::query("INSERT INTO uuid_tombstone (uuid, kind) VALUES ('x', 'work_typeset')").execute(&pool).await;
    assert!(refused.is_err(), "vor 0048 kennt der CHECK die neue Art nicht (sonst prüfte dieser Test nichts)");
    // Auf die umgebaute Tabelle zeigt kein Fremdschlüssel: Begründung, warum 0048 ohne FOREIGN_KEYS_OFF auskommt.
    let pointing = scalar(
        &pool,
        "SELECT count(*) FROM sqlite_master m, pragma_foreign_key_list(m.name) f \
         WHERE m.type = 'table' AND f.\"table\" = 'uuid_tombstone'",
    )
    .await;
    assert_eq!(pointing, 0, "kein Fremdschlüssel auf uuid_tombstone");

    // Nur 0048, wie `baruch_store::migrate` sie ausführt (nicht in FOREIGN_KEYS_OFF: Fremdschlüssel bleiben an).
    store_migrator(|v| v == 48).run(&pool).await.expect("0048 läuft auf dem Stand von development");

    // Daten: jede Zeile jeder alten Tabelle byte-gleich (ids, uuids, alle Grabsteine samt Zeit und Werk-uuid).
    let after = contents(&pool, &layout).await;
    for (table, old) in &before {
        if table != "_sqlx_migrations" {
            assert_eq!(after.get(table), Some(old), "Tabelle {table}: alle Zeilen unverändert");
        }
    }
    let layout_after = tables(&pool).await;
    for (table, cols) in &layout {
        assert_eq!(&layout_after[table], cols, "{table}: Spalten unverändert");
    }

    // Schema: alte Objekte wörtlich bis auf den CHECK von uuid_tombstone, neu genau die Objekte von work_typeset.
    let schema_after = schema(&pool).await;
    let added: BTreeSet<String> = schema_after.keys().filter(|k| !schema_before.contains_key(*k)).cloned().collect();
    let expected: BTreeSet<String> = NEW_OBJECTS.iter().map(|s| s.to_string()).collect();
    assert_eq!(added, expected, "neu sind genau Tabelle, Index und Trigger von work_typeset");
    for (name, (kind, table, sql)) in &schema_before {
        let (kind_now, table_now, sql_now) = schema_after.get(name).unwrap_or_else(|| panic!("{name} verschwunden"));
        assert_eq!((kind_now, table_now), (kind, table), "{name}: Typ und Tabelle unverändert");
        let mut now = sql_now.clone().unwrap_or_default();
        if name == "uuid_tombstone" {
            assert!(now.contains(TOMBSTONE_KIND), "Zusatz aus 0048 steht im CHECK: {now}");
            now = now.replacen(TOMBSTONE_KIND, "", 1);
        }
        // Auch die sechzehn Grabstein-Trigger: wörtlich wie vorher.
        assert_eq!(Some(now), sql.clone().or(Some(String::new())), "Schema von {name} unverändert");
    }
    for (_, trigger) in KINDS {
        assert!(schema_after.contains_key(trigger), "Grabstein-Trigger {trigger} steht wieder da");
    }
    assert!(schema_after.contains_key("uuid_tombstone_work"), "Index der Grabsteine neu angelegt");

    // CHECK von uuid_tombstone: die neue Art erlaubt, Erfundenes weiter nicht.
    exec(&pool, "INSERT INTO uuid_tombstone (uuid, kind) VALUES ('probe-satz', 'work_typeset')").await;
    assert!(
        rejected(&pool, "INSERT INTO uuid_tombstone (uuid, kind) VALUES ('probe-x', 'satz')").await,
        "CHECK lehnt eine erfundene Art weiter ab (satz ist nur das XML-Wort)"
    );
    exec(&pool, "DELETE FROM uuid_tombstone WHERE uuid LIKE 'probe-%'").await;

    // work_typeset: gültige Zeile geht; verbotene Werte nicht.
    exec(&pool, &format!("INSERT INTO work_typeset (work_id, settings) VALUES ({work}, '{{\"justify\":true}}')")).await;
    let (uuid, updated, by): (String, String, String) =
        sqlx::query_as("SELECT uuid, updated, updated_by FROM work_typeset WHERE work_id = ?")
            .bind(work)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(uuid.len() == 36 && uuid.as_bytes()[14] == b'4' && uuid == uuid.to_lowercase(), "v4-uuid: {uuid}");
    assert!(updated.ends_with('Z') && updated.len() == 24, "updated mit Millisekunden in UTC: {updated}");
    assert_eq!(by, "", "updated_by ist ohne Angabe leer");
    let other: i64 = sqlx::query_scalar(
        "INSERT INTO work (slug, kind, title, author, template, title_key) \
         VALUES ('leer', 'book', 'Leer', '', '', 'leer') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    for (bad, why) in [
        (
            format!("INSERT INTO work_typeset (work_id, settings) VALUES ({work}, '{{}}')"),
            "höchstens eine Zeile je Werk",
        ),
        (format!("INSERT INTO work_typeset (work_id, settings) VALUES ({other}, 'kaputt')"), "settings ist JSON"),
        (format!("INSERT INTO work_typeset (work_id, settings) VALUES ({other}, '[]')"), "settings ist ein Objekt"),
        (
            format!(
                "INSERT INTO work_typeset (work_id, settings) VALUES ({other}, '{{\"font\":\"{}\"}}')",
                "x".repeat(16400)
            ),
            "settings höchstens 16384 Zeichen",
        ),
        (
            format!(
                "INSERT INTO work_typeset (work_id, settings, updated_by) VALUES ({other}, '{{}}', '{}')",
                "x".repeat(201)
            ),
            "updated_by höchstens 200 Zeichen",
        ),
        ("INSERT INTO work_typeset (work_id, settings) VALUES (99999, '{}')".to_owned(), "Werk muss es geben"),
        (
            format!("INSERT INTO work_typeset (work_id, settings, uuid) VALUES ({other}, '{{}}', 'ABC')"),
            "uuid kanonisch klein mit 36 Zeichen",
        ),
        (
            format!("INSERT INTO work_typeset (work_id, settings, uuid) VALUES ({other}, '{{}}', '{uuid}')"),
            "uuid eindeutig",
        ),
    ] {
        assert!(rejected(&pool, &bad).await, "work_typeset lehnt ab: {why}");
    }

    // Grabstein beim Einzellöschen (Zurücksetzen) mit Werk-uuid.
    let work_uuid = text(&pool, &format!("SELECT uuid FROM work WHERE id = {work}")).await;
    exec(&pool, &format!("DELETE FROM work_typeset WHERE work_id = {work}")).await;
    let grave: (String, Option<String>) = sqlx::query_as("SELECT kind, work_uuid FROM uuid_tombstone WHERE uuid = ?")
        .bind(&uuid)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(grave, ("work_typeset".into(), Some(work_uuid.clone())), "Grabstein work_typeset mit Werk-uuid");
    // Ein leeres Werk mit Einstellungen löschen: Die Kaskade schreibt den Grabstein, solange das Werk noch steht.
    exec(&pool, &format!("INSERT INTO work_typeset (work_id, settings) VALUES ({other}, '{{}}')")).await;
    let other_uuid = text(&pool, &format!("SELECT uuid FROM work WHERE id = {other}")).await;
    let other_typeset = text(&pool, &format!("SELECT uuid FROM work_typeset WHERE work_id = {other}")).await;
    exec(&pool, &format!("DELETE FROM work WHERE id = {other}")).await;
    let cascaded: Option<String> = sqlx::query_scalar("SELECT work_uuid FROM uuid_tombstone WHERE uuid = ?")
        .bind(&other_typeset)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(cascaded, Some(other_uuid), "Grabstein der Kaskade kennt die Werk-uuid");

    assert_eq!(scalar(&pool, "SELECT count(*) FROM pragma_foreign_key_check").await, 0, "foreign_key_check leer");
    let integrity: String = sqlx::query_scalar("PRAGMA integrity_check").fetch_one(&pool).await.unwrap();
    assert_eq!(integrity, "ok", "integrity_check");
    pool.close().await;

    // Store::open danach; Löschen des Werks schreibt alle Arten von Grabsteinen (mit Werk-uuid), die neue eingeschlossen.
    let store = Store::open(dir.path()).await.expect("Store::open nach 0048");
    let pool = store.pool();
    exec(pool, &format!("INSERT INTO work_typeset (work_id, settings) VALUES ({work}, '{{\"lang\":\"de\"}}')")).await;
    exec(pool, "DELETE FROM idea WHERE note = 'Nebel über dem Wasser'").await; // Arten idea und idea_link (nur im Test)
    store.delete_work(work, "Der Kahn im Schilf").await.expect("Werk löschen schreibt Grabsteine");
    let fresh: Vec<(String, String, Option<String>)> = sqlx::query_as(
        "SELECT uuid, kind, work_uuid FROM uuid_tombstone WHERE uuid NOT LIKE '00000000-0000-4000-8000-%' \
         AND work_uuid = ? ORDER BY kind",
    )
    .bind(&work_uuid)
    .fetch_all(pool)
    .await
    .unwrap();
    let kinds: BTreeSet<&str> = fresh.iter().map(|(_, k, _)| k.as_str()).collect();
    for kind in KINDS.iter().map(|(k, _)| *k).chain(["work_typeset"]) {
        assert!(kinds.contains(kind), "nach 0048 schreibt der Trigger der Art {kind}: {kinds:?}");
    }
    assert_eq!(scalar(pool, "SELECT count(*) FROM work_typeset").await, 0, "work_typeset geht mit dem Werk");
}
