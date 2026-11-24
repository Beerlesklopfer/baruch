// SPDX-License-Identifier: AGPL-3.0-or-later
//! Runde 26 „import“: Austauschdatei einlesen (Vorschau, neu, abgleichen; RPC `work.exchange_preview` und
//! `work.exchange_import`, Formulare, CLI), Sicherheit und Grenzen. Die Austauschdatei ist die handgeschriebene,
//! frei erfundene Prüfdatei des Lesers (`baruch-exchange/tests/data/import/werk.baruch.xml`); Container baut der
//! Test selbst mit `zip`. Jede Zusicherung mit Begründung.

use std::collections::BTreeMap;
use std::io::{Cursor, Write};
use std::process::Command;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use serde_json::{Value, json};
use zip::CompressionMethod;
use zip::write::SimpleFileOptions;

use crate::common;

const WERK: &str = include_str!("../../baruch-exchange/tests/data/import/werk.baruch.xml");
const WORK_UUID: &str = "c0000000-0000-4000-8000-000000000001";
const WHEEL_UUID: &str = "c0000000-0000-4000-8000-000000000011";
const BOUNDARY: &str = "baruch-austausch-grenze";

const TABLES: &[&str] = &[
    "work",
    "node",
    "revision",
    "work_template_revision",
    "post",
    "post_ref",
    "idea",
    "idea_word",
    "chat_thread",
    "chat_thread_post",
    "asset",
    "asset_path",
    "asset_revision",
    "work_book",
    "work_timing",
    "work_bible",
    "work_sword_source",
    "outline_request",
    "uuid_tombstone",
    // Gedankenkarte (Runde 25): auch Verbindungen und Lagen schreibt keine Ablehnung.
    "idea_link",
    "idea_map_pos",
];

async fn counts(server: &common::Server) -> BTreeMap<&'static str, i64> {
    let mut out = BTreeMap::new();
    for table in TABLES {
        let n: i64 = sqlx::query_scalar(&format!("SELECT count(*) FROM {table}"))
            .fetch_one(server.state.store.pool())
            .await
            .unwrap();
        out.insert(*table, n);
    }
    out
}

fn sha(bytes: &[u8]) -> String {
    baruch_exchange::sha256_hex(bytes)
}

fn b64(bytes: &[u8]) -> String {
    baruch_store::encode_base64(bytes)
}

/// Multipart-Anfrage an `/austausch/vorschau` wie ein Browserformular (Felder `title`, `datei`).
fn upload(file_name: &str, content: &[u8], title: &str) -> Request<Body> {
    let mut body = Vec::new();
    body.extend_from_slice(
        format!("--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"title\"\r\n\r\n{title}\r\n").as_bytes(),
    );
    body.extend_from_slice(
        format!(
            "--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"datei\"; filename=\"{file_name}\"\r\n\
             Content-Type: application/octet-stream\r\n\r\n"
        )
        .as_bytes(),
    );
    body.extend_from_slice(content);
    body.extend_from_slice(format!("\r\n--{BOUNDARY}--\r\n").as_bytes());
    Request::post("/austausch/vorschau")
        .header(header::CONTENT_TYPE, format!("multipart/form-data; boundary={BOUNDARY}"))
        .body(Body::from(body))
        .unwrap()
}

fn take(sha256: &str, mode: &str, title: &str) -> Request<Body> {
    Request::post("/austausch/uebernehmen")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(format!("sha256={sha256}&mode={mode}&title={title}")))
        .unwrap()
}

fn inbox(server: &common::Server) -> Vec<String> {
    match std::fs::read_dir(server.path().join("austausch-eingang")) {
        Ok(entries) => entries.map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect(),
        Err(_) => Vec::new(),
    }
}

/// Container `.baruch` aus der Prüfdatei: Bilder als Einträge, Manifest zuletzt; `extra` hängt weitere Einträge an.
fn container(extra: &[(&str, &[u8])]) -> Vec<u8> {
    container_with(extra, false)
}

