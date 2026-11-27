// SPDX-License-Identifier: AGPL-3.0-or-later
//! [`DocxDocument`] → Typst per section (task docx_import, contract Round 32 §2).
//!
//! The forms are those of the Markdown import (Round 21b), so that the WYSIWYG editor recognises them:
//! - Text stays verbatim: `\ # * _ $ @ < [ ] ` ~` get a backslash, as do `/` before `/` or `*` and `-` before
//!   `-` or `?`; at the beginning of a line also `= + - /` and the digit sequence before a full stop.
//! - Markup as a function: `#strong[…]`, `#emph[…]`, `#underline[…]`, `#strike[…]`, `#super[…]`, `#sub[…]`,
//!   `#smallcaps[…]`, `#link("…")[…]`, `#footnote[…]`, `#quote[…]`; if a character follows that Typst would
//!   attach to the expression, a `;` is put before it.
//! - Blocks: `#quote(block: true, attribution: […])[…]`, lists with `-`/`+` (two spaces per level),
//!   `#table(columns: n, table.header(…), …)` (a Typst island in the editor), `#figure(image("…", alt: "…"),
//!   caption: […])` or `#image("…", alt: "…")`, `#pagebreak()`; Typst islands verbatim.
//! - Outline: part and heading 1–3 start a section; heading 4–6 stays bold text in the section (warning
//!   `docx_ueberschrift_tief`). Text before the first heading becomes a first section of its own with an
//!   **empty title** (warning `docx_text_vor_ueberschrift`); the caller gives it the title of the work.
//!
//! Further warnings: `docx_tabelle_vereinfacht` (rows of unequal length), `docx_ueberschrift_vereinfacht`
//! (footnote or image in a heading), `docx_bild_fehlt` (image reference without image).

use crate::resolve::{has_preamble, is_outline, plain};
use crate::{Block, DocxDocument, DocxWarning, Inline, Marks};

/// Anchor of a comment in the section: the commented wording.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommentAnchor {
    pub comment: u32,
    pub quote: String,
}

/// A section of the work: `level` 0 = part, 1–3 = outline; `body` without the heading line.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TypstSection {
    pub level: u8,
    pub numbered: bool,
    pub title: String,
    pub body: String,
    pub anchors: Vec<CommentAnchor>,
}

/// Result: sections in reading order and warnings.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TypstOut {
    pub sections: Vec<TypstSection>,
    pub warnings: Vec<DocxWarning>,
}

/// Escape running text for Typst markup (without line beginnings).
fn escape(text: &str) -> String {
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
        out.push(if matches!(c, '\n' | '\r' | '\t') { ' ' } else { c });
    }
    out
}

/// String as a Typst literal.
fn literal(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' | '\r' | '\t' => out.push(' '),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// A sequence of inline content as Typst.
#[derive(Default)]
struct Line {
    text: String,
    /// The last contribution was an expression (`#f[…]`) to which Typst could attach what follows.
    after_expr: bool,
}

impl Line {
    fn at_line_start(&self) -> bool {
        self.text.is_empty() || self.text.ends_with('\n')
    }

    fn text(&mut self, raw: &str) {
        let mut escaped = escape(raw);
        if self.at_line_start() {
            escaped = escaped.trim_start().to_owned();
            let digits = escaped.bytes().take_while(u8::is_ascii_digit).count();
            if escaped.starts_with(['=', '+', '-', '/']) {
                escaped.insert(0, '\\');
            } else if digits > 0 && escaped[digits..].starts_with('.') {
                escaped.insert(digits, '\\');
            }
        }
        if escaped.is_empty() {
            return;
        }
        let mut chars = escaped.chars();
        let (first, second) = (chars.next(), chars.next());
        if self.after_expr
            && (matches!(first, Some('(' | ';' | '['))
                || (first == Some('.') && second.is_some_and(char::is_alphabetic)))
        {
            self.text.push(';');
        }
        self.text.push_str(&escaped);
        self.after_expr = false;
    }

    fn expr(&mut self, code: &str) {
        self.text.push_str(code);
        self.after_expr = true;
    }
}

fn marked(text: &str, marks: Marks) -> String {
    let mut out = escape(text);
    let wraps = [
        (marks.small_caps, "smallcaps"),
        (marks.subscript, "sub"),
        (marks.superscript, "super"),
        (marks.strike, "strike"),
        (marks.underline, "underline"),
        (marks.italic, "emph"),
        (marks.bold, "strong"),
    ];
    for (_, function) in wraps.iter().filter(|(set, _)| *set) {
        out = format!("#{function}[{out}]");
    }
    out
}

struct Writer<'a> {
    document: &'a DocxDocument,
    warnings: Vec<DocxWarning>,
    /// Open comment ranges: identifier and wording collected so far.
    open: Vec<(u32, String)>,
    anchors: Vec<CommentAnchor>,
}

