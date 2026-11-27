// SPDX-License-Identifier: AGPL-3.0-or-later
//! Round 27, task umschlag-satz (R27b): typesetting of the layout, front matter from the work template, print check of
//! the cover and of the mandatory details.
//!
//! Real data directory in the temp directory, the own test file `gartenbuch.typ` and images generated in the test (PNG
//! with a dark upper third, SVG). No network, no model.

use crate::common;

use serde_json::{Value, json};

fn result(answer: &Value) -> &Value {
    assert!(answer["error"].is_null(), "RPC-Fehler: {answer}");
    &answer["result"]
}

/// RGB PNG (stored deflate blocks): upper third almost black, light yellow below (invented).
pub(crate) fn dark_top_png(width: u32, height: u32) -> Vec<u8> {
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
        for _ in 0..width {
            if y < height / 3 { raw.extend([12, 14, 20]) } else { raw.extend([250, 240, 200]) }
        }
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
    let mut header = width.to_be_bytes().to_vec();
    header.extend(height.to_be_bytes());
    header.extend([8, 2, 0, 0, 0]);
    chunk(&mut out, b"IHDR", &header);
    chunk(&mut out, b"IDAT", &z);
    chunk(&mut out, b"IEND", &[]);
    out
}

/// Invented author photo.
const FACE: &str = "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"40\" height=\"50\">\
<rect width=\"40\" height=\"50\" fill=\"#c9a27e\"/></svg>\n";

async fn add_png(server: &common::Server, work: i64, name: &str, bytes: &[u8]) -> String {
    let added = server
        .rpc(
            "asset.add",
            json!({ "work": work, "file_name": name, "content": baruch_store::encode_base64(bytes),
                    "encoding": "base64", "caption": "Erfundene Wolke", "alt": "Wolke" }),
        )
        .await;
    result(&added)["name"].as_str().unwrap().to_owned()
}

async fn add_svg(server: &common::Server, work: i64, name: &str, svg: &str) -> String {
    let added = server
        .rpc("asset.add", json!({ "work": work, "file_name": name, "content": svg, "encoding": "svg", "alt": "Foto" }))
        .await;
    result(&added)["name"].as_str().unwrap().to_owned()
}

/// State of the typesetting source (`typeset::prepare`) and SHA-256 of the cover PDF.
async fn fingerprints(server: &common::Server, work: i64) -> (String, String) {
    let prepared = baruch_server::typeset::prepare(&server.state, work).await.unwrap();
    let pdf = cover_pdf(server, work).await;
    (prepared.stand, result(&pdf)["sha256"].as_str().unwrap().to_owned())
}

async fn cover_pdf(server: &common::Server, work: i64) -> Value {
    let _serial = common::PDF_SERIAL.lock().await;
    server.rpc("work.cover_pdf", json!({ "work": work })).await
}

/// Golden states before the first change of R27b (measured for these outputs from the state of the interface S1 =
/// af07921): (case, state of the typesetting source, SHA-256 of the cover PDF). Round 30a: the states of the
/// typesetting source have been measured anew with the jump labels `<kN>` (contract R30 §3.3); the cover PDFs stay
/// byte-identical.
const GOLDEN: [(&str, &str, &str); 6] = [
    (
        "ohne",
        "66f6ebb13b517e08ff41c16279a27fca17f29f98380db0c5d0a739a840901e57",
        "94f808b18c475a9ca84e73018f1e2f4e41948b4708999c05e765941a9e0dd77d",
    ),
    (
        "titelei",
        "0314b4d7b4ee5903eea3540f5cadfb484f29ff9b6b0ade0d868af18da9ff220d",
        "b8ddee07a1e818536b1322f514bf92bcbe7997cca9114db39aee1f7fe6f2b7fe",
    ),
    (
        "beides",
        "d097a6ff27d8b04419a9990f2f0b51f3aef1218849a13e8f79a9bfb709038ce6",
        "1d7ca4d55232dd3c889f783b057606d33204d41e5053bc7c355721e5e5c73704",
    ),
    (
        "umschlag",
        "84f5cae1c72d9596e3ed351516bdace42aed4c06f3ffa15f02909350bd6b9eef",
        "d034b1c2705e23cfb1b705d48f8e088edd8cdef14094a270f68276765f618bc5",
    ),
    (
        "ruecken_duenn",
        "84f5cae1c72d9596e3ed351516bdace42aed4c06f3ffa15f02909350bd6b9eef",
        "4e8fccb01d3328fabe45351d37a505e150567b371c253685e2fd8cb4187afd6b",
    ),
    (
        "ruecken_dick",
        "e5bbf93d918d4619041f25be6a36f24509ebc918dbdb76a54a9ea98dbb107766",
        "a73698182a8574b3ad8fdcc4767a9c1d2f32dbd91eeacb5cd268eefab8b6a055",
    ),
];

