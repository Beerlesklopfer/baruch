// SPDX-License-Identifier: AGPL-3.0-or-later
//! Runde 34 „Mindmap“ – Daten (Besitz mindmap_daten; in der Schnittstelle R34-S1 nur die S1-Zusage von
//! `idea_map::tree::tree`). Werk und Einfälle sind frei erfunden; kein Modell, kein Netz.

use crate::common;

use baruch_server::idea_map::MapFilter;
use baruch_server::idea_map::tree::{self, TreeKind};
use serde_json::{Value, json};

fn result(answer: &Value) -> &Value {
    assert!(answer["error"].is_null(), "RPC sollte gelingen: {answer}");
    &answer["result"]
}

/// S1-Zusage: `tree` liefert das Werk als Wurzel `w`, Kapitel darunter, Einfälle am Anker bzw. lose am Werk.
#[tokio::test]
async fn the_tree_has_the_work_as_root_and_hangs_ideas_at_their_anchor() {
    let server = common::open().await;
    let source =
        "#set document(title: \"Leuchtturmbuch\")\n\n= Nebel\n\nText.\n\n== Horn\n\nText.\n\n= Brandung\n\nText.\n";
    let report = server.import(source, "leuchtturmbuch.typ").await;
    let work = report.work;
    let nodes = result(&server.rpc("work.get", json!({ "work": work })).await)["work"]["nodes"].clone();
    let key = |title: &str| {
        nodes.as_array().unwrap().iter().find(|n| n["title"] == title).unwrap_or_else(|| panic!("kein Punkt {title}"))
            ["key"]
            .as_i64()
            .unwrap()
    };
    let (nebel, horn) = (key("Nebel"), key("Horn"));
    let create = |params: Value| {
        let server = &server;
        async move { result(&server.rpc("idea.create", params).await)["idea"]["id"].as_i64().unwrap() }
    };
    let at_section = create(json!({ "work": work, "key": horn, "words": "Signal", "note": "Zwei lange Töne." })).await;
    let loose = create(json!({ "work": work, "words": "Laterne" })).await;

    let filter = MapFilter { near: false, ..MapFilter::default() };
    let map = tree::tree(&server.state, work, &filter).await.expect("tree gelingt für ein vorhandenes Werk");

    assert_eq!(map.root.id, "w", "die Wurzel ist das Werk mit der festen Kennung w (Vertrag S1.2)");
    assert_eq!(map.root.kind, TreeKind::Work, "Art der Wurzel");
    assert_eq!(map.root.title, "Leuchtturmbuch", "die Wurzel trägt den Werktitel");
    assert_eq!(map.title, "Leuchtturmbuch", "MindMap::title ist der Werktitel");
    assert_eq!(map.total_ideas, 2, "beide angelegten Einfälle sind gezeigt");
    assert_eq!(map.root.descendants, 2, "die Wurzel zählt alle Nachfahren der Art Einfall");

    let chapter = map.find(&format!("n:{nebel}")).expect("das Kapitel ist ein Knoten n:<key>");
    assert_eq!(chapter.kind, TreeKind::Chapter, "Kapitel als Hauptast");
    assert!(!chapter.editable, "Kapitel sind in der Mindmap nicht änderbar (Gliederung = Antragsregel)");
    assert!(map.find(&format!("n:{horn}")).is_none(), "ein Abschnitt unter einem Kapitel ist kein eigener Knoten");

    let idea = map.find(&format!("i:{at_section}")).expect("der Einfall ist ein Knoten i:<id>");
    assert!(
        chapter.children.iter().any(|c| c.id == idea.id),
        "ein Einfall mit Bezug auf einen Abschnitt hängt am Kapitel darüber (Anker)"
    );
    assert_eq!(idea.idea, Some(at_section), "die id des Einfalls steht am Knoten");
    assert_eq!(idea.key, Some(horn), "der genaue Bezug (Abschnitt) bleibt am Einfall");
    assert_eq!(idea.title, "Signal", "ohne eigenen Titel zeigt der Knoten die Schlagwörter");
    assert!(idea.editable && idea.has_note, "Einfälle sind änderbar; die Notiz ergibt das Notiz-Symbol");
    assert!(
        idea.section_title.as_deref().is_some_and(|t| t.contains("Horn")),
        "der Titel des Abschnitts im Bezug steht am Einfall: {:?}",
        idea.section_title
    );

    assert!(
        map.root.children.iter().any(|c| c.id == format!("i:{loose}")),
        "ein Einfall ohne Bezug hängt direkt am Werk (keine Wolke „Ohne Kapitel“ im Baum)"
    );
    assert!(map.find("none").is_none(), "die Wolke none gibt es im Baum nicht");
    let loose_node = map.find(&format!("i:{loose}")).unwrap();
    assert!(!loose_node.has_note, "ohne Notiz, Zitat, Quelle und Bibelstelle kein Notiz-Symbol");

    let order: Vec<&str> = map.root.children.iter().map(|c| c.id.as_str()).collect();
    let last_chapter = order.iter().rposition(|id| id.starts_with("n:")).unwrap();
    let first_idea = order.iter().position(|id| id.starts_with("i:")).unwrap();
    assert!(last_chapter < first_idea, "Hauptäste: erst die Gliederung, danach die freien Einfälle: {order:?}");
    assert_eq!(map.walk().first().map(|(n, d)| (n.id.as_str(), *d)), Some(("w", 0)), "walk beginnt mit der Wurzel");
}
