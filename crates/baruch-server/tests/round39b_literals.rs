// SPDX-License-Identifier: AGPL-3.0-or-later
//! Round 39b „User interface German and English, outputs and help“, owner rest_literals: the rest of the interface
//! literals, the help link per interface language (contract §2.3).
//!
//! Checks: (1) completeness of the interface catalogs per area file (same keys and placeables in en and de, the
//! English catalog without German letters except a listed exception with its reason); (2) the language switch on the
//! sign-in and setup pages and the way back of the menu form (open low findings of Round 39a); (3) labels of other
//! crates (`baruch-typst`, `baruch-store`, `baruch-epub`) in English on English pages and verbatim on German pages;
//! (4) the default watermark of the settings form in the work language; (5) the English word "Revisions". The help
//! link per language is checked in `round37_bilder`, the sheet count in the unit tests of `src/cover.rs`.

use crate::common;

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode, header};
use baruch_server::i18n::{self, CATALOGS, Lang};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use std::net::SocketAddr;

/// German letters and quotation marks that must not appear in an English text.
const GERMAN: [char; 9] = ['ä', 'ö', 'ü', 'Ä', 'Ö', 'Ü', 'ß', '„', '‚'];

/// English catalog entries that quote German on purpose, each with its reason. An entry that no longer carries German
/// is allowed (the owner of the file may have changed it meanwhile); a key that does not exist fails.
const ENGLISH_EXCEPTIONS: &[(&str, &str)] = &[
    ("export.word.styles", "names the Word style names the exported file really contains (Überschrift 1–3)"),
    (
        "export.epub_preview.kindle_html",
        "quotes the German reader profile names; export.ftl belongs to ausgaben, the names read „via EPUB“ in English \
         since Round 39b (to align)",
    ),
    ("settings.mcp.name_rules_html", "shows how an umlaut is transcribed in an author name (ü → ue)"),
    ("works.delete.mirror_html", "quotes the commit text the Git mirror writes (mirror texts stay German)"),
];

/// Area file → (key → raw value) of one language.
fn areas(lang: &str) -> BTreeMap<String, Vec<(String, String)>> {
    CATALOGS
        .iter()
        .filter(|(code, _, _)| *code == lang)
        .map(|(_, stem, source)| (stem.to_string(), i18n::catalog_entries(source)))
        .collect()
}

fn get(uri: &str, lang: &str) -> Request<Body> {
    common::with_lang(Request::get(uri).header(header::HOST, "127.0.0.1:8765").body(Body::empty()).unwrap(), lang)
}

/// A request that comes directly from the own machine (loopback peer).
fn local(mut request: Request<Body>) -> Request<Body> {
    request.extensions_mut().insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 42000))));
    request
}

/// The `<form …>…</form>` of the language switch in a page (the first form posting to the switch).
fn switch_form(page: &str) -> &str {
    let start = page.find("action=\"/einstellungen/sprache\"").expect("the page has the language switch");
    let start = page[..start].rfind("<form").expect("the switch is a form");
    let end = page[start..].find("</form>").expect("the form is closed") + start;
    &page[start..end]
}

// ---- (1) catalog completeness ----------------------------------------------------------------------------------

#[test]
fn every_area_file_has_the_same_keys_and_placeables_in_both_languages() {
    let (en, de) = (areas("en"), areas("de"));
    assert_eq!(
        en.keys().collect::<Vec<_>>(),
        de.keys().collect::<Vec<_>>(),
        "every area file exists in both languages: a missing German file would show English on German pages"
    );
    for (stem, entries) in &en {
        let german: BTreeMap<&str, &str> = de[stem].iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
        let keys: Vec<&str> = entries.iter().map(|(k, _)| k.as_str()).collect();
        let german_keys: Vec<&str> = de[stem].iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(
            keys, german_keys,
            "{stem}.ftl: the same keys in the same order, so a diff of both files stays readable"
        );
        for (key, value) in entries {
            assert!(
                key.starts_with(&format!("{stem}.")),
                "{key}: the first segment names its area file {stem}.ftl (keys by area)"
            );
            let want: BTreeSet<String> = i18n::placeables(value).into_iter().collect();
            let got: BTreeSet<String> = i18n::placeables(german[key.as_str()]).into_iter().collect();
            assert_eq!(want, got, "{key}: both languages fill the same placeholders, otherwise a value is lost");
            assert!(!german[key.as_str()].trim().is_empty(), "{key}: the German text is not empty");
        }
    }
}

