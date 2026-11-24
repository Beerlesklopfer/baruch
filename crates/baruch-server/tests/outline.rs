// SPDX-License-Identifier: AGPL-3.0-or-later
//! Gliederungsmanager über RPC, Formulare und Seiten (Vertrag Runde 4, Abschnitte 4–6): echter
//! Datenordner im Temp-Ordner (SQLite, Bibliothek, Git-Spiegel), Gartenbuch aus `tests/data`.

use crate::common;

use std::borrow::Cow;
use std::net::SocketAddr;
use std::str::FromStr;

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode, header};
use serde_json::{Value, json};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

/// Gartenbuch importieren: Frühling (0), Aussaat (1), Tiefe (2), Sommer (3).
async fn garden() -> (common::Server, i64, std::path::PathBuf) {
    let server = common::open().await;
    let report = server.import_garden().await;
    let dir = server.mirror_dir_of(&report.slug);
    (server, report.work, dir)
}

fn result(answer: &Value) -> &Value {
    assert!(answer.get("error").is_none(), "RPC-Fehler: {answer}");
    &answer["result"]
}

fn titles(work: &Value) -> Vec<String> {
    work["work"]["nodes"].as_array().unwrap().iter().map(|n| n["title"].as_str().unwrap().to_owned()).collect()
}

fn commits(dir: &std::path::Path) -> Option<u32> {
    common::git(dir, &["rev-list", "--count", "HEAD"]).map(|c| c.trim().parse().unwrap())
}

#[tokio::test]
async fn moving_over_rpc_reorders_the_work_and_answers_with_the_new_seq() {
    let (server, work, dir) = garden().await;
    let before = commits(&dir);
    let answer = server.rpc("outline.move", json!({ "work": work, "node": 3, "target": 0, "place": "before" })).await;
    let moved = result(&answer);
    assert_eq!(moved["node"], 0, "Sommer steht vorn: {answer}");
    assert_eq!(moved["action"], "moved", "die Antwort nennt die Art der Änderung");
    assert_eq!(moved["renumbered"], json!([[3, 0], [0, 1], [1, 2], [2, 3]]), "[alt, neu] in Baumreihenfolge");
    assert!(moved["commit"].is_string(), "ein Commit gemeldet: {answer}");
    let loaded = server.rpc("work.get", json!({ "work": work })).await;
    assert_eq!(titles(&loaded["result"]), ["Sommer", "Frühling", "Aussaat", "Tiefe"], "neue Reihenfolge");
    let export = server.rpc("work.export", json!({ "work": work })).await;
    let typst = export["result"]["typst"].as_str().unwrap();
    assert!(typst.find("= Sommer").unwrap() < typst.find("= Frühling").unwrap(), "Export in neuer Reihenfolge");
    assert!(typst.starts_with("// Gartenbuch"), "Werkvorlage bleibt vorn: {typst}");
    if let (Some(before), Some(after)) = (before, commits(&dir)) {
        assert_eq!(after, before + 1, "genau ein Commit");
    }
    assert!(dir.join("abschnitte/000-sommer.typ").is_file(), "Spiegeldatei nach neuer seq benannt");
}

#[tokio::test]
async fn invalid_outline_changes_answer_32000_and_write_no_commit() {
    let (server, work, dir) = garden().await;
    let before = commits(&dir);
    for (method, params, why) in [
        ("outline.move", json!({ "node": 3, "target": 1, "place": "into" }), "Kapitel unter einen Abschnitt"),
        ("outline.move", json!({ "node": 0, "target": 2, "place": "after" }), "in den eigenen Teilbaum"),
        ("outline.set_level", json!({ "node": 2, "level": 4 }), "Ebene 4"),
        ("outline.set_level", json!({ "node": 1, "level": 3 }), "Unterpunkt käme auf Ebene 4"),
    ] {
        let mut params = params;
        params["work"] = json!(work);
        let answer = server.rpc(method, params).await;
        assert_eq!(answer["error"]["code"], -32000, "{why}: {answer}");
    }
    let bad = server.rpc("outline.move", json!({ "work": work, "node": 3, "target": 0, "place": "neben" })).await;
    assert_eq!(bad["error"]["code"], -32602, "unbekannter Ort: {bad}");
    assert_eq!(commits(&dir), before, "abgelehnte Änderungen committen nichts");
    let loaded = server.rpc("work.get", json!({ "work": work })).await;
    assert_eq!(titles(&loaded["result"]), ["Frühling", "Aussaat", "Tiefe", "Sommer"], "nichts umgestellt");
}

