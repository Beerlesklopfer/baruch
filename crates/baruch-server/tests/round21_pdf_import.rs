// SPDX-License-Identifier: AGPL-3.0-or-later
//! Runde 21a (pdf-import): PDF/EPUB-Import mit Vorschau der Gliederung (RPC `source_import.*`, Seiten unter
//! `/werke/import/quelle`, CLI), MultiGPT nur über den Nachbau. Tests schreibt der Besitzer (Vertrag R21, S1 legt nur
//! die Datei an).
//!
//! Alle Dokumente sind frei erfunden und entstehen im Test: PDF aus Typst über `baruch_render::compile_pdf` (unter
//! `common::PDF_SERIAL`), EPUB von Hand gezippt, PNG über `baruch_epub::sample::png`. MultiGPT ist der Nachbau
//! `connections::multigpt::mock::MockMultigpt` mit `full_tools()` auf 127.0.0.1:0; nie Eliezer, nie Internet.
//! Der lokale PDF-Weg braucht Poppler (`pdftotext`, `pdfinfo`); fehlt es auf dem Rechner, melden die betroffenen
//! Tests „ÜBERSPRUNGEN“ (die Meldung ohne pdftotext prüft ein eigener Test mit nicht vorhandenem Programm).

use std::io::Write;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use baruch_server::connections::multigpt::mock::{MockMultigpt, full_tools};
use baruch_server::pdf_import::{self, CliSourceImport, PdfTools};
use serde_json::{Value, json};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

use crate::common;

// ---- Hilfen ------------------------------------------------------------------------------------------------------

async fn ok(server: &common::Server, method: &str, params: Value) -> Value {
    let answer = server.rpc(method, params).await;
    assert!(answer["error"].is_null(), "{method} sollte gelingen: {answer}");
    answer["result"].clone()
}

async fn fails(server: &common::Server, method: &str, params: Value) -> Value {
    let answer = server.rpc(method, params).await;
    assert!(answer["error"].is_object(), "{method} sollte scheitern: {answer}");
    answer["error"].clone()
}

async fn works(server: &common::Server) -> usize {
    server.state.store.work_list().await.unwrap().len()
}

async fn titles(server: &common::Server) -> usize {
    baruch_library::list_titles(server.state.store.pool()).await.unwrap().len()
}

/// Ist Poppler da? Sonst überspringen die Tests des lokalen PDF-Wegs (mit Meldung).
fn poppler() -> bool {
    let found = ["pdftotext", "pdfinfo"].iter().all(|p| {
        std::process::Command::new(p).arg("-v").output().is_ok_and(|o| o.status.success() || !o.stderr.is_empty())
    });
    if !found {
        eprintln!("ÜBERSPRUNGEN: pdftotext/pdfinfo (Poppler) fehlen auf diesem Rechner");
    }
    found
}

/// Frei erfundenes Buch mit drei Kapiteln und Unterabschnitten (Typst-Vorgabe: Ebene 1 größer als Ebene 2).
const KLIFF: &str = "#set document(title: \"Das Leuchtfeuer am Kliff\", author: \"Hanne Sturm\")
#set text(lang: \"de\")

= Ankunft am Kliff

Die Fähre legt am frühen Morgen an. Hanne trägt ihren Koffer den schmalen Pfad hinauf, vorbei an den Netzen, die zum Trocknen hängen. Oben wartet der alte Leuchtturm, weiß und still, als hätte er schon lange auf sie gewartet.

Die Miete beträgt 3 \\$ am Tag und Zimmer \\#7 liegt ganz oben.

== Der erste Abend

Am ersten Abend zählt sie die Stufen bis zur Laterne. Es sind einhundertzwölf, und jede knarrt ein wenig anders. Draußen zieht Nebel vom Wasser herauf und legt sich über die Klippen.

= Der Sturm

In der dritten Woche kommt der Sturm. Die Fenster zittern, und das Meer wirft Gischt bis an die Mauer. Hanne hält die Lampe in Gang, Stunde um Stunde, bis der Morgen grau über den Horizont kriecht.

== Nach dem Sturm

Danach liegt Treibholz am Strand, und die Möwen streiten sich um das, was die Wellen gebracht haben. Sie sammelt Muscheln und schreibt alles in ihr Buch.

= Abschied

Als der Herbst kommt, packt sie den Koffer wieder. Der Leuchtturm bleibt, die Fähre wartet, und das Licht dreht sich weiter über dem dunklen Wasser.
";

/// PDF aus Typst (unter PDF_SERIAL, wie jedes PDF der Server-Tests).
async fn pdf_of(typst: &str) -> Vec<u8> {
    let _serial = common::PDF_SERIAL.lock().await;
    let source = baruch_render::Source { main: typst.to_owned(), files: Vec::new() };
    let pdf = tokio::task::spawn_blocking(move || baruch_render::compile_pdf(&source)).await.unwrap();
    pdf.bytes.unwrap_or_else(|| panic!("Test-PDF lässt sich nicht setzen: {:?}", pdf.diagnostics))
}

/// Überschriften (Ebene, Titel) eines Werks über den Export.
async fn outline_of(server: &common::Server, work: i64) -> Vec<(u8, String)> {
    let typst = server.state.store.export_typst(work).await.unwrap();
    baruch_typst::parse(&typst).unwrap().headings.into_iter().map(|h| (h.level, h.title)).collect()
}

fn zip_of(entries: &[(&str, Vec<u8>)]) -> Vec<u8> {
    let mut zip = ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for (name, bytes) in entries {
        let method = if *name == "mimetype" { CompressionMethod::Stored } else { CompressionMethod::Deflated };
        zip.start_file(*name, SimpleFileOptions::default().compression_method(method)).unwrap();
        zip.write_all(bytes).unwrap();
    }
    zip.finish().unwrap().into_inner()
}

