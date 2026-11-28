// SPDX-License-Identifier: AGPL-3.0-or-later
//! Round 12, task „lancedb“: the main RAG in the data folder through the service `crate::rag` – catching up the index
//! (only changed pieces), search through RPC `rag.search` with states per way, model change, fallback without an
//! embedding model, `rag.status`, `rag.reindex` and „Verbindung prüfen“.
//!
//! Embeddings only from the mock [`MockEliezer`] on 127.0.0.1:0 (never the real Eliezer); texts freely invented
//! („Das Jahr am Leuchtturm“).

use crate::common;

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use baruch_llm::mock::MockEliezer;
use baruch_rag::{BoxFuture, Embedder, EmbedderFactory, Embeddings, RagError};
use baruch_server::i18n::{self, Lang};
use baruch_server::rag;
use serde_json::{Value, json};

const LIGHTHOUSE: &str = "// Testbuch Runde 12 (frei erfunden).
= Der Wärter

Jeden Abend stieg der Wärter die einhundertzwölf Stufen hinauf, putzte das Glas der Laterne, zündete den Docht \
an und sah zu, wie der Lichtkegel über die Klippen wanderte. Die Möwen schliefen dann schon auf den Felsen.

Bei Nebel ließ er das Nebelhorn tuten, alle dreißig Atemzüge einmal, bis die Fischer im Hafen das Signal \
erwiderten. Das Nebelhorn klang tief und traurig, und die Kinder im Dorf zählten die Stöße mit.

Im Logbuch notierte er Wind, Wellenhöhe und die Namen der Schiffe, die vorbeizogen. Das Logbuch lag auf dem \
Tisch neben der Kanne mit Tee, und seine Seiten rochen nach Salz und Petroleum.

= Das Senfkorn

Im kleinen Garten hinter dem Turm säte die Tochter des Wärters ein Senfkorn, das kleinste aller Samenkörner. \
Sie goss es jeden Morgen mit Regenwasser aus der Tonne, und im Sommer stand dort ein Strauch voller Spatzen.
";

/// A sentence of the semantic search in the German interface language (the test router pins German, Round 39c).
fn de(sentence: fn() -> String) -> String {
    i18n::sync_scope(Lang::De, sentence)
}

fn result(answer: &Value) -> &Value {
    answer.get("result").unwrap_or_else(|| panic!("Fehler statt Ergebnis: {answer}"))
}

fn error_code(answer: &Value) -> i64 {
    answer["error"]["code"].as_i64().unwrap_or_else(|| panic!("Fehler erwartet: {answer}"))
}

async fn nodes(server: &common::Server, work: i64) -> Vec<Value> {
    let loaded = server.rpc("work.get", json!({ "work": work })).await;
    result(&loaded)["work"]["nodes"].as_array().unwrap().clone()
}

/// Server with a mock as Eliezer, own work, index built.
async fn lighthouse() -> (common::Server, MockEliezer, i64) {
    let server = common::open().await;
    let mock = MockEliezer::start().await;
    let set = server.rpc("connection.set", json!({ "kind": "eliezer", "config": { "url": mock.url() } })).await;
    result(&set);
    rag::set_embedders(&server.state, rag::eliezer_embedders(&server.state));
    let work = server.import(LIGHTHOUSE, "leuchtturm.typ").await.work;
    rag::catch_up(&server.state).await;
    rag::wait_idle(&server.state).await;
    (server, mock, work)
}

async fn search(server: &common::Server, params: Value) -> Value {
    let answer = server.rpc("rag.search", params).await;
    result(&answer).clone()
}

fn way<'a>(answer: &'a Value, name: &str) -> &'a Value {
    answer["ways"]
        .as_array()
        .unwrap()
        .iter()
        .find(|w| w["way"] == name)
        .unwrap_or_else(|| panic!("{name} fehlt: {answer}"))
}

fn hits(answer: &Value) -> Vec<Value> {
    answer["hits"].as_array().unwrap().clone()
}

fn index_dir(server: &common::Server) -> std::path::PathBuf {
    server.path().join("lancedb")
}

