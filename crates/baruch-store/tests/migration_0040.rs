// SPDX-License-Identifier: AGPL-3.0-or-later
//! Migration 0040 (Gliederungsschema „Buch“): Umbau von `work` mit neuem CHECK nach dem offiziellen
//! SQLite-Verfahren, auf einer nachgebauten Datenbank im Stand 0039 (Store bis 0039, Bibliothek bis
//! 0106) mit frei erfundenen Werken und Zeilen in jeder Tabelle, die auf `work` verweist – auch in denen
//! mit ON DELETE CASCADE bzw. SET NULL, die ein `DROP TABLE work` mit eingeschalteten Fremdschlüsseln
//! leeren würde.
//!
//! Belegt: was an `work` hängt (Indizes, keine Trigger, keine Views), dass die Migration mit
//! eingeschalteten Fremdschlüsseln (ein Migrator direkt auf dem Pool) abbricht, ohne etwas zu ändern,
//! und dass `Store::open` (über `baruch_store::migrate`) jede Zeile jeder Tabelle erhält, das Schema
//! jedes anderen Objekts unverändert lässt, Fremdschlüssel danach wieder an sind und greifen,
//! `sqlite_sequence` die größte je vergebene id behält und 'book' wie alle alten Werte gespeichert wird.

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::path::Path;
use std::str::FromStr;

use baruch_store::{NumberingScheme, Store, title_key};
use sqlx::migrate::Migrator;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};

const BOOK: &str = "#set document(title: \"Das Moorbuch\", author: \"Lene Torf\")\n\n\
= Vorrede\n\nÜber das Moor.\n\n= Wege\n\nBohlen liegen quer.\n\n= Ende\n\nNebel.\n";

const SERMON: &str = "#set document(title: \"Vom Schilfrohr\", author: \"Lene Torf\")\n\n\
= Hinführung\n\nDas Rohr biegt sich.\n\n= Ausblick\n\nEs bricht nicht.\n";

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

/// Store bis 0039 und Bibliothek bis 0106.
async fn migrate_before(pool: &SqlitePool) {
    assert!(baruch_store::MIGRATOR.iter().any(|m| m.version == 40), "die neue Migration ist 0040");
    assert!(baruch_store::MIGRATOR.iter().any(|m| m.version == 39), "Stand davor hat 0039");
    assert_eq!(baruch_store::FOREIGN_KEYS_OFF, &[40], "0040 läuft mit ausgeschalteten Fremdschlüsseln");
    store_migrator(|v| v < 40).run(pool).await.unwrap();
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../baruch-library/migrations");
    let mut library = Migrator::new(dir).await.unwrap();
    library.migrations = Cow::Owned(library.iter().filter(|m| m.version <= 106).cloned().collect());
    library.ignore_missing = true;
    library.run(pool).await.unwrap();
}

async fn exec(pool: &SqlitePool, sql: &str) {
    sqlx::query(sql).execute(pool).await.unwrap_or_else(|e| panic!("{sql}: {e}"));
}

async fn scalar(pool: &SqlitePool, sql: &str) -> i64 {
    sqlx::query_scalar(sql).fetch_one(pool).await.unwrap_or_else(|e| panic!("{sql}: {e}"))
}

