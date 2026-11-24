// SPDX-License-Identifier: AGPL-3.0-or-later
//! Das Buch als PDF und die Druckprüfung (Vertrag Runde 11): RPC `work.pdf`, Seite `GET /werke/{id}/pdf`,
//! CLI `export --pdf`, gleiche Seitenzahl wie `preview.typst`, keine halbe Datei bei Fehlern, eine
//! Erzeugung je Werk, Zeitgrenze; Druckprüfung in `preview.typst`, `typst_check` und der Satz-Seite.
//! Werke frei erfunden, Testbilder entstehen im Test (PNG per Code, SVG als Text).
//!
//! Der Zwischenspeicher der Vorschau ist statisch: Jedes Werk hier hat einen eigenen Titel und Text, also
//! einen eigenen Stand.

use crate::common;

use std::net::SocketAddr;
use std::process::Command;
use std::time::Duration;

use axum::body::{Body, to_bytes};
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode, header};
use serde_json::{Value, json};
use tower::ServiceExt;

/// PNG in Graustufen (ungepackte Deflate-Blöcke) mit erfundenem Muster.
fn png(width: u32, height: u32) -> Vec<u8> {
    fn crc32(bytes: &[u8]) -> u32 {
        let mut crc = 0xffff_ffffu32;
        for &b in bytes {
            crc ^= u32::from(b);
            for _ in 0..8 {
                crc = if crc & 1 == 1 { (crc >> 1) ^ 0xedb8_8320 } else { crc >> 1 };
            }
        }
        !crc
    }
    fn chunk(out: &mut Vec<u8>, kind: &[u8], data: &[u8]) {
        out.extend((data.len() as u32).to_be_bytes());
        let mut body = kind.to_vec();
        body.extend(data);
        out.extend(&body);
        out.extend(crc32(&body).to_be_bytes());
    }
    let mut raw = Vec::new();
    for y in 0..height {
        raw.push(0);
        raw.extend((0..width).map(|x| ((x * 5 + y * 11) % 256) as u8));
    }
    let mut z = vec![0x78, 0x01];
    let blocks: Vec<&[u8]> = raw.chunks(65_535).collect();
    for (i, block) in blocks.iter().enumerate() {
        z.push(u8::from(i + 1 == blocks.len()));
        let len = block.len() as u16;
        z.extend(len.to_le_bytes());
        z.extend((!len).to_le_bytes());
        z.extend(*block);
    }
    let (mut a, mut b) = (1u32, 0u32);
    for &byte in &raw {
        a = (a + u32::from(byte)) % 65_521;
        b = (b + a) % 65_521;
    }
    z.extend(((b << 16) | a).to_be_bytes());
    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut ihdr = Vec::new();
    ihdr.extend(width.to_be_bytes());
    ihdr.extend(height.to_be_bytes());
    ihdr.extend([8, 0, 0, 0, 0]);
    chunk(&mut out, b"IHDR", &ihdr);
    chunk(&mut out, b"IDAT", &z);
    chunk(&mut out, b"IEND", &[]);
    out
}

/// Wie oft `name` mit dem Wert `value` vorkommt (`/Type /Page` bzw. kompakt `/Type/Page`, nicht `/Pages`).
fn entries(pdf: &[u8], name: &str, value: &str) -> usize {
    let mut count = 0;
    let mut at = 0;
    while let Some(found) = pdf[at..].windows(name.len()).position(|w| w == name.as_bytes()) {
        let mut i = at + found + name.len();
        while pdf.get(i).is_some_and(u8::is_ascii_whitespace) {
            i += 1;
        }
        let rest = &pdf[i..];
        if rest.starts_with(value.as_bytes()) && !rest.get(value.len()).is_some_and(u8::is_ascii_alphanumeric) {
            count += 1;
        }
        at += found + name.len();
    }
    count
}

fn contains(pdf: &[u8], text: &str) -> bool {
    pdf.windows(text.len()).any(|w| w == text.as_bytes())
}

