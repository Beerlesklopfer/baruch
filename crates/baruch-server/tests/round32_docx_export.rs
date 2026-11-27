// SPDX-License-Identifier: AGPL-3.0-or-later
//! Round 32, task docx_export: Word export (page, file, RPC work.docx_export, CLI).
//!
//! Real data directory, freely invented work, no model, no network. The file is checked with own small reading helpers
//! (zip, search in the XML), not with the reader of the neighbouring task.

use crate::common;

use std::io::{Cursor, Read};
use std::process::Command;

use axum::body::{Body, to_bytes};
use axum::http::{HeaderMap, Request, StatusCode, header};
use baruch_server::docx_export::{DocxExportOptions, cli_export, export_file};
use serde_json::{Value, json};
use tower::ServiceExt;

const BOOK: &str = "#set document(title: \"Der Steg am Weiher\", author: \"Malte Rohr\")\n\n\
= Der Weiher\n\nAm Morgen lag Nebel über dem Wasser.#footnote[Der Weiher ist erfunden.] Der alte Steg knarrte leise.\n\n\
#figure(image(\"bilder/steg.png\"), caption: [Der Steg im Nebel])\n\n\
== Das Ufer\n\nIm Schilf saß *ein* Reiher und wartete.\n\n\
= Der Abend\n\nAm Abend kamen die Mücken.\n\n#let rahmen(inhalt) = box(stroke: 1pt)[#inhalt]\n";

const MEDIA_TYPE: &str = "application/vnd.openxmlformats-officedocument.wordprocessingml.document";

fn result(answer: &Value) -> &Value {
    assert!(answer.get("error").is_none(), "RPC-Fehler: {answer}");
    &answer["result"]
}

fn error(answer: &Value) -> (i64, String) {
    let e = answer.get("error").unwrap_or_else(|| panic!("Fehler erwartet: {answer}"));
    (e["code"].as_i64().unwrap(), e["message"].as_str().unwrap().to_owned())
}

struct Book {
    work: i64,
    slug: String,
}

/// The work with an embedded image (`bilder/steg.png`, alternative text in the image details).
async fn setup(server: &common::Server) -> Book {
    let report = server.import(BOOK, "steg.typ").await;
    let added = server
        .rpc(
            "asset.add",
            json!({ "work": report.work, "file_name": "steg.png", "path": "bilder/steg.png", "encoding": "base64",
                    "content": baruch_store::encode_base64(baruch_docx::sample::SAMPLE_PNG),
                    "alt": "Ein Steg aus grauen Brettern" }),
        )
        .await;
    result(&added);
    Book { work: report.work, slug: report.slug }
}

/// Current text of the section `node` with one replacement.
async fn changed(server: &common::Server, work: i64, node: usize, from: &str, to: &str) -> String {
    let loaded = server.state.store.load_work(work).await.unwrap();
    let n = &loaded.nodes[node];
    let body = &n.revisions[n.current_index().unwrap()].body;
    assert!(body.contains(from), "Abschnitt {node} enthält „{from}“ nicht: {body}");
    body.replacen(from, to, 1)
}

async fn get_raw(server: &common::Server, uri: &str, headers: &[(&str, &str)]) -> (StatusCode, HeaderMap, Vec<u8>) {
    let mut request = Request::get(uri);
    for (name, value) in headers {
        request = request.header(*name, *value);
    }
    let response = server.app.clone().oneshot(request.body(Body::empty()).unwrap()).await.unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    (status, headers, to_bytes(response.into_body(), usize::MAX).await.unwrap().to_vec())
}

/// One part of the Word file as text.
fn part(docx: &[u8], name: &str) -> String {
    let mut zip = zip::ZipArchive::new(Cursor::new(docx)).expect("die Antwort ist ein ZIP-Archiv");
    let mut entry = zip.by_name(name).unwrap_or_else(|_| panic!("Teil {name} fehlt in der Word-Datei"));
    let mut text = String::new();
    entry.read_to_string(&mut text).expect("Teil ist UTF-8");
    text
}

fn names(docx: &[u8]) -> Vec<String> {
    let mut zip = zip::ZipArchive::new(Cursor::new(docx)).expect("die Antwort ist ein ZIP-Archiv");
    (0..zip.len())
        .map(|i| zip.by_index(i).expect("Eintrag lesbar").name().expect("Name des Eintrags lesbar").into_owned())
        .collect()
}

