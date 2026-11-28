// SPDX-License-Identifier: AGPL-3.0-or-later
//! Round 12, task „pgvector“: user interface of the semantic search without JavaScript and the RPC for it.
//!
//! - Einstellungen → Verbindungen, section `#rag`: collections per way (repeated field `collection`, `collection_new`),
//!   „Status der Wege“, „Index neu aufbauen“ (with/without `confirm`), message codes, LanceDB check button free, usage
//!   texts according to contract §5.
//! - RPC `connection.collections_get/set` (not through MCP, not through `POST /act`).
//! - Literature (`lmode`) and ideas (`mode`): switch as a GET form, `lmode` travels along in links; without an embedder
//!   (tests) LanceDB honestly fails, the page shows the reason and links the FTS search.
//! - Pure render functions for an answer (`literature::rag_view`): way badge, state line, safe links.

use crate::common;

use axum::http::StatusCode;
use baruch_rag::{Answer, Hit, MergedHit, Way, WayRank, WayReport, WayState};
use baruch_server::connections::{render_connections, usage_is_stub, usage_stub};
use baruch_server::literature::{hit_link, rag_view, safe_link, ways_line};
use baruch_store::{ConnectionKind, RagWay};
use serde_json::{Value, json};

/// Since the integration the semantic search is wired; tests attach no embedder (never network), so LanceDB honestly
/// fails with this reason (beginning of `rag::NO_EMBEDDER`).
const NO_EMBEDDER: &str = "Kein Einbettungsmodell angebunden";

async fn ok(server: &common::Server, method: &str, params: Value) -> Value {
    let answer = server.rpc(method, params).await;
    assert!(answer["error"].is_null(), "{method} sollte gelingen: {answer}");
    answer["result"].clone()
}

async fn fails(server: &common::Server, method: &str, params: Value, code: i64) -> String {
    let answer = server.rpc(method, params).await;
    assert_eq!(answer["error"]["code"], code, "{method} muss mit {code} scheitern: {answer}");
    answer["error"]["message"].as_str().unwrap().to_owned()
}

async fn page(server: &common::Server, work: Option<i64>, pairs: &[(&str, &str)]) -> String {
    let q = pairs.iter().map(|(k, v)| ((*k).to_owned(), (*v).to_owned())).collect();
    baruch_server::i18n::scope(baruch_server::i18n::Lang::De, render_connections(&server.state, work, &q))
        .await
        .expect("Seite Verbindungen")
}

fn section<'a>(html: &'a str, kind: &str) -> &'a str {
    let start = html.find(&format!("id=\"{kind}\"")).unwrap_or_else(|| panic!("Abschnitt {kind} fehlt: {html}"));
    let rest = &html[start..];
    let end = rest[1..].find("<section class=\"connection\"").map_or(rest.len(), |i| i + 1);
    &rest[..end]
}

/// Part of the RAG section for one way (`data-collections="…"` up to the next form).
fn collections_form<'a>(html: &'a str, way: &str) -> &'a str {
    let rag = section(html, "rag");
    let start = rag.find(&format!("data-collections=\"{way}\"")).unwrap_or_else(|| panic!("Sammlungen {way}: {rag}"));
    let rest = &rag[start..];
    let end = rest.find("</form>").unwrap_or(rest.len());
    &rest[..end]
}

async fn form(server: &common::Server, body: &str) -> String {
    let (status, location) = server.post_form("/einstellungen/verbindungen/act", body).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "das Formular antwortet immer mit 303 ({body})");
    location.expect("303 mit Location")
}

fn esc(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
        .replace('/', "&#x2f;")
}

// ---- Einstellungen → Verbindungen -----------------------------------------------------------------