#[tokio::test]
async fn the_index_follows_saving_switching_and_ideas_and_embeds_only_changed_pieces() {
    let (server, mock, work) = lighthouse().await;
    assert!(mock.requests().iter().all(|r| r.path == "/v1/embeddings"), "nur Einbettungen, kein Chat");
    let list = nodes(&server, work).await;
    let keeper = list[0]["key"].as_i64().unwrap();

    let found = search(&server, json!({ "work": work, "query": "Nebelhorn tuten Fischer Signal" })).await;
    let first = &hits(&found)[0];
    assert_eq!(first["way"], "lancedb", "Treffer aus dem lokalen Index: {found}");
    assert_eq!(first["collection"], "section", "Art section: {found}");
    assert_eq!(first["document"], format!("work:{work}"), "Dokument = Werk: {found}");
    assert_eq!(first["section"], format!("key:{keeper}"), "Stelle = Schlüssel des Abschnitts: {found}");
    assert_eq!(first["url"], format!("/?work={work}&key={keeper}"), "Link in Key-Form: {found}");
    assert_eq!(first["meta"]["key"], keeper, "meta.key: {found}");
    assert_eq!(first["meta"]["rev"], 1, "meta.rev = aktuelle Fassung: {found}");
    assert_eq!(first["meta"]["chunk"], 2, "das Nebelhorn steht im zweiten Stück: {found}");
    assert!(first["snippet"].as_str().unwrap().contains("Nebelhorn"), "Ausschnitt ist das beste Stück: {found}");
    assert_eq!(first["found_by"][0]["way"], "lancedb", "found_by nennt den Weg: {found}");
    assert_eq!(
        hits(&found).iter().filter(|h| h["section"] == format!("key:{keeper}")).count(),
        1,
        "mehrere Stücke desselben Abschnitts sind ein Treffer: {found}"
    );
    assert_eq!(way(&found, "lancedb")["state"], "connected", "LanceDB verbunden: {found}");

    // Change only the third piece: exactly one text is embedded anew.
    let before = mock.embedding_inputs().len();
    let body = list[0]["revisions"][0]["body"].as_str().unwrap().replace(
        "Im Logbuch notierte er Wind",
        "Im Logbuch notierte er Sturmflut, Leuchtfeuer der Nachbarinsel und Wind",
    );
    let saved = server.rpc("outline.save", json!({ "work": work, "key": keeper, "body": body, "based_on": 1 })).await;
    result(&saved);
    rag::wait_idle(&server.state).await;
    let inputs = mock.embedding_inputs();
    let new: Vec<&String> = inputs[before..].iter().collect();
    assert_eq!(new.len(), 1, "nur das geänderte Stück neu eingebettet (Fassungsnummer allein zählt nicht): {new:?}");
    assert!(new[0].contains("Sturmflut") && new[0].starts_with("Im Logbuch"), "das dritte Stück: {new:?}");

    let found =
        search(&server, json!({ "work": work, "query": "Sturmflut Leuchtfeuer Nachbarinsel", "limit": 1 })).await;
    assert_eq!(hits(&found).len(), 1, "limit 1: {found}");
    assert_eq!(hits(&found)[0]["meta"]["rev"], 2, "Treffer aus der neuen Fassung: {found}");
    assert_eq!(hits(&found)[0]["meta"]["chunk"], 3, "drittes Stück: {found}");

    // Switching back: the old third piece comes back, the others stay untouched.
    let before = mock.embedding_inputs().len();
    let back = server.rpc("revision.set_current", json!({ "work": work, "key": keeper, "rev": 1 })).await;
    result(&back);
    rag::wait_idle(&server.state).await;
    let inputs = mock.embedding_inputs();
    assert_eq!(inputs.len() - before, 1, "nur das alte dritte Stück: {:?}", &inputs[before..]);
    let found = search(&server, json!({ "work": work, "query": "Sturmflut Leuchtfeuer Nachbarinsel" })).await;
    assert!(
        hits(&found).iter().all(|h| !h["snippet"].as_str().unwrap().contains("Sturmflut")),
        "der Text der abgeschalteten Fassung ist aus dem Index: {found}"
    );

    // Ideas: create, change, delete.
    let created = server
        .rpc(
            "idea.create",
            json!({ "work": work, "words": "Kompass", "note": "Ein Kompass aus Messing im Nachttisch." }),
        )
        .await;
    let id = result(&created)["idea"]["id"].as_i64().unwrap();
    rag::wait_idle(&server.state).await;
    let found =
        search(&server, json!({ "work": work, "query": "Kompass Messing Nachttisch", "collections": ["idea"] })).await;
    let idea = &hits(&found)[0];
    assert_eq!(idea["document"], format!("idea:{id}"), "Einfall als Dokument idea:<id>: {found}");
    assert_eq!(idea["meta"]["idea"], id, "meta.idea: {found}");
    assert_eq!(idea["url"], format!("/einfaelle/{id}"), "Link auf den Einfall: {found}");
    assert!(hits(&found).iter().all(|h| h["collection"] == "idea"), "collections schränkt auf Einfälle ein: {found}");

    let before = mock.embedding_inputs().len();
    let changed = server.rpc("idea.update", json!({ "id": id, "note": "Ein Kompass aus Bronze im Nachttisch." })).await;
    result(&changed);
    rag::wait_idle(&server.state).await;
    let inputs = mock.embedding_inputs();
    assert_eq!(inputs.len() - before, 1, "der geänderte Einfall, sonst nichts: {:?}", &inputs[before..]);
    assert!(inputs.last().unwrap().contains("Bronze"), "neuer Text eingebettet");

    let deleted = server.rpc("idea.delete", json!({ "id": id, "confirm": true })).await;
    result(&deleted);
    rag::wait_idle(&server.state).await;
    let found =
        search(&server, json!({ "work": work, "query": "Kompass Bronze Nachttisch", "collections": ["idea"] })).await;
    assert!(hits(&found).iter().all(|h| h["document"] != format!("idea:{id}")), "gelöschter Einfall ist weg: {found}");

    // The search enqueues the clean-up; after that the queue rests.
    rag::wait_idle(&server.state).await;
    let status = result(&server.rpc("rag.status", json!({ "work": work })).await).clone();
    assert_eq!(status["index"]["model"], "text-embedding-nomic-embed-text-v1.5", "Modell vermerkt: {status}");
    assert_eq!(status["index"]["dimension"], baruch_llm::mock::MOCK_DIMENSION, "Dimension vermerkt: {status}");
    assert_eq!(status["index"]["configured_model"], "nomic-embed-text", "eingestellt (Vorgabe): {status}");
    assert!(status["index"]["built"].is_string(), "Zeitpunkt des Aufbaus: {status}");
    assert_eq!(status["index"]["pending"], 0, "nichts ausstehend: {status}");
    assert!(status["index"]["problem"].is_null(), "kein Problem: {status}");
    assert_eq!(status["fts"], true, "Wortlaut-Suche immer da: {status}");
    let meta = std::fs::read_to_string(index_dir(&server).join("baruch-index.json")).expect("baruch-index.json");
    assert!(meta.contains("\"dimension\": 512"), "Datei nennt die Dimension: {meta}");
}

