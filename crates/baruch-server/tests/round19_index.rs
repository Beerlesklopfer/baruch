// SPDX-License-Identifier: AGPL-3.0-or-later
//! Runde 19a (index): RAG-Index-Pflege – Auftrags-Worker ohne Versuchszyklus je Suche, liegengebliebene
//! Aufträge, `weak` an rag.search/rag_search, Abgleich mit MultiGPT.
//!
//! - Nach dem Rückzug (Einbetter scheitert) stoßen Suchen keinen neuen Versuch an; `rag.status` bleibt ehrlich
//!   (Problem und `index.stale`), bis eine echte Änderung oder „Index neu aufbauen“ die Aufträge zurückholt.
//! - `rag.reindex` antwortet sofort, auch wenn eine Einbettung hängt (`deferred`); ein dabei unterbrochener
//!   Auftrag eines anderen Werks kommt zurück in die Warteschlange.
//! - Einfälle eines gelöschten Werks sind sofort im gemeinsamen Bereich.
//! - `rag::catch_up` mit eigenem Datenordner ohne gespeicherte Verbindung Eliezer bindet keinen Einbetter.
//! - Aufgaben-Präfixe in LanceDB (Dokument/Anfrage), Wechsel = ModelMismatch bis zum Neuaufbau.
//! - `weak` je Treffer in `rag.search` und MCP `rag_search`.
//! - Abgleich mit MultiGPT gehärtet (MockDocs) und die Verbindungsprüfung endet mit `DELETE`.
//!
//! Einbettungen nur von eigenen Nachbauten ([`Recording`], [`MockEliezer`] auf 127.0.0.1:0), MultiGPT nur als
//! [`MockDocs`]/[`MockMultigpt`] auf 127.0.0.1:0. Texte frei erfunden („Der Hafen der Anker“).

use crate::common;

use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, header};
use baruch_llm::mock::MockEliezer;
use baruch_rag::{BoxFuture, Embedder, EmbedderFactory, Embeddings, RagError};
use baruch_server::connections::multigpt::mock::MockMultigpt;
use baruch_server::rag;
use baruch_server::sync::mock::{Deny, MockDocs};
use baruch_server::sync::{self, Limits};
use baruch_store::{ConnectionKind, SyncSide};
use serde_json::{Value, json};

const HARBOUR: &str = "// Testbuch Runde 19a (frei erfunden).
#set document(title: \"Der Hafen der Anker\", author: \"Mara Tau\")

= Die Mole

Am Ende der Mole lag ein rostiger Anker, so groß wie ein Ruderboot, und die Kinder kletterten darauf herum.

= Der Leuchtturm

Der Kompass des alten Kapitäns hing an der Wand des Turms und zeigte stur nach Norden.

= Das Wetter

Eine Wolke zog über die Bucht, und der Regen roch nach Salz.
";

fn result(answer: &Value) -> &Value {
    answer.get("result").unwrap_or_else(|| panic!("Fehler statt Ergebnis: {answer}"))
}

fn error_code(answer: &Value) -> i64 {
    answer["error"]["code"].as_i64().unwrap_or_else(|| panic!("Fehler erwartet: {answer}"))
}

fn way<'a>(answer: &'a Value, name: &str) -> &'a Value {
    answer["ways"].as_array().unwrap().iter().find(|w| w["way"] == name).unwrap_or_else(|| panic!("{name}: {answer}"))
}

fn hits(answer: &Value) -> Vec<Value> {
    answer["hits"].as_array().unwrap().clone()
}

/// Wie der Nachbau gerade antwortet.
#[derive(Debug, Clone)]
enum Mode {
    Ok,
    Fail(RagError),
    /// Hängt, bis wieder `Ok` gilt (höchstens 30 s), dann wie `Ok`.
    Hang,
}

/// Einbetter-Nachbau, der jeden Aufruf aufzeichnet. Vektoren nach Schlüsselwörtern (3 Dimensionen, von Hand
/// gewählt): „Anker“ → (1, 0, 0), „Kompass“ → Kosinus 0,5 zu „Anker“, „Wolke“ → Kosinus 0,4, sonst senkrecht.
#[derive(Clone)]
struct Recording {
    calls: Arc<Mutex<Vec<Vec<String>>>>,
    mode: Arc<Mutex<Mode>>,
    started: Arc<AtomicUsize>,
}

impl Recording {
    fn new(mode: Mode) -> Recording {
        Recording { calls: Arc::default(), mode: Arc::new(Mutex::new(mode)), started: Arc::default() }
    }

    fn set(&self, mode: Mode) {
        *self.mode.lock().unwrap() = mode;
    }

    /// Aufrufe, die keine Anfrage `query` (allein) waren: Einbettungen für den Index.
    fn index_calls(&self, query: &str) -> usize {
        self.calls.lock().unwrap().iter().filter(|texts| !(texts.len() == 1 && texts[0].ends_with(query))).count()
    }

    fn texts(&self) -> Vec<String> {
        self.calls.lock().unwrap().iter().flatten().cloned().collect()
    }
}

fn vector(text: &str) -> Vec<f32> {
    if text.contains("Anker") {
        vec![1.0, 0.0, 0.0]
    } else if text.contains("Kompass") {
        vec![0.5, 0.75f32.sqrt(), 0.0]
    } else if text.contains("Wolke") {
        vec![0.4, 0.84f32.sqrt(), 0.0]
    } else {
        vec![0.0, 0.0, 1.0]
    }
}

