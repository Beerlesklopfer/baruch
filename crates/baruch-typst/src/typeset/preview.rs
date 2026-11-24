// SPDX-License-Identifier: AGPL-3.0-or-later
//! Quelle der Vorschau in den Einstellungen (Runde 29, Vertrag §6.1 `work.typeset_preview`): Werkvorlage, Regelblock
//! der (ungespeicherten) Einstellungen und ein frei erfundener Mustertext – ein Teil, drei Kapitel mit Abschnitten,
//! eine Fußnote je Kapitel, genug Text für mindestens sechs Seiten A5. So zeigt die Vorschau Kapitelanfang,
//! Teil-Seite, Seitenzahlen, Kolumnentitel und Fußnoten, ohne den Buchtext zu setzen.
//!
//! Wie im Buchsatz (`typeset::prepare` im Server) steht hinter dem Regelblock die Regel der Teil-Seiten
//! ([`part_page_rule_for`]) – die Teil-Seite sieht in der Vorschau also aus wie im Buch.

use super::model::TypesetSettings;
use super::rules::{part_page_rule_for, rules};

/// Frei erfundene Sätze des Mustertexts (kein Werktext).
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

/// Absätze je Abschnitt, Sätze je Absatz, Abschnitte je Kapitel: 3 Kapitel × 2 Abschnitte × 4 Absätze × 8 Sätze
/// = 192 Sätze ≈ 3000 Wörter; auf A5 mit Vorgabeschrift (≈ 400 Wörter je Seite) gut sieben Seiten Text, dazu die
/// Teil-Seite.
const PARAGRAPHS: usize = 4;
const SENTENCES_PER_PARAGRAPH: usize = 8;
const SECTIONS: usize = 2;

const CHAPTERS: [&str; 3] = ["Der Leuchtturm", "Die Dünen", "Der Sonntag"];

/// Quelle der Vorschau: `template` + "\n" + `rules(settings)` + Regel der Teil-Seiten + Mustertext; `title` erscheint
/// auf der Teil-Seite.
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
        // Grob ≈ 400 Wörter je A5-Seite mit Vorgabeschrift; 2500 Wörter reichen sicher für sechs Seiten.
        assert!(words > 2500, "genug Text für mindestens sechs Seiten A5: {words}");
    }
}
