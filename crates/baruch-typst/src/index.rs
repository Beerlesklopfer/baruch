// SPDX-License-Identifier: AGPL-3.0-or-later
//! Indexes and glossary (Round 31, contract §2.1; interface R31-S1, types and signatures **frozen** – changes only
//! through integrator/PM; bodies belong to the task register-satz).
//!
//! - **Index mark** in the Typst text, standard Typst only (no package, no network, can be typeset in every mirror
//!   file): `#metadata((register: "stichwort", term: "Gnade"))<baruch-idx>` – keys only in the order register,
//!   term, sub, see, also, sort; leave out empty ones; Typst string literals with `\"`, `\\`, `\n`, `\t`; no space
//!   before the label; the mark stands directly behind the marked word ([`mark_source`], [`parse_mark`]).
//! - **Indexes:** built in `stichwort` (keyword index), `personen` (index of persons), `bibel` (index of scripture
//!   references, automatically from recognised references); own identifiers `^[a-z][a-z0-9-]{0,31}$`, at most
//!   [`MAX_CUSTOM_REGISTERS`].
//! - **Keys:** [`term_key`] is stored in the database and is therefore immutable from S1 on (NFC, whitespace to one
//!   space, trimmed, lower case); [`sort_key`] is never stored.
//! - **Projection** ([`Projector`], [`back_matter`]): only export, export map, mirror, typesetting, PDF and EPUB;
//!   the texts in SQLite stay unchanged. Without active settings ([`BackMatter::is_active`]) the projection is the
//!   identity and the back matter empty – the export stays byte-identical to the state before Round 31.

mod back;
mod project;
mod scan;

use std::borrow::Cow;

use serde::{Deserialize, Serialize};
use unicode_normalization::UnicodeNormalization;

use crate::typeset::TypesetSettings;

/// Label of every index mark.
pub const INDEX_LABEL: &str = "baruch-idx";
/// Prefix of the labels of the glossary entries (`<baruch-gl-UUID>`).
pub const GLOSSARY_ANCHOR_PREFIX: &str = "baruch-gl-";
/// First line of the back matter (index pages and glossary page) that [`back_matter`] delivers.
pub const BACK_MATTER_MARK: &str = "// Baruch: Register und Glossar";
/// Identifier of the built-in keyword index.
pub const KEYWORD_REGISTER: &str = "stichwort";
/// Identifier of the built-in index of persons.
pub const PERSON_REGISTER: &str = "personen";
/// Identifier of the built-in index of scripture references (automatically from recognised references).
pub const SCRIPTURE_REGISTER: &str = "bibel";
/// Longest field of a mark (characters).
pub const MAX_MARK_FIELD_CHARS: usize = 200;
/// Longest title of an own index (characters).
pub const MAX_REGISTER_TITLE_CHARS: usize = 80;
/// Maximum number of own indexes per work.
pub const MAX_CUSTOM_REGISTERS: usize = 13;

/// An index mark: index, keyword and extras. Empty extras appear neither in JSON nor in Typst.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IndexMark {
    /// Identifier of the index ([`KEYWORD_REGISTER`], [`PERSON_REGISTER`], [`SCRIPTURE_REGISTER`] or an own one).
    pub register: String,
    /// Keyword.
    pub term: String,
    /// Sub-keyword (indented under `term`).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub sub: String,
    /// „siehe“: entry without a page number, refers to this keyword.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub see: String,
    /// „siehe auch“.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub also: String,
    /// Sort key, only for automatic entries (Bible: biblical order of the books).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub sort: String,
}

impl IndexMark {
    /// Mark with index and keyword, without extras.
    pub fn new(register: &str, term: &str) -> Self {
        IndexMark { register: register.to_owned(), term: term.to_owned(), ..IndexMark::default() }
    }

    /// Checks the mark: valid index identifier ([`valid_register_slug`]), keyword not empty, every field at most
    /// [`MAX_MARK_FIELD_CHARS`] characters and without control characters (line break, tab …).
    pub fn validate(&self) -> Result<(), IndexError> {
        if !valid_register_slug(&self.register) {
            return Err(IndexError::Invalid(format!(
                "Register „{}“: keine gültige Kennung (Kleinbuchstabe am Anfang, dann Kleinbuchstaben, Ziffern oder \
                 Bindestrich, höchstens 32 Zeichen).",
                self.register
            )));
        }
        if self.term.trim().is_empty() {
            return Err(IndexError::Invalid("Stichwort: darf nicht leer sein.".into()));
        }
        let fields = [
            ("Stichwort", &self.term),
            ("Unterstichwort", &self.sub),
            ("siehe", &self.see),
            ("siehe auch", &self.also),
            ("Sortierschlüssel", &self.sort),
        ];
        for (label, value) in fields {
            if value.chars().count() > MAX_MARK_FIELD_CHARS {
                return Err(IndexError::Invalid(format!("{label}: höchstens {MAX_MARK_FIELD_CHARS} Zeichen.")));
            }
            if value.chars().any(char::is_control) {
                return Err(IndexError::Invalid(format!("{label}: ohne Zeilenumbruch und Steuerzeichen.")));
            }
        }
        Ok(())
    }

