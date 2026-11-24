// SPDX-License-Identifier: AGPL-3.0-or-later
//! Nutzerfrage 12 (Entscheidung des Nutzers 2026-11-07: „es braucht eine aktive Zustimmung (Popup)“):
//! Legt ein Assistent über MCP `outline_add` einen Teil so an, dass vorhandene Kapitel unter ihn wandern,
//! entsteht ein Antrag statt des Teils. Der Mensch stimmt zu oder lehnt ab – im Dialog (Widget
//! `consent`) oder ohne JavaScript über Hinweisleiste und Formular auf `/gliederung`. Herkunft und
//! Entscheidung stehen im Git-Spiegel. Ein Teil, der nichts übernimmt (am Werkende), bleibt Anlegen.
//! Alle Werke und Texte frei erfunden.

use crate::common;

use std::net::SocketAddr;

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode, header};
use baruch_core::{Author, NodeKind};
use serde_json::{Value, json};

const COAST: &str = "= Ebbe\n\nDas Wasser weicht.\n\n= Flut\n\nDas Wasser kommt.\n\n= Brandung\n\nGischt am Fels.\n";

fn result(answer: &Value) -> &Value {
    assert!(answer["error"].is_null(), "RPC sollte gelingen: {answer}");
    &answer["result"]
}

async fn key_of(server: &common::Server, work: i64, title: &str) -> i64 {
    let loaded = server.state.store.load_work(work).await.unwrap();
    loaded.nodes.iter().find(|n| n.title == title).unwrap_or_else(|| panic!("kein Punkt „{title}“")).key
}

/// Baum als (Art, Titel, Titel des Elternpunkts) in Baumreihenfolge.
async fn tree(server: &common::Server, work: i64) -> Vec<(NodeKind, String, Option<String>)> {
    let loaded = server.state.store.load_work(work).await.unwrap();
    loaded.nodes.iter().map(|n| (n.kind, n.title.clone(), n.parent.map(|p| loaded.nodes[p].title.clone()))).collect()
}

/// Commits im Spiegel (neueste zuerst); `None` ohne git-Programm.
fn log(server: &common::Server, slug: &str) -> Option<Vec<String>> {
    common::git(&server.mirror_dir_of(slug), &["log", "--format=%H"])
        .map(|text| text.lines().map(str::to_owned).collect())
}

fn last_commit(server: &common::Server, slug: &str) -> Option<String> {
    common::git(&server.mirror_dir_of(slug), &["log", "-1", "--format=%an%n%B"])
}

// ---- MCP (wie round14.rs) ---------------------------------------------------------------------------

async fn mcp_post(server: &common::Server, session: Option<&str>, message: Value) -> (axum::http::HeaderMap, Value) {
    let mut request = Request::post("/mcp").header(header::CONTENT_TYPE, "application/json");
    if let Some(id) = session {
        request = request.header("mcp-session-id", id);
    }
    let mut request = request.body(Body::from(message.to_string())).unwrap();
    request.extensions_mut().insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 40000))));
    let (status, headers, body) = server.send(request).await;
    assert_eq!(status, StatusCode::OK, "MCP antwortet: {body}");
    (headers, serde_json::from_str(&body).unwrap())
}

async fn mcp_session(server: &common::Server) -> String {
    let (headers, answer) = mcp_post(
        server,
        None,
        json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize",
                "params": { "protocolVersion": "2025-06-18", "capabilities": {},
                            "clientInfo": { "name": "Gezeiten", "version": "1.0" } } }),
    )
    .await;
    assert!(answer["result"].is_object(), "initialize: {answer}");
    headers.get("mcp-session-id").unwrap().to_str().unwrap().to_owned()
}

async fn mcp_ok(server: &common::Server, session: &str, tool: &str, arguments: Value) -> Value {
    let message = json!({ "jsonrpc": "2.0", "id": 3, "method": "tools/call",
                          "params": { "name": tool, "arguments": arguments } });
    let answer = mcp_post(server, Some(session), message).await.1;
    assert_eq!(answer["result"]["isError"], false, "{tool} sollte gelingen: {answer}");
    answer["result"]["structuredContent"].clone()
}

