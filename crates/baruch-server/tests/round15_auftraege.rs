// SPDX-License-Identifier: AGPL-3.0-or-later
//! Round 15 „auftraege“ (R15c): jobs for personas from the user interface.
//!
//! - Stream `POST /assistenten/stream`: `posted` (own post) before the first `delta`, `retry` on a newly requested
//!   answer, `compressed` with `message`/`summarized`/`chunks` and `done.compressed`.
//! - Without JavaScript: notice page (`assist_notice.html`, status 200) instead of 422, with toast building block on
//!   `compressed`; `scope` from the form reaches the RPC.
//! - Processing an idea with persona selection and visible origin; literature „an Persona geben“
//!   (`literature.to_persona`, form `action=persona`); adopting a persona highlight as one's own (`highlight.own`);
//!   badge „neu“ (server data).
//!
//! Model: `assist::mock::ScriptedModel` – no network, no Eliezer. Texts freely invented (a small book about an
//! orchard).

use crate::common;

use std::net::SocketAddr;
use std::sync::Arc;

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode, header};
use baruch_library::{HighlightAuthor, HighlightKind, HighlightStatus, NewHighlight};
use baruch_server::assist::ModelFailure;
use baruch_server::assist::mock::{ScriptedModel, Step};
use serde_json::{Value, json};
use tokio_stream::StreamExt;
use tower::ServiceExt;

const ORCHARD: &str = "#set document(title: \"Der Obstgarten\", author: \"Rita Ast\")\n\n\
= Blüte\n\nDie Kirschen blühen im April.\n\n= Ernte\n\nIm Juli sind die Körbe voll.\n";

struct Orchard {
    server: common::Server,
    model: ScriptedModel,
    work: i64,
    keys: Vec<i64>,
}

async fn orchard(steps: impl IntoIterator<Item = Step>) -> Orchard {
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
    let server = common::Server { app: common::router(state.clone()), state, data };
    let work = server.import(ORCHARD, "obstgarten.typ").await.work;
    let keys = server.state.store.load_work(work).await.unwrap().nodes.iter().map(|n| n.key).collect();
    Orchard { server, model, work, keys }
}

fn form_request(uri: &str, form: &str) -> Request<Body> {
    Request::post(uri)
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(form.to_owned()))
        .unwrap()
}

fn result(answer: &Value) -> &Value {
    assert!(answer.get("error").is_none(), "unerwarteter Fehler: {answer}");
    &answer["result"]
}

async fn posts(o: &Orchard, node: usize) -> Vec<baruch_core::Post> {
    o.server.state.store.load_work(o.work).await.unwrap().nodes[node].posts.clone()
}

// ---- Without JavaScript: notice page instead of 422 ----------------------------------------------

#[tokio::test]
async fn a_failing_persona_shows_a_notice_page_with_status_200_and_a_link_back() {
    let o = orchard([Step::Fail(ModelFailure::Unreachable("Der Modellserver schläft".into()))]).await;
    let form =
        format!("method=persona.ask&work={}&key={}&persona=%40lektor&question=Wann%3F&lq=Kirsche", o.work, o.keys[0]);
    let (status, _, page) = o.server.send(form_request("/assistenten/act", &form)).await;
    assert_eq!(status, StatusCode::OK, "Hinweisseite statt 422: {page}");
    assert!(page.contains("Der Modellserver schläft"), "die ehrliche Meldung steht auf der Seite: {page}");
    // Round 33: the header is the ribbon (header.band instead of header.topbar)
    assert!(page.contains("<header class=\"band\""), "eine echte Seite (base.html), kein nackter Text: {page}");
    assert!(
        page.contains(&format!("href=\"/?work={}&amp;key={}&amp;lq=Kirsche\"", o.work, o.keys[0])),
        "Link zurück zum Abschnitt (mit Suche): {page}"
    );
    assert!(posts(&o, 0).await.is_empty(), "Einzelauftrag gescheitert: nichts gespeichert");
}

#[tokio::test]
async fn a_chat_with_a_failing_persona_says_what_was_saved_on_the_notice_page() {
    let o = orchard([Step::Fail(ModelFailure::Busy("voll".into()))]).await;
    let form = format!("method=persona.chat&work={}&key={}&message=%40lektor+hallo", o.work, o.keys[1]);
    let (status, _, page) = o.server.send(form_request("/assistenten/act", &form)).await;
    assert_eq!(status, StatusCode::OK, "Hinweisseite statt 422: {page}");
    assert!(page.contains("Dein Beitrag und die fertigen Antworten sind gespeichert"), "was gespeichert ist: {page}");
    assert!(page.contains("@lektor: Das Modell ist ausgelastet: voll"), "je gescheiterter Persona eine Zeile: {page}");
    let stored = posts(&o, 1).await;
    assert_eq!(stored.len(), 1, "der Beitrag des Menschen bleibt");
    assert_eq!(stored[0].author, "Autor", "Autor des Beitrags ist der Mensch");
}

