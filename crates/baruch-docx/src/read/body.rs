// SPDX-License-Identifier: AGPL-3.0-or-later
//! Textkörper einer Word-Datei → Blockmodell: Absätze, Läufe, Änderungsverfolgung, Fußnoten, Bilder, Listen,
//! Tabellen, Typst-Inseln. Alles, was nicht abgebildet wird, ergibt eine Warnung mit festem Code.

use std::collections::{HashMap, HashSet};

use super::pack::{Package, Rel};
use super::xml::Element;
use super::{Numbering, Role, Styles, Warnings};
use crate::{Block, Change, ChangeKind, DocxError, DocxImage, DocxMeta, Inline, Limits, ListItem, Marks, ooxml};

pub(crate) struct BodyReader<'a, 'b> {
    pub package: &'a mut Package<'b>,
    pub limits: Limits,
    pub styles: &'a Styles,
    pub numbering: &'a Numbering,
    pub footnotes: &'a HashMap<String, Element>,
    pub endnotes: &'a HashMap<String, Element>,
    pub rels: &'a HashMap<String, Rel>,
    pub meta: DocxMeta,
    pub images: Vec<DocxImage>,
    /// Teilpfad → Index in `images` (`None` = übersprungen, schon gemeldet).
    pub image_parts: HashMap<String, Option<usize>>,
    pub warnings: Warnings,
    pub text_bytes: usize,
    pub in_note: bool,
    /// `numId` der Liste, die als letzter Block steht.
    pub last_list: Option<String>,
    /// Kommentare, die über einen Bereich (`w:commentRangeStart`) verankert sind.
    pub ranges: HashSet<u32>,
}

/// Was ein Absatz neben seinem Inhalt mitbringt.
struct Para {
    role: Role,
    list: Option<(String, u8)>,
    content: Vec<Inline>,
    mark: Option<Change>,
    page_break: bool,
}

#[derive(Default)]
struct RunState {
    page_break: bool,
}

fn on(parent: &Element, name: &str) -> bool {
    parent.child(name).is_some_and(|e| !matches!(e.attr("w:val"), Some("0" | "false" | "off" | "none")))
}

fn change(el: &Element, kind: ChangeKind) -> Change {
    Change {
        kind,
        author: el.attr("w:author").unwrap_or("").trim().to_owned(),
        date: super::iso_date(el.attr("w:date").unwrap_or("")),
    }
}

fn media_type(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        Some("image/jpeg")
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some("image/gif")
    } else if bytes.len() > 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
        Some("image/webp")
    } else {
        let head = String::from_utf8_lossy(&bytes[..bytes.len().min(1024)]).to_lowercase();
        let head = head.trim_start_matches('\u{feff}').trim_start();
        ((head.starts_with("<?xml") || head.starts_with("<svg")) && head.contains("<svg")).then_some("image/svg+xml")
    }
}

/// Text ohne Auszeichnung; `original` lässt Einfügungen weg, sonst Löschungen.
pub(crate) fn plain(inlines: &[Inline], original: bool, out: &mut String) {
    for inline in inlines {
        match inline {
            Inline::Text { text, .. } => out.push_str(text),
            Inline::LineBreak => out.push(' '),
            Inline::Quote { content } | Inline::Link { content, .. } => plain(content, original, out),
            Inline::Tracked { change, content } => {
                if (change.kind == ChangeKind::Delete) == original {
                    plain(content, original, out);
                }
            }
            Inline::Footnote { .. }
            | Inline::Image { .. }
            | Inline::Typst { .. }
            | Inline::CommentStart { .. }
            | Inline::CommentEnd { .. } => {}
        }
    }
}

fn short(text: &str, max: usize) -> String {
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if text.chars().count() > max { format!("{}…", text.chars().take(max).collect::<String>()) } else { text }
}

