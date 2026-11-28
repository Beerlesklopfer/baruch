// SPDX-License-Identifier: AGPL-3.0-or-later
//! End-to-end tests of Round 7 (integrator): the book bug „Absatz wird Titel“ and the title rules on all ways,
//! proposals of two personas in the reading flow in their colours (one accepted, one folded), an idea created over MCP,
//! processed in the user interface as an outline item, deleted and restored (with `einfaelle.md` and commits in the
//! mirror), and the migrations 0010 and 0011 together on a database in the state development through a full server
//! start. Real data directory in the temp directory; all texts freely invented.

use crate::common;

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::path::Path;
use std::str::FromStr;

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode, header};
use serde_json::{Value, json};
use sqlx::SqlitePool;
use sqlx::migrate::Migrator;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

fn result(answer: &Value, what: &str) -> Value {
    assert!(answer.get("error").is_none(), "{what} scheitert: {answer}");
    answer["result"].clone()
}

fn commits(dir: &Path) -> Option<u32> {
    common::git(dir, &["rev-list", "--count", "HEAD"]).map(|c| c.trim().parse().unwrap())
}

fn last_message(dir: &Path) -> Option<String> {
    common::git(dir, &["log", "-1", "--format=%an%n%B"])
}

/// Outline as (title, level, key, first line of the current revision), parts left out.
async fn outline(server: &common::Server, work: i64) -> Vec<(String, u8, i64, String)> {
    let loaded = result(&server.rpc("work.get", json!({ "work": work })).await, "work.get");
    let nodes = loaded["work"]["nodes"].as_array().unwrap().clone();
    let mut levels: Vec<u8> = Vec::new();
    let mut out = Vec::new();
    for node in &nodes {
        let parent = node["parent"].as_u64().map(|p| levels[p as usize]);
        let level = match node["kind"].as_str().unwrap() {
            "part" => 0,
            "chapter" => 1,
            _ if parent.is_some_and(|p| p >= 2) => 3,
            _ => 2,
        };
        levels.push(level);
        let current = node["revisions"].as_array().unwrap().iter().find(|r| r["status"] == "current").unwrap();
        let first = current["body"].as_str().unwrap().lines().next().unwrap_or("").to_owned();
        out.push((node["title"].as_str().unwrap().to_owned(), level, node["key"].as_i64().unwrap(), first));
    }
    out
}

fn key_of(points: &[(String, u8, i64, String)], title: &str) -> i64 {
    points.iter().find(|p| p.0 == title).unwrap_or_else(|| panic!("„{title}“ fehlt: {points:?}")).2
}

fn assert_titles_match_headings(points: &[(String, u8, i64, String)], when: &str) {
    for (title, level, _, first) in points {
        let expected = format!("{} {title}", "=".repeat(usize::from(*level)));
        assert_eq!(first, &expected, "{when}: Überschriftszeile und Gliederung laufen auseinander: {points:?}");
    }
}

async fn body_of(server: &common::Server, work: i64, key: i64) -> String {
    let node = result(&server.rpc("node.get", json!({ "work": work, "key": key })).await, "node.get");
    let current = node["revisions"].as_array().unwrap().iter().find(|r| r["status"] == "current").unwrap().clone();
    current["body"].as_str().unwrap().to_owned()
}

/// As the typeset view saves: parse → serialize with the instructions → check → save.
async fn save_from_sheet(server: &common::Server, work: i64, key: i64, edits: Value) -> Value {
    let body = body_of(server, work, key).await;
    result(&server.rpc("wysiwyg.parse", json!({ "work": work, "body": body })).await, "wysiwyg.parse");
    let text = result(
        &server.rpc("wysiwyg.serialize", json!({ "work": work, "body": body, "edits": edits })).await,
        "wysiwyg.serialize",
    )["body"]
        .as_str()
        .unwrap()
        .to_owned();
    let check = result(&server.rpc("outline.check", json!({ "work": work, "key": key, "body": text })).await, "check");
    let split = !check["split"].as_array().unwrap().is_empty();
    server.rpc("outline.save", json!({ "work": work, "key": key, "body": text, "split": split })).await
}

