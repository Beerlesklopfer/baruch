// SPDX-License-Identifier: AGPL-3.0-or-later
//! Round 27, acceptance by the integrator: cover editor and imprint in the work template across all three tasks
//! (umschlag-daten, umschlag-satz, umschlag-widget).
//!
//! A freely invented work („Das Leuchtturmjahr“, 148 × 210 mm) with a PNG cover generated in the test (dark upper
//! third) and complete invented book details. Checked are: the same position in preview, cover PDF and book PDF
//! (measured with `pdftotext -bbox`), the same typesetting source (`baruch_render::stand`) of preview and cover PDF,
//! the print check (bleed, barcode, contrast, font), form without JavaScript equal to RPC, quick choice and reset down
//! to the byte-identical typesetting, imprint from the work template including the button, MCP read-only or as a
//! request, and the exchange between two throwaway instances. No network, no model, no Eliezer.

use crate::common;
use crate::round27_satz::dark_top_png;

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode, header};
use baruch_store::{CoverBlock, CoverBlockKind, CoverFace, CoverLayout};
use serde_json::{Value, json};
use std::net::SocketAddr;
use std::process::Command;

const MM_PER_PT: f64 = 25.4 / 72.0;

/// The invented work: work template with page size, two chapters.
const LIGHTHOUSE: &str = "// Leuchtturmbuch für die Abnahme der Runde 27 (frei erfunden).
#set document(title: \"Das Leuchtturmjahr\", author: \"Hanna Wellen\")
#set page(width: 148mm, height: 210mm)
#set text(lang: \"de\")

= Winter

Die Lampe dreht sich über dem Eis, und die Wärterin zählt die Schiffe.

= Sommer

Die Möwen schreien über der Mole.
";

/// Publisher with Typst special characters: must appear verbatim everywhere.
const PUBLISHER: &str = "Verlag \"Mole\" #1 [Hafen] \\ Süd";
/// Own text of the free-text block with Typst special characters.
const FREE_TEXT: &str = "Erstausgabe #7 [Mole] \"frei\" \\ Ende";

fn result(answer: &Value) -> &Value {
    assert!(answer["error"].is_null(), "RPC-Fehler: {answer}");
    &answer["result"]
}

/// New throwaway data directory with the lighthouse book, PNG cover and complete book details; work id and slug.
async fn lighthouse() -> (common::Server, i64, String) {
    let server = common::open().await;
    let report = server.import(LIGHTHOUSE, "leuchtturm.typ").await;
    let work = report.work;
    let added = server
        .rpc(
            "asset.add",
            json!({ "work": work, "file_name": "leuchtturm.png",
                    "content": baruch_store::encode_base64(&dark_top_png(60, 90)),
                    "encoding": "base64", "caption": "Erfundener Himmel", "alt": "Himmel über dem Meer" }),
        )
        .await;
    let name = result(&added)["name"].as_str().unwrap().to_owned();
    result(&server.rpc("work.set_cover", json!({ "work": work, "name": name })).await);
    let set = server
        .rpc(
            "work.meta.set",
            json!({ "work": work, "front_matter": true, "cover_pages": true, "subtitle": "Ein Jahr am Meer",
                    "publisher": PUBLISHER, "publisher_address": "Molenweg 3\n99999 Hafenstadt",
                    "place": "Hafenstadt", "year": "2026", "edition": "1. Auflage", "isbn": "978-3-16-148410-0",
                    "price": "14,00 €", "series": "Küstenreihe", "printer": "Druckerei Gischt",
                    "printer_address": "Gischtstraße 5, 99998 Dünen", "bible": "Erfundene Übersetzung",
                    "bible_notice": "Bibelworte nach einer erfundenen Übersetzung.",
                    "blurb": "Brandung, Möwen und ein Jahr am Leuchtturm.", "bio": "Hanna Wellen hütet Lampen.",
                    "dedication": "Für alle Wärterinnen." }),
        )
        .await;
    result(&set);
    (server, work, report.slug)
}

async fn layout(server: &common::Server, work: i64) -> Value {
    result(&server.rpc("cover.layout", json!({ "work": work })).await).clone()
}

/// Effective block of the kind from `cover.layout` as `CoverBlock` (without `stored` and `height_mm`).
fn block_in(view: &Value, kind: CoverBlockKind) -> CoverBlock {
    let mut value = view["blocks"].as_array().unwrap().iter().find(|b| b["kind"] == kind.as_str()).unwrap().clone();
    let object = value.as_object_mut().unwrap();
    object.remove("stored");
    object.remove("height_mm");
    serde_json::from_value(value).unwrap()
}

