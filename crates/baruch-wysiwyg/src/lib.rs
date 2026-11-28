// SPDX-License-Identifier: AGPL-3.0-or-later
//! Typeset view editor (contract Round 5, "wysiwyg"; refinement Round 6, "satzbild"): translate section texts
//! (Typst) into editable blocks and back. The crate is pure, without database and
//! file system.
//!
//! - [`parse`] splits a text into [`Block`]s. Every block carries its source text `raw`
//!   **verbatim**, including the following empty lines; the concatenation of all `raw` is the text, byte for
//!   byte. `parse` never fails: what cannot be mapped safely becomes a
//!   [`BlockKind::Island`] ("Typst island") or an [`Inline::Island`] in a paragraph.
//! - [`serialize`] builds the new text from the original and a list of [`Edit`]s: `Keep`
//!   takes over `raw` verbatim, a new or edited block is generated as canonical
//!   Typst with [`emit`]. So an untouched block is never rewritten. In the body of a scene or
//!   work single blocks can be changed ([`Edit::Body`]).
//! - [`render_html`] shows the blocks as HTML for the typeset view (contenteditable), [`diff_html`]
//!   the difference of two texts in the typeset view, [`sheet_style`]/[`sheet_css`] derive font, colours
//!   and indentations from the top-level rules of the work template. [`render_html_with`] and
//!   [`diff_html_with`] additionally show images in islands with an image path ([`image_path`]).
//! - [`align`] aligns two revisions by top-level blocks and delivers the changed,
//!   inserted and removed hunks ([`Hunk`], Round 8); [`hunk_html_with`] shows a hunk in the
//!   typeset view. With that proposals stand at their place in the reading flow.
//!
//! Recognised are: paragraph, heading `=` … `======`, part and "without number" as markers (Round 14,
//! [`BlockKind::Part`], [`BlockKind::Unnumbered`], only at the top level), `_…_`/`#emph[…]`,
//! `*…*`/`#strong[…]`, `#footnote[…]`, `#quote[…]` (in the paragraph) and
//! `#quote(block: …, attribution: […])[…]` (as a block), lists with `- ` or `+ `, `#szene[…]`/`#szene(…)[…]`,
//! `#arbeit[…]` (own blocks with blocks in the body), labels `<…>` (also alone on the line after a heading),
//! escapes `\x`, `~`.
//! Round 18b ("konstrukte"): enumerations `1.` ([`BlockKind::Enum`]), term lists `/ Begriff: Erklärung`
//! ([`BlockKind::Terms`]), nested lists (`depths`), forced line break `\`
//! ([`Inline::Linebreak`]), `#link("…")[…]` ([`Inline::Link`]) and references `@name` ([`Inline::Ref`]). An
//! island can be changed in place in the source text ([`Edit::Raw`]).
//! Everything else is an island: `#let`/`#set`/`#show` and other code lines, own functions,
//! maths, raw text, comments, lists with mixed markers, indented term lists. Description in
//! `docs/WYSIWYG.md`.

mod align;
mod emit;
mod html;
mod parse;
mod scan;
mod style;

use serde::{Deserialize, Serialize};

pub use align::{Hunk, HunkKind, align, hunk_html_in, hunk_html_with};
pub use baruch_core::WorkLanguage;
pub use emit::{emit, emit_in};
pub use html::{
    diff_html, diff_html_in, diff_html_with, index_mark_label, render_html, render_html_in, render_html_titled,
    render_html_with,
};
pub use parse::parse;
pub use scan::{image_path, image_paths};
pub use style::{BoxStyle, SheetStyle, sheet_css, sheet_style, sheet_style_over, template_font};

/// A section text as a sequence of blocks.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Doc {
    pub blocks: Vec<Block>,
}

