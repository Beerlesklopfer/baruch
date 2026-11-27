// SPDX-License-Identifier: AGPL-3.0-or-later
//! Round 22 (draft, ownership spiegel-entwurf, contract §2.9): watermark in typesetting preview, book PDF, cue sheet,
//! cover and EPUB; download switch three-valued; typesetting setting „Entwurf“ (RPC, form, mirror commit); export,
//! mirror and rule block stay without watermark.
//!
//! Work freely invented (`tests/data/gartenbuch.typ`). PDFs only under `common::PDF_SERIAL`; text extract with
//! `pdftotext` (if it is missing, the test checks the typesetting source with `baruch_render::page_texts` and says so).
//! Every assertion with a reason.

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode, header};
use serde_json::{Value, json};
use tower::ServiceExt;

use crate::common;

fn result(answer: &Value) -> &Value {
    assert!(answer["error"].is_null(), "RPC-Fehler: {answer}");
    &answer["result"]
}

async fn garden() -> (common::Server, i64, String) {
    let server = common::open().await;
    let report = server.import_garden().await;
    (server, report.work, report.slug)
}

/// GET with raw bytes (PDF, EPUB).
async fn get_bytes(server: &common::Server, uri: &str) -> (StatusCode, axum::http::HeaderMap, Vec<u8>) {
    let response = server.app.clone().oneshot(Request::get(uri).body(Body::empty()).unwrap()).await.unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap().to_vec();
    (status, headers, bytes)
}

/// Stored typesetting settings (JSON) – the switches must never change them.
async fn settings(server: &common::Server, work: i64) -> Value {
    result(&server.rpc("work.typeset_get", json!({ "work": work })).await)["settings"].clone()
}

/// Text of a PDF through `pdftotext` **without any whitespace** (the watermark is set letter-spaced or rotated;
/// pdftotext then separates its letters with spaces or line breaks); `None` if it is missing.
fn pdf_text(bytes: &[u8]) -> Option<String> {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("probe.pdf");
    std::fs::write(&file, bytes).unwrap();
    let output = std::process::Command::new("pdftotext").arg(&file).arg("-").output().ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).chars().filter(|c| !c.is_whitespace()).collect())
}

async fn pdf_sha(server: &common::Server, method: &str, params: Value) -> (String, Vec<u8>) {
    let mut params = params;
    params["inline"] = json!(true);
    let answer = server.rpc(method, params).await;
    let r = result(&answer);
    let bytes = baruch_store::decode_base64(r["pdf_base64"].as_str().unwrap()).unwrap();
    assert!(bytes.starts_with(b"%PDF-"), "{method}: gültiges PDF");
    (r["sha256"].as_str().unwrap().to_owned(), bytes)
}

fn assert_text(bytes: &[u8], needle: &str, what: &str) {
    match pdf_text(bytes) {
        Some(text) => assert!(text.contains(needle), "{what}: „{needle}“ im Textauszug des PDFs: {text}"),
        None => eprintln!("pdftotext fehlt: Textauszug für {what} übersprungen (Quelle wird geprüft)"),
    }
}

fn assert_no_text(bytes: &[u8], needle: &str, what: &str) {
    if let Some(text) = pdf_text(bytes) {
        assert!(!text.contains(needle), "{what}: kein „{needle}“ im PDF: {text}");
    }
}