/// The changes of the acceptance as form fields `b.<kind>.<feld>`: title into the upper (dark) third, author to the
/// bottom, blurb moved and narrower, free text on the front.
fn moved_fields(blurb_width: f64) -> Vec<(String, String)> {
    let mut fields = Vec::new();
    let mut put = |kind: &str, field: &str, value: &str| fields.push((format!("b.{kind}.{field}"), value.to_owned()));
    for (field, value) in
        [("x_mm", "14"), ("y_mm", "18"), ("width_mm", "120"), ("size_pt", "26"), ("color", "light"), ("align", "left")]
    {
        put("front_title", field, value);
    }
    for (field, value) in [("x_mm", "14"), ("y_mm", "175"), ("width_mm", "120"), ("color", "dark"), ("align", "left")] {
        put("front_author", field, value);
    }
    let width = format!("{blurb_width}");
    for (field, value) in [("x_mm", "24"), ("y_mm", "36"), ("width_mm", width.as_str()), ("color", "light")] {
        put("back_blurb", field, value);
    }
    for (field, value) in [
        ("x_mm", "14"),
        ("y_mm", "120"),
        ("width_mm", "120"),
        ("size_pt", "12"),
        ("color", "dark"),
        ("visible", "1"),
        ("text", FREE_TEXT),
        ("align", "left"),
    ] {
        put("front_free", field, value);
    }
    fields
}

/// Form or query encoding.
fn encode(pairs: &[(String, String)]) -> String {
    let escape = |text: &str| {
        let mut out = String::new();
        for b in text.bytes() {
            match b {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
                _ => out.push_str(&format!("%{b:02X}")),
            }
        }
        out
    };
    pairs.iter().map(|(k, v)| format!("{}={}", escape(k), escape(v))).collect::<Vec<_>>().join("&")
}

fn pairs(list: &[(&str, &str)]) -> Vec<(String, String)> {
    list.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

async fn post(server: &common::Server, uri: &str, form: &str) -> (StatusCode, Option<String>, String) {
    let request = Request::post(uri)
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(form.to_owned()))
        .unwrap();
    let (status, headers, text) = server.send(request).await;
    let location = headers.get(header::LOCATION).map(|v| v.to_str().unwrap().to_owned());
    (status, location, text)
}

/// Cover PDF through `work.cover_pdf {inline: true}`; answer together with the bytes.
async fn cover_pdf(server: &common::Server, work: i64) -> (Value, Vec<u8>) {
    let answer = {
        let _serial = common::PDF_SERIAL.lock().await;
        server.rpc("work.cover_pdf", json!({ "work": work, "inline": true })).await
    };
    let answer = result(&answer).clone();
    let bytes = baruch_store::decode_base64(answer["pdf_base64"].as_str().unwrap()).unwrap();
    (answer, bytes)
}

async fn book_pdf(server: &common::Server, work: i64) -> baruch_server::pdf::PdfOut {
    let _serial = common::PDF_SERIAL.lock().await;
    baruch_server::pdf::book_pdf(&server.state, work).await.expect("Buch-PDF")
}

/// One word from `pdftotext -bbox`: page (1-based), upper left corner in pt, text.
#[derive(Debug, Clone)]
struct Word {
    page: usize,
    x_min: f64,
    y_min: f64,
    text: String,
}

/// Words of a PDF with position; `None` if `pdftotext` is missing.
fn words(pdf: &[u8]) -> Option<Vec<Word>> {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("probe.pdf");
    std::fs::write(&path, pdf).unwrap();
    let out = Command::new("pdftotext").arg("-bbox").arg(&path).arg("-").output().ok()?;
    assert!(out.status.success(), "pdftotext scheitert: {}", String::from_utf8_lossy(&out.stderr));
    let html = String::from_utf8(out.stdout).unwrap();
    let mut page = 0;
    let mut list = Vec::new();
    for line in html.lines() {
        let line = line.trim();
        if line.starts_with("<page ") {
            page += 1;
        }
        let Some(rest) = line.strip_prefix("<word ") else { continue };
        let num = |name: &str| -> f64 {
            let at = rest.find(&format!("{name}=\"")).unwrap() + name.len() + 2;
            rest[at..at + rest[at..].find('"').unwrap()].parse().unwrap()
        };
        let text = rest[rest.find('>').unwrap() + 1..rest.find("</word>").unwrap()]
            .replace("&quot;", "\"")
            .replace("&apos;", "'")
            .replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&amp;", "&");
        list.push(Word { page, x_min: num("xMin"), y_min: num("yMin"), text });
    }
    Some(list)
}

