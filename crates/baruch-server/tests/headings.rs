// SPDX-License-Identifier: AGPL-3.0-or-later
//! Gliederung und Überschriftszeilen bleiben gleich, auch außerhalb des Editors: „Fassung ändern“ und
//! `revision.save` nehmen nur Texte mit genau einer Überschrift der Ebene des Punkts (ein neuer Titel
//! derselben Ebene benennt um). Umschalten und Übernehmen (Runde 5) lassen die Gliederung der Fassung
//! folgen, auch in der Ebene; Fassungen ohne oder mit weiteren Überschriften lehnen sie ab. Sonst ergäbe
//! ein Export und erneuter Import eine andere Gliederung. Gartenbuch: Frühling (0), Aussaat (1), Tiefe
//! (2), Sommer (3).

use crate::common;

use axum::http::StatusCode;
use serde_json::{Value, json};

fn titles(loaded: &Value) -> Vec<String> {
    loaded["result"]["work"]["nodes"].as_array().unwrap().iter().map(|n| n["title"].as_str().unwrap().into()).collect()
}

/// Exportieren und als neues Werk wieder einlesen: dieselben Titel in derselben Reihenfolge?
async fn reimported_titles(server: &common::Server, work: i64) -> Vec<String> {
    let export = server.rpc("work.export", json!({ "work": work })).await;
    let typst = export["result"]["typst"].as_str().unwrap().to_owned();
    // Anderer Werktitel, damit der zweite Import weder als Doppel noch als gleicher Titel scheitert.
    let renamed = typst.replacen("title: \"Das Gartenbuch\"", "title: \"Gartenbuch, wieder eingelesen\"", 1);
    assert_ne!(renamed, typst, "der Export trägt den Werktitel in der Vorlage");
    let again = server.import(&renamed, "wieder.typ").await;
    titles(&server.rpc("work.get", json!({ "work": again.work })).await)
}

/// Ebene je Knoten aus `work.get` (Teil 0, Kapitel 1, Abschnitt 2, unter einem Abschnitt 3).
fn levels(loaded: &Value) -> Vec<u8> {
    let mut out: Vec<u8> = Vec::new();
    for node in loaded["result"]["work"]["nodes"].as_array().unwrap() {
        let parent = node["parent"].as_u64().map(|p| out[p as usize]);
        out.push(match node["kind"].as_str().unwrap() {
            "part" => 0,
            "chapter" => 1,
            _ if parent.is_some_and(|p| p >= 2) => 3,
            _ => 2,
        });
    }
    out
}

#[tokio::test]
async fn switching_back_to_a_version_with_another_level_moves_the_outline_along() {
    let server = common::open().await;
    let report = server.import_garden().await;
    let work = report.work;
    // Aussaat wird Kapitel: neue Fassung 2 „= Aussaat“, Tiefe dabei Abschnitt; Fassung 1 trägt „== Aussaat“.
    let level = server.rpc("outline.set_level", json!({ "work": work, "node": 1, "level": 1 })).await;
    assert!(level.get("error").is_none(), "{level}");
    let dir = server.mirror_dir_of(&report.slug);
    let commits = |dir: &std::path::Path| -> Option<u64> {
        common::git(dir, &["rev-list", "--count", "HEAD"]).map(|c| c.trim().parse().unwrap())
    };
    let before = commits(&dir);
    let old =
        server.rpc("node.get", json!({ "work": work, "node": 1 })).await["result"]["revisions"][0]["body"].clone();

    // Nutzerentscheidung „Nein“: Die Fassung wird nicht angepasst, die Gliederung folgt ihr.
    let back = server.rpc("revision.set_current", json!({ "work": work, "node": 1, "rev": 1 })).await;
    assert!(back.get("error").is_none(), "Umschalten auf die alte Ebene ist erlaubt: {back}");
    assert_eq!(back["result"]["rev"], 1, "Fassung 1 ist aktuell: {back}");
    let outline = &back["result"]["outline"];
    assert_eq!(outline["action"], "level", "die Antwort trägt die Gliederungsänderung: {back}");
    assert_eq!(outline["detail"], "„Aussaat“ Ebene 1 → 2", "mit alter und neuer Ebene: {back}");
    assert!(outline.get("commit").is_none(), "die Gliederungsantwort trägt keinen eigenen Commit: {back}");
    let node = server.rpc("node.get", json!({ "work": work, "node": 1 })).await;
    assert_eq!(common::currents(&node["result"]), 1, "genau eine aktuelle Fassung");
    assert_eq!(node["result"]["revisions"][0]["body"], old, "der Text von Fassung 1 bleibt byte-gleich");
    let loaded = server.rpc("work.get", json!({ "work": work })).await;
    assert_eq!(levels(&loaded), [1, 2, 3, 1], "Aussaat wieder Abschnitt, Tiefe wieder Unterabschnitt");
    assert_eq!(
        reimported_titles(&server, work).await,
        titles(&loaded),
        "Export und Import ergeben dieselbe Gliederung"
    );

    // Spiegel: genau ein Commit für Umschalten und Mitstufen.
    let (Some(before), Some(after)) = (before, commits(&dir)) else { return }; // kein git-Programm
    assert_eq!(after, before + 1, "Umschalten mit anderer Ebene ist genau ein Commit");
    let log = common::git(&dir, &["log", "-1", "--format=%s%n%b"]).unwrap();
    assert!(log.contains(": auf Fassung 1 umgeschaltet · Ebene 1 → 2"), "Betreff mit Ebene: {log}");
    assert!(log.contains("Tiefe: Fassung"), "der Rumpf nennt den mitgestuften Punkt: {log}");
    assert!(log.contains("Baruch-Action: switched"), "Trailer der Umschaltung: {log}");
    assert_eq!(back["result"]["commit"], common::git(&dir, &["rev-parse", "HEAD"]).unwrap().trim(), "Commit-Id");
}