#[tokio::test]
async fn without_layout_and_template_functions_source_and_cover_pdf_stay_byte_identical() {
    let server = common::open().await;
    let work = server.import_garden().await.work;
    let mut seen = Vec::new();
    seen.push(("ohne", fingerprints(&server, work).await));
    let set = server
        .rpc(
            "work.meta.set",
            json!({ "work": work, "front_matter": true, "isbn": "978-3-16-148410-0", "publisher": "Beetverlag",
                    "place": "Kressbach", "year": "2026", "edition": "1. Auflage", "price": "12,00 €",
                    "series": "Gartenreihe", "printer": "Druckerei Spaten", "bible": "Erfundene Übersetzung",
                    "bible_notice": "Abdruck mit Genehmigung.", "blurb": "Ein Jahr im Garten.\n\nVon Saat bis Ernte.",
                    "bio": "Grete gärtnert.", "dedication": "Für alle Beete.", "subtitle": "Ein Jahr" }),
        )
        .await;
    result(&set);
    seen.push(("titelei", fingerprints(&server, work).await));
    let cover = add_png(&server, work, "wolke.png", &dark_top_png(60, 90)).await;
    let photo = add_svg(&server, work, "gesicht.svg", FACE).await;
    result(&server.rpc("work.set_cover", json!({ "work": work, "name": cover })).await);
    result(&server.rpc("work.meta.set", json!({ "work": work, "author_photo": photo, "title_area": "middle" })).await);
    seen.push(("beides", fingerprints(&server, work).await));
    result(&server.rpc("work.meta.set", json!({ "work": work, "front_matter": false })).await);
    seen.push(("umschlag", fingerprints(&server, work).await));
    result(&server.rpc("work.meta.set", json!({ "work": work, "paper_mm": 0.3 })).await);
    seen.push(("ruecken_duenn", fingerprints(&server, work).await));
    result(
        &server
            .rpc(
                "work.meta.set",
                json!({ "work": work, "paper_mm": 1.0, "front_matter": true, "title_area": "bottom" }),
            )
            .await,
    );
    seen.push(("ruecken_dick", fingerprints(&server, work).await));
    // The cases cover both sides of the 6 mm limit for the spine text.
    let spine = |pdf: Value| result(&pdf)["spine_mm"].as_f64().unwrap();
    let thick = spine(cover_pdf(&server, work).await);
    assert!(thick > 6.0, "Fall ruecken_dick: Rücken über 6 mm erwartet, {thick} mm");
    result(&server.rpc("work.meta.set", json!({ "work": work, "paper_mm": 0.3 })).await);
    let thin = spine(cover_pdf(&server, work).await);
    assert!(thin < 6.0, "Fall ruecken_duenn: Rücken unter 6 mm erwartet, {thin} mm");

    let report: Vec<String> = seen.iter().map(|(name, (s, p))| format!("(\"{name}\", \"{s}\", \"{p}\"),")).collect();
    for ((name, (stand, pdf)), (golden_name, golden_stand, golden_pdf)) in seen.iter().zip(GOLDEN) {
        assert_eq!(*name, golden_name, "Reihenfolge der Fälle");
        assert_eq!(
            stand,
            golden_stand,
            "Fall {name}: Satzquelle muss byte-gleich zum Stand vor R27 bleiben (gemessen:\n{})",
            report.join("\n")
        );
        assert_eq!(
            pdf,
            golden_pdf,
            "Fall {name}: Umschlag-PDF muss byte-gleich bleiben (gemessen:\n{})",
            report.join("\n")
        );
    }
}

// ---- Layout in the cover, print check ----------------------------------------------------------------

use baruch_server::cover_print::{CoverWarning, cover_checks, imprint_checks};
use baruch_store::{BookMeta, CoverBlock, CoverBlockKind, CoverLayout};

