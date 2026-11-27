// SPDX-License-Identifier: AGPL-3.0-or-later
//! Managing works (contract Round 4, sections 4–6): create, change, delete and export over RPC and forms without
//! JavaScript, work choice and cookie, mirror commits and archive of deleted works; none of it over MCP. Real data
//! directory (SQLite, library, Git mirror) in the temp directory.

use crate::common;

use std::net::SocketAddr;

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode, header};
use baruch_server::rpc;
use serde_json::{Value, json};

/// Form to `uri`; answer as status, headers and HTML.
async fn form(server: &common::Server, uri: &str, body: &str) -> (StatusCode, axum::http::HeaderMap, String) {
    let request = Request::post(uri)
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(body.to_owned()))
        .unwrap();
    server.send(request).await
}

fn location(headers: &axum::http::HeaderMap) -> String {
    headers.get(header::LOCATION).map(|v| v.to_str().unwrap().to_owned()).unwrap_or_default()
}

fn result(answer: &Value) -> &Value {
    assert!(answer["error"].is_null(), "Fehler statt Ergebnis: {answer}");
    &answer["result"]
}

#[tokio::test]
async fn creating_a_book_over_rpc_shows_it_on_the_page_and_commits_it() {
    let server = common::open().await;
    let answer = server.rpc("work.create", json!({ "title": "Das Wanderbuch", "author": "Wanda Weg" })).await;
    let created = result(&answer);
    let work = created["work"].as_i64().unwrap();
    assert_eq!(
        (created["slug"].as_str(), created["nodes"].as_u64()),
        (Some("das-wanderbuch"), Some(1)),
        "slug und ein Kapitel: {answer}"
    );
    assert!(created["commit"].is_string(), "Anlegen committet in den neuen Spiegel: {answer}");

    let request = Request::get(format!("/?work={work}")).body(Body::empty()).unwrap();
    let (status, headers, html) = server.send(request).await;
    assert_eq!(status, StatusCode::OK, "Seite des neuen Werks lädt");
    assert!(html.contains("<span class=\"work-title\">Das Wanderbuch</span>"), "Titel in der Kopfzeile");
    assert!(html.contains("Erstes Kapitel"), "Gliederung zeigt das erste Kapitel");
    assert_eq!(
        headers.get(header::SET_COOKIE).and_then(|v| v.to_str().ok()),
        Some(format!("baruch_work={work}; Path=/; Max-Age=31536000; SameSite=Lax").as_str()),
        "die Seite merkt sich das Werk im Cookie"
    );

    let dir = server.mirror_dir_of("das-wanderbuch");
    let vorlage = std::fs::read_to_string(dir.join("vorlage.typ")).unwrap();
    assert!(
        vorlage.starts_with("#set document(title: \"Das Wanderbuch\", author: \"Wanda Weg\")"),
        "Werkvorlage im Spiegel: {vorlage}"
    );
    let Some(log) = common::git(&dir, &["log", "--format=%s|%an"]) else { return };
    assert_eq!(log, "Werk angelegt: Das Wanderbuch|Autor\n", "genau ein Commit des Menschen beim Anlegen");
}

#[tokio::test]
async fn the_form_creates_a_sermon_with_its_outline_without_javascript() {
    let server = common::open().await;
    let (status, headers, _) = form(
        &server,
        "/werke/act",
        "action=create&title=Pfingsten&author=&kind=sermon&preset=&subtitle=&audience=Gemeinde&bible=",
    )
    .await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Anlegen leitet weiter");
    let target = location(&headers);
    let work: i64 = target.strip_prefix("/?work=").and_then(|w| w.parse().ok()).expect("Weiterleitung auf /?work=<id>");
    let answer = server.rpc("work.get", json!({ "work": work })).await;
    let titles: Vec<&str> =
        result(&answer)["work"]["nodes"].as_array().unwrap().iter().map(|n| n["title"].as_str().unwrap()).collect();
    assert_eq!(
        titles,
        ["Predigttext", "Hinführung", "Beobachtung am Text", "Deutung", "Zuspruch", "Anwendung", "Ausblick"],
        "Predigt ohne Grundgerüst-Angabe: Predigtvorlage aus Runde 16a (Predigttext, Einleitung, drei Hauptpunkte, Anwendung, Schluss)"
    );
    let meta = server.rpc("work.meta", json!({ "work": work })).await;
    assert_eq!(result(&meta)["work"]["kind"], "sermon", "Art Predigt: {meta}");
    assert_eq!(result(&meta)["work"]["audience"], "Gemeinde", "Zielgruppe gespeichert: {meta}");
}

