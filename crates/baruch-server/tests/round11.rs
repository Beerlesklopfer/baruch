// SPDX-License-Identifier: AGPL-3.0-or-later
//! End to end Round 11 (integrator): „Bilder und Umschlag“ after merging mcp-assets, cover and images. What is checked
//! is the interplay across the whole router:
//! - cover through an MCP request (`asset_add` with `role: "cover"`), confirmed by the human;
//! - book with cover and front matter: typesetting view and PDF (RPC, route, command line) have the same number of
//!   pages, page 1 is the cover, the last one the back with blurb and EAN-13;
//! - a work without cover and front matter typesets unchanged (same state);
//! - cover PDF with spine width from the page count and bleed;
//! - the print check reports an 832 × 1216 pixel image (created in the test) below 300 dpi;
//! - page „Bilder“ with cover block, requests and search; inserting an image; outline and work overview show the cover
//!   image.
//!
//! Text freely invented, test images are created in the test (PNG by code, SVG as text).

use crate::common;

use std::net::SocketAddr;
use std::process::Command;

use axum::body::{Body, to_bytes};
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode, header};
use serde_json::{Value, json};
use tower::ServiceExt;

/// The mark „PDF entsteht“ holds per work id in the whole process; all server tests (one bundle) have data folders of
/// their own with the same ids and therefore build PDFs one after the other (shared lock).
use crate::common::PDF_SERIAL as SERIAL;

/// Valid ISBN-13 (textbook example).
const ISBN: &str = "978-3-16-148410-0";

/// Invented book: work template with page size 12 × 16 cm, three chapters with page break.
fn book(title: &str) -> String {
    format!(
        "#set document(title: \"{title}\", author: \"Hanne Holm\")\n#set page(width: 12cm, height: 16cm)\n\
         #set text(lang: \"de\")\n\n= Ebbe\n\nAm Morgen liegt das Watt bis zum Horizont.\n\n#pagebreak()\n\n\
         = Flut\n\nMittags läuft das Wasser in die Priele.\n\n#pagebreak()\n\n= Nacht\n\nDas Feuer dreht sich über dem Kai.\n"
    )
}

/// PNG in greyscale (unpacked deflate blocks), dark invented pattern.
fn dark_png(width: u32, height: u32) -> Vec<u8> {
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
        raw.extend((0..width).map(|x| (20 + (x * 3 + y * 7) % 60) as u8));
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

const KAI: &str = "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"80\" height=\"40\">\
<rect width=\"80\" height=\"40\" fill=\"#335577\"/></svg>\n";

fn result(answer: &Value) -> &Value {
    assert!(answer["error"].is_null(), "RPC sollte gelingen: {answer}");
    &answer["result"]
}

/// How often `name` occurs with the value `value` (`/Type /Page`, not `/Pages`).
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

/// Width and height (in mm) of all `/MediaBox [x0 y0 x1 y1]` in the PDF.
fn media_boxes_mm(pdf: &[u8]) -> Vec<(f64, f64)> {
    let text = String::from_utf8_lossy(pdf);
    let mut boxes = Vec::new();
    let mut rest = text.as_ref();
    while let Some(at) = rest.find("/MediaBox") {
        rest = &rest[at + "/MediaBox".len()..];
        let Some(open) = rest.find('[') else { break };
        let Some(close) = rest.find(']') else { break };
        let numbers: Vec<f64> = rest[open + 1..close].split_whitespace().filter_map(|n| n.parse().ok()).collect();
        if numbers.len() == 4 {
            let pt = 25.4 / 72.0;
            boxes.push(((numbers[2] - numbers[0]) * pt, (numbers[3] - numbers[1]) * pt));
        }
    }
    boxes
}

async fn mcp_post(server: &common::Server, session: Option<&str>, message: Value) -> (axum::http::HeaderMap, Value) {
    let mut request = Request::post("/mcp").header(header::CONTENT_TYPE, "application/json");
    if let Some(id) = session {
        request = request.header("mcp-session-id", id);
    }
    let mut request = request.body(Body::from(message.to_string())).unwrap();
    request.extensions_mut().insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 40000))));
    let (status, headers, body) = server.send(request).await;
    assert_eq!(status, StatusCode::OK, "MCP antwortet: {body}");
    (headers, serde_json::from_str(&body).unwrap())
}

