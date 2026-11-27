// SPDX-License-Identifier: AGPL-3.0-or-later
//! XHTML mapping of the block model of baruch-wysiwyg (ownership "epub", Round 20a).
//!
//! A section text is split into blocks with [`baruch_wysiwyg::parse`] and written as XHTML here:
//!
//! | Block model | XHTML |
//! |---|---|
//! | heading of the section (`=`, part, "without number") | `<h1>`–`<h3>` with `id="k{key}"`, number from the outline scheme |
//! | further heading in the text | `<h{level}>` (at most `<h6>`) |
//! | paragraph | `<p>` |
//! | `_…_` / `*…*` | `<em>` / `<strong>` |
//! | list `-` / `+`, enumeration `1.` | `<ul>` / `<ol>` (nested, `start`) |
//! | term list `/ A: B` | `<dl><dt>A</dt><dd>B</dd></dl>` |
//! | `#quote(block: true)[…]` / `#quote[…]` | `<blockquote>` (source in `<footer>`) / `<q>` |
//! | `#szene[…]` / `#arbeit[…]` | `<section class="baruch-scene">` / `<section class="baruch-work">` |
//! | `#link("https://…")[…]` | `<a href="…">` (only http, https, mailto; otherwise only the text) |
//! | cross-reference `#link(<kN>)[…]` | `<a href="k{chapter}.xhtml#kN">` |
//! | reference `@name` | link to the target, otherwise the name as text |
//! | label `<name>` | `<span id="lbl-name"></span>` |
//! | `#footnote[…]` | popup: `<a epub:type="noteref">` + `<aside epub:type="footnote">` at the end of the file; endnotes: link into `notes.xhtml` |
//! | island with `image("…")` | `<figure>` with `<img>` (caption from `caption:`), inline `<img>` |
//! | `` `Code` `` | `<code>` |
//! | escape `\x` as an island | the character |
//! | comment, anchor line `#metadata("baruch-key") <kN>` | nothing (without a warning) |
//! | index mark `#metadata((…))<baruch-idx>` (R31) | `<span id="baruch-idx-{key}-{n}" class="baruch-idx"></span>` (target of the index pages) |
//! | first mention `#link(<baruch-gl-UUID>)[#emph[Wort]]` (R31) | `<a epub:type="glossref" href="glossary.xhtml#baruch-gl-UUID"><em>Wort</em></a>` |
//! | every other island | nothing, warning `ISLAND_OMITTED` with key |
//!
//! All text is escaped with [`escape_text`]; nothing from the work is taken over as HTML.

use std::collections::{BTreeMap, BTreeSet};

use baruch_typst::typeset::FootnoteNumbering;
use baruch_wysiwyg::{Block, BlockKind, Inline, Term};

use crate::EpubWarning;

/// Escape text for XML/XHTML (`&`, `<`, `>`, `"`, `'`). Characters that XML 1.0 forbids (control characters
/// except tab, line feed and carriage return) are dropped – otherwise the file would not be well-formed XML.
pub fn escape_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            '\t' | '\n' | '\r' => out.push(c),
            c if c.is_control() => {}
            '\u{FFFE}' | '\u{FFFF}' => {}
            c => out.push(c),
        }
    }
    out
}

/// An image in the package: target relative to `OEBPS/` and alternative text from the image details.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageTarget {
    pub href: String,
    pub alt: String,
}

/// A footnote: number, displayed mark, content (XHTML) and file in which the reference is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Note {
    pub number: usize,
    pub marker: String,
    pub html: String,
    /// File name of the reference within `text/` (`k11.xhtml`).
    pub file: String,
}

/// Where footnote references point.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoteTarget {
    /// Into the same file (`#fn3`), `<aside>` at the end of the file.
    SameFile,
    /// Into `notes.xhtml`.
    Endnotes,
}

/// What all files know in common: targets of the cross-references, labels, images.
#[derive(Debug, Default)]
pub struct Links {
    /// R31: UUIDs of the glossary terms in `glossary.xhtml` (only with a glossary page); first mentions link there.
    pub glossary: BTreeSet<String>,
    /// Key → file name in `text/` (`k11.xhtml`).
    pub key_files: BTreeMap<i64, String>,
    /// Key → displayed heading (for `@kN`).
    pub key_titles: BTreeMap<i64, String>,
    /// Label → file name in `text/`.
    pub labels: BTreeMap<String, String>,
    /// Round 23a: label of a figure (`#figure(…) <name>`) → running number in the book (1-based, all figures with
    /// an image count in reading order, as Typst counts). `@name` then shows „Abbildung n“ instead of the label name.
    pub figures: BTreeMap<String, usize>,
    /// Normalised Typst path → image in the package.
    pub images: BTreeMap<String, ImageTarget>,
}

