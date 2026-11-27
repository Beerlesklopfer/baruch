// SPDX-License-Identifier: AGPL-3.0-or-later
//! Print check of the cover and of the mandatory details in the imprint (contract Round 27, sections 1.7 and 1.8; S1
//! writes the signatures, afterwards the file belongs to umschlag-satz).
//!
//! - [`cover_checks`]: per visible block with content of the layout (stored or preview; without an own
//!   layout no block check, because then the previous picture applies):
//!   - `bleed`: the block extends beyond the trim edge of its face (for front and back also beyond the fold).
//!   - `safety`: the block lies closer to the edge than the safety margin (5 mm); on the spine only top and bottom.
//!   - `barcode`: a block of the back covers the barcode block.
//!   - `spine`: a spine block is thicker than the spine or falls out of it sideways.
//!   - `contrast`: the WCAG contrast ratio between text colour and the mean background below the block is
//!     below 3.0. Measured in the raster image of the cover (image crate, mapping as in typesetting with `fit: "cover"`
//!     onto the front including bleed), otherwise on the dark area; shadow (50 % black) and gradient
//!     (on average 30 % black) darken the background. SVG covers are not measured.
//!   - `font`: the chosen font is missing; typeset with the font of the work template or the embedded
//!     default.
//! - [`imprint_checks`] (only with front matter switched on): `imprint_publisher` (publisher or editor or its
//!   address missing), `imprint_printer` (print shop or address missing), `imprint_year` (year missing),
//!   `imprint_bible_notice` (Bible translation set, rights notice empty). Customary under the German state press
//!   laws, no legal advice.
//! - Round 22: `draft` – the cover PDF is created with the watermark „Entwurf“ (setting `draft.cover` or
//!   explicitly `?entwurf=1`/`draft: true`); [`cover_checks_with`].
//! - The image resolution check (`baruch_render::PrintWarning`) is unaffected by this.

use baruch_store::{BookMeta, CoverBlock, CoverBlockKind, CoverFace, CoverGeometry, CoverLayout, WorkId};
use baruch_typst::cover_layout::{Backdrop, effective_blocks};

use crate::AppState;
use crate::rpc::RpcError;

/// A warning of the print check: `code` from [`CODES`], affected block (none for the mandatory details) and
/// German message.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct CoverWarning {
    pub code: &'static str,
    pub block: Option<CoverBlockKind>,
    pub message: String,
}

/// All codes of the print check (contract 1.7, 1.8; Round 22: `draft`).
pub const CODES: [&str; 11] = [
    "bleed",
    "safety",
    "barcode",
    "spine",
    "contrast",
    "font",
    "imprint_publisher",
    "imprint_printer",
    "imprint_year",
    "imprint_bible_notice",
    "draft",
];

/// Round 22: message if the cover PDF carries a watermark.
pub const DRAFT_WARNING: &str = "Das Umschlag-PDF trägt das Wasserzeichen „Entwurf“ – so nicht an die Druckerei geben \
     (ohne Wasserzeichen: Umschlag-PDF mit ?entwurf=0 oder „Entwurf auch auf dem Umschlag“ ausschalten).";

/// Minimum contrast (WCAG ratio) between text and background.
pub const MIN_CONTRAST: f64 = 3.0;

/// Dark area of back and spine (`rgb("#1f2a33")`).
const DARK_RGB: [f64; 3] = [31.0, 42.0, 51.0];

/// Darkening by the background of the block (share of black).
fn backdrop_alpha(backdrop: Backdrop) -> f64 {
    match backdrop {
        Backdrop::None => 0.0,
        Backdrop::Shadow => 0.5,
        // Gradient 0 % → 35 % → 0 %: on average a good 17 % over the block with margin; counted with the edge of the
        // gradient just above the text 30 %.
        Backdrop::Gradient => 0.3,
    }
}

/// Print check of the cover with `layout` (preview) or the stored layout (`None`), plus the
/// mandatory details from [`imprint_checks`]. The watermark applies by setting (see [`cover_checks_with`]).
pub async fn cover_checks(
    state: &AppState,
    work: WorkId,
    layout: Option<&CoverLayout>,
) -> Result<Vec<CoverWarning>, RpcError> {
    cover_checks_with(state, work, layout, None).await
}

