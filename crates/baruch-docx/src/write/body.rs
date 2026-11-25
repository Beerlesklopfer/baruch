// SPDX-License-Identifier: AGPL-3.0-or-later
//! Blöcke und Inhalte des Modells als WordprocessingML: Fließtext, Fußnoten, Kommentar-Anker, Bilder, Listen,
//! Tabellen, Typst-Inseln und verfolgte Änderungen. Der [`Renderer`] sammelt nebenbei, was andere Teile der Datei
//! brauchen: Fußnoten, Beziehungen je Teil, benutzte Bilder und zusätzliche Nummerierungen.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use super::image;
use crate::ooxml::*;
use crate::{Block, Change, ChangeKind, DocxDocument, DocxError, Inline, ListItem, Marks};

/// Zeichenvorlage für Verweise (eingebaute Vorlage „Hyperlink“).
pub(super) const STYLE_HYPERLINK: &str = "Hyperlink";
/// Absatzvorlage des Kommentartexts (eingebaut „annotation text“) und Zeichenvorlage des Kommentarzeichens.
pub(super) const STYLE_COMMENT_TEXT: &str = "CommentText";
pub(super) const STYLE_COMMENT_REFERENCE: &str = "CommentReference";

/// Erste Kennung einer echten Fußnote; 0 und 1 sind die Trennlinien.
pub(super) const FIRST_FOOTNOTE_ID: u32 = 2;

/// Teil der Datei, in dem gerade geschrieben wird (jeder hat eigene Beziehungen).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Part {
    Document,
    Footnotes,
}

/// Eine Beziehung eines Teils.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Rel {
    pub id: String,
    pub kind: &'static str,
    pub target: String,
    pub external: bool,
}

/// Beziehungen eines Teils; gleiche Ziele teilen sich eine Kennung.
#[derive(Debug, Default)]
pub(super) struct Rels {
    pub entries: Vec<Rel>,
}

impl Rels {
    pub fn add(&mut self, kind: &'static str, target: &str, external: bool) -> String {
        if let Some(found) =
            self.entries.iter().find(|r| r.kind == kind && r.target == target && r.external == external)
        {
            return found.id.clone();
        }
        let id = format!("rId{}", self.entries.len() + 1);
        self.entries.push(Rel { id: id.clone(), kind, target: target.to_owned(), external });
        id
    }
}

/// Zeichen, die XML 1.0 nicht erlaubt, durch U+FFFD ersetzen; `\r` entfällt.
fn clean(text: &str) -> impl Iterator<Item = char> + '_ {
    text.chars().filter(|c| *c != '\r').map(|c| match c {
        '\t' | '\n' => c,
        c if (c as u32) < 0x20 || c == '\u{fffe}' || c == '\u{ffff}' => '\u{fffd}',
        c => c,
    })
}

/// Text für Elementinhalt und Attributwerte maskieren.
pub(super) fn esc(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in clean(text) {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\n' => out.push_str("&#10;"),
            '\t' => out.push_str("&#9;"),
            c => out.push(c),
        }
    }
    out
}

/// Ist `date` ein Zeitpunkt, den Word als `w:date` annimmt (`2026-11-20T09:30:00Z`, auch mit Sekundenbruchteilen)?
pub(super) fn valid_date(date: &str) -> bool {
    let bytes = date.as_bytes();
    bytes.len() >= 20
        && bytes[bytes.len() - 1] == b'Z'
        && bytes[..19].iter().enumerate().all(|(i, b)| match i {
            4 | 7 => *b == b'-',
            10 => *b == b'T',
            13 | 16 => *b == b':',
            _ => b.is_ascii_digit(),
        })
        && (bytes.len() == 20 || (bytes[19] == b'.' && bytes[20..bytes.len() - 1].iter().all(u8::is_ascii_digit)))
}