#[tokio::test]
async fn highlights_and_library_entries_are_found_and_a_deleted_work_takes_its_entries_along() {
    let (server, _mock, work) = lighthouse().await;
    let list = server.rpc("library.list", json!(null)).await;
    let title = result(&list)["titles"][0].clone();
    let added = server
        .rpc("highlight.add", json!({ "title": title["id"], "text": "Quargelperle im Nebelmeer", "kind": "core" }))
        .await;
    let highlight = result(&added)["highlight"]["id"].as_i64().unwrap();
    // Highlights do not report through the mirror: notify(None) or the next search enqueues.
    rag::notify(&server.state, None);
    rag::wait_idle(&server.state).await;
    let found =
        search(&server, json!({ "work": work, "query": "Quargelperle Nebelmeer", "collections": ["highlight"] })).await;
    let hit = &hits(&found)[0];
    assert_eq!(hit["collection"], "highlight", "Art highlight: {found}");
    assert_eq!(hit["section"], format!("highlight:{highlight}"), "Stelle = Hervorhebung: {found}");
    assert_eq!(hit["document"], format!("title:{}", title["id"]), "Dokument = Titel: {found}");
    assert_eq!(
        hit["url"],
        format!("/quelle?work={work}&title={}&highlight={highlight}", title["id"]),
        "Link auf die Quelle: {found}"
    );
    let found = search(&server, json!({ "work": work, "query": title["title"], "collections": ["library"] })).await;
    assert!(
        hits(&found).iter().any(|h| h["document"] == format!("title:{}", title["id"])),
        "Bibliothekseintrag auffindbar: {found}"
    );

    let index = baruch_lancedb::Index::open(&index_dir(&server)).await.unwrap();
    let rows = index.count(Some(baruch_lancedb::Scope::Work(work))).await.unwrap();
    assert!(rows >= 4, "das Werk hat Stücke im Index: {rows}");
    let works = server.state.store.works().await.unwrap();
    let title = works.iter().find(|(id, _, _)| *id == work).map(|(_, _, t)| t.clone()).unwrap();
    let deleted = server.rpc("work.delete", json!({ "work": work, "confirm_title": title })).await;
    result(&deleted);
    // Deleting a work does not report through the mirror; the next search cleans up.
    let other = server.state.default_work().await.unwrap();
    search(&server, json!({ "work": other, "query": "Garten" })).await;
    rag::wait_idle(&server.state).await;
    let shared = index.count(Some(baruch_lancedb::Scope::Shared)).await.unwrap();
    assert_eq!(index.count(Some(baruch_lancedb::Scope::Work(work))).await.unwrap(), 0, "gelöschtes Werk: Einträge weg");
    assert!(shared > 0, "die Bibliothek bleibt: {shared}");
}

