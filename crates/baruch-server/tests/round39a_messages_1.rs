// SPDX-License-Identifier: AGPL-3.0-or-later
//! Round 39a, task messages_1: the server messages of the content, work, output and literature modules come from the
//! catalogs (`locales/<lang>/*_msg.ftl`). Each test sends the same request with `Accept-Language: en` and `de` and
//! checks the English text against the old German wording (the German pin of `common::router` stays for all older
//! tests); direct calls outside a request run in an explicit language scope.

use crate::common;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use baruch_server::i18n::{self, Lang};
use regex::Regex;
use serde_json::{Value, json};

/// The twelve catalog files of this task.
const STEMS: [&str; 12] = [
    "works_msg",
    "outline_msg",
    "posts_msg",
    "ideas_msg",
    "assets_msg",
    "cover_msg",
    "findings_msg",
    "reference_msg",
    "view_msg",
    "assist_msg",
    "export_msg",
    "sources_msg",
];

/// GET in the language `lang`: status and body.
async fn get(server: &common::Server, uri: &str, lang: &str) -> (StatusCode, String) {
    let request = common::with_lang(Request::get(uri).body(Body::empty()).unwrap(), lang);
    let (status, _, body) = server.send(request).await;
    (status, body)
}

/// Form POST in the language `lang`: status and body.
async fn post_form(server: &common::Server, uri: &str, form: &str, lang: &str) -> (StatusCode, String) {
    let request = Request::post(uri)
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(form.to_owned()))
        .unwrap();
    let (status, _, body) = server.send(common::with_lang(request, lang)).await;
    (status, body)
}

/// JSON-RPC call in the language `lang`: the answer as JSON.
async fn rpc(server: &common::Server, method: &str, params: Value, lang: &str) -> Value {
    let request = Request::post("/rpc")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params }).to_string()))
        .unwrap();
    let (_, _, body) = server.send(common::with_lang(request, lang)).await;
    serde_json::from_str(&body).unwrap_or(Value::Null)
}

/// Keys of one of the own catalogs in the English file.
fn keys_of(stem: &str) -> Vec<String> {
    let (_, _, source) = i18n::CATALOGS
        .iter()
        .find(|(lang, s, _)| *lang == "en" && *s == stem)
        .unwrap_or_else(|| panic!("locales/en/{stem}.ftl is embedded by build.rs"));
    i18n::catalog_entries(source).into_iter().map(|(key, _)| key).collect()
}

#[test]
fn every_own_catalog_exists_in_both_languages_and_english_has_no_german_letters() {
    let german = Regex::new("[äöüÄÖÜß„‚]").unwrap();
    for stem in STEMS {
        let keys = keys_of(stem);
        assert!(keys.len() > 3, "{stem}: the area file carries the messages of its modules, found {}", keys.len());
        for key in &keys {
            assert!(key.starts_with(&format!("{stem}.")), "{key}: the first segment names the file {stem}");
            let en = i18n::t(Lang::En, key, &[]);
            let de = i18n::t(Lang::De, key, &[]);
            assert_ne!(en, *key, "{key}: English text exists (no raw key on the page)");
            assert_ne!(de, *key, "{key}: German text exists (no raw key on the page)");
            assert!(!german.is_match(&en), "{key}: the English text has no German letters or quotes: {en}");
        }
    }
}

#[tokio::test]
async fn a_notice_follows_the_interface_language() {
    let server = common::open().await;
    let (status, en) = get(&server, "/werke?geloescht=Gartenbuch", "en").await;
    assert_eq!(status, StatusCode::OK, "the work list answers: {en}");
    assert!(
        en.contains("The work “Gartenbuch” is deleted; its Git mirror now lies under works&#x2f;.geloescht&#x2f;."),
        "English notice after deleting: {en}"
    );
    let (_, de) = get(&server, "/werke?geloescht=Gartenbuch", "de").await;
    assert!(
        de.contains(
            "Das Werk „Gartenbuch“ ist gelöscht; sein Git-Spiegel liegt jetzt unter works&#x2f;.geloescht&#x2f;."
        ),
        "German notice word for word as before Round 39a: {de}"
    );
}

