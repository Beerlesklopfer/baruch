// SPDX-License-Identifier: AGPL-3.0-or-later
//! Typst text → blocks. First the logical lines (line ends outside of brackets,
//! raw text and comments) are grouped into chunks: empty lines separate, a heading is always a
//! chunk of its own, likewise lists (`-`, `+`, enumerations `1.`, term lists `/`, including indented
//! continuation lines and nested items) and code lines (`#let`, `#set` …). Then every
//! chunk is read; what does not fit safely becomes an island. The empty lines after a chunk belong
//! to its `raw`, empty lines at the beginning of the text form an island of their own.

use crate::scan::{
    Scanner, after_scheme, char_at, closed_content, closed_group, heading_comment_start, ident_end, is_keyword,
    is_label_char, logical_lines, next_char, skip_escape, skip_line_comment, string_value,
};
use crate::{Block, BlockKind, Doc, Inline, Term};

/// Deepest nesting of blocks (`#szene[#quote(…)[…]]`) and markup (`_*…*_`).
const MAX_NESTING: usize = 16;

/// Deepest nesting of list items (Round 18b; 0 = top level). Deeper → island.
pub(crate) const MAX_LIST_DEPTH: u8 = 8;

/// Split text into blocks; never fails. The concatenation of all `raw` is `body`, byte for byte.
pub fn parse(body: &str) -> Doc {
    Doc { blocks: blocks(body, 0) }
}

fn blocks(s: &str, depth: usize) -> Vec<Block> {
    chunks(s)
        .into_iter()
        .map(|chunk| {
            let raw = &s[chunk.start..chunk.end];
            Block { kind: kind_of(raw.trim_end(), chunk.kind, depth), raw: raw.to_owned() }
        })
        .collect()
}

/// Kind of a logical line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Line {
    Blank,
    Heading(u8),
    /// Part marker or marker "without number" (Round 14, [`marker_of`]).
    Marker,
    Item {
        marker: u8,
        indented: bool,
    },
    /// Round 18b: enumeration `12. …` with content.
    Enum {
        indented: bool,
    },
    /// Round 18b: term `/ …` with content.
    Term {
        indented: bool,
    },
    /// Line that starts with `#let`, `#set`, `#show` ….
    Code,
    /// `#szene[…]` or `#arbeit(…)[…]`, since Round 23a also `#quote(block: true, …)[…]`, as a whole (logical,
    /// also multi-line) line.
    Container,
    /// Only a label `<name>` (belongs to a heading when it follows one).
    Label,
    /// Empty list or enumeration item, empty term, indented heading.
    Structural,
    Indented,
    Text,
}

fn classify(line: &str) -> Line {
    let content = line.trim_end_matches('\n');
    if content.trim().is_empty() {
        return Line::Blank;
    }
    let rest = content.trim_start_matches([' ', '\t']);
    let indented = rest.len() < content.len();
    let bytes = rest.as_bytes();
    let blank_after = |at: usize| matches!(bytes.get(at), None | Some(b' ' | b'\t' | b'\r'));
    let equals = bytes.iter().take_while(|&&b| b == b'=').count();
    if equals > 0 && blank_after(equals) {
        return if !indented && (1..=6).contains(&equals) && bytes.len() > equals {
            Line::Heading(equals as u8)
        } else {
            Line::Structural
        };
    }
    if matches!(bytes[0], b'-' | b'+') && blank_after(1) {
        return if bytes.len() > 1 { Line::Item { marker: bytes[0], indented } } else { Line::Structural };
    }
    if bytes[0] == b'/' && blank_after(1) {
        return if bytes.len() > 1 { Line::Term { indented } } else { Line::Structural };
    }
    let digits = bytes.iter().take_while(|b| b.is_ascii_digit()).count();
    if digits > 0 && bytes.get(digits) == Some(&b'.') && blank_after(digits + 1) {
        return if bytes.len() > digits + 1 { Line::Enum { indented } } else { Line::Structural };
    }
    if !indented && marker_of(content).is_some() {
        return Line::Marker;
    }
    if bytes[0] == b'#' && is_keyword(&rest[1..ident_end(rest, 1)]) {
        return Line::Code;
    }
    if !indented && is_container_line(rest) {
        return Line::Container;
    }
    if !indented && label_only(rest).is_some() {
        return Line::Label;
    }
    if indented { Line::Indented } else { Line::Text }
}

/// Is the line exactly one closed expression `#szene…[…]` or `#arbeit…[…]`, or (Round 23a) a
/// quote block `#quote(block: true, …)[…]`? A quote block is a block of its own in Typst: a line directly
/// after it (for instance the attribution) starts a new paragraph, a line before it ends its paragraph.
/// Therefore it is, like scene and work, a chunk of its own instead of sticking together with the neighbouring
/// lines into an island. `#quote[…]` and `block: false` stand in running text and stay part of their paragraph;
/// unreadable arguments likewise (as before).
fn is_container_line(rest: &str) -> bool {
    if !rest.starts_with('#') {
        return false;
    }
    let name_end = ident_end(rest, 1);
    let name = &rest[1..name_end];
    if !matches!(name, "szene" | "arbeit" | "quote") {
        return false;
    }
    let content = rest.trim_end();
    let mut scanner = Scanner::new(content);
    let end = scanner.hash(0, 0);
    if scanner.open || end != content.len() || !content.ends_with(']') {
        return false;
    }
    if name != "quote" {
        return true;
    }
    if !content[name_end..].starts_with('(') {
        return false;
    }
    let Some(args_end) = closed_group(content, name_end) else { return false };
    content[args_end..].starts_with('[')
        && closed_content(content, args_end) == Some(content.len())
        && matches!(quote_args(&content[name_end + 1..args_end - 1], 0), Some((true, _)))
}

