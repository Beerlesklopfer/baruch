// SPDX-License-Identifier: AGPL-3.0-or-later
//! Migration 0008 (Vertrag Runde 6, Abschnitt 1) auf einer nachgebauten Datenbank im Stand von
//! `development` vor Runde 6: Store-Migrationen 0001–0007 und Bibliotheks-Migrationen 0101–0105, vier
//! importierte, frei erfundene Bücher mit den Nummerierungen NULL (aus der Vorlage), '' (keine),
//! „I.a“ (eigenes Muster) und „Ohne“ (altes Muster ohne Zählzeichen), dazu Vorschlag, Beitrag mit
//! Verweis, Persona, Titel, Zitat und Hervorhebung der Bibliothek.
//!
//! Die Bibliotheks-Migrationen liest der Test zur Laufzeit aus `crates/baruch-library/migrations`.

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::path::Path;
use std::str::FromStr;

use baruch_store::{Store, WorkId, title_key};
use sqlx::SqlitePool;
use sqlx::migrate::Migrator;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

// ---- Datenbank im Stand von development ------------------------------------------------------

async fn raw_pool(dir: &Path) -> SqlitePool {
    let options = SqliteConnectOptions::from_str("sqlite:")
        .unwrap()
        .filename(dir.join("baruch.sqlite"))
        .create_if_missing(true)
        .foreign_keys(true);
    SqlitePoolOptions::new().max_connections(1).connect_with(options).await.unwrap()
}

/// Store-Migrationen bis 0007 und die Bibliotheks-Migrationen 0101–0105 (aus dem Ordner gelesen).
async fn migrate_like_development(pool: &SqlitePool) {
    let old: Vec<_> = baruch_store::MIGRATOR.iter().filter(|m| m.version <= 7).cloned().collect();
    assert!(
        old.iter().any(|m| m.version == 7) && baruch_store::MIGRATOR.iter().any(|m| m.version == 8),
        "Stand development hat 0007, die neue Migration ist 0008"
    );
    let store = Migrator { migrations: Cow::Owned(old), ignore_missing: true, ..Migrator::DEFAULT };
    store.run(pool).await.unwrap();
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../baruch-library/migrations");
    let mut library = Migrator::new(dir).await.unwrap();
    let versions: Vec<i64> = library.iter().map(|m| m.version).collect();
    assert!(versions.starts_with(&[101, 102, 103, 104, 105]), "Bibliothek im Stand 0101–0105: {versions:?}");
    library.migrations = Cow::Owned(library.iter().filter(|m| m.version <= 105).cloned().collect());
    library.ignore_missing = true;
    library.run(pool).await.unwrap();
}

/// Frei erfundenes Buch: Vorlage mit römischer Nummerierung, drei Kapitel, Abschnitte, ein
/// Unterabschnitt, mit Maskierung, Label und Kommentar.
fn invented_book(title: &str) -> String {
    let mut out = format!(
        "#set document(title: \"{title}\", author: \"Lore Laterne\")\n#set heading(numbering: \"I.1\")\n\
         #let notiz(body) = text(size: 9pt, body)\n\n"
    );
    for chapter in 1..=3 {
        out.push_str(&format!("= Leuchtturm {chapter}\n\nDas Licht kreist {chapter} Mal.\n\n"));
        for section in 1..=2 {
            if section == 2 {
                out.push_str(&format!("== Wärter\\'s Stube {chapter} <stube-{chapter}> // Notiz\n\n"));
            } else {
                out.push_str(&format!("== Linse {chapter}.{section}\n\n#notiz[Glas {section}.]\n\n"));
            }
            if chapter == 2 && section == 1 {
                out.push_str("=== Prisma\n\nGeschliffen.\n\n");
            }
        }
    }
    out
}

