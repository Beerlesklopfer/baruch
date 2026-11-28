// SPDX-License-Identifier: AGPL-3.0-or-later
//! Round 13 („websuche“): web search for `@belege` through MultiGPT (`web_search`, `fetch_url`) with a check in code.
//!
//! - Switch (default off): off → not a single request to the mock; only `@belege`, never for the proposal.
//! - Preferred sources first („site:<Rechner>“, at most three), then the general web; labelling by the host of the
//!   address.
//! - Only fetched pages in the block (at most four), too large and too slow ones skipped with a hint; the whole web
//!   search ends after 30 s, the job goes on with „Websuche ausgefallen“.
//! - Prompt injection stays in the block, the rule is in the system prompt.
//! - Check of the quotations in code, source line from the code, evidence in `web_evidence`, evidence page and adoption
//!   into the library without JavaScript (only for „belegt“), afterwards the library reports to the index.
//! - No MCP tool.
//!
//! Mocks: `websearch::mock::{MockWebMcp, MockSite}` (127.0.0.1:0, pages under `*.example`, never the internet), model
//! `assist::mock::ScriptedModel`, embedder a fixed mock. Test texts freely invented.

use crate::common;

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode, header};
use baruch_rag::{BoxFuture, Embedder, EmbedderFactory, Embeddings, RagError};
use baruch_server::assist::mock::{ScriptedModel, Step};
use baruch_server::assist::web_facts::{BLOCK_TITLE, SYSTEM_RULE};
use baruch_server::assist::{ModelCall, Role};
use baruch_server::websearch::mock::{MockSite, MockWebMcp};
use serde_json::{Value, json};

const LIGHTHOUSE: &str = "#set document(title: \"Das Jahr am Leuchtturm\", author: \"Mara Lind\")\n\n\
= Frühling\n\nDer Wärter putzt die Linse.\n\n\
== Feuer\n\nDas Feuer dreht sich, und das Licht wandert über das Wasser.\n\n\
= Sommer\n\nGäste steigen die Treppe hinauf.\n";

const LIGHT: &str = "https://lichtkunde.example/feuer";
const ARCHIVE: &str = "https://archiv.example/chronik";
const GUESTS: &str = "https://gaeste.example/buch";
const BIG: &str = "https://gross.example/riesig";
const SLOW: &str = "https://langsam.example/warten";
const QUESTION: &str = "Wie oft dreht sich das Feuer?";

struct House {
    server: common::Server,
    model: ScriptedModel,
    work: i64,
    key: i64,
    mcp: MockWebMcp,
    _site: MockSite,
}

/// Work, mock model with `steps`, test pages and MCP mock entered as MultiGPT; switch off.
async fn house(steps: impl IntoIterator<Item = Step>) -> House {
    let model = ScriptedModel::with(steps);
    let data = tempfile::tempdir().unwrap();
    let state = baruch_server::AppState::open_with(
        data.path(),
        Arc::new(baruch_server::connections::MemorySecrets::default()),
        Arc::new(model.clone()),
    )
    .await
    .expect("Datenordner öffnen");
    let state = Arc::new(state);
    let server = common::Server { app: baruch_server::router(state.clone()), state, data };
    let work = server.import(LIGHTHOUSE, "leuchtturm.typ").await.work;
    let key = server.state.store.load_work(work).await.unwrap().nodes[1].key;
    let site = MockSite::with_fixtures().await;
    let mcp = MockWebMcp::start(Some(&site)).await;
    ok(&server, "connection.set", json!({ "kind": "multigpt", "config": { "url": mcp.url() } })).await;
    House { server, model, work, key, mcp, _site: site }
}

fn result(answer: &Value) -> &Value {
    assert!(answer.get("error").is_none(), "unerwarteter Fehler: {answer}");
    &answer["result"]
}

async fn ok(server: &common::Server, method: &str, params: Value) -> Value {
    result(&server.rpc(method, params).await).clone()
}

fn error_code(answer: &Value) -> i64 {
    answer["error"]["code"].as_i64().unwrap_or_else(|| panic!("Fehler erwartet: {answer}"))
}

fn system(call: &ModelCall) -> &str {
    call.messages.iter().find(|m| m.role == Role::System).map(|m| m.content.as_str()).unwrap()
}

fn user(call: &ModelCall) -> &str {
    call.messages.iter().find(|m| m.role == Role::User).map(|m| m.content.as_str()).unwrap()
}

fn hit(title: &str, url: &str) -> Value {
    json!({ "title": title, "url": url, "content": format!("Ausschnitt zu {title}") })
}

async fn add_source(h: &House, title: &str, url: &str) {
    ok(&h.server, "sources.create", json!({ "title": title, "url": url })).await;
}

