// SPDX-License-Identifier: AGPL-3.0-or-later
//! Cover layout (contract Round 27, sections 1.1–1.3 and 2.1): freely placed blocks on front, spine and back.
//!
//! - **Blocks** ([`CoverBlockKind`], 15 kinds): at most one per work and kind. Only the free texts and the motto
//!   have text of their own ([`CoverBlock::text`], at most [`MAX_TEXT_CHARS`] characters)
//!   ([`CoverBlockKind::own_text`]); all others take their text from work and book details.
//! - **Coordinates** in millimetres, relative to the trim edge (top left) of the own face, without bleed; negative
//!   values reach into the bleed. `(x_mm, y_mm)` is the top left corner of the block frame in the unrotated page
//!   coordinate system, `width_mm` the width in reading direction (on the spine the vertical extent on the page).
//!   [`CoverGeometry`] calculates the faces in the whole cover (back | spine | front).
//! - **Limits** ([`POS_MM`], [`WIDTH_MM`], [`SIZE_PT`], [`LEADING_EM`], [`MAX_TEXT_CHARS`], [`MAX_FONT_CHARS`]) are
//!   the same as the CHECKs of migration 0045 (`cover_block`).
//! - Without a stored row the default block applies ([`default_blocks`]); [`effective_blocks`] mixes both.
//!
//! The types and signatures are frozen after the interface (S1); the bodies of [`default_blocks`] and
//! [`effective_blocks`] belong to the task umschlag-satz (R27b). The blocks are set in
//! [`crate::cover::frame_parts`] and [`crate::cover::wrap_source`].

use serde::{Deserialize, Serialize};

use crate::cover::TitleArea;

/// Safety distance to the trim edge in mm (default).
pub const SAFETY_MM: f64 = 5.0;
/// Limits of `x_mm` and `y_mm` (mm, inclusive).
pub const POS_MM: (f64, f64) = (-50.0, 1000.0);
/// Limits of `width_mm` (mm, inclusive).
pub const WIDTH_MM: (f64, f64) = (1.0, 1000.0);
/// Limits of `size_pt` (pt, inclusive).
pub const SIZE_PT: (f64, f64) = (4.0, 200.0);
/// Limits of `leading_em` (em, inclusive).
pub const LEADING_EM: (f64, f64) = (0.0, 3.0);
/// Maximum length of the own text in characters.
pub const MAX_TEXT_CHARS: usize = 1000;
/// Maximum length of a font name in characters.
pub const MAX_FONT_CHARS: usize = 200;

/// Face of the cover.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CoverFace {
    Front,
    Spine,
    Back,
}

impl CoverFace {
    pub const ALL: [CoverFace; 3] = [CoverFace::Front, CoverFace::Spine, CoverFace::Back];

    /// `front`, `spine`, `back` (RPC, forms).
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

    /// German name: Vorderseite, Rücken, Rückseite.
    pub fn label(self) -> &'static str {
        match self {
            CoverFace::Front => "Vorderseite",
            CoverFace::Spine => "Rücken",
            CoverFace::Back => "Rückseite",
        }
    }
}

/// Kind of a block (database, RPC and serde with the same name, e.g. `front_title`).
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

    /// Name in database, RPC and forms (`front_title` …).
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

    /// Face on which the block stands.
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

    /// German name without face (e.g. „Titel“; with face: „Titel (Vorderseite)“).
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

    /// Does the block have text of its own (`text`)? Only the three free texts and the motto.
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

/// Words for the small enumerations (database, RPC, forms).
macro_rules! words {
    ($name:ident { $($variant:ident => $word:literal),+ $(,)? }) => {
        impl $name {
            /// Name in database, RPC and forms.
            pub fn as_str(self) -> &'static str {
                match self { $($name::$variant => $word),+ }
            }

            pub fn parse(s: &str) -> Option<$name> {
                match s.trim() { $($word => Some($name::$variant),)+ _ => None }
            }
        }
    };
}

/// Alignment of the text in the block frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CoverAlign {
    Left,
    Center,
    Right,
    Justify,
}
words!(CoverAlign { Left => "left", Center => "center", Right => "right", Justify => "justify" });

/// Font weight.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FontStyle {
    Normal,
    Bold,
    Italic,
}
words!(FontStyle { Normal => "normal", Bold => "bold", Italic => "italic" });

/// Area behind the block: none, semi-transparent shadow or gradient (as before behind the title).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Backdrop {
    None,
    Shadow,
    Gradient,
}
words!(Backdrop { None => "none", Shadow => "shadow", Gradient => "gradient" });

/// Direction of the text on the spine: `down` = readable from top to bottom (`rotate(90deg)`), `up` = from bottom
/// to top (`rotate(-90deg)`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpineDirection {
    Down,
    Up,
}
words!(SpineDirection { Down => "down", Up => "up" });

/// One block of the layout (values and limits: module documentation, contract 1.3).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CoverBlock {
    pub kind: CoverBlockKind,
    pub x_mm: f64,
    pub y_mm: f64,
    pub width_mm: f64,
    pub align: CoverAlign,
    pub size_pt: f64,
    /// Line spacing as Typst `leading` in em (default 0.65).
    pub leading_em: f64,
    /// `light` (white), `dark` (`rgb("#1f2a33")`) or `#rrggbb`.
    pub color: String,
    /// `None` = font of the work template.
    #[serde(default)]
    pub font: Option<String>,
    pub style: FontStyle,
    pub backdrop: Backdrop,
    pub visible: bool,
    /// Only set on the spine, otherwise `None`.
    #[serde(default)]
    pub direction: Option<SpineDirection>,
    /// Only for kinds with text of their own ([`CoverBlockKind::own_text`]).
    #[serde(default)]
    pub text: Option<String>,
}

