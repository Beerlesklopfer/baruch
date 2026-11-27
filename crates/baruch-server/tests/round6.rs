// SPDX-License-Identifier: AGPL-3.0-or-later
//! End-to-end tests of Round 6 (integrator): outline scheme, typeset view with scene and image, import and export with
//! images, migrations 0008 and 0009 together. Real data directory in the temp directory (SQLite, library, Git mirror);
//! all texts and images freely invented and generated in the test.

use crate::common;

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::path::Path;
use std::str::FromStr;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use serde_json::{Value, json};
use sqlx::SqlitePool;
use sqlx::migrate::Migrator;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

fn result(answer: &Value) -> &Value {
    assert!(answer.get("error").is_none(), "RPC-Fehler: {answer}");
    &answer["result"]
}

fn commits(dir: &Path) -> Option<u32> {
    common::git(dir, &["rev-list", "--count", "HEAD"]).map(|c| c.trim().parse().unwrap())
}

async fn numbers(server: &common::Server, work: i64) -> Vec<String> {
    let loaded = server.rpc("work.get", json!({ "work": work })).await;
    result(&loaded)["work"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n["number"].as_str().unwrap().to_owned())
        .collect()
}

fn editor_head(html: &str) -> &str {
    let start = html.find("<h2 class=\"node-title\"").expect("Kopf des Editors");
    let rest = &html[start..];
    &rest[..rest.find("</h2>").unwrap()]
}

// ---- Outline scheme ---------------------------------------------------------------------------

#[tokio::test]
async fn choosing_a_scheme_without_javascript_drives_outline_editor_and_export_and_template_is_byte_identical() {
    let server = common::open().await;
    let report = server.import_garden().await;
    let (work, dir) = (report.work, server.mirror_dir_of(&report.slug));
    assert_eq!(numbers(&server, work).await, ["1", "1.1", "1.1.1", "2"], "Vorgabe aus der Werkvorlage: 1.1");
    let before = commits(&dir);

    let (status, location) =
        server.post_form("/werke/nummerierung/act", &format!("work={work}&scheme=chapter_roman&custom=")).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Übernehmen per Formular ohne JavaScript leitet weiter");
    assert_eq!(location.as_deref(), Some(format!("/werke/{work}/nummerierung?ok=1").as_str()), "zurück mit Hinweis");
    let (_, page) = server.get(&location.unwrap()).await;
    assert!(page.contains("Das Gliederungsschema ist übernommen."), "Hinweis auf der Seite: {page}");
    if let (Some(before), Some(after)) = (before, commits(&dir)) {
        assert_eq!(after, before + 1, "genau ein Spiegel-Commit für das Schema");
    }

    assert_eq!(numbers(&server, work).await, ["I", "", "", "II"], "Gliederung: nur Kapitel römisch");
    let (_, html) = server.get(&format!("/?work={work}&node=3")).await;
    assert!(html.contains("Nummern: Nur Kapitel römisch"), "Gliederungskopf nennt das Schema");
    assert!(html.contains(&format!("href=\"/werke/{work}/nummerierung\"")), "Link „Schema wählen…“ im Kopf");
    assert!(
        editor_head(&html).contains("<span class=\"num\">II</span>"),
        "Editor-Kopf folgt dem Schema: {}",
        editor_head(&html)
    );
    let (_, outline) = server.get(&format!("/gliederung?work={work}&node=0")).await;
    assert!(outline.contains("Nur Kapitel römisch"), "der Gliederungsmanager nennt das Schema");

    let export = baruch_typst::xref::strip_key_labels(
        result(&server.rpc("work.export", json!({ "work": work })).await)["typst"].as_str().unwrap(),
    )
    .into_owned();
    let rule = "// Baruch: Gliederungsschema „Nur Kapitel römisch“\n";
    assert!(export.contains(rule), "der Export trägt die Regel des Schemas: {export}");
    if commits(&dir).is_some() {
        let main = std::fs::read_to_string(dir.join("main.typ")).unwrap();
        assert!(main.contains(rule), "main.typ im Spiegel folgt dem Schema: {main}");
    }

    let back = server.rpc("numbering.set", json!({ "work": work, "scheme": "template" })).await;
    result(&back);
    let export = baruch_typst::xref::strip_key_labels(
        result(&server.rpc("work.export", json!({ "work": work })).await)["typst"].as_str().unwrap(),
    )
    .into_owned();
    assert_eq!(export, common::GARDEN, "aus der Werkvorlage: Export wieder byte-gleich mit der Import-Datei");
    assert_eq!(numbers(&server, work).await, ["1", "1.1", "1.1.1", "2"], "und die alten Nummern");
}

