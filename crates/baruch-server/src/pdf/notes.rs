// SPDX-License-Identifier: AGPL-3.0-or-later
//! Cue notes („Stichwortzettel“, contract Round 16c): per outline item heading, time mark from the time guard,
//! cue points and scripture references – as a preview, as PDF (A5 portrait or cards A6 landscape) and through
//! the command line. Applies to every work, meant above all for sermons.
//!
//! - **Cue points** ([`points_of`], fixed rule, also in `docs/Bedienung.md` „Stichwortzettel“), per
//!   section the first rule that finds something:
//!   1. `marker` – own marker: comment lines `// Stichwort: …` (case does not matter), one point per
//!      line. Comments appear in no typesetting, so they do not disturb the text.
//!   2. `strong` – emphasised passages `*…*` and `#strong[…]`, in the order of the text.
//!   3. `first_sentence` – the first sentence of every paragraph (sentence end: `.`, `!`, `?` before whitespace or
//!      at the end of the paragraph; a full stop directly after a digit as in „1. Kor“ ends no sentence, nor does
//!      the full stop of an abbreviation such as „z. B.“, „d. h.“, „u. a.“, „bzw.“, „vgl.“, „usw.“, „ca.“, „Nr.“,
//!      „S.“, „Kap.“, „V.“, „etc.“ – since Round 17b).
//!
//!   Headings, comments, lines with `#set`/`#show`/`#let`/`#import`/`#include`,
//!   raw text, maths, `#footnote[…]` and `#arbeit[…]` do not count here; markup characters are dropped. Every
//!   point has at most [`MAX_POINT_CHARS`] characters (longer: shortened at the last word boundary, with „…“), per
//!   section at most [`MAX_POINTS`] points (the first ones).
//! - **Scripture references:** [`crate::scripture::refs`] over the section text without heading and comments;
//!   only valid references in uniform notation (`normalized`), each only once (first occurrence).
//! - **Times:** from the time guard ([`crate::timing::timing_of`] with the settings of the work, the same
//!   calculation as `work.timing`), never a count of its own: `start_clock` = time mark, `clock` = duration of
//!   the own text.
//! - **Typesetting:** [`baruch_render::notes`] (all texts as Typst strings, verbatim), through the paths of
//!   [`super::source_to_pdf`]: at most one PDF generation per work (`pdf::claim`, 409), typesetting error
//!   422, timeout 503. Read-only: no commit, no write lock.
//! - **RPC** `notes.get {work}` and `notes.pdf {work, format?, font_pt?, inline?}`; **pages**
//!   `GET /werke/{id}/stichwortzettel` and `GET /werke/{id}/stichwortzettel.pdf?format=&schrift=`;
//!   **CLI** `export ID|SLUG --stichwort DATEI [--format a5|karten] [--schrift PT]` ([`write_notes_file`]).
//! - **Draft (Round 22):** `notes.pdf {draft?}` and `?entwurf=1|0` (three-valued like `work.pdf`); the watermark
//!   ([`crate::typeset::draft_rule_for`]) stands before the first line of the notes. Link „Als Entwurf
//!   herunterladen“.

use std::sync::{Arc, OnceLock};
use std::time::Duration;

use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{Html, IntoResponse, Response};
use baruch_render::Source;
use baruch_render::notes::{MAX_FONT_PT, MIN_FONT_PT, NotesCard, NotesDoc, NotesFormat, notes_source};
use baruch_store::{StoreError, WorkId};
use minijinja::Environment;
use minijinja::value::{Serde, Value as JinjaValue};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::{Failed, PDF_TIMEOUT, PdfKind, PdfOut};
use crate::AppState;
use crate::rpc::{INVALID_PARAMS, METHOD_NOT_FOUND, RpcError};
use crate::view::Settings;

/// At most this many cue points per section.
pub const MAX_POINTS: usize = 6;
/// At most this many characters per cue point (with „…“).
pub const MAX_POINT_CHARS: usize = 160;
/// Content blocks `#name[…]` are opened at most this deep for the reading text; anything nested deeper is dropped.
/// Protects the stack (the recursion in `plain_into`) and keeps the computing time linear in the text.
pub const MAX_NESTING: usize = 32;

/// Which rule supplied the cue points of a section.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Rule {
    /// Own marker `// Stichwort: …`.
    Marker,
    /// Emphasis `*…*` or `#strong[…]`.
    Strong,
    /// First sentence per paragraph.
    FirstSentence,
}

impl Rule {
    /// Display: „eigene Markierung“, „Hervorhebung“, „erster Satz je Absatz“.
    pub fn label(self) -> &'static str {
        match self {
            Rule::Marker => "eigene Markierung",
            Rule::Strong => "Hervorhebung",
            Rule::FirstSentence => "erster Satz je Absatz",
        }
    }
}

/// A card (one outline item) as in `notes.get`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Card {
    pub key: i64,
    pub number: String,
    pub title: String,
    pub depth: usize,
    pub start_clock: String,
    pub clock: String,
    pub points: Vec<String>,
    pub refs: Vec<String>,
    pub rule: Rule,
}

/// The cue notes of a work (answer of `notes.get` without `format_choices`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Notes {
    pub work: WorkId,
    pub title: String,
    pub slug: String,
    pub kind: baruch_core::WorkKind,
    pub cards: Vec<Card>,
    pub total_clock: String,
    pub target_clock: Option<String>,
}

impl Notes {
    /// Input for [`baruch_render::notes::notes_source`].
    pub fn doc(&self) -> NotesDoc {
        NotesDoc {
            title: self.title.clone(),
            total_clock: self.total_clock.clone(),
            target_clock: self.target_clock.clone(),
            cards: self
                .cards
                .iter()
                .map(|c| NotesCard {
                    number: c.number.clone(),
                    title: c.title.clone(),
                    depth: c.depth,
                    start_clock: c.start_clock.clone(),
                    clock: c.clock.clone(),
                    points: c.points.clone(),
                    refs: c.refs.clone(),
                })
                .collect(),
        }
    }
}

