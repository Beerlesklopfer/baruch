// SPDX-License-Identifier: AGPL-3.0-or-later
//! Bilder für den Schreiber: Dateiendung nach Medientyp und Größe im Dokument aus den Bildpunkten (PNG, JPEG, GIF).

/// Seite A4 in Twips (1/20 Punkt) und Ränder (2,5 cm seitlich und oben, 2 cm unten).
pub(super) const PAGE_WIDTH_TWIPS: u32 = 11906;
pub(super) const PAGE_HEIGHT_TWIPS: u32 = 16838;
pub(super) const MARGIN_SIDE_TWIPS: u32 = 1418;
pub(super) const MARGIN_TOP_TWIPS: u32 = 1418;
pub(super) const MARGIN_BOTTOM_TWIPS: u32 = 1134;
/// Breite des Satzspiegels.
pub(super) const TEXT_WIDTH_TWIPS: u32 = PAGE_WIDTH_TWIPS - 2 * MARGIN_SIDE_TWIPS;

/// EMU (English Metric Units) je Twip und je Bildpunkt bei 96 dpi.
const EMU_PER_TWIP: u64 = 635;
const EMU_PER_PIXEL: u64 = 9525;
/// Größte Bildhöhe: zwei Drittel der Seite, damit die Unterschrift auf derselben Seite bleibt.
const MAX_HEIGHT_EMU: u64 = (PAGE_HEIGHT_TWIPS as u64) * EMU_PER_TWIP * 2 / 3;
/// Größe, wenn sich die Bildpunkte nicht lesen lassen (8 cm × 6 cm).
const FALLBACK_EMU: (u64, u64) = (2_880_000, 2_160_000);

/// Dateiendung unter `word/media/`: nach dem Medientyp, sonst nach dem Dateinamen, sonst `bin`.
pub(super) fn extension(media_type: &str, file_name: &str) -> String {
    match media_type.trim().to_ascii_lowercase().as_str() {
        "image/png" => "png".into(),
        "image/jpeg" | "image/jpg" => "jpeg".into(),
        "image/gif" => "gif".into(),
        "image/svg+xml" => "svg".into(),
        "image/webp" => "webp".into(),
        "image/tiff" => "tiff".into(),
        "image/bmp" => "bmp".into(),
        _ => file_name
            .rsplit_once('.')
            .map(|(_, ext)| ext.to_ascii_lowercase())
            .filter(|ext| !ext.is_empty() && ext.len() <= 5 && ext.bytes().all(|b| b.is_ascii_alphanumeric()))
            .unwrap_or_else(|| "bin".into()),
    }
}

/// Medientyp zu einer Dateiendung aus [`extension`] (für `[Content_Types].xml`).
pub(super) fn content_type(extension: &str) -> &'static str {
    match extension {
        "png" => "image/png",
        "jpeg" | "jpg" => "image/jpeg",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        "webp" => "image/webp",
        "tiff" | "tif" => "image/tiff",
        "bmp" => "image/bmp",
        _ => "application/octet-stream",
    }
}

fn be16(bytes: &[u8], at: usize) -> Option<u64> {
    Some(u64::from(u16::from_be_bytes([*bytes.get(at)?, *bytes.get(at + 1)?])))
}