/// A block with its verbatim source text (including the following empty lines). In JSON the fields
/// of the kind and `raw` stand next to each other: `{"kind": "paragraph", "inlines": […], "raw": "…"}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Block {
    #[serde(flatten)]
    pub kind: BlockKind,
    #[serde(default)]
    pub raw: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BlockKind {
    /// `=` (1) to `======` (6) at the beginning of a line; one line. A label at the end of the line is in
    /// `inlines`; a label alone on the line after it (`= Titel⏎<name>`) also belongs to the
    /// heading and is in `label`.
    Heading {
        level: u8,
        inlines: Vec<Inline>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        label: Option<String>,
    },
    Paragraph {
        inlines: Vec<Inline>,
    },
    /// `#quote(block: …, attribution: […])[…]` as a whole block.
    Quote {
        block: bool,
        attribution: Option<Vec<Inline>>,
        body: Vec<Block>,
    },
    /// `- ` (unordered) or `+ ` (ordered). Round 18b: nested with `depths` (depth per item, 0 =
    /// top level, two spaces of indentation per level; empty = all 0). Nested items carry the same
    /// marker as the list; mixed markers stay an island.
    List {
        ordered: bool,
        items: Vec<Vec<Inline>>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        depths: Vec<u8>,
    },
    /// Round 18b: enumeration with digits `1. …`; `start` = number of the first item, then consecutive (per
    /// nested group from 1). `depths` as with [`BlockKind::List`].
    Enum {
        #[serde(default = "one")]
        start: u64,
        items: Vec<Vec<Inline>>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        depths: Vec<u8>,
    },
    /// Round 18b: term list `/ Begriff: Erklärung` (without nesting).
    Terms {
        items: Vec<Term>,
    },
    /// `#szene[…]` as a whole block, also with arguments before it (`#szene(ort: [Küche])[…]`, verbatim in
    /// `args`, without the brackets). The body consists of own, individually editable blocks.
    Scene {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        args: Option<String>,
        body: Vec<Block>,
    },
    /// `#arbeit[…]` as a whole block (like [`BlockKind::Scene`]).
    Work {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        args: Option<String>,
        body: Vec<Block>,
    },
    /// Round 14: part marker `#heading(level: 1, numbering: none)[T] <baruch-part>` (one line,
    /// [`baruch_typst::marker`]); `title` is the resolved title. Newly generated with
    /// [`baruch_typst::part_line`], on renaming a comment of the line is kept.
    Part {
        title: String,
    },
    /// Round 14: marker "without number" `#heading(level: N, numbering: none)[T] <baruch-unnumbered>`
    /// (`level` 1–3, one line); newly generated with [`baruch_typst::unnumbered_line`].
    Unnumbered {
        level: u8,
        title: String,
    },
    /// Typst island: is never newly generated, only taken over verbatim or deleted as a whole.
    Island,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum Inline {
    /// Text without escapes (`\#` → `#`, `~` → non-breaking space).
    Text {
        v: String,
    },
    /// Simple line break in the source text (a space in the typeset result).
    Soft,
    Emph {
        c: Vec<Inline>,
    },
    Strong {
        c: Vec<Inline>,
    },
    Footnote {
        c: Vec<Inline>,
    },
    /// `#quote[…]` in running text.
    Quote {
        c: Vec<Inline>,
    },
    /// `<name>`
    Label {
        v: String,
    },
    /// Typst island in the paragraph (formula, raw, own function, comment, an escape such as
    /// `\\'` or `\\-` that would become a shorthand as text), verbatim.
    Island {
        raw: String,
    },
    /// Round 18b: forced line break `\` (backslash before whitespace, line end or at the end).
    Linebreak,
    /// Round 18b: `#link("adresse")` or `#link("adresse")[Text]`; `c` = text of the link (`None` = without
    /// content block, the address is displayed).
    Link {
        url: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        c: Option<Vec<Inline>>,
    },
    /// Round 18b: reference `@ziel` or `@ziel[Zusatz]` (also `#ref(<ziel>)`); `supplement` is the content of the
    /// bracket as Typst markup, verbatim. In the typeset view a button, not editable.
    Ref {
        target: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        supplement: Option<String>,
    },
    /// Round 30a: cross-reference `#link(<k103>)[Text]` (with page number addition according to §3.3); `key` =
    /// fixed key.
    Xref {
        key: i64,
        c: Vec<Inline>,
        #[serde(default)]
        page: bool,
    },
    /// R31: index mark `#metadata((…))<baruch-idx>` ([`baruch_typst::index`]); unobtrusive in the typeset view,
    /// editable.
    IndexMark {
        mark: baruch_typst::index::IndexMark,
    },
}

/// An entry of a term list (Round 18b): `/ term: desc`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Term {
    pub term: Vec<Inline>,
    pub desc: Vec<Inline>,
}

