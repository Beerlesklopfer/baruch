// SPDX-License-Identifier: AGPL-3.0-or-later
//! Round 39c „Developer documentation on the website, user help gaps, rest of the interface literals“, task reste_2,
//! owner reste_2: crate messages, presets in the work language, shortcode words, catalog wording.
//!
//! Checks (contract §5): (1) store texts that the interface shows in both interface languages, German verbatim;
//! (2) the notes of the Bible checks and the refusals of `scripture.check`; (3) typeset-settings labels and
//! messages, the pattern refusals of the outline numbering, the print-check block names and the cross-reference
//! tooltip; (4) `work.create` with `language` and the form on `/werke/neu`; (5) the shortcode words (the files are
//! read, Hugo itself runs in `scripts/ci-site.sh`); (6) the catalog wording of the Round 38 vocabulary.
//! Texts freely invented.

use crate::common;

use axum::body::Body;
use axum::http::{Request, header};
use baruch_server::i18n::{self, Lang};
use serde_json::{Value, json};

/// JSON-RPC call in the interface language `lang` (the test router would otherwise pin German).
async fn rpc_in(server: &common::Server, lang: &str, method: &str, params: Value) -> Value {
    let body = json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params });
    let request = Request::post("/rpc")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    let (_, _, text) = server.send(common::with_lang(request, lang)).await;
    serde_json::from_str(&text).expect("the answer is JSON")
}

fn get_in(uri: &str, lang: &str) -> Request<Body> {
    common::with_lang(Request::get(uri).body(Body::empty()).unwrap(), lang)
}

fn repo_file(path: &str) -> String {
    let full = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(path);
    std::fs::read_to_string(&full).unwrap_or_else(|e| panic!("{} cannot be read: {e}", full.display()))
}

fn catalog(lang: &str, stem: &str) -> String {
    repo_file(&format!("crates/baruch-server/locales/{lang}/{stem}.ftl"))
}

// ---- (1) store texts -----------------------------------------------------------------------------------------

/// Store texts of reachable forms and RPC calls: (German wording of the store, English wording).
const STORE_TEXTS: &[(&str, &str)] = &[
    ("Der Name darf nicht leer sein.", "The name must not be empty."),
    ("Einfall 7 ist gelöscht; erst wiederherstellen.", "Idea 7 is deleted; restore it first."),
    (
        "Das Schlagwort „Dornen…“ ist zu lang (höchstens 60 Zeichen, hier 64).",
        "The keyword “Dornen…” is too long (at most 60 characters, here 64).",
    ),
    (
        "Einfall 3 und Einfall 9 gehören nicht zum selben Werk; verbinden lassen sich nur Einfälle eines Werks.",
        "Idea 3 and idea 9 do not belong to the same work; only ideas of one work can be connected.",
    ),
    (
        "Lage: x muss eine endliche Zahl zwischen -100000 und 100000 sein.",
        "Position: x must be a finite number between -100000 and 100000.",
    ),
    (
        "Der Baum hat 900 Knoten; erlaubt sind höchstens 500.",
        "The tree has too many nodes (900); at most 500 are allowed.",
    ),
    ("Rand oben: 5 bis 60 mm", "Top margin: 5 to 60 mm"),
    ("Schriftgröße: 6 bis 24 pt", "Font size: 6 to 24 pt"),
    (
        "Ränder: Die Textbreite muss mindestens 50 mm betragen (jetzt 40,5 mm)",
        "Margins: the text width must be at least 50 mm (now 40,5 mm)",
    ),
    ("Registerseiten: Register „orte“ ist doppelt gewählt", "Index pages: index “orte” is chosen twice"),
    ("Anlegen beantragt man über outline_add, nicht als Antrag.", "Creating is not requested; use outline_add."),
];

#[test]
fn reachable_store_texts_are_english_in_english_and_verbatim_in_german() {
    for (german, english) in STORE_TEXTS {
        assert_eq!(
            baruch_server::store_text::translate(Lang::De, german),
            *german,
            "German output is the wording of the store, byte for byte: {german}"
        );
        assert_eq!(
            baruch_server::store_text::translate(Lang::En, german),
            *english,
            "the English interface shows the catalog text and carries the arguments over: {german}"
        );
    }
}

