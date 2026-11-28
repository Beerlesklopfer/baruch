// SPDX-License-Identifier: AGPL-3.0-or-later
//! Blocks → canonical Typst. Only for new or edited blocks; untouched ones are taken over verbatim by
//! [`crate::serialize`]. For every kind except the island `parse(emit(k)) == k` holds (without the
//! `raw` of nested blocks), provided `k` is canonical: text without line ends, no two texts
//! next to each other, no whitespace at the beginning of a line or at the end of a block, no two
//! line breaks in a row, no line break in a heading (top level or in
//! `_…_`/`*…*`). A heading with such a break (from the typeset view) ends there; the rest
//! becomes a paragraph after it ([`split_heading`], Round 7).
//!
//! Rules: special characters in the text are escaped (`\#`, `\*`, `\_` …), at the beginning of a line also
//! list, heading and enumeration markers; a non-breaking space becomes `~`. `_…_` and
//! `*…*` stand only at word boundaries and not nested in themselves, otherwise `#emph[…]` or
//! `#strong[…]`. After a `#` expression `(` and `.name` are escaped, after a
//! line comment a new line starts.

use crate::parse::MAX_LIST_DEPTH;
use crate::scan::is_label_char;
use crate::{Block, BlockKind, Inline, Term};
use baruch_core::WorkLanguage;

/// Canonical Typst of a block, without line end. An island yields `""` (it is never newly
/// generated); in a body (`#szene[…]`) an island stands with its `raw`. Same as [`emit_in`] for a German work.
pub fn emit(kind: &BlockKind) -> String {
    emit_in(kind, WorkLanguage::De)
}

/// Like [`emit`] in the work language `lang`: the page number addition of a cross reference uses the page
/// abbreviation of the book catalog (`S.` in German, `p.` in English, Round 39b).
pub fn emit_in(kind: &BlockKind, lang: WorkLanguage) -> String {
    let base = Ctx { lang, ..Ctx::default() };
    match kind {
        BlockKind::Heading { level, inlines, label } => {
            // Round 7 („Absatz wird Titel“): the heading ends at the first line break; what
            // stands after it is a paragraph of its own and never part of the heading line.
            let (head, rest) = split_heading(inlines);
            let mut line = heading_text(*level, &head, lang);
            if let Some(label) = label {
                line.push_str(&label_line(label));
            }
            if let Some(rest) = rest {
                line.push_str("\n\n");
                line.push_str(&emit_in(&BlockKind::Paragraph { inlines: rest }, lang));
            }
            line
        }
        BlockKind::Paragraph { inlines } => seq(inlines, &base, true),
        BlockKind::List { ordered, items, depths } => {
            let marker = if *ordered { "+" } else { "-" };
            let levels = list_depths(items.len(), depths);
            items
                .iter()
                .zip(&levels)
                .map(|(item, &d)| format!("{}{marker} {}", spaces(d), seq(item, &item_ctx(d, lang), true)))
                .collect::<Vec<_>>()
                .join("\n")
        }
        BlockKind::Enum { start, items, depths } => {
            // Consecutive numbers as when reading: at the top from `start`, every nested group from 1.
            let levels = list_depths(items.len(), depths);
            let mut counters: Vec<u64> = Vec::new();
            let mut lines = Vec::with_capacity(items.len());
            for (k, (item, &d)) in items.iter().zip(&levels).enumerate() {
                counters.truncate(d + 1);
                if counters.len() == d {
                    counters.push(if k == 0 { *start } else { 1 });
                }
                let number = counters[d];
                counters[d] = number.saturating_add(1);
                lines.push(format!("{}{number}. {}", spaces(d), seq(item, &item_ctx(d, lang), true)));
            }
            lines.join("\n")
        }
        BlockKind::Terms { items } => items
            .iter()
            .map(|Term { term, desc }| {
                // The term is one line; every colon in it is escaped (it would end the term).
                let term_ctx = Ctx { heading: true, term: true, after: Some(':'), ..base.clone() };
                let desc = seq(desc, &item_ctx(0, lang), true);
                let sep = if desc.is_empty() { "" } else { " " };
                format!("/ {}:{sep}{desc}", seq(term, &term_ctx, true))
            })
            .collect::<Vec<_>>()
            .join("\n"),
        BlockKind::Quote { block, attribution, body } => {
            let mut args = Vec::new();
            if *block || attribution.is_none() {
                args.push(format!("block: {block}"));
            }
            if let Some(attribution) = attribution {
                args.push(format!(
                    "attribution: [{}]",
                    seq(attribution, &Ctx { after: Some(']'), ..base.clone() }, false)
                ));
            }
            format!("#quote({}){}", args.join(", "), content(body, false, lang))
        }
        BlockKind::Scene { args, body } => format!("#szene{}{}", arguments(args.as_deref()), content(body, true, lang)),
        BlockKind::Work { args, body } => format!("#arbeit{}{}", arguments(args.as_deref()), content(body, true, lang)),
        BlockKind::Part { title } => baruch_typst::part_line(title).trim_end_matches('\n').to_owned(),
        BlockKind::Unnumbered { level, title } => {
            baruch_typst::unnumbered_line(*level, title).trim_end_matches('\n').to_owned()
        }
        BlockKind::Island => String::new(),
    }
}

