// SPDX-License-Identifier: AGPL-3.0-or-later
//! Small readers over Typst source text: where does a raw text, a comment, a formula, a
//! `#` expression, a content block `[…]` end? All positions are byte positions in `&str` on
//! character boundaries. If something is not closed, it reaches up to the end of the text, and the
//! [`Scanner`] remembers that (`open`); the caller turns it into an island. The nesting is
//! limited to [`MAX_DEPTH`], so that a malicious text (`[[[[…`) does not blow the stack.

/// Deepest nesting of brackets and content blocks that is still read.
pub(crate) const MAX_DEPTH: usize = 48;

/// Keywords after which a `#` expression is code up to the end of the line (`#let`, `#set` …).
const KEYWORDS: &[&str] =
    &["let", "set", "show", "import", "include", "if", "for", "while", "context", "return", "break", "continue"];

pub(crate) fn is_keyword(name: &str) -> bool {
    KEYWORDS.contains(&name)
}

pub(crate) fn is_ident_start(c: char) -> bool {
    c.is_alphabetic() || c == '_'
}

pub(crate) fn is_ident_continue(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '-'
}

/// Characters of a label `<…>` (as in Typst).
pub(crate) fn is_label_char(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '_' | '-' | ':' | '.')
}

/// Character at position `i` (if present).
pub(crate) fn char_at(s: &str, i: usize) -> Option<char> {
    s.get(i..).and_then(|rest| rest.chars().next())
}

/// Position after the character at `i`.
pub(crate) fn next_char(s: &str, i: usize) -> usize {
    i + char_at(s, i).map_or(1, char::len_utf8)
}

/// Is there a link start (`http:` or `https:`) before position `i`? Then `//` is not a comment.
pub(crate) fn after_scheme(s: &str, i: usize) -> bool {
    let before = &s[..i];
    before.ends_with("http:") || before.ends_with("https:")
}

/// Identifier from `i`; end (exclusive), `i` if none starts there.
pub(crate) fn ident_end(s: &str, i: usize) -> usize {
    let mut j = i;
    while let Some(c) = char_at(s, j) {
        if (j == i && !is_ident_start(c)) || (j > i && !is_ident_continue(c)) {
            break;
        }
        j += c.len_utf8();
    }
    j
}

/// Escape `\x` from `i` (`s[i] == '\\'`): end. `\u{…}` reaches up to the closing bracket. Before whitespace
/// (also a line end) `\` is a forced break and only the one character (Round 18b): the
/// line end after it still ends the line.
pub(crate) fn skip_escape(s: &str, i: usize) -> usize {
    let j = i + 1;
    if char_at(s, j).is_some_and(char::is_whitespace) {
        return j;
    }
    if s[j..].starts_with("u{")
        && let Some(close) = s[j..].find('}')
    {
        return j + close + 1;
    }
    if j >= s.len() { s.len() } else { next_char(s, j) }
}

/// Line comment from `i`: end (before the line end).
pub(crate) fn skip_line_comment(s: &str, i: usize) -> usize {
    s[i..].find('\n').map_or(s.len(), |n| i + n)
}

/// Reader over a text; `open` becomes true as soon as something is not closed.
pub(crate) struct Scanner<'a> {
    pub s: &'a str,
    pub open: bool,
}

impl<'a> Scanner<'a> {
    pub fn new(s: &'a str) -> Self {
        Scanner { s, open: false }
    }

    fn unclosed(&mut self) -> usize {
        self.open = true;
        self.s.len()
    }

    /// Raw text from `i` (backticks): end. Two backticks are empty raw text, from three on the
    /// same number closes.
    pub fn raw(&mut self, i: usize) -> usize {
        let s = self.s;
        let run = s[i..].bytes().take_while(|&b| b == b'`').count();
        if run == 2 {
            return i + 2;
        }
        let fence = "`".repeat(run);
        match s[i + run..].find(&fence) {
            Some(close) => i + run + close + run,
            None => self.unclosed(),
        }
    }

