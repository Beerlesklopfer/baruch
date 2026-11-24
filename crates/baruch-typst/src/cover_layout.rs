// SPDX-License-Identifier: AGPL-3.0-or-later
//! Umschlag-Anordnung (Vertrag Runde 27, Abschnitte 1.1–1.3 und 2.1): frei gesetzte Blöcke auf Vorderseite,
//! Rücken und Rückseite.
//!
//! - **Blöcke** ([`CoverBlockKind`], 15 Arten): je Werk und Art höchstens einer. Eigenen Text
//!   ([`CoverBlock::text`], höchstens [`MAX_TEXT_CHARS`] Zeichen) haben nur die Freitexte und das Motto
//!   ([`CoverBlockKind::own_text`]); alle anderen nehmen ihren Text aus Werk und Buchangaben.
//! - **Koordinaten** in Millimetern, relativ zur Schnittkante (oben links) der eigenen Fläche, ohne Beschnitt;
//!   negative Werte ragen in den Beschnitt. `(x_mm, y_mm)` ist die linke obere Ecke des Blockrahmens im
//!   ungedrehten Seitenkoordinatensystem, `width_mm` die Breite in Leserichtung (beim Rücken die senkrechte
//!   Ausdehnung auf der Seite). [`CoverGeometry`] rechnet die Flächen im Gesamtumschlag
//!   (Rückseite | Rücken | Vorderseite) aus.
//! - **Grenzen** ([`POS_MM`], [`WIDTH_MM`], [`SIZE_PT`], [`LEADING_EM`], [`MAX_TEXT_CHARS`], [`MAX_FONT_CHARS`])
//!   sind dieselben wie die CHECKs der Migration 0045 (`cover_block`).
//! - Ohne gespeicherte Zeile gilt der Vorgabeblock ([`default_blocks`]); [`effective_blocks`] mischt beides.
//!
//! Die Typen und Signaturen sind nach der Schnittstelle (S1) eingefroren; die Körper von [`default_blocks`] und
//! [`effective_blocks`] gehören der Aufgabe umschlag-satz (R27b). Gesetzt werden die Blöcke in
//! [`crate::cover::frame_parts`] und [`crate::cover::wrap_source`].

use serde::{Deserialize, Serialize};

use crate::cover::TitleArea;

/// Sicherheitsabstand zur Schnittkante in mm (Vorgabe).
pub const SAFETY_MM: f64 = 5.0;
/// Grenzen von `x_mm` und `y_mm` (mm, einschließlich).
pub const POS_MM: (f64, f64) = (-50.0, 1000.0);
/// Grenzen von `width_mm` (mm, einschließlich).
pub const WIDTH_MM: (f64, f64) = (1.0, 1000.0);
/// Grenzen von `size_pt` (pt, einschließlich).
pub const SIZE_PT: (f64, f64) = (4.0, 200.0);
/// Grenzen von `leading_em` (em, einschließlich).
pub const LEADING_EM: (f64, f64) = (0.0, 3.0);
/// Höchstlänge des eigenen Texts in Zeichen.
pub const MAX_TEXT_CHARS: usize = 1000;
/// Höchstlänge eines Schriftnamens in Zeichen.
pub const MAX_FONT_CHARS: usize = 200;

/// Fläche des Umschlags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CoverFace {
    Front,
    Spine,
    Back,
}

impl CoverFace {
    pub const ALL: [CoverFace; 3] = [CoverFace::Front, CoverFace::Spine, CoverFace::Back];

    /// `front`, `spine`, `back` (RPC, Formulare).
    pub fn as_str(self) -> &'static str {
        match self {
            CoverFace::Front => "front",
            CoverFace::Spine => "spine",
            CoverFace::Back => "back",
        }
    }

    pub fn parse(s: &str) -> Option<CoverFace> {
        CoverFace::ALL.into_iter().find(|f| f.as_str() == s.trim())
    }

    /// Deutscher Name: Vorderseite, Rücken, Rückseite.
    pub fn label(self) -> &'static str {
        match self {
            CoverFace::Front => "Vorderseite",
            CoverFace::Spine => "Rücken",
            CoverFace::Back => "Rückseite",
        }
    }
}

