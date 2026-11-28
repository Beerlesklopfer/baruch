// SPDX-License-Identifier: AGPL-3.0-or-later
//! Images through MCP with requests (contract Round 11, section 7): `asset_add` creates at once (origin in the mirror),
//! `role: "cover"` additionally requests the cover; `asset_replace`, `asset_set_cover`, `asset_delete` change nothing
//! and only file requests; the human confirms (RPC `asset.request_decide` or form `POST /werke/bilder/antraege/act`) or
//! declines; outdated requests are closed with a reason. Work freely invented (`tests/data/gartenbuch.typ`), images
//! created in the test.

use crate::common;

use std::net::SocketAddr;

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode, header};
use serde_json::{Value, json};

const BED: &str =
    "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"8\" height=\"4\"><rect width=\"8\" height=\"4\"/></svg>\n";
const BED_GREEN: &str = "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"8\" height=\"4\"><rect fill=\"green\" width=\"8\" height=\"4\"/></svg>\n";

async fn post(server: &common::Server, session: Option<&str>, message: Value) -> (axum::http::HeaderMap, Value) {
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

async fn initialize(server: &common::Server) -> String {
    let (headers, answer) = post(
        server,
        None,
        json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize",
                "params": { "protocolVersion": "2025-06-18", "capabilities": {},
                            "clientInfo": { "name": "Claude Code", "version": "2.1" } } }),
    )
    .await;
    assert!(answer["result"].is_object(), "initialize: {answer}");
    headers.get("mcp-session-id").unwrap().to_str().unwrap().to_owned()
}

async fn call(server: &common::Server, session: &str, tool: &str, arguments: Value) -> Value {
    let message = json!({ "jsonrpc": "2.0", "id": 3, "method": "tools/call",
                          "params": { "name": tool, "arguments": arguments } });
    post(server, Some(session), message).await.1
}

async fn ok(server: &common::Server, session: &str, tool: &str, arguments: Value) -> Value {
    let answer = call(server, session, tool, arguments).await;
    assert_eq!(answer["result"]["isError"], false, "{tool} sollte gelingen: {answer}");
    answer["result"]["structuredContent"].clone()
}

async fn refused(server: &common::Server, session: &str, tool: &str, arguments: Value) -> String {
    let answer = call(server, session, tool, arguments).await;
    assert_eq!(answer["result"]["isError"], true, "{tool} sollte fachlich scheitern: {answer}");
    answer["result"]["content"][0]["text"].as_str().unwrap().to_owned()
}

fn result(answer: &Value) -> &Value {
    assert!(answer["error"].is_null(), "RPC-Fehler: {answer}");
    &answer["result"]
}

fn commits(server: &common::Server, slug: &str) -> usize {
    common::git(&server.mirror_dir_of(slug), &["rev-list", "--count", "HEAD"]).map_or(0, |n| n.trim().parse().unwrap())
}

fn last_commit(server: &common::Server, slug: &str) -> String {
    common::git(&server.mirror_dir_of(slug), &["log", "-1", "--format=%an <%ae>%n%cn <%ce>%n%B"]).unwrap_or_default()
}

/// Gartenbuch with the image `beet.svg` (by the human) and an MCP session.
async fn garden() -> (common::Server, i64, String, String) {
    let server = common::open().await;
    let report = server.import_garden().await;
    let added = server
        .rpc(
            "asset.add",
            json!({ "work": report.work, "file_name": "beet.svg", "content": BED, "encoding": "svg",
                    "caption": "Das Beet" }),
        )
        .await;
    assert!(added["result"]["commit"].is_string(), "Bild des Menschen angelegt: {added}");
    let session = initialize(&server).await;
    (server, report.work, report.slug, session)
}

async fn revisions(server: &common::Server, work: i64, name: &str) -> Vec<baruch_store::AssetRevisionInfo> {
    server.state.store.asset_revisions(work, name).await.unwrap()
}