    /// The fields in Typst order with their keys.
    fn fields(&self) -> [(&'static str, &String); 6] {
        [
            ("register", &self.register),
            ("term", &self.term),
            ("sub", &self.sub),
            ("see", &self.see),
            ("also", &self.also),
            ("sort", &self.sort),
        ]
    }
}

/// Typst string literal: `\"`, `\\`, `\n`, `\t`, `\r`; remaining control characters as `\u{…}`.
fn typst_string(text: &str, out: &mut String) {
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            c if c.is_control() => out.push_str(&format!("\\u{{{:x}}}", u32::from(c))),
            c => out.push(c),
        }
    }
    out.push('"');
}

/// Canonical Typst form of the mark: `#metadata((register: "…", term: "…"[, sub: "…"]…))<baruch-idx>`.
pub fn mark_source(mark: &IndexMark) -> String {
    let mut out = String::from("#metadata((");
    let mut first = true;
    for (key, value) in mark.fields() {
        // register and term always stand there (also empty), so that the mark stays a dictionary.
        if value.is_empty() && key != "register" && key != "term" {
            continue;
        }
        if !first {
            out.push_str(", ");
        }
        first = false;
        out.push_str(key);
        out.push_str(": ");
        typst_string(value, &mut out);
    }
    out.push_str("))<");
    out.push_str(INDEX_LABEL);
    out.push('>');
    out
}

/// Small reader for [`parse_mark`] over bytes (the mark itself is ASCII except for the contents of the strings).
struct Cursor<'s> {
    src: &'s str,
    pos: usize,
}

impl Cursor<'_> {
    fn eat(&mut self, token: &str) -> bool {
        if self.src[self.pos..].starts_with(token) {
            self.pos += token.len();
            true
        } else {
            false
        }
    }

    /// Whitespace within the line (space and tab).
    fn space(&mut self) {
        while matches!(self.src.as_bytes().get(self.pos), Some(b' ' | b'\t')) {
            self.pos += 1;
        }
    }

    fn ident(&mut self) -> Option<&str> {
        let start = self.pos;
        while matches!(self.src.as_bytes().get(self.pos), Some(b'a'..=b'z')) {
            self.pos += 1;
        }
        (self.pos > start).then(|| &self.src[start..self.pos])
    }

    /// Typst string literal without a raw line break; escapes `\"`, `\\`, `\n`, `\t`, `\r`, `\u{…}`.
    fn string(&mut self) -> Option<String> {
        if !self.eat("\"") {
            return None;
        }
        let mut out = String::new();
        let mut chars = self.src[self.pos..].char_indices();
        while let Some((i, c)) = chars.next() {
            match c {
                '"' => {
                    self.pos += i + 1;
                    return Some(out);
                }
                '\n' | '\r' => return None,
                '\\' => match chars.next()?.1 {
                    '"' => out.push('"'),
                    '\\' => out.push('\\'),
                    'n' => out.push('\n'),
                    't' => out.push('\t'),
                    'r' => out.push('\r'),
                    'u' => {
                        if chars.next()?.1 != '{' {
                            return None;
                        }
                        let mut hex = String::new();
                        loop {
                            let (_, h) = chars.next()?;
                            if h == '}' {
                                break;
                            }
                            if !h.is_ascii_hexdigit() || hex.len() >= 6 {
                                return None;
                            }
                            hex.push(h);
                        }
                        out.push(char::from_u32(u32::from_str_radix(&hex, 16).ok()?)?);
                    }
                    _ => return None,
                },
                c => out.push(c),
            }
        }
        None
    }
}

