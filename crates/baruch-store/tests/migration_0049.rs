// SPDX-License-Identifier: AGPL-3.0-or-later
//! Migration 0049 (Glossar, eigene Register, Registervorschläge; Runde 31a, Vertrag §1) auf einer nachgebauten
//! Datenbank im Stand von development vor Runde 31 (Store bis 0048, Bibliothek vollständig) mit frei erfundenen Zeilen
//! in allen Tabellen mit uuid (Werk, Kapitel, Fassungen, Vorlagenfassung, Beiträge samt Verweis und alter Fassung,
//! Faden, Bild, Einfälle samt Verbindung und Lage, Umschlag-Block, Befunde samt Verlauf, Protokoll des Ersetzens,
//! Satz-Einstellungen) und Grabsteinen aller siebzehn Arten.
//!
//! Belegt: 0049 ist die einzige neue Store-Migration (0050–0052 unbenutzt) und steht nicht in `FOREIGN_KEYS_OFF`; auf
//! die umgebaute Tabelle `uuid_tombstone` zeigt kein Fremdschlüssel (darum löst das implizite DELETE von DROP TABLE
//! keine Aktion aus); jede Zeile jeder alten Tabelle bleibt byte-gleich samt ids, auch alle Grabsteine; im Schema
//! ändert sich nur der CHECK von `uuid_tombstone`, die siebzehn Grabstein-Trigger stehen wörtlich wieder da; neu sind
//! genau die Objekte von `glossary_term`, `glossary_variant`, `index_register` und `index_proposal`; die CHECKs lehnen
//! ab, was der Vertrag verbietet; Fülltrigger und Grabsteine (mit Werk-uuid) feuern beim Einzellöschen und beim Löschen
//! des Werks, und alle alten Grabstein-Trigger schreiben danach weiter.

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

/// Die siebzehn Grabstein-Arten vor 0049 (Wert in `uuid_tombstone.kind`) und der Name ihres Grabstein-Triggers.
const KINDS: [(&str, &str); 17] = [
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
    ("work_typeset", "work_typeset_uuid_tombstone"),
];

/// Zusatz, um den sich der CHECK von `uuid_tombstone` nach 0049 unterscheiden darf.
const TOMBSTONE_KIND: &str = ",\n                                          'glossary_term','index_register'";