/// Import wie in development (Runde 5): Vorlage als Fassung 1, Knoten mit Revision 1 vom Menschen.
async fn insert_import(pool: &SqlitePool, slug: &str, title: &str, numbering: Option<&str>) -> (WorkId, String) {
    let source = invented_book(title);
    let doc = baruch_typst::parse(&source).unwrap();
    let work: WorkId = sqlx::query_scalar(
        "INSERT INTO work (slug, kind, title, author, template, source_name, source_sha256, title_key, numbering) \
         VALUES (?, 'book', ?, 'Lore Laterne', ?, ?, ?, ?, ?) RETURNING id",
    )
    .bind(slug)
    .bind(title)
    .bind(&doc.template)
    .bind(format!("{slug}.typ"))
    .bind(format!("pruefsumme-{slug}"))
    .bind(title_key(title))
    .bind(numbering)
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO work_template_revision (work_id, number, body, author_name, note) VALUES (?, 1, ?, 'Autor', 'Import')",
    )
    .bind(work)
    .bind(&doc.template)
    .execute(pool)
    .await
    .unwrap();
    let mut stack: Vec<(u8, usize, usize)> = Vec::new();
    let mut roots = 0usize;
    for (seq, heading) in doc.headings.iter().enumerate() {
        while stack.last().is_some_and(|&(level, ..)| level >= heading.level) {
            stack.pop();
        }
        let (parent, position) = match stack.last_mut() {
            Some((_, parent, children)) => {
                *children += 1;
                (Some(*parent), *children - 1)
            }
            None => {
                roots += 1;
                (None, roots - 1)
            }
        };
        let kind = if heading.level == 1 { "chapter" } else { "section" };
        let node_id: i64 = sqlx::query_scalar(
            "INSERT INTO node (work_id, seq, parent_seq, position, number, title, kind) VALUES (?, ?, ?, ?, '1', ?, ?) RETURNING id",
        )
        .bind(work)
        .bind(seq as i64)
        .bind(parent.map(|p| p as i64))
        .bind(position as i64)
        .bind(&heading.title)
        .bind(kind)
        .fetch_one(pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO revision (node_id, number, body, author_kind, author_name, model, status, note) \
             VALUES (?, 1, ?, 'human', 'Autor', NULL, 'current', 'Import')",
        )
        .bind(node_id)
        .bind(&heading.body)
        .execute(pool)
        .await
        .unwrap();
        stack.push((heading.level, seq, 0));
    }
    (work, source)
}

/// Vorschlag, Beitrag mit Verweis, Persona und Bibliothek (Titel, Zitat, Hervorhebung).
async fn insert_extras(pool: &SqlitePool, work: WorkId) {
    let node: i64 = sqlx::query_scalar("SELECT id FROM node WHERE work_id = ? AND seq = 1")
        .bind(work)
        .fetch_one(pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO revision (node_id, number, body, author_kind, author_name, model, status, note, based_on) \
         VALUES (?, 2, '== Linse 1.1\n\nKlarer.\n', 'assistant', '@lektor', 'probe-modell', 'proposed', 'Kürzer.', 1)",
    )
    .bind(node)
    .execute(pool)
    .await
    .unwrap();
    let post: i64 = sqlx::query_scalar(
        "INSERT INTO post (node_id, position, author, body, sources) VALUES (?, 0, '@nathan', 'Wie hell ist das Licht?', '[]') RETURNING id",
    )
    .bind(node)
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO post_ref (post_id, position, node_seq, rev, quote) VALUES (?, 0, 1, 1, 'Glas 1.')")
        .bind(post)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO persona (handle, name, role, model, context_length, active, position) \
         VALUES ('@lektor', 'Lektor', 'Lektorat', 'probe-modell', 8192, 1, 0)",
    )
    .execute(pool)
    .await
    .unwrap();
    let title: i64 = sqlx::query_scalar(
        "INSERT INTO library_title (key, kind, title, authors, year) VALUES ('laterne1911', 'book', 'Vom Licht am Meer', 'Laterne, Lore', 1911) RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    let quote: i64 = sqlx::query_scalar(
        "INSERT INTO library_quote (title_id, position, text, page) VALUES (?, 0, 'Jedes Licht braucht einen Wärter.', 'S. 3') RETURNING id",
    )
    .bind(title)
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO library_highlight (title_id, quote_id, start_char, end_char, text, kind, author_kind, author_name, status) \
         VALUES (?, ?, 6, 11, 'Licht', 'core', 'human', 'Autor', 'active')",
    )
    .bind(title)
    .bind(quote)
    .execute(pool)
    .await
    .unwrap();
}

