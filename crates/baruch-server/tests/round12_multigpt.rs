// SPDX-License-Identifier: AGPL-3.0-or-later
//! Round 12 („multigpt“): MultiGPT MCP as a RAG way and the MCP tools `rag_search`/`rag_status`.
//!
//! Everything against the mock `baruch_server::connections::multigpt::mock::MockMultigpt` on 127.0.0.1:0 – never
//! against the real MultiGPT, never against Eliezer. Secrets only in `MemorySecrets`. Test texts freely invented
//! („Das Jahr am Leuchtturm“, „Senfkornbuch“).

use crate::common;

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode, header};
use baruch_rag::{Query, SearchWay};
use baruch_server::AppState;
use baruch_server::connections::MemorySecrets;
use baruch_server::connections::multigpt::mock::{MockMultigpt, ResultForm};
use baruch_server::connections::multigpt::{Limits, MultigptWay, open_way, rank_score};
use serde_json::{Value, json};

/// Unique test token; it must appear nowhere.
const TOKEN: &str = "Mg7-nachbau-token-4b2e9d";

async fn open_with(secrets: MemorySecrets) -> common::Server {
    let data = tempfile::tempdir().unwrap();
    let state = AppState::open_with_secrets(data.path(), Arc::new(secrets)).await.expect("Datenordner öffnen");
    let state = Arc::new(state);
    common::Server { app: common::router(state.clone()), state, data }
}

async fn ok(server: &common::Server, method: &str, params: Value) -> Value {
    let answer = server.rpc(method, params).await;
    assert!(answer["error"].is_null(), "{method} sollte gelingen: {answer}");
    answer["result"].clone()
}

/// Point MultiGPT to the mock and (like the human after an address change) enter the token anew.
async fn point_to(server: &common::Server, mock: &MockMultigpt, token: Option<&str>) {
    ok(server, "connection.set", json!({ "kind": "multigpt", "config": { "url": mock.url() } })).await;
    if let Some(token) = token {
        ok(server, "connection.set_secret", json!({ "kind": "multigpt", "secret": token })).await;
    }
}

async fn check(server: &common::Server) -> Value {
    ok(server, "connection.check", json!({ "kind": "multigpt" })).await
}

fn tool(name: &str) -> Value {
    json!({ "name": name, "inputSchema": { "type": "object", "properties": {} } })
}

fn query(collections: &[&str], limit: usize) -> Query {
    Query {
        text: "Licht über dem Wasser".into(),
        limit,
        collections: collections.iter().map(|c| (*c).to_owned()).collect(),
        work: 1,
    }
}

fn hits() -> Vec<Value> {
    vec![
        json!({ "title": "Das Jahr am Leuchtturm", "document_id": "lt-1", "collection": "Leuchtturm", "page": 12,
                "text": "Das Licht kreist über dem Wasser.", "score": 0.91, "url": "https://bib.example/lt-1" }),
        json!({ "title": "Senfkornbuch", "document_id": "sk-3", "collection": "Senfkorn",
                "text": "Ein kleines Korn.", "score": 0.88 }),
        json!({ "title": "Leuchtfeuer", "document_id": "lt-2", "collection": "Leuchtturm",
                "text": "Nebel kommt.", "score": 7.5 }),
    ]
}

// ---- Checking the connection --------------------------------------------------------------------------