#[tokio::test]
async fn a_compressed_result_without_javascript_shows_the_notice_page_with_the_toast() {
    baruch_server::i18n::scope(baruch_server::i18n::Lang::De, async {
    // Without „compressed“ (today in the branch without chunking) the form redirects as before; the notice page with
    // toast building block is checked by the building block itself (rendering with a result that carries compressed).
    let compressed = json!({ "message": "Komprimiert: Teile 2 und 3 nur zusammengefasst.", "summarized": ["== Ernte"], "chunks": 2 });
    let response =
        baruch_server::assist::routes::notice_page("Die Antwort ist gespeichert.", "/?work=1&key=2", Some(&compressed));
    assert_eq!(response.status(), StatusCode::OK, "Hinweisseite mit Status 200");
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let page = String::from_utf8(body.to_vec()).unwrap();
    assert!(page.contains("toast toast-warn") && page.contains("data-widget=\"toast\""), "Toast-Baustein: {page}");
    assert!(page.contains("Komprimiert: Teile 2 und 3 nur zusammengefasst."), "Meldung im Toast: {page}");
    assert!(page.contains("== Ernte"), "die nur zusammengefasst gesehenen Teile: {page}");
    assert!(page.contains("href=\"/?work=1&amp;key=2\""), "Link weiter zum Abschnitt: {page}");
    assert!(page.contains("Weiter zum Abschnitt"), "„weiter“ statt „zurück“: {page}");
    let unsafe_back = baruch_server::assist::routes::notice_page("x", "//fremd.example/\"><b>", None);
    let body = axum::body::to_bytes(unsafe_back.into_body(), usize::MAX).await.unwrap();
    let page = String::from_utf8(body.to_vec()).unwrap();
    assert!(!page.contains("fremd.example"), "fremde oder unsichere Rücksprünge werden nicht verlinkt: {page}");
})
    .await
}

// ---- Processing an idea -------------------------------------------------------------------------

async fn mcp_tool(o: &Orchard, name: &str, arguments: Value) -> Value {
    let init = json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": { "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": { "name": "Probe", "version": "1" } } });
    let mut request = Request::post("/mcp")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(init.to_string()))
        .unwrap();
    request.extensions_mut().insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 40000))));
    let (_, headers, _) = o.server.send(request).await;
    let session = headers.get("mcp-session-id").unwrap().to_str().unwrap().to_owned();
    let call = json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": { "name": name, "arguments": arguments } });
    let mut request = Request::post("/mcp")
        .header(header::CONTENT_TYPE, "application/json")
        .header("mcp-session-id", session)
        .body(Body::from(call.to_string()))
        .unwrap();
    request.extensions_mut().insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 40000))));
    let (_, _, body) = o.server.send(request).await;
    serde_json::from_str::<Value>(&body).unwrap()["result"].clone()
}