// ---- Rule for cue points -----------------------------------------------------------------------

/// Text without comments (`/* … */`, `//` at the beginning of a line or after whitespace) and without raw blocks
/// (```` ``` ````); line breaks stay, so that paragraphs are kept. CRLF becomes LF.
fn strip_comments(text: &str) -> String {
    let text = text.replace("\r\n", "\n");
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let next = chars.get(i + 1).copied();
        if c == '/' && next == Some('*') {
            // Block comment up to */ (line breaks stay as paragraph boundaries).
            i += 2;
            while i < chars.len() && !(chars[i] == '*' && chars.get(i + 1) == Some(&'/')) {
                if chars[i] == '\n' {
                    out.push('\n');
                }
                i += 1;
            }
            i += 2;
            continue;
        }
        if c == '/' && next == Some('/') && (i == 0 || chars[i - 1].is_whitespace()) {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
            continue;
        }
        if c == '`' && next == Some('`') && chars.get(i + 2) == Some(&'`') {
            i += 3;
            while i < chars.len()
                && !(chars[i] == '`' && chars.get(i + 1) == Some(&'`') && chars.get(i + 2) == Some(&'`'))
            {
                if chars[i] == '\n' {
                    out.push('\n');
                }
                i += 1;
            }
            i += 3;
            continue;
        }
        if c == '\\'
            && let Some(n) = next
        {
            // An escaped character stays with its backslash (for `plain`), so that \/\/ does not become a comment.
            out.push(c);
            out.push(n);
            i += 2;
            continue;
        }
        out.push(c);
        i += 1;
    }
    out
}

/// Line that is not spoken: heading (`=`… + space, part marker `#heading(`) or
/// `#set`/`#show`/`#let`/`#import`/`#include`.
fn silent_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    let equals = trimmed.chars().take_while(|&c| c == '=').count();
    if equals > 0 && trimmed[equals..].starts_with([' ', '\t']) {
        return true;
    }
    ["#heading(", "#set ", "#show ", "#show:", "#let ", "#import ", "#include "].iter().any(|p| trimmed.starts_with(p))
}

/// Text without comments and without silent lines.
fn body_text(typst: &str) -> String {
    strip_comments(typst).lines().map(|line| if silent_line(line) { "" } else { line }).collect::<Vec<_>>().join("\n")
}

/// End of the bracket group from `open` (character `(`/`[`), strings in `"…"` respected; `None` without an end.
fn group_end(chars: &[char], open: usize) -> Option<usize> {
    let (start, stop) = match chars[open] {
        '(' => ('(', ')'),
        '[' => ('[', ']'),
        _ => return None,
    };
    let mut depth = 0usize;
    let mut i = open;
    let mut in_string = false;
    while i < chars.len() {
        let c = chars[i];
        if in_string {
            if c == '\\' {
                i += 1;
            } else if c == '"' {
                in_string = false;
            }
        } else if c == '\\' {
            i += 1;
        } else if c == '"' && start == '(' {
            in_string = true;
        } else if c == start {
            depth += 1;
        } else if c == stop {
            depth -= 1;
            if depth == 0 {
                return Some(i);
            }
        }
        i += 1;
    }
    None
}

