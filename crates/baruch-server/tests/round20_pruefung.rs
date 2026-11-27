// SPDX-License-Identifier: AGPL-3.0-or-later
//! Round 20c (epub-pruefung, ownership „epub-pruefung“): own check through RPC, page and form, EPUBCheck only as a
//! self-written script through `path_env` (never the PATH of the process, never a download), setting of the
//! epubcheck.jar (RPC and form, no MCP), MCP `epub_check` without EPUBCheck, book-detail leftovers (ISBN prefix and
//! spelling, thickness per sheet, deleting the cover image with a mirror commit, one commit per confirmed cover
//! request) and the text rules of the work template in the cover for the print shop. Works freely invented.

use crate::common;

use std::net::SocketAddr;
use std::path::Path;

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode, header};
use baruch_epub::FootnoteMode;
use baruch_epub::check::{EpubcheckStatus, find_epubcheck};
use baruch_epub::readers::profile;
use serde_json::{Value, json};

const BED: &str =
    "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"8\" height=\"4\"><rect width=\"8\" height=\"4\"/></svg>\n";

fn result(answer: &Value) -> &Value {
    assert!(answer["error"].is_null(), "RPC-Fehler: {answer}");
    &answer["result"]
}

fn error_code(answer: &Value) -> i64 {
    answer["error"]["code"].as_i64().unwrap_or_else(|| panic!("Fehler erwartet: {answer}"))
}

fn make_executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

/// If a real EPUBCheck is in the PATH of this machine, the assertions about „nicht gefunden“ do not hold.
fn epubcheck_on_this_machine() -> bool {
    find_epubcheck(std::env::var_os("PATH").as_deref(), None).is_some()
}

// ---- MCP ---------------------------------------------------------------------------------------------

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

async fn session(server: &common::Server) -> String {
    let (headers, answer) = mcp_post(
        server,
        None,
        json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize",
                "params": { "protocolVersion": "2025-06-18", "capabilities": {},
                            "clientInfo": { "name": "Claude Code", "version": "2.1" } } }),
    )
    .await;
    assert!(answer["result"].is_object(), "initialize: {answer}");
    headers.get("mcp-session-id").unwrap().to_str().unwrap().to_owned()
}

async fn tool(server: &common::Server, session: &str, name: &str, arguments: Value) -> Value {
    let message = json!({ "jsonrpc": "2.0", "id": 3, "method": "tools/call",
                          "params": { "name": name, "arguments": arguments } });
    let answer = mcp_post(server, Some(session), message).await.1;
    assert_eq!(answer["result"]["isError"], false, "{name} sollte gelingen: {answer}");
    answer["result"]["structuredContent"].clone()
}

fn commits(server: &common::Server, slug: &str) -> usize {
    common::git(&server.mirror_dir_of(slug), &["rev-list", "--count", "HEAD"]).map_or(0, |n| n.trim().parse().unwrap())
}

fn subjects(server: &common::Server, slug: &str, n: usize) -> Vec<String> {
    common::git(&server.mirror_dir_of(slug), &["log", &format!("-{n}"), "--format=%s"])
        .unwrap_or_default()
        .lines()
        .map(str::to_owned)
        .collect()
}

fn last_body(server: &common::Server, slug: &str) -> String {
    common::git(&server.mirror_dir_of(slug), &["log", "-1", "--format=%B"]).unwrap_or_default()
}

/// Gartenbuch with the image `beet.svg` as cover.
async fn garden_with_cover() -> (common::Server, i64, String) {
    let server = common::open().await;
    let report = server.import_garden().await;
    let added = server
        .rpc(
            "asset.add",
            json!({ "work": report.work, "file_name": "beet.svg", "content": BED, "encoding": "svg", "alt": "Beet" }),
        )
        .await;
    assert!(added["result"]["commit"].is_string(), "Bild angelegt: {added}");
    result(&server.rpc("work.set_cover", json!({ "work": report.work, "name": "beet.svg" })).await);
    (server, report.work, report.slug)
}

// ---- Check ------------------------------------------------------------------------------------------