impl Writer<'_> {
    fn warn(&mut self, code: &str, message: &str) {
        if !self.warnings.iter().any(|w| w.code == code) {
            self.warnings.push(DocxWarning { code: code.to_owned(), message: message.to_owned() });
        }
    }

    fn heard(&mut self, text: &str) {
        for (_, quote) in &mut self.open {
            quote.push_str(text);
        }
    }

    fn close(&mut self, at: usize) {
        let (comment, quote) = self.open.remove(at);
        let quote = quote.split_whitespace().collect::<Vec<_>>().join(" ");
        self.anchors.push(CommentAnchor { comment, quote });
    }

    fn image(&mut self, image: usize, alt: &str) -> Option<String> {
        let Some(found) = self.document.images.get(image) else {
            self.warn("docx_bild_fehlt", "Ein Bildverweis zeigt auf kein Bild der Datei; ausgelassen.");
            return None;
        };
        let alt = alt.trim();
        Some(if alt.is_empty() {
            format!("image({})", literal(&found.path))
        } else {
            format!("image({}, alt: {})", literal(&found.path), literal(alt))
        })
    }

    fn inlines(&mut self, content: &[Inline]) -> String {
        let mut line = Line::default();
        self.inlines_into(content, &mut line);
        line.text.trim_end().to_owned()
    }

    fn inlines_into(&mut self, content: &[Inline], line: &mut Line) {
        for inline in content {
            match inline {
                Inline::Text { text, marks } => {
                    self.heard(text);
                    if *marks == Marks::default() {
                        line.text(text);
                    } else {
                        let core = text.trim();
                        if core.is_empty() {
                            line.text(text);
                            continue;
                        }
                        let lead = &text[..text.len() - text.trim_start().len()];
                        let trail = &text[text.trim_end().len()..];
                        line.text(lead);
                        line.expr(&marked(core, *marks));
                        line.text(trail);
                    }
                }
                Inline::LineBreak => {
                    self.heard(" ");
                    if !line.at_line_start() {
                        let trimmed = line.text.trim_end().len();
                        line.text.truncate(trimmed);
                        line.text.push_str(" \\\n");
                        line.after_expr = false;
                    }
                }
                Inline::Footnote { content } => {
                    // The wording of the footnote does not belong to the commented place.
                    let open = std::mem::take(&mut self.open);
                    let inner = self.inlines(content);
                    self.open = open;
                    line.expr(&format!("#footnote[{inner}]"));
                }
                Inline::Quote { content } => {
                    let inner = self.inlines(content);
                    line.expr(&format!("#quote[{inner}]"));
                }
                Inline::Link { url, content } => {
                    let inner = self.inlines(content);
                    if inner.is_empty() {
                        line.expr(&format!("#link({})", literal(url)));
                    } else {
                        line.expr(&format!("#link({})[{inner}]", literal(url)));
                    }
                }
                Inline::Image { image, alt } => {
                    if let Some(code) = self.image(*image, alt) {
                        line.expr(&format!("#{code}"));
                    }
                }
                Inline::Typst { raw, shown } => {
                    if raw.is_empty() {
                        self.inlines_into(shown, line);
                    } else {
                        line.text.push_str(raw);
                        line.after_expr = true;
                    }
                }
                // Only a resolved document has none; otherwise the content applies (final state), nothing is lost.
                Inline::Tracked { content, .. } => self.inlines_into(content, line),
                Inline::CommentStart { id } => self.open.push((*id, String::new())),
                Inline::CommentEnd { id } => {
                    if let Some(at) = self.open.iter().position(|(open, _)| open == id) {
                        self.close(at);
                    }
                }
            }
        }
    }

    fn cell(&mut self, blocks: &[Block]) -> String {
        let parts: Vec<String> = blocks.iter().filter_map(|b| self.block(b)).collect();
        match (parts.len(), blocks.first()) {
            (0, _) => "[]".to_owned(),
            (1, Some(Block::Paragraph { .. })) => format!("[{}]", parts[0]),
            _ => format!("[\n{}\n]", parts.join("\n\n")),
        }
    }

    /// A block as Typst (without an empty line at the end); `None` if there is nothing to write.
    fn block(&mut self, block: &Block) -> Option<String> {
        self.heard(" ");
        let text = match block {
            Block::Paragraph { content, .. } => self.inlines(content),
            Block::Part { content, .. } => {
                let inner = self.inlines(content);
                if inner.is_empty() { inner } else { format!("#strong[{inner}]") }
            }
            Block::Heading { level, content, .. } => {
                if *level > crate::ooxml::MAX_OUTLINE_LEVEL {
                    self.warn(
                        "docx_ueberschrift_tief",
                        "Überschriften der Ebenen 4 bis 6 gehören nicht zur Gliederung; sie stehen als fetter Text \
                         im Abschnitt.",
                    );
                }
                let inner = self.inlines(content);
                if inner.is_empty() { inner } else { format!("#strong[{inner}]") }
            }
            Block::Quote { blocks, attribution } => {
                let parts: Vec<String> = blocks.iter().filter_map(|b| self.block(b)).collect();
                let source = self.inlines(attribution);
                if parts.is_empty() {
                    return None;
                }
                let args = if source.is_empty() {
                    "block: true".to_owned()
                } else {
                    format!("block: true, attribution: [{source}]")
                };
                format!("#quote({args})[\n{}\n]", parts.join("\n\n"))
            }
            Block::List { ordered, start, items } => {
                let mut lines = Vec::new();
                // If the enumeration does not start at 1, every item carries its number ("3.", "4." …; nested from
                // 1): Typst would also understand "3." followed by "+", but the typeset view editor (`baruch-wysiwyg`)
                // would read that as two lists – and the Word export would then write them separately.
                let mut counters: Vec<u64> = Vec::new();
                for item in items {
                    let depth = usize::from(item.depth.min(8));
                    let indent = "  ".repeat(depth);
                    let marker = if !*ordered {
                        "-".to_owned()
                    } else if *start != 1 {
                        counters.truncate(depth + 1);
                        while counters.len() <= depth {
                            counters.push(if counters.is_empty() { *start } else { 1 });
                        }
                        let number = counters[depth];
                        counters[depth] = number.saturating_add(1);
                        format!("{number}.")
                    } else {
                        "+".to_owned()
                    };
                    let content = self.inlines(&item.content).replace('\n', &format!("\n{indent}  "));
                    lines.push(format!("{indent}{marker} {content}"));
                    self.heard(" ");
                }
                lines.join("\n")
            }
            Block::Table { header_rows, rows } => {
                let columns = rows.iter().map(Vec::len).max().unwrap_or(0);
                if columns == 0 {
                    return None;
                }
                if rows.iter().any(|row| row.len() != columns) {
                    self.warn(
                        "docx_tabelle_vereinfacht",
                        "Eine Tabelle hat ungleich lange Zeilen; fehlende Zellen sind leer aufgefüllt.",
                    );
                }
                let mut lines = vec!["#table(".to_owned(), format!("  columns: {columns},")];
                for (index, row) in rows.iter().enumerate() {
                    let mut cells: Vec<String> = row.iter().map(|cell| self.cell(cell)).collect();
                    cells.resize(columns, "[]".to_owned());
                    let joined = cells.join(", ");
                    if index < *header_rows {
                        lines.push(format!("  table.header({joined}),"));
                    } else {
                        lines.push(format!("  {joined},"));
                    }
                }
                lines.push(")".to_owned());
                lines.join("\n")
            }
            Block::Figure { image, alt, caption, raw } => {
                if !raw.is_empty() {
                    raw.clone()
                } else {
                    let caption = self.inlines(caption);
                    let image = self.image(*image, alt)?;
                    if caption.is_empty() {
                        format!("#{image}")
                    } else {
                        format!("#figure({image}, caption: [{caption}])")
                    }
                }
            }
            Block::Typst { raw, shown } => {
                if raw.is_empty() {
                    let parts: Vec<String> = shown.iter().filter_map(|b| self.block(b)).collect();
                    parts.join("\n\n")
                } else {
                    raw.clone()
                }
            }
            Block::PageBreak => "#pagebreak()".to_owned(),
        };
        (!text.trim().is_empty()).then_some(text)
    }
}

