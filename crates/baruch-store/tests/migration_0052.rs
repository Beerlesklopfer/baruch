// SPDX-License-Identifier: AGPL-3.0-or-later
//! Migration 0052 (Runde 34 „Mindmap wie XMind“, Vertrag §1: Titel, Elternknoten und Reihenfolge am Einfall; dazu
//! nach dem Nachtrag des Nutzers vom 2026-11-25 die Spalte `main_topic`) auf einer nachgebauten Datenbank im Stand von
//! development vor Runde 34 (Store bis 0051, Bibliothek vollständig) mit frei erfundenen Einfällen (offen,
//! verarbeitet, gelöscht, mit und ohne Bezug, ohne Werk), Verbindungen (auch einer gelösten) und Lagen.
//!
//! Belegt: 0052 ist die einzige neue Store-Migration und steht nicht in `FOREIGN_KEYS_OFF`; jede Zeile jeder alten
//! Tabelle bleibt in ihren alten Spalten byte-gleich (ids, uuid, `updated`), auch der Volltextindex; die neuen Spalten
//! tragen in jeder alten Zeile ihre Vorgabe; im Schema ändert sich nur die Tabelle `idea` um genau die vier Spalten,
//! neu ist nur der Index `idea_parent`; die CHECKs lehnen ab, was der Vertrag verbietet; ein hart gelöschter
//! Elternknoten löst nur die Bindung; der Store liest die alten Einfälle danach als Einfälle der Wolke.

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::path::Path;
use std::str::FromStr;

use baruch_store::{IdeaFilter, IdeaScope, Store};
use sqlx::migrate::Migrator;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};

const BOOK: &str = "#set document(title: \"Der Kahn am Wehr\", author: \"Tilda Rohr\")\n\n\
= Schleuse\n\nDas Wasser steigt langsam.\n\n= Wehr\n\nDas Wasser fällt laut.\n";

