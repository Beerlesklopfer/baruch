// SPDX-License-Identifier: AGPL-3.0-or-later
//! Runde 27, Aufgabe umschlag-satz (R27b): Umschlag-Blöcke, Titelei aus der Werkvorlage und Schriften, gesetzt mit
//! dem eingebetteten Typst. Lage gemessen mit `pdftotext -bbox` (fehlt es, wird der Test mit Meldung übersprungen).
//! Alle Texte und Bilder sind erfunden.

use std::process::Command;

use baruch_render::{Severity, Source, compile, compile_pdf, font_families, has_font_family, page_sizes_pt};
use baruch_typst::cover::{
    BlockRefs, BookInfo, CoverRef, FrameOptions, TemplateFunctions, WrapOptions, block_measure_source, frame_parts,
    imprint_lines, imprint_template, template_functions, wrap_source,
};
use baruch_typst::cover_layout::{
    Backdrop, CoverAlign, CoverBlock, CoverBlockKind, CoverFace, CoverGeometry, CoverLayout, FontStyle, SpineDirection,
    default_blocks,
};

const MM_PER_PT: f64 = 25.4 / 72.0;

/// Erfundenes Umschlagbild (SVG, hell oben, dunkel unten).
const CLOUD: &str = "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"600\" height=\"900\">\
<rect width=\"600\" height=\"900\" fill=\"#9ec5e8\"/><rect y=\"600\" width=\"600\" height=\"300\" fill=\"#203040\"/></svg>\n";

fn info() -> BookInfo {
    BookInfo {
        title: "Wolkenhaus".into(),
        subtitle: "Geschichten".into(),
        author: "Mira Mondschein".into(),
        publisher: "Lampenverlag".into(),
        year: "2026".into(),
        blurb: "Ein Haus aus Wolken.".into(),
        ..BookInfo::default()
    }
}

fn block(kind: CoverBlockKind, x: f64, y: f64, width: f64, size: f64) -> CoverBlock {
    CoverBlock {
        kind,
        x_mm: x,
        y_mm: y,
        width_mm: width,
        align: CoverAlign::Left,
        size_pt: size,
        leading_em: 0.65,
        color: "light".into(),
        font: None,
        style: FontStyle::Normal,
        backdrop: Backdrop::None,
        visible: true,
        direction: (kind.face() == CoverFace::Spine).then_some(SpineDirection::Down),
        text: kind.own_text().then(String::new),
    }
}

fn errors(diagnostics: &[baruch_render::Diagnostic]) -> Vec<String> {
    diagnostics.iter().filter(|d| d.severity == Severity::Error).map(|d| d.message.clone()).collect()
}

/// Ein Wort aus `pdftotext -bbox`: Seite (1-basiert), Rahmen in pt, Text.
#[derive(Debug, Clone)]
struct Word {
    page: usize,
    x_min: f64,
    y_min: f64,
    x_max: f64,
    y_max: f64,
    text: String,
}

/// Wörter eines PDF mit Lage; `None`, wenn `pdftotext` fehlt.
fn words(pdf: &[u8], tag: &str) -> Option<Vec<Word>> {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(format!("{tag}.pdf"));
    std::fs::write(&path, pdf).unwrap();
    let out = Command::new("pdftotext").arg("-bbox").arg(&path).arg("-").output().ok()?;
    assert!(out.status.success(), "pdftotext scheitert: {}", String::from_utf8_lossy(&out.stderr));
    let html = String::from_utf8(out.stdout).unwrap();
    let mut page = 0;
    let mut list = Vec::new();
    for line in html.lines() {
        let line = line.trim();
        if line.starts_with("<page ") {
            page += 1;
        }
        let Some(rest) = line.strip_prefix("<word ") else { continue };
        let num = |name: &str| -> f64 {
            let at = rest.find(&format!("{name}=\"")).unwrap() + name.len() + 2;
            rest[at..at + rest[at..].find('"').unwrap()].parse().unwrap()
        };
        let text = rest[rest.find('>').unwrap() + 1..rest.find("</word>").unwrap()]
            .replace("&quot;", "\"")
            .replace("&apos;", "'")
            .replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&amp;", "&");
        list.push(Word { page, x_min: num("xMin"), y_min: num("yMin"), x_max: num("xMax"), y_max: num("yMax"), text });
    }
    Some(list)
}

fn find<'a>(list: &'a [Word], text: &str, page: Option<usize>) -> &'a Word {
    list.iter()
        .find(|w| w.text == text && page.is_none_or(|p| w.page == p))
        .unwrap_or_else(|| panic!("Wort {text:?} (Seite {page:?}) fehlt: {list:?}"))
}

