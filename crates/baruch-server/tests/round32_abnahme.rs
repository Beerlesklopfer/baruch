// SPDX-License-Identifier: AGPL-3.0-or-later
//! Acceptance of Round 32 (integrator): round trip Word through the server, menu entries, no MCP tool, docs.

/// Interface R32-S1: the three entries are at the places of the ribbon (contract Round 32 §0.5) – verbatim, as
/// ordinary links (usable without JavaScript). The markers themselves are checked by Round 33.
/// Round 35 (contract §3.2): import and quit carry the class `nur-autor` (hidden for Lektors; without accounts
/// `konten_ui::apply` takes it out of the delivered page). After „Baruch beenden…“ and its marker follows the marker
/// for the entry for the account.
#[test]
fn the_three_entries_stand_at_their_places_in_the_ribbon() {
    let base = include_str!("../templates/base.html");
    let import = "<a class=\"menu-item nur-autor\" role=\"menuitem\" href=\"/werke/import/word\" title=\"Word-Datei lesen, \
                  Gliederung, Bearbeiter und Warnungen prüfen und als Werk übernehmen\">Word (.docx) importieren…</a>";
    let quit = "<a class=\"menu-item nur-autor\" role=\"menuitem\" href=\"/beenden\" title=\"Baruch auf diesem Rechner beenden \
                (mit Rückfrage)\">Baruch beenden…</a>";
    let export = "<a class=\"btn\" href=\"/werke/{{ w }}/word\" title=\"Das Werk als Word-Datei für den Verlag \
                  (Formatvorlagen, Fußnoten, Bilder; auf Wunsch offene Vorschläge als Änderungsverfolgung)\">Word …</a>";
    for entry in [import, quit, export] {
        assert_eq!(base.matches(entry).count(), 1, "der Eintrag steht genau einmal und wörtlich in base.html: {entry}");
    }
    let after = |mark: &str| base.split(mark).nth(1).unwrap_or_else(|| panic!("Marke fehlt: {mark}")).trim_start();
    assert!(
        after("{#- R32-PLATZ import-word -#}").starts_with(import),
        "„Word (.docx) importieren…“ folgt direkt auf seine Marke (bei den Importen im Menü Baruch ▾)"
    );
    assert!(
        after("{#- R32-PLATZ export-word -#}").starts_with(export),
        "„Word …“ folgt direkt auf seine Marke (Gruppe Ausgabe, hinter EPUB)"
    );
    let before_quit = base.split("{#- R32-PLATZ beenden -#}").next().unwrap().trim_end();
    assert!(
        before_quit.ends_with(quit),
        "„Baruch beenden…“ steht direkt vor seiner Marke – der letzte feste Eintrag des Menüs"
    );
    let behind = after("{#- R32-PLATZ beenden -#}");
    assert!(
        behind.starts_with("{#- Runde 35:") && !behind[..behind.find("</div>").unwrap()].contains("<a "),
        "dahinter folgt im Menü kein fester Eintrag mehr, nur die Marke für den Eintrag zum Konto (Runde 35)"
    );
}

// ---- Round trip Word through the server -----------------------------------------------------------------------

use std::io::{Cursor, Read};

use baruch_core::{Author, RevisionStatus};
use serde_json::{Value, json};

use crate::common;

/// The same freely invented Word file „Das Haus am Deich“ with which the reader is tested: headings, footnote, image,
/// lists, table, change tracking of two editors and comments including a reply.
use crate::round32_docx_import::docx;

const ERIKA: &str = "@word:erika-beispiel";
const JOERG: &str = "@word:joerg-mueller";

async fn ok(server: &common::Server, method: &str, params: Value) -> Value {
    let answer = server.rpc(method, params).await;
    assert!(answer["error"].is_null(), "{method} sollte gelingen: {answer}");
    answer["result"].clone()
}