/// Die vier Werke: (id, Quelle, gespeicherte Nummerierung, erwartetes Schema).
struct Development {
    dir: tempfile::TempDir,
    works: Vec<(WorkId, String, Option<&'static str>, &'static str)>,
}

async fn development_database() -> Development {
    let dir = tempfile::tempdir().unwrap();
    let pool = raw_pool(dir.path()).await;
    migrate_like_development(&pool).await;
    let mut works = Vec::new();
    for (slug, title, numbering, scheme) in [
        ("leuchtfeuer", "Leuchtfeuer", None, "template"),
        ("nebelhorn", "Nebelhorn", Some(""), "none"),
        ("seezeichen", "Seezeichen", Some("I.a"), "custom"),
        ("bake", "Bake", Some("Ohne"), "custom"),
    ] {
        let (work, source) = insert_import(&pool, slug, title, numbering).await;
        works.push((work, source, numbering, scheme));
    }
    insert_extras(&pool, works[0].0).await;
    // Die Datenbank von development ist längst gesät (Beispielwerk gelöscht).
    sqlx::query("INSERT OR REPLACE INTO store_meta (key, value) VALUES ('seeded', '1')").execute(&pool).await.unwrap();
    pool.close().await;
    Development { dir, works }
}

// ---- Abzüge -----------------------------------------------------------------------------------

async fn schema(pool: &SqlitePool) -> BTreeMap<String, Option<String>> {
    let rows: Vec<(String, Option<String>)> =
        sqlx::query_as("SELECT name, sql FROM sqlite_master WHERE name NOT LIKE 'sqlite_stat%'")
            .fetch_all(pool)
            .await
            .unwrap();
    rows.into_iter().collect()
}

async fn columns(pool: &SqlitePool, table: &str) -> Vec<String> {
    sqlx::query_scalar("SELECT name FROM pragma_table_info(?) ORDER BY cid").bind(table).fetch_all(pool).await.unwrap()
}

/// Alle Zeilen aller Tabellen als `quote()`-Text, je Tabelle sortiert; nur die Spalten aus
/// `only` (Tabelle → Spalten), sonst alle. Die Spaltennamen stehen als erste Zeile.
async fn contents(pool: &SqlitePool, only: &BTreeMap<String, Vec<String>>) -> BTreeMap<String, Vec<String>> {
    let tables: Vec<String> = sqlx::query_scalar("SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name")
        .fetch_all(pool)
        .await
        .unwrap();
    let mut out = BTreeMap::new();
    for table in tables {
        let columns = match only.get(&table) {
            Some(columns) => columns.clone(),
            None => columns(pool, &table).await,
        };
        let select = columns.iter().map(|c| format!("quote(\"{c}\")")).collect::<Vec<_>>().join(" || char(31) || ");
        let mut rows: Vec<String> =
            sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT {select} FROM \"{table}\"")))
                .fetch_all(pool)
                .await
                .unwrap();
        rows.sort();
        rows.insert(0, columns.join(","));
        out.insert(table, rows);
    }
    out
}

async fn count(pool: &SqlitePool, sql: &str) -> i64 {
    sqlx::query_scalar(sqlx::AssertSqlSafe(sql)).fetch_one(pool).await.unwrap()
}

// ---- Tests ------------------------------------------------------------------------------------

