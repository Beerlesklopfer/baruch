// SPDX-License-Identifier: AGPL-3.0-or-later
//! Round 39a, task messages_2: server messages of settings, connections, accounts/roles, system, sync, web search,
//! import and the messages of the store (`src/store_text.rs`) in English and German.
//!
//! Each case is triggered over the router in both languages: English must not contain the German wording, German
//! must be the wording from before Round 39a (the moved assertions of the older tests stay German through the pin of
//! `common::router`). The store messages are checked against the sources of `baruch-store`: every `Refused("…")`
//! literal and every message constant passed to `Refused` has a `store_msg.*` pattern whose German text is the
//! store's wording verbatim. `/mcp` answers store refusals in English, whatever the request asks for.

use crate::common;

use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode, header};
use baruch_server::i18n::{self, Lang};
use baruch_server::store_text;
use baruch_store::StoreError;
use regex::Regex;
use serde_json::{Value, json};

// ---- helpers ---------------------------------------------------------------------------------------------------

fn manifest() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn get(uri: &str, lang: &str) -> Request<Body> {
    common::with_lang(Request::get(uri).body(Body::empty()).unwrap(), lang)
}

fn form(uri: &str, body: &str, lang: &str) -> Request<Body> {
    let request = Request::post(uri)
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(body.to_owned()))
        .unwrap();
    common::with_lang(request, lang)
}

/// JSON-RPC call in the language `lang`; returns the whole answer.
async fn rpc(server: &common::Server, method: &str, params: Value, lang: &str) -> Value {
    let body = json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params });
    let request = Request::post("/rpc")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    let (_, _, text) = server.send(common::with_lang(request, lang)).await;
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{method}: no JSON ({e}): {text}"))
}

fn error_message(answer: &Value) -> String {
    answer["error"]["message"].as_str().unwrap_or_else(|| panic!("an error answer expected: {answer}")).to_owned()
}

/// A German umlaut, sharp s or the German opening quotation mark – never part of an English message of these modules.
fn looks_german(text: &str) -> bool {
    text.chars().any(|c| "äöüÄÖÜß„".contains(c))
}

// ---- triggered messages in both languages ------------------------------------------------------------------------

#[tokio::test]
async fn a_settings_notice_follows_the_language_of_the_request() {
    let server = common::open().await;
    let (_, _, en) = server.send(get("/einstellungen/assistenten?ok=farbe", "en")).await;
    assert!(en.contains("The colour is saved."), "?ok=farbe shows the English notice in an English browser");
    assert!(!en.contains("Die Farbe ist gespeichert."), "the German notice must not leak into the English page");
    let (_, _, de) = server.send(get("/einstellungen/assistenten?ok=farbe", "de")).await;
    assert!(de.contains("Die Farbe ist gespeichert."), "German keeps the wording from before Round 39a");
}

#[tokio::test]
async fn a_form_error_of_the_settings_follows_the_language() {
    let server = common::open().await;
    let (status, _, en) = server.send(form("/einstellungen/darstellung/act", "theme=bunt", "en")).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "an unknown theme is refused with 422: {en}");
    assert!(
        en.contains("Theme: choose “System”, “Light” or “Dark”."),
        "the form error is English in an English browser"
    );
    let (_, _, de) = server.send(form("/einstellungen/darstellung/act", "theme=bunt", "de")).await;
    assert!(de.contains("Thema: „System“, „Hell“ oder „Dunkel“ wählen."), "German wording unchanged");
}

#[tokio::test]
async fn a_failed_sign_in_is_refused_in_the_language_of_the_browser() {
    let server = common::open().await;
    let new = baruch_store::NewAccount {
        name: "erika",
        slug: "erika",
        display: "Erika",
        kind: "author",
        password_phc: "$argon2id$v=19$m=19456,t=2,p=1$c2FsdA$aGFzaA",
    };
    server.state.store.account_create_first(&new).await.unwrap();
    let (status, _, en) = server.send(form("/anmelden", "name=erika&password=falsch&next=%2F", "en")).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "a wrong password is 401: {en}");
    assert!(en.contains("Name or password is wrong."), "the refusal is English for an English browser");
    assert!(!en.contains(baruch_server::auth::routes::LOGIN_FAILED), "no German sentence in the English page");
    let (_, _, de) = server.send(form("/anmelden", "name=erika&password=falsch&next=%2F", "de")).await;
    assert!(de.contains(baruch_server::auth::routes::LOGIN_FAILED), "German keeps the constant's wording");
}

