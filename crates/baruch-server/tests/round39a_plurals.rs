// SPDX-License-Identifier: AGPL-3.0-or-later
//! Round 39a, follow-up: English texts with a count read correctly for a count of 1. Server and template texts choose
//! singular or plural with a Fluent selector; widget texts cannot (`build_widget_catalog` hands the widgets plain
//! `{ $name }` strings), so they are two keys (`…_one`/`…_other`) and the widget picks one. A guard scans every
//! English catalog value for a counted plural noun without a selector, so a new text cannot bring "1 chapters" back.

use std::collections::BTreeMap;

use baruch_server::i18n::{self, Arg, CATALOGS, Lang};
use regex::Regex;

/// All values of one language: key → raw value.
fn values(lang: &str) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for (_, _, source) in CATALOGS.iter().filter(|(code, _, _)| *code == lang) {
        out.extend(i18n::catalog_entries(source));
    }
    out
}

/// `(key, variable, English for 1, English for 2, German for 1, German for 2)`: each text is a part of the result.
const SELECTED: &[(&str, &str, &str, &str, &str, &str)] = &[
    ("assets.list.found", "total", "of 1 image for", "of 2 images for", "von 1 Bild für", "von 2 Bildern für"),
    (
        "assets.one.delete_image_hint",
        "count",
        "with its 1 revision and",
        "with all 2 revisions and",
        "mit seiner 1 Fassung und",
        "mit allen 2 Fassungen und",
    ),
    ("assist_msg.solution.no_such_solution", "count", "(1 solution)", "(2 solutions)", "(1 Lösung)", "(2 Lösungen)"),
    ("cover_msg.measure.typeset_errors", "errors", "has 1 error;", "has 2 errors;", "hat 1 Fehler;", "hat 2 Fehler;"),
    (
        "editor.page.fold_show_title",
        "hidden",
        "Show 1 older revision (",
        "Show 2 older revisions (",
        "1 ältere Fassung zeigen",
        "2 ältere Fassungen zeigen",
    ),
    (
        "editor.toolbar.fold_show_title",
        "hidden",
        "Show 1 older revision (",
        "Show 2 older revisions (",
        "1 ältere Fassung zeigen",
        "2 ältere Fassungen zeigen",
    ),
    (
        "editor.lektor_side.findings_open_title",
        "open",
        "1 open finding in",
        "2 open findings in",
        "1 offener Befund in",
        "2 offene Befunde in",
    ),
    ("export.word_import.chars", "count", "1 character", "2 characters", "1 Zeichen", "2 Zeichen"),
    ("findings.page.old_label", "days", "older than 1 day", "older than 2 days", "älter als 1 Tag", "älter als 2 Tage"),
    (
        "findings.page.old_heading",
        "days",
        "older than 1 day (",
        "older than 2 days (",
        "älter als 1 Tag (",
        "älter als 2 Tage (",
    ),
    (
        "findings.panel.open_hint",
        "count",
        "1 open finding in",
        "2 open findings in",
        "1 offener Befund in",
        "2 offene Befunde in",
    ),
    (
        "ideas_msg.import.cut_titles",
        "count",
        "1 title was longer",
        "2 titles were longer",
        "1 Titel war länger",
        "2 Titel waren länger",
    ),
    (
        "ideas_msg.import.cut_notes",
        "count",
        "1 note was longer",
        "2 notes were longer",
        "1 Notiz war länger",
        "2 Notizen waren länger",
    ),
    (
        "ideas_msg.import.untitled",
        "count",
        "1 node without a title is",
        "2 nodes without a title are",
        "Titel heißt jetzt",
        "Titel heißen jetzt",
    ),
    (
        "ideas_msg.import.cut_words",
        "count",
        "For 1 node keywords",
        "For 2 nodes keywords",
        "Bei 1 Knoten",
        "Bei 2 Knoten",
    ),
    (
        "ideas_msg.near.ready",
        "count",
        "of 1 idea embedded",
        "of 2 ideas embedded",
        "von 1 Einfall eingebettet",
        "von 2 Einfällen eingebettet",
    ),
    (
        "import_msg.docx.unaligned_work",
        "work",
        "has 1 section, the file",
        "has 2 sections, the file",
        "hat 1 Abschnitt, die Datei",
        "hat 2 Abschnitte, die Datei",
    ),
    ("outline.page.total_words", "words", "(1 word,", "(2 words,", "(1 Wort,", "(2 Wörter,"),
    ("outline.page.words", "words", "1 word", "2 words", "1 Wort", "2 Wörter"),
    ("reference.scripture.matches", "count", "1 match", "2 matches", "1 stimmt", "2 stimmt"),
    ("reference.scripture.differs", "count", "1 differs", "2 differ", "1 weicht ab", "2 weicht ab"),
    (
        "reference_msg.map.chapters",
        "count",
        "only 1 chapter in",
        "only 2 chapters in",
        "nur 1 Kapitel",
        "nur 2 Kapitel",
    ),
    ("reference_msg.map.verses", "count", "only 1 verse in", "only 2 verses in", "nur 1 Vers", "nur 2 Verse"),
    ("search.page.preview_heading", "total", "· 1 match ·", "· 2 matches ·", "· 1 Treffer ·", "· 2 Treffer ·"),
    (
        "settings.work.timing_summary_html",
        "words",
        "(1 spoken word)",
        "(2 spoken words)",
        "(1 gesprochenes Wort)",
        "(2 gesprochene Wörter)",
    ),
    (
        "settings.work.sync_on_html",
        "documents",
        "(1 chapter uploaded",
        "(2 chapters uploaded",
        "(1 Kapitel hochgeladen",
        "(2 Kapitel hochgeladen",
    ),
    (
        "settings.typeset.preview_pages",
        "pages",
        "sample text: 1 page",
        "sample text: 2 pages",
        "Mustertext: 1 Seite",
        "Mustertext: 2 Seiten",
    ),
    ("settings.typeset.preview_errors", "errors", ", 1 error", ", 2 errors", ", 1 Fehler", ", 2 Fehler"),
    ("settings.typeset.preview_warnings", "warnings", ", 1 warning", ", 2 warnings", ", 1 Warnung", ", 2 Warnungen"),
    (
        "sources.source.marks",
        "marks",
        "· 1 place marked",
        "· 2 places marked",
        "· 1 Fundstelle markiert",
        "· 2 Fundstellen markiert",
    ),
    (
        "sources.multigpt.marks",
        "marks",
        "· 1 place marked",
        "· 2 places marked",
        "· 1 Fundstelle markiert",
        "· 2 Fundstellen markiert",
    ),
    ("sources.import.chars", "chars", "1 character", "2 characters", "1 Zeichen", "2 Zeichen"),
    ("sources.bible.chapters", "chapters", "1 chapter", "2 chapters", "1 Kapitel", "2 Kapitel"),
    (
        "sync_msg.tool.no_ids",
        "count",
        ": 1 entry without",
        ": 2 entries without",
        ": 1 Eintrag ohne",
        ": 2 Einträge ohne",
    ),
    ("system.sync.upload_on", "documents", ", 1 chapter", ", 2 chapters", ", 1 Kapitel", ", 2 Kapitel"),
    (
        "system.sync.still_there",
        "documents",
        "(1 chapter is still in",
        "(2 chapters are still in",
        "(1 Kapitel liegt noch",
        "(2 Kapitel liegen noch",
    ),
    (
        "view_msg.wysiwyg.image_list",
        "count",
        "List of 1 image,",
        "List of 2 images,",
        "Liste mit 1 Bild,",
        "Liste mit 2 Bildern,",
    ),
    ("works_msg.sermon.passage_missing", "missing", "(1 verse);", "(2 verses);", "(1 Vers);", "(2 Verse);"),
    (
        "works_msg.sermon.verses_missing",
        "missing",
        "1 verse is missing",
        "2 verses are missing",
        "1 Vers fehlt",
        "2 Verse fehlen",
    ),
];

