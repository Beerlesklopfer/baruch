// SPDX-License-Identifier: AGPL-3.0-or-later
//! Reader: .docx → [`DocxDocument`] including tracked changes, comments and images (task docx_import, contract
//! Round 32 §2). Submodules: `read/pack.rs` (archive, relationships), `read/xml.rs` (XML tree), `read/body.rs`
//! (paragraphs, runs, tables → block model).
//!
//! Promises (contract): ZIP only within the [`Limits`] (the sizes in the header are not trusted), no entity
//! resolution, targets of relationships do not leave the folder ([`DocxError::Unsafe`]), external images are
//! never fetched; what is not taken over is in the result as a warning.
//!
//! Warnings carry fixed codes (`docx_formataenderung`, `docx_verschoben`, `docx_textfeld`, `docx_formel`,
//! `docx_objekt`, `docx_tabelle_vereinfacht`, `docx_bild_format`, `docx_bild_extern`, `docx_endnote`,
//! `docx_fussnote_absaetze`, `docx_verborgener_text`, `docx_symbol`, `docx_unbekannt`); per code one warning with
//! the number of places.

mod body;
mod pack;
mod xml;

use std::collections::HashMap;

use crate::{Comment, DocxDocument, DocxError, DocxMeta, DocxWarning, Limits, ooxml};
use pack::Package;
use xml::Element;

/// Result of the reader: the document and everything noticed along the way.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ReadDocx {
    pub document: DocxDocument,
    pub warnings: Vec<DocxWarning>,
}

/// Warnings, summarised per code (first message, number of places).
#[derive(Debug, Default)]
pub(crate) struct Warnings {
    list: Vec<(String, String, usize)>,
}

impl Warnings {
    pub fn add(&mut self, code: &str, message: impl Into<String>) {
        match self.list.iter_mut().find(|(c, _, _)| c == code) {
            Some(entry) => entry.2 += 1,
            None => self.list.push((code.to_owned(), message.into(), 1)),
        }
    }

    fn finish(self) -> Vec<DocxWarning> {
        self.list
            .into_iter()
            .map(|(code, message, count)| DocxWarning {
                code,
                message: if count > 1 { format!("{message} ({count} Stellen)") } else { message },
            })
            .collect()
    }
}

/// Meaning of a paragraph style.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Role {
    Normal,
    Title,
    Subtitle,
    Author,
    Imprint,
    Part,
    Heading { level: u8, numbered: bool },
    Quote,
    QuoteSource,
    Caption,
}

#[derive(Debug, Clone, Default)]
struct Style {
    /// `w:name`, lower case.
    name: String,
    based_on: Option<String>,
    outline: Option<u8>,
}

/// The styles from `styles.xml`, as far as they count for the outline.
#[derive(Debug, Default)]
pub(crate) struct Styles {
    map: HashMap<String, Style>,
}

fn numbered_suffix(text: &str, prefixes: &[&str]) -> Option<u8> {
    prefixes.iter().find_map(|prefix| {
        let rest = text.strip_prefix(prefix)?.trim_start();
        let level: u8 = rest.parse().ok()?;
        (1..=9).contains(&level).then_some(level.min(ooxml::MAX_HEADING_LEVEL))
    })
}

impl Styles {
    fn load(root: &Element) -> Styles {
        let mut map = HashMap::new();
        for style in root.elements().filter(|e| e.name == "w:style") {
            let Some(id) = style.attr("w:styleId") else { continue };
            let outline = style
                .child("w:pPr")
                .and_then(|p| p.child_val("w:outlineLvl"))
                .and_then(|v| v.parse::<u8>().ok())
                .filter(|l| *l < 9);
            map.insert(
                id.to_owned(),
                Style {
                    name: style.child_val("w:name").unwrap_or("").to_lowercase(),
                    based_on: style.child_val("w:basedOn").map(str::to_owned),
                    outline,
                },
            );
        }
        Styles { map }
    }

