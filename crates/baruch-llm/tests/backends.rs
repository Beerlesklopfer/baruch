// SPDX-License-Identifier: AGPL-3.0-or-later
//! Client je Endpunkt (Runde 15b, Vertrag §3) gegen den eigenen Nachbau auf 127.0.0.1:0 in den Arten Eliezer,
//! Ollama und OpenAI-kompatibel: ein HTTP-Client für alle Aufrufe derselben Verbindung (gezählt an den
//! TCP-Verbindungen des Nachbaus mit keep-alive), gemeldete Kontextlänge je Art, nur lesend (nie
//! load/unload/estimate), Chat über den gemeinsamen Client, Schlüssel nur im Kopf. Kein echter Modell-Server.

use std::time::Duration;

use baruch_llm::mock::{Flavor, MockEliezer, Reply};
use baruch_llm::{BackendKind, ChatMessage, ChatRequest, Client, Endpoint, Limits, Role, models_with};

const SECRET: &str = "Pp4-test-schluessel-r15b";

fn limits() -> Limits {
    Limits {
        connect: Duration::from_secs(2),
        first_token: Duration::from_secs(2),
        idle: Duration::from_secs(2),
        total: Duration::from_secs(5),
        models_total: Duration::from_secs(2),
        max_text: 1024 * 1024,
    }
}

#[tokio::test]
async fn several_calls_of_one_connection_share_one_http_client() {
    let mock = MockEliezer::start().await;
    mock.set_keep_alive(true);
    let client = Client::new(BackendKind::Openai, Endpoint::new(mock.url(), None), limits()).unwrap();
    for _ in 0..3 {
        let list = client.models().await.unwrap();
        assert_eq!(list.len(), 2, "Modellliste des Nachbaus");
    }
    let copy = client.clone();
    copy.models().await.unwrap();
    assert_eq!(
        mock.connections(),
        1,
        "vier Abfragen über einen Client (auch geklont) gehen über eine offene Verbindung – derselbe HTTP-Client"
    );
    // Zum Vergleich: die alte freie Funktion baut je Aufruf einen eigenen HTTP-Client (neue Verbindung).
    models_with(&Endpoint::new(mock.url(), None), &limits()).await.unwrap();
    models_with(&Endpoint::new(mock.url(), None), &limits()).await.unwrap();
    assert_eq!(mock.connections(), 3, "jede freie Abfrage öffnet eine eigene Verbindung (Hülle bleibt erhalten)");
}

#[tokio::test]
async fn ollama_reports_its_context_through_api_show() {
    let mock = MockEliezer::start().await;
    mock.set_flavor(Flavor::Ollama);
    mock.set_models(&["kiesel:8b"]);
    mock.set_context_length("kiesel:8b", 8192);
    let client = Client::new(BackendKind::Ollama, Endpoint::new(mock.url(), None), limits()).unwrap();
    assert_eq!(client.context_length("kiesel:8b").await.unwrap(), Some(8192), "num_ctx aus /api/show");
    assert!(mock.paths().iter().any(|p| p == "/api/show"), "gefragt wurde POST /api/show: {:?}", mock.paths());
    assert!(client.context_length("fehlt:1b").await.is_err(), "unbekanntes Modell: Fehler der Gegenstelle");
}

#[tokio::test]
async fn openai_compatible_reports_no_context_and_asks_nothing() {
    let mock = MockEliezer::start().await;
    mock.set_flavor(Flavor::Openai);
    let client = Client::new(BackendKind::Openai, Endpoint::new(mock.url(), None), limits()).unwrap();
    assert_eq!(client.context_length("mock-modell").await.unwrap(), None, "OpenAI-kompatibel meldet keine Länge");
    assert!(mock.requests().is_empty(), "keine Anfrage für die Kontextlänge: {:?}", mock.paths());
    let list = client.models().await.unwrap();
    assert!(list.iter().all(|m| m.context_length.is_none()), "Liste ohne Kontextlängen: {list:?}");
}

#[tokio::test]
async fn eliezer_reports_context_read_only_and_with_the_key_only_in_the_header() {
    let mock = MockEliezer::start().await;
    mock.require_bearer(Some(SECRET));
    mock.set_context_length("google/gemma-4-26b-a4b", 32768);
    let client = Client::new(BackendKind::Eliezer, Endpoint::new(mock.url(), Some(SECRET.into())), limits()).unwrap();
    assert_eq!(
        client.context_length("google/gemma-4-26b-a4b").await.unwrap(),
        Some(32768),
        "Länge aus GET /api/v1/models (max_context_length)"
    );
    let list = client.models().await.unwrap();
    let gemma = list.iter().find(|m| m.id == "google/gemma-4-26b-a4b").expect("Modell in der Liste");
    assert_eq!(gemma.context_length, Some(32768), "die Liste trägt die gemeldete Länge");
    assert!(mock.forbidden_paths().is_empty(), "nie load/unload/estimate: {:?}", mock.forbidden_paths());
    assert!(
        mock.requests().iter().all(|r| r.authorization.as_deref() == Some(&format!("Bearer {SECRET}")[..])),
        "jede Anfrage trägt den Schlüssel im Kopf"
    );
    let shown = format!("{client:?}");
    assert!(!shown.contains(SECRET), "Debug ohne Schlüssel: {shown}");
    let wrong = Client::new(BackendKind::Eliezer, Endpoint::new(mock.url(), Some("falsch".into())), limits()).unwrap();
    let error = wrong.context_length("google/gemma-4-26b-a4b").await.unwrap_err().to_string();
    assert!(!error.contains("falsch"), "der gespiegelte Schlüssel erscheint nicht in der Meldung: {error}");
}

#[tokio::test]
async fn chat_goes_through_the_shared_client() {
    let mock = MockEliezer::start().await;
    mock.push(Reply::text(&["Hallo ", "Welt"]));
    let client = Client::new(BackendKind::Ollama, Endpoint::new(mock.url(), None), limits()).unwrap();
    let request = ChatRequest {
        model: "mock-modell".into(),
        messages: vec![ChatMessage { role: Role::User, content: "Sag etwas.".into() }],
        max_tokens: 16,
        temperature: None,
    };
    let mut pieces = Vec::new();
    let reply = client.chat_stream(&request, |p| pieces.push(p.to_owned())).await.unwrap();
    assert_eq!(reply.text, "Hallo Welt", "ganzer Text");
    assert_eq!(pieces, vec!["Hallo ", "Welt"], "Stück für Stück");
    assert!(mock.context_fields().is_empty(), "nie eine Kontextlänge im Rumpf");
}