fn has_text(inlines: &[Inline]) -> bool {
    inlines.iter().any(|i| match i {
        Inline::Text { text, .. } => !text.trim().is_empty(),
        Inline::CommentStart { .. } | Inline::CommentEnd { .. } => false,
        Inline::Tracked { content, .. } | Inline::Quote { content } | Inline::Link { content, .. } => has_text(content),
        Inline::LineBreak => false,
        Inline::Footnote { .. } | Inline::Image { .. } | Inline::Typst { .. } => true,
    })
}

/// Ist der Absatz nur ein Bild (neben Leerraum und Kommentarmarken)?
fn only_image(inlines: &[Inline]) -> Option<(usize, String)> {
    let mut found = None;
    for inline in inlines {
        match inline {
            Inline::Image { image, alt } if found.is_none() => found = Some((*image, alt.clone())),
            Inline::Text { text, .. } if text.trim().is_empty() => {}
            Inline::CommentStart { .. } | Inline::CommentEnd { .. } => {}
            _ => return None,
        }
    }
    found
}

/// Wörtlicher Typst-Quelltext einer Insel: der mit `w:vanish` verborgene Text; Absätze werden Zeilen.
fn island_raw(content: &Element) -> String {
    fn vanish(el: &Element, out: &mut String, any: &mut bool) {
        for e in el.elements() {
            if e.name == "w:r" {
                if e.child("w:rPr").is_some_and(|p| on(p, "w:vanish")) {
                    *any = true;
                    for c in e.elements() {
                        match c.name.as_str() {
                            "w:t" => out.push_str(&c.text()),
                            "w:br" | "w:cr" => out.push('\n'),
                            "w:tab" => out.push('\t'),
                            _ => {}
                        }
                    }
                }
            } else if e.name != "w:p" {
                vanish(e, out, any);
            }
        }
    }
    fn paragraphs<'e>(el: &'e Element, out: &mut Vec<&'e Element>) {
        for e in el.elements() {
            if e.name == "w:p" {
                out.push(e);
            } else {
                paragraphs(e, out);
            }
        }
    }
    let mut paras = Vec::new();
    paragraphs(content, &mut paras);
    let mut lines = Vec::new();
    if paras.is_empty() {
        paras.push(content);
    }
    for p in paras {
        let (mut line, mut any) = (String::new(), false);
        vanish(p, &mut line, &mut any);
        if any {
            lines.push(line);
        }
    }
    lines.join("\n")
}

fn is_typst_island(sdt: &Element) -> bool {
    sdt.child("w:sdtPr").and_then(|p| p.child_val("w:tag")) == Some(ooxml::SDT_TAG_TYPST)
}

/// Der Name einer Zeichnung als Pfad eines Bildes im Werk – nur wenn er wie einer aussieht: relativ, ohne `..`,
/// Rückstrich, Laufwerk oder Steuerzeichen, höchstens 200 Zeichen und mit der Endung, die zum Inhalt passt. Word
/// selbst nennt Zeichnungen „Grafik 1“ oder „Picture 1“; das bleibt außen vor.
fn work_path(name: &str, media: &str) -> Option<String> {
    let endings: &[&str] = match media {
        "image/png" => &["png"],
        "image/jpeg" => &["jpg", "jpeg"],
        "image/gif" => &["gif"],
        "image/webp" => &["webp"],
        "image/svg+xml" => &["svg"],
        _ => return None,
    };
    let (stem, ending) = name.rsplit_once('.')?;
    let safe = name.len() <= 200
        && !stem.is_empty()
        && endings.contains(&ending.to_ascii_lowercase().as_str())
        && !name.starts_with('/')
        && !name.chars().any(|c| c.is_control() || matches!(c, '\\' | ':' | '"' | '<' | '>' | '|' | '?' | '*'))
        && name.split('/').all(|part| !part.is_empty() && part != "." && part != ".." && part.trim() == part);
    safe.then(|| name.to_owned())
}

