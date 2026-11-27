// SPDX-License-Identifier: AGPL-3.0-or-later
//! „Verbindung prüfen“: one attempt per call (no retry), time limit [`TIMEOUT`] per request
//! and – since Round 10 – once more around the whole check (`connections::run_check`), response
//! at most [`MAX_BODY`]. A failure is a result (`ok: false` with message), not an error.
//!
//! - Eliezer: `GET <url>/v1/models` (Bearer if a key is stored) → `{models: [id…]}`;
//!   `run_check` then reconciles the models of the personas (`details.personas`, like `model.list`).
//! - MultiGPT: MCP over HTTP – `initialize` (protocol 2025-06-18, `clientInfo.name = "baruch"`),
//!   `notifications/initialized`, `tools/list` with `Mcp-Session-Id` through `nextCursor` (Round 12: at most
//!   10 pages), `list_collections` if offered; responses as `application/json` or
//!   `text/event-stream` (`data:` lines) → `{server: {name, version}, tools, pages, collections: [{name,
//!   count}]}`. Since Round 12 the MCP client lives in [`super::multigpt`]. Since Round 13 the check has
//!   a shared time limit [`TIMEOUT`] around all requests and uses no session pool (the session of the
//!   check is, as before, left to the remote side to clean up).
//! - pgvector: connection through sqlx (runtime API), `SELECT version()` and the version of the
//!   extension `vector` → `{server_version, vector}`; if the extension is missing, that is `ok: false`.
//!
//! The secret goes only into the header or the login. Messages and details pass through [`scrub`] before
//! storing: should a remote side echo the secret, `***` stands there.
//! The check does not follow redirects (the key would otherwise go to another address).

use std::time::Duration;

use serde_json::{Value, json};

use super::multigpt::{COLLECTIONS_TOOL, Limits, MAX_TOOL_PAGES, McpClient, parse_collections, tool_payload};

/// Time limit per check.
pub const TIMEOUT: Duration = Duration::from_secs(10);
/// Largest response that is read.
pub const MAX_BODY: usize = 1024 * 1024;
/// MCP protocol version for `initialize`.
pub const MCP_PROTOCOL: &str = "2025-06-18";

/// Result of a check.
#[derive(Debug, Clone, PartialEq)]
pub struct Outcome {
    pub ok: bool,
    pub message: String,
    pub details: Value,
}

impl Outcome {
    fn ok(message: impl Into<String>, details: Value) -> Outcome {
        Outcome { ok: true, message: message.into(), details }
    }

    fn failed(message: impl Into<String>) -> Outcome {
        Outcome { ok: false, message: message.into(), details: json!({}) }
    }
}

/// Replace every occurrence of `secret` in message and details with `***` – through [`baruch_llm::scrub`]
/// (Round 23: also Base64- and percent-encoded).
pub fn scrub(outcome: Outcome, secret: Option<&str>) -> Outcome {
    let Some(secret) = secret.filter(|s| !s.is_empty()) else { return outcome };
    Outcome {
        ok: outcome.ok,
        message: baruch_llm::scrub(&outcome.message, Some(secret)),
        details: scrub_value(outcome.details, Some(secret)),
    }
}

/// Replace every occurrence of `secret` in texts and keys of a JSON value with `***`
/// ([`baruch_llm::scrub`] per text).
pub fn scrub_value(value: Value, secret: Option<&str>) -> Value {
    let Some(secret) = secret.filter(|s| !s.is_empty()) else { return value };
    let clean = |s: &str| baruch_llm::scrub(s, Some(secret));
    match value {
        Value::String(s) => Value::String(clean(&s)),
        Value::Array(items) => Value::Array(items.into_iter().map(|v| scrub_value(v, Some(secret))).collect()),
        Value::Object(map) => {
            Value::Object(map.into_iter().map(|(k, v)| (clean(&k), scrub_value(v, Some(secret)))).collect())
        }
        other => other,
    }
}

fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(TIMEOUT)
        .connect_timeout(TIMEOUT)
        .redirect(reqwest::redirect::Policy::none())
        .user_agent(concat!("baruch/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| format!("HTTP-Client: {e}"))
}

/// Error of reqwest as a German sentence with cause chain (without headers, so without key).
fn http_error(e: &reqwest::Error, url: &str) -> String {
    if e.is_timeout() {
        return format!("Keine Antwort von {url} innerhalb von {} s.", TIMEOUT.as_secs());
    }
    let mut causes = Vec::new();
    let mut source = std::error::Error::source(e);
    while let Some(cause) = source {
        causes.push(cause.to_string());
        source = cause.source();
    }
    let why = if causes.is_empty() { e.to_string() } else { causes.join(": ") };
    if e.is_connect() {
        format!("Keine Verbindung zu {url}: {why}")
    } else {
        format!("Anfrage an {url} gescheitert: {why}")
    }
}

/// Read the response, at most [`MAX_BODY`] bytes.
async fn body(mut response: reqwest::Response, url: &str) -> Result<Vec<u8>, String> {
    let too_big = || format!("Antwort von {url} ist größer als 1 MiB; abgebrochen.");
    if response.content_length().is_some_and(|n| n > MAX_BODY as u64) {
        return Err(too_big());
    }
    let mut out = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|e| http_error(&e, url))? {
        if out.len() + chunk.len() > MAX_BODY {
            return Err(too_big());
        }
        out.extend_from_slice(&chunk);
    }
    Ok(out)
}

/// Message for a redirect (Round 23): the check never follows it – the key would otherwise go to another
/// address. The target (`Location`) is not named (it would come unchecked from the remote side).
pub(super) fn redirected(target: &str, status: reqwest::StatusCode) -> String {
    format!(
        "{target} leitet weiter (HTTP {}); Baruch folgt Weiterleitungen nicht – bitte die endgültige Adresse eintragen.",
        status.as_u16()
    )
}

pub(super) fn denied(status: reqwest::StatusCode, account: &str, has_secret: bool) -> String {
    let hint = if has_secret {
        format!("Schlüssel baruch/{account} prüfen")
    } else {
        format!("kein Schlüssel hinterlegt (baruch/{account})")
    };
    format!("Zugang verweigert (HTTP {}): {hint}.", status.as_u16())
}

/// `<url>/v1/models`; if the address already ends in `/v1`, append only `/models`.
pub fn models_url(url: &str) -> String {
    let base = url.trim_end_matches('/');
    if base.ends_with("/v1") { format!("{base}/models") } else { format!("{base}/v1/models") }
}

/// Eliezer: model list.
pub async fn eliezer(url: &str, secret: Option<&str>) -> Outcome {
    let target = models_url(url);
    let client = match client() {
        Ok(client) => client,
        Err(e) => return Outcome::failed(e),
    };
    let mut request = client.get(&target).header(reqwest::header::ACCEPT, "application/json");
    if let Some(secret) = secret {
        request = request.bearer_auth(secret);
    }
    let response = match request.send().await {
        Ok(response) => response,
        Err(e) => return Outcome::failed(http_error(&e, &target)),
    };
    let status = response.status();
    if matches!(status.as_u16(), 401 | 403) {
        return Outcome::failed(denied(status, "eliezer", secret.is_some()));
    }
    if status.is_redirection() {
        return Outcome::failed(redirected(&target, status));
    }
    if !status.is_success() {
        return Outcome::failed(format!("{target} antwortet mit HTTP {}.", status.as_u16()));
    }
    let bytes = match body(response, &target).await {
        Ok(bytes) => bytes,
        Err(e) => return Outcome::failed(e),
    };
    let parsed: Value = match serde_json::from_slice(&bytes) {
        Ok(parsed) => parsed,
        Err(_) => return Outcome::failed(format!("{target} liefert kein JSON.")),
    };
    let Some(data) = parsed.get("data").and_then(Value::as_array) else {
        return Outcome::failed(format!("{target} liefert keine Modellliste (Feld data fehlt)."));
    };
    let models: Vec<String> =
        data.iter().filter_map(|m| m.get("id").and_then(Value::as_str)).map(str::to_owned).collect();
    let word = if models.len() == 1 { "Modell" } else { "Modelle" };
    Outcome::ok(format!("Verbindung steht: {} {word}.", models.len()), json!({ "models": models }))
}