/// Die Markierung „PDF entsteht“ gilt je Werk-id im ganzen Prozess; alle Server-Tests (ein Bündel) haben
/// eigene Datenordner mit denselben ids und erzeugen PDFs darum nacheinander (gemeinsame Sperre).
use crate::common::PDF_SERIAL as SERIAL;

fn result(answer: &Value) -> &Value {
    assert!(answer["error"].is_null(), "RPC sollte gelingen: {answer}");
    &answer["result"]
}

/// Erfundenes Buch: drei Kapitel mit Seitenumbruch, im ersten ein PNG.
fn book(title: &str) -> String {
    format!(
        "#set document(title: \"{title}\", author: \"Erna Erfunden\")\n#set page(width: 12cm, height: 16cm)\n\
         #set text(lang: \"de\")\n\n= Aussaat\n\nIm März kommen die Bohnen in die Erde.\n\n\
         #figure(image(\"bilder/beet.png\", width: 4cm), caption: [Das Beet])\n\n#pagebreak()\n\n= Ernte\n\n\
         Im August füllt sich der Korb.\n\n#pagebreak()\n\n= Ruhe\n\nIm Winter schläft das Beet.\n"
    )
}

async fn add_png(server: &common::Server, work: i64, name: &str, path: &str, bytes: &[u8]) {
    let added = server
        .rpc(
            "asset.add",
            json!({ "work": work, "file_name": name, "content": baruch_store::encode_base64(bytes),
                    "encoding": "base64", "path": path }),
        )
        .await;
    assert!(added["result"].is_object(), "Bild muss sich anlegen lassen: {added}");
}

/// Werk mit Bild anlegen: (id, slug).
async fn book_work(server: &common::Server, title: &str) -> (i64, String) {
    let report = server.import(&book(title), &format!("{}.typ", report_name(title))).await;
    add_png(server, report.work, "beet.png", "bilder/beet.png", &png(64, 48)).await;
    (report.work, report.slug)
}

fn report_name(title: &str) -> String {
    title.to_lowercase().replace(' ', "-")
}

/// GET mit Kopfzeilen und Bytes.
async fn get_bytes(server: &common::Server, uri: &str) -> (StatusCode, axum::http::HeaderMap, Vec<u8>) {
    let response = server.app.clone().oneshot(Request::get(uri).body(Body::empty()).unwrap()).await.unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap().to_vec();
    (status, headers, bytes)
}

#[tokio::test]
async fn work_pdf_and_the_download_give_a_valid_pdf_with_the_pages_of_the_preview() {
    let _serial = SERIAL.lock().await;
    let server = common::open().await;
    let (work, slug) = book_work(&server, "Bohnenbuch Eins").await;
    let preview = server.rpc("preview.typst", json!({ "work": work })).await;
    let pages = result(&preview)["pages"].as_u64().unwrap() as usize;
    assert_eq!(pages, 3, "Voraussetzung: drei Kapitel mit zwei Umbrüchen sind drei Seiten");

    let answer = server.rpc("work.pdf", json!({ "work": work, "inline": true })).await;
    let r = result(&answer);
    assert_eq!(r["file_name"], format!("{slug}.pdf"), "Dateiname aus dem slug");
    assert_eq!(r["url"], format!("/werke/{work}/pdf"), "Adresse zum Herunterladen");
    assert_eq!(r["pages"].as_u64().unwrap() as usize, pages, "work.pdf nennt dieselbe Seitenzahl wie preview.typst");
    let inline = baruch_store::decode_base64(r["pdf_base64"].as_str().expect("inline: true liefert Base64")).unwrap();
    assert_eq!(r["bytes"].as_u64().unwrap() as usize, inline.len(), "bytes ist die Länge des PDF");
    assert_eq!(r["sha256"], baruch_render::sha256_hex(&inline), "sha256 gehört zu genau diesen Bytes");
    let print = r["print"].as_array().unwrap();
    assert_eq!(print.len(), 1, "64 Pixel auf 4 cm sind etwa 40 dpi: work.pdf meldet das Bild in print: {print:?}");
    assert_eq!(print[0]["path"], "bilder/beet.png", "mit Pfad");

    let plain = server.rpc("work.pdf", json!({ "work": work })).await;
    assert!(result(&plain).get("pdf_base64").is_none(), "ohne inline keine Bytes in der Antwort");

    let (status, headers, bytes) = get_bytes(&server, &format!("/werke/{work}/pdf")).await;
    assert_eq!(status, StatusCode::OK, "Herunterladen gelingt");
    assert_eq!(headers[header::CONTENT_TYPE], "application/pdf", "MIME-Typ PDF");
    assert_eq!(
        headers[header::CONTENT_DISPOSITION],
        format!("attachment; filename=\"{slug}.pdf\""),
        "als Anhang mit dem Namen <slug>.pdf"
    );
    assert!(bytes.starts_with(b"%PDF-"), "PDF-Kopf");
    assert!(contains(&bytes[bytes.len().saturating_sub(32)..], "%%EOF"), "vollständig bis %%EOF");
    assert_eq!(entries(&bytes, "/Type", "/Page"), pages, "gezählte Seitenobjekte = Seiten der Satz-Ansicht");
    assert!(contains(&bytes, "/FontFile"), "Schrift eingebettet");
    assert!(entries(&bytes, "/Subtype", "/Image") >= 1, "das selbst erzeugte PNG steht als Bild im PDF");
}