impl BodyReader<'_, '_> {
    /// Bilder im sichtbaren Teil einer Typst-Insel: Das Modell behält von der Insel nur den Quelltext, das Bild
    /// selbst muss trotzdem ins Werk – der Quelltext nennt es (`image("bilder/…")`).
    fn island_images(&mut self, el: &Element) -> Result<(), DocxError> {
        for e in el.elements() {
            if e.name == "w:drawing" {
                if let Some(rid) = e.find("a:blip").and_then(|blip| blip.attr("r:embed")) {
                    let name = e.find("wp:docPr").and_then(|d| d.attr("name"));
                    self.image(rid, name)?;
                }
            } else {
                self.island_images(e)?;
            }
        }
        Ok(())
    }

    fn count_text(&mut self, text: &str) -> Result<(), DocxError> {
        self.text_bytes += text.len();
        if self.text_bytes > self.limits.max_text_bytes {
            return Err(DocxError::Limit(format!("mehr als {} Bytes Text", self.limits.max_text_bytes)));
        }
        Ok(())
    }

    fn push_text(&mut self, out: &mut Vec<Inline>, text: &str, marks: Marks, quote: bool) -> Result<(), DocxError> {
        if text.is_empty() {
            return Ok(());
        }
        self.count_text(text)?;
        let target = if quote {
            if !matches!(out.last(), Some(Inline::Quote { .. })) {
                out.push(Inline::Quote { content: Vec::new() });
            }
            match out.last_mut() {
                Some(Inline::Quote { content }) => content,
                _ => unreachable!("eben angehängt"),
            }
        } else {
            out
        };
        if let Some(Inline::Text { text: last, marks: last_marks }) = target.last_mut()
            && *last_marks == marks
        {
            last.push_str(text);
        } else {
            target.push(Inline::Text { text: text.to_owned(), marks });
        }
        Ok(())
    }

    /// Bild einer Beziehung einlesen; `None` = übersprungen (mit Warnung).
    ///
    /// `wanted` ist der Name der Zeichnung (`wp:docPr name`): Der Schreiber von Baruch legt dort den Pfad des Bildes
    /// im Werk ab. Sieht der Name wie ein solcher Pfad aus (siehe [`work_path`]), bekommt das Bild ihn wieder –
    /// sonst zeigte der wörtliche Typst-Quelltext einer Abbildung nach der Rundreise auf ein Bild, das es nicht gibt.
    fn image(&mut self, rid: &str, wanted: Option<&str>) -> Result<Option<usize>, DocxError> {
        let Some(rel) = self.rels.get(rid) else {
            self.warnings
                .add("docx_bild_format", "Ein Bild verweist auf eine Beziehung, die es nicht gibt; ausgelassen.");
            return Ok(None);
        };
        if rel.external {
            self.warnings.add(
                "docx_bild_extern",
                "Ein Bild ist nur verknüpft (liegt außerhalb der Datei); Baruch ruft es nie ab, es fehlt im Werk.",
            );
            return Ok(None);
        }
        if let Some(known) = self.image_parts.get(&rel.target) {
            return Ok(*known);
        }
        let file_name = rel.target.rsplit('/').next().unwrap_or("bild").to_owned();
        let bytes = self.package.read(&rel.target, self.limits.max_entry_bytes)?;
        let index = match bytes.as_deref().and_then(|b| media_type(b).map(|m| (b, m))) {
            Some((bytes, media)) => {
                if self.images.len() >= self.limits.max_images {
                    return Err(DocxError::Limit(format!("mehr als {} Bilder", self.limits.max_images)));
                }
                let mut path = match wanted.and_then(|name| work_path(name, media)) {
                    Some(own) => own,
                    None => format!("bilder/{file_name}"),
                };
                let mut n = 2;
                while self.images.iter().any(|i| i.path == path) {
                    path = format!("bilder/{n}-{file_name}");
                    n += 1;
                }
                self.images.push(DocxImage {
                    path,
                    file_name: file_name.clone(),
                    media_type: media.to_owned(),
                    bytes: bytes.to_vec(),
                });
                Some(self.images.len() - 1)
            }
            None => {
                self.warnings.add(
                    "docx_bild_format",
                    format!(
                        "Bild „{file_name}“ hat kein unterstütztes Format (PNG, JPEG, GIF, WebP, SVG) oder fehlt im \
                         Archiv; ausgelassen."
                    ),
                );
                None
            }
        };
        self.image_parts.insert(rel.target.clone(), index);
        Ok(index)
    }

