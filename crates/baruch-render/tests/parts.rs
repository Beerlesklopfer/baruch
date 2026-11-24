// SPDX-License-Identifier: AGPL-3.0-or-later
//! Runde 14: Teil-Markierungen im Satz. Typst prüft selbst (`#context assert…`): Das Label hängt an
//! der Überschrift, Teile zählen nicht im Überschriftenzähler, und mit [`part_page_rule`] steht jeder
//! Teil allein auf einer eigenen rechten Seite. Alle Texte frei erfunden.

use baruch_render::{Rendered, Severity, Source, compile};
use baruch_typst::{part_line, part_page_rule, unnumbered_line};

const TEMPLATE: &str = "#set page(width: 100mm, height: 140mm, numbering: \"1\")\n#set heading(numbering: \"1.1\")\n";

/// Werk mit zwei Teilen, einem Vorwort ohne Nummer und drei Kapiteln; jedes Kapitel prüft seinen
/// Zähler. `rule`: Regel für die Teil-Seite hinter der Werkvorlage.
fn book(rule: &str) -> String {
    format!(
        "{TEMPLATE}{rule}{vorwort}Ein Wort vorab.\n\n{teil1}\n= Erstes Kapitel\n\
         #context assert.eq(counter(heading).get(), (1,), message: \"Kapitel 1 nach Teil und Vorwort\")\n\
         Text eins.\n\n== Ein Abschnitt\n#context assert.eq(counter(heading).get(), (1, 1))\n\n\
         = Zweites Kapitel\n#context assert.eq(counter(heading).get(), (2,))\nText zwei.\n\n\
         {teil2}\n= Drittes Kapitel\n\
         #context assert.eq(counter(heading).get(), (3,), message: \"Kapitel laufen über Teile fort\")\n\
         Text drei.\n\n\
         #context assert.eq(query(<baruch-part>).len(), 2, message: \"Label hängt an beiden Teilen\")\n\
         #context assert(query(<baruch-part>).all(h => h.func() == heading), message: \"Label an der Überschrift\")\n\
         #context assert.eq(query(<baruch-unnumbered>).len(), 1)\n",
        vorwort = unnumbered_line(1, "Vorwort"),
        teil1 = part_line("Vom Säen"),
        teil2 = part_line("Vom Ernten"),
    )
}

fn errors(rendered: &Rendered) -> Vec<String> {
    rendered.diagnostics.iter().filter(|d| d.severity == Severity::Error).map(|d| d.message.clone()).collect()
}

/// Seite der Überschrift in Zeile `line` (1-basiert).
fn page_at(rendered: &Rendered, line: usize) -> usize {
    rendered
        .headings
        .iter()
        .find(|h| h.line == line)
        .unwrap_or_else(|| panic!("keine Überschrift in Zeile {line}"))
        .page
}

/// Zeile (1-basiert), die mit `prefix` beginnt.
fn line_of(source: &str, prefix: &str) -> usize {
    source.lines().position(|l| l.starts_with(prefix)).unwrap_or_else(|| panic!("{prefix:?} fehlt")) + 1
}

#[test]
fn markers_set_without_rule_and_parts_do_not_step_the_heading_counter() {
    let source = book("");
    let rendered = compile(&Source { main: source, files: vec![] });
    assert_eq!(
        errors(&rendered),
        Vec::<String>::new(),
        "Ohne Werkvorlage-Definition setzt Typst beide Markierungen; numbering: none zählt nicht mit, \
         die Labels hängen an den Überschriften (sonst scheitert ein assert im Text)"
    );
}

#[test]
fn every_part_stands_alone_on_its_own_right_page() {
    let rule = part_page_rule(&["I".to_owned(), "II".to_owned()]);
    let source = book(&rule);
    let rendered = compile(&Source { main: source.clone(), files: vec![] });
    assert_eq!(errors(&rendered), Vec::<String>::new(), "Werk mit zwei Teilen setzt fehlerfrei, Zähler stimmen weiter");

    let vorwort = page_at(&rendered, line_of(&source, "#heading(level: 1, numbering: none)[Vorwort]"));
    let part1 = page_at(&rendered, line_of(&source, "#heading(level: 1, numbering: none)[Vom Säen]"));
    let first = page_at(&rendered, line_of(&source, "= Erstes Kapitel"));
    let second = page_at(&rendered, line_of(&source, "= Zweites Kapitel"));
    let part2 = page_at(&rendered, line_of(&source, "#heading(level: 1, numbering: none)[Vom Ernten]"));
    let third = page_at(&rendered, line_of(&source, "= Drittes Kapitel"));
    assert!(part1 > vorwort, "Der Teil beginnt eine neue Seite nach dem Vorwort ({vorwort} → {part1})");
    assert!(part1 % 2 == 1 && part2 % 2 == 1, "Teil-Seiten sind rechte (ungerade) Seiten: {part1}, {part2}");
    assert!(first > part1, "Das folgende Kapitel beginnt auf einer späteren Seite ({part1} → {first})");
    assert!(part2 > second, "Der zweite Teil steht nicht auf der Seite des vorigen Kapitels ({second} → {part2})");
    assert!(third > part2, "Nach dem zweiten Teil beginnt das Kapitel auf einer späteren Seite ({part2} → {third})");
    assert_eq!(
        first,
        part1 + 1,
        "Nach der Teil-Seite folgt direkt die nächste Seite (weicher Umbruch, keine Leerseite)"
    );
}

