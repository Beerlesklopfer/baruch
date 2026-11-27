// SPDX-License-Identifier: AGPL-3.0-or-later
//! Ctrl+mouse wheel enlarges only the area under the mouse pointer (wish 2026-11-06, `web/src/zoom.ts`): zones through
//! `data-zoom-zone`, in the editor header, toolbar, sheet and status line each for itself.

use crate::common;

fn script(name: &str) -> &'static str {
    baruch_server::SCRIPTS
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, body)| *body)
        .unwrap_or_else(|| panic!("{name} fehlt"))
}

#[test]
fn ctrl_wheel_is_caught_everywhere_and_zooms_only_the_zone_under_the_pointer() {
    let main = script("main.js");
    assert!(main.contains("./zoom.js"), "main.js muss zoom.js laden, sonst zoomt der Browser die ganze Seite");
    let zoom = script("zoom.js");
    assert!(zoom.contains("passive: false"), "nur ein nicht-passiver Hörer darf preventDefault für das Mausrad rufen");
    let caught = zoom.find("event.preventDefault()").expect("Strg+Mausrad muss abgefangen werden");
    let zone = zoom.find("zoneAt(event.target)").expect("der Bereich kommt vom Element unter dem Mauszeiger");
    assert!(
        caught < zone,
        "erst abfangen, dann den Bereich suchen: auch außerhalb jedes Bereichs zoomt die Seite nicht"
    );
    for zone in ["[data-zoom-zone]", "dialog[open]", "main"] {
        assert!(zoom.contains(zone), "{zone} gehört zu den Bereichen, die sich einzeln vergrößern lassen");
    }
    assert!(
        zoom.contains("dataset.zoomZone"),
        "der Name der Zone (Cookie) kommt aus data-zoom-zone, nicht aus einer festen id"
    );
    assert!(
        zoom.contains("MutationObserver"),
        "Zonen im Editor entstehen bei jedem Neuzeichnen (ui.render) neu und bekommen ihre Stufe sofort wieder"
    );
    assert!(zoom.contains("\"zoom\""), "die Stufen stehen im Cookie baruch_zoom und überstehen das Neuladen");
}

#[test]
fn the_zoom_scales_the_children_so_the_column_keeps_its_width() {
    let css = include_str!("../static/app.css");
    assert!(
        css.contains("[data-zoomed] > * { zoom: var(--zoom, 1); }"),
        "gezoomt werden die Kinder des Bereichs; die Spalte selbst behält ihre Breite im Raster"
    );
}

#[test]
fn outline_and_right_column_stay_zones_of_their_own() {
    let page = include_str!("../templates/workspace.html");
    for zone in ["id=\"pane-tree\" aria-label=\"Gliederung\" data-zoom-zone=\"tree\"", "data-zoom-zone=\"assistants\""]
    {
        assert!(page.contains(zone), "{zone}: Gliederung und rechte Spalte lassen sich als Ganzes vergrößern");
    }
    assert!(
        !page.contains("id=\"pane-editor\" aria-label=\"Editor\" data-zoom-zone"),
        "der Editor als Ganzes ist keine Zone mehr: Kopf, Werkzeugleiste, Blatt und Statuszeile zoomen je für sich"
    );
}

#[tokio::test]
async fn each_part_of_the_editor_is_a_zone_with_its_own_name() {
    let server = common::open().await;
    let ui = server.rpc("ui.render", serde_json::json!({ "node": 2 })).await;
    let editor = ui["result"]["editor"].as_str().unwrap();
    for zone in ["head", "toolbar", "sheet", "status"] {
        assert_eq!(
            editor.matches(&format!("data-zoom-zone=\"{zone}\"")).count(),
            1,
            "genau eine Zone {zone} im Editor, mit eigener gemerkter Stufe: {editor}"
        );
    }
    let right = ui["result"]["assistants"].as_str().unwrap();
    assert!(
        right.contains("data-zoom-zone=\"revisions\""),
        "der Fassungsbereich rechts zoomt für sich (innerste Zone gewinnt vor der rechten Spalte)"
    );
}
