// SPDX-License-Identifier: AGPL-3.0-or-later
//! Migration 0007 (contract Round 5, section 2) on a rebuilt database in the state of `development` before Round 5:
//! store migrations 0001–0006 and library migrations 0101–0105, work 1 = seeded sample work (old seed with „= Teil
//! …“), work 2 = imported, freely invented book with 87 outline points, plus personas, posts with references,
//! proposals, titles, quotes and highlights of the library.
//!
//! The comparison happens after a migrator up to 0007 (Round 6); afterwards `Store::open` opens the database with
//! all further migrations for the remaining checks.
//!
//! The test reads the library migrations at run time from `crates/baruch-library/migrations` (baruch-store does not
//! depend on baruch-library); it checks the full-text index through SQL.

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::path::Path;
use std::str::FromStr;

use baruch_core::{RevisionStatus, sample};
use baruch_store::{NewWork, Preset, Store, WorkId, title_key};
use sqlx::SqlitePool;
use sqlx::migrate::Migrator;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

/// Title of the invented book (work 2).
const BOOK_TITLE: &str = "Das Uhrmacherbuch";

/// New objects from 0007 that did not exist before.
const NEW_TABLES: &[&str] = &["store_meta", "work_retired", "sqlite_sequence"];

// ---- Database in the state of development ----------------------------------------------------

async fn raw_pool(dir: &Path) -> SqlitePool {
    let options = SqliteConnectOptions::from_str("sqlite:")
        .unwrap()
        .filename(dir.join("baruch.sqlite"))
        .create_if_missing(true)
        .foreign_keys(true);
    SqlitePoolOptions::new().max_connections(1).connect_with(options).await.unwrap()
}