#[tokio::test]
async fn a_title_that_is_taken_is_refused_everywhere() {
    let server = common::open().await;
    let garden = server.import_garden().await;
    let taken = server.rpc("work.create", json!({ "title": "  das gartenbuch " })).await;
    assert_eq!(taken["error"]["code"], rpc::REFUSED, "Doppel beim Anlegen → -32000: {taken}");
    assert_eq!(
        taken["error"]["message"], "Es gibt schon ein Werk „Das Gartenbuch“ (das-gartenbuch).",
        "Meldung nennt das vorhandene Werk"
    );
    let empty = server.rpc("work.create", json!({ "title": " " })).await;
    assert_eq!(empty["error"]["code"], rpc::INVALID_PARAMS, "leerer Titel → -32602: {empty}");

    let sample = server.state.work;
    let rename = server.rpc("work.update", json!({ "work": sample, "title": "DAS GARTENBUCH" })).await;
    assert_eq!(rename["error"]["code"], rpc::REFUSED, "Umbenennen auf einen vergebenen Titel → -32000: {rename}");
    let blank = server.rpc("work.update", json!({ "work": sample, "title": "" })).await;
    assert_eq!(blank["error"]["code"], rpc::INVALID_PARAMS, "Umbenennen auf leer → -32602: {blank}");
    let no_work = server.rpc("work.update", json!({ "title": "Ohne Werk" })).await;
    assert_eq!(no_work["error"]["code"], rpc::INVALID_PARAMS, "work.update ohne work → -32602: {no_work}");

    let other = common::GARDEN.replace("Gießen, gießen, gießen.", "Ernten.");
    let upload = server.rpc("work.import", json!({ "file_name": "garten-2.typ", "source": other })).await;
    assert_eq!(upload["error"]["code"], rpc::REFUSED, "Import mit vergebenem Titel → -32000: {upload}");
    let titled = server
        .rpc("work.import", json!({ "file_name": "garten-2.typ", "source": other, "title": "Gartenbuch, Band 2" }))
        .await;
    assert_eq!(result(&titled)["title"], "Gartenbuch, Band 2", "mit eigenem Titel gelingt der Import: {titled}");
    let txt = server.rpc("work.import", json!({ "file_name": "notiz.txt", "source": "= A\n" })).await;
    assert_eq!(txt["error"]["code"], rpc::INVALID_PARAMS, "nur .typ: {txt}");

    let (status, _, html) = form(&server, "/werke/act", "action=create&title=Das+Gartenbuch&kind=book").await;
    assert_eq!(status, StatusCode::CONFLICT, "Formular mit vergebenem Titel → 409");
    assert!(
        html.contains("Es gibt schon ein Werk „Das Gartenbuch“ (das-gartenbuch)."),
        "Fehlerseite nennt den Grund: {html}"
    );
    assert!(html.contains("value=\"Das Gartenbuch\""), "Fehlerseite zeigt das Formular mit der Eingabe wieder");
    let list = server.rpc("work.list", json!({})).await;
    assert_eq!(
        result(&list)["works"].as_array().unwrap().len(),
        3,
        "Beispielwerk, Gartenbuch, Band 2 – kein weiteres: {list}"
    );
    let meta = server.rpc("work.meta", json!({ "work": garden.work })).await;
    assert_eq!(result(&meta)["work"]["title"], "Das Gartenbuch", "das vorhandene Werk behält seinen Titel: {meta}");
}

