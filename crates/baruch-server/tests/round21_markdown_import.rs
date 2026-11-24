// SPDX-License-Identifier: AGPL-3.0-or-later
//! Runde 21b (markdown-import): Markdown- und Archiv-Import über Formular (`POST /werke/import`), RPC (`work.import`,
//! `work.import_archive`) und CLI, Warnungen statt stillem Verwerfen; dazu die Reste des Imports: `export --out` prüft
//! vorab, Buchende nur schneiden, wenn erzeugt, erste Erwähnung ohne Glossarseite, Regelblock „// Baruch: Satz“ und
//! „Nummer aus“ über Export → Import. Alle Texte und Bilder sind frei erfunden und im Test erzeugt.

use crate::common;

use std::io::{Cursor, Write};
use std::process::Command;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use baruch_server::rpc;
use baruch_typst::typeset::{
    ChapterStart, IndexPages, PageNumberPosition, PageNumberStart, PageNumbers, TypesetSettings,
};
use serde_json::{Value, json};
use zip::CompressionMethod;
use zip::write::SimpleFileOptions;

// ---- Hilfen ------------------------------------------------------------------------------------------------------

/// CRC-32 (PNG), bitweise.
fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &byte in data {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            crc = if crc & 1 == 1 { (crc >> 1) ^ 0xEDB8_8320 } else { crc >> 1 };
        }
    }
    !crc
}

/// Gültiges PNG mit einem grauen Pixel, im Test erzeugt (keine Binärdatei im Repo).
fn tiny_png(gray: u8) -> Vec<u8> {
    fn chunk(out: &mut Vec<u8>, kind: &[u8], data: &[u8]) {
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        let mut body = kind.to_vec();
        body.extend_from_slice(data);
        out.extend_from_slice(&body);
        out.extend_from_slice(&crc32(&body).to_be_bytes());
    }
    let raw = [0u8, gray];
    let (mut a, mut b) = (1u32, 0u32);
    for &x in &raw {
        a = (a + u32::from(x)) % 65521;
        b = (b + a) % 65521;
    }
    let mut zlib = vec![0x78, 0x01, 0x01, 0x02, 0x00, 0xFD, 0xFF];
    zlib.extend_from_slice(&raw);
    zlib.extend_from_slice(&((b << 16) | a).to_be_bytes());
    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    chunk(&mut out, b"IHDR", &[0, 0, 0, 1, 0, 0, 0, 1, 8, 0, 0, 0, 0]);
    chunk(&mut out, b"IDAT", &zlib);
    chunk(&mut out, b"IEND", &[]);
    out
}

/// Frei erfundenes SVG.
const ROOF: &str =
    "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"8\" height=\"8\"><path d=\"M0 8 L4 0 L8 8 Z\"/></svg>\n";

/// ZIP aus (Name, Inhalt), DEFLATE.
fn zip_of(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    for (name, bytes) in entries {
        writer.start_file(*name, options).unwrap();
        writer.write_all(bytes).unwrap();
    }
    writer.finish().unwrap().into_inner()
}

const BOUNDARY: &str = "baruch-r21b-grenze";

/// Multipart-Anfrage wie ein Browserformular: Feld `title`, dann je Datei ein Teil `field` mit Dateinamen.
fn upload(field: &str, files: &[(&str, &[u8])], title: &str) -> Request<Body> {
    let mut body = Vec::new();
    body.extend_from_slice(
        format!("--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"title\"\r\n\r\n{title}\r\n").as_bytes(),
    );
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
    Request::post("/werke/import")
        .header(header::CONTENT_TYPE, format!("multipart/form-data; boundary={BOUNDARY}"))
        .body(Body::from(body))
        .unwrap()
}

fn result(answer: &Value) -> &Value {
    assert!(answer["error"].is_null(), "RPC-Fehler: {answer}");
    &answer["result"]
}

fn error_code(answer: &Value) -> i64 {
    answer["error"]["code"].as_i64().unwrap_or_else(|| panic!("Fehler erwartet: {answer}"))
}

async fn work_count(server: &common::Server) -> usize {
    result(&server.rpc("work.list", json!({})).await)["works"].as_array().unwrap().len()
}

async fn export(server: &common::Server, work: i64) -> String {
    server.state.store.export_typst(work).await.unwrap()
}

fn unkeyed(text: &str) -> String {
    baruch_typst::xref::strip_key_labels(text).into_owned()
}

fn codes(report: &Value) -> Vec<String> {
    report["warnings"].as_array().unwrap().iter().map(|w| w["code"].as_str().unwrap().to_owned()).collect()
}

async fn numbered(server: &common::Server, work: i64, title: &str) -> i64 {
    sqlx::query_scalar("SELECT numbered FROM node WHERE work_id = ? AND title = ?")
        .bind(work)
        .bind(title)
        .fetch_one(server.state.store.pool())
        .await
        .unwrap()
}

async fn key_of(server: &common::Server, work: i64, title: &str) -> i64 {
    sqlx::query_scalar("SELECT id FROM node WHERE work_id = ? AND title = ?")
        .bind(work)
        .bind(title)
        .fetch_one(server.state.store.pool())
        .await
        .unwrap()
}

