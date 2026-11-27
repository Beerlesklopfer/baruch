// SPDX-License-Identifier: AGPL-3.0-or-later
//! Images in the server (contract Round 6, section 4f): RPC `asset.*`, pages „Bilder“ without JavaScript, upload
//! (multipart) and forms, mirror commits per change. Real data folder (SQLite, Git mirror) in the temp folder; all
//! images are created in the test.

use crate::common;

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode, header};
use serde_json::{Value, json};
use tower::ServiceExt;

const HOUSE: &str =
    "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"8\" height=\"8\"><path d=\"M0 8 L4 0 L8 8 Z\"/></svg>\n";
const HOUSE_RED: &str = "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"8\" height=\"8\"><path fill=\"red\" d=\"M0 8 L4 0 L8 8 Z\"/></svg>\n";
/// PNG signature and invented bytes: enough for the type to be recognised.
const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR\0\0\0\x01\0\0\0\x01\x08\0\0\0\0erfunden";
const BOUNDARY: &str = "baruch-bild-grenze";

fn result(answer: &Value) -> &Value {
    assert!(answer["error"].is_null(), "RPC-Fehler: {answer}");
    &answer["result"]
}

/// Response as status, headers and bytes (images are not always UTF-8).
async fn send_raw(server: &common::Server, request: Request<Body>) -> (StatusCode, axum::http::HeaderMap, Vec<u8>) {
    let response = server.app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    (status, headers, to_bytes(response.into_body(), usize::MAX).await.unwrap().to_vec())
}

/// Multipart request to `POST /werke/bilder/hochladen` like a browser form.
fn upload(fields: &[(&str, &str)], file_name: &str, content: &[u8]) -> Request<Body> {
    let mut body = Vec::new();
    for (name, value) in fields {
        body.extend_from_slice(
            format!("--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n{value}\r\n").as_bytes(),
        );
    }
    body.extend_from_slice(
        format!(
            "--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{file_name}\"\r\n\
             Content-Type: application/octet-stream\r\n\r\n"
        )
        .as_bytes(),
    );
    body.extend_from_slice(content);
    body.extend_from_slice(format!("\r\n--{BOUNDARY}--\r\n").as_bytes());
    Request::post("/werke/bilder/hochladen")
        .header(header::CONTENT_TYPE, format!("multipart/form-data; boundary={BOUNDARY}"))
        .body(Body::from(body))
        .unwrap()
}

async fn add_house(server: &common::Server, work: i64) -> Value {
    let answer = server
        .rpc(
            "asset.add",
            json!({ "work": work, "file_name": "Haus.svg", "content": HOUSE, "encoding": "svg",
                    "path": "/bilder/haus.svg", "caption": "Das Haus", "alt": "Ein Haus mit Dach" }),
        )
        .await;
    result(&answer).clone()
}

fn git_log(server: &common::Server, format: &str) -> Option<String> {
    let dir = server.mirror_dir();
    common::git(&dir, &["log", &format!("--format={format}")])
}

