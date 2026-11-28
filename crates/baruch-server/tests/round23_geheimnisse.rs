// SPDX-License-Identifier: AGPL-3.0-or-later
//! Round 23 „Härtung“ (ownership geheimnisse): secrets and scrub, redirect policy, filter for fetch_url/web_search,
//! limits on reading in, temp folder for EPUBCheck, `confirm` for `model.connection_delete`.
//!
//! Works freely invented; no Eliezer, no network (models only through mocks on 127.0.0.1:0). Every assertion with a
//! reason. Further files as submodules under `tests/round23_geheimnisse/*.rs`, included with
//! `#[path = "round23_geheimnisse/x.rs"] mod x;` – `tests/main.rs` stays unchanged.
//!
//! The temp folder for EPUBCheck is checked in the module (`epub_check::tests`), `baruch_llm::scrub` and the redirects
//! of the model client in `baruch-llm` (`secret.rs`, `tests/redirect.rs`), `fetch_allowed` as a table in
//! `websearch::fetch::tests`.

use crate::common;

use std::sync::{Arc, Mutex};

use baruch_exchange::sample::{sample_document, sample_document_u8, sample_uuid};
use baruch_exchange::write::{ImageMode, write_xml};
use baruch_exchange::{ExchangeDocument, ExchangeNode, ExchangeRevision};
use baruch_llm::mock::{Flavor, MockEliezer};
use baruch_rag::RagError;
use baruch_server::AppState;
use baruch_server::assist::eliezer::EliezerPort;
use baruch_server::connections::MemorySecrets;
use baruch_server::websearch::mock::{MockSite, MockWebMcp};
use baruch_store::Store;
use baruch_store::exchange_apply::{ApplyMode, DeviationAction, derive_uuid};
use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

/// Invented secret; „Wmlubm9iZXItNDcxMQ==“ is its Base64 (`printf … | base64`), „Zinnober%2D4711“ its percent form. It
/// must be in no response.
const SECRET: &str = "Zinnober-4711";
const SECRET_BASE64: &str = "Wmlubm9iZXItNDcxMQ==";
const SHA: &str = "0000000000000000000000000000000000000000000000000000000000000023";

async fn ok(server: &common::Server, method: &str, params: Value) -> Value {
    let answer = server.rpc(method, params).await;
    assert!(answer["error"].is_null(), "{method} sollte gelingen: {answer}");
    answer["result"].clone()
}

async fn err(server: &common::Server, method: &str, params: Value) -> Value {
    let answer = server.rpc(method, params).await;
    assert!(answer["error"].is_object(), "{method} sollte scheitern: {answer}");
    answer["error"].clone()
}

/// Server with the real model adapter; Eliezer points to an own mock (never 127.0.0.1:1234).
pub(crate) async fn with_models() -> (common::Server, MockEliezer) {
    let data = tempfile::tempdir().unwrap();
    let state = AppState::open_with(data.path(), Arc::new(MemorySecrets::default()), Arc::new(EliezerPort))
        .await
        .expect("Datenordner öffnen");
    let state = Arc::new(state);
    let server = common::Server { app: common::router(state.clone()), state, data };
    let eliezer = MockEliezer::start().await;
    ok(&server, "connection.set", json!({ "kind": "eliezer", "config": { "url": eliezer.url() } })).await;
    (server, eliezer)
}

async fn store() -> (tempfile::TempDir, Store) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).await.unwrap();
    (dir, store)
}

// ---- Mock for redirects ---------------------------------------------------------------------------

/// Small HTTP mock: answers every request with `answer(pfad)` and remembers the raw request.
pub(crate) struct Recorder {
    pub(crate) url: String,
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

    pub(crate) fn requests(&self) -> Vec<String> {
        self.seen.lock().unwrap().clone()
    }
}

