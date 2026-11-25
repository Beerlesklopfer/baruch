// SPDX-License-Identifier: AGPL-3.0-or-later
//! Runde 34 „Mindmap“ – Bild und Widget (Besitz mindmap_widget; in der Schnittstelle R34-S1 nur die S1-Zusage von
//! `idea_map::bild::mindmap::render`). Werk und Einfälle sind frei erfunden; kein Modell, kein Netz.

use crate::common;

use baruch_server::idea_map::MapFilter;
use baruch_server::idea_map::bild::mindmap::{RenderOptions, render};
use baruch_server::idea_map::tree;
use serde_json::json;

/// S1-Zusage: `render` liefert gültiges SVG mit dem (maskierten) Werktitel, als Fragment und eigenständig.
#[tokio::test]
async fn render_returns_svg_with_the_work_title() {
    let server = common::open().await;
    let source = "#set document(title: \"Ebbe & Flut\")\n\n= Priel\n\nText.\n";
    let work = server.import(source, "ebbe.typ").await.work;
    let answer = server.rpc("idea.create", json!({ "work": work, "words": "Watt <nass>" })).await;
    assert!(answer["error"].is_null(), "Einfall anlegen gelingt: {answer}");
    let filter = MapFilter { near: false, ..MapFilter::default() };
    let map = tree::tree(&server.state, work, &filter).await.expect("tree gelingt");

    let fragment = render(&map, &RenderOptions::default());
    assert!(fragment.svg.starts_with("<svg"), "das Fragment beginnt mit <svg: {}", fragment.svg);
    assert!(fragment.svg.trim_end().ends_with("</svg>"), "und endet mit </svg>");
    assert!(fragment.svg.contains("Ebbe &amp; Flut"), "der Werktitel steht maskiert im Bild: {}", fragment.svg);
    assert!(!fragment.svg.contains("Ebbe & Flut"), "ein rohes & machte das SVG ungültig");
    assert!(
        fragment.svg.contains("Watt &lt;nass&gt;"),
        "Titel der Einfälle sind maskiert (kein Markup aus Nutzertext)"
    );
    assert!(!fragment.svg.contains("<nass>"), "Nutzertext wird nie zu einem Element");
    assert!(!fragment.svg.contains("xmlns="), "das Fragment für die Seite trägt kein xmlns (standalone: false)");
    assert!(fragment.width > 0.0 && fragment.height > 0.0, "Breite und Höhe sind gesetzt");

    let file = render(&map, &RenderOptions { standalone: true, ..RenderOptions::default() });
    assert!(
        file.svg.contains("xmlns=\"http://www.w3.org/2000/svg\""),
        "die eigenständige Datei trägt xmlns (sonst zeigt sie kein Programm als Bild)"
    );
    assert!(file.svg.contains("Ebbe &amp; Flut"), "auch die Datei nennt den Werktitel");
    assert_eq!(
        file.svg.matches("<text ").count(),
        map.walk().len(),
        "je Knoten des Baums genau eine Textzeile (S1: schlichte Zeichnung)"
    );
}
