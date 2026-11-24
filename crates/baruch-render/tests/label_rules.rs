// SPDX-License-Identifier: AGPL-3.0-or-later
//! Runde 22 (spiegel-entwurf, R21b-Rest): Die Show-Regel über die Sprungmarke `<kN>`, die der Export für einen Knoten
//! „ohne Nummer“ mit gleichlautender Überschrift schreibt (`snapshot::label_rules`), wirkt in Typst genau auf diese eine
//! Überschrift – die gleichlautende nummerierte zählt weiter. Texte frei erfunden.

use baruch_render::{Severity, Source, compile, page_texts};

#[test]
fn a_show_rule_over_the_jump_label_unnumbers_exactly_that_heading() {
    let main = "#set heading(numbering: \"1.\")\n\
                // Baruch: Überschriften ohne Nummer über die Sprungmarke\n\
                #show <k2>: set heading(numbering: none)\n\
                = Anfang <k1>\n\nText.\n\n= #emph[Rast] <k2>\n\nPause.\n\n= #emph[Rast] <k3>\n\nWeiter.\n";
    let source = Source { main: main.to_owned(), files: Vec::new() };
    let errors: Vec<String> =
        compile(&source).diagnostics.into_iter().filter(|d| d.severity == Severity::Error).map(|d| d.message).collect();
    assert!(errors.is_empty(), "die Regel setzt fehlerfrei: {errors:?}");
    let text: String =
        page_texts(&source).expect("setzbar").into_iter().flatten().map(|t| t.text).collect::<Vec<_>>().join("|");
    let first = text.find("1.").expect("erste Überschrift nummeriert");
    let second = text.find("2.").expect("die gleichlautende nummerierte Überschrift zählt als 2");
    assert!(first < second, "Reihenfolge 1., 2.: {text}");
    assert!(!text.contains("3."), "die Überschrift mit <k2> zählt nicht mit (sonst gäbe es 3.): {text}");
    let pause = text.find("Pause").expect("Text unter <k2>");
    assert!(second > pause, "„2.“ steht erst bei der dritten Überschrift, nicht bei <k2>: {text}");
}
