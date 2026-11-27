// SPDX-License-Identifier: AGPL-3.0-or-later
//! Splitting a Typst file into work template and outline (contract B1.5, section 2).
//!
//! - **Work template** is everything before the first heading (preamble with `#set`, `#show`, `#let` …).
//! - **Heading** is a line that starts in column 0 with 1–3 `=` followed by a space or tab. `====` and deeper stays
//!   text. A comment behind it (`== Titel // Notiz`) does not belong to the title.
//! - **Not counted as outline** are lines in raw blocks (```` ``` ```` at the start of a line opens and closes), in
//!   multi-line inline raw (`` `…` `` across lines), in block comments `/* … */`, in multi-line strings in code
//!   (`#let x = "…"`) and in multi-line content blocks from code (`#block[…]`, `#f(x)[…]`, `#let x = [ … ]`, `#show
//!   …: it => [ … ]`). For this the scanner tracks brackets (`(`, `[`, `{`) from a `#` expression on; a `[` in the
//!   running text of the top level stays text.
//! - **Parts and „ohne Nummer“** (Round 14, [`marker`]): the canonical lines `#heading(level: 1, numbering:
//!   none)[T] <baruch-part>` (part, [`Heading::level`] 0) and `#heading(level: N, numbering: none)[T]
//!   <baruch-unnumbered>` (level N, [`Heading::numbered`] `false`) count like headings; one of them before the
//!   first `=` ends the work template.
//! - **Known limit:** indented headings and every other `#heading(...)` are not recognised; they stay text (before
//!   the first heading therefore part of the work template). An `else [` at the start of a continuation line (`]
//!   else [`) counts as running text.
//! - The **text** of a heading reaches from its line to before the next heading, with all line ends. Therefore
//!   `join(&d.template, bodies) == source` always holds, byte for byte.
//! - Title and author come from the first `#set document(title: …, author: …)` outside of comments (`//`, `/* …
//!   */`) and raw text.
//! - **Escapes** (contract Round 4, section 2): [`Heading::title`] is the resolved title (`\'` → `'`, `\#` → `#` …,
//!   [`unescape_title`]), [`Heading::raw_title`] the literal one from the line. New heading lines are built by
//!   [`heading_line`], which escapes with [`escape_title`].
//! - **Numbering:** [`heading_numbering`] reads the last `#set heading(numbering: …)` rule of the top level of the
//!   work template (like Typst) and returns it in the pattern language of [`scheme`] (`I|1`, `I.;A.;1.`);
//!   [`format_number`] applies a pattern by the rules of Typst (`"I.1"` with `[2, 3]` → `II.3`). Outline schemes
//!   and the export of their rule: [`scheme`].
//! - A **UTF-8 BOM** (U+FEFF) at the start of the file belongs to the work template; a heading directly after it
//!   counts nevertheless (Windows editors often write it). This way the round trip stays byte-identical.
//!
//! Line ends are not rewritten; CRLF → LF is done by the caller (the store) beforehand. The crate knows neither
//! database nor file system.

pub mod cover;
pub mod cover_layout;
pub mod index;
pub mod markdown;
pub mod marker;
pub mod scheme;
pub mod timing;
pub mod typeset;
pub mod xref;

pub use markdown::{markdown_heading_line, markdown_headings};

pub use marker::{
    PART_LABEL, UNNUMBERED_LABEL, part_line, part_page_rule, relevel_line, set_numbered_line, unnumbered_line,
};

/// Result of [`parse`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Document {
    /// Everything before the first heading (empty if the file starts with a heading).
    pub template: String,
    /// `title` from `#set document(...)`: string literal or raw text of a content block `[…]`.
    pub title: Option<String>,
    /// `author` from `#set document(...)`: string literal or tuple of strings (joined with ", ").
    pub author: Option<String>,
    pub headings: Vec<Heading>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Heading {
    /// 0 = part (marker `<baruch-part>`, [`marker`]), 1 (`=`) to 3 (`===`).
    pub level: u8,
    /// Title with resolved Typst escapes ([`unescape_title`] of `raw_title`).
    pub title: String,
    /// Text after the `=`, trimmed, without an attached `<label>` and without comment, literal.
    pub raw_title: String,
    /// From the heading line to before the next heading, including all line ends.
    pub body: String,
    /// `false` only for the marker „ohne Nummer“ (`<baruch-unnumbered>`, [`marker`]); `=` lines and parts are `true`.
    pub numbered: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("keine Überschrift (= …) gefunden")]
    NoHeadings,
}

/// Splits `source` into work template and headings (rules see module description).
pub fn parse(source: &str) -> Result<Document, ParseError> {
    let mut scanner = Scanner::default();
    let mut starts: Vec<(usize, u8, String, bool)> = Vec::new();
    let bom = if source.starts_with(BOM) { BOM.len_utf8() } else { 0 };
    let mut offset = bom;
    for line in source[bom..].split_inclusive('\n') {
        if scanner.in_markup() {
            if let Some((level, title)) = read_heading(line) {
                starts.push((offset, level, title, true));
            } else if let Some(found) = marker::read_marker(line) {
                starts.push((offset, found.level, found.raw_title.to_owned(), found.level == 0));
            }
        }
        scanner.feed(line);
        offset += line.len();
    }
    let Some(&(first, ..)) = starts.first() else {
        return Err(ParseError::NoHeadings);
    };
    let mut headings = Vec::with_capacity(starts.len());
    for (i, (start, level, title, numbered)) in starts.iter().enumerate() {
        let end = starts.get(i + 1).map_or(source.len(), |next| next.0);
        headings.push(Heading {
            level: *level,
            title: unescape_title(title),
            raw_title: title.clone(),
            body: source[*start..end].to_owned(),
            numbered: *numbered,
        });
    }
    let (title, author) = document_meta(source);
    Ok(Document { template: source[..first].to_owned(), title, author, headings })
}

