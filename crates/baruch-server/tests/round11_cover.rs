// SPDX-License-Identifier: AGPL-3.0-or-later
//! Round 11, task cover: cover, front matter and book details in the server. Real data folder (SQLite, Git mirror) in
//! the temp folder, the own test file `gartenbuch.typ` and SVG images created in the test.
//!
//! Checked are the frame in the book typesetting (`cover::frame`: without cover byte-identical, with cover front and
//! back, front matter, mapping of the messages), user texts as string literals, the cover for the print shop
//! (`cover::cover_source`), RPC with limits and commits, forms without JavaScript, settings page „Werk“ and work
//! overview.

use crate::common;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use baruch_render::Severity;
use serde_json::{Value, json};

/// Invented cover image (portrait format, two areas).
const POND: &str = "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"600\" height=\"900\">\
<rect width=\"600\" height=\"900\" fill=\"#2e7d4f\"/><circle cx=\"300\" cy=\"500\" r=\"180\" fill=\"#6fb3d9\"/></svg>\n";
/// Invented author photo.
const FACE: &str = "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"40\" height=\"50\">\
<rect width=\"40\" height=\"50\" fill=\"#c9a27e\"/></svg>\n";
/// Valid ISBN-13 (textbook example 978-3-16-148410-0).
const ISBN: &str = "978-3-16-148410-0";

fn result(answer: &Value) -> &Value {
    assert!(answer["error"].is_null(), "RPC-Fehler: {answer}");
    &answer["result"]
}

fn error(answer: &Value) -> (i64, String) {
    assert!(answer["result"].is_null(), "Fehler erwartet, bekam {answer}");
    (answer["error"]["code"].as_i64().unwrap(), answer["error"]["message"].as_str().unwrap().to_owned())
}

async fn add_svg(server: &common::Server, work: i64, file_name: &str, svg: &str, caption: &str, alt: &str) -> String {
    let answer = server
        .rpc(
            "asset.add",
            json!({ "work": work, "file_name": file_name, "content": svg, "encoding": "svg",
                    "caption": caption, "alt": alt }),
        )
        .await;
    result(&answer)["name"].as_str().unwrap().to_owned()
}

/// Import the Gartenbuch; work id and slug.
async fn garden(server: &common::Server) -> (i64, String) {
    let report = server.import_garden().await;
    (report.work, report.slug)
}

fn git_log(server: &common::Server, slug: &str, format: &str) -> Option<String> {
    common::git(&server.mirror_dir_of(slug), &["log", "-1", &format!("--format={format}")])
}

async fn frame_of(
    server: &common::Server,
    work: i64,
) -> (baruch_render::Source, baruch_store::ExportMap, baruch_render::Source, baruch_store::ExportMap) {
    // The source without frame as in `typeset::prepare` before the frame (prepare itself has framed since the
    // integration already; here it is about the frame alone).
    let map = server.state.store.export_map(work).await.unwrap();
    let files = server.state.store.export_assets(work).await.unwrap().into_iter().map(|f| (f.path, f.bytes)).collect();
    let plain = baruch_render::Source { main: map.typst.clone(), files };
    let (source, framed_map) =
        baruch_server::cover::frame(&server.state, work, plain.clone(), map.clone()).await.unwrap();
    (plain, map, source, framed_map)
}

#[tokio::test]
async fn without_cover_and_front_matter_the_frame_changes_nothing() {
    let server = common::open().await;
    let (work, _) = garden(&server).await;
    let (source, map, framed, framed_map) = frame_of(&server, work).await;
    assert_eq!(framed, source, "ohne Umschlag und Titelei: Quelle byte-gleich (bestehende Werke ändern sich nicht)");
    assert_eq!(framed_map, map, "Karte der Abschnitte unverändert");
    assert_eq!(
        baruch_render::stand(&framed),
        baruch_render::stand(&source),
        "derselbe Stand → derselbe Zwischenspeicher"
    );

    // A cover without „Umschlagseiten im Satz“ does not change the book typesetting either.
    let name = add_svg(&server, work, "teich.svg", POND, "Teich im Garten", "Ein Teich").await;
    result(&server.rpc("work.set_cover", json!({ "work": work, "name": name })).await);
    result(&server.rpc("work.meta.set", json!({ "work": work, "cover_pages": false })).await);
    let (source, _, framed, _) = frame_of(&server, work).await;
    assert_eq!(framed, source, "cover_pages aus: der Umschlag wirkt nur im Umschlag-PDF, nicht im Buchsatz");
}