#[tokio::test]
async fn the_check_reads_every_tools_page_and_the_collections() {
    let mock = MockMultigpt::start().await;
    mock.require_bearer(Some(TOKEN));
    mock.configure(|b| {
        b.page_size = 2;
        b.tools.push(tool("ask"));
    });
    let server = open_with(MemorySecrets::with([("multigpt", TOKEN)])).await;
    point_to(&server, &mock, Some(TOKEN)).await;
    let result = check(&server).await;
    assert_eq!(result["ok"], true, "Nachbau antwortet: {result}");
    assert_eq!(
        result["details"]["tools"],
        json!(["search_documents", "list_collections", "document_info", "read_document", "ask"]),
        "alle fünf Werkzeuge über drei Seiten (nextCursor) in der Reihenfolge der Gegenstelle"
    );
    assert_eq!(result["details"]["pages"], 3, "drei Seiten zu je zwei Werkzeugen");
    assert_eq!(
        result["details"]["collections"],
        json!([{ "name": "Leuchtturm", "count": 3 }, { "name": "Senfkorn", "count": 2 }]),
        "Sammlungen aus list_collections in der gemeinsamen Form {{name, count}}"
    );
    assert_eq!(result["details"]["server"], json!({ "name": "multigpt-nachbau", "version": "0.1" }), "Server");
    let methods: Vec<String> = mock
        .requests()
        .iter()
        .map(|r| format!("{} {}", r["method"].as_str().unwrap_or(""), r["params"]["cursor"].as_str().unwrap_or("-")))
        .collect();
    assert_eq!(
        methods,
        [
            "initialize -",
            "notifications/initialized -",
            "tools/list -",
            "tools/list seite-1",
            "tools/list seite-2",
            "tools/call -",
            "DELETE -"
        ],
        "Reihenfolge: Sitzung, dann tools/list mit dem Cursor der Gegenstelle, list_collections und zuletzt DELETE \
         (die Prüfung nutzt keinen Sitzungsvorrat und beendet ihre Sitzung, Runde 19a)"
    );
    for r in mock.requests() {
        assert_eq!(r["authorization"], "passt", "jede Anfrage trägt das Token: {r}");
        if r["method"] != "initialize" {
            assert_eq!(r["session"], "nachbau-sitzung-1", "Mcp-Session-Id nach initialize: {r}");
            assert_eq!(r["protocol"], "2025-06-18", "MCP-Protocol-Version: {r}");
        }
    }
}

#[tokio::test]
async fn the_check_stops_after_ten_pages_and_says_so() {
    let mock = MockMultigpt::start().await;
    mock.configure(|b| {
        b.page_size = 2;
        b.tools = (0..25).map(|i| tool(&format!("werkzeug_{i:02}"))).collect();
    });
    let server = common::open().await;
    point_to(&server, &mock, None).await;
    let result = check(&server).await;
    assert_eq!(result["ok"], true, "zu viele Seiten sind kein Fehler: {result}");
    assert_eq!(result["details"]["pages"], 10, "höchstens zehn Seiten");
    assert_eq!(result["details"]["tools"].as_array().unwrap().len(), 20, "die Werkzeuge der ersten zehn Seiten");
    let message = result["message"].as_str().unwrap();
    assert!(message.contains("mehr als 10 Seiten"), "Hinweis auf die Grenze: {message}");
    assert!(message.contains("list_collections fehlt"), "ehrlich: keine Sammlungen ohne das Werkzeug: {message}");
    assert_eq!(result["details"]["collections"], json!([]), "keine Sammlungen gemeldet");
    let lists = mock.requests().iter().filter(|r| r["method"] == "tools/list").count();
    assert_eq!(lists, 10, "die elfte Seite wird nicht mehr angefragt");
}

#[tokio::test]
async fn a_missing_or_wrong_token_is_denied_with_a_hint_and_never_shown() {
    let mock = MockMultigpt::start().await;
    mock.require_bearer(Some(TOKEN));
    let server = common::open().await;
    point_to(&server, &mock, None).await;
    let result = check(&server).await;
    assert_eq!(result["ok"], false, "ohne Token: {result}");
    let message = result["message"].as_str().unwrap();
    assert!(
        message.contains("HTTP 401") && message.contains("kein Schlüssel hinterlegt (baruch/multigpt)"),
        "{message}"
    );

    ok(&server, "connection.set_secret", json!({ "kind": "multigpt", "secret": "falsches-token-1" })).await;
    let result = check(&server).await;
    assert_eq!(result["ok"], false, "falsches Token: {result}");
    let message = result["message"].as_str().unwrap();
    assert!(message.contains("Schlüssel baruch/multigpt prüfen"), "Hinweis auf das Konto: {message}");
    assert!(!result.to_string().contains("falsches-token-1"), "das Token steht nirgends im Ergebnis: {result}");

    // The peer mirrors the token back in its name and in collections: Baruch replaces it everywhere.
    ok(&server, "connection.set_secret", json!({ "kind": "multigpt", "secret": TOKEN })).await;
    mock.configure(|b| {
        b.server_name = format!("spiegel {TOKEN}");
        b.collections = vec![json!({ "name": format!("Sammlung {TOKEN}"), "count": 1 })];
    });
    let result = check(&server).await;
    assert_eq!(result["ok"], true, "richtiges Token: {result}");
    let listed = ok(&server, "connection.get", json!({ "kind": "multigpt" })).await;
    let (_, page) = server.get("/einstellungen/verbindungen").await;
    for (what, text) in [("Ergebnis", result.to_string()), ("connection.get", listed.to_string()), ("Seite", page)] {
        assert!(!text.contains(TOKEN), "{what}: das Token erscheint nie: {text}");
    }
    assert!(result["details"]["server"]["name"] == "spiegel ***", "durch *** ersetzt: {result}");
}

