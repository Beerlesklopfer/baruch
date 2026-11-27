// SPDX-License-Identifier: AGPL-3.0-or-later
//! Shared constants for reader and writer (interface R32-S1, frozen): part paths in the archive, media types,
//! namespaces, relationship types, the styles of the writer and the marker of the Typst island. What the writer
//! stores under these names, the reader recognises again (round trip).

// --- Part paths in the archive ---------------------------------------------------------------------------------

pub const PART_CONTENT_TYPES: &str = "[Content_Types].xml";
pub const PART_ROOT_RELS: &str = "_rels/.rels";
pub const PART_DOCUMENT: &str = "word/document.xml";
pub const PART_DOCUMENT_RELS: &str = "word/_rels/document.xml.rels";
pub const PART_STYLES: &str = "word/styles.xml";
pub const PART_FOOTNOTES: &str = "word/footnotes.xml";
/// Reader only: endnotes are read as footnotes (with a warning); the writer creates none.
pub const PART_ENDNOTES: &str = "word/endnotes.xml";
pub const PART_COMMENTS: &str = "word/comments.xml";
/// Reader only: replies to comments (`w15:commentEx`), as far as readable.
pub const PART_COMMENTS_EXTENDED: &str = "word/commentsExtended.xml";
pub const PART_NUMBERING: &str = "word/numbering.xml";
pub const PART_SETTINGS: &str = "word/settings.xml";
pub const PART_CORE: &str = "docProps/core.xml";
pub const PART_APP: &str = "docProps/app.xml";
/// Folder of the embedded images (with a trailing slash).
pub const MEDIA_DIR: &str = "word/media/";

// --- Media types -----------------------------------------------------------------------------------------------

/// Media type of a .docx file (HTTP `Content-Type`, RPC `media_type`).
pub const MEDIA_TYPE_DOCX: &str = "application/vnd.openxmlformats-officedocument.wordprocessingml.document";
/// Content types of the parts in `[Content_Types].xml`.
pub const CT_DOCUMENT: &str = "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml";
pub const CT_STYLES: &str = "application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml";
pub const CT_FOOTNOTES: &str = "application/vnd.openxmlformats-officedocument.wordprocessingml.footnotes+xml";
pub const CT_COMMENTS: &str = "application/vnd.openxmlformats-officedocument.wordprocessingml.comments+xml";
pub const CT_NUMBERING: &str = "application/vnd.openxmlformats-officedocument.wordprocessingml.numbering+xml";
pub const CT_SETTINGS: &str = "application/vnd.openxmlformats-officedocument.wordprocessingml.settings+xml";
pub const CT_CORE: &str = "application/vnd.openxmlformats-package.core-properties+xml";
pub const CT_APP: &str = "application/vnd.openxmlformats-officedocument.extended-properties+xml";
pub const CT_RELS: &str = "application/vnd.openxmlformats-package.relationships+xml";
pub const CT_XML: &str = "application/xml";

// --- Namespaces ------------------------------------------------------------------------------------------------

pub const NS_W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
pub const NS_R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
pub const NS_WP: &str = "http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing";
pub const NS_A: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
pub const NS_PIC: &str = "http://schemas.openxmlformats.org/drawingml/2006/picture";
pub const NS_CP: &str = "http://schemas.openxmlformats.org/package/2006/metadata/core-properties";
pub const NS_DC: &str = "http://purl.org/dc/elements/1.1/";
pub const NS_DCTERMS: &str = "http://purl.org/dc/terms/";
pub const NS_XSI: &str = "http://www.w3.org/2001/XMLSchema-instance";
/// Replies to comments (`commentsExtended.xml`, reader only).
pub const NS_W15: &str = "http://schemas.microsoft.com/office/word/2012/wordml";
/// Paragraph identifier `w14:paraId` (reference of the comment replies, reader only).
pub const NS_W14: &str = "http://schemas.microsoft.com/office/word/2010/wordml";
/// Namespace of `[Content_Types].xml`.
pub const NS_CONTENT_TYPES: &str = "http://schemas.openxmlformats.org/package/2006/content-types";
/// Namespace of the relationship files (`*.rels`).
pub const NS_PACKAGE_RELS: &str = "http://schemas.openxmlformats.org/package/2006/relationships";

// --- Relationship types ----------------------------------------------------------------------------------------

pub const REL_OFFICE_DOCUMENT: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument";
pub const REL_CORE: &str = "http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties";
pub const REL_APP: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/extended-properties";
pub const REL_STYLES: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles";
pub const REL_FOOTNOTES: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/footnotes";
pub const REL_ENDNOTES: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/endnotes";
pub const REL_COMMENTS: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/comments";
pub const REL_NUMBERING: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/numbering";
pub const REL_SETTINGS: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/settings";
pub const REL_IMAGE: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/image";
pub const REL_HYPERLINK: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink";
/// Value of `TargetMode` for targets outside the file (hyperlinks; images of that kind are never fetched).
pub const TARGET_MODE_EXTERNAL: &str = "External";

