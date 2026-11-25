// SPDX-License-Identifier: AGPL-3.0-or-later
//! Migration 0018 (Anträge zu Bildern, Runde 11) auf einer nachgebauten Datenbank im Stand von
//! development davor: Store-Migrationen 0001–0016 und Bibliothek 0101 ff., ein frei erfundenes Werk mit
//! Kapiteln, Personas, Beiträgen, Einfällen, einem Bild, einem Gliederungsantrag und einem MCP-Token.
//!
//! Danach `Store::open`: Jede Zeile jeder Tabelle bleibt unverändert, das Schema jeder alten Tabelle
//! ebenso; dazu kommt die leere Tabelle `asset_request` mit ihrem Index. Ein zweites Öffnen ändert nichts
//! mehr; ein Werk mit Bildantrag lässt sich löschen (die Anträge gehen mit). Andere Migrationen derselben
//! oder späterer Runden (z. B. 0017) laufen beim Öffnen mit; der Test nimmt nicht an, dass 0018 die
//! letzte ist, und vergleicht nur, was 0018 betrifft.

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::path::Path;
use std::str::FromStr;

use baruch_store::{AssetOp, IdeaOrigin, NewAsset, Store, title_key};
use sqlx::SqlitePool;
use sqlx::migrate::Migrator;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

const SOURCE: &str = "#set document(title: \"Die Laterne am Kai\", author: \"Ida Morgenstern\")\n\n\
= Abend\n\nDie Fähre legt an.\n\n= Nacht\n\nNur die Laterne brennt noch.\n";

async fn raw_pool(dir: &Path) -> SqlitePool {
    let options = SqliteConnectOptions::from_str("sqlite:")
        .unwrap()
        .filename(dir.join("baruch.sqlite"))
        .create_if_missing(true)
        .foreign_keys(true);
    SqlitePoolOptions::new().max_connections(1).connect_with(options).await.unwrap()
}

/// Store bis 0016 (Stand development vor Runde 11) und die Bibliothek.
async fn migrate_before(pool: &SqlitePool) {
    let old: Vec<_> = baruch_store::MIGRATOR.iter().filter(|m| m.version <= 16).cloned().collect();
    assert!(old.iter().any(|m| m.version == 16), "Stand vorher hat 0016");
    let store = Migrator { migrations: Cow::Owned(old), ignore_missing: true, ..Migrator::DEFAULT };
    store.run(pool).await.unwrap();
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../baruch-library/migrations");
    let mut library = Migrator::new(dir).await.unwrap();
    library.ignore_missing = true;
    library.run(pool).await.unwrap();
}

async fn exec(pool: &SqlitePool, sql: &str) {
    sqlx::query(sql).execute(pool).await.unwrap_or_else(|e| panic!("{sql}: {e}"));
}

async fn insert_rows(pool: &SqlitePool) -> i64 {
    let doc = baruch_typst::parse(SOURCE).unwrap();
    let work: i64 = sqlx::query_scalar(
        "INSERT INTO work (slug, kind, title, author, template, source_name, source_sha256, title_key) \
         VALUES ('die-laterne-am-kai', 'book', 'Die Laterne am Kai', 'Ida Morgenstern', ?, 'kai.typ', 'erfunden', ?) RETURNING id",
    )
    .bind(&doc.template)
    .bind(title_key("Die Laterne am Kai"))
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO work_template_revision (work_id, number, body, author_name, note) VALUES (?, 1, ?, 'Autor', 'Anfang')")
        .bind(work)
        .bind(&doc.template)
        .execute(pool)
        .await
        .unwrap();
    exec(pool, "INSERT INTO store_meta (key, value) VALUES ('seeded', '1') ON CONFLICT DO NOTHING").await;
    let mut first_node = 0;
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
        if seq == 0 {
            first_node = id;
        }
        sqlx::query(
            "INSERT INTO revision (node_id, number, body, author_kind, author_name, model, status, note) \
             VALUES (?, 1, ?, 'human', 'Autor', NULL, 'current', 'Import')",
        )
        .bind(id)
        .bind(&heading.body)
        .execute(pool)
        .await
        .unwrap();
    }
    exec(
        pool,
        "INSERT INTO persona (handle, name, role, model, context_length, active, position, color) VALUES \
         ('@lektor', 'Lektor', 'Lektorat', 'erfundenes-modell', 32768, 1, 0, '#336699'), \
         ('@hoerer', 'Hörer', 'Gemeinde', 'erfundenes-modell', 8192, 0, 1, NULL)",
    )
    .await;
    sqlx::query("INSERT INTO post (node_id, position, author, body) VALUES (?, 0, 'Autor', 'Brennt die Laterne zu lange?'), (?, 1, '@lektor', 'Das Bild trägt.')")
        .bind(first_node)
        .bind(first_node)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO idea (work_id, node_id, note, bible, origin_kind, origin_name) VALUES (?, ?, 'Möwen am Morgen', 'Ps 139,9', 'human', 'Autor')",
    )
    .bind(work)
    .bind(first_node)
    .execute(pool)
    .await
    .unwrap();
    exec(pool, "INSERT INTO idea (note, origin_kind, origin_name) VALUES ('ohne Werk', 'mcp', '@mcp:probe')").await;
    exec(pool, "INSERT INTO connection (kind, config) VALUES ('eliezer', '{\"url\":\"http://127.0.0.1:4321\"}')").await;
    exec(
        pool,
        "INSERT INTO preferred_source (title, url, note, position) VALUES ('Hafenkunde', 'https://hafen.example/', 'erfunden', 1)",
    )
    .await;
    // Ein Bild (0009) mit Pfad, ein Gliederungsantrag (0012) und ein Token (0016).
    let asset: i64 = sqlx::query_scalar("INSERT INTO asset (work_id, name) VALUES (?, 'kai.svg') RETURNING id")
        .bind(work)
        .fetch_one(pool)
        .await
        .unwrap();
    let svg = "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"4\" height=\"2\"/>";
    sqlx::query(
        "INSERT INTO asset_revision (asset_id, number, mime, encoding, content, bytes, sha256, caption, author_name, status) \
         VALUES (?, 1, 'image/svg+xml', 'svg', ?, ?, 'erfunden', 'Der Kai', 'Autor', 'current')",
    )
    .bind(asset)
    .bind(svg)
    .bind(svg.len() as i64)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO asset_path (work_id, path, asset_id) VALUES (?, 'bilder/kai.svg', ?)")
        .bind(work)
        .bind(asset)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO outline_request (work_id, op, params, origin_kind, origin_name, note) \
         VALUES (?, 'delete', '{\"op\":{\"op\":\"delete\",\"key\":1},\"expected\":{\"title\":\"Abend\",\"level\":1}}', 'mcp', '@mcp:probe', '')",
    )
    .bind(work)
    .execute(pool)
    .await
    .unwrap();
    exec(pool, "INSERT INTO mcp_token (name, hash, prefix) VALUES ('laptop', printf('%064d', 7), 'bm_1234')").await;
    work
}