#[tokio::test]
async fn epub_check_reports_no_error_for_a_built_book_and_names_tool_and_readers() {
    let server = common::open().await;
    let work = server.import_garden().await.work;
    let answer = server.rpc("epub.check", json!({ "work": work })).await;
    let r = result(&answer);
    assert_eq!(r["summary"]["errors"], 0, "das gebaute EPUB des Gartenbuchs ist gültig: {r}");
    assert!(matches!(r["xml_tool"].as_str(), Some("xmllint" | "quick_xml")), "Werkzeug genannt: {r}");
    assert_eq!(r["epubcheck"]["status"], "not_requested", "ohne epubcheck: true läuft EPUBCheck nicht");
    let required: Vec<&str> = baruch_epub::readers::PROFILES.iter().filter(|p| p.required).map(|p| p.slug).collect();
    assert_eq!(r["readers"], json!(required), "ohne Angabe die Pflicht-Reader");
    let unknown = server.rpc("epub.check", json!({ "work": work, "readers": ["gibt-es-nicht"] })).await;
    assert_eq!(error_code(&unknown), -32602, "unbekannter Reader → ungültige Parameter");
    let kindle = result(&server.rpc("epub.check", json!({ "work": work, "readers": ["kindle"] })).await).clone();
    assert!(
        kindle["findings"].as_array().unwrap().iter().all(|f| f["reader"].is_null() || f["reader"] == "kindle"),
        "Reader-Warnungen nur für den gewählten Reader: {kindle}"
    );
}

#[tokio::test]
async fn without_epubcheck_the_report_says_not_found_and_shows_the_own_check() {
    if epubcheck_on_this_machine() {
        println!("ÜBERSPRUNGEN: epubcheck liegt im PATH dieses Rechners");
        return;
    }
    let server = common::open().await;
    let work = server.import_garden().await.work;
    let r = result(&server.rpc("epub.check", json!({ "work": work, "epubcheck": true })).await).clone();
    assert_eq!(r["epubcheck"]["status"], "not_found", "ohne Programm und Jar: nicht gefunden: {r}");
    assert_eq!(r["summary"]["errors"], 0, "die eigene Prüfung läuft trotzdem: {r}");

    let (status, html) = server.get(&format!("/werke/{work}/epub/pruefung")).await;
    assert_eq!(status, StatusCode::OK, "Prüfseite: {html}");
    assert!(html.contains("EPUBCheck nicht gefunden"), "die Seite sagt es ehrlich: {html}");
    assert!(html.contains("Keine Fehler."), "eigene Prüfung mit Zusammenfassung: {html}");
    assert!(
        html.contains("action=\"/werke/epub/pruefen\"") && html.contains("EPUBCheck ausführen"),
        "Knopf als Formular, ohne JavaScript: {html}"
    );
    assert!(!html.contains("{{") && !html.contains("{%"), "keine Reste der Vorlage");

    let request = Request::post("/werke/epub/pruefen")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(format!("work={work}&leser=kobo&fussnoten=endnoten")))
        .unwrap();
    let (status, _, html) = server.send(request).await;
    assert_eq!(status, StatusCode::OK, "Formular „EPUBCheck ausführen“ antwortet mit dem Bericht: {html}");
    assert!(html.contains("EPUBCheck nicht gefunden"), "nach dem Knopf: nicht gefunden: {html}");
    assert!(html.contains("value=\"kobo\" checked"), "die Wahl des Readers bleibt: {html}");

    let (status, html) = server.get(&format!("/werke/{work}/epub/pruefung?leser=unbekannt")).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "unbekannter Reader: {html}");
    let (status, _) = server.get("/werke/999999/epub/pruefung").await;
    assert_eq!(status, StatusCode::NOT_FOUND, "unbekanntes Werk");
}

