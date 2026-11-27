// SPDX-License-Identifier: AGPL-3.0-or-later
//! Cross-references to outline points (Round 30a).
//!
//! - Every outline point has a fixed jump label `<k{key}>` ([`key_label`]); it is only projected on export, mirror
//!   and typesetting ([`label_heading`], [`label_section`]), the texts in the database stay unchanged.
//!   - A `=` heading line without a label of its own gets ` <kN>` appended (before a comment): `= Titel <k5> // Notiz`.
//!   - A heading with a label of its own (part `<baruch-part>`, „ohne Nummer“ `<baruch-unnumbered>`, a label of the
//!     human at the line end or alone on the line below) keeps it; below it (behind a label line) stands the anchor
//!     line [`anchor_line`] `#metadata("baruch-key") <kN>`. Typst knows only one label per element.
//! - A reference in the text has the form `#link(<k103>)[die Einleitung]`, with page number
//!   `#link(<k103>)[die Einleitung, S.~#context counter(page).at(<k103>).first()]` ([`xref_snippet`]);
//!   [`find_xrefs`] finds both forms.
//! - On reading, [`strip_key_label`] (per line), [`strip_section`] (per section) or [`strip_key_labels`] (whole
//!   file) remove the jump labels again; [`remap_keys`] converts references to new keys.
//!
//! Only the canonical forms that these functions write themselves are recognised; the functions do not track raw
//! blocks and comments (a line `#metadata("baruch-key") <k5>` in a raw block would count as an anchor line).

use std::borrow::Cow;
use std::collections::HashMap;

use crate::marker::read_marker;
use crate::{comment_start, label_start, read_heading};

/// Start of the name of a jump label: `k` + key.
pub const KEY_LABEL_PREFIX: &str = "k";

/// Value of the anchor line ([`anchor_line`]).
pub const ANCHOR_VALUE: &str = "baruch-key";

/// Name of the jump label of the outline point `key` (without angle brackets): `k103`.
pub fn key_label(key: i64) -> String {
    format!("{KEY_LABEL_PREFIX}{key}")
}

/// Key from the name of a jump label: `k103` → 103; everything else (also `<k103>`, `k`, `k-1`, `k01`) → `None`.
pub fn parse_key_label(label: &str) -> Option<i64> {
    let digits = label.strip_prefix(KEY_LABEL_PREFIX)?;
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) || (digits.len() > 1 && digits.starts_with('0'))
    {
        return None;
    }
    digits.parse().ok()
}

/// Invisible anchor line (without line end) for headings with a label of their own: `#metadata("baruch-key") <k5>`.
pub fn anchor_line(key: i64) -> String {
    format!("#metadata(\"{ANCHOR_VALUE}\") <{}>", key_label(key))
}