/// Spaces for the indentation of nested list items (two per level).
const SPACES: &str = "                                        ";

/// Indentation of the level `depth` (two spaces per level, at most [`MAX_LIST_DEPTH`] + 1 levels).
fn spaces(depth: usize) -> &'static str {
    &SPACES[..(2 * depth.min(usize::from(MAX_LIST_DEPTH) + 1)).min(SPACES.len())]
}

/// Context for the text of a list item of the level `depth`: continuation lines indented one level deeper than
/// its marker.
fn item_ctx(depth: usize, lang: WorkLanguage) -> Ctx {
    Ctx { indent: spaces(depth + 1), lang, ..Ctx::default() }
}

/// Depth per item, made valid: the first item at the top, every further one at most one level deeper than the
/// one before, at most [`MAX_LIST_DEPTH`]; missing values = 0.
fn list_depths(count: usize, depths: &[u8]) -> Vec<usize> {
    let mut out: Vec<usize> = Vec::with_capacity(count);
    for k in 0..count {
        let wanted = usize::from(depths.get(k).copied().unwrap_or(0).min(MAX_LIST_DEPTH));
        let limit = out.last().map_or(0, |&d| d + 1);
        out.push(wanted.min(limit));
    }
    out
}

/// Heading line without label and without line end: `=` × level, space, title.
pub(crate) fn heading_text(level: u8, inlines: &[Inline], lang: WorkLanguage) -> String {
    let ctx = Ctx { heading: true, lang, ..Ctx::default() };
    format!("{} {}", "=".repeat(usize::from(level.clamp(1, 6))), seq(inlines, &ctx, false))
}

/// Split the inlines of a heading at the first line break ([`Inline::Soft`] or a line end
/// in the sense of Typst, [`baruch_typst::is_newline`], in a text), at the top level as in
/// `_…_` or `*…*`: before it the title, after it (if anything remains) the paragraph that stands under the
/// heading. A break in an emphasis splits it (`*Laub⏎Absatz*` → title
/// `*Laub*`, paragraph `*Absatz*`), because in the heading line it would otherwise become a space and
/// pull the paragraph into the title after all. Footnotes and quotes (`#footnote[…]`, `#quote[…]`) stay
/// whole; their content is a content block of its own. Breaks at the beginning do not count, whitespace at the
/// cut is dropped. That way a soft break (Shift+Enter, `<br>` of the browser, also in
/// `<b>`/`<i>`) in the typeset view can never pull paragraph text into the heading line; Typst knows only
/// single-line headings anyway.
pub(crate) fn split_heading(inlines: &[Inline]) -> (Vec<Inline>, Option<Vec<Inline>>) {
    let flat = trim_front(flatten_newlines(inlines));
    let (head, rest) = cut_at_break(flat);
    let Some(rest) = rest else {
        return (head, None);
    };
    let head = trim_back(head);
    let rest = trim_back(trim_front(rest));
    (head, (!rest.is_empty()).then_some(rest))
}