/// Writes the blocks of a file. It collects footnotes and warnings across all sections of the file.
pub struct Renderer<'a> {
    pub links: &'a Links,
    /// File name of this file in `text/`.
    pub file: String,
    /// Key of the section that is currently being written (for warnings).
    pub key: i64,
    pub notes: Vec<Note>,
    /// Next footnote number.
    pub next_note: usize,
    pub note_target: NoteTarget,
    pub numbering: FootnoteNumbering,
    pub warnings: Vec<EpubWarning>,
    /// R31: anchors of the index marks that were set (file, id, section, mark) for the index pages.
    pub index_marks: Vec<PlacedMark>,
    ids: BTreeSet<String>,
    in_link: bool,
    in_note: bool,
}

/// R31: anchor of an index mark in the EPUB.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlacedMark {
    /// Section in which the mark is.
    pub key: i64,
    /// File in `text/` (with endnotes `notes.xhtml`, if the mark is in a footnote).
    pub file: String,
    /// id of the anchor (`baruch-idx-{key}-{n}`, unique in the book).
    pub id: String,
    pub mark: baruch_typst::index::IndexMark,
}

/// R31: word and UUID of the markup of the first mention with a glossary page
/// (`#link(<baruch-gl-UUID>)[#emph[Wort]]`, form from `baruch_typst::index::Projector`).
pub fn glossary_mention(raw: &str) -> Option<(&str, &str)> {
    let rest = raw.strip_prefix("#link(<")?.strip_prefix(baruch_typst::index::GLOSSARY_ANCHOR_PREFIX)?;
    let (uuid, rest) = rest.split_once(">)[#emph[")?;
    let word = rest.strip_suffix("]]")?;
    let plain = |s: &str| !s.is_empty() && !s.contains(['[', ']', '#', '<', '>']);
    (plain(uuid) && plain(word)).then_some((uuid, word))
}

/// Is this the `;` with which the projection ends an embedded expression before `(`, `[`, `;` or `.Wort` (Typst
/// swallows it)?
fn expression_guard(text: &str) -> bool {
    let Some(rest) = text.strip_prefix(';') else { return false };
    let mut chars = rest.chars();
    match chars.next() {
        Some('(' | '[' | ';') => true,
        Some('.') => chars.next().is_some_and(|c| c.is_alphabetic() || c == '_'),
        _ => false,
    }
}

/// Normal form of an image path as in store and export: without leading `/`, without `.` parts; `..` → `None`.
pub fn normalize_path(path: &str) -> Option<String> {
    let mut parts = Vec::new();
    for part in path.trim().split('/') {
        match part {
            "" | "." => {}
            ".." => return None,
            p => parts.push(p),
        }
    }
    (!parts.is_empty()).then(|| parts.join("/"))
}

/// Mark of a footnote for the number (1-based) according to the numbering style of the typesetting settings.
pub fn note_marker(numbering: FootnoteNumbering, n: usize) -> String {
    match numbering {
        FootnoteNumbering::Arabic => n.to_string(),
        FootnoteNumbering::Letters => {
            let mut n = n;
            let mut out = Vec::new();
            while n > 0 {
                n -= 1;
                out.push(char::from(b'a' + (n % 26) as u8));
                n /= 26;
            }
            out.iter().rev().collect()
        }
        FootnoteNumbering::Symbols => {
            const SYMBOLS: [char; 3] = ['*', '†', '‡'];
            let symbol = SYMBOLS[(n - 1) % 3];
            std::iter::repeat_n(symbol, (n - 1) / 3 + 1).collect()
        }
    }
}

/// Content of a string literal after `name:` in `raw` (e.g. `alt: "…"`), escapes `\"` and `\\` resolved.
pub fn named_string(raw: &str, name: &str) -> Option<String> {
    let at = find_named(raw, name)?;
    let rest = raw[at..].trim_start().strip_prefix('"')?;
    let mut out = String::new();
    let mut chars = rest.chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => match chars.next()? {
                'n' => out.push('\n'),
                't' => out.push('\t'),
                other => out.push(other),
            },
            '"' => return Some(out),
            c => out.push(c),
        }
    }
    None
}