/// Reads a Word file through the preview and accepts it under `title`; returns the preview and the result.
async fn import(server: &common::Server, file: &[u8], title: &str) -> (Value, Value) {
    let shown = ok(
        server,
        "docx_import.preview",
        json!({ "file_name": "deich.docx", "data_base64": baruch_store::encode_base64(file), "title": title }),
    )
    .await;
    let applied = ok(server, "docx_import.apply", json!({ "draft": shown["draft"] })).await;
    (shown, applied)
}

async fn export(server: &common::Server, work: i64, proposals: bool) -> (Value, Vec<u8>) {
    let answer = ok(server, "work.docx_export", json!({ "work": work, "proposals": proposals })).await;
    let bytes = baruch_store::decode_base64(answer["data_base64"].as_str().expect("Datei als Base64"))
        .expect("gültiges Base64");
    (answer, bytes)
}

fn names(file: &[u8]) -> Vec<String> {
    let mut zip = zip::ZipArchive::new(Cursor::new(file)).expect("die Word-Datei ist ein ZIP-Archiv");
    (0..zip.len())
        .map(|i| zip.by_index(i).expect("Eintrag lesbar").name().expect("Name des Eintrags lesbar").into_owned())
        .collect()
}

fn part(file: &[u8], name: &str) -> Vec<u8> {
    let mut zip = zip::ZipArchive::new(Cursor::new(file)).expect("die Word-Datei ist ein ZIP-Archiv");
    let mut entry = zip.by_name(name).unwrap_or_else(|_| panic!("Teil {name} fehlt in der Word-Datei"));
    let mut bytes = Vec::new();
    entry.read_to_end(&mut bytes).expect("Teil lesbar");
    bytes
}

fn text(file: &[u8], name: &str) -> String {
    String::from_utf8(part(file, name)).unwrap_or_else(|_| panic!("Teil {name} ist UTF-8"))
}

/// Outline (kind, number, title) and current text per section.
async fn current(server: &common::Server, work: i64) -> Vec<(String, String)> {
    let loaded = server.state.store.load_work(work).await.unwrap();
    loaded
        .nodes
        .iter()
        .map(|n| {
            let revision = &n.revisions[n.current_index().expect("jeder Abschnitt hat eine aktuelle Fassung")];
            (format!("{:?}/{}/{}", n.kind, n.number, n.title), revision.body.clone())
        })
        .collect()
}

fn codes(value: &Value) -> Vec<String> {
    value["warnings"].as_array().unwrap().iter().map(|w| w["code"].as_str().unwrap().to_owned()).collect()
}