#[tokio::test]
async fn updating_the_template_makes_a_new_revision_and_mirrors_it() {
    let server = common::open().await;
    let garden = server.import_garden().await;
    let dir = server.mirror_dir_of(&garden.slug);
    let template = server.state.store.work_template(garden.work).await.unwrap();
    let changed = format!("{template}#set par(justify: true)\n");
    let answer = server
        .rpc(
            "work.update",
            json!({ "work": garden.work, "template": changed, "note": "Blocksatz", "subtitle": "Für Anfänger" }),
        )
        .await;
    let update = result(&answer);
    assert_eq!(update["changed"], json!(["subtitle", "template"]), "Untertitel und Vorlage geändert: {answer}");
    assert_eq!(update["template_rev"], 2, "neue Fassung 2 der Werkvorlage");
    assert!(update["commit"].is_string(), "geänderte vorlage.typ ergibt einen Commit: {answer}");
    assert_eq!(std::fs::read_to_string(dir.join("vorlage.typ")).unwrap(), changed, "vorlage.typ im Spiegel geändert");

    let meta = server.rpc("work.meta", json!({ "work": garden.work })).await;
    let revisions = result(&meta)["template_revisions"].as_array().unwrap().clone();
    assert_eq!(revisions.len(), 2, "zwei Fassungen: {meta}");
    assert_eq!(revisions[1]["note"], "Blocksatz", "Notiz der neuen Fassung");
    let (status, page) = server.get(&format!("/werke/{}/bearbeiten?vorlage=1", garden.work)).await;
    assert_eq!(status, StatusCode::OK, "Bearbeiten-Seite lädt");
    assert!(page.contains("Fassung 1 (nur lesen)"), "alte Fassung wird gezeigt: {page}");
    assert!(page.contains("#let beet(body) = block(inset: 4pt, body)"), "alte Fassung lesbar");
    assert!(page.contains("value=\"Für Anfänger\""), "Formular zeigt den gespeicherten Untertitel");

    let only_meta = server.rpc("work.update", json!({ "work": garden.work, "audience": "Kleingärtner" })).await;
    assert!(
        result(&only_meta)["commit"].is_null(),
        "nur die Zielgruppe: keine Datei ändert sich, kein Commit: {only_meta}"
    );
    let Some(log) = common::git(&dir, &["log", "--format=%s"]) else { return };
    assert_eq!(
        log, "Werk geändert: Untertitel, Werkvorlage Fassung 2\nImport aus gartenbuch.typ\n",
        "Betreff nennt die Felder und die Fassung der Vorlage"
    );
}

#[tokio::test]
async fn the_edit_form_saves_without_javascript_and_shows_errors_as_a_page() {
    let server = common::open().await;
    let garden = server.import_garden().await;
    let body = format!(
        "action=update&work={}&title=Gartenbuch+neu&subtitle=&author=Grete+G%C3%A4rtner&kind=book&audience=&bible=Luther",
        garden.work
    );
    let (status, headers, _) = form(&server, "/werke/act", &body).await;
    assert_eq!(
        (status, location(&headers)),
        (StatusCode::SEE_OTHER, format!("/?work={}", garden.work)),
        "Speichern leitet zum Werk"
    );
    let meta = server.rpc("work.meta", json!({ "work": garden.work })).await;
    assert_eq!(
        (result(&meta)["work"]["title"].as_str(), result(&meta)["work"]["bible"].as_str()),
        (Some("Gartenbuch neu"), Some("Luther")),
        "gespeichert: {meta}"
    );

    let sample_title = server.state.store.works().await.unwrap()[0].2.clone();
    let body = format!("action=update&work={}&title={}", garden.work, baruch_server::view::query_escape(&sample_title));
    let (status, _, html) = form(&server, "/werke/act", &body).await;
    assert_eq!(status, StatusCode::CONFLICT, "vergebener Titel → 409");
    assert!(html.contains("Es gibt schon ein Werk"), "Fehlerseite mit Grund: {html}");
}