#[tokio::test]
async fn asset_add_creates_the_picture_at_once_with_the_client_as_author_in_one_commit() {
    let (server, work, slug, s) = garden().await;
    let before = commits(&server, &slug);
    let added = ok(
        &server,
        &s,
        "asset_add",
        json!({ "work": work, "file_name": "kompost.svg", "content": BED_GREEN, "encoding": "svg",
                "path": "bilder/kompost.svg", "alt": "Kompost", "note": "für Kapitel Sommer" }),
    )
    .await;
    assert_eq!(added["name"], "kompost.svg", "Name aus dem Dateinamen");
    assert_eq!(added["rev"], 1, "Fassung 1");
    assert_eq!(added["mime"], "image/svg+xml", "Typ erkannt");
    assert_eq!(added["bytes"], BED_GREEN.len(), "Größe entschlüsselt");
    assert_eq!(added["url"], format!("/werke/{work}/bilder/kompost.svg"), "Adresse des Bilds");
    assert!(added["commit"].is_string(), "ein Commit im Spiegel: {added}");
    assert!(added.get("request").is_none(), "ohne role kein Antrag");
    let list = revisions(&server, work, "kompost.svg").await;
    assert_eq!(list[0].author, "@mcp:claude-code", "Autor der Fassung ist der Client, nicht der Mensch");
    assert_eq!(commits(&server, &slug), before + 1, "genau ein Commit");
    let commit = last_commit(&server, &slug);
    assert!(
        commit.starts_with("MCP-Client (@mcp:claude-code) <mcp-claude-code@persona.baruch.invalid>"),
        "Autor: {commit}"
    );
    assert!(commit.contains("\nAutor <autor@baruch.invalid>\n"), "Committer der Mensch: {commit}");
    assert!(commit.contains("Bild kompost.svg: hinzugefügt"), "Betreff wie beim Menschen: {commit}");
    assert!(
        commit.contains("Baruch-Origin: mcp") && commit.contains("Persona: @mcp:claude-code"),
        "Herkunft: {commit}"
    );

    let persona = ok(
        &server,
        &s,
        "asset_add",
        json!({ "file_name": "laube.svg", "content": BED, "encoding": "svg", "persona": "@lektor", "work": work }),
    )
    .await;
    assert_eq!(revisions(&server, work, "laube.svg").await[0].author, "@lektor", "mit persona ist sie der Autor");
    assert!(persona["commit"].is_string(), "ein Commit");
    let commit = last_commit(&server, &slug);
    assert!(commit.contains("(@lektor) <lektor@persona.baruch.invalid>"), "Commit-Autor die Persona: {commit}");
    assert!(commit.contains("Baruch-Origin: persona"), "Herkunft persona: {commit}");
}

#[tokio::test]
async fn asset_add_refuses_more_than_8_mib_and_wrong_arguments() {
    let (server, work, slug, s) = garden().await;
    let before = commits(&server, &slug);
    // Over 8 MiB: the route /mcp today accepts at most 2 MiB (default of axum, 413); the tool itself rejects over 8 MiB
    // decoded (unit test in src/mcp/assets.rs). Neither changes anything.
    let big = vec![0u8; baruch_store::MAX_ASSET_BYTES + 1];
    let message = json!({ "jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": { "name": "asset_add",
        "arguments": { "work": work, "file_name": "riesig.png", "content": baruch_store::encode_base64(&big), "encoding": "base64" } } });
    let mut request = Request::post("/mcp")
        .header(header::CONTENT_TYPE, "application/json")
        .header("mcp-session-id", s.as_str())
        .body(Body::from(message.to_string()))
        .unwrap();
    request.extensions_mut().insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 40000))));
    let (status, _, body) = server.send(request).await;
    let refused_big = status == StatusCode::PAYLOAD_TOO_LARGE
        || serde_json::from_str::<Value>(&body).is_ok_and(|a| a["result"]["isError"] == true && body.contains("8 MiB"));
    assert!(
        refused_big,
        "über 8 MiB wird abgelehnt (413 bzw. isError mit 8 MiB): {status} {}",
        &body[..body.len().min(300)]
    );
    let wrong =
        call(&server, &s, "asset_add", json!({ "file_name": "x.svg", "content": BED, "encoding": "zip" })).await;
    assert_eq!(wrong["error"]["code"], -32602, "unbekannte Kodierung ist ein Argumentfehler: {wrong}");
    let role = call(
        &server,
        &s,
        "asset_add",
        json!({ "file_name": "x.svg", "content": BED, "encoding": "svg", "role": "titel" }),
    )
    .await;
    assert_eq!(role["error"]["code"], -32602, "nur role cover: {role}");
    assert_eq!(server.state.store.asset_list(work).await.unwrap().len(), 1, "kein Bild dazugekommen");
    assert_eq!(commits(&server, &slug), before, "kein Commit");
}

