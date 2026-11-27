// SPDX-License-Identifier: AGPL-3.0-or-later
//! Round 10 (B2), end to end after the merge: personas talk through the adapter `assist::eliezer::EliezerPort` and
//! `crate::models` to a **mocked** Eliezer (`baruch_llm::mock::MockEliezer` on 127.0.0.1:0, address through
//! `connection.set`). No test names or reaches the real model server; secrets lie only in `MemorySecrets`.
//!
//! Covered: lector fixed @lektor through the form, @schrift with fact block and „Wortlaut: ungeprüft“, proposal only
//! `proposed`, cleaned introduction „Als Assistent …“, stream word by word, model selection from `/v1/models`, MCP with
//! a token from another machine (`ask_persona`, `scripture_check`) and exactly one request at a time at the mock. All
//! texts are freely invented (a small bread book).

use crate::common;

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode, header};
use baruch_llm::mock::{MockEliezer, Reply};
use baruch_server::assist::eliezer::EliezerPort;
use baruch_server::connections::MemorySecrets;
use serde_json::{Value, json};

const BREAD: &str = "#set document(title: \"Das Brotbuch\", author: \"Lena Kruste\")\n#set heading(numbering: \"1.1\")\n\n\
= Sauerteig\n\nDer Teig ruht über Nacht.\n\n\
== Anstellgut\n\nWie es in Joh 3,16 und Ps 151,2 heißt, braucht Geduld Zeit.\n\n\
== Backen\n\nDer Ofen wird auf 250 Grad vorgeheizt.\n";

struct Bakery {
    server: common::Server,
    mock: MockEliezer,
    work: i64,
    /// Keys of Sauerteig, Anstellgut, Backen.
    keys: Vec<i64>,
}

/// Server with the real adapter (EliezerPort) and an own mock of Eliezer.
async fn bakery() -> Bakery {
    let data = tempfile::tempdir().unwrap();
    let state =
        baruch_server::AppState::open_with(data.path(), Arc::new(MemorySecrets::default()), Arc::new(EliezerPort))
            .await
            .expect("Datenordner öffnen");
    let state = Arc::new(state);
    let server = common::Server { app: baruch_server::router(state.clone()), state, data };
    let mock = MockEliezer::start().await;
    mock.set_models(&["google/gemma-4-26b-a4b", "qwen3-4b-q4-k-m", "llama-3.2-3b-instruct-q4-k-m"]);
    let set = server.rpc("connection.set", json!({ "kind": "eliezer", "config": { "url": mock.url() } })).await;
    assert!(set["error"].is_null(), "Adresse des Nachbaus eingestellt: {set}");
    let report = server.import(BREAD, "brot.typ").await;
    let work = report.work;
    let keys = server.state.store.load_work(work).await.unwrap().nodes.iter().map(|n| n.key).collect();
    Bakery { server, mock, work, keys }
}

fn result(answer: &Value) -> &Value {
    assert!(answer.get("error").is_none(), "unerwarteter Fehler: {answer}");
    &answer["result"]
}

async fn posts(b: &Bakery, node: usize) -> Vec<baruch_core::Post> {
    b.server.state.store.load_work(b.work).await.unwrap().nodes[node].posts.clone()
}

/// Content of the message with role `role` in the n-th chat call at the mock.
fn message(body: &Value, role: &str) -> String {
    body["messages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["role"] == role)
        .and_then(|m| m["content"].as_str())
        .unwrap_or_default()
        .to_owned()
}

#[tokio::test]
async fn the_fixed_lector_answers_a_form_through_eliezer_and_the_answer_becomes_a_post() {
    let _turn = common::model_queue(module_path!()).await;
    let b = bakery().await;
    b.mock.push(Reply::text(&[
        "Kritisch: ",
        "„ruht über Nacht“ ",
        "nennt keine Temperatur.",
        "\n- Schritt: Temperatur nennen.",
    ]));
    let lector = result(&b.server.rpc("persona.lector_get", json!({})).await).clone();
    assert_eq!(
        (lector["mode"].as_str(), lector["lector"].as_str()),
        (Some("fixed"), Some("@lektor")),
        "Versuch seit 2026-11-06: ohne Einstellung ist der Lektor fest @lektor: {lector}"
    );
    // Form „Prüfen“ without JavaScript and without persona: the fixed lector answers.
    let form = format!("method=persona.review&work={}&key={}", b.work, b.keys[0]);
    let (status, location) = b.server.post_form("/assistenten/act", &form).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "nach dem Modellaufruf 303 (ohne JavaScript bedienbar)");
    assert_eq!(
        location.as_deref(),
        Some(format!("/?work={}&key={}", b.work, b.keys[0]).as_str()),
        "zurück auf die Key-Adresse des Abschnitts"
    );
    let stored = posts(&b, 0).await;
    assert_eq!(stored.len(), 1, "genau ein Beitrag mit den Befunden: {stored:?}");
    assert_eq!(stored[0].author, "@lektor", "die Befunde stehen als Beitrag des festen Lektors");
    assert_eq!(
        stored[0].body, "Kritisch: „ruht über Nacht“ nennt keine Temperatur.\n- Schritt: Temperatur nennen.",
        "gestreamte Stücke zusammengesetzt"
    );
    let bodies = b.mock.chat_bodies();
    assert_eq!(bodies.len(), 1, "genau ein Aufruf beim Nachbau");
    assert_eq!(bodies[0]["model"], "google/gemma-4-26b-a4b", "Modell des Lektors geht an Eliezer");
    assert_eq!(bodies[0]["stream"], true, "Antwort wird gestreamt");
    assert!(message(&bodies[0], "user").contains("Der Teig ruht über Nacht."), "Abschnittstext im Kontext");
    assert!(
        b.mock.context_fields().is_empty(),
        "Baruch fordert nie eine Kontextlänge an: {:?}",
        b.mock.context_fields()
    );
    assert!(
        b.mock.forbidden_paths().is_empty(),
        "nie /api/v1/models/load|unload|estimate: {:?}",
        b.mock.forbidden_paths()
    );
}