struct RecordingEmbedder(Recording, String);

impl Embedder for RecordingEmbedder {
    fn model(&self) -> &str {
        &self.1
    }
    fn embed<'a>(&'a self, texts: &'a [String]) -> BoxFuture<'a, Result<Embeddings, RagError>> {
        Box::pin(async move {
            self.0.started.fetch_add(1, Ordering::SeqCst);
            self.0.calls.lock().unwrap().push(texts.to_vec());
            let started = Instant::now();
            loop {
                let mode = self.0.mode.lock().unwrap().clone();
                match mode {
                    Mode::Hang if started.elapsed() < Duration::from_secs(30) => {
                        tokio::time::sleep(Duration::from_millis(20)).await;
                    }
                    Mode::Fail(e) => return Err(e),
                    _ => break,
                }
            }
            Ok(Embeddings { model: self.1.clone(), dimension: 3, vectors: texts.iter().map(|t| vector(t)).collect() })
        })
    }
}

impl EmbedderFactory for Recording {
    fn for_model(&self, model: &str) -> Arc<dyn Embedder> {
        Arc::new(RecordingEmbedder(self.clone(), baruch_llm::embedding_model_name(model)))
    }
}

async fn status(server: &common::Server, work: i64) -> Value {
    result(&server.rpc("rag.status", json!({ "work": work })).await).clone()
}

async fn search(server: &common::Server, params: Value) -> Value {
    result(&server.rpc("rag.search", params).await).clone()
}

/// Server mit eigenem Werk und dem aufzeichnenden Nachbau als Einbetter (das Werk ist schon eingereiht).
async fn harbour(mode: Mode) -> (common::Server, Recording, i64) {
    harbour_with(mode, None).await
}

/// Wie [`harbour`]; mit Aufgaben-Präfixen (Anfrage, Dokument), die vor dem ersten Auftrag gelten.
async fn harbour_with(mode: Mode, prefixes: Option<(&str, &str)>) -> (common::Server, Recording, i64) {
    let server = common::open().await;
    if let Some((query, document)) = prefixes {
        set_prefixes(&server, query, document).await;
    }
    let embedder = Recording::new(mode);
    rag::set_embedders(&server.state, Arc::new(embedder.clone()));
    let work = server.import(HARBOUR, "hafen.typ").await.work;
    (server, embedder, work)
}

/// Wie `rag::wait_idle`, mit Obergrenze (ein hängender Test soll scheitern, nicht ewig warten).
async fn idle(server: &common::Server) {
    tokio::time::timeout(Duration::from_secs(90), rag::wait_idle(&server.state))
        .await
        .expect("Index-Worker kommt innerhalb von 90 s zur Ruhe");
}

// ---- Worker, Status, Neuaufbau --------------------------------------------------------------------------

#[tokio::test]
async fn after_the_retreat_searches_start_no_new_embedding_cycle_and_the_status_stays_honest() {
    let failures = [
        RagError::NoEmbedder("Kein Einbettungsmodell erreichbar: Testnachbau aus.".into()),
        RagError::Timeout("Testnachbau: keine Antwort innerhalb von 120 s.".into()),
    ];
    for (round, failure) in failures.into_iter().enumerate() {
        let (server, embedder, work) = harbour(Mode::Fail(failure.clone())).await;
        rag::catch_up(&server.state).await;
        idle(&server).await;
        let after_retreat = embedder.index_calls("Anker");
        assert!(after_retreat > 0, "{failure:?}: der Start hat es versucht");
        let before = status(&server, work).await;
        assert!(before["index"]["stale"].as_u64().unwrap() > 0, "{failure:?}: liegengeblieben gezählt: {before}");
        assert!(before["index"]["problem"].is_string(), "{failure:?}: Problem gesetzt: {before}");

        for n in 1..=5 {
            let found = search(&server, json!({ "work": work, "query": "Anker" })).await;
            assert_eq!(
                way(&found, "lancedb")["state"],
                "failed",
                "{failure:?}: Suche {n} ehrlich ausgefallen: {found}"
            );
            // Gleich nach der Suche (das billige Aufräumen läuft womöglich noch) und danach: nie „kein Problem“.
            let now = status(&server, work).await;
            assert!(now["index"]["problem"].is_string(), "{failure:?}: Suche {n}, sofort: Problem bleibt: {now}");
            idle(&server).await;
            let later = status(&server, work).await;
            assert!(later["index"]["problem"].is_string(), "{failure:?}: Suche {n}, danach: Problem bleibt: {later}");
            assert!(later["index"]["stale"].as_u64().unwrap() > 0, "{failure:?}: Suche {n}: stale > 0: {later}");
            assert_eq!(way(&later, "lancedb")["state"], "failed", "{failure:?}: Weg ausgefallen: {later}");
            assert!(
                way(&later, "lancedb")["message"].as_str().unwrap().contains("liegengeblieben"),
                "{failure:?}: der Status sagt, dass Aufträge liegen: {later}"
            );
        }
        assert_eq!(
            embedder.index_calls("Anker"),
            after_retreat,
            "Begründung: Suchen reihen nur still das Aufräumen ein und überspringen liegengebliebene Aufträge – \
             fünf Suchen lösen keinen einzigen weiteren Einbettungsaufruf für den Index aus ({failure:?})"
        );

        // Eine echte Änderung (Runde 0) bzw. „Index neu aufbauen“ (Runde 1) holt die Aufträge zurück.
        embedder.set(Mode::Ok);
        if round == 0 {
            rag::notify(&server.state, None);
        } else {
            let queued = server.rpc("rag.reindex", json!({ "work": work, "confirm": true })).await;
            assert_eq!(result(&queued)["queued"], true, "neu aufbauen eingereiht: {queued}");
        }
        idle(&server).await;
        assert!(embedder.index_calls("Anker") > after_retreat, "{failure:?}: zurückgeholt und eingebettet");
        let healed = status(&server, work).await;
        assert_eq!(healed["index"]["stale"], 0, "{failure:?}: nichts liegt mehr: {healed}");
        assert!(healed["index"]["problem"].is_null(), "{failure:?}: Problem behoben: {healed}");
        let found = search(&server, json!({ "work": work, "query": "Anker" })).await;
        assert_eq!(way(&found, "lancedb")["state"], "connected", "{failure:?}: Suche geht wieder: {found}");
        assert!(hits(&found)[0]["snippet"].as_str().unwrap().contains("Anker"), "{failure:?}: Treffer: {found}");
    }
}