// --- Styles of the writer (w:styleId) --------------------------------------------------------------------------

pub const STYLE_TITLE: &str = "Title";
pub const STYLE_SUBTITLE: &str = "Subtitle";
/// Heading 1–6: `Heading1` … `Heading6`, `w:name` "heading N" ([`heading_style`], [`heading_name`]).
pub const STYLE_HEADING_PREFIX: &str = "Heading";
pub const STYLE_QUOTE: &str = "Quote";
pub const STYLE_LIST_PARAGRAPH: &str = "ListParagraph";
pub const STYLE_FOOTNOTE_TEXT: &str = "FootnoteText";
/// Character style of the footnote mark.
pub const STYLE_FOOTNOTE_REFERENCE: &str = "FootnoteReference";
pub const STYLE_CAPTION: &str = "Caption";
/// Part of the work; `w:name` [`STYLE_PART_NAME`].
pub const STYLE_PART: &str = "BaruchTeil";
pub const STYLE_PART_NAME: &str = "Baruch Teil";
/// Heading without number: `BaruchOhneNummer1` … `3`, `basedOn` Heading N ([`unnumbered_style`]).
pub const STYLE_UNNUMBERED_PREFIX: &str = "BaruchOhneNummer";
pub const STYLE_AUTHOR: &str = "BaruchAutor";
pub const STYLE_IMPRINT: &str = "BaruchImpressum";
/// Source of a block quote.
pub const STYLE_QUOTE_SOURCE: &str = "BaruchZitatquelle";
/// Character style: inline quote.
pub const STYLE_INLINE_QUOTE: &str = "BaruchZitat";
/// Paragraph style of a Typst island as a block.
pub const STYLE_TYPST: &str = "BaruchTypst";
/// Character style of a Typst island in the text.
pub const STYLE_TYPST_CHAR: &str = "BaruchTypstZeichen";

/// Highest level of a heading.
pub const MAX_HEADING_LEVEL: u8 = 6;
/// Highest level that belongs to the outline of the work (and has a style "without number").
pub const MAX_OUTLINE_LEVEL: u8 = 3;

/// Style ID of the heading `level` (1–6), e.g. `Heading2`; outside the range the value is clamped.
pub fn heading_style(level: u8) -> String {
    format!("{STYLE_HEADING_PREFIX}{}", level.clamp(1, MAX_HEADING_LEVEL))
}

/// `w:name` of the heading `level` (1–6), e.g. `heading 2` – by this Word recognises navigation and table of contents.
pub fn heading_name(level: u8) -> String {
    format!("heading {}", level.clamp(1, MAX_HEADING_LEVEL))
}

/// Style ID of the heading without number (1–3), e.g. `BaruchOhneNummer1`; outside the range the value is clamped.
pub fn unnumbered_style(level: u8) -> String {
    format!("{STYLE_UNNUMBERED_PREFIX}{}", level.clamp(1, MAX_OUTLINE_LEVEL))
}

// --- Typst island ----------------------------------------------------------------------------------------------

/// Marker of the island: content control `w:sdt` with `w:tag w:val="baruch-typst"`; inside it first the Typst
/// source text hidden with `w:vanish`, then the visible part (`shown`).
pub const SDT_TAG_TYPST: &str = "baruch-typst";

// --- Numberings (w:numId in numbering.xml) ---------------------------------------------------------------------

/// Unordered list (bullets).
pub const NUM_BULLET: u32 = 1;
/// Ordered list (decimal numbers).
pub const NUM_DECIMAL: u32 = 2;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn style_ids_follow_the_contract() {
        assert_eq!(
            heading_style(1),
            "Heading1",
            "Überschrift 1 heißt wie in Word, damit Navigation und Verzeichnis sie kennen"
        );
        assert_eq!(heading_style(6), "Heading6", "sechs Ebenen gibt es");
        assert_eq!(
            heading_style(9),
            "Heading6",
            "eine zu große Ebene wird geklemmt statt eine unbekannte Vorlage zu nennen"
        );
        assert_eq!(heading_name(3), "heading 3", "w:name ist der eingebaute Name „heading N“");
        assert_eq!(unnumbered_style(2), "BaruchOhneNummer2", "ohne Nummer: eigene Vorlage je Gliederungsebene");
        assert_eq!(unnumbered_style(0), "BaruchOhneNummer1", "Ebene 0 gibt es als Überschrift nicht – geklemmt auf 1");
        assert!(
            MEDIA_DIR.ends_with('/'),
            "der Medienordner endet mit Schrägstrich, damit Pfade einfach angehängt werden"
        );
    }
}