/// Pure reading text of a piece of Typst markup (without comments): code calls without arguments, content
/// in `[…]` stays (except `#footnote`/`#arbeit`; deeper than [`MAX_NESTING`] levels it is dropped), labels,
/// references, maths, raw and markup characters are dropped, whitespace collapsed.
fn plain(markup: &str) -> String {
    let chars: Vec<char> = markup.chars().collect();
    let mut out = String::new();
    plain_into(&chars, &mut out, 0);
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Is `name` a call whose content is not spoken (`#footnote`, `#arbeit`)?
fn silent_call(name: &str) -> bool {
    matches!(name, "footnote" | "arbeit")
}

/// End of a name `#name` from `start` (character after `#`): letters, digits, `_`, `-` and `.` before a
/// letter (`#name.feld`).
fn name_end(chars: &[char], start: usize) -> usize {
    let mut j = start;
    while j < chars.len() && (chars[j].is_alphanumeric() || matches!(chars[j], '_' | '-' | '.')) {
        if chars[j] == '.' && !chars.get(j + 1).is_some_and(|n| n.is_alphabetic()) {
            break;
        }
        j += 1;
    }
    j
}

fn plain_into(chars: &[char], out: &mut String, depth: usize) {
    let mut i = 0;
    // Only back to the last non-whitespace, not over the whole line (otherwise quadratic).
    let at_line_start = |out: &String| out.chars().rev().take_while(|&c| c != '\n').all(char::is_whitespace);
    while i < chars.len() {
        let c = chars[i];
        let next = chars.get(i + 1).copied();
        match c {
            '\\' => {
                match next {
                    Some(n) if n.is_whitespace() => out.push(' '),
                    Some(n) => out.push(n),
                    None => {}
                }
                i += 2;
            }
            '#' if next.is_some_and(|n| n.is_alphabetic() || n == '_') => {
                let mut j = name_end(chars, i + 1);
                let name: String = chars[i + 1..j].iter().collect();
                let silent = silent_call(&name);
                loop {
                    match chars.get(j) {
                        Some('(') => match group_end(chars, j) {
                            Some(end) => j = end + 1,
                            None => {
                                j = chars.len();
                                break;
                            }
                        },
                        Some('[') => match group_end(chars, j) {
                            Some(end) => {
                                out.push(' ');
                                if !silent && depth < MAX_NESTING {
                                    plain_into(&chars[j + 1..end], out, depth + 1);
                                    out.push(' ');
                                }
                                j = end + 1;
                            }
                            None => {
                                j = chars.len();
                                break;
                            }
                        },
                        _ => break,
                    }
                }
                i = j;
            }
            '$' => {
                let end = chars[i + 1..].iter().position(|&x| x == '$').map_or(chars.len(), |p| i + 1 + p);
                out.push(' ');
                i = end + 1;
            }
            '`' => {
                let end = chars[i + 1..].iter().position(|&x| x == '`').map_or(chars.len(), |p| i + 1 + p);
                out.push(' ');
                i = end + 1;
            }
            '<' if next.is_some_and(|n| n.is_alphanumeric() || n == '_') => {
                let mut j = i + 1;
                while j < chars.len() && (chars[j].is_alphanumeric() || matches!(chars[j], '_' | '-' | ':' | '.')) {
                    j += 1;
                }
                if chars.get(j) == Some(&'>') {
                    i = j + 1;
                } else {
                    out.push(c);
                    i += 1;
                }
            }
            // Reference `@name` after whitespace, bracket, quotation mark or punctuation (Round 17b, L7);
            // in the middle of a word („a@b.example“) the @ stays text.
            '@' if next.is_some_and(|n| n.is_alphanumeric() || n == '_')
                && (i == 0 || !(chars[i - 1].is_alphanumeric() || chars[i - 1] == '_')) =>
            {
                let mut j = i + 1;
                while j < chars.len() && (chars[j].is_alphanumeric() || matches!(chars[j], '_' | '-' | ':')) {
                    j += 1;
                }
                i = j;
            }
            '*' | '_' => i += 1,
            '~' => {
                out.push(' ');
                i += 1;
            }
            '-' | '+' | '/' if next == Some(' ') && at_line_start(out) => i += 2,
            _ => {
                out.push(c);
                i += 1;
            }
        }
    }
}

/// End (index after the last character) of a silent call `#footnote…`/`#arbeit…` from `#` at `at`, including
/// directly following arguments `(…)` and content blocks `[…]`; `None` if there is no silent call there.
fn silent_call_end(chars: &[char], at: usize) -> Option<usize> {
    let mut j = name_end(chars, at + 1);
    if !silent_call(&chars[at + 1..j].iter().collect::<String>()) {
        return None;
    }
    while matches!(chars.get(j), Some('(' | '[')) {
        j = group_end(chars, j).map_or(chars.len(), |end| end + 1);
    }
    Some(j)
}

/// Emphasis `*…*` and `#strong[…]` as reading text, in the order of the text. Footnotes and
/// work orders (`#footnote`, `#arbeit`) do not count, not even with emphasis inside.
fn strong_spans(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut found = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '#'
            && let Some(end) = silent_call_end(&chars, i)
        {
            i = end;
            continue;
        }
        match chars[i] {
            '\\' => i += 2,
            '`' => {
                i = chars[i + 1..].iter().position(|&x| x == '`').map_or(chars.len(), |p| i + 2 + p);
            }
            '$' => {
                i = chars[i + 1..].iter().position(|&x| x == '$').map_or(chars.len(), |p| i + 2 + p);
            }
            '#' if chars[i + 1..].starts_with(&['s', 't', 'r', 'o', 'n', 'g', '[']) => {
                let open = i + 7;
                match group_end(&chars, open) {
                    Some(end) => {
                        found.push(plain(&chars[open + 1..end].iter().collect::<String>()));
                        i = end + 1;
                    }
                    None => i = chars.len(),
                }
            }
            '*' => {
                // Up to the next unescaped * in the same paragraph.
                let mut j = i + 1;
                let mut end = None;
                while j < chars.len() {
                    if chars[j] == '\\' {
                        j += 2;
                        continue;
                    }
                    if chars[j] == '\n' && chars.get(j + 1).is_some_and(|&n| n == '\n') {
                        break;
                    }
                    if chars[j] == '*' {
                        end = Some(j);
                        break;
                    }
                    j += 1;
                }
                match end {
                    Some(end) => {
                        found.push(plain(&chars[i + 1..end].iter().collect::<String>()));
                        i = end + 1;
                    }
                    None => i += 1,
                }
            }
            _ => i += 1,
        }
    }
    found.retain(|s| s.chars().any(char::is_alphanumeric));
    found
}

/// Abbreviations whose full stop ends no sentence (Round 17b, L7), without the full stop. Case counts only
/// for one-letter forms („S.“ Seite, „V.“ Vers); the others also apply at the beginning of a sentence („Vgl.“,
/// „Ca.“).
const ABBREVIATIONS: [&str; 14] =
    ["bzw", "vgl", "usw", "ca", "nr", "kap", "etc", "ggf", "evtl", "inkl", "hrsg", "jh", "bd", "abs"];
/// Abbreviations of one letter, written in upper case („S. 3“, „V. 23“).
const LETTER_ABBREVIATIONS: [&str; 2] = ["S", "V"];
/// Two-part abbreviations („z. B.“, „d. h.“, „u. a.“ …): per part one letter with a full stop.
const PAIR_ABBREVIATIONS: [(&str, &str); 8] =
    [("z", "B"), ("d", "h"), ("u", "a"), ("z", "T"), ("o", "ä"), ("u", "U"), ("s", "o"), ("v", "a")];

/// Word (letters only) directly before `end` and its beginning.
fn word_before(chars: &[char], end: usize) -> (String, usize) {
    let mut start = end;
    while start > 0 && chars[start - 1].is_alphabetic() {
        start -= 1;
    }
    (chars[start..end].iter().collect(), start)
}

