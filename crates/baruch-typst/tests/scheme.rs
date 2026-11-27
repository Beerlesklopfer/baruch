// SPDX-License-Identifier: AGPL-3.0-or-later
//! Outline schemes, pattern language and the rule in the export (contract Round 6, section 2a–c). All templates
//! freely invented. The numbers on a whole outline (parts, nodes without number) are checked by
//! `baruch-store/tests/numbering_scheme.rs`.

use baruch_typst::scheme::{Numbering, Scheme, export_template};
use baruch_typst::{format_number, heading_numbering};

/// Counter values down to level 4 in several combinations.
const PATHS: &[&[usize]] = &[
    &[1],
    &[2],
    &[14],
    &[1, 1],
    &[3, 2],
    &[2, 27],
    &[1, 1, 1],
    &[4, 2, 3],
    &[2, 1, 28],
    &[1, 2, 3, 4],
    &[5, 4, 3, 2, 1],
];

fn numbers(numbering: &Numbering) -> Vec<String> {
    PATHS.iter().map(|path| numbering.heading(path, 9)).collect()
}

#[test]
fn every_scheme_has_an_id_a_german_label_and_an_example() {
    let ids: Vec<&str> = Scheme::ALL.iter().map(|s| s.id()).collect();
    let mut unique = ids.clone();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(unique.len(), 13, "dreizehn verschiedene ids: {ids:?}");
    for scheme in Scheme::ALL {
        assert_eq!(Scheme::from_id(scheme.id()), Some(scheme), "{:?}: id und from_id passen zusammen", scheme);
        assert!(!scheme.label().is_empty() && !scheme.example().is_empty(), "{scheme:?}: Name und Beispiel");
    }
    assert_eq!(Scheme::from_id("roemisch"), None, "unbekannte id → None");
    assert_eq!(Scheme::Legal.label(), "Juristisch", "Namen sind Deutsch");
    assert_eq!(Scheme::from_id("decimal_dot"), Some(Scheme::DecimalDot), "ids wie in Migration 0008");
}

#[test]
fn existing_patterns_give_the_same_numbers_as_before() {
    // Patterns that could be stored before Round 6: hierarchical for all levels.
    for pattern in ["1.1", "I.1", "1.a)", "A.1", "1.", "(1)", "§ A:", "i.1", "1-1"] {
        let numbering = Numbering::parse_spec(pattern).unwrap();
        for path in PATHS {
            assert_eq!(
                numbering.heading(path, 9),
                format_number(pattern, path),
                "Muster {pattern} mit {path:?}: dieselbe Nummer wie bisher (format_number)"
            );
        }
        assert_eq!(numbering.spec().as_deref(), Some(pattern), "das Muster bleibt in der Mustersprache dasselbe");
    }
}

#[test]
fn a_pattern_per_level_gives_own_numbers_and_doubled_letters() {
    let numbering = Numbering::of_scheme(Scheme::Alphanumeric, None, "").unwrap();
    let levels: Vec<String> = (1..=5).map(|level| numbering.heading(&vec![1; level], 9)).collect();
    assert_eq!(levels, ["I.", "A.", "1.", "a)", "aa)"], "I. · A. · 1. · a) · aa)");
    assert_eq!(numbering.heading(&[2, 3, 4, 5, 6], 9), "ff)", "jede Ebene nur ihre eigene Zahl, aa) zählt aa, bb … ff");
    assert_eq!(numbering.heading(&[1, 1, 1, 1, 1, 1], 9), "", "Ebenen hinter der Liste ohne Nummer");
    assert_eq!(numbering.spec().as_deref(), Some("I.;A.;1.;a);aa)"), "Mustersprache des Schemas");

    let mixed = Numbering::parse_spec("I.;;1.1").unwrap();
    assert_eq!(mixed.heading(&[2], 9), "II.", "Ebene 1 eigen");
    assert_eq!(mixed.heading(&[2, 1], 9), "", "leeres Muster: Ebene ohne Nummer");
    assert_eq!(mixed.heading(&[2, 1, 3], 9), "2.1.3", "zwei Zählzeichen: hierarchisch");

    let depth = Numbering::parse_spec("I.1|2").unwrap();
    assert_eq!((depth.heading(&[3, 2], 9), depth.heading(&[3, 2, 1], 9)), ("III.2".into(), String::new()), "P|k");
}

