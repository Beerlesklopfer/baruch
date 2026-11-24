// SPDX-License-Identifier: AGPL-3.0-or-later
//! Runde 24, Aufgabe „webseite“ (Schnittstelle R24-S1): Betrieb hinter einem Reverse-Proxy – erlaubte Rechnernamen
//! (`--allow-host`, `guard::AllowedHosts`), `/mcp` unter einem zusätzlichen Namen nur mit Token, leere
//! MultiGPT-Vorgabe.
//!
//! Der Proxy ist nachgestellt: Anfragen an den Router mit der `Host`-Kopfzeile, die ein Proxy durchreicht, und
//! Loopback als Gegenstelle (`ConnectInfo`), denn der Proxy läuft auf demselben Rechner. Namen sind erfunden
//! (`*.example`, `*.example.org`); kein Netz, kein Eliezer, kein MultiGPT.

use crate::common;

use std::net::SocketAddr;
use std::process::Command;
use std::sync::Arc;

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode, header};
use baruch_server::AppState;
use baruch_server::connections::multigpt::open_way;
use baruch_server::guard::AllowedHosts;
use baruch_store::ConnectionKind;
use serde_json::{Value, json};

const NAME: &str = "baruch.example.org";
const LOOPBACK: ([u8; 4], u16) = ([127, 0, 0, 1], 40000);

/// Server wie nach `serve --allow-host <names>` (leer = ohne die Angabe).
pub(crate) async fn open_allowing(names: &str) -> common::Server {
    let data = tempfile::tempdir().unwrap();
    open_in(data, names).await
}

async fn open_in(data: tempfile::TempDir, names: &str) -> common::Server {
    let mut state = AppState::open(data.path()).await.expect("Datenordner öffnen");
    state.allowed_hosts = AllowedHosts::parse(names).expect("gültige Namen im Test");
    let state = Arc::new(state);
    common::Server { app: baruch_server::router(state.clone()), state, data }
}

/// `GET /` mit dieser `Host`-Kopfzeile: Status und Text.
async fn get_as(server: &common::Server, host: &str) -> (StatusCode, String) {
    let request = Request::get("/").header(header::HOST, host).body(Body::empty()).unwrap();
    let (status, _, body) = server.send(request).await;
    (status, body)
}

/// HTML-Escaping der Vorlagen (minijinja).
fn esc(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;").replace('/', "&#x2f;")
}

/// `POST /rpc` (`work.export`, nur lesend) wie aus einem Browser: `Host` und `Origin`.
async fn rpc_as(server: &common::Server, host: &str, origin: &str) -> (StatusCode, String) {
    let body = json!({ "jsonrpc": "2.0", "id": 1, "method": "work.export", "params": {} });
    let mut request = Request::post("/rpc")
        .header(header::HOST, host)
        .header(header::ORIGIN, origin)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    request.extensions_mut().insert(ConnectInfo(SocketAddr::from(LOOPBACK)));
    let (status, _, text) = server.send(request).await;
    (status, text)
}