fn book_source(front: &str, back: &str) -> Source {
    Source {
        main: format!(
            "#set page(width: 148mm, height: 210mm)\n#set text(lang: \"de\")\n{front}= Kapitel\n\nText im Buch.\n{back}"
        ),
        files: vec![(".baruch/wolke.svg".into(), CLOUD.as_bytes().to_vec())],
    }
}

fn cover_ref() -> Option<CoverRef> {
    Some(CoverRef { path: ".baruch/wolke.svg".into(), alt: "Wolke".into() })
}

#[test]
fn a_block_lies_at_the_same_place_from_the_trim_edge_in_cover_pdf_and_book_pdf() {
    let geometry = CoverGeometry::new(148.0, 210.0, 12.0, 3.0);
    let mut blocks = default_blocks(baruch_typst::cover::TitleArea::Top, &geometry);
    for b in &mut blocks {
        b.visible = false;
    }
    let title = block(CoverBlockKind::FrontTitle, 21.0, 33.0, 100.0, 20.0);
    let mut blurb = block(CoverBlockKind::BackBlurb, 17.0, 41.0, 90.0, 11.0);
    blurb.align = CoverAlign::Justify;
    for b in [title, blurb] {
        let at = blocks.iter().position(|x| x.kind == b.kind).unwrap();
        blocks[at] = b;
    }
    let layout = CoverLayout { blocks };

    let parts = frame_parts(
        &info(),
        &FrameOptions {
            cover: cover_ref(),
            cover_pages: true,
            layout: Some(layout.clone()),
            ..FrameOptions::default()
        },
    );
    let book = compile_pdf(&book_source(&parts.front, &parts.back));
    assert!(errors(&book.diagnostics).is_empty(), "Buch mit Anordnung setzt fehlerfrei: {:?}", book.diagnostics);
    let wrap = wrap_source(
        &info(),
        &WrapOptions {
            page_width_mm: 148.0,
            page_height_mm: 210.0,
            spine_mm: 12.0,
            bleed_mm: 3.0,
            title_area: baruch_typst::cover::TitleArea::Top,
            cover: cover_ref(),
            barcode: None,
            author_photo: None,
            layout: Some(layout),
        },
    );
    let cover = compile_pdf(&Source { main: wrap, files: book_source("", "").files });
    assert!(errors(&cover.diagnostics).is_empty(), "Umschlag setzt fehlerfrei: {:?}", cover.diagnostics);

    let (Some(book_words), Some(cover_words)) =
        (words(book.bytes.as_ref().unwrap(), "buch"), words(cover.bytes.as_ref().unwrap(), "umschlag"))
    else {
        eprintln!("ÜBERSPRUNGEN: pdftotext fehlt (Lage nicht gemessen)");
        return;
    };
    let pages = book.pages;
    for (word, face, book_page) in [("Wolkenhaus", CoverFace::Front, 1), ("Ein", CoverFace::Back, pages)] {
        let in_book = find(&book_words, word, Some(book_page));
        let in_cover = find(&cover_words, word, Some(1));
        let (ox, oy) = geometry.face_origin(face);
        let dx = (in_book.x_min * MM_PER_PT) - (in_cover.x_min * MM_PER_PT - ox);
        let dy = (in_book.y_min * MM_PER_PT) - (in_cover.y_min * MM_PER_PT - oy);
        assert!(
            dx.abs() <= 0.5 && dy.abs() <= 0.5,
            "{word} ({face:?}): gleiche Lage relativ zur Schnittkante in Buch und Umschlag erwartet, Abweichung \
             {dx:.3} mm / {dy:.3} mm ({in_book:?} / {in_cover:?})"
        );
    }
    // Und die Lage stimmt mit der Anordnung: Der Text beginnt am linken Blockrand (bis auf die Seitenpeilung der
    // Glyphe) und liegt knapp unter der Oberkante (Typst setzt ab der Versalhöhe).
    let title = find(&book_words, "Wolkenhaus", Some(1));
    assert!((title.x_min * MM_PER_PT - 21.0).abs() < 1.0, "Titel links bei 21 mm: {title:?}");
    assert!((title.y_min * MM_PER_PT - 33.0).abs() < 2.0, "Titel oben bei 33 mm: {title:?}");
}