/// Wie [`container`]; mit `wrong_sum` nennt das Manifest für werk.xml eine falsche Prüfsumme.
fn container_with(extra: &[(&str, &[u8])], wrong_sum: bool) -> Vec<u8> {
    let doc = baruch_exchange::read::read_xml(WERK.as_bytes(), &baruch_exchange::Limits::DEFAULT).unwrap();
    let png_daten =
        WERK[WERK.find("<daten>iVBOR").unwrap()..].split_once("</daten>").unwrap().0.to_owned() + "</daten>";
    let svg_daten = WERK[WERK.find("<daten>PHN2").unwrap()..].split_once("</daten>").unwrap().0.to_owned() + "</daten>";
    let png = format!("bilder/{}.png", doc.work.assets[0].revisions[0].uuid);
    let svg = format!("bilder/{}.svg", doc.work.assets[1].revisions[0].uuid);
    let xml = WERK
        .replace(&png_daten, "")
        .replace(&svg_daten, "")
        .replace(
            "angelegt=\"2026-02-06T10:00:00.000Z\">\n        <unterschrift>",
            &format!("angelegt=\"2026-02-06T10:00:00.000Z\" datei=\"{png}\">\n        <unterschrift>"),
        )
        .replace(
            "angelegt=\"2026-02-06T11:00:00.000Z\">\n        <unterschrift>",
            &format!("angelegt=\"2026-02-06T11:00:00.000Z\" datei=\"{svg}\">\n        <unterschrift>"),
        );
    let mut entries: Vec<(String, Vec<u8>)> = vec![
        ("werk.xml".into(), xml.into_bytes()),
        (png, doc.work.assets[0].revisions[0].data.clone()),
        (svg, doc.work.assets[1].revisions[0].data.clone()),
    ];
    for (name, bytes) in extra {
        entries.push(((*name).into(), bytes.to_vec()));
    }
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    zip.start_file("mimetype", SimpleFileOptions::default().compression_method(CompressionMethod::Stored)).unwrap();
    zip.write_all(b"application/vnd.baruch.werk+zip").unwrap();
    let mut manifest = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<manifest xmlns=\"urn:baruch:werk:1\" version=\"1\">\n",
    );
    for (name, bytes) in &entries {
        zip.start_file(name.as_str(), SimpleFileOptions::default().compression_method(CompressionMethod::Deflated))
            .unwrap();
        zip.write_all(bytes).unwrap();
        let sum = if wrong_sum && name == "werk.xml" { sha(b"anderer Inhalt") } else { sha(bytes) };
        manifest.push_str(&format!("  <datei pfad=\"{name}\" sha256=\"{sum}\" bytes=\"{}\"/>\n", bytes.len()));
    }
    manifest.push_str("</manifest>\n");
    zip.start_file("MANIFEST.xml", SimpleFileOptions::default()).unwrap();
    zip.write_all(manifest.as_bytes()).unwrap();
    zip.finish().unwrap().into_inner()
}

/// Die Prüfdatei mit anderem Text der aktuellen Fassung von „Das Rad“ (wie auf der anderen Seite geändert).
fn changed_xml() -> String {
    WERK.replace("Äpfel.  </text>", "Birnen.  </text>")
}

async fn import_new(server: &common::Server) -> i64 {
    let data = WERK.as_bytes();
    let answer =
        server.rpc("work.exchange_import", json!({ "data": b64(data), "sha256": sha(data), "mode": "new" })).await;
    answer["result"]["work"].as_i64().unwrap_or_else(|| panic!("Einlesen scheitert: {answer}"))
}

fn log(server: &common::Server, slug: &str) -> Option<String> {
    common::git(&server.mirror_dir_of(slug), &["log", "--format=%s|%an"])
}

