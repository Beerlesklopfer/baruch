// SPDX-License-Identifier: AGPL-3.0-or-later
//! Round 23 „Härtung“ (ownership mcp-namen): handle rules and ownership of the MCP clients, quota and upper limits,
//! output fields of `ask_persona`, `index_propose` with `node`, `StoreError::Stale` for outdated requests.
//!
//! Works freely invented (a small „Mühlenbuch“); no Eliezer, no network: models only through the mock
//! `assist::mock::ScriptedModel`, which records every call. Every assertion with a reason. The tests on chat and
//! repeating lie in `tests/round23_mcp_namen/chat.rs` (included with `#[path]`, `tests/main.rs` stays unchanged).

use crate::common;

use std::net::SocketAddr;
use std::sync::Arc;

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode, header};
use baruch_server::assist::mock::{ScriptedModel, Step};
use baruch_server::assist::port::ModelCall;
use baruch_store::{FindingKind, NewFinding};
use serde_json::{Value, json};

#[path = "round23_mcp_namen/chat.rs"]
mod chat;

pub(crate) const MILL: &str = "#set document(title: \"Das Mühlenbuch\", author: \"Greta Holm\")\n\n\
= Das Wehr\n\nDas Wasser staut sich vor dem alten Wehr.\n\n\
== Der Mahlgang\n\nDer Stein dreht sich langsam und zählt die Körner.\n\n\
== Der Speicher\n\nIm Speicher liegt das Korn in Säcken.\n";

pub(crate) const FOREIGN: ([u8; 4], u16) = ([192, 0, 2, 7], 50000);
pub(crate) const LOOPBACK: ([u8; 4], u16) = ([127, 0, 0, 1], 40000);

pub(crate) struct Mill {
    pub server: common::Server,
    pub model: ScriptedModel,
    pub work: i64,
    pub slug: String,
    /// Keys of Wehr, Mahlgang, Speicher.
    pub keys: Vec<i64>,
}

pub(crate) async fn mill(steps: impl IntoIterator<Item = Step>) -> Mill {
    let model = ScriptedModel::with(steps);
    let data = tempfile::tempdir().unwrap();
    let state = baruch_server::AppState::open_with(
        data.path(),
        Arc::new(baruch_server::connections::MemorySecrets::default()),
        Arc::new(model.clone()),
    )
    .await
    .expect("Datenordner lässt sich nicht öffnen");
    let state = Arc::new(state);
    let server = common::Server { app: baruch_server::router(state.clone()), state, data };
    let report = server.import(MILL, "muehle.typ").await;
    let keys = server.state.store.load_work(report.work).await.unwrap().nodes.iter().map(|n| n.key).collect();
    Mill { server, model, work: report.work, slug: report.slug, keys }
}

/// Who makes an MCP request: peer, token, session.
#[derive(Clone, Default)]
pub(crate) struct Caller {
    pub foreign: bool,
    pub token: Option<String>,
    pub session: Option<String>,
}

/// An MCP message through the router (`oneshot`), peer as `ConnectInfo` as in the real service.
pub(crate) async fn mcp(m: &Mill, caller: &Caller, message: Value) -> (StatusCode, axum::http::HeaderMap, Value) {
    let mut request = Request::post("/mcp").header(header::CONTENT_TYPE, "application/json");
    if let Some(token) = &caller.token {
        request = request.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    if let Some(id) = &caller.session {
        request = request.header("mcp-session-id", id);
    }
    let mut request = request.body(Body::from(message.to_string())).unwrap();
    let peer = if caller.foreign { FOREIGN } else { LOOPBACK };
    request.extensions_mut().insert(ConnectInfo(SocketAddr::from(peer)));
    let (status, headers, body) = m.server.send(request).await;
    (status, headers, serde_json::from_str(&body).unwrap_or(Value::String(body)))
}

fn initialize(name: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize",
            "params": { "protocolVersion": "2025-06-18", "capabilities": {},
                        "clientInfo": { "name": name, "version": "1.0" } } })
}

/// Client without token from the own machine with the name `name`.
pub(crate) async fn local(m: &Mill, name: &str) -> Caller {
    let mut caller = Caller::default();
    let (status, headers, answer) = mcp(m, &caller, initialize(name)).await;
    assert_eq!(status, StatusCode::OK, "initialize: {answer}");
    caller.session = Some(headers.get("mcp-session-id").unwrap().to_str().unwrap().to_owned());
    caller
}

/// Client with token from another machine.
pub(crate) async fn remote(m: &Mill, token: &str) -> Caller {
    let mut caller = Caller { foreign: true, token: Some(token.to_owned()), session: None };
    let (status, headers, answer) = mcp(m, &caller, initialize("irgendein-name")).await;
    assert_eq!(status, StatusCode::OK, "initialize mit Token: {answer}");
    caller.session = Some(headers.get("mcp-session-id").unwrap().to_str().unwrap().to_owned());
    caller
}

/// `tools/call`: the whole JSON-RPC response (with `result` or `error`).
pub(crate) async fn call(m: &Mill, caller: &Caller, name: &str, arguments: Value) -> Value {
    let message = json!({ "jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": { "name": name, "arguments": arguments } });
    let (status, _, answer) = mcp(m, caller, message).await;
    assert_eq!(status, StatusCode::OK, "MCP antwortet: {answer}");
    answer
}

/// Successful tool call: `structuredContent`.
pub(crate) async fn ok(m: &Mill, caller: &Caller, name: &str, arguments: Value) -> Value {
    let answer = call(m, caller, name, arguments).await;
    assert_eq!(answer["result"]["isError"], false, "{name} gelingt: {answer}");
    answer["result"]["structuredContent"].clone()
}

/// Rejected tool call (`isError`): the text of the message.
pub(crate) async fn refused(m: &Mill, caller: &Caller, name: &str, arguments: Value) -> String {
    let answer = call(m, caller, name, arguments).await;
    assert_eq!(answer["result"]["isError"], true, "{name} wird als Werkzeugfehler abgelehnt: {answer}");
    answer["result"]["content"][0]["text"].as_str().unwrap().to_owned()
}

pub(crate) fn result(answer: &Value) -> &Value {
    assert!(answer.get("error").is_none(), "unerwarteter Fehler: {answer}");
    &answer["result"]
}