#[test]
fn invalid_specs_are_refused_with_a_german_reason() {
    for (spec, part) in [
        ("Ohne", "Zählzeichen"),
        ("I.;Nr.;1.", "Ebene 2"),
        ("I|0", "1 bis 6"),
        ("I|7", "1 bis 6"),
        ("I|x", "1 bis 6"),
        ("|2", "Zählzeichen"),
        ("I.;A.|2", "nicht mischen"),
    ] {
        let err = Numbering::parse_spec(spec).expect_err(spec);
        assert!(err.contains(part), "{spec}: Fehler nennt „{part}“: {err}");
    }
    assert!(Numbering::parse_spec("").unwrap().is_off(), "leer = keine Nummern");
    let err = Numbering::of_scheme(Scheme::Custom, None, "").expect_err("eigenes Muster fehlt");
    assert!(err.contains("fehlt"), "ohne eigenes Muster ein Fehler: {err}");
}

#[test]
fn the_template_scheme_reads_the_template_and_defaults_to_decimal() {
    let roman = "#set heading(numbering: \"I.1\")\n";
    assert_eq!(
        Numbering::of_scheme(Scheme::Template, None, roman).unwrap().heading(&[2, 1], 1),
        "II.1",
        "aus der Vorlage"
    );
    assert_eq!(Numbering::of_scheme(Scheme::Template, None, "").unwrap().heading(&[2, 1], 1), "2.1", "Vorgabe 1.1");
    let none = Numbering::of_scheme(Scheme::Template, None, "#set heading(numbering: none)\n").unwrap();
    assert!(none.is_off() && none.part(1).is_empty(), "none in der Vorlage: keine Nummern, auch für Teile");
    let odd = Numbering::of_scheme(Scheme::Template, None, "#set heading(numbering: \"Nr.\")\n").unwrap();
    assert_eq!(odd.heading(&[1], 1), "", "ein Vorlagenmuster ohne Zählzeichen ergibt wie bisher keine Nummer");
}

#[test]
fn the_template_export_is_unchanged_and_other_schemes_append_one_rule() {
    let template = "#set document(title: \"Probe\")\n#set heading(numbering: \"a\")\n\n";
    let numbering = Numbering::of_scheme(Scheme::Template, None, template).unwrap();
    assert_eq!(export_template(template, Scheme::Template, &numbering), template, "Vorlage byte-gleich");

    let roman = Numbering::of_scheme(Scheme::ChapterRoman, None, template).unwrap();
    let exported = export_template(template, Scheme::ChapterRoman, &roman);
    assert!(exported.starts_with(template), "die Vorlage bleibt vorn unverändert: {exported}");
    assert!(
        exported.ends_with(
            "// Baruch: Gliederungsschema „Nur Kapitel römisch“\n\
             #set heading(numbering: (..n) => if n.pos().len() <= 1 { numbering(\"I\", ..n) } else { \"\" })\n"
        ),
        "Kommentar und Regel am Ende: {exported}"
    );
    assert_eq!(heading_numbering(&exported).as_deref(), Some("I|1"), "die eigene Regel ist die letzte und gilt");

    let open = export_template("#set text(lang: \"de\")", Scheme::None, &Numbering::off());
    assert_eq!(
        open, "#set text(lang: \"de\")\n// Baruch: Gliederungsschema „Ohne Nummern“\n#set heading(numbering: none)\n",
        "ohne Zeilenende am Ende der Vorlage kommt eins dazu; Ohne = none"
    );
    let empty = export_template("", Scheme::Decimal, &Numbering::of_scheme(Scheme::Decimal, None, "").unwrap());
    assert_eq!(
        empty, "// Baruch: Gliederungsschema „Dezimal (DIN 1421)“\n#set heading(numbering: \"1.1\")\n",
        "leere Vorlage: nur Kommentar und Regel"
    );
}