/// Art eines Blocks (Datenbank, RPC und serde mit demselben Namen, z. B. `front_title`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CoverBlockKind {
    FrontTitle,
    FrontSubtitle,
    FrontAuthor,
    FrontPublisher,
    FrontFree,
    SpineTitle,
    SpineAuthor,
    SpinePublisher,
    SpineFree,
    BackMotto,
    BackBlurb,
    BackBio,
    BackBarcode,
    BackPublisherPrice,
    BackFree,
}

impl CoverBlockKind {
    pub const ALL: [CoverBlockKind; 15] = [
        CoverBlockKind::FrontTitle,
        CoverBlockKind::FrontSubtitle,
        CoverBlockKind::FrontAuthor,
        CoverBlockKind::FrontPublisher,
        CoverBlockKind::FrontFree,
        CoverBlockKind::SpineTitle,
        CoverBlockKind::SpineAuthor,
        CoverBlockKind::SpinePublisher,
        CoverBlockKind::SpineFree,
        CoverBlockKind::BackMotto,
        CoverBlockKind::BackBlurb,
        CoverBlockKind::BackBio,
        CoverBlockKind::BackBarcode,
        CoverBlockKind::BackPublisherPrice,
        CoverBlockKind::BackFree,
    ];

    /// Name in Datenbank, RPC und Formularen (`front_title` …).
    pub fn as_str(self) -> &'static str {
        match self {
            CoverBlockKind::FrontTitle => "front_title",
            CoverBlockKind::FrontSubtitle => "front_subtitle",
            CoverBlockKind::FrontAuthor => "front_author",
            CoverBlockKind::FrontPublisher => "front_publisher",
            CoverBlockKind::FrontFree => "front_free",
            CoverBlockKind::SpineTitle => "spine_title",
            CoverBlockKind::SpineAuthor => "spine_author",
            CoverBlockKind::SpinePublisher => "spine_publisher",
            CoverBlockKind::SpineFree => "spine_free",
            CoverBlockKind::BackMotto => "back_motto",
            CoverBlockKind::BackBlurb => "back_blurb",
            CoverBlockKind::BackBio => "back_bio",
            CoverBlockKind::BackBarcode => "back_barcode",
            CoverBlockKind::BackPublisherPrice => "back_publisher_price",
            CoverBlockKind::BackFree => "back_free",
        }
    }

    pub fn parse(s: &str) -> Option<CoverBlockKind> {
        CoverBlockKind::ALL.into_iter().find(|k| k.as_str() == s.trim())
    }

    /// Fläche, auf der der Block steht.
    pub fn face(self) -> CoverFace {
        match self {
            CoverBlockKind::FrontTitle
            | CoverBlockKind::FrontSubtitle
            | CoverBlockKind::FrontAuthor
            | CoverBlockKind::FrontPublisher
            | CoverBlockKind::FrontFree => CoverFace::Front,
            CoverBlockKind::SpineTitle
            | CoverBlockKind::SpineAuthor
            | CoverBlockKind::SpinePublisher
            | CoverBlockKind::SpineFree => CoverFace::Spine,
            CoverBlockKind::BackMotto
            | CoverBlockKind::BackBlurb
            | CoverBlockKind::BackBio
            | CoverBlockKind::BackBarcode
            | CoverBlockKind::BackPublisherPrice
            | CoverBlockKind::BackFree => CoverFace::Back,
        }
    }

    /// Deutscher Name ohne Fläche (z. B. „Titel“; mit Fläche: „Titel (Vorderseite)“).
    pub fn label(self) -> &'static str {
        match self {
            CoverBlockKind::FrontTitle | CoverBlockKind::SpineTitle => "Titel",
            CoverBlockKind::FrontSubtitle => "Untertitel",
            CoverBlockKind::FrontAuthor | CoverBlockKind::SpineAuthor => "Autor",
            CoverBlockKind::FrontPublisher | CoverBlockKind::SpinePublisher => "Verlag",
            CoverBlockKind::FrontFree | CoverBlockKind::SpineFree | CoverBlockKind::BackFree => "Freitext",
            CoverBlockKind::BackMotto => "Motto/Bibelwort",
            CoverBlockKind::BackBlurb => "Klappentext",
            CoverBlockKind::BackBio => "Kurzbiografie mit Foto",
            CoverBlockKind::BackBarcode => "ISBN mit Strichcode",
            CoverBlockKind::BackPublisherPrice => "Verlag und Preis",
        }
    }

    /// Hat der Block eigenen Text (`text`)? Nur die drei Freitexte und das Motto.
    pub fn own_text(self) -> bool {
        matches!(
            self,
            CoverBlockKind::FrontFree
                | CoverBlockKind::SpineFree
                | CoverBlockKind::BackFree
                | CoverBlockKind::BackMotto
        )
    }
}