/// Writes a resolved document (without [`crate::Inline::Tracked`]) as Typst per section.
pub fn to_typst(document: &DocxDocument) -> TypstOut {
    let mut writer = Writer { document, warnings: Vec::new(), open: Vec::new(), anchors: Vec::new() };
    let mut sections: Vec<TypstSection> = Vec::new();
    let mut parts: Vec<String> = Vec::new();
    fn finish(writer: &mut Writer<'_>, sections: &mut [TypstSection], parts: &mut Vec<String>) {
        while !writer.open.is_empty() {
            writer.close(0);
        }
        if let Some(section) = sections.last_mut() {
            section.body = std::mem::take(parts).join("\n\n");
            section.anchors.append(&mut writer.anchors);
        }
    }
    if has_preamble(&document.blocks) {
        writer.warn(
            "docx_text_vor_ueberschrift",
            "Text vor der ersten Überschrift steht in einem eigenen ersten Kapitel mit dem Titel des Werks.",
        );
        sections.push(TypstSection { level: 1, numbered: true, ..TypstSection::default() });
    }
    for block in &document.blocks {
        if is_outline(block) {
            finish(&mut writer, &mut sections, &mut parts);
            let (level, numbered, content) = match block {
                Block::Heading { level, numbered, content, .. } => (*level, *numbered, content),
                Block::Part { content, .. } => (0, true, content),
                _ => unreachable!("is_outline"),
            };
            if content.iter().any(|i| matches!(i, Inline::Footnote { .. } | Inline::Image { .. })) {
                writer.warn(
                    "docx_ueberschrift_vereinfacht",
                    "Fußnoten und Bilder in Überschriften sind nicht übernommen.",
                );
            }
            sections.push(TypstSection { level, numbered, title: plain(content), ..TypstSection::default() });
            // Only for the comment anchors; the title itself is in `title`.
            let _ = writer.inlines(content);
        } else if sections.is_empty() {
            // Page break before the first heading.
        } else if let Some(text) = writer.block(block) {
            parts.push(text);
        }
    }
    finish(&mut writer, &mut sections, &mut parts);
    TypstOut { sections, warnings: writer.warnings }
}