#[tokio::test]
async fn with_cover_and_front_matter_the_book_gains_cover_title_pages_and_back() {
    let server = common::open().await;
    let (work, _) = garden(&server).await;
    let plain = baruch_render::compile(&frame_of(&server, work).await.0);
    assert!(!plain.has_errors(), "Gartenbuch setzt fehlerfrei: {:?}", plain.diagnostics);

    let name = add_svg(&server, work, "teich.svg", POND, "Teich im Garten, Foto: Erfunden", "Ein Teich").await;
    let photo = add_svg(&server, work, "gesicht.svg", FACE, "", "Die Autorin").await;
    result(&server.rpc("work.set_cover", json!({ "work": work, "name": name })).await);
    let set = server
        .rpc(
            "work.meta.set",
            json!({ "work": work, "front_matter": true, "title_area": "bottom", "isbn": ISBN, "publisher": "Beetverlag",
                    "place": "Kressbach", "year": "2026", "edition": "1. Auflage", "bible": "Erfundene Übersetzung",
                    "bible_notice": "Abdruck mit freundlicher Genehmigung.", "blurb": "Ein Jahr im Garten.\n\nVon der Aussaat bis zur Ernte.",
                    "bio": "Grete Gärtner gärtnert.", "author_photo": photo, "dedication": "Für alle Beete." }),
        )
        .await;
    result(&set);
    let (source, map, framed, framed_map) = frame_of(&server, work).await;
    let front = &framed.main[..framed_map.sections[0].start];
    let back = &framed.main[framed.main.len() - (framed.main.len() - framed_map.sections.last().unwrap().end)..];

    // Order of the front matter in the source text: cover, half-title, page 2, main title, imprint, dedication.
    let at = |needle: &str| front.find(needle).unwrap_or_else(|| panic!("{needle:?} fehlt in der Titelei:\n{front}"));
    assert!(at("image(\".baruch/teich.svg\"") < at("Bibliografische Information"), "Umschlag vor dem Impressum");
    for line in [
        "Bibliografische Information der Deutschen Nationalbibliothek",
        "© 2026 Grete Gärtner, Beetverlag, Kressbach",
        "1. Auflage",
        "ISBN 978-3-16-148410-0",
        "Bibelzitate: Erfundene Übersetzung. Abdruck mit freundlicher Genehmigung.",
        "Umschlaggestaltung und Bildnachweis: Teich im Garten, Foto: Erfunden",
        "Gesetzt mit Baruch",
    ] {
        at(line);
    }
    assert!(at("Gesetzt mit Baruch") < at("Für alle Beete."), "die Widmung folgt dem Impressum");
    assert!(at("bottom + center") > 0, "Titel im gewählten Bereich unten");
    assert!(
        back.contains("Ein Jahr im Garten.") && back.contains(".baruch/isbn.svg"),
        "Rückseite: Klappentext und Strichcode"
    );
    assert!(back.contains(".baruch/gesicht.svg"), "Rückseite: Autorenfoto");
    assert!(
        !front.contains("heading(") && !front.contains("outline(") && !back.contains("heading("),
        "keine Überschrift, kein Inhaltsverzeichnis im Rahmen"
    );
    for path in [".baruch/teich.svg", ".baruch/gesicht.svg", ".baruch/isbn.svg"] {
        assert!(framed.files.iter().any(|(p, _)| p == path), "virtuelle Datei {path} liegt dem Satz bei");
    }
    assert_eq!(framed_map.sections.len(), map.sections.len(), "gleich viele Abschnitte");
    for (old, new) in map.sections.iter().zip(&framed_map.sections) {
        assert_eq!(
            &framed.main[new.start..new.end],
            &source.main[old.start..old.end],
            "Abschnitt {} liegt nach dem Einfügen weiter unter seinem Bytebereich",
            old.key
        );
    }

    let rendered = baruch_render::compile(&framed);
    assert!(!rendered.has_errors(), "Buch mit Rahmen setzt fehlerfrei: {:?}", rendered.diagnostics);
    // Cover 3 (front, inside, back) + half-title, page 2, main title, imprint + dedication with empty reverse 2.
    assert_eq!(
        rendered.pages.len(),
        plain.pages.len() + 9,
        "Seiten: Buch + 3 Umschlag (mit leerer Innenseite) + 4 Titelei + 2 Widmung"
    );
    assert!(rendered.pages[0].contains("<image"), "erste Seite: das vollflächige Umschlagbild");
    let last = rendered.pages.last().unwrap();
    assert!(last.contains("#1f2a33") && last.contains("<image"), "letzte Seite: dunkle Rückseite mit Strichcode-Bild");
    let first_heading = rendered.headings.first().expect("Überschriften gefunden");
    assert_eq!(
        first_heading.page, 9,
        "das erste Kapitel beginnt nach Vorder- und Innenseite, 4 Seiten Titelei und 2 Seiten Widmung (physisch \
         Seite 9, rechts)"
    );
}

