// SPDX-License-Identifier: AGPL-3.0-or-later
//! Round 25 „karte-daten“: links, positions, proximity and filters of the idea map over RPC and MCP (contract
//! §4.1/§4.2). Tests are written by karte-daten.
//!
//! - `idea.map` with 40 invented ideas: nodes, anchors, counters, unfolding, filters status/word/origin, `q`.
//! - Links over RPC: exactly one mirror commit per change with trailers, line in `einfaelle.md`; same pair without
//!   commit; positions without commit; -32602 for wrong parameters.
//! - MCP `idea_map`/`idea_links`/`idea_link`/`idea_unlink` with origin persona or `@mcp:<client>`.
//! - A deleted idea drops out of map, links and positions and comes back when restored.
//! - Proximity from stored vectors (own mock embedder, never Eliezer) without an embedding request when drawing;
//!   without an embedder honestly `no_embedder` with keyword proximity.
//!
//! No network: no request to Eliezer, MultiGPT or the internet. Texts freely invented.

use crate::common;

use std::collections::{BTreeMap, HashSet};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode, header};
use baruch_core::NodeKind;
use baruch_rag::{BoxFuture, Embedder, EmbedderFactory, Embeddings, RagError};
use baruch_server::rag;
use baruch_store::{IdeaOrigin, NewIdea};
use serde_json::{Value, json};

/// Invented book: two parts, chapters, one section with a subsection.
fn book() -> String {
    format!(
        "#set document(title: \"Das Laubenbuch\", author: \"Erna Efeu\")\n\n{}\n= Säen\n\nSamen in Reihen.\n\n\
         == Beete\n\nGerade Linien.\n\n=== Ränder\n\nMit Buchs.\n\n= Gießen\n\nMorgens und abends.\n\n{}\n= Ernten\n\n\
         Körbe voll.\n",
        baruch_typst::part_line("Frühling"),
        baruch_typst::part_line("Herbst")
    )
}

async fn ok(server: &common::Server, method: &str, params: Value) -> Value {
    let answer = server.rpc(method, params).await;
    assert!(answer["error"].is_null(), "{method} sollte gelingen: {answer}");
    answer["result"].clone()
}

/// Error code of an RPC answer (must fail).
async fn code(server: &common::Server, method: &str, params: Value) -> i64 {
    let answer = server.rpc(method, params).await;
    answer["error"]["code"].as_i64().unwrap_or_else(|| panic!("{method} sollte scheitern: {answer}"))
}

/// Stable keys by title.
async fn keys(server: &common::Server, work: i64) -> BTreeMap<String, (i64, NodeKind)> {
    let loaded = server.state.store.load_work(work).await.unwrap();
    loaded.nodes.iter().map(|n| (n.title.clone(), (n.key, n.kind))).collect()
}

fn origin_of(i: usize) -> IdeaOrigin {
    match i % 3 {
        0 => IdeaOrigin::Human { name: "Autor".into() },
        1 => IdeaOrigin::Persona { handle: "@lektor".into() },
        _ => IdeaOrigin::Mcp { handle: "@mcp:probe".into() },
    }
}

/// One invented idea of the plan: (reference as title or None, keywords, status open?, origin).
struct Planned {
    at: Option<&'static str>,
    words: Vec<String>,
    open: bool,
    origin: IdeaOrigin,
}

/// 40 invented ideas: 8 at the chapter „Säen“, 6 at the section „Beete“, 4 at the subsection „Ränder“, 8 at „Gießen“, 6
/// at „Ernten“, 8 without reference; every fourth processed, origin in rotation, every fifth with „Regen“.
fn plan() -> Vec<Planned> {
    let spread: [(Option<&'static str>, usize, &str); 6] = [
        (Some("Säen"), 8, "Saat"),
        (Some("Beete"), 6, "Beet"),
        (Some("Ränder"), 4, "Buchs"),
        (Some("Gießen"), 8, "Kanne"),
        (Some("Ernten"), 6, "Korb"),
        (None, 8, "Wolke"),
    ];
    let mut out = Vec::new();
    for (at, n, topic) in spread {
        for _ in 0..n {
            let i = out.len();
            let mut words = vec![topic.to_owned(), format!("Gedanke {i}")];
            if i % 5 == 0 {
                words.push("Regen".into());
            }
            out.push(Planned { at, words, open: i % 4 != 3, origin: origin_of(i) });
        }
    }
    assert_eq!(out.len(), 40, "Abnahme: 40 erfundene Einfälle");
    out
}

/// Work with the book and the 40 ideas; (work, slug, ids in plan order).
async fn forty(server: &common::Server) -> (i64, String, Vec<i64>, Vec<Planned>) {
    let report = server.import(&book(), "laube.typ").await;
    let work = report.work;
    let keys = keys(server, work).await;
    let plan = plan();
    let mut ids = Vec::new();
    for (i, p) in plan.iter().enumerate() {
        let key = p.at.map(|t| keys[t].0);
        let new = NewIdea {
            work: Some(work),
            key,
            words: &p.words,
            note: &format!("Notiz {i}: {} im Garten", p.words[0]),
            quote: "",
            source: "",
            bible: "",
            origin: p.origin.clone(),
        };
        let idea = server.state.store.idea_create(&new).await.unwrap();
        if !p.open {
            server
                .state
                .store
                .idea_update(
                    idea.id,
                    &baruch_store::IdeaEdit { status: Some(baruch_store::IdeaStatus::Done), ..Default::default() },
                )
                .await
                .unwrap();
        }
        ids.push(idea.id);
    }
    (work, report.slug, ids, plan)
}

fn node<'a>(map: &'a Value, id: &str) -> &'a Value {
    map["nodes"].as_array().unwrap().iter().find(|n| n["id"] == id).unwrap_or_else(|| panic!("Knoten {id}: {map}"))
}

