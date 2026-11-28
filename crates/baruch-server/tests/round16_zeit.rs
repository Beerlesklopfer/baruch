// SPDX-License-Identifier: AGPL-3.0-or-later
//! Round 16b „Zeitwächter“: speaking time per outline item and in total (RPC `work.timing`, `work.timing_set`,
//! `timing.count`, MCP `work_timing`), form „Predigtzeit“ without JavaScript, display in outline and editor header
//! (only for a sermon or a target time) and the labelling local/external of the personas in the accordion
//! „Assistenten“.
//!
//! All texts are freely invented. No model call, no network request: the external connection in the test points to the
//! closed port 127.0.0.1:9 and is only displayed, never queried.

use crate::common;

use std::net::SocketAddr;

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode, header};
use baruch_typst::timing::spoken_words;
use serde_json::{Value, json};

/// Invented words for sections with a known word count.
fn words(n: usize) -> String {
    const POOL: [&str; 6] = ["Licht", "Weg", "Brot", "Wasser", "Stein", "Frieden"];
    (0..n).map(|i| POOL[i % POOL.len()]).collect::<Vec<_>>().join(" ")
}

/// Sermon with four items: Einleitung (30 words), Erster Gedanke (20) with Vertiefung (25), Schluss (15). The
/// introduction additionally contains markup, footnote and comment, which do not count.
fn sermon_source() -> String {
    format!(
        "= Einleitung\n\n{} #footnote[Quelle: erfunden] // Notiz am Rand\n\n= Erster Gedanke\n\n*{}*\n\n\
         == Vertiefung\n\n{}\n\n= Schluss\n\n{}\n",
        words(30),
        words(20),
        words(25),
        words(15)
    )
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

/// Imports the sermon and makes it the work kind sermon (`work.kind = 'sermon'`, like `WorkKind::Sermon`); so the test
/// does not depend on the form „Neues Werk“.
async fn sermon(server: &common::Server) -> i64 {
    let report = server.import(&sermon_source(), "zeitpredigt.typ").await;
    let work = report.work;
    sqlx::query("UPDATE work SET kind = 'sermon' WHERE id = ?")
        .bind(work)
        .execute(server.state.store.pool())
        .await
        .unwrap();
    work
}

/// Text without tags and with collapsed whitespace (for assertions about visible text).
fn visible(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut inside = false;
    for ch in html.chars() {
        match ch {
            '<' => inside = true,
            '>' => inside = false,
            c if !inside => out.push(c),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[tokio::test]
async fn work_timing_reports_every_point_with_its_own_words_marks_and_subtree() {
    let server = common::open().await;
    let work = sermon(&server).await;
    // 60 words per minute: seconds = words, so the times can be read directly from the word counts.
    let set = ok(&server, "work.timing_set", json!({ "work": work, "words_per_minute": 60 })).await;
    assert_eq!(set["words_per_minute"], 60, "die neue Geschwindigkeit wird zurückgemeldet: {set}");
    assert_eq!(set["target_seconds"], Value::Null, "ohne target_minutes bleibt es bei keiner Zielzeit: {set}");

    let timing = ok(&server, "work.timing", json!({ "work": work })).await;
    assert_eq!(timing["kind"], "sermon", "die Werkart steht in der Antwort: {timing}");
    assert_eq!(timing["shown"], true, "eine Predigt zeigt ihre Zeiten immer: {timing}");
    let sections = timing["sections"].as_array().unwrap();
    let field = |name: &str| sections.iter().map(|s| s[name].as_u64().unwrap()).collect::<Vec<_>>();
    assert_eq!(
        field("words"),
        [30, 20, 25, 15],
        "je Punkt nur der eigene Text ohne Überschrift, Fußnote, Kommentar und Sternchen"
    );
    assert_eq!(field("seconds"), [30, 20, 25, 15], "bei 60 Wörtern je Minute ist jedes Wort eine Sekunde");
    assert_eq!(field("start_seconds"), [0, 30, 50, 75], "Zeitmarke = Summe der Sekunden aller vorherigen Punkte");
    assert_eq!(
        field("subtree_seconds"),
        [30, 45, 25, 15],
        "der Erste Gedanke trägt seine Vertiefung mit (20 + 25), Blätter nur sich selbst"
    );
    assert_eq!(field("depth"), [0, 0, 1, 0], "die Vertiefung hat einen Vorfahren, die übrigen keinen");
    assert_eq!(field("seq"), [0, 1, 2, 3], "Punkte in seq-Reihenfolge");
    assert_eq!(sections[2]["title"], "Vertiefung", "Titel aus der Gliederung: {}", sections[2]);
    assert!(sections.iter().all(|s| s["key"].as_i64().unwrap() > 0), "jeder Punkt trägt seinen festen Schlüssel");
    assert_eq!(sections[1]["subtree_clock"], "0:45", "Uhrzeit des Teilbaums: {}", sections[1]);
    assert_eq!(sections[3]["start_clock"], "1:15", "Uhrzeit der Zeitmarke: {}", sections[3]);
    assert_eq!((timing["words"].as_u64(), timing["seconds"].as_u64()), (Some(90), Some(90)), "gesamt = Summe");
    assert_eq!(timing["clock"], "1:30", "Gesamtzeit als Uhrzeit");
    assert_eq!(timing["status"], "none", "ohne Zielzeit: status none");
    assert_eq!(timing["over_seconds"], Value::Null, "ohne Zielzeit keine Überschreitung");
    assert_eq!(timing["target_clock"], Value::Null, "ohne Zielzeit keine Zieluhrzeit");
}

#[tokio::test]
async fn a_target_time_turns_the_status_to_over_or_ok() {
    let server = common::open().await;
    let work = sermon(&server).await;
    let set =
        ok(&server, "work.timing_set", json!({ "work": work, "words_per_minute": 60, "target_minutes": 1 })).await;
    assert_eq!(set["target_seconds"], 60, "eine Minute Zielzeit wird als 60 Sekunden gespeichert: {set}");
    let over = ok(&server, "work.timing", json!({ "work": work })).await;
    assert_eq!(over["status"], "over", "90 s gegen 60 s Ziel: überschritten: {over}");
    assert_eq!(over["over_seconds"], 30, "um 30 Sekunden überschritten");
    assert_eq!((over["over_clock"].as_str(), over["target_clock"].as_str()), (Some("0:30"), Some("1:00")), "Uhrzeiten");

    ok(&server, "work.timing_set", json!({ "work": work, "target_minutes": 2 })).await;
    let fine = ok(&server, "work.timing", json!({ "work": work })).await;
    assert_eq!(fine["status"], "ok", "90 s gegen 120 s Ziel: im Rahmen: {fine}");
    assert_eq!(fine["words_per_minute"], 60, "nur die Zielzeit geändert, die Geschwindigkeit bleibt");
    assert_eq!(fine["over_seconds"], Value::Null, "keine Überschreitung im Rahmen");

    ok(&server, "work.timing_set", json!({ "work": work, "target_minutes": null })).await;
    let none = ok(&server, "work.timing", json!({ "work": work })).await;
    assert_eq!(none["status"], "none", "null löscht die Zielzeit wieder: {none}");
}

#[tokio::test]
async fn timing_set_refuses_values_outside_the_limits_and_changes_nothing() {
    let server = common::open().await;
    let work = sermon(&server).await;
    ok(&server, "work.timing_set", json!({ "work": work, "words_per_minute": 100, "target_minutes": 20 })).await;
    for params in [
        json!({ "work": work, "words_per_minute": 59 }),
        json!({ "work": work, "words_per_minute": 251 }),
        json!({ "work": work, "words_per_minute": null }),
        json!({ "work": work, "target_minutes": 0 }),
        json!({ "work": work, "target_minutes": 241 }),
        json!({ "work": work, "target_minutes": 1.5 }),
        json!({ "work": work, "target_minutes": "zwanzig" }),
        json!({ "words_per_minute": 120 }),
    ] {
        let error = err(&server, "work.timing_set", params.clone()).await;
        assert_eq!(error["code"], -32602, "ungültige Eingabe → -32602: {params} → {error}");
    }
    let unknown = err(&server, "work.timing_set", json!({ "work": 4711, "words_per_minute": 120 })).await;
    assert_eq!(unknown["code"], -32602, "unbekanntes Werk → -32602: {unknown}");
    let timing = ok(&server, "work.timing", json!({ "work": work })).await;
    assert_eq!(
        (timing["words_per_minute"].as_u64(), timing["target_seconds"].as_u64()),
        (Some(100), Some(1200)),
        "nach allen Ablehnungen gilt der alte Stand: {timing}"
    );
}

#[tokio::test]
async fn without_work_timing_uses_the_default_work_and_a_book_without_target_is_not_shown() {
    let server = common::open().await;
    let default = server.state.default_work().await.unwrap();
    let timing = ok(&server, "work.timing", json!({})).await;
    assert_eq!(timing["work"], default, "fehlt work, gilt das Vorgabewerk: {timing}");
    assert_eq!(timing["kind"], "book", "das Beispielwerk ist ein Buch");
    assert_eq!(timing["shown"], false, "ein Buch ohne Zielzeit zeigt keine Zeiten (RPC liefert sie trotzdem)");
    assert_eq!(timing["words_per_minute"], 110, "Vorgabe 110 Wörter je Minute ohne Einstellung");
    assert!(!timing["sections"].as_array().unwrap().is_empty(), "die Abschnitte stehen trotzdem in der Antwort");
}

#[tokio::test]
async fn a_new_sermon_from_work_create_shows_its_times() {
    let server = common::open().await;
    let made = ok(&server, "work.create", json!({ "title": "Zeit der Ernte", "kind": "sermon" })).await;
    let work = made["work"].as_i64().or_else(|| made["id"].as_i64()).expect("work.create nennt die Werk-id");
    let timing = ok(&server, "work.timing", json!({ "work": work })).await;
    assert_eq!((timing["kind"].as_str(), timing["shown"].as_bool()), (Some("sermon"), Some(true)), "Predigt: {timing}");
}

#[tokio::test]
async fn timing_count_counts_with_the_same_rule_as_the_server() {
    let server = common::open().await;
    let work = sermon(&server).await;
    ok(&server, "work.timing_set", json!({ "work": work, "words_per_minute": 120 })).await;
    let text = "= Gnade\n\nWie Joh 3,16 sagt: *so sehr* hat Gott#footnote[erfunden] geliebt. <marke> // Notiz\n\
                #set text(size: 11pt)\n$x$ `raw` - Punkt eins\n";
    let counted = ok(&server, "timing.count", json!({ "work": work, "text": text })).await;
    assert_eq!(
        counted["words"].as_u64(),
        Some(spoken_words(text) as u64),
        "timing.count zählt genau wie baruch_typst::timing::spoken_words: {counted}"
    );
    assert_eq!(counted["words"], 11, "„Wie Joh 3,16 sagt: so sehr hat Gott geliebt. Punkt eins“ = 11 Wörter");
    assert_eq!(counted["words_per_minute"], 120, "die Geschwindigkeit des Werks gilt: {counted}");
    assert_eq!(
        (counted["seconds"].as_u64(), counted["clock"].as_str()),
        (Some(6), Some("0:06")),
        "11 Wörter bei 120/min = 5,5 s, aufgerundet 6 s"
    );
    let long = "a".repeat(1_000_001);
    let error = err(&server, "timing.count", json!({ "work": work, "text": long })).await;
    assert_eq!(error["code"], -32602, "über 1 000 000 Zeichen → -32602: {error}");
    let missing = err(&server, "timing.count", json!({ "work": work })).await;
    assert_eq!(missing["code"], -32602, "ohne text → -32602");
}

#[tokio::test]
async fn the_form_saves_the_sermon_time_without_javascript_and_redirects_to_the_block() {
    let server = common::open().await;
    let work = sermon(&server).await;
    let (status, location) = server
        .post_form(
            "/einstellungen/werk/zeit",
            &format!("method=work.timing_set&work={work}&words_per_minute=95&target_minutes=20"),
        )
        .await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Formular ohne JavaScript: 303 zurück");
    assert_eq!(
        location.as_deref(),
        Some(format!("/einstellungen/werk?work={work}&ok=zeit#zeit").as_str()),
        "Rücksprung"
    );
    let values = ok(&server, "settings.get", json!({ "page": "werk", "work": work })).await;
    assert_eq!(
        values["timing"],
        json!({ "words_per_minute": 95, "target_seconds": 1200 }),
        "settings.get werk nennt die gespeicherte Predigtzeit: {values}"
    );

    let (status, location) = server
        .post_form("/einstellungen/werk/zeit", &format!("method=work.timing_set&work={work}&words_per_minute=300"))
        .await;
    assert_eq!(status, StatusCode::SEE_OTHER, "auch ein Fehler führt ohne JavaScript zurück");
    assert_eq!(
        location.as_deref(),
        Some(format!("/einstellungen/werk?work={work}&zeitfehler=1#zeit").as_str()),
        "Rücksprung mit Fehlermarke"
    );
    let values = ok(&server, "settings.get", json!({ "page": "werk", "work": work })).await;
    assert_eq!(values["timing"]["words_per_minute"], 95, "nach dem Fehler unverändert: {values}");

    let (status, location) = server
        .post_form("/einstellungen/werk/zeit", &format!("method=work.timing_set&work={work}&target_minutes="))
        .await;
    assert_eq!((status, location.is_some()), (StatusCode::SEE_OTHER, true), "leere Zielzeit wird angenommen");
    let values = ok(&server, "settings.get", json!({ "page": "werk", "work": work })).await;
    assert_eq!(values["timing"]["target_seconds"], Value::Null, "leere Zielzeit = keine Zielzeit: {values}");

    let (status, page) = server.get(&format!("/einstellungen/werk?work={work}&zeitfehler=1")).await;
    assert_eq!(status, StatusCode::OK, "Einstellungsseite Werk");
    assert!(page.contains("id=\"zeit\"") && page.contains("Predigtzeit"), "Block „Predigtzeit“ mit Anker #zeit");
    assert!(page.contains("action=\"/einstellungen/werk/zeit\""), "das Formular geht an die neue Route");
    assert!(page.contains("ließ sich nicht speichern"), "nach zeitfehler=1 steht die Meldung auf der Seite");
    assert!(page.contains("value=\"95\""), "das Feld zeigt die gespeicherten Wörter je Minute");
}

#[tokio::test]
async fn the_tree_and_the_editor_head_show_durations_and_the_overrun_without_javascript() {
    let server = common::open().await;
    let work = sermon(&server).await;
    ok(&server, "work.timing_set", json!({ "work": work, "words_per_minute": 60, "target_minutes": 1 })).await;
    let timing = ok(&server, "work.timing", json!({ "work": work })).await;
    let key = timing["sections"][0]["key"].as_i64().unwrap();
    let (status, page) = server.get(&format!("/?work={work}&key={key}")).await;
    assert_eq!(status, StatusCode::OK, "Arbeitsseite der Predigt");
    let text = visible(&page);
    assert!(text.contains("Gesamt ≈ 1:30 / Ziel 1:00"), "Kopf der Gliederung: Gesamtzeit und Zielzeit: {text}");
    assert!(
        page.contains("role=\"alert\">Zielzeit überschritten um 0:30</span>"),
        "die Überschreitung steht als Text mit role=alert, nicht nur als Farbe"
    );
    assert!(text.contains("≈ 0:45"), "der Erste Gedanke zeigt die Dauer samt Vertiefung (20 + 25 s)");
    assert!(text.contains("≈ 0:25"), "die Vertiefung zeigt ihre eigene Dauer");
    assert_eq!(page.matches("data-duration").count(), 4, "jeder der vier Punkte trägt seine Dauer");
    assert!(
        text.contains("Abschnitt ≈ 0:30 · gesamt 1:30 / Ziel 1:00"),
        "Editorkopf beim Laden vom Server: Abschnitt, gesamt, Ziel: {text}"
    );
    assert!(page.contains("data-widget=\"timing\""), "das Widget hängt sich für die Live-Zählung an");
    assert!(page.contains("data-rest=\"60\""), "Summe ohne die Einleitung (90 − 30) für die Live-Rechnung");

    // The same comes on redrawing through ui.render (with JavaScript).
    let panes = ok(&server, "ui.render", json!({ "work": work, "key": key })).await;
    assert!(panes["editor"].as_str().unwrap().contains("editor-timing"), "ui.render liefert den Editorkopf mit Zeit");
    assert!(panes["tree"].as_str().unwrap().contains("outline-timing"), "ui.render liefert den Baum mit Zeit");
}

#[tokio::test]
async fn a_book_shows_no_times_until_it_gets_a_target_but_rpc_and_mcp_always_report_them() {
    let server = common::open().await;
    let book = server.import(&sermon_source(), "zeitbuch.typ").await.work;
    let (_, page) = server.get(&format!("/?work={book}")).await;
    assert!(!page.contains("data-duration"), "Buch ohne Zielzeit: keine Dauer im Baum");
    assert!(!page.contains("outline-timing"), "Buch ohne Zielzeit: keine Gesamtzeit im Kopf der Gliederung");
    assert!(!page.contains("editor-timing"), "Buch ohne Zielzeit: keine Zeit im Editorkopf");
    let rpc = ok(&server, "work.timing", json!({ "work": book })).await;
    assert_eq!(rpc["sections"].as_array().unwrap().len(), 4, "RPC liefert die Zeiten trotzdem: {rpc}");

    ok(&server, "work.timing_set", json!({ "work": book, "target_minutes": 30 })).await;
    let (_, page) = server.get(&format!("/?work={book}")).await;
    assert!(page.contains("outline-timing"), "mit Zielzeit zeigt auch ein Buch die Gesamtzeit");
    assert!(page.contains("editor-timing"), "mit Zielzeit zeigt auch ein Buch die Zeit im Editorkopf");
    // Round 39a: the widget catalog in the page head carries every widget text; only the shown page counts.
    let shown = match page.split_once("id=\"baruch-i18n\">") {
        Some((head, rest)) => format!("{head}{}", &rest[rest.find("</script>").unwrap()..]),
        None => page.clone(),
    };
    assert!(!shown.contains("Zielzeit überschritten um"), "90 Wörter bei 110/min liegen unter 30 Minuten");

    let mcp = mcp_tool(&server, "work_timing", json!({ "work": book })).await;
    assert_eq!(mcp["result"]["isError"], false, "work_timing gelingt: {mcp}");
    let rpc = ok(&server, "work.timing", json!({ "work": book })).await;
    assert_eq!(mcp["result"]["structuredContent"], rpc, "MCP work_timing liefert dasselbe wie RPC work.timing");
}

/// Call an MCP tool from the own machine (without a session).
async fn mcp_tool(server: &common::Server, tool: &str, arguments: Value) -> Value {
    let body = json!({ "jsonrpc": "2.0", "id": 5, "method": "tools/call", "params": { "name": tool, "arguments": arguments } });
    let mut request = Request::post("/mcp")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    request.extensions_mut().insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 40000))));
    let (status, _, text) = server.send(request).await;
    assert_eq!(status, StatusCode::OK, "tools/call {tool}: {text}");
    serde_json::from_str(&text).unwrap()
}