fn xml_escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// Frei erfundenes EPUB 3: nav.xhtml mit zwei Kapiteln (das erste mit Unterabschnitt), Fußnote, Hervorhebungen,
/// PNG-Bild mit Unterschrift, Zeichen, die Typst maskieren muss.
fn epub_entries(title: &str) -> Vec<(&'static str, Vec<u8>)> {
    let container = r#"<?xml version="1.0"?><container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container"><rootfiles><rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/></rootfiles></container>"#;
    let opf = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="id"><metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:identifier id="id">urn:uuid:7d0c2a4e-0000-4000-8000-000000000021</dc:identifier><dc:title>{}</dc:title><dc:creator>Wenke Brise</dc:creator><dc:language>de</dc:language></metadata><manifest><item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/><item id="k1" href="text/kap1.xhtml" media-type="application/xhtml+xml"/><item id="k2" href="text/kap2.xhtml" media-type="application/xhtml+xml"/><item id="img" href="bilder/boje.png" media-type="image/png"/></manifest><spine><itemref idref="nav" linear="no"/><itemref idref="k1"/><itemref idref="k2"/></spine></package>"#,
        xml_escape(title)
    );
    let nav = r#"<?xml version="1.0" encoding="UTF-8"?><html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops"><head><title>Inhalt</title></head><body><nav epub:type="toc"><h1>Inhalt</h1><ol><li><a href="text/kap1.xhtml">Die Boje</a><ol><li><a href="text/kap1.xhtml#netz">Das Netz</a></li></ol></li><li><a href="text/kap2.xhtml#start">Heimkehr</a></li></ol></nav></body></html>"#;
    let kap1 = r##"<?xml version="1.0" encoding="UTF-8"?><html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops"><head><title>Die Boje</title></head><body><section><h1>Die Boje</h1><p>Die rote Boje schaukelt <em>leise</em> im&nbsp;Wind.<a epub:type="noteref" href="#fn1">1</a> Sie kostet 3 $ und trägt die #7.</p><figure><img src="../bilder/boje.png" alt="Eine rote Boje"/><figcaption>Die Boje bei Ebbe</figcaption></figure><h2 id="netz">Das Netz</h2><p>Das Netz hängt <strong>schwer</strong> am Steg.</p><aside epub:type="footnote" id="fn1"><p>Gezählt am Morgen.</p></aside></section></body></html>"##;
    let kap2 = r#"<?xml version="1.0" encoding="UTF-8"?><html xmlns="http://www.w3.org/1999/xhtml"><head><title>Heimkehr</title></head><body><h1 id="start">Heimkehr</h1><p>Am Abend kehren die Boote heim.</p></body></html>"#;
    vec![
        ("mimetype", b"application/epub+zip".to_vec()),
        ("META-INF/container.xml", container.as_bytes().to_vec()),
        ("OEBPS/content.opf", opf.into_bytes()),
        ("OEBPS/nav.xhtml", nav.as_bytes().to_vec()),
        ("OEBPS/text/kap1.xhtml", kap1.as_bytes().to_vec()),
        ("OEBPS/text/kap2.xhtml", kap2.as_bytes().to_vec()),
        ("OEBPS/bilder/boje.png", baruch_epub::sample::png(4, 3, [200, 30, 30])),
    ]
}

fn epub(title: &str) -> Vec<u8> {
    zip_of(&epub_entries(title))
}

async fn preview(server: &common::Server, file_name: &str, bytes: &[u8], extra: Value) -> Value {
    let mut params = json!({ "file_name": file_name, "content": baruch_store::encode_base64(bytes) });
    if let (Some(map), Some(extra)) = (params.as_object_mut(), extra.as_object()) {
        map.extend(extra.clone());
    }
    server.rpc("source_import.preview", params).await
}

fn outline_pairs(preview: &Value) -> Vec<(u64, String)> {
    preview["outline"]
        .as_array()
        .unwrap()
        .iter()
        .map(|o| (o["level"].as_u64().unwrap(), o["title"].as_str().unwrap().to_owned()))
        .collect()
}

/// multipart/form-data mit Textfeldern und höchstens einer Datei.
fn multipart(fields: &[(&str, &str)], file: Option<(&str, &[u8])>) -> (String, Vec<u8>) {
    let boundary = "----baruch-r21a-grenze".to_owned();
    let mut body = Vec::new();
    for (name, value) in fields {
        body.extend_from_slice(
            format!("--{boundary}\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n{value}\r\n").as_bytes(),
        );
    }
    if let Some((name, bytes)) = file {
        body.extend_from_slice(
            format!("--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{name}\"\r\nContent-Type: application/octet-stream\r\n\r\n").as_bytes(),
        );
        body.extend_from_slice(bytes);
        body.extend_from_slice(b"\r\n");
    }
    body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
    (format!("multipart/form-data; boundary={boundary}"), body)
}

async fn post_multipart(
    server: &common::Server,
    uri: &str,
    fields: &[(&str, &str)],
    file: Option<(&str, &[u8])>,
) -> (StatusCode, axum::http::HeaderMap, String) {
    let (content_type, body) = multipart(fields, file);
    let request = Request::post(uri).header(header::CONTENT_TYPE, content_type).body(Body::from(body)).unwrap();
    server.send(request).await
}

async fn post_form(
    server: &common::Server,
    uri: &str,
    pairs: &[(&str, &str)],
) -> (StatusCode, axum::http::HeaderMap, String) {
    let body = pairs
        .iter()
        .map(|(k, v)| format!("{}={}", baruch_server::view::query_escape(k), baruch_server::view::query_escape(v)))
        .collect::<Vec<_>>()
        .join("&");
    let request = Request::post(uri)
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(body))
        .unwrap();
    server.send(request).await
}

// ---- PDF lokal ---------------------------------------------------------------------------------------------------