#[tokio::test]
async fn the_form_page_offers_a_multipart_upload_without_javascript() {
    let server = common::open().await;
    let (status, html) = server.get("/austausch/einlesen?work=1").await;
    assert_eq!(status, StatusCode::OK, "Seite da");
    assert!(html.contains("action=\"/austausch/vorschau\""), "Formular an die Vorschau: {html}");
    assert!(html.contains("enctype=\"multipart/form-data\""), "multipart ohne JavaScript");
    assert!(html.contains("name=\"datei\""), "Feld datei");
    assert!(!html.contains("Noch nicht umgesetzt"), "kein Stub-Text mehr");
}

#[tokio::test]
async fn rpc_preview_and_import_follow_the_contract() {
    let server = common::open().await;
    let data = WERK.as_bytes();
    let before = counts(&server).await;
    let preview =
        server.rpc("work.exchange_preview", json!({ "data": b64(data), "file_name": "werk.baruch.xml" })).await;
    let result = &preview["result"];
    assert_eq!(result["mode"], "new", "unbekanntes Werk → neu: {preview}");
    assert_eq!(result["work_uuid"], WORK_UUID, "Kennung des Werks");
    assert_eq!(result["known_work"], Value::Null, "kein Werk hier");
    assert_eq!(result["instance"]["name"], "Prüfrechner", "Instanz der Datei");
    assert_eq!(result["sha256"], sha(data), "Prüfsumme der Datei");
    assert_eq!(result["add"]["nodes"], 3, "drei Punkte kämen dazu");
    assert_eq!(result["add"]["assets"], 2, "zwei Bilder");
    assert!(result["items"].as_array().is_some_and(|i| !i.is_empty()), "Einträge gelistet");
    assert_eq!(counts(&server).await, before, "Vorschau schreibt nichts");

    let wrong =
        server.rpc("work.exchange_import", json!({ "data": b64(data), "sha256": sha(b"anders"), "mode": "new" })).await;
    assert_eq!(wrong["error"]["code"], -32602, "falscher sha256 → INVALID_PARAMS: {wrong}");
    let mismatch =
        server.rpc("work.exchange_import", json!({ "data": b64(data), "sha256": sha(data), "mode": "merge" })).await;
    assert_eq!(mismatch["error"]["code"], -32000, "Modus passt nicht → REFUSED: {mismatch}");
    assert_eq!(counts(&server).await, before, "Ablehnungen schreiben nichts");

    let answer =
        server.rpc("work.exchange_import", json!({ "data": b64(data), "sha256": sha(data), "mode": "new" })).await;
    let result = &answer["result"];
    assert_eq!(result["mode"], "new", "neu eingelesen: {answer}");
    assert_eq!(result["title"], "Die Mühle am Bach", "Titel aus der Datei");
    assert_eq!(result["added"]["nodes"], preview["result"]["add"]["nodes"], "Vorschau und Übernehmen gleich");
    assert!(result["commit"].is_string(), "ein Commit im Spiegel: {answer}");
    let slug = result["slug"].as_str().unwrap();
    if let Some(log) = log(&server, slug) {
        assert_eq!(log.lines().count(), 1, "genau ein Commit: {log}");
        assert!(
            log.starts_with(
                "Austausch aus Prüfrechner: Werk „Die Mühle am Bach“ neu eingelesen|Austausch aus Prüfrechner"
            ),
            "Herkunft im Commit: {log}"
        );
    }
    let work = result["work"].as_i64().unwrap();
    let meta = server.rpc("work.meta", json!({ "work": work })).await;
    assert_eq!(meta["result"]["work"]["title"], "Die Mühle am Bach", "Werk ist angelegt: {meta}");

    // Zweiter gleicher Import als Abgleich: nichts kommt hinzu, kein Commit.
    let again =
        server.rpc("work.exchange_import", json!({ "data": b64(data), "sha256": sha(data), "mode": "merge" })).await;
    let total: u64 = again["result"]["added"].as_object().unwrap().values().map(|v| v.as_u64().unwrap()).sum();
    assert_eq!(total, 0, "nichts doppelt: {again}");
    assert!(again["result"]["commit"].is_null(), "ohne Änderung kein Commit: {again}");
}