#[tokio::test]
async fn mcp_work_timing_is_read_only_and_says_so() {
    let server = common::open().await;
    let body = json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" });
    let mut request = Request::post("/mcp")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    request.extensions_mut().insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 40000))));
    let (_, _, text) = server.send(request).await;
    let list: Value = serde_json::from_str(&text).unwrap();
    let tool = list["result"]["tools"].as_array().unwrap().iter().find(|t| t["name"] == "work_timing").cloned();
    let tool = tool.expect("work_timing steht in tools/list");
    assert!(tool["description"].as_str().unwrap().contains("read-only"), "die Beschreibung nennt „nur lesend“: {tool}");
    assert_eq!(tool["inputSchema"]["required"], json!(["work"]), "work ist Pflicht: {tool}");
    for setter in ["work_timing_set", "work.timing_set", "timing_set"] {
        let call = mcp_tool(&server, setter, json!({ "work": 1, "words_per_minute": 90 })).await;
        assert!(
            call["error"].is_object() || call["result"]["isError"] == true,
            "Einstellen geht über MCP nicht ({setter}): {call}"
        );
    }
    let unknown = mcp_tool(&server, "work_timing", json!({ "work": 4711 })).await;
    assert_eq!(unknown["result"]["isError"], true, "unbekanntes Werk: fachlicher Fehler: {unknown}");
}