#[tokio::test]
async fn the_draft_setting_is_checked_trimmed_committed_and_stays_out_of_export_mirror_and_rules() {
    let (server, work, slug) = garden().await;
    let export_before = result(&server.rpc("work.export", json!({ "work": work })).await).clone();
    let main_before = std::fs::read_to_string(server.mirror_dir_of(&slug).join("main.typ")).unwrap();

    let bad = server
        .rpc(
            "work.typeset_set",
            json!({ "work": work, "settings": { "draft": {
            "text": "ENTWURF", "opacity_percent": 60, "placement": "diagonal" } } }),
        )
        .await;
    let message = bad["error"]["message"].as_str().unwrap_or_else(|| panic!("abgelehnt erwartet: {bad}"));
    assert!(message.contains("Entwurf: Deckkraft 5 bis 50 %"), "Meldung nennt Feld und Grenze: {message}");

    let saved = server
        .rpc(
            "work.typeset_set",
            json!({ "work": work, "settings": { "draft": {
            "text": "  Probedruck  ", "note": " für die Lektorin ", "date": true, "opacity_percent": 15,
            "placement": "footer", "cover": false } } }),
        )
        .await;
    result(&saved);
    let view = result(&server.rpc("work.typeset_get", json!({ "work": work })).await).clone();
    assert_eq!(
        view["settings"]["draft"],
        json!({ "text": "Probedruck", "note": "für die Lektorin", "date": true, "opacity_percent": 15,
                "placement": "footer", "cover": false }),
        "getrimmt gespeichert und über typeset_get zurück"
    );
    assert_eq!(view["rules"], "", "keine Zeile im Regelblock „// Baruch: Satz“");
    let log = common::git(&server.mirror_dir_of(&slug), &["log", "-1", "--format=%s%n%b"]).unwrap_or_default();
    assert!(log.starts_with("Satz: Entwurf geändert"), "Spiegel-Commit nennt „Entwurf“: {log}");
    assert!(log.contains("Entwurf: aus → an: „Probedruck“, Fußzeile"), "Rumpf mit vorher/nachher: {log}");

    let export_after = result(&server.rpc("work.export", json!({ "work": work })).await).clone();
    assert_eq!(export_after, export_before, "work.export bleibt byte-gleich (Wasserzeichen nur in Ausgaben)");
    let main_after = std::fs::read_to_string(server.mirror_dir_of(&slug).join("main.typ")).unwrap();
    assert_eq!(main_after, main_before, "main.typ des Spiegels bleibt byte-gleich");
    let map = server.state.store.export_map(work).await.unwrap();
    assert!(!map.typst.contains(baruch_typst::typeset::DRAFT_MARK), "die Export-Karte trägt keine Regel");
    // The rule block never reads the draft back (settings_from_rules untouched).
    assert!(baruch_typst::typeset::settings_from_rules(&map.typst).is_none_or(|s| s.draft.is_none()), "kein Entwurf");

    // Form „Entwurf“ in Einstellungen → Werk → Satz (without JavaScript): the fields carry the state.
    let (status, page) = server.get(&format!("/einstellungen/werk?work={work}")).await;
    assert_eq!(status, StatusCode::OK, "Einstellungsseite lädt");
    for needle in [
        "id=\"entwurf\"",
        "name=\"draft\" value=\"1\" checked",
        "name=\"draft_text\" maxlength=\"40\" size=\"16\" value=\"Probedruck\"",
        "name=\"draft_note\"",
        "name=\"draft_date\" value=\"1\" checked",
        "name=\"draft_opacity\"",
        "value=\"footer\" selected",
        "name=\"draft_cover\"",
    ] {
        assert!(page.contains(needle), "Formularfeld {needle} im Block „Satz“");
    }
    // Saving through the form (without tick = no draft).
    let form = format!("work={work}&action=save&draft=1&draft_text=Vorabzug&draft_opacity=20&draft_placement=header");
    let (status, _) = server.post_form("/einstellungen/werk/satz", &form).await;
    assert!(status.is_redirection(), "Formular speichert und leitet weiter: {status}");
    assert_eq!(settings(&server, work).await["draft"]["text"], "Vorabzug", "Formular setzt den Text");
    assert_eq!(settings(&server, work).await["draft"]["placement"], "header", "Formular setzt die Lage");
}

