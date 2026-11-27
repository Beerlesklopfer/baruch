// SPDX-License-Identifier: AGPL-3.0-or-later
//! Round 10, task „backend“: model backend Eliezer in the server (`baruch_server::models`), RPC
//! `model.list`/`model.status`/`model.probe` (directly through `models::call`; the wiring in `rpc.rs` is done by the
//! integrator), „Verbindung prüfen“ with reconciliation of the persona models and the short test on the connection page
//! (without JavaScript).
//!
//! Every test talks only to an own mock (`baruch_llm::mock`, 127.0.0.1:0), whose address is set beforehand through
//! `connection.set`; secrets only in `MemorySecrets` (unique test secret that must appear nowhere). No test names or
//! reaches the real Eliezer.

use crate::common;

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::Router;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::post;
use baruch_llm::mock::{MockEliezer, Reply};
use baruch_llm::{ChatMessage, ChatRequest, LlmError, Role};
use baruch_server::AppState;
use baruch_server::connections::{MemorySecrets, render_connections};
use baruch_server::models;
use serde_json::{Value, json};

/// Unique test secret; it must appear nowhere.
const SECRET: &str = "Mm4-geheim-b2e9d1-Modell-Test";

async fn open_with(secrets: MemorySecrets) -> common::Server {
    let data = tempfile::tempdir().unwrap();
    let state = AppState::open_with_secrets(data.path(), Arc::new(secrets)).await.expect("Datenordner öffnen");
    let state = Arc::new(state);
    common::Server { app: baruch_server::router(state.clone()), state, data }
}

/// Successful RPC call through the router (connection.*).
async fn ok(server: &common::Server, method: &str, params: Value) -> Value {
    let answer = server.rpc(method, params).await;
    assert!(answer["error"].is_null(), "{method} sollte gelingen: {answer}");
    answer["result"].clone()
}

/// `model.*` directly through the module.
async fn model_call(server: &common::Server, method: &str, params: Value) -> Result<Value, (i64, String)> {
    models::call(&server.state, method, params).await.map_err(|e| (e.code, e.message))
}

async fn use_mock(server: &common::Server, mock: &MockEliezer) {
    ok(server, "connection.set", json!({ "kind": "eliezer", "config": { "url": mock.url() } })).await;
}

async fn page(server: &common::Server, pairs: &[(&str, &str)]) -> String {
    let q: BTreeMap<String, String> = pairs.iter().map(|(k, v)| ((*k).to_owned(), (*v).to_owned())).collect();
    render_connections(&server.state, None, &q).await.expect("Seite Verbindungen")
}

/// Excerpt of the page: the connection `kind` (up to the next one).
fn section<'a>(html: &'a str, kind: &str) -> &'a str {
    let start = html.find(&format!("id=\"{kind}\"")).unwrap_or_else(|| panic!("Abschnitt {kind} fehlt: {html}"));
    let rest = &html[start..];
    let end = rest[1..].find("<section class=\"connection\"").map_or(rest.len(), |i| i + 1);
    &rest[..end]
}

fn request(model: &str) -> ChatRequest {
    ChatRequest {
        model: model.into(),
        messages: vec![ChatMessage { role: Role::User, content: "Ein frei erfundener Prüfsatz.".into() }],
        max_tokens: 40,
        temperature: Some(0.4),
    }
}

async fn chat(
    server: &common::Server,
    model: &str,
    label: &str,
) -> (Result<baruch_llm::ChatReply, LlmError>, Vec<String>) {
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let result = models::chat(&server.state, request(model), label, tx).await;
    let mut pieces = Vec::new();
    while let Ok(piece) = rx.try_recv() {
        pieces.push(piece);
    }
    (result, pieces)
}

fn assert_clean(what: &str, text: &str) {
    assert!(!text.contains(SECRET), "{what} enthält das Geheimnis: {text}");
}

