// SPDX-License-Identifier: AGPL-3.0-or-later
//! Round 39a, task widgets: the TypeScript widgets take their texts from the catalog in the page head
//! (`#baruch-i18n`, keys `widget.<module>.<name>` in `locales/{en,de}/widget.ftl`).
//!
//! Checked here: the page catalog carries every widget text in the page language; every key the widgets name exists
//! and none is left over; English has no German left and German is the wording the widgets had before; widgets run
//! under Node with the English and with the German catalog and show the respective text.

use crate::common;

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::process::Command;

use axum::body::Body;
use axum::http::Request;
use baruch_server::i18n::{self, CATALOGS, Lang};
use regex::Regex;
use serde_json::{Value, json};

fn web_src() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("web/src")
}

/// `key → raw value` of `locales/<lang>/widget.ftl`, multiline values with trimmed lines.
fn widget_catalog(lang: &str) -> BTreeMap<String, String> {
    let (_, _, source) = CATALOGS
        .iter()
        .find(|(code, stem, _)| *code == lang && *stem == "widget")
        .unwrap_or_else(|| panic!("locales/{lang}/widget.ftl is built into the server"));
    i18n::catalog_entries(source)
        .into_iter()
        .map(|(key, value)| (key, value.lines().map(str::trim).collect::<Vec<_>>().join("\n").trim().to_owned()))
        .collect()
}

/// Every `"widget.…"` literal in the widget sources (calls `t("…")`, but also keys in tables and ternaries).
fn keys_in_sources() -> BTreeMap<String, String> {
    let literal = Regex::new(r#"["'](widget\.[a-z0-9_]+(?:\.[a-z0-9_]+)+)["']"#).unwrap();
    let mut found = BTreeMap::new();
    for entry in std::fs::read_dir(web_src()).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|e| e == "ts") {
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            let text = std::fs::read_to_string(&path).unwrap();
            for key in literal.captures_iter(&text) {
                found.entry(key[1].to_owned()).or_insert_with(|| name.clone());
            }
        }
    }
    found
}

fn page_catalog(page: &str) -> Value {
    let open = "<script type=\"application/json\" id=\"baruch-i18n\">";
    let start = page.find(open).expect("the page head carries the widget catalog") + open.len();
    let json = &page[start..start + page[start..].find("</script>").unwrap()];
    serde_json::from_str(json).expect("the widget catalog is valid JSON")
}

#[tokio::test]
async fn the_page_catalog_carries_every_widget_text_in_the_page_language() {
    let server = common::open().await;
    for lang in ["en", "de"] {
        let request = Request::get("/ueber").body(Body::empty()).unwrap();
        let (_, _, page) = server.send(common::with_lang(request, lang)).await;
        let catalog = page_catalog(&page);
        assert_eq!(catalog["lang"], lang, "the catalog has the language of the page");
        let messages = catalog["messages"].as_object().expect("messages is an object");
        let want = widget_catalog(lang);
        let got: BTreeSet<&str> = messages.keys().map(String::as_str).collect();
        let expected: BTreeSet<&str> = want.keys().map(String::as_str).collect();
        assert_eq!(
            got, expected,
            "{lang}: the page carries exactly the keys of widget.ftl (nothing missing, nothing else)"
        );
        for (key, raw) in &want {
            let text = messages[key].as_str().unwrap();
            assert_eq!(
                text, raw,
                "{lang} {key}: the page has the catalog text with its {{ $name }} placeables kept for the widget"
            );
        }
    }
}

#[test]
fn every_key_the_widgets_name_exists_and_no_widget_key_is_left_over() {
    let used = keys_in_sources();
    assert!(used.len() > 300, "the widgets name their texts by key (found {})", used.len());
    let en = widget_catalog("en");
    for (key, file) in &used {
        assert!(en.contains_key(key), "{file}: {key} is used but missing in locales/en/widget.ftl (shows as raw key)");
        let module = key.split('.').nth(1).unwrap();
        assert_eq!(format!("{module}.ts"), *file, "{key}: the second segment names the widget module that uses it");
    }
    for key in en.keys() {
        assert!(used.contains_key(key), "{key} is in widget.ftl but no widget names it – an orphan");
    }
}

#[test]
fn english_has_no_german_left_and_german_is_the_old_wording() {
    let en = widget_catalog("en");
    let de = widget_catalog("de");
    assert_eq!(en.keys().collect::<Vec<_>>(), de.keys().collect::<Vec<_>>(), "en and de have the same widget keys");
    let german = Regex::new(
        r"[äöüÄÖÜß„‚]|(?i)\b(?:und|oder|der|die|das|nicht|mit|ist|ein|eine|kein|keine|bitte|noch|nur|wird|zum|zur|Fehler|Abschnitt|Einfall|Gliederung|Fassung)\b",
    )
    .unwrap();
    for (key, text) in &en {
        assert!(!german.is_match(text), "en {key}: no German in the English catalog: {text:?}");
    }
    // Spot checks: the German wording is byte for byte what the widgets showed before Round 39a.
    for (key, old) in [
        (
            "widget.chat.delete_question",
            "Beitrag { $id } von { $author } löschen? Der Spiegel hält ihn im Lösch-Commit fest.",
        ),
        ("widget.widgets.discard", "Ungespeicherte Änderung verwerfen?"),
        ("widget.rpc.unchanged", "Nichts zu speichern: Der Text ist unverändert."),
        ("widget.mindmap.link_selected", "Beziehung gewählt: Entf löst sie (wiederherstellbar mit Strg+Z)."),
        (
            "widget.outline.split_question",
            "Der Text enthält neue Überschriften: { $headings }.\nAbschnitt teilen und speichern?",
        ),
        ("widget.wysiwyg.linebreak", "erzwungener Zeilenumbruch (\\)"),
        (
            "widget.search.select_first",
            "Erst im Quelltext Text markieren, dann die Suche öffnen und „In Markierung“ einschalten.",
        ),
        ("widget.ws_command.register_stichwort", "Stichwortregister"),
        ("widget.typeset_settings.label_widows_orphans", "Schusterjungen/Hurenkinder"),
    ] {
        assert_eq!(de[key], old, "de {key}: the German text stays the old wording of the widget");
    }
    let mindmap_help = &de["widget.mindmap.help"];
    assert!(
        mindmap_help.starts_with("Klick wählt, Doppelklick oder F2 benennt um.") && mindmap_help.ends_with("0 zoomen."),
        "the mind map help is joined into one German text: {mindmap_help}"
    );
}