#[tokio::test]
async fn the_rpc_methods_add_read_replace_switch_update_and_delete_images() {
    let server = common::open().await;
    let work = server.state.default_work().await.unwrap();
    let added = add_house(&server, work).await;
    assert_eq!((added["name"].as_str(), added["rev"].as_u64()), (Some("haus.svg"), Some(1)), "{added}");
    assert!(added["commit"].is_string(), "Hinzufügen ist ein Spiegel-Commit: {added}");
    let png = server
        .rpc(
            "asset.add",
            json!({ "file_name": "punkt.png", "content": baruch_store::encode_base64(PNG), "encoding": "base64" }),
        )
        .await;
    assert_eq!(result(&png)["name"], "punkt.png", "ohne work das Vorgabewerk, Base64 aus RPC: {png}");

    let list = server.rpc("asset.list", json!({ "work": work })).await;
    let names: Vec<&str> =
        result(&list)["assets"].as_array().unwrap().iter().map(|a| a["name"].as_str().unwrap()).collect();
    assert_eq!(names, ["haus.svg", "punkt.png"], "Liste nach Namen: {list}");

    let got = server.rpc("asset.get", json!({ "name": "haus.svg" })).await;
    let got = result(&got);
    assert_eq!(got["content"], HOUSE, "content ist ohne Angabe dabei");
    assert_eq!(got["encoding"], "svg", "Kodierung genannt");
    assert_eq!(got["asset"]["paths"], json!(["/bilder/haus.svg"]), "Pfade in der Übersicht");
    assert_eq!(got["revision"]["current"], true, "ohne rev die aktuelle Fassung");
    let meta = server.rpc("asset.get", json!({ "name": "punkt.png", "content": false })).await;
    assert!(result(&meta).get("content").is_none(), "content: false lässt den Inhalt weg: {meta}");
    assert_eq!(result(&meta)["encoding"], "base64", "PNG als Base64");

    let replaced = server
        .rpc("asset.replace", json!({ "name": "haus.svg", "file_name": "rot.svg", "content": HOUSE_RED, "encoding": "svg", "note": "rot" }))
        .await;
    assert_eq!(result(&replaced)["rev"], 2, "Ersetzen ergibt Fassung 2: {replaced}");
    let same = server
        .rpc(
            "asset.replace",
            json!({ "name": "haus.svg", "file_name": "rot.svg", "content": HOUSE_RED, "encoding": "svg" }),
        )
        .await;
    assert_eq!(same["error"]["code"], -32000, "unverändert ersetzt → abgelehnt: {same}");
    assert!(same["error"]["message"].as_str().unwrap().contains("unverändert"), "{same}");

    let switched = server.rpc("asset.set_current", json!({ "name": "haus.svg", "rev": 1 })).await;
    assert_eq!(result(&switched)["rev"], 1, "auf Fassung 1 umgeschaltet: {switched}");
    let revisions = server.rpc("asset.revisions", json!({ "name": "haus.svg" })).await;
    let currents: Vec<u64> = result(&revisions)["revisions"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["current"] == true)
        .map(|r| r["number"].as_u64().unwrap())
        .collect();
    assert_eq!(currents, [1], "genau eine aktuelle Fassung: {revisions}");

    let updated = server
        .rpc(
            "asset.update",
            json!({ "name": "haus.svg", "rev": 2, "alt": "Rotes Haus", "paths": ["/bilder/haus.svg", "/titel.svg"] }),
        )
        .await;
    assert_eq!(result(&updated)["changed"], true, "Alternativtext und Pfade geändert: {updated}");
    assert!(result(&updated)["commit"].is_string(), "ein Commit für beides: {updated}");
    let again = server.rpc("asset.update", json!({ "name": "haus.svg", "rev": 2, "alt": "Rotes Haus" })).await;
    assert_eq!(result(&again)["changed"], false, "nichts geändert: {again}");
    assert!(result(&again).get("commit").is_none(), "ohne Änderung kein Commit: {again}");

    let unconfirmed = server.rpc("asset.delete", json!({ "name": "haus.svg", "rev": 2 })).await;
    assert_eq!(unconfirmed["error"]["code"], -32602, "Löschen ohne confirm → -32602: {unconfirmed}");
    let current = server.rpc("asset.delete", json!({ "name": "haus.svg", "rev": 1, "confirm": true })).await;
    assert_eq!(current["error"]["code"], -32000, "die aktuelle Fassung lässt sich nicht löschen: {current}");
    let deleted = server.rpc("asset.delete", json!({ "name": "haus.svg", "rev": 2, "confirm": true })).await;
    assert_eq!((result(&deleted)["rev"].as_u64(), &result(&deleted)["deleted"]), (Some(2), &json!(true)), "{deleted}");
    let removed = server.rpc("asset.delete", json!({ "name": "punkt.png", "confirm": true })).await;
    assert_eq!(result(&removed)["deleted"], true, "ganzes Bild gelöscht: {removed}");
    assert!(result(&removed).get("rev").is_none(), "ohne rev keine Fassung in der Antwort: {removed}");

    let unknown = server.rpc("asset.get", json!({ "name": "fehlt.svg" })).await;
    assert_eq!(unknown["error"]["code"], -32602, "unbekanntes Bild → -32602: {unknown}");
    let broken = server.rpc("asset.add", json!({ "file_name": "x.png", "content": "@@", "encoding": "base64" })).await;
    assert_eq!(broken["error"]["code"], -32602, "kaputtes Base64 → -32602: {broken}");
    let wrong = server.rpc("asset.add", json!({ "file_name": "x.svg", "content": "<svg/>", "encoding": "text" })).await;
    assert_eq!(wrong["error"]["code"], -32602, "unbekannte Kodierung → -32602: {wrong}");
}