/// MCP request from the own machine (without a session: client `@mcp:unbekannt`).
async fn mcp(server: &common::Server, tool: &str, arguments: Value) -> Value {
    let message = json!({ "jsonrpc": "2.0", "id": 9, "method": "tools/call",
        "params": { "name": tool, "arguments": arguments } });
    let mut request = Request::post("/mcp")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(message.to_string()))
        .unwrap();
    request.extensions_mut().insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 40000))));
    let (status, _, body) = server.send(request).await;
    assert_eq!(status, StatusCode::OK, "tools/call {tool}: {body}");
    serde_json::from_str(&body).unwrap()
}

fn form(fields: &[(&str, &str)]) -> String {
    fields.iter().map(|(k, v)| format!("{k}={}", baruch_server::view::query_escape(v))).collect::<Vec<_>>().join("&")
}

const LEAVES: &str = "Im Oktober färbt sich der Ahorn hinter Annas Laube rot, und die Amseln suchen im Laub.";

// ---- Book bug and title rules -----------------------------------------------------------------

#[tokio::test]
async fn paragraphs_in_a_new_chapter_never_become_titles_and_bad_titles_are_refused_on_every_way() {
    let server = common::open().await;
    let report = server.import_garden().await;
    let work = report.work;
    let mirror = server.mirror_dir_of(&report.slug);
    let summer = key_of(&outline(&server, work).await, "Sommer");

    // New chapter and a section below it, as in the tree.
    result(
        &server
            .rpc("outline.add", json!({ "work": work, "target_key": summer, "place": "after", "title": "Herbst" }))
            .await,
        "Kapitel anlegen",
    );
    let autumn = key_of(&outline(&server, work).await, "Herbst");
    result(
        &server
            .rpc("outline.add", json!({ "work": work, "target_key": autumn, "place": "into", "title": "Ahorn" }))
            .await,
        "Abschnitt anlegen",
    );
    let maple = key_of(&outline(&server, work).await, "Ahorn");

    // Typeset view: after the title a line break and a paragraph (this is how the old widget sent it), plus two further
    // paragraphs.
    let edits = json!([
        { "replaces": 0, "block": { "kind": "heading", "level": 2, "inlines": [
            { "t": "text", "v": "Ahorn" }, { "t": "soft" }, { "t": "text", "v": LEAVES } ] } },
        { "replaces": null, "block": { "kind": "paragraph", "inlines": [ { "t": "text", "v": "Dann kommt der Frost." } ] } },
        { "replaces": null, "block": { "kind": "paragraph", "inlines": [ { "t": "text", "v": "Die Beete ruhen." } ] } }
    ]);
    result(&save_from_sheet(&server, work, maple, edits).await, "Speichern aus dem Satzbild");
    assert_eq!(
        body_of(&server, work, maple).await,
        format!("== Ahorn\n\n{LEAVES}\n\nDann kommt der Frost.\n\nDie Beete ruhen.\n\n"),
        "die Absätze stehen unter der Überschrift, nicht in ihr"
    );
    // Change the level: through the outline and through the format selection in the typeset view.
    result(&server.rpc("outline.set_level", json!({ "work": work, "key": maple, "level": 1 })).await, "Ebene 1");
    assert_titles_match_headings(&outline(&server, work).await, "nach outline.set_level");
    let edits = json!([
        { "replaces": 0, "block": { "kind": "heading", "level": 2, "inlines": [ { "t": "text", "v": "Ahorn" } ] } },
        { "keep": 1 }, { "keep": 2 }, { "keep": 3 }
    ]);
    result(&save_from_sheet(&server, work, maple, edits).await, "Format-Auswahl Überschrift 2");
    let points = outline(&server, work).await;
    assert_titles_match_headings(&points, "nach der Format-Auswahl");
    let titles: Vec<&str> = points.iter().map(|p| p.0.as_str()).collect();
    assert_eq!(
        titles,
        ["Frühling", "Aussaat", "Tiefe", "Sommer", "Herbst", "Ahorn"],
        "kein Absatz wurde Titel, kein Punkt kam hinzu"
    );

    // Title rules: empty, multi-line, 201 characters – through the tree form, RPC and MCP.
    let long = "a".repeat(201);
    let export_before = result(&server.rpc("work.export", json!({ "work": work })).await, "export")["typst"].clone();
    let commits_before = commits(&mirror);
    for (title, start) in [
        (" \t ", "Ein Gliederungspunkt braucht einen Titel."),
        ("Ahorn\nund Eiche", "Der Titel darf keinen Zeilenumbruch enthalten."),
        (long.as_str(), "Der Titel ist zu lang (höchstens 200 Zeichen, hier 201)."),
    ] {
        let renamed = server.rpc("outline.rename", json!({ "work": work, "key": maple, "title": title })).await;
        assert_eq!(renamed["error"]["code"], -32602, "RPC outline.rename „{title:?}“ → -32602: {renamed}");
        assert_eq!(renamed["error"]["message"], start, "RPC mit deutscher Meldung");
        let fields = form(&[
            ("method", "outline.rename"),
            ("work", &work.to_string()),
            ("key", &maple.to_string()),
            ("node", "5"),
            ("title", title),
        ]);
        let (status, _) = server.post_form("/gliederung/act", &fields).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "Baum-Formular „{title:?}“ → 422");
        // Over MCP a title stands only in the heading line of a proposal; a line break ends it there (the rest is
        // text), therefore only empty and too long.
        if !title.contains('\n') {
            let body = format!("== {title}\n\nText.\n");
            let answer = mcp(
                &server,
                "propose_revision",
                json!({ "work": work, "key": maple, "body": body, "persona": "@lektor" }),
            )
            .await;
            assert_eq!(answer["result"]["isError"], true, "MCP-Vorschlag mit Titel „{title:?}“ abgelehnt: {answer}");
            let english = baruch_server::store_text::translate(baruch_server::i18n::Lang::En, start);
            assert_eq!(
                answer["result"]["content"][0]["text"], english,
                "MCP mit derselben Meldung, englisch: {answer}"
            );
        }
    }
    assert_eq!(
        result(&server.rpc("work.export", json!({ "work": work })).await, "export")["typst"],
        export_before,
        "keine Ablehnung hat den Text geändert"
    );
    assert_eq!(commits(&mirror), commits_before, "keine Ablehnung hat einen Commit erzeugt");
}

