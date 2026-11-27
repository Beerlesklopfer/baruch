// SPDX-License-Identifier: AGPL-3.0-or-later
//! Round 29b: form of the rule block `rules` (contract §5.1): empty without settings, otherwise first line
//! `RULES_MARK`, rules in FIELDS order, end with a line break, one recognisable rule per setting. The effect in the
//! typesetting is measured by `baruch-render/tests/typeset_measure.rs`.

use baruch_typst::typeset::*;

/// The contract example (§2): all 16 fields set.
const EXAMPLE: &str = r#"{"chapter_start":"right_page","part_page":{"side":"right","blank_back":true},"page_format":{"kind":"a5"},"margins":{"inside_mm":20.0,"outside_mm":15.0,"top_mm":18.0,"bottom_mm":22.0,"binding_mm":5.0},"font":"Libertinus Serif","font_size_pt":10.5,"leading_em":0.65,"paragraph":{"kind":"indent","em":1.0},"justify":true,"hyphenate":true,"lang":"de","page_numbers":{"position":"bottom_outside","start":"first_chapter"},"running_head":"chapter","footnotes":{"numbering":"arabic","restart":"per_chapter"},"widows_orphans":true,"xref_with_page":true}"#;

fn example() -> TypesetSettings {
    TypesetSettings::from_json(EXAMPLE).expect("Vertragsbeispiel lesbar")
}

/// Distinguishing mark per field (part of the line the field produces); `xref_with_page` produces no rule.
fn marker(field: &str) -> Option<&'static str> {
    Some(match field {
        "chapter_start" => "pagebreak(weak: true, to: \"odd\")",
        "part_page" => "// Teil-Seite:",
        "page_format" => "#set page(width: 148mm, height: 210mm)",
        "margins" => "#set page(margin: (inside: 25mm",
        "font" => "#set text(font: \"Libertinus Serif\")",
        "font_size_pt" => "#set text(size: 10.5pt)",
        "leading_em" => "#set par(leading: 0.65em)",
        "paragraph" => "#set par(first-line-indent: 1em",
        "justify" => "#set par(justify: true)",
        "hyphenate" => "#set text(hyphenate: true)",
        "lang" => "#set text(lang: \"de\")",
        "page_numbers" => "#set page(numbering: \"1\", footer:",
        "running_head" => "#set page(header:",
        "footnotes" => "#set footnote(numbering: \"1\")",
        "widows_orphans" => "#set text(costs: (widow: 100%, orphan: 100%))",
        _ => return None,
    })
}

#[test]
fn empty_settings_give_an_empty_block() {
    assert_eq!(rules(&TypesetSettings::default()), "", "ohne Einstellungen keine Regel – Export byte-gleich");
    let only_xref = TypesetSettings { xref_with_page: Some(false), ..Default::default() };
    assert_eq!(rules(&only_xref), "", "„Verweis mit Seitenzahl“ ist keine Satz-Regel und ändert keine Datei");
}

#[test]
fn the_block_starts_with_the_mark_and_ends_with_a_line_break() {
    let block = rules(&example());
    assert_eq!(block.lines().next(), Some(RULES_MARK), "erste Zeile ist die Marke „// Baruch: Satz“");
    assert!(block.ends_with('\n'), "der Block endet mit Zeilenumbruch, damit der Text dahinter neu beginnt");
    assert!(!block.contains("\n\n"), "keine Leerzeilen im Block (eine Regel je Zeile)");
}

#[test]
fn every_field_has_a_recognisable_rule_in_fields_order() {
    let block = rules(&example());
    let mut last = 0usize;
    for (field, label) in FIELDS {
        let Some(marker) = marker(field) else {
            // R31 (§2.2): glossary page, index pages and first mention control the back matter, no rule.
            // R22 (§2.9): the draft only goes into the typesetting source of the outputs, never into the rule block.
            assert!(
                matches!(
                    field,
                    "xref_with_page" | "glossary_page" | "index_pages" | "glossary_first_mention" | "draft"
                ),
                "nur der Verweis-Schalter, die drei R31-Felder und der Entwurf haben keine Regel: {field}"
            );
            continue;
        };
        let at = block.find(marker).unwrap_or_else(|| panic!("{label} ({field}): Regel {marker:?} fehlt in\n{block}"));
        assert!(at >= last, "{label} ({field}) steht in FIELDS-Reihenfolge (Stelle {at} nach {last})\n{block}");
        last = at;
    }
}

