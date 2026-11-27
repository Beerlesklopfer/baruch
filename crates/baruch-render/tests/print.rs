// SPDX-License-Identifier: AGPL-3.0-or-later
//! Print check (contract Round 11, section 3): raster images below 300 dpi in their typeset size
//! are in `Rendered::print` or `Pdf::print`, not in `diagnostics`. Test images are created in the test
//! (invented pattern in the size 832 × 1216 pixels like a typical generated image).

use baruch_render::{MIN_DPI, Source, check_raster, compile, compile_pdf};

/// Small PNG (greyscale, unpacked deflate blocks) from a function per pixel.
fn png(width: u32, height: u32) -> Vec<u8> {
    fn crc32(bytes: &[u8]) -> u32 {
        let mut crc = 0xffff_ffffu32;
        for &b in bytes {
            crc ^= u32::from(b);
            for _ in 0..8 {
                crc = if crc & 1 == 1 { (crc >> 1) ^ 0xedb8_8320 } else { crc >> 1 };
            }
        }
        !crc
    }
    fn chunk(out: &mut Vec<u8>, kind: &[u8], data: &[u8]) {
        out.extend((data.len() as u32).to_be_bytes());
        let mut body = kind.to_vec();
        body.extend(data);
        out.extend(&body);
        out.extend(crc32(&body).to_be_bytes());
    }
    let mut raw = Vec::new();
    for y in 0..height {
        raw.push(0);
        raw.extend((0..width).map(|x| ((x * 7 + y * 3) % 256) as u8));
    }
    let mut z = vec![0x78, 0x01];
    let blocks: Vec<&[u8]> = raw.chunks(65_535).collect();
    for (i, block) in blocks.iter().enumerate() {
        z.push(u8::from(i + 1 == blocks.len()));
        let len = block.len() as u16;
        z.extend(len.to_le_bytes());
        z.extend((!len).to_le_bytes());
        z.extend(*block);
    }
    let (mut a, mut b) = (1u32, 0u32);
    for &byte in &raw {
        a = (a + u32::from(byte)) % 65_521;
        b = (b + a) % 65_521;
    }
    z.extend(((b << 16) | a).to_be_bytes());
    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut ihdr = Vec::new();
    ihdr.extend(width.to_be_bytes());
    ihdr.extend(height.to_be_bytes());
    ihdr.extend([8, 0, 0, 0, 0]);
    chunk(&mut out, b"IHDR", &ihdr);
    chunk(&mut out, b"IDAT", &z);
    chunk(&mut out, b"IEND", &[]);
    out
}

/// A5 without margin, the image fills the page.
fn full_page(path: &str, bytes: Vec<u8>) -> Source {
    let main = format!(
        "#set page(paper: \"a5\", margin: 0pt)\n#image(\"{path}\", width: 100%, height: 100%, fit: \"cover\")\n"
    );
    Source { main, files: vec![(path.to_owned(), bytes)] }
}

#[test]
fn a_generated_picture_on_a_full_a5_page_is_too_coarse_for_print() {
    let source = full_page("bilder/umschlag.png", png(832, 1216));
    let rendered = compile(&source);
    assert!(!rendered.has_errors(), "das Testwerk setzt fehlerfrei: {:?}", rendered.diagnostics);
    assert_eq!(rendered.print.len(), 1, "genau eine Warnung für das eine Bild: {:?}", rendered.print);
    let w = &rendered.print[0];
    assert_eq!(w.path, "bilder/umschlag.png", "die Warnung nennt den Pfad in Normalform");
    assert_eq!(w.page, 1, "das Bild steht auf Seite 1");
    assert_eq!((w.width_px, w.height_px), (832, 1216), "die Pixel des Bilds");
    assert!(w.dpi < MIN_DPI && w.dpi > 100.0, "832 Pixel auf knapp 15 cm sind etwa 140 dpi, nicht {}", w.dpi);
    assert!(
        w.width_mm >= 147.0 && w.height_mm >= 209.0,
        "das Bild bedeckt die ganze A5-Seite (148 × 210 mm): {} × {}",
        w.width_mm,
        w.height_mm
    );
    assert!(
        w.needed_width_px >= 1748 && w.needed_height_px >= 2480,
        "für 300 dpi auf A5 braucht es mindestens 1748 × 2480 Pixel: {} × {}",
        w.needed_width_px,
        w.needed_height_px
    );
    assert!(w.message.starts_with("Bild „umschlag.png“: "), "die Meldung nennt den Dateinamen: {}", w.message);
    assert!(w.message.contains("für 300 dpi braucht es"), "die Meldung sagt, was fehlt: {}", w.message);
    assert!(rendered.diagnostics.is_empty(), "die Druckprüfung steht nicht in diagnostics: {:?}", rendered.diagnostics);

    let pdf = compile_pdf(&source);
    assert_eq!(pdf.print, rendered.print, "PDF und Vorschau prüfen dasselbe");
}

#[test]
fn a_large_enough_picture_and_an_svg_give_no_warning() {
    // 2000 × 1400 pixels on 6 cm width: above 800 dpi.
    let main = "#set page(paper: \"a5\")\n#image(\"bilder/gross.png\", width: 6cm)\n#image(\"bilder/linie.svg\", width: 100%)\n";
    let svg =
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"4\" height=\"2\"><rect width=\"4\" height=\"2\"/></svg>";
    let source = Source {
        main: main.into(),
        files: vec![("bilder/gross.png".into(), png(2000, 1400)), ("bilder/linie.svg".into(), svg.as_bytes().to_vec())],
    };
    let rendered = compile(&source);
    assert!(!rendered.has_errors(), "fehlerfrei: {:?}", rendered.diagnostics);
    assert!(rendered.print.is_empty(), "genug Pixel und Vektorgrafik → keine Warnung: {:?}", rendered.print);
}

#[test]
fn a_small_picture_drawn_small_is_fine_and_scaled_up_is_not() {
    let small = |width: &str| Source {
        main: format!("#image(\"bilder/klein.png\", width: {width})\n"),
        files: vec![("bilder/klein.png".into(), png(120, 120))],
    };
    assert!(compile(&small("1cm")).print.is_empty(), "120 Pixel auf 1 cm sind über 300 dpi");
    let big = compile(&small("5cm"));
    assert_eq!(big.print.len(), 1, "120 Pixel auf 5 cm sind nur etwa 61 dpi");
    assert_eq!(big.print[0].needed_width_px, 591, "5 cm bei 300 dpi sind 591 Pixel (aufgerundet)");
}

#[test]
fn the_message_rounds_like_the_contract_example() {
    let w = check_raster("bilder/umschlag.png".into(), 1, 832, 1216, 187.0, 272.9).expect("unter 300 dpi");
    assert_eq!(
        w.message, "Bild „umschlag.png“: 113 dpi bei 18,7 × 27,3 cm – für 300 dpi braucht es 2209 × 3224 Pixel",
        "Meldung mit ganzen dpi, Zentimetern mit Komma und aufgerundeten Pixeln"
    );
    assert!(check_raster("x.png".into(), 1, 3000, 3000, 100.0, 100.0).is_none(), "über 300 dpi keine Warnung");
    assert!(check_raster("x.png".into(), 1, 10, 10, 0.0, 10.0).is_none(), "ohne Fläche keine Warnung");
}