async fn mcp_session(server: &common::Server) -> String {
    let (headers, answer) = mcp_post(
        server,
        None,
        json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize",
                "params": { "protocolVersion": "2025-06-18", "capabilities": {},
                            "clientInfo": { "name": "Claude Code", "version": "2.1" } } }),
    )
    .await;
    assert!(answer["result"].is_object(), "initialize: {answer}");
    headers.get("mcp-session-id").unwrap().to_str().unwrap().to_owned()
}

async fn mcp_tool(server: &common::Server, session: &str, tool: &str, arguments: Value) -> Value {
    let message = json!({ "jsonrpc": "2.0", "id": 3, "method": "tools/call",
                          "params": { "name": tool, "arguments": arguments } });
    let answer = mcp_post(server, Some(session), message).await.1;
    assert_eq!(answer["result"]["isError"], false, "{tool} sollte gelingen: {answer}");
    answer["result"]["structuredContent"].clone()
}

async fn get_bytes(server: &common::Server, uri: &str) -> (StatusCode, axum::http::HeaderMap, Vec<u8>) {
    let response = server.app.clone().oneshot(Request::get(uri).body(Body::empty()).unwrap()).await.unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap().to_vec();
    (status, headers, bytes)
}

/// Import the work, request the cover through MCP and confirm it through the form, set the book details. Response:
/// (work, slug).
async fn lighthouse(server: &common::Server, title: &str, file: &str) -> (i64, String) {
    let report = server.import(&book(title), file).await;
    let (work, slug) = (report.work, report.slug);
    let session = mcp_session(server).await;
    let added = mcp_tool(
        server,
        &session,
        "asset_add",
        json!({ "work": work, "file_name": "umschlag.png", "encoding": "base64",
                "content": baruch_store::encode_base64(&dark_png(832, 1216)),
                "caption": "Leuchtturm bei Nacht, Bild: erfunden", "alt": "Ein dunkler Leuchtturm", "role": "cover",
                "note": "dunkles Hochformat" }),
    )
    .await;
    assert_eq!(added["name"], "umschlag.png", "asset_add legt das Bild sofort an: {added}");
    let request = added["request"]["id"].as_i64().expect("role cover öffnet einen Antrag");
    assert!(
        result(&server.rpc("work.get", json!({ "work": work })).await)["cover"].is_null(),
        "der Antrag allein setzt keinen Umschlag"
    );
    let (status, location) =
        server.post_form("/werke/bilder/antraege/act", &format!("work={work}&id={request}&decision=confirm")).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Formular „Bestätigen“ ohne JavaScript");
    assert_eq!(
        location.as_deref(),
        Some(format!("/werke/{work}/bilder?hinweis=antrag_bestaetigt#antraege").as_str()),
        "zurück zur Bilder-Seite mit Hinweis (der Antrag ist nicht veraltet)"
    );
    let set = server
        .rpc(
            "work.meta.set",
            json!({ "work": work, "front_matter": true, "title_area": "middle", "isbn": ISBN,
                    "publisher": "Wattverlag", "place": "Kressbach", "year": "2026", "edition": "1. Auflage",
                    "blurb": "Ein Jahr am Leuchtturm.", "bio": "Hanne Holm hütet das Feuer." }),
        )
        .await;
    result(&set);
    (work, slug)
}

