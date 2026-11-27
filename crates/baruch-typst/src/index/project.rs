// SPDX-License-Identifier: AGPL-3.0-or-later
//! Projection of a section text (Round 31b): automatic scripture reference marks and markup of the first mention of
//! a glossary term per chapter. Only export, map, mirror, typesetting, PDF and EPUB – the texts in SQLite stay.
//!
//! **Scripture references:** if the index `bibel` is in `index_pages.registers`, every valid reference that
//! [`baruch_scripture::scan`] finds in the running text (not in code, comment, raw, heading line or mark) gets a
//! mark directly behind it: `register: "bibel"`, `term` = book („Johannes“), `sub` = chapter and verses („3,16“),
//! `sort` = [`baruch_scripture::Passage::sort_key`] (biblical order).
//!
//! **First mention** (only with `glossary_first_mention == Some(true)` and glossary): per chapter (a heading of
//! level 1, a part or „ohne Nummer“ level 1 resets) the first occurrence of a term or of a variant as whole words
//! (case does not matter). Form, standard Typst only:
//! - with glossary page: `#link(<baruch-gl-UUID>)[#emph[Wort]]` – a link to the entry of the glossary page (the
//!   label stands in the back matter; without glossary page there would be no reference to a missing label);
//! - without glossary page: `/*baruch-erste-erwaehnung*/#emph[Wort]` – the comment before it is invisible and makes
//!   the markup recognisable again when importing an export (Round 21b, [`super::strip_first_mentions`]); emphasis
//!   written by hand stays untouched this way.
//!
//! If the word is followed by a character that Typst would attach to the expression (`(`, `[`, `.` before a letter,
//! `;`), a `;` stands in between (ends the embedded expression and disappears in the typesetting).

use std::borrow::Cow;
use std::collections::{HashMap, HashSet};

use super::scan::{Scan, scan};
use super::{
    BackMatter, FIRST_MENTION_MARK, GLOSSARY_ANCHOR_PREFIX, IndexMark, SCRIPTURE_REGISTER, mark_source, term_key,
};

/// Pattern of a glossary term: words (comparison form) and the separators between them.
struct Pattern {
    entry: usize,
    words: Vec<String>,
    seps: Vec<String>,
}

/// Prepared projection (private, belongs to [`super::Projector`]).
pub(super) struct Plan {
    pub active: bool,
    scripture: bool,
    /// Patterns by the first word.
    patterns: HashMap<String, Vec<Pattern>>,
    /// Label per entry if it is linked (glossary page on and valid label).
    anchors: Vec<Option<String>>,
    /// Entries that are already marked up in the running chapter.
    seen: HashSet<usize>,
}

/// May a UUID be part of a Typst label?
pub(super) fn label_safe(uuid: &str) -> bool {
    !uuid.is_empty() && uuid.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
}

/// Is the index of scripture references switched on (index pages with `bibel`)?
pub(super) fn scripture_enabled(back: &BackMatter) -> bool {
    back.settings.index_pages.as_ref().is_some_and(|p| p.registers.iter().any(|r| r == SCRIPTURE_REGISTER))
}

/// Splits a term into words (letters and digits) and the separators between them (whitespace to one space).
/// Characters other than whitespace, `-`, `'`, `’` and `.` between the words or at the edge → `None` (the term is
/// not searched).
pub(super) fn split_term(term: &str) -> Option<(Vec<String>, Vec<String>)> {
    // Runs of word characters and non-word characters, alternating.
    let mut runs: Vec<(bool, String)> = Vec::new();
    for c in term.trim().chars() {
        let word = c.is_alphanumeric();
        match runs.last_mut() {
            Some((w, run)) if *w == word => run.push(c),
            _ => runs.push((word, c.to_string())),
        }
    }
    if !runs.first().is_some_and(|r| r.0) || !runs.last().is_some_and(|r| r.0) {
        return None;
    }
    let mut words = Vec::new();
    let mut seps = Vec::new();
    for (word, run) in runs {
        if word {
            words.push(term_key(&run));
        } else {
            seps.push(normalize_sep(&run)?);
        }
    }
    Some((words, seps))
}