/// Position after `name:` (as a whole word) in `raw`.
fn find_named(raw: &str, name: &str) -> Option<usize> {
    let pattern = format!("{name}:");
    let mut from = 0;
    while let Some(found) = raw[from..].find(&pattern) {
        let at = from + found;
        let before = raw[..at].chars().next_back();
        if !before.is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '-') {
            return Some(at + pattern.len());
        }
        from = at + pattern.len();
    }
    None
}

/// Content of `caption: […]` (verbatim, without brackets) or `caption: "…"` (as text) of a figure.
pub enum Caption {
    Markup(String),
    Text(String),
}

pub fn caption_of(raw: &str) -> Option<Caption> {
    let at = find_named(raw, "caption")?;
    let rest = raw[at..].trim_start();
    if rest.starts_with('"') {
        return named_string(raw, "caption").map(Caption::Text);
    }
    let rest = rest.strip_prefix('[')?;
    let mut depth = 0usize;
    let mut escaped = false;
    for (i, c) in rest.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match c {
            '\\' => escaped = true,
            '[' => depth += 1,
            ']' if depth == 0 => return (i > 0).then_some(Caption::Markup(rest[..i].trim().to_owned())),
            ']' => depth -= 1,
            _ => {}
        }
    }
    None
}

/// Only comments (and whitespace)?
fn only_comments(raw: &str) -> bool {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return true;
    }
    if trimmed.starts_with("/*") && trimmed.ends_with("*/") && trimmed[2..].find("*/") == Some(trimmed.len() - 4) {
        return true;
    }
    trimmed.lines().all(|l| l.trim().is_empty() || l.trim_start().starts_with("//"))
}

/// Short excerpt for warnings (one line, at most 60 characters).
fn excerpt(raw: &str) -> String {
    let line = raw.trim().lines().next().unwrap_or("");
    let mut out: String = line.chars().take(60).collect();
    if line.chars().count() > 60 || raw.trim().lines().nth(1).is_some() {
        out.push('…');
    }
    out
}

/// Valid id from a label (characters other than letters, digits, `-`, `_`, `.` → `_`).
/// Round 23a: is the island a figure (`#figure(…)`)? Then its label, if one follows it
/// (`#figure(…) <name>`): `Some(None)` = figure without label, `None` = not a figure.
pub fn figure_label(raw: &str) -> Option<Option<String>> {
    let text = raw.trim();
    if !text.starts_with("#figure(") {
        return None;
    }
    let label = text.strip_suffix('>').and_then(|rest| {
        let (before, name) = rest.rsplit_once('<')?;
        let valid = !name.is_empty() && name.chars().all(|c| c.is_alphanumeric() || matches!(c, '-' | '_' | '.' | ':'));
        (valid && before.trim_end().ends_with([')', ']'])).then(|| name.to_owned())
    });
    Some(label)
}

/// Round 23a: does the paragraph consist only of a figure and its label (`#figure(…) <name>`, at most whitespace
/// in between)? Then the raw text of the figure and the label name.
pub fn labelled_figure(inlines: &[Inline]) -> Option<(&str, &str)> {
    let mut parts = inlines.iter().filter(|i| match i {
        Inline::Text { v } => !v.trim().is_empty(),
        Inline::Soft => false,
        _ => true,
    });
    let (Some(Inline::Island { raw }), Some(Inline::Label { v }), None) = (parts.next(), parts.next(), parts.next())
    else {
        return None;
    };
    figure_label(raw).map(|_| (raw.as_str(), v.as_str()))
}

/// Word before the number of a figure if the reference does not bring its own (Typst with `lang: "de"`).
pub const FIGURE_SUPPLEMENT: &str = "Abbildung";

pub fn label_id(label: &str) -> String {
    let clean: String =
        label.chars().map(|c| if c.is_alphanumeric() || matches!(c, '-' | '_' | '.') { c } else { '_' }).collect();
    format!("lbl-{clean}")
}