/// Wörter für die kleinen Aufzählungen (Datenbank, RPC, Formulare).
macro_rules! words {
    ($name:ident { $($variant:ident => $word:literal),+ $(,)? }) => {
        impl $name {
            /// Name in Datenbank, RPC und Formularen.
            pub fn as_str(self) -> &'static str {
                match self { $($name::$variant => $word),+ }
            }

            pub fn parse(s: &str) -> Option<$name> {
                match s.trim() { $($word => Some($name::$variant),)+ _ => None }
            }
        }
    };
}

/// Ausrichtung des Texts im Blockrahmen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CoverAlign {
    Left,
    Center,
    Right,
    Justify,
}
words!(CoverAlign { Left => "left", Center => "center", Right => "right", Justify => "justify" });

/// Schriftschnitt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FontStyle {
    Normal,
    Bold,
    Italic,
}
words!(FontStyle { Normal => "normal", Bold => "bold", Italic => "italic" });

/// Fläche hinter dem Block: keine, halbtransparenter Schatten oder Verlauf (wie bisher hinter dem Titel).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Backdrop {
    None,
    Shadow,
    Gradient,
}
words!(Backdrop { None => "none", Shadow => "shadow", Gradient => "gradient" });

/// Laufrichtung des Texts auf dem Rücken: `down` = von oben nach unten lesbar (`rotate(90deg)`), `up` = von
/// unten nach oben (`rotate(-90deg)`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpineDirection {
    Down,
    Up,
}
words!(SpineDirection { Down => "down", Up => "up" });

/// Ein Block der Anordnung (Werte und Grenzen: Moduldoku, Vertrag 1.3).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CoverBlock {
    pub kind: CoverBlockKind,
    pub x_mm: f64,
    pub y_mm: f64,
    pub width_mm: f64,
    pub align: CoverAlign,
    pub size_pt: f64,
    /// Zeilenabstand als Typst-`leading` in em (Vorgabe 0.65).
    pub leading_em: f64,
    /// `light` (weiß), `dark` (`rgb("#1f2a33")`) oder `#rrggbb`.
    pub color: String,
    /// `None` = Schrift der Werkvorlage.
    #[serde(default)]
    pub font: Option<String>,
    pub style: FontStyle,
    pub backdrop: Backdrop,
    pub visible: bool,
    /// Nur beim Rücken gesetzt, sonst `None`.
    #[serde(default)]
    pub direction: Option<SpineDirection>,
    /// Nur bei Arten mit eigenem Text ([`CoverBlockKind::own_text`]).
    #[serde(default)]
    pub text: Option<String>,
}

/// Gespeicherte Anordnung eines Werks (leer = keine eigene Anordnung).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CoverLayout {
    pub blocks: Vec<CoverBlock>,
}

impl CoverLayout {
    pub fn is_empty(&self) -> bool {
        self.blocks.is_empty()
    }

    /// Block der Art `kind`, falls gespeichert.
    pub fn get(&self, kind: CoverBlockKind) -> Option<&CoverBlock> {
        self.blocks.iter().find(|b| b.kind == kind)
    }
}

/// Maße des Umschlags (mm): Seitenbreite W, Seitenhöhe H, Rücken S, Beschnitt b, Sicherheitsabstand.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CoverGeometry {
    pub page_width_mm: f64,
    pub page_height_mm: f64,
    pub spine_mm: f64,
    pub bleed_mm: f64,
    pub safety_mm: f64,
}

impl CoverGeometry {
    /// Maße mit dem Vorgabe-Sicherheitsabstand [`SAFETY_MM`].
    pub fn new(page_width_mm: f64, page_height_mm: f64, spine_mm: f64, bleed_mm: f64) -> CoverGeometry {
        CoverGeometry { page_width_mm, page_height_mm, spine_mm, bleed_mm, safety_mm: SAFETY_MM }
    }