async fn switch_on(h: &House) {
    assert_eq!(ok(&h.server, "websearch.set", json!({ "on": true })).await, json!({ "on": true }), "Schalter an");
}

async fn ask(h: &House, persona: &str) -> Value {
    ok(&h.server, "persona.ask", json!({ "work": h.work, "key": h.key, "persona": persona, "question": QUESTION }))
        .await
}

/// As minijinja escapes (also `/`).
fn esc(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
        .replace('/', "&#x2f;")
}

fn today() -> String {
    baruch_server::websearch::now_stamp().1
}

/// Queries of `web_search` in order.
fn queries(mcp: &MockWebMcp) -> Vec<String> {
    mcp.tool_calls("web_search").iter().map(|a| a["query"].as_str().unwrap_or("").to_owned()).collect()
}

/// The block „Abgerufene Webseiten“ in the user message (up to the section).
fn web_block(message: &str) -> &str {
    let start = message.find(BLOCK_TITLE).unwrap_or_else(|| panic!("kein Block der Webseiten: {message}"));
    let rest = &message[start..];
    let end = rest.find("\n\nAbschnitt ").unwrap_or(rest.len());
    &rest[..end]
}

// ---- Switch and personas -----------------------------------------------------------------------------

#[tokio::test]
async fn with_the_switch_off_belege_sends_not_a_single_request_and_other_personas_never_search() {
    let h = house([
        Step::words("Antwort ohne Web."),
        Step::words("Kritisch: Lektor antwortet.\n- Schritt: prüfen."),
        Step::words("= Feuer\n\nNeu."),
    ])
    .await;
    h.mcp.set_results(vec![hit("Lichtkunde", LIGHT)]);
    assert_eq!(
        ok(&h.server, "websearch.get", json!({})).await,
        json!({ "on": false, "configured": true, "preferred": 0 }),
        "Vorgabe aus (Vertrag §4), MultiGPT eingerichtet, keine bevorzugten Quellen"
    );
    let answer = ask(&h, "@belege").await;
    assert!(h.mcp.requests().is_empty(), "Schalter aus: keine einzige Anfrage an MultiGPT: {:?}", h.mcp.requests());
    let calls = h.model.calls();
    assert!(!user(&calls[0]).contains(BLOCK_TITLE), "kein Block der Webseiten: {}", user(&calls[0]));
    assert!(!system(&calls[0]).contains(SYSTEM_RULE), "keine Web-Regel ohne Websuche");
    let body = answer["post"]["body"].as_str().unwrap();
    assert_eq!(body, "Antwort ohne Web.", "ohne Websuche keine Quellenzeile und kein Hinweis: {body}");

    switch_on(&h).await;
    ask(&h, "@lektor").await;
    // Reason: contract §4 – the web search runs only for @belege; never for the lector (or any other persona).
    assert!(h.mcp.requests().is_empty(), "@lektor sucht nie im Web, auch mit Schalter an: {:?}", h.mcp.requests());
    let proposed =
        ok(&h.server, "persona.propose", json!({ "work": h.work, "key": h.key, "persona": "@belege" })).await;
    assert_eq!(proposed["status"], "proposed", "Vorschlag angelegt: {proposed}");
    assert!(
        h.mcp.requests().is_empty(),
        "ein Vorschlag ist Buchtext und bekommt keine Websuche (keine Quellenzeile im Buch): {:?}",
        h.mcp.requests()
    );
}

#[tokio::test]
async fn an_unreachable_multigpt_gives_an_honest_note_and_the_order_still_runs() {
    let h = house([Step::words("Kurz.")]).await;
    // A free port on 127.0.0.1 on which nobody listens (reason: failed peer without network).
    let closed = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap();
    let url = format!("http://{closed}/mcp");
    ok(&h.server, "connection.set", json!({ "kind": "multigpt", "config": { "url": url } })).await;
    switch_on(&h).await;
    let answer = ask(&h, "@belege").await;
    let body = answer["post"]["body"].as_str().unwrap();
    assert!(body.starts_with("Kurz."), "der Auftrag lief weiter: {body}");
    let note = body.lines().find(|l| l.starts_with("Hinweis: Websuche ausgefallen (")).unwrap_or("");
    assert!(note.contains("initialize") && note.contains(&url), "ehrlicher Grund mit Adresse im Beitrag: {body}");
    assert!(body.contains("Angabe prüfen: nicht belegt"), "ohne Seite gibt es keinen Beleg: {body}");
    let message = user(&h.model.calls()[0]).to_owned();
    assert!(message.contains("Hinweis: Websuche ausgefallen ("), "der Hinweis steht auch im Prompt: {message}");
    assert!(web_block(&message).contains("- no page fetched"), "der Block sagt, dass nichts abgerufen wurde");
    assert!(h.mcp.requests().is_empty(), "der Nachbau ist nicht mehr eingetragen und bekommt nichts");
}

