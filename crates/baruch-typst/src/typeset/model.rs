// SPDX-License-Identifier: AGPL-3.0-or-later
//! Data model of the typesetting settings (Round 29, contract §2; interface R29-S1, **frozen** – changes only
//! through integrator/PM).
//!
//! - [`TypesetSettings`]: every field `None` = "from the work template" (no rule). Empty settings
//!   ([`TypesetSettings::is_empty`]) produce no Typst rule; a work without settings typesets byte-identically as
//!   before Round 29.
//! - JSON (RPC, MCP, column `work_typeset.settings`): only set fields, order like the struct
//!   ([`TypesetSettings::to_json`]); unknown fields are refused by [`TypesetSettings::from_json`].
//! - Check ([`TypesetSettings::validate`]) with German messages per field ([`FieldError`]); before saving,
//!   [`TypesetSettings::normalized`] unifies (whitespace, rounding).
//! - `xref_with_page` is not a typesetting rule but the default „mit Seitenzahl“ in the dialog „Verweis“ (R29c).
//! - R31 (interface R31-S1, frozen): `glossary_page`, `index_pages` and `glossary_first_mention` produce no line in
//!   the rule block but control [`crate::index`] (projection and back matter).
//! - R22 (interface R22-S1, frozen): `draft` (draft/watermark, [`Draft`]) produces no line in the rule block; the
//!   rule only goes into the typesetting source of the outputs (typesetting preview, PDF, cue sheet, cover, EPUB
//!   note), not into export and mirror.

use serde::{Deserialize, Serialize};

/// First line of the rule block that [`crate::typeset::rules`] puts behind the work template.
pub const RULES_MARK: &str = "// Baruch: Satz";

/// Limits of the page width (mm) for an own format.
pub const PAGE_WIDTH_MM: (f64, f64) = (80.0, 420.0);
/// Limits of the page height (mm) for an own format.
pub const PAGE_HEIGHT_MM: (f64, f64) = (100.0, 600.0);
/// Limits of every margin (mm).
pub const MARGIN_MM: (f64, f64) = (5.0, 80.0);
/// Limits of the binding margin (mm); it is added to the inner margin.
pub const BINDING_MM: (f64, f64) = (0.0, 30.0);
/// Smallest text width (mm), checked only with page format and margins.
pub const MIN_TEXT_WIDTH_MM: f64 = 40.0;
/// Smallest text height (mm), checked only with page format and margins.
pub const MIN_TEXT_HEIGHT_MM: f64 = 60.0;
/// Longest family name of the base font (characters).
pub const MAX_FONT_CHARS: usize = 200;
/// Limits of the font size (pt).
pub const FONT_SIZE_PT: (f64, f64) = (6.0, 24.0);
/// Limits of the line spacing (Typst `par.leading`, em).
pub const LEADING_EM: (f64, f64) = (0.2, 2.0);
/// Limits of paragraph indent or paragraph spacing (em).
pub const PARAGRAPH_EM: (f64, f64) = (0.1, 4.0);
/// R31: limits of the number of columns of the index pages.
pub const INDEX_COLUMNS: (u8, u8) = (1, 3);
/// R31: at most this many indexes on the index pages (3 built-in + 13 own).
pub const MAX_INDEX_PAGE_REGISTERS: usize = 16;
/// R22: default text of the watermark.
pub const DEFAULT_DRAFT_TEXT: &str = "ENTWURF";
/// R22: longest text of the watermark (characters, trimmed).
pub const MAX_DRAFT_TEXT_CHARS: usize = 40;
/// R22: longest note on the watermark (characters).
pub const MAX_DRAFT_NOTE_CHARS: usize = 120;
/// R22: limits of the opacity of the watermark (percent).
pub const DRAFT_OPACITY_PERCENT: (u8, u8) = (5, 50);