    /// Gesamtumschlag (Breite, Höhe): (2W + S + 2b) × (H + 2b).
    pub fn wrap_size(&self) -> (f64, f64) {
        (2.0 * self.page_width_mm + self.spine_mm + 2.0 * self.bleed_mm, self.page_height_mm + 2.0 * self.bleed_mm)
    }

    /// Schnittkante (oben links) der Fläche im Gesamtumschlag: Rückseite (b, b), Rücken (b + W, b),
    /// Vorderseite (b + W + S, b).
    pub fn face_origin(&self, face: CoverFace) -> (f64, f64) {
        let b = self.bleed_mm;
        match face {
            CoverFace::Back => (b, b),
            CoverFace::Spine => (b + self.page_width_mm, b),
            CoverFace::Front => (b + self.page_width_mm + self.spine_mm, b),
        }
    }

    /// Größe der Fläche ohne Beschnitt: Rückseite und Vorderseite W × H, Rücken S × H.
    pub fn face_size(&self, face: CoverFace) -> (f64, f64) {
        match face {
            CoverFace::Back | CoverFace::Front => (self.page_width_mm, self.page_height_mm),
            CoverFace::Spine => (self.spine_mm, self.page_height_mm),
        }
    }
}

/// Ungefähre Höhe einer Textzeile in mm (Typst setzt von der Versalhöhe bis zur Grundlinie, ≈ 0,7 em).
fn line_mm(size_pt: f64) -> f64 {
    size_pt * 0.7 * 25.4 / 72.0
}

fn clamp_pos(value: f64) -> f64 {
    if value.is_finite() { value.clamp(POS_MM.0, POS_MM.1) } else { 0.0 }
}

fn clamp_width(value: f64) -> f64 {
    if value.is_finite() { value.clamp(WIDTH_MM.0, WIDTH_MM.1) } else { WIDTH_MM.0 }
}

/// Rand der bisherigen Rückseite (mm), wie `PANEL_MARGIN_MM` in [`crate::cover`].
const BACK_MARGIN_MM: f64 = 14.0;
/// Breite des Strichcode-Kastens: 36 mm Bild und 2 × 3 pt Innenabstand.
const BARCODE_BOX_MM: f64 = 36.0 + 6.0 * 25.4 / 72.0;
/// Höhe des Strichcode-Kastens (Bild 36 mm × 60/113, ISBN-Zeile, Innenabstand), gerundet.
const BARCODE_BOX_HEIGHT_MM: f64 = 25.0;