#[tokio::test]
async fn the_token_never_goes_to_another_address() {
    let first = MockMultigpt::start().await;
    let second = MockMultigpt::start().await;
    let server = open_with(MemorySecrets::with([("multigpt", TOKEN)])).await;
    point_to(&server, &first, Some(TOKEN)).await;
    // New address without a new token: the old one holds only for the old address.
    ok(&server, "connection.set", json!({ "kind": "multigpt", "config": { "url": second.url() } })).await;
    let result = check(&server).await;
    assert_eq!(result["ok"], false, "nicht geprüft: {result}");
    assert!(second.requests().is_empty(), "Prüfen schickt nichts an die neue Adresse");
    let refused = open_way(&server.state).await.err().expect("kein Weg mit fremdem Token");
    assert_eq!(refused.code(), "not_configured", "Weg aus, nicht ausgefallen: {refused}");
    assert!(refused.message().contains("andere Adresse"), "ehrlicher Grund: {refused}");
    assert!(second.requests().is_empty(), "auch die Suche fragt die neue Adresse nicht");
}

// ---- Search -------------------------------------------------------------------------------------------

#[tokio::test]
async fn search_asks_only_the_chosen_collections_and_reads_source_details() {
    let mock = MockMultigpt::start().await;
    mock.require_bearer(Some(TOKEN));
    mock.set_hits(hits());
    let server = open_with(MemorySecrets::with([("multigpt", TOKEN)])).await;
    point_to(&server, &mock, Some(TOKEN)).await;
    let way = open_way(&server.state).await.expect("Weg aus den Einstellungen");
    assert_eq!(way.way(), baruch_rag::Way::Multigpt, "Weg MultiGPT");
    let found = way.search(&query(&["Leuchtturm"], 5)).await.expect("Suche");
    assert_eq!(
        mock.tool_calls("search_documents"),
        vec![json!({ "query": "Licht über dem Wasser", "collections": ["Leuchtturm"], "limit": 5 })],
        "ein Aufruf, nur die gewählte Sammlung, Namen aus dem inputSchema"
    );
    assert_eq!(found.len(), 2, "nur die Treffer der gewählten Sammlung: {found:?}");
    let first = &found[0];
    assert_eq!(
        (first.title.as_str(), first.document.as_str(), first.collection.as_str(), first.section.as_deref()),
        ("Leuchtfeuer", "lt-2", "Leuchtturm", None),
        "Score 7,5 wird auf 1 geklemmt und steht damit vorn"
    );
    assert_eq!(first.score, 1.0, "geklemmt");
    let second = &found[1];
    assert_eq!(
        (second.title.as_str(), second.section.as_deref(), second.url.as_deref(), second.score),
        ("Das Jahr am Leuchtturm", Some("12"), Some("https://bib.example/lt-1"), 0.91),
        "Titel, Stelle, URL und Score der Gegenstelle"
    );
    assert_eq!(second.snippet, "Das Licht kreist über dem Wasser.", "Ausschnitt");
    assert!(mock.requests().iter().all(|r| r["authorization"] == "passt"), "Token bei jeder Anfrage");
}