/// Level (0 = part, 1–3 = "without number") and resolved title if `line` (one logical line)
/// is exactly one canonical marker. Authoritative is [`baruch_typst::parse`]: the same line is
/// a part there (`level` 0) or a heading with `numbered: false`.
pub(crate) fn marker_of(line: &str) -> Option<(u8, String)> {
    if !line.starts_with("#heading(") || line.trim_end_matches(['\n', '\r']).contains('\n') {
        return None;
    }
    let doc = baruch_typst::parse(line).ok()?;
    let [heading] = doc.headings.as_slice() else { return None };
    (doc.template.is_empty() && (heading.level == 0 || !heading.numbered))
        .then(|| (heading.level, heading.title.clone()))
}

/// Name of the label if the line consists only of `<name>` (and whitespace).
fn label_only(line: &str) -> Option<&str> {
    let inner = line.trim().strip_prefix('<')?.strip_suffix('>')?;
    (!inner.is_empty() && inner.chars().all(is_label_char)).then_some(inner)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ChunkKind {
    /// Empty lines at the beginning.
    Leading,
    Heading(u8),
    /// A marker line (part or "without number"), always alone.
    Marker,
    /// List with the marker of its first item: `-`, `+`, `1` (enumeration) or `/` (term list).
    List {
        marker: u8,
    },
    Para,
    /// `#szene[…]`/`#arbeit[…]`/`#quote(block: true)[…]` alone: a block of its own, never joined with neighbouring
    /// lines.
    Container,
    Code,
    Structural,
}

struct Chunk {
    start: usize,
    end: usize,
    kind: ChunkKind,
    /// Number of non-empty logical lines.
    lines: usize,
    /// Empty lines are appended: the next non-empty chunk starts anew.
    closed: bool,
}

fn chunks(s: &str) -> Vec<Chunk> {
    let mut out: Vec<Chunk> = Vec::new();
    for (start, end) in logical_lines(s) {
        let line = classify(&s[start..end]);
        if line == Line::Blank {
            match out.last_mut() {
                Some(chunk) => {
                    chunk.end = end;
                    chunk.closed = chunk.kind != ChunkKind::Leading;
                }
                None => out.push(Chunk { start, end, kind: ChunkKind::Leading, lines: 0, closed: false }),
            }
            continue;
        }
        let continues = match out.last() {
            None => false,
            Some(chunk) if chunk.closed => false,
            Some(chunk) => match (chunk.kind, line) {
                // A label directly after the heading line belongs to the heading.
                (ChunkKind::Heading(_), Line::Label) => chunk.lines == 1,
                (ChunkKind::List { marker }, Line::Item { marker: m, indented: false }) => m == marker,
                (ChunkKind::List { marker }, Line::Enum { indented: false }) => marker == b'1',
                (ChunkKind::List { marker }, Line::Term { indented: false }) => marker == b'/',
                (
                    ChunkKind::List { .. },
                    Line::Item { indented: true, .. }
                    | Line::Enum { indented: true }
                    | Line::Term { indented: true }
                    | Line::Indented,
                ) => true,
                (ChunkKind::Para, Line::Text | Line::Indented | Line::Label) => true,
                (ChunkKind::Code, Line::Code) => true,
                (ChunkKind::Structural, Line::Text | Line::Indented | Line::Structural | Line::Label) => true,
                _ => false,
            },
        };
        if continues && let Some(chunk) = out.last_mut() {
            chunk.end = end;
            chunk.lines += 1;
            continue;
        }
        let kind = match line {
            Line::Heading(level) => ChunkKind::Heading(level),
            Line::Marker => ChunkKind::Marker,
            Line::Item { marker, indented: false } => ChunkKind::List { marker },
            Line::Enum { indented: false } => ChunkKind::List { marker: b'1' },
            Line::Term { indented: false } => ChunkKind::List { marker: b'/' },
            Line::Code => ChunkKind::Code,
            Line::Container => ChunkKind::Container,
            Line::Text | Line::Indented | Line::Label => ChunkKind::Para,
            Line::Item { indented: true, .. }
            | Line::Enum { indented: true }
            | Line::Term { indented: true }
            | Line::Structural
            | Line::Blank => ChunkKind::Structural,
        };
        out.push(Chunk { start, end, kind, lines: 1, closed: false });
    }
    out
}

fn kind_of(content: &str, kind: ChunkKind, depth: usize) -> BlockKind {
    let found = match kind {
        ChunkKind::Leading | ChunkKind::Code | ChunkKind::Structural => None,
        ChunkKind::Heading(level) => heading(content, level, depth),
        // Only at the top level; in the body of a scene the line is text for baruch_typst too.
        ChunkKind::Marker if depth == 0 => marker_of(content).map(|(level, title)| match level {
            0 => BlockKind::Part { title },
            level => BlockKind::Unnumbered { level, title },
        }),
        ChunkKind::Marker => None,
        ChunkKind::List { marker } => list(content, marker, depth),
        ChunkKind::Para | ChunkKind::Container => paragraph(content, depth),
    };
    found.unwrap_or(BlockKind::Island)
}

fn heading(content: &str, level: u8, depth: usize) -> Option<BlockKind> {
    // Second logical line: only a label (see `chunks`).
    let (line, label) = match logical_lines(content).get(1) {
        Some(&(start, _)) => (content[..start].trim_end(), Some(label_only(&content[start..])?.to_owned())),
        None => (content, None),
    };
    let text = line[usize::from(level)..].trim_start_matches([' ', '\t']);
    // The comment after it is kept by `serialize` from the original line (like `heading_line`).
    let text = text[..heading_comment_start(text).unwrap_or(text.len())].trim_end();
    Some(BlockKind::Heading { level, inlines: inlines(text, depth)?, label })
}

/// Marker of a list item at the beginning of `rest` (without indentation) for a list with `marker`: length of
/// the marker and for enumerations the number. `None` if the line is not such an item.
fn item_marker(rest: &str, marker: u8) -> Option<(usize, Option<u64>)> {
    let bytes = rest.as_bytes();
    let blank_after = |at: usize| matches!(bytes.get(at), None | Some(b' ' | b'\t' | b'\n'));
    if marker == b'1' {
        let digits = bytes.iter().take_while(|b| b.is_ascii_digit()).count();
        if digits == 0 || bytes.get(digits) != Some(&b'.') || !blank_after(digits + 1) {
            return None;
        }
        // Numbers that are too large (above u64) cannot be mapped safely: then it is not an item of this list.
        return rest[..digits].parse::<u64>().ok().map(|n| (digits + 1, Some(n)));
    }
    (bytes.first() == Some(&marker) && blank_after(1)).then_some((1, None))
}

/// List, enumeration or term list from a list chunk (Round 18b also nested). Every line that starts with the
/// marker of the list (after indentation) starts an item; the depth follows from the column of the marker
/// (further right than the item before = nested, otherwise the level with the same column). Other lines are
/// continuation lines of the item and must be indented further than its marker. Unsafe things → `None`
/// (island): tabs in the indentation, a column between two levels, empty items, continuation lines after
/// nested items that would belong to the outer one, enumerations with a number other than consecutive (per
/// nested group from 1), nested term lists, more than [`MAX_LIST_DEPTH`] levels.
fn list(content: &str, marker: u8, depth: usize) -> Option<BlockKind> {
    // Per item: column of the marker, start of its line, start of the text after the marker, number.
    let mut items: Vec<(usize, usize, usize, Option<u64>)> = Vec::new();
    for (start, end) in logical_lines(content) {
        let line = &content[start..end];
        let rest = line.trim_start_matches([' ', '\t']);
        let lead = &line[..line.len() - rest.len()];
        if lead.contains('\t') {
            return None;
        }
        let column = lead.len();
        match item_marker(rest, marker) {
            Some((len, number)) => items.push((column, start, start + column + len, number)),
            None => {
                // Continuation line: indented further than the marker of its item.
                let &(current, ..) = items.last()?;
                if column <= current {
                    return None;
                }
            }
        }
    }
    let mut columns: Vec<usize> = Vec::new();
    let mut counters: Vec<u64> = Vec::new();
    let mut depths: Vec<u8> = Vec::with_capacity(items.len());
    let mut texts: Vec<&str> = Vec::with_capacity(items.len());
    let mut start = None;
    for (k, &(column, _, text_start, number)) in items.iter().enumerate() {
        let mut popped = false;
        while columns.last().is_some_and(|&top| top > column) {
            columns.pop();
            popped = true;
        }
        if columns.last() != Some(&column) {
            // Between two levels (further left than the item before, but on no known column).
            if popped {
                return None;
            }
            // Further right than the item before (or the first item): a new level.
            columns.push(column);
        }
        let level = u8::try_from(columns.len() - 1).ok().filter(|&l| l <= MAX_LIST_DEPTH)?;
        if let Some(number) = number {
            let at = usize::from(level);
            counters.truncate(at + 1);
            if counters.len() == at {
                // New group: at the very top the first number, nested from 1.
                counters.push(if k == 0 { number } else { 1 });
            }
            if counters[at] != number {
                return None;
            }
            counters[at] = number.checked_add(1)?;
            start.get_or_insert(number);
        }
        let end = items.get(k + 1).map_or(content.len(), |&(_, line_start, ..)| line_start);
        let text = content[text_start..end].trim_start_matches([' ', '\t']).trim_end();
        if text.is_empty() {
            return None;
        }
        depths.push(level);
        texts.push(text);
    }
    let nested = depths.iter().any(|&d| d > 0);
    let depths = if nested { depths } else { Vec::new() };
    if marker == b'/' {
        if nested {
            return None;
        }
        let items = texts.into_iter().map(|text| term(text, depth)).collect::<Option<Vec<Term>>>()?;
        return Some(BlockKind::Terms { items });
    }
    let items = texts.into_iter().map(|text| inlines(text, depth)).collect::<Option<Vec<_>>>()?;
    Some(match marker {
        b'1' => BlockKind::Enum { start: start.unwrap_or(1), items, depths },
        _ => BlockKind::List { ordered: marker == b'+', items, depths },
    })
}

/// An entry of a term list from the text after `/ `: term up to the first colon of the top
/// level ([`term_colon`]), explanation after it (without leading whitespace). Without a colon, with an empty or
/// multi-line term → `None`.
fn term(text: &str, depth: usize) -> Option<Term> {
    let colon = term_colon(text)?;
    let term_text = text[..colon].trim_end();
    let desc_text = text[colon + 1..].trim_start_matches([' ', '\t']);
    let term = inlines(term_text, depth)?;
    let desc = inlines(desc_text, depth)?;
    // A line break at the top level of the term (not in a footnote or the like) is not safe.
    (substantive(&term) && !term.contains(&Inline::Soft)).then_some(Term { term, desc })
}

/// Position of the colon that separates term and explanation: the first outside of escapes, raw text,
/// formulas, `#` expressions and content blocks; a comment or line end before it → `None`.
fn term_colon(text: &str) -> Option<usize> {
    let mut scanner = Scanner::new(text);
    let bytes = text.as_bytes();
    let mut j = 0;
    while j < text.len() {
        j = match bytes[j] {
            b'\\' => skip_escape(text, j),
            b'`' => scanner.raw(j),
            b'$' => scanner.math(j),
            b'#' => scanner.hash(j, 0).max(j + 1),
            b'[' => scanner.content(j, 0),
            b'/' if text[j..].starts_with("//") && !after_scheme(text, j) => return None,
            b'/' if text[j..].starts_with("/*") => scanner.block_comment(j),
            // Labels and references are tokens of their own; their `:` separates nothing (a `:` at the end of a
            // reference does not belong to it, as when reading).
            b'<' | b'@' => {
                let mut k = j + 1;
                while let Some(c) = char_at(text, k).filter(|&c| is_label_char(c)) {
                    k += c.len_utf8();
                }
                if bytes[j] == b'<' {
                    if k > j + 1 && text[k..].starts_with('>') { k + 1 } else { j + 1 }
                } else {
                    while k > j + 1 && text[..k].ends_with(['.', ':']) {
                        k -= 1;
                    }
                    k
                }
            }
            b':' => return (!scanner.open).then_some(j),
            b'\n' => return None,
            _ => next_char(text, j),
        };
    }
    None
}

fn paragraph(content: &str, depth: usize) -> Option<BlockKind> {
    let text = content.trim_start_matches([' ', '\t']);
    if text.starts_with('#') {
        let mut scanner = Scanner::new(text);
        let end = scanner.hash(0, 0);
        if scanner.open {
            return None;
        }
        if end == text.len() {
            let name_end = ident_end(text, 1);
            let name = &text[1..name_end];
            if is_keyword(name) {
                return None;
            }
            if let Some(kind) = container(text, name, name_end, depth) {
                return kind;
            }
        }
    }
    let inlines = inlines(text, depth)?;
    substantive(&inlines).then_some(BlockKind::Paragraph { inlines })
}

/// `#szene[…]`, `#arbeit[…]`, `#quote(…)[…]` as a whole paragraph: `Some(Some(kind))`, with unreadable
/// arguments `Some(None)` (island), otherwise `None` (ordinary paragraph).
fn container(text: &str, name: &str, name_end: usize, depth: usize) -> Option<Option<BlockKind>> {
    let rest = &text[name_end..];
    match name {
        "szene" | "arbeit" if rest.starts_with(['[', '(']) => {
            let Some((args, open)) = container_args(text, name_end) else { return Some(None) };
            if closed_content(text, open) != Some(text.len()) || depth >= MAX_NESTING {
                return Some(None);
            }
            let body = body_blocks(&text[open + 1..text.len() - 1], depth + 1);
            Some(Some(if name == "szene" { BlockKind::Scene { args, body } } else { BlockKind::Work { args, body } }))
        }
        "quote" if rest.starts_with('(') => {
            let Some(args_end) = closed_group(text, name_end) else { return Some(None) };
            if !text[args_end..].starts_with('[') || closed_content(text, args_end) != Some(text.len()) {
                return Some(None);
            }
            if depth >= MAX_NESTING {
                return Some(None);
            }
            let Some((block, attribution)) = quote_args(&text[name_end + 1..args_end - 1], depth) else {
                return Some(None);
            };
            let body = body_blocks(&text[args_end + 1..text.len() - 1], depth + 1);
            Some(Some(BlockKind::Quote { block, attribution, body }))
        }
        _ => None,
    }
}

/// Arguments before the body from `name_end`: (arguments without brackets, position of the `[`). Several
/// brackets in a row or an open one → `None`.
fn container_args(text: &str, name_end: usize) -> Option<(Option<String>, usize)> {
    if text[name_end..].starts_with('[') {
        return Some((None, name_end));
    }
    let args_end = closed_group(text, name_end)?;
    text[args_end..].starts_with('[').then(|| (Some(text[name_end + 1..args_end - 1].to_owned()), args_end))
}

/// Frame of a scene or work in its `raw`: everything up to `[` (`#szene(…)[`), the content in between
/// (with whitespace at the edge) and everything from `]` (including following empty lines); together `raw`.
/// `None` if `raw` is not a scene/work.
pub(crate) fn container_frame(raw: &str) -> Option<(&str, &str, &str)> {
    let content = raw.trim_end();
    let text = content.trim_start_matches([' ', '\t']);
    let lead = content.len() - text.len();
    let name_end = ident_end(text, 1);
    if !text.starts_with('#') || !matches!(&text[1..name_end], "szene" | "arbeit") {
        return None;
    }
    let (_, open) = container_args(text, name_end)?;
    if closed_content(text, open) != Some(text.len()) {
        return None;
    }
    let (open, close) = (lead + open + 1, lead + text.len() - 1);
    Some((&raw[..open], &raw[open..close], &raw[close..]))
}

/// Common indentation (spaces and tabs) of a body: `inner` is the content between
/// `[` and `]`. If the text starts in the line of the `[`, there is none (`#arbeit[- a⏎  b]`); otherwise it is
/// the longest common beginning of the indentation of the first line and all further non-empty ones.
pub(crate) fn body_indent(inner: &str) -> &str {
    let body = inner.trim();
    let lead = &inner[..inner.len() - inner.trim_start().len()];
    let Some(newline) = lead.rfind('\n') else { return "" };
    let first = &lead[newline + 1..];
    let first = &first[first.trim_end_matches([' ', '\t']).len()..];
    let mut indent: Option<&str> = Some(first);
    for line in body.split('\n').skip(1) {
        if line.trim().is_empty() {
            continue;
        }
        let lead = &line[..line.len() - line.trim_start_matches([' ', '\t']).len()];
        indent = Some(match indent {
            None => lead,
            Some(known) => {
                let common = known.bytes().zip(lead.bytes()).take_while(|(a, b)| a == b).count();
                &known[..common]
            }
        });
    }
    indent.unwrap_or_default()
}

/// Blocks of a body (`#szene[…]`, `#arbeit[…]`, `#quote(…)[…]`; `inner` = content between `[` and
/// `]`). The content is read without whitespace at the edge; the common indentation ([`body_indent`]) does
/// not count, so that indented lists in the body are recognised. The `raw` stay pieces of the original
/// (with indentation), their concatenation is `inner.trim()`.
pub(crate) fn body_blocks(inner: &str, depth: usize) -> Vec<Block> {
    let body = inner.trim();
    let indent = body_indent(inner);
    if indent.is_empty() {
        return blocks(body, depth);
    }
    // Read without indentation; remember line starts in the read text and in the original.
    let mut flat = String::with_capacity(body.len());
    let mut starts: Vec<(usize, usize)> = Vec::new();
    let mut original = 0;
    for (k, line) in body.split_inclusive('\n').enumerate() {
        starts.push((flat.len(), original));
        let stripped = if k > 0 { line.strip_prefix(indent).unwrap_or(line) } else { line };
        flat.push_str(stripped);
        original += line.len();
    }
    let to_original = |at: usize| -> usize {
        if at >= flat.len() {
            return body.len();
        }
        starts.binary_search_by_key(&at, |&(f, _)| f).map_or(at, |k| starts[k].1)
    };
    chunks(&flat)
        .into_iter()
        .map(|chunk| {
            let read = &flat[chunk.start..chunk.end];
            let raw = &body[to_original(chunk.start)..to_original(chunk.end)];
            Block { kind: kind_of(read.trim_end(), chunk.kind, depth), raw: raw.to_owned() }
        })
        .collect()
}

/// Arguments of `#quote(…)`: only `block: true|false` and `attribution: […]`, each at most once.
fn quote_args(args: &str, depth: usize) -> Option<(bool, Option<Vec<Inline>>)> {
    let mut block = None;
    let mut attribution = None;
    for part in split_args(args) {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let (name, value) = part.split_once(':')?;
        let value = value.trim();
        match name.trim() {
            "block" if block.is_none() => {
                block = Some(match value {
                    "true" => true,
                    "false" => false,
                    _ => return None,
                })
            }
            "attribution" if attribution.is_none() => {
                if !value.starts_with('[') || closed_content(value, 0) != Some(value.len()) {
                    return None;
                }
                attribution = Some(inlines(&value[1..value.len() - 1], depth + 1)?);
            }
            _ => return None,
        }
    }
    Some((block.unwrap_or(false), attribution))
}

/// Split an argument list at the commas of the top level.
pub(crate) fn split_args(args: &str) -> Vec<&str> {
    let mut scanner = Scanner::new(args);
    let mut parts = Vec::new();
    let mut start = 0;
    let mut j = 0;
    while j < args.len() {
        match args.as_bytes()[j] {
            b'"' => j = scanner.string(j),
            b'(' | b'{' => j = scanner.code_group(j, 0),
            b'[' => j = scanner.content(j, 0),
            b',' => {
                parts.push(&args[start..j]);
                j += 1;
                start = j;
            }
            _ => j = crate::scan::next_char(args, j),
        }
    }
    parts.push(&args[start..]);
    parts
}

/// Does a paragraph have content of its own (text or markup)? Only islands and line breaks → island.
fn substantive(inlines: &[Inline]) -> bool {
    inlines.iter().any(|inline| match inline {
        Inline::Text { v } => !v.trim().is_empty(),
        Inline::Soft | Inline::Island { .. } | Inline::IndexMark { .. } | Inline::Linebreak => false,
        _ => true,
    })
}

/// Read running text; `None` if something cannot be mapped safely (the block then becomes an island).
pub(crate) fn inlines(text: &str, depth: usize) -> Option<Vec<Inline>> {
    InlineParser { s: text, i: 0, depth }.seq(None).ok()
}

struct InlineParser<'a> {
    s: &'a str,
    i: usize,
    depth: usize,
}

