// SPDX-License-Identifier: AGPL-3.0-or-later
//! Runde 21 (Import und Bilder): Abnahme des Integrators Ende zu Ende über die Grenzen der drei Aufgaben hinweg.
//!
//! - PDF mit drei Kapiteln lokal (Formular ohne JavaScript) und über den MultiGPT-Nachbau (RPC) → Vorschau →
//!   bearbeitete Gliederung bestätigen → Werk; nichts wird vor der Bestätigung angelegt.
//! - EPUB mit Bild → Werk mit Bild; EPUB als Quelle → `/quelle` zeigt den lokalen Volltext (Verdrahtung
//!   `fulltext::has_fulltext`).
//! - Markdown-ZIP mit Bildern → Werk mit Warnungen → `export --out` → erneuter Import gleichwertig;
//!   `export --out` lehnt einen verknüpften Zwischenordner vorab ab.
//! - Manipulierte Archive (Pfadflucht, Bombe) werden über alle Wege ohne Schreiben abgelehnt.
//! - Bild mit Typwechsel ersetzen, Altbestand `/x.png` gegen `x.png`, MCP-Kontingent, Aufräumen entschiedener Anträge.
//! - EPUB-Cover eines importierten Werks zeigt Titel, Untertitel und Autor als Bild.
//! - CLI-Weiche (main.rs, Integrator): Flags nur für PDF/EPUB, Hinweise auf stderr.
//!
//! Alle Dokumente sind frei erfunden und entstehen im Test (PDF über `baruch_render::compile_pdf` unter
//! `common::PDF_SERIAL`, EPUB und ZIP von Hand gezippt, Bilder über `baruch_epub::sample::png`). Kein Eliezer,
//! kein MultiGPT (nur `connections::multigpt::mock` auf 127.0.0.1:0), kein Internet.

use crate::common;

use std::io::{Cursor, Write};
use std::net::SocketAddr;

use axum::body::{Body, to_bytes};
use axum::extract::ConnectInfo;
use axum::http::{HeaderMap, Request, StatusCode, header};
use baruch_server::connections::multigpt::mock::{MockMultigpt, full_tools};
use baruch_server::pdf_import;
use serde_json::{Value, json};
use tower::ServiceExt;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

// ---- Hilfen ------------------------------------------------------------------------------------------------------

fn result(answer: &Value) -> Value {
    assert!(answer["error"].is_null(), "RPC sollte gelingen: {answer}");
    answer["result"].clone()
}

async fn works(server: &common::Server) -> usize {
    server.state.store.work_list().await.unwrap().len()
}

async fn titles(server: &common::Server) -> usize {
    baruch_library::list_titles(server.state.store.pool()).await.unwrap().len()
}

fn poppler() -> bool {
    let found = ["pdftotext", "pdfinfo"].iter().all(|p| {
        std::process::Command::new(p).arg("-v").output().is_ok_and(|o| o.status.success() || !o.stderr.is_empty())
    });
    if !found {
        eprintln!("ÜBERSPRUNGEN: pdftotext/pdfinfo (Poppler) fehlen auf diesem Rechner");
    }
    found
}

/// Frei erfundenes Buch: drei Kapitel, zwei Abschnitte.
const ORCHARD: &str = "#set document(title: \"Der Apfelgarten am Deich\", author: \"Ilse Morgen\")
#set text(lang: \"de\")

= Der Frühling

Im April blühen die Bäume hinter dem Deich. Ilse geht jeden Morgen die Reihen ab und zählt die Knospen, die der Frost verschont hat. Die Bienen kommen aus dem Kasten am Zaun und summen über die weißen Kronen.

== Die Bienen

Die Bienen gehören dem Nachbarn, aber sie arbeiten für alle. Er bringt im Mai zwei neue Völker und stellt sie an den Graben, wo der Wind nicht hinkommt.

= Der Sommer

Im Sommer wird gemäht und gewässert. Die Äpfel sind noch klein und hart, und die Kinder aus dem Dorf werfen sie trotzdem in den Graben, um zu sehen, wie weit sie schwimmen.

== Das Gewitter

Ein Gewitter bricht im Juli zwei Äste ab. Ilse bindet sie mit Draht an den Stamm und hofft, dass sie wieder anwachsen.

= Die Ernte

Im Oktober stehen die Kisten unter den Bäumen. Jeder Apfel geht durch zwei Hände, bevor er in den Keller kommt, und der Most riecht bis auf die Straße.
";

async fn pdf_of(typst: &str) -> Vec<u8> {
    let _serial = common::PDF_SERIAL.lock().await;
    let source = baruch_render::Source { main: typst.to_owned(), files: Vec::new() };
    let pdf = tokio::task::spawn_blocking(move || baruch_render::compile_pdf(&source)).await.unwrap();
    pdf.bytes.unwrap_or_else(|| panic!("Test-PDF lässt sich nicht setzen: {:?}", pdf.diagnostics))
}

async fn outline_of(server: &common::Server, work: i64) -> Vec<(u8, String)> {
    let typst = server.state.store.export_typst(work).await.unwrap();
    baruch_typst::parse(&typst).unwrap().headings.into_iter().map(|h| (h.level, h.title)).collect()
}

fn pairs(list: &[(u8, &str)]) -> Vec<(u8, String)> {
    list.iter().map(|(l, t)| (*l, (*t).to_owned())).collect()
}

fn zip_of(entries: &[(&str, Vec<u8>)]) -> Vec<u8> {
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    for (name, bytes) in entries {
        let method = if *name == "mimetype" { CompressionMethod::Stored } else { CompressionMethod::Deflated };
        zip.start_file(*name, SimpleFileOptions::default().compression_method(method)).unwrap();
        zip.write_all(bytes).unwrap();
    }
    zip.finish().unwrap().into_inner()
}