#[tokio::test]
async fn usage_texts_follow_the_contract_and_every_connection_can_be_checked() {
    assert_eq!(
        usage_stub(ConnectionKind::Multigpt),
        "Suche über MultiGPT (search_documents) in den gewählten Sammlungen des Werks, Treffer mit Quellenangabe; read_document als Ausschnitt einer Fundstelle und als Volltext in der Dokumentansicht der Literatur (teilweise, ohne JavaScript); Hervorhebungen ohne Zitat werden gegen den Volltext geprüft.",
        "MultiGPT wörtlich (Runde 13: Volltext umgesetzt)"
    );
    assert_eq!(
        usage_stub(ConnectionKind::Pgvector),
        "Bedeutungssuche direkt in pgvector (Kosinus-Abstand, nur lesend) in den gewählten Sammlungen des Werks; die Anfrage wird mit demselben Einbettungsmodell eingebettet wie die Daten.",
        "pgvector wörtlich nach Vertrag §5"
    );
    assert_eq!(
        usage_stub(ConnectionKind::Lancedb),
        "Haupt-RAG im Datenordner: Abschnitte (aktuelle Fassung), Einfälle, Hervorhebungen und die Autorenbibliothek, eingebettet über Eliezer; ohne Einbettungsmodell bleibt die Wortlaut-Suche (FTS).",
        "LanceDB wörtlich nach Vertrag §5"
    );
    for kind in ConnectionKind::ALL {
        assert!(!usage_is_stub(kind), "{kind}: die Nutzung ist umgesetzt (usage_is_stub = false)");
    }
    let server = common::open().await;
    let html = page(&server, None, &[]).await;
    for kind in ["eliezer", "multigpt", "pgvector", "lancedb"] {
        let part = section(&html, kind);
        assert!(part.contains("value=\"connection.check\""), "{kind}: Knopf „Verbindung prüfen“ als Formular: {part}");
        assert!(!part.contains("disabled"), "{kind}: kein gesperrter Knopf mehr: {part}");
        let usage = part.split("data-usage>").nth(1).unwrap_or("");
        assert!(
            !usage.starts_with("Noch nicht umgesetzt"),
            "{kind}: Nutzung nicht mehr als „Noch nicht umgesetzt“: {part}"
        );
    }
    let multigpt = section(&html, "multigpt");
    assert!(
        multigpt.contains("data-usage") && !multigpt.contains("data-stub"),
        "Runde 13: bei MultiGPT ist nichts mehr offen (Volltext, Abgleich) – kein data-stub, nur die Nutzung: {multigpt}"
    );
    assert!(
        baruch_server::connections::open_part(ConnectionKind::Multigpt).is_none(),
        "open_part(Multigpt) ist seit Runde 13 None"
    );
    let pg = section(&html, "pgvector");
    for (name, value) in [
        ("table", "chunks"),
        ("text_column", "content"),
        ("vector_column", "embedding"),
        ("collection_column", "collection"),
        ("document_column", "document"),
        ("embedding_model", "nomic-embed-text"),
    ] {
        assert!(
            pg.contains(&format!("name=\"{name}\" value=\"{value}\"")),
            "Zuordnung {name} mit Vorgabe {value}: {pg}"
        );
    }
    assert!(pg.contains("name=\"section_column\" value=\"\""), "Stelle: Vorgabe leer");
}