// ---- Preferred sources ---------------------------------------------------------------------------------

#[tokio::test]
async fn preferred_sources_are_searched_first_by_site_and_marked_by_host() {
    let h = house([Step::words("Laut [W1] dreht es sich.")]).await;
    add_source(&h, "Lichtkunde", "https://www.lichtkunde.example/").await;
    add_source(&h, "Archiv", "https://archiv.example/start").await;
    add_source(&h, "Lichtkunde zweite Seite", "https://lichtkunde.example/andere").await;
    add_source(&h, "Gästebuch", "https://gaeste.example/").await;
    add_source(&h, "Vierte", "https://vierte.example/").await;
    h.mcp.set_results(vec![hit("Frei", "https://frei.example/x"), hit("Licht", LIGHT), hit("Chronik", ARCHIVE)]);
    switch_on(&h).await;
    ask(&h, "@belege").await;
    assert_eq!(
        queries(&h.mcp),
        vec![
            format!("site:lichtkunde.example {QUESTION}"),
            format!("site:archiv.example {QUESTION}"),
            format!("site:gaeste.example {QUESTION}"),
            QUESTION.to_owned(),
        ],
        "erst höchstens drei bevorzugte Rechner in ihrer Reihenfolge (gleiche nur einmal, www. weg), dann das Web"
    );
    let fetched: Vec<String> =
        h.mcp.tool_calls("fetch_url").iter().map(|a| a["url"].as_str().unwrap().to_owned()).collect();
    assert_eq!(
        fetched,
        vec![LIGHT.to_owned(), ARCHIVE.to_owned(), "https://frei.example/x".to_owned()],
        "Treffer der bevorzugten Quellen zuerst, Doppelte aus der allgemeinen Suche nur einmal"
    );
    let message = user(&h.model.calls()[0]).to_owned();
    let block = web_block(&message);
    let day = today();
    assert!(
        block.contains(&format!(
            "[W1] Lichtkunde: Das kreisende Feuer · {LIGHT} · fetched {day} · preferred source\n<<<seite\n"
        )),
        "Kopfzeile mit Titel der Seite, Adresse, Abrufdatum und Kennzeichnung: {block}"
    );
    assert!(
        block.contains(&format!("[W2] Hafenchronik 1890 · {ARCHIVE} · fetched {day} · preferred source")),
        "zweite bevorzugte Quelle: {block}"
    );
    assert!(
        block.contains("Hinweis: https://frei.example/x übersprungen"),
        "die Seite ohne Antwort (404) ist ein Hinweis, keine Quelle: {block}"
    );
    assert!(!block.contains("[W3]"), "nur abgerufene Seiten bekommen eine Nummer: {block}");
}

// ---- Limits ----------------------------------------------------------------------------------------------

#[tokio::test]
async fn only_fetched_pages_reach_the_block_and_big_or_slow_pages_become_notes() {
    let h = house([Step::words("Siehe [W1].")]).await;
    h.mcp.set_results(vec![
        hit("Riesig", BIG),
        hit("Langsam", SLOW),
        hit("Licht", LIGHT),
        hit("Chronik", ARCHIVE),
        hit("Fünfter", "https://fuenfter.example/nie"),
    ]);
    switch_on(&h).await;
    let started = Instant::now();
    ask(&h, "@belege").await;
    assert!(
        started.elapsed() < Duration::from_secs(14),
        "die langsame Seite (15 s) bricht nach 10 s ab: {:?}",
        started.elapsed()
    );
    assert_eq!(
        h.mcp.tool_calls("fetch_url").len(),
        4,
        "höchstens vier Abrufe, der fünfte Treffer wird nicht abgerufen"
    );
    let message = user(&h.model.calls()[0]).to_owned();
    let block = web_block(&message);
    assert!(
        block.contains(&format!("[W1] Lichtkunde: Das kreisende Feuer · {LIGHT}")),
        "W1 ist die erste abgerufene: {block}"
    );
    assert!(block.contains(&format!("[W2] Hafenchronik 1890 · {ARCHIVE}")), "W2: {block}");
    assert!(
        block.contains(&format!("Hinweis: {BIG} übersprungen (Seite ist größer als 1 MiB")),
        "über 1 MiB: übersprungen mit Hinweis: {block}"
    );
    let slow = block.lines().find(|l| l.starts_with(&format!("Hinweis: {SLOW} übersprungen ("))).unwrap_or("");
    assert!(slow.contains("innerhalb von 10 s"), "Zeitüberschreitung nach 10 s: übersprungen mit Hinweis: {block}");
    assert!(!block.contains("fuenfter.example"), "nicht abgerufene Treffer stehen nicht im Block: {block}");
    assert!(!block.contains("Sand und Salz"), "vom übersprungenen Inhalt nichts im Block");
    let page = block.split("<<<seite\n").nth(1).unwrap().split("\n>>>").next().unwrap();
    assert!(page.chars().count() <= 1500, "höchstens 1 500 Zeichen je Seite im Block: {}", page.chars().count());
    assert!(!page.contains("nicht lesen") && !page.contains("font:serif"), "Skript und Stil sind kein Text: {page}");
}