struct Coast {
    server: common::Server,
    work: i64,
    slug: String,
    session: String,
}

async fn coast() -> Coast {
    let server = common::open().await;
    let report = server.import(COAST, "kueste.typ").await;
    let session = mcp_session(&server).await;
    Coast { server, work: report.work, slug: report.slug, session }
}

/// Der Assistent will „Vom Wasser“ vor „Flut“ anlegen: „Flut“ und „Brandung“ würden wandern → Antrag.
async fn request_part(c: &Coast) -> (i64, Value) {
    let flood = key_of(&c.server, c.work, "Flut").await;
    let answer = mcp_ok(
        &c.server,
        &c.session,
        "outline_add",
        json!({ "work": c.work, "key": flood, "place": "before", "title": "Vom Wasser", "level": 0,
                "note": "Die Gezeiten gehören zusammen." }),
    )
    .await;
    (answer["request"]["id"].as_i64().unwrap_or_else(|| panic!("Antrag mit id erwartet: {answer}")), answer)
}

fn original_tree() -> Vec<(NodeKind, String, Option<String>)> {
    ["Ebbe", "Flut", "Brandung"].iter().map(|t| (NodeKind::Chapter, (*t).to_owned(), None)).collect()
}

// ---- Antrag statt Anlegen ---------------------------------------------------------------------------

#[tokio::test]
async fn a_part_in_the_middle_becomes_a_request_and_changes_nothing() {
    let c = coast().await;
    let commits = log(&c.server, &c.slug);
    let flood = key_of(&c.server, c.work, "Flut").await;
    let surf = key_of(&c.server, c.work, "Brandung").await;
    let (id, answer) = request_part(&c).await;
    assert_eq!(answer["action"], "requested", "kein Anlegen, sondern ein Antrag: {answer}");
    assert_eq!(answer["status"], "open", "der Antrag ist offen: {answer}");
    let message = answer["message"].as_str().unwrap();
    assert!(
        message.contains(&format!("Antrag {id} gestellt, wartet auf Zustimmung")),
        "die Antwort sagt ehrlich, dass nichts geschehen ist und worauf sie wartet: {message}"
    );
    assert_eq!(answer["adopted"], json!([flood, surf]), "adopted nennt die Kapitel, die wandern würden");
    assert_eq!(answer["request"]["op"]["op"], "add", "Antragsart add (Migration 0035): {answer}");
    assert!(answer["key"].is_null() && answer["points"].is_null(), "kein neuer Punkt in der Antwort: {answer}");
    assert_eq!(tree(&c.server, c.work).await, original_tree(), "die Gliederung ist unverändert");
    assert_eq!(log(&c.server, &c.slug), commits, "und kein Commit, solange der Mensch nicht entschieden hat");

    // outline_requests zeigt die Vorschau: der neue Teil „neu“, die beiden Kapitel „wandern“.
    let listed = mcp_ok(&c.server, &c.session, "outline_requests", json!({ "work": c.work, "preview": true })).await;
    let request = &listed["requests"][0];
    assert_eq!(request["id"], id, "der Antrag steht in der Liste: {listed}");
    let marks: Vec<(String, String)> = request["preview"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| (p["title"].as_str().unwrap().to_owned(), p["mark"].as_str().unwrap().to_owned()))
        .collect();
    let expected: Vec<(String, String)> =
        [("Ebbe", "same"), ("Vom Wasser", "added"), ("Flut", "adopted"), ("Brandung", "adopted")]
            .iter()
            .map(|(t, m)| ((*t).to_owned(), (*m).to_owned()))
            .collect();
    assert_eq!(marks, expected, "Vorschau zeigt Teil und wandernde Kapitel in der neuen Reihenfolge");
}