/// Byte order mark that some editors write at the start of the file.
const BOM: char = '\u{feff}';

/// Work template and texts one after the other; inverse of [`parse`].
pub fn join<'a>(template: &str, bodies: impl IntoIterator<Item = &'a str>) -> String {
    let mut out = template.to_owned();
    for body in bodies {
        out.push_str(body);
    }
    out
}

/// Like [`join`], plus per text its byte range in the result (Round 8, Typst preview: trace messages of a Typst run
/// back to the section). The result is byte-identical with [`join`].
pub fn join_with_offsets<'a>(
    template: &str,
    bodies: impl IntoIterator<Item = &'a str>,
) -> (String, Vec<std::ops::Range<usize>>) {
    let mut out = template.to_owned();
    let mut ranges = Vec::new();
    for body in bodies {
        let start = out.len();
        out.push_str(body);
        ranges.push(start..out.len());
    }
    (out, ranges)
}

/// Line (1-based) of the byte position `byte` in `text`, counted like Typst (`Lines` in typst-syntax): every
/// [`is_newline`] character ends a line, `\r\n` counts once. Positions behind the end count like the end.
pub fn line_at(text: &str, byte: usize) -> usize {
    let mut line = 1;
    let mut chars = text.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        if i >= byte {
            break;
        }
        if is_newline(c) {
            if let Some(&(j, '\n')) = chars.peek().filter(|_| c == '\r') {
                if j >= byte {
                    break; // between \r and \n: still the same line
                }
                chars.next();
            }
            line += 1;
        }
    }
    line
}

/// Level and literal title if `line` is a heading (regardless of raw/comment before it). A comment (`//`, `/*`)
/// behind the heading does not belong to the title.
fn read_heading(line: &str) -> Option<(u8, String)> {
    let level = line.bytes().take_while(|&b| b == b'=').count();
    if !(1..=3).contains(&level) {
        return None;
    }
    let rest = &line[level..];
    if !rest.starts_with([' ', '\t']) {
        return None;
    }
    let rest = &rest[..comment_start(rest).unwrap_or(rest.len())];
    Some((level as u8, strip_label(rest.trim()).to_owned()))
}

/// Does the start of a link (`http:` or `https:`) stand before position `i` of `text`? Then `//` is not a comment.
fn after_scheme(text: &str, i: usize) -> bool {
    let before = &text[..i];
    before.ends_with("http:") || before.ends_with("https:")
}

/// Start of a comment (`//` or `/*`) in a markup line. Escapes (`\/`), inline raw (`` `//` ``) and links
/// (`https://…`) do not count.
fn comment_start(text: &str) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => i += 2,
            b'`' => {
                let run = backticks(&text[i..]);
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

/// Removes an attached `<label>` at the end of the title.
fn strip_label(title: &str) -> &str {
    match label_start(title) {
        Some(open) => title[..open].trim_end(),
        None => title,
    }
}

/// Position of the attached `<label>` at the end of `title` (characters as in Typst), if present.
fn label_start(title: &str) -> Option<usize> {
    let inner = title.strip_suffix('>')?;
    let open = inner.rfind('<')?;
    let label = &inner[open + 1..];
    let valid = !label.is_empty() && label.chars().all(|c| c.is_alphanumeric() || matches!(c, '_' | '-' | ':' | '.'));
    // An escaped `\<` opens no label.
    (valid && !inner[..open].ends_with('\\')).then_some(open)
}

/// Characters whose escape `\X` [`unescape_title`] resolves (the same list as migration 0006 of baruch-store; `\\`
/// becomes `\`).
const UNESCAPED: &[char] = &['\'', '"', '#', '*', '_', '$', '@', '<', '>', '[', ']', '`', '~', '=', '-', '/', '+', '.'];

/// Characters that [`escape_title`] escapes: markup characters of Typst and the backslash itself.
const ESCAPED: &[char] = &['\\', '#', '*', '_', '$', '@', '<', '>', '[', ']', '`', '~'];

/// Resolve Typst escapes of a heading title: `\\` → `\`, `\X` → `X` for the characters
/// `' " # * _ $ @ < > [ ] ` ~ = - / + .`; every other backslash stays (`\n`, `\u{…}`).
/// Example: `Anna\'s Laube` → `Anna's Laube`.
pub fn unescape_title(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\'
            && let Some(&next) = chars.peek()
            && (next == '\\' || UNESCAPED.contains(&next))
        {
            out.push(next);
            chars.next();
            continue;
        }
        out.push(c);
    }
    out
}

/// Line end in the sense of Typst (`is_newline` in typst-syntax): besides `\n` and `\r` also `\x0B`, `\x0C`,
/// U+0085, U+2028 and U+2029. Behind it a new line starts, at whose start `= `, `- `, `+ `, `1. ` and `/ ` would be
/// structure.
pub fn is_newline(c: char) -> bool {
    matches!(c, '\n' | '\r' | '\x0B' | '\x0C' | '\u{85}' | '\u{2028}' | '\u{2029}')
}