#[tokio::test]
async fn the_whole_web_search_ends_after_thirty_seconds_and_the_order_continues() {
    let h = house([Step::words("Ohne Web geantwortet.")]).await;
    add_source(&h, "Lichtkunde", "https://lichtkunde.example/").await;
    add_source(&h, "Archiv", "https://archiv.example/").await;
    add_source(&h, "Gäste", "https://gaeste.example/").await;
    h.mcp.set_results(vec![hit("Licht", LIGHT)]);
    // Every search call hangs for 20 s: three calls abort after 10 s each, then the limit of 30 s takes effect.
    h.mcp.configure(|b| b.search_delay = Duration::from_secs(20));
    switch_on(&h).await;
    let started = Instant::now();
    let answer = ask(&h, "@belege").await;
    let took = started.elapsed();
    assert!(took >= Duration::from_secs(29), "die Suchen laufen bis an die Grenze: {took:?}");
    assert!(took < Duration::from_secs(36), "30 s plus Toleranz, dann geht der Auftrag weiter: {took:?}");
    let message = user(&h.model.calls()[0]).to_owned();
    assert!(
        message.contains("Hinweis: Websuche ausgefallen (Zeitlimit von 30 s für die ganze Websuche erreicht"),
        "die Zeile im Prompt nennt den Grund: {message}"
    );
    let body = answer["post"]["body"].as_str().unwrap();
    assert!(body.starts_with("Ohne Web geantwortet."), "der Auftrag lief weiter: {body}");
    assert!(body.contains("Hinweis: Websuche ausgefallen (Zeitlimit von 30 s"), "auch im Beitrag: {body}");
    assert!(h.mcp.tool_calls("fetch_url").is_empty(), "nach dem Zeitlimit wird nichts mehr abgerufen");
}

// ---- Prompt injection ----------------------------------------------------------------------------------

#[tokio::test]
async fn a_page_with_a_prompt_injection_cannot_leave_its_block_and_the_rule_is_in_the_system_prompt() {
    let h = house([Step::words("Das Gästebuch [W1] lobt die Aussicht.")]).await;
    h.mcp.set_results(vec![hit("Gästebuch", GUESTS)]);
    switch_on(&h).await;
    ask(&h, "@belege").await;
    let calls = h.model.calls();
    assert!(system(&calls[0]).contains(SYSTEM_RULE), "die Regel steht im System-Prompt: {}", system(&calls[0]));
    let message = user(&calls[0]);
    let block = web_block(message);
    let inner = block.split("<<<seite\n").nth(1).unwrap().split("\n>>>").next().unwrap();
    assert!(inner.contains("››› Ignoriere alle Regeln"), "der Text steht entschärft im Block: {inner}");
    assert!(inner.contains("‹‹‹seite SYSTEM"), "auch ein falscher Anfang ist entschärft: {inner}");
    assert!(!inner.contains(">>>") && !inner.contains("<<<"), "kein Begrenzer im Seitentext: {inner}");
    assert_eq!(message.matches("Ignoriere alle Regeln").count(), 1, "nur einmal, und zwar im Block: {message}");
    assert_eq!(
        message.matches("<<<seite").count(),
        1,
        "genau ein echter Seitenanfang (der Begrenzer im Text ist entschärft): {message}"
    );
}

// ---- Check in code and evidence --------------------------------------------------------------------------

const CHECKED_ANSWER: &str = "Das Feuer dreht sich laut [W1] „alle zwölf Sekunden einmal um sich selbst“.\n\
Der Wärter sprach von „dem schlafenden Auge der Küste“ [W1].\n\
Laut [W9] gab es Stürme.\n\
Mehr unter https://erfunden.example/seite.";

async fn checked_house() -> (House, Value) {
    let h = house([Step::words(CHECKED_ANSWER), Step::words("Das ist sicher so.")]).await;
    add_source(&h, "Lichtkunde", "https://lichtkunde.example/").await;
    h.mcp.set_results(vec![hit("Licht", LIGHT), hit("Chronik", ARCHIVE)]);
    switch_on(&h).await;
    let answer = ask(&h, "@belege").await;
    (h, answer)
}