#[tokio::test]
async fn processing_an_idea_offers_a_persona_choice_without_javascript_and_shows_its_origin() {
    let o =
        orchard([Step::words("Kritisch: Der April ist zu früh für Kirschen in den Bergen.\n- Schritt: prüfen.")]).await;
    let created = mcp_tool(
        &o,
        "idea_create",
        json!({ "work": o.work, "key": o.keys[0], "words": ["Kirsche"], "note": "Blüte später?", "persona": "@belege" }),
    )
    .await;
    let id = created["structuredContent"]["idea"]["id"].as_i64().unwrap_or_else(|| panic!("Einfall: {created}"));
    let (_, page) = o.server.get(&format!("/einfaelle/{id}")).await;
    assert!(page.contains("<select name=\"persona\">"), "Persona-Auswahl im Formular „Verarbeiten“: {page}");
    assert!(page.contains("<option value=\"@lektor\" selected>"), "Vorgabe ist der Lektor des Werks: {page}");

    let form = format!("method=idea.process&id={id}&as=persona&key={}&persona=%40exeget", o.keys[0]);
    let (status, _) = o.server.post_form("/einfaelle/act", &form).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "verarbeitet, Weiterleitung");
    let call = o.model.calls().pop().expect("Auftrag an die gewählte Persona");
    assert_eq!(call.persona, "@exeget", "die gewählte Persona antwortet, nicht der Lektor");
    let asked: String = call.messages.iter().map(|m| m.content.clone()).collect();
    assert!(asked.contains("Einfall von @belege"), "die Persona erfährt die Herkunft des Einfalls: {asked}");
    let stored = posts(&o, 0).await;
    assert_eq!(stored.last().unwrap().author, "@exeget", "Antwort der gewählten Persona");
    assert_eq!(stored.last().unwrap().sources, [format!("Einfall {id} von @belege")], "Herkunft als Quelle sichtbar");

    let second = mcp_tool(&o, "idea_create", json!({ "work": o.work, "key": o.keys[1], "words": ["Korb"] })).await;
    let other = second["structuredContent"]["idea"]["id"].as_i64().unwrap();
    let (status, _) = o
        .server
        .post_form("/einfaelle/act", &format!("method=idea.process&id={other}&as=post&key={}", o.keys[1]))
        .await;
    assert_eq!(status, StatusCode::SEE_OTHER, "als Beitrag verarbeitet");
    let post = posts(&o, 1).await.pop().unwrap();
    assert_eq!(post.author, "Autor", "der Mensch verarbeitet, er bleibt Autor");
    assert_eq!(post.sources, [format!("Einfall {other} von @mcp:probe")], "Herkunft MCP-Client im Beitrag sichtbar");
    let (_, workspace) = o.server.get(&format!("/?work={}&key={}", o.work, o.keys[1])).await;
    assert!(workspace.contains(&format!("Quelle: Einfall {other} von @mcp:probe")), "sichtbar im Chat: {workspace}");
}

// ---- Literature: giving to a persona, adopting a highlight ----------------------------------------

#[tokio::test]
async fn literature_to_persona_posts_the_answer_with_the_source_and_changes_no_text() {
    let o = orchard([
        Step::words("Kritisch: Das Zitat stützt den April.\n- Schritt: prüfen."),
        Step::words("Kritisch: Formular.\n- Schritt: prüfen."),
    ])
    .await;
    let answer = o
        .server
        .rpc(
            "literature.to_persona",
            json!({ "work": o.work, "key": o.keys[0], "persona": "@lektor", "text": "Bäume blühen, wenn es warm wird.",
                    "source": "@gartenkunde, S. 12", "question": "Passt das?" }),
        )
        .await;
    let r = result(&answer);
    assert_eq!(r["post"]["author"], "@lektor", "Antwort der gewählten Persona: {r}");
    let stored = posts(&o, 0).await;
    assert_eq!(stored.len(), 2, "Frage des Menschen und Antwort");
    assert!(
        stored[0].body.contains("Bäume blühen, wenn es warm wird.") && stored[0].body.contains("@gartenkunde, S. 12"),
        "die Frage nennt Text und Quelle: {}",
        stored[0].body
    );
    assert_eq!(stored[1].sources, ["@gartenkunde, S. 12"], "Quelle am Beitrag der Persona");
    let node = o.server.state.store.load_work(o.work).await.unwrap().nodes[0].clone();
    assert_eq!(node.revisions.len(), 1, "nichts am Buchtext");

    let missing = o.server.rpc("literature.to_persona", json!({ "work": o.work, "text": "x", "source": "y" })).await;
    assert_eq!(missing["error"]["code"], -32602, "ohne key abgelehnt: {missing}");
    assert!(missing["error"]["message"].as_str().unwrap().contains("Abschnitt wählen"), "Hinweis: {missing}");
    let long = "x".repeat(4001);
    let too_long =
        o.server.rpc("literature.to_persona", json!({ "work": o.work, "key": o.keys[0], "text": long })).await;
    assert_eq!(too_long["error"]["code"], -32602, "mehr als 4000 Zeichen abgelehnt: {too_long}");
    let two_lines = o
        .server
        .rpc(
            "literature.to_persona",
            json!({ "work": o.work, "key": o.keys[0], "text": "x", "source": "a".repeat(301) }),
        )
        .await;
    assert_eq!(two_lines["error"]["code"], -32602, "Quelle über 300 Zeichen abgelehnt: {two_lines}");

    let title = baruch_library::list_titles(o.server.state.store.pool()).await.unwrap()[0].title.clone();
    let form = format!(
        "action=persona&work={}&key={}&title={}&text=Gr%C3%BCn&locator=S.+3&persona=%40lektor",
        o.work, o.keys[1], title.id
    );
    let (status, location) = o.server.post_form("/literatur/act", &form).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Formular ohne JavaScript: Weiterleitung zum Abschnitt");
    assert_eq!(location.as_deref(), Some(format!("/?work={}&key={}", o.work, o.keys[1]).as_str()), "zum Abschnitt");
    let answer = posts(&o, 1).await.pop().unwrap();
    assert_eq!(answer.sources, [format!("@{}, S. 3", title.key)], "Quelle aus Titel und Fundstelle");
}