    fn own_role(id: &str, style: Option<&Style>) -> Option<Role> {
        let name = style.map_or("", |s| s.name.as_str());
        if id == ooxml::STYLE_PART {
            return Some(Role::Part);
        }
        if let Some(level) = numbered_suffix(id, &[ooxml::STYLE_UNNUMBERED_PREFIX]) {
            return Some(Role::Heading { level: level.min(ooxml::MAX_OUTLINE_LEVEL), numbered: false });
        }
        let fixed = [
            (ooxml::STYLE_AUTHOR, Role::Author),
            (ooxml::STYLE_IMPRINT, Role::Imprint),
            (ooxml::STYLE_QUOTE_SOURCE, Role::QuoteSource),
        ];
        if let Some((_, role)) = fixed.iter().find(|(fixed_id, _)| *fixed_id == id) {
            return Some(*role);
        }
        if id == ooxml::STYLE_TITLE || matches!(name, "title" | "titel") {
            return Some(Role::Title);
        }
        if id == ooxml::STYLE_SUBTITLE || matches!(name, "subtitle" | "untertitel") {
            return Some(Role::Subtitle);
        }
        if let Some(level) = numbered_suffix(id, &[ooxml::STYLE_HEADING_PREFIX])
            .or_else(|| numbered_suffix(name, &["heading", "überschrift", "ueberschrift"]))
        {
            return Some(Role::Heading { level, numbered: true });
        }
        if id == ooxml::STYLE_QUOTE
            || id == "IntenseQuote"
            || matches!(name, "quote" | "zitat" | "intense quote" | "intensives zitat")
        {
            return Some(Role::Quote);
        }
        if id == ooxml::STYLE_CAPTION || matches!(name, "caption" | "beschriftung") {
            return Some(Role::Caption);
        }
        style.and_then(|s| s.outline).map(|outline| Role::Heading { level: (outline + 1).min(6), numbered: true })
    }

    /// Role of the style `id` (inherited through `basedOn`); `outline` is `w:outlineLvl` directly on the paragraph.
    pub(crate) fn role(&self, id: Option<&str>, outline: Option<u8>) -> Role {
        let mut current = id.map(str::to_owned);
        for _ in 0..16 {
            let Some(id) = current else { break };
            let style = self.map.get(&id);
            if let Some(role) = Self::own_role(&id, style) {
                return role;
            }
            current = style.and_then(|s| s.based_on.clone());
        }
        match outline {
            Some(level) if level < 9 => Role::Heading { level: (level + 1).min(6), numbered: true },
            _ => Role::Normal,
        }
    }
}

/// Lists from `numbering.xml`: per `numId` and level, whether ordered and with which first number.
#[derive(Debug, Default)]
pub(crate) struct Numbering {
    nums: HashMap<String, String>,
    levels: HashMap<(String, u8), (bool, u64)>,
    /// Deviating first number per `numId` and level (`w:lvlOverride`/`w:startOverride`): this is how Word restarts
    /// a list or starts it at another number without creating an abstract numbering of its own.
    overrides: HashMap<(String, u8), u64>,
    known: bool,
}

