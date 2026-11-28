// SPDX-License-Identifier: AGPL-3.0-or-later
//! EPUB 3 with EPUB 2 fallback from the current revisions of a work (Round 20 "EPUB preview").
//!
//! Pure: no database, no network, no Typst typesetting. The server collects the input ([`EpubInput`]) from
//! `Store::export_map` (the same revisions and the same work template as export and typesetting), [`build`]
//! creates the files ([`EpubBook`]), [`package`] packs them as a ZIP. The types of this file have been frozen
//! since the interface R20-S1 (change only through PM/integrator).
//!
//! Naming rules (frozen, contract §1.4), hrefs relative to [`CONTENT_DIR`]: `nav.xhtml` (id `nav`), `toc.ncx`
//! (id `ncx`), `style/book.css` (id `css`), `text/cover.xhtml` (id `cover`), `images/cover.<ext>` (id
//! `cover-image`), `text/title.xhtml`, `text/imprint.xhtml`, `text/dedication.xhtml`, `text/front.xhtml` (id
//! `front`), `text/k{key}.xhtml` (id `k{key}`), `text/notes.xhtml` (id `notes`), `images/<Pfad>` (id `img-{n}`).
//! Since R31-S1 (frozen): `text/glossary.xhtml` (id `glossary`), `text/index-{slug}.xhtml` (id `index-{slug}`).

pub mod build; // epub
pub mod check; // epub-pruefung
pub mod package; // epub
pub mod readers; // reader
pub mod sample; // epub (freely invented test work)
pub mod template; // epub (pagebreak rules and language of the work template)
pub mod xhtml; // epub

use baruch_typst::typeset::TypesetSettings;
use serde::{Deserialize, Serialize};

pub use build::build;
pub use package::package;

pub const MIMETYPE: &str = "application/epub+zip";
pub const CONTENT_DIR: &str = "OEBPS";
pub const OPF_PATH: &str = "OEBPS/content.opf";
pub const CONTAINER_PATH: &str = "META-INF/container.xml";

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EpubMeta {
    /// `urn:uuid:<work.uuid>` (unique identifier of the package).
    pub identifier: String,
    pub title: String,
    pub subtitle: String,
    pub author: String,
    /// BCP 47, never empty (typesetting setting → `#set text(lang:)` of the template → "de").
    pub language: String,
    /// 13 digits, checked with baruch_typst::cover::isbn13_check; otherwise None.
    pub isbn: Option<String>,
    pub publisher: String,
    pub year: String,
    /// copyright_line of the book details.
    pub rights: String,
    /// Image credit (book details, otherwise caption of the cover).
    pub image_credit: String,
    /// Blurb.
    pub description: String,
    /// dcterms:modified, "YYYY-MM-DDThh:mm:ssZ", from the server out of the state.
    pub modified: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EpubImage {
    /// Path as in `image("…")` of the Typst source.
    pub path: String,
    pub media_type: String,
    pub bytes: Vec<u8>,
    /// Alternative text (empty = none).
    pub alt: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EpubSection {
    pub key: i64,
    /// 0 = part, 1–3 as baruch_typst::Heading.level.
    pub level: u8,
    /// Displayed number from the outline scheme ("2.1"), None = without number.
    pub number: Option<String>,
    pub title: String,
    /// Section text as in the export (with heading line and anchor <kN>).
    pub body: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FootnoteMode {
    #[default]
    Popup,
    Endnotes,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EpubInput {
    pub meta: EpubMeta,
    /// Work template (export before the first section).
    pub template: String,
    pub sections: Vec<EpubSection>,
    pub typeset: TypesetSettings,
    pub images: Vec<EpubImage>,
    pub cover: Option<EpubImage>,
    /// Front matter on (title page, imprint, dedication as files of their own).
    pub front_matter: bool,
    /// baruch_typst::cover::imprint_lines – plain text.
    pub imprint: Vec<String>,
    pub dedication: String,
    pub footnotes: FootnoteMode,
    /// R31: index and glossary (typesetting settings, indexes, accepted glossary terms) from
    /// `Store::back_matter`; inactive ([`baruch_typst::index::BackMatter::is_active`]) → no files for them.
    #[serde(default)]
    pub back_matter: baruch_typst::index::BackMatter,
    /// Round 39b: language of the work. Labels and generated words of the book (navigation, landmarks, guide, cover,
    /// title page, glossary, index, notes, figures) follow it through the book catalog
    /// ([`baruch_core::book_text`]); without `meta.language` it also decides the language tag
    /// ([`template::book_language`]). Default German: every input from before Round 39b stays byte-identical.
    #[serde(default)]
    pub work_language: baruch_core::WorkLanguage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemKind {
    Cover,
    Front,
    Part,
    Chapter,
    Notes,
    Nav,
    Ncx,
    Style,
    Image,
    CoverImage,
    /// R31: `text/glossary.xhtml` (id `glossary`).
    Glossary,
    /// R31: `text/index-{slug}.xhtml` (id `index-{slug}`).
    Index,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EpubItem {
    /// Manifest id (see naming rules §1.4).
    pub id: String,
    /// Relative to OEBPS/.
    pub href: String,
    pub media_type: String,
    pub kind: ItemKind,
    pub properties: Option<String>,
    /// Title for preview/table of contents (files with text).
    pub title: Option<String>,
    /// Key of the section the file starts with.
    pub key: Option<i64>,
    #[serde(skip)]
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TocEntry {
    pub title: String,
    pub href: String,
    pub level: u8,
    pub key: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EpubWarning {
    pub code: String,
    pub message: String,
    pub key: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EpubBook {
    pub meta: EpubMeta,
    /// Manifest in manifest order.
    pub items: Vec<EpubItem>,
    /// Manifest ids in reading order.
    pub spine: Vec<String>,
    pub toc: Vec<TocEntry>,
    /// Content of OEBPS/content.opf.
    pub opf: String,
    pub warnings: Vec<EpubWarning>,
}

impl EpubBook {
    /// Entry for `href` (relative to OEBPS/).
    pub fn item(&self, href: &str) -> Option<&EpubItem> {
        self.items.iter().find(|i| i.href == href)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EpubError {
    #[error("Das Werk hat keinen Abschnitt; ein EPUB braucht mindestens eine Datei mit Text.")]
    NoSections,
    #[error("Ungültige Eingabe: {0}")]
    Invalid(String),
    #[error("ZIP: {0}")]
    Zip(String),
}