#[test]
fn every_new_store_pattern_key_exists_in_both_catalogs_and_is_listed() {
    let (en, de) = (catalog("en", "store_msg"), catalog("de", "store_msg"));
    for key in baruch_server::store_text::PATTERN_KEYS {
        assert!(en.contains(&format!("{key} = ")), "{key} is listed as a pattern and needs an English text");
        assert!(de.contains(&format!("{key} = ")), "{key} is listed as a pattern and needs a German text");
    }
    for key in ["store_msg.ideas.not_sibling", "store_msg.typeset.text_height", "store_msg.personas.context_range"] {
        assert!(
            baruch_server::store_text::PATTERN_KEYS.contains(&key),
            "{key} was added in Round 39c and must be a pattern, otherwise its text would stay German"
        );
    }
}

#[tokio::test]
async fn a_typeset_refusal_of_the_store_reaches_the_rpc_in_the_interface_language() {
    let server = common::open().await;
    let made = server.rpc("work.create", json!({ "title": "Das Lichtschiff" })).await;
    let work = made["result"]["work"].as_i64().expect("work.create names the work");
    let settings = json!({ "work": work, "settings": { "font_size_pt": 500.0 } });
    let german = rpc_in(&server, "de", "work.typeset_set", settings.clone()).await;
    let english = rpc_in(&server, "en", "work.typeset_set", settings).await;
    let (de_text, en_text) = (
        german["error"]["message"].as_str().expect("an error in German").to_owned(),
        english["error"]["message"].as_str().expect("an error in English").to_owned(),
    );
    assert!(de_text.contains("Schriftgröße:"), "German keeps the label of the crate: {de_text}");
    assert!(
        en_text.contains("Font size:") && !en_text.contains("Schriftgröße"),
        "the English interface names the field in English: {en_text}"
    );
}

// ---- (2) Bible notes and refusals ----------------------------------------------------------------------------

fn fragmented_sample() -> Vec<String> {
    let mut verses: Vec<String> = (0..3).map(|i| format!("Das die ses Wort {i} bleibt")).collect();
    verses.push("Dieses Wort bleibt".to_owned());
    verses
}

#[test]
fn the_module_note_follows_the_interface_language_and_german_keeps_its_wording() {
    let sample = fragmented_sample();
    let german = i18n::sync_scope(Lang::De, || baruch_server::bibles::fragment_note("Probe", &sample))
        .expect("three split words are conspicuous");
    assert!(
        german.starts_with("Modul Probe hat auffällig viele Wortfragmente (3 von ")
            && german.contains("z. B. „die ses“")
            && german.ends_with("auch ohne Leerraum."),
        "German wording as before Round 39c: {german}"
    );
    let english = i18n::sync_scope(Lang::En, || baruch_server::bibles::fragment_note("Probe", &sample)).unwrap();
    assert!(
        english.starts_with("Module Probe has strikingly many word fragments (3 of ")
            && english.contains("e.g. “die ses”")
            && english.ends_with("without whitespace."),
        "English wording with the same numbers and examples: {english}"
    );
    assert!(!english.contains("Wortfragmente"), "no German word is left in the English note: {english}");
}

#[tokio::test]
async fn the_refusals_of_scripture_check_follow_the_interface_language() {
    let server = common::open().await;
    let made = server.rpc("work.create", json!({ "title": "Das Schilfmeer" })).await;
    let work = made["result"]["work"].as_i64().expect("a work");
    for (lang, missing, long) in [
        ("de", "text fehlt (Stellenangabe oder Fließtext)", "text: höchstens "),
        ("en", "text missing (reference or running text)", "text: at most "),
    ] {
        let answer = rpc_in(&server, lang, "scripture.check", json!({ "work": work })).await;
        assert_eq!(answer["error"]["message"], missing, "a missing text is refused in {lang}");
        let big = "x".repeat(baruch_server::scripture::MAX_TEXT_CHARS + 1);
        let answer = rpc_in(&server, lang, "scripture.check", json!({ "work": work, "text": big })).await;
        let message = answer["error"]["message"].as_str().expect("a refusal");
        assert!(message.starts_with(long), "an over-long text is refused in {lang}: {message}");
    }
}

