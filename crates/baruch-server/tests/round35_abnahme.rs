// SPDX-License-Identifier: AGPL-3.0-or-later
//! Abnahme der Runde 35 „Konten und Rollen“ (Vertrag §5, Integrator): der echte Weg über die Naht der drei Aufgaben
//! – Formulare, Cookie der Sitzung, Schicht `auth::gate`, Tabellen in `roles` – vom Einrichten bis zum Entzug der
//! Freigabe. Die Tests der Aufgaben setzen die Identität über `common::with_who`; hier kommt sie allein aus dem
//! Cookie, das `/einrichten` und `/anmelden` ausgeben.
//!
//! Alles läuft über Formulare (ohne JavaScript bedienbar); `/rpc` dient nur zum Nachlesen und für die Gegenprobe.
//! Kein Test fragt einen fremden Dienst.

use std::net::SocketAddr;
use std::sync::Arc;

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{HeaderMap, Request, StatusCode, header};
use baruch_server::guard::AllowedHosts;
use baruch_server::roles::{LEKTOR_ROUTES, LEKTOR_RPC};
use baruch_server::rpc::{FORBIDDEN, UNAUTHENTICATED};
use serde_json::{Value, json};

use crate::common::{self, Server};

/// Erfundene Passwörter (10 bis 200 Zeichen, ohne Zeichen, die im Formular kodiert werden müssten).
const AUTHOR_PW: &str = "kranich-ueber-dem-moor-7";
const LEKTOR_PW: &str = "torfstich-im-fruehnebel";
/// Name, unter dem der Proxy im Test erreichbar ist (`--allow-host`).
const PROXY: &str = "lektorat.example.org";
const LOOPBACK: SocketAddr = SocketAddr::new(std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST), 50123);

fn get(uri: &str) -> Request<Body> {
    Request::get(uri).body(Body::empty()).unwrap()
}

fn form(uri: &str, body: &str) -> Request<Body> {
    Request::post(uri)
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(body.to_owned()))
        .unwrap()
}

fn rpc_request(method: &str, params: Value) -> Request<Body> {
    let body = json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params });
    Request::post("/rpc").header(header::CONTENT_TYPE, "application/json").body(Body::from(body.to_string())).unwrap()
}

/// Die Anfrage kommt vom eigenen Rechner: Gegenstelle Loopback, `Host` eine IP-Adresse.
fn local(mut request: Request<Body>) -> Request<Body> {
    request.extensions_mut().insert(ConnectInfo(LOOPBACK));
    request.headers_mut().insert(header::HOST, "127.0.0.1:8765".parse().unwrap());
    request
}

/// Die Anfrage kommt über den Proxy auf demselben Rechner: Gegenstelle Loopback, `Host` der Proxy-Name, dazu die
/// Kopfzeile, die ein Proxy anhängt.
fn proxied(mut request: Request<Body>) -> Request<Body> {
    request.extensions_mut().insert(ConnectInfo(LOOPBACK));
    request.headers_mut().insert(header::HOST, PROXY.parse().unwrap());
    request.headers_mut().insert("x-forwarded-for", "203.0.113.9".parse().unwrap());
    request
}

/// Sitzung als Cookie, wie der Browser sie mitschickt – über den Proxy.
fn with_session(request: Request<Body>, token: &str) -> Request<Body> {
    let mut request = proxied(request);
    request.headers_mut().insert(header::COOKIE, format!("baruch_session={token}").parse().unwrap());
    request
}

fn location(headers: &HeaderMap) -> &str {
    headers.get(header::LOCATION).map_or("", |value| value.to_str().unwrap())
}

fn denied_by_role(headers: &HeaderMap) -> bool {
    headers.get("x-baruch-abgelehnt").is_some_and(|value| value == "rolle")
}

/// Das Token aus dem `Set-Cookie` der Antwort (genau eines).
fn token_of(headers: &HeaderMap) -> String {
    let cookies: Vec<&str> = headers
        .get_all(header::SET_COOKIE)
        .iter()
        .map(|value| value.to_str().unwrap())
        .filter(|cookie| cookie.starts_with("baruch_session="))
        .collect();
    assert_eq!(cookies.len(), 1, "genau ein Set-Cookie der Sitzung erwartet: {} Stück", cookies.len());
    cookies[0].trim_start_matches("baruch_session=").split(';').next().unwrap().to_owned()
}

fn enc(text: &str) -> String {
    text.bytes()
        .map(|b| if b.is_ascii_alphanumeric() { (b as char).to_string() } else { format!("%{b:02X}") })
        .collect()
}

/// Server, der zusätzlich den Proxy-Namen erlaubt (wie `serve --allow-host lektorat.example.org`).
async fn open_behind_proxy() -> Server {
    let data = tempfile::tempdir().unwrap();
    let mut state = baruch_server::AppState::open(data.path()).await.expect("Datenordner");
    state.allowed_hosts = AllowedHosts::parse(PROXY).unwrap();
    let state = Arc::new(state);
    Server { app: baruch_server::router(state.clone()), state, data }
}

async fn rpc_with(server: &Server, token: &str, method: &str, params: Value) -> Value {
    let (status, _, body) = server.send(with_session(rpc_request(method, params), token)).await;
    assert_eq!(status, StatusCode::OK, "{method}: /rpc antwortet einem Angemeldeten mit 200 und JSON: {body}");
    serde_json::from_str(&body).unwrap()
}