pub(crate) fn error_code(answer: &Value) -> i64 {
    answer["error"]["code"].as_i64().unwrap_or_else(|| panic!("Fehler erwartet: {answer}"))
}

pub(crate) fn all_text(call: &ModelCall) -> String {
    call.messages.iter().map(|m| m.content.as_str()).collect::<Vec<_>>().join("\n")
}

pub(crate) async fn posts(m: &Mill, key: usize) -> Vec<baruch_core::Post> {
    let loaded = m.server.state.store.load_work(m.work).await.unwrap();
    loaded.nodes.iter().find(|n| n.key == m.keys[key]).unwrap().posts.clone()
}

pub(crate) async fn add_post(m: &Mill, key: usize, author: &str, body: &str) -> i64 {
    let seq = m.server.state.store.node_seq(m.work, m.keys[key]).await.unwrap();
    let post = baruch_core::Post {
        id: 0,
        created: String::new(),
        author: author.into(),
        body: body.into(),
        sources: vec![],
        refs: vec![],
    };
    m.server.state.store.add_post(m.work, seq, &post).await.unwrap()
}

pub(crate) async fn finding(m: &Mill, key: usize, post: Option<i64>, position: u32, persona: &str, body: &str) -> i64 {
    let new = NewFinding {
        work: m.work,
        key: Some(m.keys[key]),
        post,
        thread_post: None,
        position,
        persona,
        kind: FindingKind::Critical,
        body,
        quote: "",
        quote_check: "",
        similar_to: None,
        summary_of: &[],
    };
    m.server.state.store.finding_insert(&new).await.unwrap().id
}

pub(crate) fn commits(m: &Mill) -> usize {
    common::git(&m.server.mirror_dir_of(&m.slug), &["rev-list", "--count", "HEAD"])
        .map_or(0, |c| c.trim().parse().unwrap())
}

pub(crate) async fn create_token(m: &Mill, name: &str) -> Value {
    m.server.rpc("mcp_token.create", json!({ "name": name })).await
}

async fn token_names(m: &Mill) -> Vec<String> {
    m.server.state.store.mcp_token_list().await.unwrap().into_iter().map(|t| t.name).collect()
}

// ---- Tokens: unique in the handle ---------------------------------------------------------------------------------

#[tokio::test]
async fn a_token_whose_handle_is_taken_reserved_or_revoked_is_refused_over_rpc_and_form() {
    let m = mill([]).await;
    let first = create_token(&m, "Laptop Küche").await;
    let id = result(&first)["token"]["id"].as_i64().expect("der erste Token entsteht");
    for (name, why) in [
        ("laptop kueche", "gleicher Handle laptop-kueche (Umlaut umschrieben, Groß/klein, Leerzeichen)"),
        ("LAPTOP-KUECHE", "Groß/klein zählt nicht"),
        ("Laptop-K-che", "sein Handle gleicht dem Alt-Handle des vorhandenen Tokens (laptop-k-che)"),
        ("lokal-Drucker", "die Vorsilbe lokal- ist Clients ohne Token vorbehalten"),
        ("  Lokal-Drucker ", "auch groß und mit Rand"),
    ] {
        let answer = create_token(&m, name).await;
        assert_eq!(error_code(&answer), -32000, "{name:?} wird abgelehnt – {why}: {answer}");
        assert!(answer.to_string().find("baruch_").is_none(), "eine Ablehnung trägt keinen Klartext: {answer}");
    }
    let reserved = create_token(&m, "lokal-Drucker").await;
    assert!(
        reserved["error"]["message"].as_str().unwrap().contains("lokal-"),
        "die Meldung nennt die Vorsilbe: {reserved}"
    );
    assert_eq!(token_names(&m).await, ["Laptop Küche"], "nach fünf Ablehnungen gibt es weiter genau einen Token");

    // Form: the same way, 422 with message, no token.
    let request = Request::post("/einstellungen/mcp/act")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from("action=create&name=laptop+kueche"))
        .unwrap();
    let (status, _, html) = m.server.send(request).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "das Formular lehnt wie RPC ab: {html}");
    assert!(html.contains("denselben Autor"), "mit der ehrlichen Meldung: {html}");
    assert!(!html.contains("nur jetzt sichtbar"), "kein neuer Token auf der Seite");
    assert_eq!(token_names(&m).await.len(), 1, "auch das Formular hat nichts angelegt");

    // Revoking does not free the handle: the posts of the old token keep carrying it.
    result(&m.server.rpc("mcp_token.revoke", json!({ "id": id, "confirm": true })).await);
    let again = create_token(&m, "Laptop Küche").await;
    assert_eq!(error_code(&again), -32000, "der Name eines widerrufenen Tokens ist nicht wieder frei: {again}");
    assert!(
        again["error"]["message"].as_str().unwrap().contains("widerrufener"),
        "die Meldung nennt den Grund: {again}"
    );

    // Everything after 32 characters does not count; a really different name works.
    let long = "x".repeat(32);
    result(&create_token(&m, &format!("{long}-eins")).await);
    let cut = create_token(&m, &format!("{long}-zwei")).await;
    assert_eq!(error_code(&cut), -32000, "nach 32 Zeichen fallen beide Namen zusammen: {cut}");
    result(&create_token(&m, "Büro").await);
    assert_eq!(token_names(&m).await.len(), 3, "widerrufener, langer und neuer Token");
    let (_, page) = m.server.get("/einstellungen/mcp").await;
    assert!(page.contains("<code>@mcp:buero</code>"), "die Seite zeigt den Autor nach der neuen Regel: {page}");
    assert!(page.contains("früher <code>@mcp:b-ro</code>"), "und den Alt-Handle, wo er abweicht");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_requests_with_the_same_handle_create_exactly_one_token() {
    let m = mill([]).await;
    let names =
        ["Werkstatt Süd", "werkstatt sued", "WERKSTATT-SUED", "Werkstatt_Süd ", "werkstatt süd", "Werkstatt Sued"];
    let mut tasks = Vec::new();
    for name in names {
        // „Werkstatt_Süd“ has a handle of its own (the underscore stays); the other five coincide.
        let app = m.server.app.clone();
        tasks.push(tokio::spawn(async move { common::rpc(app, "mcp_token.create", json!({ "name": name })).await }));
    }
    let mut made = Vec::new();
    let mut refused = 0;
    for task in tasks {
        let answer = task.await.unwrap();
        match answer["result"]["token"]["name"].as_str() {
            Some(name) => made.push(name.to_owned()),
            None => {
                assert_eq!(error_code(&answer), -32000, "der Verlierer bekommt die ehrliche Ablehnung: {answer}");
                refused += 1;
            }
        }
    }
    let same: Vec<&String> = made.iter().filter(|n| n.trim() != "Werkstatt_Süd").collect();
    assert_eq!(same.len(), 1, "von fünf gleichzeitigen Anfragen mit demselben Handle gewinnt genau eine: {made:?}");
    assert_eq!(refused, 4, "die vier anderen sind abgelehnt");
    let handles: Vec<String> =
        token_names(&m).await.iter().map(|n| baruch_server::mcp::session::handle_name(n)).collect();
    assert_eq!(
        handles.iter().filter(|h| *h == "werkstatt-sued").count(),
        1,
        "in der Datenbank steht der Handle genau einmal (Prüfung und Anlegen in einer Transaktion): {handles:?}"
    );
}