#[tokio::test]
async fn role_cover_adds_the_picture_and_opens_a_request_for_the_cover() {
    let (server, work, slug, s) = garden().await;
    let added = ok(
        &server,
        &s,
        "asset_add",
        json!({ "work": work, "file_name": "umschlag.svg", "content": BED_GREEN, "encoding": "svg", "role": "cover" }),
    )
    .await;
    assert_eq!(added["name"], "umschlag.svg", "das Bild ist angelegt");
    let request = &added["request"];
    assert_eq!(request["op"], "set_cover", "dazu ein Antrag für den Umschlag: {added}");
    assert_eq!(request["name"], "umschlag.svg", "für genau dieses Bild");
    assert!(request["decision"].is_null(), "offen");
    let listed = ok(&server, &s, "asset_requests", json!({ "work": work })).await;
    assert_eq!(listed["requests"].as_array().unwrap().len(), 1, "asset_requests zeigt ihn: {listed}");
    let rpc = server.rpc("asset.request_list", json!({ "work": work })).await;
    assert_eq!(result(&rpc)["requests"][0]["id"], request["id"], "und RPC asset.request_list");
    assert_eq!(result(&rpc)["requests"][0]["origin_label"], "MCP @mcp:claude-code", "mit Herkunft");
    let rejected =
        server.rpc("asset.request_decide", json!({ "work": work, "id": request["id"], "decision": "reject" })).await;
    assert_eq!(result(&rejected)["request"]["decision"], "rejected", "Ablehnen schließt den Antrag");
    let _ = slug;
}

#[tokio::test]
async fn replace_set_cover_and_delete_only_open_requests() {
    let (server, work, slug, s) = garden().await;
    let before = commits(&server, &slug);
    let revs = revisions(&server, work, "beet.svg").await;
    let replace = ok(
        &server,
        &s,
        "asset_replace",
        json!({ "work": work, "name": "beet.svg", "content": BED_GREEN, "encoding": "svg", "alt": "Grünes Beet",
                "note": "grüner" }),
    )
    .await;
    assert_eq!(replace["status"], "open", "nur ein Antrag: {replace}");
    assert_eq!(replace["request"]["op"], "replace", "Art replace");
    assert!(replace["request"].get("content").is_none(), "der Inhalt steht nicht in der Antwort");
    assert_eq!(replace["request"]["bytes"], BED_GREEN.len(), "aber die Größe");
    let cover = ok(&server, &s, "asset_set_cover", json!({ "work": work, "name": "beet.svg" })).await;
    assert_eq!(cover["status"], "open", "Umschlag nur als Antrag");
    let delete = ok(&server, &s, "asset_delete", json!({ "work": work, "name": "beet.svg", "note": "weg" })).await;
    assert_eq!(delete["status"], "open", "Löschen nur als Antrag");
    assert_eq!(revisions(&server, work, "beet.svg").await, revs, "das Bild ist unverändert");
    assert_eq!(commits(&server, &slug), before, "kein Commit für Anträge");

    let twice = refused(&server, &s, "asset_delete", json!({ "work": work, "name": "beet.svg", "note": "weg" })).await;
    assert!(twice.contains("already exists"), "derselbe offene Antrag nicht zweimal (MCP englisch): {twice}");
    let unknown = refused(&server, &s, "asset_delete", json!({ "work": work, "name": "fehlt.svg" })).await;
    assert!(unknown.contains("fehlt.svg"), "unbekanntes Bild: {unknown}");
    let same = refused(
        &server,
        &s,
        "asset_replace",
        json!({ "work": work, "name": "beet.svg", "content": BED, "encoding": "svg" }),
    )
    .await;
    assert!(same.contains("unchanged"), "gleicher Inhalt ist kein Antrag (MCP englisch): {same}");
    let none = refused(&server, &s, "asset_set_cover", json!({ "work": work, "name": null })).await;
    assert!(none.contains("no cover"), "nichts zu entfernen: {none}");
    let mine = ok(&server, &s, "asset_requests", json!({ "work": work, "mine": true, "persona": "@lektor" })).await;
    assert!(mine["requests"].as_array().unwrap().is_empty(), "von @lektor gibt es keinen: {mine}");
    let all = ok(&server, &s, "asset_requests", json!({ "work": work, "mine": true })).await;
    assert_eq!(all["requests"].as_array().unwrap().len(), 3, "drei eigene Anträge");
}

