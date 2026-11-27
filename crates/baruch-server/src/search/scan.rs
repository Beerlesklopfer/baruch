// SPDX-License-Identifier: AGPL-3.0-or-later
//! Text kinds of a section text and hits in it (Round 30a).
//!
//! [`classify`] assigns a [`TextKind`] to every byte:
//! - **heading**: title of a heading line (`= Titel`, for the markers part/„ohne Nummer“ the title in the
//!   content block); the `=`, labels and comments of the line are code;
//! - **footnote**: content of `#footnote[…]`;
//! - **quote**: quotations according to `crate::quotes::quote_spans` (quotation marks, `#quote[…]`, block
//!   quotations) – covers text, heading and footnote;
//! - **code**: Typst code: `#name`, arguments `(…)`, keyword lines (`#let`, `#set`, `#show` …), raw, formulas,
//!   comments, labels `<…>`, references `@…`, escaping backslashes not (they belong to the character);
//! - **text**: everything else, also the content of content blocks such as `#emph[…]`, `#szene[…]`,
//!   `#link(<k5>)[…]`.
//!
//! The search runs in the Typst source text. In the literal mode a Typst special character of the search also
//! finds its escaped form (`#` finds `\#`), and the replacement is escaped for Typst ([`escape_literal`]).

use std::ops::Range;
use std::time::Instant;

use regex::Regex;

use super::TextKind;

/// Characters that have a meaning in the running text of Typst and are escaped in the literal mode.
const SPECIAL: &[char] = &['\\', '#', '*', '_', '$', '@', '<', '>', '[', ']', '`', '~'];

/// Rank of a text kind for hits across several kinds: the strictest counts.
fn rank(kind: TextKind) -> u8 {
    match kind {
        TextKind::Text => 0,
        TextKind::Heading => 1,
        TextKind::Footnote => 2,
        TextKind::Quote => 3,
        TextKind::Code => 4,
    }
}

/// Text kind per byte of `text`.
pub fn classify(text: &str) -> Vec<TextKind> {
    let mut kinds = vec![TextKind::Text; text.len()];
    let mut scanner = Scanner { s: text, kinds: &mut kinds };
    scanner.lines();
    for quote in crate::quotes::quote_spans(text) {
        let (start, end) = (quote.start.min(text.len()), quote.end.min(text.len()));
        for kind in &mut kinds[start..end] {
            if matches!(kind, TextKind::Text | TextKind::Heading | TextKind::Footnote) {
                *kind = TextKind::Quote;
            }
        }
    }
    kinds
}

/// Text kind of a hit `range`: the strictest kind of its bytes.
pub fn kind_of(kinds: &[TextKind], range: &Range<usize>) -> TextKind {
    kinds[range.start..range.end.min(kinds.len())].iter().copied().max_by_key(|k| rank(*k)).unwrap_or(TextKind::Text)
}

struct Scanner<'a> {
    s: &'a str,
    kinds: &'a mut [TextKind],
}

