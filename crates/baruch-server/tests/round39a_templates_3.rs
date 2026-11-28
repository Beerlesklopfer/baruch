// SPDX-License-Identifier: AGPL-3.0-or-later
//! Round 39a, task templates_3: the settings pages, the account pages (sign in, set up, my account, contributors),
//! the system pages (about, quit, sync, consent) and the pages of sources, literature and web evidence take their
//! texts from the catalogs `settings`, `accounts`, `system` and `sources` (contract R39a §5, §6).
//!
//! Every page is rendered in English and in German: `<html lang>` follows, no key of the four catalogs shows up raw,
//! the English page carries none of the German texts of the four catalogs, and the German page keeps the wording it
//! had before the round (spot checks of sentences that older tests and the user know).

use crate::common;

use std::net::SocketAddr;

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode, header};
use baruch_server::auth::{self, AccountKind, Identity, Who};
use baruch_server::i18n::{self, CATALOGS, Lang};
use serde_json::{Value, json};

/// The catalogs of this task (contract §4).
const STEMS: [&str; 4] = ["settings", "accounts", "system", "sources"];

/// `(key, text)` of the four catalogs in one language, in catalog order.
fn entries(lang: &str) -> Vec<(String, String)> {
    CATALOGS
        .iter()
        .filter(|(l, stem, _)| *l == lang && STEMS.contains(stem))
        .flat_map(|(_, _, source)| i18n::catalog_entries(source))
        .collect()
}

/// Text as minijinja's auto-escaping writes it into a page.
fn esc(text: &str) -> String {
    let mut out = String::new();
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#x27;"),
            '/' => out.push_str("&#x2f;"),
            c => out.push(c),
        }
    }
    out
}

/// The 28 templates of this task (contract R39a, task templates_3).
const OWN_TEMPLATES: [&str; 28] = [
    "settings.html",
    "settings/frame.html",
    "settings/darstellung.html",
    "settings/assistenten.html",
    "settings/daten.html",
    "settings/konten.html",
    "settings/mcp.html",
    "settings/quellen.html",
    "settings/verbindungen.html",
    "settings/werk.html",
    "settings/werk_satz.html",
    "anmelden.html",
    "einrichten.html",
    "konto.html",
    "mitwirkende.html",
    "consent.html",
    "about.html",
    "shutdown.html",
    "shutdown_done.html",
    "sync.html",
    "source.html",
    "source_import.html",
    "source_multigpt.html",
    "literature.html",
    "literature_bible.html",
    "fragments/literature_bibles.html",
    "fragments/library.html",
    "websearch_evidence.html",
];

/// Source text of every template that belongs to another task and of the Rust sources (messages handed to the
/// templates belong to the tasks messages_1/messages_2): they may still write the same German words.
fn other_sources() -> String {
    fn walk(dir: &std::path::Path, root: &std::path::Path, out: &mut String) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(&path, root, out);
            } else if path.extension().is_some_and(|e| e == "html" || e == "rs") {
                let rel = path.strip_prefix(root).unwrap().to_string_lossy().replace('\\', "/");
                if !OWN_TEMPLATES.contains(&rel.as_str()) {
                    out.push_str(&std::fs::read_to_string(&path).unwrap());
                }
            }
        }
    }
    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut out = String::new();
    for dir in ["templates", "src"] {
        let root = manifest.join(dir);
        walk(&root, &root, &mut out);
    }
    out
}

/// German texts of the four catalogs that can be searched for literally: no placeable, no markup, long enough not to
/// be a common word, different from the English text, and not also written by another template or by Rust code (the
/// owning task translates it; until then the same words may still appear in German outside this task).
fn german_sentences() -> Vec<(String, String)> {
    let en: std::collections::BTreeMap<String, String> = entries("en").into_iter().collect();
    let others = other_sources();
    entries("de")
        .into_iter()
        .filter(|(key, text)| {
            !text.contains('{')
                && !text.contains('<')
                && !text.contains('\n')
                && text.chars().count() >= 16
                && en.get(key) != Some(text)
                && !others.contains(text.as_str())
        })
        .collect()
}

