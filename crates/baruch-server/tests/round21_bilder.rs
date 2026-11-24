// SPDX-License-Identifier: AGPL-3.0-or-later
//! Runde 21c (bilder): Bilder (Typwechsel beim Ersetzen, Pfadkonflikte im Altbestand, Kontingent für MCP, Aufräumen
//! entschiedener Anträge), EPUB-Cover als Rasterbild der Vorderseite. Tests schreibt der Besitzer (Vertrag R21, S1 legt
//! nur die Datei an).
//!
//! Dazu: Maskierung auf den Seiten „Bilder“ und „Bild“, SVG mit Skript über `/werke/{id}/bilder/pfad`, Kopfzeilen der
//! Antragsdatei, Bilder in der Literatur, `LibraryView::images`, Bild einfügen in einer mehrzeiligen Fußnote und die
//! Meldung von `pdfinfo` bei allen PDFs von Baruch. Echter Datenordner im Temp-Ordner, Router über `oneshot`; kein
//! Eliezer, kein Netz. Werke und Bilder frei erfunden und im Test erzeugt; PDFs und Typst-Satz unter
//! `common::PDF_SERIAL`.

use crate::common;

use std::io::Read as _;
use std::net::SocketAddr;

use axum::body::{Body, to_bytes};
use axum::extract::ConnectInfo;
use axum::http::{HeaderMap, Request, StatusCode, header};
use serde_json::{Value, json};
use tower::ServiceExt;

/// Frei erfundenes Buch mit einem Bild unter `bilder/haus.svg` und festem Seitenformat (120 × 180 mm).
const BOOK: &str = "#set document(title: \"Das Haus am Hang\")\n#set page(width: 120mm, height: 180mm)\n\
#set text(lang: \"de\")\n\n= Das Haus\nDas Dach ist neu gedeckt.\n\n\
#figure(image(\"bilder/haus.svg\", width: 40mm), caption: [Das Haus])\n\n= Der Garten\nDie Bohnen ranken.\n";

const HOUSE_SVG: &str = "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"40\" height=\"20\">\
<rect width=\"40\" height=\"20\" fill=\"#3a6\"/></svg>\n";

const BOUNDARY: &str = "----baruch-r21c-bilder";

fn result(answer: &Value) -> &Value {
    assert!(answer["error"].is_null(), "RPC-Fehler: {answer}");
    &answer["result"]
}

/// Eine SVG-Grafik mit eigener Farbe (verschiedene Inhalte für verschiedene Anträge).
fn svg(n: usize) -> String {
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"8\" height=\"4\"><rect width=\"8\" height=\"4\" \
         fill=\"#{n:06x}\"/></svg>\n"
    )
}

async fn get_bytes(server: &common::Server, uri: &str) -> (StatusCode, HeaderMap, Vec<u8>) {
    let response = server.app.clone().oneshot(Request::get(uri).body(Body::empty()).unwrap()).await.unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    (status, headers, to_bytes(response.into_body(), usize::MAX).await.unwrap().to_vec())
}

fn commits(server: &common::Server, slug: &str) -> usize {
    common::git(&server.mirror_dir_of(slug), &["rev-list", "--count", "HEAD"]).map_or(0, |n| n.trim().parse().unwrap())
}

/// Werk „Haus am Hang“ mit dem Bild `haus.svg` unter `bilder/haus.svg`: (id, slug).
async fn house(server: &common::Server) -> (i64, String) {
    let report = server.import(BOOK, "haus.typ").await;
    let added = server
        .rpc(
            "asset.add",
            json!({ "work": report.work, "file_name": "haus.svg", "content": HOUSE_SVG, "encoding": "svg",
                    "path": "bilder/haus.svg", "caption": "Das Haus", "alt": "Ein grünes Haus" }),
        )
        .await;
    assert_eq!(result(&added)["name"], "haus.svg", "Bild des Menschen angelegt: {added}");
    (report.work, report.slug)
}

/// `multipart/form-data` an `/werke/bilder/hochladen` mit Textfeldern und einer Datei.
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

/// Größen (Breite, Höhe) der Rasterbilder in einem PDF laut `pdfimages -list`; `None`, wenn es das Programm nicht gibt.
fn pdf_images(pdf: &[u8]) -> Option<Vec<(u32, u32)>> {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("satz.pdf");
    std::fs::write(&path, pdf).unwrap();
    let output = std::process::Command::new("pdfimages").arg("-list").arg(&path).output().ok()?;
    assert!(output.status.success(), "pdfimages: {}", String::from_utf8_lossy(&output.stderr));
    let text = String::from_utf8_lossy(&output.stdout).into_owned();
    // Kopf zwei Zeilen; Spalten: page num type width height …
    Some(
        text.lines()
            .skip(2)
            .filter_map(|line| {
                let cols: Vec<&str> = line.split_whitespace().collect();
                (cols.len() > 4 && cols[2] == "image").then(|| (cols[3].parse().ok(), cols[4].parse().ok()))
            })
            .filter_map(|(w, h)| Some((w?, h?)))
            .collect(),
    )
}

async fn book_pdf(server: &common::Server, work: i64) -> Vec<u8> {
    let (status, headers, bytes) = get_bytes(server, &format!("/werke/{work}/pdf")).await;
    assert_eq!(status, StatusCode::OK, "Buch-PDF: {}", String::from_utf8_lossy(&bytes));
    assert_eq!(headers[header::CONTENT_TYPE], "application/pdf", "als PDF");
    bytes
}

// ---- Typwechsel beim Ersetzen ----------------------------------------------------------------------