fn result<'a>(answer: &'a Value, what: &str) -> &'a Value {
    assert!(answer.get("error").is_none(), "{what}: RPC-Fehler {answer}");
    &answer["result"]
}

/// Text der aktuellen Fassung des ersten Abschnitts.
async fn current_body(server: &Server, work: i64) -> String {
    let loaded = server.state.store.load_work(work).await.unwrap();
    let node = &loaded.nodes[0];
    node.revision(node.current_number().unwrap()).unwrap().body.clone()
}

async fn count(server: &Server, sql: &str) -> i64 {
    sqlx::query_scalar(sqlx::AssertSqlSafe(sql))
        .fetch_one(server.state.store.pool())
        .await
        .unwrap_or_else(|e| panic!("{sql}: {e}"))
}

/// Zeilenzahl jeder Tabelle: der Stand der Datenbank in Zahlen.
async fn row_counts(server: &Server) -> std::collections::BTreeMap<String, i64> {
    let pool = server.state.store.pool();
    let tables: Vec<String> = sqlx::query_scalar("SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name")
        .fetch_all(pool)
        .await
        .unwrap();
    let mut counts = std::collections::BTreeMap::new();
    for table in tables {
        let n: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT count(*) FROM \"{table}\"")))
            .fetch_one(pool)
            .await
            .unwrap();
        counts.insert(table, n);
    }
    counts
}

/// Nimmt ein Muster des Routers diesen Pfad an?
fn matches(pattern: &str, path: &str) -> bool {
    let wanted: Vec<&str> = pattern.split('/').collect();
    let given: Vec<&str> = path.split('/').collect();
    wanted.len() == given.len()
        && wanted.iter().zip(&given).all(|(w, g)| if w.starts_with('{') { !g.is_empty() } else { w == g })
}

/// Muster mit Werten füllen: `{id}` mit dem Werk, jeder andere Platzhalter mit `1`.
fn fill(pattern: &str, work: i64) -> String {
    pattern
        .split('/')
        .map(|segment| match segment {
            "{id}" => work.to_string(),
            s if s.starts_with('{') => "1".to_owned(),
            s => s.to_owned(),
        })
        .collect::<Vec<_>>()
        .join("/")
}

/// Kommt `needle` in irgendeiner Datei unter `dir` vor (Datenbank, WAL, Spiegel, alles)?
fn files_containing(dir: &std::path::Path, needle: &[u8], found: &mut Vec<String>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            files_containing(&path, needle, found);
        } else if let Ok(bytes) = std::fs::read(&path)
            && bytes.windows(needle.len()).any(|window| window == needle)
        {
            found.push(path.display().to_string());
        }
    }
}

/// Der Stand nach dem Einrichten: Autor „Tilda“, Lektorin „Bärbel“ (Handle `@lektorat:baerbel`) mit Freigabe für das
/// importierte Werk, beide angemeldet. Das Beispielwerk der Instanz bleibt ohne Freigabe.
struct Desk {
    server: Server,
    /// Das freigegebene Werk und sein Spiegelordner.
    granted: i64,
    slug: String,
    title: String,
    /// Das nicht freigegebene Beispielwerk (kleinste id, Vorgabewerk der Instanz).
    foreign: i64,
    foreign_title: String,
    author: String,
    lektor: String,
    lektor_id: i64,
}

async fn desk() -> Desk {
    let server = open_behind_proxy().await;
    let works = server.rpc("work.list", json!({})).await;
    let sample = &works["result"]["works"][0];
    let foreign = sample["id"].as_i64().expect("das Beispielwerk der neuen Instanz");
    let foreign_title = sample["title"].as_str().unwrap().to_owned();
    let report = server.import_garden().await;

    let body = format!("name=Tilda&display=Tilda+Moorkamp&password={AUTHOR_PW}&password2={AUTHOR_PW}");
    let (status, headers, text) = server.send(local(form("/einrichten", &body))).await;
    assert_eq!(
        (status, location(&headers)),
        (StatusCode::SEE_OTHER, "/einstellungen/konten?ok=konto_angelegt"),
        "Einrichten vom eigenen Rechner legt das Autor-Konto an: {text}"
    );
    let author = token_of(&headers);

    let body = format!(
        "action=create&name=B%C3%A4rbel&display=B%C3%A4rbel+Riedgras&password={LEKTOR_PW}&password2={LEKTOR_PW}"
    );
    let (status, headers, text) = server.send(with_session(form("/einstellungen/konten/act", &body), &author)).await;
    assert_eq!(
        (status, location(&headers)),
        (StatusCode::SEE_OTHER, "/einstellungen/konten?ok=konto_angelegt"),
        "der Autor legt die Lektorin an: {text}"
    );
    let lektor_id = count(&server, "SELECT id FROM account WHERE kind = 'lektor'").await;

    let grant = format!("action=grant&account={lektor_id}");
    let (status, headers, text) =
        server.send(with_session(form(&format!("/werke/{}/mitwirkende/act", report.work), &grant), &author)).await;
    assert_eq!(
        (status, location(&headers)),
        (StatusCode::SEE_OTHER, format!("/werke/{}/mitwirkende?ok=freigegeben", report.work).as_str()),
        "der Autor gibt das Werk frei: {text}"
    );

    let login = format!("name=B%C3%A4rbel&password={LEKTOR_PW}&next=/werke");
    let (status, headers, text) = server.send(proxied(form("/anmelden", &login))).await;
    assert_eq!(
        (status, location(&headers)),
        (StatusCode::SEE_OTHER, "/werke"),
        "die Lektorin meldet sich über den Proxy an und landet auf next: {text}"
    );
    let cookie = headers.get(header::SET_COOKIE).unwrap().to_str().unwrap().to_owned();
    assert!(
        cookie.contains("HttpOnly") && cookie.contains("SameSite=Strict") && cookie.contains("Secure"),
        "das Cookie der Sitzung ist HttpOnly, SameSite=Strict und hinter dem Proxy Secure (Token nicht ausgegeben)"
    );
    let lektor = token_of(&headers);
    Desk {
        server,
        granted: report.work,
        slug: report.slug,
        title: report.title,
        foreign,
        foreign_title,
        author,
        lektor,
        lektor_id,
    }
}