/// Programm `baruch-server` mit Datenordner `data` aufrufen: (Exit-Code, stdout, stderr).
fn cli(data: &std::path::Path, args: &[&str]) -> (i32, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_baruch-server"))
        .args(args)
        .arg("--data")
        .arg(data)
        .env_remove("BARUCH_DATA")
        .output()
        .expect("baruch-server lässt sich nicht starten");
    (
        output.status.code().unwrap_or(-1),
        String::from_utf8(output.stdout).unwrap(),
        String::from_utf8(output.stderr).unwrap(),
    )
}

/// Erfundenes Markdown mit allen Formen der Abnahme: vier Überschriftsebenen, Hervorhebung, verschachtelte Liste,
/// Aufzählung, Zitat, Codeblock, Link, Fußnote, Bild und eine Zeile voller Typst-Sonderzeichen.
const LIGHTHOUSE: &str = "# Der Leuchtturm\n\
\n\
Am Kap steht ein *alter* Turm mit **rotem** Dach und `Lampe 3`.[^wind]\n\
\n\
## Die Treppe\n\
\n\
- Erste Stufe\n  - knarrt\n  - glänzt\n\
- Zweite Stufe\n\
\n\
1. Licht an\n\
2. Licht aus\n\
\n\
> Der Wärter sagt: Nie schlafen.\n\
\n\
### Das Logbuch\n\
\n\
```text\n\
Eintrag 1: Nebel\n\
```\n\
\n\
Mehr im [Hafenamt](https://hafen.example/logbuch).\n\
\n\
#### Die Lampe\n\
\n\
Preis #3 kostet $5 *und* _mehr_ @wärter 1<2 \\` \\\\ Ende\n\
\\= kein Kapitel\n\
\n\
![Der Turm bei Nacht](bilder/turm.png)\n\
\n\
[^wind]: Der Wind kommt von Westen.\n";

// ---- Markdown mit allen Formen -----------------------------------------------------------------------------------

