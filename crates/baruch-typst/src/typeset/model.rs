// SPDX-License-Identifier: AGPL-3.0-or-later
//! Datenmodell der Satz-Einstellungen (Runde 29, Vertrag §2; Schnittstelle R29-S1, **eingefroren** – Änderungen nur
//! über Integrator/PM).
//!
//! - [`TypesetSettings`]: jedes Feld `None` = „aus der Werkvorlage“ (keine Regel). Leere Einstellungen
//!   ([`TypesetSettings::is_empty`]) erzeugen keine Typst-Regel; ein Werk ohne Einstellungen setzt byte-gleich wie vor
//!   Runde 29.
//! - JSON (RPC, MCP, Spalte `work_typeset.settings`): nur gesetzte Felder, Reihenfolge wie die Struktur
//!   ([`TypesetSettings::to_json`]); unbekannte Felder lehnt [`TypesetSettings::from_json`] ab.
//! - Prüfung ([`TypesetSettings::validate`]) mit deutschen Meldungen je Feld ([`FieldError`]); vor dem Speichern
//!   vereinheitlicht [`TypesetSettings::normalized`] (Leerraum, Rundung).
//! - `xref_with_page` ist keine Satz-Regel, sondern die Vorgabe „mit Seitenzahl“ im Dialog „Verweis“ (R29c).
//! - R31 (Schnittstelle R31-S1, eingefroren): `glossary_page`, `index_pages` und `glossary_first_mention` erzeugen keine
//!   Zeile im Regelblock, sondern steuern [`crate::index`] (Projektion und Buchende).
//! - R22 (Schnittstelle R22-S1, eingefroren): `draft` (Entwurf/Wasserzeichen, [`Draft`]) erzeugt keine Zeile im
//!   Regelblock; die Regel kommt nur in die Satzquelle der Ausgaben (Satz-Vorschau, PDF, Stichwortzettel, Umschlag,
//!   EPUB-Hinweis), nicht in Export und Spiegel.

use serde::{Deserialize, Serialize};

/// Erste Zeile des Regelblocks, den [`crate::typeset::rules`] hinter die Werkvorlage setzt.
pub const RULES_MARK: &str = "// Baruch: Satz";

/// Grenzen der Seitenbreite (mm) bei eigenem Format.
pub const PAGE_WIDTH_MM: (f64, f64) = (80.0, 420.0);
/// Grenzen der Seitenhöhe (mm) bei eigenem Format.
pub const PAGE_HEIGHT_MM: (f64, f64) = (100.0, 600.0);
/// Grenzen jedes Rands (mm).
pub const MARGIN_MM: (f64, f64) = (5.0, 80.0);
/// Grenzen des Bundstegs (mm); er kommt zum Innenrand hinzu.
pub const BINDING_MM: (f64, f64) = (0.0, 30.0);
/// Kleinste Textbreite (mm), geprüft nur mit Seitenformat und Rändern.
pub const MIN_TEXT_WIDTH_MM: f64 = 40.0;
/// Kleinste Texthöhe (mm), geprüft nur mit Seitenformat und Rändern.
pub const MIN_TEXT_HEIGHT_MM: f64 = 60.0;
/// Längster Familienname der Grundschrift (Zeichen).
pub const MAX_FONT_CHARS: usize = 200;
/// Grenzen der Schriftgröße (pt).
pub const FONT_SIZE_PT: (f64, f64) = (6.0, 24.0);
/// Grenzen des Zeilenabstands (Typst `par.leading`, em).
pub const LEADING_EM: (f64, f64) = (0.2, 2.0);
/// Grenzen von Absatzeinzug bzw. Absatzabstand (em).
pub const PARAGRAPH_EM: (f64, f64) = (0.1, 4.0);
/// R31: Grenzen der Spaltenzahl der Registerseiten.
pub const INDEX_COLUMNS: (u8, u8) = (1, 3);
/// R31: höchstens so viele Register auf den Registerseiten (3 eingebaute + 13 eigene).
pub const MAX_INDEX_PAGE_REGISTERS: usize = 16;
/// R22: Vorgabetext des Wasserzeichens.
pub const DEFAULT_DRAFT_TEXT: &str = "ENTWURF";
/// R22: längster Text des Wasserzeichens (Zeichen, getrimmt).
pub const MAX_DRAFT_TEXT_CHARS: usize = 40;
/// R22: längste Notiz zum Wasserzeichen (Zeichen).
pub const MAX_DRAFT_NOTE_CHARS: usize = 120;
/// R22: Grenzen der Deckkraft des Wasserzeichens (Prozent).
pub const DRAFT_OPACITY_PERCENT: (u8, u8) = (5, 50);