fn one() -> u64 {
    1
}

/// An instruction to [`serialize`]: take over block `keep` of the original verbatim, generate a block
/// anew (`replaces` = index of the replaced original block, `None` = inserted) or change single blocks in the
/// body of a scene or work (`#szene[…]`, `#arbeit[…]`) (`body` = instructions
/// for the blocks of the body, by the same rules). Original blocks that occur in no instruction
/// are deleted. JSON: `{"keep": 3}`, `{"replaces": 2, "block": {"kind": "paragraph",
/// "inlines": […]}}` or `{"replaces": 4, "body": [{"keep": 0}, {"replaces": 1, "block": …}]}`.
/// Round 18b: `{"replaces": 5, "raw": "#let x = 2"}` replaces the source text of block 5 verbatim (island
/// edited in place); without `replaces` (new island) → [`WysiwygError::NewIsland`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Edit {
    Keep { keep: usize },
    Block { replaces: Option<usize>, block: BlockKind },
    Body { replaces: usize, body: Vec<Edit> },
    Raw { replaces: Option<usize>, raw: String },
}

/// An image for an island with an image path (see [`image_path`] and [`render_html_with`]): address
/// and alternative text for `<img>`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImageRef {
    pub src: String,
    pub alt: String,
    /// Round 11: there is no image in the work for the path. The typeset view then clearly reports „Bild fehlt:
    /// <Pfad>“ instead of an empty frame (the island stays byte-identical).
    #[serde(default)]
    pub missing: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum WysiwygError {
    #[error("Block {index} gibt es nicht; der Text hat {count} Blöcke.")]
    UnknownBlock { index: usize, count: usize },
    #[error("Block {0} ist mehrfach angegeben.")]
    DuplicateBlock(usize),
    #[error("Block {0} ist eine Typst-Insel; sie lässt sich nur im Quelltext ändern oder als Ganzes löschen.")]
    IslandEdited(usize),
    #[error("Eine Typst-Insel lässt sich im Satzbild nicht anlegen; bitte im Quelltext schreiben.")]
    NewIsland,
    #[error("Block {0} ist keine Szene und keine Arbeit; nur deren Rumpf lässt sich blockweise ändern.")]
    NotContainer(usize),
    /// Round 18b: a part arises only in the outline, not as a new or converted block in the typeset view.
    #[error("Einen Teil legt man in der Gliederung an, nicht im Satzbild.")]
    NewPart,
}

/// New text from `original` and `edits` (rules see [`Edit`]).
///
/// - `Keep` takes over `raw` verbatim; every index may occur at most once (not
///   additionally as `replaces` either), an unknown index is an error.
/// - A block edit generates canonical Typst ([`emit`]) and appends the separator of the replaced block
///   (its following empty lines), for an inserted block `\n\n`. A block that comes out
///   empty is dropped.
/// - An edited heading keeps label and comment of the original line, like
///   [`baruch_typst::heading_line`] (the label only if the new heading carries none).
/// - An island as a new block or as a replacement of an island → error; its source text is changed by
///   [`Edit::Raw`] (Round 18b: taken over verbatim, with the separator of the old block; empty = deleted).
/// - A part as a new block or in place of another block → [`WysiwygError::NewPart`] (Round 18b:
///   parts arise in the outline); renaming an existing part works.
/// - If two pieces do not follow each other as in the original (inserted, deleted, rearranged), there is
///   at least one empty line in between, so that nothing merges into one paragraph.
pub fn serialize(original: &Doc, edits: &[Edit]) -> Result<String, WysiwygError> {
    serialize_in(original, edits, WorkLanguage::De)
}

