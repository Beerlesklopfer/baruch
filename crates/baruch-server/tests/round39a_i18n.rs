// SPDX-License-Identifier: AGPL-3.0-or-later
//! Round 39a (interface R39a-S1): checks of the catalogs and of the localisation scaffold (contract §6).
//!
//! (1)–(6) the catalogs themselves and the keys used in templates, widgets and Rust; (7) the literal finder over
//! templates and widgets with its allow list `scripts/i18n-literal-allow.txt`; (8) resolution of the language and the
//! switch `POST /einstellungen/sprache`; (9) the widget catalog in the page head; (10) dates and numbers per language.

use crate::common;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use baruch_server::auth::{AccountKind, Identity, Who};
use baruch_server::i18n::{self, CATALOGS, Lang};
use regex::Regex;
use serde_json::Value;

// ---- helpers ---------------------------------------------------------------------------------------------------

fn manifest() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn repo() -> PathBuf {
    manifest().join("../..").canonicalize().expect("repository root")
}

/// Files below `dir` with the extension `ext`, recursively, sorted.
fn files(dir: &Path, ext: &str) -> Vec<PathBuf> {
    fn walk(dir: &Path, ext: &str, found: &mut Vec<PathBuf>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                if path.file_name().is_some_and(|n| n != "node_modules") {
                    walk(&path, ext, found);
                }
            } else if path.extension().is_some_and(|e| e == ext) {
                found.push(path);
            }
        }
    }
    let mut found = Vec::new();
    walk(dir, ext, &mut found);
    found.sort();
    found
}

/// `(repo-relative path, content)` of all templates, widget sources and server sources.
fn sources(kind: &str) -> Vec<(String, String)> {
    let (dir, ext) = match kind {
        "templates" => (manifest().join("templates"), "html"),
        "web" => (manifest().join("web/src"), "ts"),
        _ => (manifest().join("src"), "rs"),
    };
    let root = repo();
    files(&dir, ext)
        .into_iter()
        .map(|path| {
            let rel = path.canonicalize().unwrap().strip_prefix(&root).unwrap().to_string_lossy().into_owned();
            (rel, std::fs::read_to_string(&path).unwrap())
        })
        .collect()
}

/// Keys of one language: stem → keys in file order, plus key → raw value.
fn catalog(lang: &str) -> (BTreeMap<String, Vec<String>>, BTreeMap<String, String>) {
    let mut by_file = BTreeMap::new();
    let mut values = BTreeMap::new();
    for (code, stem, source) in CATALOGS.iter().filter(|(code, _, _)| *code == lang) {
        let entries = i18n::catalog_entries(source);
        by_file.insert(stem.to_string(), entries.iter().map(|(key, _)| key.clone()).collect());
        for (key, value) in entries {
            assert!(values.insert(key.clone(), value).is_none(), "{key} appears twice in the {code} catalogs");
        }
    }
    (by_file, values)
}

async fn body(response: axum::response::Response) -> String {
    String::from_utf8(axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap().to_vec()).unwrap()
}

/// `<html …>` tag of a page.
fn html_tag(page: &str) -> &str {
    let start = page.find("<html").expect("page has an <html> tag");
    &page[start..start + page[start..].find('>').unwrap() + 1]
}

fn get(uri: &str) -> Request<Body> {
    Request::get(uri).header(header::HOST, "127.0.0.1:8765").body(Body::empty()).unwrap()
}

fn form(uri: &str, body: &str) -> Request<Body> {
    Request::post(uri)
        .header(header::HOST, "127.0.0.1:8765")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(body.to_owned()))
        .unwrap()
}

fn with_cookie(mut request: Request<Body>, cookie: &str) -> Request<Body> {
    request.headers_mut().insert(header::COOKIE, cookie.parse().unwrap());
    request
}

fn account(id: i64, kind: AccountKind) -> Who {
    Who::Account(Identity {
        id,
        name: format!("konto{id}"),
        display: format!("Konto {id}"),
        kind,
        handle: format!("@lektorat:konto{id}"),
    })
}

fn location(headers: &axum::http::HeaderMap) -> &str {
    headers.get(header::LOCATION).and_then(|v| v.to_str().ok()).unwrap_or("")
}

fn set_cookies(headers: &axum::http::HeaderMap) -> Vec<String> {
    headers.get_all(header::SET_COOKIE).iter().map(|v| v.to_str().unwrap().to_owned()).collect()
}

// ---- (1)–(6) catalogs ------------------------------------------------------------------------------------------