    fn textbox(&mut self, el: &Element) {
        let mut text = String::new();
        el.text_of("w:t", &mut text);
        self.warnings
            .add("docx_textfeld", format!("Ein Textfeld ist nicht übernommen (Anfang: „{}“).", short(&text, 80)));
    }

    /// Zeichnung, VML-Bild, eingebettetes Objekt.
    fn graphic(&mut self, el: &Element, out: &mut Vec<Inline>) -> Result<(), DocxError> {
        match el.name.as_str() {
            "mc:AlternateContent" => {
                let branch = el.child("mc:Choice").or_else(|| el.child("mc:Fallback"));
                for child in branch.into_iter().flat_map(Element::elements) {
                    self.graphic(child, out)?;
                }
            }
            "w:drawing" => {
                if el.find("w:txbxContent").is_some() {
                    self.textbox(el);
                } else if let Some(blip) = el.find("a:blip") {
                    let alt = el
                        .find("wp:docPr")
                        .and_then(|d| d.attr("descr").filter(|a| !a.trim().is_empty()).or_else(|| d.attr("title")))
                        .unwrap_or("")
                        .trim()
                        .to_owned();
                    match blip.attr("r:embed") {
                        Some(rid) => {
                            let name = el.find("wp:docPr").and_then(|d| d.attr("name"));
                            if let Some(image) = self.image(rid, name)? {
                                out.push(Inline::Image { image, alt });
                            }
                        }
                        None => self.warnings.add(
                            "docx_bild_extern",
                            "Ein Bild ist nur verknüpft (liegt außerhalb der Datei); Baruch ruft es nie ab, es fehlt \
                             im Werk.",
                        ),
                    }
                } else {
                    self.warnings.add("docx_objekt", "Eine Zeichnung (Diagramm, Form, SmartArt) ist nicht übernommen.");
                }
            }
            "w:pict" => {
                if el.find("v:textbox").is_some() {
                    self.textbox(el);
                } else if let Some(data) = el.find("v:imagedata") {
                    let alt = data.attr("o:title").unwrap_or("").trim().to_owned();
                    if let Some(rid) = data.attr("r:id")
                        && let Some(image) = self.image(rid, None)?
                    {
                        out.push(Inline::Image { image, alt });
                    }
                } else {
                    self.warnings.add("docx_objekt", "Eine Zeichnung (VML) ist nicht übernommen.");
                }
            }
            _ => self.warnings.add("docx_objekt", "Ein eingebettetes Objekt (OLE) ist nicht übernommen."),
        }
        Ok(())
    }