/// Escape a title for a new heading line (inverse of [`unescape_title`]): backslash and
/// `# * _ $ @ < > [ ] ` ~` get a backslash, likewise a `/` before `/` or `*` (otherwise a comment would
/// start). Line ends ([`is_newline`]) become spaces, the edges are trimmed.
pub fn escape_title(title: &str) -> String {
    let flat: String = title.chars().map(|c| if is_newline(c) { ' ' } else { c }).collect();
    let chars: Vec<char> = flat.trim().chars().collect();
    let mut out = String::with_capacity(flat.len() + 8);
    for (i, &c) in chars.iter().enumerate() {
        let comment = c == '/' && matches!(chars.get(i + 1), Some('/' | '*'));
        if ESCAPED.contains(&c) || comment {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// Heading line `"=" × level + " " + escape_title(title)` with line end. If the previous line (`previous`) had a
/// label `<…>` or a comment `// …`, both are kept.
///
/// Round 14: `level` 0 yields the part marker ([`part_line`]); if `previous` is a marker „ohne Nummer“ and `level`
/// 1–3, it stays one ([`unnumbered_line`]). In both cases a comment of the previous line is kept.
pub fn heading_line(level: u8, title: &str, previous: Option<&str>) -> String {
    if let Some(line) = marker::marker_heading_line(level, title, previous) {
        return line;
    }
    let mut line = format!("{} {}", "=".repeat(usize::from(level.max(1))), escape_title(title));
    if let Some(previous) = previous {
        let previous = previous.trim_end_matches(['\n', '\r']);
        let equals = previous.bytes().take_while(|&b| b == b'=').count();
        let rest = &previous[equals..];
        if equals > 0 && rest.starts_with([' ', '\t']) {
            let cut = comment_start(rest).unwrap_or(rest.len());
            let content = rest[..cut].trim();
            if let Some(open) = label_start(content) {
                line.push(' ');
                line.push_str(&content[open..]);
            }
            let comment = rest[cut..].trim_end();
            if !comment.is_empty() {
                line.push(' ');
                line.push_str(comment);
            }
        }
    }
    line.push('\n');
    line
}

/// Numbering of the work template in the pattern language of [`scheme`] (contract Round 6, section 2b).
///
/// Only `#set heading(numbering: …)` at the **top level** counts (not in `#show …` rules, content blocks or
/// function bodies), outside of comments and raw text, and of those the **last** rule with `numbering` (as in
/// Typst). A string → `Some(pattern)`, `none` → `Some("")` (no numbers). A function (`(..n) => …`, directly or
/// through the name of a `#let` function) is evaluated per level if it only contains depth conditions
/// (`n.pos().len() == k`, `<= k`, `< k`, also through `let p = n.pos()`), text and `numbering(…)`; an empty else
/// branch (`""`, `none`, `[]` or no `else`) means "without number": `len() == 1` with `numbering("I", ..n)` →
/// `I|1`, `<= k` → `P|k`, `< k` → `P|k-1`, `== k` (k > 1) → list only with position k (`;1.1`). Otherwise, as
/// before, the first `numbering("PATTERN", …)` of the function counts for all levels. Without a rule or with a
/// value that cannot be evaluated `None`.
pub fn heading_numbering(template: &str) -> Option<String> {
    const NEEDLE: &str = "#set heading(";
    let blanked = blank_comments(template);
    let tops = top_level_hashes(template);
    let mut found = None;
    for &rule in &tops {
        if !blanked[rule..].starts_with(NEEDLE) {
            continue;
        }
        let start = rule + NEEDLE.len();
        let mut cursor = Cursor { s: &blanked[start..], i: 0 };
        for (name, value, at) in cursor.arguments_at() {
            if name != "numbering" {
                continue;
            }
            found = Some(match value {
                Arg::Str(pattern) => Some(pattern),
                Arg::Other(text) if text.trim() == "none" => Some(String::new()),
                _ => {
                    let rest = &blanked[start + at..];
                    let expression = rest[..expression_end(rest)].trim();
                    function_numbering(&blanked, &tops, rule, expression)
                }
            });
        }
    }
    found.flatten()
}

/// Numbering from a function in the rule or from the name of a `#let` function that is defined at the top level
/// before the rule (`rule`) (the last such definition applies).
fn function_numbering(blanked: &str, tops: &[usize], rule: usize, expression: &str) -> Option<String> {
    if let Some(spec) = scheme::read_function(expression) {
        return Some(spec);
    }
    let is_name = !expression.is_empty()
        && expression.bytes().all(ident_char)
        && expression.bytes().next().is_some_and(ident_start);
    if is_name {
        let needle = format!("#let {expression}");
        let definition = tops.iter().enumerate().rev().find(|&(_, &at)| {
            at < rule
                && blanked[at..].starts_with(&needle)
                && blanked[at + needle.len()..].trim_start_matches([' ', '\t']).starts_with(['(', '='])
        });
        if let Some((index, &at)) = definition {
            let end = tops.get(index + 1).copied().unwrap_or(blanked.len());
            let body = blanked[at + needle.len()..end].trim_start();
            let spec = match body.strip_prefix('=') {
                Some(value) => scheme::read_function(value.trim()),
                None => scheme::read_let_function(body),
            };
            if spec.is_some() {
                return spec;
            }
        }
    }
    numbering_call(expression).or_else(|| let_numbering(blanked, expression))
}

/// Positions of all `#` that stand at the top level in the markup (not in comments, raw text, strings, content
/// blocks, arguments or code blocks), in order.
fn top_level_hashes(source: &str) -> Vec<usize> {
    let mut scanner = Scanner { record: true, ..Scanner::default() };
    let mut offset = 0;
    for line in source.split_inclusive('\n') {
        scanner.base = offset;
        scanner.feed(line);
        offset += line.len();
    }
    scanner.tops
}

/// Length of an expression up to the comma or the closing parenthesis at level 0 (strings skipped).
fn expression_end(s: &str) -> usize {
    let bytes = s.as_bytes();
    let mut depth = 0usize;
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' if depth == 0 => return i,
            b')' | b']' | b'}' => depth -= 1,
            b',' if depth == 0 => return i,
            b'"' => {
                // A string that does not close in the line counts as an ordinary character.
                i += 1 + closing_quote(&bytes[i + 1..]).map_or(0, |len| len + 1);
                continue;
            }
            b'\\' => i += 1,
            _ => {}
        }
        i += 1;
    }
    bytes.len()
}

/// First `numbering("PATTERN"` in the text (e.g. the body of a function), otherwise `None`.
fn numbering_call(text: &str) -> Option<String> {
    const CALL: &str = "numbering(";
    let mut from = 0;
    while let Some(found) = text[from..].find(CALL) {
        let at = from + found;
        // Only the call itself, not `heading-numbering(` or `my_numbering(`.
        let whole = text[..at].bytes().next_back().is_none_or(|b| !ident_char(b));
        let mut cursor = Cursor { s: &text[at + CALL.len()..], i: 0 };
        while cursor.peek().is_some_and(char::is_whitespace) {
            cursor.bump();
        }
        if whole && cursor.peek() == Some('"') {
            cursor.bump();
            return Some(cursor.string());
        }
        from = at + CALL.len();
    }
    None
}

/// `numbering("PATTERN"` in the body of `#let name(…) = …` or `#let name = …`; the body reaches to the next line
/// that starts with `#` (next rule or definition), or to the end.
fn let_numbering(blanked: &str, name: &str) -> Option<String> {
    if name.is_empty() || !name.bytes().all(ident_char) || !name.bytes().next().is_some_and(ident_start) {
        return None;
    }
    let needle = format!("#let {name}");
    let mut from = 0;
    while let Some(found) = blanked[from..].find(&needle) {
        let after = from + found + needle.len();
        let next = blanked[after..].trim_start_matches([' ', '\t']);
        if next.starts_with('(') || next.starts_with('=') {
            let body = &blanked[after..];
            let end = body.find("\n#").map_or(body.len(), |n| n + 1);
            return numbering_call(&body[..end]);
        }
        from = after;
    }
    None
}

/// Counting symbol of a numbering pattern (as in Typst).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Counter {
    Arabic,
    LowerLatin,
    UpperLatin,
    LowerRoman,
    UpperRoman,
    Symbol,
}