/// Separator in comparison form: whitespace to one space; only whitespace, `-`, `'`, `’`, `.` allowed, no empty line.
fn normalize_sep(sep: &str) -> Option<String> {
    if sep.matches('\n').count() > 1 {
        return None;
    }
    let mut out = String::new();
    let mut space = false;
    for c in sep.chars() {
        if c.is_whitespace() {
            space = true;
            continue;
        }
        if !matches!(c, '-' | '\'' | '’' | '.') {
            return None;
        }
        if space {
            out.push(' ');
            space = false;
        }
        out.push(c);
    }
    if space {
        out.push(' ');
    }
    (!out.is_empty()).then_some(out)
}

impl Plan {
    pub fn new(back: &BackMatter) -> Plan {
        let active = back.is_active();
        let scripture = active && scripture_enabled(back);
        let mention = active && back.settings.glossary_first_mention == Some(true);
        let link = back.settings.glossary_page.is_some();
        let mut patterns: HashMap<String, Vec<Pattern>> = HashMap::new();
        let mut anchors = Vec::new();
        for (entry, gloss) in back.glossary.iter().enumerate() {
            anchors.push((link && label_safe(&gloss.uuid)).then(|| format!("{GLOSSARY_ANCHOR_PREFIX}{}", gloss.uuid)));
            if !mention {
                continue;
            }
            for form in std::iter::once(&gloss.term).chain(&gloss.variants) {
                if let Some((words, seps)) = split_term(form) {
                    patterns.entry(words[0].clone()).or_default().push(Pattern { entry, words, seps });
                }
            }
        }
        // Longer patterns first („Neuer Bund“ before „Neuer“).
        for list in patterns.values_mut() {
            list.sort_by_key(|p| std::cmp::Reverse(p.words.len()));
        }
        Plan { active, scripture, patterns, anchors, seen: HashSet::new() }
    }

    pub fn project<'b>(&mut self, body: &'b str) -> Cow<'b, str> {
        if !self.active || (!self.scripture && self.patterns.is_empty()) {
            return Cow::Borrowed(body);
        }
        let scanned = scan(body);
        // (start, end, replacement); insertions have start == end.
        let mut edits: Vec<(usize, usize, String)> = Vec::new();
        if !self.patterns.is_empty() {
            self.mentions(body, &scanned, &mut edits);
        } else {
            // Count the change of chapter nevertheless (plays no role without patterns).
            self.seen.clear();
        }
        if self.scripture {
            for (at, mark) in scripture_marks(body, &scanned) {
                let inside = edits.iter().any(|(a, b, _)| *a < at && at < *b);
                if !inside {
                    edits.push((at, at, mark_source(&mark)));
                }
            }
        }
        if edits.is_empty() {
            return Cow::Borrowed(body);
        }
        edits.sort_by_key(|(a, b, _)| (*a, *b));
        let mut out = String::with_capacity(body.len() + edits.iter().map(|e| e.2.len()).sum::<usize>());
        let mut at = 0;
        for (a, b, text) in edits {
            if a < at {
                continue;
            }
            out.push_str(&body[at..a]);
            out.push_str(&text);
            at = b;
        }
        out.push_str(&body[at..]);
        Cow::Owned(out)
    }

    /// First mentions in `body` → replacements.
    fn mentions(&mut self, body: &str, scanned: &Scan, edits: &mut Vec<(usize, usize, String)>) {
        let words = scanned.words(body);
        let keys: Vec<String> = words.iter().map(|&(a, b)| term_key(&body[a..b])).collect();
        let mut headings = scanned.headings.iter().filter(|h| h.level <= 1).map(|h| h.start).peekable();
        let mut t = 0;
        while t < words.len() {
            let (start, _) = words[t];
            while headings.next_if(|&h| h <= start).is_some() {
                self.seen.clear();
            }
            let Some(found) = self.match_at(body, scanned, &words, &keys, t) else {
                t += 1;
                continue;
            };
            let (entry, n) = found;
            let end = words[t + n - 1].1;
            if self.seen.insert(entry) {
                let slice = &body[start..end];
                let wrapped = match &self.anchors[entry] {
                    Some(label) => format!("#link(<{label}>)[#emph[{slice}]]"),
                    None => format!("{FIRST_MENTION_MARK}#emph[{slice}]"),
                };
                let guard = needs_guard(&body[end..]);
                edits.push((start, end, if guard { format!("{wrapped};") } else { wrapped }));
            }
            t += n;
        }
        for _ in headings {
            self.seen.clear();
        }
    }

    /// Longest pattern from word `t` on: (entry, number of words).
    fn match_at(
        &self,
        body: &str,
        scanned: &Scan,
        words: &[(usize, usize)],
        keys: &[String],
        t: usize,
    ) -> Option<(usize, usize)> {
        let list = self.patterns.get(&keys[t])?;
        'pattern: for pattern in list {
            let n = pattern.words.len();
            if t + n > words.len() {
                continue;
            }
            for k in 1..n {
                if keys[t + k] != pattern.words[k] {
                    continue 'pattern;
                }
                let (a, b) = (words[t + k - 1].1, words[t + k].0);
                if !scanned.prose_range(a, b) || normalize_sep(&body[a..b]).as_deref() != Some(&pattern.seps[k - 1]) {
                    continue 'pattern;
                }
            }
            return Some((pattern.entry, n));
        }
        None
    }
}

