// SPDX-License-Identifier: AGPL-3.0-or-later
//! Round 15b „anbieter“: connection per persona (Eliezer, OpenAI-compatible, Ollama), local/external with clearance,
//! model list per connection with fallback to the expired cache, reconciliation of the context length, short test in
//! the database, key per connection bound to its target, `persona.list` without a work, forms without JavaScript and
//! switch „Bibel-Belege“.
//!
//! Every peer is a mock of its own (`baruch_llm::mock::MockEliezer` on 127.0.0.1:0, as Eliezer, Ollama or
//! OpenAI-compatible); secrets only in `MemorySecrets` with a unique test secret. No test reaches the real Eliezer or a
//! foreign service.

use crate::common;

use std::sync::Arc;

use baruch_llm::mock::{Flavor, MockEliezer, Reply};
use baruch_server::AppState;
use baruch_server::assist::eliezer::EliezerPort;
use baruch_server::connections::MemorySecrets;
use serde_json::{Value, json};

/// Unique test secret; it must be in no response and on no page.
const SECRET: &str = "Rr8-anbieter-geheim-5f2c";

const TEXT: &str = "#set document(title: \"Die Fähre am Morgen\", author: \"Ida Kranich\")\n\n\
= Ablegen\n\nDie Fähre legt ab, als der Nebel noch über dem Fluss liegt.\n";

/// Server with the real adapter (EliezerPort), Eliezer pointed to an own mock, and a work.
async fn ferry() -> (common::Server, MockEliezer, i64, i64) {
    let data = tempfile::tempdir().unwrap();
    let state = AppState::open_with(data.path(), Arc::new(MemorySecrets::default()), Arc::new(EliezerPort))
        .await
        .expect("Datenordner öffnen");
    let state = Arc::new(state);
    let server = common::Server { app: baruch_server::router(state.clone()), state, data };
    let eliezer = MockEliezer::start().await;
    ok(&server, "connection.set", json!({ "kind": "eliezer", "config": { "url": eliezer.url() } })).await;
    let work = server.import(TEXT, "faehre.typ").await.work;
    let key = server.state.store.load_work(work).await.unwrap().nodes[0].key;
    (server, eliezer, work, key)
}

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

/// New model connection to the mock `mock`.
async fn connection(server: &common::Server, name: &str, kind: &str, mock: &MockEliezer, external: bool) -> i64 {
    let made = ok(
        server,
        "model.connection_set",
        json!({ "name": name, "kind": kind, "url": mock.url(), "external": external }),
    )
    .await;
    assert_eq!(made["locality"], if external { "extern" } else { "lokal" }, "Anzeige lokal/extern: {made}");
    made["id"].as_i64().unwrap()
}

fn chat_count(mock: &MockEliezer) -> usize {
    mock.chat_bodies().len()
}

#[tokio::test]
async fn an_external_connection_without_release_refuses_and_the_mock_sees_no_request() {
    let _turn = common::model_queue(module_path!()).await;
    let (server, eliezer, work, key) = ferry().await;
    let far = MockEliezer::start().await;
    let id = connection(&server, "Fernrechner", "openai", &far, true).await;
    let set = ok(&server, "persona.backend_set", json!({ "persona": "@lektor", "connection": id })).await;
    assert_eq!(
        (set["locality"].as_str(), set["external_allowed"].as_bool(), set["usable"].as_bool()),
        (Some("extern"), Some(false), Some(false)),
        "Vorgabe der Freigabe: aus; so ist die Persona gesperrt: {set}"
    );
    assert!(set["warning"].as_str().unwrap_or("").contains("nicht freigegeben"), "Warnung als Satz: {set}");

    let ask = json!({ "work": work, "key": key, "persona": "@lektor", "question": "Passt der Nebel?" });
    let refused = err(&server, "persona.ask", ask.clone()).await;
    let message = refused["message"].as_str().unwrap_or("");
    assert!(
        message.contains("extern") && message.contains("nicht freigegeben") && message.contains("nichts gesendet"),
        "ehrlicher Satz statt Anfrage: {refused}"
    );
    assert!(far.requests().is_empty(), "der externe Nachbau sieht keine einzige Anfrage: {:?}", far.paths());
    assert_eq!(chat_count(&eliezer), 0, "und es geht auch nicht still an Eliezer");

    ok(&server, "persona.backend_set", json!({ "persona": "@lektor", "connection": id, "external_allowed": true }))
        .await;
    far.push(Reply::text(&["Kritisch: ", "Der Nebel trägt.", "\n- Schritt: prüfen."]));
    let answered = ok(&server, "persona.ask", ask).await;
    assert_eq!(chat_count(&far), 1, "mit Freigabe geht genau ein Aufruf an die externe Verbindung: {answered}");
    let body = &far.chat_bodies()[0];
    assert!(body.to_string().contains("Die Fähre legt ab"), "der Abschnittstext ging an die freigegebene Verbindung");
    assert_eq!(chat_count(&eliezer), 0, "Eliezer bleibt unberührt");
}

