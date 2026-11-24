// SPDX-License-Identifier: AGPL-3.0-or-later
//! Die dreizehn Gliederungsschemata (`baruch_typst::scheme::export_template`) setzen mit Typst ohne
//! Fehler (Backlog: „nicht mit typst compile geprüft“, Vertrag Runde 8, Abschnitt 4e), und seit
//! Runde 14 (Vertrag Abschnitt 3/4) zählt Typst wie Baruch: Je Überschrift prüft der Testtext selbst
//! mit `#context assert.eq(…)`, dass Typst dieselbe Nummer setzt wie `Numbering::heading` – mit Teilen
//! und Kapiteln ohne Nummer als Markierung (`baruch_typst::part_line`, `unnumbered_line`).

use baruch_render::{Severity, Source, compile};
use baruch_typst::scheme::{Numbering, Scheme, export_template};

/// Erfundene Gliederung mit Teil, Kapiteln, Abschnitten und Unterabschnitten – vier Kapitel, damit
/// „Predigt“ und „Buch“ Einleitung, I., II. und Schluss bilden.
const BODY: &str = "= Erster Teil\n== Am Anfang\nText.\n=== Ein Unterpunkt\nMehr.\n== Mitte\nText.\n\
= Zweiter Teil\n== Vorletztes\nText.\n== Ende\nSchluss.\n";

#[test]
fn all_thirteen_numbering_schemes_compile_without_errors() {
    let template = "#set heading(numbering: \"1.1\")\n";
    for scheme in Scheme::ALL {
        let numbering = Numbering::of_scheme(scheme, Some("I.;A.;1.;a)"), template)
            .unwrap_or_else(|e| panic!("Schema {} ergibt keine Nummerierung: {e}", scheme.id()));
        let main = format!("{}{BODY}", export_template(template, scheme, &numbering));
        let rendered = compile(&Source { main: main.clone(), files: Vec::new() });
        let errors: Vec<_> = rendered.diagnostics.iter().filter(|d| d.severity == Severity::Error).collect();
        assert!(errors.is_empty(), "Schema „{}“ setzt nicht fehlerfrei: {errors:?}\n{main}", scheme.label());
        assert!(!rendered.pages.is_empty(), "Schema „{}“ muss Seiten ergeben", scheme.label());
    }
}

#[test]
fn the_custom_scheme_with_the_part_pattern_compiles_too() {
    // „I|1“ (Teile römisch, Kapitel arabisch) ist die zweite Form der Mustersprache.
    let numbering = Numbering::of_scheme(Scheme::Custom, Some("I|1"), "").expect("Muster I|1 ist gültig");
    let main = format!("{}{BODY}", export_template("", Scheme::Custom, &numbering));
    let rendered = compile(&Source { main, files: Vec::new() });
    assert!(!rendered.has_errors(), "„I|1“ muss fehlerfrei setzen: {:?}", rendered.diagnostics);
}

// ---- Runde 14: Typst zählt wie Baruch ------------------------------------------------------------

/// Gliederung (Ebene 0 = Teil, 1–3; mit Nummer?; Titel), frei erfunden: zwei Teile, ein Vorwort und
/// ein Nachwort ohne Nummer, ein Abschnitt ohne Nummer zwischen nummerierten, 13 nummerierte Kapitel
/// (≥ 12: römische Zahlen und Ordnungswörter über X hinaus).
fn outline() -> Vec<(u8, bool, String)> {
    let mut items = vec![
        (1, false, "Vorwort".to_owned()),
        (0, true, "Erster Teil".to_owned()),
        (1, true, "Am Anfang".to_owned()),
        (2, true, "Ein Abschnitt".to_owned()),
        (3, true, "Ein Unterpunkt".to_owned()),
        (3, true, "Noch ein Unterpunkt".to_owned()),
        (2, false, "Randnotiz".to_owned()),
        (2, true, "Zweiter Abschnitt".to_owned()),
        (0, true, "Zweiter Teil".to_owned()),
    ];
    for n in 2..=13 {
        items.push((1, true, format!("Kapitel Nummer {n}")));
        if n == 12 {
            items.push((2, true, "Spät".to_owned()));
        }
    }
    items.push((1, false, "Nachwort".to_owned()));
    items
}

/// Zählerstände wie `baruch-store` (`load::numbers`): Kapitel über Teile hinweg fortlaufend,
/// Abschnitte je Elternteil, Punkte ohne Nummer zählen nicht. `None` für Teile und Punkte ohne Nummer.
fn paths(items: &[(u8, bool, String)]) -> Vec<Option<Vec<usize>>> {
    let mut counters = [0usize; 4];
    items
        .iter()
        .map(|(level, numbered, _)| {
            let level = usize::from(*level);
            if level == 0 || !numbered {
                return None;
            }
            counters[level] += 1;
            for deeper in counters.iter_mut().skip(level + 1) {
                *deeper = 0;
            }
            Some(counters[1..=level].to_vec())
        })
        .collect()
}

/// Typst-String (`\` und `"` maskiert).
fn typst_str(text: &str) -> String {
    format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""))
}