/// Like [`cover_checks`] with the draft switch of the cover PDF (Round 22): if the PDF is created with a watermark
/// ([`crate::typeset::cover_draft`]), the warning `draft` stands in front ([`DRAFT_WARNING`]).
pub async fn cover_checks_with(
    state: &AppState,
    work: WorkId,
    layout: Option<&CoverLayout>,
    draft: Option<bool>,
) -> Result<Vec<CoverWarning>, RpcError> {
    let mut warnings = Vec::new();
    let stored = crate::typeset::stored_draft(state, work).await?;
    if crate::typeset::cover_draft(stored.as_ref(), draft).is_some() {
        warnings.push(CoverWarning { code: "draft", block: None, message: DRAFT_WARNING.to_owned() });
    }
    warnings.extend(block_and_imprint_checks(state, work, layout).await?);
    Ok(warnings)
}

async fn block_and_imprint_checks(
    state: &AppState,
    work: WorkId,
    layout: Option<&CoverLayout>,
) -> Result<Vec<CoverWarning>, RpcError> {
    let meta = state.store.book_meta(work).await?;
    let bible = state.store.work_meta(work).await?.bible;
    let stored;
    let layout = match layout {
        Some(layout) => layout,
        None => {
            stored = state.store.cover_layout(work).await?;
            &stored
        }
    };
    let mut warnings = Vec::new();
    if !layout.is_empty() {
        let wrap = crate::cover::build_wrap_for(state, work, Some(layout)).await?;
        let geometry = wrap.geometry;
        let blocks: Vec<CoverBlock> = effective_blocks(layout, crate::cover::title_area(meta.title_area), &geometry)
            .into_iter()
            .map(|(b, _)| b)
            .collect();
        let heights = crate::cover::block_heights(state, work, &blocks, &geometry).await?;
        let background = front_background(state, work, &meta).await?;
        let fonts: Vec<Option<String>> = blocks.iter().map(|b| b.font.clone()).collect();
        let missing: Vec<bool> = tokio::task::spawn_blocking(move || {
            fonts.iter().map(|f| f.as_deref().is_some_and(|f| !baruch_render::has_font_family(f))).collect()
        })
        .await
        .unwrap_or_default();
        let default_font = crate::cover_layout::default_font(state, work).await?;
        warnings.extend(block_checks(&blocks, &heights, &geometry, &background));
        for (block, missing) in blocks.iter().zip(missing) {
            if missing && block.visible {
                warnings.push(CoverWarning {
                    code: "font",
                    block: Some(block.kind),
                    message: format!(
                        "{}: Schrift „{}“ fehlt; gesetzt mit der Vorgabe „{default_font}“.",
                        name(block.kind),
                        block.font.as_deref().unwrap_or("")
                    ),
                });
            }
        }
    }
    warnings.extend(imprint_checks(&meta, &bible));
    Ok(warnings)
}

/// „Titel (Vorderseite)“.
fn name(kind: CoverBlockKind) -> String {
    format!("{} ({})", kind.label(), kind.face().label())
}

/// „12,5 mm“.
fn mm(value: f64) -> String {
    format!("{value:.1} mm").replace('.', ",")
}

/// Frame of a block on its face (x, y, width, height in mm, unrotated): on the spine the width in
/// reading direction is the vertical extent and the measured height the horizontal one.
fn rect(block: &CoverBlock, height: f64) -> (f64, f64, f64, f64) {
    if block.kind.face() == CoverFace::Spine {
        (block.x_mm, block.y_mm, height, block.width_mm)
    } else {
        (block.x_mm, block.y_mm, block.width_mm, height)
    }
}

/// Background of the front for the contrast check.
pub enum Background {
    /// Cover image as a raster image (PNG, JPEG …).
    Raster(std::sync::Arc<image::RgbImage>),
    /// No cover image: the dark area like back and spine.
    Dark,
    /// SVG (no pixels), unreadable or larger than [`MAX_CONTRAST_SIDE_PX`]: not measured.
    NotMeasured,
}

/// Round 23a: maximum side length (pixels) of a cover image that is unpacked for the contrast check – as for
/// the thumbnails (`assets::thumbnail_bytes`). A larger image is not unpacked and not measured.
pub const MAX_CONTRAST_SIDE_PX: u32 = 20_000;

/// Maximum memory when unpacking (512 MiB, as for the thumbnails).
const MAX_CONTRAST_ALLOC: u64 = 512 * 1024 * 1024;

/// Cache of the unpacked cover images: together at most this many bytes (as RGB); a single image
/// that is larger is unpacked every time.
const MAX_CACHED_RGB_BYTES: usize = 96 * 1024 * 1024;

/// Maximum number of images in the cache (most recently used first).
const MAX_CACHED_IMAGES: usize = 8;