/// Reads a canonical mark at the start of `source` (whitespace around `:` and `,` as well as inside at the
/// parentheses allowed, keys only in the order register, term, sub, see, also, sort, register and term mandatory,
/// no space before the label). Returns the mark and the consumed bytes; everything else (also a mark that does not
/// pass [`IndexMark::validate`]) → `None` and stays an island.
pub fn parse_mark(source: &str) -> Option<(IndexMark, usize)> {
    const KEYS: [&str; 6] = ["register", "term", "sub", "see", "also", "sort"];
    let mut cur = Cursor { src: source, pos: 0 };
    if !cur.eat("#metadata((") {
        return None;
    }
    let mut mark = IndexMark::default();
    let mut next_key = 0usize;
    let mut seen = [false; 6];
    cur.space();
    loop {
        if cur.eat(")") {
            break;
        }
        let key = cur.ident()?;
        let index = KEYS.iter().position(|k| *k == key)?;
        if index < next_key {
            return None;
        }
        next_key = index + 1;
        seen[index] = true;
        cur.space();
        if !cur.eat(":") {
            return None;
        }
        cur.space();
        let value = cur.string()?;
        match index {
            0 => mark.register = value,
            1 => mark.term = value,
            2 => mark.sub = value,
            3 => mark.see = value,
            4 => mark.also = value,
            _ => mark.sort = value,
        }
        cur.space();
        if cur.eat(",") {
            cur.space();
            continue;
        }
        if !cur.eat(")") {
            return None;
        }
        break;
    }
    if !(seen[0] && seen[1]) {
        return None;
    }
    if !cur.eat(")") || !cur.eat("<") || !cur.eat(INDEX_LABEL) || !cur.eat(">") {
        return None;
    }
    mark.validate().ok()?;
    Some((mark, cur.pos))
}

/// Which occurrence of a word is marked: the first, all or the n-th (from 1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(into = "String", try_from = "String")]
pub enum Occurrence {
    First,
    All,
    Nth(u32),
}

impl Occurrence {
    /// `"first"`, `"all"` or the number as text.
    pub fn as_param(self) -> String {
        match self {
            Occurrence::First => "first".to_owned(),
            Occurrence::All => "all".to_owned(),
            Occurrence::Nth(n) => n.to_string(),
        }
    }

    /// Reverses [`Occurrence::as_param`]; the number must be at least 1. Unknown → `None`.
    pub fn from_param(s: &str) -> Option<Self> {
        match s.trim() {
            "first" => Some(Occurrence::First),
            "all" => Some(Occurrence::All),
            n if !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()) => {
                n.parse::<u32>().ok().filter(|n| *n >= 1).map(Occurrence::Nth)
            }
            _ => None,
        }
    }
}

impl From<Occurrence> for String {
    fn from(occurrence: Occurrence) -> String {
        occurrence.as_param()
    }
}

impl TryFrom<String> for Occurrence {
    type Error = String;

    fn try_from(value: String) -> Result<Self, String> {
        Occurrence::from_param(&value)
            .ok_or_else(|| format!("Fundstelle „{value}“: erwartet „first“, „all“ oder eine Zahl ab 1"))
    }
}

/// A mark in the text with its byte range (`span.0..span.1` = the whole mark).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FoundMark {
    pub span: (usize, usize),
    pub mark: IndexMark,
}

/// Errors when setting, removing and checking marks.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum IndexError {
    #[error("{0}")]
    Invalid(String),
    #[error("„{0}“ steht nicht im Abschnitt (außerhalb von Überschriftszeilen, Code, Kommentaren und Marken).")]
    WordNotFound(String),
    #[error(
        "„{0}“ trägt an der gewählten Fundstelle schon diese Registermarke; nichts geändert. Eine andere Fundstelle \
         wählen („alle“ setzt nur die fehlenden Marken)."
    )]
    AlreadyMarked(String),
}

/// All canonical marks in `text` (outside of code, comments and raw), in text order. A mark in a content block
/// (`#emph[Wort#metadata(…)<baruch-idx>]`) counts; a mark in a comment, in raw or in a string does not (Typst does
/// not set it there).
pub fn find_marks(text: &str) -> Vec<FoundMark> {
    scan::scan(text).marks
}

/// Occurrences of `word` in the running text: start and end. The word is searched literally (case matters);
/// whitespace in the word matches any whitespace without an empty line. Whole words: before and behind the
/// occurrence there is no letter and no digit if the word starts or ends with one there.
fn find_word(text: &str, scanned: &scan::Scan, word: &str) -> Vec<(usize, usize)> {
    let pieces: Vec<&str> = word.split_whitespace().collect();
    let Some(first) = pieces.first() else { return Vec::new() };
    let starts_alnum = word.trim().chars().next().is_some_and(char::is_alphanumeric);
    let ends_alnum = word.trim().chars().next_back().is_some_and(char::is_alphanumeric);
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(found) = text[from..].find(first) {
        let start = from + found;
        from = start + first.len();
        let mut end = start + first.len();
        let mut ok = true;
        for piece in &pieces[1..] {
            let gap = text[end..].len() - text[end..].trim_start().len();
            let between = &text[end..end + gap];
            if gap == 0 || between.matches('\n').count() > 1 || !text[end + gap..].starts_with(piece) {
                ok = false;
                break;
            }
            end += gap + piece.len();
        }
        if !ok || !scanned.prose_range(start, end) {
            continue;
        }
        if starts_alnum && text[..start].chars().next_back().is_some_and(char::is_alphanumeric) {
            continue;
        }
        if ends_alnum && text[end..].chars().next().is_some_and(char::is_alphanumeric) {
            continue;
        }
        out.push((start, end));
    }
    out
}

