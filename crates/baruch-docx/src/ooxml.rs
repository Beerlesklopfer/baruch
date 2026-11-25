// SPDX-License-Identifier: AGPL-3.0-or-later
//! Gemeinsame Konstanten für Leser und Schreiber (Schnittstelle R32-S1, eingefroren): Teilpfade im Archiv,
//! Medientypen, Namensräume, Beziehungsarten, die Formatvorlagen des Schreibers und das Kennzeichen der
//! Typst-Insel. Was der Schreiber unter diesen Namen ablegt, erkennt der Leser wieder (Rundreise).

// --- Teilpfade im Archiv ---------------------------------------------------------------------------------------

pub const PART_CONTENT_TYPES: &str = "[Content_Types].xml";
pub const PART_ROOT_RELS: &str = "_rels/.rels";
pub const PART_DOCUMENT: &str = "word/document.xml";
pub const PART_DOCUMENT_RELS: &str = "word/_rels/document.xml.rels";
pub const PART_STYLES: &str = "word/styles.xml";
pub const PART_FOOTNOTES: &str = "word/footnotes.xml";
/// Nur der Leser: Endnoten werden als Fußnoten gelesen (mit Warnung); der Schreiber legt keine an.
pub const PART_ENDNOTES: &str = "word/endnotes.xml";
pub const PART_COMMENTS: &str = "word/comments.xml";
/// Nur der Leser: Antworten auf Kommentare (`w15:commentEx`), soweit lesbar.
pub const PART_COMMENTS_EXTENDED: &str = "word/commentsExtended.xml";
pub const PART_NUMBERING: &str = "word/numbering.xml";
pub const PART_SETTINGS: &str = "word/settings.xml";
pub const PART_CORE: &str = "docProps/core.xml";
pub const PART_APP: &str = "docProps/app.xml";
/// Ordner der eingebetteten Bilder (mit Schrägstrich am Ende).
pub const MEDIA_DIR: &str = "word/media/";

// --- Medientypen -----------------------------------------------------------------------------------------------

/// Medientyp einer .docx-Datei (HTTP `Content-Type`, RPC `media_type`).
pub const MEDIA_TYPE_DOCX: &str = "application/vnd.openxmlformats-officedocument.wordprocessingml.document";
/// Inhaltstypen der Teile in `[Content_Types].xml`.
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

// --- Namensräume -----------------------------------------------------------------------------------------------

pub const NS_W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
pub const NS_R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
pub const NS_WP: &str = "http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing";
pub const NS_A: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
pub const NS_PIC: &str = "http://schemas.openxmlformats.org/drawingml/2006/picture";
pub const NS_CP: &str = "http://schemas.openxmlformats.org/package/2006/metadata/core-properties";
pub const NS_DC: &str = "http://purl.org/dc/elements/1.1/";
pub const NS_DCTERMS: &str = "http://purl.org/dc/terms/";
pub const NS_XSI: &str = "http://www.w3.org/2001/XMLSchema-instance";
/// Antworten auf Kommentare (`commentsExtended.xml`, nur der Leser).
pub const NS_W15: &str = "http://schemas.microsoft.com/office/word/2012/wordml";
/// Absatzkennung `w14:paraId` (Bezug der Kommentar-Antworten, nur der Leser).
pub const NS_W14: &str = "http://schemas.microsoft.com/office/word/2010/wordml";
/// Namensraum von `[Content_Types].xml`.
pub const NS_CONTENT_TYPES: &str = "http://schemas.openxmlformats.org/package/2006/content-types";
/// Namensraum der Beziehungsdateien (`*.rels`).
pub const NS_PACKAGE_RELS: &str = "http://schemas.openxmlformats.org/package/2006/relationships";

// --- Beziehungsarten -------------------------------------------------------------------------------------------

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
/// Wert von `TargetMode` für Ziele außerhalb der Datei (Hyperlinks; Bilder so werden nie abgerufen).
pub const TARGET_MODE_EXTERNAL: &str = "External";