#[tokio::test]
async fn a_local_pdf_preview_proposes_three_chapters_in_order_and_creates_nothing_before_apply() {
    if !poppler() {
        return;
    }
    let server = common::open().await;
    let pdf = pdf_of(KLIFF).await;
    let (works_before, titles_before) = (works(&server).await, titles(&server).await);
    let answer = preview(&server, "kliff.pdf", &pdf, json!({ "way": "local" })).await;
    let shown = answer["result"].clone();
    assert!(answer["error"].is_null(), "Vorschau gelingt: {answer}");
    assert_eq!(shown["way"], "local", "lokal gewählt, lokal gelesen: {shown}");
    assert_eq!(shown["kind"], "pdf");
    assert!(shown["way_reason"].as_str().unwrap().contains("pdftotext"), "Grund nennt das Werkzeug: {shown}");
    let chapters: Vec<String> = outline_pairs(&shown).into_iter().filter(|(l, _)| *l == 1).map(|(_, t)| t).collect();
    assert_eq!(
        chapters,
        ["Ankunft am Kliff", "Der Sturm", "Abschied"],
        "Begründung: die drei Kapitel stehen in ihrer Reihenfolge auf Ebene 1 (größte Schrift): {shown}"
    );
    let subsections: Vec<String> = outline_pairs(&shown).into_iter().filter(|(l, _)| *l == 2).map(|(_, t)| t).collect();
    assert_eq!(subsections, ["Der erste Abend", "Nach dem Sturm"], "Begründung: Unterabschnitte auf Ebene 2: {shown}");
    assert_eq!(
        shown["title"], "Das Leuchtfeuer am Kliff",
        "Begründung: Titel aus den Angaben der PDF (pdfinfo): {shown}"
    );
    assert_eq!(shown["author"], "Hanne Sturm", "Begründung: Autor aus den Angaben der PDF: {shown}");
    let first = &shown["outline"][0];
    assert!(
        first["sample"].as_str().unwrap().starts_with("Die Fähre legt"),
        "Begründung: die Textprobe zeigt den Anfang des Kapitels: {first}"
    );
    assert!(first["chars"].as_u64().unwrap() > 100, "Begründung: Zeichenzahl des Kapitels: {first}");
    assert!(
        shown["warnings"].as_array().unwrap().iter().any(|w| w["code"] == "import-pdf-plain"),
        "Begründung: der lokale Weg sagt ehrlich, was er nicht erkennt: {shown}"
    );
    assert_eq!(works(&server).await, works_before, "Begründung: die Vorschau legt kein Werk an");
    assert_eq!(titles(&server).await, titles_before, "Begründung: die Vorschau legt keinen Titel an");
    assert_eq!(pdf_import::draft_count(&server.state), 1, "Begründung: nur ein Entwurf entsteht");
    assert_eq!(pdf_import::local_pdf_reads(&server.state), 1, "Begründung: genau ein lokaler Lesevorgang");
    let again = ok(&server, "source_import.get", json!({ "draft": shown["draft"] })).await;
    assert_eq!(again["outline"], shown["outline"], "Begründung: get zeigt denselben Entwurf");

    // Bestätigen mit bearbeiteter Gliederung: umbenannt, Ebene geändert, „Nach dem Sturm“ gestrichen.
    let outline: Vec<Value> = shown["outline"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|o| o["title"] != "Nach dem Sturm")
        .map(|o| {
            let title = o["title"].as_str().unwrap();
            let (level, title) = match title {
                "Ankunft am Kliff" => (1, "Die Ankunft"),
                "Der erste Abend" => (3, "Der erste Abend"),
                other => (o["level"].as_u64().unwrap(), other),
            };
            json!({ "level": level, "title": title, "source": o["index"] })
        })
        .collect();
    let applied = ok(&server, "source_import.apply", json!({ "draft": shown["draft"], "outline": outline })).await;
    let work = applied["report"]["work"].as_i64().unwrap_or_else(|| panic!("Werk angelegt: {applied}"));
    assert!(applied.get("library_title").is_none(), "Begründung: Ziel Werk (Vorgabe) legt keinen Titel an: {applied}");
    let headings = outline_of(&server, work).await;
    let expected: Vec<(u8, String)> = [(1, "Die Ankunft"), (3, "Der erste Abend"), (1, "Der Sturm"), (1, "Abschied")]
        .iter()
        .map(|(l, t)| (*l, (*t).to_owned()))
        .collect();
    assert_eq!(headings, expected, "Begründung: genau die bestätigte Gliederung, nichts still angelegt");
    let typst = server.state.store.export_typst(work).await.unwrap();
    assert!(
        typst.contains("Treibholz"),
        "Begründung: der Text des gestrichenen Punkts bleibt (beim vorigen Punkt): {typst}"
    );
    assert!(
        typst.contains("3 \\$") && typst.contains("\\#7"),
        "Begründung: Zeichen aus der PDF sind für Typst maskiert: {typst}"
    );
    assert!(
        applied["report"]["warnings"].as_array().unwrap().iter().any(|w| w["code"] == "import-outline-merged"),
        "Begründung: das Zusammenlegen steht im Bericht: {applied}"
    );
    assert_eq!(pdf_import::draft_count(&server.state), 0, "Begründung: nach dem Übernehmen ist der Entwurf weg");
    let gone = fails(&server, "source_import.get", json!({ "draft": shown["draft"] })).await;
    assert_eq!(gone["code"], -32602, "Begründung: ein übernommener Entwurf ist unbekannt: {gone}");
}

// ---- PDF über MultiGPT -------------------------------------------------------------------------------------------

/// Nachbau mit allen Werkzeugen; das hochgeladene Dokument hat zwölf Seiten und ein Inhaltsverzeichnis.
async fn multigpt(server: &common::Server) -> MockMultigpt {
    let mock = MockMultigpt::start().await;
    mock.set_tools(full_tools());
    let mut pages: Vec<String> = Vec::new();
    pages.push("Ankunft am Kliff\n\nDie Fähre legt am frühen Morgen an.".into());
    pages.push("1.1 Der erste Abend\n\nAm ersten Abend zählt sie die Stufen.".into());
    for n in 3..=10 {
        pages.push(format!("Nebel über den Klippen, Seite {n}."));
    }
    pages.push("Der Sturm\n\nIn der dritten Woche kommt der Sturm.".into());
    pages.push("Abschied\n\nAls der Herbst kommt, packt sie den Koffer.".into());
    mock.configure(|b| {
        b.upload_pages = pages;
        b.upload_toc = vec![
            json!({ "section": "1", "title": "Ankunft am Kliff", "page": 1 }),
            json!({ "section": "1.1", "title": "Der erste Abend", "page": 2 }),
            json!({ "section": "2", "title": "Der Sturm", "page": 11 }),
            json!({ "section": "3", "title": "Abschied", "page": 12 }),
        ];
        b.upload_title = Some("Das Leuchtfeuer am Kliff".into());
        b.upload_author = Some("Hanne Sturm".into());
        b.index_polls = 1;
    });
    ok(server, "connection.set", json!({ "kind": "multigpt", "config": { "url": mock.url() } })).await;
    mock
}

