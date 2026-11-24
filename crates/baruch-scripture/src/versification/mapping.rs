// SPDX-License-Identifier: AGPL-3.0-or-later
//! Abbildung zwischen zwei Verszählungen (Runde 17a): ein Vers bzw. eine Stelle der einen Zählung → Verse der
//! anderen.
//!
//! Drehpunkt ist KJV: Die Regeltabelle `data/versification-map.tsv` beschreibt je Zählung, wohin ein Vers in KJV
//! gehört (Kopf der Datei: Herkunft und Prüfung); der Weg von KJV in eine Zählung ist dieselbe Regel umgekehrt.
//! Ein Kapitel ohne Regel gilt als gleich, wenn es in beiden Zählungen gleich viele Verse hat – aber nur in einem
//! Buch, für das die Zählung Regeln hat oder dessen Verszahlen in jedem Kapitel mit KJV übereinstimmen
//! ([`unruled_book`]). Ein Buch ohne jede Regel, das irgendwo von KJV abweicht, ist ganz nicht abbildbar: Eine
//! Psalmüberschrift als Vers 1 (Catholic, Segond, Calvin) verschiebt auch Psalmen, die zufällig gleich viele Verse
//! haben (Ps 13: 6 = 6). Was keine Regel deckt und in der Kapitellänge abweicht, bleibt [`Mapped::Unmapped`] mit
//! „Versifikation passt nicht: …“ – nie geraten. Zwei Zählungen mit gleichen Verszahlen in jedem Kapitel eines Buchs
//! und denselben Regeln dafür bilden das Buch Vers für Vers auf sich ab ([`same_book`]).

use std::sync::LazyLock;

use serde::Serialize;

use super::{KJV, Versification};

/// Ein Vers: Buch (OSIS-Kürzel wie [`crate::Book::id`]), Kapitel und Vers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub struct VerseRef {
    pub book: &'static str,
    pub chapter: u16,
    pub verse: u16,
}

impl VerseRef {
    /// Deutsche Schreibweise, z. B. „Joh 3,16“.
    pub fn label(&self) -> String {
        let abbr = crate::books().iter().find(|b| b.id == self.book).map_or(self.book, |b| b.abbr);
        format!("{abbr} {},{}", self.chapter, self.verse)
    }
}

/// Ergebnis der Abbildung eines Verses.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Mapped {
    /// Gleiche Stelle in beiden Zählungen.
    Same { to: VerseRef },
    /// Andere Stelle nach einer bekannten Regel (`rule`: deutscher Kurztext aus der Regeltabelle).
    Moved { to: VerseRef, rule: &'static str },
    /// Nicht abbildbar; `reason` beginnt mit „Versifikation passt nicht: “.
    Unmapped { reason: String },
}

impl Mapped {
    /// Der Zielvers, wenn es einen gibt.
    pub fn target(&self) -> Option<VerseRef> {
        match self {
            Mapped::Same { to } | Mapped::Moved { to, .. } => Some(*to),
            Mapped::Unmapped { .. } => None,
        }
    }
}

/// Anfang jedes Grundes, warum sich ein Vers nicht abbilden lässt.
pub const MISMATCH: &str = "Versifikation passt nicht: ";

/// Anfang der Regeln für Psalmüberschriften ohne eigene Verszahl in KJV: Eine Stelle „ganzes Kapitel“ lässt solche
/// Verse aus ([`map_range`]), eine Versangabe darauf ist nicht abbildbar.
const HEADING: &str = "Psalmüberschrift";

/// Eine Zeile der Regeltabelle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rule {
    /// Zählungen (Schreibweise von sword-modules).
    pub systems: Vec<&'static str>,
    pub book: &'static str,
    /// Kapitel; `None` = das ganze Buch ist nicht abbildbar.
    pub chapter: Option<u16>,
    pub first: u16,
    pub last: u16,
    /// Ziel in KJV (Kapitel, erster Vers); `None` = keine eigene Entsprechung.
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

    /// KJV-Vers zu einem Vers der Quelle (nur innerhalb der Regel).
    fn forward(&self, verse: u16) -> Option<(u16, u16)> {
        self.target.map(|(c, v)| (c, v + (verse - self.first)))
    }

    /// Vers der Quelle zu einem KJV-Vers, wenn die Regel ihn trifft.
    fn backward(&self, chapter: u16, verse: u16) -> Option<(u16, u16)> {
        let (tc, tv) = self.target?;
        let len = self.last - self.first;
        (tc == chapter && verse >= tv && verse - tv <= len)
            .then(|| (self.chapter.unwrap_or(0), self.first + (verse - tv)))
    }
}