/// How often the contrast check unpacked the image with this checksum (SHA-256 of the bytes, hex) in this process
/// (for tests of the cache; at most 64 images are counted, then the counting starts anew).
pub fn contrast_decodes(sha256: &str) -> usize {
    CONTRAST_DECODES.lock().ok().and_then(|counts| counts.get(sha256).copied()).unwrap_or(0)
}

static CONTRAST_DECODES: std::sync::Mutex<std::collections::BTreeMap<String, usize>> =
    std::sync::Mutex::new(std::collections::BTreeMap::new());

/// Most recently unpacked cover images (checksum of the bytes, pixels), the most recently used in front. The print
/// check runs on every drawing of the cover page and on every drag of a block for the same image; several entries
/// so that two works (or simultaneous tests) do not displace each other.
static CONTRAST_CACHE: std::sync::Mutex<Vec<(String, std::sync::Arc<image::RgbImage>)>> =
    std::sync::Mutex::new(Vec::new());

/// Image from the cache (and moved to the front).
fn cached_contrast_image(sum: &str) -> Option<std::sync::Arc<image::RgbImage>> {
    let mut cache = CONTRAST_CACHE.lock().ok()?;
    let at = cache.iter().position(|(known, _)| known == sum)?;
    let entry = cache.remove(at);
    let image = entry.1.clone();
    cache.insert(0, entry);
    Some(image)
}

/// Remember an image; older ones are dropped until number and total size fit.
fn remember_contrast_image(sum: String, image: &std::sync::Arc<image::RgbImage>) {
    if image.as_raw().len() > MAX_CACHED_RGB_BYTES {
        return;
    }
    let Ok(mut cache) = CONTRAST_CACHE.lock() else { return };
    cache.retain(|(known, _)| *known != sum);
    cache.insert(0, (sum, image.clone()));
    let mut total = 0usize;
    let mut keep = 0usize;
    for (_, kept) in cache.iter() {
        total += kept.as_raw().len();
        if keep >= MAX_CACHED_IMAGES || total > MAX_CACHED_RGB_BYTES {
            break;
        }
        keep += 1;
    }
    cache.truncate(keep.max(1));
}

/// Unpack a raster image for the contrast check: `None` for SVG, unreadable bytes or more than
/// [`MAX_CONTRAST_SIDE_PX`] per side (protection against “pixel bombs”). Blocking.
pub fn decode_for_contrast(bytes: &[u8]) -> Option<image::RgbImage> {
    if bytes.trim_ascii_start().starts_with(b"<") {
        return None; // SVG: no pixels
    }
    let mut reader = image::ImageReader::new(std::io::Cursor::new(bytes)).with_guessed_format().ok()?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_CONTRAST_SIDE_PX);
    limits.max_image_height = Some(MAX_CONTRAST_SIDE_PX);
    limits.max_alloc = Some(MAX_CONTRAST_ALLOC);
    reader.limits(limits);
    reader.decode().ok().map(|picture| picture.to_rgb8())
}

/// Background of the front: raster image, dark area (without cover) or not measurable (SVG, too large).
async fn front_background(state: &AppState, work: WorkId, meta: &BookMeta) -> Result<Background, RpcError> {
    let Some(name) = &meta.cover else { return Ok(Background::Dark) };
    let data = state.store.asset_data(work, name, None).await?;
    let bytes = data.bytes()?;
    let decoded = tokio::task::spawn_blocking(move || {
        let sum = baruch_render::sha256_hex(&bytes);
        if let Some(image) = cached_contrast_image(&sum) {
            return Some(image);
        }
        if let Ok(mut counts) = CONTRAST_DECODES.lock() {
            if counts.len() >= 64 && !counts.contains_key(&sum) {
                counts.clear();
            }
            *counts.entry(sum.clone()).or_insert(0) += 1;
        }
        let image = std::sync::Arc::new(decode_for_contrast(&bytes)?);
        remember_contrast_image(sum, &image);
        Some(image)
    })
    .await
    .ok()
    .flatten();
    Ok(decoded.map_or(Background::NotMeasured, Background::Raster))
}

/// Image of a raster cover.
struct Raster<'a>(&'a image::RgbImage);

