// SPDX-License-Identifier: AGPL-3.0-or-later
//! Round 12, task „pgvector“: pgvector directly against an **own** PostgreSQL (podman container with pgvector,
//! otherwise initdb cluster without pgvector, otherwise honestly skipped; `pgvector_support`). Freely invented data
//! („Das Jahr am Leuchtturm“, „Senfkornbuch“), mock embedder with fixed vectors – no Eliezer, no pgvector of the user.
//!
//! Checked: „Verbindung prüfen“ shows collections with count, dimension and models; wrong column/table → `ok: false`
//! with the name; the search delivers only hits of the chosen collections in cosine order with `score = 1 − distance`;
//! model or dimension do not fit → `ModelMismatch` without a query; the transaction is READ ONLY and has a time limit;
//! SQL special characters go in only as parameters.

use crate::common;
use crate::pgvector_support;

use std::sync::Arc;
use std::time::{Duration, Instant};

use baruch_rag::{Query, RagError, Way};
use baruch_server::AppState;
use baruch_server::connections::MemorySecrets;
use baruch_server::connections::pgvector::{self, TIMEOUT, begin_read_only};
use pgvector_support::{FixedFactory, PASSWORD, Pg};
use serde_json::{Value, json};

async fn open_with(secrets: MemorySecrets) -> common::Server {
    let data = tempfile::tempdir().unwrap();
    let state = AppState::open_with_secrets(data.path(), Arc::new(secrets)).await.expect("Datenordner öffnen");
    let state = Arc::new(state);
    common::Server { app: common::router(state.clone()), state, data }
}

async fn ok(server: &common::Server, method: &str, params: Value) -> Value {
    let answer = server.rpc(method, params).await;
    assert!(answer["error"].is_null(), "{method} sollte gelingen: {answer}");
    answer["result"].clone()
}

/// Invented sections in three collections; vectors of dimension 3.
const FIXTURE: &str = "
CREATE EXTENSION IF NOT EXISTS vector;
CREATE TABLE chunks (
  id serial PRIMARY KEY, collection text, document text, section text, title text, url text, content text,
  embedding vector(3), model text
);
INSERT INTO chunks (collection, document, section, title, url, content, embedding, model) VALUES
  ('leuchtturm', 'logbuch', 'fruehling', 'Logbuch Frühling', 'https://leuchtturm.example/logbuch#f',
   'Die Möwen kehren im Frühling zurück.', '[1,0,0]', 'nomic-embed-text'),
  ('leuchtturm', 'logbuch', 'herbst', 'Logbuch Herbst', 'javascript:alert(1)',
   'Der Nebel liegt auf dem Wasser.', '[0.6,0.8,0]', 'nomic-embed-text'),
  ('leuchtturm', 'karte', NULL, NULL, NULL, 'Die Seekarte zeigt die Untiefen.', '[0,1,0]', 'nomic-embed-text'),
  ('senfkorn', 'saat', '1', 'Saat', NULL, 'Klein beginnt es.', '[0.9,0.1,0]', 'nomic-embed-text'),
  ('fremd', 'anderswo', NULL, 'Anderswo', NULL, 'Mit einem anderen Modell eingebettet.', '[1,0,0]', 'bge-m3');
CREATE TABLE lose (id serial PRIMARY KEY, collection text, document text, content text, embedding vector);
INSERT INTO lose (collection, document, content, embedding) VALUES ('leuchtturm', 'zettel', 'Ein Zettel.', '[0,0,1]');
";

/// Settings with the whole mapping to the table `chunks` (all fields, so that nothing of an earlier setting remains),
/// `extra` on top.
fn mapped(pg: &Pg, extra: Value) -> Value {
    let mut config = pg.config();
    for (k, v) in [
        ("table", "chunks"),
        ("text_column", "content"),
        ("vector_column", "embedding"),
        ("collection_column", "collection"),
        ("document_column", "document"),
        ("section_column", "section"),
        ("title_column", "title"),
        ("url_column", "url"),
        ("model_column", "model"),
    ] {
        config[k] = json!(v);
    }
    for (k, v) in extra.as_object().unwrap() {
        config[k] = v.clone();
    }
    config
}