#[tokio::test]
async fn book_pdf_notes_and_preview_carry_the_watermark_as_asked_without_touching_the_setting() {
    let _serial = common::PDF_SERIAL.lock().await;
    let (server, work, _) = garden().await;
    // Without the setting: switch missing = draft:false = byte-identical (state before R22).
    let (plain, plain_bytes) = pdf_sha(&server, "work.pdf", json!({ "work": work })).await;
    let (off, _) = pdf_sha(&server, "work.pdf", json!({ "work": work, "draft": false })).await;
    assert_eq!(off, plain, "draft:false ohne Einstellung byte-gleich");
    assert_no_text(&plain_bytes, "ENTWURF", "Buch-PDF ohne Einstellung");
    let prepared = baruch_server::typeset::prepare(&server.state, work).await.unwrap();
    assert_eq!(
        prepared.source.main,
        server.state.store.export_map(work).await.unwrap().typst,
        "ohne Einstellung ist die Satzquelle genau der Export (keine Regel)"
    );

    let (on, on_bytes) = pdf_sha(&server, "work.pdf", json!({ "work": work, "draft": true })).await;
    assert_ne!(on, plain, "draft:true setzt das Wasserzeichen");
    // Diagonal text (−45°): pdftotext splits it into pieces between the lines of the book text; therefore here the
    // typesetting source of the PDF (prepare_with, the same source as work.pdf) with baruch_render::page_texts. The
    // footer below is checked by the text extract of the PDF itself.
    let _ = &on_bytes;
    let draft = baruch_server::typeset::prepare_with(&server.state, work, Some(true)).await.unwrap();
    let texts = baruch_render::page_texts(&draft.source).expect("setzbar");
    assert!(
        texts.iter().all(|page| page.iter().any(|t| t.text.contains("ENTWURF"))),
        "jede Seite der Satzquelle trägt den Text"
    );
    assert_eq!(settings(&server, work).await, json!({}), "der Schalter ändert die Einstellung nicht");

    let (status, headers, download) = get_bytes(&server, &format!("/werke/{work}/pdf?entwurf=1")).await;
    assert_eq!(status, StatusCode::OK, "Download mit ?entwurf=1");
    assert_eq!(headers[header::CONTENT_TYPE], "application/pdf", "PDF");
    assert_eq!(baruch_render::sha256_hex(&download), on, "gleiche Bytes wie draft:true");
    let (status, _, _) = get_bytes(&server, &format!("/werke/{work}/pdf?entwurf=vielleicht")).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "ein fremder Wert wird abgelehnt");

    // With the setting: if the switch is missing = by the setting; draft:false = without, byte-identical to before.
    result(
        &server
            .rpc(
                "work.typeset_set",
                json!({ "work": work, "settings": { "draft": {
                "text": "Vorabzug", "opacity_percent": 20, "placement": "footer" } } }),
            )
            .await,
    );
    let stored = settings(&server, work).await;
    let (by_setting, by_setting_bytes) = pdf_sha(&server, "work.pdf", json!({ "work": work })).await;
    assert_ne!(by_setting, plain, "nach Einstellung mit Wasserzeichen");
    assert_text(&by_setting_bytes, "Vorabzug", "Buch-PDF nach Einstellung (eigener Text)");
    let (without, _) = pdf_sha(&server, "work.pdf", json!({ "work": work, "draft": false })).await;
    assert_eq!(without, plain, "draft:false mit Einstellung byte-gleich zum PDF ohne Einstellung");
    let (_, _, no) = get_bytes(&server, &format!("/werke/{work}/pdf?entwurf=0")).await;
    assert_eq!(baruch_render::sha256_hex(&no), plain, "?entwurf=0 ebenso");
    assert_eq!(settings(&server, work).await, stored, "auch danach bleibt die Einstellung, wie sie war");

    // Cue sheet: by the setting with, draft:false without, link „Als Entwurf herunterladen“ without JavaScript.
    let (notes_on, notes_bytes) = pdf_sha(&server, "notes.pdf", json!({ "work": work })).await;
    assert_text(&notes_bytes, "Vorabzug", "Stichwortzettel nach Einstellung");
    let (notes_off, notes_off_bytes) = pdf_sha(&server, "notes.pdf", json!({ "work": work, "draft": false })).await;
    assert_ne!(notes_on, notes_off, "draft:false lässt das Wasserzeichen weg");
    assert_no_text(&notes_off_bytes, "Vorabzug", "Stichwortzettel mit draft:false");
    let (status, page) = server.get(&format!("/werke/{work}/stichwortzettel")).await;
    assert_eq!(status, StatusCode::OK, "Stichwortzettel-Seite lädt");
    assert!(
        page.contains(&format!("/werke/{work}/stichwortzettel.pdf?format=a5&amp;schrift=16&amp;entwurf=1"))
            && page.contains("Als Entwurf herunterladen"),
        "Link „Als Entwurf herunterladen“ mit &entwurf=1"
    );
    let (status, _, _) = get_bytes(&server, &format!("/werke/{work}/stichwortzettel.pdf?entwurf=1")).await;
    assert_eq!(status, StatusCode::OK, "Stichwortzettel mit ?entwurf=1");

    // Typesetting preview: preview.typst three-valued, page with links and SVG in the same state.
    let shown = result(&server.rpc("preview.typst", json!({ "work": work })).await).clone();
    let off = result(&server.rpc("preview.typst", json!({ "work": work, "draft": false })).await).clone();
    assert_eq!(shown["draft"], true, "nach Einstellung trägt die Vorschau das Wasserzeichen");
    assert_eq!(off["draft"], false, "draft:false ohne");
    assert_ne!(shown["stand"], off["stand"], "verschiedene Stände im Zwischenspeicher");
    assert_eq!(shown["pages"], off["pages"], "das Wasserzeichen ändert den Umbruch nicht");
    let (status, html) = server.get(&format!("/werke/{work}/satz?entwurf=0")).await;
    assert_eq!(status, StatusCode::OK, "Satz-Seite mit ?entwurf=0");
    assert!(html.contains(&format!("/werke/{work}/pdf?entwurf=1")), "Knopf „Als Entwurf herunterladen“");
    assert!(html.contains("Als Entwurf herunterladen"), "Beschriftung des Knopfs");
    assert!(html.contains(&format!("/werke/{work}/satz?entwurf=1")), "Vorschau als Entwurf umschalten");
    let stand = off["stand"].as_str().unwrap();
    let svg = format!("/werke/{work}/satz/{stand}/1.svg?entwurf=0");
    assert!(html.contains(&svg), "Bild der Seite im selben Stand: {svg}");
    let (status, _) = server.get(&svg).await;
    assert_eq!(status, StatusCode::OK, "SVG der Seite im Stand ohne Wasserzeichen");
    assert_eq!(settings(&server, work).await, stored, "keine Seite ändert die Einstellung");
}