fn find<'a>(list: &'a [Word], text: &str, page: usize) -> &'a Word {
    list.iter()
        .find(|w| w.text == text && w.page == page)
        .unwrap_or_else(|| panic!("Wort {text:?} auf Seite {page} fehlt: {list:?}"))
}

/// Text of one page (`pdftotext -f n -l n`); `None` if `pdftotext` is missing.
fn page_text(pdf: &[u8], page: usize) -> Option<String> {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("seite.pdf");
    std::fs::write(&path, pdf).unwrap();
    let page = page.to_string();
    let out = Command::new("pdftotext").args(["-f", &page, "-l", &page]).arg(&path).arg("-").output().ok()?;
    Some(String::from_utf8_lossy(&out.stdout).into_owned())
}

fn last_commit(server: &common::Server, slug: &str) -> String {
    common::git(&server.mirror_dir_of(slug), &["log", "-1", "--format=%B"]).unwrap_or_default()
}

fn commit_count(server: &common::Server, slug: &str) -> u32 {
    common::git(&server.mirror_dir_of(slug), &["rev-list", "--count", "HEAD"]).map_or(0, |c| c.trim().parse().unwrap())
}

/// Title into the upper third, author to the bottom, blurb moved and narrower, free text: the preview without
/// JavaScript (GET with `b.*` fields) saves nothing; the form saves the same as `cover.layout_set` on a second
/// instance; preview SVG and typesetting source of the preview equal the saved cover; the position in cover PDF and
/// book PDF is the same relative to the trim edge (within 0.5 mm).
#[tokio::test]
async fn the_moved_layout_lies_at_the_same_place_in_preview_cover_pdf_and_book_pdf_with_and_without_javascript() {
    let (form_side, work, slug) = lighthouse().await;
    let (rpc_side, work_rpc, _) = lighthouse().await;
    let view = layout(&form_side, work).await;
    assert_eq!(view["stored"], false, "frisches Werk ohne eigene Anordnung");
    let geometry: baruch_store::CoverGeometry = serde_json::from_value(view["geometry"].clone()).unwrap();
    assert!(
        (geometry.page_width_mm - 148.0).abs() < 0.01 && (geometry.page_height_mm - 210.0).abs() < 0.01,
        "Seitenmaß aus der Werkvorlage: {geometry:?}"
    );
    let old_blurb = block_in(&view, CoverBlockKind::BackBlurb);
    let blurb_width = (old_blurb.width_mm - 40.0).round();
    assert!(blurb_width >= 40.0, "Klappentext wird schmaler, bleibt aber lesbar: {old_blurb:?}");
    let fields = moved_fields(blurb_width);

    // Preview without JavaScript: page and SVG with the fields, nothing saved.
    let query = encode(&fields);
    let (status, page) = form_side.get(&format!("/werke/{work}/umschlag?{query}")).await;
    assert_eq!(status, StatusCode::OK, "Vorschau-Seite: {page}");
    assert!(page.contains("aria-label=\"Titel (Vorderseite): x 14"), "Rahmen des Titels an der neuen Stelle: {page}");
    let (status, preview_svg) = form_side.get(&format!("/werke/{work}/umschlag.svg?{query}")).await;
    assert_eq!(status, StatusCode::OK, "Vorschau-SVG");
    assert!(form_side.state.store.cover_layout(work).await.unwrap().is_empty(), "Vorschau speichert nichts");
    let base: Vec<CoverBlock> = CoverBlockKind::ALL.iter().map(|k| block_in(&view, *k)).collect();
    let preview = baruch_server::cover::blocks_from_fields(&fields, &base).unwrap();
    assert_eq!(preview.len(), 4, "vier geänderte Blöcke");
    let preview_wrap =
        baruch_server::cover::build_wrap_for(&form_side.state, work, Some(&CoverLayout { blocks: preview.clone() }))
            .await
            .unwrap();
    let preview_stand = baruch_render::stand(&preview_wrap.source);

    // Save without JavaScript (form) …
    let commits = commit_count(&form_side, &slug);
    let mut form =
        pairs(&[("work", &work.to_string()), ("action", "save"), ("back", &format!("/werke/{work}/umschlag"))]);
    form.extend(fields.clone());
    let (status, location, text) = post(&form_side, "/werke/umschlag/anordnung", &encode(&form)).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Formular speichert: {text}");
    assert_eq!(location.as_deref(), Some(format!("/werke/{work}/umschlag?ok=anordnung").as_str()), "Rücksprung");
    assert_eq!(commit_count(&form_side, &slug), commits + 1, "genau ein Commit");
    let message = last_commit(&form_side, &slug);
    assert!(message.starts_with("Umschlag: Anordnung geändert"), "Betreff: {message}");
    assert!(message.contains("Titel (Vorderseite): x "), "Rumpf mit vorher → nachher: {message}");
    // … and with JavaScript (the widget sends cover.layout_set) on the second instance.
    result(&rpc_side.rpc("cover.layout_set", json!({ "work": work_rpc, "blocks": preview })).await);
    let stored = form_side.state.store.cover_layout(work).await.unwrap();
    assert_eq!(stored.blocks.len(), 4, "vier Zeilen gespeichert");
    assert_eq!(
        stored,
        rpc_side.state.store.cover_layout(work_rpc).await.unwrap(),
        "Formular und RPC speichern dieselbe Anordnung"
    );

    // Same source: preview, saved cover (cover PDF) and the second instance.
    let cover_source = baruch_server::cover::cover_source(&form_side.state, work).await.unwrap();
    assert_eq!(baruch_render::stand(&cover_source), preview_stand, "Vorschau und Umschlag-PDF setzen dieselbe Quelle");
    let other = baruch_server::cover::cover_source(&rpc_side.state, work_rpc).await.unwrap();
    assert_eq!(baruch_render::stand(&other), preview_stand, "mit und ohne JavaScript dieselbe Quelle");
    let (_, saved_svg) = form_side.get(&format!("/werke/{work}/umschlag.svg")).await;
    assert_eq!(saved_svg, preview_svg, "Vorschau-SVG vor dem Speichern = SVG der gespeicherten Anordnung");

    // Measure the position: cover PDF and book PDF (front = page 1, back = last page).
    let (answer, cover_bytes) = cover_pdf(&form_side, work).await;
    let checks = answer["checks"].as_array().unwrap();
    for kind in ["front_title", "front_author", "back_blurb", "front_free"] {
        assert!(
            !checks.iter().any(|c| c["block"] == kind),
            "die gut gesetzten Blöcke warnen nicht ({kind}): {checks:?}"
        );
    }
    assert!(
        !checks.iter().any(|c| c["code"].as_str().unwrap().starts_with("imprint_")),
        "alle Pflichtangaben da: {checks:?}"
    );
    let book = book_pdf(&form_side, work).await;
    let (Some(cover_words), Some(book_words)) = (words(&cover_bytes), words(&book.bytes)) else {
        eprintln!("ÜBERSPRUNGEN: pdftotext fehlt (Lage nicht gemessen)");
        return;
    };
    for (word, face, book_page) in [
        ("Leuchtturmjahr", CoverFace::Front, 1),
        ("Erstausgabe", CoverFace::Front, 1),
        ("Brandung,", CoverFace::Back, book.pages),
    ] {
        let in_book = find(&book_words, word, book_page);
        let in_cover = find(&cover_words, word, 1);
        let (ox, oy) = geometry.face_origin(face);
        let dx = in_book.x_min * MM_PER_PT - (in_cover.x_min * MM_PER_PT - ox);
        let dy = in_book.y_min * MM_PER_PT - (in_cover.y_min * MM_PER_PT - oy);
        assert!(
            dx.abs() <= 0.5 && dy.abs() <= 0.5,
            "{word}: gleiche Lage relativ zur Schnittkante in Buch und Umschlag, Abweichung {dx:.3} / {dy:.3} mm \
             ({in_book:?} / {in_cover:?})"
        );
    }
    // And the position follows the layout (side bearing of the glyph and cap height as tolerance).
    let title = find(&book_words, "Das", 1);
    assert!(
        (title.x_min * MM_PER_PT - 14.0).abs() < 1.5 && (title.y_min * MM_PER_PT - 18.0).abs() < 3.0,
        "Titel bei x 14 mm, y 18 mm im oberen Drittel: {title:?}"
    );
    let author = find(&book_words, "Wellen", 1);
    assert!(author.y_min * MM_PER_PT > 170.0, "Autor unten (y 175 mm): {author:?}");
    let blurb = find(&book_words, "Brandung,", book.pages);
    assert!((blurb.x_min * MM_PER_PT - 24.0).abs() < 1.5, "Klappentext bei x 24 mm: {blurb:?}");
    // The free text stands there verbatim, special characters included.
    let front: Vec<&str> = cover_words.iter().map(|w| w.text.as_str()).collect();
    let joined = front.join(" ");
    assert!(joined.contains(FREE_TEXT), "Freitext wörtlich im Umschlag-PDF: {joined}");
    assert!(joined.contains(PUBLISHER), "Verlag mit Sonderzeichen wörtlich auf dem Umschlag: {joined}");
}