async fn tables(pool: &SqlitePool) -> BTreeMap<String, Vec<String>> {
    let names: Vec<String> = sqlx::query_scalar("SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name")
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

async fn schema(pool: &SqlitePool) -> BTreeMap<String, Option<String>> {
    let rows: Vec<(String, Option<String>)> =
        sqlx::query_as("SELECT name, sql FROM sqlite_master WHERE name NOT LIKE 'sqlite_%' ORDER BY name")
            .fetch_all(pool)
            .await
            .unwrap();
    rows.into_iter().collect()
}

/// Alle Zeilen über die Spalten von `layout` (vorher), sortiert.
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
async fn migration_0018_keeps_every_row_and_only_adds_the_request_table() {
    assert!(baruch_store::MIGRATOR.iter().any(|m| m.version == 18), "MIGRATOR enthält 0018");
    let dir = tempfile::tempdir().unwrap();
    let pool = raw_pool(dir.path()).await;
    migrate_before(&pool).await;
    let work = insert_rows(&pool).await;
    let layout = tables(&pool).await;
    let schema_before = schema(&pool).await;
    let before = contents(&pool, &layout).await;
    assert!(!layout.contains_key("asset_request"), "vorher keine Tabelle asset_request");
    for (table, rows) in [
        ("persona", 2),
        ("post", 2),
        ("idea", 2),
        ("node", 2),
        ("asset", 1),
        ("asset_revision", 1),
        ("outline_request", 1),
        ("mcp_token", 1),
    ] {
        assert_eq!(before[table].len(), rows, "nachgebaute Datenbank hat {rows} Zeilen in {table}");
    }
    pool.close().await;

    let store = Store::open(dir.path()).await.expect("Store::open führt 0018 aus");
    let pool = store.pool();
    let after = without_instance_row(contents(pool, &layout).await);
    for (table, rows) in &before {
        if table == "_sqlx_migrations" {
            continue;
        }
        assert_eq!(after.get(table), Some(rows), "Tabelle {table}: alle Zeilen in den alten Spalten unverändert");
    }
    let schema_after = schema(pool).await;
    for (name, sql) in &schema_before {
        // `Store::open` führt alle Migrationen aus; die spätere 0035 (Nutzerfrage 12) legt `outline_request`
        // mit der Antragsart `add` neu an (geprüft in migration_0035.rs), 0040 `work` mit dem Schema `book`
        // (geprüft in migration_0040.rs).
        if name == "outline_request" || name == "work" {
            continue;
        }
        assert_eq!(
            schema_after.get(name).map(|s| s
                .as_deref()
                .map(|sql| without_r30(name, &without_r28_edited(name, &without_uuid_column(sql))))),
            Some(sql.clone()),
            "Schema von {name} unverändert (bis auf die Spalte uuid aus 0041)"
        );
    }
    let table = schema_after.get("asset_request").cloned().flatten().expect("neue Tabelle asset_request");
    assert!(table.contains("REFERENCES work(id) ON DELETE CASCADE"), "Anträge hängen am Werk: {table}");
    // Die spätere 0045 (Runde 27) baut `asset_request` mit der vierten Art `cover_layout` neu (geprüft in
    // migration_0045.rs); die drei Arten aus 0018 stehen vorn.
    assert!(
        table.contains("CHECK (op IN ('replace','set_cover','delete','cover_layout'))"),
        "drei Arten aus 0018, dazu cover_layout aus 0045: {table}"
    );
    assert!(table.contains("CHECK (origin_kind IN ('persona','mcp'))"), "nur Assistenten stellen Anträge: {table}");
    assert!(schema_after.contains_key("asset_request_open"), "Index (work_id, decided)");
    let columns = tables(pool).await;
    assert_eq!(
        columns["asset_request"],
        [
            "id",
            "work_id",
            "op",
            "asset_name",
            "params",
            "origin_kind",
            "origin_name",
            "note",
            "created",
            "decided",
            "decision",
            "decided_by",
            "message"
        ],
        "Spalten nach Vertrag"
    );
    let requests: i64 = sqlx::query_scalar("SELECT count(*) FROM asset_request").fetch_one(pool).await.unwrap();
    assert_eq!(requests, 0, "die Migration legt keinen Antrag an");
    let broken: i64 =
        sqlx::query_scalar("SELECT count(*) FROM pragma_foreign_key_check").fetch_one(pool).await.unwrap();
    assert_eq!(broken, 0, "keine Verstöße gegen Fremdschlüssel");
    assert_eq!(
        baruch_typst::xref::strip_key_labels(&store.export_typst(work).await.unwrap()).into_owned(),
        SOURCE,
        "Export byte-gleich wie vor der Migration"
    );
    let full_layout = tables(pool).await;
    let full_schema = schema_after.clone();
    let full = contents(pool, &full_layout).await;
    store.pool().close().await;
    drop(store);

    let again = Store::open(dir.path()).await.expect("zweites Öffnen");
    assert_eq!(schema(again.pool()).await, full_schema, "zweites Öffnen ändert das Schema nicht");
    assert_eq!(contents(again.pool(), &full_layout).await, full, "zweites Öffnen ändert keine Zeile");

    // Mit den neuen Zeilen: ein Bild über den Store, ein Antrag, dann das Werk löschen.
    let new = NewAsset {
        file_name: "laterne.svg",
        name: None,
        bytes: b"<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"2\" height=\"2\"/>",
        as_base64: false,
        path: None,
        caption: "",
        alt: "",
        note: "",
    };
    again.add_asset(work, &new, "Autor").await.unwrap();
    let op = AssetOp::SetCover { name: Some("laterne.svg".into()), expected_cover: None };
    again.asset_request(work, &op, &IdeaOrigin::Mcp { handle: "@mcp:probe".into() }, "").await.unwrap();
    again.delete_work(work, "Die Laterne am Kai").await.expect("Werk mit Bildantrag lässt sich löschen");
    let left: i64 = sqlx::query_scalar("SELECT count(*) FROM asset_request").fetch_one(again.pool()).await.unwrap();
    assert_eq!(left, 0, "die Anträge gehen mit dem Werk");
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

/// Migration 0041 (Runde 26) hängt an Tabellen mit Werkinhalt die Spalte `uuid` an (`ALTER TABLE … ADD COLUMN`
/// erweitert den gespeicherten CREATE-Text um genau diesen Zusatz); der Vergleich des alten Schemas nimmt ihn heraus.
/// Runde 28 (0046) hängt `edited` an `post` und `chat_thread_post` an (geprüft in migration_0046.rs); sonst
/// bleibt das Schema wörtlich.
fn without_r28_edited(name: &str, sql: &str) -> String {
    if name == "post" || name == "chat_thread_post" { sql.replace(", edited TEXT", "") } else { sql.to_owned() }
}

fn without_uuid_column(sql: &str) -> String {
    sql.replace(R34_IDEA_COLUMNS, "").replace(UUID_COLUMN, "")
}

/// Runde 34 (0052) hängt Titel, Elternknoten, Reihenfolge und Hauptthema an `idea` an (`ALTER TABLE … ADD COLUMN`
/// erweitert den gespeicherten CREATE-Text um genau diesen Zusatz, direkt hinter der Spalte uuid; geprüft in
/// migration_0052.rs); der Vergleich des alten Schemas nimmt ihn heraus.
const R34_IDEA_COLUMNS: &str = ", title TEXT NOT NULL DEFAULT '' CHECK (length(title) <= 200), parent_id INTEGER REFERENCES idea(id) ON DELETE SET NULL, position INTEGER NOT NULL DEFAULT 0, main_topic INTEGER NOT NULL DEFAULT 0 CHECK (main_topic IN (0, 1))";

const UUID_COLUMN: &str = ", uuid TEXT CHECK (uuid IS NULL OR (length(uuid) = 36 AND uuid = lower(uuid)))";

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