/// MCP `ping` an `/mcp` von Loopback (der Proxy bzw. ein Programm auf dem eigenen Rechner).
pub(crate) async fn mcp_ping(server: &common::Server, host: Option<&str>, token: Option<&str>) -> (StatusCode, String) {
    let mut request = Request::post("/mcp").header(header::CONTENT_TYPE, "application/json");
    if let Some(host) = host {
        request = request.header(header::HOST, host);
    }
    if let Some(token) = token {
        request = request.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    let message = json!({ "jsonrpc": "2.0", "id": 1, "method": "ping" });
    let mut request = request.body(Body::from(message.to_string())).unwrap();
    request.extensions_mut().insert(ConnectInfo(SocketAddr::from(LOOPBACK)));
    let (status, _, text) = server.send(request).await;
    (status, text)
}

async fn ok(server: &common::Server, method: &str, params: Value) -> Value {
    let answer = server.rpc(method, params).await;
    assert!(answer["error"].is_null(), "{method} sollte gelingen: {answer}");
    answer["result"].clone()
}

// ---- Host-Regel ------------------------------------------------------------------------------------------

#[tokio::test]
async fn without_allow_host_a_name_stays_refused() {
    let server = open_allowing("").await;
    for host in [NAME, "baruch.example.org:8443"] {
        let (status, body) = get_as(&server, host).await;
        assert_eq!(status, StatusCode::MISDIRECTED_REQUEST, "ohne --allow-host bleibt {host} abgelehnt (421): {body}");
        assert!(
            body.contains("DNS-Rebinding") && body.contains("--allow-host"),
            "die Meldung nennt den Grund und den Weg für den Betrieb hinter einem Proxy: {body}"
        );
        assert!(!body.contains("<html"), "die Ablehnung gibt keine Seite heraus: {body}");
    }
    let (status, _) = get_as(&server, "127.0.0.1:8765").await;
    assert_eq!(status, StatusCode::OK, "die IP-Adresse geht wie bisher");
}

#[tokio::test]
async fn an_allowed_name_passes_with_and_without_port_and_nothing_else_does() {
    let server = open_allowing(NAME).await;
    for host in [NAME, "baruch.example.org:8443", "Baruch.Example.ORG"] {
        let (status, body) = get_as(&server, host).await;
        assert_eq!(status, StatusCode::OK, "{host}: erlaubter Name, Port beliebig, Groß/klein egal: {body}");
        assert!(body.contains("<html"), "{host}: die Seite kommt: {body}");
    }
    for host in ["127.0.0.1:8765", "localhost:8765", "[::1]:8765"] {
        let (status, _) = get_as(&server, host).await;
        assert_eq!(status, StatusCode::OK, "{host}: IP-Adresse und localhost gelten neben dem Namen weiter");
    }
    for host in [
        "andere.example.org",
        "x.baruch.example.org",
        "baruch.example.org.evil.example",
        "baruch.example.org evil.example",
        "baruch.example.org ",
        "*.example.org",
        "*",
        "baruch.example.org:",
        "baruch.example.org:8443:1",
        "baruch.example.org:hafen",
    ] {
        let (status, body) = get_as(&server, host).await;
        assert_eq!(
            status,
            StatusCode::MISDIRECTED_REQUEST,
            "„{host}“ ist nicht genau der erlaubte Name (anderer Name, Leerzeichen, Platzhalter, kein Port): 421: {body}"
        );
    }
}

#[tokio::test]
async fn several_names_are_allowed_side_by_side() {
    let server = open_allowing("baruch.example.org, schreibstube.example.net").await;
    for host in [NAME, "schreibstube.example.net:443"] {
        assert_eq!(get_as(&server, host).await.0, StatusCode::OK, "{host}: jeder genannte Name gilt");
    }
    assert_eq!(
        get_as(&server, "example.net").await.0,
        StatusCode::MISDIRECTED_REQUEST,
        "ein Teil eines Namens ist kein erlaubter Name"
    );
}

#[tokio::test]
async fn a_post_passes_the_origin_check_only_if_the_proxy_keeps_the_host() {
    let server = open_allowing(NAME).await;
    let (status, body) = rpc_as(&server, NAME, "https://baruch.example.org").await;
    assert_eq!(
        status,
        StatusCode::OK,
        "Origin https://Name und Host Name: dieselbe Herkunft, same_origin besteht: {body}"
    );
    assert!(body.contains("\"result\""), "der RPC-Aufruf wurde ausgeführt: {body}");
    let (status, body) = rpc_as(&server, "baruch.example.org:8443", "https://baruch.example.org:8443").await;
    assert_eq!(status, StatusCode::OK, "auch mit Port, wenn er in Origin und Host gleich ist: {body}");

    // Der Proxy schreibt Host um (nginx ohne `proxy_set_header Host $host`): Origin passt nicht mehr zu Host.
    let (status, body) = rpc_as(&server, "127.0.0.1:8765", "https://baruch.example.org").await;
    assert_eq!(status, StatusCode::FORBIDDEN, "umgeschriebener Host: same_origin lehnt ab (403): {body}");
    assert!(body.contains("Origin passt nicht zu Host"), "die Meldung nennt den Grund: {body}");
    let (status, body) = rpc_as(&server, NAME, "https://baruch.example.org:8443").await;
    assert_eq!(status, StatusCode::FORBIDDEN, "Port nur in Origin (Proxy lässt ihn im Host weg): abgelehnt: {body}");
    let (status, body) = rpc_as(&server, NAME, "https://boese.example").await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "eine fremde Seite bleibt auch unter dem erlaubten Namen draußen: {body}"
    );
}

