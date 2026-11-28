// SPDX-License-Identifier: AGPL-3.0-or-later
//! Round 9, task „connections“: settings pages „Verbindungen“ and „Quellen“ (directly through
//! `render_connections`/`render_sources`, without the frame of the settings), their forms, RPC
//! `connection.*`/`sources.*`, „Verbindung prüfen“ against own test servers on 127.0.0.1:0 and the MCP tool
//! `sources_list`.
//!
//! Secrets: every test takes `MemorySecrets` (through `AppState::open_with_secrets` or `AppState::open`); the real
//! keyring (`KeyringSecrets`) occurs in no test. A unique test secret must not appear in any page, answer, message,
//! redirect or check result.
//!
//! pgvector: error paths (closed port, rejected password through a mocked peer) always run; with
//! `/usr/lib/postgresql/*/bin` additionally against an own throwaway cluster under `CARGO_TARGET_TMPDIR`
//! (scram-sha-256, only 127.0.0.1, without Unix socket), stopped with `pg_ctl` of the own cluster.

use crate::common;

use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::Router;
use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{HeaderMap, Request, StatusCode, header};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use baruch_server::AppState;
use baruch_server::connections::{MemorySecrets, render_connections, render_sources};
use baruch_store::RagWay;
use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

/// Unique test secret; it must not appear anywhere.
const SECRET: &str = "Zz9-geheim-7f3c1e-Test-Wert";

async fn open_with(secrets: MemorySecrets) -> common::Server {
    let data = tempfile::tempdir().unwrap();
    let state = AppState::open_with_secrets(data.path(), Arc::new(secrets)).await.expect("Datenordner öffnen");
    let state = Arc::new(state);
    common::Server { app: common::router(state.clone()), state, data }
}

/// This is how minijinja escapes text in HTML (also `/`).
fn esc(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
        .replace('/', "&#x2f;")
}

fn query(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs.iter().map(|(k, v)| ((*k).to_owned(), (*v).to_owned())).collect()
}

async fn page(server: &common::Server, work: Option<i64>, pairs: &[(&str, &str)]) -> String {
    let q = query(pairs);
    render_connections(&server.state, work, &q).await.expect("Seite Verbindungen")
}

async fn sources_page(server: &common::Server, pairs: &[(&str, &str)]) -> String {
    let q = query(pairs);
    render_sources(&server.state, None, &q).await.expect("Seite Quellen")
}

/// Successful RPC call.
async fn ok(server: &common::Server, method: &str, params: Value) -> Value {
    let answer = server.rpc(method, params).await;
    assert!(answer["error"].is_null(), "{method} sollte gelingen: {answer}");
    answer["result"].clone()
}

/// RPC error with code; returns the message.
async fn fails(server: &common::Server, method: &str, params: Value, code: i64) -> String {
    let answer = server.rpc(method, params).await;
    assert_eq!(answer["error"]["code"], code, "{method} muss mit {code} scheitern: {answer}");
    answer["error"]["message"].as_str().unwrap().to_owned()
}

/// Excerpt of a page: the connection `kind` (up to the next one).
fn section<'a>(html: &'a str, kind: &str) -> &'a str {
    let start = html.find(&format!("id=\"{kind}\"")).unwrap_or_else(|| panic!("Abschnitt {kind} fehlt: {html}"));
    let rest = &html[start..];
    let end = rest[1..].find("<section class=\"connection\"").map_or(rest.len(), |i| i + 1);
    &rest[..end]
}

/// Send a form and return the location of the redirect (always 303).
async fn form(server: &common::Server, uri: &str, body: &str) -> String {
    let (status, location) = server.post_form(uri, body).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "{uri} antwortet immer mit 303 ({body})");
    location.expect("303 mit Location")
}

fn form_escape(text: &str) -> String {
    text.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// Test server on 127.0.0.1:0; returns the address `http://127.0.0.1:PORT`.
async fn serve(app: Router) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    format!("http://{address}")
}

fn bearer(headers: &HeaderMap) -> String {
    headers.get(header::AUTHORIZATION).and_then(|v| v.to_str().ok()).unwrap_or("").to_owned()
}

/// Eliezer as a mock: `/v1/models` only with the right bearer. With a wrong key the answer mirrors the key back, and
/// one model carries the secret in its name – Baruch must not pass on either.
async fn fake_eliezer() -> String {
    let app = Router::new().route(
        "/v1/models",
        get(|headers: HeaderMap| async move {
            let presented = bearer(&headers);
            if presented != format!("Bearer {SECRET}") {
                return (StatusCode::UNAUTHORIZED, format!("falscher Schlüssel: {presented}")).into_response();
            }
            axum::Json(json!({ "object": "list", "data": [
                { "id": "probe-modell-1" }, { "id": "probe-modell-2" }, { "id": format!("spiegel-{SECRET}") } ] }))
            .into_response()
        }),
    );
    serve(app).await
}

/// MultiGPT (MCP) as a mock, answers as `application/json`: bearer and `Mcp-Session-Id` are checked; the methods in
/// their order end up in `seen`.
async fn fake_mcp_json(seen: Arc<Mutex<Vec<String>>>) -> String {
    let app = Router::new().route(
        "/mcp/",
        post(move |headers: HeaderMap, body: String| {
            let seen = seen.clone();
            async move {
                if bearer(&headers) != format!("Bearer {SECRET}") {
                    return StatusCode::UNAUTHORIZED.into_response();
                }
                let message: Value = serde_json::from_str(&body).unwrap();
                let method = message["method"].as_str().unwrap_or("").to_owned();
                let session = headers.get("mcp-session-id").and_then(|v| v.to_str().ok()).unwrap_or("").to_owned();
                let version =
                    headers.get("mcp-protocol-version").and_then(|v| v.to_str().ok()).unwrap_or("").to_owned();
                seen.lock().unwrap().push(format!("{method} [{session}] [{version}]"));
                match method.as_str() {
                    "initialize" => {
                        assert_eq!(message["params"]["clientInfo"]["name"], "baruch", "Baruch nennt sich: {message}");
                        assert_eq!(message["params"]["protocolVersion"], "2025-06-18", "Protokollversion: {message}");
                        let answer = json!({ "jsonrpc": "2.0", "id": message["id"], "result": {
                            "protocolVersion": "2025-06-18", "capabilities": { "tools": {} },
                            "serverInfo": { "name": "multigpt-probe", "version": "1.2.3" } } });
                        ([("mcp-session-id", "sitzung-json")], axum::Json(answer)).into_response()
                    }
                    _ if session != "sitzung-json" => StatusCode::BAD_REQUEST.into_response(),
                    "notifications/initialized" => StatusCode::ACCEPTED.into_response(),
                    "tools/list" => axum::Json(json!({ "jsonrpc": "2.0", "id": message["id"], "result": {
                        "tools": [ { "name": "suche" }, { "name": "sammlungen" } ] } }))
                    .into_response(),
                    _ => StatusCode::BAD_REQUEST.into_response(),
                }
            }
        }),
    );
    format!("{}/mcp/", serve(app).await)
}

/// Read one HTTP request from a raw socket: (head, body). `None` at the end of the connection.
async fn read_request(socket: &mut tokio::net::TcpStream) -> Option<(String, String)> {
    let mut buffer = Vec::new();
    let mut chunk = [0u8; 4096];
    loop {
        if let Some(end) = buffer.windows(4).position(|w| w == b"\r\n\r\n") {
            let head = String::from_utf8_lossy(&buffer[..end]).to_string();
            let length = head
                .lines()
                .find_map(|l| {
                    l.to_ascii_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse::<usize>().unwrap())
                })
                .unwrap_or(0);
            while buffer.len() < end + 4 + length {
                let n = socket.read(&mut chunk).await.ok()?;
                if n == 0 {
                    return None;
                }
                buffer.extend_from_slice(&chunk[..n]);
            }
            let body = String::from_utf8_lossy(&buffer[end + 4..end + 4 + length]).to_string();
            return Some((head, body));
        }
        let n = socket.read(&mut chunk).await.ok()?;
        if n == 0 {
            return None;
        }
        buffer.extend_from_slice(&chunk[..n]);
    }
}

