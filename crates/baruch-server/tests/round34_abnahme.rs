// SPDX-License-Identifier: AGPL-3.0-or-later
//! Runde 34 „Mindmap“ – Abnahme (Besitz Integrator). In der Schnittstelle R34-S1 nur die Zusage der mechanischen
//! Verschiebung: Die Routen aus `idea_map::files` (Ausgaben, Formular) und die Seite aus `idea_map::bild` antworten
//! wie zuvor. Werk und Einfälle sind frei erfunden; kein Modell, kein Netz (kein PDF: das prüft `round25_bild`).

use crate::common;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use serde_json::{Value, json};

fn result(answer: &Value) -> &Value {
    assert!(answer["error"].is_null(), "RPC sollte gelingen: {answer}");
    &answer["result"]
}

#[tokio::test]
async fn the_moved_routes_answer_as_before() {
    let server = common::open().await;
    let source = "#set document(title: \"Deichbuch\")\n\n= Schafe\n\nText.\n\n= Siel\n\nText.\n";
    let work = server.import(source, "deichbuch.typ").await.work;
    let nodes = result(&server.rpc("work.get", json!({ "work": work })).await)["work"]["nodes"].clone();
    let siel = nodes.as_array().unwrap().iter().find(|n| n["title"] == "Siel").unwrap()["key"].as_i64().unwrap();
    let idea = result(&server.rpc("idea.create", json!({ "work": work, "words": "Flutmarke" })).await)["idea"]["id"]
        .as_i64()
        .unwrap();

    let (status, html) = server.get(&format!("/werke/{work}/karte")).await;
    assert_eq!(status, StatusCode::OK, "die Seite der Karte bleibt unter ihrer Adresse (bild::pages)");
    assert!(html.contains("Flutmarke"), "die Seite zeigt den Einfall");

    for (ending, content_type, marker) in [
        ("svg", "image/svg+xml", "<svg"),
        ("md", "text/markdown; charset=utf-8", "Flutmarke"),
        ("mm", "application/x-freemind", "<map"),
    ] {
        let request = Request::get(format!("/werke/{work}/karte.{ending}")).body(Body::empty()).unwrap();
        let (status, headers, body) = server.send(request).await;
        assert_eq!(status, StatusCode::OK, "karte.{ending} antwortet (files::pages ist in lib.rs verdrahtet)");
        assert_eq!(
            headers.get(header::CONTENT_TYPE).and_then(|v| v.to_str().ok()),
            Some(content_type),
            "karte.{ending} mit derselben Art wie vor der Verschiebung"
        );
        assert!(body.contains(marker), "karte.{ending} enthält {marker}: {body}");
    }
    let (status, _) = server.get("/werke/999999/karte.md").await;
    assert_eq!(status, StatusCode::NOT_FOUND, "unbekanntes Werk → 404 wie zuvor");

    let (status, location) = server
        .post_form(
            &format!("/werke/{work}/karte/act"),
            &format!("method=idea.update&id={idea}&key={siel}&back=near%3D0"),
        )
        .await;
    assert_eq!(status, StatusCode::SEE_OTHER, "das Formular leitet nach dem Speichern weiter (files::writes)");
    assert_eq!(
        location.as_deref(),
        Some(format!("/werke/{work}/karte?near=0&ok=bezug").as_str()),
        "zurück auf die gefilterte Karte mit der Rückmeldung"
    );
    let stored = result(&server.rpc("idea.get", json!({ "id": idea })).await)["idea"]["key"].as_i64();
    assert_eq!(stored, Some(siel), "„Bezug setzen“ hat den Bezug gespeichert");

    let (status, _) = server.post_form(&format!("/werke/{work}/karte/act"), "method=idea.delete&id=1").await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "im S1-Stand nimmt das Formular nur die bisherigen Aktionen an");
}