#[test]
fn the_spine_turns_down_or_up_and_stays_inside_the_spine() {
    let geometry = CoverGeometry::new(148.0, 210.0, 20.0, 3.0);
    let mut measured = Vec::new();
    for direction in [SpineDirection::Down, SpineDirection::Up] {
        let mut title = block(CoverBlockKind::SpineTitle, 4.0, 30.0, 150.0, 14.0);
        title.direction = Some(direction);
        let mut info = info();
        info.title = "Wolkenhaus Dachboden".into();
        let wrap = wrap_source(
            &info,
            &WrapOptions {
                page_width_mm: 148.0,
                page_height_mm: 210.0,
                spine_mm: 20.0,
                bleed_mm: 3.0,
                title_area: baruch_typst::cover::TitleArea::Top,
                cover: None,
                barcode: None,
                author_photo: None,
                layout: Some(CoverLayout { blocks: vec![title] }),
            },
        );
        let angle = if direction == SpineDirection::Down { "rotate(90deg" } else { "rotate(-90deg" };
        assert!(wrap.contains(angle), "{direction:?}: Drehung {angle} im Quelltext:\n{wrap}");
        let pdf = compile_pdf(&Source { main: wrap, files: Vec::new() });
        assert!(errors(&pdf.diagnostics).is_empty(), "Rücken setzt fehlerfrei: {:?}", pdf.diagnostics);
        let Some(list) = words(pdf.bytes.as_ref().unwrap(), "ruecken") else {
            eprintln!("ÜBERSPRUNGEN: pdftotext fehlt (Rücken nicht gemessen)");
            return;
        };
        // Nur Wörter auf dem Rücken (die Vorgabeblöcke der Vorderseite tragen denselben Titel).
        let (ox, _) = geometry.face_origin(CoverFace::Spine);
        let on_spine: Vec<Word> = list
            .into_iter()
            .filter(|w| (w.x_min * MM_PER_PT - ox).abs() < 20.0 && w.x_max * MM_PER_PT < ox + 25.0)
            .collect();
        let first = find(&on_spine, "Wolkenhaus", None).clone();
        let second = find(&on_spine, "Dachboden", None).clone();
        measured.push((direction, first, second));
    }
    let (ox, _) = geometry.face_origin(CoverFace::Spine);
    for (direction, first, second) in &measured {
        for word in [first, second] {
            let (left, right) = (word.x_min * MM_PER_PT - ox, word.x_max * MM_PER_PT - ox);
            // pdftotext misst den Rahmen der Schrift samt Unterlänge, Typst den Block von der Versalhöhe bis zur
            // Grundlinie: Der Wortrahmen darf darum gut 1 mm über den Block (ab x = 4 mm) hinausragen.
            assert!(
                left >= 2.0 && right <= 12.0,
                "{direction:?}: Wort {} muss im Rücken nahe x = 4 mm liegen (Rücken 20 mm): {left:.2}–{right:.2} mm",
                word.text
            );
            assert!(
                word.y_max - word.y_min > word.x_max - word.x_min,
                "{direction:?}: gedrehtes Wort {} ist höher als breit: {word:?}",
                word.text
            );
        }
        match direction {
            SpineDirection::Down => assert!(
                first.y_min < second.y_min,
                "down: von oben nach unten lesbar – das erste Wort steht oben ({first:?}, {second:?})"
            ),
            SpineDirection::Up => assert!(
                first.y_min > second.y_min,
                "up: von unten nach oben lesbar – das erste Wort steht unten ({first:?}, {second:?})"
            ),
        }
    }
}

#[test]
fn block_heights_are_measured_with_typst_one_page_per_block() {
    let one = block(CoverBlockKind::FrontTitle, 0.0, 0.0, 100.0, 20.0);
    let mut free = block(CoverBlockKind::FrontFree, 0.0, 0.0, 100.0, 20.0);
    free.text = Some("Erste Zeile\n\nZweiter Absatz".into());
    let mut narrow = block(CoverBlockKind::FrontFree, 0.0, 0.0, 40.0, 20.0);
    narrow.text = Some("Wolkenhaus Dachboden".into());
    let mut wide = narrow.clone();
    wide.width_mm = 150.0;
    let empty = block(CoverBlockKind::FrontSubtitle, 0.0, 0.0, 100.0, 20.0);
    let mut info = info();
    info.subtitle.clear();
    let source = block_measure_source(&info, &[one, free, narrow, empty, wide], &BlockRefs::default());
    let rendered = compile(&Source { main: source, files: Vec::new() });
    assert!(errors(&rendered.diagnostics).is_empty(), "Messquelle setzt fehlerfrei: {:?}", rendered.diagnostics);
    let heights: Vec<f64> = page_sizes_pt(&rendered).into_iter().map(|(_, h)| h * MM_PER_PT).collect();
    assert_eq!(heights.len(), 5, "eine Seite je Block");
    assert!(heights[0] > 3.0 && heights[0] < 8.0, "eine Zeile 20 pt ≈ Versalhöhe (rund 5 mm): {heights:?}");
    assert!(heights[1] > 2.0 * heights[0], "zwei Absätze sind mehr als doppelt so hoch wie eine Zeile: {heights:?}");
    assert!(
        heights[2] > 1.5 * heights[4],
        "zu schmal → Umbruch in zwei Zeilen, also höher als breit gesetzt: {heights:?}"
    );
    assert!(
        heights[3] < 0.5,
        "Block ohne Inhalt (kein Untertitel) ist (fast) 0 hoch – Typst gibt der Seite 1 pt: {heights:?}"
    );
}