#[tokio::test]
async fn the_multigpt_way_uploads_once_reads_page_ranges_and_takes_the_outline_from_the_toc() {
    let server = common::open().await;
    let mock = multigpt(&server).await;
    let pdf = pdf_of(KLIFF).await;
    let works_before = works(&server).await;
    let shown = preview(&server, "kliff.pdf", &pdf, json!({ "way": "auto", "collection": "Kliff" })).await;
    assert!(shown["error"].is_null(), "Vorschau über MultiGPT gelingt: {shown}");
    let shown = shown["result"].clone();
    assert_eq!(shown["way"], "multigpt", "Begründung: auto mit Verbindung und Sammlung wählt MultiGPT: {shown}");
    let uploads = mock.tool_calls("upload_document");
    assert_eq!(uploads.len(), 1, "Begründung: genau ein Hochladen (Nachbau-Protokoll): {uploads:?}");
    assert_eq!(uploads[0]["collection"], "Kliff", "Begründung: in die gewählte Sammlung");
    assert_eq!(uploads[0]["filename"], "kliff.pdf", "Begründung: mit Dateinamen");
    assert_eq!(
        baruch_store::decode_base64(uploads[0]["content_base64"].as_str().unwrap()).unwrap(),
        pdf,
        "Begründung: content_base64 ist genau die PDF"
    );
    assert!(!mock.tool_calls("run_status").is_empty(), "Begründung: Baruch wartet über run_status auf die Indexierung");
    let reads = mock.tool_calls("read_document");
    let ranges: Vec<(u64, u64)> =
        reads.iter().filter_map(|a| Some((a.get("page_from")?.as_u64()?, a.get("page_to")?.as_u64()?))).collect();
    assert!(
        ranges.contains(&(1, 10)) && ranges.contains(&(11, 12)),
        "Begründung: seitenweise in Bereichen bis zehn Seiten: {reads:?}"
    );
    assert!(ranges.iter().all(|(f, t)| t - f < 10), "Begründung: nie mehr als zehn Seiten je Aufruf: {ranges:?}");
    assert_eq!(
        outline_pairs(&shown),
        [
            (1, "Ankunft am Kliff".to_owned()),
            (2, "Der erste Abend".into()),
            (1, "Der Sturm".into()),
            (1, "Abschied".into())
        ],
        "Begründung: Gliederung aus dem Inhaltsverzeichnis von document_info (Ebenen aus den Nummern): {shown}"
    );
    assert_eq!(shown["document"]["collection"], "Kliff", "Begründung: das Dokument in MultiGPT ist genannt: {shown}");
    assert_eq!(shown["document"]["id"], "up-1");
    assert!(
        shown["outline"][2]["sample"].as_str().unwrap().contains("dritten Woche"),
        "Textprobe aus read_document: {shown}"
    );
    assert_eq!(pdf_import::local_pdf_reads(&server.state), 0, "Begründung: über MultiGPT läuft kein pdftotext");
    assert_eq!(works(&server).await, works_before, "Begründung: vor apply kein Werk");

    // Beides: Werk und Quelle mit Verknüpfung.
    let applied = ok(&server, "source_import.apply", json!({ "draft": shown["draft"], "target": "both" })).await;
    let id = applied["library_title"].as_i64().unwrap_or_else(|| panic!("Titel angelegt: {applied}"));
    let pool = server.state.store.pool();
    let remote = baruch_library::title_remote(pool, id).await.unwrap().expect("Verknüpfung mit MultiGPT");
    assert_eq!(
        (remote.collection.as_str(), remote.document.as_str()),
        ("Kliff", "up-1"),
        "Begründung: Weg MultiGPT verknüpft den Titel"
    );
    assert!(
        baruch_library::title_text(pool, id).await.unwrap().is_none(),
        "Begründung: über MultiGPT kein lokaler Volltext"
    );
    let entry = baruch_library::get_title(pool, id).await.unwrap();
    assert_eq!(
        (entry.title.title.as_str(), entry.title.authors.as_str()),
        ("Das Leuchtfeuer am Kliff", "Hanne Sturm"),
        "Begründung: Titel und Autor aus den Angaben von document_info"
    );
    let work = applied["report"]["work"].as_i64().expect("Werk angelegt");
    assert_eq!(outline_of(&server, work).await.len(), 4, "Begründung: das Werk hat die vier Punkte der Gliederung");
    assert_eq!(mock.tool_calls("upload_document").len(), 1, "Begründung: apply lädt nichts erneut hoch");
}

#[tokio::test]
async fn a_multigpt_failure_is_reported_with_an_offer_to_read_locally_and_never_switches_by_itself() {
    let server = common::open().await;
    let mock = multigpt(&server).await;
    mock.configure(|b| b.upload_error = Some("Speicher der Sammlung voll".into()));
    let pdf = pdf_of(KLIFF).await;
    let works_before = works(&server).await;
    let error = fails(&server, "source_import.preview", json!({
        "file_name": "kliff.pdf", "content": baruch_store::encode_base64(&pdf), "way": "multigpt", "collection": "Kliff" })).await;
    let message = error["message"].as_str().unwrap();
    assert_eq!(error["code"], -32000, "Begründung: fachlicher Fehler der Gegenstelle: {error}");
    assert!(message.contains("Speicher der Sammlung voll"), "Begründung: der Grund von MultiGPT steht da: {message}");
    assert!(message.contains("Weg multigpt"), "Begründung: der Weg bleibt MultiGPT: {message}");
    assert!(
        message.contains("lokal") && message.contains("\"local\"") || message.contains("„local“"),
        "Begründung: Angebot „lokal lesen“: {message}"
    );
    assert_eq!(
        pdf_import::local_pdf_reads(&server.state),
        0,
        "Begründung: kein stiller Wechsel – kein pdftotext-Aufruf"
    );
    assert_eq!(works(&server).await, works_before, "Begründung: nichts angelegt");

    // Formular: 422 mit Meldung und „Lokal lesen“, die Datei liegt bereit.
    let (status, _, page) = post_multipart(
        &server,
        "/werke/import/quelle/vorschau",
        &[("way", "multigpt"), ("collection", "Kliff"), ("upload", "1"), ("title", "")],
        Some(("kliff.pdf", &pdf)),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "Begründung: Fehler der Vorschau → 422: {page}");
    assert!(page.contains("Speicher der Sammlung voll"), "Meldung auf der Seite");
    assert!(
        page.contains("data-retry-local") && page.contains("Lokal lesen"),
        "Begründung: Angebot als Knopf ohne JavaScript: {page}"
    );
    assert!(page.contains("value=\"multigpt\" checked"), "Begründung: der gewählte Weg bleibt gewählt: {page}");
    assert_eq!(pdf_import::local_pdf_reads(&server.state), 0, "Begründung: auch das Formular wechselt nicht still");
    if !poppler() {
        return;
    }
    let draft = page
        .split("name=\"draft\" value=\"")
        .nth(1)
        .and_then(|r| r.split('"').next())
        .expect("Entwurf im Formular")
        .to_owned();
    let (status, _, page) = post_multipart(
        &server,
        "/werke/import/quelle/vorschau",
        &[("draft", &draft), ("way", "local"), ("title", "")],
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "Begründung: „Lokal lesen“ liest die abgelegte Datei ohne neues Wählen: {page}");
    assert!(page.contains("data-way=\"local\""), "Begründung: erst auf Wunsch des Menschen lokal: {page}");
    assert_eq!(pdf_import::local_pdf_reads(&server.state), 1, "Begründung: jetzt genau ein lokaler Lesevorgang");
}

// ---- EPUB --------------------------------------------------------------------------------------------------------