#[tokio::test]
async fn deleting_needs_confirm_and_keeps_the_text_in_the_commit() {
    let (server, work, dir) = garden().await;
    let missing = server.rpc("outline.delete", json!({ "work": work, "node": 2 })).await;
    assert_eq!(missing["error"]["code"], -32602, "ohne confirm: {missing}");
    assert_eq!(missing["error"]["message"], "Löschen bitte bestätigen (confirm).", "ohne confirm wird nicht gelöscht");
    let children = server.rpc("outline.delete", json!({ "work": work, "node": 1, "confirm": true })).await;
    assert_eq!(children["error"]["code"], -32000, "mit Unterpunkten: {children}");
    let done = server.rpc("outline.delete", json!({ "work": work, "node": 2, "confirm": "1" })).await;
    assert_eq!(result(&done)["action"], "deleted", "mit confirm wird gelöscht");
    assert_eq!(result(&done)["node"], 1, "danach gezeigt: Aussaat");
    let Some(log) = common::git(&dir, &["log", "-1", "--format=%s%n%b"]) else { return };
    assert!(log.starts_with("Gliederung: „Tiefe“ gelöscht"), "Betreff: {log}");
    assert!(log.contains("Gelöschter Text „Tiefe“:\n=== Tiefe\n\nZwei Zentimeter.\n"), "voller Text im Rumpf: {log}");
    assert!(!dir.join("abschnitte/002-tiefe.typ").exists(), "Datei weg, Text bleibt in der Git-Historie");
}

#[tokio::test]
async fn set_level_and_rename_write_new_human_versions_of_the_heading_line_only() {
    let (server, work, _dir) = garden().await;
    let level = server.rpc("outline.set_level", json!({ "work": work, "node": "1", "level": "1" })).await;
    let level = result(&level);
    assert_eq!(
        level["revisions"],
        json!([{ "node": 1, "rev": 2 }, { "node": 2, "rev": 2 }]),
        "Aussaat und Tiefe: {level}"
    );
    let node = server.rpc("node.get", json!({ "work": work, "node": 1 })).await;
    let revisions = node["result"]["revisions"].as_array().unwrap();
    let (old, new) = (revisions[0]["body"].as_str().unwrap(), revisions[1]["body"].as_str().unwrap());
    assert_eq!(new, old.replacen("== Aussaat", "= Aussaat", 1), "nur die Überschriftszeile ändert sich");
    assert_eq!(revisions[1]["author"], json!({ "kind": "human", "name": "Autor" }), "Fassung vom Menschen");
    assert_eq!(common::currents(&node["result"]), 1, "genau eine aktuelle Fassung");
    let renamed = server.rpc("outline.rename", json!({ "work": work, "node": 3, "title": "Hoch*sommer" })).await;
    assert_eq!(result(&renamed)["detail"], "„Sommer“ → „Hoch*sommer“ umbenannt", "Betreff nennt alten und neuen Titel");
    let node = server.rpc("node.get", json!({ "work": work, "node": 3 })).await;
    assert_eq!(node["result"]["title"], "Hoch*sommer", "Titel aufgelöst");
    assert!(
        node["result"]["revisions"][1]["body"].as_str().unwrap().starts_with("= Hoch\\*sommer\n"),
        "Überschriftszeile maskiert: {node}"
    );
}

