// SPDX-License-Identifier: AGPL-3.0-or-later
//! Markdown → Typst (Round 21b): traceable conversion with `pulldown-cmark` (CommonMark, plus footnotes,
//! tables, strikethrough, definition lists, heading attributes and YAML front matter).
//!
//! The table of the conversion is in `docs/Architektur.md` („21b markdown-import“). Principles:
//! - **Text stays verbatim:** every character that means something in Typst markup (`\ # * _ $ @ < [ ] ` ~`, `/`
//!   before `/` or `*`, `-` before `-` or `?`) gets a backslash; at the beginning of a line also `= + - /` and the
//!   run of digits before a full stop (otherwise heading, list, enumeration, definition list).
//! - **Markup as a function:** `#emph[…]`, `#strong[…]`, `#strike[…]`, `#raw("…")`, `#link("…")[…]`,
//!   `#footnote[…]`, `#image("…")` – functions also apply in the middle of a word (unlike `_…_`/`*…*` in Typst). If
//!   a character follows directly that Typst would attach to the expression (`(`, `[`, `;`, `.` before a letter), a
//!   `;` stands before it (as with the projection of the first mention, Round 31).
//! - **Outline:** `#` → `=`, `##` → `==`, `###` → `===`; `####` to `######` are limited to level 3 (warning).
//!   Text before the first heading gets a chapter with the title of the work (warning), because in Baruch everything
//!   before the first heading is the work template.
//! - **Never discard silently:** every form that is not mapped yields an [`ImportWarning`] with a line
//!   (codes `md-…`); at most [`MAX_WARNINGS`], after that a collective note.

use std::collections::HashMap;
use std::ops::Range;

use pulldown_cmark::{Alignment, CodeBlockKind, Event, HeadingLevel, LinkType, Options, Parser, Tag};

use crate::i18n::tc;
use crate::import::ImportWarning;

/// Title of a chapter without a title of its own. It becomes text of the new work (book content, not interface text),
/// and so do the labels „Bild: …“ of images that are not taken over; they stay German like the default work language
/// of a new work (Round 39c, reste_1). The notes ([`ImportWarning`]) follow the interface language.
const UNTITLED: &str = "Ohne Titel";

/// Maximum number of notes of one conversion; above that a collective note `md-warnings-truncated`.
pub const MAX_WARNINGS: usize = 200;

/// Result of the conversion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Converted {
    /// Typst text (work template from the YAML front matter, then the sections), ends with `\n`.
    pub typst: String,
    /// Notes with the line in the Markdown file (1-based).
    pub warnings: Vec<ImportWarning>,
    /// Title from the YAML front matter (`title:`), if given.
    pub title: Option<String>,
}

/// Convert Markdown to Typst. `fallback_title` is the title of the chapter that takes text before the first heading
/// if the YAML front matter names none (as a rule the file name without extension).
pub fn to_typst(markdown: &str, fallback_title: &str) -> Converted {
    let text = markdown.strip_prefix('\u{feff}').unwrap_or(markdown).replace("\r\n", "\n");
    let options = Options::ENABLE_FOOTNOTES
        | Options::ENABLE_TABLES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_HEADING_ATTRIBUTES
        | Options::ENABLE_YAML_STYLE_METADATA_BLOCKS
        | Options::ENABLE_DEFINITION_LIST;
    let events: Vec<(Event<'_>, Range<usize>)> = Parser::new_ext(&text, options).into_offset_iter().collect();
    let nodes = tree(events);
    let mut w = Writer::new(&text);

    // YAML front matter: title and author become `#set document(…)`, everything else a note.
    let mut title = None;
    let mut author = None;
    for node in &nodes {
        if let Node::Elem { tag: Tag::MetadataBlock(_), range, children } = node {
            let (t, a) = w.front_matter(&plain_leaves(children), range.start);
            title = title.or(t);
            author = author.or(a);
        }
    }
    // Footnotes first (definitions can stand after the reference).
    let mut definitions = Vec::new();
    collect_footnotes(&nodes, &mut definitions);
    w.in_definition = true;
    for (label, children) in definitions {
        let content = w.blocks(children, true).join("\n\n");
        w.footnotes.entry(label).or_insert(content);
    }
    w.in_definition = false;

    let mut out = String::new();
    if title.is_some() || author.is_some() {
        let mut args = Vec::new();
        if let Some(t) = &title {
            args.push(format!("title: {}", lit(t)));
        }
        if let Some(a) = &author {
            args.push(format!("author: {}", lit(a)));
        }
        out.push_str(&format!("#set document({})\n\n", args.join(", ")));
    }
    let body: Vec<&Node<'_>> = nodes
        .iter()
        .filter(|n| !matches!(n, Node::Elem { tag: Tag::MetadataBlock(_) | Tag::FootnoteDefinition(_), .. }))
        .collect();
    let starts_with_heading = matches!(body.first(), Some(Node::Elem { tag: Tag::Heading { .. }, .. }));
    let blocks = w.blocks(body.iter().copied(), false);
    if !blocks.is_empty() && !starts_with_heading {
        let chapter = title.clone().unwrap_or_else(|| fallback_title.trim().to_owned());
        let chapter = if chapter.is_empty() { UNTITLED.to_owned() } else { chapter };
        let at = body.first().map_or(0, |n| n.range().start);
        w.warn("md-preamble-heading", tc("markdown_msg.md.preamble", &[("chapter", chapter.as_str().into())]), at);
        out.push_str(baruch_typst::heading_line(1, &chapter, None).trim_end());
        out.push_str("\n\n");
    }
    out.push_str(&blocks.join("\n\n"));
    if !out.ends_with('\n') {
        out.push('\n');
    }
    let unused: Vec<String> = w.footnotes.keys().filter(|l| !w.used_footnotes.contains_key(*l)).cloned().collect();
    for label in unused {
        w.warn("md-footnote-unused", tc("markdown_msg.md.footnote_unused", &[("label", label.into())]), 0);
    }
    w.finish_warnings();
    Converted { typst: out, warnings: w.warnings, title }
}

// ---- Tree from the events ------------------------------------------------------------------------

enum Node<'a> {
    Elem { tag: Tag<'a>, range: Range<usize>, children: Vec<Node<'a>> },
    Leaf { event: Event<'a>, range: Range<usize> },
}

impl Node<'_> {
    fn range(&self) -> &Range<usize> {
        match self {
            Node::Elem { range, .. } | Node::Leaf { range, .. } => range,
        }
    }

    /// Inline nodes (text, markup) as opposed to blocks.
    fn is_inline(&self) -> bool {
        match self {
            Node::Leaf { event, .. } => !matches!(event, Event::Rule | Event::Html(_) | Event::DisplayMath(_)),
            Node::Elem { tag, .. } => matches!(
                tag,
                Tag::Emphasis
                    | Tag::Strong
                    | Tag::Strikethrough
                    | Tag::Superscript
                    | Tag::Subscript
                    | Tag::Link { .. }
                    | Tag::Image { .. }
            ),
        }
    }
}