impl Counter {
    fn of(c: char) -> Option<Counter> {
        Some(match c {
            '1' => Counter::Arabic,
            'a' => Counter::LowerLatin,
            'A' => Counter::UpperLatin,
            'i' => Counter::LowerRoman,
            'I' => Counter::UpperRoman,
            '*' => Counter::Symbol,
            _ => return None,
        })
    }

    fn apply(self, n: usize) -> String {
        match self {
            Counter::Arabic => n.to_string(),
            Counter::LowerLatin => latin(n),
            Counter::UpperLatin => latin(n).to_uppercase(),
            Counter::LowerRoman => roman(n).to_lowercase(),
            Counter::UpperRoman => roman(n),
            Counter::Symbol => {
                const SYMBOLS: [&str; 6] = ["*", "†", "‡", "§", "¶", "‖"];
                if n == 0 {
                    return "-".to_owned();
                }
                SYMBOLS[(n - 1) % SYMBOLS.len()].repeat((n - 1) / SYMBOLS.len() + 1)
            }
        }
    }
}

/// a, b, …, z, aa, ab … (bijective to base 26); 0 → "-".
fn latin(mut n: usize) -> String {
    if n == 0 {
        return "-".to_owned();
    }
    let mut letters = Vec::new();
    while n > 0 {
        n -= 1;
        letters.push(char::from(b'a' + (n % 26) as u8));
        n /= 26;
    }
    letters.iter().rev().collect()
}

/// Roman numeral in upper case; 0 → "N" (like Typst).
fn roman(mut n: usize) -> String {
    const TABLE: [(usize, &str); 13] = [
        (1000, "M"),
        (900, "CM"),
        (500, "D"),
        (400, "CD"),
        (100, "C"),
        (90, "XC"),
        (50, "L"),
        (40, "XL"),
        (10, "X"),
        (9, "IX"),
        (5, "V"),
        (4, "IV"),
        (1, "I"),
    ];
    if n == 0 {
        return "N".to_owned();
    }
    let mut out = String::new();
    for (value, letters) in TABLE {
        while n >= value {
            out.push_str(letters);
            n -= value;
        }
    }
    out
}

/// Counting symbols with their prefix and the rest of the pattern (suffix); `None` without a counting symbol.
type Pieces = (Vec<(String, Counter)>, String);

fn pattern_pieces(pattern: &str) -> Option<Pieces> {
    let mut pieces = Vec::new();
    let mut text = String::new();
    for c in pattern.chars() {
        match Counter::of(c) {
            Some(counter) => pieces.push((std::mem::take(&mut text), counter)),
            None => text.push(c),
        }
    }
    (!pieces.is_empty()).then_some((pieces, text))
}

/// Does `pattern` have at least one counting symbol (`1 a A i I *`)? Otherwise it is not a numbering pattern.
pub fn is_numbering_pattern(pattern: &str) -> bool {
    pattern_pieces(pattern).is_some()
}