#[tokio::test]
async fn saving_in_the_editor_follows_the_headings_split_and_merge_need_confirmation() {
    let (server, work, _dir) = garden().await;
    let body = "= Sommer\n\nGießen, gießen, gießen.\n\n== Mulchen\n\nStroh.\n";
    let check = server.rpc("outline.check", json!({ "work": work, "node": 3, "body": body })).await;
    assert_eq!(result(&check)["split"], json!(["Mulchen"]), "outline.check meldet die neue Überschrift: {check}");
    assert_eq!(result(&check)["valid"], true, "Teilen ergäbe eine gültige Gliederung");
    let refused = server.rpc("outline.save", json!({ "work": work, "node": 3, "body": body, "based_on": 1 })).await;
    assert_eq!(refused["error"]["code"], -32000, "ohne split abgelehnt: {refused}");
    let split = server.rpc("outline.save", json!({ "work": work, "node": 3, "body": body, "split": "1" })).await;
    let split = result(&split);
    assert_eq!(split["outline"]["action"], "split", "{split}");
    assert_eq!((split["node"].clone(), split["rev"].clone()), (json!(3), json!(2)), "gezeigt: Sommer, Fassung 2");
    assert!(split["commit"].is_string(), "ein Commit: {split}");
    let loaded = server.rpc("work.get", json!({ "work": work })).await;
    assert_eq!(
        titles(&loaded["result"]),
        ["Frühling", "Aussaat", "Tiefe", "Sommer", "Mulchen"],
        "der neue Punkt steht hinter Sommer"
    );

    let merge =
        server.rpc("outline.check", json!({ "work": work, "node": 4, "body": "Stroh ohne Überschrift.\n" })).await;
    assert_eq!(result(&merge)["merge"], true, "keine Überschrift: zusammenführen");
    let refused = server.rpc("outline.save", json!({ "work": work, "node": 4, "body": "Stroh.\n" })).await;
    assert_eq!(refused["error"]["code"], -32000, "ohne merge abgelehnt: {refused}");
    let merged =
        server.rpc("outline.save", json!({ "work": work, "node": 4, "body": "Stroh.\n", "merge": true })).await;
    assert_eq!(result(&merged)["outline"]["action"], "merged", "ohne Überschrift wird zusammengeführt");
    assert_eq!(result(&merged)["node"], 3, "gezeigt: Sommer, an den angehängt wurde");
    let renamed = server
        .rpc("outline.save", json!({ "work": work, "node": 0, "body": "= Lenz\n\nDie Beete werden umgegraben.\n\n" }))
        .await;
    assert_eq!(result(&renamed)["outline"]["action"], "renamed", "geänderte Überschriftszeile benennt um: {renamed}");
    // Der rohe Weg (revision.save) ändert die Gliederung nie: eine andere Ebene wird abgelehnt,
    // sonst liefen Gliederung und Überschriftszeile auseinander (Export und Import ergäben anderes).
    let plain = server.rpc("revision.save", json!({ "work": work, "node": 3, "body": "== Roh\n" })).await;
    assert_eq!(plain["error"]["code"], -32000, "revision.save mit anderer Ebene abgelehnt: {plain}");
    let loaded = server.rpc("work.get", json!({ "work": work })).await;
    assert_eq!(titles(&loaded["result"]), ["Lenz", "Aussaat", "Tiefe", "Sommer"], "revision.save ändert keinen Titel");
}

#[tokio::test]
async fn numbering_follows_the_setting_and_is_one_commit() {
    let (server, work, dir) = garden().await;
    let before = commits(&dir);
    let set = server.rpc("outline.set_numbering", json!({ "work": work, "numbering": "I.a" })).await;
    assert_eq!(result(&set)["action"], "numbering", "die Antwort nennt die Nummerierung");
    let loaded = server.rpc("work.get", json!({ "work": work })).await;
    let numbers: Vec<&str> =
        loaded["result"]["work"]["nodes"].as_array().unwrap().iter().map(|n| n["number"].as_str().unwrap()).collect();
    assert_eq!(numbers, ["I", "I.a", "I.a.a", "II"], "eigenes Muster");
    if let (Some(before), Some(after)) = (before, commits(&dir)) {
        assert_eq!(after, before + 1, "auch ohne Dateiänderung genau ein Commit");
        let trailers = common::git(&dir, &["log", "-1", "--format=%(trailers:only,unfold)"]).unwrap();
        assert!(trailers.contains("Baruch-Action: outline-numbering"), "Trailer: {trailers}");
    }
    let reset = server.rpc("outline.set_numbering", json!({ "work": work, "numbering": null })).await;
    assert!(result(&reset)["detail"].as_str().unwrap().starts_with("Nummerierung aus der Vorlage"), "{reset}");
    let bad = server.rpc("outline.set_numbering", json!({ "work": work, "numbering": "Ohne" })).await;
    assert_eq!(bad["error"]["code"], -32602, "Muster ohne Zählzeichen: ungültige Parameter (Runde 6): {bad}");
}

// ---- Formulare ohne JavaScript -----------------------------------------------------------------