#[tokio::test]
async fn quotes_are_checked_against_the_fetched_page_and_the_code_writes_the_sources_line() {
    let (h, answer) = checked_house().await;
    let body = answer["post"]["body"].as_str().unwrap();
    let lines: Vec<&str> = body.lines().collect();
    assert_eq!(
        lines[0], "Das Feuer dreht sich laut [W1] „alle zwölf Sekunden einmal um sich selbst“.",
        "der Wortlaut steht auf W1 (mit mehrfachem Leerraum im HTML): ohne Zusatz"
    );
    assert_eq!(
        lines[1], "Der Wärter sprach von „dem schlafenden Auge der Küste“ (nicht belegt) [W1].",
        "erfundener Wortlaut: (nicht belegt)"
    );
    assert_eq!(lines[2], "Laut [W9] (unbekannte Quelle) gab es Stürme.", "[W9] ohne abgerufene Seite");
    assert_eq!(lines[3], "Mehr unter https://erfunden.example/seite (nicht abgerufen).", "nicht abgerufene Adresse");
    assert_eq!(
        lines.last().copied().unwrap(),
        format!("Quellen: [W1] Lichtkunde: Das kreisende Feuer – {LIGHT} (abgerufen am {})", today()),
        "die Quellenzeile setzt der Code, nur genannte Seiten: {body}"
    );
    assert!(!body.contains("Angabe prüfen"), "es gibt einen Beleg: {body}");
    let post = answer["post"]["id"].as_i64().unwrap();
    let evidence = ok(&h.server, "websearch.evidence", json!({ "post": post })).await;
    let rows = evidence["evidence"].as_array().unwrap();
    assert_eq!(
        rows.iter().map(|r| (r["label"].as_str().unwrap(), r["verdict"].as_str().unwrap())).collect::<Vec<_>>(),
        vec![("W1", "belegt"), ("W1", "nicht_belegt")],
        "je Zitat eine Zeile in web_evidence mit dem Urteil des Codes: {evidence}"
    );
    assert_eq!(rows[0]["quote"], "alle zwölf Sekunden einmal um sich selbst", "Zitat gespeichert: {evidence}");
    assert_eq!(
        (rows[0]["url"].as_str(), rows[0]["preferred"].as_bool(), rows[0]["library_title"].is_null()),
        (Some(LIGHT), Some(true), true),
        "Adresse, bevorzugte Quelle, noch nicht übernommen: {evidence}"
    );
    assert!(
        rows[0]["accessed"].as_str().is_some_and(|a| a.ends_with('Z') && a.len() == 20),
        "Abrufzeit als UTC-Zeitpunkt: {evidence}"
    );
    let stored = h.server.state.store.post_get(h.work, post).await.unwrap();
    assert_eq!(stored.sources, vec!["Web: 2 Belege".to_owned()], "der Beitrag trägt die Quelle „Web:“");

    // Second job without a named page: check the statement.
    let again = ask(&h, "@belege").await;
    let body = again["post"]["body"].as_str().unwrap();
    assert!(
        body.ends_with("Das ist sicher so.\n\nAngabe prüfen: nicht belegt\nQuellen: keine abgerufene Seite genannt"),
        "ohne Beleg hängt der Code den Hinweis und die (leere) Quellenzeile an: {body}"
    );
    let post = again["post"]["id"].as_i64().unwrap();
    let none = ok(&h.server, "websearch.evidence", json!({ "post": post })).await;
    assert_eq!(none["evidence"], json!([]), "keine genannte Seite, kein Beleg: {none}");
    assert!(
        h.server.state.store.post_get(h.work, post).await.unwrap().sources.is_empty(),
        "ohne Belege keine Quelle Web:"
    );
}

// ---- Evidence page and library -----------------------------------------------------------------------------

const DIMENSION: usize = 4;

/// Embedder with a fixed vector space; remembers every embedded text.
#[derive(Clone, Default)]
struct Seen(Arc<Mutex<Vec<String>>>);

struct SeenEmbedder {
    model: String,
    seen: Seen,
}

impl Embedder for SeenEmbedder {
    fn model(&self) -> &str {
        &self.model
    }

    fn embed<'a>(&'a self, texts: &'a [String]) -> BoxFuture<'a, Result<Embeddings, RagError>> {
        Box::pin(async move {
            self.seen.0.lock().unwrap().extend(texts.iter().cloned());
            let vector = |t: &String| {
                let n = (t.len() % 7) as f32 + 1.0;
                let v = [1.0, n, 0.5, 0.25];
                let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
                v.iter().map(|x| x / norm).collect::<Vec<f32>>()
            };
            Ok(Embeddings {
                model: self.model.clone(),
                dimension: DIMENSION,
                vectors: texts.iter().map(vector).collect(),
            })
        })
    }
}