/// Field ids and labels in struct order.
pub const FIELDS: [(&str, &str); 20] = [
    ("chapter_start", "Kapitelanfang"),
    ("part_page", "Teil-Seite"),
    ("page_format", "Seitenformat"),
    ("margins", "Ränder"),
    ("font", "Grundschrift"),
    ("font_size_pt", "Schriftgröße"),
    ("leading_em", "Zeilenabstand"),
    ("paragraph", "Absatz"),
    ("justify", "Blocksatz"),
    ("hyphenate", "Silbentrennung"),
    ("lang", "Sprache"),
    ("page_numbers", "Seitenzahlen"),
    ("running_head", "Kolumnentitel"),
    ("footnotes", "Fußnoten"),
    ("widows_orphans", "Schusterjungen/Hurenkinder"),
    ("xref_with_page", "Verweis mit Seitenzahl"),
    ("glossary_page", "Glossarseite"),
    ("index_pages", "Registerseiten"),
    ("glossary_first_mention", "Erste Erwähnung im Glossar auszeichnen"),
    ("draft", "Entwurf"),
];

/// Label of a field (for subfields like `margins.inside_mm` that of the field); unknown → "".
pub fn field_label(field: &str) -> &'static str {
    let head = field.split('.').next().unwrap_or(field);
    FIELDS.iter().find(|(id, _)| *id == head).map_or("", |(_, label)| label)
}

/// Typesetting settings of a work (Round 29). Every field `None` = from the work template (no rule).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct TypesetSettings {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chapter_start: Option<ChapterStart>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub part_page: Option<PartPage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_format: Option<PageFormat>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub margins: Option<Margins>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_size_pt: Option<f64>,
    /// Typst `par.leading` in em (leading).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub leading_em: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub paragraph: Option<Paragraph>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub justify: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hyphenate: Option<bool>,
    /// Hyphenation/language: "de", "en", "de-CH".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lang: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_numbers: Option<PageNumbers>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub running_head: Option<RunningHead>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub footnotes: Option<Footnotes>,
    /// Avoid orphans/widows.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub widows_orphans: Option<bool>,
    /// R29c: default „mit Seitenzahl“ in the dialog „Verweis“; no Typst rule.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub xref_with_page: Option<bool>,
    /// R31: glossary page in the back matter (None = none).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub glossary_page: Option<GlossaryPosition>,
    /// R31: index pages in the back matter (None = none).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub index_pages: Option<IndexPages>,
    /// R31: mark up the first mention of a glossary term per chapter.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub glossary_first_mention: Option<bool>,
    /// R22: draft (watermark in the outputs); no line in the rule block.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub draft: Option<Draft>,
}

/// Simple enumeration with `ALL`, `id` (= serde value), `from_id` and German `label`.
macro_rules! simple_enum {
    ($(#[$meta:meta])* $name:ident { $($(#[$vmeta:meta])* $variant:ident = $id:literal, $label:literal;)+ }) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
        #[serde(rename_all = "snake_case")]
        pub enum $name {
            $($(#[$vmeta])* $variant,)+
        }

        impl $name {
            /// All values in display order.
            pub const ALL: &'static [$name] = &[$($name::$variant,)+];

            /// Identifier (= serde value).
            pub fn id(self) -> &'static str {
                match self { $($name::$variant => $id,)+ }
            }

            /// Value for the identifier; unknown → `None`.
            pub fn from_id(id: &str) -> Option<Self> {
                match id { $($id => Some($name::$variant),)+ _ => None }
            }

            /// German label for the user interface.
            pub fn label(self) -> &'static str {
                match self { $($name::$variant => $label,)+ }
            }
        }
    };
}

simple_enum! {
    /// Where a chapter (heading level 1 without `<baruch-part>`) starts.
    ChapterStart {
        Continuous = "continuous", "fortlaufend";
        NewPage = "new_page", "neue Seite";
        RightPage = "right_page", "neue rechte Seite";
    }
}

simple_enum! {
    /// Side of the part page.
    PartSide {
        Right = "right", "rechte Seite";
        Any = "any", "beliebige Seite";
    }
}

simple_enum! {
    /// Position of the page number.
    PageNumberPosition {
        BottomCenter = "bottom_center", "unten mittig";
        BottomOutside = "bottom_outside", "unten außen";
        TopOutside = "top_outside", "oben außen";
        Hidden = "hidden", "keine";
    }
}

simple_enum! {
    /// Start of the page counting. `FirstChapter`: the first chapter page carries "1", no number before it.
    PageNumberStart {
        FirstPage = "first_page", "ab der ersten Seite";
        FirstChapter = "first_chapter", "ab dem ersten Kapitel";
    }
}

simple_enum! {
    /// Running head (never on chapter-start, part and empty pages).
    RunningHead {
        Off = "off", "aus";
        WorkTitle = "work_title", "Werktitel";
        Chapter = "chapter", "Kapitel";
    }
}

simple_enum! {
    /// Footnote marks: "1" | "*" | "a".
    FootnoteNumbering {
        Arabic = "arabic", "1, 2, 3";
        Symbols = "symbols", "*, †, ‡";
        Letters = "letters", "a, b, c";
    }
}

simple_enum! {
    /// Counting of the footnotes.
    FootnoteRestart {
        Continuous = "continuous", "fortlaufend";
        PerChapter = "per_chapter", "je Kapitel neu";
    }
}

simple_enum! {
    /// R31: position of the glossary page in the back matter.
    GlossaryPosition {
        BeforeIndexes = "before_indexes", "vor den Registern";
        AfterIndexes = "after_indexes", "nach den Registern";
    }
}

simple_enum! {
    /// Position of the watermark.
    DraftPlacement {
        Diagonal = "diagonal", "diagonal über die Seite";
        Header = "header", "Kopfzeile";
        Footer = "footer", "Fußzeile";
    }
}

/// R22: draft – watermark in the outputs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Draft {
    pub text: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub note: String,
    #[serde(default)]
    pub date: bool,
    pub opacity_percent: u8,
    pub placement: DraftPlacement,
    #[serde(default)]
    pub cover: bool,
}