#[test]
fn pattern_schemes_read_back_from_the_export_give_the_same_numbers() {
    let template = "#set document(title: \"Rundreise\")\n#set heading(numbering: \"a.a\")\n\n";
    let cases: Vec<(Scheme, Option<&str>)> = vec![
        (Scheme::Decimal, None),
        (Scheme::DecimalDot, None),
        (Scheme::Alphanumeric, None),
        (Scheme::ChapterRoman, None),
        (Scheme::ChapterArabic, None),
        (Scheme::Custom, Some("I.1")),
        (Scheme::Custom, Some("I|2")),
        (Scheme::Custom, Some("I.;;a)")),
        (Scheme::Custom, Some(";1.1")),
        (Scheme::Custom, Some("1.aa")),
        (Scheme::Custom, Some("AA|1")),
        (Scheme::Custom, Some("§ 1;(a);\"x\" 1")),
    ];
    for (scheme, custom) in cases {
        assert!(scheme.is_pattern(), "{scheme:?} beruht auf der Mustersprache");
        let numbering = Numbering::of_scheme(scheme, custom, template).unwrap();
        let exported = export_template(template, scheme, &numbering);
        let rule = numbering.heading_rule();
        assert_eq!(rule.lines().count(), 1, "{scheme:?} {custom:?}: die Regel ist eine Zeile: {rule}");
        let spec =
            heading_numbering(&exported).unwrap_or_else(|| panic!("{scheme:?} {custom:?}: Regel lesbar: {rule}"));
        let back = Numbering::parse_spec(&spec).unwrap();
        assert_eq!(numbers(&back), numbers(&numbering), "{scheme:?} {custom:?}: gelesen als {spec:?} aus {rule}");
    }
}

#[test]
fn word_schemes_give_their_numbers_and_a_typst_function() {
    let parts = Numbering::of_scheme(Scheme::PartsChapters, None, "").unwrap();
    assert_eq!(
        (parts.part(2), parts.heading(&[3], 9), parts.heading(&[3, 1], 9), parts.heading(&[3, 1, 2], 9)),
        ("Teil II".into(), "Kapitel 3".into(), "3.1".into(), "3.1.2".into()),
        "Teil II · Kapitel 3 · 3.1 · 3.1.2"
    );
    let words = Numbering::of_scheme(Scheme::ChapterWords, None, "").unwrap();
    assert_eq!(
        (words.part(1), words.heading(&[2], 9), words.heading(&[2, 1], 9), words.heading(&[100], 100)),
        ("Erster Teil".into(), "Zweites Kapitel".into(), String::new(), "100. Kapitel".into()),
        "Ordnungszahlwörter, Abschnitte ohne, ab 100 Ziffern"
    );
    let legal = Numbering::of_scheme(Scheme::Legal, None, "").unwrap();
    assert_eq!(
        (legal.part(1), legal.heading(&[4], 9), legal.heading(&[4, 2], 9), legal.heading(&[4, 2, 3], 9)),
        (String::new(), "§ 4".into(), "Abs. 2".into(), "Nr. 3".into()),
        "§ · Abs. · Nr., Teile ohne"
    );
    let sermon = Numbering::of_scheme(Scheme::Sermon, None, "").unwrap();
    let chapters: Vec<String> = (1..=4).map(|c| sermon.heading(&[c], 4)).collect();
    assert_eq!(
        chapters,
        ["Einleitung", "I.", "II.", "Schluss"],
        "erstes Einleitung, letztes Schluss, dazwischen römisch"
    );
    assert_eq!(sermon.heading(&[1], 1), "Einleitung", "nur ein Kapitel: Einleitung");
    assert_eq!(sermon.heading(&[2, 1], 4), "", "Abschnitte ohne Nummer");

    for (scheme, word) in [
        (Scheme::PartsChapters, "\"Kapitel \""),
        (Scheme::ChapterWords, "\"Neunundneunzigstes\""),
        (Scheme::Legal, "\"Abs. \""),
        (Scheme::Sermon, "\"Einleitung\""),
        (Scheme::Book, "\"Einleitung\""),
    ] {
        let rule = Numbering::of_scheme(scheme, None, "").unwrap().heading_rule();
        assert!(rule.starts_with("#set heading(numbering: (..n) =>") && rule.ends_with(")\n"), "{scheme:?}: {rule}");
        assert_eq!(rule.lines().count(), 1, "{scheme:?}: eine Zeile");
        assert!(rule.contains(word), "{scheme:?}: die Funktion schreibt {word}: {rule}");
        let opened = rule.matches(['(', '[', '{']).count();
        let closed = rule.matches([')', ']', '}']).count();
        assert_eq!(opened, closed, "{scheme:?}: Klammern ausgeglichen: {rule}");
        assert_eq!(rule.matches('"').count() % 2, 0, "{scheme:?}: Strings geschlossen: {rule}");
    }
}

