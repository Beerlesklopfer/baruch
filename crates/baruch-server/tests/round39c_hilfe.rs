// SPDX-License-Identifier: AGPL-3.0-or-later
//! Round 39c „Developer documentation on the website, user help gaps, rest of the interface literals“,
//! task hilfe_luecken, owner hilfe_luecken: gaps of the user help (German and English).
//!
//! The five thin help topics of the PM (what is logged, the Git mirror, the dark theme, rebuilding the index,
//! reading in a source) and the word count found in the app are new sections with fixed anchors in existing help
//! pages, identical in `site/content/de/hilfe/` and `site/content/en/hilfe/` (contract §3.2). These tests read
//! only files of the repository – the help pages and the catalogs `crates/baruch-server/locales/{de,en}/*.ftl` –
//! no server, no network, no Hugo. They check that every new anchor exists in both languages, that every new
//! section has substance, and that the interface terms the help quotes are the words of the catalogs (German text
//! in the German help, English text in the English help), so a wording change in a catalog shows up here.

use std::path::PathBuf;

/// The new sections of the contract (§3.2): (page, anchor). `statistik` exists because the app has word counts
/// (the time keeper on the page Outline); no other statistics exist.
const NEW_SECTIONS: [(&str, &str); 6] = [
    ("austausch-sicherung", "protokoll"),
    ("austausch-sicherung", "spiegel"),
    ("einstellungen", "dunkles-thema"),
    ("einstellungen", "index-neu-aufbauen"),
    ("bibel-quellen", "quelle-einlesen"),
    ("schreiben", "statistik"),
];

/// How a catalog value is quoted in the help.
#[derive(Clone, Copy)]
enum Quote {
    /// The whole value, as shown in the interface (button, heading, menu entry).
    Whole,
    /// The value up to its first " (" – labels whose tail carries a placeholder or a remark.
    BeforeParen,
}

/// Interface terms each new section quotes in both languages: (anchor, catalog key, how it is quoted).
const TERMS: [(&str, &str, Quote); 33] = [
    ("dunkles-thema", "layout.band.settings", Quote::Whole),
    ("dunkles-thema", "settings.display.title", Quote::Whole),
    ("dunkles-thema", "settings.display.theme", Quote::Whole),
    ("dunkles-thema", "settings.display.theme_system", Quote::Whole),
    ("dunkles-thema", "settings.display.theme_light", Quote::Whole),
    ("dunkles-thema", "settings.display.theme_dark", Quote::Whole),
    ("dunkles-thema", "common.save", Quote::Whole),
    ("dunkles-thema", "layout.band.tab_view", Quote::Whole),
    ("dunkles-thema", "layout.band.display", Quote::Whole),
    ("index-neu-aufbauen", "settings.connections.reindex", Quote::Whole),
    ("index-neu-aufbauen", "settings.connections.really_reindex", Quote::Whole),
    ("index-neu-aufbauen", "settings.connections.status_title", Quote::Whole),
    ("index-neu-aufbauen", "settings.connections.index_pending", Quote::Whole),
    ("index-neu-aufbauen", "settings.connections.index_built", Quote::Whole),
    ("index-neu-aufbauen", "settings.connections.index_stale", Quote::Whole),
    ("index-neu-aufbauen", "settings.connections.index_problem", Quote::Whole),
    ("quelle-einlesen", "layout.band.import_pdf", Quote::Whole),
    ("quelle-einlesen", "sources.import.file", Quote::BeforeParen),
    ("quelle-einlesen", "sources.import.way", Quote::BeforeParen),
    ("quelle-einlesen", "sources.import.way_local", Quote::Whole),
    ("quelle-einlesen", "sources.import.title_field", Quote::Whole),
    ("quelle-einlesen", "sources.import.read_submit", Quote::Whole),
    ("quelle-einlesen", "sources.import.adopt_as", Quote::Whole),
    ("quelle-einlesen", "sources.import.as_library", Quote::Whole),
    ("quelle-einlesen", "sources.import.as_both", Quote::Whole),
    ("quelle-einlesen", "sources.import.author", Quote::Whole),
    ("quelle-einlesen", "sources.import.adopt", Quote::Whole),
    ("quelle-einlesen", "sources.import.discard", Quote::Whole),
    ("statistik", "layout.band.outline", Quote::Whole),
    ("statistik", "outline.page.total", Quote::Whole),
    ("statistik", "outline.page.col_duration", Quote::Whole),
    ("statistik", "settings.work.timing_title", Quote::Whole),
    ("protokoll", "editor.history.back_here", Quote::Whole),
];

/// Further terms of the log and mirror sections, quoted in both languages.
const LOG_TERMS: [(&str, &str); 4] = [
    ("protokoll", "editor.assistants.history_page"),
    ("spiegel", "settings.data.title"),
    ("spiegel", "settings.data.col_dir"),
    ("spiegel", "settings.data.dir"),
];