#[tokio::test]
async fn messages_still_point_to_their_section_after_the_frame() {
    let server = common::open().await;
    let source = "#set document(title: \"Fehlerbuch\", author: \"Erfunden\")\n\n= Eins\n\nGut.\n\n= Zwei\n\nHier #gibtsnicht() ein Fehler.\n";
    let report = server.import(source, "fehlerbuch.typ").await;
    let work = report.work;
    let name = add_svg(&server, work, "teich.svg", POND, "", "Ein Teich").await;
    result(&server.rpc("work.set_cover", json!({ "work": work, "name": name })).await);
    result(&server.rpc("work.meta.set", json!({ "work": work, "front_matter": true })).await);
    let (_, map, framed, framed_map) = frame_of(&server, work).await;
    let rendered = baruch_render::compile(&framed);
    let error = rendered.diagnostics.iter().find(|d| d.severity == Severity::Error).expect("Fehler in Kapitel 2");
    let line = error.line.expect("Meldung mit Zeile");
    let section = framed_map.section_at_line(line).expect("Zeile gehört zu einem Abschnitt, nicht zur Werkvorlage");
    assert_eq!(
        section.key, map.sections[1].key,
        "die Meldung gehört zu Kapitel 2, auch mit Umschlag und Titelei davor"
    );
    let in_section = line - section.first_line + 1;
    assert_eq!(in_section, 3, "Zeile im Abschnitt: Überschrift, Leerzeile, dann der Fehler");
}

#[tokio::test]
async fn user_texts_are_set_literally_and_never_run_as_typst() {
    let server = common::open().await;
    let (work, _) = garden(&server).await;
    let wild = "Ein #Test *fett* ] \"zitat\" $x$ \\ ende";
    let attack = "\") #panic(\"eingeschleust\") #(\"";
    result(&server.rpc("work.update", json!({ "work": work, "title": wild, "subtitle": attack })).await);
    let name = add_svg(&server, work, "teich.svg", POND, attack, attack).await;
    result(&server.rpc("work.set_cover", json!({ "work": work, "name": name })).await);
    let meta = json!({ "work": work, "front_matter": true, "blurb": wild, "bio": attack, "publisher": attack,
                       "series": wild, "dedication": attack, "price": "$5 #x" });
    result(&server.rpc("work.meta.set", meta).await);
    let (_, _, framed, _) = frame_of(&server, work).await;
    assert!(
        framed.main.contains(&baruch_typst::cover::typst_string(wild)),
        "der Titel steht als maskiertes String-Literal im Satz"
    );
    let rendered = baruch_render::compile(&framed);
    assert!(
        !rendered.has_errors(),
        "# * ] \" $ \\ und ein Ausbruchsversuch werden gesetzt, nicht ausgeführt: {:?}",
        rendered.diagnostics
    );
    let wrap = baruch_server::cover::cover_source(&server.state, work).await.unwrap();
    let rendered = baruch_render::compile(&wrap);
    assert!(!rendered.has_errors(), "auch der Umschlag für die Druckerei: {:?}", rendered.diagnostics);
}

/// Width and height (mm) of an SVG page of Typst.
fn svg_mm(svg: &str) -> (f64, f64) {
    let head = &svg[..svg.find('>').unwrap()];
    let attr = |name: &str| -> f64 {
        let at = head.find(&format!(" {name}=\"")).unwrap() + name.len() + 3;
        let value = &head[at..at + head[at..].find('"').unwrap()];
        value.strip_suffix("pt").unwrap().parse::<f64>().unwrap() * 25.4 / 72.0
    };
    (attr("width"), attr("height"))
}