// ---- Ownership: legacy handle of the token, nobody else obtains it --------------------------------------------------

#[tokio::test]
async fn an_old_token_with_an_umlaut_keeps_its_old_posts_and_nobody_else_gains_them() {
    let m = mill([]).await;
    // Stock before Round 23: token created without handle check, post under the old handle (ü → '-').
    let (_, secret) = m.server.state.store.mcp_token_create("Laptop Küche").await.unwrap();
    let (_, other_secret) = m.server.state.store.mcp_token_create("Zweitgerät").await.unwrap();
    let old = add_post(&m, 0, "@mcp:laptop-k-che", "Alter Beitrag vom Laptop.").await;

    let owner = remote(&m, &secret).await;
    let fresh = ok(&m, &owner, "add_post", json!({ "work": m.work, "key": m.keys[0], "body": "Neuer Beitrag." })).await;
    assert_eq!(fresh["author"], "@mcp:laptop-kueche", "neue Beiträge tragen den Handle der neuen Regel: {fresh}");

    // Nobody else: another token, a client without token with the legacy handle or with the name of the token.
    let other = remote(&m, &other_secret).await;
    let text = refused(&m, &other, "post_edit", json!({ "work": m.work, "id": old, "body": "gekapert" })).await;
    assert!(text.contains("Nur eigene"), "ein anderer Token besitzt den Beitrag nicht: {text}");
    for name in ["laptop-k-che", "Laptop Küche", "laptop kueche"] {
        let intruder = local(&m, name).await;
        let mine =
            ok(&m, &intruder, "add_post", json!({ "work": m.work, "key": m.keys[1], "body": "ohne Token" })).await;
        let author = mine["author"].as_str().unwrap();
        assert!(author.starts_with("@mcp:lokal-"), "{name:?} ohne Token schreibt mit der Vorsilbe lokal-: {author}");
        for (tool, extra) in [
            ("post_edit", json!({ "body": "gekapert" })),
            ("post_delete", json!({})),
            ("post_move", json!({ "key": m.keys[2] })),
            ("post_to_idea", json!({ "words": "mühle" })),
        ] {
            let mut arguments = json!({ "work": m.work, "id": old });
            arguments.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
            let text = refused(&m, &intruder, tool, arguments).await;
            assert!(text.contains("eigene"), "{name:?} ohne Token: {tool} am alten Beitrag abgelehnt: {text}");
        }
    }
    let untouched = m.server.state.store.post_get(m.work, old).await.unwrap();
    assert_eq!(
        (untouched.body.as_str(), untouched.key, untouched.author.as_str()),
        ("Alter Beitrag vom Laptop.", m.keys[0], "@mcp:laptop-k-che"),
        "der alte Beitrag ist unverändert, und sein Autor wurde nicht umgeschrieben"
    );
    let stolen = create_token(&m, "laptop-k-che").await;
    assert_eq!(error_code(&stolen), -32000, "auch ein neuer Token bekommt den Alt-Handle nicht: {stolen}");

    // The token itself: editing, moving, deleting – through the legacy handle.
    let edited =
        ok(&m, &owner, "post_edit", json!({ "work": m.work, "id": old, "body": "Vom Laptop, bearbeitet." })).await;
    assert_eq!(edited["versions"].as_array().unwrap().len(), 1, "die alte Fassung bleibt als Verlauf: {edited}");
    let moved = ok(&m, &owner, "post_move", json!({ "work": m.work, "id": old, "key": m.keys[2] })).await;
    assert_eq!(moved["key"], m.keys[2], "verschoben in den Speicher: {moved}");
    ok(&m, &owner, "post_delete", json!({ "work": m.work, "id": old })).await;
    assert!(m.server.state.store.post_get(m.work, old).await.is_err(), "gelöscht vom eigenen Token");
}

// ---- Stock: two tokens that only the rule since Round 23 merged -----------------------------------------------------