/// Cannot be mapped: the block becomes an island.
struct Unmappable;

/// Word character in the sense of Typst: `_` and `*` between two of them are text (`a_b_c`).
fn wordy(c: Option<char>) -> bool {
    c.is_some_and(char::is_alphanumeric)
}

/// Does the escaping of `c` keep Typst from a shorthand that would come back as pure text?
/// Besides the always escaped characters `emit` escapes only by context (beginning of a line …);
/// `\'` and `\"` (straight instead of typographic quotation marks), `\-` before `-`, `?` or a digit
/// (otherwise `--`, `-?`, minus), `\.` next to a full stop (otherwise `...`) as well as `\-`, `\?` or a digit
/// after `-` would be something else as text afterwards. Such escapes therefore stay verbatim as an island.
/// `prev`: character before it in the source text, `rest`: source text after it (an escaped character
/// after it does not combine and therefore does not count).
fn escape_matters(prev: Option<char>, c: char, rest: &str) -> bool {
    let next = rest.chars().next();
    match c {
        '\'' | '"' => true,
        '-' => prev == Some('-') || next.is_some_and(|n| n == '-' || n == '?' || n.is_ascii_digit()),
        '.' => prev == Some('.') || next == Some('.'),
        '?' => prev == Some('-'),
        _ => c.is_ascii_digit() && prev == Some('-'),
    }
}