/// A block with defaults; position, width, size and colour from the arguments.
fn block(kind: CoverBlockKind, x: f64, y: f64, width: f64, color: &str) -> CoverBlock {
    serde_json::from_value(json!({
        "kind": kind, "x_mm": x, "y_mm": y, "width_mm": width, "align": "left", "size_pt": 14.0,
        "leading_em": 0.65, "color": color, "font": null, "style": "normal", "backdrop": "none", "visible": true,
        "direction": if kind.face() == baruch_store::CoverFace::Spine { json!("down") } else { json!(null) },
        "text": if kind.own_text() { json!("") } else { json!(null) },
    }))
    .unwrap()
}

/// Garden book with book details and PNG cover (dark upper third).
async fn garden_with_cover(server: &common::Server) -> i64 {
    let work = server.import_garden().await.work;
    let cover = add_png(server, work, "wolke.png", &dark_top_png(60, 90)).await;
    result(&server.rpc("work.set_cover", json!({ "work": work, "name": cover })).await);
    let set = server
        .rpc(
            "work.meta.set",
            json!({ "work": work, "subtitle": "Ein Jahr", "author": "Grete Gärtner", "publisher": "Beetverlag",
                    "isbn": "978-3-16-148410-0", "price": "12,00 €", "blurb": "Ein Jahr im Garten." }),
        )
        .await;
    result(&set);
    work
}

fn has(warnings: &[CoverWarning], code: &str, kind: CoverBlockKind) -> bool {
    warnings.iter().any(|w| w.code == code && w.block == Some(kind))
}

