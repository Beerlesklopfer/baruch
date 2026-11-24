// SPDX-License-Identifier: AGPL-3.0-or-later
//! Runde 21, Nachbesserung nach dem Gutachten: Schutz-`;` hinter eingebetteten Ausdrücken beim EPUB-Import, führende
//! Zahlen in Fußnoten, Bilder in Fußnoten, Bildtyp im EPUB-Export nach einem Typwechsel, Seitengrenze des lokalen
//! PDF-Wegs.
//!
//! Alle Texte und Bilder sind frei erfunden und entstehen im Test (EPUB von Hand gezippt, PNG über
//! `baruch_epub::sample::png`). Poppler wird für die Seitengrenze durch zwei kleine Shell-Skripte ersetzt, die ihre
//! Argumente mitschreiben; kein Eliezer, kein Netz. Satz und EPUB unter `common::PDF_SERIAL`.

use std::io::{Read as _, Write as _};

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode, header};
use baruch_server::pdf_import::{self, CliSourceImport, PdfTools};
use serde_json::{Value, json};
use tower::ServiceExt;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

use crate::common;

fn result(answer: &Value) -> &Value {
    assert!(answer["error"].is_null(), "RPC-Fehler: {answer}");
    &answer["result"]
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

fn zip_entry(zip_bytes: &[u8], name: &str) -> Option<Vec<u8>> {
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(zip_bytes)).expect("gültiges ZIP");
    let mut file = archive.by_name(name).ok()?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).unwrap();
    Some(bytes)
}

/// EPUB 3 mit einem Kapitel: Zeilenumbruch vor „(“, Code vor „.Feld“, Hervorhebung mitten im Wort vor „(n)“; drei
/// Fußnoten: eine beginnt mit einer Bibelstelle (Rücksprung trägt die Nummer), eine mit ihrer eigenen Nummer
/// (gehört zum Zeichen), eine enthält ein Bild.
fn tricky_epub() -> Vec<u8> {
    let container = r#"<?xml version="1.0"?><container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container"><rootfiles><rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/></rootfiles></container>"#;
    let opf = r#"<?xml version="1.0" encoding="UTF-8"?><package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="id"><metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:identifier id="id">urn:uuid:7d0c2a4e-0000-4000-8000-0000000021fe</dc:identifier><dc:title>Die Leiter am Kai</dc:title><dc:language>de</dc:language></metadata><manifest><item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/><item id="k1" href="kap1.xhtml" media-type="application/xhtml+xml"/><item id="b" href="b.png" media-type="image/png"/></manifest><spine><itemref idref="k1"/></spine></package>"#;
    let nav = r#"<?xml version="1.0" encoding="UTF-8"?><html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops"><head><title>Inhalt</title></head><body><nav epub:type="toc"><ol><li><a href="kap1.xhtml">Am Kai</a></li></ol></nav></body></html>"#;
    let kap1 = r##"<?xml version="1.0" encoding="UTF-8"?><html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops"><head><title>Am Kai</title></head><body><h1>Am Kai</h1><p>Erste Zeile<br/>(Fortsetzung) hier.</p><p>Ein <code>x</code>.Feld und Haus<i>tür</i>(n).</p><p>Siehe<a epub:type="noteref" href="#n1">1</a> dort, auch<a epub:type="noteref" href="#n2">2</a> hier und<a epub:type="noteref" href="#n3">3</a> da.</p><aside epub:type="footnote" id="n1"><p><a epub:type="backlink" href="#r1">1</a> 1 Kor 13,4 sagt es.</p></aside><aside epub:type="footnote" id="n2"><p>2. Gezählt am Morgen.</p></aside><aside epub:type="footnote" id="n3"><p>Die Leiter: <img src="b.png" alt="Bild in Note"/></p></aside></body></html>"##;
    zip_of(&[
        ("mimetype", b"application/epub+zip".to_vec()),
        ("META-INF/container.xml", container.as_bytes().to_vec()),
        ("OEBPS/content.opf", opf.as_bytes().to_vec()),
        ("OEBPS/nav.xhtml", nav.as_bytes().to_vec()),
        ("OEBPS/kap1.xhtml", kap1.as_bytes().to_vec()),
        ("OEBPS/b.png", baruch_epub::sample::png(4, 3, [30, 60, 200])),
    ])
}

