// SPDX-License-Identifier: AGPL-3.0-or-later
//! Rahmen (Umschlag, Titelei) im Satz: Seitenzahl und Seitenparität des ersten Kapitels dürfen nicht davon
//! abhängen, ob die Umschlagseiten mitgesetzt werden (Nachbesserung Runde 11). Typst prüft selbst: ein
//! `assert` im Kapitel scheitert mit einer Fehlermeldung, wenn Zähler oder physische Seite nicht stimmen.

use baruch_render::{Severity, Source, compile};
use baruch_typst::cover::{BookInfo, CoverRef, FrameOptions, frame_parts};

const COVER_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="40" height="60"><rect width="40" height="60" fill="#468"/></svg>"##;

fn info(dedication: &str) -> BookInfo {
    BookInfo {
        title: "Das Teichbuch".into(),
        author: "Frieda Fröhlich".into(),
        dedication: dedication.into(),
        ..BookInfo::default()
    }
}

/// Rahmen + ein Kapitel, das prüft: logische Seitenzahl `logical`, physische Seite ungerade (rechts).
fn book(cover_pages: bool, front_matter: bool, dedication: &str, logical: usize) -> Source {
    let parts = frame_parts(
        &info(dedication),
        &FrameOptions {
            cover: Some(CoverRef { path: ".baruch/u.svg".into(), alt: "Umschlag".into() }),
            cover_pages,
            front_matter,
            ..FrameOptions::default()
        },
    );
    let chapter = format!(
        "#set page(numbering: \"1\")\n= Erstes Kapitel\n#context {{\n  \
         assert.eq(counter(page).get().first(), {logical}, message: \"logische Seite\")\n  \
         assert(calc.odd(here().page()), message: \"physische Seite ungerade (rechts)\")\n}}\nText.\n"
    );
    Source {
        main: format!("{}{chapter}{}", parts.front, parts.back),
        files: vec![(".baruch/u.svg".into(), COVER_SVG.as_bytes().to_vec())],
    }
}

fn errors(source: &Source) -> Vec<String> {
    compile(source).diagnostics.into_iter().filter(|d| d.severity == Severity::Error).map(|d| d.message).collect()
}

#[test]
fn the_first_chapter_starts_right_with_the_same_number_with_or_without_cover_pages() {
    // Titelei: Schmutztitel, Seite 2, Haupttitel, Impressum → Kapitel auf Seite 5; mit Widmung (Widmung und
    // Leerseite) auf Seite 7. Ohne Titelei auf Seite 1. Diese Zahlen gelten mit und ohne Umschlagseiten,
    // denn die leere Innenseite des Umschlags hält die Parität und setzt den Zähler zurück.
    for cover_pages in [false, true] {
        for (front_matter, dedication, logical) in [(true, "", 5), (true, "Für Mia.", 7), (false, "", 1)] {
            let source = book(cover_pages, front_matter, dedication, logical);
            assert_eq!(
                errors(&source),
                Vec::<String>::new(),
                "Umschlagseiten {cover_pages}, Titelei {front_matter}, Widmung {dedication:?}: erstes Kapitel \
                 auf Seite {logical}, rechts"
            );
        }
    }
}

#[test]
fn the_check_itself_notices_a_wrong_number() {
    // Gegenprobe: dieselbe Prüfung mit falscher Erwartung scheitert, die Prüfung oben ist also wirksam.
    let wrong = book(true, true, "", 6);
    let found = errors(&wrong);
    assert!(found.iter().any(|m| m.contains("logische Seite")), "falsche Seitenzahl wird bemerkt: {found:?}");
}

/// Grundlage von `chapter_number` im Editorkopf (baruch-server `view.rs`): Typst zählt in `counter(heading)`
/// nur Überschriften mit Nummerierung. Ein Vorwort mit `numbering: none` (so exportiert Baruch Knoten mit
/// „Nummer aus“) und eine Vorlage ganz ohne Nummerierung zählen nicht; `at(-1)` ist das letzte Element.
#[test]
fn typst_counts_only_numbered_headings() {
    let numbered = "#set heading(numbering: \"1.\")\n\
        #show heading.where(level: 1, body: [Vorwort]): set heading(numbering: none)\n\
        = Vorwort\n#context assert.eq(counter(heading).get().first(), 0, message: \"Vorwort zählt nicht\")\n\
        = Erstes\n#context assert.eq(counter(heading).get().first(), 1, message: \"Erstes ist 1\")\n\
        = Zweites\n#context assert.eq(counter(heading).get().first(), 2, message: \"Zweites ist 2\")\n";
    let plain = "= Eins\n#context assert.eq(counter(heading).get().first(), 0, message: \"ohne Nummerierung 0\")\n\
        #assert.eq((\"a\", \"b\").at(0 - 1), \"b\", message: \"negativer Index vom Ende\")\n";
    for main in [numbered, plain] {
        let source = Source { main: main.into(), files: Vec::new() };
        assert_eq!(errors(&source), Vec::<String>::new(), "Typst zählt wie beschrieben:\n{main}");
    }
}
