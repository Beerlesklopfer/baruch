// SPDX-License-Identifier: AGPL-3.0-or-later
//! End-to-end acceptance of Round 5 (tree · wysiwyg · r4) after the merge, everything through `/rpc` or the pages as in
//! the browser: a click in the tree opens the editor with the typeset view area (without JavaScript only the text
//! field), editing in the typeset view and saving yields a new revision with correct Typst; loading and saving without
//! a change is byte-identical across the whole sample work; changing the level and switching to an old revision makes
//! the outline follow (one commit, the revision stays unchanged); MCP writes only with `key`; `work.import` through
//! `/rpc` with 3 MiB; a new work after the deletion gets no old id; migration 0007 on a database in the state of Round
//! 4. Real data directory in the temp directory, garden book from `tests/data`, otherwise freely invented texts.

use crate::common;

use std::net::SocketAddr;
use std::path::Path;

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode, header};
use serde_json::{Value, json};

fn result(answer: &Value) -> &Value {
    assert!(answer.get("error").is_none_or(Value::is_null), "Fehler statt Ergebnis: {answer}");
    &answer["result"]
}

/// Stable key of the section at position `seq` (according to `node.get`).
async fn key_of(server: &common::Server, work: i64, seq: usize) -> i64 {
    let node = server.rpc("node.get", json!({ "work": work, "node": seq })).await;
    result(&node)["key"].as_i64().expect("node.get nennt den festen Schlüssel")
}

/// Text of the current revision and its number.
async fn current(server: &common::Server, work: i64, seq: usize) -> (String, u64) {
    let node = server.rpc("node.get", json!({ "work": work, "node": seq })).await;
    assert_eq!(common::currents(result(&node)), 1, "je Abschnitt genau eine aktuelle Fassung: {node}");
    let rev = result(&node)["revisions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["status"] == "current")
        .expect("aktuelle Fassung vorhanden")
        .clone();
    (rev["body"].as_str().unwrap().to_owned(), rev["number"].as_u64().unwrap())
}

/// (number, title) of all outline items.
async fn outline_of(server: &common::Server, work: i64) -> Vec<(String, String)> {
    let answer = server.rpc("work.get", json!({ "work": work })).await;
    result(&answer)["work"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| (n["number"].as_str().unwrap().to_owned(), n["title"].as_str().unwrap().to_owned()))
        .collect()
}

fn commits(dir: &Path) -> Option<u64> {
    common::git(dir, &["rev-list", "--count", "HEAD"]).map(|c| c.trim().parse().unwrap())
}

/// MCP request from the own machine (without a token only loopback).
async fn mcp(server: &common::Server, method: &str, params: Value) -> Value {
    let message = json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params });
    let mut request = Request::post("/mcp")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(message.to_string()))
        .unwrap();
    request.extensions_mut().insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 40000))));
    let (status, _, body) = server.send(request).await;
    assert_eq!(status, StatusCode::OK, "MCP {method} antwortet mit 200: {body}");
    serde_json::from_str(&body).unwrap()
}

/// Nothing but `Keep` – this is how the widget sends a text in which nothing was touched.
fn keep_all(blocks: &Value) -> Value {
    Value::Array((0..blocks.as_array().unwrap().len()).map(|i| json!({ "keep": i })).collect())
}

// ---- Click → typeset view → save ----------------------------------------------------------------

