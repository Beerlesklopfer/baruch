// SPDX-License-Identifier: AGPL-3.0-or-later
//! Follow-up fix Round 15 (findings of the review): chunked context measured instead of only estimated, context length
//! of Ollama never the maximum of the model, no embedding through an Eliezer marked as external, location
//! "local/external" by the address with a warning, no framing (clickjacking).
//!
//! No network, no Eliezer: own mocks (`Recorder` below, `baruch_llm::mock::MockEliezer` on 127.0.0.1:0). Addresses such
//! as `api.example.com` are only stored and displayed, never called. All texts freely invented (a small book about
//! lighthouses).

use crate::common;

use std::sync::{Arc, Mutex};

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use baruch_llm::mock::{Flavor, MockEliezer};
use baruch_server::assist::port::{BoxFuture, ModelAnswer, ModelCall, ModelFailure, ModelList, ModelPort};
use baruch_server::assist::{Mode, Order, Scope};
use serde_json::{Value, json};

async fn ok(server: &common::Server, method: &str, params: Value) -> Value {
    let answer = server.rpc(method, params).await;
    assert!(answer["error"].is_null(), "{method} sollte gelingen: {answer}");
    answer["result"].clone()
}

/// Mock of a model that records every call: summaries get a short text, checks a finding.
#[derive(Clone, Default)]
struct Recorder {
    calls: Arc<Mutex<Vec<ModelCall>>>,
}

impl Recorder {
    fn calls(&self) -> Vec<ModelCall> {
        self.calls.lock().unwrap().clone()
    }
}