/// MultiGPT as `text/event-stream`, raw over TCP: the stream stays open after the answer (as with some MCP servers);
/// Baruch has to read the answer nevertheless, without waiting for the end.
async fn fake_mcp_sse(seen: Arc<Mutex<Vec<String>>>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move {
        loop {
            let (mut socket, _) = listener.accept().await.unwrap();
            let seen = seen.clone();
            tokio::spawn(async move {
                while let Some((head, body)) = read_request(&mut socket).await {
                    let message: Value = serde_json::from_str(&body).unwrap_or(Value::Null);
                    let method = message["method"].as_str().unwrap_or("").to_owned();
                    let lower = head.to_ascii_lowercase();
                    let session = lower.contains("mcp-session-id: sitzung-sse");
                    let authorized = head.contains(&format!("Bearer {SECRET}"));
                    seen.lock().unwrap().push(format!("{method} session={session}"));
                    if !authorized {
                        let _ = socket.write_all(b"HTTP/1.1 401 Unauthorized\r\ncontent-length: 0\r\n\r\n").await;
                        continue;
                    }
                    let event = |result: Value| {
                        let answer = json!({ "jsonrpc": "2.0", "id": message["id"], "result": result });
                        format!(
                            "event: message\ndata: {{\"jsonrpc\":\"2.0\",\"method\":\"notifications/message\",\"params\":{{}}}}\n\n\
                             event: message\ndata: {answer}\n\n"
                        )
                    };
                    let head_sse = |extra: &str| {
                        format!(
                            "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\ncache-control: no-cache\r\n{extra}\r\n"
                        )
                    };
                    match method.as_str() {
                        "initialize" => {
                            let payload = event(json!({ "protocolVersion": "2025-06-18", "capabilities": {},
                                "serverInfo": { "name": "multigpt-sse", "version": "0.9" } }));
                            let response = format!("{}{payload}", head_sse("mcp-session-id: sitzung-sse\r\n"));
                            let _ = socket.write_all(response.as_bytes()).await;
                            // Leave the stream open (no length, no end).
                            tokio::time::sleep(Duration::from_secs(60)).await;
                            return;
                        }
                        "notifications/initialized" if session => {
                            let _ = socket.write_all(b"HTTP/1.1 202 Accepted\r\ncontent-length: 0\r\n\r\n").await;
                        }
                        "tools/list" if session => {
                            let payload = event(json!({ "tools": [ { "name": "rag_suche" } ] }));
                            let _ = socket.write_all(format!("{}{payload}", head_sse("")).as_bytes()).await;
                            tokio::time::sleep(Duration::from_secs(60)).await;
                            return;
                        }
                        _ => {
                            let _ = socket.write_all(b"HTTP/1.1 400 Bad Request\r\ncontent-length: 0\r\n\r\n").await;
                        }
                    }
                }
            });
        }
    });
    format!("http://{address}/mcp/")
}

/// Accepts connections and never answers.
async fn silent_server() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let mut held = Vec::new();
        loop {
            let (socket, _) = listener.accept().await.unwrap();
            held.push(socket);
        }
    });
    format!("http://{address}")
}

/// A free port on which nobody listens.
async fn closed_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    port
}

/// Everything that a human or a program gets to see from Baruch must not contain the
/// secret.
/// Enter the secret anew for the present target – like the human after a change of address (the
/// one stored before applies only to the old target and is otherwise held back).
async fn confirm_secret(server: &common::Server, kind: &str) {
    ok(server, "connection.set_secret", json!({ "kind": kind, "secret": SECRET })).await;
}

fn assert_clean(what: &str, text: &str) {
    assert!(!text.contains(SECRET), "{what} enthält das Geheimnis: {text}");
}

// ---- Pages -----------------------------------------------------------------------------------------

#[tokio::test]
async fn the_connections_page_shows_defaults_secret_states_and_honest_stubs() {
    let server = common::open().await;
    let work = server.state.default_work().await.unwrap();
    let html = page(&server, Some(work), &[]).await;
    assert!(
        html.starts_with("<section class=\"settings-page connections-page\" data-settings-page-body=\"verbindungen\">"),
        "nur der Seiteninhalt für den Rahmen, als section.settings-page[data-settings-page-body]: {html}"
    );
    assert!(html.trim_end().ends_with("</section>") && !html.contains("<html"), "kein ganzes Dokument: {html}");
    for value in ["http://127.0.0.1:1234", "lancedb", "nomic-embed-text"] {
        assert!(html.contains(&format!("value=\"{}\"", esc(value))), "Vorgabe {value} im Formular: {html}");
    }
    // Round 24: MultiGPT has no address as a default; the field is empty and only shows an example address.
    let multigpt = section(&html, "multigpt");
    assert!(multigpt.contains("name=\"url\" value=\"\""), "MultiGPT ohne Vorgabe-Adresse (leeres Feld): {multigpt}");
    assert!(
        multigpt.contains("placeholder=\"https://multigpt.example/mcp/\""),
        "Beispieladresse nur als Platzhalter: {multigpt}"
    );
    assert!(
        !section(&html, "eliezer").contains("multigpt.example"),
        "der Platzhalter gehört nur zum Adressfeld von MultiGPT"
    );
    let pg = section(&html, "pgvector");
    assert!(pg.contains("name=\"port\" value=\"5432\""), "Port-Vorgabe 5432: {pg}");
    assert!(pg.contains("<option value=\"prefer\" selected>"), "TLS-Vorgabe prefer: {pg}");
    for kind in ["eliezer", "multigpt", "pgvector"] {
        let part = section(&html, kind);
        assert!(part.contains("data-secret=\"fehlt\""), "{kind}: ohne Geheimnis „fehlt“: {part}");
        assert!(
            part.contains(&format!("<code>baruch/{kind}</code>")),
            "{kind}: Konto im Schlüsselbund genannt: {part}"
        );
        assert!(part.contains("Verbindung prüfen") && !part.contains("disabled title"), "{kind} lässt sich prüfen");
        if kind == "eliezer" {
            // Round 10 (backend): the personas use Eliezer; the usage STUB has been dropped.
            assert!(
                part.contains("data-usage") && part.contains("Personas nutzen Eliezer") && !part.contains("data-stub"),
                "eliezer: Nutzung ehrlich als umgesetzt benannt, kein STUB-Hinweis mehr: {part}"
            );
        } else {
            // Round 12: the usage is implemented for all ways (usage texts according to contract §5).
            let usage = baruch_server::connections::usage_stub(
                baruch_store::ConnectionKind::ALL.into_iter().find(|k| k.as_str() == kind).unwrap(),
            );
            assert!(
                part.contains("data-usage") && part.contains(&esc(usage)),
                "{kind}: Nutzung ehrlich als umgesetzt benannt: {part}"
            );
        }
    }
    let lance = section(&html, "lancedb");
    assert!(
        !lance.contains("name=\"secret\"") && !lance.contains("data-secret"),
        "LanceDB hat kein Geheimnis: {lance}"
    );
    assert!(
        lance.contains("value=\"connection.check\"") && !lance.contains("disabled"),
        "Runde 12: Prüfen für LanceDB ist freigegeben (das Ergebnis liefert der Integrator über crate::rag): {lance}"
    );
    assert!(html.matches("<span class=\"badge\">Vorgabe</span>").count() == 4, "alle vier noch Vorgabe: {html}");
    let rag = section(&html, "rag");
    assert!(
        rag.contains("name=\"way_lancedb\" value=\"1\" checked")
            && !rag.contains("name=\"way_pgvector\" value=\"1\" checked"),
        "RAG-Vorgabe nur LanceDB: {rag}"
    );
    assert!(
        rag.contains("(Vorgabe)") && rag.contains(&format!("name=\"work\" value=\"{work}\"")),
        "Werk und Vorgabe: {rag}"
    );
    let none = page(&server, None, &[]).await;
    assert!(section(&none, "rag").contains("Kein Werk gewählt"), "ohne Werk keine RAG-Wege: {none}");
}

#[tokio::test]
async fn a_stored_secret_shows_as_deposited_never_as_text() {
    let server = open_with(MemorySecrets::with([("eliezer", SECRET)])).await;
    let html = page(&server, None, &[]).await;
    assert!(section(&html, "eliezer").contains("data-secret=\"hinterlegt\""), "Schlüssel hinterlegt: {html}");
    assert!(section(&html, "eliezer").contains("Geheimnis entfernen"), "Entfernen nur bei hinterlegtem Geheimnis");
    assert!(section(&html, "multigpt").contains("data-secret=\"fehlt\""), "anderes Konto fehlt: {html}");
    assert!(!section(&html, "multigpt").contains("Geheimnis entfernen"), "nichts zu entfernen");
    assert_clean("Seite Verbindungen", &html);
}

#[tokio::test]
async fn ok_and_error_codes_become_german_text_and_unknown_codes_echo_nothing() {
    let server = common::open().await;
    let html = page(&server, None, &[("ok", "gespeichert")]).await;
    assert!(html.contains("role=\"status\">Die Einstellungen sind gespeichert.</p>"), "ok=gespeichert: {html}");
    let html = page(&server, None, &[("fehler", "url_schema"), ("kind", "multigpt")]).await;
    let text = "Die Adresse muss mit http:// oder https:// beginnen; nichts geändert.";
    assert_eq!(html.matches(&esc(text)).count(), 2, "Fehler oben und bei der Verbindung: {html}");
    assert!(section(&html, "multigpt").contains(&esc(text)), "bei MultiGPT angezeigt");
    assert!(!section(&html, "eliezer").contains(&esc(text)), "nicht bei Eliezer");
    let html = page(&server, None, &[("fehler", "<script>x</script>"), ("ok", "<b>")]).await;
    assert!(
        html.contains("unbekannter Fehlercode") && !html.contains("<script>") && !html.contains("<b>"),
        "nichts gespiegelt: {html}"
    );
    for (code, expected) in [
        ("schluesselbund", "Der Schlüsselbund ist nicht erreichbar"),
        ("geheimnis_leer", "Das Geheimnis ist leer"),
        ("port", "ganze Zahl von 1 bis 65535"),
    ] {
        let html = page(&server, None, &[("fehler", code)]).await;
        assert!(html.contains(expected), "fehler={code} → „{expected}“: {html}");
    }
    let sources = sources_page(&server, &[("ok", "angelegt")]).await;
    assert!(sources.contains("Die Quelle ist angelegt."), "ok=angelegt: {sources}");
    let sources = sources_page(&server, &[("fehler", "adresse")]).await;
    assert!(sources.contains("Die Adresse muss mit http:"), "fehler=adresse: {sources}");
}