#[tokio::test]
async fn a_changed_text_shows_up_with_a_link_and_arrives_as_a_further_revision() {
    let server = common::open().await;
    let work = import_new(&server).await;
    let changed = changed_xml();
    let preview = server.rpc("work.exchange_preview", json!({ "data": b64(changed.as_bytes()) })).await;
    let deviation = preview["result"]["deviations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["action"] == "fassung")
        .cloned()
        .unwrap_or_else(|| panic!("Abweichung als Fassung: {preview}"));
    let key: i64 = sqlx::query_scalar("SELECT id FROM node WHERE uuid = ?")
        .bind(WHEEL_UUID)
        .fetch_one(server.state.store.pool())
        .await
        .unwrap();
    assert_eq!(
        (deviation["key"].as_i64(), deviation["rev"].as_u64()),
        (Some(key), Some(3)),
        "Link-Ziel key/rev: {deviation}"
    );

    // Über die Formulare: Vorschau mit Link, Übernehmen mit Ergebnisseite.
    let (status, _, html) = server.send(upload("werk.baruch.xml", changed.as_bytes(), "")).await;
    assert_eq!(status, StatusCode::OK, "Vorschau: {html}");
    assert!(
        html.contains(&format!("href=\"/?work={work}&key={key}&rev=3\"")),
        "Link zur Abweichung in der Vorschau: {html}"
    );
    let file_sha = sha(changed.as_bytes());
    assert!(inbox(&server).contains(&file_sha), "Eingangsdatei liegt unter austausch-eingang/<sha256>");
    let (status, _, html) = server.send(take(&file_sha, "merge", "")).await;
    assert_eq!(status, StatusCode::OK, "Ergebnis: {html}");
    assert!(
        html.contains(&format!("href=\"/?work={work}&key={key}&rev=3\"")),
        "Link zur Abweichung im Ergebnis: {html}"
    );
    assert!(!inbox(&server).contains(&file_sha), "Eingangsdatei danach gelöscht");
    let current: i64 = sqlx::query_scalar("SELECT number FROM revision WHERE node_id = ? AND status = 'current'")
        .bind(key)
        .fetch_one(server.state.store.pool())
        .await
        .unwrap();
    assert_eq!(current, 1, "die aktuelle Markierung bleibt");
    let derived = baruch_store::exchange_apply::derive_uuid(
        "c0000000-0000-4000-8000-000000000111",
        "  == Das Rad\n\n\tEs dreht <&> und endet nie mit ]]> – Grüße.\r\nBirnen.  ",
    );
    let status: String = sqlx::query_scalar("SELECT status FROM revision WHERE uuid = ?")
        .bind(&derived)
        .fetch_one(server.state.store.pool())
        .await
        .unwrap();
    assert_eq!(status, "superseded", "weitere Fassung mit abgeleiteter Kennung, nie aktuell");
}

#[tokio::test]
async fn outline_differences_become_requests_with_a_link_to_them() {
    let server = common::open().await;
    let work = import_new(&server).await;
    let renamed = WERK.replace("<titel>Die Speichen</titel>", "<titel>Die Radspeichen</titel>");
    let file_sha = sha(renamed.as_bytes());
    let (status, _, _) = server.send(upload("werk.baruch.xml", renamed.as_bytes(), "")).await;
    assert_eq!(status, StatusCode::OK, "Vorschau");
    let (status, _, html) = server.send(take(&file_sha, "merge", "")).await;
    assert_eq!(status, StatusCode::OK, "Ergebnis: {html}");
    assert!(html.contains(&format!("href=\"/gliederung?work={work}#antraege\"")), "Link zu den Anträgen: {html}");
    let requests = server.state.store.outline_requests(work, true).await.unwrap();
    assert!(
        requests
            .iter()
            .any(|r| matches!(&r.op, baruch_store::OutlineOp::Rename { title, .. } if title == "Die Radspeichen")),
        "Antrag Rename: {requests:?}"
    );
    assert_eq!(
        requests[0].origin,
        baruch_store::IdeaOrigin::Human { name: "Austausch: Prüfrechner".into() },
        "Herkunft des Antrags"
    );
}

#[tokio::test]
async fn the_inbox_sweeps_files_older_than_a_day() {
    let server = common::open().await;
    let dir = server.path().join("austausch-eingang");
    std::fs::create_dir_all(&dir).unwrap();
    let old = dir.join("a".repeat(64));
    std::fs::write(&old, b"alt").unwrap();
    let file = std::fs::File::options().write(true).open(&old).unwrap();
    file.set_modified(std::time::SystemTime::now() - std::time::Duration::from_secs(25 * 60 * 60)).unwrap();
    let fresh = dir.join("b".repeat(64));
    std::fs::write(&fresh, b"neu").unwrap();
    let (status, _, _) = server.send(upload("werk.baruch.xml", WERK.as_bytes(), "")).await;
    assert_eq!(status, StatusCode::OK, "Vorschau");
    assert!(!old.exists(), "älter als 24 h → weggeräumt");
    assert!(fresh.exists(), "jüngere Einträge bleiben");
}

#[tokio::test]
async fn writes_from_a_foreign_origin_and_oversized_uploads_are_refused() {
    let server = common::open().await;
    for mut request in [upload("werk.baruch.xml", WERK.as_bytes(), ""), take(&"a".repeat(64), "new", "")] {
        request.headers_mut().insert(header::HOST, "127.0.0.1:8765".parse().unwrap());
        request.headers_mut().insert(header::ORIGIN, "https://evil.example".parse().unwrap());
        let (status, _, _) = server.send(request).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "fremde Origin → 403");
    }
    let big = vec![b'x'; baruch_server::exchange::import::UPLOAD_LIMIT + 1];
    let (status, _, _) = server.send(upload("riesig.baruch", &big, "")).await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE, "über UPLOAD_LIMIT → 413");
    assert!(inbox(&server).is_empty(), "nichts im Eingang");
}