#[tokio::test]
async fn an_invented_markdown_with_every_form_becomes_a_work_that_typesets() {
    let server = common::open().await;
    let archive = zip_of(&[("turm/leuchtturm.md", LIGHTHOUSE.as_bytes()), ("turm/bilder/turm.png", &tiny_png(90))]);
    let answer = server
        .rpc(
            "work.import_archive",
            json!({ "file_name": "turm.zip", "content": baruch_store::encode_base64(&archive) }),
        )
        .await;
    let report = result(&answer).clone();
    let work = report["work"].as_i64().unwrap();
    assert_eq!(report["nodes"], 4, "vier Überschriften (#, ##, ###, ####) werden vier Abschnitte: {report}");
    assert_eq!(report["assets"].as_array().unwrap().len(), 1, "das Bild aus dem Archiv ist angelegt: {report}");
    assert_eq!(report["skipped"], json!([]), "nichts übersprungen: {report}");
    let warning = report["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|w| w["code"] == "md-heading-level")
        .unwrap_or_else(|| panic!("#### muss einen Hinweis geben: {report}"));
    assert_eq!(warning["line"], 25, "der Hinweis nennt die Zeile der ####-Überschrift im Markdown: {warning}");

    let typst = export(&server, work).await;
    let levels: Vec<u8> = baruch_typst::parse(&typst).unwrap().headings.iter().map(|h| h.level).collect();
    assert_eq!(levels, vec![1, 2, 3, 3], "#### ist auf Ebene 3 begrenzt; „\\= kein Kapitel“ ist keine Überschrift");
    for expected in [
        "#emph[alter]",
        "#strong[rotem]",
        "#raw(\"Lampe 3\")",
        "#footnote[Der Wind kommt von Westen.]",
        "- Erste Stufe\n  - knarrt\n  - glänzt\n- Zweite Stufe",
        "1. Licht an\n2. Licht aus",
        "#quote(block: true)[\nDer Wärter sagt: Nie schlafen.\n]",
        "```text\nEintrag 1: Nebel\n```",
        "#link(\"https://hafen.example/logbuch\")[Hafenamt]",
        "#image(\"bilder/turm.png\", alt: \"Der Turm bei Nacht\")",
    ] {
        assert!(typst.contains(expected), "Umwandlung „{expected}“ fehlt im Typst-Text: {typst}");
    }
    let asset = server.state.store.asset_by_path(work, "bilder/turm.png").await.unwrap();
    assert!(asset.is_some(), "das Bild gehört unter seinem Pfad zum Werk (Satz und Satzbild finden es)");

    // Setzen mit dem eingebetteten Typst (wie Satz-Vorschau und PDF); PDF_SERIAL, weil Satz-Läufe prozessweit sind.
    let _serial = common::PDF_SERIAL.lock().await;
    let typeset = baruch_server::typeset::typeset(&server.state, work).await.unwrap();
    let errors: Vec<_> = typeset.diagnostics().into_iter().filter(|d| d.severity == "error").collect();
    assert!(errors.is_empty(), "der umgewandelte Text setzt sich fehlerfrei (samt Bild): {errors:?}");
    let source = typeset.prepared.source.clone();
    let pages = tokio::task::spawn_blocking(move || baruch_render::page_texts(&source)).await.unwrap();
    let text: String = pages.expect("Seiten").iter().flatten().map(|t| t.text.as_str()).collect();
    // Jedes Sonderzeichen muss als Zeichen im Satz stehen: wäre es Typst-Markup geblieben, verschwände es (#, $, @, <,
    // `, \ als Code, Mathe, Verweis, Label, Raw, Escape) bzw. würde Auszeichnung (*, _) oder Überschrift (=).
    for c in ['#', '$', '@', '<', '`', '\\', '='] {
        assert!(text.contains(c), "„{c}“ steht wörtlich im Satz: {text}");
    }
    assert!(text.contains("kein Kapitel"), "die =-Zeile ist Fließtext geblieben: {text}");
}

// ---- Hochladen ohne JavaScript ---------------------------------------------------------------------------------

#[tokio::test]
async fn a_zip_upload_reports_missing_large_non_images_and_escapes_with_a_reason() {
    let server = common::open().await;
    let md = "# Die Mühle\n\nDas Rad.\n\n![Rad](bilder/rad.png)\n![fehlt](bilder/fehlt.png)\n![Notiz](bilder/notiz.png)\n\
              ![weg](../draussen.png)\n![groß](bilder/gross.png)\n";
    // Kaum packbar (Pseudozufall), sonst hielte der Archivleser es für eine Zip-Bombe.
    let mut seed = 0x2545_f491u32;
    let big: Vec<u8> = (0..8 * 1024 * 1024 + 1)
        .map(|_| {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            (seed >> 24) as u8
        })
        .collect();
    let archive = zip_of(&[
        ("muehle/index.md", md.as_bytes()),
        ("muehle/bilder/rad.png", &tiny_png(10)),
        ("muehle/bilder/notiz.png", b"Nur eine Notiz, kein Bild.\n"),
        ("muehle/bilder/gross.png", &big),
        ("muehle/unbenutzt.txt", b"liegt nur bei"),
    ]);
    let before = work_count(&server).await;
    let (status, _, html) = server.send(upload("file", &[("muehle.zip", &archive)], "")).await;
    assert_eq!(status, StatusCode::OK, "mit Hinweisen zeigt der Import den Bericht statt weiterzuleiten: {html}");
    assert_eq!(work_count(&server).await, before + 1, "das Werk ist trotzdem angelegt");
    for (path, reason) in [
        ("bilder/fehlt.png", "nicht gefunden"),
        ("bilder/notiz.png", "kein unterstütztes Bild"),
        ("../draussen.png", "&#x27;..&#x27; ist nicht erlaubt"),
        ("bilder/gross.png", "größer als 8 MiB"),
    ] {
        // minijinja schreibt „/“ als `&#x2f;` (SKILLS.md); der Browser zeigt den Pfad wie geschrieben.
        let row = html.lines().find(|l| l.contains(&format!("<code>{}</code>", path.replace('/', "&#x2f;"))));
        assert!(
            row.is_some_and(|r| r.contains(reason)),
            "übersprungenes Bild {path} steht mit Grund „{reason}“ im Bericht: {html}"
        );
    }
    assert!(html.contains("data-code=\"archive-unused\""), "die unbenutzte Datei ist ein Hinweis: {html}");
    assert!(html.contains("unbenutzt.txt"), "der Hinweis nennt die Datei: {html}");
    assert!(html.contains("href=\"/?work="), "der Bericht verlinkt das Werk: {html}");
    assert!(!html.contains("{{") && !html.contains("{%"), "keine Platzhalterreste: {html}");
    let work: i64 = result(&server.rpc("work.list", json!({})).await)["works"]
        .as_array()
        .unwrap()
        .iter()
        .find(|w| w["title"] == "muehle")
        .unwrap_or_else(|| panic!("Titel aus dem Archivnamen (index.md sagt nichts über das Werk)"))["id"]
        .as_i64()
        .unwrap();
    let assets = server.state.store.asset_list(work).await.unwrap();
    assert_eq!(assets.len(), 1, "nur das gültige Bild ist angelegt: {assets:?}");
}

#[tokio::test]
async fn a_zip_upload_without_hints_redirects_to_the_work_and_keeps_the_image_paths() {
    let server = common::open().await;
    let typ = "#set document(title: \"Das Dünenheft\")\n= Die Düne\n\n#image(\"bilder/sand.svg\")\n\nSand.\n";
    let archive = zip_of(&[("main.typ", typ.as_bytes()), ("bilder/sand.svg", ROOF.as_bytes())]);
    let (status, headers, body) = server.send(upload("file", &[("duene.zip", &archive)], "")).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "ohne Hinweise wie bisher eine Weiterleitung: {body}");
    let work: i64 = headers[header::LOCATION].to_str().unwrap().strip_prefix("/?work=").unwrap().parse().unwrap();
    let meta = result(&server.rpc("work.meta", json!({ "work": work })).await).clone();
    assert_eq!(meta["work"]["title"], "Das Dünenheft", "Titel aus #set document der Haupt-Datei");
    let asset = server.state.store.asset_by_path(work, "bilder/sand.svg").await.unwrap();
    assert!(asset.is_some(), "das SVG gehört mit seinem Pfad zum Werk");
    assert_eq!(unkeyed(&export(&server, work).await), typ, "Typst aus dem Archiv byte-gleich übernommen");
}