impl Scanner<'_> {
    fn mark(&mut self, range: Range<usize>, kind: TextKind) {
        let end = range.end.min(self.kinds.len());
        for k in &mut self.kinds[range.start.min(end)..end] {
            *k = kind;
        }
    }

    /// Line by line: heading lines and markers, otherwise markup.
    fn lines(&mut self) {
        let s = self.s;
        let mut at = 0;
        // Raw blocks across lines: ``` at the start of a line opens and closes.
        let mut in_raw = false;
        while at < s.len() {
            let end = s[at..].find('\n').map_or(s.len(), |n| at + n + 1);
            let line = &s[at..end];
            let line_start = at == 0 || s.as_bytes()[at - 1] == b'\n';
            let trimmed = line.trim_start();
            if line_start && trimmed.starts_with("```") {
                self.mark(at..end, TextKind::Code);
                if !(trimmed.len() > 3 && trimmed[3..].contains("```")) {
                    in_raw = !in_raw;
                }
            } else if in_raw {
                self.mark(at..end, TextKind::Code);
            } else if let Some(title) = heading_title(line).filter(|_| line_start) {
                self.mark(at..end, TextKind::Code);
                self.markup(at + title.start, at + title.end, TextKind::Heading);
            } else {
                // An expression (`#szene[…]`) can reach beyond the line: then it goes on behind it.
                at = self.markup(at, end, TextKind::Text).max(end);
                continue;
            }
            at = end;
        }
    }

    /// Markup from `start` to `end` with the base kind `base`; returns how far was read (an expression can
    /// reach beyond `end`).
    fn markup(&mut self, start: usize, end: usize, base: TextKind) -> usize {
        let s = self.s;
        let bytes = s.as_bytes();
        let mut i = start;
        while i < end.min(s.len()) {
            match bytes[i] {
                b'\\' => {
                    let next = s[i + 1..].chars().next().map_or(0, char::len_utf8);
                    self.mark(i..i + 1 + next, base);
                    i += 1 + next;
                }
                b'`' => {
                    let run = s[i..].bytes().take_while(|&b| b == b'`').count();
                    let close = s[i + run..].find(&"`".repeat(run)).map_or(s.len(), |c| i + run + c + run);
                    self.mark(i..close, TextKind::Code);
                    i = close;
                }
                b'$' => {
                    let close = s[i + 1..].find('$').map_or(s.len(), |c| i + 1 + c + 1);
                    self.mark(i..close, TextKind::Code);
                    i = close;
                }
                b'/' if bytes.get(i + 1) == Some(&b'/') && !s[..i].ends_with(':') => {
                    let close = s[i..].find('\n').map_or(s.len(), |c| i + c);
                    self.mark(i..close, TextKind::Code);
                    i = close;
                }
                b'/' if bytes.get(i + 1) == Some(&b'*') => {
                    let close = s[i + 2..].find("*/").map_or(s.len(), |c| i + 2 + c + 2);
                    self.mark(i..close, TextKind::Code);
                    i = close;
                }
                b'<' if label_end(s, i).is_some() => {
                    let close = label_end(s, i).expect("eben geprüft");
                    self.mark(i..close, TextKind::Code);
                    i = close;
                }
                b'@' if s[i + 1..].chars().next().is_some_and(|c| c.is_alphanumeric() || c == '_') => {
                    let close = i + 1 + s[i + 1..].find(|c: char| !is_label_char(c)).unwrap_or(s.len() - i - 1);
                    self.mark(i..close, TextKind::Code);
                    i = close;
                }
                b'#' => i = self.hash(i, base),
                _ => {
                    let len = s[i..].chars().next().map_or(1, char::len_utf8);
                    self.mark(i..i + len, base);
                    i += len;
                }
            }
        }
        i
    }

    /// Expression from `#` at `at`: name and arguments are code, content blocks markup (for `#footnote` footnote).
    /// Keywords (`#let`, `#set`, `#show`, `#import` …) turn the rest of the line into code.
    fn hash(&mut self, at: usize, base: TextKind) -> usize {
        let s = self.s;
        let name_end = at
            + 1
            + s[at + 1..]
                .find(|c: char| !(c.is_alphanumeric() || c == '_' || c == '-' || c == '.'))
                .unwrap_or(s.len() - at - 1);
        let name = &s[at + 1..name_end];
        if name.is_empty() {
            self.mark(at..at + 1, base);
            return at + 1;
        }
        if matches!(name, "let" | "set" | "show" | "import" | "include" | "if" | "for" | "while" | "context" | "return")
        {
            // Up to the end of the line or until the brackets close.
            let close = group_end(s, at);
            self.mark(at..close, TextKind::Code);
            return close;
        }
        self.mark(at..name_end, TextKind::Code);
        let mut i = name_end;
        loop {
            match s.as_bytes().get(i) {
                Some(b'(') => {
                    let close = matching(s, i, b'(', b')').unwrap_or(s.len());
                    self.mark(i..close, TextKind::Code);
                    i = close;
                }
                Some(b'[') => {
                    let close = matching(s, i, b'[', b']').unwrap_or(s.len());
                    self.mark(i..i + 1, TextKind::Code);
                    let inner = if name == "footnote" { TextKind::Footnote } else { base };
                    let inner_end = close.saturating_sub(1).max(i + 1);
                    self.markup(i + 1, inner_end, inner);
                    self.mark(inner_end..close, TextKind::Code);
                    i = close;
                }
                _ => return i,
            }
        }
    }
}

