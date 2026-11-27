// SPDX-License-Identifier: AGPL-3.0-or-later
//! Fixed keys of the outline items (`key`): every outline change reassigns the positions (`node`, seq). An outdated tab
//! sends the old position, but also the key; the key takes precedence so that nothing lands in a foreign section or the
//! wrong item is deleted. Real data folder in the temp folder, Gartenbuch: Frühling (0), Aussaat (1), Tiefe (2), Sommer
//! (3).

use crate::common;

use axum::http::StatusCode;
use serde_json::{Value, json};

fn result(answer: &Value) -> &Value {
    assert!(answer.get("error").is_none(), "RPC-Fehler: {answer}");
    &answer["result"]
}

async fn nodes(server: &common::Server, work: i64) -> Vec<Value> {
    let loaded = server.rpc("work.get", json!({ "work": work })).await;
    result(&loaded)["work"]["nodes"].as_array().unwrap().clone()
}

fn titles(nodes: &[Value]) -> Vec<&str> {
    nodes.iter().map(|n| n["title"].as_str().unwrap()).collect()
}

fn current_body(node: &Value) -> &str {
    node["revisions"].as_array().unwrap().iter().find(|r| r["status"] == "current").unwrap()["body"].as_str().unwrap()
}

/// Gartenbuch; plus the keys in the order before.
async fn garden() -> (common::Server, i64, Vec<i64>) {
    let server = common::open().await;
    let work = server.import_garden().await.work;
    let keys = nodes(&server, work).await.iter().map(|n| n["key"].as_i64().unwrap()).collect();
    (server, work, keys)
}

#[tokio::test]
async fn a_stale_tab_saves_into_the_section_it_showed_after_another_tab_moved_it() {
    let (server, work, keys) = garden().await;
    let summer = keys[3];
    // Tab B moves „Sommer“ to the front; „Tiefe“ is then at the old position 3.
    let moved = server.rpc("outline.move", json!({ "work": work, "node": 3, "target": 0, "place": "before" })).await;
    assert_eq!(result(&moved)["node"], 0, "Sommer steht jetzt vorn: {moved}");
    // Tab A still knows position 3, but sends the key of „Sommer“ along.
    let saved = server
        .rpc(
            "outline.save",
            json!({ "work": work, "node": 3, "key": summer, "body": "= Sommer\n\nNeuer Sommertext.\n", "based_on": 1 }),
        )
        .await;
    assert_eq!(result(&saved)["node"], 0, "gespeichert wurde im Punkt hinter dem Schlüssel, jetzt Stelle 0: {saved}");
    assert!(result(&saved).get("outline").is_none(), "die Gliederung bleibt, nichts umbenannt: {saved}");
    let after = nodes(&server, work).await;
    assert_eq!(titles(&after), ["Sommer", "Frühling", "Aussaat", "Tiefe"], "keine zweite „Sommer“, Tiefe bleibt");
    assert_eq!(current_body(&after[0]), "= Sommer\n\nNeuer Sommertext.\n", "der Text steht im echten „Sommer“");
    assert!(current_body(&after[3]).starts_with("=== Tiefe"), "„Tiefe“ an der alten Stelle 3 ist unberührt");
    assert_eq!(after[0]["key"], summer, "der Schlüssel bleibt beim Verschieben gleich");
}

#[tokio::test]
async fn a_stale_delete_form_deletes_the_point_it_named_or_nothing() {
    let (server, work, keys) = garden().await;
    let moved = server.rpc("outline.move", json!({ "work": work, "node": 3, "target": 0, "place": "before" })).await;
    result(&moved);
    // Without JavaScript: form from the old state with position 3 (now „Tiefe“) and the key of „Sommer“.
    let (status, location) = server
        .post_form(
            "/gliederung/act",
            &format!("method=outline.delete&work={work}&node=3&key={}&confirm=1&view=gliederung", keys[3]),
        )
        .await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Löschen über das Formular gelingt");
    assert!(location.is_some_and(|l| l.starts_with("/gliederung?")), "zurück zum Gliederungsmanager");
    let after = nodes(&server, work).await;
    assert_eq!(titles(&after), ["Frühling", "Aussaat", "Tiefe"], "gelöscht ist „Sommer“, nicht „Tiefe“ an Stelle 3");
    // The same key once more: the item no longer exists, nothing else is deleted.
    let again = server.rpc("outline.delete", json!({ "work": work, "node": 2, "key": keys[3], "confirm": true })).await;
    assert_eq!(again["error"]["code"], -32000, "gelöschter Schlüssel → abgelehnt: {again}");
    assert!(
        again["error"]["message"].as_str().unwrap().contains("gibt es nicht mehr"),
        "die Meldung sagt, dass der Punkt fehlt: {again}"
    );
    assert_eq!(titles(&nodes(&server, work).await), ["Frühling", "Aussaat", "Tiefe"], "nichts weiter gelöscht");
}

