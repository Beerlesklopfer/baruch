// SPDX-License-Identifier: AGPL-3.0-or-later
//! Speaking time of a section text (Round 16b „Zeitwächter“, interface R16-S1).
//!
//! [`spoken_words`] counts the spoken words of a Typst text, [`speaking_seconds`] converts them into seconds with
//! words per minute, [`clock`] writes seconds as a clock time. Counting rules (also in `docs/Bedienung.md`, section
//! „Zeitwächter“):
//!
//! 1. CRLF → LF.
//! 2. Comments do not count: `/* … */` (also multi-line, nested) and `//` to the line end if `//` stands at the
//!    start of a line or after whitespace (`https://` stays text).
//! 3. Heading lines (after whitespace `=`…`=` + space, also the part markers `#heading(…)[…] <baruch-part>`) do not
//!    count.
//! 4. Raw `` `…` `` and ```` ```…``` ```` do not count; math `$…$` does not count.
//! 5. Code: `#name`, `#name.feld` including directly following argument groups `(…)` (bracketed, strings respected)
//!    is dropped; directly following content blocks `[…]` count – except for `#footnote` and `#arbeit` (not
//!    spoken). Lines with `#set`, `#show`, `#let`, `#import`, `#include` are dropped entirely (including brackets
//!    that reach beyond the line).
//! 6. Labels `<…>` and references `@name` are dropped.
//! 7. Markup characters `*` `_` are dropped, `~` becomes a space, list characters `- `, `+ `, `/ ` at the start of
//!    a line are dropped, escape `\x` → `x`, `\` as a line break → space.
//! 8. Word = run of non-whitespace with at least one letter or digit (`char::is_alphanumeric`); „Joh 3,16“ = 2
//!    words, a hyphenated word = 1.
//!
//! The crate only calculates; settings (words per minute, target time) are kept by the store.

/// Default: words per minute in free speech.
pub const DEFAULT_WORDS_PER_MINUTE: u32 = 110;
/// Smallest permitted speaking rate.
pub const MIN_WORDS_PER_MINUTE: u32 = 60;
/// Largest permitted speaking rate.
pub const MAX_WORDS_PER_MINUTE: u32 = 250;
/// Longest target time in minutes.
pub const MAX_TARGET_MINUTES: u32 = 240;

/// Spoken words of a section text (Typst) by the counting rules below.
///
/// Counting rules: see module documentation (1)–(8).
pub fn spoken_words(typst: &str) -> usize {
    let text = typst.replace("\r\n", "\n");
    spoken_text(&text).split_whitespace().filter(|w| w.chars().any(char::is_alphanumeric)).count()
}

/// Seconds for `words` words: rounded up ceil(words*60/wpm); wpm clamped to [MIN, MAX]; 0 words → 0.
pub fn speaking_seconds(words: usize, words_per_minute: u32) -> u32 {
    if words == 0 {
        return 0;
    }
    let wpm = u64::from(words_per_minute.clamp(MIN_WORDS_PER_MINUTE, MAX_WORDS_PER_MINUTE));
    let seconds = (words as u64).saturating_mul(60).div_ceil(wpm);
    u32::try_from(seconds).unwrap_or(u32::MAX)
}

/// Clock notation: below one hour „m:ss“ („0:45“, „12:05“), otherwise „h:mm:ss“ („1:02:03“).
pub fn clock(seconds: u32) -> String {
    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;
    let secs = seconds % 60;
    if hours == 0 { format!("{minutes}:{secs:02}") } else { format!("{hours}:{minutes:02}:{secs:02}") }
}

fn ident_start(ch: char) -> bool {
    ch.is_alphabetic() || ch == '_'
}

fn ident_char(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_' || ch == '-'
}

/// Skip the string `"…"` from `i` on (on the `"`); index behind the closing `"`.
fn skip_string(c: &[char], mut i: usize) -> usize {
    i += 1;
    while i < c.len() {
        match c[i] {
            '\\' => i += 2,
            '"' => return i + 1,
            _ => i += 1,
        }
    }
    c.len()
}

/// Skip to the next `close`, from `i` on (behind the opener); with `escapes`, `\x` does not end it.
fn skip_until(c: &[char], mut i: usize, close: &[char], escapes: bool) -> usize {
    while i < c.len() {
        if escapes && c[i] == '\\' {
            i += 2;
            continue;
        }
        if c[i..].starts_with(close) {
            return i + close.len();
        }
        i += 1;
    }
    c.len()
}