/// Feld-ids und Bezeichnungen in Strukturreihenfolge.
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

/// Bezeichnung eines Felds (auch für Unterfelder wie `margins.inside_mm` die des Felds); unbekannt → "".
pub fn field_label(field: &str) -> &'static str {
    let head = field.split('.').next().unwrap_or(field);
    FIELDS.iter().find(|(id, _)| *id == head).map_or("", |(_, label)| label)
}

/// Satz-Einstellungen eines Werks (Runde 29). Jedes Feld `None` = aus der Werkvorlage (keine Regel).
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
    /// Typst `par.leading` in em (Durchschuss).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub leading_em: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub paragraph: Option<Paragraph>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub justify: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hyphenate: Option<bool>,
    /// Silbentrennung/Sprache: "de", "en", "de-CH".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lang: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_numbers: Option<PageNumbers>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub running_head: Option<RunningHead>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub footnotes: Option<Footnotes>,
    /// Schusterjungen/Hurenkinder vermeiden.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub widows_orphans: Option<bool>,
    /// R29c: Vorgabe „mit Seitenzahl“ im Dialog „Verweis“; keine Typst-Regel.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub xref_with_page: Option<bool>,
    /// R31: Glossarseite am Buchende (None = keine).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub glossary_page: Option<GlossaryPosition>,
    /// R31: Registerseiten am Buchende (None = keine).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub index_pages: Option<IndexPages>,
    /// R31: erste Erwähnung eines Glossarbegriffs je Kapitel auszeichnen.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub glossary_first_mention: Option<bool>,
    /// R22: Entwurf (Wasserzeichen in den Ausgaben); keine Zeile im Regelblock.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub draft: Option<Draft>,
}

/// Einfache Aufzählung mit `ALL`, `id` (= serde-Wert), `from_id` und deutscher `label`.
macro_rules! simple_enum {
    ($(#[$meta:meta])* $name:ident { $($(#[$vmeta:meta])* $variant:ident = $id:literal, $label:literal;)+ }) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
        #[serde(rename_all = "snake_case")]
        pub enum $name {
            $($(#[$vmeta])* $variant,)+
        }

        impl $name {
            /// Alle Werte in Anzeigereihenfolge.
            pub const ALL: &'static [$name] = &[$($name::$variant,)+];

            /// Kennung (= serde-Wert).
            pub fn id(self) -> &'static str {
                match self { $($name::$variant => $id,)+ }
            }

            /// Wert zur Kennung; unbekannt → `None`.
            pub fn from_id(id: &str) -> Option<Self> {
                match id { $($id => Some($name::$variant),)+ _ => None }
            }

            /// Deutsche Bezeichnung für die Oberfläche.
            pub fn label(self) -> &'static str {
                match self { $($name::$variant => $label,)+ }
            }
        }
    };
}

simple_enum! {
    /// Wo ein Kapitel (Überschrift Ebene 1 ohne `<baruch-part>`) beginnt.
    ChapterStart {
        Continuous = "continuous", "fortlaufend";
        NewPage = "new_page", "neue Seite";
        RightPage = "right_page", "neue rechte Seite";
    }
}

simple_enum! {
    /// Seite der Teil-Seite.
    PartSide {
        Right = "right", "rechte Seite";
        Any = "any", "beliebige Seite";
    }
}

simple_enum! {
    /// Lage der Seitenzahl.
    PageNumberPosition {
        BottomCenter = "bottom_center", "unten mittig";
        BottomOutside = "bottom_outside", "unten außen";
        TopOutside = "top_outside", "oben außen";
        Hidden = "hidden", "keine";
    }
}

simple_enum! {
    /// Beginn der Seitenzählung. `FirstChapter`: die erste Kapitelseite trägt „1“, davor keine Zahl.
    PageNumberStart {
        FirstPage = "first_page", "ab der ersten Seite";
        FirstChapter = "first_chapter", "ab dem ersten Kapitel";
    }
}

simple_enum! {
    /// Kolumnentitel (nie auf Kapitelanfangs-, Teil- und leeren Seiten).
    RunningHead {
        Off = "off", "aus";
        WorkTitle = "work_title", "Werktitel";
        Chapter = "chapter", "Kapitel";
    }
}

simple_enum! {
    /// Fußnotenzeichen: "1" | "*" | "a".
    FootnoteNumbering {
        Arabic = "arabic", "1, 2, 3";
        Symbols = "symbols", "*, †, ‡";
        Letters = "letters", "a, b, c";
    }
}

simple_enum! {
    /// Zählung der Fußnoten.
    FootnoteRestart {
        Continuous = "continuous", "fortlaufend";
        PerChapter = "per_chapter", "je Kapitel neu";
    }
}

simple_enum! {
    /// R31: Lage der Glossarseite am Buchende.
    GlossaryPosition {
        BeforeIndexes = "before_indexes", "vor den Registern";
        AfterIndexes = "after_indexes", "nach den Registern";
    }
}

simple_enum! {
    /// Lage des Wasserzeichens.
    DraftPlacement {
        Diagonal = "diagonal", "diagonal über die Seite";
        Header = "header", "Kopfzeile";
        Footer = "footer", "Fußzeile";
    }
}

/// R22: Entwurf – Wasserzeichen in den Ausgaben.
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

/// R31: Registerseiten am Buchende – welche Register (Kennungen, Reihenfolge = Satzreihenfolge) in wie vielen Spalten.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IndexPages {
    pub registers: Vec<String>,
    pub columns: u8,
}