#[tokio::test]
async fn scripture_gets_the_fact_block_and_its_post_says_the_wording_is_unchecked() {
    let _turn = common::model_queue(module_path!()).await;
    let b = bakery().await;
    b.mock.push(Reply::text(&["Joh 3,16 ist zugeordnet. ", "Ps 151,2 gibt es nicht."]));
    let answer = b
        .server
        .rpc(
            "persona.ask",
            json!({ "work": b.work, "key": b.keys[1], "persona": "@schrift", "question": "Stimmen die Stellen?" }),
        )
        .await;
    let r = result(&answer);
    assert_eq!(r["post"]["author"], "@schrift", "{r}");
    let body = r["post"]["body"].as_str().unwrap();
    assert!(body.contains("Wortlaut: ungeprüft"), "Schrift bestätigt nie den Wortlaut: {body}");
    assert_eq!(r["model"], "mock-modell", "tatsächliches Modell laut Antwort des Nachbaus, nicht das angefragte: {r}");
    let bodies = b.mock.chat_bodies();
    assert_eq!(bodies[0]["model"], "qwen3-4b-q4-k-m", "alter Name qwen3:4b vor dem Senden aufgelöst");
    let system = message(&bodies[0], "system");
    assert!(
        system.contains("Stellenprüfung (Baruch, Verszählung Schlachter 1951"),
        "Faktenblock im System-Prompt: {system}"
    );
    assert!(system.contains("→ Joh 3,16: vorhanden"), "gültige Stelle erkannt: {system}");
    assert!(system.contains("150 Kapitel"), "ungültige Stelle mit Maximum gemeldet: {system}");
    assert!(system.contains("Bestätige keinen Wortlaut."), "Schlusssatz zum Wortlaut: {system}");

    // Other personas get no fact block, but the hint if they name a passage.
    b.mock.push(Reply::text(&["Kritisch: Joh 3,16 passt hier nicht zum Brot.\n- Schritt: Stelle streichen."]));
    let answer = b
        .server
        .rpc("persona.ask", json!({ "work": b.work, "key": b.keys[1], "persona": "@lektor", "question": "Und?" }))
        .await;
    let body = result(&answer)["post"]["body"].as_str().unwrap().to_owned();
    assert!(body.contains("prüft @schrift"), "Hinweis auf @schrift bei anderen Personas: {body}");
    let system = message(&b.mock.chat_bodies()[1], "system");
    assert!(!system.contains("Stellenprüfung (Baruch"), "der Lektor bekommt keinen Faktenblock");
}