#[tokio::test]
async fn a_folder_upload_with_relative_paths_brings_its_images() {
    let server = common::open().await;
    let md = "# Der Garten\n\nBeete und Wege.\n\n![Ein Beet](bilder/beet.png)\n";
    let png = tiny_png(200);
    let (status, headers, body) =
        server.send(upload("files", &[("garten/heft.md", md.as_bytes()), ("garten/bilder/beet.png", &png)], "")).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "ein Ordner ohne Hinweise leitet aufs Werk: {body}");
    let work: i64 = headers[header::LOCATION].to_str().unwrap().strip_prefix("/?work=").unwrap().parse().unwrap();
    let asset = server.state.store.asset_by_path(work, "bilder/beet.png").await.unwrap();
    assert!(asset.is_some(), "Bildpfad relativ zur Haupt-Datei im Ordner");
    let typst = export(&server, work).await;
    assert!(typst.contains("#image(\"bilder/beet.png\", alt: \"Ein Beet\")"), "Bild im Text: {typst}");

    // Datei und Ordner zugleich: nicht raten, welches gemeint ist.
    let mut request = upload("files", &[("x/a.md", b"# A\n")], "");
    let both = {
        let mut body = Vec::new();
        body.extend_from_slice(
            format!(
                "--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"b.md\"\r\n\r\n# B\n\r\n\
                 --{BOUNDARY}\r\nContent-Disposition: form-data; name=\"files\"; filename=\"x/a.md\"\r\n\r\n# A\n\r\n\
                 --{BOUNDARY}--\r\n"
            )
            .as_bytes(),
        );
        body
    };
    *request.body_mut() = Body::from(both);
    let before = work_count(&server).await;
    let (status, _, html) = server.send(request).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "Datei und Ordner zugleich → 422");
    assert!(html.contains("nicht beides"), "Meldung sagt warum: {html}");
    assert_eq!(work_count(&server).await, before, "nichts angelegt");
}

#[tokio::test]
async fn unsafe_archives_and_folders_are_refused_and_nothing_is_created() {
    let server = common::open().await;
    let before = work_count(&server).await;
    // Pfadflucht im Archiv.
    let escape = zip_of(&[("../flucht.md", b"# Flucht\n")]);
    let (status, _, html) = server.send(upload("file", &[("flucht.zip", &escape)], "")).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "../ im Archiv → 422: {html}");
    assert!(html.contains("unsicherer Eintrag"), "Meldung des Archivlesers: {html}");
    // Verknüpfung im Archiv.
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    writer.start_file("buch.md", SimpleFileOptions::default()).unwrap();
    writer.write_all(b"# Buch\n\n![x](bild.png)\n").unwrap();
    writer.add_symlink("bild.png", "/etc/passwd", SimpleFileOptions::default()).unwrap();
    let link = writer.finish().unwrap().into_inner();
    let (status, _, html) = server.send(upload("file", &[("link.zip", &link)], "")).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "Verknüpfung im Archiv → 422: {html}");
    assert!(html.contains("Verknüpfung"), "Meldung nennt die Verknüpfung: {html}");
    // Zip-Bombe: 4 MiB Nullen packen auf wenige KiB (Verhältnis weit über 200).
    let bomb = zip_of(&[("buch.md", b"# Buch\n"), ("null.bin", &vec![0u8; 4 * 1024 * 1024])]);
    assert!(bomb.len() < 64 * 1024, "Testvoraussetzung: stark gepackt ({} Bytes)", bomb.len());
    let (status, _, html) = server.send(upload("file", &[("bombe.zip", &bomb)], "")).await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE, "Zip-Bombe → 413: {html}");
    // Ordner mit Pfadflucht im Dateinamen (ein Browser schickt das nie, ein Skript schon).
    let (status, _, html) = server.send(upload("files", &[("../a.md", b"# A\n")], "")).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "../ im Ordner-Upload → 422: {html}");
    // Kein ZIP.
    let (status, _, html) = server.send(upload("file", &[("kaputt.zip", b"kein Archiv")], "")).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "kein ZIP → 422: {html}");
    assert!(html.contains("kein ZIP-Archiv"), "Meldung: {html}");
    // Archiv ohne Haupt-Datei bzw. mit zwei möglichen.
    let none = zip_of(&[("bild.png", &tiny_png(1))]);
    let (status, _, html) = server.send(upload("file", &[("leer.zip", &none)], "")).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "ohne Haupt-Datei → 422: {html}");
    let two = zip_of(&[("a.md", b"# A\n"), ("b.md", b"# B\n")]);
    let (status, _, html) = server.send(upload("file", &[("zwei.zip", &two)], "")).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "zwei Kandidaten → 422: {html}");
    assert!(html.contains("a.md") && html.contains("b.md"), "Meldung nennt die Kandidaten: {html}");
    assert_eq!(work_count(&server).await, before, "kein einziges Werk angelegt");
}

