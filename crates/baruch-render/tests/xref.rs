// SPDX-License-Identifier: AGPL-3.0-or-later
//! Cross-references in the typeset result (Round 30a, contract §3.3): the anchors `<kN>` from
//! `baruch_typst::xref` carry `#link(<kN>)` and `counter(page).at(<kN>)` – on a `=` heading (label at the end of
//! the line) and on headings with a label of their own (part with part page, "without number", label of the
//! human) through the anchor line. Typst checks itself (`#context assert…`) that every anchor lies on the page
//! of its heading. All texts freely invented.

use baruch_render::{Rendered, Severity, Source, compile, compile_pdf};
use baruch_typst::xref::{label_section, part_anchors_for_typesetting, xref_snippet};
use baruch_typst::{part_line, part_page_rule, unnumbered_line};

const TEMPLATE: &str = "#set page(width: 100mm, height: 140mm, numbering: \"1\")\n#set heading(numbering: \"1.1\")\n";

fn errors(rendered: &Rendered) -> Vec<String> {
    rendered.diagnostics.iter().filter(|d| d.severity == Severity::Error).map(|d| d.message.clone()).collect()
}

/// Typst check: the anchor `<kN>` lies on the same page as the heading with the title `title`.
fn same_page(key: i64, title: &str) -> String {
    format!(
        "#context assert.eq(locate(<k{key}>).page(), query(heading).filter(h => h.body == [{title}]).first().location().page(), \
         message: \"Sprungmarke k{key} nicht auf der Seite von {title}\")\n"
    )
}

/// A work of invented sections, each with an anchor as in the export; after that references with page number
/// and the page checks. `rule`: rule for the part page (as in the typesetting) or empty (as in the export).
fn book(rule: &str) -> String {
    let sections = [
        (1, format!("{}Ein Wort vorab.\n\n", unnumbered_line(1, "Vorwort"))),
        (2, format!("{}\n", part_line("Vom Leuchtturm"))),
        (3, "= Leuchtturm\n\nDer Leuchtturm blinkt über dem Hafen.\n\n#pagebreak()\nWeiter.\n\n".to_owned()),
        (4, "== Wachturm // Notiz\n\nDer Wachturm steht still.\n\n".to_owned()),
        (5, "= Brückenbuch <bruecke>\n\nDas Brückenbuch liegt offen.\n\n".to_owned()),
    ];
    let mut out = format!("{TEMPLATE}{rule}");
    for (key, body) in &sections {
        out.push_str(&label_section(body, *key));
    }
    out.push_str("= Verweise\n\n");
    for (key, title) in
        [(1, "das Vorwort"), (2, "der Teil"), (3, "der Leuchtturm"), (4, "der Wachturm"), (5, "das Buch")]
    {
        out.push_str(&format!("Siehe {}.\n\n", xref_snippet(key, title, true)));
    }
    for (key, title) in [(1, "Vorwort"), (2, "Vom Leuchtturm"), (3, "Leuchtturm"), (4, "Wachturm"), (5, "Brückenbuch")]
    {
        out.push_str(&same_page(key, title));
    }
    out
}

#[test]
fn every_key_label_sits_on_the_page_of_its_heading_in_the_export_form() {
    let rendered = compile(&Source { main: book(""), files: vec![] });
    assert_eq!(
        errors(&rendered),
        Vec::<String>::new(),
        "Export ohne Teil-Regel: Links und Seitenzahlen lösen auf, jede Sprungmarke auf der Seite ihrer Überschrift"
    );
}

#[test]
fn the_part_anchor_stays_on_the_part_page_with_the_typesetting_rule() {
    // Typesetting: part alone on a right page, then a weak break. The anchor line comes after the
    // part heading; if it nevertheless lies on its page, "S. n" points to the part page.
    let rule = part_page_rule(&["I".to_owned()]);
    let export = book(&rule);
    let rendered = compile(&Source { main: export.clone(), files: vec![] });
    assert_eq!(
        errors(&rendered),
        vec!["equality assertion failed: Sprungmarke k2 nicht auf der Seite von Vom Leuchtturm".to_owned()],
        "Beleg: hinter der Teil-Zeile landet die Ankerzeile nach dem Umbruch der Teil-Regel auf der Folgeseite"
    );
    // Therefore the anchor line moves before the part line in the typesetting, with a weak break of its own onto a
    // right page.
    let typeset = part_anchors_for_typesetting(&export).into_owned();
    assert_ne!(typeset, export, "die Satz-Form ändert die Quelle");
    assert_eq!(typeset.lines().count(), export.lines().count(), "Zeilenzahl bleibt (Meldungen behalten ihre Zeile)");
    let rendered = compile(&Source { main: typeset, files: vec![] });
    assert_eq!(
        errors(&rendered),
        Vec::<String>::new(),
        "Satz mit Teil-Seite: Sprungmarken auf der Seite ihrer Überschrift"
    );
    // Comparison: the export form without the page check of the part (anchor after the break, no additional break).
    let plain = compile(&Source { main: export.replace(&same_page(2, "Vom Leuchtturm"), ""), files: vec![] });
    assert_eq!(errors(&plain), Vec::<String>::new(), "Vergleichssatz fehlerfrei");
    assert_eq!(
        rendered.pages.len(),
        plain.pages.len(),
        "der zusätzliche weiche Umbruch erzeugt keine Leerseite (gleiche Seitenzahl wie ohne Anker)"
    );
}

#[test]
fn a_missing_target_is_a_typst_error_naming_the_label() {
    let source = format!("{TEMPLATE}= Leuchtturm <k1>\n\nSiehe {}.\n", xref_snippet(99, "nirgends", false));
    let rendered = compile(&Source { main: source, files: vec![] });
    let errors = errors(&rendered);
    assert!(
        errors.iter().any(|e| e.contains("k99")),
        "ein kaputter Verweis scheitert im Satz mit dem Label in der Meldung (darum die Druckprüfung): {errors:?}"
    );
}

#[test]
fn the_pdf_carries_an_internal_link_and_the_page_number() {
    let rendered = compile_pdf(&Source { main: book(""), files: vec![] });
    assert!(!rendered.has_errors(), "PDF fehlerfrei: {:?}", rendered.diagnostics);
    let bytes = rendered.bytes.expect("PDF-Bytes");
    let pdf = String::from_utf8_lossy(&bytes);
    assert!(
        pdf.contains("/Link") && (pdf.contains("/GoTo") || pdf.contains("/Dest")),
        "ein interner Link (Link-Annotation mit Ziel im Dokument) steht im PDF"
    );
}