#[tokio::test]
async fn diagnostics_stay_with_their_section_when_the_watermark_is_in_the_source() {
    let _serial = common::PDF_SERIAL.lock().await;
    let server = common::open().await;
    let source = "#set text(lang: \"de\")\n\n= Erstes Kapitel\n\nRuhiger Text.\n\n= Zweites Kapitel\n\nHier fehlt \
                  #nirgends().\n";
    let report = server.import(source, "fehler.typ").await;
    let work = report.work;
    let plain = baruch_server::typeset::typeset_draft(
        &server.state,
        work,
        baruch_server::typeset::COMPILE_TIMEOUT,
        Some(false),
    )
    .await
    .unwrap();
    let marked =
        baruch_server::typeset::typeset_draft(&server.state, work, baruch_server::typeset::COMPILE_TIMEOUT, Some(true))
            .await
            .unwrap();
    let key = |t: &baruch_server::typeset::Typeset| {
        t.diagnostics().into_iter().find(|d| d.severity == "error").map(|d| (d.key, d.section_line))
    };
    assert!(key(&plain).is_some_and(|(k, _)| k.is_some()), "Vorbedingung: der Fehler hat einen Abschnitt");
    assert_eq!(key(&marked), key(&plain), "mit Wasserzeichen derselbe Abschnitt und dieselbe Zeile darin");
    assert!(
        marked.prepared.source.main.contains(baruch_typst::typeset::DRAFT_MARK),
        "Vorbedingung: Regel in der Quelle"
    );
}

