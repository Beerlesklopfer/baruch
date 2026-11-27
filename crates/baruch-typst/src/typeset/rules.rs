// SPDX-License-Identifier: AGPL-3.0-or-later
//! Typst rules from the typesetting settings (Round 29b, contract §5.1/§5.2).
//!
//! [`rules`] delivers "" for empty settings and otherwise a block that starts with the line
//! [`RULES_MARK`](super::RULES_MARK) and ends with '\n'. The store puts it in `snapshot::head` behind work
//! template, scheme rules and fallback rules – so it applies equally in export, mirror, typesetting preview, book
//! PDF and cover.
//!
//! **Form:** one line per field in [`FIELDS`](super::FIELDS) order; several lines only where Typst needs it (helper
//! functions for page numbers and running heads, restart of the footnotes). `xref_with_page` is not a typesetting
//! rule: settings that only set this field yield "" (the switch changes no file, contract §6.5).
//!
//! **Why the rules look like this** (measured in `baruch-render/tests/typeset_measure.rs`):
//! - **Chapter** = heading level 1 without the label `<baruch-part>` (footnotes, running heads, chapter-start
//!   pages). The break of the chapter start, however, applies to every heading of level 1, also to parts: in the
//!   export (without the part rule of the typesetting) parts are headings of level 1 and so start at least like a
//!   chapter. So the rule is exactly the template rule that can be taken over (`it => { pagebreak(…); it }`), and
//!   „Vorlagenregel übernehmen“ loses nothing in the export (finding R29; before, the rule let parts through, after
//!   taking it over parts no longer started on a new page in the export). In the typesetting the part rule, which
//!   does not return `it`, applies before it. `pagebreak(to: "odd")` counts the physical pages in Typst – so empty
//!   pages are created correctly, even if the page counting only starts at the first chapter.
//! - **Page counting from the first chapter on:** the cross-reference „S. n“ reads `counter(page).at(<kN>)` at the
//!   heading. A `counter(page).update(1)` in the show rule of the heading would stand behind its start marker
//!   (Typst puts everything a show rule delivers between start and end marker of the element) and would apply too
//!   late for the reference. Therefore the reset stands in the page background (`page.background`) of the first
//!   chapter page: Typst puts the background into the page frame before header and content, so the reset applies to
//!   everything on this page – footer, header and references show the same number.
//! - **Empty pages** (running head and page number are omitted): before every chapter heading and (with an own part
//!   rule) part heading an invisible marker `<baruch-satz-end>` stands *before* the page break of the template or
//!   of the setting. Typst leaves completed markers before the break (only uncompleted start markers move to the
//!   new page); the marker thus shows where the text before it ends. Pages between marker and heading are empty. If
//!   only the marker stands before the break (for example directly after a part page), Typst puts it at the start
//!   of the next page (position 0 pt) – that counts as empty too. The empty back of a part page carries
//!   `<baruch-satz-blank>`.
//! - Header and footer are `context` expressions that compare the page through `here().page()` with the pages of
//!   the headings; the page position "outside" follows the physical page (odd = right), like the margins
//!   `inside`/`outside`.

use super::model::RULES_MARK;
use super::model::{
    ChapterStart, FootnoteNumbering, FootnoteRestart, PageNumberPosition, PageNumberStart, Paragraph, PartPage,
    PartSide, RunningHead, TypesetSettings,
};

/// Label of the marker before the page break of a chapter or part heading (see module header).
pub(crate) const END_LABEL: &str = "baruch-satz-end";
/// Label of the empty back of a part page.
pub(crate) const BLANK_LABEL: &str = "baruch-satz-blank";

/// Typst number (decimal point, without unnecessary zeros); not finite → `None` (no rule).
fn num(x: f64) -> Option<String> {
    if !x.is_finite() {
        return None;
    }
    let rounded = (x * 1000.0).round() / 1000.0;
    Some(format!("{rounded}"))
}