// ---- Proposals in the reading flow ------------------------------------------------------------

async fn page_with_cookie(server: &common::Server, uri: &str, cookie: &str) -> String {
    let request = Request::get(uri).header(header::COOKIE, cookie).body(Body::empty()).unwrap();
    let (status, _, html) = server.send(request).await;
    assert_eq!(status, StatusCode::OK, "{uri} lädt: {html}");
    html
}

/// Opening `<details …>` of the proposal `<key>-<rev>`, if present.
fn pv_tag(html: &str, key: i64, rev: u32) -> Option<&str> {
    let at = html.find(&format!("data-pv=\"{key}-{rev}\""))?;
    let start = html[..at].rfind("<details")?;
    Some(&html[start..at + html[at..].find('>').unwrap() + 1])
}

#[tokio::test]
async fn two_personas_propose_in_two_colors_one_is_folded_and_the_other_accepted() {
    let server = common::open().await;
    let report = server.import_garden().await;
    let work = report.work;
    let mirror = server.mirror_dir_of(&report.slug);
    let points = outline(&server, work).await;
    let seed = key_of(&points, "Aussaat");
    let seq = points.iter().position(|p| p.2 == seed).unwrap();
    let store = &server.state.store;
    let lektor = store
        .add_proposal(work, seq, "== Aussaat\n\nRadieschen in Reihen, Möhren dazwischen.\n", "@lektor", "m-1", "Reihen")
        .await
        .unwrap();
    let nathan = store
        .add_proposal(work, seq, "== Aussaat\n\nIm April säen, im Mai jäten.\n", "@nathan", "m-2", "Monate")
        .await
        .unwrap();
    assert_eq!((lektor, nathan), (2, 3), "zwei Vorschläge auf Fassung 1");

    let colors = store.persona_colors().await.unwrap();
    let color = |handle: &str| colors.iter().find(|c| c.handle == handle).unwrap().color.clone();
    assert_ne!(color("@lektor"), color("@nathan"), "zwei Personas haben verschiedene Vorgabefarben");

    // Nathan's proposal is folded (cookie), the lector's open; both in their colour.
    let cookie = format!("baruch_folded={seed}-{nathan}");
    let uri = format!("/?work={work}&node={seq}&rev=1");
    let html = page_with_cookie(&server, &uri, &cookie).await;
    let lektor_tag = pv_tag(&html, seed, lektor).expect("Vorschlag des Lektors im Lesefluss");
    let nathan_tag = pv_tag(&html, seed, nathan).expect("Vorschlag Nathans im Lesefluss");
    let lektor_style = baruch_server::personas::style(&color("@lektor"));
    let nathan_style = baruch_server::personas::style(&color("@nathan"));
    assert!(lektor_tag.contains(&format!("style=\"{lektor_style}\"")), "Lektor in seiner Farbe: {lektor_tag}");
    assert!(nathan_tag.contains(&format!("style=\"{nathan_style}\"")), "Nathan in seiner Farbe: {nathan_tag}");
    assert!(lektor_tag.ends_with(" open>"), "nicht gefaltet → offen: {lektor_tag}");
    assert!(!nathan_tag.contains(" open"), "gefaltet → nur die farbige Linie: {nathan_tag}");
    assert!(
        html.find(&format!("data-pv=\"{seed}-{lektor}\"")) < html.find(&format!("data-pv=\"{seed}-{nathan}\"")),
        "Reihenfolge der Personas: Lektor vor Nathan"
    );
    // In the preview (reading flow across the chapter) both stand after the section as well.
    let preview = page_with_cookie(&server, &format!("{uri}&view=preview"), &cookie).await;
    assert!(
        pv_tag(&preview, seed, lektor).is_some() && pv_tag(&preview, seed, nathan).is_some(),
        "Vorschau zeigt beide Vorschläge"
    );

    // Accept via form (without JavaScript): exactly one commit, the lector's revision becomes current.
    let before = commits(&mirror);
    let fields = format!("method=revision.accept&work={work}&node={seq}&key={seed}&rev={lektor}&view=edit");
    let (status, location) = server.post_form("/act", &fields).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Übernehmen leitet zurück: {location:?}");
    let node = result(&server.rpc("node.get", json!({ "work": work, "key": seed })).await, "node.get");
    assert_eq!(common::currents(&node), 1, "genau eine aktuelle Fassung");
    let current = node["revisions"].as_array().unwrap().iter().find(|r| r["status"] == "current").unwrap();
    assert_eq!(current["number"], lektor, "der Vorschlag des Lektors ist aktuell");
    if let (Some(before), Some(after)) = (before, commits(&mirror)) {
        assert_eq!(after, before + 1, "Übernehmen ist genau ein Commit");
    }
    let html = page_with_cookie(&server, &format!("/?work={work}&node={seq}"), &cookie).await;
    assert!(pv_tag(&html, seed, lektor).is_none(), "der übernommene Vorschlag ist keine Variante mehr");
    let nathan_tag = pv_tag(&html, seed, nathan).expect("Nathans Vorschlag bleibt offen im Sinne von „proposed“");
    assert!(!nathan_tag.contains(" open"), "und bleibt gefaltet: {nathan_tag}");
}