#[tokio::test]
async fn reindex_answers_at_once_while_an_embedding_hangs_and_rebuilds_after_release() {
    let (server, embedder, work) = harbour(Mode::Hang).await;
    rag::notify(&server.state, Some(work));
    let waited = Instant::now();
    while embedder.started.load(Ordering::SeqCst) == 0 {
        assert!(waited.elapsed() < Duration::from_secs(20), "der Worker beginnt einzubetten");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    // Zahl der Aufrufe des Einbetters, während der erste hängt (für die Prüfung der Unterbrechung unten).
    let hanging_calls = embedder.started.load(Ordering::SeqCst);
    let started = Instant::now();
    let queued = server.rpc("rag.reindex", json!({ "work": work, "confirm": true })).await;
    let took = started.elapsed();
    assert!(
        took < Duration::from_secs(2),
        "Begründung: rag.reindex wartet nicht auf die Schreibsperre des hängenden Workers ({took:?})"
    );
    assert_eq!(result(&queued)["deferred"], true, "Rücksetzwunsch vermerkt (Worker hielt die Sperre): {queued}");
    assert_eq!(result(&queued)["queued"], true, "eingereiht: {queued}");

    // Runde 23 (Backlog „Nach Runde 19 offen“): Vor der Freigabe warten, bis der Worker die hängende Einbettung
    // wirklich unterbrochen hat. Begründung: Der Worker prüft den Rücksetzwunsch alle 50 ms (`ABORT_POLL`), lässt
    // die hängende Einbettung fallen, setzt zurück und beginnt den wieder eingereihten Auftrag neu – das zeigt sich
    // als **weiterer** Aufruf des Einbetters, während dieser noch hängt. Gäbe der Test sofort frei, würde die
    // Einbettung regulär fertig, und der Test wäre grün, ohne dass je etwas unterbrochen wurde. Ohne Unterbrechung
    // bleibt `started` stehen (der Einbetter hängt bis zu 30 s), und diese Zusicherung schlägt nach 10 s an.
    let waited = Instant::now();
    while embedder.started.load(Ordering::SeqCst) == hanging_calls {
        assert!(
            waited.elapsed() < Duration::from_secs(10),
            "der Worker unterbricht die hängende Einbettung und beginnt den Auftrag neu, solange der Einbetter noch \
             hängt (kein neuer Aufruf seit {hanging_calls})"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }

    embedder.set(Mode::Ok);
    idle(&server).await;
    let after = status(&server, work).await;
    assert!(after["index"]["problem"].is_null(), "nach der Freigabe kein Problem: {after}");
    assert_eq!(after["index"]["pending"], 0, "nichts steht aus: {after}");
    assert!(after["index"]["built"].is_string(), "das Werk ist neu aufgebaut: {after}");
    let index = baruch_lancedb::Index::open(&server.path().join("lancedb")).await.unwrap();
    assert!(index.count(Some(baruch_lancedb::Scope::Work(work))).await.unwrap() >= 3, "drei Kapitel im Index");
    let found = search(&server, json!({ "work": work, "query": "Anker" })).await;
    assert!(hits(&found)[0]["snippet"].as_str().unwrap().contains("Anker"), "neu aufgebaut und auffindbar: {found}");

    // Ohne laufenden Auftrag geht es sofort (deferred false); die Suche eben reihte noch das Aufräumen ein.
    idle(&server).await;
    let queued = server.rpc("rag.reindex", json!({ "work": work, "confirm": true })).await;
    assert_eq!(result(&queued)["deferred"], false, "freie Sperre: sofort zurückgesetzt: {queued}");
    idle(&server).await;
}

const ORCHARD: &str = "// Zweites Testbuch Runde 19 (frei erfunden).
#set document(title: \"Der Obstgarten\", author: \"Lio Fenn\")

= Die Leiter

An der alten Leiter lehnte ein Korb voller Birnen.
";

#[tokio::test]
async fn reindexing_one_work_requeues_the_interrupted_job_of_another_work() {
    // Beide Werke erst fertig aufbauen; dann den Hafen allein neu aufbauen lassen (freie Sperre, sofort), so dass
    // genau ein Auftrag `Work(Hafen)` läuft und hängt – kein zweiter für dasselbe Werk wartet in der Schlange.
    let (server, embedder, harbour_work) = harbour(Mode::Ok).await;
    let orchard = server.import(ORCHARD, "obstgarten.typ").await.work;
    idle(&server).await;
    embedder.set(Mode::Hang);
    let before = embedder.started.load(Ordering::SeqCst);
    let first = server.rpc("rag.reindex", json!({ "work": harbour_work, "confirm": true })).await;
    assert_eq!(result(&first)["deferred"], false, "freie Sperre: sofort zurückgesetzt: {first}");
    let waited = Instant::now();
    while embedder.started.load(Ordering::SeqCst) == before {
        assert!(waited.elapsed() < Duration::from_secs(20), "der Worker beginnt einzubetten");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let hanging = embedder.calls.lock().unwrap().last().cloned().unwrap();
    assert!(
        hanging.iter().any(|t| t.contains("Mole")),
        "der hängende Auftrag gehört zum Hafen-Werk, nicht zum Obstgarten: {hanging:?}"
    );
    let queued = server.rpc("rag.reindex", json!({ "work": orchard, "confirm": true })).await;
    assert_eq!(result(&queued)["deferred"], true, "Rücksetzwunsch vermerkt (Worker hielt die Sperre): {queued}");
    // Der Worker prüft den Wunsch alle 50 ms; erst danach freigeben, sonst könnte die hängende Einbettung noch
    // regulär fertig werden und es gäbe nichts zu unterbrechen.
    tokio::time::sleep(Duration::from_millis(500)).await;

    embedder.set(Mode::Ok);
    idle(&server).await;
    let after = status(&server, harbour_work).await;
    assert_eq!(after["index"]["pending"], 0, "nichts steht aus: {after}");
    assert_eq!(after["index"]["stale"], 0, "nichts liegt: {after}");
    let index = baruch_lancedb::Index::open(&server.path().join("lancedb")).await.unwrap();
    assert!(
        index.count(Some(baruch_lancedb::Scope::Work(harbour_work))).await.unwrap() >= 3,
        "Begründung: Der vom Neuaufbau des Obstgartens unterbrochene Auftrag des Hafen-Werks kommt zurück in die \
         Warteschlange und läuft zu Ende – sonst blieben seine drei Kapitel aus dem Index verschwunden, ohne \
         pending oder stale (Gegenprobe ohne Wiedereinreihen: 0 Zeilen): {after}"
    );
    assert!(after["index"]["built"].is_string(), "das Werk trägt einen Aufbau-Zeitpunkt: {after}");
    assert!(
        index.count(Some(baruch_lancedb::Scope::Work(orchard))).await.unwrap() >= 1,
        "der Obstgarten ist neu aufgebaut"
    );
}

#[tokio::test]
async fn ideas_of_a_deleted_work_are_found_at_once_in_the_shared_area_of_another_work() {
    let (server, _embedder, work) = harbour(Mode::Ok).await;
    let created = server
        .rpc(
            "idea.create",
            json!({ "work": work, "words": "Anker", "note": "Der Anker an der Mole als Bild für Halt." }),
        )
        .await;
    let idea = result(&created)["idea"]["id"].as_i64().unwrap();
    rag::catch_up(&server.state).await;
    idle(&server).await;
    let deleted = server.rpc("work.delete", json!({ "work": work, "confirm_title": "Der Hafen der Anker" })).await;
    result(&deleted);
    idle(&server).await;
    // Keine Suche dazwischen: Erst die Suche frischt den gemeinsamen Bereich sonst auf (bis zu 30 s später).
    let other = server.state.default_work().await.unwrap();
    let found = search(&server, json!({ "work": other, "query": "Anker", "collections": ["idea"] })).await;
    let hit = hits(&found).into_iter().find(|h| h["document"] == format!("idea:{idea}"));
    let hit = hit.unwrap_or_else(|| {
        panic!(
            "Begründung: Job::Work eines gelöschten Werks reiht den gemeinsamen Bereich gleich ein – der Einfall ist \
             jetzt „ohne Werk“ und in einem anderen Werk sofort auffindbar: {found}"
        )
    });
    assert!(hit["meta"]["work"].is_null(), "der Einfall gehört zu keinem Werk mehr: {hit}");
    let index = baruch_lancedb::Index::open(&server.path().join("lancedb")).await.unwrap();
    assert_eq!(
        index.count(Some(baruch_lancedb::Scope::Work(work))).await.unwrap(),
        0,
        "das Werk hat keine Zeilen mehr"
    );
}

#[tokio::test]
async fn catch_up_with_own_data_and_no_saved_eliezer_binds_no_embedder_and_says_why() {
    let server = common::open().await;
    let work = server.state.default_work().await.unwrap();
    rag::catch_up(&server.state).await;
    rag::wait_idle(&server.state).await;
    let now = status(&server, work).await;
    assert_eq!(
        now["index"]["problem"],
        rag::NO_EMBEDDER_OWN_DATA,
        "Begründung: eigener Datenordner, Verbindung Eliezer nie gespeichert – beim Start keine Anfrage an die \
         Vorgabe-Adresse, und der Status sagt warum: {now}"
    );
    assert!(rag::NO_EMBEDDER_OWN_DATA.starts_with("Kein Einbettungsmodell angebunden"), "wie NO_EMBEDDER");
    assert_eq!(now["index"]["pending"], 0, "nichts eingereiht (ohne Einbetter gibt es keine Aufträge): {now}");
    let refused = server.rpc("rag.reindex", json!({ "work": work, "confirm": true })).await;
    assert_eq!(error_code(&refused), -32000, "neu aufbauen ohne Einbetter: abgelehnt: {refused}");
    assert!(refused["error"]["message"].as_str().unwrap().contains("eigener Datenordner"), "mit Grund: {refused}");

    // Wird Eliezer gespeichert (hier der Nachbau), bindet „Index neu aufbauen“ ihn an.
    let mock = MockEliezer::start().await;
    result(&server.rpc("connection.set", json!({ "kind": "eliezer", "config": { "url": mock.url() } })).await);
    let queued = server.rpc("rag.reindex", json!({ "work": work, "confirm": true })).await;
    assert_eq!(result(&queued)["queued"], true, "jetzt eingereiht: {queued}");
    idle(&server).await;
    assert!(!mock.embedding_inputs().is_empty(), "über den gespeicherten Nachbau eingebettet");
    assert!(status(&server, work).await["index"]["problem"].is_null(), "kein Problem mehr");

    // Mit gesetztem Nachbau-Einbetter unverändert: catch_up bettet über ihn ein.
    let (other, embedder, work) = harbour(Mode::Ok).await;
    rag::catch_up(&other.state).await;
    idle(&other).await;
    assert!(embedder.index_calls("Anker") > 0, "gesetzter Einbetter: der Start bettet ein");
    assert!(status(&other, work).await["index"]["problem"].is_null(), "kein Problem");
}

// ---- Präfixe ---------------------------------------------------------------------------------------------

async fn set_prefixes(server: &common::Server, query: &str, document: &str) {
    let config = json!({ "dir": "lancedb", "embedding_model": "nomic-embed-text", "query_prefix": query,
                         "document_prefix": document });
    server.state.store.connection_set(ConnectionKind::Lancedb, &config).await.unwrap();
}

#[tokio::test]
async fn lancedb_embeds_documents_and_queries_with_their_prefixes_and_a_changed_prefix_waits_for_reindex() {
    // Ohne Präfix: byte-gleich wie bisher.
    let (plain, plain_embedder, plain_work) = harbour(Mode::Ok).await;
    rag::catch_up(&plain.state).await;
    idle(&plain).await;
    let loaded = plain.state.store.load_work(plain_work).await.unwrap();
    let desired: Vec<String> = rag::index::desired_sections(plain_work, &loaded).into_iter().map(|e| e.text).collect();
    let embedded = plain_embedder.texts();
    for text in &desired {
        assert!(
            embedded.contains(text),
            "Begründung: ohne Präfix geht der Text eines Stücks unverändert ans Modell: {text:?} fehlt in {embedded:?}"
        );
    }
    search(&plain, json!({ "work": plain_work, "query": "Anker" })).await;
    assert_eq!(plain_embedder.texts().last().map(String::as_str), Some("Anker"), "Anfrage unverändert");

    // Mit Präfixen (über den Store gesetzt; die Seite lernt die Felder mit pgvector).
    let (server, embedder, work) = harbour_with(Mode::Ok, Some(("search_query: ", "search_document: "))).await;
    rag::catch_up(&server.state).await;
    idle(&server).await;
    let texts = embedder.texts();
    assert!(!texts.is_empty(), "eingebettet");
    assert!(
        texts.iter().all(|t| t.starts_with("search_document: ")),
        "Begründung: Dokumente gehen mit dem Dokument-Präfix ans Modell: {texts:?}"
    );
    for text in &desired {
        assert!(texts.contains(&format!("search_document: {text}")), "Präfix + Stück byte-gleich: {text:?}");
    }
    let found = search(&server, json!({ "work": work, "query": "Anker" })).await;
    assert_eq!(embedder.texts().last().map(String::as_str), Some("search_query: Anker"), "Anfrage mit Anfrage-Präfix");
    assert_eq!(way(&found, "lancedb")["state"], "connected", "Suche geht: {found}");
    let meta = std::fs::read_to_string(server.path().join("lancedb/baruch-index.json")).unwrap();
    assert!(meta.contains("\"document_prefix\": \"search_document: \""), "Präfix im Index vermerkt: {meta}");

    // Anderes Dokument-Präfix: nichts gemischt, bis neu aufgebaut wird.
    set_prefixes(&server, "search_query: ", "passage: ").await;
    let found = search(&server, json!({ "work": work, "query": "Anker" })).await;
    let lance = way(&found, "lancedb");
    assert_eq!(lance["state"], "failed", "Präfixwechsel: ausgefallen: {found}");
    assert!(
        lance["message"].as_str().unwrap().contains(rag::index::PREFIX_CHANGED),
        "Begründung: ein anderes Präfix ist wie ein anderes Modell (ModelMismatch) – ehrlicher Satz: {found}"
    );
    idle(&server).await;
    let before = embedder.calls.lock().unwrap().len();
    rag::notify(&server.state, Some(work));
    idle(&server).await;
    assert_eq!(embedder.calls.lock().unwrap().len(), before, "nichts mit dem neuen Präfix in den alten Index gemischt");
    let now = status(&server, work).await;
    assert!(now["index"]["problem"].as_str().unwrap().contains(rag::index::PREFIX_CHANGED), "Status: {now}");

    let queued = server.rpc("rag.reindex", json!({ "work": work, "confirm": true })).await;
    assert_eq!(result(&queued)["queued"], true, "neu aufbauen: {queued}");
    idle(&server).await;
    let rebuilt: Vec<String> = embedder.calls.lock().unwrap()[before..].iter().flatten().cloned().collect();
    assert!(
        !rebuilt.is_empty() && rebuilt.iter().all(|t| t.starts_with("passage: ")),
        "der ganze Index neu mit dem neuen Präfix: {rebuilt:?}"
    );
    let found = search(&server, json!({ "work": work, "query": "Anker" })).await;
    assert_eq!(way(&found, "lancedb")["state"], "connected", "nach dem Neuaufbau wieder verbunden: {found}");
}

// ---- schwach -------------------------------------------------------------------------------------------

async fn mcp(server: &common::Server, name: &str, arguments: Value) -> Value {
    let message = json!({ "jsonrpc": "2.0", "id": 7, "method": "tools/call", "params": { "name": name, "arguments": arguments } });
    let mut request = Request::post("/mcp")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(message.to_string()))
        .unwrap();
    request.extensions_mut().insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 40000))));
    let (_, _, body) = server.send(request).await;
    let answer: Value = serde_json::from_str(&body).unwrap_or(Value::Null);
    assert_ne!(answer["result"]["isError"], true, "{name} gelingt: {answer}");
    let text = answer["result"]["content"][0]["text"].as_str().unwrap_or("");
    serde_json::from_str(text).unwrap_or_else(|_| answer["result"]["structuredContent"].clone())
}