/// Stored layout of a work (empty = no layout of its own).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CoverLayout {
    pub blocks: Vec<CoverBlock>,
}

impl CoverLayout {
    pub fn is_empty(&self) -> bool {
        self.blocks.is_empty()
    }

    /// Block of the kind `kind`, if stored.
    pub fn get(&self, kind: CoverBlockKind) -> Option<&CoverBlock> {
        self.blocks.iter().find(|b| b.kind == kind)
    }
}

/// Dimensions of the cover (mm): page width W, page height H, spine S, bleed b, safety distance.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CoverGeometry {
    pub page_width_mm: f64,
    pub page_height_mm: f64,
    pub spine_mm: f64,
    pub bleed_mm: f64,
    pub safety_mm: f64,
}

impl CoverGeometry {
    /// Dimensions with the default safety distance [`SAFETY_MM`].
    pub fn new(page_width_mm: f64, page_height_mm: f64, spine_mm: f64, bleed_mm: f64) -> CoverGeometry {
        CoverGeometry { page_width_mm, page_height_mm, spine_mm, bleed_mm, safety_mm: SAFETY_MM }
    }

    /// Whole cover (width, height): (2W + S + 2b) × (H + 2b).
    pub fn wrap_size(&self) -> (f64, f64) {
        (2.0 * self.page_width_mm + self.spine_mm + 2.0 * self.bleed_mm, self.page_height_mm + 2.0 * self.bleed_mm)
    }

    /// Trim edge (top left) of the face in the whole cover: back (b, b), spine (b + W, b), front (b + W + S, b).
    pub fn face_origin(&self, face: CoverFace) -> (f64, f64) {
        let b = self.bleed_mm;
        match face {
            CoverFace::Back => (b, b),
            CoverFace::Spine => (b + self.page_width_mm, b),
            CoverFace::Front => (b + self.page_width_mm + self.spine_mm, b),
        }
    }

    /// Size of the face without bleed: back and front W × H, spine S × H.
    pub fn face_size(&self, face: CoverFace) -> (f64, f64) {
        match face {
            CoverFace::Back | CoverFace::Front => (self.page_width_mm, self.page_height_mm),
            CoverFace::Spine => (self.spine_mm, self.page_height_mm),
        }
    }
}

/// Approximate height of a text line in mm (Typst sets from the cap height to the baseline, ≈ 0.7 em).
fn line_mm(size_pt: f64) -> f64 {
    size_pt * 0.7 * 25.4 / 72.0
}

fn clamp_pos(value: f64) -> f64 {
    if value.is_finite() { value.clamp(POS_MM.0, POS_MM.1) } else { 0.0 }
}

fn clamp_width(value: f64) -> f64 {
    if value.is_finite() { value.clamp(WIDTH_MM.0, WIDTH_MM.1) } else { WIDTH_MM.0 }
}

/// Margin of the previous back (mm), like `PANEL_MARGIN_MM` in [`crate::cover`].
const BACK_MARGIN_MM: f64 = 14.0;
/// Width of the barcode box: 36 mm image and 2 × 3 pt padding.
const BARCODE_BOX_MM: f64 = 36.0 + 6.0 * 25.4 / 72.0;
/// Height of the barcode box (image 36 mm × 60/113, ISBN line, padding), rounded.
const BARCODE_BOX_HEIGHT_MM: f64 = 25.0;

/// Default blocks of all 15 kinds in [`CoverBlockKind::ALL`] order; they imitate the appearance without a layout of
/// its own (not byte-identical):
///
/// - Front: author, title (bold, 30 pt) and subtitle below each other, 10 % margin left and right, in the title
///   area `area` at the top, in the middle or at the bottom; behind the title a gradient (top/bottom) or a shadow
///   (middle). Publisher and free text are hidden.
/// - Spine: author and title readable from top to bottom, font size as before (0.45 × spine width, at most 11 pt),
///   hidden below 6 mm spine width as before; publisher and free text hidden.
/// - Back: blurb at the top (justified), short biography below it, bottom left publisher and price, bottom right
///   the barcode, 14 mm margin as before; motto and free text hidden.
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

    // Front: author (13 pt), gap, title (30 pt), gap, subtitle (15 pt).
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

    // Spine: text rotated, width in reading direction = vertical extent.
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

    // Back.
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
    // Round 23a (decided by the PM): position and width to 0.01 mm. The defaults calculate from the page dimension
    // in pt; without rounding, values like 20.999999 mm would stand in `cover.layout` and after the first save in
    // the database.
    for block in &mut out {
        block.x_mm = round_hundredth(block.x_mm);
        block.y_mm = round_hundredth(block.y_mm);
        block.width_mm = round_hundredth(block.width_mm);
    }
    out
}

/// Rounded to 0.01 mm (half away from zero); `-0.0` becomes `0.0`.
fn round_hundredth(mm: f64) -> f64 {
    let rounded = (mm * 100.0).round() / 100.0;
    if rounded == 0.0 { 0.0 } else { rounded }
}

/// Per kind in [`CoverBlockKind::ALL`] order the stored block (`true`) or the default block (`false`).
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