#[tokio::test]
async fn an_import_error_over_rpc_follows_the_language() {
    let server = common::open().await;
    let en = error_message(&rpc(&server, "docx_import.discard", json!({ "draft": "kaputt" }), "en").await);
    assert_eq!(en, "draft: 32 lower-case hex characters (from docx_import.preview).", "English refusal of the import");
    let de = error_message(&rpc(&server, "docx_import.discard", json!({ "draft": "kaputt" }), "de").await);
    assert_eq!(de, "draft: 32 Kleinbuchstaben-Hex (aus docx_import.preview).", "German wording unchanged");
}

#[tokio::test]
async fn a_store_refusal_over_rpc_follows_the_language_of_the_request() {
    let server = common::open().await;
    let params = json!({ "id": 999_999, "title": "Quelle", "url": "https://a.example/", "note": "" });
    let en = error_message(&rpc(&server, "sources.update", params.clone(), "en").await);
    assert_eq!(en, "This source does not exist (any more).", "Refused of the store in English for /rpc");
    let de = error_message(&rpc(&server, "sources.update", params, "de").await);
    assert_eq!(de, "Diese Quelle gibt es nicht (mehr).", "the store's own German wording for a German browser");
    let en = error_message(&rpc(&server, "idea.get", json!({ "id": 999_999 }), "en").await);
    assert_eq!(en, "Idea 999999 does not exist.", "a variant with a fixed wording (UnknownIdea) in English");
    let de = error_message(&rpc(&server, "idea.get", json!({ "id": 999_999 }), "de").await);
    assert_eq!(de, "Einfall 999999 gibt es nicht.", "and in German as the store writes it");
}

#[tokio::test]
async fn mcp_answers_the_same_store_refusal_in_english_even_for_a_german_browser() {
    let server = common::open().await;
    let call = json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/call",
        "params": { "name": "idea_read", "arguments": { "id": 999_999 } } });
    let mut request = Request::post("/mcp")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(call.to_string()))
        .unwrap();
    request.extensions_mut().insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 40000))));
    let (_, _, body) = server.send(common::with_lang(request, "de")).await;
    assert!(body.contains("Idea 999999 does not exist."), "MCP is English (contract §3.3): {body}");
    assert!(!body.contains("gibt es nicht"), "no German store text over MCP: {body}");
}

#[tokio::test]
async fn json_rpc_protocol_errors_stay_english_in_a_german_browser() {
    let server = common::open().await;
    let answer = rpc(&server, "gibt.es.nicht", json!({}), "de").await;
    let message = error_message(&answer);
    assert!(!looks_german(&message), "protocol errors stay English as since Round 38: {message}");
}

#[tokio::test]
async fn the_guard_refusal_of_a_foreign_page_follows_the_language() {
    let server = common::open().await;
    let foreign = |lang: &str| {
        let mut request = form("/einstellungen/darstellung/act", "theme=dark", lang);
        request.headers_mut().insert(header::ORIGIN, "http://boese.example".parse().unwrap());
        request.headers_mut().insert(header::HOST, "127.0.0.1:8765".parse().unwrap());
        request
    };
    let (status, _, en) = server.send(foreign("en")).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "a foreign Origin is refused: {en}");
    assert_eq!(en, "Refused: request from a foreign page (Origin does not match Host)", "English refusal");
    let (_, _, de) = server.send(foreign("de")).await;
    assert_eq!(de, "Abgelehnt: Anfrage von einer fremden Seite (Origin passt nicht zu Host)", "German as before");
}