fn check_weak(answer: &Value, what: &str) {
    let list = hits(answer);
    assert!(!list.is_empty(), "{what}: Treffer: {answer}");
    for hit in &list {
        let score = hit["score"].as_f64().unwrap() as f32;
        assert_eq!(
            hit["weak"].as_bool(),
            Some(score < baruch_rag::WEAK_SCORE),
            "Begründung: weak genau für score < WEAK_SCORE ({}): {what}: {hit}",
            baruch_rag::WEAK_SCORE
        );
    }
    let by_word = |word: &str| {
        list.iter().find(|h| h["snippet"].as_str().unwrap_or("").contains(word)).cloned().unwrap_or_else(|| {
            panic!("{what}: Treffer mit „{word}“ fehlt: {answer}");
        })
    };
    assert_eq!(by_word("Anker")["weak"], false, "{what}: Kosinus 1 ist nicht schwach");
    assert_eq!(by_word("Kompass")["weak"], false, "{what}: Kosinus 0,5 liegt über der Schwelle 0,45");
    assert_eq!(by_word("Wolke")["weak"], true, "{what}: Kosinus 0,4 ist schwach");
}

#[tokio::test]
async fn rag_search_and_mcp_rag_search_mark_exactly_the_hits_below_the_threshold_as_weak() {
    let (server, _embedder, work) = harbour(Mode::Ok).await;
    rag::catch_up(&server.state).await;
    idle(&server).await;
    let params = json!({ "work": work, "query": "Anker", "collections": ["section"], "limit": 10 });
    let found = search(&server, params.clone()).await;
    check_weak(&found, "rag.search");
    let found = mcp(&server, "rag_search", json!({ "work": work, "query": "Anker", "limit": 10 })).await;
    check_weak(&found, "MCP rag_search");
}