#[test]
fn matching_names_the_scheme_whose_pattern_equals_the_given_one() {
    // Round 7: the outline header shows „Nur Kapitel römisch“ instead of the raw „I|1“.
    for (pattern, expected) in [
        ("I|1", Scheme::ChapterRoman),
        (" I | 1 ", Scheme::ChapterRoman),
        ("1|1", Scheme::ChapterArabic),
        ("1.1", Scheme::Decimal),
        ("1.1.", Scheme::DecimalDot),
        ("I.;A.;1.;a);aa)", Scheme::Alphanumeric),
    ] {
        assert_eq!(Scheme::matching(pattern), Some(expected), "{pattern:?} ist das Muster von {expected:?}");
    }
    for pattern in ["I", "I.1", "1.a)", "I|2", "", "   ", "kein Muster", "I;A"] {
        assert_eq!(Scheme::matching(pattern), None, "{pattern:?}: kein festes Schema (auch nie Vorlage/Eigenes/Ohne)");
    }
    for scheme in Scheme::ALL {
        let Some(spec) = Numbering::of_scheme(scheme, Some("I|1"), "").ok().and_then(|n| n.spec()) else { continue };
        if let Some(found) = Scheme::matching(&spec) {
            assert!(
                !matches!(found, Scheme::Template | Scheme::Custom | Scheme::None),
                "{scheme:?} → {found:?}: matching liefert nur feste Schemata"
            );
        }
    }
}

// ---- Round 14: Numbering::from_template ----------------------------------------------------------

/// All paths down to depth 3 with 13 chapters (≥ 12, so that Roman and word numbers run beyond X) and three
/// sections and subsections each.
fn all_paths() -> Vec<Vec<usize>> {
    let mut paths = Vec::new();
    for c in 1..=13 {
        paths.push(vec![c]);
        for s in 1..=3 {
            paths.push(vec![c, s]);
            for u in 1..=3 {
                paths.push(vec![c, s, u]);
            }
        }
    }
    paths
}

/// Numbers of a numbering: parts 1–4 and all paths for several chapter counts (sermon: „Schluss“).
fn fingerprint(numbering: &Numbering) -> Vec<String> {
    let mut out: Vec<String> = (1..=4).map(|n| numbering.part(n)).collect();
    for chapters in [1, 4, 13] {
        out.extend(all_paths().iter().map(|path| numbering.heading(path, chapters)));
    }
    out
}

