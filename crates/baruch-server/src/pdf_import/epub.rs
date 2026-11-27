// SPDX-License-Identifier: AGPL-3.0-or-later
//! Read EPUB (Round 21a), always locally: archive through [`crate::archive::read_zip`] with
//! [`crate::archive::IMPORT_LIMITS`] (no path escape, no symlinks, limits for size, number and compression ratio),
//! `META-INF/container.xml` → OPF (title, author, language, manifest, reading order), navigation from `nav.xhtml`
//! (EPUB 3) or `toc.ncx` (EPUB 2), text from the XHTML files of the reading order.
//!
//! Conversion to Typst: paragraphs, emphasis (`_…_`/`*…*`), footnotes (`epub:type="noteref"` or
//! `role="doc-noteref"` to an element with `footnote`/`endnote`) as `#footnote[…]`, lists, quotes, code, images
//! as an image of the work (`#image("pfad")` or `#figure` with caption) – path as in the EPUB relative to the OPF
//! file. What cannot be mapped (tables as rows, SVG/MathML/video in the text, headings outside of the
//! navigation) is in the report as an [`ImportWarning`]; no text is lost in the process.

use std::collections::{HashMap, HashSet};

use super::Extracted;
use super::typst_out::{Inline, Section, inline_is_blank, plain_inline, render_inline, string_literal, trim_inline};
use super::xml::{self, Element, Node};
use crate::archive::{IMPORT_LIMITS, read_zip};
use crate::import::{ImportWarning, MAX_IMPORT_ASSET_BYTES, SkippedAsset};

fn warn(code: &str, message: impl Into<String>) -> ImportWarning {
    ImportWarning { code: code.into(), message: message.into(), line: None }
}

/// Decode `%xx` in a reference (invalid sequences are left as they are).
fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let Ok(b) = u8::from_str_radix(&text[i + 1..i + 3], 16)
        {
            out.push(b);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Resolve the reference `href` relative to the folder `dir` (without a trailing `/`, empty = root): (path in the
/// archive, anchor). Addresses (`http:` …), absolute paths and escape above the root → `None`.
pub(crate) fn resolve(dir: &str, href: &str) -> Option<(String, String)> {
    let href = href.trim();
    if href.contains("://") || href.starts_with("mailto:") || href.starts_with("data:") || href.starts_with('/') {
        return None;
    }
    let (path, fragment) = href.split_once('#').unwrap_or((href, ""));
    let path = path.split('?').next().unwrap_or("");
    let decoded = percent_decode(path);
    let mut parts: Vec<&str> = if dir.is_empty() { Vec::new() } else { dir.split('/').collect() };
    if decoded.is_empty() {
        return Some((String::new(), percent_decode(fragment)));
    }
    for part in decoded.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            name => parts.push(name),
        }
    }
    Some((parts.join("/"), percent_decode(fragment)))
}

fn dir_of(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(dir, _)| dir)
}

fn has_token(value: Option<&str>, tokens: &[&str]) -> bool {
    value.is_some_and(|v| v.split_whitespace().any(|t| tokens.contains(&t.to_ascii_lowercase().as_str())))
}

/// Is the element a footnote or endnote (target of a reference)?
fn is_note(e: &Element) -> bool {
    has_token(e.prefixed_attr("type"), &["footnote", "endnote", "rearnote", "note"])
        || has_token(e.attr("role"), &["doc-footnote", "doc-endnote"])
}

/// Is the reference a footnote mark?
fn is_noteref(e: &Element) -> bool {
    has_token(e.prefixed_attr("type"), &["noteref"]) || has_token(e.attr("role"), &["doc-noteref"])
}

fn is_backlink(e: &Element) -> bool {
    has_token(e.prefixed_attr("type"), &["backlink"]) || has_token(e.attr("role"), &["doc-backlink"])
}

const BLOCKS: &[&str] = &[
    "p",
    "div",
    "section",
    "article",
    "main",
    "header",
    "footer",
    "aside",
    "nav",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "ul",
    "ol",
    "li",
    "dl",
    "dt",
    "dd",
    "blockquote",
    "figure",
    "figcaption",
    "table",
    "pre",
    "hr",
    "address",
    "hgroup",
    "body",
    "html",
    "form",
    "fieldset",
    "details",
    "summary",
    "caption",
    "thead",
    "tbody",
    "tfoot",
    "tr",
    "td",
    "th",
];

const UNSUPPORTED: &[&str] = &["svg", "math", "video", "audio", "iframe", "object", "embed", "canvas"];

/// A piece of the text flow.
#[derive(Debug, Clone)]
enum Flow {
    /// Jump target (`pfad` or `pfad#id`).
    Anchor(String),
    Heading {
        level: u8,
        text: String,
    },
    Block {
        typst: String,
        plain: String,
    },
}

/// Image of the EPUB, as an image of the work.
#[derive(Debug, Clone)]
pub(crate) struct EpubAsset {
    pub path: String,
    pub file_name: String,
    pub bytes: Vec<u8>,
}