#[test]
fn a_count_of_one_selects_the_singular_in_english_and_german() {
    for (key, var, en_one, en_two, de_one, de_two) in SELECTED {
        for (lang, count, want) in
            [(Lang::En, 1, en_one), (Lang::En, 2, en_two), (Lang::De, 1, de_one), (Lang::De, 2, de_two)]
        {
            let text = i18n::t(lang, key, &[(var, Arg::from(count))]);
            assert!(
                text.contains(want),
                "{key} with {var} = {count} in {}: the selector chooses the right form ({want:?}), got {text:?}",
                lang.code()
            );
        }
    }
}

#[test]
fn a_text_with_several_counts_selects_each_one_on_its_own() {
    // The import report counts sections, proposals and posts independently; each count chooses its own form.
    let args = [("sections", Arg::from(1)), ("proposals", Arg::from(2)), ("posts", Arg::from(1))];
    let en = i18n::t(Lang::En, "export.word_import.report", &args);
    assert!(en.contains("1 section, 2 open proposals, 1 post."), "every count has its own selector: {en:?}");
    let de = i18n::t(Lang::De, "export.word_import.report", &args);
    assert!(de.contains("1 Abschnitt, 2 offene Vorschläge, 1 Beitrag."), "also in German: {de:?}");
    let args = [("ideas", Arg::from(1)), ("links", Arg::from(3))];
    let en = i18n::t(Lang::En, "ideas.map.stats", &args);
    assert_eq!(en, "1 idea · 3 links ·", "ideas and links are counted separately");
    // Two book pages are one sheet: the sheet count follows the page count, which is the number passed as a number.
    let facts = |pages: i64, sheets: &str| {
        i18n::t(Lang::En, "cover.page.facts", &[("book_pages", Arg::from(pages)), ("sheets", Arg::from(sheets))])
    };
    assert!(facts(1, "0.5").contains("= 1 book page (0.5 sheets)"), "one page: {:?}", facts(1, "0.5"));
    assert!(facts(2, "1").contains("= 2 book pages (1 sheet)"), "two pages are one sheet: {:?}", facts(2, "1"));
    assert!(facts(300, "150").contains("= 300 book pages (150 sheets)"), "many pages: {:?}", facts(300, "150"));
}