/// Key of an anchor line ([`anchor_line`], line end allowed), otherwise `None`.
pub fn parse_anchor_line(line: &str) -> Option<i64> {
    let (content, _) = split_ending(line);
    let rest = content.strip_prefix(&format!("#metadata(\"{ANCHOR_VALUE}\") <"))?;
    parse_key_label(rest.strip_suffix('>')?)
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

/// Kind of a heading line for the projection.
enum HeadingForm {
    /// `=` line without a label of its own: position at which ` <kN>` is inserted (end of the title before
    /// whitespace and comment).
    Plain { insert_at: usize },
    /// Part, „ohne Nummer“ or `=` line with a label of its own: anchor line below it.
    Labelled,
}

/// Form of the heading line `content` (without line end); `None` = no heading.
fn heading_form(content: &str) -> Option<HeadingForm> {
    if read_marker(content).is_some() {
        return Some(HeadingForm::Labelled);
    }
    read_heading(content)?;
    let level = content.bytes().take_while(|&b| b == b'=').count();
    let rest = &content[level..];
    let cut = comment_start(rest).unwrap_or(rest.len());
    let head = &rest[..cut];
    if label_start(head.trim()).is_some() {
        return Some(HeadingForm::Labelled);
    }
    Some(HeadingForm::Plain { insert_at: level + head.trim_end().len() })
}

/// Heading line with the jump label of the outline point `key` (for export, mirror, typesetting). `=` line without
/// a label of its own: ` <kN>` behind the title (before whitespace, comment and line end). With a label of its own
/// (also part and „ohne Nummer“): the line, a line end and the anchor line ([`anchor_line`]) with the line end of
/// the line. No heading: unchanged. A label alone on the line below is only known to [`label_section`].
pub fn label_heading(line: &str, key: i64) -> String {
    let (content, ending) = split_ending(line);
    match heading_form(content) {
        None => line.to_owned(),
        Some(HeadingForm::Plain { insert_at }) => {
            format!("{} <{}>{}", &content[..insert_at], key_label(key), &line[insert_at..])
        }
        Some(HeadingForm::Labelled) => {
            let newline = if ending.is_empty() { "\n" } else { ending };
            format!("{content}{newline}{}{ending}", anchor_line(key))
        }
    }
}

/// Is `line` (without line end) a label alone on the line (`<name>`, indentation allowed)?
fn is_label_line(line: &str) -> bool {
    let trimmed = split_ending(line).0.trim();
    trimmed.starts_with('<') && label_start(trimmed) == Some(0)
}

/// Text of a section with the jump label `key` at its first line (the heading, [`label_heading`]). If a label
/// stands alone on the line directly below a heading (`= Titel⏎<name>`), it belongs to the heading; the anchor line
/// then goes behind the label. Without a heading in the first line: unchanged.
pub fn label_section(body: &str, key: i64) -> Cow<'_, str> {
    let mut lines = body.split_inclusive('\n');
    let Some(first) = lines.next() else { return Cow::Borrowed(body) };
    let (content, ending) = split_ending(first);
    match heading_form(content) {
        None => Cow::Borrowed(body),
        Some(HeadingForm::Plain { .. }) if !lines.clone().next().is_some_and(is_label_line) => {
            Cow::Owned(format!("{}{}", label_heading(first, key), &body[first.len()..]))
        }
        Some(_) => {
            // Anchor line behind the heading or behind the label line below it.
            let mut end = first.len();
            let mut last_ending = ending;
            if let Some(next) = lines.next().filter(|l| is_label_line(l)) {
                end += next.len();
                last_ending = split_ending(next).1;
            }
            let (head, rest) = body.split_at(end);
            let newline = if last_ending.is_empty() { "\n" } else { "" };
            let tail = if rest.is_empty() && last_ending.is_empty() { "" } else { "\n" };
            Cow::Owned(format!("{head}{newline}{}{tail}{rest}", anchor_line(key)))
        }
    }
}

/// Heading line without the jump label and the key that stood in it (on reading). A `=` line loses ` <kN>` behind
/// the title; an anchor line ([`anchor_line`]) is removed entirely (empty string including line end). Everything
/// else stays borrowed and without key.
pub fn strip_key_label(line: &str) -> (Cow<'_, str>, Option<i64>) {
    if let Some(key) = parse_anchor_line(line) {
        return (Cow::Borrowed(""), Some(key));
    }
    let (content, _) = split_ending(line);
    if read_heading(content).is_none() {
        return (Cow::Borrowed(line), None);
    }
    let level = content.bytes().take_while(|&b| b == b'=').count();
    let rest = &content[level..];
    let cut = comment_start(rest).unwrap_or(rest.len());
    let head = rest[..cut].trim_end();
    let end = level + head.len();
    let Some(open) = head.rfind(" <") else { return (Cow::Borrowed(line), None) };
    let Some(key) = head[open + 2..].strip_suffix('>').and_then(parse_key_label) else {
        return (Cow::Borrowed(line), None);
    };
    let start = level + open;
    (Cow::Owned(format!("{}{}", &line[..start], &line[end..])), Some(key))
}

