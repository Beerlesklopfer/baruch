// SPDX-License-Identifier: AGPL-3.0-or-later
//! Writer: [`DocxDocument`] → .docx (task docx_export, contract Round 32 §3).
//!
//! Promises (contract): valid OOXML according to [`crate::ooxml`], deterministic (fixed ZIP times, fixed order:
//! same input → same bytes), characters forbidden in XML are replaced instead of writing a broken document.
//!
//! Structure of the file:
//! - Front matter from [`crate::DocxMeta`]: title (`Title`), subtitle (`Subtitle`), author (`BaruchAutor`), one
//!   paragraph per line of the imprint (`BaruchImpressum`); the same details are in `docProps/core.xml`
//!   (`dc:title`, `dc:subject` = subtitle, `dc:creator`, `dc:description` = imprint, `dc:language`).
//! - Blocks according to [`body`]: headings `Heading1…6` or `BaruchOhneNummer1…3`, parts `BaruchTeil`, block quote
//!   `Quote` with source `BaruchZitatquelle`, lists through `numbering.xml`, tables, figures with `Caption`.
//! - Footnotes for real in `word/footnotes.xml` (identifiers from 2; 0 and 1 are the separator lines), comments
//!   in `word/comments.xml`. Replies (`reply_to`) are comments of their own at the place of the comment they reply
//!   to; the writer does not write `commentsExtended.xml` (defined in [`crate::ooxml`] for the reader only).
//! - Images under `word/media/imageN.<endung>` (N = index in the document + 1); the path in the work is in
//!   `wp:docPr name`, the alternative text in `descr`.
//! - Typst island: `w:sdt` with `w:tag w:val="baruch-typst"`, inside it first the source text hidden with
//!   `w:vanish` (line break = `w:br`), then the visible part. A figure with `raw` is likewise in an island.
//! - Tracked changes: `w:ins`/`w:del` around every run (deleted text as `w:delText`), the paragraph mark in
//!   `w:pPr/w:rPr`.

mod body;
mod image;
mod styles;

use std::fmt::Write as _;
use std::io::{Cursor, Write as _};

use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, DateTime, ZipWriter};

use self::body::{Part, Rels, Renderer, esc, valid_date};
use crate::ooxml::*;
use crate::{DocxDocument, DocxError};

/// Namespace of `docProps/app.xml`.
const NS_APP: &str = "http://schemas.openxmlformats.org/officeDocument/2006/extended-properties";
const XML_HEAD: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n";
/// Relationships of the footnotes (references and images in footnotes).
const PART_FOOTNOTES_RELS: &str = "word/_rels/footnotes.xml.rels";

/// Details that do not come from the document.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WriteOptions {
    /// Time for `docProps/core.xml` (ISO 8601 UTC); empty = not given.
    pub created: String,
    /// Name of the generating program for `docProps/app.xml` (e.g. "Baruch 0.9.0").
    pub generator: String,
}

fn namespaces() -> String {
    format!("xmlns:w=\"{NS_W}\" xmlns:r=\"{NS_R}\" xmlns:wp=\"{NS_WP}\" xmlns:a=\"{NS_A}\" xmlns:pic=\"{NS_PIC}\"")
}

fn rels_xml(rels: &Rels) -> String {
    let mut out = format!("{XML_HEAD}<Relationships xmlns=\"{NS_PACKAGE_RELS}\">");
    for rel in &rels.entries {
        let _ = write!(out, "<Relationship Id=\"{}\" Type=\"{}\" Target=\"{}\"", rel.id, rel.kind, esc(&rel.target));
        if rel.external {
            let _ = write!(out, " TargetMode=\"{TARGET_MODE_EXTERNAL}\"");
        }
        out.push_str("/>");
    }
    out.push_str("</Relationships>");
    out
}

fn core_xml(document: &DocxDocument, options: &WriteOptions) -> String {
    let meta = &document.meta;
    let mut out = format!(
        "{XML_HEAD}<cp:coreProperties xmlns:cp=\"{NS_CP}\" xmlns:dc=\"{NS_DC}\" xmlns:dcterms=\"{NS_DCTERMS}\" \
         xmlns:xsi=\"{NS_XSI}\">"
    );
    let imprint = meta.imprint.join("\n");
    for (tag, value) in [
        ("dc:title", meta.title.as_str()),
        ("dc:subject", meta.subtitle.as_str()),
        ("dc:creator", meta.author.as_str()),
        ("dc:description", imprint.as_str()),
        ("dc:language", meta.lang.as_str()),
    ] {
        if !value.trim().is_empty() {
            // In element content the line break stays a line break.
            let _ = write!(out, "<{tag}>{}</{tag}>", esc(value.trim()).replace("&#10;", "\n"));
        }
    }
    if valid_date(&options.created) {
        for tag in ["dcterms:created", "dcterms:modified"] {
            let _ = write!(out, "<{tag} xsi:type=\"dcterms:W3CDTF\">{}</{tag}>", esc(&options.created));
        }
    }
    out.push_str("</cp:coreProperties>");
    out
}