/// Does a structure (list, heading, enumeration) start after a line break? Then it is no
/// simple running text any more.
fn starts_structure(rest: &str) -> bool {
    let line = rest.split('\n').next().unwrap_or_default();
    !matches!(classify(line), Line::Text | Line::Indented | Line::Code | Line::Container | Line::Label)
}

fn push_text(out: &mut Vec<Inline>, text: &str) {
    if text.is_empty() {
        return;
    }
    if let Some(Inline::Text { v }) = out.last_mut() {
        v.push_str(text);
    } else {
        out.push(Inline::Text { v: text.to_owned() });
    }
}

impl InlineParser<'_> {
    fn seq(&mut self, close: Option<char>) -> Result<Vec<Inline>, Unmappable> {
        let s = self.s;
        let mut out = Vec::new();
        loop {
            let Some(c) = char_at(s, self.i) else {
                return if close.is_some() { Err(Unmappable) } else { Ok(out) };
            };
            let i = self.i;
            match c {
                '\\' => match char_at(s, i + 1) {
                    // Round 18b: forced line break. Spaces after it belong to it, a
                    // line end stays a simple break after it.
                    None | Some('\n') => {
                        out.push(Inline::Linebreak);
                        self.i = i + 1;
                    }
                    Some(' ' | '\t') => {
                        out.push(Inline::Linebreak);
                        self.i = i + 1 + s[i + 1..].len() - s[i + 1..].trim_start_matches([' ', '\t']).len();
                    }
                    Some(n) if n.is_whitespace() => return Err(Unmappable),
                    Some(n) => {
                        let (ch, end) = if n == 'u' && s[i + 2..].starts_with('{') {
                            let end = skip_escape(s, i);
                            let hex = s[i + 3..end].strip_suffix('}').ok_or(Unmappable)?;
                            let ch = u32::from_str_radix(hex, 16).ok().and_then(char::from_u32).ok_or(Unmappable)?;
                            (ch, end)
                        } else {
                            (n, i + 1 + n.len_utf8())
                        };
                        if escape_matters(s[..i].chars().next_back(), ch, &s[end..]) {
                            out.push(Inline::Island { raw: s[i..end].to_owned() });
                        } else {
                            push_text(&mut out, ch.encode_utf8(&mut [0; 4]));
                        }
                        self.i = end;
                    }
                },
                '_' | '*' => {
                    let prev = s[..i].chars().next_back();
                    if wordy(prev) && wordy(char_at(s, i + 1)) {
                        push_text(&mut out, if c == '_' { "_" } else { "*" });
                        self.i = i + 1;
                        continue;
                    }
                    self.i = i + 1;
                    if close == Some(c) {
                        return Ok(out);
                    }
                    if self.depth >= MAX_NESTING {
                        return Err(Unmappable);
                    }
                    self.depth += 1;
                    let inner = self.seq(Some(c));
                    self.depth -= 1;
                    let inner = inner?;
                    out.push(if c == '_' { Inline::Emph { c: inner } } else { Inline::Strong { c: inner } });
                }
                '#' => {
                    // R31: canonical index mark (including label) before the general expression.
                    if let Some((mark, used)) = baruch_typst::index::parse_mark(&s[i..]) {
                        out.push(Inline::IndexMark { mark });
                        self.i = i + used;
                        continue;
                    }
                    let mut scanner = Scanner::new(s);
                    let end = scanner.hash(i, 0);
                    if scanner.open || end == i + 1 {
                        return Err(Unmappable);
                    }
                    let expr = &s[i..end];
                    let inline = self.function(expr)?.unwrap_or_else(|| Inline::Island { raw: expr.to_owned() });
                    out.push(inline);
                    self.i = end;
                }
                '`' | '$' => {
                    let mut scanner = Scanner::new(s);
                    let end = if c == '`' { scanner.raw(i) } else { scanner.math(i) };
                    if scanner.open {
                        return Err(Unmappable);
                    }
                    out.push(Inline::Island { raw: s[i..end].to_owned() });
                    self.i = end;
                }
                '/' if s[i..].starts_with("//") && !after_scheme(s, i) => {
                    let end = skip_line_comment(s, i);
                    out.push(Inline::Island { raw: s[i..end].to_owned() });
                    self.i = end;
                }
                '/' if s[i..].starts_with("/*") => {
                    let mut scanner = Scanner::new(s);
                    let end = scanner.block_comment(i);
                    if scanner.open {
                        return Err(Unmappable);
                    }
                    out.push(Inline::Island { raw: s[i..end].to_owned() });
                    self.i = end;
                }
                '<' => {
                    let mut j = i + 1;
                    while let Some(n) = char_at(s, j).filter(|&n| is_label_char(n)) {
                        j += n.len_utf8();
                    }
                    if j > i + 1 && char_at(s, j) == Some('>') {
                        out.push(Inline::Label { v: s[i + 1..j].to_owned() });
                        self.i = j + 1;
                    } else {
                        push_text(&mut out, "<");
                        self.i = i + 1;
                    }
                }
                '@' if char_at(s, i + 1).is_some_and(|n| n.is_alphanumeric() || n == '_') => {
                    let mut j = i + 1;
                    while let Some(n) = char_at(s, j).filter(|&n| is_label_char(n)) {
                        j += n.len_utf8();
                    }
                    // Punctuation at the end does not belong to the reference („siehe @abb.“).
                    while s[..j].ends_with(['.', ':']) {
                        j -= 1;
                    }
                    // Round 18b: a reference (button in the typeset view), addition in square brackets verbatim.
                    let target = s[i + 1..j].to_owned();
                    let supplement = if s[j..].starts_with('[') {
                        let end = closed_content(s, j).ok_or(Unmappable)?;
                        let inner = s[j + 1..end - 1].to_owned();
                        j = end;
                        Some(inner)
                    } else {
                        None
                    };
                    out.push(Inline::Ref { target, supplement });
                    self.i = j;
                }
                '[' | ']' | '\r' => return Err(Unmappable),
                '~' => {
                    push_text(&mut out, "\u{a0}");
                    self.i = i + 1;
                }
                '\n' => {
                    let rest = s[i + 1..].trim_start_matches([' ', '\t']);
                    if rest.is_empty() {
                        // Whitespace at the end of a content block: without effect in the typeset result.
                        self.i = s.len();
                        continue;
                    }
                    if rest.starts_with('\n') || starts_structure(rest) {
                        return Err(Unmappable);
                    }
                    out.push(Inline::Soft);
                    self.i = s.len() - rest.len();
                }
                _ => {
                    push_text(&mut out, &s[i..i + c.len_utf8()]);
                    self.i = i + c.len_utf8();
                }
            }
        }
    }

    /// `#footnote[…]`, `#emph[…]`, `#strong[…]`, `#quote[…]` with exactly one content block; other
    /// expressions → `None` (island). A keyword (`#set` in the middle of the paragraph) → cannot be mapped.
    fn function(&self, expr: &str) -> Result<Option<Inline>, Unmappable> {
        let name_end = ident_end(expr, 1);
        let name = &expr[1..name_end];
        if is_keyword(name) {
            return Err(Unmappable);
        }
        match name {
            "link" => return self.link(expr, name_end),
            "ref" => return Ok(reference(expr, name_end)),
            _ => {}
        }
        if !matches!(name, "footnote" | "emph" | "strong" | "quote")
            || !expr[name_end..].starts_with('[')
            || closed_content(expr, name_end) != Some(expr.len())
        {
            return Ok(None);
        }
        if self.depth >= MAX_NESTING {
            return Err(Unmappable);
        }
        let inner = &expr[name_end + 1..expr.len() - 1];
        let c = InlineParser { s: inner, i: 0, depth: self.depth + 1 }.seq(None)?;
        Ok(Some(match name {
            "footnote" => Inline::Footnote { c },
            "emph" => Inline::Emph { c },
            "strong" => Inline::Strong { c },
            _ => Inline::Quote { c },
        }))
    }

    /// Round 18b: `#link("adresse")` or `#link("adresse")[Text]` – exactly one string as argument, at most
    /// one content block after it; otherwise island (`None`).
    fn link(&self, expr: &str, name_end: usize) -> Result<Option<Inline>, Unmappable> {
        if !expr[name_end..].starts_with('(') {
            return Ok(None);
        }
        let Some(args_end) = closed_group(expr, name_end) else { return Ok(None) };
        let args: Vec<&str> =
            split_args(&expr[name_end + 1..args_end - 1]).into_iter().filter(|a| !a.trim().is_empty()).collect();
        let [url] = args.as_slice() else { return Ok(None) };
        if let Some(key) = url.trim().strip_prefix('<').and_then(|l| l.strip_suffix('>')) {
            return self.xref(expr, args_end, key);
        }
        let Some(url) = string_value(url.trim()) else { return Ok(None) };
        let rest = &expr[args_end..];
        if rest.is_empty() {
            return Ok(Some(Inline::Link { url, c: None }));
        }
        if !rest.starts_with('[') || closed_content(expr, args_end) != Some(expr.len()) {
            return Ok(None);
        }
        if self.depth >= MAX_NESTING {
            return Err(Unmappable);
        }
        let inner = &expr[args_end + 1..expr.len() - 1];
        let c = InlineParser { s: inner, i: 0, depth: self.depth + 1 }.seq(None)?;
        Ok(Some(Inline::Link { url, c: Some(c) }))
    }

    /// Round 30a: cross-reference `#link(<kN>)[Text]` or with page number addition
    /// `#link(<kN>)[Text, S.~#context counter(page).at(<kN>).first()]` (form from
    /// `baruch_typst::xref::xref_snippet_in`, the same key in the addition; `p.` instead of `S.` in an English
    /// work). Another label, no content block or an empty link text → island (`None`).
    fn xref(&self, expr: &str, args_end: usize, label: &str) -> Result<Option<Inline>, Unmappable> {
        let Some(key) = baruch_typst::xref::parse_key_label(label) else { return Ok(None) };
        let rest = &expr[args_end..];
        if !rest.starts_with('[') || closed_content(expr, args_end) != Some(expr.len()) {
            return Ok(None);
        }
        if self.depth >= MAX_NESTING {
            return Err(Unmappable);
        }
        let inner = &expr[args_end + 1..expr.len() - 1];
        // Addition = what `xref_snippet_in` writes with page number after the (empty) link text, without the closing
        // `]`; in every work language (Round 39b: `, S.~…` German, `, p.~…` English), like
        // `baruch_typst::xref::find_xrefs`.
        let prefix = format!("#link(<{label}>)[").len();
        let shorter = baruch_core::WorkLanguage::ALL.into_iter().find_map(|lang| {
            let snippet = baruch_typst::xref::xref_snippet_in(key, "", true, lang);
            inner.strip_suffix(&snippet[prefix..snippet.len() - 1])
        });
        let (inner, page) = match shorter {
            Some(shorter) => (shorter, true),
            None => (inner, false),
        };
        let c = InlineParser { s: inner, i: 0, depth: self.depth + 1 }.seq(None)?;
        if c.is_empty() {
            return Ok(None);
        }
        Ok(Some(Inline::Xref { key, c, page }))
    }
}