/// Print check through the preview without JavaScript: a block in the bleed, blurb above the barcode, light author on a
/// light background and a missing font are warned about; in the typesetting the font falls back to the default.
#[tokio::test]
async fn bleed_barcode_contrast_and_a_missing_font_are_warned_and_the_font_falls_back() {
    let (server, work, _) = lighthouse().await;
    let view = layout(&server, work).await;
    let barcode = block_in(&view, CoverBlockKind::BackBarcode);
    assert!(barcode.visible, "mit ISBN ist der Strichcode sichtbar: {barcode:?}");
    let fields = vec![
        ("b.front_title.x_mm".to_owned(), "-2".to_owned()),
        ("b.front_title.font".to_owned(), "Nirgendwo Antiqua".to_owned()),
        ("b.front_title.color".to_owned(), "light".to_owned()),
        ("b.front_author.y_mm".to_owned(), "175".to_owned()),
        ("b.front_author.color".to_owned(), "light".to_owned()),
        ("b.back_blurb.x_mm".to_owned(), format!("{}", barcode.x_mm)),
        ("b.back_blurb.y_mm".to_owned(), format!("{}", barcode.y_mm)),
        ("b.back_blurb.width_mm".to_owned(), "30".to_owned()),
    ];
    let (status, page) = server.get(&format!("/werke/{work}/umschlag?{}", encode(&fields))).await;
    assert_eq!(status, StatusCode::OK, "Vorschau-Seite: {page}");
    for (code, kind, why) in [
        ("bleed", "front_title", "x = −2 mm ragt in den Beschnitt"),
        ("font", "front_title", "die erfundene Schrift gibt es nicht"),
        ("contrast", "front_author", "heller Text auf dem hellgelben unteren Teil"),
        ("barcode", "back_blurb", "der Klappentext liegt auf dem Strichcode"),
    ] {
        assert!(
            page.contains(&format!("data-cover-warning=\"{code}\" data-block=\"{kind}\"")),
            "{why}: Warnung {code} bei {kind} auf der Seite erwartet"
        );
    }
    assert!(page.contains("Nirgendwo Antiqua"), "die Meldung nennt die fehlende Schrift");
    assert!(
        !page.contains("Titel (Vorderseite): Titel (Vorderseite)"),
        "die Meldung nennt den Block nur einmal (Sichtprüfung R27)"
    );
    assert!(server.state.store.cover_layout(work).await.unwrap().is_empty(), "die Vorschau speichert nichts");

    // Typesetting: the missing font is not requested at all (fallback), the cover typesets without error.
    let base: Vec<CoverBlock> = CoverBlockKind::ALL.iter().map(|k| block_in(&view, *k)).collect();
    let blocks = baruch_server::cover::blocks_from_fields(&fields, &base).unwrap();
    let wrap = baruch_server::cover::build_wrap_for(&server.state, work, Some(&CoverLayout { blocks })).await.unwrap();
    assert!(!wrap.source.main.contains("Nirgendwo Antiqua"), "keine unbekannte Schrift im Satz");
    let rendered = baruch_render::compile(&wrap.source);
    assert!(!rendered.has_errors(), "Umschlag mit Rückfall setzt fehlerfrei: {:?}", rendered.diagnostics);

    // The same layout saved: work.cover_pdf names the same warnings.
    let mut form = pairs(&[("work", &work.to_string()), ("action", "save")]);
    form.extend(fields);
    let (status, _, text) = post(&server, "/werke/umschlag/anordnung", &encode(&form)).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "gespeichert, auch mit Warnungen: {text}");
    let (answer, _) = cover_pdf(&server, work).await;
    let codes: Vec<(String, String)> = answer["checks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| (c["code"].as_str().unwrap().to_owned(), c["block"].as_str().unwrap_or("").to_owned()))
        .collect();
    for (code, kind) in
        [("bleed", "front_title"), ("font", "front_title"), ("contrast", "front_author"), ("barcode", "back_blurb")]
    {
        assert!(
            codes.iter().any(|(c, k)| c == code && k == kind),
            "{code}/{kind} auch in work.cover_pdf (gespeicherte Anordnung): {codes:?}"
        );
    }
}