#[tokio::test]
async fn the_sources_page_lists_and_offers_every_form_without_javascript() {
    let server = common::open().await;
    let empty = sources_page(&server, &[]).await;
    assert!(
        empty.starts_with("<section class=\"settings-page sources-page\" data-settings-page-body=\"quellen\">"),
        "Seiteninhalt für den Rahmen: {empty}"
    );
    assert!(empty.contains("Noch keine Quellen.") && empty.contains("value=\"sources.create\""), "leer, Anlegen da");
    // Round 13 (websuche): the web search of @belege uses the list; the STUB hint has been dropped.
    let usage = esc("Die Websuche für @belege (Einstellungen → Assistenten, Vorgabe aus) fragt zuerst die ersten drei");
    assert!(
        empty.contains(&format!("<p class=\"muted tiny\" data-usage>{usage}"))
            && !empty.contains("data-stub")
            && !empty.contains("Noch nicht umgesetzt"),
        "Nutzung in der Websuche als data-usage, ohne STUB-Hinweis: {empty}"
    );
    ok(
        &server,
        "sources.create",
        json!({ "title": "Psalmen <online>", "url": "https://psalmen.example/?a=1&b=2", "note": "Zeile \"eins\"" }),
    )
    .await;
    ok(&server, "sources.create", json!({ "title": "Konkordanz", "url": "http://konkordanz.example/" })).await;
    let html = sources_page(&server, &[]).await;
    assert!(html.contains("Psalmen &lt;online&gt;") && !html.contains("<online>"), "Titel maskiert: {html}");
    assert!(
        html.contains(&format!("href=\"{}\"", esc("https://psalmen.example/?a=1&b=2"))),
        "Adresse maskiert als Link"
    );
    assert!(html.contains("rel=\"noopener noreferrer\""), "fremde Seiten ohne Zugriff auf Baruch");
    assert!(html.contains("Zeile &quot;eins&quot;"), "Notiz maskiert");
    for method in ["sources.update", "sources.move", "sources.delete"] {
        assert_eq!(html.matches(&format!("value=\"{method}\"")).count(), 2, "{method} je Quelle: {html}");
    }
    assert!(html.contains("value=\"up\"") && html.contains("value=\"down\""), "auf und ab");
    let first = &html[html.find("data-source=").unwrap()..];
    let first = &first[..first.find("</li>").unwrap()];
    assert!(
        first.contains("value=\"up\" aria-label=\"„Psalmen &lt;online&gt;“ nach oben\" disabled"),
        "erste nicht nach oben: {first}"
    );
    assert!(html.contains("name=\"confirm\" value=\"1\" required"), "Löschen mit Kästchen");
}

// ---- Forms ---------------------------------------------------------------------------------------

#[tokio::test]
async fn connection_forms_always_redirect_with_codes_only() {
    let server = common::open().await;
    let work = server.state.default_work().await.unwrap();
    let act = "/einstellungen/verbindungen/act";
    let location = form(
        &server,
        act,
        &format!("method=connection.set&kind=eliezer&work={work}&url=http%3A%2F%2F127.0.0.1%3A4321"),
    )
    .await;
    assert_eq!(location, format!("/einstellungen/verbindungen?ok=gespeichert&work={work}#eliezer"), "Erfolg mit Anker");
    let row = ok(&server, "connection.get", json!({ "kind": "eliezer" })).await;
    assert_eq!(row["config"]["url"], "http://127.0.0.1:4321", "gespeichert: {row}");

    let location =
        form(&server, act, &format!("method=connection.set&kind=eliezer&work={work}&url=ftp%3A%2F%2Fgeheim.example"))
            .await;
    assert_eq!(
        location,
        format!("/einstellungen/verbindungen?fehler=url_schema&kind=eliezer&work={work}"),
        "Fehlercode"
    );
    assert!(!location.contains("geheim.example"), "keine Eingabe in der Adresse");
    let location = form(&server, act, "method=connection.set&kind=pgvector&host=db.example&port=99999").await;
    assert_eq!(location, "/einstellungen/verbindungen?fehler=port&kind=pgvector", "Port außerhalb");
    let location = form(
        &server,
        act,
        "method=connection.set&kind=pgvector&host=db.example&port=6543&tls=require&database=rag&user=baruch",
    )
    .await;
    assert_eq!(location, "/einstellungen/verbindungen?ok=gespeichert#pgvector", "pgvector gespeichert");
    let row = ok(&server, "connection.get", json!({ "kind": "pgvector" })).await;
    assert_eq!(
        row["config"],
        json!({ "host": "db.example", "port": 6543, "database": "rag", "user": "baruch", "tls": "require" }),
        "Felder aus dem Formular, Port als Zahl"
    );
    let location = form(&server, act, "method=connection.set&kind=ollama&url=http%3A%2F%2Fx").await;
    assert_eq!(location, "/einstellungen/verbindungen?fehler=kind", "unbekannte Art ohne kind in der Adresse");
    let location = form(&server, act, "method=connection.drop&kind=eliezer").await;
    assert_eq!(location, "/einstellungen/verbindungen?fehler=methode&kind=eliezer", "unbekannte Aktion");
    let location = form(&server, act, "method=connection.check&kind=lancedb").await;
    // Since Round 12 LanceDB really checks (directory, index, embedding model); without an embedder that is a result of
    // the connection (ok: false), not an error of the action.
    assert_eq!(location, "/einstellungen/verbindungen?ok=geprueft#lancedb", "LanceDB-Prüfung ist ein Ergebnis");
}

#[tokio::test]
async fn secrets_from_the_form_go_to_the_keyring_and_never_into_the_address() {
    let server = common::open().await;
    let act = "/einstellungen/verbindungen/act";
    let body = format!(
        "method=connection.set&kind=multigpt&url={}&secret={}",
        form_escape("https://mcp.example/mcp/"),
        form_escape(SECRET)
    );
    let location = form(&server, act, &body).await;
    assert_eq!(
        location, "/einstellungen/verbindungen?ok=gespeichert_geheimnis#multigpt",
        "Einstellungen und Geheimnis"
    );
    assert_eq!(server.state.secrets.get("multigpt").unwrap().as_deref(), Some(SECRET), "im Schlüsselbund (Speicher)");
    let row = ok(&server, "connection.get", json!({ "kind": "multigpt" })).await;
    assert_eq!(row["secret"], "hinterlegt", "RPC nennt nur den Zustand: {row}");
    assert_clean("connection.get", &row.to_string());
    let stored: String = sqlx::query_scalar("SELECT config FROM connection WHERE kind = 'multigpt'")
        .fetch_one(server.state.store.pool())
        .await
        .unwrap();
    assert_clean("SQLite", &stored);

    let location = form(&server, act, "method=connection.delete_secret&kind=multigpt").await;
    assert_eq!(
        location, "/einstellungen/verbindungen?fehler=bestaetigen&kind=multigpt",
        "ohne Kästchen nichts entfernt"
    );
    assert!(server.state.secrets.get("multigpt").unwrap().is_some(), "noch da");
    let location = form(&server, act, "method=connection.delete_secret&kind=multigpt&confirm=1").await;
    assert_eq!(location, "/einstellungen/verbindungen?ok=geheimnis_entfernt#multigpt", "entfernt");
    assert!(server.state.secrets.get("multigpt").unwrap().is_none(), "Schlüsselbund leer");
    let location = form(&server, act, "method=connection.set_secret&kind=eliezer&secret=+++").await;
    assert_eq!(location, "/einstellungen/verbindungen?fehler=geheimnis_leer&kind=eliezer", "leeres Geheimnis");
    let location =
        form(&server, act, &format!("method=connection.set_secret&kind=lancedb&secret={}", form_escape(SECRET))).await;
    assert_eq!(location, "/einstellungen/verbindungen?fehler=kein_geheimnis&kind=lancedb", "LanceDB ohne Geheimnis");
    assert!(server.state.secrets.get("lancedb").unwrap().is_none(), "nichts für LanceDB hinterlegt");
}

#[tokio::test]
async fn checkboxes_set_the_rag_ways_of_the_work() {
    let server = common::open().await;
    let work = server.state.default_work().await.unwrap();
    let act = "/einstellungen/verbindungen/act";
    let location =
        form(&server, act, &format!("method=connection.rag_set&work={work}&way_pgvector=1&way_multigpt=1")).await;
    assert_eq!(location, format!("/einstellungen/verbindungen?ok=rag&work={work}#rag"), "gespeichert");
    assert_eq!(
        server.state.store.work_rag(work).await.unwrap(),
        (vec![RagWay::Pgvector, RagWay::Multigpt], false),
        "angehakte Wege gelten"
    );
    let html = page(&server, Some(work), &[]).await;
    let rag = section(&html, "rag");
    assert!(
        rag.contains("name=\"way_pgvector\" value=\"1\" checked")
            && !rag.contains("name=\"way_lancedb\" value=\"1\" checked"),
        "Seite zeigt die Wahl: {rag}"
    );
    form(&server, act, &format!("method=connection.rag_set&work={work}")).await;
    assert_eq!(server.state.store.work_rag(work).await.unwrap(), (vec![], false), "kein Haken = nur Volltext");
    let location = form(&server, act, "method=connection.rag_set&work=4711&way_lancedb=1").await;
    assert_eq!(location, "/einstellungen/verbindungen?fehler=werk&work=4711", "unbekanntes Werk");
    let location = form(&server, act, "method=connection.rag_set&way_lancedb=1").await;
    assert_eq!(location, "/einstellungen/verbindungen?fehler=werk", "ohne Werk keine Wege");
}

