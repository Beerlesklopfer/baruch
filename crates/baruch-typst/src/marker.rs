// SPDX-License-Identifier: AGPL-3.0-or-later
//! Part and „ohne Nummer“ as a marker in the Typst text (contract Round 14, section 1).
//!
//! Two line forms, verbatim, one line in column 0:
//!
//! ```text
//! #heading(level: 1, numbering: none)[T] <baruch-part>          Teil
//! #heading(level: N, numbering: none)[T] <baruch-unnumbered>    Kapitel/Abschnitt ohne Nummer, N = 1..=3
//! ```
//!
//! Behind it a comment `// …` may stand after space/tab (it is kept when renaming). `T` is the title escaped like
//! [`crate::escape_title`], plus a `\` before a leading `=`, `-`, `+`, `/` and before the dot of a leading
//! `digits.` (otherwise a structure would start in the content block). `T` contains no unescaped `[` or `]`, no
//! comment and no open inline raw.
//!
//! Why this form: Typst sets both without a definition in the work template, `numbering: none` does not count in
//! Typst (chapters continue across parts as in Baruch), and the labels make the lines unambiguous as Baruch
//! markers. Only the canonical form is recognised; every deviation (other spacing, other label, indentation, digit
//! ≠ 1 for the part) stays text or work template – this way a freely written `#heading(...)` stays untouched as
//! before.

use crate::{comment_start, escape_title, label_start, read_heading};

/// Label of the part marker.
pub const PART_LABEL: &str = "baruch-part";
/// Label of the marker „ohne Nummer“.
pub const UNNUMBERED_LABEL: &str = "baruch-unnumbered";

/// Start of both markers up to the level.
const PREFIX: &str = "#heading(level: ";
/// Between level and title.
const MIDDLE: &str = ", numbering: none)[";

/// A recognised marker line (regardless of raw/comment before it).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Marker<'a> {
    /// 0 = part, 1–3 = „ohne Nummer“.
    pub level: u8,
    /// `T` verbatim.
    pub raw_title: &'a str,
    /// Comment from `//` to before the line end (empty if there is none).
    pub comment: &'a str,
    /// `\n`, `\r\n` or empty (last line without line end).
    pub ending: &'a str,
}

/// Split off the line end: (content, line end).
fn split_ending(line: &str) -> (&str, &str) {
    if let Some(content) = line.strip_suffix("\r\n") {
        (content, "\r\n")
    } else if let Some(content) = line.strip_suffix('\n') {
        (content, "\n")
    } else {
        (line, "")
    }
}

/// Length of `T` to before the closing `]` if `T` is canonical: escapes `\X` are skipped, inline raw must close in
/// the line, no unescaped `[`, no comment.
fn title_end(text: &str) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => {
                // A backslash at the end escapes the `]` – then the block does not close.
                let next = text[i + 1..].chars().next()?;
                i += 1 + next.len_utf8();
            }
            b'`' => {
                let run = text[i..].bytes().take_while(|&b| b == b'`').count();
                let close = text[i + run..].find(&"`".repeat(run))?;
                i += run + close + run;
            }
            b'/' if matches!(bytes.get(i + 1), Some(b'*')) => return None,
            b'/' if matches!(bytes.get(i + 1), Some(b'/')) && !crate::after_scheme(text, i) => return None,
            b'[' => return None,
            b']' => return Some(i),
            _ => i += 1,
        }
    }
    None
}

/// Reads a marker line in canonical form, otherwise `None`.
pub(crate) fn read_marker(line: &str) -> Option<Marker<'_>> {
    let (content, ending) = split_ending(line);
    let rest = content.strip_prefix(PREFIX)?;
    let level = match rest.as_bytes().first()? {
        digit @ b'1'..=b'3' => digit - b'0',
        _ => return None,
    };
    let rest = rest[1..].strip_prefix(MIDDLE)?;
    let end = title_end(rest)?;
    let raw_title = &rest[..end];
    let after = &rest[end + 1..];
    let (level, after) = if let Some(after) = after.strip_prefix(" <baruch-part>") {
        if level != 1 {
            return None;
        }
        (0, after)
    } else {
        (level, after.strip_prefix(" <baruch-unnumbered>")?)
    };
    let comment = if after.is_empty() {
        ""
    } else {
        let trimmed = after.trim_start_matches([' ', '\t']);
        if trimmed.len() == after.len() || !trimmed.starts_with("//") {
            return None;
        }
        trimmed
    };
    Some(Marker { level, raw_title, comment, ending })
}