/// Sets `mark` directly behind the chosen occurrence(s) of `word` (whole words, not in heading lines, code,
/// comments, raw or other marks). Result: new text and number of marks set.
///
/// If a label stands directly behind the word (`Wort<name>`), the mark goes behind the label (otherwise the label
/// would hang on the mark). Invalid mark or empty word → `Invalid`; no (or no n-th) occurrence → `WordNotFound`.
///
/// Nothing is marked twice: if an occurrence already carries the same mark (in the chain of marks directly behind
/// the word or label), it stays as it is. "first" and "n-th" still mean the first or n-th occurrence in the text;
/// if precisely that one is already marked, `AlreadyMarked` comes (nothing changed, no new revision). "all" only
/// sets the missing marks; if none is missing any more → `AlreadyMarked` as well.
pub fn mark_word(
    text: &str,
    word: &str,
    occurrence: Occurrence,
    mark: &IndexMark,
) -> Result<(String, usize), IndexError> {
    mark.validate()?;
    let word = word.trim();
    if word.is_empty() {
        return Err(IndexError::Invalid("Wort: darf nicht leer sein.".into()));
    }
    let scanned = scan::scan(text);
    let mut spots: Vec<usize> = Vec::new();
    for (_, end) in find_word(text, &scanned, word) {
        let at = scanned.label_at(end).unwrap_or(end);
        if scanned.insertable(text, at) {
            spots.push(at);
        }
    }
    let chosen: Vec<usize> = match occurrence {
        Occurrence::First => spots.into_iter().take(1).collect(),
        Occurrence::All => spots,
        Occurrence::Nth(n) => spots.into_iter().nth(n.saturating_sub(1) as usize).into_iter().collect(),
    };
    if chosen.is_empty() {
        return Err(IndexError::WordNotFound(word.to_owned()));
    }
    let chosen: Vec<usize> = chosen.into_iter().filter(|&spot| !carries_mark(&scanned.marks, spot, mark)).collect();
    if chosen.is_empty() {
        return Err(IndexError::AlreadyMarked(word.to_owned()));
    }
    let source = mark_source(mark);
    let mut out = String::with_capacity(text.len() + chosen.len() * source.len());
    let mut at = 0;
    for &spot in &chosen {
        out.push_str(&text[at..spot]);
        out.push_str(&source);
        at = spot;
    }
    out.push_str(&text[at..]);
    Ok((out, chosen.len()))
}

/// Does `mark` already stand in the chain of marks that starts at `spot` (`Wort#metadata(…)<baruch-idx>#metadata(…)…`)?
fn carries_mark(marks: &[FoundMark], spot: usize, mark: &IndexMark) -> bool {
    let mut at = spot;
    while let Some(found) = marks.iter().find(|m| m.span.0 == at) {
        if found.mark == *mark {
            return true;
        }
        at = found.span.1;
    }
    false
}

/// Sets `mark` at the byte position `byte`; in heading lines, code, comments, raw or a mark → `Invalid`.
pub fn insert_mark_at(text: &str, byte: usize, mark: &IndexMark) -> Result<String, IndexError> {
    mark.validate()?;
    if byte > text.len() || !text.is_char_boundary(byte) {
        return Err(IndexError::Invalid(format!("Stelle {byte} liegt nicht im Text bzw. mitten in einem Zeichen.")));
    }
    let scanned = scan::scan(text);
    if !scanned.insertable(text, byte) {
        return Err(IndexError::Invalid(format!(
            "An Stelle {byte} darf keine Registermarke stehen (Überschriftszeile, Code, Kommentar, Raw, Label oder \
             andere Marke)."
        )));
    }
    let source = mark_source(mark);
    let mut out = String::with_capacity(text.len() + source.len());
    out.push_str(&text[..byte]);
    out.push_str(&source);
    out.push_str(&text[byte..]);
    Ok(out)
}

