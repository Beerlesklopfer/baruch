// SPDX-License-Identifier: AGPL-3.0-or-later
//! Runde 23: „Der Client folgt keiner Weiterleitung“ – je Weg (Modellliste, Chat, Einbettungen,
//! Kontextlänge bei Eliezer und Ollama). Zwei eigene Nachbauten auf 127.0.0.1:0: Der erste antwortet auf
//! alles mit 302 auf den zweiten; der zweite schreibt jede Anfrage mit. Erwartet: Der zweite sieht keine
//! Anfrage (also auch keinen Schlüssel), und der Fehler nennt die Weiterleitung. Kein echter Modell-Server.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use baruch_llm::{
    BackendKind, ChatMessage, ChatRequest, Client, Endpoint, Limits, LlmError, Role, chat_stream_with, embeddings_with,
    models_with,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

/// Erfundener Schlüssel; er darf beim zweiten Nachbau nie ankommen und in keiner Meldung stehen.
const SECRET: &str = "Rr5-weiterleitung-schluessel-33cd";

/// Kleiner HTTP-Nachbau: beantwortet jede Anfrage mit `answer(pfad)` und merkt sich die rohe Anfrage.
struct Recorder {
    url: String,
    seen: Arc<Mutex<Vec<String>>>,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for Recorder {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl Recorder {
    async fn start(answer: impl Fn(&str) -> String + Send + Sync + 'static) -> Recorder {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("Nachbau binden");
        let url = format!("http://{}", listener.local_addr().expect("Adresse"));
        let seen = Arc::new(Mutex::new(Vec::new()));
        let log = seen.clone();
        let answer = Arc::new(answer);
        let task = tokio::spawn(async move {
            loop {
                let Ok((mut socket, _)) = listener.accept().await else { break };
                let log = log.clone();
                let answer = answer.clone();
                tokio::spawn(async move {
                    let mut raw = Vec::new();
                    let mut buffer = [0u8; 4096];
                    // Kopf lesen (bis zur Leerzeile); der Rumpf ist für die Prüfung unwichtig.
                    while !raw.windows(4).any(|w| w == b"\r\n\r\n") {
                        match socket.read(&mut buffer).await {
                            Ok(0) | Err(_) => break,
                            Ok(n) => raw.extend_from_slice(&buffer[..n]),
                        }
                    }
                    let text = String::from_utf8_lossy(&raw).into_owned();
                    let path = text.split_whitespace().nth(1).unwrap_or("/").to_owned();
                    log.lock().unwrap().push(text);
                    let _ = socket.write_all(answer(&path).as_bytes()).await;
                    let _ = socket.shutdown().await;
                });
            }
        });
        Recorder { url, seen, task }
    }

    fn requests(&self) -> Vec<String> {
        self.seen.lock().unwrap().clone()
    }
}

/// Erster Nachbau (302 auf denselben Pfad beim zweiten) und zweiter Nachbau (antwortet 200 mit JSON).
async fn pair(status: &'static str) -> (Recorder, Recorder) {
    let target = Recorder::start(|_| {
        let body = r#"{"data":[{"id":"fremdes-modell"}],"models":[]}"#;
        format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
    })
    .await;
    let to = target.url.clone();
    let first = Recorder::start(move |path| {
        format!("HTTP/1.1 {status}\r\nLocation: {to}{path}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
    })
    .await;
    (first, target)
}

fn limits() -> Limits {
    Limits {
        connect: Duration::from_secs(2),
        first_token: Duration::from_secs(3),
        idle: Duration::from_secs(3),
        total: Duration::from_secs(5),
        models_total: Duration::from_secs(3),
        max_text: 1024 * 1024,
    }
}

/// Gemeinsame Zusicherungen: Fehler nennt die Weiterleitung, der zweite Nachbau blieb unberührt.
fn refused(what: &str, error: &LlmError, first: &Recorder, target: &Recorder) {
    let text = error.to_string();
    assert!(
        matches!(error, LlmError::Failed(_)),
        "{what}: eine Weiterleitung ist ein Fehler der Verbindung (Failed), kein Erfolg: {error:?}"
    );
    assert!(
        text.contains("Weiterleitung") && text.contains("HTTP 30"),
        "{what}: die Meldung nennt die Weiterleitung mit ihrem Code, damit der Mensch die Adresse berichtigt: {text}"
    );
    assert!(!text.contains(SECRET), "{what}: der Schlüssel steht nicht in der Meldung: {text}");
    assert!(
        !first.requests().is_empty(),
        "{what}: die Anfrage kam beim ersten Nachbau an (sonst prüft der Test nichts)"
    );
    assert!(
        first.requests().iter().any(|r| r.contains(&format!("Bearer {SECRET}"))),
        "{what}: der erste Nachbau (die eingetragene Adresse) bekam den Schlüssel – nur er"
    );
    let stray = target.requests();
    assert!(
        stray.is_empty(),
        "{what}: der zweite Nachbau sieht keine Anfrage – der Client folgt der Weiterleitung nicht ({} Anfragen)",
        stray.len()
    );
    assert!(
        !stray.iter().any(|r| r.contains(SECRET)),
        "{what}: der Schlüssel ging nicht an die Zieladresse der Weiterleitung"
    );
}

#[tokio::test]
async fn the_model_list_does_not_follow_a_redirect() {
    for status in ["302 Found", "307 Temporary Redirect", "308 Permanent Redirect", "301 Moved Permanently"] {
        let (first, target) = pair(status).await;
        let endpoint = Endpoint::new(first.url.clone(), Some(SECRET.into()));
        let error = models_with(&endpoint, &limits()).await.expect_err("302 ist keine Modellliste");
        refused(&format!("Modellliste ({status})"), &error, &first, &target);
    }
}

#[tokio::test]
async fn chat_does_not_follow_a_redirect() {
    // 307/308 behalten Methode und Rumpf: Gerade hier ginge der Buchtext samt Schlüssel an die fremde Adresse.
    for status in ["302 Found", "307 Temporary Redirect", "308 Permanent Redirect"] {
        let (first, target) = pair(status).await;
        let endpoint = Endpoint::new(first.url.clone(), Some(SECRET.into()));
        let request = ChatRequest {
            model: "erfundenes-modell".into(),
            messages: vec![ChatMessage { role: Role::User, content: "Ein erfundener Satz über Leuchttürme.".into() }],
            max_tokens: 20,
            temperature: None,
        };
        let error = chat_stream_with(&endpoint, &request, &limits(), |_| {}).await.expect_err("302 ist keine Antwort");
        refused(&format!("Chat ({status})"), &error, &first, &target);
    }
}

#[tokio::test]
async fn embeddings_do_not_follow_a_redirect() {
    let (first, target) = pair("307 Temporary Redirect").await;
    let endpoint = Endpoint::new(first.url.clone(), Some(SECRET.into()));
    let inputs = vec!["Ein erfundener Absatz über Gezeiten.".to_owned()];
    let error = embeddings_with(&endpoint, "erfundene-einbettung", &inputs, &limits(), Duration::from_secs(3))
        .await
        .expect_err("307 ist keine Einbettung");
    refused("Einbettungen", &error, &first, &target);
}

#[tokio::test]
async fn the_context_length_does_not_follow_a_redirect() {
    for kind in [BackendKind::Eliezer, BackendKind::Ollama] {
        let (first, target) = pair("302 Found").await;
        let endpoint = Endpoint::new(first.url.clone(), Some(SECRET.into()));
        let client = Client::new(kind, endpoint, limits()).expect("Client");
        let error = client.context_length("erfundenes-modell").await.expect_err("302 ist keine Kontextlänge");
        refused(&format!("Kontextlänge ({})", kind.as_str()), &error, &first, &target);
        let listed = client.models().await.expect_err("302 ist keine Modellliste");
        refused(&format!("Modellliste des Clients ({})", kind.as_str()), &listed, &first, &target);
    }
}