#[tokio::test]
async fn source_forms_create_edit_move_and_delete_with_codes_only() {
    let server = common::open().await;
    let act = "/einstellungen/quellen/act";
    let location = form(
        &server,
        act,
        &format!("method=sources.create&title=Atlas&url={}&work=1", form_escape("https://atlas.example/")),
    )
    .await;
    let atlas = ok(&server, "sources.list", json!({})).await["sources"][0]["id"].as_i64().unwrap();
    assert_eq!(
        location,
        format!("/einstellungen/quellen?ok=angelegt&work=1#quelle-{atlas}"),
        "Anker auf die neue Quelle"
    );
    form(&server, act, &format!("method=sources.create&title=Brevier&url={}", form_escape("https://brevier.example/")))
        .await;
    let location = form(&server, act, &format!("method=sources.move&id={atlas}&direction=down")).await;
    assert_eq!(location, format!("/einstellungen/quellen?ok=verschoben#quelle-{atlas}"), "verschoben");
    let titles: Vec<String> = ok(&server, "sources.list", json!({})).await["sources"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| format!("{}{}", s["position"], s["title"].as_str().unwrap()))
        .collect();
    assert_eq!(titles, ["1Brevier", "2Atlas"], "Atlas nach unten, Stellen lückenlos");
    let location = form(
        &server,
        act,
        &format!(
            "method=sources.update&id={atlas}&title=Weltatlas&url={}&note=neu",
            form_escape("https://atlas.example/welt")
        ),
    )
    .await;
    assert_eq!(location, format!("/einstellungen/quellen?ok=geaendert#quelle-{atlas}"), "geändert");
    let location =
        form(&server, act, &format!("method=sources.update&id={atlas}&url={}", form_escape("javascript:alert(1)")))
            .await;
    assert_eq!(location, "/einstellungen/quellen?fehler=adresse", "nur http(s), Eingabe nicht in der Adresse");
    let location = form(&server, act, "method=sources.create&title=&url=https%3A%2F%2Fx.example%2F").await;
    assert_eq!(location, "/einstellungen/quellen?fehler=titel", "Titel fehlt");
    let location = form(&server, act, &format!("method=sources.delete&id={atlas}")).await;
    assert_eq!(location, "/einstellungen/quellen?fehler=bestaetigen", "Löschen nur mit Kästchen");
    let location = form(&server, act, &format!("method=sources.delete&id={atlas}&confirm=1")).await;
    assert_eq!(location, "/einstellungen/quellen?ok=geloescht#quellen", "gelöscht");
    let location = form(&server, act, &format!("method=sources.delete&id={atlas}&confirm=1")).await;
    assert_eq!(location, "/einstellungen/quellen?fehler=quelle", "zweimal löschen: gibt es nicht mehr");
    let list = ok(&server, "sources.list", json!({})).await;
    assert_eq!(list["sources"].as_array().unwrap().len(), 1, "nur Brevier bleibt: {list}");
    assert_eq!(list["sources"][0]["position"], 1, "lückenlos ab 1");
}

#[tokio::test]
async fn the_forms_refuse_foreign_sites_and_the_generic_act_knows_none_of_these_methods() {
    let server = common::open().await;
    for uri in ["/einstellungen/verbindungen/act", "/einstellungen/quellen/act"] {
        let request = Request::post(uri)
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .header(header::HOST, "127.0.0.1:8765")
            .header(header::ORIGIN, "https://fremd.example")
            .body(Body::from("method=sources.create&title=X&url=https%3A%2F%2Fx.example%2F"))
            .unwrap();
        let (status, _, _) = server.send(request).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{uri} liegt hinter same_origin");
    }
    assert_eq!(ok(&server, "sources.list", json!({})).await["sources"], json!([]), "nichts angelegt");
    for method in ["connection.set_secret", "connection.check", "sources.create"] {
        let (status, _) = server.post_form("/act", &format!("method={method}")).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{method} gibt es über /act nicht (nur Mensch, eigene Formulare)");
    }
}

// ---- RPC -------------------------------------------------------------------------------------------

#[tokio::test]
async fn connection_rpc_lists_and_validates() {
    let server = common::open().await;
    let list = ok(&server, "connection.list", json!({})).await;
    let kinds: Vec<&str> =
        list["connections"].as_array().unwrap().iter().map(|c| c["kind"].as_str().unwrap()).collect();
    assert_eq!(kinds, ["eliezer", "multigpt", "pgvector", "lancedb"], "alle vier: {list}");
    let lance = &list["connections"][3];
    assert!(lance["secret"].is_null() && lance["check"].is_null(), "LanceDB: kein Geheimnis, nie geprüft: {lance}");
    assert_eq!(list["connections"][0]["secret"], "fehlt", "Eliezer ohne Schlüssel");
    assert_eq!(list["connections"][0]["title"], "Eliezer (Modell-Backend)", "mit Titel");

    let set =
        ok(&server, "connection.set", json!({ "kind": "multigpt", "config": { "url": "https://mcp.example/mcp/" } }))
            .await;
    assert_eq!(set["config"]["url"], "https://mcp.example/mcp/", "gespeichert: {set}");
    for (config, why) in [
        (json!({ "url": "https://ich:pw@mcp.example/" }), "Benutzerinfo in der Adresse"),
        (json!({ "url": "ftp://mcp.example/" }), "Schema nicht http/https"),
        (json!({ "url": "https://mcp.example/?token=abc" }), "Token in der Abfrage"),
        (json!({ "token": "abc" }), "Geheimnis-Schlüssel in config"),
        (json!({ "url": "https://mcp.example/", "password": "abc" }), "Passwort in config"),
        (json!({ "api_key": "abc" }), "Schlüssel in config"),
        (json!({ "endpoint": "x" }), "unbekannter Schlüssel"),
        (json!("https://mcp.example/"), "config ist kein Objekt"),
    ] {
        fails(&server, "connection.set", json!({ "kind": "multigpt", "config": config }), -32602).await;
        let now = ok(&server, "connection.get", json!({ "kind": "multigpt" })).await;
        assert_eq!(now["config"]["url"], "https://mcp.example/mcp/", "{why}: abgelehnt, nichts geändert");
    }
    for (config, why) in [
        (json!({ "port": 0 }), "Port 0"),
        (json!({ "port": "abc" }), "Port keine Zahl"),
        (json!({ "tls": "allow" }), "TLS-Modus unbekannt"),
        (json!({ "host": "db example" }), "Rechner mit Leerzeichen"),
        (json!({ "database": "a\nb" }), "Datenbank mehrzeilig"),
    ] {
        let message = fails(&server, "connection.set", json!({ "kind": "pgvector", "config": config }), -32602).await;
        assert!(!message.is_empty(), "{why}: Meldung");
    }
    fails(&server, "connection.set", json!({ "kind": "lancedb", "config": { "dir": "  " } }), -32602).await;
    fails(&server, "connection.get", json!({ "kind": "ollama" }), -32602).await;
    fails(&server, "connection.get", json!({}), -32602).await;
    fails(&server, "connection.set_secret", json!({ "kind": "eliezer", "secret": "   " }), -32602).await;
    fails(&server, "connection.set_secret", json!({ "kind": "lancedb", "secret": "x" }), -32602).await;
    let answer = ok(&server, "connection.set_secret", json!({ "kind": "eliezer", "secret": SECRET })).await;
    assert_eq!(answer, json!({ "kind": "eliezer", "secret": "hinterlegt" }), "nur der Zustand");
    fails(&server, "connection.delete_secret", json!({ "kind": "eliezer" }), -32602).await;
    fails(&server, "connection.delete_secret", json!({ "kind": "eliezer", "confirm": "ja" }), -32602).await;
    assert_eq!(ok(&server, "connection.get", json!({ "kind": "eliezer" })).await["secret"], "hinterlegt", "noch da");
    let answer = ok(&server, "connection.delete_secret", json!({ "kind": "eliezer", "confirm": true })).await;
    assert_eq!(answer, json!({ "kind": "eliezer", "secret": "fehlt" }), "entfernt");
    // Since Round 12 LanceDB really checks: without an embedder (tests attach none, never network) that is honestly a
    // failure with a reason, no longer a STUB.
    let check = ok(&server, "connection.check", json!({ "kind": "lancedb" })).await;
    assert_eq!(check["kind"], "lancedb", "Art der Prüfung: {check}");
    assert_eq!(check["ok"], false, "ohne Einbettungsmodell nicht nutzbar: {check}");
    assert!(check["details"]["embedder"].as_str().is_some_and(|e| e != "ok"), "Grund beim Einbetter: {check}");
    assert!(!check.to_string().contains("STUB"), "kein STUB mehr: {check}");
}