#[test]
fn font_families_name_embedded_and_system_fonts_with_their_styles() {
    let families = font_families();
    let libertinus = families
        .iter()
        .find(|f| f.name == "Libertinus Serif")
        .unwrap_or_else(|| panic!("die eingebettete Libertinus Serif fehlt: {families:?}"));
    assert!(libertinus.embedded, "Libertinus Serif kommt mit Baruch (eingebettet)");
    assert_eq!(libertinus.styles, ["normal", "bold", "italic"], "Libertinus hat aufrecht, fett und kursiv");
    let mut names: Vec<String> = families.iter().map(|f| f.name.to_lowercase()).collect();
    let count = names.len();
    names.dedup();
    assert_eq!(names.len(), count, "sortiert und ohne Doppel (ohne Unterschied der Schreibung)");
    assert!(families.iter().all(|f| !f.styles.is_empty()), "jede Familie hat mindestens einen Schnitt");
    match families.iter().find(|f| !f.embedded) {
        Some(system) => assert!(!system.styles.is_empty(), "Systemschrift mit Schnitten: {system:?}"),
        None => eprintln!("Hinweis: keine Systemschriften gefunden; geprüft nur die eingebetteten"),
    }
    assert!(has_font_family("libertinus serif"), "Vergleich ohne Groß-/Kleinschreibung");
    assert!(!has_font_family("Gibt Es Nicht Grotesk"), "erfundene Schrift fehlt");
}

#[test]
fn hostile_texts_and_font_names_are_set_literally_and_the_dictionary_holds_only_strings() {
    let nasty = "A \" B \\ C # D ] E ) F $ G // H /* I */ J\nK";
    let info = BookInfo {
        title: nasty.into(),
        subtitle: nasty.into(),
        author: nasty.into(),
        publisher: nasty.into(),
        price: nasty.into(),
        blurb: nasty.into(),
        bio: nasty.into(),
        year: nasty.into(),
        publisher_address: nasty.into(),
        printer_address: nasty.into(),
        printer: nasty.into(),
        dedication: nasty.into(),
        ..BookInfo::default()
    };
    let geometry = CoverGeometry::new(148.0, 210.0, 15.0, 3.0);
    let mut blocks = default_blocks(baruch_typst::cover::TitleArea::Middle, &geometry);
    for b in &mut blocks {
        b.visible = true;
        b.font = Some("Name\") #panic(\"x".into());
        if b.kind.own_text() {
            b.text = Some(nasty.into());
        }
    }
    let layout = CoverLayout { blocks };
    let template = "#set text(lang: \"de\")\n#let impressum(buch) = { for z in buch.impressum_zeilen { par(z) } }\n\
                    #let widmung(buch) = text(buch.widmung)\n";
    let parts = frame_parts(
        &info,
        &FrameOptions {
            cover: cover_ref(),
            cover_pages: true,
            front_matter: true,
            layout: Some(layout.clone()),
            template_functions: template_functions(template),
            ..FrameOptions::default()
        },
    );
    let book = compile_pdf(&Source {
        main: format!("{template}{}= Kapitel\n\nText.\n{}", parts.front, parts.back),
        files: book_source("", "").files,
    });
    assert!(
        errors(&book.diagnostics).is_empty(),
        "Buch mit feindlichen Texten setzt fehlerfrei: {:?}",
        book.diagnostics
    );
    let wrap = wrap_source(
        &info,
        &WrapOptions {
            page_width_mm: 148.0,
            page_height_mm: 210.0,
            spine_mm: 15.0,
            bleed_mm: 3.0,
            title_area: baruch_typst::cover::TitleArea::Middle,
            cover: cover_ref(),
            barcode: None,
            author_photo: None,
            layout: Some(layout),
        },
    );
    let cover = compile_pdf(&Source { main: wrap, files: book_source("", "").files });
    assert!(
        errors(&cover.diagnostics).is_empty(),
        "Umschlag mit feindlichen Texten setzt fehlerfrei: {:?}",
        cover.diagnostics
    );
    let Some(list) = words(book.bytes.as_ref().unwrap(), "feindlich") else {
        eprintln!("ÜBERSPRUNGEN: pdftotext fehlt (Wortlaut nicht gelesen)");
        return;
    };
    for token in ["\"", "\\", "#", "]", ")", "$", "//", "/*", "*/"] {
        assert!(list.iter().any(|w| w.text.contains(token)), "Zeichen {token:?} erscheint wörtlich im Buch-PDF");
    }
    // Das Impressum aus der Vorlagenfunktion zeigt den Text der Anschrift wörtlich.
    let all: Vec<&str> = list.iter().map(|w| w.text.as_str()).collect();
    assert!(all.windows(2).any(|p| p == ["/*", "I"]), "/* I */ wörtlich in einer Zeile: {all:?}");
}