/// Quick choice top/middle/bottom and reset through the form; after the reset book and cover typeset byte-identically
/// as before the first layout (without layout, without template functions).
#[tokio::test]
async fn quick_choices_and_reset_return_to_a_byte_identical_typesetting() {
    let (server, work, slug) = lighthouse().await;
    let before_book = baruch_server::typeset::prepare(&server.state, work).await.unwrap();
    assert!(!before_book.source.main.contains("baruch_buch"), "importiertes Werk ohne Vorlagenfunktionen");
    let before_cover = baruch_render::stand(&baruch_server::cover::cover_source(&server.state, work).await.unwrap());
    assert_eq!(
        server.state.store.book_meta(work).await.unwrap().title_area,
        baruch_store::TitleArea::Top,
        "Ausgangslage: Titelbereich oben, damit die Schnellwahl Mitte/unten/oben ihn sichtbar umschaltet und zurückkehrt"
    );

    let mut form = pairs(&[("work", &work.to_string()), ("action", "save")]);
    form.extend(moved_fields(60.0));
    assert_eq!(post(&server, "/werke/umschlag/anordnung", &encode(&form)).await.0, StatusCode::SEE_OTHER);
    let moved = baruch_server::typeset::prepare(&server.state, work).await.unwrap();
    assert_ne!(moved.stand, before_book.stand, "mit Anordnung ändert sich die Umschlagseite im Buch");

    for (area, word, expected) in [
        ("middle", "Mitte", baruch_store::TitleArea::Middle),
        ("bottom", "unten", baruch_store::TitleArea::Bottom),
        ("top", "oben", baruch_store::TitleArea::Top),
    ] {
        let (status, _, text) =
            post(&server, "/werke/umschlag/anordnung", &format!("work={work}&action=quick&area={area}")).await;
        assert_eq!(status, StatusCode::SEE_OTHER, "Schnellwahl {area}: {text}");
        assert_eq!(server.state.store.book_meta(work).await.unwrap().title_area, expected, "Titelbereich {area}");
        let stored = server.state.store.cover_layout(work).await.unwrap();
        assert!(stored.blocks.iter().all(|b| b.kind.face() != CoverFace::Front), "Vorderseite zurückgesetzt");
        assert!(stored.get(CoverBlockKind::BackBlurb).is_some(), "die Rückseite bleibt bei der Schnellwahl");
        let message = last_commit(&server, &slug);
        assert!(message.starts_with(&format!("Umschlag: Schnellwahl {word}")), "Betreff: {message}");
    }
    let (status, _, _) = post(&server, "/werke/umschlag/anordnung", &format!("work={work}&action=reset")).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Anordnung zurücksetzen");
    assert!(server.state.store.cover_layout(work).await.unwrap().is_empty(), "keine Zeile mehr");
    assert!(last_commit(&server, &slug).starts_with("Umschlag: Anordnung zurückgesetzt"), "Betreff Zurücksetzen");
    let tombstones: i64 = sqlx::query_scalar("SELECT count(*) FROM uuid_tombstone WHERE kind = 'cover_block'")
        .fetch_one(server.state.store.pool())
        .await
        .unwrap();
    assert_eq!(tombstones, 4, "je gelöschtem Block ein Grabstein (vier Blöcke, je einmal gelöscht)");

    let after_book = baruch_server::typeset::prepare(&server.state, work).await.unwrap();
    assert_eq!(after_book.stand, before_book.stand, "ohne Anordnung setzt das Buch byte-gleich wie vorher");
    assert_eq!(after_book.source.main, before_book.source.main, "Satzquelle byte-gleich");
    let after_cover = baruch_render::stand(&baruch_server::cover::cover_source(&server.state, work).await.unwrap());
    assert_eq!(after_cover, before_cover, "Umschlag byte-gleich wie vor der ersten Anordnung");
}

