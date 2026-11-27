// SPDX-License-Identifier: AGPL-3.0-or-later
//! Hiding older revisions (user 2026-11-24, release 0.9.0): mark „bis hierhin“ per section through the form (`/act`,
//! without JavaScript), RPC and Einstellungen → Werk; delivered pages and fragments (accordion „Fassungen“, editor
//! header with counter, tree view), `ui.render` with the switch open and `/mcp` (cannot set it). Real data folder,
//! freely invented work, no model, no network.

use crate::common;

use std::net::SocketAddr;

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode, header};
use serde_json::{Value, json};

const BOOK: &str = "#set document(title: \"Der Fährmann von Lindwerder\", author: \"Tilde Marx\")\n\n\
= Überfahrt\n\nDer Fährmann stieß die Stange in den Schlick, und das Boot glitt langsam auf den Fluss hinaus.\n\n\
= Ufer\n\nAm anderen Ufer wartete niemand, nur ein Reiher stand im Schilf.\n";

fn result(answer: &Value) -> &Value {
    assert!(answer.get("error").is_none(), "RPC-Fehler: {answer}");
    &answer["result"]
}

fn error(answer: &Value) -> (i64, String) {
    let e = answer.get("error").unwrap_or_else(|| panic!("Fehler erwartet: {answer}"));
    (e["code"].as_i64().unwrap(), e["message"].as_str().unwrap().to_owned())
}

/// Page with resolved escapes in addresses (minijinja escapes `/` as `&#x2f;`, `&` as `&amp;`).
fn readable(page: &str) -> String {
    page.replace("&#x2f;", "/").replace("&amp;", "&")
}

/// Excerpt of an area of the page (from `start` to the next `end`).
fn part<'a>(page: &'a str, start: &str, end: &str) -> &'a str {
    let from = page.find(start).unwrap_or_else(|| panic!("{start} fehlt in der Seite"));
    let rest = &page[from..];
    &rest[..rest.find(end).unwrap_or(rest.len())]
}

struct Book {
    work: i64,
    key: i64,
}

/// Current text of section 0 with a word before „Boot“ (makes the revision different).
async fn variant(server: &common::Server, work: i64, word: &str) -> String {
    let loaded = server.state.store.load_work(work).await.unwrap();
    let n = &loaded.nodes[0];
    n.revisions[n.current_index().unwrap()].body.replacen("Boot", &format!("{word} Boot"), 1)
}