#[test]
fn the_part_page_shows_its_number_and_title() {
    let source =
        format!("{TEMPLATE}{}{}= Kapitel\nText.\n", part_page_rule(&["Teil Eins".to_owned()]), part_line("Saat"));
    let rendered = compile(&Source { main: source, files: vec![] });
    assert_eq!(errors(&rendered), Vec::<String>::new(), "fehlerfrei");
    assert!(rendered.pages.len() >= 2, "Teil-Seite und Kapitelseite: {} Seiten", rendered.pages.len());
    // typst-svg setzt Text als Glyphen; die Teil-Seite (Seite 1) muss also mehr als eine leere Seite sein
    // und sich von der Kapitelseite unterscheiden.
    assert_ne!(rendered.pages[0], rendered.pages[1], "eigene Seite für den Teil");
}

#[test]
fn a_template_heading_rule_does_not_touch_the_part_page() {
    // Eine Werkvorlage, die jede Überschrift der ersten Ebene mit einem Seitenumbruch auf eine rechte
    // Seite und großem Text setzt: Die Teil-Regel steht dahinter und greift zuerst, nutzt nur
    // `it.body` – die Vorlagen-Regel wirkt also nicht doppelt (sonst gäbe es eine zusätzliche Leerseite).
    let template = format!(
        "{TEMPLATE}#show heading.where(level: 1): it => {{ pagebreak(weak: true, to: \"odd\"); text(size: 20pt, it) }}\n"
    );
    let source = format!(
        "{template}{}{}= Kapitel\n#context assert.eq(counter(heading).get(), (1,))\nText.\n",
        part_page_rule(&[String::new()]),
        part_line("Ohne Nummer")
    );
    let rendered = compile(&Source { main: source.clone(), files: vec![] });
    assert_eq!(errors(&rendered), Vec::<String>::new(), "fehlerfrei");
    let part = page_at(&rendered, line_of(&source, "#heading(level: 1, numbering: none)[Ohne Nummer]"));
    let chapter = page_at(&rendered, line_of(&source, "= Kapitel"));
    assert_eq!((part, chapter), (1, 3), "Teil auf Seite 1, Kapitel auf der nächsten rechten Seite 3 (Vorlage)");
}

#[test]
fn the_counter_checks_would_catch_a_counting_part() {
    // Gegenprobe: Wäre der Teil eine gewöhnliche `=`-Überschrift, zählte er mit, und das assert im
    // ersten Kapitel schlüge fehl. So ist belegt, dass die Prüfungen oben wirklich greifen.
    let source = book("").replace(&part_line("Vom Säen"), "= Vom Säen\n");
    let rendered = compile(&Source { main: source, files: vec![] });
    let found = errors(&rendered);
    assert!(
        found.iter().any(|e| e.contains("Kapitel 1 nach Teil und Vorwort")),
        "ein mitzählender Teil verschiebt die Kapitelnummern: {found:?}"
    );
}

#[test]
fn the_show_rule_fallback_sets_a_title_with_code_without_number() {
    // Befund Gutachten Runde 14: Titel mit Code (`#emph[…]`) projiziert der Export nicht mehr auf die
    // Markierung, sondern nimmt die Show-Regel (`snapshot::fallback_rules`, Form wörtlich wie dort).
    // Typst muss sie treffen (Vorwort zählt nicht) und den Titel als Hervorhebung setzen, nicht als
    // Raw-Text „emph“.
    let source = format!(
        "{TEMPLATE}// Baruch: Überschriften ohne Nummer\n\
         #show heading.where(level: 1, body: [Das #emph[Jahr]]): set heading(numbering: none)\n\
         = Das #emph[Jahr]\nVorab.\n\n= Erstes Kapitel\n\
         #context assert.eq(counter(heading).get(), (1,), message: \"Vorwort mit Code zählt nicht\")\n\
         #context assert.eq(query(heading.where(numbering: none)).len(), 1, message: \"Regel trifft das Vorwort\")\n\
         #context assert.eq(query(heading).first().body, [Das #emph[Jahr]], message: \"Titel mit Hervorhebung\")\n\
         #context assert.eq(query(raw).len(), 0, message: \"kein Raw-Text im Titel\")\n"
    );
    let rendered = compile(&Source { main: source, files: vec![] });
    assert_eq!(errors(&rendered), Vec::<String>::new(), "die Show-Regel greift, der Titel bleibt Inhalt");
}