#[tokio::test]
async fn a_schema_with_one_collection_per_call_gets_one_call_each_and_ranked_scores() {
    let mock = MockMultigpt::start().await;
    mock.set_tools(vec![json!({ "name": "search_documents", "inputSchema": { "type": "object",
        "required": ["q", "collection_id"],
        "properties": { "q": { "type": "string" }, "collection_id": { "type": "string" },
                        "top_k": { "type": "integer", "maximum": 3 } } } })]);
    mock.set_hits(vec![
        json!({ "name": "Senfkornbuch", "doc_id": "sk-1", "collection": "Senfkorn", "content": "Klein und groß." }),
        json!({ "name": "Senfkornbuch", "doc_id": "sk-2", "collection": "Senfkorn", "content": "Ein Baum." }),
        json!({ "name": "Das Jahr am Leuchtturm", "doc_id": "lt-1", "collection": "Leuchtturm", "content": "Licht." }),
    ]);
    mock.configure(|b| b.result = ResultForm::TextJson);
    let way = MultigptWay::new(&mock.url(), None, Limits::SEARCH);
    let found = way.search(&query(&["Senfkorn", "Leuchtturm"], 10)).await.expect("Suche");
    assert_eq!(
        mock.tool_calls("search_documents"),
        vec![
            json!({ "q": "Licht über dem Wasser", "collection_id": "Senfkorn", "top_k": 3 }),
            json!({ "q": "Licht über dem Wasser", "collection_id": "Leuchtturm", "top_k": 3 }),
        ],
        "q, collection_id als Text (ein Aufruf je Sammlung) und top_k an maximum 3 geklemmt"
    );
    let summary: Vec<(String, f32)> = found.iter().map(|h| (h.document.clone(), h.score)).collect();
    assert_eq!(
        summary,
        vec![("sk-1".into(), 1.0), ("lt-1".into(), 1.0), ("sk-2".into(), rank_score(2) as f32)],
        "ohne Score Ersatz aus dem Rang je Aufruf (rank_score, seit R21a nie „schwach“; aus JSON in content gelesen); gleicher Score behält die Reihenfolge"
    );
    assert_eq!(found[0].title, "Senfkornbuch", "Titel aus name");
}

#[tokio::test]
async fn unreadable_answers_errors_timeouts_and_redirects_are_honest() {
    let mock = MockMultigpt::start().await;
    mock.set_hits(hits());
    let way = MultigptWay::new(&mock.url(), None, Limits::SEARCH);

    mock.configure(|b| b.result = ResultForm::Unreadable);
    let e = way.search(&query(&["Leuchtturm"], 5)).await.unwrap_err();
    assert_eq!(e.code(), "schema", "unlesbares Ergebnis ist ein Schema-Fehler: {e}");

    mock.configure(|b| {
        b.result = ResultForm::Structured;
        b.tool_error = Some("Sammlung Leuchtturm ist gesperrt".into());
    });
    let e = way.search(&query(&["Leuchtturm"], 5)).await.unwrap_err();
    assert_eq!(e.code(), "failed", "isError der Gegenstelle: {e}");
    assert!(e.message().contains("Sammlung Leuchtturm ist gesperrt"), "mit ihrem Text: {e}");

    mock.configure(|b| {
        b.tool_error = None;
        b.delay = Duration::from_secs(5);
    });
    let quick = MultigptWay::new(&mock.url(), None, Limits { timeout: Duration::from_secs(1), max_body: 1 << 20 });
    let started = Instant::now();
    let e = quick.search(&query(&["Leuchtturm"], 5)).await.unwrap_err();
    assert_eq!(e.code(), "timeout", "Zeitlimit greift: {e}");
    assert!(started.elapsed() < Duration::from_secs(4), "nach etwa einer Sekunde: {:?}", started.elapsed());

    let target = MockMultigpt::start().await;
    mock.configure(|b| {
        b.delay = Duration::ZERO;
        b.redirect = Some(target.url());
    });
    let e = MultigptWay::new(&mock.url(), Some(TOKEN), Limits::SEARCH)
        .search(&query(&["Leuchtturm"], 5))
        .await
        .unwrap_err();
    assert!(e.message().contains("leitet weiter") && e.message().contains("307"), "Weiterleitung benannt: {e}");
    assert!(target.requests().is_empty(), "der Weiterleitung wird nicht gefolgt – das Token geht nicht ans Ziel");
    assert!(!e.message().contains(TOKEN), "kein Token in der Meldung");

    mock.configure(|b| b.redirect = None);
    let none = way.search(&query(&[], 5)).await.unwrap_err();
    assert_eq!(none.code(), "not_configured", "ohne Sammlung wird nicht gefragt: {none}");
}