#[tokio::test]
async fn only_one_embedding_request_runs_at_a_time_and_batches_hold_at_most_32_texts() {
    let (server, mock, work) = lighthouse().await;
    mock.set_embedding_delay(Duration::from_millis(30));
    let mut book = String::from("// Viele Absätze (frei erfunden).\n= Das Logbuch\n\n");
    for i in 0..70 {
        book.push_str(&format!(
            "Eintrag {i}: Heute zog die Möwe{i} über das Watt, der Wind kam aus Nordwest, das Wasser stand hoch \
             am Deich, und der Wärter schrieb alles in sein Buch, Zeile für Zeile, mit Tinte aus dem Fass.\n\n"
        ));
    }
    book.push_str("= Anhang\n\nEin kurzer Anhang mit Tabellen der Gezeiten, Flut und Ebbe, für das ganze Jahr, Monat für Monat und Tag für Tag, sorgfältig abgeschrieben aus dem Almanach.\n");
    let second = server.import(&book, "logbuch.typ").await.work;
    let keeper = nodes(&server, work).await[0].clone();
    let key = keeper["key"].as_i64().unwrap();
    let body = keeper["revisions"][0]["body"].as_str().unwrap().replace("Möwen schliefen", "Möwen träumten");
    let (save, a, b, c) = tokio::join!(
        server.rpc("outline.save", json!({ "work": work, "key": key, "body": body, "based_on": 1 })),
        server.rpc("rag.search", json!({ "work": work, "query": "Möwen Felsen" })),
        server.rpc("rag.search", json!({ "work": second, "query": "Gezeiten Almanach" })),
        server.rpc("rag.search", json!({ "work": work, "query": "Senfkorn Garten" })),
    );
    for answer in [&save, &a, &b, &c] {
        result(answer);
    }
    rag::wait_idle(&server.state).await;
    assert_eq!(mock.embed_max_active(), 1, "höchstens eine Einbettungsanfrage zur Zeit bei Eliezer");
    let sizes: Vec<usize> = mock.embedding_bodies().iter().map(|b| b["input"].as_array().unwrap().len()).collect();
    assert!(sizes.iter().all(|&n| (1..=32).contains(&n)), "Stapel 1..=32: {sizes:?}");
    assert!(sizes.contains(&32), "70 Absätze ergeben volle Stapel zu 32: {sizes:?}");
    let found = search(&server, json!({ "work": second, "query": "Möwe69 Watt" })).await;
    assert!(
        hits(&found).iter().any(|h| h["snippet"].as_str().unwrap().contains("Möwe69")),
        "auch der letzte Stapel ist im Index: {found}"
    );
}

