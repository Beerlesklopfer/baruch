// SPDX-License-Identifier: AGPL-3.0-or-later
//! JSON-RPC through `POST /rpc` and forms through `POST /act` on a real SQLite file.

use crate::common;

use baruch_server::rpc;
use serde_json::{Value, json};

use common::currents;

#[tokio::test]
async fn accepting_a_proposal_keeps_exactly_one_current() {
    let server = common::open().await;
    // Section 2 (introduction) has proposal 3/3.
    let answer = server.rpc("revision.accept", json!({ "node": 2, "rev": 3 })).await;
    assert_eq!(answer["result"]["node"], 2, "die Antwort nennt den Abschnitt: {answer}");
    assert_eq!(answer["result"]["rev"], 3, "und die jetzt aktuelle Fassung 3: {answer}");
    let node = server.rpc("node.get", json!({ "node": 2 })).await;
    assert_eq!(currents(&node["result"]), 1, "genau eine aktuelle Revision");
    assert_eq!(node["result"]["revisions"][2]["status"], "current", "der übernommene Vorschlag ist aktuell: {node}");
    let again = server.rpc("revision.accept", json!({ "node": 2, "rev": 3 })).await;
    assert_eq!(again["error"]["code"], rpc::REFUSED, "zweites Übernehmen muss abgelehnt werden: {again}");
}

#[tokio::test]
async fn saving_appends_a_human_revision_that_survives_a_restart() {
    let server = common::open().await;
    let before = server.rpc("node.get", json!({ "node": 4 })).await["result"]["revisions"].as_array().unwrap().len();
    let answer = server.rpc("revision.save", json!({ "node": 4, "body": "== Neu\r\n" })).await;
    assert_eq!(answer["result"]["rev"], before + 1, "Speichern hängt die nächste Nummer an: {answer}");
    let node = server.rpc("node.get", json!({ "node": 4 })).await;
    assert_eq!(currents(&node["result"]), 1, "nach dem Speichern genau eine aktuelle Fassung");
    assert_eq!(
        node["result"]["revisions"][before]["author"]["kind"], "human",
        "eine Speicherung stammt vom Menschen: {node}"
    );
    assert_eq!(node["result"]["revisions"][before]["body"], "== Neu\n", "CRLF wird zu LF");

    // Same text once more: no new revision, the answer shows the current one.
    let same = server.rpc("revision.save", json!({ "node": 4, "body": "== Neu\n" })).await;
    assert_eq!(same["result"]["rev"], before + 1, "gleicher Text ergibt keine neue Fassung: {same}");
    assert!(same["result"].get("commit").is_none(), "ohne Änderung kein Commit: {same}");

    // Reopen: the database is the source, not the memory.
    let state = baruch_server::AppState::open(server.path()).await.unwrap();
    let work = state.store.load_work(state.work).await.unwrap();
    assert_eq!(work.nodes[4].revisions.len(), before + 1, "Revision fehlt nach Neustart");
}

#[tokio::test]
async fn rejecting_keeps_the_current_revision() {
    let server = common::open().await;
    let answer = server.rpc("revision.reject", json!({ "node": 2, "rev": 3 })).await;
    assert_eq!(answer["result"]["node"], 2, "die Antwort nennt den Abschnitt: {answer}");
    let node = server.rpc("node.get", json!({ "node": 2 })).await;
    assert_eq!(node["result"]["revisions"][2]["status"], "rejected", "der Vorschlag ist verworfen: {node}");
    assert_eq!(
        answer["result"]["rev"],
        node["result"]["revisions"].as_array().unwrap().iter().position(|r| r["status"] == "current").unwrap() + 1,
        "die Antwort zeigt danach die unveränderte aktuelle Fassung: {answer}"
    );
    let again = server.rpc("revision.reject", json!({ "node": 2, "rev": 3 })).await;
    assert_eq!(
        again["error"]["code"],
        rpc::REFUSED,
        "ein verworfener Vorschlag lässt sich nicht noch einmal verwerfen: {again}"
    );
}