impl<'a> Renderer<'a> {
    pub fn new(
        links: &'a Links,
        file: &str,
        note_target: NoteTarget,
        numbering: FootnoteNumbering,
        next_note: usize,
    ) -> Self {
        Renderer {
            links,
            file: file.to_owned(),
            key: 0,
            notes: Vec::new(),
            next_note,
            note_target,
            numbering,
            warnings: Vec::new(),
            index_marks: Vec::new(),
            ids: BTreeSet::new(),
            in_link: false,
            in_note: false,
        }
    }

    fn warn(&mut self, code: &str, message: String) {
        self.warnings.push(EpubWarning { code: code.to_owned(), message, key: Some(self.key) });
    }

    /// Reserve an id for this file; already taken → `None`.
    pub fn claim_id(&mut self, id: &str) -> bool {
        self.ids.insert(id.to_owned())
    }

    /// A section: heading (with `id="k{key}"` and displayed number) and blocks. `level` 0 = part.
    pub fn section(&mut self, out: &mut String, key: i64, level: u8, number: Option<&str>, title: &str, body: &str) {
        self.key = key;
        let doc = baruch_wysiwyg::parse(body);
        let tag = level.clamp(1, 3);
        let class = if level == 0 { " class=\"baruch-part-title\"" } else { "" };
        let number = number.map(str::trim).filter(|n| !n.is_empty());
        let mut heading_done = false;
        let own_id = format!("k{key}");
        self.claim_id(&own_id);
        for block in &doc.blocks {
            if !heading_done {
                let inner = match &block.kind {
                    BlockKind::Heading { inlines, label, .. } => {
                        let mut inner = String::new();
                        self.inlines(&mut inner, inlines);
                        if let Some(label) = label {
                            self.label_anchor(&mut inner, label);
                        }
                        Some(inner)
                    }
                    BlockKind::Part { title } | BlockKind::Unnumbered { title, .. } => Some(escape_text(title)),
                    _ if block.raw.trim().is_empty() || only_comments(&block.raw) => continue,
                    _ => None,
                };
                heading_done = true;
                let inner = match inner {
                    Some(inner) if !inner.trim().is_empty() => inner,
                    other => {
                        let shown = escape_text(title);
                        if other.is_none() {
                            // The text does not start with a heading: the heading from the outline before it.
                            self.heading(out, tag, class, &own_id, number, &shown);
                            self.block(out, block);
                            continue;
                        }
                        shown
                    }
                };
                self.heading(out, tag, class, &own_id, number, inner.trim());
                continue;
            }
            self.block(out, block);
        }
        if !heading_done {
            self.heading(out, tag, class, &own_id, number, &escape_text(title));
        }
    }

    fn heading(&mut self, out: &mut String, tag: u8, class: &str, id: &str, number: Option<&str>, inner: &str) {
        let number = number.map(|n| format!("<span class=\"baruch-number\">{}</span> ", escape_text(n)));
        out.push_str(&format!("<h{tag} id=\"{id}\"{class}>{}{inner}</h{tag}>\n", number.unwrap_or_default()));
    }

    fn label_anchor(&mut self, out: &mut String, label: &str) {
        if baruch_typst::xref::parse_key_label(label).is_some() {
            // Anchors `<kN>` are carried by the headings themselves.
            return;
        }
        let id = label_id(label);
        if self.claim_id(&id) {
            out.push_str(&format!("<span id=\"{id}\"></span>"));
        }
    }

    /// Blocks one after another.
    pub fn blocks(&mut self, out: &mut String, blocks: &[Block]) {
        for block in blocks {
            self.block(out, block);
        }
    }