#[tokio::test]
async fn a_click_opens_the_typeset_view_and_saving_an_edit_there_makes_a_new_revision_with_correct_typst() {
    let server = common::open().await;
    let garden = server.import_garden().await;
    let work = garden.work;
    let dir = server.mirror_dir_of(&garden.slug);
    let key = key_of(&server, work, 3).await;

    // Without JavaScript: the page has the text field, the typeset view area stays hidden.
    let (status, page) = server.get(&format!("/?work={work}&node=3")).await;
    assert_eq!(status, StatusCode::OK, "Arbeitsseite lädt: {page}");
    assert!(page.contains("<textarea name=\"body\""), "ohne JavaScript bleibt das Textfeld die Eingabe: {page}");
    let area = format!(
        "<div class=\"wysiwyg\" data-widget=\"wysiwyg\" data-work=\"{work}\" data-key=\"{key}\" data-node=\"3\" data-rev=\"1\""
    );
    let at = page.find(&area).unwrap_or_else(|| panic!("Satzbild-Bereich mit festem Schlüssel fehlt: {page}"));
    // Wish 2026-11-06: div.wysiwyg encloses toolbar, sheet (with the form) and status line; without JavaScript toolbar
    // and typeset view are hidden, not the whole area (otherwise the text field would be missing).
    let form = page.find("<form class=\"editor-form\"").expect("Editor-Formular vorhanden");
    let wrap = page.find("<div class=\"ws-wrap\" data-ws-wrap hidden>").expect("Satzbild ohne JavaScript verborgen");
    let textarea = page.find("aria-label=\"Text des Abschnitts\"").expect("Textfeld des Editors vorhanden");
    assert!(
        at < form && form < wrap && wrap < textarea,
        "das Satzbild steht im Editor-Formular vor dem Textfeld (Vertrag 3a), das Formular im Bereich des Widgets"
    );
    assert!(page.contains("data-edit-toolbar hidden>"), "ohne JavaScript ist die Werkzeugleiste verborgen: {page}");

    // The widget is wired: main.js loads wysiwyg.js, and both sides of the event „vor dem Speichern“ are delivered
    // (outline.js triggers it, wysiwyg.js listens for it).
    for (script, needle) in
        [("main.js", "./wysiwyg.js"), ("wysiwyg.js", "baruch:before-save"), ("outline.js", "baruch:before-save")]
    {
        let (status, js) = server.get(&format!("/static/js/{script}")).await;
        assert_eq!(status, StatusCode::OK, "{script} wird ausgeliefert");
        assert!(js.contains(needle), "{script} enthält {needle}, sonst bleibt das Satzbild ungespeichert");
    }

    // The click in the tree: ui.render returns the editor with the typeset view area, the widget parses the text.
    let click = server.rpc("ui.render", json!({ "work": work, "node": 3 })).await;
    let editor = result(&click)["editor"].as_str().unwrap();
    assert!(editor.contains(&area), "auch der neu gezeichnete Editor trägt den Satzbild-Bereich: {editor}");
    let (body, rev) = current(&server, work, 3).await;
    assert_eq!(rev, 1, "Sommer hat nach dem Import genau eine Fassung");
    let parsed = server.rpc("wysiwyg.parse", json!({ "work": work, "body": body })).await;
    let blocks = &result(&parsed)["blocks"];
    let html = result(&parsed)["html"].as_str().unwrap();
    assert!(html.contains("data-b=\"0\" data-kind=\"heading\""), "Überschrift als Block im Satzbild: {html}");
    assert!(html.contains("data-b=\"1\" data-kind=\"paragraph\""), "Absatz als Block im Satzbild: {html}");
    let style = server.rpc("wysiwyg.style", json!({ "work": work })).await;
    assert!(
        result(&style)["css"].as_str().unwrap().contains(".ws-sheet"),
        "Satzbild-Stil aus der Werkvorlage: {style}"
    );

    // Change a paragraph in the typeset view (emphasis), heading untouched → Keep.
    let edits = json!([
        { "keep": 0 },
        { "replaces": 1, "block": { "kind": "paragraph", "inlines": [
            { "t": "text", "v": "Gießen am " },
            { "t": "emph", "c": [{ "t": "text", "v": "Morgen" }] },
            { "t": "text", "v": "." }
        ] } }
    ]);
    assert_eq!(blocks.as_array().unwrap().len(), 2, "Sommer besteht aus Überschrift und einem Absatz: {blocks}");
    let serialized = server.rpc("wysiwyg.serialize", json!({ "work": work, "body": body, "edits": edits })).await;
    let new_body = result(&serialized)["body"].as_str().unwrap().to_owned();
    assert_eq!(
        new_body, "= Sommer\n\nGießen am _Morgen_.\n",
        "kanonisches Typst; die Überschriftszeile bleibt wörtlich"
    );

    // Save like the form: outline.save with key and based_on.
    let saved = server
        .rpc("outline.save", json!({ "work": work, "node": 3, "key": key, "based_on": 1, "body": new_body }))
        .await;
    assert_eq!(result(&saved)["rev"], 2, "Speichern ergibt Fassung 2: {saved}");
    assert_eq!(current(&server, work, 3).await, (new_body.clone(), 2), "die neue Fassung ist aktuell und byte-gleich");
    let export = server.rpc("work.export", json!({ "work": work })).await;
    assert!(
        baruch_typst::xref::strip_key_labels(result(&export)["typst"].as_str().unwrap())
            .ends_with("= Sommer\n\nGießen am _Morgen_.\n"),
        "der Export trägt das neue Typst: {export}"
    );
    let reparsed = server.rpc("wysiwyg.parse", json!({ "work": work, "body": new_body })).await;
    assert!(
        result(&reparsed)["html"].as_str().unwrap().contains("<em>Morgen</em>"),
        "wieder geladen zeigt das Satzbild die Hervorhebung: {reparsed}"
    );
    if common::git(&dir, &["status"]).is_some() {
        let file = baruch_typst::xref::strip_key_labels(
            &std::fs::read_to_string(dir.join("abschnitte/003-sommer.typ")).unwrap(),
        )
        .into_owned();
        assert!(file.ends_with(&new_body), "die Spiegeldatei trägt nach der Import-Zeile die neue Fassung: {file}");
    }
}

