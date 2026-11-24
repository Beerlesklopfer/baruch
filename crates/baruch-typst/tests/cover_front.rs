// SPDX-License-Identifier: AGPL-3.0-or-later
//! Runde 21c (bilder): `front_source_with_rules` – nur die Umschlag-Vorderseite im Endformat als Vorlage des
//! EPUB-Covers. Rein als Text geprüft; gesetzt und gerastert wird im Server-Test `round21_bilder.rs`. Buchangaben frei
//! erfunden.

use baruch_typst::cover::{BookInfo, CoverRef, TitleArea, WrapOptions, front_source_with_rules, wrap_source};
use baruch_typst::cover_layout::{CoverGeometry, CoverLayout, default_blocks};

fn info() -> BookInfo {
    BookInfo {
        title: "Der Leuchtturm #1 [Probe]".into(),
        subtitle: "Ein Jahr am Kap".into(),
        author: "Kai Küste".into(),
        isbn: "978-3-16-148410-0".into(),
        blurb: "Klappentext, der vorn nicht stehen darf.".into(),
        ..BookInfo::default()
    }
}

fn opts(layout: Option<CoverLayout>, cover: bool) -> WrapOptions {
    WrapOptions {
        page_width_mm: 120.0,
        page_height_mm: 180.0,
        spine_mm: 9.0,
        bleed_mm: 3.0,
        title_area: TitleArea::Bottom,
        cover: cover.then(|| CoverRef { path: ".baruch/umschlag.png".into(), alt: "Meer".into() }),
        barcode: Some(".baruch/isbn.svg".into()),
        author_photo: None,
        layout,
    }
}

#[test]
fn the_front_page_has_the_trim_size_and_only_front_texts() {
    let source = front_source_with_rules(&info(), &opts(None, true), "");
    assert!(
        source.contains("#set page(width: 120.000mm, height: 180.000mm, margin: 0pt)"),
        "Endformat ohne Beschnitt und ohne Rücken: {source}"
    );
    assert!(source.contains("image(\".baruch/umschlag.png\""), "das Umschlagbild füllt die Seite: {source}");
    for text in ["\"Der Leuchtturm #1 [Probe]\"", "\"Ein Jahr am Kap\"", "\"Kai Küste\""] {
        assert!(source.contains(text), "Titel, Untertitel und Autor als String-Literale ({text}): {source}");
    }
    assert!(!source.contains("Klappentext") && !source.contains("isbn.svg"), "nichts von der Rückseite: {source}");
    assert!(source.contains("bottom + center"), "ohne Anordnung im gewählten Titelbereich wie im Buchsatz");
}

#[test]
fn with_a_layout_the_front_blocks_stand_at_their_place_from_the_trim_edge() {
    let geometry = CoverGeometry::new(120.0, 180.0, 0.0, 3.0);
    let mut blocks = default_blocks(TitleArea::Top, &geometry);
    for block in &mut blocks {
        if block.kind.as_str() == "front_title" {
            block.x_mm = 11.0;
            block.y_mm = 22.0;
        }
    }
    let source =
        front_source_with_rules(&info(), &opts(Some(CoverLayout { blocks }), false), "#set par(justify: true)\n");
    assert!(source.contains("dx: 11.000mm, dy: 22.000mm"), "Titelblock relativ zur Schnittkante: {source}");
    assert!(source.contains("// front_title") && source.contains("// front_author"), "Blöcke der Vorderseite");
    assert!(!source.contains("// spine_") && !source.contains("// back_"), "kein Rücken, keine Rückseite: {source}");
    assert!(source.contains("rgb(\"#1f2a33\")"), "ohne Umschlagbild die dunkle Fläche");
    assert!(source.contains("#set par(justify: true)\n"), "Textregeln der Werkvorlage nach der Sprache");
    let wrap = wrap_source(&info(), &opts(None, true));
    assert!(wrap.contains("Rückseite | Rücken | Vorderseite"), "der Druckerei-Umschlag bleibt unverändert");
}
