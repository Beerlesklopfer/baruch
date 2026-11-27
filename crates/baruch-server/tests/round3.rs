// SPDX-License-Identifier: AGPL-3.0-or-later
//! End-to-end acceptance of Round 3 (revtree · litpopup · small) after the merge: proposal over MCP → switch → switch
//! back with one mirror commit each; changing and deleting only by the human; MCP reads through numbers after the
//! deletion; a literature hit opens the section at the place found; highlight → quotation as a proposal → accept; port
//! default 8765; no `import_typst` in `tools/list`. Real data directory (SQLite, library, Git mirror) in the temp
//! directory.

use crate::common;

use std::net::SocketAddr;
use std::path::Path;
use std::process::Command;

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode, header};
use serde_json::{Value, json};

/// MCP request from the own machine (without a token only loopback).
async fn mcp(server: &common::Server, id: u64, method: &str, params: Value) -> Value {
    let message = json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params });
    let mut request = Request::post("/mcp")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(message.to_string()))
        .unwrap();
    request.extensions_mut().insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 40000))));
    let (status, _, body) = server.send(request).await;
    assert_eq!(status, StatusCode::OK, "MCP {method} antwortet mit 200: {body}");
    serde_json::from_str(&body).unwrap()
}

/// Successful MCP tool call: `structuredContent`.
async fn tool(server: &common::Server, name: &str, arguments: Value) -> Value {
    let answer = mcp(server, 9, "tools/call", json!({ "name": name, "arguments": arguments })).await;
    assert_eq!(answer["result"]["isError"], false, "Werkzeug {name} sollte gelingen: {answer}");
    answer["result"]["structuredContent"].clone()
}

fn commit_count(dir: &Path) -> Option<u32> {
    common::git(dir, &["rev-list", "--count", "HEAD"]).map(|n| n.trim().parse().unwrap())
}

fn last_subject(dir: &Path) -> String {
    common::git(dir, &["log", "-1", "--format=%s"]).unwrap().trim().to_owned()
}

fn last_trailers(dir: &Path) -> String {
    common::git(dir, &["log", "-1", "--format=%(trailers:only,unfold)"]).unwrap()
}

/// Stable key (`key`) of the section at position `seq`: writing over MCP and quoting take only it (contract Round 5).
async fn key_of(server: &common::Server, work: i64, seq: i64) -> i64 {
    sqlx::query_scalar("SELECT id FROM node WHERE work_id = ? AND seq = ?")
        .bind(work)
        .bind(seq)
        .fetch_one(server.state.store.pool())
        .await
        .unwrap()
}

/// (number, status) of all revisions of a section according to `node.get`.
async fn numbers(server: &common::Server, node: usize) -> Vec<(u64, String)> {
    let answer = server.rpc("node.get", json!({ "node": node })).await;
    answer["result"]["revisions"]
        .as_array()
        .unwrap_or_else(|| panic!("node.get {node} ohne Revisionen: {answer}"))
        .iter()
        .map(|r| (r["number"].as_u64().unwrap(), r["status"].as_str().unwrap().to_owned()))
        .collect()
}

async fn current_of(server: &common::Server, node: usize) -> u64 {
    let all = numbers(server, node).await;
    let current: Vec<u64> = all.iter().filter(|(_, s)| s == "current").map(|(n, _)| *n).collect();
    assert_eq!(current.len(), 1, "genau eine aktuelle Fassung je Abschnitt: {all:?}");
    current[0]
}

#[tokio::test]
async fn a_proposal_from_mcp_is_switched_on_and_back_with_one_commit_each() {
    let server = common::open().await;
    let dir = server.mirror_dir();
    let before = current_of(&server, 2).await;

    let intro = key_of(&server, 1, 2).await;
    let proposed = tool(
        &server,
        "propose_revision",
        json!({ "work": 1, "key": intro, "body": "= Einleitung\n\nNeu über MCP.\n" }),
    )
    .await;
    let rev = proposed["rev"].as_u64().unwrap();
    assert_eq!(proposed["status"], "proposed", "MCP legt nur einen Vorschlag an: {proposed}");
    assert_eq!(current_of(&server, 2).await, before, "der Vorschlag ändert die Markierung nicht");
    let start = commit_count(&dir);

    let on = server.rpc("revision.set_current", json!({ "node": 2, "rev": rev })).await;
    assert_eq!(on["result"]["rev"], rev, "der Vorschlag ist jetzt aktuell: {on}");
    assert!(on["result"]["commit"].is_string(), "Umschalten meldet den Commit: {on}");
    assert_eq!(current_of(&server, 2).await, rev, "Markierung auf dem Vorschlag");

    let back = server.rpc("revision.set_current", json!({ "node": 2, "rev": before })).await;
    assert_eq!(back["result"]["rev"], before, "zurück auf die alte Fassung: {back}");
    assert!(back["result"]["commit"].is_string(), "Zurückschalten meldet den Commit: {back}");
    let all = numbers(&server, 2).await;
    assert!(all.contains(&(rev, "superseded".to_owned())), "der Vorschlag bleibt als andere Fassung: {all:?}");

    let Some(start) = start else { return };
    assert_eq!(commit_count(&dir), Some(start + 2), "jede Umschaltung ist genau ein Commit");
    assert!(last_subject(&dir).ends_with(&format!("auf Fassung {before} umgeschaltet")), "Betreff nennt das Ziel");
    let trailers = last_trailers(&dir);
    assert!(trailers.contains("Baruch-Action: switched"), "Trailer der Umschaltung: {trailers}");
    assert!(trailers.contains(&format!("Baruch-Revision: {before}")), "Trailer nennt die Nummer: {trailers}");
    let previous = common::git(&dir, &["log", "-1", "--skip=1", "--format=%s"]).unwrap();
    assert!(
        previous.trim().ends_with(&format!("Vorschlag {rev} übernommen")),
        "Umschalten auf einen Vorschlag heißt Übernehmen: {previous}"
    );
}

