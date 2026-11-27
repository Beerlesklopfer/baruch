// SPDX-License-Identifier: AGPL-3.0-or-later
//! Round 18b (konstrukte): lector leftovers from „Nach Runde 15 offen“ – summaries of the chunking cached in memory
//! (number of model calls counted with an own `ModelPort`), „an Persona geben“ on `/quelle/multigpt` without an adopted
//! title (mock `MockMultigpt` on 127.0.0.1:0), retry in the stream discards only the running piece (SSE). The line of
//! the omitted parts is checked by `assist::context` (unit test), the calculation for it by `assist::chunk`. No
//! Eliezer, no network except own mocks; tests with `common::model_queue`. All texts freely invented.

use crate::common;

use std::sync::{Arc, Mutex};

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use baruch_server::assist::port::{BoxFuture, ModelAnswer, ModelCall, ModelFailure, ModelList, ModelPort, Role};
use baruch_server::assist::{Mode, Order, SUMMARY_CACHE_MAX, Scope, prompts, summary_cache};
use baruch_server::connections::multigpt::mock::MockMultigpt;
use serde_json::{Value, json};
use tokio_stream::StreamExt;
use tower::ServiceExt;

async fn server_with(model: Arc<dyn ModelPort>) -> common::Server {
    let data = tempfile::tempdir().unwrap();
    let state = baruch_server::AppState::open_with(
        data.path(),
        Arc::new(baruch_server::connections::MemorySecrets::default()),
        model,
    )
    .await
    .expect("Datenordner lässt sich nicht öffnen");
    let state = Arc::new(state);
    common::Server { app: baruch_server::router(state.clone()), state, data }
}

async fn ok(server: &common::Server, method: &str, params: Value) -> Value {
    let answer = server.rpc(method, params).await;
    assert!(answer["error"].is_null(), "{method} sollte gelingen: {answer}");
    answer["result"].clone()
}

async fn keys_of(server: &common::Server, work: i64) -> Vec<i64> {
    server.state.store.load_work(work).await.unwrap().nodes.iter().map(|n| n.key).collect()
}

fn system(call: &ModelCall) -> &str {
    call.messages.iter().find(|m| m.role == Role::System).map(|m| m.content.as_str()).unwrap_or("")
}

/// Mock: counts calls; summaries get a fixed sentence. Checks per piece: piece 1 with „Kritisch:“, from piece 2 on at
/// first without (then Baruch asks again once), after the repeated request with.
#[derive(Clone, Default)]
struct Counter {
    calls: Arc<Mutex<Vec<ModelCall>>>,
}

impl Counter {
    fn summaries(&self) -> usize {
        self.calls.lock().unwrap().iter().filter(|c| c.label.contains("Zusammenfassung")).count()
    }
}

impl ModelPort for Counter {
    fn chat<'a>(
        &'a self,
        _state: &'a baruch_server::AppState,
        call: ModelCall,
        deltas: tokio::sync::mpsc::UnboundedSender<String>,
    ) -> BoxFuture<'a, Result<ModelAnswer, ModelFailure>> {
        Box::pin(async move {
            self.calls.lock().unwrap().push(call.clone());
            let text = if call.label.contains("Zusammenfassung") {
                "Kurz gefasst: Wasser holen am Morgen.".to_owned()
            } else if call.label.contains("Stück 1/") || system(&call).contains(prompts::CRITICAL_RETRY_NOTE) {
                "Kritisch: „Tränke 1 am Bach“ wiederholt sich.\n- Schritt: prüfen.".to_owned()
            } else if call.label.contains("Stück") {
                "Zweites Stück ohne Pflichtbefund.".to_owned()
            } else {
                "Kritisch: „Gelbe Höschen“ ist unklar.\n- Schritt: prüfen.".to_owned()
            };
            for word in text.split_inclusive(' ') {
                let _ = deltas.send(word.to_owned());
            }
            Ok(ModelAnswer {
                text,
                model: call.model.clone(),
                finish_reason: Some("stop".into()),
                prompt_tokens: None,
                completion_tokens: None,
            })
        })
    }

    fn models<'a>(&'a self, _state: &'a baruch_server::AppState) -> BoxFuture<'a, Result<ModelList, ModelFailure>> {
        Box::pin(async { Err(ModelFailure::NotConfigured("Nachbau ohne Modellliste".into())) })
    }

    fn alias(&self, _name: &str) -> Option<String> {
        None
    }
}

