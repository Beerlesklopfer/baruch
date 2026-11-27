// SPDX-License-Identifier: AGPL-3.0-or-later
//! Block model of a Word file (interface R32-S1, frozen – changes only through the integrator).
//!
//! The model is the pivot between reader ([`crate::read`]), resolution ([`crate::resolve`]), Typst
//! ([`crate::to_typst`], [`crate::from_typst`]), tracked changes ([`crate::diff`]) and writer
//! ([`crate::write`]).
//!
//! Meaning of the fields:
//! - `level` of a heading is 1–6: 1–3 form the outline of the work, 4–6 stay headings in the text.
//! - `mark` is the change of the paragraph mark: with [`ChangeKind::Insert`] the paragraph was joined with the
//!   following one in the original, with [`ChangeKind::Delete`] it is in the final state.
//! - `date` is ISO 8601 in UTC (`2026-11-20T09:30:00Z`) or empty.
//! - `image` is the index in [`DocxDocument::images`]; [`DocxImage::path`] is the path in the work (`bilder/…`).
//! - [`Inline::Typst`]/[`Block::Typst`]: `raw` is the verbatim Typst source text of an island, `shown` what the
//!   reader sees in Word (discarded on reading if `raw` is not empty).
//! - [`Block::Figure`]: `raw` is the verbatim island of the image or empty.
//! - Footnotes are inline; several paragraphs of a Word footnote are joined by the reader with
//!   [`Inline::LineBreak`] (with a warning).

/// A whole document: front matter, blocks in reading order, comments and embedded images.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DocxDocument {
    pub meta: DocxMeta,
    pub blocks: Vec<Block>,
    pub comments: Vec<Comment>,
    pub images: Vec<DocxImage>,
}

/// Front matter and details from `docProps/core.xml`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DocxMeta {
    pub title: String,
    pub subtitle: String,
    pub author: String,
    /// Language as a BCP 47 tag (`de`, `de-DE`) or empty.
    pub lang: String,
    /// Lines of the imprint (writer: style `BaruchImpressum`).
    pub imprint: Vec<String>,
}

/// Embedded image.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DocxImage {
    /// Path in the work (`bilder/…`).
    pub path: String,
    /// File name without folder.
    pub file_name: String,
    /// Media type (`image/png`, `image/jpeg`, …).
    pub media_type: String,
    pub bytes: Vec<u8>,
}

/// Word comment; anchored through [`Inline::CommentStart`] and [`Inline::CommentEnd`] with the same `id`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Comment {
    pub id: u32,
    pub author: String,
    pub initials: String,
    /// ISO 8601 UTC or empty.
    pub date: String,
    pub text: String,
    /// `id` of the comment this one replies to.
    pub reply_to: Option<u32>,
}

/// Kind of a tracked change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChangeKind {
    Insert,
    Delete,
}

/// Tracked change: kind, editor (verbatim name from Word) and time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    pub kind: ChangeKind,
    pub author: String,
    /// ISO 8601 UTC or empty.
    pub date: String,
}

/// Character formats of a text run.
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

/// Content within a paragraph.
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
    /// Inline quote (writer: character style `BaruchZitat`).
    Quote {
        content: Vec<Inline>,
    },
    Link {
        url: String,
        content: Vec<Inline>,
    },
    /// Image in the text; `image` is the index in [`DocxDocument::images`].
    Image {
        image: usize,
        alt: String,
    },
    /// Typst island in the text: `raw` verbatim, `shown` what is visible in Word.
    Typst {
        raw: String,
        shown: Vec<Inline>,
    },
    /// Tracked insertion or deletion.
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

/// Item of a list; `depth` 0 is the top level.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ListItem {
    pub depth: u8,
    pub content: Vec<Inline>,
    pub mark: Option<Change>,
}

/// Block of the document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Block {
    /// Part of the work (writer: style `BaruchTeil`).
    Part {
        content: Vec<Inline>,
        mark: Option<Change>,
    },
    /// Heading of level 1–6; `numbered: false` = without number (style `BaruchOhneNummerN`).
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
    /// Block quote with source (empty = without).
    Quote {
        blocks: Vec<Block>,
        attribution: Vec<Inline>,
    },
    /// List; `start` is the first number of an ordered list.
    List {
        ordered: bool,
        start: u64,
        items: Vec<ListItem>,
    },
    /// Table: rows → cells → blocks; the first `header_rows` rows are header rows.
    Table {
        header_rows: usize,
        rows: Vec<Vec<Vec<Block>>>,
    },
    /// Figure with caption; `raw` is the verbatim Typst island of the image or empty.
    Figure {
        image: usize,
        alt: String,
        caption: Vec<Inline>,
        raw: String,
    },
    /// Typst island as a block: `raw` verbatim, `shown` what is visible in Word.
    Typst {
        raw: String,
        shown: Vec<Block>,
    },
    PageBreak,
}

/// Note of the reader or writer (never discard silently): fixed `code` (e.g. `docx_formataenderung`) and plain
/// text for the human.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DocxWarning {
    pub code: String,
    pub message: String,
}