/// Does the full stop at `i` end an abbreviation (instead of a sentence)?
fn abbreviation_dot(chars: &[char], i: usize) -> bool {
    let (word, start) = word_before(chars, i);
    if word.is_empty() || start > 0 && chars[start - 1].is_alphanumeric() {
        return false;
    }
    if ABBREVIATIONS.contains(&word.to_lowercase().as_str()) || LETTER_ABBREVIATIONS.contains(&word.as_str()) {
        return true;
    }
    // First part of a pair: „z.“ before „B.“ (with or without whitespace in between).
    let mut j = i + 1;
    while chars.get(j).is_some_and(|c| c.is_whitespace()) {
        j += 1;
    }
    let (next, next_end) = {
        let mut k = j;
        while chars.get(k).is_some_and(|c| c.is_alphabetic()) {
            k += 1;
        }
        (chars[j..k].iter().collect::<String>(), k)
    };
    if chars.get(next_end) == Some(&'.') && PAIR_ABBREVIATIONS.iter().any(|(a, b)| *a == word && *b == next) {
        return true;
    }
    // Second part: „B.“ after „z.“.
    let mut k = start;
    while k > 0 && chars[k - 1].is_whitespace() {
        k -= 1;
    }
    if k > 0 && chars[k - 1] == '.' {
        let (prev, _) = word_before(chars, k - 1);
        return PAIR_ABBREVIATIONS.iter().any(|(a, b)| *a == prev && *b == word);
    }
    false
}

/// First sentence of a paragraph (reading text). A full stop after a digit („1. Kor“) or after an abbreviation
/// ([`ABBREVIATIONS`], „z. B.“, „S. 3“) ends no sentence.
fn first_sentence(paragraph: &str) -> String {
    let chars: Vec<char> = paragraph.chars().collect();
    for (i, &c) in chars.iter().enumerate() {
        if !matches!(c, '.' | '!' | '?') {
            continue;
        }
        let ends = chars.get(i + 1).is_none_or(|n| n.is_whitespace());
        let after_digit = c == '.' && i > 0 && chars[i - 1].is_ascii_digit();
        let abbreviation = c == '.' && chars.get(i + 1).is_some() && abbreviation_dot(&chars, i);
        if ends && !after_digit && !abbreviation {
            return chars[..=i].iter().collect();
        }
    }
    paragraph.to_owned()
}

/// Shorten to [`MAX_POINT_CHARS`]: at the last word boundary, with „…“.
fn shorten(point: &str) -> String {
    let point = point.trim();
    if point.chars().count() <= MAX_POINT_CHARS {
        return point.to_owned();
    }
    let head: String = point.chars().take(MAX_POINT_CHARS - 1).collect();
    let cut = match head.rfind(char::is_whitespace) {
        Some(at) if head[..at].chars().count() >= MAX_POINT_CHARS / 2 => head[..at].trim_end().to_owned(),
        _ => head,
    };
    format!("{cut}…")
}

/// Cue points of a section text (Typst) by the rule above, with the rule that supplied them.
pub fn points_of(typst: &str) -> (Vec<String>, Rule) {
    let text = typst.replace("\r\n", "\n");
    let finish = |points: Vec<String>| -> Vec<String> {
        points.iter().map(|p| shorten(p)).filter(|p| !p.is_empty()).take(MAX_POINTS).collect()
    };
    let markers: Vec<String> = text
        .lines()
        .filter_map(|line| {
            let rest = line.trim_start().strip_prefix("//")?.trim_start();
            let head: String = rest.chars().take(10).collect();
            if !head.eq_ignore_ascii_case("stichwort:") {
                return None;
            }
            let point = rest.chars().skip(10).collect::<String>();
            Some(point.split_whitespace().collect::<Vec<_>>().join(" "))
        })
        .filter(|p| !p.is_empty())
        .collect();
    if !markers.is_empty() {
        return (finish(markers), Rule::Marker);
    }
    let body = body_text(&text);
    let strong = strong_spans(&body);
    if !strong.is_empty() {
        return (finish(strong), Rule::Strong);
    }
    let sentences: Vec<String> = body
        .split("\n\n")
        .map(plain)
        .filter(|p| p.chars().any(char::is_alphanumeric))
        .map(|p| first_sentence(&p))
        .collect();
    (finish(sentences), Rule::FirstSentence)
}

/// Valid scripture references of a section in uniform notation, without duplicates (first occurrence).
pub fn refs_of(typst: &str) -> Vec<String> {
    let body = body_text(typst);
    let mut out: Vec<String> = Vec::new();
    for found in crate::scripture::refs(&body) {
        if found.status != baruch_scripture::RefStatus::Valid {
            continue;
        }
        if let Some(normalized) = found.normalized
            && !out.contains(&normalized)
        {
            out.push(normalized);
        }
    }
    out
}

// ---- Notes of a work ---------------------------------------------------------------------------

/// Cue notes of the work: cards in outline order with times from the time guard.
pub async fn notes_of(state: &AppState, work_id: WorkId) -> Result<Notes, RpcError> {
    let work = state.store.load_work(work_id).await?;
    let slug = state.store.work_slug(work_id).await?;
    let settings = state.store.timing_settings(work_id).await?;
    let timing = crate::timing::timing_of(work_id, &work, settings);
    let clock = baruch_typst::timing::clock;
    let cards = work
        .nodes
        .iter()
        .map(|node| {
            let body = node.current_index().map_or("", |i| node.revisions[i].body.as_str());
            let (points, rule) = points_of(body);
            let section = timing.sections.iter().find(|s| s.key == node.key);
            Card {
                key: node.key,
                number: node.number.clone(),
                title: node.title.clone(),
                depth: section.map_or(0, |s| s.depth),
                start_clock: clock(section.map_or(0, |s| s.start_seconds)),
                clock: clock(section.map_or(0, |s| s.seconds)),
                points,
                refs: refs_of(body),
                rule,
            }
        })
        .collect();
    Ok(Notes {
        work: work_id,
        title: work.title.clone(),
        slug,
        kind: work.kind,
        cards,
        total_clock: clock(timing.seconds),
        target_clock: timing.target_seconds.map(clock),
    })
}

