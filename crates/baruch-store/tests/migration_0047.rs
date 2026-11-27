// SPDX-License-Identifier: AGPL-3.0-or-later
//! Migration 0047 (Runde 30: Metadaten von Beiträgen, Lösungen je Befund, Varianten, Protokoll des Ersetzens) auf einer
//! nachgebauten Datenbank im Stand von development vor Runde 30 (Store bis 0046, Bibliothek vollständig) mit frei
//! erfundenen Zeilen: Werk mit Kapiteln und Fassungen, Beiträge, Faden mit Beiträgen, Befunde.
//!
//! Belegt: 0047 steht nicht in `FOREIGN_KEYS_OFF` und baut keine Tabelle um; jede Zeile jeder alten Tabelle bleibt in
//! den alten Spalten byte-gleich samt ids und uuids; die neuen Spalten hängen hinten an mit ihren Vorgaben; neu sind
//! genau `replace_batch` (samt Index und AUTOINCREMENT-Zähler) und der Teilindex `revision_variant_group`; die
//! Fremdschlüssel bleiben intakt (`pragma_foreign_key_check` leer); die CHECKs lehnen ab, was der Vertrag verbietet;
//! ein Löschen eines Beitrags setzt `reply_to` der Antwort auf NULL, ein Löschen des Werks nimmt `replace_batch` mit.

use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::str::FromStr;

use baruch_store::title_key;
use sqlx::migrate::Migrator;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};

const BOOK: &str = "#set document(title: \"Der Leuchtturm am Kap\", author: \"Mira Docht\")\n\n\
= Abend\n\nDer Leuchtturm wirft sein Licht.\n\n= Nacht\n\nDer Wachturm schweigt bis zum Morgen.\n";