#[tokio::test]
async fn timing_set_is_not_reachable_through_the_generic_act_form() {
    let server = common::open().await;
    let work = sermon(&server).await;
    let (status, _) = server.post_form("/act", &format!("method=work.timing_set&work={work}")).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "/act kennt work.timing_set nicht (nur das eigene Formular)");
    let timing = ok(&server, "work.timing", json!({ "work": work })).await;
    assert_eq!(timing["words_per_minute"], 110, "nichts geändert: Vorgabe bleibt");
}

#[tokio::test]
async fn the_assistants_accordion_marks_local_and_blocks_external_personas_without_release() {
    let server = common::open().await;
    let made = ok(
        &server,
        "model.connection_set",
        json!({ "name": "Fernrechner", "kind": "openai", "url": "http://127.0.0.1:9", "external": true }),
    )
    .await;
    let id = made["id"].as_i64().unwrap();
    ok(&server, "persona.backend_set", json!({ "persona": "@hoerer", "connection": id })).await;
    let list = ok(&server, "persona.list", json!({})).await;
    let hoerer = list["personas"].as_array().unwrap().iter().find(|p| p["handle"] == "@hoerer").unwrap().clone();
    assert_eq!(
        (hoerer["locality"].as_str(), hoerer["external_allowed"].as_bool()),
        (Some("extern"), Some(false)),
        "Ausgangslage aus persona.list: extern ohne Freigabe: {hoerer}"
    );

    let (_, page) = server.get("/").await;
    let start = page.find("data-section=\"personas\"").expect("Akkordeon Assistenten");
    let end = page[start..].find("</details>").map(|e| start + e).unwrap();
    let accordion = &page[start..end];
    let item = |handle: &str| -> &str {
        let at = accordion.find(&format!(">{handle}<")).unwrap_or_else(|| panic!("{handle} fehlt"));
        let from = accordion[..at].rfind("<li").unwrap();
        let to = accordion[at..].find("</li>").map(|e| at + e).unwrap();
        &accordion[from..to]
    };
    let hoerer_html = item("@hoerer");
    assert!(hoerer_html.contains("data-locality=\"extern\""), "Hörer ist als extern markiert: {hoerer_html}");
    assert!(hoerer_html.contains("Gesperrt"), "extern ohne Freigabe ist sichtbar gesperrt: {hoerer_html}");
    assert!(hoerer_html.contains("/einstellungen/assistenten"), "Hinweis führt zu Einstellungen → Assistenten");
    let lektor_html = item("@lektor");
    assert!(lektor_html.contains("data-locality=\"lokal\""), "der Lektor bleibt lokal: {lektor_html}");
    assert!(!lektor_html.contains("Gesperrt"), "lokale Personas sind nicht gesperrt");
    let select = accordion.find("name=\"persona\"").map(|at| &accordion[at..]).unwrap_or("");
    let select = &select[..select.find("</select>").unwrap_or(select.len())];
    assert!(!select.contains("value=\"@hoerer\""), "die gesperrte Persona steht nicht zur Wahl: {select}");
    assert!(select.contains("value=\"@lektor\""), "die lokale Persona steht zur Wahl");

    ok(&server, "persona.backend_set", json!({ "persona": "@hoerer", "connection": id, "external_allowed": true }))
        .await;
    let (_, page) = server.get("/").await;
    assert!(page.contains("data-locality=\"extern\""), "nach der Freigabe weiter als extern gekennzeichnet");
    assert!(!page.contains("data-persona-blocked"), "nach der Freigabe nicht mehr gesperrt");
}