#[tokio::test]
async fn a_cover_requested_over_mcp_and_confirmed_by_the_human_frames_the_book_in_preview_and_pdf() {
    let _serial = SERIAL.lock().await;
    let server = common::open().await;
    let (work, slug) = lighthouse(&server, "Leuchtturmjahr Eins", "eins.typ").await;

    // The cover is set, with origin in the mirror.
    let got = server.rpc("work.get", json!({ "work": work })).await;
    let cover = &result(&got)["cover"];
    assert_eq!(cover["name"], "umschlag.png", "work.get nennt den Umschlag: {got}");
    assert_eq!(cover["thumb"], format!("/werke/{work}/bilder/umschlag.png/vorschau?b=320"), "mit Vorschaubild");
    if let Some(log) = common::git(&server.mirror_dir_of(&slug), &["log", "-3", "--format=%B%n----"]) {
        // Round 20c: one commit per confirmed cover request („Bildantrag n bestätigt“ with cover before/after).
        assert!(
            log.contains("Baruch-Action: asset-request-confirmed") && log.contains("Umschlag nachher: umschlag.png"),
            "Commit des bestätigten Umschlag-Antrags im Spiegel: {log}"
        );
        assert!(log.contains("@mcp:claude-code"), "die Herkunft (MCP-Client) steht im Spiegel: {log}");
        assert!(log.contains("Baruch-Confirmed-By: Autor"), "der bestätigende Mensch steht im Spiegel: {log}");
    }

    // Typesetting view and PDF: the same number of pages, first page cover, last page back.
    let preview = server.rpc("preview.typst", json!({ "work": work })).await;
    let preview = result(&preview);
    let pages = preview["pages"].as_u64().unwrap() as usize;
    // 3 chapters + front, inside and back + 4 pages of front matter (without dedication).
    assert_eq!(pages, 3 + 3 + 4, "Buch mit Rahmen: {preview}");
    let typeset = baruch_server::typeset::typeset(&server.state, work).await.unwrap();
    let rendered = typeset.rendered();
    assert!(rendered.pages[0].contains("<image"), "Seite 1 ist der Umschlag mit Bild");
    assert_eq!(
        rendered.headings.first().map(|h| h.page),
        Some(7),
        "Kapitel 1 nach Umschlag (3 Seiten) und Titelei (4) physisch auf Seite 7, rechts"
    );
    assert!(rendered.pages.last().unwrap().contains("<image"), "letzte Seite: Rückseite mit Strichcode-Bild");

    let prepared = baruch_server::typeset::prepare(&server.state, work).await.unwrap();
    let main = &prepared.source.main;
    let front = &main[..prepared.map.sections[0].start];
    let back = &main[prepared.map.sections.last().unwrap().end..];
    for text in [
        "\"Leuchtturmjahr Eins\"",
        "Bibliografische Information der Deutschen Nationalbibliothek",
        "© 2026 Hanne Holm, Wattverlag, Kressbach",
        "ISBN 978-3-16-148410-0",
        "Leuchtturm bei Nacht, Bild: erfunden",
        "Gesetzt mit Baruch",
    ] {
        assert!(front.contains(text), "Titelei aus den Werkangaben enthält {text:?}:\n{front}");
    }
    assert!(back.contains("Ein Jahr am Leuchtturm.") && back.contains(".baruch/isbn.svg"), "Rückseite: {back}");
    assert_eq!(
        baruch_typst::cover::isbn13(ISBN).as_deref(),
        Ok("9783161484100"),
        "Strichcode aus der geprüften ISBN (Prüfziffer 0)"
    );
    assert!(baruch_typst::cover::isbn13("978-3-16-148410-1").is_err(), "falsche Prüfziffer wird abgelehnt");
    let barcode = prepared.source.files.iter().find(|(p, _)| p == ".baruch/isbn.svg").expect("Strichcode beigelegt");
    assert_eq!(
        String::from_utf8_lossy(&barcode.1),
        baruch_typst::cover::ean13_svg("9783161484100"),
        "der beigelegte Strichcode ist die EAN-13 der ISBN"
    );

    // PDF through RPC and route: the same number of pages as the typesetting view.
    let answer = server.rpc("work.pdf", json!({ "work": work })).await;
    assert_eq!(result(&answer)["pages"].as_u64().unwrap() as usize, pages, "work.pdf = Satz-Ansicht: {answer}");
    let (status, headers, bytes) = get_bytes(&server, &format!("/werke/{work}/pdf")).await;
    assert_eq!(status, StatusCode::OK, "Buch-PDF lässt sich herunterladen");
    assert_eq!(headers[header::CONTENT_TYPE], "application/pdf", "als PDF");
    assert!(bytes.starts_with(b"%PDF-") && contains(&bytes[bytes.len().saturating_sub(32)..], "%%EOF"), "vollständig");
    assert_eq!(entries(&bytes, "/Type", "/Page"), pages, "Seitenobjekte im PDF = Seiten der Satz-Ansicht");
    assert!(entries(&bytes, "/Subtype", "/Image") >= 1, "das Umschlagbild steht im PDF");

    // Print check: 832 × 1216 pixels on 12 × 16 cm are clearly below 300 dpi.
    let print = preview["print"].as_array().unwrap();
    let warning = print
        .iter()
        .find(|w| w["path"].as_str().is_some_and(|p| p.ends_with("umschlag.png")))
        .unwrap_or_else(|| panic!("Umschlagbild in der Druckprüfung: {print:?}"));
    assert_eq!((warning["width_px"].clone(), warning["height_px"].clone()), (json!(832), json!(1216)), "{warning}");
    assert!(warning["dpi"].as_f64().unwrap() < 300.0, "unter 300 dpi: {warning}");
    assert_eq!(warning["page"], 1, "auf der Umschlagseite 1");
    let (status, html) = server.get(&format!("/werke/{work}/satz")).await;
    assert_eq!(status, StatusCode::OK, "Satz-Seite lädt");
    let at = html.find("id=\"druckpruefung\"").expect("Abschnitt #druckpruefung");
    let section = &html[at..at + html[at..].find("</section>").unwrap()];
    assert!(section.contains("dpi") && section.contains("umschlag.png"), "Warnung auf der Satz-Seite: {section}");
}