/// Store migrations before 0007 and the library migrations 0101–0105 (read from the folder).
async fn migrate_like_development(pool: &SqlitePool) {
    let old: Vec<_> = baruch_store::MIGRATOR.iter().filter(|m| m.version < 7).cloned().collect();
    assert!(
        old.iter().any(|m| m.version == 6) && baruch_store::MIGRATOR.iter().any(|m| m.version == 7),
        "Stand development hat 0006, die neue Migration ist 0007"
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

/// The sample work the way the seed created it before Round 5: parts with „= Teil …“ as text, title key, template
/// as revision 1, posts with references, personas.
async fn insert_old_sample(pool: &SqlitePool) -> WorkId {
    let work = sample::work();
    let work_id: WorkId = sqlx::query_scalar(
        "INSERT INTO work (slug, kind, title, author, title_key) VALUES ('beispielwerk-leise-abhaengigkeit-auszug', 'book', ?, ?, ?) \
         RETURNING id",
    )
    .bind(&work.title)
    .bind(&work.author)
    .bind(title_key(&work.title))
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO work_template_revision (work_id, number, body, author_name, note) VALUES (?, 1, '', 'Autor', 'Beispielwerk gesät')",
    )
    .bind(work_id)
    .execute(pool)
    .await
    .unwrap();
    let mut positions: BTreeMap<Option<usize>, i64> = BTreeMap::new();
    for node in &work.nodes {
        let position = positions.entry(node.parent).or_insert(0);
        let kind = match node.kind {
            baruch_core::NodeKind::Part => "part",
            baruch_core::NodeKind::Chapter => "chapter",
            baruch_core::NodeKind::Section => "section",
        };
        let node_id: i64 = sqlx::query_scalar(
            "INSERT INTO node (work_id, seq, parent_seq, position, number, title, kind, findings) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?) RETURNING id",
        )
        .bind(work_id)
        .bind(node.id as i64)
        .bind(node.parent.map(|p| p as i64))
        .bind(*position)
        .bind(&node.number)
        .bind(&node.title)
        .bind(kind)
        .bind(node.findings as i64)
        .fetch_one(pool)
        .await
        .unwrap();
        *position += 1;
        let mut shown: Option<u32> = None;
        for r in &node.revisions {
            // The old seed also wrote a heading line for parts.
            let body =
                if node.kind == baruch_core::NodeKind::Part { format!("= {}\n", node.title) } else { r.body.clone() };
            let (author_kind, name, model) = match &r.author {
                baruch_core::Author::Human { name } => ("human", name.clone(), None),
                baruch_core::Author::Assistant { persona, model } => {
                    ("assistant", persona.clone(), Some(model.clone()))
                }
            };
            let status = match r.status {
                RevisionStatus::Proposed => "proposed",
                RevisionStatus::Current => "current",
                RevisionStatus::Rejected => "rejected",
                RevisionStatus::Superseded => "superseded",
            };
            sqlx::query(
                "INSERT INTO revision (node_id, number, body, author_kind, author_name, model, status, note, based_on) \
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(node_id)
            .bind(i64::from(r.number))
            .bind(body)
            .bind(author_kind)
            .bind(name)
            .bind(model)
            .bind(status)
            .bind(&r.note)
            .bind(shown.map(i64::from))
            .execute(pool)
            .await
            .unwrap();
            if matches!(r.status, RevisionStatus::Current | RevisionStatus::Superseded) {
                shown = Some(r.number);
            }
        }
        for (position, post) in node.posts.iter().enumerate() {
            let post_id: i64 = sqlx::query_scalar(
                "INSERT INTO post (node_id, position, author, body, sources) VALUES (?, ?, ?, ?, ?) RETURNING id",
            )
            .bind(node_id)
            .bind(position as i64)
            .bind(&post.author)
            .bind(&post.body)
            .bind(serde_json::to_string(&post.sources).unwrap())
            .fetch_one(pool)
            .await
            .unwrap();
            for (i, r) in post.refs.iter().enumerate() {
                sqlx::query("INSERT INTO post_ref (post_id, position, node_seq, rev, quote) VALUES (?, ?, ?, ?, ?)")
                    .bind(post_id)
                    .bind(i as i64)
                    .bind(r.node as i64)
                    .bind(i64::from(r.rev))
                    .bind(&r.quote)
                    .execute(pool)
                    .await
                    .unwrap();
            }
        }
    }
    for (position, p) in work.personas.iter().enumerate() {
        sqlx::query(
            "INSERT INTO persona (handle, name, role, model, context_length, active, position) VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&p.handle)
        .bind(&p.name)
        .bind(&p.role)
        .bind(&p.model)
        .bind(i64::from(p.context_length))
        .bind(p.active as i64)
        .bind(position as i64)
        .execute(pool)
        .await
        .unwrap();
    }
    work_id
}

/// Freely invented book with 87 outline points: six chapters with eleven sections and two subsections each, a
/// seventh with two sections. With escaping, label and comment.
fn invented_book() -> String {
    let mut out = String::from(
        "#set document(title: \"Das Uhrmacherbuch\", author: \"Ida Feder\")\n#set heading(numbering: \"I.1\")\n\
         #let werkbank(body) = block(inset: 4pt, body)\n\n",
    );
    for chapter in 1..=7 {
        out.push_str(&format!("= Zahnrad {chapter}\n\nDie Unruh schwingt {chapter} Mal je Atemzug.\n\n"));
        let sections = if chapter == 7 { 2 } else { 11 };
        for section in 1..=sections {
            if section == 3 {
                out.push_str(&format!("== Feder\\'s Lager {chapter} <lager-{chapter}> // Notiz\n\n"));
            } else {
                out.push_str(&format!("== Hemmung {chapter}.{section}\n\n"));
            }
            out.push_str(&format!("#werkbank[Stift {section} greift in Rad {chapter}.]\n\n"));
            if section == 1 && chapter < 7 {
                for sub in 1..=2 {
                    out.push_str(&format!("=== Kloben {chapter}.{section}.{sub}\n\nMessing, poliert.\n\n"));
                }
            }
        }
    }
    out
}

/// Import as in development: preamble → work template (revision 1), headings → nodes with revision 1 by the human,
/// numbers „1“, „1.1“ (display after the template comes when loading).
async fn insert_old_import(pool: &SqlitePool, source: &str) -> (WorkId, usize) {
    let doc = baruch_typst::parse(source).unwrap();
    let title = doc.title.clone().unwrap();
    let work: WorkId = sqlx::query_scalar(
        "INSERT INTO work (slug, kind, title, author, template, source_name, source_sha256, title_key) \
         VALUES ('das-uhrmacherbuch', 'book', ?, ?, ?, 'uhrmacherbuch.typ', 'erfundene-pruefsumme', ?) RETURNING id",
    )
    .bind(&title)
    .bind(doc.author.clone().unwrap())
    .bind(&doc.template)
    .bind(title_key(&title))
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO work_template_revision (work_id, number, body, author_name, note) VALUES (?, 1, ?, 'Autor', 'Import aus uhrmacherbuch.typ')",
    )
    .bind(work)
    .bind(&doc.template)
    .execute(pool)
    .await
    .unwrap();
    let mut stack: Vec<(u8, usize, String, usize)> = Vec::new();
    let mut roots = 0usize;
    for (seq, heading) in doc.headings.iter().enumerate() {
        while stack.last().is_some_and(|&(level, ..)| level >= heading.level) {
            stack.pop();
        }
        let (parent, position, number) = match stack.last_mut() {
            Some((_, parent, parent_number, children)) => {
                *children += 1;
                (Some(*parent), *children - 1, format!("{parent_number}.{children}"))
            }
            None => {
                roots += 1;
                (None, roots - 1, roots.to_string())
            }
        };
        let kind = if heading.level == 1 { "chapter" } else { "section" };
        let node_id: i64 = sqlx::query_scalar(
            "INSERT INTO node (work_id, seq, parent_seq, position, number, title, kind) VALUES (?, ?, ?, ?, ?, ?, ?) RETURNING id",
        )
        .bind(work)
        .bind(seq as i64)
        .bind(parent.map(|p| p as i64))
        .bind(position as i64)
        .bind(&number)
        .bind(&heading.title)
        .bind(kind)
        .fetch_one(pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO revision (node_id, number, body, author_kind, author_name, model, status, note) \
             VALUES (?, 1, ?, 'human', 'Autor', NULL, 'current', 'Import aus uhrmacherbuch.typ')",
        )
        .bind(node_id)
        .bind(&heading.body)
        .execute(pool)
        .await
        .unwrap();
        stack.push((heading.level, seq, number, 0));
    }
    (work, doc.headings.len())
}

/// Proposal, post with reference, library title, quote and highlight in work 2.
async fn insert_extras(pool: &SqlitePool, work: WorkId) {
    let node: i64 = sqlx::query_scalar("SELECT id FROM node WHERE work_id = ? AND seq = 2")
        .bind(work)
        .fetch_one(pool)
        .await
        .unwrap();
    let body: String = sqlx::query_scalar("SELECT body FROM revision WHERE node_id = ? AND number = 1")
        .bind(node)
        .fetch_one(pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO revision (node_id, number, body, author_kind, author_name, model, status, note, based_on) \
         VALUES (?, 2, ?, 'assistant', '@lektor', 'probe-modell', 'proposed', 'Ein Satz mehr.', 1)",
    )
    .bind(node)
    .bind(format!("{body}Der Anker hält das Rad.\n"))
    .execute(pool)
    .await
    .unwrap();
    let post: i64 = sqlx::query_scalar(
        "INSERT INTO post (node_id, position, author, body, sources) VALUES (?, 0, '@nathan', 'Befund: Wie oft schwingt die Unruh?', '[\"Uhrenlexikon\"]') RETURNING id",
    )
    .bind(node)
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO post_ref (post_id, position, node_seq, rev, quote) VALUES (?, 0, 2, 1, 'Messing, poliert.')",
    )
    .bind(post)
    .execute(pool)
    .await
    .unwrap();
    let title: i64 = sqlx::query_scalar(
        "INSERT INTO library_title (key, kind, title, authors, year) VALUES ('feder1901', 'book', 'Vom Gang der Räder', 'Feder, Ida', 1901) RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    let quote: i64 = sqlx::query_scalar(
        "INSERT INTO library_quote (title_id, position, text, page) VALUES (?, 0, 'Jede Hemmung braucht Geduld.', 'S. 7') RETURNING id",
    )
    .bind(title)
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO library_highlight (title_id, quote_id, start_char, end_char, text, kind, author_kind, author_name, status) \
         VALUES (?, ?, 5, 12, 'Hemmung', 'core', 'human', 'Autor', 'active')",
    )
    .bind(title)
    .bind(quote)
    .execute(pool)
    .await
    .unwrap();
}