impl Default for Draft {
    fn default() -> Self {
        Draft {
            text: DEFAULT_DRAFT_TEXT.to_owned(),
            note: String::new(),
            date: false,
            opacity_percent: 12,
            placement: DraftPlacement::Diagonal,
            cover: false,
        }
    }
}

/// R31: index pages in the back matter – which indexes (identifiers, order = typesetting order) in how many columns.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IndexPages {
    pub registers: Vec<String>,
    pub columns: u8,
}

/// Part page: side and whether its back stays empty.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PartPage {
    pub side: PartSide,
    pub blank_back: bool,
}

/// Page format. `Custom` with width and height in mm.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum PageFormat {
    #[serde(rename = "a5")]
    A5,
    #[serde(rename = "120x190")]
    Mm120x190,
    #[serde(rename = "135x215")]
    Mm135x215,
    #[serde(rename = "a4")]
    A4,
    #[serde(rename = "custom")]
    Custom { width_mm: f64, height_mm: f64 },
}

impl PageFormat {
    /// The fixed formats in display order (the own format needs dimensions and is not in it).
    pub const ALL: &'static [PageFormat] =
        &[PageFormat::A5, PageFormat::Mm120x190, PageFormat::Mm135x215, PageFormat::A4];

    /// Identifier (= serde value of `kind`).
    pub fn id(self) -> &'static str {
        match self {
            PageFormat::A5 => "a5",
            PageFormat::Mm120x190 => "120x190",
            PageFormat::Mm135x215 => "135x215",
            PageFormat::A4 => "a4",
            PageFormat::Custom { .. } => "custom",
        }
    }

    /// Fixed format for the identifier; `"custom"` (needs dimensions) and unknown → `None`.
    pub fn from_id(id: &str) -> Option<Self> {
        PageFormat::ALL.iter().copied().find(|f| f.id() == id)
    }

    /// German label.
    pub fn label(self) -> &'static str {
        match self {
            PageFormat::A5 => "A5 (148 × 210 mm)",
            PageFormat::Mm120x190 => "120 × 190 mm",
            PageFormat::Mm135x215 => "135 × 215 mm",
            PageFormat::A4 => "A4 (210 × 297 mm)",
            PageFormat::Custom { .. } => "eigenes Format",
        }
    }

    /// (width, height) in mm.
    pub fn size_mm(&self) -> (f64, f64) {
        match *self {
            PageFormat::A5 => (148.0, 210.0),
            PageFormat::Mm120x190 => (120.0, 190.0),
            PageFormat::Mm135x215 => (135.0, 215.0),
            PageFormat::A4 => (210.0, 297.0),
            PageFormat::Custom { width_mm, height_mm } => (width_mm, height_mm),
        }
    }
}

/// Margins in mm; `binding_mm` (binding margin) is added to the inner margin. Even/odd pages mirrored.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Margins {
    pub inside_mm: f64,
    pub outside_mm: f64,
    pub top_mm: f64,
    pub bottom_mm: f64,
    pub binding_mm: f64,
}

