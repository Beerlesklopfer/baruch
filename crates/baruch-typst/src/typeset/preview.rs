// SPDX-License-Identifier: AGPL-3.0-or-later
//! Source of the preview in the settings (Round 29, contract §6.1 `work.typeset_preview`): work template, rule
//! block of the (unsaved) settings and a freely invented sample text – one part, three chapters with sections, one
//! footnote per chapter, enough text for at least six pages A5. This way the preview shows chapter start, part
//! page, page numbers, running heads and footnotes without typesetting the book text.
//!
//! As in the book typesetting (`typeset::prepare` in the server) the rule of the part pages stands behind the rule
//! block ([`part_page_rule_for`]) – so the part page looks in the preview as in the book.

use super::model::TypesetSettings;
use super::rules::{part_page_rule_for, rules};

/// Freely invented sentences of the sample text (no work text).
const SENTENCES: [&str; 12] = [
    "Am Rand der kleinen Stadt stand ein Leuchtturm, der seit Jahren kein Schiff mehr gesehen hatte.",
    "Die Wärterin stieg trotzdem jeden Abend die einhundertzwölf Stufen hinauf und putzte das Glas.",
    "Im Winter froren die Pfützen auf dem Weg zum Hafen, und die Möwen saßen dicht gedrängt auf den Pollern.",
    "Niemand wusste genau, wer die Lampe zuletzt angezündet hatte, aber alle erzählten davon.",
    "Der Bäcker behauptete, das Licht sei in einer Novembernacht dreimal kurz aufgeblinkt.",
    "Seine Nachbarin hielt das für eine Ausrede, weil er an jenem Morgen das Brot verbrannt hatte.",
    "Zwischen den Dünen wuchs Strandhafer, und der Wind schrieb lange Linien in den Sand.",
    "Wer genau hinsah, fand dort Muscheln, die wie kleine Ohren aussahen und das Meer zu hören schienen.",
    "Die Kinder sammelten sie in Blechdosen und stellten sie in Reihen auf die Fensterbänke.",
    "Am Sonntag läutete die Glocke der Kapelle, auch wenn nur drei Leute zum Gottesdienst kamen.",
    "Danach tranken alle Tee im Gemeindehaus und stritten freundlich über das Wetter der kommenden Woche.",
    "So verging ein Jahr nach dem anderen, und der Leuchtturm wartete geduldig auf sein Schiff.",
];

/// Paragraphs per section, sentences per paragraph, sections per chapter: 3 chapters × 2 sections × 4 paragraphs ×
/// 8 sentences = 192 sentences ≈ 3000 words; on A5 with the default font (≈ 400 words per page) a good seven pages
/// of text, plus the part page.
const PARAGRAPHS: usize = 4;
const SENTENCES_PER_PARAGRAPH: usize = 8;
const SECTIONS: usize = 2;

const CHAPTERS: [&str; 3] = ["Der Leuchtturm", "Die Dünen", "Der Sonntag"];

/// Source of the preview: `template` + "\n" + `rules(settings)` + rule of the part pages + sample text; `title`
/// appears on the part page.
pub fn preview_source(settings: &TypesetSettings, template: &str, title: &str) -> String {
    let mut out = String::with_capacity(template.len() + 26_000);
    out.push_str(template);
    out.push('\n');
    out.push_str(&rules(settings));
    out.push_str(&part_page_rule_for(&["I".to_owned()], settings));
    let part = if title.trim().is_empty() { "Probeseiten".to_owned() } else { format!("Probeseiten: {title}") };
    out.push_str(&crate::marker::part_line(&part));
    out.push('\n');
    let mut n = 0usize;
    for (c, chapter) in CHAPTERS.iter().enumerate() {
        out.push_str(&format!("= {chapter}\n\n"));
        for s in 0..SECTIONS {
            out.push_str(&format!("== Abschnitt {}.{}\n\n", c + 1, s + 1));
            for p in 0..PARAGRAPHS {
                for i in 0..SENTENCES_PER_PARAGRAPH {
                    if i > 0 {
                        out.push(' ');
                    }
                    out.push_str(SENTENCES[n % SENTENCES.len()]);
                    n += 1;
                }
                if s == 0 && p == 0 {
                    out.push_str(&format!(
                        "#footnote[Eine erfundene Anmerkung zum Kapitel „{chapter}“, damit die Fußnoten zu sehen sind.]"
                    ));
                }
                out.push_str("\n\n");
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_starts_with_template_and_holds_part_chapters_and_footnotes() {
        let source = preview_source(&TypesetSettings::default(), "#set page(paper: \"a5\")", "Erfundenes Buch");
        assert!(source.starts_with("#set page(paper: \"a5\")\n"), "Werkvorlage zuerst, dann Zeilenende");
        assert_eq!(
            source.matches("<baruch-part>\n").count(),
            1,
            "genau ein Teil (die Teil-Regel davor nennt das Label auch)"
        );
        assert_eq!(source.lines().filter(|l| l.starts_with("= ")).count(), 3, "drei Kapitel");
        assert_eq!(source.matches("#footnote[").count(), 3, "eine Fußnote je Kapitel");
        let words = source.split_whitespace().count();
        // Roughly ≈ 400 words per A5 page with the default font; 2500 words are certainly enough for six pages.
        assert!(words > 2500, "genug Text für mindestens sechs Seiten A5: {words}");
    }
}