/// Frei erfundenes EPUB 3 mit zwei Kapiteln (nav.xhtml) und einem PNG mit Unterschrift im ersten Kapitel.
fn epub_with_picture(title: &str, picture: &[u8]) -> Vec<u8> {
    let container = r#"<?xml version="1.0"?><container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container"><rootfiles><rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/></rootfiles></container>"#;
    let opf = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="id"><metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:identifier id="id">urn:uuid:5b1e7f00-0000-4000-8000-0000000021ab</dc:identifier><dc:title>{title}</dc:title><dc:creator>Jost Weide</dc:creator><dc:language>de</dc:language></metadata><manifest><item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/><item id="k1" href="k1.xhtml" media-type="application/xhtml+xml"/><item id="k2" href="k2.xhtml" media-type="application/xhtml+xml"/><item id="img" href="bilder/weide.png" media-type="image/png"/></manifest><spine><itemref idref="k1"/><itemref idref="k2"/></spine></package>"#
    );
    let nav = r#"<?xml version="1.0" encoding="UTF-8"?><html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops"><head><title>Inhalt</title></head><body><nav epub:type="toc"><ol><li><a href="k1.xhtml">Die Weide</a></li><li><a href="k2.xhtml">Der Bach</a></li></ol></nav></body></html>"#;
    let k1 = r#"<?xml version="1.0" encoding="UTF-8"?><html xmlns="http://www.w3.org/1999/xhtml"><head><title>Die Weide</title></head><body><h1>Die Weide</h1><p>Die alte Weide steht schief am Ufer und hält sich mit allen Wurzeln fest.</p><figure><img src="bilder/weide.png" alt="Eine grüne Weide"/><figcaption>Die Weide im Juni</figcaption></figure></body></html>"#;
    let k2 = r#"<?xml version="1.0" encoding="UTF-8"?><html xmlns="http://www.w3.org/1999/xhtml"><head><title>Der Bach</title></head><body><h1>Der Bach</h1><p>Der Bach fließt langsam unter der Brücke hindurch.</p></body></html>"#;
    zip_of(&[
        ("mimetype", b"application/epub+zip".to_vec()),
        ("META-INF/container.xml", container.as_bytes().to_vec()),
        ("OEBPS/content.opf", opf.into_bytes()),
        ("OEBPS/nav.xhtml", nav.as_bytes().to_vec()),
        ("OEBPS/k1.xhtml", k1.as_bytes().to_vec()),
        ("OEBPS/k2.xhtml", k2.as_bytes().to_vec()),
        ("OEBPS/bilder/weide.png", picture.to_vec()),
    ])
}

const BOUNDARY: &str = "----baruch-r21-abnahme";

/// multipart/form-data wie ein Browserformular: Textfelder, dann Dateien im Feld `field`.
fn multipart(uri: &str, fields: &[(&str, &str)], field: &str, files: &[(&str, &[u8])]) -> Request<Body> {
    let mut body = Vec::new();
    for (name, value) in fields {
        body.extend_from_slice(
            format!("--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n{value}\r\n").as_bytes(),
        );
    }
    for (name, bytes) in files {
        body.extend_from_slice(
            format!(
                "--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"{field}\"; filename=\"{name}\"\r\n\
                 Content-Type: application/octet-stream\r\n\r\n"
            )
            .as_bytes(),
        );
        body.extend_from_slice(bytes);
        body.extend_from_slice(b"\r\n");
    }
    body.extend_from_slice(format!("--{BOUNDARY}--\r\n").as_bytes());
    Request::post(uri)
        .header(header::CONTENT_TYPE, format!("multipart/form-data; boundary={BOUNDARY}"))
        .body(Body::from(body))
        .unwrap()
}

fn form(uri: &str, pairs: &[(&str, &str)]) -> Request<Body> {
    let body = pairs
        .iter()
        .map(|(k, v)| format!("{}={}", baruch_server::view::query_escape(k), baruch_server::view::query_escape(v)))
        .collect::<Vec<_>>()
        .join("&");
    Request::post(uri).header(header::CONTENT_TYPE, "application/x-www-form-urlencoded").body(Body::from(body)).unwrap()
}

async fn get_bytes(server: &common::Server, uri: &str) -> (StatusCode, HeaderMap, Vec<u8>) {
    let response = server.app.clone().oneshot(Request::get(uri).body(Body::empty()).unwrap()).await.unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    (status, headers, to_bytes(response.into_body(), usize::MAX).await.unwrap().to_vec())
}

/// Wert eines Attributs nach `marker` in einer Seite (erstes Vorkommen).
fn attribute(page: &str, marker: &str) -> String {
    page.split(marker).nth(1).and_then(|r| r.split('"').next()).unwrap_or_else(|| panic!("{marker} fehlt")).to_owned()
}

/// Zeilen der bearbeitbaren Gliederung in der Vorschau-Seite: (src, line).
fn outline_rows(page: &str) -> Vec<(String, String)> {
    let srcs: Vec<String> =
        page.split("name=\"src\" value=\"").skip(1).map(|r| r.split('"').next().unwrap().to_owned()).collect();
    let lines: Vec<String> = page
        .split("name=\"line\" value=\"")
        .skip(1)
        .map(|r| r.split('"').next().unwrap().replace("&#x2f;", "/").replace("&amp;", "&"))
        .collect();
    assert_eq!(srcs.len(), lines.len(), "Begründung: je Zeile ein src und ein Textfeld: {page}");
    srcs.into_iter().zip(lines).collect()
}