#[tokio::test]
async fn the_rag_section_offers_collections_status_and_reindex_without_javascript() {
    let server = common::open().await;
    let work = server.state.default_work().await.unwrap();
    let html = page(&server, Some(work), &[]).await;
    let rag = section(&html, "rag");
    for way in ["pgvector", "multigpt"] {
        let part = collections_form(&html, way);
        assert!(
            part.contains("value=\"connection.collections_set\"")
                && part.contains(&format!("name=\"way\" value=\"{way}\""))
                && part.contains(&format!("name=\"work\" value=\"{work}\"")),
            "{way}: Formular mit Methode, Weg und Werk: {part}"
        );
        assert!(part.contains("name=\"collection_new\""), "{way}: Textfeld für eine neue Sammlung: {part}");
        assert!(
            part.contains(&esc("Erst „Verbindung prüfen“, dann erscheinen die Sammlungen.")),
            "{way}: Hinweis ohne Prüfung"
        );
    }
    assert!(
        !rag.contains("data-rag-status-missing") && rag.contains(NO_EMBEDDER),
        "Status der Wege nennt den Grund (ohne Einbetter): {rag}"
    );
    assert!(
        rag.contains("value=\"rag.reindex\"") && rag.contains("name=\"confirm\" value=\"1\" required"),
        "„Index neu aufbauen“ mit Bestätigung: {rag}"
    );
    assert!(!rag.contains("Noch nicht umgesetzt"), "der alte STUB-Hinweis unter den Wegen ist weg: {rag}");

    // Choosing without JavaScript: repeated field collection and collection_new.
    let body = format!(
        "method=connection.collections_set&work={work}&way=pgvector&collection=Leuchtfeuer&collection=Gezeiten&collection_new=+Seekarten+"
    );
    assert_eq!(
        form(&server, &body).await,
        format!("/einstellungen/verbindungen?ok=sammlungen&work={work}#rag"),
        "gespeichert"
    );
    assert_eq!(
        server.state.store.work_collections(work, RagWay::Pgvector).await.unwrap(),
        ["Leuchtfeuer", "Gezeiten", "Seekarten"],
        "in der Reihenfolge des Formulars, neue Sammlung beschnitten am Ende"
    );
    assert!(
        server.state.store.work_collections(work, RagWay::Multigpt).await.unwrap().is_empty(),
        "MultiGPT unberührt"
    );
    let html = page(&server, Some(work), &[("ok", "sammlungen")]).await;
    assert!(html.contains("Die Sammlungen des Werks sind gespeichert."), "Erfolgsmeldung auf Deutsch");
    let part = collections_form(&html, "pgvector");
    assert!(
        part.contains("name=\"collection\" value=\"Leuchtfeuer\" checked")
            && part.contains("value=\"Seekarten\" checked"),
        "gewählte Sammlungen angehakt: {part}"
    );
    assert!(!part.contains("data-gone"), "ohne Prüfung heißt nichts „nicht mehr gemeldet“: {part}");

    // After a check: reported collections as checkboxes, chosen ones without the message „nicht mehr gemeldet“.
    let details = json!({ "server_version": "PostgreSQL 17", "vector": "0.8.0", "table": "public.chunks",
                          "collections": [{ "name": "Leuchtfeuer", "count": 12 }, { "name": "Brandung", "count": 3 }],
                          "chunks": 15, "dimension": 768, "models": ["nomic-embed-text"] });
    server
        .state
        .store
        .connection_record_check(ConnectionKind::Pgvector, true, "Verbindung steht", &details)
        .await
        .unwrap();
    let html = page(&server, Some(work), &[]).await;
    let part = collections_form(&html, "pgvector");
    assert!(part.contains("value=\"Brandung\">") && part.contains("(3 Abschnitte)"), "gemeldet, nicht gewählt: {part}");
    assert!(
        part.contains("value=\"Leuchtfeuer\" checked") && part.contains("(12 Abschnitte)"),
        "gemeldet und gewählt: {part}"
    );
    let gone = part.split("value=\"Gezeiten\"").nth(1).unwrap_or("");
    assert!(
        gone.split("</label>").next().unwrap_or("").contains("nicht mehr gemeldet"),
        "Gezeiten fehlt in der Prüfung: {part}"
    );
    assert!(!part.contains("Erst „Verbindung prüfen“"), "mit Prüfung kein Hinweis mehr");
    let pg = section(&html, "pgvector");
    for line in [
        "Tabelle: public.chunks",
        "Abschnitte: 15",
        "Dimension: 768",
        "Sammlungen: Leuchtfeuer (12 Abschnitte), Brandung (3 Abschnitte)",
    ] {
        assert!(pg.contains(line), "Angaben der Prüfung als Zeile „{line}“: {pg}");
    }

    // Deselecting: only the ticked ones stay.
    form(&server, &format!("method=connection.collections_set&work={work}&way=pgvector&collection=Brandung")).await;
    assert_eq!(server.state.store.work_collections(work, RagWay::Pgvector).await.unwrap(), ["Brandung"], "ersetzt");

    // Errors: fixed codes, German texts, nothing changed.
    let long = "x".repeat(201);
    let location =
        form(&server, &format!("method=connection.collections_set&work={work}&way=pgvector&collection_new={long}"))
            .await;
    assert_eq!(location, format!("/einstellungen/verbindungen?fehler=sammlung&work={work}"), "zu lang");
    assert_eq!(
        server.state.store.work_collections(work, RagWay::Pgvector).await.unwrap(),
        ["Brandung"],
        "nichts geändert"
    );
    let location =
        form(&server, &format!("method=connection.collections_set&work={work}&way=lancedb&collection=x")).await;
    assert_eq!(location, format!("/einstellungen/verbindungen?fehler=weg&work={work}"), "LanceDB hat keine Sammlungen");
    let html = page(&server, Some(work), &[("fehler", "sammlung")]).await;
    assert!(
        html.contains(&esc("Sammlung ungültig (1 bis 200 Zeichen, eine Zeile, höchstens 100); nichts geändert.")),
        "fehler=sammlung auf Deutsch"
    );

    // Rebuilding the index: without the checkbox fehler=bestaetigen, with the checkbox without an embedder honestly
    // „ließ sich nicht“.
    let location = form(&server, &format!("method=rag.reindex&work={work}")).await;
    assert_eq!(location, format!("/einstellungen/verbindungen?fehler=bestaetigen&work={work}"), "ohne confirm");
    let location = form(&server, &format!("method=rag.reindex&work={work}&confirm=1")).await;
    assert_eq!(
        location,
        format!("/einstellungen/verbindungen?fehler=reindex&work={work}"),
        "ohne Einbetter ehrlich gescheitert, nicht „ok“"
    );
    let html = page(&server, Some(work), &[("fehler", "reindex")]).await;
    assert!(
        html.contains(&esc("Der Index ließ sich nicht neu aufbauen; den Grund nennt „Status der Wege“.")),
        "fehler=reindex"
    );
    let html = page(&server, Some(work), &[("ok", "reindex")]).await;
    assert!(
        html.contains(&esc("Der Index des Werks wird neu aufgebaut; den Stand zeigt „Status der Wege“.")),
        "ok=reindex"
    );
}

