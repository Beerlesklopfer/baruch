// SPDX-License-Identifier: AGPL-3.0-or-later
//! Satzbild nach der Werkvorlage: Werte aus einer frei erfundenen Vorlage, Vorgaben ohne Angaben.

use baruch_wysiwyg::{BoxStyle, SheetStyle, sheet_css, sheet_style, sheet_style_over, template_font};

/// Frei erfundene Vorlage (Schrift, Farben und Funktionsnamen ausgedacht).
const TEMPLATE: &str = r##"#set document(title: "Der Brunnen", author: "Niemand")
// #set text(font: "Auskommentiert")
#set page(paper: "a4")
#set page(width: 13cm, height: 20cm)
#set text(font: ("Erfundene Antiqua", "Ersatz Serif"), size: 10.5pt, fill: rgb("#2A2A33"), lang: "de")
#set par(first-line-indent: (amount: 1.2em, all: false), leading: 0.7em, justify: true)
#show emph: set text(fill: rgb("#1f4e9c"))
#show strong: set text(fill: luma(60))
#show heading: set text(font: "Erfundene Grotesk", fill: maroon)
#let szene(body) = block(fill: luma(240), inset: 8pt, stroke: 0.5pt + gray, width: 100%)[#body]
#let arbeit(body) = rect(fill: rgb(230, 240, 250), inset: 1em)[#body]
```
#set text(fill: red)
```
"##;

#[test]
fn the_style_comes_from_the_template() {
    let style = sheet_style(TEMPLATE);
    assert_eq!(style.font.as_deref(), Some("\"Erfundene Antiqua\", \"Ersatz Serif\""), "Schriftliste");
    assert_eq!(style.size.as_deref(), Some("10.5pt"), "Größe");
    assert_eq!(style.fill.as_deref(), Some("#2a2a33"), "Textfarbe; der Raw-Block zählt nicht");
    assert_eq!(style.indent.as_deref(), Some("1.2em"), "Einzug aus amount");
    assert_eq!(style.leading.as_deref(), Some("0.7em"), "Zeilenabstand");
    assert_eq!(style.justify, Some(true), "Blocksatz");
    assert_eq!(style.width.as_deref(), Some("13cm"), "die letzte Seitenregel gewinnt");
    assert_eq!(style.emph_fill.as_deref(), Some("#1f4e9c"), "Farbe der Hervorhebung");
    assert_eq!(style.strong_fill.as_deref(), Some("rgb(60, 60, 60)"), "Farbe von Fett");
    assert_eq!(style.heading_font.as_deref(), Some("\"Erfundene Grotesk\""), "Schrift der Überschriften");
    assert_eq!(style.heading_fill.as_deref(), Some("#85144b"), "Farbname von Typst");
    assert_eq!(
        style.scene,
        Some(BoxStyle {
            fill: Some("rgb(240, 240, 240)".into()),
            inset: Some("8pt".into()),
            stroke: Some("0.5pt solid #aaaaaa".into())
        }),
        "Kasten von #szene"
    );
    assert_eq!(
        style.work,
        Some(BoxStyle { fill: Some("rgb(230, 240, 250)".into()), inset: Some("1em".into()), stroke: None }),
        "Kasten von #arbeit (rect)"
    );
    let css = sheet_css(&style);
    assert!(css.starts_with(".ws-sheet {"), "Variablen unter .ws-sheet: {css}");
    for var in [
        "--ws-font: \"Erfundene Antiqua\"",
        "--ws-size: 10.5pt",
        "--ws-emph-fill: #1f4e9c",
        "--ws-justify: justify",
        "--ws-scene-stroke: 0.5pt solid #aaaaaa",
        "--ws-width: 13cm",
    ] {
        assert!(css.contains(var), "{var} gesetzt: {css}");
    }
}