#[tokio::test]
async fn a_form_error_follows_the_interface_language() {
    let server = common::open().await;
    let (status, en) = post_form(&server, "/werke/act", "action=delete", "en").await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "a delete without a work is refused: {en}");
    assert_eq!(en, "Error: work missing.", "English form error");
    let (status, de) = post_form(&server, "/werke/act", "action=delete", "de").await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "the same refusal in German");
    assert_eq!(de, "Fehler: Werk fehlt.", "German wording as before Round 39a");
}

#[tokio::test]
async fn an_rpc_refusal_follows_the_interface_language() {
    let server = common::open().await;
    let work = server.state.default_work().await.unwrap();
    let params = json!({ "work": work, "confirm_title": "Ein ganz anderer Titel" });
    let en = rpc(&server, "work.delete", params.clone(), "en").await;
    assert_eq!(en["error"]["code"], -32000, "a wrong title is a refusal (REFUSED): {en}");
    assert_eq!(en["error"]["message"], "The title entered does not match; nothing deleted.", "English refusal: {en}");
    let de = rpc(&server, "work.delete", params, "de").await;
    assert_eq!(de["error"]["message"], "Der eingegebene Titel passt nicht; nichts gelöscht.", "German refusal: {de}");
    let missing = rpc(&server, "work.update", json!({ "title": "x" }), "en").await;
    assert_eq!(missing["error"]["message"], "work missing: which work? (work id, a number)", "form check over RPC");
}

#[tokio::test]
async fn an_error_page_follows_the_interface_language() {
    let server = common::open().await;
    let (status, en) = get(&server, "/werke/987654/bearbeiten", "en").await;
    assert_eq!(status, StatusCode::NOT_FOUND, "an unknown work is 404: {en}");
    assert_eq!(en, "unknown work", "English body of the error page");
    let (status, de) = get(&server, "/werke/987654/bearbeiten", "de").await;
    assert_eq!(status, StatusCode::NOT_FOUND, "the same 404 in German");
    assert_eq!(de, "unbekanntes Werk", "German body as before Round 39a");
    let (_, en) = get(&server, "/werke/987654/umschlag", "en").await;
    assert_eq!(en, "unknown work", "the cover page answers in the same language");
}

#[tokio::test]
async fn direct_calls_follow_the_language_scope_and_default_to_english() {
    let en = baruch_server::literature::pick_section();
    assert!(en.starts_with("Choose a section:"), "outside a request the interface language is English: {en}");
    let de = i18n::sync_scope(Lang::De, baruch_server::literature::pick_section);
    assert!(de.starts_with("Abschnitt wählen: Die Persona antwortet im Abschnitts-Chat."), "German scope: {de}");
    let cancelled = i18n::scope(Lang::De, async { baruch_server::typeset::cancelled_text() }).await;
    assert_eq!(cancelled, "Satz abgebrochen.", "an async scope carries the language into the call");
    let busy = i18n::scope(Lang::En, async { baruch_server::pdf::busy_text() }).await;
    assert!(baruch_server::pdf::is_busy(&busy), "the busy check knows the English text: {busy}");
    let german_busy = i18n::t(Lang::De, "export_msg.pdf.busy", &[]);
    assert!(
        i18n::scope(Lang::En, async { baruch_server::pdf::is_busy(&german_busy) }).await,
        "and still the German one (status mapping across languages)"
    );
}

#[tokio::test]
async fn counted_words_choose_singular_and_plural_in_both_languages() {
    let one = [("count", i18n::Arg::from(1usize))];
    let two = [("count", i18n::Arg::from(2usize))];
    assert_eq!(i18n::t(Lang::De, "export_msg.exchange.add.revisions", &one), "Fassung", "German singular");
    assert_eq!(i18n::t(Lang::De, "export_msg.exchange.add.revisions", &two), "Fassungen", "German plural");
    assert_eq!(i18n::t(Lang::En, "export_msg.exchange.add.revisions", &one), "revision", "English singular");
    assert_eq!(i18n::t(Lang::En, "export_msg.exchange.add.revisions", &two), "revisions", "English plural");
}