fn idea_ids(map: &Value) -> Vec<i64> {
    map["ideas"].as_array().unwrap().iter().map(|i| i["id"].as_i64().unwrap()).collect()
}

fn commits(dir: &std::path::Path) -> Option<usize> {
    common::git(dir, &["rev-list", "--count", "HEAD"]).map(|s| s.trim().parse().unwrap())
}

// ---- Map -----------------------------------------------------------------------------------------------

#[tokio::test]
async fn the_map_of_forty_ideas_has_nodes_anchors_and_counters() {
    let server = common::open().await;
    let (work, _, ids, plan) = forty(&server).await;
    let keys = keys(&server, work).await;
    let map = ok(&server, "idea.map", json!({ "work": work, "near": false })).await;
    assert_eq!(map["work"], work, "Werk der Karte");
    assert_eq!(map["title"], "Das Laubenbuch", "Titel");
    let w = node(&map, "w");
    assert_eq!((w["kind"].as_str(), w["parent"].is_null()), (Some("work"), true), "Mitte: Werk ohne Elternknoten");
    assert_eq!(w["total_ideas"], 40, "das Werk zählt alle gezeigten Einfälle");
    let open_total = plan.iter().filter(|p| p.open).count();
    assert_eq!(w["open_ideas"], open_total, "offene beim Werk");

    // Parts and chapters as nodes; sections below a chapter are merged into the chapter.
    let part = node(&map, &format!("n:{}", keys["Frühling"].0));
    assert_eq!((part["kind"].as_str(), part["parent"].as_str()), (Some("part"), Some("w")), "Teil hängt am Werk");
    let sow = node(&map, &format!("n:{}", keys["Säen"].0));
    assert_eq!(sow["kind"], "chapter", "Kapitel");
    assert_eq!(sow["parent"], part["id"], "Kapitel hängt am Teil");
    let sections: Vec<&str> =
        sow["sections"].as_array().unwrap().iter().map(|s| s["title"].as_str().unwrap()).collect();
    assert_eq!(sections, ["Beete", "Ränder"], "Abschnitt und Unterabschnitt zusammengefasst");
    let nodes: Vec<&str> = map["nodes"].as_array().unwrap().iter().map(|n| n["id"].as_str().unwrap()).collect();
    assert!(!nodes.contains(&format!("n:{}", keys["Beete"].0).as_str()), "zugeklappt kein eigener Knoten: {nodes:?}");
    // Counters: Säen carries its own 8 and the 6 + 4 from the merged sections.
    assert_eq!(sow["total_ideas"], 18, "8 + 6 + 4 an „Säen“: {sow}");
    let open_sow = plan.iter().filter(|p| matches!(p.at, Some("Säen" | "Beete" | "Ränder")) && p.open).count();
    assert_eq!(sow["open_ideas"], open_sow, "offene an „Säen“");
    let beete = sow["sections"].as_array().unwrap().iter().find(|s| s["title"] == "Beete").unwrap();
    assert_eq!(beete["ideas"], 6, "Abschnitt zählt genau seine Einfälle");
    assert_eq!(node(&map, &format!("n:{}", keys["Ernten"].0))["total_ideas"], 6, "Ernten");
    // A part counts the ideas of its chapters as well (otherwise it would look empty, finding Round 25): Frühling =
    // Säen (18) + Gießen (8), Herbst = Ernten (6).
    assert_eq!(part["total_ideas"], 26, "Frühling samt Kapiteln: {part}");
    let open_spring =
        plan.iter().filter(|p| matches!(p.at, Some("Säen" | "Beete" | "Ränder" | "Gießen")) && p.open).count();
    assert_eq!(part["open_ideas"], open_spring, "offene im Teil Frühling");
    assert_eq!(node(&map, &format!("n:{}", keys["Herbst"].0))["total_ideas"], 6, "Herbst samt Ernten");
    let none = node(&map, "none");
    assert_eq!((none["kind"].as_str(), none["title"].as_str()), (Some("no_chapter"), Some("Ohne Kapitel")), "Wolke");
    assert_eq!(none["total_ideas"], 8, "acht ohne Bezug");

    // Ideas: anchor = chapter (also for sections), key = exact reference, label from the keywords.
    assert_eq!(idea_ids(&map), ids, "alle 40, nach id");
    let edge = &map["ideas"][15]; // plan 15 lies in „Ränder“
    assert_eq!(edge["anchor"], sow["id"], "Unterabschnitt hängt zugeklappt am Kapitel: {edge}");
    assert_eq!(edge["key"], keys["Ränder"].0, "genauer Bezug bleibt: {edge}");
    assert_eq!(edge["node"], format!("i:{}", ids[15]), "Knoten-id des Einfalls");
    assert_eq!(edge["label"], plan[15].words.join(", "), "Beschriftung „a, b“");
    assert_eq!(map["ideas"][39]["anchor"], "none", "ohne Bezug an der Wolke");
    assert!(map["ideas"].as_array().unwrap().iter().all(|i| i["hit"] == true), "ohne q alle Treffer");
    assert_eq!(map["filter"]["near_limit"], 3, "angewandter Filter mit Vorgabe near_limit");
    assert!((map["filter"]["near_threshold"].as_f64().unwrap() - 0.75).abs() < 1e-6, "Vorgabe near_threshold");
    assert_eq!(map["near"]["state"], "off", "near=false: aus");

    // Unfolding: open=[Säen] turns sections into nodes of their own; Ränder hangs at Beete.
    let open = ok(&server, "idea.map", json!({ "work": work, "near": false, "open": [keys["Säen"].0, 999_999] })).await;
    let beete = node(&open, &format!("n:{}", keys["Beete"].0));
    assert_eq!((beete["kind"].as_str(), beete["parent"].clone()), (Some("section"), sow["id"].clone()), "{beete}");
    assert_eq!(beete["total_ideas"], 10, "aufgeklappt zählt der Abschnitt samt Unterabschnitt Ränder: 6 + 4");
    let edges = node(&open, &format!("n:{}", keys["Ränder"].0));
    assert_eq!(edges["parent"], beete["id"], "Unterabschnitt hängt am Abschnitt");
    assert_eq!(edges["total_ideas"], 4, "Ränder zählt nur seine vier");
    // The number at the chapter does not change when unfolding: it counts the ideas of its sections as well.
    assert_eq!(node(&open, sow["id"].as_str().unwrap())["total_ideas"], 18, "Kapitel samt aufgeklappten Abschnitten");
    assert_eq!(node(&open, &format!("n:{}", keys["Frühling"].0))["total_ideas"], part["total_ideas"], "Teil gleich");
    assert!(
        node(&open, sow["id"].as_str().unwrap())["sections"].as_array().unwrap().is_empty(),
        "nichts mehr zusammengefasst"
    );
    assert_eq!(open["ideas"][15]["anchor"], edges["id"], "Einfall am aufgeklappten Unterabschnitt");
    assert_eq!(open["filter"]["open"], json!([keys["Säen"].0]), "angewandt nur Kapitel des Werks");
}