#[tokio::test]
async fn the_check_page_is_not_built_on_behalf_of_a_foreign_site() {
    // The check builds and packs the EPUB and starts xmllint per file: a foreign page (for instance through <img>) must
    // not trigger this load, as with /epub and /epub/vorschau.
    let server = common::open().await;
    let work = server.import_garden().await.work;
    for site in ["cross-site", "same-site"] {
        let request = Request::get(format!("/werke/{work}/epub/pruefung"))
            .header("sec-fetch-site", site)
            .body(Body::empty())
            .unwrap();
        let (status, _, body) = server.send(request).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "Sec-Fetch-Site {site} → 403: {body}");
        assert!(body.contains("fremden Seite"), "die Ablehnung sagt warum: {body}");
    }
    let request = Request::get(format!("/werke/{work}/epub/pruefung"))
        .header("sec-fetch-site", "same-origin")
        .body(Body::empty())
        .unwrap();
    let (status, _, body) = server.send(request).await;
    assert_eq!(status, StatusCode::OK, "eigene Seite darf prüfen: {body}");
}

#[tokio::test]
async fn a_fake_epubcheck_in_the_given_path_runs_only_when_asked() {
    let server = common::open().await;
    let work = server.import_garden().await.work;
    let bin = tempfile::tempdir().unwrap();
    let marker = bin.path().join("gelaufen");
    let script = bin.path().join("epubcheck");
    std::fs::write(
        &script,
        format!(
            "#!/bin/sh\ntouch '{}'\necho 'EPUBCheck v5.1.0'\necho \"ERROR(RSC-005): $1/OEBPS/nav.xhtml(1,1): Probe\" >&2\nexit 1\n",
            marker.display()
        ),
    )
    .unwrap();
    make_executable(&script);
    let readers = [profile("tolino").unwrap()];
    let path = Some(bin.path().as_os_str());

    let quiet = baruch_server::epub_check::run_check(&server.state, work, &readers, FootnoteMode::Popup, false, path)
        .await
        .unwrap();
    assert_eq!(quiet.report.epubcheck, EpubcheckStatus::NotRequested, "ohne Wunsch kein Lauf");
    assert!(!marker.exists(), "das Skript lief nicht");

    let ran = baruch_server::epub_check::run_check(&server.state, work, &readers, FootnoteMode::Popup, true, path)
        .await
        .unwrap();
    assert!(marker.exists(), "auf Wunsch läuft das Skript aus dem übergebenen PATH");
    match &ran.report.epubcheck {
        EpubcheckStatus::Ran { version, ok, .. } => {
            assert_eq!(version.as_deref(), Some("5.1.0"), "Version aus der Ausgabe");
            assert!(!ok, "Exit 1 mit Fehler: nicht bestanden");
        }
        other => panic!("EPUBCheck lief: {other:?}"),
    }
    let finding = ran.report.findings.iter().find(|f| f.code == "EPUBCHECK_RSC-005").expect("Meldung übernommen");
    assert_eq!(finding.path.as_deref(), Some("OEBPS/nav.xhtml(1,1)"), "Pfad im EPUB, nicht im Temp-Ordner");
    assert_eq!(ran.summary.errors, 1, "die Meldung zählt als Fehler");
}

// ---- Setting ----------------------------------------------------------------------------------------

#[tokio::test]
async fn checker_set_accepts_only_absolute_existing_jars_and_null_removes() {
    let server = common::open().await;
    let dir = tempfile::tempdir().unwrap();
    let jar = dir.path().join("epubcheck.jar");
    std::fs::write(&jar, b"PK").unwrap();
    let zip = dir.path().join("epubcheck.zip");
    std::fs::write(&zip, b"PK").unwrap();
    for (bad, why) in [
        (json!("epubcheck.jar"), "relativer Pfad"),
        (json!(zip.to_str().unwrap()), "keine .jar-Datei"),
        (json!(dir.path().join("fehlt.jar").to_str().unwrap()), "Datei fehlt"),
        (json!(format!("{}\u{1b}x.jar", dir.path().display())), "Steuerzeichen"),
        (json!(format!("/{}.jar", "a".repeat(5000))), "länger als 4096 Bytes"),
        (json!(42), "keine Zeichenkette"),
    ] {
        let answer = server.rpc("epub.checker_set", json!({ "jar": bad })).await;
        assert_eq!(error_code(&answer), -32602, "{why} wird abgelehnt: {answer}");
    }
    assert_eq!(server.state.store.epubcheck_jar().await.unwrap(), None, "nichts gespeichert");

    let set = result(&server.rpc("epub.checker_set", json!({ "jar": jar.to_str().unwrap() })).await).clone();
    assert_eq!(set["jar"], jar.to_str().unwrap(), "gültiger Pfad gespeichert: {set}");
    assert_eq!(
        server.state.store.epubcheck_jar().await.unwrap().as_deref(),
        jar.to_str(),
        "in store_meta (epubcheck_jar)"
    );
    if !epubcheck_on_this_machine() {
        assert_eq!(set["found"]["kind"], "jar", "ohne Programm im PATH gilt das Jar: {set}");
    }
    assert!(set["java"].is_boolean(), "Antwort sagt, ob java im PATH liegt: {set}");
    let got = result(&server.rpc("epub.checker_get", json!({})).await).clone();
    assert_eq!(got["jar"], set["jar"], "checker_get liefert denselben Stand");
    let removed = result(&server.rpc("epub.checker_set", json!({ "jar": null })).await).clone();
    assert!(removed["jar"].is_null(), "null entfernt: {removed}");
    assert_eq!(server.state.store.epubcheck_jar().await.unwrap(), None, "auch im Store");
}