#[tokio::test]
async fn two_old_tokens_that_only_the_new_rule_would_merge_stay_apart() {
    let m = mill([]).await;
    // Stock before Round 23 (created without handle check): „Büro“ wrote as @mcp:b-ro, „buero“ as @mcp:buero.
    let (_, umlaut_secret) = m.server.state.store.mcp_token_create("Büro").await.unwrap();
    let (_, plain_secret) = m.server.state.store.mcp_token_create("buero").await.unwrap();
    let of_umlaut = add_post(&m, 0, "@mcp:b-ro", "Alter Beitrag aus dem Büro.").await;
    let of_plain = add_post(&m, 0, "@mcp:buero", "Alter Beitrag vom zweiten Rechner.").await;
    let umlaut = remote(&m, &umlaut_secret).await;
    let plain = remote(&m, &plain_secret).await;

    let fresh =
        ok(&m, &umlaut, "add_post", json!({ "work": m.work, "key": m.keys[1], "body": "Neu aus dem Büro." })).await;
    assert_eq!(
        fresh["author"], "@mcp:b-ro",
        "„Büro“ schreibt weiter unter seinem Autor von vor Runde 23 – @mcp:buero gehört dem Token „buero“: {fresh}"
    );
    let other =
        ok(&m, &plain, "add_post", json!({ "work": m.work, "key": m.keys[1], "body": "Neu vom zweiten." })).await;
    assert_eq!(other["author"], "@mcp:buero", "„buero“ behält seinen Autor: {other}");

    // Neither edits, moves, deletes or converts the posts of the other.
    for (who, caller, foreign_post) in [("Büro", &umlaut, of_plain), ("buero", &plain, of_umlaut)] {
        for (tool, extra) in [
            ("post_edit", json!({ "body": "gekapert" })),
            ("post_delete", json!({})),
            ("post_move", json!({ "key": m.keys[2] })),
            ("post_to_idea", json!({ "words": "mühle" })),
        ] {
            let mut arguments = json!({ "work": m.work, "id": foreign_post });
            arguments.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
            let text = refused(&m, caller, tool, arguments).await;
            assert!(text.contains("eigene"), "Token „{who}“: {tool} am Beitrag des anderen Tokens abgelehnt: {text}");
        }
    }
    for (id, body, author) in [
        (of_umlaut, "Alter Beitrag aus dem Büro.", "@mcp:b-ro"),
        (of_plain, "Alter Beitrag vom zweiten Rechner.", "@mcp:buero"),
    ] {
        let post = m.server.state.store.post_get(m.work, id).await.unwrap();
        assert_eq!(
            (post.body.as_str(), post.key, post.author.as_str()),
            (body, m.keys[0], author),
            "der Beitrag ist nach den abgelehnten Versuchen unverändert"
        );
    }

    // Each keeps managing its own old posts.
    let edited =
        ok(&m, &umlaut, "post_edit", json!({ "work": m.work, "id": of_umlaut, "body": "Büro, bearbeitet." })).await;
    assert_eq!(edited["versions"].as_array().unwrap().len(), 1, "„Büro“ bearbeitet seinen alten Beitrag: {edited}");
    ok(&m, &plain, "post_delete", json!({ "work": m.work, "id": of_plain })).await;
    assert!(m.server.state.store.post_get(m.work, of_plain).await.is_err(), "„buero“ löscht seinen alten Beitrag");

    // A client without token gets neither the one nor the other author.
    for name in ["Büro", "buero", "b-ro"] {
        let intruder = local(&m, name).await;
        let mine =
            ok(&m, &intruder, "add_post", json!({ "work": m.work, "key": m.keys[2], "body": "ohne Token" })).await;
        let author = mine["author"].as_str().unwrap();
        assert!(author.starts_with("@mcp:lokal-"), "{name:?} ohne Token bleibt bei der Vorsilbe lokal-: {author}");
    }

    // The settings page names the author under which the token really writes, and the reason.
    let (_, page) = m.server.get("/einstellungen/mcp").await;
    let row = page.split("<tr>").find(|r| r.starts_with("<td>Büro</td>")).expect("Zeile des Tokens „Büro“");
    assert!(row.contains("<code>@mcp:b-ro</code>"), "Autor des Tokens „Büro“ ist der alte: {row}");
    assert!(
        row.contains("<code>@mcp:buero</code> gehört einem anderen Token"),
        "die Seite sagt, warum er nicht nach der neuen Regel heißt: {row}"
    );
    let row = page.split("<tr>").find(|r| r.starts_with("<td>buero</td>")).expect("Zeile des Tokens „buero“");
    assert!(row.contains("<code>@mcp:buero</code>") && !row.contains("gehört einem anderen"), "ohne Hinweis: {row}");
}

// ---- Clients without token: changing names open no new quota --------------------------------------------------------

#[tokio::test]
async fn clients_without_token_share_the_model_slots_whatever_they_call_themselves() {
    use baruch_server::mcp::tools::quota;
    let m = mill([Step::Hang, Step::Hang, Step::Hang, Step::Hang, Step::words("Hinweis: vom Token gefragt.")]).await;
    let (_, secret) = m.server.state.store.mcp_token_create("Werkbank").await.unwrap();
    let mut running = Vec::new();
    for n in 0..quota::MAX_WITHOUT_TOKEN {
        // Every job under a new name: per name only one of two places would be taken.
        let caller = local(&m, &format!("Wechsler {n}")).await;
        let app = m.server.app.clone();
        let (work, key) = (m.work, m.keys[0]);
        running.push(tokio::spawn(async move {
            let message = json!({ "jsonrpc": "2.0", "id": 7, "method": "tools/call", "params": { "name": "ask_persona",
                "arguments": { "work": work, "key": key, "persona": "@lektor", "question": "?" } } });
            let mut request = Request::post("/mcp")
                .header(header::CONTENT_TYPE, "application/json")
                .header("mcp-session-id", caller.session.unwrap())
                .body(Body::from(message.to_string()))
                .unwrap();
            request.extensions_mut().insert(ConnectInfo(SocketAddr::from(LOOPBACK)));
            let _ = tower::ServiceExt::oneshot(app, request).await;
        }));
    }
    let mut waited = 0;
    while quota::in_flight(&m.server.state, quota::WITHOUT_TOKEN) < quota::MAX_WITHOUT_TOKEN
        || m.model.calls().len() < quota::MAX_WITHOUT_TOKEN
    {
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        waited += 1;
        assert!(waited < 1000, "die vier Aufträge der Clients ohne Token laufen (hängen am Modell)");
    }
    assert_eq!(quota::in_flight(&m.server.state, "@mcp:wechsler-0"), 1, "je Name läuft nur ein Auftrag");

    let next = local(&m, "Noch ein Name").await;
    let arguments = json!({ "work": m.work, "key": m.keys[0], "persona": "@lektor", "question": "?" });
    let text = refused(&m, &next, "ask_persona", arguments.clone()).await;
    assert!(
        text.contains("without a token") && text.contains("nothing was stored"),
        "ein neuer Name bekommt keine frischen Plätze – die Clients ohne Token teilen sich vier: {text}"
    );
    assert_eq!(m.model.calls().len(), quota::MAX_WITHOUT_TOKEN, "für den abgelehnten Auftrag geht nichts ans Modell");
    assert_eq!(quota::in_flight(&m.server.state, "@mcp:noch-ein-name"), 0, "der abgelehnte Auftrag belegt nichts");

    let with_token = remote(&m, &secret).await;
    let answer = ok(&m, &with_token, "ask_persona", arguments).await;
    assert_eq!(answer["persona"], "@lektor", "ein Client mit Token ist davon nicht betroffen: {answer}");
    assert!(answer["text"].as_str().unwrap().contains("vom Token gefragt"), "er bekommt seine Antwort: {answer}");

    for task in &running {
        task.abort();
    }
    for task in running {
        let _ = task.await;
    }
    let mut waited = 0;
    while quota::in_flight(&m.server.state, quota::WITHOUT_TOKEN) > 0 {
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        waited += 1;
        assert!(waited < 1000, "abgebrochene Aufträge geben auch den gemeinsamen Platz frei");
    }
}