#[tokio::test]
async fn a_png_replaces_an_svg_only_with_rename_and_the_old_path_shows_the_new_picture() {
    let server = common::open().await;
    let (work, slug) = house(&server).await;
    let png = baruch_epub::sample::png(30, 15, [200, 40, 40]);
    let content = baruch_store::encode_base64(&png);

    let _serial = common::PDF_SERIAL.lock().await;
    let before_pdf = book_pdf(&server, work).await;

    let refused = server
        .rpc(
            "asset.replace",
            json!({ "work": work, "name": "haus.svg", "file_name": "haus.png", "content": content, "encoding": "base64" }),
        )
        .await;
    assert_eq!(refused["error"]["code"], -32602, "anderes Format ohne rename ist eine ungültige Eingabe: {refused}");
    let message = refused["error"]["message"].as_str().unwrap();
    assert!(
        message.contains("Endung anpassen") && message.contains("rename"),
        "die Meldung nennt den Ausweg: {message}"
    );
    assert_eq!(
        server.state.store.asset_revisions(work, "haus.svg").await.unwrap().len(),
        1,
        "abgelehnt heißt: keine neue Fassung"
    );

    let commits_before = commits(&server, &slug);
    let answer = server
        .rpc(
            "asset.replace",
            json!({ "work": work, "name": "haus.svg", "file_name": "haus.png", "content": content,
                    "encoding": "base64", "rename": true }),
        )
        .await;
    let r = result(&answer);
    assert_eq!(r["name"], "haus.png", "der Name wechselt auf die neue Endung: {r}");
    assert_eq!(r["renamed_from"], "haus.svg", "die Antwort nennt den alten Namen");
    assert_eq!(r["rev"], 2, "neue Fassung 2");
    assert!(r["commit"].is_string(), "ein Spiegel-Commit: {r}");
    assert_eq!(commits(&server, &slug), commits_before + 1, "genau ein Commit für Fassung und Umbenennen");

    let got = server.rpc("asset.get", json!({ "work": work, "name": "haus.png", "content": false })).await;
    let got = result(&got);
    assert_eq!(got["revision"]["mime"], "image/png", "neue Fassung mit neuem MIME: {got}");
    assert_eq!(got["asset"]["paths"], json!(["bilder/haus.svg"]), "der Pfad bleibt beim Bild (Text unverändert)");
    assert_eq!(got["asset"]["revisions"], 2, "beide Fassungen bleiben am Bild");
    let old = server.rpc("asset.get", json!({ "work": work, "name": "haus.svg" })).await;
    assert!(old["error"].is_object(), "den alten Namen gibt es nicht mehr: {old}");

    let (status, headers, bytes) = get_bytes(&server, &format!("/werke/{work}/bilder/pfad?p=bilder/haus.svg")).await;
    assert_eq!(status, StatusCode::OK, "der alte Pfad liefert ein Bild");
    assert_eq!(headers[header::CONTENT_TYPE], "image/png", "… und zwar das neue PNG");
    assert_eq!(bytes, png, "Bytes der neuen Fassung");

    let mirror = common::git(&server.mirror_dir_of(&slug), &["ls-tree", "-r", "--name-only", "HEAD"]).unwrap();
    assert!(mirror.contains("bilder/haus.png.b64"), "der Spiegel trägt das Bild unter dem neuen Namen: {mirror}");
    assert!(!mirror.lines().any(|l| l == "bilder/haus.svg"), "die alte SVG-Datei ist im Spiegel weg: {mirror}");
    let body = common::git(&server.mirror_dir_of(&slug), &["log", "-1", "--format=%B"]).unwrap();
    assert!(body.contains("haus.svg → haus.png"), "der Commit nennt die Umbenennung: {body}");

    // Gemessen im PDF-Satz: Vorher ist das Bild eine SVG-Grafik (Vektoren, kein Rasterbild), danach steht am alten
    // Pfad ein Rasterbild mit den Maßen des neuen PNG.
    let after_pdf = book_pdf(&server, work).await;
    match (pdf_images(&before_pdf), pdf_images(&after_pdf)) {
        (Some(before), Some(after)) => {
            assert!(!before.contains(&(30, 15)), "vor dem Ersetzen kein 30×15-Rasterbild im Satz: {before:?}");
            assert!(after.contains(&(30, 15)), "nach dem Ersetzen setzt Typst am alten Pfad das PNG: {after:?}");
        }
        _ => eprintln!("ÜBERSPRUNGEN: pdfimages fehlt, PDF-Satz nicht vermessen"),
    }
}

#[tokio::test]
async fn the_replace_form_offers_adapt_extension_and_works_without_javascript() {
    let server = common::open().await;
    let (work, _) = house(&server).await;
    let (status, page) = server.get(&format!("/werke/{work}/bilder/haus.svg")).await;
    assert_eq!(status, StatusCode::OK, "Seite „Bild“");
    assert!(
        page.contains("name=\"rename\"") && page.contains("Endung anpassen"),
        "Häkchen „Endung anpassen“ im Formular"
    );

    let png = baruch_epub::sample::png(4, 4, [10, 20, 30]);
    let work_text = work.to_string();
    let (status, _, page) = server.send(upload(&[("work", &work_text), ("name", "haus.svg")], "neu.png", &png)).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "ohne Häkchen abgelehnt (ungültige Eingabe)");
    assert!(page.contains("Endung anpassen"), "die Seite erklärt den Ausweg: {page}");

    let (status, headers, _) =
        server.send(upload(&[("work", &work_text), ("name", "haus.svg"), ("rename", "1")], "neu.png", &png)).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "mit Häkchen gespeichert und weitergeleitet");
    let location = headers[header::LOCATION].to_str().unwrap();
    assert_eq!(
        location,
        format!("/werke/{work}/bilder/haus.png?hinweis=endung_angepasst"),
        "weiter zur Seite unter dem neuen Namen"
    );
    let (_, page) = server.get(location).await;
    assert!(page.contains("neuen Endung"), "Hinweis nach der Weiterleitung: {page}");
}

#[tokio::test]
async fn mcp_asset_replace_with_rename_only_files_a_request_and_confirming_renames() {
    let server = common::open().await;
    let (work, slug) = house(&server).await;
    let session = initialize(&server, "Bildhilfe").await;
    let png = baruch_epub::sample::png(6, 3, [1, 2, 3]);
    let content = baruch_store::encode_base64(&png);

    let text = refused(
        &server,
        &session,
        "asset_replace",
        json!({ "work": work, "name": "haus.svg", "content": content, "encoding": "base64" }),
    )
    .await;
    assert!(text.contains("rename"), "ohne rename sagt der Fehler, wie es geht: {text}");

    let commits_before = commits(&server, &slug);
    let asked = ok(
        &server,
        &session,
        "asset_replace",
        json!({ "work": work, "name": "haus.svg", "content": content, "encoding": "base64", "rename": true }),
    )
    .await;
    assert_eq!(asked["status"], "open", "nur ein Antrag: {asked}");
    assert_eq!(asked["request"]["rename"], true, "der Antrag trägt den Typwechsel");
    let open = server.state.store.asset_requests(work, true).await.unwrap();
    assert_eq!(open.len(), 1, "genau ein offener Antrag");
    assert_eq!(server.state.store.asset_revisions(work, "haus.svg").await.unwrap().len(), 1, "nichts ersetzt");
    assert_eq!(commits(&server, &slug), commits_before, "ein Antrag ist kein Commit");

    let id = open[0].id;
    let decided = server.rpc("asset.request_decide", json!({ "work": work, "id": id, "decision": "confirm" })).await;
    let decided = result(&decided);
    assert_eq!(decided["request"]["decision"], "confirmed", "bestätigt: {decided}");
    let list = server.state.store.asset_list(work).await.unwrap();
    let names: Vec<&str> = list.iter().map(|a| a.name.as_str()).collect();
    assert_eq!(names, ["haus.png"], "beim Bestätigen bekommt das Bild die neue Endung");
    assert_eq!(list[0].paths, ["bilder/haus.svg"], "der Pfad bleibt");
    assert_eq!(commits(&server, &slug), commits_before + 1, "ein Commit beim Bestätigen");
}

