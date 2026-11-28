// SPDX-License-Identifier: AGPL-3.0-or-later
//! Freely invented test work „Das Gartenhaus am Bach“ (ownership "epub"; no third-party text).
//!
//! Minimum contents according to contract R20-S1 (may be extended, not removed): one part, two chapters with one
//! section of level 2 each, a footnote, a cross-reference `#link(<kN>)[…]`, one PNG with and one without
//! alternative text (generated in code), a cover PNG, an ISBN with a valid check digit, language "de".

use baruch_typst::typeset::TypesetSettings;

use crate::{EpubImage, EpubInput, EpubMeta, EpubSection, FootnoteMode};

/// Keys of the sections in the test work (part, chapter 1, section 1.1, chapter 2, section 2.1).
pub const KEYS: [i64; 5] = [10, 11, 12, 13, 14];

/// The test work as input for [`crate::build`].
pub fn input() -> EpubInput {
    let [part, ch1, s11, ch2, s21] = KEYS;
    let sections = vec![
        EpubSection {
            key: part,
            level: 0,
            number: None,
            title: "Planen".into(),
            body: format!(
                "#heading(level: 1, numbering: none)[Planen] <baruch-part>\n{}\n",
                baruch_typst::xref::anchor_line(part)
            ),
        },
        EpubSection {
            key: ch1,
            level: 1,
            number: Some("1".into()),
            title: "Der Platz".into(),
            body: format!(
                "= Der Platz <k{ch1}>\n\nAm Bach steht eine alte Weide. Unter ihr soll das Gartenhaus stehen, \
                 nahe genug am Wasser, aber sicher vor dem Hochwasser im Frühjahr.#footnote[Der Bach trat zuletzt \
                 vor zwölf Jahren über die Ufer.]\n\n#figure(image(\"bilder/weide.png\", alt: \"Skizze der Weide am \
                 Bach\"), caption: [Die Weide])\n"
            ),
        },
        EpubSection {
            key: s11,
            level: 2,
            number: Some("1.1".into()),
            title: "Boden und Licht".into(),
            body: format!(
                "== Boden und Licht <k{s11}>\n\nDer Boden ist lehmig und hält das Wasser lange. Morgens fällt \
                 Licht durch die Zweige, nachmittags liegt der Platz im Schatten.\n"
            ),
        },
        EpubSection {
            key: ch2,
            level: 1,
            number: Some("2".into()),
            title: "Das Fundament".into(),
            body: format!(
                "= Das Fundament <k{ch2}>\n\nWie in #link(<k{s11}>)[Boden und Licht] beschrieben, trägt der \
                 Lehm schlecht. Darum ruht das Haus auf sechs Punktfundamenten.\n\n#image(\"bilder/plan.png\")\n"
            ),
        },
        EpubSection {
            key: s21,
            level: 2,
            number: Some("2.1".into()),
            title: "Beton & Kies".into(),
            body: format!(
                "== Beton & Kies <k{s21}>\n\nUnter jeden Pfeiler kommt eine Handbreit Kies, darauf der \
                 Beton.\n"
            ),
        },
    ];
    EpubInput {
        meta: EpubMeta {
            identifier: "urn:uuid:3f6c2a1e-8b4d-4c7a-9e21-5d0f7a9b1c42".into(),
            title: "Das Gartenhaus am Bach".into(),
            subtitle: "Ein Bautagebuch".into(),
            author: "Erika Beispiel".into(),
            language: "de".into(),
            isbn: Some("9783161484100".into()),
            publisher: "Beispielverlag".into(),
            year: "2026".into(),
            rights: "© 2026 Erika Beispiel".into(),
            image_credit: "Zeichnungen: Erika Beispiel".into(),
            description: "Wie aus einer Weide am Bach ein Gartenhaus wurde.".into(),
            modified: "2026-11-09T12:00:00Z".into(),
        },
        template: "#set text(lang: \"de\")\n#set heading(numbering: \"1.1\")\n".into(),
        sections,
        typeset: TypesetSettings::default(),
        back_matter: Default::default(),
        images: vec![
            EpubImage {
                path: "bilder/weide.png".into(),
                media_type: "image/png".into(),
                bytes: png(4, 3, [40, 120, 60]),
                alt: "Skizze der Weide am Bach".into(),
            },
            EpubImage {
                path: "bilder/plan.png".into(),
                media_type: "image/png".into(),
                bytes: png(3, 3, [90, 90, 160]),
                alt: String::new(),
            },
        ],
        cover: Some(EpubImage {
            path: "bilder/umschlag.png".into(),
            media_type: "image/png".into(),
            bytes: png(6, 9, [200, 160, 60]),
            alt: "Umschlag: Das Gartenhaus am Bach".into(),
        }),
        front_matter: true,
        imprint: vec!["© 2026 Erika Beispiel".into(), "Beispielverlag".into(), "ISBN 978-3-16-148410-0".into()],
        dedication: "Für alle, die am Bach bauen.".into(),
        footnotes: FootnoteMode::Popup,
        work_language: Default::default(),
    }
}

/// Single-colour RGB PNG `width` × `height` (uncompressed DEFLATE blocks; no image crate needed).
pub fn png(width: u32, height: u32, rgb: [u8; 3]) -> Vec<u8> {
    png_with(width, height, |_, _| rgb)
}

/// RGB PNG `width` × `height` with the colour `pixel(x, y)` per pixel (e.g. noise for large test files that
/// cannot be compressed).
pub fn png_with(width: u32, height: u32, pixel: impl Fn(u32, u32) -> [u8; 3]) -> Vec<u8> {
    let mut raw = Vec::new();
    for y in 0..height {
        raw.push(0); // filter "None"
        for x in 0..width {
            raw.extend_from_slice(&pixel(x, y));
        }
    }
    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]); // 8 bit, RGB, deflate, default filter, no interlacing
    chunk(&mut out, b"IHDR", &ihdr);
    chunk(&mut out, b"IDAT", &zlib_stored(&raw));
    chunk(&mut out, b"IEND", &[]);
    out
}

fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    let start = out.len();
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    let crc = crc32(&out[start..]);
    out.extend_from_slice(&crc.to_be_bytes());
}

fn zlib_stored(data: &[u8]) -> Vec<u8> {
    let mut out = vec![0x78, 0x01];
    let blocks: Vec<&[u8]> = if data.is_empty() { vec![&[]] } else { data.chunks(65_535).collect() };
    for (i, block) in blocks.iter().enumerate() {
        out.push(u8::from(i + 1 == blocks.len()));
        let len = block.len() as u16;
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(&(!len).to_le_bytes());
        out.extend_from_slice(block);
    }
    let (mut a, mut b) = (1u32, 0u32);
    for byte in data {
        a = (a + u32::from(*byte)) % 65_521;
        b = (b + a) % 65_521;
    }
    out.extend_from_slice(&((b << 16) | a).to_be_bytes());
    out
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for byte in data {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = if crc & 1 == 1 { (crc >> 1) ^ 0xEDB8_8320 } else { crc >> 1 };
        }
    }
    !crc
}