const HIVE: &str = "#set document(title: \"Das Bienenjahr\", author: \"Ida Wabe\")\n#set heading(numbering: \"1.1\")\n\n\
= Frühjahr\n\nDie Völker erwachen im März.\n\n\
== Pollen\n\nGelbe Höschen an den Beinen zeigen, dass das Volk brütet.\n\n\
== Wasser\n\nAn der Tränke stehen morgens die Wasserholerinnen.\n";

/// Long section „Wasser“ of `n` different sentences.
fn long_water(n: usize) -> String {
    let sentences: Vec<String> = (1..=n).map(|i| format!("Die Tränke {i} am Bach steht leer.")).collect();
    format!("== Wasser\n\n{}\n", sentences.join(" "))
}

fn order(work: i64, key: i64) -> Order {
    Order {
        work,
        key,
        persona: "@lektor".into(),
        mode: Mode::Review,
        text: String::new(),
        rev: None,
        asker: None,
        post_question: false,
        sources: vec![],
        web: false,
        scope: Scope::Section,
        frame: Default::default(),
        links: Default::default(),
        basis: None,
    }
}

#[tokio::test]
async fn summaries_of_the_same_text_and_model_come_from_the_cache() {
    let _turn = common::model_queue(module_path!()).await;
    let port = Counter::default();
    let server = server_with(Arc::new(port.clone())).await;
    let work = server.import(HIVE, "bienen.typ").await.work;
    let keys = keys_of(&server, work).await;
    ok(&server, "revision.save", json!({ "work": work, "key": keys[2], "body": long_water(160) })).await;
    ok(&server, "persona.update", json!({ "handle": "@lektor", "context_length": 3200 })).await;

    let first = baruch_server::assist::run(&server.state, &order(work, keys[1]), None).await.unwrap();
    assert!(first.compressed.is_some(), "das lange Umfeld passt nur zusammengefasst (sonst prüft der Test nichts)");
    let after_first = port.summaries();
    assert!(after_first > 0, "beim ersten Auftrag fasst das Modell zusammen");

    let second = baruch_server::assist::run(&server.state, &order(work, keys[1]), None).await.unwrap();
    assert!(second.compressed.is_some(), "auch der zweite Auftrag ist komprimiert");
    assert_eq!(
        port.summaries(),
        after_first,
        "derselbe Text, dasselbe Modell: keine neue Zusammenfassung (Zwischenspeicher, vorher fasste jeder Auftrag neu)"
    );

    // Different text (every sentence different): new summaries, never an old one for changed text.
    let other = long_water(160).replace("steht leer", "ist voll");
    ok(&server, "revision.save", json!({ "work": work, "key": keys[2], "body": other })).await;
    baruch_server::assist::run(&server.state, &order(work, keys[1]), None).await.unwrap();
    assert!(port.summaries() > after_first, "geänderter Text wird neu zusammengefasst");
    assert!(summary_cache().len() <= SUMMARY_CACHE_MAX, "der Zwischenspeicher hat eine Obergrenze");
}

/// Chapter of `sections` sections of about `chars` characters each (invented text about mills).
fn mills(sections: usize, chars: usize) -> String {
    let mut text = String::from(
        "#set document(title: \"Die Mühlen\", author: \"Jo Rad\")\n#set heading(numbering: \"1.1\")\n\n\
         = Am Fluss\n\nAm Morgen drehen sich die Räder.\n\n",
    );
    for i in 1..=sections {
        let sentence = format!("Mühle {i} mahlt das Korn des Dorfes zu feinem Mehl. ");
        let body = sentence.repeat(chars / sentence.chars().count() + 1);
        text.push_str(&format!("== Mühle {i}\n\n{}\n\n", body.trim_end()));
    }
    text
}