/// Dateien unter einem Ordner (relativ, sortiert).
fn files_under(dir: &std::path::Path) -> Vec<String> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&d) else { continue };
        for entry in entries {
            let entry = entry.unwrap();
            if entry.file_type().unwrap().is_dir() {
                stack.push(entry.path());
            } else {
                out.push(entry.path().strip_prefix(dir).unwrap().to_string_lossy().replace('\\', "/"));
            }
        }
    }
    out.sort();
    out
}

fn unkeyed(text: &str) -> String {
    baruch_typst::xref::strip_key_labels(text).into_owned()
}

// ---- 1. PDF lokal über die Seiten ohne JavaScript ----------------------------------------------------------------

#[tokio::test]
async fn a_local_pdf_goes_through_preview_edited_outline_and_confirmation_into_a_work_without_javascript() {
    if !poppler() {
        return;
    }
    let server = common::open().await;
    let pdf = pdf_of(ORCHARD).await;
    let works_before = works(&server).await;

    let (status, form_page) = server.get("/werke/import/quelle").await;
    assert_eq!(status, StatusCode::OK, "Begründung: die Import-Seite ist erreichbar");
    assert!(
        form_page.contains("action=\"/werke/import/quelle/vorschau\" method=\"post\" enctype=\"multipart/form-data\""),
        "Begründung: ein gewöhnliches Formular ohne JavaScript schickt die Datei: {form_page}"
    );

    let (status, _, page) = server
        .send(multipart(
            "/werke/import/quelle/vorschau",
            &[("way", "local"), ("title", "")],
            "file",
            &[("apfel.pdf", &pdf)],
        ))
        .await;
    assert_eq!(status, StatusCode::OK, "Begründung: die Vorschau ist eine Seite (200): {page}");
    assert!(page.contains("data-way=\"local\""), "Begründung: der gewählte Weg wird genannt: {page}");
    assert_eq!(works(&server).await, works_before, "Begründung: die Vorschau legt kein Werk an");
    let rows = outline_rows(&page);
    let lines: Vec<&str> = rows.iter().map(|(_, l)| l.as_str()).collect();
    assert_eq!(
        lines,
        ["= Der Frühling", "== Die Bienen", "= Der Sommer", "== Das Gewitter", "= Die Ernte"],
        "Begründung: drei Kapitel und zwei Abschnitte, in der Reihenfolge der PDF, als bearbeitbare Zeilen: {page}"
    );
    assert!(page.contains("Im April blühen"), "Begründung: Textprobe je Kapitel: {page}");
    let draft = attribute(&page, "data-draft=\"");

    // Der Mensch bearbeitet: „Der Frühling“ umbenannt, „Die Bienen“ eine Ebene tiefer, „Das Gewitter“ gestrichen.
    let edited: Vec<(String, String)> = rows
        .iter()
        .map(|(src, line)| {
            let line = match line.as_str() {
                "= Der Frühling" => "= Blüte".to_owned(),
                "== Die Bienen" => "=== Die Bienen".to_owned(),
                "== Das Gewitter" => String::new(),
                other => other.to_owned(),
            };
            (src.clone(), line)
        })
        .collect();
    let mut fields: Vec<(&str, &str)> =
        vec![("draft", &draft), ("target", "work"), ("title", "Der Apfelgarten"), ("author", "Ilse Morgen")];
    for (src, line) in &edited {
        fields.push(("line", line));
        fields.push(("src", src));
    }
    let (status, headers, body) = server.send(form("/werke/import/quelle/uebernehmen", &fields)).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Begründung: Übernehmen leitet auf das Werk: {body}");
    let work: i64 = headers[header::LOCATION].to_str().unwrap().strip_prefix("/?work=").unwrap().parse().unwrap();
    assert_eq!(
        outline_of(&server, work).await,
        pairs(&[(1, "Blüte"), (3, "Die Bienen"), (1, "Der Sommer"), (1, "Die Ernte")]),
        "Begründung: genau die vom Menschen bestätigte Gliederung"
    );
    let typst = server.state.store.export_typst(work).await.unwrap();
    assert!(typst.contains("zwei Äste ab"), "Begründung: der Text des gestrichenen Punkts bleibt erhalten: {typst}");
    assert_eq!(works(&server).await, works_before + 1, "Begründung: genau ein Werk angelegt");
    assert_eq!(pdf_import::draft_count(&server.state), 0, "Begründung: der Entwurf ist nach dem Übernehmen weg");
    let (status, workspace) = server.get(&format!("/?work={work}")).await;
    assert_eq!(status, StatusCode::OK, "Begründung: das neue Werk öffnet sich");
    assert!(workspace.contains("Blüte"), "Begründung: der Baum zeigt die bestätigte Gliederung");
}

// ---- 2. PDF über den MultiGPT-Nachbau ----------------------------------------------------------------------------