// ---- /mcp ------------------------------------------------------------------------------------------------

#[tokio::test]
async fn mcp_under_an_extra_name_needs_a_token_even_from_loopback() {
    let server = open_allowing(NAME).await;
    let made = baruch_server::mcp::tokens::call(&server.state, "mcp_token.create", json!({ "name": "Proxy-Probe" }))
        .await
        .expect("Token erzeugen");
    let secret = made["secret"].as_str().expect("Klartext einmal in der Antwort").to_owned();

    for host in [NAME, "baruch.example.org:8443"] {
        let (status, body) = mcp_ping(&server, Some(host), None).await;
        assert_eq!(
            status,
            StatusCode::UNAUTHORIZED,
            "{host}: über den Proxy ist die Gegenstelle Loopback, die Anfrage aber nie lokal – ohne Token 401"
        );
        assert!(!body.contains("\"result\""), "{host}: ohne Token keine Antwort des Werkzeugs");
    }
    let (status, body) = mcp_ping(&server, Some(NAME), Some(&secret)).await;
    assert_eq!(status, StatusCode::OK, "mit gültigem Token nimmt /mcp die Anfrage unter dem Namen an");
    assert!(body.contains("\"result\""), "ping beantwortet");
    assert!(!body.contains(&secret), "die Antwort enthält den Token nicht");
    let (status, _) = mcp_ping(&server, Some(NAME), Some("baruch_falsch")).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "ein falscher Token bleibt abgelehnt");

    let (status, body) = mcp_ping(&server, Some("127.0.0.1:8765"), None).await;
    assert_eq!(status, StatusCode::OK, "unter 127.0.0.1 von Loopback wie bisher ohne Token: {body}");
    assert!(body.contains("\"result\""), "ping beantwortet: {body}");

    let (status, _) = mcp_ping(&server, None, None).await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "sind Namen erlaubt, steht ein Proxy davor: eine Anfrage ohne Host ist nicht als lokal belegt und braucht den Token"
    );
}

#[tokio::test]
async fn without_allow_host_mcp_from_loopback_stays_open_without_token() {
    let server = open_allowing("").await;
    let (status, body) = mcp_ping(&server, Some("127.0.0.1:8765"), None).await;
    assert_eq!(status, StatusCode::OK, "ohne --allow-host ändert sich nichts: Loopback ohne Token: {body}");
    let (status, body) = mcp_ping(&server, None, None).await;
    assert_eq!(status, StatusCode::OK, "auch ohne Host-Kopfzeile wie vor Runde 24 (Programme ohne Browser): {body}");
    let (status, _) = mcp_ping(&server, Some(NAME), None).await;
    assert_eq!(status, StatusCode::MISDIRECTED_REQUEST, "ein Name erreicht /mcp ohne --allow-host gar nicht (421)");
}

