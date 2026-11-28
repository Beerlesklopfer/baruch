// SPDX-License-Identifier: AGPL-3.0-or-later
//! Round 19c (treffer): weak hits in literature, ideas and evidence, `idea.search` with `mode: "rag"` (failure -32001),
//! wording with hyphenation leftovers in the module (`module_spacing`, `module_note`), „Mal“, lector hint with Bible,
//! reverse mapping of the Bible page, lemma probe of the lexicons and the Bible context of the work page without
//! JavaScript.
//!
//! No network: embeddings only through an own mock vector space ([`Concepts`]), Bibles and lexicons are written by the
//! test itself (`baruch_sword::fixture`, freely invented, temp folder); foreign modules are never read.

use crate::common;

use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode, header};
use baruch_rag::{BoxFuture, Embedder, EmbedderFactory, Embeddings, RagError};
use baruch_scripture::versification::{german, versification};
use baruch_server::bibles::{Choice, SPACING_LABEL};
use baruch_server::rag;
use baruch_server::scripture::{MARK_BIBLE, MARK_OTHERS};
use baruch_sword::fixture::{Block, Fixture, Layout, LexLayout, LexiconFixture, hyphen_remnants, write, write_lexicon};
use serde_json::{Value, json};

// ---- Helpers --------------------------------------------------------------------------------------------

async fn ok(server: &common::Server, method: &str, params: Value) -> Value {
    let answer = server.rpc(method, params).await;
    assert!(answer["error"].is_null(), "{method} sollte gelingen: {answer}");
    answer["result"].clone()
}

/// MCP tool from the own machine (without token, session from `initialize`).
async fn mcp(server: &common::Server, tool: &str, arguments: Value) -> Value {
    async fn post(server: &common::Server, session: Option<&str>, message: Value) -> (axum::http::HeaderMap, Value) {
        let mut request = Request::post("/mcp").header(header::CONTENT_TYPE, "application/json");
        if let Some(id) = session {
            request = request.header("mcp-session-id", id);
        }
        let mut request = request.body(Body::from(message.to_string())).unwrap();
        request.extensions_mut().insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 40000))));
        let (status, headers, body) = server.send(request).await;
        assert_eq!(status, StatusCode::OK, "MCP antwortet: {body}");
        (headers, serde_json::from_str(&body).unwrap())
    }
    let (headers, _) = post(
        server,
        None,
        json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize",
                "params": { "protocolVersion": "2025-06-18", "capabilities": {},
                            "clientInfo": { "name": "treffer", "version": "1.0" } } }),
    )
    .await;
    let session = headers.get("mcp-session-id").unwrap().to_str().unwrap().to_owned();
    post(
        server,
        Some(&session),
        json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": { "name": tool, "arguments": arguments } }),
    )
    .await
    .1
}

fn url_escape(text: &str) -> String {
    text.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// HTML escaping like minijinja (also `/`).
fn esc(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
        .replace('/', "&#x2f;")
}

/// Part of `html` from `start` to the next `end` (panic with text if it is missing).
fn between<'a>(html: &'a str, start: &str, end: &str) -> &'a str {
    let from = html.find(start).unwrap_or_else(|| panic!("„{start}“ fehlt: {html}"));
    let rest = &html[from..];
    &rest[..rest.find(end).map_or(rest.len(), |i| i + end.len())]
}

// ---- Mock vector space (fixed, without network) -----------------------------------------------------------

/// Concept axes: words of the same axis are "equal in meaning", different axes are perpendicular.
const AXES: [&[&str]; 3] = [
    &["meer", "wellen", "wogen", "gischt", "küste", "flut", "ebbe", "kutter", "hafen", "segel"],
    &["garten", "beet", "samen", "wurzel", "erde", "pflanze", "blüte", "saat"],
    &["laterne", "flamme", "docht", "kerze", "lampe", "schein"],
];
const DIMENSION: usize = 4;