/// The `<main>` element of a page (the content of the page itself, without menu and ribbon of the layout, which
/// belong to the task templates_1), or the whole page if it has none.
fn main_of(page: &str) -> &str {
    match (page.find("<main"), page.rfind("</main>")) {
        (Some(start), Some(end)) if start < end => &page[start..end],
        _ => page,
    }
}

fn html_tag(page: &str) -> &str {
    let start = page.find("<html").expect("page has an <html> tag");
    &page[start..start + page[start..].find('>').unwrap() + 1]
}

fn get(uri: &str, lang: &str) -> Request<Body> {
    common::with_lang(Request::get(uri).header(header::HOST, "127.0.0.1:8765").body(Body::empty()).unwrap(), lang)
}

/// A request that comes directly from the own machine (loopback peer, own host name).
fn local(mut request: Request<Body>) -> Request<Body> {
    request.extensions_mut().insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 42000))));
    request
}

fn author() -> Who {
    Who::Account(Identity {
        id: 1,
        name: "hanna".into(),
        display: "Hanna Beispiel".into(),
        kind: AccountKind::Author,
        handle: auth::lektor_handle("hanna"),
    })
}

fn lektor() -> Who {
    Who::Account(Identity {
        id: 2,
        name: "Paul".into(),
        display: "Paul Muster".into(),
        kind: AccountKind::Lektor,
        handle: auth::lektor_handle("Paul"),
    })
}

/// Creates an account via SQL (the password hash is a fixed text that is never checked).
async fn insert_account(server: &common::Server, name: &str, kind: &str) {
    sqlx::query("INSERT INTO account (name, name_key, slug, display, kind, password) VALUES (?, ?, ?, '', ?, ?)")
        .bind(name)
        .bind(name.to_lowercase())
        .bind(name.to_lowercase())
        .bind(kind)
        .bind("$argon2id$test")
        .execute(server.state.store.pool())
        .await
        .expect("create account");
}

/// Checks one rendered page in `lang`: the language tag, no raw key, the expected text of `key`, and on an English
/// page none of the German sentences of the four catalogs.
fn check(page: &str, lang: Lang, uri: &str, key: &str) {
    assert!(
        html_tag(page).starts_with(&format!("<html lang=\"{}\"", lang.code())),
        "{uri}: <html lang> must name the page language {}: {}",
        lang.code(),
        html_tag(page)
    );
    for (raw, _) in entries("en") {
        assert!(!page.contains(&raw), "{uri} ({}): the key {raw} shows up raw – missing in the catalog?", lang.code());
    }
    let expected = esc(&i18n::t(lang, key, &[]));
    assert!(page.contains(&expected), "{uri} ({}): the page shows the text of {key}: {expected:?}", lang.code());
    if lang == Lang::En {
        for (german_key, text) in german_sentences() {
            assert!(
                !main_of(page).contains(&esc(&text)),
                "{uri}: the English page still carries the German text of {german_key}: {text:?}"
            );
        }
    }
}

async fn page(server: &common::Server, request: Request<Body>) -> (StatusCode, String) {
    let (status, _, body) = server.send(request).await;
    (status, body)
}

/// Id of the sample title „Lutherbibel 1912“ of the library.
async fn sample_title(server: &common::Server) -> i64 {
    let answer = server.rpc("library.list", json!(null)).await;
    let titles = answer["result"]["titles"].as_array().unwrap_or_else(|| panic!("the library has titles: {answer}"));
    titles.iter().find(|t| t["key"] == "luther1912").expect("the sample library has the Luther Bible")["id"]
        .as_i64()
        .unwrap()
}

#[test]
fn the_four_catalogs_exist_with_the_same_keys_and_natural_english() {
    let en = entries("en");
    let de = entries("de");
    for stem in STEMS {
        assert!(
            en.iter().any(|(key, _)| key.starts_with(&format!("{stem}."))),
            "locales/en/{stem}.ftl must exist and hold keys of the task templates_3"
        );
    }
    let en_keys: Vec<&String> = en.iter().map(|(k, _)| k).collect();
    let de_keys: Vec<&String> = de.iter().map(|(k, _)| k).collect();
    assert_eq!(en_keys, de_keys, "en and de carry the same keys in the same order (contract §4)");
    for (key, text) in &en {
        assert!(
            !text.contains('„') && !text.contains('‚'),
            "{key}: English texts use English quotation marks, not German ones: {text}"
        );
    }
    assert!(german_sentences().len() > 200, "the German texts to look for are many (the four areas are large)");
}

