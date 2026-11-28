// SPDX-License-Identifier: AGPL-3.0-or-later
//! Cue sheet („Stichwortzettel“, Round 16c): build from finished cards (per outline item number, title, time
//! mark, duration, key points, Bible references) a Typst source for large print that [`crate::compile_pdf`]
//! typesets.
//!
//! - **Pure:** which key points a section has is decided by the server (`pdf::notes`); here
//!   only the typesetting is done.
//! - **Formats:** [`NotesFormat::A5`] (A5 portrait, default 16 pt, sections flow one after another) and
//!   [`NotesFormat::Cards`] (cards A6 landscape, default 14 pt, every section starts on a card of its
//!   own – long sections run over onto further cards). Font size [`MIN_FONT_PT`] to
//!   [`MAX_FONT_PT`], clamped outside.
//! - **Safely escaped:** no text of the cards is read as Typst markup. Each one is a
//!   Typst string (`"…"`, [`typst_string`]) in the source and is typeset as text; `#`, `[`, `]`,
//!   `*`, `$`, `\`, `<`, `@` therefore appear verbatim. Every title and key point is additionally in the
//!   document as `metadata` with the label `<baruch-note>` (checkable through `query`).

use baruch_core::WorkLanguage;
use baruch_core::book_text::book_t;

/// Smallest font size in points.
pub const MIN_FONT_PT: u32 = 14;
/// Largest font size in points.
pub const MAX_FONT_PT: u32 = 28;

/// Print format of the cue sheet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotesFormat {
    /// A5 portrait, sections one after another.
    A5,
    /// Cards A6 landscape, at least one card per section.
    Cards,
}

impl NotesFormat {
    /// All formats in the order of the selection.
    pub const ALL: [NotesFormat; 2] = [NotesFormat::A5, NotesFormat::Cards];

    /// From the interface name `a5` or `karten` (case does not matter).
    pub fn parse(text: &str) -> Option<NotesFormat> {
        match text.trim().to_ascii_lowercase().as_str() {
            "a5" => Some(NotesFormat::A5),
            "karten" => Some(NotesFormat::Cards),
            _ => None,
        }
    }

    /// Interface name: `a5` or `karten`.
    pub fn as_str(self) -> &'static str {
        match self {
            NotesFormat::A5 => "a5",
            NotesFormat::Cards => "karten",
        }
    }

    /// Display: „A5 hoch“ or „Karten (A6 quer)“.
    pub fn label(self) -> &'static str {
        match self {
            NotesFormat::A5 => "A5 hoch",
            NotesFormat::Cards => "Karten (A6 quer)",
        }
    }

    /// Default of the font size: 16 pt (A5) or 14 pt (cards).
    pub fn default_font_pt(self) -> u32 {
        match self {
            NotesFormat::A5 => 16,
            NotesFormat::Cards => 14,
        }
    }
}

/// A card: one outline item.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NotesCard {
    /// Number according to the scheme of the work ("I.", "2.1") or empty.
    pub number: String,
    pub title: String,
    /// Number of ancestors (0 = top level); indents the title.
    pub depth: usize,
    /// Time mark at which the item starts ("3:20").
    pub start_clock: String,
    /// Duration of its own text ("1:05").
    pub clock: String,
    pub points: Vec<String>,
    /// Bible references in uniform notation.
    pub refs: Vec<String>,
}

/// The whole sheet.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NotesDoc {
    pub title: String,
    /// Total duration ("18:30").
    pub total_clock: String,
    /// Target time ("20:00"), if set.
    pub target_clock: Option<String>,
    pub cards: Vec<NotesCard>,
    /// Round 39b: work language of the generated words (title „Stichwortzettel“, „Gesamt“, „Ziel“, „Keine
    /// Abschnitte.“) and of `text(lang:)`; German exactly as before.
    pub lang: WorkLanguage,
}