/// Treat line ends in texts (also in `_…_`/`*…*`) like simple breaks.
fn flatten_newlines(inlines: &[Inline]) -> Vec<Inline> {
    let mut flat: Vec<Inline> = Vec::with_capacity(inlines.len());
    for inline in inlines {
        match inline {
            Inline::Text { v } if v.chars().any(baruch_typst::is_newline) => {
                for (k, piece) in v.split(baruch_typst::is_newline).enumerate() {
                    if k > 0 {
                        flat.push(Inline::Soft);
                    }
                    if !piece.is_empty() {
                        flat.push(Inline::Text { v: piece.to_owned() });
                    }
                }
            }
            Inline::Emph { c } => flat.push(Inline::Emph { c: flatten_newlines(c) }),
            Inline::Strong { c } => flat.push(Inline::Strong { c: flatten_newlines(c) }),
            other => flat.push(other.clone()),
        }
    }
    flat
}

/// Builds the same emphasis (`Emph` or `Strong`) again from a content.
type Rewrap = fn(Vec<Inline>) -> Inline;

/// Split at the first break that would stand in the heading line (top level or in `_…_`/`*…*`,
/// arbitrarily deep). A split emphasis stands with its front part in the title and with the
/// rear part at the beginning of the rest; empty parts are dropped.
fn cut_at_break(inlines: Vec<Inline>) -> (Vec<Inline>, Option<Vec<Inline>>) {
    let mut head = Vec::with_capacity(inlines.len());
    let mut iter = inlines.into_iter();
    while let Some(inline) = iter.next() {
        let (c, wrap): (Vec<Inline>, Rewrap) = match inline {
            Inline::Soft => return (head, Some(iter.collect())),
            Inline::Emph { c } => (c, |c| Inline::Emph { c }),
            Inline::Strong { c } => (c, |c| Inline::Strong { c }),
            other => {
                head.push(other);
                continue;
            }
        };
        let (front, back) = cut_at_break(c);
        let Some(back) = back else {
            head.push(wrap(front));
            continue;
        };
        if !front.is_empty() {
            head.push(wrap(front));
        }
        let mut rest = Vec::new();
        if !back.is_empty() {
            rest.push(wrap(back));
        }
        rest.extend(iter);
        return (head, Some(rest));
    }
    (head, None)
}

/// Remove breaks, empty texts, empty emphases and whitespace at the beginning of a sequence, also
/// at the beginning of an emphasis standing at the front.
fn trim_front(mut inlines: Vec<Inline>) -> Vec<Inline> {
    loop {
        match inlines.first_mut() {
            Some(Inline::Soft) => {
                inlines.remove(0);
            }
            Some(Inline::Text { v }) => {
                let trimmed = v.trim_start_matches([' ', '\t']);
                if trimmed.is_empty() {
                    inlines.remove(0);
                } else {
                    *v = trimmed.to_owned();
                    return inlines;
                }
            }
            Some(Inline::Emph { c } | Inline::Strong { c }) => {
                *c = trim_front(std::mem::take(c));
                if c.is_empty() {
                    inlines.remove(0);
                } else {
                    return inlines;
                }
            }
            _ => return inlines,
        }
    }
}

/// Like [`trim_front`], but at the end of the sequence.
fn trim_back(mut inlines: Vec<Inline>) -> Vec<Inline> {
    loop {
        match inlines.last_mut() {
            Some(Inline::Soft) => {
                inlines.pop();
            }
            Some(Inline::Text { v }) => {
                let trimmed = v.trim_end_matches([' ', '\t']).len();
                if trimmed == 0 {
                    inlines.pop();
                } else {
                    v.truncate(trimmed);
                    return inlines;
                }
            }
            Some(Inline::Emph { c } | Inline::Strong { c }) => {
                *c = trim_back(std::mem::take(c));
                if c.is_empty() {
                    inlines.pop();
                } else {
                    return inlines;
                }
            }
            _ => return inlines,
        }
    }
}