// ---- ohne Konten ----------------------------------------------------------------------------------------------

#[tokio::test]
async fn without_accounts_the_start_page_differs_from_before_only_by_the_menu_entry() {
    let server = common::open().await;
    let (status, headers, page) = server.send(local(get("/"))).await;
    assert_eq!(status, StatusCode::OK, "ohne Konten lädt die Startseite ohne Anmeldung und ohne Umleitung");
    let cookies: Vec<&str> = headers.get_all(header::SET_COOKIE).iter().map(|v| v.to_str().unwrap()).collect();
    assert!(
        cookies.iter().all(|cookie| cookie.starts_with("baruch_work=")),
        "ohne Konten setzt die Startseite wie bisher nur das Cookie des Werks, keines einer Sitzung: {cookies:?}"
    );
    assert!(headers.get(header::CACHE_CONTROL).is_none(), "ohne Konten hängt die Anmelde-Schicht keine Kopfzeile an");
    let (_, plain) = server.get("/").await;
    assert_eq!(page, plain, "mit und ohne bekannte Gegenstelle dieselbe Seite");

    // Der eine neue Eintrag: steht direkt hinter „Baruch beenden…“. Ohne ihn ist die Seite die von vor Runde 35 –
    // nichts von Konten, Rollen oder den Marken der Vorlage bleibt zurück.
    let start = page
        .find("<a class=\"menu-item\" role=\"menuitem\" href=\"/einrichten\"")
        .expect("Eintrag „Konten einrichten…“ im Menü");
    let end = start + page[start..].find("</a>").expect("Ende des Eintrags") + "</a>".len();
    assert!(page[start..end].ends_with(">Konten einrichten…</a>"), "der Eintrag heißt „Konten einrichten…“");
    assert!(
        page[..start].trim_end().ends_with("Baruch beenden…</a>"),
        "der Eintrag folgt im Menü „Baruch ▾“ auf „Baruch beenden…“"
    );
    let before = format!("{}{}", &page[..start], &page[end..]);
    for trace in [
        "Konten einrichten",
        "Mein Konto",
        "/einrichten",
        "/anmelden",
        "/abmelden",
        "/konto",
        "data-rolle",
        "nur-autor",
        "nur-lektor",
        "NUR-",
        "BARUCH-KONTO",
        "/lektorat/act",
        "@lektorat",
        "/mitwirkende",
        "Angemeldet:",
    ] {
        assert!(
            !before.contains(trace),
            "ohne den Menüeintrag darf nichts aus Runde 35 in der Startseite stehen, gefunden: {trace}"
        );
    }
    for kept in ["Baruch beenden…", "data-widget=\"workspace\"", "id=\"band-tab-einfuegen\"", "action=\"/act\""] {
        assert!(before.contains(kept), "die Startseite hat weiter alles von vorher: {kept}");
    }

    // RPC und MCP antworten vom eigenen Rechner ohne Anmeldung wie bisher.
    let (status, _, body) = server.send(local(rpc_request("work.list", json!({})))).await;
    let answer: Value = serde_json::from_str(&body).unwrap();
    assert!(
        status == StatusCode::OK && result(&answer, "work.list")["works"].as_array().is_some_and(|w| w.len() == 1),
        "work.list ohne Konten: {answer}"
    );
    let init = json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
        "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": { "name": "abnahme", "version": "1" } } });
    let request = Request::post("/mcp")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::ACCEPT, "application/json, text/event-stream")
        .body(Body::from(init.to_string()))
        .unwrap();
    let (status, headers, body) = server.send(local(request)).await;
    assert_eq!(status, StatusCode::OK, "MCP initialize vom eigenen Rechner ohne Token: {body}");
    assert!(headers.get("mcp-session-id").is_some(), "MCP vergibt wie bisher eine Sitzung");
}

// ---- Einrichten -----------------------------------------------------------------------------------------------