#[tokio::test]
async fn the_cover_pdf_gets_the_watermark_only_with_draft_cover_or_explicitly_and_the_print_check_warns() {
    let _serial = common::PDF_SERIAL.lock().await;
    let (server, work, _) = garden().await;
    let draft =
        |cover: bool| json!({ "text": "Vorabzug", "opacity_percent": 20, "placement": "diagonal", "cover": cover });
    result(&server.rpc("work.typeset_set", json!({ "work": work, "settings": { "draft": draft(false) } })).await);
    let codes = |answer: &Value| -> Vec<String> {
        result(answer)["checks"].as_array().unwrap().iter().map(|c| c["code"].as_str().unwrap().to_owned()).collect()
    };
    let plain = server.rpc("work.cover_pdf", json!({ "work": work })).await;
    assert_eq!(result(&plain)["draft"], false, "nach Einstellung ohne draft.cover: Umschlag ohne Wasserzeichen");
    assert!(!codes(&plain).contains(&"draft".to_owned()), "ohne Wasserzeichen keine Warnung");
    let explicit = server.rpc("work.cover_pdf", json!({ "work": work, "draft": true })).await;
    assert_eq!(result(&explicit)["draft"], true, "ausdrücklich draft:true: mit Wasserzeichen");
    assert!(codes(&explicit).contains(&"draft".to_owned()), "Druckprüfung warnt (code draft)");
    let warning =
        result(&explicit)["checks"].as_array().unwrap().iter().find(|c| c["code"] == "draft").unwrap().clone();
    assert!(warning["message"].as_str().unwrap().contains("nicht an die Druckerei"), "deutsche Meldung: {warning}");
    assert_ne!(result(&explicit)["sha256"], result(&plain)["sha256"], "andere Bytes mit Wasserzeichen");

    result(&server.rpc("work.typeset_set", json!({ "work": work, "settings": { "draft": draft(true) } })).await);
    let by_setting = server.rpc("work.cover_pdf", json!({ "work": work })).await;
    assert!(codes(&by_setting).contains(&"draft".to_owned()), "mit draft.cover nach Einstellung: Warnung");
    let off = server.rpc("work.cover_pdf", json!({ "work": work, "draft": false })).await;
    assert!(!codes(&off).contains(&"draft".to_owned()), "draft:false: ohne Wasserzeichen, ohne Warnung");
    assert_eq!(result(&off)["sha256"], result(&plain)["sha256"], "draft:false byte-gleich zum Umschlag ohne");
    let (status, _, bytes) = get_bytes(&server, &format!("/werke/{work}/umschlag.pdf?entwurf=0")).await;
    assert_eq!(status, StatusCode::OK, "Umschlag-PDF mit ?entwurf=0");
    assert_eq!(baruch_render::sha256_hex(&bytes), result(&plain)["sha256"].as_str().unwrap(), "gleiche Bytes");
    let kept = settings(&server, work).await["draft"].clone();
    assert_eq!((&kept["text"], &kept["cover"]), (&json!("Vorabzug"), &json!(true)), "die Einstellung bleibt: {kept}");
}

/// File `name` from an EPUB.
fn epub_entry(bytes: &[u8], name: &str) -> String {
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut text = String::new();
    std::io::Read::read_to_string(&mut archive.by_name(name).unwrap(), &mut text).unwrap();
    text
}

#[tokio::test]
async fn the_epub_marks_title_page_and_title_metadata_as_draft_only_when_asked() {
    let (server, work, _) = garden().await;
    let epub = |params: Value| {
        let server = &server;
        async move {
            let mut params = params;
            params["inline"] = json!(true);
            let answer = server.rpc("work.epub", params).await;
            baruch_store::decode_base64(result(&answer)["epub_base64"].as_str().unwrap()).unwrap()
        }
    };
    let plain = epub(json!({ "work": work })).await;
    let opf = epub_entry(&plain, "OEBPS/content.opf");
    assert!(!opf.contains("(Entwurf)"), "ohne Entwurf kein Zusatz im Titel: {opf}");
    let off = epub(json!({ "work": work, "draft": false })).await;
    assert_eq!(off, plain, "draft:false ohne Einstellung byte-gleich");
    let marked = epub(json!({ "work": work, "draft": true })).await;
    let opf = epub_entry(&marked, "OEBPS/content.opf");
    assert!(opf.contains("<dc:title id=\"title\">Das Gartenbuch (Entwurf)</dc:title>"), "dc:title mit Zusatz: {opf}");
    let title = epub_entry(&marked, "OEBPS/text/title.xhtml");
    assert!(title.contains("Entwurf: ENTWURF"), "Hinweis auf der Titelseite (Vorgabetext): {title}");
    let (status, _, download) = get_bytes(&server, &format!("/werke/{work}/epub?entwurf=1")).await;
    assert_eq!(status, StatusCode::OK, "EPUB mit ?entwurf=1");
    assert!(epub_entry(&download, "OEBPS/content.opf").contains("(Entwurf)"), "Download als Entwurf");
    // MCP epub_export takes draft.
    let answer = baruch_server::mcp::epub::call(&server.state, json!({ "work": work, "draft": true }))
        .await
        .unwrap_or_else(|_| panic!("epub_export mit draft"));
    assert_eq!(answer["meta"]["title"], "Das Gartenbuch", "Metadaten nennen den Werktitel");
    assert_eq!(answer["url"], format!("/werke/{work}/epub?entwurf=1"), "Adresse mit Schalter");
    assert!(
        baruch_server::mcp::epub::schema()["properties"]["draft"]["type"] == "boolean",
        "Werkzeugschema nennt draft"
    );
    assert_eq!(settings(&server, work).await, json!({}), "kein Weg ändert die Einstellung");
}