#[tokio::test]
async fn forms_post_to_gliederung_act_and_redirect_back() {
    let (server, work, _dir) = garden().await;
    let (status, location) = server
        .post_form("/gliederung/act", &format!("method=outline.move&work={work}&node=3&target=0&place=before"))
        .await;
    assert_eq!(status, StatusCode::SEE_OTHER, "303 nach dem Verschieben");
    let summer = key_at(&server, work, 0).await;
    assert_eq!(
        location.as_deref(),
        Some(format!("/?work={work}&key={summer}").as_str()),
        "zurück zur Arbeitsseite, zum verschobenen Punkt über seinen Schlüssel (Runde 8)"
    );
    let (status, location) = server
        .post_form(
            "/gliederung/act",
            &format!("method=outline.set_level&work={work}&node=3&level=2&view=gliederung&lq=Beet"),
        )
        .await;
    assert_eq!(status, StatusCode::SEE_OTHER, "das Formular leitet weiter");
    assert_eq!(
        location.as_deref(),
        Some(format!("/gliederung?work={work}&key={}&lq=Beet#punkt-3", key_at(&server, work, 3).await).as_str()),
        "view=gliederung führt zum Gliederungsmanager zurück, die Suche bleibt"
    );
    let (status, _) = server.post_form("/gliederung/act", &format!("method=outline.delete&work={work}&node=3")).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "Löschen ohne Kästchen confirm → 422");
    let (status, _) = server
        .post_form("/gliederung/act", &format!("method=outline.move&work={work}&node=0&target=2&place=into"))
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "ungültiges Ziel → 409");
    let (status, _) = server.post_form("/gliederung/act", &format!("method=revision.delete&work={work}&node=0")).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "nur outline.* über dieses Formular");
}

#[tokio::test]
async fn saving_a_split_without_javascript_asks_first_on_its_own_page() {
    let (server, work, _dir) = garden().await;
    let body = "%3D+Sommer%0A%0AGie%C3%9Fen.%0A%0A%3D%3D+%3Cb%3EMulchen%3C%2Fb%3E%0A%0AStroh.%0A";
    let form = format!("method=outline.save&work={work}&node=3&based_on=1&body={body}");
    let request = Request::post("/gliederung/act")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(form.clone()))
        .unwrap();
    let (status, _, html) = server.send(request).await;
    assert_eq!(status, StatusCode::OK, "Rückfrage statt Speichern: {html}");
    assert!(html.contains("Teilen und speichern"), "Knopf zum Teilen fehlt: {html}");
    assert!(html.contains("<li>&lt;b&gt;Mulchen&lt;&#x2f;b&gt;</li>"), "neuer Titel maskiert genannt: {html}");
    assert!(
        html.contains("name=\"body\" value=\"= Sommer\n\nGießen.\n\n== &lt;b&gt;Mulchen"),
        "Text im versteckten Feld: {html}"
    );
    assert!(html.contains("name=\"split\" value=\"1\""), "Bestätigung schickt split=1");
    assert!(html.contains(">Zurück</a>"), "Zurück-Link");
    assert!(!html.contains("{{") && !html.contains("{%"), "keine Platzhalterreste");
    let loaded = server.rpc("work.get", json!({ "work": work })).await;
    assert_eq!(titles(&loaded["result"]).len(), 4, "die Rückfrage speichert nichts");
    let (status, location) = server.post_form("/gliederung/act", &format!("{form}&split=1")).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "das Formular leitet weiter");
    let summer = key_at(&server, work, 3).await;
    assert_eq!(
        location.as_deref(),
        Some(format!("/?work={work}&key={summer}&rev=2").as_str()),
        "danach Sommer, Fassung 2 (Key-Form)"
    );
    let loaded = server.rpc("work.get", json!({ "work": work })).await;
    assert_eq!(titles(&loaded["result"])[4], "<b>Mulchen</b>", "geteilt");
}

#[tokio::test]
async fn the_outline_form_from_a_foreign_origin_is_refused() {
    let (server, work, _dir) = garden().await;
    let request = Request::post("/gliederung/act")
        .header(header::HOST, "127.0.0.1:8765")
        .header(header::ORIGIN, "https://evil.example")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(format!("method=outline.delete&work={work}&node=2&confirm=1")))
        .unwrap();
    assert_eq!(server.send(request).await.0, StatusCode::FORBIDDEN, "fremde Seite → 403");
    let loaded = server.rpc("work.get", json!({ "work": work })).await;
    assert_eq!(titles(&loaded["result"]).len(), 4, "abgelehntes Formular löscht nichts");
}