#[tokio::test]
async fn a_work_without_cover_and_front_matter_is_set_unchanged() {
    let server = common::open().await;
    let report = server.import(&book("Leuchtturmjahr Ohne"), "ohne.typ").await;
    let work = report.work;
    let map = server.state.store.export_map(work).await.unwrap();
    let files = server.state.store.export_assets(work).await.unwrap().into_iter().map(|f| (f.path, f.bytes)).collect();
    let plain = baruch_render::Source { main: map.typst.clone(), files };
    let prepared = baruch_server::typeset::prepare(&server.state, work).await.unwrap();
    assert_eq!(prepared.source, plain, "ohne Umschlag und Titelei geht der Export byte-gleich in den Satz");
    assert_eq!(prepared.stand, baruch_render::stand(&plain), "derselbe Stand wie vor Runde 11");
    let preview = server.rpc("preview.typst", json!({ "work": work })).await;
    assert_eq!(result(&preview)["pages"], 3, "drei Kapitel, drei Seiten – kein Rahmen");
    assert!(result(&server.rpc("work.get", json!({ "work": work })).await)["cover"].is_null(), "kein Umschlag");

    // Front matter without cover: exactly 4 pages more, chapters stay unchanged in the typesetting.
    result(&server.rpc("work.meta.set", json!({ "work": work, "front_matter": true })).await);
    let preview = server.rpc("preview.typst", json!({ "work": work })).await;
    assert_eq!(result(&preview)["pages"], 3 + 4, "nur die Titelei kommt dazu: {preview}");
}