#[tokio::test]
async fn rag_ways_over_rpc() {
    let server = common::open().await;
    let work = server.state.default_work().await.unwrap();
    let got = ok(&server, "connection.rag_get", json!({ "work": work })).await;
    assert_eq!(got, json!({ "work": work, "ways": ["lancedb"], "default": true }), "Vorgabe");
    let set = ok(&server, "connection.rag_set", json!({ "work": work, "ways": ["multigpt", "lancedb"] })).await;
    assert_eq!(set, json!({ "work": work, "ways": ["lancedb", "multigpt"], "default": false }), "feste Reihenfolge");
    assert_eq!(
        ok(&server, "connection.rag_get", json!({})).await["ways"],
        json!(["lancedb", "multigpt"]),
        "Vorgabewerk"
    );
    fails(&server, "connection.rag_set", json!({ "work": work, "ways": ["volltext"] }), -32602).await;
    fails(&server, "connection.rag_set", json!({ "work": work, "ways": "lancedb" }), -32602).await;
    fails(&server, "connection.rag_set", json!({ "work": 4711, "ways": [] }), -32602).await;
    fails(&server, "connection.rag_get", json!({ "work": 4711 }), -32602).await;
    assert_eq!(
        ok(&server, "connection.rag_get", json!({ "work": work })).await["ways"],
        json!(["lancedb", "multigpt"]),
        "unverändert"
    );
}

#[tokio::test]
async fn sources_rpc_keeps_order_and_validates() {
    let server = common::open().await;
    let a = ok(&server, "sources.create", json!({ "title": "Alpha", "url": "https://alpha.example/" })).await;
    assert_eq!((a["position"].as_i64(), a["note"].as_str()), (Some(1), Some("")), "erste Quelle, leere Notiz: {a}");
    let b =
        ok(&server, "sources.create", json!({ "title": "Beta", "url": "https://beta.example/", "note": "gut" })).await;
    let c = ok(&server, "sources.create", json!({ "title": "Gamma", "url": "https://gamma.example/" })).await;
    let moved = ok(&server, "sources.move", json!({ "id": c["id"], "position": 1 })).await;
    let order: Vec<&str> = moved["sources"].as_array().unwrap().iter().map(|s| s["title"].as_str().unwrap()).collect();
    assert_eq!(order, ["Gamma", "Alpha", "Beta"], "nach vorn");
    let updated = ok(&server, "sources.update", json!({ "id": b["id"], "title": "Beta 2" })).await;
    assert_eq!((updated["title"].as_str(), updated["note"].as_str()), (Some("Beta 2"), Some("gut")), "nur der Titel");
    for (params, why) in [
        (json!({ "title": "", "url": "https://x.example/" }), "leerer Titel"),
        (json!({ "title": "X", "url": "ftp://x.example/" }), "kein http(s)"),
        (json!({ "title": "X", "url": "https://ich:pw@x.example/" }), "Benutzerinfo"),
        (json!({ "title": "X".repeat(201), "url": "https://x.example/" }), "Titel zu lang"),
        (json!({ "title": "X", "url": format!("https://x.example/{}", "a".repeat(2000)) }), "Adresse zu lang"),
        (json!({ "title": 5, "url": "https://x.example/" }), "Titel keine Zeichenkette"),
    ] {
        let message = fails(&server, "sources.create", params, -32602).await;
        assert!(!message.is_empty(), "{why}: Meldung");
    }
    fails(&server, "sources.update", json!({ "id": 9999, "title": "X" }), -32602).await;
    fails(&server, "sources.move", json!({ "id": a["id"] }), -32602).await;
    fails(&server, "sources.delete", json!({ "id": a["id"] }), -32602).await;
    let deleted = ok(&server, "sources.delete", json!({ "id": a["id"], "confirm": true })).await;
    assert_eq!(deleted["deleted"]["title"], "Alpha", "gelöscht: {deleted}");
    fails(&server, "sources.delete", json!({ "id": a["id"], "confirm": true }), -32602).await;
    let list = ok(&server, "sources.list", json!({})).await;
    let positions: Vec<i64> =
        list["sources"].as_array().unwrap().iter().map(|s| s["position"].as_i64().unwrap()).collect();
    assert_eq!(positions, [1, 2], "lückenlos nach Verschieben und Löschen: {list}");
}

// ---- Checking --------------------------------------------------------------------------------------

#[tokio::test]
async fn checking_eliezer_lists_models_with_the_bearer_and_stores_the_result() {
    let server = open_with(MemorySecrets::with([("eliezer", SECRET)])).await;
    let url = fake_eliezer().await;
    ok(&server, "connection.set", json!({ "kind": "eliezer", "config": { "url": url } })).await;
    confirm_secret(&server, "eliezer").await;
    let result = ok(&server, "connection.check", json!({ "kind": "eliezer" })).await;
    assert_eq!(result["ok"], true, "mit richtigem Schlüssel: {result}");
    assert_eq!(
        result["details"]["models"][0], "probe-modell-1",
        "Modellliste in details.models in der Reihenfolge der Gegenstelle (die Seite Verbindungen und der Abgleich \
         der Personas lesen sie dort): {result}"
    );
    assert_eq!(
        result["details"]["models"][2], "spiegel-***",
        "die Gegenstelle spiegelt das Geheimnis in einem Modellnamen; vor dem Speichern ersetzt scrub es durch *** \
         (kein Geheimnis in Prüfergebnissen, SKILLS.md): {result}"
    );
    assert!(result["at"].as_str().is_some_and(|at| at.ends_with('Z')), "Zeitpunkt: {result}");
    assert_clean("connection.check", &result.to_string());
    let row = ok(&server, "connection.get", json!({ "kind": "eliezer" })).await;
    assert_eq!(row["check"]["ok"], true, "Ergebnis in connection gespeichert: {row}");
    // Round 10 (backend): after that the message names the persona models that the mock does not know.
    let message = row["check"]["message"].as_str().unwrap();
    assert!(
        message.starts_with("Verbindung steht: 3 Modelle.") && message.contains("Unbekannt bei Eliezer"),
        "Meldung mit Zahl der Modelle und Abgleich der Personas: {message}"
    );
    let html = page(&server, None, &[("ok", "geprueft")]).await;
    let part = section(&html, "eliezer");
    assert!(part.contains("data-check=\"ok\"") && part.contains("probe-modell-2"), "auf der Seite sichtbar: {part}");
    assert_clean("Seite nach dem Prüfen", &html);
    let details: String = sqlx::query_scalar("SELECT check_details FROM connection WHERE kind = 'eliezer'")
        .fetch_one(server.state.store.pool())
        .await
        .unwrap();
    assert_clean("check_details", &details);

    // Wrong key: ok:false with a message, the answer of the peer (with the key) is not passed on.
    server.state.secrets.set("eliezer", "falsch-aber-lang").unwrap();
    let result = ok(&server, "connection.check", json!({ "kind": "eliezer" })).await;
    assert_eq!(result["ok"], false, "falscher Schlüssel ist ein Ergebnis, kein RPC-Fehler: {result}");
    let message = result["message"].as_str().unwrap();
    assert!(message.contains("HTTP 401") && message.contains("baruch/eliezer"), "Meldung: {message}");
    assert!(!result.to_string().contains("falsch-aber-lang"), "auch ein falscher Schlüssel bleibt geheim: {result}");
    let html = page(&server, None, &[]).await;
    assert!(section(&html, "eliezer").contains("data-check=\"failed\""), "Fehlschlag sichtbar");

    server.state.secrets.delete("eliezer").unwrap();
    let result = ok(&server, "connection.check", json!({ "kind": "eliezer" })).await;
    assert!(
        result["message"].as_str().unwrap().contains("kein Schlüssel hinterlegt"),
        "Hinweis ohne Schlüssel: {result}"
    );
}

#[tokio::test]
async fn checking_multigpt_speaks_mcp_as_json_and_as_event_stream() {
    let server = open_with(MemorySecrets::with([("multigpt", SECRET)])).await;
    let seen = Arc::new(Mutex::new(Vec::new()));
    let url = fake_mcp_json(seen.clone()).await;
    ok(&server, "connection.set", json!({ "kind": "multigpt", "config": { "url": url } })).await;
    confirm_secret(&server, "multigpt").await;
    let result = ok(&server, "connection.check", json!({ "kind": "multigpt" })).await;
    assert_eq!(result["ok"], true, "JSON-Antworten: {result}");
    assert_eq!(
        result["details"],
        json!({
            "server": { "name": "multigpt-probe", "version": "1.2.3" },
            "tools": ["suche", "sammlungen"],
            "pages": 1,
            "collections": []
        }),
        "Vertrag Runde 12 §5: details mit Seitenzahl von tools/list und Sammlungen (Nachbau ohne list_collections)"
    );
    assert_eq!(
        *seen.lock().unwrap(),
        [
            "initialize [] []",
            "notifications/initialized [sitzung-json] [2025-06-18]",
            "tools/list [sitzung-json] [2025-06-18]"
        ],
        "Reihenfolge und Mcp-Session-Id nach initialize"
    );
    assert_clean("MultiGPT JSON", &result.to_string());

    let seen = Arc::new(Mutex::new(Vec::new()));
    let url = fake_mcp_sse(seen.clone()).await;
    ok(&server, "connection.set", json!({ "kind": "multigpt", "config": { "url": url } })).await;
    confirm_secret(&server, "multigpt").await;
    let started = Instant::now();
    let result = ok(&server, "connection.check", json!({ "kind": "multigpt" })).await;
    assert_eq!(result["ok"], true, "text/event-stream: {result}");
    assert_eq!(
        result["details"],
        json!({ "server": { "name": "multigpt-sse", "version": "0.9" }, "tools": ["rag_suche"], "pages": 1, "collections": [] }),
        "Vertrag Runde 12 §5: details auch über text/event-stream mit Seiten und Sammlungen"
    );
    assert!(started.elapsed() < Duration::from_secs(8), "offener Strom: die Antwort reicht, kein Warten aufs Ende");
    assert_eq!(
        *seen.lock().unwrap(),
        [
            "initialize session=false",
            "notifications/initialized session=true",
            "tools/list session=true",
            " session=true"
        ],
        "Sitzung auch beim Ereignisstrom; zuletzt das DELETE (ohne Rumpf, also ohne Methode) mit Sitzung (Runde 19a)"
    );
    let html = page(&server, None, &[]).await;
    assert!(section(&html, "multigpt").contains("rag_suche"), "Werkzeuge auf der Seite: {html}");

    server.state.secrets.set("multigpt", "anderes-token").unwrap();
    let result = ok(&server, "connection.check", json!({ "kind": "multigpt" })).await;
    assert_eq!(result["ok"], false, "falsches Token: {result}");
    assert!(result["message"].as_str().unwrap().contains("HTTP 401"), "Meldung: {result}");
}