#[tokio::test]
async fn loading_and_saving_without_a_change_is_byte_identical_over_the_whole_sample_work() {
    let server = common::open().await;
    let work = server.state.default_work().await.unwrap();
    let before = result(&server.rpc("work.export", json!({ "work": work })).await)["typst"].clone();
    let loaded = server.rpc("work.get", json!({ "work": work })).await;
    let nodes = result(&loaded)["work"]["nodes"].as_array().unwrap().clone();
    assert!(nodes.len() > 3, "das Beispielwerk hat mehrere Gliederungspunkte: {}", nodes.len());
    let mut revisions = 0;
    for node in &nodes {
        let seq = node["id"].as_u64().unwrap() as usize;
        for rev in node["revisions"].as_array().unwrap() {
            let body = rev["body"].as_str().unwrap();
            let parsed = server.rpc("wysiwyg.parse", json!({ "work": work, "body": body })).await;
            let blocks = &result(&parsed)["blocks"];
            let joined: String = blocks.as_array().unwrap().iter().map(|b| b["raw"].as_str().unwrap()).collect();
            assert_eq!(joined, body, "Abschnitt {seq}, Fassung {}: die Blöcke ergeben den Text", rev["number"]);
            let edits = keep_all(blocks);
            let back = server.rpc("wysiwyg.serialize", json!({ "work": work, "body": body, "edits": edits })).await;
            assert_eq!(
                result(&back)["body"].as_str().unwrap(),
                body,
                "Abschnitt {seq}, Fassung {}: ohne Änderung byte-gleich zurück",
                rev["number"]
            );
            revisions += 1;
        }
        // Saving the unchanged text like the form creates no new revision.
        let (body, number) = current(&server, work, seq).await;
        let saved =
            server.rpc("outline.save", json!({ "work": work, "node": seq, "based_on": number, "body": body })).await;
        assert_eq!(result(&saved)["rev"], number, "Abschnitt {seq}: unverändert gespeichert bleibt die Fassung");
        assert_eq!(current(&server, work, seq).await, (body, number), "Abschnitt {seq}: Text und Nummer unverändert");
    }
    assert!(revisions >= nodes.len(), "jede Fassung jedes Abschnitts geprüft: {revisions}");
    let after = result(&server.rpc("work.export", json!({ "work": work })).await)["typst"].clone();
    assert_eq!(after, before, "der Export des ganzen Beispielwerks ist danach byte-gleich");
}

// ---- Change the level → old revision → outline follows ----------------------------------------