// ---- Ideas ----------------------------------------------------------------------------------

#[tokio::test]
async fn an_idea_from_mcp_becomes_an_outline_point_in_the_ui_and_survives_delete_and_restore() {
    let server = common::open().await;
    let report = server.import_garden().await;
    let work = report.work;
    let mirror = server.mirror_dir_of(&report.slug);
    let summer = key_of(&outline(&server, work).await, "Sommer");
    let ideas_file = mirror.join("einfaelle.md");
    assert!(!ideas_file.exists(), "ohne Einfälle keine einfaelle.md");

    // Create over MCP as a persona.
    let created = mcp(
        &server,
        "idea_create",
        json!({ "work": work, "key": summer, "words": ["Kompost", "Regenwurm"],
                "note": "Der Kompost <wärmt> & nährt.", "persona": "@lektor" }),
    )
    .await;
    assert_eq!(created["result"]["isError"], false, "idea_create gelingt: {created}");
    let idea = created["result"]["structuredContent"]["idea"].clone();
    let id = idea["id"].as_i64().unwrap();
    assert_eq!(idea["origin"], json!({ "kind": "persona", "handle": "@lektor" }), "Herkunft Persona");
    let md = std::fs::read_to_string(&ideas_file).expect("einfaelle.md nach dem Anlegen");
    assert!(md.contains(&format!("## Einfall {id} · offen")), "Eintrag offen: {md}");
    assert!(md.contains("- Gedanke: Der Kompost <wärmt> & nährt."), "Werte wörtlich: {md}");
    if let Some(message) = last_message(&mirror) {
        for part in [
            format!("Einfall {id}: angelegt"),
            format!("Baruch-Idea: {id}"),
            "Baruch-Action: idea-create".into(),
            "Baruch-Origin: persona".into(),
            "Persona: @lektor".into(),
        ] {
            assert!(message.contains(&part), "Commit nennt {part}: {message}");
        }
    }

    // Process in the user interface: first a multi-line title (rejected, stays open), then a valid one.
    let id_text = id.to_string();
    let bad = form(&[("method", "idea.process"), ("id", &id_text), ("as", "outline"), ("title", "Kompost\nWurm")]);
    let (status, _) = server.post_form("/einfaelle/act", &bad).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "Titelregeln gelten auch beim Verarbeiten");
    let open = result(&server.rpc("idea.get", json!({ "id": id })).await, "idea.get");
    assert_eq!(open["idea"]["status"], "open", "abgelehnt → der Einfall bleibt offen");
    let good = form(&[
        ("method", "idea.process"),
        ("id", &id_text),
        ("as", "outline"),
        ("place", "after"),
        ("title", "Kompost"),
        ("back", &format!("/einfaelle/{id}")),
    ]);
    let (status, location) = server.post_form("/einfaelle/act", &good).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Verarbeiten leitet zurück");
    assert_eq!(location.as_deref(), Some(format!("/einfaelle/{id}").as_str()), "auf die Seite des Einfalls");
    let points = outline(&server, work).await;
    let compost = key_of(&points, "Kompost");
    assert_titles_match_headings(&points, "nach dem Verarbeiten");
    let done = result(&server.rpc("idea.get", json!({ "id": id })).await, "idea.get");
    assert_eq!(done["idea"]["status"], "done", "verarbeitet");
    assert_eq!(done["idea"]["done_ref"], format!("key {compost}"), "mit Verweis auf den neuen Punkt");
    if let Some(log) = common::git(&mirror, &["log", "-2", "--format=%B"]) {
        assert!(log.contains("Baruch-Action: idea-process"), "Commit der Verarbeitung: {log}");
        assert!(log.contains("Baruch-Action: outline-add"), "und der Gliederungs-Commit: {log}");
    }

    // Delete over MCP (restorable), restore in the user interface.
    let deleted = mcp(&server, "idea_delete", json!({ "id": id, "persona": "@lektor" })).await;
    assert_eq!(deleted["result"]["isError"], false, "idea_delete gelingt: {deleted}");
    assert!(!ideas_file.exists(), "ohne vorhandene Einfälle verschwindet einfaelle.md");
    if let Some(message) = last_message(&mirror) {
        assert!(message.contains("Baruch-Action: idea-delete"), "Lösch-Commit: {message}");
        assert!(message.contains("Der Kompost <wärmt> & nährt."), "der Lösch-Commit trägt den ganzen Eintrag");
    }
    let (status, _) = server.post_form("/einfaelle/act", &form(&[("method", "idea.restore"), ("id", &id_text)])).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Wiederherstellen per Formular");
    let md = std::fs::read_to_string(&ideas_file).expect("einfaelle.md wieder da");
    assert!(md.contains(&format!("## Einfall {id} · verarbeitet (outline)")), "verarbeitet wiederhergestellt: {md}");
    if let Some(message) = last_message(&mirror) {
        assert!(message.contains("Baruch-Action: idea-restore"), "Commit des Wiederherstellens: {message}");
        assert!(message.contains("Baruch-Origin: human"), "der Mensch stellt wieder her: {message}");
    }
    let (status, page) = server.get(&format!("/einfaelle?work={work}")).await;
    assert_eq!(status, StatusCode::OK, "Seite der Einfälle");
    assert!(page.contains("Der Kompost &lt;wärmt&gt; &amp; nährt."), "maskiert in der Liste");
}

