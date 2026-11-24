// SPDX-License-Identifier: AGPL-3.0-or-later
//! Runde 20a (epub, Besitz „epub“): EPUB über Server, RPC `work.epub`, Seite `/werke/{id}/epub`, MCP `epub_export` und
//! CLI `export --epub`.
//!
//! Echter Datenordner im Temp-Ordner, Router über `oneshot`; keine Anfrage an Eliezer oder ins Netz, kein Typst-Satz.
//! Testtexte frei erfunden, Bilder im Test erzeugt.

use crate::common;

use std::io::Read as _;
use std::net::SocketAddr;
use std::process::Command;

use axum::body::{Body, to_bytes};
use axum::extract::ConnectInfo;
use axum::http::{HeaderMap, Request, StatusCode, header};
use serde_json::{Value, json};
use tower::ServiceExt;

/// Frei erfundenes Buch: Teil, zwei Kapitel, Abschnitt, Fußnote, Abbildung, Seitenumbruch-Regel der Vorlage.
const BOOK: &str = "#set text(lang: \"de\")\n\
#show heading.where(level: 1): it => { pagebreak(weak: true); it }\n\n\
#heading(level: 1, numbering: none)[Am Wasser] <baruch-part>\n\n\
= Die Mühle\nDas Rad dreht sich am Bach.#footnote[Seit dem Frühjahr wieder.]\n\n\
#figure(image(\"bilder/stein.png\"), caption: [Der Mahlstein])\n\n\
== Der Stein\nZwei Steine mahlen das Korn.\n\n\
= Der Müller\nEr singt beim Mahlen.\n";

fn result(answer: &Value) -> &Value {
    assert!(answer["error"].is_null(), "RPC-Fehler: {answer}");
    &answer["result"]
}

async fn get_bytes(server: &common::Server, uri: &str, headers: &[(&str, &str)]) -> (StatusCode, HeaderMap, Vec<u8>) {
    let mut request = Request::get(uri);
    for (name, value) in headers {
        request = request.header(*name, *value);
    }
    let response = server.app.clone().oneshot(request.body(Body::empty()).unwrap()).await.unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    (status, headers, to_bytes(response.into_body(), usize::MAX).await.unwrap().to_vec())
}

