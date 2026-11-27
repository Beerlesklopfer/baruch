// SPDX-License-Identifier: AGPL-3.0-or-later
//! Round 20 (acceptance, integrator): end to end with a freely invented work „Das Feuer am Kap“ (two parts, chapters
//! with sections, footnotes, images, a cross-reference, a Bible reference, cover, ISBN, book details with special
//! characters, typesetting settings, work template with a chapter-break rule).
//!
//! Ways: menu (link in the menu Werk), download, RPC `work.epub`, CLI `export --epub`, preview per reader with iframes
//! and reader files, own check (also of deliberately broken EPUBs), EPUBCheck as a self-written script through
//! `path_env` (never a real program, never a download), MCP `epub_export` and `epub_check`. Everything without
//! JavaScript (form GET, links, iframes). No request to Eliezer or into the network, no Typst typesetting; images
//! created in the test.

use crate::common;

use std::io::{Read as _, Write as _};
use std::net::SocketAddr;
use std::path::Path;
use std::process::Command;

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::extract::ConnectInfo;
use axum::http::{HeaderMap, Request, StatusCode, header};
use baruch_epub::FootnoteMode;
use baruch_epub::check::{EpubcheckStatus, Severity, check_structure, codes};
use baruch_epub::readers::{PROFILES, profile};
use serde_json::{Value, json};
use tower::ServiceExt;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

/// Freely invented book. Template with language and a show rule that breaks before every chapter.
fn cape() -> String {
    format!(
        "#set text(lang: \"de\")\n\
         #show heading.where(level: 1): it => {{ pagebreak(weak: true); it }}\n\n\
         {}\n\n\
         = Die Lampe\nDie Lampe brennt mit Rüböl.#footnote[Erst später kam Petroleum.] Ein Licht für den Weg \
         (Ps 119,105).\n\n\
         #figure(image(\"bilder/linse.png\", alt: \"Eine gestufte Glaslinse\"), caption: [Die Linse])\n\n\
         == Der Docht\nDer Docht wird jeden Abend gekürzt.#footnote[Mit einer kleinen Schere.]\n\n\
         = Der Wärter\nDer Wärter schreibt jede Nacht ins Buch.\n\n\
         #image(\"bilder/buch.png\")\n\n\
         {}\n\n\
         = Die Stürme\nIm Herbst kommen die Stürme von Westen.\n\n\
         == Die Glocke\nBei Nebel läutet die Glocke.\n",
        baruch_typst::part_line("Das Feuer"),
        baruch_typst::part_line("Das Wetter"),
    )
}

fn result(answer: &Value) -> &Value {
    assert!(answer["error"].is_null(), "RPC-Fehler: {answer}");
    &answer["result"]
}

async fn rpc(app: &Router, method: &str, params: Value) -> Value {
    common::rpc(app.clone(), method, params).await
}

async fn get_bytes(app: &Router, uri: &str, headers: &[(&str, &str)]) -> (StatusCode, HeaderMap, Vec<u8>) {
    let mut request = Request::get(uri);
    for (name, value) in headers {
        request = request.header(*name, *value);
    }
    let response = app.clone().oneshot(request.body(Body::empty()).unwrap()).await.unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    (status, headers, to_bytes(response.into_body(), usize::MAX).await.unwrap().to_vec())
}

async fn get_text(app: &Router, uri: &str) -> (StatusCode, String) {
    let (status, _, bytes) = get_bytes(app, uri, &[]).await;
    (status, String::from_utf8(bytes).unwrap())
}

fn entries(zip_bytes: &[u8]) -> Vec<(String, Vec<u8>)> {
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(zip_bytes)).expect("gültiges ZIP");
    (0..archive.len())
        .map(|i| {
            let mut file = archive.by_index(i).unwrap();
            let mut bytes = Vec::new();
            file.read_to_end(&mut bytes).unwrap();
            (file.name().unwrap().into_owned(), bytes)
        })
        .collect()
}