#[tokio::test]
async fn a_check_gives_up_after_ten_seconds_and_on_oversized_answers() {
    let server = common::open().await;
    let url = silent_server().await;
    ok(&server, "connection.set", json!({ "kind": "eliezer", "config": { "url": url } })).await;
    let started = Instant::now();
    let result = ok(&server, "connection.check", json!({ "kind": "eliezer" })).await;
    let took = started.elapsed();
    assert_eq!(result["ok"], false, "keine Antwort: {result}");
    assert!(result["message"].as_str().unwrap().contains("innerhalb von 10 s"), "Zeitlimit genannt: {result}");
    assert!(
        took >= Duration::from_secs(9) && took < Duration::from_secs(20),
        "Zeitlimit greift nach etwa 10 s: {took:?}"
    );

    let big = Router::new().route("/v1/models", get(|| async { "x".repeat(2 * 1024 * 1024) }));
    let url = serve(big).await;
    ok(&server, "connection.set", json!({ "kind": "eliezer", "config": { "url": url } })).await;
    let result = ok(&server, "connection.check", json!({ "kind": "eliezer" })).await;
    assert_eq!(result["ok"], false, "zu groß: {result}");
    assert!(result["message"].as_str().unwrap().contains("größer als 1 MiB"), "Grenze genannt: {result}");

    let port = closed_port().await;
    ok(
        &server,
        "connection.set",
        json!({ "kind": "multigpt", "config": { "url": format!("http://127.0.0.1:{port}/mcp/") } }),
    )
    .await;
    let result = ok(&server, "connection.check", json!({ "kind": "multigpt" })).await;
    assert_eq!(result["ok"], false, "geschlossener Port: {result}");
    assert!(result["message"].as_str().unwrap().contains("Keine Verbindung"), "Meldung: {result}");
}

/// Mocked PostgreSQL peer: demands a plain-text password, remembers it and rejects (SQLSTATE 28P01), like a server on a
/// wrong password.
async fn fake_postgres(received: Arc<Mutex<Option<String>>>) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        loop {
            let (mut socket, _) = listener.accept().await.unwrap();
            let received = received.clone();
            tokio::spawn(async move {
                // Startup message (possibly an SSLRequest before it, then „N“).
                loop {
                    let mut len = [0u8; 4];
                    if socket.read_exact(&mut len).await.is_err() {
                        return;
                    }
                    let mut rest = vec![0u8; u32::from_be_bytes(len) as usize - 4];
                    socket.read_exact(&mut rest).await.unwrap();
                    if rest[..4] == 80877103u32.to_be_bytes() {
                        socket.write_all(b"N").await.unwrap();
                        continue;
                    }
                    break;
                }
                // AuthenticationCleartextPassword
                socket.write_all(&[b'R', 0, 0, 0, 8, 0, 0, 0, 3]).await.unwrap();
                let mut tag = [0u8; 5];
                if socket.read_exact(&mut tag).await.is_err() {
                    return;
                }
                let len = u32::from_be_bytes([tag[1], tag[2], tag[3], tag[4]]) as usize;
                let mut password = vec![0u8; len - 4];
                socket.read_exact(&mut password).await.unwrap();
                *received.lock().unwrap() = Some(String::from_utf8_lossy(&password).trim_end_matches('\0').to_owned());
                let mut fields = Vec::new();
                for (code, text) in [
                    (b'S', "FATAL"),
                    (b'V', "FATAL"),
                    (b'C', "28P01"),
                    (b'M', "password authentication failed for user \"probe\""),
                ] {
                    fields.push(code);
                    fields.extend_from_slice(text.as_bytes());
                    fields.push(0);
                }
                fields.push(0);
                let mut message = vec![b'E'];
                message.extend_from_slice(&(fields.len() as u32 + 4).to_be_bytes());
                message.extend_from_slice(&fields);
                let _ = socket.write_all(&message).await;
            });
        }
    });
    port
}

#[tokio::test]
async fn pgvector_failures_are_results_with_a_message() {
    let server = open_with(MemorySecrets::with([("pgvector", SECRET)])).await;
    let result = ok(&server, "connection.check", json!({ "kind": "pgvector" })).await;
    assert_eq!(result["ok"], false, "ohne Host: {result}");
    assert!(result["message"].as_str().unwrap().contains("Kein Rechner"), "Meldung ohne Host: {result}");

    let port = closed_port().await;
    ok(&server, "connection.set", json!({ "kind": "pgvector", "config": { "host": "127.0.0.1", "port": port, "user": "probe", "tls": "disable" } })).await;
    confirm_secret(&server, "pgvector").await;
    let result = ok(&server, "connection.check", json!({ "kind": "pgvector" })).await;
    assert_eq!(result["ok"], false, "geschlossener Port: {result}");
    assert!(result["message"].as_str().unwrap().contains("Keine Verbindung"), "Meldung: {result}");

    let received = Arc::new(Mutex::new(None));
    let port = fake_postgres(received.clone()).await;
    ok(&server, "connection.set", json!({ "kind": "pgvector", "config": { "port": port, "tls": "prefer" } })).await;
    confirm_secret(&server, "pgvector").await;
    let result = ok(&server, "connection.check", json!({ "kind": "pgvector" })).await;
    assert_eq!(result["ok"], false, "abgelehntes Passwort: {result}");
    let message = result["message"].as_str().unwrap();
    assert!(message.contains("Anmeldung abgelehnt") && message.contains("baruch/pgvector"), "Meldung: {message}");
    assert_eq!(received.lock().unwrap().as_deref(), Some(SECRET), "das Passwort kam aus dem Schlüsselbund (Speicher)");
    assert_clean("pgvector-Prüfung", &result.to_string());
    let row = ok(&server, "connection.get", json!({ "kind": "pgvector" })).await;
    assert_eq!(row["check"]["ok"], false, "Fehlschlag gespeichert: {row}");
}

/// Own throwaway cluster; `Drop` stops it with `pg_ctl -m immediate` (only this data directory).
struct Cluster {
    pg_ctl: PathBuf,
    data: PathBuf,
    port: u16,
    _dir: tempfile::TempDir,
}

impl Drop for Cluster {
    fn drop(&mut self) {
        let _ = std::process::Command::new(&self.pg_ctl)
            .arg("-D")
            .arg(&self.data)
            .args(["-m", "immediate", "-w", "stop"])
            .output();
    }
}

/// `/usr/lib/postgresql/<n>/bin` with `initdb`, `pg_ctl`, `postgres`; highest version first.
fn postgres_bin() -> Option<PathBuf> {
    let mut found: Vec<PathBuf> = std::fs::read_dir("/usr/lib/postgresql")
        .ok()?
        .filter_map(|e| e.ok().map(|e| e.path().join("bin")))
        .filter(|bin| ["initdb", "pg_ctl", "postgres"].iter().all(|p| bin.join(p).is_file()))
        .collect();
    found.sort();
    found.pop()
}

fn run(program: &Path, args: &[&std::ffi::OsStr]) -> Result<(), String> {
    let output = std::process::Command::new(program).args(args).output().map_err(|e| e.to_string())?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "{}: {}{}",
            program.display(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ))
    }
}

async fn start_cluster(bin: &Path, password: &str) -> Cluster {
    let dir = tempfile::tempdir_in(env!("CARGO_TARGET_TMPDIR")).unwrap();
    let data = dir.path().join("daten");
    let pwfile = dir.path().join("passwort");
    std::fs::write(&pwfile, format!("{password}\n")).unwrap();
    run(
        &bin.join("initdb"),
        &[
            "-D".as_ref(),
            data.as_os_str(),
            "-U".as_ref(),
            "probe".as_ref(),
            "-A".as_ref(),
            "scram-sha-256".as_ref(),
            format!("--pwfile={}", pwfile.display()).as_ref(),
            "-E".as_ref(),
            "UTF8".as_ref(),
            "--locale=C".as_ref(),
            "--no-sync".as_ref(),
            "--no-instructions".as_ref(),
        ],
    )
    .expect("initdb des Wegwerf-Clusters");
    let port = closed_port().await;
    let options = format!("-p {port} -c listen_addresses=127.0.0.1 -c unix_socket_directories='' -c fsync=off");
    let cluster = Cluster { pg_ctl: bin.join("pg_ctl"), data: data.clone(), port, _dir: dir };
    run(
        &cluster.pg_ctl,
        &[
            "-D".as_ref(),
            data.as_os_str(),
            "-l".as_ref(),
            data.join("log.txt").as_os_str(),
            "-o".as_ref(),
            options.as_ref(),
            "-w".as_ref(),
            "-t".as_ref(),
            "60".as_ref(),
            "start".as_ref(),
        ],
    )
    .expect("pg_ctl start des Wegwerf-Clusters");
    cluster
}