fn concept_vector(text: &str) -> Vec<f32> {
    let mut v = vec![0f32; DIMENSION];
    for word in text.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()) {
        let word = word.to_lowercase();
        match AXES.iter().position(|axis| axis.contains(&word.as_str())) {
            Some(axis) => v[axis] += 1.0,
            None => v[DIMENSION - 1] += 0.01,
        }
    }
    let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm == 0.0 {
        v[DIMENSION - 1] = 1.0;
    } else {
        v.iter_mut().for_each(|x| *x /= norm);
    }
    v
}

#[derive(Clone, Default)]
struct Concepts;

struct ConceptEmbedder(String);

impl Embedder for ConceptEmbedder {
    fn model(&self) -> &str {
        &self.0
    }

    fn embed<'a>(&'a self, texts: &'a [String]) -> BoxFuture<'a, Result<Embeddings, RagError>> {
        Box::pin(async move {
            Ok(Embeddings {
                model: self.0.clone(),
                dimension: DIMENSION,
                vectors: texts.iter().map(|t| concept_vector(t)).collect(),
            })
        })
    }
}

impl EmbedderFactory for Concepts {
    fn for_model(&self, model: &str) -> Arc<dyn Embedder> {
        Arc::new(ConceptEmbedder(model.to_owned()))
    }
}

/// Test book (freely invented) with a sea section and a garden section.
const SEA_BOOK: &str = "// Testbuch Runde 19c (frei erfunden).
= Am Meer

Die Wellen rollen an die Küste, und bei Flut kreuzt ein Kutter mit weißem Segel vor dem Hafen.

= Im Garten

Im Beet keimen die Samen, die Wurzel greift in die Erde, und bald trägt die Pflanze eine Blüte.
";

/// Server with mock embedder, test book and two ideas (sea and garden); index built.
async fn indexed() -> (common::Server, i64, i64, i64) {
    let server = common::open().await;
    rag::set_embedders(&server.state, Arc::new(Concepts));
    let work = server.import(SEA_BOOK, "meer19.typ").await.work;
    let sea =
        ok(&server, "idea.create", json!({ "work": work, "words": "Kutter", "note": "Ein Kutter im Hafen bei Ebbe." }))
            .await["idea"]["id"]
            .as_i64()
            .unwrap();
    let garden = ok(&server, "idea.create", json!({ "work": work, "words": "Beet", "note": "Samen für das Beet." }))
        .await["idea"]["id"]
        .as_i64()
        .unwrap();
    rag::catch_up(&server.state).await;
    rag::wait_idle(&server.state).await;
    (server, work, sea, garden)
}

// ---- Weak hits ------------------------------------------------------------------------------------------

#[tokio::test]
async fn idea_search_by_meaning_marks_weak_hits_and_puts_them_last() {
    let (server, work, sea, garden) = indexed().await;
    let found = ok(&server, "idea.search", json!({ "work": work, "query": "Wogen und Gischt", "mode": "rag" })).await;
    let hits = found["hits"].as_array().unwrap();
    let sea_hit = hits.iter().find(|h| h["id"] == sea).unwrap_or_else(|| panic!("Meeres-Einfall: {found}"));
    assert_eq!(sea_hit["weak"], false, "Begründung: gleiche Begriffsachse, Ähnlichkeit nahe 1, nicht schwach: {found}");
    let garden_hit = hits.iter().find(|h| h["id"] == garden).unwrap_or_else(|| panic!("Garten-Einfall: {found}"));
    assert_eq!(
        garden_hit["weak"], true,
        "Begründung: senkrechte Achse, Ähnlichkeit nahe 0 und damit unter WEAK_SCORE = schwach: {found}"
    );
    assert!(
        garden_hit["score"].as_f64().unwrap() < f64::from(baruch_rag::WEAK_SCORE),
        "Begründung: weak folgt allein aus der Schwelle von baruch-rag: {found}"
    );
    let order: Vec<i64> = hits.iter().map(|h| h["id"].as_i64().unwrap()).collect();
    assert_eq!(order, [sea, garden], "Begründung: schwache Treffer stehen nach den übrigen: {found}");

    let (status, html) =
        server.get(&format!("/einfaelle?work={work}&q={}&mode=rag", url_escape("Wogen und Gischt"))).await;
    assert_eq!(status, StatusCode::OK, "Einfälle im Modus Bedeutung");
    let folded = between(&html, "<details class=\"weak-hits\" data-weak-hits>", "</details>");
    assert!(
        folded.contains("1 schwacher Treffer") && folded.contains(&format!("id=\"einfall-{garden}\"")),
        "Begründung: der schwache Einfall steht zugeklappt in <details> (ohne JavaScript aufklappbar): {folded}"
    );
    assert!(folded.contains("data-weak-label>schwach</span>"), "Begründung: Text „schwach“ neben dem Prozentwert");
    let sea_at = html.find(&format!("id=\"einfall-{sea}\"")).expect("Meeres-Einfall auf der Seite");
    assert!(
        sea_at < html.find("data-weak-hits").unwrap(),
        "Begründung: der starke Treffer steht vor den schwachen, nicht im zugeklappten Teil"
    );
}