#[tokio::test]
async fn the_first_account_is_only_set_up_from_the_own_machine_and_then_everything_needs_a_session() {
    let server = open_behind_proxy().await;
    let body = format!("name=Tilda&display=&password={AUTHOR_PW}&password2={AUTHOR_PW}");
    // Über den Proxy und ohne bekannte Gegenstelle: kein Einrichten.
    let (status, _, _) = server.send(proxied(form("/einrichten", &body))).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "über den Proxy richtet niemand das erste Konto ein");
    let (status, _, _) = server.send(form("/einrichten", &body)).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "ohne bekannte Gegenstelle gilt die Anfrage nicht als eigener Rechner");
    let mut disguised = local(form("/einrichten", &body));
    disguised.headers_mut().insert("x-forwarded-for", "203.0.113.9".parse().unwrap());
    let (status, _, _) = server.send(disguised).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "ein Proxy, der Host umschreibt, verrät sich an X-Forwarded-For");
    assert_eq!(count(&server, "SELECT count(*) FROM account").await, 0, "nach drei Abweisungen gibt es kein Konto");

    let (status, headers, text) = server.send(local(form("/einrichten", &body))).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "vom eigenen Rechner gelingt das Einrichten: {text}");
    let author = token_of(&headers);
    assert_eq!(
        count(&server, "SELECT count(*) FROM account WHERE kind = 'author' AND active = 1").await,
        1,
        "das erste Konto ist der Autor"
    );
    let (status, _, _) = server.send(local(form("/einrichten", &body.replace("Tilda", "Zweite")))).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "ein zweites Einrichten ohne Sitzung: 401 wie jedes Formular");
    let second = with_session(form("/einrichten", &body.replace("Tilda", "Zweite")), &author);
    let (status, _, _) = server.send(second).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "und auch der Autor richtet kein zweites erstes Konto ein");
    assert_eq!(count(&server, "SELECT count(*) FROM account").await, 1, "es bleibt bei einem Konto");

    // Ab jetzt: ohne Sitzung keine Seite, kein RPC, kein Formular – auch vom eigenen Rechner (Entscheidung 0.2).
    for request in [local(get("/?work=1")), proxied(get("/?work=1"))] {
        let (status, headers, _) = server.send(request).await;
        assert_eq!(
            (status, location(&headers)),
            (StatusCode::SEE_OTHER, "/anmelden?next=/%3Fwork%3D1"),
            "eine Seite ohne Sitzung leitet zur Anmeldung und merkt sich das Ziel"
        );
    }
    let (status, _, body) = server.send(local(rpc_request("work.list", json!({})))).await;
    let answer: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(
        (status, answer["error"]["code"].as_i64()),
        (StatusCode::UNAUTHORIZED, Some(UNAUTHENTICATED)),
        "/rpc ohne Sitzung: 401 mit JSON-RPC-Fehler"
    );
    assert!(answer.get("result").is_none(), "und ohne Ergebnis: {answer}");
    let posts_before = count(&server, "SELECT count(*) FROM post").await;
    let (status, _, _) = server.send(local(form("/act", "method=post.add&node=0&body=Notiz"))).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "ein Formular ohne Sitzung: 401");
    assert_eq!(count(&server, "SELECT count(*) FROM post").await, posts_before, "und nichts ist geschrieben");
    let (status, _, page) = server.send(proxied(get("/anmelden?next=/werke"))).await;
    assert!(
        status == StatusCode::OK && page.contains("name=\"password\"") && page.contains("action=\"/anmelden\""),
        "die Anmeldeseite ist ohne Sitzung erreichbar und ein schlichtes Formular"
    );
    assert!(!page.contains("<script"), "die Anmeldeseite braucht kein JavaScript");

    // MCP bleibt bei Tokens: Die Anmelde-Schicht fasst /mcp nicht an.
    let init = json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
        "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": { "name": "abnahme", "version": "1" } } });
    let request = Request::post("/mcp")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::ACCEPT, "application/json, text/event-stream")
        .body(Body::from(init.to_string()))
        .unwrap();
    let (status, _, body) = server.send(local(request)).await;
    assert_eq!(status, StatusCode::OK, "MCP vom eigenen Rechner braucht auch mit Konten keine Sitzung: {body}");

    // Mit der Sitzung des Autors geht alles wie vorher.
    let (status, _, page) = server.send(with_session(get("/?work=1"), &author)).await;
    assert!(
        status == StatusCode::OK && page.contains("data-rolle=\"autor\"") && page.contains("Angemeldet: Tilda (Autor)"),
        "der Autor sieht die Arbeitsseite und im Menü, wer angemeldet ist"
    );

    // Falsches Passwort mehrfach: Verzögerung, während der auch das richtige nicht hilft.
    let wrong = "name=Tilda&password=falsch-geraten-1&next=/";
    for attempt in 1..=3 {
        let (status, headers, _) = server.send(proxied(form("/anmelden", wrong))).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "Fehlversuch {attempt}: 401");
        assert!(headers.get(header::SET_COOKIE).is_none(), "Fehlversuch {attempt}: kein Cookie");
    }
    let (status, headers, _) = server.send(proxied(form("/anmelden", wrong))).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS, "der vierte Fehlversuch: 429");
    assert!(headers.get(header::RETRY_AFTER).is_some(), "mit Retry-After");
    let right = format!("name=Tilda&password={AUTHOR_PW}&next=/");
    let (status, headers, _) = server.send(proxied(form("/anmelden", &right))).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS, "während der Wartezeit hilft auch das richtige Passwort nicht");
    assert!(headers.get(header::SET_COOKIE).is_none(), "und es gibt keine Sitzung");
    sqlx::query("UPDATE login_attempt SET blocked_until = strftime('%Y-%m-%dT%H:%M:%fZ','now','-1 seconds')")
        .execute(server.state.store.pool())
        .await
        .unwrap();
    let (status, headers, _) = server.send(proxied(form("/anmelden", &right))).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "nach der Wartezeit gelingt die Anmeldung");
    let second = token_of(&headers);

    // Abmelden beendet genau diese Sitzung.
    let (status, headers, _) = server.send(with_session(form("/abmelden", ""), &second)).await;
    assert_eq!(
        (status, location(&headers)),
        (StatusCode::SEE_OTHER, "/anmelden?ok=abgemeldet"),
        "Abmelden führt zur Anmeldeseite"
    );
    let (status, headers, _) = server.send(with_session(get("/werke"), &second)).await;
    assert_eq!(
        (status, location(&headers)),
        (StatusCode::SEE_OTHER, "/anmelden?next=/werke"),
        "das alte Cookie gilt nach dem Abmelden nicht mehr"
    );
    let (status, _, _) = server.send(with_session(get("/werke"), &author)).await;
    assert_eq!(status, StatusCode::OK, "die andere Sitzung des Autors läuft weiter");

    // Das Passwort steht nirgends im Klartext: weder in der Datenbank noch sonst im Datenordner.
    sqlx::query("PRAGMA wal_checkpoint(FULL)").execute(server.state.store.pool()).await.unwrap();
    let mut found = Vec::new();
    files_containing(server.path(), AUTHOR_PW.as_bytes(), &mut found);
    files_containing(server.path(), b"falsch-geraten-1", &mut found);
    assert!(found.is_empty(), "ein Passwort steht im Klartext im Datenordner: {found:?}");
    let hash: String =
        sqlx::query_scalar("SELECT password FROM account").fetch_one(server.state.store.pool()).await.unwrap();
    assert!(hash.starts_with("$argon2id$v=19$m=19456,t=2,p=1$"), "gespeichert ist der Argon2id-Hash mit den Vorgaben");
    let token_rows: i64 = sqlx::query_scalar("SELECT count(*) FROM account_session WHERE token_hash = ?")
        .bind(&author)
        .fetch_one(server.state.store.pool())
        .await
        .unwrap();
    assert_eq!(token_rows, 0, "auch das Token der Sitzung steht nicht selbst in der Datenbank, nur sein Hash");
}

