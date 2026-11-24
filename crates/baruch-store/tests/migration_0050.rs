// SPDX-License-Identifier: AGPL-3.0-or-later
//! Migration 0050 (Fortschreiben der aktuellen Fassung, Einstellung „Fassungen“, Stand übernommener Befunde;
//! Runde 22, Vertrag §1/§3.1) auf einer nachgebauten Datenbank im Stand von development vor Runde 22 (Store bis 0049,
//! Bibliothek vollständig) mit frei erfundenen Zeilen in allen Tabellen mit uuid – darunter die drei Tabellen, die 0050
//! um Spalten erweitert (`revision`, `finding`, `finding_status`) – und Grabsteinen aller neunzehn bisherigen Arten.
//!
//! Hinweis: Der Vertrag spricht von „21 bisherigen Arten“ und „20 Grabstein-Triggern“; im CHECK von 0049 stehen
//! tatsächlich neunzehn Arten mit je einem Grabstein-Trigger, nach 0050 sind es zwanzig (mit `revision_amend`). Der Test
//! prüft die tatsächlichen Zahlen.
//!
//! Belegt: 0050 ist die einzige neue Store-Migration (0051–0054 reserviert, unbenutzt) und steht nicht in
//! `FOREIGN_KEYS_OFF`; auf `uuid_tombstone` zeigt kein Fremdschlüssel; jede Zeile jeder alten Tabelle bleibt in ihren alten
//! Spalten byte-gleich samt ids, auch alle Grabsteine; die neuen Spalten tragen ihre Vorgaben (`amend_seq` 0, die übrigen
//! NULL); im Schema ändern sich nur der CHECK von `uuid_tombstone` und die drei erweiterten Tabellen (um genau die neuen
//! Spalten), die neunzehn Grabstein-Trigger stehen wörtlich wieder da; neu sind genau die Objekte von `revision_amend`
//! und `work_revision_policy`; die CHECKs lehnen ab, was der Vertrag verbietet; Fülltrigger, Grabstein (mit Werk-uuid)
//! und Kaskade feuern beim Löschen einer Fassung und beim Löschen des Werks, alle alten Grabstein-Trigger danach weiter.

use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::str::FromStr;

use baruch_store::{RevisionPolicy, SaveMode, Store, title_key};
use sqlx::migrate::Migrator;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};

const BOOK: &str = "#set document(title: \"Die Fähre im Nebel\", author: \"Mira Holt\")\n\n\
= Morgen\n\nDie Fähre legt im Nebel ab.\n\n= Abend\n\nDie Möwen kehren heim.\n";

/// Die neunzehn Grabstein-Arten vor 0050 (Wert in `uuid_tombstone.kind`) und der Name ihres Grabstein-Triggers.
const KINDS: [(&str, &str); 19] = [
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
    ("glossary_term", "glossary_term_uuid_tombstone"),
    ("index_register", "index_register_uuid_tombstone"),
];

/// Zusatz, um den sich der CHECK von `uuid_tombstone` nach 0050 unterscheiden darf.
const TOMBSTONE_KIND: &str = ",\n                                          'revision_amend'";

/// Objekte, die 0050 neu anlegt (Tabellen, Indizes samt der automatischen für UNIQUE, Trigger).
const NEW_OBJECTS: [&str; 7] = [
    "revision_amend",
    "sqlite_autoindex_revision_amend_1",
    "revision_amend_uuid",
    "revision_amend_uuid_fill",
    "revision_amend_uuid_tombstone",
    "revision_amend_before_revision_delete",
    "work_revision_policy",
];

