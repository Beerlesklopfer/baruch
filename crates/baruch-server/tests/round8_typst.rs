// SPDX-License-Identifier: AGPL-3.0-or-later
//! Typst-Vorschau mit Werkvorlage (Vertrag Runde 8, Abschnitt 4): RPC `preview.typst`, Seiten
//! `/werke/{id}/satz` und `/werke/{id}/satz/{stand}/{n}.svg`, MCP `typst_check`. Echter
//! Datenordner im Temp-Verzeichnis, frei erfundene Werke unter `tests/data/round8/`.
//!
//! Der Zwischenspeicher ist statisch und gilt für alle Tests dieser Datei: Jeder Test hängt darum
//! eine eigene Kommentarzeile an sein Werk, damit sein Stand nur ihm gehört.

use crate::common;

use std::net::SocketAddr;
use std::time::Duration;

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode, header};
use baruch_core::Work;
use serde_json::{Value, json};

const BOOK: &str = include_str!("data/round8/laubenbuch.typ");
const BROKEN: &str = include_str!("data/round8/laube-fehler.typ");
const LAUBE_SVG: &str = "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"40\" height=\"20\" viewBox=\"0 0 40 20\">\
<rect width=\"40\" height=\"20\" fill=\"#3a7\"/><circle cx=\"10\" cy=\"10\" r=\"6\" fill=\"#fd4\"/></svg>";
const LOOPBACK: ([u8; 4], u16) = ([127, 0, 0, 1], 40000);

/// Werk aus `source` mit eigener Schlusszeile `tag` anlegen; mit `image` das Bild der Laube.
async fn import(server: &common::Server, source: &str, tag: &str, image: bool) -> (i64, Work) {
    let report = server.import(&format!("{source}// Test: {tag}\n"), &format!("{tag}.typ")).await;
    if image {
        let added = server
            .rpc(
                "asset.add",
                json!({ "work": report.work, "file_name": "laube.svg", "content": LAUBE_SVG, "encoding": "svg",
                        "path": "./bilder/laube.svg", "alt": "Die Laube" }),
            )
            .await;
        assert!(added["result"].is_object(), "Bild muss sich anlegen lassen: {added}");
    }
    let work = server.state.store.load_work(report.work).await.unwrap();
    (report.work, work)
}

fn node<'a>(work: &'a Work, title: &str) -> &'a baruch_core::Node {
    work.nodes.iter().find(|n| n.title == title).unwrap_or_else(|| panic!("Abschnitt {title} fehlt"))
}

fn result(answer: &Value) -> &Value {
    assert!(answer["error"].is_null(), "RPC sollte gelingen: {answer}");
    &answer["result"]
}

#[tokio::test]
async fn preview_typst_reports_pages_the_chapter_page_and_diagnostics_with_section() {
    let server = common::open().await;
    let (work, loaded) = import(&server, BOOK, "rpc-kapitel", true).await;
    let ofen = node(&loaded, "Am Ofen");
    let winter = node(&loaded, "Der Winter");
    assert_eq!(ofen.number, "2.1", "Voraussetzung: „Am Ofen“ ist Abschnitt 2.1");

    let answer = server.rpc("preview.typst", json!({ "work": work, "key": ofen.key })).await;
    let r = result(&answer);
    assert_eq!(r["pages"], 3, "drei Kapitel, jedes beginnt auf einer neuen Seite: {r}");
    assert_eq!(r["page"], 2, "„2.1 Am Ofen“ steht im zweiten Kapitel auf Seite 2");
    assert_eq!(r["chapter"], json!({ "first": 2, "last": 2 }), "das Kapitel „Der Winter“ ist Seite 2");
    assert_eq!(r["stand"].as_str().map(str::len), Some(64), "der Stand ist ein SHA-256 als Hex");
    assert_eq!(r["cached"], false, "erster Lauf für diesen Stand");
    assert!(r["millis"].is_u64(), "die Dauer des Laufs steht in millis");
    assert!(r.get("svg").is_none(), "ohne page kein SVG in der Antwort");

    let diagnostics = r["diagnostics"].as_array().unwrap();
    let template = diagnostics
        .iter()
        .find(|d| d["message"].as_str().unwrap().contains("vorlagenschrift fehlt"))
        .unwrap_or_else(|| panic!("Warnung der Vorlagenschrift fehlt: {diagnostics:?}"));
    assert_eq!(template["severity"], "warning", "eine fehlende Schrift ist eine Warnung");
    assert_eq!(template["section"], "Werkvorlage", "Zeile 1 gehört der Werkvorlage");
    assert_eq!(template["key"], Value::Null, "die Werkvorlage hat keinen Abschnitt");
    assert_eq!(template["section_line"], 1, "Zeile in der Vorlage");

    let oven = diagnostics
        .iter()
        .find(|d| d["message"].as_str().unwrap().contains("ofenschrift fehlt"))
        .unwrap_or_else(|| panic!("Warnung der Ofenschrift fehlt: {diagnostics:?}"));
    assert_eq!(oven["key"], ofen.key, "die Warnung gehört zum Abschnitt „Am Ofen“ (fester Schlüssel)");
    assert_eq!(oven["section"], "2.1 Am Ofen", "Abschnitt mit Nummer und Titel");
    assert_eq!(oven["section_line"], 2, "zweite Zeile des Abschnitts (nach der Überschrift)");
    assert_eq!(oven["line"], 12, "Zeile im ganzen Export");
    assert!(oven["column"].as_u64().is_some_and(|c| c >= 1), "Spalte 1-basiert: {oven}");

    let chapter = result(&server.rpc("preview.typst", json!({ "work": work, "key": winter.key })).await).clone();
    assert_eq!(chapter["page"], 2, "das Kapitel selbst beginnt auf Seite 2");
    let none = result(&server.rpc("preview.typst", json!({ "work": work })).await).clone();
    assert_eq!(none["page"], 1, "ohne key Seite 1");
    assert_eq!(none["chapter"], json!({ "first": 1, "last": 3 }), "ohne key das ganze Werk");
}