/// `T` for the content block from an already escaped title: leading structure characters get a backslash.
fn guard_start(escaped: &str) -> String {
    if escaped.starts_with(['=', '-', '+', '/']) {
        return format!("\\{escaped}");
    }
    let digits = escaped.bytes().take_while(u8::is_ascii_digit).count();
    if digits > 0 && escaped[digits..].starts_with('.') {
        return format!("{}\\{}", &escaped[..digits], &escaped[digits..]);
    }
    escaped.to_owned()
}

/// `T` from a title (plain text).
fn content_title(title: &str) -> String {
    guard_start(&escape_title(title))
}

/// `T` from the literal title of a `=` line: unescaped `[` and `]` (outside of inline raw) get a backslash, leading
/// structure characters as well. `None` if the title would not stand safely in the content block (open inline raw,
/// comment) or contains code (unescaped `#` outside of inline raw, e.g. `#emph[…]`, `#footnote[…]`): the brackets
/// of a call must not get a backslash, otherwise Typst would set the function value as raw text. Then the fallback
/// through the show rule (`snapshot::fallback_rules`) applies, the text stays.
fn content_title_from_raw(raw: &str) -> Option<String> {
    let mut out = String::with_capacity(raw.len() + 4);
    let mut i = 0;
    while i < raw.len() {
        let c = raw[i..].chars().next()?;
        match c {
            '\\' => {
                let next = raw[i + 1..].chars().next().map_or(0, char::len_utf8);
                out.push_str(&raw[i..i + 1 + next]);
                i += 1 + next;
            }
            '`' => {
                let run = raw[i..].bytes().take_while(|&b| b == b'`').count();
                let close = raw[i + run..].find(&"`".repeat(run))?;
                out.push_str(&raw[i..i + run + close + run]);
                i += run + close + run;
            }
            '#' => return None,
            '[' | ']' => {
                out.push('\\');
                out.push(c);
                i += 1;
            }
            _ => {
                out.push(c);
                i += c.len_utf8();
            }
        }
    }
    let out = guard_start(&out);
    title_end(&format!("{out}]")).filter(|&end| end == out.len())?;
    Some(out)
}

fn with_comment(mut line: String, comment: &str) -> String {
    if !comment.is_empty() {
        line.push(' ');
        line.push_str(comment);
    }
    line
}

/// Part marker with line end.
pub fn part_line(title: &str) -> String {
    format!("{PREFIX}1{MIDDLE}{}] <{PART_LABEL}>\n", content_title(title))
}

/// Marker „ohne Nummer“ (level clamped to 1..=3) with line end.
pub fn unnumbered_line(level: u8, title: &str) -> String {
    format!("{PREFIX}{}{MIDDLE}{}] <{UNNUMBERED_LABEL}>\n", level.clamp(1, 3), content_title(title))
}

/// Comment (from `//` or `/*`) of a `=` line or of a marker, otherwise empty.
pub(crate) fn line_comment(line: &str) -> &str {
    if let Some(marker) = read_marker(line) {
        return marker.comment;
    }
    let content = split_ending(line).0;
    let equals = content.bytes().take_while(|&b| b == b'=').count();
    let rest = &content[equals..];
    if equals == 0 || !rest.starts_with([' ', '\t']) {
        return "";
    }
    let cut = comment_start(rest).unwrap_or(rest.len());
    rest[cut..].trim_end()
}

/// New line for [`crate::heading_line`] if a marker is created: level 0 → part; previous line „ohne Nummer“ and
/// level 1–3 → „ohne Nummer“ again. The comment of the previous line stays.
pub(crate) fn marker_heading_line(level: u8, title: &str, previous: Option<&str>) -> Option<String> {
    let comment = previous.map_or("", line_comment);
    let line = if level == 0 {
        part_line(title)
    } else if previous.and_then(read_marker).is_some_and(|m| m.level > 0) && level <= 3 {
        unnumbered_line(level, title)
    } else {
        return None;
    };
    let line = line.trim_end_matches('\n').to_owned();
    Some(with_comment(line, comment) + "\n")
}