/// Spalten, die 0052 an `idea` hängt (Spalte, Vorgabe per `quote()`, Definition wörtlich).
const NEW_COLUMNS: [(&str, &str, &str); 4] = [
    ("title", "''", "title TEXT NOT NULL DEFAULT '' CHECK (length(title) <= 200)"),
    ("parent_id", "NULL", "parent_id INTEGER REFERENCES idea(id) ON DELETE SET NULL"),
    ("position", "0", "position INTEGER NOT NULL DEFAULT 0"),
    ("main_topic", "0", "main_topic INTEGER NOT NULL DEFAULT 0 CHECK (main_topic IN (0, 1))"),
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

async fn rejected(pool: &SqlitePool, sql: &str) -> bool {
    sqlx::query(sql).execute(pool).await.is_err()
}

/// Store bis 0051 (0040 wie in `baruch_store::migrate` mit ausgeschalteten Fremdschlüsseln) und die Bibliothek: der
/// Stand von development vor Runde 34.
async fn migrate_before(pool: &SqlitePool) {
    let versions: Vec<i64> = baruch_store::MIGRATOR.iter().map(|m| m.version).filter(|v| *v < 100).collect();
    assert_eq!(
        versions.iter().copied().filter(|v| *v > 51 && *v < 53).collect::<Vec<_>>(),
        [52],
        "einzige Store-Migration der Runde 34: 0052 (ihre Reservierungen 0053 bis 0055 blieben unbenutzt; 0053 und \
         0054 gehören seit Runde 35 den Konten und Freigaben)"
    );
    assert!(
        !baruch_store::FOREIGN_KEYS_OFF.contains(&52),
        "0052 hängt nur Spalten an und baut keine Tabelle um: nicht in FOREIGN_KEYS_OFF"
    );
    store_migrator(|v| v < 40).run(pool).await.unwrap();
    {
        let mut conn = pool.acquire().await.unwrap();
        sqlx::query("PRAGMA foreign_keys = OFF").execute(&mut *conn).await.unwrap();
        store_migrator(|v| v == 40).run(&mut *conn).await.unwrap();
        sqlx::query("PRAGMA foreign_keys = ON").execute(&mut *conn).await.unwrap();
    }
    store_migrator(|v| (41..=51).contains(&v)).run(pool).await.unwrap();
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../baruch-library/migrations");
    let mut library = Migrator::new(dir).await.unwrap();
    library.ignore_missing = true;
    library.run(pool).await.unwrap();
    assert_eq!(
        scalar(pool, "SELECT max(version) FROM _sqlx_migrations WHERE version < 100").await,
        51,
        "Stand davor: Store bis 0051"
    );
}

/// Ein Werk mit zwei Kapiteln und sechs Einfällen direkt in SQLite (wie der Store sie im Stand 0051 schrieb, samt
/// Schlagwörtern und Volltextindex), dazu ein Einfall ohne Werk, drei Verbindungen (eine gelöst) und zwei Lagen.
/// Rückgabe: id des Werks.
async fn insert_rows(pool: &SqlitePool) -> i64 {
    let doc = baruch_typst::parse(BOOK).unwrap();
    let work: i64 = sqlx::query_scalar(
        "INSERT INTO work (slug, kind, title, author, template, source_name, source_sha256, title_key) \
         VALUES ('der-kahn-am-wehr', 'book', 'Der Kahn am Wehr', 'Tilda Rohr', ?, 'kahn.typ', 'sha-kahn', ?) \
         RETURNING id",
    )
    .bind(&doc.template)
    .bind(baruch_store::title_key("Der Kahn am Wehr"))
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
        nodes.push(id);
    }
    assert_eq!(nodes.len(), 2, "Vorbedingung: zwei Kapitel");
    // (Werk, Bezug, Gedanke, Herkunft, Name, Status, erledigt als, angelegt/geändert, gelöscht, Schlagwörter)
    type Seed<'a> = (
        Option<i64>,
        Option<i64>,
        &'a str,
        &'a str,
        &'a str,
        &'a str,
        Option<&'a str>,
        &'a str,
        Option<&'a str>,
        &'a [&'a str],
    );
    let seeds: [Seed<'_>; 7] = [
        (
            Some(work),
            Some(nodes[0]),
            "Das Tor klemmt bei Frost.",
            "human",
            "Autor",
            "open",
            None,
            "2026-03-01T08:00:00.000Z",
            None,
            &["Tor", "Frost"],
        ),
        (
            Some(work),
            Some(nodes[1]),
            "Der Lärm als Gegenbild zur Stille.",
            "persona",
            "@lektor",
            "done",
            Some("note"),
            "2026-03-02T09:30:00.000Z",
            None,
            &["Lärm"],
        ),
        (
            Some(work),
            None,
            "Ein Aal im Eimer.",
            "mcp",
            "@mcp:probe",
            "open",
            None,
            "2026-03-03T10:00:00.000Z",
            None,
            &["Aal", "Eimer"],
        ),
        (
            Some(work),
            None,
            "Verworfen: zu viel Schilf.",
            "human",
            "Autor",
            "open",
            None,
            "2026-03-04T11:00:00.000Z",
            Some("2026-03-05T12:00:00.000Z"),
            &["Schilf"],
        ),
        (Some(work), Some(nodes[0]), "", "human", "Autor", "open", None, "2026-03-06T07:15:00.000Z", None, &["Pegel"]),
        (
            Some(work),
            None,
            "Gelöscht samt Verbindung.",
            "human",
            "Autor",
            "done",
            Some("post"),
            "2026-03-07T07:15:00.000Z",
            Some("2026-03-08T07:15:00.000Z"),
            &["Kette"],
        ),
        (
            None,
            None,
            "Ohne Werk: ein Reiher.",
            "human",
            "Autor",
            "open",
            None,
            "2026-03-09T06:00:00.000Z",
            None,
            &["Reiher"],
        ),
    ];
    for (work_id, node, note, kind, name, status, done_as, time, deleted, words) in seeds {
        let id: i64 = sqlx::query_scalar(
            "INSERT INTO idea (work_id, node_id, note, origin_kind, origin_name, status, done_as, created, updated, deleted) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?) RETURNING id",
        )
        .bind(work_id)
        .bind(node)
        .bind(note)
        .bind(kind)
        .bind(name)
        .bind(status)
        .bind(done_as)
        .bind(time)
        .bind(deleted.unwrap_or(time))
        .bind(deleted)
        .fetch_one(pool)
        .await
        .unwrap();
        for (position, word) in words.iter().enumerate() {
            sqlx::query("INSERT INTO idea_word (idea_id, position, word, word_key) VALUES (?, ?, ?, ?)")
                .bind(id)
                .bind(position as i64)
                .bind(word)
                .bind(word.to_lowercase())
                .execute(pool)
                .await
                .unwrap();
        }
        if deleted.is_none() {
            sqlx::query("INSERT INTO idea_fts (rowid, words, note, quote, source, bible) VALUES (?, ?, ?, '', '', '')")
                .bind(id)
                .bind(words.join(" "))
                .bind(note)
                .execute(pool)
                .await
                .unwrap();
        }
    }
    let ideas: Vec<i64> = sqlx::query_scalar("SELECT id FROM idea WHERE work_id = ? ORDER BY id")
        .bind(work)
        .fetch_all(pool)
        .await
        .unwrap();
    for (from, to, kind, note, deleted) in [
        (ideas[0], ideas[1], "leads_to", "vom Frost zum Lärm", None),
        (ideas[0], ideas[2], "contradicts", "", None),
        (ideas[1], ideas[4], "belongs_to", "gelöst", Some("2026-03-10T10:00:00.000Z")),
    ] {
        sqlx::query(
            "INSERT INTO idea_link (from_id, to_id, kind, note, origin_kind, origin_name, deleted) \
             VALUES (?, ?, ?, ?, 'human', 'Autor', ?)",
        )
        .bind(from)
        .bind(to)
        .bind(kind)
        .bind(note)
        .bind(deleted)
        .execute(pool)
        .await
        .unwrap();
    }
    for (idea, x, y) in [(ideas[0], 120.5, -40.0), (ideas[2], -300.0, 75.25)] {
        exec(pool, &format!("INSERT INTO idea_map_pos (work_id, idea_id, x, y) VALUES ({work}, {idea}, {x}, {y})"))
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

/// Der Volltextindex der Einfälle als sortierte Zeilen (er ist eine virtuelle Tabelle und fehlt in [`contents`]).
async fn fts(pool: &SqlitePool) -> Vec<String> {
    sqlx::query_scalar("SELECT rowid || char(31) || words || char(31) || note FROM idea_fts ORDER BY rowid")
        .fetch_all(pool)
        .await
        .unwrap()
}

#[tokio::test]
async fn migration_0052_keeps_every_idea_link_and_position_and_adds_the_tree_columns() {
    let dir = tempfile::tempdir().unwrap();
    let pool = raw_pool(dir.path()).await;
    migrate_before(&pool).await;
    let work = insert_rows(&pool).await;
    let layout = tables(&pool).await;
    let schema_before = schema(&pool).await;
    let before = contents(&pool, &layout).await;
    let fts_before = fts(&pool).await;
    assert_eq!(scalar(&pool, "SELECT count(*) FROM idea").await, 7, "Vorbedingung: sieben Einfälle (einer ohne Werk)");
    assert_eq!(
        scalar(&pool, "SELECT count(*) FROM idea WHERE deleted IS NOT NULL").await,
        2,
        "Vorbedingung: zwei gelöschte Einfälle"
    );
    assert_eq!(scalar(&pool, "SELECT count(*) FROM idea_link").await, 3, "Vorbedingung: drei Verbindungen");
    assert_eq!(scalar(&pool, "SELECT count(*) FROM idea_map_pos").await, 2, "Vorbedingung: zwei Lagen");
    assert_eq!(
        scalar(&pool, "SELECT count(*) FROM idea WHERE uuid IS NULL").await,
        0,
        "Vorbedingung: jeder Einfall trägt seine uuid (Fülltrigger aus 0041)"
    );
    assert!(
        rejected(&pool, "UPDATE idea SET title = 'x'").await,
        "vor 0052 gibt es die Spalte title nicht (sonst prüfte dieser Test nichts)"
    );

    // Nur 0052, wie `baruch_store::migrate` sie ausführt (nicht in FOREIGN_KEYS_OFF: Fremdschlüssel bleiben an).
    store_migrator(|v| v == 52).run(&pool).await.expect("0052 läuft auf dem Stand von development");

    // Daten: jede Zeile jeder alten Tabelle in ihren alten Spalten byte-gleich (ids, uuid, updated, deleted).
    let after = contents(&pool, &layout).await;
    for (table, old) in &before {
        if table != "_sqlx_migrations" {
            assert_eq!(after.get(table), Some(old), "Tabelle {table}: alle Zeilen in den alten Spalten unverändert");
        }
    }
    assert_eq!(fts(&pool).await, fts_before, "der Volltextindex der Einfälle bleibt, wie er war (nichts nachgefüllt)");

    // Neue Spalten: genau die vier, hinten an `idea`, mit der Vorgabe in jeder alten Zeile.
    let layout_after = tables(&pool).await;
    for (table, cols) in &layout {
        let mut expected = cols.clone();
        if table == "idea" {
            expected.extend(NEW_COLUMNS.iter().map(|(column, ..)| (*column).to_owned()));
        }
        assert_eq!(layout_after[table], expected, "{table}: alte Spalten in Reihenfolge, neue nur an idea und hinten");
    }
    for (column, default, _) in NEW_COLUMNS {
        let values: Vec<String> =
            sqlx::query_scalar(&format!("SELECT DISTINCT quote({column}) FROM idea")).fetch_all(&pool).await.unwrap();
        assert_eq!(values, [default], "idea.{column}: jede bestehende Zeile trägt die Vorgabe");
    }

    // Schema: nur `idea` ändert sich (um genau die neuen Spalten), neu ist nur der Index `idea_parent`.
    let schema_after = schema(&pool).await;
    for (name, (kind, table, sql)) in &schema_before {
        let now = schema_after.get(name).unwrap_or_else(|| panic!("Schemaobjekt {name} fehlt nach 0052"));
        if name == "idea" {
            // SQLite schreibt die neuen Spalten in den gespeicherten CREATE-Text hinter die zuletzt angehängte
            // Spalte (uuid aus 0041): Nimmt man genau diesen Zusatz heraus, steht die alte Definition wörtlich da.
            let added: String = NEW_COLUMNS.iter().map(|(_, _, definition)| format!(", {definition}")).collect();
            let text = now.2.clone().unwrap();
            assert_eq!(text.matches(&added).count(), 1, "idea: genau einmal der Zusatz der vier Spalten: {text}");
            assert_eq!(
                Some(text.replace(&added, "")),
                *sql,
                "idea: die alte Definition, ergänzt um genau die vier Spalten des Vertrags"
            );
        } else {
            assert_eq!(now, &(kind.clone(), table.clone(), sql.clone()), "{name}: unverändert");
        }
    }
    let new: Vec<&String> = schema_after.keys().filter(|name| !schema_before.contains_key(*name)).collect();
    assert_eq!(new, ["idea_parent"], "neu ist nur der Index über (parent_id, position)");
    assert_eq!(
        scalar(&pool, "SELECT count(*) FROM pragma_foreign_key_check").await,
        0,
        "kein Fremdschlüssel ist nach 0052 verletzt"
    );

    // CHECKs und Fremdschlüssel der neuen Spalten.
    let ideas: Vec<i64> = sqlx::query_scalar("SELECT id FROM idea WHERE work_id = ? ORDER BY id")
        .bind(work)
        .fetch_all(&pool)
        .await
        .unwrap();
    assert!(
        rejected(&pool, &format!("UPDATE idea SET title = '{}' WHERE id = {}", "ä".repeat(201), ideas[0])).await,
        "ein Titel mit 201 Zeichen scheitert am CHECK"
    );
    exec(&pool, &format!("UPDATE idea SET title = '{}' WHERE id = {}", "ä".repeat(200), ideas[0])).await;
    assert!(
        rejected(&pool, &format!("UPDATE idea SET main_topic = 2 WHERE id = {}", ideas[2])).await,
        "main_topic kennt nur 0 und 1"
    );
    assert!(
        rejected(&pool, &format!("UPDATE idea SET parent_id = 999999 WHERE id = {}", ideas[2])).await,
        "ein Elternknoten, den es nicht gibt, scheitert am Fremdschlüssel"
    );
    exec(&pool, &format!("UPDATE idea SET parent_id = {}, position = 3 WHERE id = {}", ideas[5], ideas[2])).await;
    exec(&pool, &format!("DELETE FROM idea WHERE id = {}", ideas[5])).await;
    assert_eq!(
        scalar(&pool, &format!("SELECT count(*) FROM idea WHERE id = {} AND parent_id IS NULL", ideas[2])).await,
        1,
        "ein hart gelöschter Elternknoten löst nur die Bindung (ON DELETE SET NULL), das Kind bleibt"
    );
    assert_eq!(
        scalar(&pool, "SELECT count(*) FROM uuid_tombstone WHERE kind = 'idea'").await,
        1,
        "der Grabstein-Trigger der Einfälle feuert weiter"
    );
    pool.close().await;

    // Der Store öffnet die Datenbank (migrate ist danach ein Nichtstun) und liest die alten Einfälle mit Vorgaben.
    let store = Store::open(dir.path()).await.expect("der Store öffnet die migrierte Datenbank");
    let listed = store.idea_list(&IdeaFilter { scope: IdeaScope::Work(work), ..IdeaFilter::default() }).await.unwrap();
    assert_eq!(listed.len(), 4, "vier nicht gelöschte Einfälle des Werks (einer wurde oben hart gelöscht)");
    let eel = listed.iter().find(|i| i.words == ["Aal", "Eimer"]).expect("der Einfall „Aal“ ist da");
    assert!(
        eel.cloud && eel.parent.is_none() && !eel.main_topic && eel.title.is_empty(),
        "ein alter Einfall ohne Bezug liegt in der Wolke (Nachtrag 2026-11-25), ohne Titel und Elternknoten: {eel:?}"
    );
    let noise = listed.iter().find(|i| i.words == ["Lärm"]).expect("der Einfall „Lärm“ ist da");
    assert!(!noise.cloud && noise.key.is_some(), "ein alter Einfall mit Bezug hängt weiter an seinem Abschnitt");
    assert_eq!(store.idea_links_of_work(work, true).await.unwrap().len(), 3, "alle Verbindungen sind noch da");
    assert_eq!(store.idea_map_positions(work).await.unwrap().len(), 2, "beide Lagen sind noch da");
}