/// Save the settings, then store the password for exactly this target.
async fn configure(server: &common::Server, config: Value) {
    ok(server, "connection.set", json!({ "kind": "pgvector", "config": config })).await;
    ok(server, "connection.set_secret", json!({ "kind": "pgvector", "secret": PASSWORD })).await;
}

fn query(text: &str, collections: &[&str]) -> Query {
    Query {
        text: text.to_owned(),
        limit: 10,
        collections: collections.iter().map(|c| (*c).to_owned()).collect(),
        work: 1,
    }
}

fn factory() -> Arc<FixedFactory> {
    Arc::new(FixedFactory::new(vec![], vec![1.0, 0.0, 0.0]))
}

async fn count_chunks(pg: &Pg) -> i64 {
    let mut conn = pg.connect().await;
    sqlx::query_scalar("SELECT count(*) FROM chunks").fetch_one(&mut conn).await.expect("Tabelle chunks lesbar")
}

#[tokio::test]
async fn pgvector_against_an_own_server_checks_searches_and_stays_read_only() {
    let Some(pg) = pgvector_support::start("pgvector").await else { return };
    eprintln!("eigenes PostgreSQL: {} auf 127.0.0.1:{}", pg.origin, pg.port);

    // READ ONLY and time limit hold independently of pgvector (in the initdb cluster too).
    pg.exec("CREATE TABLE probe_schreiben (x int)").await;
    let mut conn = pg.connect().await;
    let mut tx = begin_read_only(&mut conn, TIMEOUT).await.expect("Transaktion READ ONLY");
    let insert = sqlx::query("INSERT INTO probe_schreiben VALUES (1)").execute(&mut *tx).await;
    let code = insert.as_ref().err().and_then(|e| e.as_database_error()).and_then(|d| d.code()).map(|c| c.into_owned());
    assert_eq!(
        code.as_deref(),
        Some("25006"),
        "INSERT in der Transaktion scheitert als read_only_sql_transaction: {insert:?}"
    );
    drop(tx);
    let mut tx = begin_read_only(&mut conn, TIMEOUT).await.unwrap();
    let create = sqlx::query("CREATE TABLE eingeschleust (x int)").execute(&mut *tx).await;
    assert!(create.is_err(), "auch CREATE TABLE scheitert (READ ONLY)");
    drop(tx);
    let mut tx = begin_read_only(&mut conn, Duration::from_secs(1)).await.unwrap();
    let start = Instant::now();
    let sleep = sqlx::query("SELECT pg_sleep(5)").execute(&mut *tx).await;
    let code = sleep.as_ref().err().and_then(|e| e.as_database_error()).and_then(|d| d.code()).map(|c| c.into_owned());
    assert_eq!(code.as_deref(), Some("57014"), "statement_timeout bricht pg_sleep ab (query_canceled): {sleep:?}");
    assert!(
        start.elapsed() < Duration::from_secs(4),
        "nach etwa 1 s abgebrochen, nicht nach 5 s: {:?}",
        start.elapsed()
    );
    drop(tx);
    let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM probe_schreiben").fetch_one(&mut conn).await.unwrap();
    assert_eq!(rows, 0, "nichts wurde geschrieben");

    let server = open_with(MemorySecrets::default()).await;
    if !pg.vector {
        configure(&server, mapped(&pg, json!({}))).await;
        let result = ok(&server, "connection.check", json!({ "kind": "pgvector" })).await;
        assert_eq!(result["ok"], false, "ohne Erweiterung vector nicht nutzbar: {result}");
        assert!(result["message"].as_str().unwrap().contains("Erweiterung vector fehlt"), "Meldung: {result}");
        eprintln!("ÜBERSPRUNGEN: Prüfung und Suche mit pgvector ({} hat keine Erweiterung vector)", pg.origin);
        return;
    }
    pg.exec(FIXTURE).await;

    // ---- Checking the connection ----------------------------------------------------------------
    configure(&server, mapped(&pg, json!({}))).await;
    let result = ok(&server, "connection.check", json!({ "kind": "pgvector" })).await;
    assert_eq!(result["ok"], true, "verbunden, vector da, Tabelle und Spalten passen, Dimension bekannt: {result}");
    let details = &result["details"];
    assert_eq!(
        details["collections"],
        json!([
            { "name": "fremd", "count": 1 },
            { "name": "leuchtturm", "count": 3 },
            { "name": "senfkorn", "count": 1 },
        ]),
        "Sammlungen mit Zahl der Abschnitte, nach Namen: {details}"
    );
    assert_eq!(details["chunks"], 5, "alle Abschnitte gezählt");
    assert_eq!(details["dimension"], 3, "Dimension aus vector(3)");
    assert_eq!(details["models"], json!(["bge-m3", "nomic-embed-text"]), "vermerkte Modelle aus model_column");
    assert_eq!(details["table"], "public.chunks", "Tabelle mit dem gefundenen Schema");
    assert_eq!(details["columns"]["vector"], "embedding", "Zuordnung der Rollen genannt");
    let message = result["message"].as_str().unwrap();
    assert!(
        message.contains("3 Sammlungen") && message.contains("Achtung") && message.contains("bge-m3"),
        "Meldung nennt Zahl und warnt vor dem fremden Modell: {message}"
    );
    assert!(!result.to_string().contains(PASSWORD), "kein Passwort im Ergebnis");

    let work = server.state.default_work().await.unwrap();
    let got = ok(&server, "connection.collections_get", json!({ "work": work, "way": "pgvector" })).await;
    assert_eq!(got["available"], details["collections"], "verfügbare Sammlungen aus der letzten Prüfung: {got}");
    assert!(got["available_at"].is_string(), "Zeit der Prüfung: {got}");
    assert_eq!(got["collections"], json!([]), "noch keine gewählt");

    // Without a fixed dimension: the first row counts.
    configure(
        &server,
        json!({ "table": "lose", "section_column": "", "title_column": "", "url_column": "", "model_column": "" }),
    )
    .await;
    let loose = ok(&server, "connection.check", json!({ "kind": "pgvector" })).await;
    assert_eq!(
        (loose["ok"].clone(), loose["details"]["dimension"].clone()),
        (json!(true), json!(3)),
        "vector ohne (n): {loose}"
    );

    // Wrong column or table: ok false with the name.
    configure(&server, mapped(&pg, json!({ "text_column": "inhalt" }))).await;
    let wrong = ok(&server, "connection.check", json!({ "kind": "pgvector" })).await;
    assert_eq!(wrong["ok"], false, "fehlende Spalte: {wrong}");
    assert!(wrong["message"].as_str().unwrap().contains("„inhalt“"), "Meldung nennt die fehlende Spalte: {wrong}");
    configure(&server, mapped(&pg, json!({ "table": "gibtsnicht" }))).await;
    let wrong = ok(&server, "connection.check", json!({ "kind": "pgvector" })).await;
    assert_eq!(wrong["ok"], false, "fehlende Tabelle: {wrong}");
    assert!(wrong["message"].as_str().unwrap().contains("„gibtsnicht“"), "Meldung nennt die Tabelle: {wrong}");
    configure(&server, mapped(&pg, json!({ "vector_column": "content" }))).await;
    let wrong = ok(&server, "connection.check", json!({ "kind": "pgvector" })).await;
    assert!(
        wrong["ok"] == false && wrong["message"].as_str().unwrap().contains("nicht vector"),
        "Textspalte als Vektor abgelehnt: {wrong}"
    );

    // ---- Search ---------------------------------------------------------------------------------
    configure(&server, mapped(&pg, json!({}))).await;
    let embedders = factory();
    let way = pgvector::open_way(&server.state, embedders.clone()).await.expect("Weg pgvector offen");
    assert_eq!(way.way(), Way::Pgvector, "Weg pgvector");
    assert_eq!(*embedders.made.lock().unwrap(), ["nomic-embed-text"], "Einbetter für embedding_model geholt");
    let hits = way.search(&query("Wann kehren die Möwen zurück?", &["leuchtturm"])).await.expect("Suche");
    let shown: Vec<(&str, Option<&str>)> = hits.iter().map(|h| (h.document.as_str(), h.section.as_deref())).collect();
    assert_eq!(
        shown,
        [("logbuch", Some("fruehling")), ("logbuch", Some("herbst")), ("karte", None)],
        "nur „leuchtturm“ (senfkorn läge sonst auf Platz 2), nach Kosinus-Abstand aufsteigend"
    );
    let scores: Vec<f32> = hits.iter().map(|h| h.score).collect();
    for (got, want) in scores.iter().zip([1.0_f32, 0.6, 0.0]) {
        assert!((got - want).abs() < 1e-4, "score = 1 − Kosinus-Abstand ([1,0,0] gegen die Vektoren): {scores:?}");
    }
    assert!(hits.iter().all(|h| h.way == Way::Pgvector && h.collection == "leuchtturm"), "Weg und Sammlung: {hits:?}");
    assert_eq!(hits[0].title, "Logbuch Frühling", "Titel aus title_column");
    assert_eq!(hits[2].title, "karte", "ohne Titel: das Dokument");
    assert_eq!(hits[0].url.as_deref(), Some("https://leuchtturm.example/logbuch#f"), "http(s)-Adresse bleibt");
    assert_eq!(hits[1].url, None, "javascript: wird verworfen");
    assert_eq!(hits[0].snippet, "Die Möwen kehren im Frühling zurück.", "Ausschnitt aus text_column");
    assert!((hits[1].meta["distance"].as_f64().unwrap() - 0.4).abs() < 1e-4, "Abstand in meta: {}", hits[1].meta);

    let both = way.search(&query("Möwen", &["leuchtturm", "senfkorn"])).await.unwrap();
    assert_eq!(
        both.iter().map(|h| h.document.as_str()).collect::<Vec<_>>(),
        ["logbuch", "saat", "logbuch", "karte"],
        "mit senfkorn: der Treffer aus „saat“ (Kosinus ≈ 0,99) steht an zweiter Stelle"
    );
    let limited = way.search(&Query { limit: 1, ..query("Möwen", &["leuchtturm"]) }).await.unwrap();
    assert_eq!(limited.len(), 1, "höchstens limit Treffer");

    // SQL special characters go in only as parameters.
    let evil = way
        .search(&query("'; DROP TABLE chunks; --", &["leuchtturm", "x'); DROP TABLE chunks; --"]))
        .await
        .expect("Sonderzeichen sind gewöhnliche Daten");
    assert_eq!(evil.len(), 3, "die erfundene Sammlung mit SQL im Namen gibt es nicht; leuchtturm trifft weiter");
    assert_eq!(count_chunks(&pg).await, 5, "Tabelle unversehrt: die Anfrage wurde nur gebunden, nie eingesetzt");

    // ---- Model and dimension --------------------------------------------------------------------
    let calls_before = embedders.calls();
    let foreign = way.search(&query("Möwen", &["fremd"])).await;
    assert!(
        matches!(&foreign, Err(RagError::ModelMismatch(m)) if m.contains("bge-m3")),
        "Daten mit anderem Modell: {foreign:?}"
    );
    assert_eq!(embedders.calls(), calls_before, "abgelehnt, bevor die Anfrage eingebettet oder verglichen wird");

    let other_model = Arc::new(FixedFactory {
        answer_model: Some("bge-m3".into()),
        ..FixedFactory::new(vec![], vec![1.0, 0.0, 0.0])
    });
    let way2 = pgvector::open_way(&server.state, other_model).await.unwrap();
    let mismatch = way2.search(&query("Möwen", &["leuchtturm"])).await;
    assert!(
        matches!(&mismatch, Err(RagError::ModelMismatch(_))),
        "Einbetter antwortet mit anderem Modell: {mismatch:?}"
    );

    let four = Arc::new(FixedFactory::new(vec![], vec![1.0, 0.0, 0.0, 0.0]));
    let way3 = pgvector::open_way(&server.state, four).await.unwrap();
    let dims = way3.search(&query("Möwen", &["leuchtturm"])).await;
    assert!(
        matches!(&dims, Err(RagError::ModelMismatch(m)) if m.contains("4 Dimensionen")),
        "vier statt drei Dimensionen: ModelMismatch – liefe die Abfrage, meldete PostgreSQL „different vector dimensions“ (Failed): {dims:?}"
    );

    let none = way.search(&query("Möwen", &[])).await;
    assert!(matches!(none, Err(RagError::NotConfigured(_))), "ohne Sammlung wird pgvector nicht gefragt: {none:?}");

    // Wrong mapping in the search: honest schema error.
    configure(&server, mapped(&pg, json!({ "document_column": "dokument" }))).await;
    let broken =
        pgvector::open_way(&server.state, factory()).await.unwrap().search(&query("Möwen", &["leuchtturm"])).await;
    assert!(
        matches!(&broken, Err(RagError::Schema(m)) if m.contains("„dokument“")),
        "fehlende Spalte bei der Suche: {broken:?}"
    );

    // Wrong password: Denied, without naming the password.
    configure(&server, mapped(&pg, json!({}))).await;
    server.state.secrets.set("pgvector", "falsch-und-geheim").unwrap();
    let denied =
        pgvector::open_way(&server.state, factory()).await.unwrap().search(&query("Möwen", &["leuchtturm"])).await;
    match denied {
        Err(RagError::Denied(m)) => {
            assert!(m.contains("Anmeldung abgelehnt") && !m.contains("falsch-und-geheim"), "Meldung ohne Passwort: {m}")
        }
        other => panic!("falsches Passwort → Denied, nicht {other:?}"),
    }
}

