// SPDX-License-Identifier: AGPL-3.0-or-later
//! Umschlag und Titelei als Typst (Runde 11, Aufgabe cover): ISBN-13 mit Prüfziffer, Strichcode EAN-13,
//! Rückenbreite, Rahmen ohne Überschrift, Nutzertexte nur als String-Literale. Gesetzt wird in den
//! Server-Tests (`round11_cover.rs`), hier nur der erzeugte Quelltext.

use baruch_typst::cover::{
    BookInfo, CoverRef, EAN_QUIET_LEFT, EAN_QUIET_RIGHT, FrameOptions, TitleArea, WrapOptions, ean13_modules,
    ean13_svg, frame_parts, isbn13, spine_mm, typst_string, wrap_source,
};

/// 978-3-16-148410-0, unabhängig nach den EAN-Tabellen (L/G/R, Parität „LGGLGL“ für die 9) berechnet.
const BARS_9783161484100: &str =
    "10101110110001001010000100110010000101001100101010101110010010001011100110011011100101110010101";

fn info() -> BookInfo {
    BookInfo {
        title: "Das Gartenbuch".into(),
        subtitle: "Ein Jahr".into(),
        author: "Grete Gärtner".into(),
        isbn: "978-3-16-148410-0".into(),
        blurb: "Klappentext.".into(),
        ..BookInfo::default()
    }
}

#[test]
fn isbn13_accepts_valid_numbers_and_explains_wrong_ones() {
    for ok in ["978-3-16-148410-0", "9783161484100", "978 3 16 148410 0", "ISBN 978-3-16-148410-0"] {
        assert_eq!(isbn13(ok).as_deref(), Ok("9783161484100"), "gültig mit/ohne Bindestriche: {ok}");
    }
    let wrong = isbn13("978-3-16-148410-1").unwrap_err();
    assert!(wrong.contains("Prüfziffer") && wrong.contains("erwartet 0"), "falsche Prüfziffer benannt: {wrong}");
    assert!(isbn13("978-3-16-14841").unwrap_err().contains("13 Ziffern"), "zu kurz");
    assert!(isbn13("97831614841000").unwrap_err().contains("gefunden 14"), "zu lang");
    assert!(isbn13("3-16-148410-X").unwrap_err().contains("„X“"), "ISBN-10 mit X ist keine ISBN-13");
}

#[test]
fn the_barcode_has_95_modules_and_quiet_zones() {
    let modules = ean13_modules("9783161484100").unwrap();
    assert_eq!(modules.len(), 95, "EAN-13: 3 + 42 + 5 + 42 + 3 Module");
    assert_eq!(modules, BARS_9783161484100, "Balkenfolge der bekannten Nummer");
    assert_eq!(ean13_modules("123"), None, "keine 13 Ziffern → kein Strichcode");

    // Aus den <rect>-Balken der SVG wieder die Module lesen.
    let svg = ean13_svg("9783161484100");
    let total = EAN_QUIET_LEFT + 95 + EAN_QUIET_RIGHT;
    assert!(svg.contains(&format!("viewBox=\"0 0 {total} ")), "Breite = Ruhezonen + 95 Module");
    let mut read = vec![b'0'; total];
    for rect in svg.split("<rect ").skip(2) {
        let num = |name: &str| -> usize {
            let at = rect.find(&format!("{name}=\"")).unwrap() + name.len() + 2;
            rect[at..at + rect[at..].find('"').unwrap()].parse().unwrap()
        };
        let x = num("x");
        read[x..x + num("width")].fill(b'1');
    }
    let read = String::from_utf8(read).unwrap();
    assert_eq!(&read[..EAN_QUIET_LEFT], "0".repeat(EAN_QUIET_LEFT), "Ruhezone links leer");
    assert_eq!(&read[EAN_QUIET_LEFT + 95..], "0".repeat(EAN_QUIET_RIGHT), "Ruhezone rechts leer");
    assert_eq!(&read[EAN_QUIET_LEFT..EAN_QUIET_LEFT + 95], BARS_9783161484100, "SVG zeigt genau diese Balken");
    assert_eq!(ean13_svg("kaputt"), "", "ungültig → leer (der Server setzt dann den Platzhalter)");
}