// ---- Client without token with the name of a token: real server on 127.0.0.1:0 --------------------------------------

/// Own server on `127.0.0.1:0` (real connections, `ConnectInfo` as in the service); returns the address.
async fn listen(m: &Mill) -> (String, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = format!("http://{}", listener.local_addr().unwrap());
    let app = m.server.app.clone();
    let task = tokio::spawn(async move {
        axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>()).await.unwrap();
    });
    (address, task)
}

struct Wire {
    http: reqwest::Client,
    address: String,
    token: Option<String>,
    session: Option<String>,
}

impl Wire {
    async fn send(&self, message: Value) -> (reqwest::header::HeaderMap, Value) {
        let mut request = self.http.post(format!("{}/mcp", self.address)).json(&message);
        if let Some(token) = &self.token {
            request = request.bearer_auth(token);
        }
        if let Some(id) = &self.session {
            request = request.header("mcp-session-id", id);
        }
        let response = request.send().await.expect("der eigene Server antwortet");
        assert_eq!(response.status(), reqwest::StatusCode::OK, "MCP über die echte Verbindung");
        let headers = response.headers().clone();
        (headers, response.json().await.unwrap())
    }

    async fn open(address: &str, name: &str, token: Option<&str>) -> Wire {
        let http = reqwest::Client::builder().no_proxy().build().unwrap();
        let mut wire = Wire { http, address: address.to_owned(), token: token.map(str::to_owned), session: None };
        let (headers, answer) = wire.send(initialize(name)).await;
        assert!(answer["result"].is_object(), "initialize: {answer}");
        wire.session = Some(headers.get("mcp-session-id").unwrap().to_str().unwrap().to_owned());
        wire
    }

    async fn tool(&self, name: &str, arguments: Value) -> Value {
        let message = json!({ "jsonrpc": "2.0", "id": 3, "method": "tools/call",
                              "params": { "name": name, "arguments": arguments } });
        self.send(message).await.1["result"].clone()
    }
}

#[tokio::test]
async fn a_client_without_token_named_like_a_token_writes_as_lokal_and_owns_nothing_of_the_token() {
    let m = mill([]).await;
    let secret = result(&create_token(&m, "Schreibtisch").await)["secret"].as_str().unwrap().to_owned();
    let revoked = result(&create_token(&m, "Altgerät").await)["token"]["id"].as_i64().unwrap();
    result(&m.server.rpc("mcp_token.revoke", json!({ "id": revoked, "confirm": true })).await);
    let (address, task) = listen(&m).await;

    let owner = Wire::open(&address, "wird-ersetzt", Some(&secret)).await;
    let post = owner.tool("add_post", json!({ "work": m.work, "key": m.keys[0], "body": "Vom Schreibtisch." })).await;
    assert_eq!(post["structuredContent"]["author"], "@mcp:schreibtisch", "mit Token heißt der Client wie der Token");
    let id = post["structuredContent"]["id"].as_i64().unwrap();

    let twin = Wire::open(&address, "Schreibtisch", None).await;
    let mine = twin.tool("add_post", json!({ "work": m.work, "key": m.keys[0], "body": "Vom Zwilling." })).await;
    assert_eq!(
        mine["structuredContent"]["author"], "@mcp:lokal-schreibtisch",
        "ohne Token mit dem Namen eines Tokens: Vorsilbe lokal- ({mine})"
    );
    let own = mine["structuredContent"]["id"].as_i64().unwrap();
    for (tool, extra) in [
        ("post_edit", json!({ "body": "gekapert" })),
        ("post_delete", json!({})),
        ("post_move", json!({ "key": m.keys[1] })),
        ("post_to_idea", json!({ "words": "tisch" })),
    ] {
        let mut arguments = json!({ "work": m.work, "id": id });
        arguments.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
        let answer = twin.tool(tool, arguments).await;
        assert_eq!(answer["isError"], true, "{tool} am Beitrag des Token-Clients wird abgelehnt: {answer}");
    }
    let kept = m.server.state.store.post_get(m.work, id).await.unwrap();
    assert_eq!((kept.body.as_str(), kept.key), ("Vom Schreibtisch.", m.keys[0]), "der Beitrag ist unberührt");
    let edited = twin.tool("post_edit", json!({ "work": m.work, "id": own, "body": "Vom Zwilling, besser." })).await;
    assert_eq!(edited["isError"], false, "seinen eigenen Beitrag darf er bearbeiten: {edited}");

    // The name of a revoked token stays protected too; a free name stays as it is.
    let ghost = Wire::open(&address, "altgeraet", None).await;
    let post = ghost.tool("add_post", json!({ "work": m.work, "key": m.keys[1], "body": "Vom Geist." })).await;
    assert_eq!(post["structuredContent"]["author"], "@mcp:lokal-altgeraet", "widerrufen zählt wie vorhanden: {post}");
    let free = Wire::open(&address, "Notizbuch", None).await;
    let post = free.tool("add_post", json!({ "work": m.work, "key": m.keys[1], "body": "Frei." })).await;
    assert_eq!(post["structuredContent"]["author"], "@mcp:notizbuch", "ohne gleichnamigen Token keine Vorsilbe");

    // "On every request": if the token is created only later, the prefix holds for the running session at once.
    result(&create_token(&m, "Notizbuch").await);
    let post = free.tool("add_post", json!({ "work": m.work, "key": m.keys[1], "body": "Danach." })).await;
    assert_eq!(post["structuredContent"]["author"], "@mcp:lokal-notizbuch", "die Sitzung wird neu abgegrenzt: {post}");
    task.abort();
}

// ---- Image quota ----------------------------------------------------------------------------------------------------

fn svg(n: usize) -> String {
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"4\" height=\"4\"><rect width=\"{}\" height=\"4\"/></svg>",
        n % 4 + 1
    ) + &format!("<!-- {n} -->")
}