fn entry(files: &[(String, Vec<u8>)], name: &str) -> String {
    let file = files
        .iter()
        .find(|f| f.0 == name)
        .unwrap_or_else(|| panic!("{name} fehlt: {:?}", files.iter().map(|f| f.0.as_str()).collect::<Vec<_>>()));
    String::from_utf8(file.1.clone()).unwrap()
}

/// ZIP from `list`; `mimetype` with `mimetype_method`, everything else DEFLATE (like a tool that packs wrongly).
fn zip_of(list: &[(String, Vec<u8>)], mimetype_method: CompressionMethod) -> Vec<u8> {
    let mut zip = ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for (name, bytes) in list {
        let method = if name == "mimetype" { mimetype_method } else { CompressionMethod::Deflated };
        zip.start_file(name.as_str(), SimpleFileOptions::default().compression_method(method)).unwrap();
        zip.write_all(bytes).unwrap();
    }
    zip.finish().unwrap().into_inner()
}

/// Images (one with alternative text in the text, one with alternative text only in the image details), cover, book
/// details with special characters, typesetting settings and a cross-reference from „Die Stürme“ to „Der Wärter“.
/// Returns (key „Der Wärter“, key „Die Stürme“).
async fn furnish(app: &Router, store: &baruch_store::Store, work: i64) -> (i64, i64) {
    for (file, path, alt, color) in [
        ("linse.png", "bilder/linse.png", "", [200, 200, 220]),
        ("buch.png", "bilder/buch.png", "Ein aufgeschlagenes Wachbuch", [120, 90, 60]),
    ] {
        let png = baruch_epub::sample::png(5, 4, color);
        let answer = rpc(
            app,
            "asset.add",
            json!({ "work": work, "file_name": file, "content": baruch_store::encode_base64(&png),
                    "encoding": "base64", "path": path, "alt": alt }),
        )
        .await;
        result(&answer);
    }
    let cover = baruch_epub::sample::png(6, 9, [20, 40, 90]);
    let answer = rpc(
        app,
        "asset.add",
        json!({ "work": work, "file_name": "umschlag.png", "content": baruch_store::encode_base64(&cover),
                "encoding": "base64", "caption": "Foto: Ida Erfunden", "alt": "Ein Leuchtturm bei Nacht" }),
    )
    .await;
    let name = result(&answer)["name"].as_str().unwrap().to_owned();
    result(&rpc(app, "work.set_cover", json!({ "work": work, "name": name })).await);
    let answer = rpc(
        app,
        "work.meta.set",
        json!({ "work": work, "author": "Ole Probe", "subtitle": "Licht & <Nebel> #[nachts]",
                "publisher": "Kap & Klippe", "year": "2026", "isbn": "978-3-16-148410-0", "front_matter": true,
                "dedication": "Für alle Wärter & <Wärterinnen>", "blurb": "Ein Turm, ein Licht, #ein Winter." }),
    )
    .await;
    result(&answer);
    let settings = json!({ "font": "Libertinus Serif", "paragraph": { "kind": "indent", "em": 1.5 },
                           "hyphenate": true, "lang": "de" });
    result(&rpc(app, "work.typeset_set", json!({ "work": work, "settings": settings })).await);

    let nodes = store.load_work(work).await.unwrap().nodes;
    let keeper = nodes.iter().find(|n| n.title == "Der Wärter").expect("Kapitel Der Wärter").key;
    let storms = nodes.iter().find(|n| n.title == "Die Stürme").expect("Kapitel Die Stürme");
    let text = format!(
        "= Die Stürme\nIm Herbst kommen die Stürme von Westen; mehr dazu bei #link(<k{keeper}>)[dem Wärter].\n"
    );
    let rev = store.add_proposal(work, storms.id, &text, "@lektor", "probe", "Querverweis").await.unwrap();
    result(&rpc(app, "revision.accept", json!({ "work": work, "node": storms.id, "rev": rev })).await);
    (keeper, storms.key)
}

/// Create and equip the work; (server, work, slug, key Wärter, key Stürme).
async fn cape_server() -> (common::Server, i64, String, i64, i64) {
    let server = common::open().await;
    let report = server.import(&cape(), "kap.typ").await;
    let (keeper, storms) = furnish(&server.app, &server.state.store, report.work).await;
    (server, report.work, report.slug, keeper, storms)
}

