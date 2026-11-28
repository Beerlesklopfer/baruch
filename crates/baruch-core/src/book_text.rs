// SPDX-License-Identifier: AGPL-3.0-or-later
//! Book catalog (Round 39b, interface R39b-S1): labels and generated words that go into a book output (EPUB,
//! Typst/PDF, DOCX, notes sheet) and the words Baruch writes for read-back, in the WORK language
//! ([`WorkLanguage`]), never in the interface language.
//!
//! The catalogs `locales/en/book.ftl` (base) and `locales/de/book.ftl` are embedded at compile time. Format: a strict
//! subset of Fluent – one entry per line `key = value`, lines starting with `#` and blank lines are skipped, no
//! multi-line values, selectors, terms or attributes; placeholders exactly `{ $name }`. The value is the text after
//! the first ` = `, trimmed. Keys match `^book\.[a-z0-9_]+(\.[a-z0-9_]+)+$` (one listed exception: the contract
//! seed `book.untitled`). Values are plain text: callers escape for XML/XHTML/Typst/OOXML themselves.
//!
//! The German values are verbatim the literals the outputs wrote before Round 39b, so German output stays
//! byte-equal. Lookup order: the work language, then English, then the key itself.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::LazyLock;

use crate::WorkLanguage;

/// Catalog files, embedded: (language, content).
pub const BOOK_CATALOGS: [(WorkLanguage, &str); 2] = [
    (WorkLanguage::En, include_str!("../locales/en/book.ftl")),
    (WorkLanguage::De, include_str!("../locales/de/book.ftl")),
];

/// One parsed catalog: entries by key, plus the problems found while parsing.
struct Catalog {
    lang: WorkLanguage,
    entries: BTreeMap<&'static str, &'static str>,
    problems: Vec<String>,
}

/// Both catalogs, parsed once.
static CATALOGS: LazyLock<Vec<Catalog>> =
    LazyLock::new(|| BOOK_CATALOGS.iter().map(|(lang, content)| parse(*lang, content)).collect());

/// Parses one catalog file; malformed lines, bad keys, duplicates and empty values become problems.
fn parse(lang: WorkLanguage, content: &'static str) -> Catalog {
    let mut entries = BTreeMap::new();
    let mut problems = Vec::new();
    let file = format!("locales/{}/book.ftl", lang.as_str());
    for (index, line) in content.lines().enumerate() {
        let number = index + 1;
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once(" = ") else {
            problems.push(format!("{file}:{number}: malformed line (no ` = `): {line}"));
            continue;
        };
        let (key, value) = (key.trim(), value.trim());
        if !key_is_valid(key) {
            problems.push(format!("{file}:{number}: bad key `{key}`"));
            continue;
        }
        if value.is_empty() {
            problems.push(format!("{file}:{number}: empty value for `{key}`"));
        }
        if let Err(message) = placeholders(value) {
            problems.push(format!("{file}:{number}: `{key}`: {message}"));
        }
        if entries.insert(key, value).is_some() {
            problems.push(format!("{file}:{number}: duplicate key `{key}`"));
        }
    }
    Catalog { lang, entries, problems }
}

/// Seed keys of the Round 39b contract that do not match the key pattern (the contract names them explicitly and
/// other tasks code against these names). No new key may be added here; new keys follow the pattern.
const KEY_PATTERN_EXCEPTIONS: [&str; 1] = ["book.untitled"];

/// `^book\.[a-z0-9_]+(\.[a-z0-9_]+)+$`, without a regex crate (baruch-core keeps only `serde` and `similar`), or one
/// of [`KEY_PATTERN_EXCEPTIONS`].
fn key_is_valid(key: &str) -> bool {
    if KEY_PATTERN_EXCEPTIONS.contains(&key) {
        return true;
    }
    let mut segments = key.split('.');
    let first = segments.next();
    let rest: Vec<&str> = segments.collect();
    first == Some("book")
        && rest.len() >= 2
        && rest.iter().all(|segment| {
            !segment.is_empty() && segment.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
        })
}