/// Character of a label or reference target (as in Typst).
fn is_label_char(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '_' | '-' | ':' | '.')
}

/// End (behind `>`) of a label `<name>` from `at`, else `None`.
fn label_end(s: &str, at: usize) -> Option<usize> {
    let rest = &s[at + 1..];
    let close = rest.find('>')?;
    let name = &rest[..close];
    (!name.is_empty() && name.chars().all(is_label_char)).then_some(at + 1 + close + 1)
}

/// Matching closing bracket for `open` at `at` (strings and escapes skipped); end behind it.
fn matching(s: &str, at: usize, open: u8, close: u8) -> Option<usize> {
    let bytes = s.as_bytes();
    let mut depth = 0usize;
    let mut i = at;
    let mut in_string = false;
    while i < bytes.len() {
        let b = bytes[i];
        if in_string {
            match b {
                b'\\' => i += 1,
                b'"' => in_string = false,
                _ => {}
            }
        } else if b == b'\\' {
            i += 1;
        } else if b == b'"' && open == b'(' {
            in_string = true;
        } else if b == open {
            depth += 1;
        } else if b == close {
            depth -= 1;
            if depth == 0 {
                return Some(i + 1);
            }
        }
        i += 1;
    }
    None
}

/// End of a keyword expression: end of the line outside of brackets (or end of the text).
fn group_end(s: &str, at: usize) -> usize {
    let bytes = s.as_bytes();
    let mut depth = 0i32;
    let mut i = at;
    let mut in_string = false;
    while i < bytes.len() {
        let b = bytes[i];
        if in_string {
            match b {
                b'\\' => i += 1,
                b'"' => in_string = false,
                _ => {}
            }
        } else {
            match b {
                b'"' => in_string = true,
                b'(' | b'[' | b'{' => depth += 1,
                b')' | b']' | b'}' => depth -= 1,
                b'\n' if depth <= 0 => return i,
                _ => {}
            }
        }
        i += 1;
    }
    s.len()
}

/// Title range (bytes in the line) of a heading line: `= Titel` (without label and comment) or the
/// content block of a marker (part, „ohne Nummer“). No heading → `None`.
pub fn heading_title(line: &str) -> Option<Range<usize>> {
    let content = line.trim_end_matches(['\n', '\r']);
    let doc = baruch_typst::parse(content).ok()?;
    let heading = doc.headings.first().filter(|_| doc.template.is_empty())?;
    if heading.raw_title.is_empty() {
        return None;
    }
    let start = if content.starts_with('=') {
        let level = content.bytes().take_while(|&b| b == b'=').count();
        level + content[level..].len() - content[level..].trim_start().len()
    } else {
        content.find(")[")? + 2
    };
    let title_at = content[start..].find(heading.raw_title.as_str())? + start;
    Some(title_at..title_at + heading.raw_title.len())
}