// ---- Typeset view with scene and image, import and export with images -------------------------

/// SVG text, freely invented.
const HOUSE: &str =
    "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"8\" height=\"8\"><path d=\"M0 8 L4 0 L8 8 Z\"/></svg>\n";
const HOUSE_RED: &str = "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"8\" height=\"8\"><path fill=\"red\" d=\"M0 8 L4 0 L8 8 Z\"/></svg>\n";

/// Freely invented work: template with `#let szene`, chapter with label and scene, figure, section with PNG.
const WORKSHOP: &str = "#set document(title: \"Die Hobelbank\", author: \"Ida Feder\")\n\
#let szene(body) = block(fill: luma(245), inset: 6pt)[#body]\n\n\
= Die Werkstatt\n<werkstatt>\n#szene[\n  Der Morgen begann mit Hobeln.\n\n  Jonas strich über das Brett.\n]\n\n\
#figure(image(\"/bilder/haus.svg\"), caption: [Das Haus am Hang.])\n\n\
== Der Hof\n\n#image(\"bilder/punkt.png\", width: 1cm)\n\nSpäne im Wind.\n";

/// CRC-32 (PNG), bitwise.
fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &byte in data {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            crc = if crc & 1 == 1 { (crc >> 1) ^ 0xEDB8_8320 } else { crc >> 1 };
        }
    }
    !crc
}

/// Valid PNG with one grey pixel, generated in the test (no binary file in the repository).
fn tiny_png(gray: u8) -> Vec<u8> {
    fn chunk(out: &mut Vec<u8>, kind: &[u8], data: &[u8]) {
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        let mut body = kind.to_vec();
        body.extend_from_slice(data);
        out.extend_from_slice(&body);
        out.extend_from_slice(&crc32(&body).to_be_bytes());
    }
    let raw = [0u8, gray];
    let (mut a, mut b) = (1u32, 0u32);
    for &x in &raw {
        a = (a + u32::from(x)) % 65521;
        b = (b + a) % 65521;
    }
    let mut zlib = vec![0x78, 0x01, 0x01, 0x02, 0x00, 0xFD, 0xFF];
    zlib.extend_from_slice(&raw);
    zlib.extend_from_slice(&((b << 16) | a).to_be_bytes());
    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    chunk(&mut out, b"IHDR", &[0, 0, 0, 1, 0, 0, 0, 1, 8, 0, 0, 0, 0]);
    chunk(&mut out, b"IDAT", &zlib);
    chunk(&mut out, b"IEND", &[]);
    out
}

/// Directory with `hobelbank.typ`, `bilder/haus.svg` and `bilder/punkt.png`.
fn workshop_folder() -> tempfile::TempDir {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(tmp.path().join("bilder")).unwrap();
    std::fs::write(tmp.path().join("hobelbank.typ"), WORKSHOP).unwrap();
    std::fs::write(tmp.path().join("bilder/haus.svg"), HOUSE).unwrap();
    std::fs::write(tmp.path().join("bilder/punkt.png"), tiny_png(77)).unwrap();
    tmp
}

const BOUNDARY: &str = "baruch-runde6-grenze";

fn upload(fields: &[(&str, &str)], file_name: &str, content: &[u8]) -> Request<Body> {
    let mut body = Vec::new();
    for (name, value) in fields {
        body.extend_from_slice(
            format!("--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n{value}\r\n").as_bytes(),
        );
    }
    body.extend_from_slice(
        format!(
            "--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{file_name}\"\r\n\
             Content-Type: application/octet-stream\r\n\r\n"
        )
        .as_bytes(),
    );
    body.extend_from_slice(content);
    body.extend_from_slice(format!("\r\n--{BOUNDARY}--\r\n").as_bytes());
    Request::post("/werke/bilder/hochladen")
        .header(header::CONTENT_TYPE, format!("multipart/form-data; boundary={BOUNDARY}"))
        .body(Body::from(body))
        .unwrap()
}