/// Removes the mark exactly in the range `span` (like [`FoundMark::span`]); not a mark range → `Invalid`.
pub fn remove_mark(text: &str, span: (usize, usize)) -> Result<String, IndexError> {
    if !find_marks(text).iter().any(|m| m.span == span) {
        return Err(IndexError::Invalid(format!(
            "Im Bereich {}–{} steht keine Registermarke (Text inzwischen geändert?).",
            span.0, span.1
        )));
    }
    let mut out = String::with_capacity(text.len() - (span.1 - span.0));
    out.push_str(&text[..span.0]);
    out.push_str(&text[span.1..]);
    Ok(out)
}

/// Kind of an index.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RegisterKind {
    Keyword,
    Person,
    Scripture,
    Custom,
}

/// An index: identifier, title and kind.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Register {
    pub slug: String,
    pub title: String,
    pub kind: RegisterKind,
}

/// The three built-in indexes in fixed order.
pub fn builtin_registers() -> Vec<Register> {
    [
        (KEYWORD_REGISTER, "Stichwortregister", RegisterKind::Keyword),
        (PERSON_REGISTER, "Personenregister", RegisterKind::Person),
        (SCRIPTURE_REGISTER, "Bibelstellenregister", RegisterKind::Scripture),
    ]
    .into_iter()
    .map(|(slug, title, kind)| Register { slug: slug.to_owned(), title: title.to_owned(), kind })
    .collect()
}

/// `^[a-z][a-z0-9-]{0,31}$` (the built-in identifiers fulfil that too).
pub fn valid_register_slug(slug: &str) -> bool {
    let bytes = slug.as_bytes();
    !bytes.is_empty()
        && bytes.len() <= 32
        && bytes[0].is_ascii_lowercase()
        && bytes.iter().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || *b == b'-')
}

/// Comparison key of a term (stored in the database, therefore **immutable**): NFC, every whitespace collapsed to
/// one space, trimmed, `to_lowercase`.
pub fn term_key(term: &str) -> String {
    let nfc: String = term.nfc().collect();
    nfc.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}

/// Sort key of a term (never stored): DIN 5007-1 – umlauts like their base letters (ä = a, ö = o, ü = u), ß = ss,
/// other diacritics are dropped (é = e); lower case, whitespace collapsed, punctuation at the start (quotation
/// marks, parentheses) skipped.
pub fn sort_key(term: &str) -> String {
    let base: String = term
        .nfd()
        .filter(|c| !unicode_normalization::char::is_combining_mark(*c))
        .flat_map(char::to_lowercase)
        .map(|c| if c == 'ß' { "ss".to_owned() } else { c.to_string() })
        .collect();
    let words = base.split_whitespace().collect::<Vec<_>>().join(" ");
    let trimmed = words.trim_start_matches(|c: char| !c.is_alphanumeric());
    if trimmed.is_empty() { words } else { trimmed.to_owned() }
}

/// Label of a glossary entry: `baruch-gl-` + uuid.
pub fn glossary_anchor(uuid: &str) -> String {
    format!("{GLOSSARY_ANCHOR_PREFIX}{uuid}")
}

/// An accepted glossary term as typesetting and EPUB need it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GlossaryEntry {
    pub uuid: String,
    pub term: String,
    pub variants: Vec<String>,
    pub short: String,
    pub long: String,
    pub bible_refs: Vec<String>,
}

/// Everything projection and back matter of a work need: typesetting settings, indexes (built-in + own) and the
/// accepted glossary terms.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct BackMatter {
    pub settings: TypesetSettings,
    pub registers: Vec<Register>,
    pub glossary: Vec<GlossaryEntry>,
}

impl BackMatter {
    /// Active as soon as index pages, glossary page or the markup of the first mention are configured.
    pub fn is_active(&self) -> bool {
        self.settings.index_pages.is_some()
            || self.settings.glossary_page.is_some()
            || self.settings.glossary_first_mention == Some(true)
    }
}

/// Projection of the section texts of an export (one instance per export, sections in tree order).
///
/// Sets (a) with index `bibel` in `index_pages.registers` automatic marks behind recognised scripture references
/// and (b) with `glossary_first_mention == Some(true)` the markup of the first mention per chapter (form and rules
/// in `index/project.rs`: with glossary page `#link(<baruch-gl-UUID>)[#emph[Wort]]`, otherwise `#emph[Wort]`).
pub struct Projector<'a> {
    plan: project::Plan,
    _back: std::marker::PhantomData<&'a BackMatter>,
}

impl<'a> Projector<'a> {
    /// New projection for a work.
    pub fn new(back: &'a BackMatter) -> Self {
        Projector { plan: project::Plan::new(back), _back: std::marker::PhantomData }
    }

