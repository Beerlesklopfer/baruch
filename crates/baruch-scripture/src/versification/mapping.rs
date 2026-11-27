// SPDX-License-Identifier: AGPL-3.0-or-later
//! Mapping between two versifications (Round 17a): a verse or a reference of the one versification → verses of
//! the other.
//!
//! The pivot is KJV: the rule table `data/versification-map.tsv` describes per versification where a verse
//! belongs in KJV (header of the file: origin and check); the way from KJV into a versification is the same rule
//! reversed. A chapter without a rule counts as equal if it has the same number of verses in both versifications
//! – but only in a book for which the versification has rules or whose verse counts agree with KJV in every
//! chapter ([`unruled_book`]). A book without any rule that deviates from KJV somewhere cannot be mapped at all:
//! a psalm title as verse 1 (Catholic, Segond, Calvin) also shifts psalms that happen to have the same number of
//! verses (Ps 13: 6 = 6). What no rule covers and deviates in chapter length stays [`Mapped::Unmapped`] with
//! „Versifikation passt nicht: …“ – never guessed. Two versifications with equal verse counts in every chapter
//! of a book and the same rules for it map the book onto itself verse by verse ([`same_book`]).

use std::sync::LazyLock;

use serde::Serialize;

use super::{KJV, Versification};

/// A verse: book (OSIS code like [`crate::Book::id`]), chapter and verse.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub struct VerseRef {
    pub book: &'static str,
    pub chapter: u16,
    pub verse: u16,
}

impl VerseRef {
    /// German notation, e.g. „Joh 3,16“.
    pub fn label(&self) -> String {
        let abbr = crate::books().iter().find(|b| b.id == self.book).map_or(self.book, |b| b.abbr);
        format!("{abbr} {},{}", self.chapter, self.verse)
    }
}

/// Result of the mapping of a verse.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Mapped {
    /// Same reference in both versifications.
    Same { to: VerseRef },
    /// Another reference according to a known rule (`rule`: German short text from the rule table).
    Moved { to: VerseRef, rule: &'static str },
    /// Cannot be mapped; `reason` starts with „Versifikation passt nicht: “.
    Unmapped { reason: String },
}

impl Mapped {
    /// The target verse, if there is one.
    pub fn target(&self) -> Option<VerseRef> {
        match self {
            Mapped::Same { to } | Mapped::Moved { to, .. } => Some(*to),
            Mapped::Unmapped { .. } => None,
        }
    }
}

/// Beginning of every reason why a verse cannot be mapped.
pub const MISMATCH: &str = "Versifikation passt nicht: ";

/// Beginning of the rules for psalm titles without a verse number of their own in KJV: a reference "whole
/// chapter" leaves out such verses ([`map_range`]), a verse reference to them cannot be mapped.
const HEADING: &str = "Psalmüberschrift";

/// A line of the rule table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rule {
    /// Versifications (spelling of sword-modules).
    pub systems: Vec<&'static str>,
    pub book: &'static str,
    /// Chapter; `None` = the whole book cannot be mapped.
    pub chapter: Option<u16>,
    pub first: u16,
    pub last: u16,
    /// Target in KJV (chapter, first verse); `None` = no counterpart of its own.
    pub target: Option<(u16, u16)>,
    pub name: &'static str,
}

impl Rule {
    fn applies_to(&self, system: &str) -> bool {
        self.systems.iter().any(|s| s.eq_ignore_ascii_case(system))
    }

    fn covers(&self, chapter: u16, verse: u16) -> bool {
        self.chapter == Some(chapter) && (self.first..=self.last).contains(&verse)
    }

    /// KJV verse for a verse of the source (only within the rule).
    fn forward(&self, verse: u16) -> Option<(u16, u16)> {
        self.target.map(|(c, v)| (c, v + (verse - self.first)))
    }

    /// Verse of the source for a KJV verse, if the rule hits it.
    fn backward(&self, chapter: u16, verse: u16) -> Option<(u16, u16)> {
        let (tc, tv) = self.target?;
        let len = self.last - self.first;
        (tc == chapter && verse >= tv && verse - tv <= len)
            .then(|| (self.chapter.unwrap_or(0), self.first + (verse - tv)))
    }
}

/// The embedded table.
pub const TABLE: &str = include_str!("../../data/versification-map.tsv");

fn number(field: &str, line: usize) -> Result<u16, String> {
    field.parse().map_err(|_| format!("Zeile {line}: „{field}“ ist keine Zahl"))
}