#[tokio::test]
async fn the_mcp_page_shows_the_command_with_token_under_an_extra_name() {
    let server = open_allowing(NAME).await;
    let request = Request::get("/einstellungen/mcp?fragment=1").header(header::HOST, NAME).body(Body::empty()).unwrap();
    let (status, _, html) = server.send(request).await;
    assert_eq!(status, StatusCode::OK, "die Seite kommt unter dem erlaubten Namen");
    assert!(html.contains("data-mcp-proxied"), "Hinweis auf den Betrieb über den Proxy: {html}");
    let address = esc("https://baruch.example.org/mcp");
    assert!(
        html.contains(&format!("value=\"{address}\"")),
        "Adresse zum Kopieren mit https (TLS endet im Proxy) und dem Namen: {html}"
    );
    assert!(
        html.contains(&format!(
            "claude mcp add -s local --transport http baruch {address} --header \"Authorization: Bearer &lt;Token&gt;\""
        )),
        "der Befehl nennt Adresse und Token-Kopfzeile: {html}"
    );
    assert!(
        html.contains("<strong>nur mit Token</strong>"),
        "sagt ehrlich, dass es hier ohne Token nicht geht: {html}"
    );
    assert!(
        !html.contains("Nur über Loopback (127.0.0.1 bzw. ::1) ohne Token erreichbar."),
        "der Satz für den lokalen Zugang stünde hier falsch: {html}"
    );
    assert!(!html.contains("http:&#x2f;&#x2f;baruch.example.org"), "nie die unverschlüsselte Adresse: {html}");

    let request = Request::get("/einstellungen/mcp?fragment=1")
        .header(header::HOST, "127.0.0.1:8765")
        .body(Body::empty())
        .unwrap();
    let (_, _, local) = server.send(request).await;
    assert!(!local.contains("data-mcp-proxied"), "unter der IP-Adresse bleibt die Seite wie bisher: {local}");
    assert!(
        local.contains(&format!("baruch {}</code>", esc("http://127.0.0.1:8765/mcp"))),
        "lokal der Befehl ohne Token: {local}"
    );
}

// ---- Kommandozeile ---------------------------------------------------------------------------------------

#[test]
fn parse_refuses_invalid_names_with_a_message() {
    for (list, named) in [
        ("*.example.org", "*.example.org"),
        ("baruch.example.org:8443", "baruch.example.org:8443"),
        ("baruch.example.org, 192.0.2.7", "192.0.2.7"),
        ("baruch_test.example", "baruch_test.example"),
    ] {
        let error = AllowedHosts::parse(list).expect_err(&format!("{list} ist ungültig"));
        assert!(
            error.contains(&format!("„{named}“")) && error.contains("--allow-host"),
            "die Meldung nennt den ungültigen Namen und die Angabe: {error}"
        );
    }
    let hosts = AllowedHosts::parse("Baruch.Example.org schreibstube.example.net").expect("zwei gültige Namen");
    assert_eq!(
        hosts.names(),
        ["baruch.example.org", "schreibstube.example.net"],
        "klein, in der Reihenfolge der Angabe"
    );
}

#[test]
fn the_program_refuses_an_invalid_allow_host_before_it_starts() {
    let data = tempfile::tempdir().unwrap();
    // Erst der Befehl samt `--data`, dann die geprüften Angaben (so steht `--allow-host` auch am Ende).
    let run = |command: &[&str], args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_baruch-server"))
            .args(command)
            .arg("--data")
            .arg(data.path())
            .args(args)
            .env_remove("BARUCH_ALLOWED_HOSTS")
            .env_remove("BARUCH_LISTEN")
            .output()
            .expect("baruch-server startet")
    };
    let out = run(&["serve"], &["--listen", "127.0.0.1:0", "--secrets", "memory", "--allow-host", "*.example.org"]);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(
        out.status.code(),
        Some(2),
        "ungültiger Name: Exit 2 (Aufruf falsch), der Server startet nicht: {stderr}"
    );
    assert!(stderr.contains("„*.example.org“") && stderr.contains("Platzhalter"), "nennt Namen und Grund: {stderr}");
    assert!(stderr.contains("--allow-host zusätzlich erlaubter Rechnername"), "die Hilfe erklärt die Angabe: {stderr}");
    assert!(!data.path().join("baruch.sqlite").exists(), "vor dem Fehler wurde kein Datenordner angelegt");

    let out = run(&["serve"], &["--allow-host"]);
    assert_eq!(out.status.code(), Some(2), "--allow-host ohne Namen: Exit 2");
    assert!(String::from_utf8_lossy(&out.stderr).contains("--allow-host braucht einen Rechnernamen"), "mit Hinweis");

    let out = run(&["export", "1"], &["--allow-host", NAME]);
    assert_eq!(out.status.code(), Some(2), "--allow-host gilt nur für serve");
    assert!(String::from_utf8_lossy(&out.stderr).contains("unbekannte Angabe: --allow-host"), "ehrlich abgelehnt");
}