impl EmbedderFactory for Seen {
    fn for_model(&self, model: &str) -> Arc<dyn Embedder> {
        Arc::new(SeenEmbedder { model: model.to_owned(), seen: self.clone() })
    }
}

async fn library_count(h: &House) -> (i64, i64, i64) {
    let pool = h.server.state.store.pool();
    let count = |table: &'static str| async move {
        sqlx::query_scalar::<_, i64>(sqlx::AssertSqlSafe(format!("SELECT count(*) FROM {table}")))
            .fetch_one(pool)
            .await
            .unwrap()
    };
    (count("library_title").await, count("library_quote").await, count("library_highlight").await)
}

#[tokio::test]
async fn the_evidence_page_adopts_only_proven_quotes_into_the_library_without_javascript() {
    let (h, answer) = checked_house().await;
    let post = answer["post"]["id"].as_i64().unwrap();
    let rows = ok(&h.server, "websearch.evidence", json!({ "post": post })).await["evidence"].clone();
    let (proven, unproven) = (rows[0]["id"].as_i64().unwrap(), rows[1]["id"].as_i64().unwrap());
    let seen = Seen::default();
    baruch_server::rag::set_embedders(&h.server.state, Arc::new(seen.clone()));
    baruch_server::rag::catch_up(&h.server.state).await;
    baruch_server::rag::wait_idle(&h.server.state).await;

    // The work page links the post to its evidence page.
    let (status, workspace) = h.server.get(&format!("/?work={}&key={}", h.work, h.key)).await;
    assert_eq!(status, StatusCode::OK, "Arbeitsseite: {workspace}");
    let link = format!("/websuche/belege?work={}&amp;key={}&amp;post={post}", h.work, h.key);
    assert!(workspace.contains(&link), "Beitrag mit Quelle „Web:“ verlinkt die Belegseite ({link}): {workspace}");

    let page_uri = format!("/websuche/belege?work={}&key={}&post={post}", h.work, h.key);
    let (status, page) = h.server.get(&page_uri).await;
    assert_eq!(status, StatusCode::OK, "Belegseite: {page}");
    // Round 33: the header is the ribbon (header.band instead of header.topbar)
    assert_eq!(page.matches("<header class=\"band\"").count(), 1, "Seite mit der gemeinsamen Kopfzeile: {page}");
    assert!(
        page.contains(&format!("<a href=\"{}\" rel=\"noopener noreferrer\" target=\"_blank\">", esc(LIGHT))),
        "Link auf die Seite, maskiert, ohne Zugriff auf Baruch: {page}"
    );
    assert!(
        page.contains("bevorzugte Quelle") && page.contains(&format!("abgerufen am {}", today())),
        "Angaben: {page}"
    );
    assert!(page.contains("„alle zwölf Sekunden einmal um sich selbst“"), "Zitat sichtbar: {page}");
    assert_eq!(page.matches("value=\"adopt\"").count(), 1, "nur der belegte Beleg hat den Knopf: {page}");
    assert!(page.contains(&format!("name=\"evidence\" value=\"{proven}\"")), "Knopf am belegten Beleg: {page}");

    // Not evidenced: through RPC -32602, through the form a message; nothing created.
    let before = library_count(&h).await;
    let refused = h.server.rpc("websearch.adopt", json!({ "evidence": unproven })).await;
    assert_eq!(error_code(&refused), -32602, "nur belegte Zitate: {refused}");
    let (status, location) =
        h.server.post_form("/websuche/act", &format!("action=adopt&evidence={unproven}&post={post}")).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Formular leitet zurück");
    let location = location.unwrap();
    assert!(location.contains("fehler=nicht_belegt") && location.contains(&format!("post={post}")), "{location}");
    let (_, shown) = h.server.get(&location).await;
    assert!(shown.contains("Nur belegte Zitate kommen in die Bibliothek"), "Meldung auf der Seite: {shown}");
    assert_eq!(library_count(&h).await, before, "nicht belegt: kein Titel, kein Zitat, keine Hervorhebung");

    // Evidenced: title (web, URL, note with fetch date), quotation, highlight by the human; index caught up.
    seen.0.lock().unwrap().clear();
    let (status, location) =
        h.server.post_form("/websuche/act", &format!("action=adopt&evidence={proven}&work={}", h.work)).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Übernahme leitet zurück");
    let location = location.unwrap();
    assert!(location.contains("ok=uebernommen") && location.ends_with(&format!("#beleg-{proven}")), "{location}");
    let after = library_count(&h).await;
    assert_eq!(
        (after.0 - before.0, after.1 - before.1, after.2 - before.2),
        (1, 1, 1),
        "je ein Titel, Zitat, Hervorhebung"
    );
    let row = ok(&h.server, "websearch.evidence", json!({ "post": post })).await["evidence"][0].clone();
    let title_id = row["library_title"].as_i64().expect("library_title_id gesetzt");
    let entry = baruch_library::get_title(h.server.state.store.pool(), title_id).await.unwrap();
    assert_eq!(
        (entry.title.kind.as_str(), entry.title.url.as_str(), entry.title.title.as_str()),
        ("web", LIGHT, "Lichtkunde: Das kreisende Feuer"),
        "Titel der Art web mit Adresse und Seitentitel"
    );
    assert_eq!(
        entry.title.note,
        format!("Abgerufen am {} (Websuche @belege)", today()),
        "Notiz mit Abrufdatum (Vertrag §4)"
    );
    assert_eq!(entry.quotes[0].text, "alle zwölf Sekunden einmal um sich selbst", "das Zitat als library_quote");
    let marks = baruch_library::list_highlights(h.server.state.store.pool(), Some(title_id)).await.unwrap();
    assert_eq!(
        (marks.len(), marks[0].kind, marks[0].status, marks[0].quote_id),
        (1, baruch_library::HighlightKind::Quote, baruch_library::HighlightStatus::Active, Some(entry.quotes[0].id)),
        "eine Hervorhebung der Art Zitat, aktiv (Mensch), am Zitat"
    );
    assert!(
        matches!(&marks[0].author, baruch_library::HighlightAuthor::Human { .. }),
        "Urheber ist der Mensch: {:?}",
        marks[0].author
    );
    baruch_server::rag::wait_idle(&h.server.state).await;
    assert!(
        seen.0.lock().unwrap().iter().any(|t| t.contains("alle zwölf Sekunden")),
        "nach der Übernahme reiht sich die Bibliothek beim gemeinsamen Index ein (Nachbau-Einbetter sah das Zitat)"
    );
    let (_, page) = h.server.get(&page_uri).await;
    assert!(
        page.contains(&esc(&format!("/quelle?work={}&title={title_id}", h.work)))
            && page.matches("value=\"adopt\"").count() == 0,
        "danach Link auf den Titel statt Knopf: {page}"
    );
    let twice = h.server.rpc("websearch.adopt", json!({ "evidence": proven })).await;
    assert_eq!(error_code(&twice), -32602, "ein zweites Mal wird nichts angelegt: {twice}");
    assert_eq!(library_count(&h).await, after, "unverändert nach dem zweiten Versuch");
}