/// Value of the attribute `attr` in the first element `<tag …>` after `from`.
fn attribute(xml: &str, tag: &str, attr: &str) -> String {
    let at = xml.find(&format!("<{tag} ")).unwrap_or_else(|| panic!("<{tag}> fehlt: {xml}"));
    let element = &xml[at..at + xml[at..].find('>').unwrap()];
    let key = format!(" {attr}=\"");
    let start = element.find(&key).unwrap_or_else(|| panic!("{attr} fehlt in {element}")) + key.len();
    element[start..start + element[start..].find('"').unwrap()].to_owned()
}

/// State of the work that the export must not change: all revisions including text, the posts and the head of the
/// mirror.
async fn snapshot(server: &common::Server, book: &Book) -> (String, String, Option<String>) {
    let store = &server.state.store;
    let tree = serde_json::to_string(&store.revision_tree(book.work).await.unwrap()).unwrap();
    let loaded = store.load_work(book.work).await.unwrap();
    let texts: Vec<String> = loaded
        .nodes
        .iter()
        .flat_map(|n| n.revisions.iter().map(|r| format!("{}#{}#{:?}#{}", n.key, r.number, r.status, r.body)))
        .chain(loaded.nodes.iter().map(|n| format!("{} Beiträge: {}", n.key, n.posts.len())))
        .collect();
    let head = common::git(&server.mirror_dir_of(&book.slug), &["rev-parse", "HEAD"]);
    (tree, texts.join("\n"), head)
}

#[tokio::test]
async fn rpc_and_download_deliver_the_same_word_file() {
    let server = common::open().await;
    let book = setup(&server).await;

    let answer = server.rpc("work.docx_export", json!({ "work": book.work })).await;
    let exported = result(&answer);
    assert_eq!(exported["file_name"], format!("{}.docx", book.slug), "die Datei heißt wie der Ordner des Werks");
    assert_eq!(exported["proposals"], 0, "ohne die Wahl stehen keine Vorschläge in der Datei");
    let bytes = baruch_store::decode_base64(exported["data_base64"].as_str().unwrap()).unwrap();
    assert_eq!(exported["bytes"], bytes.len(), "bytes nennt die Länge der Datei");

    let (status, headers, file) = get_raw(&server, &format!("/werke/{}/word.docx", book.work), &[]).await;
    assert_eq!(status, StatusCode::OK, "die Datei wird ausgeliefert");
    assert_eq!(headers.get(header::CONTENT_TYPE).unwrap(), MEDIA_TYPE, "Medientyp einer Word-Datei");
    assert_eq!(
        headers.get(header::CONTENT_DISPOSITION).unwrap().to_str().unwrap(),
        format!("attachment; filename=\"{}.docx\"", book.slug),
        "als Anhang zum Speichern mit dem Namen <slug>.docx"
    );
    assert_eq!(file, bytes, "RPC und Download liefern dieselbe Datei (gleicher Stand → gleiche Bytes)");

    let by_slug = server.rpc("work.docx_export", json!({ "work": book.slug })).await;
    assert_eq!(
        result(&by_slug)["data_base64"],
        exported["data_base64"],
        "das Werk lässt sich auch über den Ordnernamen nennen"
    );

    let all = names(&file);
    for wanted in
        ["[Content_Types].xml", "word/document.xml", "word/styles.xml", "word/footnotes.xml", "word/media/image1.png"]
    {
        assert!(all.iter().any(|n| n == wanted), "{wanted} gehört in die Datei: {all:?}");
    }
    let document = part(&file, "word/document.xml");
    assert!(
        document
            .contains("<w:pStyle w:val=\"Title\"/></w:pPr><w:r><w:t xml:space=\"preserve\">Der Steg am Weiher</w:t>"),
        "die Titelei beginnt mit dem Titel aus den Werkangaben: {document}"
    );
    assert!(
        document.contains("<w:pStyle w:val=\"BaruchAutor\"/>") && document.contains("Malte Rohr"),
        "und nennt den Autor"
    );
    assert_eq!(document.matches("<w:pStyle w:val=\"Heading1\"/>").count(), 2, "zwei Kapitel als Überschrift 1");
    assert_eq!(document.matches("<w:pStyle w:val=\"Heading2\"/>").count(), 1, "ein Abschnitt als Überschrift 2");
    let order: Vec<usize> = ["Der Weiher", "Das Ufer", "Der Abend"].iter().map(|t| document.find(t).unwrap()).collect();
    assert!(order[0] < order[1] && order[1] < order[2], "die Abschnitte stehen in Lesereihenfolge");
    assert_eq!(document.matches("<w:footnoteReference ").count(), 1, "die Fußnote ist ein echtes Fußnotenzeichen");
    assert!(part(&file, "word/footnotes.xml").contains("Der Weiher ist erfunden."), "ihr Text steht in footnotes.xml");
    assert!(!document.contains("Der Weiher ist erfunden."), "und nicht im Fließtext");
    assert_eq!(
        attribute(&document, "wp:docPr", "descr"),
        "Ein Steg aus grauen Brettern",
        "Alternativtext aus den Bildangaben des Werks"
    );
    assert!(
        document.contains("<w:pStyle w:val=\"Caption\"/>") && document.contains("Der Steg im Nebel"),
        "Bildunterschrift"
    );
    assert!(document.contains("<w:b/>"), "fett bleibt fett");
    assert!(
        !document.contains("<w:ins ") && !document.contains("<w:del "),
        "ohne die Wahl gibt es keine Änderungsverfolgung"
    );

    let warnings = exported["warnings"].as_array().unwrap();
    assert!(
        warnings
            .iter()
            .any(|w| w["code"] == "docx_typst_insel" && w["message"].as_str().unwrap().contains("Der Abend")),
        "die #let-Zeile kann Word nicht abbilden – das meldet der Export mit dem Abschnitt: {warnings:?}"
    );
    assert!(
        document.contains("#let rahmen(inhalt) = box(stroke: 1pt)[#inhalt]"),
        "ihr Quelltext steht wörtlich in der Datei"
    );
    assert!(
        part(&file, "docProps/app.xml").contains(&format!("Baruch {}", env!("CARGO_PKG_VERSION"))),
        "erzeugt von Baruch <Version>"
    );
}