#[tokio::test]
async fn every_rejection_on_every_way_writes_nothing() {
    let server = common::open().await;
    let before = counts(&server).await;
    let bad_files: Vec<(&str, Vec<u8>)> = vec![
        ("..-Pfad", container(&[("../boese.txt", b"x")])),
        ("DTD", include_bytes!("../../baruch-exchange/tests/data/import/lachen.baruch.xml").to_vec()),
        ("Prüfsumme", container_with(&[], true)),
        ("Unterversion falsch", WERK.replace("urn:baruch:werk:1", "urn:baruch:werk:9").into_bytes()),
    ];
    for (what, bytes) in &bad_files {
        let preview = server.rpc("work.exchange_preview", json!({ "data": b64(bytes) })).await;
        assert_eq!(preview["error"]["code"], -32602, "{what}: RPC-Vorschau lehnt ab: {preview}");
        let import = server
            .rpc("work.exchange_import", json!({ "data": b64(bytes), "sha256": sha(bytes), "mode": "new" }))
            .await;
        assert_eq!(import["error"]["code"], -32602, "{what}: RPC-Import lehnt ab: {import}");
        let (status, _, _) = server.send(upload("datei.baruch", bytes, "")).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{what}: Formular lehnt ab");
        assert_eq!(counts(&server).await, before, "{what}: nichts geschrieben");
        assert!(inbox(&server).is_empty(), "{what}: keine Eingangsdatei");
    }
    // Ablehnung erst im Übernehmen-Schritt (Modus passt nicht): nichts geschrieben, Eingangsdatei weg.
    let (status, _, _) = server.send(upload("werk.baruch.xml", WERK.as_bytes(), "")).await;
    assert_eq!(status, StatusCode::OK, "Vorschau geht");
    let (status, _, html) = server.send(take(&sha(WERK.as_bytes()), "merge", "")).await;
    assert_eq!(status, StatusCode::CONFLICT, "Modus passt nicht → 409: {html}");
    assert_eq!(counts(&server).await, before, "nichts geschrieben");
    assert!(inbox(&server).is_empty(), "keine Eingangsdatei bleibt nach einer Ablehnung beim Übernehmen");
    assert!(!server.path().join("works").join("die-muehle-am-bach").exists(), "kein Spiegel, also kein Commit");
}