/// Number by a Typst pattern: per number in `counts` the next counting symbol with its prefix; if there are more
/// numbers than counting symbols, the last one repeats (with its prefix, with an empty prefix with the suffix as
/// separator); at the end stands the suffix. `format_number("I.1", &[2, 3])` = `II.3`, `format_number("1.", &[2])`
/// = `2.`, `format_number("1.1", &[1, 2, 3])` = `1.2.3`. A pattern without a counting symbol or empty `counts` →
/// empty.
pub fn format_number(pattern: &str, counts: &[usize]) -> String {
    let Some((pieces, suffix)) = pattern_pieces(pattern) else { return String::new() };
    if counts.is_empty() {
        return String::new();
    }
    let mut out = String::new();
    let mut numbers = counts.iter();
    for ((prefix, counter), &n) in pieces.iter().zip(&mut numbers) {
        out.push_str(prefix);
        out.push_str(&counter.apply(n));
    }
    if let Some((last_prefix, last_counter)) = pieces.last() {
        for &n in numbers {
            out.push_str(if last_prefix.is_empty() { &suffix } else { last_prefix });
            out.push_str(&last_counter.apply(n));
        }
    }
    out.push_str(&suffix);
    out
}

/// Open bracket that can reach across lines.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Open {
    /// Content block `[…]` from code (`#block[`, `#f(x)[`, `[` in arguments): inside is markup, but no outline.
    /// `call`: directly after it another `[…]` may follow (`#grid[a][b]`).
    Content { call: bool },
    /// Square bracket in the markup of a content block (Typst counts it, so that `]` closes correctly).
    Bracket,
    /// Argument list or parenthesised expression in code; `call` as above (`#f(x)[…]`).
    Paren { call: bool },
    /// Code block `{…}`.
    Brace,
}

/// State across lines. A line can only be a heading if it starts in the markup of the top level, that is, none of
/// this is open.
#[derive(Default)]
struct Scanner {
    /// Raw block with ```` ``` ```` at the start of a line: length of the fence, closed at the start of a line.
    raw_fence: Option<usize>,
    /// Inline raw that reaches beyond the line end: length of the run of backticks.
    raw_inline: Option<usize>,
    /// Depth of the block comments (Typst nests them).
    comment_depth: u32,
    /// String in code that reaches beyond the line end.
    in_string: bool,
    /// Open brackets, innermost last.
    stack: Vec<Open>,
    /// Remember positions of the `#` at the top level ([`top_level_hashes`]); `base` = start of the line.
    record: bool,
    base: usize,
    tops: Vec<usize>,
}

/// Length of the run of backticks at the start of `s`.
fn backticks(s: &str) -> usize {
    s.bytes().take_while(|&b| b == b'`').count()
}

/// Keywords after which the rest of the line is code (`#let x = "…"`, `#show …: it => [`).
const LINE_CODE: &[&str] = &["let", "set", "show", "import", "include", "if", "else", "for", "while", "return"];

fn ident_start(b: u8) -> bool {
    b.is_ascii_alphabetic() || b == b'_' || b >= 0x80
}

fn ident_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'-' || b >= 0x80
}

impl Scanner {
    /// Does the start of the next line stand in the markup of the top level?
    fn in_markup(&self) -> bool {
        self.raw_fence.is_none()
            && self.raw_inline.is_none()
            && self.comment_depth == 0
            && !self.in_string
            && self.stack.is_empty()
    }

    /// Is there code (not markup) at the current position?
    fn in_code(&self, line_code: bool) -> bool {
        match self.stack.last() {
            Some(Open::Paren { .. } | Open::Brace) => true,
            Some(Open::Content { .. } | Open::Bracket) => false,
            None => line_code,
        }
    }

    fn feed(&mut self, line: &str) {
        if let Some(fence) = self.raw_fence {
            if backticks(line.trim_start()) == fence {
                self.raw_fence = None;
            }
            return;
        }
        // Bytewise: all characters that matter are ASCII; UTF-8 continuation bytes match none of them.
        let bytes = line.as_bytes();
        let indent = line.len() - line.trim_start().len();
        // Rest of the line is code after `#let`, `#set`, `#show` … (ends with the line).
        let mut line_code = false;
        let mut i = 0;
        while i < bytes.len() {
            let rest = &bytes[i..];
            if self.comment_depth > 0 {
                if rest.starts_with(b"/*") {
                    self.comment_depth += 1;
                    i += 2;
                } else if rest.starts_with(b"*/") {
                    self.comment_depth -= 1;
                    i += 2;
                } else {
                    i += 1;
                }
                continue;
            }
            if let Some(run) = self.raw_inline {
                let found = backticks(&line[i..]);
                if found == run {
                    self.raw_inline = None;
                }
                i += found.max(1);
                continue;
            }
            if self.in_string {
                match bytes[i] {
                    b'\\' => i += 2,
                    b'"' => {
                        self.in_string = false;
                        i += 1;
                    }
                    _ => i += 1,
                }
                continue;
            }
            let code = self.in_code(line_code);
            match bytes[i] {
                b'`' => {
                    let run = backticks(&line[i..]);
                    let close = rest[run..].windows(run).position(|w| w.iter().all(|&b| b == b'`'));
                    match close {
                        // Inline raw or raw block in one line: skip.
                        Some(close) => i += run + close + run,
                        // Open fence at the start of a line: raw block up to the closing line.
                        None if run >= 3 && i == indent => {
                            self.raw_fence = Some(run);
                            return;
                        }
                        // Otherwise inline raw beyond the line end.
                        None => {
                            self.raw_inline = Some(run);
                            i += run;
                        }
                    }
                }
                b'\\' => i += 2,
                b'/' if rest.starts_with(b"//") && !after_scheme(line, i) => break,
                b'/' if rest.starts_with(b"/*") => {
                    self.comment_depth = 1;
                    i += 2;
                }
                b'"' if code => {
                    self.in_string = true;
                    i += 1;
                }
                b'"' => {
                    // In markup only a string closed in the same line counts as a string; a single quotation mark
                    // in the text stays text. This way "src/*.typ" opens no comment.
                    match closing_quote(&rest[1..]) {
                        Some(len) => i += 1 + len + 1,
                        None => i += 1,
                    }
                }
                b'#' if !code => {
                    if self.record && self.stack.is_empty() {
                        self.tops.push(self.base + i);
                    }
                    i += 1;
                    if self.hash_expression(line, &mut i) {
                        line_code = true;
                    }
                }
                b'[' if code => {
                    self.stack.push(Open::Content { call: false });
                    i += 1;
                }
                // `[` in the markup of the top level is text; in a content block it is counted.
                b'[' if !self.stack.is_empty() => {
                    self.stack.push(Open::Bracket);
                    i += 1;
                }
                b']' if !code => {
                    i += 1;
                    if let Some(Open::Content { call: true }) = self.stack.pop() {
                        self.trailing_content(bytes, &mut i);
                    }
                }
                b'(' if code => {
                    self.stack.push(Open::Paren { call: false });
                    i += 1;
                }
                b'{' if code => {
                    self.stack.push(Open::Brace);
                    i += 1;
                }
                b')' | b'}' if code => {
                    i += 1;
                    if let Some(Open::Paren { call: true }) = self.stack.pop() {
                        self.trailing_content(bytes, &mut i);
                    }
                }
                _ => i += 1,
            }
        }
    }