#[tokio::test]
async fn clients_without_token_share_one_asset_quota_and_a_token_has_its_own() {
    use baruch_server::mcp::assets::MAX_MCP_ASSET_WRITES_PER_DAY;
    let m = mill([]).await;
    let secret = result(&create_token(&m, "Atelier").await)["secret"].as_str().unwrap().to_owned();
    let (first, second) = (local(&m, "Maler Eins").await, local(&m, "Maler Zwei").await);
    for n in 0..MAX_MCP_ASSET_WRITES_PER_DAY {
        // Alternating two names, every third image additionally as a persona: all the same quota.
        let caller = if n % 2 == 0 { &first } else { &second };
        let mut arguments =
            json!({ "work": m.work, "file_name": format!("m{n}.svg"), "content": svg(n), "encoding": "svg" });
        if n % 3 == 0 {
            arguments["persona"] = json!("@lektor");
        }
        ok(&m, caller, "asset_add", arguments).await;
    }
    let before = m.server.state.store.asset_list(m.work).await.unwrap().len();
    let third = local(&m, "Maler Drei").await;
    let text = refused(
        &m,
        &third,
        "asset_add",
        json!({ "work": m.work, "file_name": "zuviel.svg", "content": svg(900), "encoding": "svg" }),
    )
    .await;
    assert!(
        text.contains("Quota") && text.contains("without a token"),
        "ein neuer Name öffnet kein neues Kontingent, und die Meldung sagt, wessen es ist: {text}"
    );
    let text = refused(
        &m,
        &third,
        "asset_add",
        json!({ "work": m.work, "file_name": "persona.svg", "content": svg(901), "encoding": "svg", "persona": "@exeget" }),
    )
    .await;
    assert!(text.contains("Quota"), "auch `persona` öffnet keines: {text}");
    assert_eq!(m.server.state.store.asset_list(m.work).await.unwrap().len(), before, "nichts gespeichert");

    let owner = remote(&m, &secret).await;
    let added = ok(
        &m,
        &owner,
        "asset_add",
        json!({ "work": m.work, "file_name": "atelier.svg", "content": svg(902), "encoding": "svg" }),
    )
    .await;
    assert_eq!(added["name"], "atelier.svg", "der Token-Client hat sein eigenes Kontingent: {added}");
    let shared =
        m.server.state.store.asset_mcp_usage_without_token(m.work, &["@mcp:atelier".to_owned()]).await.unwrap();
    assert_eq!(shared.writes, MAX_MCP_ASSET_WRITES_PER_DAY, "das Bild des Tokens zählt nicht zur Gruppe ohne Token");
    let own = m.server.state.store.asset_mcp_usage(m.work, "@mcp:atelier").await.unwrap();
    assert_eq!(own.writes, 1, "und der Token zählt nur sein eigenes");
}

// ---- index_propose, title rule, limits ------------------------------------------------------------------------------

#[tokio::test]
async fn index_propose_refuses_node_as_an_argument_error_and_stops_at_its_limits() {
    let m = mill([]).await;
    let client = local(&m, "Register Hilfe").await;
    let with_node =
        call(&m, &client, "index_propose", json!({ "work": m.work, "node": 0, "key": m.keys[0], "word": "Wehr" }))
            .await;
    assert_eq!(error_code(&with_node), -32602, "node → Argumentfehler wie bei propose_revision: {with_node}");
    let reference =
        call(&m, &client, "propose_revision", json!({ "work": m.work, "node": 0, "body": "= Das Wehr\n\nNeu." })).await;
    assert_eq!(error_code(&reference), error_code(&with_node), "beide Werkzeuge melden node gleich");
    let without = call(&m, &client, "index_propose", json!({ "work": m.work, "word": "Wehr" })).await;
    assert_eq!(error_code(&without), -32602, "auch ein fehlender key ist ein Argumentfehler: {without}");

    let made = ok(&m, &client, "index_propose", json!({ "work": m.work, "key": m.keys[0], "word": "Wehr" })).await;
    assert!(made.to_string().contains("Wehr"), "ein Vorschlag mit key gelingt: {made}");
    let pool = m.server.state.store.pool();
    let count = || async {
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM index_proposal WHERE work_id = ? AND status = 'open'")
            .bind(m.work)
            .fetch_one(pool)
            .await
            .unwrap()
    };
    // Fill up to the limit per origin (200): copies of the real proposal.
    let fill = "WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM n WHERE i < ?) \
                INSERT INTO index_proposal (work_id, node_id, based_on, word, occurrence, mark, reason, origin, status) \
                SELECT p.work_id, p.node_id, p.based_on, p.word, p.occurrence, p.mark, p.reason, ?, 'open' \
                FROM n, (SELECT * FROM index_proposal WHERE work_id = ? ORDER BY id LIMIT 1) p";
    sqlx::query(fill).bind(199).bind("@mcp:register-hilfe").bind(m.work).execute(pool).await.unwrap();
    assert_eq!(count().await, 200, "200 offene Vorschläge dieser Herkunft (MAX_OPEN_INDEX_PROPOSALS_PER_ORIGIN)");
    let before = commits(&m);
    let text =
        refused(&m, &client, "index_propose", json!({ "work": m.work, "key": m.keys[1], "word": "Stein" })).await;
    assert!(text.contains("200") && text.contains("Nichts angelegt"), "deutsche Meldung mit der Grenze: {text}");
    assert_eq!((count().await, commits(&m)), (200, before), "nichts angelegt, kein Commit");
    let other = local(&m, "Zweite Hilfe").await;
    ok(&m, &other, "index_propose", json!({ "work": m.work, "key": m.keys[1], "word": "Stein" })).await;

    // Limit per work (2000): fill up with foreign origins.
    sqlx::query(fill).bind(1799).bind("@mcp:viele").bind(m.work).execute(pool).await.unwrap();
    assert_eq!(count().await, 2000, "2000 offene Vorschläge im Werk (MAX_OPEN_INDEX_PROPOSALS_PER_WORK)");
    let text = refused(&m, &other, "index_propose", json!({ "work": m.work, "key": m.keys[2], "word": "Korn" })).await;
    assert!(text.contains("2000") && text.contains("Nichts angelegt"), "Grenze je Werk gemeldet: {text}");
    assert_eq!(count().await, 2000, "nichts angelegt");
}