// ---- Altbestand: /x.png und x.png ------------------------------------------------------------------

#[tokio::test]
async fn a_legacy_path_collision_is_shown_and_resolved_by_the_human_in_one_commit() {
    let server = common::open().await;
    let (work, slug) = house(&server).await;
    let red = baruch_epub::sample::png(2, 2, [255, 0, 0]);
    let blue = baruch_epub::sample::png(2, 2, [0, 0, 255]);
    for (name, bytes, path) in [("rot.png", &red, Some("/x.png")), ("blau.png", &blue, None)] {
        let mut params = json!({ "work": work, "file_name": name, "content": baruch_store::encode_base64(bytes),
                                 "encoding": "base64" });
        if let Some(path) = path {
            params["path"] = json!(path);
        }
        result(&server.rpc("asset.add", params).await);
    }
    // Altbestand nachbauen: Seit Runde 7 lehnt der Store so etwas beim Anlegen ab, darum direkt in SQLite.
    sqlx::query(
        "INSERT INTO asset_path (work_id, path, asset_id) SELECT ?, 'x.png', id FROM asset WHERE work_id = ? AND \
         name = 'blau.png'",
    )
    .bind(work)
    .bind(work)
    .execute(server.state.store.pool())
    .await
    .unwrap();

    let conflicts = server.rpc("asset.path_conflicts", json!({ "work": work })).await;
    assert_eq!(
        result(&conflicts)["conflicts"],
        json!([{ "normal": "x.png", "paths": [{ "path": "/x.png", "asset": "rot.png" }, { "path": "x.png", "asset": "blau.png" }] }]),
        "eine Kollision in Normalform mit beiden Schreibweisen und ihren Bildern"
    );
    let (status, page) = server.get(&format!("/werke/{work}/bilder")).await;
    assert_eq!(status, StatusCode::OK, "Seite „Bilder“");
    assert!(page.contains("Pfade mit zwei Bildern"), "Hinweis auf der Bilder-Seite");
    assert!(page.contains("value=\"resolve_path\""), "Formular ohne JavaScript: {page}");

    let wrong = server.rpc("asset.path_resolve", json!({ "work": work, "normal": "x.png", "asset": "haus.svg" })).await;
    assert_eq!(wrong["error"]["code"], -32602, "ein Bild ohne diese Pfade steht nicht zur Wahl: {wrong}");

    let commits_before = commits(&server, &slug);
    let (status, location) = server
        .post_form("/werke/bilder/act", &format!("action=resolve_path&work={work}&normal=x.png&name=blau.png"))
        .await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Formular gespeichert");
    assert_eq!(location.as_deref(), Some(format!("/werke/{work}/bilder?hinweis=pfad_zugeordnet").as_str()), "zurück");
    assert_eq!(commits(&server, &slug), commits_before + 1, "genau ein Spiegel-Commit");
    let body = common::git(&server.mirror_dir_of(&slug), &["log", "-1", "--format=%B"]).unwrap();
    assert!(body.contains("Pfad-Kollision") && body.contains("blau.png"), "Commit nennt die Entscheidung: {body}");

    let conflicts = server.rpc("asset.path_conflicts", json!({ "work": work })).await;
    assert_eq!(result(&conflicts)["conflicts"], json!([]), "danach keine Kollision mehr");
    let list = server.state.store.asset_list(work).await.unwrap();
    let paths_of = |name: &str| list.iter().find(|a| a.name == name).unwrap().paths.clone();
    assert_eq!(paths_of("blau.png"), ["/x.png", "x.png"], "das gewählte Bild hat jetzt beide Schreibweisen");
    assert!(paths_of("rot.png").is_empty(), "das andere Bild hat den Pfad verloren (sonst nichts)");
    for path in ["/x.png", "x.png", "./x.png"] {
        let (_, _, bytes) = get_bytes(&server, &format!("/werke/{work}/bilder/pfad?p={path}")).await;
        assert_eq!(bytes, blue, "jede Schreibweise ({path}) liefert jetzt das blaue Bild");
    }
    let again = server.rpc("asset.path_resolve", json!({ "work": work, "normal": "x.png", "asset": "blau.png" })).await;
    assert_eq!(again["error"]["code"], -32000, "ohne Kollision gibt es nichts aufzulösen: {again}");
    let (_, page) = server.get(&format!("/werke/{work}/bilder")).await;
    assert!(!page.contains("Pfade mit zwei Bildern"), "der Hinweis ist verschwunden");
}

// ---- Kontingent für MCP ----------------------------------------------------------------------------

async fn post_mcp(server: &common::Server, session: Option<&str>, message: Value) -> (HeaderMap, Value) {
    let mut request = Request::post("/mcp").header(header::CONTENT_TYPE, "application/json");
    if let Some(id) = session {
        request = request.header("mcp-session-id", id);
    }
    let mut request = request.body(Body::from(message.to_string())).unwrap();
    request.extensions_mut().insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 40000))));
    let response = server.app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    assert_eq!(status, StatusCode::OK, "MCP antwortet: {}", String::from_utf8_lossy(&bytes));
    (headers, serde_json::from_slice(&bytes).unwrap())
}

async fn initialize(server: &common::Server, client: &str) -> String {
    let (headers, answer) = post_mcp(
        server,
        None,
        json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize",
                "params": { "protocolVersion": "2025-06-18", "capabilities": {},
                            "clientInfo": { "name": client, "version": "1.0" } } }),
    )
    .await;
    assert!(answer["result"].is_object(), "initialize: {answer}");
    headers.get("mcp-session-id").unwrap().to_str().unwrap().to_owned()
}

async fn tool(server: &common::Server, session: &str, name: &str, arguments: Value) -> Value {
    let message = json!({ "jsonrpc": "2.0", "id": 3, "method": "tools/call",
                          "params": { "name": name, "arguments": arguments } });
    post_mcp(server, Some(session), message).await.1
}

async fn ok(server: &common::Server, session: &str, name: &str, arguments: Value) -> Value {
    let answer = tool(server, session, name, arguments).await;
    assert_eq!(answer["result"]["isError"], false, "{name} sollte gelingen: {answer}");
    answer["result"]["structuredContent"].clone()
}