#[tokio::test]
async fn the_wrap_is_one_page_as_wide_as_both_sides_spine_and_bleed() {
    // Builds a cover PDF below: the mark „PDF entsteht“ is process-wide per work id.
    let _serial = common::PDF_SERIAL.lock().await;
    let server = common::open().await;
    let (work, _) = garden(&server).await;
    let name = add_svg(&server, work, "teich.svg", POND, "", "Ein Teich").await;
    result(&server.rpc("work.set_cover", json!({ "work": work, "name": name })).await);
    result(&server.rpc("work.meta.set", json!({ "work": work, "isbn": ISBN, "blurb": "Rückseite." })).await);
    let book = baruch_server::typeset::typeset(&server.state, work).await.unwrap();
    let (page_w, page_h) = svg_mm(&book.rendered().pages[0]);
    let pages = book.page_count();
    let wrap = baruch_server::cover::cover_source(&server.state, work).await.unwrap();
    let rendered = baruch_render::compile(&wrap);
    assert!(!rendered.has_errors(), "Umschlag setzt fehlerfrei: {:?}", rendered.diagnostics);
    assert_eq!(rendered.pages.len(), 1, "Rückseite, Rücken und Vorderseite auf einer Seite");
    let (w, h) = svg_mm(&rendered.pages[0]);
    // Defaults: 0.1 mm paper per page, 3 mm bleed. Since the integration the frame is in the book typesetting (front,
    // inside and back of the cover, `cover_pages` on); these three pages do not count towards the book block.
    let block = pages - baruch_typst::cover::COVER_PAGES;
    let expected_w = 2.0 * page_w + block as f64 * 0.1 + 2.0 * 3.0;
    assert!(
        (w - expected_w).abs() < 0.05,
        "Breite {w:.2} mm = 2 × {page_w:.2} + {block} × 0,1 + 2 × 3 = {expected_w:.2} mm"
    );
    assert!((h - (page_h + 6.0)).abs() < 0.05, "Höhe {h:.2} mm = Seitenhöhe {page_h:.2} + 2 × 3 mm Beschnitt");
    assert!(rendered.pages[0].contains("<image"), "Umschlagbild und Strichcode sind eingebettet");

    // Integration: the cover PDF is built through crate::pdf::source_to_pdf.
    let pdf = server.rpc("work.cover_pdf", json!({ "work": work, "inline": true })).await;
    let answer = result(&pdf);
    assert_eq!(answer["pages"], 1, "Umschlag-PDF hat eine Seite: {answer}");
    assert!(answer["file_name"].as_str().unwrap().ends_with("-umschlag.pdf"), "Dateiname: {answer}");
    let spine = answer["spine_mm"].as_f64().unwrap();
    assert!((spine - block as f64 * 0.1).abs() < 1e-6, "Rücken {spine} mm = {block} Seiten × 0,1 mm");
    let bytes = baruch_store::decode_base64(answer["pdf_base64"].as_str().unwrap()).unwrap();
    assert!(bytes.starts_with(b"%PDF-"), "gültiger PDF-Kopf");
    assert_eq!(answer["bytes"].as_u64().unwrap() as usize, bytes.len(), "bytes = Länge der Datei");
    assert_eq!(answer["sha256"].as_str().unwrap(), baruch_render::sha256_hex(&bytes), "sha256 der Datei");
    // The response is bytes (no UTF-8): directly through the router, not through `send`.
    let response = tower::ServiceExt::oneshot(
        server.app.clone(),
        Request::get(format!("/werke/{work}/umschlag.pdf")).body(Body::empty()).unwrap(),
    )
    .await
    .unwrap();
    let (status, headers) = (response.status(), response.headers().clone());
    assert_eq!(status, StatusCode::OK, "Umschlag-PDF lässt sich herunterladen");
    assert_eq!(headers.get(header::CONTENT_TYPE).unwrap(), "application/pdf", "als PDF ausgeliefert");
    assert!(
        headers.get(header::CONTENT_DISPOSITION).unwrap().to_str().unwrap().contains("-umschlag.pdf"),
        "als Anhang mit Dateinamen"
    );
}