/// Names of the placeholders `{ $name }` in `value`; an opening `{` that does not start a well-formed placeholder
/// (or a stray `}`) is an error, because the format allows nothing else in braces.
fn placeholders(value: &str) -> Result<BTreeSet<&str>, String> {
    let mut names = BTreeSet::new();
    let mut rest = value;
    while let Some(start) = rest.find(['{', '}']) {
        if rest[start..].starts_with('}') {
            return Err(format!("stray `}}` in `{value}`"));
        }
        let after = &rest[start..];
        let Some(end) = after.find('}') else {
            return Err(format!("unclosed `{{` in `{value}`"));
        };
        let inner = &after[..=end];
        let name = inner.strip_prefix("{ $").and_then(|tail| tail.strip_suffix(" }")).filter(|name| {
            !name.is_empty() && name.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
        });
        match name {
            Some(name) => {
                names.insert(name);
            }
            None => return Err(format!("placeholder `{inner}` is not of the form `{{ $name }}`")),
        }
        rest = &after[end + 1..];
    }
    Ok(names)
}

/// The parsed catalog of `lang`.
fn catalog(lang: WorkLanguage) -> &'static Catalog {
    CATALOGS.iter().find(|catalog| catalog.lang == lang).expect("every WorkLanguage has a book catalog")
}

/// Text of `key` in `lang`, `{ $name }` filled from `args` (unknown placeholders stay verbatim);
/// missing in `lang` -> English text; missing in English -> the key itself.
pub fn book_t(lang: WorkLanguage, key: &str, args: &[(&str, &str)]) -> String {
    let Some(template) = catalog(lang).entries.get(key).or_else(|| catalog(WorkLanguage::En).entries.get(key)) else {
        return key.to_owned();
    };
    let mut text = (*template).to_owned();
    for (name, value) in args {
        text = text.replace(&format!("{{ ${name} }}"), value);
    }
    text
}

/// Is `key` in the English book catalog?
pub fn book_exists(key: &str) -> bool {
    catalog(WorkLanguage::En).entries.contains_key(key)
}

/// All keys of the English book catalog, sorted.
pub fn book_keys() -> Vec<&'static str> {
    catalog(WorkLanguage::En).entries.keys().copied().collect()
}