#[tokio::test]
async fn the_cover_pdf_has_one_page_with_both_sides_spine_from_the_page_count_and_bleed() {
    let _serial = SERIAL.lock().await;
    let server = common::open().await;
    let (work, slug) = lighthouse(&server, "Leuchtturmjahr Zwei", "zwei.typ").await;
    let pages = baruch_server::typeset::typeset(&server.state, work).await.unwrap().page_count();

    let answer = server.rpc("work.cover_pdf", json!({ "work": work, "inline": true })).await;
    let r = result(&answer);
    assert_eq!(r["file_name"], format!("{slug}-umschlag.pdf"), "Dateiname");
    assert_eq!(r["pages"], 1, "Rückseite, Rücken und Vorderseite auf einer Seite");
    // Spine: pages of the book block (without front, inside and back of the cover) × 0.1 mm.
    let block = pages - 3;
    let spine = r["spine_mm"].as_f64().unwrap();
    assert!((spine - block as f64 * 0.1).abs() < 1e-9, "Rücken {spine} mm = {block} Seiten × 0,1 mm");
    let bytes = baruch_store::decode_base64(r["pdf_base64"].as_str().unwrap()).unwrap();
    assert!(bytes.starts_with(b"%PDF-"), "gültiger PDF-Kopf");
    assert_eq!(entries(&bytes, "/Type", "/Page"), 1, "eine Seite im PDF");
    let boxes = media_boxes_mm(&bytes);
    let (w, h) = *boxes.first().unwrap_or_else(|| panic!("MediaBox im Umschlag-PDF"));
    let expected = 2.0 * 120.0 + spine + 2.0 * 3.0;
    assert!((w - expected).abs() < 0.1, "Breite {w:.2} mm = 2 × 120 + {spine:.1} + 2 × 3 = {expected:.2} mm");
    assert!((h - 166.0).abs() < 0.1, "Höhe {h:.2} mm = 160 + 2 × 3 mm Beschnitt");

    let (status, headers, download) = get_bytes(&server, &format!("/werke/{work}/umschlag.pdf")).await;
    assert_eq!(status, StatusCode::OK, "Umschlag-PDF über die Route");
    assert_eq!(
        headers[header::CONTENT_DISPOSITION],
        format!("attachment; filename=\"{slug}-umschlag.pdf\""),
        "als Anhang"
    );
    assert_eq!(entries(&download, "/Type", "/Page"), 1, "eine Seite");

    let (status, page) = server.get(&format!("/werke/{work}/umschlag")).await;
    assert_eq!(status, StatusCode::OK, "Umschlagseite lädt");
    assert!(page.contains(&format!("/werke/{work}/umschlag.pdf")), "Link zum Umschlag-PDF");
    assert!(page.contains("dpi"), "die Druckprüfung der Umschlagseite meldet das grobe Bild: {page}");
}

#[tokio::test]
async fn the_cli_writes_the_framed_book_as_pdf_with_the_pages_of_the_preview() {
    let _serial = SERIAL.lock().await;
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().join("daten");
    let (slug, pages) = {
        let state = baruch_server::AppState::open(&data).await.unwrap();
        let report =
            baruch_server::import::import_typst(&state, &book("Leuchtturmjahr Drei"), "drei.typ").await.unwrap();
        let svg = KAI.replace("#335577", "#112233");
        let new = baruch_store::NewAsset {
            file_name: "umschlag.svg",
            name: None,
            bytes: svg.as_bytes(),
            as_base64: false,
            path: None,
            caption: "",
            alt: "Dunkle Fläche",
            note: "",
        };
        state.store.add_asset(report.work, &new, "Autor").await.unwrap();
        let settings = baruch_server::view::Settings::default();
        let set = baruch_server::rpc::call(
            &state,
            &settings,
            "work.set_cover",
            json!({ "work": report.work, "name": "umschlag.svg" }),
        )
        .await;
        assert!(set.is_ok(), "Umschlag über RPC gesetzt: {set:?}");
        let pages = baruch_server::typeset::typeset(&state, report.work).await.unwrap().page_count();
        state.store.pool().close().await;
        (report.slug, pages)
    };
    assert_eq!(pages, 3 + 3, "drei Kapitel mit Vorder-, Innen- und Rückseite des Umschlags");
    let target = dir.path().join("buch.pdf");
    let out = Command::new(env!("CARGO_BIN_EXE_baruch-server"))
        .args(["export", &slug, "--pdf", target.to_str().unwrap(), "--data", data.to_str().unwrap()])
        .output()
        .expect("baruch-server startet");
    assert!(out.status.success(), "export --pdf gelingt: {}", String::from_utf8_lossy(&out.stderr));
    let written = std::fs::read(&target).unwrap();
    assert!(written.starts_with(b"%PDF-") && contains(&written, "%%EOF"), "gültiges PDF");
    assert_eq!(entries(&written, "/Type", "/Page"), pages, "die Kommandozeile setzt den Rahmen wie die Vorschau");
}