#[tokio::test]
async fn collections_over_rpc_only_for_remote_ways_and_never_over_mcp_or_act() {
    let server = common::open().await;
    let work = server.state.default_work().await.unwrap();
    let got = ok(&server, "connection.collections_get", json!({ "way": "multigpt" })).await;
    assert_eq!(
        got,
        json!({ "work": work, "way": "multigpt", "collections": [], "available": [], "available_at": null }),
        "ohne Werk das Vorgabewerk; nie geprüft: available leer, available_at null"
    );
    let set = ok(
        &server,
        "connection.collections_set",
        json!({ "work": work, "way": "multigpt", "collections": ["Seekarten", " Seekarten ", "Gezeiten"] }),
    )
    .await;
    assert_eq!(
        set,
        json!({ "work": work, "way": "multigpt", "collections": ["Seekarten", "Gezeiten"] }),
        "ohne Doppel"
    );
    let details = json!({ "server": { "name": "MultiGPT", "version": "1" }, "tools": ["search_documents"],
                          "collections": [{ "name": "Seekarten", "count": null }, { "name": "", "count": 2 }] });
    server.state.store.connection_record_check(ConnectionKind::Multigpt, true, "ok", &details).await.unwrap();
    let got = ok(&server, "connection.collections_get", json!({ "work": work, "way": "multigpt" })).await;
    assert_eq!(
        got["available"],
        json!([{ "name": "Seekarten", "count": null }]),
        "aus der letzten Prüfung, leere Namen fallen weg"
    );
    assert!(got["available_at"].is_string(), "Zeit der letzten Prüfung: {got}");

    for (params, why) in [
        (json!({ "way": "lancedb" }), "LanceDB hat keine Sammlungen"),
        (json!({ "way": "ollama" }), "unbekannter Weg"),
        (json!({}), "way fehlt"),
    ] {
        fails(&server, "connection.collections_get", params, -32602).await;
        let _ = why;
    }
    for (params, why) in [
        (json!({ "way": "lancedb", "collections": ["x"] }), "LanceDB"),
        (json!({ "way": "pgvector", "collections": "x" }), "keine Liste"),
        (json!({ "way": "pgvector", "collections": [1] }), "kein Text"),
        (json!({ "way": "pgvector", "collections": ["a\nb"] }), "mehrzeilig"),
        (json!({ "way": "pgvector", "collections": [""] }), "leer"),
        (json!({ "way": "pgvector", "work": 9999, "collections": ["x"] }), "unbekanntes Werk"),
    ] {
        let message = fails(&server, "connection.collections_set", params, -32602).await;
        assert!(!message.is_empty(), "{why}: abgelehnt mit Meldung");
    }
    assert!(server.state.store.work_collections(work, RagWay::Pgvector).await.unwrap().is_empty(), "nichts geändert");

    let tools: Vec<&str> = baruch_server::mcp::tools::TOOLS.iter().map(|t| t.name).collect();
    assert!(
        !tools.iter().any(|t| t.contains("collection") || t.contains("reindex")),
        "Sammlungen wählen und Index neu aufbauen bleiben dem Menschen vorbehalten (kein MCP-Werkzeug): {tools:?}"
    );
    let (status, _) =
        server.post_form("/act", &format!("method=connection.collections_set&work={work}&way=pgvector")).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "POST /act kennt die Methode nicht");
    let (status, _) = server.post_form("/act", &format!("method=rag.reindex&work={work}")).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "auch rag.reindex nicht über POST /act");
}