/// Umgebung eines Textlaufs.
#[derive(Clone, Copy, Default)]
struct Ctx<'c> {
    /// Zeichenvorlage (Zitat im Text, Insel, Verweis).
    style: Option<&'static str>,
    /// Offene Änderung: jeder Lauf wird einzeln in `w:ins`/`w:del` gefasst (so bleibt es auch um Verweise gültig).
    change: Option<&'c Change>,
    /// Der Text ist gelöscht (`w:delText`), auch wenn die Klammer `w:del` schon weiter außen steht.
    deleted: bool,
    in_footnote: bool,
}

pub(super) struct Renderer<'a> {
    doc: &'a DocxDocument,
    pub rels: BTreeMap<Part, Rels>,
    /// Fertige `w:footnote`-Elemente in der Reihenfolge ihrer Kennungen (ab [`FIRST_FOOTNOTE_ID`]).
    pub footnotes: Vec<String>,
    /// Benutzte Bilder: Index → Dateiname unter `word/media/`.
    pub media: BTreeMap<usize, String>,
    /// Zusätzliche Nummerierungen geordneter Listen: (`w:numId`, erste Nummer).
    pub extra_nums: Vec<(u32, u64)>,
    decimal_used: bool,
    next_change: u32,
    next_drawing: u32,
    /// Kennungen der vorhandenen Kommentare.
    known_comments: BTreeSet<u32>,
    /// Antworten ohne eigenen Anker je Kommentar, an dessen Stelle sie mit verankert werden.
    replies: BTreeMap<u32, Vec<u32>>,
    /// Kommentare, deren Zeichen (`w:commentReference`) schon im Text steht.
    referenced: BTreeSet<u32>,
}

fn walk_inlines(inlines: &[Inline], visit: &mut dyn FnMut(&Inline)) {
    for inline in inlines {
        visit(inline);
        match inline {
            Inline::Footnote { content }
            | Inline::Quote { content }
            | Inline::Link { content, .. }
            | Inline::Tracked { content, .. } => walk_inlines(content, visit),
            Inline::Typst { shown, .. } => walk_inlines(shown, visit),
            _ => {}
        }
    }
}

fn walk_blocks(blocks: &[Block], visit: &mut dyn FnMut(&Inline)) {
    for block in blocks {
        match block {
            Block::Part { content, .. } | Block::Heading { content, .. } | Block::Paragraph { content, .. } => {
                walk_inlines(content, visit);
            }
            Block::Quote { blocks, attribution } => {
                walk_blocks(blocks, visit);
                walk_inlines(attribution, visit);
            }
            Block::List { items, .. } => items.iter().for_each(|item| walk_inlines(&item.content, visit)),
            Block::Table { rows, .. } => rows.iter().flatten().for_each(|cell| walk_blocks(cell, visit)),
            Block::Figure { caption, .. } => walk_inlines(caption, visit),
            Block::Typst { shown, .. } => walk_blocks(shown, visit),
            Block::PageBreak => {}
        }
    }
}

/// Besteht der Inhalt ganz aus verfolgten Änderungen einer Art (und Kommentar-Ankern)? Dann gilt sie auch für die
/// Absatzmarke eines Absatzes, für den das Modell keine eigene Marke kennt (Quelle eines Zitats, Bildunterschrift).
fn uniform_change(inlines: &[Inline]) -> Option<&Change> {
    let mut found: Option<&Change> = None;
    for inline in inlines {
        match inline {
            Inline::Tracked { change, .. } if found.is_none_or(|f| f.kind == change.kind) => found = Some(change),
            Inline::CommentStart { .. } | Inline::CommentEnd { .. } => {}
            _ => return None,
        }
    }
    found
}