/// Objekte, die 0049 neu anlegt (Tabellen, Indizes samt der automatischen für PRIMARY KEY/UNIQUE, Trigger).
const NEW_OBJECTS: [&str; 18] = [
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

async fn rejected(pool: &SqlitePool, sql: &str) -> bool {
    sqlx::query(sqlx::AssertSqlSafe(sql)).execute(pool).await.is_err()
}

/// Store bis 0048 (0040 wie in `baruch_store::migrate` mit ausgeschalteten Fremdschlüsseln) und die Bibliothek:
/// der Stand von development vor Runde 31.
async fn migrate_before(pool: &SqlitePool) {
    let versions: Vec<i64> = baruch_store::MIGRATOR.iter().map(|m| m.version).filter(|v| *v < 100).collect();
    // Runde 22: 0050 folgt (geprüft in migration_0050.rs); hier läuft nur 0049 auf den Stand davor.
    assert_eq!(
        versions.iter().copied().filter(|v| *v > 48 && *v < 50).collect::<Vec<_>>(),
        [49],
        "einzige neue Migration 0049 vor Runde 22"
    );
    assert!(!baruch_store::FOREIGN_KEYS_OFF.contains(&49), "0049 baut keine Tabelle mit Fremdschlüsseln um");
    store_migrator(|v| v < 40).run(pool).await.unwrap();
    {
        let mut conn = pool.acquire().await.unwrap();
        sqlx::query("PRAGMA foreign_keys = OFF").execute(&mut *conn).await.unwrap();
        store_migrator(|v| v == 40).run(&mut *conn).await.unwrap();
        sqlx::query("PRAGMA foreign_keys = ON").execute(&mut *conn).await.unwrap();
    }
    store_migrator(|v| (41..=48).contains(&v)).run(pool).await.unwrap();
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../baruch-library/migrations");
    let mut library = Migrator::new(dir).await.unwrap();
    library.ignore_missing = true;
    library.run(pool).await.unwrap();
    let highest = scalar(pool, "SELECT max(version) FROM _sqlx_migrations WHERE version < 100").await;
    assert_eq!(highest, 48, "Stand davor: Store bis 0048");
    let library_highest = scalar(pool, "SELECT max(version) FROM _sqlx_migrations WHERE version >= 100").await;
    assert!(library_highest >= 107, "Bibliothek mindestens bis 0107 (0108 kam in Runde 21a dazu): {library_highest}");
}

/// Werk mit Inhalten in allen Tabellen mit uuid (auch Satz-Einstellungen) und Grabsteinen aller siebzehn Arten direkt
/// in SQLite; Rückgabe: id des Werks.
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
    exec(
        pool,
        &format!(
            "INSERT INTO work_typeset (work_id, settings, updated_by) VALUES ({work}, '{{\"justify\":true}}', 'Ole')"
        ),
    )
    .await;
    assert!(thread_post > 0, "Fadenbeitrag angelegt");
    // Grabsteine aller siebzehn Arten (wie nach früheren Löschungen; erfundene uuids).
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
async fn migration_0049_keeps_every_row_and_tombstone_and_adds_the_glossary_tables() {
    let dir = tempfile::tempdir().unwrap();
    let pool = raw_pool(dir.path()).await;
    migrate_before(&pool).await;
    let work = insert_rows(&pool).await;
    let layout = tables(&pool).await;
    let schema_before = schema(&pool).await;
    let before = contents(&pool, &layout).await;
    assert_eq!(scalar(&pool, "SELECT count(DISTINCT kind) FROM uuid_tombstone").await, 17, "alle siebzehn Arten");
    let refused =
        sqlx::query("INSERT INTO uuid_tombstone (uuid, kind) VALUES ('x', 'glossary_term')").execute(&pool).await;
    assert!(refused.is_err(), "vor 0049 kennt der CHECK die neue Art nicht (sonst prüfte dieser Test nichts)");
    // Auf die umgebaute Tabelle zeigt kein Fremdschlüssel: Begründung, warum 0049 ohne FOREIGN_KEYS_OFF auskommt.
    let pointing = scalar(
        &pool,
        "SELECT count(*) FROM sqlite_master m, pragma_foreign_key_list(m.name) f \
         WHERE m.type = 'table' AND f.\"table\" = 'uuid_tombstone'",
    )
    .await;
    assert_eq!(pointing, 0, "kein Fremdschlüssel auf uuid_tombstone");

    // Nur 0049, wie `baruch_store::migrate` sie ausführt (nicht in FOREIGN_KEYS_OFF: Fremdschlüssel bleiben an).
    store_migrator(|v| v == 49).run(&pool).await.expect("0049 läuft auf dem Stand von development");

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

    // Schema: alte Objekte wörtlich bis auf den CHECK von uuid_tombstone, neu genau die Objekte der vier Tabellen.
    let schema_after = schema(&pool).await;
    let added: BTreeSet<String> = schema_after.keys().filter(|k| !schema_before.contains_key(*k)).cloned().collect();
    let expected: BTreeSet<String> = NEW_OBJECTS.iter().map(|s| s.to_string()).collect();
    assert_eq!(added, expected, "neu sind genau Tabellen, Indizes und Trigger von Glossar und Registern");
    for (name, (kind, table, sql)) in &schema_before {
        let (kind_now, table_now, sql_now) = schema_after.get(name).unwrap_or_else(|| panic!("{name} verschwunden"));
        assert_eq!((kind_now, table_now), (kind, table), "{name}: Typ und Tabelle unverändert");
        let mut now = sql_now.clone().unwrap_or_default();
        if name == "uuid_tombstone" {
            assert!(now.contains(TOMBSTONE_KIND), "Zusatz aus 0049 steht im CHECK: {now}");
            now = now.replacen(TOMBSTONE_KIND, "", 1);
        }
        // Auch die siebzehn Grabstein-Trigger: wörtlich wie vorher.
        assert_eq!(Some(now), sql.clone().or(Some(String::new())), "Schema von {name} unverändert");
    }
    for (_, trigger) in KINDS {
        assert!(schema_after.contains_key(trigger), "Grabstein-Trigger {trigger} steht wieder da");
    }
    assert!(schema_after.contains_key("uuid_tombstone_work"), "Index der Grabsteine neu angelegt");

    // CHECK von uuid_tombstone: die neuen Arten erlaubt, Erfundenes (auch die XML-Wörter) weiter nicht.
    exec(&pool, "INSERT INTO uuid_tombstone (uuid, kind) VALUES ('probe-gl', 'glossary_term')").await;
    exec(&pool, "INSERT INTO uuid_tombstone (uuid, kind) VALUES ('probe-reg', 'index_register')").await;
    for word in ["glossar-begriff", "register", "index_proposal"] {
        assert!(
            rejected(&pool, &format!("INSERT INTO uuid_tombstone (uuid, kind) VALUES ('probe-x', '{word}')")).await,
            "CHECK lehnt die Art {word} ab (XML-Wort bzw. Vorschläge ohne uuid)"
        );
    }
    exec(&pool, "DELETE FROM uuid_tombstone WHERE uuid LIKE 'probe-%'").await;

    // glossary_term: gültige Zeile geht, Fülltrigger setzt eine v4-uuid; verbotene Werte nicht.
    exec(
        &pool,
        &format!(
            "INSERT INTO glossary_term (work_id, term, term_key, short, bible_refs, status, origin) \
             VALUES ({work}, 'Schilf', 'schilf', 'Röhricht am Ufer', '[\"Hi 8,11\"]', 'accepted', 'Ole')"
        ),
    )
    .await;
    let (term_id, uuid, created, updated_by): (i64, String, String, String) =
        sqlx::query_as("SELECT id, uuid, created, updated_by FROM glossary_term WHERE term_key = 'schilf'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(uuid.len() == 36 && uuid.as_bytes()[14] == b'4' && uuid == uuid.to_lowercase(), "v4-uuid: {uuid}");
    assert!(created.ends_with('Z') && created.len() == 24, "created mit Millisekunden in UTC: {created}");
    assert_eq!(updated_by, "", "updated_by ist ohne Angabe leer");
    exec(
        &pool,
        &format!(
            "INSERT INTO glossary_variant (term_id, position, form, form_key) VALUES ({term_id}, 0, 'Schilfes', \
             'schilfes')"
        ),
    )
    .await;
    // Ein Vorschlag mit demselben Schlüssel ist erlaubt (Teilindex nur über angenommene Begriffe).
    exec(
        &pool,
        &format!(
            "INSERT INTO glossary_term (work_id, term, term_key, status, origin) \
             VALUES ({work}, 'schilf', 'schilf', 'proposed', '@lektor')"
        ),
    )
    .await;
    for (bad, why) in [
        (
            format!(
                "INSERT INTO glossary_term (work_id, term, term_key, status, origin) VALUES ({work}, 'SCHILF', \
                 'schilf', 'accepted', 'Ole')"
            ),
            "term_key unter angenommenen Begriffen eines Werks eindeutig",
        ),
        (
            format!(
                "INSERT INTO glossary_term (work_id, term, term_key, status, origin) VALUES ({work}, 'Ried', 'ried', \
                 'offen', 'Ole')"
            ),
            "status nur accepted oder proposed",
        ),
        (
            format!(
                "INSERT INTO glossary_term (work_id, term, term_key, bible_refs, status, origin) VALUES ({work}, \
                 'Ried', 'ried', '{{}}', 'accepted', 'Ole')"
            ),
            "bible_refs ist eine JSON-Liste",
        ),
        (
            format!(
                "INSERT INTO glossary_term (work_id, term, term_key, bible_refs, status, origin) VALUES ({work}, \
                 'Ried', 'ried', 'kaputt', 'accepted', 'Ole')"
            ),
            "bible_refs ist JSON",
        ),
        (
            format!(
                "INSERT INTO glossary_term (work_id, term, term_key, status, origin) VALUES ({work}, '  ', ' ', \
                 'accepted', 'Ole')"
            ),
            "term nicht leer",
        ),
        (
            format!(
                "INSERT INTO glossary_term (work_id, term, term_key, status, origin) VALUES ({work}, '{}', 'x', \
                 'accepted', 'Ole')",
                "x".repeat(201)
            ),
            "term höchstens 200 Zeichen",
        ),
        (
            "INSERT INTO glossary_term (work_id, term, term_key, status, origin) VALUES (99999, 'Ried', 'ried', \
             'accepted', 'Ole')"
                .to_owned(),
            "Werk muss es geben",
        ),
        (
            format!(
                "INSERT INTO glossary_term (work_id, term, term_key, status, origin, uuid) VALUES ({work}, 'Ried', \
                 'ried', 'accepted', 'Ole', '{uuid}')"
            ),
            "uuid eindeutig",
        ),
        (
            format!(
                "INSERT INTO glossary_variant (term_id, position, form, form_key) VALUES ({term_id}, 0, 'Schilfs', \
                 'schilfs')"
            ),
            "eine Variante je Stelle",
        ),
        (
            "INSERT INTO glossary_variant (term_id, position, form, form_key) VALUES (99999, 0, 'a', 'a')".to_owned(),
            "Variante gehört zu einem Begriff",
        ),
    ] {
        assert!(rejected(&pool, &bad).await, "glossary_term/glossary_variant lehnt ab: {why}");
    }

    // index_register: eigene Kennungen ja, eingebaute nein, je Werk eindeutig.
    exec(
        &pool,
        &format!(
            "INSERT INTO index_register (work_id, slug, title, position) VALUES ({work}, 'orte', 'Ortsregister', 0)"
        ),
    )
    .await;
    let register_uuid = text(&pool, "SELECT uuid FROM index_register WHERE slug = 'orte'").await;
    assert_eq!(register_uuid.len(), 36, "Fülltrigger setzt die uuid des Registers: {register_uuid}");
    for slug in ["stichwort", "personen", "bibel", "orte"] {
        assert!(
            rejected(
                &pool,
                &format!(
                    "INSERT INTO index_register (work_id, slug, title, position) VALUES ({work}, '{slug}', 'X', 1)"
                )
            )
            .await,
            "index_register lehnt {slug} ab (eingebaut bzw. schon vorhanden)"
        );
    }

    // index_proposal: Status und mark geprüft; geht mit dem Abschnitt.
    let node: i64 = scalar(&pool, &format!("SELECT min(id) FROM node WHERE work_id = {work}")).await;
    exec(
        &pool,
        &format!(
            "INSERT INTO index_proposal (work_id, node_id, based_on, word, occurrence, mark, origin, status) \
             VALUES ({work}, {node}, 1, 'Kahn', 'first', '{{\"register\":\"stichwort\",\"term\":\"Kahn\"}}', \
             '@lektor', 'open')"
        ),
    )
    .await;
    for (bad, why) in [
        (
            format!(
                "INSERT INTO index_proposal (work_id, node_id, based_on, word, occurrence, mark, origin, status) \
                 VALUES ({work}, {node}, 1, 'Kahn', 'first', '{{}}', '@lektor', 'neu')"
            ),
            "status nur open/accepted/rejected/stale",
        ),
        (
            format!(
                "INSERT INTO index_proposal (work_id, node_id, based_on, word, occurrence, mark, origin, status) \
                 VALUES ({work}, {node}, 1, 'Kahn', 'first', '[]', '@lektor', 'open')"
            ),
            "mark ist ein JSON-Objekt",
        ),
        (
            format!(
                "INSERT INTO index_proposal (work_id, node_id, based_on, word, occurrence, mark, origin, status) \
                 VALUES ({work}, 99999, 1, 'Kahn', 'first', '{{}}', '@lektor', 'open')"
            ),
            "Abschnitt muss es geben",
        ),
    ] {
        assert!(rejected(&pool, &bad).await, "index_proposal lehnt ab: {why}");
    }

    // Grabsteine beim Einzellöschen mit Werk-uuid; die Varianten gehen mit dem Begriff.
    let work_uuid = text(&pool, &format!("SELECT uuid FROM work WHERE id = {work}")).await;
    exec(&pool, &format!("DELETE FROM glossary_term WHERE id = {term_id}")).await;
    let grave: (String, Option<String>) = sqlx::query_as("SELECT kind, work_uuid FROM uuid_tombstone WHERE uuid = ?")
        .bind(&uuid)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(grave, ("glossary_term".into(), Some(work_uuid.clone())), "Grabstein glossary_term mit Werk-uuid");
    assert_eq!(scalar(&pool, "SELECT count(*) FROM glossary_variant").await, 0, "Varianten gehen mit dem Begriff");
    exec(&pool, "DELETE FROM index_register WHERE slug = 'orte'").await;
    let grave: (String, Option<String>) = sqlx::query_as("SELECT kind, work_uuid FROM uuid_tombstone WHERE uuid = ?")
        .bind(&register_uuid)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(grave, ("index_register".into(), Some(work_uuid.clone())), "Grabstein index_register mit Werk-uuid");

    // Ein leeres Werk mit Begriff und Register löschen: Die Kaskade schreibt die Grabsteine, solange das Werk steht.
    let other: i64 = sqlx::query_scalar(
        "INSERT INTO work (slug, kind, title, author, template, title_key) \
         VALUES ('leer', 'book', 'Leer', '', '', 'leer') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    exec(
        &pool,
        &format!(
            "INSERT INTO glossary_term (work_id, term, term_key, status, origin) VALUES ({other}, 'Moor', 'moor', \
             'accepted', 'Ole')"
        ),
    )
    .await;
    exec(
        &pool,
        &format!("INSERT INTO index_register (work_id, slug, title, position) VALUES ({other}, 'tiere', 'Tiere', 0)"),
    )
    .await;
    let other_uuid = text(&pool, &format!("SELECT uuid FROM work WHERE id = {other}")).await;
    let other_term = text(&pool, &format!("SELECT uuid FROM glossary_term WHERE work_id = {other}")).await;
    let other_register = text(&pool, &format!("SELECT uuid FROM index_register WHERE work_id = {other}")).await;
    exec(&pool, &format!("DELETE FROM work WHERE id = {other}")).await;
    for (grave, what) in [(&other_term, "Begriff"), (&other_register, "Register")] {
        let cascaded: Option<String> = sqlx::query_scalar("SELECT work_uuid FROM uuid_tombstone WHERE uuid = ?")
            .bind(grave)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(
            cascaded.as_deref(),
            Some(other_uuid.as_str()),
            "Grabstein der Kaskade ({what}) kennt die Werk-uuid"
        );
    }

    assert_eq!(scalar(&pool, "SELECT count(*) FROM pragma_foreign_key_check").await, 0, "foreign_key_check leer");
    let integrity: String = sqlx::query_scalar("PRAGMA integrity_check").fetch_one(&pool).await.unwrap();
    assert_eq!(integrity, "ok", "integrity_check");
    pool.close().await;

    // Store::open danach; Löschen des Werks schreibt alle Arten von Grabsteinen (mit Werk-uuid), die neuen eingeschlossen.
    let store = Store::open(dir.path()).await.expect("Store::open nach 0049");
    let pool = store.pool();
    exec(
        pool,
        &format!(
            "INSERT INTO glossary_term (work_id, term, term_key, status, origin) VALUES ({work}, 'Reiher', 'reiher', \
             'accepted', 'Ole')"
        ),
    )
    .await;
    exec(
        pool,
        &format!("INSERT INTO index_register (work_id, slug, title, position) VALUES ({work}, 'voegel', 'Vögel', 0)"),
    )
    .await;
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
    for kind in KINDS.iter().map(|(k, _)| *k).chain(["glossary_term", "index_register"]) {
        assert!(kinds.contains(kind), "nach 0049 schreibt der Trigger der Art {kind}: {kinds:?}");
    }
    for table in ["glossary_term", "glossary_variant", "index_register", "index_proposal"] {
        assert_eq!(scalar(pool, &format!("SELECT count(*) FROM {table}")).await, 0, "{table} geht mit dem Werk");
    }
}
