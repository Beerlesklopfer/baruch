// SPDX-License-Identifier: AGPL-3.0-or-later
//! Migration 0045 (Umschlag-Anordnung, Anschriften, Antrag „Anordnung“; Runde 27) auf einer nachgebauten Datenbank
//! im Stand von development vor Runde 27 (Store bis 0044, Bibliothek vollständig) mit frei erfundenen Zeilen: Werk
//! mit Kapiteln, Vorlagenfassung, Beitrag, Faden, Bild, Einfällen samt Verbindung und Lage, Buchangaben, Bildanträgen
//! aller drei Arten (offen und entschieden, der jüngste Antrag gelöscht) und Grabsteinen aller zwölf Arten.
//!
//! Belegt: 0045 steht nicht in `FOREIGN_KEYS_OFF`, und auf die umgebauten Tabellen `uuid_tombstone` und
//! `asset_request` zeigt kein Fremdschlüssel (darum löst das implizite DELETE von DROP TABLE keine Aktion aus); jede
//! Zeile jeder alten Tabelle bleibt byte-gleich samt ids, auch alle Grabsteine; der Folgezähler von `asset_request`
//! bleibt beim Höchstwert; `work_book` hat die zwei Anschriften als letzte Spalten, leer; im Schema ändern sich nur
//! die CHECKs von `uuid_tombstone` und `asset_request` und die Spalten von `work_book`, die zwölf Grabstein-Trigger
//! stehen wörtlich wieder da; neu sind genau die Objekte von `cover_block`; die CHECKs lehnen ab, was der Vertrag
//! verbietet; Fülltrigger, Grabstein beim Löschen eines Blocks und beim Löschen eines Werks mit Blöcken feuern.

use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::str::FromStr;

use baruch_store::{Store, title_key};
use sqlx::migrate::Migrator;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};

const BOOK: &str = "#set document(title: \"Der Bienengarten\", author: \"Emma Wabe\")\n\n\
= Waben\n\nSechs Ecken hat jede Zelle.\n\n= Honig\n\nIm August wird geschleudert.\n";

/// Die zwölf Grabstein-Arten vor 0045 (Wert in `uuid_tombstone.kind`) und der Name ihres Grabstein-Triggers.
const KINDS: [(&str, &str); 12] = [
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
];

/// Zusätze, um die sich das Schema der alten Tabellen nach 0045 unterscheiden darf.
const TOMBSTONE_KIND: &str = ",\n                                          'cover_block'";
const OP_COVER_LAYOUT: &str = ",'cover_layout'";
const ADDRESS_COLUMNS: &str = ", publisher_address TEXT NOT NULL DEFAULT '', printer_address TEXT NOT NULL DEFAULT ''";

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

/// Store bis 0044 (0040 wie in `baruch_store::migrate` mit ausgeschalteten Fremdschlüsseln) und die Bibliothek:
/// der Stand von development vor Runde 27.
async fn migrate_before(pool: &SqlitePool) {
    assert!(baruch_store::MIGRATOR.iter().any(|m| m.version == 45), "die neue Migration ist 0045");
    assert!(
        !baruch_store::FOREIGN_KEYS_OFF.contains(&45),
        "0045 baut keine Tabelle um, auf die ein Fremdschlüssel zeigt (Vertrag R27 §3)"
    );
    store_migrator(|v| v < 40).run(pool).await.unwrap();
    {
        let mut conn = pool.acquire().await.unwrap();
        sqlx::query("PRAGMA foreign_keys = OFF").execute(&mut *conn).await.unwrap();
        store_migrator(|v| v == 40).run(&mut *conn).await.unwrap();
        sqlx::query("PRAGMA foreign_keys = ON").execute(&mut *conn).await.unwrap();
    }
    store_migrator(|v| (41..=44).contains(&v)).run(pool).await.unwrap();
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../baruch-library/migrations");
    let mut library = Migrator::new(dir).await.unwrap();
    library.ignore_missing = true;
    library.run(pool).await.unwrap();
    let highest = scalar(pool, "SELECT max(version) FROM _sqlx_migrations WHERE version < 100").await;
    assert_eq!(highest, 44, "Stand davor: Store bis 0044");
}