/// Round 18b: `#ref(<ziel>)` or `#ref(<ziel>, supplement: […])` as a reference (that is how `emit` writes a
/// reference followed by a character that would belong to the target); other forms → island (`None`).
fn reference(expr: &str, name_end: usize) -> Option<Inline> {
    if !expr[name_end..].starts_with('(') || closed_group(expr, name_end) != Some(expr.len()) {
        return None;
    }
    let args: Vec<&str> =
        split_args(&expr[name_end + 1..expr.len() - 1]).into_iter().filter(|a| !a.trim().is_empty()).collect();
    let (label, supplement) = match args.as_slice() {
        [label] => (label.trim(), None),
        [label, supplement] => {
            let (name, value) = supplement.split_once(':')?;
            let value = value.trim();
            if name.trim() != "supplement" || !value.starts_with('[') || closed_content(value, 0) != Some(value.len()) {
                return None;
            }
            (label.trim(), Some(value[1..value.len() - 1].to_owned()))
        }
        _ => return None,
    };
    let target = label.strip_prefix('<')?.strip_suffix('>')?;
    (!target.is_empty() && target.chars().all(is_label_char))
        .then(|| Inline::Ref { target: target.to_owned(), supplement })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(v: &str) -> Inline {
        Inline::Text { v: v.into() }
    }

    #[test]
    fn a_paragraph_with_markup_becomes_inlines() {
        let doc = parse("Ein _leiser_ und *fester* Satz#footnote[Mit Anmerkung.].\nZweite Zeile <ziel>\n");
        assert_eq!(doc.blocks.len(), 1, "ein Absatz");
        assert_eq!(
            doc.blocks[0].kind,
            BlockKind::Paragraph {
                inlines: vec![
                    text("Ein "),
                    Inline::Emph { c: vec![text("leiser")] },
                    text(" und "),
                    Inline::Strong { c: vec![text("fester")] },
                    text(" Satz"),
                    Inline::Footnote { c: vec![text("Mit Anmerkung.")] },
                    text("."),
                    Inline::Soft,
                    text("Zweite Zeile "),
                    Inline::Label { v: "ziel".into() },
                ]
            },
            "Hervorhebung, Fett, Fußnote, Zeilenumbruch und Label erkannt"
        );
    }

    #[test]
    fn underscores_inside_words_stay_text() {
        let doc = parse("datei_name_lang\n");
        assert_eq!(
            doc.blocks[0].kind,
            BlockKind::Paragraph { inlines: vec![text("datei_name_lang")] },
            "zwischen Wortzeichen ist _ Text (wie in Typst)"
        );
    }

    #[test]
    fn code_lines_and_unclosed_markup_become_islands() {
        let doc = parse("#set text(fill: red)\nDanach Text.\n\nEin _offener Satz.\n");
        let kinds: Vec<&BlockKind> = doc.blocks.iter().map(|b| &b.kind).collect();
        assert_eq!(kinds.len(), 3, "Code-Zeile, Absatz, Absatz: {kinds:?}");
        assert_eq!(kinds[0], &BlockKind::Island, "#set ist eine Insel");
        assert!(matches!(kinds[1], BlockKind::Paragraph { .. }), "der Text danach bleibt bearbeitbar");
        assert_eq!(kinds[2], &BlockKind::Island, "nicht geschlossenes _ ist nicht sicher abbildbar");
    }
}
