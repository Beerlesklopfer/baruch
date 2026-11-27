// SPDX-License-Identifier: AGPL-3.0-or-later
//! Client against the own mock (`baruch_llm::mock`, 127.0.0.1:0): model list, streaming in pieces across packet
//! boundaries, `[DONE]`, usage, skipped reasoning trace, error codes, time limits, cancelling by dropping, no
//! redirect, never a key in a message. No test talks to a real model server.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use baruch_llm::mock::{MockEliezer, Reply, Step, delta, finish, sse};
use baruch_llm::{ChatMessage, ChatRequest, Endpoint, Limits, LlmError, Role, chat_stream, chat_stream_with, models};
use serde_json::json;

/// Unique test key; it must not stand in any message.
const SECRET: &str = "Kk7-test-schluessel-91ab";

fn request(model: &str, max_tokens: u32) -> ChatRequest {
    ChatRequest {
        model: model.into(),
        messages: vec![
            ChatMessage { role: Role::System, content: "Du bist ein Prüfer.".into() },
            ChatMessage { role: Role::User, content: "Sag etwas Kurzes.".into() },
        ],
        max_tokens,
        temperature: Some(0.4),
    }
}

fn short_limits() -> Limits {
    Limits {
        connect: Duration::from_secs(2),
        first_token: Duration::from_millis(400),
        idle: Duration::from_millis(400),
        total: Duration::from_secs(5),
        models_total: Duration::from_secs(2),
        max_text: 1024 * 1024,
    }
}

/// Chat with collected pieces.
async fn chat(endpoint: &Endpoint, limits: &Limits) -> (Result<baruch_llm::ChatReply, LlmError>, Vec<String>) {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let sink = seen.clone();
    let result =
        chat_stream_with(endpoint, &request("mock-modell", 200), limits, move |d| sink.lock().unwrap().push(d.into()))
            .await;
    let pieces = seen.lock().unwrap().clone();
    (result, pieces)
}

#[tokio::test]
async fn the_model_list_comes_from_v1_models_with_the_bearer() {
    let mock = MockEliezer::start().await;
    mock.require_bearer(Some(SECRET));
    mock.set_models(&["google/gemma-4-26b-a4b", "llama-3.2-3b-instruct-q4-k-m"]);
    let list = models(&Endpoint::new(mock.url(), Some(SECRET.into()))).await.expect("Liste mit Schlüssel");
    assert_eq!(list, ["google/gemma-4-26b-a4b", "llama-3.2-3b-instruct-q4-k-m"], "ids aus data[].id");
    let seen = mock.requests();
    assert_eq!(seen[0].path, "/v1/models", "GET /v1/models: {seen:?}");
    assert_eq!(seen[0].authorization.as_deref(), Some(format!("Bearer {SECRET}").as_str()), "Bearer gesendet");
    let base_v1 = models(&Endpoint::new(format!("{}/v1/", mock.url()), Some(SECRET.into()))).await;
    assert!(base_v1.is_ok(), "Basis mit /v1 wird erkannt: {base_v1:?}");

    let denied = models(&Endpoint::new(mock.url(), Some("falscher-schluessel-xyz".into()))).await.unwrap_err();
    assert!(matches!(denied, LlmError::Denied(_)), "falscher Schlüssel → Denied: {denied:?}");
    let text = denied.to_string();
    assert!(text.contains("baruch/eliezer"), "Hinweis auf das Konto im Schlüsselbund: {text}");
    assert!(!text.contains("falscher-schluessel-xyz"), "gespiegelter Schlüssel nicht in der Meldung: {text}");
    let none = models(&Endpoint::new(mock.url(), None)).await.unwrap_err();
    assert!(none.to_string().contains("kein Schlüssel hinterlegt"), "ohne Schlüssel ehrlich: {none}");
}