/// Like [`serialize`] in the work language `lang` (Round 39b): newly generated blocks are written with
/// [`emit_in`], so the page number addition of a cross reference carries the page abbreviation of the work
/// language (`p.` in an English work). Untouched blocks stay verbatim.
pub fn serialize_in(original: &Doc, edits: &[Edit], lang: WorkLanguage) -> Result<String, WysiwygError> {
    serialize_indented(original, edits, "", false, lang)
}

/// [`serialize`] for a body with indentation `indent`: newly generated pieces get it before every
/// line; raw blocks stay untouched, so that their content does not change. `indent_first`: the
/// first line of the result gets it too (the body starts on a line of its own, and the
/// first raw block carries the indentation of its first line itself); otherwise it belongs to the frame.
fn serialize_indented(
    original: &Doc,
    edits: &[Edit],
    indent: &str,
    indent_first: bool,
    lang: WorkLanguage,
) -> Result<String, WysiwygError> {
    let mut used = vec![false; original.blocks.len()];
    let mut take = |index: usize| take_block(original, &mut used, index);
    let mut out = String::new();
    let mut previous: Option<usize> = None;
    for edit in edits {
        let (index, piece) = match edit {
            Edit::Keep { keep } => (Some(*keep), take(*keep)?.raw.clone()),
            Edit::Block { block: BlockKind::Island, replaces: Some(index) } => {
                take(*index)?;
                return Err(WysiwygError::IslandEdited(*index));
            }
            Edit::Block { block: BlockKind::Island, replaces: None } => return Err(WysiwygError::NewIsland),
            Edit::Raw { replaces: None, .. } => return Err(WysiwygError::NewIsland),
            Edit::Raw { replaces: Some(index), raw } => {
                let old = take(*index)?;
                // Verbatim, as typed; in the body sent without its indentation or with it (both work).
                let text = dedent(raw.trim_start_matches(['\n', '\r']).trim_end(), indent);
                if text.trim().is_empty() {
                    continue;
                }
                let text = indent_lines(&text, indent, indent_first || !out.is_empty());
                (Some(*index), text + separator(&old.raw))
            }
            Edit::Block { block: BlockKind::Part { .. }, replaces: None } => return Err(WysiwygError::NewPart),
            Edit::Body { replaces: index, body } => {
                let old = take(*index)?;
                let (BlockKind::Scene { body: blocks, .. } | BlockKind::Work { body: blocks, .. }) = &old.kind else {
                    return Err(WysiwygError::NotContainer(*index));
                };
                let Some((head, inner, tail)) = parse::container_frame(&old.raw) else {
                    return Err(WysiwygError::NotContainer(*index));
                };
                // The same splitting as when reading, with the `raw` from this block (byte-identical).
                let mut inner_doc = Doc { blocks: parse::body_blocks(inner, 1) };
                debug_assert_eq!(inner_doc.blocks.len(), blocks.len(), "gleiche Zerlegung des Rumpfs");
                let start = inner.len() - inner.trim_start().len();
                let end = start.max(inner.trim_end().len());
                // If the body starts on a line of its own, the indentation of this line belongs to the
                // first block (`body_blocks` reads `inner.trim()`): it moves along when the block
                // no longer stands at the front, otherwise its first line would move to column 0 and its
                // continuation lines would stay indented (a list would become a nested one).
                let (lead, first_indent) = match inner[..start].rfind('\n') {
                    Some(newline) => inner[..start].split_at(newline + 1),
                    None => (&inner[..start], ""),
                };
                let own_line = inner[..start].contains('\n');
                if let Some(first) = inner_doc.blocks.first_mut() {
                    first.raw.insert_str(0, first_indent);
                }
                let text = serialize_indented(&inner_doc, body, parse::body_indent(inner), own_line, lang)?;
                let piece = format!("{head}{lead}{}{}{tail}", text.trim_end(), &inner[end..]);
                (Some(*index), piece)
            }
            Edit::Block { replaces: Some(index), block } => {
                let old = take(*index)?;
                if old.kind == BlockKind::Island {
                    return Err(WysiwygError::IslandEdited(*index));
                }
                if matches!(block, BlockKind::Part { .. }) && !matches!(old.kind, BlockKind::Part { .. }) {
                    return Err(WysiwygError::NewPart);
                }
                let mut text = emit_in(block, lang);
                if text.trim().is_empty() {
                    continue;
                }
                if let Some(line) = marker_line(block, &old.raw, &old.kind) {
                    text = line;
                }
                if let (BlockKind::Heading { level, inlines, label }, BlockKind::Heading { label: old_label, .. }) =
                    (block, &old.kind)
                {
                    // Only the line up to the first break is the heading (Round 7); label and
                    // comment of the original line hang on it, the rest becomes a paragraph after it.
                    let (head, rest) = emit::split_heading(inlines);
                    text = emit::heading_text(*level, &head, lang);
                    text.push_str(&heading_suffix(&old.raw, &head));
                    text.push_str(&heading_label_line(&old.raw, old_label.as_deref(), label.as_deref()));
                    if let Some(rest) = rest {
                        text.push_str("\n\n");
                        text.push_str(&emit_in(&BlockKind::Paragraph { inlines: rest }, lang));
                    }
                }
                let text = indent_piece(&text, indent, indent_first || !out.is_empty());
                (Some(*index), text + separator(&old.raw))
            }
            Edit::Block { replaces: None, block } => {
                let text = emit_in(block, lang);
                if text.trim().is_empty() {
                    continue;
                }
                (None, indent_piece(&text, indent, indent_first || !out.is_empty()) + "\n\n")
            }
        };
        let in_order = matches!((previous, index), (Some(a), Some(b)) if b == a + 1);
        if !out.is_empty() && !in_order {
            ensure_blank_line(&mut out);
        }
        out.push_str(&piece);
        previous = index;
    }
    Ok(out)
}