/// First mock (redirect to the same path at the second) and second mock (answers 200).
pub(crate) async fn redirecting(status: &'static str) -> (Recorder, Recorder) {
    let target = Recorder::start(|_| {
        let body = r#"{"data":[{"id":"fremdes-modell"}]}"#;
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

fn untouched(what: &str, answer: &Value, first: &Recorder, target: &Recorder) {
    assert!(
        first.requests().iter().any(|r| r.contains(&format!("Bearer {SECRET}"))),
        "{what}: die eingetragene Adresse bekam die Anfrage mit dem Schlüssel (sonst prüfte der Test nichts)"
    );
    assert!(
        target.requests().is_empty(),
        "{what}: der zweite Nachbau sieht keine Anfrage und damit keinen Schlüssel – der Weiterleitung wird nicht gefolgt"
    );
    let text = answer.to_string();
    assert!(!text.contains(SECRET), "{what}: der Schlüssel steht nicht im Ergebnis: {text}");
}

// ---- Scrub in one place -----------------------------------------------------------------------------

#[test]
fn the_scrubbers_of_all_ways_mask_encoded_secrets_with_the_shared_function() {
    use baruch_server::connections::{check, multigpt};
    let echo = format!("abgelehnt: Basic {SECRET_BASE64} und key=Zinnober%2d4711");
    assert_eq!(
        multigpt::scrub_text(&echo, Some(SECRET)),
        "abgelehnt: Basic *** und key=***",
        "MultiGPT (und damit Abgleich und Websuche über die Sitzung): Base64 und Prozentform sind geschwärzt"
    );
    assert_eq!(
        multigpt::scrub_text(&echo, Some(SECRET)),
        baruch_llm::scrub(&echo, Some(SECRET)),
        "derselbe Text wie die eine Funktion in baruch-llm"
    );
    let error = multigpt::scrub_error(RagError::Denied(echo.clone()), Some(SECRET));
    assert_eq!(
        error.code(),
        RagError::Denied(String::new()).code(),
        "die Fehlerart bleibt (pgvector nutzt diesen Weg)"
    );
    assert_eq!(error.message(), "abgelehnt: Basic *** und key=***", "Fehlermeldung geschwärzt (MultiGPT, pgvector)");
    let value = check::scrub_value(json!({ "server": { "name": echo }, SECRET_BASE64: [SECRET] }), Some(SECRET));
    assert_eq!(
        value,
        json!({ "server": { "name": "abgelehnt: Basic *** und key=***" }, "***": ["***"] }),
        "„Verbindung prüfen“: Texte, Listen und Schlüssel der Angaben"
    );
    assert_eq!(multigpt::scrub_text("kurz abc", Some("abc")), "kurz ***", "kurze Geheimnisse weiter wörtlich");
}

#[tokio::test]
async fn checking_a_connection_masks_a_base64_echo_of_the_key() {
    let (server, eliezer) = with_models().await;
    // The peer mirrors the key back encoded (here as a model name).
    let echoed = format!("echo-{SECRET_BASE64}");
    eliezer.set_models(&[echoed.as_str(), "echo-Zinnober%2D4711"]);
    ok(&server, "connection.set_secret", json!({ "kind": "eliezer", "secret": SECRET })).await;
    let checked = ok(&server, "connection.check", json!({ "kind": "eliezer" })).await;
    let text = checked.to_string();
    assert!(text.contains("***"), "die gespiegelten Namen sind geschwärzt: {text}");
    assert!(
        !text.contains("Wmlubm9iZXItNDcx") && !text.contains("Zinnober"),
        "weder Base64 noch Prozentform des Schlüssels stehen im Prüfergebnis: {text}"
    );
    let stored = ok(&server, "connection.get", json!({ "kind": "eliezer" })).await.to_string();
    assert!(
        !stored.contains("Wmlubm9iZXItNDcx") && !stored.contains("Zinnober"),
        "auch das gespeicherte Prüfergebnis ist geschwärzt: {stored}"
    );
}

// ---- The check follows no redirect --------------------------------------------------------------------

#[tokio::test]
async fn checking_eliezer_does_not_follow_a_redirect() {
    let server = common::open().await;
    let (first, target) = redirecting("302 Found").await;
    ok(&server, "connection.set", json!({ "kind": "eliezer", "config": { "url": first.url } })).await;
    ok(&server, "connection.set_secret", json!({ "kind": "eliezer", "secret": SECRET })).await;
    let checked = ok(&server, "connection.check", json!({ "kind": "eliezer" })).await;
    assert_eq!(checked["ok"], false, "eine Weiterleitung ist kein Erfolg: {checked}");
    let message = checked["message"].as_str().unwrap_or("");
    assert!(
        message.contains("leitet weiter") && message.contains("302"),
        "die Meldung nennt die Weiterleitung, damit der Mensch die Adresse berichtigt: {message}"
    );
    assert!(!message.contains(&target.url), "das Ziel der Weiterleitung wird nicht ungeprüft angezeigt: {message}");
    untouched("Eliezer prüfen", &checked, &first, &target);
}

#[tokio::test]
async fn checking_multigpt_does_not_follow_a_redirect() {
    let server = common::open().await;
    // 307 keeps method and body – precisely then `initialize` including the token would go to the foreign address.
    let (first, target) = redirecting("307 Temporary Redirect").await;
    ok(&server, "connection.set", json!({ "kind": "multigpt", "config": { "url": format!("{}/mcp", first.url) } }))
        .await;
    ok(&server, "connection.set_secret", json!({ "kind": "multigpt", "secret": SECRET })).await;
    let checked = ok(&server, "connection.check", json!({ "kind": "multigpt" })).await;
    assert_eq!(checked["ok"], false, "eine Weiterleitung ist kein Erfolg: {checked}");
    let message = checked["message"].as_str().unwrap_or("");
    assert!(message.contains("leitet weiter") && message.contains("307"), "Weiterleitung benannt: {message}");
    untouched("MultiGPT prüfen", &checked, &first, &target);
}

#[tokio::test]
async fn checking_a_model_connection_does_not_follow_a_redirect() {
    let (server, _eliezer) = with_models().await;
    let (first, target) = redirecting("308 Permanent Redirect").await;
    let made = ok(
        &server,
        "model.connection_set",
        json!({ "name": "Umzug", "kind": "openai", "url": first.url, "external": false }),
    )
    .await;
    let id = made["id"].as_i64().unwrap();
    ok(&server, "model.connection_set_secret", json!({ "id": id, "secret": SECRET })).await;
    let checked = ok(&server, "model.connection_check", json!({ "id": id })).await;
    assert_eq!(checked["ok"], false, "eine Weiterleitung ist kein Erfolg: {checked}");
    let message = checked["message"].as_str().unwrap_or("");
    assert!(message.contains("Weiterleitung") && message.contains("308"), "Weiterleitung benannt: {message}");
    untouched("Modell-Verbindung prüfen (baruch-llm)", &checked, &first, &target);
}

// ---- Filter before fetch_url -----------------------------------------------------------------------------

fn hit(title: &str, url: &str) -> Value {
    json!({ "title": title, "url": url, "content": format!("Ausschnitt zu {title}") })
}

fn fetched(mcp: &MockWebMcp) -> Vec<String> {
    mcp.tool_calls("fetch_url").iter().map(|a| a["url"].as_str().unwrap_or("").to_owned()).collect()
}

#[tokio::test]
async fn search_hits_with_addresses_of_the_own_network_are_not_fetched_and_appear_with_a_note() {
    const LIGHT: &str = "https://lichtkunde.example/feuer";
    const ARCHIVE: &str = "https://archiv.example/chronik";
    let private = [
        "http://192.168.178.1/einstellungen",
        "http://127.0.0.1:8765/mcp",
        "http://localhost:8765/einstellungen/verbindungen",
        "http://[::ffff:10.0.0.7]/intern",
        "http://nas.home.arpa/freigabe",
        "https://leser:kennwort@archiv.example/chronik",
    ];
    let server = common::open().await;
    let site = MockSite::with_fixtures().await;
    let mcp = MockWebMcp::start(Some(&site)).await;
    ok(&server, "connection.set", json!({ "kind": "multigpt", "config": { "url": mcp.url() } })).await;
    // The rejected hits are at the front: they must not use up any of the four fetches.
    let mut results: Vec<Value> = private.iter().map(|url| hit("Intern", url)).collect();
    results.push(hit("Licht", LIGHT));
    results.push(hit("Chronik", ARCHIVE));
    mcp.set_results(results);

    let research = baruch_server::i18n::scope(
        baruch_server::i18n::Lang::De,
        baruch_server::websearch::research(&server.state, "Feuer am Leuchtturm"),
    )
    .await;
    assert!(research.failure.is_none(), "die Websuche läuft durch: {research:?}");
    assert_eq!(
        fetched(&mcp),
        vec![LIGHT.to_owned(), ARCHIVE.to_owned()],
        "laut Anfrageprotokoll des Nachbaus gingen nur die öffentlichen Adressen an fetch_url – MultiGPT riefe die \
         anderen im Intranet ab"
    );
    for url in private {
        assert!(
            research.hits.iter().any(|h| h.url == url),
            "der Treffer bleibt ein Treffer (nur ohne Abruf): {url} in {:?}",
            research.hits
        );
        assert!(
            research.notes.iter().any(|n| n.contains("nicht abgerufen") && n.contains(url)),
            "zu {url} steht ein Hinweis mit Grund: {:?}",
            research.notes
        );
        assert!(!research.pages.iter().any(|p| p.url == url), "keine Seite zu {url}");
    }
    assert_eq!(research.pages.len(), 2, "beide öffentlichen Seiten sind abgerufen: {:?}", research.notes);

    // Exception way only for tests (per data folder, no setting): then exactly this host goes to fetch_url.
    baruch_server::websearch::fetch::set_fetch_exceptions(&server.state, &["192.168.178.1"]);
    let again = baruch_server::websearch::research(&server.state, "Feuer am Leuchtturm").await;
    assert!(
        fetched(&mcp).contains(&private[0].to_owned()),
        "mit der Test-Ausnahme wird der genannte Rechner abgerufen (der Nachbau lehnt ihn dann selbst ab): {:?}",
        again.notes
    );
    assert!(!fetched(&mcp).contains(&private[1].to_owned()), "andere Rechner bleiben abgelehnt");
    baruch_server::websearch::fetch::set_fetch_exceptions(&server.state, &[]);
}

// ---- model.connection_delete / _delete_secret only with confirm ----------------------------------------------

#[tokio::test]
async fn deleting_a_model_connection_or_its_key_needs_confirm_and_the_forms_still_work() {
    let (server, _eliezer) = with_models().await;
    let far = MockEliezer::start().await;
    let made = ok(
        &server,
        "model.connection_set",
        json!({ "name": "Nebenrechner", "kind": "openai", "url": far.url(), "external": false }),
    )
    .await;
    let id = made["id"].as_i64().unwrap();
    ok(&server, "model.connection_set_secret", json!({ "id": id, "secret": SECRET })).await;
    let names = |list: &Value| -> Vec<String> {
        list["connections"].as_array().unwrap().iter().map(|c| c["name"].as_str().unwrap_or("").to_owned()).collect()
    };

    for params in [json!({ "id": id }), json!({ "id": id, "confirm": false }), json!({ "id": id, "confirm": "true" })] {
        for method in ["model.connection_delete_secret", "model.connection_delete"] {
            let refused = err(&server, method, params.clone()).await;
            assert_eq!(refused["code"], -32602, "{method} ohne confirm: true ist ein Parameterfehler: {refused}");
            assert_eq!(refused["message"], "confirm: true erwartet", "{method}: die Meldung sagt, was fehlt");
        }
    }
    let list = ok(&server, "model.connections", json!({})).await;
    assert!(names(&list).contains(&"Nebenrechner".to_owned()), "nichts gelöscht: {list}");
    let mine = list["connections"].as_array().unwrap().iter().find(|c| c["id"] == id).unwrap().to_string();
    assert!(mine.contains("hinterlegt"), "der Schlüssel ist noch hinterlegt: {mine}");

    let gone = ok(&server, "model.connection_delete_secret", json!({ "id": id, "confirm": true })).await;
    assert_eq!(gone, json!({ "id": id, "secret": "fehlt" }), "mit confirm wie bisher");

    // Forms without JavaScript: mandatory checkbox instead of RPC parameter.
    let (status, location) = server.post_form("/einstellungen/modelle/act", &format!("action=delete&id={id}")).await;
    assert_eq!(status, 303, "Formular antwortet mit Weiterleitung");
    assert!(
        location.as_deref().unwrap_or("").contains("modellfehler=bestaetigen"),
        "ohne Kästchen lehnt das Formular ab: {location:?}"
    );
    assert!(names(&ok(&server, "model.connections", json!({})).await).contains(&"Nebenrechner".to_owned()), "noch da");
    let (status, location) =
        server.post_form("/einstellungen/modelle/act", &format!("action=delete&id={id}&confirm=1")).await;
    assert_eq!(status, 303, "Formular antwortet mit Weiterleitung");
    assert!(
        location.as_deref().unwrap_or("").contains("ok=modell_geloescht"),
        "mit Kästchen löscht das Formular ohne JavaScript: {location:?}"
    );
    assert!(
        !names(&ok(&server, "model.connections", json!({})).await).contains(&"Nebenrechner".to_owned()),
        "gelöscht"
    );

    let again = ok(
        &server,
        "model.connection_set",
        json!({ "name": "Zweitrechner", "kind": "openai", "url": far.url(), "external": false }),
    )
    .await;
    let second = again["id"].as_i64().unwrap();
    let deleted = ok(&server, "model.connection_delete", json!({ "id": second, "confirm": true })).await;
    assert_eq!(deleted["deleted"], true, "über RPC mit confirm: true wie bisher: {deleted}");
}

// ---- Context reconciliation after the clearance; cache per kind ------------------------------------------------

#[tokio::test]
async fn the_context_check_of_an_external_connection_without_release_sends_nothing() {
    let (server, _eliezer) = with_models().await;
    let far = MockEliezer::start().await;
    far.set_flavor(Flavor::Ollama);
    far.set_models(&["kiesel:8b"]);
    far.set_context_length("kiesel:8b", 8192);
    let made = ok(
        &server,
        "model.connection_set",
        json!({ "name": "Fernrechner", "kind": "ollama", "url": far.url(), "external": true }),
    )
    .await;
    let id = made["id"].as_i64().unwrap();
    ok(&server, "persona.update", json!({ "handle": "@exeget", "model": "kiesel:8b", "context_length": 32768 })).await;
    ok(&server, "persona.backend_set", json!({ "persona": "@exeget", "connection": id })).await;

    let budget = ok(&server, "model.context", json!({ "persona": "@exeget" })).await;
    assert!(
        far.paths().is_empty(),
        "ohne Freigabe geht auch der Kontextabgleich nicht hinaus (er trüge Schlüssel und Modellname): {:?}",
        far.paths()
    );
    assert_eq!(budget["tokens"], 32768, "es gilt die Einstellung: {budget}");
    assert!(budget["reported"].is_null(), "nichts gemeldet, weil nichts gefragt: {budget}");

    // Counter-check: with clearance the same call asks the peer – so the test above really checks the clearance.
    ok(&server, "persona.backend_set", json!({ "persona": "@exeget", "connection": id, "external_allowed": true }))
        .await;
    let budget = ok(&server, "model.context", json!({ "persona": "@exeget" })).await;
    assert!(far.paths().iter().any(|p| p == "/api/show"), "mit Freigabe wird gefragt: {:?}", far.paths());
    assert_eq!(budget["reported"], 8192, "und die gemeldete Länge gilt: {budget}");
}

#[tokio::test]
async fn two_connections_of_different_kind_on_one_address_share_neither_list_nor_context() {
    let (server, _eliezer) = with_models().await;
    let shared = MockEliezer::start().await;
    shared.set_models(&["kiesel:8b"]);
    shared.set_context_length("kiesel:8b", 16384);
    let connect = |name: &'static str, kind: &'static str| {
        let url = shared.url();
        let server = &server;
        async move {
            ok(server, "model.connection_set", json!({ "name": name, "kind": kind, "url": url, "external": false }))
                .await["id"]
                .as_i64()
                .unwrap()
        }
    };
    let as_eliezer = connect("Haus als Eliezer", "eliezer").await;
    let as_ollama = connect("Haus als Ollama", "ollama").await;
    ok(&server, "persona.update", json!({ "handle": "@nathan", "model": "kiesel:8b", "context_length": 65536 })).await;
    ok(&server, "persona.update", json!({ "handle": "@exeget", "model": "kiesel:8b", "context_length": 65536 })).await;
    ok(&server, "persona.backend_set", json!({ "persona": "@nathan", "connection": as_eliezer })).await;
    ok(&server, "persona.backend_set", json!({ "persona": "@exeget", "connection": as_ollama })).await;

    let first = ok(&server, "model.list", json!({ "connection": as_eliezer })).await;
    assert_eq!(first["live"], true, "erste Abfrage der Eliezer-Verbindung ist frisch: {first}");
    let nathan = ok(&server, "model.context", json!({ "persona": "@nathan" })).await;
    assert_eq!(nathan["reported"], 16384, "Eliezer meldet 16384: {nathan}");

    // The same address now plays Ollama and reports a different length.
    shared.set_flavor(Flavor::Ollama);
    shared.set_context_length("kiesel:8b", 8192);
    let second = ok(&server, "model.list", json!({ "connection": as_ollama })).await;
    assert_eq!(
        second["live"], true,
        "die Ollama-Verbindung fragt selbst – sie bekommt nicht die 60 s alte Liste der Eliezer-Verbindung derselben \
         Adresse: {second}"
    );
    let exeget = ok(&server, "model.context", json!({ "persona": "@exeget" })).await;
    assert_eq!(
        exeget["reported"], 8192,
        "die Kontextlänge kommt aus der Abfrage der eigenen Art (/api/show), nicht aus dem Speicher der anderen: {exeget}"
    );
    let nathan = ok(&server, "model.context", json!({ "persona": "@nathan" })).await;
    assert_eq!(nathan["reported"], 16384, "und die Eliezer-Verbindung behält ihren Wert: {nathan}");
}

// ---- Sermon with Bible atomic --------------------------------------------------------------------------------

#[tokio::test]
async fn a_sermon_whose_bible_cannot_be_set_leaves_no_half_work_and_a_second_try_succeeds() {
    let server = common::open().await;
    let works = || async {
        let n: i64 =
            sqlx::query_scalar("SELECT count(*) FROM work").fetch_one(server.state.store.pool()).await.unwrap();
        n
    };
    let before = works().await;
    let title = "Vom Brunnen am Morgen";

    // (1) Through RPC: a module name that the table `work_bible` does not take (over 100 characters).
    let long = "M".repeat(101);
    let refused = err(
        &server,
        "work.create",
        json!({ "title": title, "preset": "sermon", "text_ref": "Joh 3,16", "module": long }),
    )
    .await;
    assert_eq!(refused["code"], -32602, "unzulässiger Modulname ist ein Parameterfehler: {refused}");
    assert_eq!(works().await, before, "kein halbes Werk nach dem abgelehnten Anlegen");

    // (2) In the store: if the Bible fails in the transaction, nothing is created (before: work without Bible and
    //     mirror).
    let new = baruch_store::NewWork {
        title,
        author: "Hanna Probe",
        kind: baruch_core::WorkKind::Sermon,
        preset: baruch_store::Preset::Sermon,
        subtitle: "",
        audience: "",
        bible: "",
        language: Default::default(),
    };
    let failed =
        server.state.store.create_work_with_bible(&new, "Hanna Probe", Some("Joh 3,16"), Some("Zeile\nzwei")).await;
    assert!(failed.is_err(), "ein Modulname mit Zeilenumbruch scheitert: {failed:?}");
    assert_eq!(works().await, before, "nach dem gescheiterten Setzen der Bibel existiert kein Werk");

    // (3) Second attempt with the same title: no DuplicateTitle.
    let created =
        ok(&server, "work.create", json!({ "title": title, "preset": "sermon", "text_ref": "Joh 3,16" })).await;
    assert!(created["work"].is_i64(), "der zweite Versuch legt das Werk an (kein „Es gibt schon ein Werk“): {created}");
    assert_eq!(works().await, before + 1, "genau ein Werk: {created}");

    // (4) With an admissible module work and Bible are there together after one step.
    let other = baruch_store::NewWork { title: "Vom Brunnen am Abend", ..new };
    let made = server
        .state
        .store
        .create_work_with_bible(&other, "Hanna Probe", Some("Joh 3,16"), Some("ProbeBibel"))
        .await
        .expect("Werk mit Bibel");
    assert_eq!(
        server.state.store.work_bible(made.work).await.unwrap().as_deref(),
        Some("ProbeBibel"),
        "Werk und Bibel entstehen in einer Transaktion"
    );
}

// ---- Exchange: limits u7 -----------------------------------------------------------------------------------------

fn uuid(n: usize) -> String {
    format!("d0000000-0000-4000-8000-{n:012x}")
}

/// Sample work with `extra` further invented glossary terms.
fn with_terms(extra: usize) -> ExchangeDocument {
    let mut doc = sample_document();
    let pattern = doc.work.glossary[0].clone();
    for i in 0..extra {
        let mut term = pattern.clone();
        term.uuid = uuid(i + 1);
        term.term = format!("Prüfbegriff {i}");
        term.variants = Vec::new();
        doc.work.glossary.push(term);
    }
    doc
}

#[tokio::test]
async fn a_file_with_too_many_glossary_terms_is_refused_before_anything_is_written() {
    let max = baruch_store::MAX_GLOSSARY_TERMS;
    let server = common::open().await;
    let count = |table: &'static str| {
        let pool = server.state.store.pool().clone();
        async move {
            let n: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT count(*) FROM {table}")))
                .fetch_one(&pool)
                .await
                .unwrap();
            n
        }
    };
    let (works, terms) = (count("work").await, count("glossary_term").await);
    let doc = with_terms(max);
    assert!(doc.work.glossary.len() > max, "die Datei trägt mehr Begriffe als erlaubt");
    let xml = write_xml(&doc, ImageMode::Inline).unwrap();
    let refused = err(
        &server,
        "work.exchange_import",
        json!({ "data": baruch_store::encode_base64(xml.as_bytes()),
                "sha256": baruch_exchange::sha256_hex(xml.as_bytes()), "mode": "new" }),
    )
    .await;
    let message = refused["message"].as_str().unwrap_or("");
    assert!(
        message.contains("Begriffe") && message.contains(&max.to_string()) && message.contains("Nichts eingelesen"),
        "deutsche Meldung nennt die Grenze: {message}"
    );
    assert_eq!((count("work").await, count("glossary_term").await), (works, terms), "nichts ist geschrieben");

    // Syncing: the work already exists here; with the terms of the file it would be over the limit.
    let (_dir, store) = store().await;
    let work = store.exchange_apply(&sample_document(), SHA, ApplyMode::New, None).await.unwrap().work;
    let here: i64 = sqlx::query_scalar("SELECT count(*) FROM glossary_term WHERE work_id = ?")
        .bind(work)
        .fetch_one(store.pool())
        .await
        .unwrap();
    let merged = store.exchange_apply(&doc, SHA, ApplyMode::Merge, None).await;
    let message = merged.expect_err("über der Grenze abgelehnt").to_string();
    assert!(message.contains("höchstens") && message.contains("Nichts eingelesen"), "deutsche Meldung: {message}");
    let after: i64 = sqlx::query_scalar("SELECT count(*) FROM glossary_term WHERE work_id = ?")
        .bind(work)
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!(after, here, "die Transaktion rollt zurück: kein einziger Begriff der Datei bleibt");

    // Counter-check: exactly up to the limit it works.
    let (_dir, fresh) = store_pair().await;
    let fits = with_terms(max - sample_document().work.glossary.len());
    let applied = fresh.exchange_apply(&fits, SHA, ApplyMode::New, None).await.expect("genau die Grenze ist erlaubt");
    assert_eq!(applied.report.add.glossary_terms, max, "alle Begriffe bis zur Grenze kommen an");
}

async fn store_pair() -> (tempfile::TempDir, Store) {
    store().await
}

#[tokio::test]
async fn a_register_title_over_the_limit_is_refused_and_one_at_the_limit_is_read() {
    let max = baruch_typst::index::MAX_REGISTER_TITLE_CHARS;
    let (_dir, store) = store().await;
    let mut doc = sample_document();
    doc.work.registers[0].title = "Ö".repeat(max + 1);
    let refused = store.exchange_apply(&doc, SHA, ApplyMode::New, None).await;
    let message = refused.expect_err("zu langer Registertitel abgelehnt").to_string();
    assert!(
        message.contains("Titel hat") && message.contains(&max.to_string()) && message.contains("ichts eingelesen"),
        "deutsche Meldung nennt Länge und Grenze: {message}"
    );
    let works: i64 = sqlx::query_scalar("SELECT count(*) FROM work WHERE uuid = ?")
        .bind(&doc.work.uuid)
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!(works, 0, "nichts ist geschrieben");
    // The preview (and with it the page) reports it too, instead of noticing it only on adopting.
    assert!(store.exchange_preview(&doc, SHA, None).await.is_err(), "schon die Vorschau lehnt ab");

    doc.work.registers[0].title = format!("  {}  ", "Ö".repeat(max));
    let applied = store.exchange_apply(&doc, SHA, ApplyMode::New, None).await.expect("genau an der Grenze erlaubt");
    let title: String = sqlx::query_scalar("SELECT title FROM index_register WHERE uuid = ?")
        .bind(&doc.work.registers[0].uuid)
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!(title.chars().count(), max, "gezählt wird der gespeicherte Titel (Leerraum am Rand zählt nicht)");
    assert!(applied.report.add.registers >= 1, "das Register ist eingelesen");
}

/// Hand-written check file of sub-version 1 (Round 26, freely invented).
const OLD_FILE: &str = include_str!("../../baruch-exchange/tests/data/import/werk.baruch.xml");

#[tokio::test]
async fn valid_files_of_the_oldest_and_the_current_subversion_are_still_read() {
    // The sub-versions 2 to 7 are checked by `baruch-exchange/tests/xsd.rs` on the reader (the reader is strict: it
    // rejects parts of a later sub-version in an older file); here the whole way into the store for the oldest and the
    // current one – the new limits (glossary, index titles) hit no valid file.
    assert!(OLD_FILE.contains("version=\"1\""), "die Prüfdatei ist eine Datei der Unterversion 1");
    let current = write_xml(&sample_document_u8(), ImageMode::Inline).unwrap();
    assert!(
        current.contains(&format!("version=\"{}\"", baruch_exchange::FORMAT_VERSION)),
        "die Ausgabe trägt die aktuelle Unterversion"
    );
    for (what, file) in [("Unterversion 1", OLD_FILE.to_owned()), ("aktuelle Unterversion", current)] {
        let server = common::open().await;
        let answer = server
            .rpc(
                "work.exchange_import",
                json!({ "data": baruch_store::encode_base64(file.as_bytes()),
                        "sha256": baruch_exchange::sha256_hex(file.as_bytes()), "mode": "new" }),
            )
            .await;
        assert!(answer["error"].is_null(), "{what} liest weiter: {answer}");
        assert!(answer["result"]["work"].is_i64(), "{what}: das Werk ist angelegt: {answer}");
    }
}

// ---- Exchange: cover blocks in normal form --------------------------------------------------------------------------

#[tokio::test]
async fn an_imported_cover_block_is_stored_in_normal_form_and_can_be_changed_in_the_editor() {
    let (_dir, store) = store().await;
    let mut doc = sample_document();
    let block = {
        let book = doc.work.book.as_mut().expect("Beispiel mit Buchangaben");
        let block = book.cover_blocks.iter_mut().find(|b| b.text.is_some()).expect("Block mit eigenem Text");
        block.font = Some("  Libertinus Serif ".into());
        block.color = "#AABBCC".into();
        block.text = Some("Erste Zeile\r\nzweite Zeile".into());
        block.clone()
    };
    let applied = store.exchange_apply(&doc, SHA, ApplyMode::New, None).await.expect("Einlesen gelingt");
    let layout = store.cover_layout(applied.work).await.unwrap();
    let stored =
        layout.blocks.iter().find(|b| b.kind.as_str() == block.kind.as_db()).expect("Block gespeichert").clone();
    assert_eq!(stored.font.as_deref(), Some("Libertinus Serif"), "Leerraum um den Schriftnamen ist weg");
    assert_eq!(stored.color, "#aabbcc", "Farbe klein geschrieben");
    assert_eq!(stored.text.as_deref(), Some("Erste Zeile\nzweite Zeile"), "Steuerzeichen CR aus CRLF entfernt");
    assert_eq!(
        baruch_store::check_cover_block(&stored).expect("besteht die Prüfung des Editors"),
        stored,
        "der gespeicherte Block ist schon die Normalform"
    );
    assert!(
        !applied.report.deviations.iter().any(|d| d.label.contains("unverändert übernommen")),
        "ein normalisierbarer Block braucht keinen Hinweis: {:?}",
        applied.report.deviations
    );
    let mut moved = stored.clone();
    moved.x_mm += 1.0;
    let change = store.set_cover_blocks(applied.work, &[moved]).await.expect("im Editor ohne Bereinigen änderbar");
    assert_eq!(change.changed.len(), 1, "genau dieser Block ist geändert");
    // Second sync of the same file: the normal form holds for comparing too – only the shift deviates.
    let again = store.exchange_apply(&doc, SHA, ApplyMode::Merge, None).await.unwrap();
    assert_eq!(again.report.add.cover_blocks, 0, "nichts kommt doppelt");
}

#[tokio::test]
async fn a_cover_block_that_cannot_be_normalised_is_taken_as_before_and_reported_as_a_hint() {
    let (_dir, store) = store().await;
    let mut doc = sample_document();
    let uuid = {
        let book = doc.work.book.as_mut().expect("Beispiel mit Buchangaben");
        let block = book.cover_blocks.iter_mut().find(|b| b.text.is_some()).expect("Block mit eigenem Text");
        // U+0085 (NEL) is a control character that XML 1.0 allows; the editor rejects it in the text.
        block.text = Some("Zeile\u{85}Ende".into());
        block.uuid.clone()
    };
    let applied = store.exchange_apply(&doc, SHA, ApplyMode::New, None).await.expect("das Einlesen bricht nicht ab");
    let text: Option<String> = sqlx::query_scalar("SELECT text FROM cover_block WHERE uuid = ?")
        .bind(&uuid)
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!(text.as_deref(), Some("Zeile\u{85}Ende"), "der Block ist wie bisher unverändert übernommen");
    let hint = applied
        .report
        .deviations
        .iter()
        .find(|d| d.uuid == uuid && d.label.contains("unverändert übernommen"))
        .unwrap_or_else(|| panic!("Hinweis im Einlese-Bericht fehlt: {:?}", applied.report.deviations));
    assert_eq!(hint.action, DeviationAction::Hinweis, "nur ein Hinweis, keine Ablehnung");
    assert!(hint.label.contains("Steuerzeichen"), "der Hinweis nennt den Grund der Prüfung: {}", hint.label);
}

// ---- Exchange: rules on syncing -------------------------------------------------------------------------------------

fn find<'a>(nodes: &'a mut [ExchangeNode], uuid: &str) -> Option<&'a mut ExchangeNode> {
    for node in nodes {
        if node.uuid == uuid {
            return Some(node);
        }
        if let Some(found) = find(&mut node.children, uuid) {
            return Some(found);
        }
    }
    None
}