#[tokio::test]
async fn confirming_moves_the_chapters_under_the_part_and_logs_origin_and_decision() {
    let c = coast().await;
    let (id, _) = request_part(&c).await;
    let decided =
        c.server.rpc("outline.request_decide", json!({ "work": c.work, "id": id, "decision": "confirm" })).await;
    assert_eq!(result(&decided)["request"]["decision"], "confirmed", "{decided}");
    let part = Some("Vom Wasser".to_owned());
    assert_eq!(
        tree(&c.server, c.work).await,
        vec![
            (NodeKind::Chapter, "Ebbe".to_owned(), None),
            (NodeKind::Part, "Vom Wasser".to_owned(), None),
            (NodeKind::Chapter, "Flut".to_owned(), part.clone()),
            (NodeKind::Chapter, "Brandung".to_owned(), part),
        ],
        "nach der Zustimmung stehen „Flut“ und „Brandung“ unter dem neuen Teil, „Ebbe“ bleibt davor"
    );
    let part_key = key_of(&c.server, c.work, "Vom Wasser").await;
    let loaded = c.server.state.store.load_work(c.work).await.unwrap();
    let node = loaded.nodes.iter().find(|n| n.key == part_key).unwrap();
    let author = &node.revisions[node.current_index().unwrap()].author;
    assert!(
        matches!(author, Author::Assistant { persona, .. } if persona == "@mcp:gezeiten"),
        "Revision 1 trägt den Assistenten, der den Teil verfasst hat: {author:?}"
    );
    if let Some(commit) = last_commit(&c.server, &c.slug) {
        assert!(commit.contains(&format!("Baruch-Request: {id}")), "Spiegel nennt den Antrag: {commit}");
        assert!(commit.contains("Baruch-Origin: mcp"), "Herkunft im Spiegel: {commit}");
        assert!(commit.contains("Baruch-Requested-By: @mcp:gezeiten"), "Antragsteller im Spiegel: {commit}");
        assert!(commit.contains("Baruch-Confirmed-By: Autor"), "Zustimmung des Menschen im Spiegel: {commit}");
        assert!(commit.contains("Begründung: Die Gezeiten gehören zusammen."), "Begründung im Rumpf: {commit}");
    }
}

#[tokio::test]
async fn rejecting_changes_nothing_but_the_decision_is_in_the_mirror() {
    let c = coast().await;
    let (id, _) = request_part(&c).await;
    let commits = log(&c.server, &c.slug);
    let decided =
        c.server.rpc("outline.request_decide", json!({ "work": c.work, "id": id, "decision": "reject" })).await;
    assert_eq!(result(&decided)["request"]["decision"], "rejected", "{decided}");
    assert!(result(&decided)["outline"].is_null(), "keine Gliederungsänderung: {decided}");
    assert_eq!(tree(&c.server, c.work).await, original_tree(), "Ablehnen ändert die Gliederung nicht");
    if let (Some(before), Some(after)) = (commits, log(&c.server, &c.slug)) {
        assert_eq!(after.len(), before.len() + 1, "genau ein Protokoll-Commit für die Ablehnung");
        let commit = last_commit(&c.server, &c.slug).unwrap();
        assert!(commit.contains(&format!("Gliederung: Antrag {id} abgelehnt")), "Betreff: {commit}");
        assert!(commit.contains("Baruch-Decision: rejected"), "Entscheidung im Spiegel: {commit}");
        assert!(commit.contains("Baruch-Requested-By: @mcp:gezeiten"), "Herkunft im Spiegel: {commit}");
        assert!(commit.contains("Baruch-Decided-By: Autor"), "wer entschieden hat: {commit}");
        let changed = common::git(&c.server.mirror_dir_of(&c.slug), &["show", "--stat", "--format=", "HEAD"]);
        assert_eq!(changed.as_deref().map(str::trim), Some(""), "der Commit ändert keine Datei");
    }
    let open = c.server.rpc("outline.requests", json!({ "work": c.work })).await;
    assert!(result(&open)["requests"].as_array().unwrap().is_empty(), "nicht mehr offen: {open}");
}