// ---- Switch: RPC, form, page -----------------------------------------------------------------------------

#[tokio::test]
async fn the_switch_works_over_rpc_and_the_form_and_sits_in_the_tab_of_belege() {
    let h = house([]).await;
    let bad = h.server.rpc("websearch.set", json!({ "on": "ja" })).await;
    assert_eq!(error_code(&bad), -32602, "on muss ein Wahrheitswert sein: {bad}");
    switch_on(&h).await;
    let values = ok(&h.server, "settings.get", json!({ "page": "assistenten" })).await;
    assert_eq!(values["websearch"], json!({ "on": true }), "settings.get nennt den Schalter: {values}");
    let (status, html) = h.server.get(&format!("/einstellungen/assistenten?persona=belege&work={}", h.work)).await;
    assert_eq!(status, StatusCode::OK, "Seite Assistenten: {html}");
    assert_eq!(html.matches("class=\"persona-websearch\"").count(), 1, "nur im Tab von @belege: {html}");
    let panel = html.find("id=\"persona-belege\"").expect("Tab von @belege");
    let form = html.find("class=\"persona-websearch\"").expect("Formular der Websuche");
    assert!(form > panel, "das Formular steht in der Tafel von @belege");
    let tab = &html[form..];
    let tab = &tab[..tab.find("</form>").unwrap()];
    assert!(
        tab.contains("action=\"/websuche/act\"") && tab.contains("name=\"on\" value=\"1\" checked"),
        "Formular ohne JavaScript, Kästchen an: {tab}"
    );
    let (status, location) = h.server.post_form("/websuche/act", &format!("action=switch&work={}", h.work)).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "303 zurück");
    assert_eq!(
        location.as_deref(),
        Some(format!("/einstellungen/assistenten?persona=belege&ok=websuche&work={}#persona-belege", h.work).as_str()),
        "zurück in den Tab von @belege mit Meldung"
    );
    assert_eq!(ok(&h.server, "websearch.get", json!({})).await["on"], false, "ohne on: aus");
    let (_, html) = h.server.get(&location.unwrap()).await;
    assert!(html.contains("Die Einstellung zur Websuche von @belege ist gespeichert."), "Meldung: {html}");
    h.server.post_form("/websuche/act", "action=switch&on=1").await;
    assert_eq!(ok(&h.server, "websearch.get", json!({})).await["on"], true, "on=1: an");
    let foreign = Request::post("/websuche/act")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .header(header::ORIGIN, "http://fremd.example")
        .body(Body::from("action=switch"))
        .unwrap();
    let (status, _, _) = h.server.send(foreign).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "fremder Origin: abgewiesen (guard::same_origin)");
    assert_eq!(ok(&h.server, "websearch.get", json!({})).await["on"], true, "fremder Origin ändert nichts");
}