#[tokio::test]
async fn confirming_a_replacement_runs_it_with_requester_and_confirmer_in_one_commit() {
    let (server, work, slug, s) = garden().await;
    let asked = ok(
        &server,
        &s,
        "asset_replace",
        json!({ "work": work, "name": "beet.svg", "content": BED_GREEN, "encoding": "svg", "alt": "Grünes Beet",
                "note": "grüner wirkt freundlicher" }),
    )
    .await;
    let id = asked["request"]["id"].as_i64().unwrap();
    let (status, _, body) = server
        .send(Request::get(format!("/werke/{work}/bilder/antraege/{id}/datei")).body(Body::empty()).unwrap())
        .await;
    assert_eq!(status, StatusCode::OK, "der vorgeschlagene Inhalt lässt sich ansehen");
    assert_eq!(body, BED_GREEN, "genau der beantragte Inhalt");
    let before = commits(&server, &slug);

    let decided = server.rpc("asset.request_decide", json!({ "work": work, "id": id, "decision": "confirm" })).await;
    let r = result(&decided);
    assert_eq!(r["request"]["decision"], "confirmed", "bestätigt: {r}");
    assert_eq!(r["request"]["decided_by"], "Autor", "vom Menschen");
    assert!(r["commit"].is_string(), "mit Commit: {r}");
    let revs = revisions(&server, work, "beet.svg").await;
    let current = revs.iter().find(|r| r.current).unwrap();
    assert_eq!(current.number, 2, "neue Fassung 2 ist aktuell");
    assert_eq!(current.alt, "Grünes Beet", "Alternativtext des Antrags gehört zur neuen Fassung");
    assert_eq!(current.caption, "Das Beet", "Bildunterschrift geht mit");
    let data = server.state.store.asset_data(work, "beet.svg", None).await.unwrap();
    assert_eq!(data.content, BED_GREEN, "der beantragte Inhalt ist jetzt aktuell");
    assert_eq!(commits(&server, &slug), before + 1, "genau ein Commit");
    let commit = last_commit(&server, &slug);
    assert!(commit.starts_with("Autor <autor@baruch.invalid>"), "Autor des Commits ist der Bestätiger: {commit}");
    assert!(commit.contains(&format!("(Antrag {id})")), "Betreff nennt den Antrag: {commit}");
    assert!(
        commit.contains(&format!("Antrag {id} von @mcp:claude-code, bestätigt von Autor")),
        "Rumpf nennt Antragsteller und Bestätiger: {commit}"
    );
    assert!(commit.contains("Begründung: grüner wirkt freundlicher"), "und die Begründung");
    for trailer in [
        "Baruch-Action: asset-replaced".to_owned(),
        "Baruch-Origin: mcp".to_owned(),
        format!("Baruch-Request: {id}"),
        "Baruch-Requested-By: @mcp:claude-code".to_owned(),
        "Baruch-Confirmed-By: Autor".to_owned(),
    ] {
        assert!(commit.contains(&trailer), "Trailer {trailer}: {commit}");
    }
    let again = server.rpc("asset.request_decide", json!({ "work": work, "id": id, "decision": "confirm" })).await;
    assert_eq!(again["error"]["code"], -32000, "nur einmal entscheiden: {again}");
}

#[tokio::test]
async fn the_form_confirms_a_deletion_and_rejecting_changes_nothing() {
    let (server, work, slug, s) = garden().await;
    let first = ok(&server, &s, "asset_delete", json!({ "work": work, "name": "beet.svg" })).await["request"]["id"]
        .as_i64()
        .unwrap();
    let before = commits(&server, &slug);
    let (status, location) =
        server.post_form("/werke/bilder/antraege/act", &format!("work={work}&id={first}&decision=reject")).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Formular leitet weiter");
    assert_eq!(
        location.as_deref(),
        Some(format!("/werke/{work}/bilder?hinweis=antrag_abgelehnt#antraege").as_str()),
        "zurück zu den Anträgen mit Hinweis"
    );
    assert_eq!(revisions(&server, work, "beet.svg").await.len(), 1, "Ablehnen ändert nichts");
    assert_eq!(commits(&server, &slug), before, "und schreibt keinen Commit");

    let second = ok(&server, &s, "asset_delete", json!({ "work": work, "name": "beet.svg", "persona": "@lektor" }))
        .await["request"]["id"]
        .as_i64()
        .unwrap();
    let (status, location) =
        server.post_form("/werke/bilder/antraege/act", &format!("work={work}&id={second}&decision=confirm")).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Formular leitet weiter");
    assert_eq!(
        location.as_deref(),
        Some(format!("/werke/{work}/bilder?hinweis=antrag_bestaetigt#antraege").as_str()),
        "Hinweis bestätigt"
    );
    assert!(server.state.store.asset_list(work).await.unwrap().is_empty(), "das Bild ist gelöscht");
    let commit = last_commit(&server, &slug);
    assert!(commit.contains("Bild beet.svg: gelöscht"), "Betreff wie beim Menschen: {commit}");
    assert!(commit.contains("Baruch-Requested-By: @lektor"), "Antragsteller die Persona: {commit}");
    assert!(commit.contains("Gelöschter Inhalt (SVG"), "der gelöschte Inhalt bleibt im Commit: {commit}");

    let foreign = Request::post("/werke/bilder/antraege/act")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .header(header::ORIGIN, "http://fremd.example")
        .header(header::HOST, "127.0.0.1:8765")
        .body(Body::from(format!("work={work}&id={first}&decision=confirm")))
        .unwrap();
    let (status, _, _) = server.send(foreign).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "das Formular liegt hinter same_origin");
}