#[tokio::test]
async fn the_print_check_warns_about_bleed_safety_barcode_spine_contrast_and_fonts() {
    let server = common::open().await;
    let work = garden_with_cover(&server).await;
    // Garden book: A4 (210 × 297 mm), bleed 3 mm. The image (60 × 90 px) fills the front including the bleed (213 × 303
    // mm) with 3.55 mm per pixel; the dark rows 0–29 reach from y = −11 mm to a good 95 mm.
    let mut title = block(CoverBlockKind::FrontTitle, -2.0, 10.0, 120.0, "light");
    title.font = Some("Gibt Es Nicht Grotesk".into());
    let author_dark_on_dark = block(CoverBlockKind::FrontAuthor, 30.0, 30.0, 120.0, "dark");
    let mut free_light_on_dark = block(CoverBlockKind::FrontFree, 30.0, 50.0, 120.0, "light");
    free_light_on_dark.text = Some("Heller Text oben".into());
    let publisher_near_edge = block(CoverBlockKind::FrontPublisher, 2.0, 200.0, 100.0, "dark");
    let barcode = block(CoverBlockKind::BackBarcode, 150.0, 250.0, 40.0, "light");
    let blurb_on_barcode = block(CoverBlockKind::BackBlurb, 140.0, 255.0, 40.0, "light");
    let spine_title = block(CoverBlockKind::SpineTitle, 0.0, 40.0, 150.0, "light");
    let mut back_free_dark = block(CoverBlockKind::BackFree, 20.0, 120.0, 150.0, "dark");
    back_free_dark.text = Some("Dunkel auf Dunkel".into());
    let layout = CoverLayout {
        blocks: vec![
            title,
            author_dark_on_dark,
            free_light_on_dark,
            publisher_near_edge,
            barcode,
            blurb_on_barcode,
            spine_title,
            back_free_dark,
        ],
    };
    let warnings = cover_checks(&server.state, work, Some(&layout)).await.unwrap();
    let list = format!("{warnings:#?}");
    assert!(has(&warnings, "bleed", CoverBlockKind::FrontTitle), "x = −2 mm ragt in den Beschnitt: {list}");
    assert!(has(&warnings, "safety", CoverBlockKind::FrontPublisher), "x = 2 mm liegt im Sicherheitsabstand: {list}");
    assert!(!has(&warnings, "bleed", CoverBlockKind::FrontPublisher), "aber nicht im Beschnitt: {list}");
    assert!(has(&warnings, "barcode", CoverBlockKind::BackBlurb), "Klappentext auf dem Strichcode: {list}");
    assert!(has(&warnings, "spine", CoverBlockKind::SpineTitle), "14 pt sind dicker als der dünne Rücken: {list}");
    assert!(has(&warnings, "contrast", CoverBlockKind::FrontAuthor), "dunkler Text im dunklen Drittel: {list}");
    assert!(!has(&warnings, "contrast", CoverBlockKind::FrontFree), "heller Text im dunklen Drittel ist gut: {list}");
    assert!(!has(&warnings, "contrast", CoverBlockKind::FrontPublisher), "dunkler Text auf Hellgelb ist gut: {list}");
    assert!(has(&warnings, "contrast", CoverBlockKind::BackFree), "dunkler Text auf der dunklen Rückseite: {list}");
    assert!(!has(&warnings, "contrast", CoverBlockKind::BackBlurb), "heller Text auf der Rückseite ist gut: {list}");
    assert!(has(&warnings, "font", CoverBlockKind::FrontTitle), "erfundene Schrift fehlt: {list}");
    let font = warnings.iter().find(|w| w.code == "font").unwrap();
    assert!(font.message.contains("Gibt Es Nicht Grotesk") && font.message.contains("Vorgabe"), "Meldung: {font:?}");
    assert!(
        warnings.iter().all(|w| baruch_server::cover_print::CODES.contains(&w.code)),
        "nur Codes aus CODES: {list}"
    );
    assert!(
        warnings.iter().all(|w| !w.code.starts_with("imprint_")),
        "ohne Titelei keine Pflichtangaben-Warnungen: {list}"
    );

    // The missing font is not in the typesetting (fallback to the default), the cover typesets without error.
    let wrap = baruch_server::cover::build_wrap_for(&server.state, work, Some(&layout)).await.unwrap();
    assert!(!wrap.source.main.contains("Gibt Es Nicht Grotesk"), "unbekannte Schrift fällt weg:\n{}", wrap.source.main);
    assert!(wrap.source.main.contains("// front_title"), "Anordnungsmodus im Umschlag");
    let rendered = baruch_render::compile(&wrap.source);
    assert!(!rendered.has_errors(), "Umschlag mit Anordnung setzt fehlerfrei: {:?}", rendered.diagnostics);
    assert!(
        !rendered.diagnostics.iter().any(|d| d.message.contains("Gibt Es Nicht")),
        "keine Typst-Warnung zur Schrift, weil sie gar nicht verlangt wird: {:?}",
        rendered.diagnostics
    );

    // Light text at the top with a shadow stays good; dark text stays bad even with a shadow.
    let mut shadowed = layout.clone();
    for b in &mut shadowed.blocks {
        if b.kind == CoverBlockKind::FrontAuthor {
            b.backdrop = baruch_typst::cover_layout::Backdrop::Shadow;
        }
    }
    let warnings = cover_checks(&server.state, work, Some(&shadowed)).await.unwrap();
    assert!(has(&warnings, "contrast", CoverBlockKind::FrontAuthor), "Schatten macht dunklen Text nicht lesbar");

    // An SVG cover is not measured (contract 1.8).
    let svg = "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"60\" height=\"90\"><rect width=\"60\" height=\"90\" fill=\"#000\"/></svg>";
    let name = add_svg(&server, work, "schwarz.svg", svg).await;
    result(&server.rpc("work.set_cover", json!({ "work": work, "name": name })).await);
    let warnings = cover_checks(&server.state, work, Some(&layout)).await.unwrap();
    assert!(
        !warnings
            .iter()
            .any(|w| w.code == "contrast" && w.block.is_some_and(|k| k.face() == baruch_store::CoverFace::Front)),
        "SVG-Umschlag: kein Kontrast auf der Vorderseite gemessen: {warnings:#?}"
    );
}

#[tokio::test]
async fn without_own_layout_only_the_imprint_is_checked_and_the_layout_view_measures_heights() {
    let server = common::open().await;
    let work = garden_with_cover(&server).await;
    let warnings = cover_checks(&server.state, work, None).await.unwrap();
    assert!(warnings.is_empty(), "ohne Anordnung und ohne Titelei keine Warnung: {warnings:#?}");
    let view = result(&server.rpc("cover.layout", json!({ "work": work })).await).clone();
    let blocks = view["blocks"].as_array().unwrap();
    let height = |kind: &str| blocks.iter().find(|b| b["kind"] == kind).unwrap()["height_mm"].as_f64().unwrap();
    assert!(height("front_title") > 5.0, "Titel (30 pt) ist gesetzt und hoch: {view}");
    assert!(height("front_title") > height("front_author"), "30 pt höher als 13 pt: {view}");
    assert_eq!(height("front_free"), 0.0, "leerer Freitext hat die Höhe 0: {view}");
    assert!(height("back_barcode") > 15.0, "Strichcode-Kasten mit Bild und ISBN-Zeile: {view}");
    assert_eq!(view["default_font"], "Libertinus Serif", "Gartenbuch nennt keine Schrift → Vorgabe");
}