#[tokio::test]
async fn the_model_list_follows_the_persona_and_falls_back_to_the_expired_cache() {
    let (server, eliezer, _, _) = ferry().await;
    let ollama = MockEliezer::start().await;
    ollama.set_flavor(Flavor::Ollama);
    ollama.set_models(&["kiesel:8b", "amsel:3b"]);
    let id = connection(&server, "Ollama", "ollama", &ollama, false).await;
    ok(&server, "persona.backend_set", json!({ "persona": "@exeget", "connection": id })).await;

    let listed = ok(&server, "model.list", json!({ "persona": "@exeget" })).await;
    assert_eq!(listed["models"], json!(["kiesel:8b", "amsel:3b"]), "Liste aus /v1/models der Verbindung: {listed}");
    assert_eq!(listed["live"], true, "eben abgefragt");
    assert_eq!(listed["connection"]["name"], "Ollama", "die Antwort nennt die Verbindung");
    let personas: Vec<&str> =
        listed["personas"].as_array().unwrap().iter().map(|p| p["handle"].as_str().unwrap()).collect();
    assert_eq!(personas, ["@exeget"], "abgeglichen werden nur die Personas dieser Verbindung");
    let other = ok(&server, "model.list", json!({})).await;
    assert_eq!(other["connection"]["name"], "Eliezer", "ohne Angabe die Verbindung Eliezer: {other}");
    assert_eq!(other["models"], json!(["mock-modell", "google/gemma-4-26b-a4b"]), "Liste des Eliezer-Nachbaus");

    ollama.fail_models(Some((500, "{\"error\":\"kaputt\"}")));
    baruch_server::models::expire_list(&ollama.url());
    let stale = ok(&server, "model.list", json!({ "connection": id })).await;
    assert_eq!(stale["live"], false, "Gegenstelle antwortet nicht: aus dem Zwischenspeicher: {stale}");
    assert_eq!(stale["models"], json!(["kiesel:8b", "amsel:3b"]), "die abgelaufene Liste statt eines Fehlers");
    let note = stale["message"].as_str().unwrap_or("");
    assert!(note.contains("abgelaufenen Zwischenspeicher") && note.contains("HTTP 500"), "Hinweis: {note}");
    assert!(
        eliezer.paths().iter().all(|p| p == "/v1/models" || p == "/api/v1/models"),
        "Eliezer sah nur seine eigene Liste (mit Kontextlängen): {:?}",
        eliezer.paths()
    );

    let never = MockEliezer::start().await;
    never.fail_models(Some((503, "{}")));
    let id = connection(&server, "Leer", "openai", &never, false).await;
    let refused = err(&server, "model.list", json!({ "connection": id })).await;
    assert_eq!(refused["code"], -32000, "ohne Zwischenspeicher bleibt es ein ehrlicher Fehler: {refused}");
}

