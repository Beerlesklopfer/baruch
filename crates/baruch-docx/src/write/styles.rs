// SPDX-License-Identifier: AGPL-3.0-or-later
//! Fixed parts of the file: styles (`styles.xml`), numberings (`numbering.xml`) and settings
//! (`settings.xml`).
//!
//! Built-in styles carry their built-in name (`heading 1`, `Title`, `Quote`, `footnote text` …): by this Word
//! recognises them and shows them in the language of the user interface („Überschrift 1“, „Titel“, „Zitat“,
//! „Fußnotentext“). Baruch's own styles have German names.

use std::fmt::Write as _;

use super::body::{STYLE_COMMENT_REFERENCE, STYLE_COMMENT_TEXT, STYLE_HYPERLINK, esc};
use crate::ooxml::*;

/// Language tag for `w:lang`: Word expects language and country; for a bare language code the usual country applies.
pub(super) fn word_language(lang: &str) -> String {
    let lang = lang.trim();
    let well_formed =
        !lang.is_empty() && lang.len() <= 16 && lang.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-');
    if !well_formed {
        return "de-DE".into();
    }
    match lang.to_ascii_lowercase().as_str() {
        "de" => "de-DE".into(),
        "en" => "en-GB".into(),
        "fr" => "fr-FR".into(),
        "es" => "es-ES".into(),
        "it" => "it-IT".into(),
        "nl" => "nl-NL".into(),
        _ => lang.to_owned(),
    }
}

struct Style<'a> {
    id: &'a str,
    name: &'a str,
    /// `paragraph` or `character`.
    kind: &'a str,
    based_on: Option<&'a str>,
    /// Style of the following paragraph.
    next: Option<&'a str>,
    /// Show in the quick style gallery.
    quick: bool,
    paragraph: &'a str,
    run: &'a str,
}

fn style(out: &mut String, s: &Style<'_>) {
    let _ = write!(out, "<w:style w:type=\"{}\" w:styleId=\"{}\"><w:name w:val=\"{}\"/>", s.kind, s.id, esc(s.name));
    if let Some(based_on) = s.based_on {
        let _ = write!(out, "<w:basedOn w:val=\"{based_on}\"/>");
    }
    if let Some(next) = s.next {
        let _ = write!(out, "<w:next w:val=\"{next}\"/>");
    }
    if s.quick {
        out.push_str("<w:qFormat/>");
    }
    if !s.paragraph.is_empty() {
        let _ = write!(out, "<w:pPr>{}</w:pPr>", s.paragraph);
    }
    if !s.run.is_empty() {
        let _ = write!(out, "<w:rPr>{}</w:rPr>", s.run);
    }
    out.push_str("</w:style>");
}