/// Revision `rev` of „Der Eimer“ (identifier 11) in the sample.
fn bucket_revision(doc: &mut ExchangeDocument, rev: u32) -> &mut ExchangeRevision {
    let bucket = find(&mut doc.work.outline, &sample_uuid(11)).expect("„Der Eimer“");
    bucket.revisions.iter_mut().find(|r| r.uuid == sample_uuid(rev)).expect("Fassung")
}

#[tokio::test]
async fn a_known_finding_keeps_status_and_history_and_the_hint_counts_the_entries_of_the_file() {
    let (_dir, store) = store().await;
    let work = store.exchange_apply(&sample_document(), SHA, ApplyMode::New, None).await.unwrap().work;
    let entries = || async {
        let n: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM finding_status s JOIN finding f ON f.id = s.finding_id WHERE f.work_id = ?",
        )
        .bind(work)
        .fetch_one(store.pool())
        .await
        .unwrap();
        n
    };
    let before = entries().await;
    // The other instance has a further entry in the history for the moved finding (312).
    let mut doc = sample_document();
    let finding = doc.work.findings.iter_mut().find(|f| f.uuid == sample_uuid(312)).expect("Befund 312");
    let mut more = finding.history[0].clone();
    more.uuid = uuid(900);
    finding.history.push(more);
    let merged = store.exchange_apply(&doc, SHA, ApplyMode::Merge, None).await.expect("Abgleich gelingt");
    assert_eq!(entries().await, before, "Regel: Der Verlauf eines bekannten Befunds bleibt, wie er hier ist");
    assert_eq!(merged.report.add.finding_entries, 0, "kein Eintrag kommt dazu");
    let hint = merged
        .report
        .deviations
        .iter()
        .find(|d| d.uuid == sample_uuid(312))
        .unwrap_or_else(|| panic!("nichts fällt still weg – Hinweis erwartet: {:?}", merged.report.deviations));
    assert!(
        hint.label.contains("Verlauf weicht ab")
            && hint.label.contains("1 Eintrag im Verlauf der Datei nicht übernommen"),
        "der Hinweis nennt, was nicht übernommen ist: {}",
        hint.label
    );
    // Deviating status: stays as well, the hint names status and entries.
    let finding = doc.work.findings.iter_mut().find(|f| f.uuid == sample_uuid(310)).expect("Befund 310");
    finding.reason = "Drüben anders begründet.".into();
    let merged = store.exchange_apply(&doc, SHA, ApplyMode::Merge, None).await.unwrap();
    let reason: String = sqlx::query_scalar("SELECT reason FROM finding WHERE uuid = ?")
        .bind(sample_uuid(310))
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert!(reason.starts_with("Absicht"), "Status und Begründung setzt hier der Mensch: {reason}");
    assert!(
        merged.report.deviations.iter().any(|d| d.uuid == sample_uuid(310) && d.label.contains("Status weicht ab")),
        "Hinweis zum abweichenden Status: {:?}",
        merged.report.deviations
    );
}

