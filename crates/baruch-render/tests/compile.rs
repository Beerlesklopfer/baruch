// SPDX-License-Identifier: AGPL-3.0-or-later
//! `baruch_render::compile` on a freely invented work (contract Round 8, section 4a):
//! template with image, missing font, syntax and call errors, package import without network, missing
//! image, pages of the headings.

use baruch_render::{OFFLINE, Severity, Source, compile};

/// Small SVG (text, no binary stuff in the repo).
const LAUBE_SVG: &str = "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"40\" height=\"20\" viewBox=\"0 0 40 20\">\
<rect width=\"40\" height=\"20\" fill=\"#3a7\"/><circle cx=\"10\" cy=\"10\" r=\"6\" fill=\"#fd4\"/></svg>";

/// Invented work with work template (`#let szene`), two chapters with page break and an image.
fn garden() -> Source {
    let main = "#set page(width: 12cm, height: 16cm)\n\
                #set text(lang: \"de\")\n\
                #let szene(body) = block(inset: 4pt, stroke: 0.5pt, body)\n\
                = Die Laube\n\
                Anna sitzt im Herbstlaub und zählt die Äpfel.\n\
                #szene[Ein Windstoß.]\n\
                #figure(image(\"./bilder/laube.svg\", width: 3cm), caption: [Die Laube])\n\
                #pagebreak()\n\
                = Der Winter\n\
                == Schnee\n\
                Es schneit auf die leere Bank.\n"
        .to_owned();
    Source { main, files: vec![("bilder/laube.svg".into(), LAUBE_SVG.as_bytes().to_vec())] }
}

#[test]
fn a_work_with_template_and_svg_image_yields_pages_with_the_image_embedded() {
    let rendered = compile(&garden());
    assert!(!rendered.has_errors(), "das erfundene Werk muss fehlerfrei setzen: {:?}", rendered.diagnostics);
    assert!(rendered.pages.len() >= 2, "zwei Kapitel mit #pagebreak ergeben mindestens zwei Seiten");
    assert!(rendered.pages[0].starts_with("<svg"), "jede Seite ist ein SVG-Dokument: {}", &rendered.pages[0][..80]);
    assert!(
        rendered.pages[0].contains("data:image/svg+xml"),
        "das Bild aus den virtuellen Dateien muss in der Seite eingebettet sein (nicht nachgeladen)"
    );
    assert_eq!(rendered.stand, baruch_render::stand(&garden()), "der Stand gehört zum gesetzten Werk");
}

#[test]
fn headings_report_their_line_in_main_and_their_page() {
    let rendered = compile(&garden());
    let lines: Vec<(usize, usize)> = rendered.headings.iter().map(|h| (h.line, h.page)).collect();
    assert_eq!(
        lines,
        vec![(4, 1), (9, 2), (10, 2)],
        "„Die Laube“ steht in Zeile 4 auf Seite 1, „Der Winter“ (Zeile 9) und „Schnee“ (Zeile 10) nach \
         dem Umbruch auf Seite 2"
    );
}

#[test]
fn an_unknown_font_is_a_warning_with_fallback_and_pages_remain() {
    let source = Source {
        main: "#set text(font: \"Gibt Es Nicht Grotesk\")\n= Kapitel\nText in Ersatzschrift.\n".into(),
        files: Vec::new(),
    };
    let rendered = compile(&source);
    assert!(!rendered.has_errors(), "eine fehlende Schrift ist kein Fehler: {:?}", rendered.diagnostics);
    assert!(!rendered.pages.is_empty(), "mit Ersatzschrift entstehen trotzdem Seiten");
    let warning = rendered
        .diagnostics
        .iter()
        .find(|d| d.severity == Severity::Warning)
        .expect("die fehlende Schrift muss als Warnung gemeldet werden");
    assert_eq!(
        warning.message, "Schrift „gibt es nicht grotesk“ fehlt; ersetzt durch eingebettete Schrift",
        "die Warnung nennt die Schrift auf Deutsch (Typst vergleicht Schriftnamen klein geschrieben)"
    );
    assert_eq!(warning.line, Some(1), "die Warnung zeigt auf die Zeile der Set-Regel");
}