#[tokio::test]
async fn the_human_edits_and_deletes_but_mcp_cannot_and_reads_by_number_afterwards() {
    let server = common::open().await;
    let current = current_of(&server, 2).await;
    let all = numbers(&server, 2).await;
    let older = all.iter().find(|(n, _)| *n < current).map(|(n, _)| *n).expect("eine ältere Fassung");
    let proposal = all.iter().find(|(_, s)| s == "proposed").map(|(n, _)| *n).expect("ein Vorschlag");

    // MCP: neither changing nor deleting nor switching.
    let list = mcp(&server, 1, "tools/list", json!({})).await;
    let names: Vec<&str> =
        list["result"]["tools"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap()).collect();
    for name in ["update_revision", "delete_revision", "set_current", "import_typst"] {
        assert!(!names.contains(&name), "MCP bietet {name} nicht an: {names:?}");
        let answer =
            mcp(&server, 2, "tools/call", json!({ "name": name, "arguments": { "node": 2, "rev": older } })).await;
        assert_eq!(answer["error"]["code"], -32602, "{name} ist über MCP ein unbekanntes Werkzeug: {answer}");
    }
    assert_eq!(numbers(&server, 2).await, all, "die MCP-Versuche ändern nichts");

    // Human: changing and deleting over RPC.
    let edited = server.rpc("revision.update", json!({ "node": 2, "rev": older, "note": "vom Menschen" })).await;
    assert_eq!(edited["result"]["changed"], true, "der Mensch darf eine Fassung ändern: {edited}");
    let deleted = server.rpc("revision.delete", json!({ "node": 2, "rev": proposal, "confirm": true })).await;
    assert_eq!(deleted["result"]["deleted"], proposal, "der Mensch darf einen Vorschlag löschen: {deleted}");
    let refused = server.rpc("revision.delete", json!({ "node": 2, "rev": current, "confirm": true })).await;
    assert_eq!(refused["error"]["code"], -32000, "die aktuelle Fassung lässt sich nicht löschen: {refused}");

    // New revision after the gap: MCP reads through numbers, not through positions.
    let saved = server.rpc("revision.save", json!({ "node": 2, "body": "= Einleitung\n\nNach der Lücke.\n" })).await;
    let fresh = saved["result"]["rev"].as_u64().unwrap_or_else(|| panic!("Speichern scheiterte: {saved}"));
    assert!(fresh > proposal, "die gelöschte Nummer wird nicht wiederverwendet: {fresh} nach {proposal}");
    let read = tool(&server, "read_node", json!({ "work": 1, "node": 2 })).await;
    assert_eq!(read["rev"], fresh, "read_node nennt die Nummer der aktuellen Fassung: {read}");
    assert!(read["text"].as_str().unwrap().contains("Nach der Lücke."), "und ihren Text: {read}");
    let listed: Vec<u64> =
        read["revisions"].as_array().unwrap().iter().map(|r| r["number"].as_u64().unwrap()).collect();
    assert!(!listed.contains(&proposal), "die gelöschte Fassung fehlt: {listed:?}");
    let outline = tool(&server, "outline", json!({ "work": 1 })).await;
    let entry = outline["nodes"].as_array().unwrap().iter().find(|n| n["node"] == 2).unwrap();
    assert_eq!(entry["current_rev"], fresh, "outline.current_rev ist die Nummer trotz Lücke: {entry}");
}