#[tokio::test]
async fn a_part_at_the_end_of_the_work_is_still_created_directly() {
    let c = coast().await;
    let surf = key_of(&c.server, c.work, "Brandung").await;
    let answer = mcp_ok(
        &c.server,
        &c.session,
        "outline_add",
        json!({ "work": c.work, "key": surf, "place": "after", "title": "Nachspiel", "level": 0 }),
    )
    .await;
    assert_eq!(answer["action"], "added", "ein Teil ohne Folgekapitel übernimmt nichts → Anlegen (AGENTS.md)");
    assert_eq!(answer["adopted"], json!([]), "kein Kapitel wandert: {answer}");
    assert!(answer["request"].is_null(), "kein Antrag: {answer}");
    let shown = tree(&c.server, c.work).await;
    assert_eq!(shown.last(), Some(&(NodeKind::Part, "Nachspiel".to_owned(), None)), "Teil am Werkende: {shown:?}");
    let open = c.server.rpc("outline.requests", json!({ "work": c.work })).await;
    assert!(result(&open)["requests"].as_array().unwrap().is_empty(), "kein offener Antrag: {open}");
    if let Some(commit) = last_commit(&c.server, &c.slug) {
        assert!(commit.starts_with("MCP-Client (@mcp:gezeiten)"), "direkt angelegt, Autor der Client: {commit}");
    }
}

#[tokio::test]
async fn a_request_whose_chapters_changed_meanwhile_is_closed_as_stale() {
    let c = coast().await;
    let (id, _) = request_part(&c).await;
    // Inzwischen legt der Mensch selbst einen Teil vor „Brandung“ an: der Antrag übernähme nur noch „Flut“.
    let surf = key_of(&c.server, c.work, "Brandung").await;
    let own = c
        .server
        .rpc(
            "outline.add",
            json!({ "work": c.work, "target_key": surf, "place": "before", "title": "Am Fels", "level": 0 }),
        )
        .await;
    result(&own);
    let shown = tree(&c.server, c.work).await;
    let listed = c.server.rpc("outline.requests", json!({ "work": c.work, "preview": true })).await;
    let error = result(&listed)["requests"][0]["preview_error"].as_str().unwrap_or_default().to_owned();
    assert!(error.contains("andere Kapitel"), "die Vorschau sagt, warum sie nicht mehr gilt: {listed}");
    let decided =
        c.server.rpc("outline.request_decide", json!({ "work": c.work, "id": id, "decision": "confirm" })).await;
    let request = &result(&decided)["request"];
    assert_eq!(request["decision"], "stale", "nicht stillschweigend etwas anderes ausführen: {decided}");
    assert!(request["message"].as_str().unwrap().contains("andere Kapitel"), "Grund genannt: {decided}");
    assert_eq!(tree(&c.server, c.work).await, shown, "nichts geändert");
}

// ---- Oberfläche: Hinweisleiste, Dialog, ohne JavaScript ---------------------------------------------