#[test]
fn a_syntax_error_has_line_and_column_and_no_pages() {
    let source = Source { main: "= Kapitel\nText\n#let x = (1, 2\nmehr Text\n".into(), files: Vec::new() };
    let rendered = compile(&source);
    assert!(rendered.pages.is_empty(), "bei einem Fehler gibt es keine Seiten");
    let error = rendered.diagnostics.iter().find(|d| d.severity == Severity::Error).expect("Syntaxfehler fehlt");
    assert_eq!(error.line, Some(3), "die offene Klammer steht in Zeile 3: {error:?}");
    assert!(error.column.is_some_and(|c| c >= 1), "die Spalte ist 1-basiert gesetzt: {error:?}");
    assert_eq!(error.file, None, "der Fehler liegt in der Hauptdatei");
}

#[test]
fn a_call_error_has_line_and_column_and_no_pages() {
    let source = Source { main: "= Kapitel\n\nZahl: #calc.pow(\"zwei\", 2)\n".into(), files: Vec::new() };
    let rendered = compile(&source);
    assert!(rendered.pages.is_empty(), "ein Aufruffehler bricht den Satz ab");
    let error = rendered.diagnostics.iter().find(|d| d.severity == Severity::Error).expect("Aufruffehler fehlt");
    assert_eq!(error.line, Some(3), "der Aufruf steht in Zeile 3: {error:?}");
    assert!(error.column.is_some_and(|c| c > 1), "die Spalte zeigt hinter „Zahl: “: {error:?}");
}

#[test]
fn a_preview_import_fails_offline_with_an_understandable_message() {
    let started = std::time::Instant::now();
    let source = Source { main: "#import \"@preview/gibtsnicht:0.1.0\": *\n= Kapitel\n".into(), files: Vec::new() };
    let rendered = compile(&source);
    assert!(rendered.pages.is_empty(), "ohne Paket gibt es keine Seiten");
    let error = rendered.diagnostics.iter().find(|d| d.severity == Severity::Error).expect("Paketfehler fehlt");
    assert_eq!(error.message, OFFLINE, "die Meldung sagt verständlich, dass Pakete nicht geladen werden");
    assert_eq!(error.line, Some(1), "der Import steht in Zeile 1");
    assert!(
        started.elapsed() < std::time::Duration::from_secs(30),
        "ohne Netz darf nichts auf eine Zeitüberschreitung warten"
    );
}

#[test]
fn a_missing_image_is_reported_in_german() {
    let source = Source { main: "= Kapitel\n#image(\"/bilder/fehlt.png\")\n".into(), files: Vec::new() };
    let rendered = compile(&source);
    assert!(rendered.pages.is_empty(), "ein fehlendes Bild ist in Typst ein Fehler");
    let error = rendered.diagnostics.iter().find(|d| d.severity == Severity::Error).expect("Bildfehler fehlt");
    assert!(error.message.contains("„bilder/fehlt.png“ fehlt"), "Meldung nennt den Pfad in Normalform: {error:?}");
    assert_eq!(error.line, Some(2), "das Bild steht in Zeile 2");
}

#[test]
fn image_paths_with_slash_dot_or_bare_find_the_same_file() {
    for path in ["/bilder/laube.svg", "./bilder/laube.svg", "bilder/laube.svg"] {
        let source = Source {
            main: format!("= Kapitel\n#image(\"{path}\", width: 2cm)\n"),
            files: vec![("bilder/laube.svg".into(), LAUBE_SVG.as_bytes().to_vec())],
        };
        let rendered = compile(&source);
        assert!(!rendered.has_errors(), "„{path}“ muss das Bild in Normalform finden: {:?}", rendered.diagnostics);
    }
}