/// Neue Spalten je Tabelle in der Reihenfolge der Migration.
const NEW_COLUMNS: [(&str, &[&str]); 4] = [
    ("post", &["basis_rev", "basis_label", "reply_to", "repeat_of", "relay_from", "request"]),
    ("chat_thread_post", &["basis_rev", "basis_label", "reply_to", "repeat_of", "relay_from", "request"]),
    ("finding", &["solutions"]),
    ("revision", &["variant_group", "variant_label", "variant_summary", "variant_findings"]),
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

/// Store bis 0046 (0040 wie in `baruch_store::migrate` mit ausgeschalteten Fremdschlüsseln) und die Bibliothek: der
/// Stand von development vor Runde 30.
async fn migrate_before(pool: &SqlitePool) {
    assert!(baruch_store::MIGRATOR.iter().any(|m| m.version == 47), "die neue Migration ist 0047");
    assert!(!baruch_store::FOREIGN_KEYS_OFF.contains(&47), "0047 baut keine Tabelle um (Vertrag R30 §1.1)");
    store_migrator(|v| v < 40).run(pool).await.unwrap();
    {
        let mut conn = pool.acquire().await.unwrap();
        sqlx::query("PRAGMA foreign_keys = OFF").execute(&mut *conn).await.unwrap();
        store_migrator(|v| v == 40).run(&mut *conn).await.unwrap();
        sqlx::query("PRAGMA foreign_keys = ON").execute(&mut *conn).await.unwrap();
    }
    store_migrator(|v| (41..=46).contains(&v)).run(pool).await.unwrap();
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../baruch-library/migrations");
    let mut library = Migrator::new(dir).await.unwrap();
    library.ignore_missing = true;
    library.run(pool).await.unwrap();
    let highest = scalar(pool, "SELECT max(version) FROM _sqlx_migrations WHERE version < 100").await;
    assert_eq!(highest, 46, "Stand davor: Store bis 0046");
}

/// Ids der Testdaten.
struct Rows {
    work: i64,
    posts: Vec<i64>,
    thread_posts: Vec<i64>,
}

/// Werk mit Kapiteln, zwei Fassungen, Beiträgen, Faden und Befunden direkt in SQLite.
async fn insert_rows(pool: &SqlitePool) -> Rows {
    exec(pool, "INSERT INTO store_meta (key, value) VALUES ('seeded', '1') ON CONFLICT DO NOTHING").await;
    let doc = baruch_typst::parse(BOOK).unwrap();
    let work: i64 = sqlx::query_scalar(
        "INSERT INTO work (slug, kind, title, author, template, source_name, source_sha256, title_key) \
         VALUES ('der-leuchtturm-am-kap', 'book', 'Der Leuchtturm am Kap', 'Mira Docht', ?, 'kap.typ', 'sha-kap', ?) \
         RETURNING id",
    )
    .bind(&doc.template)
    .bind(title_key("Der Leuchtturm am Kap"))
    .fetch_one(pool)
    .await
    .unwrap();
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
    sqlx::query(
        "INSERT INTO revision (node_id, number, body, author_kind, author_name, model, status, note, based_on) \
         VALUES (?, 2, '= Abend\n\nDer Leuchtturm blinkt.\n', 'assistant', '@lektor', 'm', 'proposed', 'Vorschlag', 1)",
    )
    .bind(nodes[0])
    .execute(pool)
    .await
    .unwrap();
    let mut posts = Vec::new();
    for (position, (author, body)) in
        [("Autor", "Blinkt er zu schnell?"), ("@lektor", "Kritisch: „wirft sein Licht“ ist abgegriffen.")]
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
        "INSERT INTO chat_thread (work_id, created_by, title) VALUES (?, 'Autor', 'Türme') RETURNING id",
    )
    .bind(work)
    .fetch_one(pool)
    .await
    .unwrap();
    let mut thread_posts = Vec::new();
    for (position, (author, body)) in
        [("Autor", "Welcher Turm zuerst?"), ("@lektor", "Hinweis: der Leuchtturm.")].iter().enumerate()
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
    exec(
        pool,
        &format!(
            "INSERT INTO finding (work_id, node_id, post_id, position, persona, kind, body, quote) \
             VALUES ({work}, {}, {}, 0, '@lektor', 'critical', '„wirft sein Licht“ ist abgegriffen.', 'wirft sein Licht')",
            nodes[0], posts[1]
        ),
    )
    .await;
    exec(
        pool,
        &format!(
            "INSERT INTO finding (work_id, thread_post_id, position, persona, kind, body) \
             VALUES ({work}, {}, 0, '@lektor', 'hint', 'der Leuchtturm')",
            thread_posts[1]
        ),
    )
    .await;
    Rows { work, posts, thread_posts }
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

/// Alle Schemaobjekte: Name → SQL.
async fn schema(pool: &SqlitePool) -> BTreeMap<String, Option<String>> {
    let rows = sqlx::query("SELECT name, sql FROM sqlite_master ORDER BY name").fetch_all(pool).await.unwrap();
    rows.into_iter().map(|r| (r.get(0), r.get(1))).collect()
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

async fn rejected(pool: &SqlitePool, sql: &str) -> bool {
    sqlx::query(sqlx::AssertSqlSafe(sql)).execute(pool).await.is_err()
}

#[tokio::test]
async fn migration_0047_keeps_every_row_and_adds_metadata_solutions_variants_and_the_replace_log() {
    let dir = tempfile::tempdir().unwrap();
    let pool = raw_pool(dir.path()).await;
    migrate_before(&pool).await;
    let rows = insert_rows(&pool).await;
    let layout = tables(&pool).await;
    let schema_before = schema(&pool).await;
    let before = contents(&pool, &layout).await;

    // Nur 0047, wie `baruch_store::migrate` sie ausführt (nicht in FOREIGN_KEYS_OFF: Fremdschlüssel bleiben an).
    store_migrator(|v| v == 47).run(&pool).await.expect("0047 läuft auf dem Stand von development");

    // Daten: jede Zeile jeder alten Tabelle in den alten Spalten byte-gleich (ids, uuids, Texte).
    let after = contents(&pool, &layout).await;
    for (table, old) in &before {
        if table != "_sqlx_migrations" {
            assert_eq!(after.get(table), Some(old), "Tabelle {table}: alle Zeilen unverändert");
        }
    }
    let layout_after = tables(&pool).await;
    for (table, cols) in &layout {
        let mut now = layout_after[table].clone();
        let added = now.split_off(cols.len());
        let expected: &[&str] = NEW_COLUMNS.iter().find(|(t, _)| t == table).map_or(&[], |(_, c)| c);
        assert_eq!(added, expected, "{table}: neue Spalten hinten in der Reihenfolge der Migration");
        assert_eq!(&now, cols, "{table}: alte Spalten in alter Reihenfolge");
    }
    let uuids_null = scalar(
        &pool,
        "SELECT (SELECT count(*) FROM post WHERE uuid IS NULL) + (SELECT count(*) FROM revision WHERE uuid IS NULL) \
         + (SELECT count(*) FROM finding WHERE uuid IS NULL)",
    )
    .await;
    assert_eq!(uuids_null, 0, "uuids bleiben gesetzt (Austausch kennt die Zeilen weiter)");

    // Vorgaben der neuen Spalten.
    for table in ["post", "chat_thread_post"] {
        let unset = scalar(
            &pool,
            &format!(
                "SELECT count(*) FROM {table} WHERE basis_rev IS NULL AND basis_label = '' AND reply_to IS NULL \
                 AND repeat_of IS NULL AND relay_from IS NULL AND request IS NULL"
            ),
        )
        .await;
        assert_eq!(unset, 2, "{table}: vorhandene Beiträge ohne Metadaten (NULL bzw. '')");
    }
    assert_eq!(scalar(&pool, "SELECT count(*) FROM finding WHERE solutions = '[]'").await, 2, "keine Lösungen");
    let plain = scalar(
        &pool,
        "SELECT count(*) FROM revision WHERE variant_group IS NULL AND variant_label IS NULL AND variant_summary = '' \
         AND variant_findings = '[]'",
    )
    .await;
    assert_eq!(plain, 3, "vorhandene Fassungen sind keine Varianten");

    // Schema: alte Objekte wörtlich bis auf die angehängten Spalten, neu genau replace_batch und die zwei Indizes.
    let schema_after = schema(&pool).await;
    // `sqlite_sequence` (Zähler von AUTOINCREMENT) legt SQLite selbst an, falls es ihn noch nicht gab.
    let added: BTreeSet<&str> = schema_after
        .keys()
        .filter(|k| !schema_before.contains_key(*k) && k.as_str() != "sqlite_sequence")
        .map(String::as_str)
        .collect();
    let expected: BTreeSet<&str> =
        ["replace_batch", "replace_batch_work", "revision_variant_group"].into_iter().collect();
    assert_eq!(added, expected, "neu sind nur replace_batch samt Index und der Teilindex der Varianten");
    for (name, sql) in &schema_before {
        if NEW_COLUMNS.iter().any(|(t, _)| t == name) {
            continue;
        }
        assert_eq!(schema_after.get(name), Some(sql), "Schema von {name} unverändert");
    }

    // Fremdschlüssel intakt.
    let broken = scalar(&pool, "SELECT count(*) FROM pragma_foreign_key_check").await;
    assert_eq!(broken, 0, "pragma_foreign_key_check ist leer");

    // CHECKs: gültige Werte gehen, verbotene nicht.
    let (question, answer) = (rows.posts[0], rows.posts[1]);
    exec(
        &pool,
        &format!(
            "UPDATE post SET basis_rev = 1, basis_label = 'geprüft: aktuelle Fassung 1', reply_to = {question}, \
             relay_from = '@lektor', request = '{{\"persona\":\"@lektor\",\"mode\":\"chat\",\"scope\":\"section\",\
             \"text\":\"x\",\"rev\":1}}' WHERE id = {answer}"
        ),
    )
    .await;
    for (bad, why) in [
        (format!("UPDATE post SET basis_rev = 0 WHERE id = {answer}"), "basis_rev ab 1"),
        (format!("UPDATE post SET basis_label = '{}' WHERE id = {answer}", "x".repeat(301)), "basis_label ≤ 300"),
        (format!("UPDATE post SET relay_from = '' WHERE id = {answer}"), "relay_from nicht leer"),
        (format!("UPDATE post SET request = 'kaputt' WHERE id = {answer}"), "request ist JSON"),
        (format!("UPDATE post SET reply_to = 999999 WHERE id = {answer}"), "reply_to zeigt auf einen Beitrag"),
        ("UPDATE finding SET solutions = '{}'".to_owned(), "solutions ist eine Liste"),
        ("UPDATE revision SET variant_label = 'D'".to_owned(), "Etikett nur A/B/C"),
        ("UPDATE revision SET variant_group = 'ABC'".to_owned(), "Gruppe: uuid in Kleinbuchstaben"),
        ("UPDATE revision SET variant_findings = '7'".to_owned(), "variant_findings ist eine Liste"),
        (
            format!(
                "INSERT INTO replace_batch (work_id, query, replacement, items) VALUES ({}, '', 'b', '[]')",
                rows.work
            ),
            "Suchtext nicht leer",
        ),
        (
            format!(
                "INSERT INTO replace_batch (work_id, query, replacement, items) VALUES ({}, 'a', 'b', '{{}}')",
                rows.work
            ),
            "items ist eine Liste",
        ),
    ] {
        assert!(rejected(&pool, &bad).await, "abgelehnt: {why}");
    }
    exec(
        &pool,
        &format!("UPDATE chat_thread_post SET reply_to = {} WHERE id = {}", rows.thread_posts[0], rows.thread_posts[1]),
    )
    .await;
    exec(
        &pool,
        "UPDATE revision SET variant_group = '0f1e2d3c-4b5a-4968-8776-655443322110', variant_label = 'A', \
         variant_summary = 'kürzer', variant_findings = '[1]' WHERE number = 2",
    )
    .await;
    exec(
        &pool,
        &format!(
            "INSERT INTO replace_batch (work_id, query, replacement, items) \
             VALUES ({}, 'Leuchtturm', 'Wachturm', '[{{\"key\":1,\"prev_rev\":1,\"new_rev\":2}}]')",
            rows.work
        ),
    )
    .await;

    // Löschen: Antworten bleiben, der Verweis fällt weg; das Werk nimmt sein Protokoll mit.
    exec(&pool, &format!("DELETE FROM post WHERE id = {question}")).await;
    let reply: Option<i64> =
        sqlx::query_scalar("SELECT reply_to FROM post WHERE id = ?").bind(answer).fetch_one(&pool).await.unwrap();
    assert_eq!(reply, None, "ON DELETE SET NULL: reply_to der Antwort ist NULL");
    exec(&pool, &format!("DELETE FROM chat_thread_post WHERE id = {}", rows.thread_posts[0])).await;
    let reply: Option<i64> = sqlx::query_scalar("SELECT reply_to FROM chat_thread_post WHERE id = ?")
        .bind(rows.thread_posts[1])
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(reply, None, "Fadenbeitrag ebenso");
    // Ein leeres zweites Werk mit eigenem Protokoll: Löschen des Werks nimmt nur sein Protokoll mit.
    let empty: i64 = sqlx::query_scalar(
        "INSERT INTO work (slug, kind, title, author, template, title_key) \
         VALUES ('leer', 'book', 'Leer', '', '', 'leer') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    exec(
        &pool,
        &format!("INSERT INTO replace_batch (work_id, query, replacement, items) VALUES ({empty}, 'a', 'b', '[]')"),
    )
    .await;
    exec(&pool, &format!("DELETE FROM work WHERE id = {empty}")).await;
    let left: Vec<i64> = sqlx::query_scalar("SELECT work_id FROM replace_batch").fetch_all(&pool).await.unwrap();
    assert_eq!(left, vec![rows.work], "ON DELETE CASCADE: nur das Protokoll des gelöschten Werks fällt weg");
    assert_eq!(scalar(&pool, "SELECT count(*) FROM pragma_foreign_key_check").await, 0, "danach weiter intakt");
}

#[tokio::test]
async fn a_store_opened_after_0047_reads_works_with_the_new_columns() {
    // Der ganze Weg wie beim Nutzer: `Store::open` führt alle Migrationen aus, Lesen und Schreiben gehen.
    let dir = tempfile::tempdir().unwrap();
    let store = baruch_store::Store::open(dir.path()).await.unwrap();
    let work = store.import_typst(BOOK, "kap.typ", "Mira").await.unwrap().work;
    let loaded = store.load_work(work).await.unwrap();
    assert_eq!(loaded.nodes.len(), 2, "zwei Kapitel gelesen");
    let tree = store.revision_tree(work).await.unwrap();
    assert!(tree.iter().all(|n| n.revisions.iter().all(|r| r.variant.is_none())), "keine Varianten nach dem Import");
}