/// Edited marker (part or "without number") that replaces a block: a comment of the
/// original line is kept as with [`baruch_typst::heading_line`] (part from every line,
/// "without number" only from a marker "without number"). Without line end; `None` for other kinds.
fn marker_line(block: &BlockKind, old_raw: &str, old_kind: &BlockKind) -> Option<String> {
    let line = old_raw.split('\n').next().unwrap_or_default();
    let built = match (block, old_kind) {
        (BlockKind::Part { title }, _) => baruch_typst::heading_line(0, title, Some(line)),
        (BlockKind::Unnumbered { level, title }, BlockKind::Unnumbered { .. }) => {
            baruch_typst::heading_line((*level).clamp(1, 3), title, Some(line))
        }
        _ => return None,
    };
    Some(built.trim_end_matches('\n').to_owned())
}

/// Original block `index`, once; unknown or already used → error.
fn take_block<'a>(original: &'a Doc, used: &mut [bool], index: usize) -> Result<&'a Block, WysiwygError> {
    let count = original.blocks.len();
    let block = original.blocks.get(index).ok_or(WysiwygError::UnknownBlock { index, count })?;
    if std::mem::replace(&mut used[index], true) {
        return Err(WysiwygError::DuplicateBlock(index));
    }
    Ok(block)
}

/// Newly generated piece with the indentation of a body: before every non-empty line, the first only
/// if the piece does not stand at the beginning of the body. With a raw block (```` ``` ````) it stays as it is.
fn indent_piece(text: &str, indent: &str, first: bool) -> String {
    if text.contains("```") {
        return text.to_owned();
    }
    indent_lines(text, indent, first)
}

/// Indentation `indent` before every non-empty line, the first only with `first` – without exception, also with
/// a raw block: the source text of an island ([`Edit::Raw`]) stood in the body entirely with its indentation.
fn indent_lines(text: &str, indent: &str, first: bool) -> String {
    if indent.is_empty() {
        return text.to_owned();
    }
    let mut out = String::with_capacity(text.len() + indent.len() * 4);
    for (k, line) in text.split('\n').enumerate() {
        if k > 0 {
            out.push('\n');
        }
        if (k > 0 || first) && !line.is_empty() {
            out.push_str(indent);
        }
        out.push_str(line);
    }
    out
}