    /// Block comment `/* … */` from `i` (nested): end.
    pub fn block_comment(&mut self, i: usize) -> usize {
        let bytes = self.s.as_bytes();
        let mut depth = 0usize;
        let mut j = i;
        while j < bytes.len() {
            if bytes[j..].starts_with(b"/*") {
                depth += 1;
                j += 2;
            } else if bytes[j..].starts_with(b"*/") {
                depth -= 1;
                j += 2;
                if depth == 0 {
                    return j;
                }
            } else {
                j += 1;
            }
        }
        self.unclosed()
    }

    /// Formula `$…$` from `i`: end.
    pub fn math(&mut self, i: usize) -> usize {
        let s = self.s;
        let mut j = i + 1;
        while j < s.len() {
            match s.as_bytes()[j] {
                b'\\' => j = skip_escape(s, j),
                b'$' => return j + 1,
                _ => j = next_char(s, j),
            }
        }
        self.unclosed()
    }

    /// String `"…"` from `i`: end.
    pub fn string(&mut self, i: usize) -> usize {
        let s = self.s;
        let mut j = i + 1;
        while j < s.len() {
            match s.as_bytes()[j] {
                b'\\' => j = skip_escape(s, j),
                b'"' => return j + 1,
                _ => j = next_char(s, j),
            }
        }
        self.unclosed()
    }

    /// Code in brackets `(…)` or `{…}` from `i`: end after the closing bracket. A foreign
    /// closing bracket ends the reading before it (open).
    pub fn code_group(&mut self, i: usize, depth: usize) -> usize {
        if depth > MAX_DEPTH {
            return self.unclosed();
        }
        let s = self.s;
        let close = if s.as_bytes()[i] == b'(' { b')' } else { b'}' };
        let mut j = i + 1;
        while j < s.len() {
            let b = s.as_bytes()[j];
            match b {
                b'"' => j = self.string(j),
                b'`' => j = self.raw(j),
                b'/' if s[j..].starts_with("//") => j = skip_line_comment(s, j),
                b'/' if s[j..].starts_with("/*") => j = self.block_comment(j),
                b'(' | b'{' => j = self.code_group(j, depth + 1),
                b'[' => j = self.content(j, depth + 1),
                _ if b == close => return j + 1,
                b')' | b'}' | b']' => {
                    self.open = true;
                    return j;
                }
                _ => j = next_char(s, j),
            }
        }
        self.unclosed()
    }

    /// Content block `[…]` from `i` (markup, nested): end after the closing bracket.
    pub fn content(&mut self, i: usize, depth: usize) -> usize {
        if depth > MAX_DEPTH {
            return self.unclosed();
        }
        let s = self.s;
        let mut j = i + 1;
        while j < s.len() {
            match s.as_bytes()[j] {
                b'\\' => j = skip_escape(s, j),
                b'`' => j = self.raw(j),
                b'$' => j = self.math(j),
                b'/' if s[j..].starts_with("//") && !after_scheme(s, j) => j = skip_line_comment(s, j),
                b'/' if s[j..].starts_with("/*") => j = self.block_comment(j),
                b'#' => j = self.hash(j, depth + 1),
                b'[' => j = self.content(j, depth + 1),
                b']' => return j + 1,
                _ => j = next_char(s, j),
            }
        }
        self.unclosed()
    }

    /// Code after a keyword (`#let …`, `#show …: it => […]`): up to the line end outside
    /// of brackets, up to `;` or before a foreign closing bracket.
    fn keyword_expr(&mut self, i: usize, depth: usize) -> usize {
        let s = self.s;
        let mut j = i;
        while j < s.len() {
            match s.as_bytes()[j] {
                b'\n' | b';' => return j,
                b'"' => j = self.string(j),
                b'`' => j = self.raw(j),
                b'/' if s[j..].starts_with("//") => return j,
                b'/' if s[j..].starts_with("/*") => j = self.block_comment(j),
                b'(' | b'{' => j = self.code_group(j, depth + 1),
                b'[' => j = self.content(j, depth + 1),
                b')' | b'}' | b']' => return j,
                _ => j = next_char(s, j),
            }
        }
        s.len()
    }