fn tree(events: Vec<(Event<'_>, Range<usize>)>) -> Vec<Node<'_>> {
    let mut stack: Vec<(Tag<'_>, Range<usize>, Vec<Node<'_>>)> = Vec::new();
    let mut top = Vec::new();
    fn push<'a>(stack: &mut [(Tag<'a>, Range<usize>, Vec<Node<'a>>)], top: &mut Vec<Node<'a>>, node: Node<'a>) {
        match stack.last_mut() {
            Some(parent) => parent.2.push(node),
            None => top.push(node),
        }
    }
    for (event, range) in events {
        match event {
            Event::Start(tag) => stack.push((tag, range, Vec::new())),
            Event::End(_) => {
                if let Some((tag, range, children)) = stack.pop() {
                    push(&mut stack, &mut top, Node::Elem { tag, range, children });
                }
            }
            event => push(&mut stack, &mut top, Node::Leaf { event, range }),
        }
    }
    while let Some((tag, range, children)) = stack.pop() {
        push(&mut stack, &mut top, Node::Elem { tag, range, children });
    }
    top
}

fn collect_footnotes<'n, 'a>(nodes: &'n [Node<'a>], into: &mut Vec<(String, &'n [Node<'a>])>) {
    for node in nodes {
        if let Node::Elem { tag, children, .. } = node {
            if let Tag::FootnoteDefinition(label) = tag {
                into.push((label.to_string(), children));
            } else {
                collect_footnotes(children, into);
            }
        }
    }
}