#[test]
fn the_english_catalog_carries_no_german_except_the_listed_exceptions() {
    let en = areas("en");
    let all: BTreeMap<&str, &str> = en.values().flatten().map(|(key, value)| (key.as_str(), value.as_str())).collect();
    for (key, reason) in ENGLISH_EXCEPTIONS {
        assert!(all.contains_key(key), "exception {key} ({reason}) names a key of the English catalog");
    }
    let mut found = Vec::new();
    for (key, value) in &all {
        if value.contains(GERMAN) && !ENGLISH_EXCEPTIONS.iter().any(|(k, _)| k == key) {
            found.push(format!("{key} = {value}"));
        }
    }
    assert!(
        found.is_empty(),
        "English catalog texts with German letters (translate them or list them with a reason):\n{}",
        found.join("\n")
    );
}

#[test]
fn the_new_area_file_of_the_reader_profiles_is_complete() {
    let en = areas("en");
    let readers = en.get("readers").expect("readers.ftl exists (Round 39b, reader profiles of baruch-epub)");
    assert_eq!(readers.len(), 27, "nine profiles with name, device and note each (baruch_epub::readers::PROFILES)");
    for profile in baruch_epub::readers::PROFILES {
        let key = format!("readers.{}.name", profile.slug.replace('-', "_"));
        assert!(i18n::exists(&key), "{key}: every profile of baruch-epub has a translated name");
    }
}

#[test]
fn every_entry_of_the_literal_allow_list_stands_under_a_reason() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../scripts/i18n-literal-allow.txt");
    let text = std::fs::read_to_string(path).expect("scripts/i18n-literal-allow.txt exists");
    let mut reason: Option<&str> = None;
    let mut entries = 0;
    for line in text.lines() {
        if line.trim().is_empty() {
            // A blank line ends a group: the next entries need a reason of their own.
            reason = None;
        } else if let Some(comment) = line.strip_prefix("# reason: ") {
            assert!(comment.len() > 20, "a reason says why the literal is no interface text: {line:?}");
            reason = Some(comment);
        } else if !line.starts_with('#') {
            entries += 1;
            assert!(
                reason.is_some(),
                "{line:?} has no \"# reason:\" comment above it (contract R39b: only justified entries remain)"
            );
            assert!(line.contains('\t'), "an entry is <path><TAB><literal>: {line:?}");
        }
    }
    assert!(entries > 0, "the list still has entries (identifiers the finder sees)");
}

// ---- (2) language switch ---------------------------------------------------------------------------------------

#[tokio::test]
async fn the_setup_page_offers_the_language_switch_as_a_plain_form() {
    let server = common::open().await;
    for lang in Lang::ALL {
        let (status, _, page) = server.send(local(get("/einrichten", lang.code()))).await;
        assert_eq!(status, StatusCode::OK, "/einrichten from the own machine renders ({})", lang.code());
        let form = switch_form(&page);
        assert!(form.contains("method=\"post\""), "the switch posts (it stores the choice): {form}");
        assert!(
            form.contains("<input type=\"hidden\" name=\"back\" value=\"/einrichten\">"),
            "the switch leads back to the setup page: {form}"
        );
        assert!(
            form.contains("name=\"lang\" value=\"en\"") && form.contains("name=\"lang\" value=\"de\""),
            "both languages are offered as buttons: {form}"
        );
        assert!(!form.contains("<script") && !form.contains(" on"), "no JavaScript needed: {form}");
    }
}

#[tokio::test]
async fn the_sign_in_page_offers_the_language_switch_and_leads_back_to_itself() {
    let server = common::open().await;
    let new = baruch_store::NewAccount {
        name: "erika",
        slug: "erika",
        display: "Erika",
        kind: "author",
        password_phc: "$argon2id$v=19$m=19456,t=2,p=1$c2FsdA$aGFzaA",
    };
    server.state.store.account_create_first(&new).await.unwrap();
    for lang in Lang::ALL {
        let (status, _, page) = server.send(get("/anmelden?next=%2Fwerke", lang.code())).await;
        assert_eq!(status, StatusCode::OK, "/anmelden renders with accounts ({})", lang.code());
        let form = switch_form(&page);
        assert!(
            form.contains("<input type=\"hidden\" name=\"back\" value=\"/anmelden?next=&#x2f;werke\">"),
            "the switch leads back to this sign-in page with its target (the template writes / as &#x2f;, the browser \
             reads it as /): {form}"
        );
        assert!(
            form.contains(&format!("value=\"{}\" lang=\"{}\" aria-current=\"true\"", lang.code(), lang.code())),
            "the current language is marked: {form}"
        );
    }
    let anonymous = common::with_who(
        Request::post("/einstellungen/sprache")
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(Body::from("lang=en&back=%2Fanmelden%3Fnext%3D%2Fwerke"))
            .unwrap(),
        baruch_server::auth::Who::Anonymous,
    );
    let (status, headers, _) = server.send(anonymous).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "a visitor who is not signed in may switch (public route)");
    assert_eq!(
        headers.get(header::LOCATION).and_then(|v| v.to_str().ok()),
        Some("/anmelden?next=/werke"),
        "and lands on the sign-in page again, with the target kept"
    );
}