#[tokio::test]
async fn omitted_parts_in_one_line_let_a_long_chapter_fit_without_any_call_over_the_context() {
    // Backlog „Nach Runde 15 offen“: with about 20 overlapping pieces and 4096 tokens Baruch rejected, because every
    // omitted part cost a line of its own with a hint. Consecutive omitted parts are now on one line; the same case
    // fits, and no call exceeds the budget when measured.
    let _turn = common::model_queue(module_path!()).await;
    let port = Counter::default();
    let server = server_with(Arc::new(port.clone())).await;
    let work = server.import(&mills(10, 3000), "muehlen.typ").await.work;
    let keys = keys_of(&server, work).await;
    ok(&server, "persona.update", json!({ "handle": "@lektor", "context_length": 4096 })).await;
    let mut chapter = order(work, keys[0]);
    chapter.scope = Scope::Chapter;
    let outcome = baruch_server::assist::run(&server.state, &chapter, None)
        .await
        .expect("mit einer Zeile je Folge ausgelassener Teile passt das Kapitel gestückelt");
    assert!(outcome.compressed.as_ref().is_some_and(|c| c.chunks >= 2), "gestückelt geprüft: {:?}", outcome.compressed);
    let calls = port.calls.lock().unwrap().clone();
    for call in &calls {
        // The same formula as `context::prepare`: (context length − max_tokens − reserve 256) · 3 characters.
        let size: usize = call.messages.iter().map(|m| m.content.chars().count()).sum();
        let budget = (4096u32.saturating_sub(call.max_tokens).saturating_sub(256) * 3) as usize;
        assert!(size <= budget, "Aufruf {} mit {size} Zeichen über dem Budget {budget}", call.label);
    }
    let head = baruch_server::assist::context::OMITTED_HEAD;
    let merged = calls
        .iter()
        .flat_map(|c| c.messages.iter())
        .any(|m| m.content.lines().any(|line| line.starts_with(&format!("--- {head}: ")) && line.contains("; ")));
    assert!(merged, "mindestens eine Zeile nennt mehrere ausgelassene Teile, durch „; “ getrennt");
}

/// Stream events from SSE text.
fn events(text: &str) -> Vec<(String, Value)> {
    text.split("\n\n")
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

#[tokio::test]
async fn a_retry_in_a_split_review_discards_only_the_running_piece() {
    let _turn = common::model_queue(module_path!()).await;
    let port = Counter::default();
    let server = server_with(Arc::new(port.clone())).await;
    let work = server.import(HIVE, "bienen.typ").await.work;
    let keys = keys_of(&server, work).await;
    ok(&server, "revision.save", json!({ "work": work, "key": keys[2], "body": long_water(600) })).await;
    // Round 28: „Kritisch:“ is requested again only for Nathan (auditor) any more; the lector is a companion.
    ok(&server, "persona.update", json!({ "handle": "@nathan", "context_length": 3200 })).await;
    let request = Request::post("/assistenten/stream")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({ "method": "persona.review", "work": work, "key": keys[2], "persona": "@nathan" }).to_string(),
        ))
        .unwrap();
    let response = server.app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK, "Strom angenommen");
    let mut body = response.into_body().into_data_stream();
    let mut text = String::new();
    while let Some(chunk) = body.next().await {
        text.push_str(std::str::from_utf8(&chunk.unwrap()).unwrap());
    }
    let got = events(&text);
    let names: Vec<&str> = got.iter().map(|(n, _)| n.as_str()).collect();
    let compressed =
        got.iter().find(|(n, _)| n == "compressed").unwrap_or_else(|| panic!("compressed fehlt: {names:?}"));
    assert!(
        compressed.1["chunks"].as_u64().unwrap() >= 2,
        "der lange Abschnitt wird in Stücken geprüft: {compressed:?}"
    );
    let piece = names.iter().position(|n| *n == "piece").unwrap_or_else(|| panic!("piece fehlt: {names:?}"));
    let retry = names.iter().position(|n| *n == "retry").unwrap_or_else(|| panic!("retry fehlt: {names:?}"));
    assert_eq!(got[piece].1["index"], 2, "das zweite Stück beginnt: {:?}", got[piece]);
    assert!(piece < retry, "die Nachforderung kommt im zweiten Stück, nach seinem Beginn: {names:?}");
    let first_piece: String =
        got[..piece].iter().filter(|(n, _)| n == "delta").map(|(_, d)| d["text"].as_str().unwrap_or("")).collect();
    assert!(
        first_piece.contains("Tränke 1 am Bach"),
        "das fertige erste Stück liegt vor dem Ereignis piece – der Client behält es beim Retry: {first_piece:?}"
    );
    // The client (assist.ts): `piece` begins a new part, `retry` empties only this one.
    let script = include_str!("../web/src/assist.ts");
    let on_piece = script.find("event === \"piece\"").expect("assist.ts behandelt piece");
    let on_retry = script.find("event === \"retry\"").expect("assist.ts behandelt retry");
    assert!(script[on_piece..on_retry].contains("segment = document.createElement(\"span\")"), "neuer Teil je Stück");
    assert!(script[on_retry..].contains("segment.textContent = \"\";"), "retry leert nur den laufenden Teil");
}