/// Text as a Typst string in quotation marks: `\` and `"` escaped, line breaks and
/// tabs as `\n`/`\t`, other control characters as `\u{…}`. That way nothing of it is read as markup or
/// code.
pub fn typst_string(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => out.push_str(&format!("\\u{{{:x}}}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn string_array(items: &[String]) -> String {
    let mut out = String::from("(");
    for item in items {
        out.push_str(&typst_string(item));
        out.push_str(", ");
    }
    out.push(')');
    out
}

/// Typst source of the sheet (one file, no images). The head of every card (title, times, line) is a
/// `block(sticky: true)`: it stays with its key points and never stands alone at the bottom of a page
/// (integration Round 16, visual check). `font_pt` is clamped to
/// [`MIN_FONT_PT`]–[`MAX_FONT_PT`].
pub fn notes_source(doc: &NotesDoc, format: NotesFormat, font_pt: u32) -> String {
    let size = font_pt.clamp(MIN_FONT_PT, MAX_FONT_PT);
    let mut out = String::new();
    out.push_str("// Stichwortzettel – erzeugt von Baruch (baruch-render::notes). Alle Texte sind Zeichenketten.\n");
    let t = |key: &str, args: &[(&str, &str)]| book_t(doc.lang, key, args);
    let title = t("book.notes_sheet.title", &[("title", &doc.title)]);
    out.push_str(&format!("#set document(title: {})\n", typst_string(&title)));
    let (page, margin, below) = match format {
        NotesFormat::A5 => ("paper: \"a5\"", "(x: 14mm, top: 16mm, bottom: 14mm)", "1.1em"),
        NotesFormat::Cards => ("paper: \"a6\", flipped: true", "(x: 9mm, top: 11mm, bottom: 8mm)", "0.8em"),
    };
    let head = typst_string(&doc.title);
    out.push_str(&format!(
        "#set page({page}, margin: {margin}, header: context align(right, text(size: 0.6em, fill: luma(90))[#{head} · #counter(page).display()]))\n"
    ));
    out.push_str(&format!("#set text(size: {size}pt, lang: \"{}\")\n", doc.lang.as_str()));
    out.push_str("#set par(justify: false, leading: 0.75em)\n");
    out.push_str("#set list(indent: 0.4em, body-indent: 0.5em, spacing: 0.7em)\n");
    out.push_str(&format!(
        "#let card(c) = block(width: 100%, breakable: true, below: {below})[\n\
         \x20 #metadata(c.title) <baruch-note>\n\
         \x20 #for p in c.points [#metadata(p) <baruch-note>]\n\
         \x20 #block(sticky: true, below: 0.65em)[\n\
         \x20   #grid(columns: (1fr, auto), column-gutter: 0.8em,\n\
         \x20     pad(left: c.depth * 0.8em, text(weight: \"bold\", size: 1.1em)[#if c.number != \"\" [#c.number ]#c.title]),\n\
         \x20     align(right, text(size: 0.75em, fill: luma(70))[#c.start (#c.clock)]))\n\
         \x20   #v(-0.3em)\n\
         \x20   #line(length: 100%, stroke: 0.5pt + luma(150))\n\
         \x20 ]\n\
         \x20 #if c.points.len() > 0 [#list(..c.points.map(p => [#p]))]\n\
         \x20 #if c.refs.len() > 0 [#text(size: 0.85em, style: \"italic\")[#c.refs.join(\" · \")]]\n\
         ]\n"
    ));
    // Head: title and times (with cards on the first card above the first section).
    let mut times = t("book.notes_sheet.total", &[("clock", &doc.total_clock)]);
    if let Some(target) = &doc.target_clock {
        times.push_str(&format!(" · {}", t("book.notes_sheet.target", &[("clock", target)])));
    }
    out.push_str(&format!(
        "#text(size: 0.8em)[#strong({}) #h(1fr) #{}]\n\n",
        typst_string(&doc.title),
        typst_string(&times)
    ));
    if doc.cards.is_empty() {
        out.push_str(&format!("#emph({})\n", typst_string(&t("book.notes_sheet.empty", &[]))));
    }
    for (index, card) in doc.cards.iter().enumerate() {
        if format == NotesFormat::Cards && index > 0 {
            out.push_str("#pagebreak()\n");
        }
        out.push_str(&format!(
            "#card((number: {}, title: {}, depth: {}, start: {}, clock: {}, points: {}, refs: {}))\n",
            typst_string(&card.number),
            typst_string(&card.title),
            card.depth.min(6),
            typst_string(&card.start_clock),
            typst_string(&card.clock),
            string_array(&card.points),
            string_array(&card.refs),
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strings_mask_backslash_quote_and_control_characters() {
        assert_eq!(typst_string("a\"b\\c"), "\"a\\\"b\\\\c\"", "Anführungszeichen und Rückstrich maskiert");
        assert_eq!(typst_string("x\ny\u{7}"), "\"x\\ny\\u{7}\"", "Zeilenumbruch und Steuerzeichen als Folge");
        assert_eq!(typst_string("#[*]$<@"), "\"#[*]$<@\"", "Markup-Zeichen bleiben in der Zeichenkette Text");
    }

    #[test]
    fn the_head_of_a_card_sticks_to_its_points() {
        let doc = NotesDoc {
            title: "Probe".into(),
            total_clock: "1:00".into(),
            target_clock: None,
            cards: vec![NotesCard {
                number: "I.".into(),
                title: "Erster Gedanke".into(),
                depth: 0,
                start_clock: "0:00".into(),
                clock: "1:00".into(),
                points: vec!["Licht".into()],
                refs: vec![],
            }],
            lang: WorkLanguage::De,
        };
        let source = notes_source(&doc, NotesFormat::A5, 16);
        assert!(
            source.contains("#block(sticky: true"),
            "der Kopf jeder Karte ist sticky, sonst stand in der Sichtprüfung „III. Zuspruch“ allein unten auf Seite 1"
        );
    }

    #[test]
    fn formats_have_names_labels_and_default_sizes() {
        assert_eq!(NotesFormat::parse("KARTEN"), Some(NotesFormat::Cards), "Groß-/Kleinschreibung egal");
        assert_eq!(NotesFormat::parse("a6"), None, "unbekanntes Format");
        assert_eq!(NotesFormat::A5.default_font_pt(), 16, "A5: Vorgabe 16 pt (Vertrag)");
        assert_eq!(NotesFormat::Cards.default_font_pt(), 14, "Karten: Vorgabe 14 pt (Vertrag)");
    }
}