    fn block(&mut self, out: &mut String, block: &Block) {
        // Anchor line `#metadata("baruch-key") <kN>` (the block model sees a paragraph with island and label in it).
        if let Some(key) = baruch_typst::xref::parse_anchor_line(block.raw.trim()) {
            let id = format!("k{key}");
            if self.claim_id(&id) {
                out.push_str(&format!("<span id=\"{id}\"></span>\n"));
            }
            return;
        }
        match &block.kind {
            BlockKind::Heading { level, inlines, label } => {
                let tag = (*level).clamp(1, 6);
                let id = match label {
                    Some(label) if baruch_typst::xref::parse_key_label(label).is_none() => {
                        let id = label_id(label);
                        self.claim_id(&id).then_some(id)
                    }
                    _ => None,
                };
                let id = id.map(|id| format!(" id=\"{id}\"")).unwrap_or_default();
                let mut inner = String::new();
                self.inlines(&mut inner, inlines);
                out.push_str(&format!("<h{tag}{id}>{}</h{tag}>\n", inner.trim()));
            }
            BlockKind::Part { title } => {
                out.push_str(&format!("<h1 class=\"baruch-part-title\">{}</h1>\n", escape_text(title)));
            }
            BlockKind::Unnumbered { level, title } => {
                let tag = (*level).clamp(1, 6);
                out.push_str(&format!("<h{tag}>{}</h{tag}>\n", escape_text(title)));
            }
            // Round 23a: the block model reads `#figure(…) <name>` as a paragraph of island and label. That is a
            // figure with an anchor (target of `@name`), not an inline image – otherwise caption and anchor would be
            // missing.
            BlockKind::Paragraph { inlines } if labelled_figure(inlines).is_some() => {
                let (raw, label) = labelled_figure(inlines).expect("eben geprüft");
                self.block_island(out, &format!("{} <{label}>", raw.trim()));
            }
            BlockKind::Paragraph { inlines } => {
                let mut inner = String::new();
                self.inlines(&mut inner, inlines);
                if !inner.trim().is_empty() {
                    out.push_str(&format!("<p>{}</p>\n", inner.trim()));
                }
            }
            BlockKind::Quote { attribution, body, .. } => {
                out.push_str("<blockquote class=\"baruch-quote\">\n");
                self.blocks(out, body);
                if let Some(attribution) = attribution {
                    out.push_str("<footer class=\"baruch-attribution\">— ");
                    self.inlines(out, attribution);
                    out.push_str("</footer>\n");
                }
                out.push_str("</blockquote>\n");
            }
            BlockKind::List { ordered, items, depths } => {
                let tag = if *ordered { "ol" } else { "ul" };
                self.items(out, tag, "", items, depths);
            }
            BlockKind::Enum { start, items, depths } => {
                let attrs = if *start == 1 { String::new() } else { format!(" start=\"{start}\"") };
                self.items(out, "ol", &attrs, items, depths);
            }
            BlockKind::Terms { items } => {
                out.push_str("<dl>\n");
                for Term { term, desc } in items {
                    out.push_str("<dt>");
                    self.inlines(out, term);
                    out.push_str("</dt>\n<dd>");
                    self.inlines(out, desc);
                    out.push_str("</dd>\n");
                }
                out.push_str("</dl>\n");
            }
            BlockKind::Scene { args, body } | BlockKind::Work { args, body } => {
                let (class, what) = match block.kind {
                    BlockKind::Scene { .. } => ("baruch-scene", "Szene"),
                    _ => ("baruch-work", "Arbeit"),
                };
                if let Some(args) = args.as_deref().filter(|a| !a.trim().is_empty()) {
                    self.warn(
                        "ARGS_OMITTED",
                        format!("Angaben der {what} ({}) erscheinen im EPUB nicht.", excerpt(args)),
                    );
                }
                out.push_str(&format!("<section class=\"{class}\">\n"));
                self.blocks(out, body);
                out.push_str("</section>\n");
            }
            BlockKind::Island => self.block_island(out, &block.raw),
        }
    }

    fn items(&mut self, out: &mut String, tag: &str, attrs: &str, items: &[Vec<Inline>], depths: &[u8]) {
        out.push_str(&format!("<{tag}{attrs}>"));
        let mut open = 0usize;
        for (k, item) in items.iter().enumerate() {
            let depth = usize::from(depths.get(k).copied().unwrap_or(0)).min(open + usize::from(k > 0));
            if k > 0 {
                if depth > open {
                    out.push_str(&format!("<{tag}>"));
                } else {
                    out.push_str("</li>");
                    for _ in depth..open {
                        out.push_str(&format!("</{tag}></li>"));
                    }
                }
            }
            open = depth;
            out.push_str("<li>");
            self.inlines(out, item);
        }
        if !items.is_empty() {
            out.push_str("</li>");
        }
        for _ in 0..open {
            out.push_str(&format!("</{tag}></li>"));
        }
        out.push_str(&format!("</{tag}>\n"));
    }