#[tokio::test]
async fn a_typst_error_gives_422_with_links_refused_over_rpc_and_no_pdf() {
    let _serial = SERIAL.lock().await;
    let server = common::open().await;
    let source = "#set document(title: \"Kaputtes Beet\", author: \"Erna Erfunden\")\n\n= Beet\n\nErde.\n\n\
                  = Unkraut\n\n#unbekannt()\n";
    let report = server.import(source, "kaputt.typ").await;
    let work = report.work;
    let loaded = server.state.store.load_work(work).await.unwrap();
    let weed = loaded.nodes.iter().find(|n| n.title == "Unkraut").unwrap().key;

    let answer = server.rpc("work.pdf", json!({ "work": work })).await;
    assert_eq!(answer["error"]["code"], -32000, "Satzfehler → REFUSED: {answer}");
    let message = answer["error"]["message"].as_str().unwrap();
    assert!(message.starts_with("Kein PDF (Buch): Der Satz scheitert mit 1 Fehler."), "Zahl der Fehler: {message}");
    assert!(message.contains("unbekannt"), "die erste Meldung steht darin: {message}");

    let (status, headers, body) = get_bytes(&server, &format!("/werke/{work}/pdf")).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "Satzfehler → 422, kein PDF");
    assert!(headers[header::CONTENT_TYPE].to_str().unwrap().starts_with("text/html"), "eine Seite, kein PDF");
    let html = String::from_utf8(body).unwrap();
    assert!(!html.starts_with("%PDF"), "nie eine halbe Datei");
    assert!(html.contains("Der Satz scheitert"), "die Seite sagt, warum: {html}");
    assert!(
        html.contains(&format!("href=\"/?work={work}&amp;key={weed}\"")),
        "Link zum Abschnitt mit dem Fehler: {html}"
    );
    assert!(html.contains("<header class=\"topbar\">"), "mit Kopfzeile wie jede Seite");
}