/// Imprint from the work template: without the function the built-in page; an own `impressum(buch)` typesets page 4
/// with the book details verbatim; the button adds the remaining functions as a new template revision with a commit and
/// honestly refuses afterwards; the print check names missing mandatory details.
#[tokio::test]
async fn the_imprint_comes_from_the_template_and_the_button_adopts_the_rest() {
    let (server, work, slug) = lighthouse().await;
    let builtin = baruch_server::typeset::prepare(&server.state, work).await.unwrap();
    assert!(!builtin.source.main.contains("baruch_buch"), "ohne Funktion bleibt die eingebaute Titelei");

    let template = server.state.store.work_template(work).await.unwrap();
    let own = "#let impressum(buch) = [Impressum der Werkvorlage: #buch.verlag (#buch.jahr) bei #buch.druck]\n";
    result(&server.rpc("work.update", json!({ "work": work, "template": format!("{template}{own}") })).await);
    let rev = server.state.store.work_meta(work).await.unwrap().template_rev;
    let commits = commit_count(&server, &slug);
    let back = format!("/werke/{work}/umschlag");
    let form = encode(&pairs(&[("work", &work.to_string()), ("back", &back)]));
    let (status, location, text) = post(&server, "/werke/umschlag/impressum", &form).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Knopf übernimmt: {text}");
    assert_eq!(location.as_deref(), Some(format!("{back}?ok=impressum").as_str()), "Rücksprung mit ok=impressum");
    assert_eq!(server.state.store.work_meta(work).await.unwrap().template_rev, rev + 1, "neue Vorlagenfassung");
    assert_eq!(commit_count(&server, &slug), commits + 1, "ein Commit");
    let adopted = server.state.store.work_template(work).await.unwrap();
    assert_eq!(adopted.matches("#let impressum(buch)").count(), 1, "die eigene Funktion bleibt, keine zweite");
    assert!(adopted.contains(own), "die eigene Impressum-Funktion steht unverändert da");
    assert_eq!(
        baruch_typst::cover::template_functions(&adopted),
        baruch_typst::cover::TemplateFunctions { half_title: true, main_title: true, imprint: true, dedication: true },
        "jetzt sind alle vier Funktionen da"
    );
    let (status, _, text) = post(&server, "/werke/umschlag/impressum", &form).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "zweiter Klick: ehrliche Ablehnung");
    assert!(text.contains("Impressum-Vorlage nicht übernommen"), "Meldung: {text}");
    assert_eq!(server.state.store.work_meta(work).await.unwrap().template_rev, rev + 1, "keine weitere Fassung");

    let prepared = baruch_server::typeset::prepare(&server.state, work).await.unwrap();
    assert_eq!(prepared.source.main.matches("#let baruch_buch = (").count(), 1, "Wörterbuch genau einmal");
    let book = book_pdf(&server, work).await;
    // The front matter begins after the cover page; the page with the imprint is searched for.
    let mut found = None;
    for page in 1..=book.pages {
        let Some(text) = page_text(&book.bytes, page) else {
            eprintln!("ÜBERSPRUNGEN: pdftotext fehlt (Impressum nicht gelesen)");
            return;
        };
        if text.contains("Impressum der Werkvorlage") {
            found = Some((page, text));
            break;
        }
    }
    let (page, text) = found.expect("Impressum der Werkvorlage im Buch-PDF");
    assert_eq!(page % 2, 0, "das Impressum steht auf einer linken (geraden) Seite, Seite {page}");
    let main_title = page_text(&book.bytes, page - 1).unwrap();
    assert!(
        main_title.contains("Das Leuchtturmjahr") && main_title.contains("Hanna Wellen"),
        "die Seite davor ist der Haupttitel (Impressum auf dessen Rückseite): {main_title}"
    );
    assert!(
        text.split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .contains(&format!("Impressum der Werkvorlage: {PUBLISHER} (2026) bei Druckerei Gischt")),
        "Buchangaben wörtlich, Sonderzeichen inklusive: {text}"
    );

    // Print check of the mandatory details: printer gone → warning on the typesetting page and in work.cover_pdf.
    result(&server.rpc("work.meta.set", json!({ "work": work, "printer": "", "printer_address": "" })).await);
    let (_, html) = server.get(&format!("/werke/{work}/satz")).await;
    assert!(html.contains("data-code=\"imprint_printer\""), "Satz-Seite warnt: Druckerei fehlt");
    assert!(!html.contains("data-code=\"imprint_publisher\""), "der Verlag ist vollständig");
    let (answer, _) = cover_pdf(&server, work).await;
    assert!(
        answer["checks"].as_array().unwrap().iter().any(|c| c["code"] == "imprint_printer"),
        "work.cover_pdf nennt die fehlende Druckerei: {answer}"
    );
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