#[tokio::test]
async fn sample_pages_of_the_owned_areas_show_no_raw_keys_and_no_german_messages_in_english() {
    let server = common::open().await;
    for uri in ["/einstellungen/verbindungen?ok=gespeichert", "/abgleich?ok=gestartet", "/einstellungen/werk?ok=werk"] {
        let (status, _, page) = server.send(get(uri, "en")).await;
        assert_eq!(status, StatusCode::OK, "{uri} answers");
        for stem in ["settings_msg.", "connections_msg.", "sync_msg.", "system_msg.", "accounts_msg."] {
            assert!(!page.contains(stem), "{uri}: no raw key of {stem} – every key has an English text");
        }
    }
    let (_, _, page) = server.send(get("/einstellungen/verbindungen?ok=gespeichert", "en")).await;
    assert!(page.contains("The settings are saved."), "the connections notice is English");
    let (_, _, page) = server.send(get("/abgleich?ok=gestartet", "en")).await;
    assert!(page.contains("The synchronisation runs in the background"), "the sync notice is English");
}

// ---- the messages of the store -----------------------------------------------------------------------------------

/// Runtime text of a Rust string literal starting at `src[i] == '"'` (line continuations and simple escapes).
fn rust_string(src: &str, i: usize) -> (String, usize) {
    let bytes: Vec<char> = src[i + 1..].chars().collect();
    let mut out = String::new();
    let mut j = 0;
    let mut consumed = 1;
    while bytes[j] != '"' {
        if bytes[j] == '\\' {
            let next = bytes[j + 1];
            consumed += bytes[j].len_utf8() + next.len_utf8();
            j += 2;
            match next {
                '\n' => {
                    while bytes[j].is_whitespace() {
                        consumed += bytes[j].len_utf8();
                        j += 1;
                    }
                }
                'n' => out.push('\n'),
                '"' => out.push('"'),
                '\\' => out.push('\\'),
                other => {
                    out.push('\\');
                    out.push(other);
                }
            }
            continue;
        }
        out.push(bytes[j]);
        consumed += bytes[j].len_utf8();
        j += 1;
    }
    (out, i + consumed + 1)
}

/// Source of a store file without its test module and without comment lines.
fn store_source(path: &Path) -> String {
    let text = std::fs::read_to_string(path).unwrap();
    let text = text.split("#[cfg(test)]").next().unwrap_or("");
    text.lines().filter(|line| !line.trim_start().starts_with("//")).collect::<Vec<_>>().join("\n")
}

/// Every message the store refuses with: `Refused("…")` literals and the constants passed to `Refused(…)`.
fn store_refusals() -> Vec<(String, String)> {
    let dir = manifest().join("../baruch-store/src");
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "rs"))
        .collect();
    files.sort();
    let constant = Regex::new(r#"const ([A-Z_0-9]+): &(?:'static )?str =\s*""#).unwrap();
    let mut constants: BTreeMap<String, String> = BTreeMap::new();
    let mut found = Vec::new();
    let mut names = Vec::new();
    for path in &files {
        let src = store_source(path);
        for m in constant.captures_iter(&src) {
            let start = m.get(0).unwrap().end() - 1;
            constants.insert(m[1].to_owned(), rust_string(&src, start).0);
        }
        let shown = path.file_name().unwrap().to_string_lossy().into_owned();
        let mut rest = 0;
        while let Some(at) = src[rest..].find("Refused(") {
            let mut i = rest + at + "Refused(".len();
            let mut depth = 1;
            let mut ident = String::new();
            while depth > 0 && i < src.len() {
                let c = src[i..].chars().next().unwrap();
                if c.is_ascii_uppercase() || c == '_' || (c.is_ascii_digit() && !ident.is_empty()) {
                    ident.push(c);
                    i += 1;
                    continue;
                }
                // Any other character ends a constant name (`Refused(Self::ACCOUNT_TAKEN)`, `Refused(UNKNOWN)`).
                if ident.len() > 2 && !c.is_ascii_lowercase() {
                    names.push((shown.clone(), ident.clone()));
                }
                ident.clear();
                match c {
                    '"' => {
                        let (text, end) = rust_string(&src, i);
                        found.push((shown.clone(), text));
                        i = end;
                        continue;
                    }
                    '(' => depth += 1,
                    ')' => depth -= 1,
                    _ => {}
                }
                i += c.len_utf8();
            }
            rest = i;
        }
    }
    for (file, name) in names {
        if let Some(text) = constants.get(&name) {
            found.push((file, text.clone()));
        }
    }
    found.sort();
    found.dedup();
    found
}

