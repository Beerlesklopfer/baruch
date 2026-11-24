// SPDX-License-Identifier: AGPL-3.0-or-later
//! EPUB 3 mit EPUB-2-Rückfall aus den aktuellen Fassungen eines Werks (Runde 20 „EPUB-Vorschau“).
//!
//! Rein: keine Datenbank, kein Netz, kein Typst-Satz. Der Server sammelt die Eingabe ([`EpubInput`]) aus
//! `Store::export_map` (dieselben Fassungen und dieselbe Werkvorlage wie Export und Satz), [`build`] legt die
//! Dateien an ([`EpubBook`]), [`package`] packt sie als ZIP. Die Typen dieser Datei sind seit der Schnittstelle
//! R20-S1 eingefroren (Änderung nur über PM/Integrator).
//!
//! Namensregeln (eingefroren, Vertrag §1.4), hrefs relativ zu [`CONTENT_DIR`]: `nav.xhtml` (id `nav`), `toc.ncx`
//! (id `ncx`), `style/book.css` (id `css`), `text/cover.xhtml` (id `cover`), `images/cover.<ext>` (id
//! `cover-image`), `text/title.xhtml`, `text/imprint.xhtml`, `text/dedication.xhtml`, `text/front.xhtml` (id
//! `front`), `text/k{key}.xhtml` (id `k{key}`), `text/notes.xhtml` (id `notes`), `images/<Pfad>` (id `img-{n}`).
//! Seit R31-S1 (eingefroren): `text/glossary.xhtml` (id `glossary`), `text/index-{slug}.xhtml` (id `index-{slug}`).

pub mod build; // epub
pub mod check; // epub-pruefung
pub mod package; // epub
pub mod readers; // reader
pub mod sample; // epub (frei erfundenes Testwerk)
pub mod template; // epub (pagebreak-Regeln und Sprache der Werkvorlage)
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
    /// `urn:uuid:<work.uuid>` (eindeutige Kennung des Pakets).
    pub identifier: String,
    pub title: String,
    pub subtitle: String,
    pub author: String,
    /// BCP 47, nie leer (Satz-Einstellung → `#set text(lang:)` der Vorlage → "de").
    pub language: String,
    /// 13 Ziffern, geprüft mit baruch_typst::cover::isbn13_check; sonst None.
    pub isbn: Option<String>,
    pub publisher: String,
    pub year: String,
    /// copyright_line der Buchangaben.
    pub rights: String,
    /// Bildnachweis (Buchangaben, sonst Bildunterschrift des Umschlags).
    pub image_credit: String,
    /// Klappentext.
    pub description: String,
    /// dcterms:modified, "JJJJ-MM-TTThh:mm:ssZ", vom Server aus dem Stand.
    pub modified: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EpubImage {
    /// Pfad wie in `image("…")` der Typst-Quelle.
    pub path: String,
    pub media_type: String,
    pub bytes: Vec<u8>,
    /// Alternativtext (leer = keiner).
    pub alt: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EpubSection {
    pub key: i64,
    /// 0 = Teil, 1–3 wie baruch_typst::Heading.level.
    pub level: u8,
    /// Angezeigte Nummer aus dem Gliederungsschema ("2.1"), None = ohne Nummer.
    pub number: Option<String>,
    pub title: String,
    /// Abschnittstext wie im Export (mit Überschriftszeile und Sprungmarke <kN>).
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
    /// Werkvorlage (Export vor dem ersten Abschnitt).
    pub template: String,
    pub sections: Vec<EpubSection>,
    pub typeset: TypesetSettings,
    pub images: Vec<EpubImage>,
    pub cover: Option<EpubImage>,
    /// Titelei an (Titelseite, Impressum, Widmung als eigene Dateien).
    pub front_matter: bool,
    /// baruch_typst::cover::imprint_lines – reiner Text.
    pub imprint: Vec<String>,
    pub dedication: String,
    pub footnotes: FootnoteMode,
    /// R31: Register und Glossar (Satz-Einstellungen, Register, angenommene Glossarbegriffe) aus
    /// `Store::back_matter`; inaktiv ([`baruch_typst::index::BackMatter::is_active`]) → keine Dateien dazu.
    #[serde(default)]
    pub back_matter: baruch_typst::index::BackMatter,
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
    /// Manifest-id (siehe Namensregeln §1.4).
    pub id: String,
    /// Relativ zu OEBPS/.
    pub href: String,
    pub media_type: String,
    pub kind: ItemKind,
    pub properties: Option<String>,
    /// Titel für Vorschau/Inhaltsverzeichnis (Dateien mit Text).
    pub title: Option<String>,
    /// Schlüssel des Abschnitts, mit dem die Datei beginnt.
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
    /// Manifest in Manifest-Reihenfolge.
    pub items: Vec<EpubItem>,
    /// Manifest-ids in Lesereihenfolge.
    pub spine: Vec<String>,
    pub toc: Vec<TocEntry>,
    /// Inhalt von OEBPS/content.opf.
    pub opf: String,
    pub warnings: Vec<EpubWarning>,
}

impl EpubBook {
    /// Eintrag zu `href` (relativ zu OEBPS/).
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