/// Does Typst attach a directly following character to an embedded expression (`#emph[…]`)?
fn needs_guard(after: &str) -> bool {
    let mut chars = after.chars();
    match chars.next() {
        Some('(' | '[' | ';') => true,
        Some('.') => chars.next().is_some_and(|c| c.is_alphabetic() || c == '_'),
        _ => false,
    }
}

/// Automatic marks of the index of scripture references: (insertion point, mark) for every valid reference in the
/// running text.
pub(super) fn scripture_marks(body: &str, scanned: &Scan) -> Vec<(usize, IndexMark)> {
    let masked = scanned.masked(body);
    let mut out = Vec::new();
    for found in baruch_scripture::scan(&masked) {
        if found.status != baruch_scripture::RefStatus::Valid {
            continue;
        }
        let Some(passage) = found.passage else { continue };
        let (a, b) = found.span;
        if !scanned.prose_range(a, b) || !scanned.insertable(body, b) {
            continue;
        }
        let mark = IndexMark {
            register: SCRIPTURE_REGISTER.to_owned(),
            term: passage.book.name.to_owned(),
            sub: passage.chapter_verse(),
            sort: passage.sort_key(),
            ..IndexMark::default()
        };
        out.push((b, mark));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terms_split_into_words_and_separators() {
        assert_eq!(
            split_term("Neuer  Bund"),
            Some((vec!["neuer".into(), "bund".into()], vec![" ".into()])),
            "Leerraum zu einem Leerzeichen, Wörter klein"
        );
        assert_eq!(
            split_term("Jesus-Christus"),
            Some((vec!["jesus".into(), "christus".into()], vec!["-".into()])),
            "Bindestrich als Trennzeichen"
        );
        assert_eq!(split_term("C++"), None, "Satzzeichen am Rand: nicht gesucht");
        assert_eq!(split_term("a/b"), None, "Schrägstrich ist kein erlaubtes Trennzeichen");
    }

    #[test]
    fn a_guard_follows_only_where_typst_would_continue_the_expression() {
        assert!(needs_guard("(s) weiter"), "Klammer würde Argumente öffnen");
        assert!(needs_guard("[x]"), "eckige Klammer wäre ein weiterer Inhaltsblock");
        assert!(needs_guard(".Name"), "Punkt vor Buchstaben wäre ein Feldzugriff");
        assert!(needs_guard("; und"), "ein Semikolon würde sonst verschluckt");
        assert!(!needs_guard(". Satz"), "Satzende bleibt");
        assert!(!needs_guard(" weiter"), "Leerzeichen bleibt");
    }
}