#[tokio::test]
async fn the_latest_open_proposal_of_each_section_becomes_tracked_changes() {
    let server = common::open().await;
    let book = setup(&server).await;
    let store = &server.state.store;
    // Section 0: two open proposals (the most recent one, of @kritiker, counts).
    let older = changed(&server, book.work, 0, "knarrte leise", "knarrte laut").await;
    store.add_proposal(book.work, 0, &older, "@lektor", "probe", "laut").await.unwrap();
    let latest = changed(&server, book.work, 0, "Der alte Steg", "Der morsche Steg").await;
    let latest_number = store.add_proposal(book.work, 0, &latest, "@kritiker", "probe", "morsch").await.unwrap();
    // Section 1: a discarded proposal, after it a new current revision and the mark „bis hierhin“ – it is hidden and
    // does not count.
    let rejected = changed(&server, book.work, 1, "Reiher und", "grauer Reiher und").await;
    let number = store.add_proposal(book.work, 1, &rejected, "@lektor", "probe", "grau").await.unwrap();
    store.reject(book.work, 1, number).await.unwrap();
    let rewritten = changed(&server, book.work, 1, "und wartete", "und wartete still").await;
    store.save_human(book.work, 1, &rewritten, "Malte").await.unwrap().unwrap();
    let loaded = store.load_work(book.work).await.unwrap();
    store.set_revision_fold(book.work, 1, loaded.nodes[1].current_number().unwrap(), "Malte").await.unwrap();

    let answer = server.rpc("work.docx_export", json!({ "work": book.work, "proposals": true })).await;
    let exported = result(&answer);
    assert_eq!(
        exported["proposals"], 1,
        "ein Abschnitt hat einen offenen Vorschlag, der als Änderungsverfolgung in der Datei steht"
    );
    let bytes = baruch_store::decode_base64(exported["data_base64"].as_str().unwrap()).unwrap();
    let (status, _, file) = get_raw(&server, &format!("/werke/{}/word.docx?vorschlaege=1", book.work), &[]).await;
    assert_eq!(status, StatusCode::OK, "Download mit Vorschlägen");
    assert_eq!(file, bytes, "vorschlaege=1 und proposals: true sind dieselbe Wahl");

    let document = part(&file, "word/document.xml");
    assert_eq!(
        attribute(&document, "w:del", "w:author"),
        "@kritiker",
        "die Löschung nennt die Persona des jüngsten Vorschlags"
    );
    assert_eq!(attribute(&document, "w:ins", "w:author"), "@kritiker", "die Einfügung ebenso");
    assert!(
        document.contains("<w:delText xml:space=\"preserve\">alte</w:delText>"),
        "das ersetzte Wort ist gelöscht: {document}"
    );
    assert!(document.contains("<w:t xml:space=\"preserve\">morsche</w:t>"), "das neue Wort ist eingefügt");
    assert_eq!(
        document.matches("<w:del ").count(),
        1,
        "genau eine Löschung – nur der jüngste Vorschlag des Abschnitts"
    );
    assert_eq!(
        document.matches("<w:ins ").count(),
        1,
        "und genau eine Einfügung; der verworfene Vorschlag in Abschnitt 1 zählt nicht"
    );
    assert!(
        !document.contains("laut") && !document.contains("grauer"),
        "weder der ältere noch der verworfene Vorschlag steht in der Datei"
    );
    assert!(document.contains("und wartete still"), "Abschnitt 1 steht mit seiner aktuellen Fassung da");
    let tree = store.revision_tree(book.work).await.unwrap();
    let created = &tree[0].revisions.iter().find(|r| r.number == latest_number).unwrap().created;
    let date = attribute(&document, "w:ins", "w:date");
    assert_eq!(date.len(), 20, "das Datum ist ISO 8601 in UTC ohne Bruchteile: {date}");
    assert!(
        date.ends_with('Z') && created.starts_with(&date[..10]),
        "und der Tag, an dem der Vorschlag angelegt wurde: {date} / {created}"
    );

    let warnings = exported["warnings"].as_array().unwrap();
    let more: Vec<&str> = warnings
        .iter()
        .filter(|w| w["code"] == "docx_weitere_vorschlaege")
        .map(|w| w["message"].as_str().unwrap())
        .collect();
    assert_eq!(more.len(), 1, "genau ein Abschnitt hat weitere offene Vorschläge: {warnings:?}");
    assert!(
        more[0].contains("Der Weiher")
            && more[0].contains("1 weiterer offener Vorschlag")
            && more[0].contains("@kritiker"),
        "die Warnung nennt Abschnitt, Zahl und wessen Vorschlag in der Datei steht: {}",
        more[0]
    );

    let (_, _, plain) = get_raw(&server, &format!("/werke/{}/word.docx", book.work), &[]).await;
    let plain = part(&plain, "word/document.xml");
    assert!(
        !plain.contains("<w:ins ") && !plain.contains("<w:del "),
        "ohne die Wahl steht keine Änderung in der Datei"
    );
    assert!(plain.contains("Der alte Steg") && !plain.contains("morsche"), "sondern nur die aktuelle Fassung");
}