#[tokio::test]
async fn pgvector_against_an_own_throwaway_cluster_connects_and_reports_the_missing_extension() {
    let Some(bin) = postgres_bin() else {
        eprintln!("ÜBERSPRUNGEN: kein PostgreSQL unter /usr/lib/postgresql/*/bin (initdb, pg_ctl, postgres)");
        return;
    };
    let cluster = start_cluster(&bin, SECRET).await;
    let server = open_with(MemorySecrets::with([("pgvector", SECRET)])).await;
    ok(
        &server,
        "connection.set",
        json!({ "kind": "pgvector", "config": { "host": "127.0.0.1", "port": cluster.port, "database": "postgres", "user": "probe", "tls": "prefer" } }),
    )
    .await;
    confirm_secret(&server, "pgvector").await;
    let result = ok(&server, "connection.check", json!({ "kind": "pgvector" })).await;
    assert!(
        result["details"]["server_version"].as_str().is_some_and(|v| v.starts_with("PostgreSQL")),
        "Verbindung steht (scram-sha-256, Passwort aus dem Schlüsselbund): {result}"
    );
    assert_eq!(result["ok"], false, "ohne CREATE EXTENSION vector ist pgvector nicht nutzbar: {result}");
    assert!(result["message"].as_str().unwrap().contains("Erweiterung vector fehlt"), "Meldung: {result}");
    assert!(result["details"]["vector"].is_null(), "vector: null: {result}");
    assert_clean("pgvector echt", &result.to_string());
    let html = page(&server, None, &[]).await;
    assert!(section(&html, "pgvector").contains("Erweiterung vector: fehlt"), "auf der Seite: {html}");

    server.state.secrets.set("pgvector", "falsches-passwort").unwrap();
    let result = ok(&server, "connection.check", json!({ "kind": "pgvector" })).await;
    assert_eq!(result["ok"], false, "falsches Passwort: {result}");
    assert!(result["message"].as_str().unwrap().contains("Anmeldung abgelehnt"), "Meldung: {result}");
    drop(cluster);
}

// ---- Secrets across all ways -----------------------------------------------------------------------

#[tokio::test]
async fn the_test_secret_appears_in_no_page_answer_message_or_check() {
    let server =
        open_with(MemorySecrets::with([("eliezer", SECRET), ("multigpt", SECRET), ("pgvector", SECRET)])).await;
    let work = server.state.default_work().await.unwrap();
    let mut seen: Vec<(String, String)> = Vec::new();
    let url = fake_eliezer().await;
    for (method, params) in [
        ("connection.set", json!({ "kind": "eliezer", "config": { "url": url } })),
        ("connection.set_secret", json!({ "kind": "eliezer", "secret": SECRET })),
        ("connection.check", json!({ "kind": "eliezer" })),
        (
            "connection.set",
            json!({ "kind": "multigpt", "config": { "url": format!("https://x:{SECRET}@mcp.example/") } }),
        ),
        (
            "connection.set",
            json!({ "kind": "multigpt", "config": { "url": format!("https://mcp.example/?token={SECRET}") } }),
        ),
        ("connection.set", json!({ "kind": "multigpt", "config": { "password": SECRET } })),
        (
            "connection.set",
            json!({ "kind": "multigpt", "config": Value::Object([(SECRET.to_owned(), json!("x"))].into_iter().collect()) }),
        ),
        ("connection.set_secret", json!({ "kind": "lancedb", "secret": SECRET })),
        ("connection.set_secret", json!({ "kind": "eliezer", "secret": format!("{SECRET}\n{SECRET}") })),
        ("connection.set_secret", json!({ "kind": "eliezer", "secret": SECRET })),
        ("connection.list", json!({})),
        ("connection.get", json!({ "kind": "eliezer" })),
        ("connection.get", json!({ "kind": "multigpt" })),
        ("connection.get", json!({ "kind": "pgvector" })),
        ("connection.check", json!({ "kind": "pgvector" })),
        ("connection.check", json!({ "kind": "lancedb" })),
        ("connection.rag_get", json!({ "work": work })),
        ("sources.list", json!({})),
    ] {
        seen.push((format!("RPC {method}"), server.rpc(method, params).await.to_string()));
    }
    seen.push(("Seite Verbindungen".into(), page(&server, Some(work), &[("ok", "geprueft")]).await));
    seen.push(("Seite Quellen".into(), sources_page(&server, &[]).await));
    let act = "/einstellungen/verbindungen/act";
    for body in [
        format!("method=connection.set&kind=eliezer&url={}&secret={}", form_escape(&url), form_escape(SECRET)),
        format!("method=connection.set&kind=multigpt&url={}", form_escape(&format!("https://u:{SECRET}@x.example/"))),
        format!("method=connection.set_secret&kind=pgvector&secret={}", form_escape(SECRET)),
        format!("method=connection.set_secret&kind=lancedb&secret={}", form_escape(SECRET)),
        "method=connection.check&kind=eliezer".to_owned(),
    ] {
        let (_, location) = server.post_form(act, &body).await;
        seen.push((format!("Weiterleitung {body}"), location.unwrap_or_default()));
    }
    let checks: Vec<String> = sqlx::query_scalar(
        "SELECT coalesce(check_message, '') || coalesce(check_details, '') || config FROM connection",
    )
    .fetch_all(server.state.store.pool())
    .await
    .unwrap();
    seen.push(("Tabelle connection".into(), checks.join("\n")));
    let (_, _, tools) = mcp(&server, json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list" })).await;
    seen.push(("MCP tools/list".into(), tools.to_string()));
    for (what, text) in &seen {
        assert_clean(what, text);
    }
    assert!(
        seen.iter().any(|(w, t)| w == "RPC connection.check" && t.contains("spiegel-***")),
        "Prüfung lief wirklich: {seen:?}"
    );
    assert_eq!(
        server.state.secrets.get("eliezer").unwrap().as_deref(),
        Some(SECRET),
        "Geheimnis liegt nur im Schlüsselbund"
    );
}

// ---- MCP -------------------------------------------------------------------------------------------

async fn mcp(server: &common::Server, message: Value) -> (StatusCode, HeaderMap, Value) {
    let mut request = Request::post("/mcp")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(message.to_string()))
        .unwrap();
    request.extensions_mut().insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 40000))));
    let (status, headers, body) = server.send(request).await;
    (status, headers, serde_json::from_str(&body).unwrap_or(Value::Null))
}