#[tokio::test]
async fn the_key_goes_only_to_the_address_it_was_entered_for() {
    let _turn = common::model_queue(module_path!()).await;
    let server = open_with(MemorySecrets::with([("eliezer", SECRET)])).await;
    let mock = MockEliezer::start().await;
    mock.require_bearer(Some(SECRET));
    // New address, key still stored for the old one: nothing may go out.
    use_mock(&server, &mock).await;
    let (result, _) = chat(&server, "mock-modell", "Test Bindung").await;
    let error = result.unwrap_err();
    assert!(
        matches!(&error, LlmError::NotConfigured(m) if m.contains("anderes Ziel") && m.contains("nicht gesendet")),
        "Schlüssel an ein anderes Ziel gebunden → NotConfigured: {error:?}"
    );
    let listed = model_call(&server, "model.list", json!({})).await.unwrap_err();
    assert_eq!(listed.0, -32000, "model.list scheitert ehrlich: {listed:?}");
    assert!(listed.1.contains("anderes Ziel"), "Meldung nennt den Grund: {listed:?}");
    assert!(mock.requests().is_empty(), "der Nachbau wird gar nicht gefragt: {:?}", mock.requests());

    // Entered for the new address: now the key goes there as Bearer.
    ok(&server, "connection.set_secret", json!({ "kind": "eliezer", "secret": SECRET })).await;
    let (result, _) = chat(&server, "mock-modell", "Test Bindung").await;
    assert_eq!(result.expect("mit gebundenem Schlüssel").text, "OK", "Vorgabe-Antwort des Nachbaus");
    let seen = mock.requests();
    assert_eq!(seen.len(), 1, "genau ein Aufruf: {seen:?}");
    assert_eq!(seen[0].authorization.as_deref(), Some(format!("Bearer {SECRET}").as_str()), "Bearer gesendet");

    // Yet another address without a new secret: no Authorization arrives there.
    let other = MockEliezer::start().await;
    use_mock(&server, &other).await;
    let (result, _) = chat(&server, "mock-modell", "Test Bindung").await;
    assert!(matches!(result, Err(LlmError::NotConfigured(_))), "wieder zurückgehalten: {result:?}");
    assert!(
        other.requests().iter().all(|s| s.authorization.is_none()),
        "kein Authorization beim neuen Ziel: {:?}",
        other.requests()
    );
    assert!(other.requests().is_empty(), "das neue Ziel wird gar nicht gefragt");
}

#[tokio::test]
async fn model_list_names_models_aliases_and_the_state_of_every_persona_and_caches_it() {
    let _turn = common::model_queue(module_path!()).await;
    let server = common::open().await;
    let mock = MockEliezer::start().await;
    mock.set_models(&["google/gemma-4-26b-a4b", "llama-3.2-3b-instruct-q4-k-m"]);
    use_mock(&server, &mock).await;
    let first = model_call(&server, "model.list", json!({})).await.expect("model.list");
    assert_eq!(first["models"], json!(["google/gemma-4-26b-a4b", "llama-3.2-3b-instruct-q4-k-m"]), "Liste: {first}");
    assert_eq!(first["live"], true, "eben abgefragt: {first}");
    assert!(first["fetched"].as_str().is_some_and(|t| t.ends_with('Z') && t.contains('T')), "ISO-Zeit: {first}");
    assert_eq!(
        first["aliases"],
        json!({ "llama3.2:3b": "llama-3.2-3b-instruct-q4-k-m", "qwen3:4b": "qwen3-4b-q4-k-m" }),
        "Aliasse alt → neu"
    );
    let persona = |handle: &str| {
        first["personas"].as_array().unwrap().iter().find(|p| p["handle"] == handle).cloned().unwrap_or_default()
    };
    assert_eq!(persona("@lektor")["state"], "bekannt", "Gemma kennt Eliezer: {first}");
    assert_eq!(
        (persona("@exeget")["state"].clone(), persona("@exeget")["target"].clone()),
        (json!("alias"), json!("llama-3.2-3b-instruct-q4-k-m")),
        "alter Ollama-Name mit bekanntem Alias: {first}"
    );
    assert_eq!(
        (persona("@schrift")["state"].clone(), persona("@schrift")["target"].clone()),
        (json!("unbekannt"), json!("qwen3-4b-q4-k-m")),
        "Alias, den Eliezer (noch) nicht kennt, bleibt unbekannt mit Hinweis: {first}"
    );
    assert_eq!(persona("@nathan")["state"], "unbekannt", "fremder Name: {first}");

    let second = model_call(&server, "model.list", json!({})).await.expect("model.list");
    assert_eq!(second["live"], false, "zweiter Aufruf aus dem Zwischenspeicher: {second}");
    assert_eq!(second["fetched"], first["fetched"], "Zeitpunkt der echten Abfrage bleibt");
    let asked = mock.requests().iter().filter(|s| s.path == "/v1/models").count();
    assert_eq!(asked, 1, "innerhalb von 60 s nur eine Abfrage bei Eliezer");
    assert!(mock.requests().iter().all(|s| s.authorization.is_none()), "ohne Schlüssel kein Authorization");
}