#[tokio::test]
async fn a_container_reads_like_the_xml_file() {
    let server = common::open().await;
    let zip = container(&[]);
    let answer =
        server.rpc("work.exchange_import", json!({ "data": b64(&zip), "sha256": sha(&zip), "mode": "new" })).await;
    assert_eq!(answer["result"]["added"]["asset_revisions"], 2, "Bilder aus dem Archiv: {answer}");
    let bytes: i64 =
        sqlx::query_scalar("SELECT bytes FROM asset_revision WHERE uuid = 'c0000000-0000-4000-8000-000000000061'")
            .fetch_one(server.state.store.pool())
            .await
            .unwrap();
    assert_eq!(bytes, 70, "PNG mit seinen Bytes");
}

// ---- Kommandozeile -----------------------------------------------------------------------------

fn cli(data: &std::path::Path, args: &[&str]) -> (i32, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_baruch-server"))
        .args(args)
        .arg("--data")
        .arg(data)
        .env_remove("BARUCH_DATA")
        .output()
        .expect("baruch-server lässt sich nicht starten");
    (
        output.status.code().unwrap_or(-1),
        String::from_utf8(output.stdout).unwrap(),
        String::from_utf8(output.stderr).unwrap(),
    )
}

#[test]
fn the_command_line_imports_previews_and_merges_honestly() {
    let files = tempfile::tempdir().unwrap();
    let xml = files.path().join("muehle.baruch.xml");
    std::fs::write(&xml, WERK).unwrap();
    let zip = files.path().join("muehle.baruch");
    std::fs::write(&zip, container(&[])).unwrap();
    let typ = files.path().join("muehle.typ");
    std::fs::write(&typ, "= Mühle\n").unwrap();
    let xml = xml.to_str().unwrap();

    let data = tempfile::tempdir().unwrap();
    let data = data.path().join("daten");
    let (code, _, err) = cli(&data, &["import", xml, "--abgleichen"]);
    assert_eq!(code, 1, "--abgleichen bei unbekanntem Werk → Exit 1: {err}");
    assert!(err.contains("gibt es hier nicht"), "Grund: {err}");
    let (code, out, err) = cli(&data, &["import", xml, "--vorschau"]);
    assert_eq!(code, 0, "Vorschau: {err}");
    assert!(out.contains("nichts geschrieben") && out.contains("Modus: neu"), "Vorschau als Text: {out}");
    let (code, out, err) = cli(&data, &["import", xml]);
    assert_eq!(code, 0, "neu nach der Vorschau (die Vorschau schrieb nichts): {err}");
    assert!(out.contains("„Die Mühle am Bach“") && out.contains("Commit"), "Bericht: {out}");
    let (code, _, err) = cli(&data, &["import", xml]);
    assert_eq!(code, 1, "bekannt ohne --abgleichen → Exit 1");
    assert!(err.contains("Werk schon vorhanden") && err.contains("--abgleichen"), "Hinweis: {err}");
    let (code, out, err) = cli(&data, &["import", xml, "--abgleichen"]);
    assert_eq!(code, 0, "Abgleichen: {err}");
    assert!(out.contains("Hinzu: nichts"), "zweiter Abgleich legt nichts an: {out}");
    let (code, _, err) = cli(&data, &["import", typ.to_str().unwrap(), "--abgleichen"]);
    assert_eq!(code, 2, ".typ mit --abgleichen → Exit 2: {err}");

    let other = tempfile::tempdir().unwrap();
    let (code, out, err) = cli(&other.path().join("daten"), &["import", zip.to_str().unwrap()]);
    assert_eq!(code, 0, ".baruch neu: {err}");
    assert!(out.contains("2 Bilder"), "Bericht nennt die Bilder: {out}");
}