/// Frei erfundene Vorlage mit Regeln, die nur für einen Teil gelten (Show-Regel, Funktionsrumpf),
/// neben den Regeln der obersten Ebene.
const SCOPED: &str = r##"#show footnote: set text(size: 8.5pt)
#set text(size: 10.5pt)
#let f() = { set par(first-line-indent: 0pt) }
#set par(first-line-indent: 1.1em)
#show heading: it => [#set text(size: 20pt, fill: red); #it]
#show: inhalt => { set page(width: 9cm); inhalt }
#let rahmen(body) = [#set par(leading: 2em, justify: false)#body]
#set page(width: 12cm) #set par(leading: 0.8em)
#if true { set text(fill: blue) }
"##;

#[test]
fn only_rules_on_the_top_level_count() {
    let style = sheet_style(SCOPED);
    assert_eq!(style.size.as_deref(), Some("10.5pt"), "#show footnote: set text(size:) gilt nur für Fußnoten");
    assert_eq!(style.indent.as_deref(), Some("1.1em"), "die Regel im Funktionsrumpf zählt nicht");
    assert_eq!(style.fill, None, "Regeln in Show-Rümpfen und #if zählen nicht");
    assert_eq!(style.width.as_deref(), Some("12cm"), "#show: …-Rumpf zählt nicht, die oberste Regel schon");
    assert_eq!(
        style.leading.as_deref(),
        Some("0.8em"),
        "zweite Regel in derselben Zeile zählt, die im Inhaltsblock nicht"
    );
    assert_eq!(style.justify, None, "Regel im Inhaltsblock einer Funktion zählt nicht");
    assert_eq!(style.heading_fill, None, "#show heading: it => […] ist kein `set text` der Überschrift");
}

#[test]
fn the_last_top_level_rule_wins() {
    let style = sheet_style("#set text(size: 9pt)\n#let g() = { set text(size: 30pt) }\n#set text(size: 11pt)\n");
    assert_eq!(style.size.as_deref(), Some("11pt"), "die letzte Regel der obersten Ebene gewinnt");
    let style = sheet_style("#set text(size: 11pt)\n#let g() = [#set text(size: 30pt)]\n");
    assert_eq!(style.size.as_deref(), Some("11pt"), "eine spätere Regel im Rumpf überschreibt nicht");
}

#[test]
fn without_rules_the_defaults_apply() {
    let style = sheet_style("// nur ein Kommentar\n#set document(title: \"Leer\")\n");
    assert_eq!(style, SheetStyle::default(), "ohne Regeln nichts gesetzt");
    assert_eq!(sheet_css(&style), ".ws-sheet {  }", "keine Variablen: es gelten die Vorgaben aus wysiwyg.css");
    let paper = sheet_style("#set page(paper: \"a5\")");
    assert_eq!(paper.width.as_deref(), Some("14.8cm"), "Breite aus dem Papierformat");
}

/// Runde 18c: Die Satzbild-Einstellungen des Werks überschreiben die Vorlage feldweise
/// (`sheet_style_over`); was die Einstellungen offen lassen, bleibt aus der Vorlage.
#[test]
fn settings_override_the_template_field_by_field() {
    let base = sheet_style(TEMPLATE);
    let over = SheetStyle {
        font: Some("\"Probe Serif\", serif".into()),
        width: Some("16cm".into()),
        background: Some("#fbf8f1".into()),
        ..SheetStyle::default()
    };
    let style = sheet_style_over(base.clone(), &over);
    assert_eq!(style.font, over.font, "die Schrift der Einstellungen gewinnt über die der Vorlage");
    assert_eq!(style.width.as_deref(), Some("16cm"), "die Satzbreite der Einstellungen gewinnt (Vorlage: 13cm)");
    assert_eq!(style.size, base.size, "nicht gesetzte Felder bleiben aus der Vorlage (Größe 10.5pt)");
    assert_eq!(style.heading_font, base.heading_font, "auch die Schrift der Überschriften bleibt aus der Vorlage");
    assert_eq!(style.scene, base.scene, "die Kästen von #szene kommen weiter aus der Vorlage");
    assert_eq!(base.background, None, "die Vorlage liefert nie eine Blattfarbe (das Blatt bleibt hell)");
    let css = sheet_css(&style);
    for var in
        ["--ws-font: \"Probe Serif\", serif;", "--ws-width: 16cm;", "--ws-background: #fbf8f1;", "--ws-size: 10.5pt;"]
    {
        assert!(css.contains(var), "{var} steht im CSS des Satzbilds: {css}");
    }
    assert_eq!(sheet_style_over(base.clone(), &SheetStyle::default()), base, "leere Einstellungen ändern nichts");
}