/// `word/styles.xml`: all styles the writer uses.
pub(super) fn styles_xml(lang: &str) -> String {
    const NORMAL: &str = "Normal";
    let lang = word_language(lang);
    let mut out = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<w:styles xmlns:w=\"{NS_W}\"><w:docDefaults>\
         <w:rPrDefault><w:rPr><w:rFonts w:ascii=\"Times New Roman\" w:hAnsi=\"Times New Roman\" w:cs=\"Times New Roman\"/>\
         <w:sz w:val=\"24\"/><w:szCs w:val=\"24\"/><w:lang w:val=\"{lang}\" w:eastAsia=\"{lang}\" w:bidi=\"ar-SA\"/></w:rPr></w:rPrDefault>\
         <w:pPrDefault><w:pPr><w:spacing w:after=\"120\" w:line=\"276\" w:lineRule=\"auto\"/></w:pPr></w:pPrDefault>\
         </w:docDefaults>\
         <w:style w:type=\"paragraph\" w:default=\"1\" w:styleId=\"{NORMAL}\"><w:name w:val=\"Normal\"/><w:qFormat/></w:style>\
         <w:style w:type=\"character\" w:default=\"1\" w:styleId=\"DefaultParagraphFont\">\
         <w:name w:val=\"Default Paragraph Font\"/><w:uiPriority w:val=\"1\"/><w:semiHidden/></w:style>"
    );
    let base = Style {
        id: "",
        name: "",
        kind: "paragraph",
        based_on: Some(NORMAL),
        next: Some(NORMAL),
        quick: true,
        paragraph: "",
        run: "",
    };
    style(
        &mut out,
        &Style {
            id: STYLE_TITLE,
            name: "Title",
            paragraph: "<w:spacing w:before=\"2400\" w:after=\"240\"/><w:jc w:val=\"center\"/>",
            run: "<w:b/><w:sz w:val=\"56\"/><w:szCs w:val=\"56\"/>",
            ..base
        },
    );
    style(
        &mut out,
        &Style {
            id: STYLE_SUBTITLE,
            name: "Subtitle",
            paragraph: "<w:spacing w:after=\"480\"/><w:jc w:val=\"center\"/>",
            run: "<w:i/><w:sz w:val=\"32\"/><w:szCs w:val=\"32\"/>",
            ..base
        },
    );
    style(
        &mut out,
        &Style {
            id: STYLE_AUTHOR,
            name: "Baruch Autor",
            paragraph: "<w:spacing w:after=\"960\"/><w:jc w:val=\"center\"/>",
            run: "<w:sz w:val=\"28\"/><w:szCs w:val=\"28\"/>",
            ..base
        },
    );
    style(
        &mut out,
        &Style {
            id: STYLE_IMPRINT,
            name: "Baruch Impressum",
            next: Some(STYLE_IMPRINT),
            paragraph: "<w:spacing w:after=\"60\"/>",
            run: "<w:sz w:val=\"18\"/><w:szCs w:val=\"18\"/>",
            ..base
        },
    );
    style(
        &mut out,
        &Style {
            id: STYLE_PART,
            name: STYLE_PART_NAME,
            paragraph: "<w:keepNext/><w:pageBreakBefore/><w:spacing w:before=\"2400\" w:after=\"480\"/>\
                        <w:jc w:val=\"center\"/><w:outlineLvl w:val=\"0\"/>",
            run: "<w:b/><w:sz w:val=\"48\"/><w:szCs w:val=\"48\"/>",
            ..base
        },
    );
    // Heading 1–6: size in half points, outline level for navigation and table of contents.
    for (level, size) in (1..=MAX_HEADING_LEVEL).zip([40u32, 32, 28, 24, 24, 24]) {
        let paragraph = format!(
            "<w:keepNext/><w:keepLines/><w:spacing w:before=\"{}\" w:after=\"120\"/><w:outlineLvl w:val=\"{}\"/>",
            if level == 1 { 480 } else { 240 },
            level - 1
        );
        let run = format!(
            "<w:b/>{}<w:sz w:val=\"{size}\"/><w:szCs w:val=\"{size}\"/>",
            if level >= 5 { "<w:i/>" } else { "" }
        );
        let (id, name) = (heading_style(level), heading_name(level));
        style(&mut out, &Style { id: &id, name: &name, paragraph: &paragraph, run: &run, ..base });
    }
    for level in 1..=MAX_OUTLINE_LEVEL {
        let (id, name, parent) = (unnumbered_style(level), format!("Baruch ohne Nummer {level}"), heading_style(level));
        style(&mut out, &Style { id: &id, name: &name, based_on: Some(&parent), ..base });
    }
    style(
        &mut out,
        &Style {
            id: STYLE_QUOTE,
            name: "Quote",
            next: Some(STYLE_QUOTE),
            paragraph: "<w:ind w:left=\"567\" w:right=\"567\"/>",
            run: "<w:i/>",
            ..base
        },
    );
    style(
        &mut out,
        &Style {
            id: STYLE_QUOTE_SOURCE,
            name: "Baruch Zitatquelle",
            paragraph: "<w:ind w:left=\"567\" w:right=\"567\"/><w:jc w:val=\"right\"/>",
            run: "<w:sz w:val=\"20\"/><w:szCs w:val=\"20\"/>",
            ..base
        },
    );
    style(
        &mut out,
        &Style {
            id: STYLE_LIST_PARAGRAPH,
            name: "List Paragraph",
            next: Some(STYLE_LIST_PARAGRAPH),
            paragraph: "<w:spacing w:after=\"60\"/><w:ind w:left=\"720\"/>",
            ..base
        },
    );
    style(
        &mut out,
        &Style {
            id: STYLE_CAPTION,
            name: "caption",
            paragraph: "<w:spacing w:after=\"240\"/><w:jc w:val=\"center\"/>",
            run: "<w:i/><w:sz w:val=\"20\"/><w:szCs w:val=\"20\"/>",
            ..base
        },
    );
    style(
        &mut out,
        &Style {
            id: STYLE_FOOTNOTE_TEXT,
            name: "footnote text",
            next: Some(STYLE_FOOTNOTE_TEXT),
            quick: false,
            paragraph: "<w:spacing w:after=\"0\" w:line=\"240\" w:lineRule=\"auto\"/>",
            run: "<w:sz w:val=\"20\"/><w:szCs w:val=\"20\"/>",
            ..base
        },
    );
    style(
        &mut out,
        &Style {
            id: STYLE_COMMENT_TEXT,
            name: "annotation text",
            next: Some(STYLE_COMMENT_TEXT),
            quick: false,
            paragraph: "<w:spacing w:after=\"0\" w:line=\"240\" w:lineRule=\"auto\"/>",
            run: "<w:sz w:val=\"20\"/><w:szCs w:val=\"20\"/>",
            ..base
        },
    );
    style(
        &mut out,
        &Style {
            id: STYLE_TYPST,
            name: "Baruch Typst",
            next: Some(STYLE_TYPST),
            paragraph: "<w:shd w:val=\"clear\" w:color=\"auto\" w:fill=\"F2F2F2\"/>",
            ..base
        },
    );
    let character =
        Style { kind: "character", based_on: Some("DefaultParagraphFont"), next: None, quick: false, ..base };
    style(
        &mut out,
        &Style {
            id: STYLE_FOOTNOTE_REFERENCE,
            name: "footnote reference",
            run: "<w:vertAlign w:val=\"superscript\"/>",
            ..character
        },
    );
    style(
        &mut out,
        &Style {
            id: STYLE_COMMENT_REFERENCE,
            name: "annotation reference",
            run: "<w:sz w:val=\"16\"/><w:szCs w:val=\"16\"/>",
            ..character
        },
    );
    style(
        &mut out,
        &Style {
            id: STYLE_HYPERLINK,
            name: "Hyperlink",
            run: "<w:color w:val=\"0563C1\"/><w:u w:val=\"single\"/>",
            ..character
        },
    );
    style(&mut out, &Style { id: STYLE_INLINE_QUOTE, name: "Baruch Zitat", quick: true, run: "<w:i/>", ..character });
    style(
        &mut out,
        &Style {
            id: STYLE_TYPST_CHAR,
            name: "Baruch Typst Zeichen",
            run: "<w:shd w:val=\"clear\" w:color=\"auto\" w:fill=\"F2F2F2\"/>",
            ..character
        },
    );
    out.push_str("</w:styles>");
    out
}