/// Breite und Höhe in Bildpunkten aus dem Kopf eines PNG, GIF oder JPEG.
pub(super) fn pixels(bytes: &[u8]) -> Option<(u64, u64)> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") && bytes.get(12..16) == Some(b"IHDR") {
        let width = (be16(bytes, 16)? << 16) | be16(bytes, 18)?;
        let height = (be16(bytes, 20)? << 16) | be16(bytes, 22)?;
        return Some((width, height));
    }
    if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        let width = u64::from(u16::from_le_bytes([*bytes.get(6)?, *bytes.get(7)?]));
        let height = u64::from(u16::from_le_bytes([*bytes.get(8)?, *bytes.get(9)?]));
        return Some((width, height));
    }
    if bytes.starts_with(&[0xff, 0xd8]) {
        // JPEG: Segmente durchgehen bis zum Bildkopf (SOF0–SOF15 außer DHT, JPG, DAC).
        let mut at = 2;
        while at + 4 <= bytes.len() {
            if bytes[at] != 0xff {
                return None;
            }
            let marker = bytes[at + 1];
            if marker == 0xff {
                at += 1;
                continue;
            }
            if (0xd0..=0xd9).contains(&marker) || marker == 0x01 {
                at += 2;
                continue;
            }
            let length = be16(bytes, at + 2)? as usize;
            if (0xc0..=0xcf).contains(&marker) && !matches!(marker, 0xc4 | 0xc8 | 0xcc) {
                return Some((be16(bytes, at + 7)?, be16(bytes, at + 5)?));
            }
            at += 2 + length.max(2);
        }
    }
    None
}

/// Größe im Dokument in EMU: 96 Bildpunkte je Zoll, höchstens so breit wie der Satzspiegel und zwei Drittel der
/// Seite hoch (Seitenverhältnis bleibt).
pub(super) fn extent(bytes: &[u8]) -> (u64, u64) {
    let Some((width, height)) = pixels(bytes).filter(|(w, h)| *w > 0 && *h > 0 && *w < 1 << 20 && *h < 1 << 20) else {
        return FALLBACK_EMU;
    };
    let (mut cx, mut cy) = (width * EMU_PER_PIXEL, height * EMU_PER_PIXEL);
    let max_width = u64::from(TEXT_WIDTH_TWIPS) * EMU_PER_TWIP;
    if cx > max_width {
        cy = cy * max_width / cx;
        cx = max_width;
    }
    if cy > MAX_HEIGHT_EMU {
        cx = cx * MAX_HEIGHT_EMU / cy;
        cy = MAX_HEIGHT_EMU;
    }
    (cx.max(1), cy.max(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn size_comes_from_the_pixels_and_stays_inside_the_page() {
        let one = crate::sample::SAMPLE_PNG;
        assert_eq!(pixels(one), Some((1, 1)), "das Beispielbild hat genau einen Bildpunkt (IHDR)");
        assert_eq!(extent(one), (9525, 9525), "ein Bildpunkt bei 96 dpi sind 9525 EMU");
        let gif = b"GIF89a\x40\x06\x20\x03rest";
        assert_eq!(pixels(gif), Some((1600, 800)), "GIF nennt Breite und Höhe als 16 Bit, niederwertiges Byte zuerst");
        let (cx, cy) = extent(gif);
        assert_eq!(cx, u64::from(TEXT_WIDTH_TWIPS) * 635, "ein zu breites Bild wird auf den Satzspiegel verkleinert");
        assert_eq!(cy, cx / 2, "das Seitenverhältnis 2:1 bleibt");
        // SOI, APP0 (Länge 4), SOF0 mit Höhe 300 und Breite 200.
        let jpeg = [0xff, 0xd8, 0xff, 0xe0, 0, 4, 0, 0, 0xff, 0xc0, 0, 11, 8, 0x01, 0x2c, 0, 0xc8, 3];
        assert_eq!(pixels(&jpeg), Some((200, 300)), "JPEG: der Bildkopf steht hinter anderen Segmenten");
        assert_eq!(extent(b"kein Bild"), FALLBACK_EMU, "ohne lesbaren Kopf gilt eine feste Größe statt null");
    }

    #[test]
    fn extension_follows_the_media_type_before_the_file_name() {
        assert_eq!(extension("image/jpeg", "haus.png"), "jpeg", "der Inhalt zählt, nicht die alte Endung");
        assert_eq!(extension("", "haus.PNG"), "png", "ohne Medientyp hilft die Endung");
        assert_eq!(extension("", "haus"), "bin", "ohne beides bleibt eine neutrale Endung");
        assert_eq!(content_type("jpeg"), "image/jpeg", "die Endung führt zurück zum Medientyp");
    }
}
