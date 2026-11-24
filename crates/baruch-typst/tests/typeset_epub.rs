// SPDX-License-Identifier: AGPL-3.0-or-later
//! Runde 29b: EPUB-Abbildung der Satz-Einstellungen (`epub_style`, Vertrag §5.6); seit Runde 20a verwendet sie der
//! EPUB-Export (`baruch-epub`). Geprüft wird, dass jede abbildbare Einstellung eine erkennbare CSS-Regel ergibt und
//! dass Kapitel- und Teil-Dateien immer eine Umbruchregel haben.

use baruch_typst::typeset::*;

/// Runde 20a: Vorher ergab „keine Einstellung“ nur den Kopfkommentar. Seit Kapitel und Teile immer eigene Dateien
/// sind (Vertrag R20 §0.2), steht die Umbruchregel auch ohne Einstellung da – sonst hinge der Seitenanfang allein
/// vom Leser ab.
#[test]
fn empty_settings_give_only_the_break_rules_for_chapter_and_part_files() {
    let style = epub_style(&TypesetSettings::default());
    assert_eq!(
        style.css,
        "/* Baruch: Satz (EPUB) */\n\
         section.baruch-chapter { break-before: page; page-break-before: always; }\n\
         section.baruch-part { break-before: page; page-break-before: always; }\n",
        "ohne Einstellungen nur die Umbrüche vor Kapitel- und Teil-Dateien"
    );
    assert!(style.split_chapters, "je Kapitel eine Datei, immer");
    assert_eq!(style.lang, None, "keine Sprache gesetzt");
}

#[test]
fn continuous_chapters_keep_their_rule_but_do_not_force_a_page() {
    let s = TypesetSettings { chapter_start: Some(ChapterStart::Continuous), ..Default::default() };
    let style = epub_style(&s);
    assert!(
        style.css.contains("section.baruch-chapter { break-before: auto; page-break-before: auto; }"),
        "fortlaufend: Regel da, aber ohne erzwungenen Umbruch (die Datei bleibt trotzdem eigen): {}",
        style.css
    );
    assert!(
        style.css.contains("section.baruch-part { break-before: page;"),
        "Teile ohne Einstellung weiter auf neuer Seite: {}",
        style.css
    );
    assert!(style.split_chapters, "Dateiaufteilung auch bei fortlaufenden Kapiteln");
}

#[test]
fn a_right_page_start_becomes_break_before_right() {
    let s = TypesetSettings { chapter_start: Some(ChapterStart::RightPage), ..Default::default() };
    let style = epub_style(&s);
    assert!(
        style.css.contains("section.baruch-chapter { break-before: right; page-break-before: always; }"),
        "rechte Seite: nur Leser mit Doppelseiten beachten `right`, alle anderen brechen um: {}",
        style.css
    );
}

#[test]
fn breaks_paragraphs_and_hyphenation_are_mapped() {
    let s = TypesetSettings {
        chapter_start: Some(ChapterStart::NewPage),
        part_page: Some(PartPage { side: PartSide::Right, blank_back: true }),
        paragraph: Some(Paragraph::Indent { em: 1.5 }),
        justify: Some(true),
        hyphenate: Some(true),
        lang: Some("de-CH".into()),
        font_size_pt: Some(12.1),
        ..Default::default()
    };
    let style = epub_style(&s);
    assert!(
        style.css.contains("section.baruch-chapter { break-before: page; page-break-before: always; }"),
        "Kapitel auf neuer Seite: {}",
        style.css
    );
    assert!(style.css.contains("section.baruch-part { break-before: right;"), "Teil rechts: {}", style.css);
    assert!(style.css.contains("break-after: page"), "leere Rückseite der Teil-Seite: {}", style.css);
    assert!(style.css.contains("text-indent: 1.5em"), "Einzug 1,5 em: {}", style.css);
    assert!(style.css.contains("text-align: justify"), "Blocksatz: {}", style.css);
    assert!(style.css.contains("hyphens: auto"), "Silbentrennung: {}", style.css);
    assert!(style.css.contains("font-size: 1.1em"), "12,1 pt relativ zu 11 pt: {}", style.css);
    assert_eq!(style.lang.as_deref(), Some("de-CH"), "Sprache wandert ins Paket");
}

#[test]
fn spacing_and_symbol_footnotes_are_mapped() {
    let s = TypesetSettings {
        chapter_start: Some(ChapterStart::Continuous),
        paragraph: Some(Paragraph::Spacing { em: 0.5 }),
        justify: Some(false),
        footnotes: Some(Footnotes { numbering: FootnoteNumbering::Symbols, restart: FootnoteRestart::PerChapter }),
        ..Default::default()
    };
    let style = epub_style(&s);
    assert!(style.css.contains("break-before: auto"), "fortlaufend ohne Umbruch: {}", style.css);
    assert!(style.css.contains("margin: 0 0 0.5em 0; text-indent: 0;"), "Abstand statt Einzug: {}", style.css);
    assert!(style.css.contains("text-align: start"), "Flattersatz: {}", style.css);
    assert!(style.css.contains("list-style-type: baruch-symbols"), "Fußnotenzeichen *: {}", style.css);
    assert!(!style.css.contains("h1 + p"), "ohne Einzug keine Ausnahme nach Überschriften: {}", style.css);
}