#[tokio::test]
async fn an_epub_with_navigation_footnote_and_image_becomes_a_work_with_both_after_apply() {
    let server = common::open().await;
    let shown = ok(
        &server,
        "source_import.preview",
        json!({
        "file_name": "boje.epub", "content": baruch_store::encode_base64(&epub("Die rote Boje")), "way": "multigpt" }),
    )
    .await;
    assert_eq!(shown["kind"], "epub");
    assert_eq!(shown["way"], "local", "Begründung: EPUB immer lokal, auch wenn MultiGPT gewählt ist: {shown}");
    assert!(shown["way_reason"].as_str().unwrap().contains("kein EPUB"), "Begründung: der Grund steht da: {shown}");
    assert_eq!(
        outline_pairs(&shown),
        [(1, "Die Boje".to_owned()), (2, "Das Netz".into()), (1, "Heimkehr".into())],
        "Begründung: Gliederung aus nav.xhtml (zwei Kapitel, eines mit Unterabschnitt): {shown}"
    );
    assert_eq!(shown["title"], "Die rote Boje", "Titel aus dem OPF");
    assert_eq!(shown["author"], "Wenke Brise", "Autor aus dem OPF");
    assert_eq!(shown["assets"], json!(["bilder/boje.png"]), "Begründung: das Bild kommt mit: {shown}");
    let applied = ok(&server, "source_import.apply", json!({ "draft": shown["draft"] })).await;
    let work = applied["report"]["work"].as_i64().unwrap();
    let headings = outline_of(&server, work).await;
    assert_eq!(headings.iter().filter(|(l, _)| *l == 1).count(), 2, "Begründung: zwei Kapitel: {headings:?}");
    let typst = server.state.store.export_typst(work).await.unwrap();
    assert!(typst.contains("#footnote[Gezählt am Morgen.]"), "Begründung: Fußnote als #footnote: {typst}");
    assert!(typst.contains("_leise_") && typst.contains("*schwer*"), "Begründung: Hervorhebungen als _/*: {typst}");
    assert!(
        typst.contains("image(\"bilder/boje.png\", alt: \"Eine rote Boje\")"),
        "Begründung: Bild mit Pfad im Text: {typst}"
    );
    assert!(typst.contains("caption: [Die Boje bei Ebbe]"), "Begründung: Unterschrift bleibt: {typst}");
    assert!(typst.contains("3 \\$") && typst.contains("\\#7"), "Begründung: Typst-Zeichen maskiert: {typst}");
    assert!(
        typst.matches("= Die Boje").count() == 1
            && !typst.contains("#strong[Die Boje]")
            && !typst.contains("\nDie Boje\n"),
        "Begründung: die Überschrift steht nicht doppelt im Text: {typst}"
    );
    let assets = server.state.store.export_assets(work).await.unwrap();
    assert_eq!(assets.len(), 1, "Begründung: ein Bild im Werk");
    assert_eq!(assets[0].path, "bilder/boje.png");
    assert_eq!(assets[0].bytes, baruch_epub::sample::png(4, 3, [200, 30, 30]), "Begründung: Bytes unverändert");
}

#[tokio::test]
async fn unsafe_or_oversized_epubs_are_refused_without_creating_anything() {
    let server = common::open().await;
    let works_before = works(&server).await;
    let mut escape = epub_entries("Flucht");
    escape.push(("../x.xhtml", b"<p>draussen</p>".to_vec()));
    let mut symlink = ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for (name, bytes) in epub_entries("Verweis") {
        symlink.start_file(name, SimpleFileOptions::default()).unwrap();
        symlink.write_all(&bytes).unwrap();
    }
    symlink.add_symlink("OEBPS/bilder/link.png", "/etc/passwd", SimpleFileOptions::default()).unwrap();
    let symlink = symlink.finish().unwrap().into_inner();
    let mut big = ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for (name, bytes) in epub_entries("Groß") {
        big.start_file(name, SimpleFileOptions::default()).unwrap();
        big.write_all(&bytes).unwrap();
    }
    big.start_file(
        "OEBPS/bilder/riesig.png",
        SimpleFileOptions::default().compression_method(CompressionMethod::Stored),
    )
    .unwrap();
    let mut noise = vec![0u8; 17 * 1024 * 1024];
    for (i, b) in noise.iter_mut().enumerate() {
        *b = (i.wrapping_mul(2_654_435_761) >> 13) as u8;
    }
    big.write_all(&noise).unwrap();
    let big = big.finish().unwrap().into_inner();
    let mut bomb = epub_entries("Bombe");
    bomb.push(("OEBPS/text/nullen.xhtml", vec![b' '; 8 * 1024 * 1024]));
    let cases = [
        ("Pfadflucht", zip_of(&escape), ".."),
        ("Symlink", symlink, "link.png"),
        ("zu großer Eintrag", big, "riesig.png"),
        ("Bombenverhältnis", zip_of(&bomb), "nullen.xhtml"),
    ];
    for (what, bytes, name) in cases {
        let error = fails(
            &server,
            "source_import.preview",
            json!({
            "file_name": "boese.epub", "content": baruch_store::encode_base64(&bytes) }),
        )
        .await;
        assert_eq!(error["code"], -32602, "Begründung: {what} wird abgelehnt: {error}");
        let message = error["message"].as_str().unwrap();
        assert!(
            message.contains("EPUB abgelehnt") && message.contains(name),
            "Begründung: {what} – Meldung nennt den Eintrag: {message}"
        );
    }
    assert_eq!(works(&server).await, works_before, "Begründung: nichts angelegt");
    assert_eq!(pdf_import::draft_count(&server.state), 0, "Begründung: auch kein Entwurf");
}

// ---- Entwürfe, Titel ---------------------------------------------------------------------------------------------

#[tokio::test]
async fn discard_removes_a_draft_and_old_or_surplus_drafts_are_cleaned_on_the_next_preview() {
    let server = common::open().await;
    let shown = ok(
        &server,
        "source_import.preview",
        json!({
        "file_name": "boje.epub", "content": baruch_store::encode_base64(&epub("Die rote Boje")) }),
    )
    .await;
    let discarded = ok(&server, "source_import.discard", json!({ "draft": shown["draft"] })).await;
    assert_eq!(discarded["discarded"], true, "Begründung: es gab den Entwurf");
    let again = ok(&server, "source_import.discard", json!({ "draft": shown["draft"] })).await;
    assert_eq!(again["discarded"], false, "Begründung: ein zweites Verwerfen findet nichts");
    let bad = fails(&server, "source_import.discard", json!({ "draft": "../../baruch.sqlite" })).await;
    assert_eq!(bad["code"], -32602, "Begründung: kein Pfad als Name eines Entwurfs: {bad}");
    let error = fails(&server, "source_import.apply", json!({ "draft": shown["draft"] })).await;
    assert_eq!(error["code"], -32602, "Begründung: ein verworfener Entwurf lässt sich nicht übernehmen: {error}");

    // Alt (25 h) und zu viele: beim nächsten preview aufgeräumt.
    let root = server.path().join("import-drafts");
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
    let old = format!("{:032x}", 1);
    std::fs::create_dir_all(root.join(&old)).unwrap();
    std::fs::write(root.join(&old).join("draft.json"), json!({ "created": now - 25 * 3600 }).to_string()).unwrap();
    for n in 0..20u64 {
        let dir = root.join(format!("{:032x}", 100 + n));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("draft.json"), json!({ "created": now - 3600 + n }).to_string()).unwrap();
    }
    assert_eq!(pdf_import::draft_count(&server.state), 21, "vorher 21 Entwürfe (einer alt)");
    ok(
        &server,
        "source_import.preview",
        json!({
        "file_name": "boje.epub", "content": baruch_store::encode_base64(&epub("Die rote Boje")) }),
    )
    .await;
    assert!(!root.join(&old).exists(), "Begründung: älter als 24 h → entfernt");
    assert!(!root.join(format!("{:032x}", 100)).exists(), "Begründung: über 20 → der älteste weg");
    assert_eq!(pdf_import::draft_count(&server.state), 20, "Begründung: höchstens 20 Entwürfe samt dem neuen");
}