/// End to end: Word file → work → Word file → work. Checked is what the contract of the round promises.
#[tokio::test]
async fn a_word_file_survives_import_export_and_import_again() {
    let server = common::open().await;
    let (shown, applied) = import(&server, &docx::sample(), "Das Haus am Deich").await;
    assert_eq!(
        shown["sections"].as_array().unwrap().iter().map(|s| s["title"].as_str().unwrap()).collect::<Vec<_>>(),
        vec!["Ankunft", "Der Deich", "Die Schleuse"],
        "die Vorschau zeigt die Gliederung der Datei, bevor irgendetwas angelegt ist"
    );
    let first = applied["work"].as_i64().expect("Nummer des Werks");
    let original = current(&server, first).await;
    assert_eq!(original.len(), 3, "das Werk hat die drei Abschnitte der Vorschau");
    assert!(
        original[0].1.contains("leise") && !original[0].1.contains("laut") && !original[0].1.contains("Abends"),
        "die aktuelle Fassung ist der Text ohne die Änderungen der Bearbeiter: {}",
        original[0].1
    );
    let work = server.state.store.load_work(first).await.unwrap();
    let proposals: Vec<(&str, RevisionStatus)> = work.nodes[0]
        .revisions
        .iter()
        .filter_map(|r| match &r.author {
            Author::Assistant { persona, .. } => Some((persona.as_str(), r.status)),
            _ => None,
        })
        .collect();
    assert_eq!(
        proposals,
        vec![(ERIKA, RevisionStatus::Proposed), (JOERG, RevisionStatus::Proposed)],
        "die Änderungen stehen als offene Vorschläge unter dem Namen des jeweiligen Bearbeiters"
    );
    assert_eq!(
        work.nodes.iter().map(|n| n.posts.len()).sum::<usize>(),
        4,
        "die drei Kommentare und die geänderte Überschrift sind Beiträge"
    );
    assert!(
        work.nodes[0].posts[0].body.ends_with("Woher kam der Wind genau?") && work.nodes[0].posts[0].author == ERIKA,
        "der Kommentar steht als Beitrag unter dem Namen der Kommentierenden am Abschnitt der Stelle"
    );

    // Export: valid OOXML with the specified parts.
    let (answer, file) = export(&server, first, false).await;
    assert_eq!(answer["proposals"], 0, "ohne Wunsch stehen keine Vorschläge in der Datei");
    let listed = names(&file);
    for name in [
        "[Content_Types].xml",
        "_rels/.rels",
        "word/document.xml",
        "word/_rels/document.xml.rels",
        "word/styles.xml",
        "word/settings.xml",
        "word/numbering.xml",
        "word/footnotes.xml",
    ] {
        assert!(listed.iter().any(|n| n == name), "Teil {name} gehört zu einer gültigen Word-Datei: {listed:?}");
    }
    let media: Vec<&String> = listed.iter().filter(|n| n.starts_with("word/media/")).collect();
    assert_eq!(media.len(), 1, "das eine Bild des Werks ist eingebettet: {listed:?}");
    assert_eq!(part(&file, media[0]), docx::PNG, "und zwar mit genau den Bytes des Bildes im Werk");
    let document = text(&file, "word/document.xml");
    assert!(
        document.contains("<w:footnoteReference ") && text(&file, "word/footnotes.xml").contains("Süderhörn"),
        "die Fußnote ist eine echte Fußnote (Verweis im Text, Wortlaut in footnotes.xml)"
    );
    assert!(document.contains("<w:tbl>") && document.contains("<w:numPr>"), "Tabelle und Listen sind Word-Elemente");
    assert!(
        !document.contains("<w:ins ") && !document.contains("<w:del "),
        "ohne Wunsch enthält die Datei keine Änderungsverfolgung"
    );
    for name in &listed {
        if name.ends_with(".xml") || name.ends_with(".rels") {
            let xml = text(&file, name);
            let mut reader = quick_xml::Reader::from_str(&xml);
            loop {
                match reader.read_event() {
                    Ok(quick_xml::events::Event::Eof) => break,
                    Ok(_) => {}
                    Err(e) => panic!("{name} ist kein wohlgeformtes XML: {e}"),
                }
            }
        }
    }

    // Re-import of the own file: same outline, same text.
    let (again, applied) = import(&server, &file, "Das Haus am Deich (Rundreise)").await;
    assert_eq!(again["editors"], json!([]), "die exportierte Datei ohne Vorschläge kennt keine Bearbeiter");
    assert_eq!(applied["proposals"], 0, "und ergibt darum keinen Vorschlag");
    let second = applied["work"].as_i64().expect("Nummer des zweiten Werks");
    let returned = current(&server, second).await;
    assert_eq!(
        returned.iter().map(|(outline, _)| outline).collect::<Vec<_>>(),
        original.iter().map(|(outline, _)| outline).collect::<Vec<_>>(),
        "nach der Rundreise steht dieselbe Gliederung da (Art, Nummer, Titel)"
    );
    for ((outline, before), (_, after)) in original.iter().zip(&returned) {
        assert_eq!(after, before, "Abschnitt {outline}: nach der Rundreise steht derselbe Text da");
    }
    assert!(
        server.state.store.asset_by_path(second, "bilder/leuchtturm.png").await.unwrap().is_some(),
        "auch das Bild kommt unter seinem Pfad zurück"
    );
    assert_eq!(
        codes(&applied),
        Vec::<String>::new(),
        "die Rundreise der eigenen Datei braucht keinen Hinweis: {applied}"
    );
}