/// Werk mit Gliederung (je Überschrift ein Kapitel mit aktueller Fassung) direkt in SQLite.
async fn insert_work(pool: &SqlitePool, slug: &str, kind: &str, scheme: &str, source: &str) -> (i64, Vec<i64>) {
    let doc = baruch_typst::parse(source).unwrap();
    let title = slug.replace('-', " ");
    let work: i64 = sqlx::query_scalar(
        "INSERT INTO work (slug, kind, title, author, template, source_name, source_sha256, title_key, \
         subtitle, audience, bible, updated, numbering, numbering_scheme) \
         VALUES (?, ?, ?, 'Lene Torf', ?, ?, ?, ?, 'Unterzeile', 'Wanderer', 'Lutherbibel', '2026-11-01T10:00:00.000Z', \
         'I.;A.', ?) RETURNING id",
    )
    .bind(slug)
    .bind(kind)
    .bind(&title)
    .bind(&doc.template)
    .bind(format!("{slug}.typ"))
    .bind(format!("sha-{slug}"))
    .bind(title_key(&title))
    .bind(scheme)
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO work_template_revision (work_id, number, body, author_name, note) VALUES (?, 1, ?, 'Autor', 'Anfang')")
        .bind(work)
        .bind(&doc.template)
        .execute(pool)
        .await
        .unwrap();
    let mut nodes = Vec::new();
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
        sqlx::query(
            "INSERT INTO revision (node_id, number, body, author_kind, author_name, model, status, note) \
             VALUES (?, 1, ?, 'human', 'Autor', NULL, 'current', 'Import')",
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

/// Zeilen in jeder Tabelle, die auf `work` verweist; gibt (Buch, Predigt) zurück.
async fn insert_rows(pool: &SqlitePool) -> (i64, i64) {
    exec(pool, "INSERT INTO store_meta (key, value) VALUES ('seeded', '1') ON CONFLICT DO NOTHING").await;
    exec(
        pool,
        "INSERT INTO persona (handle, name, role, model, context_length, active, position, color, prompt) VALUES \
         ('@lektor', 'Lektor', 'Lektorat', 'probe-a', 32000, 1, 0, '#1c7ed6', NULL)",
    )
    .await;
    let (book, book_nodes) = insert_work(pool, "das-moorbuch", "book", "sermon", BOOK).await;
    let (sermon, sermon_nodes) = insert_work(pool, "vom-schilfrohr", "sermon", "sermon", SERMON).await;
    // Ein gelöschtes Werk (ohne Gliederung) mit der größten id: sqlite_sequence muss sich diese id merken
    // (0007: ids nie wiederverwenden).
    let empty: i64 = sqlx::query_scalar(
        "INSERT INTO work (slug, kind, title, author, title_key) VALUES ('leer', 'book', 'Leer', 'X', 'leer') RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    exec(pool, &format!("DELETE FROM work WHERE id = {empty}")).await;
    exec(pool, &format!("INSERT INTO work_retired (id, slug, title) VALUES ({empty}, 'leer', 'Leer')")).await;

    sqlx::query(
        "INSERT INTO revision (node_id, number, body, author_kind, author_name, model, status, note, based_on) \
         VALUES (?, 2, '= Wege\n\nBohlen liegen längs.\n', 'assistant', '@lektor', 'probe-a', 'proposed', 'Vorschlag', 1)",
    )
    .bind(book_nodes[1])
    .execute(pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO post (node_id, position, author, body, sources) VALUES (?, 0, '@lektor', 'Quer oder längs?', '[]')")
        .bind(book_nodes[1])
        .execute(pool)
        .await
        .unwrap();
    let thread: i64 = sqlx::query_scalar(
        "INSERT INTO chat_thread (work_id, created_by, title) VALUES (?, 'Autor', 'Teile?') RETURNING id",
    )
    .bind(book)
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO chat_thread_post (thread_id, position, author, body) VALUES (?, 0, 'Autor', 'Vier Teile.')",
    )
    .bind(thread)
    .execute(pool)
    .await
    .unwrap();
    let asset: i64 = sqlx::query_scalar("INSERT INTO asset (work_id, name) VALUES (?, 'moor.png') RETURNING id")
        .bind(book)
        .fetch_one(pool)
        .await
        .unwrap();
    for sql in [
        format!("INSERT INTO asset_path (work_id, path, asset_id) VALUES ({book}, 'bilder/moor.png', {asset})"),
        format!("INSERT INTO work_book (work_id, cover_asset_id, publisher) VALUES ({book}, {asset}, 'Torfverlag')"),
        format!(
            "INSERT INTO outline_request (work_id, op, params, origin_kind, origin_name, note) \
             VALUES ({book}, 'add', '{{\"title\":\"Erster Teil\",\"level\":0}}', 'persona', '@lektor', 'Teile einführen')"
        ),
        format!(
            "INSERT INTO asset_request (work_id, op, asset_name, params, origin_kind, origin_name) \
             VALUES ({book}, 'delete', 'moor.png', '{{}}', 'mcp', '@mcp:probe')"
        ),
        format!("INSERT INTO work_rag (work_id, ways) VALUES ({book}, '[\"lancedb\"]')"),
        format!(
            "INSERT INTO work_rag_collection (work_id, way, collection, position) VALUES ({book}, 'pgvector', 'moor', 1)"
        ),
        format!("INSERT INTO work_bible (work_id, module) VALUES ({sermon}, 'Kieselbibel')"),
        format!("INSERT INTO sync_work (work_id, upload, collection) VALUES ({book}, 1, 'moor')"),
        format!(
            "INSERT INTO work_sword_source (work_id, kind, module, position) VALUES ({sermon}, 'commentary', 'Randnotizen', 1)"
        ),
        format!("INSERT INTO work_timing (work_id, words_per_minute, target_seconds) VALUES ({sermon}, 120, 1200)"),
        format!(
            "INSERT INTO idea (work_id, node_id, note, origin_kind, origin_name) \
             VALUES ({book}, {}, 'Nebel als Bild', 'human', 'Autor')",
            book_nodes[2]
        ),
        format!(
            "INSERT INTO idea (work_id, node_id, note, origin_kind, origin_name) \
             VALUES ({sermon}, {}, 'Rohr im Wind', 'human', 'Autor')",
            sermon_nodes[0]
        ),
    ] {
        exec(pool, &sql).await;
    }
    (book, sermon)
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

/// Alle Schemaobjekte (auch Indizes, Trigger, Views, automatische Indizes): Name → (Typ, Tabelle, SQL).
async fn schema(pool: &SqlitePool) -> BTreeMap<String, (String, String, Option<String>)> {
    let rows =
        sqlx::query("SELECT name, type, tbl_name, sql FROM sqlite_master ORDER BY name").fetch_all(pool).await.unwrap();
    rows.into_iter().map(|r| (r.get(0), (r.get(1), r.get(2), r.get(3)))).collect()
}

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

/// Jeder Fremdschlüssel auf `work`: (Tabelle, Spalte, on_update, on_delete).
async fn references_to_work(pool: &SqlitePool) -> Vec<(String, String, String, String)> {
    sqlx::query_as(
        "SELECT m.name, f.\"from\", f.on_update, f.on_delete FROM sqlite_master m, pragma_foreign_key_list(m.name) f \
         WHERE m.type = 'table' AND f.\"table\" = 'work' ORDER BY m.name, f.\"from\"",
    )
    .fetch_all(pool)
    .await
    .unwrap()
}

#[tokio::test]
async fn migration_0040_rebuilds_work_and_keeps_every_row_and_reference() {
    let dir = tempfile::tempdir().unwrap();
    let pool = raw_pool(dir.path()).await;
    migrate_before(&pool).await;
    let (book, sermon) = insert_rows(&pool).await;

    // Was an `work` hängt (Stand 0039): nur Indizes, kein Trigger, keine View.
    let on_work: Vec<(String, String)> =
        sqlx::query_as("SELECT type, name FROM sqlite_master WHERE tbl_name = 'work' ORDER BY type, name")
            .fetch_all(&pool)
            .await
            .unwrap();
    let expected_on_work: Vec<(String, String)> = [
        ("index", "sqlite_autoindex_work_1"),
        ("index", "work_source_sha256"),
        ("index", "work_title_key"),
        ("table", "work"),
    ]
    .iter()
    .map(|(t, n)| (t.to_string(), n.to_string()))
    .collect();
    assert_eq!(on_work, expected_on_work, "an work hängen genau UNIQUE(slug) und zwei partielle Indizes");
    assert_eq!(scalar(&pool, "SELECT count(*) FROM sqlite_master WHERE type = 'view'").await, 0, "keine Views");
    let triggers_naming_work = scalar(
        &pool,
        "SELECT count(*) FROM sqlite_master WHERE type = 'trigger' AND (sql LIKE '% work %' OR sql LIKE '% work(%' \
         OR sql LIKE '%FROM work%' OR sql LIKE '%JOIN work%' OR sql LIKE '%UPDATE work%' OR sql LIKE '%INTO work %')",
    )
    .await;
    assert_eq!(triggers_naming_work, 0, "kein Trigger liest oder schreibt die Tabelle work");
    let references = references_to_work(&pool).await;
    let cascading: Vec<&str> =
        references.iter().filter(|r| r.3 != "NO ACTION").map(|r| r.0.as_str()).collect::<Vec<_>>();
    assert!(
        cascading.contains(&"outline_request") && cascading.contains(&"work_timing") && cascading.contains(&"idea"),
        "Verweise mit Aktionen beim Löschen (darum Fremdschlüssel aus): {references:?}"
    );

    let layout = tables(&pool).await;
    let schema_before = schema(&pool).await;
    let before = contents(&pool, &layout).await;
    let seq_before = scalar(&pool, "SELECT seq FROM sqlite_sequence WHERE name = 'work'").await;
    let max_before = scalar(&pool, "SELECT max(id) FROM work").await;
    assert!(seq_before > max_before, "das gelöschte Werk hatte die größte id: {seq_before} > {max_before}");
    for table in ["outline_request", "asset_request", "work_book", "work_timing", "chat_thread", "idea", "work_rag"] {
        assert!(!before[table].is_empty(), "{table} hat Zeilen vor der Migration");
    }

    // Mit eingeschalteten Fremdschlüsseln bricht 0040 an der Wache ab, nichts ändert sich.
    let refused = store_migrator(|v| v <= 40).run(&pool).await;
    let refused = refused.expect_err("0040 direkt auf dem Pool (Fremdschlüssel an) muss scheitern");
    assert!(refused.to_string().contains("CHECK constraint failed"), "die Wache (a) greift: {refused}");
    assert_eq!(contents(&pool, &layout).await, before, "nach dem Abbruch ist jede Zeile unverändert");
    assert_eq!(schema(&pool).await, schema_before, "nach dem Abbruch ist das Schema unverändert");
    assert_eq!(scalar(&pool, "SELECT count(*) FROM _sqlx_migrations WHERE version = 40").await, 0, "nicht eingetragen");
    pool.close().await;

    let store = Store::open(dir.path()).await.expect("Store::open führt 0040 aus");
    let pool = store.pool();
    assert_eq!(
        scalar(pool, "SELECT count(*) FROM _sqlx_migrations WHERE version = 40 AND success").await,
        1,
        "0040 lief"
    );

    // Daten: jede Zeile jeder Tabelle (auch sqlite_sequence) unverändert.
    let after = without_instance_row(contents(pool, &layout).await);
    for (table, rows) in &before {
        if table == "_sqlx_migrations" {
            continue;
        }
        assert_eq!(after.get(table), Some(rows), "Tabelle {table}: alle Zeilen unverändert");
    }
    assert_eq!(scalar(pool, "SELECT seq FROM sqlite_sequence WHERE name = 'work'").await, seq_before, "Zähler bleibt");

    // Schema: jedes andere Objekt unverändert, work mit denselben Spalten, Indizes wieder da. `Store::open` führt
    // auch 0041 (Runde 26) und 0043 (Runde 25) aus; deren Objekte und die Spalte uuid nimmt der Vergleich heraus.
    let schema_after = without_0041(schema(pool).await);
    assert_eq!(
        schema_after.keys().collect::<Vec<_>>(),
        schema_before.keys().collect::<Vec<_>>(),
        "dieselben Schemaobjekte (Tabellen, Indizes, Trigger) wie vorher"
    );
    for (name, entry) in &schema_before {
        if name != "work" {
            assert_eq!(schema_after.get(name), Some(entry), "Schema von {name} unverändert");
        }
    }
    let work_sql = schema_after["work"].2.clone().unwrap();
    assert!(work_sql.starts_with("CREATE TABLE \"work\"") || work_sql.starts_with("CREATE TABLE work"), "{work_sql}");
    assert!(work_sql.contains("'sermon','book'") && work_sql.contains("AUTOINCREMENT"), "neuer CHECK: {work_sql}");
    let mut work_columns = tables(pool).await["work"].clone();
    assert_eq!(work_columns.pop().as_deref(), Some("uuid"), "0041 hängt uuid als letzte Spalte an");
    assert_eq!(work_columns, layout["work"], "Spalten von work in alter Reihenfolge");
    // Runde 25 (0043), Runde 18c (0044) und Runde 27 (0045), von `Store::open` mit ausgeführt: `idea_map_pos`,
    // `work_sheet` und `cover_block` verweisen neu auf work, ebenso `finding` (0046), `replace_batch` (0047) und
    // `work_typeset` (0048); der Vergleich gilt den Verweisen von vorher.
    let mut references_after = references_to_work(pool).await;
    references_after.retain(|(table, ..)| {
        ![
            "idea_map_pos",
            "work_sheet",
            "cover_block",
            "finding",
            "replace_batch",
            "work_typeset",
            // Runde 31 (0049): Glossar, eigene Register und Registervorschläge.
            "glossary_term",
            "index_register",
            "index_proposal",
            // Runde 22 (0050): Einstellung „Fassungen“ je Werk.
            "work_revision_policy",
        ]
        .contains(&table.as_str())
    });
    assert_eq!(references_after, references, "alle Verweise auf work samt Aktionen gelten weiter");

    // Prüfungen von SQLite; Fremdschlüssel auf jeder Verbindung des Pools wieder an.
    assert_eq!(scalar(pool, "SELECT count(*) FROM pragma_foreign_key_check").await, 0, "foreign_key_check leer");
    let integrity: String = sqlx::query_scalar("PRAGMA integrity_check").fetch_one(pool).await.unwrap();
    assert_eq!(integrity, "ok", "integrity_check");
    let mut held = Vec::new();
    for _ in 0..4 {
        held.push(pool.acquire().await.unwrap());
    }
    for conn in &mut held {
        let on: i64 = sqlx::query_scalar("PRAGMA foreign_keys").fetch_one(&mut **conn).await.unwrap();
        assert_eq!(on, 1, "Fremdschlüssel nach der Migration auf jeder Verbindung an");
    }
    drop(held);

    // Werte: 'book' und alle alten Schemata speicherbar, Unbekanntes weiter abgelehnt.
    for scheme in NumberingScheme::ALL {
        sqlx::query("UPDATE work SET numbering_scheme = ? WHERE id = ?")
            .bind(scheme.id())
            .bind(book)
            .execute(pool)
            .await
            .unwrap_or_else(|e| panic!("{} speicherbar: {e}", scheme.id()));
    }
    let unknown =
        sqlx::query("UPDATE work SET numbering_scheme = 'roemisch' WHERE id = ?").bind(book).execute(pool).await;
    assert!(unknown.is_err(), "ein unbekannter Wert wird weiter abgelehnt");
    let bad_kind = sqlx::query("UPDATE work SET kind = 'roman' WHERE id = ?").bind(book).execute(pool).await;
    assert!(bad_kind.is_err(), "der CHECK auf kind gilt weiter");
    let doubled = sqlx::query("UPDATE work SET title_key = (SELECT title_key FROM work WHERE id = ?) WHERE id = ?")
        .bind(sermon)
        .bind(book)
        .execute(pool)
        .await;
    assert!(doubled.is_err(), "der eindeutige Index work_title_key gilt weiter");
    store.set_numbering_scheme(book, "book", None).await.expect("Schema Buch über den Store");
    let numbers: Vec<String> = store.load_work(book).await.unwrap().nodes.iter().map(|n| n.number.clone()).collect();
    assert_eq!(numbers, ["Einleitung", "I.", "Schluss"], "Buch nummeriert wie die Predigt");

    // Fremdschlüssel greifen: neue Werke bekommen eine nie vergebene id, Löschen kaskadiert wie vorher.
    let orphan =
        sqlx::query("INSERT INTO work_timing (work_id, words_per_minute) VALUES (99999, 110)").execute(pool).await;
    assert!(orphan.is_err(), "Fremdschlüssel: Zeile braucht ein vorhandenes Werk");
    let fresh: i64 = sqlx::query_scalar(
        "INSERT INTO work (slug, kind, title, author, title_key) VALUES ('neu', 'book', 'Neu', 'X', 'neu') RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(fresh, seq_before + 1, "neue id hinter der größten je vergebenen");
    let title: String =
        sqlx::query_scalar("SELECT title FROM work WHERE id = ?").bind(sermon).fetch_one(pool).await.unwrap();
    store.delete_work(sermon, &title).await.expect("Predigt löschen");
    assert_eq!(scalar(pool, "SELECT count(*) FROM work_timing").await, 0, "work_timing mit dem Werk fort (CASCADE)");
    assert_eq!(scalar(pool, "SELECT count(*) FROM work_bible").await, 0, "work_bible mit dem Werk fort (CASCADE)");
    assert_eq!(
        scalar(pool, "SELECT count(*) FROM idea WHERE note = 'Rohr im Wind' AND work_id IS NULL").await,
        1,
        "Einfall bleibt ohne Werk (SET NULL)"
    );
    assert_eq!(scalar(pool, "SELECT count(*) FROM pragma_foreign_key_check").await, 0, "keine Verstöße danach");
}

#[tokio::test]
async fn a_fresh_database_runs_0040_and_stores_the_book_scheme() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).await.unwrap();
    let pool = store.pool();
    let versions: Vec<i64> =
        sqlx::query_scalar("SELECT version FROM _sqlx_migrations ORDER BY version").fetch_all(pool).await.unwrap();
    assert!(versions.contains(&39) && versions.contains(&40), "{versions:?}");
    let work: i64 = scalar(pool, "SELECT min(id) FROM work").await;
    exec(pool, &format!("UPDATE work SET numbering_scheme = 'book' WHERE id = {work}")).await;
    let on: i64 = sqlx::query_scalar("PRAGMA foreign_keys").fetch_one(pool).await.unwrap();
    assert_eq!(on, 1, "Fremdschlüssel an");
    drop(store);
    // Zweites Öffnen: nichts mehr zu tun, Wert bleibt.
    let again = Store::open(dir.path()).await.expect("wiederholbar");
    let scheme: String = sqlx::query_scalar("SELECT numbering_scheme FROM work WHERE id = ?")
        .bind(work)
        .fetch_one(again.pool())
        .await
        .unwrap();
    assert_eq!(scheme, "book");
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

/// Zusätze aus 0045 (Runde 27), die `Store::open` mit ausführt: die vierte Antragsart in `asset_request` und die
/// zwei Anschriften in `work_book` (geprüft in migration_0045.rs).
const OP_COVER_LAYOUT: &str = ",'cover_layout'";
const ADDRESS_COLUMNS: &str = ", publisher_address TEXT NOT NULL DEFAULT '', printer_address TEXT NOT NULL DEFAULT ''";

fn without_0045(sql: &str) -> String {
    sql.replace(OP_COVER_LAYOUT, "").replace(ADDRESS_COLUMNS, "")
}

const UUID_COLUMN: &str = ", uuid TEXT CHECK (uuid IS NULL OR (length(uuid) = 36 AND uuid = lower(uuid)))";

/// Schema ohne die Objekte aus Migration 0041 (Runde 26: Tabelle `uuid_tombstone` samt Index, je Tabelle
/// `<T>_uuid`, `<T>_uuid_fill`, `<T>_uuid_tombstone`, Primärschlüsselindex von `uuid_tombstone`), ohne die
/// späteren Objekte, die `Store::open` mit anlegt (0043: `idea_link`, `idea_map_pos` samt Indizes und Triggern),
/// und ohne den Zusatz der Spalte uuid im CREATE-Text.
fn without_0041(
    schema: BTreeMap<String, (String, String, Option<String>)>,
) -> BTreeMap<String, (String, String, Option<String>)> {
    schema
        .into_iter()
        .filter(|(name, _)| {
            !(name.starts_with("uuid_tombstone")
                || name == "sqlite_autoindex_uuid_tombstone_1"
                // Runde 25 (0043): Tabellen der Gedankenkarte samt Indizes und Triggern.
                || name.starts_with("idea_link")
                || name.starts_with("idea_map_pos")
                || name.starts_with("sqlite_autoindex_idea_link_")
                || name.starts_with("sqlite_autoindex_idea_map_pos_")
                // Runde 18c (0044): Satzbild-Einstellungen je Werk, eine neue Tabelle ohne Index und Trigger.
                || name == "work_sheet"
                // Runde 27 (0045): Blöcke der Umschlag-Anordnung samt Indizes und Triggern.
                || name.starts_with("cover_block")
                || name == "sqlite_autoindex_cover_block_1"
                || name == "work_cover_block_cascade_uuid_tombstone"
                // Runde 28 (0046): Befunde, Befundverlauf und alte Beitragsfassungen samt Indizes und Triggern.
                || name.starts_with("finding")
                || name.starts_with("post_version")
                || name == "thread_post_version_cascade_uuid_tombstone"
                || name == "work_finding_cascade_uuid_tombstone"
                // Runde 30 (0047): Protokoll des Ersetzens samt Index, Teilindex der Varianten.
                || R30_OBJECTS.contains(&name.as_str())
                // Runde 29 (0048): Satz-Einstellungen je Werk (Index, Fülltrigger, Grabstein und Kaskade enden auf
                // _uuid, _uuid_fill bzw. _uuid_tombstone).
                || name == "work_typeset"
                // Runde 31 (0049): Glossar, eigene Register und Registervorschläge samt Indizes und Triggern.
                || name.starts_with("glossary_")
                || name.starts_with("index_register")
                || name.starts_with("index_proposal")
                || name == "sqlite_autoindex_glossary_variant_1"
                || name == "sqlite_autoindex_index_register_1"
                // Runde 22 (0050): Verlauf der Fortschreibungen samt Index und Triggern, Einstellung „Fassungen“.
                || name.starts_with("revision_amend")
                || name == "sqlite_autoindex_revision_amend_1"
                || name == "work_revision_policy"
                || name.ends_with("_uuid")
                || name.ends_with("_uuid_fill")
                || name.ends_with("_uuid_tombstone"))
        })
        .map(|(name, (kind, table, sql))| {
            // Runde 28 (0046): `edited` an Beiträgen und Fadenbeiträgen (revision hat schon vorher eine Spalte edited).
            let edited = name == "post" || name == "chat_thread_post";
            let sql = sql
                .map(|s| without_0045(&s.replace(UUID_COLUMN, "")))
                .map(|s| if edited { s.replace(", edited TEXT", "") } else { s })
                .map(|s| without_r30(&name, &s));
            (name, (kind, table, sql))
        })
        .collect()
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