#[tokio::test]
async fn a_taken_title_is_refused_with_a_hint_and_the_draft_stays() {
    let server = common::open().await;
    server.import("#set document(title: \"Die rote Boje\")\n= Schon da\n\nText.\n", "boje.typ").await;
    let works_before = works(&server).await;
    let shown = ok(
        &server,
        "source_import.preview",
        json!({
        "file_name": "boje.epub", "content": baruch_store::encode_base64(&epub("Die rote Boje")) }),
    )
    .await;
    let error = fails(&server, "source_import.apply", json!({ "draft": shown["draft"] })).await;
    assert_eq!(error["code"], -32000, "Begründung: vergebener Titel → REFUSED: {error}");
    assert!(
        error["message"].as_str().unwrap().contains(baruch_server::import::TITLE_HINT),
        "Begründung: mit Hinweis: {error}"
    );
    assert_eq!(works(&server).await, works_before, "Begründung: kein zweites Werk");
    assert_eq!(pdf_import::draft_count(&server.state), 1, "Begründung: der Entwurf bleibt für einen neuen Versuch");
    let applied =
        ok(&server, "source_import.apply", json!({ "draft": shown["draft"], "title": "Die rote Boje (EPUB)" })).await;
    assert_eq!(applied["report"]["title"], "Die rote Boje (EPUB)", "Begründung: mit anderem Titel gelingt es");
}

#[tokio::test]
async fn parts_are_created_only_when_the_human_confirms_them() {
    let server = common::open().await;
    let shown = ok(
        &server,
        "source_import.preview",
        json!({
        "file_name": "boje.epub", "content": baruch_store::encode_base64(&epub("Teile am Steg")) }),
    )
    .await;
    let mut outline = vec![json!({ "level": 1, "title": "Erster Teil", "part": true })];
    for o in shown["outline"].as_array().unwrap() {
        outline.push(json!({ "level": o["level"], "title": o["title"], "source": o["index"] }));
    }
    let applied = ok(&server, "source_import.apply", json!({ "draft": shown["draft"], "outline": outline })).await;
    let work = applied["report"]["work"].as_i64().unwrap();
    let headings = outline_of(&server, work).await;
    assert_eq!(
        headings[0],
        (0, "Erster Teil".to_owned()),
        "Begründung: der bestätigte Teil wird ein Teil: {headings:?}"
    );
    assert!(headings[1..].iter().all(|(l, _)| *l > 0), "Begründung: sonst keine Teile: {headings:?}");
}

// ---- Bibliothek ------------------------------------------------------------------------------------------------

#[tokio::test]
async fn a_local_source_lands_in_the_library_with_metadata_and_a_checkable_fulltext() {
    let server = common::open().await;
    let works_before = works(&server).await;
    let shown = ok(
        &server,
        "source_import.preview",
        json!({
        "file_name": "boje.epub", "content": baruch_store::encode_base64(&epub("Die rote Boje")) }),
    )
    .await;
    let applied = ok(&server, "source_import.apply", json!({ "draft": shown["draft"], "target": "library" })).await;
    assert!(applied.get("report").is_none(), "Begründung: nur Bibliothek legt kein Werk an: {applied}");
    assert_eq!(works(&server).await, works_before, "Begründung: kein Werk");
    let id = applied["library_title"].as_i64().unwrap();
    let pool = server.state.store.pool();
    let entry = baruch_library::get_title(pool, id).await.unwrap();
    assert_eq!(
        (entry.title.title.as_str(), entry.title.authors.as_str()),
        ("Die rote Boje", "Wenke Brise"),
        "Begründung: Titel/Autor aus dem OPF"
    );
    let text = baruch_library::title_text(pool, id).await.unwrap().expect("lokaler Volltext (0108)");
    assert_eq!(text.mime, "application/epub+zip");
    assert_eq!(text.source_name, "boje.epub");
    assert!(
        baruch_library::title_remote(pool, id).await.unwrap().is_none(),
        "Begründung: lokal ohne MultiGPT-Verknüpfung"
    );
    let read = ok(&server, "source.read", json!({ "title": id })).await;
    assert_eq!(read["origin"], "import", "Begründung: der Volltext kommt aus dem Import: {read}");
    assert!(read["text"].as_str().unwrap().contains("Die rote Boje schaukelt leise"), "Volltext lesbar: {read}");
    let full: Vec<char> = text.text.chars().collect();
    let needle: Vec<char> = "rote Boje".chars().collect();
    let start = full.windows(needle.len()).position(|w| w == needle.as_slice()).expect("Stelle im Volltext");
    let added = ok(
        &server,
        "highlight.add",
        json!({
        "title": id, "start": start, "end": start + needle.len(), "text": "rote Boje", "kind": "core" }),
    )
    .await;
    assert_eq!(
        added["check"]["result"], "found",
        "Begründung: Hervorhebung ohne Zitat geprüft gegen den lokalen Volltext: {added}"
    );
    let wrong =
        ok(&server, "highlight.add", json!({ "title": id, "start": 0, "end": 4, "text": "Möwe", "kind": "core" }))
            .await;
    assert_eq!(wrong["check"]["result"], "not_found", "Begründung: falscher Text an der Stelle fällt auf: {wrong}");

    // Dieselbe Datei noch einmal in die Bibliothek: abgelehnt, nichts angelegt.
    let titles_before = titles(&server).await;
    let shown = ok(
        &server,
        "source_import.preview",
        json!({
        "file_name": "boje.epub", "content": baruch_store::encode_base64(&epub("Die rote Boje")) }),
    )
    .await;
    let error = fails(&server, "source_import.apply", json!({ "draft": shown["draft"], "target": "both" })).await;
    assert_eq!(error["code"], -32000, "Begründung: die Quelle gibt es schon: {error}");
    assert_eq!(
        (titles(&server).await, works(&server).await),
        (titles_before, works_before),
        "Begründung: auch kein Werk"
    );
}