#[tokio::test]
async fn every_image_change_is_one_commit_with_trailers_and_deleting_keeps_the_content() {
    let server = common::open().await;
    let work = server.state.default_work().await.unwrap();
    let Some(before) = git_log(&server, "%H") else {
        eprintln!("git fehlt, Spiegel-Prüfung übersprungen");
        return;
    };
    let commits = |log: &str| log.lines().count();
    let start = commits(&before);
    add_house(&server, work).await;
    let dir = server.mirror_dir();
    assert_eq!(std::fs::read_to_string(dir.join("bilder/haus.svg")).unwrap(), HOUSE, "SVG-Text im Spiegel");
    server
        .rpc(
            "asset.add",
            json!({ "file_name": "punkt.png", "content": baruch_store::encode_base64(PNG), "encoding": "base64" }),
        )
        .await;
    let b64 = std::fs::read_to_string(dir.join("bilder/punkt.png.b64")).unwrap();
    assert!(b64.lines().all(|l| l.len() <= 76) && b64.ends_with('\n'), "Base64 in 76er-Zeilen: {b64}");
    server
        .rpc(
            "asset.replace",
            json!({ "name": "haus.svg", "file_name": "rot.svg", "content": HOUSE_RED, "encoding": "svg" }),
        )
        .await;
    assert_eq!(
        std::fs::read_to_string(dir.join("bilder/haus.svg")).unwrap(),
        HOUSE_RED,
        "Spiegel zeigt die aktuelle Fassung"
    );
    server.rpc("asset.set_current", json!({ "name": "haus.svg", "rev": 1 })).await;
    server.rpc("asset.update", json!({ "name": "haus.svg", "rev": 2, "caption": "Rot gestrichen" })).await;
    server.rpc("asset.delete", json!({ "name": "haus.svg", "rev": 2, "confirm": true })).await;
    server.rpc("asset.delete", json!({ "name": "punkt.png", "confirm": true })).await;
    assert!(!dir.join("bilder/punkt.png.b64").exists(), "gelöschtes Bild verschwindet aus dem Spiegel");

    let log = git_log(&server, "%s").unwrap();
    assert_eq!(commits(&log), start + 7, "sieben Änderungen, sieben Commits: {log}");
    let subjects: Vec<&str> = log.lines().take(7).collect();
    assert_eq!(
        subjects,
        [
            "Bild punkt.png: gelöscht",
            "Bild haus.svg: Fassung 2 gelöscht",
            "Bild haus.svg: Fassung 2 geändert",
            "Bild haus.svg: auf Fassung 1 umgeschaltet",
            "Bild haus.svg: Fassung 2 ersetzt",
            "Bild punkt.png: hinzugefügt",
            "Bild haus.svg: hinzugefügt",
        ],
        "Betreff je Änderung, neueste zuerst"
    );
    let trailers = git_log(&server, "%(trailers:only,unfold)").unwrap();
    for trailer in [
        "Baruch-Action: asset-added",
        "Baruch-Action: asset-replaced",
        "Baruch-Action: asset-switched",
        "Baruch-Action: asset-updated",
        "Baruch-Action: asset-deleted",
        "Baruch-Action: asset-removed",
        "Baruch-Asset: haus.svg",
        "Baruch-Revision: 2",
    ] {
        assert!(trailers.contains(trailer), "Trailer {trailer} fehlt: {trailers}");
    }
    let bodies = git_log(&server, "%s%n%b%n----").unwrap();
    let body_of = |subject: &str| {
        bodies.split("----").find(|b| b.trim_start().starts_with(subject)).unwrap_or_default().to_owned()
    };
    let updated = body_of("Bild haus.svg: Fassung 2 geändert");
    assert!(
        updated.contains("Bildunterschrift vorher: Das Haus")
            && updated.contains("Bildunterschrift nachher: Rot gestrichen"),
        "Ändern mit vorher/nachher: {updated}"
    );
    let deleted = body_of("Bild haus.svg: Fassung 2 gelöscht");
    assert!(deleted.contains(HOUSE_RED.trim_end()), "Löschen trägt den gelöschten SVG-Inhalt: {deleted}");
    let removed = body_of("Bild punkt.png: gelöscht");
    let encoded = baruch_store::encode_base64(PNG);
    assert!(removed.contains(&encoded), "das gelöschte Bild steht als Base64 im Rumpf: {removed}");
    let dir_status = common::git(&dir, &["status", "--porcelain"]).unwrap();
    assert!(dir_status.is_empty(), "Spiegel sauber: {dir_status}");
    // Changing and deleting a revision that is not current change no file: empty log commits.
    let names = common::git(&dir, &["show", "--name-only", "--format=", "HEAD~1"]).unwrap();
    assert!(names.trim().is_empty(), "Fassung löschen ändert keine Datei (leerer Commit): {names}");
}

