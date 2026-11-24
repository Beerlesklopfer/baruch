// SPDX-License-Identifier: AGPL-3.0-or-later
//! Runde 13, Ende-zu-Ende (Integrator): Quellen über MultiGPT, Websuche für `@belege`, Abgleich – zusammen
//! gegen **ein** MultiGPT, wie der Mensch es einträgt.
//!
//! Das eine MultiGPT ist hier eine kleine Vorderseite ([`OneMultigpt`], MCP über Streamable HTTP auf
//! 127.0.0.1:0), die jedes Werkzeug an den Nachbau seiner Aufgabe weitergibt: `search_documents` an
//! `MockMultigpt` (quellen), `web_search`/`fetch_url` an `MockWebMcp` mit der lokalen `MockSite` (websuche),
//! die Dokument-Werkzeuge an `MockDocs` (abgleich). Einbettungen kommen von `MockEliezer`, Antworten der Personas
//! von `ScriptedModel`. Nie das echte MultiGPT, Eliezer oder das Internet; Testtexte frei erfunden
//! („Das Jahr am Leuchtturm“ hier als eigenes Testbuch „Die Fähre im Nebel“).
//!
//! Geprüft wird nur, was die Teile zusammen tun: Dokumentansicht mit Volltext, Prüfung von Hervorhebungen,
//! Übernahme in die Bibliothek samt Index, @belege mit Websuche (bevorzugte Quelle zuerst und gekennzeichnet,
//! Zitat nicht auf der Seite, Seite mit eingeschleuster Anweisung), Abgleich (nur nach Bestätigung, nur Geändertes,
//! Holen und offline finden, Ausfall), und offline die lokale Kopie in der Dokumentansicht. Alles über RPC oder
//! Formulare (ohne JavaScript).

use crate::common;

use std::sync::Arc;

use axum::Router;
use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, Method, StatusCode, header};
use axum::response::{IntoResponse, Response};
use baruch_llm::mock::MockEliezer;
use baruch_server::assist::mock::{ScriptedModel, Step};
use baruch_server::assist::web_facts::BLOCK_TITLE;
use baruch_server::assist::{ModelCall, Role};
use baruch_server::connections::MemorySecrets;
use baruch_server::connections::multigpt::mock::MockMultigpt;
use baruch_server::sync::mock::MockDocs;
use baruch_server::websearch::mock::{MockSite, MockWebMcp};
use baruch_server::{rag, sync};
use serde_json::{Value, json};

const FERRY: &str = "// Testbuch Runde 13 Ende-zu-Ende (frei erfunden).
#set document(title: \"Die Fähre im Nebel\", author: \"Tilda Kran\")

= Ablegen

Die Fähre löste die Leinen, und der Nebel schluckte den Kai.

== Das Horn

Der Fährmann blies dreimal ins Horn, damit die Fischer ihn hörten.

= Überfahrt

Mitten auf dem Fluss stand das Wasser still wie Glas.

= Ankunft

Am anderen Ufer warteten zwei Kinder mit einer Laterne.
";

const COLLECTION: &str = "Baruch: Die Fähre im Nebel";
const LIGHT: &str = "https://lichtkunde.example/feuer";
const GUESTS: &str = "https://gaeste.example/buch";
const QUESTION: &str = "Wie oft dreht sich das Leuchtfeuer?";
/// Volltext des erfundenen MultiGPT-Dokuments `lf-1` in der Sammlung „Leuchtfeuer“.
const BEACON: &str = "Der Wärter putzte jeden Abend die Linse der Laterne.\n\n\
Über dem Wasser kreiste das Licht, und die Möwen schrien im Wind.";

// ---- Ein MultiGPT aus drei Nachbauten -------------------------------------------------------------------------

/// Welcher Nachbau ein Werkzeug beantwortet: 0 = MockMultigpt, 1 = MockWebMcp, 2 = MockDocs.
fn owner(tool: &str) -> Option<usize> {
    match tool {
        "search_documents" => Some(0),
        "web_search" | "fetch_url" => Some(1),
        "list_collections" | "list_documents" | "read_document" | "document_info" | "upload_document"
        | "delete_document" => Some(2),
        _ => None,
    }
}

const FRONT_SESSION: &str = "r13-ein-multigpt-1";