    /// `#` expression from `i` (`s[i] == '#'`): end. A name with attached calls `(…)`,
    /// content blocks `[…]` and fields `.name`; after a keyword code up to the end of the line;
    /// `#(…)`, `#{…}`, `#"…"`. If none of that follows the `#`, it is only the one character.
    pub fn hash(&mut self, i: usize, depth: usize) -> usize {
        if depth > MAX_DEPTH {
            return self.unclosed();
        }
        let s = self.s;
        let start = i + 1;
        let mut j = match char_at(s, start) {
            Some(c) if is_ident_start(c) => {
                let end = ident_end(s, start);
                if is_keyword(&s[start..end]) {
                    return self.keyword_expr(end, depth);
                }
                end
            }
            Some('(') => self.code_group(start, depth + 1),
            Some('{') => return self.code_group(start, depth + 1),
            Some('"') => self.string(start),
            _ => return start,
        };
        loop {
            match char_at(s, j) {
                Some('(') => j = self.code_group(j, depth + 1),
                Some('[') => j = self.content(j, depth + 1),
                Some('.') if char_at(s, j + 1).is_some_and(is_ident_start) => j = ident_end(s, j + 1),
                _ => return j,
            }
        }
    }
}

/// End of the closed content block from `i` in `s`, `None` if it does not close.
pub(crate) fn closed_content(s: &str, i: usize) -> Option<usize> {
    let mut scanner = Scanner::new(s);
    let end = scanner.content(i, 0);
    (!scanner.open).then_some(end)
}

/// End of the closed bracket `(…)` from `i` in `s`, `None` if it does not close.
pub(crate) fn closed_group(s: &str, i: usize) -> Option<usize> {
    let mut scanner = Scanner::new(s);
    let end = scanner.code_group(i, 0);
    (!scanner.open).then_some(end)
}

/// Positions of the line ends that lie in the markup of the top level (not in raw text, comments,
/// formulas or `#` expressions): the "logical lines" as ranges, each with its line end.
pub(crate) fn logical_lines(s: &str) -> Vec<(usize, usize)> {
    let mut scanner = Scanner::new(s);
    let mut lines = Vec::new();
    let mut start = 0;
    let mut j = 0;
    while j < s.len() {
        match s.as_bytes()[j] {
            b'\\' => j = skip_escape(s, j),
            b'`' => j = scanner.raw(j),
            b'$' => j = scanner.math(j),
            b'/' if s[j..].starts_with("//") && !after_scheme(s, j) => j = skip_line_comment(s, j),
            b'/' if s[j..].starts_with("/*") => j = scanner.block_comment(j),
            b'#' => j = scanner.hash(j, 0),
            b'\n' => {
                lines.push((start, j + 1));
                j += 1;
                start = j;
            }
            _ => j = next_char(s, j),
        }
    }
    if start < s.len() {
        lines.push((start, s.len()));
    }
    lines
}

/// Start of a comment in a heading line, exactly as `baruch_typst::heading_line` looks for
/// it: escapes and inline raw do not count, nor does `//` after `http:`/`https:`.
pub(crate) fn heading_comment_start(text: &str) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => i += 2,
            b'`' => {
                let run = text[i..].bytes().take_while(|&b| b == b'`').count();
                let close = text[i + run..].find(&"`".repeat(run))?;
                i += run + close + run;
            }
            b'/' if bytes.get(i + 1) == Some(&b'*') => return Some(i),
            b'/' if bytes.get(i + 1) == Some(&b'/') && !after_scheme(text, i) => return Some(i),
            _ => i += 1,
        }
    }
    None
}

/// Image path of an island: the string literal in the first argument of the first `image(` call, also
/// nested (`#figure(image("/bilder/haus.svg"), caption: […])`), outside of comments,
/// raw text and formulas. A computed path (`image("a" + p)`) or another first argument
/// yields `None`.
pub fn image_path(raw: &str) -> Option<String> {
    let mut found = Vec::new();
    find_image_markup(raw, 0, raw.len(), 0, &mut found, true);
    found.into_iter().next()?
}