/// Skip a nested block comment from `i` on (on `/*`); index behind it.
fn skip_block_comment(c: &[char], mut i: usize) -> usize {
    let mut depth = 0usize;
    while i < c.len() {
        if c[i..].starts_with(&['/', '*']) {
            depth += 1;
            i += 2;
        } else if c[i..].starts_with(&['*', '/']) {
            depth = depth.saturating_sub(1);
            i += 2;
            if depth == 0 {
                return i;
            }
        } else {
            i += 1;
        }
    }
    c.len()
}

/// Skip a bracketed group from `i` on (on `(`, `[` or `{`); index behind the closing bracket. In `(…)` and `{…}`
/// code applies (strings count as a whole), in `[…]` markup (escapes, raw).
fn skip_group(c: &[char], mut i: usize) -> usize {
    let open = c[i];
    let close = match open {
        '(' => ')',
        '[' => ']',
        _ => '}',
    };
    let code = open != '[';
    i += 1;
    while i < c.len() {
        let ch = c[i];
        if ch == close {
            return i + 1;
        }
        match ch {
            '(' | '{' if code => i = skip_group(c, i),
            '[' => i = skip_group(c, i),
            '"' if code => i = skip_string(c, i),
            '\\' if !code => i += 2,
            '`' if !code => {
                i = if c[i..].starts_with(&['`', '`', '`']) {
                    skip_until(c, i + 3, &['`', '`', '`'], false)
                } else {
                    skip_until(c, i + 1, &['`'], false)
                }
            }
            '#' if !code && c.get(i + 1).is_some_and(|&n| ident_start(n)) => {
                i += 1;
                while i < c.len()
                    && (ident_char(c[i]) || (c[i] == '.' && c.get(i + 1).is_some_and(|&n| ident_start(n))))
                {
                    i += 1;
                }
                if c.get(i).is_some_and(|&n| n == '(' || n == '{') {
                    i = skip_group(c, i);
                }
            }
            '/' if c[i..].starts_with(&['/', '*']) => i = skip_block_comment(c, i),
            _ => i += 1,
        }
    }
    c.len()
}

/// Skip the rest of a code line from `i` on (to the line end outside of brackets and strings); index on the `\n` or
/// at the end.
fn skip_code_line(c: &[char], mut i: usize) -> usize {
    while i < c.len() && c[i] != '\n' {
        match c[i] {
            '(' | '[' | '{' => i = skip_group(c, i),
            '"' => i = skip_string(c, i),
            '/' if c[i..].starts_with(&['/', '*']) => i = skip_block_comment(c, i),
            _ => i += 1,
        }
    }
    i
}

const LINE_KEYWORDS: [&str; 6] = ["set", "show", "let", "import", "include", "heading"];

/// Does a `#` keyword start at `j` whose line is dropped entirely?
fn line_keyword_at(c: &[char], j: usize) -> bool {
    if c.get(j) != Some(&'#') {
        return false;
    }
    let mut k = j + 1;
    while k < c.len() && ident_char(c[k]) {
        k += 1;
    }
    let word: String = c[j + 1..k].iter().collect();
    LINE_KEYWORDS.contains(&word.as_str())
}

/// Heading line from `j` on (first non-whitespace character): `=`…`=` + space/tab/line end.
fn heading_at(c: &[char], j: usize) -> bool {
    let mut k = j;
    while k < c.len() && c[k] == '=' {
        k += 1;
    }
    k > j && (k == c.len() || matches!(c[k], ' ' | '\t' | '\n'))
}

/// Open block in the running text: `true` = content block of a code call (its `]` is dropped).
type OpenBlocks = Vec<bool>;

/// After `#` at `i` (on the `#`): process the code expression; returns the new index and puts a spoken content
/// block on `blocks` if one follows.
fn code_at(c: &[char], mut i: usize, blocks: &mut OpenBlocks) -> Option<usize> {
    let next = *c.get(i + 1)?;
    if next == '(' || next == '{' {
        return Some(skip_group(c, i + 1));
    }
    if !ident_start(next) {
        return None;
    }
    i += 1;
    let start = i;
    while i < c.len() && ident_char(c[i]) {
        i += 1;
    }
    let name: String = c[start..i].iter().collect();
    while i < c.len() && c[i] == '.' && c.get(i + 1).is_some_and(|&n| ident_start(n)) {
        i += 1;
        while i < c.len() && ident_char(c[i]) {
            i += 1;
        }
    }
    let silent = name == "footnote" || name == "arbeit";
    Some(arguments(c, i, silent, blocks))
}

/// Process argument groups from `i` on: `(…)` is dropped, `[…]` is silent or is opened as spoken.
fn arguments(c: &[char], mut i: usize, silent: bool, blocks: &mut OpenBlocks) -> usize {
    loop {
        match c.get(i) {
            Some('(') => i = skip_group(c, i),
            Some('[') if silent => i = skip_group(c, i),
            Some('[') => {
                blocks.push(true);
                return i + 1;
            }
            _ => return i,
        }
    }
}