/// Change the level of a heading line, title verbatim: `=` line → number of `=`; marker „ohne Nummer“ → `level: N`.
/// Part marker, level 0 or no heading → None. The line end stays. Levels above 3 do not exist in the outline; they
/// yield `None` too.
pub fn relevel_line(line: &str, level: u8) -> Option<String> {
    if !(1..=3).contains(&level) {
        return None;
    }
    if let Some(marker) = read_marker(line) {
        if marker.level == 0 {
            return None;
        }
        // The digit stands at a fixed place directly behind the prefix.
        return Some(format!("{PREFIX}{level}{}", &line[PREFIX.len() + 1..]));
    }
    read_heading(line)?;
    let equals = line.bytes().take_while(|&b| b == b'=').count();
    Some(format!("{}{}", "=".repeat(usize::from(level)), &line[equals..]))
}

/// Convert the first heading line of `body` (only whitespace before it), rest byte-identical: numbered=false and
/// `=` line without label → marker „ohne Nummer“ (title verbatim, comment stays); numbered=true and marker „ohne
/// Nummer“ → `=` line. Otherwise (already so, part, `=` line with a label of its own, no heading) → None. `None`
/// also if the literal title would not stand safely in the content block (open inline raw).
pub fn set_numbered_line(body: &str, numbered: bool) -> Option<String> {
    let first = body.len() - body.trim_start().len();
    let start = body[..first].rfind('\n').map_or(0, |at| at + 1);
    if start != first {
        return None; // indented: no heading
    }
    let end = body[start..].find('\n').map_or(body.len(), |at| start + at + 1);
    let line = &body[start..end];
    let replaced = if let Some(marker) = read_marker(line) {
        if marker.level == 0 || !numbered {
            return None;
        }
        let heading = format!("{} {}", "=".repeat(usize::from(marker.level)), marker.raw_title);
        with_comment(heading, marker.comment) + marker.ending
    } else {
        let (level, raw_title) = read_heading(line)?;
        if numbered {
            return None;
        }
        let (content, ending) = split_ending(line);
        let rest = &content[usize::from(level)..];
        let cut = comment_start(rest).unwrap_or(rest.len());
        if label_start(rest[..cut].trim()).is_some() {
            return None;
        }
        let title = content_title_from_raw(&raw_title)?;
        let marker = format!("{PREFIX}{level}{MIDDLE}{title}] <{UNNUMBERED_LABEL}>");
        with_comment(marker, rest[cut..].trim_end()) + ending
    };
    Some(format!("{}{replaced}{}", &body[..start], &body[end..]))
}

/// Typst string literal.
fn string_literal(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Typst rules for the part page in the typesetting (only R14a uses them): per part marker in order labels[i] as
/// number (empty = without), a right-hand page of its own, then a page break; ends with a line end.
///
/// The rule applies through the label `<baruch-part>` and uses only `it.body`, not `it` itself: this way heading
/// rules of the work template do not affect the part page, but the heading is kept for `outline` and `query`. It
/// belongs behind the work template (the rule defined last applies first). Counting uses the own counter
/// `baruch-part`, not the heading counter.
pub fn part_page_rule(labels: &[String]) -> String {
    let list: Vec<String> = labels.iter().map(|l| string_literal(l)).collect();
    // One element needs the comma, otherwise `("I")` would not be an array.
    let array = format!("({}{})", list.join(", "), if list.len() == 1 { "," } else { "" });
    format!(
        "#show <{PART_LABEL}>: it => {{ \
pagebreak(weak: true, to: \"odd\"); \
counter(\"baruch-part\").step(); \
v(1fr); \
context {{ let n = counter(\"baruch-part\").get().first(); \
let num = {array}.at(n - 1, default: \"\"); \
align(center, block({{ if num != \"\" {{ text(size: 1.4em, num); parbreak() }}; \
text(size: 2em, weight: \"bold\", it.body) }})) }}; \
v(1fr); \
pagebreak(weak: true) }}\n"
    )
}