struct Front {
    backends: [(String, &'static str); 3],
    http: reqwest::Client,
}

/// MCP-Vorderseite auf 127.0.0.1:0: `initialize` beantwortet sie selbst, `tools/list` vereinigt die Werkzeuge
/// der Nachbauten (jedes Werkzeug von seinem Besitzer), `tools/call` geht unverändert an den Besitzer, `DELETE`
/// an alle (jeder Nachbau zählt es).
struct OneMultigpt {
    url: String,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for OneMultigpt {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl OneMultigpt {
    async fn start(search: &MockMultigpt, web: &MockWebMcp, docs: &MockDocs) -> OneMultigpt {
        let front = Arc::new(Front {
            backends: [
                (search.url(), baruch_server::connections::multigpt::mock::SESSION),
                (web.url(), baruch_server::websearch::mock::SESSION),
                (docs.url(), baruch_server::sync::mock::SESSION),
            ],
            http: reqwest::Client::new(),
        });
        let app = Router::new().route("/mcp", axum::routing::any(front_mcp)).with_state(front);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("Vorderseite: kein Port");
        let url = format!("http://{}/mcp", listener.local_addr().unwrap());
        let task = tokio::spawn(async move {
            axum::serve(listener, app).await.ok();
        });
        OneMultigpt { url, task }
    }
}

async fn forward(front: &Front, index: usize, body: Vec<u8>) -> reqwest::Response {
    let (url, session) = &front.backends[index];
    front
        .http
        .post(url)
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::ACCEPT, "application/json, text/event-stream")
        .header("mcp-session-id", *session)
        .body(body)
        .send()
        .await
        .expect("Nachbau antwortet")
}

async fn front_mcp(State(front): State<Arc<Front>>, method: Method, headers: HeaderMap, body: Bytes) -> Response {
    if method == Method::DELETE {
        for (url, session) in &front.backends {
            front.http.delete(url).header("mcp-session-id", *session).send().await.ok();
        }
        return StatusCode::OK.into_response();
    }
    let message: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
    let id = message.get("id").cloned();
    let rpc = message["method"].as_str().unwrap_or("");
    if rpc == "initialize" {
        let result = json!({ "protocolVersion": "2025-06-18", "capabilities": { "tools": {} },
            "serverInfo": { "name": "ein-multigpt-nachbau", "version": "0.1" } });
        let mut response = axum::Json(json!({ "jsonrpc": "2.0", "id": id, "result": result })).into_response();
        response.headers_mut().insert("mcp-session-id", FRONT_SESSION.parse().unwrap());
        return response;
    }
    if headers.get("mcp-session-id").and_then(|v| v.to_str().ok()) != Some(FRONT_SESSION) {
        return (StatusCode::BAD_REQUEST, "Mcp-Session-Id fehlt").into_response();
    }
    if id.is_none() {
        return StatusCode::ACCEPTED.into_response();
    }
    match rpc {
        "tools/list" => {
            let mut tools = Vec::new();
            for index in 0..front.backends.len() {
                let ask = json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list" }).to_string().into_bytes();
                let answer: Value = forward(&front, index, ask).await.json().await.expect("tools/list als JSON");
                for tool in answer["result"]["tools"].as_array().cloned().unwrap_or_default() {
                    if owner(tool["name"].as_str().unwrap_or("")) == Some(index) {
                        tools.push(tool);
                    }
                }
            }
            axum::Json(json!({ "jsonrpc": "2.0", "id": id, "result": { "tools": tools } })).into_response()
        }
        "tools/call" => {
            let Some(index) = owner(message["params"]["name"].as_str().unwrap_or("")) else {
                let error = json!({ "code": -32602, "message": "unbekanntes Werkzeug" });
                return axum::Json(json!({ "jsonrpc": "2.0", "id": id, "error": error })).into_response();
            };
            let answer = forward(&front, index, body.to_vec()).await;
            let status = StatusCode::from_u16(answer.status().as_u16()).unwrap();
            let kind = answer
                .headers()
                .get(header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .unwrap_or("application/json")
                .to_owned();
            let bytes = answer.bytes().await.unwrap_or_default();
            (status, [(header::CONTENT_TYPE, kind)], bytes).into_response()
        }
        other => {
            let error = json!({ "code": -32601, "message": format!("unbekannte Methode {other}") });
            axum::Json(json!({ "jsonrpc": "2.0", "id": id, "error": error })).into_response()
        }
    }
}

// ---- Aufbau --------------------------------------------------------------------------------------------------

struct World {
    server: common::Server,
    model: ScriptedModel,
    work: i64,
    search: MockMultigpt,
    web: MockWebMcp,
    docs: MockDocs,
    eliezer: MockEliezer,
    front: OneMultigpt,
    _site: MockSite,
}

/// Datenordner mit Testbuch, Nachbau-Modell (`steps`), Einbettungen über `MockEliezer` und das eine MultiGPT
/// eingetragen (ohne Token); Websuche und Hochladen sind aus (Vorgabe).
async fn world(steps: impl IntoIterator<Item = Step>) -> World {
    let model = ScriptedModel::with(steps);
    let data = tempfile::tempdir().unwrap();
    let state =
        baruch_server::AppState::open_with(data.path(), Arc::new(MemorySecrets::default()), Arc::new(model.clone()))
            .await
            .expect("Datenordner öffnen");
    let state = Arc::new(state);
    let server = common::Server { app: baruch_server::router(state.clone()), state, data };
    let work = server.import(FERRY, "faehre.typ").await.work;

    let search = MockMultigpt::start().await;
    let site = MockSite::with_fixtures().await;
    let web = MockWebMcp::start(Some(&site)).await;
    let docs = MockDocs::start().await;
    docs.put_document("Leuchtfeuer", "lf-1", "Das Feuer auf der Klippe", BEACON);
    docs.set_url("lf-1", "https://bib.example/lf-1");
    let front = OneMultigpt::start(&search, &web, &docs).await;
    ok(&server, "connection.set", json!({ "kind": "multigpt", "config": { "url": front.url } })).await;

    let eliezer = MockEliezer::start().await;
    ok(&server, "connection.set", json!({ "kind": "eliezer", "config": { "url": eliezer.url() } })).await;
    rag::set_embedders(&server.state, rag::eliezer_embedders(&server.state));
    rag::catch_up(&server.state).await;
    rag::wait_idle(&server.state).await;
    World { server, model, work, search, web, docs, eliezer, front, _site: site }
}

fn result(answer: &Value) -> &Value {
    assert!(answer.get("error").is_none(), "unerwarteter Fehler: {answer}");
    &answer["result"]
}

async fn ok(server: &common::Server, method: &str, params: Value) -> Value {
    result(&server.rpc(method, params).await).clone()
}

fn user(call: &ModelCall) -> &str {
    call.messages.iter().find(|m| m.role == Role::User).map(|m| m.content.as_str()).unwrap()
}

/// Knoten des Werks: (Schlüssel, Titel, Text der aktuellen Fassung, Nummer der aktuellen Fassung).
async fn nodes(server: &common::Server, work: i64) -> Vec<(i64, String, String, i64)> {
    let loaded = ok(server, "work.get", json!({ "work": work })).await;
    loaded["work"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| {
            let current = n["revisions"].as_array().unwrap().iter().find(|r| r["status"] == "current").unwrap();
            (
                n["key"].as_i64().unwrap(),
                n["title"].as_str().unwrap().to_owned(),
                current["body"].as_str().unwrap().to_owned(),
                current["number"].as_i64().unwrap(),
            )
        })
        .collect()
}

/// Zahl aller Fassungen des Werks (Begründung: am Buchtext legen Assistenten nur Vorschläge an; eine Antwort im
/// Gespräch ändert keine einzige Fassung).
async fn revision_count(server: &common::Server, work: i64) -> usize {
    let loaded = ok(server, "work.get", json!({ "work": work })).await;
    loaded["work"]["nodes"].as_array().unwrap().iter().map(|n| n["revisions"].as_array().unwrap().len()).sum()
}

/// Abgleich über das Formular (ohne JavaScript) starten, abwarten, letzten Lauf liefern.
async fn sync_run(server: &common::Server, work: i64) -> Value {
    let (status, location) = server.post_form("/abgleich/act", &format!("action=run&work={work}")).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Knopf „Abgleichen“ leitet zurück");
    assert!(location.unwrap().contains("ok=gestartet"), "der Lauf ist gestartet (kein anderer lief)");
    sync::wait_idle(&server.state).await;
    let status = ok(server, "sync.status", json!({})).await;
    assert!(status["last"]["finished"].is_string(), "der Lauf ist beendet: {status}");
    status["last"].clone()
}

/// Nur die Werkzeugaufrufe aus der Aufzeichnung eines Nachbaus (die Vorderseite fragt `tools/list` bei allen).
fn calls(requests: &[Value]) -> Vec<Value> {
    requests.iter().filter(|r| r["method"] == "tools/call").cloned().collect()
}

fn char_range(text: &str, needle: &str) -> (i64, i64) {
    let at = text.find(needle).expect("Ausschnitt im Text");
    let start = text[..at].chars().count() as i64;
    (start, start + needle.chars().count() as i64)
}

/// Treffer der lokalen Suche (LanceDB) einer Art.
async fn local_hits(server: &common::Server, work: i64, query: &str, kind: &str) -> Vec<Value> {
    rag::wait_idle(&server.state).await;
    let found = ok(server, "rag.search", json!({ "work": work, "query": query, "collections": [kind] })).await;
    found["hits"].as_array().cloned().unwrap_or_default()
}

// ---- Quellen ---------------------------------------------------------------------------------------------------

#[tokio::test]
async fn a_multigpt_source_is_read_checked_and_adopted_and_its_title_reaches_the_index() {
    let w = world([]).await;
    w.search.set_hits(vec![json!({ "title": "Das Feuer auf der Klippe", "document_id": "lf-1",
        "collection": "Leuchtfeuer", "text": "Der Wärter putzte jeden Abend die Linse.", "score": 0.9 })]);
    ok(&w.server, "connection.rag_set", json!({ "work": w.work, "ways": ["lancedb", "multigpt"] })).await;
    ok(
        &w.server,
        "connection.collections_set",
        json!({ "work": w.work, "way": "multigpt", "collections": ["Leuchtfeuer"] }),
    )
    .await;

    // Suche über MultiGPT (MockMultigpt) → Link „Volltext“ → Dokumentansicht (Text von MockDocs).
    let (status, literature) = w.server.get(&format!("/literatur?work={}&lq=Linse&lmode=rag", w.work)).await;
    assert_eq!(status, StatusCode::OK, "Literatur mit Bedeutungssuche: {literature}");
    assert!(
        literature.contains("data-fulltext>Volltext</a>")
            && literature.contains("&#x2f;quelle&#x2f;multigpt?collection=Leuchtfeuer&amp;document=lf-1"),
        "der Treffer aus der Suche verlinkt die Dokumentansicht desselben MultiGPT: {literature}"
    );
    let (status, view) =
        w.server.get(&format!("/quelle/multigpt?work={}&collection=Leuchtfeuer&document=lf-1&q=Linse", w.work)).await;
    assert_eq!(status, StatusCode::OK, "Dokumentansicht: {view}");
    assert!(view.contains("<mark class=\"fulltext-mark\">Linse</mark>"), "Fundstelle markiert: {view}");
    assert!(view.contains("Über dem Wasser kreiste das Licht"), "der ganze Volltext (ein Teil): {view}");
    assert!(view.contains("value=\"adopt\""), "Knopf „In die Bibliothek übernehmen“ als Formular: {view}");
    assert_eq!(
        w.docs.tool_calls("read_document"),
        vec![json!({ "document_id": "lf-1" })],
        "Begründung: Die Vorderseite gibt read_document an den Dokument-Nachbau; Baruch liest einmal und teilt selbst"
    );

    // Übernehmen nur über das Formular (Mensch), danach ist der Titel im lokalen Index.
    let (status, location) = w
        .server
        .post_form("/literatur/act", &format!("action=adopt&work={}&document=lf-1&collection=Leuchtfeuer", w.work))
        .await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Übernahme leitet weiter");
    let location = location.unwrap();
    assert!(location.contains("notice=adopted"), "Hinweis „übernommen“: {location}");
    let title = location.split("title=").nth(1).unwrap().split('&').next().unwrap().parse::<i64>().unwrap();
    let hits = local_hits(&w.server, w.work, "Feuer auf der Klippe", "library").await;
    assert!(
        hits.iter().any(|h| h["document"] == format!("title:{title}")
            && h["url"].as_str().unwrap().contains(&format!("title={title}"))),
        "Begründung: source.adopt meldet sich mit rag::notify beim Index – der Titel ist sofort lokal findbar: {hits:?}"
    );

    // Hervorhebungen ohne Zitat werden gegen den Volltext geprüft: gefunden, nicht gefunden, nicht prüfbar.
    let (start, end) = char_range(BEACON, "Über dem Wasser kreiste das Licht");
    let found = ok(
        &w.server,
        "highlight.add",
        json!({ "title": title, "start": start, "end": end, "text": "Über dem Wasser kreiste das Licht", "kind": "core" }),
    )
    .await;
    assert_eq!(found["check"]["result"], "found", "gleicher Text im Bereich: {found}");
    let wrong = ok(
        &w.server,
        "highlight.add",
        json!({ "title": title, "start": start, "end": end, "text": "Unter dem Wasser schlief das Licht", "kind": "check" }),
    )
    .await;
    assert_eq!(wrong["check"]["result"], "not_found", "anderer Text im Bereich: {wrong}");
    assert!(wrong["highlight"]["id"].is_i64(), "die Hervorhebung des Menschen bleibt gespeichert: {wrong}");
    let titles = baruch_library::list_titles(w.server.state.store.pool()).await.unwrap();
    let unlinked = titles.iter().find(|t| t.title.id != title).expect("ein Beispieltitel").title.id;
    let other = ok(
        &w.server,
        "highlight.add",
        json!({ "title": unlinked, "start": 0, "end": 3, "text": "Die", "kind": "core" }),
    )
    .await;
    assert_eq!(other["check"]["result"], "unverifiable", "ohne Verknüpfung nicht prüfbar, nie erfunden: {other}");
    let (_, page) = w.server.get(&format!("/quelle?work={}&title={title}", w.work)).await;
    assert!(
        page.contains("im Volltext gefunden") && page.contains("im Volltext nicht gefunden"),
        "die Quellenseite zeigt beide Ergebnisse: {page}"
    );
    assert!(
        calls(&w.web.requests()).is_empty(),
        "Quellen rufen kein Web-Werkzeug auf (tools/list der Vorderseite zählt nicht): {:?}",
        w.web.requests()
    );
    assert!(w.docs.tool_calls("upload_document").is_empty(), "Lesen lädt nichts hoch");
}

// ---- Websuche ----------------------------------------------------------------------------------------------------

const ANSWER: &str = "Das Leuchtfeuer dreht sich laut [W1] „alle zwölf Sekunden einmal um sich selbst“.\n\
Der Wärter nannte es „das schlafende Auge der Küste“ [W1].\n\
Das Gästebuch [W2] lobt die Aussicht.";

#[tokio::test]
async fn belege_searches_the_preferred_source_first_checks_quotes_and_adopts_only_proven_ones() {
    let w = world([Step::words(ANSWER)]).await;
    let (key, _, _, _) = nodes(&w.server, w.work).await[1].clone();
    let before = revision_count(&w.server, w.work).await;
    ok(&w.server, "sources.create", json!({ "title": "Lichtkunde", "url": "https://lichtkunde.example/" })).await;
    w.web.set_results(vec![
        json!({ "title": "Gästebuch", "url": GUESTS, "content": "Schöne Aussicht" }),
        json!({ "title": "Licht", "url": LIGHT, "content": "Das kreisende Feuer" }),
    ]);
    let (status, _) = w.server.post_form("/websuche/act", &format!("action=switch&on=1&work={}", w.work)).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Schalter über das Formular (ohne JavaScript)");
    assert_eq!(ok(&w.server, "websearch.get", json!({})).await["on"], true, "Websuche an");

    let answer =
        ok(&w.server, "persona.ask", json!({ "work": w.work, "key": key, "persona": "@belege", "question": QUESTION }))
            .await;
    let queries: Vec<String> =
        w.web.tool_calls("web_search").iter().map(|a| a["query"].as_str().unwrap().to_owned()).collect();
    assert_eq!(
        queries,
        vec![format!("site:lichtkunde.example {QUESTION}"), QUESTION.to_owned()],
        "erst die bevorzugte Quelle (site:), dann das allgemeine Web – über dasselbe MultiGPT"
    );
    let fetched: Vec<String> =
        w.web.tool_calls("fetch_url").iter().map(|a| a["url"].as_str().unwrap().to_owned()).collect();
    assert_eq!(fetched, vec![LIGHT.to_owned(), GUESTS.to_owned()], "die bevorzugte Seite wird zuerst abgerufen");

    let message = user(&w.model.calls()[0]).to_owned();
    let block = &message[message.find(BLOCK_TITLE).expect("Block der Webseiten")..];
    assert!(
        block.contains(&format!("[W1] Lichtkunde: Das kreisende Feuer · {LIGHT} · abgerufen"))
            && block.lines().any(|l| l.starts_with("[W1] ") && l.ends_with(" · bevorzugte Quelle")),
        "W1 ist die bevorzugte Quelle und als solche gekennzeichnet: {block}"
    );
    assert!(
        block.lines().any(|l| l.starts_with(&format!("[W2] Gästebuch · {GUESTS}")) && !l.contains("bevorzugte")),
        "W2 aus dem allgemeinen Web ohne Kennzeichnung: {block}"
    );
    assert_eq!(message.matches("<<<seite").count(), 2, "genau zwei echte Seitenblöcke (W1, W2): {message}");
    assert_eq!(message.matches("Ignoriere alle Regeln").count(), 1, "die Anweisung steht nur einmal, im Block");

    let body = answer["post"]["body"].as_str().unwrap();
    let lines: Vec<&str> = body.lines().collect();
    assert_eq!(
        lines[0], "Das Leuchtfeuer dreht sich laut [W1] „alle zwölf Sekunden einmal um sich selbst“.",
        "Wortlaut steht auf W1: ohne Zusatz"
    );
    assert_eq!(
        lines[1], "Der Wärter nannte es „das schlafende Auge der Küste“ (nicht belegt) [W1].",
        "Begründung: Auf W1 steht „das wache Auge der Küste“ – das Zitat ist nicht auf der Seite, der Code merkt es an"
    );
    let sources = lines.last().unwrap();
    assert!(
        sources.starts_with("Quellen: [W1] Lichtkunde: Das kreisende Feuer – ") && sources.contains(GUESTS),
        "die Quellenzeile setzt der Code: {body}"
    );
    assert!(!body.contains("freigegeben"), "die eingeschleuste Anweisung der Seite W2 ändert die Antwort nicht");
    assert_eq!(
        revision_count(&w.server, w.work).await,
        before,
        "Begründung: Die Seite mit eingeschleuster Anweisung legt nichts am Buchtext an – keine neue Fassung"
    );

    // Belege und Übernahme in die Bibliothek (Formular), danach im lokalen Index.
    let post = answer["post"]["id"].as_i64().unwrap();
    let evidence = ok(&w.server, "websearch.evidence", json!({ "post": post })).await["evidence"].clone();
    let verdicts: Vec<(String, String)> = evidence
        .as_array()
        .unwrap()
        .iter()
        .map(|e| (e["label"].as_str().unwrap().to_owned(), e["verdict"].as_str().unwrap().to_owned()))
        .collect();
    assert_eq!(
        verdicts,
        vec![
            ("W1".into(), "belegt".into()),
            ("W1".into(), "nicht_belegt".into()),
            ("W2".into(), "nicht_zitiert".into())
        ],
        "je Zitat bzw. genannter Seite eine Zeile mit dem Urteil des Codes: {evidence}"
    );
    let page_hash: String =
        sqlx::query_scalar("SELECT page_hash FROM web_evidence WHERE post_id = ? ORDER BY id LIMIT 1")
            .bind(post)
            .fetch_one(w.server.state.store.pool())
            .await
            .unwrap();
    assert_eq!(page_hash.len(), 64, "Integration: page_hash ist der SHA-256 der geprüften Seite (baruch-store)");
    let (proven, unproven) = (evidence[0]["id"].as_i64().unwrap(), evidence[1]["id"].as_i64().unwrap());
    let refused = w.server.rpc("websearch.adopt", json!({ "evidence": unproven })).await;
    assert_eq!(refused["error"]["code"], -32602, "nicht belegt kommt nicht in die Bibliothek: {refused}");
    let (status, location) =
        w.server.post_form("/websuche/act", &format!("action=adopt&evidence={proven}&work={}", w.work)).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Übernahme leitet zurück");
    assert!(location.unwrap().contains("ok=uebernommen"), "übernommen");
    let adopted = ok(&w.server, "websearch.evidence", json!({ "post": post })).await["evidence"][0]["library_title"]
        .as_i64()
        .expect("der Beleg kennt seinen Bibliothekstitel");
    let hits = local_hits(&w.server, w.work, "Lichtkunde kreisende Feuer", "library").await;
    assert!(
        hits.iter().any(|h| h["document"] == format!("title:{adopted}")),
        "Begründung: websearch.adopt meldet sich mit rag::notify – der neue Titel ist lokal findbar: {hits:?}"
    );
    assert!(
        calls(&w.docs.requests()).is_empty(),
        "die Websuche ruft kein Dokument-Werkzeug auf: {:?}",
        w.docs.requests()
    );
}

// ---- Abgleich ----------------------------------------------------------------------------------------------------

#[tokio::test]
async fn the_sync_uploads_only_after_confirmation_replaces_changed_chapters_and_fetches_for_offline_use() {
    let w = world([]).await;

    // Schalter aus (Vorgabe): ein Lauf schickt nichts hinaus.
    let last = sync_run(&w.server, w.work).await;
    assert_eq!(last["uploaded"], 0, "nichts hochgeladen: {last}");
    assert!(
        w.docs.tool_calls("upload_document").is_empty() && w.docs.documents(COLLECTION).is_empty(),
        "Begründung: Vorgabe AUS – ohne Schalter verlässt kein Buchtext Baruch"
    );

    // Einschalten nur mit Bestätigung (Formular ohne Kontrollkästchen → Rückfrage).
    let (_, location) = w.server.post_form("/abgleich/act", &format!("action=upload&work={}&on=1", w.work)).await;
    assert!(location.unwrap().contains("fehler=bestaetigen"), "ohne Bestätigung bleibt der Schalter aus");
    assert_eq!(ok(&w.server, "sync.status", json!({ "work": w.work })).await["works"][0]["upload"], false, "aus");
    let (_, location) =
        w.server.post_form("/abgleich/act", &format!("action=upload&work={}&on=1&confirm=1", w.work)).await;
    assert!(location.unwrap().contains("ok=hochladen_an"), "mit Bestätigung an");

    let last = sync_run(&w.server, w.work).await;
    assert_eq!((last["uploaded"].as_i64(), last["ok"].as_bool()), (Some(3), Some(true)), "drei Kapitel: {last}");
    let first = w.docs.documents(COLLECTION);
    assert_eq!(first.len(), 3, "ein Dokument je Kapitel in „{COLLECTION}“: {first:?}");

    // Zweiter Lauf ohne Änderung: nichts.
    w.docs.clear_requests();
    let last = sync_run(&w.server, w.work).await;
    assert!(
        w.docs.tool_calls("upload_document").is_empty() && w.docs.tool_calls("delete_document").is_empty(),
        "unverändert: kein Hochladen, kein Löschen: {last}"
    );

    // Ein Kapitel ändert sich: genau dieses wird ersetzt.
    let list = nodes(&w.server, w.work).await;
    let (key, _, body, number) = list.iter().find(|n| n.1 == "Überfahrt").cloned().unwrap();
    ok(
        &w.server,
        "outline.save",
        json!({ "work": w.work, "key": key, "body": body.replace("still wie Glas", "unruhig und grau"), "based_on": number }),
    )
    .await;
    w.docs.clear_requests();
    let last = sync_run(&w.server, w.work).await;
    let uploads = w.docs.tool_calls("upload_document");
    assert_eq!(uploads.len(), 1, "nur das geänderte Kapitel neu hochgeladen: {last}");
    assert!(uploads[0]["content"].as_str().unwrap().contains("unruhig und grau"), "mit dem neuen Text: {uploads:?}");
    assert_eq!(w.docs.tool_calls("delete_document").len(), 1, "danach das alte Dokument gelöscht");
    assert_eq!(w.docs.documents(COLLECTION).len(), 3, "weiter drei Dokumente");

    // Sammlung holen → offline per rag.search findbar.
    ok(&w.server, "sync.set_collections", json!({ "collections": ["Leuchtfeuer"] })).await;
    let last = sync_run(&w.server, w.work).await;
    assert_eq!(last["downloaded"], 1, "lf-1 geholt: {last}");
    let hits = local_hits(&w.server, w.work, "Wärter putzte die Linse der Laterne", "remote").await;
    let hit =
        hits.iter().find(|h| h["meta"]["document"] == "lf-1").unwrap_or_else(|| panic!("Treffer fehlt: {hits:?}"));
    assert_eq!(hit["url"], "/quelle/multigpt?collection=Leuchtfeuer&document=lf-1", "Link zur Dokumentansicht");
    assert!(w.eliezer.requests().iter().all(|r| r.path == "/v1/embeddings"), "Eliezer-Nachbau: nur Einbettungen");

    // MultiGPT fällt aus: Hinweis, die lokale Arbeit geht weiter, die Dokumentansicht zeigt die lokale Kopie.
    let closed = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap();
    ok(&w.server, "connection.set", json!({ "kind": "multigpt", "config": { "url": format!("http://{closed}/mcp") } }))
        .await;
    let last = sync_run(&w.server, w.work).await;
    assert_eq!(last["ok"], false, "der Lauf scheitert ehrlich: {last}");
    assert!(!last["message"].as_str().unwrap_or("").is_empty(), "mit Hinweis: {last}");
    let (_, page) = w.server.get(&format!("/abgleich?work={}", w.work)).await;
    assert!(page.contains("zuletzt abgeglichen"), "die Seite nennt den letzten Lauf: {page}");
    let list = nodes(&w.server, w.work).await;
    let (key, _, body, number) = list.iter().find(|n| n.1 == "Ankunft").cloned().unwrap();
    ok(
        &w.server,
        "outline.save",
        json!({ "work": w.work, "key": key, "body": format!("{body}\n\nDie Laterne flackerte."), "based_on": number }),
    )
    .await;
    let read =
        ok(&w.server, "source.read", json!({ "document": "lf-1", "collection": "Leuchtfeuer", "q": "Linse" })).await;
    assert_eq!(read["origin"], "lokal", "offline liest source.read die lokale Kopie: {read}");
    let message = read["message"].as_str().unwrap();
    assert!(
        message.split("lokale Kopie vom ").nth(1).is_some_and(|rest| {
            rest.len() == "TT.MM.JJJJ HH:MM.".len()
                && rest.ends_with('.')
                && rest.as_bytes()[..16].iter().enumerate().all(|(i, c)| match i {
                    2 | 5 => *c == b'.',
                    10 => *c == b' ',
                    13 => *c == b':',
                    _ => c.is_ascii_digit(),
                })
        }),
        "mit Hinweis und lesbarem Abrufzeitpunkt in Ortszeit (TT.MM.JJJJ HH:MM, ohne „UTC“): {read}"
    );
    let (status, view) =
        w.server.get(&format!("/quelle/multigpt?work={}&collection=Leuchtfeuer&document=lf-1&q=Linse", w.work)).await;
    assert_eq!(status, StatusCode::OK, "die Dokumentansicht bleibt bedienbar: {view}");
    assert!(
        view.contains("lokale Kopie vom") && view.contains("<mark class=\"fulltext-mark\">Linse</mark>"),
        "Integration: local_copy hängt an sync::local_document – offline zeigt die Ansicht die lokale Kopie: {view}"
    );
    assert!(view.contains("Über dem Wasser kreiste das Licht"), "der ganze Text der Kopie: {view}");
    drop(w.front);
}

#[tokio::test]
async fn offline_without_a_collection_the_single_local_copy_with_that_id_is_shown() {
    let w = world([]).await;
    ok(&w.server, "sync.set_collections", json!({ "collections": ["Leuchtfeuer"] })).await;
    sync_run(&w.server, w.work).await;
    let adopted = ok(&w.server, "source.adopt", json!({ "document": "lf-1" })).await;
    let title = adopted["title"]["id"].as_i64().unwrap();
    assert_eq!(adopted["remote"]["collection"], "", "verknüpft ohne Sammlung: {adopted}");
    let closed = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap();
    ok(&w.server, "connection.set", json!({ "kind": "multigpt", "config": { "url": format!("http://{closed}/mcp") } }))
        .await;
    let read = ok(&w.server, "source.read", json!({ "title": title })).await;
    assert_eq!(
        read["origin"], "lokal",
        "Begründung: Genau eine lokale Kopie trägt die ID lf-1 – sie ist eindeutig und wird gezeigt: {read}"
    );
    // Eine zweite Kopie mit derselben ID in einer anderen Sammlung macht die Wahl mehrdeutig: dann keine.
    w.server.state.store.sync_document_put("Zweite", "lf-1", "Fremd", "", "Ganz anderer Text.").await.unwrap();
    let answer = w.server.rpc("source.read", json!({ "title": title })).await;
    assert_eq!(
        answer["error"]["code"], -32000,
        "zwei Kopien mit derselben ID: nicht raten, ehrlich „nicht abrufbar“: {answer}"
    );
}