impl Raster<'_> {
    /// Mean colour below the rectangle (mm, relative to the trim edge of the front). Mapping as in the cover
    /// for the print shop: the image fills the front including bleed at top, bottom and outside (`fit: "cover"`,
    /// cropped centred).
    fn mean_rgb(&self, geometry: &CoverGeometry, area: (f64, f64, f64, f64)) -> Option<[f64; 3]> {
        let (iw, ih) = (f64::from(self.0.width()), f64::from(self.0.height()));
        if iw < 1.0 || ih < 1.0 {
            return None;
        }
        let b = geometry.bleed_mm.max(0.0);
        let (fx, fy, fw, fh) = (0.0, -b, geometry.page_width_mm + b, geometry.page_height_mm + 2.0 * b);
        let scale = (fw / iw).max(fh / ih); // mm per pixel
        let (ox, oy) = (fx + (fw - iw * scale) / 2.0, fy + (fh - ih * scale) / 2.0);
        let (x, y, w, h) = area;
        let steps = 40usize;
        let mut sum = [0.0; 3];
        let mut count = 0.0;
        for i in 0..steps {
            for j in 0..steps {
                let px = x + w * (i as f64 + 0.5) / steps as f64;
                let py = y + h * (j as f64 + 0.5) / steps as f64;
                let u = ((px - ox) / scale).floor();
                let v = ((py - oy) / scale).floor();
                if u < 0.0 || v < 0.0 || u >= iw || v >= ih {
                    continue;
                }
                let pixel = self.0.get_pixel(u as u32, v as u32);
                for (k, s) in sum.iter_mut().enumerate() {
                    *s += f64::from(pixel[k]);
                }
                count += 1.0;
            }
        }
        (count > 0.0).then(|| sum.map(|s| s / count))
    }
}

/// Relative luminance per WCAG 2 from sRGB (0–255).
pub fn luminance(rgb: [f64; 3]) -> f64 {
    let channel = |c: f64| {
        let c = (c / 255.0).clamp(0.0, 1.0);
        if c <= 0.039_28 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
    };
    0.2126 * channel(rgb[0]) + 0.7152 * channel(rgb[1]) + 0.0722 * channel(rgb[2])
}

/// WCAG contrast ratio of two colours (1 to 21).
pub fn contrast_ratio(a: [f64; 3], b: [f64; 3]) -> f64 {
    let (la, lb) = (luminance(a), luminance(b));
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

/// Text colour of a block as sRGB.
fn text_rgb(color: &str) -> [f64; 3] {
    match color.trim() {
        "dark" => DARK_RGB,
        hex if baruch_typst::cover::is_hex_color(hex) => {
            let at = |i: usize| f64::from(u8::from_str_radix(&hex[i..i + 2], 16).unwrap_or(255));
            [at(1), at(3), at(5)]
        }
        _ => [255.0, 255.0, 255.0],
    }
}

/// Position checks and contrast per block (without fonts; [`cover_checks`] checks those separately, blocking).
pub fn block_checks(
    blocks: &[CoverBlock],
    heights: &[f64],
    geometry: &CoverGeometry,
    background: &Background,
) -> Vec<CoverWarning> {
    let mut out = Vec::new();
    let safety = geometry.safety_mm;
    let barcode = blocks
        .iter()
        .zip(heights)
        .find(|(b, h)| b.kind == CoverBlockKind::BackBarcode && b.visible && **h > 0.0)
        .map(|(b, h)| rect(b, *h));
    for (block, &height) in blocks.iter().zip(heights) {
        if !block.visible || height <= 0.0 {
            continue;
        }
        let face = block.kind.face();
        let (fw, fh) = geometry.face_size(face);
        let (x, y, w, h) = rect(block, height);
        let who = name(block.kind);
        let mut warn = |code: &'static str, message: String| {
            out.push(CoverWarning { code, block: Some(block.kind), message });
        };
        let eps = 1e-6;
        if face == CoverFace::Spine {
            if x < -eps || x + w > fw + eps {
                warn(
                    "spine",
                    format!(
                        "{who} fällt aus dem Rücken: {} dick ab x {}, der Rücken ist {} breit.",
                        mm(w),
                        mm(x),
                        mm(fw)
                    ),
                );
            }
            if y < -eps || y + h > fh + eps {
                warn("bleed", format!("{who} ragt oben oder unten in den Beschnitt (y {} bis {}).", mm(y), mm(y + h)));
            } else if y < safety - eps || y + h > fh - safety + eps {
                warn("safety", format!("{who} liegt näher als {} an der Schnittkante oben oder unten.", mm(safety)));
            }
        } else if x < -eps || y < -eps || x + w > fw + eps || y + h > fh + eps {
            warn(
                "bleed",
                format!(
                    "{who} ragt über die Schnittkante bzw. den Falz in den Beschnitt (x {} bis {}, y {} bis {}; Fläche {} × {}).",
                    mm(x),
                    mm(x + w),
                    mm(y),
                    mm(y + h),
                    mm(fw),
                    mm(fh)
                ),
            );
        } else if x < safety - eps || y < safety - eps || x + w > fw - safety + eps || y + h > fh - safety + eps {
            warn("safety", format!("{who} liegt näher als {} an der Schnittkante bzw. am Falz.", mm(safety)));
        }
        if face == CoverFace::Back
            && block.kind != CoverBlockKind::BackBarcode
            && let Some((bx, by, bw, bh)) = barcode
            && x < bx + bw
            && bx < x + w
            && y < by + bh
            && by < y + h
        {
            warn("barcode", format!("{who} überdeckt den Strichcode."));
        }
        // Contrast: background below the block (raster image of the front or dark area), darkened
        // by shadow or gradient.
        let base = match (face, background) {
            (CoverFace::Front, Background::Raster(image)) => Raster(image).mean_rgb(geometry, (x, y, w, h)),
            (CoverFace::Front, Background::NotMeasured) => None,
            (CoverFace::Front, Background::Dark) | (CoverFace::Spine | CoverFace::Back, _) => Some(DARK_RGB),
        };
        if let Some(base) = base {
            let alpha = backdrop_alpha(block.backdrop);
            let shown = base.map(|c| c * (1.0 - alpha));
            let ratio = contrast_ratio(text_rgb(&block.color), shown);
            if ratio < MIN_CONTRAST {
                warn(
                    "contrast",
                    format!(
                        "{who}: Kontrast {} : 1 zum Hintergrund ist zu gering (mindestens {} : 1); hellere bzw. dunklere \
                         Schrift, Schatten oder Verlauf wählen.",
                        format!("{ratio:.1}").replace('.', ","),
                        format!("{MIN_CONTRAST:.1}").replace('.', ",")
                    ),
                );
            }
        }
    }
    out
}

