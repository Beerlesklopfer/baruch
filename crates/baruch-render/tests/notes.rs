// SPDX-License-Identifier: AGPL-3.0-or-later
//! Cue sheet (Round 16c): `baruch_render::notes::notes_source` builds a Typst source that
//! `compile`/`compile_pdf` typeset without errors – A5 portrait (16 pt) or cards A6 landscape (14 pt, at least
//! one card per section), texts with markup characters verbatim. Sermon texts freely invented.

use baruch_render::notes::{MAX_FONT_PT, MIN_FONT_PT, NotesCard, NotesDoc, NotesFormat, notes_source, typst_string};
use baruch_render::{Severity, Source, compile, compile_pdf};

/// Text that, read as markup, would give errors or different typesetting.
const TRICKY: &str = "Gefahr #let x = 1 [Klammer] *fett* $a^2$ \\ Rückstrich <label> @verweis \"zitiert\"";

fn card(number: &str, title: &str, points: &[&str], refs: &[&str]) -> NotesCard {
    NotesCard {
        number: number.into(),
        title: title.into(),
        depth: 0,
        start_clock: "0:00".into(),
        clock: "1:10".into(),
        points: points.iter().map(|p| (*p).to_owned()).collect(),
        refs: refs.iter().map(|r| (*r).to_owned()).collect(),
    }
}

fn sermon() -> NotesDoc {
    NotesDoc {
        title: "Brot am Wegrand".into(),
        total_clock: "4:30".into(),
        target_clock: Some("20:00".into()),
        cards: vec![
            card("", "Einleitung", &["Ein Bäcker steht um vier Uhr auf", "Wer hat heute schon gegessen?"], &[]),
            card("I.", "Das Brot", &["Brot ist Gabe", TRICKY], &["Joh 6,35", "Mt 6,11"]),
            card("II.", "Der Weg", &["Unterwegs teilen"], &["Lk 24,30"]),
            card("", "Schluss", &[], &[]),
        ],
        ..NotesDoc::default()
    }
}

/// Page size (width, height in pt) from the `viewBox` of the first SVG page.
fn page_size(svg: &str) -> (f64, f64) {
    let at = svg.find("viewBox=\"").expect("SVG mit viewBox") + "viewBox=\"".len();
    let numbers: Vec<f64> =
        svg[at..svg[at..].find('"').unwrap() + at].split_whitespace().map(|n| n.parse().unwrap()).collect();
    (numbers[2], numbers[3])
}

fn errors(rendered: &baruch_render::Rendered) -> Vec<String> {
    rendered.diagnostics.iter().filter(|d| d.severity == Severity::Error).map(|d| d.message.clone()).collect()
}

#[test]
fn the_a5_sheet_is_portrait_a5_with_16_pt_and_sets_without_errors() {
    let source = notes_source(&sermon(), NotesFormat::A5, NotesFormat::A5.default_font_pt());
    assert!(source.contains("paper: \"a5\""), "A5 hoch verlangt: {source}");
    assert!(source.contains("#set text(size: 16pt"), "Vorgabe 16 pt für A5: {source}");
    let rendered = compile(&Source { main: source.clone(), files: vec![] });
    assert!(errors(&rendered).is_empty(), "der Zettel muss ohne Fehler setzen: {:?}\n{source}", errors(&rendered));
    let (width, height) = page_size(&rendered.pages[0]);
    assert!(
        (width - 419.5).abs() < 1.0 && (height - 595.3).abs() < 1.0,
        "A5 hoch ist 148 × 210 mm: {width} × {height}"
    );
    assert!(
        (1..=3).contains(&rendered.pages.len()),
        "vier kurze Abschnitte passen auf eine bis drei A5-Seiten: {}",
        rendered.pages.len()
    );
}

#[test]
fn cards_are_landscape_a6_with_at_least_one_card_per_section() {
    let doc = sermon();
    let source = notes_source(&doc, NotesFormat::Cards, NotesFormat::Cards.default_font_pt());
    assert!(source.contains("#set text(size: 14pt"), "Vorgabe 14 pt für Karten: {source}");
    let rendered = compile(&Source { main: source.clone(), files: vec![] });
    assert!(errors(&rendered).is_empty(), "Karten setzen ohne Fehler: {:?}", errors(&rendered));
    let (width, height) = page_size(&rendered.pages[0]);
    assert!(
        (width - 419.5).abs() < 1.0 && (height - 297.6).abs() < 1.0,
        "A6 quer ist 148 × 105 mm: {width} × {height}"
    );
    assert!(
        rendered.pages.len() >= doc.cards.len(),
        "je Abschnitt mindestens eine Karte: {} Seiten für {} Abschnitte",
        rendered.pages.len(),
        doc.cards.len()
    );
    assert!(rendered.pages.len() <= doc.cards.len() + 2, "kurze Abschnitte brauchen kaum Folgekarten");
}