/// Prüfregel im Testtext: die Nummer, die Typst für die zuletzt gesetzte Überschrift bildet, ist
/// `expected`. Eine Funktion, die Inhalt liefert (Predigt: `context { … }` für „Schluss“), lässt sich
/// nicht als Text vergleichen; dann muss der gesetzte Inhalt genau so breit sein wie der erwartete Text.
const CHECK: &str = "#let baruch-check(expected) = context {\n\
  let numbering-rule = heading.numbering\n\
  if numbering-rule == none { assert.eq(\"\", expected, message: \"Typst nummeriert nicht\") } else {\n\
    let shown = numbering(numbering-rule, ..counter(heading).get())\n\
    if type(shown) == str { assert.eq(shown, expected) } else {\n\
      assert.eq(measure(shown).width, measure(expected).width, message: \"Breite für \" + expected)\n\
    }\n\
  }\n\
}\n";

fn checked_body(numbering: &Numbering) -> String {
    let items = outline();
    let paths = paths(&items);
    let chapters = paths.iter().flatten().filter(|p| p.len() == 1).count();
    let mut body = String::new();
    for ((level, numbered, title), path) in items.iter().zip(&paths) {
        match (level, numbered) {
            (0, _) => body.push_str(&baruch_typst::part_line(title)),
            (_, false) => body.push_str(&baruch_typst::unnumbered_line(*level, title)),
            (_, true) => body.push_str(&format!("{} {title}\n", "=".repeat(usize::from(*level)))),
        }
        if let Some(path) = path {
            let expected = numbering.heading(path, chapters);
            body.push_str(&format!("#baruch-check({})\n", typst_str(&expected)));
        }
        body.push_str("Text.\n");
    }
    body
}

#[test]
fn typst_numbers_every_heading_like_baruch_in_all_thirteen_schemes() {
    // Vertrag Runde 14, Abschnitt 3: eine Quelle für Baum und Satz. Jede Überschrift trägt eine
    // `#context assert.eq(…)`-Prüfung mit der Nummer aus Numbering::heading; setzt Typst eine andere,
    // bricht der Satz mit einem Fehler ab. Teile und Punkte ohne Nummer stehen als Markierung im Text
    // und zählen in Typst nicht mit (wie in Baruch).
    let template = "#set heading(numbering: \"1.1\")\n";
    let mut customs = vec![("I.;A.;1.;a);aa)", Scheme::Custom), ("I|1", Scheme::Custom), ("1.aa", Scheme::Custom)];
    customs.extend(Scheme::ALL.into_iter().filter(|s| *s != Scheme::Custom).map(|s| ("", s)));
    for (custom, scheme) in customs {
        let numbering = Numbering::of_scheme(scheme, Some(custom), template)
            .unwrap_or_else(|e| panic!("Schema {} ergibt keine Nummerierung: {e}", scheme.id()));
        let main = format!("{}{CHECK}{}", export_template(template, scheme, &numbering), checked_body(&numbering));
        let rendered = compile(&Source { main: main.clone(), files: Vec::new() });
        let errors: Vec<_> = rendered.diagnostics.iter().filter(|d| d.severity == Severity::Error).collect();
        assert!(
            errors.is_empty(),
            "Schema „{}“ {custom:?}: Typst zählt anders als Baruch oder setzt nicht: {errors:?}\n{main}",
            scheme.label()
        );
        assert!(!rendered.pages.is_empty(), "Schema „{}“ muss Seiten ergeben", scheme.label());
    }
}

#[test]
fn a_wrong_expectation_makes_the_check_fail() {
    // Gegenprobe: Die Prüfregel greift wirklich (sonst wäre der Test oben wertlos).
    let numbering = Numbering::of_scheme(Scheme::Decimal, None, "").unwrap();
    let main = format!("{}{CHECK}= Eins\n#baruch-check(\"2\")\n", export_template("", Scheme::Decimal, &numbering));
    let rendered = compile(&Source { main, files: Vec::new() });
    assert!(rendered.has_errors(), "„2“ statt „1“ muss scheitern: {:?}", rendered.diagnostics);
    let sermon = Numbering::of_scheme(Scheme::Sermon, None, "").unwrap();
    let main =
        format!("{}{CHECK}= Eins\n#baruch-check(\"Schluss\")\n= Zwei\n", export_template("", Scheme::Sermon, &sermon));
    let rendered = compile(&Source { main, files: Vec::new() });
    assert!(rendered.has_errors(), "Predigt: „Schluss“ statt „Einleitung“ muss scheitern (Breitenvergleich)");
    let book = Numbering::of_scheme(Scheme::Book, None, "").unwrap();
    let main =
        format!("{}{CHECK}= Eins\n#baruch-check(\"Schluss\")\n= Zwei\n", export_template("", Scheme::Book, &book));
    let rendered = compile(&Source { main, files: Vec::new() });
    assert!(rendered.has_errors(), "Buch: „Schluss“ statt „Einleitung“ muss scheitern (Breitenvergleich)");
    let main = format!("{}{CHECK}= Eins\n#baruch-check(\"Einleitung\")\n", export_template("", Scheme::Book, &book));
    let rendered = compile(&Source { main, files: Vec::new() });
    assert!(!rendered.has_errors(), "Buch mit einem Kapitel: Einleitung wie die Predigt: {:?}", rendered.diagnostics);
}