// ---- Literature -----------------------------------------------------------------------------------

#[tokio::test]
async fn the_literature_switch_works_without_javascript_and_says_why_rag_is_missing() {
    let server = common::open().await;
    let work = server.state.default_work().await.unwrap();
    let (status, html) = server.get(&format!("/literatur?work={work}")).await;
    assert_eq!(status, StatusCode::OK, "Seite Literatur");
    assert!(
        html.contains("<legend class=\"tiny\">Suche nach</legend>")
            && html.contains("name=\"lmode\" value=\"fts\" checked")
            && !html.contains("name=\"lmode\" value=\"rag\" checked"),
        "Umschalter als Radio im GET-Formular, Vorgabe Wortlaut: {html}"
    );
    assert!(html.contains("Wortlaut (FTS)") && html.contains("Bedeutung (RAG)"), "Beschriftungen nach Vertrag §8.2");

    let (status, html) = server.get(&format!("/literatur?work={work}&lq=M%C3%B6wen&lmode=rag")).await;
    assert_eq!(status, StatusCode::OK, "Bedeutungssuche ohne JavaScript");
    assert!(html.contains("name=\"lmode\" value=\"rag\" checked"), "rag bleibt gewählt");
    assert!(
        html.contains(&esc(&format!("Bedeutungssuche nicht verfügbar: LanceDB: {NO_EMBEDDER}")))
            && html.matches("Die Wortlaut-Suche (FTS) geht weiter.").count() == 1,
        "ehrlicher Grund statt stillen Umschaltens: {html}"
    );
    let fts_link = format!("href=\"/literatur?work={work}&amp;lq=M%C3%B6wen\" data-fts-link");
    assert!(html.contains(&fts_link), "Link auf dieselbe Suche im Wortlaut (ohne lmode): {html}");
    assert!(!html.contains("<div class=\"library-hits\">"), "keine FTS-Treffer untergeschoben (nur der RAG-Block)");
    assert!(
        html.contains("&amp;lq=M%C3%B6wen&amp;lmode=rag"),
        "lmode reist in den Links mit (z. B. Dokumentansicht): {html}"
    );

    let (_, source) = server.get(&format!("/quelle?work={work}&title=1&lq=M%C3%B6wen&lmode=rag")).await;
    assert!(source.contains("data-lmode=\"rag\""), "Dokumentansicht kennt lmode (Kontextmenü): {source}");
    assert!(
        source.contains("name=\"lmode\" value=\"rag\""),
        "das Formular für markierten Text trägt lmode als verstecktes Feld: {source}"
    );
    assert!(source.contains("lmode=rag"), "Links der Dokumentansicht (Literatur, zurück) tragen lmode");

    let result = ok(&server, "literature.render", json!({ "work": work, "lq": "Möwen", "lmode": "rag" })).await;
    assert_eq!(result["lmode"], "rag", "RPC nennt die Art der Suche");
    assert!(result["html"].as_str().unwrap().contains(NO_EMBEDDER), "Popup zeigt denselben Grund: {result}");
    let result = ok(&server, "literature.render", json!({ "work": work, "lq": "Möwen", "lmode": "quatsch" })).await;
    assert_eq!(result["lmode"], "fts", "Unbekanntes lmode ist die Vorgabe Wortlaut");

    let (_, workspace) = server.get(&format!("/?work={work}&lq=M%C3%B6wen")).await;
    assert!(!workspace.contains("Bedeutungssuche nicht verfügbar"), "die Arbeitsseite bleibt bei FTS");
}