#[tokio::test]
async fn the_satzbild_of_a_scene_section_shows_the_image_with_its_alt_text_and_saves_typst_correctly() {
    let server = common::open().await;
    let folder = workshop_folder();
    let report = baruch_server::import::import_file(&server.state, &folder.path().join("hobelbank.typ")).await.unwrap();
    let work = report.work;
    let alt = "Ein Haus mit \"spitzem\" Dach & Tür";
    let update = server
        .rpc("asset.update", json!({ "work": work, "name": "haus.svg", "rev": 1, "alt": alt, "caption": "Das Haus" }))
        .await;
    assert_eq!(result(&update)["changed"], true, "Alternativtext gesetzt: {update}");

    let node = server.rpc("node.get", json!({ "work": work, "node": 0 })).await;
    let body =
        result(&node)["revisions"].as_array().unwrap().iter().find(|r| r["status"] == "current").unwrap()["body"]
            .as_str()
            .unwrap()
            .to_owned();
    let parsed = server.rpc("wysiwyg.parse", json!({ "work": work, "body": body })).await;
    let blocks = result(&parsed)["blocks"].as_array().unwrap();
    let kinds: Vec<&str> = blocks.iter().map(|b| b["kind"].as_str().unwrap()).collect();
    assert_eq!(kinds.iter().filter(|k| **k == "scene").count(), 1, "die Szene ist ein eigener Block: {kinds:?}");
    let html = result(&parsed)["html"].as_str().unwrap();
    let src = format!("src=\"/werke/{work}/bilder/pfad?p=bilder%2Fhaus.svg\"");
    assert!(
        html.contains(&format!(
            "<img class=\"ws-image\" {src} alt=\"Ein Haus mit &quot;spitzem&quot; Dach &amp; Tür\" loading=\"eager\">"
        )),
        "die Bildinsel zeigt das Bild mit dem Alternativtext aus „Bilder“, HTML-maskiert: {html}"
    );
    let (status, svg) = server.get(&format!("/werke/{work}/bilder/pfad?p=%2Fbilder%2Fhaus.svg")).await;
    assert_eq!((status, svg.as_str()), (StatusCode::OK, HOUSE), "die Adresse im <img> liefert das Bild");

    let keep_all: Vec<Value> = (0..blocks.len()).map(|i| json!({ "keep": i })).collect();
    let same = server.rpc("wysiwyg.serialize", json!({ "work": work, "body": body, "edits": keep_all })).await;
    assert_eq!(result(&same)["body"], body.as_str(), "ohne Änderung byte-gleich");

    let scene = kinds.iter().position(|k| *k == "scene").unwrap();
    let mut edits: Vec<Value> = (0..blocks.len()).map(|i| json!({ "keep": i })).collect();
    edits[scene] = json!({ "replaces": scene, "body": [
        { "keep": 0 },
        { "replaces": 1, "block": { "kind": "paragraph", "inlines": [{ "t": "text", "v": "Jonas fegte die Späne." }] } }
    ] });
    let edited = server.rpc("wysiwyg.serialize", json!({ "work": work, "body": body, "edits": edits })).await;
    let new_body = result(&edited)["body"].as_str().unwrap();
    assert_eq!(
        new_body,
        body.replace("Jonas strich über das Brett.", "Jonas fegte die Späne."),
        "nur der Absatz in der Szene ist neu, Rahmen, Einzug, Label und Bild bleiben"
    );
    let saved = server.rpc("revision.save", json!({ "work": work, "node": 0, "body": new_body })).await;
    result(&saved);
    let export = baruch_typst::xref::strip_key_labels(
        result(&server.rpc("work.export", json!({ "work": work })).await)["typst"].as_str().unwrap(),
    )
    .into_owned();
    assert_eq!(
        export,
        WORKSHOP.replace("Jonas strich über das Brett.", "Jonas fegte die Späne."),
        "der Export ist gültiges Typst mit genau dieser Änderung"
    );
}