    /// Projects a section text: automatic scripture reference marks, markup of the first mention per chapter.
    /// Inactive ([`BackMatter::is_active`]) → `Cow::Borrowed`.
    pub fn project<'b>(&mut self, body: &'b str) -> Cow<'b, str> {
        self.plan.project(body)
    }
}

/// Counterpart to [`back_matter`] for the import (integration R31, Round 21b): if `source` has a line that reads
/// exactly [`BACK_MATTER_MARK`], **and** the helper functions that only [`back_matter`] writes follow it
/// immediately (`#let baruch-idx-ranges(ns) = …`), everything from this line on is the generated back matter of an
/// export; it belongs to no section and is created anew from the settings on the next export. A marker line without
/// this block (for example a comment by hand in the book text) cuts off nothing – the caller reports that
/// ([`has_back_matter_mark`]). Returned: the text up to before the marker line and whether something was cut off.
pub fn strip_back_matter(source: &str) -> (&str, bool) {
    let mut start = 0;
    for line in source.split_inclusive('\n') {
        let generated = source[start + line.len()..].starts_with(back::HELPERS);
        if generated && line.trim_end_matches(['\n', '\r']) == BACK_MATTER_MARK {
            return (&source[..start], true);
        }
        start += line.len();
    }
    (source, false)
}

/// Does `source` have a line that reads exactly [`BACK_MATTER_MARK`] (generated or by hand)?
pub fn has_back_matter_mark(source: &str) -> bool {
    source.lines().any(|line| line.trim_end_matches('\r') == BACK_MATTER_MARK)
}

/// Invisible note of the projection before a first mention without glossary page (Round 21b): `/*…*/#emph[Wort]`.
/// This way the import recognises the markup again ([`strip_first_mentions`]) without touching emphasis written by
/// hand.
pub const FIRST_MENTION_MARK: &str = "/*baruch-erste-erwaehnung*/";

/// Takes the noted first mention without glossary page (`/*baruch-erste-erwaehnung*/#emph[Wort]`, including the
/// guard `;`) out again: what remains is `Wort`. Independent of the back matter (without glossary page and indexes
/// there is none); everything else stays, also an emphasis without the note.
pub fn strip_first_mentions(source: &str) -> Cow<'_, str> {
    let open = format!("{FIRST_MENTION_MARK}#emph[");
    if !source.contains(&open) {
        return Cow::Borrowed(source);
    }
    let mut out = String::with_capacity(source.len());
    let mut at = 0;
    while let Some(found) = source[at..].find(&open).map(|i| at + i) {
        let word_start = found + open.len();
        let Some(word_len) = source[word_start..].find(']') else { break };
        let word = &source[word_start..word_start + word_len];
        // Round 23a (code review satz-1): the projection allows **one** line break between two words of a term
        // (hard-wrapped source) and writes it into the markup. It therefore belongs to the word and comes back with
        // it; before, the markup stayed as text in the section after the re-import. The projection never writes an
        // empty line or a `[` in the word – that is then none of its markups.
        if word.contains('[') || (word.contains('\n') && word.split('\n').any(|line| line.trim().is_empty())) {
            out.push_str(&source[at..word_start]);
            at = word_start;
            continue;
        }
        let mut end = word_start + word_len + 1;
        if source[end..].starts_with(';') && guarded(&source[end + 1..]) {
            end += 1;
        }
        out.push_str(&source[at..found]);
        out.push_str(word);
        at = end;
    }
    out.push_str(&source[at..]);
    Cow::Owned(out)
}

/// Takes the additions of the [`Projector`] out of an export with back matter again (integration R31, import): the
/// automatic marks of the index of scripture references (index `bibel` with `sort`, which only the projection sets)
/// and the linked first mention `#link(<baruch-gl-UUID>)[#emph[Wort]]` (including the guard `;` of the projection).
/// Otherwise the next export would produce duplicate scripture references and references to glossary labels that do
/// not exist in the new work. The markup without glossary page has carried the note [`FIRST_MENTION_MARK`] since
/// Round 21b and is dropped as well ([`strip_first_mentions`]); an emphasis without the note stays.
pub fn strip_projection(source: &str) -> Cow<'_, str> {
    match strip_first_mentions(source) {
        Cow::Borrowed(_) => strip_linked_projection(source),
        Cow::Owned(unmarked) => Cow::Owned(strip_linked_projection(&unmarked).into_owned()),
    }
}