/// JSON-RPC response with `id` from an MCP response (JSON or Server-Sent Events).
pub fn rpc_answer(content_type: &str, body: &[u8], id: i64) -> Result<Value, String> {
    super::multigpt::rpc_answer(content_type, body, id).map_err(|e| e.message().to_owned())
}

/// MultiGPT: `initialize`, `notifications/initialized`, `tools/list` through `nextCursor` (at most
/// [`MAX_TOOL_PAGES`] pages, otherwise a note) and – if offered – `list_collections` →
/// `{server: {name, version}, tools, pages, collections: [{name, count}]}`. The client is the same as
/// for the search ([`super::multigpt::McpClient`]), with the limits of the check ([`TIMEOUT`], [`MAX_BODY`]);
/// all requests together have a shared time limit [`TIMEOUT`] (Round 13), without session pool. At the end
/// `DELETE` ends the session (Round 19a; not if the shared time limit aborts the check).
pub async fn multigpt(url: &str, secret: Option<&str>) -> Outcome {
    match tokio::time::timeout(TIMEOUT, multigpt_steps(url, secret)).await {
        Ok(outcome) => outcome,
        Err(_) => Outcome::failed(format!(
            "Zeitlimit: Keine vollständige Antwort von {url} innerhalb von {} s; Prüfung abgebrochen.",
            TIMEOUT.as_secs()
        )),
    }
}

async fn multigpt_steps(url: &str, secret: Option<&str>) -> Outcome {
    // The limit per request lies just above the shared one (integrator Round 15): with equal length the limit of
    // list_collections could run out before the shared one under load, and the check reported „steht“.
    let limits = Limits { timeout: TIMEOUT + Duration::from_secs(1), max_body: MAX_BODY };
    let (mut mcp, server) = match McpClient::connect(url, secret, limits).await {
        Ok(connected) => connected,
        Err(e) => return Outcome::failed(e.message()),
    };
    let outcome = multigpt_session(&mut mcp, server, secret).await;
    // Round 19a: the check ends its MCP session at the remote side (HTTP DELETE with Mcp-Session-Id),
    // also after an error; otherwise sessions would pile up there (it uses no session pool).
    mcp.delete().await;
    outcome
}

/// The steps after `initialize` (tools, collections) in an open session.
async fn multigpt_session(mcp: &mut McpClient, server: Value, secret: Option<&str>) -> Outcome {
    let listed = match mcp.list_tools(MAX_TOOL_PAGES).await {
        Ok(listed) => listed,
        Err(e) => return Outcome::failed(e.message()),
    };
    let names = listed.names();
    let mut notes = Vec::new();
    if listed.more {
        notes.push(format!(
            "tools/list hat mehr als {MAX_TOOL_PAGES} Seiten; gezeigt sind die Werkzeuge der ersten {MAX_TOOL_PAGES}."
        ));
    }
    let collections = if listed.find(COLLECTIONS_TOOL).is_some() {
        match mcp.call_tool(COLLECTIONS_TOOL, json!({})).await {
            Ok(result) => match tool_payload(&result) {
                Some(payload) => parse_collections(&payload),
                None => {
                    notes.push(format!("{COLLECTIONS_TOOL}: Antwort nicht lesbar, keine Sammlungen."));
                    Vec::new()
                }
            },
            Err(e) => {
                notes.push(format!("Sammlungen nicht lesbar: {}", e.message()));
                Vec::new()
            }
        }
    } else {
        notes.push(format!("{COLLECTIONS_TOOL} fehlt; Sammlungen lassen sich von Hand eintragen."));
        Vec::new()
    };
    let name = server.get("name").and_then(Value::as_str).unwrap_or("").to_owned();
    let version = server.get("version").and_then(Value::as_str).unwrap_or("").to_owned();
    let word = if names.len() == 1 { "Werkzeug" } else { "Werkzeuge" };
    let shown = if name.is_empty() { "MCP-Server".to_owned() } else { format!("{name} {version}").trim().to_owned() };
    let mut message = format!("Verbindung steht: {shown}, {} {word}, {} Sammlungen.", names.len(), collections.len());
    for note in notes {
        message.push(' ');
        message.push_str(&note);
    }
    let details = json!({ "server": { "name": name, "version": version }, "tools": names, "pages": listed.pages,
                          "collections": collections });
    scrub(Outcome::ok(message, details), secret)
}

