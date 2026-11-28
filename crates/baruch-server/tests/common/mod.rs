// SPDX-License-Identifier: AGPL-3.0-or-later
//! Shared helpers of the server tests: real data folder in the temp directory (SQLite file, library, Git mirror),
//! router through `tower::ServiceExt::oneshot`.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode, header};
use serde_json::{Value, json};
use tower::ServiceExt;

/// Own small Typst file: preamble with `#set document`, two chapters, section, subsection.
pub const GARDEN: &str = include_str!("../data/gartenbuch.typ");

/// The mark "PDF is being built" (`baruch_server::pdf::claim`) holds per work id in the whole process. Since all server
/// tests run in one binary (`tests/main.rs`), the data folders of **all** modules share the same work ids: whoever
/// builds a PDF or sets the mark holds this lock first.
pub static PDF_SERIAL: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// The queue of the models (`baruch_server::models`, one request at a time, at most eight waiting) holds in the whole
/// process. Formerly every test file ran in its own process; in the bundle (`tests/main.rs`) the files that fill it or
/// check its state (`round10.rs`, `round10_models.rs`) would disturb each other („Warteschlange voll“, a foreign
/// `running`). [`model_queue`] therefore lets in only tests of **one** module at a time; tests of the same module keep
/// running concurrently (as before in their file).
pub async fn model_queue(module: &'static str) -> ModuleTurn {
    loop {
        let changed = TURN_CHANGED.notified();
        {
            let mut turn = TURN.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            if turn.0.is_none_or(|owner| owner == module) {
                turn.0 = Some(module);
                turn.1 += 1;
                return ModuleTurn;
            }
        }
        // `notified()` is created before the release already: no wake-up gets lost.
        changed.await;
    }
}

static TURN: std::sync::Mutex<(Option<&'static str>, usize)> = std::sync::Mutex::new((None, 0));
static TURN_CHANGED: tokio::sync::Notify = tokio::sync::Notify::const_new();

/// Share of one test in the turn of its module ([`model_queue`]); releases it on drop.
pub struct ModuleTurn;

impl Drop for ModuleTurn {
    fn drop(&mut self) {
        let mut turn = TURN.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        turn.1 -= 1;
        if turn.1 == 0 {
            turn.0 = None;
            TURN_CHANGED.notify_waiters();
        }
    }
}

pub struct Server {
    pub data: tempfile::TempDir,
    pub state: Arc<baruch_server::AppState>,
    pub app: Router,
}

/// New data folder with sample work and sample library.
pub async fn open() -> Server {
    let data = tempfile::tempdir().unwrap();
    let state = baruch_server::AppState::open(data.path()).await.expect("Datenordner lässt sich nicht öffnen");
    let state = Arc::new(state);
    Server { app: router(state.clone()), state, data }
}

/// The router of the server, pinned to German for the tests written before Round 39a (contract R39a §1.15): a
/// request with neither an `Accept-Language` header nor a `baruch_lang` cookie gets `Accept-Language: de`. Tests of
/// the language itself set one of the two ([`with_lang`]).
pub fn router(state: Arc<baruch_server::AppState>) -> Router {
    baruch_server::router(state).layer(axum::middleware::from_fn(pin_german))
}

async fn pin_german(mut request: axum::extract::Request, next: axum::middleware::Next) -> axum::response::Response {
    let has_cookie = request
        .headers()
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|line| line.split(';'))
        .any(|pair| pair.trim().starts_with(&format!("{}=", baruch_server::i18n::COOKIE)));
    if !has_cookie && !request.headers().contains_key(header::ACCEPT_LANGUAGE) {
        request.headers_mut().insert(header::ACCEPT_LANGUAGE, axum::http::HeaderValue::from_static("de"));
    }
    next.run(request).await
}

/// Sets `Accept-Language: <lang>` on a request (replaces the German pin of [`router`]).
pub fn with_lang(mut request: Request<Body>, lang: &str) -> Request<Body> {
    request.headers_mut().insert(header::ACCEPT_LANGUAGE, axum::http::HeaderValue::from_str(lang).unwrap());
    request
}

impl Server {
    pub fn path(&self) -> &Path {
        self.data.path()
    }

    /// The only mirror folder under `<data>/works/`.
    pub fn mirror_dir(&self) -> PathBuf {
        let mut dirs: Vec<PathBuf> =
            std::fs::read_dir(self.path().join("works")).unwrap().map(|e| e.unwrap().path()).collect();
        assert_eq!(dirs.len(), 1, "genau ein Werk erwartet: {dirs:?}");
        dirs.pop().unwrap()
    }