#[tokio::test]
async fn search_works_over_an_event_stream_and_scrubs_reflected_tokens() {
    let mock = MockMultigpt::start().await;
    mock.require_bearer(Some(TOKEN));
    mock.configure(|b| b.event_stream = true);
    mock.set_hits(vec![json!({ "title": format!("Titel {TOKEN}"), "document_id": "lt-1",
        "collection": "Leuchtturm", "text": format!("Ausschnitt mit {TOKEN}"), "score": 0.5 })]);
    let found = MultigptWay::new(&mock.url(), Some(TOKEN), Limits::SEARCH)
        .search(&query(&["Leuchtturm"], 5))
        .await
        .expect("Suche über text/event-stream");
    assert_eq!(found.len(), 1, "ein Treffer");
    let text = serde_json::to_string(&found).unwrap();
    assert!(!text.contains(TOKEN) && text.contains("***"), "Token aus Titel und Ausschnitt entfernt: {text}");
}

#[tokio::test]
async fn an_event_stream_cut_inside_an_umlaut_is_read_to_the_end() {
    let mock = MockMultigpt::start().await;
    // Reason: the mock sends every response in two pieces, the first ends after the first byte of the first multi-byte
    // character; before, the search then aborted with „kein UTF-8-Text“.
    mock.configure(|b| {
        b.event_stream = true;
        b.split_utf8 = true;
    });
    mock.set_hits(vec![json!({ "title": "Möwen über dem Kai", "document_id": "lt-9",
        "collection": "Leuchtturm", "text": "Die Möwen schreien „früh“.", "score": 0.6 })]);
    let found = MultigptWay::new(&mock.url(), None, Limits::SEARCH)
        .search(&query(&["Leuchtturm"], 5))
        .await
        .expect("Suche über einen geteilten Ereignisstrom");
    assert_eq!(found.len(), 1, "ein Treffer: {found:?}");
    assert_eq!(found[0].title, "Möwen über dem Kai", "Umlaute heil über die Stückgrenze");
    assert!(found[0].snippet.contains("„früh“"), "Ausschnitt heil: {}", found[0].snippet);
}

#[tokio::test]
async fn read_document_only_ever_gives_an_excerpt_and_document_info_completes_hits() {
    let mock = MockMultigpt::start().await;
    let long = "Das Senfkorn wächst. ".repeat(400);
    mock.set_hits(vec![json!({ "document_id": "sk-1", "collection": "Senfkorn", "score": 0.7 })]);
    mock.configure(|b| {
        b.texts.insert("sk-1".into(), long.clone());
        b.documents.insert("sk-1".into(), json!({ "title": "Senfkornbuch", "url": "https://bib.example/sk-1" }));
    });
    let way = MultigptWay::new(&mock.url(), None, Limits::SEARCH);
    let found = way.search(&query(&["Senfkorn"], 5)).await.expect("Suche");
    let hit = &found[0];
    assert_eq!(hit.title, "Senfkornbuch", "Titel aus document_info (der Treffer nannte nur das Dokument)");
    assert_eq!(hit.url.as_deref(), Some("https://bib.example/sk-1"), "URL aus document_info");
    assert!(
        !hit.snippet.is_empty() && hit.snippet.chars().count() <= baruch_rag::MAX_SNIPPET_CHARS,
        "fehlender Ausschnitt aus read_document, höchstens 600 Zeichen: {}",
        hit.snippet.chars().count()
    );
    assert!(long.len() > 5000, "die Gegenstelle hätte viel mehr geliefert");
    assert_eq!(mock.tool_calls("read_document"), vec![json!({ "document_id": "sk-1" })], "Dokument aus dem Schema");
    let excerpt = way.excerpt("sk-1", None).await.expect("Ausschnitt");
    assert!(excerpt.chars().count() <= baruch_rag::MAX_SNIPPET_CHARS && excerpt.ends_with('…'), "nur Ausschnitt");
    // Round 13: read_full delivers the whole text (the stub is gone); the search stays with the excerpt.
    let full = way.read_full("sk-1").await.expect("Volltext");
    assert_eq!(full, long, "read_full gibt den Text der Gegenstelle unverändert zurück");
}

#[tokio::test]
async fn open_way_needs_an_address() {
    let server = common::open().await;
    // The page rejects an empty address; an old database could contain it nevertheless.
    server.state.store.connection_set(baruch_store::ConnectionKind::Multigpt, &json!({ "url": " " })).await.unwrap();
    let e = open_way(&server.state).await.err().expect("ohne Adresse kein Weg");
    assert_eq!(e.code(), "not_configured", "aus: {e}");
}