#[tokio::test]
async fn rpc_sets_cover_and_book_meta_with_limits_and_commits() {
    let server = common::open().await;
    let (work, slug) = garden(&server).await;
    let unknown = server.rpc("work.set_cover", json!({ "work": work, "name": "gibtsnicht.png" })).await;
    assert_eq!(error(&unknown).0, -32602, "unbekanntes Bild → ungültige Eingabe");
    let name = add_svg(&server, work, "teich.svg", POND, "Teich", "Ein Teich").await;
    let set = server.rpc("work.set_cover", json!({ "work": work, "name": name })).await;
    let set = result(&set);
    assert_eq!(set["cover"]["name"], "teich.svg", "Antwort nennt den Umschlag");
    assert_eq!(set["cover"]["thumb"], format!("/werke/{work}/bilder/teich.svg/vorschau?b=320"), "Vorschaubild-Adresse");
    assert!(set["commit"].is_string(), "Umschlag setzen ergibt einen Commit: {set}");
    assert_eq!(git_log(&server, &slug, "%s").unwrap().trim(), "Umschlag: teich.svg gesetzt", "Betreff");
    let body = git_log(&server, &slug, "%B").unwrap();
    assert!(
        body.contains("Baruch-Action: cover-set") && body.contains("Umschlag nachher: teich.svg"),
        "Trailer und Rumpf: {body}"
    );
    let again = server.rpc("work.set_cover", json!({ "work": work, "name": name })).await;
    assert!(result(&again)["commit"].is_null(), "derselbe Umschlag noch einmal: kein Commit");

    let got = server.rpc("work.get", json!({ "work": work })).await;
    assert_eq!(result(&got)["cover"]["name"], "teich.svg", "work.get trägt den Umschlag");

    let changed = server
        .rpc(
            "work.meta.set",
            json!({ "work": work, "subtitle": "Ein Jahr", "publisher": "Beetverlag", "paper_mm": "0,08" }),
        )
        .await;
    let changed = result(&changed);
    assert_eq!(changed["changed"], json!(["subtitle", "publisher", "paper_mm"]), "geänderte Felder: {changed}");
    assert_eq!(
        git_log(&server, &slug, "%s").unwrap().trim(),
        "Buchangaben geändert: Untertitel, Verlag, Papierdicke",
        "Betreff nennt die Felder"
    );
    let body = git_log(&server, &slug, "%B").unwrap();
    assert!(
        body.contains("Verlag: – → Beetverlag") && body.contains("Papierdicke: 0.1 → 0.08"),
        "vorher/nachher: {body}"
    );
    assert!(body.contains("Baruch-Action: book-meta"), "Trailer: {body}");

    let meta = server.rpc("work.meta.get", json!({ "work": work })).await;
    let meta = result(&meta);
    assert_eq!(meta["meta"]["publisher"], "Beetverlag", "Verlag gespeichert");
    assert_eq!(meta["meta"]["subtitle"], "Ein Jahr", "Untertitel über update_work");
    assert_eq!(meta["meta"]["title"], "Das Gartenbuch", "Titel aus dem Werk");
    assert_eq!(meta["meta"]["paper_mm"], 0.08, "Komma als Dezimalzeichen");
    assert_eq!(meta["cover"]["name"], "teich.svg", "Umschlag dabei");

    for (params, field) in [
        (json!({ "blurb": "x".repeat(4001) }), "Klappentext"),
        (json!({ "isbn": "978-3-16-148410-1" }), "Prüfziffer"),
        (json!({ "isbn": "978-3-16" }), "13 Ziffern"),
        (json!({ "year": "26" }), "Jahr"),
        (json!({ "publisher": "zwei\nZeilen" }), "Verlag"),
        (json!({ "paper_mm": 0 }), "Papierdicke"),
        (json!({ "bleed_mm": "11" }), "Beschnitt"),
        (json!({ "title_area": "links" }), "title_area"),
        (json!({ "author_photo": "fehlt.png" }), "fehlt.png"),
    ] {
        let mut params = params;
        params["work"] = json!(work);
        params["edition"] = json!("darf nicht ankommen");
        let refused = server.rpc("work.meta.set", params).await;
        let (code, message) = error(&refused);
        assert_eq!(code, -32602, "Grenze verletzt → ungültige Eingabe ({field})");
        assert!(message.contains(field), "Meldung nennt {field}: {message}");
    }
    let meta = server.rpc("work.meta.get", json!({ "work": work })).await;
    assert_eq!(
        result(&meta)["meta"]["edition"],
        "",
        "abgelehnte Eingaben ändern nichts, auch nicht die übrigen Felder"
    );
    let ok = server.rpc("work.meta.set", json!({ "work": work, "isbn": "9783161484100", "year": "2026" })).await;
    assert_eq!(result(&ok)["changed"], json!(["year", "isbn"]), "ISBN ohne Bindestriche angenommen");

    let removed = server.rpc("work.set_cover", json!({ "work": work, "name": null })).await;
    assert!(result(&removed)["cover"].is_null(), "Umschlag entfernt");
    assert_eq!(git_log(&server, &slug, "%s").unwrap().trim(), "Umschlag entfernt", "Betreff beim Entfernen");
    assert!(git_log(&server, &slug, "%B").unwrap().contains("Baruch-Action: cover-removed"), "Trailer beim Entfernen");
}