    /// After `#` in markup (`i` stands behind it): read the expression and remember open brackets. `true` if a
    /// keyword turns the rest of the line into code.
    fn hash_expression(&mut self, line: &str, i: &mut usize) -> bool {
        let bytes = line.as_bytes();
        match bytes.get(*i) {
            Some(b'(') => {
                self.stack.push(Open::Paren { call: true });
                *i += 1;
                return false;
            }
            Some(b'{') => {
                self.stack.push(Open::Brace);
                *i += 1;
                return false;
            }
            Some(b'[') => {
                self.stack.push(Open::Content { call: true });
                *i += 1;
                return false;
            }
            Some(&b) if ident_start(b) => {}
            _ => return false,
        }
        let start = *i;
        while bytes.get(*i).is_some_and(|&b| ident_char(b)) {
            *i += 1;
        }
        if LINE_CODE.contains(&&line[start..*i]) {
            return true;
        }
        loop {
            match bytes.get(*i) {
                Some(b'.') if bytes.get(*i + 1).is_some_and(|&b| ident_start(b)) => {
                    *i += 1;
                    while bytes.get(*i).is_some_and(|&b| ident_char(b)) {
                        *i += 1;
                    }
                }
                Some(b'(') => {
                    self.stack.push(Open::Paren { call: true });
                    *i += 1;
                    return false;
                }
                Some(b'[') => {
                    self.stack.push(Open::Content { call: true });
                    *i += 1;
                    return false;
                }
                _ => return false,
            }
        }
    }

    /// After `)` or `]` of a call: a directly following `[` opens another content block.
    fn trailing_content(&mut self, bytes: &[u8], i: &mut usize) {
        if bytes.get(*i) == Some(&b'[') {
            self.stack.push(Open::Content { call: true });
            *i += 1;
        }
    }
}

/// Length of the string content up to the closing `"` (with `\` escapes), if present in `bytes`.
fn closing_quote(bytes: &[u8]) -> Option<usize> {
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => i += 2,
            b'"' => return Some(i),
            b'\n' => return None,
            _ => i += 1,
        }
    }
    None
}

/// Value of an argument in `#set document(...)`.
enum Arg {
    Str(String),
    Content(String),
    Tuple(Vec<Arg>),
    /// Other expression, raw (e.g. `none`, a variable, a call).
    Other(String),
}

/// `source` with spaces instead of comments (`//` to the line end, `/* … */` nested) and raw
/// text (`` `…` ``, ```` ```…``` ````); line ends and all positions stay. Strings that close in the same line stay
/// untouched (`"src/*.typ"`, `"https://…"`).
fn blank_comments(source: &str) -> String {
    let bytes = source.as_bytes();
    let mut out = bytes.to_vec();
    let mut blank = |from: usize, to: usize| {
        for b in &mut out[from..to] {
            if *b != b'\n' {
                *b = b' ';
            }
        }
    };
    let mut i = 0;
    while i < bytes.len() {
        let rest = &bytes[i..];
        match bytes[i] {
            b'\\' => i += 2,
            b'"' => match closing_quote(&rest[1..]) {
                Some(len) => i += 1 + len + 1,
                None => i += 1,
            },
            b'`' => {
                let run = backticks(&source[i..]);
                let end = rest[run..]
                    .windows(run)
                    .position(|w| w.iter().all(|&b| b == b'`'))
                    .map_or(bytes.len(), |close| i + run + close + run);
                blank(i, end);
                i = end;
            }
            b'/' if rest.starts_with(b"//") && !after_scheme(source, i) => {
                let end = rest.iter().position(|&b| b == b'\n').map_or(bytes.len(), |n| i + n);
                blank(i, end);
                i = end;
            }
            b'/' if rest.starts_with(b"/*") => {
                let mut depth = 0u32;
                let mut j = i;
                while j < bytes.len() {
                    if bytes[j..].starts_with(b"/*") {
                        depth += 1;
                        j += 2;
                    } else if bytes[j..].starts_with(b"*/") {
                        depth -= 1;
                        j += 2;
                        if depth == 0 {
                            break;
                        }
                    } else {
                        j += 1;
                    }
                }
                let end = j.min(bytes.len());
                blank(i, end);
                i = end;
            }
            _ => i += 1,
        }
    }
    // Only whole ranges between ASCII characters were replaced, so the result is UTF-8.
    String::from_utf8(out).unwrap_or_else(|_| source.to_owned())
}