#[tokio::test]
async fn a_divergent_variant_keeps_its_variant_mark_under_the_derived_id() {
    let (_dir, store) = store().await;
    store.exchange_apply(&sample_document(), SHA, ApplyMode::New, None).await.unwrap();
    // Over there variant A (revision 3, identifier 113) was changed in place.
    let mut doc = sample_document();
    let body = "== Der Eimer\n\nDer Eimer klirrt lauter am Rand des Brunnens.\n";
    let original = bucket_revision(&mut doc, 113);
    original.body = body.into();
    let group = original.variant.as_ref().expect("Fassung 3 ist eine Variante").group.clone();
    let merged = store.exchange_apply(&doc, SHA, ApplyMode::Merge, None).await.expect("Abgleich gelingt");
    assert_eq!(merged.report.add.revisions, 1, "die abweichende Fassung kommt als neue: {:?}", merged.report.items);
    let derived = derive_uuid(&sample_uuid(113), body);
    let (variant_group, label): (Option<String>, Option<String>) =
        sqlx::query_as("SELECT variant_group, variant_label FROM revision WHERE uuid = ?")
            .bind(&derived)
            .fetch_one(store.pool())
            .await
            .expect("abgeleitete Fassung angelegt");
    assert_eq!(
        (variant_group.as_deref(), label.as_deref()),
        (Some(group.as_str()), Some("A")),
        "die abgeleitete Fassung trägt Gruppe und Etikett der Variante (vorher verlor sie beides)"
    );
}