/// Export with open proposals: change tracking with the persona as the editor – and on reading it in again it becomes
/// exactly this proposal against the current revision again.
#[tokio::test]
async fn open_proposals_travel_as_tracked_changes_and_come_back_as_proposals() {
    let server = common::open().await;
    let (_, applied) = import(&server, &docx::sample(), "Das Haus am Deich").await;
    let first = applied["work"].as_i64().expect("Nummer des Werks");
    let work = server.state.store.load_work(first).await.unwrap();
    let latest = work.nodes[0].revisions.last().expect("Vorschläge am ersten Abschnitt");
    assert!(
        matches!(&latest.author, Author::Assistant { persona, .. } if persona == JOERG),
        "Vorbedingung: der jüngste offene Vorschlag am ersten Abschnitt stammt von Jörg"
    );

    let (answer, file) = export(&server, first, true).await;
    assert_eq!(answer["proposals"], 1, "ein Abschnitt trägt einen Vorschlag als Änderungsverfolgung: {answer}");
    assert_eq!(
        codes(&answer).iter().filter(|c| *c == "docx_weitere_vorschlaege").count(),
        1,
        "Erikas älterer Vorschlag steht nicht in der Datei – das wird gesagt, nicht verschwiegen: {answer}"
    );
    let document = text(&file, "word/document.xml");
    let author = format!("w:author=\"{JOERG}\"");
    let inserted: Vec<&str> = document.split("<w:ins ").skip(1).map(|s| &s[..s.find('>').unwrap()]).collect();
    assert!(!inserted.is_empty(), "die Einfügung des Vorschlags steht als w:ins in der Datei");
    assert!(
        inserted.iter().all(|tag| tag.contains(&author) && tag.contains("w:date=\"")),
        "jede Einfügung nennt die Persona als Bearbeiter und ein Datum: {inserted:?}"
    );
    assert!(document.contains("Abends"), "der eingefügte Wortlaut steht in der Datei");
    assert!(!document.contains(ERIKA), "Erikas Vorschlag ist nicht eingemischt");

    let (shown, applied) = import(&server, &file, "Das Haus am Deich (mit Vorschlag)").await;
    assert_eq!(
        shown["editors"].as_array().unwrap().iter().map(|e| e["name"].as_str().unwrap()).collect::<Vec<_>>(),
        vec![JOERG],
        "beim Wiedereinlesen ist die Persona der eine Bearbeiter"
    );
    let second = applied["work"].as_i64().expect("Nummer des zweiten Werks");
    assert_eq!(
        current(&server, second).await,
        current(&server, first).await,
        "die aktuelle Fassung ist wieder der Text ohne die Änderungen"
    );
    let back = server.state.store.load_work(second).await.unwrap();
    let proposal = back.nodes[0].revisions.last().unwrap();
    assert_eq!(proposal.status, RevisionStatus::Proposed, "die Änderungsverfolgung wird wieder ein offener Vorschlag");
    assert_eq!(proposal.body, latest.body, "mit genau dem Text des ausgegebenen Vorschlags");
    assert!(
        matches!(&proposal.author, Author::Assistant { persona, .. } if persona == "@word:word-joerg-mueller"),
        "unter einem Namen, der die Herkunft aus Word zeigt: {:?}",
        proposal.author
    );
}

// ---- Throwaway server as a real process: Word in, Word out, quit -----------------------------------------------

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

const PROXY_NAME: &str = "buch.example.test";

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap()
}

/// Running `baruch-server serve`; on drop the own child process is terminated (in case the test fails).
struct Process {
    child: Child,
    url: String,
    lines: std::sync::mpsc::Receiver<String>,
    data: tempfile::TempDir,
}