/// Datenbank im Stand von development (alle Store-Migrationen, Bibliothek bis 0107) mit erfundenen Zeilen.
#[tokio::test]
async fn migration_0108_keeps_a_development_database_intact() {
    use sqlx::migrate::Migrator;
    use std::borrow::Cow;
    let dir = tempfile::tempdir().unwrap();
    let store = baruch_store::Store::open(dir.path()).await.expect("Kernschema von development");
    let pool = store.pool();
    let upto: Vec<_> = baruch_library::MIGRATOR.iter().filter(|m| m.version <= 107).cloned().collect();
    Migrator { migrations: Cow::Owned(upto), ignore_missing: true, ..Migrator::DEFAULT }.run(pool).await.unwrap();
    sqlx::query("INSERT INTO library_title (id, key, kind, title, authors) VALUES (41, 'deich', 'book', 'Am Deich', 'Moor, Ina')")
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO library_quote (title_id, position, text) VALUES (41, 1, 'Der Wind kommt.')")
        .execute(pool)
        .await
        .unwrap();
    let dump = |pool: &sqlx::SqlitePool| {
        let pool = pool.clone();
        async move {
            let tables: Vec<String> = sqlx::query_scalar(
                "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' AND name <> '_sqlx_migrations' ORDER BY name",
            )
            .fetch_all(&pool)
            .await
            .unwrap();
            let mut out = std::collections::BTreeMap::new();
            for table in tables {
                let rows: Vec<String> = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
                    "SELECT quote(json_group_array(json_array(*))) FROM (SELECT * FROM \"{table}\")"
                )))
                .fetch_all(&pool)
                .await
                .unwrap_or_default();
                out.insert(table, rows);
            }
            out
        }
    };
    let before = dump(pool).await;
    assert!(!before.contains_key("library_title_text"), "vor 0108 fehlt die Tabelle");
    baruch_library::migrate(pool).await.expect("0108 auf der Datenbank von development");
    let after = dump(pool).await;
    for (table, rows) in &before {
        assert_eq!(after.get(table), Some(rows), "Begründung: {table} unverändert (verlustfrei)");
    }
    let new: Vec<&String> = after.keys().filter(|t| !before.contains_key(*t)).collect();
    assert_eq!(new, ["library_title_text"], "Begründung: genau eine neue Tabelle");
}

// ---- Seiten ohne JavaScript --------------------------------------------------------------------------------------

#[tokio::test]
async fn the_import_pages_work_without_javascript_with_status_codes_and_masking() {
    let server = common::open().await;
    let (status, page) = server.get("/werke/import/quelle").await;
    assert_eq!(status, StatusCode::OK, "Begründung: das Formular ist ohne JavaScript per GET erreichbar");
    for needle in [
        "name=\"file\"",
        "accept=\".pdf,.epub",
        "name=\"way\" value=\"local\"",
        "name=\"collection\"",
        "name=\"upload\"",
        "in MultiGPT hochladen",
        "name=\"title\"",
    ] {
        assert!(page.contains(needle), "Begründung: Formular ohne JavaScript enthält {needle}: {page}");
    }
    assert!(!page.contains("noch nicht umgesetzt"), "Begründung: kein Stub mehr");

    let tricky = "Licht \"&\" <Kai> 'Nord'";
    let (status, _, page) = post_multipart(
        &server,
        "/werke/import/quelle/vorschau",
        &[("way", "auto"), ("title", "")],
        Some(("kai.epub", &epub(tricky))),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "Begründung: Vorschau → 200: {page}");
    assert!(
        page.contains("&lt;Kai&gt;") && !page.contains("<Kai>"),
        "Begründung: Titel aus der Datei maskiert: {page}"
    );
    assert!(
        page.contains("name=\"line\" value=\"= Die Boje\"") && page.contains("name=\"line\" value=\"== Das Netz\""),
        "Begründung: Gliederung als Textfelder: {page}"
    );
    assert!(page.contains("Die rote Boje schaukelt"), "Begründung: Textprobe je Punkt: {page}");
    assert!(
        page.contains("/werke/import/quelle/verwerfen") && page.contains("Übernehmen"),
        "Knöpfe Übernehmen und Verwerfen"
    );
    let draft = page.split("data-draft=\"").nth(1).and_then(|r| r.split('"').next()).unwrap().to_owned();

    // Fehler beim Übernehmen: 422, Eingaben bleiben.
    let (status, _, page) = post_form(
        &server,
        "/werke/import/quelle/uebernehmen",
        &[
            ("draft", &draft),
            ("target", "work"),
            ("title", "Mein <Titel>"),
            ("author", "A & B"),
            ("line", "Ohne Gleichheitszeichen"),
            ("src", "0"),
            ("line", "== Das Netz"),
            ("src", "1"),
            ("line", "= Heimkehr"),
            ("src", "2"),
        ],
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "Begründung: ungültige Zeile → 422: {page}");
    assert!(page.contains("value=\"Ohne Gleichheitszeichen\""), "Begründung: die Eingabe bleibt erhalten: {page}");
    assert!(
        page.contains("value=\"Mein &lt;Titel&gt;\"") && page.contains("value=\"A &amp; B\""),
        "Begründung: Titel/Autor bleiben, maskiert: {page}"
    );

    // Übernehmen: 303 auf das Werk.
    let (status, headers, _) = post_form(
        &server,
        "/werke/import/quelle/uebernehmen",
        &[
            ("draft", &draft),
            ("target", "work"),
            ("title", tricky),
            ("author", ""),
            ("line", "= Die Boje"),
            ("src", "0"),
            ("line", ""),
            ("src", "1"),
            ("line", "== Heimkehr"),
            ("src", "2"),
        ],
    )
    .await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Begründung: Erfolg → 303");
    let location = headers.get(header::LOCATION).unwrap().to_str().unwrap().to_owned();
    let work: i64 = location.strip_prefix("/?work=").expect("auf das Werk").parse().unwrap();
    assert_eq!(
        outline_of(&server, work).await,
        [(1, "Die Boje".to_owned()), (2, "Heimkehr".into())],
        "Begründung: Formular-Gliederung (gestrichen, umgestuft) gilt"
    );
    let meta = server.state.store.work_meta(work).await.unwrap();
    assert_eq!(meta.title, tricky, "Begründung: Sonderzeichen im Titel bleiben wörtlich");

    // Nur Bibliothek: 303 auf die Quelle; Verwerfen: 303 mit Hinweis.
    let (_, _, page) = post_multipart(
        &server,
        "/werke/import/quelle/vorschau",
        &[("way", "local")],
        Some(("kai2.epub", &epub("Zweite Boje"))),
    )
    .await;
    let draft = page.split("data-draft=\"").nth(1).and_then(|r| r.split('"').next()).unwrap().to_owned();
    let (status, headers, _) = post_form(
        &server,
        "/werke/import/quelle/uebernehmen",
        &[
            ("draft", &draft),
            ("target", "library"),
            ("title", "Zweite Boje"),
            ("author", ""),
            ("line", "= Die Boje"),
            ("src", "0"),
        ],
    )
    .await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Begründung: Übernehmen leitet nach Erfolg weiter (PRG)");
    assert!(
        headers.get(header::LOCATION).unwrap().to_str().unwrap().starts_with("/quelle?title="),
        "Begründung: nur Bibliothek → Quelle"
    );
    let (_, _, page) =
        post_multipart(&server, "/werke/import/quelle/vorschau", &[], Some(("kai3.epub", &epub("Dritte Boje")))).await;
    let draft = page.split("data-draft=\"").nth(1).and_then(|r| r.split('"').next()).unwrap().to_owned();
    let (status, headers, _) = post_form(&server, "/werke/import/quelle/verwerfen", &[("draft", &draft)]).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Begründung: Verwerfen leitet zurück aufs Formular");
    assert_eq!(
        headers.get(header::LOCATION).unwrap(),
        "/werke/import/quelle?verworfen=1",
        "Begründung: Verwerfen → Formular mit Hinweis"
    );
    assert_eq!(pdf_import::draft_count(&server.state), 0, "Begründung: der Entwurf ist weg");
    let (_, page) = server.get("/werke/import/quelle?verworfen=1").await;
    assert!(page.contains("Entwurf verworfen"), "Hinweis nach dem Verwerfen");

    // Falsche Datei → 422 mit Formular; zu groß → 413.
    let (status, _, page) =
        post_multipart(&server, "/werke/import/quelle/vorschau", &[("title", "Bleibt")], Some(("notiz.txt", b"Text")))
            .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "Begründung: keine PDF/EPUB → 422");
    assert!(
        page.contains("weder PDF noch EPUB") && page.contains("value=\"Bleibt\""),
        "Begründung: Meldung, Eingaben bleiben: {page}"
    );
    let huge = vec![b'x'; pdf_import::UPLOAD_LIMIT + 1];
    let (status, _, page) =
        post_multipart(&server, "/werke/import/quelle/vorschau", &[], Some(("riesig.pdf", &huge))).await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE, "Begründung: über der Grenze → 413");
    assert!(
        page.contains("größer als 64 MiB") && page.contains("name=\"file\""),
        "Begründung: Meldung mit Formular: {page}"
    );

    // Formulare hinter same_origin.
    let (content_type, body) = multipart(&[], Some(("kai.epub", &epub("Fremd"))));
    let request = Request::post("/werke/import/quelle/vorschau")
        .header(header::CONTENT_TYPE, content_type)
        .header(header::HOST, "127.0.0.1:8765")
        .header(header::ORIGIN, "http://fremd.example")
        .body(Body::from(body))
        .unwrap();
    let (status, _, _) = server.send(request).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "Begründung: fremde Herkunft wird abgelehnt (guard::same_origin)");
}