#[tokio::test]
async fn a_title_with_a_typst_line_end_is_refused_over_mcp() {
    let m = mill([]).await;
    let client = local(&m, "Titel Hilfe").await;
    for (mark, name) in [('\u{2028}', "U+2028"), ('\u{2029}', "U+2029"), ('\u{0085}', "U+0085")] {
        let body = format!("= Das Wehr{mark}Zusatz\n\nDas Wasser staut sich.\n");
        let answer =
            call(&m, &client, "propose_revision", json!({ "work": m.work, "key": m.keys[0], "body": body })).await;
        assert!(
            answer["result"]["isError"] == true || answer.get("error").is_some(),
            "{name} im Titel: der Vorschlag wird abgelehnt (für Typst endet dort die Zeile): {answer}"
        );
    }
    let loaded = m.server.state.store.load_work(m.work).await.unwrap();
    assert_eq!(loaded.nodes[0].revisions.len(), 1, "kein Vorschlag angelegt");
    let fine = ok(
        &m,
        &client,
        "propose_revision",
        json!({ "work": m.work, "key": m.keys[0], "body": "= Das alte Wehr\n\nDas Wasser staut sich.\n" }),
    )
    .await;
    assert_eq!(fine["status"], "proposed", "ein einzeiliger Titel geht weiter: {fine}");
}

#[tokio::test]
async fn a_stale_preview_is_reported_as_stale_and_open_requests_per_origin_are_bounded() {
    let m = mill([]).await;
    let client = local(&m, "Gliederer").await;
    let made =
        ok(&m, &client, "outline_rename", json!({ "work": m.work, "key": m.keys[1], "title": "Das Mahlwerk" })).await;
    let id = made["request"]["id"].as_i64().unwrap();
    // The human meanwhile renames himself: the request is outdated.
    result(&m.server.rpc("outline.rename", json!({ "work": m.work, "key": m.keys[1], "title": "Der Läufer" })).await);
    let error = m.server.state.store.outline_request_preview(m.work, id).await.unwrap_err();
    match &error {
        baruch_store::StoreError::Stale(text) => {
            assert!(
                text.starts_with("Veraltet: ") && text.contains("Der Läufer"),
                "Wortlaut „Veraltet: <Grund>“: {text}"
            )
        }
        other => panic!("veraltet → StoreError::Stale, war {other:?}"),
    }
    let rpc = baruch_server::rpc::RpcError::from(error);
    assert_eq!(rpc.code, -32000, "über RPC bleibt es eine fachliche Ablehnung (-32000)");
    let listed = m.server.rpc("outline.requests", json!({ "work": m.work, "open": true, "preview": true })).await;
    let shown = result(&listed)["requests"][0]["preview_error"].as_str().unwrap().to_owned();
    assert_eq!(shown, rpc.message, "RPC, MCP und Seite zeigen denselben Text: {shown}");

    // Upper limit: 50 open requests per origin and work (copies of the open request fill up).
    let pool = m.server.state.store.pool();
    sqlx::query(
        "WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM n WHERE i < 49) \
         INSERT INTO outline_request (work_id, op, params, origin_kind, origin_name, note) \
         SELECT r.work_id, r.op, r.params, r.origin_kind, r.origin_name, 'Kopie ' || i FROM n, \
         (SELECT * FROM outline_request WHERE id = ?) r",
    )
    .bind(id)
    .execute(pool)
    .await
    .unwrap();
    assert_eq!(m.server.state.store.open_outline_requests(m.work).await.unwrap(), 50, "50 offene Anträge");
    let text =
        refused(&m, &client, "outline_rename", json!({ "work": m.work, "key": m.keys[2], "title": "Der Boden" })).await;
    assert!(text.contains("höchstens 50"), "der 51. Antrag derselben Herkunft wird abgelehnt: {text}");
    assert_eq!(m.server.state.store.open_outline_requests(m.work).await.unwrap(), 50, "nichts angelegt");
    let other = local(&m, "Anderer Gliederer").await;
    ok(&m, &other, "outline_rename", json!({ "work": m.work, "key": m.keys[2], "title": "Der Boden" })).await;
}

// ---- Code review ki-1 and ki-2: limits of the writing tools ---------------------------------------------------------