/// `title` and `author` from the first `#set document(...)` outside of comments and raw text.
fn document_meta(source: &str) -> (Option<String>, Option<String>) {
    const NEEDLE: &str = "#set document(";
    let source = blank_comments(source);
    let source = source.as_str();
    if let Some(found) = source.find(NEEDLE) {
        let from = found + NEEDLE.len();
        let mut cursor = Cursor { s: &source[from..], i: 0 };
        let args = cursor.arguments();
        let mut title = None;
        let mut author = None;
        for (name, value) in args {
            match (name.as_str(), value) {
                ("title", Arg::Str(s) | Arg::Content(s)) => title = Some(s),
                ("author", Arg::Str(s)) => author = Some(s),
                ("author", Arg::Tuple(items)) => {
                    let names: Vec<String> =
                        items.into_iter().filter_map(|a| if let Arg::Str(s) = a { Some(s) } else { None }).collect();
                    author = Some(names.join(", "));
                }
                _ => {}
            }
        }
        let title = title.map(|t| t.trim().to_owned()).filter(|t| !t.is_empty());
        let author = author.map(|a| a.trim().to_owned()).filter(|a| !a.is_empty());
        return (title, author);
    }
    (None, None)
}

/// Small reader for the argument list of `#set document(` (after the opening parenthesis).
struct Cursor<'a> {
    s: &'a str,
    i: usize,
}