/// Spoken text: everything that is dropped by the counting rules is removed.
fn spoken_text(src: &str) -> String {
    let c: Vec<char> = src.chars().collect();
    let mut out = String::with_capacity(src.len());
    let mut blocks: OpenBlocks = Vec::new();
    let mut i = 0usize;
    let mut line_start = true;
    while i < c.len() {
        if line_start {
            line_start = false;
            let mut j = i;
            while j < c.len() && matches!(c[j], ' ' | '\t') {
                j += 1;
            }
            if heading_at(&c, j) {
                while i < c.len() && c[i] != '\n' {
                    i += 1;
                }
                continue;
            }
            if line_keyword_at(&c, j) {
                i = skip_code_line(&c, j);
                continue;
            }
            if matches!(c.get(j), Some('-' | '+' | '/')) && matches!(c.get(j + 1), Some(' ' | '\t')) {
                i = j + 2;
                continue;
            }
        }
        let ch = c[i];
        match ch {
            '\n' => {
                out.push('\n');
                line_start = true;
                i += 1;
            }
            '/' if c.get(i + 1) == Some(&'*') => {
                let end = skip_block_comment(&c, i);
                out.extend(c[i..end].iter().filter(|&&x| x == '\n').map(|_| ' '));
                i = end;
            }
            '/' if c.get(i + 1) == Some(&'/') && (i == 0 || c[i - 1].is_whitespace()) => {
                while i < c.len() && c[i] != '\n' {
                    i += 1;
                }
            }
            '`' => {
                i = if c[i..].starts_with(&['`', '`', '`']) {
                    skip_until(&c, i + 3, &['`', '`', '`'], false)
                } else {
                    skip_until(&c, i + 1, &['`'], false)
                };
                out.push(' ');
            }
            '$' => {
                i = skip_until(&c, i + 1, &['$'], true);
                out.push(' ');
            }
            '\\' => match c.get(i + 1) {
                None => i += 1,
                Some(n) if n.is_whitespace() => {
                    out.push(' ');
                    i += 1;
                }
                Some('u') if c.get(i + 2) == Some(&'{') => {
                    let end = c[i + 3..].iter().position(|&x| x == '}').map(|p| i + 3 + p);
                    match end {
                        Some(end) => {
                            let hex: String = c[i + 3..end].iter().collect();
                            if let Some(decoded) = u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32) {
                                out.push(decoded);
                            }
                            i = end + 1;
                        }
                        None => {
                            out.push('u');
                            i += 2;
                        }
                    }
                }
                Some(&n) => {
                    out.push(n);
                    i += 2;
                }
            },
            '#' => match code_at(&c, i, &mut blocks) {
                Some(next) => i = next,
                None => {
                    out.push('#');
                    i += 1;
                }
            },
            '[' => {
                blocks.push(false);
                out.push('[');
                i += 1;
            }
            ']' => match blocks.pop() {
                Some(true) => {
                    // Another content block of the same call (`#grid[a][b]`) is a piece of text of its own.
                    if c.get(i + 1) == Some(&'[') {
                        out.push(' ');
                    }
                    i = arguments(&c, i + 1, false, &mut blocks);
                }
                Some(false) | None => {
                    out.push(']');
                    i += 1;
                }
            },
            '<' => {
                let mut k = i + 1;
                while k < c.len() && (ident_char(c[k]) || c[k] == '.' || c[k] == ':') {
                    k += 1;
                }
                if k > i + 1 && c.get(k) == Some(&'>') && ident_start(c[i + 1]) {
                    i = k + 1;
                } else {
                    out.push('<');
                    i += 1;
                }
            }
            '@' if (i == 0 || !c[i - 1].is_alphanumeric()) && c.get(i + 1).is_some_and(|&n| ident_start(n)) => {
                let mut k = i + 1;
                while k < c.len()
                    && (ident_char(c[k])
                        || ((c[k] == '.' || c[k] == ':') && c.get(k + 1).is_some_and(|&n| ident_char(n))))
                {
                    k += 1;
                }
                i = k;
            }
            '*' | '_' => i += 1,
            '~' => {
                out.push(' ');
                i += 1;
            }
            _ => {
                out.push(ch);
                i += 1;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_continued_content_block_after_a_call_counts_too() {
        assert_eq!(
            spoken_words("#grid[eins zwei][drei]"),
            3,
            "beide Inhaltsblöcke eines Aufrufs werden gesprochen, die Klammern nicht"
        );
    }
}