#[tokio::test]
async fn the_probe_resolves_aliases_sends_a_short_fixed_prompt_and_reports_the_time() {
    let _turn = common::model_queue(module_path!()).await;
    let server = common::open().await;
    let mock = MockEliezer::start().await;
    use_mock(&server, &mock).await;
    mock.push(Reply::text_from("llama-3.2-3b-instruct-q4-k-m", &["O", "K"], Duration::from_millis(10)));
    let probe = model_call(&server, "model.probe", json!({ "model": "llama3.2:3b" })).await.expect("model.probe");
    assert_eq!(probe["ok"], true, "Kurztest gelingt: {probe}");
    assert_eq!(probe["reply"], "OK", "Antwort zusammengesetzt: {probe}");
    assert_eq!(probe["model"], "llama-3.2-3b-instruct-q4-k-m", "tatsächliches Modell: {probe}");
    assert_eq!(probe["requested"], "llama3.2:3b", "angefragter Name bleibt sichtbar: {probe}");
    assert!(probe["ms"].as_u64().is_some(), "Dauer in ms: {probe}");
    let body = &mock.chat_bodies()[0];
    assert_eq!(body["model"], "llama-3.2-3b-instruct-q4-k-m", "Alias vor dem Senden aufgelöst: {body}");
    assert_eq!(body["max_tokens"], 16, "Kurztest höchstens 16 Token: {body}");
    assert_eq!(body["messages"], json!([{ "role": "user", "content": "Antworte nur mit: OK" }]), "fester Auftrag");

    model_call(&server, "model.probe", json!({ "model": "qwen3:4b" })).await.expect("zweiter Kurztest");
    assert_eq!(mock.chat_bodies()[1]["model"], "qwen3-4b-q4-k-m", "auch der zweite Alias");

    mock.push(Reply::Status(404, r#"{"error":{"message":"model 'gibt-es-nicht' not found"}}"#.into()));
    let probe = model_call(&server, "model.probe", json!({ "model": "gibt-es-nicht" })).await.expect("Ergebnis");
    assert_eq!(probe["ok"], false, "Fehlschlag ist ein Ergebnis: {probe}");
    assert!(probe["message"].as_str().unwrap().contains("Unbekanntes Modell"), "Meldung: {probe}");

    for params in [json!({}), json!({ "model": "  " }), json!({ "model": "a\nb" }), json!([1])] {
        let error = model_call(&server, "model.probe", params.clone()).await.unwrap_err();
        assert_eq!(error.0, -32602, "ungültiges Modell {params} → -32602: {error:?}");
    }
    assert!(mock.forbidden_paths().is_empty(), "nie load/unload/estimate: {:?}", mock.forbidden_paths());
    assert!(mock.context_fields().is_empty(), "nie eine Kontextlänge im Rumpf: {:?}", mock.context_fields());
    assert_eq!(model_call(&server, "model.gibts", json!({})).await.unwrap_err().0, -32601, "unbekannte Methode");
    assert!(models::handles("model.probe") && !models::handles("model.load"), "nur die drei Methoden");
}

#[tokio::test]
async fn chat_streams_every_piece_to_the_channel_and_returns_the_answering_model() {
    let _turn = common::model_queue(module_path!()).await;
    let server = common::open().await;
    let mock = MockEliezer::start().await;
    use_mock(&server, &mock).await;
    mock.push(Reply::text_from("google/gemma-4-26b-a4b", &["Ein ", "kurzer ", "Befund."], Duration::from_millis(10)));
    let (result, pieces) = chat(&server, "google/gemma-4-26b-a4b", "@lektor ask key 1").await;
    let reply = result.expect("Antwort");
    assert_eq!(pieces, ["Ein ", "kurzer ", "Befund."], "jedes Stück über den Kanal, in Reihenfolge");
    assert_eq!(reply.text, "Ein kurzer Befund.", "ganzer Text");
    assert_eq!(reply.model, "google/gemma-4-26b-a4b", "Modell laut Antwort");
    assert_eq!(reply.finish_reason.as_deref(), Some("stop"), "finish_reason");
    assert_eq!(reply.completion_tokens, Some(3), "usage durchgereicht");
    assert_eq!(models::alias("llama3.2:3b"), Some("llama-3.2-3b-instruct-q4-k-m"), "Alias über models::alias");
    assert_eq!(models::alias("google/gemma-4-26b-a4b"), None, "Eliezer-Namen bleiben");
}

#[tokio::test]
async fn the_status_names_the_running_label_and_two_calls_never_overlap() {
    let _turn = common::model_queue(module_path!()).await;
    let server = Arc::new(common::open().await);
    let mock = Arc::new(MockEliezer::start().await);
    use_mock(&server, &mock).await;
    let pause = Duration::from_millis(150);
    mock.push(Reply::text_from("mock-modell", &["a", "b", "c", "d"], pause));
    mock.push(Reply::text_from("mock-modell", &["e", "f"], pause));
    let run = |label: &'static str| {
        let server = server.clone();
        tokio::spawn(async move {
            let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
            models::chat(&server.state, request("mock-modell"), label, tx).await
        })
    };
    let first = run("@lektor ask key 17");
    let deadline = Instant::now() + Duration::from_secs(10);
    while models::status().running.as_deref() != Some("@lektor ask key 17") && Instant::now() < deadline {
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    let status = model_call(&server, "model.status", json!({})).await.expect("model.status");
    assert_eq!(status["running"], "@lektor ask key 17", "Bezeichnung des laufenden Auftrags: {status}");
    assert!(status["waiting"].as_u64().is_some(), "Zahl der Wartenden: {status}");
    let second = run("@hoerer ask key 18");
    assert!(first.await.unwrap().is_ok() && second.await.unwrap().is_ok(), "beide Aufrufe gelingen");
    assert_eq!(mock.max_active(), 1, "die prozessweite Warteschlange lässt nie zwei gleichzeitig zu Eliezer");
}

#[tokio::test]
async fn no_secret_appears_in_rpc_answers_messages_or_html() {
    let _turn = common::model_queue(module_path!()).await;
    let server = open_with(MemorySecrets::default()).await;
    let mock = MockEliezer::start().await;
    use_mock(&server, &mock).await;
    ok(&server, "connection.set_secret", json!({ "kind": "eliezer", "secret": SECRET })).await;
    // The mock wants a different key and mirrors the presented one in its message.
    mock.require_bearer(Some("anderer-schluessel-0000"));
    let probe = model_call(&server, "model.probe", json!({ "model": "mock-modell" })).await.expect("Ergebnis");
    assert_eq!(probe["ok"], false, "abgewiesen: {probe}");
    let message = probe["message"].as_str().unwrap();
    assert!(message.contains("verweigert") && message.contains("baruch/eliezer"), "ehrliche Meldung: {message}");
    assert_clean("model.probe", &probe.to_string());
    let listed = model_call(&server, "model.list", json!({})).await.unwrap_err();
    assert_clean("model.list (Fehler)", &listed.1);
    let check = ok(&server, "connection.check", json!({ "kind": "eliezer" })).await;
    assert_clean("connection.check", &check.to_string());
    assert!(
        mock.requests().iter().any(|s| s.authorization.as_deref() == Some(&format!("Bearer {SECRET}"))),
        "gesendet"
    );

    // Right key, but a model name carries the secret: that does not get passed on either.
    mock.require_bearer(Some(SECRET));
    let echo = format!("spiegel-{SECRET}");
    mock.set_models(&["mock-modell", &echo]);
    let listed = model_call(&server, "model.list", json!({})).await.expect("model.list");
    assert_eq!(listed["models"][1], "spiegel-***", "Geheimnis im Modellnamen ersetzt: {listed}");
    assert_clean("model.list", &listed.to_string());
    let status = model_call(&server, "model.status", json!({})).await.expect("model.status");
    assert_clean("model.status", &status.to_string());
    let html = page(&server, &[("ok", "kurztest")]).await;
    assert!(section(&html, "eliezer").contains("data-probe-result=\"failed\""), "Kurztest-Ergebnis sichtbar: {html}");
    assert_clean("Verbindungsseite", &html);
}

#[tokio::test]
async fn checking_eliezer_compares_persona_models_and_the_page_offers_a_probe_without_javascript() {
    let _turn = common::model_queue(module_path!()).await;
    let server = common::open().await;
    let mock = MockEliezer::start().await;
    mock.set_models(&["google/gemma-4-26b-a4b", "llama-3.2-3b-instruct-q4-k-m"]);
    use_mock(&server, &mock).await;

    let html = page(&server, &[]).await;
    let part = section(&html, "eliezer");
    assert!(
        part.contains("<form class=\"connection-probe\" method=\"post\" action=\"/einstellungen/verbindungen/act\""),
        "Kurztest als gewöhnliches Formular (ohne JavaScript): {part}"
    );
    assert!(part.contains("value=\"model.probe\""), "Methode model.probe: {part}");
    assert!(
        part.contains("<option value=\"google&#x2f;gemma-4-26b-a4b\" selected>"),
        "ohne Prüfung Auswahl aus den Persona-Modellen, Gemma vorgewählt: {part}"
    );
    assert!(mock.requests().is_empty(), "die Seite fragt Eliezer nie selbst");
    assert!(part.contains("data-usage") && !part.contains("data-stub"), "kein Nutzungs-STUB mehr: {part}");
    let multigpt = section(&html, "multigpt");
    assert!(
        multigpt.contains("data-usage") && !multigpt.contains("data-stub"),
        "Runde 13: auch bei MultiGPT kein offener STUB-Hinweis mehr (Volltext und Abgleich umgesetzt): {multigpt}"
    );

    let result = ok(&server, "connection.check", json!({ "kind": "eliezer" })).await;
    assert_eq!(result["ok"], true, "Prüfung gelingt: {result}");
    let message = result["message"].as_str().unwrap();
    assert!(message.starts_with("Verbindung steht: 2 Modelle."), "Zahl der Modelle: {message}");
    assert!(
        message.contains("Unbekannt bei Eliezer:") && message.contains("@nathan (gpt-4.1-mini)"),
        "Unbekannte: {message}"
    );
    assert!(message.contains("@exeget llama3.2:3b → llama-3.2-3b-instruct-q4-k-m"), "Alias-Hinweis: {message}");
    let states = result["details"]["personas"].as_array().expect("details.personas wie model.list");
    assert_eq!(states.len(), 6, "alle sechs Personas: {result}");
    let listed = model_call(&server, "model.list", json!({})).await.expect("model.list");
    assert_eq!(listed["live"], false, "die Prüfung füllt den Zwischenspeicher: {listed}");
    assert_eq!(listed["personas"], result["details"]["personas"], "gleicher Abgleich wie model.list");

    let html = page(&server, &[]).await;
    let part = section(&html, "eliezer");
    assert!(part.contains("data-personas") && part.contains("unbekannt bei Eliezer"), "Abgleich sichtbar: {part}");
    assert!(part.contains("Alias → llama-3.2-3b-instruct-q4-k-m"), "Alias sichtbar: {part}");
    assert!(
        part.contains("<option value=\"llama-3.2-3b-instruct-q4-k-m\">"),
        "nach der Prüfung Auswahl aus der Liste von Eliezer: {part}"
    );

    let act = "/einstellungen/verbindungen/act";
    let (status, location) =
        server.post_form(act, "method=model.probe&kind=eliezer&model=google%2Fgemma-4-26b-a4b").await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Formular antwortet mit 303");
    assert_eq!(location.as_deref(), Some("/einstellungen/verbindungen?ok=kurztest#eliezer"), "feste Codes, Anker");
    let html = page(&server, &[("ok", "kurztest")]).await;
    assert!(html.contains("Der Kurztest ist gelaufen"), "Erfolgsmeldung: {html}");
    let part = section(&html, "eliezer");
    assert!(part.contains("data-probe-result=\"ok\"") && part.contains("Antwort: „OK“"), "Ergebnis: {part}");
    assert_eq!(mock.chat_bodies()[0]["model"], "google/gemma-4-26b-a4b", "gewähltes Modell gesendet");

    let (_, location) = server.post_form(act, "method=model.probe&kind=eliezer&model=").await;
    assert_eq!(
        location.as_deref(),
        Some("/einstellungen/verbindungen?fehler=kurztest_modell&kind=eliezer"),
        "ohne Modell nichts gesendet"
    );
    let (_, location) = server.post_form(act, "method=model.probe&kind=multigpt&model=x").await;
    assert_eq!(location.as_deref(), Some("/einstellungen/verbindungen?fehler=methode&kind=multigpt"), "nur Eliezer");
    assert_eq!(mock.chat_bodies().len(), 1, "abgelehnte Formulare schicken nichts");

    // Only the human: `POST /act` does not know model.*.
    let (status, _) = server.post_form("/act", "method=model.probe&model=mock-modell").await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "model.probe nicht über /act");
    assert_eq!(mock.chat_bodies().len(), 1, "über /act geht nichts hinaus");
}

/// MCP peer that lets every request lie for 4 s: three requests together take 12 s.
async fn slow_mcp() -> String {
    let app = Router::new().route(
        "/mcp",
        post(|body: String| async move {
            tokio::time::sleep(Duration::from_secs(4)).await;
            let id = serde_json::from_str::<Value>(&body).ok().and_then(|v| v.get("id").cloned());
            match id {
                Some(id) => {
                    axum::Json(json!({ "jsonrpc": "2.0", "id": id, "result": { "tools": [] } })).into_response()
                }
                None => StatusCode::ACCEPTED.into_response(),
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    format!("http://{address}/mcp")
}

#[tokio::test]
async fn the_whole_check_has_one_time_limit_of_ten_seconds() {
    let _turn = common::model_queue(module_path!()).await;
    let server = common::open().await;
    let url = slow_mcp().await;
    ok(&server, "connection.set", json!({ "kind": "multigpt", "config": { "url": url } })).await;
    let started = Instant::now();
    let result = ok(&server, "connection.check", json!({ "kind": "multigpt" })).await;
    let took = started.elapsed();
    assert_eq!(result["ok"], false, "zu langsam: {result}");
    assert!(result["message"].as_str().unwrap().contains("innerhalb von 10 s"), "Zeitlimit genannt: {result}");
    assert!(
        took >= Duration::from_secs(9) && took < Duration::from_secs(12),
        "gemeinsames Limit greift nach 10 s: {took:?}"
    );
}