    /// `<img>` per image path of the island (missing images as a hint, with a warning).
    fn images(&mut self, raw: &str, class: &str) -> Option<String> {
        let paths = baruch_wysiwyg::image_paths(raw);
        if paths.is_empty() {
            return None;
        }
        let typst_alt = named_string(raw, "alt").filter(|a| !a.trim().is_empty());
        let mut out = String::new();
        for path in paths {
            let target = normalize_path(&path).and_then(|p| self.links.images.get(&p)).cloned();
            match target {
                Some(target) => {
                    let alt = typst_alt.clone().unwrap_or_else(|| target.alt.trim().to_owned());
                    if alt.is_empty() {
                        self.warn(
                            "IMAGE_NO_ALT",
                            format!("Bild {path} hat keinen Alternativtext (alt=\"\"); Vorleseprogramme übergehen es."),
                        );
                    }
                    out.push_str(&format!(
                        "<img class=\"{class}\" src=\"../{}\" alt=\"{}\"/>",
                        escape_text(&target.href),
                        escape_text(&alt)
                    ));
                }
                None => {
                    self.warn(
                        "IMAGE_MISSING",
                        format!("Bild {path} gibt es im Werk nicht; im EPUB steht ein Hinweis."),
                    );
                    out.push_str(&format!("<span class=\"baruch-missing\">Bild fehlt: {}</span>", escape_text(&path)));
                }
            }
        }
        Some(out)
    }

    fn block_island(&mut self, out: &mut String, raw: &str) {
        if only_comments(raw) {
            return;
        }
        if let Some(key) = baruch_typst::xref::parse_anchor_line(raw.trim()) {
            let id = format!("k{key}");
            if self.claim_id(&id) {
                out.push_str(&format!("<span id=\"{id}\"></span>\n"));
            }
            return;
        }
        if let Some(images) = self.images(raw, "baruch-image") {
            // Round 23a: a label on the figure becomes its anchor (target of `@name`).
            let anchor = match figure_label(raw).flatten().map(|label| label_id(&label)) {
                Some(id) if self.claim_id(&id) => format!(" id=\"{id}\""),
                _ => String::new(),
            };
            out.push_str(&format!("<figure class=\"baruch-figure\"{anchor}>"));
            out.push_str(&images);
            match caption_of(raw) {
                Some(Caption::Markup(markup)) => {
                    let doc = baruch_wysiwyg::parse(&markup);
                    let mut caption = String::new();
                    for block in &doc.blocks {
                        if let BlockKind::Paragraph { inlines } = &block.kind {
                            if !caption.is_empty() {
                                caption.push(' ');
                            }
                            self.inlines(&mut caption, inlines);
                        }
                    }
                    if !caption.trim().is_empty() {
                        out.push_str(&format!("<figcaption>{}</figcaption>", caption.trim()));
                    }
                }
                Some(Caption::Text(text)) if !text.trim().is_empty() => {
                    out.push_str(&format!("<figcaption>{}</figcaption>", escape_text(text.trim())));
                }
                _ => {}
            }
            out.push_str("</figure>\n");
            return;
        }
        self.warn("ISLAND_OMITTED", format!("Typst-Insel ausgelassen (im EPUB nicht abbildbar): {}", excerpt(raw)));
    }

    fn inline_island(&mut self, out: &mut String, raw: &str) {
        if only_comments(raw) {
            return;
        }
        // Escape `\x` that would become a shorthand as text: in the EPUB simply the character.
        if let Some(rest) = raw.strip_prefix('\\') {
            let mut chars = rest.chars();
            if let (Some(c), None) = (chars.next(), chars.next()) {
                out.push_str(&escape_text(&c.to_string()));
                return;
            }
        }
        if raw.starts_with('`') {
            let inner = raw.trim_start_matches('`').trim_end_matches('`');
            out.push_str(&format!("<code>{}</code>", escape_text(inner)));
            return;
        }
        if let Some(images) = self.images(raw, "baruch-inline-image") {
            out.push_str(&images);
            return;
        }
        if let Some((uuid, word)) = glossary_mention(raw) {
            if self.links.glossary.contains(uuid) && !self.in_link {
                out.push_str(&format!(
                    "<a class=\"baruch-glossref\" epub:type=\"glossref\" href=\"glossary.xhtml#{}{}\"><em>{}</em></a>",
                    baruch_typst::index::GLOSSARY_ANCHOR_PREFIX,
                    escape_text(uuid),
                    escape_text(word)
                ));
            } else {
                out.push_str(&format!("<em>{}</em>", escape_text(word)));
            }
            return;
        }
        self.warn(
            "ISLAND_OMITTED",
            format!("Typst-Insel im Satz ausgelassen (im EPUB nicht abbildbar): {}", excerpt(raw)),
        );
    }