/// Spalten, die 0050 an bestehende Tabellen hängt (Tabelle, Spalte, Vorgabe per `quote()`, Definition wörtlich).
const NEW_COLUMNS: [(&str, &str, &str, &str); 4] = [
    ("revision", "amend_seq", "0", "amend_seq INTEGER NOT NULL DEFAULT 0 CHECK (amend_seq >= 0)"),
    (
        "revision",
        "based_on_amend",
        "NULL",
        "based_on_amend INTEGER CHECK (based_on_amend IS NULL OR based_on_amend >= 0)",
    ),
    ("finding", "adopted_amend", "NULL", "adopted_amend INTEGER CHECK (adopted_amend IS NULL OR adopted_amend >= 0)"),
    (
        "finding_status",
        "adopted_amend",
        "NULL",
        "adopted_amend INTEGER CHECK (adopted_amend IS NULL OR adopted_amend >= 0)",
    ),
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

/// Store bis 0048 (0040 wie in `baruch_store::migrate` mit ausgeschalteten Fremdschlüsseln) und die Bibliothek:
/// der Stand von development vor Runde 31.
/// Store bis 0049 (0040 wie in `baruch_store::migrate` mit ausgeschalteten Fremdschlüsseln) und die Bibliothek:
/// der Stand von development vor Runde 22.
async fn migrate_before(pool: &SqlitePool) {
    let versions: Vec<i64> = baruch_store::MIGRATOR.iter().map(|m| m.version).filter(|v| *v < 100).collect();
    assert_eq!(
        versions.iter().copied().filter(|v| *v > 49).collect::<Vec<_>>(),
        [50],
        "einzige neue Migration 0050 (0051–0054 reserviert, unbenutzt)"
    );
    assert!(!baruch_store::FOREIGN_KEYS_OFF.contains(&50), "0050 baut keine Tabelle mit Fremdschlüsseln um");
    store_migrator(|v| v < 40).run(pool).await.unwrap();
    {
        let mut conn = pool.acquire().await.unwrap();
        sqlx::query("PRAGMA foreign_keys = OFF").execute(&mut *conn).await.unwrap();
        store_migrator(|v| v == 40).run(&mut *conn).await.unwrap();
        sqlx::query("PRAGMA foreign_keys = ON").execute(&mut *conn).await.unwrap();
    }
    store_migrator(|v| (41..=49).contains(&v)).run(pool).await.unwrap();
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../baruch-library/migrations");
    let mut library = Migrator::new(dir).await.unwrap();
    library.ignore_missing = true;
    library.run(pool).await.unwrap();
    let highest = scalar(pool, "SELECT max(version) FROM _sqlx_migrations WHERE version < 100").await;
    assert_eq!(highest, 49, "Stand davor: Store bis 0049");
}

/// Werk mit Inhalten in allen Tabellen mit uuid (auch Satz-Einstellungen, Glossar, Register, Vorschlag, Befund samt
/// Verlauf) und Grabsteinen aller neunzehn Arten direkt in SQLite; Rückgabe: id des Werks.
async fn insert_rows(pool: &SqlitePool) -> i64 {
    exec(pool, "INSERT INTO store_meta (key, value) VALUES ('seeded', '1') ON CONFLICT DO NOTHING").await;
    let doc = baruch_typst::parse(BOOK).unwrap();
    let work: i64 = sqlx::query_scalar(
        "INSERT INTO work (slug, kind, title, author, template, source_name, source_sha256, title_key) \
         VALUES ('die-faehre-im-nebel', 'book', 'Die Fähre im Nebel', 'Mira Ried', ?, 'faehre.typ', 'sha-faehre', ?) \
         RETURNING id",
    )
    .bind(&doc.template)
    .bind(title_key("Die Fähre im Nebel"))
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
    for (position, (author, body)) in
        [("Autor", "Wie dicht ist der Nebel?"), ("@lektor", "Kritisch: „legt ab“ ist matt.")].iter().enumerate()
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
        "INSERT INTO chat_thread (work_id, created_by, title) VALUES (?, 'Autor', 'Möwen') RETURNING id",
    )
    .bind(work)
    .fetch_one(pool)
    .await
    .unwrap();
    let thread_post: i64 = sqlx::query_scalar(
        "INSERT INTO chat_thread_post (thread_id, position, author, body) VALUES (?, 0, 'Autor', 'Möwen?') RETURNING id",
    )
    .bind(thread)
    .fetch_one(pool)
    .await
    .unwrap();
    let asset: i64 = sqlx::query_scalar("INSERT INTO asset (work_id, name) VALUES (?, 'faehre.svg') RETURNING id")
        .bind(work)
        .fetch_one(pool)
        .await
        .unwrap();
    let svg = "<svg xmlns=\"http://www.w3.org/2000/svg\"><rect width=\"6\" height=\"1\"/></svg>";
    for sql in [
        format!(
            "INSERT INTO post_ref (post_id, position, node_seq, rev, quote) VALUES ({}, 0, 0, 1, 'Fähre')",
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
             VALUES ({asset}, 1, 'image/svg+xml', 'svg', '{svg}', {}, 'abc', 'Fähre', 'Fähre', 'Mira', 'current')",
            svg.len()
        ),
        format!(
            "INSERT INTO idea (work_id, node_id, note, origin_kind, origin_name) \
             VALUES ({work}, {}, 'Nebel über dem Wasser', 'human', 'Mira')",
            nodes[1]
        ),
        format!(
            "INSERT INTO idea (work_id, node_id, note, origin_kind, origin_name) \
             VALUES ({work}, NULL, 'Geruch von Salz', 'persona', '@lektor')"
        ),
        format!(
            "INSERT INTO cover_block (work_id, kind, x_mm, y_mm, width_mm, size_pt) \
             VALUES ({work}, 'front_title', 10, 20, 100, 24)"
        ),
        format!(
            "INSERT INTO finding (work_id, node_id, post_id, position, persona, kind, body, quote) \
             VALUES ({work}, {}, {}, 0, '@lektor', 'critical', '„legt ab“ ist matt.', 'legt ab')",
            nodes[0], posts[1]
        ),
        format!(
            "INSERT INTO replace_batch (work_id, query, replacement, items) VALUES ({work}, 'Fähre', 'Schiff', '[]')"
        ),
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
             VALUES ({}, {}, 'leads_to', 'human', 'Mira')",
            ideas[0], ideas[1]
        ),
    )
    .await;
    exec(pool, &format!("INSERT INTO idea_map_pos (work_id, idea_id, x, y) VALUES ({work}, {}, 4, 2.5)", ideas[1]))
        .await;
    exec(
        pool,
        &format!(
            "INSERT INTO work_typeset (work_id, settings, updated_by) VALUES ({work}, '{{\"justify\":true}}', 'Mira')"
        ),
    )
    .await;
    assert!(thread_post > 0, "Fadenbeitrag angelegt");
    // Runde 22 berührt revision: ein Vorschlag 2 einer Persona auf Fassung 1 (Fassung 1 bleibt aktuell).
    exec(
        pool,
        &format!(
            "INSERT INTO revision (node_id, number, body, author_kind, author_name, model, status, note, based_on) \
             VALUES ({}, 2, '= Morgen\n\nDie Fähre schaukelt im Nebel.\n', 'assistant', '@lektor', 'mock', \
             'proposed', 'Vorschlag', 1)",
            nodes[0]
        ),
    )
    .await;
    // Glossar und Register (Arten glossary_term, index_register aus 0049).
    exec(
        pool,
        &format!(
            "INSERT INTO glossary_term (work_id, term, term_key, status, origin) \
             VALUES ({work}, 'Nebel', 'nebel', 'accepted', 'Mira')"
        ),
    )
    .await;
    exec(
        pool,
        &format!("INSERT INTO index_register (work_id, slug, title, position) VALUES ({work}, 'orte', 'Orte', 0)"),
    )
    .await;
    // Grabsteine aller neunzehn Arten (wie nach früheren Löschungen; erfundene uuids).
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
async fn migration_0050_keeps_every_row_and_tombstone_and_adds_the_amend_history() {
    let dir = tempfile::tempdir().unwrap();
    let pool = raw_pool(dir.path()).await;
    migrate_before(&pool).await;
    let work = insert_rows(&pool).await;
    let layout = tables(&pool).await;
    let schema_before = schema(&pool).await;
    let before = contents(&pool, &layout).await;
    assert_eq!(scalar(&pool, "SELECT count(DISTINCT kind) FROM uuid_tombstone").await, 19, "alle neunzehn Arten");
    assert_eq!(scalar(&pool, "SELECT count(*) FROM revision").await, 3, "drei Fassungen (zwei aktuell, ein Vorschlag)");
    assert_eq!(scalar(&pool, "SELECT count(*) FROM finding_status").await, 1, "ein Statuseintrag eines Befunds");
    let refused =
        sqlx::query("INSERT INTO uuid_tombstone (uuid, kind) VALUES ('x', 'revision_amend')").execute(&pool).await;
    assert!(refused.is_err(), "vor 0050 kennt der CHECK die neue Art nicht (sonst prüfte dieser Test nichts)");
    let pointing = scalar(
        &pool,
        "SELECT count(*) FROM sqlite_master m, pragma_foreign_key_list(m.name) f \
         WHERE m.type = 'table' AND f.\"table\" = 'uuid_tombstone'",
    )
    .await;
    assert_eq!(pointing, 0, "kein Fremdschlüssel auf uuid_tombstone: darum reicht 0050 ohne FOREIGN_KEYS_OFF");

    // Nur 0050, wie `baruch_store::migrate` sie ausführt (nicht in FOREIGN_KEYS_OFF: Fremdschlüssel bleiben an).
    store_migrator(|v| v == 50).run(&pool).await.expect("0050 läuft auf dem Stand von development");

    // Daten: jede Zeile jeder alten Tabelle in ihren alten Spalten byte-gleich (ids, uuids, alle Grabsteine).
    let after = contents(&pool, &layout).await;
    for (table, old) in &before {
        if table != "_sqlx_migrations" {
            assert_eq!(after.get(table), Some(old), "Tabelle {table}: alle Zeilen in den alten Spalten unverändert");
        }
    }
    // Neue Spalten: genau die vereinbarten, hinten angehängt, mit Vorgaben in jeder alten Zeile.
    let layout_after = tables(&pool).await;
    for (table, cols) in &layout {
        let mut expected: Vec<String> = cols.clone();
        expected.extend(NEW_COLUMNS.iter().filter(|(t, ..)| t == table).map(|(_, c, ..)| c.to_string()));
        assert_eq!(layout_after[table], expected, "{table}: alte Spalten in Reihenfolge, neue hinten");
    }
    for (table, column, default, _) in NEW_COLUMNS {
        let values: Vec<String> = sqlx::query_scalar(&format!("SELECT DISTINCT quote({column}) FROM {table}"))
            .fetch_all(&pool)
            .await
            .unwrap();
        assert_eq!(values, [default], "{table}.{column}: jede alte Zeile trägt die Vorgabe {default}");
    }

    // Schema: alte Objekte wörtlich bis auf den CHECK von uuid_tombstone und die angehängten Spalten.
    let schema_after = schema(&pool).await;
    let added: BTreeSet<String> = schema_after.keys().filter(|k| !schema_before.contains_key(*k)).cloned().collect();
    let expected: BTreeSet<String> = NEW_OBJECTS.iter().map(|s| s.to_string()).collect();
    assert_eq!(added, expected, "neu sind genau die Objekte von revision_amend und work_revision_policy");
    for (name, (kind, table, sql)) in &schema_before {
        let (kind_now, table_now, sql_now) = schema_after.get(name).unwrap_or_else(|| panic!("{name} verschwunden"));
        assert_eq!((kind_now, table_now), (kind, table), "{name}: Typ und Tabelle unverändert");
        let now = sql_now.clone().unwrap_or_default();
        let old = sql.clone().unwrap_or_default();
        if name == "uuid_tombstone" {
            assert!(now.contains(TOMBSTONE_KIND), "Zusatz aus 0050 steht im CHECK: {now}");
            assert_eq!(now.replacen(TOMBSTONE_KIND, "", 1), old, "uuid_tombstone sonst wörtlich gleich");
        } else if NEW_COLUMNS.iter().any(|(t, ..)| t == name) {
            // ALTER TABLE ADD COLUMN setzt „, <Definition>“ hinter die letzte Spalte; ohne sie ist der Text wörtlich alt.
            let mut stripped = now.clone();
            for (_, column, _, definition) in NEW_COLUMNS.iter().filter(|(t, ..)| t == name) {
                let clause = format!(", {definition}");
                assert!(stripped.contains(&clause), "{name}: Spalte {column} angehängt: {now}");
                stripped = stripped.replacen(&clause, "", 1);
            }
            assert_eq!(stripped, old, "{name}: bis auf die neuen Spalten wörtlich unverändert");
        } else {
            // Auch die neunzehn Grabstein-Trigger: wörtlich wie vorher.
            assert_eq!(now, old, "Schema von {name} unverändert");
        }
    }
    for (_, trigger) in KINDS {
        assert!(schema_after.contains_key(trigger), "Grabstein-Trigger {trigger} steht wieder da");
    }
    assert!(schema_after.contains_key("uuid_tombstone_work"), "Index der Grabsteine neu angelegt");

    // CHECKs: neue Art erlaubt, das XML-Wort nicht; revision_amend und work_revision_policy lehnen Verbotenes ab.
    exec(&pool, "INSERT INTO uuid_tombstone (uuid, kind) VALUES ('probe-am', 'revision_amend')").await;
    assert!(
        rejected(&pool, "INSERT INTO uuid_tombstone (uuid, kind) VALUES ('probe-x', 'fortschreibung')").await,
        "CHECK lehnt das XML-Wort „fortschreibung“ als Art ab (die DB-Art heißt revision_amend)"
    );
    exec(&pool, "DELETE FROM uuid_tombstone WHERE uuid LIKE 'probe-%'").await;
    let rev1: i64 = scalar(
        &pool,
        &format!(
            "SELECT r.id FROM revision r JOIN node n ON n.id = r.node_id \
             WHERE n.work_id = {work} AND n.seq = 0 AND r.number = 1"
        ),
    )
    .await;
    exec(
        &pool,
        &format!(
            "INSERT INTO revision_amend (revision_id, seq, kind, body_before, body_after, author_name) \
             VALUES ({rev1}, 1, 'amend', 'vorher', 'nachher', 'Mira')"
        ),
    )
    .await;
    let step_uuid = text(&pool, &format!("SELECT uuid FROM revision_amend WHERE revision_id = {rev1}")).await;
    assert!(step_uuid.len() == 36 && step_uuid.as_bytes()[14] == b'4', "Fülltrigger setzt eine v4-uuid: {step_uuid}");
    let insert = "INSERT INTO revision_amend (revision_id, seq, kind, undo_of, body_before, body_after, author_name)";
    for (bad, why) in [
        (format!("{insert} VALUES ({rev1}, 1, 'amend', NULL, 'a', 'b', 'M')"), "seq je Fassung eindeutig"),
        (format!("{insert} VALUES ({rev1}, 0, 'amend', NULL, 'a', 'b', 'M')"), "seq ab 1"),
        (format!("{insert} VALUES ({rev1}, 2, 'edit', NULL, 'a', 'b', 'M')"), "Art nur amend oder undo"),
        (format!("{insert} VALUES ({rev1}, 2, 'undo', NULL, 'a', 'b', 'M')"), "undo braucht undo_of"),
        (format!("{insert} VALUES ({rev1}, 2, 'amend', 1, 'a', 'b', 'M')"), "amend ohne undo_of"),
        (format!("{insert} VALUES ({rev1}, 2, 'amend', NULL, 'a', 'b', '')"), "Autor nicht leer"),
        (format!("{insert} VALUES (99999, 1, 'amend', NULL, 'a', 'b', 'M')"), "Fassung muss es geben"),
        (format!("UPDATE revision SET amend_seq = -1 WHERE id = {rev1}"), "amend_seq nicht negativ"),
        (format!("INSERT INTO work_revision_policy (work_id, max_change_percent) VALUES ({work}, 0)"), "ab 1 %"),
        (format!("INSERT INTO work_revision_policy (work_id, new_after_minutes) VALUES ({work}, 1441)"), "≤ 1440 min"),
        (format!("INSERT INTO work_revision_policy (work_id, amend) VALUES ({work}, 2)"), "amend nur 0 oder 1"),
        ("INSERT INTO work_revision_policy (work_id) VALUES (99999)".to_owned(), "Werk muss es geben"),
    ] {
        assert!(rejected(&pool, &bad).await, "CHECK/Fremdschlüssel lehnt ab: {why}");
    }
    exec(&pool, &format!("INSERT INTO work_revision_policy (work_id) VALUES ({work})")).await;
    let policy: (i64, i64, i64) = sqlx::query_as(
        "SELECT amend, max_change_percent, new_after_minutes FROM work_revision_policy WHERE work_id = ?",
    )
    .bind(work)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(policy, (1, 20, 30), "Vorgaben der Tabelle: an, 20 %, 30 Minuten");

    // Kaskade beim Löschen einer Fassung: erst die Schritte (Grabstein mit Werk-uuid), dann die Fassung.
    let work_uuid = text(&pool, &format!("SELECT uuid FROM work WHERE id = {work}")).await;
    let proposal: i64 = scalar(
        &pool,
        &format!(
            "SELECT r.id FROM revision r JOIN node n ON n.id = r.node_id \
             WHERE n.work_id = {work} AND n.seq = 0 AND r.number = 2"
        ),
    )
    .await;
    exec(&pool, &format!("{insert} VALUES ({proposal}, 1, 'amend', NULL, 'x', 'y', 'Mira')")).await;
    let proposal_step = text(&pool, &format!("SELECT uuid FROM revision_amend WHERE revision_id = {proposal}")).await;
    exec(&pool, &format!("DELETE FROM revision WHERE id = {proposal}")).await;
    let grave: (String, Option<String>) = sqlx::query_as("SELECT kind, work_uuid FROM uuid_tombstone WHERE uuid = ?")
        .bind(&proposal_step)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(grave, ("revision_amend".into(), Some(work_uuid.clone())), "Grabstein des Schritts mit Werk-uuid");
    assert_eq!(
        scalar(&pool, &format!("SELECT count(*) FROM revision_amend WHERE revision_id = {proposal}")).await,
        0,
        "Schritte gehen mit ihrer Fassung"
    );
    exec(&pool, &format!("DELETE FROM revision_amend WHERE revision_id = {rev1}")).await;

    assert_eq!(scalar(&pool, "SELECT count(*) FROM pragma_foreign_key_check").await, 0, "foreign_key_check leer");
    let integrity: String = sqlx::query_scalar("PRAGMA integrity_check").fetch_one(&pool).await.unwrap();
    assert_eq!(integrity, "ok", "integrity_check");
    pool.close().await;

    // Store::open danach: Fortschreiben über die Store-API, dann Werk löschen → Grabsteine aller zwanzig Arten.
    let store = Store::open(dir.path()).await.expect("Store::open nach 0050");
    assert_eq!(store.revision_policy(work).await.unwrap(), RevisionPolicy::default(), "Zeile mit Vorgaben gelesen");
    let loose = RevisionPolicy { max_change_percent: 100, ..RevisionPolicy::default() };
    store.set_revision_policy(work, &loose, "Mira").await.expect("Einstellung nach 0050 schreibbar");
    let saved = store
        .save_human_mode(work, 1, "= Abend\n\nDie Möwen kehren spät heim.\n", "Mira", Some(1), SaveMode::Auto)
        .await
        .expect("Fortschreiben nach 0050")
        .expect("Text geändert");
    assert_eq!(saved.amend_seq, Some(1), "Fortschreibung schreibt Stand 1 in die migrierte Fassung");
    let pool = store.pool();
    exec(pool, "DELETE FROM idea WHERE note = 'Nebel über dem Wasser'").await; // Arten idea und idea_link (nur im Test)
    store.delete_work(work, "Die Fähre im Nebel").await.expect("Werk löschen schreibt Grabsteine");
    let fresh: Vec<(String, String)> = sqlx::query_as(
        "SELECT uuid, kind FROM uuid_tombstone WHERE uuid NOT LIKE '00000000-0000-4000-8000-%' AND work_uuid = ?",
    )
    .bind(&work_uuid)
    .fetch_all(pool)
    .await
    .unwrap();
    let kinds: BTreeSet<&str> = fresh.iter().map(|(_, k)| k.as_str()).collect();
    for kind in KINDS.iter().map(|(k, _)| *k).chain(["revision_amend"]) {
        assert!(kinds.contains(kind), "nach 0050 schreibt der Trigger der Art {kind}: {kinds:?}");
    }
    for table in ["revision_amend", "work_revision_policy"] {
        assert_eq!(scalar(pool, &format!("SELECT count(*) FROM {table}")).await, 0, "{table} geht mit dem Werk");
    }
}