impl Numbering {
    fn load(root: &Element) -> Numbering {
        let mut numbering = Numbering { known: true, ..Numbering::default() };
        for e in root.elements() {
            match e.name.as_str() {
                "w:abstractNum" => {
                    let Some(id) = e.attr("w:abstractNumId") else { continue };
                    for lvl in e.elements().filter(|l| l.name == "w:lvl") {
                        let level = lvl.attr("w:ilvl").and_then(|v| v.parse::<u8>().ok()).unwrap_or(0);
                        let ordered = !matches!(lvl.child_val("w:numFmt"), Some("bullet" | "none"));
                        let start = lvl.child_val("w:start").and_then(|v| v.parse().ok()).unwrap_or(1);
                        numbering.levels.insert((id.to_owned(), level), (ordered, start));
                    }
                }
                "w:num" => {
                    if let (Some(id), Some(abs)) = (e.attr("w:numId"), e.child_val("w:abstractNumId")) {
                        numbering.nums.insert(id.to_owned(), abs.to_owned());
                        for over in e.elements().filter(|o| o.name == "w:lvlOverride") {
                            let level = over.attr("w:ilvl").and_then(|v| v.parse::<u8>().ok()).unwrap_or(0);
                            if let Some(start) = over.child_val("w:startOverride").and_then(|v| v.parse().ok()) {
                                numbering.overrides.insert((id.to_owned(), level), start);
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        numbering
    }

    /// Whether the list `num` is ordered at level `level`, and its first number. Without `numbering.xml` the
    /// convention of the writer applies (`NUM_DECIMAL` ordered, everything else bullets).
    pub(crate) fn level(&self, num: &str, level: u8) -> (bool, u64) {
        if !self.known {
            return (num == ooxml::NUM_DECIMAL.to_string(), 1);
        }
        let (ordered, start) =
            self.nums.get(num).and_then(|abs| self.levels.get(&(abs.clone(), level))).copied().unwrap_or((false, 1));
        (ordered, self.overrides.get(&(num.to_owned(), level)).copied().unwrap_or(start))
    }
}

fn kind_is(kind: &str, suffix: &str) -> bool {
    kind.rsplit('/').next() == Some(suffix)
}

fn iso_date(text: &str) -> String {
    let text = text.trim();
    let ok = text.len() >= 10 && text.as_bytes()[4] == b'-' && text.as_bytes()[7] == b'-' && text.is_ascii();
    if ok { text.to_owned() } else { String::new() }
}

fn load_comments(root: &Element, extended: Option<&Element>) -> Vec<Comment> {
    let mut comments = Vec::new();
    // w14:paraId of every paragraph of a comment → comment (reference of the replies).
    let mut by_para: HashMap<String, u32> = HashMap::new();
    for c in root.elements().filter(|e| e.name == "w:comment") {
        let Some(id) = c.attr("w:id").and_then(|v| v.parse::<u32>().ok()) else { continue };
        let mut lines = Vec::new();
        for p in c.elements().filter(|e| e.name == "w:p") {
            if let Some(para) = p.attr("w14:paraId") {
                by_para.insert(para.to_owned(), id);
            }
            let mut line = String::new();
            p.text_of("w:t", &mut line);
            lines.push(line);
        }
        comments.push(Comment {
            id,
            author: c.attr("w:author").unwrap_or("").trim().to_owned(),
            initials: c.attr("w:initials").unwrap_or("").trim().to_owned(),
            date: iso_date(c.attr("w:date").unwrap_or("")),
            text: lines.join("\n").trim().to_owned(),
            reply_to: None,
        });
    }
    if let Some(extended) = extended {
        for ex in extended.elements().filter(|e| e.name == "w15:commentEx") {
            let (Some(para), Some(parent)) = (ex.attr("w15:paraId"), ex.attr("w15:paraIdParent")) else { continue };
            let (Some(&child), Some(&parent)) = (by_para.get(para), by_para.get(parent)) else { continue };
            if child != parent
                && let Some(comment) = comments.iter_mut().find(|c| c.id == child)
            {
                comment.reply_to = Some(parent);
            }
        }
    }
    comments
}

/// Reads a .docx file from `bytes`.
pub fn read(bytes: &[u8], limits: &Limits) -> Result<ReadDocx, DocxError> {
    let mut package = Package::open(bytes, limits)?;
    if !package.has(ooxml::PART_CONTENT_TYPES) {
        return Err(DocxError::NotDocx("ein ZIP-Archiv, aber keine Office-Datei („[Content_Types].xml“ fehlt)".into()));
    }
    let root_rels = pack::rels(&mut package, "")?;
    let part_of = |rels: &HashMap<String, pack::Rel>, suffix: &str, default: &str| -> String {
        rels.values()
            .find(|r| !r.external && kind_is(&r.kind, suffix))
            .map_or_else(|| default.to_owned(), |r| r.target.clone())
    };
    let document_part = part_of(&root_rels, "officeDocument", ooxml::PART_DOCUMENT);
    let Some(document) = package.xml(&document_part)? else {
        return Err(DocxError::NotDocx(format!(
            "kein Word-Dokument („{document_part}“ fehlt) – vielleicht eine Tabelle, Präsentation oder ein anderes \
             Format"
        )));
    };
    let Some(body_element) = (document.name == "w:document").then(|| document.child("w:body")).flatten() else {
        return Err(DocxError::Invalid(format!("„{document_part}“ hat keinen Textkörper (w:document/w:body)")));
    };
    let rels = pack::rels(&mut package, &document_part)?;

    let styles_root = package.xml(&part_of(&rels, "styles", ooxml::PART_STYLES))?;
    let styles = styles_root.as_ref().map(Styles::load).unwrap_or_default();
    let numbering = package
        .xml(&part_of(&rels, "numbering", ooxml::PART_NUMBERING))?
        .as_ref()
        .map(Numbering::load)
        .unwrap_or_default();
    let notes = |root: Option<Element>, name: &str| -> HashMap<String, Element> {
        let mut map = HashMap::new();
        for note in root.into_iter().flat_map(|r| r.children) {
            if let xml::Node::Element(note) = note
                && note.name == name
                && note.attr("w:type").is_none_or(|t| t == "normal")
                && let Some(id) = note.attr("w:id")
            {
                map.insert(id.to_owned(), note);
            }
        }
        map
    };
    let footnotes = notes(package.xml(&part_of(&rels, "footnotes", ooxml::PART_FOOTNOTES))?, "w:footnote");
    let endnotes = notes(package.xml(&part_of(&rels, "endnotes", ooxml::PART_ENDNOTES))?, "w:endnote");
    let comments = match package.xml(&part_of(&rels, "comments", ooxml::PART_COMMENTS))? {
        Some(root) => {
            let extended = package.xml(&part_of(&rels, "commentsExtended", ooxml::PART_COMMENTS_EXTENDED))?;
            load_comments(&root, extended.as_ref())
        }
        None => Vec::new(),
    };

    let mut meta = DocxMeta::default();
    let mut core_title = String::new();
    let mut core_author = String::new();
    if let Some(core) = package.xml(&part_of(&root_rels, "core-properties", ooxml::PART_CORE))? {
        let text = |name: &str| core.child(name).map(|e| e.text().trim().to_owned()).unwrap_or_default();
        core_title = text("dc:title");
        core_author = text("dc:creator");
        meta.lang = text("dc:language");
    }
    if meta.lang.is_empty()
        && let Some(lang) = styles_root.as_ref().and_then(|s| s.find("w:lang")).and_then(|l| l.attr("w:val"))
    {
        meta.lang = lang.to_owned();
    }

    let mut reader = body::BodyReader {
        package: &mut package,
        limits: *limits,
        styles: &styles,
        numbering: &numbering,
        footnotes: &footnotes,
        endnotes: &endnotes,
        rels: &rels,
        meta,
        images: Vec::new(),
        image_parts: HashMap::new(),
        warnings: Warnings::default(),
        text_bytes: 0,
        in_note: false,
        last_list: None,
        ranges: std::collections::HashSet::new(),
    };
    let mut blocks = Vec::new();
    reader.blocks(body_element, &mut blocks, true)?;
    let body::BodyReader { mut meta, images, warnings, .. } = reader;
    if meta.title.is_empty() {
        meta.title = core_title;
    }
    if meta.author.is_empty() {
        meta.author = core_author;
    }
    Ok(ReadDocx { document: DocxDocument { meta, blocks, comments, images }, warnings: warnings.finish() })
}