#[tokio::test]
async fn the_default_font_is_the_one_typst_sets_the_body_with() {
    // Follow-up fix Round 27: cover PDF, measurement and editor take the font that Typst really sets in the book – the
    // last rule at top level, not the first occurrence in the text.
    let server = common::open().await;
    let work = garden_with_cover(&server).await;
    let template = server.state.store.work_template(work).await.unwrap();
    let with_fonts = format!(
        "// #set text(font: \"Auskommentiert\")\n#set text(font: \"Erste Antiqua\")\n\
         #show heading: set text(font: \"Titel Grotesk\")\n#set text(font: \"Zweite Antiqua\")\n{template}"
    );
    result(&server.rpc("work.update", json!({ "work": work, "template": with_fonts })).await);
    let view = result(&server.rpc("cover.layout", json!({ "work": work })).await).clone();
    assert_eq!(
        view["default_font"], "Zweite Antiqua",
        "die spätere set-Regel gewinnt wie in Typst; Kommentar und Show-Regel zählen nicht: {view}"
    );
}

#[tokio::test]
async fn hostile_free_texts_in_a_preview_are_set_literally_without_errors() {
    let server = common::open().await;
    let work = garden_with_cover(&server).await;
    let nasty = "Zitat \" Rück \\ #raute ] zu ) $x$ // k /* b */\nzweite Zeile";
    let one_line = nasty.replace('\n', " ");
    result(&server.rpc("work.meta.set", json!({ "work": work, "publisher": one_line, "blurb": nasty })).await);
    let mut free = block(CoverBlockKind::FrontFree, 20.0, 40.0, 150.0, "light");
    free.text = Some(nasty.into());
    let mut motto = block(CoverBlockKind::BackMotto, 20.0, 40.0, 150.0, "light");
    motto.text = Some(nasty.into());
    let mut spine = block(CoverBlockKind::SpineFree, 0.0, 20.0, 100.0, "light");
    spine.text = Some(nasty.into());
    let layout = CoverLayout { blocks: vec![free, motto, spine] };
    let wrap = baruch_server::cover::build_wrap_for(&server.state, work, Some(&layout)).await.unwrap();
    let rendered = baruch_render::compile(&wrap.source);
    assert!(!rendered.has_errors(), "feindlicher Text setzt fehlerfrei: {:?}", rendered.diagnostics);
    assert!(
        wrap.source.main.contains("\"Zitat \\\" Rück \\\\ #raute ] zu ) $x$ // k /* b */\\nzweite Zeile\""),
        "nur als String-Literal, der Zeilenumbruch maskiert:\n{}",
        wrap.source.main
    );
}

#[tokio::test]
async fn the_imprint_checks_name_missing_publisher_printer_year_and_bible_notice() {
    let empty = BookMeta { front_matter: true, ..BookMeta::default() };
    let codes: Vec<&str> = imprint_checks(&empty, "Erfundene Übersetzung").iter().map(|w| w.code).collect();
    assert_eq!(
        codes,
        ["imprint_publisher", "imprint_printer", "imprint_year", "imprint_bible_notice"],
        "alles leer, Bibel gesetzt: vier Warnungen (Vertrag 1.7)"
    );
    assert!(
        imprint_checks(&empty, "").iter().all(|w| w.code != "imprint_bible_notice"),
        "ohne Bibel kein Hinweis nötig"
    );
    let off = BookMeta { front_matter: false, ..BookMeta::default() };
    assert!(imprint_checks(&off, "Bibel").is_empty(), "ohne Titelei keine Pflichtangaben-Prüfung");
    let full = BookMeta {
        front_matter: true,
        publisher: "Beetverlag".into(),
        publisher_address: "Beetweg 1\n12345 Kressbach".into(),
        printer: "Druckerei Spaten".into(),
        printer_address: "Spatenstraße 2, 54321 Erde".into(),
        year: "2026".into(),
        bible_notice: "Mit Genehmigung.".into(),
        ..BookMeta::default()
    };
    assert!(imprint_checks(&full, "Erfundene Übersetzung").is_empty(), "alles da → keine Warnung");
    let no_address = BookMeta { publisher_address: String::new(), printer_address: " ".into(), ..full.clone() };
    let codes: Vec<&str> = imprint_checks(&no_address, "").iter().map(|w| w.code).collect();
    assert_eq!(codes, ["imprint_publisher", "imprint_printer"], "fehlende Anschriften zählen wie fehlende Namen");
}