/// Mandatory details in the imprint (contract 1.7), only with front matter switched on; `bible` is the Bible
/// translation of the work. Customary under the German state press laws – no legal advice.
pub fn imprint_checks(meta: &BookMeta, bible: &str) -> Vec<CoverWarning> {
    if !meta.front_matter {
        return Vec::new();
    }
    let empty = |s: &str| s.trim().is_empty();
    let mut out = Vec::new();
    let mut warn = |code: &'static str, message: &str| {
        out.push(CoverWarning { code, block: None, message: message.to_owned() });
    };
    if empty(&meta.publisher) || empty(&meta.publisher_address) {
        warn(
            "imprint_publisher",
            "Impressum: Verlag bzw. Herausgeber oder dessen Anschrift fehlt (bei Selbstverlag Name und Anschrift des \
             Verfassers bzw. Herausgebers).",
        );
    }
    if empty(&meta.printer) || empty(&meta.printer_address) {
        warn("imprint_printer", "Impressum: Druckerei oder deren Anschrift fehlt.");
    }
    if empty(&meta.year) {
        warn("imprint_year", "Impressum: Erscheinungsjahr fehlt.");
    }
    if !empty(bible) && empty(&meta.bible_notice) {
        warn(
            "imprint_bible_notice",
            "Impressum: Eine Bibelübersetzung ist gesetzt, aber der Rechtehinweis der Übersetzung fehlt.",
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wcag_contrast_matches_known_values() {
        assert!((contrast_ratio([255.0; 3], [0.0; 3]) - 21.0).abs() < 1e-9, "Weiß auf Schwarz = 21:1");
        assert!((contrast_ratio([128.0; 3], [128.0; 3]) - 1.0).abs() < 1e-9, "gleiche Farbe = 1:1");
        let grey = contrast_ratio([255.0; 3], [118.0; 3]);
        assert!(grey > 4.4 && grey < 4.7, "Weiß auf #767676 ≈ 4,5:1 (bekannter WCAG-Grenzwert): {grey}");
    }

    #[test]
    fn text_colours_follow_the_contract_words() {
        assert_eq!(text_rgb("light"), [255.0; 3], "light = weiß");
        assert_eq!(text_rgb("dark"), DARK_RGB, "dark = #1f2a33");
        assert_eq!(text_rgb("#ff8000"), [255.0, 128.0, 0.0], "Hex-Farbe");
        assert_eq!(text_rgb("#zzzzzz"), [255.0; 3], "ungültig → weiß wie im Satz");
    }
}