#[tokio::test]
async fn without_host_or_with_a_secret_for_another_target_the_way_stays_off() {
    let server = open_with(MemorySecrets::default()).await;
    let off = pgvector::open_way(&server.state, factory()).await;
    assert!(matches!(&off, Err(RagError::NotConfigured(m)) if m.contains("Kein Rechner")), "ohne Rechner aus");

    configure(&server, json!({ "host": "127.0.0.1", "port": 5999, "tls": "disable" })).await;
    assert!(pgvector::open_way(&server.state, factory()).await.is_ok(), "mit Rechner und passendem Passwort offen");
    ok(&server, "connection.set", json!({ "kind": "pgvector", "config": { "host": "db.anderswo.example" } })).await;
    let withheld = pgvector::open_way(&server.state, factory()).await;
    assert!(
        matches!(&withheld, Err(RagError::NotConfigured(m)) if m.contains("anderes Ziel")),
        "das Passwort gilt dem alten Rechner: nicht senden, Weg aus"
    );
}

#[tokio::test]
async fn a_closed_port_is_unreachable_not_a_panic() {
    let server = open_with(MemorySecrets::default()).await;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    configure(&server, json!({ "host": "127.0.0.1", "port": port, "tls": "disable" })).await;
    let way = pgvector::open_way(&server.state, factory()).await.unwrap();
    let result = way.search(&query("Möwen", &["leuchtturm"])).await;
    assert!(
        matches!(&result, Err(RagError::Unreachable(m)) if m.contains("Keine Verbindung")),
        "geschlossener Port: {result:?}"
    );
}