/// Answer of `notes.get`.
pub fn notes_json(notes: &Notes) -> Value {
    json!({
        "work": notes.work,
        "title": notes.title,
        "kind": notes.kind,
        "format_choices": NotesFormat::ALL.iter().map(|f| f.as_str()).collect::<Vec<_>>(),
        "cards": notes.cards,
        "total_clock": notes.total_clock,
        "target_clock": notes.target_clock,
    })
}

// ---- PDF ---------------------------------------------------------------------------------------

/// Format, font size and (for tests only) time limit and an addition to the source.
#[derive(Debug, Clone)]
pub struct NotesOptions {
    pub format: NotesFormat,
    pub font_pt: u32,
    /// Time limit of the typesetting (default [`PDF_TIMEOUT`]); tests reproduce the timeout with it.
    #[doc(hidden)]
    pub limit: Duration,
    /// Typst that is appended to the generated source. For tests only: the cards themselves cannot trigger a
    /// typesetting error (everything is escaped), this way the path „Satzfehler“ can be checked nevertheless.
    #[doc(hidden)]
    pub append: String,
    /// Round 22: watermark three-valued (`None` = by typeset setting).
    pub draft: Option<bool>,
}

impl Default for NotesOptions {
    fn default() -> NotesOptions {
        NotesOptions::new(NotesFormat::A5, None)
    }
}

impl NotesOptions {
    /// Format and font size (`None` = default of the format). [`NotesOptions::parse`] checks the size.
    pub fn new(format: NotesFormat, font_pt: Option<u32>) -> NotesOptions {
        NotesOptions {
            format,
            font_pt: font_pt.unwrap_or(format.default_font_pt()),
            limit: PDF_TIMEOUT,
            append: String::new(),
            draft: None,
        }
    }

    /// From texts (form, command line): `format` `a5`|`karten` (empty = a5), `schrift` 14–28 (empty =
    /// default of the format). Errors in German.
    pub fn parse(format: Option<&str>, font_pt: Option<&str>) -> Result<NotesOptions, String> {
        let format = match format.map(str::trim).filter(|f| !f.is_empty()) {
            None => NotesFormat::A5,
            Some(text) => NotesFormat::parse(text)
                .ok_or_else(|| format!("Format „{text}“ gibt es nicht; a5 oder karten erwartet"))?,
        };
        let size = match font_pt.map(str::trim).filter(|f| !f.is_empty()) {
            None => None,
            Some(text) => Some(
                text.parse::<u32>().ok().filter(|pt| (MIN_FONT_PT..=MAX_FONT_PT).contains(pt)).ok_or_else(|| {
                    format!("Schriftgröße „{text}“: ganze Zahl von {MIN_FONT_PT} bis {MAX_FONT_PT} (Punkt) erwartet")
                })?,
            ),
        };
        Ok(NotesOptions::new(format, size))
    }

    /// Address of the PDF with these settings (Round 22: with `&entwurf=1|0` if the switch is set).
    pub fn pdf_url(&self, work: WorkId) -> String {
        let pair = crate::typeset::draft_pair(self.draft);
        let pair = if pair.is_empty() { String::new() } else { format!("&{pair}") };
        format!("/werke/{work}/stichwortzettel.pdf?format={}&schrift={}{pair}", self.format.as_str(), self.font_pt)
    }
}

/// File name: `<slug>-stichwortzettel.pdf`.
pub fn file_name(slug: &str) -> String {
    format!("{slug}-stichwortzettel.pdf")
}

async fn generate(state: &AppState, work: WorkId, options: &NotesOptions) -> Result<(Notes, PdfOut), Failed> {
    let notes = notes_of(state, work).await.map_err(Failed::Other)?;
    let mut main = notes_source(&notes.doc(), options.format, options.font_pt);
    main.push_str(&options.append);
    // Round 22: watermark before the first line (before any content, the page rule of the notes only adds to it).
    let stored = crate::typeset::stored_draft(state, work).await.map_err(|e| Failed::Other(e.into()))?;
    if let Some(draft) = crate::typeset::effective_draft(stored.as_ref(), options.draft) {
        main.insert_str(0, &crate::typeset::draft_rule_for(&draft));
    }
    let pdf = super::generate(work, PdfKind::Notes, Source { main, files: Vec::new() }, options.limit).await?;
    Ok((notes, pdf))
}

/// Cue notes as PDF (at most one generation per work, time limit, typesetting error as `REFUSED`).
pub async fn notes_pdf(state: &AppState, work: WorkId, options: &NotesOptions) -> Result<(Notes, PdfOut), RpcError> {
    generate(state, work, options).await.map_err(Failed::into_rpc)
}

// ---- RPC ---------------------------------------------------------------------------------------

/// Does this module serve the method (`notes.get`, `notes.pdf`)?
pub fn handles(method: &str) -> bool {
    matches!(method, "notes.get" | "notes.pdf")
}

/// `notes.get {work}` → cards; `notes.pdf {work, format? = "a5", font_pt?, inline?}` → like `work.pdf`.
pub async fn call(state: &AppState, work: WorkId, method: &str, params: Value) -> Result<Value, RpcError> {
    match method {
        "notes.get" => Ok(notes_json(&notes_of(state, work).await?)),
        "notes.pdf" => {
            let invalid = |m: String| RpcError::new(INVALID_PARAMS, m);
            let format = match params.get("format") {
                None | Some(Value::Null) => None,
                Some(Value::String(s)) => Some(s.clone()),
                Some(_) => return Err(invalid("format: \"a5\" oder \"karten\" erwartet".into())),
            };
            let font = match params.get("font_pt") {
                None | Some(Value::Null) => None,
                Some(Value::Number(n)) => Some(n.to_string()),
                Some(_) => return Err(invalid(format!("font_pt: Zahl von {MIN_FONT_PT} bis {MAX_FONT_PT} erwartet"))),
            };
            let mut options = NotesOptions::parse(format.as_deref(), font.as_deref()).map_err(invalid)?;
            options.draft = crate::typeset::draft_param(&params)?;
            let inline = super::flag(&params, "inline")?;
            let (notes, pdf) = notes_pdf(state, work, &options).await?;
            let mut answer = json!({
                "file_name": file_name(&notes.slug),
                "format": options.format.as_str(),
                "font_pt": options.font_pt,
                "pages": pdf.pages,
                "bytes": pdf.bytes.len(),
                "sha256": baruch_render::sha256_hex(&pdf.bytes),
                "url": options.pdf_url(work),
                "print": pdf.print,
                "millis": pdf.millis,
            });
            if inline {
                answer["pdf_base64"] = json!(baruch_store::encode_base64(&pdf.bytes));
            }
            Ok(answer)
        }
        _ => Err(RpcError::new(METHOD_NOT_FOUND, format!("unknown method {method}"))),
    }
}