#[tokio::test]
async fn filters_remove_ideas_and_q_marks_hits() {
    let server = common::open().await;
    let (work, _, ids, plan) = forty(&server).await;
    let done = ok(&server, "idea.map", json!({ "work": work, "status": "done", "near": false })).await;
    let want: Vec<i64> = ids.iter().zip(&plan).filter(|(_, p)| !p.open).map(|(id, _)| *id).collect();
    assert_eq!(idea_ids(&done), want, "status=done: nur verarbeitete");
    assert_eq!(node(&done, "w")["total_ideas"], want.len(), "Zähler folgen dem Filter");
    let open = ok(&server, "idea.map", json!({ "work": work, "status": "open", "near": false })).await;
    assert_eq!(idea_ids(&open).len() + want.len(), 40, "open und done ergänzen sich");
    let all = ok(&server, "idea.map", json!({ "work": work, "status": "all", "near": false })).await;
    assert_eq!(idea_ids(&all).len(), 40, "all = alle");

    let rain = ok(&server, "idea.map", json!({ "work": work, "word": "  regen ", "near": false })).await;
    let want: Vec<i64> = ids.iter().enumerate().filter(|(i, _)| i % 5 == 0).map(|(_, id)| *id).collect();
    assert_eq!(idea_ids(&rain), want, "word über word_key (Groß/klein, Leerraum egal)");
    let persona = ok(&server, "idea.map", json!({ "work": work, "origin": "persona", "near": false })).await;
    let want: Vec<i64> = ids.iter().enumerate().filter(|(i, _)| i % 3 == 1).map(|(_, id)| *id).collect();
    assert_eq!(idea_ids(&persona), want, "origin=persona");
    assert!(
        persona["ideas"].as_array().unwrap().iter().all(|i| i["origin"]["kind"] == "persona"),
        "nur Persona-Herkunft"
    );
    // Without ideas without reference the cloud „Ohne Kapitel“ is dropped.
    let saat = ok(&server, "idea.map", json!({ "work": work, "word": "Saat", "near": false })).await;
    assert!(saat["nodes"].as_array().unwrap().iter().all(|n| n["id"] != "none"), "keine Wolke ohne passende Einfälle");

    let q = ok(&server, "idea.map", json!({ "work": work, "q": "Kanne", "near": false })).await;
    assert_eq!(idea_ids(&q).len(), 40, "q entfernt nichts");
    let hits: Vec<i64> =
        q["ideas"].as_array().unwrap().iter().filter(|i| i["hit"] == true).map(|i| i["id"].as_i64().unwrap()).collect();
    let want: Vec<i64> = ids.iter().zip(&plan).filter(|(_, p)| p.words[0] == "Kanne").map(|(id, _)| *id).collect();
    assert_eq!(hits, want, "q markiert genau die Treffer der Volltextsuche (FTS wie idea.search)");
    assert_eq!(q["filter"]["q"], "Kanne", "angewandt");

    for (params, why) in [
        (json!({ "work": work, "status": "halb" }), "status falsch"),
        (json!({ "work": work, "origin": "robot" }), "origin falsch"),
        (json!({ "work": work, "near_limit": 9 }), "near_limit über 5"),
        (json!({ "work": work, "near_threshold": 1.5 }), "near_threshold über 1"),
        (json!({ "work": work, "near": "vielleicht" }), "near kein Wahrheitswert"),
        (json!({}), "work fehlt"),
    ] {
        assert_eq!(code(&server, "idea.map", params).await, -32602, "{why}: -32602");
    }
}