// ---- Migrations 0010 and 0011 together -------------------------------------------------------

const MILL: &str = "#set document(title: \"Die Gartenmühle\", author: \"Ida Feder\")\n\
#let korn(body) = block(inset: 3pt, body)\n\n\
= Mahlen\n\nDas Rad dreht sich.\n\n#image(\"./bilder/rad.svg\")\n\n\
== Mehl\n\n#korn[Fein und weiß.]\n\n= Backen\n\nBrot aus dem Ofen.\n";

const WHEEL: &str = "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"4\" height=\"4\"><circle r=\"2\"/></svg>";

async fn raw_pool(dir: &Path) -> SqlitePool {
    let options = SqliteConnectOptions::from_str("sqlite:")
        .unwrap()
        .filename(dir.join("baruch.sqlite"))
        .create_if_missing(true)
        .foreign_keys(true);
    SqlitePoolOptions::new().max_connections(1).connect_with(options).await.unwrap()
}

/// Store 0001–0009 and library 0101–0105: the state of development before Round 7.
async fn migrate_like_development(pool: &SqlitePool) {
    let store: Vec<_> = baruch_store::MIGRATOR.iter().filter(|m| m.version <= 9).cloned().collect();
    Migrator { migrations: Cow::Owned(store), ignore_missing: true, ..Migrator::DEFAULT }.run(pool).await.unwrap();
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../baruch-library/migrations");
    let mut library = Migrator::new(dir).await.unwrap();
    library.migrations = Cow::Owned(library.iter().filter(|m| m.version <= 105).cloned().collect());
    library.ignore_missing = true;
    library.run(pool).await.unwrap();
}

