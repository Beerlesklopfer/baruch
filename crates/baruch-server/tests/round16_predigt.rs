// SPDX-License-Identifier: AGPL-3.0-or-later
//! Round 16a „predigt-vorlage“: new work → sermon with outline (sermon text, introduction, three main points,
//! application, conclusion; scheme `sermon`) and sermon text from the SWORD Bible – through RPC and form without
//! JavaScript. Bibles are written by the test itself (`baruch_sword::fixture`, freely invented text) into a temp
//! folder, and it sets the module folders through `bible.set_dirs`; the real module folders of the machine are never
//! read. No model, no network.

use crate::common;

use std::path::Path;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use baruch_scripture::versification::german;
use baruch_sword::fixture::{Block, Fixture, Layout, write};
use serde_json::{Value, json};

/// Invented test Bible in German versification: John 3:16 and 3:18 present, 3:17 missing.
fn kiesel() -> Fixture {
    Fixture::new("Kieselbibel", Layout::ZText(Block::Book), german())
        .conf("Description", "Erfundene Kieselbibel")
        .conf("DistributionLicense", "Public Domain")
        .conf("Lang", "de")
        .verse("John", 3, 16, "Der Kiesel lag am Bach, und die Amsel sang.")
        .verse("John", 3, 18, "Am Abend schwieg der Bach.")
        .verse("Mal", 3, 24, "Ende des erfundenen Alten Testaments.")
}

async fn ok(server: &common::Server, method: &str, params: Value) -> Value {
    let answer = server.rpc(method, params).await;
    assert!(answer["error"].is_null(), "{method} sollte gelingen: {answer}");
    answer["result"].clone()
}

/// Set the module folder with the Bibles `modules` (empty = an empty folder, hence "no Bible").
async fn bibles(server: &common::Server, dir: &Path, modules: &[Fixture]) {
    let root = dir.join("sword");
    std::fs::create_dir_all(&root).unwrap();
    for m in modules {
        write(&root, m).unwrap();
    }
    ok(server, "bible.set_dirs", json!({ "dirs": [root.display().to_string()] })).await;
}

async fn form(server: &common::Server, uri: &str, body: &str) -> (StatusCode, axum::http::HeaderMap, String) {
    let request = Request::post(uri)
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(body.to_owned()))
        .unwrap();
    server.send(request).await
}

/// Numbers of the column „Nummer“ on `/gliederung` in their order.
fn outline_numbers(html: &str) -> Vec<String> {
    html.split("<td class=\"num\">").skip(1).map(|rest| rest.split("</td>").next().unwrap_or("").to_owned()).collect()
}

async fn nodes(server: &common::Server, work: i64) -> Vec<Value> {
    ok(server, "work.get", json!({ "work": work })).await["work"]["nodes"].as_array().unwrap().clone()
}

async fn numbered(server: &common::Server, work: i64) -> Vec<i64> {
    sqlx::query_scalar("SELECT numbered FROM node WHERE work_id = ? ORDER BY seq")
        .bind(work)
        .fetch_all(server.state.store.pool())
        .await
        .unwrap()
}

const TITLES: [&str; 7] =
    ["Predigttext", "Hinführung", "Beobachtung am Text", "Deutung", "Zuspruch", "Anwendung", "Ausblick"];
const NUMBERS: [&str; 7] = ["", "Einleitung", "I.", "II.", "III.", "", "Schluss"];

