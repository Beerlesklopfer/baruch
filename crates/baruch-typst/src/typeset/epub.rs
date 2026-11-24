// SPDX-License-Identifier: AGPL-3.0-or-later
//! EPUB-Abbildung der Satz-Einstellungen (Runde 29, Vertrag §5.6; seit Runde 20a vom EPUB-Export in `baruch-epub`
//! verwendet).
//!
//! Ein EPUB fließt: Seitenformat, Ränder, Seitenzahlen und Kolumnentitel bestimmt der Leser, sie erscheinen nur als
//! Kommentar. Abgebildet werden Umbrüche vor Kapitel- und Teil-Dateien, Absatz (Einzug bzw. Abstand), Blocksatz,
//! Silbentrennung, Schrift, Schriftgröße relativ zur Typst-Vorgabe 11 pt, Zeilenabstand, Schusterjungen/Hurenkinder und
//! die Fußnotenzeichen. Klassen des Exports: `section.baruch-chapter` (eine Datei je Kapitel), `section.baruch-part`
//! (Teil-Seite), `ol.baruch-footnotes` (Fußnoten am Kapitelende).
//!
//! Runde 20a: Kapitel und Teile stehen **immer** in eigenen Dateien (`split_chapters`), darum gibt es die Umbruchregel
//! für `section.baruch-chapter` und `section.baruch-part` auch ohne Einstellung (Vorgabe: neue Seite). „Fortlaufend“
//! (`chapter_start = continuous`) setzt `break-before: auto`; die Dateiaufteilung bleibt, Leser beginnen eine neue Datei
//! trotzdem meist auf einer neuen Seite. Die Sprache gehört ins Paket (`xml:lang`, `dc:language`), siehe `lang`.

use serde::Serialize;

use super::model::{ChapterStart, FootnoteNumbering, FootnoteRestart, Paragraph, PartSide, TypesetSettings};

/// CSS und Aufteilung für ein EPUB aus den Satz-Einstellungen.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct EpubStyle {
    /// Stilregeln (Seitenumbrüche vor Kapitel- und Teil-Dateien, Absatz, Blocksatz, Silbentrennung, Schriftgröße
    /// relativ, Fußnotenzählung).
    pub css: String,
    /// Je Kapitel eine Datei (immer `true`).
    pub split_chapters: bool,
    /// Sprache aus `settings.lang`.
    pub lang: Option<String>,
}

/// Umbruch auf eine neue Seite (CSS 3 und der ältere Name für EPUB-2-Leser).
const PAGE_BREAK: &str = "break-before: page; page-break-before: always;";

/// Typst-Vorgabe der Schriftgröße (pt), auf die sich die relative Größe bezieht.
const TYPST_DEFAULT_PT: f64 = 11.0;
/// Versalhöhe (em) der eingebetteten Grundschrift; Typst misst den Zeilenabstand von der Grundlinie zur Versalhöhe der
/// nächsten Zeile, CSS von Zeile zu Zeile.
const CAP_HEIGHT_EM: f64 = 0.65;

fn fmt(x: f64) -> String {
    let rounded = (x * 100.0).round() / 100.0;
    format!("{rounded}")
}

fn css_string(text: &str) -> String {
    let escaped: String = text
        .chars()
        .filter(|c| !c.is_control())
        .flat_map(|c| if c == '"' || c == '\\' { vec!['\\', c] } else { vec![c] })
        .collect();
    format!("\"{escaped}\"")
}