#[tokio::test]
async fn a_different_model_is_reported_as_mismatch_without_mixing_and_reindex_rebuilds() {
    let (server, mock, work) = lighthouse().await;
    mock.set_embedding_models(&["text-embedding-nomic-embed-text-v1.5", "bge-m3"]);
    let index = baruch_lancedb::Index::open(&index_dir(&server)).await.unwrap();
    let rows = index.count(None).await.unwrap();
    let set =
        server.rpc("connection.set", json!({ "kind": "lancedb", "config": { "embedding_model": "bge-m3" } })).await;
    result(&set);
    let before = mock.embedding_inputs().len();
    let found = search(&server, json!({ "work": work, "query": "Nebelhorn" })).await;
    let lance = way(&found, "lancedb");
    assert_eq!(lance["state"], "failed", "anderes Modell: ausgefallen: {found}");
    assert!(
        lance["message"].as_str().unwrap().contains("bge-m3")
            && lance["message"].as_str().unwrap().contains("neu aufbauen"),
        "ehrlicher Satz mit Ausweg: {found}"
    );
    assert!(hits(&found).is_empty(), "keine Treffer aus einem fremden Modell: {found}");

    // Saving embeds nothing with the wrong model.
    let keeper = nodes(&server, work).await[0].clone();
    let body = keeper["revisions"][0]["body"].as_str().unwrap().replace("Möwen", "Kormorane");
    let saved =
        server.rpc("outline.save", json!({ "work": work, "key": keeper["key"], "body": body, "based_on": 1 })).await;
    result(&saved);
    rag::wait_idle(&server.state).await;
    assert_eq!(mock.embedding_inputs().len(), before, "nichts eingebettet, solange das Modell nicht passt");
    assert_eq!(index.count(None).await.unwrap(), rows, "keine Zeile dazu, keine gemischt");
    let status = result(&server.rpc("rag.status", json!({ "work": work })).await).clone();
    assert!(status["index"]["problem"].as_str().unwrap().contains("bge-m3"), "Status nennt den Grund: {status}");
    assert_eq!(way(&status, "lancedb")["state"], "failed", "Status: ausgefallen: {status}");

    let refused = server.rpc("rag.reindex", json!({ "work": work })).await;
    assert_eq!(error_code(&refused), -32602, "ohne confirm: -32602");
    let refused = server.rpc("rag.reindex", json!({ "confirm": true })).await;
    assert_eq!(error_code(&refused), -32602, "ohne work: -32602");
    let queued = server.rpc("rag.reindex", json!({ "work": work, "confirm": true })).await;
    assert_eq!(result(&queued)["queued"], true, "eingereiht: {queued}");
    assert!(result(&queued)["pending"].as_u64().unwrap() >= 1, "ausstehend gezählt: {queued}");
    rag::wait_idle(&server.state).await;
    let models: Vec<Value> = mock.embedding_bodies()[..].iter().map(|b| b["model"].clone()).collect();
    assert!(models[models.len() - 1] == "bge-m3", "neu aufgebaut mit dem neuen Modell: {models:?}");
    let status = result(&server.rpc("rag.status", json!({ "work": work })).await).clone();
    assert_eq!(status["index"]["model"], "bge-m3", "Index vermerkt das neue Modell: {status}");
    assert!(status["index"]["problem"].is_null(), "Problem behoben: {status}");
    let found = search(&server, json!({ "work": work, "query": "Kormorane Felsen" })).await;
    assert_eq!(way(&found, "lancedb")["state"], "connected", "wieder verbunden: {found}");
    assert!(hits(&found)[0]["snippet"].as_str().unwrap().contains("Kormorane"), "neuer Text gefunden: {found}");
}

/// Embedder that always fails and counts.
struct Failing(Arc<AtomicUsize>, RagError);

impl Embedder for Failing {
    fn model(&self) -> &str {
        "text-embedding-nomic-embed-text-v1.5"
    }
    fn embed<'a>(&'a self, _texts: &'a [String]) -> BoxFuture<'a, Result<Embeddings, RagError>> {
        Box::pin(async move {
            self.0.fetch_add(1, Ordering::SeqCst);
            Err(self.1.clone())
        })
    }
}

impl EmbedderFactory for Failing {
    fn for_model(&self, _model: &str) -> Arc<dyn Embedder> {
        Arc::new(Failing(self.0.clone(), self.1.clone()))
    }
}