#[tokio::test]
async fn literature_by_meaning_folds_weak_hits_on_the_page_and_in_the_popup() {
    let (server, work, ..) = indexed().await;
    let q = url_escape("Wogen und Gischt");
    let (status, html) = server.get(&format!("/literatur?work={work}&lq={q}&lmode=rag")).await;
    assert_eq!(status, StatusCode::OK, "Literatur im Modus Bedeutung");
    assert!(html.contains("data-rag=\"ok\""), "Begründung: LanceDB ist über den Nachbau verbunden: {html}");
    let folded = between(&html, "<details class=\"weak-hits\" data-weak-hits>", "</details>");
    assert!(
        folded.contains("Im Garten") && folded.contains("data-weak-label>schwach</span>"),
        "Begründung: der Gartenabschnitt ist zur Meeresfrage schwach und steht zugeklappt mit „schwach“: {folded}"
    );
    assert!(!folded.contains("Am Meer"), "Begründung: der starke Treffer steht nicht im zugeklappten Teil: {folded}");
    let strong = html.find("Am Meer").expect("Meeresabschnitt als Treffer");
    assert!(strong < html.find("data-weak-hits").unwrap(), "Begründung: starke Treffer vor den schwachen");

    // Work page: the popup loads the same template through literature.render.
    let rendered =
        ok(&server, "literature.render", json!({ "work": work, "lq": "Wogen und Gischt", "lmode": "rag" })).await;
    let popup = rendered["html"].as_str().unwrap();
    assert!(
        popup.contains("<details class=\"weak-hits\" data-weak-hits>"),
        "Begründung: auch das Popup der Arbeitsseite klappt schwache Treffer zu: {popup}"
    );
    let view = baruch_server::literature::rag_view(
        &baruch_server::search_hooks::rag_search(
            &server.state,
            &baruch_rag::SearchRequest {
                work,
                query: "Wogen und Gischt".into(),
                ways: Some(vec![baruch_rag::Way::Lancedb]),
                collections: None,
                limit: 10,
                origin: "test".into(),
            },
        )
        .await
        .unwrap(),
    );
    let weak: Vec<bool> = view["hits"].as_array().unwrap().iter().map(|h| h["weak"].as_bool().unwrap()).collect();
    assert!(weak.contains(&true) && weak.contains(&false), "Begründung: JSON trägt weak an jedem Treffer: {view}");
    assert!(weak.windows(2).all(|w| w[0] <= w[1]), "Begründung: schwache Treffer zuletzt: {view}");
    assert_eq!(
        view["weak_count"].as_u64().unwrap() as usize,
        weak.iter().filter(|w| **w).count(),
        "Begründung: weak_count zählt die schwachen Treffer für die Überschrift von <details>"
    );
}

// ---- idea.search without embedder --------------------------------------------------------------------------