/// Werk mit Inhalten in allen zwölf Tabellen mit uuid, Buchangaben, Bildanträgen und Grabsteinen aller zwölf Arten
/// direkt in SQLite. Gibt die Werk-id zurück.
async fn insert_rows(pool: &SqlitePool) -> i64 {
    exec(pool, "INSERT INTO store_meta (key, value) VALUES ('seeded', '1') ON CONFLICT DO NOTHING").await;
    let doc = baruch_typst::parse(BOOK).unwrap();
    let work: i64 = sqlx::query_scalar(
        "INSERT INTO work (slug, kind, title, author, template, source_name, source_sha256, title_key) \
         VALUES ('der-bienengarten', 'book', 'Der Bienengarten', 'Emma Wabe', ?, 'bienen.typ', 'sha-bienen', ?) \
         RETURNING id",
    )
    .bind(&doc.template)
    .bind(title_key("Der Bienengarten"))
    .fetch_one(pool)
    .await
    .unwrap();
    exec(
        pool,
        &format!(
            "INSERT INTO work_template_revision (work_id, number, body, author_name, note) \
             VALUES ({work}, 1, '// Vorlage\n', 'Emma', 'Anfang')"
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
             VALUES (?, 1, ?, 'human', 'Emma', NULL, 'current', 'Import')",
        )
        .bind(id)
        .bind(&heading.body)
        .execute(pool)
        .await
        .unwrap();
        nodes.push(id);
    }
    let post: i64 = sqlx::query_scalar(
        "INSERT INTO post (node_id, position, author, body, sources) VALUES (?, 0, '@lektor', 'Wie viele Völker?', '[]') \
         RETURNING id",
    )
    .bind(nodes[0])
    .fetch_one(pool)
    .await
    .unwrap();
    let thread: i64 = sqlx::query_scalar(
        "INSERT INTO chat_thread (work_id, created_by, title) VALUES (?, 'Emma', 'Schwärme') RETURNING id",
    )
    .bind(work)
    .fetch_one(pool)
    .await
    .unwrap();
    let asset: i64 = sqlx::query_scalar("INSERT INTO asset (work_id, name) VALUES (?, 'wabe.svg') RETURNING id")
        .bind(work)
        .fetch_one(pool)
        .await
        .unwrap();
    let svg = "<svg xmlns=\"http://www.w3.org/2000/svg\"><circle r=\"2\"/></svg>";
    for sql in [
        format!("INSERT INTO post_ref (post_id, position, node_seq, rev, quote) VALUES ({post}, 0, 0, 1, 'Sechs')"),
        format!(
            "INSERT INTO chat_thread_post (thread_id, position, author, body) VALUES ({thread}, 0, 'Emma', 'Im Mai.')"
        ),
        format!(
            "INSERT INTO asset_revision (asset_id, number, mime, encoding, content, bytes, sha256, caption, alt, \
             author_name, status) \
             VALUES ({asset}, 1, 'image/svg+xml', 'svg', '{svg}', {}, 'abc', 'Wabe', 'Wabe', 'Emma', 'current')",
            svg.len()
        ),
        format!(
            "INSERT INTO idea (work_id, node_id, note, origin_kind, origin_name) \
             VALUES ({work}, {}, 'Tanz der Bienen', 'human', 'Emma')",
            nodes[1]
        ),
        format!(
            "INSERT INTO idea (work_id, node_id, note, origin_kind, origin_name) \
             VALUES ({work}, NULL, 'Duft von Linden', 'persona', '@lektor')"
        ),
        format!(
            "INSERT INTO work_book (work_id, cover_asset_id, publisher, place, year, blurb) \
             VALUES ({work}, {asset}, 'Wabenverlag', 'Imkerdorf', '2026', 'Ein Jahr am Bienenstock.\nMit Rezepten.')"
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
             VALUES ({}, {}, 'leads_to', 'human', 'Emma')",
            ideas[0], ideas[1]
        ),
    )
    .await;
    exec(pool, &format!("INSERT INTO idea_map_pos (work_id, idea_id, x, y) VALUES ({work}, {}, 12.5, -3)", ideas[1]))
        .await;
    // Bildanträge aller drei Arten, offen und entschieden; der jüngste wird wieder gelöscht, damit der Zähler über
    // der größten vorhandenen id steht.
    for (op, name, params, decided) in [
        (
            "replace",
            "wabe.svg",
            r#"{"mime":"image/svg+xml","encoding":"svg","content":"<svg/>","expected_sha256":"abc"}"#,
            false,
        ),
        ("set_cover", "wabe.svg", r#"{"expected_cover":null}"#, true),
        ("delete", "wabe.svg", r#"{"expected_sha256":"abc"}"#, false),
        ("set_cover", "", r#"{"expected_cover":"wabe.svg"}"#, true),
        ("delete", "wabe.svg", r#"{"rev":1,"expected_sha256":"abc"}"#, false),
    ] {
        let decision = if decided {
            "'2026-11-02T09:00:00.000Z', 'stale', 'Emma', 'Der Umschlag ist inzwischen ein anderer.'"
        } else {
            "NULL, NULL, NULL, NULL"
        };
        exec(
            pool,
            &format!(
                "INSERT INTO asset_request (work_id, op, asset_name, params, origin_kind, origin_name, note, created, \
                 decided, decision, decided_by, message) \
                 VALUES ({work}, '{op}', '{name}', '{params}', 'mcp', '@mcp:probe', 'Bitte', '2026-11-01T08:00:00.000Z', \
                 {decision})"
            ),
        )
        .await;
    }
    exec(pool, "DELETE FROM asset_request WHERE id = (SELECT max(id) FROM asset_request)").await;
    // Grabsteine aller zwölf Arten (wie nach früheren Löschungen; erfundene uuids).
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

/// Objekte, die 0045 neu anlegt.
fn new_objects() -> BTreeSet<String> {
    let mut names: BTreeSet<String> =
        ["", "_uuid", "_uuid_fill", "_uuid_tombstone"].iter().map(|s| format!("cover_block{s}")).collect();
    names.insert("sqlite_autoindex_cover_block_1".into());
    names.insert("work_cover_block_cascade_uuid_tombstone".into());
    names
}

/// Ein gültiger Block der Art `kind` (Rücken mit Laufrichtung) als SQL-Werte nach `work_id`.
fn block_values(work: i64, kind: &str) -> String {
    let direction = if kind.starts_with("spine_") { "'down'" } else { "NULL" };
    format!("{work}, '{kind}', 10, 20, 100, 'center', 24, 0.65, 'light', NULL, 'bold', 'shadow', 1, {direction}, NULL")
}

const BLOCK_COLUMNS: &str = "work_id, kind, x_mm, y_mm, width_mm, align, size_pt, leading_em, color, font, style, backdrop, visible, direction, text";

#[tokio::test]
async fn migration_0045_keeps_every_row_and_adds_cover_blocks() {
    let dir = tempfile::tempdir().unwrap();
    let pool = raw_pool(dir.path()).await;
    migrate_before(&pool).await;
    let work = insert_rows(&pool).await;
    let layout = tables(&pool).await;
    let schema_before = schema(&pool).await;
    let before = contents(&pool, &layout).await;
    let seq_before = scalar(&pool, "SELECT seq FROM sqlite_sequence WHERE name = 'asset_request'").await;
    let max_before = scalar(&pool, "SELECT max(id) FROM asset_request").await;
    assert!(seq_before > max_before, "Zähler steht über der größten id (sonst prüfte der Test den Zähler nicht)");
    assert_eq!(scalar(&pool, "SELECT count(DISTINCT kind) FROM uuid_tombstone").await, 12, "alle zwölf Arten");
    assert_eq!(scalar(&pool, "SELECT count(DISTINCT op) FROM asset_request").await, 3, "alle drei Antragsarten");
    let refused =
        sqlx::query("INSERT INTO uuid_tombstone (uuid, kind) VALUES ('x', 'cover_block')").execute(&pool).await;
    assert!(refused.is_err(), "vor 0045 kennt der CHECK die neue Art nicht (sonst prüfte dieser Test nichts)");
    // Auf die umgebauten Tabellen zeigt kein Fremdschlüssel: Begründung, warum 0045 ohne FOREIGN_KEYS_OFF auskommt.
    let pointing = scalar(
        &pool,
        "SELECT count(*) FROM sqlite_master m, pragma_foreign_key_list(m.name) f \
         WHERE m.type = 'table' AND f.\"table\" IN ('uuid_tombstone', 'asset_request')",
    )
    .await;
    assert_eq!(pointing, 0, "kein Fremdschlüssel auf uuid_tombstone oder asset_request");

    // Nur 0045, wie `baruch_store::migrate` sie ausführt (nicht in FOREIGN_KEYS_OFF: Fremdschlüssel bleiben an).
    store_migrator(|v| v == 45).run(&pool).await.expect("0045 läuft auf dem Stand von development");

    // Daten: jede Zeile jeder alten Tabelle in den alten Spalten byte-gleich (ids, Grabsteine, Anträge).
    let after = contents(&pool, &layout).await;
    for (table, rows) in &before {
        if table != "_sqlx_migrations" {
            assert_eq!(after.get(table), Some(rows), "Tabelle {table}: alle Zeilen unverändert");
        }
    }
    assert_eq!(
        scalar(&pool, "SELECT seq FROM sqlite_sequence WHERE name = 'asset_request'").await,
        seq_before,
        "Folgezähler von asset_request bleibt beim Höchstwert (ids nie wiederverwendet)"
    );
    let layout_after = tables(&pool).await;
    for (table, cols) in &layout {
        let mut now = layout_after[table].clone();
        if table == "work_book" {
            let added = now.split_off(cols.len());
            assert_eq!(added, ["publisher_address", "printer_address"], "Anschriften als letzte Spalten");
        }
        assert_eq!(&now, cols, "{table}: alte Spalten in alter Reihenfolge");
    }
    let addresses: (String, String) =
        sqlx::query_as("SELECT publisher_address, printer_address FROM work_book WHERE work_id = ?")
            .bind(work)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(addresses, (String::new(), String::new()), "vorhandene Buchangaben bekommen leere Anschriften");

    // Schema: alte Objekte wörtlich bis auf die drei erlaubten Zusätze, neu genau die Objekte von cover_block.
    let schema_after = schema(&pool).await;
    let added: BTreeSet<String> = schema_after.keys().filter(|k| !schema_before.contains_key(*k)).cloned().collect();
    assert_eq!(added, new_objects(), "neu sind genau Tabelle, Indizes und Trigger von cover_block");
    for (name, (kind, table, sql)) in &schema_before {
        let (kind_now, table_now, sql_now) = schema_after.get(name).unwrap_or_else(|| panic!("{name} verschwunden"));
        assert_eq!((kind_now, table_now), (kind, table), "{name}: Typ und Tabelle unverändert");
        let now = sql_now.clone().unwrap_or_default();
        let expected_addition = match name.as_str() {
            "uuid_tombstone" => Some(TOMBSTONE_KIND),
            "asset_request" => Some(OP_COVER_LAYOUT),
            "work_book" => Some(ADDRESS_COLUMNS),
            _ => None,
        };
        let now = match expected_addition {
            Some(addition) => {
                assert!(now.contains(addition), "{name}: Zusatz aus 0045 ({addition:?}) steht im Schema: {now}");
                now.replace(addition, "")
            }
            None => now,
        };
        // Auch die zwölf Grabstein-Trigger und der Index asset_request_open: wörtlich wie vorher.
        assert_eq!(Some(now), sql.clone().or(Some(String::new())), "Schema von {name} unverändert");
    }
    for (_, trigger) in KINDS {
        assert!(schema_after.contains_key(trigger), "Grabstein-Trigger {trigger} steht wieder da");
    }
    assert!(schema_after.contains_key("asset_request_open"), "Index der offenen Anträge neu angelegt");
    assert!(schema_after.contains_key("uuid_tombstone_work"), "Index der Grabsteine neu angelegt");

    // CHECKs: neue Art und neue Antragsart erlaubt, Erfundenes weiter nicht.
    exec(&pool, "INSERT INTO uuid_tombstone (uuid, kind) VALUES ('probe-cover', 'cover_block')").await;
    let invented =
        sqlx::query("INSERT INTO uuid_tombstone (uuid, kind) VALUES ('probe-x', 'umschlag')").execute(&pool).await;
    assert!(invented.is_err(), "CHECK von uuid_tombstone lehnt eine erfundene Art weiter ab");
    exec(&pool, "DELETE FROM uuid_tombstone WHERE uuid LIKE 'probe-%'").await;
    let request = format!(
        "INSERT INTO asset_request (work_id, op, asset_name, params, origin_kind, origin_name) \
         VALUES ({work}, '{{op}}', '', '{{}}', 'mcp', '@mcp:probe')"
    );
    exec(&pool, &request.replace("{op}", "cover_layout")).await;
    let new_id = scalar(&pool, "SELECT max(id) FROM asset_request").await;
    assert_eq!(new_id, seq_before + 1, "neuer Antrag bekommt eine nie vergebene id");
    let bad_op = sqlx::query(&request.replace("{op}", "move")).execute(&pool).await;
    assert!(bad_op.is_err(), "CHECK von asset_request lehnt eine erfundene Art weiter ab");
    exec(&pool, &format!("DELETE FROM asset_request WHERE id = {new_id}")).await;

    // cover_block: ein gültiger Block je Fläche geht, jede verbotene Abweichung nicht.
    for kind in ["front_title", "spine_title", "back_free"] {
        exec(&pool, &format!("INSERT INTO cover_block ({BLOCK_COLUMNS}) VALUES ({})", block_values(work, kind))).await;
    }
    let valid = block_values(work, "front_author");
    for (bad, why) in [
        (valid.replace("'front_author'", "'front_logo'"), "unbekannte Art"),
        (valid.replace(", 10, 20, 100,", ", -51, 20, 100,"), "x_mm unter -50"),
        (valid.replace(", 10, 20, 100,", ", 10, 1001, 100,"), "y_mm über 1000"),
        (valid.replace(", 10, 20, 100,", ", 10, 20, 0.5,"), "width_mm unter 1"),
        (valid.replace("'center'", "'mittig'"), "unbekannte Ausrichtung"),
        (valid.replace(", 24, 0.65,", ", 3, 0.65,"), "size_pt unter 4"),
        (valid.replace(", 24, 0.65,", ", 24, 3.5,"), "leading_em über 3"),
        (valid.replace("'light'", "'red'"), "Farbe weder light/dark noch #rrggbb"),
        (valid.replace("'light'", "'#12345'"), "Farbe mit sechs Stellen"),
        (valid.replace("'light', NULL,", "'light', '',"), "leerer Schriftname"),
        (valid.replace("'bold'", "'thin'"), "unbekannter Schnitt"),
        (valid.replace("'shadow'", "'glow'"), "unbekannter Hintergrund"),
        (valid.replace(", 1, NULL, NULL", ", 2, NULL, NULL"), "visible nur 0 oder 1"),
        (valid.replace(", 1, NULL, NULL", ", 1, 'down', NULL"), "Laufrichtung nur auf dem Rücken"),
        (block_values(work, "spine_author").replace("'down'", "NULL"), "Rücken braucht eine Laufrichtung"),
        (block_values(work, "spine_author").replace("'down'", "'left'"), "Laufrichtung nur down oder up"),
        (valid.replace(", 1, NULL, NULL", &format!(", 1, NULL, '{}'", "x".repeat(1001))), "Text über 1000 Zeichen"),
        (block_values(work, "front_title"), "je Werk und Art höchstens eine Zeile"),
        (block_values(99_999, "front_author"), "Werk muss es geben (Fremdschlüssel)"),
    ] {
        let result =
            sqlx::query(&format!("INSERT INTO cover_block ({BLOCK_COLUMNS}) VALUES ({bad})")).execute(&pool).await;
        assert!(result.is_err(), "cover_block lehnt ab: {why}");
    }
    exec(
        &pool,
        &format!("INSERT INTO cover_block ({BLOCK_COLUMNS}) VALUES ({})", valid.replace("'light'", "'#a0b1c2'")),
    )
    .await;
    let uuids: Vec<Option<String>> = sqlx::query_scalar("SELECT uuid FROM cover_block").fetch_all(&pool).await.unwrap();
    assert_eq!(uuids.len(), 4, "vier gültige Blöcke");
    for uuid in &uuids {
        let uuid = uuid.as_deref().expect("Fülltrigger setzt die uuid");
        assert!(uuid.len() == 36 && uuid.as_bytes()[14] == b'4', "kanonische v4-uuid: {uuid}");
    }

    // Grabstein beim Löschen eines Blocks, mit der Werk-uuid.
    let work_uuid: String =
        sqlx::query_scalar("SELECT uuid FROM work WHERE id = ?").bind(work).fetch_one(&pool).await.unwrap();
    let back_uuid: String =
        sqlx::query_scalar("SELECT uuid FROM cover_block WHERE kind = 'back_free'").fetch_one(&pool).await.unwrap();
    exec(&pool, "DELETE FROM cover_block WHERE kind = 'back_free'").await;
    let grave: (String, Option<String>) = sqlx::query_as("SELECT kind, work_uuid FROM uuid_tombstone WHERE uuid = ?")
        .bind(&back_uuid)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(grave, ("cover_block".to_owned(), Some(work_uuid.clone())), "Grabstein des Blocks mit Werk-uuid");

    assert_eq!(scalar(&pool, "SELECT count(*) FROM pragma_foreign_key_check").await, 0, "foreign_key_check leer");
    let integrity: String = sqlx::query_scalar("PRAGMA integrity_check").fetch_one(&pool).await.unwrap();
    assert_eq!(integrity, "ok", "integrity_check");
    pool.close().await;

    // Store::open danach; Löschen des Werks mit Blöcken schreibt deren Grabsteine (mit Werk-uuid) und die alten
    // Trigger schreiben weiter.
    let store = Store::open(dir.path()).await.expect("Store::open nach 0045");
    let pool = store.pool();
    let block_uuids: BTreeSet<String> = sqlx::query_scalar("SELECT uuid FROM cover_block WHERE work_id = ?")
        .bind(work)
        .fetch_all(pool)
        .await
        .unwrap()
        .into_iter()
        .collect();
    assert_eq!(block_uuids.len(), 3, "drei Blöcke vor dem Löschen des Werks");
    exec(pool, "DELETE FROM idea WHERE note = 'Tanz der Bienen'").await; // Arten idea und idea_link (hart, nur im Test)
    store.delete_work(work, "Der Bienengarten").await.expect("Werk löschen schreibt Grabsteine");
    assert_eq!(scalar(pool, "SELECT count(*) FROM cover_block").await, 0, "Blöcke gehen mit dem Werk");
    let fresh: Vec<(String, String, Option<String>)> = sqlx::query_as(
        "SELECT uuid, kind, work_uuid FROM uuid_tombstone WHERE uuid NOT LIKE '00000000-0000-4000-8000-%' ORDER BY kind",
    )
    .fetch_all(pool)
    .await
    .unwrap();
    for uuid in &block_uuids {
        let found = fresh.iter().find(|(u, ..)| u == uuid).unwrap_or_else(|| panic!("Grabstein für Block {uuid}"));
        assert_eq!((found.1.as_str(), found.2.as_deref()), ("cover_block", Some(work_uuid.as_str())), "Werk-uuid");
    }
    let kinds: BTreeSet<&str> = fresh.iter().map(|(_, k, _)| k.as_str()).collect();
    for (kind, _) in KINDS {
        assert!(kinds.contains(kind), "nach 0045 schreibt der Trigger der Art {kind} weiter: {kinds:?}");
    }
    for (uuid, kind, grave) in &fresh {
        assert_eq!(grave.as_deref(), Some(work_uuid.as_str()), "Grabstein {kind} {uuid} trägt die Werk-uuid");
    }
}