/// Problems of both catalogs (empty = fine): malformed line, bad key, duplicate key, empty value,
/// key missing in one language, different placeholder sets per key.
pub fn book_catalog_problems() -> Vec<String> {
    let mut problems: Vec<String> = CATALOGS.iter().flat_map(|catalog| catalog.problems.iter().cloned()).collect();
    let english = catalog(WorkLanguage::En);
    for other in CATALOGS.iter().filter(|catalog| catalog.lang != WorkLanguage::En) {
        let code = other.lang.as_str();
        for key in english.entries.keys().filter(|key| !other.entries.contains_key(*key)) {
            problems.push(format!("`{key}` is missing in locales/{code}/book.ftl"));
        }
        for key in other.entries.keys().filter(|key| !english.entries.contains_key(*key)) {
            problems.push(format!("`{key}` is missing in locales/en/book.ftl"));
        }
        for (key, value) in &other.entries {
            let Some(base) = english.entries.get(key) else { continue };
            if let (Ok(ours), Ok(theirs)) = (placeholders(base), placeholders(value))
                && ours != theirs
            {
                problems.push(format!("`{key}`: placeholders differ (en {ours:?}, {code} {theirs:?})"));
            }
        }
    }
    problems
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalogs_have_no_problems() {
        assert_eq!(
            book_catalog_problems(),
            Vec::<String>::new(),
            "both book catalogs must parse cleanly and carry the same keys with the same placeholders"
        );
    }

    #[test]
    fn keys_are_sorted_and_all_valid() {
        let keys = book_keys();
        assert!(!keys.is_empty(), "the English catalog carries the seed keys");
        assert!(keys.windows(2).all(|pair| pair[0] < pair[1]), "book_keys promises a sorted list without duplicates");
        assert!(keys.iter().all(|key| key_is_valid(key)), "parse only keeps keys of the documented form");
        assert!(keys.iter().all(|key| book_exists(key)), "book_exists answers for every listed key");
        assert!(!book_exists("book.no_such.key"), "an unknown key is not in the catalog");
    }

    #[test]
    fn key_rule_matches_the_contract_pattern() {
        assert!(key_is_valid("book.toc.title"), "two segments after `book` are the minimum");
        assert!(key_is_valid("book.numbering.legal_number"), "underscores are allowed in a segment");
        assert!(!key_is_valid("book.untitled_only"), "one segment after `book` is too short for the pattern");
        assert!(key_is_valid("book.untitled"), "the contract seed `book.untitled` is the one listed exception");
        assert!(!key_is_valid("book.Toc.title"), "upper case is not allowed");
        assert!(!key_is_valid("book..title"), "an empty segment is not allowed");
        assert!(!key_is_valid("export.toc.title"), "every key starts with `book.`");
    }

    #[test]
    fn parser_reports_each_kind_of_problem() {
        let broken =
            "# comment\n\nbook.a.b = one\nbook.a.b = two\nno separator\nBook.x.y = z\nbook.a.c = \nbook.a.d = { $x\n";
        let problems = parse(WorkLanguage::En, broken).problems;
        let has = |needle: &str| problems.iter().any(|problem| problem.contains(needle));
        assert!(has("duplicate key `book.a.b`"), "a second entry with the same key is a problem: {problems:?}");
        assert!(has("malformed line"), "a line without ` = ` is a problem: {problems:?}");
        assert!(has("bad key `Book.x.y`"), "a key outside the pattern is a problem: {problems:?}");
        assert!(
            has("empty value for `book.a.c`"),
            "a value that is blank after the separator is a problem: {problems:?}"
        );
        assert!(has("unclosed"), "a brace that never closes is a problem: {problems:?}");
        assert_eq!(
            problems.len(),
            5,
            "comments and blank lines are skipped, each broken line counts once: {problems:?}"
        );
    }

    #[test]
    fn empty_value_is_a_problem() {
        let problems = parse(WorkLanguage::En, "book.a.b =  x\nbook.a.c =  \u{20}\u{20}\n").problems;
        assert!(
            problems.iter().any(|problem| problem.contains("empty value for `book.a.c`")),
            "a value that is only blanks after the separator is empty: {problems:?}"
        );
    }

    #[test]
    fn placeholders_are_parsed_strictly() {
        assert_eq!(placeholders("Part { $number }").unwrap(), BTreeSet::from(["number"]), "the one allowed form");
        assert!(placeholders("Part {$number}").is_err(), "exactly one space inside each brace");
        assert!(placeholders("Part { number }").is_err(), "a placeholder names a variable with `$`");
        assert!(placeholders("a } b").is_err(), "a stray closing brace is not plain text in this format");
        assert!(placeholders("no braces at all").unwrap().is_empty(), "plain text has no placeholders");
    }

    #[test]
    fn placeholders_are_filled_and_unknown_ones_stay() {
        assert_eq!(
            book_t(WorkLanguage::En, "book.numbering.chapter", &[("number", "3")]),
            "Chapter 3",
            "the argument replaces `{{ $number }}`"
        );
        assert_eq!(
            book_t(WorkLanguage::De, "book.cover.alt", &[("title", "Leise Abhängigkeit")]),
            "Umschlag: Leise Abhängigkeit",
            "the German value is filled the same way"
        );
        assert_eq!(
            book_t(WorkLanguage::En, "book.numbering.part", &[]),
            "Part { $number }",
            "without an argument the placeholder stays verbatim, visible instead of silently empty"
        );
        assert_eq!(
            book_t(WorkLanguage::En, "book.toc.title", &[("unused", "x")]),
            "Contents",
            "an argument without a placeholder changes nothing"
        );
    }

    #[test]
    fn fallback_goes_from_work_language_to_english_to_key() {
        let english = parse(WorkLanguage::En, "book.a.only_en = English only\nbook.a.both = both en\n");
        let german = parse(WorkLanguage::De, "book.a.both = beide de\n");
        // The real catalogs are complete, so the fallback is checked on the same lookup rule with test catalogs.
        let lookup = |lang: WorkLanguage, key: &str| -> String {
            let primary = if lang == WorkLanguage::De { &german } else { &english };
            primary
                .entries
                .get(key)
                .or_else(|| english.entries.get(key))
                .map_or_else(|| key.to_owned(), |v| (*v).to_owned())
        };
        assert_eq!(lookup(WorkLanguage::De, "book.a.both"), "beide de", "present in the work language: that text");
        assert_eq!(lookup(WorkLanguage::De, "book.a.only_en"), "English only", "missing in German: English text");
        assert_eq!(lookup(WorkLanguage::De, "book.a.none"), "book.a.none", "missing everywhere: the key itself");
        assert_eq!(
            book_t(WorkLanguage::De, "book.no_such.key", &[]),
            "book.no_such.key",
            "book_t itself returns the key when no catalog has it"
        );
    }

    #[test]
    fn english_seeds_have_no_german_left() {
        for key in book_keys() {
            let text = book_t(WorkLanguage::En, key, &[]);
            assert!(
                !text.contains(['ä', 'ö', 'ü', 'Ä', 'Ö', 'Ü', 'ß']),
                "English book text of `{key}` must not carry German letters: {text}"
            );
        }
    }

    #[test]
    fn german_seeds_equal_the_literals_they_replace() {
        // Copied literals (baruch-core cannot depend on baruch-typst, baruch-epub or baruch-server): byte-equal
        // German output requires exactly the wording the outputs wrote before Round 39b.
        let expected = [
            ("book.toc.title", "Inhalt"),
            ("book.cover.title", "Umschlag"),
            ("book.cover.alt", "Umschlag: { $title }"),
            ("book.title_page.title", "Titel"),
            ("book.untitled", "Ohne Titel"),
            ("book.glossary.title", "Glossar"),
            ("book.index.empty", "Keine Einträge."),
            ("book.figure.supplement", "Abbildung"),
            ("book.footnote.back", "zurück zum Text"),
            ("book.xref.page_abbr", "S."),
            // baruch_typst::typeset::model::DEFAULT_DRAFT_TEXT
            ("book.draft.watermark", "ENTWURF"),
            ("book.numbering.part", "Teil { $number }"),
            ("book.numbering.chapter", "Kapitel { $number }"),
            ("book.numbering.introduction", "Einleitung"),
            ("book.numbering.conclusion", "Schluss"),
            ("book.numbering.legal_paragraph", "Abs."),
            ("book.numbering.legal_number", "Nr."),
            // baruch_typst::cover::DNB_TITLE
            ("book.imprint.dnb_title", "Bibliografische Information der Deutschen Nationalbibliothek"),
            // baruch_typst::cover::DNB_TEXT
            (
                "book.imprint.dnb_text",
                "Die Deutsche Nationalbibliothek verzeichnet diese Publikation in der Deutschen \
                 Nationalbibliografie; detaillierte bibliografische Daten sind im Internet über https://dnb.de abrufbar.",
            ),
            // baruch_typst::cover::TYPESET_WITH
            ("book.imprint.typeset_with", "Gesetzt mit Baruch"),
            ("book.imprint.cover_credit", "Umschlaggestaltung und Bildnachweis: { $credit }"),
            ("book.cover.isbn_placeholder", "ISBN und Strichcode"),
            ("book.scripture.wording_unchecked", "Wortlaut ungeprüft"),
            ("book.claims.not_supported", "nicht belegt"),
            ("book.claims.check_reference", "Angabe prüfen"),
        ];
        for (key, literal) in expected {
            assert_eq!(
                catalog(WorkLanguage::De).entries.get(key).copied(),
                Some(literal),
                "byte-equal German output: `{key}` must be the literal the code wrote before Round 39b"
            );
        }
        assert_eq!(expected.len(), 25, "the contract names 25 seed keys");
        assert!(expected.iter().all(|(key, _)| book_exists(key)), "every seed key is in the English base catalog");
    }

    #[test]
    fn english_seeds_are_the_contract_wording() {
        let expected = [
            ("book.toc.title", "Contents"),
            ("book.cover.title", "Cover"),
            ("book.cover.alt", "Cover: { $title }"),
            ("book.title_page.title", "Title page"),
            ("book.untitled", "Untitled"),
            ("book.glossary.title", "Glossary"),
            ("book.index.empty", "No entries."),
            ("book.figure.supplement", "Figure"),
            ("book.footnote.back", "back to the text"),
            ("book.xref.page_abbr", "p."),
            ("book.draft.watermark", "DRAFT"),
            ("book.numbering.part", "Part { $number }"),
            ("book.numbering.chapter", "Chapter { $number }"),
            ("book.numbering.introduction", "Introduction"),
            ("book.numbering.conclusion", "Conclusion"),
            ("book.numbering.legal_paragraph", "para."),
            ("book.numbering.legal_number", "no."),
            ("book.imprint.dnb_title", "Bibliographic information published by the Deutsche Nationalbibliothek"),
            (
                "book.imprint.dnb_text",
                "The Deutsche Nationalbibliothek lists this publication in the Deutsche Nationalbibliografie; detailed \
                 bibliographic data are available on the Internet at https://dnb.de.",
            ),
            ("book.imprint.typeset_with", "Typeset with Baruch"),
            ("book.imprint.cover_credit", "Cover design and image credit: { $credit }"),
            ("book.cover.isbn_placeholder", "ISBN and barcode"),
            ("book.scripture.wording_unchecked", "wording unchecked"),
            ("book.claims.not_supported", "not supported"),
            ("book.claims.check_reference", "check reference"),
        ];
        for (key, text) in expected {
            assert_eq!(
                catalog(WorkLanguage::En).entries.get(key).copied(),
                Some(text),
                "the English seed of `{key}` is fixed by the Round 39b contract"
            );
        }
    }
}