/// Reads the rule table (`#` starts comments, columns see the header of `data/versification-map.tsv`).
pub fn parse_rules(text: &'static str) -> Result<Vec<Rule>, String> {
    let mut rules = Vec::new();
    for (n, line) in text.lines().enumerate() {
        let n = n + 1;
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        let cols: Vec<&'static str> = line.split('\t').collect();
        let [systems, book, chapter, first, last, tc, tv, name] = cols[..] else {
            return Err(format!("Zeile {n}: acht Spalten erwartet, {} gefunden", cols.len()));
        };
        let book = crate::books()
            .iter()
            .find(|b| b.id == book)
            .map(|b| b.id)
            .ok_or_else(|| format!("Zeile {n}: Buch „{book}“ unbekannt"))?;
        let (chapter, first, last) = if chapter == "*" {
            (None, 0, 0)
        } else {
            (Some(number(chapter, n)?), number(first, n)?, number(last, n)?)
        };
        if first > last {
            return Err(format!("Zeile {n}: erster Vers nach dem letzten"));
        }
        let target = if tc == "-" { None } else { Some((number(tc, n)?, number(tv, n)?)) };
        if name.trim().is_empty() {
            return Err(format!("Zeile {n}: Regelname fehlt"));
        }
        rules.push(Rule {
            systems: systems.split(',').map(str::trim).collect(),
            book,
            chapter,
            first,
            last,
            target,
            name,
        });
    }
    Ok(rules)
}

static RULES: LazyLock<Vec<Rule>> =
    LazyLock::new(|| parse_rules(TABLE).unwrap_or_else(|e| panic!("data/versification-map.tsv fehlerhaft: {e}")));

/// All rules of the embedded table.
pub fn rules() -> &'static [Rule] {
    &RULES
}

/// A step to or from the pivot KJV.
enum Step {
    To(VerseRef, Option<&'static str>),
    /// Cannot be mapped; `heading` = a psalm title without a verse number in KJV.
    Gone {
        reason: String,
        heading: bool,
    },
}

fn gone(reason: String) -> Step {
    Step::Gone { reason, heading: false }
}

fn kjv() -> &'static Versification {
    super::versification(KJV).expect("KJV ist eingebettet")
}

fn rules_of<'a>(system: &'a str, book: &'a str) -> impl Iterator<Item = &'static Rule> + 'a {
    RULES.iter().filter(move |r| r.book == book && r.applies_to(system))
}

/// A book without any rule for the versification whose verse counts deviate from KJV: reason why it cannot be
/// mapped. Equal chapter lengths then say nothing (a title as verse 1 also shifts a chapter with the same
/// number of verses), so no chapter of the book is guessed.
fn unruled_book(x: &Versification, book: &str) -> Option<String> {
    if x.name.eq_ignore_ascii_case(KJV) || rules_of(x.name, book).next().is_some() {
        return None;
    }
    let (own, kjv) = (x.chapters_of(book)?, kjv().chapters_of(book)?);
    (own != kjv).then(|| {
        format!(
            "für {} in {} gibt es keine geprüften Regeln, und die Verszahlen weichen von KJV ab",
            book_abbr(book),
            x.name
        )
    })
}

/// Do both versifications have the same verse counts in every chapter of the book and the same rules? Then
/// every verse of the book stays where it is (as with entirely equal versifications).
fn same_book(a: &Versification, b: &Versification, book: &str) -> bool {
    let (Some(x), Some(y)) = (a.chapters_of(book), b.chapters_of(book)) else { return false };
    let ra: Vec<*const Rule> = rules_of(a.name, book).map(|r| r as *const Rule).collect();
    let rb: Vec<*const Rule> = rules_of(b.name, book).map(|r| r as *const Rule).collect();
    x == y && ra == rb
}