/// EPUB-Stil zu `settings`.
pub fn epub_style(settings: &TypesetSettings) -> EpubStyle {
    let s = settings;
    let mut css = String::from("/* Baruch: Satz (EPUB) */\n");
    let mut body: Vec<String> = Vec::new();
    if let Some(font) = s.font.as_deref().map(str::trim).filter(|f| !f.is_empty()) {
        body.push(format!("font-family: {}, serif;", css_string(font)));
    }
    if let Some(size) = s.font_size_pt.filter(|x| x.is_finite() && *x > 0.0) {
        body.push(format!("font-size: {}em;", fmt(size / TYPST_DEFAULT_PT)));
    }
    if let Some(leading) = s.leading_em.filter(|x| x.is_finite()) {
        body.push(format!("line-height: {};", fmt(CAP_HEIGHT_EM + leading)));
    }
    if let Some(lang) = s.lang.as_deref().map(str::trim).filter(|l| !l.is_empty()) {
        // Die Sprache gehört ins Paket (xml:lang); hier nur als Kommentar, damit die Silbentrennung erklärt ist.
        css.push_str(&format!("/* Sprache: {lang} (xml:lang im Paket) */\n"));
    }
    if !body.is_empty() {
        css.push_str(&format!("body {{ {} }}\n", body.join(" ")));
    }

    let chapter_break = match s.chapter_start {
        Some(ChapterStart::Continuous) => Some("break-before: auto; page-break-before: auto;"),
        Some(ChapterStart::NewPage) => Some(PAGE_BREAK),
        // Rechte Seiten kennt ein fließendes EPUB nicht; `right` wirkt nur in Lesern mit Doppelseiten.
        Some(ChapterStart::RightPage) => Some("break-before: right; page-break-before: always;"),
        None => None,
    };
    // Kapitel stehen immer in eigenen Dateien: ohne Einstellung ein Umbruch auf eine neue Seite.
    let chapter_break = chapter_break.unwrap_or(PAGE_BREAK);
    css.push_str(&format!("section.baruch-chapter {{ {chapter_break} }}\n"));
    match &s.part_page {
        Some(part) => {
            let before = match part.side {
                PartSide::Right => "break-before: right; page-break-before: always;",
                PartSide::Any => PAGE_BREAK,
            };
            let after = if part.blank_back { " break-after: page; page-break-after: always;" } else { "" };
            css.push_str(&format!("section.baruch-part {{ {before}{after} }}\n"));
        }
        None => css.push_str(&format!("section.baruch-part {{ {PAGE_BREAK} }}\n")),
    }

    let mut p: Vec<String> = Vec::new();
    match s.paragraph {
        Some(Paragraph::Indent { em }) if em.is_finite() => {
            p.push(format!("margin: 0; text-indent: {}em;", fmt(em)));
        }
        Some(Paragraph::Spacing { em }) if em.is_finite() => {
            p.push(format!("margin: 0 0 {}em 0; text-indent: 0;", fmt(em)));
        }
        _ => {}
    }
    match s.justify {
        Some(true) => p.push("text-align: justify;".into()),
        Some(false) => p.push("text-align: start;".into()),
        None => {}
    }
    match s.hyphenate {
        Some(true) => p.push("hyphens: auto; -webkit-hyphens: auto; -epub-hyphens: auto;".into()),
        Some(false) => p.push("hyphens: manual; -webkit-hyphens: manual; -epub-hyphens: manual;".into()),
        None => {}
    }
    match s.widows_orphans {
        Some(true) => p.push("widows: 2; orphans: 2;".into()),
        Some(false) => p.push("widows: 1; orphans: 1;".into()),
        None => {}
    }
    if !p.is_empty() {
        css.push_str(&format!("p {{ {} }}\n", p.join(" ")));
    }
    if matches!(s.paragraph, Some(Paragraph::Indent { .. })) {
        // Kein Einzug nach Überschriften (wie Typst: nur Folgeabsätze werden eingezogen).
        css.push_str("h1 + p, h2 + p, h3 + p { text-indent: 0; }\n");
    }

    if let Some(footnotes) = &s.footnotes {
        let style = match footnotes.numbering {
            FootnoteNumbering::Arabic => "decimal",
            FootnoteNumbering::Letters => "lower-alpha",
            FootnoteNumbering::Symbols => {
                css.push_str(
                    "@counter-style baruch-symbols { system: symbolic; symbols: \"*\" \"†\" \"‡\"; suffix: \" \"; }\n",
                );
                "baruch-symbols"
            }
        };
        css.push_str(&format!("ol.baruch-footnotes {{ list-style-type: {style}; }}\n"));
        if footnotes.restart == FootnoteRestart::Continuous {
            css.push_str(
                "/* Fußnoten fortlaufend: der Export nummeriert über die Kapiteldateien hinweg (start=…). */\n",
            );
        }
    }
    if s.page_format.is_some() || s.margins.is_some() || s.page_numbers.is_some() || s.running_head.is_some() {
        css.push_str("/* Seitenformat, Ränder, Seitenzahlen und Kolumnentitel bestimmt der Leser (EPUB fließt). */\n");
    }
    EpubStyle {
        css,
        split_chapters: true,
        lang: s.lang.as_deref().map(str::trim).filter(|l| !l.is_empty()).map(str::to_owned),
    }
}