#[tokio::test]
async fn a_folder_import_with_images_shows_them_replacing_is_a_new_revision_and_export_out_is_byte_identical() {
    let server = common::open().await;
    let folder = workshop_folder();
    let report = baruch_server::import::import_file(&server.state, &folder.path().join("hobelbank.typ")).await.unwrap();
    assert_eq!(report.assets, ["haus.svg", "punkt.png"], "beide Bilder angelegt: {report:?}");
    assert!(report.skipped.is_empty(), "nichts übersprungen: {report:?}");
    let (work, dir) = (report.work, server.mirror_dir_of(&report.slug));

    let (status, list) = server.get(&format!("/werke/{work}/bilder")).await;
    assert_eq!(status, StatusCode::OK, "Seite „Bilder“");
    for needle in ["haus.svg", "punkt.png", "/bilder/haus.svg", "bilder/punkt.png"] {
        assert!(list.contains(needle), "die Liste nennt {needle}: {list}");
    }
    let (_, workspace) = server.get(&format!("/?work={work}")).await;
    assert!(workspace.contains(&format!("href=\"/werke/{work}/bilder\"")), "Link „Bilder“ in der Kopfzeile");

    let before = commits(&dir);
    let request =
        upload(&[("work", &work.to_string()), ("name", "haus.svg"), ("note", "rot")], "haus.svg", HOUSE_RED.as_bytes());
    let (status, headers, _) = server.send(request).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Ersetzen per Hochladen gelingt: {headers:?}");
    let revisions = server.rpc("asset.revisions", json!({ "work": work, "name": "haus.svg" })).await;
    let revisions = result(&revisions)["revisions"].as_array().unwrap().clone();
    let current: Vec<(u64, bool)> =
        revisions.iter().map(|r| (r["number"].as_u64().unwrap(), r["current"].as_bool().unwrap())).collect();
    assert!(
        current.contains(&(1, false)) && current.contains(&(2, true)),
        "Ersetzen = neue Fassung 2, aktuell: {current:?}"
    );
    assert_eq!(revisions.iter().find(|r| r["number"] == 2).unwrap()["based_on"], 1, "Fassung 2 baut auf 1 auf");
    if let (Some(before), Some(after)) = (before, commits(&dir)) {
        assert_eq!(after, before + 1, "genau ein Spiegel-Commit für das Ersetzen");
        let log = common::git(&dir, &["log", "-1", "--format=%s%n%(trailers:only,unfold)"]).unwrap();
        assert!(log.starts_with("Bild haus.svg: Fassung 2 ersetzt"), "Betreff: {log}");
        assert!(log.contains("Baruch-Action: asset-replaced"), "Trailer: {log}");
        assert_eq!(std::fs::read_to_string(dir.join("bilder/haus.svg")).unwrap(), HOUSE_RED, "Spiegel hat Fassung 2");
    }

    // Back to revision 1, then the export is byte-identical to the directory.
    let back = server.rpc("asset.set_current", json!({ "work": work, "name": "haus.svg", "rev": 1 })).await;
    result(&back);
    let target = tempfile::tempdir().unwrap();
    let out = target.path().join("zurueck");
    baruch_server::import::export_to_dir(&server.state.store, work, &out).await.unwrap();
    for path in ["hobelbank.typ", "bilder/haus.svg", "bilder/punkt.png"] {
        // Round 30a: the Typst file carries the jump labels `<kN>`; without them it is byte-identical.
        let got = std::fs::read(out.join(path)).unwrap();
        let got = if path.ends_with(".typ") {
            baruch_typst::xref::strip_key_labels(&String::from_utf8(got).unwrap()).into_owned().into_bytes()
        } else {
            got
        };
        assert_eq!(
            got,
            std::fs::read(folder.path().join(path)).unwrap(),
            "{path}: Export --out byte-gleich zum importierten Ordner"
        );
    }
}

// ---- Migrations 0008 and 0009 together --------------------------------------------------------

async fn raw_pool(dir: &Path) -> SqlitePool {
    let options = SqliteConnectOptions::from_str("sqlite:")
        .unwrap()
        .filename(dir.join("baruch.sqlite"))
        .create_if_missing(true)
        .foreign_keys(true);
    SqlitePoolOptions::new().max_connections(1).connect_with(options).await.unwrap()
}

async fn migrate_up_to(pool: &SqlitePool, store_version: i64) {
    let store: Vec<_> = baruch_store::MIGRATOR.iter().filter(|m| m.version <= store_version).cloned().collect();
    Migrator { migrations: Cow::Owned(store), ignore_missing: true, ..Migrator::DEFAULT }.run(pool).await.unwrap();
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../baruch-library/migrations");
    let mut library = Migrator::new(dir).await.unwrap();
    library.migrations = Cow::Owned(library.iter().filter(|m| m.version <= 105).cloned().collect());
    library.ignore_missing = true;
    library.run(pool).await.unwrap();
}