#[tokio::test]
async fn a_fast_forward_never_changes_the_heading_in_place() {
    let (_dir, store) = store().await;
    // Here: „Der Eimer“ revision 1 after the first step.
    let mut local = sample_document_u8();
    {
        let revision = bucket_revision(&mut local, 111);
        revision.amends.truncate(1);
        revision.amend_seq = 1;
        revision.body = revision.amends[0].after.clone();
    }
    store.exchange_apply(&local, SHA, ApplyMode::New, None).await.unwrap();
    let before: String = sqlx::query_scalar("SELECT body FROM revision WHERE uuid = ?")
        .bind(sample_uuid(111))
        .fetch_one(store.pool())
        .await
        .unwrap();
    // File: gapless chain whose last step rewrites the heading (edited by hand).
    let mut file = sample_document_u8();
    {
        let revision = bucket_revision(&mut file, 111);
        let renamed = "== Der Krug\n\nGanz anderer Titel.\n".to_owned();
        revision.amends[2].after = renamed.clone();
        revision.body = renamed;
    }
    let merged = store.exchange_apply(&file, SHA, ApplyMode::Merge, None).await.expect("Abgleich gelingt");
    let (body, seq): (String, i64) = sqlx::query_as("SELECT body, amend_seq FROM revision WHERE uuid = ?")
        .bind(sample_uuid(111))
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!(
        (body, seq),
        (before, 1),
        "die aktuelle Fassung wird nicht vorgespult, wenn dabei die Überschrift wechselte – sonst trüge sie einen \
         Titel, den die Gliederung nicht kennt (Codereview store-2)"
    );
    assert_eq!(merged.report.add.amends, 0, "keine Fortschreibung übernommen");
    assert!(
        merged.report.deviations.iter().any(|d| d.action == DeviationAction::Fassung),
        "stattdessen der bisherige Weg „abweichende Fassung“: {:?}",
        merged.report.deviations
    );
    let title: String = sqlx::query_scalar("SELECT title FROM node WHERE uuid = ?")
        .bind(sample_uuid(11))
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!(title, "Der Eimer", "die Gliederung bleibt");
}