#[test]
fn no_widget_detects_a_language_itself() {
    for entry in std::fs::read_dir(web_src()).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|e| e != "ts") {
            continue;
        }
        let text = std::fs::read_to_string(&path).unwrap();
        for forbidden in ["navigator.language", "navigator.languages", "Intl.DateTimeFormat", "toLocaleDateString"] {
            assert!(
                !text.contains(forbidden),
                "{}: the language comes from the server's catalog, never from the browser ({forbidden})",
                path.display()
            );
        }
    }
}

/// Runs `body` under Node with the delivered modules imported as `chat`, `rpc`, `menu` (context menu), `ws`
/// (ws_command), `mm` and `i18n`; `catalog` = JSON of the page catalog put into a fake `#baruch-i18n` element before
/// the first text is read (`None` = no element, as in the node tests without a page).
fn under_node(catalog: Option<&str>, body: &str) -> Value {
    let dir = tempfile::tempdir().unwrap();
    for (name, source) in baruch_server::SCRIPTS {
        std::fs::write(dir.path().join(name.replace(".js", ".mjs")), source.replace(".js\"", ".mjs\"")).unwrap();
    }
    let element = match catalog {
        Some(json) => format!(
            "globalThis.document = {{ getElementById: (id) => (id === 'baruch-i18n' ? {{ textContent: {} }} : null) }};",
            serde_json::to_string(json).unwrap()
        ),
        None => String::new(),
    };
    let script = format!(
        "import * as chat from './chat.mjs';\nimport * as rpc from './rpc.mjs';\nimport * as menu from './context_menu.mjs';\n\
         import * as ws from './ws_command.mjs';\nimport * as i18n from './i18n.mjs';\n{element}\nconsole.log(JSON.stringify({body}));\n"
    );
    std::fs::write(dir.path().join("probe.mjs"), script).unwrap();
    let out = Command::new("node").arg("probe.mjs").current_dir(dir.path()).output().expect("node runs (as for tsc)");
    assert!(out.status.success(), "node: {}", String::from_utf8_lossy(&out.stderr));
    serde_json::from_slice(&out.stdout).unwrap()
}

const PROBE: &str = "{ question: chat.deleteQuestion(7, '@lektor'), typing: chat.typingLine([{persona: '@a'}, {persona: '@b'}]), \
     saved: rpc.savedMessage({ saved: 'new', rev: 3, reason_label: 'Kapitel' }), \
     menu: menu.menuItems(true).map((i) => i.label), register: ws.BUILTIN_REGISTERS[0].title, \
     label: ws.markLabel({ register: 'personen', term: 'Paulus', see: 'Saulus' }), number: i18n.fmtNumber(1234.5, 1), \
     lang: i18n.uiLang() }";

#[test]
fn widgets_show_the_text_of_the_catalog_they_get() {
    let en = under_node(Some(&i18n::widget_catalog_json(Lang::En)), PROBE);
    assert_eq!(
        en,
        json!({
            "question": "Delete post 7 by @lektor? The mirror keeps it in the deletion commit.",
            "typing": "@a and @b are writing …",
            "saved": "Saved: new version 3 (Kapitel).",
            "menu": ["Add to glossary …", "Add to index …", "Look up in the glossary", "Edit index entry …", "Remove index entry"],
            "register": "Subject index",
            "label": "Index entry in Index of persons: Paulus; see: Saulus",
            "number": "1,234.5",
            "lang": "en",
        }),
        "with the English catalog the widgets speak English (placeables filled, numbers in English)"
    );
    let de = under_node(Some(&i18n::widget_catalog_json(Lang::De)), PROBE);
    assert_eq!(
        de,
        json!({
            "question": "Beitrag 7 von @lektor löschen? Der Spiegel hält ihn im Lösch-Commit fest.",
            "typing": "@a und @b schreiben …",
            "saved": "Gespeichert: neue Fassung 3 (Kapitel).",
            "menu": ["In Glossar aufnehmen …", "In Register aufnehmen …", "Im Glossar nachschlagen", "Registereintrag bearbeiten …", "Registereintrag entfernen"],
            "register": "Stichwortregister",
            "label": "Registereintrag im Personenregister: Paulus; siehe: Saulus",
            "number": "1.234,5",
            "lang": "de",
        }),
        "with the German catalog the widgets show the old German wording"
    );
}

#[test]
fn without_a_catalog_a_widget_shows_the_key_instead_of_failing() {
    let bare = under_node(None, "{ question: chat.deleteQuestion(7, '@lektor'), lang: i18n.uiLang() }");
    assert_eq!(
        bare,
        json!({ "question": "widget.chat.delete_question", "lang": "en" }),
        "no #baruch-i18n element (a page without catalog): the key stands there, nothing throws"
    );
}