// ---- der Weg der Lektorin ---------------------------------------------------------------------------------------

#[tokio::test]
async fn a_lektor_works_in_the_granted_work_with_her_handle_until_the_grant_is_revoked() {
    let d = desk().await;
    let (server, g) = (&d.server, d.granted);

    // Sie sieht nur das freigegebene Werk.
    let (status, _, page) = server.send(with_session(get("/werke"), &d.lektor)).await;
    assert_eq!(status, StatusCode::OK, "die Werkliste lädt");
    assert!(page.contains(&d.title), "das freigegebene Werk steht in der Liste");
    assert!(!page.contains(&d.foreign_title), "das Beispielwerk ohne Freigabe steht nirgends in der Seite");
    assert!(page.contains("data-rolle=\"lektor\""), "die Seite kennzeichnet die Rolle für das Stylesheet");
    assert!(page.contains("(Lektor)"), "das Menü nennt die Rolle");
    let listed = rpc_with(server, &d.lektor, "work.list", json!({})).await;
    let ids: Vec<i64> =
        result(&listed, "work.list")["works"].as_array().unwrap().iter().map(|w| w["id"].as_i64().unwrap()).collect();
    assert_eq!(ids, [g], "work.list liefert nur das freigegebene Werk");
    let (status, headers, _) = server.send(with_session(get("/"), &d.lektor)).await;
    assert_eq!(
        (status, location(&headers)),
        (StatusCode::SEE_OTHER, format!("/?work={g}").as_str()),
        "ohne work geht es auf das freigegebene Werk, nie auf das Vorgabewerk der Instanz"
    );
    let (status, headers, _) = server.send(with_session(get(&format!("/?work={}", d.foreign)), &d.lektor)).await;
    assert!(status == StatusCode::FORBIDDEN && denied_by_role(&headers), "das fremde Werk ist gesperrt: {status}");

    // Arbeitsseite ohne JavaScript: Formulare an /lektorat/act, keine Knöpfe des Autors.
    let node = rpc_with(server, &d.lektor, "node.get", json!({ "work": g, "node": 0 })).await;
    let key = result(&node, "node.get")["key"].as_i64().expect("fester Schlüssel des ersten Abschnitts");
    let original = current_body(server, g).await;
    let (status, _, page) = server.send(with_session(get(&format!("/?work={g}&key={key}")), &d.lektor)).await;
    assert_eq!(status, StatusCode::OK, "die Arbeitsseite des freigegebenen Werks lädt");
    assert!(
        page.contains("action=\"/lektorat/act\"") && page.contains("name=\"method\" value=\"revision.propose\""),
        "die Arbeitsseite bietet „Vorschlagen“ als Formular"
    );
    for absent in [
        "value=\"revision.save\"",
        "value=\"revision.accept\"",
        "value=\"revision.delete\"",
        "action=\"/assistenten/act\"",
    ] {
        assert!(!page.contains(absent), "die Arbeitsseite einer Lektorin hat keinen Schreibweg des Autors: {absent}");
    }

    // Vorschlag, Beitrag, Befund – alles über das Formular.
    let mirror = server.state.mirror(g).await.unwrap();
    let head_start = mirror.head().unwrap();
    let text = "= Frühling\n\nDie Beete an der Südmauer werden umgegraben.";
    let propose = format!("method=revision.propose&work={g}&key={key}&body={}&note={}", enc(text), enc("Lage geprüft"));
    let (status, headers, body) = server.send(with_session(form("/lektorat/act", &propose), &d.lektor)).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "der Vorschlag wird angelegt: {body}");
    let back = location(&headers).to_owned();
    let rev: u64 = back.rsplit("rev=").next().and_then(|n| n.parse().ok()).expect("Rücksprung auf die neue Fassung");
    assert!(back.starts_with(&format!("/?work={g}&key={key}")), "zurück zum Abschnitt: {back}");
    let node = rpc_with(server, &d.author, "node.get", json!({ "work": g, "key": key })).await;
    let revisions = result(&node, "node.get")["revisions"].as_array().unwrap().clone();
    let proposal = revisions.iter().find(|r| r["number"].as_u64() == Some(rev)).expect("die neue Fassung");
    assert_eq!(
        (proposal["status"].as_str(), proposal["author"]["persona"].as_str()),
        (Some("proposed"), Some("@lektorat:baerbel")),
        "ein offener Vorschlag unter dem Handle der Lektorin (Umlaut umschrieben)"
    );
    assert_eq!(common::currents(result(&node, "node.get")), 1, "es bleibt bei genau einer aktuellen Fassung");
    assert_eq!(mirror.head().unwrap(), head_start, "ein Vorschlag ist kein Commit am Buch");
    let side = format!("refs/revisions/{key}/{rev}");
    assert!(
        mirror.side_refs("refs/revisions/").unwrap().contains_key(&side),
        "der Vorschlag steht als Seitenzweig im Spiegel"
    );
    if let Some(log) = common::git(&server.mirror_dir_of(&d.slug), &["log", "-1", "--format=%an%n%B", &side]) {
        assert!(log.contains("@lektorat:baerbel"), "der Seitenzweig nennt die Herkunft: {log}");
    }

    let post = format!("method=post.add&work={g}&key={key}&body={}", enc("Stand die Mauer nicht im Osten?"));
    let (status, _, body) = server.send(with_session(form("/lektorat/act", &post), &d.lektor)).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "der Beitrag wird geschrieben: {body}");
    let posts = rpc_with(server, &d.author, "post.page", json!({ "work": g, "key": key })).await;
    let written = &result(&posts, "post.page")["posts"][0];
    assert_eq!(
        (written["author"].as_str(), written["body"].as_str()),
        (Some("@lektorat:baerbel"), Some("Stand die Mauer nicht im Osten?")),
        "der Beitrag steht unter dem Handle"
    );
    assert_eq!(
        mirror.head().unwrap(),
        head_start,
        "ein neuer Beitrag ist wie beim Autor und bei MCP kein Commit – er steht in der Datenbank"
    );
    // Ändert sie ihren Beitrag, steht das mit Herkunft im Spiegel (alte Fassung bleibt).
    let post_id = written["id"].as_i64().unwrap();
    let edit =
        format!("method=post.edit&work={g}&id={post_id}&body={}", enc("Stand die Mauer nicht im Osten? (Kap. 3)"));
    let (status, _, body) = server.send(with_session(form("/lektorat/act", &edit), &d.lektor)).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "den eigenen Beitrag ändert die Lektorin: {body}");
    let head_post = mirror.head().unwrap();
    assert_ne!(head_post, head_start, "das Ändern eines Beitrags ist ein Commit im Spiegel");
    if let Some(log) = common::git(&server.mirror_dir_of(&d.slug), &["log", "-1", "--format=%an%n%B"]) {
        assert!(
            log.starts_with("@lektorat:baerbel\n") && log.contains(&format!("Beitrag {post_id}: bearbeitet")),
            "Autor des Commits ist die Lektorin: {log}"
        );
    }

    let finding = format!(
        "method=finding.add&work={g}&key={key}&kind=critical&body={}&quote={}",
        enc("Kapitel 3 nennt die Ostmauer."),
        enc("Südmauer")
    );
    let (status, _, body) = server.send(with_session(form("/lektorat/act", &finding), &d.lektor)).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "der Befund wird geschrieben: {body}");
    let note = format!(
        "method=finding.add&work={g}&kind=hint&back=befunde&body={}",
        enc("Ein Pflanzplan am Ende wäre hilfreich.")
    );
    let (status, headers, body) = server.send(with_session(form("/lektorat/act", &note), &d.lektor)).await;
    assert_eq!(
        (status, location(&headers)),
        (StatusCode::SEE_OTHER, format!("/werke/{g}/befunde").as_str()),
        "die Anmerkung zum Buch führt zurück auf die Befundseite: {body}"
    );
    let findings = rpc_with(server, &d.author, "finding.list", json!({ "work": g })).await;
    let all = result(&findings, "finding.list")["findings"].as_array().unwrap().clone();
    assert_eq!(all.len(), 2, "der Autor sieht Befund und Anmerkung: {all:?}");
    assert!(all.iter().all(|f| f["persona"] == "@lektorat:baerbel"), "beide unter dem Handle der Lektorin");
    assert!(all.iter().any(|f| f["key"].is_null()), "die Anmerkung zum Buch hängt an keinem Abschnitt");
    assert_ne!(mirror.head().unwrap(), head_post, "Befunde sind Commits im Spiegel");
    if let Some(log) = common::git(&server.mirror_dir_of(&d.slug), &["log", "-2", "--format=%an <%ae>%n%B"]) {
        assert!(
            log.contains("Bärbel Riedgras (@lektorat:baerbel)") && log.contains("Baruch-Lektor: @lektorat:baerbel"),
            "die Commits der Befunde tragen Anzeigename, Handle und Trailer: {log}"
        );
    }
    let (status, _, page) = server.send(with_session(get(&format!("/werke/{g}/befunde")), &d.lektor)).await;
    assert!(
        status == StatusCode::OK && page.contains("action=\"/lektorat/act\"") && page.contains("value=\"finding.add\""),
        "die Befundseite bietet der Lektorin das Formular „Anmerkung zum Buch“"
    );

    // Übernehmen darf sie nicht – weder über /rpc noch über ein Formular.
    let accept = json!({ "work": g, "key": key, "rev": rev });
    let answer = rpc_with(server, &d.lektor, "revision.accept", accept.clone()).await;
    assert_eq!(answer["error"]["code"].as_i64(), Some(FORBIDDEN), "revision.accept bleibt dem Autor: {answer}");
    let accept_form = format!("method=revision.accept&work={g}&key={key}&rev={rev}");
    for uri in ["/act", "/lektorat/act"] {
        let (status, _, _) = server.send(with_session(form(uri, &accept_form), &d.lektor)).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "Übernehmen über {uri} scheitert mit 403");
    }
    let current = current_body(server, g).await;
    assert_eq!(current, original, "nach den Versuchen der Lektorin gilt die alte Fassung");

    // Der Autor übernimmt – ohne JavaScript über /act.
    let (status, _, body) = server.send(with_session(form("/act", &accept_form), &d.author)).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "der Autor übernimmt den Vorschlag: {body}");
    let current = current_body(server, g).await;
    assert_eq!(current, text, "jetzt ist der Text der Lektorin die aktuelle Fassung");
    if let Some(log) = common::git(&server.mirror_dir_of(&d.slug), &["log", "-1", "--format=%B"]) {
        assert!(log.contains("@lektorat:baerbel"), "der Commit der Übernahme nennt, von wem die Fassung stammt: {log}");
    }

    // Entzug der Freigabe wirkt bei der nächsten Anfrage, die Sitzung bleibt.
    let revoke = format!("action=revoke&account={}", d.lektor_id);
    let (status, headers, body) =
        server.send(with_session(form(&format!("/werke/{g}/mitwirkende/act"), &revoke), &d.author)).await;
    assert_eq!(
        (status, location(&headers)),
        (StatusCode::SEE_OTHER, format!("/werke/{g}/mitwirkende?ok=entzogen").as_str()),
        "der Autor entzieht die Freigabe: {body}"
    );
    let (status, headers, _) = server.send(with_session(get(&format!("/?work={g}&key={key}")), &d.lektor)).await;
    assert!(
        status == StatusCode::FORBIDDEN && denied_by_role(&headers),
        "die Arbeitsseite ist nach dem Entzug gesperrt"
    );
    let before = row_counts(server).await;
    let (status, _, _) = server.send(with_session(form("/lektorat/act", &post), &d.lektor)).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "auch Schreiben ist nach dem Entzug 403");
    let answer = rpc_with(server, &d.lektor, "node.get", json!({ "work": g, "key": key })).await;
    assert_eq!(answer["error"]["code"].as_i64(), Some(FORBIDDEN), "und Lesen über /rpc ebenso: {answer}");
    assert_eq!(row_counts(server).await, before, "nach dem Entzug ist nichts mehr geschrieben worden");
    let (status, _, page) = server.send(with_session(get("/werke"), &d.lektor)).await;
    assert!(
        status == StatusCode::OK && page.contains("Noch kein Werk freigegeben.") && !page.contains(&d.title),
        "die Werkliste sagt, dass nichts freigegeben ist, und nennt das Werk nicht mehr"
    );
}