fn entries(zip_bytes: &[u8]) -> Vec<(String, Vec<u8>)> {
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(zip_bytes)).expect("gültiges ZIP");
    (0..archive.len())
        .map(|i| {
            let mut file = archive.by_index(i).unwrap();
            let mut bytes = Vec::new();
            file.read_to_end(&mut bytes).unwrap();
            (file.name().to_owned(), bytes)
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

/// Werk „Mühle“ importieren; (id, slug).
async fn mill(server: &common::Server) -> (i64, String) {
    let report = server.import(BOOK, "muehle.typ").await;
    (report.work, report.slug)
}

/// Bild des Werks unter `bilder/stein.png` und Umschlag, Buchangaben mit ISBN und Titelei.
async fn furnish(server: &common::Server, work: i64) {
    let stone = baruch_epub::sample::png(4, 4, [120, 120, 120]);
    let answer = server
        .rpc(
            "asset.add",
            json!({ "work": work, "file_name": "stein.png", "content": baruch_store::encode_base64(&stone),
                    "encoding": "base64", "path": "bilder/stein.png", "caption": "Mahlstein",
                    "alt": "Ein runder Mahlstein" }),
        )
        .await;
    result(&answer);
    let cover = baruch_epub::sample::png(6, 9, [30, 90, 150]);
    let answer = server
        .rpc(
            "asset.add",
            json!({ "work": work, "file_name": "umschlag.png", "content": baruch_store::encode_base64(&cover),
                    "encoding": "base64", "caption": "Foto: Hans Probe", "alt": "Die Mühle im Nebel" }),
        )
        .await;
    let name = result(&answer)["name"].as_str().unwrap().to_owned();
    result(&server.rpc("work.set_cover", json!({ "work": work, "name": name })).await);
    let answer = server
        .rpc(
            "work.meta.set",
            json!({ "work": work, "author": "Grete Beispiel", "publisher": "Bachverlag", "year": "2026",
                    "isbn": "978-3-16-148410-0", "front_matter": true, "dedication": "Für den Müller & <alle>",
                    "blurb": "Eine Mühle, ein Bach, #ein Jahr." }),
        )
        .await;
    result(&answer);
}

#[tokio::test]
async fn work_epub_describes_the_book_and_inline_bytes_match_the_download() {
    let server = common::open().await;
    let (work, slug) = mill(&server).await;
    let answer = server.rpc("work.epub", json!({ "work": work, "inline": true })).await;
    let r = result(&answer);
    assert_eq!(r["file_name"], format!("{slug}.epub"), "Dateiname <slug>.epub");
    assert_eq!(r["url"], format!("/werke/{work}/epub"), "Adresse zum Herunterladen");
    assert_eq!(r["footnotes"], "popup", "Vorgabe Popup");
    let kinds: Vec<&str> = r["files"].as_array().unwrap().iter().map(|f| f["kind"].as_str().unwrap()).collect();
    // Runde 21c: Auch ohne Umschlagbild hat das EPUB ein Cover (Vorderseite mit Titel als Rasterbild) vorn.
    assert_eq!(
        kinds,
        ["cover", "part", "chapter", "chapter"],
        "Cover mit Titel (R21c), Teil und zwei Kapitel, der Abschnitt im Kapitel: {r}"
    );
    let codes: Vec<&str> = r["warnings"].as_array().unwrap().iter().map(|w| w["code"].as_str().unwrap()).collect();
    assert!(codes.contains(&"TEMPLATE_PAGEBREAK_MAPPED"), "Hinweis auf die Kapitelregel der Vorlage: {codes:?}");
    assert!(codes.contains(&"IMAGE_MISSING"), "ohne Bild im Werk ehrlich gemeldet: {codes:?}");
    let inline = baruch_store::decode_base64(r["epub_base64"].as_str().unwrap()).unwrap();
    assert_eq!(r["bytes"], inline.len(), "Größe stimmt");
    assert_eq!(r["sha256"], baruch_exchange::sha256_hex(&inline), "Prüfsumme der Bytes");

    let (status, _, download) = get_bytes(&server, &format!("/werke/{work}/epub"), &[]).await;
    assert_eq!(status, StatusCode::OK, "Download geht");
    assert_eq!(download, inline, "gleicher Stand → gleiche Bytes über RPC und Download (deterministisch)");

    let answer = server.rpc("work.epub", json!({ "work": work })).await;
    assert!(result(&answer).get("epub_base64").is_none(), "ohne inline keine Bytes in der Antwort");
    let answer = server.rpc("work.epub", json!({ "work": work, "footnotes": "endnotes" })).await;
    let r = result(&answer);
    assert_eq!(r["footnotes"], "endnotes", "Endnoten gewählt");
    assert_eq!(r["files"].as_array().unwrap().last().unwrap()["href"], "text/notes.xhtml", "Endnoten am Ende");
    let bad = server.rpc("work.epub", json!({ "work": work, "footnotes": "seitlich" })).await;
    assert_eq!(bad["error"]["code"], -32602, "unbekannte Fußnotenart → -32602: {bad}");
}

#[tokio::test]
async fn the_download_is_an_epub_attachment_and_refuses_cross_site_requests() {
    let server = common::open().await;
    let (work, slug) = mill(&server).await;
    let (status, headers, bytes) = get_bytes(&server, &format!("/werke/{work}/epub"), &[]).await;
    assert_eq!(status, StatusCode::OK, "Download");
    assert_eq!(headers[header::CONTENT_TYPE], "application/epub+zip", "EPUB-Medientyp");
    assert_eq!(
        headers[header::CONTENT_DISPOSITION].to_str().unwrap(),
        format!("attachment; filename=\"{slug}.epub\""),
        "Anhang <slug>.epub"
    );
    assert_eq!(headers[header::X_CONTENT_TYPE_OPTIONS], "nosniff", "kein Raten des Typs");
    assert_eq!(&bytes[30..38], b"mimetype", "ZIP beginnt mit mimetype");
    let files = entries(&bytes);
    let chapter = files.iter().find(|f| f.0.starts_with("OEBPS/text/k") && entry(&files, &f.0).contains("Die Mühle"));
    let chapter = entry(&files, &chapter.expect("Kapiteldatei der Mühle").0);
    assert!(chapter.contains("epub:type=\"noteref\""), "Fußnote als Popup-Verweis: {chapter}");
    assert!(chapter.contains("Bild fehlt: bilder/stein.png"), "fehlendes Bild sichtbar statt still weg");

    let (status, _, _) = get_bytes(&server, &format!("/werke/{work}/epub"), &[("sec-fetch-site", "cross-site")]).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "fremde Seite → 403");
    let (status, _, _) = get_bytes(&server, &format!("/werke/{work}/epub?fussnoten=seitlich"), &[]).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "unbekannte Fußnotenart → 400");
    let (status, _, endnotes) = get_bytes(&server, &format!("/werke/{work}/epub?fussnoten=endnoten"), &[]).await;
    assert_eq!(status, StatusCode::OK, "Endnoten");
    assert!(entries(&endnotes).iter().any(|f| f.0 == "OEBPS/text/notes.xhtml"), "Endnoten-Datei im ZIP");
    let (status, _, _) = get_bytes(&server, "/werke/99999/epub", &[]).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "unbekanntes Werk → 404");
}