#[tokio::test]
async fn changing_the_level_and_switching_back_to_an_old_version_lets_the_outline_follow_in_one_commit() {
    let server = common::open().await;
    let garden = server.import_garden().await;
    let work = garden.work;
    let dir = server.mirror_dir_of(&garden.slug);
    let (old_body, _) = current(&server, work, 1).await;
    assert!(old_body.starts_with("== Aussaat\n"), "Aussaat ist im Gartenbuch ein Abschnitt: {old_body}");

    let up = server.rpc("outline.set_level", json!({ "work": work, "node": 1, "level": 1 })).await;
    result(&up);
    let numbers: Vec<String> = outline_of(&server, work).await.into_iter().map(|(n, _)| n).collect();
    assert_eq!(numbers, ["1", "2", "2.1", "3"], "Aussaat ist Kapitel, Tiefe wandert als Abschnitt mit");
    let (body, rev) = current(&server, work, 1).await;
    assert!(body.starts_with("= Aussaat\n") && rev == 2, "Hochstufen schreibt Fassung 2 mit „=“: {body}");

    let before = commits(&dir);
    let back = server.rpc("revision.set_current", json!({ "work": work, "node": 1, "rev": 1 })).await;
    let answer = result(&back);
    assert_eq!(answer["outline"]["action"], "level", "die Antwort trägt die Gliederungsänderung: {back}");
    assert_eq!(answer["outline"]["detail"], "„Aussaat“ Ebene 1 → 2", "Ebene folgt der Fassung: {back}");
    let outline = outline_of(&server, work).await;
    let numbers: Vec<&str> = outline.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(numbers, ["1", "1.1", "1.1.1", "2"], "Aussaat wieder unter Frühling, Tiefe mitgestuft");
    assert_eq!(
        current(&server, work, 1).await,
        (old_body, 1),
        "Nutzerentscheidung „Nein“: die Fassung bleibt byte-gleich"
    );
    let (tiefe, tiefe_rev) = current(&server, work, 2).await;
    assert!(
        tiefe.starts_with("=== Tiefe\n"),
        "der mitgestufte Unterpunkt bekommt die passende Überschriftszeile: {tiefe}"
    );
    assert_eq!(tiefe_rev, 3, "dafür eine neue Fassung des Menschen (1 Import, 2 Hochstufen, 3 Umschalten)");
    if let (Some(before), Some(after)) = (before, commits(&dir)) {
        assert_eq!(after, before + 1, "Umschalten mit Ebenenwechsel ist genau ein Commit");
        let log = common::git(&dir, &["log", "-1", "--format=%s%n%b"]).unwrap();
        assert!(log.contains("auf Fassung 1 umgeschaltet · Ebene 1 → 2"), "Betreff nennt die Ebene: {log}");
        assert!(log.contains("Baruch-Action: switched"), "Trailer des Umschaltens: {log}");
        assert!(log.contains("Tiefe"), "der Rumpf nennt den mitgestuften Punkt: {log}");
    }
}

// ---- MCP only with key ---------------------------------------------------------------------------

#[tokio::test]
async fn mcp_writes_proposals_only_with_a_key() {
    let server = common::open().await;
    let garden = server.import_garden().await;
    let work = garden.work;
    let key = key_of(&server, work, 3).await;
    let body = "= Sommer\n\nGießen in der Dämmerung.\n";
    for arguments in [json!({ "work": work, "node": 3, "body": body }), json!({ "work": work, "body": body })] {
        let answer = mcp(&server, "tools/call", json!({ "name": "propose_revision", "arguments": arguments })).await;
        assert_eq!(answer["error"]["code"], -32602, "ohne key (oder mit node) → -32602: {answer}");
    }
    let post = json!({ "work": work, "node": 3, "body": "Ein Hinweis." });
    let answer = mcp(&server, "tools/call", json!({ "name": "add_post", "arguments": post })).await;
    assert_eq!(answer["error"]["code"], -32602, "auch add_post nimmt keine Stelle an: {answer}");
    assert_eq!(current(&server, work, 3).await.1, 1, "abgelehnte Aufrufe ändern nichts");

    let ok = json!({ "work": work, "key": key, "body": body });
    let answer = mcp(&server, "tools/call", json!({ "name": "propose_revision", "arguments": ok })).await;
    assert_eq!(answer["result"]["isError"], false, "mit key gelingt der Vorschlag: {answer}");
    let node = server.rpc("node.get", json!({ "work": work, "key": key })).await;
    let revisions = result(&node)["revisions"].as_array().unwrap();
    assert_eq!(revisions.len(), 2, "der Vorschlag ist angehängt: {node}");
    assert_eq!(revisions[1]["status"], "proposed", "MCP schreibt nur Vorschläge, die aktuelle bleibt");
    assert_eq!(current(&server, work, 3).await.1, 1, "Fassung 1 bleibt aktuell");
}

// ---- Import through /rpc and work ids --------------------------------------------------------------

/// Typst text with a good `len` bytes from invented chapters.
fn book_of_size(len: usize) -> String {
    let mut text = String::from("#set document(title: \"Das dicke Mühlenbuch\")\n\n");
    let mut chapter = 0;
    while text.len() < len {
        chapter += 1;
        text.push_str(&format!("= Mühle {chapter}\n\n"));
        for _ in 0..200 {
            text.push_str("Das Mühlrad klappert am rauschenden Bach.\n");
        }
        text.push('\n');
    }
    text
}