#[tokio::test]
async fn forms_work_without_javascript_and_keep_the_input_on_errors() {
    let server = common::open().await;
    let (work, _) = garden(&server).await;
    let name = add_svg(&server, work, "teich.svg", POND, "Teich", "Ein Teich").await;

    let (status, location) =
        server.post_form("/werke/umschlag/act", &format!("work={work}&action=set&name={name}")).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Umschlag setzen → 303");
    assert_eq!(
        location.as_deref(),
        Some(format!("/einstellungen/werk?work={work}&ok=umschlag").as_str()),
        "Vorgabe: zurück in die Einstellungen"
    );
    let (_, location) = server
        .post_form("/werke/umschlag/act", &format!("work={work}&action=remove&back=%2Fwerke%2F{work}%2Fbilder"))
        .await;
    assert_eq!(
        location.as_deref(),
        Some(format!("/werke/{work}/bilder?hinweis=umschlag_entfernt").as_str()),
        "zurück zu den Bildern mit Hinweis"
    );
    let (_, location) = server
        .post_form(
            "/werke/umschlag/act",
            &format!("work={work}&action=set&name={name}&back=https%3A%2F%2Fboese.example%2F"),
        )
        .await;
    assert_eq!(
        location.as_deref(),
        Some(format!("/einstellungen/werk?work={work}&ok=umschlag").as_str()),
        "eine fremde Adresse als back wird übergangen"
    );
    let (_, location) = server
        .post_form(
            "/werke/umschlag/act",
            &format!("work={work}&action=area&title_area=middle&back=%2Fwerke%2F{work}%2Fumschlag"),
        )
        .await;
    assert_eq!(
        location.as_deref(),
        Some(format!("/werke/{work}/umschlag?hinweis=buchangaben").as_str()),
        "Titelbereich gesetzt"
    );
    let meta = server.rpc("work.meta.get", json!({ "work": work })).await;
    assert_eq!(result(&meta)["meta"]["title_area"], "middle", "Titelbereich Mitte gespeichert");

    let form = format!(
        "work={work}&flags=1&publisher=Beetverlag&paper_mm=0%2C2&bleed_mm=5&front_matter=1&title_area=top&author_photo="
    );
    let (status, location) = server.post_form("/werke/buchangaben/act", &form).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Buchangaben speichern → 303");
    assert_eq!(
        location.as_deref(),
        Some(format!("/einstellungen/werk?work={work}&ok=buchangaben").as_str()),
        "zurück mit ok"
    );
    let meta = server.rpc("work.meta.get", json!({ "work": work })).await;
    let meta = &result(&meta)["meta"];
    assert_eq!((meta["paper_mm"].as_f64(), meta["bleed_mm"].as_f64()), (Some(0.2), Some(5.0)), "Komma und Punkt");
    assert_eq!(
        (meta["front_matter"].as_bool(), meta["cover_pages"].as_bool()),
        (Some(true), Some(false)),
        "fehlendes Häkchen = aus"
    );

    let bad = format!("work={work}&flags=1&publisher=Neuer+Gartenverlag&isbn=123&back=%2Fwerke%2F{work}%2Fumschlag");
    let request = Request::post("/werke/buchangaben/act")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(bad))
        .unwrap();
    let (status, _, page) = server.send(request).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "ungültige ISBN → 422");
    assert!(page.contains("13 Ziffern"), "Meldung steht auf der Seite");
    assert!(page.contains("value=\"Neuer Gartenverlag\""), "die Eingabe bleibt erhalten");
    let meta = server.rpc("work.meta.get", json!({ "work": work })).await;
    assert_eq!(result(&meta)["meta"]["publisher"], "Beetverlag", "nichts gespeichert");

    let settings_bad = format!("work={work}&flags=1&year=zwanzig&back=%2Feinstellungen%2Fwerk%3Fwork%3D{work}");
    let request = Request::post("/werke/buchangaben/act")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(settings_bad))
        .unwrap();
    let (status, _, page) = server.send(request).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "aus den Einstellungen: ebenfalls 422");
    assert!(
        page.contains("data-settings-page-body=\"werk\"") && page.contains("value=\"zwanzig\""),
        "Einstellungsseite mit Eingabe"
    );

    let foreign = Request::post("/werke/umschlag/act")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .header(header::ORIGIN, "https://boese.example")
        .body(Body::from(format!("work={work}&action=remove")))
        .unwrap();
    assert_eq!(server.send(foreign).await.0, StatusCode::FORBIDDEN, "Formular von fremder Seite → 403");
}