/// Typst string with escaping.
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

/// Condition "is a part heading" for the variable `it` of a show rule.
fn is_part(var: &str) -> String {
    format!("{var}.at(\"label\", default: none) == <{}>", crate::marker::PART_LABEL)
}

/// Helper functions for headers and footers (page numbers, running heads) and the marker before the break.
///
/// `baruch-satz-quiet(p)`: part page, empty back or empty page before a chapter heading. An empty page lies between
/// the marker `<baruch-satz-end>` (before the break of the chapter rule) and the heading behind it. Without a
/// chapter rule the marker stands at the same place as its heading and follows it in the query (integrator R29):
/// such a marker does not count, otherwise all pages up to the next heading would be "empty" (without page number
/// and running head).
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

/// Expression of the page number in a `context` with page `p` (string or `none`).
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

/// Header: running head (`title` = Typst expression with `p`, or `None`) and page number top outside (`folio`).
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

/// Typst rule block for `settings`; empty settings (and those that only set `xref_with_page` or the R31 fields
/// `glossary_page`, `index_pages`, `glossary_first_mention`) → "".
pub fn rules(settings: &TypesetSettings) -> String {
    // R31-S1: the three index/glossary fields control crate::index (projection, back matter), no line in the rule
    // block.
    let typeset = TypesetSettings {
        xref_with_page: None,
        glossary_page: None,
        index_pages: None,
        glossary_first_mention: None,
        // R22: the draft only goes into the typesetting source of the outputs (typeset::draft_rule), never into the
        // rule block.
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

    // Chapter start (heading level 1; parts break likewise in the export, see module header).
    if let Some(start) = s.chapter_start {
        line(match start {
            ChapterStart::Continuous => "// Kapitelanfang: fortlaufend (kein Seitenumbruch vor Kapiteln)".to_owned(),
            ChapterStart::NewPage => "#show heading.where(level: 1): it => { pagebreak(weak: true); it }".to_owned(),
            ChapterStart::RightPage => {
                "#show heading.where(level: 1): it => { pagebreak(weak: true, to: \"odd\"); it }".to_owned()
            }
        });
    }
    // Part page: takes effect through the part rule of the typesetting (`part_page_rule_with`), only noted here.
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
                // No space except the line spacing: paragraph spacing = line spacing (Typst default 0.65 em).
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

    // Page numbers and running heads share helper functions and the header.
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

/// Rule of the part pages (like [`crate::marker::part_page_rule`]) with side and empty back from `part`.
/// `part_page_rule_with(labels, None)` is byte-identical to `marker::part_page_rule(labels)`.
///
/// With `part`: `side` Right = a right-hand page of its own (`to: "odd"`), Any = next page; `blank_back` = the page
/// after the part page stays empty (two hard breaks, between them the marker `<baruch-satz-blank>`, so that header
/// and footer are omitted there). Before the first break stands the marker `<baruch-satz-end>` (to recognise empty
/// pages before it).
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
    // The standard rule starts with `{ pagebreak(weak: true, to: "odd"); ` and ends with `pagebreak(weak: true) }`;
    // in between (counter, number, title) it stays verbatim.
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

/// Rule of the part pages for a work with the typesetting settings `settings` – the way typesetting
/// (`typeset::prepare`), preview and measuring tests put it behind the rule block [`rules`].
///
/// - With `part_page`: [`part_page_rule_with`] with this setting.
/// - Without `part_page`, but with a rule block (for example only page numbers or running heads): the same page as
///   the rule from Round 14 (a right-hand page of its own, no empty back), but with the marker `<baruch-satz-end>`
///   before the break. The part rule applies before the show rule of the rule block and does not return `it`;
///   without a marker of its own `baruch-satz-quiet` would not recognise the empty page before the part, and it
///   would carry page number and running head (finding R29).
/// - Without a rule block: byte-identical to `marker::part_page_rule` (a work without typesetting settings typesets
///   as before Round 29).
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