// ---- Pages -------------------------------------------------------------------------------------

fn templates() -> &'static Environment<'static> {
    static ENV: OnceLock<Environment<'static>> = OnceLock::new();
    ENV.get_or_init(|| {
        let mut env = Environment::new();
        for (name, source) in [
            ("base.html", include_str!("../../templates/base.html")),
            ("notes.html", include_str!("../../templates/notes.html")),
        ] {
            env.add_template(name, source).expect("Vorlage des Stichwortzettels fehlerhaft");
        }
        env
    })
}

/// Form values of the pages (`format`, `schrift`).
#[derive(Debug, Default, Deserialize)]
pub struct NotesQuery {
    pub format: Option<String>,
    pub schrift: Option<String>,
    /// Round 22: watermark three-valued (`1`, `0`, missing = by setting).
    pub entwurf: Option<String>,
}

#[derive(Serialize)]
struct FormatChoice {
    value: &'static str,
    label: &'static str,
    selected: bool,
}

/// Render the page; `notes` is missing only if the loading already failed.
fn page(
    status: StatusCode,
    headers: &HeaderMap,
    work: WorkId,
    title: &str,
    notes: Option<&Notes>,
    options: &NotesOptions,
    error: Option<String>,
) -> Response {
    let settings = Settings::from_cookie_header(headers.get(header::COOKIE).and_then(|v| v.to_str().ok()));
    let choices: Vec<FormatChoice> = NotesFormat::ALL
        .iter()
        .map(|f| FormatChoice { value: f.as_str(), label: f.label(), selected: *f == options.format })
        .collect();
    let cards: Vec<Value> = notes
        .map(|n| {
            n.cards
                .iter()
                .map(|c| {
                    let mut value = json!(c);
                    value["rule_label"] = json!(c.rule.label());
                    value
                })
                .collect()
        })
        .unwrap_or_default();
    let ctx = minijinja::context! {
        settings => JinjaValue::from(Serde(settings)),
        work => minijinja::context! { id => work, title => title },
        back => crate::typeset::href(format!("/?work={work}")),
        pdf_url => crate::typeset::href(options.pdf_url(work)),
        draft_url => crate::typeset::href(NotesOptions { draft: Some(true), ..options.clone() }.pdf_url(work)),
        choices => JinjaValue::from(Serde(choices)),
        font_pt => options.font_pt,
        min_pt => MIN_FONT_PT,
        max_pt => MAX_FONT_PT,
        cards => JinjaValue::from(Serde(cards)),
        total_clock => notes.map(|n| n.total_clock.clone()),
        target_clock => notes.and_then(|n| n.target_clock.clone()),
        file_name => notes.map(|n| file_name(&n.slug)),
        error => error,
    };
    match templates().get_template("notes.html").and_then(|t| t.render(ctx)) {
        Ok(html) => (status, Html(html)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, format!("Fehler: Vorlage notes.html: {e}")).into_response(),
    }
}

async fn work_title(state: &AppState, id: WorkId) -> Result<Option<String>, StoreError> {
    Ok(state.store.works().await?.into_iter().find(|(w, _, _)| *w == id).map(|(_, _, title)| title))
}

/// `GET /werke/{id}/stichwortzettel?format=&schrift=`: choice of format (form `method=get`), preview of the
/// cards and link to the PDF. Usable without JavaScript.
pub async fn notes_page(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<WorkId>,
    Query(query): Query<NotesQuery>,
) -> Response {
    let title = match work_title(&state, id).await {
        Ok(Some(title)) => title,
        Ok(None) => return (StatusCode::NOT_FOUND, "unbekanntes Werk").into_response(),
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, format!("Fehler: {e}")).into_response(),
    };
    let (options, error) = match NotesOptions::parse(query.format.as_deref(), query.schrift.as_deref()) {
        Ok(options) => (options, None),
        Err(e) => (NotesOptions::default(), Some(e)),
    };
    let notes = match notes_of(&state, id).await {
        Ok(notes) => notes,
        Err(e) => {
            return page(StatusCode::INTERNAL_SERVER_ERROR, &headers, id, &title, None, &options, Some(e.message));
        }
    };
    let status = if error.is_some() { StatusCode::UNPROCESSABLE_ENTITY } else { StatusCode::OK };
    page(status, &headers, id, &title, Some(&notes), &options, error)
}

/// `GET /werke/{id}/stichwortzettel.pdf?format=&schrift=`: the PDF as attachment `<slug>-stichwortzettel.pdf`;
/// 409 (already running), 422 (input or typesetting error), 503 (time ran out) as a page.
pub async fn notes_download(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<WorkId>,
    Query(query): Query<NotesQuery>,
) -> Response {
    if crate::guard::cross_site_get(&headers) {
        return crate::typeset::foreign_get();
    }
    let parsed = NotesOptions::parse(query.format.as_deref(), query.schrift.as_deref()).and_then(|mut options| {
        options.draft = crate::typeset::draft_query(query.entwurf.as_deref())?;
        Ok(options)
    });
    match parsed {
        Ok(options) => notes_response(&state, &headers, id, &options).await,
        Err(e) => match work_title(&state, id).await {
            Ok(Some(title)) => {
                let notes = notes_of(&state, id).await.ok();
                page(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    &headers,
                    id,
                    &title,
                    notes.as_ref(),
                    &NotesOptions::default(),
                    Some(e),
                )
            }
            Ok(None) => (StatusCode::NOT_FOUND, "unbekanntes Werk").into_response(),
            Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, format!("Fehler: {e}")).into_response(),
        },
    }
}