async fn post_rpc(server: &common::Server, body: String) -> (StatusCode, String) {
    let request =
        Request::post("/rpc").header(header::CONTENT_TYPE, "application/json").body(Body::from(body)).unwrap();
    let (status, _, text) = server.send(request).await;
    (status, text)
}

#[tokio::test]
async fn a_three_mib_import_over_rpc_succeeds_and_a_request_over_the_limit_gets_413() {
    let server = common::open().await;
    let source = book_of_size(3 * 1024 * 1024);
    let request = json!({ "jsonrpc": "2.0", "id": 1, "method": "work.import",
        "params": { "file_name": "muehlen.typ", "source": source } });
    let body = request.to_string();
    assert!(body.len() > 2 * 1024 * 1024, "Testaufbau: größer als die Vorgabe von axum (2 MB)");
    let (status, text) = post_rpc(&server, body).await;
    assert_eq!(status, StatusCode::OK, "3 MiB über /rpc werden angenommen");
    let answer: Value = serde_json::from_str(&text).unwrap();
    let work = result(&answer)["work"].as_i64().expect("das neue Werk hat eine id");
    let export = server.rpc("work.export", json!({ "work": work })).await;
    assert_eq!(
        baruch_typst::xref::strip_key_labels(result(&export)["typst"].as_str().unwrap()),
        source,
        "Export des großen Werks byte-gleich"
    );

    let over = "x".repeat(baruch_server::works::RPC_BODY_LIMIT + 1);
    let (status, _) = post_rpc(&server, over).await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE, "mehr als RPC_BODY_LIMIT → 413 (dokumentiert in RPC.md)");
}

#[tokio::test]
async fn a_new_work_after_deleting_one_gets_neither_its_id_nor_its_slug() {
    let server = common::open().await;
    let first = server.rpc("work.create", json!({ "title": "Das Brückenbuch" })).await;
    let id = result(&first)["work"].as_i64().unwrap();
    assert_eq!(result(&first)["slug"], "das-brueckenbuch", "erster slug aus dem Titel: {first}");
    let deleted = server.rpc("work.delete", json!({ "work": id, "confirm_title": "Das Brückenbuch" })).await;
    result(&deleted);
    let again = server.rpc("work.create", json!({ "title": "Das Brückenbuch" })).await;
    let new_id = result(&again)["work"].as_i64().unwrap();
    assert_eq!(new_id, id + 1, "AUTOINCREMENT: die id des gelöschten Werks kommt nicht wieder");
    assert_ne!(result(&again)["slug"], "das-brueckenbuch", "der slug des gelöschten Werks bleibt reserviert: {again}");
    let works = server.state.store.works().await.unwrap();
    assert_eq!(works.len(), 2, "Beispielwerk und das neue Werk, kein neu gesätes Beispiel: {works:?}");
}

// ---- Migration 0007 on a database in the state of Round 4 -----------------------------------------

fn partial(migrator: &sqlx::migrate::Migrator, last: i64) -> sqlx::migrate::Migrator {
    sqlx::migrate::Migrator {
        migrations: std::borrow::Cow::Owned(
            migrator.migrations.iter().filter(|m| m.version <= last).cloned().collect(),
        ),
        ignore_missing: true,
        locking: true,
        no_tx: false,
        ..sqlx::migrate::Migrator::DEFAULT
    }
}

const TEMPLATE: &str = "#set document(title: \"Laternenbuch\")\n\n";
/// (title, number, parent, position, kind, text) of a node as the import before Round 5 wrote it.
type OldNode = (&'static str, &'static str, Option<i64>, i64, &'static str, &'static str);
const BOOK: [OldNode; 3] = [
    ("Abend", "1", None, 0, "chapter", "= Abend\n\nDie Laternen gehen an.\n\n"),
    ("Docht", "1.1", Some(0), 0, "section", "== Docht\n\nEin Docht aus Baumwolle.\n\n"),
    ("Morgen", "2", None, 1, "chapter", "= Morgen\n\nDie Laternen gehen aus.\n"),
];