#[test]
fn every_scheme_read_back_from_its_export_gives_the_same_numbers() {
    // Contract Round 14, section 3: one source for tree, typesetting and re-import. What Baruch exports must yield
    // the same numbers on re-import (scheme „aus der Werkvorlage“) – also for the schemes with words, which came
    // back as „1.1“ or „I.“ until Round 13.
    let mut cases: Vec<(Scheme, Option<&str>)> = Scheme::ALL
        .into_iter()
        .filter(|s| !matches!(s, Scheme::Template | Scheme::Custom))
        .map(|s| (s, None))
        .collect();
    for custom in ["1.aa", "AA|1", "aa)", "I.1|2", "I|1", "I.;A.;1.;a);aa)", "1;a", ";1.1", "§ 1;(a);\"x\" 1", "(i)"] {
        cases.push((Scheme::Custom, Some(custom)));
    }
    for (scheme, custom) in cases {
        let numbering = Numbering::of_scheme(scheme, custom, "").unwrap();
        let exported = export_template("", scheme, &numbering);
        let back = Numbering::from_template(&exported)
            .unwrap_or_else(|| panic!("{scheme:?} {custom:?}: die eigene Regel ist lesbar: {exported}"));
        assert_eq!(
            fingerprint(&back),
            fingerprint(&numbering),
            "{scheme:?} {custom:?}: Re-Import ergibt dieselben Nummern (Teile, Kapitel, Abschnitte) aus {exported}"
        );
        let via_scheme = Numbering::of_scheme(Scheme::Template, None, &exported).unwrap();
        assert_eq!(via_scheme, back, "{scheme:?} {custom:?}: of_scheme(Template) benutzt from_template");
    }
}

#[test]
fn word_schemes_come_back_as_themselves_not_as_an_approximation() {
    for (scheme, part, chapter, section) in [
        (Scheme::Legal, "", "§ 3", "Abs. 2"),
        (Scheme::Sermon, "", "II.", ""),
        (Scheme::Book, "Zweiter Teil", "II.", ""),
        (Scheme::PartsChapters, "Teil II", "Kapitel 3", "3.2"),
        (Scheme::ChapterWords, "Zweiter Teil", "Drittes Kapitel", ""),
    ] {
        let numbering = Numbering::of_scheme(scheme, None, "").unwrap();
        let back = Numbering::from_template(&export_template("#set text(lang: \"de\")\n", scheme, &numbering)).unwrap();
        assert_eq!(back.part(2), part, "{scheme:?}: Teile zählen wie im Schema");
        assert_eq!(back.heading(&[3], 5), chapter, "{scheme:?}: Kapitel 3 von 5 wie im Schema (nicht „1.1“/„I.“)");
        assert_eq!(back.heading(&[3, 2], 5), section, "{scheme:?}: Abschnitt wie im Schema");
        assert_eq!(back.spec(), None, "{scheme:?}: ein Schema mit Wörtern hat keine Mustersprache (wie bisher)");
    }
    // A rule that is only similar (one space different) is not Baruch's own: it is evaluated as a function per
    // level, with the result that Typst sets.
    let legal = Numbering::of_scheme(Scheme::Legal, None, "").unwrap().heading_rule().replacen("§ ", "§", 1);
    let near = Numbering::from_template(&legal).unwrap();
    assert_eq!(near.heading(&[3], 5), "§3", "abgewandelt: je Ebene ausgewertet: {legal}");
    assert_eq!(near.part(2), "II", "keine Wort-Teile: Teile römisch wie bei jeder Vorlage");
}

#[test]
fn template_patterns_with_semicolon_or_bar_are_literal_typst_patterns() {
    // In Typst `;` and `|` are ordinary text of a pattern, `aa` are two counting symbols; the pattern language of
    // Baruch only applies to own patterns, never to the work template.
    for (pattern, path, expected) in [
        ("1;", vec![2], "2;"),
        ("1;", vec![2, 3], "2;3;"),
        ("I|", vec![4], "IV|"),
        ("I|1", vec![2, 5], "II|5"),
        ("1;a", vec![3, 2], "3;b"),
        ("aa)", vec![2], "b)"),
        ("aa)", vec![2, 3], "bc)"),
    ] {
        let template = format!("#set heading(numbering: \"{pattern}\")\n");
        let numbering = Numbering::from_template(&template).unwrap();
        assert_eq!(numbering.heading(&path, 9), expected, "{pattern:?} mit {path:?} wie Typst");
        assert_eq!(numbering.heading(&path, 9), format_number(pattern, &path), "{pattern:?}: format_number = Typst");
        assert_eq!(
            numbering.heading_rule(),
            format!("#set heading(numbering: \"{pattern}\")\n"),
            "{pattern:?}: die Regel gibt das Muster wörtlich weiter"
        );
    }
    assert_eq!(Numbering::from_template("#set text(lang: \"de\")\n"), None, "ohne Regel: None");
    assert!(Numbering::from_template("#set heading(numbering: none)\n").unwrap().is_off(), "none: keine Nummern");
}