async fn refused(server: &common::Server, session: &str, name: &str, arguments: Value) -> String {
    let answer = tool(server, session, name, arguments).await;
    assert_eq!(answer["result"]["isError"], true, "{name} sollte fachlich scheitern: {answer}");
    answer["result"]["content"][0]["text"].as_str().unwrap().to_owned()
}

#[tokio::test]
async fn the_31st_writing_call_of_a_client_is_refused_and_another_name_without_token_too() {
    use baruch_server::mcp::assets::MAX_MCP_ASSET_WRITES_PER_DAY;
    let server = common::open().await;
    let (work, slug) = house(&server).await;
    let busy = initialize(&server, "Fleissige Hilfe").await;
    for n in 0..MAX_MCP_ASSET_WRITES_PER_DAY {
        ok(
            &server,
            &busy,
            "asset_add",
            json!({ "work": work, "file_name": format!("b{n}.svg"), "content": svg(n),
                                                  "encoding": "svg" }),
        )
        .await;
    }
    let assets_before = server.state.store.asset_list(work).await.unwrap().len();
    let commits_before = commits(&server, &slug);
    let text = refused(
        &server,
        &busy,
        "asset_add",
        json!({ "work": work, "file_name": "zuviel.svg", "content": svg(999), "encoding": "svg" }),
    )
    .await;
    assert!(text.contains("Kontingent") && text.contains("30"), "ehrliche Meldung mit der Grenze: {text}");
    assert!(text.contains("Nichts gespeichert"), "die Meldung sagt, dass nichts gespeichert ist: {text}");
    assert_eq!(server.state.store.asset_list(work).await.unwrap().len(), assets_before, "kein Bild angelegt");
    assert_eq!(commits(&server, &slug), commits_before, "kein Commit");
    let text = refused(
        &server,
        &busy,
        "asset_replace",
        json!({ "work": work, "name": "haus.svg", "content": svg(1000), "encoding": "svg" }),
    )
    .await;
    assert!(text.contains("Kontingent"), "auch asset_replace zählt zu den schreibenden Aufrufen: {text}");
    assert!(server.state.store.asset_requests(work, true).await.unwrap().is_empty(), "kein Antrag angelegt");

    // Runde 23: Clients ohne Token teilen je Werk EIN Kontingent – ein neuer Name hebt die Grenze nicht mehr auf
    // (ein Token hat sein eigenes: round23_mcp_namen::clients_without_token_share_one_asset_quota_and_a_token_has_its_own).
    let other = initialize(&server, "Andere Hilfe").await;
    let text = refused(
        &server,
        &other,
        "asset_add",
        json!({ "work": work, "file_name": "anders.svg", "content": svg(2000), "encoding": "svg" }),
    )
    .await;
    assert!(
        text.contains("gemeinsames Kontingent") && text.contains("Nichts gespeichert"),
        "ein anderer Name ohne Token umgeht das Kontingent nicht: {text}"
    );
    assert_eq!(server.state.store.asset_list(work).await.unwrap().len(), assets_before, "kein Bild unter neuem Namen");
}

#[tokio::test]
async fn more_than_64_mib_in_a_day_are_refused_and_nothing_is_stored() {
    use baruch_server::mcp::assets::MAX_MCP_ASSET_BYTES_PER_DAY;
    let server = common::open().await;
    let (work, _) = house(&server).await;
    let base = baruch_epub::sample::png(2, 2, [9, 9, 9]);
    result(
        &server
            .rpc(
                "asset.add",
                json!({ "work": work, "file_name": "gross.png", "content": baruch_store::encode_base64(&base),
                        "encoding": "base64" }),
            )
            .await,
    );
    let heavy = initialize(&server, "Schwere Hilfe").await;
    // Acht Anträge zu je 8 MiB (größte Bilddatei) = genau 64 MiB: noch erlaubt. Der Inhalt beginnt mit der
    // PNG-Kennung (Baruch erkennt den Typ an den ersten Bytes) und unterscheidet sich je Antrag.
    let eight = baruch_store::MAX_ASSET_BYTES;
    for n in 0..8u8 {
        let mut bytes = vec![n; eight];
        bytes[..8].copy_from_slice(b"\x89PNG\r\n\x1a\n");
        ok(
            &server,
            &heavy,
            "asset_replace",
            json!({ "work": work, "name": "gross.png", "content": baruch_store::encode_base64(&bytes),
                    "encoding": "base64" }),
        )
        .await;
    }
    let usage = server.state.store.asset_mcp_usage(work, "@mcp:schwere-hilfe").await.unwrap();
    assert_eq!(usage.bytes, MAX_MCP_ASSET_BYTES_PER_DAY, "gezählt aus den Zeilen: genau 64 MiB: {usage:?}");
    let requests_before = server.state.store.asset_requests(work, false).await.unwrap().len();
    let small = baruch_epub::sample::png(3, 3, [1, 1, 1]);
    let text = refused(
        &server,
        &heavy,
        "asset_replace",
        json!({ "work": work, "name": "gross.png", "content": baruch_store::encode_base64(&small), "encoding": "base64" }),
    )
    .await;
    assert!(text.contains("64 MiB") && text.contains("Nichts gespeichert"), "ehrliche Meldung: {text}");
    let text = refused(
        &server,
        &heavy,
        "asset_add",
        json!({ "work": work, "file_name": "klein.png", "content": baruch_store::encode_base64(&small),
                "encoding": "base64" }),
    )
    .await;
    assert!(text.contains("64 MiB"), "auch ein neues Bild zählt zu den Bytes: {text}");
    assert_eq!(
        server.state.store.asset_requests(work, false).await.unwrap().len(),
        requests_before,
        "kein weiterer Antrag gespeichert"
    );
    assert!(!server.state.store.asset_list(work).await.unwrap().iter().any(|a| a.name == "klein.png"), "kein Bild");

    // Runde 23: Auch ein anderer Name ohne Token bleibt im gemeinsamen Kontingent des Werks.
    let light = initialize(&server, "Leichte Hilfe").await;
    let text = refused(
        &server,
        &light,
        "asset_replace",
        json!({ "work": work, "name": "gross.png", "content": baruch_store::encode_base64(&small), "encoding": "base64" }),
    )
    .await;
    assert!(text.contains("64 MiB"), "ein neuer Name ohne Token hebt die Grenze der Bytes nicht auf: {text}");
    assert_eq!(
        server.state.store.asset_requests(work, false).await.unwrap().len(),
        requests_before,
        "kein Antrag unter neuem Namen"
    );
}