#[tokio::test]
async fn cover_book_details_numbers_and_images_reach_the_package() {
    let server = common::open().await;
    let (work, _) = mill(&server).await;
    furnish(&server, work).await;
    let answer = server.rpc("work.epub", json!({ "work": work, "inline": true })).await;
    let r = result(&answer);
    let codes: Vec<&str> = r["warnings"].as_array().unwrap().iter().map(|w| w["code"].as_str().unwrap()).collect();
    assert!(!codes.contains(&"IMAGE_MISSING") && !codes.contains(&"IMAGE_NO_ALT"), "Bild mit alt da: {codes:?}");
    let first: Vec<&str> = r["files"].as_array().unwrap().iter().map(|f| f["href"].as_str().unwrap()).take(4).collect();
    assert_eq!(
        first,
        ["text/cover.xhtml", "text/title.xhtml", "text/imprint.xhtml", "text/dedication.xhtml"],
        "Umschlag und Titelei vorn"
    );
    let files = entries(&baruch_store::decode_base64(r["epub_base64"].as_str().unwrap()).unwrap());
    let opf = entry(&files, "OEBPS/content.opf");
    let w = server.state.store.work_meta(work).await.unwrap();
    for (needle, why) in [
        (format!("<dc:identifier id=\"book-id\">urn:uuid:{}</dc:identifier>", w.uuid), "Kennung aus der Werk-UUID"),
        ("urn:isbn:9783161484100".to_owned(), "ISBN geprüft, ohne Bindestriche"),
        ("<dc:creator id=\"creator\">Grete Beispiel</dc:creator>".into(), "Autor aus dem Werk"),
        ("<dc:publisher>Bachverlag</dc:publisher>".into(), "Verlag aus den Buchangaben"),
        ("<dc:rights>© 2026 Grete Beispiel, Bachverlag</dc:rights>".into(), "Rechte aus copyright_line"),
        (
            "<dc:rights>Bildnachweis: Foto: Hans Probe</dc:rights>".into(),
            "Bildnachweis aus der Unterschrift des Umschlags",
        ),
        ("<dc:description>Eine Mühle, ein Bach, #ein Jahr.</dc:description>".into(), "Klappentext als Text"),
        ("<dc:language>de</dc:language>".into(), "Sprache aus #set text(lang:) der Vorlage"),
        ("properties=\"cover-image\"".into(), "Umschlag als cover-image (STUB EPUB_COVER ersetzt)"),
        ("<meta name=\"cover\" content=\"cover-image\"/>".into(), "EPUB-2-Umschlag"),
    ] {
        assert!(opf.contains(&needle), "{why} fehlt: {needle}\n{opf}");
    }
    let cover = entry(&files, "OEBPS/text/cover.xhtml");
    assert!(cover.contains("alt=\"Die Mühle im Nebel\""), "Alternativtext des Umschlags: {cover}");
    assert!(files.iter().any(|f| f.0 == "OEBPS/images/cover.png"), "Umschlagbild im Paket");
    let dedication = entry(&files, "OEBPS/text/dedication.xhtml");
    assert!(dedication.contains("Für den Müller &amp; &lt;alle&gt;"), "Widmung maskiert: {dedication}");
    let imprint = entry(&files, "OEBPS/text/imprint.xhtml");
    assert!(imprint.contains("ISBN 978-3-16-148410-0"), "Impressum aus imprint_lines: {imprint}");
    let chapter = files
        .iter()
        .find(|f| f.0.starts_with("OEBPS/text/k") && String::from_utf8_lossy(&f.1).contains("Die Mühle</h1>"))
        .map(|f| String::from_utf8(f.1.clone()).unwrap())
        .expect("Kapiteldatei der Mühle");
    assert!(
        chapter.contains("src=\"../images/bilder/stein.png\" alt=\"Ein runder Mahlstein\""),
        "Bild des Werks mit Alternativtext aus den Bildangaben: {chapter}"
    );
    assert!(chapter.contains("<figcaption>Der Mahlstein</figcaption>"), "Unterschrift aus caption:");
    assert!(
        chapter.contains("<span class=\"baruch-number\">1</span> Die Mühle"),
        "Nummer aus dem Gliederungsschema (Vorgabe 1.1): {chapter}"
    );
    assert!(chapter.contains("<span class=\"baruch-number\">1.1</span> Der Stein"), "Abschnitt 1.1 im Kapitel");
}