#[test]
fn every_refusal_of_the_store_has_a_store_msg_pattern_with_its_german_wording() {
    let refusals = store_refusals();
    assert!(refusals.len() > 100, "the scan must find the refusals of the store, found only {}", refusals.len());
    let mut missing = Vec::new();
    for (file, text) in &refusals {
        match store_text::pattern_key(text) {
            None => missing.push(format!("{file}: {text}")),
            Some(key) => {
                assert!(key.starts_with("store_msg."), "{text}: the pattern belongs to the store catalog ({key})");
                assert_eq!(&store_text::translate(Lang::De, text), text, "{file}: German stays the store's wording");
                let english = store_text::translate(Lang::En, text);
                assert!(!looks_german(&english), "{file}: {key} has a German-looking English text: {english}");
            }
        }
    }
    assert!(missing.is_empty(), "refusals of baruch-store without a store_msg pattern: {missing:#?}");
}

#[test]
fn the_variants_with_a_fixed_wording_are_translated_and_german_equals_display() {
    let samples = [
        StoreError::UnknownWork(7),
        StoreError::UnknownNode { work: 7, node: 3 },
        StoreError::UnknownRevision { node: 1, rev: 4, existing: vec![1, 2] },
        StoreError::EmptyPost,
        StoreError::AlreadyImported { work: 1, slug: "buch".into(), title: "Buch".into() },
        StoreError::DuplicateTitle { work: 1, slug: "buch".into(), title: "Buch".into() },
        StoreError::EmptyTitle,
        StoreError::GoneNode { work: 1, key: 9 },
        StoreError::UnknownAsset { work: 1, name: "a.png".into() },
        StoreError::UnknownPersona("@x".into()),
        StoreError::UnknownIdea(5),
        StoreError::UnknownPost { work: 1, post: 2 },
        StoreError::UnknownRequest { work: 1, id: 2 },
        StoreError::UnknownFinding { work: 1, id: 2 },
        StoreError::UnknownGlossaryTerm { work: 1, id: 2 },
        StoreError::UnknownIndexProposal { work: 1, id: 2 },
        StoreError::Stale("Veraltet: Der Gliederungspunkt heißt schon so.".into()),
        StoreError::Title("Der Titel darf keinen Zeilenumbruch enthalten.".into()),
        StoreError::PostEdit("Der Text ist unverändert; nichts geändert.".into()),
    ];
    for e in &samples {
        assert_eq!(
            store_text::store_error_text(Lang::De, e),
            e.to_string(),
            "German equals the store's Display: {e:?}"
        );
        let english = store_text::store_error_text(Lang::En, e);
        assert_ne!(english, e.to_string(), "English differs from the German wording: {e:?}");
        assert!(!looks_german(&english) && !english.contains("gibt es"), "English has no German: {english}");
    }
}

#[test]
fn every_message_key_used_in_the_owned_sources_exists_in_the_catalog() {
    let quoted =
        Regex::new(r#""((?:settings|connections|accounts|system|sync|websearch|import|store)_msg\.[a-z0-9_.]+)""#)
            .unwrap();
    let mut files = Vec::new();
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(&path, out);
            } else if path.extension().is_some_and(|e| e == "rs") {
                out.push(path);
            }
        }
    }
    walk(&manifest().join("src"), &mut files);
    let mut unknown = Vec::new();
    for path in files {
        let text = std::fs::read_to_string(&path).unwrap();
        for m in quoted.captures_iter(&text) {
            if !i18n::exists(&m[1]) {
                unknown.push(format!("{}: {}", path.display(), &m[1]));
            }
        }
    }
    assert!(
        unknown.is_empty(),
        "keys in src without an entry in the en catalog (a typo shows the raw key): {unknown:#?}"
    );
}