#[tokio::test]
async fn the_11th_open_replace_request_is_refused_until_the_human_decides() {
    use baruch_server::mcp::assets::MAX_MCP_OPEN_REPLACE_REQUESTS;
    let server = common::open().await;
    let (work, _) = house(&server).await;
    let eager = initialize(&server, "Eifrige Hilfe").await;
    for n in 0..MAX_MCP_OPEN_REPLACE_REQUESTS {
        ok(
            &server,
            &eager,
            "asset_replace",
            json!({ "work": work, "name": "haus.svg", "content": svg(n + 1),
                                                       "encoding": "svg" }),
        )
        .await;
    }
    let text = refused(
        &server,
        &eager,
        "asset_replace",
        json!({ "work": work, "name": "haus.svg", "content": svg(77), "encoding": "svg" }),
    )
    .await;
    assert!(text.contains("offene Anträge") && text.contains("10"), "ehrliche Meldung mit der Grenze: {text}");
    assert_eq!(
        server.state.store.asset_requests(work, true).await.unwrap().len(),
        10,
        "kein elfter Antrag gespeichert"
    );

    let first = server.state.store.asset_requests(work, true).await.unwrap()[0].id;
    result(&server.rpc("asset.request_decide", json!({ "work": work, "id": first, "decision": "reject" })).await);
    ok(
        &server,
        &eager,
        "asset_replace",
        json!({ "work": work, "name": "haus.svg", "content": svg(77), "encoding": "svg" }),
    )
    .await;
}

// ---- Aufräumen entschiedener Anträge und Kopfzeilen der Antragsdatei ------------------------------

#[tokio::test]
async fn decided_requests_lose_their_content_after_30_days_and_the_file_answers_410() {
    let server = common::open().await;
    let (work, _) = house(&server).await;
    let session = initialize(&server, "Bildhilfe").await;
    let old = ok(
        &server,
        &session,
        "asset_replace",
        json!({ "work": work, "name": "haus.svg", "content": svg(5),
                                                               "encoding": "svg", "note": "älter" }),
    )
    .await["request"]["id"]
        .as_i64()
        .unwrap();
    let young = ok(
        &server,
        &session,
        "asset_replace",
        json!({ "work": work, "name": "haus.svg", "content": svg(6),
                                                                 "encoding": "svg" }),
    )
    .await["request"]["id"]
        .as_i64()
        .unwrap();

    // Kopfzeilen der Antragsdatei (vorgeschlagener Inhalt): MIME, CSP mit sandbox, nosniff.
    let (status, headers, bytes) = get_bytes(&server, &format!("/werke/{work}/bilder/antraege/{old}/datei")).await;
    assert_eq!(status, StatusCode::OK, "offener Antrag: Datei lesbar");
    assert_eq!(headers[header::CONTENT_TYPE], "image/svg+xml", "MIME des Inhalts");
    let csp = headers[header::CONTENT_SECURITY_POLICY].to_str().unwrap();
    assert!(csp.contains("sandbox") && csp.contains("default-src 'none'"), "CSP mit sandbox: {csp}");
    assert_eq!(headers[header::X_CONTENT_TYPE_OPTIONS], "nosniff", "kein Raten des Typs");
    assert_eq!(bytes, svg(5).as_bytes(), "der vorgeschlagene Inhalt");

    for id in [old, young] {
        result(&server.rpc("asset.request_decide", json!({ "work": work, "id": id, "decision": "reject" })).await);
    }
    // Entscheidung 31 bzw. 29 Tage zurückdatieren (asset_request hat keinen Schutz-Trigger auf decided).
    for (id, days) in [(old, 31), (young, 29)] {
        sqlx::query("UPDATE asset_request SET decided = strftime('%Y-%m-%dT%H:%M:%fZ', 'now', ?) WHERE id = ?")
            .bind(format!("-{days} days"))
            .bind(id)
            .execute(server.state.store.pool())
            .await
            .unwrap();
    }
    let list = server.rpc("asset.request_list", json!({ "work": work, "open": false })).await;
    let list = result(&list)["requests"].as_array().unwrap().clone();
    let of = |id: i64| list.iter().find(|r| r["id"] == id).unwrap().clone();
    assert_eq!(of(old)["purged"], true, "älter als 30 Tage entschieden: Inhalt entfernt");
    assert_eq!(of(old)["bytes"], svg(5).len(), "die Größe bleibt bekannt");
    assert_eq!(of(old)["decision"], "rejected", "die Entscheidung bleibt");
    assert_eq!(of(young)["purged"], false, "29 Tage: Inhalt bleibt");
    let params: String = sqlx::query_scalar("SELECT params FROM asset_request WHERE id = ?")
        .bind(old)
        .fetch_one(server.state.store.pool())
        .await
        .unwrap();
    let params: Value = serde_json::from_str(&params).unwrap();
    assert_eq!(params["content"], "", "der Inhalt ist wirklich aus der Datenbank entfernt: {params}");

    let (status, _, bytes) = get_bytes(&server, &format!("/werke/{work}/bilder/antraege/{old}/datei")).await;
    assert_eq!(status, StatusCode::GONE, "Antragsdatei nach dem Aufräumen: 410");
    let text = String::from_utf8(bytes).unwrap();
    assert!(text.contains("entfernt") && text.contains("30 Tage"), "ehrliche Meldung: {text}");
    let (status, _, _) = get_bytes(&server, &format!("/werke/{work}/bilder/antraege/{young}/datei")).await;
    assert_eq!(status, StatusCode::OK, "der jüngere Antrag behält seine Datei");
}

// ---- Maskierung und SVG mit Skript -----------------------------------------------------------------

#[tokio::test]
async fn alt_caption_note_and_path_are_escaped_on_the_pictures_pages() {
    let server = common::open().await;
    let (work, _) = house(&server).await;
    let answer = server
        .rpc(
            "asset.add",
            json!({ "work": work, "file_name": "quote.svg", "content": svg(3), "encoding": "svg",
                    "alt": "Alt\"<b>Fett</b>&'", "caption": "Unter\"<i>schrift</i>&'",
                    "note": "Notiz\"<u>unten</u>&'", "path": "bilder/q\"<s>&'.svg" }),
        )
        .await;
    result(&answer);
    let (status, list) = server.get(&format!("/werke/{work}/bilder")).await;
    assert_eq!(status, StatusCode::OK, "Seite „Bilder“");
    let (status, detail) = server.get(&format!("/werke/{work}/bilder/quote.svg")).await;
    assert_eq!(status, StatusCode::OK, "Seite „Bild“");
    for (page, html) in [("Bilder", &list), ("Bild", &detail)] {
        for raw in ["<b>Fett", "<i>schrift", "<s>&", "Alt\"<", "Unter\"<"] {
            assert!(!html.contains(raw), "{page}: „{raw}“ steht unmaskiert im HTML (Einschleusen möglich)");
        }
        assert!(html.contains("&lt;b&gt;Fett"), "{page}: Alternativtext maskiert sichtbar");
        assert!(html.contains("&lt;i&gt;schrift"), "{page}: Bildunterschrift maskiert sichtbar");
        assert!(html.contains("q&quot;&lt;s&gt;&amp;"), "{page}: Pfad maskiert sichtbar");
    }
    assert!(!detail.contains("<u>unten"), "Bild: Notiz unmaskiert");
    assert!(detail.contains("&lt;u&gt;unten"), "Bild: Notiz maskiert sichtbar");
    assert!(detail.contains("&#x27;"), "Bild: Apostroph maskiert (Attribute in einfachen Anführungszeichen sicher)");
}