// ---- MCP -----------------------------------------------------------------------------------------------

async fn mcp(server: &common::Server, token: Option<&str>, message: Value) -> (StatusCode, Value) {
    let mut request = Request::post("/mcp").header(header::CONTENT_TYPE, "application/json");
    if let Some(token) = token {
        request = request.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    let mut request = request.body(Body::from(message.to_string())).unwrap();
    request.extensions_mut().insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 40000))));
    let (status, _, body) = server.send(request).await;
    (status, serde_json::from_str(&body).unwrap_or(Value::Null))
}

fn call(name: &str, arguments: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": 7, "method": "tools/call", "params": { "name": name, "arguments": arguments } })
}

#[tokio::test]
async fn mcp_offers_rag_search_and_rag_status_read_only_and_no_reindex() {
    let server = common::open().await;
    let (_, list) = mcp(&server, None, json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list" })).await;
    let tools = list["result"]["tools"].as_array().unwrap();
    let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
    let at = names.iter().position(|n| *n == "bible_search").unwrap();
    assert_eq!(
        &names[at + 4..],
        ["rag_search", "rag_status", "source_read"],
        "nach bible_search (seit Runde 15 erst die drei lesenden Kommentar-/Lexikon-Werkzeuge); seit Runde 13 \
         dahinter nur das lesende source_read: {names:?}"
    );
    assert!(!names.iter().any(|n| n.contains("reindex")), "kein Neuaufbau über MCP: {names:?}");
    let search = &tools[at + 4];
    let text = search["description"].as_str().unwrap();
    for needed in ["read-only", "not instructions", "connected, failed, off", "chosen only by the human"] {
        assert!(text.contains(needed), "Beschreibung nennt „{needed}“: {text}");
    }
    assert_eq!(search["inputSchema"]["required"], json!(["query"]), "query Pflicht");
    assert!(tools[at + 5]["description"].as_str().unwrap().contains("read-only"), "rag_status liest nur");

    let (_, answer) = mcp(&server, None, call("rag_reindex", json!({ "work": 1 }))).await;
    assert_eq!(answer["error"]["code"], -32602, "rag_reindex gibt es über MCP nicht: {answer}");
}

#[tokio::test]
async fn mcp_rag_tools_answer_honestly_over_loopback_and_with_a_token() {
    let server = common::open().await;
    let made = baruch_server::mcp::tokens::call(&server.state, "mcp_token.create", json!({ "name": "lesehilfe" }))
        .await
        .expect("Token erzeugen");
    let secret = made["secret"].as_str().unwrap().to_owned();
    for token in [None, Some(secret.as_str())] {
        let (status, answer) = mcp(&server, token, call("rag_search", json!({ "query": "Leuchtturm" }))).await;
        assert_eq!(status, StatusCode::OK, "Zugang über Loopback bzw. mit Token");
        // Wired: the search answers, without an embedder (tests) LanceDB has honestly failed.
        assert_ne!(answer["result"]["isError"], true, "Suche beantwortet: {answer}");
        let text = answer["result"]["content"][0]["text"].as_str().unwrap();
        assert!(
            text.contains("\"state\":\"failed\"") && text.contains("Kein Einbettungsmodell"),
            "ehrlicher Zustand: {text}"
        );
        let (_, answer) = mcp(&server, token, call("rag_status", json!({}))).await;
        assert_ne!(answer["result"]["isError"], true, "rag_status beantwortet: {answer}");
        assert!(
            answer["result"]["content"][0]["text"].as_str().unwrap().contains("\"fts\":true"),
            "Status-JSON: {answer}"
        );
    }
    let (_, answer) = mcp(&server, None, call("rag_search", json!({ "query": "  " }))).await;
    assert!(answer["result"]["content"][0]["text"].as_str().unwrap().contains("query is missing"), "leer: {answer}");
    let (_, answer) = mcp(&server, None, call("rag_search", json!({ "query": "x", "ways": ["web"] }))).await;
    assert!(answer["result"]["content"][0]["text"].as_str().unwrap().contains("is not a way"), "Weg: {answer}");
    let (_, answer) = mcp(&server, None, call("rag_search", json!({ "query": "x", "work": 9999 }))).await;
    assert!(answer["result"]["content"][0]["text"].as_str().unwrap().contains("Unknown work"), "Werk: {answer}");
}