#[tokio::test]
async fn an_imported_epub_with_breaks_code_and_footnotes_sets_without_errors_and_loses_no_text() {
    let server = common::open().await;
    let shown = server
        .rpc(
            "source_import.preview",
            json!({ "file_name": "kai.epub", "content": baruch_store::encode_base64(&tricky_epub()) }),
        )
        .await;
    let shown = result(&shown).clone();
    let codes: Vec<&str> = shown["warnings"].as_array().unwrap().iter().filter_map(|w| w["code"].as_str()).collect();
    assert!(
        codes.contains(&"import-epub-footnote-number"),
        "Begründung: das Entfernen der Nummer „2.“ wird gemeldet: {codes:?}"
    );
    let applied = server.rpc("source_import.apply", json!({ "draft": shown["draft"] })).await;
    let work = result(&applied)["report"]["work"].as_i64().unwrap();
    let typst = server.state.store.export_typst(work).await.unwrap();

    assert!(
        typst.contains("Erste Zeile#linebreak();(Fortsetzung) hier."),
        "Begründung: ohne `;` wäre „(Fortsetzung)“ ein Aufruf von #linebreak(): {typst}"
    );
    assert!(
        typst.contains("Ein #raw(\"x\");.Feld und Haus#emph[tür];(n)."),
        "Begründung: ohne `;` wären „.Feld“ ein Feldzugriff und „(n)“ ein Aufruf: {typst}"
    );
    assert!(
        typst.contains("#footnote[1 Kor 13,4 sagt es.]"),
        "Begründung: die Nummer steckte im Rücksprung; die „1“ der Bibelstelle bleibt: {typst}"
    );
    assert!(
        typst.contains("#footnote[Gezählt am Morgen.]"),
        "Begründung: „2.“ gleich dem Fußnotenzeichen ist die Nummer und fällt weg: {typst}"
    );
    let assets = server.state.store.export_assets(work).await.unwrap();
    assert_eq!(assets.len(), 1, "Begründung: ein Bild im Werk");
    let expected = format!("#footnote[Die Leiter: #image(\"{}\", alt: \"Bild in Note\")]", assets[0].path);
    assert!(typst.contains(&expected), "Begründung: das Bild der Fußnote steht in der Fußnote ({expected}): {typst}");

    // Gemessen am Satz: das Buch-PDF entsteht ohne Fehler (vorher „expected function, found content“).
    let _serial = common::PDF_SERIAL.lock().await;
    let response = server
        .app
        .clone()
        .oneshot(Request::get(format!("/werke/{work}/pdf")).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let content_type = response.headers().get(header::CONTENT_TYPE).cloned();
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    assert_eq!(
        status,
        StatusCode::OK,
        "Begründung: das importierte Werk lässt sich setzen: {}",
        String::from_utf8_lossy(&body)
    );
    assert_eq!(content_type.unwrap(), "application/pdf");
}

/// Frei erfundenes Buch mit einem Bild unter `bilder/haus.svg`.
const BOOK: &str = "#set document(title: \"Das Haus am Hang\")\n#set text(lang: \"de\")\n\n= Das Haus\nDas Dach ist neu.\n\n\
#figure(image(\"bilder/haus.svg\", width: 40mm), caption: [Das Haus])\n";

const HOUSE_SVG: &str = "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"40\" height=\"20\">\
<rect width=\"40\" height=\"20\" fill=\"#3a6\"/></svg>\n";

#[tokio::test]
async fn after_a_type_change_the_epub_declares_the_picture_by_its_content() {
    let server = common::open().await;
    let report = server.import(BOOK, "haus.typ").await;
    let work = report.work;
    let added = server
        .rpc(
            "asset.add",
            json!({ "work": work, "file_name": "haus.svg", "content": HOUSE_SVG, "encoding": "svg",
                    "path": "bilder/haus.svg", "alt": "Ein grünes Haus" }),
        )
        .await;
    result(&added);
    let png = baruch_epub::sample::png(30, 15, [200, 40, 40]);
    let replaced = server
        .rpc(
            "asset.replace",
            json!({ "work": work, "name": "haus.svg", "file_name": "haus.png",
                    "content": baruch_store::encode_base64(&png), "encoding": "base64", "rename": true }),
        )
        .await;
    result(&replaced);

    let _serial = common::PDF_SERIAL.lock().await;
    let answer = server.rpc("work.epub", json!({ "work": work, "inline": true })).await;
    let epub = baruch_store::decode_base64(result(&answer)["epub_base64"].as_str().unwrap()).unwrap();
    let opf = String::from_utf8(zip_entry(&epub, "OEBPS/content.opf").unwrap()).unwrap();
    let item =
        opf.split("<item ").find(|i| i.contains("haus")).unwrap_or_else(|| panic!("das Bild steht im Manifest: {opf}"));
    assert!(item.contains("media-type=\"image/png\""), "Begründung: PNG-Bytes werden als PNG gemeldet: {item}");
    let href = item.split("href=\"").nth(1).and_then(|r| r.split('"').next()).unwrap();
    assert!(href.ends_with(".png"), "Begründung: die Endung im EPUB passt zum Typ: {href}");
    assert_eq!(
        zip_entry(&epub, &format!("OEBPS/{href}")).as_deref(),
        Some(png.as_slice()),
        "Begründung: Bytes unverändert"
    );

    let tool = baruch_epub::check::find_xmllint(std::env::var_os("PATH").as_deref());
    let (findings, _) = baruch_epub::check::check_structure(&epub, tool.as_deref());
    let errors: Vec<_> =
        findings.iter().filter(|f| f.severity == baruch_epub::check::Severity::Error).map(|f| &f.code).collect();
    assert!(errors.is_empty(), "Begründung: die eigene Prüfung liest das PNG nicht mehr als XML: {errors:?}");
}

#[cfg(unix)]
#[tokio::test]
async fn the_local_pdf_way_reads_at_most_2000_pages_and_says_so() {
    use std::os::unix::fs::PermissionsExt as _;
    let server = common::open().await;
    let dir = tempfile::tempdir().unwrap();
    let args_file = dir.path().join("args.txt");
    let script = |name: &str, body: String| {
        let path = dir.path().join(name);
        std::fs::write(&path, format!("#!/bin/sh\n{body}")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path
    };
    let pdfinfo = script("pdfinfo", "printf 'Title: Lang\\nPages: 5000\\n'\n".into());
    let bbox = "<html><body><doc><page width=\"100\" height=\"100\"><flow><block><line>\
<word yMin=\"0\" yMax=\"10\">Ein</word><word yMin=\"0\" yMax=\"10\">Satz.</word></line></block></flow></page></doc></body></html>";
    let pdftotext = script("pdftotext", format!("echo \"$@\" > '{}'\nprintf '%s' '{bbox}'\n", args_file.display()));
    let file = dir.path().join("lang.pdf");
    std::fs::write(&file, b"%PDF-1.4\n% frei erfunden\n").unwrap();

    let out = pdf_import::cli_import_with(
        &server.state,
        &file,
        &CliSourceImport::default(),
        &PdfTools { pdftotext, pdfinfo },
    )
    .await
    .expect("Vorschau gelingt");
    let args = std::fs::read_to_string(&args_file).unwrap();
    assert!(args.contains("-f 1 -l 2000"), "Begründung: pdftotext liest nur die ersten 2000 Seiten: {args}");
    assert!(
        out.contains("import-pdf-pages-cut") && out.contains("5000"),
        "Begründung: die Kürzung wird gemeldet: {out}"
    );
}