#[tokio::test]
async fn idea_search_by_meaning_without_an_embedder_fails_with_rag_unavailable() {
    baruch_server::i18n::scope(baruch_server::i18n::Lang::De, async {
    let server = common::open().await;
    let work = server.state.default_work().await.unwrap();
    ok(&server, "idea.create", json!({ "work": work, "words": ["Möwen"], "note": "Die Möwen kehren zurück." })).await;
    let answer = server.rpc("idea.search", json!({ "work": work, "query": "Möwen", "mode": "rag" })).await;
    assert_eq!(
        answer["error"]["code"],
        baruch_server::rpc::RAG_UNAVAILABLE,
        "Begründung: ohne Einbetter (Tests binden keinen an) ist die Bedeutungssuche ausgefallen – -32001 statt einer \
         leeren Liste: {answer}"
    );
    let message = answer["error"]["message"].as_str().unwrap();
    assert!(
        message.starts_with("Bedeutungssuche nicht verfügbar: LanceDB") && message.contains("Einbettungsmodell"),
        "Begründung: die Nachricht nennt Weg und Grund auf Deutsch: {message}"
    );

    let tool = mcp(&server, "idea_search", json!({ "work": work, "query": "Möwen", "mode": "rag" })).await;
    assert_eq!(tool["result"]["isError"], true, "Begründung: MCP meldet den Ausfall als isError: {tool}");
    let text = tool["result"]["content"][0]["text"].as_str().unwrap();
    // Round 39a: MCP answers in English.
    assert!(text.contains("Semantic search not available"), "Begründung: mit Grund: {text}");
    let fts = mcp(&server, "idea_search", json!({ "work": work, "query": "Möwen" })).await;
    assert_eq!(fts["result"]["isError"], false, "Begründung: ohne mode bleibt es die Volltextsuche: {fts}");

    let (_, html) = server.get(&format!("/einfaelle?work={work}&q=M%C3%B6wen&mode=rag")).await;
    assert!(
        html.contains("data-rag-missing") && html.contains("Einbettungsmodell"),
        "Begründung: die Seite /einfaelle zeigt den Grund weiter (ohne JavaScript): {html}"
    );
})
    .await
}

// ---- Wording with hyphenation leftovers --------------------------------------------------------------------

/// Ten invented verses (Ps 119:1–10); `remnants`: every second one with hyphenation leftovers (`hyphen_remnants`).
fn psalm(name: &str, remnants: bool) -> Fixture {
    let base = [
        "Am Morgen gingen die Fischer hinunter zum Ufer und warteten.",
        "Dieses Wort sagten die Fischer allen Menschen am Hafen.",
        "Die Menschen hörten dieses Wort und gingen heimwärts.",
        "Am Abend leuchteten Laternen über dem stillen Wasser.",
        "Die Fischer warteten geduldig auf günstigen Wind.",
        "Menschen standen am Ufer und betrachteten die Laternen.",
        "Dieses Licht leuchtete hinaus über den ganzen Hafen.",
        "Geduldig flickten die Fischer ihre Netze am Morgen.",
        "Günstigen Wind erhofften die Menschen jeden Abend.",
        "Die Laternen verloschen erst spät nach Mitternacht.",
    ];
    let mut fixture = Fixture::new(name, Layout::ZText(Block::Book), german())
        .conf("Description", "Erfundene Testbibel")
        .conf("DistributionLicense", "Public Domain");
    for (i, text) in base.iter().enumerate() {
        let text = if remnants && i % 2 == 1 { hyphen_remnants(text, 1) } else { (*text).to_owned() };
        fixture = fixture.verse("Ps", 119, u16::try_from(i + 1).unwrap(), &text);
    }
    fixture
}

/// Invented Bible in KJV versification (Malachi 4 = German Malachi 3:19–24).
fn english() -> Fixture {
    Fixture::new("Englisch", Layout::RawText, versification("KJV").unwrap())
        .conf("Description", "Erfundene Bibel mit KJV-Zählung")
        .verse("Mal", 4, 1, "Erfundener Vers eins in englischer Zählung.")
        .verse("Mal", 4, 2, "Erfundener Vers zwei in englischer Zählung.")
}