#[tokio::test]
async fn the_image_pages_work_without_javascript() {
    let server = common::open().await;
    let work = server.state.default_work().await.unwrap();
    let (status, html) = server.get(&format!("/werke/{work}/bilder")).await;
    assert_eq!(status, StatusCode::OK, "Seite Bilder: {html}");
    assert!(html.contains("Noch keine Bilder"), "leere Liste mit Hinweis: {html}");
    assert!(
        html.contains("action=\"/werke/bilder/hochladen\" method=\"post\" enctype=\"multipart/form-data\""),
        "Formular „Bild hinzufügen“ ohne JavaScript: {html}"
    );
    add_house(&server, work).await;
    let (_, html) = server.get(&format!("/werke/{work}/bilder")).await;
    assert!(html.contains("<strong>haus.svg</strong>"), "Name in der Liste: {html}");
    assert!(
        html.contains(&format!("src=\"/werke/{work}/bilder/haus.svg/vorschau?b=320\"")),
        "Vorschau seit Runde 11 über …/vorschau (kleines Bild statt der ganzen Datei): {html}"
    );
    assert!(html.contains("<code>/bilder/haus.svg</code>"), "Pfad in der Liste, / nicht maskiert: {html}");
    assert!(html.contains("Ein Haus mit Dach"), "Alternativtext in der Liste: {html}");
    assert!(!html.contains("{{") && !html.contains("&#x2f;"), "keine Platzhalter, keine maskierten /: {html}");

    let (status, html) = server.get(&format!("/werke/{work}/bilder/haus.svg")).await;
    assert_eq!(status, StatusCode::OK, "Detailseite: {html}");
    for part in [
        "Fassung 1 (aktuell)",
        "name=\"action\" value=\"update\"",
        "name=\"action\" value=\"remove\"",
        "name=\"confirm\" value=\"1\" required",
        "Ersetzen (neue Fassung)",
        ">/bilder/haus.svg</textarea>",
    ] {
        assert!(html.contains(part), "{part} fehlt auf der Detailseite: {html}");
    }
    let (status, _) = server.get(&format!("/werke/{work}/bilder/fehlt.svg")).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "unbekanntes Bild → 404");

    let (status, headers, bytes) =
        send_raw(&server, Request::get(format!("/werke/{work}/bilder/haus.svg/datei")).body(Body::empty()).unwrap())
            .await;
    assert_eq!(status, StatusCode::OK, "Datei");
    assert_eq!(bytes, HOUSE.as_bytes(), "Bytes der aktuellen Fassung");
    assert_eq!(headers[header::CONTENT_TYPE], "image/svg+xml", "MIME-Typ");
    assert_eq!(headers[header::X_CONTENT_TYPE_OPTIONS], "nosniff", "nosniff");
    assert_eq!(
        headers[header::CONTENT_SECURITY_POLICY],
        "default-src 'none'; style-src 'unsafe-inline'; sandbox; frame-ancestors 'none'",
        "CSP mit sandbox: ein SVG führt nichts aus; frame-ancestors (no_framing): keine fremde Seite rahmt es ein"
    );
    let (status, _, bytes) = send_raw(
        &server,
        Request::get(format!("/werke/{work}/bilder/pfad?p=%2Fbilder%2Fhaus.svg")).body(Body::empty()).unwrap(),
    )
    .await;
    assert_eq!((status, bytes.as_slice()), (StatusCode::OK, HOUSE.as_bytes()), "Bild zum Pfad");
    let (status, _) = server.get(&format!("/werke/{work}/bilder/pfad?p=%2Fbilder%2Fanders.svg")).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "unbekannter Pfad → 404");

    let (status, html) = server.get(&format!("/?work={work}")).await;
    assert_eq!(status, StatusCode::OK, "Arbeitsbereich des Werks lädt");
    // Round 9: „Bilder“ is in the menu „Werk ▾“ of the header, right after the outline scheme.
    // Round 33 (ribbon): the menu Werk ▾ has become the tab Buch; „Bilder“ is there in the group Werk, still right
    // after the outline scheme and as an ordinary link.
    let menu = &html[html.find("data-band-panel=\"buch\"").expect("Feld des Reiters Buch")..];
    let menu = &menu[..menu.find("</details>").unwrap()];
    assert!(
        menu.contains(&format!("<a class=\"btn\" href=\"/werke/{work}/bilder\">Bilder</a>")),
        "Reiter Buch, Gruppe Werk, hat den Link „Bilder“ des Werks: {menu}"
    );
    assert!(
        menu.find(">Gliederungsschema</a>").expect("Gliederungsschema") < menu.find(">Bilder</a>").unwrap(),
        "„Bilder“ steht nach dem Gliederungsschema: {menu}"
    );
}

