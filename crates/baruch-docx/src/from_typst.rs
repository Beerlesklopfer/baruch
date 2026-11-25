// SPDX-License-Identifier: AGPL-3.0-or-later
//! Typst eines Abschnitts → Blöcke des Modells (Aufgabe docx_export, Vertrag Runde 32 §3), über
//! `baruch_wysiwyg::parse`. Nichts geht still verloren: Was Word nicht abbilden kann, geht als Typst-Insel mit
//! wörtlichem Quelltext mit und zählt eine Warnung `docx_typst_insel`.
//!
//! Abbildung:
//! - Überschrift `=` … → [`Block::Heading`], Teil-Markierung → [`Block::Part`], „ohne Nummer“ →
//!   `Heading { numbered: false }`; Absatz, Listen (`-`, `+`, `1.`), Blockzitat mit Quelle;
//! - im Absatz: fett, kursiv, Fußnote, Zitat, `#link("…")[…]`, erzwungener Zeilenumbruch;
//! - `#figure(image("…"), caption: […])` und `#image("…")` → [`Block::Figure`] (der Quelltext bleibt in `raw`),
//!   wenn der Pfad in `image_paths` steht; sonst Insel mit Warnung `docx_bild_fehlt`;
//! - `#table(columns: …, […], …)` → [`Block::Table`], soweit die Tabelle nur aus Zellen mit Inhalt (und
//!   `table.header(…)`) besteht; Gestaltung (`stroke`, `align`, …) entfällt mit Warnung `docx_tabelle_vereinfacht`;
//! - `#pagebreak()` → [`Block::PageBreak`];
//! - Unsichtbares (Label, Registermarke, Kommentar) geht als Insel ohne sichtbaren Ersatz und ohne Warnung mit;
//!   Querverweise `#link(<k…>)[Text]` als Insel mit ihrem Text;
//! - alles andere (eigene Funktionen, `#let`/`#set`/`#show`, Formeln, Raw-Text, Begriffslisten, `#szene[…]`,
//!   `#arbeit[…]`, Verweise `@name`) als Insel mit sichtbarem Ersatz und Warnung `docx_typst_insel`.

mod args;

use baruch_wysiwyg as wy;

use crate::{Block, DocxWarning, Inline, ListItem, Marks};

/// Warnung: eine Typst-Insel ging nur als Quelltext mit sichtbarem Ersatz mit.
pub const WARN_ISLAND: &str = "docx_typst_insel";
/// Warnung: Zu einem Bildpfad gibt es kein einbettbares Bild.
pub const WARN_IMAGE_MISSING: &str = "docx_bild_fehlt";
/// Warnung: Gestaltung einer Tabelle (Linien, Ausrichtung, …) ging nicht mit.
pub const WARN_TABLE_SIMPLIFIED: &str = "docx_tabelle_vereinfacht";

/// Ergebnis: Blöcke und Warnungen.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FromTypst {
    pub blocks: Vec<Block>,
    pub warnings: Vec<DocxWarning>,
}

/// Liest einen Abschnitt samt Überschriftszeile (`body`). Der Index eines Pfads in `image_paths` ist der Index des
/// Bildes in `DocxDocument::images`.
pub fn from_typst(body: &str, image_paths: &[String]) -> FromTypst {
    let mut cx = Cx { image_paths, warnings: Vec::new() };
    let blocks = cx.blocks(&wy::parse(body).blocks);
    FromTypst { blocks, warnings: cx.warnings }
}

struct Cx<'a> {
    image_paths: &'a [String],
    warnings: Vec<DocxWarning>,
}

fn text(text: impl Into<String>) -> Inline {
    Inline::Text { text: text.into(), marks: Marks::default() }
}

/// Kurzer Auszug für eine Warnung (eine Zeile, höchstens 60 Zeichen).
fn excerpt(raw: &str) -> String {
    let line = raw.trim().lines().next().unwrap_or("").trim();
    if line.chars().count() > 60 {
        format!("{}…", line.chars().take(60).collect::<String>())
    } else {
        line.to_owned()
    }
}