#[tokio::test]
async fn every_page_of_the_task_renders_in_english_and_german_without_accounts() {
    let server = common::open().await;
    let work = server.state.default_work().await.unwrap();
    let title = sample_title(&server).await;
    let pages: Vec<(String, &str)> = vec![
        ("/einstellungen/darstellung".into(), "settings.display.title"),
        ("/einstellungen/assistenten".into(), "settings.assistants.colors_help"),
        ("/einstellungen/daten".into(), "settings.data.backup_title"),
        ("/einstellungen/konten".into(), "settings.accounts.set_up"),
        ("/einstellungen/mcp".into(), "settings.mcp.tokens_title"),
        ("/einstellungen/quellen".into(), "settings.sources.new"),
        ("/einstellungen/verbindungen".into(), "settings.connections.models_title"),
        (format!("/einstellungen/werk?work={work}"), "settings.work.revisions_title"),
        (format!("/einstellungen/werk?work={work}"), "settings.typeset.reset_confirm"),
        ("/konto".into(), "accounts.mine.open"),
        (format!("/werke/{work}/mitwirkende"), "accounts.contributors.no_accounts"),
        ("/ueber".into(), "system.about.third_party"),
        ("/abgleich".into(), "system.sync.collections_title"),
        (format!("/quelle?title={title}&work={work}"), "sources.source.use_selection"),
        (format!("/literatur?work={work}"), "sources.library.search_by"),
        (format!("/literatur/bibel?work={work}"), "sources.bible.window_title_end"),
        (format!("/websuche/belege?post=1&work={work}"), "sources.evidence.none"),
    ];
    for (uri, key) in &pages {
        for lang in Lang::ALL {
            let (status, body) = page(&server, get(uri, lang.code())).await;
            assert_eq!(status, StatusCode::OK, "{uri} ({}) renders: {body}", lang.code());
            check(&body, lang, uri, key);
        }
    }
    // The settings frame: the heading of the page and the S1 form of the language stay plain forms.
    let (_, en) = page(&server, get("/einstellungen/darstellung", "en")).await;
    assert!(en.contains("<title>Settings · "), "the settings page is called Settings in English");
    assert!(
        en.contains("action=\"/einstellungen/sprache\"") && en.contains("name=\"lang\" value=\"auto\""),
        "the S1 block lang-form stays a plain form without JavaScript (contract §1.11b)"
    );
    assert!(
        en.contains("name=\"theme\" value=\"system\"") && en.contains("name=\"editor\" value=\"quelltext\""),
        "the values of the form fields stay unchanged in English (only the labels are translated)"
    );
}