#[tokio::test]
async fn the_epubcheck_form_in_settings_redirects_or_explains_and_there_is_no_mcp_way() {
    let server = common::open().await;
    let dir = tempfile::tempdir().unwrap();
    let jar = dir.path().join("epubcheck.jar");
    std::fs::write(&jar, b"PK").unwrap();
    let encoded: String = jar.to_str().unwrap().replace('/', "%2F");
    let (status, location) = server.post_form("/einstellungen/daten/epubcheck", &format!("jar={encoded}")).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "gültiger Pfad → Weiterleitung");
    assert_eq!(location.as_deref(), Some("/einstellungen/daten?ok=epubcheck#epubcheck"), "zurück zum Block");
    assert_eq!(server.state.store.epubcheck_jar().await.unwrap().as_deref(), jar.to_str(), "gespeichert");
    let (status, html) = server.get("/einstellungen/daten?ok=epubcheck").await;
    assert_eq!(status, StatusCode::OK, "Einstellungen → Daten: {html}");
    let shown = jar.to_str().unwrap().replace('/', "&#x2f;"); // minijinja escapes „/“ too
    assert!(html.contains("id=\"epubcheck\"") && html.contains(&shown), "Block mit dem Pfad: {html}");
    assert!(html.contains("Einstellung für EPUBCheck gespeichert"), "Hinweis nach dem Speichern");

    let request = Request::post("/einstellungen/daten/epubcheck")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from("jar=relativ.jar"))
        .unwrap();
    let (status, _, html) = server.send(request).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "ungültiger Pfad → 422: {html}");
    assert!(html.contains("kein absoluter Pfad"), "mit Meldung: {html}");
    assert_eq!(server.state.store.epubcheck_jar().await.unwrap().as_deref(), jar.to_str(), "nichts geändert");
    let (status, _) = server.post_form("/einstellungen/daten/epubcheck", "jar=").await;
    assert_eq!(status, StatusCode::SEE_OTHER, "leer = entfernen");
    assert_eq!(server.state.store.epubcheck_jar().await.unwrap(), None, "entfernt");

    let s = session(&server).await;
    let (_, list) = mcp_post(&server, Some(&s), json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" })).await;
    let names: Vec<&str> =
        list["result"]["tools"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap()).collect();
    assert!(names.contains(&"epub_check"), "epub_check ist ein Werkzeug: {names:?}");
    assert!(
        !names.iter().any(|n| n.contains("checker") || n.contains("epubcheck") || n.contains("jar")),
        "kein MCP-Weg zur Einstellung von EPUBCheck: {names:?}"
    );
}