#[tokio::test]
async fn the_export_changes_nothing_in_the_work() {
    let server = common::open().await;
    let book = setup(&server).await;
    let proposal = changed(&server, book.work, 2, "die Mücken", "die Fledermäuse").await;
    server.state.store.add_proposal(book.work, 2, &proposal, "@lektor", "probe", "Tiere").await.unwrap();
    let before = snapshot(&server, &book).await;

    result(&server.rpc("work.docx_export", json!({ "work": book.work })).await);
    result(&server.rpc("work.docx_export", json!({ "work": book.work, "proposals": true })).await);
    let (status, _, _) = get_raw(&server, &format!("/werke/{}/word.docx?vorschlaege=1", book.work), &[]).await;
    assert_eq!(status, StatusCode::OK, "Download");
    let (status, _) = server.get(&format!("/werke/{}/word", book.work)).await;
    assert_eq!(status, StatusCode::OK, "Seite");

    let after = snapshot(&server, &book).await;
    assert_eq!(after.0, before.0, "Fassungen und ihr Status sind unverändert – der Vorschlag ist weiter offen");
    assert_eq!(after.1, before.1, "kein Text und kein Beitrag hat sich geändert");
    assert_eq!(after.2, before.2, "der Spiegel hat keinen neuen Commit (HEAD gleich)");
}