/// Paragraph: first-line indent or spacing between paragraphs (em).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Paragraph {
    Indent { em: f64 },
    Spacing { em: f64 },
}

impl Paragraph {
    /// Both kinds with their default value (indent 1 em, spacing 0.5 em).
    pub const ALL: &'static [Paragraph] = &[Paragraph::Indent { em: 1.0 }, Paragraph::Spacing { em: 0.5 }];

    /// Identifier (= serde value of `kind`).
    pub fn id(self) -> &'static str {
        match self {
            Paragraph::Indent { .. } => "indent",
            Paragraph::Spacing { .. } => "spacing",
        }
    }

    /// Kind for the identifier with the default value from [`Paragraph::ALL`]; unknown → `None`.
    pub fn from_id(id: &str) -> Option<Self> {
        Paragraph::ALL.iter().copied().find(|p| p.id() == id)
    }

    /// German label.
    pub fn label(self) -> &'static str {
        match self {
            Paragraph::Indent { .. } => "Einzug",
            Paragraph::Spacing { .. } => "Abstand",
        }
    }

    /// Measure in em.
    pub fn em(self) -> f64 {
        match self {
            Paragraph::Indent { em } | Paragraph::Spacing { em } => em,
        }
    }
}

/// Page numbers: position and start of the counting.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PageNumbers {
    pub position: PageNumberPosition,
    pub start: PageNumberStart,
}

/// Footnotes: marks and counting.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Footnotes {
    pub numbering: FootnoteNumbering,
    pub restart: FootnoteRestart,
}

/// Refused value: `field` = field id or subfield (`margins.inside_mm`), `message` German with label and limit (e.g.
/// „Rand innen: 5 bis 80 mm“).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FieldError {
    pub field: &'static str,
    pub message: String,
}

/// Number in German notation („0,2“, „80“).
fn de(x: f64) -> String {
    format!("{x}").replace('.', ",")
}

fn in_range(x: f64, (lo, hi): (f64, f64)) -> bool {
    x.is_finite() && x >= lo && x <= hi
}

/// Rounds to `step` (0.1 or 0.01). Divides by the reciprocal instead of multiplying with `step`, so that no
/// floating-point remainder stays (5.3 → 5.3, not 5.300000000000001; review R29).
fn round_to(x: f64, step: f64) -> f64 {
    if x.is_finite() { (x / step).round() / (1.0 / step).round() } else { x }
}

/// `^[a-z]{2,3}(-[A-Z]{2})?$`
fn valid_lang(lang: &str) -> bool {
    let (base, region) = match lang.split_once('-') {
        Some((base, region)) => (base, Some(region)),
        None => (lang, None),
    };
    (2..=3).contains(&base.len())
        && base.bytes().all(|b| b.is_ascii_lowercase())
        && region.is_none_or(|r| r.len() == 2 && r.bytes().all(|b| b.is_ascii_uppercase()))
}

impl TypesetSettings {
    /// All fields `None`.
    pub fn is_empty(&self) -> bool {
        *self == TypesetSettings::default()
    }