/// Line with the label of a heading (`\n<name>`); a name with foreign characters would yield no
/// label and is dropped.
pub(crate) fn label_line(label: &str) -> String {
    if !label.is_empty() && label.chars().all(crate::scan::is_label_char) {
        format!("\n<{label}>")
    } else {
        String::new()
    }
}

/// Arguments before the body (`(ort: [Küche])`) verbatim; without arguments nothing.
fn arguments(args: Option<&str>) -> String {
    args.map(|a| format!("({a})")).unwrap_or_default()
}

/// Body of a block as a content block: a single single-line paragraph as `[…]`, otherwise
/// multi-line with empty lines between the blocks.
fn content(body: &[Block], multiline: bool, lang: WorkLanguage) -> String {
    let parts: Vec<String> = body
        .iter()
        .map(|block| match block.kind {
            BlockKind::Island => block.raw.trim().to_owned(),
            ref kind => emit_in(kind, lang),
        })
        .filter(|part| !part.is_empty())
        .collect();
    if parts.is_empty() {
        return "[]".into();
    }
    let single = !multiline
        && parts.len() == 1
        && matches!(body[0].kind, BlockKind::Paragraph { .. })
        && !parts[0].contains('\n')
        && !parts[0].contains("//")
        && !parts[0].ends_with('\\');
    if single { format!("[{}]", parts[0]) } else { format!("[\n{}\n]", parts.join("\n\n")) }
}

#[derive(Clone, Default)]
struct Ctx {
    /// Indentation after a line break (list item: two spaces).
    indent: &'static str,
    /// Top level of a heading: line breaks become spaces.
    heading: bool,
    /// Within `_…_` or `*…*`: the same character must not open once more.
    in_emph: bool,
    in_strong: bool,
    /// Character that stands after the whole sequence (`_`, `*` or `]`); `None` at the end of the block.
    after: Option<char>,
    /// Term of a term list (Round 18b): `:` is escaped, also in footnotes and links in it.
    term: bool,
    /// Work language: page abbreviation of a cross reference's page number addition (Round 39b).
    lang: WorkLanguage,
}

impl Ctx {
    /// New content block `[…]` (footnote, quote, link, `#emph[…]`): indentation and term stay.
    fn fresh(&self) -> Ctx {
        Ctx { indent: self.indent, after: Some(']'), term: self.term, lang: self.lang, ..Ctx::default() }
    }
}

/// First character with which an inline starts (for the word boundary before it).
fn first_char(inline: &Inline) -> Option<char> {
    match inline {
        Inline::Text { v } => v.chars().next().map(|c| if needs_escape(c) { '\\' } else { c }),
        Inline::Soft => Some('\n'),
        Inline::Island { raw } => raw.chars().next(),
        Inline::Label { .. } => Some('<'),
        Inline::Emph { .. } => Some('_'),
        Inline::Strong { .. } => Some('*'),
        Inline::Footnote { .. }
        | Inline::Quote { .. }
        | Inline::Link { .. }
        | Inline::Xref { .. }
        | Inline::IndexMark { .. } => Some('#'),
        Inline::Linebreak => Some('\\'),
        Inline::Ref { .. } => Some('@'),
    }
}