#[tokio::test]
async fn unknown_works_and_wrong_parameters_are_refused() {
    let server = common::open().await;
    let book = setup(&server).await;
    let (code, message) = error(&server.rpc("work.docx_export", json!({ "work": 9999 })).await);
    assert_eq!(code, -32602, "unbekanntes Werk (Zahl) → ungültige Parameter: {message}");
    let (code, _) = error(&server.rpc("work.docx_export", json!({ "work": "gibt-es-nicht" })).await);
    assert_eq!(code, -32602, "unbekanntes Werk (Ordnername) → ungültige Parameter");
    let (code, message) =
        error(&server.rpc("work.docx_export", json!({ "work": book.work, "vorschlaege": true })).await);
    assert_eq!(code, -32602, "ein unbekannter Parameter wird abgelehnt statt still übergangen: {message}");
    let (status, _, _) = get_raw(&server, "/werke/9999/word.docx", &[]).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "Datei eines unbekannten Werks");
    let (status, _) = server.get("/werke/9999/word").await;
    assert_eq!(status, StatusCode::NOT_FOUND, "Seite eines unbekannten Werks");
    let (status, _, body) = get_raw(&server, &format!("/werke/{}/word.docx?vorschlaege=ja", book.work), &[]).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "nur 1 oder 0 sind eine Wahl: {}", String::from_utf8_lossy(&body));
}

#[tokio::test]
async fn the_page_offers_the_choice_as_a_plain_get_form() {
    let server = common::open().await;
    let book = setup(&server).await;
    let (status, page) = server.get(&format!("/werke/{}/word", book.work)).await;
    assert_eq!(status, StatusCode::OK, "die Seite gibt es");
    let page = page.replace("&#x2f;", "/");
    let form =
        format!("<form class=\"work-form docx-export-form\" method=\"get\" action=\"/werke/{}/word.docx\">", book.work);
    assert!(page.contains(&form), "ein gewöhnliches GET-Formular auf die Datei – es geht ohne JavaScript: {page}");
    assert!(
        page.contains("<input type=\"checkbox\" name=\"vorschlaege\" value=\"1\">"),
        "die Wahl ist ein Häkchen, das vorschlaege=1 mitschickt"
    );
    assert!(
        page.contains("<button class=\"btn primary\" type=\"submit\">Word-Datei herunterladen</button>"),
        "mit einem Knopf zum Absenden"
    );
    assert!(page.contains("Zurzeit ist kein Vorschlag offen"), "ohne offene Vorschläge sagt die Seite das ehrlich");
    assert!(page.contains("Der Export ändert nichts am Werk."), "und dass der Export nichts ändert");

    let proposal = changed(&server, book.work, 2, "die Mücken", "die Fledermäuse").await;
    server.state.store.add_proposal(book.work, 2, &proposal, "@lektor", "probe", "Tiere").await.unwrap();
    let (_, page) = server.get(&format!("/werke/{}/word", book.work)).await;
    assert!(
        page.contains("ist 1 Vorschlag offen") && page.contains("1 Abschnitt"),
        "mit einem offenen Vorschlag nennt die Seite die Zahl: {page}"
    );
    let (_, workspace) = server.get(&format!("/?work={}", book.work)).await;
    assert!(
        workspace.replace("&#x2f;", "/").contains(&format!("href=\"/werke/{}/word\"", book.work)),
        "das Menüband führt zu dieser Seite (Eintrag aus der Schnittstelle S1)"
    );
}

#[tokio::test]
async fn a_foreign_site_does_not_get_the_file() {
    let server = common::open().await;
    let book = setup(&server).await;
    let uri = format!("/werke/{}/word.docx", book.work);
    let image = [("sec-fetch-site", "cross-site"), ("sec-fetch-mode", "no-cors"), ("sec-fetch-dest", "image")];
    let (status, _, body) = get_raw(&server, &uri, &image).await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "als Unterressource einer fremden Seite gibt es die Datei nicht (own_reads)"
    );
    assert!(!body.starts_with(b"PK"), "und auch keinen Inhalt der Datei");
    let fetch = [("sec-fetch-site", "same-site"), ("sec-fetch-mode", "cors"), ("sec-fetch-dest", "empty")];
    let (status, _, _) = get_raw(&server, &format!("{uri}?vorschlaege=1"), &fetch).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "auch nicht per fetch einer Nachbarseite");
    let link = [("sec-fetch-site", "cross-site"), ("sec-fetch-mode", "navigate"), ("sec-fetch-dest", "document")];
    let (status, _, _) = get_raw(&server, &uri, &link).await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "und nicht über einen Link von einer fremden Seite – die Datei entsteht wie PDF und EPUB nur auf eigene Anfrage"
    );
    let own = [("sec-fetch-site", "same-origin"), ("sec-fetch-mode", "navigate"), ("sec-fetch-dest", "document")];
    let (status, _, body) = get_raw(&server, &uri, &own).await;
    assert_eq!(status, StatusCode::OK, "die eigene Seite bekommt sie");
    assert!(body.starts_with(b"PK"), "als ZIP-Archiv");
}