/// Ein Wert mit unpaarigem Anführungszeichen öffnete im CSS eine Zeichenkette bis zum Zeilenende und nähme die
/// übrigen Variablen (und die schließende Klammer) mit; spitze Klammern könnten das `<style>`-Element beenden.
/// Beides fällt weg, der Rest bleibt.
#[test]
fn values_that_could_break_the_rule_or_the_style_element_are_dropped() {
    let style = SheetStyle {
        font: Some("\"Offen, serif".into()),
        heading_font: Some("A</style><script>alert(1)</script>".into()),
        size: Some("11pt".into()),
        ..SheetStyle::default()
    };
    let css = sheet_css(&style);
    assert!(!css.contains("Offen"), "unpaariges Anführungszeichen: Wert fällt weg: {css}");
    assert!(!css.contains('<') && !css.contains("script"), "nichts beendet das style-Element: {css}");
    assert!(css.contains("--ws-size: 11pt;") && css.ends_with('}'), "die übrigen Werte und die Klammer bleiben: {css}");
}

// ---- Grundschrift für den Umschlag (Runde 27, Nachbesserung) ---------------------------------------------

#[test]
fn template_font_takes_the_first_font_of_the_last_top_level_set_text() {
    // Typst lässt die spätere set-Regel gewinnen; der Umschlag muss dieselbe Schrift nehmen wie das Buch.
    assert_eq!(
        template_font("#set text(font: \"Erste Antiqua\")\n#set text(font: \"Zweite Antiqua\")"),
        Some("Zweite Antiqua".into()),
        "zwei Regeln auf oberster Ebene: die letzte gilt, nicht die erste"
    );
    assert_eq!(
        template_font("#set text(lang: \"de\", font: \"Erfundene Serif\")"),
        Some("Erfundene Serif".into()),
        "eine Schrift als String neben anderen Argumenten"
    );
    assert_eq!(
        template_font("#set text(font: \"Alt\")\n#set text(size: 11pt)"),
        Some("Alt".into()),
        "eine spätere Regel ohne font: lässt die Schrift stehen"
    );
    assert_eq!(
        template_font("#set text(size: 11pt)\n#set text(font: (\"Erfundene Antiqua\", \"Ersatz Serif\"), size: 10pt)"),
        Some("Erfundene Antiqua".into()),
        "aus einem Array zählt das erste Element (Typst nimmt die erste vorhandene Schrift)"
    );
    assert_eq!(template_font("#set text(lang: \"(\", font: \"A\")"), Some("A".into()), "Klammern in Strings");
    assert_eq!(template_font("#set par(justify: true)"), None, "ohne set text keine Schrift");
}

#[test]
fn template_font_ignores_comments_show_rules_and_bodies() {
    let template = "#set text(font: \"Grund Antiqua\")\n\
                    // #set text(font: \"Auskommentiert\")\n\
                    /* #set text(font: \"Blockkommentar\") */\n\
                    #show heading: set text(font: \"Titel Grotesk\")\n\
                    #show: rest => { set text(font: \"Rumpf\"); rest }\n\
                    #let f(x) = { set text(font: \"Funktion\"); x }\n\
                    ```\n#set text(font: \"Raw\")\n```\n";
    // Jede dieser späteren Fundstellen stünde bei einer reinen Textsuche nach dem letzten „set text(“ vorn; Typst
    // setzt den Fließtext (und damit den Umschlagblock ohne eigene Schrift) aber in der Grundschrift.
    assert_eq!(
        template_font(template),
        Some("Grund Antiqua".into()),
        "Kommentar, Show-Regel, show-Rumpf, Funktionsrumpf und Raw-Text zählen nicht"
    );
    assert_eq!(
        template_font("// #set text(font: \"Nur Kommentar\")\n#show heading: set text(font: \"Nur Titel\")"),
        None,
        "ohne Regel auf oberster Ebene keine Schrift (dann gilt die Vorgabe)"
    );
}