#[tokio::test]
async fn the_context_is_the_smaller_of_setting_and_reported_length() {
    let (server, eliezer, _, _) = ferry().await;
    let ollama = MockEliezer::start().await;
    ollama.set_flavor(Flavor::Ollama);
    ollama.set_models(&["kiesel:8b"]);
    ollama.set_context_length("kiesel:8b", 8192);
    let id = connection(&server, "Ollama klein", "ollama", &ollama, false).await;
    ok(&server, "persona.update", json!({ "handle": "@exeget", "model": "kiesel:8b", "context_length": 32768 })).await;
    ok(&server, "persona.backend_set", json!({ "persona": "@exeget", "connection": id })).await;
    let budget = ok(&server, "model.context", json!({ "persona": "@exeget" })).await;
    assert_eq!(
        (budget["tokens"].as_u64(), budget["reported"].as_u64()),
        (Some(8192), Some(8192)),
        "wirksam ist das Minimum aus Einstellung (32768) und Meldung von Ollama (8192): {budget}"
    );
    let warning = budget["warning"].as_str().unwrap_or("");
    assert!(warning.contains("Einstellung 32768") && warning.contains("8192"), "Warnung nennt beide: {warning}");
    assert!(ollama.paths().iter().any(|p| p == "/api/show"), "gemeldet über /api/show: {:?}", ollama.paths());

    let open = MockEliezer::start().await;
    open.set_flavor(Flavor::Openai);
    let id = connection(&server, "Offen", "openai", &open, false).await;
    ok(&server, "persona.backend_set", json!({ "persona": "@hoerer", "connection": id })).await;
    let budget = ok(&server, "model.context", json!({ "persona": "@hoerer" })).await;
    assert_eq!(
        (budget["tokens"].as_u64(), budget["reported"].clone(), budget["warning"].clone()),
        (Some(8192), Value::Null, Value::Null),
        "OpenAI-kompatibel meldet nichts: es gilt die Einstellung der Persona (8192): {budget}"
    );

    eliezer.set_context_length("google/gemma-4-26b-a4b", 16384);
    ok(
        &server,
        "persona.update",
        json!({ "handle": "@nathan", "model": "google/gemma-4-26b-a4b", "context_length": 65536 }),
    )
    .await;
    let budget = ok(&server, "model.context", json!({ "persona": "@nathan" })).await;
    assert_eq!(budget["tokens"], 16384, "Eliezer meldet 16384 über /api/v1/models: {budget}");
    assert!(
        budget["warning"].as_str().unwrap_or("").contains("Einstellung 65536, Eliezer meldet 16384"),
        "Warnung im Wortlaut des Vertrags: {budget}"
    );
    assert!(eliezer.forbidden_paths().is_empty(), "nie load/unload/estimate: {:?}", eliezer.forbidden_paths());
    assert!(eliezer.chat_bodies().is_empty(), "für die Kontextlänge kein Modellaufruf");
}

#[tokio::test]
async fn the_last_probe_survives_a_restart() {
    let _turn = common::model_queue(module_path!()).await;
    let (server, _eliezer, _, _) = ferry().await;
    let far = MockEliezer::start().await;
    let id = connection(&server, "Kurztest-Ziel", "openai", &far, true).await;
    far.push(Reply::text(&["OK"]));
    let probe = ok(&server, "model.probe", json!({ "model": "mock-modell", "connection": id })).await;
    assert_eq!((probe["ok"].as_bool(), probe["reply"].as_str()), (Some(true), Some("OK")), "Kurztest: {probe}");
    let body = far.chat_bodies()[0].to_string();
    assert!(body.contains("Answer only with: OK") && !body.contains("Fähre"), "nur der feste Auftrag, kein Buchtext");

    let path = server.data.path().to_path_buf();
    let again = AppState::open_with(&path, Arc::new(MemorySecrets::default()), Arc::new(EliezerPort)).await.unwrap();
    let again = Arc::new(again);
    let listed = baruch_server::models::call(&again, "model.connections", json!({})).await.unwrap();
    let mine = listed["connections"].as_array().unwrap().iter().find(|c| c["id"] == id).cloned().unwrap();
    assert_eq!(mine["probe"]["ok"], true, "der Kurztest steht nach dem Neustart in der Datenbank: {mine}");
    assert_eq!(mine["probe"]["reply"], "OK", "mit Antwort");
}

#[tokio::test]
async fn a_key_belongs_to_its_address_and_never_appears() {
    let (server, _eliezer, _, _) = ferry().await;
    let first = MockEliezer::start().await;
    first.require_bearer(Some(SECRET));
    let other = MockEliezer::start().await;
    let id = connection(&server, "Mit Schlüssel", "openai", &first, false).await;
    let mut answers = Vec::new();
    answers.push(ok(&server, "model.connection_set_secret", json!({ "id": id, "secret": SECRET })).await);
    let check = ok(&server, "model.connection_check", json!({ "id": id })).await;
    assert_eq!(check["ok"], true, "mit Schlüssel geprüft: {check}");
    assert!(
        first.requests().iter().all(|r| r.authorization.as_deref() == Some(&format!("Bearer {SECRET}")[..])),
        "der Schlüssel ging im Kopf an seine Adresse"
    );
    answers.push(check);
    answers.push(ok(&server, "model.connection_set", json!({ "id": id, "url": other.url() })).await);
    let moved = ok(&server, "model.connection_check", json!({ "id": id })).await;
    assert_eq!(moved["ok"], false, "neue Adresse ohne neuen Schlüssel: nicht geprüft: {moved}");
    assert!(moved["message"].as_str().unwrap_or("").contains("anderes Ziel"), "ehrlicher Grund: {moved}");
    assert!(other.requests().is_empty(), "an die neue Adresse ging nichts, schon gar nicht der Schlüssel");
    answers.push(moved);
    let listed = ok(&server, "model.connections", json!({})).await;
    let mine = listed["connections"].as_array().unwrap().iter().find(|c| c["id"] == id).cloned().unwrap();
    assert_eq!(mine["secret"], "hinterlegt", "Zustand des Schlüssels ohne Wert: {mine}");
    assert!(mine["secret_for"].as_str().is_some_and(|t| t.starts_with("http://127.0.0.1:")), "gilt nur fürs alte Ziel");
    answers.push(listed);
    let (_, page) = server.get("/einstellungen/verbindungen").await;
    assert!(page.contains("data-model-connection=") && page.contains("Mit Schlüssel"), "Seite zeigt die Verbindung");
    for text in answers.iter().map(Value::to_string).chain([page]) {
        assert!(!text.contains(SECRET), "nie der Schlüssel in Antwort oder Seite: {text}");
    }
    assert_eq!(
        server.state.secrets.get(&format!("model-{id}")).unwrap().as_deref(),
        Some(SECRET),
        "der Schlüssel liegt nur im Schlüsselbund (MemorySecrets), Konto baruch/model-<id>"
    );
}