#[tokio::test]
async fn mcp_epub_check_never_starts_epubcheck() {
    let server = common::open().await;
    let work = server.import_garden().await.work;
    // A configured jar would be "found" for the human – MCP starts nothing nevertheless.
    let dir = tempfile::tempdir().unwrap();
    let jar = dir.path().join("epubcheck.jar");
    std::fs::write(&jar, b"PK").unwrap();
    server.state.store.set_epubcheck_jar(jar.to_str()).await.unwrap();
    let s = session(&server).await;
    let answer = tool(&server, &s, "epub_check", json!({ "work": work, "readers": ["kobo"], "epubcheck": true })).await;
    assert_eq!(answer["epubcheck"]["status"], "not_requested", "MCP startet nie EPUBCheck: {answer}");
    assert_eq!(answer["summary"]["errors"], 0, "eigene Prüfung ohne Fehler: {answer}");
    assert_eq!(answer["readers"], json!(["kobo"]), "gewählter Reader");
    assert_eq!(answer["url"], format!("/werke/{work}/epub/pruefung"), "Adresse der Prüfseite für den Menschen");
}

#[tokio::test]
async fn the_preview_panel_is_a_finished_fragment_with_summary_and_reader_warnings() {
    let server = common::open().await;
    let work = server.import_garden().await.work;
    let kindle = profile("kindle").unwrap();
    let html = baruch_server::epub_check::preview_panel(&server.state, work, &[kindle]).await;
    assert!(html.contains("EPUB-Prüfung") && html.contains("data-epub-check-summary"), "Zusammenfassung: {html}");
    assert!(html.contains(kindle.name), "der Reader steht im Bericht: {html}");
    assert!(html.contains("Send to Kindle"), "Kindle geht über Send to Kindle (Hinweis je Reader): {html}");
    assert!(html.contains(&format!("/werke/{work}/epub/pruefung")), "Link zur ganzen Prüfung: {html}");
    assert!(!html.contains("<html") && !html.contains("{{"), "nur ein Fragment, keine Vorlagenreste");
    let none = baruch_server::epub_check::preview_panel(&server.state, 999_999, &[kindle]).await;
    assert!(none.contains("role=\"alert\""), "Fehler erscheinen als Meldung im Fragment: {none}");
}

// ---- Book-detail leftovers --------------------------------------------------------------------------

#[tokio::test]
async fn isbn_prefix_lowercase_label_and_a_differently_written_isbn() {
    let server = common::open().await;
    let report = server.import_garden().await;
    let work = report.work;
    let wrong = server.rpc("work.meta.set", json!({ "work": work, "isbn": "4006381333931" })).await;
    assert_eq!(error_code(&wrong), -32602, "Präfix 400: abgelehnt");
    assert!(wrong["error"]["message"].as_str().unwrap().contains("978 oder 979"), "eigene Meldung: {wrong}");
    let lower =
        result(&server.rpc("work.meta.set", json!({ "work": work, "isbn": "isbn-13 9783161484100" })).await).clone();
    assert_eq!(lower["changed"], json!(["isbn"]), "„isbn-13 …“ in Kleinschrift gilt: {lower}");
    let plain = result(&server.rpc("work.meta.set", json!({ "work": work, "isbn": "9783161484100" })).await).clone();
    assert_eq!(plain["changed"], json!(["isbn"]), "ohne Vorsatz ist eine andere Schreibweise: {plain}");
    let before = commits(&server, &report.slug);
    let dashed =
        result(&server.rpc("work.meta.set", json!({ "work": work, "isbn": "978-3-16-148410-0" })).await).clone();
    assert_eq!(dashed["changed"], json!(["isbn"]), "nur Bindestriche anders: wird gespeichert: {dashed}");
    assert_eq!(commits(&server, &report.slug), before + 1, "mit Commit „Buchangaben geändert: ISBN“");
    let meta = result(&server.rpc("work.meta.get", json!({ "work": work })).await).clone();
    assert_eq!(meta["meta"]["isbn"], "978-3-16-148410-0", "so gespeichert, wie eingegeben");
    let same = result(&server.rpc("work.meta.set", json!({ "work": work, "isbn": "978-3-16-148410-0" })).await).clone();
    assert_eq!(same["changed"], json!([]), "gleich geschrieben: keine Änderung");
}