#[tokio::test]
async fn a_pdf_through_the_multigpt_mock_becomes_a_work_and_a_linked_source_after_confirmation() {
    let server = common::open().await;
    let mock = MockMultigpt::start().await;
    mock.set_tools(full_tools());
    mock.configure(|b| {
        b.upload_pages = vec![
            "Der Frühling\n\nIm April blühen die Bäume.".into(),
            "Der Sommer\n\nIm Sommer wird gemäht.".into(),
            "Die Ernte\n\nIm Oktober stehen die Kisten.".into(),
        ];
        b.upload_toc = vec![
            json!({ "section": "1", "title": "Der Frühling", "page": 1 }),
            json!({ "section": "2", "title": "Der Sommer", "page": 2 }),
            json!({ "section": "3", "title": "Die Ernte", "page": 3 }),
        ];
        b.upload_title = Some("Der Apfelgarten am Deich".into());
        b.upload_author = Some("Ilse Morgen".into());
        b.index_polls = 1;
    });
    result(&server.rpc("connection.set", json!({ "kind": "multigpt", "config": { "url": mock.url() } })).await);
    let pdf = pdf_of(ORCHARD).await;
    let (works_before, titles_before) = (works(&server).await, titles(&server).await);

    let shown = result(
        &server
            .rpc(
                "source_import.preview",
                json!({ "file_name": "apfel.pdf", "content": baruch_store::encode_base64(&pdf), "way": "auto",
                        "collection": "Garten" }),
            )
            .await,
    );
    assert_eq!(shown["way"], "multigpt", "Begründung: Verbindung und Sammlung → MultiGPT: {shown}");
    assert_eq!(mock.tool_calls("upload_document").len(), 1, "Begründung: genau ein Hochladen in den Nachbau");
    assert_eq!(
        (works(&server).await, titles(&server).await),
        (works_before, titles_before),
        "Begründung: vor der Bestätigung nichts angelegt"
    );
    let outline: Vec<Value> = shown["outline"]
        .as_array()
        .unwrap()
        .iter()
        .map(|o| {
            let title = if o["title"] == "Die Ernte" { "Die Apfelernte" } else { o["title"].as_str().unwrap() };
            json!({ "level": 1, "title": title, "source": o["index"] })
        })
        .collect();
    let applied = result(
        &server
            .rpc("source_import.apply", json!({ "draft": shown["draft"], "target": "both", "outline": outline }))
            .await,
    );
    let work = applied["report"]["work"].as_i64().expect("Werk angelegt");
    assert_eq!(
        outline_of(&server, work).await,
        pairs(&[(1, "Der Frühling"), (1, "Der Sommer"), (1, "Die Apfelernte")]),
        "Begründung: die bestätigte (umbenannte) Gliederung aus dem Inhaltsverzeichnis von MultiGPT"
    );
    let id = applied["library_title"].as_i64().expect("Quelle angelegt");
    let remote = baruch_library::title_remote(server.state.store.pool(), id).await.unwrap().expect("verknüpft");
    assert_eq!(remote.collection, "Garten", "Begründung: die Quelle zeigt auf das Dokument in MultiGPT");
    assert_eq!(mock.tool_calls("upload_document").len(), 1, "Begründung: Übernehmen lädt nicht erneut hoch");
}

// ---- 3. EPUB mit Bild → Werk; EPUB als Quelle → lokaler Volltext auf /quelle ---------------------------------------

#[tokio::test]
async fn an_epub_with_a_picture_becomes_a_work_with_the_picture_and_a_source_with_local_fulltext() {
    let server = common::open().await;
    let picture = baruch_epub::sample::png(6, 4, [40, 160, 60]);
    let epub = epub_with_picture("Die Weide am Bach", &picture);
    let shown = result(
        &server
            .rpc(
                "source_import.preview",
                json!({ "file_name": "weide.epub", "content": baruch_store::encode_base64(&epub) }),
            )
            .await,
    );
    assert_eq!(shown["way"], "local", "Begründung: EPUB liest Baruch immer selbst: {shown}");
    let applied = result(&server.rpc("source_import.apply", json!({ "draft": shown["draft"] })).await);
    let work = applied["report"]["work"].as_i64().expect("Werk");
    assert_eq!(
        outline_of(&server, work).await,
        pairs(&[(1, "Die Weide"), (1, "Der Bach")]),
        "Begründung: die Kapitel aus nav.xhtml"
    );
    let assets = server.state.store.asset_list(work).await.unwrap();
    assert_eq!(assets.len(), 1, "Begründung: das Bild des EPUB ist ein Bild des Werks: {assets:?}");
    let path = assets[0].paths.first().cloned().expect("Bild mit Pfad im Text");
    let typst = server.state.store.export_typst(work).await.unwrap();
    assert!(typst.contains(&format!("\"{path}\"")), "Begründung: der Text verweist auf das Bild: {typst}");
    assert!(typst.contains("Die Weide im Juni"), "Begründung: die Bildunterschrift kommt mit: {typst}");
    let (status, headers, bytes) = get_bytes(&server, &format!("/werke/{work}/bilder/pfad?p={path}")).await;
    assert_eq!(status, StatusCode::OK, "Begründung: der Pfad liefert das Bild");
    assert_eq!(headers[header::CONTENT_TYPE], "image/png");
    assert_eq!(bytes, picture, "Begründung: byte-gleich mit dem Bild aus dem EPUB");

    // Dieselbe Datei als Quelle: lokaler Volltext, sichtbar in der Dokumentansicht ohne JavaScript.
    let shown = result(
        &server
            .rpc(
                "source_import.preview",
                json!({ "file_name": "weide.epub", "content": baruch_store::encode_base64(&epub) }),
            )
            .await,
    );
    let applied =
        result(&server.rpc("source_import.apply", json!({ "draft": shown["draft"], "target": "library" })).await);
    let id = applied["library_title"].as_i64().expect("Quelle");
    let (status, page) = server.get(&format!("/quelle?title={id}")).await;
    assert_eq!(status, StatusCode::OK, "Begründung: Dokumentansicht der Quelle");
    assert!(
        !page.contains("data-fulltext=\"none\""),
        "Begründung: eine Quelle mit lokalem Volltext gilt als verknüpft (has_fulltext statt nur title_remote): {page}"
    );
    assert!(
        page.contains("Die alte Weide steht schief am Ufer"),
        "Begründung: der lokale Volltext steht in Teilen auf der Seite: {page}"
    );
}