// ---- Links and positions over RPC ----------------------------------------------------------------------

#[tokio::test]
async fn every_link_change_is_one_commit_with_trailers_and_shows_in_einfaelle_md() {
    let server = common::open().await;
    let (work, slug, ids, _) = forty(&server).await;
    let mirror = server.mirror_dir_of(&slug);
    let (a, b, c) = (ids[0], ids[1], ids[2]);
    let before = commits(&mirror);

    let made =
        ok(&server, "idea.link", json!({ "from": c, "to": a, "kind": "contradicts", "note": "beißt sich" })).await;
    let link = &made["link"];
    assert_eq!((link["from"].as_i64(), link["to"].as_i64()), (Some(a), Some(c)), "widerspricht normalisiert: {made}");
    assert_eq!(made["changed"], true, "neu");
    assert!(made["commit"].is_string(), "ein Commit: {made}");
    assert_eq!(link["origin"], json!({ "kind": "human", "name": "Autor" }), "Herkunft über RPC immer der Mensch");
    assert_eq!(link["kind"], "contradicts", "Art als Wort");
    assert!(link["uuid"].as_str().unwrap().len() == 36 && link["deleted"].is_null(), "uuid, aktiv: {link}");
    let id = link["id"].as_i64().unwrap();
    if let Some(log) = common::git(&mirror, &["log", "-1", "--format=%s%n%b"]) {
        assert!(
            log.starts_with(&format!("Verbindung {id}: Einfall {a} widerspricht Einfall {c} (angelegt)")),
            "Zusammenfassung nach Vertrag: {log}"
        );
        for trailer in ["Baruch-Action: idea-link", &format!("Baruch-Idea-Link: {id}"), "Baruch-Origin: human"] {
            assert!(log.contains(trailer), "Trailer {trailer}: {log}");
        }
    }
    let lines = std::fs::read_to_string(mirror.join("einfaelle.md")).unwrap();
    assert!(lines.contains(&format!("- Verbindungen: ↔ {c} widerspricht („beißt sich“)\n")), "einfaelle.md: {lines}");

    let led = ok(&server, "idea.link", json!({ "from": a, "to": b, "kind": "leads_to" })).await;
    let lines = std::fs::read_to_string(mirror.join("einfaelle.md")).unwrap();
    assert!(
        lines.contains(&format!("- Verbindungen: ↔ {c} widerspricht („beißt sich“); → {b} führt weiter\n")),
        "beide Verbindungen bei a, nach id: {lines}"
    );
    assert!(lines.contains(&format!("- Verbindungen: ← {a} führt weiter\n")), "eingehend bei b: {lines}");

    // Same pair and kind once more: unchanged, no commit.
    let after_two = commits(&mirror);
    let same = ok(&server, "idea.link", json!({ "from": a, "to": c, "kind": "contradicts", "note": "neu" })).await;
    assert_eq!(same["changed"], false, "aktiv → unverändert");
    assert!(same.get("commit").is_none() && same.get("mirror").is_none(), "kein Commit: {same}");
    assert_eq!(commits(&mirror), after_two, "Zahl der Commits unverändert");

    let gone = ok(&server, "idea.unlink", json!({ "id": id })).await;
    assert!(gone["link"]["deleted"].is_string() && gone["commit"].is_string(), "weich gelöscht mit Commit: {gone}");
    if let Some(log) = common::git(&mirror, &["log", "-1", "--format=%s%n%b"]) {
        assert!(log.contains("(gelöscht)") && log.contains("Baruch-Action: idea-unlink"), "{log}");
    }
    assert_eq!(code(&server, "idea.unlink", json!({ "id": id })).await, -32602, "zweimal lösen: -32602");
    let back = ok(&server, "idea.link_restore", json!({ "id": id })).await;
    assert!(back["link"]["deleted"].is_null() && back["commit"].is_string(), "zurück mit Commit: {back}");
    if let Some(log) = common::git(&mirror, &["log", "-1", "--format=%s%n%b"]) {
        assert!(log.contains("(wiederhergestellt)") && log.contains("Baruch-Action: idea-link-restore"), "{log}");
    }
    // A repeated idea.link on a deleted link restores it as well.
    ok(&server, "idea.unlink", json!({ "id": led["link"]["id"] })).await;
    let relinked = ok(&server, "idea.link", json!({ "from": a, "to": b, "kind": "leads_to", "note": "doch" })).await;
    assert_eq!(
        (relinked["changed"].as_bool(), relinked["link"]["id"].clone()),
        (Some(true), led["link"]["id"].clone()),
        "gelöschte Verbindung kommt als dieselbe Zeile zurück (changed true, gleiche id): {relinked}"
    );
    assert_eq!(relinked["link"]["note"], "doch", "Notiz ersetzt");
    if let Some(log) = common::git(&mirror, &["log", "-1", "--format=%s%n%b"]) {
        assert!(log.contains("Baruch-Action: idea-link-restore"), "Wiederherstellen über idea.link: {log}");
    }
    if let (Some(start), Some(end)) = (before, commits(&mirror)) {
        assert_eq!(end - start, 6, "sechs Änderungen, sechs Commits (das unveränderte ohne)");
    }

    let by_work = ok(&server, "idea.links", json!({ "work": work })).await;
    assert_eq!(by_work["links"].as_array().unwrap().len(), 2, "zwei aktive im Werk: {by_work}");
    let by_idea = ok(&server, "idea.links", json!({ "idea": b, "deleted": true })).await;
    assert_eq!(by_idea["links"].as_array().unwrap().len(), 1, "eine bei b");
    let map = ok(&server, "idea.map", json!({ "work": work, "near": false })).await;
    assert_eq!(map["links"].as_array().unwrap().len(), 2, "Karte zeigt die aktiven Verbindungen: {}", map["links"]);

    for (method, params, why) in [
        ("idea.links", json!({ "work": work, "idea": a }), "work und idea zugleich"),
        ("idea.links", json!({}), "weder work noch idea"),
        ("idea.link", json!({ "from": a, "to": b, "kind": "widerspricht" }), "Anzeige statt Art"),
        ("idea.link", json!({ "from": a, "to": a, "kind": "leads_to" }), "Selbstbezug"),
        ("idea.link", json!({ "from": a, "kind": "leads_to" }), "to fehlt"),
        ("idea.link", json!({ "from": a, "to": 999_999, "kind": "leads_to" }), "unbekannter Einfall"),
        ("idea.link", json!({ "from": a, "to": b, "kind": "leads_to", "note": "x".repeat(501) }), "Notiz zu lang"),
        ("idea.unlink", json!({ "id": "sieben" }), "id keine Zahl"),
        ("idea.link_restore", json!({ "id": 999_999 }), "unbekannte Verbindung"),
    ] {
        assert_eq!(code(&server, method, params).await, -32602, "{why}: -32602");
    }
}