// ---- (3) typeset settings, numbering, print check, tooltip --------------------------------------------------

#[test]
fn typeset_field_labels_and_template_conflicts_are_translated() {
    let german = i18n::sync_scope(Lang::De, || baruch_server::typeset_settings::field_label("chapter_start"));
    assert_eq!(german, "Kapitelanfang", "German label equals the one of baruch-typst");
    let english = i18n::sync_scope(Lang::En, || baruch_server::typeset_settings::field_label("margins.inside_mm"));
    assert_eq!(english, "Margins", "a subfield names its field in English");
    let conflict = baruch_typst::typeset::template_conflicts(
        "#set page(margin: 2cm)\n",
        &baruch_typst::typeset::TypesetSettings {
            margins: Some(baruch_typst::typeset::Margins {
                inside_mm: 20.0,
                outside_mm: 20.0,
                top_mm: 20.0,
                bottom_mm: 20.0,
                binding_mm: 0.0,
            }),
            ..Default::default()
        },
    );
    let first = conflict.first().expect("the template sets the margin itself");
    assert_eq!(
        i18n::sync_scope(Lang::De, || baruch_server::typeset_settings::conflict_message(first)),
        first.message,
        "German equals the message of the crate, byte for byte"
    );
    let text = i18n::sync_scope(Lang::En, || baruch_server::typeset_settings::conflict_message(first));
    assert!(
        text.starts_with("The work template sets Margins itself (line 1)"),
        "the English message names the field and the line: {text}"
    );
}

#[test]
fn pattern_refusals_of_the_numbering_are_translated_and_german_stays_verbatim() {
    for spec in ["x", "I.;x", "I|9", "I|1;A."] {
        let message = baruch_typst::scheme::Numbering::parse_spec(spec)
            .err()
            .unwrap_or_else(|| panic!("`{spec}` is no valid pattern and must be refused"));
        assert_eq!(
            baruch_server::numbering::scheme_message(Lang::De, &message),
            message,
            "German output is the message of the crate: {spec}"
        );
        let english = baruch_server::numbering::scheme_message(Lang::En, &message);
        assert!(
            !english.contains("Zählzeichen") && !english.contains("Ebene") && !english.contains("Muster"),
            "the English refusal has no German word for `{spec}`: {english}"
        );
        assert!(english.contains(spec.split(['|', ';']).next().unwrap()), "the argument is carried over: {english}");
    }
}

#[test]
fn the_cross_reference_tooltip_is_in_the_interface_language_and_german_is_unchanged() {
    let doc = baruch_wysiwyg::parse("#link(<k3>)[Brücke]\n");
    let german = baruch_wysiwyg::render_html_in(&doc, &|_| None, baruch_wysiwyg::WorkLanguage::De);
    assert!(
        german.contains("title=\"Querverweis auf k3 (Strg+Klick: springen)\""),
        "the crate keeps the German tooltip byte for byte: {german}"
    );
    for (lang, expected) in [
        (Lang::De, "Querverweis auf k3 (Strg+Klick: springen)"),
        (Lang::En, "Cross reference to k3 (Ctrl+click: jump)"),
    ] {
        let html = i18n::sync_scope(lang, || {
            baruch_wysiwyg::render_html_titled(
                &doc,
                &|_| None,
                baruch_wysiwyg::WorkLanguage::De,
                &baruch_server::wysiwyg::xref_title,
            )
        });
        assert!(
            html.contains(&format!("title=\"{expected}\"")),
            "the tooltip of the server follows the interface language {lang:?}: {html}"
        );
    }
}

// ---- (4) work.create with a work language ---------------------------------------------------------------------

async fn outline_titles(server: &common::Server, work: i64) -> Vec<String> {
    let loaded = server.state.store.load_work(work).await.expect("the work exists");
    loaded.nodes.iter().map(|n| n.title.clone()).collect()
}