impl Drop for Process {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Process {
    /// Prepare the data directory (connections to a dead address – no request to real services), start the server with
    /// one allowed proxy name, wait for „Baruch läuft“.
    async fn start() -> Process {
        let data = tempfile::tempdir().unwrap();
        {
            let state = Arc::new(baruch_server::AppState::open(data.path()).await.expect("Datenordner"));
            let app = common::router(state.clone());
            for kind in ["eliezer", "multigpt"] {
                let set = json!({ "kind": kind, "config": { "url": "http://127.0.0.1:9" } });
                let answer = common::rpc(app.clone(), "connection.set", set).await;
                assert!(answer["error"].is_null(), "Verbindung {kind} auf eine tote Adresse gestellt: {answer}");
            }
            state.store.pool().close().await;
        }
        let mut child = Command::new(env!("CARGO_BIN_EXE_baruch-server"))
            .args(["serve", "--listen", "127.0.0.1:0", "--secrets", "memory", "--allow-host", PROXY_NAME, "--data"])
            .arg(data.path())
            .env_remove("BARUCH_ALLOWED_HOSTS")
            .env_remove("BARUCH_LISTEN")
            .env_remove("BARUCH_DATA")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("baruch-server startet");
        let stdout = BufReader::new(child.stdout.take().unwrap());
        let (send, lines) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            for line in stdout.lines().map_while(Result::ok) {
                if send.send(line).is_err() {
                    break;
                }
            }
        });
        let mut process = Process { child, url: String::new(), lines, data };
        let limit = Instant::now() + Duration::from_secs(120);
        while process.url.is_empty() {
            match process.lines.recv_timeout(limit.saturating_duration_since(Instant::now())) {
                Ok(line) => {
                    if let Some(url) = line.strip_prefix("Baruch läuft: ") {
                        process.url = url.trim_end_matches('/').to_owned();
                    }
                }
                Err(e) => panic!("der Server meldet sich nicht mit „Baruch läuft“: {e}"),
            }
        }
        process
    }

    fn at(&self, path: &str) -> String {
        format!("{}{path}", self.url)
    }

    /// Waits for the end of the process; returns the exit code and the further lines of the standard output.
    fn finish(&mut self, limit: Duration) -> (Option<i32>, Vec<String>) {
        let end = Instant::now() + limit;
        let status = loop {
            if let Some(status) = self.child.try_wait().unwrap() {
                break status;
            }
            assert!(Instant::now() < end, "der Prozess endet nicht innerhalb von {limit:?}");
            std::thread::sleep(Duration::from_millis(50));
        };
        let mut lines = Vec::new();
        while let Ok(line) = self.lines.recv_timeout(Duration::from_secs(2)) {
            lines.push(line);
        }
        (status.code(), lines)
    }
}

fn location(response: &reqwest::Response) -> String {
    response.headers().get("location").map(|v| v.to_str().unwrap().to_owned()).unwrap_or_default()
}