#[tokio::test]
async fn mcp_offers_no_tool_for_the_web_search() {
    let h = house([]).await;
    let mut request = Request::post("/mcp")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" }).to_string()))
        .unwrap();
    request.extensions_mut().insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 40000))));
    let (status, _, body) = h.server.send(request).await;
    assert_eq!(status, StatusCode::OK, "tools/list: {body}");
    let list: Value = serde_json::from_str(&body).unwrap();
    let names: Vec<&str> =
        list["result"]["tools"].as_array().unwrap().iter().filter_map(|t| t["name"].as_str()).collect();
    assert!(!names.is_empty(), "es gibt Werkzeuge: {list}");
    // Reason (docs/Architektur.md): the web search has its value in the check within @belege; an MCP passage into the
    // internet would enlarge the attack surface, and MCP clients have web searches of their own.
    for name in &names {
        assert!(
            !name.contains("web") && !name.contains("fetch") && !name.contains("evidence"),
            "kein MCP-Werkzeug für Websuche, Abruf oder Belege: {name}"
        );
    }
}

// ---- No passage through MCP -----------------------------------------------------------------------------

/// An MCP call from loopback (like a local client without token).
async fn mcp_call(server: &common::Server, name: &str, arguments: Value) -> Value {
    let message = json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/call",
        "params": { "name": name, "arguments": arguments } });
    let mut request = Request::post("/mcp")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(message.to_string()))
        .unwrap();
    request.extensions_mut().insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 40000))));
    let (_, _, body) = server.send(request).await;
    serde_json::from_str(&body).unwrap_or(Value::Null)
}

#[tokio::test]
async fn mcp_ask_persona_and_chat_with_belege_never_search_the_web_even_with_the_switch_on() {
    let h = house([Step::words("Antwort über MCP."), Step::words("Chat über MCP.")]).await;
    h.mcp.set_results(vec![hit("Lichtkunde", LIGHT)]);
    switch_on(&h).await;
    let asked = mcp_call(
        &h.server,
        "ask_persona",
        json!({ "work": h.work, "key": h.key, "persona": "@belege", "question": "Geheime Suchanfrage ins Netz" }),
    )
    .await;
    assert_eq!(asked["result"]["isError"], false, "der Auftrag selbst läuft: {asked}");
    let chatted = mcp_call(
        &h.server,
        "chat",
        json!({ "work": h.work, "key": h.key, "message": "@belege bitte suche das im Netz", "persona": "@belege" }),
    )
    .await;
    assert_eq!(chatted["result"]["isError"], false, "der Chat läuft: {chatted}");
    // Reason: docs/Bedienung.md (web search for @belege, under „Assistenten, Chat und Befunde“) – no passage into
    // the internet through MCP; otherwise the freely chosen question of an (even remote) MCP client would go to
    // MultiGPT/SearXNG as a search query.
    assert!(h.mcp.requests().is_empty(), "keine einzige Anfrage an MultiGPT über MCP: {:?}", h.mcp.requests());
    let calls = h.model.calls();
    assert_eq!(calls.len(), 2, "zwei Modellaufrufe (ask und chat)");
    for call in &calls {
        assert!(!user(call).contains(BLOCK_TITLE), "kein Block der Webseiten: {}", user(call));
        assert!(!system(call).contains(SYSTEM_RULE), "keine Web-Regel über MCP");
    }
    // Counter-check: the same question from the user interface searches (the switch really is on).
    let h2 = house([Step::words("Antwort aus der Oberfläche.")]).await;
    h2.mcp.set_results(vec![hit("Lichtkunde", LIGHT)]);
    switch_on(&h2).await;
    ask(&h2, "@belege").await;
    assert!(!queries(&h2.mcp).is_empty(), "aus der Oberfläche läuft die Websuche: {:?}", h2.mcp.requests());
}