    /// Inline elements.
    pub fn inlines(&mut self, out: &mut String, inlines: &[Inline]) {
        let mut after_expression = false;
        for inline in inlines {
            let guard = std::mem::replace(
                &mut after_expression,
                matches!(
                    inline,
                    Inline::Emph { .. }
                        | Inline::Strong { .. }
                        | Inline::Quote { .. }
                        | Inline::Footnote { .. }
                        | Inline::Link { .. }
                        | Inline::Xref { .. }
                        | Inline::Island { .. }
                ),
            );
            match inline {
                // R31: Typst does not typeset the `;` after an embedded expression (projection of the first mention).
                Inline::Text { v } if guard && expression_guard(v) => out.push_str(&escape_text(&v[1..])),
                Inline::Text { v } => out.push_str(&escape_text(v)),
                Inline::Soft => out.push('\n'),
                Inline::Linebreak => out.push_str("<br/>"),
                Inline::Emph { c } => self.wrap(out, "<em>", c, "</em>"),
                Inline::Strong { c } => self.wrap(out, "<strong>", c, "</strong>"),
                Inline::Quote { c } => self.wrap(out, "<q>", c, "</q>"),
                Inline::Footnote { c } => self.footnote(out, c),
                Inline::Label { v } => self.label_anchor(out, v),
                Inline::Island { raw } => self.inline_island(out, raw),
                Inline::Link { url, c } => {
                    let safe =
                        ["http://", "https://", "mailto:"].iter().any(|s| url.to_ascii_lowercase().starts_with(s));
                    let text = |me: &mut Self, out: &mut String| match c {
                        Some(c) => me.inlines(out, c),
                        None => out.push_str(&escape_text(url)),
                    };
                    if safe && !self.in_link {
                        out.push_str(&format!("<a href=\"{}\">", escape_text(url)));
                        self.in_link = true;
                        text(self, out);
                        self.in_link = false;
                        out.push_str("</a>");
                    } else {
                        text(self, out);
                    }
                }
                Inline::Xref { key, c, .. } => match self.links.key_files.get(key).cloned() {
                    Some(file) if !self.in_link => {
                        out.push_str(&format!("<a class=\"baruch-xref\" href=\"{file}#k{key}\">"));
                        self.in_link = true;
                        self.inlines(out, c);
                        self.in_link = false;
                        out.push_str("</a>");
                    }
                    Some(_) => self.inlines(out, c),
                    None => {
                        self.warn(
                            "XREF_UNKNOWN",
                            format!("Querverweis auf Abschnitt {key}, den es im Werk nicht gibt; nur der Text bleibt."),
                        );
                        self.inlines(out, c);
                    }
                },
                Inline::Ref { target, supplement } => self.reference(out, target, supplement.as_deref()),
                // R31: anchor of the index mark; the index pages link here.
                Inline::IndexMark { mark } => {
                    // Key in the id: unique also in notes.xhtml, where footnotes of all chapters are.
                    let key = self.key;
                    let mut n = self.index_marks.iter().filter(|m| m.key == key).count() + 1;
                    while !self.claim_id(&format!("baruch-idx-{key}-{n}")) {
                        n += 1;
                    }
                    let id = format!("baruch-idx-{key}-{n}");
                    out.push_str(&format!("<span id=\"{id}\" class=\"baruch-idx\"></span>"));
                    let file = if self.in_note && self.note_target == NoteTarget::Endnotes {
                        "notes.xhtml".to_owned()
                    } else {
                        self.file.clone()
                    };
                    self.index_marks.push(PlacedMark { key: self.key, file, id, mark: mark.clone() });
                }
            }
        }
    }

    fn reference(&mut self, out: &mut String, target: &str, supplement: Option<&str>) {
        let (href, shown) = if let Some(key) = baruch_typst::xref::parse_key_label(target) {
            let href = self.links.key_files.get(&key).map(|f| format!("{f}#k{key}"));
            let title = self.links.key_titles.get(&key).cloned().unwrap_or_else(|| target.to_owned());
            (href, title)
        } else {
            let href = self.links.labels.get(target).map(|f| format!("{f}#{}", label_id(target)));
            // Round 23a: a reference to a figure shows its number like Typst („Abbildung 3“), not the label name.
            match self.links.figures.get(target) {
                Some(number) => {
                    let word = supplement.map(str::trim).filter(|s| !s.is_empty()).unwrap_or(FIGURE_SUPPLEMENT);
                    let shown = format!("{word}\u{a0}{number}");
                    return self.reference_link(out, href, &shown);
                }
                None => (href, target.to_owned()),
            }
        };
        let shown = match supplement.map(str::trim).filter(|s| !s.is_empty()) {
            Some(s) => format!("{s} {shown}"),
            None => shown,
        };
        self.reference_link(out, href, &shown);
    }