#[test]
fn both_languages_have_the_same_files_and_keys_and_the_bundles_build() {
    assert!(i18n::catalog_problems().is_empty(), "Fluent must read every catalog: {:?}", i18n::catalog_problems());
    let (en, _) = catalog("en");
    let (de, _) = catalog("de");
    assert!(en.contains_key("common"), "the shared file common.ftl exists (S1)");
    assert_eq!(
        en.keys().collect::<Vec<_>>(),
        de.keys().collect::<Vec<_>>(),
        "every area file exists in en and de (contract §4: both files always exist)"
    );
    let pattern = Regex::new(i18n::KEY_PATTERN).unwrap();
    for (stem, keys) in &en {
        assert_eq!(Some(keys), de.get(stem), "{stem}.ftl: same keys in the same order in en and de (contract §4)");
        for key in keys {
            assert!(pattern.is_match(key), "{key}: a key is lower-case dotted segments ({})", i18n::KEY_PATTERN);
            assert_eq!(
                key.split('.').next(),
                Some(stem.as_str()),
                "{key}: the first segment names its file {stem}.ftl"
            );
        }
    }
    for (code, stem, _) in CATALOGS {
        assert!(matches!(*code, "en" | "de"), "{code}/{stem}.ftl: only en and de are offered in Round 39a");
    }
}

#[test]
fn every_key_has_the_same_placeables_in_both_languages() {
    let (_, en) = catalog("en");
    let (_, de) = catalog("de");
    for (key, value) in &en {
        let want: BTreeSet<String> = i18n::placeables(value).into_iter().collect();
        let got: BTreeSet<String> = i18n::placeables(&de[key]).into_iter().collect();
        assert_eq!(want, got, "{key}: the German text must use the same variables as the English one");
    }
}