#[tokio::test]
async fn cli_export_writes_the_file_and_never_overwrites() {
    let server = common::open().await;
    let book = setup(&server).await;
    let proposal = changed(&server, book.work, 2, "die Mücken", "die Fledermäuse").await;
    server.state.store.add_proposal(book.work, 2, &proposal, "@lektor", "probe", "Tiere").await.unwrap();
    let dir = tempfile::tempdir().unwrap();

    let plain = dir.path().join("steg.docx");
    let text = cli_export(&server.state, book.work, &plain, &DocxExportOptions::default()).await.unwrap();
    let written = std::fs::read(&plain).unwrap();
    let file = export_file(&server.state, book.work, &DocxExportOptions::default()).await.unwrap();
    assert_eq!(written, file.bytes, "in der Datei stehen genau die Bytes des Exports");
    assert!(
        text.contains("Word-Datei geschrieben") && text.contains("steg.docx"),
        "die Ausgabe nennt die Datei: {text}"
    );
    assert!(text.contains("docx_typst_insel"), "und die Warnungen: {text}");
    assert!(!part(&written, "word/document.xml").contains("<w:ins "), "ohne --vorschlaege keine Änderungsverfolgung");

    let again = cli_export(&server.state, book.work, &plain, &DocxExportOptions { proposals: true }).await.unwrap_err();
    assert!(again.contains("nichts überschrieben"), "eine vorhandene Datei bleibt: {again}");
    assert_eq!(std::fs::read(&plain).unwrap(), written, "unverändert, Byte für Byte");

    let tracked = dir.path().join("steg-vorschlaege.docx");
    let text = cli_export(&server.state, book.work, &tracked, &DocxExportOptions { proposals: true }).await.unwrap();
    assert!(text.contains("Vorschläge als Änderungsverfolgung: 1"), "--vorschlaege wirkt und nennt die Zahl: {text}");
    let document = part(&std::fs::read(&tracked).unwrap(), "word/document.xml");
    assert!(
        document.contains("<w:ins ") && document.contains("Fledermäuse") && document.contains("w:author=\"@lektor\""),
        "der Vorschlag steht als Einfügung der Persona in der Datei"
    );

    let missing = dir.path().join("fehlt").join("steg.docx");
    let failed = cli_export(&server.state, book.work, &missing, &DocxExportOptions::default()).await.unwrap_err();
    assert!(failed.contains("fehlt"), "ein Ordner, den es nicht gibt, ist ein Fehler mit dem Pfad: {failed}");
    assert!(!missing.exists(), "und hinterlässt keine Datei");
}

/// Call the program `baruch-server` with the data directory `data`: (exit code, stdout, stderr).
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

#[test]
fn the_program_exports_a_word_file_and_ends_with_exit_1_on_an_existing_file() {
    let data = tempfile::tempdir().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("steg.typ");
    std::fs::write(&source, BOOK).unwrap();
    let (code, _, err) = cli(data.path(), &["import", source.to_str().unwrap()]);
    assert_eq!(code, 0, "Import des Werks über die Kommandozeile: {err}");

    let target = dir.path().join("steg.docx");
    let (code, out, err) =
        cli(data.path(), &["export", "der-steg-am-weiher", "--docx", target.to_str().unwrap(), "--vorschlaege"]);
    assert_eq!(code, 0, "export SLUG --docx DATEI --vorschlaege endet mit Exit 0: {err}");
    assert!(
        out.contains("Word-Datei geschrieben") && out.contains("Vorschläge als Änderungsverfolgung: 0"),
        "Ausgabe mit Zahl der Vorschläge: {out}"
    );
    let written = std::fs::read(&target).unwrap();
    assert!(part(&written, "word/document.xml").contains("Der Steg am Weiher"), "die Datei enthält das Werk");

    let (code, _, err) = cli(data.path(), &["export", "der-steg-am-weiher", "--docx", target.to_str().unwrap()]);
    assert_eq!(code, 1, "eine vorhandene Datei → Exit 1");
    assert!(err.contains("nichts überschrieben"), "mit Klartext auf der Fehlerausgabe: {err}");
    assert_eq!(std::fs::read(&target).unwrap(), written, "die vorhandene Datei ist unverändert");
}
