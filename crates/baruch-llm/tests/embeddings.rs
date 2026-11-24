// SPDX-License-Identifier: AGPL-3.0-or-later
//! Einbettungen gegen den Nachbau (Runde 12): `POST /v1/embeddings` mit Liste, Ordnung nach `index`,
//! deterministische Vektoren, unbekanntes Modell, Schlüssel, Grenzen.

use std::time::Duration;

use baruch_llm::mock::{MOCK_DIMENSION, MOCK_EMBEDDING_MODEL, MockEliezer, mock_vector};
use baruch_llm::{Endpoint, Limits, LlmError, embeddings, embeddings_with};

fn texts(list: &[&str]) -> Vec<String> {
    list.iter().map(|t| (*t).to_owned()).collect()
}

#[tokio::test]
async fn embeddings_come_back_in_input_order_with_model_and_dimension() {
    let mock = MockEliezer::start().await;
    let endpoint = Endpoint::new(mock.url(), None);
    let input = texts(&["Der Leuchtturm im Nebel", "Das Senfkorn im Garten", "Der Leuchtturm"]);
    let reply = embeddings(&endpoint, MOCK_EMBEDDING_MODEL, &input).await.unwrap();
    assert_eq!(reply.model, MOCK_EMBEDDING_MODEL, "Modell laut Antwort");
    assert_eq!(reply.dimension, MOCK_DIMENSION, "Dimension des Nachbaus");
    assert_eq!(reply.vectors.len(), 3, "je Text ein Vektor");
    // Der Nachbau antwortet rückwärts; der Client ordnet nach `index`.
    for (text, vector) in input.iter().zip(&reply.vectors) {
        assert_eq!(vector, &mock_vector(text, MOCK_DIMENSION), "Vektor gehört zu „{text}“");
    }
    let bodies = mock.embedding_bodies();
    assert_eq!(bodies.len(), 1, "eine Anfrage für drei Texte");
    assert_eq!(bodies[0]["input"], serde_json::json!(input), "input als Liste");
    assert_eq!(bodies[0]["model"], MOCK_EMBEDDING_MODEL, "Modell im Rumpf");
    assert_eq!(mock.requests()[0].path, "/v1/embeddings", "OpenAI-kompatibler Pfad");
}

#[test]
fn texts_sharing_words_are_closer_in_the_mock() {
    let cos = |a: &[f32], b: &[f32]| a.iter().zip(b).map(|(x, y)| x * y).sum::<f32>();
    let lighthouse = mock_vector("Leuchtturm Nebel Horn", MOCK_DIMENSION);
    let near = mock_vector("Nebel am Leuchtturm", MOCK_DIMENSION);
    let far = mock_vector("Senfkorn Garten Spatzen", MOCK_DIMENSION);
    assert!(cos(&lighthouse, &near) > cos(&lighthouse, &far), "gemeinsame Wörter → ähnlicher");
    let norm: f32 = lighthouse.iter().map(|x| x * x).sum::<f32>().sqrt();
    assert!((norm - 1.0).abs() < 1e-5, "normiert: {norm}");
    assert_eq!(mock_vector("", 8)[0], 1.0, "ohne Wörter: erstes Fach");
}

#[tokio::test]
async fn an_unknown_model_a_wrong_key_and_too_many_texts_are_honest_errors() {
    let mock = MockEliezer::start().await;
    let endpoint = Endpoint::new(mock.url(), None);
    let e = embeddings(&endpoint, "gibt-es-nicht", &texts(&["x"])).await.unwrap_err();
    assert!(matches!(e, LlmError::UnknownModel(_)), "404 model not found → UnknownModel: {e:?}");

    mock.require_bearer(Some("Ee-test-schluessel"));
    let wrong = Endpoint::new(mock.url(), Some("Ff-falsch".into()));
    let e = embeddings(&wrong, MOCK_EMBEDDING_MODEL, &texts(&["x"])).await.unwrap_err();
    assert!(matches!(e, LlmError::Denied(_)), "401 → Denied: {e:?}");
    assert!(!e.to_string().contains("Ff-falsch"), "der gespiegelte Schlüssel steht nicht in der Meldung: {e}");
    let right = Endpoint::new(mock.url(), Some("Ee-test-schluessel".into()));
    assert!(embeddings(&right, MOCK_EMBEDDING_MODEL, &texts(&["x"])).await.is_ok(), "mit Schlüssel geht es");

    let many = vec!["x".to_owned(); 33];
    assert!(embeddings(&right, MOCK_EMBEDDING_MODEL, &many).await.is_err(), "33 Texte: abgelehnt, nichts gesendet");
    assert!(embeddings(&right, MOCK_EMBEDDING_MODEL, &[]).await.is_err(), "kein Text: abgelehnt");
    let sent = mock.embedding_bodies().len();
    assert_eq!(sent, 3, "nur die drei gültigen Anfragen gingen hinaus");

    mock.fail_embeddings(Some((503, r#"{"error":{"message":"beschäftigt"}}"#)));
    let e = embeddings(&right, MOCK_EMBEDDING_MODEL, &texts(&["x"])).await.unwrap_err();
    assert!(matches!(e, LlmError::Busy(_)), "503 → Busy: {e:?}");
}

#[tokio::test]
async fn a_slow_answer_runs_into_the_total_limit_and_a_closed_port_is_unreachable() {
    let mock = MockEliezer::start().await;
    mock.set_embedding_delay(Duration::from_millis(500));
    let endpoint = Endpoint::new(mock.url(), None);
    let e = embeddings_with(
        &endpoint,
        MOCK_EMBEDDING_MODEL,
        &texts(&["x"]),
        &Limits::default(),
        Duration::from_millis(100),
    )
    .await
    .unwrap_err();
    assert!(matches!(e, LlmError::Timeout(_)), "Zeitlimit: {e:?}");
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    drop(listener);
    let e = embeddings(&Endpoint::new(url, None), MOCK_EMBEDDING_MODEL, &texts(&["x"])).await.unwrap_err();
    assert!(matches!(e, LlmError::Unreachable(_)), "geschlossener Port: {e:?}");
}