#[test]
fn each_setting_alone_gives_exactly_its_rule() {
    let full = example();
    let singles: Vec<(&str, TypesetSettings)> = vec![
        ("chapter_start", TypesetSettings { chapter_start: full.chapter_start, ..Default::default() }),
        ("part_page", TypesetSettings { part_page: full.part_page.clone(), ..Default::default() }),
        ("page_format", TypesetSettings { page_format: full.page_format, ..Default::default() }),
        ("font", TypesetSettings { font: full.font.clone(), ..Default::default() }),
        ("font_size_pt", TypesetSettings { font_size_pt: full.font_size_pt, ..Default::default() }),
        ("leading_em", TypesetSettings { leading_em: full.leading_em, ..Default::default() }),
        ("paragraph", TypesetSettings { paragraph: full.paragraph, ..Default::default() }),
        ("justify", TypesetSettings { justify: full.justify, ..Default::default() }),
        ("hyphenate", TypesetSettings { hyphenate: full.hyphenate, ..Default::default() }),
        ("lang", TypesetSettings { lang: full.lang.clone(), ..Default::default() }),
        ("footnotes", TypesetSettings { footnotes: full.footnotes.clone(), ..Default::default() }),
        ("widows_orphans", TypesetSettings { widows_orphans: full.widows_orphans, ..Default::default() }),
    ];
    for (field, settings) in singles {
        let block = rules(&settings);
        let own = marker(field).expect("Feld mit Regel");
        assert!(block.starts_with(&format!("{RULES_MARK}\n")), "{field}: Block mit Marke");
        assert!(block.contains(own), "{field}: eigene Regel {own:?} in\n{block}");
        for other in FIELDS.iter().map(|(id, _)| *id).filter(|id| *id != field) {
            if let Some(m) = marker(other) {
                assert!(!block.contains(m), "{field} allein erzeugt keine Regel für {other} ({m:?})\n{block}");
            }
        }
    }
}

#[test]
fn chapter_rules_are_exactly_the_adoptable_template_rules() {
    // Finding R29: the chapter rule applies, like the template rule that can be taken over, to every heading of
    // level 1 (also parts) – otherwise „Vorlagenregel übernehmen“ would lose the break before parts in the export.
    // Therefore `detect_chapter_break` recognises the own line again, with the same value.
    for start in [ChapterStart::NewPage, ChapterStart::RightPage] {
        let block = rules(&TypesetSettings { chapter_start: Some(start), ..Default::default() });
        let line = block.lines().find(|l| l.contains("pagebreak")).expect("Kapitelregel");
        assert!(!line.contains("<baruch-part>"), "{start:?}: keine Ausnahme für Teile: {line}");
        let found = baruch_typst::typeset::detect_chapter_break(line).expect("Regel in übernehmbarer Form");
        assert_eq!(found.chapter_start, start, "{start:?}: die Zeile bewirkt denselben Kapitelanfang");
    }
    assert_eq!(
        rules(&TypesetSettings { chapter_start: Some(ChapterStart::RightPage), ..Default::default() })
            .lines()
            .find(|l| l.contains("pagebreak")),
        Some(baruch_typst::typeset::CANONICAL_CHAPTER_BREAK),
        "rechte Seite = genau die kanonische Vorlagenregel"
    );
    let continuous = rules(&TypesetSettings { chapter_start: Some(ChapterStart::Continuous), ..Default::default() });
    assert!(!continuous.contains("pagebreak"), "fortlaufend: kein Umbruch: {continuous}");
    assert!(continuous.contains("Kapitelanfang: fortlaufend"), "fortlaufend ist trotzdem als Zeile erkennbar");
}