    fn note(&mut self, id: Option<&str>, endnote: bool, out: &mut Vec<Inline>) -> Result<(), DocxError> {
        if self.in_note {
            return Ok(());
        }
        let notes = if endnote { self.endnotes } else { self.footnotes };
        let Some(note) = id.and_then(|id| notes.get(id)) else {
            self.warnings.add("docx_unbekannt", "Ein Fußnotenzeichen ohne Fußnotentext ist ausgelassen.");
            return Ok(());
        };
        if endnote {
            self.warnings.add("docx_endnote", "Endnoten stehen im Werk als Fußnoten.");
        }
        self.in_note = true;
        let mut content = Vec::new();
        let mut state = RunState::default();
        let mut paragraphs = 0;
        for p in note.elements().filter(|e| e.name == "w:p") {
            let mut part = Vec::new();
            self.inlines(p, &mut part, &mut state)?;
            if !has_text(&part) {
                continue;
            }
            if paragraphs > 0 {
                content.push(Inline::LineBreak);
            }
            paragraphs += 1;
            content.extend(part);
        }
        self.in_note = false;
        if paragraphs > 1 {
            self.warnings.add(
                "docx_fussnote_absaetze",
                "Eine Fußnote mit mehreren Absätzen steht im Werk als ein Absatz mit Zeilenumbrüchen.",
            );
        }
        // Word setzt hinter das Fußnotenzeichen ein Leerzeichen.
        if let Some(Inline::Text { text, .. }) = content.first_mut() {
            *text = text.trim_start().to_owned();
        }
        if !content.is_empty() {
            out.push(Inline::Footnote { content });
        }
        Ok(())
    }

    fn run(&mut self, r: &Element, out: &mut Vec<Inline>, state: &mut RunState) -> Result<(), DocxError> {
        let props = r.child("w:rPr");
        let mut marks = Marks::default();
        let (mut hidden, mut quote) = (false, false);
        if let Some(p) = props {
            marks.bold = on(p, "w:b");
            marks.italic = on(p, "w:i");
            marks.underline = on(p, "w:u");
            marks.strike = on(p, "w:strike") || on(p, "w:dstrike");
            marks.small_caps = on(p, "w:smallCaps");
            match p.child_val("w:vertAlign") {
                Some("superscript") => marks.superscript = true,
                Some("subscript") => marks.subscript = true,
                _ => {}
            }
            hidden = on(p, "w:vanish");
            quote = p.child_val("w:rStyle") == Some(ooxml::STYLE_INLINE_QUOTE);
            if p.child("w:rPrChange").is_some() {
                self.format_change();
            }
        }
        for c in r.elements() {
            match c.name.as_str() {
                "w:t" | "w:delText" => {
                    if hidden {
                        self.warnings.add(
                            "docx_verborgener_text",
                            format!("Verborgener Text ist nicht übernommen (Anfang: „{}“).", short(&c.text(), 80)),
                        );
                    } else {
                        self.push_text(out, &c.text(), marks, quote)?;
                    }
                }
                "w:tab" => self.push_text(out, " ", marks, quote)?,
                "w:noBreakHyphen" => self.push_text(out, "\u{2011}", marks, quote)?,
                "w:softHyphen" => self.push_text(out, "\u{ad}", marks, quote)?,
                "w:br" => match c.attr("w:type") {
                    Some("page") => state.page_break = true,
                    Some("column") => {}
                    _ => out.push(Inline::LineBreak),
                },
                "w:cr" => out.push(Inline::LineBreak),
                "w:sym" => {
                    let symbol = c
                        .attr("w:char")
                        .and_then(|hex| u32::from_str_radix(hex, 16).ok())
                        .and_then(char::from_u32)
                        .filter(|ch| !('\u{e000}'..='\u{f8ff}').contains(ch));
                    match symbol {
                        Some(ch) => self.push_text(out, &ch.to_string(), marks, quote)?,
                        None => self.warnings.add(
                            "docx_symbol",
                            "Ein Sonderzeichen aus einer Symbolschrift (Symbol, Wingdings) ist ausgelassen.",
                        ),
                    }
                }
                "w:footnoteReference" => self.note(c.attr("w:id"), false, out)?,
                "w:endnoteReference" => self.note(c.attr("w:id"), true, out)?,
                "w:commentReference" => {
                    if let Some(id) = c.attr("w:id").and_then(|v| v.parse::<u32>().ok())
                        && !self.ranges.contains(&id)
                    {
                        out.push(Inline::CommentStart { id });
                        out.push(Inline::CommentEnd { id });
                    }
                }
                "w:drawing" | "w:pict" | "w:object" | "mc:AlternateContent" => self.graphic(c, out)?,
                "w:rPr"
                | "w:instrText"
                | "w:delInstrText"
                | "w:fldChar"
                | "w:lastRenderedPageBreak"
                | "w:footnoteRef"
                | "w:endnoteRef"
                | "w:annotationRef"
                | "w:separator"
                | "w:continuationSeparator" => {}
                other => {
                    self.warnings.add("docx_unbekannt", format!("Unbekanntes Element im Text ({other}) ausgelassen."))
                }
            }
        }
        Ok(())
    }