#[tokio::test]
async fn a_second_pdf_for_the_same_work_is_refused_while_one_is_running() {
    let _serial = SERIAL.lock().await;
    let server = common::open().await;
    let (work, _) = book_work(&server, "Bohnenbuch Zwei").await;
    let (other, _) = book_work(&server, "Bohnenbuch Drei").await;
    let running = baruch_server::pdf::claim(work).expect("niemand erzeugt gerade ein PDF für dieses Werk");
    let (status, _, body) = get_bytes(&server, &format!("/werke/{work}/pdf")).await;
    assert_eq!(status, StatusCode::CONFLICT, "läuft schon → 409");
    assert!(String::from_utf8(body).unwrap().contains("entsteht gerade ein PDF"), "mit ehrlicher Meldung");
    let answer = server.rpc("work.pdf", json!({ "work": work })).await;
    assert_eq!(answer["error"]["code"], -32000, "über RPC REFUSED: {answer}");
    assert_eq!(answer["error"]["message"], baruch_server::pdf::BUSY, "dieselbe Meldung");
    let (status, _, _) = get_bytes(&server, &format!("/werke/{other}/pdf")).await;
    assert_eq!(status, StatusCode::OK, "ein anderes Werk ist nicht betroffen");
    drop(running);
    let (status, _, _) = get_bytes(&server, &format!("/werke/{work}/pdf")).await;
    assert_eq!(status, StatusCode::OK, "nach dem Ende geht es wieder");
}