    /// Mirror folder of a particular work: `<data>/works/<slug>`.
    pub fn mirror_dir_of(&self, slug: &str) -> PathBuf {
        self.path().join("works").join(slug)
    }

    /// Create Typst source text as a new work through `baruch_server::import::import_typst`.
    pub async fn import(&self, source: &str, file_name: &str) -> baruch_server::import::ImportReport {
        baruch_server::import::import_typst(&self.state, source, file_name).await.expect("Import scheitert")
    }

    /// Import the own test file `tests/data/gartenbuch.typ`.
    pub async fn import_garden(&self) -> baruch_server::import::ImportReport {
        self.import(GARDEN, "gartenbuch.typ").await
    }

    pub async fn rpc(&self, method: &str, params: Value) -> Value {
        rpc(self.app.clone(), method, params).await
    }

    /// Any request; response as status, headers and text.
    pub async fn send(&self, request: Request<Body>) -> (StatusCode, axum::http::HeaderMap, String) {
        let response = self.app.clone().oneshot(request).await.unwrap();
        let status = response.status();
        let headers = response.headers().clone();
        let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        (status, headers, String::from_utf8(bytes.to_vec()).unwrap())
    }

    pub async fn get(&self, uri: &str) -> (StatusCode, String) {
        let response = self.app.clone().oneshot(Request::get(uri).body(Body::empty()).unwrap()).await.unwrap();
        let status = response.status();
        let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        (status, String::from_utf8(bytes.to_vec()).unwrap())
    }

    pub async fn post_form(&self, uri: &str, form: &str) -> (StatusCode, Option<String>) {
        let request = Request::post(uri)
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(Body::from(form.to_owned()))
            .unwrap();
        let response = self.app.clone().oneshot(request).await.unwrap();
        let location = response.headers().get(header::LOCATION).map(|v| v.to_str().unwrap().to_owned());
        (response.status(), location)
    }
}

/// JSON-RPC call to a router (also from `tokio::spawn`, hence without `&Server`).
pub async fn rpc(app: Router, method: &str, params: Value) -> Value {
    let body = json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params });
    let request = Request::post("/rpc")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    let response = app.oneshot(request).await.unwrap();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

/// Number of revisions with status `current` in a `node.get` response.
pub fn currents(node: &Value) -> usize {
    node["revisions"].as_array().unwrap().iter().filter(|r| r["status"] == "current").count()
}

/// `git` in the mirror folder; `None` if there is no git program.
pub fn git(dir: &Path, args: &[&str]) -> Option<String> {
    let output = std::process::Command::new("git").arg("-C").arg(dir).args(args).output().ok()?;
    assert!(output.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&output.stderr));
    Some(String::from_utf8(output.stdout).unwrap())
}

// ---------------------------------------------------------------------------------------------------------------
// Round 35 (interface R35-S1): helpers for the tests of accounts and roles.
// ---------------------------------------------------------------------------------------------------------------

/// Determines who makes the request, without session and password: `auth::gate` takes a [`baruch_server::auth::Who`]
/// from the extensions of the request instead of resolving it itself. Extensions can only be set inside the process,
/// never over the network.
pub fn with_who(mut request: Request<Body>, who: baruch_server::auth::Who) -> Request<Body> {
    request.extensions_mut().insert(who);
    request
}

/// All source files of the server (`crates/baruch-server/src/**/*.rs`) as `(path, content)`, sorted.
fn server_sources() -> Vec<(PathBuf, String)> {
    fn walk(dir: &Path, found: &mut Vec<(PathBuf, String)>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(&path, found);
            } else if path.extension().is_some_and(|e| e == "rs") {
                let text = std::fs::read_to_string(&path).unwrap();
                found.push((path, text));
            }
        }
    }
    let mut found = Vec::new();
    walk(Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/src")), &mut found);
    found.sort();
    found
}