#[tokio::test]
async fn the_menu_form_sends_the_current_page_as_way_back() {
    let server = common::open().await;
    let (_, _, page) = server.send(get("/werke?work=1&q=Turm", "de")).await;
    let form = switch_form(&page);
    assert!(
        form.contains("<input type=\"hidden\" name=\"back\" value=\"/werke?work=1\">"),
        "the menu form names the page it stands on (finding R39a: it relied on the Referer), with the place only – \
         text from the address (a search) never goes back into the page: {form}"
    );
    let request = Request::post("/einstellungen/sprache")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from("lang=en&back=%2Fwerke%3Fwork%3D1"))
        .unwrap();
    let (status, headers, _) = server.send(request).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "the switch answers with a redirect");
    assert_eq!(
        headers.get(header::LOCATION).and_then(|v| v.to_str().ok()),
        Some("/werke?work=1"),
        "without a Referer the switch still leads back to the page (not to /)"
    );
}

// ---- (3) labels of other crates --------------------------------------------------------------------------------

/// A small work in plain ASCII, so German on an English page can only come from the interface.
const PLAIN: &str =
    "#set document(title: \"The Plain Book\", author: \"Ann Example\")\n\n= Spring\n\nThe beds are dug.\n";

#[tokio::test]
async fn labels_of_other_crates_are_english_on_english_pages_and_verbatim_on_german_pages() {
    let server = common::open().await;
    let work = server.import(PLAIN, "plain.typ").await.work;
    let cases: [(String, &[&str], &[&str]); 3] = [
        (
            format!("/werke/{work}/nummerierung"),
            &["Nur Kapitel römisch", "Einleitung · I. · II. · Schluss", "Für „Eigenes Muster“ fehlt das Muster"],
            &[
                "Chapters only, Roman",
                "Introduction · I. · II. · Conclusion",
                "For “Own pattern” the pattern is missing",
            ],
        ),
        (
            format!("/einstellungen/werk?work={work}"),
            &["Schriftgröße", "unten außen", "diagonal über die Seite", "Erste Erwähnung im Glossar auszeichnen"],
            &["Font size", "bottom outside", "diagonally across the page", "Mark the first mention in the glossary"],
        ),
        (
            format!("/werke/{work}/befunde"),
            &["erledigt ohne Änderung", "übernommen"],
            &["done without a change", "adopted"],
        ),
    ];
    for (uri, german, english) in &cases {
        let (status, _, de) = server.send(get(uri, "de")).await;
        assert_eq!(status, StatusCode::OK, "{uri} renders in German");
        let de = de.replace("&quot;", "\"");
        for text in *german {
            assert!(de.contains(&html_escape(text)), "{uri}: the German page keeps the crate's wording {text:?}");
        }
        let (_, _, en) = server.send(get(uri, "en")).await;
        let en = en.replace("&quot;", "\"");
        for text in *english {
            assert!(en.contains(&html_escape(text)), "{uri}: the English page says {text:?}");
        }
        for text in *german {
            assert!(!en.contains(&html_escape(text)), "{uri}: the German label {text:?} is gone from the English page");
        }
    }
}

/// The text as minijinja writes it into a page (only `&`, `<`, `>` and the apostrophe matter here).
fn html_escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('\'', "&#x27;")
}

// ---- (4) default watermark -------------------------------------------------------------------------------------

#[tokio::test]
async fn the_draft_default_of_the_form_follows_the_work_language() {
    let server = common::open().await;
    let work = server.import(PLAIN, "plain.typ").await.work;
    let field = |value: &str| format!("name=\"draft_text\" maxlength=\"40\" size=\"16\" value=\"{value}\"");
    let (_, _, page) = server.send(get(&format!("/einstellungen/werk?work={work}"), "en")).await;
    assert!(
        page.contains(&field("ENTWURF")),
        "a German work proposes the German watermark, even on an English page (work language, not interface)"
    );
    let answer = server.rpc("work.language_set", json!({ "work": work, "language": "en" })).await;
    assert!(answer["error"].is_null(), "the work becomes English: {answer}");
    let (_, _, page) = server.send(get(&format!("/einstellungen/werk?work={work}"), "de")).await;
    assert!(
        page.contains(&field("DRAFT")),
        "an English work proposes book_t(en, \"book.draft.watermark\"), even on a German page"
    );
}

// ---- (5) wording ------------------------------------------------------------------------------------------------

#[test]
fn the_english_settings_say_revisions_like_the_work_page() {
    for key in ["settings.display.accordion_revisions", "settings.work.revisions_title"] {
        assert_eq!(i18n::t(Lang::En, key, &[]), "Revisions", "{key}: the same word as the work page (review R39a)");
        assert_eq!(i18n::t(Lang::De, key, &[]), "Fassungen", "{key}: the German word stays");
    }
}