#[tokio::test]
async fn a_sermon_over_rpc_gets_the_sermon_outline_and_the_outline_page_shows_each_number_once() {
    let server = common::open().await;
    let created = ok(&server, "work.create", json!({ "title": "Pfingstpredigt", "kind": "sermon" })).await;
    let work = created["work"].as_i64().unwrap();
    assert_eq!(created["nodes"], 7, "sieben Punkte angelegt: {created}");
    assert!(created.get("sermon_text").is_none(), "ohne text_ref kein sermon_text: {created}");
    let list = nodes(&server, work).await;
    let titles: Vec<&str> = list.iter().map(|n| n["title"].as_str().unwrap()).collect();
    assert_eq!(titles, TITLES, "Predigttext, Einleitung, drei Hauptpunkte, Anwendung, Schluss");
    let numbers: Vec<&str> = list.iter().map(|n| n["number"].as_str().unwrap()).collect();
    assert_eq!(numbers, NUMBERS, "Schema sermon: Einleitung · I. · II. · III. · Schluss, die anderen ohne Nummer");
    assert_eq!(numbered(&server, work).await, [0, 1, 1, 1, 1, 0, 1], "Predigttext und Anwendung „Nummer aus“");
    let scheme: String = sqlx::query_scalar("SELECT numbering_scheme FROM work WHERE id = ?")
        .bind(work)
        .fetch_one(server.state.store.pool())
        .await
        .unwrap();
    assert_eq!(scheme, "sermon", "Gliederungsschema der Predigt (numbering_scheme = sermon)");

    let (status, page) = server.get(&format!("/gliederung?work={work}")).await;
    assert_eq!(status, StatusCode::OK, "die Gliederungsseite geht ohne JavaScript");
    assert_eq!(outline_numbers(&page), NUMBERS, "die Seite zeigt dieselben Nummern in derselben Reihenfolge");
    for word in ["Einleitung", "Schluss"] {
        assert!(
            !page.contains(&format!(">{word}</a>")),
            "kein Titel wiederholt die Nummer „{word}“ (sonst stünde sie doppelt)"
        );
    }
    for title in TITLES {
        assert!(page.contains(&format!(">{title}</a>")), "Titel „{title}“ steht als Link in der Gliederung");
    }
}

#[tokio::test]
async fn the_form_creates_a_sermon_without_javascript_and_export_import_keeps_outline_and_unnumbered_points() {
    let server = common::open().await;
    let (status, headers, _) = form(
        &server,
        "/werke/act",
        "action=create&title=Erntedank&author=&kind=sermon&preset=&subtitle=&audience=Gemeinde&bible=&text_ref=&module=",
    )
    .await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Anlegen ohne JavaScript leitet weiter");
    let location = headers.get(header::LOCATION).unwrap().to_str().unwrap().to_owned();
    let work: i64 = location.strip_prefix("/?work=").and_then(|w| w.parse().ok()).expect("Weiterleitung auf /?work=N");
    let numbers: Vec<String> =
        nodes(&server, work).await.iter().map(|n| n["number"].as_str().unwrap().to_owned()).collect();
    assert_eq!(numbers, NUMBERS, "leeres text_ref zählt als nicht angegeben: die Predigt entsteht wie über RPC");

    let (status, typst) = server.get(&format!("/werke/{work}/export")).await;
    assert_eq!(status, StatusCode::OK, "Export als Typst-Datei");
    let imported =
        ok(&server, "work.import", json!({ "file_name": "erntedank.typ", "source": typst, "title": "Erntedank (2)" }))
            .await;
    let again = imported["work"].as_i64().unwrap();
    let list = nodes(&server, again).await;
    let titles: Vec<&str> = list.iter().map(|n| n["title"].as_str().unwrap()).collect();
    assert_eq!(titles, TITLES, "Export → Import erhält die Gliederung");
    let numbers: Vec<&str> = list.iter().map(|n| n["number"].as_str().unwrap()).collect();
    assert_eq!(numbers, NUMBERS, "… und die Nummern nach dem Predigtschema: {typst}");
    assert_eq!(
        numbered(&server, again).await,
        [0, 1, 1, 1, 1, 0, 1],
        "… und „Nummer aus“ bei Predigttext und Anwendung"
    );
}

