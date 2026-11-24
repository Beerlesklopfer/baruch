// SPDX-License-Identifier: AGPL-3.0-or-later
//! Runde 29b: Werkvorlage gegen Satz-Einstellungen (Vertrag §5.3/§5.4) – Konflikte je gesetztem Feld mit Zeile, Text
//! und Wirkung, Kommentare zählen nicht; Kapitelanfangs-Regel der Vorlage erkennen (kanonisch und abgewandelt) und
//! genau ihre Zeilen entfernen. Alle Vorlagen frei erfunden.

use baruch_typst::typeset::*;

fn all_set() -> TypesetSettings {
    TypesetSettings {
        chapter_start: Some(ChapterStart::RightPage),
        page_format: Some(PageFormat::A5),
        margins: Some(Margins { inside_mm: 20.0, outside_mm: 15.0, top_mm: 18.0, bottom_mm: 22.0, binding_mm: 5.0 }),
        font: Some("Libertinus Serif".into()),
        font_size_pt: Some(10.5),
        leading_em: Some(0.65),
        paragraph: Some(Paragraph::Indent { em: 1.0 }),
        justify: Some(true),
        hyphenate: Some(true),
        lang: Some("de".into()),
        page_numbers: Some(PageNumbers {
            position: PageNumberPosition::BottomOutside,
            start: PageNumberStart::FirstChapter,
        }),
        running_head: Some(RunningHead::Chapter),
        footnotes: Some(Footnotes { numbering: FootnoteNumbering::Arabic, restart: FootnoteRestart::PerChapter }),
        widows_orphans: Some(true),
        ..Default::default()
    }
}

/// Erfundene Vorlage, die fast alles selbst setzt (Zeilen 1–9 und 13), dazu Auskommentiertes (Zeilen 10–12).
const BUSY: &str = "#set page(paper: \"a4\", margin: (x: 2cm, y: 3cm), numbering: \"1\")\n\
#set page(header: [Kopf])\n\
#set text(font: \"New Computer Modern\", size: 11pt, lang: \"en\", hyphenate: false)\n\
#set text(costs: (widow: 0%))\n\
#set par(\n  leading: 0.8em,\n  justify: false,\n  first-line-indent: 2em)\n\
#set footnote(numbering: \"i\")\n\
// #set page(paper: \"a5\")\n\
/* #set text(size: 9pt)\n#set par(spacing: 1em) */\n\
#show heading.where(level: 1): it => { pagebreak(weak: true); it }\n";

#[test]
fn every_set_field_reports_its_template_line() {
    let conflicts = template_conflicts(BUSY, &all_set());
    let found: Vec<(&str, usize)> = conflicts.iter().map(|c| (c.field, c.line)).collect();
    let expected: Vec<(&str, usize)> = vec![
        ("page_format", 1),
        ("margins", 1),
        ("page_numbers", 1),
        ("running_head", 2),
        ("font", 3),
        ("font_size_pt", 3),
        ("hyphenate", 3),
        ("lang", 3),
        ("widows_orphans", 4),
        ("leading_em", 6),
        ("justify", 7),
        ("paragraph", 8),
        ("footnotes", 9),
        ("chapter_start", 13),
    ];
    assert_eq!(
        found, expected,
        "je Feld die Zeile des Arguments, nach Zeile sortiert; Kommentare (10–12) zählen nicht"
    );
    let page = &conflicts[0];
    assert_eq!(page.text, "#set page(paper: \"a4\", margin: (x: 2cm, y: 3cm), numbering: \"1\")", "Text der Zeile");
    assert_eq!(page.effect, ConflictEffect::Overrides, "set-Regeln überstimmt die Einstellung dahinter");
    assert_eq!(
        page.message,
        "Die Werkvorlage setzt Seitenformat selbst (Zeile 1); die Einstellung gilt, weil Baruch sie dahinter setzt.",
        "Meldung nach Vertrag §5.3"
    );
    // Vorlage bricht auf neue Seite, Einstellung verlangt die rechte Seite: beide Umbrüche fallen zusammen, die
    // Einstellung gilt.
    assert_eq!(
        conflicts.last().map(|c| c.effect),
        Some(ConflictEffect::Overrides),
        "new_page in der Vorlage stört right_page nicht"
    );
}