/// Numbers per level 1–4 of a template for the path `[3, 2, 4, 1]` (shortened).
fn levels_of(template: &str) -> Vec<String> {
    let numbering = Numbering::from_template(template).unwrap_or_else(|| panic!("lesbar: {template}"));
    (1..=4).map(|level| numbering.heading(&[3, 2, 4, 1][..level], 9)).collect()
}

#[test]
fn numbering_functions_are_evaluated_exactly_per_level() {
    // Freely invented templates. Path [3, 2, 4, 1]: chapter 3, section 2, subsection 4, level 4. The expected
    // values are calculated by hand by the rules of Typst (numbering = format_number).
    let table: Vec<(&str, [&str; 4], &str)> = vec![
        (
            "#set heading(numbering: (..n) => if n.pos().len() == 1 { \"Kapitel \" + str(n.pos().at(0)) } else { numbering(\"1.1\", ..n) })\n",
            ["Kapitel 3", "3.2", "3.2.4", "3.2.4.1"],
            "== mit Text und str(), sonst hierarchisch",
        ),
        (
            "#set heading(numbering: (..n) => { let p = n.pos(); if p.len() != 1 { numbering(\"a)\", p.last()) } else { numbering(\"I.\", p.first()) } })\n",
            ["III.", "b)", "d)", "a)"],
            "!= mit eigener Zahl über last() und first()",
        ),
        (
            "#set heading(numbering: (..n) => { let p = n.pos(); if p.len() < 3 { numbering(\"1.1\", ..p) } else if p.len() >= 4 { \"*\" } else { \"(\" + numbering(\"i\", p.at(2)) + \")\" } })\n",
            ["3", "3.2", "(iv)", "*"],
            "< und >= mit Verkettung und römischen Kleinbuchstaben",
        ),
        (
            "#set heading(numbering: (..n) => { let p = n.pos(); if p.len() > 2 { \"\" } else { if p.len() <= 1 { \"§ \" + str(p.at(0)) } else { str(p.at(0)) + \"/\" + str(p.at(1)) } } })\n",
            ["§ 3", "3/2", "", ""],
            "> und <= in verschachteltem if/else",
        ),
        (
            "#set heading(numbering: (..n) => { let p = n.pos(); if p.len() == 2 { numbering(\"a\", p.at(1)) * 2 + \")\" } else if p.len() == 1 { numbering(\"I\", p.at(0)) } })\n",
            ["III", "bb)", "", ""],
            "verdoppelter Buchstabe als numbering(…) * 2, fehlendes else = ohne Nummer",
        ),
        (
            "#let zaehlung(..n) = if n.pos().len() == 1 { \"Teilstück \" + numbering(\"A\", n.pos().at(0)) } else { numbering(\"1.1\", ..n) }\n#set heading(numbering: zaehlung)\n",
            ["Teilstück C", "3.2", "3.2.4", "3.2.4.1"],
            "#let-Funktion über ihren Namen",
        ),
        (
            "#let zaehlung = (..z) => {\n  let q = z.pos()\n  if q.len() == 1 { numbering(\"1;\", ..q) } else { \"|\" + str(q.at(1)) }\n}\n#set heading(numbering: zaehlung)\n",
            ["3;", "|2", "|2", "|2"],
            "#let name = (..z) => {…} über Zeilen; ; und | im Muster sind Text",
        ),
    ];
    for (template, expected, why) in table {
        assert_eq!(levels_of(template), expected, "{why}: {template}");
    }
}