#[tokio::test]
async fn add_post_and_propose_revision_have_size_limits_and_quotas() {
    use baruch_server::mcp::tools::{
        MAX_MCP_OPEN_PROPOSALS, MAX_MCP_POST_CHARS, MAX_MCP_POSTS_PER_DAY, MAX_MCP_PROPOSAL_BYTES,
    };
    let m = mill([]).await;
    let client = local(&m, "Vielschreiber").await;
    let key = m.keys[0];
    let long = "ä".repeat(MAX_MCP_POST_CHARS + 1);
    let text = refused(&m, &client, "add_post", json!({ "work": m.work, "key": key, "body": long })).await;
    assert!(text.contains("at most 20000 characters") && text.contains("Nothing stored"), "Länge gemeldet: {text}");
    let exact = "ä".repeat(MAX_MCP_POST_CHARS);
    ok(&m, &client, "add_post", json!({ "work": m.work, "key": key, "body": exact })).await;
    let sources: Vec<String> = (0..21).map(|n| format!("Quelle {n}")).collect();
    let text =
        refused(&m, &client, "add_post", json!({ "work": m.work, "key": key, "body": "kurz", "sources": sources }))
            .await;
    assert!(text.contains("sources"), "zu viele Quellen: {text}");
    let big = format!("= Das Wehr\n\n{}\n", "Wasser ".repeat(MAX_MCP_PROPOSAL_BYTES / 7 + 1));
    let text = refused(&m, &client, "propose_revision", json!({ "work": m.work, "key": key, "body": big })).await;
    assert!(text.contains("The proposal") && text.contains("bytes"), "zu großer Vorschlag: {text}");
    assert_eq!(posts(&m, 0).await.len(), 1, "nur der eine erlaubte Beitrag ist gespeichert");

    // Daily quota of the posts and limit of open proposals: fill up the stock by SQL.
    let pool = m.server.state.store.pool();
    sqlx::query(
        "WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM n WHERE i < ?) \
         INSERT INTO post (node_id, position, author, body) SELECT ?, 100 + i, '@mcp:vielschreiber', 'Füllbeitrag' FROM n",
    )
    .bind(i64::try_from(MAX_MCP_POSTS_PER_DAY).unwrap() - 1)
    .bind(key)
    .execute(pool)
    .await
    .unwrap();
    let text = refused(&m, &client, "add_post", json!({ "work": m.work, "key": key, "body": "einer zu viel" })).await;
    assert!(text.contains("Quota") && text.contains("300"), "301. Beitrag in 24 Stunden abgelehnt: {text}");
    // Follow-up fix Round 23: a new name without token opens no new quota, a token has its own.
    let renamed = local(&m, "Wenigschreiber").await;
    let text = refused(&m, &renamed, "add_post", json!({ "work": m.work, "key": key, "body": "neuer Name" })).await;
    assert!(
        text.contains("clients without a token") && text.contains("300"),
        "derselbe Rechner unter anderem Namen bleibt im gemeinsamen Kontingent: {text}"
    );
    let (_, secret) = m.server.state.store.mcp_token_create("Zweitrechner").await.unwrap();
    let other = remote(&m, &secret).await;
    ok(&m, &other, "add_post", json!({ "work": m.work, "key": key, "body": "ein Client mit Token schreibt weiter" }))
        .await;
    let as_persona = json!({ "work": m.work, "key": key, "body": "im Namen der Persona", "persona": "@lektor" });
    let posted = ok(&m, &renamed, "add_post", as_persona).await;
    assert_eq!(posted["author"], "@lektor", "im Namen einer Persona zählt deren eigener Bestand: {posted}");

    sqlx::query(
        "WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM n WHERE i < ?) \
         INSERT INTO revision (node_id, number, body, author_kind, author_name, model, status) \
         SELECT ?, 1 + i, '= Das Wehr' || char(10) || char(10) || 'Fassung ' || i, 'assistant', '@mcp:vielschreiber', 'm', \
         'proposed' FROM n",
    )
    .bind(i64::try_from(MAX_MCP_OPEN_PROPOSALS).unwrap())
    .bind(key)
    .execute(pool)
    .await
    .unwrap();
    let body = "= Das Wehr\n\nNoch ein Vorschlag.\n";
    let text = refused(&m, &client, "propose_revision", json!({ "work": m.work, "key": key, "body": body })).await;
    assert!(text.contains("open proposals") && text.contains("100"), "101. offener Vorschlag abgelehnt: {text}");
    let text = refused(&m, &renamed, "propose_revision", json!({ "work": m.work, "key": key, "body": body })).await;
    assert!(
        text.contains("clients without a token") && text.contains("open proposals"),
        "auch die Grenze offener Vorschläge gilt für alle Namen ohne Token zusammen: {text}"
    );
    let fine = ok(&m, &other, "propose_revision", json!({ "work": m.work, "key": key, "body": body })).await;
    assert_eq!(fine["status"], "proposed", "ein Token ist davon nicht betroffen: {fine}");
}

#[tokio::test]
async fn a_batch_is_bounded_and_source_read_bounds_its_query() {
    let m = mill([]).await;
    let client = local(&m, "Stapler").await;
    let ping = json!({ "jsonrpc": "2.0", "id": 1, "method": "ping" });
    let (_, _, answer) = mcp(&m, &client, Value::Array(vec![ping.clone(); baruch_server::mcp::MAX_BATCH + 1])).await;
    assert_eq!(error_code(&answer), -32600, "51 Nachrichten in einem Stapel: abgelehnt, keine ausgeführt: {answer}");
    let (_, _, answer) = mcp(&m, &client, Value::Array(vec![ping; baruch_server::mcp::MAX_BATCH])).await;
    assert_eq!(answer.as_array().map(Vec::len), Some(50), "50 gehen weiter: jede bekommt ihre Antwort");

    let words = "zqx ".repeat(60);
    let text = refused(&m, &client, "source_read", json!({ "document": "d1", "collection": "c", "q": words })).await;
    assert!(
        text.starts_with("q: at most 500 characters and 40 words"),
        "zu viele Wörter, bevor etwas gesucht wird: {text}"
    );
    let text =
        refused(&m, &client, "source_read", json!({ "document": "d1", "collection": "c", "q": "x".repeat(501) })).await;
    assert!(text.starts_with("q: at most 500 characters"), "zu lang: {text}");
}

// ---- Findings move along on moving ----------------------------------------------------------------------------------

#[tokio::test]
async fn moving_a_post_takes_its_findings_along_except_adopted_ones() {
    let m = mill([]).await;
    let post = add_post(&m, 1, "@lektor", "Kritisch: der Stein.\nKritisch: die Körner.").await;
    let open = finding(&m, 1, Some(post), 0, "@lektor", "Der Stein dreht zu langsam.").await;
    let adopted = finding(&m, 1, Some(post), 1, "@lektor", "Die Körner sind gezählt.").await;
    let elsewhere = finding(&m, 1, None, 0, "@nathan", "Ein Befund ohne diesen Beitrag.").await;
    let pool = m.server.state.store.pool();
    sqlx::query(
        "UPDATE finding SET status = 'adopted', adopted_revision = (SELECT uuid FROM revision WHERE node_id = ?) WHERE id = ?",
    )
    .bind(m.keys[1])
    .bind(adopted)
    .execute(pool)
    .await
    .unwrap();
    let before = commits(&m);
    let moved = m.server.rpc("post.move", json!({ "work": m.work, "id": post, "target_key": m.keys[2] })).await;
    assert_eq!(result(&moved)["key"], m.keys[2], "der Beitrag steht im Speicher: {moved}");
    let store = &m.server.state.store;
    assert_eq!(store.finding_get(m.work, open).await.unwrap().key, Some(m.keys[2]), "der offene Befund zieht mit");
    assert_eq!(
        store.finding_get(m.work, adopted).await.unwrap().key,
        Some(m.keys[1]),
        "der übernommene bleibt: sein Status nennt eine Fassung des alten Abschnitts"
    );
    assert_eq!(store.finding_get(m.work, elsewhere).await.unwrap().key, Some(m.keys[1]), "fremde Befunde bleiben");
    assert_eq!(commits(&m), before + 1, "Beitrag und Befunde in einem Commit");
}