#[tokio::test]
async fn the_upload_limit_is_80_mib_only_at_the_import_route() {
    assert_eq!(baruch_server::works::IMPORT_UPLOAD_LIMIT, 80 * 1024 * 1024 + 64 * 1024, "80 MiB + 64 KiB Rahmen");
    assert_eq!(
        baruch_server::works::UPLOAD_LIMIT,
        16 * 1024 * 1024 + 64 * 1024,
        "die alte Grenze für eine Datei bleibt (Vertrag R21: UPLOAD_LIMIT unverändert)"
    );
    assert_eq!(
        baruch_server::archive::IMPORT_LIMITS.max_total_bytes,
        80 * 1024 * 1024,
        "Archiv- und Ordnergrenze passen zur Anfragegrenze"
    );
    let server = common::open().await;
    let before = work_count(&server).await;
    // Eine Datei über 16 MiB im Ordner: Grenze je Eintrag (IMPORT_LIMITS) → 413.
    let big = vec![b'x'; 16 * 1024 * 1024 + 1];
    let (status, _, html) = server.send(upload("files", &[("r/a.md", b"# A\n"), ("r/gross.bin", &big)], "")).await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE, "Eintrag über 16 MiB → 413: {html}");
    assert!(html.contains("größer als 16 MiB"), "Meldung nennt die Grenze: {html}");
    // Die ganze Anfrage über 80 MiB + 64 KiB: axum lehnt beim Lesen ab → 413 mit Formular.
    let huge = vec![b'y'; 80 * 1024 * 1024 + 64 * 1024];
    let (status, _, html) = server.send(upload("files", &[("r/a.md", b"# A\n"), ("r/riesig.bin", &huge)], "")).await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE, "Anfrage über der Grenze → 413");
    assert!(html.contains("80 MiB"), "Meldung nennt 80 MiB: {html}");
    assert_eq!(work_count(&server).await, before, "nichts angelegt");
}

// ---- RPC und CLI ---------------------------------------------------------------------------------------------------

#[tokio::test]
async fn rpc_imports_markdown_and_archives_with_warnings_and_refuses_wrong_endings_and_taken_titles() {
    let server = common::open().await;
    let md = "# Kai\n\nDie Mole.\n\n![Mole](bilder/mole.png)\n\n##### Tief\n";
    let report = result(&server.rpc("work.import", json!({ "file_name": "kai.md", "source": md })).await).clone();
    assert_eq!(report["title"], "kai", "Titel aus dem Dateinamen");
    assert!(codes(&report).contains(&"md-heading-level".to_owned()), "warnings im Bericht: {report}");
    assert_eq!(report["skipped"][0]["path"], "bilder/mole.png", "das Bild kam nicht mit und ist genannt: {report}");
    assert!(
        report["skipped"][0]["reason"].as_str().unwrap().contains("nur die Markdown-Datei"),
        "Grund sagt, dass nur Text ankam: {report}"
    );
    let typ =
        result(&server.rpc("work.import", json!({ "file_name": "dock.typ", "source": "= Dock\n" })).await).clone();
    assert_eq!(typ["warnings"], json!([]), "der Typst-Import hat jetzt ein leeres warnings-Feld: {typ}");

    let archive = zip_of(&[("hafen.md", b"# Hafen\n\nWasser.\n")]);
    let content = baruch_store::encode_base64(&archive);
    let zipped =
        result(&server.rpc("work.import_archive", json!({ "file_name": "hafen.zip", "content": content })).await)
            .clone();
    assert_eq!((zipped["title"].as_str(), zipped["nodes"].as_i64()), (Some("hafen"), Some(1)), "{zipped}");
    assert_eq!(zipped["warnings"], json!([]), "sauberes Markdown ohne Hinweise: {zipped}");

    let wrong = server.rpc("work.import_archive", json!({ "file_name": "hafen.md", "content": content })).await;
    assert_eq!(error_code(&wrong), rpc::INVALID_PARAMS, "falsche Endung bei import_archive → -32602");
    let wrong = server.rpc("work.import", json!({ "file_name": "hafen.zip", "source": "x" })).await;
    assert_eq!(error_code(&wrong), rpc::INVALID_PARAMS, "ZIP über work.import → -32602 mit Hinweis");
    let wrong = server.rpc("work.import", json!({ "file_name": "hafen.txt", "source": "= x\n" })).await;
    assert_eq!(error_code(&wrong), rpc::INVALID_PARAMS, "andere Endung → -32602");
    let bad = server.rpc("work.import_archive", json!({ "file_name": "b.zip", "content": "%%%" })).await;
    assert_eq!(error_code(&bad), rpc::INVALID_PARAMS, "kein Base64 → -32602");
    let other = zip_of(&[("hafen.md", b"# Hafen\n\nAnderes Wasser.\n")]);
    let taken = server
        .rpc("work.import_archive", json!({ "file_name": "hafen.zip", "content": baruch_store::encode_base64(&other) }))
        .await;
    assert_eq!(error_code(&taken), rpc::REFUSED, "vergebener Titel → REFUSED: {taken}");
    assert!(taken["error"]["message"].as_str().unwrap().contains("--title"), "mit Hinweis auf einen anderen Titel");
}