/// Verse of the versification `x` → verse in KJV.
fn to_pivot(x: &Versification, v: VerseRef) -> Step {
    let len = x.verses_in(v.book, v.chapter);
    if v.verse == 0 || len.is_none_or(|n| v.verse > n) {
        return gone(format!("{} gibt es in {} nicht", v.label(), x.name));
    }
    if x.name.eq_ignore_ascii_case(KJV) {
        return Step::To(v, None);
    }
    let rules: Vec<&Rule> = rules_of(x.name, v.book).collect();
    if let Some(all) = rules.iter().find(|r| r.chapter.is_none()) {
        return gone(format!("{} ({})", all.name, x.name));
    }
    if let Some(reason) = unruled_book(x, v.book) {
        return gone(reason);
    }
    if let Some(rule) = rules.iter().find(|r| r.covers(v.chapter, v.verse)) {
        return match rule.forward(v.verse) {
            Some((chapter, verse)) => {
                let to = VerseRef { book: v.book, chapter, verse };
                Step::To(to, (to != v).then_some(rule.name))
            }
            None => Step::Gone {
                reason: format!("{} in {}: {}", v.label(), x.name, rule.name),
                heading: rule.name.starts_with(HEADING),
            },
        };
    }
    if rules.iter().any(|r| r.chapter == Some(v.chapter)) {
        return gone(format!("für {} in {} gibt es keine Regel", v.label(), x.name));
    }
    match kjv().verses_in(v.book, v.chapter) {
        Some(n) if Some(n) == len => Step::To(v, None),
        Some(n) => gone(format!(
            "Kapitel {} {} hat in {} {} Verse, in KJV {n}, und keine Regel deckt das ab",
            book_abbr(v.book),
            v.chapter,
            x.name,
            len.unwrap_or(0)
        )),
        None => gone(format!("Kapitel {} {} gibt es in KJV nicht", book_abbr(v.book), v.chapter)),
    }
}

/// Verse in KJV → verse of the versification `y`.
fn from_pivot(y: &Versification, k: VerseRef) -> Step {
    if y.name.eq_ignore_ascii_case(KJV) {
        return Step::To(k, None);
    }
    if y.chapters_of(k.book).is_none() {
        return gone(format!("das Buch {} gibt es in {} nicht", book_abbr(k.book), y.name));
    }
    let rules: Vec<&Rule> = rules_of(y.name, k.book).collect();
    if let Some(all) = rules.iter().find(|r| r.chapter.is_none()) {
        return gone(format!("{} ({})", all.name, y.name));
    }
    if let Some(reason) = unruled_book(y, k.book) {
        return gone(reason);
    }
    if let Some((rule, (chapter, verse))) = rules.iter().find_map(|r| r.backward(k.chapter, k.verse).map(|t| (r, t))) {
        let to = VerseRef { book: k.book, chapter, verse };
        return Step::To(to, (to != k).then_some(rule.name));
    }
    let touched = rules.iter().any(|r| r.chapter == Some(k.chapter) || r.target.is_some_and(|(c, _)| c == k.chapter));
    if touched {
        return gone(format!("{} (KJV) hat in {} keine eigene Entsprechung", k.label(), y.name));
    }
    let len = y.verses_in(k.book, k.chapter);
    match (kjv().verses_in(k.book, k.chapter), len) {
        (Some(a), Some(b)) if a == b => Step::To(k, None),
        (Some(a), Some(b)) => gone(format!(
            "Kapitel {} {} hat in KJV {a} Verse, in {} {b}, und keine Regel deckt das ab",
            book_abbr(k.book),
            k.chapter,
            y.name
        )),
        _ => gone(format!("Kapitel {} {} gibt es in {} nicht", book_abbr(k.book), k.chapter, y.name)),
    }
}

fn book_abbr(book: &str) -> &str {
    crate::books().iter().find(|b| b.id == book).map_or(book, |b| b.abbr)
}

/// Mapping with the note whether a verse that cannot be mapped is a psalm title.
fn map_verse_marked(from: &Versification, to: &Versification, verse: VerseRef) -> (Mapped, bool) {
    let unmapped = |reason: String, heading: bool| {
        (
            Mapped::Unmapped {
                reason: format!("{MISMATCH}{} ({} → {}): {reason}", verse.label(), from.name, to.name)
            },
            heading,
        )
    };
    // Same versification or equal verse counts in every chapter (German/Luther, KJV/KJVA, NRSV/NRSVA; per book
    // with the same rules, e.g. Catholic/Catholic2 outside the deuterocanonical books): every verse stays where
    // it is.
    if from.name.eq_ignore_ascii_case(to.name) || from.books().eq(to.books()) || same_book(from, to, verse.book) {
        return match from.verses_in(verse.book, verse.chapter) {
            Some(n) if verse.verse >= 1 && verse.verse <= n => (Mapped::Same { to: verse }, false),
            _ => unmapped(format!("{} gibt es in {} nicht", verse.label(), from.name), false),
        };
    }
    let (pivot, first_rule) = match to_pivot(from, verse) {
        Step::To(v, rule) => (v, rule),
        Step::Gone { reason, heading } => return unmapped(reason, heading),
    };
    match from_pivot(to, pivot) {
        Step::To(target, rule) if target == verse => {
            let _ = rule;
            (Mapped::Same { to: target }, false)
        }
        Step::To(target, rule) => match first_rule.or(rule) {
            Some(rule) => (Mapped::Moved { to: target, rule }, false),
            None => (Mapped::Same { to: target }, false),
        },
        Step::Gone { reason, heading } => unmapped(reason, heading),
    }
}