#[tokio::test]
async fn a_stale_request_is_closed_with_a_reason_and_changes_nothing() {
    let (server, work, slug, s) = garden().await;
    let id = ok(
        &server,
        &s,
        "asset_replace",
        json!({ "work": work, "name": "beet.svg", "content": BED_GREEN, "encoding": "svg" }),
    )
    .await["request"]["id"]
        .as_i64()
        .unwrap();
    // Meanwhile the human replaces the image himself.
    let other = BED.replace("width=\"8\" height=\"4\"/>", "width=\"8\" height=\"3\"/>");
    let human = server
        .rpc(
            "asset.replace",
            json!({ "work": work, "name": "beet.svg", "file_name": "beet.svg", "content": other, "encoding": "svg" }),
        )
        .await;
    assert!(human["result"]["commit"].is_string(), "der Mensch ersetzt: {human}");
    let before = commits(&server, &slug);
    let (status, location) =
        server.post_form("/werke/bilder/antraege/act", &format!("work={work}&id={id}&decision=confirm")).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Formular leitet weiter");
    assert!(location.unwrap().contains("hinweis=antrag_veraltet"), "Hinweis veraltet");
    let request = server.state.store.asset_request_get(work, id).await.unwrap();
    assert_eq!(request.decision, Some(baruch_store::Decision::Stale), "als veraltet geschlossen");
    assert!(
        request.message.as_deref().unwrap().contains("andere aktuelle Fassung (2)"),
        "mit Grund: {:?}",
        request.message
    );
    assert_eq!(
        server.state.store.asset_data(work, "beet.svg", None).await.unwrap().content,
        other,
        "die Fassung des Menschen bleibt"
    );
    assert_eq!(commits(&server, &slug), before, "kein Commit");
}

#[tokio::test]
async fn the_panel_lists_open_requests_with_forms_and_decided_ones_with_reason() {
    let (server, work, _, s) = garden().await;
    let open = ok(
        &server,
        &s,
        "asset_replace",
        json!({ "work": work, "name": "beet.svg", "content": BED_GREEN, "encoding": "svg",
                                                      "note": "Bitte <grün>" }),
    )
    .await["request"]["id"]
        .as_i64()
        .unwrap();
    let closed = ok(&server, &s, "asset_set_cover", json!({ "work": work, "name": "beet.svg" })).await["request"]["id"]
        .as_i64()
        .unwrap();
    let rejected =
        server.rpc("asset.request_decide", json!({ "work": work, "id": closed, "decision": "reject" })).await;
    assert!(rejected["result"].is_object(), "{rejected}");
    let html = baruch_server::i18n::scope(
        baruch_server::i18n::Lang::De,
        baruch_server::asset_requests::panel(&server.state, work),
    )
    .await
    .unwrap();
    assert!(html.contains("id=\"antraege\""), "Abschnitt #antraege für die Seite „Bilder“");
    assert!(html.contains("(1 offen)"), "Zahl der offenen: {html}");
    assert!(html.contains(&format!("id=\"bildantrag-{open}\"")), "offener Antrag mit Anker");
    assert!(html.contains("<form method=\"post\" action=\"/werke/bilder/antraege/act\">"), "Formulare ohne JavaScript");
    assert!(
        html.contains("name=\"decision\" value=\"confirm\"") && html.contains("name=\"decision\" value=\"reject\""),
        "beide Knöpfe"
    );
    assert!(html.contains("Bitte &lt;grün&gt;"), "Begründung maskiert: {html}");
    assert!(html.contains("„beet.svg“ als Umschlag setzen") && html.contains("abgelehnt"), "entschiedene mit Ergebnis");
    assert!(html.contains(&format!("/bilder/antraege/{open}/datei")), "Vorschau des vorgeschlagenen Inhalts");
}