#[test]
fn links_of_hits_are_only_own_addresses_or_http() {
    assert_eq!(safe_link("/?work=1&key=2").as_deref(), Some("/?work=1&key=2"), "eigene Adresse");
    assert_eq!(safe_link("https://quelle.example/a?b=1").as_deref(), Some("https://quelle.example/a?b=1"), "https");
    assert_eq!(
        safe_link("HTTP://quelle.example/").as_deref(),
        Some("HTTP://quelle.example/"),
        "Schema ohne Rücksicht auf Groß/klein"
    );
    for bad in [
        "javascript:alert(1)",
        " JavaScript:alert(1)",
        "data:text/html,x",
        "//fremd.example/",
        "/\\fremd.example",
        "https://a.example/\" onclick=\"x",
        "https://a.example/<b>",
        "/a b",
        "https://",
        "",
        "relativ/ohne/schraegstrich",
    ] {
        assert_eq!(safe_link(bad), None, "{bad:?} wird verworfen (kein Link)");
    }
}

fn hit(way: Way, document: &str, section: Option<&str>, url: Option<&str>, meta: Value, score: f32) -> Hit {
    Hit {
        way,
        collection: if way == Way::Lancedb { "section".into() } else { "Seekarten".into() },
        document: document.into(),
        section: section.map(str::to_owned),
        title: format!("Titel {document}"),
        snippet: "Die Möwen <kehren> zurück.".into(),
        score,
        url: url.map(str::to_owned),
        meta,
    }
}

fn merged(hit: Hit, rank: usize, found_by: Vec<Way>) -> MergedHit {
    let found_by = found_by.into_iter().map(|way| WayRank { way, rank, score: hit.score }).collect();
    MergedHit { hit, rank, fused: 0.0, found_by }
}