/// Pfad ohne führendes `/` oder `./` (so stehen die Bildpfade im Werk).
fn normal_path(path: &str) -> &str {
    let mut path = path.trim();
    while let Some(rest) = path.strip_prefix("./").or_else(|| path.strip_prefix('/')) {
        path = rest;
    }
    path
}

/// Nur Kommentare (`// …`, `/* … */`) und Leerraum? Dann ist im Satz nichts zu sehen.
fn only_comments(raw: &str) -> bool {
    let mut rest = raw.trim_start();
    if rest.is_empty() {
        return false;
    }
    while !rest.is_empty() {
        if let Some(after) = rest.strip_prefix("//") {
            rest = after.split_once('\n').map_or("", |(_, tail)| tail);
        } else if let Some(after) = rest.strip_prefix("/*") {
            match after.find("*/") {
                Some(end) if !after[..end].contains("/*") => rest = &after[end + 2..],
                _ => return false,
            }
        } else {
            return false;
        }
        rest = rest.trim_start();
    }
    true
}

impl Cx<'_> {
    fn warn(&mut self, code: &str, message: String) {
        self.warnings.push(DocxWarning { code: code.to_owned(), message });
    }

    fn warn_island(&mut self, raw: &str) {
        self.warn(
            WARN_ISLAND,
            format!(
                "Typst-Insel „{}“: Word kann sie nicht abbilden – der Quelltext steht sichtbar und wörtlich in der Datei.",
                excerpt(raw)
            ),
        );
    }

    fn blocks(&mut self, blocks: &[wy::Block]) -> Vec<Block> {
        blocks.iter().flat_map(|block| self.block(block)).collect()
    }

    fn block(&mut self, block: &wy::Block) -> Vec<Block> {
        let raw = block.raw.trim_end().to_owned();
        match &block.kind {
            wy::BlockKind::Heading { level, inlines, label } => {
                let mut content = self.inlines(inlines);
                if let Some(label) = label {
                    content.push(Inline::Typst { raw: format!("<{label}>"), shown: Vec::new() });
                }
                vec![Block::Heading { level: (*level).clamp(1, 6), numbered: true, content, mark: None }]
            }
            wy::BlockKind::Part { title } => vec![Block::Part { content: vec![text(title.as_str())], mark: None }],
            wy::BlockKind::Unnumbered { level, title } => vec![Block::Heading {
                level: (*level).clamp(1, 3),
                numbered: false,
                content: vec![text(title.as_str())],
                mark: None,
            }],
            wy::BlockKind::Paragraph { inlines } => {
                vec![Block::Paragraph { content: self.inlines(inlines), mark: None }]
            }
            wy::BlockKind::Quote { block: as_block, attribution, body } => {
                let attribution = attribution.as_deref().map(|a| self.inlines(a)).unwrap_or_default();
                // Ein Zitat im Fließtext, das allein im Absatz steht, bleibt ein Absatz mit Zitat im Text.
                if !*as_block
                    && attribution.is_empty()
                    && let [wy::Block { kind: wy::BlockKind::Paragraph { inlines }, .. }] = body.as_slice()
                {
                    let content = vec![Inline::Quote { content: self.inlines(inlines) }];
                    return vec![Block::Paragraph { content, mark: None }];
                }
                vec![Block::Quote { blocks: self.blocks(body), attribution }]
            }
            wy::BlockKind::List { ordered, items, depths } => {
                vec![Block::List { ordered: *ordered, start: 1, items: self.items(items, depths) }]
            }
            wy::BlockKind::Enum { start, items, depths } => {
                vec![Block::List { ordered: true, start: *start, items: self.items(items, depths) }]
            }
            wy::BlockKind::Terms { items } => {
                self.warn_island(&raw);
                let shown = items
                    .iter()
                    .map(|term| {
                        let mut content = bold(self.inlines(&term.term));
                        content.push(text(": "));
                        content.extend(self.inlines(&term.desc));
                        Block::Paragraph { content: merge(content), mark: None }
                    })
                    .collect();
                vec![Block::Typst { raw, shown }]
            }
            wy::BlockKind::Scene { body, .. } | wy::BlockKind::Work { body, .. } => {
                self.warn_island(&raw);
                let shown = self.blocks(body);
                vec![Block::Typst { raw, shown }]
            }
            wy::BlockKind::Island => self.island(raw),
        }
    }

    fn items(&mut self, items: &[Vec<wy::Inline>], depths: &[u8]) -> Vec<ListItem> {
        items
            .iter()
            .enumerate()
            .map(|(i, item)| ListItem {
                depth: depths.get(i).copied().unwrap_or(0),
                content: self.inlines(item),
                mark: None,
            })
            .collect()
    }

    /// Insel als Block: Abbildung, Tabelle, Seitenumbruch – sonst wörtlich mit sichtbarem Ersatz.
    fn island(&mut self, raw: String) -> Vec<Block> {
        if raw.trim().is_empty() {
            return Vec::new();
        }
        let trimmed = raw.trim();
        if trimmed == "#pagebreak()" {
            return vec![Block::PageBreak];
        }
        if only_comments(trimmed) {
            return vec![Block::Typst { raw, shown: Vec::new() }];
        }
        if (trimmed.starts_with("#figure(") || trimmed.starts_with("#image("))
            && let Some(path) = wy::image_path(trimmed)
        {
            let wanted = normal_path(&path);
            return match self.image_paths.iter().position(|p| normal_path(p) == wanted) {
                Some(image) => {
                    let figure = args::figure(trimmed);
                    let caption = figure.caption.map(|c| self.content_inlines(&c)).unwrap_or_default();
                    vec![Block::Figure { image, alt: figure.alt.unwrap_or_default(), caption, raw }]
                }
                None => {
                    self.warn(
                        WARN_IMAGE_MISSING,
                        format!(
                            "Bild „{path}“ ist im Werk nicht vorhanden oder hat ein Format, das Word nicht einbettet \
                             (PNG, JPEG, GIF gehen) – in der Datei steht nur der Quelltext der Abbildung."
                        ),
                    );
                    let shown = vec![Block::Paragraph { content: vec![text(format!("[Bild: {path}]"))], mark: None }];
                    vec![Block::Typst { raw, shown }]
                }
            };
        }
        if trimmed.starts_with("#table(")
            && let Some(table) = args::table(trimmed)
        {
            if table.simplified {
                self.warn(
                    WARN_TABLE_SIMPLIFIED,
                    format!(
                        "Tabelle „{}“: Gestaltung (Linien, Ausrichtung, Abstände) geht nicht nach Word mit.",
                        excerpt(trimmed)
                    ),
                );
            }
            let cell = |cx: &mut Self, source: &String| -> Vec<Block> {
                let blocks = cx.blocks(&wy::parse(source).blocks);
                if blocks.is_empty() { vec![Block::Paragraph { content: Vec::new(), mark: None }] } else { blocks }
            };
            let rows =
                table.rows.iter().map(|row| row.iter().map(|source| cell(self, source)).collect()).collect::<Vec<_>>();
            return vec![Block::Table { header_rows: table.header_rows, rows }];
        }
        self.warn_island(&raw);
        let shown = vec![Block::Paragraph { content: vec![text(trimmed)], mark: None }];
        vec![Block::Typst { raw, shown }]
    }

    /// Inhalt eines Inhaltsblocks `[…]` (Bildunterschrift) als Text im Absatz.
    fn content_inlines(&mut self, source: &str) -> Vec<Inline> {
        let doc = wy::parse(source);
        match doc.blocks.as_slice() {
            [] => Vec::new(),
            [wy::Block { kind: wy::BlockKind::Paragraph { inlines }, .. }] => self.inlines(inlines),
            _ => vec![text(source.trim())],
        }
    }

    fn inlines(&mut self, inlines: &[wy::Inline]) -> Vec<Inline> {
        let mut out = Vec::new();
        for inline in inlines {
            self.inline(inline, Marks::default(), &mut out);
        }
        merge(out)
    }

    fn marked(&mut self, inlines: &[wy::Inline], marks: Marks) -> Vec<Inline> {
        let mut out = Vec::new();
        for inline in inlines {
            self.inline(inline, marks, &mut out);
        }
        merge(out)
    }

    fn inline(&mut self, inline: &wy::Inline, marks: Marks, out: &mut Vec<Inline>) {
        match inline {
            wy::Inline::Text { v } => out.push(Inline::Text { text: v.clone(), marks }),
            // Der Zeilenwechsel im Quelltext ist im Satz ein Leerzeichen – außer direkt hinter einem erzwungenen
            // Zeilenumbruch, wo es die neue Zeile einrücken würde.
            wy::Inline::Soft => {
                if !matches!(out.last(), Some(Inline::LineBreak)) {
                    out.push(Inline::Text { text: " ".into(), marks });
                }
            }
            wy::Inline::Emph { c } => out.extend(self.marked(c, Marks { italic: true, ..marks })),
            wy::Inline::Strong { c } => out.extend(self.marked(c, Marks { bold: true, ..marks })),
            // Der Text einer Fußnote beginnt ohne die Auszeichnung der Stelle, an der ihr Zeichen steht.
            wy::Inline::Footnote { c } => out.push(Inline::Footnote { content: self.inlines(c) }),
            wy::Inline::Quote { c } => out.push(Inline::Quote { content: self.marked(c, marks) }),
            wy::Inline::Linebreak => out.push(Inline::LineBreak),
            wy::Inline::Link { url, c } => {
                let content = match c {
                    Some(c) => self.marked(c, marks),
                    None => vec![Inline::Text { text: url.clone(), marks }],
                };
                out.push(Inline::Link { url: url.clone(), content });
            }
            // Unsichtbar im Satz: wörtlich mitnehmen, nichts zeigen, nichts melden.
            wy::Inline::Label { v } => out.push(Inline::Typst { raw: format!("<{v}>"), shown: Vec::new() }),
            wy::Inline::IndexMark { .. } => out.push(Inline::Typst { raw: canonical(inline), shown: Vec::new() }),
            // Der Text des Querverweises bleibt lesbar; Ziel und Seitenzahl stehen im Quelltext.
            wy::Inline::Xref { c, .. } => {
                let shown = self.marked(c, marks);
                out.push(Inline::Typst { raw: canonical(inline), shown });
            }
            wy::Inline::Ref { .. } => {
                let raw = canonical(inline);
                self.warn_island(&raw);
                out.push(Inline::Typst { shown: vec![Inline::Text { text: raw.clone(), marks }], raw });
            }
            wy::Inline::Island { raw } => {
                if only_comments(raw) {
                    out.push(Inline::Typst { raw: raw.clone(), shown: Vec::new() });
                } else {
                    self.warn_island(raw);
                    out.push(Inline::Typst {
                        raw: raw.clone(),
                        shown: vec![Inline::Text { text: raw.clone(), marks }],
                    });
                }
            }
        }
    }
}

/// Kanonisches Typst eines einzelnen Inline-Elements (Verweis, Querverweis, Registermarke).
fn canonical(inline: &wy::Inline) -> String {
    wy::emit(&wy::BlockKind::Paragraph { inlines: vec![inline.clone()] })
}

fn bold(inlines: Vec<Inline>) -> Vec<Inline> {
    inlines
        .into_iter()
        .map(|inline| match inline {
            Inline::Text { text, marks } => Inline::Text { text, marks: Marks { bold: true, ..marks } },
            other => other,
        })
        .collect()
}

/// Benachbarte Textstücke mit gleicher Auszeichnung verbinden, leere entfernen.
pub(crate) fn merge(inlines: Vec<Inline>) -> Vec<Inline> {
    let mut out: Vec<Inline> = Vec::with_capacity(inlines.len());
    for inline in inlines {
        match (out.last_mut(), inline) {
            (_, Inline::Text { text, .. }) if text.is_empty() => {}
            (Some(Inline::Text { text: last, marks: last_marks }), Inline::Text { text, marks })
                if *last_marks == marks =>
            {
                last.push_str(&text);
            }
            (_, inline) => out.push(inline),
        }
    }
    out
}