async fn insert_book(pool: &sqlx::SqlitePool, slug: &str, title: &str) -> i64 {
    let work: i64 = sqlx::query_scalar(
        "INSERT INTO work (slug, kind, title, title_key, author, template, source_name, source_sha256) \
         VALUES (?, 'book', ?, lower(?), 'Jemand', ?, 'laterne.typ', ?) RETURNING id",
    )
    .bind(slug)
    .bind(title)
    .bind(title)
    .bind(TEMPLATE)
    .bind(slug)
    .fetch_one(pool)
    .await
    .unwrap();
    for (seq, (title, number, parent, position, kind, body)) in BOOK.iter().enumerate() {
        let node: i64 = sqlx::query_scalar(
            "INSERT INTO node (work_id, seq, parent_seq, position, number, title, kind) \
             VALUES (?, ?, ?, ?, ?, ?, ?) RETURNING id",
        )
        .bind(work)
        .bind(seq as i64)
        .bind(parent)
        .bind(position)
        .bind(number)
        .bind(title)
        .bind(kind)
        .fetch_one(pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO revision (node_id, number, body, author_kind, author_name, status, note) \
             VALUES (?, 1, ?, 'human', 'Autor', 'current', 'Import aus laterne.typ')",
        )
        .bind(node)
        .bind(body)
        .execute(pool)
        .await
        .unwrap();
    }
    work
}

async fn counts(pool: &sqlx::SqlitePool) -> Vec<i64> {
    let mut out = Vec::new();
    for table in ["work", "node", "revision", "post"] {
        out.push(
            sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT count(*) FROM {table}")))
                .fetch_one(pool)
                .await
                .unwrap(),
        );
    }
    out
}

#[tokio::test]
async fn migration_0007_keeps_every_row_of_a_round_4_database_and_never_hands_out_an_old_id_again() {
    let data = tempfile::tempdir().unwrap();
    let options = sqlx::sqlite::SqliteConnectOptions::new()
        .filename(data.path().join("baruch.sqlite"))
        .create_if_missing(true)
        .foreign_keys(true)
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal);
    let pool = sqlx::sqlite::SqlitePoolOptions::new().max_connections(1).connect_with(options).await.unwrap();
    partial(&baruch_store::MIGRATOR, 6).run(&pool).await.unwrap();
    partial(&baruch_library::MIGRATOR, 105).run(&pool).await.unwrap();
    let first = insert_book(&pool, "laternenbuch", "Laternenbuch").await;
    let second = insert_book(&pool, "laternenbuch-2", "Laternenbuch (2)").await;
    let before = counts(&pool).await;
    pool.close().await;

    let state = baruch_server::AppState::open(data.path()).await.expect("Start mit Migration 0007");
    let pool = state.store.pool();
    let versions: Vec<i64> =
        sqlx::query_scalar("SELECT version FROM _sqlx_migrations ORDER BY version").fetch_all(pool).await.unwrap();
    assert!(versions.contains(&7), "Migration 0007 ist gelaufen: {versions:?}");
    assert_eq!(counts(pool).await, before, "gleich viele Werke, Knoten, Fassungen, Beiträge; nichts neu gesät");
    let works = state.store.works().await.unwrap();
    assert_eq!(
        works.iter().map(|(id, s, _)| (*id, s.as_str())).collect::<Vec<_>>(),
        [(first, "laternenbuch"), (second, "laternenbuch-2")],
        "ids und slugs bleiben"
    );
    let violations: i64 =
        sqlx::query_scalar("SELECT count(*) FROM pragma_foreign_key_check").fetch_one(pool).await.unwrap();
    assert_eq!(violations, 0, "keine verletzten Fremdschlüssel nach dem Neubau der Tabelle work");
    for work in [first, second] {
        let book: String = BOOK.iter().map(|b| b.5).collect();
        assert_eq!(
            baruch_typst::xref::strip_key_labels(&state.store.export_typst(work).await.unwrap()).into_owned(),
            format!("{TEMPLATE}{book}"),
            "Export byte-gleich"
        );
    }
    let sql: String =
        sqlx::query_scalar("SELECT sql FROM sqlite_master WHERE name = 'work'").fetch_one(pool).await.unwrap();
    assert!(sql.contains("AUTOINCREMENT"), "work.id zählt jetzt nur aufwärts: {sql}");

    let server = common::router(std::sync::Arc::new(state));
    let deleted =
        common::rpc(server.clone(), "work.delete", json!({ "work": second, "confirm_title": "Laternenbuch (2)" }))
            .await;
    result(&deleted);
    let created = common::rpc(server, "work.create", json!({ "title": "Das Kerzenbuch" })).await;
    assert_eq!(result(&created)["work"], second + 1, "nach dem Löschen kommt die alte id nicht wieder: {created}");
}