#[test]
fn the_cli_imports_markdown_and_zip_and_prints_report_and_hints() {
    let data = tempfile::tempdir().unwrap();
    let folder = tempfile::tempdir().unwrap();
    let md = "# Die Bucht\n\nWellen.\n\n![Boot](bilder/boot.png)\n\n#### Tief unten\n";
    std::fs::create_dir_all(folder.path().join("bilder")).unwrap();
    std::fs::write(folder.path().join("bucht.md"), md).unwrap();
    std::fs::write(folder.path().join("bilder/boot.png"), tiny_png(30)).unwrap();
    let (code, out, err) = cli(data.path(), &["import", folder.path().join("bucht.md").to_str().unwrap()]);
    assert_eq!(code, 0, "Markdown-Import gelingt: {out}{err}");
    assert!(out.contains("Werk: bucht"), "Bericht nennt das Werk: {out}");
    assert!(out.contains("Bilder: 1"), "das Bild neben der Datei ist gelesen (relativ zur Datei): {out}");
    assert!(out.contains("Hinweis (md-heading-level, Zeile 7)"), "Hinweis mit Code und Zeile: {out}");

    let archive = zip_of(&[("klippe/index.md", b"# Klippe\n\nFels.\n")]);
    let zip_path = folder.path().join("klippe.zip");
    std::fs::write(&zip_path, archive).unwrap();
    let (code, out, err) = cli(data.path(), &["import", zip_path.to_str().unwrap(), "--title", "Die Klippe"]);
    assert_eq!(code, 0, "ZIP-Import gelingt: {out}{err}");
    assert!(out.contains("Werk: Die Klippe"), "--title gilt auch hier: {out}");
    let (code, _, err) = cli(data.path(), &["import", zip_path.to_str().unwrap(), "--title", "Die Klippe"]);
    assert_eq!(code, 1, "zweiter Import mit demselben Titel scheitert");
    assert!(err.contains("Die Klippe"), "Fehler nennt das vorhandene Werk: {err}");
    let (code, _, err) = cli(data.path(), &["import", zip_path.to_str().unwrap(), "--uebernehmen"]);
    assert_eq!(code, 2, "PDF-Flags gelten nicht für ZIP (Weiche aus S1): {err}");
}

// ---- export --out prüft vorab --------------------------------------------------------------------------------------

/// Alle Dateien unter `dir` (relativ, sortiert), Verknüpfungen nicht verfolgt.
fn files_under(dir: &std::path::Path) -> Vec<String> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&d) else { continue };
        for entry in entries {
            let entry = entry.unwrap();
            let kind = entry.file_type().unwrap();
            if kind.is_dir() {
                stack.push(entry.path());
            } else {
                out.push(entry.path().strip_prefix(dir).unwrap().display().to_string());
            }
        }
    }
    out.sort();
    out
}

#[cfg(unix)]
#[tokio::test]
async fn export_out_refuses_a_symlinked_folder_and_a_file_folder_clash_before_writing_anything() {
    let server = common::open().await;
    let typ = "#set document(title: \"Das Kellerheft\")\n= Keller\n\n#image(\"bilder/fass.png\")\n";
    let archive = zip_of(&[("keller.typ", typ.as_bytes()), ("bilder/fass.png", &tiny_png(50))]);
    let keller = markdown_work(&server, &archive, "keller.zip").await;

    let tmp = tempfile::tempdir().unwrap();
    let outside = tmp.path().join("draussen");
    std::fs::create_dir_all(&outside).unwrap();
    let out = tmp.path().join("aus");
    std::fs::create_dir_all(&out).unwrap();
    std::os::unix::fs::symlink(&outside, out.join("bilder")).unwrap();
    let error = baruch_server::import::export_to_dir(&server.state.store, keller, &out).await.unwrap_err();
    assert!(error.contains("Verknüpfung") && error.contains("nichts geschrieben"), "Grund und Zusage: {error}");
    assert_eq!(files_under(&outside), Vec::<String>::new(), "kein Bild ist außerhalb von --out gelandet");
    assert_eq!(
        files_under(&out),
        vec!["bilder".to_owned()],
        "auch die Typst-Datei ist nicht geschrieben (nur der Link)"
    );

    // a.png und a.png/b.svg: eine Datei und ein Ordner gleichen Namens.
    let clash = "#set document(title: \"Das Doppelheft\")\n= Doppel\n\n#image(\"a.png\")\n#image(\"a.png/b.svg\")\n";
    let archive =
        zip_of(&[("doppel.typ", clash.as_bytes()), ("a.png", &tiny_png(70)), ("a.png/b.svg", ROOF.as_bytes())]);
    let work = markdown_work(&server, &archive, "doppel.zip").await;
    let target = tmp.path().join("doppel");
    let error = baruch_server::import::export_to_dir(&server.state.store, work, &target).await.unwrap_err();
    assert!(error.contains("zugleich Datei und Ordner"), "Kollision erkannt: {error}");
    assert!(!target.exists(), "nicht einmal der Zielordner ist angelegt: vorab geprüft");

    // Gegenprobe: ohne Hindernis schreibt export --out alles.
    let clean = tmp.path().join("sauber");
    let written = baruch_server::import::export_to_dir(&server.state.store, keller, &clean).await.unwrap();
    assert_eq!(written.len(), 2, "Typst-Datei und Bild: {written:?}");
}