#[tokio::test]
async fn render_and_errors_map_to_rpc_codes() {
    let server = common::open().await;
    let ui = server.rpc("ui.render", json!({ "node": 2, "rev": 3 })).await;
    let editor = ui["result"]["editor"].as_str().unwrap();
    assert!(editor.contains("Übernehmen"), "Übernehmen des offenen Vorschlags fehlt im Editor");
    assert!(
        ui["result"]["tree"].as_str().unwrap().contains("Einleitung"),
        "die Gliederung nennt den gewählten Abschnitt: {ui}"
    );
    let unknown = server.rpc("nope", Value::Null).await;
    assert_eq!(unknown["error"]["code"], rpc::METHOD_NOT_FOUND, "unbekannte Methode → -32601: {unknown}");
    let bad = server.rpc("revision.accept", json!({ "node": 99, "rev": 1 })).await;
    assert_eq!(bad["error"]["code"], rpc::INVALID_PARAMS, "unbekannter Abschnitt: {bad}");
    let bad_rev = server.rpc("revision.accept", json!({ "node": 2, "rev": 9 })).await;
    assert_eq!(bad_rev["error"]["code"], rpc::INVALID_PARAMS, "unbekannte Revision: {bad_rev}");
    let empty = server.rpc("post.add", json!({ "node": 2, "body": "  " })).await;
    assert_eq!(empty["error"]["code"], rpc::INVALID_PARAMS, "leerer Beitrag: {empty}");
    let no_query = server.rpc("library.search", json!({ "query": " ** " })).await;
    assert_eq!(no_query["error"]["code"], rpc::INVALID_PARAMS, "leere Suche: {no_query}");
}

#[tokio::test]
async fn posts_are_stored_and_link_to_earlier_passages() {
    let server = common::open().await;
    let ui = server.rpc("ui.render", json!({ "node": 5 })).await;
    let html = ui["result"]["assistants"].as_str().unwrap();
    assert!(html.contains("@lektor"), "der Abschnitts-Chat zeigt den Beitrag des Lektors: {html}");
    // minijinja escapes „/“ as &#x2f; and „&“ as &amp; – the browser reads /?work=1&key=K&rev=2.
    let k = key_at(&server, 1, 4).await;
    assert!(
        html.contains(&format!("href=\"&#x2f;?work=1&amp;key={k}&amp;rev=2\"")),
        "Verweis auf 2.1 (mit Werk und Schlüssel) fehlt: {html}"
    );
    assert!(html.contains("2.1 Erste Anzeichen"), "der Verweis nennt Nummer und Titel des Ziels: {html}");

    let added = server.rpc("post.add", json!({ "node": 5, "body": "  Bitte prüfen.  " })).await;
    assert_eq!(added["result"]["node"], 5, "die Antwort nennt den Abschnitt: {added}");
    assert_eq!(added["result"]["key"], key_at(&server, 1, 5).await, "und seinen Schlüssel (Runde 8): {added}");
    assert!(added["result"]["id"].as_i64().is_some_and(|id| id > 0), "und die id des Beitrags: {added}");
    let node = server.rpc("node.get", json!({ "node": 5 })).await;
    let last = node["result"]["posts"].as_array().unwrap().last().unwrap().clone();
    assert_eq!(last["body"], "Bitte prüfen.", "Beitrag getrimmt gespeichert");
    assert_eq!(last["author"], "Autor", "Beiträge über RPC stammen vom Menschen: {last}");
}

#[tokio::test]
async fn forms_without_javascript_redirect_back_to_the_page() {
    let server = common::open().await;
    let (status, location) = server.post_form("/act", "method=revision.accept&node=2&rev=3&view=preview").await;
    assert!(status.is_redirection(), "Weiterleitung erwartet: {status}");
    let k = key_at(&server, 1, 2).await;
    assert_eq!(
        location,
        Some(format!("/?work=1&key={k}&rev=3&view=preview")),
        "zurück zur Stelle im selben Werk, über den Schlüssel"
    );
    let (status, _) = server.post_form("/act", "method=library.search&node=2").await;
    assert_eq!(status, axum::http::StatusCode::BAD_REQUEST, "nur erlaubte Methoden über /act");
}

/// Stable key (`Node::key`) of the section at position `seq`: generated addresses carry it instead of the position
/// (`/?work=W&key=K`, contract Round 8).
async fn key_at(server: &common::Server, work: i64, seq: i64) -> i64 {
    sqlx::query_scalar("SELECT id FROM node WHERE work_id = ? AND seq = ?")
        .bind(work)
        .bind(seq)
        .fetch_one(server.state.store.pool())
        .await
        .unwrap()
}