struct Reader<'a> {
    files: HashMap<String, &'a [u8]>,
    /// Folder of the OPF file (image paths in the work relative to it).
    root: String,
    /// Footnotes of all files: `pfad#id` → element.
    notes: HashMap<String, Element>,
    /// Targets of footnote marks (these elements are missing in the text flow).
    referenced: HashSet<String>,
    warnings: Vec<ImportWarning>,
    warned: HashSet<String>,
    skipped: Vec<SkippedAsset>,
    assets: Vec<EpubAsset>,
    /// Archive path → path in the work.
    asset_paths: HashMap<String, String>,
    asset_bytes: u64,
    flow: Vec<Flow>,
    /// Running paragraph.
    inline: Vec<Inline>,
    /// Jump targets in the running paragraph (follow after it).
    deferred: Vec<String>,
    /// Images in the running paragraph (as blocks of their own after it).
    images: Vec<String>,
    /// File that is being read right now.
    path: String,
}

impl<'a> Reader<'a> {
    fn warn_once(&mut self, key: &str, code: &str, message: String) {
        if self.warned.insert(key.to_owned()) {
            self.warnings.push(warn(code, message));
        }
    }

    fn file(&self, path: &str) -> Option<&'a [u8]> {
        self.files.get(path).copied().or_else(|| {
            let lower = path.to_lowercase();
            self.files.iter().find(|(k, _)| k.to_lowercase() == lower).map(|(_, v)| *v)
        })
    }

    fn flush(&mut self) {
        let pieces = trim_inline(std::mem::take(&mut self.inline));
        if !inline_is_blank(&pieces) {
            let typst = render_inline(&pieces, true);
            let plain = plain_inline(&pieces).split_whitespace().collect::<Vec<_>>().join(" ");
            self.flow.push(Flow::Block { typst, plain });
        }
        for image in std::mem::take(&mut self.images) {
            self.flow.push(Flow::Block { typst: image, plain: String::new() });
        }
        for anchor in std::mem::take(&mut self.deferred) {
            self.flow.push(Flow::Anchor(anchor));
        }
    }

    fn anchor_of(&mut self, e: &Element) {
        if let Some(id) = e.attr("id").map(str::trim).filter(|id| !id.is_empty()) {
            let key = format!("{}#{id}", self.path);
            if self.inline.is_empty() {
                self.flow.push(Flow::Anchor(key));
            } else {
                self.deferred.push(key);
            }
        }
    }

    /// Take over an image from the EPUB; Typst call or `None` (skipped, with a reason).
    fn image(&mut self, e: &Element, caption: Option<&str>) -> Option<String> {
        let src = e.attr("src").or_else(|| e.attr("href")).or_else(|| e.attr("xlink:href"))?.to_owned();
        let Some((archive_path, _)) = resolve(dir_of(&self.path), &src) else {
            self.skipped
                .push(SkippedAsset { path: src, reason: "keine Datei im EPUB (Adresse oder Pfad außerhalb)".into() });
            return None;
        };
        let path = match self.asset_paths.get(&archive_path) {
            Some(path) => path.clone(),
            None => {
                let Some(bytes) = self.file(&archive_path) else {
                    self.skipped.push(SkippedAsset { path: src, reason: "nicht im EPUB".into() });
                    return None;
                };
                if bytes.len() > baruch_store::MAX_ASSET_BYTES {
                    self.skipped.push(SkippedAsset { path: src, reason: "größer als 8 MiB".into() });
                    return None;
                }
                if baruch_store::detect_mime(bytes).is_none() {
                    self.skipped.push(SkippedAsset {
                        path: src,
                        reason: "kein unterstütztes Bild (PNG, JPEG, GIF, WebP, SVG)".into(),
                    });
                    return None;
                }
                if self.asset_bytes + bytes.len() as u64 > MAX_IMPORT_ASSET_BYTES {
                    self.skipped.push(SkippedAsset {
                        path: src,
                        reason: "die Bilder dieser Datei sind zusammen größer als 64 MiB".into(),
                    });
                    return None;
                }
                let relative = archive_path
                    .strip_prefix(&format!("{}/", self.root))
                    .filter(|_| !self.root.is_empty())
                    .unwrap_or(&archive_path)
                    .to_owned();
                let path = match baruch_store::check_asset_path(&relative) {
                    Ok(path) => path,
                    Err(e) => {
                        self.skipped.push(SkippedAsset { path: src, reason: e.to_string() });
                        return None;
                    }
                };
                self.asset_bytes += bytes.len() as u64;
                let file_name = path.rsplit('/').next().unwrap_or("bild").to_owned();
                self.assets.push(EpubAsset { path: path.clone(), file_name, bytes: bytes.to_vec() });
                self.asset_paths.insert(archive_path, path.clone());
                path
            }
        };
        let alt = e.attr("alt").map(|a| a.split_whitespace().collect::<Vec<_>>().join(" ")).filter(|a| !a.is_empty());
        let mut call = format!("image({}", string_literal(&path));
        if let Some(alt) = alt {
            call.push_str(", alt: ");
            call.push_str(&string_literal(&alt));
        }
        call.push(')');
        Some(match caption.filter(|c| !c.is_empty()) {
            Some(caption) => format!("#figure({call}, caption: [{caption}])"),
            None => format!("#{call}"),
        })
    }

    /// Content of an element as inline pieces.
    fn inline_children(&mut self, e: &Element) -> Vec<Inline> {
        let mut out = Vec::new();
        for node in &e.children {
            match node {
                Node::Text(t) => out.push(Inline::Text(collapse(t))),
                Node::Element(child) => out.extend(self.inline_element(child)),
            }
        }
        out
    }

    fn inline_element(&mut self, e: &Element) -> Vec<Inline> {
        let key = e.attr("id").map(|id| format!("{}#{id}", self.path));
        if key.as_ref().is_some_and(|k| self.referenced.contains(k)) && is_note(e) {
            return Vec::new();
        }
        if has_token(e.prefixed_attr("type"), &["pagebreak"]) || has_token(e.attr("role"), &["doc-pagebreak"]) {
            return Vec::new();
        }
        match e.name.as_str() {
            "em" | "i" | "cite" | "dfn" | "var" => vec![Inline::Emph(self.inline_children(e))],
            "strong" | "b" => vec![Inline::Strong(self.inline_children(e))],
            "sup" => {
                let inner = self.inline_children(e);
                let only_notes = inner
                    .iter()
                    .all(|p| matches!(p, Inline::Footnote(_)) || matches!(p, Inline::Text(t) if t.trim().is_empty()));
                if only_notes && inner.iter().any(|p| matches!(p, Inline::Footnote(_))) {
                    inner.into_iter().filter(|p| matches!(p, Inline::Footnote(_))).collect()
                } else {
                    vec![Inline::Super(inner)]
                }
            }
            "sub" => vec![Inline::Sub(self.inline_children(e))],
            "code" | "kbd" | "samp" | "tt" => vec![Inline::Code(e.text())],
            "br" => vec![Inline::Break],
            "img" | "image" => {
                if let Some(call) = self.image(e, None) {
                    self.images.push(call);
                }
                Vec::new()
            }
            "a" => self.link(e),
            "script" | "style" | "head" | "title" => Vec::new(),
            name if UNSUPPORTED.contains(&name) => {
                self.warn_once(
                    &format!("unsupported-{name}"),
                    "import-epub-unsupported",
                    format!("<{name}> im Text lässt sich nicht nach Typst übernehmen; sein Text (falls vorhanden) bleibt stehen."),
                );
                let text = e.text();
                if text.is_empty() { Vec::new() } else { vec![Inline::Text(text)] }
            }
            _ => self.inline_children(e),
        }
    }

    fn link(&mut self, e: &Element) -> Vec<Inline> {
        let href = e.attr("href").unwrap_or("").trim().to_owned();
        if is_backlink(e) {
            return Vec::new();
        }
        let target = resolve(dir_of(&self.path), &href).map(|(path, fragment)| {
            let path = if path.is_empty() { self.path.clone() } else { path };
            format!("{path}#{fragment}")
        });
        let points_at_note = target.as_ref().is_some_and(|t| self.notes.contains_key(t));
        if is_noteref(e) || points_at_note {
            match target.as_ref().and_then(|t| self.notes.get(t)).cloned() {
                Some(note) => {
                    // Images in the footnote stay in the footnote (not after the paragraph, not lost).
                    let saved = std::mem::take(&mut self.images);
                    let mut content = self.note_content(&note, &e.text());
                    for call in std::mem::replace(&mut self.images, saved) {
                        if !content.is_empty() {
                            content.push(Inline::Text(" ".into()));
                        }
                        content.push(Inline::Typst(call));
                    }
                    return vec![Inline::Footnote(content)];
                }
                None => {
                    self.warn_once(
                        &format!("note-{href}"),
                        "import-epub-footnote-missing",
                        format!("Fußnotenzeichen „{}“ verweist auf {href}, das es im EPUB nicht gibt; das Zeichen bleibt als Text.", e.text()),
                    );
                    return self.inline_children(e);
                }
            }
        }
        let lower = href.to_ascii_lowercase();
        if lower.starts_with("https://") || lower.starts_with("http://") {
            let label = self.inline_children(e);
            return vec![Inline::Link(href, label)];
        }
        self.inline_children(e)
    }

    /// Content of a footnote (paragraphs joined with spaces, back links removed). A leading number is dropped only
    /// if it is the number of the footnote mark `label` and no back link already carried the number – otherwise
    /// it belongs to the text („1 Kor 13,4“, „1984 erschien …“). Dropping reports a warning once per import.
    fn note_content(&mut self, note: &Element, label: &str) -> Vec<Inline> {
        let mut out = trim_inline(self.note_pieces(note));
        let number: String = label.trim().trim_matches(['[', ']', '(', ')']).trim().to_owned();
        let numeric = !number.is_empty() && number.chars().all(|c| c.is_ascii_digit());
        let mut anchors = Vec::new();
        note.find_all("a", &mut anchors);
        let backlink_carries_number =
            anchors.iter().any(|a| is_backlink(a) && a.text().trim().trim_matches(['[', ']']) == number);
        if numeric && !backlink_carries_number {
            let mut stripped = false;
            if let Some(Inline::Text(t)) = out.first_mut() {
                let digits = t.chars().take_while(|c| c.is_ascii_digit()).count();
                let after = &t[digits..];
                if t[..digits] == number
                    && (after.is_empty() || after.starts_with(['.', ')']) || after.starts_with(char::is_whitespace))
                {
                    *t = after.trim_start_matches(['.', ')']).to_owned();
                    stripped = true;
                }
            }
            if stripped {
                self.warn_once(
                    "footnote-number",
                    "import-epub-footnote-number",
                    format!(
                        "Am Anfang von Fußnoten wurde die Fußnotennummer entfernt (zuerst „{number}“, gleich dem \
                         Fußnotenzeichen). Beginnt eine Fußnote wirklich mit dieser Zahl (etwa „1 Kor“), bitte nachtragen."
                    ),
                );
            }
        }
        trim_inline(out)
    }

    /// Pieces of a footnote (recursively over blocks).
    fn note_pieces(&mut self, note: &Element) -> Vec<Inline> {
        let mut out = Vec::new();
        for node in &note.children {
            match node {
                Node::Text(t) => out.push(Inline::Text(collapse(t))),
                Node::Element(child) if BLOCKS.contains(&child.name.as_str()) => {
                    if !out.is_empty() {
                        out.push(Inline::Text(" ".into()));
                    }
                    out.extend(self.note_pieces(child));
                }
                Node::Element(child) => out.extend(self.inline_element(child)),
            }
        }
        out
    }

    /// An element in block context.
    fn block(&mut self, e: &Element) {
        let key = e.attr("id").map(|id| format!("{}#{id}", self.path));
        if is_note(e) && key.as_ref().is_some_and(|k| self.referenced.contains(k)) {
            return;
        }
        self.anchor_of(e);
        match e.name.as_str() {
            "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                self.flush();
                let level = e.name[1..].parse::<u8>().unwrap_or(1);
                let text = e.text();
                if !text.is_empty() {
                    self.flow.push(Flow::Heading { level, text });
                }
                // Images in a heading come after it.
                let mut found = Vec::new();
                e.find_all("img", &mut found);
                for img in found {
                    if let Some(call) = self.image(img, None) {
                        self.flow.push(Flow::Block { typst: call, plain: String::new() });
                    }
                }
            }
            "p" | "dt" | "dd" | "figcaption" | "caption" | "address" | "summary" => {
                self.flush();
                let pieces = self.inline_children(e);
                self.inline = pieces;
                self.flush();
            }
            "ul" | "ol" => {
                self.flush();
                let mut lines = Vec::new();
                let mut plain = Vec::new();
                self.list(e, 0, &mut lines, &mut plain);
                if !lines.is_empty() {
                    self.flow.push(Flow::Block { typst: lines.join("\n"), plain: plain.join("\n") });
                }
                self.flush();
            }
            "blockquote" => {
                self.flush();
                let start = self.flow.len();
                self.container(e);
                self.flush();
                let inner: Vec<Flow> = self.flow.drain(start..).collect();
                let mut typst = Vec::new();
                let mut plain = Vec::new();
                let mut rest = Vec::new();
                for item in inner {
                    match item {
                        Flow::Block { typst: t, plain: p } => {
                            typst.push(t);
                            plain.push(p);
                        }
                        other => rest.push(other),
                    }
                }
                self.flow.extend(rest);
                if !typst.is_empty() {
                    self.flow.push(Flow::Block {
                        typst: format!("#quote(block: true)[\n{}\n]", typst.join("\n\n")),
                        plain: plain.join("\n\n"),
                    });
                }
            }
            "figure" => {
                self.flush();
                let caption = e.child("figcaption").map(|c| {
                    let pieces = trim_inline(self.inline_children(c));
                    render_inline(&pieces, false)
                });
                let caption_plain = e.child("figcaption").map(Element::text).unwrap_or_default();
                let mut found = Vec::new();
                e.find_all("img", &mut found);
                let mut first = true;
                for img in found {
                    let call = if first { self.image(img, caption.as_deref()) } else { self.image(img, None) };
                    first = false;
                    if let Some(call) = call {
                        self.flow.push(Flow::Block { typst: call, plain: String::new() });
                    }
                }
                if !caption_plain.is_empty()
                    && self
                        .flow
                        .last()
                        .is_none_or(|f| !matches!(f, Flow::Block { typst, .. } if typst.starts_with("#figure")))
                {
                    // Caption without an image taken over: keep as text.
                    self.flow.push(Flow::Block {
                        typst: super::typst_out::escape_text(&caption_plain, true),
                        plain: caption_plain,
                    });
                }
                for child in e.elements().filter(|c| c.name != "figcaption" && c.name != "img") {
                    if BLOCKS.contains(&child.name.as_str()) {
                        self.block(child);
                    }
                }
            }
            "table" => {
                self.flush();
                self.warn_once(
                    &format!("table-{}", self.path),
                    "import-epub-table",
                    "Tabellen übernimmt der Import als Zeilen Text (Zellen mit „ | “ getrennt); die Tabellenform fehlt."
                        .into(),
                );
                let mut rows = Vec::new();
                e.find_all("tr", &mut rows);
                for row in rows {
                    let cells: Vec<String> =
                        row.elements().filter(|c| c.name == "td" || c.name == "th").map(Element::text).collect();
                    let line = cells.join(" | ");
                    if !line.trim().is_empty() {
                        self.flow.push(Flow::Block { typst: super::typst_out::escape_text(&line, true), plain: line });
                    }
                }
            }
            "pre" => {
                self.flush();
                let mut text = String::new();
                raw_text(e, &mut text);
                if !text.trim().is_empty() {
                    self.flow
                        .push(Flow::Block { typst: format!("#raw(block: true, {})", raw_literal(&text)), plain: text });
                }
            }
            "hr" => {
                self.flush();
                self.flow.push(Flow::Block { typst: "#line(length: 100%)".into(), plain: String::new() });
            }
            "nav" if has_token(e.prefixed_attr("type"), &["toc", "landmarks", "page-list"]) => {
                self.flush();
            }
            "script" | "style" | "head" => {}
            _ => {
                self.flush();
                self.container(e);
                self.flush();
            }
        }
    }

    /// Children of a block container: text and inline elements form paragraphs, block elements blocks of their own.
    fn container(&mut self, e: &Element) {
        for node in &e.children {
            match node {
                Node::Text(t) => {
                    let text = collapse(t);
                    if !text.trim().is_empty() || !self.inline.is_empty() {
                        self.inline.push(Inline::Text(text));
                    }
                }
                Node::Element(child) if BLOCKS.contains(&child.name.as_str()) => {
                    self.flush();
                    self.block(child);
                }
                Node::Element(child) => {
                    self.anchor_of(child);
                    let pieces = self.inline_element(child);
                    self.inline.extend(pieces);
                }
            }
        }
    }

    fn list(&mut self, e: &Element, depth: usize, lines: &mut Vec<String>, plain: &mut Vec<String>) {
        let marker = if e.name == "ol" { "+" } else { "-" };
        for li in e.elements().filter(|c| c.name == "li") {
            let mut pieces = Vec::new();
            let mut nested = Vec::new();
            for node in &li.children {
                match node {
                    Node::Text(t) => pieces.push(Inline::Text(collapse(t))),
                    Node::Element(c) if c.name == "ul" || c.name == "ol" => nested.push(c),
                    Node::Element(c) if BLOCKS.contains(&c.name.as_str()) => {
                        if !pieces.is_empty() {
                            pieces.push(Inline::Text(" ".into()));
                        }
                        let inner = self.inline_children(c);
                        pieces.extend(inner);
                    }
                    Node::Element(c) => pieces.extend(self.inline_element(c)),
                }
            }
            let pieces = trim_inline(pieces);
            let indent = "  ".repeat(depth);
            lines.push(format!("{indent}{marker} {}", render_inline(&pieces, false)));
            plain.push(plain_inline(&pieces).split_whitespace().collect::<Vec<_>>().join(" "));
            for list in nested {
                self.list(list, depth + 1, lines, plain);
            }
        }
        // Images in list items follow after the list (flush in block()).
    }
}