#[tokio::test]
async fn a_second_call_comes_from_the_cache_and_page_returns_the_svg() {
    let server = common::open().await;
    let (work, _) = import(&server, BOOK, "rpc-zwischenspeicher", true).await;
    let first = result(&server.rpc("preview.typst", json!({ "work": work })).await).clone();
    assert_eq!(first["cached"], false, "erster Lauf setzt");
    let second = result(&server.rpc("preview.typst", json!({ "work": work, "page": 2 })).await).clone();
    assert_eq!(second["cached"], true, "gleicher Stand: aus dem Zwischenspeicher");
    assert_eq!(second["stand"], first["stand"], "unveränderte Quelle, gleicher Stand");
    assert_eq!(second["page"], 2, "mit page gilt die verlangte Seite");
    let svg = second["svg"].as_str().expect("mit page steht das SVG in der Antwort");
    assert!(svg.starts_with("<svg") && svg.contains("data:image/svg+xml"), "Seite 2 mit eingebettetem Bild");
    assert!(baruch_server::typeset::cache_len() <= baruch_server::typeset::CACHE_SIZE, "höchstens 8 Stände");

    let out = server.rpc("preview.typst", json!({ "work": work, "page": 9 })).await;
    assert_eq!(out["error"]["code"], -32602, "eine Seite hinter dem Ende ist ungültig: {out}");
    let unknown = server.rpc("preview.typst", json!({ "work": work, "key": 999_999 })).await;
    assert_eq!(unknown["error"]["code"], -32602, "unbekannter Schlüssel ist ungültig: {unknown}");
}

#[tokio::test]
async fn the_time_limit_answers_refused() {
    let server = common::open().await;
    let (work, _) = import(&server, BOOK, "zeitlimit", true).await;
    let refused = baruch_server::typeset::typeset_with_limit(&server.state, work, Duration::ZERO).await;
    let error = refused.err().expect("mit Zeitlimit 0 darf kein Ergebnis kommen");
    assert_eq!(error.code, baruch_server::rpc::REFUSED, "Zeitüberschreitung ist ein fachlicher Fehler (REFUSED)");
    assert!(error.message.starts_with("Typst brauchte länger als"), "verständliche Meldung: {}", error.message);
    // Ob der abgelaufene Lauf schon begonnen hatte, hängt davon ab, ob ein paralleler Test gerade die
    // Sperre hielt; dass er zu Ende rechnet und den Zwischenspeicher füllt, prüft darum der
    // Unit-Test `a_timed_out_run_finishes_in_the_background_and_fills_the_cache` in `typeset.rs`.
    let later = baruch_server::typeset::typeset(&server.state, work).await.expect("mit 60 s gelingt der Satz");
    assert_eq!(later.page_count(), 3, "nach der Zeitüberschreitung setzt der nächste Aufruf normal");
}