/// The whole way on a real process, only with what a browser without JavaScript can do (pages, forms, redirects):
/// upload a Word file, preview, accept, download as Word, quit Baruch.
#[tokio::test]
async fn a_throwaway_server_takes_word_gives_word_and_quits_cleanly() {
    let mut process = Process::start().await;
    let client = reqwest::Client::builder().no_proxy().redirect(reqwest::redirect::Policy::none()).build().unwrap();

    // Menu: the three entries of the round are on the start page as ordinary links.
    let home = client.get(process.at("/")).send().await.unwrap().text().await.unwrap();
    for link in ["href=\"/werke/import/word\"", "href=\"/beenden\"", "/word\" title=\"Das Werk als Word-Datei"] {
        assert!(home.contains(link), "die Startseite trägt den Eintrag {link} im Menüband (ohne JavaScript ein Link)");
    }

    // Word import: form with file → 303 to the preview.
    let boundary = "----baruch-r32-abnahme";
    let mut body = format!("--{boundary}\r\nContent-Disposition: form-data; name=\"title\"\r\n\r\n\r\n").into_bytes();
    body.extend_from_slice(
        format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"deich.docx\"\r\n\
             Content-Type: application/octet-stream\r\n\r\n"
        )
        .as_bytes(),
    );
    body.extend_from_slice(&docx::sample());
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    let answer = client
        .post(process.at("/werke/import/word/vorschau"))
        .header("content-type", format!("multipart/form-data; boundary={boundary}"))
        .body(body)
        .send()
        .await
        .unwrap();
    assert_eq!(answer.status(), 303, "das Hochladen leitet auf die Vorschau um");
    let preview = location(&answer);
    assert!(preview.starts_with("/werke/import/word?draft="), "Ziel ist die Vorschau des Entwurfs: {preview}");
    let page = client.get(process.at(&preview)).send().await.unwrap().text().await.unwrap();
    for shown in ["Ankunft", "Der Deich", "Die Schleuse", "@word:erika-beispiel", "@word:joerg-mueller"] {
        assert!(page.contains(shown), "die Vorschau zeigt Gliederung und Bearbeiter, hier „{shown}“");
    }
    assert!(
        page.contains("Änderungen und Kommentare werden nie zur aktuellen Fassung."),
        "und sagt, was mit Änderungen und Kommentaren geschieht"
    );
    let draft = preview.rsplit('=').next().unwrap();
    let answer = client
        .post(process.at("/werke/import/word/uebernehmen"))
        .form(&[("draft", draft), ("title", "Das Haus am Deich"), ("changes", "1"), ("comments", "1")])
        .send()
        .await
        .unwrap();
    assert_eq!(answer.status(), 303, "das Übernehmen leitet auf das neue Werk um");
    let target = location(&answer);
    let work: i64 = target.strip_prefix("/?work=").and_then(|id| id.parse().ok()).expect("Umleitung auf /?work=<id>");

    // Word export: page with a GET form, file with open proposals as change tracking.
    let page = client.get(process.at(&format!("/werke/{work}/word"))).send().await.unwrap().text().await.unwrap();
    assert!(
        page.contains(&format!("action=\"/werke/{work}/word.docx\"")) && page.contains("name=\"vorschlaege\""),
        "die Export-Seite ist ein gewöhnliches Formular mit dem Häkchen für die Vorschläge"
    );
    let answer = client.get(process.at(&format!("/werke/{work}/word.docx?vorschlaege=1"))).send().await.unwrap();
    assert_eq!(answer.status(), 200, "die Datei kommt als Antwort auf das Formular");
    assert_eq!(
        answer.headers().get("content-type").unwrap(),
        baruch_docx::ooxml::MEDIA_TYPE_DOCX,
        "mit dem Medientyp von Word"
    );
    let file = answer.bytes().await.unwrap().to_vec();
    let document = text(&file, "word/document.xml");
    assert!(
        document.contains("<w:ins ") && document.contains(&format!("w:author=\"{JOERG}\"")),
        "der jüngste offene Vorschlag steht als Einfügung mit seinem Bearbeiter in der Datei"
    );

    // Quit: rejected under the proxy name and over MCP, allowed from the own machine with a confirmation prompt.
    let answer = client.get(process.at("/beenden")).header("host", PROXY_NAME).send().await.unwrap();
    assert_eq!(answer.status(), 403, "unter dem erlaubten Proxy-Namen gibt es die Rückfrage nicht");
    assert!(!answer.text().await.unwrap().contains("action=\"/beenden\""), "und kein Formular zum Beenden");
    let answer = client.post(process.at("/beenden")).header("host", PROXY_NAME).send().await.unwrap();
    assert_eq!(answer.status(), 403, "auch das Formular selbst wird unter dem Proxy-Namen abgelehnt");
    let call = json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list", "params": {} });
    let tools: Value = client.post(process.at("/mcp")).json(&call).send().await.unwrap().json().await.unwrap();
    let names: Vec<&str> = tools["result"]["tools"]
        .as_array()
        .expect("Werkzeugliste")
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    assert!(names.len() > 20, "die echte Werkzeugliste: {}", names.len());
    for name in &names {
        for word in ["shutdown", "beenden", "quit", "docx", "word_import", "word_export"] {
            assert!(!name.contains(word), "kein MCP-Werkzeug zum Beenden oder für Word: {name}");
        }
    }
    for tool in ["shutdown", "beenden", "docx_import", "docx_export"] {
        let call =
            json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": { "name": tool, "arguments": {} } });
        let answer: Value = client.post(process.at("/mcp")).json(&call).send().await.unwrap().json().await.unwrap();
        assert_eq!(answer["error"]["code"], -32602, "MCP kennt das Werkzeug {tool} nicht: {answer}");
    }
    tokio::time::sleep(Duration::from_millis(900)).await;
    assert!(process.child.try_wait().unwrap().is_none(), "keiner der abgelehnten Versuche hat den Server beendet");

    let page = client.get(process.at("/beenden")).send().await.unwrap();
    assert_eq!(page.status(), 200, "vom eigenen Rechner kommt die Rückfrage");
    let page = page.text().await.unwrap();
    assert!(
        page.contains("method=\"post\"") && page.contains("action=\"/beenden\""),
        "als gewöhnliches Formular (ohne JavaScript bedienbar)"
    );
    let done = client.post(process.at("/beenden")).send().await.unwrap();
    assert_eq!(done.status(), 200, "das Formular vom eigenen Rechner wird angenommen");
    let done = done.text().await.unwrap();
    assert!(done.contains(baruch_server::shutdown::DONE_TEXT), "mit dem Satz, dass Baruch beendet ist");
    assert!(
        !done.contains("<link") && !done.contains("src="),
        "die Abschlussseite lädt nichts nach – der Server liefert danach nichts mehr"
    );
    let (code, lines) = process.finish(baruch_server::shutdown::GRACE + Duration::from_secs(10));
    assert_eq!(code, Some(0), "der Prozess endet mit Exit 0");
    assert_eq!(
        lines.last().map(String::as_str),
        Some(baruch_server::shutdown::GOODBYE),
        "letzte Zeile der Standardausgabe: {lines:?}"
    );

    // Database after quitting: consistent in itself, and everything written is in it.
    let state = baruch_server::AppState::open(process.data.path()).await.expect("der Datenordner öffnet sich wieder");
    let check: String = sqlx::query_scalar("PRAGMA integrity_check").fetch_one(state.store.pool()).await.unwrap();
    assert_eq!(check, "ok", "SQLite findet die Datenbank nach dem Beenden unversehrt");
    let loaded = state.store.load_work(work).await.expect("das importierte Werk ist da");
    assert_eq!(loaded.title, "Das Haus am Deich", "mit seinem Titel");
    assert_eq!(loaded.nodes[0].revisions.len(), 3, "mit Fassung 1 und den beiden Vorschlägen aus Word");
    assert_eq!(loaded.nodes.iter().map(|n| n.posts.len()).sum::<usize>(), 4, "und den vier Beiträgen");
    state.store.pool().close().await;
}