#[tokio::test]
async fn a_heading_level_that_does_not_fit_the_point_is_reported_as_a_hint_when_reading() {
    let (_dir, store) = store().await;
    let mut doc = sample_document();
    // Hand-written file: chapter „Der Eimer“ with a heading of level 3.
    bucket_revision(&mut doc, 111).body = "=== Der Eimer\n\nDer Eimer steht am Brunnen.\n".into();
    let applied = store.exchange_apply(&doc, SHA, ApplyMode::New, None).await.expect("das Einlesen bleibt möglich");
    let body: String = sqlx::query_scalar("SELECT body FROM revision WHERE uuid = ?")
        .bind(sample_uuid(111))
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert!(body.starts_with("=== Der Eimer"), "der Text bleibt exakt, wie er in der Datei steht");
    let hint = applied
        .report
        .deviations
        .iter()
        .find(|d| d.label.contains("Der Eimer") && d.label.contains("Überschrift der eingelesenen Fassung"))
        .unwrap_or_else(|| panic!("Hinweis zur Ebene fehlt: {:?}", applied.report.deviations));
    assert_eq!(hint.action, DeviationAction::Hinweis, "nur ein Hinweis");
    assert!(
        hint.label.contains("Ebene 3") && hint.label.contains("Ebene 1 (Kapitel)"),
        "beide Ebenen genannt: {}",
        hint.label
    );
    let again = store.exchange_apply(&doc, SHA, ApplyMode::Merge, None).await.unwrap();
    assert!(
        !again.report.deviations.iter().any(|d| d.label.contains("Überschrift der eingelesenen Fassung")),
        "bekannte Fassungen meldet ein späterer Abgleich nicht noch einmal: {:?}",
        again.report.deviations
    );
}