/// `word/numbering.xml`: bullets ([`NUM_BULLET`]), decimal numbers ([`NUM_DECIMAL`]) and, per further ordered
/// list, a count of its own (`extra`: `w:numId`, first number) that restarts.
pub(super) fn numbering_xml(extra: &[(u32, u64)]) -> String {
    let mut out =
        format!("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<w:numbering xmlns:w=\"{NS_W}\">");
    for (id, ordered) in [(NUM_BULLET, false), (NUM_DECIMAL, true)] {
        let _ = write!(out, "<w:abstractNum w:abstractNumId=\"{id}\"><w:multiLevelType w:val=\"hybridMultilevel\"/>");
        for level in 0..9u32 {
            let (format, text) = if ordered {
                ("decimal", format!("%{}.", level + 1))
            } else {
                ("bullet", ["•", "–", "·"][(level % 3) as usize].to_owned())
            };
            let _ = write!(
                out,
                "<w:lvl w:ilvl=\"{level}\"><w:start w:val=\"1\"/><w:numFmt w:val=\"{format}\"/>\
                 <w:lvlText w:val=\"{text}\"/><w:lvlJc w:val=\"left\"/>\
                 <w:pPr><w:ind w:left=\"{}\" w:hanging=\"360\"/></w:pPr></w:lvl>",
                720 * (level + 1)
            );
        }
        out.push_str("</w:abstractNum>");
    }
    for id in [NUM_BULLET, NUM_DECIMAL] {
        let _ = write!(out, "<w:num w:numId=\"{id}\"><w:abstractNumId w:val=\"{id}\"/></w:num>");
    }
    for (id, start) in extra {
        let _ = write!(
            out,
            "<w:num w:numId=\"{id}\"><w:abstractNumId w:val=\"{NUM_DECIMAL}\"/>\
             <w:lvlOverride w:ilvl=\"0\"><w:startOverride w:val=\"{start}\"/></w:lvlOverride></w:num>"
        );
    }
    out.push_str("</w:numbering>");
    out
}

/// `word/settings.xml`: separator lines of the footnotes and the compatibility level (Word 2013 and newer).
pub(super) fn settings_xml() -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<w:settings xmlns:w=\"{NS_W}\">\
         <w:defaultTabStop w:val=\"708\"/><w:hyphenationZone w:val=\"425\"/>\
         <w:footnotePr><w:footnote w:id=\"0\"/><w:footnote w:id=\"1\"/></w:footnotePr>\
         <w:compat><w:compatSetting w:name=\"compatibilityMode\" w:uri=\"http://schemas.microsoft.com/office/word\" \
         w:val=\"15\"/></w:compat></w:settings>"
    )
}