#[tokio::test]
async fn owning_a_persona_highlight_makes_an_active_copy_and_the_original_stays_a_proposal() {
    let o = orchard([]).await;
    let pool = o.server.state.store.pool();
    let title = baruch_library::list_titles(pool).await.unwrap()[0].title.clone();
    let proposed = baruch_library::add_highlight(
        pool,
        &NewHighlight {
            title_id: title.id,
            quote_id: None,
            locator: "S. 7".into(),
            start: None,
            end: None,
            text: "Geduld trägt Frucht.".into(),
            kind: HighlightKind::Core,
            note: String::new(),
            author: HighlightAuthor::Persona { handle: "@belege".into(), model: "probe".into() },
        },
    )
    .await
    .unwrap();
    let owned = o.server.rpc("highlight.own", json!({ "work": o.work, "highlight": proposed })).await;
    let r = result(&owned);
    assert_eq!(r["highlight"]["status"], "active", "die eigene Kopie ist aktiv: {r}");
    assert_eq!(r["highlight"]["author"]["kind"], "human", "Autor der Kopie ist der Mensch: {r}");
    assert_eq!(r["highlight"]["text"], "Geduld trägt Frucht.", "Text übernommen");
    assert_eq!(r["highlight"]["locator"], "S. 7", "Fundstelle übernommen");
    assert_eq!(r["original"]["status"], "proposed", "das Original bleibt Vorschlag: {r}");
    let original = baruch_library::get_highlight(pool, proposed).await.unwrap();
    assert_eq!(original.status, HighlightStatus::Proposed, "auch in der Datenbank");

    let again = o.server.rpc("highlight.own", json!({ "work": o.work, "highlight": proposed })).await;
    assert_eq!(again["error"]["code"], -32000, "zweimal übernehmen wird abgelehnt: {again}");
    let own_id = r["highlight"]["id"].as_i64().unwrap();
    let mine = o.server.rpc("highlight.own", json!({ "work": o.work, "highlight": own_id })).await;
    assert_eq!(mine["error"]["code"], -32000, "eine eigene Hervorhebung ist schon eigen: {mine}");

    let (_, page) = o.server.get(&format!("/quelle?title={}&work={}&key={}", title.id, o.work, o.keys[0])).await;
    assert!(page.contains("name=\"action\" value=\"own\""), "Knopf „Als eigene übernehmen“ ohne JavaScript: {page}");
    let tools = baruch_server::mcp::tools::list();
    let names: Vec<&str> = tools["tools"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap()).collect();
    assert!(!names.iter().any(|n| n.contains("own") || n.starts_with("highlight")), "nicht über MCP: {names:?}");

    let second = baruch_library::add_highlight(
        pool,
        &NewHighlight {
            title_id: title.id,
            quote_id: None,
            locator: String::new(),
            start: None,
            end: None,
            text: "Zweiter Vorschlag.".into(),
            kind: HighlightKind::Check,
            note: String::new(),
            author: HighlightAuthor::Persona { handle: "@lektor".into(), model: "probe".into() },
        },
    )
    .await
    .unwrap();
    let (status, location) = o
        .server
        .post_form("/literatur/act", &format!("action=own&work={}&key={}&highlight={second}", o.work, o.keys[0]))
        .await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Formular ohne JavaScript: Weiterleitung");
    assert!(location.unwrap().contains("notice=owned"), "Hinweis „übernommen“");
}

// ---- Badge „neu“ (server data) ------------------------------------------------------------------

#[tokio::test]
async fn the_chat_marks_own_posts_and_links_the_threads_for_the_new_badge() {
    let o = orchard([Step::words("Kritisch: Antwort.\n- Schritt: prüfen.")]).await;
    let answer =
        o.server.rpc("persona.chat", json!({ "work": o.work, "key": o.keys[0], "message": "@lektor Frage" })).await;
    result(&answer);
    let (_, page) = o.server.get(&format!("/?work={}&key={}", o.work, o.keys[0])).await;
    let own = page.matches("data-post-own").count();
    assert_eq!(own, 1, "genau der Beitrag des Menschen trägt data-post-own (zählt nie als neu): {page}");
    assert!(page.contains(&format!("href=\"/faeden?work={}\"", o.work)), "Link „Fäden ohne Abschnitt“: {page}");
    let script = include_str!("../web/src/chat.ts");
    assert!(script.contains("data-post-own"), "das Widget liest data-post-own");
    assert!(!script.contains("new Date()"), "keine Uhrzeit des Browsers mehr (leerer Chat merkt „leer“)");
}