// ---- jeder andere Schreibweg ------------------------------------------------------------------------------------

#[tokio::test]
async fn with_a_real_session_every_route_and_method_outside_the_tables_is_closed_for_a_lektor() {
    let d = desk().await;
    let (server, g) = (&d.server, d.granted);
    sqlx::query("PRAGMA wal_checkpoint(FULL)").execute(server.state.store.pool()).await.unwrap();
    let before = (row_counts(server).await, server.state.mirror(g).await.unwrap().head().unwrap());

    let routes = common::registered_routes();
    let mut closed = 0;
    for (verb, pattern) in &routes {
        let path = fill(pattern, g);
        if path == "/mcp" || LEKTOR_ROUTES.iter().any(|(v, allowed, _)| v == verb && matches(allowed, &path)) {
            continue;
        }
        let request = match verb.as_str() {
            "GET" => get(&path),
            _ => form(&path, &format!("work={g}&method=revision.accept&action=delete&confirm=1&id=1&key=1&rev=1")),
        };
        let (status, headers, _) = server.send(with_session(request, &d.lektor)).await;
        assert!(
            status == StatusCode::FORBIDDEN && denied_by_role(&headers),
            "{verb} {path} steht nicht in LEKTOR_ROUTES und muss für die Lektorin 403 sein, war {status}"
        );
        closed += 1;
    }
    assert!(closed >= 100, "die Liste der Routen ist plausibel groß: {closed} gesperrte Routen");

    let mut forbidden = 0;
    for method in common::rpc_method_candidates() {
        if LEKTOR_RPC.iter().any(|(allowed, _)| *allowed == method) {
            continue;
        }
        let params = json!({ "work": g, "key": 1, "node": 0, "rev": 1, "id": 1, "confirm": true, "body": "x" });
        let answer = rpc_with(server, &d.lektor, &method, params).await;
        assert_eq!(
            answer["error"]["code"].as_i64(),
            Some(FORBIDDEN),
            "{method} steht nicht in LEKTOR_RPC und muss für die Lektorin gesperrt sein: {answer}"
        );
        forbidden += 1;
    }
    assert!(forbidden >= 200, "die Liste der Methoden ist plausibel groß: {forbidden} gesperrte Methoden");

    assert_eq!(
        (row_counts(server).await, server.state.mirror(g).await.unwrap().head().unwrap()),
        before,
        "nach allen Versuchen sind Datenbank (Zeilenzahlen) und Spiegel (HEAD) unverändert"
    );
    // Die Sitzung hat die Abweisungen überstanden: Erlaubtes geht weiter.
    let (status, _, _) = server.send(with_session(get(&format!("/?work={g}")), &d.lektor)).await;
    assert_eq!(status, StatusCode::OK, "die Arbeitsseite des freigegebenen Werks lädt weiter");
}