/// Vorgabeblöcke aller 15 Arten in [`CoverBlockKind::ALL`]-Reihenfolge; sie ahmen das Bild ohne eigene Anordnung
/// nach (nicht byte-gleich):
///
/// - Vorderseite: Autor, Titel (fett, 30 pt) und Untertitel untereinander, 10 % Rand links und rechts, im
///   Titelbereich `area` oben, in der Mitte oder unten; hinter dem Titel ein Verlauf (oben/unten) bzw. ein Schatten
///   (Mitte). Verlag und Freitext sind ausgeblendet.
/// - Rücken: Autor und Titel von oben nach unten lesbar, Schriftgröße wie bisher (0,45 × Rückenbreite, höchstens
///   11 pt), unter 6 mm Rückenbreite ausgeblendet wie bisher; Verlag und Freitext ausgeblendet.
/// - Rückseite: Klappentext oben (Blocksatz), Kurzbiografie darunter, unten links Verlag und Preis, unten rechts
///   der Strichcode, 14 mm Rand wie bisher; Motto und Freitext ausgeblendet.
pub fn default_blocks(area: TitleArea, geometry: &CoverGeometry) -> Vec<CoverBlock> {
    let (w, h) = geometry.face_size(CoverFace::Front);
    let spine = geometry.spine_mm.max(0.0);
    let safety = geometry.safety_mm;
    let block = |kind: CoverBlockKind, x: f64, y: f64, width: f64, size_pt: f64| CoverBlock {
        kind,
        x_mm: clamp_pos(x),
        y_mm: clamp_pos(y),
        width_mm: clamp_width(width),
        align: CoverAlign::Center,
        size_pt: size_pt.clamp(SIZE_PT.0, SIZE_PT.1),
        leading_em: 0.65,
        color: "light".to_owned(),
        font: None,
        style: FontStyle::Normal,
        backdrop: Backdrop::None,
        visible: !(kind.own_text() || matches!(kind, CoverBlockKind::FrontPublisher | CoverBlockKind::SpinePublisher)),
        direction: (kind.face() == CoverFace::Spine).then_some(SpineDirection::Down),
        text: kind.own_text().then(String::new),
    };

    // Vorderseite: Autor (13 pt), Abstand, Titel (30 pt), Abstand, Untertitel (15 pt).
    let (author_h, gap1, title_h, gap2, subtitle_h) = (line_mm(13.0), 4.0, line_mm(30.0), 3.5, line_mm(15.0));
    let stack = author_h + gap1 + title_h + gap2 + subtitle_h;
    let edge = (0.12 * w).max(safety);
    let top = match area {
        TitleArea::Top => edge,
        TitleArea::Middle => (h - stack) / 2.0,
        TitleArea::Bottom => h - edge - stack,
    };
    let (front_x, front_w) = (0.1 * w, 0.8 * w);
    let mut title = block(CoverBlockKind::FrontTitle, front_x, top + author_h + gap1, front_w, 30.0);
    title.style = FontStyle::Bold;
    title.backdrop = if area == TitleArea::Middle { Backdrop::Shadow } else { Backdrop::Gradient };
    let subtitle = block(CoverBlockKind::FrontSubtitle, front_x, top + author_h + gap1 + title_h + gap2, front_w, 15.0);
    let author = block(CoverBlockKind::FrontAuthor, front_x, top, front_w, 13.0);
    let publisher_y = if area == TitleArea::Bottom { edge } else { h - edge - line_mm(11.0) };
    let publisher = block(CoverBlockKind::FrontPublisher, front_x, publisher_y, front_w, 11.0);
    let front_free = block(CoverBlockKind::FrontFree, front_x, h * 0.5, front_w, 12.0);

    // Rücken: Text gedreht, Breite in Leserichtung = senkrechte Ausdehnung.
    let spine_size = (spine * 0.45 * 72.0 / 25.4).min(11.0);
    let spine_x = (spine - line_mm(spine_size.max(SIZE_PT.0))) / 2.0;
    let spine_block = |kind, y: f64, width: f64| {
        let mut b = block(kind, spine_x, y, width, spine_size);
        if spine < 6.0 {
            b.visible = false;
        }
        b
    };
    let spine_author = spine_block(CoverBlockKind::SpineAuthor, 0.1 * h, 0.3 * h);
    let spine_title = spine_block(CoverBlockKind::SpineTitle, 0.42 * h, 0.5 * h);
    let mut spine_publisher = spine_block(CoverBlockKind::SpinePublisher, h - safety - 0.15 * h, 0.15 * h);
    spine_publisher.visible = false;
    let mut spine_free = spine_block(CoverBlockKind::SpineFree, 0.42 * h, 0.5 * h);
    spine_free.visible = false;

    // Rückseite.
    let m = BACK_MARGIN_MM;
    let inner = w - 2.0 * m;
    let mut blurb = block(CoverBlockKind::BackBlurb, m, m, inner, 10.5);
    blurb.align = CoverAlign::Justify;
    let mut motto = block(CoverBlockKind::BackMotto, m, h * 0.42, inner, 11.0);
    motto.style = FontStyle::Italic;
    let barcode_y = h - m - BARCODE_BOX_HEIGHT_MM;
    let mut bio = block(CoverBlockKind::BackBio, m, barcode_y - 6.0 - 28.0, inner, 9.0);
    bio.align = CoverAlign::Left;
    let mut barcode = block(CoverBlockKind::BackBarcode, w - m - BARCODE_BOX_MM, barcode_y, BARCODE_BOX_MM, 7.5);
    barcode.align = CoverAlign::Right;
    let mut publisher_price = block(
        CoverBlockKind::BackPublisherPrice,
        m,
        h - m - 2.0 * line_mm(9.0) - 3.0,
        inner - BARCODE_BOX_MM - 4.0,
        9.0,
    );
    publisher_price.align = CoverAlign::Left;
    let back_free = block(CoverBlockKind::BackFree, m, h * 0.6, inner, 10.5);

    let all = [
        title,
        subtitle,
        author,
        publisher,
        front_free,
        spine_title,
        spine_author,
        spine_publisher,
        spine_free,
        motto,
        blurb,
        bio,
        barcode,
        publisher_price,
        back_free,
    ];
    let mut out: Vec<CoverBlock> = all.into_iter().collect();
    out.sort_by_key(|b| CoverBlockKind::ALL.iter().position(|k| *k == b.kind));
    // Runde 23a (vom PM entschieden): Lage und Breite auf 0,01 mm. Die Vorgaben rechnen aus dem Seitenmaß in pt;
    // ohne Runden stünden Werte wie 20,999999 mm in `cover.layout` und nach dem ersten Speichern in der Datenbank.
    for block in &mut out {
        block.x_mm = round_hundredth(block.x_mm);
        block.y_mm = round_hundredth(block.y_mm);
        block.width_mm = round_hundredth(block.width_mm);
    }
    out
}