    fn reference_link(&mut self, out: &mut String, href: Option<String>, shown: &str) {
        match href {
            Some(href) if !self.in_link => {
                out.push_str(&format!(
                    "<a class=\"baruch-ref\" href=\"{}\">{}</a>",
                    escape_text(&href),
                    escape_text(shown)
                ));
            }
            _ => out.push_str(&escape_text(shown)),
        }
    }

    fn wrap(&mut self, out: &mut String, open: &str, inlines: &[Inline], close: &str) {
        out.push_str(open);
        self.inlines(out, inlines);
        out.push_str(close);
    }

    fn footnote(&mut self, out: &mut String, content: &[Inline]) {
        let number = self.next_note;
        self.next_note += 1;
        let marker = note_marker(self.numbering, number);
        // Round 23a (review R20): the place in the list is assigned **before** the content is written. A footnote
        // in a footnote gets the next number and comes after it – before, the inner one landed before the outer
        // one, and `start` together with the visible numbers of the list shifted.
        let slot = self.notes.len();
        self.notes.push(Note { number, marker: marker.clone(), html: String::new(), file: self.file.clone() });
        let mut html = String::new();
        let was = self.in_link;
        self.in_link = false;
        let was_note = std::mem::replace(&mut self.in_note, true);
        self.inlines(&mut html, content);
        self.in_note = was_note;
        self.in_link = was;
        let href = match self.note_target {
            NoteTarget::SameFile => format!("#fn{number}"),
            NoteTarget::Endnotes => format!("notes.xhtml#fn{number}"),
        };
        if self.in_link {
            // A link in a link would not be valid XHTML: the mark without a link, the footnote is there all the same.
            out.push_str(&format!("<sup id=\"fnref{number}\">{}</sup>", escape_text(&marker)));
        } else {
            out.push_str(&format!(
                "<sup><a class=\"baruch-noteref\" id=\"fnref{number}\" href=\"{href}\" epub:type=\"noteref\" \
                 role=\"doc-noteref\">{}</a></sup>",
                escape_text(&marker)
            ));
        }
        self.claim_id(&format!("fnref{number}"));
        self.notes[slot].html = html.trim().to_owned();
    }
}

/// Footnotes at the end of a chapter file (popup): `<ol class="baruch-footnotes">` with one
/// `<aside epub:type="footnote">` each; readable also in readers without popups.
pub fn footnote_list(notes: &[Note]) -> String {
    if notes.is_empty() {
        return String::new();
    }
    let start = notes[0].number;
    let start_attr = if start == 1 { String::new() } else { format!(" start=\"{start}\"") };
    let mut out = format!("<hr class=\"baruch-notes-rule\"/>\n<ol class=\"baruch-footnotes\"{start_attr}>\n");
    for note in notes {
        out.push_str(&format!(
            "<li><aside id=\"fn{n}\" epub:type=\"footnote\" role=\"doc-footnote\"><p>{html} <a href=\"#fnref{n}\" \
             class=\"baruch-backlink\" role=\"doc-backlink\" title=\"zurück zum Text\">↩</a></p></aside></li>\n",
            n = note.number,
            html = note.html,
        ));
    }
    out.push_str("</ol>\n");
    out
}

/// List of the endnotes (`text/notes.xhtml`) with back references into the chapter files.
pub fn endnote_list(notes: &[Note]) -> String {
    let mut out = String::from("<ol class=\"baruch-footnotes\">\n");
    for note in notes {
        out.push_str(&format!(
            "<li id=\"fn{n}\" epub:type=\"endnote\"><p>{html} <a href=\"{file}#fnref{n}\" class=\"baruch-backlink\" \
             role=\"doc-backlink\" title=\"zurück zum Text\">↩</a></p></li>\n",
            n = note.number,
            html = note.html,
            file = escape_text(&note.file),
        ));
    }
    out.push_str("</ol>\n");
    out
}