#[tokio::test]
async fn pieces_split_across_packets_arrive_in_order_and_reasoning_is_skipped() {
    let mock = MockEliezer::start().await;
    // One event across three packets, split in the middle of the „ü“ (UTF-8), with \r\n, comment and reasoning trace.
    let first = delta("echt-modell", "Grü");
    let (a, b) = first.split_at(first.len() - 10);
    let umlaut =
        "data: {\"model\":\"echt-modell\",\"choices\":[{\"delta\":{\"content\":\"ße aus \"}}]}\r\n\r\n".as_bytes();
    let cut = umlaut.iter().position(|&x| x == 0xC3).unwrap() + 1;
    let reasoning = sse(&json!({ "model": "echt-modell",
        "choices": [{ "delta": { "reasoning_content": "Ich denke nach …" } }] }));
    let mut steps = vec![
        Step::Send(b": ping\n\n".to_vec()),
        Step::Send(reasoning),
        Step::Pause(Duration::from_millis(20)),
        Step::Send(a.to_vec()),
        Step::Pause(Duration::from_millis(20)),
        Step::Send(b.to_vec()),
        Step::Pause(Duration::from_millis(20)),
        Step::Send(umlaut[..cut].to_vec()),
        Step::Pause(Duration::from_millis(20)),
        Step::Send(umlaut[cut..].to_vec()),
        Step::Send(delta("echt-modell", "Bernau.")),
    ];
    steps.extend(finish("echt-modell", "stop", 42, 3).into_iter().map(Step::Send));
    mock.push(Reply::Stream(steps));
    let (result, pieces) = chat(&Endpoint::new(mock.url(), None), &short_limits()).await;
    let reply = result.expect("Strom gelesen");
    assert_eq!(reply.text, "Grüße aus Bernau.", "Stücke zusammengesetzt, Denkspur übergangen");
    assert_eq!(pieces, ["Grü", "ße aus ", "Bernau."], "jedes Stück sofort und in Reihenfolge an on_delta");
    assert_eq!(reply.model, "echt-modell", "Modell laut Antwort");
    assert_eq!(reply.finish_reason.as_deref(), Some("stop"), "finish_reason aus dem letzten Stück");
    assert_eq!((reply.prompt_tokens, reply.completion_tokens), (Some(42), Some(3)), "usage aus dem Schlussstück");
    let body = &mock.chat_bodies()[0];
    assert_eq!(body["stream"], true, "gestreamt angefragt: {body}");
    assert_eq!(body["stream_options"]["include_usage"], true, "usage angefordert: {body}");
    assert_eq!(body["messages"][0]["role"], "system", "Rollen kleingeschrieben: {body}");
    assert!(mock.context_fields().is_empty() && mock.forbidden_paths().is_empty(), "keine Kontextlänge, kein Laden");
}

#[tokio::test]
async fn done_ends_the_answer_even_if_the_server_keeps_the_connection_open() {
    let mock = MockEliezer::start().await;
    let mut steps = vec![Step::Send(delta("m", "fertig"))];
    steps.extend(finish("m", "stop", 1, 1).into_iter().map(Step::Send));
    steps.push(Step::Pause(Duration::from_secs(30)));
    mock.push(Reply::Stream(steps));
    let started = Instant::now();
    let (result, _) = chat(&Endpoint::new(mock.url(), None), &Limits::default()).await;
    assert_eq!(result.expect("Antwort").text, "fertig", "Text bis [DONE]");
    assert!(started.elapsed() < Duration::from_secs(5), "[DONE] beendet das Lesen sofort: {:?}", started.elapsed());
}

#[tokio::test]
async fn a_server_without_streaming_still_answers() {
    let mock = MockEliezer::start().await;
    mock.push(Reply::Json(
        json!({ "model": "m2", "choices": [{ "message": { "content": "ganz" }, "finish_reason": "stop" }],
        "usage": { "prompt_tokens": 5, "completion_tokens": 1 } }),
    ));
    let (result, pieces) = chat(&Endpoint::new(mock.url(), None), &short_limits()).await;
    let reply = result.expect("JSON-Antwort");
    assert_eq!((reply.text.as_str(), reply.model.as_str()), ("ganz", "m2"), "Text und Modell aus message.content");
    assert_eq!(pieces, ["ganz"], "als ein Stück gemeldet");
}