/// Teil-Seite: Seite und ob ihre Rückseite leer bleibt.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PartPage {
    pub side: PartSide,
    pub blank_back: bool,
}

/// Seitenformat. `Custom` mit Breite und Höhe in mm.
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
    /// Die festen Formate in Anzeigereihenfolge (das eigene Format braucht Maße und steht nicht darin).
    pub const ALL: &'static [PageFormat] =
        &[PageFormat::A5, PageFormat::Mm120x190, PageFormat::Mm135x215, PageFormat::A4];

    /// Kennung (= serde-Wert von `kind`).
    pub fn id(self) -> &'static str {
        match self {
            PageFormat::A5 => "a5",
            PageFormat::Mm120x190 => "120x190",
            PageFormat::Mm135x215 => "135x215",
            PageFormat::A4 => "a4",
            PageFormat::Custom { .. } => "custom",
        }
    }

    /// Festes Format zur Kennung; `"custom"` (braucht Maße) und Unbekanntes → `None`.
    pub fn from_id(id: &str) -> Option<Self> {
        PageFormat::ALL.iter().copied().find(|f| f.id() == id)
    }

    /// Deutsche Bezeichnung.
    pub fn label(self) -> &'static str {
        match self {
            PageFormat::A5 => "A5 (148 × 210 mm)",
            PageFormat::Mm120x190 => "120 × 190 mm",
            PageFormat::Mm135x215 => "135 × 215 mm",
            PageFormat::A4 => "A4 (210 × 297 mm)",
            PageFormat::Custom { .. } => "eigenes Format",
        }
    }

    /// (Breite, Höhe) in mm.
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

/// Ränder in mm; `binding_mm` (Bundsteg) kommt zum Innenrand hinzu. Gerade/ungerade Seiten gespiegelt.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Margins {
    pub inside_mm: f64,
    pub outside_mm: f64,
    pub top_mm: f64,
    pub bottom_mm: f64,
    pub binding_mm: f64,
}

/// Absatz: Erstzeileneinzug oder Abstand zwischen Absätzen (em).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Paragraph {
    Indent { em: f64 },
    Spacing { em: f64 },
}

impl Paragraph {
    /// Beide Arten mit ihrem Vorgabewert (Einzug 1 em, Abstand 0,5 em).
    pub const ALL: &'static [Paragraph] = &[Paragraph::Indent { em: 1.0 }, Paragraph::Spacing { em: 0.5 }];