#[tokio::test]
async fn uploading_adds_or_replaces_and_too_large_is_413() {
    let server = common::open().await;
    let work = server.state.default_work().await.unwrap().to_string();
    let request = upload(
        &[("work", &work), ("path", "/bilder/haus.svg"), ("caption", "Haus"), ("alt", "Ein Haus"), ("note", "")],
        "Mein Haus.svg",
        HOUSE.as_bytes(),
    );
    let (status, headers, _) = server.send(request).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Hochladen gelingt: {headers:?}");
    assert_eq!(
        headers[header::LOCATION],
        format!("/werke/{work}/bilder/mein-haus.svg?hinweis=hinzugefuegt"),
        "weiter zur Detailseite mit Hinweis"
    );
    let (_, html) = server.get(headers[header::LOCATION].to_str().unwrap()).await;
    assert!(html.contains("Das Bild ist hinzugefügt."), "Hinweis auf der Seite: {html}");

    let request =
        upload(&[("work", &work), ("name", "mein-haus.svg"), ("note", "rot")], "rot.svg", HOUSE_RED.as_bytes());
    let (status, headers, _) = server.send(request).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "vorhandener Name → neue Fassung: {headers:?}");
    assert!(headers[header::LOCATION].to_str().unwrap().ends_with("hinweis=ersetzt"), "{headers:?}");
    let revisions =
        server.state.store.asset_revisions(server.state.default_work().await.unwrap(), "mein-haus.svg").await.unwrap();
    assert_eq!(revisions.len(), 2, "Ersetzen ist eine neue Fassung");
    assert!(revisions[1].current && revisions[1].note == "rot", "die neue ist aktuell: {revisions:?}");

    let request = upload(&[("work", &work)], "notiz.txt", b"kein Bild");
    let (status, _, html) = server.send(request).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "kein Bild → 422");
    assert!(html.contains("ist kein unterstütztes Bild"), "Meldung auf der Seite: {html}");

    let big = vec![b'x'; baruch_server::assets::UPLOAD_LIMIT + 1];
    let (status, _, html) = server.send(upload(&[("work", &work)], "gross.png", &big)).await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE, "über UPLOAD_LIMIT → 413: {html}");
    assert_eq!(server.state.store.asset_list(work.parse().unwrap()).await.unwrap().len(), 1, "nichts dazugekommen");
}

