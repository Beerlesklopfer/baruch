// SPDX-License-Identifier: AGPL-3.0-or-later
//! The thirteen outline schemes (`baruch_typst::scheme::export_template`) typeset with Typst without
//! errors (backlog: „nicht mit typst compile geprüft“, contract Round 8, section 4e), and since
//! Round 14 (contract section 3/4) Typst counts like Baruch: per heading the test text itself checks
//! with `#context assert.eq(…)` that Typst sets the same number as `Numbering::heading` – with parts
//! and chapters without number as markers (`baruch_typst::part_line`, `unnumbered_line`).

use baruch_render::{Severity, Source, compile};
use baruch_typst::scheme::{Numbering, Scheme, export_template};

/// Invented outline with part, chapters, sections and subsections – four chapters, so that
/// „Predigt“ and „Buch“ form Einleitung, I., II. and Schluss.
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
    // "I|1" (parts Roman, chapters Arabic) is the second form of the pattern language.
    let numbering = Numbering::of_scheme(Scheme::Custom, Some("I|1"), "").expect("Muster I|1 ist gültig");
    let main = format!("{}{BODY}", export_template("", Scheme::Custom, &numbering));
    let rendered = compile(&Source { main, files: Vec::new() });
    assert!(!rendered.has_errors(), "„I|1“ muss fehlerfrei setzen: {:?}", rendered.diagnostics);
}

// ---- Round 14: Typst counts like Baruch ----------------------------------------------------------

/// Outline (level 0 = part, 1–3; with number?; title), freely invented: two parts, a preface and
/// an afterword without number, a section without number between numbered ones, 13 numbered chapters
/// (≥ 12: Roman numerals and ordinal words beyond X).
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

/// Counter values like `baruch-store` (`load::numbers`): chapters consecutive across parts,
/// sections per parent, items without number do not count. `None` for parts and items without number.
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

/// Typst string (`\` and `"` escaped).
fn typst_str(text: &str) -> String {
    format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""))
}

/// Check rule in the test text: the number that Typst forms for the heading set last is
/// `expected`. A function that delivers content (Predigt: `context { … }` for „Schluss“) cannot be
/// compared as text; then the typeset content must be exactly as wide as the expected text.
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
    // Contract Round 14, section 3: one source for tree and typesetting. Every heading carries a
    // `#context assert.eq(…)` check with the number from Numbering::heading; if Typst sets another one,
    // the typesetting aborts with an error. Parts and items without number are markers in the text
    // and do not count in Typst (as in Baruch).
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
    // Cross-check: the check rule really takes effect (otherwise the test above would be worthless).
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