#[tokio::test]
async fn a_proposal_through_eliezer_is_only_proposed_and_a_forbidden_opening_is_removed() {
    let _turn = common::model_queue(module_path!()).await;
    let b = bakery().await;
    b.mock.push(Reply::text(&["```\n= Backen\n\nDer Ofen wird auf 240 Grad vorgeheizt.\n```"]));
    let answer = b
        .server
        .rpc(
            "persona.propose",
            json!({ "work": b.work, "key": b.keys[2], "persona": "@lektor", "instruction": "Temperatur senken" }),
        )
        .await;
    let r = result(&answer);
    assert_eq!(r["status"], "proposed", "nur ein Vorschlag: {r}");
    let node = b.server.state.store.load_work(b.work).await.unwrap().nodes[2].clone();
    assert_eq!(node.current_number(), Some(1), "die aktuelle Fassung bleibt die des Menschen");
    let added = node.revisions.iter().find(|rev| rev.number == 2).expect("Vorschlag gespeichert");
    assert_eq!(added.status, baruch_core::RevisionStatus::Proposed, "Assistenten schreiben nur proposed");
    assert!(
        added.body.starts_with("== Backen\n"),
        "Überschriftszeile der gezeigten Fassung erzwungen: {:?}",
        added.body
    );
    assert!(!added.body.contains("```"), "Codeblock entfernt: {:?}", added.body);

    // „Als Assistent …“ twice: exactly one second call, then the first sentence is dropped.
    b.mock.push(Reply::text(&[
        "Als Assistent ",
        "finde ich das gut. ",
        "Kritisch: „250 Grad“ ist sehr heiß.\n- statt: „250 Grad“\n- besser: „220 Grad“",
    ]));
    b.mock.push(Reply::text(&[
        "Als Assistent finde ich das gut. ",
        "Kritisch: „250 Grad“ ist sehr heiß.\n- statt: „250 Grad“\n- besser: „220 Grad“",
    ]));
    let answer = b
        .server
        .rpc("persona.ask", json!({ "work": b.work, "key": b.keys[2], "persona": "@lektor", "question": "Heiß?" }))
        .await;
    let body = result(&answer)["post"]["body"].as_str().unwrap().to_owned();
    assert_eq!(
        body, "Kritisch: „250 Grad“ ist sehr heiß.\n- statt: „250 Grad“\n- besser: „220 Grad“",
        "verbotene Einleitung entfernt"
    );
    assert_eq!(b.mock.chat_bodies().len(), 3, "Vorschlag + Frage + genau eine Wiederholung");
}

#[tokio::test]
async fn the_stream_delivers_eliezer_word_by_word_and_saves_once() {
    let _turn = common::model_queue(module_path!()).await;
    let b = bakery().await;
    let pieces = ["Kritisch: ", "Die ", "Nacht ", "ist ", "lang.", "\n- Schritt: kürzen."];
    b.mock.push(Reply::text(&pieces));
    let request = Request::post("/assistenten/stream")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({ "method": "persona.ask", "work": b.work, "key": b.keys[0], "persona": "@lektor", "question": "Wie lange?" })
                .to_string(),
        ))
        .unwrap();
    let (status, headers, text) = b.server.send(request).await;
    assert_eq!(status, StatusCode::OK, "{text}");
    assert!(headers[header::CONTENT_TYPE].to_str().unwrap().starts_with("text/event-stream"), "SSE");
    let deltas = text.matches("event: delta").count();
    assert!(deltas >= 2, "mehrere Deltas, nicht ein Block am Ende: {text}");
    assert!(text.contains("event: start") && text.contains("event: done"), "start … done: {text}");
    assert_eq!(posts(&b, 0).await.last().unwrap().body, pieces.concat(), "genau die gestreamte Antwort gespeichert");
}

#[tokio::test]
async fn the_models_come_from_eliezer_in_rpc_and_on_the_settings_page() {
    let _turn = common::model_queue(module_path!()).await;
    let b = bakery().await;
    let listed = result(&b.server.rpc("model.list", json!({})).await).clone();
    assert_eq!(listed["live"], true, "eben abgefragt: {listed}");
    assert!(
        listed["models"].as_array().unwrap().iter().any(|m| m == "google/gemma-4-26b-a4b"),
        "Liste aus GET /v1/models über /rpc (Dispatch des Integrators): {listed}"
    );
    let status = result(&b.server.rpc("model.status", json!({})).await).clone();
    // The queue holds process-wide (other tests of this file run at the same time): check only the shape.
    assert!(status["waiting"].as_u64().is_some_and(|w| w <= 8), "Zahl der Wartenden, höchstens acht: {status}");
    let (code, page) = b.server.get("/einstellungen/assistenten").await;
    assert_eq!(code, StatusCode::OK, "Einstellungsseite Assistenten lädt mit der Modellliste des Nachbaus");
    assert!(page.contains("<datalist id=\"baruch-models\">"), "Modellauswahl als datalist");
    assert!(page.contains("<option value=\"qwen3-4b-q4-k-m\">"), "Modelle des Nachbaus in der Auswahl");
    assert!(page.contains("unbekannt bei Eliezer"), "@nathan (gpt-4.1-mini) ist beim Nachbau unbekannt");
    // Human only: through /act there is no model.*.
    let (status, _) = b.server.post_form("/act", "method=model.probe&model=google%2Fgemma-4-26b-a4b").await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "model.probe nicht über /act");
}