#[tokio::test]
async fn every_page_shows_the_bar_and_the_dialog_while_a_request_waits() {
    let c = coast().await;
    let (status, page) = c.server.get("/werke").await;
    assert_eq!(status, StatusCode::OK, "die Werkliste muss ohne offenen Antrag normal laden");
    assert!(!page.contains("consent-bar") && !page.contains("<!--baruch-consent-->"), "ohne Antrag nichts davon");

    let (id, _) = request_part(&c).await;
    let link = format!("/gliederung?work={}#antrag-{id}", c.work);
    for uri in ["/werke".to_owned(), format!("/?work={}", c.work), format!("/einfaelle?work={}", c.work)] {
        let (status, page) = c.server.get(&uri).await;
        assert_eq!(status, StatusCode::OK, "{uri}");
        assert!(page.contains("class=\"consent-bar\""), "{uri}: Hinweisleiste auf jeder Seite");
        assert!(page.contains(&format!("href=\"{link}\"")), "{uri}: ohne JavaScript ein Link zum Antrag");
        assert!(page.contains("data-widget=\"consent\""), "{uri}: Dialog für das Widget");
        assert!(!page.contains("<!--baruch-consent-->"), "{uri}: die Marke ist ersetzt");
    }
    let (_, page) = c.server.get("/werke").await;
    let dialog = &page[page.find("<dialog class=\"consent-dialog\"").unwrap()..];
    let dialog = &dialog[..dialog.find("</dialog>").unwrap()];
    let start_tag = &dialog[..dialog.find('>').unwrap()];
    assert!(!start_tag.contains(" open"), "der Dialog öffnet erst durch das Widget (ohne JavaScript die Leiste)");
    assert!(dialog.contains("Vom Wasser") && dialog.contains("wandert unter den neuen Teil"), "Vorschau im Dialog");
    assert!(
        dialog.contains("name=\"decision\" value=\"confirm\">Zustimmen</button>")
            && dialog.contains("name=\"decision\" value=\"reject\">Ablehnen</button>"),
        "die beiden Knöpfe: {dialog}"
    );
    assert!(!dialog.contains("<script") && !dialog.contains("onclick"), "kein Inline-JavaScript");
    let script = baruch_server::SCRIPTS.iter().find(|(name, _)| *name == "consent.js").expect("Widget consent.js");
    assert!(
        !script.1.contains("setTimeout") && !script.1.contains("setInterval"),
        "kein Zeitablauf im Widget: Zustimmung nur durch Klick"
    );
    let main = baruch_server::SCRIPTS.iter().find(|(name, _)| *name == "main.js").unwrap();
    assert!(main.1.contains("./consent.js"), "main.js lädt das Widget");
    // JSON bleibt unberührt.
    let answer = c.server.rpc("outline.requests", json!({ "work": c.work })).await;
    assert!(!answer.to_string().contains("consent-bar"), "nur HTML-Seiten bekommen die Leiste");
}

#[tokio::test]
async fn without_javascript_the_form_on_the_request_page_decides_with_csrf_protection() {
    let c = coast().await;
    let (id, _) = request_part(&c).await;
    let (status, page) = c.server.get(&format!("/gliederung?work={}", c.work)).await;
    assert_eq!(status, StatusCode::OK, "die Gliederungsseite mit dem offenen Antrag muss laden");
    let row = &page[page.find(&format!("id=\"antrag-{id}\"")).expect("Antrag auf der Seite")..];
    let row = &row[..row.find("</tr>").unwrap()];
    assert!(row.contains(">Zustimmen</button>") && row.contains(">Ablehnen</button>"), "Formular-Knöpfe: {row}");
    assert!(row.contains("wandert unter den neuen Teil"), "Vorschau ohne JavaScript: {row}");

    // Fremde Seite: abgelehnt, nichts geändert (derselbe Schutz wie bei allen Formularen).
    let form = format!("method=outline.request_decide&work={}&id={id}&decision=confirm&view=gliederung", c.work);
    let request = Request::post("/gliederung/act")
        .header(header::HOST, "127.0.0.1:8765")
        .header(header::ORIGIN, "https://evil.example")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(form.clone()))
        .unwrap();
    assert_eq!(c.server.send(request).await.0, StatusCode::FORBIDDEN, "fremde Seite → 403");
    assert_eq!(tree(&c.server, c.work).await, original_tree(), "abgelehntes Formular ändert nichts");

    let (status, location) = c.server.post_form("/gliederung/act", &form).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "ohne JavaScript: 303");
    assert!(location.unwrap().ends_with("#antraege"), "zurück zu den Anträgen");
    assert_eq!(tree(&c.server, c.work).await[1].1, "Vom Wasser", "zugestimmt: der Teil steht da");
    let (_, page) = c.server.get("/werke").await;
    assert!(!page.contains("consent-bar"), "nach der Entscheidung verschwindet die Leiste");
}