#[test]
fn unset_fields_report_nothing() {
    assert_eq!(template_conflicts(BUSY, &TypesetSettings::default()), Vec::new(), "ohne Einstellungen keine Konflikte");
    let only_font = TypesetSettings { font: Some("Libertinus Serif".into()), ..Default::default() };
    let conflicts = template_conflicts(BUSY, &only_font);
    assert_eq!(conflicts.len(), 1, "nur das gesetzte Feld: {conflicts:?}");
    assert_eq!((conflicts[0].field, conflicts[0].line), ("font", 3), "Schrift in Zeile 3");
}

#[test]
fn a_template_page_break_makes_continuous_ineffective() {
    let template = format!("#set text(lang: \"de\")\n{CANONICAL_CHAPTER_BREAK}\n");
    let continuous = TypesetSettings { chapter_start: Some(ChapterStart::Continuous), ..Default::default() };
    let conflicts = template_conflicts(&template, &continuous);
    assert_eq!(conflicts.len(), 1, "eine Show-Regel: {conflicts:?}");
    let c = &conflicts[0];
    assert_eq!((c.field, c.line, c.effect), ("chapter_start", 2, ConflictEffect::Ineffective), "{c:?}");
    assert_eq!(
        c.message,
        "Die Werkvorlage setzt Kapitelanfang selbst (Zeile 2); die Einstellung wirkt nicht, solange die Zeile in der \
         Vorlage steht.",
        "Meldung nach Vertrag §5.3"
    );
    assert_eq!(c.text, CANONICAL_CHAPTER_BREAK, "Text der Zeile");
    let new_page = TypesetSettings { chapter_start: Some(ChapterStart::NewPage), ..Default::default() };
    assert_eq!(
        template_conflicts(&template, &new_page)[0].effect,
        ConflictEffect::Ineffective,
        "neue Seite gegen to: \"odd\" der Vorlage: die Vorlage zwingt weiter auf rechte Seiten"
    );
    let right = TypesetSettings { chapter_start: Some(ChapterStart::RightPage), ..Default::default() };
    assert_eq!(template_conflicts(&template, &right)[0].effect, ConflictEffect::Overrides, "gleicher Wert: gilt");
    let even = "#show heading.where(level: 1): it => { pagebreak(to: \"even\"); it }\n";
    assert_eq!(template_conflicts(even, &right)[0].effect, ConflictEffect::Ineffective, "to: \"even\" widerspricht");
}

#[test]
fn page_header_counts_for_numbers_only_on_top_and_background_only_for_first_chapter() {
    let template = "#set page(header: [Kopf], background: none)\n";
    let bottom = TypesetSettings {
        page_numbers: Some(PageNumbers {
            position: PageNumberPosition::BottomCenter,
            start: PageNumberStart::FirstPage,
        }),
        ..Default::default()
    };
    assert_eq!(
        template_conflicts(template, &bottom),
        Vec::new(),
        "Zahl unten, Zählung ab Seite 1: Kopf und Hintergrund frei"
    );
    let top = TypesetSettings {
        page_numbers: Some(PageNumbers {
            position: PageNumberPosition::TopOutside,
            start: PageNumberStart::FirstChapter,
        }),
        ..Default::default()
    };
    let fields: Vec<&str> = template_conflicts(template, &top).iter().map(|c| c.field).collect();
    assert_eq!(
        fields,
        ["page_numbers"],
        "oben außen setzt die Kopfzeile, ab dem ersten Kapitel den Hintergrund (eine Meldung je Zeile)"
    );
}

#[test]
fn set_rules_inside_template_functions_count() {
    let template =
        "#let buch(doc) = {\n  set page(width: 12cm, height: 19cm)\n  set text(size: 10pt); doc\n}\n#show: buch\n";
    let s = TypesetSettings { page_format: Some(PageFormat::A5), font_size_pt: Some(11.0), ..Default::default() };
    let found: Vec<(&str, usize)> = template_conflicts(template, &s).iter().map(|c| (c.field, c.line)).collect();
    assert_eq!(found, [("page_format", 2), ("font_size_pt", 3)], "set im Code einer Vorlagenfunktion zählt");
    let prose = "Ein Satz über offset und reset (nicht set page(…)) bleibt Text.\n";
    assert_eq!(template_conflicts(prose, &s).len(), 0, "„set“ als Wortteil oder im Fließtext zählt nicht");
}