#[tokio::test]
async fn an_svg_with_a_script_is_served_by_path_only_inside_a_sandbox() {
    let server = common::open().await;
    let (work, _) = house(&server).await;
    let evil = "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"4\" height=\"4\"><script>alert('x')</script>\
                <rect width=\"4\" height=\"4\"/></svg>\n";
    result(
        &server
            .rpc(
                "asset.add",
                json!({ "work": work, "file_name": "boese.svg", "content": evil, "encoding": "svg",
                        "path": "/bilder/boese.svg" }),
            )
            .await,
    );
    let (status, headers, bytes) = get_bytes(&server, &format!("/werke/{work}/bilder/pfad?p=bilder/boese.svg")).await;
    assert_eq!(status, StatusCode::OK, "das Bild kommt über den Pfad");
    assert_eq!(headers[header::CONTENT_TYPE], "image/svg+xml", "als SVG");
    let csp = headers[header::CONTENT_SECURITY_POLICY].to_str().unwrap();
    assert!(csp.contains("sandbox"), "CSP sandbox: ein direkt geöffnetes SVG führt kein Skript aus: {csp}");
    assert!(csp.contains("default-src 'none'") && !csp.contains("script-src"), "kein Skript erlaubt: {csp}");
    assert_eq!(headers[header::X_CONTENT_TYPE_OPTIONS], "nosniff", "kein Raten des Typs");
    assert_eq!(bytes, evil.as_bytes(), "der Inhalt bleibt unverändert (Schutz über die Kopfzeilen)");
}

// ---- Literatur, LibraryView, Bild einfügen ---------------------------------------------------------

#[tokio::test]
async fn the_literature_shows_matching_pictures_with_thumbnail_and_link_without_javascript() {
    let server = common::open().await;
    let (work, _) = house(&server).await;
    result(
        &server
            .rpc(
                "asset.add",
                json!({ "work": work, "file_name": "brunnen.svg", "content": svg(4), "encoding": "svg",
                        "caption": "Der alte Brunnen <hof>", "alt": "Ein Brunnen & Eimer" }),
            )
            .await,
    );
    let (status, page) = server.get(&format!("/literatur?work={work}&lq=Brunnen")).await;
    assert_eq!(status, StatusCode::OK, "Seite /literatur ohne JavaScript");
    assert!(page.contains("data-library-images"), "Kasten „Bilder des Werks“: {page}");
    assert!(page.contains(&format!("href=\"/werke/{work}/bilder/brunnen.svg\"")), "Link auf die Seite des Bilds");
    assert!(page.contains(&format!("src=\"/werke/{work}/bilder/brunnen.svg/vorschau?b=160\"")), "Vorschaubild");
    assert!(page.contains("&lt;hof&gt;") && !page.contains("<hof>"), "Bildunterschrift maskiert");
    assert!(page.contains("Ein Brunnen &amp; Eimer"), "Alternativtext maskiert");
    assert!(!page.contains("bilder/haus.svg\""), "ein Bild ohne Treffer erscheint nicht");

    let rendered = server.rpc("literature.render", json!({ "work": work, "lq": "brunnen EIMER" })).await;
    let html = result(&rendered)["html"].as_str().unwrap().to_owned();
    assert!(
        html.contains("brunnen.svg"),
        "im Popup (literature.render): alle Wörter gefunden, ohne Groß/Klein: {html}"
    );
    let rendered = server.rpc("literature.render", json!({ "work": work, "lq": "Brunnen Leiter" })).await;
    let html = result(&rendered)["html"].as_str().unwrap().to_owned();
    assert!(html.contains("Kein Bild passt"), "ein fehlendes Wort: kein Bild, ehrlich gesagt");
    let rendered = server.rpc("literature.render", json!({ "work": work })).await;
    let html = result(&rendered)["html"].as_str().unwrap().to_owned();
    assert!(!html.contains("data-library-images"), "ohne Suche kein Kasten");
}

#[tokio::test]
async fn the_library_view_carries_the_image_view_in_its_own_field() {
    let server = common::open().await;
    let (work, _) = house(&server).await;
    let view = baruch_server::rpc::library_view(&server.state, work, None).await.unwrap();
    assert_eq!(view.images.assets.len(), 1, "ImageView als eigenes Feld: {:?}", view.images);
    assert_eq!(view.images.assets[0].paths, ["bilder/haus.svg"], "Pfade in Normalform");
    assert_eq!(view.image_alts.get("bilder/haus.svg").map(String::as_str), Some("Ein grünes Haus"), "Alternativtext");
    assert!(
        view.image_alts.keys().all(|k| !k.chars().any(char::is_control)),
        "kein reservierter Schlüssel mehr in image_alts: {:?}",
        view.image_alts.keys().collect::<Vec<_>>()
    );
    assert_eq!(view.image_alts.len(), 1, "nur echte Pfade");
}

#[test]
fn a_figure_at_an_offset_inside_a_multiline_footnote_goes_behind_the_construct() {
    use baruch_server::assets::{Place, insert_figure_at};
    let text = "= Hof\n\nEs regnet#footnote[eins\nzwei] heute.\n\nDann Sonne.\n";
    let inside = "= Hof\n\nEs regnet#footnote[ei".encode_utf16().count();
    let out = insert_figure_at(text, Place::Offset(inside), "#figure(image(\"a.png\"))");
    assert_eq!(
        out, "= Hof\n\nEs regnet#footnote[eins\nzwei] heute.\n\n#figure(image(\"a.png\"))\n\nDann Sonne.\n",
        "die Abbildung steht hinter der Fußnote, nicht in ihr (Gutachten R18)"
    );
    assert!(!out.contains("eins\n\n#figure"), "nie mitten in die Fußnote");
}