/// Text of a section without the jump label (inverse of [`label_section`]) and the key from it: the first line
/// through [`strip_key_label`], an anchor line directly below it or behind a label line is removed.
pub fn strip_section(body: &str) -> (Cow<'_, str>, Option<i64>) {
    let mut lines = body.split_inclusive('\n');
    let Some(first) = lines.next() else { return (Cow::Borrowed(body), None) };
    let (head, key) = strip_key_label(first);
    if key.is_some() {
        return (Cow::Owned(format!("{head}{}", &body[first.len()..])), key);
    }
    let (content, _) = split_ending(first);
    if heading_form(content).is_none() {
        return (Cow::Borrowed(body), None);
    }
    // Anchor line directly below the heading or below its label line.
    let mut at = first.len();
    let mut next = lines.next();
    if let Some(label) = next.filter(|l| is_label_line(l)) {
        at += label.len();
        next = lines.next();
    }
    let Some(anchor) = next else { return (Cow::Borrowed(body), None) };
    let Some(key) = parse_anchor_line(anchor) else { return (Cow::Borrowed(body), None) };
    let mut out = String::with_capacity(body.len());
    out.push_str(&body[..at]);
    let rest = &body[at + anchor.len()..];
    if rest.is_empty() && split_ending(anchor).1.is_empty() {
        // The anchor line was the last line without a line end: the line end added before it is dropped with it.
        if let Some(stripped) = out.strip_suffix('\n') {
            out.truncate(stripped.len());
        }
    }
    out.push_str(rest);
    (Cow::Owned(out), Some(key))
}

/// Start of the typesetting form of an anchor line before a part ([`part_anchors_for_typesetting`]).
const PART_ANCHOR_BREAK: &str = "#pagebreak(weak: true, to: \"odd\")";

/// Only for the typesetting with part pages (`baruch_typst::part_page_rule`): the rule sets the part alone on a
/// right-hand page and breaks afterwards – an anchor line behind the part line would thus lie on the page after it.
/// Here it moves before the part line, with the same weak break to a right-hand page before it: `#pagebreak(weak:
/// true, to: "odd")#metadata("baruch-key") <kN>` – the break of the rule is then omitted, because the page only
/// carries the anchor (proven in `baruch-render/tests/xref.rs`). Number of lines and lines of the remaining
/// sections stay; heading and anchor only swap the line. Without a part with an anchor line: borrowed.
pub fn part_anchors_for_typesetting(text: &str) -> Cow<'_, str> {
    let lines: Vec<&str> = text.split_inclusive('\n').collect();
    let mut out = String::with_capacity(text.len() + 40);
    let mut changed = false;
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        let anchored = lines.get(i + 1).and_then(|next| parse_anchor_line(next));
        if let (Some(marker), Some(key)) = (read_marker(line), anchored)
            && marker.level == 0
            && !split_ending(line).1.is_empty()
        {
            let (part, ending) = split_ending(line);
            let after = split_ending(lines[i + 1]).1;
            out.push_str(&format!("{PART_ANCHOR_BREAK}{}{ending}{part}{after}", anchor_line(key)));
            changed = true;
            i += 2;
            continue;
        }
        out.push_str(line);
        i += 1;
    }
    if changed { Cow::Owned(out) } else { Cow::Borrowed(text) }
}

/// Whole text without jump labels: ` <kN>` removed in every heading line, anchor lines removed (for tests and for
/// comparing an export with its source). Without jump labels the text stays borrowed.
pub fn strip_key_labels(text: &str) -> Cow<'_, str> {
    if !text.contains(&format!("<{KEY_LABEL_PREFIX}")) {
        return Cow::Borrowed(text);
    }
    let lines: Vec<&str> = text.split_inclusive('\n').collect();
    let mut out = String::with_capacity(text.len());
    let mut changed = false;
    for (i, line) in lines.iter().enumerate() {
        // A byte order mark at the start of the file belongs to the work template; the heading behind it counts
        // nevertheless.
        let line = match line.strip_prefix(crate::BOM).filter(|_| i == 0) {
            Some(rest) => {
                out.push(crate::BOM);
                rest
            }
            None => line,
        };
        let (stripped, key) = strip_key_label(line);
        if key.is_some() {
            changed = true;
            if stripped.is_empty() && i + 1 == lines.len() && split_ending(line).1.is_empty() {
                // Last anchor line without a line end: the line end added before is dropped with it.
                if out.ends_with('\n') {
                    out.pop();
                }
            }
        }
        out.push_str(&stripped);
    }
    if changed { Cow::Owned(out) } else { Cow::Borrowed(text) }
}