#[tokio::test]
async fn a_work_without_sections_is_refused_with_422_and_minus_32000() {
    let server = common::open().await;
    let (work, _) = mill(&server).await;
    let pool = server.state.store.pool();
    // Ein Werk ohne Gliederungspunkt entsteht über die Oberfläche nicht; nachgestellt direkt in SQLite (wie
    // outline_mcp.rs).
    for sql in [
        "UPDATE revision SET status = 'superseded' WHERE status = 'current' AND node_id IN (SELECT id FROM node WHERE work_id = ?)",
        "DELETE FROM revision WHERE node_id IN (SELECT id FROM node WHERE work_id = ?)",
        "DELETE FROM node WHERE work_id = ?",
    ] {
        sqlx::query(sql).bind(work).execute(pool).await.unwrap();
    }
    let answer = server.rpc("work.epub", json!({ "work": work })).await;
    assert_eq!(answer["error"]["code"], -32000, "keine Abschnitte → -32000: {answer}");
    let message = answer["error"]["message"].as_str().unwrap();
    assert!(message.contains("keinen Abschnitt"), "deutsche Begründung: {message}");
    let (status, _, body) = get_bytes(&server, &format!("/werke/{work}/epub"), &[]).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "Download ohne Abschnitte → 422");
    assert!(String::from_utf8_lossy(&body).contains("keinen Abschnitt"), "Begründung auf der Seite");
}

async fn mcp(server: &common::Server, message: Value) -> Value {
    let mut request = Request::post("/mcp")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(message.to_string()))
        .unwrap();
    request.extensions_mut().insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 40000))));
    let (status, _, body) = server.send(request).await;
    assert_eq!(status, StatusCode::OK, "MCP: {body}");
    serde_json::from_str(&body).unwrap()
}