/// Work with section 0 in these revisions: 1 import, 2 human (both superseded), 3 proposal rejected, 4 human
/// (superseded), 5 open proposal (older than the later mark), 6 human (current), 7 open proposal (newer).
async fn setup(server: &common::Server) -> Book {
    let report = server.import(BOOK, "faehrmann.typ").await;
    let work = report.work;
    let store = &server.state.store;
    store.save_human(work, 0, &variant(server, work, "kleine").await, "Tilde").await.unwrap().unwrap();
    let three =
        store.add_proposal(work, 0, &variant(server, work, "graue").await, "@lektor", "probe", "grau").await.unwrap();
    store.reject(work, 0, three).await.unwrap();
    store.save_human(work, 0, &variant(server, work, "alte").await, "Tilde").await.unwrap().unwrap();
    store.add_proposal(work, 0, &variant(server, work, "rote").await, "@lektor", "probe", "rot").await.unwrap();
    store.save_human(work, 0, &variant(server, work, "neue").await, "Tilde").await.unwrap().unwrap();
    store.add_proposal(work, 0, &variant(server, work, "blaue").await, "@lektor", "probe", "blau").await.unwrap();
    let node = server.rpc("node.get", json!({ "work": work, "node": 0 })).await;
    let node = result(&node);
    let statuses: Vec<(u64, &str)> = node["revisions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| (r["number"].as_u64().unwrap(), r["status"].as_str().unwrap()))
        .collect();
    assert_eq!(
        statuses,
        vec![
            (1, "superseded"),
            (2, "superseded"),
            (3, "rejected"),
            (4, "superseded"),
            (5, "proposed"),
            (6, "current"),
            (7, "proposed")
        ],
        "Ausgangslage des Tests"
    );
    Book { work, key: node["key"].as_i64().unwrap() }
}

async fn page(server: &common::Server, uri: &str) -> String {
    let (status, page) = server.get(uri).await;
    assert_eq!(status, StatusCode::OK, "{uri}");
    readable(&page)
}

#[tokio::test]
async fn folding_without_javascript_hides_older_revisions_behind_the_switch_and_keeps_current_and_open_proposals() {
    let server = common::open().await;
    let Book { work, key } = setup(&server).await;

    // Before: everything visible, counter over all seven, offer at the current revision (below it there is superseded
    // material).
    let before = page(&server, &format!("/?work={work}&key={key}")).await;
    assert!(before.contains(">6/7</span>"), "ohne Marke zählt der Umschalter alle Fassungen");
    assert!(!before.contains("ausgeblendet)"), "ohne Marke kein Zusatz am Zähler");
    assert!(
        before.contains(r#"form="rev-fold-6""#)
            && before.contains(r#"<form id="rev-fold-6" method="post" action="/act""#),
        "„bis hier ausblenden“ an der aktuellen Fassung, Formular an /act (ohne JavaScript)"
    );

    // Form as without JavaScript: mark at revision 6, back to the revision.
    let (status, location) =
        server.post_form("/act", &format!("method=revision.fold_set&work={work}&key={key}&rev=6&view=edit")).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Weiterleitung nach dem Formular");
    assert_eq!(location.as_deref(), Some(format!("/?work={work}&key={key}&rev=6").as_str()), "zurück auf Fassung 6");

    let after = page(&server, &format!("/?work={work}&key={key}&rev=6")).await;
    // Counter: visible are 5 (open proposal, older than the mark), 6 (current) and 7 (proposal) → 2/3, plus the
    // addition with the four hidden ones (1, 2, 4 superseded, 3 rejected).
    assert!(after.contains(">2/3</span>"), "Zähler über die sichtbaren Fassungen");
    assert!(after.contains("(+ 4 ausgeblendet) ▸"), "Zusatz am Zähler nennt die Ausgeblendeten");
    assert!(
        after.contains(&format!(r#"href="/?work={work}&key={key}&rev=6&fassungen=alle""#)),
        "der Zusatz öffnet den Schalter per Link (ohne JavaScript)"
    );
    // Paging skips hidden ones: ‹ leads to 5, not to 4.
    assert!(after.contains(&format!(r#"data-key="prev" href="/?work={work}&key={key}&rev=5""#)), "‹ → Fassung 5");
    assert!(after.contains(&format!(r#"data-key="next" href="/?work={work}&key={key}&rev=7""#)), "› → Fassung 7");

    // Accordion: switch as <details> (closed), badge, „Wieder alle zeigen“ as a form to /act.
    let accordion = part(&after, r#"data-section="revisions""#, "</details>\n</div>");
    assert!(accordion.contains("4 ausgeblendet</span>"), "Abzeichen im Kopf des Akkordeons");
    assert!(
        accordion.contains(r#"<details class="rev-fold" data-rev-fold-list>"#),
        "Schalter zu (ohne open) – die Ausgeblendeten liegen im zugeklappten Abschnitt"
    );
    assert!(accordion.contains("4 ältere Fassungen zeigen ▸"), "vier ersetzte/verworfene in „Weitere Fassungen“");
    assert!(accordion.contains("verbergen ▾"), "Gegenstück für den offenen Schalter");
    assert!(
        accordion.contains(r#"<input type="hidden" name="method" value="revision.fold_clear">"#)
            && accordion.contains("Wieder alle zeigen"),
        "Marke aufheben per Formular"
    );
    // Tree view: the four hidden ones behind „4 ältere Fassungen zeigen“, the others before it.
    let tree = part(&after, r#"<section class="revtree""#, "</section>");
    let (shown, folded) = tree.split_once(r#"<details class="rev-fold""#).expect("Schalter in der Baumansicht");
    let folded = folded.split("</details>").next().unwrap();
    for n in [5, 6, 7] {
        assert!(shown.contains(&format!(r#"data-rev="{n}""#)), "Fassung {n} sichtbar (aktuell bzw. offener Vorschlag)");
        assert!(!folded.contains(&format!(r#"data-rev="{n}""#)), "Fassung {n} nicht hinter dem Schalter");
    }
    for n in [1, 2, 3, 4] {
        assert!(folded.contains(&format!(r#"data-rev="{n}""#)), "Fassung {n} hinter dem Schalter");
        assert!(!shown.contains(&format!(r#"data-rev="{n}""#)), "Fassung {n} nicht sichtbar");
    }
    assert!(folded.starts_with(r#" data-rev-fold-list>"#), "Baumansicht: Schalter zu");
    assert!(folded.contains("4 ältere Fassungen zeigen ▸"), "Zahl im Schalter der Baumansicht");

    // Nothing deleted: all seven revisions through RPC, status unchanged.
    let node = server.rpc("node.get", json!({ "work": work, "key": key })).await;
    assert_eq!(result(&node)["revisions"].as_array().unwrap().len(), 7, "alle Fassungen bleiben");
}

#[tokio::test]
async fn the_open_switch_counts_all_and_a_hidden_revision_opens_it_by_itself() {
    let server = common::open().await;
    let Book { work, key } = setup(&server).await;
    result(&server.rpc("revision.fold_set", json!({ "work": work, "key": key, "rev": 6 })).await);

    // Switch open through the address: counter over all, paging keeps the switch (from 6 to 5, from 5 to the hidden 4),
    // „verbergen“ leads back.
    let open = page(&server, &format!("/?work={work}&key={key}&rev=6&fassungen=alle")).await;
    assert!(open.contains(">6/7</span>"), "offener Schalter: Zähler über alle");
    assert!(
        open.contains(&format!(r#"data-key="prev" href="/?work={work}&key={key}&rev=5&fassungen=alle""#)),
        "‹ behält den offenen Schalter"
    );
    let five = page(&server, &format!("/?work={work}&key={key}&rev=5&fassungen=alle")).await;
    assert!(
        five.contains(&format!(r#"data-key="prev" href="/?work={work}&key={key}&rev=4&fassungen=alle""#)),
        "‹ führt bei offenem Schalter zur ausgeblendeten Fassung 4"
    );
    let five_closed = page(&server, &format!("/?work={work}&key={key}&rev=5")).await;
    assert!(five_closed.contains(r#"<span class="btn" aria-disabled="true">‹</span>"#), "zu: vor 5 nichts mehr");
    assert!(open.contains("4 ältere verbergen ▾"), "Gegenschalter im Editor-Kopf");
    assert!(
        open.contains(&format!(r#"href="/?work={work}&key={key}&rev=6" title="Ausgeblendete"#)),
        "verbergen = ohne Zusatz"
    );
    assert!(open.contains(r#"<details class="rev-fold" data-rev-fold-list open>"#), "Schalter im Akkordeon offen");

    // Directly onto a hidden revision: switch open, counter over all; „verbergen“ leads to the current one.
    let hidden = page(&server, &format!("/?work={work}&key={key}&rev=2")).await;
    assert!(hidden.contains(">2/7</span>"), "ausgeblendete Fassung gezeigt: Zähler über alle");
    assert!(hidden.contains(&format!(r#"href="/?work={work}&key={key}&rev=6" title="Ausgeblendete"#)), "verbergen → 6");

    // With JavaScript: ui.render takes `fassungen` and returns the address with the addition.
    let rendered = server.rpc("ui.render", json!({ "work": work, "key": key, "rev": 6, "fassungen": "alle" })).await;
    let rendered = result(&rendered);
    assert!(rendered["editor"].as_str().unwrap().contains(">6/7</span>"), "ui.render mit offenem Schalter");
    assert_eq!(rendered["url"].as_str(), Some(format!("/?work={work}&key={key}&rev=6&fassungen=alle").as_str()));
    let closed = server.rpc("ui.render", json!({ "work": work, "key": key, "rev": 6 })).await;
    assert!(result(&closed)["editor"].as_str().unwrap().contains(">2/3</span>"), "ui.render zu: 2/3");
}

#[tokio::test]
async fn a_mark_above_the_current_revision_only_reaches_below_it_and_cannot_be_set_there() {
    let server = common::open().await;
    let Book { work, key } = setup(&server).await;
    result(&server.rpc("revision.fold_set", json!({ "work": work, "key": key, "rev": 6 })).await);
    // Back to revision 4: the mark at 6 now lies above the current one and holds only up to below 4.
    result(&server.rpc("revision.set_current", json!({ "work": work, "key": key, "rev": 4 })).await);
    let fold = server.rpc("revision.fold_get", json!({ "work": work, "key": key })).await;
    let fold = result(&fold);
    assert_eq!(
        (fold["below"].as_u64(), fold["cutoff"].as_u64(), fold["hidden"].as_u64()),
        (Some(6), Some(4), Some(3)),
        "gespeichert 6, wirksam 4: ausgeblendet 1, 2 (ersetzt), 3 (verworfen); die aktuelle 4 nie: {fold}"
    );
    let shown = page(&server, &format!("/?work={work}&key={key}&rev=4")).await;
    assert!(shown.contains("(+ 3 ausgeblendet)"), "Zähler-Zusatz nach der wirksamen Grenze");
    assert!(shown.contains("gilt nur bis unter die aktuelle"), "Hinweis auf die gekappte Marke im Akkordeon");
    // At the now superseded revision 6 (above the current one) nothing is offered, and RPC rejects there.
    let six = page(&server, &format!("/?work={work}&key={key}&rev=6")).await;
    assert!(
        !six.contains(r#"form="rev-fold-6""#) && !six.contains(r#"id="rev-fold-6""#),
        "kein Angebot über der aktuellen"
    );
    let (code, message) = error(&server.rpc("revision.fold_set", json!({ "work": work, "key": key, "rev": 7 })).await);
    assert_eq!(code, -32000, "über der aktuellen Fassung abgelehnt: {message}");
    assert!(message.contains("aktuellen Fassung"), "{message}");
}

#[tokio::test]
async fn rpc_and_the_work_settings_set_and_clear_marks() {
    let server = common::open().await;
    let Book { work, key } = setup(&server).await;
    let set = server.rpc("revision.fold_set", json!({ "work": work, "key": key, "rev": "6" })).await;
    let set = result(&set);
    assert_eq!((set["below"].as_u64(), set["hidden"].as_u64(), set["rev"].as_u64()), (Some(6), Some(4), Some(6)));
    assert_eq!(set["updated_by"].as_str(), Some("Autor"), "protokolliert mit dem Menschen: {set}");
    let (code, _) = error(&server.rpc("revision.fold_set", json!({ "work": work, "key": key, "rev": 99 })).await);
    assert_eq!(code, -32602, "unbekannte Fassung wie bei den übrigen Revisionsmethoden");
    let (code, _) = error(&server.rpc("revision.fold_set", json!({ "work": work, "key": key })).await);
    assert_eq!(code, -32602, "rev fehlt");

    // Lifting through /act (without JavaScript): back to the revision shown.
    let (status, location) =
        server.post_form("/act", &format!("method=revision.fold_clear&work={work}&key={key}&rev=6&view=edit")).await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    assert_eq!(location.as_deref(), Some(format!("/?work={work}&key={key}&rev=6").as_str()));
    let fold = server.rpc("revision.fold_get", json!({ "work": work, "key": key })).await;
    assert_eq!(result(&fold)["below"], Value::Null, "Marke aufgehoben");
    assert!(page(&server, &format!("/?work={work}&key={key}")).await.contains(">6/7</span>"), "wieder alle");

    // Work-wide through the settings: day in the future → mark per section on the current revision.
    let (status, location) = server
        .post_form("/einstellungen/werk/fassungen/ausblenden", &format!("work={work}&date=2999-01-01&do=fold"))
        .await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    assert_eq!(
        location.as_deref(),
        Some(format!("/einstellungen/werk?work={work}&ok=ausgeblendet#fassungen").as_str())
    );
    let fold = server.rpc("revision.fold_get", json!({ "work": work, "key": key })).await;
    assert_eq!(
        result(&fold)["below"].as_u64(),
        Some(6),
        "letzte Fassung vor dem Tag, höchstens die aktuelle (7 ist Vorschlag)"
    );
    let settings = page(&server, &format!("/einstellungen/werk?work={work}&ok=ausgeblendet")).await;
    assert!(settings.contains("Ältere Fassungen sind ausgeblendet"), "Rückmeldung");
    assert!(
        settings.contains(r#"action="/einstellungen/werk/fassungen/ausblenden""#),
        "Formular in Einstellungen → Werk"
    );
    assert!(settings.contains("haben 2 Abschnitte eine Marke"), "Zahl der Abschnitte mit Marke");
    // Invalid day → back with an error, nothing changed.
    let (_, location) =
        server.post_form("/einstellungen/werk/fassungen/ausblenden", &format!("work={work}&date=morgen")).await;
    assert!(location.unwrap().ends_with("&ausblendenfehler=1#fassungen"), "abgelehnt");
    let (code, _) = error(&server.rpc("revision.fold_before", json!({ "work": work, "date": "24.11.2026" })).await);
    assert_eq!(code, -32602, "nur JJJJ-MM-TT");
    // Lift all.
    let (_, location) =
        server.post_form("/einstellungen/werk/fassungen/ausblenden", &format!("work={work}&do=clear")).await;
    assert!(location.unwrap().ends_with("&ok=eingeblendet#fassungen"));
    let all = server.rpc("revision.fold_clear_all", json!({ "work": work })).await;
    assert_eq!(result(&all)["cleared"].as_u64(), Some(0), "schon alle aufgehoben");
}

/// MCP request from the own machine (without token only loopback).
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

#[tokio::test]
async fn mcp_cannot_set_or_clear_a_mark() {
    let server = common::open().await;
    let Book { work, key } = setup(&server).await;
    let tools = mcp(&server, "tools/list", json!({})).await;
    let names: Vec<&str> =
        tools["result"]["tools"].as_array().unwrap().iter().filter_map(|t| t["name"].as_str()).collect();
    assert!(!names.is_empty(), "Werkzeugliste geladen");
    assert!(!names.iter().any(|n| n.contains("fold")), "kein Werkzeug für die Marke: {names:?}");
    for name in ["revision_fold_set", "revision.fold_set", "fold_set"] {
        let answer =
            mcp(&server, "tools/call", json!({ "name": name, "arguments": { "work": work, "key": key, "rev": 6 } }))
                .await;
        let failed = answer.get("error").is_some() || answer["result"]["isError"] == json!(true);
        assert!(failed, "{name} über MCP abgelehnt: {answer}");
    }
    let fold = server.rpc("revision.fold_get", json!({ "work": work, "key": key })).await;
    assert_eq!(result(&fold)["below"], Value::Null, "über MCP nichts gesetzt");
}