#[tokio::test]
async fn the_time_limit_refuses_honestly() {
    let _serial = SERIAL.lock().await;
    let server = common::open().await;
    let (work, _) = book_work(&server, "Bohnenbuch Vier").await;
    let refused = baruch_server::pdf::book_pdf_with_limit(&server.state, work, Duration::ZERO)
        .await
        .expect_err("Limit 0 läuft ab");
    assert_eq!(refused.code, -32000, "Zeit abgelaufen → REFUSED");
    assert_eq!(refused.message, "Typst brauchte länger als 0 s", "die Meldung nennt das Limit");
    assert_eq!(baruch_server::pdf::PDF_TIMEOUT, Duration::from_secs(120), "im Betrieb 120 s");
    // Der abgelaufene Lauf rechnet im Hintergrund zu Ende und hält so lange das Werk; danach ist es frei.
    let freed = async {
        loop {
            if let Some(claim) = baruch_server::pdf::claim(work) {
                break claim;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    };
    let claim = tokio::time::timeout(Duration::from_secs(60), freed).await.expect("der Hintergrundlauf endet");
    drop(claim);
}

#[tokio::test]
async fn unknown_works_are_404_and_invalid_params_over_rpc() {
    let _serial = SERIAL.lock().await;
    let server = common::open().await;
    let (status, _, _) = get_bytes(&server, "/werke/999/pdf").await;
    assert_eq!(status, StatusCode::NOT_FOUND, "unbekanntes Werk");
    let answer = server.rpc("work.pdf", json!({ "work": 999 })).await;
    assert_eq!(answer["error"]["code"], -32602, "unbekanntes Werk über RPC: {answer}");
    let work = server.state.default_work().await.unwrap();
    let answer = server.rpc("work.pdf", json!({ "work": work, "inline": "ja" })).await;
    assert_eq!(answer["error"]["code"], -32602, "inline nur true/false: {answer}");
}

// ---- Kommandozeile -------------------------------------------------------------------------------

fn cli(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_baruch-server")).args(args).output().expect("baruch-server startet")
}

#[tokio::test]
async fn the_cli_writes_a_valid_pdf_never_overwrites_and_writes_nothing_on_errors() {
    let _serial = SERIAL.lock().await;
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().join("daten");
    let (good, bad) = {
        let state = baruch_server::AppState::open(&data).await.unwrap();
        let good = baruch_server::import::import_typst(&state, &book("Bohnenbuch Fünf"), "fuenf.typ").await.unwrap();
        let bytes = png(64, 48);
        let new = baruch_store::NewAsset {
            file_name: "beet.png",
            name: None,
            bytes: &bytes,
            as_base64: true,
            path: Some("bilder/beet.png"),
            caption: "",
            alt: "",
            note: "",
        };
        state.store.add_asset(good.work, &new, "Autor").await.unwrap();
        let broken = "#set document(title: \"Kaputt per Kommandozeile\")\n\n= Beet\n\n#unbekannt()\n";
        let bad = baruch_server::import::import_typst(&state, broken, "kaputt.typ").await.unwrap();
        state.store.pool().close().await;
        (good, bad)
    };
    let data_arg = data.to_str().unwrap();
    let target = dir.path().join("buch.pdf");
    let out = cli(&["export", &good.slug, "--pdf", target.to_str().unwrap(), "--data", data_arg]);
    assert!(out.status.success(), "export --pdf gelingt: {}", String::from_utf8_lossy(&out.stderr));
    assert!(String::from_utf8_lossy(&out.stdout).contains("3 Seiten"), "nennt die Seitenzahl");
    let written = std::fs::read(&target).unwrap();
    assert!(written.starts_with(b"%PDF-") && contains(&written, "%%EOF"), "gültiges PDF");
    assert_eq!(entries(&written, "/Type", "/Page"), 3, "drei Seiten wie in der Vorschau");

    let again = cli(&["export", &good.work.to_string(), "--pdf", target.to_str().unwrap(), "--data", data_arg]);
    assert_eq!(again.status.code(), Some(1), "vorhandene Datei → Exit 1");
    assert!(String::from_utf8_lossy(&again.stderr).contains("nichts überschrieben"), "mit Grund");
    assert_eq!(std::fs::read(&target).unwrap(), written, "die vorhandene Datei bleibt unverändert");

    let failed = dir.path().join("kaputt.pdf");
    let out = cli(&["export", &bad.slug, "--pdf", failed.to_str().unwrap(), "--data", data_arg]);
    assert_eq!(out.status.code(), Some(1), "Satzfehler → Exit 1");
    assert!(String::from_utf8_lossy(&out.stderr).contains("Der Satz scheitert"), "mit Meldung");
    assert!(!failed.exists(), "bei einem Fehler entsteht keine Datei");

    let both = dir.path().join("beides.pdf");
    let out_dir = dir.path().join("ordner");
    let out = cli(&[
        "export",
        &good.slug,
        "--pdf",
        both.to_str().unwrap(),
        "--out",
        out_dir.to_str().unwrap(),
        "--data",
        data_arg,
    ]);
    assert_eq!(out.status.code(), Some(2), "--pdf und --out zusammen → Bedienfehler (Exit 2)");
    assert!(String::from_utf8_lossy(&out.stderr).contains("schließen sich aus"), "mit Hinweis");
    assert!(!both.exists() && !out_dir.exists(), "und nichts geschrieben");
}

// ---- Druckprüfung --------------------------------------------------------------------------------

/// Werk mit einem Bild auf einer A5-Seite ohne Rand.
async fn print_work(server: &common::Server, title: &str, image: Option<(&str, Vec<u8>)>) -> i64 {
    let figure = match &image {
        Some((name, _)) => format!(
            "#page(paper: \"a5\", margin: 0pt)[#image(\"bilder/{name}\", width: 100%, height: 100%, fit: \"cover\")]\n"
        ),
        None => "#image(\"bilder/fehlt.png\", width: 3cm)\n".to_owned(),
    };
    let source = format!(
        "#set document(title: \"{title}\", author: \"Erna Erfunden\")\n#set page(paper: \"a5\")\n\n= Umschlag\n\n\
         Vorne.\n\n{figure}"
    );
    let report = server.import(&source, &format!("{}.typ", report_name(title))).await;
    if let Some((name, bytes)) = image {
        add_png(server, report.work, name, &format!("bilder/{name}"), &bytes).await;
    }
    report.work
}

async fn typst_check(server: &common::Server, work: i64) -> Value {
    let message = json!({ "jsonrpc": "2.0", "id": 4, "method": "tools/call",
                          "params": { "name": "typst_check", "arguments": { "work": work } } });
    let mut request = Request::post("/mcp")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(message.to_string()))
        .unwrap();
    request.extensions_mut().insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 40000))));
    let (status, _, body) = server.send(request).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let answer: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(answer["result"]["isError"], false, "typst_check gelingt: {answer}");
    answer["result"]["structuredContent"].clone()
}