fn comments_xml(document: &DocxDocument) -> String {
    let mut out = format!("{XML_HEAD}<w:comments {}>", namespaces());
    for comment in &document.comments {
        let _ = write!(out, "<w:comment w:id=\"{}\" w:author=\"{}\"", comment.id, esc(&comment.author));
        if valid_date(&comment.date) {
            let _ = write!(out, " w:date=\"{}\"", esc(&comment.date));
        }
        if !comment.initials.is_empty() {
            let _ = write!(out, " w:initials=\"{}\"", esc(&comment.initials));
        }
        out.push('>');
        for (index, line) in comment.text.lines().chain(comment.text.is_empty().then_some("")).enumerate() {
            let _ = write!(out, "<w:p><w:pPr><w:pStyle w:val=\"{}\"/></w:pPr>", body::STYLE_COMMENT_TEXT);
            if index == 0 {
                let _ = write!(
                    out,
                    "<w:r><w:rPr><w:rStyle w:val=\"{}\"/></w:rPr><w:annotationRef/></w:r>",
                    body::STYLE_COMMENT_REFERENCE
                );
            }
            if !line.is_empty() {
                let _ = write!(out, "<w:r><w:t xml:space=\"preserve\">{}</w:t></w:r>", esc(line));
            }
            out.push_str("</w:p>");
        }
        out.push_str("</w:comment>");
    }
    out.push_str("</w:comments>");
    out
}

fn footnotes_xml(footnotes: &[String]) -> String {
    let mut out = format!(
        "{XML_HEAD}<w:footnotes {}>\
         <w:footnote w:type=\"separator\" w:id=\"0\"><w:p><w:r><w:separator/></w:r></w:p></w:footnote>\
         <w:footnote w:type=\"continuationSeparator\" w:id=\"1\"><w:p><w:r><w:continuationSeparator/></w:r></w:p>\
         </w:footnote>",
        namespaces()
    );
    footnotes.iter().for_each(|footnote| out.push_str(footnote));
    out.push_str("</w:footnotes>");
    out
}