    /// Kennung (= serde-Wert von `kind`).
    pub fn id(self) -> &'static str {
        match self {
            Paragraph::Indent { .. } => "indent",
            Paragraph::Spacing { .. } => "spacing",
        }
    }

    /// Art zur Kennung mit Vorgabewert aus [`Paragraph::ALL`]; unbekannt → `None`.
    pub fn from_id(id: &str) -> Option<Self> {
        Paragraph::ALL.iter().copied().find(|p| p.id() == id)
    }

    /// Deutsche Bezeichnung.
    pub fn label(self) -> &'static str {
        match self {
            Paragraph::Indent { .. } => "Einzug",
            Paragraph::Spacing { .. } => "Abstand",
        }
    }

    /// Maß in em.
    pub fn em(self) -> f64 {
        match self {
            Paragraph::Indent { em } | Paragraph::Spacing { em } => em,
        }
    }
}

/// Seitenzahlen: Lage und Beginn der Zählung.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PageNumbers {
    pub position: PageNumberPosition,
    pub start: PageNumberStart,
}

/// Fußnoten: Zeichen und Zählung.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Footnotes {
    pub numbering: FootnoteNumbering,
    pub restart: FootnoteRestart,
}

/// Abgelehnter Wert: `field` = Feld-id bzw. Unterfeld (`margins.inside_mm`), `message` Deutsch mit Bezeichnung und
/// Grenze (z. B. „Rand innen: 5 bis 80 mm“).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FieldError {
    pub field: &'static str,
    pub message: String,
}

/// Zahl in deutscher Schreibweise („0,2“, „80“).
fn de(x: f64) -> String {
    format!("{x}").replace('.', ",")
}

fn in_range(x: f64, (lo, hi): (f64, f64)) -> bool {
    x.is_finite() && x >= lo && x <= hi
}

/// Rundet auf `step` (0,1 oder 0,01). Teilt durch den Kehrwert statt mit `step` zu multiplizieren, damit kein
/// Gleitkomma-Rest bleibt (5,3 → 5.3, nicht 5.300000000000001; Gutachten R29).
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
    /// Alle Felder `None`.
    pub fn is_empty(&self) -> bool {
        *self == TypesetSettings::default()
    }

    /// Prüft alle Werte gegen die Grenzen (§2). Alle Meldungen auf einmal, Reihenfolge wie [`FIELDS`].
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
        // Satzspiegel nur mit Format und Rändern, und nur, wenn die Einzelwerte endlich sind.
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

    /// Vereinheitlicht: `font`/`lang` getrimmt (leer → `None`), mm und pt auf 0,1, em auf 0,01 gerundet.
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
            // R22: Text und Zusatz getrimmt (ein leerer Text bleibt leer und fällt in `validate` auf).
            draft: self.draft.as_ref().map(|d| Draft {
                text: d.text.trim().to_owned(),
                note: d.note.trim().to_owned(),
                ..d.clone()
            }),
            ..self.clone()
        }
    }

    /// Feld-ids, deren Wert sich gegenüber `other` unterscheidet, in [`FIELDS`]-Reihenfolge.
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

    /// Kanonisches JSON: nur gesetzte Felder, Reihenfolge wie die Struktur.
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "{}".to_owned())
    }

    /// Liest JSON; unbekannte Felder und falsche Typen → Fehlermeldung von serde.
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
        // Kanonisches JSON = Reihenfolge der Struktur; das Vertragsbeispiel ist so geschrieben.
        assert_eq!(s.to_json(), EXAMPLE, "to_json gibt das Vertragsbeispiel unverändert zurück");
        assert_eq!(s.validate(), Ok(()), "das Vertragsbeispiel liegt in allen Grenzen");
        assert_eq!(s.changed_fields(&TypesetSettings::default()).len(), 16, "alle 16 Felder sind gesetzt");
    }

    #[test]
    fn normalized_values_carry_no_floating_point_residue() {
        // Jeder mm-Wert in 0,1-Schritten und jeder em-Wert in 0,01-Schritten muss nach dem Runden genau die kürzeste
        // Dezimalform haben – sonst stünde z. B. „5,300000000000001“ im Formular, im JSON und im Austausch-XML.
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