async fn mcp_call(server: &common::Server, arguments: Value) -> Value {
    let message = json!({ "jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": { "name": "epub_export", "arguments": arguments } });
    mcp(server, message).await
}

#[tokio::test]
async fn mcp_epub_export_reads_only_and_delivers_bytes_only_on_request() {
    let server = common::open().await;
    let (work, slug) = mill(&server).await;
    let list = mcp(&server, json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list" })).await;
    let tool = list["result"]["tools"].as_array().unwrap().iter().find(|t| t["name"] == "epub_export").cloned();
    let description = tool.expect("epub_export in tools/list")["description"].as_str().unwrap().to_owned();
    assert!(!description.starts_with("STUB"), "kein Stub mehr: {description}");
    assert!(description.contains("nur lesend") && description.contains("8 MiB"), "Beschreibung: {description}");

    let commits_before = common::git(&server.mirror_dir_of(&slug), &["rev-list", "--count", "HEAD"]);
    let answer = mcp_call(&server, json!({ "work": work })).await;
    let content = &answer["result"]["structuredContent"];
    assert_eq!(answer["result"]["isError"], false, "gelingt: {answer}");
    assert_eq!(content["file_name"], format!("{slug}.epub"), "Dateiname");
    assert_eq!(content["url"], format!("/werke/{work}/epub"), "Adresse");
    assert_eq!(content["meta"]["language"], "de", "Metadaten dabei");
    assert_eq!(
        content["files"].as_array().unwrap().len(),
        4,
        "Dateiliste mit Cover (R21c) und drei Textdateien: {content}"
    );
    assert!(content.get("epub_base64").is_none(), "ohne include_bytes keine Bytes");
    assert_eq!(
        common::git(&server.mirror_dir_of(&slug), &["rev-list", "--count", "HEAD"]),
        commits_before,
        "nur lesend: kein Commit"
    );
    let answer = mcp_call(&server, json!({ "work": work, "include_bytes": true, "footnotes": "endnotes" })).await;
    let content = &answer["result"]["structuredContent"];
    let bytes = baruch_store::decode_base64(content["epub_base64"].as_str().unwrap()).unwrap();
    assert_eq!(content["sha256"], baruch_exchange::sha256_hex(&bytes), "Bytes passen zur Prüfsumme");
    assert!(entries(&bytes).iter().any(|f| f.0 == "OEBPS/text/notes.xhtml"), "Endnoten gewählt");
}

/// RGB-Rauschen, das sich nicht komprimieren lässt (lineare Kongruenz, fester Startwert).
fn noise_png(side: u32, seed: u32) -> Vec<u8> {
    baruch_epub::sample::png_with(side, side, |x, y| {
        let mut v = seed ^ x.wrapping_mul(0x9E37_79B1) ^ y.wrapping_mul(0x85EB_CA77);
        v ^= v >> 15;
        v = v.wrapping_mul(0x2C1B_3C6D);
        v ^= v >> 12;
        let b = v.to_le_bytes();
        [b[0], b[1], b[2]]
    })
}

#[tokio::test]
async fn mcp_epub_export_refuses_bytes_above_eight_mib_and_names_the_way_out() {
    let server = common::open().await;
    let (work, slug) = mill(&server).await;
    // Drei Bilder mit Rauschen zu je gut 3 MiB: das EPUB wird größer als 8 MiB, jedes Bild bleibt unter der
    // Bildgrenze des Werks.
    let mut text = String::from("= Die Mühle\nDas Rad dreht sich am Bach.\n");
    for n in 0..3 {
        let png = noise_png(1050, n + 1);
        assert!(png.len() < baruch_store::MAX_ASSET_BYTES, "Bild unter der Grenze des Werks");
        let path = format!("bilder/rauschen{n}.png");
        let answer = server
            .rpc(
                "asset.add",
                json!({ "work": work, "file_name": format!("rauschen{n}.png"),
                        "content": baruch_store::encode_base64(&png), "encoding": "base64", "path": path,
                        "alt": "Rauschen" }),
            )
            .await;
        result(&answer);
        text.push_str(&format!("\n#image(\"{path}\")\n"));
    }
    // Abschnitt 1 („Die Mühle“, nach dem Teil) bekommt die Bilder über einen angenommenen Vorschlag.
    let rev = server.state.store.add_proposal(work, 1, &text, "@lektor", "probe", "Bilder").await.unwrap();
    result(&server.rpc("revision.accept", json!({ "work": work, "node": 1, "rev": rev })).await);

    let answer = mcp_call(&server, json!({ "work": work })).await;
    assert_eq!(answer["result"]["isError"], false, "ohne include_bytes gelingt es auch bei großer Datei: {}", answer);
    let size = answer["result"]["structuredContent"]["bytes"].as_u64().unwrap();
    assert!(size > 8 * 1024 * 1024, "Datei über 8 MiB: {size}");
    let answer = mcp_call(&server, json!({ "work": work, "include_bytes": true })).await;
    assert_eq!(answer["result"]["isError"], true, "über 8 MiB ehrlich als Fehler");
    let message = answer["result"]["content"][0]["text"].as_str().unwrap();
    assert!(message.contains("8 MiB"), "Grenze genannt: {message}");
    assert!(message.contains(&format!("/werke/{work}/epub")), "Hinweis auf die Adresse: {message}");
    assert!(message.contains(&format!("baruch-server export {slug} --epub")), "Hinweis auf die CLI: {message}");
}

fn cli(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_baruch-server")).args(args).output().expect("baruch-server startet")
}

#[tokio::test]
async fn the_cli_writes_an_epub_never_overwrites_and_refuses_mixed_exports() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().join("daten");
    let slug = {
        let state = baruch_server::AppState::open(&data).await.unwrap();
        baruch_server::import::import_typst(&state, BOOK, "muehle.typ").await.unwrap().slug
    };
    let data_arg = data.to_str().unwrap();
    let file = dir.path().join("muehle.epub");
    let out = cli(&["export", &slug, "--epub", file.to_str().unwrap(), "--data", data_arg]);
    assert!(out.status.success(), "export --epub: {}", String::from_utf8_lossy(&out.stderr));
    let bytes = std::fs::read(&file).unwrap();
    assert_eq!(&bytes[30..58], b"mimetypeapplication/epub+zip", "EPUB-Datei mit mimetype vorn");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("EPUB geschrieben") && stdout.contains(&bytes.len().to_string()), "Ausgabe: {stdout}");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("TEMPLATE_PAGEBREAK_MAPPED"), "Hinweise auf stderr: {stderr}");

    std::fs::write(&file, "alt").unwrap();
    let out = cli(&["export", &slug, "--epub", file.to_str().unwrap(), "--data", data_arg]);
    assert_eq!(out.status.code(), Some(1), "vorhandene Datei → Exit 1");
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "alt", "nicht überschrieben");
    assert!(String::from_utf8_lossy(&out.stderr).contains("nichts überschrieben"), "ehrliche Meldung");

    let other = dir.path().join("neu.epub");
    for extra in [["--pdf", "buch.pdf"], ["--out", "ordner"], ["--stichwort", "zettel.pdf"], ["--baruch", "w.baruch"]] {
        let out = cli(&["export", &slug, "--epub", other.to_str().unwrap(), extra[0], extra[1], "--data", data_arg]);
        assert_eq!(out.status.code(), Some(2), "--epub mit {}: Exit 2", extra[0]);
        assert!(!other.exists(), "keine Datei bei {}", extra[0]);
    }
    let out = cli(&["export", "gibt-es-nicht", "--epub", other.to_str().unwrap(), "--data", data_arg]);
    assert_eq!(out.status.code(), Some(1), "unbekanntes Werk → Exit 1");
    assert!(!other.exists(), "keine Datei für ein unbekanntes Werk");
}