/// Archiv über RPC importieren, Werk-id zurück.
async fn markdown_work(server: &common::Server, archive: &[u8], name: &str) -> i64 {
    let answer = server
        .rpc("work.import_archive", json!({ "file_name": name, "content": baruch_store::encode_base64(archive) }))
        .await;
    result(&answer)["work"].as_i64().unwrap()
}

// ---- Buchende, erste Erwähnung, Satz-Regeln, „Nummer aus“ -----------------------------------------------------------

/// Erfundenes Werk für die Rundreisen.
const HARBOUR: &str = "#set document(title: \"Das Hafenbuch\")\n= Ankunft\n\nDas Leuchtfeuer brennt am Kai.\n\n\
                       = Abfahrt\n\nWieder das Leuchtfeuer.\n";

#[tokio::test]
async fn a_hand_written_back_matter_line_cuts_nothing_and_is_reported() {
    let server = common::open().await;
    let source = format!("{HARBOUR}\n{}\nMein Nachwort bleibt.\n", baruch_typst::index::BACK_MATTER_MARK);
    let answer = server.rpc("work.import", json!({ "file_name": "hafen.typ", "source": source })).await;
    let report = result(&answer).clone();
    assert!(codes(&report).contains(&"import-back-matter-kept".to_owned()), "die Kennzeile ist gemeldet: {report}");
    assert!(!codes(&report).contains(&"import-back-matter-cut".to_owned()), "aber nicht geschnitten: {report}");
    let typst = export(&server, report["work"].as_i64().unwrap()).await;
    assert!(typst.contains("Mein Nachwort bleibt."), "der Text hinter der Kennzeile von Hand bleibt: {typst}");
}

#[tokio::test]
async fn a_generated_back_matter_is_cut_and_reported() {
    let server = common::open().await;
    let work = server.import(HARBOUR, "hafen.typ").await.work;
    let pages = TypesetSettings {
        index_pages: Some(IndexPages { registers: vec!["stichwort".into()], columns: 1 }),
        ..TypesetSettings::default()
    };
    server.state.store.set_work_typeset(work, &pages, "Autor").await.unwrap();
    let exported = export(&server, work).await;
    assert!(exported.contains(baruch_typst::index::BACK_MATTER_MARK), "Testvoraussetzung: Buchende im Export");
    let answer = server
        .rpc("work.import", json!({ "file_name": "hafen-export.typ", "source": exported, "title": "Hafen, Abzug" }))
        .await;
    let report = result(&answer).clone();
    let cut = report["warnings"].as_array().unwrap().iter().find(|w| w["code"] == "import-back-matter-cut").cloned();
    let cut = cut.unwrap_or_else(|| panic!("der Schnitt steht im Bericht: {report}"));
    let mark_line = exported.lines().position(|l| l == baruch_typst::index::BACK_MATTER_MARK).unwrap() + 1;
    assert_eq!(cut["line"], mark_line, "der Hinweis nennt die Zeile der Kennzeile");
    let copy = export(&server, report["work"].as_i64().unwrap()).await;
    assert!(
        !copy.contains("baruch-idx-ranges"),
        "das erzeugte Buchende wanderte nicht in den letzten Abschnitt: {copy}"
    );
    server.state.store.reset_work_typeset(work, "Autor").await.unwrap();
    assert_eq!(unkeyed(&copy), unkeyed(&export(&server, work).await), "sonst ist der Text gleich");
}

#[tokio::test]
async fn a_first_mention_without_glossary_page_survives_export_and_import() {
    let server = common::open().await;
    let work = server.import(HARBOUR, "hafen.typ").await.work;
    result(&server.rpc("glossary.add", json!({ "work": work, "term": "Leuchtfeuer", "short": "Ein Licht." })).await);
    let first = TypesetSettings { glossary_first_mention: Some(true), ..TypesetSettings::default() };
    server.state.store.set_work_typeset(work, &first, "Autor").await.unwrap();
    let exported = export(&server, work).await;
    assert!(
        exported.contains(&format!("{}#emph[Leuchtfeuer]", baruch_typst::index::FIRST_MENTION_MARK)),
        "Testvoraussetzung: die erste Erwähnung ist ausgezeichnet und vermerkt: {exported}"
    );
    assert!(!exported.contains(baruch_typst::index::BACK_MATTER_MARK), "ohne Glossarseite kein Buchende");
    let answer = server
        .rpc("work.import", json!({ "file_name": "hafen-export.typ", "source": exported, "title": "Hafen, Abzug" }))
        .await;
    let report = result(&answer).clone();
    assert!(
        codes(&report).contains(&"import-projection-removed".to_owned()),
        "der Eingriff steht im Bericht: {report}"
    );
    let copy = export(&server, report["work"].as_i64().unwrap()).await;
    // Das neue Werk hat weder Glossar noch die Einstellung: sein Export muss der Text vor der Projektion sein.
    server.state.store.reset_work_typeset(work, "Autor").await.unwrap();
    assert_eq!(
        unkeyed(&copy),
        unkeyed(&export(&server, work).await),
        "Export → Import ändert den Text nicht (die Hervorhebung wird nicht Teil des Buchtexts)"
    );
}