#[tokio::test]
async fn a_generated_picture_on_a_full_a5_page_is_reported_everywhere_below_300_dpi() {
    let server = common::open().await;
    let work = print_work(&server, "Druckprobe Grob", Some(("umschlag.png", png(832, 1216)))).await;

    let preview = server.rpc("preview.typst", json!({ "work": work })).await;
    let r = result(&preview);
    let print = r["print"].as_array().expect("preview.typst trägt print");
    assert_eq!(print.len(), 1, "genau eine Warnung: {print:?}");
    let w = &print[0];
    assert_eq!(w["path"], "bilder/umschlag.png", "Pfad in Normalform");
    assert_eq!(w["page"], 2, "das Bild steht auf der eigenen Seite 2");
    assert!(w["dpi"].as_f64().unwrap() < 300.0, "unter 300 dpi: {w}");
    assert!(
        w["needed_width_px"].as_u64().unwrap() >= 1748 && w["needed_height_px"].as_u64().unwrap() >= 2480,
        "für A5 bei 300 dpi braucht es mindestens 1748 × 2480 Pixel: {w}"
    );
    assert_eq!(r["diagnostics"].as_array().unwrap().len(), 0, "die Druckprüfung zählt nicht als Meldung: {r}");

    let check = typst_check(&server, work).await;
    assert_eq!(check["print"], r["print"], "typst_check meldet dasselbe");
    assert_eq!((check["errors"].clone(), check["warnings"].clone()), (json!(0), json!(0)), "Zählungen unverändert");

    let (status, html) = server.get(&format!("/werke/{work}/satz")).await;
    assert_eq!(status, StatusCode::OK, "Satz-Seite lädt");
    let at = html.find("id=\"druckpruefung\"").expect("Abschnitt #druckpruefung");
    let section = &html[at..at + html[at..].find("</section>").unwrap()];
    let message = w["message"].as_str().unwrap();
    assert!(section.contains(message), "die Warnung steht in der Druckprüfung: {section}");
    assert!(section.contains("dpi"), "mit dpi");
    assert!(
        html.contains(&format!("<a class=\"btn\" href=\"/werke/{work}/pdf\" download")),
        "Knopf „PDF herunterladen“ auf der Satz-Seite"
    );
}

#[tokio::test]
async fn enough_pixels_or_svg_give_no_warning_and_missing_pictures_are_named() {
    let server = common::open().await;
    let fine = print_work(&server, "Druckprobe Fein", Some(("fein.png", png(1800, 2600)))).await;
    let preview = server.rpc("preview.typst", json!({ "work": fine })).await;
    assert!(result(&preview)["print"].as_array().unwrap().is_empty(), "1800 × 2600 Pixel reichen für A5");
    let (_, html) = server.get(&format!("/werke/{fine}/satz")).await;
    assert!(html.contains("kein Rasterbild liegt unter 300 dpi"), "die Druckprüfung sagt, dass alles passt");

    let missing = print_work(&server, "Druckprobe Ohne Bild", None).await;
    let (status, html) = server.get(&format!("/werke/{missing}/satz")).await;
    assert_eq!(status, StatusCode::OK, "auch mit fehlendem Bild lädt die Seite");
    let at = html.find("id=\"druckpruefung\"").expect("Abschnitt #druckpruefung");
    let section = &html[at..at + html[at..].find("</section>").unwrap()];
    // minijinja maskiert / im Pfad als &#x2f; (der Browser zeigt „bilder/fehlt.png“).
    assert!(section.contains("Bild fehlt: bilder&#x2f;fehlt.png"), "das fehlende Bild steht deutlich da: {section}");
    assert!(section.contains(&format!("href=\"/werke/{missing}/bilder\"")), "mit Link zur Seite „Bilder“: {section}");
    let check = typst_check(&server, missing).await;
    assert_eq!(check["missing"], json!(["bilder/fehlt.png"]), "typst_check nennt fehlende Bilder");
}