// ---- Abgleich mit MultiGPT -----------------------------------------------------------------------------

const ISLAND: &str = "// Testbuch Runde 19a „abgleich“ (frei erfunden).
#set document(title: \"Die Insel der Möwen\", author: \"Ole Wind\")

= Ankunft

Das Boot legte am Steg an, und die Möwen schrien über den Dünen.

= Leer

= Der Abschied

Im Herbst wurden die Boote an Land gezogen und mit Planen bedeckt.
";

const COLLECTION: &str = "Baruch: Die Insel der Möwen";

async fn island() -> (common::Server, MockDocs, i64) {
    let server = common::open().await;
    let mock = MockDocs::start().await;
    result(&server.rpc("connection.set", json!({ "kind": "multigpt", "config": { "url": mock.url() } })).await);
    let work = server.import(ISLAND, "insel.typ").await.work;
    (server, mock, work)
}

async fn run(server: &common::Server) -> Value {
    let started = server.rpc("sync.run", json!({})).await;
    assert_eq!(result(&started)["started"], true, "der Lauf startet: {started}");
    sync::wait_idle(&server.state).await;
    let status = server.rpc("sync.status", json!({})).await;
    result(&status)["last"].clone()
}

async fn upload_on(server: &common::Server, work: i64) {
    let on = server.rpc("sync.set_upload", json!({ "work": work, "on": true, "confirm": true })).await;
    assert_eq!(result(&on)["upload"], true, "eingeschaltet: {on}");
}