#[tokio::test]
async fn with_a_fixture_bible_the_sermon_text_is_quoted_with_its_source_and_the_wording_check_matches() {
    let server = common::open().await;
    let dir = tempfile::tempdir().unwrap();
    bibles(&server, dir.path(), &[kiesel()]).await;
    let created = ok(
        &server,
        "work.create",
        json!({ "title": "Kieselpredigt", "kind": "sermon", "text_ref": "Joh 3,16", "module": "Kieselbibel" }),
    )
    .await;
    let work = created["work"].as_i64().unwrap();
    let text = &created["sermon_text"];
    assert_eq!(text["quoted"], true, "mit Bibel steht der Wortlaut da: {created}");
    assert_eq!(
        (text["verses"].as_u64(), text["missing"].as_u64()),
        (Some(1), Some(0)),
        "ein Vers, keiner fehlt: {text}"
    );
    assert_eq!(text["module"], "Kieselbibel", "Quelle: das gewählte Modul: {text}");
    assert_eq!(text["normalized"], "Joh 3,16", "einheitliche Schreibweise der Stelle: {text}");
    assert!(text["notice"].is_null(), "nichts zu bemängeln, kein Hinweis: {text}");

    let list = nodes(&server, work).await;
    let first = &list[0];
    let body = first["revisions"][0]["body"].as_str().unwrap();
    assert!(
        body.contains(
            "#quote(block: true, attribution: [Joh 3,16 (Kieselbibel)])[Der Kiesel lag am Bach, und die Amsel sang.]"
        ),
        "Zitat mit Quelle (Stelle und Modul) im Punkt Predigttext: {body}"
    );
    assert!(body.starts_with("= Predigttext\n\n"), "unter der Überschrift des ersten Punkts: {body}");
    let check = ok(&server, "bible.check", json!({ "work": work, "key": first["key"] })).await;
    assert_eq!(check["refs"][0]["wording_code"], "match", "die Wortlautprüfung bestätigt das Zitat: {check}");
    let modules = ok(&server, "bible.modules", json!({ "work": work })).await;
    assert_eq!(modules["work_bible"], "Kieselbibel", "die gewählte Bibel ist die SWORD-Bibel der neuen Predigt");
}

#[tokio::test]
async fn missing_verses_are_counted_and_the_present_ones_are_still_quoted() {
    let server = common::open().await;
    let dir = tempfile::tempdir().unwrap();
    bibles(&server, dir.path(), &[kiesel()]).await;
    let created = ok(
        &server,
        "work.create",
        json!({ "title": "Lückenpredigt", "kind": "sermon", "text_ref": "Joh 3,16-18", "module": "Kieselbibel" }),
    )
    .await;
    let text = &created["sermon_text"];
    assert_eq!(
        (text["quoted"].as_bool(), text["verses"].as_u64(), text["missing"].as_u64()),
        (Some(true), Some(2), Some(1)),
        "Joh 3,17 fehlt im Modul: gemeldet, die beiden anderen Verse stehen da: {text}"
    );
    assert!(
        text["notice"].as_str().unwrap().contains("1 Vers fehlt"),
        "German notice of the gap, singular for one verse: {text}"
    );
}

#[tokio::test]
async fn without_a_bible_only_the_reference_stands_there_with_an_honest_notice() {
    let server = common::open().await;
    let dir = tempfile::tempdir().unwrap();
    bibles(&server, dir.path(), &[]).await;
    let created =
        ok(&server, "work.create", json!({ "title": "Stellenpredigt", "kind": "sermon", "text_ref": "joh 3,16" }))
            .await;
    let work = created["work"].as_i64().unwrap();
    let text = &created["sermon_text"];
    assert_eq!(text["quoted"], false, "ohne Bibel kein Wortlaut: {text}");
    let notice = text["notice"].as_str().expect("Hinweis");
    assert!(notice.contains("Keine SWORD-Bibel"), "Hinweis auf Deutsch, warum nur die Stelle dasteht: {notice}");
    let list = nodes(&server, work).await;
    assert_eq!(
        list[0]["revisions"][0]["body"], "= Predigttext\n\nJoh 3,16\n\n",
        "nur die Stellenangabe (einheitlich geschrieben), kein erfundener Wortlaut"
    );
    let modules = ok(&server, "bible.modules", json!({ "work": work })).await;
    assert!(modules["work_bible"].is_null(), "ohne Bibel wird keine SWORD-Bibel gesetzt: {modules}");
}

