// SPDX-License-Identifier: AGPL-3.0-or-later
//! Blockmodell einer Word-Datei (Schnittstelle R32-S1, eingefroren – Änderungen nur über den Integrator).
//!
//! Das Modell ist der Drehpunkt zwischen Leser ([`crate::read`]), Auflösung ([`crate::resolve`]), Typst
//! ([`crate::to_typst`], [`crate::from_typst`]), Änderungsverfolgung ([`crate::diff`]) und Schreiber
//! ([`crate::write`]).
//!
//! Bedeutung der Felder:
//! - `level` einer Überschrift ist 1–6: 1–3 bilden die Gliederung des Werks, 4–6 bleiben Überschriften im Text.
//! - `mark` ist die Änderung der Absatzmarke: bei [`ChangeKind::Insert`] war der Absatz im Original mit dem
//!   folgenden vereint, bei [`ChangeKind::Delete`] ist er es im Endstand.
//! - `date` ist ISO 8601 in UTC (`2026-11-20T09:30:00Z`) oder leer.
//! - `image` ist der Index in [`DocxDocument::images`]; [`DocxImage::path`] ist der Pfad im Werk (`bilder/…`).
//! - [`Inline::Typst`]/[`Block::Typst`]: `raw` ist der wörtliche Typst-Quelltext einer Insel, `shown` das, was der
//!   Leser in Word sieht (beim Lesen verworfen, wenn `raw` nicht leer ist).
//! - [`Block::Figure`]: `raw` ist die wörtliche Insel des Bildes oder leer.
//! - Fußnoten stehen inline; mehrere Absätze einer Word-Fußnote verbindet der Leser mit [`Inline::LineBreak`]
//!   (mit Warnung).

/// Ein ganzes Dokument: Titelei, Blöcke in Lesereihenfolge, Kommentare und eingebettete Bilder.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DocxDocument {
    pub meta: DocxMeta,
    pub blocks: Vec<Block>,
    pub comments: Vec<Comment>,
    pub images: Vec<DocxImage>,
}

/// Titelei und Angaben aus `docProps/core.xml`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DocxMeta {
    pub title: String,
    pub subtitle: String,
    pub author: String,
    /// Sprache als BCP-47-Kürzel (`de`, `de-DE`) oder leer.
    pub lang: String,
    /// Zeilen des Impressums (Schreiber: Vorlage `BaruchImpressum`).
    pub imprint: Vec<String>,
}

/// Eingebettetes Bild.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DocxImage {
    /// Pfad im Werk (`bilder/…`).
    pub path: String,
    /// Dateiname ohne Ordner.
    pub file_name: String,
    /// Medientyp (`image/png`, `image/jpeg`, …).
    pub media_type: String,
    pub bytes: Vec<u8>,
}

/// Word-Kommentar; verankert über [`Inline::CommentStart`] und [`Inline::CommentEnd`] mit derselben `id`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Comment {
    pub id: u32,
    pub author: String,
    pub initials: String,
    /// ISO 8601 UTC oder leer.
    pub date: String,
    pub text: String,
    /// `id` des Kommentars, auf den dieser antwortet.
    pub reply_to: Option<u32>,
}

/// Art einer verfolgten Änderung.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChangeKind {
    Insert,
    Delete,
}

/// Verfolgte Änderung: Art, Bearbeiter (wörtlicher Name aus Word) und Zeitpunkt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    pub kind: ChangeKind,
    pub author: String,
    /// ISO 8601 UTC oder leer.
    pub date: String,
}

/// Zeichenformate eines Textlaufs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct Marks {
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strike: bool,
    pub superscript: bool,
    pub subscript: bool,
    pub small_caps: bool,
}

/// Inhalt innerhalb eines Absatzes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Inline {
    Text {
        text: String,
        marks: Marks,
    },
    LineBreak,
    Footnote {
        content: Vec<Inline>,
    },
    /// Zitat im Fließtext (Schreiber: Zeichenvorlage `BaruchZitat`).
    Quote {
        content: Vec<Inline>,
    },
    Link {
        url: String,
        content: Vec<Inline>,
    },
    /// Bild im Text; `image` ist der Index in [`DocxDocument::images`].
    Image {
        image: usize,
        alt: String,
    },
    /// Typst-Insel im Text: `raw` wörtlich, `shown` das in Word Sichtbare.
    Typst {
        raw: String,
        shown: Vec<Inline>,
    },
    /// Verfolgte Einfügung oder Löschung.
    Tracked {
        change: Change,
        content: Vec<Inline>,
    },
    CommentStart {
        id: u32,
    },
    CommentEnd {
        id: u32,
    },
}

/// Punkt einer Liste; `depth` 0 ist die oberste Ebene.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ListItem {
    pub depth: u8,
    pub content: Vec<Inline>,
    pub mark: Option<Change>,
}

/// Block des Dokuments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Block {
    /// Teil des Werks (Schreiber: Vorlage `BaruchTeil`).
    Part {
        content: Vec<Inline>,
        mark: Option<Change>,
    },
    /// Überschrift der Ebene 1–6; `numbered: false` = ohne Nummer (Vorlage `BaruchOhneNummerN`).
    Heading {
        level: u8,
        numbered: bool,
        content: Vec<Inline>,
        mark: Option<Change>,
    },
    Paragraph {
        content: Vec<Inline>,
        mark: Option<Change>,
    },
    /// Blockzitat mit Quellenangabe (leer = ohne).
    Quote {
        blocks: Vec<Block>,
        attribution: Vec<Inline>,
    },
    /// Liste; `start` ist die erste Nummer einer geordneten Liste.
    List {
        ordered: bool,
        start: u64,
        items: Vec<ListItem>,
    },
    /// Tabelle: Zeilen → Zellen → Blöcke; die ersten `header_rows` Zeilen sind Kopfzeilen.
    Table {
        header_rows: usize,
        rows: Vec<Vec<Vec<Block>>>,
    },
    /// Abbildung mit Bildunterschrift; `raw` ist die wörtliche Typst-Insel des Bildes oder leer.
    Figure {
        image: usize,
        alt: String,
        caption: Vec<Inline>,
        raw: String,
    },
    /// Typst-Insel als Block: `raw` wörtlich, `shown` das in Word Sichtbare.
    Typst {
        raw: String,
        shown: Vec<Block>,
    },
    PageBreak,
}

/// Hinweis des Lesers oder Schreibers (nie still verwerfen): fester `code` (z. B. `docx_formataenderung`) und
/// Klartext für den Menschen.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DocxWarning {
    pub code: String,
    pub message: String,
}