#[tokio::test]
async fn positions_are_saved_and_reset_without_any_commit() {
    let server = common::open().await;
    let (work, slug, ids, _) = forty(&server).await;
    let mirror = server.mirror_dir_of(&slug);
    let before = commits(&mirror);
    let set = ok(&server, "idea.map_pos_set", json!({ "work": work, "idea": ids[3], "x": 120.5, "y": "-40" })).await;
    assert_eq!(set["pos"]["idea"], ids[3], "Lage des Einfalls: {set}");
    assert_eq!((set["pos"]["x"].as_f64(), set["pos"]["y"].as_f64()), (Some(120.5), Some(-40.0)), "Koordinaten");
    assert!(set.get("commit").is_none(), "Lagen sind Anzeige: kein Commit");
    ok(&server, "idea.map_pos_set", json!({ "work": work, "idea": ids[4], "x": 0, "y": 0 })).await;
    let map = ok(&server, "idea.map", json!({ "work": work, "near": false })).await;
    assert_eq!(map["positions"].as_array().unwrap().len(), 2, "Karte trägt die Lagen: {}", map["positions"]);
    let one = ok(&server, "idea.map_pos_reset", json!({ "work": work, "idea": ids[3] })).await;
    assert_eq!(one["removed"], 1, "eine entfernt");
    let all = ok(&server, "idea.map_pos_reset", json!({ "work": work })).await;
    assert_eq!(all["removed"], 1, "die übrige");
    assert_eq!(commits(&mirror), before, "keine Commits für Lagen");
    for (params, why) in [
        (json!({ "work": work, "idea": ids[3], "x": 1 }), "y fehlt"),
        (json!({ "work": work, "idea": ids[3], "x": 2_000_000, "y": 0 }), "x über der Grenze"),
        (json!({ "work": work, "idea": ids[3], "x": "NaN", "y": 0 }), "x keine endliche Zahl"),
    ] {
        assert_eq!(code(&server, "idea.map_pos_set", params).await, -32602, "{why}: -32602");
    }
}

#[tokio::test]
async fn a_deleted_idea_leaves_map_links_and_positions_and_comes_back() {
    let server = common::open().await;
    let (work, _, ids, _) = forty(&server).await;
    let (a, b) = (ids[5], ids[6]);
    ok(&server, "idea.link", json!({ "from": a, "to": b, "kind": "example_of" })).await;
    ok(&server, "idea.map_pos_set", json!({ "work": work, "idea": b, "x": 5, "y": 5 })).await;
    ok(&server, "idea.delete", json!({ "id": b, "confirm": true })).await;
    let map = ok(&server, "idea.map", json!({ "work": work, "near": false })).await;
    assert!(!idea_ids(&map).contains(&b), "gelöschter Einfall nicht auf der Karte");
    assert!(map["links"].as_array().unwrap().is_empty(), "seine Verbindung fällt heraus");
    assert!(map["positions"].as_array().unwrap().is_empty(), "seine Lage fällt heraus");
    assert_eq!(node(&map, "w")["total_ideas"], 39, "Zähler ohne ihn");
    ok(&server, "idea.restore", json!({ "id": b })).await;
    let map = ok(&server, "idea.map", json!({ "work": work, "near": false })).await;
    assert!(idea_ids(&map).contains(&b), "zurück auf der Karte");
    assert_eq!(map["links"].as_array().unwrap().len(), 1, "Verbindung zurück");
    assert_eq!(map["positions"].as_array().unwrap().len(), 1, "Lage zurück");
}

// ---- MCP -----------------------------------------------------------------------------------------------

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

async fn initialize(server: &common::Server, name: &str) -> String {
    let (headers, answer) = mcp(
        server,
        None,
        json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize",
                "params": { "protocolVersion": "2025-06-18", "capabilities": {},
                            "clientInfo": { "name": name, "version": "1.0" } } }),
    )
    .await;
    assert!(answer["result"].is_object(), "initialize: {answer}");
    headers.get("mcp-session-id").unwrap().to_str().unwrap().to_owned()
}