#[tokio::test]
async fn the_raw_ways_refuse_texts_that_would_split_merge_or_change_the_level() {
    let server = common::open().await;
    let work = server.import_garden().await.work;
    let split = "= Sommer\n\nText C.\n\n= Herbst\n\nText D.\n";
    let saved = server.rpc("revision.save", json!({ "work": work, "node": 3, "body": split })).await;
    assert_eq!(saved["error"]["code"], -32000, "revision.save mit weiterer Überschrift abgelehnt: {saved}");
    assert!(saved["error"]["message"].as_str().unwrap().contains("„Herbst“"), "nennt die Überschrift: {saved}");
    let bare = server.rpc("revision.save", json!({ "work": work, "node": 3, "body": "Ohne Überschrift.\n" })).await;
    assert_eq!(bare["error"]["code"], -32000, "revision.save ohne Überschrift abgelehnt: {bare}");
    let update = server
        .rpc("revision.update", json!({ "work": work, "node": 3, "rev": 1, "body": "== Sommer\n\nGießen.\n" }))
        .await;
    assert_eq!(
        update["error"]["code"], -32000,
        "„Fassung ändern“ der aktuellen Fassung auf Ebene 2 abgelehnt: {update}"
    );
    // Ohne JavaScript: dasselbe Formular an /act wird mit 422 und Meldung abgelehnt.
    let (status, _) = server
        .post_form(
            "/act",
            &format!("method=revision.update&work={work}&node=3&rev=1&body=%3D%3D+Sommer%0A%0AGie%C3%9Fen.%0A"),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "das Formular „Fassung ändern“ wird abgelehnt");
    let node = server.rpc("node.get", json!({ "work": work, "node": 3 })).await;
    assert_eq!(node["result"]["revisions"].as_array().unwrap().len(), 1, "nichts angelegt: {node}");
    assert_eq!(node["result"]["revisions"][0]["body"], "= Sommer\n\nGießen, gießen, gießen.\n", "Text unverändert");
    let loaded = server.rpc("work.get", json!({ "work": work })).await;
    assert_eq!(titles(&loaded), ["Frühling", "Aussaat", "Tiefe", "Sommer"], "Gliederung unverändert");
    assert_eq!(
        reimported_titles(&server, work).await,
        titles(&loaded),
        "Export und Import ergeben dieselbe Gliederung"
    );
}

#[tokio::test]
async fn editing_an_older_version_is_not_blocked_by_the_heading_rule() {
    let server = common::open().await;
    let work = server.import_garden().await.work;
    let saved = server.rpc("revision.save", json!({ "work": work, "node": 3, "body": "= Sommer\n\nNeu.\n" })).await;
    assert_eq!(saved["result"]["rev"], 2, "{saved}");
    // Fassung 1 ist nicht mehr aktuell: ihr Text bestimmt die Gliederung nicht, Notizen und Text gehen.
    let update = server
        .rpc("revision.update", json!({ "work": work, "node": 3, "rev": 1, "body": "Entwurf ohne Überschrift.\n" }))
        .await;
    assert_eq!(update["result"]["changed"], true, "eine ältere Fassung lässt sich frei ändern: {update}");
    // Zurückschalten auf sie geht dann aber nicht.
    let back = server.rpc("revision.set_current", json!({ "work": work, "node": 3, "rev": 1 })).await;
    assert_eq!(back["error"]["code"], -32000, "Fassung ohne Überschrift wird nicht aktuell: {back}");
}