#[tokio::test]
async fn an_invalid_reference_creates_nothing_and_shows_the_form_again_with_error_and_inputs() {
    let server = common::open().await;
    let before = ok(&server, "work.list", json!({})).await["works"].as_array().unwrap().len();
    let refused =
        server.rpc("work.create", json!({ "title": "Fehlpredigt", "kind": "sermon", "text_ref": "Ps 151,1" })).await;
    assert_eq!(refused["error"]["code"], -32602, "ungültige Stelle → -32602: {refused}");
    let (status, _, page) = form(
        &server,
        "/werke/act",
        "action=create&title=Fehlpredigt&kind=sermon&preset=&text_ref=Hausbuch+9%2C9&module=&audience=Jugend",
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "Fehler als Seite mit Status 422, nicht als Klartext");
    assert!(page.contains("role=\"alert\"") && page.contains("Predigttext"), "Fehlermeldung auf der Seite: {page}");
    assert!(page.contains("value=\"Hausbuch 9,9\""), "die eingegebene Stelle steht wieder im Feld: {page}");
    assert!(page.contains("value=\"Fehlpredigt\"") && page.contains("value=\"Jugend\""), "übrige Eingaben bleiben");
    assert!(page.contains("<form class=\"work-form\" action=\"/werke/act\""), "das Formular steht wieder da");
    let after = ok(&server, "work.list", json!({})).await["works"].as_array().unwrap().len();
    assert_eq!(after, before, "nichts angelegt");
    let book = server.rpc("work.create", json!({ "title": "Buch mit Stelle", "text_ref": "Joh 3,16" })).await;
    assert_eq!(book["error"]["code"], -32602, "ein Predigttext gehört nur zur Predigt: {book}");
}

#[tokio::test]
async fn the_import_dialog_works_without_a_module_list_and_takes_the_bible_of_the_current_work() {
    let server = common::open().await;
    let dir = tempfile::tempdir().unwrap();
    bibles(&server, dir.path(), &[kiesel()]).await;
    let garden = server.import_garden().await.work;
    ok(&server, "bible.set_work", json!({ "work": garden, "module": "Kieselbibel" })).await;

    let (status, page) = server.get(&format!("/?work={garden}")).await;
    assert_eq!(status, StatusCode::OK, "die Arbeitsseite muss laden, sonst gibt es keinen Dialog „Importieren…“");
    let dialog = page.split("id=\"work-import\"").nth(1).expect("Dialog „Importieren…“ auf der Arbeitsseite");
    assert!(dialog.contains("<input type=\"text\" name=\"module\""), "ohne Modulliste ein Textfeld für das Modul");
    assert!(
        dialog.contains(&format!("name=\"from_work\" value=\"{garden}\"")),
        "das aktuelle Werk geht als from_work mit, damit seine Bibel gilt"
    );
    let (status, new_page) = server.get("/werke/neu").await;
    assert_eq!(status, StatusCode::OK, "/werke/neu muss laden, sonst lässt sich keine Bibel wählen");
    assert!(new_page.contains("<select name=\"module\">"), "/werke/neu zeigt eine Auswahl der SWORD-Bibeln");
    assert!(new_page.contains("value=\"Kieselbibel\""), "mit der Bibel aus dem Modulordner: {new_page}");

    let body = format!("action=create&title=Dialogpredigt&kind=sermon&text_ref=Joh+3%2C16&module=&from_work={garden}");
    let (status, headers, _) = form(&server, "/werke/act", &body).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Anlegen aus dem Dialog");
    let location = headers.get(header::LOCATION).unwrap().to_str().unwrap().to_owned();
    let work: i64 = location.strip_prefix("/?work=").and_then(|w| w.parse().ok()).unwrap();
    let list = nodes(&server, work).await;
    assert!(
        list[0]["revisions"][0]["body"].as_str().unwrap().contains("(Kieselbibel)])[Der Kiesel lag am Bach"),
        "leeres module: die Bibel des Ausgangswerks liefert den Wortlaut"
    );
    let modules = ok(&server, "bible.modules", json!({ "work": work })).await;
    assert_eq!(modules["work_bible"], "Kieselbibel", "… und wird die SWORD-Bibel der neuen Predigt");
}