/// Take out automatic scripture reference marks and linked first mentions (part of [`strip_projection`]).
fn strip_linked_projection(source: &str) -> Cow<'_, str> {
    let mut cuts: Vec<(usize, usize, String)> = find_marks(source)
        .into_iter()
        .filter(|f| f.mark.register == SCRIPTURE_REGISTER && !f.mark.sort.is_empty())
        .map(|f| (f.span.0, f.span.1, String::new()))
        .collect();
    let open = format!("#link(<{GLOSSARY_ANCHOR_PREFIX}");
    let mut from = 0;
    while let Some(at) = source[from..].find(&open).map(|i| from + i) {
        let rest = &source[at + open.len()..];
        let parsed = rest.find(">)[#emph[").and_then(|label_end| {
            let label = &rest[..label_end];
            let word_start = label_end + ">)[#emph[".len();
            let word_len = rest[word_start..].find("]]")?;
            let valid = !label.is_empty() && label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-');
            valid.then_some((word_start, word_len))
        });
        let Some((word_start, word_len)) = parsed else {
            from = at + open.len();
            continue;
        };
        let word = &rest[word_start..word_start + word_len];
        let mut end = at + open.len() + word_start + word_len + 2;
        if source[end..].starts_with(';') && guarded(&source[end + 1..]) {
            end += 1;
        }
        cuts.push((at, end, word.to_owned()));
        from = end;
    }
    if cuts.is_empty() {
        return Cow::Borrowed(source);
    }
    cuts.sort_by_key(|c| c.0);
    let mut out = String::with_capacity(source.len());
    let mut at = 0;
    for (a, b, with) in cuts {
        if a < at {
            continue;
        }
        out.push_str(&source[at..a]);
        out.push_str(&with);
        at = b;
    }
    out.push_str(&source[at..]);
    Cow::Owned(out)
}

/// Would the projection have set a guard `;` before `after`? (Counterpart to `needs_guard` in `index/project.rs`.)
fn guarded(after: &str) -> bool {
    let mut chars = after.chars();
    match chars.next() {
        Some('(' | '[' | ';') => true,
        Some('.') => chars.next().is_some_and(|c| c.is_alphabetic() || c == '_'),
        _ => false,
    }
}

/// Back matter (index pages and glossary page) for the projected section texts `bodies`; inactive or without index
/// pages and glossary page → "". Otherwise a block from the line [`BACK_MATTER_MARK`] on: helper functions, then
/// one page per configured index (order of `index_pages.registers`, unknown identifiers are dropped) and the
/// glossary page before or after the indexes (details in `index/back.rs`).
pub fn back_matter(back: &BackMatter, bodies: &[&str]) -> String {
    if !back.is_active() {
        return String::new();
    }
    let mut collected = back::Collected::new();
    for (i, body) in bodies.iter().enumerate() {
        for found in find_marks(body) {
            back::collect(&mut collected, i as i64, &found.mark, back::is_auto(&found.mark));
        }
    }
    back::render(back, &collected)
}

/// A keyword of the overview with the sections (`keys`) in which it occurs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct IndexTerm {
    pub term: String,
    pub sub: String,
    pub keys: Vec<i64>,
    pub see: Vec<String>,
    pub also: Vec<String>,
    pub auto: bool,
}

/// Overview of an index.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RegisterView {
    pub register: Register,
    pub terms: Vec<IndexTerm>,
}