#[test]
fn texts_with_markup_characters_appear_literally() {
    let source = notes_source(&sermon(), NotesFormat::A5, 16);
    assert!(source.contains(&typst_string(TRICKY)), "der Text steht nur als Zeichenkette in der Quelle");
    // Ask Typst itself which texts are in the document: the key point must arrive character by character.
    let expected = TRICKY.replace('\\', "\\\\").replace('"', "\\\"");
    let check = format!(
        "{source}\n#context {{ let texts = query(<baruch-note>).map(m => m.value); \
         assert(texts.contains(\"{expected}\"), message: \"Stichpunkt nicht wörtlich: \" + repr(texts)) }}\n"
    );
    let rendered = compile(&Source { main: check, files: vec![] });
    assert!(
        errors(&rendered).is_empty(),
        "Typst findet den Text wörtlich (sonst schlägt assert an): {:?}",
        errors(&rendered)
    );
}

#[test]
fn the_pdf_starts_with_the_pdf_magic_and_has_plausible_pages() {
    let doc = sermon();
    for format in NotesFormat::ALL {
        let pdf = compile_pdf(&Source { main: notes_source(&doc, format, 20), files: vec![] });
        assert!(!pdf.has_errors(), "{}: PDF ohne Fehler: {:?}", format.label(), pdf.diagnostics);
        let bytes = pdf.bytes.expect("Bytes ohne Fehler");
        assert!(bytes.starts_with(b"%PDF"), "{}: echte PDF-Datei", format.label());
        let minimum = if format == NotesFormat::Cards { doc.cards.len() } else { 1 };
        assert!((minimum..=12).contains(&pdf.pages), "{}: Seitenzahl plausibel ({} Seiten)", format.label(), pdf.pages);
    }
}

#[test]
fn font_sizes_are_clamped_to_14_to_28_pt() {
    let small = notes_source(&sermon(), NotesFormat::A5, 6);
    assert!(small.contains(&format!("size: {MIN_FONT_PT}pt")), "unter 14 pt wird auf 14 pt angehoben");
    let big = notes_source(&sermon(), NotesFormat::A5, 99);
    assert!(big.contains(&format!("size: {MAX_FONT_PT}pt")), "über 28 pt wird auf 28 pt begrenzt");
    let rendered = compile(&Source { main: big, files: vec![] });
    assert!(errors(&rendered).is_empty(), "auch 28 pt setzen ohne Fehler: {:?}", errors(&rendered));
}

#[test]
fn larger_print_needs_at_least_as_many_pages() {
    let mut doc = sermon();
    for card in &mut doc.cards {
        card.points = (1..=6).map(|i| format!("Stichpunkt {i} mit einigen Worten mehr, damit er umbricht")).collect();
    }
    let pages = |pt| compile(&Source { main: notes_source(&doc, NotesFormat::A5, pt), files: vec![] }).pages.len();
    let (small, big) = (pages(14), pages(28));
    assert!(big >= small, "größere Schrift braucht nicht weniger Seiten: {small} bei 14 pt, {big} bei 28 pt");
    assert!(big > 1, "sechs Punkte je Abschnitt in 28 pt passen nicht auf eine A5-Seite: {big}");
}

#[test]
fn an_empty_outline_still_gives_a_page() {
    let doc = NotesDoc {
        title: "Leer".into(),
        total_clock: "0:00".into(),
        target_clock: None,
        cards: vec![],
        ..NotesDoc::default()
    };
    let rendered = compile(&Source { main: notes_source(&doc, NotesFormat::Cards, 14), files: vec![] });
    assert!(errors(&rendered).is_empty(), "ohne Abschnitte kein Fehler: {:?}", errors(&rendered));
    assert_eq!(rendered.pages.len(), 1, "eine Karte mit dem Hinweis „Keine Abschnitte.“");
}