async fn tool(server: &common::Server, session: &str, name: &str, arguments: Value) -> Value {
    mcp(
        server,
        Some(session),
        json!({ "jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": { "name": name, "arguments": arguments } }),
    )
    .await
    .1
}

async fn tool_ok(server: &common::Server, session: &str, name: &str, arguments: Value) -> Value {
    let answer = tool(server, session, name, arguments).await;
    assert_eq!(answer["result"]["isError"], false, "{name} sollte gelingen: {answer}");
    answer["result"]["structuredContent"].clone()
}

#[tokio::test]
async fn mcp_links_carry_persona_or_client_origin_and_the_map_carries_urls() {
    let server = common::open().await;
    let (work, slug, ids, _) = forty(&server).await;
    let mirror = server.mirror_dir_of(&slug);
    let s = initialize(&server, "karten-probe").await;

    let (_, listed) = mcp(&server, Some(&s), json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" })).await;
    for name in ["idea_map", "idea_links", "idea_link", "idea_unlink"] {
        let t = listed["result"]["tools"].as_array().unwrap().iter().find(|t| t["name"] == name).unwrap();
        let text = t["description"].as_str().unwrap();
        assert!(!text.contains("noch nicht umgesetzt") && !text.starts_with("STUB"), "{name} ist kein Stub: {text}");
    }

    let map = tool_ok(&server, &s, "idea_map", json!({ "work": slug, "status": "open", "near": false })).await;
    assert_eq!(map["work"], work, "Werk über slug");
    let first = &map["ideas"][0];
    assert_eq!(first["url"], format!("/einfaelle/{}", first["id"]), "je Einfall url: {first}");
    assert!(map["ideas"].as_array().unwrap().iter().all(|i| i["status"] == "open"), "Filter status gilt");

    let by_persona = tool_ok(
        &server,
        &s,
        "idea_link",
        json!({ "from": ids[7], "to": ids[8], "kind": "belongs_to", "persona": "@lektor" }),
    )
    .await;
    assert_eq!(by_persona["link"]["origin"], json!({ "kind": "persona", "handle": "@lektor" }), "{by_persona}");
    assert!(by_persona["commit"].is_string(), "Commit: {by_persona}");
    if let Some(log) = common::git(&mirror, &["log", "-1", "--format=%an%n%B"]) {
        for part in ["(@lektor)", "Baruch-Origin: persona", "Persona: @lektor", "Baruch-Action: idea-link"] {
            assert!(log.contains(part), "{part}: {log}");
        }
    }
    let by_client =
        tool_ok(&server, &s, "idea_link", json!({ "from": ids[9], "to": ids[8], "kind": "leads_to", "note": "dann" }))
            .await;
    assert_eq!(by_client["link"]["origin"], json!({ "kind": "mcp", "handle": "@mcp:karten-probe" }), "{by_client}");
    if let Some(log) = common::git(&mirror, &["log", "-1", "--format=%an <%ae>%n%B"]) {
        assert!(log.starts_with("MCP-Client (@mcp:karten-probe) <mcp-karten-probe@persona.baruch.invalid>"), "{log}");
        assert!(log.contains("Baruch-Origin: mcp") && log.contains("Persona: @mcp:karten-probe"), "{log}");
    }

    let links = tool_ok(&server, &s, "idea_links", json!({ "id": ids[8] })).await;
    assert_eq!(links["links"].as_array().unwrap().len(), 2, "zwei am Einfall: {links}");
    // Anybody may remove any link (like ideas): the client removes the one of the persona.
    let gone = tool_ok(&server, &s, "idea_unlink", json!({ "id": by_persona["link"]["id"] })).await;
    assert!(gone["link"]["deleted"].is_string() && gone["commit"].is_string(), "gelöst mit Commit: {gone}");
    if let Some(log) = common::git(&mirror, &["log", "-1", "--format=%B"]) {
        assert!(log.contains("Baruch-Action: idea-unlink") && log.contains("Baruch-Origin: mcp"), "{log}");
    }
    let rest = tool_ok(&server, &s, "idea_links", json!({ "work": work })).await;
    assert_eq!(rest["links"].as_array().unwrap().len(), 1, "eine aktiv im Werk");
    let all = tool_ok(&server, &s, "idea_links", json!({ "work": work, "deleted": true })).await;
    assert_eq!(all["links"].as_array().unwrap().len(), 2, "mit deleted beide");

    // Domain rejections are isError, wrong arguments -32602.
    let unknown = tool(
        &server,
        &s,
        "idea_link",
        json!({ "from": ids[7], "to": ids[8], "kind": "leads_to", "persona": "@niemand" }),
    )
    .await;
    assert_eq!(unknown["result"]["isError"], true, "unbekannte Persona: {unknown}");
    let selfie = tool(&server, &s, "idea_link", json!({ "from": ids[7], "to": ids[7], "kind": "leads_to" })).await;
    assert_eq!(selfie["result"]["isError"], true, "Selbstbezug fachlich abgelehnt: {selfie}");
    for (name, arguments, why) in [
        ("idea_link", json!({ "from": ids[7], "to": ids[8], "kind": "widerspricht" }), "falsche Art"),
        ("idea_links", json!({ "work": work, "id": ids[8] }), "work und id zugleich"),
        ("idea_map", json!({ "status": "open" }), "work fehlt"),
        ("idea_map", json!({ "work": work, "origin": "robot" }), "origin falsch"),
        ("idea_unlink", json!({}), "id fehlt"),
    ] {
        let answer = tool(&server, &s, name, arguments).await;
        assert_eq!(answer["error"]["code"], -32602, "{why}: -32602: {answer}");
    }
}

// ---- Proximity -----------------------------------------------------------------------------------------

/// Concept axes of the mock vector space (as in `round12.rs`): words of the same axis are „bedeutungsgleich“.
const AXES: [&[&str]; 3] = [
    &["meer", "wellen", "brandung", "küste", "schiff", "hafen", "kutter", "segel"],
    &["laterne", "flamme", "docht", "kerze", "schein", "lampe"],
    &["beet", "samen", "wurzel", "erde", "pflanze", "keimen"],
];
const DIMENSION: usize = 4;

fn concept_vector(text: &str) -> Vec<f32> {
    let mut v = vec![0f32; DIMENSION];
    for word in text.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()) {
        match AXES.iter().position(|axis| axis.contains(&word.to_lowercase().as_str())) {
            Some(axis) => v[axis] += 1.0,
            None => v[DIMENSION - 1] += 0.02,
        }
    }
    let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    v.iter_mut().for_each(|x| *x /= norm.max(f32::MIN_POSITIVE));
    v
}