/// pgvector: log in, read server version and version of the extension `vector`.
pub async fn pgvector(config: &Value, secret: Option<&str>) -> Outcome {
    use sqlx::Connection;
    use sqlx::postgres::{PgConnectOptions, PgConnection, PgSslMode};

    let field = |name: &str| config.get(name).and_then(Value::as_str).unwrap_or("").trim().to_owned();
    let host = field("host");
    if host.is_empty() {
        return Outcome::failed("Kein Rechner (Host) eingetragen.");
    }
    let port = config.get("port").and_then(Value::as_u64).and_then(|p| u16::try_from(p).ok()).unwrap_or(5432);
    let ssl = match field("tls").as_str() {
        "disable" => PgSslMode::Disable,
        "require" => PgSslMode::Require,
        "verify-full" => PgSslMode::VerifyFull,
        _ => PgSslMode::Prefer,
    };
    // Without pgpass and with an explicit (possibly empty) password: nothing from the server's environment
    // (PGPASSWORD, ~/.pgpass) gets into the check.
    let mut options = PgConnectOptions::new_without_pgpass()
        .host(&host)
        .port(port)
        .ssl_mode(ssl)
        .password(secret.unwrap_or(""))
        .application_name("baruch");
    let user = field("user");
    if !user.is_empty() {
        options = options.username(&user);
    }
    let database = field("database");
    if !database.is_empty() {
        options = options.database(&database);
    }
    let address = format!("{host}:{port}");
    let attempt = async {
        let mut conn = PgConnection::connect_with(&options).await?;
        let version: String = sqlx::query_scalar("SELECT version()").fetch_one(&mut conn).await?;
        let vector: Option<String> = sqlx::query_scalar("SELECT extversion FROM pg_extension WHERE extname = 'vector'")
            .fetch_optional(&mut conn)
            .await?;
        let _ = conn.close().await;
        Ok::<_, sqlx::Error>((version, vector))
    };
    match tokio::time::timeout(TIMEOUT, attempt).await {
        Err(_) => Outcome::failed(format!("Keine Antwort von {address} innerhalb von {} s.", TIMEOUT.as_secs())),
        Ok(Err(e)) => Outcome::failed(pg_error(&e, &address, secret.is_some())),
        Ok(Ok((version, Some(vector)))) => Outcome::ok(
            format!("Verbindung steht, Erweiterung vector {vector}."),
            json!({ "server_version": version, "vector": vector }),
        ),
        Ok(Ok((version, None))) => Outcome {
            ok: false,
            message: "Verbindung steht, aber die Erweiterung vector fehlt (CREATE EXTENSION vector).".into(),
            details: json!({ "server_version": version, "vector": null }),
        },
    }
}