// ---- CLI -------------------------------------------------------------------------------------------------------

#[tokio::test]
async fn the_cli_previews_without_writing_and_creates_work_or_source_only_when_asked() {
    let server = common::open().await;
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("boje.epub");
    std::fs::write(&file, epub("Die rote Boje")).unwrap();
    let (works_before, titles_before) = (works(&server).await, titles(&server).await);
    let out =
        pdf_import::cli_import(&server.state, &file, &CliSourceImport::default()).await.expect("Vorschau gelingt");
    assert!(
        out.contains("= Die Boje") && out.contains("== Das Netz") && out.contains("= Heimkehr"),
        "Begründung: Gliederung gedruckt: {out}"
    );
    assert!(out.contains("Nichts angelegt"), "Begründung: sagt, dass nichts geschrieben wurde: {out}");
    assert_eq!(
        (works(&server).await, titles(&server).await),
        (works_before, titles_before),
        "Begründung: ohne Flags nichts angelegt"
    );
    assert_eq!(pdf_import::draft_count(&server.state), 0, "Begründung: die CLI legt auch keinen Entwurf an");

    let options = CliSourceImport { apply: true, ..CliSourceImport::default() };
    let out = pdf_import::cli_import(&server.state, &file, &options).await.expect("Werk angelegt");
    assert!(out.contains("Werk") && out.contains("angelegt"), "{out}");
    assert_eq!(works(&server).await, works_before + 1, "Begründung: --uebernehmen legt das Werk an");
    assert_eq!(titles(&server).await, titles_before, "Begründung: ohne --bibliothek keine Quelle");
    let options =
        CliSourceImport { library: true, title: Some("Boje als Quelle".into()), ..CliSourceImport::default() };
    let out = pdf_import::cli_import(&server.state, &file, &options).await.expect("Quelle angelegt");
    assert!(out.contains("Quelle"), "{out}");
    assert_eq!(titles(&server).await, titles_before + 1, "Begründung: --bibliothek legt die Quelle an");
    assert_eq!(works(&server).await, works_before + 1, "Begründung: nur --bibliothek legt kein Werk an");

    // PDF ohne pdftotext: ehrliche Meldung, nichts angelegt.
    let pdf_file = dir.path().join("kliff.pdf");
    std::fs::write(&pdf_file, pdf_of(KLIFF).await).unwrap();
    let missing =
        PdfTools { pdftotext: dir.path().join("gibt-es-nicht"), pdfinfo: dir.path().join("gibt-es-auch-nicht") };
    let error = pdf_import::cli_import_with(&server.state, &pdf_file, &options, &missing).await.unwrap_err();
    assert!(
        error.contains("nicht installiert") && error.contains("poppler"),
        "Begründung: ehrlich, was fehlt: {error}"
    );
    assert_eq!(titles(&server).await, titles_before + 1, "Begründung: ohne pdftotext nichts angelegt");
    if !poppler() {
        return;
    }
    let out =
        pdf_import::cli_import(&server.state, &pdf_file, &CliSourceImport::default()).await.expect("PDF-Vorschau");
    assert!(
        out.contains("= Ankunft am Kliff") && out.contains("Hinweis (import-pdf-plain)"),
        "Begründung: Gliederung und Warnungen: {out}"
    );
    let out = pdf_import::cli_import(&server.state, &pdf_file, &CliSourceImport { apply: true, ..Default::default() })
        .await
        .expect("PDF-Werk");
    assert!(out.contains("Werk"), "{out}");
    assert_eq!(works(&server).await, works_before + 2, "Begründung: PDF mit --uebernehmen legt das Werk an");
}
