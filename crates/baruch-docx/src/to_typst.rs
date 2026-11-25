// SPDX-License-Identifier: AGPL-3.0-or-later
//! [`DocxDocument`] → Typst je Abschnitt (Aufgabe docx_import, Vertrag Runde 32 §2).
//!
//! Die Formen sind die des Markdown-Imports (Runde 21b), damit der WYSIWYG-Editor sie wiedererkennt:
//! - Wörtlich bleibt Text: `\ # * _ $ @ < [ ] ` ~` bekommen einen Backslash, ebenso `/` vor `/` oder `*` und `-` vor
//!   `-` oder `?`; am Zeilenanfang auch `= + - /` und die Ziffernfolge vor einem Punkt.
//! - Auszeichnung als Funktion: `#strong[…]`, `#emph[…]`, `#underline[…]`, `#strike[…]`, `#super[…]`, `#sub[…]`,
//!   `#smallcaps[…]`, `#link("…")[…]`, `#footnote[…]`, `#quote[…]`; folgt ein Zeichen, das Typst an den Ausdruck
//!   hängen würde, steht davor ein `;`.
//! - Blöcke: `#quote(block: true, attribution: […])[…]`, Listen mit `-`/`+` (zwei Leerzeichen je Ebene),
//!   `#table(columns: n, table.header(…), …)` (im Editor eine Typst-Insel), `#figure(image("…", alt: "…"),
//!   caption: […])` bzw. `#image("…", alt: "…")`, `#pagebreak()`; Typst-Inseln wörtlich.
//! - Gliederung: Teil und Überschrift 1–3 beginnen einen Abschnitt; Überschrift 4–6 bleibt fetter Text im Abschnitt
//!   (Warnung `docx_ueberschrift_tief`). Text vor der ersten Überschrift wird ein eigener erster Abschnitt mit
//!   **leerem Titel** (Warnung `docx_text_vor_ueberschrift`); der Aufrufer gibt ihm den Titel des Werks.
//!
//! Weitere Warnungen: `docx_tabelle_vereinfacht` (ungleich lange Zeilen), `docx_ueberschrift_vereinfacht` (Fußnote
//! oder Bild in einer Überschrift), `docx_bild_fehlt` (Bildverweis ohne Bild).

use crate::resolve::{has_preamble, is_outline, plain};
use crate::{Block, DocxDocument, DocxWarning, Inline, Marks};

/// Anker eines Kommentars im Abschnitt: der kommentierte Wortlaut.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommentAnchor {
    pub comment: u32,
    pub quote: String,
}

/// Ein Abschnitt des Werks: `level` 0 = Teil, 1–3 = Gliederung; `body` ohne die Überschriftszeile.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TypstSection {
    pub level: u8,
    pub numbered: bool,
    pub title: String,
    pub body: String,
    pub anchors: Vec<CommentAnchor>,
}

/// Ergebnis: Abschnitte in Lesereihenfolge und Warnungen.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TypstOut {
    pub sections: Vec<TypstSection>,
    pub warnings: Vec<DocxWarning>,
}

/// Fließtext für Typst-Markup maskieren (ohne Zeilenanfänge).
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

/// Zeichenkette als Typst-Literal.
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

/// Eine Folge von Inline-Inhalt als Typst.
#[derive(Default)]
struct Line {
    text: String,
    /// Der letzte Beitrag war ein Ausdruck (`#f[…]`), an den Typst Folgendes hängen könnte.
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
    /// Offene Kommentarbereiche: Kennung und bisher gesammelter Wortlaut.
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
                    // Der Wortlaut der Fußnote gehört nicht zur kommentierten Stelle.
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
                // Nur ein aufgelöstes Dokument hat keine; sonst gilt der Inhalt (Endstand), nichts geht verloren.
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

    /// Ein Block als Typst (ohne Leerzeile am Ende); `None`, wenn nichts zu schreiben ist.
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
                let mut first = true;
                for item in items {
                    let indent = "  ".repeat(usize::from(item.depth.min(8)));
                    let marker = if !*ordered {
                        "-".to_owned()
                    } else if first && item.depth == 0 && *start != 1 {
                        format!("{start}.")
                    } else {
                        "+".to_owned()
                    };
                    first = false;
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

/// Schreibt ein aufgelöstes Dokument (ohne [`crate::Inline::Tracked`]) als Typst je Abschnitt.
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
            // Nur für die Kommentar-Anker; der Titel selbst steht in `title`.
            let _ = writer.inlines(content);
        } else if sections.is_empty() {
            // Seitenumbruch vor der ersten Überschrift.
        } else if let Some(text) = writer.block(block) {
            parts.push(text);
        }
    }
    finish(&mut writer, &mut sections, &mut parts);
    TypstOut { sections, warnings: writer.warnings }
}