#[tokio::test]
async fn a_literature_hit_opens_the_section_at_the_place_found() {
    let server = common::open().await;
    let saved = server
        .rpc("revision.save", json!({ "node": 4, "body": "== Erste Anzeichen\n\nDer Zaunkoenigsgesang beginnt.\n" }))
        .await;
    let rev = saved["result"]["rev"].as_u64().unwrap_or_else(|| panic!("Speichern scheiterte: {saved}"));

    let (status, page) = server.get("/literatur?node=4&lq=Zaunkoenigsgesang").await;
    assert_eq!(status, StatusCode::OK, "Literatur-Seite ohne JavaScript: {page}");
    let item_start = page.find("data-kind=\"revision\"").unwrap_or_else(|| panic!("kein Werk-Treffer: {page}"));
    let item = &page[item_start..];
    let href = item.split("href=\"").nth(1).and_then(|s| s.split('"').next()).unwrap().replace("&amp;", "&");
    assert_eq!(
        href,
        format!("/?work=1&key={}&rev={rev}&lq=Zaunkoenigsgesang#fundstelle", key_at(&server, 1, 4).await),
        "der Treffer zeigt auf Abschnitt, Fassung und Fundstelle"
    );

    let (status, workspace) = server.get(href.split('#').next().unwrap()).await;
    assert_eq!(status, StatusCode::OK, "der Link öffnet die Arbeitsseite: {workspace}");
    assert!(
        workspace.contains("<textarea name=\"body\" id=\"fundstelle\""),
        "der Anker #fundstelle hat auch ohne JavaScript ein Ziel: {workspace}"
    );
    assert!(workspace.contains("Der Zaunkoenigsgesang beginnt."), "der Editor zeigt die gefundene Fassung");
    assert!(
        workspace.contains("<input type=\"hidden\" name=\"lq\" value=\"Zaunkoenigsgesang\">"),
        "die Suche bleibt in den Formularen erhalten"
    );
}

#[tokio::test]
async fn a_highlight_becomes_a_quote_proposal_that_can_be_accepted_into_the_mirror() {
    let server = common::open().await;
    let dir = server.mirror_dir();
    let titles = server.rpc("library.list", json!(null)).await;
    let luther = titles["result"]["titles"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["key"] == "luther1912")
        .unwrap_or_else(|| panic!("Beispieltitel luther1912 fehlt: {titles}"))
        .clone();
    let added = server
        .rpc("highlight.add", json!({ "title": luther["id"], "text": "Die Liebe ist langmütig", "kind": "quote" }))
        .await;
    let highlight = added["result"]["highlight"]["id"].as_i64().unwrap_or_else(|| panic!("Hervorheben: {added}"));
    let before = current_of(&server, 4).await;

    let signs = key_of(&server, server.state.work, 4).await;
    let cited = server.rpc("highlight.cite", json!({ "highlight": highlight, "key": signs })).await;
    let rev = cited["result"]["rev"].as_u64().unwrap_or_else(|| panic!("Zitieren scheiterte: {cited}"));
    assert!(numbers(&server, 4).await.contains(&(rev, "proposed".to_owned())), "das Zitat ist ein Vorschlag");
    assert_eq!(current_of(&server, 4).await, before, "die aktuelle Fassung bleibt bis zum Übernehmen");
    let start = commit_count(&dir);

    let accepted = server.rpc("revision.set_current", json!({ "node": 4, "rev": rev })).await;
    assert!(accepted["result"]["commit"].is_string(), "Übernehmen des Zitats ist ein Commit: {accepted}");
    let Some(start) = start else { return };
    assert_eq!(commit_count(&dir), Some(start + 1), "genau ein Commit");
    assert!(last_subject(&dir).ends_with(&format!("Vorschlag {rev} übernommen")), "Betreff: {}", last_subject(&dir));
    let trailers = last_trailers(&dir);
    assert!(trailers.contains("Persona: @literatur"), "der Commit nennt die Literatur als Urheber: {trailers}");
    assert!(trailers.contains("Model: baruch-zitat"), "und das Modell: {trailers}");
    let files = common::git(&dir, &["show", "--format=", "HEAD"]).unwrap();
    assert!(files.contains("#quote(block: true, attribution: [@luther1912])"), "das Zitat steht im Spiegel: {files}");
}

#[test]
fn the_default_port_is_8765() {
    let data = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_baruch-server"))
        .arg("--help")
        .env_remove("BARUCH_LISTEN")
        .env_remove("BARUCH_DATA")
        .arg("--data")
        .arg(data.path())
        .output()
        .expect("baruch-server lässt sich nicht starten");
    let help = String::from_utf8(output.stdout).unwrap();
    assert!(output.status.success(), "--help endet mit Exit 0");
    assert!(help.contains("127.0.0.1:8765"), "Vorgabe ist 127.0.0.1:8765: {help}");
    assert!(!help.contains("8740"), "der alte Port 8740 kommt nicht mehr vor: {help}");
    assert!(include_str!("../src/main.rs").contains("127.0.0.1:8765"), "Vorgabe auch im Code");
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
