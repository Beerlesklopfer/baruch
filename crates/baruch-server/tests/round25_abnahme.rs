// SPDX-License-Identifier: AGPL-3.0-or-later
//! Runde 25, Abnahme des Integrators: die Gedankenkarte von Ende zu Ende, nachdem karte-daten, karte-bild und
//! karte-widget zusammengeführt sind (Vertrag §7).
//!
//! Ein erfundenes Werk mit zwei Teilen, Kapiteln und einem Abschnitt bekommt 40 erfundene Einfälle (mit und ohne
//! Bezug, offen und verarbeitet, mit gleichen Schlagwörtern, ein Zwillingspaar mit gleichem Gedanken für die Nähe).
//! Einbettungen kommen vom Nachbau `MockEliezer` (512 Dimensionen) – nie vom echten Eliezer, kein Netz außer
//! 127.0.0.1:0. Geprüft wird über dieselben Wege wie Oberfläche, Widget und Assistenten: Seite ohne JavaScript,
//! Formulare an `/werke/{id}/karte/act`, die RPC, die das Widget ruft (`idea.update`, `idea.link`,
//! `idea.map_pos_set`), Spiegel `einfaelle.md` und MCP `idea_links`.
//!
//! Was die Zweige schon einzeln prüfen, steht dort (300 Einfälle < 200 ms und ohne Überlappung in
//! `round25_bild.rs`, Rundreise und Abgleich des Austauschs in `round25_widget.rs`); hier geht es um das
//! Zusammenspiel.

use crate::common;

use std::collections::BTreeSet;
use std::net::SocketAddr;
use std::process::Command;

use axum::body::{Body, to_bytes};
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode, header};
use baruch_llm::mock::MockEliezer;
use baruch_server::rag;
use serde_json::{Value, json};
use tower::ServiceExt;

/// Erfundenes Buch: Teil „Morgen“ mit „Tau“ und „Nebel“ (Abschnitt „Schwaden“), Teil „Abend“ mit „Dämmerung“
/// und „Sterne“ (bleibt ohne Einfälle).
fn book() -> String {
    format!(
        "#set document(title: \"Das Wetterheft\", author: \"Wenzel Wolke\")\n\n{}\n= Tau\n\nTropfen am Gras.\n\n\
         = Nebel\n\nGrau über dem Teich.\n\n== Schwaden\n\nZiehen langsam.\n\n{}\n= Dämmerung\n\nDas Licht geht.\n\n\
         = Sterne\n\nNoch keine Gedanken.\n",
        baruch_typst::part_line("Morgen"),
        baruch_typst::part_line("Abend")
    )
}

fn ok(answer: Value, what: &str) -> Value {
    assert!(answer["error"].is_null(), "{what} sollte gelingen: {answer}");
    answer["result"].clone()
}

fn key_of(nodes: &[Value], title: &str) -> i64 {
    nodes
        .iter()
        .find(|n| n["title"] == title)
        .and_then(|n| n["key"].as_i64())
        .unwrap_or_else(|| panic!("kein Punkt {title}"))
}

fn commits(server: &common::Server, slug: &str) -> Option<usize> {
    common::git(&server.mirror_dir_of(slug), &["rev-list", "--count", "HEAD"]).map(|n| n.trim().parse().unwrap())
}

/// Ids der gezeichneten Einfälle (`data-node="i:<id>"`).
fn drawn(html: &str) -> BTreeSet<i64> {
    html.split("data-node=\"i:").skip(1).map(|r| r[..r.find('"').unwrap()].parse().unwrap()).collect()
}

/// Das erste Tag, das `marker` enthält.
fn tag_with<'a>(html: &'a str, marker: &str) -> &'a str {
    let at = html.find(marker).unwrap_or_else(|| panic!("{marker} fehlt"));
    let start = html[..at].rfind('<').unwrap();
    &html[start..at + html[at..].find('>').unwrap() + 1]
}