// ---- 4. Markdown-ZIP mit Bildern → Werk → export --out → Re-Import ----------------------------------------------

const MEADOW: &str = "---\ntitle: Das Wiesenheft\nauthor: Paul Klee-Feld\n---\n\n# Die Wiese\n\n\
Die Wiese ist *bunt* und **hoch**.\n\n![Ein Schaf](bilder/schaf.png)\n\n\
<div class=\"kasten\">Ein Kasten aus HTML.</div>\n\n## Die Hecke\n\nIn der Hecke wohnt ein Igel.\n\n\
![Das Dach](bilder/dach.svg)\n\n# Der Zaun\n\nDer Zaun hat drei Latten.\n";

const ROOF: &str =
    "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"8\" height=\"8\"><path d=\"M0 8 L4 0 L8 8 Z\"/></svg>\n";

#[cfg(unix)]
#[tokio::test]
async fn a_markdown_zip_with_pictures_becomes_a_work_with_warnings_and_survives_export_out_and_reimport() {
    let server = common::open().await;
    let sheep = baruch_epub::sample::png(5, 5, [250, 250, 250]);
    let archive = zip_of(&[
        ("wiese/index.md", MEADOW.as_bytes().to_vec()),
        ("wiese/bilder/schaf.png", sheep.clone()),
        ("wiese/bilder/dach.svg", ROOF.as_bytes().to_vec()),
    ]);
    let (status, _, report) =
        server.send(multipart("/werke/import", &[("title", "")], "file", &[("wiese.zip", &archive)])).await;
    assert_eq!(status, StatusCode::OK, "Begründung: mit Warnungen zeigt der Import den Bericht: {report}");
    assert!(
        report.contains("data-code=\"md-html-block\""),
        "Begründung: der HTML-Block wird nicht still verworfen, sondern gemeldet: {report}"
    );
    let work: i64 = attribute(&report, "href=\"/?work=").parse().unwrap();
    assert_eq!(
        outline_of(&server, work).await,
        pairs(&[(1, "Die Wiese"), (2, "Die Hecke"), (1, "Der Zaun")]),
        "Begründung: Markdown-Überschriften werden die Gliederung"
    );
    let meta = server.state.store.work_meta(work).await.unwrap();
    assert_eq!(meta.title, "Das Wiesenheft", "Begründung: Titel aus dem YAML-Kopf");
    let mut names: Vec<String> =
        server.state.store.asset_list(work).await.unwrap().into_iter().map(|a| a.name).collect();
    names.sort();
    assert_eq!(names, ["dach.svg", "schaf.png"], "Begründung: beide Bilder relativ zur Haupt-Datei");
    let typst = server.state.store.export_typst(work).await.unwrap();
    assert!(typst.contains("emph[bunt]") || typst.contains("_bunt_"), "Begründung: Hervorhebung abgebildet: {typst}");

    // export --out: ein verknüpfter Zwischenordner wird vorab abgelehnt, ohne Datei.
    let tmp = tempfile::tempdir().unwrap();
    let outside = tmp.path().join("draussen");
    std::fs::create_dir_all(&outside).unwrap();
    let trap = tmp.path().join("falle");
    std::fs::create_dir_all(&trap).unwrap();
    std::os::unix::fs::symlink(&outside, trap.join("bilder")).unwrap();
    let error = baruch_server::import::export_to_dir(&server.state.store, work, &trap).await.unwrap_err();
    assert!(error.contains("nichts geschrieben"), "Begründung: Ablehnung vorab mit Zusage: {error}");
    assert!(files_under(&outside).is_empty(), "Begründung: nichts außerhalb von --out gelandet");

    // export --out sauber → Ordner als ZIP → Re-Import auf einem frischen Datenordner gleichwertig.
    let out = tmp.path().join("aus");
    baruch_server::import::export_to_dir(&server.state.store, work, &out).await.expect("export --out");
    let files = files_under(&out);
    assert_eq!(files.iter().filter(|f| f.ends_with(".typ")).count(), 1, "Begründung: eine Typst-Datei: {files:?}");
    let entries: Vec<(&str, Vec<u8>)> =
        files.iter().map(|f| (f.as_str(), std::fs::read(out.join(f)).unwrap())).collect();
    let again = zip_of(&entries);
    let other = common::open().await;
    let report2 = result(
        &other
            .rpc(
                "work.import_archive",
                json!({ "file_name": "wiese-export.zip", "content": baruch_store::encode_base64(&again) }),
            )
            .await,
    );
    let work2 = report2["work"].as_i64().unwrap();
    assert!(report2["skipped"].as_array().unwrap().is_empty(), "Begründung: kein Bild übersprungen: {report2}");
    assert_eq!(
        unkeyed(&other.state.store.export_typst(work2).await.unwrap()),
        unkeyed(&typst),
        "Begründung: der erneute Import ergibt denselben Typst-Text"
    );
    let first = server.state.store.export_assets(work).await.unwrap();
    let second = other.state.store.export_assets(work2).await.unwrap();
    assert_eq!(
        format!("{first:?}"),
        format!("{second:?}"),
        "Begründung: dieselben Bilder unter denselben Pfaden mit denselben Bytes"
    );
}

// ---- 5. Manipulierte Archive -------------------------------------------------------------------------------------