// ---- EPUB-Cover als Rasterbild ---------------------------------------------------------------------

fn zip_entry(zip_bytes: &[u8], name: &str) -> Option<Vec<u8>> {
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(zip_bytes)).expect("gültiges ZIP");
    let mut file = archive.by_name(name).ok()?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).unwrap();
    Some(bytes)
}

async fn epub_of(server: &common::Server, work: i64) -> (Value, Vec<u8>) {
    let answer = server.rpc("work.epub", json!({ "work": work, "inline": true })).await;
    let r = result(&answer).clone();
    let bytes = baruch_store::decode_base64(r["epub_base64"].as_str().unwrap()).unwrap();
    (r, bytes)
}

fn block(kind: &str, y_mm: f64, size_pt: f64) -> Value {
    json!({ "kind": kind, "x_mm": 10.0, "y_mm": y_mm, "width_mm": 100.0, "align": "center", "size_pt": size_pt,
            "leading_em": 0.65, "color": "light", "font": null, "style": "bold", "backdrop": "none", "visible": true })
}

/// Bildpunkte im Rechteck (mm) der Seite (120 × 180 mm), die zwischen `a` und `b` deutlich abweichen.
fn differing(a: &image::RgbaImage, b: &image::RgbaImage, x_mm: (f64, f64), y_mm: (f64, f64)) -> usize {
    let scale = f64::from(a.width()) / 120.0;
    let mut count = 0;
    for y in (y_mm.0 * scale) as u32..((y_mm.1 * scale) as u32).min(a.height()) {
        for x in (x_mm.0 * scale) as u32..((x_mm.1 * scale) as u32).min(a.width()) {
            let (p, q) = (a.get_pixel(x, y), b.get_pixel(x, y));
            if (0..3).any(|c| (i32::from(p[c]) - i32::from(q[c])).abs() > 60) {
                count += 1;
            }
        }
    }
    count
}

#[tokio::test]
async fn the_epub_cover_is_a_png_of_the_front_with_title_subtitle_and_author() {
    let server = common::open().await;
    let (work, _) = house(&server).await;
    let raw = baruch_epub::sample::png(60, 90, [40, 90, 140]);
    let added = server
        .rpc(
            "asset.add",
            json!({ "work": work, "file_name": "umschlag.png", "content": baruch_store::encode_base64(&raw),
                    "encoding": "base64", "alt": "Blauer Himmel" }),
        )
        .await;
    result(&added);
    result(&server.rpc("work.set_cover", json!({ "work": work, "name": "umschlag.png" })).await);
    result(
        &server
            .rpc("work.meta.set", json!({ "work": work, "subtitle": "Ein Jahr am Hang", "author": "Grete Beispiel" }))
            .await,
    );
    let blocks = json!([
        block("front_title", 20.0, 28.0),
        block("front_subtitle", 60.0, 16.0),
        block("front_author", 150.0, 14.0)
    ]);
    result(&server.rpc("cover.layout_set", json!({ "work": work, "blocks": blocks })).await);

    let _serial = common::PDF_SERIAL.lock().await;
    let (answer, epub) = epub_of(&server, work).await;
    let codes: Vec<&str> = answer["warnings"].as_array().unwrap().iter().map(|w| w["code"].as_str().unwrap()).collect();
    assert!(!codes.contains(&"COVER_RASTER_FAILED"), "das Rastern gelang: {codes:?}");
    let cover = zip_entry(&epub, "OEBPS/images/cover.png").expect("cover-image als PNG im Paket");
    assert!(cover.starts_with(b"\x89PNG\r\n\x1a\n"), "cover-image ist ein PNG");
    assert_ne!(cover, raw, "nicht die rohe Bilddatei, sondern die gesetzte Vorderseite");
    let opf = String::from_utf8(zip_entry(&epub, "OEBPS/content.opf").unwrap()).unwrap();
    assert!(opf.contains("href=\"images/cover.png\"") && opf.contains("properties=\"cover-image\""), "im Manifest");
    let picture = image::load_from_memory(&cover).unwrap().to_rgba8();
    assert!(picture.width() <= 1600 && picture.height() <= 2560, "höchstens 1600 × 2560: {:?}", picture.dimensions());
    assert!(picture.height() >= 2400, "so groß wie möglich (Höhe ist die Grenze): {:?}", picture.dimensions());
    let ratio = f64::from(picture.width()) / f64::from(picture.height());
    assert!((ratio / (120.0 / 180.0) - 1.0).abs() < 0.01, "Seitenverhältnis der Vorderseite ± 1 %: {ratio}");

    // Derselbe Umschlag ohne Text, gerastert wie das Cover: In den Blockrahmen von Titel und Autor weichen die Pixel
    // ab (dort steht Schrift), dazwischen nicht (nur das Bild).
    let plain = baruch_render::Source {
        main: "#set page(width: 120mm, height: 180mm, margin: 0pt)\n\
               #place(top + left, image(\"u.png\", width: 100%, height: 100%, fit: \"cover\"))\n"
            .to_owned(),
        files: vec![("u.png".to_owned(), raw.clone())],
    };
    let plain = baruch_render::render_png(&plain, 0, 1600, 2560).expect("Umschlag ohne Text gerastert");
    let plain = image::load_from_memory(&plain.png).unwrap().to_rgba8();
    assert_eq!(plain.dimensions(), picture.dimensions(), "gleiche Rastergröße zum Vergleich");
    let title = differing(&picture, &plain, (10.0, 110.0), (20.0, 34.0));
    let author = differing(&picture, &plain, (10.0, 110.0), (150.0, 158.0));
    let between = differing(&picture, &plain, (10.0, 110.0), (90.0, 140.0));
    assert!(title > 500, "im Blockrahmen des Titels steht Schrift: {title} abweichende Bildpunkte");
    assert!(author > 200, "im Blockrahmen des Autors steht Schrift: {author} abweichende Bildpunkte");
    assert_eq!(between, 0, "zwischen den Blöcken nur das Umschlagbild");

    let tool = baruch_epub::check::find_xmllint(std::env::var_os("PATH").as_deref());
    let (findings, _) = baruch_epub::check::check_structure(&epub, tool.as_deref());
    let errors: Vec<_> =
        findings.iter().filter(|f| f.severity == baruch_epub::check::Severity::Error).map(|f| &f.code).collect();
    assert!(errors.is_empty(), "EPUB-Prüfung (R20c) ohne Fehler: {errors:?}");
}