impl Cursor<'_> {
    fn peek(&self) -> Option<char> {
        self.s[self.i..].chars().next()
    }

    fn bump(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.i += c.len_utf8();
        Some(c)
    }

    fn skip_space(&mut self) {
        while self.peek().is_some_and(|c| c.is_whitespace() || c == ',') {
            self.bump();
        }
    }

    /// `name: value, …` up to the closing `)`.
    fn arguments(&mut self) -> Vec<(String, Arg)> {
        self.arguments_at().into_iter().map(|(name, value, _)| (name, value)).collect()
    }

    /// Like [`Cursor::arguments`], plus per value the position of its start in `s`.
    fn arguments_at(&mut self) -> Vec<(String, Arg, usize)> {
        let mut args = Vec::new();
        loop {
            self.skip_space();
            match self.peek() {
                None | Some(')') => return args,
                _ => {}
            }
            let start = self.i;
            while self.peek().is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '-') {
                self.bump();
            }
            let name = self.s[start..self.i].to_owned();
            while self.peek().is_some_and(char::is_whitespace) {
                self.bump();
            }
            if name.is_empty() || self.peek() != Some(':') {
                // Positional argument or something unreadable: skip to the next comma.
                self.value();
                if self.i == start {
                    self.bump();
                }
                continue;
            }
            self.bump();
            while self.peek().is_some_and(char::is_whitespace) {
                self.bump();
            }
            let at = self.i;
            let value = self.value();
            args.push((name, value, at));
        }
    }

    fn value(&mut self) -> Arg {
        match self.peek() {
            Some('"') => {
                self.bump();
                Arg::Str(self.string())
            }
            Some('[') => {
                self.bump();
                Arg::Content(self.content())
            }
            Some('(') => {
                self.bump();
                let mut items = Vec::new();
                loop {
                    self.skip_space();
                    match self.peek() {
                        None => break,
                        Some(')') => {
                            self.bump();
                            break;
                        }
                        _ => {
                            let before = self.i;
                            items.push(self.value());
                            if self.i == before {
                                self.bump();
                            }
                        }
                    }
                }
                Arg::Tuple(items)
            }
            _ => {
                // Expression (variable, call …): up to the comma or the parenthesis at level 0.
                let start = self.i;
                let mut depth = 0usize;
                while let Some(c) = self.peek() {
                    match c {
                        '(' | '[' | '{' => depth += 1,
                        ')' | ']' | '}' if depth == 0 => break,
                        ')' | ']' | '}' => depth -= 1,
                        ',' if depth == 0 => break,
                        '"' => {
                            self.bump();
                            self.string();
                            continue;
                        }
                        _ => {}
                    }
                    self.bump();
                }
                Arg::Other(self.s[start..self.i].to_owned())
            }
        }
    }

    /// Rest of a string literal after the opening `"`, escapes resolved.
    fn string(&mut self) -> String {
        let mut out = String::new();
        while let Some(c) = self.bump() {
            match c {
                '"' => break,
                '\\' => match self.bump() {
                    Some('n') => out.push('\n'),
                    Some('r') => out.push('\r'),
                    Some('t') => out.push('\t'),
                    Some('u') if self.peek() == Some('{') => {
                        self.bump();
                        let start = self.i;
                        while self.peek().is_some_and(|c| c != '}') {
                            self.bump();
                        }
                        let hex = &self.s[start..self.i];
                        self.bump();
                        if let Some(c) = u32::from_str_radix(hex, 16).ok().and_then(char::from_u32) {
                            out.push(c);
                        }
                    }
                    Some(other) => out.push(other),
                    None => break,
                },
                c => out.push(c),
            }
        }
        out
    }

    /// Raw text of a content block after the opening `[` up to the matching `]`.
    fn content(&mut self) -> String {
        let start = self.i;
        let mut depth = 0usize;
        while let Some(c) = self.peek() {
            match c {
                '\\' => {
                    self.bump();
                }
                '[' => depth += 1,
                ']' if depth == 0 => break,
                ']' => depth -= 1,
                _ => {}
            }
            self.bump();
        }
        let text = self.s[start..self.i].to_owned();
        self.bump();
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heading_lines_need_one_to_three_equals_and_a_space() {
        assert_eq!(read_heading("= Titel\n"), Some((1, "Titel".into())), "einfache Überschrift");
        assert_eq!(read_heading("===\tDrei <sec:drei>\n"), Some((3, "Drei".into())), "Tab erlaubt, Label fällt weg");
        assert_eq!(read_heading("==== Vier\n"), None, "vierte Ebene bleibt Text");
        assert_eq!(read_heading("=Ohne\n"), None, "ohne Leerzeichen keine Überschrift");
        assert_eq!(read_heading(" = Eingerückt\n"), None, "eingerückt ist keine Überschrift");
        assert_eq!(read_heading("= a < b\n"), Some((1, "a < b".into())), "kein Label, bleibt im Titel");
    }

    #[test]
    fn a_comment_behind_a_heading_is_not_part_of_the_title() {
        assert_eq!(read_heading("== Titel // Kommentar\n"), Some((2, "Titel".into())), "// beendet den Titel");
        assert_eq!(
            read_heading("= Titel <t> // noch\n"),
            Some((1, "Titel".into())),
            "Label vor dem Kommentar fällt weg"
        );
        assert_eq!(read_heading("= A /* B */\n"), Some((1, "A".into())), "Blockkommentar gehört nicht zum Titel");
        assert_eq!(
            read_heading("= Siehe https://example.org/x\n"),
            Some((1, "Siehe https://example.org/x".into())),
            "ein Link ist kein Kommentar"
        );
        assert_eq!(read_heading("= Der `//`-Strich\n"), Some((1, "Der `//`-Strich".into())), "// in Raw bleibt");
        assert_eq!(read_heading("= a \\// b\n"), Some((1, "a \\// b".into())), "maskiertes / beginnt keinen Kommentar");
    }

    #[test]
    fn document_meta_reads_strings_tuples_and_content_blocks() {
        let (t, a) = document_meta("#set document(title: [Die *Probe*], author: (\"A. Muster\", \"B. Beispiel\"))\n");
        assert_eq!(t.as_deref(), Some("Die *Probe*"), "Titel aus dem Inhaltsblock, roh");
        assert_eq!(a.as_deref(), Some("A. Muster, B. Beispiel"), "Autoren aus dem Tupel");
        let (t, a) = document_meta(
            "// #set document(title: \"alt\")\n#set document(\n  date: auto,\n  title: \"Neu \\\"x\\\"\",\n)\n",
        );
        assert_eq!(t.as_deref(), Some("Neu \"x\""), "auskommentierte Zeile zählt nicht");
        assert_eq!(a, None, "ohne author kein Autor");
    }

    #[test]
    fn document_meta_skips_block_comments_and_trailing_line_comments() {
        let (t, _) = document_meta("/*\n#set document(title: \"alt\")\n*/\n#set document(title: \"neu\")\n");
        assert_eq!(t.as_deref(), Some("neu"), "#set document in /* … */ zählt nicht");
        let (t, _) =
            document_meta("/* a /* b */\n#set document(title: \"innen\")\n*/\n#set document(title: \"außen\")\n");
        assert_eq!(t.as_deref(), Some("außen"), "Blockkommentare schachteln wie in Typst");
        let (t, _) = document_meta("#set text(lang: \"de\") // #set document(title: \"hinten\")\n= A\n");
        assert_eq!(t, None, "ein Zeilenkommentar hinter Code zählt nicht");
        let (t, _) = document_meta("#let p = \"a/*b\"\n#set document(title: \"echt\")\n");
        assert_eq!(t.as_deref(), Some("echt"), "/* in einem String öffnet keinen Kommentar");
        let (t, a) = document_meta("#set document(\n  title: \"T\", // Arbeitstitel\n  author: \"A\",\n)\n");
        assert_eq!((t.as_deref(), a.as_deref()), (Some("T"), Some("A")), "Kommentar in der Argumentliste stört nicht");
    }

    #[test]
    fn blank_comments_keeps_positions_and_line_ends() {
        let source = "a // ä\nb /* ö\n*/ c\n";
        let blanked = blank_comments(source);
        assert_eq!(blanked.len(), source.len(), "gleiche Länge, damit Positionen stimmen");
        assert_eq!(blanked.lines().count(), source.lines().count(), "Zeilenenden bleiben");
        assert!(!blanked.contains('ä') && !blanked.contains('ö'), "Kommentartext ist weg: {blanked:?}");
        assert!(blanked.starts_with("a ") && blanked.ends_with(" c\n"), "Code bleibt: {blanked:?}");
    }

    #[test]
    fn strings_with_slash_star_do_not_open_a_comment() {
        let d = parse("#let p = \"src/*.typ\"\n= Eins\n").unwrap();
        assert_eq!(d.headings.len(), 1, "String darf keinen Kommentar öffnen: {d:?}");
    }
}

pub mod images;