/// Die eingebettete Tabelle.
pub const TABLE: &str = include_str!("../../data/versification-map.tsv");

fn number(field: &str, line: usize) -> Result<u16, String> {
    field.parse().map_err(|_| format!("Zeile {line}: „{field}“ ist keine Zahl"))
}

/// Liest die Regeltabelle (`#` leitet Kommentare ein, Spalten siehe Kopf von `data/versification-map.tsv`).
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

/// Alle Regeln der eingebetteten Tabelle.
pub fn rules() -> &'static [Rule] {
    &RULES
}

/// Ein Schritt zum bzw. vom Drehpunkt KJV.
enum Step {
    To(VerseRef, Option<&'static str>),
    /// Nicht abbildbar; `heading` = eine Psalmüberschrift ohne Verszahl in KJV.
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

/// Ein Buch ohne jede Regel für die Zählung, dessen Verszahlen von KJV abweichen: Grund, warum es nicht abbildbar
/// ist. Gleiche Kapitellängen sagen dann nichts (eine Überschrift als Vers 1 verschiebt auch ein Kapitel mit
/// gleich vielen Versen), also wird kein Kapitel des Buchs geraten.
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

/// Haben beide Zählungen im Buch dieselben Verszahlen in jedem Kapitel und dieselben Regeln? Dann bleibt jeder Vers
/// des Buchs, wo er ist (wie bei ganz gleichen Zählungen).
fn same_book(a: &Versification, b: &Versification, book: &str) -> bool {
    let (Some(x), Some(y)) = (a.chapters_of(book), b.chapters_of(book)) else { return false };
    let ra: Vec<*const Rule> = rules_of(a.name, book).map(|r| r as *const Rule).collect();
    let rb: Vec<*const Rule> = rules_of(b.name, book).map(|r| r as *const Rule).collect();
    x == y && ra == rb
}

/// Vers der Zählung `x` → Vers in KJV.
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

/// Vers in KJV → Vers der Zählung `y`.
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

/// Abbildung mit dem Vermerk, ob ein nicht abbildbarer Vers eine Psalmüberschrift ist.
fn map_verse_marked(from: &Versification, to: &Versification, verse: VerseRef) -> (Mapped, bool) {
    let unmapped = |reason: String, heading: bool| {
        (
            Mapped::Unmapped {
                reason: format!("{MISMATCH}{} ({} → {}): {reason}", verse.label(), from.name, to.name)
            },
            heading,
        )
    };
    // Gleiche Zählung oder gleiche Verszahlen in jedem Kapitel (German/Luther, KJV/KJVA, NRSV/NRSVA; je Buch mit
    // denselben Regeln, z. B. Catholic/Catholic2 außerhalb der Spätschriften): jeder Vers bleibt, wo er ist.
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

/// Einen Vers aus `from` nach `to` abbilden (Vers ab 1).
pub fn map_verse(from: &Versification, to: &Versification, verse: VerseRef) -> Mapped {
    map_verse_marked(from, to, verse).0
}

/// Eine Stelle abgebildet: Verse der Zielzählung in Textreihenfolge, ohne Doppel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MappedRange {
    pub verses: Vec<VerseRef>,
    /// Die Stelle hat in der Ausgangszählung mehr als `limit` Verse.
    pub truncated: bool,
    /// Angewandte Regeln (ohne Doppel, in der Reihenfolge ihres ersten Auftretens).
    pub rules: Vec<&'static str>,
}

/// Stelle in `from` (Vers 0 = ganzes Kapitel) → Verse in `to`, Reihenfolge des Textes, ohne Doppel, höchstens
/// `limit` (truncated = die Stelle hat in `from` mehr als `limit` Verse; genau `limit` Verse ⇒ false).
/// Err(Grund, Deutsch, „Versifikation passt nicht: …“ mit beiden Zählungsnamen) bei unbekanntem Buch/Kapitel in
/// `from` oder einem nicht abbildbaren Vers. Eine Stelle „ganze Kapitel“ (Vers 0 an beiden Enden) lässt
/// Psalmüberschriften aus, die die Zielzählung nicht als Vers zählt (Regel in `rules`).
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