#[tokio::test]
async fn without_a_cover_picture_the_epub_still_gets_a_cover_with_the_title() {
    let server = common::open().await;
    let (work, _) = house(&server).await;
    let _serial = common::PDF_SERIAL.lock().await;
    let (answer, epub) = epub_of(&server, work).await;
    assert_eq!(answer["files"][0]["kind"], "cover", "Cover vorn in der Lesereihenfolge: {answer}");
    let cover = zip_entry(&epub, "OEBPS/images/cover.png").expect("Cover-Bild auch ohne Umschlagbild");
    let picture = image::load_from_memory(&cover).unwrap().to_rgba8();
    // Dunkle Fläche (#1f2a33), heller Titel im Titelbereich oben: helle Bildpunkte im oberen Drittel.
    let bright = (0..picture.height() / 3)
        .flat_map(|y| (0..picture.width()).map(move |x| (x, y)))
        .filter(|&(x, y)| picture.get_pixel(x, y).0[..3].iter().all(|&c| c > 200))
        .count();
    assert!(bright > 500, "der Titel steht hell auf der dunklen Fläche: {bright} helle Bildpunkte");
    let xhtml = String::from_utf8(zip_entry(&epub, "OEBPS/text/cover.xhtml").unwrap()).unwrap();
    assert!(xhtml.contains("alt=\"Umschlag: Das Haus am Hang\""), "Alternativtext mit Titel: {xhtml}");
}

#[tokio::test]
async fn a_raster_failure_falls_back_to_the_raw_picture_with_a_warning() {
    let server = common::open().await;
    // Werkvorlage mit einem Fehler (unbekannte Variable): Das Seitenformat lässt sich nicht setzen, also auch die
    // Vorderseite nicht. Das EPUB entsteht trotzdem, mit der rohen Bilddatei und einer Warnung.
    let book = "#set text(lang: \"de\")\n#let breite = nicht-da\n\n= Kapitel\nText.\n";
    let report = server.import(book, "kaputt.typ").await;
    let raw = baruch_epub::sample::png(5, 8, [200, 200, 0]);
    result(
        &server
            .rpc(
                "asset.add",
                json!({ "work": report.work, "file_name": "umschlag.png", "content": baruch_store::encode_base64(&raw),
                        "encoding": "base64" }),
            )
            .await,
    );
    result(&server.rpc("work.set_cover", json!({ "work": report.work, "name": "umschlag.png" })).await);
    let _serial = common::PDF_SERIAL.lock().await;
    let (answer, epub) = epub_of(&server, report.work).await;
    let warning = answer["warnings"].as_array().unwrap().iter().find(|w| w["code"] == "COVER_RASTER_FAILED").cloned();
    let warning = warning.expect("Rasterfehler wird gemeldet, nie still weggelassen");
    assert!(warning["message"].as_str().unwrap().contains("umschlag.png"), "die Warnung nennt den Rückfall: {warning}");
    let cover = zip_entry(&epub, "OEBPS/images/cover.png").expect("Rückfall: die rohe Bilddatei als Cover");
    assert_eq!(cover, raw, "Rohbild unverändert");
}

// ---- pdfinfo bei allen PDFs von Baruch -------------------------------------------------------------

/// Meldungen von `pdfinfo` auf stderr; `None`, wenn es das Programm nicht gibt.
fn pdfinfo_errors(pdf: &[u8]) -> Option<Vec<String>> {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("probe.pdf");
    std::fs::write(&path, pdf).unwrap();
    let output = std::process::Command::new("pdfinfo").arg(&path).output().ok()?;
    assert!(output.status.success(), "pdfinfo liest das PDF: {}", String::from_utf8_lossy(&output.stderr));
    Some(String::from_utf8_lossy(&output.stderr).lines().map(str::to_owned).filter(|l| !l.is_empty()).collect())
}

#[tokio::test]
async fn pdfinfo_only_complains_about_the_suspects_flag_that_typst_writes_into_every_pdf() {
    const SUSPECTS: &str = "Syntax Error: Suspects object is wrong type (boolean)";
    let server = common::open().await;
    let (work, _) = house(&server).await;
    let raw = baruch_epub::sample::png(60, 90, [40, 90, 140]);
    result(
        &server
            .rpc(
                "asset.add",
                json!({ "work": work, "file_name": "umschlag.png", "content": baruch_store::encode_base64(&raw),
                        "encoding": "base64" }),
            )
            .await,
    );
    result(&server.rpc("work.set_cover", json!({ "work": work, "name": "umschlag.png" })).await);
    let _serial = common::PDF_SERIAL.lock().await;
    for (kind, uri) in [
        ("Buch", format!("/werke/{work}/pdf")),
        ("Stichwortzettel", format!("/werke/{work}/stichwortzettel.pdf")),
        ("Karte", format!("/werke/{work}/karte.pdf")),
        ("Umschlag", format!("/werke/{work}/umschlag.pdf")),
    ] {
        let (status, _, pdf) = get_bytes(&server, &uri).await;
        assert_eq!(status, StatusCode::OK, "{kind}: PDF entsteht: {}", String::from_utf8_lossy(&pdf));
        assert!(pdf.starts_with(b"%PDF-"), "{kind}: ein PDF");
        let Some(errors) = pdfinfo_errors(&pdf) else {
            eprintln!("ÜBERSPRUNGEN: pdfinfo fehlt");
            return;
        };
        assert!(errors.iter().all(|e| e == SUSPECTS), "{kind}: pdfinfo meldet nur die Suspects-Zeile: {errors:?}");
        // Ursache belegt: krilla (PDF-Schreiber von typst-pdf) setzt im getaggten PDF `/MarkInfo << /Marked true
        // /Suspects false >>` (PDF/UA verlangt Suspects = false). poppler meldet jeden Wert außer `true` als
        // „wrong type (boolean)“, obwohl `false` ein gültiger boolescher Wert ist. Ohne das Feld schweigt pdfinfo.
        let at = pdf.windows(15).position(|w| w == b"/Suspects false");
        eprintln!("pdfinfo {kind}: Meldungen {errors:?}, „/Suspects false“ an Byte {at:?}");
        match at {
            Some(at) => {
                assert!(!errors.is_empty(), "{kind}: mit /Suspects false meldet poppler die Zeile");
                let mut patched = pdf.clone();
                patched[at..at + 15].copy_from_slice(b"/Xuspects false"); // gleiche Länge: Querverweise stimmen
                let after = pdfinfo_errors(&patched).unwrap();
                assert!(after.is_empty(), "{kind}: ohne /Suspects keine Meldung mehr – die Ursache: {after:?}");
            }
            None => assert!(errors.is_empty(), "{kind}: ohne /Suspects-Feld keine Meldung: {errors:?}"),
        }
    }
}
