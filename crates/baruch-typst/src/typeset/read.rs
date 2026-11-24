// SPDX-License-Identifier: AGPL-3.0-or-later
//! Regelblock „// Baruch: Satz“ zurück in Satz-Einstellungen (Runde 21b, Import eines Exports).
//!
//! [`settings_from_rules`] liest die Zeilen, die [`rules`](super::rules) schreibt, Feld für Feld zurück und nimmt das
//! Ergebnis **nur**, wenn [`rules`](super::rules) daraus wieder genau denselben Block macht – nie raten. Seitenzahlen
//! und Kolumnentitel teilen sich Hilfsfunktionen und eine Kopfzeile; für sie werden alle Kombinationen probiert (9 × 4).
//!
//! Grenzen (dokumentiert in `docs/Architektur.md`, „Typst-Import“): Der Block trägt die R31-Felder (Register, Glossar,
//! erste Erwähnung) und `xref_with_page` nicht; sie kommen nicht zurück. Der Bundsteg ist im Block schon zum Innenrand
//! gezählt: zurück kommt der Innenrand als Summe (Bundsteg 0, über 80 mm der Rest als Bundsteg) – der Satz ist gleich.

use super::model::{
    ChapterStart, FootnoteNumbering, FootnoteRestart, Footnotes, MARGIN_MM, Margins, PageFormat, PageNumberPosition,
    PageNumberStart, PageNumbers, Paragraph, PartPage, PartSide, RULES_MARK, RunningHead, TypesetSettings,
};
use super::rules::rules;

/// Satz-Einstellungen aus einem Regelblock (beginnt mit der Zeile [`RULES_MARK`], wie [`rules`] ihn schreibt). `None`,
/// wenn der Block nicht genau so aus Einstellungen entstanden sein kann.
pub fn settings_from_rules(block: &str) -> Option<TypesetSettings> {
    let mut lines = block.lines();
    if lines.next()? != RULES_MARK {
        return None;
    }
    let mut s = TypesetSettings::default();
    for line in lines {
        read_line(&mut s, line);
    }
    let mut numbers: Vec<Option<PageNumbers>> = vec![None];
    for position in PageNumberPosition::ALL {
        for start in PageNumberStart::ALL {
            numbers.push(Some(PageNumbers { position: *position, start: *start }));
        }
    }
    let heads = [None, Some(RunningHead::Off), Some(RunningHead::WorkTitle), Some(RunningHead::Chapter)];
    for page_numbers in &numbers {
        for running_head in heads {
            let candidate = TypesetSettings { page_numbers: page_numbers.clone(), running_head, ..s.clone() };
            if rules(&candidate) == block {
                return Some(candidate);
            }
        }
    }
    None
}

/// Inhalt zwischen `prefix` und `suffix`.
fn between<'a>(line: &'a str, prefix: &str, suffix: &str) -> Option<&'a str> {
    line.strip_prefix(prefix)?.strip_suffix(suffix)
}

fn number(text: &str, unit: &str) -> Option<f64> {
    text.trim().strip_suffix(unit)?.parse().ok().filter(|x: &f64| x.is_finite())
}

/// Typst-String-Literal (Gegenstück zu `literal` in `rules.rs`).
fn unquote(text: &str) -> Option<String> {
    let inner = text.trim().strip_prefix('"')?.strip_suffix('"')?;
    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next()? {
                'n' => out.push('\n'),
                'r' => out.push('\r'),
                't' => out.push('\t'),
                other => out.push(other),
            }
        } else {
            out.push(c);
        }
    }
    Some(out)
}

fn read_line(s: &mut TypesetSettings, line: &str) {
    match line {
        "// Kapitelanfang: fortlaufend (kein Seitenumbruch vor Kapiteln)" => {
            s.chapter_start = Some(ChapterStart::Continuous)
        }
        "#show heading.where(level: 1): it => { pagebreak(weak: true); it }" => {
            s.chapter_start = Some(ChapterStart::NewPage)
        }
        "#show heading.where(level: 1): it => { pagebreak(weak: true, to: \"odd\"); it }" => {
            s.chapter_start = Some(ChapterStart::RightPage)
        }
        "#set text(costs: (widow: 100%, orphan: 100%))" => s.widows_orphans = Some(true),
        "#set text(costs: (widow: 0%, orphan: 0%))" => s.widows_orphans = Some(false),
        "#set par(justify: true)" => s.justify = Some(true),
        "#set par(justify: false)" => s.justify = Some(false),
        "#set text(hyphenate: true)" => s.hyphenate = Some(true),
        "#set text(hyphenate: false)" => s.hyphenate = Some(false),
        _ => read_valued(s, line),
    }
}