#[tokio::test]
async fn without_an_embedding_model_lancedb_fails_honestly_and_fts_keeps_working() {
    let server = common::open().await;
    let work = server.import(LIGHTHOUSE, "leuchtturm.typ").await.work;
    // Without an embedder (as in tests and helper commands): honest, without network.
    let found = search(&server, json!({ "work": work, "query": "Nebelhorn" })).await;
    let lance = way(&found, "lancedb");
    assert_eq!(lance["state"], "failed", "kein Einbettungsmodell: ausgefallen: {found}");
    assert_eq!(lance["message"], de(rag::no_embedder), "fester Satz: {found}");
    let status = result(&server.rpc("rag.status", json!({ "work": work })).await).clone();
    assert_eq!(status["index"]["problem"], de(rag::no_embedder), "Status nennt den Grund: {status}");
    let fts = server.rpc("library.search", json!({ "work": work, "query": "Nebelhorn" })).await;
    assert!(!result(&fts)["hits"].as_array().unwrap().is_empty(), "Wortlaut-Suche (FTS) geht weiter: {fts}");

    // Mock gone (address points to a closed port): NoEmbedder sentence.
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    drop(listener);
    let set = server.rpc("connection.set", json!({ "kind": "eliezer", "config": { "url": url } })).await;
    result(&set);
    rag::set_embedders(&server.state, rag::eliezer_embedders(&server.state));
    let found = search(&server, json!({ "work": work, "query": "Nebelhorn" })).await;
    let message = way(&found, "lancedb")["message"].as_str().unwrap().to_owned();
    assert!(message.starts_with("Kein Einbettungsmodell erreichbar"), "NoEmbedder-Satz: {found}");
    assert!(message.contains("Wortlaut-Suche (FTS) geht weiter"), "mit Hinweis auf FTS: {message}");
}

#[tokio::test]
async fn failing_embeddings_are_retried_a_bounded_number_of_times_without_an_endless_loop() {
    let server = common::open().await;
    let work = server.import(LIGHTHOUSE, "leuchtturm.typ").await.work;
    let calls = Arc::new(AtomicUsize::new(0));
    let failing =
        Failing(calls.clone(), RagError::NoEmbedder("Kein Einbettungsmodell erreichbar: Testnachbau aus.".into()));
    rag::set_embedders(&server.state, Arc::new(failing));
    rag::notify(&server.state, Some(work));
    rag::notify(&server.state, Some(work));
    rag::wait_idle(&server.state).await;
    assert_eq!(calls.load(Ordering::SeqCst), rag::MAX_ATTEMPTS, "ein Auftrag (entdoppelt), höchstens drei Versuche");
    tokio::time::sleep(Duration::from_millis(1500)).await;
    assert_eq!(calls.load(Ordering::SeqCst), rag::MAX_ATTEMPTS, "danach Ruhe: keine Dauerschleife");
    let status = result(&server.rpc("rag.status", json!({ "work": work })).await).clone();
    assert!(status["index"]["problem"].as_str().unwrap().contains("Testnachbau aus"), "Grund im Status: {status}");
    assert_eq!(status["index"]["pending"], 0, "nichts läuft weiter: {status}");
    // A new save tries again (also what was left lying), limited again.
    rag::notify(&server.state, None);
    rag::wait_idle(&server.state).await;
    let after = calls.load(Ordering::SeqCst);
    assert!(after <= 3 * rag::MAX_ATTEMPTS, "neuer Anstoß, wieder begrenzt (Rückzug ohne Einbetter): {after}");
    assert!(after > rag::MAX_ATTEMPTS, "neuer Anstoß versucht es wieder: {after}");
}