#[test]
fn a_template_hands_counts_to_the_selector_as_numbers() {
    // The template function `t` passes numbers as numbers (not as text), otherwise a selector could never choose
    // `[one]`. A book of the Bible with a single chapter (Obadiah) is the case from the review.
    let mut env = minijinja::Environment::new();
    i18n::install(&mut env);
    let source = r#"{{ t("sources.bible.chapters", chapters=b.chapters) }}|{{ t("findings.panel.open_hint", count=items | length) }}"#;
    let render = |lang: &str, chapters: usize, items: usize| {
        env.render_str(source, minijinja::context! { lang => lang, b => minijinja::context! { chapters => chapters }, items => vec![0; items] })
            .unwrap()
    };
    assert_eq!(render("en", 1, 1), "1 chapter|1 open finding in this section", "a count of 1 reads singular");
    assert_eq!(render("en", 21, 2), "21 chapters|2 open findings in this section", "other counts read plural");
    assert_eq!(render("de", 1, 1), "1 Kapitel|1 offener Befund in diesem Abschnitt", "German keeps its own forms");
}

/// Widget keys that are a singular/plural pair: the widget picks one by the count.
const WIDGET_PAIRS: &[(&str, &str)] = &[
    ("widget.search.total", "search.ts"),
    ("widget.typeset_settings.preview_pages", "typeset_settings.ts"),
    ("widget.typeset_settings.preview_errors", "typeset_settings.ts"),
    ("widget.typeset_settings.preview_warnings", "typeset_settings.ts"),
    ("widget.mindmap.delete_question_children", "mindmap.ts"),
    ("widget.mindmap.deleted_children", "mindmap.ts"),
    ("widget.mindmap.to_cloud_children", "mindmap.ts"),
];

#[test]
fn widget_plurals_are_two_keys_and_the_widget_names_both() {
    let web = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("web/src");
    for lang in ["en", "de"] {
        let values = values(lang);
        for (base, _) in WIDGET_PAIRS {
            assert!(!values.contains_key(*base), "{lang} {base}: the single key without a singular is gone");
            for suffix in ["_one", "_other"] {
                assert!(values.contains_key(&format!("{base}{suffix}")), "{lang} {base}{suffix}: both forms exist");
            }
        }
    }
    let en = values("en");
    assert_eq!(en["widget.search.total_one"], "{ $count } hit · { $range }", "the singular reads “1 hit”");
    assert_eq!(en["widget.typeset_settings.preview_errors_one"], "{ $count } error", "the singular reads “1 error”");
    for (base, file) in WIDGET_PAIRS {
        let source = std::fs::read_to_string(web.join(file)).unwrap();
        // Each form is a complete literal at its call site (the catalog checks find it there), chosen by `=== 1`.
        assert!(source.contains(&format!("\"{base}_one\"")), "{file} names {base}_one as a literal");
        assert!(source.contains(&format!("\"{base}_other\"")), "{file} names {base}_other as a literal");
        assert!(!source.contains(&format!("\"{base}\"")), "{file} no longer uses the plural-only key {base}");
    }
    let search = std::fs::read_to_string(web.join("search.ts")).unwrap();
    assert!(
        search.contains("one || section.hits.length === 1"),
        "replacing in a section with a single hit shows “Replaced”, not “1 places replaced”"
    );
    // The scripture check counts are labels in English that read for every number.
    assert_eq!(
        en["widget.scripture_check.counts"], "{ $matches } matching · { $differs } differing · { $others } other",
        "“1 differ” cannot occur"
    );
}