fn read_valued(s: &mut TypesetSettings, line: &str) {
    if let Some(rest) = between(line, "// Teil-Seite: ", " (Regel der Teil-Seiten im Satz)") {
        let (label, blank_back) = match rest.strip_suffix(", Rückseite leer") {
            Some(label) => (label, true),
            None => (rest, false),
        };
        if let Some(side) = PartSide::ALL.iter().copied().find(|side| side.label() == label) {
            s.part_page = Some(PartPage { side, blank_back });
        }
    } else if let Some(rest) = between(line, "#set page(width: ", ")") {
        let Some((w, h)) = rest.split_once(", height: ") else { return };
        let (Some(w), Some(h)) = (number(w, "mm"), number(h, "mm")) else { return };
        let fixed = PageFormat::ALL.iter().copied().find(|f| f.size_mm() == (w, h));
        s.page_format = Some(fixed.unwrap_or(PageFormat::Custom { width_mm: w, height_mm: h }));
    } else if let Some(rest) = between(line, "#set page(margin: (", "))") {
        let mut values = [None; 4];
        for part in rest.split(", ") {
            let Some((key, value)) = part.split_once(": ") else { return };
            let slot = match key {
                "inside" => 0,
                "outside" => 1,
                "top" => 2,
                "bottom" => 3,
                _ => return,
            };
            values[slot] = number(value, "mm");
        }
        let [Some(inside), Some(outside), Some(top), Some(bottom)] = values else { return };
        let binding = (inside - MARGIN_MM.1).max(0.0);
        s.margins = Some(Margins {
            inside_mm: inside - binding,
            outside_mm: outside,
            top_mm: top,
            bottom_mm: bottom,
            binding_mm: binding,
        });
    } else if let Some(rest) = between(line, "#set text(font: ", ")") {
        s.font = unquote(rest);
    } else if let Some(rest) = between(line, "#set text(size: ", ")") {
        s.font_size_pt = number(rest, "pt");
    } else if let Some(rest) = between(line, "#set par(leading: ", ")") {
        s.leading_em = number(rest, "em");
    } else if let Some(rest) = between(line, "#set par(first-line-indent: ", ")") {
        let Some((indent, spacing)) = rest.split_once(", spacing: ") else { return };
        let (Some(indent), Some(spacing)) = (number(indent, "em"), number(spacing, "em")) else { return };
        s.paragraph =
            Some(if indent == 0.0 { Paragraph::Spacing { em: spacing } } else { Paragraph::Indent { em: indent } });
    } else if let Some(rest) = between(line, "#set text(lang: ", ")") {
        s.lang = match rest.split_once(", region: ") {
            Some((base, region)) => {
                Some(format!("{}-{}", unquote(base).unwrap_or_default(), unquote(region).unwrap_or_default()))
            }
            None => unquote(rest),
        };
    } else if let Some(rest) = between(line, "#set footnote(numbering: \"", "\")") {
        let numbering = match rest {
            "1" => FootnoteNumbering::Arabic,
            "*" => FootnoteNumbering::Symbols,
            "a" => FootnoteNumbering::Letters,
            _ => return,
        };
        let restart = s.footnotes.as_ref().map_or(FootnoteRestart::Continuous, |f| f.restart);
        s.footnotes = Some(Footnotes { numbering, restart });
    } else if line.contains("counter(footnote).update(0)")
        && let Some(f) = s.footnotes.as_mut()
    {
        f.restart = FootnoteRestart::PerChapter;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_block_of_full_settings_reads_back_to_the_same_rules() {
        let s = TypesetSettings {
            chapter_start: Some(ChapterStart::RightPage),
            part_page: Some(PartPage { side: PartSide::Right, blank_back: true }),
            page_format: Some(PageFormat::Mm120x190),
            margins: Some(Margins {
                inside_mm: 18.0,
                outside_mm: 14.0,
                top_mm: 16.0,
                bottom_mm: 20.0,
                binding_mm: 4.0,
            }),
            font: Some("Linux \"Libertine\"".into()),
            font_size_pt: Some(10.5),
            leading_em: Some(0.7),
            paragraph: Some(Paragraph::Indent { em: 1.2 }),
            justify: Some(true),
            hyphenate: Some(false),
            lang: Some("de-CH".into()),
            page_numbers: Some(PageNumbers {
                position: PageNumberPosition::TopOutside,
                start: PageNumberStart::FirstChapter,
            }),
            running_head: Some(RunningHead::Chapter),
            footnotes: Some(Footnotes { numbering: FootnoteNumbering::Symbols, restart: FootnoteRestart::PerChapter }),
            widows_orphans: Some(true),
            ..TypesetSettings::default()
        };
        let block = rules(&s);
        let back = settings_from_rules(&block).expect("ein Block aus Einstellungen liest sich zurück");
        assert_eq!(rules(&back), block, "derselbe Block (das ist die Bedingung der Rückwandlung)");
        assert_eq!(back.page_numbers, s.page_numbers, "Seitenzahlen gefunden");
        assert_eq!(back.running_head, s.running_head, "Kolumnentitel gefunden");
        assert_eq!(back.footnotes, s.footnotes, "Fußnoten samt Neubeginn je Kapitel");
        assert_eq!(back.font, s.font, "Schrift mit Anführungszeichen unmaskiert");
        assert_eq!(
            back.margins.map(|m| m.inside_mm + m.binding_mm),
            Some(22.0),
            "Innenrand und Bundsteg kommen als Summe zurück (Grenze, der Satz ist gleich)"
        );
    }

    #[test]
    fn a_changed_block_is_not_guessed() {
        let s = TypesetSettings { justify: Some(true), ..TypesetSettings::default() };
        let block = rules(&s);
        assert!(settings_from_rules(&block).is_some(), "unverändert lesbar");
        let edited = block.replace("#set par(justify: true)\n", "#set par(justify: true)\n#set text(fill: red)\n");
        assert_eq!(settings_from_rules(&edited), None, "eine fremde Zeile im Block: nicht raten");
        assert_eq!(settings_from_rules("// Baruch: Satz von Hand\n"), None, "andere Kennzeile");
    }
}