#[tokio::test]
async fn rag_search_validates_its_parameters_and_always_names_all_three_ways() {
    let (server, _mock, work) = lighthouse().await;
    for (params, why) in [
        (json!({ "work": work, "query": "  " }), "leere Anfrage"),
        (json!({ "work": work, "query": "x".repeat(1001) }), "über 1000 Zeichen"),
        (json!({ "work": work, "query": "x", "limit": 0 }), "limit 0"),
        (json!({ "work": work, "query": "x", "limit": 51 }), "limit 51"),
        (json!({ "work": work, "query": "x", "ways": ["fts"] }), "unbekannter Weg"),
        (json!({ "work": 9999, "query": "x" }), "unbekanntes Werk"),
        (json!({ "work": work }), "query fehlt"),
    ] {
        let answer = server.rpc("rag.search", params).await;
        assert_eq!(error_code(&answer), -32602, "{why}: -32602 ({answer})");
    }
    let found = search(&server, json!({ "work": work, "query": "Senfkorn" })).await;
    let names: Vec<&str> = found["ways"].as_array().unwrap().iter().map(|w| w["way"].as_str().unwrap()).collect();
    assert_eq!(names, ["lancedb", "pgvector", "multigpt"], "immer alle drei, feste Reihenfolge: {found}");
    assert_eq!(way(&found, "pgvector")["state"], "off", "Vorgabe nur lancedb: pgvector aus: {found}");
    assert_eq!(way(&found, "pgvector")["message"], de(rag::not_chosen), "Vertragssatz: {found}");
    assert_eq!(way(&found, "multigpt")["label"], "aus", "label auf Deutsch: {found}");
    assert_eq!(found["query"], "Senfkorn", "Anfrage beschnitten zurück: {found}");

    let set =
        server.rpc("connection.rag_set", json!({ "work": work, "ways": ["lancedb", "pgvector", "multigpt"] })).await;
    result(&set);
    let found = search(&server, json!({ "work": work, "query": "Senfkorn" })).await;
    assert_eq!(way(&found, "pgvector")["state"], "off", "ohne gewählte Sammlung aus: {found}");
    assert_eq!(way(&found, "pgvector")["message"], de(rag::no_collection), "Vertragssatz: {found}");
    assert_eq!(way(&found, "multigpt")["message"], de(rag::no_collection), "Vertragssatz: {found}");
    let found = search(&server, json!({ "work": work, "query": "Senfkorn", "ways": ["pgvector"] })).await;
    assert_eq!(way(&found, "lancedb")["message"], de(rag::not_asked), "weggelassen: {found}");
    assert!(hits(&found).is_empty(), "ohne LanceDB keine Treffer: {found}");
    let found = search(&server, json!({ "work": work, "query": "Senfkorn", "collections": ["Predigten"] })).await;
    assert_eq!(way(&found, "lancedb")["state"], "off", "keine LanceDB-Art gewünscht: aus: {found}");

    // Wired since the integration: pgvector without a host is „aus“. Since Round 24 MultiGPT no longer has a default
    // address: without a stored address this way is „aus“ too (not set up), without network.
    let opened = rag::hooks::open_pgvector(&server.state, rag::eliezer_embedders(&server.state)).await;
    assert!(opened.err().is_some_and(|e| e.is_off()), "pgvector ohne Rechner: nicht eingerichtet, Weg aus");
    let opened = rag::hooks::open_multigpt(&server.state).await;
    assert!(opened.err().is_some_and(|e| e.is_off()), "MultiGPT ohne Adresse: nicht eingerichtet, Weg aus");

    let status = result(&server.rpc("rag.status", json!({ "work": work })).await).clone();
    let names: Vec<&str> = status["ways"].as_array().unwrap().iter().map(|w| w["way"].as_str().unwrap()).collect();
    assert_eq!(names, ["lancedb", "pgvector", "multigpt"], "Status: alle drei: {status}");
    assert_eq!(way(&status, "lancedb")["state"], "connected", "Index bereit: {status}");
    assert!(status["index"]["dir"].as_str().unwrap().ends_with("lancedb"), "Vorgabe-Ordner: {status}");
    assert!(status["index"]["entries"].as_u64().unwrap() >= 4, "Einträge des Werks samt Bibliothek: {status}");
    let refused = server.rpc("rag.status", json!({ "work": 9999 })).await;
    assert_eq!(error_code(&refused), -32602, "unbekanntes Werk");
}