    fn format_change(&mut self) {
        self.warnings.add(
            "docx_formataenderung",
            "Verfolgte Formatänderungen sind nicht übernommen; es gilt die Formatierung des Endstands.",
        );
    }

    fn tracked(
        &mut self,
        el: &Element,
        kind: ChangeKind,
        moved: bool,
        out: &mut Vec<Inline>,
        state: &mut RunState,
    ) -> Result<(), DocxError> {
        if moved {
            self.warnings.add(
                "docx_verschoben",
                "Verschobener Text steht als Löschung an der alten und Einfügung an der neuen Stelle.",
            );
        }
        let mut content = Vec::new();
        self.inlines(el, &mut content, state)?;
        if !content.is_empty() {
            out.push(Inline::Tracked { change: change(el, kind), content });
        }
        Ok(())
    }

    /// Inhalt eines Absatzes (oder eines Behälters darin).
    fn inlines(&mut self, parent: &Element, out: &mut Vec<Inline>, state: &mut RunState) -> Result<(), DocxError> {
        for e in parent.elements() {
            match e.name.as_str() {
                "w:r" => self.run(e, out, state)?,
                "w:ins" => self.tracked(e, ChangeKind::Insert, false, out, state)?,
                "w:del" => self.tracked(e, ChangeKind::Delete, false, out, state)?,
                "w:moveTo" => self.tracked(e, ChangeKind::Insert, true, out, state)?,
                "w:moveFrom" => self.tracked(e, ChangeKind::Delete, true, out, state)?,
                "w:hyperlink" => {
                    let mut content = Vec::new();
                    self.inlines(e, &mut content, state)?;
                    let url = e
                        .attr("r:id")
                        .and_then(|rid| self.rels.get(rid))
                        .filter(|rel| rel.external)
                        .map(|rel| rel.target.clone());
                    match url {
                        Some(url) if !content.is_empty() => out.push(Inline::Link { url, content }),
                        _ => out.extend(content),
                    }
                }
                "w:sdt" => {
                    let raw = if is_typst_island(e) { island_raw(e) } else { String::new() };
                    if !raw.is_empty() {
                        self.count_text(&raw)?;
                        self.island_images(e)?;
                        out.push(Inline::Typst { raw, shown: Vec::new() });
                    } else if let Some(content) = e.child("w:sdtContent") {
                        self.inlines(content, out, state)?;
                    }
                }
                "w:smartTag" | "w:customXml" | "w:fldSimple" | "w:bdo" | "w:dir" => self.inlines(e, out, state)?,
                "w:commentRangeStart" | "w:commentRangeEnd" => {
                    if let Some(id) = e.attr("w:id").and_then(|v| v.parse::<u32>().ok()) {
                        if e.name == "w:commentRangeStart" {
                            self.ranges.insert(id);
                            out.push(Inline::CommentStart { id });
                        } else {
                            out.push(Inline::CommentEnd { id });
                        }
                    }
                }
                "m:oMath" | "m:oMathPara" => {
                    let mut text = String::new();
                    e.text_of("m:t", &mut text);
                    self.warnings.add(
                        "docx_formel",
                        format!("Eine Formel steht nur als einfacher Text im Werk („{}“).", short(&text, 80)),
                    );
                    self.push_text(out, &text, Marks::default(), false)?;
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn paragraph(&mut self, p: &Element) -> Result<Para, DocxError> {
        let mut state = RunState::default();
        let (mut style, mut outline, mut list, mut mark) = (None, None, None, None);
        if let Some(props) = p.child("w:pPr") {
            style = props.child_val("w:pStyle");
            outline = props.child_val("w:outlineLvl").and_then(|v| v.parse::<u8>().ok());
            if let Some(num) = props.child("w:numPr")
                && let Some(id) = num.child_val("w:numId").filter(|id| *id != "0")
            {
                let level = num.child_val("w:ilvl").and_then(|v| v.parse::<u8>().ok()).unwrap_or(0);
                list = Some((id.to_owned(), level));
            }
            if on(props, "w:pageBreakBefore") {
                state.page_break = true;
            }
            if props.child("w:pPrChange").is_some() {
                self.format_change();
            }
            if let Some(run_props) = props.child("w:rPr") {
                if run_props.child("w:rPrChange").is_some() {
                    self.format_change();
                }
                mark = run_props
                    .child("w:ins")
                    .or_else(|| run_props.child("w:moveTo"))
                    .map(|e| change(e, ChangeKind::Insert))
                    .or_else(|| {
                        run_props
                            .child("w:del")
                            .or_else(|| run_props.child("w:moveFrom"))
                            .map(|e| change(e, ChangeKind::Delete))
                    });
            }
        }
        let mut content = Vec::new();
        self.inlines(p, &mut content, &mut state)?;
        Ok(Para { role: self.styles.role(style, outline), list, content, mark, page_break: state.page_break })
    }

    fn push(&mut self, out: &mut Vec<Block>, block: Block) {
        self.last_list = None;
        out.push(block);
    }

    fn push_paragraph(&mut self, para: Para, out: &mut Vec<Block>, top: bool) {
        let Para { role, list, content, mark, page_break } = para;
        if page_break && top {
            self.push(out, Block::PageBreak);
        }
        if content.is_empty() {
            return;
        }
        let text = |content: &[Inline]| {
            let mut text = String::new();
            plain(content, true, &mut text);
            text.trim().to_owned()
        };
        match role {
            Role::Title if top && self.meta.title.is_empty() && !text(&content).is_empty() => {
                self.meta.title = text(&content);
            }
            Role::Subtitle if top && self.meta.subtitle.is_empty() && !text(&content).is_empty() => {
                self.meta.subtitle = text(&content);
            }
            Role::Author if top && self.meta.author.is_empty() => self.meta.author = text(&content),
            Role::Imprint if top => self.meta.imprint.push(text(&content)),
            Role::Part if has_text(&content) => self.push(out, Block::Part { content, mark }),
            Role::Heading { level, numbered } if has_text(&content) => {
                self.push(out, Block::Heading { level, numbered, content, mark });
            }
            Role::Quote => {
                let paragraph = Block::Paragraph { content, mark };
                if let Some(Block::Quote { blocks, attribution }) = out.last_mut()
                    && attribution.is_empty()
                {
                    blocks.push(paragraph);
                } else {
                    self.push(out, Block::Quote { blocks: vec![paragraph], attribution: Vec::new() });
                }
            }
            Role::QuoteSource if matches!(out.last(), Some(Block::Quote { attribution, .. }) if attribution.is_empty()) => {
                if let Some(Block::Quote { attribution, .. }) = out.last_mut() {
                    *attribution = content;
                }
            }
            Role::Caption if matches!(out.last(), Some(Block::Figure { caption, .. }) if caption.is_empty()) => {
                if let Some(Block::Figure { caption, .. }) = out.last_mut() {
                    *caption = content;
                }
            }
            _ => {
                if let Some((num, depth)) = list {
                    let item = ListItem { depth, content, mark };
                    if self.last_list.as_deref() == Some(num.as_str())
                        && let Some(Block::List { items, .. }) = out.last_mut()
                    {
                        items.push(item);
                    } else {
                        let (ordered, start) = self.numbering.level(&num, 0);
                        out.push(Block::List { ordered, start, items: vec![item] });
                        self.last_list = Some(num);
                    }
                } else if let Some((image, alt)) = only_image(&content) {
                    self.push(out, Block::Figure { image, alt, caption: Vec::new(), raw: String::new() });
                } else {
                    self.push(out, Block::Paragraph { content, mark });
                }
            }
        }
    }

    fn rows(&mut self, parent: &Element, rows: &mut Vec<(bool, Vec<Vec<Block>>)>) -> Result<(), DocxError> {
        for e in parent.elements() {
            match e.name.as_str() {
                "w:tr" => {
                    let props = e.child("w:trPr");
                    if props.is_some_and(|p| p.child("w:ins").is_some() || p.child("w:del").is_some()) {
                        self.warnings.add(
                            "docx_tabelle_vereinfacht",
                            "Eingefügte oder gelöschte Tabellenzeilen sind nicht als Änderung übernommen; die Zeile \
                             steht in jeder Fassung.",
                        );
                    }
                    let header = props.is_some_and(|p| on(p, "w:tblHeader"));
                    let mut cells = Vec::new();
                    for cell in e.elements().filter(|c| c.name == "w:tc") {
                        if let Some(p) = cell.child("w:tcPr")
                            && (p.child("w:vMerge").is_some() || p.child_val("w:gridSpan").is_some_and(|v| v != "1"))
                        {
                            self.warnings.add(
                                "docx_tabelle_vereinfacht",
                                "Verbundene Tabellenzellen stehen im Werk als einzelne Zellen.",
                            );
                        }
                        let mut blocks = Vec::new();
                        let list = self.last_list.take();
                        self.blocks(cell, &mut blocks, false)?;
                        self.last_list = list;
                        cells.push(blocks);
                    }
                    rows.push((header, cells));
                }
                "w:sdt" | "w:sdtContent" | "w:customXml" | "w:ins" | "w:del" => self.rows(e, rows)?,
                _ => {}
            }
        }
        Ok(())
    }

    /// Blöcke eines Behälters (Textkörper, Tabellenzelle, Inhaltssteuerelement). `top` = Textkörper selbst: nur
    /// dort werden Titel, Untertitel, Autor und Impressum zur Titelei.
    pub(crate) fn blocks(&mut self, parent: &Element, out: &mut Vec<Block>, top: bool) -> Result<(), DocxError> {
        for e in parent.elements() {
            match e.name.as_str() {
                "w:p" => {
                    let para = self.paragraph(e)?;
                    self.push_paragraph(para, out, top);
                }
                "w:tbl" => {
                    let mut rows = Vec::new();
                    self.rows(e, &mut rows)?;
                    let header_rows = rows.iter().take_while(|(header, _)| *header).count();
                    let rows: Vec<Vec<Vec<Block>>> = rows.into_iter().map(|(_, cells)| cells).collect();
                    if !rows.is_empty() {
                        self.push(out, Block::Table { header_rows, rows });
                    }
                }
                "w:sdt" => {
                    let raw = if is_typst_island(e) { island_raw(e) } else { String::new() };
                    if !raw.is_empty() {
                        self.count_text(&raw)?;
                        self.island_images(e)?;
                        self.push(out, Block::Typst { raw, shown: Vec::new() });
                    } else if let Some(content) = e.child("w:sdtContent") {
                        self.blocks(content, out, top)?;
                    }
                }
                "w:customXml" | "w:ins" | "w:del" | "w:moveTo" | "w:moveFrom" => self.blocks(e, out, top)?,
                "w:altChunk" => {
                    self.warnings
                        .add("docx_objekt", "Ein eingebundenes Fremddokument (altChunk) ist nicht übernommen.");
                }
                _ => {}
            }
        }
        Ok(())
    }
}