#[tokio::test]
async fn paper_thickness_per_sheet_without_migration_and_with_unchanged_spines() {
    let server = common::open().await;
    let work = server.import_garden().await.work;
    let meta = result(&server.rpc("work.meta.get", json!({ "work": work })).await).clone();
    assert_eq!(meta["meta"]["paper_mm"], 0.1, "gespeichert bleibt die Dicke je Seite (Vorgabe unverändert)");
    assert_eq!(meta["meta"]["paper_sheet_mm"], 0.2, "dazu je Blatt = 2 × je Seite");
    {
        let _serial = common::PDF_SERIAL.lock().await;
        let wrap = baruch_server::cover::build_wrap(&server.state, work).await.unwrap();
        let expected = (wrap.pages - wrap.cover_pages) as f64 * 0.1;
        assert!(
            (wrap.spine_mm - expected).abs() < 1e-9,
            "Rücken eines bestehenden Werks unverändert: {}",
            wrap.spine_mm
        );
    }
    let both = server.rpc("work.meta.set", json!({ "work": work, "paper_mm": 0.1, "paper_sheet_mm": 0.2 })).await;
    assert_eq!(error_code(&both), -32602, "paper_mm und paper_sheet_mm zusammen: abgelehnt");
    let thick = server.rpc("work.meta.set", json!({ "work": work, "paper_sheet_mm": 2.5 })).await;
    assert_eq!(error_code(&thick), -32602, "mehr als 2 mm je Blatt: abgelehnt");
    let set = result(&server.rpc("work.meta.set", json!({ "work": work, "paper_sheet_mm": "0,15" })).await).clone();
    assert_eq!(set["changed"], json!(["paper_mm"]), "gespeichert wird die Dicke je Seite: {set}");
    let meta = result(&server.rpc("work.meta.get", json!({ "work": work })).await).clone();
    assert_eq!(meta["meta"]["paper_mm"], 0.075, "0,15 mm je Blatt = 0,075 mm je Seite");
    assert_eq!(meta["meta"]["paper_sheet_mm"], 0.15, "und zurück je Blatt");

    let (status, location) = server
        .post_form("/werke/buchangaben/act", &format!("work={work}&flags=1&paper_sheet_mm=0%2C1&title_area=top"))
        .await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Formular mit der Dicke je Blatt: {location:?}");
    let meta = result(&server.rpc("work.meta.get", json!({ "work": work })).await).clone();
    assert_eq!(meta["meta"]["paper_mm"], 0.05, "0,1 mm je Blatt aus dem Formular");
    let (_, html) = server.get(&format!("/einstellungen/werk?work={work}")).await;
    assert!(
        html.contains("Papierdicke je Blatt in mm (1 Blatt = 2 Seiten)") && html.contains("name=\"paper_sheet_mm\""),
        "Beschriftung spricht von der Dicke je Blatt: {html}"
    );
    assert!(html.contains("value=\"0,1\""), "angezeigt je Blatt mit Komma");
    assert!(html.contains("Vorgabe 0,2 mm je Blatt"), "die Vorgabe ist erklärt");
}

#[tokio::test]
async fn deleting_the_cover_picture_first_commits_cover_removed() {
    let (server, work, slug) = garden_with_cover().await;
    let deleted =
        result(&server.rpc("asset.delete", json!({ "work": work, "name": "beet.svg", "confirm": true })).await).clone();
    assert_eq!(deleted["cover_removed"], true, "die Antwort sagt, dass der Umschlag entfernt wurde: {deleted}");
    let log = subjects(&server, &slug, 2);
    assert_eq!(log[1], "Umschlag entfernt", "zuerst der Umschlag (eigener Commit): {log:?}");
    assert!(log[0].contains("beet.svg") && log[0].contains("gelöscht"), "danach das Bild: {log:?}");
    assert_eq!(server.state.store.work_cover(work).await.unwrap(), None, "kein Umschlag mehr");
}

#[tokio::test]
async fn deleting_the_cover_picture_by_form_also_commits_cover_removed() {
    let (server, work, slug) = garden_with_cover().await;
    let (status, _) =
        server.post_form("/werke/bilder/act", &format!("action=remove&work={work}&name=beet.svg&confirm=1")).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Formular löscht");
    let log = subjects(&server, &slug, 2);
    assert_eq!(log[1], "Umschlag entfernt", "Formular wie RPC: erst der Umschlag: {log:?}");
}