// ---- Stream (SSE) -------------------------------------------------------------------------------

/// Split an SSE text into (event, data).
fn events(text: &str) -> Vec<(String, Value)> {
    text.replace("\r\n", "\n")
        .split("\n\n")
        .filter_map(|block| {
            let mut name = String::from("message");
            let mut data = String::new();
            for line in block.lines() {
                if let Some(v) = line.strip_prefix("event:") {
                    name = v.trim().to_owned();
                } else if let Some(v) = line.strip_prefix("data:") {
                    data.push_str(v.trim_start());
                }
            }
            (!data.is_empty()).then(|| (name, serde_json::from_str(&data).unwrap_or(Value::Null)))
        })
        .collect()
}

async fn stream(o: &Orchard, body: Value) -> Vec<(String, Value)> {
    let request = Request::post("/assistenten/stream")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    let response = o.server.app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK, "Strom angenommen");
    let mut body = response.into_body().into_data_stream();
    let mut text = String::new();
    while let Some(chunk) = body.next().await {
        text.push_str(std::str::from_utf8(&chunk.unwrap()).unwrap());
    }
    events(&text)
}

#[tokio::test]
async fn the_chat_stream_shows_the_own_post_before_the_first_delta_and_a_retry() {
    let o = orchard([
        Step::words("Gerne helfe ich dir dabei."),
        Step::words("Kritisch: Der April passt.\n- Schritt: prüfen."),
    ])
    .await;
    let got = stream(
        &o,
        json!({ "method": "persona.chat", "work": o.work, "key": o.keys[0], "message": "@lektor Passt der April?" }),
    )
    .await;
    let names: Vec<&str> = got.iter().map(|(n, _)| n.as_str()).collect();
    let posted = names.iter().position(|n| *n == "posted").unwrap_or_else(|| panic!("posted fehlt: {names:?}"));
    let first_delta = names.iter().position(|n| *n == "delta").unwrap_or_else(|| panic!("delta fehlt: {names:?}"));
    assert!(posted < first_delta, "der eigene Beitrag kommt vor dem ersten Stück der Antwort: {names:?}");
    assert_eq!(got[posted].1["post"]["author"], "Autor", "posted trägt den gespeicherten Beitrag: {got:?}");
    assert_eq!(got[posted].1["post"]["body"], "@lektor Passt der April?", "Text des Beitrags");
    assert!(got[posted].1["post"]["id"].is_number(), "mit id (gespeichert)");
    let retry = got.iter().find(|(n, _)| n == "retry").unwrap_or_else(|| panic!("retry fehlt: {names:?}"));
    assert_eq!(retry.1["persona"], "@lektor", "retry nennt die Persona: {retry:?}");
    assert!(retry.1["reason"].as_str().is_some_and(|r| !r.is_empty()), "retry nennt den Grund: {retry:?}");
    let done = got.iter().find(|(n, _)| n == "done").unwrap_or_else(|| panic!("done fehlt: {names:?}"));
    assert!(done.1.get("compressed").is_some(), "done trägt compressed (null oder Objekt): {done:?}");
    let shown: String =
        got.iter().filter(|(n, _)| n == "delta").map(|(_, d)| d["text"].as_str().unwrap_or("")).collect();
    assert!(!shown.contains("Gerne"), "die verworfene Einleitung erreicht den Aufrufer nicht: {shown}");
}

#[tokio::test]
async fn the_widget_handles_posted_retry_compressed_and_cancel() {
    let script = include_str!("../web/src/assist.ts");
    for event in ["\"posted\"", "\"retry\"", "\"compressed\""] {
        assert!(script.contains(event), "assist.ts behandelt das Ereignis {event}");
    }
    assert!(script.contains("\"baruch:toast\""), "compressed geht als baruch:toast an toast.ts (Vertrag §4)");
    let de = |key: &str| baruch_server::i18n::t(baruch_server::i18n::Lang::De, key, &[]);
    assert!(
        script.contains("\"widget.assist.cancelled_chat\"")
            && de("widget.assist.cancelled_chat").contains("Dein Beitrag und schon fertige Antworten sind gespeichert"),
        "Abbrechen im Chat ehrlich"
    );
    assert!(
        script.contains("\"widget.assist.cancelled_nothing\"")
            && de("widget.assist.cancelled_nothing") == "Abgebrochen; es wurde nichts gespeichert.",
        "Abbrechen eines Einzelauftrags ehrlich"
    );
}