fn erfundenes_lexikon() -> LexiconFixture {
    LexiconFixture::new("Probelexikon", LexLayout::RawLD4)
        .conf("Description", "Erfundenes Probelexikon")
        .conf("DistributionLicense", "Public Domain")
        .article("Baum", "Gewächs mit Stamm und Krone (erfunden).")
        .article("Ufer", "Rand des Wassers (erfunden).")
}

fn strong_lexikon() -> LexiconFixture {
    LexiconFixture::new("Stronglexikon", LexLayout::RawLD)
        .conf("Description", "Erfundenes Strong-Lexikon ohne Polster")
        .conf("DistributionLicense", "Public Domain")
        .conf("StrongsPadding", "false")
        .article("G0026", "Liebe (erfundener Artikel).")
}

async fn modules(server: &common::Server, dir: &Path, bibles: &[Fixture], lexicons: &[LexiconFixture]) {
    let root = dir.join("sword");
    std::fs::create_dir_all(&root).unwrap();
    for m in bibles {
        write(&root, m).unwrap();
    }
    for m in lexicons {
        write_lexicon(&root, m).unwrap();
    }
    ok(server, "bible.set_dirs", json!({ "dirs": [root.display().to_string()] })).await;
}

const PSALM_BOOK: &str = "= Am Hafen

Wie Ps 119,2 sagt: „Dieses Wort sagten die Fischer allen Menschen am Hafen.“

Falsch erinnert: „Dieses Wort sagten die Fischer allen Völkern am Hafen.“ (Ps 119,2)
";

#[tokio::test]
async fn a_correct_quote_against_a_module_with_remnants_matches_with_module_spacing() {
    let server = common::open().await;
    let dir = tempfile::tempdir().unwrap();
    modules(&server, dir.path(), &[psalm("Trennrest", true), psalm("Sauber", false)], &[]).await;
    let work = server.import(PSALM_BOOK, "hafen.typ").await.work;

    let checked =
        ok(&server, "scripture.check", json!({ "work": work, "text": PSALM_BOOK, "module": "Trennrest" })).await;
    let refs = checked["refs"].as_array().unwrap();
    assert_eq!(refs.len(), 2, "zwei Stellen: {checked}");
    assert_eq!(
        (refs[0]["wording_code"].as_str(), refs[0]["module_spacing"].as_bool()),
        (Some("match"), Some(true)),
        "Begründung: das Zitat ist korrekt, im Modul steht „Die ses … Mens chen“ – es weicht nur Leerraum ab, der Code \
         bleibt match und module_spacing zeigt den Leerraum-Treffer: {checked}"
    );
    assert_eq!(refs[0]["wording"], SPACING_LABEL, "Begründung: Bezeichnung „Wortlaut stimmt (Leerraum im Modul)“");
    assert_eq!(
        (refs[1]["wording_code"].as_str(), refs[1]["module_spacing"].as_bool()),
        (Some("differs"), Some(false)),
        "Begründung: „Völkern“ statt „Menschen“ bleibt „weicht ab“, auch ohne Leerraum: {checked}"
    );
    let note = checked["module_note"].as_str().unwrap_or_else(|| panic!("Hinweis zum Modul: {checked}"));
    assert!(
        note.contains("Modul Trennrest") && note.contains("Wortfragmente"),
        "Begründung: das Modul hat viele Trennreste neben ganzen Wörtern – Hinweis je Antwort: {note}"
    );
    assert_eq!(refs[0]["module_note"], checked["module_note"], "Begründung: derselbe Hinweis auch je Stelle");

    let clean = ok(&server, "scripture.check", json!({ "work": work, "text": PSALM_BOOK, "module": "Sauber" })).await;
    assert!(clean["module_note"].is_null(), "Begründung: ein sauberes Modul bekommt keinen Hinweis: {clean}");
    assert_eq!(
        (clean["refs"][0]["wording_code"].as_str(), clean["refs"][0]["module_spacing"].as_bool()),
        (Some("match"), Some(false)),
        "Begründung: ohne Trennreste stimmt der Wortlaut ohne Leerraum-Vergleich: {clean}"
    );
}