#[tokio::test]
async fn an_upload_without_id_gives_a_hint_and_is_not_repeated_while_the_content_is_the_same() {
    let (server, mock, work) = island().await;
    upload_on(&server, work).await;
    mock.configure(|b| b.upload_without_id = true);
    let last = run(&server).await;
    let message = last["message"].as_str().unwrap();
    assert!(message.contains("keine Dokument-ID"), "Hinweis: {last}");
    assert_eq!(last["uploaded"], 3, "hochgeladen gezählt (das Dokument liegt bei MultiGPT): {last}");
    assert_eq!(mock.documents(COLLECTION).len(), 3, "drei Dokumente bei MultiGPT");
    let states = server.state.store.sync_states(SyncSide::Baruch, Some(COLLECTION)).await.unwrap();
    assert!(
        states.len() == 3 && states.iter().all(|s| s.remote_id.is_empty() && s.hash.len() == 64),
        "Sperrvermerk: Stand mit Hash, ohne ID: {states:?}"
    );
    mock.clear_requests();
    let last = run(&server).await;
    assert!(
        mock.tool_calls("upload_document").is_empty(),
        "Begründung: gleicher Inhalt (Hash im Sperrvermerk) – kein erneutes Hochladen: {last}"
    );
    assert_eq!(last["unchanged"], 3, "alles unverändert: {last}");
    assert_eq!(mock.documents(COLLECTION).len(), 3, "keine Doppel bei MultiGPT");
}

