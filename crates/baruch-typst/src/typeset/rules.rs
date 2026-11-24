// SPDX-License-Identifier: AGPL-3.0-or-later
//! Typst-Regeln aus den Satz-Einstellungen (Runde 29b, Vertrag §5.1/§5.2).
//!
//! [`rules`] liefert für leere Einstellungen "" und sonst einen Block, der mit der Zeile
//! [`RULES_MARK`](super::RULES_MARK) beginnt und mit '\n' endet. Der Store setzt ihn in `snapshot::head` hinter
//! Werkvorlage, Schema- und Rückfall-Regeln – damit gilt er gleich in Export, Spiegel, Satz-Vorschau, Buch-PDF und
//! Umschlag.
//!
//! **Form:** je Feld eine Zeile in [`FIELDS`](super::FIELDS)-Reihenfolge; mehrere Zeilen nur dort, wo Typst es braucht
//! (Hilfsfunktionen für Seitenzahlen und Kolumnentitel, Neubeginn der Fußnoten). `xref_with_page` ist keine Satz-Regel:
//! Einstellungen, die nur dieses Feld setzen, ergeben "" (der Schalter ändert keine Datei, Vertrag §6.5).
//!
//! **Warum die Regeln so aussehen** (gemessen in `baruch-render/tests/typeset_measure.rs`):
//! - **Kapitel** = Überschrift Ebene 1 ohne Label `<baruch-part>` (Fußnoten, Kolumnentitel, Kapitelanfangsseiten).
//!   Der Umbruch des Kapitelanfangs gilt aber für jede Überschrift der Ebene 1, auch für Teile: Im Export (ohne
//!   Teil-Regel des Satzes) sind Teile Überschriften der Ebene 1 und beginnen so mindestens wie ein Kapitel. Damit ist
//!   die Regel genau die übernehmbare Vorlagenregel (`it => { pagebreak(…); it }`), und „Vorlagenregel übernehmen“
//!   verliert im Export nichts (Befund R29; vorher ließ die Regel Teile durch, nach der Übernahme begannen Teile im
//!   Export nicht mehr auf einer neuen Seite). Im Satz greift vor ihr die Teil-Regel, die `it` nicht zurückgibt.
//!   `pagebreak(to: "odd")` zählt in Typst die physischen Seiten – Leerseiten entstehen also richtig, auch wenn die
//!   Seitenzählung erst beim ersten Kapitel beginnt.
//! - **Seitenzählung ab dem ersten Kapitel:** Der Querverweis „S. n“ liest `counter(page).at(<kN>)` an der Überschrift.
//!   Ein `counter(page).update(1)` in der Show-Regel der Überschrift stünde hinter deren Startmarke (Typst legt alles,
//!   was eine Show-Regel liefert, zwischen Start- und Endmarke des Elements) und gälte für den Verweis zu spät. Darum
//!   steht die Rücksetzung im Seitenhintergrund (`page.background`) der ersten Kapitelseite: Typst legt den Hintergrund
//!   vor Kopfzeile und Inhalt in den Seitenrahmen, die Rücksetzung gilt so für alles auf dieser Seite – Fußzeile,
//!   Kopfzeile und Verweise zeigen dieselbe Zahl.
//! - **Leerseiten** (Kolumnentitel und Seitenzahl entfallen): Vor jeder Kapitel- und (mit eigener Teil-Regel)
//!   Teil-Überschrift steht eine unsichtbare Marke `<baruch-satz-end>` *vor* dem Seitenumbruch der Vorlage bzw. der
//!   Einstellung. Typst lässt abgeschlossene Marken vor dem Umbruch stehen (nur unabgeschlossene Startmarken wandern auf
//!   die neue Seite); die Marke zeigt so, wo der Text davor endet. Seiten zwischen Marke und Überschrift sind leer. Steht
//!   vor dem Umbruch nur noch die Marke (etwa direkt nach einer Teil-Seite), legt Typst sie an den Anfang der nächsten
//!   Seite (Lage 0 pt) – auch das zählt als leer. Die leere Rückseite einer Teil-Seite trägt `<baruch-satz-blank>`.
//! - Kopf- und Fußzeile sind `context`-Ausdrücke, die die Seite über `here().page()` mit den Seiten der Überschriften
//!   vergleichen; die Seitenlage „außen“ folgt der physischen Seite (ungerade = rechts), wie die Ränder `inside`/`outside`.

use super::model::RULES_MARK;
use super::model::{
    ChapterStart, FootnoteNumbering, FootnoteRestart, PageNumberPosition, PageNumberStart, Paragraph, PartPage,
    PartSide, RunningHead, TypesetSettings,
};