/// Writes the document as .docx.
pub fn write(document: &DocxDocument, options: &WriteOptions) -> Result<Vec<u8>, DocxError> {
    let mut renderer = Renderer::new(document);
    let has_comments = !document.comments.is_empty();

    // Fixed relationships of the main part first, so that their identifiers do not depend on the content.
    {
        let rels = renderer.rels_mut(Part::Document);
        rels.add(REL_STYLES, "styles.xml", false);
        rels.add(REL_SETTINGS, "settings.xml", false);
        rels.add(REL_NUMBERING, "numbering.xml", false);
        rels.add(REL_FOOTNOTES, "footnotes.xml", false);
        if has_comments {
            rels.add(REL_COMMENTS, "comments.xml", false);
        }
    }

    let mut text = String::new();
    let meta = &document.meta;
    renderer.front_line(STYLE_TITLE, &meta.title, &mut text)?;
    renderer.front_line(STYLE_SUBTITLE, &meta.subtitle, &mut text)?;
    renderer.front_line(STYLE_AUTHOR, &meta.author, &mut text)?;
    for line in &meta.imprint {
        renderer.front_line(STYLE_IMPRINT, line, &mut text)?;
    }
    renderer.blocks(&document.blocks, None, &mut text)?;
    // Comments without a place in the text: their mark is in a paragraph at the end, otherwise Word would not show
    // them.
    let loose = renderer.unreferenced_comments();
    if !loose.is_empty() {
        text.push_str("<w:p>");
        loose.iter().for_each(|id| text.push_str(&Renderer::comment_reference(*id)));
        text.push_str("</w:p>");
    }
    let document_xml = format!(
        "{XML_HEAD}<w:document {}><w:body>{text}<w:sectPr><w:pgSz w:w=\"{}\" w:h=\"{}\"/>\
         <w:pgMar w:top=\"{}\" w:right=\"{side}\" w:bottom=\"{}\" w:left=\"{side}\" w:header=\"708\" w:footer=\"708\" \
         w:gutter=\"0\"/></w:sectPr></w:body></w:document>",
        namespaces(),
        image::PAGE_WIDTH_TWIPS,
        image::PAGE_HEIGHT_TWIPS,
        image::MARGIN_TOP_TWIPS,
        image::MARGIN_BOTTOM_TWIPS,
        side = image::MARGIN_SIDE_TWIPS,
    );

    // Content types: extensions of the images, then the parts.
    let mut extensions: Vec<&str> =
        renderer.media.values().filter_map(|file| file.rsplit_once('.').map(|(_, ext)| ext)).collect();
    extensions.sort_unstable();
    extensions.dedup();
    let mut types = format!(
        "{XML_HEAD}<Types xmlns=\"{NS_CONTENT_TYPES}\"><Default Extension=\"rels\" ContentType=\"{CT_RELS}\"/>\
         <Default Extension=\"xml\" ContentType=\"{CT_XML}\"/>"
    );
    for extension in extensions {
        let _ =
            write!(types, "<Default Extension=\"{extension}\" ContentType=\"{}\"/>", image::content_type(extension));
    }
    let mut overrides = vec![
        (PART_DOCUMENT, CT_DOCUMENT),
        (PART_STYLES, CT_STYLES),
        (PART_SETTINGS, CT_SETTINGS),
        (PART_NUMBERING, CT_NUMBERING),
        (PART_FOOTNOTES, CT_FOOTNOTES),
    ];
    if has_comments {
        overrides.push((PART_COMMENTS, CT_COMMENTS));
    }
    overrides.extend([(PART_CORE, CT_CORE), (PART_APP, CT_APP)]);
    for (part, content_type) in overrides {
        let _ = write!(types, "<Override PartName=\"/{part}\" ContentType=\"{content_type}\"/>");
    }
    types.push_str("</Types>");

    let mut root = Rels::default();
    root.add(REL_OFFICE_DOCUMENT, PART_DOCUMENT, false);
    root.add(REL_CORE, PART_CORE, false);
    root.add(REL_APP, PART_APP, false);
    let app = format!(
        "{XML_HEAD}<Properties xmlns=\"{NS_APP}\"><Application>{}</Application></Properties>",
        esc(options.generator.trim())
    );

    let mut files: Vec<(String, Vec<u8>)> = vec![
        (PART_CONTENT_TYPES.into(), types.into_bytes()),
        (PART_ROOT_RELS.into(), rels_xml(&root).into_bytes()),
        (PART_APP.into(), app.into_bytes()),
        (PART_CORE.into(), core_xml(document, options).into_bytes()),
        (PART_DOCUMENT.into(), document_xml.into_bytes()),
        (PART_DOCUMENT_RELS.into(), rels_xml(renderer.rels_mut(Part::Document)).into_bytes()),
        (PART_STYLES.into(), styles::styles_xml(&meta.lang).into_bytes()),
        (PART_SETTINGS.into(), styles::settings_xml().into_bytes()),
        (PART_NUMBERING.into(), styles::numbering_xml(&renderer.extra_nums).into_bytes()),
        (PART_FOOTNOTES.into(), footnotes_xml(&renderer.footnotes).into_bytes()),
    ];
    if let Some(rels) = renderer.rels.get(&Part::Footnotes).filter(|rels| !rels.entries.is_empty()) {
        files.push((PART_FOOTNOTES_RELS.into(), rels_xml(rels).into_bytes()));
    }
    if has_comments {
        files.push((PART_COMMENTS.into(), comments_xml(document).into_bytes()));
    }
    for (index, file) in &renderer.media {
        files.push((format!("{MEDIA_DIR}{file}"), document.images[*index].bytes.clone()));
    }

    let failed = |e: &dyn std::fmt::Display| DocxError::Write(e.to_string());
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    // Fixed time (1980-01-01) and fixed permissions: same input yields same bytes.
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .last_modified_time(DateTime::default())
        .unix_permissions(0o644);
    for (name, bytes) in &files {
        zip.start_file(name.as_str(), options).map_err(|e| failed(&e))?;
        zip.write_all(bytes).map_err(|e| failed(&e))?;
    }
    Ok(zip.finish().map_err(|e| failed(&e))?.into_inner())
}
