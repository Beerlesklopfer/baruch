// SPDX-License-Identifier: AGPL-3.0-or-later
//! Migration 0016 (Tokens für den MCP-Zugang) auf einer nachgebauten Datenbank im Stand von development
//! davor: Store-Migrationen 0001–0014 und Bibliothek 0101 ff., ein frei erfundenes Werk mit Kapiteln,
//! Personas (eine mit Farbe), Beiträgen, Einfällen, Verbindungen und einer bevorzugten Quelle.
//!
//! Danach `Store::open`: Jede Zeile jeder Tabelle bleibt unverändert, das Schema jeder alten Tabelle
//! ebenso; dazu kommt nur die leere Tabelle `mcp_token` mit ihrem Teilindex. Ein zweites Öffnen ändert
//! nichts mehr. Spätere Migrationen anderer Aufgaben (z. B. 0015) laufen beim Öffnen mit; verglichen
//! wird darum nur, was 0016 betrifft.

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::path::Path;
use std::str::FromStr;

use baruch_store::{Store, title_key};
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

/// Store bis 0014 (Stand development vor Runde 10) und die Bibliothek.
async fn migrate_before(pool: &SqlitePool) {
    let old: Vec<_> = baruch_store::MIGRATOR.iter().filter(|m| m.version <= 14).cloned().collect();
    assert!(old.iter().any(|m| m.version == 14), "Stand vorher hat 0014");
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
async fn migration_0016_keeps_every_row_and_only_adds_the_token_table() {
    assert!(baruch_store::MIGRATOR.iter().any(|m| m.version == 16), "MIGRATOR enthält 0016");
    let dir = tempfile::tempdir().unwrap();
    let pool = raw_pool(dir.path()).await;
    migrate_before(&pool).await;
    let work = insert_rows(&pool).await;
    let layout = tables(&pool).await;
    let schema_before = schema(&pool).await;
    let before = contents(&pool, &layout).await;
    assert!(!layout.contains_key("mcp_token"), "vorher keine Tabelle mcp_token");
    for (table, rows) in [("persona", 2), ("post", 2), ("idea", 2), ("connection", 1), ("node", 2)] {
        assert_eq!(before[table].len(), rows, "nachgebaute Datenbank hat {rows} Zeilen in {table}");
    }
    pool.close().await;

    let store = Store::open(dir.path()).await.expect("Store::open führt 0016 aus");
    let pool = store.pool();
    let after = without_instance_row(contents(pool, &layout).await);
    for (table, rows) in &before {
        if table == "_sqlx_migrations" {
            continue;
        }
        assert_eq!(after.get(table), Some(rows), "Tabelle {table}: alle Zeilen in den alten Spalten unverändert");
    }
    let schema_after = schema(pool).await;
    // 0015 (Personas, andere Aufgabe derselben Runde) darf `persona` erweitern; 0016 fasst sie nicht an.
    // Ebenso legt die spätere 0035 (Nutzerfrage 12) `outline_request` mit der Antragsart `add` neu an
    // (geprüft in migration_0035.rs), und 0040 baut `work` mit dem Schema `book` neu (migration_0040.rs);
    // `Store::open` führt hier alle Migrationen aus.
    for (name, sql) in &schema_before {
        if name == "persona" || name == "outline_request" || name == "work" {
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
    let table = schema_after.get("mcp_token").cloned().flatten().expect("neue Tabelle mcp_token");
    assert!(table.contains("hash TEXT NOT NULL UNIQUE CHECK (length(hash) = 64)"), "nur ein Hash: {table}");
    let index = schema_after.get("mcp_token_active_name").cloned().flatten().expect("Teilindex der Namen");
    assert!(index.contains("WHERE revoked IS NULL"), "Name eindeutig nur unter gültigen: {index}");
    let columns = tables(pool).await;
    assert_eq!(
        columns["mcp_token"],
        ["id", "name", "hash", "prefix", "created", "last_used", "revoked"],
        "Spalten nach Vertrag"
    );
    let tokens: i64 = sqlx::query_scalar("SELECT count(*) FROM mcp_token").fetch_one(pool).await.unwrap();
    assert_eq!(tokens, 0, "die Migration legt keinen Token an");
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
    sql.replace(UUID_COLUMN, "")
}

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