#[tokio::test]
async fn revision_forms_and_move_targets_follow_the_key_without_javascript() {
    let (server, work, keys) = garden().await;
    // „Frühling“ to the end: all positions shift by one.
    let moved = server.rpc("outline.move", json!({ "work": work, "node": 0, "target": 3, "place": "after" })).await;
    assert_eq!(titles(&nodes(&server, work).await), ["Sommer", "Frühling", "Aussaat", "Tiefe"], "{moved}");
    // Outdated post at „Sommer“ (formerly position 3, now 0) through /act.
    let (status, location) = server
        .post_form("/act", &format!("method=post.add&work={work}&node=3&key={}&body=Wann+gie%C3%9Fen%3F", keys[3]))
        .await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Beitrag über das Formular gelingt");
    let location = location.unwrap();
    assert!(
        location.starts_with(&format!("/?work={work}&key={}", keys[3])) && !location.contains("node="),
        "die Weiterleitung zeigt „Sommer“ über seinen Schlüssel, ohne Stelle (Runde 8): {location}"
    );
    let after = nodes(&server, work).await;
    assert_eq!(after[0]["posts"].as_array().unwrap().len(), 1, "der Beitrag steht bei „Sommer“");
    assert!(after[3]["posts"].as_array().unwrap().is_empty(), "„Tiefe“ an Stelle 3 bekommt keinen Beitrag");
    // Target through target_key: „Frühling“ (key) before „Sommer“ again (both chapters, the level stays); the positions
    // 0 and 0 sent along („Sommer vor Sommer“) would be invalid.
    let reordered = server
        .post_form(
            "/gliederung/act",
            &format!(
                "method=outline.move&work={work}&node=0&key={}&target=0&target_key={}&place=before",
                keys[0], keys[3]
            ),
        )
        .await;
    assert_eq!(reordered.0, StatusCode::SEE_OTHER, "Verschieben über Schlüssel gelingt");
    assert_eq!(
        titles(&nodes(&server, work).await),
        ["Frühling", "Aussaat", "Tiefe", "Sommer"],
        "Ziel und Punkt kamen aus den Schlüsseln, die (veralteten) Stellen 0 und 0 zählten nicht"
    );
}

#[tokio::test]
async fn pages_carry_the_key_in_every_form_that_names_a_section() {
    let (server, work, keys) = garden().await;
    let (status, html) = server.get(&format!("/?work={work}&node=3")).await;
    assert_eq!(status, StatusCode::OK, "die Arbeitsseite lädt");
    let key = format!("<input type=\"hidden\" name=\"key\" value=\"{}\">", keys[3]);
    assert!(html.contains(&key), "Editor- und Chatformulare tragen den Schlüssel von „Sommer“: {html}");
    assert!(
        !html.contains("<input type=\"hidden\" name=\"key\" value=\"\">"),
        "kein Formular schickt einen leeren Schlüssel (node.key steht im Kontext): {html}"
    );
    // The editor form itself (not only the left column) carries the key.
    let editor = html.split("data-outline-save>").nth(1).expect("Editor-Formular vorhanden");
    let editor = &editor[..editor.find("</form>").unwrap()];
    assert!(editor.contains(&key), "das Editor-Formular nennt „Sommer“ über seinen Schlüssel: {editor}");
    // Round 10: the chat input goes to /assistenten/act (method persona.chat, personas answer).
    let chat = html.split("value=\"persona.chat\"").nth(1).expect("Chatformular vorhanden");
    let chat = &chat[..chat.find("</form>").unwrap()];
    assert!(chat.contains(&key), "das Chatformular nennt „Sommer“ über seinen Schlüssel: {chat}");
    assert!(
        html.contains(&format!("data-key=\"{}\"", keys[0])),
        "die linke Spalte trägt die Schlüssel für Ziehen und Ablegen: {html}"
    );
    assert!(
        html.contains(&format!("name=\"target_key\" value=\"{}\"", keys[0])),
        "↑ bei „Sommer“ zielt über den Schlüssel auf „Frühling“: {html}"
    );
    let (_, edit) = server.get(&format!("/gliederung/bearbeiten?work={work}&node=1")).await;
    assert!(edit.contains("<select name=\"target_key\""), "die Zielauswahl nennt Schlüssel statt Stellen: {edit}");
    assert!(edit.contains(&format!("<option value=\"{}\">", keys[3])), "Ziel „Sommer“ über seinen Schlüssel: {edit}");
}