impl<'a> Renderer<'a> {
    pub fn new(doc: &'a DocxDocument) -> Self {
        let known_comments: BTreeSet<u32> = doc.comments.iter().map(|c| c.id).collect();
        let mut anchored = BTreeSet::new();
        walk_blocks(&doc.blocks, &mut |inline| {
            if let Inline::CommentStart { id } = inline {
                anchored.insert(*id);
            }
        });
        let mut replies: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
        for comment in &doc.comments {
            if let Some(parent) = comment.reply_to
                && !anchored.contains(&comment.id)
            {
                // Antwort auf eine Antwort: bis zum verankerten Kommentar hinaufgehen (höchstens so oft, wie es Kommentare gibt).
                let mut root = parent;
                for _ in 0..doc.comments.len() {
                    match doc.comments.iter().find(|c| c.id == root).and_then(|c| c.reply_to) {
                        Some(up) if !anchored.contains(&root) => root = up,
                        _ => break,
                    }
                }
                replies.entry(root).or_default().push(comment.id);
            }
        }
        Self {
            doc,
            rels: BTreeMap::new(),
            footnotes: Vec::new(),
            media: BTreeMap::new(),
            extra_nums: Vec::new(),
            decimal_used: false,
            next_change: 1,
            next_drawing: 1,
            known_comments,
            replies,
            referenced: BTreeSet::new(),
        }
    }

    pub fn rels_mut(&mut self, part: Part) -> &mut Rels {
        self.rels.entry(part).or_default()
    }

    /// Kommentare, deren Zeichen nirgends im Text steht (ohne Anker, Antwort auf einen Kommentar ohne Anker).
    pub fn unreferenced_comments(&self) -> Vec<u32> {
        self.doc.comments.iter().map(|c| c.id).filter(|id| !self.referenced.contains(id)).collect()
    }

    fn change_attrs(&mut self, change: &Change) -> String {
        let id = self.next_change;
        self.next_change += 1;
        let mut attrs = format!(" w:id=\"{id}\" w:author=\"{}\"", esc(&change.author));
        if valid_date(&change.date) {
            let _ = write!(attrs, " w:date=\"{}\"", esc(&change.date));
        }
        attrs
    }