/// Overview per index over the sections `(key, text)` (stored, not projected texts): all indexes from
/// `back.registers` in their order, keywords sorted as in the back matter, `keys` = sections with marks that carry
/// a page. If the index of scripture references is switched on, the automatic scripture references of the
/// projection are added (`auto`). Marks with an unknown index do not appear.
pub fn overview(back: &BackMatter, bodies: &[(i64, &str)]) -> Vec<RegisterView> {
    let scripture = project::scripture_enabled(back);
    let mut collected = back::Collected::new();
    for &(key, body) in bodies {
        let scanned = scan::scan(body);
        for found in &scanned.marks {
            back::collect(&mut collected, key, &found.mark, back::is_auto(&found.mark));
        }
        if scripture {
            for (_, mark) in project::scripture_marks(body, &scanned) {
                back::collect(&mut collected, key, &mark, true);
            }
        }
    }
    back::views(back, &collected)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mark_source_writes_the_contract_form() {
        assert_eq!(
            mark_source(&IndexMark::new("stichwort", "Gnade")),
            r#"#metadata((register: "stichwort", term: "Gnade"))<baruch-idx>"#,
            "Form laut Vertrag §2.1, kein Leerzeichen vor dem Label"
        );
        let mark = IndexMark {
            register: "personen".into(),
            term: "Ruth \"die Moabiterin\"".into(),
            sub: "Weg\\Ziel".into(),
            also: "Noomi".into(),
            ..IndexMark::default()
        };
        assert_eq!(
            mark_source(&mark),
            r#"#metadata((register: "personen", term: "Ruth \"die Moabiterin\"", sub: "Weg\\Ziel", also: "Noomi"))<baruch-idx>"#,
            "leere Schlüssel fehlen, Anführungszeichen und Rückstrich sind maskiert"
        );
    }

    #[test]
    fn parse_mark_reverses_mark_source_and_counts_bytes() {
        let mark = IndexMark {
            register: "orte".into(),
            term: "Ölberg".into(),
            sub: "Garten".into(),
            see: "Berg".into(),
            also: "Tal".into(),
            sort: "0001".into(),
        };
        let source = mark_source(&mark);
        let text = format!("{source} danach");
        assert_eq!(parse_mark(&text), Some((mark, source.len())), "Rundreise, verbrauchte Bytes = Länge der Marke");
    }

    #[test]
    fn parse_mark_allows_space_but_refuses_everything_else() {
        let spaced = r#"#metadata(( register : "stichwort" ,term:"Gnade" ))<baruch-idx>"#;
        assert_eq!(
            parse_mark(spaced).map(|(m, n)| (m, n == spaced.len())),
            Some((IndexMark::new("stichwort", "Gnade"), true)),
            "Leerraum um : und , ist erlaubt"
        );
        for island in [
            r#"#metadata((register: "stichwort", term: "Gnade")) <baruch-idx>"#,
            r#"#metadata((term: "Gnade", register: "stichwort"))<baruch-idx>"#,
            r#"#metadata((register: "stichwort"))<baruch-idx>"#,
            r#"#metadata((register: "stichwort", term: "Gnade", farbe: "rot"))<baruch-idx>"#,
            r#"#metadata((register: "stichwort", term: "Gnade"))<anderes>"#,
            r#"#metadata((register: "Stichwort", term: "Gnade"))<baruch-idx>"#,
            "#metadata((register: \"stichwort\", term: \"Gna\nde\"))<baruch-idx>",
        ] {
            assert_eq!(parse_mark(island), None, "keine kanonische Marke, bleibt Insel: {island}");
        }
    }

    #[test]
    fn validate_checks_register_term_and_lengths() {
        assert_eq!(IndexMark::new("stichwort", "Gnade").validate(), Ok(()), "gültige Marke");
        assert!(IndexMark::new("Stichwort", "Gnade").validate().is_err(), "Großbuchstabe in der Kennung");
        assert!(IndexMark::new("stichwort", "  ").validate().is_err(), "leeres Stichwort");
        let long = "x".repeat(MAX_MARK_FIELD_CHARS + 1);
        assert!(IndexMark::new("stichwort", &long).validate().is_err(), "zu langes Stichwort");
    }

    #[test]
    fn occurrence_params_round_trip() {
        for o in [Occurrence::First, Occurrence::All, Occurrence::Nth(3)] {
            assert_eq!(Occurrence::from_param(&o.as_param()), Some(o), "from_param kehrt as_param um: {o:?}");
            let json = serde_json::to_string(&o).expect("serialisierbar");
            assert_eq!(serde_json::from_str::<Occurrence>(&json).ok(), Some(o), "JSON als Text: {json}");
        }
        assert_eq!(Occurrence::from_param("0"), None, "die Zählung beginnt bei 1");
        assert_eq!(Occurrence::from_param("zweite"), None, "Unbekanntes wird abgelehnt");
    }

    #[test]
    fn slugs_keys_and_anchor() {
        assert!(builtin_registers().iter().all(|r| valid_register_slug(&r.slug)), "eingebaute Kennungen gültig");
        assert!(valid_register_slug("orte-2"), "eigene Kennung");
        assert!(!valid_register_slug("2orte") && !valid_register_slug("") && !valid_register_slug(&"a".repeat(33)));
        assert_eq!(term_key("  Gna\u{0308}de \t im   Alltag "), "gnäde im alltag", "NFC, Leerraum, klein");
        assert_eq!(glossary_anchor("abc"), "baruch-gl-abc", "Label der Glossarseite");
    }

    #[test]
    fn inactive_back_matter_changes_nothing() {
        let back = BackMatter::default();
        assert!(!back.is_active(), "ohne R31-Einstellungen inaktiv");
        let mut projector = Projector::new(&back);
        assert!(matches!(projector.project("= Kapitel\nText\n"), Cow::Borrowed(_)), "Projektion = Identität");
        assert_eq!(back_matter(&back, &["Text"]), "", "kein Buchende");
        let active = BackMatter {
            settings: TypesetSettings { glossary_first_mention: Some(true), ..TypesetSettings::default() },
            ..BackMatter::default()
        };
        assert!(active.is_active(), "erste Erwähnung = aktiv");
    }
}