#[tokio::test]
async fn manipulated_archives_are_refused_on_every_way_without_writing_anything() {
    let server = common::open().await;
    let before = (works(&server).await, titles(&server).await);
    let escape = zip_of(&[("../flucht.md", b"# Flucht\n".to_vec())]);
    let bomb = zip_of(&[("buch.md", b"# Buch\n".to_vec()), ("null.bin", vec![0u8; 4 * 1024 * 1024])]);
    assert!(bomb.len() < 64 * 1024, "Testvoraussetzung: stark gepackt ({} Bytes)", bomb.len());

    for (name, bytes) in [("flucht.zip", &escape), ("bombe.zip", &bomb)] {
        let answer = server
            .rpc("work.import_archive", json!({ "file_name": name, "content": baruch_store::encode_base64(bytes) }))
            .await;
        assert!(answer["error"].is_object(), "Begründung: RPC lehnt {name} ab: {answer}");
        let (status, _, page) =
            server.send(multipart("/werke/import", &[("title", "")], "file", &[(name, bytes)])).await;
        assert!(
            status == StatusCode::UNPROCESSABLE_ENTITY || status == StatusCode::PAYLOAD_TOO_LARGE,
            "Begründung: das Formular lehnt {name} mit 422/413 ab: {status} {page}"
        );
    }
    // Dieselben Fallen als EPUB über den PDF/EPUB-Import.
    let epub_escape = zip_of(&[
        ("mimetype", b"application/epub+zip".to_vec()),
        ("../META-INF/container.xml", b"<container/>".to_vec()),
    ]);
    let epub_bomb =
        zip_of(&[("mimetype", b"application/epub+zip".to_vec()), ("OEBPS/null.xhtml", vec![b' '; 8 * 1024 * 1024])]);
    for (name, bytes) in [("flucht.epub", &epub_escape), ("bombe.epub", &epub_bomb)] {
        let answer = server
            .rpc("source_import.preview", json!({ "file_name": name, "content": baruch_store::encode_base64(bytes) }))
            .await;
        assert!(answer["error"].is_object(), "Begründung: source_import lehnt {name} ab: {answer}");
    }
    assert_eq!((works(&server).await, titles(&server).await), before, "Begründung: kein Werk, keine Quelle");
    assert_eq!(pdf_import::draft_count(&server.state), 0, "Begründung: auch kein Entwurf bleibt liegen");
    assert!(!server.path().join("flucht.md").exists(), "Begründung: nichts neben dem Datenordner geschrieben");
    assert!(
        !server.path().parent().unwrap().join("flucht.md").exists() && !server.path().join("META-INF").exists(),
        "Begründung: keine Pfadflucht auf die Platte"
    );
}

// ---- 6. Bilder: Typwechsel, Altbestand, MCP-Kontingent, Aufräumen ------------------------------------------------

async fn post_mcp(server: &common::Server, session: Option<&str>, message: Value) -> (HeaderMap, Value) {
    let mut request = Request::post("/mcp").header(header::CONTENT_TYPE, "application/json");
    if let Some(id) = session {
        request = request.header("mcp-session-id", id);
    }
    let mut request = request.body(Body::from(message.to_string())).unwrap();
    request.extensions_mut().insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 40021))));
    let response = server.app.clone().oneshot(request).await.unwrap();
    let headers = response.headers().clone();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    (headers, serde_json::from_slice(&bytes).unwrap())
}

async fn mcp_session(server: &common::Server, client: &str) -> String {
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

async fn mcp_tool(server: &common::Server, session: &str, name: &str, arguments: Value) -> Value {
    let message = json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/call",
                          "params": { "name": name, "arguments": arguments } });
    post_mcp(server, Some(session), message).await.1["result"].clone()
}

fn svg(n: usize) -> String {
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"4\" height=\"4\"><rect width=\"4\" height=\"4\" fill=\"#{n:06x}\"/></svg>\n"
    )
}