#[test]
fn an_answer_renders_badges_states_percent_and_safe_links() {
    let answer = Answer {
        work: 1,
        query: "Möwen".into(),
        hits: vec![
            merged(
                hit(Way::Lancedb, "work:1", Some("key:7"), None, json!({ "work": 1, "key": 7 }), 0.875),
                1,
                vec![Way::Lancedb, Way::Multigpt],
            ),
            merged(
                hit(Way::Multigpt, "doc-9", None, Some("javascript:alert(1)"), json!({}), 0.5),
                2,
                vec![Way::Multigpt],
            ),
            merged(
                hit(Way::Multigpt, "doc-10", None, Some("https://gpt.example/d/10"), json!({}), 0.4),
                3,
                vec![Way::Multigpt],
            ),
        ],
        ways: vec![
            WayReport::new(Way::Lancedb, WayState::Connected, "", 1, 3),
            WayReport::new(
                Way::Pgvector,
                WayState::Off,
                "Keine Sammlung gewählt (Einstellungen → Verbindungen).",
                0,
                0,
            ),
            WayReport::new(Way::Multigpt, WayState::Failed, "Zeitlimit", 0, 20000),
        ],
    };
    assert_eq!(
        ways_line(&answer),
        "LanceDB: verbunden · pgvector: aus · MultiGPT: ausgefallen (Zeitlimit)",
        "Zustandszeile nach Vertrag §8.2"
    );
    let view = rag_view(&answer);
    assert_eq!(view["available"], true, "ein Weg ist verbunden: {view}");
    let hits = view["hits"].as_array().unwrap();
    assert_eq!(hits[0]["link"], "/?work=1&key=7", "LanceDB ohne Adresse: Ort aus meta.work/meta.key");
    assert_eq!(hits[1]["link"], Value::Null, "javascript: ergibt keinen Link");
    assert_eq!(hits[2]["link"], "https://gpt.example/d/10", "http(s) der Gegenstelle bleibt");
    assert_eq!((hits[0]["percent"].clone(), hits[1]["percent"].clone()), (json!(88), json!(50)), "Ähnlichkeit in %");
    assert_eq!(hits[0]["way_title"], "LanceDB", "Weg-Abzeichen");
    assert_eq!(hits[0]["found_by"], json!(["LanceDB", "MultiGPT"]), "alle findenden Wege");
    assert_eq!(hits[0]["snippet"], "Die Möwen <kehren> zurück.", "Ausschnitt als Text (die Vorlage maskiert)");
    assert_eq!(hit_link(&answer.hits[1]), None, "dieselbe Regel in hit_link");

    let off = Answer {
        hits: vec![],
        ways: vec![
            WayReport::new(Way::Lancedb, WayState::Failed, "Kein Einbettungsmodell erreichbar", 0, 1),
            WayReport::new(
                Way::Pgvector,
                WayState::Off,
                "Für dieses Werk nicht gewählt (Einstellungen → Verbindungen).",
                0,
                0,
            ),
            WayReport::new(Way::Multigpt, WayState::Off, "In dieser Anfrage nicht gefragt.", 0, 0),
        ],
        ..answer
    };
    let view = rag_view(&off);
    assert_eq!(view["available"], false, "kein Weg verbunden: nicht verfügbar");
    let message = view["message"].as_str().unwrap();
    assert!(
        message.starts_with("Bedeutungssuche nicht verfügbar: LanceDB: Kein Einbettungsmodell erreichbar")
            && message.ends_with("Die Wortlaut-Suche (FTS) geht weiter."),
        "Grund je Weg und der Hinweis auf FTS: {message}"
    );
    assert!(view["ways_line"].as_str().unwrap().contains("LanceDB: ausgefallen"), "Zustandszeile bleibt: {view}");
}

// ---- Ideas ----------------------------------------------------------------------------------------