#[tokio::test]
async fn the_module_note_reaches_mcp_the_check_page_and_the_belege_facts() {
    let server = common::open().await;
    let dir = tempfile::tempdir().unwrap();
    modules(&server, dir.path(), &[psalm("Trennrest", true)], &[]).await;
    let work = server.import(PSALM_BOOK, "hafen.typ").await.work;
    ok(&server, "bible.set_work", json!({ "work": work, "module": "Trennrest" })).await;
    let loaded = ok(&server, "work.get", json!({ "work": work })).await;
    let key = loaded["work"]["nodes"][0]["key"].as_i64().unwrap();

    let tool = mcp(&server, "scripture_check", json!({ "work": work, "key": key })).await;
    let result = &tool["result"]["structuredContent"];
    assert!(
        result["module_note"].as_str().is_some_and(|n| n.contains("word fragments")),
        "Begründung: MCP (always English since Round 39c) scripture_check trägt denselben Hinweis wie RPC: {tool}"
    );
    assert_eq!(result["refs"][0]["module_spacing"], true, "Begründung: Leerraum-Treffer auch über MCP: {tool}");

    let (status, html) = server.get(&format!("/werke/{work}/pruefung?key={key}")).await;
    assert_eq!(status, StatusCode::OK, "Prüfanzeige ohne JavaScript");
    assert!(html.contains("data-module-note"), "Begründung: die Prüfanzeige zeigt den Hinweis zum Modul: {html}");
    assert!(
        html.contains(&esc(SPACING_LABEL)) && html.contains("data-module-spacing"),
        "Begründung: die Stelle trägt „Wortlaut stimmt (Leerraum im Modul)“: {html}"
    );

    let section = loaded["work"]["nodes"][0]["revisions"][0]["body"].as_str().unwrap();
    let facts = baruch_server::scripture::facts_for_work(&server.state, work, "@belege", section).await.unwrap();
    assert!(
        facts.contains("Hinweis: Modul Trennrest hat auffällig viele Wortfragmente"),
        "Begründung: @belege bekommt den Hinweis im Faktenblock: {facts}"
    );
    assert!(facts.contains(SPACING_LABEL), "Begründung: das Ergebnis der Stelle steht im Faktenblock: {facts}");
    let marked = baruch_server::scripture::mark_answer_for_work(
        &server.state,
        work,
        "@belege",
        section,
        "- Ps 119,2: Zitat.".into(),
    )
    .await;
    assert!(
        marked.contains(MARK_BIBLE) && marked.contains("Wortfragmente"),
        "Begründung: die Zeile von Baruch unter der Antwort nennt den Hinweis ebenfalls: {marked}"
    );
}

// ---- Lector hint ------------------------------------------------------------------------------------------

#[tokio::test]
async fn the_lector_note_names_the_module_when_the_work_has_a_bible() {
    let server = common::open().await;
    let dir = tempfile::tempdir().unwrap();
    modules(&server, dir.path(), &[psalm("Sauber", false)], &[]).await;
    let work = server.import(PSALM_BOOK, "hafen.typ").await.work;
    ok(&server, "bible.set_work", json!({ "work": work, "module": "Sauber" })).await;
    let answer = "Kritisch: vgl. Ps 119,3 – der Übergang fehlt.".to_owned();
    let marked =
        baruch_server::scripture::mark_answer_for_work(&server.state, work, "@lektor", PSALM_BOOK, answer.clone())
            .await;
    assert!(
        !marked.contains("hier ungeprüft"),
        "Begründung: mit gewählter Bibel prüft Baruch den Wortlaut selbst, „hier ungeprüft“ wäre falsch: {marked}"
    );
    assert!(
        marked.ends_with(&baruch_server::scripture::mark_others_bible("Sauber")),
        "Begründung: der Hinweis nennt das Modul und sagt, dass der Lektor den Wortlaut nicht selbst bestätigt: {marked}"
    );
    assert!(marked.contains("nicht selbst bestätigt"), "Begründung: der Satz ist ausdrücklich");

    ok(&server, "bible.set_work", json!({ "work": work, "module": null })).await;
    let without =
        baruch_server::scripture::mark_answer_for_work(&server.state, work, "@lektor", PSALM_BOOK, answer).await;
    assert!(without.ends_with(MARK_OTHERS), "Begründung: ohne Bibel bleibt der Hinweis wie bisher: {without}");
}