/// Remove the indentation `indent` of a body (source text of an island in the body, [`Edit::Raw`]);
/// [`indent_lines`] sets it uniformly again afterwards. The decision is made for the whole text, not line by
/// line: only if every non-empty continuation line starts with the indentation was the text sent with it (like
/// `data-raw`, the first line then carries it or not), and it is dropped everywhere. Otherwise the text came
/// without indentation, and nothing is removed – that way the indentation of indented lines (for instance in a
/// raw block) is kept.
fn dedent(text: &str, indent: &str) -> String {
    if indent.is_empty() {
        return text.to_owned();
    }
    let with_indent = text.split('\n').skip(1).all(|line| line.trim().is_empty() || line.starts_with(indent));
    if !with_indent {
        return text.to_owned();
    }
    text.split('\n').map(|line| line.strip_prefix(indent).unwrap_or(line)).collect::<Vec<_>>().join("\n")
}

/// Whitespace at the end of a block (line end and empty lines): is taken over on replacing.
fn separator(raw: &str) -> &str {
    &raw[raw.trim_end().len()..]
}

/// Label and comment of the original line of a heading, with a leading space (or empty):
/// what [`baruch_typst::heading_line`] would put after a new title. If the new
/// heading itself carries a label at the end, the old one is dropped.
fn heading_suffix(old_raw: &str, inlines: &[Inline]) -> String {
    let line = old_raw.split('\n').next().unwrap_or_default();
    let full = baruch_typst::heading_line(1, "X", Some(line));
    let mut suffix = full.trim_end_matches('\n').strip_prefix("= X").unwrap_or_default().to_owned();
    let has_label = matches!(inlines.last(), Some(Inline::Label { .. }));
    if has_label
        && suffix.starts_with(" <")
        && let Some(close) = suffix.find('>')
    {
        suffix.replace_range(..close + 1, "");
    }
    suffix
}

/// Label line after an edited heading (with a leading line end, or empty): the
/// original line stays verbatim as long as the label stays the same or the new one is missing; another
/// label yields a new line `<name>`.
fn heading_label_line(old_raw: &str, old_label: Option<&str>, new_label: Option<&str>) -> String {
    let old_line = old_label.and_then(|_| {
        let content = old_raw.trim_end();
        content.find('\n').map(|n| &content[n..])
    });
    match (new_label, old_line) {
        (Some(new), Some(line)) if Some(new) == old_label => line.to_owned(),
        (Some(new), _) => emit::label_line(new),
        (None, Some(line)) => line.to_owned(),
        (None, None) => String::new(),
    }
}

/// Makes sure that `out` ends with an empty line.
fn ensure_blank_line(out: &mut String) {
    let tail = &out[out.trim_end().len()..];
    match tail.matches('\n').count() {
        0 => out.push_str("\n\n"),
        1 => out.push('\n'),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_separator_is_the_trailing_whitespace() {
        assert_eq!(separator("Text\n\n"), "\n\n", "Leerzeilen hinter dem Block");
        assert_eq!(separator("= Titel\n"), "\n", "Zeilenende einer Überschrift");
        assert_eq!(separator("Ende"), "", "letzter Block ohne Zeilenende");
    }

    #[test]
    fn the_heading_suffix_keeps_label_and_comment() {
        assert_eq!(heading_suffix("= Alt <ziel> // Notiz\n\n", &[]), " <ziel> // Notiz", "beides bleibt");
        let labelled = [Inline::Label { v: "neu".into() }];
        assert_eq!(
            heading_label_line("= Alt\n  <ziel>\n\n", Some("ziel"), None),
            "\n  <ziel>",
            "die Labelzeile bleibt wörtlich"
        );
        assert_eq!(heading_label_line("= Alt\n<ziel>\n", Some("ziel"), Some("neu")), "\n<neu>", "neues Label");
        assert_eq!(heading_suffix("= Alt <ziel> // Notiz\n", &labelled), " // Notiz", "eigenes Label ersetzt das alte");
        assert_eq!(heading_suffix("= Alt\n", &[]), "", "nichts anzuhängen");
    }
}