#[tokio::test]
async fn deleting_needs_the_typed_title_and_archives_the_mirror_with_a_last_commit() {
    let server = common::open().await;
    let garden = server.import_garden().await;
    let posted =
        server.rpc("post.add", json!({ "work": garden.work, "node": 1, "body": "Wann kommen die Radieschen?" })).await;
    result(&posted);
    let sample = server.state.work;
    let sample_before = server.rpc("work.export", json!({ "work": sample })).await;

    let wrong = server.rpc("work.delete", json!({ "work": garden.work, "confirm_title": "Das Gartenbuc" })).await;
    assert_eq!(wrong["error"]["code"], rpc::REFUSED, "falscher Titel → -32000: {wrong}");
    assert_eq!(wrong["error"]["message"], "Der eingegebene Titel passt nicht; nichts gelöscht.", "deutsche Meldung");
    let missing = server.rpc("work.delete", json!({ "confirm_title": "Das Gartenbuch" })).await;
    assert_eq!(missing["error"]["code"], rpc::INVALID_PARAMS, "work.delete ohne work → -32602: {missing}");
    assert!(server.mirror_dir_of(&garden.slug).is_dir(), "nach Ablehnung ist der Spiegel noch da");

    let answer = server.rpc("work.delete", json!({ "work": garden.work, "confirm_title": "das gartenbuch" })).await;
    let deleted = result(&answer);
    assert_eq!(
        (deleted["deleted"].as_i64(), deleted["title"].as_str()),
        (Some(garden.work), Some("Das Gartenbuch")),
        "Antwort nennt das Werk: {answer}"
    );
    assert_eq!(deleted["next"].as_i64(), Some(sample), "danach ist das Beispielwerk die Vorgabe");
    assert!(deleted["mirror"].is_null(), "Spiegel ohne Fehler: {answer}");
    assert!(!server.mirror_dir_of(&garden.slug).exists(), "der Spiegelordner ist weg");
    let trash = server.path().join("works/.geloescht");
    let names: Vec<String> =
        std::fs::read_dir(&trash).unwrap().map(|e| e.unwrap().file_name().into_string().unwrap()).collect();
    assert_eq!(names.len(), 1, "genau ein Archiv: {names:?}");
    let name = &names[0];
    let date = name.strip_prefix("das-gartenbuch-").expect("Archiv heißt <slug>-<datum>");
    assert!(date.len() == 10 && date.as_bytes()[4] == b'-' && date.as_bytes()[7] == b'-', "Datum JJJJ-MM-TT: {name}");
    assert_eq!(
        deleted["archive"].as_str(),
        Some(format!("works/.geloescht/{name}").as_str()),
        "Antwort nennt das Archiv"
    );

    for sql in [
        "SELECT count(*) FROM work WHERE id = ?",
        "SELECT count(*) FROM node WHERE work_id = ?",
        "SELECT count(*) FROM work_template_revision WHERE work_id = ?",
    ] {
        let left: i64 = sqlx::query_scalar(sql).bind(garden.work).fetch_one(server.state.store.pool()).await.unwrap();
        assert_eq!(left, 0, "nichts übrig: {sql}");
    }
    let options = baruch_library::SearchOptions { all_revisions: true, ..Default::default() };
    let hits = baruch_library::search(server.state.store.pool(), "Radieschen", &options).await.unwrap();
    assert!(hits.is_empty(), "Volltext: weder Revisionen noch Beiträge des gelöschten Werks: {hits:?}");
    assert_eq!(
        server.rpc("work.export", json!({ "work": sample })).await,
        sample_before,
        "das andere Werk bleibt unberührt"
    );

    let archive = trash.join(name);
    let Some(log) = common::git(&archive, &["log", "--format=%s|%an"]) else { return };
    assert_eq!(
        log, "Werk gelöscht: Das Gartenbuch|Autor\nImport aus gartenbuch.typ|Autor\n",
        "Protokoll-Commit in der Historie des Archivs"
    );
    let message = common::git(&archive, &["log", "-1", "--format=%B"]).unwrap();
    assert!(message.contains("Baruch-Action: work-deleted"), "Trailer des Protokolls: {message}");
    let files = common::git(&archive, &["show", "--stat", "--format=", "HEAD"]).unwrap();
    assert_eq!(files.trim(), "", "der Protokoll-Commit ändert keine Datei (leerer Commit): {files}");

    // The same book once more: new id, and the slug of the deleted work stays reserved (contract Round 5), so that its
    // archive and a new mirror are not confused.
    let again = server.import_garden().await;
    assert!(again.work > garden.work, "die id des gelöschten Werks wird nicht wiederverwendet: {again:?}");
    assert_eq!(again.slug, "das-gartenbuch-2", "der alte slug bleibt reserviert, der neue bekommt -2: {again:?}");
    let second = server.rpc("work.delete", json!({ "work": again.work, "confirm_title": "Das Gartenbuch" })).await;
    assert_eq!(
        result(&second)["archive"].as_str(),
        Some(format!("works/.geloescht/das-gartenbuch-2-{date}").as_str()),
        "das zweite Werk hat einen eigenen Ordner und damit ein eigenes Archiv: {second}"
    );
}