async fn exec(pool: &SqlitePool, sql: &str, binds: &[&str]) -> i64 {
    let mut query = sqlx::query_scalar::<_, i64>(sqlx::AssertSqlSafe(sql));
    for b in binds {
        query = query.bind(*b);
    }
    query.fetch_optional(pool).await.unwrap().unwrap_or(0)
}

/// Work like an import in development, with personas, one proposal, one post and one image.
async fn insert_development_work(pool: &SqlitePool) -> i64 {
    let doc = baruch_typst::parse(MILL).unwrap();
    let work: i64 = sqlx::query_scalar(
        "INSERT INTO work (slug, kind, title, author, template, source_name, source_sha256, title_key) \
         VALUES ('die-gartenmuehle', 'book', 'Die Gartenmühle', 'Ida Feder', ?, 'muehle.typ', 'erfunden', ?) RETURNING id",
    )
    .bind(&doc.template)
    .bind(baruch_store::title_key("Die Gartenmühle"))
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO work_template_revision (work_id, number, body, author_name, note) VALUES (?, 1, ?, 'Autor', 'Import')")
        .bind(work)
        .bind(&doc.template)
        .execute(pool)
        .await
        .unwrap();
    let mut chapter: Option<(i64, i64)> = None;
    let mut roots = 0i64;
    let mut first_section = None;
    for (seq, heading) in doc.headings.iter().enumerate() {
        let (parent, position, kind) = if heading.level == 1 {
            roots += 1;
            chapter = Some((seq as i64, 0));
            (None, roots - 1, "chapter")
        } else {
            let (parent, children) = chapter.as_mut().unwrap();
            *children += 1;
            (Some(*parent), *children - 1, "section")
        };
        let node: i64 = sqlx::query_scalar(
            "INSERT INTO node (work_id, seq, parent_seq, position, number, title, kind) VALUES (?, ?, ?, ?, '1', ?, ?) RETURNING id",
        )
        .bind(work)
        .bind(seq as i64)
        .bind(parent)
        .bind(position)
        .bind(&heading.title)
        .bind(kind)
        .fetch_one(pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO revision (node_id, number, body, author_kind, author_name, status, note) \
             VALUES (?, 1, ?, 'human', 'Autor', 'current', 'Import')",
        )
        .bind(node)
        .bind(&heading.body)
        .execute(pool)
        .await
        .unwrap();
        if kind == "section" && first_section.is_none() {
            first_section = Some(node);
        }
    }
    let section = first_section.unwrap().to_string();
    for (handle, name, position) in [("@lektor", "Lektor", "0"), ("@nathan", "Nathan", "1")] {
        exec(
            pool,
            "INSERT INTO persona (handle, name, role, model, context_length, active, position) VALUES (?, ?, 'Rolle', 'm', 8192, 1, ?)",
            &[handle, name, position],
        )
        .await;
    }
    exec(
        pool,
        "INSERT INTO revision (node_id, number, body, author_kind, author_name, model, status, note, based_on) \
         VALUES (?, 2, '== Mehl\n\nGrob und braun.\n', 'assistant', '@nathan', 'm', 'proposed', 'Vollkorn', 1)",
        &[&section],
    )
    .await;
    exec(pool, "INSERT INTO post (node_id, position, author, body) VALUES (?, 0, '@lektor', 'Gut so.')", &[&section])
        .await;
    let work_text = work.to_string();
    let asset = exec(pool, "INSERT INTO asset (work_id, name) VALUES (?, 'rad.svg') RETURNING id", &[&work_text])
        .await
        .to_string();
    exec(
        pool,
        "INSERT INTO asset_revision (asset_id, number, mime, encoding, content, bytes, sha256, alt, author_name, status) \
         VALUES (?, 1, 'image/svg+xml', 'svg', ?, ?, 'erfunden', 'Mühlrad', 'Autor', 'current')",
        &[&asset, WHEEL, &WHEEL.len().to_string()],
    )
    .await;
    // In development the store rejected `./` in paths: the image hangs at `bilder/rad.svg`, the text writes
    // `./bilder/rad.svg` (the normal form since Round 7 finds it).
    exec(
        pool,
        "INSERT INTO asset_path (work_id, path, asset_id) VALUES (?, 'bilder/rad.svg', ?)",
        &[&work_text, &asset],
    )
    .await;
    exec(pool, "INSERT OR REPLACE INTO store_meta (key, value) VALUES ('seeded', '1')", &[]).await;
    // The conversion of old part texts at start (user question 8) is checked by `round14_conversion.rs`; this is only
    // about the migrations, therefore it counts as done (flag `parts_marked_r14`).
    exec(pool, "INSERT OR REPLACE INTO store_meta (key, value) VALUES (?, '1')", &[baruch_store::CONVERTED_KEY]).await;
    work
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

async fn count(pool: &SqlitePool, sql: &str) -> i64 {
    sqlx::query_scalar(sqlx::AssertSqlSafe(sql)).fetch_one(pool).await.unwrap()
}

#[tokio::test]
async fn migrations_0010_and_0011_together_keep_a_development_database_lossless() {
    let data = tempfile::tempdir().unwrap();
    let pool = raw_pool(data.path()).await;
    migrate_like_development(&pool).await;
    let work = insert_development_work(&pool).await;
    assert!(baruch_library::seed_if_empty(&pool).await.unwrap(), "Beispielbibliothek gesät wie in development");
    let old_columns = table_columns(&pool).await;
    let before = rows(&pool, &old_columns).await;
    pool.close().await;

    // Full server start: Store::open (0010, 0011), library, no seed, bring the mirror up to date.
    let state = baruch_server::AppState::open(data.path()).await.expect("Start auf der Datenbank von development");
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
    // After color later migrations may append further columns (0015: prompt).
    let persona_old = old_columns["persona"].len();
    assert_eq!(
        new_columns["persona"].get(persona_old).map(String::as_str),
        Some("color"),
        "0010 hängt persona.color an"
    );
    assert_eq!(count(&pool, "SELECT count(*) FROM persona WHERE color IS NOT NULL").await, 0, "0010 setzt keine Farbe");
    for table in ["idea", "idea_word", "idea_fts"] {
        assert!(new_columns.contains_key(table), "0011 legt {table} an");
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
    assert_eq!(
        baruch_typst::xref::strip_key_labels(&state.store.export_typst(work).await.unwrap()).into_owned(),
        MILL,
        "Export byte-gleich zur Quelle"
    );
    let versions: Vec<i64> =
        sqlx::query_scalar("SELECT version FROM _sqlx_migrations ORDER BY version").fetch_all(&pool).await.unwrap();
    assert!(versions.contains(&10) && versions.contains(&11) && versions.contains(&105), "0010, 0011: {versions:?}");
    let mirror = data.path().join("works/die-gartenmuehle");
    assert!(mirror.join("main.typ").exists(), "Spiegel beim Start nachgeführt");
    assert!(!mirror.join("einfaelle.md").exists(), "ohne Einfälle keine einfaelle.md");
    assert!(!data.path().join("einfaelle").exists(), "kein Spiegel für Einfälle ohne Werk angelegt");
    let colors = state.store.persona_colors().await.unwrap();
    assert_eq!(
        colors.iter().map(|c| (c.handle.as_str(), c.color.as_str(), c.chosen)).collect::<Vec<_>>(),
        [("@lektor", baruch_store::PERSONA_PALETTE[0], false), ("@nathan", baruch_store::PERSONA_PALETTE[1], false)],
        "Vorgabefarben nach Reihenfolge der Personas"
    );

    // After the migration: the typeset view finds the image with a ./ path, proposal in the reading flow, an idea can
    // be created.
    let state = std::sync::Arc::new(state);
    let server = common::Server { app: common::router(state.clone()), state, data };
    let (status, html) = server.get(&format!("/?work={work}&node=1")).await;
    assert_eq!(status, StatusCode::OK, "Arbeitsbereich lädt");
    assert!(html.contains("class=\"pv"), "Vorschlag aus development erscheint als Variante im Lesefluss");
    let parsed = server.rpc("wysiwyg.parse", json!({ "work": work, "body": "#image(\"./bilder/rad.svg\")\n" })).await;
    let parsed_html = result(&parsed, "wysiwyg.parse")["html"].as_str().unwrap().to_owned();
    assert!(parsed_html.contains("alt=\"Mühlrad\""), "Alternativtext über den ./-Pfad gefunden: {parsed_html}");
    let created =
        server.rpc("idea.create", json!({ "work": work, "words": "Mühle, Korn", "note": "Mahlgang prüfen" })).await;
    result(&created, "Einfall nach der Migration");

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