impl ModelPort for Recorder {
    fn chat<'a>(
        &'a self,
        _state: &'a baruch_server::AppState,
        call: ModelCall,
        _deltas: tokio::sync::mpsc::UnboundedSender<String>,
    ) -> BoxFuture<'a, Result<ModelAnswer, ModelFailure>> {
        Box::pin(async move {
            let text = if call.label.contains("Zusammenfassung") {
                "Der Wärter steigt die Treppe hinauf und prüft das Licht.".to_owned()
            } else {
                "Kritisch: „Licht“ steht in jedem Satz.\n- Schritt: prüfen.\n- Der Schluss fehlt.".to_owned()
            };
            self.calls.lock().unwrap().push(call.clone());
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

/// Work with one chapter of `sections` sections of about `chars` characters each (invented text).
fn lighthouse(sections: usize, chars: usize) -> String {
    let mut text = String::from(
        "#set document(title: \"Die Leuchttürme\", author: \"Mara Klippe\")\n#set heading(numbering: \"1.1\")\n\n\
         = Die Küste\n\nAm Abend gehen die Lichter an.\n\n",
    );
    for i in 1..=sections {
        let sentence = format!("Turm {i} wirft sein Licht weit über das dunkle Wasser hinaus. ");
        let body = sentence.repeat(chars / sentence.chars().count() + 1);
        text.push_str(&format!("== Turm {i}\n\n{}\n\n", body.trim_end()));
    }
    text
}

async fn coast(model: Recorder, sections: usize, chars: usize) -> (common::Server, i64, Vec<i64>) {
    let data = tempfile::tempdir().unwrap();
    let state = baruch_server::AppState::open_with(
        data.path(),
        Arc::new(baruch_server::connections::MemorySecrets::default()),
        Arc::new(model),
    )
    .await
    .expect("Datenordner lässt sich nicht öffnen");
    let state = Arc::new(state);
    let server = common::Server { app: common::router(state.clone()), state, data };
    let work = server.import(&lighthouse(sections, chars), "kueste.typ").await.work;
    let keys = server.state.store.load_work(work).await.unwrap().nodes.iter().map(|n| n.key).collect();
    (server, work, keys)
}

fn chapter_review(work: i64, key: i64) -> Order {
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
        scope: Scope::Chapter,
        frame: Default::default(),
        links: Default::default(),
        basis: None,
    }
}

/// Input of a call in characters and the budget by the same formula as `context::prepare` ((context length − max_tokens
/// − reserve 256) · 3 characters).
fn size_and_budget(call: &ModelCall, context: u32) -> (usize, usize) {
    let size = call.messages.iter().map(|m| m.content.chars().count()).sum();
    (size, (context.saturating_sub(call.max_tokens).saturating_sub(256) * 3) as usize)
}

// ---- Chunked context: measured, never over the context length -----------------------------------------------

#[tokio::test]
async fn a_chapter_with_many_parts_never_sends_a_call_over_the_context_length() {
    baruch_server::i18n::scope(baruch_server::i18n::Lang::De, async {
        // Recalculation of the finding: check the whole chapter, 10 sections of 3000 characters each → 20 overlapping
        // parts. Before, at 4096 tokens one piece went out with ≈ 5655 instead of at most ≈ 4400 characters of surroundings
        // and core. Since Round 18b consecutive omitted parts share one line; at 4096 tokens the case now fits chunked
        // (measured in `round18_lektor::omitted_parts_in_one_line_…`). Hence a smaller context here (CONTEXT) at which even
        // the condensed lower bound does not fit: the case stays an honest rejection. At 2600 tokens the room is not even
        // enough for role prompt and job (different message), at 4096 it fits. Round 30b: the solution obligation lengthens
        // every default prompt (≈ 150 tokens); 3000 tokens therefore already fall below the limit for role prompt and job,
        // 3200 lie in the intended range again.
        const CONTEXT: u32 = 3200;
        let recorder = Recorder::default();
        let (server, work, keys) = coast(recorder.clone(), 10, 3000).await;
        ok(&server, "persona.update", json!({ "handle": "@lektor", "context_length": CONTEXT })).await;
        let posts_before = server.state.store.load_work(work).await.unwrap().nodes[0].posts.len();
        let result = baruch_server::assist::run(&server.state, &chapter_review(work, keys[0]), None).await;

        // Even with all other parts only as titles (or an empty summary) every piece bursts the room: the lower bound is
        // known before every model call, so reject honestly – without summaries for nothing.
        let e = result.expect_err("20 Teile passen bei so kleinem Kontext auch komprimiert nicht");
        assert_eq!(e.code, -32000, "ehrlicher Fehler: {e:?}");
        assert!(
            e.message.contains("reicht auch komprimiert nicht") && e.message.contains("Nichts gespeichert"),
            "nennt den Grund und dass nichts gespeichert ist: {}",
            e.message
        );
        assert!(e.message.contains(&format!("{CONTEXT} Tokens")), "nennt die Kontextlänge: {}", e.message);
        assert!(
            recorder.calls().is_empty(),
            "kein Aufruf ging hinaus (weder ein zu großes Prüfstück noch nutzlose Zusammenfassungen): {:?}",
            recorder.calls().iter().map(|c| c.label.clone()).collect::<Vec<_>>()
        );
        let posts_after = server.state.store.load_work(work).await.unwrap().nodes[0].posts.len();
        assert_eq!(posts_after, posts_before, "nichts gespeichert");
    })
    .await
}

#[tokio::test]
async fn a_chapter_that_fits_compressed_keeps_every_piece_and_summary_inside_the_budget() {
    // Middle case: enough context for pieces with surroundings. Every call – also the head „Stück k von n (…)“, the
    // addition in the job and the lines „wird geprüft, steht unten“ – fits, measured, into the budget, and everything
    // is checked.
    let recorder = Recorder::default();
    let (server, work, keys) = coast(recorder.clone(), 6, 2500).await;
    ok(&server, "persona.update", json!({ "handle": "@lektor", "context_length": 6144 })).await;
    let outcome = baruch_server::assist::run(&server.state, &chapter_review(work, keys[0]), None)
        .await
        .expect("mit 6144 Tokens passt das Kapitel gestückelt");
    let compressed = outcome.compressed.clone().expect("komprimiert, sonst prüfte dieser Test nichts");
    let calls = recorder.calls();
    let reviews: Vec<&ModelCall> = calls.iter().filter(|c| !c.label.contains("Zusammenfassung")).collect();
    assert_eq!(compressed.chunks, reviews.len(), "Zahl der Stücke wie gemeldet");
    for call in &calls {
        let (size, budget) = size_and_budget(call, 6144);
        assert!(size <= budget, "{}: {size} Zeichen > Budget {budget}", call.label);
    }
    for i in 1..=6 {
        let probe = format!("Turm {i} wirft sein Licht");
        assert!(
            reviews.iter().any(|c| c.messages.last().unwrap().content.contains(&probe)),
            "Abschnitt {i} wurde in einem Stück im Wortlaut geprüft (nichts still verloren)"
        );
    }
}

// ---- Ollama: effective context length instead of maximum -----------------------------------------------------

#[tokio::test]
async fn ollama_without_num_ctx_reports_its_default_and_not_the_model_maximum() {
    let data = tempfile::tempdir().unwrap();
    let state = baruch_server::AppState::open_with(
        data.path(),
        Arc::new(baruch_server::connections::MemorySecrets::default()),
        Arc::new(baruch_server::assist::eliezer::EliezerPort),
    )
    .await
    .unwrap();
    let state = Arc::new(state);
    let server = common::Server { app: common::router(state.clone()), state, data };
    let parked = MockEliezer::start().await;
    ok(&server, "connection.set", json!({ "kind": "eliezer", "config": { "url": parked.url() } })).await;
    let ollama = MockEliezer::start().await;
    ollama.set_flavor(Flavor::Ollama);
    ollama.set_models(&["moewe:8b"]);
    // No num_ctx set: the mock reports the maximum 131072 in model_info and no loaded model under /api/ps – like a
    // fresh Ollama without Modelfile parameters.
    let made = ok(
        &server,
        "model.connection_set",
        json!({ "name": "Ollama", "kind": "ollama", "url": ollama.url(), "external": false }),
    )
    .await;
    ok(&server, "persona.update", json!({ "handle": "@exeget", "model": "moewe:8b", "context_length": 32768 })).await;
    ok(&server, "persona.backend_set", json!({ "persona": "@exeget", "connection": made["id"] })).await;
    let budget = ok(&server, "model.context", json!({ "persona": "@exeget" })).await;
    assert_eq!(
        (budget["tokens"].as_u64(), budget["reported"].as_u64()),
        (Some(u64::from(baruch_llm::client::OLLAMA_DEFAULT_CONTEXT)), Some(4096)),
        "über /v1/chat/completions läuft Ollama ohne num_ctx mit seiner Vorgabe; mit 32768 (Einstellung) oder \
         131072 (Höchstwert) würde Ollama längere Aufrufe still vorn abschneiden: {budget}"
    );
    let warning = budget["warning"].as_str().unwrap_or("");
    assert!(warning.contains("32768") && warning.contains("4096"), "die Warnung nennt beide Längen: {warning}");
    let paths = ollama.paths();
    assert!(
        paths.iter().any(|p| p == "/api/show") && paths.iter().any(|p| p == "/api/ps"),
        "erst /api/show (num_ctx), dann /api/ps (geladene Länge): {paths:?}"
    );
    assert!(ollama.chat_bodies().is_empty(), "nur gelesen, kein Chat");
}

// ---- Embedding through an Eliezer marked as external ----------------------------------------------------------

#[tokio::test]
async fn an_eliezer_marked_external_receives_no_book_text_for_embeddings() {
    let data = tempfile::tempdir().unwrap();
    let state = baruch_server::AppState::open_with(
        data.path(),
        Arc::new(baruch_server::connections::MemorySecrets::default()),
        Arc::new(baruch_server::assist::eliezer::EliezerPort),
    )
    .await
    .unwrap();
    let state = Arc::new(state);
    let server = common::Server { app: common::router(state.clone()), state, data };
    let eliezer = MockEliezer::start().await;
    ok(&server, "model.connection_set", json!({ "id": 1, "external": true })).await;
    ok(&server, "connection.set", json!({ "kind": "eliezer", "config": { "url": eliezer.url() } })).await;
    let embedder =
        baruch_server::rag::eliezer_embedders(&server.state).for_model("text-embedding-nomic-embed-text-v1.5");
    let texts = vec!["Der Leuchtturm steht seit hundert Jahren auf der Klippe.".to_owned()];
    let refused = baruch_server::i18n::scope(baruch_server::i18n::Lang::De, embedder.embed(&texts))
        .await
        .expect_err("extern ohne Freigabe: nichts einbetten");
    let message = refused.to_string();
    assert!(message.contains("extern") && message.contains("Nichts gesendet"), "ehrlicher Grund: {message}");
    assert!(
        !eliezer.paths().iter().any(|p| p.ends_with("/embeddings")),
        "der Nachbau sah keine Einbettung – kein Buchtext ging hinaus: {:?}",
        eliezer.paths()
    );

    ok(&server, "model.connection_set", json!({ "id": 1, "external": false })).await;
    let done = embedder.embed(&texts).await.expect("lokal: Einbetten wie bisher");
    assert_eq!(done.vectors.len(), 1, "ein Vektor je Text");
    assert!(eliezer.paths().iter().any(|p| p.ends_with("/embeddings")), "jetzt geht die Einbettung hinaus");
}

// ---- Location by the address ---------------------------------------------------------------------------------

#[test]
fn only_loopback_private_and_local_names_look_local() {
    use baruch_server::models::looks_local;
    for local in [
        "http://127.0.0.1:1234",
        "http://localhost:11434",
        "http://[::1]:8080/v1",
        "http://192.168.1.20:11434",
        "http://10.0.0.5/v1",
        "http://172.20.0.2:8000",
        "http://kellerrechner:11434",
        "http://ollama.lan/v1",
        "http://gpu.home.arpa",
        "http://[fd12:3456::1]:11434",
    ] {
        assert!(looks_local(local), "{local} liegt erkennbar auf diesem Rechner bzw. im eigenen Netz");
    }
    for far in ["https://api.example.com/v1", "http://8.8.8.8", "http://172.32.0.1", "https://[2001:db8::1]", "kaputt"]
    {
        assert!(!looks_local(far), "{far} ist nicht erkennbar lokal (kein DNS, im Zweifel extern)");
    }
}

#[tokio::test]
async fn a_new_connection_without_a_choice_follows_the_address_and_a_public_one_marked_local_warns() {
    let server = common::open().await;
    let far = ok(
        &server,
        "model.connection_set",
        json!({ "name": "Ferner Dienst", "kind": "openai", "url": "https://api.example.com/v1" }),
    )
    .await;
    assert_eq!(far["locality"], "extern", "ohne Angabe: nicht erkennbar lokal → extern (Sperre greift): {far}");
    let near = ok(
        &server,
        "model.connection_set",
        json!({ "name": "Nebenan", "kind": "ollama", "url": "http://127.0.0.1:9" }),
    )
    .await;
    assert_eq!(near["locality"], "lokal", "Loopback ohne Angabe → lokal: {near}");
    let moved =
        ok(&server, "model.connection_set", json!({ "id": near["id"], "url": "https://api.example.com/v1" })).await;
    assert_eq!(moved["locality"], "extern", "neue, nicht lokale Adresse ohne Angabe → extern: {moved}");

    let chosen = ok(
        &server,
        "model.connection_set",
        json!({ "name": "Bewusst lokal", "kind": "openai", "url": "https://api.example.com/v1", "external": false }),
    )
    .await;
    assert_eq!(chosen["locality"], "lokal", "ausdrücklich lokal bleibt lokal (z. B. eigener Name im Intranet)");
    let list = ok(&server, "model.connections", json!({})).await;
    let entry = list["connections"]
        .as_array()
        .and_then(|l| l.iter().find(|c| c["id"] == chosen["id"]).cloned())
        .unwrap_or_else(|| panic!("Verbindung in der Liste: {list}"));
    assert!(
        entry["locality_warning"].as_str().is_some_and(|w| w.contains("ohne Freigabe")),
        "als lokal geführt, Adresse nicht erkennbar lokal: Warnung: {entry}"
    );
    assert!(
        list.to_string().matches("locality_warning").count() == 1,
        "nur diese eine Verbindung warnt (Eliezer auf Loopback, ferner Dienst extern): {list}"
    );
    let (status, page) = server.get("/einstellungen/verbindungen").await;
    assert_eq!(status, StatusCode::OK);
    assert!(page.contains("data-locality-warning"), "die Seite zeigt die Warnung ohne JavaScript");
    assert!(page.contains("<option value=\"auto\" selected>"), "beim Anlegen ist „nach der Adresse“ vorgewählt");

    let (status, _) = server
        .post_form(
            "/einstellungen/modelle/act",
            "action=save&name=Formular&kind=openai&url=https%3A%2F%2Fapi.example.com%2Fv1&external=auto",
        )
        .await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    let list = ok(&server, "model.connections", json!({})).await;
    assert!(list.to_string().contains("\"name\":\"Formular\""), "angelegt: {list}");
    let form_made =
        list["connections"].as_array().and_then(|l| l.iter().find(|c| c["name"] == "Formular").cloned()).unwrap();
    assert_eq!(form_made["locality"], "extern", "Formular ohne JavaScript mit „nach der Adresse“ → extern");
}

// ---- Clickjacking ---------------------------------------------------------------------------------------------

#[tokio::test]
async fn no_page_may_be_framed_by_another_site() {
    let server = common::open().await;
    for uri in ["/", "/gliederung", "/einstellungen/assistenten", "/einstellungen/verbindungen", "/static/app.css"] {
        let (status, headers, _) = server.send(Request::get(uri).body(Body::empty()).unwrap()).await;
        assert_ne!(status, StatusCode::NOT_FOUND, "{uri} gibt es");
        assert_eq!(
            headers.get(header::X_FRAME_OPTIONS).and_then(|v| v.to_str().ok()),
            Some("DENY"),
            "{uri}: X-Frame-Options gegen Einrahmen (ältere Browser)"
        );
        let csp = headers.get(header::CONTENT_SECURITY_POLICY).and_then(|v| v.to_str().ok()).unwrap_or("");
        assert!(csp.contains("frame-ancestors 'none'"), "{uri}: CSP frame-ancestors (heutige Browser): {csp}");
    }
    // A rejected request (foreign origin) carries the headers too.
    let request = Request::post("/gliederung/act")
        .header(header::ORIGIN, "http://boese.example")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from("decision=confirm"))
        .unwrap();
    let (status, headers, _) = server.send(request).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "CSRF-Schutz wie bisher");
    assert_eq!(headers.get(header::X_FRAME_OPTIONS).and_then(|v| v.to_str().ok()), Some("DENY"));
}