#[tokio::test]
async fn the_typeset_page_shows_imprint_warnings_in_the_print_check() {
    let server = common::open().await;
    let work = server.import_garden().await.work;
    let (status, html) = server.get(&format!("/werke/{work}/satz")).await;
    assert_eq!(status, axum::http::StatusCode::OK, "Satz-Seite: {html}");
    assert!(!html.contains("typeset-imprint"), "ohne Titelei keine Impressum-Warnung");
    result(
        &server
            .rpc("work.meta.set", json!({ "work": work, "front_matter": true, "bible": "Erfundene Übersetzung" }))
            .await,
    );
    let (_, html) = server.get(&format!("/werke/{work}/satz")).await;
    let section = &html[html.find("id=\"druckpruefung\"").expect("Abschnitt #druckpruefung")..];
    let section = &section[..section.find("</section>").unwrap()];
    for code in ["imprint_publisher", "imprint_printer", "imprint_year", "imprint_bible_notice"] {
        assert!(section.contains(&format!("data-code=\"{code}\"")), "{code} in #druckpruefung: {section}");
    }
    assert!(section.contains("Landespressegesetzen") && section.contains("keine Rechtsberatung"), "Hinweis: {section}");
    assert!(section.contains("Erscheinungsjahr fehlt"), "deutsche Meldung: {section}");
}

#[tokio::test]
async fn an_imprint_function_in_the_template_is_set_on_the_back_of_the_main_title() {
    let server = common::open().await;
    let work = server.import_garden().await.work;
    let set = server
        .rpc(
            "work.meta.set",
            json!({ "work": work, "front_matter": true, "publisher": "Beetverlag \" ) #x", "year": "2026" }),
        )
        .await;
    result(&set);
    let builtin = baruch_server::typeset::prepare(&server.state, work).await.unwrap();
    assert!(!builtin.source.main.contains("baruch_buch"), "ohne Funktion kein Wörterbuch (Rückfall)");
    let template = server.state.store.work_template(work).await.unwrap();
    let with_function = format!("{template}#let impressum(buch) = [Eigenes Impressum: #buch.verlag (#buch.jahr)]\n");
    result(&server.rpc("work.update", json!({ "work": work, "template": with_function })).await);
    let prepared = baruch_server::typeset::prepare(&server.state, work).await.unwrap();
    assert_eq!(prepared.source.main.matches("#let baruch_buch = (").count(), 1, "Wörterbuch genau einmal");
    assert!(prepared.source.main.contains("  impressum(baruch_buch)\n"), "Aufruf auf eigener Seite");
    assert!(!prepared.source.main.contains("text(\"Bibliografische Information"), "eingebautes Impressum entfällt");
    let pdf = {
        let _serial = common::PDF_SERIAL.lock().await;
        tokio::task::spawn_blocking(move || baruch_render::compile_pdf(&prepared.source)).await.unwrap()
    };
    assert!(!pdf.has_errors(), "Satz mit Vorlagenfunktion fehlerfrei: {:?}", pdf.diagnostics);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("impressum.pdf");
    std::fs::write(&path, pdf.bytes.as_ref().unwrap()).unwrap();
    let Ok(out) = std::process::Command::new("pdftotext").args(["-f", "4", "-l", "4"]).arg(&path).arg("-").output()
    else {
        eprintln!("ÜBERSPRUNGEN: pdftotext fehlt (Seite 4 nicht gelesen)");
        return;
    };
    let page4 = String::from_utf8_lossy(&out.stdout);
    assert!(
        page4.contains("Eigenes Impressum: Beetverlag \" ) #x (2026)"),
        "Seite 4 (Rückseite des Haupttitels) zeigt das Impressum der Vorlage wörtlich: {page4}"
    );
}