/// Mock embedder with a fixed vector space; counts every embedded text (proof: none when drawing).
#[derive(Clone, Default)]
struct Concepts {
    seen: Arc<Mutex<Vec<String>>>,
}

struct ConceptEmbedder {
    model: String,
    seen: Arc<Mutex<Vec<String>>>,
}

impl Embedder for ConceptEmbedder {
    fn model(&self) -> &str {
        &self.model
    }

    fn embed<'a>(&'a self, texts: &'a [String]) -> BoxFuture<'a, Result<Embeddings, RagError>> {
        Box::pin(async move {
            self.seen.lock().unwrap().extend(texts.iter().cloned());
            Ok(Embeddings {
                model: self.model.clone(),
                dimension: DIMENSION,
                vectors: texts.iter().map(|t| concept_vector(t)).collect(),
            })
        })
    }
}

impl EmbedderFactory for Concepts {
    fn for_model(&self, model: &str) -> Arc<dyn Embedder> {
        Arc::new(ConceptEmbedder { model: model.to_owned(), seen: self.seen.clone() })
    }
}

async fn ideas_with(server: &common::Server, work: i64, list: &[(&[&str], &str)]) -> Vec<i64> {
    let mut ids = Vec::new();
    for (words, note) in list {
        let words: Vec<String> = words.iter().map(|w| (*w).to_owned()).collect();
        let new = NewIdea {
            work: Some(work),
            key: None,
            words: &words,
            note,
            quote: "",
            source: "",
            bible: "",
            origin: IdeaOrigin::Human { name: "Autor".into() },
        };
        ids.push(server.state.store.idea_create(&new).await.unwrap().id);
    }
    ids
}

fn pair(map: &Value, a: i64, b: i64) -> Option<&Value> {
    map["near"]["pairs"].as_array().unwrap().iter().find(|p| p["a"] == a.min(b) && p["b"] == a.max(b))
}

#[tokio::test]
async fn nearness_comes_from_stored_vectors_without_embedding_while_drawing() {
    let server = common::open().await;
    let concepts = Concepts::default();
    rag::set_embedders(&server.state, Arc::new(concepts.clone()));
    let work = server.import(&book(), "laube.typ").await.work;
    let ids = ideas_with(
        &server,
        work,
        &[
            (&["Wellen"], "Die Brandung an der Küste"),
            (&["Kutter"], "Im Hafen schaukelt ein Schiff"),
            (&["Docht"], "Die Kerze und ihre Flamme"),
            (&["Laterne"], "Ein Schein im Fenster"),
            (&["Ackerrand", "Docht"], "Unkraut zupfen"),
        ],
    )
    .await;
    rag::catch_up(&server.state).await;
    rag::wait_idle(&server.state).await;
    let embedded = concepts.seen.lock().unwrap().len();
    assert!(embedded > 0, "der Index hat die Einfälle eingebettet (Hintergrund-Aufgabe)");

    let map = ok(&server, "idea.map", json!({ "work": work })).await;
    // Proof: drawing reads only stored vectors – not a single embedding request for it.
    assert_eq!(concepts.seen.lock().unwrap().len(), embedded, "keine Einbettungsanfrage beim Zeichnen");
    assert_eq!(map["near"]["state"], "ok", "Nähe verfügbar: {}", map["near"]);
    assert!(map["near"]["message"].as_str().unwrap().contains("gespeicherten Einbettungen"), "{}", map["near"]);
    let sea = pair(&map, ids[0], ids[1]).unwrap_or_else(|| panic!("Meer-Paar fehlt: {}", map["near"]));
    assert_eq!(sea["via"], "vector", "ohne gemeinsames Wort nur über Vektoren: {sea}");
    assert!(sea["score"].as_f64().unwrap() >= 0.75, "ab der Schwelle: {sea}");
    assert_eq!(pair(&map, ids[2], ids[3]).map(|p| p["via"].clone()), Some(json!("vector")), "Licht-Paar");
    assert!(pair(&map, ids[0], ids[2]).is_none(), "Meer und Licht sind sich nicht nah: {}", map["near"]);
    let both = pair(&map, ids[2], ids[4]).unwrap_or_else(|| panic!("Docht-Paar fehlt: {}", map["near"]));
    assert_eq!(both["shared_words"], json!(["Docht"]), "gleiches Schlagwort: {both}");
    assert!(both["via"] == "both" || both["via"] == "words", "Wort-Nähe zählt mit: {both}");
    for p in map["near"]["pairs"].as_array().unwrap() {
        assert!(p["a"].as_i64() < p["b"].as_i64(), "a < b: {p}");
    }
    // near=false switches it off, near_threshold takes effect.
    let off = ok(&server, "idea.map", json!({ "work": work, "near": false })).await;
    assert_eq!(
        (off["near"]["state"].as_str(), off["near"]["pairs"].as_array().unwrap().len()),
        (Some("off"), 0),
        "near=false schaltet die Nähe ab: Zustand off, keine Paare"
    );
    let strict = ok(&server, "idea.map", json!({ "work": work, "near_threshold": 1, "near_limit": 1 })).await;
    let mut degree: BTreeMap<i64, u32> = BTreeMap::new();
    for p in strict["near"]["pairs"].as_array().unwrap() {
        *degree.entry(p["a"].as_i64().unwrap()).or_default() += 1;
        *degree.entry(p["b"].as_i64().unwrap()).or_default() += 1;
    }
    assert!(degree.values().all(|d| *d <= 1), "near_limit=1: höchstens ein Partner je Einfall: {degree:?}");
    assert_eq!(concepts.seen.lock().unwrap().len(), embedded, "auch danach keine Einbettung");
}

