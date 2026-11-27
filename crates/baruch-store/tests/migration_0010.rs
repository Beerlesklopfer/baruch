// SPDX-License-Identifier: AGPL-3.0-or-later
//! Migration 0010 (Farbe je Persona, Vertrag Runde 7, „proposals“) auf einer nachgebauten Datenbank im
//! Stand von `development` vor Runde 7: Store-Migrationen 0001–0009 und Bibliotheks-Migrationen
//! 0101–0105, Werk 1 = Beispielwerk mit Personas, Werk 2 = importiertes, frei erfundenes Buch mit
//! Vorschlag, Beitrag mit Verweis, Bild (Fassung, Pfad) und Bibliothek (Titel, Zitat, Hervorhebung).
//!
//! Verglichen wird nach einem Migrator bis 0010 (spätere Migrationen wie 0011 Einfälle stören so den
//! Schemavergleich nicht): Alle Zeilen aller vorhandenen Tabellen bleiben gleich (über die Spalten von
//! vorher), nur `persona` bekommt die Spalte `color` (überall NULL = Vorgabe), keine Verstöße gegen
//! Fremdschlüssel, je Abschnitt genau eine aktuelle Fassung. Danach `Store::open` mit allen
//! Migrationen: Export byte-gleich, Vorgabefarben nach Reihenfolge, ein zweiter Start ändert nichts.

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::path::Path;
use std::str::FromStr;

use baruch_core::{RevisionStatus, sample};
use baruch_store::{PERSONA_PALETTE, Store, WorkId, title_key};
use sqlx::SqlitePool;
use sqlx::migrate::Migrator;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

async fn raw_pool(dir: &Path) -> SqlitePool {
    let options = SqliteConnectOptions::from_str("sqlite:")
        .unwrap()
        .filename(dir.join("baruch.sqlite"))
        .create_if_missing(true)
        .foreign_keys(true);
    SqlitePoolOptions::new().max_connections(1).connect_with(options).await.unwrap()
}