#[tokio::test]
async fn the_persona_list_is_complete_without_any_work() {
    let server = common::open().await;
    for (id, _, title) in server.state.store.works().await.unwrap() {
        server.state.store.delete_work(id, &title).await.unwrap();
    }
    assert!(server.state.store.works().await.unwrap().is_empty(), "kein Werk mehr");
    let listed = ok(&server, "persona.list", json!({})).await;
    let personas = listed["personas"].as_array().unwrap();
    assert!(personas.len() >= 6, "alle Personas der Tabelle, ohne Vorgabewerk: {listed}");
    for p in personas {
        assert!(p["model"].as_str().is_some_and(|m| !m.is_empty()), "Modell da: {p}");
        assert_eq!(p["connection"]["name"], "Eliezer", "Vorgabe-Verbindung Eliezer: {p}");
        assert_eq!(p["locality"], "lokal", "Eliezer ist lokal: {p}");
        assert_eq!(p["external_allowed"], false, "Freigabe aus: {p}");
        assert_eq!(p["context"]["tokens"], p["context_length"], "ohne Meldung wirksam die Einstellung: {p}");
    }
}

#[tokio::test]
async fn connections_and_releases_are_set_by_forms_without_javascript() {
    let (server, _eliezer, _, _) = ferry().await;
    let far = MockEliezer::start().await;
    let form = format!("action=save&name=Nachbar&kind=openai&url={}&external=1", far.url());
    let (status, location) = server.post_form("/einstellungen/modelle/act", &form).await;
    assert_eq!(status, 303, "Formular antwortet mit Weiterleitung");
    let location = location.unwrap();
    assert!(location.starts_with("/einstellungen/verbindungen?ok=modell_angelegt"), "fester Code: {location}");
    let listed = ok(&server, "model.connections", json!({})).await;
    let id = listed["connections"].as_array().unwrap().iter().find(|c| c["name"] == "Nachbar").unwrap()["id"]
        .as_i64()
        .unwrap();
    let (_, location) = server.post_form("/einstellungen/modelle/act", "action=save&name=&kind=openai&url=x").await;
    assert!(location.unwrap().contains("modellfehler="), "ungültige Eingabe: Fehlercode, nichts angelegt");

    let (status, location) = server
        .post_form("/einstellungen/personas/act", &format!("action=backend&handle=@belege&connection={id}"))
        .await;
    assert_eq!(status, 303, "Verbindung je Persona per Formular");
    assert!(location.unwrap().contains("ok=verbindung"), "Erfolgscode");
    let backend = ok(&server, "persona.backend_get", json!({ "persona": "@belege" })).await;
    assert_eq!(
        (backend["connection"]["id"].as_i64(), backend["external_allowed"].as_bool()),
        (Some(id), Some(false)),
        "die gewählte Verbindung ist gespeichert; ohne angehaktes Kästchen bleibt „extern“ nicht freigegeben: {backend}"
    );
    let (_, html) = server.get("/einstellungen/assistenten?persona=belege").await;
    assert!(html.contains("value=\"backend\"") && html.contains("name=\"external_allowed\""), "Formular im Tab");
    assert!(html.contains("data-backend-blocked"), "Warnung: extern ohne Freigabe: {html}");
    assert!(html.contains("data-locality-badge>extern<"), "Anzeige „extern“");

    server
        .post_form(
            "/einstellungen/personas/act",
            &format!("action=backend&handle=@belege&connection={id}&external_allowed=1"),
        )
        .await;
    let backend = ok(&server, "persona.backend_get", json!({ "persona": "@belege" })).await;
    assert_eq!(backend["usable"], true, "mit Freigabe nutzbar: {backend}");

    for method in ["model.connection_set", "model.connection_delete", "persona.backend_set", "bible.set_work_sources"] {
        let (status, _) = server.post_form("/act", &format!("method={method}&work=1")).await;
        assert_eq!(status, 400, "{method} nicht über das allgemeine /act (nur Mensch über eigene Formulare)");
    }
    let names: Vec<&str> = baruch_server::mcp::tools::TOOLS.iter().map(|t| t.name).collect();
    assert!(
        !names.iter().any(|n| n.starts_with("model") || n.contains("backend") || n.contains("connection")),
        "kein MCP-Werkzeug ändert Verbindungen: {names:?}"
    );
    ok(&server, "model.connection_delete", json!({ "id": id, "confirm": true })).await;
    let backend = ok(&server, "persona.backend_get", json!({ "persona": "@belege" })).await;
    assert_eq!(backend["connection"]["name"], "Eliezer", "nach dem Löschen wieder Eliezer: {backend}");
}