#[tokio::test]
async fn checking_lancedb_reports_folder_index_model_and_probe_and_saves_the_result() {
    let server = common::open().await;
    let checked = i18n::scope(Lang::De, rag::check_lancedb(&server.state)).await.expect("Prüfung");
    assert_eq!(checked["kind"], "lancedb", "Art: {checked}");
    assert_eq!(checked["ok"], false, "ohne Einbetter nicht bereit: {checked}");
    assert_eq!(checked["details"]["embedder"], de(rag::no_embedder), "Grund im Feld embedder: {checked}");
    assert_eq!(checked["details"]["writable"], true, "Ordner beschreibbar: {checked}");
    assert!(checked["at"].is_string(), "Zeitpunkt: {checked}");

    let mock = MockEliezer::start().await;
    result(&server.rpc("connection.set", json!({ "kind": "eliezer", "config": { "url": mock.url() } })).await);
    rag::set_embedders(&server.state, rag::eliezer_embedders(&server.state));
    rag::catch_up(&server.state).await;
    rag::wait_idle(&server.state).await;
    let checked = i18n::scope(Lang::De, rag::check_lancedb(&server.state)).await.expect("Prüfung");
    let details = &checked["details"];
    assert_eq!(checked["ok"], true, "bereit: {checked}");
    assert_eq!(details["embedder"], "ok", "Probe gelungen: {checked}");
    assert_eq!(details["exists"], true, "Ordner da: {checked}");
    assert_eq!(details["tables"], json!(["chunks"]), "eine Tabelle: {checked}");
    assert!(details["entries"].as_u64().unwrap() > 0, "Einträge: {checked}");
    assert_eq!(details["model"], "text-embedding-nomic-embed-text-v1.5", "Modell des Index: {checked}");
    assert_eq!(details["dimension"], baruch_llm::mock::MOCK_DIMENSION, "Dimension: {checked}");
    assert_eq!(details["configured_model"], "nomic-embed-text", "eingestellt: {checked}");
    assert!(mock.embedding_inputs().iter().any(|t| t == rag::PROBE_TEXT), "fester Probetext eingebettet");
    let row = server.rpc("connection.get", json!({ "kind": "lancedb" })).await;
    assert_eq!(result(&row)["check"]["ok"], true, "Ergebnis gespeichert: {row}");
    assert_eq!(
        result(&row)["check"]["details"]["dimension"],
        baruch_llm::mock::MOCK_DIMENSION,
        "details gespeichert: {row}"
    );

    result(&server.rpc("connection.set", json!({ "kind": "lancedb", "config": { "dir": "works/rag" } })).await);
    let checked = i18n::scope(Lang::De, rag::check_lancedb(&server.state)).await.expect("Prüfung");
    assert_eq!(checked["ok"], false, "Ordner im Git-Spiegel abgelehnt: {checked}");
    assert!(checked["message"].as_str().unwrap().contains("Git-Spiegel"), "Grund: {checked}");
    assert!(!server.path().join("works/rag").exists(), "nichts im Spiegel angelegt");
}

#[tokio::test]
async fn a_rejected_highlight_leaves_the_index_like_in_the_wording_search() {
    let (server, _mock, work) = lighthouse().await;
    let list = server.rpc("library.list", json!(null)).await;
    let title = result(&list)["titles"][0].clone();
    let add = |text: &'static str| {
        let server = &server;
        let title = title.clone();
        async move {
            let added =
                server.rpc("highlight.add", json!({ "title": title["id"], "text": text, "kind": "core" })).await;
            result(&added)["highlight"]["id"].as_i64().unwrap()
        }
    };
    let kept = add("Wolkenharfe über der Bucht").await;
    let rejected = add("Wolkenharfe unter dem Kai").await;
    let update = server.rpc("highlight.update", json!({ "highlight": rejected, "status": "rejected" })).await;
    assert_eq!(result(&update)["highlight"]["status"], "rejected", "abgelehnt gespeichert: {update}");

    // Reason: the wanted content of the shared area decides what is embedded.
    let desired = rag::index::desired_shared(&server.state.store).await.unwrap();
    let sections: Vec<String> = desired.iter().filter_map(|e| e.section.clone()).collect();
    assert!(sections.contains(&format!("highlight:{kept}")), "aktive Hervorhebung im Index: {sections:?}");
    assert!(
        !sections.contains(&format!("highlight:{rejected}")),
        "abgelehnte Hervorhebung fehlt im Index (FTS schließt sie ebenso aus): {sections:?}"
    );

    rag::wait_idle(&server.state).await;
    let found =
        search(&server, json!({ "work": work, "query": "Wolkenharfe", "collections": ["highlight"], "limit": 10 }))
            .await;
    let places: Vec<Value> = hits(&found).iter().map(|h| h["section"].clone()).collect();
    assert!(places.contains(&json!(format!("highlight:{kept}"))), "die aktive wird gefunden: {found}");
    assert!(
        !places.contains(&json!(format!("highlight:{rejected}"))),
        "die abgelehnte taucht in der Bedeutungssuche nicht auf: {found}"
    );
}