#[test]
fn only_the_allowed_fluent_is_used_and_widget_texts_stay_simple() {
    let simple = Regex::new(r"^\{\s*\$[A-Za-z0-9_]+\s*\}$").unwrap();
    let placeable = Regex::new(r"\{[^{}]*\}").unwrap();
    let forbidden = Regex::new(r"\{\s*-|\{\s*[A-Za-z]|(?m)^\s+\.[a-z]").unwrap();
    for lang in ["en", "de"] {
        let (_, values) = catalog(lang);
        for (key, value) in &values {
            assert!(
                !forbidden.is_match(value),
                "{lang} {key}: no terms, message references, functions or attributes (contract §4): {value:?}"
            );
            if key.starts_with(i18n::WIDGET_PREFIX) {
                assert!(!value.contains("->"), "{lang} {key}: widget texts have no selectors (two keys for a plural)");
                for found in placeable.find_iter(value) {
                    assert!(
                        simple.is_match(found.as_str()),
                        "{lang} {key}: only {{ $name }} in widget texts: {value:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn only_markup_keys_carry_markup_and_only_the_allowed_tags() {
    let tag = Regex::new(r"<[^>]*>").unwrap();
    for lang in ["en", "de"] {
        let (_, values) = catalog(lang);
        for (key, value) in &values {
            if key.ends_with("_html") {
                for found in tag.find_iter(value) {
                    assert!(
                        i18n::MARKUP_TAGS.contains(&found.as_str()),
                        "{lang} {key}: only {:?} are allowed, found {}",
                        i18n::MARKUP_TAGS,
                        found.as_str()
                    );
                }
            } else {
                assert!(!value.contains('<'), "{lang} {key}: markup only in keys ending in _html: {value:?}");
            }
        }
    }
}

/// Keys used at the call sites (contract §6 (5)): `t("…")` in templates and widgets, `i18n::t(lang, "…")` and
/// `tc("…")` in the server.
fn used_keys() -> Vec<(String, String)> {
    let template = Regex::new(r#"\bt\(\s*["']([^"']+)["']"#).unwrap();
    let rust = Regex::new(r#"\bi18n::t\(\s*[^,()]+,\s*"([^"]+)"|\btc\(\s*"([^"]+)""#).unwrap();
    let mut used = Vec::new();
    for (kind, re) in [("templates", &template), ("web", &template), ("src", &rust)] {
        for (path, text) in sources(kind) {
            for found in re.captures_iter(&text) {
                let key = found.get(1).or_else(|| found.get(2)).unwrap().as_str().to_owned();
                used.push((path.clone(), key));
            }
        }
    }
    used
}

#[test]
fn every_key_used_exists_in_the_english_catalog() {
    let used = used_keys();
    assert!(used.iter().any(|(_, key)| key == "common.lang.en"), "the finder sees the switch in base.html: {used:?}");
    for (path, key) in &used {
        assert!(i18n::exists(key), "{path}: key {key} is used but missing in locales/en (shows as raw key)");
    }
}

#[test]
fn every_catalog_key_is_used_somewhere() {
    let mut text = String::new();
    for kind in ["templates", "web", "src"] {
        for (_, content) in sources(kind) {
            text.push_str(&content);
        }
    }
    let (_, en) = catalog("en");
    for key in en.keys() {
        assert!(
            text.contains(&format!("\"{key}\"")) || text.contains(&format!("'{key}'")),
            "{key} appears quoted nowhere in templates, web/src or src – an orphan (contract §6 (6))"
        );
    }
}

// ---- (7) literal finder ----------------------------------------------------------------------------------------

/// German stop words of the finder (whole words, case-insensitive); with umlauts, sharp s and German quotes they mark a
/// literal as German.
const STOP_WORDS: &[&str] = &[
    "und",
    "oder",
    "der",
    "die",
    "das",
    "nicht",
    "mit",
    "ist",
    "ein",
    "eine",
    "einen",
    "einem",
    "dem",
    "den",
    "zum",
    "zur",
    "bitte",
    "noch",
    "nur",
    "wird",
    "werden",
    "kein",
    "keine",
    "sind",
    "sich",
    "auch",
    "alle",
    "aus",
    "bei",
    "wie",
    "wenn",
    "dann",
    "hier",
    "schon",
    "mehr",
    "neu",
    "neues",
    "neue",
    "werk",
    "werke",
    "buch",
    "kapitel",
    "abschnitt",
    "fassung",
    "fassungen",
    "einstellungen",
    "speichern",
    "abbrechen",
    "zeigen",
    "suchen",
    "vorschlag",
    "vorschläge",
    "beitrag",
    "beiträge",
    "gliederung",
    "befunde",
    "quelle",
    "quellen",
    "ansicht",
    "einfügen",
];

fn german() -> Regex {
    Regex::new(&format!("[äöüÄÖÜß„“‚‘]|(?i)\\b(?:{})\\b", STOP_WORDS.join("|"))).unwrap()
}

fn normalise(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// String literals of a template expression (`{{ … }}`, `{% … %}`) without the keys of `t(…)`.
fn jinja_literals(block: &str, out: &mut Vec<String>) {
    let literal = Regex::new(r#""((?:[^"\\]|\\.)*)"|'((?:[^'\\]|\\.)*)'"#).unwrap();
    for found in literal.captures_iter(block) {
        let whole = found.get(0).unwrap();
        let before = block[..whole.start()].trim_end();
        if before.ends_with("t(") && (before.len() == 2 || !before[..before.len() - 2].ends_with(char::is_alphanumeric))
        {
            continue;
        }
        out.push(found.get(1).or_else(|| found.get(2)).unwrap().as_str().to_owned());
    }
}

/// Replaces the template expressions of `text` by a space and collects their string literals.
fn strip_jinja(text: &str, literals: &mut Vec<String>) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(start) = rest.find("{{").into_iter().chain(rest.find("{%")).min() {
        out.push_str(&rest[..start]);
        let close = if rest[start..].starts_with("{{") { "}}" } else { "%}" };
        let end = rest[start + 2..].find(close).map_or(rest.len(), |e| start + 2 + e + 2);
        jinja_literals(&rest[start..end], literals);
        out.push(' ');
        rest = &rest[end..];
    }
    out.push_str(rest);
    out
}

/// Attributes whose values are never shown to the user (contract §6 (7)).
const NOT_TEXT_ATTRIBUTES: &[&str] = &[
    "href",
    "src",
    "action",
    "method",
    "class",
    "id",
    "name",
    "for",
    "type",
    "style",
    "pattern",
    "accept",
    "rel",
    "target",
    "role",
    "data-widget",
];

/// Candidate literals of a template: text nodes, shown attribute values and string literals of expressions.
fn template_literals(source: &str) -> Vec<String> {
    let comment = Regex::new(r"(?s)\{#.*?#\}|<!--.*?-->").unwrap();
    let script = Regex::new(r"(?is)<script\b.*?</script\s*>|<style\b.*?</style\s*>").unwrap();
    let attribute = Regex::new(r#"([A-Za-z_:][-A-Za-z0-9_:.]*)\s*=\s*(?:"([^"]*)"|'([^']*)')"#).unwrap();
    let cleaned = comment.replace_all(source, "");
    let cleaned = script.replace_all(&cleaned, "");
    let s: &str = &cleaned;
    let mut literals = Vec::new();
    let mut text = String::new();
    let mut i = 0;
    let bytes = s.as_bytes();
    while i < s.len() {
        let rest = &s[i..];
        if rest.starts_with("{{") || rest.starts_with("{%") {
            let close = if rest.starts_with("{{") { "}}" } else { "%}" };
            let end = rest[2..].find(close).map_or(rest.len(), |e| e + 4);
            jinja_literals(&rest[..end], &mut literals);
            text.push(' ');
            i += end;
        } else if bytes[i] == b'<' && rest[1..].starts_with(|c: char| c.is_ascii_alphabetic() || c == '/' || c == '!') {
            literals.push(std::mem::take(&mut text));
            // The tag ends at the first `>` outside quotes and template expressions.
            let mut j = 1;
            let mut quote: Option<u8> = None;
            while i + j < s.len() {
                if quote.is_none() && bytes[i + j] == b'{' && matches!(bytes.get(i + j + 1), Some(b'{' | b'%')) {
                    let here = &s[i + j..];
                    let close = if here.starts_with("{{") { "}}" } else { "%}" };
                    j += here[2..].find(close).map_or(here.len(), |e| e + 4);
                    continue;
                }
                let b = bytes[i + j];
                match quote {
                    Some(q) if b == q => quote = None,
                    None if b == b'"' || b == b'\'' => quote = Some(b),
                    None if b == b'>' => break,
                    _ => {}
                }
                j += 1;
            }
            let tag = &s[i..(i + j + 1).min(s.len())];
            let plain = strip_jinja(tag, &mut literals);
            let name = plain[1..].split(|c: char| c.is_whitespace() || c == '>' || c == '/').next().unwrap_or("");
            let name = name.to_ascii_lowercase();
            let input_type = attribute
                .captures_iter(&plain)
                .find(|c| c[1].eq_ignore_ascii_case("type"))
                .map(|c| c.get(2).or_else(|| c.get(3)).unwrap().as_str().to_ascii_lowercase());
            let button = name == "button"
                || (name == "input" && matches!(input_type.as_deref(), Some("button" | "submit" | "reset")));
            for found in attribute.captures_iter(&plain) {
                let attr = found[1].to_ascii_lowercase();
                if NOT_TEXT_ATTRIBUTES.contains(&attr.as_str()) || (attr == "value" && !button) {
                    continue;
                }
                literals.push(found.get(2).or_else(|| found.get(3)).unwrap().as_str().to_owned());
            }
            i += j + 1;
        } else {
            let c = rest.chars().next().unwrap();
            text.push(c);
            i += c.len_utf8();
        }
    }
    literals.push(text);
    literals
}

/// Candidate literals of a TypeScript file: string and template literals outside comments and import lines.
fn ts_literals(source: &str) -> Vec<String> {
    fn string(chars: &[char], mut i: usize, quote: char) -> (String, usize) {
        let mut out = String::new();
        while i < chars.len() && chars[i] != quote && chars[i] != '\n' {
            if chars[i] == '\\' && i + 1 < chars.len() {
                out.push(chars[i]);
                i += 1;
            }
            out.push(chars[i]);
            i += 1;
        }
        (out, i + 1)
    }
    fn template(chars: &[char], mut i: usize, out: &mut Vec<String>) -> usize {
        let mut text = String::new();
        while i < chars.len() && chars[i] != '`' {
            if chars[i] == '\\' && i + 1 < chars.len() {
                text.push(chars[i]);
                text.push(chars[i + 1]);
                i += 2;
            } else if chars[i] == '$' && chars.get(i + 1) == Some(&'{') {
                text.push(' ');
                i = code(chars, i + 2, Some('}'), out);
            } else {
                text.push(chars[i]);
                i += 1;
            }
        }
        out.push(text);
        i + 1
    }
    /// Code until `end` (closing brace of `${…}`) or the end of the file; returns the index after it.
    fn code(chars: &[char], mut i: usize, end: Option<char>, out: &mut Vec<String>) -> usize {
        let mut depth = 0usize;
        let mut last = ' ';
        let mut line_start = true;
        while i < chars.len() {
            let c = chars[i];
            let next = chars.get(i + 1).copied();
            if line_start && !c.is_whitespace() {
                line_start = false;
                let rest: String = chars[i..chars.len().min(i + 400)].iter().collect();
                let line = rest.lines().next().unwrap_or("");
                if rest.starts_with("import ") || (rest.starts_with("export ") && line.contains(" from ")) {
                    while i < chars.len() && chars[i] != ';' && chars[i] != '\n' {
                        i += 1;
                    }
                    continue;
                }
            }
            match c {
                '\n' => {
                    line_start = true;
                    i += 1;
                    continue;
                }
                '/' if next == Some('/') => {
                    while i < chars.len() && chars[i] != '\n' {
                        i += 1;
                    }
                    continue;
                }
                '/' if next == Some('*') => {
                    i += 2;
                    while i + 1 < chars.len() && !(chars[i] == '*' && chars[i + 1] == '/') {
                        i += 1;
                    }
                    i += 2;
                    continue;
                }
                '/' if "(,=:[!&|?{};+-*%<>~^ ".contains(last) => {
                    // Regular expression literal.
                    i += 1;
                    let mut class = false;
                    while i < chars.len() && chars[i] != '\n' {
                        match chars[i] {
                            '\\' => i += 1,
                            '[' => class = true,
                            ']' => class = false,
                            '/' if !class => break,
                            _ => {}
                        }
                        i += 1;
                    }
                    i += 1;
                    last = 'r';
                    continue;
                }
                '"' | '\'' => {
                    let (text, after) = string(chars, i + 1, c);
                    out.push(text);
                    i = after;
                    last = 'x';
                    continue;
                }
                '`' => {
                    i = template(chars, i + 1, out);
                    last = 'x';
                    continue;
                }
                '{' => depth += 1,
                '}' => {
                    if depth == 0 && end == Some('}') {
                        return i + 1;
                    }
                    depth = depth.saturating_sub(1);
                }
                _ => {}
            }
            if !c.is_whitespace() {
                last = c;
            }
            i += 1;
        }
        i
    }
    let chars: Vec<char> = source.chars().collect();
    let mut out = Vec::new();
    code(&chars, 0, None, &mut out);
    out
}

/// Every finding of the literal finder: `(repo-relative path, normalised literal)`, sorted, each once.
fn findings() -> BTreeSet<(String, String)> {
    let german = german();
    let mut found = BTreeSet::new();
    for (kind, scan) in [("templates", template_literals as fn(&str) -> Vec<String>), ("web", ts_literals)] {
        for (path, text) in sources(kind) {
            for literal in scan(&text) {
                let literal = normalise(&literal);
                if !literal.is_empty() && german.is_match(&literal) {
                    found.insert((path.clone(), literal));
                }
            }
        }
    }
    found
}

fn allow_list() -> BTreeSet<(String, String)> {
    let text = std::fs::read_to_string(repo().join("scripts/i18n-literal-allow.txt")).expect("allow list exists");
    text.lines()
        .filter(|line| !line.trim().is_empty() && !line.starts_with('#'))
        .map(|line| {
            let (path, literal) = line.split_once('\t').unwrap_or_else(|| panic!("allow line without a tab: {line:?}"));
            (path.to_owned(), literal.to_owned())
        })
        .collect()
}

#[test]
fn the_literal_finder_sees_german_in_templates_and_widgets() {
    let literals = template_literals(
        "{# Kommentar mit Wörtern #}<p title=\"Öffnen\" class=\"und\">Text und mehr {{ t(\"x.y\") }}</p>\
         <input type=\"hidden\" value=\"oder\"><button value=\"Löschen\">{{ choice(\"a\", \"Wie das System\") }}</button>\
         <script>var s = \"für\";</script>",
    );
    let literals: Vec<String> = literals.iter().map(|l| normalise(l)).filter(|l| !l.is_empty()).collect();
    for want in ["Öffnen", "Text und mehr", "Löschen", "Wie das System", "a"] {
        assert!(literals.contains(&want.to_owned()), "the finder must see {want:?}: {literals:?}");
    }
    for unwanted in ["und", "oder", "x.y", "für", "Kommentar mit Wörtern"] {
        assert!(
            !literals.contains(&unwanted.to_owned()),
            "{unwanted:?} is a class, a hidden value, a key, a script or a comment, not shown text: {literals:?}"
        );
    }
    let ts = ts_literals(
        "import { t } from \"./für.js\";\n// Kommentar: Größe\nconst a = \"Größe\"; /* über */\nconst b = `Hallo ${n} Welt`;\n\
         const r = /ä\"/g; const k = t(\"widget.x\");\n",
    );
    assert!(ts.contains(&"Größe".to_owned()), "a string literal is seen: {ts:?}");
    assert!(ts.iter().any(|l| normalise(l) == "Hallo Welt"), "the text of a template literal is seen: {ts:?}");
    assert!(!ts.iter().any(|l| l.contains("für") || l.contains("über")), "imports and comments are skipped: {ts:?}");
    assert!(!ts.iter().any(|l| l.contains('ä')), "a regular expression is not a string: {ts:?}");
}

#[test]
fn every_german_literal_is_listed_and_every_allow_entry_still_exists() {
    let found = findings();
    let allowed = allow_list();
    let missing: Vec<String> = found.difference(&allowed).map(|(p, l)| format!("{p}\t{l}")).collect();
    let stale: Vec<String> = allowed.difference(&found).map(|(p, l)| format!("{p}\t{l}")).collect();
    assert!(
        missing.is_empty(),
        "German text in templates/widgets that is neither translated nor in scripts/i18n-literal-allow.txt \
         ({} lines; use t(\"…\") or add the line with a # reason):\n{}",
        missing.len(),
        missing.join("\n")
    );
    assert!(
        stale.is_empty(),
        "entries of scripts/i18n-literal-allow.txt that the finder no longer sees – remove them:\n{}",
        stale.join("\n")
    );
}

// ---- (8) behaviour ---------------------------------------------------------------------------------------------

#[tokio::test]
async fn without_a_wish_the_interface_is_english_and_follows_accept_language() {
    let server = common::open().await;
    // The raw router of the server: without the German pin of the test harness.
    let raw = baruch_server::router(server.state.clone());
    let page = body(tower::ServiceExt::oneshot(raw.clone(), get("/ueber")).await.unwrap()).await;
    assert!(html_tag(&page).starts_with("<html lang=\"en\""), "default language is English: {}", html_tag(&page));
    for (accept, want) in [
        ("de", "de"),
        ("de-AT,de;q=0.9,en;q=0.5", "de"),
        ("fr, en;q=0.1", "en"),
        ("en;q=0.3, de;q=0.8", "de"),
        ("de;q=0, en", "en"),
        ("fr", "en"),
    ] {
        let page =
            body(tower::ServiceExt::oneshot(raw.clone(), common::with_lang(get("/ueber"), accept)).await.unwrap())
                .await;
        assert!(
            html_tag(&page).starts_with(&format!("<html lang=\"{want}\"")),
            "Accept-Language {accept:?} → {want} (q-values honoured, unsupported ignored): {}",
            html_tag(&page)
        );
    }
}

#[tokio::test]
async fn cookie_beats_accept_language_and_the_stored_setting_beats_the_cookie() {
    let server = common::open().await;
    let lang_of = |request| async {
        let (_, _, page) = server.send(request).await;
        html_tag(&page).to_owned()
    };
    let tag = lang_of(with_cookie(common::with_lang(get("/ueber"), "de"), "baruch_lang=en")).await;
    assert!(tag.starts_with("<html lang=\"en\""), "the cookie is a choice and beats the browser: {tag}");
    server.state.store.set_ui_language(None, Some("de")).await.unwrap();
    let tag = lang_of(with_cookie(get("/ueber"), "baruch_lang=en")).await;
    assert!(tag.starts_with("<html lang=\"de\""), "the stored instance setting beats the cookie: {tag}");
    let tag = lang_of(common::with_lang(get("/ueber"), "en")).await;
    assert!(tag.starts_with("<html lang=\"de\""), "and the browser: {tag}");
}

#[tokio::test]
async fn an_account_setting_beats_the_instance_setting() {
    let server = common::open().await;
    server.state.store.set_ui_language(None, Some("en")).await.unwrap();
    server.state.store.set_ui_language(Some(42), Some("de")).await.unwrap();
    let request = common::with_who(common::with_lang(get("/ueber"), "en"), account(42, AccountKind::Author));
    let (status, _, page) = server.send(request).await;
    assert_eq!(status, StatusCode::OK, "the author account reaches the page: {page}");
    assert!(html_tag(&page).starts_with("<html lang=\"de\""), "the own account setting wins: {}", html_tag(&page));
    let request = common::with_who(common::with_lang(get("/ueber"), "de"), account(43, AccountKind::Lektor));
    let (_, _, page) = server.send(request).await;
    assert!(
        html_tag(&page).starts_with("<html lang=\"en\""),
        "an account without its own setting keeps the instance setting while there are no accounts: {}",
        html_tag(&page)
    );
}

#[tokio::test]
async fn with_accounts_the_instance_setting_belongs_to_the_author_on_the_own_machine() {
    let server = common::open().await;
    let new = baruch_store::NewAccount {
        name: "erika",
        slug: "erika",
        display: "Erika",
        kind: "author",
        password_phc: "$argon2id$v=19$m=19456,t=2,p=1$c2FsdA$aGFzaA",
    };
    server.state.store.account_create_first(&new).await.unwrap();
    server.state.store.set_ui_language(None, Some("de")).await.unwrap();
    let local = common::with_who(common::with_lang(get("/ueber"), "en"), Who::LocalAuthor);
    let (_, _, page) = server.send(local).await;
    assert!(html_tag(&page).starts_with("<html lang=\"de\""), "the local author uses the instance setting");
    let lektor = common::with_who(common::with_lang(get("/ueber"), "en"), account(77, AccountKind::Lektor));
    let (_, _, page) = server.send(lektor).await;
    assert!(
        html_tag(&page).starts_with("<html lang=\"en\""),
        "with accounts the instance setting is not imposed on another account: {}",
        html_tag(&page)
    );
}

#[tokio::test]
async fn the_layer_sets_the_language_and_mcp_is_always_english() {
    use axum::routing::{get as get_route, post};
    let server = common::open().await;
    let state = server.state.clone();
    let probe = axum::Router::new()
        .route("/x", get_route(|| async { i18n::current().code() }))
        .route("/mcp", post(|| async { i18n::current().code() }))
        .layer(axum::middleware::from_fn_with_state(state.clone(), i18n::layer))
        .with_state(state);
    let ask = |request| async { body(tower::ServiceExt::oneshot(probe.clone(), request).await.unwrap()).await };
    assert_eq!(ask(common::with_lang(get("/x"), "de")).await, "de", "a page follows the browser");
    let mcp = common::with_lang(Request::post("/mcp").body(Body::empty()).unwrap(), "de");
    assert_eq!(ask(mcp).await, "en", "/mcp answers in English whatever the browser says (contract §3.2)");
    assert_eq!(i18n::current(), Lang::En, "outside a request the language is English");
    assert_eq!(i18n::scope(Lang::De, async { i18n::current() }).await, Lang::De, "scope sets the language");
    assert_eq!(i18n::sync_scope(Lang::De, i18n::current), Lang::De, "sync_scope too");
    let carried = i18n::scope(Lang::De, async { tokio::task::spawn_blocking(i18n::carry(i18n::current)).await }).await;
    assert_eq!(carried.unwrap(), Lang::De, "carry takes the language into spawn_blocking");
}

#[tokio::test]
async fn the_switch_sets_cookie_and_setting_and_leads_back() {
    let server = common::open().await;
    let (status, headers, text) = server.send(form("/einstellungen/sprache", "lang=en&back=%2Fwerke%3Fx%3D1")).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "the switch answers with a redirect: {text}");
    assert_eq!(location(&headers), "/werke?x=1", "back to the local path of `back`");
    assert_eq!(
        set_cookies(&headers),
        ["baruch_lang=en; Path=/; Max-Age=31536000; SameSite=Lax"],
        "the cookie holds the choice for a year"
    );
    assert_eq!(
        server.state.store.ui_language(None).await.unwrap().as_deref(),
        Some("en"),
        "without accounts the choice is the instance setting"
    );
    let (_, _, page) = server.send(get("/ueber")).await;
    assert!(html_tag(&page).starts_with("<html lang=\"en\""), "the next page is English despite the German pin");

    let mut request = form("/einstellungen/sprache", "lang=de");
    request.headers_mut().insert(header::REFERER, "http://127.0.0.1:8765/ueber?tab=start".parse().unwrap());
    let (status, headers, _) = server.send(request).await;
    assert_eq!((status, location(&headers)), (StatusCode::SEE_OTHER, "/ueber?tab=start"), "back to the Referer");

    let mut request = form("/einstellungen/sprache", "lang=auto&back=%2F%2Fevil.example");
    request.headers_mut().insert(header::REFERER, "http://evil.example/x".parse().unwrap());
    let (status, headers, _) = server.send(request).await;
    assert_eq!((status, location(&headers)), (StatusCode::SEE_OTHER, "/"), "foreign back and Referer → /");
    assert_eq!(set_cookies(&headers), ["baruch_lang=; Path=/; Max-Age=0; SameSite=Lax"], "auto deletes the cookie");
    assert_eq!(server.state.store.ui_language(None).await.unwrap(), None, "auto removes the setting");
}

#[tokio::test]
async fn the_switch_refuses_foreign_pages_and_invalid_languages() {
    let server = common::open().await;
    let mut foreign = form("/einstellungen/sprache", "lang=en");
    foreign.headers_mut().insert(header::ORIGIN, "http://evil.example".parse().unwrap());
    let (status, headers, _) = server.send(foreign).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "a foreign Origin is refused like every form (guard::same_origin)");
    assert!(set_cookies(&headers).is_empty(), "nothing is set for a foreign page");

    for bad in ["lang=fr", "lang=DE", "lang=de-AT", "", "back=%2F"] {
        let (status, headers, text) = server.send(form("/einstellungen/sprache", bad)).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{bad:?} is no language of the switch");
        assert_eq!(text, i18n::t(Lang::De, "common.lang.invalid", &[]), "{bad:?}: the message in the request language");
        assert!(set_cookies(&headers).is_empty(), "{bad:?}: no cookie");
    }
    let (_, _, text) = server.send(common::with_lang(form("/einstellungen/sprache", "lang=xx"), "en")).await;
    assert_eq!(
        text,
        i18n::t(Lang::En, "common.lang.invalid", &[]),
        "the same refusal in English for an English browser"
    );
    assert_eq!(server.state.store.ui_language(None).await.unwrap(), None, "a refused switch stores nothing");
}

#[tokio::test]
async fn lektors_and_anonymous_visitors_may_switch_but_only_for_themselves() {
    let server = common::open().await;
    let lektor =
        common::with_who(form("/einstellungen/sprache", "lang=en&back=%2Fkonto"), account(9, AccountKind::Lektor));
    let (status, headers, text) = server.send(lektor).await;
    assert_eq!((status, location(&headers)), (StatusCode::SEE_OTHER, "/konto"), "a Lektor may switch: {text}");
    assert_eq!(server.state.store.ui_language(Some(9)).await.unwrap().as_deref(), Some("en"), "under his account");
    assert_eq!(server.state.store.ui_language(None).await.unwrap(), None, "the instance setting stays untouched");

    let new = baruch_store::NewAccount {
        name: "erika",
        slug: "erika",
        display: "Erika",
        kind: "author",
        password_phc: "$argon2id$v=19$m=19456,t=2,p=1$c2FsdA$aGFzaA",
    };
    server.state.store.account_create_first(&new).await.unwrap();
    let anonymous = common::with_who(form("/einstellungen/sprache", "lang=en"), Who::Anonymous);
    let (status, headers, text) = server.send(anonymous).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "the switch is public (the sign-in page has a language too): {text}");
    assert_eq!(set_cookies(&headers).len(), 1, "an anonymous visitor gets the cookie");
    assert_eq!(server.state.store.ui_language(None).await.unwrap(), None, "but changes no stored setting");
}

#[tokio::test]
async fn the_switch_is_a_plain_form_in_the_menu_and_on_the_settings_page() {
    let server = common::open().await;
    let (_, _, page) = server.send(get("/ueber")).await;
    assert!(
        page.contains(
            "<form class=\"menu-item lang-switch\" method=\"post\" action=\"/einstellungen/sprache\" role=\"none\">"
        ),
        "the menu „Baruch ▾“ has a plain form (works without JavaScript)"
    );
    assert!(
        page.contains("name=\"lang\" value=\"de\" lang=\"de\" aria-current=\"true\">Deutsch</button>"),
        "the current language is marked and named in its own language"
    );
    assert!(page.contains("name=\"lang\" value=\"en\" lang=\"en\">English</button>"), "the other one is offered");

    let (_, _, settings) = server.send(with_cookie(get("/einstellungen/darstellung?work=1"), "baruch_lang=en")).await;
    assert!(
        settings.contains("<form class=\"settings-form\" method=\"post\" action=\"/einstellungen/sprache\">"),
        "the settings page has its own form"
    );
    assert!(
        settings.contains("<input type=\"hidden\" name=\"back\" value=\"/einstellungen/darstellung?work=1\">"),
        "it leads back to the settings page of the work"
    );
    assert!(settings.contains("value=\"en\" checked> English"), "the cookie choice is checked");
    assert!(settings.contains("<em>work language</em>"), "the markup key keeps its <em> and is not escaped");
    let (_, _, settings) = server.send(get("/einstellungen/darstellung")).await;
    assert!(settings.contains("value=\"auto\" checked> Wie der Browser"), "without a choice: like the browser");

    let answer = server.rpc("settings.get", serde_json::json!({ "page": "darstellung" })).await;
    assert_eq!(answer["result"]["lang"], "auto", "settings.get names the choice too: {answer}");
}

// ---- (9) widget catalog ----------------------------------------------------------------------------------------

#[tokio::test]
async fn the_widget_catalog_is_json_in_the_page_language() {
    let server = common::open().await;
    for lang in ["de", "en"] {
        let (_, _, page) = server.send(common::with_lang(get("/ueber"), lang)).await;
        let open = "<script type=\"application/json\" id=\"baruch-i18n\">";
        let start = page.find(open).expect("the page head carries the widget catalog") + open.len();
        let json = &page[start..start + page[start..].find("</script>").unwrap()];
        let catalog: Value = serde_json::from_str(json).expect("the widget catalog is valid JSON");
        assert_eq!(catalog["lang"], lang, "the catalog has the language of the page");
        assert!(catalog["messages"].is_object(), "messages is an object of widget.* texts: {catalog}");
        let main = page.find("/static/js/main.js").expect("main.js");
        assert!(start < main, "the catalog comes before the module script that reads it");
    }
    for lang in Lang::ALL {
        let json = i18n::widget_catalog_json(lang);
        assert!(!json.contains(['<', '>', '&']), "no markup characters inside the script element: {json}");
        assert!(
            !json.contains("{{") && !json.contains("}}") && !json.contains("{%"),
            "nothing that looks like a leftover of the template language: {json}"
        );
        let parsed: Value = serde_json::from_str(&json).unwrap();
        for key in parsed["messages"].as_object().unwrap().keys() {
            assert!(key.starts_with(i18n::WIDGET_PREFIX), "only widget texts go to the browser: {key}");
        }
    }
}

// ---- (10) formatting -------------------------------------------------------------------------------------------

#[test]
fn numbers_dates_and_template_helpers_follow_the_language() {
    assert_eq!(i18n::number(Lang::De, 1234.5, 1), "1.234,5", "German: dot groups, comma decimals");
    assert_eq!(i18n::number(Lang::En, 1234.5, 1), "1,234.5", "English: comma groups, dot decimals");
    assert_eq!(i18n::number(Lang::En, 1_234_567.0, 0), "1,234,567", "every three digits");
    assert_eq!(i18n::number(Lang::De, -0.04, 1), "0,0", "a rounded zero has no minus");
    assert_eq!(i18n::number(Lang::De, -12.25, 2), "-12,25", "negative numbers keep the minus");
    assert_eq!(i18n::number(Lang::En, 999.0, 0), "999", "no separator below a thousand");

    let stamp = "2026-11-07T12:00:00Z";
    let de = baruch_server::localtime::ui_minutes(Lang::De, stamp);
    let en = baruch_server::localtime::ui_minutes(Lang::En, stamp);
    assert!(Regex::new(r"^\d\d\.\d\d\.2026 \d\d:\d\d$").unwrap().is_match(&de), "German date and time: {de}");
    assert!(Regex::new(r"^2026-\d\d-\d\d \d\d:\d\d$").unwrap().is_match(&en), "ISO date and time: {en}");
    assert!(Regex::new(r"^\d\d\.11\.2026$").unwrap().is_match(&baruch_server::localtime::ui_day(Lang::De, stamp)));
    assert!(Regex::new(r"^2026-11-\d\d$").unwrap().is_match(&baruch_server::localtime::ui_day(Lang::En, stamp)));

    let mut env = minijinja::Environment::new();
    i18n::install(&mut env);
    let render = |source: &str, lang: &str| env.render_str(source, minijinja::context! { lang => lang }).unwrap();
    assert_eq!(render("{{ t(\"common.save\") }}", "de"), "Speichern", "the template variable lang selects German");
    assert_eq!(render("{{ t(\"common.save\") }}", "en"), "Save", "and English");
    assert_eq!(render("{{ t(\"common.save\") }}", "fr"), "Save", "another value falls back to the request (English)");
    assert_eq!(render("{{ ui_lang() }}", "de"), "de", "ui_lang names the language");
    assert_eq!(render("{{ 1234.5 | ui_number(1) }}", "de"), "1.234,5", "ui_number formats per language");
    assert_eq!(render("{{ 1234 | ui_number }}", "en"), "1,234", "without decimals");
    assert_eq!(render("{{ \"gestern\" | ui_day }}", "de"), "gestern", "an unreadable stamp stays");
    assert!(render("{{ t(\"common.lang.hint_html\") }}", "de").contains("<em>Werksprache</em>"), "markup stays");
    assert_eq!(render("{{ t(\"common.no.such\") }}", "de"), "common.no.such", "an unknown key shows itself");
    let json = render("{{ widget_catalog() }}", "de");
    assert!(json.starts_with("{\"lang\":\"de\""), "widget_catalog is not escaped as HTML: {json}");
}