#[tokio::test]
async fn bible_evidence_is_only_for_belege_and_the_lector_with_their_defaults() {
    let server = common::open().await;
    let belege = ok(&server, "persona.bible_get", json!({ "persona": "@belege" })).await;
    assert_eq!(
        (belege["bible_context"].as_bool(), belege["allowed"].as_bool(), belege["default"].as_bool()),
        (Some(true), Some(true), Some(true)),
        "@belege: erlaubt, Vorgabe an: {belege}"
    );
    let lector = ok(&server, "persona.bible_get", json!({ "persona": "@lektor" })).await;
    assert_eq!(
        (lector["bible_context"].as_bool(), lector["allowed"].as_bool()),
        (Some(false), Some(true)),
        "Lektor: erlaubt, Vorgabe aus: {lector}"
    );
    let refused = err(&server, "persona.bible_set", json!({ "persona": "@hoerer", "on": true })).await;
    assert_eq!(refused["code"], -32602, "andere Personas abgelehnt: {refused}");
    assert!(refused["message"].as_str().unwrap_or("").contains("@belege und den Lektor"), "Grund: {refused}");
    let (status, location) = server.post_form("/einstellungen/personas/act", "action=bible&handle=@lektor&on=1").await;
    assert_eq!((status.as_u16(), location.unwrap().contains("ok=bibel")), (303, true), "Schalter per Formular");
    let lector = ok(&server, "persona.bible_get", json!({ "persona": "@lektor" })).await;
    assert_eq!(lector["bible_context"], true, "eingeschaltet: {lector}");
    let (_, html) = server.get("/einstellungen/assistenten?persona=belege").await;
    assert!(html.contains("value=\"bible\"") && html.contains("Bibel-Belege aus SWORD"), "Schalter im Tab von @belege");
    let (_, html) = server.get("/einstellungen/assistenten?persona=hoerer").await;
    let panel = &html[html.find("id=\"persona-hoerer\"").unwrap()..];
    let panel = &panel[..panel.find("role=\"tabpanel\"").unwrap_or(panel.len())];
    assert!(!panel.contains("value=\"bible\""), "kein Schalter im Tab des Hörers");
}

#[tokio::test]
async fn repeated_calls_of_one_connection_reuse_one_http_client_in_the_server() {
    let (server, _eliezer, _, _) = ferry().await;
    let open = MockEliezer::start().await;
    open.set_flavor(Flavor::Openai);
    open.set_keep_alive(true);
    let id = connection(&server, "Wiederholt", "openai", &open, false).await;
    for _ in 0..3 {
        baruch_server::models::expire_list(&open.url());
        let listed = ok(&server, "model.list", json!({ "connection": id })).await;
        assert_eq!(listed["live"], true, "jedes Mal neu abgefragt (Zwischenspeicher abgelaufen): {listed}");
    }
    ok(&server, "model.connection_check", json!({ "id": id })).await;
    assert_eq!(open.requests().len(), 4, "vier Abfragen der Modellliste beim Nachbau: {:?}", open.paths());
    assert_eq!(
        open.connections(),
        1,
        "alle über eine offene TCP-Verbindung – derselbe HTTP-Client der Verbindung (ein Client je Endpunkt)"
    );
}