fn xmllint() -> Option<&'static Path> {
    let path = Path::new("/usr/bin/xmllint");
    path.exists().then_some(path)
}

/// Every XML file of the EPUB with `xmllint --noout --nonet` (stdin); without xmllint honestly skipped.
fn xmllint_every_file(files: &[(String, Vec<u8>)]) {
    let Some(tool) = xmllint() else {
        println!("ÜBERSPRUNGEN: xmllint fehlt, Wohlgeformtheit nur über die eigene Prüfung");
        return;
    };
    let mut checked = 0;
    for (name, bytes) in files {
        if !(name.ends_with(".xhtml") || name.ends_with(".opf") || name.ends_with(".ncx") || name.ends_with(".xml")) {
            continue;
        }
        let mut child = Command::new(tool)
            .args(["--noout", "--nonet", "-"])
            .stdin(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(bytes).unwrap();
        let out = child.wait_with_output().unwrap();
        assert!(out.status.success(), "{name} ist wohlgeformt: {}", String::from_utf8_lossy(&out.stderr));
        checked += 1;
    }
    assert!(checked >= 10, "alle XML-Dateien geprüft (Umschlag, Titelei, Teile, Kapitel, nav, ncx, opf …): {checked}");
}

#[tokio::test]
async fn the_invented_book_downloads_by_menu_and_rpc_with_parts_chapters_notes_links_cover_metadata_and_css() {
    let (server, work, slug, keeper, storms) = cape_server().await;
    let app = &server.app;

    // Menu Werk → „Als EPUB herunterladen“ (ordinary link with download, without JavaScript).
    let (status, html) = get_text(app, &format!("/?work={work}")).await;
    assert_eq!(status, StatusCode::OK, "Werkseite lädt");
    assert!(
        html.contains(&format!("href=\"/werke/{work}/epub\" download")),
        "Menü Werk führt auf den Download des Werks"
    );
    let (status, headers, download) = get_bytes(app, &format!("/werke/{work}/epub"), &[]).await;
    assert_eq!(status, StatusCode::OK, "Download geht");
    assert_eq!(headers[header::CONTENT_TYPE], "application/epub+zip", "EPUB-Medientyp");
    assert!(
        headers[header::CONTENT_DISPOSITION].to_str().unwrap().contains(&format!("{slug}.epub")),
        "Anhang <slug>.epub"
    );
    let answer = rpc(app, "work.epub", json!({ "work": work, "inline": true })).await;
    let r = result(&answer).clone();
    let inline = baruch_store::decode_base64(r["epub_base64"].as_str().unwrap()).unwrap();
    assert_eq!(inline, download, "RPC und Download liefern dieselben Bytes (gleicher Stand, deterministisch)");

    // Every part page and every chapter is a file of its own; cover and front matter at the front.
    let listed: Vec<(String, String)> = r["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| (f["kind"].as_str().unwrap().to_owned(), f["title"].as_str().unwrap_or("").to_owned()))
        .collect();
    let kinds: Vec<&str> = listed.iter().map(|f| f.0.as_str()).collect();
    assert_eq!(
        kinds,
        ["cover", "front", "front", "front", "part", "chapter", "chapter", "part", "chapter"],
        "Umschlag, Titelseite, Impressum, Widmung, dann zwei Teile mit drei Kapiteln; Abschnitte im Kapitel: {listed:?}"
    );
    let parts: Vec<&str> = listed.iter().filter(|f| f.0 == "part").map(|f| f.1.as_str()).collect();
    assert_eq!(parts, ["I Das Feuer", "II Das Wetter"], "beide Teile als eigene Seite, mit Nummer aus dem Schema");
    let codes_built: Vec<&str> =
        r["warnings"].as_array().unwrap().iter().map(|w| w["code"].as_str().unwrap()).collect();
    assert!(codes_built.contains(&"TEMPLATE_PAGEBREAK_MAPPED"), "Kapitelumbruch-Regel erkannt: {codes_built:?}");
    assert!(
        !codes_built.iter().any(|c| matches!(*c, "IMAGE_MISSING" | "IMAGE_NO_ALT" | "XREF_UNKNOWN" | "ISBN_INVALID")),
        "Bilder mit Alternativtext, Querverweis und ISBN in Ordnung: {codes_built:?}"
    );

    let files = entries(&download);
    let nav = entry(&files, "OEBPS/nav.xhtml");
    for title in ["Das Feuer", "Die Lampe", "Der Docht", "Der Wärter", "Das Wetter", "Die Stürme", "Die Glocke"] {
        assert!(nav.contains(title), "Inhaltsverzeichnis nennt {title}: {nav}");
    }
    assert!(nav.contains("epub:type=\"toc\"") && nav.contains("epub:type=\"landmarks\""), "toc und landmarks");
    let ncx = entry(&files, "OEBPS/toc.ncx");
    assert!(ncx.contains("Das Wetter") && ncx.contains("Die Glocke"), "EPUB-2-Rückfall toc.ncx: {ncx}");

    // Footnotes as popup: reference and footnote in the same chapter file, the target exists.
    let lamp = files
        .iter()
        .find(|f| f.0.starts_with("OEBPS/text/k") && String::from_utf8_lossy(&f.1).contains("Die Lampe</h"))
        .map(|f| String::from_utf8(f.1.clone()).unwrap())
        .expect("Kapiteldatei der Lampe");
    assert_eq!(lamp.matches("epub:type=\"noteref\"").count(), 2, "zwei Fußnoten im Kapitel (auch im Abschnitt)");
    assert_eq!(lamp.matches("epub:type=\"footnote\"").count(), 2, "beide Fußnoten am Ende der Kapiteldatei");
    let note_href = lamp.split("epub:type=\"noteref\" href=\"#").nth(1).map(|s| s.split('"').next().unwrap());
    if let Some(id) = note_href {
        assert!(lamp.contains(&format!("id=\"{id}\"")), "Fußnotenziel {id} existiert");
    } else {
        assert!(lamp.contains("href=\"#"), "Fußnotenverweis ist ein interner Link: {lamp}");
    }
    assert!(lamp.contains("Erst später kam Petroleum."), "Fußnotentext lesbar auch ohne Popup");
    assert!(lamp.contains("Ps 119,105"), "Bibelverweis bleibt als Text erhalten");
    assert!(lamp.contains("alt=\"Eine gestufte Glaslinse\""), "Alternativtext aus dem Typst-Text");
    assert!(lamp.contains("<figcaption>Die Linse</figcaption>"), "Bildunterschrift");
    assert!(lamp.contains("class=\"baruch-chapter\""), "Kapitel als section.baruch-chapter");
    let keeper_file = format!("OEBPS/text/k{keeper}.xhtml");
    let keeper_text = entry(&files, &keeper_file);
    assert!(keeper_text.contains("alt=\"Ein aufgeschlagenes Wachbuch\""), "Alternativtext aus den Bildangaben");
    assert!(keeper_text.contains(&format!("id=\"k{keeper}\"")), "Überschrift mit Sprungmarke k{keeper}");

    // Cross-reference as an internal link into the file of the target.
    let storms_text = entry(&files, &format!("OEBPS/text/k{storms}.xhtml"));
    assert!(
        storms_text.contains(&format!("href=\"k{keeper}.xhtml#k{keeper}\"")),
        "#link(<k{keeper}>) wird ein Link in die Kapiteldatei: {storms_text}"
    );

    // Cover image as cover, metadata literally (escaped).
    let opf = entry(&files, "OEBPS/content.opf");
    for (needle, why) in [
        ("properties=\"cover-image\"", "Umschlag als cover-image"),
        ("<meta name=\"cover\" content=\"cover-image\"/>", "EPUB-2-Umschlag"),
        ("urn:isbn:9783161484100", "ISBN geprüft"),
        ("<dc:creator id=\"creator\">Ole Probe</dc:creator>", "Autor"),
        ("<dc:publisher>Kap &amp; Klippe</dc:publisher>", "Verlag mit & maskiert"),
        ("Licht &amp; &lt;Nebel&gt; #[nachts]", "Untertitel wörtlich, nicht als Typst/HTML ausgewertet"),
        ("<dc:description>Ein Turm, ein Licht, #ein Winter.</dc:description>", "Klappentext als Text"),
        ("<dc:language>de</dc:language>", "Sprache"),
        ("Bildnachweis: Foto: Ida Erfunden", "Bildnachweis aus der Unterschrift des Umschlags"),
        ("toc=\"ncx\"", "Spine mit NCX"),
    ] {
        assert!(opf.contains(needle), "{why}: {needle}\n{opf}");
    }
    assert!(files.iter().any(|f| f.0 == "OEBPS/images/cover.png"), "Umschlagbild im Paket");
    let spine_first = opf.split("<itemref").nth(1).unwrap_or("");
    assert!(spine_first.contains("idref=\"cover\""), "cover.xhtml ist die erste Spine-Datei: {opf}");
    let dedication = entry(&files, "OEBPS/text/dedication.xhtml");
    assert!(dedication.contains("Für alle Wärter &amp; &lt;Wärterinnen&gt;"), "Widmung wörtlich: {dedication}");

    // Typesetting settings as CSS.
    let css = entry(&files, "OEBPS/style/book.css");
    assert!(css.contains("\"Libertinus Serif\""), "Schrift aus den Satz-Einstellungen: {css}");
    assert!(css.contains("text-indent: 1.5em"), "Einzug 1,5 em: {css}");
    assert!(css.contains("hyphens: auto"), "Silbentrennung an: {css}");
    assert!(css.contains("Sprache: de"), "Sprache der Silbentrennung: {css}");
    assert!(css.contains("section.baruch-chapter") && css.contains("break-before"), "Kapitel auf neuer Seite");

    // Well-formed and without errors in the own check.
    xmllint_every_file(&files);
    let (findings, tool) = check_structure(&download, xmllint());
    let errors: Vec<_> = findings.iter().filter(|f| f.severity == Severity::Error).collect();
    assert!(errors.is_empty(), "das EPUB des Werks hat keinen Fehler: {errors:?}");
    if xmllint().is_some() {
        assert_eq!(tool, baruch_epub::check::XmlTool::Xmllint, "xmllint benutzt, wenn vorhanden");
    }
}

#[tokio::test]
async fn the_cli_writes_the_same_bytes_as_the_download() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().join("daten");
    let (slug, download) = {
        let state = std::sync::Arc::new(baruch_server::AppState::open(&data).await.unwrap());
        let app = baruch_server::router(state.clone());
        let report = baruch_server::import::import_typst(&state, &cape(), "kap.typ").await.unwrap();
        furnish(&app, &state.store, report.work).await;
        let (status, _, bytes) = get_bytes(&app, &format!("/werke/{}/epub", report.work), &[]).await;
        assert_eq!(status, StatusCode::OK, "Download");
        (report.slug, bytes)
    };
    let file = dir.path().join("kap.epub");
    let out = Command::new(env!("CARGO_BIN_EXE_baruch-server"))
        .args(["export", &slug, "--epub", file.to_str().unwrap(), "--data", data.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(out.status.success(), "export --epub: {}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(std::fs::read(&file).unwrap(), download, "CLI und Download: gleicher Stand → gleiche Bytes");
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("TEMPLATE_PAGEBREAK_MAPPED"),
        "Hinweis zur Kapitelregel der Vorlage auf stderr"
    );
}

#[tokio::test]
async fn the_preview_shows_several_readers_side_by_side_with_chapter_boundaries_and_the_template_hint() {
    let (server, work, _, keeper, _) = cape_server().await;
    let app = &server.app;
    let (status, html) = get_text(app, &format!("/werke/{work}/epub/vorschau")).await;
    assert_eq!(status, StatusCode::OK, "Vorschau lädt");
    for p in PROFILES.iter().filter(|p| p.required) {
        assert!(html.contains(&format!("data-reader=\"{}\"", p.slug)), "Pflicht-Profil {} vorgewählt", p.slug);
    }
    assert!(html.contains("Kindle (über EPUB)"), "Kindle über EPUB unter den Profilen");
    assert!(html.contains("Send to Kindle"), "Hinweis Send to Kindle nimmt EPUB an");
    assert_eq!(html.matches("<iframe ").count(), 5, "ein Rahmen je Pflicht-Profil, nebeneinander");
    assert!(html.contains("class=\"epub-file boundary\""), "Kapitelgrenzen markiert");
    assert!(html.contains("Der Wärter") && html.contains("Das Wetter"), "Dateien mit Titeln");
    assert!(html.contains("data-code=\"TEMPLATE_PAGEBREAK_MAPPED\""), "Hinweis zur Kapitelregel der Vorlage sichtbar");
    assert!(html.contains("data-epub-check-summary"), "darunter der Prüfbericht");
    assert!(html.contains("method=\"get\""), "Wahl der Reader als Formular-GET (ohne JavaScript)");

    // Two freely chosen profiles, a particular file, night mode.
    let file = format!("text/k{keeper}.xhtml");
    let uri = format!("/werke/{work}/epub/vorschau?leser=tolino&leser=kindle&datei={file}&nacht=1");
    let (status, html) = get_text(app, &uri).await;
    assert_eq!(status, StatusCode::OK, "Auswahl per GET: {html}");
    assert_eq!(html.matches("<iframe ").count(), 2, "genau die zwei gewählten Reader");
    let tolino = format!("/werke/{work}/epub/leser/tolino/{file}");
    assert!(html.contains(&tolino), "Rahmen zeigt die gewählte Datei: {html}");

    // The file in the frame: XHTML with reader CSS, own CSP, frameable only in Baruch.
    let (status, headers, body) = get_bytes(app, &tolino, &[]).await;
    assert_eq!(status, StatusCode::OK, "Reader-Datei lädt");
    let content_type = headers[header::CONTENT_TYPE].to_str().unwrap();
    assert!(content_type.starts_with("application/xhtml+xml"), "XHTML: {content_type}");
    let csp = headers[header::CONTENT_SECURITY_POLICY].to_str().unwrap();
    assert!(csp.contains("sandbox") && csp.contains("frame-ancestors 'self'"), "eigene CSP: {csp}");
    assert_eq!(headers[header::X_FRAME_OPTIONS], "SAMEORIGIN", "Rahmen nur aus Baruch");
    let body = String::from_utf8(body).unwrap();
    assert!(body.contains("id=\"baruch-reader\"") && body.contains("Der Wärter"), "Reader-CSS und Kapiteltext");
    let (_, headers, _) = get_bytes(app, &format!("/werke/{work}/epub/vorschau"), &[]).await;
    assert_eq!(headers[header::X_FRAME_OPTIONS], "DENY", "die Vorschau-Seite selbst bleibt unrahmbar");

    // RPC epub.preview names all profiles with addresses.
    let all: Vec<&str> = PROFILES.iter().map(|p| p.slug).collect();
    let answer = rpc(app, "epub.preview", json!({ "work": work, "readers": all })).await;
    let readers = result(&answer)["readers"].as_array().unwrap().clone();
    assert_eq!(readers.len(), PROFILES.len(), "jedes Profil in der Vorschau");
    assert!(
        readers.iter().all(|r| r["url"].as_str().unwrap().starts_with(&format!("/werke/{work}/epub/leser/"))),
        "jede Vorschau-Adresse zeigt auf die Leser-Route dieses Werks (sonst lädt der Rahmen nichts): {readers:?}"
    );
}

/// Takes a work template with a pagebreak rule that cannot be mapped to chapters.
#[tokio::test]
async fn a_pagebreak_rule_the_epub_cannot_map_is_a_visible_warning() {
    let server = common::open().await;
    let source = "#set text(lang: \"de\")\n#show figure: it => { pagebreak(); it }\n\n= Die Bucht\nRuhiges Wasser.\n";
    let work = server.import(source, "bucht.typ").await.work;
    let answer = server.rpc("work.epub", json!({ "work": work })).await;
    let codes_built: Vec<String> = result(&answer)["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|w| w["code"].as_str().unwrap().to_owned())
        .collect();
    assert!(
        codes_built.iter().any(|c| c == "TEMPLATE_PAGEBREAK_UNMAPPED"),
        "nicht abbildbar gemeldet: {codes_built:?}"
    );
    let (_, html) = server.get(&format!("/werke/{work}/epub/vorschau")).await;
    assert!(html.contains("data-code=\"TEMPLATE_PAGEBREAK_UNMAPPED\""), "auf der Vorschau sichtbar");
}

#[tokio::test]
async fn the_check_finds_deliberately_broken_epubs() {
    let (server, work, _, _, _) = cape_server().await;
    let (_, _, good) = get_bytes(&server.app, &format!("/werke/{work}/epub"), &[]).await;
    let list = entries(&good);
    let error_codes = |epub: &[u8]| -> Vec<String> {
        check_structure(epub, xmllint())
            .0
            .into_iter()
            .filter(|f| f.severity == Severity::Error)
            .map(|f| f.code)
            .collect()
    };
    assert!(error_codes(&good).is_empty(), "Ausgangslage ohne Fehler");

    // Missing file: a chapter file taken out of the ZIP, the manifest still names it.
    let mut missing = list.clone();
    let gone = missing.iter().position(|f| f.0.starts_with("OEBPS/text/k")).unwrap();
    missing.remove(gone);
    let found = error_codes(&zip_of(&missing, CompressionMethod::Stored));
    assert!(found.iter().any(|c| c == codes::MANIFEST_FILE_MISSING), "fehlende Datei gefunden: {found:?}");

    // Wrong mimetype: once wrong content, once compressed.
    let mut wrong = list.clone();
    wrong[0].1 = b"application/zip".to_vec();
    let found = error_codes(&zip_of(&wrong, CompressionMethod::Stored));
    assert!(found.iter().any(|c| c == codes::MIMETYPE_CONTENT), "falscher mimetype gefunden: {found:?}");
    let found = error_codes(&zip_of(&list, CompressionMethod::Deflated));
    assert!(found.iter().any(|c| c == codes::MIMETYPE_COMPRESSED), "komprimierter mimetype gefunden: {found:?}");

    // Broken XHTML: a closing tag is missing.
    let mut broken = list.clone();
    let chapter = broken
        .iter()
        .position(|f| f.0.starts_with("OEBPS/text/k") && String::from_utf8_lossy(&f.1).contains("</p>"))
        .expect("Kapiteldatei mit Absatz");
    let text = String::from_utf8(broken[chapter].1.clone()).unwrap().replacen("</p>", "", 1);
    broken[chapter].1 = text.into_bytes();
    let found = error_codes(&zip_of(&broken, CompressionMethod::Stored));
    assert!(found.iter().any(|c| c == codes::XHTML_NOT_WELLFORMED), "kaputtes XHTML gefunden: {found:?}");
}

fn make_executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt as _;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

#[tokio::test]
async fn epubcheck_is_used_when_present_and_otherwise_reported_honestly() {
    let (server, work, _, _, _) = cape_server().await;
    let readers = [profile("kindle").unwrap(), profile("apple-books-iphone").unwrap()];

    // Empty PATH content and no jar: honestly „nicht gefunden“, the own check holds nevertheless.
    let empty = tempfile::tempdir().unwrap();
    let none = Some(empty.path().as_os_str());
    let checked = baruch_server::epub_check::run_check(&server.state, work, &readers, FootnoteMode::Popup, true, none)
        .await
        .unwrap();
    assert_eq!(checked.report.epubcheck, EpubcheckStatus::NotFound, "ohne Programm: nicht gefunden");
    assert_eq!(checked.summary.errors, 0, "eigene Prüfung ohne Fehler");
    assert!(
        checked.report.findings.iter().any(|f| f.code == codes::READER_CONVERSION),
        "Kindle-Hinweis zur Umwandlung über Send to Kindle"
    );

    // Self-written EPUBCheck in the PATH content passed in: it runs and passes.
    let bin = tempfile::tempdir().unwrap();
    let script = bin.path().join("epubcheck");
    std::fs::write(&script, "#!/bin/sh\necho 'EPUBCheck v5.2.0'\necho 'No errors or warnings detected.'\nexit 0\n")
        .unwrap();
    make_executable(&script);
    let path = Some(bin.path().as_os_str());
    let checked = baruch_server::epub_check::run_check(&server.state, work, &readers, FootnoteMode::Popup, true, path)
        .await
        .unwrap();
    match &checked.report.epubcheck {
        EpubcheckStatus::Ran { version, ok, .. } => {
            assert_eq!(version.as_deref(), Some("5.2.0"), "Version aus der Ausgabe");
            assert!(ok, "Exit 0: bestanden");
        }
        other => panic!("EPUBCheck lief: {other:?}"),
    }

    // Check page without JavaScript: report, status and button as a form.
    let (status, html) = server.get(&format!("/werke/{work}/epub/pruefung?leser=kindle")).await;
    assert_eq!(status, StatusCode::OK, "Prüfseite lädt: {html}");
    assert!(html.contains("<form") && html.contains("EPUBCheck ausführen"), "Knopf als Formular");
    assert!(html.contains("Keine Fehler."), "Zusammenfassung der eigenen Prüfung: {html}");
}

async fn mcp(server: &common::Server, session: Option<&str>, message: Value) -> (HeaderMap, Value) {
    let mut request = Request::post("/mcp")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::ACCEPT, "application/json, text/event-stream");
    if let Some(id) = session {
        request = request.header("mcp-session-id", id);
    }
    let mut request = request.body(Body::from(message.to_string())).unwrap();
    request.extensions_mut().insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 40000))));
    let (status, headers, body) = server.send(request).await;
    assert_eq!(status, StatusCode::OK, "MCP: {body}");
    (headers, serde_json::from_str(&body).unwrap())
}