// --- Formatvorlagen des Schreibers (w:styleId) -----------------------------------------------------------------

pub const STYLE_TITLE: &str = "Title";
pub const STYLE_SUBTITLE: &str = "Subtitle";
/// Überschrift 1–6: `Heading1` … `Heading6`, `w:name` „heading N“ ([`heading_style`], [`heading_name`]).
pub const STYLE_HEADING_PREFIX: &str = "Heading";
pub const STYLE_QUOTE: &str = "Quote";
pub const STYLE_LIST_PARAGRAPH: &str = "ListParagraph";
pub const STYLE_FOOTNOTE_TEXT: &str = "FootnoteText";
/// Zeichenvorlage des Fußnotenzeichens.
pub const STYLE_FOOTNOTE_REFERENCE: &str = "FootnoteReference";
pub const STYLE_CAPTION: &str = "Caption";
/// Teil des Werks; `w:name` [`STYLE_PART_NAME`].
pub const STYLE_PART: &str = "BaruchTeil";
pub const STYLE_PART_NAME: &str = "Baruch Teil";
/// Überschrift ohne Nummer: `BaruchOhneNummer1` … `3`, `basedOn` Heading N ([`unnumbered_style`]).
pub const STYLE_UNNUMBERED_PREFIX: &str = "BaruchOhneNummer";
pub const STYLE_AUTHOR: &str = "BaruchAutor";
pub const STYLE_IMPRINT: &str = "BaruchImpressum";
/// Quellenangabe eines Blockzitats.
pub const STYLE_QUOTE_SOURCE: &str = "BaruchZitatquelle";
/// Zeichenvorlage: Zitat im Fließtext.
pub const STYLE_INLINE_QUOTE: &str = "BaruchZitat";
/// Absatzvorlage einer Typst-Insel als Block.
pub const STYLE_TYPST: &str = "BaruchTypst";
/// Zeichenvorlage einer Typst-Insel im Text.
pub const STYLE_TYPST_CHAR: &str = "BaruchTypstZeichen";

/// Größte Ebene einer Überschrift.
pub const MAX_HEADING_LEVEL: u8 = 6;
/// Größte Ebene, die zur Gliederung des Werks gehört (und eine Vorlage „ohne Nummer“ hat).
pub const MAX_OUTLINE_LEVEL: u8 = 3;

/// Vorlagen-ID der Überschrift `level` (1–6), z. B. `Heading2`; außerhalb des Bereichs wird geklemmt.
pub fn heading_style(level: u8) -> String {
    format!("{STYLE_HEADING_PREFIX}{}", level.clamp(1, MAX_HEADING_LEVEL))
}

/// `w:name` der Überschrift `level` (1–6), z. B. `heading 2` – daran erkennt Word Navigation und Inhaltsverzeichnis.
pub fn heading_name(level: u8) -> String {
    format!("heading {}", level.clamp(1, MAX_HEADING_LEVEL))
}

/// Vorlagen-ID der Überschrift ohne Nummer (1–3), z. B. `BaruchOhneNummer1`; außerhalb des Bereichs wird geklemmt.
pub fn unnumbered_style(level: u8) -> String {
    format!("{STYLE_UNNUMBERED_PREFIX}{}", level.clamp(1, MAX_OUTLINE_LEVEL))
}

// --- Typst-Insel -----------------------------------------------------------------------------------------------

/// Kennzeichen der Insel: Inhaltssteuerelement `w:sdt` mit `w:tag w:val="baruch-typst"`; darin steht erst der mit
/// `w:vanish` verborgene Typst-Quelltext, dann das Sichtbare (`shown`).
pub const SDT_TAG_TYPST: &str = "baruch-typst";

// --- Nummerierungen (w:numId in numbering.xml) -----------------------------------------------------------------

/// Ungeordnete Liste (Aufzählungszeichen).
pub const NUM_BULLET: u32 = 1;
/// Geordnete Liste (Dezimalzahlen).
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