/// Round 18b: image paths of **all** `image(` calls of an island in text order (like [`image_path`] per
/// call; calls without a string literal are missing). A block with several images thus shows all of them.
pub fn image_paths(raw: &str) -> Vec<String> {
    let mut found = Vec::new();
    find_image_markup(raw, 0, raw.len(), 0, &mut found, false);
    found.into_iter().flatten().collect()
}

/// Value of a string literal `"…"` (whole text), escapes resolved; otherwise `None`.
pub(crate) fn string_value(literal: &str) -> Option<String> {
    let mut scanner = Scanner::new(literal);
    if !literal.starts_with('"') || scanner.string(0) != literal.len() || scanner.open {
        return None;
    }
    unescape(&literal[1..literal.len() - 1])
}

/// Resolve the content of a string literal without quotation marks (`\n`, `\t`, `\r`, `\u{…}`, `\x` → `x`).
fn unescape(inner: &str) -> Option<String> {
    let mut value = String::new();
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            value.push(c);
            continue;
        }
        match chars.next()? {
            'n' => value.push('\n'),
            't' => value.push('\t'),
            'r' => value.push('\r'),
            'u' => {
                let rest: String = chars.by_ref().take_while(|&c| c != '}').collect();
                let hex = rest.strip_prefix('{')?;
                value.push(u32::from_str_radix(hex, 16).ok().and_then(char::from_u32)?);
            }
            other => value.push(other),
        }
    }
    Some(value)
}

/// Search in the markup `s[from..to]` for `#…` expressions and collect every `image(` call in `found`
/// (`None`: without string literal); with `first` the search ends at the first. `true` if it shall end.
fn find_image_markup(
    s: &str,
    from: usize,
    to: usize,
    depth: usize,
    found: &mut Vec<Option<String>>,
    first: bool,
) -> bool {
    if depth > MAX_DEPTH {
        return false;
    }
    let s = &s[..to];
    let mut scanner = Scanner::new(s);
    let mut j = from;
    while j < s.len() {
        match s.as_bytes()[j] {
            b'\\' => j = skip_escape(s, j),
            b'`' => j = scanner.raw(j),
            b'$' => j = scanner.math(j),
            b'/' if s[j..].starts_with("//") && !after_scheme(s, j) => j = skip_line_comment(s, j),
            b'/' if s[j..].starts_with("/*") => j = scanner.block_comment(j),
            b'#' => {
                let end = scanner.hash(j, 0);
                if find_image_code(s, j + 1, end, depth + 1, found, first) {
                    return true;
                }
                j = end.max(j + 1);
            }
            _ => j = next_char(s, j),
        }
    }
    false
}

/// Search in the code `s[from..to]` (arguments, brackets) for `image(`; content blocks `[…]` are
/// markup again. Like [`find_image_markup`].
fn find_image_code(
    s: &str,
    from: usize,
    to: usize,
    depth: usize,
    found: &mut Vec<Option<String>>,
    first: bool,
) -> bool {
    if depth > MAX_DEPTH {
        return false;
    }
    let s = &s[..to];
    let mut scanner = Scanner::new(s);
    let mut j = from;
    while j < s.len() {
        let Some(c) = char_at(s, j) else { return false };
        match c {
            '"' => j = scanner.string(j),
            '`' => j = scanner.raw(j),
            '/' if s[j..].starts_with("//") => j = skip_line_comment(s, j),
            '/' if s[j..].starts_with("/*") => j = scanner.block_comment(j),
            '[' => {
                let end = scanner.content(j, 0);
                // Without the closing bracket, if the block is closed.
                let inner_end = if s[..end].ends_with(']') && end > j + 1 { end - 1 } else { end };
                if find_image_markup(s, j + 1, inner_end, depth + 1, found, first) {
                    return true;
                }
                j = end.max(j + 1);
            }
            _ if is_ident_start(c) => {
                let end = ident_end(s, j);
                let method = s[..j].ends_with('.');
                if &s[j..end] == "image" && !method && s[end..].starts_with('(') {
                    found.push(string_argument(s, end + 1));
                    if first {
                        return true;
                    }
                }
                j = end;
            }
            _ => j += c.len_utf8(),
        }
    }
    false
}

