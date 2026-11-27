// SPDX-License-Identifier: AGPL-3.0-or-later
//! Queue: exactly one running request (the mock counts simultaneous connections), at most eight waiting, dropped
//! waiting jobs free their place, `status()` names the running job. Every test has its own `Queue` (no shared
//! static one).

use std::sync::Arc;
use std::time::Duration;

use baruch_llm::mock::{MockEliezer, Reply};
use baruch_llm::{ChatMessage, ChatRequest, Endpoint, LlmError, MAX_WAITING, Queue, QueueStatus, Role, chat_stream};

fn request() -> ChatRequest {
    ChatRequest {
        model: "mock-modell".into(),
        messages: vec![ChatMessage { role: Role::User, content: "Hallo".into() }],
        max_tokens: 20,
        temperature: None,
    }
}

/// Waits until `test` holds (at most 3 s).
async fn until(test: impl Fn() -> bool) -> bool {
    for _ in 0..300 {
        if test() {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    test()
}

#[tokio::test]
async fn the_second_request_starts_only_after_the_first_has_finished() {
    let mock = Arc::new(MockEliezer::start().await);
    let pause = Duration::from_millis(80);
    mock.push(Reply::text_from("mock-modell", &["eins ", "zwei ", "drei"], pause));
    mock.push(Reply::text_from("mock-modell", &["vier ", "fünf"], pause));
    let queue = Arc::new(Queue::default());
    let endpoint = Endpoint::new(mock.url(), None);
    let run = |label: &'static str| {
        let (queue, endpoint) = (queue.clone(), endpoint.clone());
        tokio::spawn(async move { queue.run(label, chat_stream(&endpoint, &request(), |_| {})).await })
    };
    let first = run("@lektor ask key 1");
    assert!(until(|| mock.active() == 1).await, "erste Anfrage läuft beim Nachbau");
    let second = run("@hoerer ask key 2");
    assert!(until(|| queue.status().waiting == 1).await, "zweite wartet");
    assert_eq!(
        queue.status(),
        QueueStatus { running: Some("@lektor ask key 1".into()), waiting: 1 },
        "status nennt die Bezeichnung des laufenden Auftrags und die Zahl der Wartenden"
    );
    assert_eq!(mock.requests().len(), 1, "die zweite ist beim Nachbau noch gar nicht angekommen");
    let a = first.await.unwrap().expect("erste Antwort");
    let b = second.await.unwrap().expect("zweite Antwort");
    assert_eq!((a.text.as_str(), b.text.as_str()), ("eins zwei drei", "vier fünf"), "beide in Reihenfolge");
    assert_eq!(mock.max_active(), 1, "nie zwei Anfragen gleichzeitig bei Eliezer");
    assert_eq!(queue.status(), QueueStatus { running: None, waiting: 0 }, "danach leer");
}

#[tokio::test]
async fn the_ninth_waiter_is_turned_away_and_a_dropped_waiter_frees_its_place() {
    let queue = Arc::new(Queue::new(MAX_WAITING));
    let (release, hold) = tokio::sync::oneshot::channel::<()>();
    let busy = queue.clone();
    let running = tokio::spawn(async move {
        busy.run("lange", async move {
            let _ = hold.await;
            Ok::<_, LlmError>(0)
        })
        .await
    });
    assert!(until(|| queue.status().running.is_some()).await, "der erste läuft");
    let mut waiters = Vec::new();
    for i in 1..=MAX_WAITING {
        let q = queue.clone();
        waiters.push(tokio::spawn(async move { q.run("wartet", async move { Ok::<_, LlmError>(i) }).await }));
    }
    assert!(until(|| queue.status().waiting == MAX_WAITING).await, "acht warten: {:?}", queue.status());
    let ninth = queue.run("zu viel", async { Ok::<_, LlmError>(99) }).await;
    assert!(
        matches!(&ninth, Err(LlmError::Busy(m)) if m.contains("Warteschlange voll")),
        "der neunte Wartende bekommt sofort Busy: {ninth:?}"
    );
    assert_eq!(queue.status().waiting, MAX_WAITING, "der Abgewiesene zählt nicht mit");

    waiters.pop().unwrap().abort();
    assert!(
        until(|| queue.status().waiting == MAX_WAITING - 1).await,
        "fallen gelassener Wartender geht: {:?}",
        queue.status()
    );
    let q = queue.clone();
    let late = tokio::spawn(async move { q.run("nachgerückt", async { Ok::<_, LlmError>(100) }).await });
    assert!(until(|| queue.status().waiting == MAX_WAITING).await, "der freie Platz wird wieder genutzt");

    release.send(()).unwrap();
    assert_eq!(running.await.unwrap(), Ok(0), "der erste endet normal");
    for waiter in waiters {
        assert!(waiter.await.unwrap().is_ok(), "jeder Wartende kommt dran");
    }
    assert_eq!(late.await.unwrap(), Ok(100), "auch der Nachgerückte");
    assert_eq!(queue.status(), QueueStatus { running: None, waiting: 0 }, "am Ende leer");
}

#[tokio::test]
async fn dropping_the_running_job_frees_the_queue_at_once() {
    let queue = Arc::new(Queue::default());
    let q = queue.clone();
    let stuck = tokio::spawn(async move { q.run("hängt", std::future::pending::<Result<(), LlmError>>()).await });
    assert!(until(|| queue.status().running.as_deref() == Some("hängt")).await, "läuft");
    stuck.abort();
    let _ = stuck.await;
    let next = tokio::time::timeout(Duration::from_secs(2), queue.run("danach", async { Ok::<_, LlmError>(1) })).await;
    assert_eq!(next.expect("kein Warten mehr"), Ok(1), "nach dem Abbruch ist der Platz sofort frei");
}