#[tokio::test]
async fn an_error_for_one_document_blocks_only_that_one_but_missing_rights_block_the_rest() {
    let (server, mock, work) = island().await;
    upload_on(&server, work).await;
    mock.configure(|b| b.reject_empty = true);
    let last = run(&server).await;
    let titles: Vec<String> =
        mock.tool_calls("upload_document").iter().map(|a| a["title"].as_str().unwrap().to_owned()).collect();
    assert_eq!(titles, vec!["1 Ankunft", "2 Leer", "3 Der Abschied"], "alle drei versucht: {titles:?}");
    assert_eq!(
        (last["uploaded"].as_i64(), last["failed"].as_i64()),
        (Some(2), Some(1)),
        "Begründung: das isError für das leere Kapitel blockiert die übrigen Kapitel nicht: {last}"
    );
    assert_eq!(last["ok"], false, "mit Hinweis: {last}");
    let message = last["message"].as_str().unwrap();
    assert!(message.contains("Leerer Inhalt"), "der Grund der Gegenstelle steht da: {message}");
    assert!(!message.contains("ruhen"), "kein „Weitere Änderungen ruhen“ für ein einzelnes Dokument: {message}");
    let states = server.state.store.sync_states(SyncSide::Baruch, Some(COLLECTION)).await.unwrap();
    assert_eq!(states.len(), 2, "nur die hochgeladenen Kapitel haben einen Stand (keine Marke bleibt): {states:?}");
    mock.clear_requests();
    run(&server).await;
    assert_eq!(mock.tool_calls("upload_document").len(), 1, "der nächste Lauf versucht nur das leere Kapitel erneut");

    // Fehlende Rechte (HTTP 403 bzw. isError mit „docs.write“) blockieren weiter nach dem ersten Versuch.
    for deny in [Deny::Forbidden, Deny::ToolError] {
        let (server, mock, work) = island().await;
        upload_on(&server, work).await;
        mock.deny_write(Some(deny));
        let last = run(&server).await;
        assert_eq!(mock.tool_calls("upload_document").len(), 1, "{deny:?}: nach dem ersten Fehler kein Versuch mehr");
        assert_eq!(last["failed"], 1, "{deny:?}: ein Fehler: {last}");
        assert!(last["message"].as_str().unwrap().contains("ruhen"), "{deny:?}: der Rest ruht: {last}");
        let states = server.state.store.sync_states(SyncSide::Baruch, None).await.unwrap();
        assert!(states.is_empty(), "{deny:?}: keine Marke bleibt stehen (nichts kam an): {states:?}");
    }
}

#[tokio::test]
async fn a_capped_listing_deletes_no_local_copy() {
    let (server, mock, _work) = island().await;
    for (id, title) in [("g-1", "Psalmen am Meer"), ("g-2", "Lieder der Fischer"), ("g-3", "Gebete im Sturm")] {
        mock.put_document("Gemeinde", id, title, &format!("{title}: frei erfundener Text."));
    }
    // Das Schema nennt für limit höchstens 2 (die Werkzeugliste merkt sich die Sitzung, darum von Anfang an).
    mock.configure(|b| {
        for tool in b.tools.iter_mut().filter(|t| t["name"] == "list_documents") {
            tool["inputSchema"]["properties"]["limit"]["maximum"] = json!(2);
        }
    });
    result(&server.rpc("sync.set_collections", json!({ "collections": ["Gemeinde"] })).await);
    let last = run(&server).await;
    assert_eq!(last["downloaded"], 3, "drei geholt: {last}");

    // Jetzt kappt der Server still bei zwei Einträgen (wie erbeten), aber ohne Fortsetzung.
    mock.configure(|b| b.silent_cap = 2);
    let last = run(&server).await;
    let local: Vec<(String, String)> = server.state.store.sync_document_ids().await.unwrap();
    assert_eq!(
        local.len(),
        3,
        "Begründung: eine volle Seite ohne has_more/Fortsetzung kann unvollständig sein – keine lokale Kopie wird \
         gelöscht: {local:?} ({last})"
    );
    assert!(last["message"].as_str().unwrap().contains("womöglich unvollständig"), "Hinweis: {last}");
    assert_eq!(last["removed"], 0, "nichts entfernt: {last}");
    assert_eq!(mock.tool_calls("list_documents").last().unwrap()["limit"], 2, "limit am maximum des Schemas gekappt");
}