#[tokio::test]
async fn errors_point_to_their_section_and_there_are_no_pages() {
    let server = common::open().await;
    let (work, loaded) = import(&server, BROKEN, "fehler", false).await;
    let broken = node(&loaded, "Kaputt");
    let r = result(&server.rpc("preview.typst", json!({ "work": work })).await).clone();
    assert_eq!(r["pages"], 0, "ein Fehler bricht den Satz ab");
    let error = r["diagnostics"].as_array().unwrap().iter().find(|d| d["severity"] == "error").cloned();
    let error = error.unwrap_or_else(|| panic!("Fehler fehlt: {r}"));
    assert_eq!(error["key"], broken.key, "der Fehler gehört zu „Kaputt“");
    assert_eq!(error["section"], format!("{} Kaputt", broken.number), "Abschnitt mit Nummer und Titel");
    assert_eq!(error["section_line"], 3, "dritte Zeile von „Kaputt“");

    let (status, html) = server.get(&format!("/werke/{work}/satz")).await;
    assert_eq!(status, StatusCode::OK, "die Seite zeigt auch einen Fehler: {html}");
    assert!(html.contains(">Fehler</span>"), "die Meldung ist als Fehler gekennzeichnet: {html}");
    assert!(
        html.contains(&format!("href=\"/?work={work}&amp;key={}\"", broken.key)),
        "Link zum Abschnitt in Key-Form: {html}"
    );
    assert!(!html.contains("<img"), "ohne Seiten kein Bild: {html}");
}

#[tokio::test]
async fn the_page_shows_the_chapter_with_navigation_and_links_without_javascript() {
    let server = common::open().await;
    let (work, loaded) = import(&server, BOOK, "seite", true).await;
    let ofen = node(&loaded, "Am Ofen");
    let r = result(&server.rpc("preview.typst", json!({ "work": work })).await).clone();
    let stand = r["stand"].as_str().unwrap();

    let (status, html) = server.get(&format!("/werke/{work}/satz?key={}", ofen.key)).await;
    assert_eq!(status, StatusCode::OK, "Satz-Seite: {html}");
    assert!(html.contains("<title>Satz – "), "deutscher Seitentitel: {html}");
    assert!(
        html.contains(&format!("<img src=\"/werke/{work}/satz/{stand}/2.svg\" alt=\"Seite 2 von 3\">")),
        "die Kapitelseite 2 groß als Bild: {html}"
    );
    let page = |n: usize| format!("href=\"/werke/{work}/satz?key={}&amp;page={n}\"", ofen.key);
    assert!(html.contains(&format!("{} rel=\"prev\"", page(1))), "‹ führt auf Seite 1: {html}");
    assert!(html.contains(&format!("{} rel=\"next\"", page(3))), "› führt auf Seite 3: {html}");
    assert!(html.contains(&format!("aria-current=\"page\" {}", page(2))), "Seitenliste des Kapitels mit Seite 2");
    assert!(html.contains("Werkvorlage"), "Warnung der Vorlage mit Abschnitt „Werkvorlage“: {html}");
    assert!(
        html.contains(&format!("<a href=\"/?work={work}&amp;key={}\">2.1 Am Ofen</a>", ofen.key)),
        "Warnung im Abschnitt mit Link /?work=W&key=K: {html}"
    );
    assert!(html.contains(&format!("href=\"/?work={work}&amp;key={}\"", ofen.key)), "Zurück zum Werk am Abschnitt");
    assert!(!html.contains("{{") && !html.contains("{%"), "keine Reste von Platzhaltern: {html}");

    let (status, html) = server.get(&format!("/werke/{work}/satz?key={}&page=3", ofen.key)).await;
    assert_eq!(status, StatusCode::OK, "Blättern ohne JavaScript");
    assert!(html.contains("/3.svg\" alt=\"Seite 3 von 3\""), "Seite 3 gezeigt: {html}");
    // Seit Runde 18a steht › im Menüband, gesperrt mit aria-label (Bildschirmleser lesen sonst nur „›“).
    assert!(
        html.contains(
            "<span class=\"btn\" aria-disabled=\"true\" aria-label=\"Nächste Seite (dies ist die letzte)\">›</span>"
        ),
        "auf der letzten Seite kein ›, sondern ein gesperrter Knopf mit Beschriftung: {html}"
    );

    let (status, _) = server.get("/werke/999999/satz").await;
    assert_eq!(status, StatusCode::NOT_FOUND, "unbekanntes Werk → 404");
}