/// English counted texts without a selector that stay so, each with the reason why a count of 1 cannot occur or the
/// code picks a separate singular key.
const NO_SINGULAR_NEEDED: &[(&str, &str)] = &[
    ("assets_msg.request.purged", "days: the fixed retention period of the server"),
    ("assist_msg.compress.too_many_parts", "tokens: a context length, never 1"),
    ("assist_msg.variant.too_long", "tokens: a context length, never 1"),
    ("assist_msg.variant.paragraphs_too_long", "tokens: a context length, never 1"),
    ("connections_msg.keyring.ambiguous", "count: ambiguous means at least two entries"),
    ("connections_msg.detail.tool_pages", "pages: shown only for more than one page"),
    ("connections_msg.detail.tokens", "count: the context length of a model"),
    ("cover.page.facts_cover_pages", "count: the fixed number of cover pages (3)"),
    ("cover_msg.measure.timeout", "seconds: the fixed time limit"),
    ("editor.page.timing_title", "words_per_minute: 60 to 250 (timing.rs)"),
    ("editor.tree.timing_title", "words_per_minute: 60 to 250 (timing.rs)"),
    ("editor.lektor_tree.timing_title", "words_per_minute: 60 to 250 (timing.rs)"),
    ("export_msg.pdf.failed", "count: pdf.rs picks export_msg.pdf.failed_one for 1"),
    ("findings.page.truncated", "limit, total: the fixed limit and a larger total"),
    ("findings.page.old_truncated", "total: larger than the limit of shown findings"),
    ("ideas.read_in.file", "nodes, depth: fixed import limits"),
    ("ideas_msg.near.capped", "count: larger than the limit"),
    ("ideas_msg.import.too_deep", "deepest: deeper than the import limit"),
    ("reference_msg.search.range_picked", "count: search picks reference_msg.search.range_picked_one for 1"),
    ("search.page.truncated", "total: larger than the shown hits"),
    ("settings.data.size_bytes", "size_bytes: the size of a data folder with a database"),
    ("settings.assistants.lector_tokens", "tokens: a context length, never 1"),
    ("settings.assistants.context_tokens", "context_tokens: a context length, never 1"),
    ("settings.work.timing_default", "default_wpm: the fixed default speaking rate"),
    ("settings_msg.work.revisions_error", "min_hi: bounds of a setting"),
    ("sources_msg.images.only_first", "count: the fixed page size"),
    ("sources_msg.commentary.entries", "count: bibles.rs picks sources_msg.commentary.one_entry for 1"),
    ("store_msg.exchange.glossary_max", "count: larger than the limit"),
    ("store_msg.exchange.register_title", "chars: larger than the limit"),
    ("system.consent.many", "requests: consent.html uses system.consent.one for a single request"),
    ("system_msg.models.context_lower", "reported: a context length, never 1"),
    ("view_msg.menu.sections", "count: menu.rs picks view_msg.menu.one_section for 1"),
];

#[test]
fn every_counted_english_text_has_a_singular() {
    let nouns = "chapters|sections|verses|words|errors|warnings|pages|hits|matches|characters|entries|titles|nodes|\
                 sub-nodes|notes|ideas|links|revisions|images|solutions|findings|proposals|posts|documents|days|\
                 places|sheets|blocks|terms|bytes|tokens|seconds|minutes|requests|backups|levels|quotations|differ";
    let counted = Regex::new(&format!(r"\{{ \$([a-z_]+) \}}\s+(?:[A-Za-z-]+\s+)?(?:{nouns})\b")).unwrap();
    // A limit (`at most { $max } characters`) is a fixed number larger than 1 and needs no singular.
    let limit = Regex::new(r"^(max|min|max_[a-z_]+|mib)$").unwrap();
    let allowed: BTreeMap<&str, &str> = NO_SINGULAR_NEEDED.iter().copied().collect();
    let mut problems = Vec::new();
    for (key, value) in values("en") {
        // A selector, or the plural half of a pair whose singular is a key of its own.
        if value.contains("->") || key.ends_with("_other") || key.ends_with("_many") || key.contains("_other_") {
            continue;
        }
        let unbounded = counted.captures_iter(&value).any(|found| !limit.is_match(&found[1]));
        if unbounded && !allowed.contains_key(key.as_str()) {
            problems.push(format!("{key} = {value}"));
        }
    }
    assert!(
        problems.is_empty(),
        "counted English texts without a singular (selector or _one/_other):\n{}",
        problems.join("\n")
    );
    let en = values("en");
    for (key, reason) in NO_SINGULAR_NEEDED {
        assert!(en.contains_key(*key), "{key} is listed ({reason}) but no longer exists – remove it from the list");
    }
}