#[tokio::test]
async fn pictures_of_an_imported_work_change_type_resolve_legacy_paths_keep_the_quota_and_get_purged() {
    let server = common::open().await;
    let archive = zip_of(&[
        ("garten.md", b"# Das Beet\n\n![Dach](bilder/dach.svg)\n\nKohl und Bohnen.\n".to_vec()),
        ("bilder/dach.svg", ROOF.as_bytes().to_vec()),
    ]);
    let report = result(
        &server
            .rpc(
                "work.import_archive",
                json!({ "file_name": "garten.zip", "content": baruch_store::encode_base64(&archive) }),
            )
            .await,
    );
    let work = report["work"].as_i64().unwrap();

    // Typwechsel: PNG statt SVG nur mit rename; der alte Pfad liefert danach das PNG.
    let png = baruch_epub::sample::png(12, 6, [180, 40, 40]);
    let content = baruch_store::encode_base64(&png);
    let refused = server
        .rpc(
            "asset.replace",
            json!({ "work": work, "name": "dach.svg", "file_name": "dach.png", "content": content,
                                       "encoding": "base64" }),
        )
        .await;
    assert_eq!(refused["error"]["code"], -32602, "Begründung: anderes Format ohne rename abgelehnt: {refused}");
    let replaced = result(
        &server
            .rpc(
                "asset.replace",
                json!({ "work": work, "name": "dach.svg", "file_name": "dach.png", "content": content,
                                           "encoding": "base64", "rename": true }),
            )
            .await,
    );
    assert_eq!(replaced["name"], "dach.png", "Begründung: Name folgt der neuen Endung: {replaced}");
    let (_, headers, bytes) = get_bytes(&server, &format!("/werke/{work}/bilder/pfad?p=bilder/dach.svg")).await;
    assert_eq!(
        (headers[header::CONTENT_TYPE].to_str().unwrap(), bytes),
        ("image/png", png.clone()),
        "Begründung: der Pfad im Text bleibt und liefert das neue PNG"
    );
    {
        let _serial = common::PDF_SERIAL.lock().await;
        let (status, headers, body) = get_bytes(&server, &format!("/werke/{work}/pdf")).await;
        assert_eq!(
            status,
            StatusCode::OK,
            "Begründung: das Buch setzt sich trotz Typwechsel: {}",
            String::from_utf8_lossy(&body)
        );
        assert_eq!(headers[header::CONTENT_TYPE], "application/pdf");
    }

    // Altbestand: /x.png und x.png zeigen auf zwei Bilder; der Mensch wählt.
    let red = baruch_epub::sample::png(2, 2, [255, 0, 0]);
    let blue = baruch_epub::sample::png(2, 2, [0, 0, 255]);
    result(
        &server
            .rpc(
                "asset.add",
                json!({ "work": work, "file_name": "rot.png", "content": baruch_store::encode_base64(&red),
                                             "encoding": "base64", "path": "/x.png" }),
            )
            .await,
    );
    result(
        &server
            .rpc(
                "asset.add",
                json!({ "work": work, "file_name": "blau.png", "content": baruch_store::encode_base64(&blue),
                                             "encoding": "base64" }),
            )
            .await,
    );
    sqlx::query("INSERT INTO asset_path (work_id, path, asset_id) SELECT ?, 'x.png', id FROM asset WHERE work_id = ? AND name = 'blau.png'")
        .bind(work)
        .bind(work)
        .execute(server.state.store.pool())
        .await
        .unwrap();
    let conflicts = result(&server.rpc("asset.path_conflicts", json!({ "work": work })).await);
    assert_eq!(conflicts["conflicts"].as_array().unwrap().len(), 1, "Begründung: eine Kollision erkannt: {conflicts}");
    let (status, _) = server
        .post_form("/werke/bilder/act", &format!("action=resolve_path&work={work}&normal=x.png&name=blau.png"))
        .await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Begründung: Auflösen über ein Formular ohne JavaScript");
    for path in ["/x.png", "x.png"] {
        let (_, _, bytes) = get_bytes(&server, &format!("/werke/{work}/bilder/pfad?p={path}")).await;
        assert_eq!(bytes, blue, "Begründung: {path} liefert das gewählte Bild");
    }

    // MCP-Kontingent: der 31. schreibende Aufruf eines Clients scheitert ehrlich, nichts gespeichert.
    use baruch_server::mcp::assets::MAX_MCP_ASSET_WRITES_PER_DAY;
    let session = mcp_session(&server, "Abnahme-Hilfe").await;
    for n in 0..MAX_MCP_ASSET_WRITES_PER_DAY - 1 {
        let answer = mcp_tool(
            &server,
            &session,
            "asset_add",
            json!({ "work": work, "file_name": format!("k{n}.svg"), "content": svg(n), "encoding": "svg" }),
        )
        .await;
        assert_eq!(answer["isError"], false, "Begründung: unter der Grenze erlaubt: {answer}");
    }
    let request = mcp_tool(
        &server,
        &session,
        "asset_replace",
        json!({ "work": work, "name": "dach.png", "content": svg(77), "encoding": "svg", "rename": true }),
    )
    .await;
    assert_eq!(request["isError"], false, "Begründung: der 30. Aufruf (Antrag mit rename) ist noch erlaubt: {request}");
    let request_id = request["structuredContent"]["request"]["id"].as_i64().expect("Antrag");
    let assets_before = server.state.store.asset_list(work).await.unwrap().len();
    let over = mcp_tool(
        &server,
        &session,
        "asset_add",
        json!({ "work": work, "file_name": "zuviel.svg", "content": svg(999), "encoding": "svg" }),
    )
    .await;
    assert_eq!(over["isError"], true, "Begründung: der 31. Aufruf überschreitet das Kontingent: {over}");
    assert!(
        over["content"][0]["text"].as_str().unwrap().contains("Kontingent"),
        "Begründung: ehrliche Meldung: {over}"
    );
    assert_eq!(
        server.state.store.asset_list(work).await.unwrap().len(),
        assets_before,
        "Begründung: nichts gespeichert"
    );

    // Aufräumen: Antrag entscheiden, Entscheidung 31 Tage zurückdatieren → Inhalt weg, Datei 410.
    result(&server.rpc("asset.request_decide", json!({ "work": work, "id": request_id, "decision": "reject" })).await);
    sqlx::query("UPDATE asset_request SET decided = strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-31 days') WHERE id = ?")
        .bind(request_id)
        .execute(server.state.store.pool())
        .await
        .unwrap();
    let list = result(&server.rpc("asset.request_list", json!({ "work": work, "open": false })).await);
    let entry = list["requests"].as_array().unwrap().iter().find(|r| r["id"] == request_id).unwrap().clone();
    assert_eq!(entry["purged"], true, "Begründung: über 30 Tage entschieden → Inhalt entfernt: {entry}");
    let (status, _, _) = get_bytes(&server, &format!("/werke/{work}/bilder/antraege/{request_id}/datei")).await;
    assert_eq!(status, StatusCode::GONE, "Begründung: die Antragsdatei antwortet 410");
}

// ---- 7. EPUB-Cover eines importierten Werks ----------------------------------------------------------------------

fn block(kind: &str, y_mm: f64, size_pt: f64, visible: bool) -> Value {
    json!({ "kind": kind, "x_mm": 10.0, "y_mm": y_mm, "width_mm": 100.0, "align": "center", "size_pt": size_pt,
            "leading_em": 0.65, "color": "light", "font": null, "style": "bold", "backdrop": "none", "visible": visible })
}

fn zip_entry(bytes: &[u8], name: &str) -> Option<Vec<u8>> {
    use std::io::Read as _;
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).expect("gültiges ZIP");
    let mut file = archive.by_name(name).ok()?;
    let mut out = Vec::new();
    file.read_to_end(&mut out).unwrap();
    Some(out)
}