/// Auf 0,01 mm gerundet (kaufmännisch); `-0.0` wird `0.0`.
fn round_hundredth(mm: f64) -> f64 {
    let rounded = (mm * 100.0).round() / 100.0;
    if rounded == 0.0 { 0.0 } else { rounded }
}

/// Je Art in [`CoverBlockKind::ALL`]-Reihenfolge der gespeicherte Block (`true`) oder der Vorgabeblock (`false`).
pub fn effective_blocks(stored: &CoverLayout, area: TitleArea, geometry: &CoverGeometry) -> Vec<(CoverBlock, bool)> {
    default_blocks(area, geometry)
        .into_iter()
        .map(|default| match stored.get(default.kind) {
            Some(block) => (block.clone(), true),
            None => (default, false),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_read_back_and_match_the_contract() {
        for kind in CoverBlockKind::ALL {
            assert_eq!(CoverBlockKind::parse(kind.as_str()), Some(kind), "Name liest sich zurück: {kind:?}");
            let json = serde_json::to_value(kind).unwrap();
            assert_eq!(json, kind.as_str(), "serde-Name = Datenbank-Name (Vertrag 1.1)");
            assert_eq!(kind.as_str().starts_with("spine_"), kind.face() == CoverFace::Spine, "Rücken-Präfix");
        }
        let own: Vec<&str> = CoverBlockKind::ALL.into_iter().filter(|k| k.own_text()).map(|k| k.as_str()).collect();
        assert_eq!(own, ["front_free", "spine_free", "back_motto", "back_free"], "eigener Text nur hier (1.1)");
    }

    #[test]
    fn faces_lie_side_by_side_from_the_bleed() {
        let g = CoverGeometry::new(148.0, 210.0, 12.0, 3.0);
        assert_eq!(g.wrap_size(), (2.0 * 148.0 + 12.0 + 6.0, 216.0), "(2W + S + 2b) × (H + 2b), Vertrag 1.2");
        assert_eq!(g.face_origin(CoverFace::Back), (3.0, 3.0), "Rückseite ab (b, b)");
        assert_eq!(g.face_origin(CoverFace::Spine), (151.0, 3.0), "Rücken ab (b + W, b)");
        assert_eq!(g.face_origin(CoverFace::Front), (163.0, 3.0), "Vorderseite ab (b + W + S, b)");
        assert_eq!(g.face_size(CoverFace::Spine), (12.0, 210.0), "Rücken S × H");
        assert_eq!(g.safety_mm, SAFETY_MM, "Vorgabe-Sicherheitsabstand");
    }

    #[test]
    fn effective_blocks_prefer_the_stored_one() {
        let g = CoverGeometry::new(148.0, 210.0, 12.0, 3.0);
        let defaults = default_blocks(TitleArea::Top, &g);
        assert_eq!(defaults.len(), 15, "alle Arten");
        let mut title = defaults[0].clone();
        title.x_mm = 20.0;
        let stored = CoverLayout { blocks: vec![title.clone()] };
        let effective = effective_blocks(&stored, TitleArea::Top, &g);
        assert_eq!(effective[0], (title, true), "gespeicherter Titel geht vor");
        assert!(effective[1..].iter().all(|(_, stored)| !stored), "sonst Vorgabeblöcke");
    }
}
