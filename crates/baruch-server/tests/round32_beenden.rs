// SPDX-License-Identifier: AGPL-3.0-or-later
//! Runde 32, Aufgabe beenden: Seite und Formular /beenden, Herunterfahren, Starter.
//!
//! - Seiten und Schutz: echter Server im Testprozess (`shutdown::serve_until` auf `127.0.0.1:0`, kurze Frist,
//!   keine Signale), damit die Gegenstelle (`ConnectInfo`) wie im Betrieb vorliegt; ohne Gegenstelle über `oneshot`.
//! - Prozess: `baruch-server serve --listen 127.0.0.1:0 --data <Temp> --secrets memory`. Vorher stellt der Test im
//!   Datenordner die Verbindungen `eliezer` und `multigpt` auf `http://127.0.0.1:9` (Umweg aus SKILLS.md), damit
//!   beim Start keine Anfrage an einen echten Dienst geht. Beendet wird nur über die Oberfläche bzw. per Signal an
//!   die eigene PID.
//! - Starter: `packaging/linux/baruch` mit Attrappen in einem abgeschlossenen `PATH` – nie ein echter Browser.

use crate::common;

use std::io::{BufRead, BufReader};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode, header};
use baruch_server::shutdown::{self, DONE_TEXT, GOODBYE, GRACE, REPLY_DELAY};
use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// Länger als [`REPLY_DELAY`]: Wäre ein Auslöser unterwegs, hätte er bis dahin gewirkt.
const AFTER_DELAY: Duration = Duration::from_millis(900);

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap()
}

fn client() -> reqwest::Client {
    reqwest::Client::builder().no_proxy().build().unwrap()
}

// ---- Server im Testprozess ----------------------------------------------------------------------------------

struct Running {
    address: SocketAddr,
    state: Arc<baruch_server::AppState>,
    served: tokio::task::JoinHandle<std::io::Result<()>>,
    _data: tempfile::TempDir,
}

impl Running {
    fn url(&self, path: &str) -> String {
        format!("http://{}{path}", self.address)
    }
}