/// The mock with the document `lt-1` including title.
fn with_document(mock: &MockMultigpt, text: &str) {
    let text = text.to_owned();
    mock.configure(move |b| {
        b.texts.insert("lt-1".into(), text);
        b.documents
            .insert("lt-1".into(), json!({ "title": "Das Jahr am Leuchtturm", "url": "https://bib.example/lt-1" }));
    });
}

fn form_value(text: &str) -> String {
    text.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' => (b as char).to_string(),
            b' ' => "+".to_owned(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

#[tokio::test]
async fn the_multigpt_page_gives_the_text_to_a_persona_without_adopting_a_title() {
    let _turn = common::model_queue(module_path!()).await;
    let port = Counter::default();
    let server = server_with(Arc::new(port.clone())).await;
    let mock = MockMultigpt::start().await;
    with_document(&mock, "Das Leuchtfeuer brennt jede Nacht. Der Wärter notiert das Wetter.");
    ok(&server, "connection.set", json!({ "kind": "multigpt", "config": { "url": mock.url() } })).await;
    let work = server.import(HIVE, "bienen.typ").await.work;
    let keys = keys_of(&server, work).await;
    let key = keys[1];

    let (status, page) =
        server.get(&format!("/quelle/multigpt?work={work}&key={key}&collection=Leuchtturm&document=lt-1")).await;
    assert_eq!(status, StatusCode::OK, "Seite da: {page}");
    assert!(page.contains("data-give=\"multigpt\""), "Formular „An Persona geben“ mit Abschnitt: {page}");
    assert!(page.contains("value=\"persona\""), "Knopf action=persona (ohne JavaScript): {page}");
    assert!(
        page.contains("Das Leuchtfeuer brennt jede Nacht. Der Wärter notiert das Wetter.</textarea>"),
        "der Text des gezeigten Teils ist vorbelegt: {page}"
    );
    assert!(
        page.contains("name=\"source\" value=\"MultiGPT: Das Jahr am Leuchtturm (Dokument lt-1)\""),
        "Quelle aus Titel und Dokument: {page}"
    );
    assert!(!page.contains("name=\"title\""), "kein Titel der Bibliothek im Formular: {page}");
    let (_, without_key) =
        server.get(&format!("/quelle/multigpt?work={work}&collection=Leuchtturm&document=lt-1")).await;
    assert!(without_key.contains("data-give=\"no-key\""), "ohne Abschnitt nur der Hinweis: {without_key}");

    let revisions = |w: &baruch_core::Work| w.nodes.iter().map(|n| n.revisions.len()).sum::<usize>();
    let before = revisions(&server.state.store.load_work(work).await.unwrap());
    let form = format!(
        "action=persona&work={work}&key={key}&persona=%40lektor&source={}&text={}&question={}",
        form_value("MultiGPT: Das Jahr am Leuchtturm (Dokument lt-1)"),
        form_value("Der Wärter notiert das Wetter."),
        form_value("Passt das zu den Pollen?")
    );
    let (status, location) = server.post_form("/literatur/act", &form).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "zurück zum Abschnitt: {location:?}");
    let loaded = server.state.store.load_work(work).await.unwrap();
    let node = loaded.nodes.iter().find(|n| n.key == key).unwrap();
    let answer = node.posts.iter().find(|p| p.author == "@lektor").expect("die Persona hat geantwortet");
    assert!(
        answer.sources.iter().any(|s| s == "MultiGPT: Das Jahr am Leuchtturm (Dokument lt-1)"),
        "die Antwort trägt die Quelle aus MultiGPT: {:?}",
        answer.sources
    );
    let question = node.posts.iter().find(|p| p.author != "@lektor").expect("die Frage steht als Beitrag");
    assert!(question.body.contains("Der Wärter notiert das Wetter."), "Frage mit dem Text: {}", question.body);
    let remote =
        baruch_library::RemoteRef { way: "multigpt".into(), collection: "Leuchtturm".into(), document: "lt-1".into() };
    assert_eq!(
        baruch_library::title_for_remote(server.state.store.pool(), &remote).await.unwrap(),
        None,
        "kein Titel übernommen: „an Persona geben“ braucht keinen Eintrag in der Bibliothek"
    );
    assert_eq!(revisions(&loaded), before, "am Buchtext ändert sich nichts (keine neue Fassung)");
}