/// Literal text escaped for Typst (replacement in the literal mode): special characters get a backslash, as does
/// a `/` before `/` or `*` (otherwise a comment would begin). Unlike `escape_title` without trimming.
pub fn escape_literal(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len() + 4);
    for (i, &c) in chars.iter().enumerate() {
        let comment = c == '/' && matches!(chars.get(i + 1), Some('/' | '*'));
        if SPECIAL.contains(&c) || comment {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// Options of a search pattern.
#[derive(Debug, Clone, Copy, Default)]
pub struct PatternOptions {
    pub case: bool,
    pub word: bool,
    pub regex: bool,
}

/// Size limits of the crate `regex` (contract §3.1).
const SIZE_LIMIT: usize = 1 << 20;
const DFA_SIZE_LIMIT: usize = 1 << 22;

/// Build the pattern from the search. Literal: every Typst special character also finds its escaped form; whole
/// word: `\b` at the edges that begin or end with a word character. Error (invalid, too large) → message.
pub fn pattern(q: &str, options: PatternOptions) -> Result<Regex, String> {
    let core = if options.regex {
        format!("(?:{q})")
    } else {
        q.chars()
            .map(|c| {
                let escaped = regex::escape(&c.to_string());
                if SPECIAL.contains(&c) { format!("\\\\?{escaped}") } else { escaped }
            })
            .collect::<String>()
    };
    let wordy = |c: Option<char>| c.is_some_and(|c| c.is_alphanumeric() || c == '_');
    let core = if options.word {
        let front = if options.regex || wordy(q.chars().next()) { "\\b" } else { "" };
        let back = if options.regex || wordy(q.chars().last()) { "\\b" } else { "" };
        format!("{front}{core}{back}")
    } else {
        core
    };
    regex::RegexBuilder::new(&core)
        .case_insensitive(!options.case)
        .size_limit(SIZE_LIMIT)
        .dfa_size_limit(DFA_SIZE_LIMIT)
        .build()
        .map_err(|e| match e {
            regex::Error::CompiledTooBig(_) => "Der reguläre Ausdruck ist zu groß.".to_owned(),
            other => format!("Ungültiger regulärer Ausdruck: {other}"),
        })
}

/// A hit in a text: byte range, kind and (in the regex mode) the replacement with the groups inserted.
#[derive(Debug, Clone)]
pub struct Found {
    pub range: Range<usize>,
    pub kind: TextKind,
    pub replacement: Option<String>,
}

/// Why [`find_in`] stopped before the end of the text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stop {
    /// Ran through.
    Done,
    /// Time expired (`deadline`).
    Late,
    /// The inserted replacement texts exceeded the budget (bytes).
    Budget,
}

/// Hits of `re` in `text` within `window` (bytes), only kinds from `kinds`; empty hits do not count.
/// `replacement`: replacement per hit (regex: `$1`/`${name}` inserted; literal: escaped). Aborts after `deadline`
/// ([`Stop::Late`]) and as soon as the inserted replacement texts together need more than `budget` bytes
/// ([`Stop::Budget`]; `budget` is reduced by every replacement) – otherwise the memory would grow with
/// hits × replacement.
pub fn find_in(
    text: &str,
    re: &Regex,
    kinds: &[TextKind],
    window: Range<usize>,
    replacement: Option<(&str, bool)>,
    deadline: Instant,
    budget: &mut usize,
) -> (Vec<Found>, Stop) {
    let classes = classify(text);
    let mut out = Vec::new();
    for caps in re.captures_iter(text) {
        if Instant::now() >= deadline {
            return (out, Stop::Late);
        }
        let whole = caps.get(0).expect("Gruppe 0 gibt es immer");
        let range = whole.start()..whole.end();
        if range.is_empty() || range.start < window.start || range.end > window.end {
            continue;
        }
        let kind = kind_of(&classes, &range);
        if !kinds.contains(&kind) {
            continue;
        }
        let replacement = replacement.map(|(r, regex)| {
            if regex {
                let mut expanded = String::new();
                caps.expand(r, &mut expanded);
                expanded
            } else {
                escape_literal(r)
            }
        });
        // An empty replacement counts as well (one byte per hit), so that the number of hits stays limited.
        let cost = replacement.as_ref().map_or(0, |r| r.len().max(1));
        if cost > *budget {
            return (out, Stop::Budget);
        }
        *budget -= cost;
        out.push(Found { range, kind, replacement });
    }
    (out, Stop::Done)
}

/// Text with all replacements from `found` (ascending, without overlap), except the positions in `skip`.
pub fn apply(text: &str, found: &[Found], skip: &[usize]) -> String {
    let mut out = String::with_capacity(text.len());
    let mut at = 0;
    for (index, f) in found.iter().enumerate() {
        if skip.contains(&index) {
            continue;
        }
        let Some(replacement) = &f.replacement else { continue };
        out.push_str(&text[at..f.range.start]);
        out.push_str(replacement);
        at = f.range.end;
    }
    out.push_str(&text[at..]);
    out
}

/// UTF-16 position of the byte `byte` in `text`.
pub fn utf16_at(text: &str, byte: usize) -> usize {
    text[..byte.min(text.len())].encode_utf16().count()
}

/// Byte of the UTF-16 position `unit` in `text` (rounded to a character boundary, at most the end).
pub fn byte_at_utf16(text: &str, unit: usize) -> usize {
    let mut units = 0;
    for (i, c) in text.char_indices() {
        if units >= unit {
            return i;
        }
        units += c.len_utf16();
    }
    text.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds_of(text: &str, part: &str) -> Vec<TextKind> {
        let at = text.find(part).unwrap();
        let classes = classify(text);
        let mut out: Vec<TextKind> = classes[at..at + part.len()].to_vec();
        out.dedup();
        out
    }

    #[test]
    fn text_kinds_follow_typst_syntax() {
        let text = "= Leuchtturm <k5>\n\nDer Turm#footnote[Eine Notiz.] steht. „Licht“ #emph[hell] `roh` $x$ // Kommentar\n#let a = 3\n";
        assert_eq!(kinds_of(text, "Leuchtturm"), [TextKind::Heading], "Titel der Überschrift");
        assert_eq!(kinds_of(text, "<k5>"), [TextKind::Code], "Label ist Code");
        assert_eq!(kinds_of(text, "Eine Notiz."), [TextKind::Footnote], "Fußnote");
        assert_eq!(kinds_of(text, "#footnote["), [TextKind::Code], "Funktionsname ist Code");
        assert_eq!(kinds_of(text, "Licht"), [TextKind::Quote], "Zitat in Anführungszeichen");
        assert_eq!(kinds_of(text, "hell"), [TextKind::Text], "Inhaltsblock ist Text");
        assert_eq!(kinds_of(text, "roh"), [TextKind::Code], "Raw");
        assert_eq!(kinds_of(text, "Kommentar"), [TextKind::Code], "Kommentar");
        assert_eq!(kinds_of(text, "#let a = 3"), [TextKind::Code], "Codezeile");
        assert_eq!(kinds_of(text, "steht."), [TextKind::Text], "Fließtext");
    }

    #[test]
    fn literal_search_finds_escaped_typst_characters_and_escapes_the_replacement() {
        let re = pattern("C#", PatternOptions::default()).unwrap();
        assert!(re.is_match("Sprache C\\# heute"), "`#` findet auch `\\#`");
        assert_eq!(
            escape_literal("a#b // c*d"),
            "a\\#b \\// c\\*d",
            "Ersatz maskiert: nur das erste / vor / (dann kein Kommentar)"
        );
        let word = pattern("Turm", PatternOptions { word: true, ..Default::default() }).unwrap();
        assert!(!word.is_match("Leuchtturm"), "ganzes Wort");
        assert!(word.is_match("Der Turm."), "ganzes Wort mit Satzzeichen");
    }

    #[test]
    fn utf16_offsets_count_surrogate_pairs() {
        let text = "a😀ä";
        assert_eq!(utf16_at(text, text.find('ä').unwrap()), 3, "Emoji zählt zwei Einheiten");
        assert_eq!(byte_at_utf16(text, 3), text.find('ä').unwrap(), "Rückweg");
    }
}