// ---- Kapitelanfangs-Regel erkennen -----------------------------------------------------------------

#[test]
fn variants_of_the_chapter_rule_are_detected() {
    let cases: [(&str, ChapterStart); 7] = [
        ("#show heading.where(level: 1): it => { pagebreak(weak: true, to: \"odd\"); it }", ChapterStart::RightPage),
        ("#show heading.where( level:1 ):it=>{pagebreak(to:\"odd\",weak:true);it}", ChapterStart::RightPage),
        ("#show heading.where(level: 1): it => [#pagebreak(weak: true, to: \"odd\") #it]", ChapterStart::RightPage),
        (
            "#show heading.where(level: 1): it => {\n  pagebreak(weak: true, to: \"odd\")\n  it\n}",
            ChapterStart::RightPage,
        ),
        ("#show heading.where(level: 1): it => { pagebreak(to: \"odd\"); it }", ChapterStart::RightPage),
        ("#show heading.where(level: 1): it => { pagebreak(weak: true); it }", ChapterStart::NewPage),
        ("#show heading.where(level: 1): kapitel => { pagebreak(); kapitel }", ChapterStart::NewPage),
    ];
    for (rule, want) in cases {
        let template = format!("#set text(lang: \"de\")\n{rule}\n#set par(justify: true)\n");
        let found = detect_chapter_break(&template).unwrap_or_else(|| panic!("nicht erkannt: {rule}"));
        assert_eq!(found.chapter_start, want, "{rule}");
        assert_eq!(found.line, 2, "erste Zeile der Regel: {rule}");
        assert_eq!(found.text, rule, "Regeltext ohne Zeilenende (bei mehrzeiligen Regeln alle Zeilen): {rule}");
        assert_eq!(
            remove_rule(&template, &found),
            "#set text(lang: \"de\")\n#set par(justify: true)\n",
            "remove_rule entfernt genau die Zeile(n) der Regel: {rule}"
        );
    }
}

#[test]
fn rules_that_do_more_or_other_things_are_not_detected() {
    let not: [&str; 6] = [
        "#show heading.where(level: 1): it => { pagebreak(to: \"even\"); it }",
        "#show heading.where(level: 2): it => { pagebreak(); it }",
        "#show heading.where(level: 1): it => { pagebreak(); text(red, it) }",
        "#show heading.where(level: 1): it => { set text(size: 20pt); pagebreak(); it }",
        "#show heading.where(level: 1, outlined: true): it => { pagebreak(); it }",
        "#show heading.where(level: 1): it => { pagebreak(); it } #set text(lang: \"de\")",
    ];
    for rule in not {
        let template = format!("{rule}\n");
        assert_eq!(detect_chapter_break(&template), None, "nicht übernehmbar (Entfernen verlöre etwas): {rule}");
    }
    let commented = format!("// {CANONICAL_CHAPTER_BREAK}\n/* {CANONICAL_CHAPTER_BREAK}\n*/\n");
    assert_eq!(detect_chapter_break(&commented), None, "Kommentare zählen nicht");
    // Auch ohne erkennbare Regel meldet template_conflicts die Zeile (hier to: "even").
    let right = TypesetSettings { chapter_start: Some(ChapterStart::RightPage), ..Default::default() };
    assert_eq!(template_conflicts(&format!("{}\n", not[0]), &right).len(), 1, "Konflikt trotzdem gemeldet");
}

#[test]
fn rule_without_trailing_line_end_and_with_crlf() {
    let template = format!("#set text(lang: \"de\")\r\n{CANONICAL_CHAPTER_BREAK}");
    let found = detect_chapter_break(&template).expect("auch als letzte Zeile ohne Zeilenende");
    assert_eq!(found.end, template.len(), "bis zum Ende der Vorlage");
    assert_eq!(remove_rule(&template, &found), "#set text(lang: \"de\")\r\n", "die Zeile davor bleibt samt CRLF");
    let crlf = format!("{CANONICAL_CHAPTER_BREAK}\r\n#set par(justify: true)\r\n");
    let found = detect_chapter_break(&crlf).expect("mit CRLF");
    assert_eq!(found.text, CANONICAL_CHAPTER_BREAK, "Text ohne \\r");
    assert_eq!(remove_rule(&crlf, &found), "#set par(justify: true)\r\n", "Zeile samt CRLF entfernt");
}