#[test]
fn page_numbers_and_running_head_variants() {
    let with = |position, start, head| TypesetSettings {
        page_numbers: Some(PageNumbers { position, start }),
        running_head: head,
        ..Default::default()
    };
    let hidden = rules(&with(PageNumberPosition::Hidden, PageNumberStart::FirstPage, None));
    assert!(hidden.contains("#set page(numbering: none, footer: none)"), "keine Zahlen: {hidden}");
    assert!(!hidden.contains("counter(page).update"), "ohne Zahlen keine Rücksetzung: {hidden}");
    let first = rules(&with(PageNumberPosition::BottomCenter, PageNumberStart::FirstChapter, None));
    assert!(first.contains("align(center,"), "unten mittig: {first}");
    assert!(first.contains("#set page(background: context"), "Rücksetzung im Seitenhintergrund: {first}");
    let top = rules(&with(PageNumberPosition::TopOutside, PageNumberStart::FirstPage, Some(RunningHead::Chapter)));
    let header_lines = top.lines().filter(|l| l.contains("header:")).count();
    assert_eq!(header_lines, 2, "oben außen und Kolumnentitel: die Kopfzeile des Titels trägt die Zahl mit: {top}");
    assert!(top.lines().last().is_some_and(|l| l.contains("c.last().body") && l.contains("let n = if")), "{top}");
    let off = rules(&TypesetSettings { running_head: Some(RunningHead::Off), ..Default::default() });
    assert!(off.contains("#set page(header: none)"), "Kolumnentitel aus: Kopfzeile leer: {off}");
    let title = rules(&TypesetSettings { running_head: Some(RunningHead::WorkTitle), ..Default::default() });
    assert!(title.contains("document.title"), "Werktitel aus #set document(title: …): {title}");
}

#[test]
fn font_names_are_typst_string_literals() {
    // validate refuses " and \; rules escapes nevertheless, so that Typst code never arises from a name.
    let block = rules(&TypesetSettings { font: Some("A\"B\\C".into()), ..Default::default() });
    assert!(block.contains("#set text(font: \"A\\\"B\\\\C\")"), "maskiert: {block}");
}

#[test]
fn region_is_split_from_the_language() {
    let block = rules(&TypesetSettings { lang: Some("de-CH".into()), ..Default::default() });
    assert!(block.contains("#set text(lang: \"de\", region: \"CH\")"), "Sprache und Region getrennt: {block}");
}

#[test]
fn part_page_rule_with_none_is_byte_identical_and_some_changes_side_and_back() {
    for labels in [vec![], vec!["I".to_owned()], vec!["I".to_owned(), String::new(), "III".to_owned()]] {
        assert_eq!(
            part_page_rule_with(&labels, None),
            baruch_typst::part_page_rule(&labels),
            "ohne Einstellung byte-gleich zur Teil-Regel von Runde 14 ({labels:?})"
        );
    }
    let labels = vec!["I".to_owned()];
    let default = baruch_typst::part_page_rule(&labels);
    let any = part_page_rule_with(&labels, Some(&PartPage { side: PartSide::Any, blank_back: false }));
    assert!(!any.contains("to: \"odd\""), "beliebige Seite: kein Umbruch auf ungerade Seiten: {any}");
    assert!(any.contains("<baruch-satz-end>"), "Marke vor dem Umbruch (Leerseiten davor erkennen): {any}");
    let right = part_page_rule_with(&labels, Some(&PartPage { side: PartSide::Right, blank_back: true }));
    assert!(right.contains("pagebreak(weak: true, to: \"odd\")"), "rechte Seite: {right}");
    assert!(
        right.contains("pagebreak(); [#metadata(\"baruch-satz\") <baruch-satz-blank>]; pagebreak() }"),
        "leere Rückseite: {right}"
    );
    // Counter, number and title of the part page stay verbatim as in the standard rule.
    let middle_start = default.find("counter(\"baruch-part\").step()").expect("Zähler");
    let middle_end = default.rfind("v(1fr); ").expect("unterer Leerraum") + "v(1fr); ".len();
    assert!(right.contains(&default[middle_start..middle_end]), "Mitte der Regel unverändert:\n{right}\n{default}");
    assert!(right.ends_with('\n') && right.starts_with("#show <baruch-part>: it => {"), "Form wie die Standardregel");
}

#[test]
fn preview_source_holds_template_rules_part_rule_and_sample() {
    let s = example();
    let source = preview_source(&s, "#set page(paper: \"a5\")", "Erfundenes Buch");
    let rules_at = source.find(RULES_MARK).expect("Regelblock in der Vorschau");
    let part_rule_at = source.find("#show <baruch-part>").expect("Teil-Regel in der Vorschau");
    let part_at = source.find("<baruch-part>\n").expect("Teil-Zeile");
    assert!(source.starts_with("#set page(paper: \"a5\")\n"), "Werkvorlage zuerst");
    assert!(rules_at < part_rule_at && part_rule_at < part_at, "Vorlage, Regeln, Teil-Regel, Text – wie im Buchsatz");
    assert!(
        source[part_rule_at..].contains("<baruch-satz-blank>"),
        "die Teil-Regel folgt der Einstellung (Rückseite leer)"
    );
}