/// Reference in the text to the outline point `key` with the link text `text` (Typst markup, inserted verbatim):
/// `#link(<k103>)[die Einleitung]`; with page number `#link(<k103>)[die Einleitung, S.~#context
/// counter(page).at(<k103>).first()]`.
pub fn xref_snippet(key: i64, text: &str, with_page: bool) -> String {
    let label = key_label(key);
    if with_page {
        format!("#link(<{label}>)[{text}{}]", page_suffix(key))
    } else {
        format!("#link(<{label}>)[{text}]")
    }
}

/// Page number addition in the link text: `, S.~#context counter(page).at(<k103>).first()`.
fn page_suffix(key: i64) -> String {
    format!(", S.~#context counter(page).at(<{}>).first()", key_label(key))
}

/// A reference in the text: bytes `start..end` of the whole form, key, link text, with page number?
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct XrefUse {
    pub start: usize,
    pub end: usize,
    pub key: i64,
    pub text: String,
    pub page: bool,
}

/// End (exclusive, behind `]`) of the content block that starts at `open` (byte of `[`): escapes `\X` and nested
/// brackets count. No end → `None`.
fn block_end(text: &str, open: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut depth = 0usize;
    let mut i = open;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => {
                let next = text[i + 1..].chars().next().map_or(0, char::len_utf8);
                i += 1 + next;
                continue;
            }
            b'[' => depth += 1,
            b']' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i + 1);
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}

/// All references (`#link(<kN>)[…]`, also with page number addition) in a text, in text order. An escaped `\#link`
/// does not count.
pub fn find_xrefs(text: &str) -> Vec<XrefUse> {
    const OPEN: &str = "#link(<";
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(found) = text[from..].find(OPEN) {
        let start = from + found;
        from = start + OPEN.len();
        if text[..start].ends_with('\\') && !text[..start].ends_with("\\\\") {
            continue;
        }
        let rest = &text[from..];
        let Some(close) = rest.find(">)[") else { continue };
        let Some(key) = parse_key_label(&rest[..close]) else { continue };
        let open = from + close + 2;
        let Some(end) = block_end(text, open) else { continue };
        let inner = &text[open + 1..end - 1];
        let suffix = page_suffix(key);
        let (inner, page) = match inner.strip_suffix(&suffix) {
            Some(shorter) => (shorter, true),
            None => (inner, false),
        };
        out.push(XrefUse { start, end, key, text: inner.to_owned(), page });
        from = end;
    }
    out
}

/// Replace references to old keys by new ones (`map`: old → new), also in the page number addition; keys without an
/// entry stay. Unchanged text stays borrowed.
pub fn remap_keys<'a>(text: &'a str, map: &HashMap<i64, i64>) -> Cow<'a, str> {
    let uses: Vec<XrefUse> =
        find_xrefs(text).into_iter().filter(|u| map.get(&u.key).is_some_and(|n| *n != u.key)).collect();
    if uses.is_empty() {
        return Cow::Borrowed(text);
    }
    let mut out = String::with_capacity(text.len() + 8 * uses.len());
    let mut at = 0;
    for u in uses {
        let new = map[&u.key];
        out.push_str(&text[at..u.start]);
        let old_label = format!("<{}>", key_label(u.key));
        let new_label = format!("<{}>", key_label(new));
        out.push_str(&text[u.start..u.end].replace(&old_label, &new_label));
        at = u.end;
    }
    out.push_str(&text[at..]);
    Cow::Owned(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_labels_round_trip_and_reject_other_names() {
        assert_eq!(key_label(103), "k103", "Name = k + Schlüssel");
        assert_eq!(parse_key_label("k103"), Some(103), "Rückweg zum Schlüssel");
        for bad in ["<k103>", "k", "k-1", "k01", "x103", "k1a"] {
            assert_eq!(parse_key_label(bad), None, "{bad:?} ist keine Sprungmarke eines Gliederungspunkts");
        }
    }

    #[test]
    fn a_snippet_links_to_the_label_with_or_without_page() {
        assert_eq!(xref_snippet(103, "die Einleitung", false), "#link(<k103>)[die Einleitung]", "Form aus §3.3");
        assert_eq!(
            xref_snippet(103, "die Einleitung", true),
            "#link(<k103>)[die Einleitung, S.~#context counter(page).at(<k103>).first()]",
            "Form mit Seitenzahl aus §3.3"
        );
    }
}