#[tokio::test]
async fn mcp_sources_list_reads_the_list_and_changes_nothing() {
    let server = common::open().await;
    let (_, _, tools) = mcp(&server, json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list" })).await;
    let names: Vec<&str> =
        tools["result"]["tools"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap()).collect();
    let at = names.iter().position(|n| *n == "sources_list").expect("sources_list in tools/list");
    assert_eq!(names[at - 1], "read_asset", "direkt hinter read_asset: {names:?}");
    assert_eq!(names[at + 1], "idea_create", "vor idea_create: {names:?}");
    assert!(
        !names.iter().any(|n| n.starts_with("connection") || n.starts_with("settings")),
        "keine Verbindungen über MCP"
    );
    let tool = &tools["result"]["tools"][at];
    assert!(!tool["description"].as_str().unwrap().starts_with("STUB"), "kein STUB: {tool}");

    ok(&server, "sources.create", json!({ "title": "Atlas", "url": "https://atlas.example/", "note": "Karten" })).await;
    ok(&server, "sources.create", json!({ "title": "Brevier", "url": "https://brevier.example/" })).await;
    let before = ok(&server, "sources.list", json!({})).await;
    let (status, _, answer) = mcp(
        &server,
        json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": { "name": "sources_list", "arguments": { "title": "X" } } }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "tools/call: {answer}");
    assert_eq!(answer["result"]["isError"], false, "gelingt: {answer}");
    let listed = &answer["result"]["structuredContent"]["sources"];
    assert_eq!(
        listed[0],
        json!({ "id": before["sources"][0]["id"], "title": "Atlas", "url": "https://atlas.example/", "note": "Karten", "position": 1 }),
        "Felder nach Vertrag: {answer}"
    );
    assert_eq!(listed.as_array().unwrap().len(), 2, "beide Quellen");
    assert_eq!(ok(&server, "sources.list", json!({})).await, before, "nichts geändert (auch nicht mit Argumenten)");
}

// ---- Binding of the secret to the target ---------------------------------------------------------

/// Foreign target that records every `Authorization` header and answers like Eliezer.
async fn recording_eliezer(seen: Arc<Mutex<Vec<String>>>) -> String {
    let app = Router::new().route(
        "/v1/models",
        get(move |headers: HeaderMap| {
            let seen = seen.clone();
            async move {
                seen.lock().unwrap().push(bearer(&headers));
                axum::Json(json!({ "object": "list", "data": [{ "id": "fremd" }] }))
            }
        }),
    );
    serve(app).await
}

#[tokio::test]
async fn a_changed_address_never_receives_the_stored_secret() {
    // Without the binding two RPC calls (connection.set with a foreign address, connection.check) would draw off every
    // stored key: as a bearer or as a plain-text password to the new target.
    let server = open_with(MemorySecrets::with([("eliezer", SECRET), ("pgvector", SECRET)])).await;
    let seen = Arc::new(Mutex::new(Vec::new()));
    let url = recording_eliezer(seen.clone()).await;
    ok(&server, "connection.set", json!({ "kind": "eliezer", "config": { "url": format!("{url}/v1") } })).await;
    let row = ok(&server, "connection.get", json!({ "kind": "eliezer" })).await;
    assert_eq!(row["secret"], "hinterlegt", "das Geheimnis bleibt im Schlüsselbund (nichts gelöscht): {row}");
    assert_eq!(row["secret_for"], "http://127.0.0.1:1234", "es gilt nur für das bisherige Ziel: {row}");

    let result = ok(&server, "connection.check", json!({ "kind": "eliezer" })).await;
    assert_eq!(result["ok"], false, "nicht geprüft: {result}");
    assert_eq!(result["details"]["secret_withheld"], true, "zurückgehalten: {result}");
    let message = result["message"].as_str().unwrap();
    assert!(message.contains("Nicht geprüft") && message.contains("neu eintragen"), "Meldung: {message}");
    assert!(seen.lock().unwrap().is_empty(), "das neue Ziel wird gar nicht erst gefragt");
    assert_clean("zurückgehaltene Prüfung", &result.to_string());
    let html = page(&server, None, &[]).await;
    let part = section(&html, "eliezer");
    assert!(part.contains("data-secret-for") && part.contains("127.0.0.1:1234"), "Hinweis auf der Seite: {part}");
    assert_clean("Seite mit Hinweis", &html);

    // pgvector: even with TLS „prefer“ and a peer that demands plain text nothing goes out.
    let received = Arc::new(Mutex::new(None));
    let port = fake_postgres(received.clone()).await;
    let config = json!({ "host": "127.0.0.1", "port": port, "user": "probe", "tls": "prefer" });
    ok(&server, "connection.set", json!({ "kind": "pgvector", "config": config })).await;
    let result = ok(&server, "connection.check", json!({ "kind": "pgvector" })).await;
    assert_eq!(result["details"]["secret_withheld"], true, "pgvector zurückgehalten: {result}");
    assert_eq!(*received.lock().unwrap(), None, "kein Passwort beim neuen Rechner");

    // Another path on the same server is the same target; back at the old target it applies again.
    ok(&server, "connection.set", json!({ "kind": "eliezer", "config": { "url": "http://127.0.0.1:1234/v1" } })).await;
    let row = ok(&server, "connection.get", json!({ "kind": "eliezer" })).await;
    assert!(row.get("secret_for").is_none(), "wieder beim gebundenen Ziel: {row}");

    // Confirming means: entering the secret anew for the new target. Only then does it go there.
    ok(&server, "connection.set", json!({ "kind": "eliezer", "config": { "url": url } })).await;
    let result = ok(&server, "connection.check", json!({ "kind": "eliezer" })).await;
    assert_eq!(result["details"]["secret_withheld"], true, "weiter zurückgehalten: {result}");
    ok(&server, "connection.set_secret", json!({ "kind": "eliezer", "secret": SECRET })).await;
    let result = ok(&server, "connection.check", json!({ "kind": "eliezer" })).await;
    assert_eq!(result["ok"], true, "neu eingetragen: jetzt geprüft: {result}");
    assert_eq!(*seen.lock().unwrap(), [format!("Bearer {SECRET}")], "genau einmal, nach dem Neueintrag");

    // Removing dissolves the binding; a secret stored later (outside) applies to the present target.
    ok(&server, "connection.set", json!({ "kind": "eliezer", "config": { "url": "http://127.0.0.1:1234" } })).await;
    ok(&server, "connection.delete_secret", json!({ "kind": "eliezer", "confirm": true })).await;
    let row = ok(&server, "connection.get", json!({ "kind": "eliezer" })).await;
    assert_eq!((row["secret"].as_str(), row.get("secret_for")), (Some("fehlt"), None), "ohne Geheimnis keine Bindung");
}

#[tokio::test]
async fn without_a_stored_secret_an_address_change_binds_nothing() {
    let server = common::open().await;
    let seen = Arc::new(Mutex::new(Vec::new()));
    let url = recording_eliezer(seen.clone()).await;
    ok(&server, "connection.set", json!({ "kind": "eliezer", "config": { "url": url } })).await;
    let bound: Option<String> = sqlx::query_scalar("SELECT secret_target FROM connection WHERE kind = 'eliezer'")
        .fetch_one(server.state.store.pool())
        .await
        .unwrap();
    assert_eq!(bound, None, "nichts zu binden, wenn kein Geheimnis da ist");
    server.state.secrets.set("eliezer", SECRET).unwrap();
    let result = ok(&server, "connection.check", json!({ "kind": "eliezer" })).await;
    assert_eq!(result["ok"], true, "ein danach hinterlegtes Geheimnis gilt dem jetzigen Ziel: {result}");
}

/// Keyring that reads but rejects every write (locked).
struct LockedKeyring;

impl baruch_server::connections::SecretStore for LockedKeyring {
    fn get(&self, account: &str) -> Result<Option<String>, String> {
        Ok((account == "eliezer").then(|| SECRET.to_owned()))
    }
    fn set(&self, _account: &str, _secret: &str) -> Result<(), String> {
        Err("kein Zugriff auf den Schlüsselbund (gesperrt oder nicht erreichbar)".into())
    }
    fn delete(&self, _account: &str) -> Result<(), String> {
        Err("kein Zugriff auf den Schlüsselbund (gesperrt oder nicht erreichbar)".into())
    }
}

#[tokio::test]
async fn saving_with_a_secret_checks_it_first_and_reports_what_really_changed() {
    let server = open_with(MemorySecrets::default()).await;
    let act = "/einstellungen/verbindungen/act";
    ok(&server, "connection.set", json!({ "kind": "multigpt", "config": { "url": "https://alt.example/mcp/" } })).await;
    server
        .state
        .store
        .connection_record_check(baruch_store::ConnectionKind::Multigpt, true, "steht", &json!({}))
        .await
        .unwrap();

    // Invalid secret (control character) with a new address: rejected before anything is saved.
    let body =
        format!("method=connection.set&kind=multigpt&url={}&secret=ab%07cd", form_escape("https://neu.example/mcp/"));
    let location = form(&server, act, &body).await;
    assert_eq!(location, "/einstellungen/verbindungen?fehler=geheimnis_ungueltig&kind=multigpt", "abgelehnt");
    let row = ok(&server, "connection.get", json!({ "kind": "multigpt" })).await;
    assert_eq!(row["config"]["url"], "https://alt.example/mcp/", "„nichts geändert“ stimmt: alte Adresse: {row}");
    assert_eq!(row["check"]["ok"], true, "und die letzte Prüfung bleibt: {row}");
    let long = "x".repeat(4097);
    let body =
        format!("method=connection.set&kind=multigpt&url={}&secret={long}", form_escape("https://neu.example/mcp/"));
    assert!(form(&server, act, &body).await.contains("fehler=geheimnis_ungueltig"), "zu lang ebenso");
    let row = ok(&server, "connection.get", json!({ "kind": "multigpt" })).await;
    assert_eq!(row["config"]["url"], "https://alt.example/mcp/", "auch dann nichts gespeichert");

    // Locked keyring: the settings are saved, the secret is not – and that is what the page says.
    let data = tempfile::tempdir().unwrap();
    let state = AppState::open_with_secrets(data.path(), Arc::new(LockedKeyring)).await.unwrap();
    let state = Arc::new(state);
    let locked = common::Server { app: common::router(state.clone()), state, data };
    let body = format!(
        "method=connection.set&kind=eliezer&url={}&secret={}",
        form_escape("http://127.0.0.1:9/"),
        form_escape("neu-und-lang")
    );
    let location = form(&locked, act, &body).await;
    assert_eq!(
        location, "/einstellungen/verbindungen?fehler=gespeichert_ohne_geheimnis&kind=eliezer",
        "ehrlicher Code"
    );
    let row = ok(&locked, "connection.get", json!({ "kind": "eliezer" })).await;
    assert_eq!(row["config"]["url"], "http://127.0.0.1:9/", "die Einstellungen sind tatsächlich gespeichert: {row}");
    assert_eq!(row["secret_for"], "http://127.0.0.1:1234", "das alte Geheimnis bleibt beim alten Ziel: {row}");
    let html = page(&locked, None, &[("fehler", "gespeichert_ohne_geheimnis"), ("kind", "eliezer")]).await;
    assert!(html.contains("Die Einstellungen sind gespeichert, das Geheimnis aber nicht"), "Meldung: {html}");
    assert!(!section(&html, "eliezer").contains("nichts geändert"), "kein falsches „nichts geändert“");
    assert_clean("Seite nach gesperrtem Schlüsselbund", &html);
}