// ---- zuletzt geöffnetes Werk ------------------------------------------------------------------------------------

#[tokio::test]
async fn a_page_without_work_leads_a_lektor_to_the_last_opened_work_if_it_is_granted() {
    let d = desk().await;
    let server = &d.server;
    let with_work_cookie = |work: i64| {
        let mut request = proxied(get("/gliederung"));
        let cookies = format!("baruch_work={work}; baruch_session={}", d.lektor);
        request.headers_mut().insert(header::COOKIE, cookies.parse().unwrap());
        request
    };
    // Erst eine Freigabe: Das Cookie zeigt auf das fremde Werk und zählt nicht.
    let (status, headers, _) = server.send(with_work_cookie(d.foreign)).await;
    assert_eq!(
        (status, location(&headers)),
        (StatusCode::SEE_OTHER, format!("/gliederung?work={}", d.granted).as_str()),
        "ein Cookie auf ein nicht freigegebenes Werk gibt kein Recht – es geht auf das freigegebene"
    );
    // Seiten ohne eigenes Werk (Werkliste, Mein Konto) nehmen ihr Werk für „Zurück zum Werk“ aus dem Cookie bzw. ohne
    // Cookie das Vorgabewerk der Instanz – für die Lektorin darf daraus nie ein Link auf das fremde Werk werden.
    for page in ["/werke", "/konto", "/ueber"] {
        let mut stale = proxied(get(page));
        let cookies = format!("baruch_work={}; baruch_session={}", d.foreign, d.lektor);
        stale.headers_mut().insert(header::COOKIE, cookies.parse().unwrap());
        for (case, request) in [("altes Cookie", stale), ("ohne Cookie", with_session(get(page), &d.lektor))] {
            let (status, _, html) = server.send(request).await;
            assert_eq!(status, StatusCode::OK, "{page} ({case}) lädt");
            for foreign in
                [format!("work={}\"", d.foreign), format!("work={}&", d.foreign), format!("/werke/{}/", d.foreign)]
            {
                assert!(
                    !html.contains(&foreign),
                    "{page} ({case}): kein Link auf das nicht freigegebene Werk ({foreign})"
                );
            }
            assert!(!html.contains(&d.foreign_title), "{page} ({case}): der Titel des fremden Werks steht nirgends");
        }
    }
    let (_, _, html) = server.send(with_session(get("/konto"), &d.lektor)).await;
    assert!(
        html.contains(&format!("href=\"/?work={}\"", d.granted)),
        "„Mein Konto“ führt die Lektorin zurück in das freigegebene Werk"
    );

    // Zweite Freigabe (das Beispielwerk mit der kleineren id).
    let grant = format!("action=grant&account={}", d.lektor_id);
    let (status, _, body) =
        server.send(with_session(form(&format!("/werke/{}/mitwirkende/act", d.foreign), &grant), &d.author)).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "der Autor gibt auch das Beispielwerk frei: {body}");
    assert!(d.foreign < d.granted, "Vorbedingung: Das Beispielwerk hat die kleinere id");
    let (_, headers, _) = server.send(with_session(get("/gliederung"), &d.lektor)).await;
    assert_eq!(
        location(&headers),
        format!("/gliederung?work={}", d.foreign),
        "ohne Cookie geht es auf das kleinste freigegebene Werk"
    );
    let (_, headers, _) = server.send(with_work_cookie(d.granted)).await;
    assert_eq!(
        location(&headers),
        format!("/gliederung?work={}", d.granted),
        "mit dem Cookie des zuletzt geöffneten Werks bleibt die Lektorin in diesem Werk"
    );
    let (_, headers, _) = server.send(with_work_cookie(987_654)).await;
    assert_eq!(
        location(&headers),
        format!("/gliederung?work={}", d.foreign),
        "ein Cookie auf ein unbekanntes Werk zählt nicht"
    );
}