// ---- Seiten ------------------------------------------------------------------------------------

#[tokio::test]
async fn the_outline_pages_list_every_point_with_buttons_in_german() {
    let (server, work, _dir) = garden().await;
    server.rpc("outline.rename", json!({ "work": work, "node": 3, "title": "<i>Sommer</i>" })).await;
    let (status, html) = server.get(&format!("/gliederung?work={work}&node=1")).await;
    assert_eq!(status, StatusCode::OK, "{html}");
    assert!(html.contains("Gliederung von „Das Gartenbuch“"), "Überschrift: {html}");
    for part in ["Kapitel", "Abschnitt", "Unterabschnitt", "Nummerierung", "Aus der Vorlage", "Keine Nummern"] {
        assert!(html.contains(part), "{part} fehlt: {html}");
    }
    assert!(html.contains("<td class=\"num\">1.1.1</td>"), "Nummer von Tiefe: {html}");
    assert!(html.contains("&lt;i&gt;Sommer&lt;&#x2f;i&gt;"), "Titel maskiert: {html}");
    assert!(!html.contains("<i>Sommer</i>"), "kein rohes HTML aus dem Titel");
    assert!(html.contains("name=\"method\" value=\"outline.move\""), "↑/↓ als Formular");
    assert!(html.contains("name=\"method\" value=\"outline.set_level\""), "←/→ als Formular");
    assert!(html.contains("action=\"/gliederung/act\""), "Formulare an /gliederung/act");
    let k = key_at(&server, work, 1).await;
    assert!(html.contains(&format!("href=\"/?work=2&amp;key={k}\"")), "zurück zum Werk, lesbar, mit Schlüssel: {html}");
    assert!(!html.contains("&amp;node=") && !html.contains("&node="), "kein Link mit node= (Runde 8): {html}");
    assert!(!html.contains("{{") && !html.contains("{%"), "keine Platzhalterreste");

    let (status, edit) = server.get(&format!("/gliederung/bearbeiten?work={work}&node=2")).await;
    assert_eq!(status, StatusCode::OK, "{edit}");
    for part in ["Umbenennen", "Ebene ändern", "Verschieben", "Neuer Gliederungspunkt", "Löschen"] {
        assert!(edit.contains(part), "{part} fehlt: {edit}");
    }
    assert!(edit.contains("name=\"confirm\" value=\"1\" required"), "Löschen nur mit Kästchen");
    assert!(edit.contains("value=\"outline.add\""), "Anlegen davor/danach/darunter");
    // Runde 7: Titelfelder schon im Browser begrenzt (maßgeblich bleibt der Server, `check_title`).
    assert!(edit.matches("name=\"title\"").count() >= 2, "Umbenennen und Anlegen haben ein Titelfeld: {edit}");
    assert_eq!(
        edit.matches("name=\"title\"").count(),
        edit.matches("required maxlength=\"200\" pattern=\".*\\S.*\"").count(),
        "jedes Titelfeld: Pflicht, höchstens 200 Zeichen, nicht nur Leerraum: {edit}"
    );
    let (_, parent) = server.get(&format!("/gliederung/bearbeiten?work={work}&node=1")).await;
    assert!(parent.contains("hat Unterpunkte; erst verschieben oder löschen."), "kein Löschen mit Unterpunkten");
    let (status, _) = server.get("/gliederung?work=99").await;
    assert_eq!(status, StatusCode::NOT_FOUND, "unbekanntes Werk");
}

#[tokio::test]
async fn the_outline_page_names_the_scheme_and_links_to_its_choice() {
    let (server, work, _dir) = garden().await;
    let (status, html) = server.get(&format!("/gliederung?work={work}&node=0")).await;
    assert_eq!(status, StatusCode::OK, "{html}");
    assert!(html.contains("Jetzt: <strong>Aus der Werkvorlage</strong> – Muster „1.1“"), "Schema und Muster: {html}");
    assert!(html.contains(&format!("<a href=\"/werke/{work}/nummerierung\">Schema wählen…</a>")), "Link zur Auswahl");
    let set = server.rpc("numbering.set", json!({ "work": work, "scheme": "parts_chapters" })).await;
    assert!(set.get("error").is_none(), "{set}");
    let (_, html) = server.get(&format!("/gliederung?work={work}&node=0")).await;
    assert!(html.contains("Jetzt: <strong>Teile und Kapitel</strong>."), "Schema mit Wörtern nur mit Namen: {html}");
    assert!(html.contains("<td class=\"num\">Kapitel 1</td>"), "Nummern nach dem Schema in der Tabelle");
    assert!(html.contains("name=\"numbering\" value=\"\""), "das Musterfeld bleibt leer (Schema ohne Muster)");
    let (_, html) = server.get(&format!("/?work={work}&node=0")).await;
    assert!(html.contains("Nummern: Teile und Kapitel"), "Gliederungskopf nennt das Schema");
}