#[tokio::test]
async fn svg_pages_are_served_safely_and_only_for_the_current_stand() {
    let server = common::open().await;
    let (work, loaded) = import(&server, BOOK, "svg", true).await;
    let (other, _) = import(&server, BROKEN, "svg-fremd", false).await;
    let stand =
        result(&server.rpc("preview.typst", json!({ "work": work })).await)["stand"].as_str().unwrap().to_owned();
    let other_stand =
        result(&server.rpc("preview.typst", json!({ "work": other })).await)["stand"].as_str().unwrap().to_owned();

    let request = Request::get(format!("/werke/{work}/satz/{stand}/2.svg")).body(Body::empty()).unwrap();
    let (status, headers, body) = server.send(request).await;
    assert_eq!(status, StatusCode::OK, "Seite 2 als SVG: {body}");
    assert_eq!(headers[header::CONTENT_TYPE], "image/svg+xml", "richtiger MIME-Typ");
    assert_eq!(headers[header::X_CONTENT_TYPE_OPTIONS], "nosniff", "kein Raten des Typs");
    assert_eq!(
        headers[header::CONTENT_SECURITY_POLICY],
        "sandbox; default-src 'none'; img-src data:; style-src 'unsafe-inline'; frame-ancestors 'none'",
        "CSP mit sandbox: nichts ausführen, nichts nachladen; frame-ancestors (no_framing): kein fremder Rahmen"
    );
    assert!(body.starts_with("<svg") && body.contains("data:image/svg+xml"), "SVG mit eingebettetem Bild");

    for (uri, why) in [
        (format!("/werke/{work}/satz/{stand}/4.svg"), "Seite hinter dem Ende"),
        (format!("/werke/{work}/satz/{stand}/0.svg"), "Seite 0 gibt es nicht"),
        (format!("/werke/{work}/satz/{stand}/2.png"), "nur .svg"),
        (format!("/werke/{work}/satz/{}/1.svg", "0".repeat(64)), "fremder Stand"),
        (format!("/werke/{work}/satz/{other_stand}/1.svg"), "Stand eines anderen Werks"),
        (format!("/werke/{work}/satz/abc/1.svg"), "kein Stand"),
        (format!("/werke/999999/satz/{stand}/1.svg"), "unbekanntes Werk"),
    ] {
        let (status, _) = server.get(&uri).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{why} → 404 ({uri})");
    }

    // Nach dem Speichern ist der alte Stand veraltet.
    let winter = node(&loaded, "Der Frühling");
    let body = "= Der Frühling\nDie Äpfel blühen schon.\n";
    server.state.store.save_human(work, winter.id, body, "Mensch").await.unwrap();
    let (status, _) = server.get(&format!("/werke/{work}/satz/{stand}/1.svg")).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "alter Stand nach dem Speichern → 404");
}

async fn mcp(server: &common::Server, method: &str, params: Value) -> Value {
    let message = json!({ "jsonrpc": "2.0", "id": 8, "method": method, "params": params });
    let mut request = Request::post("/mcp")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(message.to_string()))
        .unwrap();
    request.extensions_mut().insert(ConnectInfo(SocketAddr::from(LOOPBACK)));
    let (status, _, body) = server.send(request).await;
    assert_eq!(status, StatusCode::OK, "MCP {method}: {body}");
    serde_json::from_str(&body).unwrap()
}