async fn mcp_tool(server: &common::Server, session: &str, tool: &str, arguments: Value) -> Value {
    let message = json!({ "jsonrpc": "2.0", "id": 3, "method": "tools/call",
                          "params": { "name": tool, "arguments": arguments } });
    let answer = mcp_post(server, Some(session), message).await.1;
    assert_eq!(answer["result"]["isError"], false, "{tool} sollte gelingen: {answer}");
    answer["result"]["structuredContent"].clone()
}

/// MCP reads the saved layout and only files a request; only the confirmation changes anything. After that the exchange
/// (file `.baruch`) carries layout, addresses and typeset view without loss to a second instance, which typesets the
/// cover with the same source.
#[tokio::test]
async fn mcp_reads_and_requests_and_the_exchange_carries_layout_addresses_and_sheet() {
    let (a, work, _) = lighthouse().await;
    let mut form = pairs(&[("work", &work.to_string()), ("action", "save")]);
    form.extend(moved_fields(60.0));
    assert_eq!(post(&a, "/werke/umschlag/anordnung", &encode(&form)).await.0, StatusCode::SEE_OTHER);

    let (headers, init) = mcp_post(
        &a,
        None,
        json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize",
                "params": { "protocolVersion": "2025-06-18", "capabilities": {},
                            "clientInfo": { "name": "Abnahme27", "version": "1.0" } } }),
    )
    .await;
    assert!(init["result"].is_object(), "initialize: {init}");
    // The hint on connecting names the tools, otherwise a client finds them only through tools/list.
    let instructions = init["result"]["instructions"].as_str().unwrap();
    assert!(
        instructions.contains("cover_layout reads")
            && instructions.contains("cover_layout_set changes nothing immediately"),
        "instructions nennen cover_layout und sagen, dass cover_layout_set nur ein Antrag ist: {instructions}"
    );
    let session = headers.get("mcp-session-id").unwrap().to_str().unwrap().to_owned();
    let read = mcp_tool(&a, &session, "cover_layout", json!({ "work": work })).await;
    assert_eq!(read["stored"], true, "MCP sieht die gespeicherte Anordnung: {read}");
    let title = read["blocks"].as_array().unwrap().iter().find(|b| b["kind"] == "front_title").unwrap();
    assert_eq!((title["x_mm"].as_f64(), title["stored"].as_bool()), (Some(14.0), Some(true)), "Titel wie gespeichert");
    let before = a.state.store.cover_layout(work).await.unwrap();
    let filed = mcp_tool(
        &a,
        &session,
        "cover_layout_set",
        json!({ "work": work, "blocks": [{ "kind": "front_free", "y_mm": 140 }], "note": "Freitext tiefer" }),
    )
    .await;
    assert_eq!(filed["status"], "open", "nur ein Antrag: {filed}");
    assert_eq!(a.state.store.cover_layout(work).await.unwrap(), before, "der Antrag ändert nichts sofort");
    let id = filed["request"]["id"].as_i64().unwrap();
    result(&a.rpc("asset.request_decide", json!({ "work": work, "id": id, "decision": "confirm" })).await);
    let after = a.state.store.cover_layout(work).await.unwrap();
    assert_eq!(after.get(CoverBlockKind::FrontFree).unwrap().y_mm, 140.0, "nach Bestätigung durch den Menschen");

    result(&a.rpc("work.sheet_set", json!({ "work": work, "size": "13pt", "background": "#f4f1ea" })).await);
    let exported = result(&a.rpc("work.exchange_export", json!({ "work": work, "format": "baruch" })).await).clone();
    let data = exported["data"].as_str().unwrap();
    let b = common::open().await;
    let imported = result(
        &b.rpc("work.exchange_import", json!({ "data": data, "sha256": exported["sha256"], "mode": "new" })).await,
    )
    .clone();
    let work_b = imported["work"].as_i64().unwrap();
    assert_eq!(b.state.store.cover_layout(work_b).await.unwrap(), after, "Anordnung ohne Verlust");
    let (meta_a, meta_b) =
        (a.state.store.book_meta(work).await.unwrap(), b.state.store.book_meta(work_b).await.unwrap());
    assert_eq!(
        (meta_b.publisher_address.as_str(), meta_b.printer_address.as_str(), meta_b.publisher.as_str()),
        (meta_a.publisher_address.as_str(), meta_a.printer_address.as_str(), PUBLISHER),
        "Anschriften und Verlag ohne Verlust"
    );
    assert_eq!(
        b.state.store.sheet_settings(work_b).await.unwrap(),
        a.state.store.sheet_settings(work).await.unwrap(),
        "Satzbild-Einstellungen ohne Verlust"
    );
    let source_a = baruch_server::cover::cover_source(&a.state, work).await.unwrap();
    let source_b = baruch_server::cover::cover_source(&b.state, work_b).await.unwrap();
    assert_eq!(
        baruch_render::stand(&source_b),
        baruch_render::stand(&source_a),
        "die zweite Instanz setzt den Umschlag aus derselben Quelle"
    );
}