#[tokio::test]
async fn after_deleting_the_first_work_page_and_rpc_use_the_next_and_without_works_lead_to_the_list() {
    let server = common::open().await;
    let garden = server.import_garden().await;
    let sample_title = server.state.store.works().await.unwrap()[0].2.clone();
    let body = format!(
        "action=delete&work={}&confirm_title={}",
        server.state.work,
        baruch_server::view::query_escape(&sample_title)
    );
    let (status, headers, _) = form(&server, "/werke/act", &body).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Löschen über das Formular leitet weiter");
    assert!(location(&headers).starts_with("/werke?geloescht="), "zurück zur Werkliste: {}", location(&headers));
    let (status, list) = server.get(&location(&headers)).await;
    assert_eq!(status, StatusCode::OK, "Werkliste lädt");
    assert!(list.contains("ist gelöscht"), "Hinweis auf das gelöschte Werk: {list}");

    let (status, html) = server.get("/").await;
    assert_eq!(status, StatusCode::OK, "Seite ohne ?work lädt");
    assert!(html.contains("<span class=\"work-title\">Das Gartenbuch</span>"), "jetzt ist das Gartenbuch die Vorgabe");
    let node = server.rpc("node.get", json!({ "node": 1 })).await;
    assert_eq!(result(&node)["title"], "Aussaat", "RPC ohne work nimmt das nächste Werk: {node}");

    let (status, _, page) =
        form(&server, "/werke/act", &format!("action=delete&work={}&confirm_title=Falsch", garden.work)).await;
    assert_eq!(status, StatusCode::CONFLICT, "falscher Titel im Formular → 409");
    assert!(page.contains("Der eingegebene Titel passt nicht"), "Fehlerseite nennt den Grund: {page}");
    let gone = server.rpc("work.delete", json!({ "work": garden.work, "confirm_title": "Das Gartenbuch" })).await;
    assert!(result(&gone)["next"].is_null(), "kein Werk mehr: {gone}");

    let request = Request::get("/").body(Body::empty()).unwrap();
    let (status, headers, _) = server.send(request).await;
    assert_eq!(
        (status, location(&headers).as_str()),
        (StatusCode::SEE_OTHER, "/werke"),
        "ohne Werke leitet / auf /werke"
    );
    let (status, list) = server.get("/werke").await;
    assert_eq!(status, StatusCode::OK, "Werkliste geht auch ohne Werke");
    assert!(list.contains("Noch kein Werk"), "leere Liste mit Hinweis: {list}");
    let works = server.rpc("work.list", json!({})).await;
    assert_eq!(result(&works)["works"], json!([]), "work.list geht ohne Werke");
    let none = server.rpc("node.get", json!({ "node": 0 })).await;
    assert_eq!(none["error"]["code"], rpc::INVALID_PARAMS, "ohne Werke gibt es kein Vorgabewerk: {none}");
    let created = server.rpc("work.create", json!({ "title": "Neuanfang" })).await;
    assert!(result(&created)["work"].is_i64(), "anlegen geht auch ohne Werke: {created}");
}