#[tokio::test]
async fn the_german_pages_keep_their_wording() {
    let server = common::open().await;
    let work = server.state.default_work().await.unwrap();
    let samples: Vec<(String, Vec<&str>, Vec<&str>)> = vec![
        (
            "/einstellungen/daten".into(),
            vec!["Sicherung jetzt anlegen", "Git-Spiegel je Werk", "&lt;Name&gt;.sicherung-&lt;Datum-Uhrzeit&gt;"],
            vec!["Create backup now", "Git mirror per work", "&lt;name&gt;.sicherung-&lt;date-time&gt;"],
        ),
        (
            "/einstellungen/konten".into(),
            vec!["Baruch läuft <strong>ohne Konten</strong>: Alles ist offen wie bisher"],
            vec!["Baruch runs <strong>without accounts</strong>: everything is open as before"],
        ),
        (
            "/einstellungen/mcp".into(),
            vec!["Tokens für andere Rechner", "Noch kein Token erzeugt.", "<code>@mcp:&lt;name&gt;</code>"],
            vec!["Tokens for other computers", "No token created yet.", "<code>@mcp:&lt;name&gt;</code>"],
        ),
        (
            "/einstellungen/darstellung".into(),
            vec!["> Wie das System</label>", "Spaltenbreiten: links Vorgabe, rechts Vorgabe."],
            vec!["> Like the system</label>", "Column widths: left default, right default."],
        ),
        (
            format!("/einstellungen/werk?work={work}"),
            vec!["Zurzeit haben 0 Abschnitte eine Marke.", "Ältere Fassungen ausblenden"],
            vec!["Currently 0 sections have a mark.", "Hide older versions"],
        ),
        (
            "/ueber".into(),
            vec!["Baruch ist eine freie Schreibwerkstatt für Bücher und Predigten.", "Bibliotheken Dritter"],
            vec!["Baruch is a free writing workshop for books and sermons.", "Third-party libraries"],
        ),
        (
            "/abgleich".into(),
            vec!["Noch nie abgeglichen.", "Abgleich mit MultiGPT"],
            vec!["Never synced.", "Sync with MultiGPT"],
        ),
    ];
    for (uri, german, english) in &samples {
        let (_, de) = page(&server, get(uri, "de")).await;
        let (_, en) = page(&server, get(uri, "en")).await;
        for text in german {
            assert!(de.contains(text), "{uri}: the German page keeps the wording {text:?} of before Round 39a");
            assert!(
                !main_of(&en).contains(text) || english.contains(text),
                "{uri}: {text:?} is German and not in the content of the English page"
            );
        }
        for text in english {
            assert!(en.contains(text), "{uri}: the English page says {text:?}");
        }
    }
}

#[tokio::test]
async fn setting_up_and_quitting_render_for_the_own_machine_and_for_others() {
    let server = common::open().await;
    for lang in Lang::ALL {
        let (status, body) = page(&server, local(get("/einrichten", lang.code()))).await;
        assert_eq!(status, StatusCode::OK, "/einrichten from the own machine shows the form ({})", lang.code());
        check(&body, lang, "/einrichten", "accounts.setup.create");
        let password = esc(&format!("baruch-server passwort <{}>", i18n::t(lang, "accounts.cmd_name", &[])));
        assert!(body.contains(&password), "the command line stays readable with its placeholder: {password}");
        let (status, body) = page(&server, get("/einrichten", lang.code())).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "/einrichten from elsewhere only explains ({})", lang.code());
        check(&body, lang, "/einrichten (remote)", "accounts.setup.local_only");

        let (status, body) = page(&server, local(get("/beenden", lang.code()))).await;
        assert_eq!(status, StatusCode::OK, "/beenden from the own machine asks ({})", lang.code());
        check(&body, lang, "/beenden", "system.shutdown.question");
        let (status, body) = page(&server, get("/beenden", lang.code())).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "/beenden from elsewhere only explains ({})", lang.code());
        check(&body, lang, "/beenden (remote)", "system.shutdown.local_only");
    }
}

