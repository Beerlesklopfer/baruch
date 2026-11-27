// SPDX-License-Identifier: AGPL-3.0-or-later
//! EPUB mapping of the typesetting settings (Round 29, contract §5.6; used by the EPUB export in `baruch-epub`
//! since Round 20a).
//!
//! An EPUB flows: page format, margins, page numbers and running heads are determined by the reader, they only
//! appear as a comment. Mapped are breaks before chapter and part files, paragraph (indent or spacing),
//! justification, hyphenation, font, font size relative to the Typst default 11 pt, line spacing, widows/orphans
//! and the footnote marks. Classes of the export: `section.baruch-chapter` (one file per chapter),
//! `section.baruch-part` (part page), `ol.baruch-footnotes` (footnotes at the end of the chapter).
//!
//! Round 20a: chapters and parts **always** stand in files of their own (`split_chapters`), therefore the break
//! rule for `section.baruch-chapter` and `section.baruch-part` exists also without a setting (default: new page).
//! „Fortlaufend“ (`chapter_start = continuous`) sets `break-before: auto`; the file split stays, readers mostly
//! start a new file on a new page nevertheless. The language belongs into the package (`xml:lang`, `dc:language`),
//! see `lang`.

use serde::Serialize;

use super::model::{ChapterStart, FootnoteNumbering, FootnoteRestart, Paragraph, PartSide, TypesetSettings};

/// CSS and split for an EPUB from the typesetting settings.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct EpubStyle {
    /// Style rules (page breaks before chapter and part files, paragraph, justification, hyphenation, relative font
    /// size, footnote counting).
    pub css: String,
    /// One file per chapter (always `true`).
    pub split_chapters: bool,
    /// Language from `settings.lang`.
    pub lang: Option<String>,
}

/// Break to a new page (CSS 3 and the older name for EPUB 2 readers).
const PAGE_BREAK: &str = "break-before: page; page-break-before: always;";

/// Typst default of the font size (pt) to which the relative size refers.
const TYPST_DEFAULT_PT: f64 = 11.0;
/// Cap height (em) of the embedded base font; Typst measures the line spacing from the baseline to the cap height
/// of the next line, CSS from line to line.
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

/// EPUB style for `settings`.
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
        // The language belongs into the package (xml:lang); here only as a comment, so that the hyphenation is
        // explained.
        css.push_str(&format!("/* Sprache: {lang} (xml:lang im Paket) */\n"));
    }
    if !body.is_empty() {
        css.push_str(&format!("body {{ {} }}\n", body.join(" ")));
    }

    let chapter_break = match s.chapter_start {
        Some(ChapterStart::Continuous) => Some("break-before: auto; page-break-before: auto;"),
        Some(ChapterStart::NewPage) => Some(PAGE_BREAK),
        // A flowing EPUB does not know right-hand pages; `right` only takes effect in readers with double pages.
        Some(ChapterStart::RightPage) => Some("break-before: right; page-break-before: always;"),
        None => None,
    };
    // Chapters always stand in files of their own: without a setting a break to a new page.
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
        // No indent after headings (like Typst: only following paragraphs are indented).
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