/// Store-Migrationen bis 0009 (Stand development) und die Bibliotheks-Migrationen 0101–0105.
async fn migrate_like_development(pool: &SqlitePool) {
    let old: Vec<_> = baruch_store::MIGRATOR.iter().filter(|m| m.version <= 9).cloned().collect();
    assert!(old.iter().any(|m| m.version == 9), "Stand development hat 0009");
    assert!(baruch_store::MIGRATOR.iter().any(|m| m.version == 10), "die neue Migration ist 0010");
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

/// Ein Werk mit Gliederung, je Abschnitt Revisionen wie in `sample::work()` bzw. Revision 1.
async fn insert_work(pool: &SqlitePool, slug: &str, title: &str, template: &str, source_name: Option<&str>) -> WorkId {
    let work: WorkId = sqlx::query_scalar(
        "INSERT INTO work (slug, kind, title, author, template, source_name, source_sha256, title_key) \
         VALUES (?, 'book', ?, 'Ida Feder', ?, ?, ?, ?) RETURNING id",
    )
    .bind(slug)
    .bind(title)
    .bind(template)
    .bind(source_name)
    .bind(source_name.map(|_| "erfundene-pruefsumme"))
    .bind(title_key(title))
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO work_template_revision (work_id, number, body, author_name, note) VALUES (?, 1, ?, 'Autor', 'Anfang')")
        .bind(work)
        .bind(template)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO store_meta (key, value) VALUES ('seeded', '1') ON CONFLICT DO NOTHING")
        .execute(pool)
        .await
        .unwrap();
    work
}

async fn insert_sample(pool: &SqlitePool) -> WorkId {
    let work = sample::work();
    let work_id = insert_work(pool, "beispielwerk-leise-abhaengigkeit-auszug", &work.title, "", None).await;
    let mut positions: BTreeMap<Option<usize>, i64> = BTreeMap::new();
    for node in &work.nodes {
        let position = positions.entry(node.parent).or_insert(0);
        let kind = match node.kind {
            baruch_core::NodeKind::Part => "part",
            baruch_core::NodeKind::Chapter => "chapter",
            baruch_core::NodeKind::Section => "section",
        };
        let node_id: i64 = sqlx::query_scalar(
            "INSERT INTO node (work_id, seq, parent_seq, position, number, title, kind) VALUES (?, ?, ?, ?, ?, ?, ?) RETURNING id",
        )
        .bind(work_id)
        .bind(node.id as i64)
        .bind(node.parent.map(|p| p as i64))
        .bind(*position)
        .bind(&node.number)
        .bind(&node.title)
        .bind(kind)
        .fetch_one(pool)
        .await
        .unwrap();
        *position += 1;
        let mut shown: Option<u32> = None;
        for r in &node.revisions {
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
            .bind(&r.body)
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
            sqlx::query("INSERT INTO post (node_id, position, author, body, sources) VALUES (?, ?, ?, ?, ?)")
                .bind(node_id)
                .bind(position as i64)
                .bind(&post.author)
                .bind(&post.body)
                .bind(serde_json::to_string(&post.sources).unwrap())
                .execute(pool)
                .await
                .unwrap();
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

/// Frei erfundenes Buch mit Vorlage, Bildaufruf, Kapiteln und Abschnitten.
fn invented_book() -> String {
    let mut out = String::from(
        "#set document(title: \"Das Mühlenbuch\", author: \"Ida Feder\")\n#set heading(numbering: \"1.1\")\n\
         #let rad(body) = block(inset: 4pt, body)\n\n",
    );
    for chapter in 1..=4 {
        out.push_str(&format!("= Mühlrad {chapter}\n\nDas Wasser fällt {chapter} Ellen tief.\n\n"));
        out.push_str(&format!("#figure(image(\"/bilder/rad-{chapter}.svg\"), caption: [Rad {chapter}])\n\n"));
        for section in 1..=3 {
            out.push_str(&format!("== Schaufel {chapter}.{section}\n\n#rad[Eiche, geölt.]\n\n"));
        }
    }
    out
}

async fn insert_import(pool: &SqlitePool, source: &str) -> WorkId {
    let doc = baruch_typst::parse(source).unwrap();
    let work = insert_work(pool, "das-muehlenbuch", "Das Mühlenbuch", &doc.template, Some("muehlenbuch.typ")).await;
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
             VALUES (?, 1, ?, 'human', 'Autor', NULL, 'current', 'Import aus muehlenbuch.typ')",
        )
        .bind(node_id)
        .bind(&heading.body)
        .execute(pool)
        .await
        .unwrap();
        stack.push((heading.level, seq, number, 0));
    }
    // Vorschlag, Beitrag mit Verweis, Bibliothek.
    let node: i64 = sqlx::query_scalar("SELECT id FROM node WHERE work_id = ? AND seq = 1")
        .bind(work)
        .fetch_one(pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO revision (node_id, number, body, author_kind, author_name, model, status, note, based_on) \
         SELECT node_id, 2, body || 'Ein Satz mehr.\n', 'assistant', '@lektor', 'probe-modell', 'proposed', 'Vorschlag', 1 \
           FROM revision WHERE node_id = ? AND number = 1",
    )
    .bind(node)
    .execute(pool)
    .await
    .unwrap();
    let post: i64 = sqlx::query_scalar(
        "INSERT INTO post (node_id, position, author, body, sources) VALUES (?, 0, '@nathan', 'Wie tief fällt das Wasser?', '[]') RETURNING id",
    )
    .bind(node)
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO post_ref (post_id, position, node_seq, rev, quote) VALUES (?, 0, 1, 1, 'Eiche, geölt.')")
        .bind(post)
        .execute(pool)
        .await
        .unwrap();
    let title: i64 = sqlx::query_scalar(
        "INSERT INTO library_title (key, kind, title, authors, year) VALUES ('feder1902', 'book', 'Vom Lauf des Wassers', 'Feder, Ida', 1902) RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    let quote: i64 = sqlx::query_scalar(
        "INSERT INTO library_quote (title_id, position, text, page) VALUES (?, 0, 'Jedes Rad braucht Wasser.', 'S. 3') RETURNING id",
    )
    .bind(title)
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO library_highlight (title_id, quote_id, start_char, end_char, text, kind, author_kind, author_name, status) \
         VALUES (?, ?, 5, 8, 'Rad', 'core', 'human', 'Autor', 'active')",
    )
    .bind(title)
    .bind(quote)
    .execute(pool)
    .await
    .unwrap();
    work
}

/// Schema aus `sqlite_master`: Name → SQL.
async fn schema(pool: &SqlitePool) -> BTreeMap<String, Option<String>> {
    let rows: Vec<(String, Option<String>)> =
        sqlx::query_as("SELECT name, sql FROM sqlite_master WHERE name NOT LIKE 'sqlite_stat%'")
            .fetch_all(pool)
            .await
            .unwrap();
    rows.into_iter().collect()
}

/// Spalten jeder Tabelle in ihrer Reihenfolge.
async fn columns(pool: &SqlitePool) -> BTreeMap<String, Vec<String>> {
    let tables: Vec<String> = sqlx::query_scalar("SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name")
        .fetch_all(pool)
        .await
        .unwrap();
    let mut out = BTreeMap::new();
    for table in tables {
        let names: Vec<String> = sqlx::query_scalar("SELECT name FROM pragma_table_info(?) ORDER BY cid")
            .bind(&table)
            .fetch_all(pool)
            .await
            .unwrap();
        out.insert(table, names);
    }
    out
}

/// Alle Zeilen der Tabellen in `layout` über genau diese Spalten, als `quote()`-Text, sortiert.
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

async fn count(pool: &SqlitePool, sql: &str) -> i64 {
    sqlx::query_scalar(sqlx::AssertSqlSafe(sql)).fetch_one(pool).await.unwrap()
}

/// Ein Bild (SVG, eine Fassung) mit Pfad `/bilder/rad-1.svg` im Werk, wie es 0009 speichert.
async fn insert_asset(pool: &SqlitePool, work: WorkId) {
    let svg = "<svg xmlns=\"http://www.w3.org/2000/svg\"><circle r=\"2\"/></svg>";
    let asset: i64 = sqlx::query_scalar("INSERT INTO asset (work_id, name) VALUES (?, 'rad-1') RETURNING id")
        .bind(work)
        .fetch_one(pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO asset_revision (asset_id, number, mime, encoding, content, bytes, sha256, caption, alt, author_name, status) \
         VALUES (?, 1, 'image/svg+xml', 'svg', ?, ?, 'erfundene-pruefsumme', 'Rad 1', 'Ein Mühlrad', 'Autor', 'current')",
    )
    .bind(asset)
    .bind(svg)
    .bind(svg.len() as i64)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO asset_path (work_id, path, asset_id) VALUES (?, '/bilder/rad-1.svg', ?)")
        .bind(work)
        .bind(asset)
        .execute(pool)
        .await
        .unwrap();
}

#[tokio::test]
async fn migration_0010_keeps_every_row_and_adds_only_the_persona_color() {
    let dir = tempfile::tempdir().unwrap();
    let pool = raw_pool(dir.path()).await;
    migrate_like_development(&pool).await;
    let sample = insert_sample(&pool).await;
    let source = invented_book();
    let book = insert_import(&pool, &source).await;
    insert_asset(&pool, book).await;
    assert_eq!((sample, book), (1, 2), "Werk 1 Beispielwerk, Werk 2 importiertes Buch");
    let schema_before = schema(&pool).await;
    let layout = columns(&pool).await;
    assert!(!layout["persona"].contains(&"color".to_owned()), "vorher ohne Spalte color: {:?}", layout["persona"]);
    let before = contents(&pool, &layout).await;
    for (table, minimum) in
        [("persona", 6), ("revision", 10), ("post", 3), ("asset_revision", 1), ("library_highlight", 1)]
    {
        assert!(before[table].len() >= minimum, "Testdaten: {table} hat mindestens {minimum} Zeilen");
    }

    let upto_0010: Vec<_> = baruch_store::MIGRATOR.iter().filter(|m| m.version <= 10).cloned().collect();
    let migrator = Migrator { migrations: Cow::Owned(upto_0010), ignore_missing: true, ..Migrator::DEFAULT };
    migrator.run(&pool).await.expect("Migrator bis 0010 führt 0010 aus");
    let after = contents(&pool, &layout).await;
    for (table, rows) in &before {
        if table == "_sqlx_migrations" {
            continue;
        }
        assert_eq!(after.get(table), Some(rows), "Tabelle {table}: alle Zeilen und (alten) Spalten unverändert");
    }
    let schema_after = schema(&pool).await;
    assert_eq!(
        schema_after.keys().collect::<Vec<_>>(),
        schema_before.keys().collect::<Vec<_>>(),
        "0010 legt keine Tabelle, keinen Index und keinen Trigger an"
    );
    for (name, sql) in &schema_before {
        if name != "persona" {
            assert_eq!(schema_after.get(name), Some(sql), "Schema von {name} unverändert");
        }
    }
    let persona_sql = schema_after["persona"].as_deref().unwrap();
    assert!(persona_sql.contains("color TEXT CHECK"), "persona bekommt color mit CHECK: {persona_sql}");
    assert_eq!(
        count(&pool, "SELECT count(*) FROM persona WHERE color IS NOT NULL").await,
        0,
        "keine Datenänderung: alle Farben NULL (Vorgabe)"
    );
    for bad in ["#ABCDEF", "#abc", "rot", "#12345g", "#1234567"] {
        let refused =
            sqlx::query("UPDATE persona SET color = ? WHERE handle = '@lektor'").bind(bad).execute(&pool).await;
        assert!(refused.is_err(), "CHECK lehnt {bad:?} ab (nur #rrggbb klein)");
    }
    assert_eq!(
        count(&pool, "SELECT count(*) FROM pragma_foreign_key_check").await,
        0,
        "keine Verstöße gegen Fremdschlüssel"
    );
    assert_eq!(
        count(
            &pool,
            "SELECT count(*) FROM node WHERE (SELECT count(*) FROM revision r WHERE r.node_id = node.id AND r.status = 'current') <> 1"
        )
        .await,
        0,
        "je Abschnitt genau eine aktuelle Fassung"
    );
    pool.close().await;

    let store = Store::open(dir.path()).await.expect("Store::open führt die übrigen Migrationen aus");
    assert_eq!(
        baruch_typst::xref::strip_key_labels(&store.export_typst(book).await.unwrap()).into_owned(),
        source,
        "Export von Werk 2 byte-gleich mit der Quelle"
    );
    assert_eq!(store.works().await.unwrap().len(), 2, "kein Beispielwerk zusätzlich gesät");
    let colors = store.persona_colors().await.unwrap();
    let handles: Vec<&str> = colors.iter().map(|c| c.handle.as_str()).collect();
    let expected: Vec<String> = sample::work().personas.iter().map(|p| p.handle.clone()).collect();
    assert_eq!(handles, expected, "Reihenfolge der Personas nach position");
    for (index, c) in colors.iter().enumerate() {
        assert_eq!(c.color, PERSONA_PALETTE[index % PERSONA_PALETTE.len()], "{}: Vorgabe nach Reihenfolge", c.handle);
        assert!(!c.chosen, "{}: nach der Migration keine eigene Farbe", c.handle);
    }

    // Zweiter Start ändert nichts.
    let layout_now = columns(store.pool()).await;
    let schema_once = schema(store.pool()).await;
    let once = contents(store.pool(), &layout_now).await;
    drop(store);
    let store = Store::open(dir.path()).await.unwrap();
    assert_eq!(schema(store.pool()).await, schema_once, "zweites Öffnen: Schema gleich");
    assert_eq!(
        contents(store.pool(), &layout_now).await,
        once,
        "zweites Öffnen: alle Zeilen gleich, auch _sqlx_migrations"
    );
}