#[tokio::test]
async fn work_create_writes_the_language_and_takes_the_presets_from_it() {
    let server = common::open().await;
    let german = server.rpc("work.create", json!({ "title": "Der Leuchtturm" })).await;
    let work = german["result"]["work"].as_i64().expect("a work");
    assert_eq!(
        outline_titles(&server, work).await,
        ["Erstes Kapitel"],
        "without `language` the work is German and keeps the old preset (existing callers stay as they are)"
    );
    assert_eq!(server.state.store.work_language(work).await.unwrap().as_str(), "de", "default language de");

    let english = server.rpc("work.create", json!({ "title": "The Lighthouse", "language": "en" })).await;
    let work = english["result"]["work"].as_i64().expect("a work");
    assert_eq!(outline_titles(&server, work).await, ["First chapter"], "an English book gets the English chapter");
    assert_eq!(server.state.store.work_language(work).await.unwrap().as_str(), "en", "language en is stored");

    let sermon =
        server.rpc("work.create", json!({ "title": "A Word on Water", "kind": "sermon", "language": "en" })).await;
    let work = sermon["result"]["work"].as_i64().expect("a work");
    let titles = outline_titles(&server, work).await;
    assert_eq!(titles[0], "Sermon text", "the sermon text item has its English title");
    assert_eq!(titles.len(), 7, "the seven parts as in German");
    let german_sermon = server.rpc("work.create", json!({ "title": "Ein Wort vom Wasser", "kind": "sermon" })).await;
    let work = german_sermon["result"]["work"].as_i64().expect("a work");
    assert_eq!(
        outline_titles(&server, work).await[..3],
        ["Predigttext", "Hinführung", "Beobachtung am Text"],
        "the German sermon keeps its words byte for byte"
    );
}

#[tokio::test]
async fn work_create_refuses_another_language_with_a_catalog_message() {
    let server = common::open().await;
    for (lang, expected) in
        [("de", "language: „de“ oder „en“ erwartet, nicht „fr“"), ("en", "language: expected “de” or “en”, not “fr”")]
    {
        let answer = rpc_in(&server, lang, "work.create", json!({ "title": "Le Phare", "language": "fr" })).await;
        assert_eq!(answer["error"]["code"], -32602, "a wrong language is a parameter error ({lang})");
        assert_eq!(answer["error"]["message"], expected, "the message comes from works_msg in {lang}");
    }
    let listed = server.rpc("work.list", json!({})).await;
    assert!(!listed.to_string().contains("Le Phare"), "a refused call creates nothing: {listed}");
}

#[tokio::test]
async fn the_new_work_form_preselects_the_interface_language_and_a_missing_field_means_german() {
    let server = common::open().await;
    for (lang, selected, other) in [("en", "en", "de"), ("de", "de", "en")] {
        let (_, _, page) = server.send(get_in("/werke/neu", lang)).await;
        assert!(
            page.contains(&format!("<option value=\"{selected}\" selected>")),
            "the select of `language` preselects the interface language ({lang})"
        );
        assert!(
            page.contains(&format!("<option value=\"{other}\">")),
            "the other language is offered without selection ({lang})"
        );
    }
    let (status, location) = server.post_form("/werke/act", "action=create&title=Ohne+Sprache").await;
    assert!(status.is_redirection(), "creating without the field works: {status}");
    let work: i64 = location.expect("redirect").rsplit('=').next().unwrap().parse().unwrap();
    assert_eq!(
        server.state.store.work_language(work).await.unwrap().as_str(),
        "de",
        "a missing field means German, whatever the interface language is"
    );
    let (_, location) = server.post_form("/werke/act", "action=create&title=With+Language&language=en").await;
    let work: i64 = location.expect("redirect").rsplit('=').next().unwrap().parse().unwrap();
    assert_eq!(server.state.store.work_language(work).await.unwrap().as_str(), "en", "the chosen language is stored");
    assert_eq!(outline_titles(&server, work).await, ["First chapter"], "and drives the preset chapter");
}

// ---- (5) shortcode words -----------------------------------------------------------------------------------------