/// Messages the German help quotes verbatim (the English help paraphrases them, because their English wording is
/// still being aligned in this round): (anchor, key).
const GERMAN_MESSAGES: [(&str, &str); 3] = [
    ("index-neu-aufbauen", "connections_msg.ok.reindex"),
    ("index-neu-aufbauen", "connections_msg.error.reindex"),
    ("index-neu-aufbauen", "connections_msg.error.bestaetigen"),
];

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn page(lang: &str, name: &str) -> String {
    let path = root().join(format!("site/content/{lang}/hilfe/{name}.md"));
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{} cannot be read: {e}", path.display()))
}

/// The value of a single-line catalog entry `key = value` in `locales/<lang>/<first segment>.ftl`.
fn catalog(lang: &str, key: &str) -> String {
    let area = key.split('.').next().unwrap();
    let path = root().join(format!("crates/baruch-server/locales/{lang}/{area}.ftl"));
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{} cannot be read: {e}", path.display()));
    let prefix = format!("{key} =");
    let value = text
        .lines()
        .find_map(|line| line.strip_prefix(&prefix))
        .unwrap_or_else(|| panic!("{key} exists in {} (the help quotes it)", path.display()))
        .trim()
        .to_owned();
    assert!(!value.is_empty(), "{key} in {lang} is a single-line entry, not a plural block (the help quotes it as is)");
    value
}

fn quoted(lang: &str, key: &str, how: Quote) -> String {
    let value = catalog(lang, key);
    match how {
        Quote::Whole => value,
        Quote::BeforeParen => value.split(" (").next().unwrap().trim().to_owned(),
    }
}

/// The level (number of `#`) of a heading line, if it is one.
fn heading_level(line: &str) -> Option<usize> {
    let hashes = line.chars().take_while(|&c| c == '#').count();
    (hashes > 0 && line[hashes..].starts_with(' ')).then_some(hashes)
}

/// The text of the section with the anchor `id`: from its heading up to the next heading of the same or a higher
/// level (sub-headings belong to it). `None` if the page has no such heading.
fn section(text: &str, id: &str) -> Option<String> {
    let marker = format!("{{#{id}}}");
    let lines: Vec<&str> = text.lines().collect();
    let start = lines.iter().position(|line| heading_level(line).is_some() && line.trim_end().ends_with(&marker))?;
    let level = heading_level(lines[start]).unwrap();
    let end = lines[start + 1..]
        .iter()
        .position(|line| heading_level(line).is_some_and(|l| l <= level))
        .map_or(lines.len(), |offset| start + 1 + offset);
    Some(lines[start..end].join("\n"))
}

/// Prose of a section: without shortcode lines (picture calls) and fenced code.
fn prose(section: &str) -> String {
    let mut fenced = false;
    let mut out = Vec::new();
    for line in section.lines() {
        if line.trim_start().starts_with("```") {
            fenced = !fenced;
            continue;
        }
        if !fenced && !line.trim_start().starts_with("{{<") {
            out.push(line);
        }
    }
    out.join("\n")
}

fn page_of(id: &str) -> &'static str {
    NEW_SECTIONS.iter().find(|(_, a)| *a == id).map(|(p, _)| *p).unwrap()
}

#[test]
fn every_new_anchor_exists_in_german_and_english() {
    for (name, id) in NEW_SECTIONS {
        for lang in ["de", "en"] {
            assert!(
                section(&page(lang, name), id).is_some(),
                "site/content/{lang}/hilfe/{name}.md has a heading with the fixed anchor {{#{id}}} (contract §3.2: \
                 the same anchor in both languages, other pages link to it)"
            );
        }
    }
}

#[test]
fn new_sections_have_the_same_level_and_order_in_both_languages() {
    for name in ["austausch-sicherung", "einstellungen", "bibel-quellen", "schreiben"] {
        let order = |lang: &str| -> Vec<(usize, String)> {
            page(lang, name)
                .lines()
                .filter_map(|line| {
                    let level = heading_level(line)?;
                    let id = line.rsplit_once("{#")?.1.trim_end().strip_suffix('}')?.to_owned();
                    Some((level, id))
                })
                .collect()
        };
        assert_eq!(
            order("de"),
            order("en"),
            "{name}.md: headings (level and anchor) stand in the same order in de and en – the English page is the \
             translation of the German one"
        );
    }
}

#[test]
fn every_new_section_has_substance() {
    for (name, id) in NEW_SECTIONS {
        for lang in ["de", "en"] {
            let text = prose(&section(&page(lang, name), id).unwrap());
            let chars = text.chars().count();
            assert!(
                chars >= 900,
                "{lang}/hilfe/{name}.md#{id} explains its topic, not a one-liner: the PM found these topics thin, the \
                 contract asks for steps, conditions and consequences ({chars} characters of prose are too few)"
            );
            let items =
                text.lines().filter(|l| l.starts_with("- ") || l.starts_with("1. ") || l.starts_with("| ")).count();
            assert!(
                items >= 3,
                "{lang}/hilfe/{name}.md#{id} gives at least three steps, list points or table rows ({items} found) – \
                 help is read step by step"
            );
        }
    }
}