/// Work like an import in development (state 0007): template, nodes, revision 1 per node.
async fn insert_work(pool: &SqlitePool, slug: &str, source: &str, numbering: Option<&str>) -> i64 {
    let doc = baruch_typst::parse(source).unwrap();
    let work: i64 = sqlx::query_scalar(
        "INSERT INTO work (slug, kind, title, author, template, source_name, source_sha256, title_key, numbering) \
         VALUES (?, 'book', ?, 'Ida Feder', ?, ?, ?, ?, ?) RETURNING id",
    )
    .bind(slug)
    .bind(slug)
    .bind(&doc.template)
    .bind(format!("{slug}.typ"))
    .bind(format!("pruefsumme-{slug}"))
    .bind(baruch_store::title_key(slug))
    .bind(numbering)
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO work_template_revision (work_id, number, body, author_name, note) VALUES (?, 1, ?, 'Autor', 'Import')")
        .bind(work)
        .bind(&doc.template)
        .execute(pool)
        .await
        .unwrap();
    let mut chapter: Option<(usize, usize)> = None;
    let mut roots = 0usize;
    for (seq, heading) in doc.headings.iter().enumerate() {
        let (parent, position, kind) = if heading.level == 1 {
            roots += 1;
            chapter = Some((seq, 0));
            (None, roots - 1, "chapter")
        } else {
            let (parent, children) = chapter.as_mut().expect("Abschnitt unter einem Kapitel");
            *children += 1;
            (Some(*parent), *children - 1, "section")
        };
        let node: i64 = sqlx::query_scalar(
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
        .bind(node)
        .bind(&heading.body)
        .execute(pool)
        .await
        .unwrap();
    }
    work
}

/// All rows of all tables (per table the columns from `columns`), sorted.
async fn rows(pool: &SqlitePool, columns: &BTreeMap<String, Vec<String>>) -> BTreeMap<String, Vec<String>> {
    let mut out = BTreeMap::new();
    for (table, cols) in columns {
        let select = cols.iter().map(|c| format!("quote(\"{c}\")")).collect::<Vec<_>>().join(" || char(31) || ");
        let mut all: Vec<String> = sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT {select} FROM \"{table}\"")))
            .fetch_all(pool)
            .await
            .unwrap();
        all.sort();
        out.insert(table.clone(), all);
    }
    out
}

async fn table_columns(pool: &SqlitePool) -> BTreeMap<String, Vec<String>> {
    let tables: Vec<String> = sqlx::query_scalar(
        "SELECT name FROM sqlite_master WHERE type = 'table' AND name <> '_sqlx_migrations' ORDER BY name",
    )
    .fetch_all(pool)
    .await
    .unwrap();
    let mut out = BTreeMap::new();
    for table in tables {
        let cols: Vec<String> = sqlx::query_scalar("SELECT name FROM pragma_table_info(?) ORDER BY cid")
            .bind(&table)
            .fetch_all(pool)
            .await
            .unwrap();
        out.insert(table, cols);
    }
    out
}

async fn count(pool: &SqlitePool, sql: &str) -> i64 {
    sqlx::query_scalar(sqlx::AssertSqlSafe(sql)).fetch_one(pool).await.unwrap()
}