// ---- Reverse mapping of the Bible page --------------------------------------------------------------------

#[tokio::test]
async fn a_verse_of_a_kjv_module_sets_the_german_reference_by_link() {
    let server = common::open().await;
    let dir = tempfile::tempdir().unwrap();
    modules(&server, dir.path(), &[english()], &[]).await;
    let work = server.import(PSALM_BOOK, "hafen.typ").await.work;
    let (status, html) =
        server.get(&format!("/literatur/bibel?work={work}&module=Englisch&ref={}", url_escape("Mal 3,20"))).await;
    assert_eq!(status, StatusCode::OK, "Bibelseite ohne JavaScript");
    let verse = between(&html, "<li class=\"bible-verse\" data-ref=\"Mal 3,20\"", "</li>");
    assert!(
        verse.contains(">4,2</a>"),
        "Begründung: die Versnummer bleibt die des Moduls (KJV Mal 4,2), der Vers trägt aber die deutsche Stelle: {verse}"
    );
    assert!(
        verse.contains("data-verse-ref=\"Mal 3,20\"") && verse.contains("data-german-ref>= Mal 3,20"),
        "Begründung: ein Klick (mit JavaScript) setzt die deutsche Stelle, sichtbar als „= Mal 3,20“: {verse}"
    );
    assert!(
        verse.contains("href=\"/literatur/bibel?work=") && verse.contains("module=Englisch&amp;ref=Mal%203%2C20"),
        "Begründung: ohne JavaScript schlägt der Link die deutsche Stelle auf (Feld „Stelle“ = Mal 3,20): {verse}"
    );

    let looked = ok(&server, "bible.lookup", json!({ "work": work, "module": "Englisch", "ref": "Mal 3,20" })).await;
    assert_eq!(
        looked["refs"][0]["verses"][0]["german_ref"], "Mal 3,20",
        "Begründung: RPC liefert german_ref: {looked}"
    );

    // Not mappable (2 Kings 15 has 39 verses in German, 38 in KJV and no rule): honest reason.
    let Choice::Ready(bible) = baruch_server::bibles::by_name(&server.state, "Englisch").await.unwrap() else {
        panic!("Testbibel lesbar");
    };
    let (german_ref, problem) = baruch_server::bibles::german_of(
        &bible,
        baruch_scripture::versification::mapping::VerseRef { book: "2Kgs", chapter: 15, verse: 1 },
    );
    assert!(german_ref.is_none(), "Begründung: ohne Regel wird nicht geraten");
    assert!(
        problem.as_deref().is_some_and(|p| p.starts_with("Versifikation passt nicht")),
        "Begründung: der Grund wird gemeldet: {problem:?}"
    );
    assert_eq!(german().name, "German", "Begründung: Ziel der Rückabbildung ist die deutsche Zählung");
}

// ---- Lemma probe ------------------------------------------------------------------------------------------