#[tokio::test]
async fn the_left_pane_has_outline_forms_and_the_editor_saves_through_outline_save() {
    let (server, work, _dir) = garden().await;
    let (status, html) = server.get(&format!("/?work={work}&node=1")).await;
    assert_eq!(status, StatusCode::OK, "die Seite lädt");
    assert!(html.contains("data-widget=\"outline\""), "Widget-Wurzel für Ziehen und Ablegen");
    assert!(html.contains("draggable=\"true\""), "Einträge lassen sich ziehen");
    for id in ["o-down-0", "o-left-1", "o-right-1", "o-left-2", "o-up-3"] {
        assert!(html.contains(&format!("id=\"{id}\"")), "Formular {id} fehlt: {html}");
        assert!(html.contains(&format!("form=\"{id}\"")), "Knopf für {id} fehlt");
    }
    assert!(!html.contains("id=\"o-up-0\""), "das erste Kapitel hat kein ↑");
    let k = key_at(&server, work, 1).await;
    assert!(html.contains(&format!("href=\"/gliederung/bearbeiten?work=2&key={k}\"")), "Link Bearbeiten: {html}");
    assert!(
        html.contains("action=\"/gliederung/act\" data-rpc data-widget=\"editor\" data-outline-save"),
        "Editor-Formular"
    );
    assert!(html.contains("name=\"method\" value=\"outline.save\""), "Editor speichert über outline.save");
    let script = baruch_server::SCRIPTS.iter().find(|(name, _)| *name == "outline.js").expect("outline.js fehlt").1;
    for part in ["baruch:navigate", "dragover", "outline.check", "shiftKey", "outline.set_level", "confirm("] {
        assert!(script.contains(part), "{part} fehlt in outline.js");
    }
    let main = baruch_server::SCRIPTS.iter().find(|(name, _)| *name == "main.js").unwrap().1;
    assert!(main.contains("./outline.js"), "main.js lädt das Widget");
    let (_, css) = server.get("/static/app.css").await;
    assert!(css.contains(".outline-row.drop-into"), "outline.css hängt an app.css");
}

// ---- MCP ---------------------------------------------------------------------------------------

#[tokio::test]
async fn mcp_offers_no_tool_that_changes_the_outline() {
    let server = common::open().await;
    let mut request = Request::post("/mcp")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" }).to_string()))
        .unwrap();
    request.extensions_mut().insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 40000))));
    let (status, _, body) = server.send(request).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let answer: Value = serde_json::from_str(&body).unwrap();
    let names: Vec<&str> =
        answer["result"]["tools"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap()).collect();
    // Einfälle (idea_*, Runde 7) und eigene Beiträge (post_*, Runde 8) ändern weder Gliederung noch
    // Buchtext; `idea_delete`, `post_move` und `post_delete` zählen nicht. Die Gliederungswerkzeuge
    // outline_* (Wunsch 2026-11-06) legen an oder stellen nur Anträge; das prüft der Text unten. Die
    // Bildwerkzeuge asset_* (Runde 11) betreffen die Gliederung nicht; `asset_delete` ist ein Antrag.
    let excepted = |n: &str| {
        n.starts_with("idea_") || n.starts_with("post_") || n.starts_with("outline_") || n.starts_with("asset_")
    };
    for forbidden in ["move", "level", "rename", "delete", "numbering", "split", "merge", "create_work"] {
        assert!(
            !names.iter().any(|n| !excepted(n) && n.contains(forbidden)),
            "kein MCP-Werkzeug mit {forbidden}: {names:?}"
        );
    }
    for tool in answer["result"]["tools"].as_array().unwrap() {
        let name = tool["name"].as_str().unwrap();
        if [
            "outline_rename",
            "outline_move",
            "outline_set_level",
            "outline_set_numbered",
            "outline_delete",
            "asset_replace",
            "asset_set_cover",
            "asset_delete",
        ]
        .contains(&name)
        {
            let text = tool["description"].as_str().unwrap();
            assert!(
                text.starts_with("Antrag") && text.contains("Ändert nichts sofort"),
                "{name} ist nur ein Antrag: {text}"
            );
        }
    }
    assert!(names.contains(&"outline"), "das lesende Werkzeug outline bleibt: {names:?}");
}