#[test]
fn an_evaluated_function_keeps_the_old_approximation_for_the_display() {
    // spec() stays the approximation of heading_numbering: outline header and display (store/outline.rs) show the
    // same as before, the numbers themselves are exact.
    let template = "#set heading(numbering: (..n) => if n.pos().len() == 1 { \"Kapitel \" + str(n.pos().at(0)) } else { numbering(\"1.1\", ..n) })\n";
    let numbering = Numbering::from_template(template).unwrap();
    assert_eq!(numbering.spec(), heading_numbering(template), "spec() = bisherige Näherung");
    assert_eq!(numbering.spec().as_deref(), Some("1.1"), "erstes numbering-Muster");
    assert_eq!(numbering.heading(&[2], 1), "Kapitel 2", "die Nummer selbst ist exakt");
    let rule = numbering.heading_rule();
    let again = Numbering::from_template(&rule).unwrap();
    assert_eq!(fingerprint(&again), fingerprint(&numbering), "die erzeugte Regel ergibt dieselben Nummern: {rule}");
}

#[test]
fn functions_that_cannot_be_evaluated_fall_back_to_the_old_approximation() {
    for (template, why) in [
        (
            "#set heading(numbering: (..zahlen) => {\n  let z = zahlen.pos()\n  if z.len() == 1 { numbering(\"I\", ..z) } else { numbering(\"1.1\", ..z.slice(1)) }\n})\n",
            "slice() kennt der Leser nicht",
        ),
        (
            "#set heading(numbering: (..n) => numbering(\"I\", ..n).replace(\"I\", \"J\"))\n",
            "Methode hinter dem Aufruf",
        ),
        (
            "#set heading(numbering: (..n) => if n.pos().len() == 2 { numbering(\"1\", n.pos().at(3)) })\n",
            "Stelle fehlt",
        ),
        ("#set heading(numbering: (..n) => numbering(\"Nr.\", ..n))\n", "Muster ohne Zählzeichen (Fehler in Typst)"),
    ] {
        let approx = heading_numbering(template);
        assert!(approx.is_some(), "{why}: die alte Näherung findet ein Muster");
        assert_eq!(
            Numbering::from_template(template),
            approx.as_deref().map(Numbering::lenient),
            "{why}: wie bisher die Näherung {approx:?}"
        );
    }
    assert_eq!(
        Numbering::from_template("#set heading(numbering: (..n) => n.pos().map(str).join(\".\"))\n"),
        None,
        "ohne jedes numbering-Muster: None, die Vorgabe 1.1 gilt dann über of_scheme"
    );
}

#[test]
fn huge_repetitions_in_a_template_function_are_not_evaluated() {
    // Finding of the review of Round 14: `* k` from a foreign template was repeated without an upper limit; `*
    // 40000000000` aborted the process at the memory allocation (on import and on every load).
    let huge = "#set heading(numbering: (..n) => str(n.pos().at(0)) * 40000000000)\n";
    let numbering = Numbering::from_template(huge);
    assert_eq!(
        numbering,
        heading_numbering(huge).as_deref().map(Numbering::lenient),
        "zu große Wiederholung: nicht ausgewertet, sondern wie bisher die Näherung (hier keine) – kein Abbruch"
    );
    let three = "#set heading(numbering: (..n) => str(n.pos().at(0)) * 3)\n";
    assert_eq!(
        levels_of(three),
        ["333", "333", "333", "333"],
        "kleine Wiederholung bleibt exakt ausgewertet (Grenze nicht zu eng)"
    );
    let limit = "#set heading(numbering: (..n) => str(n.pos().at(0)) * 16)\n";
    assert_eq!(levels_of(limit)[0], "3".repeat(16), "16 ist die Grenze und noch auswertbar");
    for (template, why) in [
        ("#set heading(numbering: (..n) => str(n.pos().at(0)) * 17)\n", "17 liegt über der Grenze"),
        (
            "#set heading(numbering: (..n) => numbering(\"a\", n.pos().at(0)) * 99999999999999999999999)\n",
            "Zahl über usize: nicht lesbar statt Überlauf",
        ),
        ("#set heading(numbering: (..n) => \"x\" * 1000000000000)\n", "auch reiner Text wird nicht riesig wiederholt"),
    ] {
        assert_eq!(
            Numbering::from_template(template),
            heading_numbering(template).as_deref().map(Numbering::lenient),
            "{why}: Näherung statt exakter Auswertung"
        );
    }
}