/// String literal as the whole first argument from `i` (after the opening bracket), resolved.
fn string_argument(s: &str, i: usize) -> Option<String> {
    let start = i + (s[i..].len() - s[i..].trim_start().len());
    if !s[start..].starts_with('"') {
        return None;
    }
    let mut scanner = Scanner::new(s);
    let end = scanner.string(start);
    if scanner.open || !matches!(s[end..].trim_start().chars().next(), Some(',' | ')')) {
        return None;
    }
    unescape(&s[start + 1..end - 1])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(s: &str) -> (&str, bool) {
        let mut scanner = Scanner::new(s);
        let end = scanner.hash(0, 0);
        (&s[..end], scanner.open)
    }

    #[test]
    fn a_hash_expression_ends_after_its_arguments_and_fields() {
        assert_eq!(
            hash("#link(\"a]b\")[Text] danach"),
            ("#link(\"a]b\")[Text]", false),
            "String mit ] und Inhaltsblock gehören dazu"
        );
        assert_eq!(hash("#sym.arrow.r x"), ("#sym.arrow.r", false), "Felder gehören dazu");
        assert_eq!(hash("#footnote[x]. Satz"), ("#footnote[x]", false), "ein Punkt vor Leerzeichen ist Text");
        assert_eq!(
            hash("#let x = [a\n\nb]\nText"),
            ("#let x = [a\n\nb]", false),
            "Schlüsselwort: Code bis zum Zeilenende außerhalb der Klammern"
        );
        assert!(hash("#f[offen").1, "nicht geschlossener Inhaltsblock ist offen");
    }

    #[test]
    fn deep_nesting_stops_at_the_limit_instead_of_overflowing() {
        let s = "[".repeat(100_000);
        assert_eq!(closed_content(&s, 0), None, "nicht geschlossen und zu tief: offen, ohne Absturz");
        let s = format!("x{}", "#f[".repeat(50_000));
        assert_eq!(logical_lines(&s).len(), 1, "eine logische Zeile, ohne Absturz");
    }

    #[test]
    fn the_image_path_is_the_literal_of_the_first_image_call() {
        assert_eq!(image_path("#image(\"/bilder/a.png\")").as_deref(), Some("/bilder/a.png"), "Markup");
        assert_eq!(
            image_path("#figure(image(\"/bilder/b.svg\", width: 50%), caption: [Ein #emph[Bild]])").as_deref(),
            Some("/bilder/b.svg"),
            "in einer Abbildung"
        );
        assert_eq!(image_path("// #image(\"/bilder/c.png\")"), None, "im Kommentar nicht");
        assert_eq!(image_path("/* #image(\"/bilder/c.png\") */"), None, "im Blockkommentar nicht");
        assert_eq!(image_path("`#image(\"/x.png\")`"), None, "im Raw-Text nicht");
        assert_eq!(image_path("#image(\"a\" + p)"), None, "berechneter Pfad nicht");
        assert_eq!(image_path("#x.image(\"/m.png\")"), None, "eine Methode heißt nur so");
        assert_eq!(image_path("#images(\"/n.png\")"), None, "anderer Name");
        assert_eq!(
            image_path("#box[Text #image(\"/i\\u{e4}.png\")]").as_deref(),
            Some("/iä.png"),
            "im Inhaltsblock, Maskierung aufgelöst"
        );
        assert_eq!(
            image_path("#figure(image(\"/erst.png\"))\n#image(\"/zweit.png\")").as_deref(),
            Some("/erst.png"),
            "der erste Aufruf zählt"
        );
    }

    #[test]
    fn logical_lines_skip_newlines_inside_constructs() {
        let s = "a #szene[\nb\n\nc\n]\nd\n";
        let lines: Vec<&str> = logical_lines(s).into_iter().map(|(a, b)| &s[a..b]).collect();
        assert_eq!(lines, ["a #szene[\nb\n\nc\n]\n", "d\n"], "Zeilenenden im Inhaltsblock zählen nicht");
    }
}