#[tokio::test]
async fn http_errors_map_to_honest_kinds_without_the_secret() {
    let mock = MockEliezer::start().await;
    let endpoint = Endpoint::new(mock.url(), Some(SECRET.into()));
    let long = format!(r#"{{"error":{{"message":"Ungültige Anfrage mit {SECRET}: {}"}}}}"#, "x".repeat(800));
    for (reply, check, what) in [
        (Reply::Status(401, r#"{"error":{"message":"nope"}}"#.into()), "Denied", "401"),
        (Reply::Status(403, "verboten".into()), "Denied", "403"),
        (Reply::Status(404, r#"{"error":{"message":"no route"}}"#.into()), "UnknownModel", "404"),
        (
            Reply::Status(400, r#"{"error":{"message":"Model not found: xyz"}}"#.into()),
            "UnknownModel",
            "400 model not found",
        ),
        (Reply::Status(503, "busy".into()), "Busy", "503"),
        (Reply::Status(429, "langsam".into()), "Busy", "429"),
        (Reply::Status(409, "lädt".into()), "Busy", "409"),
        (Reply::Status(400, long), "Failed", "400 lang"),
        (Reply::Redirect("http://example.invalid/v1/chat/completions".into()), "Failed", "302"),
    ] {
        mock.push(reply);
        let (result, _) = chat(&endpoint, &short_limits()).await;
        let error = result.unwrap_err();
        let kind = format!("{error:?}");
        assert!(kind.starts_with(check), "{what} → {check}: {kind}");
        let text = error.to_string();
        assert!(!text.contains(SECRET), "{what}: kein Schlüssel in der Meldung: {text}");
        if what == "400 lang" {
            assert!(
                text.contains("***") && text.contains("Ungültige Anfrage"),
                "Meldung der Gegenstelle bleibt lesbar: {text}"
            );
            assert!(text.chars().count() < 300 + 120, "gekürzt auf etwa 300 Zeichen: {}", text.chars().count());
        }
        if what == "302" {
            assert!(text.contains("HTTP 302"), "Weiterleitung nicht gefolgt, sondern gemeldet: {text}");
        }
    }
    let paths: Vec<String> = mock.requests().into_iter().map(|s| s.path).collect();
    assert!(paths.iter().all(|p| p == "/v1/chat/completions"), "keiner Weiterleitung gefolgt: {paths:?}");
    let error = chat_stream(&endpoint, &request("m", 10), |_| {}).await;
    assert!(error.is_ok(), "danach wieder die Vorgabe-Antwort: {error:?}");
}

#[tokio::test]
async fn an_error_inside_the_stream_is_reported() {
    let mock = MockEliezer::start().await;
    mock.push(Reply::Stream(vec![Step::Send(sse(&json!({ "error": { "message": "model not found: abc" } })))]));
    let (result, _) = chat(&Endpoint::new(mock.url(), None), &short_limits()).await;
    assert!(matches!(result, Err(LlmError::UnknownModel(_))), "Fehler im Strom → UnknownModel: {result:?}");
}

#[tokio::test]
async fn a_closed_port_is_unreachable() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    let result = models(&Endpoint::new(format!("http://127.0.0.1:{port}"), None)).await;
    assert!(matches!(result, Err(LlmError::Unreachable(_))), "geschlossener Port → Unreachable: {result:?}");
    let (result, _) = chat(&Endpoint::new(format!("http://127.0.0.1:{port}"), None), &short_limits()).await;
    assert!(matches!(result, Err(LlmError::Unreachable(_))), "auch beim Chat: {result:?}");
}

#[tokio::test]
async fn no_first_piece_in_time_is_a_timeout_and_closes_the_connection() {
    let mock = MockEliezer::start().await;
    mock.push(Reply::Hang);
    let started = Instant::now();
    let (result, _) = chat(&Endpoint::new(mock.url(), None), &short_limits()).await;
    let took = started.elapsed();
    let error = result.unwrap_err();
    assert!(matches!(error, LlmError::Timeout(_)), "kein erstes Stück → Timeout: {error:?}");
    assert!(error.to_string().contains("400 ms"), "Grenze genannt: {error}");
    assert!(took >= Duration::from_millis(350) && took < Duration::from_secs(3), "nach etwa 400 ms: {took:?}");
    assert!(mock.wait_for(Duration::from_secs(3), |m| m.cancelled() == 1).await, "Verbindung beim Nachbau geschlossen");
}

#[tokio::test]
async fn a_long_pause_between_pieces_is_a_timeout() {
    let mock = MockEliezer::start().await;
    mock.push(Reply::Stream(vec![
        Step::Send(delta("m", "Anfang")),
        Step::Pause(Duration::from_secs(5)),
        Step::Send(delta("m", "nie")),
    ]));
    let (result, pieces) = chat(&Endpoint::new(mock.url(), None), &short_limits()).await;
    let error = result.unwrap_err();
    assert!(matches!(error, LlmError::Timeout(_)), "Pause über idle → Timeout: {error:?}");
    assert!(error.to_string().contains("stockt"), "Meldung nennt die Pause: {error}");
    assert_eq!(pieces, ["Anfang"], "das erste Stück kam noch an");
    assert!(mock.wait_for(Duration::from_secs(3), |m| m.cancelled() == 1).await, "Nachbau merkt den Abbruch");
}

#[tokio::test]
async fn the_whole_call_has_a_total_limit_and_the_text_a_size_limit() {
    let mock = MockEliezer::start().await;
    let steps: Vec<Step> =
        (0..40).flat_map(|_| [Step::Send(delta("m", "x")), Step::Pause(Duration::from_millis(100))]).collect();
    mock.push(Reply::Stream(steps));
    let limits = Limits { total: Duration::from_millis(700), ..short_limits() };
    let (result, _) = chat(&Endpoint::new(mock.url(), None), &limits).await;
    assert!(
        matches!(&result, Err(LlmError::Timeout(m)) if m.contains("700 ms")),
        "Gesamtgrenze greift trotz laufender Stücke: {result:?}"
    );
    mock.push(Reply::text(&["0123456789", "0123456789"]));
    let limits = Limits { max_text: 15, ..short_limits() };
    let (result, _) = chat(&Endpoint::new(mock.url(), None), &limits).await;
    assert!(matches!(&result, Err(LlmError::Failed(m)) if m.contains("1 MiB")), "zu langer Text bricht ab: {result:?}");
}

#[tokio::test]
async fn dropping_the_call_closes_the_connection_at_the_server() {
    let mock = MockEliezer::start().await;
    mock.push(Reply::Stream(vec![
        Step::Send(delta("m", "eins")),
        Step::Pause(Duration::from_secs(30)),
        Step::Send(delta("m", "nie")),
    ]));
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    let endpoint = Endpoint::new(mock.url(), None);
    let call = tokio::spawn(async move {
        chat_stream(&endpoint, &request("m", 50), move |d| {
            let _ = tx.send(d.to_owned());
        })
        .await
    });
    assert_eq!(rx.recv().await.as_deref(), Some("eins"), "erstes Stück kommt an");
    assert_eq!(mock.active(), 1, "Aufruf läuft beim Nachbau");
    call.abort();
    assert!(call.await.unwrap_err().is_cancelled(), "Aufgabe abgebrochen");
    assert!(
        mock.wait_for(Duration::from_secs(3), |m| m.cancelled() == 1 && m.active() == 0).await,
        "Fallenlassen des Futures schließt die HTTP-Verbindung (Nachbau zählt den Abbruch)"
    );
}