// ---- Scheme „Buch“ ---------------------------------------------------------------------------------

#[test]
fn the_book_scheme_numbers_chapters_like_the_sermon_and_parts_as_words() {
    assert_eq!(Scheme::from_id("book"), Some(Scheme::Book), "id book");
    assert_eq!(Scheme::Book.label(), "Buch", "Name Buch");
    let position = |s| Scheme::ALL.iter().position(|x| *x == s).unwrap();
    assert_eq!(position(Scheme::Book), position(Scheme::Sermon) + 1, "direkt hinter Predigt in der Auswahl");
    assert!(!Scheme::Book.is_pattern(), "ein Schema mit Wörtern, keine Mustersprache");

    let book = Numbering::of_scheme(Scheme::Book, None, "").unwrap();
    let sermon = Numbering::of_scheme(Scheme::Sermon, None, "").unwrap();
    let chapters: Vec<String> = (1..=5).map(|c| book.heading(&[c], 5)).collect();
    assert_eq!(chapters, ["Einleitung", "I.", "II.", "III.", "Schluss"], "Einleitung · I. · II. · III. · Schluss");
    for chapters in [1, 2, 4, 13] {
        for path in all_paths() {
            assert_eq!(
                book.heading(&path, chapters),
                sermon.heading(&path, chapters),
                "{path:?} von {chapters}: wie Predigt"
            );
        }
    }
    assert_eq!(book.heading(&[1], 1), "Einleitung", "nur ein Kapitel: Einleitung wie bei der Predigt");
    assert_eq!(sermon.heading(&[1], 1), "Einleitung", "Predigt mit einem Kapitel: Einleitung");
    assert_eq!(book.heading(&[2, 1], 4), "", "Abschnitte ohne Nummer");
    let parts: Vec<String> = (1..=4).map(|n| book.part(n)).collect();
    assert_eq!(parts, ["Erster Teil", "Zweiter Teil", "Dritter Teil", "Vierter Teil"], "Teile als Wörter");
    assert_eq!(sermon.part(1), "", "die Predigt zählt Teile weiter nicht");
}

#[test]
fn the_book_rule_differs_from_the_sermon_rule_and_comes_back_as_book() {
    let book = Numbering::of_scheme(Scheme::Book, None, "").unwrap();
    let sermon = Numbering::of_scheme(Scheme::Sermon, None, "").unwrap();
    assert_ne!(book.heading_rule(), sermon.heading_rule(), "eigene Regel für Buch");
    assert_ne!(book, sermon, "zwei verschiedene Nummerierungen");
    let exported = export_template("#set text(lang: \"de\")\n", Scheme::Book, &book);
    assert!(exported.contains("// Baruch: Gliederungsschema „Buch“\n"), "Kommentar nennt das Schema: {exported}");
    assert_eq!(Numbering::from_template(&exported), Some(book.clone()), "Buch-Stand wird wieder Buch");
    let exported = export_template("", Scheme::Sermon, &sermon);
    assert_eq!(Numbering::from_template(&exported), Some(sermon), "Predigt-Stand bleibt Predigt");
    // Even without the comment (as in a template whose comments were removed) the rule counts.
    let bare = book.heading_rule();
    assert_eq!(Numbering::from_template(&bare).map(|n| n.part(3)), Some("Dritter Teil".to_owned()), "{bare}");
}