#[tokio::test]
async fn pages_show_the_cover_groups_preview_and_pdf_links() {
    let server = common::open().await;
    let (work, _) = garden(&server).await;
    let name = add_svg(&server, work, "teich.svg", POND, "Teich", "Ein Teich").await;
    result(&server.rpc("work.set_cover", json!({ "work": work, "name": name })).await);

    let (status, page) = server.get(&format!("/einstellungen/werk?work={work}")).await;
    assert_eq!(status, StatusCode::OK, "Einstellungsseite Werk");
    for needle in [
        "id=\"buchangaben\"",
        "id=\"umschlag\"",
        "action=\"/werke/buchangaben/act\"",
        "action=\"/werke/umschlag/act\"",
        "action=\"/werke/act\"",
    ] {
        assert!(page.contains(needle), "Einstellungen → Werk zeigt {needle} (bestehendes Werk-Formular bleibt)");
    }
    assert!(
        page.contains("name=\"title_area\" value=\"bottom\"") && page.contains("name=\"front_matter\""),
        "Titelbereich und Titelei"
    );

    let (status, page) = server.get("/werke").await;
    assert_eq!(status, StatusCode::OK, "Werkübersicht");
    assert!(page.contains(&format!("/werke/{work}/bilder/teich.svg/vorschau?b=320")), "Vorschaubild des Umschlags");
    assert!(page.contains("loading=\"lazy\""), "Vorschaubilder laden erst bei Bedarf");
    assert!(page.contains(&format!("href=\"/werke/{work}/pdf\" download")), "Link „PDF“ je Werk");

    let (status, page) = server.get(&format!("/werke/{work}/umschlag")).await;
    assert_eq!(status, StatusCode::OK, "Umschlagseite: {page}");
    assert!(page.contains("id=\"druckpruefung\""), "Druckprüfung");
    assert!(page.contains(&format!("/werke/{work}/umschlag.svg?stand=")), "Vorschau als SVG");
    assert!(page.contains(&format!("/werke/{work}/umschlag.pdf")), "Link zum Umschlag-PDF");
    let (status, headers, svg) =
        server.send(Request::get(format!("/werke/{work}/umschlag.svg")).body(Body::empty()).unwrap()).await;
    assert_eq!(status, StatusCode::OK, "Vorschau-SVG");
    assert_eq!(headers.get(header::CONTENT_TYPE).unwrap(), "image/svg+xml", "als SVG");
    assert!(
        headers.get(header::CONTENT_SECURITY_POLICY).unwrap().to_str().unwrap().contains("sandbox"),
        "ohne Skripte"
    );
    assert!(svg.starts_with("<svg"), "eine SVG-Seite");

    // minijinja escapes „/“ in values as &#x2f; (the browser reads it as „/“).
    let panel = baruch_server::i18n::scope(
        baruch_server::i18n::Lang::De,
        baruch_server::cover::assets_panel(&server.state, work),
    )
    .await
    .unwrap()
    .replace("&#x2f;", "/");
    assert!(
        panel.contains("id=\"umschlag\"") && panel.contains(&format!("value=\"/werke/{work}/bilder\"")),
        "Block für die Seite „Bilder“"
    );
    assert!(panel.contains("Umschlag entfernen"), "mit Umschlag: Entfernen");
    let (status, _) = server.get("/werke/9999/umschlag").await;
    assert_eq!(status, StatusCode::NOT_FOUND, "unbekanntes Werk → 404");
}