#[tokio::test]
async fn migrations_0008_and_0009_together_keep_a_development_database_lossless() {
    let data = tempfile::tempdir().unwrap();
    let pool = raw_pool(data.path()).await;
    migrate_up_to(&pool, 7).await;
    let plain = insert_work(&pool, "hobelbank", WORKSHOP, None).await;
    let without = insert_work(&pool, "spaene", &WORKSHOP.replace("Hobelbank", "Späne"), Some("")).await;
    let custom = insert_work(&pool, "zwinge", &WORKSHOP.replace("Hobelbank", "Zwinge"), Some("I.a")).await;
    // As in development: seed done, sample library present.
    sqlx::query("INSERT OR REPLACE INTO store_meta (key, value) VALUES ('seeded', '1')").execute(&pool).await.unwrap();
    // The conversion of old part texts at start (user question 8) is checked by `round14_conversion.rs`; this is only
    // about the migrations, therefore it counts as done (flag `parts_marked_r14`).
    sqlx::query("INSERT OR REPLACE INTO store_meta (key, value) VALUES (?, '1')")
        .bind(baruch_store::CONVERTED_KEY)
        .execute(&pool)
        .await
        .unwrap();
    assert!(baruch_library::seed_if_empty(&pool).await.unwrap(), "Beispielbibliothek gesät");
    let old_columns = table_columns(&pool).await;
    let before = rows(&pool, &old_columns).await;
    pool.close().await;

    // Full server start: Store::open (0008, 0009), library, no seed, bring the mirror up to date.
    let state = baruch_server::AppState::open(data.path()).await.expect("Start auf der alten Datenbank");
    let pool = state.store.pool().clone();
    let mut after = rows(&pool, &old_columns).await;
    // Migration 0041 (Round 26) enters the instance identifier as a new row in store_meta; that is an addition, not a
    // change of an old row. The comparison takes out exactly this one row.
    if let Some(meta) = after.get_mut("store_meta") {
        let count = meta.len();
        meta.retain(|row| !row.starts_with("'instance_uuid'\u{1f}"));
        assert_eq!(count - meta.len(), 1, "genau eine neue Zeile instance_uuid in store_meta (Migration 0041)");
    }
    assert_eq!(after, before, "alle alten Zeilen und Spalten aller Tabellen unverändert");
    let new_columns = table_columns(&pool).await;
    for (table, cols) in &old_columns {
        assert_eq!(&new_columns[table][..cols.len()], &cols[..], "{table}: alte Spalten in alter Reihenfolge");
    }
    for table in ["asset", "asset_path", "asset_revision"] {
        assert_eq!(count(&pool, &format!("SELECT count(*) FROM {table}")).await, 0, "{table} neu und leer");
    }
    assert_eq!(count(&pool, "SELECT count(*) FROM pragma_foreign_key_check").await, 0, "keine Fremdschlüssel-Verstöße");
    assert_eq!(
        count(
            &pool,
            "SELECT count(*) FROM node WHERE (SELECT count(*) FROM revision r WHERE r.node_id = node.id AND r.status = 'current') <> 1"
        )
        .await,
        0,
        "je Abschnitt genau eine aktuelle Fassung"
    );
    for (work, scheme) in [(plain, "template"), (without, "none"), (custom, "custom")] {
        let stored: String = sqlx::query_scalar("SELECT numbering_scheme FROM work WHERE id = ?")
            .bind(work)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(stored, scheme, "Werk {work}: altes numbering → Schema {scheme}");
    }
    assert_eq!(
        baruch_typst::xref::strip_key_labels(&state.store.export_typst(plain).await.unwrap()).into_owned(),
        WORKSHOP,
        "Schema template: Export byte-gleich"
    );
    let versions: Vec<i64> =
        sqlx::query_scalar("SELECT version FROM _sqlx_migrations ORDER BY version").fetch_all(&pool).await.unwrap();
    assert!(
        versions.contains(&8) && versions.contains(&9) && versions.contains(&105),
        "0008, 0009, 0105: {versions:?}"
    );

    // After the migration images can be created.
    let state = std::sync::Arc::new(state);
    let server = common::Server { app: baruch_server::router(state.clone()), state, data };
    let added = server
        .rpc("asset.add", json!({ "work": plain, "file_name": "haus.svg", "content": HOUSE, "encoding": "svg", "path": "/bilder/haus.svg" }))
        .await;
    assert_eq!(result(&added)["rev"], 1, "Bild nach der Migration angelegt: {added}");

    // A second start changes nothing any more.
    let all = table_columns(&pool).await;
    let once = rows(&pool, &all).await;
    let migrations_once = count(&pool, "SELECT count(*) FROM _sqlx_migrations").await;
    let again = baruch_server::AppState::open(server.path()).await.unwrap();
    let again_pool = again.store.pool();
    assert_eq!(rows(again_pool, &all).await, once, "zweiter Start: alle Zeilen gleich");
    assert_eq!(
        count(again_pool, "SELECT count(*) FROM _sqlx_migrations").await,
        migrations_once,
        "keine neue Migration"
    );
}