/// Answer of the download for fixed settings (public for tests: time limit and typesetting error).
#[doc(hidden)]
pub async fn notes_response(state: &AppState, headers: &HeaderMap, id: WorkId, options: &NotesOptions) -> Response {
    let title = match work_title(state, id).await {
        Ok(Some(title)) => title,
        Ok(None) => return (StatusCode::NOT_FOUND, "unbekanntes Werk").into_response(),
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, format!("Fehler: {e}")).into_response(),
    };
    let failed = match generate(state, id, options).await {
        Ok((notes, pdf)) => return super::attachment(&file_name(&notes.slug), pdf.bytes),
        Err(failed) => failed,
    };
    let (status, message) = match failed {
        Failed::Busy(e) => (StatusCode::CONFLICT, e.message),
        Failed::Timeout(e) => (StatusCode::SERVICE_UNAVAILABLE, e.message),
        Failed::Typst(_, e) => (StatusCode::UNPROCESSABLE_ENTITY, e.message),
        Failed::Other(e) if e.code == INVALID_PARAMS => (StatusCode::NOT_FOUND, e.message),
        Failed::Other(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.message),
    };
    let notes = notes_of(state, id).await.ok();
    page(status, headers, id, &title, notes.as_ref(), options, Some(message))
}

// ---- Command line ------------------------------------------------------------------------------