// ---- Migration 0006 mit Bibliothek ------------------------------------------------------------

#[tokio::test]
async fn the_server_starts_on_a_database_before_0006_and_resolves_titles() {
    let data = tempfile::tempdir().unwrap();
    let options = SqliteConnectOptions::from_str("sqlite:")
        .unwrap()
        .filename(data.path().join("baruch.sqlite"))
        .create_if_missing(true)
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new().max_connections(1).connect_with(options).await.unwrap();
    let core: Vec<_> = baruch_store::MIGRATOR.iter().filter(|m| m.version <= 4).cloned().collect();
    let library: Vec<_> =
        baruch_library::MIGRATOR.iter().filter(|m| (101..=104).contains(&m.version)).cloned().collect();
    for migrations in [core, library] {
        let migrator = sqlx::migrate::Migrator {
            migrations: Cow::Owned(migrations),
            ignore_missing: true,
            ..sqlx::migrate::Migrator::DEFAULT
        };
        migrator.run(&pool).await.unwrap();
    }
    // Ein Werk wie aus dem alten Import: Titel wörtlich mit Maskierung.
    sqlx::raw_sql(
        "INSERT INTO work (id, slug, kind, title, author, template) VALUES (1, 'laube', 'book', 'Laube', 'Test', '');
         INSERT INTO node (work_id, seq, parent_seq, position, number, title, kind) VALUES
           (1, 0, NULL, 0, '1', 'Anna\\''s Laube', 'chapter'),
           (1, 1, 0, 0, '1.1', 'Preis \\$5', 'section');
         INSERT INTO revision (node_id, number, body, author_kind, author_name, status) VALUES
           (1, 1, '= Anna\\''s Laube\n\nText.\n', 'human', 'Autor', 'current'),
           (2, 1, '== Preis \\$5\n\nMehr.\n', 'human', 'Autor', 'current');",
    )
    .execute(&pool)
    .await
    .unwrap();
    let bodies: Vec<String> =
        sqlx::query_scalar("SELECT body FROM revision ORDER BY id").fetch_all(&pool).await.unwrap();
    pool.close().await;

    let state = baruch_server::AppState::open(data.path()).await.expect("Start mit Migration 0006");
    let work = state.store.load_work(1).await.unwrap();
    let titles: Vec<&str> = work.nodes.iter().map(|n| n.title.as_str()).collect();
    assert_eq!(titles, ["Anna's Laube", "Preis $5"], "Titel aufgelöst");
    let after: Vec<String> =
        sqlx::query_scalar("SELECT body FROM revision ORDER BY id").fetch_all(state.store.pool()).await.unwrap();
    assert_eq!(after, bodies, "Revisionstexte unverändert");
    assert_eq!(
        baruch_typst::xref::strip_key_labels(&state.store.export_typst(1).await.unwrap()).into_owned(),
        bodies.concat(),
        "Export byte-gleich"
    );
    let hits = baruch_library::search(state.store.pool(), "Laube", &baruch_library::SearchOptions::default()).await;
    assert!(hits.is_ok_and(|h| !h.is_empty()), "Volltextindex trägt die Titel weiter");
    assert_eq!(state.store.works().await.unwrap().len(), 1, "keine neue Saat");
    assert!(data.path().join("works/laube/abschnitte/000-anna-s-laube.typ").is_file(), "Spiegel mit aufgelöstem Titel");
}

/// Fester Schlüssel (`Node::key`) des Abschnitts an Stelle `seq`: Erzeugte Adressen tragen ihn statt
/// der Stelle (`/?work=W&key=K`, Vertrag Runde 8).
async fn key_at(server: &common::Server, work: i64, seq: i64) -> i64 {
    sqlx::query_scalar("SELECT id FROM node WHERE work_id = ? AND seq = ?")
        .bind(work)
        .bind(seq)
        .fetch_one(server.state.store.pool())
        .await
        .unwrap()
}