/// End of the call whose opening parenthesis is directly before `start`: position of the matching closing parenthesis.
/// Strings and line comments are skipped.
fn closing_paren(text: &str, start: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    let (mut depth, mut i) = (1usize, start);
    while i < bytes.len() {
        match bytes[i] {
            b'"' => {
                i += 1;
                while i < bytes.len() && bytes[i] != b'"' {
                    i += if bytes[i] == b'\\' { 2 } else { 1 };
                }
            }
            b'/' if bytes.get(i + 1) == Some(&b'/') => {
                while i < bytes.len() && bytes[i] != b'\n' {
                    i += 1;
                }
            }
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}

/// All routes of the server as `("GET" | "POST", pattern)`: every `.route("…", …get(…)…post(…))` from
/// `crates/baruch-server/src/**/*.rs`, also across several lines and with the pattern as a constant of the same file.
/// Without test modules (everything from `#[cfg(test)]` on) and without mocks (`mock.rs`). Sorted, without duplicates.
///
/// The list is built from the source text, not from a maintained table: a new route of future rounds is in it by
/// itself, and the tests of the roles check it without further effort.
pub fn registered_routes() -> Vec<(String, String)> {
    let call = regex::Regex::new(r#"\.route\(\s*(?:"([^"]*)"|([A-Z][A-Z0-9_]*))\s*,"#).unwrap();
    let verb = regex::Regex::new(r"\b(get|post)\(").unwrap();
    let mut routes = std::collections::BTreeSet::new();
    for (path, text) in server_sources() {
        if path.file_name().is_some_and(|name| name == "mock.rs") {
            continue;
        }
        let text = text.split("#[cfg(test)]").next().unwrap();
        for found in call.captures_iter(text) {
            let pattern = match (found.get(1), found.get(2)) {
                (Some(literal), _) => literal.as_str().to_owned(),
                (None, Some(name)) => {
                    let constant = regex::Regex::new(&format!(r#"const {}: &str = "([^"]*)""#, name.as_str())).unwrap();
                    let value = constant.captures(text).unwrap_or_else(|| {
                        panic!("{}: Konstante {} des Routenmusters nicht gefunden", path.display(), name.as_str())
                    });
                    value[1].to_owned()
                }
                (None, None) => unreachable!("eine der beiden Gruppen trifft immer"),
            };
            let start = found.get(0).unwrap().end();
            let end = closing_paren(text, start)
                .unwrap_or_else(|| panic!("{}: .route(\"{pattern}\" ohne schließende Klammer", path.display()));
            let before = routes.len();
            for method in verb.captures_iter(&text[start..end]) {
                routes.insert((method[1].to_uppercase(), pattern.clone()));
            }
            assert!(
                routes.len() > before || routes.iter().any(|(_, p)| *p == pattern),
                "{}: .route(\"{pattern}\" ohne get(…) oder post(…) – registered_routes kennt die Methode nicht",
                path.display()
            );
        }
    }
    routes.into_iter().collect()
}

/// Endings by which a string of the form `word.word` is recognised as a file name. `pdf` and `epub` are missing on
/// purpose: `work.pdf` and `work.epub` are RPC methods.
const FILE_ENDINGS: &[&str] = &[
    "html", "xhtml", "css", "js", "ts", "rs", "typ", "svg", "png", "jpg", "jpeg", "gif", "webp", "json", "xml", "md",
    "mdx", "txt", "sqlite", "docx", "doc", "jar", "baruch", "toml", "zip",
];

/// Everything in the source text of the server that looks like an RPC method: every string
/// `^[a-z][a-z_]*\.[a-z][a-z_]*$` from `crates/baruch-server/src/**/*.rs` (also from test modules), without file names
/// ([`FILE_ENDINGS`]). Sorted, without duplicates. Deliberately a superset (also `baruch.example` and the like): for
/// Lektors every method is locked that is not explicitly allowed – even one that does not exist at all.
pub fn rpc_method_candidates() -> Vec<String> {
    let literal = regex::Regex::new(r#""([a-z][a-z_]*\.([a-z][a-z_]*))""#).unwrap();
    let mut methods = std::collections::BTreeSet::new();
    for (_, text) in server_sources() {
        for found in literal.captures_iter(&text) {
            if !FILE_ENDINGS.contains(&&found[2]) {
                methods.insert(found[1].to_owned());
            }
        }
    }
    methods.into_iter().collect()
}

#[test]
fn registered_routes_finds_the_routes_of_the_server() {
    let routes = registered_routes();
    assert!(routes.len() >= 130, "mindestens 130 Routen (Stand Runde 34), gefunden: {}", routes.len());
    let has = |method: &str, pattern: &str| routes.iter().any(|(m, p)| m == method && p == pattern);
    assert!(has("POST", "/rpc"), "POST /rpc steht mit Schicht (.layer) hinter post(…) – trotzdem erkannt");
    assert!(
        has("POST", "/werke/{id}/karte/einlesen/anlegen"),
        "eine über mehrere Zeilen geschriebene Route (idea_map::files) wird gefunden"
    );
    assert!(has("GET", "/"), "die Arbeitsseite");
    assert!(has("GET", "/werke/import/word"), "ein Muster, das als Konstante (PAGE) im Router steht");
    assert!(has("GET", "/static/js/{name}"), "eine Route mit Platzhalter");
    assert!(
        !routes.iter().any(|(_, p)| p == "/mcp") || has("POST", "/mcp"),
        "/mcp gibt es nur als POST des Servers – die Nachbauten (mock.rs) zählen nicht mit"
    );
    assert!(
        routes.iter().all(|(m, p)| (m == "GET" || m == "POST") && p.starts_with('/')),
        "nur GET und POST, jedes Muster beginnt mit /"
    );
    let mut sorted = routes.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(sorted, routes, "sortiert und ohne Doppel – Tests können die Liste direkt vergleichen");
}

#[test]
fn rpc_method_candidates_contain_every_documented_method() {
    let candidates = rpc_method_candidates();
    let row = regex::Regex::new(r"^\| `([a-z][a-z_]*\.[a-z][a-z_]*)`").unwrap();
    let doc = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/RPC.md")).unwrap();
    let documented: Vec<&str> =
        doc.lines().filter_map(|line| row.captures(line)).map(|c| c.get(1).unwrap().as_str()).collect();
    assert!(documented.len() >= 200, "docs/RPC.md nennt über 200 Methoden als Tabellenzeile: {}", documented.len());
    let missing: Vec<&&str> = documented.iter().filter(|m| !candidates.iter().any(|c| c == **m)).collect();
    assert!(missing.is_empty(), "dokumentierte Methoden, die rpc_method_candidates nicht findet: {missing:?}");
    for method in ["work.pdf", "work.epub"] {
        assert!(candidates.iter().any(|c| c == method), "{method} ist eine Methode, kein Dateiname");
    }
    for method in ["account.me", "member.grant", "revision.propose", "finding.add"] {
        assert!(
            candidates.iter().any(|c| c == method),
            "{method}: die Methoden der Runde 35 stehen schon im Quelltext"
        );
    }
    for file in ["base.html", "baruch.sqlite", "consent.html"] {
        assert!(!candidates.iter().any(|c| c == file), "{file} ist ein Dateiname und keine Methode");
    }
}

#[tokio::test]
async fn with_who_decides_who_asks_without_a_session() {
    use baruch_server::auth::Who;
    let server = open().await;
    let anonymous = with_who(Request::get("/werke?x=1&y=2").body(Body::empty()).unwrap(), Who::Anonymous);
    let (status, headers, _) = server.send(anonymous).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "ohne Anmeldung leitet eine Seite zur Anmeldung um");
    assert_eq!(
        headers.get(header::LOCATION).unwrap(),
        "/anmelden?next=/werke%3Fx%3D1%26y%3D2",
        "das Ziel steht prozentkodiert in next, damit ? und & die Adresse nicht zerteilen"
    );
    let rpc_call = Request::post("/rpc")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({ "jsonrpc": "2.0", "id": 1, "method": "work.list" }).to_string()))
        .unwrap();
    let (status, _, body) = server.send(with_who(rpc_call, Who::Anonymous)).await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "/rpc ohne Anmeldung: 401 statt Umleitung (Skripte folgen keiner Seite)"
    );
    let answer: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(answer["error"]["code"], baruch_server::rpc::UNAUTHENTICATED, "JSON-RPC-Fehler „nicht angemeldet“");
    let form = Request::post("/act")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from("method=post.add&body=x"))
        .unwrap();
    let (status, _, _) = server.send(with_who(form, Who::Anonymous)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "ein Formular ohne Anmeldung wird abgewiesen, nichts geschrieben");
    let style = with_who(Request::get("/static/app.css").body(Body::empty()).unwrap(), Who::Anonymous);
    assert_eq!(server.send(style).await.0, StatusCode::OK, "die Anmeldeseite braucht ihr Stylesheet ohne Anmeldung");
    let open_page = Request::get("/werke").body(Body::empty()).unwrap();
    assert_eq!(server.send(open_page).await.0, StatusCode::OK, "ohne Konten und ohne Who bleibt alles offen");
    let local = with_who(Request::get("/werke").body(Body::empty()).unwrap(), Who::LocalAuthor);
    assert_eq!(server.send(local).await.0, StatusCode::OK, "volle Rechte sehen die Seite");
}