#[test]
fn the_program_prints_its_version_and_starts_nothing() {
    // Nachtrag PM-Abschluss R24 (Gutachten): `--version`/`-V` war nur von Hand geprüft.
    let home = tempfile::tempdir().unwrap();
    for flag in ["--version", "-V"] {
        let out = Command::new(env!("CARGO_BIN_EXE_baruch-server"))
            .arg(flag)
            .env("HOME", home.path())
            .env("XDG_DATA_HOME", home.path().join("data"))
            .env("XDG_CONFIG_HOME", home.path().join("config"))
            .env_remove("BARUCH_ALLOWED_HOSTS")
            .env_remove("BARUCH_LISTEN")
            .output()
            .expect("baruch-server startet");
        assert_eq!(out.status.code(), Some(0), "{flag}: Exit 0, es ist eine Auskunft und kein Fehler");
        assert_eq!(
            String::from_utf8_lossy(&out.stdout),
            format!("baruch-server {}\n", env!("CARGO_PKG_VERSION")),
            "{flag}: genau eine Zeile mit Name und Version aus dem Manifest (Pakete und README.Debian nennen sie so)"
        );
        assert!(out.stderr.is_empty(), "{flag}: keine Hilfe und keine Warnung daneben");
    }
    assert_eq!(
        std::fs::read_dir(home.path()).unwrap().count(),
        0,
        "die Auskunft legt keinen Datenordner an und startet keinen Server"
    );
}

#[test]
fn the_environment_variable_is_checked_like_the_argument() {
    let data = tempfile::tempdir().unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_baruch-server"))
        .args(["serve", "--listen", "127.0.0.1:0", "--secrets", "memory", "--data"])
        .arg(data.path())
        .env("BARUCH_ALLOWED_HOSTS", "baruch.example.org,baruch.example.org:8443")
        .output()
        .expect("baruch-server startet");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(2), "BARUCH_ALLOWED_HOSTS mit Port: Exit 2 wie bei --allow-host: {stderr}");
    assert!(
        stderr.contains("„baruch.example.org:8443“") && stderr.contains("ohne Port"),
        "nennt Namen und Grund: {stderr}"
    );
    assert!(!data.path().join("baruch.sqlite").exists(), "vor dem Fehler wurde kein Datenordner angelegt");
}

// ---- MultiGPT-Vorgabe ------------------------------------------------------------------------------------

#[tokio::test]
async fn a_new_database_has_no_multigpt_address_and_shows_only_a_placeholder() {
    let server = open_allowing("").await;
    let row = ok(&server, "connection.get", json!({ "kind": "multigpt" })).await;
    assert_eq!(row["config"], json!({ "url": "" }), "neue Datenbank: keine Adresse für MultiGPT: {row}");
    assert!(row["updated"].is_null(), "die Vorgabe ist keine gespeicherte Zeile: {row}");

    let (status, html) = server.get("/einstellungen/verbindungen?fragment=1").await;
    assert_eq!(status, StatusCode::OK, "Seite Verbindungen");
    let start = html.find("id=\"multigpt\"").expect("Abschnitt MultiGPT");
    let part = &html[start..];
    let part = &part[..part.find("id=\"pgvector\"").expect("danach pgvector")];
    assert!(part.contains("name=\"url\" value=\"\""), "das Adressfeld ist leer: {part}");
    assert!(
        part.contains("placeholder=\"https://multigpt.example/mcp/\""),
        "als Platzhalter eine Beispieladresse (wird nie gespeichert): {part}"
    );
    assert!(!html.contains("intranet"), "keine Intranet-Adresse mehr auf der Seite: {html}");
    let eliezer = &html[html.find("id=\"eliezer\"").expect("Abschnitt Eliezer")..start];
    assert!(!eliezer.contains("multigpt.example"), "der Platzhalter steht nur bei MultiGPT: {eliezer}");
}