/// Label der Marke vor dem Seitenumbruch einer Kapitel- bzw. Teil-Überschrift (siehe Modulkopf).
pub(crate) const END_LABEL: &str = "baruch-satz-end";
/// Label der leeren Rückseite einer Teil-Seite.
pub(crate) const BLANK_LABEL: &str = "baruch-satz-blank";

/// Typst-Zahl (Dezimalpunkt, ohne unnötige Nullen); nicht endlich → `None` (keine Regel).
fn num(x: f64) -> Option<String> {
    if !x.is_finite() {
        return None;
    }
    let rounded = (x * 1000.0).round() / 1000.0;
    Some(format!("{rounded}"))
}

/// Typst-Zeichenkette mit Maskierung.
pub(super) fn literal(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Bedingung „ist eine Teil-Überschrift“ für die Variable `it` einer Show-Regel.
fn is_part(var: &str) -> String {
    format!("{var}.at(\"label\", default: none) == <{}>", crate::marker::PART_LABEL)
}

/// Hilfsfunktionen für Kopf- und Fußzeilen (Seitenzahlen, Kolumnentitel) und die Marke vor dem Umbruch.
///
/// `baruch-satz-quiet(p)`: Teil-Seite, leere Rückseite oder Leerseite vor einer Kapitelüberschrift. Eine Leerseite liegt
/// zwischen der Marke `<baruch-satz-end>` (vor dem Umbruch der Kapitelregel) und der Überschrift dahinter. Ohne
/// Kapitelregel steht die Marke an derselben Stelle wie ihre Überschrift und folgt ihr in der Abfrage (Integrator R29):
/// Eine solche Marke zählt nicht, sonst wären alle Seiten bis zur nächsten Überschrift „leer“ (ohne Seitenzahl und
/// Kolumnentitel).
fn helpers() -> String {
    let part = crate::marker::PART_LABEL;
    format!(
        "#let baruch-satz-chapters() = query(heading.where(level: 1)).filter(h => not ({part_h}))\n\
         #let baruch-satz-opening(p) = baruch-satz-chapters().any(h => h.location().page() == p)\n\
         #let baruch-satz-quiet(p) = {{ \
if query(<{part}>).any(h => h.location().page() == p) {{ return true }}; \
if query(<{BLANK_LABEL}>).any(m => m.location().page() == p) {{ return true }}; \
let mark = none; let last = none; \
for e in query(selector(<{END_LABEL}>).or(heading.where(level: 1))) {{ \
if e.func() == metadata {{ if e.location().position() != last {{ mark = e.location().position() }} }} else {{ \
let q = e.location().page(); \
if mark != none and p < q and (mark.page < p or (mark.page == p and mark.y == 0pt)) {{ return true }}; \
mark = none; last = e.location().position() }} }}; \
false }}\n\
         #show heading.where(level: 1): it => {{ [#metadata(\"baruch-satz\") <{END_LABEL}>]; it }}\n",
        part_h = is_part("h"),
    )
}

/// Ausdruck der Seitenzahl in einem `context` mit Seite `p` (Zeichenkette oder `none`).
fn folio(start: PageNumberStart) -> String {
    let number = "str(counter(page).get().first())";
    match start {
        PageNumberStart::FirstPage => format!("if baruch-satz-quiet(p) {{ none }} else {{ {number} }}"),
        PageNumberStart::FirstChapter => format!(
            "if baruch-satz-quiet(p) {{ none }} else {{ let c = baruch-satz-chapters(); \
if c.len() > 0 and p < c.first().location().page() {{ none }} else {{ {number} }} }}"
        ),
    }
}

/// Kopfzeile: Kolumnentitel (`title` = Typst-Ausdruck mit `p`, oder `None`) und Seitenzahl oben außen (`folio`).
fn header(title: Option<&str>, folio: Option<&str>) -> String {
    let title = title.unwrap_or("none");
    let folio = folio.unwrap_or("none");
    format!(
        "context {{ let p = here().page(); let t = {title}; let n = {folio}; \
set text(size: 0.85em); \
grid(columns: (1fr, auto, 1fr), \
align(left, if calc.even(p) and n != none {{ n }}), \
align(center, if t != none {{ t }}), \
align(right, if calc.odd(p) and n != none {{ n }})) }}"
    )
}

/// Typst-Regelblock zu `settings`; leere Einstellungen (und solche, die nur `xref_with_page` oder die R31-Felder
/// `glossary_page`, `index_pages`, `glossary_first_mention` setzen) → "".
pub fn rules(settings: &TypesetSettings) -> String {
    // R31-S1: die drei Register/Glossar-Felder steuern crate::index (Projektion, Buchende), keine Zeile im Regelblock.
    let typeset = TypesetSettings {
        xref_with_page: None,
        glossary_page: None,
        index_pages: None,
        glossary_first_mention: None,
        // R22: der Entwurf kommt nur in die Satzquelle der Ausgaben (typeset::draft_rule), nie in den Regelblock.
        draft: None,
        ..settings.clone()
    };
    if typeset.is_empty() {
        return String::new();
    }
    let s = &typeset;
    let mut out = format!("{RULES_MARK}\n");
    let mut line = |text: String| {
        out.push_str(&text);
        if !text.ends_with('\n') {
            out.push('\n');
        }
    };

    // Kapitelanfang (Überschrift Ebene 1; Teile brechen im Export ebenso um, siehe Modulkopf).
    if let Some(start) = s.chapter_start {
        line(match start {
            ChapterStart::Continuous => "// Kapitelanfang: fortlaufend (kein Seitenumbruch vor Kapiteln)".to_owned(),
            ChapterStart::NewPage => "#show heading.where(level: 1): it => { pagebreak(weak: true); it }".to_owned(),
            ChapterStart::RightPage => {
                "#show heading.where(level: 1): it => { pagebreak(weak: true, to: \"odd\"); it }".to_owned()
            }
        });
    }
    // Teil-Seite: wirkt über die Teil-Regel des Satzes (`part_page_rule_with`), hier nur vermerkt.
    if let Some(part) = &s.part_page {
        line(format!(
            "// Teil-Seite: {}{} (Regel der Teil-Seiten im Satz)",
            part.side.label(),
            if part.blank_back { ", Rückseite leer" } else { "" }
        ));
    }
    if let Some(format) = &s.page_format {
        let (w, h) = format.size_mm();
        if let (Some(w), Some(h)) = (num(w), num(h)) {
            line(format!("#set page(width: {w}mm, height: {h}mm)"));
        }
    }
    if let Some(m) = &s.margins
        && let (Some(i), Some(o), Some(t), Some(b)) =
            (num(m.inside_mm + m.binding_mm), num(m.outside_mm), num(m.top_mm), num(m.bottom_mm))
    {
        line(format!("#set page(margin: (inside: {i}mm, outside: {o}mm, top: {t}mm, bottom: {b}mm))"));
    }
    if let Some(font) = &s.font {
        line(format!("#set text(font: {})", literal(font.trim())));
    }
    if let Some(size) = s.font_size_pt.and_then(num) {
        line(format!("#set text(size: {size}pt)"));
    }
    if let Some(leading) = s.leading_em.and_then(num) {
        line(format!("#set par(leading: {leading}em)"));
    }
    if let Some(paragraph) = s.paragraph {
        match paragraph {
            Paragraph::Indent { em } => {
                // Kein Zwischenraum außer dem Zeilenabstand: Absatzabstand = Zeilenabstand (Typst-Vorgabe 0,65 em).
                let gap = num(s.leading_em.unwrap_or(0.65)).unwrap_or_else(|| "0.65".into());
                if let Some(em) = num(em) {
                    line(format!("#set par(first-line-indent: {em}em, spacing: {gap}em)"));
                }
            }
            Paragraph::Spacing { em } => {
                if let Some(em) = num(em) {
                    line(format!("#set par(first-line-indent: 0em, spacing: {em}em)"));
                }
            }
        }
    }
    if let Some(justify) = s.justify {
        line(format!("#set par(justify: {justify})"));
    }
    if let Some(hyphenate) = s.hyphenate {
        line(format!("#set text(hyphenate: {hyphenate})"));
    }
    if let Some(lang) = &s.lang {
        let lang = lang.trim();
        line(match lang.split_once('-') {
            Some((base, region)) => format!("#set text(lang: {}, region: {})", literal(base), literal(region)),
            None => format!("#set text(lang: {})", literal(lang)),
        });
    }

    // Seitenzahlen und Kolumnentitel teilen sich Hilfsfunktionen und die Kopfzeile.
    let numbers = s.page_numbers.as_ref().filter(|n| n.position != PageNumberPosition::Hidden);
    let head = s.running_head.filter(|h| *h != RunningHead::Off);
    if numbers.is_some() || head.is_some() {
        line(helpers());
    }
    if let Some(pn) = &s.page_numbers {
        let f = folio(pn.start);
        line(match pn.position {
            PageNumberPosition::Hidden => "#set page(numbering: none, footer: none)".to_owned(),
            PageNumberPosition::BottomCenter => {
                format!("#set page(numbering: \"1\", footer: context {{ let p = here().page(); align(center, {f}) }})")
            }
            PageNumberPosition::BottomOutside => format!(
                "#set page(numbering: \"1\", footer: context {{ let p = here().page(); \
align(if calc.odd(p) {{ right }} else {{ left }}, {f}) }})"
            ),
            PageNumberPosition::TopOutside => {
                format!("#set page(numbering: \"1\", footer: none, header: {})", header(None, Some(&f)))
            }
        });
        if pn.start == PageNumberStart::FirstChapter && pn.position != PageNumberPosition::Hidden {
            line(
                "#set page(background: context { let c = baruch-satz-chapters(); \
if c.len() > 0 and here().page() == c.first().location().page() { counter(page).update(1) } })"
                    .to_owned(),
            );
        }
    }
    if let Some(rh) = s.running_head {
        let top_folio = numbers.filter(|n| n.position == PageNumberPosition::TopOutside).map(|n| folio(n.start));
        let quiet = "baruch-satz-opening(p) or baruch-satz-quiet(p)";
        let title = match rh {
            RunningHead::Off => None,
            RunningHead::WorkTitle => Some(format!("if {quiet} {{ none }} else {{ document.title }}")),
            RunningHead::Chapter => Some(format!(
                "if {quiet} {{ none }} else {{ let c = baruch-satz-chapters().filter(h => h.location().page() < p); \
if c.len() > 0 {{ c.last().body }} }}"
            )),
        };
        line(match (&title, &top_folio) {
            (None, None) => "#set page(header: none)".to_owned(),
            _ => format!("#set page(header: {})", header(title.as_deref(), top_folio.as_deref())),
        });
    }
    if let Some(footnotes) = &s.footnotes {
        let numbering = match footnotes.numbering {
            FootnoteNumbering::Arabic => "1",
            FootnoteNumbering::Symbols => "*",
            FootnoteNumbering::Letters => "a",
        };
        line(format!("#set footnote(numbering: \"{numbering}\")"));
        if footnotes.restart == FootnoteRestart::PerChapter {
            line(format!(
                "#show heading.where(level: 1): it => {{ if not ({}) {{ counter(footnote).update(0) }}; it }}",
                is_part("it")
            ));
        }
    }
    if let Some(avoid) = s.widows_orphans {
        let cost = if avoid { "100%" } else { "0%" };
        line(format!("#set text(costs: (widow: {cost}, orphan: {cost}))"));
    }
    out
}

/// Regel der Teil-Seiten (wie [`crate::marker::part_page_rule`]) mit Seite und leerer Rückseite aus `part`.
/// `part_page_rule_with(labels, None)` ist byte-gleich zu `marker::part_page_rule(labels)`.
///
/// Mit `part`: `side` Right = eigene rechte Seite (`to: "odd"`), Any = nächste Seite; `blank_back` = die Seite nach der
/// Teil-Seite bleibt leer (zwei harte Umbrüche, dazwischen die Marke `<baruch-satz-blank>`, damit Kopf- und Fußzeile
/// dort entfallen). Vor dem ersten Umbruch steht die Marke `<baruch-satz-end>` (Leerseiten davor erkennen).
pub fn part_page_rule_with(labels: &[String], part: Option<&PartPage>) -> String {
    let Some(part) = part else { return crate::marker::part_page_rule(labels) };
    let default = crate::marker::part_page_rule(labels);
    let before = match part.side {
        PartSide::Right => "pagebreak(weak: true, to: \"odd\"); ",
        PartSide::Any => "pagebreak(weak: true); ",
    };
    let after = if part.blank_back {
        format!("pagebreak(); [#metadata(\"baruch-satz\") <{BLANK_LABEL}>]; pagebreak() }}\n")
    } else {
        "pagebreak(weak: true) }\n".to_owned()
    };
    // Die Standardregel beginnt mit `{ pagebreak(weak: true, to: "odd"); ` und endet mit `pagebreak(weak: true) }`;
    // dazwischen (Zähler, Nummer, Titel) bleibt sie wörtlich.
    let head = "it => { pagebreak(weak: true, to: \"odd\"); ";
    let tail = "pagebreak(weak: true) }\n";
    match (default.find(head), default.strip_suffix(tail)) {
        (Some(at), Some(body)) if at + head.len() <= body.len() => format!(
            "{}it => {{ [#metadata(\"baruch-satz\") <{END_LABEL}>]; {before}{}{after}",
            &body[..at],
            &body[at + head.len()..]
        ),
        _ => default,
    }
}

/// Regel der Teil-Seiten für ein Werk mit den Satz-Einstellungen `settings` – so, wie Satz (`typeset::prepare`),
/// Vorschau und Messtests sie hinter den Regelblock [`rules`] setzen.
///
/// - Mit `part_page`: [`part_page_rule_with`] mit dieser Einstellung.
/// - Ohne `part_page`, aber mit Regelblock (etwa nur Seitenzahlen oder Kolumnentitel): dieselbe Seite wie die Regel
///   aus Runde 14 (eigene rechte Seite, keine leere Rückseite), aber mit der Marke `<baruch-satz-end>` vor dem Umbruch.
///   Die Teil-Regel greift vor der Show-Regel des Regelblocks und gibt `it` nicht zurück; ohne eigene Marke erkennte
///   `baruch-satz-quiet` die Leerseite vor dem Teil nicht, und sie trüge Seitenzahl und Kolumnentitel (Befund R29).
/// - Ohne Regelblock: byte-gleich zu `marker::part_page_rule` (ein Werk ohne Satz-Einstellungen setzt wie vor Runde 29).
pub fn part_page_rule_for(labels: &[String], settings: &TypesetSettings) -> String {
    if let Some(part) = settings.part_page.as_ref() {
        return part_page_rule_with(labels, Some(part));
    }
    if rules(settings).is_empty() {
        return crate::marker::part_page_rule(labels);
    }
    part_page_rule_with(labels, Some(&PartPage { side: PartSide::Right, blank_back: false }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::typeset::model::{Footnotes, Margins, PageFormat, PageNumbers};

    #[test]
    fn only_the_reference_switch_gives_no_block() {
        let s = TypesetSettings { xref_with_page: Some(true), ..Default::default() };
        assert_eq!(rules(&s), "", "xref_with_page ist keine Satz-Regel und ändert keine Datei (Vertrag §6.5)");
    }

    #[test]
    fn register_and_glossary_fields_give_no_block() {
        let s = TypesetSettings {
            glossary_page: Some(crate::typeset::GlossaryPosition::AfterIndexes),
            index_pages: Some(crate::typeset::IndexPages { registers: vec!["stichwort".into()], columns: 2 }),
            glossary_first_mention: Some(true),
            ..Default::default()
        };
        assert_eq!(rules(&s), "", "die R31-Felder steuern Projektion und Buchende, keine Zeile im Regelblock (§2.2)");
    }

    #[test]
    fn numbers_are_written_with_a_decimal_point() {
        assert_eq!(num(10.5).as_deref(), Some("10.5"), "Typst braucht den Dezimalpunkt");
        assert_eq!(num(20.0).as_deref(), Some("20"), "ganze Zahlen ohne .0");
        assert_eq!(num(f64::NAN), None, "keine Regel aus einer ungültigen Zahl");
    }

    #[test]
    fn margins_add_the_binding_to_the_inside() {
        let s = TypesetSettings {
            page_format: Some(PageFormat::A5),
            margins: Some(Margins {
                inside_mm: 20.0,
                outside_mm: 15.0,
                top_mm: 18.0,
                bottom_mm: 22.0,
                binding_mm: 5.0,
            }),
            ..Default::default()
        };
        let block = rules(&s);
        assert!(block.contains("#set page(width: 148mm, height: 210mm)\n"), "A5 als Maß: {block}");
        assert!(
            block.contains("#set page(margin: (inside: 25mm, outside: 15mm, top: 18mm, bottom: 22mm))\n"),
            "Innenrand = 20 + 5 mm Bundsteg: {block}"
        );
    }

    #[test]
    fn footnotes_restart_only_at_chapters() {
        let s = TypesetSettings {
            footnotes: Some(Footnotes { numbering: FootnoteNumbering::Symbols, restart: FootnoteRestart::PerChapter }),
            page_numbers: Some(PageNumbers {
                position: PageNumberPosition::BottomOutside,
                start: PageNumberStart::FirstChapter,
            }),
            ..Default::default()
        };
        let block = rules(&s);
        assert!(block.contains("#set footnote(numbering: \"*\")"), "Symbole: {block}");
        assert!(block.contains("counter(footnote).update(0)"), "Neubeginn je Kapitel: {block}");
        assert!(block.contains("counter(page).update(1)"), "Zählung ab dem ersten Kapitel: {block}");
    }
}