fn pg_error(e: &sqlx::Error, address: &str, has_secret: bool) -> String {
    match e {
        sqlx::Error::Database(db) if matches!(db.code().as_deref(), Some("28P01" | "28000")) => {
            let hint = if has_secret {
                "Passwort baruch/pgvector prüfen"
            } else {
                "kein Passwort hinterlegt (baruch/pgvector)"
            };
            format!("Anmeldung abgelehnt: {} ({hint}).", db.message())
        }
        sqlx::Error::Database(db) => format!("Datenbank {address}: {}", db.message()),
        sqlx::Error::Io(io) => format!("Keine Verbindung zu {address}: {io}"),
        sqlx::Error::Tls(tls) => format!("TLS mit {address}: {tls}"),
        other => format!("Verbindung zu {address}: {other}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_models_address_is_built_from_the_base() {
        assert_eq!(models_url("http://127.0.0.1:1234"), "http://127.0.0.1:1234/v1/models", "Grundadresse");
        assert_eq!(models_url("http://127.0.0.1:1234/"), "http://127.0.0.1:1234/v1/models", "Schrägstrich am Ende");
        assert_eq!(models_url("http://h/v1/"), "http://h/v1/models", "schon mit /v1");
    }

    #[test]
    fn answers_come_as_json_or_as_event_stream() {
        let json = br#"{"jsonrpc":"2.0","id":1,"result":{"ok":true}}"#;
        assert_eq!(rpc_answer("application/json", json, 1).unwrap(), json!({ "ok": true }), "JSON");
        let sse = b"event: message\ndata: {\"jsonrpc\":\"2.0\",\"method\":\"notifications/progress\"}\n\nevent: message\ndata: {\"jsonrpc\":\"2.0\",\"id\":2,\n data: \"result\":{\"tools\":[]}}\n\n";
        assert_eq!(
            rpc_answer("text/event-stream; charset=utf-8", sse, 2).unwrap_err(),
            "keine JSON-RPC-Antwort mit id 2 gefunden",
            "eine mit Leerzeichen eingerückte Zeile ist keine data:-Zeile"
        );
        let sse = b"event: message\ndata: {\"jsonrpc\":\"2.0\",\"method\":\"notifications/progress\"}\n\ndata: {\"jsonrpc\":\"2.0\",\"id\":2,\ndata: \"result\":{\"tools\":[]}}\n";
        assert_eq!(
            rpc_answer("text/event-stream", sse, 2).unwrap(),
            json!({ "tools": [] }),
            "mehrzeilige data, letzte ohne Leerzeile"
        );
        let error = br#"{"jsonrpc":"2.0","id":1,"error":{"code":-1,"message":"nein"}}"#;
        assert_eq!(rpc_answer("application/json", error, 1).unwrap_err(), "Fehler der Gegenstelle: nein", "Fehler");
    }

    #[test]
    fn scrub_uses_the_shared_function_for_encoded_secrets() {
        // Invented secret; „Wmlubm9iZXItNDcxMQ==“ is its Base64, „Zinnober%2D4711“ its percent form.
        let outcome = Outcome::ok(
            "abgelehnt: Basic Wmlubm9iZXItNDcxMQ==",
            json!({ "echo": ["key=Zinnober%2d4711"], "Wmlubm9iZXItNDcxMQ": "Zinnober-4711" }),
        );
        let clean = scrub(outcome, Some("Zinnober-4711"));
        assert_eq!(clean.message, "abgelehnt: Basic ***", "Base64 in der Meldung ist geschwärzt");
        assert_eq!(
            clean.details,
            json!({ "echo": ["key=***"], "***": "***" }),
            "Prozentform in den Angaben, Base64 als Schlüssel und der Wortlaut als Wert sind geschwärzt"
        );
    }

    #[test]
    fn scrub_removes_the_secret_everywhere() {
        let outcome = Outcome::ok("Schlüssel s3cr3t", json!({ "models": ["s3cr3t-modell"], "s3cr3t": 1 }));
        let clean = scrub(outcome, Some("s3cr3t"));
        let text = format!("{} {}", clean.message, clean.details);
        assert!(!text.contains("s3cr3t") && text.contains("***"), "überall ersetzt: {text}");
    }
}