/// `SIGTERM` (systemctl stop, kill) ends the same process just as cleanly – even immediately after the line „Baruch
/// läuft“: the receivers of the signals are set up beforehand (otherwise the process would end hard on the signal).
#[cfg(unix)]
#[tokio::test]
async fn sigterm_ends_the_throwaway_server_with_exit_0() {
    let mut process = Process::start().await;
    let sent = Command::new("kill").arg("-TERM").arg(process.child.id().to_string()).status().unwrap();
    assert!(sent.success(), "SIGTERM nur an den eigenen Kindprozess (per PID)");
    let (code, lines) = process.finish(baruch_server::shutdown::GRACE + Duration::from_secs(10));
    assert_eq!(code, Some(0), "SIGTERM endet mit Exit 0, nicht mit einem Abbruch durch das Signal");
    assert_eq!(
        lines.last().map(String::as_str),
        Some(baruch_server::shutdown::GOODBYE),
        "letzte Zeile der Standardausgabe: {lines:?}"
    );
    let state = baruch_server::AppState::open(process.data.path()).await.expect("der Datenordner öffnet sich wieder");
    let check: String = sqlx::query_scalar("PRAGMA integrity_check").fetch_one(state.store.pool()).await.unwrap();
    assert_eq!(check, "ok", "die Datenbank ist nach SIGTERM unversehrt");
    state.store.pool().close().await;
}

// ---- CI, MCP, docs ----------------------------------------------------------------------------------------------

