// SPDX-License-Identifier: AGPL-3.0-or-later
//! Stichwortzettel (Runde 16c): aus fertigen Karten (je Gliederungspunkt Nummer, Titel, Zeitmarke,
//! Dauer, Stichpunkte, Bibelstellen) eine Typst-Quelle für großen Druck bauen, die [`crate::compile_pdf`]
//! setzt.
//!
//! - **Rein:** Welche Stichpunkte ein Abschnitt hat, entscheidet der Server (`pdf::notes`); hier
//!   entsteht nur der Satz.
//! - **Formate:** [`NotesFormat::A5`] (A5 hoch, Vorgabe 16 pt, Abschnitte fließen nacheinander) und
//!   [`NotesFormat::Cards`] (Karten A6 quer, Vorgabe 14 pt, jeder Abschnitt beginnt auf einer eigenen
//!   Karte – lange Abschnitte laufen auf weitere Karten über). Schriftgröße [`MIN_FONT_PT`] bis
//!   [`MAX_FONT_PT`], außerhalb geklemmt.
//! - **Sicher maskiert:** Kein Text der Karten wird als Typst-Markup gelesen. Jeder steht als
//!   Typst-Zeichenkette (`"…"`, [`typst_string`]) in der Quelle und wird als Text gesetzt; `#`, `[`, `]`,
//!   `*`, `$`, `\`, `<`, `@` erscheinen darum wörtlich. Jeder Titel und Stichpunkt steht zusätzlich als
//!   `metadata` mit dem Label `<baruch-note>` im Dokument (prüfbar über `query`).

/// Kleinste Schriftgröße in Punkt.
pub const MIN_FONT_PT: u32 = 14;
/// Größte Schriftgröße in Punkt.
pub const MAX_FONT_PT: u32 = 28;

/// Druckformat des Stichwortzettels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotesFormat {
    /// A5 hoch, Abschnitte nacheinander.
    A5,
    /// Karten A6 quer, je Abschnitt mindestens eine Karte.
    Cards,
}

impl NotesFormat {
    /// Alle Formate in der Reihenfolge der Auswahl.
    pub const ALL: [NotesFormat; 2] = [NotesFormat::A5, NotesFormat::Cards];

    /// Aus dem Schnittstellen-Namen `a5` bzw. `karten` (Groß-/Kleinschreibung egal).
    pub fn parse(text: &str) -> Option<NotesFormat> {
        match text.trim().to_ascii_lowercase().as_str() {
            "a5" => Some(NotesFormat::A5),
            "karten" => Some(NotesFormat::Cards),
            _ => None,
        }
    }

    /// Schnittstellen-Name: `a5` bzw. `karten`.
    pub fn as_str(self) -> &'static str {
        match self {
            NotesFormat::A5 => "a5",
            NotesFormat::Cards => "karten",
        }
    }

    /// Anzeige: „A5 hoch“ bzw. „Karten (A6 quer)“.
    pub fn label(self) -> &'static str {
        match self {
            NotesFormat::A5 => "A5 hoch",
            NotesFormat::Cards => "Karten (A6 quer)",
        }
    }

    /// Vorgabe der Schriftgröße: 16 pt (A5) bzw. 14 pt (Karten).
    pub fn default_font_pt(self) -> u32 {
        match self {
            NotesFormat::A5 => 16,
            NotesFormat::Cards => 14,
        }
    }
}

/// Eine Karte: ein Gliederungspunkt.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NotesCard {
    /// Nummer nach dem Schema des Werks („I.“, „2.1“) oder leer.
    pub number: String,
    pub title: String,
    /// Zahl der Vorfahren (0 = oberste Ebene); rückt den Titel ein.
    pub depth: usize,
    /// Zeitmarke, zu der der Punkt beginnt („3:20“).
    pub start_clock: String,
    /// Dauer des eigenen Texts („1:05“).
    pub clock: String,
    pub points: Vec<String>,
    /// Bibelstellen in einheitlicher Schreibweise.
    pub refs: Vec<String>,
}

/// Der ganze Zettel.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NotesDoc {
    pub title: String,
    /// Gesamtdauer („18:30“).
    pub total_clock: String,
    /// Zielzeit („20:00“), wenn gesetzt.
    pub target_clock: Option<String>,
    pub cards: Vec<NotesCard>,
}

/// Text als Typst-Zeichenkette in Anführungszeichen: `\` und `"` maskiert, Zeilenumbrüche und
/// Tabulatoren als `\n`/`\t`, übrige Steuerzeichen als `\u{…}`. So wird nichts davon als Markup oder
/// Code gelesen.
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

/// Typst-Quelle des Zettels (eine Datei, keine Bilder). Der Kopf jeder Karte (Titel, Zeiten, Linie) ist ein
/// `block(sticky: true)`: Er bleibt bei seinen Stichpunkten und steht nie allein unten auf einer Seite
/// (Integration Runde 16, Sichtprüfung). `font_pt` wird auf
/// [`MIN_FONT_PT`]–[`MAX_FONT_PT`] geklemmt.
pub fn notes_source(doc: &NotesDoc, format: NotesFormat, font_pt: u32) -> String {
    let size = font_pt.clamp(MIN_FONT_PT, MAX_FONT_PT);
    let mut out = String::new();
    out.push_str("// Stichwortzettel – erzeugt von Baruch (baruch-render::notes). Alle Texte sind Zeichenketten.\n");
    out.push_str(&format!("#set document(title: {})\n", typst_string(&format!("Stichwortzettel – {}", doc.title))));
    let (page, margin, below) = match format {
        NotesFormat::A5 => ("paper: \"a5\"", "(x: 14mm, top: 16mm, bottom: 14mm)", "1.1em"),
        NotesFormat::Cards => ("paper: \"a6\", flipped: true", "(x: 9mm, top: 11mm, bottom: 8mm)", "0.8em"),
    };
    let head = typst_string(&doc.title);
    out.push_str(&format!(
        "#set page({page}, margin: {margin}, header: context align(right, text(size: 0.6em, fill: luma(90))[#{head} · #counter(page).display()]))\n"
    ));
    out.push_str(&format!("#set text(size: {size}pt, lang: \"de\")\n"));
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
    // Kopf: Titel und Zeiten (bei Karten auf der ersten Karte über dem ersten Abschnitt).
    let mut times = format!("Gesamt {}", doc.total_clock);
    if let Some(target) = &doc.target_clock {
        times.push_str(&format!(" · Ziel {target}"));
    }
    out.push_str(&format!(
        "#text(size: 0.8em)[#strong({}) #h(1fr) #{}]\n\n",
        typst_string(&doc.title),
        typst_string(&times)
    ));
    if doc.cards.is_empty() {
        out.push_str("#emph(\"Keine Abschnitte.\")\n");
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