/// String literal for Typst (`"…"`, backslash, quotation marks and line ends escaped).
fn string_literal(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for c in value.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Can a reference stand as `@ziel`? The target must start as when reading (letter, digit, `_`), must not end
/// with `.` or `:`, and the character after it must not belong to the target; otherwise `#ref(<ziel>)`.
fn plain_ref(target: &str, following: Option<char>) -> bool {
    target.chars().next().is_some_and(|c| c.is_alphanumeric() || c == '_')
        && !target.ends_with(['.', ':'])
        && !following.is_some_and(is_label_char)
}

/// Characters that are always escaped in markup.
fn needs_escape(c: char) -> bool {
    matches!(c, '\\' | '#' | '*' | '_' | '$' | '@' | '<' | '>' | '[' | ']' | '`' | '~')
}

fn wordy(c: Option<char>) -> bool {
    c.is_some_and(char::is_alphanumeric)
}

/// Sequence of inlines; `line_start`: does the first stand at the beginning of a line (paragraph, list item)?
fn seq(inlines: &[Inline], ctx: &Ctx, line_start: bool) -> String {
    let mut out = String::new();
    let mut at_line_start = line_start;
    let mut after_hash = false;
    for (k, inline) in inlines.iter().enumerate() {
        let next = inlines.get(k + 1);
        // First character after this inline: the next one or what closes the sequence.
        let following = next.and_then(first_char).or(ctx.after);
        let was_hash = std::mem::take(&mut after_hash);
        let starting = std::mem::take(&mut at_line_start);
        match inline {
            Inline::Text { v } => text(&mut out, v, starting, was_hash, following, ctx.term),
            Inline::Soft if ctx.heading => out.push(' '),
            Inline::Soft => {
                out.push('\n');
                out.push_str(ctx.indent);
                at_line_start = true;
            }
            Inline::Emph { c } | Inline::Strong { c } => {
                let emph = matches!(inline, Inline::Emph { .. });
                let (delimiter, function, nested) =
                    if emph { ('_', "emph", ctx.in_emph) } else { ('*', "strong", ctx.in_strong) };
                let inner_ctx = Ctx {
                    in_emph: ctx.in_emph || emph,
                    in_strong: ctx.in_strong || !emph,
                    after: Some(delimiter),
                    ..ctx.clone()
                };
                let inner = seq(c, &inner_ctx, false);
                let prev = out.chars().next_back();
                let in_word = (wordy(prev) && wordy(inner.chars().next()))
                    || (wordy(inner.chars().next_back()) && wordy(following));
                // `*/` or `/*` at the edges of `*…*` would be comment markers.
                let comment = !emph && (inner.starts_with('/') || inner.ends_with('/'));
                if nested || in_word || comment || inner.is_empty() {
                    out.push_str(&format!("#{function}[{}]", seq(c, &ctx.fresh(), false)));
                    after_hash = true;
                } else {
                    out.push(delimiter);
                    out.push_str(&inner);
                    out.push(delimiter);
                }
            }
            Inline::Footnote { c } | Inline::Quote { c } => {
                let function = if matches!(inline, Inline::Footnote { .. }) { "footnote" } else { "quote" };
                out.push_str(&format!("#{function}[{}]", seq(c, &ctx.fresh(), false)));
                after_hash = true;
            }
            Inline::Linebreak => {
                // A space separates the break from what follows (`\]`, `\_` would be escapes); at the
                // end of the block and before a line end it stands alone.
                out.push('\\');
                if !matches!(next, Some(Inline::Soft)) && (next.is_some() || ctx.after.is_some()) {
                    out.push(' ');
                }
            }
            Inline::Link { url, c } => {
                out.push_str("#link(");
                out.push_str(&string_literal(url));
                out.push(')');
                if let Some(c) = c {
                    out.push_str(&format!("[{}]", seq(c, &ctx.fresh(), false)));
                }
                after_hash = true;
            }
            Inline::Xref { key, c, page } => {
                // Round 30a: form from `baruch_typst::xref::xref_snippet_in` (with page number addition in the work
                // language, Round 39b), `parse` reads it back in every work language.
                let link_text = seq(c, &ctx.fresh(), false);
                out.push_str(&baruch_typst::xref::xref_snippet_in(*key, &link_text, *page, ctx.lang));
                after_hash = true;
            }
            Inline::Ref { target, supplement } => {
                if target.is_empty() || !target.chars().all(is_label_char) {
                    // Not a valid target: as text, as typed.
                    text(&mut out, &format!("@{target}"), starting, was_hash, following, ctx.term);
                } else if plain_ref(target, following) {
                    out.push('@');
                    out.push_str(target);
                    if let Some(supplement) = supplement {
                        out.push_str(&format!("[{supplement}]"));
                    }
                } else {
                    out.push_str(&format!("#ref(<{target}>"));
                    if let Some(supplement) = supplement {
                        out.push_str(&format!(", supplement: [{supplement}]"));
                    }
                    out.push(')');
                    after_hash = true;
                }
            }
            Inline::Label { v } => {
                if !v.is_empty() && v.chars().all(crate::scan::is_label_char) {
                    out.push('<');
                    out.push_str(v);
                    out.push('>');
                } else {
                    text(&mut out, &format!("<{v}>"), starting, was_hash, following, ctx.term);
                }
            }
            Inline::IndexMark { mark } => {
                // R31: canonical form; ends with the label, after it text may stand without escaping.
                out.push_str(&baruch_typst::index::mark_source(mark));
            }
            Inline::Island { raw } => {
                out.push_str(raw);
                after_hash = raw.starts_with('#');
                // After a line comment the rest of the line would otherwise be lost in the comment.
                if raw.starts_with("//") && !matches!(next, None | Some(Inline::Soft)) {
                    out.push('\n');
                    out.push_str(ctx.indent);
                    at_line_start = true;
                }
            }
        }
    }
    out
}

/// Append text escaped; `term`: in the term of a term list also escape `:`.
fn text(out: &mut String, v: &str, line_start: bool, after_hash: bool, after: Option<char>, term: bool) {
    // All line ends in the sense of Typst, also U+2028 & co.: after them text would stand at the beginning of a line.
    let flat: String = v.chars().map(|c| if baruch_typst::is_newline(c) { ' ' } else { c }).collect();
    let v = if line_start { flat.trim_start_matches([' ', '\t']) } else { flat.as_str() };
    let chars: Vec<char> = v.chars().collect();
    // Enumeration at the beginning of a line: "1. …" → "1\. …".
    let digits = if line_start { chars.iter().take_while(|c| c.is_ascii_digit()).count() } else { 0 };
    for (k, &c) in chars.iter().enumerate() {
        let following = chars.get(k + 1).copied().or(after);
        let escape = needs_escape(c)
            || (term && c == ':')
            // At the beginning of a line prevent heading, list, enumeration, term. `--`, `-?` and `-1`
            // are not list items; escaped they would lose their shorthand (like `1\..`).
            || (k == 0
                && line_start
                && (matches!(c, '=' | '+' | '/')
                    || (c == '-' && !following.is_some_and(|n| n == '-' || n == '?' || n.is_ascii_digit()))))
            || (digits > 0 && k == digits && c == '.' && following != Some('.'))
            || (k == 0 && after_hash && (c == '(' || (c == '.' && following.is_some_and(crate::scan::is_ident_start))))
            // `//` and `/*` would start a comment (also with the character after the text).
            || (c == '/'
                && matches!(following, Some('/' | '*'))
                && !(out.ends_with("http:") || out.ends_with("https:")));
        match c {
            '\u{a0}' => out.push('~'),
            _ => {
                if escape {
                    out.push('\\');
                }
                out.push(c);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(v: &str) -> Inline {
        Inline::Text { v: v.into() }
    }

    #[test]
    fn special_characters_are_escaped() {
        let kind = BlockKind::Paragraph { inlines: vec![text("- #1 kostet $5 *heute* @x <y> [z] a_b ~ \\")] };
        assert_eq!(emit(&kind), r"\- \#1 kostet \$5 \*heute\* \@x \<y\> \[z\] a\_b \~ \\", "alles maskiert");
    }

    #[test]
    fn emphasis_inside_a_word_uses_the_function() {
        let kind =
            BlockKind::Paragraph { inlines: vec![text("Haus"), Inline::Emph { c: vec![text("tür")] }, text(" zu")] };
        assert_eq!(emit(&kind), "Haus#emph[tür] zu", "_ zwischen Wortzeichen wäre Text");
    }

    #[test]
    fn a_line_comment_island_is_followed_by_a_new_line() {
        let kind = BlockKind::Paragraph {
            inlines: vec![text("Satz "), Inline::Island { raw: "// Notiz".into() }, text("weiter")],
        };
        assert_eq!(emit(&kind), "Satz // Notiz\nweiter", "der Text danach landet nicht im Kommentar");
    }
}