/// Map a verse from `from` to `to` (verse from 1).
pub fn map_verse(from: &Versification, to: &Versification, verse: VerseRef) -> Mapped {
    map_verse_marked(from, to, verse).0
}

/// A mapped reference: verses of the target versification in text order, without duplicates.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MappedRange {
    pub verses: Vec<VerseRef>,
    /// The reference has more than `limit` verses in the source versification.
    pub truncated: bool,
    /// Applied rules (without duplicates, in the order of their first appearance).
    pub rules: Vec<&'static str>,
}

/// Reference in `from` (verse 0 = whole chapter) → verses in `to`, order of the text, without duplicates, at
/// most `limit` (truncated = the reference has more than `limit` verses in `from`; exactly `limit` verses ⇒
/// false). Err(reason, German, „Versifikation passt nicht: …“ with both versification names) for an unknown
/// book/chapter in `from` or a verse that cannot be mapped. A reference "whole chapters" (verse 0 at both ends)
/// leaves out psalm titles that the target versification does not count as a verse (rule in `rules`).
pub fn map_range(
    from: &Versification,
    to: &Versification,
    book: &str,
    start: (u16, u16),
    end: (u16, u16),
    limit: usize,
) -> Result<MappedRange, String> {
    let book_id = from.books().map(|(id, _)| id).find(|id| *id == book).ok_or_else(|| {
        format!("{MISMATCH}das Buch {} gibt es in {} nicht (Ziel {})", book_abbr(book), from.name, to.name)
    })?;
    let total = from.count_range(book_id, start, end).ok_or_else(|| {
        format!(
            "{MISMATCH}{} {}–{} gibt es in {} nicht (Ziel {})",
            book_abbr(book_id),
            cv(start),
            cv(end),
            from.name,
            to.name
        )
    })?;
    let whole_chapters = start.1 == 0 && end.1 == 0;
    let mut out = MappedRange { verses: Vec::new(), truncated: total as usize > limit, rules: Vec::new() };
    let mut seen = 0usize;
    for chapter in start.0..=end.0 {
        let len = from.verses_in(book_id, chapter).unwrap_or(0);
        let first = if chapter == start.0 { start.1.max(1) } else { 1 };
        let last = if chapter == end.0 && end.1 != 0 { end.1 } else { len };
        for verse in first..=last {
            if seen >= limit {
                return Ok(out);
            }
            seen += 1;
            match map_verse_marked(from, to, VerseRef { book: book_id, chapter, verse }) {
                (Mapped::Same { to }, _) => push_unique(&mut out.verses, to),
                (Mapped::Moved { to, rule }, _) => {
                    push_unique(&mut out.verses, to);
                    push_rule(&mut out.rules, rule);
                }
                (Mapped::Unmapped { .. }, true) if whole_chapters => {
                    push_rule(&mut out.rules, "Psalmüberschrift ohne eigene Verszahl ausgelassen");
                }
                (Mapped::Unmapped { reason }, _) => return Err(reason),
            }
        }
    }
    Ok(out)
}

fn push_unique(verses: &mut Vec<VerseRef>, v: VerseRef) {
    if !verses.contains(&v) {
        verses.push(v);
    }
}

fn push_rule(rules: &mut Vec<&'static str>, rule: &'static str) {
    if !rules.contains(&rule) {
        rules.push(rule);
    }
}

fn cv((c, v): (u16, u16)) -> String {
    if v == 0 { c.to_string() } else { format!("{c},{v}") }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_parses_and_every_rule_has_a_name() {
        assert!(rules().len() > 100, "Regeln für Psalmen und Kapitelgrenzen: {}", rules().len());
        assert!(rules().iter().all(|r| !r.name.is_empty() && !r.systems.is_empty()), "jede Regel benannt");
    }

    #[test]
    fn a_broken_line_is_reported_with_its_number() {
        let err = parse_rules("# Kopf\nGerman\tGen\t1\t1\n").unwrap_err();
        assert!(err.contains("Zeile 2"), "Zeilennummer im Fehler: {err}");
        let err = parse_rules("German\tXyz\t1\t1\t1\t1\t1\tRegel\n").unwrap_err();
        assert!(err.contains("Xyz"), "unbekanntes Buch genannt: {err}");
    }
}