#[tokio::test]
async fn a_confirmed_delete_request_of_the_cover_commits_cover_removed_first() {
    let (server, work, slug) = garden_with_cover().await;
    let s = session(&server).await;
    let asked = tool(&server, &s, "asset_delete", json!({ "work": work, "name": "beet.svg" })).await;
    let id = asked["request"]["id"].as_i64().unwrap();
    result(&server.rpc("asset.request_decide", json!({ "work": work, "id": id, "decision": "confirm" })).await);
    let log = subjects(&server, &slug, 2);
    assert_eq!(log[1], "Umschlag entfernt", "bestätigter MCP-Antrag: erst der Umschlag: {log:?}");
    assert!(log[0].contains("gelöscht"), "dann das Bild: {log:?}");
}

#[tokio::test]
async fn a_confirmed_cover_request_writes_exactly_one_commit() {
    let server = common::open().await;
    let report = server.import_garden().await;
    let (work, slug) = (report.work, report.slug);
    let added = server
        .rpc("asset.add", json!({ "work": work, "file_name": "beet.svg", "content": BED, "encoding": "svg" }))
        .await;
    assert!(added["result"]["commit"].is_string(), "Bild angelegt: {added}");
    let s = session(&server).await;
    let asked = tool(&server, &s, "asset_set_cover", json!({ "work": work, "name": "beet.svg" })).await;
    let id = asked["request"]["id"].as_i64().unwrap();
    let before = commits(&server, &slug);
    let decided =
        result(&server.rpc("asset.request_decide", json!({ "work": work, "id": id, "decision": "confirm" })).await)
            .clone();
    assert!(decided["commit"].is_string(), "mit Commit: {decided}");
    assert_eq!(commits(&server, &slug), before + 1, "genau ein Commit je bestätigtem Umschlag-Antrag");
    let body = last_body(&server, &slug);
    assert!(body.starts_with(&format!("Bildantrag {id} bestätigt")), "Betreff nennt den Antrag: {body}");
    assert!(body.contains("Umschlag nachher: beet.svg"), "Rumpf mit dem Umschlag vorher/nachher: {body}");
    assert!(body.contains("Baruch-Requested-By: @mcp:claude-code"), "Herkunft im Trailer: {body}");
    assert_eq!(server.state.store.work_cover(work).await.unwrap().as_deref(), Some("beet.svg"), "Umschlag gesetzt");
}

// ---- Cover for the print shop -----------------------------------------------------------------------

#[tokio::test]
async fn the_printer_cover_takes_the_text_rules_of_the_template() {
    let server = common::open().await;
    let book = "#set document(title: \"Teichbuch\")\n#set page(width: 120mm, height: 190mm)\n\
                #set text(lang: \"en\", hyphenate: true, font: \"Libertinus Serif\")\n#set par(justify: true)\n\n\
                = Frogs\n\nThe pond is quiet in the morning.\n";
    let work = server.import(book, "teichbuch.typ").await.work;
    let garden = server.import_garden().await.work;
    let _serial = common::PDF_SERIAL.lock().await;
    let source = baruch_server::cover::cover_source(&server.state, work).await.unwrap();
    assert!(
        source
            .main
            .contains("#set text(lang: \"de\")\n#set text(lang: \"en\", hyphenate: true)\n#set par(justify: true)\n"),
        "Sprache, Silbentrennung und Blocksatz der Vorlage gelten auch auf dem Umschlag (ohne Schrift):\n{}",
        source.main
    );
    let plain = baruch_server::cover::cover_source(&server.state, garden).await.unwrap();
    let after_lang = plain.main.split_once("#set text(lang: \"de\")\n").map(|(_, rest)| rest).unwrap_or("");
    assert!(
        !after_lang.starts_with("#set "),
        "eine Vorlage mit nur „de“ fügt keine Regel ein (byte-gleich, Goldstände R26/R27 in round27_satz.rs):\n{}",
        plain.main
    );
}