#[tokio::test]
async fn typst_check_reports_errors_warnings_and_url_and_writes_nothing() {
    let server = common::open().await;
    let (work, loaded) = import(&server, BOOK, "mcp", true).await;
    let ofen = node(&loaded, "Am Ofen");
    let slug = server.state.store.work_slug(work).await.unwrap();
    let commits = |dir: std::path::PathBuf| common::git(&dir, &["rev-list", "--count", "HEAD"]);
    let before = commits(server.mirror_dir_of(&slug));

    let list = mcp(&server, "tools/list", json!({})).await;
    let names: Vec<&str> =
        list["result"]["tools"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap()).collect();
    let at = names.iter().position(|n| *n == "typst_check").expect("typst_check steht in tools/list");
    assert_eq!(names[at - 1], "search", "typst_check folgt direkt auf search: {names:?}");
    let tool = &list["result"]["tools"][at];
    assert!(tool["description"].as_str().unwrap().contains("read-only"), "Beschreibung sagt „nur lesend“");

    let answer =
        mcp(&server, "tools/call", json!({ "name": "typst_check", "arguments": { "work": work, "key": ofen.key } }))
            .await;
    assert_eq!(answer["result"]["isError"], false, "typst_check gelingt: {answer}");
    let r = &answer["result"]["structuredContent"];
    assert_eq!(r["ok"], true, "Warnungen sind keine Fehler");
    assert_eq!(r["pages"], 3, "drei Seiten");
    assert_eq!(r["errors"], 0, "keine Fehler");
    assert_eq!(r["warnings"], 2, "zwei fehlende Schriften");
    assert_eq!(r["url"], format!("/werke/{work}/satz?key={}", ofen.key), "url der Satz-Seite am Kapitel");
    assert_eq!(r["stand"].as_str().map(str::len), Some(64), "Stand als Hex");
    let sections: Vec<&str> =
        r["diagnostics"].as_array().unwrap().iter().filter_map(|d| d["section"].as_str()).collect();
    assert!(sections.contains(&"2.1 Am Ofen") && sections.contains(&"Werkvorlage"), "Abschnitte: {sections:?}");

    let by_slug = mcp(&server, "tools/call", json!({ "name": "typst_check", "arguments": { "work": slug } })).await;
    assert_eq!(by_slug["result"]["structuredContent"]["url"], format!("/werke/{work}/satz"), "ohne key: url ohne key");
    let bad =
        mcp(&server, "tools/call", json!({ "name": "typst_check", "arguments": { "work": work, "key": 999_999 } }))
            .await;
    assert_eq!(bad["result"]["isError"], true, "unbekannter Schlüssel ist ein Werkzeugfehler: {bad}");

    assert_eq!(commits(server.mirror_dir_of(&slug)), before, "typst_check schreibt keinen Commit");
}

#[tokio::test]
async fn the_rule_for_unnumbered_headings_compiles_under_every_scheme() {
    // Backlog „nicht mit typst compile geprüft“: die Show-Regel `numbering: none` für Knoten mit
    // `numbered = 0` (hier „Die Laube“ als Vorwort) zusammen mit jedem Gliederungsschema.
    let server = common::open().await;
    let (work, loaded) = import(&server, BOOK, "ohne-nummer", true).await;
    let preface = node(&loaded, "Die Laube");
    sqlx::query("UPDATE node SET numbered = 0 WHERE id = ?")
        .bind(preface.key)
        .execute(server.state.store.pool())
        .await
        .unwrap();
    for scheme in baruch_store::NumberingScheme::ALL {
        let custom = (scheme == baruch_store::NumberingScheme::Custom).then_some("I.;A.;1.");
        match server.state.store.set_numbering_scheme(work, scheme.id(), custom).await {
            // „aus der Werkvorlage“ ist nach dem Import schon eingestellt.
            Ok(_) | Err(baruch_store::StoreError::Refused(_)) => {}
            Err(e) => panic!("Schema {} lässt sich nicht setzen: {e}", scheme.id()),
        }
        let exported = server.state.store.export_typst(work).await.unwrap();
        let typeset = baruch_server::typeset::typeset(&server.state, work).await.expect("Satz gelingt");
        let errors: Vec<_> = typeset.diagnostics().into_iter().filter(|d| d.severity == "error").collect();
        assert!(errors.is_empty(), "Schema {}: Typst meldet Fehler {errors:?}\n{exported}", scheme.id());
        assert_eq!(typeset.page_count(), 3, "Schema {}: drei Kapitel, drei Seiten", scheme.id());
        // Seit Runde 14 (Vertrag §4) projiziert der Export die erste Überschriftszeile auf die Markierung
        // „ohne Nummer“ statt einer Show-Regel, sobald die wirksame Nummerierung nicht aus ist.
        if scheme != baruch_store::NumberingScheme::None {
            assert!(
                exported.contains("#heading(level: 1, numbering: none)[Die Laube] <baruch-unnumbered>"),
                "Schema {}: Markierung „ohne Nummer“ im Export",
                scheme.id()
            );
            assert!(!exported.contains("Überschriften ohne Nummer"), "Schema {}: keine Show-Regel mehr", scheme.id());
        }
    }
}