#[test]
fn the_shortcodes_take_their_words_from_the_theme_catalogs() {
    for (file, key) in [
        ("fingerabdruck.html", "shortcode_fingerprint_label"),
        ("fingerabdruck.html", "shortcode_placeholder_label"),
        ("fingerabdruck.html", "shortcode_placeholder_note"),
        ("release.html", "shortcode_release_page"),
        ("taste.html", "shortcode_mac"),
    ] {
        let source = repo_file(&format!("site/themes/baruch/layouts/_shortcodes/{file}"));
        assert!(source.contains(&format!("i18n \"{key}\"")), "{file} must take its word from the key {key}");
        for lang in ["en", "de"] {
            let toml = repo_file(&format!("site/themes/baruch/i18n/{lang}.toml"));
            assert!(toml.contains(&format!("[{key}]")), "{lang}.toml has the key {key} that {file} uses");
        }
    }
    for file in ["fingerabdruck.html", "release.html", "taste.html"] {
        let source = repo_file(&format!("site/themes/baruch/layouts/_shortcodes/{file}"));
        // Only the template code counts: the comment blocks `{{- /* … */ -}}` explain the old German wording.
        let mut code = String::new();
        let mut rest = source.as_str();
        while let Some(start) = rest.find("/*") {
            code.push_str(&rest[..start]);
            rest = &rest[rest[start..].find("*/").map_or(rest.len(), |end| start + end + 2)..];
        }
        code.push_str(rest);
        for word in ["Fingerabdruck des Archivschlüssels", "Release-Seite", "(Mac:"] {
            assert!(
                !code.contains(word),
                "{file} must not print the German word \"{word}\" directly; it comes from the i18n catalog"
            );
        }
    }
    assert!(
        repo_file("site/themes/baruch/i18n/de.toml").contains("other = \"Release-Seite v%s\""),
        "the German value is the wording the shortcode carried before, so German pages stay byte-equal"
    );
}

// ---- (6) catalog wording ---------------------------------------------------------------------------------------

#[test]
fn the_english_catalogs_use_one_vocabulary() {
    let accounts = catalog("en", "accounts_msg");
    assert!(accounts.contains("accounts_msg.menu.lektor = Lector"), "the persona and role word is Lector in English");
    let editor = catalog("en", "editor");
    for key in ["lector", "lector_fixed", "lector_auto", "lector_badge"] {
        let line = editor.lines().find(|l| l.starts_with(&format!("editor.assistants.{key} = "))).unwrap();
        assert!(line.contains("Lector") && !line.contains("Lektor"), "{line}: persona is Lector in English");
    }
    assert!(
        catalog("en", "layout").contains("layout.band.exchange = Exchange: Write out")
            && catalog("en", "export").contains("export.exchange.heading = Exchange: Write out"),
        "the ribbon button and the heading of the page say the same words"
    );
    let settings = catalog("en", "settings");
    assert!(
        settings.contains("settings.work.fold_until = Hide older revisions up to")
            && settings.contains("(show all revisions again)"),
        "the English help calls revisions revisions"
    );
    assert!(
        catalog("en", "editor").contains("editor.toolbar.emph_glyph = I")
            && catalog("en", "editor").contains("editor.toolbar.strong_glyph = B")
            && catalog("de", "editor").contains("editor.toolbar.emph_glyph = K")
            && catalog("de", "editor").contains("editor.toolbar.strong_glyph = F"),
        "the italic and bold buttons read I and B in English, K and F in German"
    );
    let toolbar = repo_file("crates/baruch-server/templates/fragments/edit_toolbar.html");
    assert!(
        !toolbar.contains("<em>K</em>") && !toolbar.contains("<strong>F</strong>"),
        "the template holds no literal glyph any more"
    );
}

#[tokio::test]
async fn the_german_old_wording_of_changed_labels_is_unchanged() {
    let server = common::open().await;
    let (_, _, page) = server.send(get_in("/werke/neu", "de")).await;
    assert!(page.contains("Werksprache"), "the new field is labelled in German on the German page");
    let german = catalog("de", "layout");
    assert!(german.contains("layout.band.exchange = Austausch: Ausgeben"), "the German label is the old one");
}