#[tokio::test]
async fn migration_0008_keeps_every_row_and_column_and_maps_the_old_numbering_to_a_scheme() {
    let dev = development_database().await;
    let pool = raw_pool(dev.dir.path()).await;
    let schema_before = schema(&pool).await;
    let work_columns = columns(&pool, "work").await;
    assert!(!work_columns.contains(&"numbering_scheme".to_owned()), "vorher ohne Schema-Spalte: {work_columns:?}");
    let old_columns = BTreeMap::from([("work".to_owned(), work_columns.clone())]);
    let before = contents(&pool, &old_columns).await;

    // Verglichen wird nach einem Migrator bis 0008, damit spätere Migrationen (0009 Bilder) mit
    // ihren neuen Tabellen diesen Vergleich nicht stören; danach öffnet `Store::open` mit allen.
    let upto_0008: Vec<_> = baruch_store::MIGRATOR.iter().filter(|m| m.version <= 8).cloned().collect();
    let migrator = Migrator { migrations: Cow::Owned(upto_0008), ignore_missing: true, ..Migrator::DEFAULT };
    migrator.run(&pool).await.expect("Migrator bis 0008 führt 0008 aus");
    let raw = pool;
    let pool = &raw;
    let after = contents(pool, &old_columns).await;
    for (table, rows) in &before {
        if table == "_sqlx_migrations" {
            continue;
        }
        assert_eq!(after.get(table), Some(rows), "Tabelle {table}: alle Zeilen und (alten) Spalten unverändert");
    }
    assert_eq!(after.len(), before.len(), "keine neue Tabelle: {:?}", after.keys().collect::<Vec<_>>());

    let schema_after = schema(pool).await;
    assert_eq!(
        schema_after.keys().collect::<Vec<_>>(),
        schema_before.keys().collect::<Vec<_>>(),
        "keine neuen oder entfernten Schemaobjekte"
    );
    for (name, sql) in &schema_before {
        if name != "work" {
            assert_eq!(schema_after.get(name), Some(sql), "Schema von {name} unverändert");
        }
    }
    let work_sql = schema_after["work"].as_deref().unwrap();
    assert!(
        work_sql.contains("numbering_scheme TEXT NOT NULL DEFAULT 'template'") && work_sql.contains("'chapter_words'"),
        "work bekommt die Spalte mit Vorgabe und CHECK: {work_sql}"
    );
    let new_columns = columns(pool, "work").await;
    assert_eq!(&new_columns[..work_columns.len()], &work_columns[..], "alte Spalten in alter Reihenfolge");
    assert_eq!(new_columns.last().map(String::as_str), Some("numbering_scheme"), "die neue Spalte steht hinten");
    raw.close().await;

    let store = Store::open(dev.dir.path()).await.expect("Store::open führt die übrigen Migrationen aus");
    let pool = store.pool();

    for (work, _, numbering, scheme) in &dev.works {
        let (stored_scheme, stored_numbering): (String, Option<String>) =
            sqlx::query_as("SELECT numbering_scheme, numbering FROM work WHERE id = ?")
                .bind(work)
                .fetch_one(pool)
                .await
                .unwrap();
        assert_eq!(stored_scheme, *scheme, "Werk {work}: numbering {numbering:?} → Schema {scheme}");
        assert_eq!(stored_numbering.as_deref(), *numbering, "Werk {work}: work.numbering bleibt unverändert");
    }

    assert_eq!(
        count(pool, "SELECT count(*) FROM pragma_foreign_key_check").await,
        0,
        "keine Verstöße gegen Fremdschlüssel"
    );
    assert_eq!(
        count(
            pool,
            "SELECT count(*) FROM node WHERE (SELECT count(*) FROM revision r WHERE r.node_id = node.id AND r.status = 'current') <> 1"
        )
        .await,
        0,
        "je Abschnitt genau eine aktuelle Fassung"
    );
    let versions: Vec<i64> =
        sqlx::query_scalar("SELECT version FROM _sqlx_migrations ORDER BY version").fetch_all(pool).await.unwrap();
    assert!(
        baruch_store::MIGRATOR.iter().all(|m| versions.contains(&m.version)),
        "alle Store-Migrationen: {versions:?}"
    );
    assert!(versions.contains(&105), "Bibliotheks-Migrationen bleiben eingetragen: {versions:?}");
    assert_eq!(
        count(pool, "SELECT count(*) FROM search_fts WHERE search_fts MATCH 'leuchtturm' AND kind = 'revision'").await,
        12,
        "Volltextsuche findet die Leuchttürme weiter (drei Kapitel in vier Werken)"
    );
    let constraint = sqlx::query("UPDATE work SET numbering_scheme = 'roemisch' WHERE id = ?")
        .bind(dev.works[0].0)
        .execute(pool)
        .await;
    assert!(constraint.is_err(), "der CHECK lässt nur die bekannten Schemata zu");

    // Export: aus der Werkvorlage byte-gleich; die anderen Schemata hängen ihre Regel an die Vorlage.
    let (template_work, source, ..) = &dev.works[0];
    assert_eq!(
        &baruch_typst::xref::strip_key_labels(&store.export_typst(*template_work).await.unwrap()).into_owned(),
        source,
        "Schema template: Export byte-gleich"
    );
    for (work, source, numbering, _) in &dev.works[1..] {
        let exported = baruch_typst::xref::strip_key_labels(&store.export_typst(*work).await.unwrap()).into_owned();
        let rule = match numbering.unwrap() {
            "" => "// Baruch: Gliederungsschema „Ohne Nummern“\n#set heading(numbering: none)\n".to_owned(),
            pattern => {
                format!("// Baruch: Gliederungsschema „Eigenes Muster“\n#set heading(numbering: \"{pattern}\")\n")
            }
        };
        assert!(exported.contains(&rule), "Werk {work}: Regel des Schemas im Export: {exported}");
        assert_eq!(exported.replacen(&rule, "", 1), *source, "Werk {work}: ohne die Regel byte-gleich mit der Quelle");
    }
    let numbers = |w: &baruch_core::Work| w.nodes.iter().map(|n| n.number.clone()).collect::<Vec<_>>();
    let roman = numbers(&store.load_work(dev.works[0].0).await.unwrap());
    assert_eq!(&roman[..3], ["I", "I.1", "I.2"], "aus der Vorlage „I.1“ wie vorher");
    assert!(numbers(&store.load_work(dev.works[1].0).await.unwrap()).iter().all(String::is_empty), "keine Nummern");
    let custom = numbers(&store.load_work(dev.works[2].0).await.unwrap());
    assert_eq!(&custom[..4], ["I", "I.a", "I.b", "II"], "eigenes Muster „I.a“ wie vorher");
    let odd = numbers(&store.load_work(dev.works[3].0).await.unwrap());
    assert!(odd.iter().all(String::is_empty), "altes Muster ohne Zählzeichen lädt wie vorher ohne Nummern: {odd:?}");
    let info = store.outline_numbering(dev.works[2].0).await.unwrap();
    assert_eq!(
        (info.scheme.as_str(), info.custom.as_deref(), info.numbering.as_deref()),
        ("custom", Some("I.a"), Some("I.a")),
        "eigenes Muster als Schema custom"
    );

    // Ein zweiter Start ändert nichts mehr.
    let all = BTreeMap::new();
    let schema_once = schema(pool).await;
    let once = contents(pool, &all).await;
    drop(store);
    let store = Store::open(dev.dir.path()).await.unwrap();
    assert_eq!(schema(store.pool()).await, schema_once, "zweiter Start: Schema gleich");
    assert_eq!(contents(store.pool(), &all).await, once, "zweiter Start: alle Zeilen gleich, auch _sqlx_migrations");
}

#[tokio::test]
async fn new_and_imported_works_start_with_the_template_scheme() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).await.unwrap();
    let source = invented_book("Feuerschiff");
    let work = store.import_typst(&source, "feuerschiff.typ", "Autor").await.unwrap().work;
    let scheme: String = sqlx::query_scalar("SELECT numbering_scheme FROM work WHERE id = ?")
        .bind(work)
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!(scheme, "template", "Vorgabe für importierte Werke: aus der Werkvorlage");
    assert_eq!(
        baruch_typst::xref::strip_key_labels(&store.export_typst(work).await.unwrap()).into_owned(),
        source,
        "und damit byte-gleicher Export"
    );
}