struct Development {
    dir: tempfile::TempDir,
    sample: WorkId,
    book: WorkId,
    nodes: usize,
    source: String,
}

async fn development_database() -> Development {
    let dir = tempfile::tempdir().unwrap();
    let pool = raw_pool(dir.path()).await;
    migrate_like_development(&pool).await;
    let sample = insert_old_sample(&pool).await;
    let source = invented_book();
    let (book, nodes) = insert_old_import(&pool, &source).await;
    insert_extras(&pool, book).await;
    pool.close().await;
    Development { dir, sample, book, nodes, source }
}

/// Store migrations up to and including 0007 (state after Round 5).
fn up_to_0007() -> Migrator {
    let migrations: Vec<_> = baruch_store::MIGRATOR.iter().filter(|m| m.version <= 7).cloned().collect();
    assert!(migrations.iter().any(|m| m.version == 7), "0007 gehört dazu");
    Migrator { migrations: Cow::Owned(migrations), ignore_missing: true, ..Migrator::DEFAULT }
}

// ---- Dumps ------------------------------------------------------------------------------------

/// Schema from `sqlite_master`: name → SQL (without `sqlite_stat*`).
async fn schema(pool: &SqlitePool) -> BTreeMap<String, Option<String>> {
    let rows: Vec<(String, Option<String>)> =
        sqlx::query_as("SELECT name, sql FROM sqlite_master WHERE name NOT LIKE 'sqlite_stat%'")
            .fetch_all(pool)
            .await
            .unwrap();
    rows.into_iter().collect()
}