#[tokio::test]
async fn the_typeset_rules_block_returns_into_the_typeset_settings_of_the_new_work() {
    let server = common::open().await;
    let work = server.import(HARBOUR, "hafen.typ").await.work;
    let settings = TypesetSettings {
        chapter_start: Some(ChapterStart::NewPage),
        font_size_pt: Some(11.0),
        justify: Some(true),
        page_numbers: Some(PageNumbers {
            position: PageNumberPosition::BottomCenter,
            start: PageNumberStart::FirstPage,
        }),
        ..TypesetSettings::default()
    };
    server.state.store.set_work_typeset(work, &settings, "Autor").await.unwrap();
    let exported = export(&server, work).await;
    assert!(exported.contains(baruch_typst::typeset::RULES_MARK), "Testvoraussetzung: Regelblock im Export");
    let answer = server
        .rpc(
            "work.import",
            json!({ "file_name": "hafen-export.typ", "source": exported.clone(), "title": "Zweitdruck" }),
        )
        .await;
    let report = result(&answer).clone();
    let copy = report["work"].as_i64().unwrap();
    assert!(codes(&report).contains(&"import-typeset-adopted".to_owned()), "Übernahme im Bericht: {report}");
    let stored = server.state.store.work_typeset(copy).await.unwrap().expect("Satz-Einstellungen angelegt");
    let original = server.state.store.work_typeset(work).await.unwrap().unwrap();
    assert_eq!(stored.settings, original.settings, "dieselben Satz-Einstellungen wie das Ausgangswerk");
    let template = server.state.store.work_template(copy).await.unwrap();
    assert!(
        !template.contains(baruch_typst::typeset::RULES_MARK),
        "der Block steht nicht in der Werkvorlage: {template}"
    );
    assert_eq!(unkeyed(&export(&server, copy).await), unkeyed(&exported), "der neue Export ist textgleich");

    // Von Hand geändert: nicht raten, in der Vorlage lassen und melden.
    let edited = exported.replace("#set par(justify: true)\n", "#set par(justify: true)\n#set text(fill: blue)\n");
    let answer = server
        .rpc("work.import", json!({ "file_name": "hafen-hand.typ", "source": edited, "title": "Handdruck" }))
        .await;
    let report = result(&answer).clone();
    assert!(codes(&report).contains(&"import-typeset-kept".to_owned()), "Block bleibt in der Vorlage: {report}");
    let hand = report["work"].as_i64().unwrap();
    assert!(server.state.store.work_typeset(hand).await.unwrap().is_none(), "keine Satz-Einstellungen geraten");
}

#[tokio::test]
async fn number_off_with_its_own_label_and_under_no_numbering_survives_export_and_import() {
    let server = common::open().await;
    let source = "#set document(title: \"Das Mühlenbuch\")\n#set heading(numbering: \"1.\")\n= Vorwort <vorwort>\n\n\
                  Kurz.\n\n= Mühle\n\nMehl.\n\n= Nachwort\n\nEnde.\n";
    let work = server.import(source, "muehle.typ").await.work;
    let preface = key_of(&server, work, "Vorwort").await;
    result(&server.rpc("outline.set_numbered", json!({ "work": work, "key": preface, "numbered": false })).await);
    let exported = export(&server, work).await;
    assert!(exported.contains("// Baruch: Überschriften ohne Nummer"), "Testvoraussetzung: Show-Regel des Rückfalls");
    let answer = server
        .rpc("work.import", json!({ "file_name": "muehle-export.typ", "source": exported.clone(), "title": "Mühle 2" }))
        .await;
    let copy = result(&answer)["work"].as_i64().unwrap();
    assert_eq!(numbered(&server, copy, "Vorwort").await, 0, "„Nummer aus“ mit eigenem Label bleibt aus");
    assert_eq!(numbered(&server, copy, "Mühle").await, 1, "die anderen zählen weiter");
    let template = server.state.store.work_template(copy).await.unwrap();
    assert!(!template.contains("Überschriften ohne Nummer"), "die Regel ist Merker, nicht Vorlage: {template}");
    assert_eq!(unkeyed(&export(&server, copy).await), unkeyed(&exported), "und der Export bleibt textgleich");

    // „Ohne Nummern“: die Textform bleibt (= Zeile), der Merker reist als Show-Regel mit.
    let epilogue = key_of(&server, work, "Nachwort").await;
    result(&server.rpc("outline.set_numbered", json!({ "work": work, "key": epilogue, "numbered": false })).await);
    server.state.store.set_numbering_scheme(work, "none", None).await.unwrap();
    let plain = export(&server, work).await;
    assert!(unkeyed(&plain).contains("= Nachwort\n"), "unter „Ohne Nummern“ Text wie gespeichert: {plain}");
    let answer =
        server.rpc("work.import", json!({ "file_name": "muehle-ohne.typ", "source": plain, "title": "Mühle 3" })).await;
    let none = result(&answer)["work"].as_i64().unwrap();
    assert_eq!(numbered(&server, none, "Nachwort").await, 0, "„Nummer aus“ unter „Ohne Nummern“ bleibt aus");
    assert_eq!(numbered(&server, none, "Vorwort").await, 0, "ebenso das Vorwort mit eigenem Label");
    assert_eq!(numbered(&server, none, "Mühle").await, 1, "ein nummerierter Abschnitt bleibt nummeriert");
}