#[tokio::test]
async fn a_deselected_collection_leaves_no_remote_hits_in_rag_search() {
    let (server, mock, work) = island().await;
    let eliezer = MockEliezer::start().await;
    result(&server.rpc("connection.set", json!({ "kind": "eliezer", "config": { "url": eliezer.url() } })).await);
    rag::set_embedders(&server.state, rag::eliezer_embedders(&server.state));
    rag::catch_up(&server.state).await;
    idle(&server).await;
    mock.put_document("Leuchtfeuer", "lf-1", "Das Feuer", "Der Wärter putzte jeden Abend die Linse der Laterne.");
    result(&server.rpc("sync.set_collections", json!({ "collections": ["Leuchtfeuer"] })).await);
    run(&server).await;
    idle(&server).await;
    let query = json!({ "work": work, "query": "Wärter putzte die Linse der Laterne", "collections": ["remote"] });
    let found = search(&server, query.clone()).await;
    assert!(hits(&found).iter().any(|h| h["meta"]["document"] == "lf-1"), "geholt und gefunden: {found}");

    result(&server.rpc("sync.set_collections", json!({ "collections": [] })).await);
    run(&server).await;
    idle(&server).await;
    let found = search(&server, query).await;
    assert!(
        hits(&found).iter().all(|h| h["collection"] != "remote"),
        "Begründung: abgewählt → lokale Kopie entfernt → der Index zieht nach; keine Treffer der Art remote: {found}"
    );
    assert!(eliezer.requests().iter().all(|r| r.path == "/v1/embeddings"), "nur Einbettungen beim Nachbau");
}

#[tokio::test]
async fn renaming_a_work_keeps_its_collection_and_uploads_nothing_again() {
    let (server, mock, work) = island().await;
    upload_on(&server, work).await;
    run(&server).await;
    let renamed = server.rpc("work.update", json!({ "work": work, "title": "Die Insel der Seeschwalben" })).await;
    result(&renamed);
    mock.clear_requests();
    let last = run(&server).await;
    assert!(
        mock.tool_calls("upload_document").is_empty() && mock.tool_calls("delete_document").is_empty(),
        "Begründung: die Sammlung ist beim Einschalten festgehalten, die Kapitel sind unverändert: {last}"
    );
    assert_eq!(last["unchanged"], 3, "drei unverändert: {last}");
    let status = result(&server.rpc("sync.status", json!({ "work": work })).await).clone();
    assert_eq!(status["works"][0]["collection"], COLLECTION, "die Sammlung trägt weiter den alten Titel: {status}");
    assert_eq!(mock.documents(COLLECTION).len(), 3, "die Dokumente bleiben in der alten Sammlung");
    assert!(mock.documents("Baruch: Die Insel der Seeschwalben").is_empty(), "keine zweite Sammlung");
}

#[tokio::test]
async fn an_answer_lost_between_upload_and_state_is_reported_by_the_next_run() {
    let (server, mock, work) = island().await;
    upload_on(&server, work).await;
    sync::set_limits(&server.state, Limits { call_timeout: Duration::from_secs(5), ..Limits::DEFAULT });
    mock.configure(|b| b.lose_upload_answer = true);
    let last = run(&server).await;
    assert_eq!(last["ok"], false, "der Lauf scheitert: {last}");
    assert_eq!(mock.tool_calls("upload_document").len(), 1, "nach dem verlorenen Hochladen ruht der Rest: {last}");
    assert_eq!(mock.documents(COLLECTION).len(), 1, "bei MultiGPT liegt das Dokument trotzdem");
    let states = server.state.store.sync_states(SyncSide::Baruch, Some(COLLECTION)).await.unwrap();
    assert_eq!(states.len(), 1, "die Marke „Hochladen begonnen“ bleibt stehen: {states:?}");

    mock.configure(|b| b.lose_upload_answer = false);
    let last = run(&server).await;
    let message = last["message"].as_str().unwrap();
    assert!(
        message.contains("brach beim Hochladen von „1 Ankunft“ ab") && message.contains("doppelt"),
        "Begründung: Der Abbruch zwischen Hochladen und Stand bleibt nicht stumm – der Hinweis nennt das Kapitel: \
         {message}"
    );
    assert_eq!(last["uploaded"], 3, "alle drei jetzt hochgeladen (das erste erneut): {last}");
    assert_eq!(mock.documents(COLLECTION).len(), 4, "das erste liegt womöglich doppelt (wie der Hinweis sagt)");
    let states = server.state.store.sync_states(SyncSide::Baruch, Some(COLLECTION)).await.unwrap();
    assert!(
        states.len() == 3 && states.iter().all(|s| s.remote_stamp.is_empty() && !s.remote_id.is_empty()),
        "danach keine Marke mehr: {states:?}"
    );
    let last = run(&server).await;
    assert!(!last["message"].as_str().unwrap().contains("brach"), "der Hinweis kommt nur einmal: {last}");
}

// ---- Verbindungsprüfung -------------------------------------------------------------------------------

#[tokio::test]
async fn checking_multigpt_ends_its_session_with_delete() {
    let server = common::open().await;
    let mock = MockMultigpt::start().await;
    result(&server.rpc("connection.set", json!({ "kind": "multigpt", "config": { "url": mock.url() } })).await);
    let checked = server.rpc("connection.check", json!({ "kind": "multigpt" })).await;
    assert_eq!(result(&checked)["ok"], true, "Prüfung gelingt: {checked}");
    let requests = mock.requests();
    let last = requests.last().unwrap();
    assert_eq!(
        last["method"], "DELETE",
        "Begründung: die Prüfung nutzt keinen Sitzungsvorrat und beendet ihre MCP-Sitzung zuletzt mit DELETE: {requests:?}"
    );
    assert_eq!(last["session"], "nachbau-sitzung-1", "DELETE trägt die Mcp-Session-Id: {last}");
    assert_eq!(requests.iter().filter(|r| r["method"] == "DELETE").count(), 1, "genau ein DELETE: {requests:?}");
}