#[tokio::test]
async fn both_mcp_tools_read_only_and_epub_check_never_starts_epubcheck() {
    let (server, work, slug, _, _) = cape_server().await;
    let init = json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize",
                       "params": { "protocolVersion": "2025-03-26", "capabilities": {},
                                   "clientInfo": { "name": "abnahme", "version": "1" } } });
    let (headers, init) = mcp(&server, None, init).await;
    let session = headers.get("mcp-session-id").map(|v| v.to_str().unwrap().to_owned());
    let instructions = init["result"]["instructions"].as_str().unwrap_or("");
    assert!(instructions.contains("epub_export") && instructions.contains("epub_check"), "instructions nennen beide");
    let commits = || common::git(&server.mirror_dir_of(&slug), &["rev-list", "--count", "HEAD"]);
    let before = commits();

    let call = |name: &str, arguments: Value| json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": { "name": name, "arguments": arguments } });
    let (_, answer) = mcp(&server, session.as_deref(), call("epub_export", json!({ "work": work }))).await;
    assert_eq!(answer["result"]["isError"], false, "epub_export gelingt: {answer}");
    let content = &answer["result"]["structuredContent"];
    assert_eq!(content["meta"]["isbn"], "9783161484100", "ISBN in den Metadaten");
    let kinds: Vec<&str> = content["files"].as_array().unwrap().iter().map(|f| f["kind"].as_str().unwrap()).collect();
    assert_eq!(kinds.iter().filter(|k| **k == "chapter").count(), 3, "drei Kapiteldateien: {kinds:?}");

    let (_, answer) =
        mcp(&server, session.as_deref(), call("epub_check", json!({ "work": work, "epubcheck": true }))).await;
    assert_eq!(answer["result"]["isError"], false, "epub_check gelingt: {answer}");
    let content = &answer["result"]["structuredContent"];
    assert_eq!(content["epubcheck"]["status"], "not_requested", "MCP startet nie EPUBCheck");
    assert_eq!(content["summary"]["errors"], 0, "eigene Prüfung ohne Fehler: {content}");
    assert_eq!(commits(), before, "beide Werkzeuge nur lesend: kein Commit");
}