#[tokio::test]
async fn the_images_page_shows_cover_block_requests_and_search_and_the_cover_appears_everywhere() {
    let server = common::open().await;
    let report = server.import(&book("Leuchtturmjahr Vier"), "vier.typ").await;
    let work = report.work;
    let session = mcp_session(&server).await;
    mcp_tool(
        &server,
        &session,
        "asset_add",
        json!({ "work": work, "file_name": "kai.svg", "content": KAI, "encoding": "svg", "caption": "Der Kai",
                "alt": "Blaue Fläche" }),
    )
    .await;
    let added = server
        .rpc(
            "asset.add",
            json!({ "work": work, "file_name": "turm.svg", "encoding": "svg", "caption": "Der Leuchtturm",
                    "content": KAI.replace("#335577", "#221100"), "alt": "Dunkler Turm" }),
        )
        .await;
    result(&added);
    result(&server.rpc("work.set_cover", json!({ "work": work, "name": "turm.svg" })).await);
    let asked = mcp_tool(&server, &session, "asset_set_cover", json!({ "work": work, "name": "kai.svg" })).await;
    assert_eq!(asked["status"], "open", "MCP darf den Umschlag nur beantragen: {asked}");

    let (status, page) = server.get(&format!("/werke/{work}/bilder")).await;
    assert_eq!(status, StatusCode::OK, "Bilder-Seite lädt");
    let at_cover = page.find("id=\"umschlag\"").expect("Umschlag-Block (cover) auf der Bilder-Seite");
    let at_requests = page.find("id=\"antraege\"").expect("Anträge (mcp-assets) auf der Bilder-Seite");
    assert!(at_cover < at_requests, "Umschlag oben, Anträge unten");
    assert!(page.contains("<span class=\"badge primary\">Umschlag</span>"), "Abzeichen am Umschlagbild");
    assert!(page[at_requests..].contains("@mcp:claude-code"), "offener Antrag mit Herkunft: {}", &page[at_requests..]);
    assert!(page.contains("id=\"suche\""), "Suchformular");

    let (_, found) = server.get(&format!("/werke/{work}/bilder?q=LEUCHT")).await;
    let table_at = found.find("<table class=\"works-table assets-table\">").expect("Bildertabelle");
    let table = &found[table_at..table_at + found[table_at..].find("</table>").unwrap()];
    assert!(table.contains("turm.svg"), "Suche ohne Groß/Klein findet die Bildunterschrift: {table}");
    assert!(!table.contains("kai.svg"), "nicht passende Bilder fehlen in der Tabelle: {table}");
    let searched = server.rpc("asset.search", json!({ "work": work, "query": "blaue" })).await;
    let names: Vec<&str> =
        result(&searched)["assets"].as_array().unwrap().iter().filter_map(|a| a["name"].as_str()).collect();
    assert_eq!(names, ["kai.svg"], "asset.search über den Alternativtext");

    // Outline and work overview show the cover image.
    let (_, workspace) = server.get(&format!("/?work={work}")).await;
    assert!(workspace.contains("class=\"tree-cover\""), "kleines Umschlagbild im Kopf der Gliederung");
    let (_, works) = server.get("/werke").await;
    assert!(
        works.contains(&format!("/werke/{work}/bilder/turm.svg/vorschau?b=320")),
        "Werkübersicht mit Umschlag-Vorschaubild"
    );
    assert!(works.contains(&format!("/werke/{work}/pdf")), "Werkübersicht mit Link „PDF“");

    // Insert an image from the typeset view: new revision of the human, the typesetting stays free of errors.
    let nodes = result(&server.rpc("work.get", json!({ "work": work })).await)["work"]["nodes"].clone();
    let section = nodes.as_array().unwrap().iter().find(|n| n["title"] == "Flut").expect("Kapitel Flut").clone();
    let key = section["key"].as_i64().unwrap();
    let base = section["revisions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["status"] == "current")
        .and_then(|r| r["number"].as_u64())
        .unwrap();
    let (status, _) = server
        .post_form(
            "/werke/bilder/einfuegen",
            &format!("work={work}&key={key}&rev={base}&name=kai.svg&caption=Der+Kai&place=end"),
        )
        .await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Bild eingefügt");
    let preview = server.rpc("preview.typst", json!({ "work": work })).await;
    let preview = result(&preview);
    let errors = preview["diagnostics"].as_array().unwrap().iter().filter(|d| d["severity"] == "error").count();
    assert_eq!(errors, 0, "mit eingefügtem Bild setzt das Buch fehlerfrei: {preview}");
    assert!(
        baruch_server::typeset::prepare(&server.state, work)
            .await
            .unwrap()
            .source
            .main
            .contains("image(\"bilder/kai.svg\""),
        "die Abbildung steht im gesetzten Text"
    );
}