#[tokio::test]
async fn the_forms_switch_update_and_delete_only_with_confirmation() {
    let server = common::open().await;
    let work = server.state.default_work().await.unwrap();
    add_house(&server, work).await;
    server
        .rpc(
            "asset.replace",
            json!({ "name": "haus.svg", "file_name": "rot.svg", "content": HOUSE_RED, "encoding": "svg" }),
        )
        .await;

    let (status, location) =
        server.post_form("/werke/bilder/act", &format!("action=set_current&work={work}&name=haus.svg&rev=1")).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Umschalten über das Formular");
    assert_eq!(
        location.as_deref(),
        Some(format!("/werke/{work}/bilder/haus.svg?fassung=1&hinweis=umgeschaltet").as_str()),
        "nach dem Umschalten zurück auf die Bildseite mit Fassung 1 und Hinweis"
    );
    let (status, _) = server
        .post_form(
            "/werke/bilder/act",
            &format!("action=update&work={work}&name=haus.svg&rev=2&caption=Rot&alt=Rotes+Haus&note=&paths=%2Fbilder%2Fhaus.svg%0D%0A%2Fb.svg"),
        )
        .await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Ändern über das Formular");
    let list = server.state.store.asset_list(work).await.unwrap();
    assert_eq!(list[0].paths, ["/b.svg", "/bilder/haus.svg"], "Pfade aus dem Textfeld, eine je Zeile");
    let revisions = server.state.store.asset_revisions(work, "haus.svg").await.unwrap();
    assert_eq!((revisions[1].caption.as_str(), revisions[1].alt.as_str()), ("Rot", "Rotes Haus"), "Felder geändert");

    let (status, _) =
        server.post_form("/werke/bilder/act", &format!("action=delete&work={work}&name=haus.svg&rev=2")).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "ohne Kästchen wird nichts gelöscht");
    assert_eq!(server.state.store.asset_revisions(work, "haus.svg").await.unwrap().len(), 2, "beide Fassungen noch da");
    let (status, _) = server
        .post_form("/werke/bilder/act", &format!("action=delete&work={work}&name=haus.svg&rev=1&confirm=1"))
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "die aktuelle Fassung lässt sich nicht löschen → 409");
    let (status, _) = server
        .post_form("/werke/bilder/act", &format!("action=delete&work={work}&name=haus.svg&rev=2&confirm=1"))
        .await;
    assert_eq!(status, StatusCode::SEE_OTHER, "mit Kästchen gelöscht");
    let (status, _) = server.post_form("/werke/bilder/act", &format!("action=remove&work={work}&name=haus.svg")).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "Bild löschen ohne Kästchen → 422");
    let (status, location) =
        server.post_form("/werke/bilder/act", &format!("action=remove&work={work}&name=haus.svg&confirm=on")).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Bild gelöscht");
    assert_eq!(
        location.as_deref(),
        Some(format!("/werke/{work}/bilder?hinweis=geloescht").as_str()),
        "zurück zur Liste"
    );
    assert!(server.state.store.asset_list(work).await.unwrap().is_empty(), "kein Bild mehr");
}

#[tokio::test]
async fn image_forms_from_a_foreign_page_are_refused() {
    let server = common::open().await;
    let work = server.state.default_work().await.unwrap();
    let mut request = upload(&[("work", &work.to_string())], "haus.svg", HOUSE.as_bytes());
    request.headers_mut().insert(header::HOST, "127.0.0.1:8765".parse().unwrap());
    request.headers_mut().insert(header::ORIGIN, "https://evil.example".parse().unwrap());
    assert_eq!(server.send(request).await.0, StatusCode::FORBIDDEN, "Hochladen von fremder Seite → 403");
    add_house(&server, work).await;
    let request = Request::post("/werke/bilder/act")
        .header(header::HOST, "127.0.0.1:8765")
        .header(header::ORIGIN, "https://evil.example")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(format!("action=remove&work={work}&name=haus.svg&confirm=1")))
        .unwrap();
    assert_eq!(server.send(request).await.0, StatusCode::FORBIDDEN, "Formular von fremder Seite → 403");
    assert_eq!(server.state.store.asset_list(work).await.unwrap().len(), 1, "nichts gelöscht");
    let (status, _) = server.post_form("/act", &format!("method=asset.delete&work={work}&node=0&confirm=1")).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "Bilder gibt es nicht über /act");
}