#[test]
fn imprint_template_gives_the_same_imprint_lines_as_the_built_in_page() {
    let info = BookInfo {
        title: "Wolkenhaus".into(),
        author: "Mira Mondschein".into(),
        publisher: "Lampenverlag".into(),
        publisher_address: "Lampenweg 3\n12345 Kieselbach".into(),
        place: "Kieselbach".into(),
        year: "2026".into(),
        edition: "1. Auflage".into(),
        isbn: "978-3-16-148410-0".into(),
        printer: "Druckhaus Feder".into(),
        printer_address: "Federgasse 9, 54321 Tinte".into(),
        bible: "Erfundene Übersetzung".into(),
        bible_notice: "Mit Genehmigung.".into(),
        dedication: "Für Ilse.".into(),
        ..BookInfo::default()
    };
    let functions = template_functions(imprint_template());
    assert_eq!(
        functions,
        TemplateFunctions { half_title: true, main_title: true, imprint: true, dedication: true },
        "die Impressum-Vorlage definiert alle vier Funktionen"
    );
    let set = |template: &str, functions: TemplateFunctions| {
        let parts = frame_parts(
            &info,
            &FrameOptions { front_matter: true, template_functions: functions, ..FrameOptions::default() },
        );
        let pdf = compile_pdf(&Source {
            main: format!("#set page(width: 148mm, height: 210mm)\n{template}\n{}= Kapitel\n\nText.\n", parts.front),
            files: Vec::new(),
        });
        assert!(errors(&pdf.diagnostics).is_empty(), "Titelei setzt fehlerfrei: {:?}", pdf.diagnostics);
        pdf
    };
    let built_in = set("", TemplateFunctions::default());
    let from_template = set(imprint_template(), functions);
    assert_eq!(built_in.pages, from_template.pages, "gleiche Seitenzahl: die Seiten stehen an fester Stelle");
    let (Some(a), Some(b)) =
        (words(built_in.bytes.as_ref().unwrap(), "eingebaut"), words(from_template.bytes.as_ref().unwrap(), "vorlage"))
    else {
        eprintln!("ÜBERSPRUNGEN: pdftotext fehlt (Impressumszeilen nicht verglichen)");
        return;
    };
    let text_of = |list: &[Word], page: usize| -> String {
        list.iter().filter(|w| w.page == page).map(|w| w.text.as_str()).collect::<Vec<_>>().join(" ")
    };
    // Seite 4 = Rückseite des Haupttitels (Schmutztitel, Seite 2, Haupttitel, Impressum).
    let expected = imprint_lines(&info).join(" ");
    assert_eq!(text_of(&a, 4), expected, "eingebautes Impressum auf Seite 4 mit allen Zeilen");
    assert_eq!(text_of(&b, 4), expected, "Impressum aus der Vorlage: dieselben Zeilen auf derselben Seite");
    for page in 1..=built_in.pages {
        assert_eq!(
            text_of(&a, page),
            text_of(&b, page),
            "Seite {page}: Vorlage setzt denselben Text wie die eingebaute Seite"
        );
    }
    assert!(expected.contains("Lampenweg 3, 12345 Kieselbach"), "Anschrift des Verlags im Impressum: {expected}");
    assert!(
        expected.contains("Druck: Druckhaus Feder, Federgasse 9"),
        "Anschrift der Druckerei im Impressum: {expected}"
    );
}