#[tokio::test]
async fn a_saved_multigpt_address_survives_reopening_unchanged() {
    let data = tempfile::tempdir().unwrap();
    let saved = json!({ "url": "http://127.0.0.1:9/mcp/" });
    let (updated, target) = {
        let server = open_in(data, "").await;
        ok(&server, "connection.set", json!({ "kind": "multigpt", "config": saved })).await;
        let row = server.state.store.connection_get(ConnectionKind::Multigpt).await.unwrap();
        assert_eq!(row.config, saved, "gespeichert wie eingegeben");
        let common::Server { data, state, app } = server;
        drop(app);
        drop(state);
        (row.updated, data)
    };
    let server = open_in(target, "").await;
    let row = server.state.store.connection_get(ConnectionKind::Multigpt).await.unwrap();
    assert_eq!(row.config, saved, "nach dem erneuten Öffnen dieselbe Adresse – die neue Vorgabe überschreibt nichts");
    assert_eq!(row.updated, updated, "die Zeile wurde beim Öffnen nicht angefasst (gleicher Zeitpunkt)");
    let (_, html) = server.get("/einstellungen/verbindungen?fragment=1").await;
    assert!(
        html.contains(&format!("name=\"url\" value=\"{}\"", esc("http://127.0.0.1:9/mcp/"))),
        "das Formular zeigt die gespeicherte Adresse: {html}"
    );
}

#[tokio::test]
async fn without_an_address_multigpt_is_not_set_up_and_nothing_is_asked() {
    let server = open_allowing("").await;
    let refused = open_way(&server.state).await.err().expect("ohne Adresse gibt es keinen Suchweg");
    assert_eq!(refused.code(), "not_configured", "nicht eingerichtet (Weg aus), kein Ausfall: {refused}");
    assert!(refused.is_off(), "der Weg gilt als aus, nicht als gescheitert: {refused}");
    assert!(
        refused.message().contains("Keine Adresse für MultiGPT eingetragen"),
        "die Meldung nennt den Grund und den Ort der Einstellung: {refused}"
    );

    // Auch wenn das Werk den Weg MultiGPT gewählt hat: Die Suche meldet ihn als „aus“ und fragt niemanden.
    let work = server.state.default_work().await.unwrap();
    ok(&server, "connection.rag_set", json!({ "work": work, "ways": ["lancedb", "multigpt"] })).await;
    let found =
        ok(&server, "rag.search", json!({ "work": work, "query": "Leuchtfeuer am Deich", "ways": ["multigpt"] })).await;
    let way = found["ways"]
        .as_array()
        .and_then(|ways| ways.iter().find(|w| w["way"] == "multigpt"))
        .unwrap_or_else(|| panic!("Bericht für MultiGPT fehlt: {found}"));
    assert_eq!(way["state"], "off", "MultiGPT ohne Adresse: Weg aus, nicht ausgefallen: {way}");
    assert_eq!(way["hits"], 0, "keine Treffer von einem Weg, der nicht gefragt wurde: {way}");

    let checked = ok(&server, "connection.check", json!({ "kind": "multigpt" })).await;
    assert_eq!(checked["ok"], false, "„Verbindung prüfen“ ohne Adresse gelingt nicht: {checked}");
    let message = checked["message"].as_str().unwrap_or_default();
    assert!(!message.contains("intranet"), "die Prüfung kennt keine alte Vorgabe-Adresse mehr: {checked}");
}