#[tokio::test]
async fn the_page_opens_the_work_from_the_cookie_and_falls_back_when_it_is_gone() {
    let server = common::open().await;
    let garden = server.import_garden().await;
    let get = |cookie: String, uri: &str| Request::get(uri).header(header::COOKIE, cookie).body(Body::empty()).unwrap();

    let (_, headers, html) = server.send(get(format!("baruch_theme=dark; baruch_work={}", garden.work), "/")).await;
    assert!(html.contains("<span class=\"work-title\">Das Gartenbuch</span>"), "ohne ?work das Werk aus dem Cookie");
    assert!(
        headers
            .get(header::SET_COOKIE)
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with(&format!("baruch_work={};", garden.work)),
        "Cookie wird bei jeder Antwort gesetzt"
    );
    let (_, headers, html) = server.send(get("baruch_work=999".into(), "/")).await;
    assert!(
        !html.contains("<span class=\"work-title\">Das Gartenbuch</span>"),
        "unbekanntes Werk im Cookie → Vorgabewerk"
    );
    assert!(
        headers
            .get(header::SET_COOKIE)
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with(&format!("baruch_work={};", server.state.work)),
        "der Cookie zeigt danach auf das gezeigte Werk"
    );
    let (_, _, html) =
        server.send(get(format!("baruch_work={}", garden.work), &format!("/?work={}", server.state.work))).await;
    assert!(!html.contains("<span class=\"work-title\">Das Gartenbuch</span>"), "?work= geht dem Cookie vor");
}

#[tokio::test]
async fn the_header_offers_works_and_an_import_dialog() {
    let server = common::open().await;
    let (_, html) = server.get("/").await;
    // Round 9: „Alle Werke“ and „Neues Werk / Importieren…“ are in the menu „Werk ▾“ of the header.
    // Round 33 (ribbon): both are now in the menu „Baruch ▾“ (program) – the same links and hooks.
    assert!(
        html.contains("<a class=\"menu-item\" role=\"menuitem\" href=\"/werke\">Alle Werke</a>"),
        "Link „Alle Werke“ im Menü der Kopfzeile"
    );
    assert!(
        html.contains("href=\"/werke/neu\" data-work-import-open"),
        "„Importieren…“ führt ohne JavaScript nach /werke/neu"
    );
    assert!(html.contains(">Neues Werk / Importieren…</a>"), "Beschriftung enthält „Importieren…“");
    assert!(
        html.contains("<dialog class=\"work-dialog\" id=\"work-import\" data-widget=\"work-import\""),
        "Dialog für JavaScript"
    );
    assert!(
        html.contains("action=\"/werke/import\" method=\"post\" enctype=\"multipart/form-data\""),
        "im Dialog das Upload-Formular"
    );
    assert!(!html.contains("{{") && !html.contains("{%"), "keine Reste von Platzhaltern");
    assert!(server_script_has("works.js"), "works.js wird ausgeliefert");
}

fn server_script_has(name: &str) -> bool {
    baruch_server::SCRIPTS.iter().any(|(file, body)| *file == name && body.contains("work-select"))
}

#[tokio::test]
async fn the_works_pages_list_every_work_and_work_without_javascript() {
    let server = common::open().await;
    let garden = server.import_garden().await;
    let (status, html) = server.get("/werke").await;
    assert_eq!(status, StatusCode::OK, "Werkliste lädt");
    for part in [
        "Das Gartenbuch".to_owned(),
        "Grete Gärtner".to_owned(),
        format!("href=\"/?work={}\">Öffnen</a>", garden.work),
        format!("href=\"/werke/{}/bearbeiten\">Ändern</a>", garden.work),
        format!("href=\"/werke/{}/export\" download>Export</a>", garden.work),
        format!("href=\"/werke/{}/loeschen\">Löschen</a>", garden.work),
        "<td class=\"number\">4</td>".to_owned(),
    ] {
        assert!(html.contains(&part), "Werkliste zeigt {part}: {html}");
    }
    let (status, new) = server.get("/werke/neu").await;
    assert_eq!(status, StatusCode::OK, "Seite „Neues Werk“ lädt");
    assert!(
        new.contains("<input type=\"file\" name=\"file\" accept=\".typ,.md,.markdown,.zip\">"),
        "Datei-Feld für .typ, Markdown und ZIP (seit R21b ohne required, weil auch das Ordner-Feld genügt): {new}"
    );
    assert!(new.contains("name=\"files\" multiple webkitdirectory"), "Ordner-Feld (R21b): {new}");
    assert!(new.contains("<input type=\"hidden\" name=\"action\" value=\"create\">"), "Anlege-Formular an /werke/act");
    let (status, delete) = server.get(&format!("/werke/{}/loeschen", garden.work)).await;
    assert_eq!(status, StatusCode::OK, "Lösch-Seite lädt");
    assert!(delete.contains("name=\"confirm_title\" required"), "Titel eintippen ist Pflicht");
    assert!(delete.contains("works/.geloescht/das-gartenbuch-JJJJ-MM-TT/"), "Seite nennt das Archiv");
    assert_eq!(server.get("/werke/999/bearbeiten").await.0, StatusCode::NOT_FOUND, "unbekanntes Werk → 404");
}