/// Text of all leaves (text, code) without markup.
fn plain_leaves(nodes: &[Node<'_>]) -> String {
    let mut out = String::new();
    for node in nodes {
        match node {
            Node::Leaf { event: Event::Text(t) | Event::Code(t), .. } => out.push_str(t),
            Node::Leaf { event: Event::SoftBreak | Event::HardBreak, .. } => out.push(' '),
            Node::Elem { children, .. } => out.push_str(&plain_leaves(children)),
            _ => {}
        }
    }
    out
}

// ---- Escaping --------------------------------------------------------------------------------------

/// Typst string literal.
pub(crate) fn lit(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Escape running text for Typst markup (without line starts, see [`escape_line_starts`]).
pub(crate) fn escape(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len() + 8);
    for (i, &c) in chars.iter().enumerate() {
        let next = chars.get(i + 1).copied();
        let special = matches!(c, '\\' | '#' | '*' | '_' | '$' | '@' | '<' | '[' | ']' | '`' | '~')
            || (c == '/' && matches!(next, Some('/' | '*')))
            || (c == '-' && matches!(next, Some('-' | '?')));
        if special {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// Escape line starts that would be structure in Typst: `=` (heading), `-`/`+` (list/enumeration), `/`
/// (definition list) and `12.` (enumeration with a number). Idempotent (every escape puts a backslash before it).
pub(crate) fn escape_line_starts(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 4);
    for (i, line) in text.split('\n').enumerate() {
        if i > 0 {
            out.push('\n');
        }
        let indent = line.len() - line.trim_start_matches([' ', '\t']).len();
        let (lead, rest) = line.split_at(indent);
        out.push_str(lead);
        if rest.starts_with(['=', '+', '-', '/']) {
            out.push('\\');
            out.push_str(rest);
            continue;
        }
        let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
        if digits > 0 && rest[digits..].starts_with('.') {
            out.push_str(&rest[..digits]);
            out.push('\\');
            out.push_str(&rest[digits..]);
        } else {
            out.push_str(rest);
        }
    }
    out
}

/// Would Typst attach `after` to an embedded expression directly before it (call, field, content block, `;`)?
fn needs_guard(after: &str) -> bool {
    let mut chars = after.chars();
    match chars.next() {
        Some('(' | '[' | ';') => true,
        Some('.') => chars.next().is_some_and(|c| c.is_alphabetic() || c == '_'),
        _ => false,
    }
}

/// Indent all lines except the first by `width` spaces (empty lines stay empty).
fn indent_rest(text: &str, width: usize) -> String {
    let pad = " ".repeat(width);
    let mut out = String::with_capacity(text.len() + 16);
    for (i, line) in text.split('\n').enumerate() {
        if i > 0 {
            out.push('\n');
            if !line.is_empty() {
                out.push_str(&pad);
            }
        }
        out.push_str(line);
    }
    out
}

/// Output of an inline sequence with a guard `;` after embedded expressions.
#[derive(Default)]
struct Inline {
    out: String,
    guard: bool,
}

impl Inline {
    /// Text (already escaped) or markup that is not an embedded expression.
    fn text(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        if self.guard && needs_guard(text) {
            self.out.push(';');
        }
        self.guard = false;
        self.out.push_str(text);
    }

    /// An embedded expression (`#emph[…]`, `#raw("…")` …); what comes after it possibly needs a `;`.
    /// Running text (already escaped): if it stands at the beginning of a line (beginning of the sequence or after a
    /// line end), a leading `= + - /` or `digits.` gets a backslash ([`escape_line_starts`]). Per piece of text instead
    /// of over the finished paragraph, so that lists in a footnote (embedded expression) stay lists.
    fn prose(&mut self, escaped: &str) {
        if self.out.is_empty() || self.out.ends_with('\n') {
            self.text(&escape_line_starts(escaped));
        } else {
            self.text(escaped);
        }
    }

    fn expr(&mut self, expr: &str) {
        self.text(expr);
        self.guard = true;
    }
}

// ---- Writer ------------------------------------------------------------------------------------------

struct Writer {
    /// Beginning of every line (byte) of the Markdown file.
    lines: Vec<usize>,
    warnings: Vec<ImportWarning>,
    dropped: usize,
    footnotes: HashMap<String, String>,
    used_footnotes: HashMap<String, usize>,
    /// Footnote definitions are being converted right now (references in them stay text).
    in_definition: bool,
}

impl Writer {
    fn new(text: &str) -> Writer {
        let mut lines = vec![0];
        lines.extend(text.match_indices('\n').map(|(i, _)| i + 1));
        Writer {
            lines,
            warnings: Vec::new(),
            dropped: 0,
            footnotes: HashMap::new(),
            used_footnotes: HashMap::new(),
            in_definition: false,
        }
    }

    fn line(&self, offset: usize) -> usize {
        self.lines.partition_point(|&start| start <= offset).max(1)
    }

    fn warn(&mut self, code: &str, message: String, offset: usize) {
        if self.warnings.len() >= MAX_WARNINGS {
            self.dropped += 1;
            return;
        }
        let line = Some(self.line(offset));
        self.warnings.push(ImportWarning { code: code.to_owned(), message, line });
    }

    fn finish_warnings(&mut self) {
        if self.dropped > 0 {
            self.warnings.push(ImportWarning {
                code: "md-warnings-truncated".into(),
                message: tc("markdown_msg.md.truncated", &[("count", self.dropped.into())]),
                line: None,
            });
        }
    }

    /// YAML front matter: `title:` and `author:` (outer quotation marks are dropped); other keys → note.
    fn front_matter(&mut self, text: &str, offset: usize) -> (Option<String>, Option<String>) {
        let (mut title, mut author) = (None, None);
        let mut others = Vec::new();
        for line in text.lines() {
            let Some((key, value)) = line.split_once(':') else { continue };
            if line.starts_with([' ', '\t', '-']) {
                continue;
            }
            let value = value.trim();
            let value = value
                .strip_prefix('"')
                .and_then(|v| v.strip_suffix('"'))
                .or_else(|| value.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')))
                .unwrap_or(value)
                .trim()
                .to_owned();
            match key.trim() {
                "title" if !value.is_empty() => title = Some(value),
                "author" if !value.is_empty() => author = Some(value),
                other => others.push(other.to_owned()),
            }
        }
        if !others.is_empty() {
            self.warn(
                "md-front-matter",
                tc("markdown_msg.md.front_matter", &[("keys", others.join(", ").into())]),
                offset,
            );
        }
        (title, author)
    }

    /// Convert blocks; consecutive inline nodes (content of a tight list item) are one paragraph.
    /// `nested`: inside a quote, list or footnote (headings are no outline there).
    fn blocks<'n, 'a: 'n>(&mut self, nodes: impl IntoIterator<Item = &'n Node<'a>>, nested: bool) -> Vec<String> {
        let mut out = Vec::new();
        let mut run: Vec<&Node<'_>> = Vec::new();
        for node in nodes {
            if node.is_inline() {
                run.push(node);
                continue;
            }
            if !run.is_empty() {
                out.push(self.paragraph(&std::mem::take(&mut run)));
            }
            if let Some(block) = self.block(node, nested) {
                out.push(block);
            }
        }
        if !run.is_empty() {
            out.push(self.paragraph(&run));
        }
        out.retain(|b| !b.trim().is_empty());
        out
    }

    fn block(&mut self, node: &Node<'_>, nested: bool) -> Option<String> {
        let (tag, range, children) = match node {
            Node::Leaf { event: Event::Rule, .. } => return Some("#line(length: 100%)".into()),
            Node::Leaf { event: Event::Html(html), range } => return Some(self.html_block(html, range.start)),
            Node::Leaf { range, .. } => {
                self.warn("md-unknown", tc("markdown_msg.md.unknown", &[]), range.start);
                return None;
            }
            Node::Elem { tag, range, children } => (tag, range, children),
        };
        match tag {
            Tag::Paragraph => Some(self.paragraph(&children.iter().collect::<Vec<_>>())),
            Tag::Heading { level, id, classes, attrs } => {
                if id.is_some() || !classes.is_empty() || !attrs.is_empty() {
                    self.warn(
                        "md-heading-attributes",
                        tc("markdown_msg.md.heading_attributes", &[("syntax", "{#…}".into())]),
                        range.start,
                    );
                }
                let (title, formatted) = self.heading_text(children, range.start);
                if formatted {
                    self.warn(
                        "md-heading-format",
                        tc("markdown_msg.md.heading_format", &[("title", title.as_str().into())]),
                        range.start,
                    );
                }
                if nested {
                    self.warn(
                        "md-heading-nested",
                        tc("markdown_msg.md.heading_nested", &[("title", title.as_str().into())]),
                        range.start,
                    );
                    return Some(format!("#strong[{}]", escape(&title)));
                }
                let depth = heading_depth(*level);
                if depth > 3 {
                    self.warn(
                        "md-heading-level",
                        tc(
                            "markdown_msg.md.heading_level",
                            &[("depth", depth.to_string().into()), ("title", title.as_str().into())],
                        ),
                        range.start,
                    );
                }
                let title = if title.trim().is_empty() {
                    self.warn(
                        "md-heading-empty",
                        tc("markdown_msg.md.heading_empty", &[("title", UNTITLED.into())]),
                        range.start,
                    );
                    UNTITLED.to_owned()
                } else {
                    title
                };
                Some(baruch_typst::heading_line(depth.min(3) as u8, &title, None).trim_end().to_owned())
            }
            Tag::BlockQuote(kind) => {
                if kind.is_some() {
                    self.warn("md-quote-kind", tc("markdown_msg.md.quote_kind", &[]), range.start);
                }
                let inner = self.blocks(children, true).join("\n\n");
                (!inner.trim().is_empty()).then(|| format!("#quote(block: true)[\n{inner}\n]"))
            }
            Tag::CodeBlock(kind) => Some(self.code_block(kind, children, range.start)),
            Tag::HtmlBlock => {
                let html: String = children
                    .iter()
                    .filter_map(|c| match c {
                        Node::Leaf { event: Event::Html(h) | Event::Text(h), .. } => Some(h.to_string()),
                        _ => None,
                    })
                    .collect();
                Some(self.html_block(&html, range.start))
            }
            Tag::List(start) => Some(self.list(*start, children)),
            Tag::DefinitionList => Some(self.definition_list(children)),
            Tag::Table(aligns) => Some(self.table(aligns, children)),
            Tag::FootnoteDefinition(_) | Tag::MetadataBlock(_) => None,
            _ => {
                self.warn("md-unknown", tc("markdown_msg.md.unknown_block", &[]), range.start);
                Some(self.paragraph(&children.iter().collect::<Vec<_>>()))
            }
        }
    }

    /// Title of a heading as text and whether markup was dropped.
    fn heading_text(&mut self, children: &[Node<'_>], offset: usize) -> (String, bool) {
        let mut formatted = false;
        for child in children {
            match child {
                Node::Leaf { event: Event::Text(_) | Event::SoftBreak, .. } => {}
                Node::Leaf { event: Event::FootnoteReference(label), .. } => {
                    self.warn(
                        "md-heading-footnote",
                        tc("markdown_msg.md.heading_footnote", &[("label", label.as_ref().into())]),
                        offset,
                    );
                }
                _ => formatted = true,
            }
        }
        let text: String = children
            .iter()
            .filter(|c| !matches!(c, Node::Leaf { event: Event::FootnoteReference(_), .. }))
            .map(|c| plain_leaves(std::slice::from_ref(c)))
            .collect();
        (text.split_whitespace().collect::<Vec<_>>().join(" "), formatted)
    }

    /// Paragraph from inline nodes; a paragraph consisting of just one image becomes an image block.
    fn paragraph(&mut self, nodes: &[&Node<'_>]) -> String {
        let visible: Vec<&&Node<'_>> = nodes
            .iter()
            .filter(|n| !matches!(n, Node::Leaf { event: Event::Text(t), .. } if t.trim().is_empty()))
            .collect();
        if let [Node::Elem { tag: Tag::Image { dest_url, title, .. }, range, children }] = visible.as_slice() {
            return self.image(dest_url, title, children, range.start, true);
        }
        let mut inline = Inline::default();
        for node in nodes {
            self.inline(node, &mut inline);
        }
        inline.out.trim_matches([' ', '\n']).to_owned()
    }

    fn inlines(&mut self, nodes: &[Node<'_>]) -> String {
        let mut inline = Inline::default();
        for node in nodes {
            self.inline(node, &mut inline);
        }
        inline.out
    }

    fn inline(&mut self, node: &Node<'_>, out: &mut Inline) {
        match node {
            Node::Leaf { event, range } => match event {
                Event::Text(t) => out.prose(&escape(t)),
                Event::Code(t) => out.expr(&format!("#raw({})", lit(t))),
                Event::SoftBreak => out.text("\n"),
                Event::HardBreak => out.text("\\\n"),
                Event::FootnoteReference(label) => self.footnote(label, range.start, out),
                Event::InlineHtml(html) => self.inline_html(html, range.start, out),
                Event::InlineMath(m) | Event::DisplayMath(m) => out.text(&escape(m)),
                Event::Html(html) => self.inline_html(html, range.start, out),
                Event::TaskListMarker(done) => out.text(if *done { "☒ " } else { "☐ " }),
                Event::Rule | Event::Start(_) | Event::End(_) => {}
            },
            Node::Elem { tag, range, children } => match tag {
                Tag::Emphasis => {
                    let inner = self.inlines(children);
                    out.expr(&format!("#emph[{inner}]"));
                }
                Tag::Strong => {
                    let inner = self.inlines(children);
                    out.expr(&format!("#strong[{inner}]"));
                }
                Tag::Strikethrough => {
                    let inner = self.inlines(children);
                    out.expr(&format!("#strike[{inner}]"));
                }
                Tag::Link { link_type, dest_url, .. } => self.link(*link_type, dest_url, children, range.start, out),
                Tag::Image { dest_url, title, .. } => {
                    let image = self.image(dest_url, title, children, range.start, false);
                    out.expr(&image);
                }
                _ => {
                    self.warn("md-unknown", tc("markdown_msg.md.unknown_inline", &[]), range.start);
                    let inner = self.inlines(children);
                    out.text(&inner);
                }
            },
        }
    }

    fn footnote(&mut self, label: &str, offset: usize, out: &mut Inline) {
        if self.in_definition {
            self.warn("md-footnote-nested", tc("markdown_msg.md.footnote_nested", &[("label", label.into())]), offset);
            out.text(&escape(&format!("[^{label}]")));
            return;
        }
        let Some(content) = self.footnotes.get(label).cloned() else {
            self.warn(
                "md-footnote-missing",
                tc("markdown_msg.md.footnote_missing", &[("label", label.into())]),
                offset,
            );
            out.text(&escape(&format!("[^{label}]")));
            return;
        };
        let uses = self.used_footnotes.entry(label.to_owned()).or_insert(0);
        *uses += 1;
        if *uses == 2 {
            self.warn(
                "md-footnote-repeated",
                tc("markdown_msg.md.footnote_repeated", &[("label", label.into())]),
                offset,
            );
        }
        out.expr(&format!("#footnote[{content}]"));
    }

    fn inline_html(&mut self, html: &str, offset: usize, out: &mut Inline) {
        let tag = html.trim().to_ascii_lowercase();
        if matches!(tag.as_str(), "<br>" | "<br/>" | "<br />") {
            out.text("\\ ");
        } else if tag.starts_with("<!--") {
            self.warn("md-html-comment", tc("markdown_msg.md.html_comment", &[]), offset);
        } else {
            let shown: String = html.trim().chars().take(40).collect();
            self.warn("md-html", tc("markdown_msg.md.html", &[("shown", shown.into())]), offset);
        }
    }

    /// HTML block as a Typst comment (nothing is lost, nothing is typeset); except for pure comments a note.
    fn html_block(&mut self, html: &str, offset: usize) -> String {
        let trimmed = html.trim();
        if !(trimmed.starts_with("<!--") && trimmed.ends_with("-->")) {
            let shown: String = trimmed.chars().take(40).collect();
            self.warn("md-html-block", tc("markdown_msg.md.html_block", &[("shown", shown.into())]), offset);
        }
        trimmed.lines().map(|l| format!("// {l}").trim_end().to_owned()).collect::<Vec<_>>().join("\n")
    }

    fn code_block(&mut self, kind: &CodeBlockKind<'_>, children: &[Node<'_>], offset: usize) -> String {
        let code: String = children
            .iter()
            .filter_map(|c| match c {
                Node::Leaf { event: Event::Text(t), .. } => Some(t.to_string()),
                _ => None,
            })
            .collect();
        let lang = match kind {
            CodeBlockKind::Fenced(info) => {
                let word = info.split_whitespace().next().unwrap_or("");
                if word.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '+' | '-')) {
                    word.to_owned()
                } else {
                    self.warn("md-code-lang", tc("markdown_msg.md.code_lang", &[("word", word.into())]), offset);
                    String::new()
                }
            }
            CodeBlockKind::Indented => String::new(),
        };
        let longest = code.split(|c| c != '`').map(str::len).max().unwrap_or(0);
        let fence = "`".repeat((longest + 1).max(3));
        let mut out = format!("{fence}{lang}\n{code}");
        if !out.ends_with('\n') {
            out.push('\n');
        }
        out.push_str(&fence);
        out
    }

    fn list(&mut self, start: Option<u64>, items: &[Node<'_>]) -> String {
        let loose = items.iter().any(|item| match item {
            Node::Elem { children, .. } => children.iter().any(|c| matches!(c, Node::Elem { tag: Tag::Paragraph, .. })),
            Node::Leaf { .. } => false,
        });
        let mut rendered = Vec::new();
        for (i, item) in items.iter().enumerate() {
            let Node::Elem { children, .. } = item else { continue };
            let marker = match start {
                Some(n) => format!("{}. ", n + i as u64),
                None => "- ".to_owned(),
            };
            let blocks = self.blocks(children, true);
            let content = blocks.join(if loose { "\n\n" } else { "\n" });
            rendered.push(format!("{marker}{}", indent_rest(&content, marker.len())).trim_end().to_owned());
        }
        rendered.join(if loose { "\n\n" } else { "\n" })
    }

    fn definition_list(&mut self, children: &[Node<'_>]) -> String {
        let mut out: Vec<String> = Vec::new();
        let mut term: Option<(String, usize, usize)> = None; // (term, line, number of explanations)
        for child in children {
            let Node::Elem { tag, range, children } = child else { continue };
            match tag {
                Tag::DefinitionListTitle => {
                    if let Some((t, _, 0)) = &term {
                        out.push(format!("/ {t}:"));
                    }
                    let text = self.inlines(children).replace(':', "\\:").replace('\n', " ");
                    term = Some((text.trim().to_owned(), range.start, 0));
                }
                Tag::DefinitionListDefinition => {
                    let (t, at, n) = term.clone().unwrap_or_default();
                    if n == 1 {
                        self.warn(
                            "md-definition-multiple",
                            tc("markdown_msg.md.definition_multiple", &[("term", t.replace("\\:", ":").into())]),
                            at,
                        );
                    }
                    term = Some((t.clone(), at, n + 1));
                    let content = self.blocks(children, true).join("\n\n");
                    let line = format!("/ {t}: {content}");
                    out.push(indent_rest(&line, 2).trim_end().to_owned());
                }
                _ => {}
            }
        }
        if let Some((t, _, 0)) = &term {
            out.push(format!("/ {t}:"));
        }
        out.join("\n")
    }

    fn table(&mut self, aligns: &[Alignment], children: &[Node<'_>]) -> String {
        let cells = |w: &mut Writer, row: &[Node<'_>]| -> Vec<String> {
            row.iter()
                .filter_map(|cell| match cell {
                    Node::Elem { tag: Tag::TableCell, children, .. } => {
                        Some(format!("[{}]", w.inlines(children).replace('\n', " ").trim()))
                    }
                    _ => None,
                })
                .collect()
        };
        let mut lines = vec![format!("  columns: {},", aligns.len().max(1))];
        if aligns.iter().any(|a| *a != Alignment::None) {
            let words: Vec<&str> = aligns
                .iter()
                .map(|a| match a {
                    Alignment::None => "auto",
                    Alignment::Left => "left",
                    Alignment::Center => "center",
                    Alignment::Right => "right",
                })
                .collect();
            lines.push(format!("  align: ({},),", words.join(", ")));
        }
        for child in children {
            let Node::Elem { tag, children: row, .. } = child else { continue };
            match tag {
                Tag::TableHead => {
                    let head = cells(self, row);
                    if !head.is_empty() {
                        lines.push(format!("  table.header({}),", head.join(", ")));
                    }
                }
                Tag::TableRow => {
                    let body = cells(self, row);
                    if !body.is_empty() {
                        lines.push(format!("  {},", body.join(", ")));
                    }
                }
                _ => {}
            }
        }
        format!("#table(\n{}\n)", lines.join("\n"))
    }

    fn link(&mut self, link_type: LinkType, dest: &str, children: &[Node<'_>], offset: usize, out: &mut Inline) {
        let text = self.inlines(children);
        let dest = dest.trim();
        let target = if link_type == LinkType::Email {
            Some(format!("mailto:{dest}"))
        } else if has_scheme(dest, &["http", "https", "mailto", "ftp", "tel"]) {
            Some(dest.to_owned())
        } else {
            None
        };
        match target {
            Some(url) if link_type == LinkType::Autolink || link_type == LinkType::Email => {
                if text == escape(dest) || text.is_empty() {
                    out.expr(&format!("#link({})", lit(&url)));
                } else {
                    out.expr(&format!("#link({})[{text}]", lit(&url)));
                }
            }
            Some(url) if text.trim().is_empty() => out.expr(&format!("#link({})", lit(&url))),
            Some(url) => out.expr(&format!("#link({})[{text}]", lit(&url))),
            None => {
                let why = if dest.starts_with('#') {
                    tc("markdown_msg.md.link_anchor", &[])
                } else if dest.contains(':') {
                    tc("markdown_msg.md.link_scheme", &[])
                } else {
                    tc("markdown_msg.md.link_file", &[])
                };
                let shown: String = dest.chars().take(60).collect();
                self.warn(
                    "md-link-local",
                    tc("markdown_msg.md.link_local", &[("shown", shown.into()), ("why", why.into())]),
                    offset,
                );
                out.text(&text);
            }
        }
    }

    /// Image: local path → `#image("…", alt: "…")` (as a block with the title as caption in `#figure`, in running text
    /// in `#box`); address or `data:` → note, instead of the image a link or the alternative text.
    fn image(&mut self, dest: &str, title: &str, children: &[Node<'_>], offset: usize, block: bool) -> String {
        let alt = plain_leaves(children).split_whitespace().collect::<Vec<_>>().join(" ");
        let dest = dest.trim();
        if dest.starts_with("data:") {
            self.warn("md-image-data", tc("markdown_msg.md.image_data", &[]), offset);
            return escape(&format!("[Bild: {alt}]"));
        }
        if dest.contains("://") {
            let shown: String = dest.chars().take(60).collect();
            self.warn(
                "md-image-remote",
                tc("markdown_msg.md.image_remote", &[("shown", shown.as_str().into())]),
                offset,
            );
            let label = escape(&format!("Bild: {}", if alt.is_empty() { dest } else { &alt }));
            return format!("#link({})[{label}]", lit(dest));
        }
        let path = percent_decode(dest).unwrap_or_else(|| {
            self.warn("md-image-path", tc("markdown_msg.md.image_path", &[("path", dest.into())]), offset);
            dest.to_owned()
        });
        let alt_arg = if alt.is_empty() { String::new() } else { format!(", alt: {}", lit(&alt)) };
        let image = format!("image({}{alt_arg})", lit(&path));
        match (block, title.trim()) {
            (true, "") => format!("#{image}"),
            (true, caption) => format!("#figure({image}, caption: [{}])", escape(caption)),
            (false, caption) => {
                if !caption.is_empty() {
                    self.warn(
                        "md-image-title",
                        tc("markdown_msg.md.image_title", &[("caption", caption.into())]),
                        offset,
                    );
                }
                format!("#box({image})")
            }
        }
    }
}

fn heading_depth(level: HeadingLevel) -> usize {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

/// Does `dest` begin with one of the schemes (`http:` …, regardless of case)?
fn has_scheme(dest: &str, schemes: &[&str]) -> bool {
    let Some((scheme, _)) = dest.split_once(':') else { return false };
    schemes.iter().any(|s| scheme.eq_ignore_ascii_case(s))
}

/// Resolve `%20` and other % sequences (UTF-8); invalid → `None`.
fn percent_decode(text: &str) -> Option<String> {
    if !text.contains('%') {
        return Some(text.to_owned());
    }
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = text.get(i + 1..i + 3)?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn typst(md: &str) -> String {
        to_typst(md, "Datei").typst
    }

    fn codes(md: &str) -> Vec<String> {
        to_typst(md, "Datei").warnings.into_iter().map(|w| w.code).collect()
    }

    #[test]
    fn headings_become_the_outline_and_deep_levels_are_capped_with_a_warning() {
        let out = to_typst("# Eins\n\n## Zwei\n\n### Drei\n\n#### Vier\n", "x");
        assert_eq!(
            out.typst, "= Eins\n\n== Zwei\n\n=== Drei\n\n=== Vier\n",
            "#### wird === (Baruch kennt drei Ebenen)"
        );
        assert_eq!(out.warnings.len(), 1, "genau ein Hinweis für die begrenzte Ebene: {:?}", out.warnings);
        assert_eq!(
            (out.warnings[0].code.as_str(), out.warnings[0].line),
            ("md-heading-level", Some(7)),
            "Code und Zeile der ####-Überschrift"
        );
    }

    #[test]
    fn typst_special_characters_stay_literal_text() {
        let out = typst("# K\n\nPreis $5 #1 *a* _b_ @c 1<2 `e` \\f ~g [h] //i --j\n\n\\= keine Überschrift\n");
        assert!(
            out.contains(r"Preis \$5 \#1 #emph[a] #emph[b] \@c 1\<2 #raw("),
            "Dollar, Raute, At und Kleiner-als maskiert, *a*/_b_ als Funktion: {out}"
        );
        assert!(
            out.contains(r"\\f \~g \[h\] \//i \--j"),
            "Backslash, Tilde, Klammern; vor Kommentar und Gedankenstrich genügt ein Backslash am ersten Zeichen: {out}"
        );
        assert!(out.contains("\n\\= keine Überschrift\n"), "= am Zeilenanfang bleibt Text: {out}");
        // Running text in Markdown (no list marker with a space, „12.“ interrupts no paragraph), structure in Typst.
        let lines = typst("# K\n\nZeile\n=zwei\n+drei\n-vier\n/ fünf\n12. sechs\n");
        assert!(
            lines.contains("Zeile\n\\=zwei\n\\+drei\n\\-vier\n\\/ fünf\n12\\. sechs"),
            "jede Zeile, die in Typst Struktur wäre, ist maskiert: {lines}"
        );
    }

    #[test]
    fn a_function_directly_followed_by_a_parenthesis_gets_a_guard() {
        let out = typst("# K\n\n*kursiv*(Klammer) und `x`.feld\n");
        assert!(out.contains("#emph[kursiv];(Klammer)"), "`;` trennt die Klammer vom Aufruf: {out}");
        assert!(out.contains("#raw(\"x\");.feld"), "`;` vor .feld (sonst Feldzugriff): {out}");
    }

    #[test]
    fn lists_quotes_code_links_footnotes_and_images_are_mapped() {
        let md = "# K\n\n- a\n  - b\n- c\n\n3. drei\n4. vier\n\n> Zitat\n\n```rust\nlet x = 1;\n```\n\n\
                  [Seite](https://example.org) und [lokal](andere.md)[^n]\n\n[^n]: Die Fußnote.\n\n\
                  ![Ein Haus](bilder/haus%20eins.png \"Das Haus\")\n";
        let out = to_typst(md, "x");
        let t = &out.typst;
        assert!(t.contains("- a\n  - b\n- c"), "verschachtelte Liste mit Einrückung: {t}");
        assert!(t.contains("3. drei\n4. vier"), "Nummern der Aufzählung bleiben: {t}");
        assert!(t.contains("#quote(block: true)[\nZitat\n]"), "Zitat als Blockzitat: {t}");
        assert!(t.contains("```rust\nlet x = 1;\n```"), "Codeblock mit Sprache: {t}");
        assert!(t.contains("#link(\"https://example.org\")[Seite]"), "Link mit Text: {t}");
        assert!(t.contains("lokal#footnote[Die Fußnote.]"), "lokaler Link nur als Text, Fußnote an der Stelle: {t}");
        assert!(
            t.contains("#figure(image(\"bilder/haus eins.png\", alt: \"Ein Haus\"), caption: [Das Haus])"),
            "Bild mit Titel als Abbildung, %20 aufgelöst: {t}"
        );
        assert!(
            out.warnings.iter().any(|w| w.code == "md-link-local" && w.line == Some(16)),
            "der lokale Link ist ein Hinweis mit Zeile: {:?}",
            out.warnings
        );
    }

    #[test]
    fn text_before_the_first_heading_gets_a_chapter_and_front_matter_sets_the_document() {
        let out = to_typst("---\ntitle: \"Mein Heft\"\nauthor: Ada\ndate: 2026\n---\n\nVorwort.\n\n# Eins\n", "heft");
        assert!(
            out.typst.starts_with(
                "#set document(title: \"Mein Heft\", author: \"Ada\")\n\n= Mein Heft\n\nVorwort.\n\n= Eins\n"
            ),
            "YAML-Kopf wird #set document, Vorwort bekommt ein Kapitel: {}",
            out.typst
        );
        assert_eq!(out.title.as_deref(), Some("Mein Heft"), "Titel aus dem YAML-Kopf");
        let codes: Vec<&str> = out.warnings.iter().map(|w| w.code.as_str()).collect();
        assert!(
            codes.contains(&"md-front-matter") && codes.contains(&"md-preamble-heading"),
            "beide Hinweise: {codes:?}"
        );
    }

    #[test]
    fn html_and_unknown_footnotes_are_reported_not_dropped_silently() {
        assert!(codes("# K\n\n<div>x</div>\n").contains(&"md-html-block".to_owned()), "HTML-Block meldet sich");
        assert!(codes("# K\n\nText <span>a</span>\n").contains(&"md-html".to_owned()), "HTML im Text meldet sich");
        let missing = typst("# K\n\nText[^fehlt]\n");
        assert!(
            missing.contains("Text\\[^fehlt\\]"),
            "Verweis auf eine fehlende Fußnote bleibt wörtlich Text: {missing}"
        );
        let html = typst("# K\n\n<div>x</div>\n");
        assert!(html.contains("// <div>x</div>"), "HTML-Block als Kommentar erhalten: {html}");
        let br = typst("# K\n\na<br>b\n");
        assert!(br.contains("a\\ b"), "<br> wird ein Zeilenumbruch: {br}");
    }

    #[test]
    fn remote_images_and_definition_lists_and_tables() {
        let out = to_typst("# K\n\n![x](https://example.org/a.png)\n", "x");
        assert!(
            out.typst.contains("#link(\"https://example.org/a.png\")[Bild: x]"),
            "Netzbild als Link: {}",
            out.typst
        );
        assert!(out.warnings.iter().any(|w| w.code == "md-image-remote"), "Hinweis: {:?}", out.warnings);
        let terms = typst("# K\n\nApfel\n: Eine Frucht\n\nZeit: Uhr\n: Messung\n");
        assert!(terms.contains("/ Apfel: Eine Frucht"), "Begriffsliste: {terms}");
        assert!(terms.contains("/ Zeit\\: Uhr: Messung"), "Doppelpunkt im Begriff maskiert: {terms}");
        let table = typst("# K\n\n| a | b |\n|:--|--:|\n| 1 | 2 |\n");
        assert!(
            table.contains(
                "#table(\n  columns: 2,\n  align: (left, right,),\n  table.header([a], [b]),\n  [1], [2],\n)"
            ),
            "Tabelle mit Ausrichtung und Kopf: {table}"
        );
    }

    #[test]
    fn a_list_inside_a_footnote_stays_a_list() {
        let out = typst("# K\n\nText[^n] weiter.\n\n[^n]: Vorne\n\n    - eins\n    - zwei\n");
        assert!(
            out.contains("Text#footnote[Vorne\n\n- eins\n- zwei] weiter."),
            "Zeilenanfänge der Fußnote sind Struktur, nicht maskiert (nur Fließtext wird maskiert): {out}"
        );
    }

    #[test]
    fn line_starts_are_escaped_idempotently() {
        let once = escape_line_starts("= a\n12. b\n  - c");
        assert_eq!(once, "\\= a\n12\\. b\n  \\- c", "Zeilenanfänge maskiert");
        assert_eq!(escape_line_starts(&once), once, "zweimal angewandt ändert nichts");
    }
}