#[tokio::test]
async fn a_token_from_another_host_asks_a_persona_and_checks_scripture_over_mcp() {
    let _turn = common::model_queue(module_path!()).await;
    let b = bakery().await;
    let made = result(&b.server.rpc("mcp_token.create", json!({ "name": "Fremder Rechner" })).await).clone();
    let secret = made["secret"].as_str().unwrap().to_owned();
    assert!(secret.starts_with("baruch_"), "Token-Präfix");
    let listed = result(&b.server.rpc("mcp_token.list", json!({})).await).to_string();
    assert!(!listed.contains(&secret), "die Liste zeigt den Klartext nie");
    b.mock.push(Reply::text(&["Kritisch: ", "„250 Grad“ ohne Backzeit.", "\n- Schritt: Backzeit nennen."]));
    let call = |name: &str, arguments: Value| {
        let mut request = Request::post("/mcp")
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::AUTHORIZATION, format!("Bearer {secret}"))
            .body(Body::from(
                json!({ "jsonrpc": "2.0", "id": 7, "method": "tools/call",
                        "params": { "name": name, "arguments": arguments } })
                .to_string(),
            ))
            .unwrap();
        request.extensions_mut().insert(ConnectInfo(SocketAddr::from(([192, 0, 2, 44], 50_000))));
        request
    };
    let asked = call(
        "ask_persona",
        json!({ "work": b.work, "key": b.keys[2], "persona": "@lektor", "question": "Was fehlt?" }),
    );
    let (status, _, text) = b.server.send(asked).await;
    assert_eq!(status, StatusCode::OK, "Token gilt von 192.0.2.44: {text}");
    let answer: Value = serde_json::from_str(&text).unwrap();
    let r = &answer["result"]["structuredContent"];
    assert_eq!(answer["result"]["isError"], false, "{text}");
    assert_eq!((r["persona"].as_str(), r["mode"].as_str()), (Some("@lektor"), Some("ask")), "{r}");
    let stored = posts(&b, 2).await;
    let authors: Vec<&str> = stored.iter().map(|p| p.author.as_str()).collect();
    assert_eq!(authors, ["@mcp:fremder-rechner", "@lektor"], "Frage des Clients (Token-Name), Antwort der Persona");

    let (_, _, text) = b.server.send(call("scripture_check", json!({ "text": "Siehe Joh 3,16 und Ps 151,2." }))).await;
    let answer: Value = serde_json::from_str(&text).unwrap();
    let refs = answer["result"]["structuredContent"]["refs"].as_array().unwrap().clone();
    let statuses: Vec<&str> = refs.iter().map(|r| r["status"].as_str().unwrap()).collect();
    assert_eq!(statuses, ["valid", "no_such_chapter"], "gültig und ungültig erkannt: {refs:?}");
    assert!(refs.iter().all(|r| r["wording"] == "ungeprüft"), "Wortlaut bleibt ungeprüft: {refs:?}");

    // From another machine there is only /mcp.
    let mut page = Request::get("/rpc").body(Body::empty()).unwrap();
    page.extensions_mut().insert(ConnectInfo(SocketAddr::from(([192, 0, 2, 44], 50_001))));
    let (status, ..) = b.server.send(page).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "remote_only_mcp ist im Router verdrahtet");
}

#[tokio::test]
async fn two_requests_at_the_same_time_reach_the_mock_one_after_the_other() {
    let _turn = common::model_queue(module_path!()).await;
    let b = bakery().await;
    for _ in 0..2 {
        b.mock.push(Reply::text_from(
            "google/gemma-4-26b-a4b",
            &["Kritisch: ", "eins ", "zwei ", "drei.", "\n- Schritt: zählen."],
            Duration::from_millis(40),
        ));
    }
    let ask = |question: &str| {
        let app = b.server.app.clone();
        let params = json!({ "work": b.work, "key": b.keys[0], "persona": "@lektor", "question": question });
        tokio::spawn(async move { common::rpc(app, "persona.ask", params).await })
    };
    let (first, second) = (ask("Erste Frage?"), ask("Zweite Frage?"));
    let (first, second) = (first.await.unwrap(), second.await.unwrap());
    assert_eq!(result(&first)["post"]["author"], "@lektor", "erste Frage: Antwort des Lektors gespeichert");
    assert_eq!(result(&second)["post"]["author"], "@lektor", "zweite Frage: ebenso, nur nach der ersten");
    assert_eq!(b.mock.chat_bodies().len(), 2, "beide Aufrufe angekommen");
    assert_eq!(b.mock.max_active(), 1, "höchstens eine Anfrage gleichzeitig (Gemma wird nicht zweimal geladen)");
    assert_eq!(posts(&b, 0).await.len(), 4, "zwei Fragen und zwei Antworten gespeichert");
}