#[tokio::test]
async fn ideas_switch_to_meaning_only_within_a_work_and_say_why_it_is_missing() {
    let server = common::open().await;
    let work = server.state.default_work().await.unwrap();
    ok(
        &server,
        "idea.create",
        json!({ "work": work, "words": ["Möwen", "Frühling"], "note": "Die Möwen kehren zurück." }),
    )
    .await;

    let (status, html) = server.get(&format!("/einfaelle?work={work}")).await;
    assert_eq!(status, StatusCode::OK, "Liste");
    assert!(
        html.contains("name=\"mode\" value=\"fts\" checked") && html.contains("Bedeutung (RAG)"),
        "Umschalter im Filter, Vorgabe Wortlaut: {html}"
    );
    let (_, html) = server.get(&format!("/einfaelle?work={work}&q=M%C3%B6wen&mode=rag")).await;
    assert!(html.contains("name=\"mode\" value=\"rag\" checked"), "rag gewählt");
    assert!(
        html.contains(&esc(&format!("Bedeutungssuche nicht verfügbar: LanceDB: {NO_EMBEDDER}")))
            && html.matches("Die Wortlaut-Suche (FTS) geht weiter.").count() == 1,
        "ehrlicher Grund: {html}"
    );
    assert!(
        html.contains(&format!("href=\"/einfaelle?work={work}&amp;q=M%C3%B6wen\" data-fts-link")),
        "Link auf dieselbe Suche im Wortlaut: {html}"
    );
    assert!(!html.contains("id=\"einfall-"), "keine Treffer untergeschoben");

    let (_, html) = server.get("/einfaelle?work=all&q=M%C3%B6wen&mode=rag").await;
    assert!(
        html.contains(&esc("Bedeutungssuche nur innerhalb eines Werks; gezeigt wird die Wortlaut-Suche (FTS).")),
        "ohne Werk: Hinweis …: {html}"
    );
    assert!(html.contains("Treffer der Volltextsuche") && html.contains("id=\"einfall-"), "… und die Wortlaut-Suche");

    let message = fails(&server, "idea.search", json!({ "query": "Möwen", "mode": "rag" }), -32602).await;
    assert!(message.contains("Bedeutungssuche nur innerhalb eines Werks"), "ohne Werk -32602: {message}");
    fails(&server, "idea.search", json!({ "query": "Möwen", "mode": "rag", "work": "none" }), -32602).await;
    fails(&server, "idea.search", json!({ "query": "Möwen", "mode": "bild", "work": work }), -32602).await;
    fails(&server, "idea.search", json!({ "query": " ", "mode": "rag", "work": work }), -32602).await;
    // With a work the search asks; without an embedder (tests) LanceDB has honestly failed. Since Round 19c that is an
    // error -32001 (RAG_UNAVAILABLE) with a reason instead of an empty list.
    let message = fails(&server, "idea.search", json!({ "query": "Möwen", "mode": "rag", "work": work }), -32001).await;
    assert!(message.contains(NO_EMBEDDER), "Ausfall mit Grund: {message}");
    let fts = ok(&server, "idea.search", json!({ "query": "Möwen", "mode": "fts", "work": work })).await;
    assert_eq!(fts["hits"].as_array().unwrap().len(), 1, "mode fts wie bisher: {fts}");
}

/// GET as from a foreign page (`<img src>` or link): `Sec-Fetch-Site: cross-site`, Host 127.0.0.1.
async fn get_from(server: &common::Server, uri: &str, site: &str) -> (StatusCode, String) {
    let request = axum::http::Request::get(uri)
        .header("host", "127.0.0.1:8765")
        .header("sec-fetch-site", site)
        .body(axum::body::Body::empty())
        .unwrap();
    let (status, _, body) = server.send(request).await;
    (status, body)
}

#[tokio::test]
async fn a_foreign_page_cannot_start_the_meaning_search_over_get() {
    let server = common::open().await;
    let work = server.state.default_work().await.unwrap();
    let foreign = esc(baruch_server::literature::FOREIGN_PAGE);
    // Reason: without an embedder (tests) every search that actually ran names the failure of LanceDB (NO_EMBEDDER). If
    // this reason is missing and the hint about the foreign page is there, no search ran – hence no embedding, no
    // pgvector connection and no MultiGPT session either.
    for uri in [
        format!("/literatur?work={work}&lq=M%C3%B6wen&lmode=rag"),
        format!("/einfaelle?work={work}&q=M%C3%B6wen&mode=rag"),
    ] {
        let (status, html) = get_from(&server, &uri, "cross-site").await;
        assert_eq!(status, StatusCode::OK, "die Seite selbst ist lesbar ({uri})");
        assert!(html.contains(&foreign), "Hinweis auf die fremde Seite ({uri}): {html}");
        assert!(!html.contains(NO_EMBEDDER), "keine Suche gelaufen ({uri}): {html}");
        assert!(html.contains("data-fts-link"), "Link auf die Wortlaut-Suche bleibt ({uri})");

        let (_, html) = get_from(&server, &uri, "same-origin").await;
        assert!(html.contains(NO_EMBEDDER) && !html.contains(&foreign), "aus Baruch selbst wird gesucht ({uri})");
        let (_, html) = get_from(&server, &uri, "none").await;
        assert!(html.contains(NO_EMBEDDER), "selbst eingegebene Adresse (none) sucht ebenfalls ({uri})");
    }
    let (_, html) = get_from(&server, &format!("/literatur?work={work}&lq=M%C3%B6wen"), "cross-site").await;
    assert!(!html.contains(&foreign), "die Wortlaut-Suche (lokal, ohne Zugangsdaten) bleibt auch fremd erreichbar");
}