#[test]
fn quoted_interface_terms_are_the_words_of_the_catalogs() {
    let mut checked = 0;
    for (id, key, how) in TERMS {
        let name = page_of(id);
        for lang in ["de", "en"] {
            let term = quoted(lang, key, how);
            let text = section(&page(lang, name), id).unwrap();
            assert!(
                text.contains(&term),
                "{lang}/hilfe/{name}.md#{id} quotes the interface term „{term}“ ({key}) with the words of the \
                 {lang} catalog – otherwise the reader looks for a button that is called differently"
            );
            checked += 1;
        }
    }
    for (id, key) in LOG_TERMS {
        let name = page_of(id);
        for lang in ["de", "en"] {
            let term = catalog(lang, key);
            let text = section(&page(lang, name), id).unwrap();
            assert!(text.contains(&term), "{lang}/hilfe/{name}.md#{id} names „{term}“ ({key}) as the interface does");
            checked += 1;
        }
    }
    for (id, key) in GERMAN_MESSAGES {
        let name = page_of(id);
        let message = catalog("de", key);
        let text = section(&page("de", name), id).unwrap();
        // The help puts the message in „…“ and turns the inner „…“ of the catalog text into the same marks; compare
        // without line breaks.
        let flat: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
        assert!(
            flat.contains(&message),
            "de/hilfe/{name}.md#{id} quotes the message „{message}“ ({key}) verbatim, as the page shows it"
        );
        checked += 1;
    }
    assert!(checked >= 70, "the term lists were really read ({checked} checks)");
}

#[test]
fn the_theme_hint_is_quoted_as_the_ribbon_shows_it() {
    // widget.panes.theme = „Thema: { $name }“ with name = widget.panes.theme_dark: the help quotes the filled hint.
    for lang in ["de", "en"] {
        let hint = catalog(lang, "widget.panes.theme").replace("{ $name }", &catalog(lang, "widget.panes.theme_dark"));
        let text = section(&page(lang, "einstellungen"), "dunkles-thema").unwrap();
        assert!(
            text.contains(&hint),
            "{lang}/hilfe/einstellungen.md#dunkles-thema quotes the hint of the theme button „{hint}“ as the ribbon \
             shows it after switching to dark"
        );
        assert!(
            text.contains("◐") && text.contains("☀") && text.contains("☾"),
            "{lang}: the three signs of the theme button (base.html data-auto/-light/-dark) appear in the order of \
             the switch"
        );
    }
}

#[test]
fn old_one_liners_link_to_the_new_anchors() {
    // (page, link target without language prefix): the short mentions that existed before now lead to the sections.
    let links = [
        ("einstellungen", "/hilfe/einstellungen/#dunkles-thema"),
        ("einstellungen", "/hilfe/einstellungen/#index-neu-aufbauen"),
        ("einstellungen", "/hilfe/austausch-sicherung/#spiegel"),
        ("bibel-quellen", "/hilfe/bibel-quellen/#quelle-einlesen"),
        ("einlesen", "/hilfe/bibel-quellen/#quelle-einlesen"),
        ("austausch-sicherung", "/hilfe/austausch-sicherung/#protokoll"),
        ("schreiben", "/hilfe/austausch-sicherung/#spiegel"),
    ];
    for (name, target) in links {
        for (lang, prefix) in [("de", ""), ("en", "/en")] {
            let wanted = format!("]({prefix}{target})");
            assert!(
                page(lang, name).contains(&wanted),
                "{lang}/hilfe/{name}.md links to {prefix}{target} – the old one-line mention leads the reader to the \
                 new section"
            );
        }
    }
}

#[test]
fn the_helpers_find_sections_as_the_checks_assume() {
    let text = "intro\n## A {#a}\ntext a\n### B {#b}\ntext b\n## C {#c}\ntext c\n";
    assert_eq!(
        section(text, "a").unwrap(),
        "## A {#a}\ntext a\n### B {#b}\ntext b",
        "a section keeps its sub-sections"
    );
    assert_eq!(section(text, "b").unwrap(), "### B {#b}\ntext b", "a sub-section ends at the next higher heading");
    assert_eq!(section(text, "c").unwrap(), "## C {#c}\ntext c", "the last section runs to the end of the page");
    assert!(section(text, "x").is_none(), "an unknown anchor is not found");
    assert_eq!(
        prose("## A\n{{< hilfebild name=\"x\" >}}\n```sh\ncode\n```\nword"),
        "## A\nword",
        "pictures and code are no prose"
    );
    assert_eq!(catalog("de", "common.save"), "Speichern", "catalog values are read raw from the right file");
    assert_eq!(quoted("en", "sources.import.file", Quote::BeforeParen), "File", "the tail in parentheses is cut");
}