#[tokio::test]
async fn with_accounts_the_account_pages_render_in_both_languages() {
    let server = common::open().await;
    let work = server.state.default_work().await.unwrap();
    insert_account(&server, "hanna", "author").await;
    insert_account(&server, "Paul", "lektor").await;
    let cases: Vec<(String, Who, &str)> = vec![
        ("/einstellungen/konten".into(), author(), "settings.accounts.add_lector"),
        ("/konto".into(), author(), "accounts.mine.change_password"),
        (format!("/werke/{work}/mitwirkende"), author(), "accounts.contributors.help"),
        ("/konto".into(), lektor(), "accounts.mine.lector_help"),
        ("/einstellungen/darstellung".into(), lektor(), "settings.display.cookies_html"),
    ];
    for (uri, who, key) in &cases {
        for lang in Lang::ALL {
            let (status, body) = page(&server, common::with_who(get(uri, lang.code()), who.clone())).await;
            assert_eq!(status, StatusCode::OK, "{uri} ({}) renders for the account: {body}", lang.code());
            if key.ends_with("_html") {
                let text = i18n::t(lang, key, &[]);
                let start: String = text.chars().take(20).collect();
                assert!(body.contains(&esc(&start)), "{uri} ({}): the page shows {key}", lang.code());
                check(&body, lang, uri, "settings.display.title");
            } else {
                check(&body, lang, uri, key);
            }
        }
    }
    // Signing in exists only with accounts (without them /anmelden leads to the start page).
    for lang in Lang::ALL {
        let (status, body) = page(&server, get("/anmelden", lang.code())).await;
        assert_eq!(status, StatusCode::OK, "/anmelden shows the form when accounts exist ({})", lang.code());
        check(&body, lang, "/anmelden", "accounts.sign_in.already_help");
    }
    let (_, de) = page(&server, get("/anmelden", "de")).await;
    assert!(de.contains("<title>Anmelden – Baruch</title>"), "the German sign-in page keeps its title");
    let (_, en) = page(&server, get("/anmelden", "en")).await;
    assert!(en.contains("<title>Sign in – Baruch</title>"), "the English sign-in page is called Sign in");
    let (_, de) = page(&server, common::with_who(get("/einstellungen/konten", "de"), author())).await;
    assert!(
        de.contains("role=\"note\"><strong>Vorsicht mit einem Proxy auf demselben Rechner:</strong>"),
        "the German warning about a proxy keeps its wording and markup"
    );
    let (_, en) = page(&server, common::with_who(get("/einstellungen/konten", "en"), author())).await;
    assert!(
        en.contains("role=\"note\"><strong>Careful with a proxy on the same computer:</strong>"),
        "the English warning about a proxy has the same markup"
    );
}

/// An MCP client requests a part under which existing chapters would move: a request that needs consent.
async fn request_needing_consent(server: &common::Server) {
    let report = server
        .import("= Saat\n\nKörner im Boden.\n\n= Wuchs\n\nHalme im Wind.\n\n= Ernte\n\nGarben am Weg.\n", "feld.typ")
        .await;
    let loaded = server.state.store.load_work(report.work).await.unwrap();
    let key = loaded.nodes.iter().find(|n| n.title == "Wuchs").unwrap().key;
    let post = |session: Option<String>, message: Value| {
        let mut build = Request::post("/mcp").header(header::CONTENT_TYPE, "application/json");
        if let Some(id) = session {
            build = build.header("mcp-session-id", id);
        }
        local(build.body(Body::from(message.to_string())).unwrap())
    };
    let init = json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": { "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": { "name": "Feldbuch", "version": "1.0" } } });
    let (status, headers, body) = server.send(post(None, init)).await;
    assert_eq!(status, StatusCode::OK, "MCP initialize: {body}");
    let session = headers.get("mcp-session-id").unwrap().to_str().unwrap().to_owned();
    let call = json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": { "name": "outline_add",
        "arguments": { "work": report.work, "key": key, "place": "before", "title": "Vom Wachsen", "level": 0 } } });
    let (_, _, body) = server.send(post(Some(session), call)).await;
    let answer: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(answer["result"]["structuredContent"]["action"], "requested", "a request, not a plain add: {answer}");
}

#[tokio::test]
async fn the_consent_bar_and_dialog_follow_the_page_language() {
    let server = common::open().await;
    request_needing_consent(&server).await;
    for lang in Lang::ALL {
        let (status, body) = page(&server, get("/werke", lang.code())).await;
        assert_eq!(status, StatusCode::OK, "/werke renders ({})", lang.code());
        assert!(body.contains("class=\"consent-bar\""), "the open request brings the consent bar ({})", lang.code());
        check(&body, lang, "/werke (consent)", "system.consent.title");
        let later = esc(&i18n::t(lang, "system.consent.later", &[]));
        assert!(body.contains(&later), "the dialog button „later“ is in the page language: {later}");
    }
    let (_, de) = page(&server, get("/werke", "de")).await;
    assert!(de.contains("<strong>Zustimmung nötig:</strong>"), "the German bar keeps its wording");
    let (_, en) = page(&server, get("/werke", "en")).await;
    assert!(
        !en.contains("Zustimmung zur Gliederung") && !en.contains("Später entscheiden"),
        "the consent bar and dialog (outside <main>) carry no German on the English page"
    );
}