#[test]
fn spine_is_pages_times_paper() {
    assert!((spine_mm(200, 0.1) - 20.0).abs() < 1e-9, "200 Seiten × 0,1 mm = 20 mm");
    assert_eq!(spine_mm(0, 0.1), 0.0, "ohne Seiten kein Rücken");
}

#[test]
fn nothing_is_added_without_cover_pages_or_front_matter() {
    let none = frame_parts(&info(), &FrameOptions::default());
    assert!(none.front.is_empty() && none.back.is_empty() && none.added_cover_pages == 0, "nichts gewünscht → nichts");
    let cover = Some(CoverRef { path: ".baruch/u.png".into(), alt: "Umschlag".into() });
    let off =
        frame_parts(&info(), &FrameOptions { cover: cover.clone(), cover_pages: false, ..FrameOptions::default() });
    assert!(off.front.is_empty() && off.back.is_empty(), "Umschlag ohne Umschlagseiten: Buchsatz bleibt");
    let on = frame_parts(&info(), &FrameOptions { cover, cover_pages: true, ..FrameOptions::default() });
    assert_eq!(on.added_cover_pages, 3, "Vorderseite, leere Innenseite (Parität) und Rückseite");
    assert!(on.back.contains("ISBN und Strichcode"), "ohne Strichcode-Datei ein Platzhalter");
}

#[test]
fn the_frame_has_no_heading_no_outline_and_only_string_literals() {
    let wild = "Ein #Test *fett* ] \"zitat\" $x$ \\ ende";
    let mut info = info();
    info.title = wild.into();
    info.dedication = "Für alle.".into();
    let parts = frame_parts(
        &info,
        &FrameOptions {
            cover: Some(CoverRef { path: ".baruch/u.svg".into(), alt: "Umschlag".into() }),
            cover_pages: true,
            front_matter: true,
            title_area: TitleArea::Middle,
            barcode: Some(".baruch/isbn.svg".into()),
            author_photo: None,
            ..FrameOptions::default()
        },
    );
    for part in [&parts.front, &parts.back] {
        assert!(!part.contains("heading") && !part.contains("outline"), "keine Überschrift, kein Verzeichnis:\n{part}");
        assert!(!part.lines().any(|l| l.starts_with('=')), "keine Markup-Überschrift");
    }
    assert!(parts.front.contains(&typst_string(wild)), "Titel nur als maskiertes String-Literal");
    assert!(!parts.front.contains(wild), "nie unmaskiert im Quelltext");
    assert!(parts.front.contains("horizon + center"), "Titelbereich Mitte");
    assert_eq!(
        parts.front.matches("#page(").count(),
        1 + 1 + 4 + 2,
        "Vorderseite, leere Innenseite, 4 Seiten Titelei, Widmung mit leerer Rückseite"
    );
    assert_eq!(parts.back.matches("#page(").count(), 1, "eine Rückseite");
}

#[test]
fn the_wrap_page_has_the_full_size_with_bleed() {
    let source = wrap_source(
        &info(),
        &WrapOptions {
            page_width_mm: 148.0,
            page_height_mm: 210.0,
            spine_mm: 12.5,
            bleed_mm: 3.0,
            title_area: TitleArea::Top,
            cover: None,
            barcode: None,
            author_photo: None,
            layout: None,
        },
    );
    assert!(
        source.contains("#set page(width: 314.500mm, height: 216.000mm, margin: 0pt)"),
        "2 × 148 + 12,5 + 2 × 3 breit, 210 + 6 hoch:\n{source}"
    );
    assert!(source.contains("rotate(90deg"), "ab 6 mm Rücken steht der Titel gedreht darauf");
    assert!(source.contains(&typst_string("Grete Gärtner · Das Gartenbuch")), "Rückentext als String-Literal");
}