/// `baruch-server export <werk> --stichwort DATEI [--format a5|karten] [--schrift PT]`: cue notes as
/// PDF to `target`. If the file already exists, nothing is written; if the typesetting fails, no file is
/// created.
pub async fn write_notes_file(
    state: &AppState,
    work: WorkId,
    target: &std::path::Path,
    options: NotesOptions,
) -> Result<PdfOut, String> {
    if std::fs::symlink_metadata(target).is_ok() {
        return Err(format!("{} gibt es schon; nichts überschrieben", target.display()));
    }
    let (_, pdf) = notes_pdf(state, work, &options).await.map_err(|e| e.message)?;
    super::write_new_file(target, pdf.bytes.clone()).await?;
    Ok(pdf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markers_win_over_emphasis_and_sentences() {
        let text = "= Brot\n\n// Stichwort: Gabe vor Leistung\nEin *starkes* Wort.\n  // STICHWORT:   Teilen  \n";
        let (points, rule) = points_of(text);
        assert_eq!(rule, Rule::Marker, "eigene Markierung geht vor");
        assert_eq!(points, ["Gabe vor Leistung", "Teilen"], "je Kommentarzeile ein Punkt, Leerraum bereinigt");
    }

    #[test]
    fn emphasis_counts_without_markers() {
        let text = "= Weg\n\nWir gehen *gemeinsam* los. Dann #strong[mit Brot] weiter.\n\n2 \\* 3 ist kein Stern.\n";
        let (points, rule) = points_of(text);
        assert_eq!(rule, Rule::Strong, "Hervorhebungen ohne Markierung");
        assert_eq!(points, ["gemeinsam", "mit Brot"], "*…* und #strong[…] in Textreihenfolge, \\* zählt nicht");
    }

    #[test]
    fn otherwise_the_first_sentence_of_each_paragraph() {
        let text = "= Schluss\n\nIn 1. Kor 13 steht es. Der Rest fällt weg.\n\n#set text(size: 12pt)\n\n\
                    - Geht hin! Und mehr.\n\n/* Notiz. */ Ohne Punkt am Ende\n";
        let (points, rule) = points_of(text);
        assert_eq!(rule, Rule::FirstSentence, "ohne Markierung und Hervorhebung");
        assert_eq!(
            points,
            ["In 1. Kor 13 steht es.", "Geht hin!", "Ohne Punkt am Ende"],
            "„1.“ beendet keinen Satz, #set-Zeilen und Kommentare zählen nicht, Listenzeichen fallen weg"
        );
    }

    #[test]
    fn abbreviations_do_not_end_the_first_sentence() {
        for (paragraph, sentence) in [
            ("Er kam z. B. am Abend. Der Rest fällt weg.", "Er kam z. B. am Abend."),
            ("Das heißt d. h. nichts anderes. Weiter.", "Das heißt d. h. nichts anderes."),
            ("Brot, Wein u. a. lagen da. Weiter.", "Brot, Wein u. a. lagen da."),
            (
                "Vgl. dazu Kap. 3 bzw. Nr. 7 auf S. 12, ca. drei Zeilen. Weiter.",
                "Vgl. dazu Kap. 3 bzw. Nr. 7 auf S. 12, ca. drei Zeilen.",
            ),
            ("Psalm 139, V. 23 trägt usw. bis zum Ende. Weiter.", "Psalm 139, V. 23 trägt usw. bis zum Ende."),
            ("Steine, Sand etc. sind da. Weiter.", "Steine, Sand etc. sind da."),
            ("Er sagte z.B. Brot. Weiter.", "Er sagte z.B. Brot."),
        ] {
            assert_eq!(
                first_sentence(paragraph),
                sentence,
                "der Punkt einer Abkürzung beendet den ersten Satz nicht (Bedienung „Stichwortzettel“, L7)"
            );
        }
        assert_eq!(
            first_sentence("Ein Hof. Zwei Höfe."),
            "Ein Hof.",
            "ein gewöhnliches Wort vor dem Punkt bleibt ein Satzende (Hof ist keine Abkürzung)"
        );
        assert_eq!(
            first_sentence("Am Bus. Weiter."),
            "Am Bus.",
            "„Bus“ endet auf „s“, ist aber nicht „S.“ – nur das ganze Wort zählt"
        );
    }

    #[test]
    fn references_after_brackets_quotes_and_punctuation_are_dropped() {
        assert_eq!(
            plain("Siehe (@abb) und [@tab], dazu „@zitat“ sowie ,@liste."),
            "Siehe () und [], dazu „“ sowie ,.",
            "@-Verweise nach „(“, „[“, „„“ und Satzzeichen sind Verweise und fallen weg (L7)"
        );
        assert_eq!(
            plain("Mail an a@b.example oder 7@y"),
            "Mail an a@b.example oder 7@y",
            "ein @ direkt hinter Buchstabe oder Ziffer ist kein Verweis, sondern Text"
        );
    }

    #[test]
    fn points_are_limited_to_six_and_160_characters() {
        let markers: String = (1..=8).map(|i| format!("// Stichwort: Punkt {i}\n")).collect();
        let (points, _) = points_of(&markers);
        assert_eq!(points.len(), MAX_POINTS, "höchstens sechs Punkte je Abschnitt");
        assert_eq!(points[5], "Punkt 6", "die ersten sechs");
        let long = format!("// Stichwort: {}\n", "Wort ".repeat(60));
        let (points, _) = points_of(&long);
        assert!(points[0].chars().count() <= MAX_POINT_CHARS, "höchstens 160 Zeichen: {}", points[0].chars().count());
        assert!(points[0].ends_with("Wort…"), "an der Wortgrenze gekürzt, mit „…“: {}", points[0]);
    }

    #[test]
    fn plain_text_drops_code_labels_math_and_footnotes() {
        assert_eq!(
            plain("Siehe #footnote[Quelle] @abb <ziel> $x^2$ `code` und #emph[dies]~hier\\!"),
            "Siehe und dies hier!",
            "Fußnote, Verweis, Label, Mathe und Raw fallen weg, Inhalt von #emph bleibt"
        );
        assert_eq!(plain("#quote(block: true, attribution: [A])[Im Anfang]"), "Im Anfang", "Argumente fallen weg");
        assert_eq!(plain("Mail an a@b.example"), "Mail an a@b.example", "@ mitten im Wort ist kein Verweis");
    }

    #[test]
    fn emphasis_inside_footnotes_and_work_notes_does_not_count() {
        let text = "= Teil\n\nDer Hirte sucht. #footnote[Vgl. *Kommentar*, S. 3]\n\nDann kommt er heim. \
                    #arbeit(von: \"*x*\")[*TODO* prüfen] #footnote[#strong[Quelle]]\n";
        let (points, rule) = points_of(text);
        assert_eq!(
            (points, rule),
            (vec!["Der Hirte sucht.".to_owned(), "Dann kommt er heim.".to_owned()], Rule::FirstSentence),
            "Fußnoten und Arbeitsaufträge zählen nicht (Bedienung „Stichwortzettel“), auch ihre Hervorhebungen \
             nicht – sonst verdrängte Regel 2 die ersten Sätze und ein Arbeitsauftrag käme auf die Kanzelkarte"
        );
        let (points, rule) = points_of("= Teil\n\nEin *Wort* hier. #footnote[*nicht*] Und *noch eins*.\n");
        assert_eq!(
            (points, rule),
            (vec!["Wort".to_owned(), "noch eins".to_owned()], Rule::Strong),
            "Hervorhebungen im gesprochenen Text vor und nach der Fußnote zählen weiter"
        );
    }

    #[test]
    fn deeply_nested_markup_neither_overflows_the_stack_nor_takes_quadratic_time() {
        // 60 000 levels #a[…] used to run into a recursion without a limit: stack overflow, the whole server
        // aborted. A thread of its own with a small stack (256 KiB) shows that the depth is limited now.
        let deep = format!("= Tiefe\n\nEin Satz zum Anfang.\n\n{}x{}\n", "#a[".repeat(60_000), "]".repeat(60_000));
        let started = std::time::Instant::now();
        let (points, rule) = std::thread::Builder::new()
            .stack_size(256 * 1024)
            .spawn(move || {
                assert!(refs_of(&deep).is_empty(), "keine Bibelstelle im Text");
                points_of(&deep)
            })
            .expect("Thread startet")
            .join()
            .expect("kein Stacküberlauf und keine Panik bei tiefer Verschachtelung");
        assert_eq!(rule, Rule::FirstSentence, "keine Hervorhebung im Text");
        assert_eq!(points, ["Ein Satz zum Anfang."], "der tief verschachtelte Rest liefert keinen Satz");
        assert!(
            started.elapsed() < std::time::Duration::from_secs(10),
            "linear statt quadratisch: {:?} für 240 KB",
            started.elapsed()
        );
        let nested = format!("{}innen{}", "#emph[".repeat(MAX_NESTING), "]".repeat(MAX_NESTING));
        assert_eq!(plain(&nested), "innen", "bis zur Grenze bleibt der Inhalt erhalten");
        let deeper = format!("{}innen{}", "#emph[".repeat(MAX_NESTING + 1), "]".repeat(MAX_NESTING + 1));
        assert_eq!(plain(&deeper), "", "tiefer Verschachteltes fällt weg");
    }

    #[test]
    fn refs_are_valid_normalized_and_unique() {
        let refs = refs_of("= Brot (Joh 6,35)\n\nJoh 6,35 und Johannes 6,35, dazu Mt 6,11 und Joh 99,1.\n// Lk 1,1\n");
        assert_eq!(refs, ["Joh 6,35", "Mt 6,11"], "Überschrift und Kommentar zählen nicht, ungültige Stelle fehlt");
    }
}