    /// Checks all values against the limits (§2). All messages at once, order like [`FIELDS`].
    pub fn validate(&self) -> Result<(), Vec<FieldError>> {
        let mut errors = Vec::new();
        let range =
            |errors: &mut Vec<FieldError>, field: &'static str, x: f64, bounds: (f64, f64), what: &str, unit: &str| {
                if !in_range(x, bounds) {
                    errors.push(FieldError {
                        field,
                        message: format!("{what}: {} bis {} {unit}", de(bounds.0), de(bounds.1)),
                    });
                }
            };
        if let Some(PageFormat::Custom { width_mm, height_mm }) = self.page_format {
            range(&mut errors, "page_format.width_mm", width_mm, PAGE_WIDTH_MM, "Seitenbreite", "mm");
            range(&mut errors, "page_format.height_mm", height_mm, PAGE_HEIGHT_MM, "Seitenhöhe", "mm");
        }
        if let Some(m) = &self.margins {
            range(&mut errors, "margins.inside_mm", m.inside_mm, MARGIN_MM, "Rand innen", "mm");
            range(&mut errors, "margins.outside_mm", m.outside_mm, MARGIN_MM, "Rand außen", "mm");
            range(&mut errors, "margins.top_mm", m.top_mm, MARGIN_MM, "Rand oben", "mm");
            range(&mut errors, "margins.bottom_mm", m.bottom_mm, MARGIN_MM, "Rand unten", "mm");
            range(&mut errors, "margins.binding_mm", m.binding_mm, BINDING_MM, "Bundsteg", "mm");
        }
        if let Some(font) = &self.font {
            let trimmed = font.trim();
            if trimmed.is_empty() {
                errors.push(FieldError { field: "font", message: "Grundschrift: einen Familiennamen angeben".into() });
            } else if trimmed.chars().count() > MAX_FONT_CHARS
                || trimmed.chars().any(|c| c.is_control() || c == '"' || c == '\\')
            {
                errors.push(FieldError {
                    field: "font",
                    message: format!(
                        "Grundschrift: ein Familienname mit höchstens {MAX_FONT_CHARS} Zeichen, ohne Zeilenumbruch, \
                         \" und \\"
                    ),
                });
            }
        }
        if let Some(size) = self.font_size_pt {
            range(&mut errors, "font_size_pt", size, FONT_SIZE_PT, "Schriftgröße", "pt");
        }
        if let Some(leading) = self.leading_em {
            range(&mut errors, "leading_em", leading, LEADING_EM, "Zeilenabstand", "em");
        }
        if let Some(p) = self.paragraph {
            let what = match p {
                Paragraph::Indent { .. } => "Absatzeinzug",
                Paragraph::Spacing { .. } => "Absatzabstand",
            };
            range(&mut errors, "paragraph.em", p.em(), PARAGRAPH_EM, what, "em");
        }
        if let Some(lang) = &self.lang
            && !valid_lang(lang.trim())
        {
            errors.push(FieldError {
                field: "lang", message: "Sprache: Kürzel wie „de“, „en“ oder „de-CH“".into()
            });
        }
        if let Some(pages) = &self.index_pages {
            if pages.registers.is_empty() {
                errors.push(FieldError {
                    field: "index_pages.registers",
                    message: "Registerseiten: mindestens ein Register wählen".into(),
                });
            } else if pages.registers.len() > MAX_INDEX_PAGE_REGISTERS {
                errors.push(FieldError {
                    field: "index_pages.registers",
                    message: format!("Registerseiten: höchstens {MAX_INDEX_PAGE_REGISTERS} Register"),
                });
            } else if let Some(bad) = pages.registers.iter().find(|r| !crate::index::valid_register_slug(r)) {
                errors.push(FieldError {
                    field: "index_pages.registers",
                    message: format!(
                        "Registerseiten: „{bad}“ ist keine gültige Register-Kennung (Kleinbuchstaben, Ziffern, \
                         Bindestrich, höchstens 32 Zeichen)"
                    ),
                });
            } else if let Some(double) =
                pages.registers.iter().enumerate().find(|(i, r)| pages.registers[..*i].contains(r)).map(|(_, r)| r)
            {
                errors.push(FieldError {
                    field: "index_pages.registers",
                    message: format!("Registerseiten: Register „{double}“ ist doppelt gewählt"),
                });
            }
            if !(INDEX_COLUMNS.0..=INDEX_COLUMNS.1).contains(&pages.columns) {
                errors.push(FieldError {
                    field: "index_pages.columns",
                    message: format!("Registerseiten: {} bis {} Spalten", INDEX_COLUMNS.0, INDEX_COLUMNS.1),
                });
            }
        }
        if let Some(draft) = &self.draft {
            let text = draft.text.trim();
            if text.is_empty() || text.chars().count() > MAX_DRAFT_TEXT_CHARS || text.chars().any(char::is_control) {
                errors.push(FieldError {
                    field: "draft.text",
                    message: format!(
                        "Entwurf: ein Text mit 1 bis {MAX_DRAFT_TEXT_CHARS} Zeichen, ohne Zeilenumbruch und Steuerzeichen"
                    ),
                });
            }
            let note = draft.note.trim();
            if note.chars().count() > MAX_DRAFT_NOTE_CHARS || note.chars().any(char::is_control) {
                errors.push(FieldError {
                    field: "draft.note",
                    message: format!(
                        "Entwurf: Zusatz höchstens {MAX_DRAFT_NOTE_CHARS} Zeichen, ohne Zeilenumbruch und Steuerzeichen"
                    ),
                });
            }
            if !(DRAFT_OPACITY_PERCENT.0..=DRAFT_OPACITY_PERCENT.1).contains(&draft.opacity_percent) {
                errors.push(FieldError {
                    field: "draft.opacity_percent",
                    message: format!(
                        "Entwurf: Deckkraft {} bis {} %",
                        DRAFT_OPACITY_PERCENT.0, DRAFT_OPACITY_PERCENT.1
                    ),
                });
            }
        }
        // Type area only with format and margins, and only if the individual values are finite.
        if let (Some(format), Some(m)) = (&self.page_format, &self.margins) {
            let (w, h) = format.size_mm();
            let width = w - m.inside_mm - m.binding_mm - m.outside_mm;
            let height = h - m.top_mm - m.bottom_mm;
            if width.is_finite() && width < MIN_TEXT_WIDTH_MM {
                errors.push(FieldError {
                    field: "margins",
                    message: format!(
                        "Ränder: Die Textbreite muss mindestens {} mm betragen (jetzt {} mm)",
                        de(MIN_TEXT_WIDTH_MM),
                        de(round_to(width, 0.1))
                    ),
                });
            }
            if height.is_finite() && height < MIN_TEXT_HEIGHT_MM {
                errors.push(FieldError {
                    field: "margins",
                    message: format!(
                        "Ränder: Die Texthöhe muss mindestens {} mm betragen (jetzt {} mm)",
                        de(MIN_TEXT_HEIGHT_MM),
                        de(round_to(height, 0.1))
                    ),
                });
            }
        }
        if errors.is_empty() { Ok(()) } else { Err(errors) }
    }

    /// Unifies: `font`/`lang` trimmed (empty → `None`), mm and pt rounded to 0.1, em to 0.01.
    pub fn normalized(&self) -> TypesetSettings {
        let text = |v: &Option<String>| v.as_deref().map(str::trim).filter(|v| !v.is_empty()).map(str::to_owned);
        let mm = |x: f64| round_to(x, 0.1);
        let em = |x: f64| round_to(x, 0.01);
        TypesetSettings {
            page_format: self.page_format.map(|f| match f {
                PageFormat::Custom { width_mm, height_mm } => {
                    PageFormat::Custom { width_mm: mm(width_mm), height_mm: mm(height_mm) }
                }
                fixed => fixed,
            }),
            margins: self.margins.as_ref().map(|m| Margins {
                inside_mm: mm(m.inside_mm),
                outside_mm: mm(m.outside_mm),
                top_mm: mm(m.top_mm),
                bottom_mm: mm(m.bottom_mm),
                binding_mm: mm(m.binding_mm),
            }),
            font: text(&self.font),
            font_size_pt: self.font_size_pt.map(mm),
            leading_em: self.leading_em.map(em),
            paragraph: self.paragraph.map(|p| match p {
                Paragraph::Indent { em: x } => Paragraph::Indent { em: em(x) },
                Paragraph::Spacing { em: x } => Paragraph::Spacing { em: em(x) },
            }),
            lang: text(&self.lang),
            index_pages: self.index_pages.as_ref().map(|p| IndexPages {
                registers: p.registers.iter().map(|r| r.trim().to_owned()).collect(),
                columns: p.columns,
            }),
            // R22: text and addition trimmed (an empty text stays empty and is noticed in `validate`).
            draft: self.draft.as_ref().map(|d| Draft {
                text: d.text.trim().to_owned(),
                note: d.note.trim().to_owned(),
                ..d.clone()
            }),
            ..self.clone()
        }
    }

    /// Field ids whose value differs from `other`, in [`FIELDS`] order.
    pub fn changed_fields(&self, other: &TypesetSettings) -> Vec<&'static str> {
        let differs = [
            self.chapter_start != other.chapter_start,
            self.part_page != other.part_page,
            self.page_format != other.page_format,
            self.margins != other.margins,
            self.font != other.font,
            self.font_size_pt != other.font_size_pt,
            self.leading_em != other.leading_em,
            self.paragraph != other.paragraph,
            self.justify != other.justify,
            self.hyphenate != other.hyphenate,
            self.lang != other.lang,
            self.page_numbers != other.page_numbers,
            self.running_head != other.running_head,
            self.footnotes != other.footnotes,
            self.widows_orphans != other.widows_orphans,
            self.xref_with_page != other.xref_with_page,
            self.glossary_page != other.glossary_page,
            self.index_pages != other.index_pages,
            self.glossary_first_mention != other.glossary_first_mention,
            self.draft != other.draft,
        ];
        FIELDS.iter().zip(differs).filter(|(_, d)| *d).map(|((id, _), _)| *id).collect()
    }

    /// Canonical JSON: only set fields, order like the struct.
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "{}".to_owned())
    }

    /// Reads JSON; unknown fields and wrong types → error message of serde.
    pub fn from_json(json: &str) -> Result<TypesetSettings, String> {
        serde_json::from_str(json).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXAMPLE: &str = r#"{"chapter_start":"right_page","part_page":{"side":"right","blank_back":true},"page_format":{"kind":"a5"},"margins":{"inside_mm":20.0,"outside_mm":15.0,"top_mm":18.0,"bottom_mm":22.0,"binding_mm":5.0},"font":"Libertinus Serif","font_size_pt":10.5,"leading_em":0.65,"paragraph":{"kind":"indent","em":1.0},"justify":true,"hyphenate":true,"lang":"de","page_numbers":{"position":"bottom_outside","start":"first_chapter"},"running_head":"chapter","footnotes":{"numbering":"arabic","restart":"per_chapter"},"widows_orphans":true,"xref_with_page":true}"#;

    #[test]
    fn contract_example_round_trips_byte_for_byte() {
        let s = TypesetSettings::from_json(EXAMPLE).expect("Beispiel aus dem Vertrag (§2) muss lesbar sein");
        // Canonical JSON = order of the struct; the contract example is written this way.
        assert_eq!(s.to_json(), EXAMPLE, "to_json gibt das Vertragsbeispiel unverändert zurück");
        assert_eq!(s.validate(), Ok(()), "das Vertragsbeispiel liegt in allen Grenzen");
        assert_eq!(s.changed_fields(&TypesetSettings::default()).len(), 16, "alle 16 Felder sind gesetzt");
    }

    #[test]
    fn normalized_values_carry_no_floating_point_residue() {
        // Every mm value in steps of 0.1 and every em value in steps of 0.01 must have exactly the shortest decimal
        // form after rounding – otherwise e.g. „5,300000000000001“ would stand in the form, in the JSON and in the
        // exchange XML.
        for tenths in 50..=800 {
            let x = f64::from(tenths) / 10.0;
            assert_eq!(round_to(x, 0.1).to_string(), x.to_string(), "{x} mm bleibt nach dem Runden {x}");
        }
        for hundredths in 1..=400 {
            let x = f64::from(hundredths) / 100.0;
            assert_eq!(round_to(x, 0.01).to_string(), x.to_string(), "{x} em bleibt nach dem Runden {x}");
        }
        let s = TypesetSettings {
            margins: Some(Margins { inside_mm: 20.3, outside_mm: 5.3, top_mm: 18.0, bottom_mm: 22.0, binding_mm: 0.0 }),
            leading_em: Some(0.7),
            ..TypesetSettings::default()
        };
        let json = s.normalized().to_json();
        assert!(
            json.contains("\"outside_mm\":5.3,") && json.contains("\"leading_em\":0.7"),
            "Befund aus dem Gutachten: {json}"
        );
        assert_eq!(round_to(5.34, 0.1), 5.3, "gerundet wird weiterhin auf den Schritt");
    }

    #[test]
    fn empty_settings_serialize_to_empty_object_and_unknown_fields_are_refused() {
        assert_eq!(TypesetSettings::default().to_json(), "{}", "nur gesetzte Felder erscheinen");
        assert!(TypesetSettings::default().is_empty(), "Vorgabe = alles aus der Werkvorlage");
        assert!(TypesetSettings::from_json(r#"{"farbe":"rot"}"#).is_err(), "deny_unknown_fields");
        assert!(
            TypesetSettings::from_json(
                r#"{"margins":{"inside_mm":20,"outside_mm":15,"top_mm":18,"bottom_mm":22,"binding_mm":0,"x":1}}"#
            )
            .is_err(),
            "auch Unterstrukturen lehnen unbekannte Felder ab"
        );
    }

    #[test]
    fn validate_reports_every_violation_with_label_and_bound() {
        let s = TypesetSettings {
            margins: Some(Margins {
                inside_mm: 2.0,
                outside_mm: 15.0,
                top_mm: 18.0,
                bottom_mm: 22.0,
                binding_mm: 40.0,
            }),
            font_size_pt: Some(30.0),
            lang: Some("Deutsch".into()),
            ..Default::default()
        };
        let errors = s.validate().expect_err("drei Felder liegen außerhalb der Grenzen");
        let fields: Vec<&str> = errors.iter().map(|e| e.field).collect();
        assert_eq!(fields, ["margins.inside_mm", "margins.binding_mm", "font_size_pt", "lang"], "alle Meldungen");
        assert_eq!(errors[0].message, "Rand innen: 5 bis 80 mm", "Meldung nennt Bezeichnung und Grenze");
        assert_eq!(field_label("margins.inside_mm"), "Ränder", "Unterfeld → Bezeichnung des Felds");
    }

    #[test]
    fn normalized_trims_and_rounds() {
        let s = TypesetSettings {
            font: Some("  ".into()),
            lang: Some(" de-CH ".into()),
            font_size_pt: Some(10.54),
            leading_em: Some(0.654),
            ..Default::default()
        }
        .normalized();
        assert_eq!(s.font, None, "leere Schrift → aus der Werkvorlage");
        assert_eq!(s.lang.as_deref(), Some("de-CH"), "getrimmt");
        assert!((s.font_size_pt.unwrap() - 10.5).abs() < 1e-9, "pt auf 0,1 gerundet");
        assert!((s.leading_em.unwrap() - 0.65).abs() < 1e-9, "em auf 0,01 gerundet");
    }

    #[test]
    fn draft_is_trimmed_and_checked_against_its_bounds() {
        let padded = TypesetSettings {
            draft: Some(Draft { text: "  Probe  ".into(), note: " Stand ".into(), ..Draft::default() }),
            ..Default::default()
        }
        .normalized();
        let d = padded.draft.as_ref().expect("Entwurf bleibt beim Vereinheitlichen erhalten");
        assert_eq!((d.text.as_str(), d.note.as_str()), ("Probe", "Stand"), "Text und Zusatz werden getrimmt");
        assert_eq!(padded.validate(), Ok(()), "getrimmter Text in den Grenzen ist gültig");

        let bad = |draft: Draft| {
            TypesetSettings { draft: Some(draft), ..Default::default() }
                .normalized()
                .validate()
                .expect_err("Wert außerhalb der Grenzen")
                .into_iter()
                .map(|e| e.field)
                .collect::<Vec<_>>()
        };
        assert_eq!(bad(Draft { text: "   ".into(), ..Draft::default() }), ["draft.text"], "leerer Text");
        assert_eq!(bad(Draft { text: "x".repeat(41), ..Draft::default() }), ["draft.text"], "41 Zeichen");
        assert_eq!(bad(Draft { text: "ENT\nWURF".into(), ..Draft::default() }), ["draft.text"], "Zeilenumbruch");
        assert_eq!(bad(Draft { note: "n".repeat(121), ..Draft::default() }), ["draft.note"], "Zusatz 121 Zeichen");
        assert_eq!(bad(Draft { opacity_percent: 4, ..Draft::default() }), ["draft.opacity_percent"], "Deckkraft 4");
        assert_eq!(bad(Draft { opacity_percent: 51, ..Draft::default() }), ["draft.opacity_percent"], "Deckkraft 51");
        let ok = TypesetSettings {
            draft: Some(Draft {
                text: "Ä".repeat(40), note: "n".repeat(120), opacity_percent: 50, ..Draft::default()
            }),
            ..Default::default()
        };
        assert_eq!(ok.validate(), Ok(()), "40 Zeichen (Unicode zählt als ein Zeichen), 120 Zeichen Zusatz, 50 %");
        assert_eq!(field_label("draft.text"), "Entwurf", "Unterfeld → Bezeichnung des Felds");
    }

    #[test]
    fn enum_ids_match_serde_values() {
        for c in ChapterStart::ALL {
            assert_eq!(serde_json::to_string(c).unwrap(), format!("\"{}\"", c.id()), "id = serde-Wert");
            assert_eq!(ChapterStart::from_id(c.id()), Some(*c), "from_id kehrt id um");
        }
        for f in PageFormat::ALL {
            assert_eq!(PageFormat::from_id(f.id()), Some(*f), "festes Format über id wiederfindbar");
        }
        assert_eq!(PageFormat::from_id("custom"), None, "eigenes Format braucht Maße");
    }
}