async fn cover_with(server: &common::Server, work: i64, visible: [bool; 3]) -> image::RgbaImage {
    let blocks = json!([
        block("front_title", 20.0, 28.0, visible[0]),
        block("front_subtitle", 60.0, 16.0, visible[1]),
        block("front_author", 150.0, 14.0, visible[2]),
    ]);
    result(&server.rpc("cover.layout_set", json!({ "work": work, "blocks": blocks })).await);
    let answer = result(&server.rpc("work.epub", json!({ "work": work, "inline": true })).await);
    let codes: Vec<&str> = answer["warnings"].as_array().unwrap().iter().filter_map(|w| w["code"].as_str()).collect();
    assert!(!codes.contains(&"COVER_RASTER_FAILED"), "Begründung: das Rastern gelingt: {codes:?}");
    let epub = baruch_store::decode_base64(answer["epub_base64"].as_str().unwrap()).unwrap();
    let cover = zip_entry(&epub, "OEBPS/images/cover.png").expect("Cover-Bild im EPUB");
    image::load_from_memory(&cover).unwrap().to_rgba8()
}

fn differing(a: &image::RgbaImage, b: &image::RgbaImage) -> usize {
    assert_eq!(a.dimensions(), b.dimensions(), "gleiche Rastergröße");
    a.pixels().zip(b.pixels()).filter(|(p, q)| (0..3).any(|c| (i32::from(p[c]) - i32::from(q[c])).abs() > 60)).count()
}

#[tokio::test]
async fn the_epub_cover_of_an_imported_work_shows_title_subtitle_and_author_as_a_picture() {
    let server = common::open().await;
    let archive = zip_of(&[("deich.md", b"# Am Deich\n\nDer Wind kommt von Westen.\n".to_vec())]);
    let report = result(
        &server
            .rpc(
                "work.import_archive",
                json!({ "file_name": "deich.zip", "content": baruch_store::encode_base64(&archive),
                                                "title": "Das Deichbuch" }),
            )
            .await,
    );
    let work = report["work"].as_i64().unwrap();
    result(
        &server
            .rpc("work.meta.set", json!({ "work": work, "subtitle": "Ein Jahr am Wasser", "author": "Ilse Morgen" }))
            .await,
    );

    let _serial = common::PDF_SERIAL.lock().await;
    let all = cover_with(&server, work, [true, true, true]).await;
    let no_title = cover_with(&server, work, [false, true, true]).await;
    let no_subtitle = cover_with(&server, work, [true, false, true]).await;
    let no_author = cover_with(&server, work, [true, true, false]).await;
    let none = cover_with(&server, work, [false, false, false]).await;
    assert!(differing(&all, &none) > 1000, "Begründung: die Blöcke stehen als Schrift im Bild");
    assert!(differing(&all, &no_title) > 500, "Begründung: der Titel steht im Bild");
    assert!(differing(&all, &no_subtitle) > 200, "Begründung: der Untertitel steht im Bild");
    assert!(differing(&all, &no_author) > 200, "Begründung: der Autor steht im Bild");
}

// ---- 8. CLI-Weiche und Seiten ohne JavaScript ---------------------------------------------------------------------

fn cli(data: &std::path::Path, args: &[&std::ffi::OsStr]) -> (i32, String, String) {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_baruch-server"))
        .args(args)
        .arg("--data")
        .arg(data)
        .env_remove("BARUCH_DATA")
        .output()
        .expect("baruch-server startet");
    (
        output.status.code().unwrap_or(-1),
        String::from_utf8(output.stdout).unwrap(),
        String::from_utf8(output.stderr).unwrap(),
    )
}

#[tokio::test]
async fn the_cli_switch_keeps_pdf_flags_to_pdf_and_prints_hints_and_the_menus_lead_to_the_import_pages() {
    let data = tempfile::tempdir().unwrap();
    let files = tempfile::tempdir().unwrap();
    let md = files.path().join("notiz.md");
    std::fs::write(&md, "# Notiz\n\n<div>roh</div>\n\nText.\n").unwrap();
    let (code, _, err) = cli(data.path(), &["import".as_ref(), md.as_os_str(), "--uebernehmen".as_ref()]);
    assert_eq!(code, 2, "Begründung: --uebernehmen gilt nur für PDF/EPUB: {err}");
    assert!(err.contains("nur für PDF/EPUB"), "Begründung: die Meldung sagt es: {err}");
    let (code, out, err) = cli(data.path(), &["import".as_ref(), md.as_os_str()]);
    assert_eq!(code, 0, "Begründung: Markdown-Import über die CLI gelingt: {err}");
    assert!(out.contains("Werk"), "Begründung: Bericht auf stdout: {out}");
    // markdown_import::cli_import liefert den Bericht samt Hinweisen als Text (Ok → stdout, wie die Austausch-Befehle).
    assert!(
        out.contains("Hinweis (md-html-block, Zeile 3)"),
        "Begründung: die Warnung steht mit Code und Zeile im Bericht, nicht still verworfen: {out}"
    );

    let server = common::open().await;
    let (_, new) = server.get("/werke/neu").await;
    assert!(new.contains("href=\"/werke/import/quelle\""), "Begründung: „Neues Werk“ verweist auf den PDF/EPUB-Import");
    assert!(
        new.contains("accept=\".typ,.md,.markdown,.zip\""),
        "Begründung: Typst-, Markdown- und ZIP-Datei im selben Formular"
    );
    let (_, home) = server.get("/").await;
    assert!(
        home.contains("PDF oder EPUB importieren…"),
        "Begründung: Menüeintrag unter „Baruch ▾“ (Runde 33: vorher „Werk ▾“)"
    );
}