/// Echter Server im Testprozess mit eigener Frist; `proxy_names` wie `--allow-host`.
async fn run(grace: Duration, proxy_names: &str) -> Running {
    let data = tempfile::tempdir().unwrap();
    let mut state = baruch_server::AppState::open(data.path()).await.expect("Datenordner lässt sich nicht öffnen");
    state.allowed_hosts = baruch_server::guard::AllowedHosts::parse(proxy_names).unwrap();
    let state = Arc::new(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let served = tokio::spawn(shutdown::serve_until(listener, state.clone(), grace, std::future::pending()));
    Running { address, state, served, _data: data }
}

/// Wartet auf das Ende des Servers; `Err` nach `limit`.
async fn ended(running: &mut Running, limit: Duration) -> Result<Duration, ()> {
    let begin = Instant::now();
    match tokio::time::timeout(limit, &mut running.served).await {
        Ok(result) => {
            result.expect("der Server endet ohne Panik").expect("der Server endet ohne Fehler");
            Ok(begin.elapsed())
        }
        Err(_) => Err(()),
    }
}

#[tokio::test]
async fn the_page_asks_first_and_works_as_a_plain_form() {
    let mut running = run(Duration::from_secs(5), "").await;
    let answer = client().get(running.url("/beenden")).send().await.unwrap();
    assert_eq!(answer.status(), StatusCode::OK, "vom eigenen Rechner (Loopback, Host ist die Adresse): die Rückfrage");
    let html = answer.text().await.unwrap();
    assert!(html.contains("Soll Baruch auf diesem Rechner beendet werden?"), "die Seite fragt, bevor etwas geschieht");
    assert!(
        html.contains(r#"<form class="shutdown-form" method="post" action="/beenden""#),
        "ein gewöhnliches Formular mit POST: ohne JavaScript bedienbar: {html}"
    );
    assert!(
        html.contains(r#"<button class="btn danger" type="submit">Baruch beenden</button>"#),
        "der Knopf heißt wie der Menüeintrag und schickt das Formular ab"
    );
    assert!(html.contains(r#"<a class="btn" href="/">Abbrechen</a>"#), "Abbrechen ist ein Link zurück, kein Skript");
    assert!(html.contains(r#"href="/beenden""#), "das Menü Baruch ▾ der Seite selbst führt den Eintrag (base.html)");
    tokio::time::sleep(AFTER_DELAY).await;
    assert!(!running.state.shutdown.is_requested(), "die Rückfrage allein (GET) beendet nichts");
    assert!(ended(&mut running, Duration::from_millis(50)).await.is_err(), "der Server läuft weiter");

    // Mit dem Cookie des zuletzt geöffneten Werks führt „Abbrechen“ dorthin zurück.
    let work = running.state.default_work().await.unwrap();
    let html = client()
        .get(running.url("/beenden"))
        .header(header::COOKIE, format!("baruch_work={work}"))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(
        html.contains(&format!(r#"<a class="btn" href="/?work={work}">Abbrechen</a>"#)),
        "Abbrechen kehrt zum zuletzt geöffneten Werk zurück: {html}"
    );
}

#[tokio::test]
async fn under_a_proxy_name_the_page_explains_and_offers_no_form() {
    const NAME: &str = "baruch.example.org";
    let mut running = run(Duration::from_secs(5), NAME).await;
    let answer = client().get(running.url("/beenden")).header(header::HOST, NAME).send().await.unwrap();
    assert_eq!(
        answer.status(),
        StatusCode::FORBIDDEN,
        "Loopback, aber unter dem Namen des Reverse-Proxys: Dahinter kann jeder Rechner stehen"
    );
    let html = answer.text().await.unwrap();
    assert!(html.contains("Baruch lässt sich nur am eigenen Rechner beenden."), "ehrliche Erklärung: {html}");
    assert!(html.contains("Reverse-Proxy"), "nennt den Grund (anderer Rechner oder vorgeschalteter Server)");
    assert!(!html.contains(r#"action="/beenden""#), "kein Formular, das doch nur abgelehnt würde");
    assert!(!html.contains(r#"type="submit">Baruch beenden"#), "und kein Knopf");

    let answer = client().post(running.url("/beenden")).header(header::HOST, NAME).send().await.unwrap();
    assert_eq!(answer.status(), StatusCode::FORBIDDEN, "auch das Formular selbst ist unter dem Proxy-Namen abgelehnt");
    assert!(!answer.text().await.unwrap().contains(DONE_TEXT), "die Antwort behauptet kein Beenden");

    // Unter der Adresse geht es am selben Server weiterhin (Proxy-Namen sperren den eigenen Rechner nicht aus).
    let answer = client().get(running.url("/beenden")).send().await.unwrap();
    assert_eq!(answer.status(), StatusCode::OK, "direkt am Rechner bleibt die Rückfrage erreichbar");

    tokio::time::sleep(AFTER_DELAY).await;
    assert!(!running.state.shutdown.is_requested(), "der abgelehnte POST hat nichts ausgelöst");
    assert!(ended(&mut running, Duration::from_millis(50)).await.is_err(), "der Server läuft weiter");
}

#[tokio::test]
async fn a_foreign_origin_or_an_unknown_peer_cannot_quit() {
    let mut running = run(Duration::from_secs(5), "").await;
    let answer =
        client().post(running.url("/beenden")).header(header::ORIGIN, "http://fremd.example.net").send().await.unwrap();
    assert_eq!(answer.status(), StatusCode::FORBIDDEN, "Formular einer fremden Seite: guard::same_origin lehnt ab");
    assert!(answer.text().await.unwrap().starts_with("Abgelehnt:"), "die Ablehnung kommt vom Schutz der Schreibwege");
    let answer = client().post(running.url("/beenden")).header("sec-fetch-site", "cross-site").send().await.unwrap();
    assert_eq!(answer.status(), StatusCode::FORBIDDEN, "ebenso mit Sec-Fetch-Site: cross-site");

    // Ohne Gegenstelle (oneshot kennt keine): im Zweifel nicht der eigene Rechner.
    let server = common::open().await;
    let (status, html) = server.get("/beenden").await;
    assert_eq!(status, StatusCode::FORBIDDEN, "ohne bekannte Gegenstelle zeigt GET die Erklärung");
    assert!(!html.contains(r#"action="/beenden""#), "ohne Formular: {html}");
    let (status, _, html) = server.send(Request::post("/beenden").body(Body::empty()).unwrap()).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "ohne bekannte Gegenstelle ist POST abgelehnt");
    assert!(!html.contains(DONE_TEXT), "keine Abschlussseite");
    // Eine fremde Gegenstelle (Betrieb mit --listen im Heimnetz) ebenso.
    let mut request = Request::post("/beenden").header(header::HOST, "192.168.7.20:8765").body(Body::empty()).unwrap();
    request.extensions_mut().insert(ConnectInfo(SocketAddr::from(([192, 168, 7, 21], 40000))));
    let (status, _, _) = server.send(request).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "ein anderer Rechner im Netz kann Baruch nicht beenden");

    tokio::time::sleep(AFTER_DELAY).await;
    assert!(!server.state.shutdown.is_requested(), "nach den Ablehnungen ohne Gegenstelle ist nichts verlangt");
    assert!(!running.state.shutdown.is_requested(), "nach den Ablehnungen fremder Herkunft ist nichts verlangt");
    assert!(ended(&mut running, Duration::from_millis(50)).await.is_err(), "der Server läuft weiter");
}

#[tokio::test]
async fn quitting_answers_with_a_self_contained_page_and_stops_the_server() {
    let mut running = run(Duration::from_secs(5), "").await;
    let begin = Instant::now();
    let answer = client()
        .post(running.url("/beenden"))
        .header(header::ORIGIN, format!("http://{}", running.address))
        .send()
        .await
        .unwrap();
    assert_eq!(answer.status(), StatusCode::OK, "vom eigenen Rechner und von der eigenen Seite: angenommen");
    assert!(!running.state.shutdown.is_requested(), "der Auslöser kommt erst nach REPLY_DELAY, die Antwort zuerst");
    let html = answer.text().await.unwrap();
    assert!(html.contains(&format!("<h1>{DONE_TEXT}.</h1>")), "der Satz steht als Text da, auch ohne JavaScript");
    assert!(
        html.contains("<script>window.close();</script>"),
        "mit JavaScript versucht die Seite, das Fenster zu schließen"
    );
    assert_eq!(html.matches("<script").count(), 1, "nur dieses eine Skript, inline");
    for forbidden in ["<link", " src=", " href=", "@import", "url(", "<img", "<iframe", "/static/"] {
        assert!(
            !html.contains(forbidden),
            "die Abschlussseite verweist auf keine Datei – der Server liefert gleich nichts mehr ({forbidden}): {html}"
        );
    }
    let script = html.find("<script>").unwrap();
    assert!(html.find(DONE_TEXT).unwrap() < script, "der Text steht im Dokument, nicht erst vom Skript erzeugt");

    let waited = ended(&mut running, Duration::from_secs(4)).await.expect("der Server endet von selbst");
    assert!(begin.elapsed() >= REPLY_DELAY, "nicht vor REPLY_DELAY: Die Antwort soll den Browser erreichen");
    assert!(
        waited < Duration::from_secs(4),
        "ohne laufende Anfragen endet er sofort nach dem Auslöser, nicht erst zur Frist"
    );
    assert!(running.state.shutdown.is_requested(), "beendet hat der Auslöser im AppState");
    assert!(running.state.store.pool().is_closed(), "die Datenbank ist nach dem Ende geschlossen");
    assert!(
        client().get(running.url("/")).send().await.is_err(),
        "nach dem Ende nimmt der Server keine Verbindung mehr an"
    );
}

/// Kopfzeilen und der erste Teil des Rumpfs einer RPC-Anfrage; der Rest (`.1`) fehlt noch.
fn half_request(address: SocketAddr) -> (String, String) {
    let body = json!({ "jsonrpc": "2.0", "id": 7, "method": "work.list", "params": {} }).to_string();
    let (first, rest) = body.split_at(body.len() / 2);
    let head = format!(
        "POST /rpc HTTP/1.1\r\nHost: {address}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\
         Connection: close\r\n\r\n{first}",
        body.len()
    );
    (head, rest.to_owned())
}

#[tokio::test]
async fn a_running_request_is_still_answered_after_the_trigger() {
    let mut running = run(Duration::from_secs(30), "").await;
    let (head, rest) = half_request(running.address);
    let mut stream = tokio::net::TcpStream::connect(running.address).await.unwrap();
    stream.write_all(head.as_bytes()).await.unwrap();
    stream.flush().await.unwrap();
    // Der Server hat die Kopfzeilen und wartet auf den Rest: Die Anfrage läuft.
    tokio::time::sleep(Duration::from_millis(300)).await;
    running.state.shutdown.request();
    assert!(
        ended(&mut running, Duration::from_millis(700)).await.is_err(),
        "nach dem Auslöser wartet der Server auf die laufende Anfrage (Frist 30 s), statt sie abzuschneiden"
    );
    assert!(
        tokio::net::TcpStream::connect(running.address).await.is_err()
            || client().get(running.url("/")).timeout(Duration::from_secs(2)).send().await.is_err(),
        "neue Anfragen nimmt er dabei nicht mehr an"
    );
    stream.write_all(rest.as_bytes()).await.unwrap();
    let mut answer = String::new();
    stream.read_to_string(&mut answer).await.unwrap();
    assert!(answer.starts_with("HTTP/1.1 200"), "die laufende Anfrage bekommt ihre Antwort: {answer}");
    let json: Value =
        serde_json::from_str(answer.split("\r\n\r\n").nth(1).unwrap_or_default().trim_matches(|c: char| {
            // Bei `Transfer-Encoding: chunked` stehen Längenzeilen um den Rumpf.
            c.is_ascii_hexdigit() || c.is_whitespace()
        }))
        .unwrap_or_else(|e| panic!("vollständige JSON-Antwort erwartet ({e}): {answer}"));
    assert!(json["result"]["works"].is_array(), "und zwar die ganze, mit Daten aus der noch offenen Datenbank: {json}");
    let waited = ended(&mut running, Duration::from_secs(10)).await.expect("danach endet der Server");
    assert!(waited < Duration::from_secs(10), "sobald die letzte Antwort draußen ist – nicht erst zur Frist von 30 s");
    assert!(running.state.store.pool().is_closed(), "erst dann wurde die Datenbank geschlossen");
}

#[tokio::test]
async fn an_open_connection_holds_the_shutdown_no_longer_than_the_grace_period() {
    // Ersatz für einen offenen Strom (Live-Chat): eine Verbindung mitten in einer Anfrage, die nie zu Ende geht.
    // Für das Herunterfahren ist beides dasselbe – eine Verbindung, die von selbst nicht endet.
    const SHORT: Duration = Duration::from_millis(600);
    let mut running = run(SHORT, "").await;
    let (head, _never_sent) = half_request(running.address);
    let mut stream = tokio::net::TcpStream::connect(running.address).await.unwrap();
    stream.write_all(head.as_bytes()).await.unwrap();
    stream.flush().await.unwrap();
    tokio::time::sleep(Duration::from_millis(300)).await;
    let begin = Instant::now();
    running.state.shutdown.request();
    ended(&mut running, Duration::from_secs(8))
        .await
        .expect("die offene Verbindung hält das Beenden nicht auf Dauer auf");
    let waited = begin.elapsed();
    assert!(waited >= SHORT, "bis zur Frist bekommt die Verbindung Zeit ({waited:?})");
    assert!(
        waited < SHORT + Duration::from_secs(4),
        "danach wird nicht länger gewartet (Frist {SHORT:?}, gewartet {waited:?}; im Betrieb GRACE = {GRACE:?})"
    );
    assert!(running.state.store.pool().is_closed(), "auch dann ist die Datenbank am Ende geschlossen");
    drop(stream);
}

// ---- kein RPC, kein MCP -------------------------------------------------------------------------------------

#[tokio::test]
async fn there_is_no_rpc_method_and_no_mcp_tool_to_quit() {
    let server = common::open().await;
    for method in [
        "shutdown",
        "shutdown.request",
        "server.shutdown",
        "server.quit",
        "server.stop",
        "app.shutdown",
        "app.quit",
        "system.shutdown",
        "baruch.beenden",
        "beenden",
    ] {
        let answer = server.rpc(method, json!({})).await;
        assert_eq!(answer["error"]["code"], -32601, "{method}: Beenden ist keine RPC-Methode: {answer}");
    }
    let message = json!({ "jsonrpc": "2.0", "id": 3, "method": "tools/list", "params": {} });
    let mut request = Request::post("/mcp")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(message.to_string()))
        .unwrap();
    request.extensions_mut().insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 40000))));
    let (status, _, body) = server.send(request).await;
    assert_eq!(status, StatusCode::OK, "tools/list: {body}");
    let answer: Value = serde_json::from_str(&body).unwrap();
    let tools = answer["result"]["tools"].as_array().expect("Werkzeugliste");
    assert!(tools.len() > 20, "die Liste ist die echte (viele Werkzeuge), nicht leer: {}", tools.len());
    for tool in tools {
        let name = tool["name"].as_str().unwrap();
        for word in ["shutdown", "beenden", "quit", "exit", "terminate", "herunterfahr", "stop_server"] {
            assert!(!name.contains(word), "kein MCP-Werkzeug zum Beenden: {name}");
        }
    }
    assert!(!server.state.shutdown.is_requested(), "keiner der Aufrufe hat etwas ausgelöst");

    // Im Quelltext verlangt nur shutdown.rs das Herunterfahren – kein anderer Weg (RPC, MCP, Seiten) kommt heran.
    fn sources(dir: &Path, out: &mut Vec<PathBuf>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                sources(&path, out);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                out.push(path);
            }
        }
    }
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    sources(&src, &mut files);
    assert!(files.len() > 50, "alle Quelldateien des Servers gelesen: {}", files.len());
    let callers: Vec<String> = files
        .iter()
        .filter(|path| std::fs::read_to_string(path).unwrap().contains("shutdown.request()"))
        .map(|path| path.strip_prefix(&src).unwrap().display().to_string())
        .collect();
    assert_eq!(callers, ["shutdown.rs"], "nur POST /beenden löst aus (die Signale gehen nicht über den Auslöser)");
}

// ---- echter Prozess -----------------------------------------------------------------------------------------

/// Laufender `baruch-server serve`; beim Verwerfen wird der eigene Kindprozess beendet (falls ein Test scheitert).
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
    /// Datenordner vorbereiten (keine Anfrage an echte Dienste), Server starten, auf „Baruch läuft“ warten.
    async fn start() -> Process {
        let data = tempfile::tempdir().unwrap();
        {
            let state = Arc::new(baruch_server::AppState::open(data.path()).await.expect("Datenordner"));
            let app = baruch_server::router(state.clone());
            for kind in ["eliezer", "multigpt"] {
                let set = json!({ "kind": kind, "config": { "url": "http://127.0.0.1:9" } });
                let answer = common::rpc(app.clone(), "connection.set", set).await;
                assert!(answer["error"].is_null(), "Verbindung {kind} auf eine tote Adresse gestellt: {answer}");
            }
            // Der Testprozess gibt die Datenbank ab: Nur so ist der Server später ihre letzte Verbindung.
            state.store.pool().close().await;
        }
        let mut child = Command::new(env!("CARGO_BIN_EXE_baruch-server"))
            .args(["serve", "--listen", "127.0.0.1:0", "--secrets", "memory", "--data"])
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

    /// Schreibt etwas über die Oberfläche, damit das Protokoll der Datenbank (`-wal`) nicht leer ist.
    async fn write_something(&self) {
        let body = json!({ "jsonrpc": "2.0", "id": 1, "method": "connection.set",
            "params": { "kind": "multigpt", "config": { "url": "http://127.0.0.1:9/mcp" } } });
        let answer: Value =
            client().post(format!("{}/rpc", self.url)).json(&body).send().await.unwrap().json().await.unwrap();
        assert!(answer["error"].is_null(), "Schreiben über RPC am laufenden Server: {answer}");
        let wal = self.data.path().join("baruch.sqlite-wal");
        assert!(
            std::fs::metadata(&wal).is_ok_and(|meta| meta.len() > 0),
            "Vorbedingung: Im Betrieb liegen Änderungen im Protokoll (-wal) neben der Datenbank"
        );
    }

    /// Wartet auf das Ende des Prozesses; liefert Exit-Code und alle weiteren Zeilen der Standardausgabe.
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

    /// Die Datenbank wurde sauber geschlossen: SQLite hat das Protokoll in die Datei zurückgeschrieben.
    async fn assert_closed_cleanly(&self, how: &str) {
        for suffix in ["-wal", "-shm"] {
            let file = self.data.path().join(format!("baruch.sqlite{suffix}"));
            assert!(
                !std::fs::metadata(&file).is_ok_and(|meta| meta.len() > 0),
                "{how}: Beim Schließen der letzten Verbindung schreibt SQLite das Protokoll zurück und entfernt \
                 {suffix}; läge es noch mit Inhalt da, wäre der Prozess ohne Schließen geendet"
            );
        }
        let state = Arc::new(baruch_server::AppState::open(self.data.path()).await.expect("erneutes Öffnen gelingt"));
        let row =
            common::rpc(baruch_server::router(state.clone()), "connection.get", json!({ "kind": "multigpt" })).await;
        state.store.pool().close().await;
        assert_eq!(
            row["result"]["config"]["url"], "http://127.0.0.1:9/mcp",
            "{how}: Die letzte Änderung steht in der Datenbankdatei selbst (kein Protokoll mehr nötig): {row}"
        );
    }
}

#[tokio::test]
async fn the_button_ends_the_real_process_cleanly() {
    let mut process = Process::start().await;
    process.write_something().await;
    let answer = client().post(format!("{}/beenden", process.url)).send().await.unwrap();
    assert_eq!(answer.status(), StatusCode::OK, "POST /beenden am echten Server vom eigenen Rechner");
    assert!(answer.text().await.unwrap().contains(DONE_TEXT), "mit der Abschlussseite");
    let (code, lines) = process.finish(GRACE + Duration::from_secs(10));
    assert_eq!(code, Some(0), "sauberes Ende: Exit 0 (systemd mit Restart=on-failure startet dann nicht neu)");
    assert_eq!(lines.last().map(String::as_str), Some(GOODBYE), "letzte Zeile der Standardausgabe: {lines:?}");
    assert!(
        lines.iter().any(|line| line.contains("über die Oberfläche")),
        "die Ausgabe nennt den Anlass (Protokoll des Dienstes): {lines:?}"
    );
    process.assert_closed_cleanly("nach dem Knopf").await;
}

#[cfg(unix)]
#[tokio::test]
async fn sigterm_ends_the_real_process_as_cleanly() {
    let mut process = Process::start().await;
    process.write_something().await;
    // Nur an die eigene PID (kein pkill, kein Muster).
    let sent = Command::new("kill").arg("-TERM").arg(process.child.id().to_string()).status().unwrap();
    assert!(sent.success(), "SIGTERM an den eigenen Kindprozess");
    let (code, lines) = process.finish(GRACE + Duration::from_secs(10));
    assert_eq!(code, Some(0), "SIGTERM (systemctl stop, kill) endet wie Strg+C mit Exit 0, nicht mit Signal-Abbruch");
    assert_eq!(lines.last().map(String::as_str), Some(GOODBYE), "letzte Zeile der Standardausgabe: {lines:?}");
    assert!(lines.iter().any(|line| line.contains("SIGTERM")), "die Ausgabe nennt das Signal: {lines:?}");
    process.assert_closed_cleanly("nach SIGTERM").await;
}

#[cfg(unix)]
#[tokio::test]
async fn ctrl_c_ends_the_real_process_the_same_way() {
    let mut process = Process::start().await;
    process.write_something().await;
    let sent = Command::new("kill").arg("-INT").arg(process.child.id().to_string()).status().unwrap();
    assert!(sent.success(), "SIGINT (Strg+C) an den eigenen Kindprozess");
    let (code, lines) = process.finish(GRACE + Duration::from_secs(10));
    assert_eq!(code, Some(0), "Strg+C: Exit 0 wie vor Runde 32");
    assert_eq!(lines.last().map(String::as_str), Some(GOODBYE), "neu: auch hier die Schlusszeile: {lines:?}");
    process.assert_closed_cleanly("nach Strg+C").await;
}

// ---- Starter ------------------------------------------------------------------------------------------------

#[cfg(unix)]
mod starter {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    /// Abgeschlossene Umgebung für den Starter: `PATH` enthält nur Attrappen und die paar Werkzeuge, die das Skript
    /// braucht – ein echter Browser des Rechners ist so nicht erreichbar.
    struct Sandbox {
        dir: tempfile::TempDir,
        /// Lauscht, damit der Starter „Server antwortet schon“ feststellt und nur den Browser öffnet.
        listener: std::net::TcpListener,
    }

    impl Sandbox {
        fn new() -> Sandbox {
            let dir = tempfile::tempdir().unwrap();
            std::fs::create_dir(dir.path().join("bin")).unwrap();
            let sandbox = Sandbox { dir, listener: std::net::TcpListener::bind("127.0.0.1:0").unwrap() };
            for tool in ["dirname", "readlink", "uname"] {
                let real = ["/usr/bin", "/bin"].iter().map(|dir| Path::new(dir).join(tool)).find(|p| p.exists());
                std::os::unix::fs::symlink(real.unwrap_or_else(|| panic!("{tool} fehlt")), sandbox.bin().join(tool))
                    .unwrap();
            }
            // Wird nie gestartet (der Server „läuft schon“), muss aber ausführbar sein.
            sandbox.script(&sandbox.dir.path().join("baruch-server"), "exit 0");
            sandbox
        }

        fn bin(&self) -> PathBuf {
            self.dir.path().join("bin")
        }

        fn log(&self) -> PathBuf {
            self.dir.path().join("aufrufe.log")
        }

        fn script(&self, path: &Path, body: &str) {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, format!("#!/bin/sh\n{body}\n")).unwrap();
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
        }

        /// Attrappe eines Programms: schreibt Namen und Argumente in das Protokoll, startet nichts.
        fn decoy_at(&self, path: &Path, label: &str) {
            self.script(path, &format!("echo \"{label} $*\" >> '{}'", self.log().display()));
        }

        fn decoy(&self, name: &str) {
            self.decoy_at(&self.bin().join(name), name);
        }

        fn url(&self) -> String {
            format!("http://{}/", self.listener.local_addr().unwrap())
        }

        /// Führt den Starter aus und liefert die protokollierten Aufrufe (nach kurzem Warten: Der Starter schickt
        /// den Browser in den Hintergrund).
        fn start(&self, env: &[(&str, &str)], expect_call: bool) -> Vec<String> {
            let _ = std::fs::remove_file(self.log());
            let mut command = Command::new("/bin/bash");
            command
                .arg(root().join("packaging/linux/baruch"))
                .env_clear()
                .env("PATH", self.bin())
                .env("HOME", self.dir.path())
                .env("BARUCH_SERVER", self.dir.path().join("baruch-server"))
                .env("BARUCH_LISTEN", self.listener.local_addr().unwrap().to_string());
            for (key, value) in env {
                command.env(key, value);
            }
            let out = command.output().expect("Starter läuft");
            let stderr = String::from_utf8_lossy(&out.stderr);
            assert_eq!(out.status.code(), Some(0), "der Starter endet ohne Fehler: {stderr}");
            assert!(
                String::from_utf8_lossy(&out.stdout).contains("Server antwortet schon"),
                "er hat den lauschenden Port gefunden und keinen Server gestartet"
            );
            let end = Instant::now() + if expect_call { Duration::from_secs(5) } else { Duration::from_millis(400) };
            loop {
                let calls = std::fs::read_to_string(self.log()).unwrap_or_default();
                if !calls.is_empty() || Instant::now() >= end {
                    return calls.lines().map(str::to_owned).collect();
                }
                std::thread::sleep(Duration::from_millis(20));
            }
        }
    }

    #[test]
    fn the_scripts_are_valid_shell() {
        for script in ["packaging/linux/baruch", "packaging/appimage/AppRun"] {
            let out = Command::new("/bin/bash").arg("-n").arg(root().join(script)).output().unwrap();
            assert!(out.status.success(), "{script}: bash -n: {}", String::from_utf8_lossy(&out.stderr));
        }
    }

    #[test]
    fn by_default_a_chromium_like_browser_opens_an_app_window() {
        for name in
            ["chromium", "chromium-browser", "google-chrome", "google-chrome-stable", "brave-browser", "microsoft-edge"]
        {
            let sandbox = Sandbox::new();
            sandbox.decoy(name);
            sandbox.decoy("xdg-open");
            let calls = sandbox.start(&[], true);
            assert_eq!(
                calls,
                [format!("{name} --app={}", sandbox.url())],
                "{name}: eigenes Fenster (--app), in dem window.close() nach dem Beenden greift; kein xdg-open daneben"
            );
        }
        let sandbox = Sandbox::new();
        sandbox.decoy("chromium");
        sandbox.decoy("xdg-open");
        assert_eq!(
            sandbox.start(&[("BARUCH_BROWSER", "app")], true),
            [format!("chromium --app={}", sandbox.url())],
            "BARUCH_BROWSER=app ist die Vorgabe, ausgeschrieben dasselbe"
        );
    }

    #[test]
    fn the_standard_browser_is_used_on_request_or_without_chromium() {
        let sandbox = Sandbox::new();
        sandbox.decoy("chromium");
        sandbox.decoy("xdg-open");
        assert_eq!(
            sandbox.start(&[("BARUCH_BROWSER", "standard")], true),
            [format!("xdg-open {}", sandbox.url())],
            "BARUCH_BROWSER=standard: der Standardbrowser, auch wenn Chromium da ist"
        );
        assert_eq!(
            sandbox.start(&[("BARUCH_BROWSER", "netscape")], true),
            [format!("xdg-open {}", sandbox.url())],
            "ein unbekannter Wert startet nichts Unerwartetes, sondern den Standardbrowser"
        );

        let sandbox = Sandbox::new();
        sandbox.decoy("xdg-open");
        assert_eq!(
            sandbox.start(&[], true),
            [format!("xdg-open {}", sandbox.url())],
            "ohne Chromium-artigen Browser im PATH wie bisher xdg-open"
        );
    }

    #[test]
    fn no_browser_still_means_no_browser() {
        let sandbox = Sandbox::new();
        sandbox.decoy("chromium");
        sandbox.decoy("xdg-open");
        for mode in ["app", "standard"] {
            let calls = sandbox.start(&[("BARUCH_NO_BROWSER", "1"), ("BARUCH_BROWSER", mode)], false);
            assert!(calls.is_empty(), "BARUCH_NO_BROWSER=1 gilt vor der Browserwahl ({mode}): {calls:?}");
        }
    }

    #[test]
    fn on_macos_the_app_bundles_are_searched_and_open_is_the_fallback() {
        // macOS nachgestellt: `uname` meldet Darwin, die Programme liegen in einem eigenen Ordner.
        for app in ["Google Chrome", "Chromium", "Brave Browser", "Microsoft Edge"] {
            let sandbox = Sandbox::new();
            std::fs::remove_file(sandbox.bin().join("uname")).unwrap();
            sandbox.script(&sandbox.bin().join("uname"), "echo Darwin");
            sandbox.decoy("open");
            let apps = sandbox.dir.path().join("Programme");
            sandbox.decoy_at(&apps.join(format!("{app}.app/Contents/MacOS/{app}")), app);
            let calls = sandbox.start(&[("BARUCH_APPLICATIONS", apps.to_str().unwrap())], true);
            assert_eq!(
                calls,
                [format!("{app} --app={}", sandbox.url())],
                "{app}: Programm aus dem Bündel mit --app (Leerzeichen im Pfad überstanden); kein open daneben"
            );
            assert_eq!(
                sandbox.start(&[("BARUCH_APPLICATIONS", apps.to_str().unwrap()), ("BARUCH_BROWSER", "standard")], true),
                [format!("open {}", sandbox.url())],
                "BARUCH_BROWSER=standard: open wie bisher"
            );
        }
        let sandbox = Sandbox::new();
        std::fs::remove_file(sandbox.bin().join("uname")).unwrap();
        sandbox.script(&sandbox.bin().join("uname"), "echo Darwin");
        sandbox.decoy("open");
        sandbox.decoy("chromium");
        let empty = sandbox.dir.path().join("leer");
        std::fs::create_dir(&empty).unwrap();
        assert_eq!(
            sandbox.start(&[("BARUCH_APPLICATIONS", empty.to_str().unwrap())], true),
            [format!("open {}", sandbox.url())],
            "ohne passendes Bündel: open (ein `chromium` im PATH zählt unter macOS nicht)"
        );
    }
}

#[test]
fn the_windows_starter_prefers_edge_as_an_app_window() {
    // Nur am Text geprüft – die Datei ist auf keinem Windows gelaufen (docs/Verteilung.md nennt das).
    let cmd = std::fs::read_to_string(root().join("packaging/windows/baruch-start.cmd")).unwrap();
    assert!(cmd.is_ascii(), "cmd.exe liest die Datei in der Codepage des Systems: nur ASCII");
    let edge = cmd.find(r#"start "" "%BARUCH_EDGE%" --app=%BARUCH_URL%"#).expect("Edge als App-Fenster");
    let standard = cmd.find(r#"start "" "%BARUCH_URL%""#).expect("sonst der Standardbrowser");
    assert!(edge < standard, "erst Edge, der Standardbrowser als Rückfall");
    assert!(cmd.contains("if not defined BARUCH_EDGE goto standard"), "ohne msedge.exe der Standardbrowser");
    assert!(
        cmd.contains(r#"if /i "%BARUCH_BROWSER%"=="standard" goto standard"#),
        "BARUCH_BROWSER=standard wie unter Linux"
    );
    assert!(cmd.contains(r#"if "%BARUCH_NO_BROWSER%"=="1" goto done"#), "BARUCH_NO_BROWSER=1 öffnet nichts");
    assert_eq!(
        cmd.matches("msedge.exe").count(),
        7,
        "drei übliche Orte von msedge.exe (je Prüfung und Zuweisung) und die Kopfzeile"
    );
}

#[test]
fn the_service_units_do_not_restart_after_a_clean_exit() {
    for unit in ["packaging/systemd/baruch.service", "packaging/debian/baruch.service"] {
        let text = std::fs::read_to_string(root().join(unit)).unwrap();
        let settings: Vec<&str> = text.lines().filter(|line| line.starts_with("Restart")).collect();
        assert_eq!(
            settings,
            ["Restart=on-failure", "RestartSec=3"],
            "{unit}: on-failure startet nur nach Fehler oder Signal-Abbruch neu, nicht nach Exit 0 \
             (systemd.service(5)); Beenden über die Oberfläche endet mit Exit 0 (Prozesstest oben)"
        );
        for forbidden in ["SuccessExitStatus", "RestartForceExitStatus", "Restart=always"] {
            assert!(
                !text.lines().any(|line| !line.starts_with('#') && line.contains(forbidden)),
                "{unit}: {forbidden} würde Exit 0 doch zum Neustart machen"
            );
        }
    }
}

#[test]
fn pages_are_built_and_deployed_in_two_jobs() {
    // Die Regeln selbst prüft scripts/ci-check-workflows.sh (Gegenprobe in scripts/ci-selftest.sh); hier nur, dass
    // beide Abläufe die Trennung tragen und die Aktionen ihre geprüften SHAs behalten.
    for workflow in ["pages.yml", "release.yml"] {
        let text = std::fs::read_to_string(root().join(".github/workflows").join(workflow)).unwrap();
        let build = text.find("\n  pages-build:\n").unwrap_or_else(|| panic!("{workflow}: Job pages-build"));
        let deploy = text.find("\n  pages-deploy:\n").unwrap_or_else(|| panic!("{workflow}: Job pages-deploy"));
        assert!(build < deploy, "{workflow}: erst bauen, dann ausliefern");
        assert!(!text.contains("\n  pages:\n"), "{workflow}: der alte Job mit beidem ist weg");
        let (building, deploying) = (&text[build..deploy], &text[deploy..]);
        assert!(
            building.contains("actions/upload-pages-artifact@fc324d3547104276b827a68afc52ff2a11cc49c9 # v5.0.0"),
            "{workflow}: hochgeladen wird im bauenden Job, SHA unverändert"
        );
        assert!(!building.contains(": write"), "{workflow}: der bauende Job hat keine Schreibrechte");
        assert!(!building.contains("deploy-pages@"), "{workflow}: und liefert nicht aus");
        assert!(
            deploying.contains("actions/deploy-pages@368f82528645a54fb793d4d04e342629a3f51346 # v5.0.1"),
            "{workflow}: ausgeliefert wird im zweiten Job, SHA unverändert"
        );
        assert!(deploying.contains("needs: [pages-build]"), "{workflow}: das Ausliefern wartet auf das Bauen");
        assert!(deploying.contains("name: github-pages"), "{workflow}: Umgebung github-pages am ausliefernden Job");
        assert!(
            !deploying.contains("upload-pages-artifact@")
                && !deploying.contains("run:")
                && !deploying.contains("checkout@"),
            "{workflow}: der Job mit pages: write führt nichts aus dem Repository aus"
        );
    }
    let release = std::fs::read_to_string(root().join(".github/workflows/release.yml")).unwrap();
    let build = &release[release.find("\n  pages-build:\n").unwrap()..];
    assert!(build.contains("needs: [publish]"), "release.yml: gebaut wird nach dem Veröffentlichen");
    assert!(
        build.contains("if: ${{ !cancelled() && needs.publish.result == 'success' }}"),
        "release.yml: auch wenn macOS oder Windows scheitern"
    );
    assert!(
        build.contains("if: ${{ !cancelled() && needs.pages-build.result == 'success' }}"),
        "release.yml: das Ausliefern erbt die Bedingung, sonst würde es dann übersprungen"
    );
}