#[tokio::test]
async fn the_export_route_downloads_the_typst_file() {
    let server = common::open().await;
    let garden = server.import_garden().await;
    let request = Request::get(format!("/werke/{}/export", garden.work)).body(Body::empty()).unwrap();
    let (status, headers, body) = server.send(request).await;
    assert_eq!(status, StatusCode::OK, "Export lädt");
    assert_eq!(headers[header::CONTENT_TYPE], "text/plain; charset=utf-8", "Text in UTF-8");
    assert_eq!(
        headers[header::CONTENT_DISPOSITION],
        "attachment; filename=\"das-gartenbuch.typ\"",
        "Download als <slug>.typ"
    );
    assert_eq!(
        baruch_typst::xref::strip_key_labels(&body),
        common::GARDEN,
        "Inhalt = export_typst, nach dem Import die Datei bis auf die Sprungmarken"
    );
    assert_eq!(server.get("/werke/999/export").await.0, StatusCode::NOT_FOUND, "unbekanntes Werk → 404");
}

#[tokio::test]
async fn writing_work_routes_refuse_foreign_origins() {
    let server = common::open().await;
    let before = server.state.store.works().await.unwrap().len();
    let request = Request::post("/werke/act")
        .header(header::HOST, "127.0.0.1:8765")
        .header(header::ORIGIN, "https://evil.example")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from("action=create&title=Fremd"))
        .unwrap();
    assert_eq!(server.send(request).await.0, StatusCode::FORBIDDEN, "fremde Seite → 403");
    assert_eq!(server.state.store.works().await.unwrap().len(), before, "nichts angelegt");
}

#[tokio::test]
async fn mcp_offers_no_tool_to_create_change_delete_or_import_works() {
    let server = common::open().await;
    let message = json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list" });
    let mut request = Request::post("/mcp")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(message.to_string()))
        .unwrap();
    request.extensions_mut().insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 40000))));
    let (status, _, body) = server.send(request).await;
    assert_eq!(status, StatusCode::OK, "tools/list antwortet: {body}");
    let answer: Value = serde_json::from_str(&body).unwrap();
    let names: Vec<&str> =
        answer["result"]["tools"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap()).collect();
    assert!(names.contains(&"list_works"), "Lesen bleibt: {names:?}");
    // Ideas (idea_*, Round 7) and own posts (post_*, Round 8) are not works; managing them
    // over MCP is intended (IDEA_CRUD or only own posts).
    // The outline tools outline_* (wish 2026-11-06) concern items, not works; the
    // image tools asset_* (Round 11) concern images and change only through requests.
    for name in names.iter().filter(|n| {
        !n.starts_with("idea_") && !n.starts_with("post_") && !n.starts_with("outline_") && !n.starts_with("asset_")
    }) {
        let bad = ["create", "update", "delete", "import", "rename", "work_meta"].iter().any(|w| name.contains(w));
        assert!(!bad, "MCP darf Werke nicht anlegen, ändern, löschen oder importieren: {name}");
    }
    let call = json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": { "name": "work.delete", "arguments": {} } });
    let mut request = Request::post("/mcp")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(call.to_string()))
        .unwrap();
    request.extensions_mut().insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 40000))));
    let (_, _, body) = server.send(request).await;
    let answer: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(answer["error"]["code"], rpc::INVALID_PARAMS, "RPC-Methode als MCP-Werkzeug → unbekannt: {answer}");
}