    fn change_tag(kind: ChangeKind) -> &'static str {
        match kind {
            ChangeKind::Insert => "w:ins",
            ChangeKind::Delete => "w:del",
        }
    }

    /// Ein Stück auf Laufebene, bei offener Änderung in `w:ins`/`w:del` gefasst.
    fn leaf(&mut self, ctx: Ctx<'_>, inner: &str, out: &mut String) {
        match ctx.change {
            Some(change) => {
                let tag = Self::change_tag(change.kind);
                let attrs = self.change_attrs(change);
                let _ = write!(out, "<{tag}{attrs}>{inner}</{tag}>");
            }
            None => out.push_str(inner),
        }
    }

    fn run_properties(style: Option<&str>, marks: Marks, vanish: bool) -> String {
        let mut props = String::new();
        if let Some(style) = style {
            let _ = write!(props, "<w:rStyle w:val=\"{style}\"/>");
        }
        for (on, tag) in [
            (marks.bold, "<w:b/>"),
            (marks.italic, "<w:i/>"),
            (marks.small_caps, "<w:smallCaps/>"),
            (marks.strike, "<w:strike/>"),
            (vanish, "<w:vanish/>"),
            (marks.underline, "<w:u w:val=\"single\"/>"),
            (marks.superscript, "<w:vertAlign w:val=\"superscript\"/>"),
            (marks.subscript && !marks.superscript, "<w:vertAlign w:val=\"subscript\"/>"),
        ] {
            if on {
                props.push_str(tag);
            }
        }
        if props.is_empty() { props } else { format!("<w:rPr>{props}</w:rPr>") }
    }

    /// Ein Textlauf; Zeilenumbrüche werden `w:br`, Tabulatoren `w:tab`.
    fn text_run(text: &str, props: &str, deleted: bool) -> String {
        let tag = if deleted { "w:delText" } else { "w:t" };
        let mut run = format!("<w:r>{props}");
        let cleaned: String = clean(text).collect();
        let mut piece = String::new();
        let flush = |piece: &mut String, run: &mut String| {
            if !piece.is_empty() {
                let _ = write!(run, "<{tag} xml:space=\"preserve\">{}</{tag}>", esc(piece));
                piece.clear();
            }
        };
        for c in cleaned.chars() {
            match c {
                '\n' => {
                    flush(&mut piece, &mut run);
                    run.push_str("<w:br/>");
                }
                '\t' => {
                    flush(&mut piece, &mut run);
                    run.push_str("<w:tab/>");
                }
                c => piece.push(c),
            }
        }
        flush(&mut piece, &mut run);
        run.push_str("</w:r>");
        run
    }

    fn inlines(&mut self, part: Part, inlines: &[Inline], ctx: Ctx<'_>, out: &mut String) -> Result<(), DocxError> {
        for inline in inlines {
            self.inline(part, inline, ctx, out)?;
        }
        Ok(())
    }

    fn inline(&mut self, part: Part, inline: &Inline, ctx: Ctx<'_>, out: &mut String) -> Result<(), DocxError> {
        match inline {
            Inline::Text { text, marks } => {
                if !text.is_empty() {
                    let run = Self::text_run(text, &Self::run_properties(ctx.style, *marks, false), ctx.deleted);
                    self.leaf(ctx, &run, out);
                }
            }
            Inline::LineBreak => self.leaf(ctx, "<w:r><w:br/></w:r>", out),
            Inline::Footnote { content } => {
                if ctx.in_footnote {
                    // Word kennt keine Fußnote in der Fußnote: der Text bleibt, in Klammern.
                    let open = Inline::Text { text: " (".into(), marks: Marks::default() };
                    let close = Inline::Text { text: ")".into(), marks: Marks::default() };
                    self.inline(part, &open, ctx, out)?;
                    self.inlines(part, content, ctx, out)?;
                    self.inline(part, &close, ctx, out)?;
                } else {
                    let id = FIRST_FOOTNOTE_ID + self.footnotes.len() as u32;
                    // Platz reservieren, damit Fußnoten in Lesereihenfolge stehen.
                    self.footnotes.push(String::new());
                    let mut body = String::new();
                    let inner = Ctx { in_footnote: true, ..Ctx::default() };
                    self.inlines(Part::Footnotes, content, inner, &mut body)?;
                    self.footnotes[(id - FIRST_FOOTNOTE_ID) as usize] = format!(
                        "<w:footnote w:id=\"{id}\"><w:p><w:pPr><w:pStyle w:val=\"{STYLE_FOOTNOTE_TEXT}\"/></w:pPr>\
                         <w:r><w:rPr><w:rStyle w:val=\"{STYLE_FOOTNOTE_REFERENCE}\"/></w:rPr><w:footnoteRef/></w:r>\
                         <w:r><w:t xml:space=\"preserve\"> </w:t></w:r>{body}</w:p></w:footnote>"
                    );
                    let reference = format!(
                        "<w:r><w:rPr><w:rStyle w:val=\"{STYLE_FOOTNOTE_REFERENCE}\"/></w:rPr>\
                         <w:footnoteReference w:id=\"{id}\"/></w:r>"
                    );
                    self.leaf(ctx, &reference, out);
                }
            }
            Inline::Quote { content } => {
                self.inlines(part, content, Ctx { style: Some(STYLE_INLINE_QUOTE), ..ctx }, out)?;
            }
            Inline::Link { url, content } => {
                if url.trim().is_empty() {
                    self.inlines(part, content, ctx, out)?;
                } else {
                    let id = self.rels_mut(part).add(REL_HYPERLINK, url.trim(), true);
                    let _ = write!(out, "<w:hyperlink r:id=\"{id}\" w:history=\"1\">");
                    self.inlines(part, content, Ctx { style: Some(STYLE_HYPERLINK), ..ctx }, out)?;
                    out.push_str("</w:hyperlink>");
                }
            }
            Inline::Image { image, alt } => {
                let drawing = self.drawing(part, *image, alt)?;
                self.leaf(ctx, &format!("<w:r>{drawing}</w:r>"), out);
            }
            Inline::Typst { raw, shown } => {
                let mut inner = format!(
                    "<w:sdt><w:sdtPr><w:alias w:val=\"Typst\"/><w:tag w:val=\"{SDT_TAG_TYPST}\"/></w:sdtPr><w:sdtContent>"
                );
                let hidden = Self::run_properties(Some(STYLE_TYPST_CHAR), Marks::default(), true);
                inner.push_str(&Self::text_run(raw, &hidden, ctx.deleted));
                let within = Ctx { style: Some(STYLE_TYPST_CHAR), change: None, ..ctx };
                self.inlines(part, shown, within, &mut inner)?;
                inner.push_str("</w:sdtContent></w:sdt>");
                self.leaf(ctx, &inner, out);
            }
            Inline::Tracked { change, content } => {
                let deleted = ctx.deleted || change.kind == ChangeKind::Delete;
                self.inlines(part, content, Ctx { change: Some(change), deleted, ..ctx }, out)?;
            }
            Inline::CommentStart { id } => {
                if self.known_comments.contains(id) {
                    let _ = write!(out, "<w:commentRangeStart w:id=\"{id}\"/>");
                    for reply in self.replies.get(id).cloned().unwrap_or_default() {
                        let _ = write!(out, "<w:commentRangeStart w:id=\"{reply}\"/>");
                    }
                }
            }
            Inline::CommentEnd { id } => {
                if self.known_comments.contains(id) {
                    let mut ids = vec![*id];
                    ids.extend(self.replies.get(id).cloned().unwrap_or_default());
                    for id in ids {
                        if self.referenced.insert(id) {
                            let _ = write!(out, "<w:commentRangeEnd w:id=\"{id}\"/>{}", Self::comment_reference(id));
                        }
                    }
                }
            }
        }
        Ok(())
    }

    pub fn comment_reference(id: u32) -> String {
        format!(
            "<w:r><w:rPr><w:rStyle w:val=\"{STYLE_COMMENT_REFERENCE}\"/></w:rPr><w:commentReference w:id=\"{id}\"/></w:r>"
        )
    }

    /// `w:drawing` eines eingebetteten Bildes; der Pfad im Werk steht in `wp:docPr name`, der Alternativtext in
    /// `descr`.
    fn drawing(&mut self, part: Part, index: usize, alt: &str) -> Result<String, DocxError> {
        let doc = self.doc;
        let picture = doc.images.get(index).ok_or_else(|| {
            DocxError::Invalid(format!("Bild {index} gibt es im Dokument nicht ({} Bilder)", doc.images.len()))
        })?;
        let file = format!("image{}.{}", index + 1, image::extension(&picture.media_type, &picture.file_name));
        self.media.insert(index, file.clone());
        let rel = self.rels_mut(part).add(REL_IMAGE, &format!("media/{file}"), false);
        let (cx, cy) = image::extent(&picture.bytes);
        let id = self.next_drawing;
        self.next_drawing += 1;
        let name = if picture.path.is_empty() { &picture.file_name } else { &picture.path };
        Ok(format!(
            "<w:drawing><wp:inline distT=\"0\" distB=\"0\" distL=\"0\" distR=\"0\"><wp:extent cx=\"{cx}\" cy=\"{cy}\"/>\
             <wp:docPr id=\"{id}\" name=\"{name}\" descr=\"{descr}\"/>\
             <wp:cNvGraphicFramePr><a:graphicFrameLocks noChangeAspect=\"1\"/></wp:cNvGraphicFramePr>\
             <a:graphic><a:graphicData uri=\"{NS_PIC}\"><pic:pic>\
             <pic:nvPicPr><pic:cNvPr id=\"{id}\" name=\"{file_name}\" descr=\"{descr}\"/><pic:cNvPicPr/></pic:nvPicPr>\
             <pic:blipFill><a:blip r:embed=\"{rel}\"/><a:stretch><a:fillRect/></a:stretch></pic:blipFill>\
             <pic:spPr><a:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"{cx}\" cy=\"{cy}\"/></a:xfrm>\
             <a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom></pic:spPr>\
             </pic:pic></a:graphicData></a:graphic></wp:inline></w:drawing>",
            name = esc(name),
            descr = esc(alt),
            file_name = esc(&picture.file_name),
        ))
    }

    /// Ein Absatz: Vorlage, weitere Absatzeigenschaften (`extra`, in Schemareihenfolge hinter `w:pStyle`),
    /// verfolgte Absatzmarke, Inhalt.
    #[allow(clippy::too_many_arguments)]
    fn paragraph(
        &mut self,
        part: Part,
        style: Option<&str>,
        extra: &str,
        mark: Option<&Change>,
        content: &[Inline],
        ctx: Ctx<'_>,
        out: &mut String,
    ) -> Result<(), DocxError> {
        let mut props = String::new();
        if let Some(style) = style {
            let _ = write!(props, "<w:pStyle w:val=\"{style}\"/>");
        }
        props.push_str(extra);
        if let Some(mark) = mark {
            let tag = Self::change_tag(mark.kind);
            let attrs = self.change_attrs(mark);
            let _ = write!(props, "<w:rPr><{tag}{attrs}/></w:rPr>");
        }
        out.push_str("<w:p>");
        if !props.is_empty() {
            let _ = write!(out, "<w:pPr>{props}</w:pPr>");
        }
        self.inlines(part, content, ctx, out)?;
        out.push_str("</w:p>");
        Ok(())
    }

    /// Verborgener Absatz mit dem wörtlichen Typst-Quelltext einer Insel.
    fn hidden_source(raw: &str, out: &mut String) {
        let props = Self::run_properties(None, Marks::default(), true);
        let _ = write!(
            out,
            "<w:p><w:pPr><w:pStyle w:val=\"{STYLE_TYPST}\"/><w:rPr><w:vanish/></w:rPr></w:pPr>{}</w:p>",
            Self::text_run(raw, &props, false)
        );
    }

    fn sdt_open(out: &mut String) {
        let _ = write!(
            out,
            "<w:sdt><w:sdtPr><w:alias w:val=\"Typst\"/><w:tag w:val=\"{SDT_TAG_TYPST}\"/></w:sdtPr><w:sdtContent>"
        );
    }

    /// Schreibt Blöcke; `plain` ist die Vorlage gewöhnlicher Absätze in dieser Umgebung (Zitat, Insel).
    pub fn blocks(&mut self, blocks: &[Block], plain: Option<&'static str>, out: &mut String) -> Result<(), DocxError> {
        let part = Part::Document;
        let ctx = Ctx::default();
        for block in blocks {
            match block {
                Block::Part { content, mark } => {
                    self.paragraph(part, Some(STYLE_PART), "", mark.as_ref(), content, ctx, out)?;
                }
                Block::Heading { level, numbered, content, mark } => {
                    let style = if *numbered || *level > MAX_OUTLINE_LEVEL {
                        heading_style(*level)
                    } else {
                        unnumbered_style(*level)
                    };
                    self.paragraph(part, Some(&style), "", mark.as_ref(), content, ctx, out)?;
                }
                Block::Paragraph { content, mark } => {
                    self.paragraph(part, plain, "", mark.as_ref(), content, ctx, out)?;
                }
                Block::Quote { blocks, attribution } => {
                    self.blocks(blocks, Some(STYLE_QUOTE), out)?;
                    if !attribution.is_empty() {
                        let mark = uniform_change(attribution).cloned();
                        self.paragraph(part, Some(STYLE_QUOTE_SOURCE), "", mark.as_ref(), attribution, ctx, out)?;
                    }
                }
                Block::List { ordered, start, items } => self.list(*ordered, *start, items, out)?,
                Block::Table { header_rows, rows } => self.table(*header_rows, rows, out)?,
                Block::Figure { image, alt, caption, raw } => {
                    if !raw.is_empty() {
                        Self::sdt_open(out);
                        Self::hidden_source(raw, out);
                    }
                    let picture = [Inline::Image { image: *image, alt: alt.clone() }];
                    let extra = if caption.is_empty() {
                        "<w:jc w:val=\"center\"/>"
                    } else {
                        "<w:keepNext/><w:jc w:val=\"center\"/>"
                    };
                    self.paragraph(part, None, extra, None, &picture, ctx, out)?;
                    if !caption.is_empty() {
                        let mark = uniform_change(caption).cloned();
                        self.paragraph(part, Some(STYLE_CAPTION), "", mark.as_ref(), caption, ctx, out)?;
                    }
                    if !raw.is_empty() {
                        out.push_str("</w:sdtContent></w:sdt>");
                    }
                }
                Block::Typst { raw, shown } => {
                    Self::sdt_open(out);
                    Self::hidden_source(raw, out);
                    self.blocks(shown, Some(STYLE_TYPST), out)?;
                    out.push_str("</w:sdtContent></w:sdt>");
                }
                Block::PageBreak => out.push_str("<w:p><w:r><w:br w:type=\"page\"/></w:r></w:p>"),
            }
        }
        Ok(())
    }

    fn list(&mut self, ordered: bool, start: u64, items: &[ListItem], out: &mut String) -> Result<(), DocxError> {
        let num = if !ordered {
            NUM_BULLET
        } else if start == 1 && !self.decimal_used {
            self.decimal_used = true;
            NUM_DECIMAL
        } else {
            // Jede weitere geordnete Liste zählt für sich: eigene Nummerierung mit ihrer ersten Nummer.
            let id = NUM_DECIMAL + 1 + self.extra_nums.len() as u32;
            self.extra_nums.push((id, start));
            id
        };
        for item in items {
            let extra =
                format!("<w:numPr><w:ilvl w:val=\"{}\"/><w:numId w:val=\"{num}\"/></w:numPr>", item.depth.min(8));
            let style = Some(STYLE_LIST_PARAGRAPH);
            self.paragraph(Part::Document, style, &extra, item.mark.as_ref(), &item.content, Ctx::default(), out)?;
        }
        Ok(())
    }

    fn table(&mut self, header_rows: usize, rows: &[Vec<Vec<Block>>], out: &mut String) -> Result<(), DocxError> {
        let columns = rows.iter().map(Vec::len).max().unwrap_or(0);
        if columns == 0 {
            return Ok(());
        }
        // Satzspiegel der Seite (A4 mit 2,5 cm Rand) in Twips, gleichmäßig auf die Spalten verteilt.
        let width = image::TEXT_WIDTH_TWIPS / columns as u32;
        out.push_str("<w:tbl><w:tblPr><w:tblW w:w=\"5000\" w:type=\"pct\"/><w:tblBorders>");
        for side in ["top", "left", "bottom", "right", "insideH", "insideV"] {
            let _ = write!(out, "<w:{side} w:val=\"single\" w:sz=\"4\" w:space=\"0\" w:color=\"auto\"/>");
        }
        out.push_str("</w:tblBorders></w:tblPr><w:tblGrid>");
        for _ in 0..columns {
            let _ = write!(out, "<w:gridCol w:w=\"{width}\"/>");
        }
        out.push_str("</w:tblGrid>");
        for (index, row) in rows.iter().enumerate() {
            out.push_str("<w:tr>");
            if index < header_rows {
                out.push_str("<w:trPr><w:tblHeader/></w:trPr>");
            }
            for column in 0..columns {
                let _ = write!(out, "<w:tc><w:tcPr><w:tcW w:w=\"{width}\" w:type=\"dxa\"/></w:tcPr>");
                let mut cell = String::new();
                if let Some(blocks) = row.get(column) {
                    self.blocks(blocks, None, &mut cell)?;
                }
                // Eine Zelle endet immer mit einem Absatz, sonst öffnet Word die Datei nicht.
                if !cell.ends_with("</w:p>") {
                    cell.push_str("<w:p/>");
                }
                out.push_str(&cell);
                out.push_str("</w:tc>");
            }
            out.push_str("</w:tr>");
        }
        out.push_str("</w:tbl>");
        Ok(())
    }

    /// Ein Absatz der Titelei.
    pub fn front_line(&mut self, style: &str, text: &str, out: &mut String) -> Result<(), DocxError> {
        if text.trim().is_empty() {
            return Ok(());
        }
        let content = [Inline::Text { text: text.trim().to_owned(), marks: Marks::default() }];
        self.paragraph(Part::Document, Some(style), "", None, &content, Ctx::default(), out)
    }
}