#[tokio::test]
async fn lexicon_lookup_probes_umlaut_compound_and_strong_padding() {
    let server = common::open().await;
    let dir = tempfile::tempdir().unwrap();
    modules(&server, dir.path(), &[], &[erfundenes_lexikon(), strong_lexikon()]).await;
    let work = server.import(PSALM_BOOK, "hafen.typ").await.work;
    let lookup = |key: &'static str, module: &'static str| {
        let server = &server;
        async move { ok(server, "lexicon.lookup", json!({ "work": work, "key": key, "module": module })).await }
    };
    let trees = lookup("Bäume", "Probelexikon").await;
    assert_eq!(
        (trees["found"].as_bool(), trees["key"].as_str(), trees["probe"].as_str()),
        (Some(true), Some("BAUM"), Some("Baum")),
        "Begründung: Umlaut-Rückführung „Bäume“ → „Baum“, die probierte Form steht in probe: {trees}"
    );
    let compound = lookup("Feigenbaum", "Probelexikon").await;
    assert_eq!(
        (compound["key"].as_str(), compound["probe"].as_str()),
        (Some("BAUM"), Some("Baum")),
        "Begründung: ein Kompositum fällt auf seinen letzten Bestandteil zurück: {compound}"
    );
    let strong = lookup("G26", "Stronglexikon").await;
    assert_eq!(
        (strong["found"].as_bool(), strong["key"].as_str()),
        (Some(true), Some("G0026")),
        "Begründung: „G26“ findet bei StrongsPadding=false den gepolsterten Eintrag „G0026“: {strong}"
    );
    let direct = lookup("Ufer", "Probelexikon").await;
    assert!(direct["probe"].is_null(), "Begründung: das Stichwort selbst braucht keine Probe: {direct}");
    let missing = lookup("Morgenrot", "Probelexikon").await;
    assert_eq!(missing["found"], false, "Begründung: was nicht passt, bleibt ehrlich „nicht gefunden“: {missing}");

    let tool = mcp(&server, "lexicon_lookup", json!({ "work": work, "key": "Bäume", "module": "Probelexikon" })).await;
    assert_eq!(
        tool["result"]["structuredContent"]["key"], "BAUM",
        "Begründung: MCP lexicon_lookup nutzt dieselbe Probe: {tool}"
    );
}

// ---- Bible context of the work page -----------------------------------------------------------------------

#[tokio::test]
async fn the_workspace_renders_the_bible_shelf_without_javascript() {
    let server = common::open().await;
    let dir = tempfile::tempdir().unwrap();
    modules(&server, dir.path(), &[psalm("Sauber", false)], &[]).await;
    let work = server.import(PSALM_BOOK, "hafen.typ").await.work;
    ok(&server, "bible.set_work", json!({ "work": work, "module": "Sauber" })).await;
    let loaded = ok(&server, "work.get", json!({ "work": work })).await;
    let key = loaded["work"]["nodes"][0]["key"].as_i64().unwrap();
    let (status, html) = server.get(&format!("/?work={work}&key={key}")).await;
    assert_eq!(status, StatusCode::OK, "Arbeitsseite");
    let shelf = between(&html, "<section class=\"literature-bibles\"", "</section>");
    assert!(
        shelf.contains("data-module=\"Sauber\"") && shelf.contains("Bibel des Werks"),
        "Begründung: der Kasten „Bibeln“ steht serverseitig im Literatur-Bereich der Arbeitsseite: {shelf}"
    );
    assert!(
        shelf.contains(&format!("&#x2f;literatur&#x2f;bibel?work={work}&amp;module=Sauber&amp;key={key}&amp;rev=1"))
            || shelf.contains(&format!("/literatur/bibel?work={work}&amp;module=Sauber&amp;key={key}&amp;rev=1")),
        "Begründung: der Link trägt Werk, Schlüssel und Fassung (Zitieren aus dem Abschnitt geht): {shelf}"
    );

    // Without configured module folders and without a Bible of the work: honest hint, no reading of the default
    // folders.
    let plain = common::open().await;
    let other = plain.import(PSALM_BOOK, "hafen.typ").await.work;
    let (_, html) = plain.get(&format!("/?work={other}")).await;
    assert!(
        html.contains("data-bibles=\"none\""),
        "Begründung: ohne Bibel zeigt der Kasten den Hinweis auf Einstellungen → Quellen: {html}"
    );
}