fn raw_text(e: &Element, out: &mut String) {
    for node in &e.children {
        match node {
            Node::Text(t) => out.push_str(t),
            Node::Element(c) if c.name == "br" => out.push('\n'),
            Node::Element(c) => raw_text(c, out),
        }
    }
}

/// String for `#raw(…)` with line breaks kept.
fn raw_literal(text: &str) -> String {
    let mut out = String::from("\"");
    for c in text.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => {}
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Collapse whitespace as in HTML (non-breaking space stays).
fn collapse(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut space = false;
    for c in text.chars() {
        if matches!(c, ' ' | '\t' | '\n' | '\r' | '\x0c') {
            if !space {
                out.push(' ');
            }
            space = true;
        } else {
            out.push(c);
            space = false;
        }
    }
    out
}

/// Entry of the navigation.
#[derive(Debug, Clone)]
struct NavEntry {
    depth: usize,
    title: String,
    /// `pfad` or `pfad#anker` (without a target: `None`).
    target: Option<String>,
}

fn nav_target(dir: &str, href: &str) -> Option<String> {
    let (path, fragment) = resolve(dir, href)?;
    if path.is_empty() {
        return None;
    }
    Some(if fragment.is_empty() { path } else { format!("{path}#{fragment}") })
}

fn nav_list(ol: &Element, dir: &str, depth: usize, out: &mut Vec<NavEntry>) {
    for li in ol.elements().filter(|c| c.name == "li") {
        let link = li.elements().find(|c| c.name == "a" || c.name == "span");
        if let Some(link) = link {
            let title = link.text();
            let target = link.attr("href").and_then(|h| nav_target(dir, h));
            if !title.is_empty() {
                out.push(NavEntry { depth, title, target });
            }
        }
        if let Some(nested) = li.child("ol").or_else(|| li.child("ul")) {
            nav_list(nested, dir, depth + 1, out);
        }
    }
}

fn ncx_points(parent: &Element, dir: &str, depth: usize, out: &mut Vec<NavEntry>) {
    for point in parent.elements().filter(|c| c.name == "navpoint") {
        let title = point.child("navlabel").map(Element::text).unwrap_or_default();
        let target = point.child("content").and_then(|c| c.attr("src")).and_then(|h| nav_target(dir, h));
        if !title.is_empty() {
            out.push(NavEntry { depth, title, target });
        }
        ncx_points(point, dir, depth + 1, out);
    }
}

fn normalized(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}

fn looks_like_part(title: &str) -> bool {
    let lower = title.trim().to_lowercase();
    ["teil ", "part ", "buch "].iter().any(|p| lower.starts_with(p))
}

/// Read an EPUB. Errors (not an archive, unsafe paths, limits, no OPF) → message, nothing read.
pub(crate) fn read(bytes: &[u8]) -> Result<Extracted, String> {
    let entries = read_zip(bytes, &IMPORT_LIMITS).map_err(|e| format!("EPUB abgelehnt: {e}"))?;
    let mut warnings = Vec::new();
    let files: HashMap<String, &[u8]> = entries.iter().map(|e| (e.path.clone(), e.bytes.as_slice())).collect();
    match files.get("mimetype") {
        Some(m) if m.trim_ascii() == b"application/epub+zip" => {}
        Some(_) => warnings.push(warn(
            "import-epub-mimetype",
            "Die Datei „mimetype“ nennt nicht application/epub+zip; gelesen wird trotzdem.",
        )),
        None => warnings.push(warn("import-epub-mimetype", "Die Datei „mimetype“ fehlt; gelesen wird trotzdem.")),
    }
    let text = |path: &str| -> Result<String, String> {
        let raw = files.get(path).ok_or_else(|| format!("EPUB unvollständig: {path} fehlt."))?;
        String::from_utf8(raw.to_vec()).map_err(|_| format!("EPUB: {path} ist kein UTF-8-Text."))
    };
    let container = xml::parse(&text("META-INF/container.xml")?).map_err(|e| format!("EPUB: container.xml: {e}"))?;
    let opf_path = container
        .find("rootfile")
        .and_then(|r| r.attr("full-path"))
        .map(str::to_owned)
        .ok_or_else(|| "EPUB: container.xml nennt keine OPF-Datei (rootfile full-path).".to_owned())?;
    let opf_path =
        resolve("", &opf_path).map(|(p, _)| p).ok_or_else(|| "EPUB: Pfad der OPF-Datei ungültig.".to_owned())?;
    let opf = xml::parse(&text(&opf_path)?).map_err(|e| format!("EPUB: {opf_path}: {e}"))?;
    let root = dir_of(&opf_path).to_owned();
    let metadata = opf.find("metadata");
    let meta_text = |name: &str| metadata.and_then(|m| m.find(name)).map(Element::text).filter(|t| !t.is_empty());
    let title = meta_text("title");
    let author = metadata
        .map(|m| {
            let mut creators = Vec::new();
            m.find_all("creator", &mut creators);
            creators.iter().map(|c| c.text()).filter(|t| !t.is_empty()).collect::<Vec<_>>().join("; ")
        })
        .filter(|a| !a.is_empty());
    let lang = meta_text("language").map(|l| l.split(['-', '_']).next().unwrap_or("").to_lowercase());

    // Manifest and reading order.
    struct Item {
        path: String,
        media: String,
        properties: String,
    }
    let mut manifest: HashMap<String, Item> = HashMap::new();
    if let Some(m) = opf.find("manifest") {
        for item in m.elements().filter(|e| e.name == "item") {
            let (Some(id), Some(href)) = (item.attr("id"), item.attr("href")) else { continue };
            let Some((path, _)) = resolve(&root, href) else { continue };
            manifest.insert(
                id.to_owned(),
                Item {
                    path,
                    media: item.attr("media-type").unwrap_or("").to_owned(),
                    properties: item.attr("properties").unwrap_or("").to_owned(),
                },
            );
        }
    }
    let spine_el = opf.find("spine");
    let spine: Vec<String> = spine_el
        .map(|s| {
            s.elements()
                .filter(|e| e.name == "itemref")
                .filter_map(|e| e.attr("idref"))
                .filter_map(|id| manifest.get(id))
                .filter(|item| {
                    item.media.contains("html") || item.path.ends_with(".xhtml") || item.path.ends_with(".html")
                })
                .map(|item| item.path.clone())
                .collect()
        })
        .unwrap_or_default();
    if spine.is_empty() {
        return Err("EPUB: Die Lesereihenfolge (spine) nennt keine XHTML-Datei.".into());
    }

    // Navigation: EPUB 3 nav, otherwise NCX.
    let mut nav: Vec<NavEntry> = Vec::new();
    let mut nav_path: Option<String> = None;
    if let Some(item) = manifest.values().find(|i| i.properties.split_whitespace().any(|p| p == "nav")) {
        nav_path = Some(item.path.clone());
        if let Ok(source) = text(&item.path)
            && let Ok(doc) = xml::parse(&source)
        {
            let mut navs = Vec::new();
            doc.find_all("nav", &mut navs);
            let toc = navs.iter().find(|n| has_token(n.prefixed_attr("type"), &["toc"])).or(navs.first());
            if let Some(list) = toc.and_then(|n| n.find("ol")) {
                nav_list(list, dir_of(&item.path), 1, &mut nav);
            }
        }
    }
    if nav.is_empty() {
        let ncx = spine_el
            .and_then(|s| s.attr("toc"))
            .and_then(|id| manifest.get(id))
            .or_else(|| manifest.values().find(|i| i.media == "application/x-dtbncx+xml"));
        if let Some(item) = ncx
            && let Ok(source) = text(&item.path)
            && let Ok(doc) = xml::parse(&source)
            && let Some(map) = doc.find("navmap")
        {
            ncx_points(map, dir_of(&item.path), 1, &mut nav);
        }
    }

    // First pass: footnotes and their marks.
    let mut reader = Reader {
        files: files.clone(),
        root: root.clone(),
        notes: HashMap::new(),
        referenced: HashSet::new(),
        warnings,
        warned: HashSet::new(),
        skipped: Vec::new(),
        assets: Vec::new(),
        asset_paths: HashMap::new(),
        asset_bytes: 0,
        flow: Vec::new(),
        inline: Vec::new(),
        deferred: Vec::new(),
        images: Vec::new(),
        path: String::new(),
    };
    let mut docs: Vec<(String, Element)> = Vec::new();
    for path in &spine {
        if Some(path) == nav_path.as_ref() && !nav.is_empty() {
            // The table of contents itself is navigation, not text.
            continue;
        }
        let source = match text(path) {
            Ok(source) => source,
            Err(e) => {
                reader.warnings.push(warn("import-epub-missing", format!("{e} Die Datei fehlt im Text.")));
                continue;
            }
        };
        match xml::parse(&source) {
            Ok(doc) => docs.push((path.clone(), doc)),
            Err(e) => reader.warnings.push(warn(
                "import-epub-unreadable",
                format!("{path} ist kein lesbares XHTML ({e}); die Datei fehlt im Text."),
            )),
        }
    }
    fn collect_notes(e: &Element, path: &str, notes: &mut HashMap<String, Element>, refs: &mut Vec<String>) {
        if is_note(e)
            && let Some(id) = e.attr("id")
        {
            notes.insert(format!("{path}#{id}"), e.clone());
        }
        if e.name == "a"
            && let Some(href) = e.attr("href")
        {
            refs.push(href.to_owned());
        }
        for c in e.elements() {
            collect_notes(c, path, notes, refs);
        }
    }
    let mut noterefs: Vec<(String, String)> = Vec::new();
    for (path, doc) in &docs {
        let mut refs = Vec::new();
        collect_notes(doc, path, &mut reader.notes, &mut refs);
        noterefs.extend(refs.into_iter().map(|r| (path.clone(), r)));
    }
    for (path, href) in noterefs {
        if let Some((target, fragment)) = resolve(dir_of(&path), &href) {
            let target = if target.is_empty() { path.clone() } else { target };
            let key = format!("{target}#{fragment}");
            if reader.notes.contains_key(&key) {
                reader.referenced.insert(key);
            }
        }
    }

    // Second pass: text flow.
    for (path, doc) in &docs {
        reader.path = path.clone();
        reader.flow.push(Flow::Anchor(path.clone()));
        let body = doc.find("body").unwrap_or(doc);
        reader.anchor_of(body);
        reader.container(body);
        reader.flush();
    }

    let flow = std::mem::take(&mut reader.flow);
    let positions: HashMap<&str, usize> = flow
        .iter()
        .enumerate()
        .filter_map(|(i, f)| match f {
            Flow::Anchor(key) => Some((key.as_str(), i)),
            _ => None,
        })
        .rev()
        .collect();
    let mut warnings = std::mem::take(&mut reader.warnings);
    let plain_text = flow
        .iter()
        .filter_map(|f| match f {
            Flow::Block { plain, .. } if !plain.trim().is_empty() => Some(plain.as_str()),
            Flow::Heading { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n\n");

    // Sections: from the navigation, otherwise from the headings.
    let mut cuts: Vec<(usize, u8, String, bool)> = Vec::new();
    if !nav.is_empty() {
        let deepest = nav.iter().map(|n| n.depth).max().unwrap_or(1);
        if deepest > 3 {
            warnings.push(warn(
                "import-epub-deep-nav",
                format!("Die Navigation hat {deepest} Ebenen; Baruch kennt drei – tiefere Punkte stehen auf Ebene 3."),
            ));
        }
        let mut last = 0usize;
        for (i, entry) in nav.iter().enumerate() {
            let found = entry.target.as_deref().and_then(|t| positions.get(t).copied());
            let pos = match found {
                Some(pos) if pos >= last => pos,
                Some(_) => {
                    warnings.push(warn(
                        "import-epub-nav-order",
                        format!("„{}“ steht in der Navigation an anderer Stelle als im Text; der Punkt bleibt in der Reihenfolge der Navigation (ohne eigenen Text).", entry.title),
                    ));
                    last
                }
                None => {
                    warnings.push(warn(
                        "import-epub-nav-target-missing",
                        format!(
                            "„{}“: das Ziel der Navigation fehlt im Text; der Punkt bleibt ohne eigenen Text.",
                            entry.title
                        ),
                    ));
                    last
                }
            };
            last = pos;
            let has_children = nav.get(i + 1).is_some_and(|n| n.depth > entry.depth);
            let part = entry.depth == 1 && has_children && looks_like_part(&entry.title);
            cuts.push((pos, entry.depth.min(3) as u8, entry.title.clone(), part));
        }
    } else {
        let min_level = flow
            .iter()
            .filter_map(|f| match f {
                Flow::Heading { level, .. } => Some(*level),
                _ => None,
            })
            .min();
        if let Some(min_level) = min_level {
            warnings.push(warn(
                "import-epub-no-nav",
                "Das EPUB hat keine Navigation; die Gliederung folgt seinen Überschriften.",
            ));
            for (i, f) in flow.iter().enumerate() {
                if let Flow::Heading { level, text } = f {
                    let level = (level - min_level + 1).min(3);
                    cuts.push((i, level, text.clone(), level == 1 && looks_like_part(text)));
                }
            }
        }
    }

    let mut sections = Vec::new();
    let mut heading_as_text = 0usize;
    let build = |range: &[Flow], title: Option<&str>, heading_as_text: &mut usize| -> (String, String) {
        let mut typst = Vec::new();
        let mut plain = Vec::new();
        let mut dropped_title = false;
        for f in range {
            match f {
                Flow::Block { typst: t, plain: p } => {
                    typst.push(t.clone());
                    if !p.trim().is_empty() {
                        plain.push(p.clone());
                    }
                }
                Flow::Heading { text, .. } => {
                    if !dropped_title && title.is_some_and(|t| normalized(t) == normalized(text)) {
                        dropped_title = true;
                        continue;
                    }
                    *heading_as_text += 1;
                    typst.push(format!("#strong[{}]", super::typst_out::escape_text(text, false)));
                    plain.push(text.clone());
                }
                Flow::Anchor(_) => {}
            }
        }
        (typst.join("\n\n"), plain.join("\n\n"))
    };
    match cuts.first() {
        None => {
            warnings
                .push(warn("import-no-outline", "Keine Gliederung erkannt: der ganze Text steht in einem Kapitel."));
            let (body, plain) = build(&flow, None, &mut heading_as_text);
            sections.push(Section {
                level: 1,
                title: title.clone().unwrap_or_else(|| "Text".into()),
                part: false,
                body,
                plain,
            });
        }
        Some(&(first, ..)) => {
            let (body, plain) = build(&flow[..first], None, &mut heading_as_text);
            if !plain.trim().is_empty() || !body.trim().is_empty() {
                sections.push(Section { level: 1, title: "Vorspann".into(), part: false, body, plain });
                warnings.push(warn(
                    "import-front-matter",
                    "Text vor dem ersten Punkt der Gliederung steht im Punkt „Vorspann“ (streichen hängt ihn an das erste Kapitel).",
                ));
            }
            for (i, (pos, level, name, part)) in cuts.iter().enumerate() {
                let end = cuts.get(i + 1).map_or(flow.len(), |c| c.0);
                let (body, plain) = build(&flow[*pos..end.max(*pos)], Some(name), &mut heading_as_text);
                sections.push(Section { level: *level, title: name.clone(), part: *part, body, plain });
            }
        }
    }
    if heading_as_text > 0 {
        warnings.push(warn(
            "import-epub-heading-as-text",
            format!("{heading_as_text} Überschrift(en) im Text stehen nicht in der Navigation; sie bleiben fett gesetzter Text, kein Punkt der Gliederung."),
        ));
    }
    let assets = std::mem::take(&mut reader.assets);
    let skipped = std::mem::take(&mut reader.skipped);
    Ok(Extracted {
        title,
        author,
        lang: lang.filter(|l| !l.is_empty()),
        pages: None,
        sections,
        assets: assets.into_iter().map(|a| (a.path, a.file_name, a.bytes)).collect(),
        skipped,
        warnings,
        plain_text,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hrefs_resolve_inside_the_archive_only() {
        assert_eq!(resolve("OEBPS/text", "../bilder/a%20b.png#x"), Some(("OEBPS/bilder/a b.png".into(), "x".into())));
        assert_eq!(resolve("OEBPS", "#fn1"), Some((String::new(), "fn1".into())), "nur Anker: dieselbe Datei");
        assert_eq!(resolve("", "../flucht.png"), None, "über die Wurzel hinaus: kein Ziel");
        assert_eq!(resolve("OEBPS", "https://x.example/a.png"), None, "Adressen sind keine Dateien im Archiv");
        assert_eq!(resolve("OEBPS", "/etc/passwd"), None, "absolute Pfade sind keine Dateien im Archiv");
    }
}