fn xmllint(text: &str, name: &str) -> Option<(bool, String)> {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(name);
    std::fs::write(&path, text).unwrap();
    let output = Command::new("xmllint").arg("--noout").arg("--nonet").arg(&path).output().ok()?;
    Some((output.status.success(), String::from_utf8_lossy(&output.stderr).into_owned()))
}

async fn mcp(server: &common::Server, session: Option<&str>, message: Value) -> (axum::http::HeaderMap, Value) {
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

async fn mcp_tool(server: &common::Server, name: &str, arguments: Value) -> Value {
    let (headers, answer) = mcp(
        server,
        None,
        json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize",
                "params": { "protocolVersion": "2025-06-18", "capabilities": {},
                            "clientInfo": { "name": "abnahme-r25", "version": "1.0" } } }),
    )
    .await;
    assert!(answer["result"].is_object(), "initialize: {answer}");
    let session = headers.get("mcp-session-id").unwrap().to_str().unwrap().to_owned();
    let (_, answer) = mcp(
        server,
        Some(&session),
        json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": { "name": name, "arguments": arguments } }),
    )
    .await;
    assert_eq!(answer["result"]["isError"], false, "{name} sollte gelingen: {answer}");
    answer["result"]["structuredContent"].clone()
}

/// Ein Einfall des Plans: Bezug (Titel oder keiner), Schlagwörter, Gedanke, verarbeitet?
struct Planned {
    at: Option<&'static str>,
    words: String,
    note: String,
    done: bool,
}