/// All rows of all tables (also the full-text index and its shadow tables) as `quote()` text, sorted per table.
/// Column names in their order stand as the first line.
async fn contents(pool: &SqlitePool) -> BTreeMap<String, Vec<String>> {
    let tables: Vec<String> = sqlx::query_scalar("SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name")
        .fetch_all(pool)
        .await
        .unwrap();
    let mut out = BTreeMap::new();
    for table in tables {
        let columns: Vec<String> = sqlx::query_scalar("SELECT name FROM pragma_table_info(?) ORDER BY cid")
            .bind(&table)
            .fetch_all(pool)
            .await
            .unwrap();
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
async fn migration_0007_keeps_every_row_and_column_and_makes_ids_autoincrement() {
    let dev = development_database().await;
    assert!((85..=90).contains(&dev.nodes), "Werk 2 hat etwa 87 Gliederungspunkte: {}", dev.nodes);
    assert_eq!((dev.sample, dev.book), (1, 2), "Werk 1 Beispielwerk, Werk 2 importiertes Buch");
    let pool = raw_pool(dev.dir.path()).await;
    let schema_before = schema(&pool).await;
    let before = contents(&pool).await;
    pool.close().await;
    assert!(
        !schema_before["work"].as_deref().unwrap().contains("AUTOINCREMENT"),
        "vorher vergibt SQLite ids ohne Sperre: {:?}",
        schema_before["work"]
    );

    // First only up to 0007 (Round 6): later migrations may add columns and tables without this comparison
    // breaking; they have tests of their own (migration_0008.rs …).
    let pool = raw_pool(dev.dir.path()).await;
    up_to_0007().run(&pool).await.expect("Migrator bis 0007 läuft");
    let after = contents(&pool).await;
    for (table, rows) in &before {
        if table == "_sqlx_migrations" {
            continue;
        }
        assert_eq!(after.get(table), Some(rows), "Tabelle {table}: alle Zeilen und Spalten unverändert");
    }

    let schema_after = schema(&pool).await;
    let work_sql = schema_after["work"].as_deref().unwrap();
    assert!(
        work_sql.contains("id            INTEGER PRIMARY KEY AUTOINCREMENT"),
        "work.id mit AUTOINCREMENT: {work_sql}"
    );
    for (name, sql) in &schema_before {
        if name == "work" {
            continue;
        }
        assert_eq!(schema_after.get(name), Some(sql), "Schema von {name} unverändert (auch die Indizes von work)");
    }
    for name in NEW_TABLES {
        assert!(schema_after.contains_key(*name), "0007 legt {name} an");
    }
    assert!(
        schema_after.keys().all(|k| schema_before.contains_key(k)
            || NEW_TABLES.contains(&k.as_str())
            || k.starts_with("sqlite_autoindex_")),
        "keine Reste wie work_new oder fk_guard: {:?}",
        schema_after.keys().collect::<Vec<_>>()
    );
    pool.close().await;

    let store = Store::open(dev.dir.path()).await.expect("Store::open führt die übrigen Migrationen aus");
    let pool = store.pool();
    let versions: Vec<i64> =
        sqlx::query_scalar("SELECT version FROM _sqlx_migrations ORDER BY version").fetch_all(pool).await.unwrap();
    let known: Vec<i64> = baruch_store::MIGRATOR.iter().map(|m| m.version).collect();
    assert!(known.iter().all(|v| versions.contains(v)), "alle Store-Migrationen gelaufen: {versions:?}");
    assert!(versions.contains(&105), "Bibliotheks-Migrationen bleiben eingetragen: {versions:?}");

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
    assert_eq!(
        baruch_typst::xref::strip_key_labels(&store.export_typst(dev.book).await.unwrap()).into_owned(),
        dev.source,
        "Export von Werk 2 byte-gleich mit der Quelle"
    );
    assert_eq!(
        count(
            pool,
            "SELECT count(*) FROM search_fts WHERE search_fts MATCH 'unruh' AND kind = 'revision' AND work_id = 2"
        )
        .await,
        7,
        "Volltextsuche findet die Unruh weiter in allen sieben Kapiteln von Werk 2"
    );
    assert_eq!(
        count(pool, "SELECT count(*) FROM search_fts WHERE search_fts MATCH 'geduld'").await,
        1,
        "und das Zitat der Bibliothek"
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT value FROM store_meta WHERE key = 'seeded'")
            .fetch_optional(pool)
            .await
            .unwrap(),
        Some("1".to_owned()),
        "eine Datenbank mit Werken gilt nach 0007 als gesät"
    );
    assert_eq!(store.works().await.unwrap().len(), 2, "kein zweites Beispielwerk gesät");
    let template_revisions = count(pool, "SELECT count(*) FROM work_template_revision").await;
    assert_eq!(template_revisions, 2, "Vorlagenfassungen beider Werke bleiben");

    // A second opening changes nothing more, not on the new tables either.
    let schema_once = schema(pool).await;
    let once = contents(pool).await;
    drop(store);
    let store = Store::open(dev.dir.path()).await.unwrap();
    assert_eq!(schema(store.pool()).await, schema_once, "zweites Öffnen: Schema gleich");
    assert_eq!(contents(store.pool()).await, once, "zweites Öffnen: alle Zeilen gleich, auch _sqlx_migrations");

    // Delete work 2 and create it anew: new id 3, the old slug stays reserved.
    let deleted = store.delete_work(dev.book, BOOK_TITLE).await.unwrap();
    assert_eq!(deleted.slug, "das-uhrmacherbuch", "das gelöschte Werk trug diesen slug");
    let retired: Vec<(i64, String, String)> =
        sqlx::query_as("SELECT id, slug, title FROM work_retired").fetch_all(store.pool()).await.unwrap();
    assert_eq!(
        retired,
        vec![(2, "das-uhrmacherbuch".to_owned(), BOOK_TITLE.to_owned())],
        "work_retired merkt sich id, slug und Titel"
    );
    let new = NewWork {
        title: BOOK_TITLE,
        author: "Ida Feder",
        kind: baruch_core::WorkKind::Book,
        preset: Preset::Book,
        subtitle: "",
        audience: "",
        bible: "",
        language: Default::default(),
    };
    let created = store.create_work(&new, "Autor").await.unwrap();
    assert_eq!(created.work, 3, "AUTOINCREMENT: nach dem Löschen von Werk 2 bekommt das neue Werk 3, nicht 2");
    assert_eq!(created.slug, "das-uhrmacherbuch-2", "der slug des gelöschten Werks wird nicht vergeben");
    let reimported = store.import_typst(&dev.source, "uhrmacherbuch.typ", "Autor").await;
    assert!(reimported.is_err(), "derselbe Titel ist wieder vergeben (an Werk 3): {reimported:?}");
    let other = store
        .import_typst_titled(&dev.source, "uhrmacherbuch.typ", "Autor", "zweiter Import", Some("Uhren II"))
        .await
        .unwrap();
    assert_eq!(other.work, 4, "auch der Import zählt weiter");
}

#[tokio::test]
async fn a_foreign_key_violation_stops_migration_0007_and_nothing_is_applied() {
    let dev = development_database().await;
    let pool = raw_pool(dev.dir.path()).await;
    // Orphaned template revision of a work that does not exist (only possible with foreign keys switched off, e.g.
    // from a database repaired by hand).
    sqlx::query("PRAGMA foreign_keys = OFF").execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO work_template_revision (work_id, number, body, author_name) VALUES (99, 1, '', 'Autor')")
        .execute(&pool)
        .await
        .unwrap();
    let before = contents(&pool).await;
    let schema_before = schema(&pool).await;
    pool.close().await;

    let err = Store::open(dev.dir.path()).await.expect_err("Verstoß gegen einen Fremdschlüssel → Migration scheitert");
    assert!(err.to_string().contains("CHECK constraint failed"), "die Wache fk_guard schlägt an: {err}");

    let pool = raw_pool(dev.dir.path()).await;
    assert_eq!(
        schema(&pool).await,
        schema_before,
        "nichts übernommen: Schema wie vorher (kein store_meta, work ohne AUTOINCREMENT)"
    );
    assert_eq!(
        contents(&pool).await,
        before,
        "nichts übernommen: alle Zeilen wie vorher, auch _sqlx_migrations ohne 7"
    );
}

#[tokio::test]
async fn an_empty_database_from_before_0007_is_seeded_but_one_with_only_personas_is_not() {
    let empty = tempfile::tempdir().unwrap();
    let pool = raw_pool(empty.path()).await;
    migrate_like_development(&pool).await;
    pool.close().await;
    let store = Store::open(empty.path()).await.unwrap();
    assert_eq!(
        store.works().await.unwrap().len(),
        1,
        "leere Datenbank: 0007 setzt kein seeded, das Beispielwerk wird gesät"
    );

    let personas = tempfile::tempdir().unwrap();
    let pool = raw_pool(personas.path()).await;
    migrate_like_development(&pool).await;
    sqlx::query("INSERT INTO persona (handle, name, role, model, context_length, position) VALUES ('@probe', 'Probe', 'Test', 'm', 1, 0)")
        .execute(&pool)
        .await
        .unwrap();
    pool.close().await;
    let store = Store::open(personas.path()).await.unwrap();
    assert!(
        store.works().await.unwrap().is_empty(),
        "Personas, aber keine Werke (alle gelöscht): die Datenbank gilt als gesät, kein neues Beispielwerk"
    );
}

/// Evidence for the chosen way, updated in Round 36: sqlx-sqlite 0.8.6 recognised `-- no-transaction` (`no_tx`),
/// but ran the migration in a transaction nevertheless – an own `BEGIN IMMEDIATE` failed, and `PRAGMA
/// foreign_keys=OFF` had no effect there. 0007 therefore runs with `PRAGMA defer_foreign_keys=ON` in the
/// transaction of sqlx. **sqlx-sqlite 0.9 respects the marker** (`sqlx-sqlite-0.9.1/src/migrate.rs`, `apply`): a
/// migration marked this way runs without a transaction. For Baruch that changes nothing, because no store
/// migration carries the marker – each still runs in the transaction of sqlx, and 0040 still needs
/// `baruch_store::migrate` with `FOREIGN_KEYS_OFF`.
#[tokio::test]
async fn sqlx_09_honours_no_transaction_but_no_store_migration_carries_the_mark() {
    let dir = tempfile::tempdir().unwrap();
    let migrations = dir.path().join("probe");
    std::fs::create_dir(&migrations).unwrap();
    std::fs::write(
        migrations.join("0001_probe.sql"),
        "-- no-transaction\nPRAGMA foreign_keys=OFF;\nBEGIN IMMEDIATE;\nCREATE TABLE probe (x);\nCOMMIT;\nPRAGMA foreign_keys=ON;\n",
    )
    .unwrap();
    std::fs::write(migrations.join("0002_plain.sql"), "BEGIN IMMEDIATE;\nCREATE TABLE plain (x);\nCOMMIT;\n").unwrap();
    let migrator = Migrator::new(migrations.as_path()).await.unwrap();
    let marks: Vec<bool> = migrator.iter().map(|m| m.no_tx).collect();
    assert_eq!(marks, [true, false], "sqlx liest die Markierung -- no-transaction (nur in der ersten Datei)");
    let pool = raw_pool(dir.path()).await;
    let err = migrator.run(&pool).await.expect_err("BEGIN in der Transaktion von sqlx (zweite Datei, ohne Markierung)");
    assert!(err.to_string().contains("within a transaction"), "SQLite lehnt die verschachtelte Transaktion ab: {err}");
    assert_eq!(
        count(&pool, "SELECT count(*) FROM sqlite_master WHERE name = 'probe'").await,
        1,
        "mit Markierung läuft die Migration unter sqlx 0.9 ohne Transaktion: das eigene BEGIN gelingt (unter 0.8 nicht)"
    );
    assert_eq!(
        count(&pool, "SELECT count(*) FROM sqlite_master WHERE name = 'plain'").await,
        0,
        "ohne Markierung bleibt es wie bisher: Die Migration läuft in der Transaktion von sqlx, ein eigenes BEGIN \
         scheitert und nichts entsteht"
    );
    assert!(
        baruch_store::MIGRATOR.iter().all(|m| !m.no_tx),
        "keine Store-Migration (auch 0007 nicht) trägt die Markierung – der Wechsel im Verhalten von sqlx trifft Baruch \
         nicht"
    );
    assert!(!baruch_store::MIGRATOR.no_tx, "auch der Migrator als Ganzes verzichtet nicht auf die Transaktion");
}