/// The rule check of the workflows recognises `pages.yml` and `release.yml` with separate jobs as valid.
#[cfg(unix)]
#[test]
fn the_workflow_check_accepts_the_separated_pages_jobs() {
    let run = Command::new("sh").arg("scripts/ci-check-workflows.sh").current_dir(root()).output().unwrap();
    let said = format!("{}{}", String::from_utf8_lossy(&run.stdout), String::from_utf8_lossy(&run.stderr));
    assert!(run.status.success(), "scripts/ci-check-workflows.sh nimmt die Arbeitsabläufe an: {said}");
    for workflow in ["pages.yml", "release.yml"] {
        let text = std::fs::read_to_string(root().join(".github/workflows").join(workflow)).unwrap();
        assert!(
            text.contains("\n  pages-build:\n")
                && text.contains("\n  pages-deploy:\n")
                && !text.contains("\n  pages:\n"),
            "{workflow}: bauen und ausliefern sind zwei Jobs – genau diesen Stand hat die Prüfung angenommen"
        );
    }
}

/// The docs have been brought up to date: user guide, RPC, MCP, architecture, README and CHANGELOG name the round; the
/// building blocks of the developers are inserted and deleted; no stub of the round is open.
#[test]
fn the_documentation_names_word_and_quitting_and_no_stub_is_left() {
    let read = |path: &str| std::fs::read_to_string(root().join(path)).unwrap_or_else(|e| panic!("{path}: {e}"));
    for (path, needles) in [
        // Round 37: the user guide for users is in the help of the website. The visible texts of the user interface
        // (menu entries) are therefore searched for in the responsible help page, the technical part (route, origin of
        // the Word proposals) still in docs/Bedienung.md, which since then is only a pointer and a technical note.
        ("site/content/de/hilfe/einlesen.md", &["Word (.docx) importieren…"][..]),
        ("site/content/de/hilfe/erste-schritte.md", &["Baruch beenden…"][..]),
        ("docs/Bedienung.md", &["/werke/<id>/word", "@word:"][..]),
        ("docs/RPC.md", &["docx_import.preview", "docx_import.apply", "work.docx_export", "/beenden"][..]),
        ("docs/MCP.md", &["Word import", "Word export", "Quitting Baruch exists neither over RPC nor over MCP"][..]),
        ("docs/Architektur.md", &["`baruch-docx`", "docx_import", "docx_export", "shutdown.rs"][..]),
        ("README.md", &["Word (.docx)", "--docx", "Baruch beenden"][..]),
        ("CHANGELOG.md", &["Word-Import", "Word-Export", "Baruch beenden"][..]),
    ] {
        let text = read(path);
        for needle in needles {
            assert!(text.contains(needle), "{path} nennt „{needle}“ (Doku wächst mit dem Code)");
        }
    }
    assert!(!root().join("docs/r32").exists(), "die Bausteine docs/r32 sind eingesetzt und gelöscht");
    let changelog = read("CHANGELOG.md");
    assert!(!changelog.contains("(in Arbeit)"), "der CHANGELOG nennt die Runde nicht mehr als „in Arbeit“");

    fn sources(dir: &Path, out: &mut Vec<PathBuf>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                if path.file_name().is_some_and(|name| name != "target") {
                    sources(&path, out);
                }
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                out.push(path);
            }
        }
    }
    let mut files = Vec::new();
    sources(&root().join("crates/baruch-docx/src"), &mut files);
    sources(&root().join("crates/baruch-server/src"), &mut files);
    assert!(files.len() > 60, "alle Quelldateien gelesen: {}", files.len());
    let marker = ["STUB", ":"].concat();
    let open: Vec<String> = files
        .iter()
        .filter(|path| {
            std::fs::read_to_string(path).unwrap().lines().any(|line| line.contains(&marker) && line.contains("R32"))
        })
        .map(|path| path.display().to_string())
        .collect();
    assert_eq!(open, Vec::<String>::new(), "kein Stub der Runde 32 ist mehr offen");
}