#[tokio::test]
async fn without_an_embedder_the_map_says_so_and_keeps_word_nearness() {
    let server = common::open().await;
    let work = server.import(&book(), "laube.typ").await.work;
    let ids = ideas_with(&server, work, &[(&["Wellen", "Ufer"], ""), (&["ufer"], ""), (&["Kerze"], "")]).await;
    let map = ok(&server, "idea.map", json!({ "work": work })).await;
    assert_eq!(map["near"]["state"], "no_embedder", "ehrlich ohne Einbetter: {}", map["near"]);
    assert!(
        map["near"]["message"].as_str().unwrap().contains("Schlagwörter"),
        "Meldung nennt den Ersatz: {}",
        map["near"]
    );
    let words = pair(&map, ids[0], ids[1]).unwrap_or_else(|| panic!("Wort-Nähe fehlt: {}", map["near"]));
    assert_eq!((words["via"].as_str(), words["shared_words"].clone()), (Some("words"), json!(["Ufer"])), "{words}");
    let shown: HashSet<i64> = map["near"]["pairs"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|p| [p["a"].as_i64().unwrap(), p["b"].as_i64().unwrap()])
        .collect();
    assert!(!shown.contains(&ids[2]), "ohne gemeinsames Wort keine Nähe ohne Einbetter");
}

#[tokio::test]
async fn moving_a_linked_idea_to_another_work_unlinks_it_everywhere() {
    // Finding Round 25: links exist only within one work; a move used to leave them active, the page of the idea and
    // einfaelle.md of both works kept showing them, the map did not.
    let server = common::open().await;
    let (work, slug, ids, _) = forty(&server).await;
    let other = server
        .import("#set document(title: \"Das Teichbuch\", author: \"Erna Efeu\")\n\n= Schilf\n\nGrün.\n", "teich.typ")
        .await;
    let (a, b, c) = (ids[0], ids[1], ids[2]);
    let ab = ok(&server, "idea.link", json!({ "from": a, "to": b, "kind": "leads_to" })).await["link"]["id"].clone();
    let ca = ok(&server, "idea.link", json!({ "from": c, "to": a, "kind": "example_of" })).await["link"]["id"].clone();
    ok(&server, "idea.map_pos_set", json!({ "work": work, "idea": a, "x": 5.0, "y": 6.0 })).await;

    let moved = ok(&server, "idea.update", json!({ "id": a, "work": other.work })).await;
    assert_eq!(moved["unlinked"], json!([ab, ca]), "Antwort nennt die gelösten Verbindungen: {moved}");
    let links = ok(&server, "idea.links", json!({ "idea": a })).await;
    assert_eq!(links["links"], json!([]), "idea.links {{idea}} ohne die gelösten: {links}");
    let map = ok(&server, "idea.map", json!({ "work": work, "near": false })).await;
    assert!(
        map["positions"].as_array().unwrap().iter().all(|p| p["idea"] != a),
        "keine Lage des umgezogenen Einfalls im alten Werk"
    );
    let (_, page) = server.get(&format!("/einfaelle/{a}")).await;
    assert!(!page.contains("value=\"idea.unlink\""), "Seite des Einfalls ohne „Lösen“ einer Verbindung über Werke");
    let old_mirror = server.mirror_dir_of(&slug);
    if let Ok(md) = std::fs::read_to_string(old_mirror.join("einfaelle.md")) {
        assert!(!md.contains(&format!("→ {a} ")) && !md.contains(&format!("← {a} ")), "altes einfaelle.md: {md}");
    }
    if let Some(log) = common::git(&old_mirror, &["log", "-1", "--format=%B"]) {
        assert!(log.contains("Verbindungen gelöst"), "Commit im alten Spiegel nennt das Lösen: {log}");
    }
    // Restoring across the work boundary stays rejected (-32602), back in the work it works again.
    assert_eq!(code(&server, "idea.link_restore", json!({ "id": ab })).await, -32602, "über Werke abgelehnt");
    ok(&server, "idea.update", json!({ "id": a, "work": work })).await;
    ok(&server, "idea.link_restore", json!({ "id": ab })).await;
}