/// 40 erfundene Einfälle: je 7 an „Tau“, „Nebel“, „Schwaden“ (Abschnitt), „Dämmerung“, 12 ohne Bezug; jeder
/// vierte verarbeitet; jeder fünfte trägt zusätzlich „Regen“ (Wort-Nähe). Gedanken aus Kunstwörtern je Einfall,
/// damit sich nur gewollte Paare nahe sind; Nr. 38 und 39 sind Zwillinge mit gleichem Gedanken (Vektor-Nähe ohne
/// gemeinsames Schlagwort).
fn plan() -> Vec<Planned> {
    let places: [(Option<&'static str>, usize); 5] =
        [(Some("Tau"), 7), (Some("Nebel"), 7), (Some("Schwaden"), 7), (Some("Dämmerung"), 7), (None, 12)];
    let mut out = Vec::new();
    for (at, count) in places {
        for _ in 0..count {
            let i = out.len();
            let mut words = format!("Wetter{i}");
            if i % 5 == 0 {
                words.push_str(", Regen");
            }
            out.push(Planned { at, words, note: format!("q{i}a q{i}b q{i}c"), done: i % 4 == 3 });
        }
    }
    let twin = "Ein Leuchtfeuer blinkt über der stillen Bucht und warnt jedes Boot vor den Klippen";
    out[38].words = "Leuchtfeuer".into();
    out[38].note = twin.into();
    out[39].words = "Nebelhorn".into();
    out[39].note = twin.into();
    out
}

#[tokio::test]
async fn forty_ideas_on_the_map_from_page_and_widget_to_mirror_and_mcp() {
    let server = common::open().await;
    let mock = MockEliezer::start().await;
    ok(server.rpc("connection.set", json!({ "kind": "eliezer", "config": { "url": mock.url() } })).await, "Nachbau");
    rag::set_embedders(&server.state, rag::eliezer_embedders(&server.state));
    let report = server.import(&book(), "wetterheft.typ").await;
    let (w, slug) = (report.work, report.slug.clone());
    let nodes =
        ok(server.rpc("work.get", json!({ "work": w })).await, "work.get")["work"]["nodes"].as_array().unwrap().clone();
    let (tau, nebel, schwaden, sterne) =
        (key_of(&nodes, "Tau"), key_of(&nodes, "Nebel"), key_of(&nodes, "Schwaden"), key_of(&nodes, "Sterne"));

    let plan = plan();
    let mut ids = Vec::new();
    for p in &plan {
        let mut params = json!({ "work": w, "words": p.words, "note": p.note });
        if let Some(title) = p.at {
            params["key"] = json!(key_of(&nodes, title));
        }
        let id = ok(server.rpc("idea.create", params).await, "idea.create")["idea"]["id"].as_i64().unwrap();
        if p.done {
            ok(server.rpc("idea.update", json!({ "id": id, "status": "done" })).await, "verarbeitet");
        }
        ids.push(id);
    }
    rag::catch_up(&server.state).await;
    rag::wait_idle(&server.state).await;
    let embedded = mock.embedding_inputs().len();
    assert!(embedded >= 40, "der Index hat alle 40 Einfälle über den Nachbau eingebettet: {embedded}");

    // --- Seite ohne JavaScript ------------------------------------------------------------------------------
    let (status, html) = server.get(&format!("/werke/{w}/karte")).await;
    assert_eq!(status, StatusCode::OK, "Karte lädt: {html}");
    assert_eq!(
        mock.embedding_inputs().len(),
        embedded,
        "Zeichnen fragt keine Einbettung an (nur gespeicherte Vektoren)"
    );
    for (needle, why) in [
        ("<form class=\"im-filter\" method=\"get\"", "Filter per GET"),
        // Runde 34: Die Gedankenkarte ist die Mindmap (Widget `mindmap`, Bild `mm-svg`).
        ("data-widget=\"mindmap\"", "Widget-Anker"),
        ("<svg class=\"mm-svg\"", "SVG steht im HTML"),
        ("<section class=\"im-contradictions\"", "Liste Widersprüche"),
    ] {
        assert!(html.contains(needle), "{why} fehlt auf der Karte");
    }
    // Runde 34: Die Formulare der alten Gliederungsliste stehen in der Gliederungsansicht (ohne JavaScript).
    assert!(html.contains(&format!("href=\"/werke/{w}/karte/gliederung\"")), "Verweis auf die Gliederungsansicht");
    let (status, outline) = server.get(&format!("/werke/{w}/karte/gliederung?wahl=i:{}", ids[0])).await;
    assert_eq!(status, StatusCode::OK, "Gliederungsansicht lädt");
    for (needle, why) in [
        ("name=\"method\" value=\"idea.update\"", "Formular „Bezug setzen“"),
        ("name=\"method\" value=\"idea.link\"", "Formular „Verbinden mit …“"),
    ] {
        assert!(outline.contains(needle), "{why} fehlt in der Gliederungsansicht");
    }
    assert!(!html.contains("idea_capture"), "die Einfall-Erfassung ist auf der Karte nicht eingebunden (Alt+E)");
    assert_eq!(drawn(&html), ids.iter().copied().collect(), "alle 40 Einfälle sind gezeichnet");
    assert!(
        !tag_with(&html, &format!("data-node=\"n:{sterne}\"")).contains("data-children"),
        "Kapitel ohne Einfälle ist ein Ast ohne Unterknoten"
    );
    assert!(
        tag_with(&html, &format!("data-node=\"i:{}\"", ids[14])).contains(&format!("data-parent=\"n:{nebel}\"")),
        "ein Einfall am Abschnitt hängt am Kapitel"
    );
    assert!(
        tag_with(&html, &format!("data-node=\"i:{}\"", ids[30])).contains("data-cloud=\"1\""),
        "ein Einfall ohne Bezug liegt in der Wolke"
    );
    assert!(html.contains("12 in der Wolke"), "die Wolke zählt die zwölf Einfälle ohne Bezug");

    // Nähe: Zwillinge über Vektoren (512 Dim. vom Nachbau), „Regen“ über Schlagwörter.
    let map = ok(server.rpc("idea.map", json!({ "work": w })).await, "idea.map");
    assert_eq!(map["near"]["state"], "ok", "Nähe mit Einbettungen verfügbar: {}", map["near"]);
    let pairs = map["near"]["pairs"].as_array().unwrap();
    let twin = pairs
        .iter()
        .find(|p| p["a"] == ids[38].min(ids[39]) && p["b"] == ids[38].max(ids[39]))
        .unwrap_or_else(|| panic!("Zwillingspaar fehlt in der Nähe: {}", map["near"]));
    assert_eq!(twin["via"], "vector", "ohne gemeinsames Schlagwort nur über Vektoren: {twin}");
    assert!(pairs.iter().any(|p| p["via"] != "vector"), "gleiche Schlagwörter zählen als Nähe mit: {}", map["near"]);
    let a = ids[38].min(ids[39]);
    let b = ids[38].max(ids[39]);
    // Runde 34: Nähe-Linien stehen zwischen Einfällen im Baum (hier „Regen“ an zwei Einfällen von „Tau“); für
    // Einfälle der Wolke steht statt einer Linie der Vorschlag „passt zu Ast …“ am Kärtchen.
    assert!(html.contains("<path class=\"mm-near\" data-from=\"i:"), "Nähe als eigene, gepunktete Linie im Baum");
    assert!(
        !html.contains(&format!("class=\"mm-near\" data-from=\"i:{a}\" data-to=\"i:{b}\"")),
        "die Zwillinge liegen beide in der Wolke: keine Linie quer durchs Bild"
    );
    let (_, no_near) = server.get(&format!("/werke/{w}/karte?near=0")).await;
    assert!(!no_near.contains("class=\"mm-near\""), "near=0 lässt die Nähe-Linien weg");
    // Verbindung von Wolke und Mindmap: Hängt ein Zwilling im Baum, bekommt der andere den Vorschlag.
    ok(server.rpc("idea.move", json!({ "id": a, "parent": null, "key": tau })).await, "Zwilling an „Tau“");
    let (_, hinted) = server.get(&format!("/werke/{w}/karte")).await;
    let card = tag_with(&hinted, &format!("data-node=\"i:{b}\""));
    assert!(
        card.contains("data-cloud=\"1\"") && card.contains("data-fits=\"n:"),
        "das Kärtchen der Wolke nennt den Ast, zu dem es passt: {card}"
    );
    assert!(hinted.contains("passt zu Ast „"), "in Worten am Kärtchen");
    ok(server.rpc("idea.move", json!({ "id": a, "cloud": true })).await, "Zwilling zurück in die Wolke");

    // Filter per GET.
    let open: BTreeSet<i64> = ids.iter().zip(&plan).filter(|(_, p)| !p.done).map(|(id, _)| *id).collect();
    let (_, filtered) = server.get(&format!("/werke/{w}/karte?status=open")).await;
    assert_eq!(drawn(&filtered), open, "status=open zeigt nur offene Einfälle");
    let (_, rain) = server.get(&format!("/werke/{w}/karte?word=Regen")).await;
    assert_eq!(drawn(&rain).len(), 8, "word=Regen zeigt die acht mit „Regen“");

    // --- Bezug setzen: Widget (RPC beim Ablegen auf ein Kapitel) und Formular, je ein Commit ------------------
    let start = commits(&server, &slug);
    ok(server.rpc("idea.update", json!({ "id": ids[30], "key": tau })).await, "Ziehen auf Kapitel");
    let (status, location) = server
        .post_form(&format!("/werke/{w}/karte/act"), &format!("method=idea.update&id={}&key={schwaden}", ids[31]))
        .await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Formular „Bezug setzen“ → 303");
    assert!(location.unwrap().starts_with(&format!("/werke/{w}/karte?")), "zurück auf die Karte");
    if let (Some(before), Some(after)) = (start, commits(&server, &slug)) {
        assert_eq!(after, before + 2, "zwei Bezüge, zwei Commits im Spiegel");
    }
    let (_, html) = server.get(&format!("/werke/{w}/karte")).await;
    assert!(
        tag_with(&html, &format!("data-node=\"i:{}\"", ids[30])).contains(&format!("data-parent=\"n:{tau}\"")),
        "gezogener Einfall hängt jetzt an „Tau“"
    );

    // --- Verbinden: Widget-Dialog (RPC) „widerspricht“ und Formular „führt weiter“ ---------------------------
    let (x, y) = (ids[2], ids[9]);
    let made = ok(
        server.rpc("idea.link", json!({ "from": y, "to": x, "kind": "contradicts", "note": "Tau oder Nebel" })).await,
        "Ablegen auf einen Einfall",
    );
    let link = made["link"]["id"].as_i64().unwrap();
    assert!(made["commit"].is_string(), "Verbindung mit Commit: {made}");
    let (status, _) = server
        .post_form(
            &format!("/werke/{w}/karte/act"),
            &format!("method=idea.link&from={}&to={}&kind=leads_to", ids[0], ids[1]),
        )
        .await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Formular „Verbinden mit …“ → 303");

    let (_, html) = server.get(&format!("/werke/{w}/karte")).await;
    let edge = tag_with(&html, &format!("data-link=\"{link}\""));
    assert!(
        edge.contains("mm-link-contradicts") && edge.contains(&format!("data-from=\"i:{x}\"")),
        "Kante normalisiert x<y: {edge}"
    );
    assert!(html.contains("mm-link-leads_to"), "auch die Formular-Verbindung ist gezeichnet");
    let list = &html[html.find("<section class=\"im-contradictions\"").unwrap()..];
    let list = &list[..list.find("</section>").unwrap()];
    assert!(
        list.contains(&format!("Einfall {x}</a>"))
            && list.contains(&format!("Einfall {y}</a>"))
            && list.contains("„Tau oder Nebel“"),
        "„widerspricht“ in der Liste Widersprüche: {list}"
    );
    let md = std::fs::read_to_string(server.mirror_dir_of(&slug).join("einfaelle.md")).unwrap();
    assert!(
        md.contains(&format!("- Verbindungen: ↔ {y} widerspricht („Tau oder Nebel“)")),
        "Spiegel einfaelle.md: {md}"
    );
    let links = mcp_tool(&server, "idea_links", json!({ "work": w })).await;
    assert!(
        links["links"].as_array().unwrap().iter().any(|l| l["id"] == link && l["kind"] == "contradicts"),
        "MCP idea_links kennt den Widerspruch: {links}"
    );

    // --- Lagen: speichern (Widget) und zurücksetzen (Formular), ohne Commit -----------------------------------
    let before = commits(&server, &slug);
    ok(server.rpc("idea.map_pos_set", json!({ "work": w, "idea": ids[5], "x": 777.5, "y": -12.0 })).await, "Lage");
    let (_, html) = server.get(&format!("/werke/{w}/karte")).await;
    // Runde 34: Lagen bleiben gespeichert (und im Austausch), wirken in der Mindmap aber nicht – der Baum ordnet
    // sich selbst.
    let stored = ok(server.rpc("idea.map", json!({ "work": w, "near": false })).await, "idea.map");
    assert_eq!(stored["positions"][0]["x"], 777.5, "die Lage ist gespeichert: {}", stored["positions"]);
    assert!(!html.contains("data-x=\"777.5\""), "im Bild der Mindmap hat sie keine Wirkung");
    server.post_form(&format!("/werke/{w}/karte/act"), "method=idea.map_pos_reset").await;
    let stored = ok(server.rpc("idea.map", json!({ "work": w, "near": false })).await, "idea.map");
    assert_eq!(stored["positions"].as_array().map(Vec::len), Some(0), "nach dem Zurücksetzen keine Lage mehr");
    assert_eq!(commits(&server, &slug), before, "Lagen sind Anzeige: kein Commit");

    // --- Gelöschter Einfall fällt samt Verbindung heraus und kommt zurück ------------------------------------
    ok(server.rpc("idea.delete", json!({ "id": x, "confirm": true })).await, "Einfall löschen");
    let (_, html) = server.get(&format!("/werke/{w}/karte")).await;
    assert!(!drawn(&html).contains(&x), "gelöschter Einfall ist nicht gezeichnet");
    assert!(!html.contains(&format!("data-link=\"{link}\"")), "seine Verbindung fällt aus der Karte");
    let list = &html[html.find("<section class=\"im-contradictions\"").unwrap()..];
    assert!(!list[..list.find("</section>").unwrap()].contains("„Tau oder Nebel“"), "und aus der Liste Widersprüche");
    let md = std::fs::read_to_string(server.mirror_dir_of(&slug).join("einfaelle.md")).unwrap();
    assert!(!md.contains("widerspricht"), "und aus dem Spiegel: {md}");
    ok(server.rpc("idea.restore", json!({ "id": x })).await, "Einfall wiederherstellen");
    let (_, html) = server.get(&format!("/werke/{w}/karte")).await;
    assert!(drawn(&html).contains(&x) && html.contains(&format!("data-link=\"{link}\"")), "mit Verbindung zurück");

    // --- Ausgaben ---------------------------------------------------------------------------------------------
    let (status, _, svg) =
        server.send(Request::get(format!("/werke/{w}/karte.svg")).body(Body::empty()).unwrap()).await;
    assert_eq!(status, StatusCode::OK, "SVG");
    assert!(svg.contains(&format!("data-link=\"{link}\"")), "SVG trägt die Verbindung");
    let (status, _, md) = server.send(Request::get(format!("/werke/{w}/karte.md")).body(Body::empty()).unwrap()).await;
    assert_eq!(status, StatusCode::OK, "Markdown");
    assert!(md.contains("## Widersprüche") && md.contains("Tau oder Nebel"), "Markdown nennt den Widerspruch: {md}");
    let (status, _, mm) = server.send(Request::get(format!("/werke/{w}/karte.mm")).body(Body::empty()).unwrap()).await;
    assert_eq!(status, StatusCode::OK, "FreeMind");
    for (text, name) in [(&svg, "karte.svg"), (&mm, "karte.mm")] {
        match xmllint(text, name) {
            Some((good, err)) => assert!(good, "xmllint hält {name} für wohlgeformt: {err}"),
            None => println!("ÜBERSPRUNGEN: xmllint fehlt ({name})"),
        }
    }
    {
        let _serial = common::PDF_SERIAL.lock().await;
        let response = server
            .app
            .clone()
            .oneshot(Request::get(format!("/werke/{w}/karte.pdf")).body(Body::empty()).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        assert_eq!(status, StatusCode::OK, "PDF entsteht");
        assert!(bytes.starts_with(b"%PDF-"), "echte PDF-Bytes");
    }
    assert!(mock.forbidden_paths().is_empty(), "der Nachbau sah keine verbotenen Pfade: {:?}", mock.forbidden_paths());
}

#[tokio::test]
async fn without_an_embedding_model_the_page_says_so_honestly() {
    let server = common::open().await;
    let w = server.import(&book(), "wetterheft.typ").await.work;
    // Runde 34: mit Bezug, damit die Einfälle im Baum hängen (Nähe-Linien gibt es nur dort, nicht in der Wolke).
    let nodes =
        ok(server.rpc("work.get", json!({ "work": w })).await, "work.get")["work"]["nodes"].as_array().unwrap().clone();
    let tau = key_of(&nodes, "Tau");
    for words in ["Regen, Wind", "regen", "Hagel"] {
        ok(server.rpc("idea.create", json!({ "work": w, "words": words, "key": tau })).await, "idea.create");
    }
    let map = ok(server.rpc("idea.map", json!({ "work": w })).await, "idea.map");
    assert_eq!(map["near"]["state"], "no_embedder", "ehrlich ohne Einbettungsmodell: {}", map["near"]);
    let (_, html) = server.get(&format!("/werke/{w}/karte")).await;
    let note = tag_with(&html, "im-near-note");
    assert!(note.contains("role=\"status\""), "Meldung als Status: {note}");
    let at = html.find("im-near-note").unwrap();
    let text = &html[at..at + html[at..].find("</p>").unwrap()];
    assert!(text.contains("Schlagw"), "die Seite nennt den Ersatz über Schlagwörter: {text}");
    assert!(html.contains("class=\"mm-near\""), "Wort-Nähe (Regen/regen) bleibt als Linie");
}
