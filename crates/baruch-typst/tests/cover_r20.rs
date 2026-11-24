// SPDX-License-Identifier: AGPL-3.0-or-later
//! Buchangaben-Reste und Druckerei-Umschlag (Runde 20c, Besitz „epub-pruefung“): ISBN-Präfix 978/979 und Vorsatz in
//! jeder Schreibung, Text- und Absatzregeln der Werkvorlage für den Umschlag für die Druckerei und das Messen der
//! Blöcke – ohne solche Regeln byte-gleich zu `wrap_source`/`block_measure_source` (Goldstände R26/R27 in
//! `cover_layout.rs` prüfen diese weiter).

use baruch_typst::cover::{
    BlockRefs, BookInfo, IsbnError, TitleArea, WrapOptions, block_measure_source, block_measure_source_with_rules,
    isbn13, isbn13_check, template_text_rules, wrap_source, wrap_source_with_rules,
};
use baruch_typst::cover_layout::{Backdrop, CoverAlign, CoverBlock, CoverBlockKind, FontStyle};

fn info() -> BookInfo {
    BookInfo {
        title: "Das Teichbuch".into(),
        author: "Hanna Ufer".into(),
        blurb: "Ein Jahr am Teich, mit Fröschen, Libellen und viel Geduld.".into(),
        ..BookInfo::default()
    }
}

fn blurb() -> CoverBlock {
    CoverBlock {
        kind: CoverBlockKind::BackBlurb,
        x_mm: 10.0,
        y_mm: 10.0,
        width_mm: 80.0,
        align: CoverAlign::Left,
        size_pt: 10.0,
        leading_em: 0.65,
        color: "light".into(),
        font: None,
        style: FontStyle::Normal,
        backdrop: Backdrop::None,
        visible: true,
        direction: None,
        text: None,
    }
}

fn opts() -> WrapOptions {
    WrapOptions {
        page_width_mm: 120.0,
        page_height_mm: 190.0,
        spine_mm: 8.0,
        bleed_mm: 3.0,
        title_area: TitleArea::Top,
        cover: None,
        barcode: None,
        author_photo: None,
        layout: None,
    }
}

#[test]
fn an_isbn_must_start_with_978_or_979() {
    // 400… ist eine gültige EAN-13 (Prüfziffer stimmt), aber keine ISBN.
    assert_eq!(isbn13_check("4006381333931"), Err(IsbnError::Prefix(400)), "Präfix 400 hat eine eigene Fehlerart");
    let message = isbn13("4006381333931").unwrap_err();
    assert!(message.contains("978 oder 979") && message.contains("400"), "Meldung nennt beides: {message}");
    assert!(IsbnError::Prefix(400).message().contains("978 oder 979"), "kurze feste Meldung für den Store");
    assert_eq!(isbn13("979-10-90636-07-1").as_deref(), Ok("9791090636071"), "979 ist erlaubt");
}

#[test]
fn the_isbn_label_is_cut_off_in_any_spelling() {
    for written in [
        "isbn-13 978-3-16-148410-0",
        "ISBN-13: 978-3-16-148410-0",
        "Isbn 13 978-3-16-148410-0",
        "isbn13 978-3-16-148410-0",
        "isbn: 9783161484100",
        "ISBN 978-3-16-148410-0",
    ] {
        assert_eq!(
            isbn13(written).as_deref(),
            Ok("9783161484100"),
            "Vorsatz „{written}“ gehört nicht zu den Ziffern (früher blieb bei Kleinschrift „-13“ stehen)"
        );
    }
}

#[test]
fn text_rules_take_language_hyphenation_tracking_and_paragraph_rules_only() {
    let template = "#set page(width: 120mm, height: 190mm)\n\
                    #set text(font: \"Linux Libertine\", size: 11pt, lang: \"en\", hyphenate: true, tracking: 0.02em)\n\
                    #set par(justify: true, leading: 0.7em, first-line-indent: 1em)\n\
                    #let abstand = 0.5em\n\
                    #set par(spacing: abstand)\n\
                    // #set text(lang: \"fr\")\n\
                    /* #set par(justify: false) */\n  #set text(lang: \"it\")\n\
                    #set text(lang: \"es\") if false\n\
                    #set text(fill: red)\n";
    let rules = template_text_rules(template);
    assert_eq!(
        rules,
        "#set text(lang: \"en\", hyphenate: true, tracking: 0.02em)\n\
         #set par(justify: true, leading: 0.7em, first-line-indent: 1em)\n",
        "Schrift, Größe und Farbe bestimmt der Umschlag selbst; Variablen, Kommentare, eingerückte und bedingte \
         Regeln zählen nicht"
    );
}

#[test]
fn a_template_that_only_sets_german_gives_no_rules() {
    assert_eq!(
        template_text_rules("#set text(lang: \"de\")\n#set heading(numbering: \"1.\")\n"),
        "",
        "„de“ gilt ohnehin"
    );
    assert_eq!(template_text_rules(""), "", "leere Vorlage");
    assert_eq!(
        template_text_rules("#set text(lang: \"en\")\n#set text(lang: \"de\")\n"),
        "#set text(lang: \"en\")\n#set text(lang: \"de\")\n",
        "zurück auf Deutsch nach Englisch bleibt stehen (die letzte Regel gewinnt)"
    );
    assert_eq!(
        template_text_rules("#set text(costs: (widow: 100%, orphan: 100%))\n"),
        "#set text(costs: (widow: 100%, orphan: 100%))\n",
        "Wörterbücher mit Schlüsseln sind einfache Werte"
    );
}

#[test]
fn without_rules_the_sources_stay_byte_identical() {
    assert_eq!(wrap_source_with_rules(&info(), &opts(), ""), wrap_source(&info(), &opts()), "Umschlag unverändert");
    let block = blurb();
    assert_eq!(
        block_measure_source_with_rules(&info(), std::slice::from_ref(&block), &BlockRefs::default(), ""),
        block_measure_source(&info(), &[block], &BlockRefs::default()),
        "Messquelle unverändert"
    );
}

#[test]
fn rules_follow_the_german_default_in_both_sources() {
    let rules = "#set text(hyphenate: true)\n#set par(justify: true)\n";
    let wrap = wrap_source_with_rules(&info(), &opts(), rules);
    assert!(
        wrap.contains(&format!("#set text(lang: \"de\")\n{rules}")),
        "die Regeln der Vorlage stehen direkt hinter der Vorgabe „de“ und gelten so für alle Flächen:\n{wrap}"
    );
    let block = blurb();
    let measure = block_measure_source_with_rules(&info(), &[block], &BlockRefs::default(), rules);
    assert!(
        measure.contains(&format!("#set text(lang: \"de\")\n{rules}")),
        "gemessen wird mit denselben Regeln, sonst stimmen Höhe und Umbruch nicht überein:\n{measure}"
    );
}
