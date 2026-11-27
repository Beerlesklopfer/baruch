// SPDX-License-Identifier: AGPL-3.0-or-later
//! Round 23a (hardening): a first mention across a line break comes out again on import (code review satz-1),
//! default blocks of the cover rounded to 0.01 mm. Terms and texts freely invented.

use baruch_typst::cover::TitleArea;
use baruch_typst::cover_layout::{CoverGeometry, default_blocks};
use baruch_typst::index::{
    BackMatter, FIRST_MENTION_MARK, GlossaryEntry, Projector, builtin_registers, strip_first_mentions,
};
use baruch_typst::typeset::TypesetSettings;

fn glossary(terms: &[&str]) -> BackMatter {
    BackMatter {
        settings: TypesetSettings { glossary_first_mention: Some(true), ..TypesetSettings::default() },
        registers: builtin_registers(),
        glossary: terms
            .iter()
            .enumerate()
            .map(|(n, term)| GlossaryEntry { uuid: format!("u{n}"), term: (*term).into(), ..GlossaryEntry::default() })
            .collect(),
    }
}

#[test]
fn a_first_mention_across_a_line_break_round_trips_through_the_projection() {
    let back = glossary(&["Stiller Winkel", "Alter Krummer Steg"]);
    // Hard-wrapped source: the term stands across a line end, once even across two.
    for body in [
        "= Eins\nSie fanden den Stiller\nWinkel hinter der Mühle.\n",
        "= Zwei\nÜber den Alter\nKrummer\nSteg ging niemand mehr.\n",
        "= Drei\nDer Stiller Winkel; so hieß er.\n",
    ] {
        let projected = Projector::new(&back).project(body).into_owned();
        assert!(
            projected.contains(FIRST_MENTION_MARK),
            "Testvoraussetzung: die Projektion zeichnet den Begriff aus, auch über den Umbruch: {projected:?}"
        );
        assert_eq!(
            strip_first_mentions(&projected),
            body,
            "der Import gibt den ursprünglichen Text samt Zeilenumbruch zurück (vorher blieb die Auszeichnung stehen)"
        );
    }
}

#[test]
fn a_marked_emphasis_with_a_blank_line_or_bracket_is_not_ours_and_stays() {
    let blank = format!("Der {FIRST_MENTION_MARK}#emph[Stille\n\nWinkel] bleibt.");
    assert_eq!(
        strip_first_mentions(&blank),
        blank,
        "eine Leerzeile im Wort schreibt die Projektion nie: von Hand geschrieben, bleibt unangetastet"
    );
    let nested = format!("Der {FIRST_MENTION_MARK}#emph[Stille #strong[Winkel]] bleibt.");
    assert_eq!(strip_first_mentions(&nested), nested, "eine weitere Klammer im Wort ebenso");
}

#[test]
fn default_blocks_are_rounded_to_a_hundredth_of_a_millimetre() {
    let hundredths = |mm: f64| ((mm * 100.0).round() - mm * 100.0).abs() < 1e-6;
    // Page dimensions as they arrive measured from pt (not round), and an odd spine.
    for (width, height, spine) in [(148.001_27, 209.999_93, 11.333_3), (125.0, 190.0, 4.2), (155.575, 234.95, 27.777)] {
        let geometry = CoverGeometry::new(width, height, spine, 3.0);
        for area in [TitleArea::Top, TitleArea::Middle, TitleArea::Bottom] {
            for block in default_blocks(area, &geometry) {
                assert!(
                    hundredths(block.x_mm) && hundredths(block.y_mm) && hundredths(block.width_mm),
                    "Lage und Breite auf 0,01 mm (sonst stünden Werte wie 20,999999 mm in cover.layout): {block:?}"
                );
            }
        }
    }
    let geometry = CoverGeometry::new(148.0, 210.0, 12.0, 3.0);
    let blocks = default_blocks(TitleArea::Top, &geometry);
    let title = blocks.iter().find(|b| b.kind == baruch_typst::cover_layout::CoverBlockKind::FrontTitle).unwrap();
    assert_eq!((title.x_mm, title.width_mm), (14.8, 118.4), "10 % Rand und 80 % Breite bleiben, nur gerundet");
}
