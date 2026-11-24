// SPDX-License-Identifier: AGPL-3.0-or-later
//! Typst aus gelesenem Text (Runde 21a): Maskierung von Fließtext ([`escape_text`]), Inline-Stücke mit
//! Hervorhebungen und Fußnoten ([`Inline`], [`render_inline`]) und das ganze Werk aus der vom Menschen bestätigten
//! Gliederung ([`build_typst`]).
//!
//! Text aus PDF und EPUB ist Text, nie Typst: Jedes Zeichen, das in Typst-Markup etwas bedeutet, bekommt einen
//! Backslash (Typst lässt jedes Zeichen nach `\` wörtlich stehen). Hervorhebungen werden `_…_` bzw. `*…*`, wo Typst
//! die Zeichen als Auszeichnung liest (nicht mitten im Wort), sonst `#emph[…]` bzw. `#strong[…]`; Fußnoten
//! `#footnote[…]`.

use serde::{Deserialize, Serialize};

use crate::import::ImportWarning;

/// Ein Abschnitt des gelesenen Dokuments (Vorschlag der Gliederung samt Text).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Section {
    /// Ebene 1–3 (Kapitel, Abschnitt, Unterabschnitt).
    pub level: u8,
    pub title: String,
    /// Vermutlich ein Teil (nur Hinweis in der Vorschau; angelegt wird ein Teil nur, wenn der Mensch ihn so bestätigt).
    #[serde(default)]
    pub part: bool,
    /// Text als Typst-Markup (Absätze durch Leerzeilen getrennt).
    pub body: String,
    /// Derselbe Text ohne Auszeichnung (Zeichenzahl, Textprobe, Volltext der Bibliothek).
    pub plain: String,
}

/// Ein Punkt der vom Menschen bestätigten Gliederung.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Confirmed {
    pub level: u8,
    pub title: String,
    pub part: bool,
    /// Index des vorgeschlagenen Abschnitts, dessen Text hierher gehört; `None` = neuer, leerer Punkt bzw.
    /// Zuordnung nach der Stelle (alle ohne `source`, gleich viele wie vorgeschlagen).
    pub source: Option<usize>,
}

/// Inline-Stück eines Absatzes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Inline {
    /// Klartext (wird maskiert).
    Text(String),
    Emph(Vec<Inline>),
    Strong(Vec<Inline>),
    /// Fußnote mit ihrem Inhalt.
    Footnote(Vec<Inline>),
    Super(Vec<Inline>),
    Sub(Vec<Inline>),
    /// Verweis auf eine http(s)-Adresse.
    Link(String, Vec<Inline>),
    /// Code (wörtlich).
    Code(String),
    /// Zeilenumbruch im Absatz.
    Break,
    /// Fertiger eingebetteter Typst-Ausdruck (etwa `#image("…")` aus einer Fußnote); bleibt unverändert.
    Typst(String),
}

/// Typst-Zeichen, die überall einen Backslash bekommen.
const ESCAPED: &[char] = &['\\', '#', '*', '_', '`', '$', '@', '<', '>', '[', ']', '~'];

/// Klartext für Typst-Markup maskieren. `line_start` = der Text beginnt eine Zeile (Absatzanfang): dann bekommen
/// auch `=`, `-`, `+`, `/` und der Punkt eines führenden `12.` einen Backslash (sonst Überschrift oder Liste). Ein
/// `/` vor `/` oder `*` (Kommentar), ein `-` vor `-` oder `?` (Strich, weiches Trennzeichen) werden ebenfalls
/// maskiert. Zeilenumbrüche im Text werden zu Leerzeichen.
pub fn escape_text(text: &str, line_start: bool) -> String {
    let chars: Vec<char> = text.chars().map(|c| if baruch_typst::is_newline(c) { ' ' } else { c }).collect();
    let mut out = String::with_capacity(text.len() + 8);
    let mut start = line_start;
    let mut digits_then_dot = None;
    if start {
        let digits = chars.iter().take_while(|c| c.is_ascii_digit()).count();
        if digits > 0 && chars.get(digits) == Some(&'.') && chars.get(digits + 1).is_none_or(|c| c.is_whitespace()) {
            digits_then_dot = Some(digits);
        }
    }
    for (i, &c) in chars.iter().enumerate() {
        let next = chars.get(i + 1).copied();
        let escape = ESCAPED.contains(&c)
            || (c == '/' && matches!(next, Some('/' | '*')))
            || (c == '-' && matches!(next, Some('-' | '?')))
            || (start && matches!(c, '=' | '-' | '+' | '/'))
            || digits_then_dot == Some(i);
        if escape {
            out.push('\\');
        }
        out.push(c);
        if !c.is_whitespace() {
            start = false;
        }
    }
    out
}

/// Text in einer Typst-Zeichenkette (`"…"`).
pub fn string_literal(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            c if baruch_typst::is_newline(c) => out.push(' '),
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn wordy(c: Option<char>) -> bool {
    c.is_some_and(char::is_alphanumeric)
}

/// Klartext der Stücke (für Zeichenzahl, Textprobe und Volltext).
pub fn plain_inline(pieces: &[Inline]) -> String {
    let mut out = String::new();
    for piece in pieces {
        match piece {
            Inline::Text(t) | Inline::Code(t) => out.push_str(t),
            Inline::Emph(c) | Inline::Strong(c) | Inline::Super(c) | Inline::Sub(c) | Inline::Link(_, c) => {
                out.push_str(&plain_inline(c));
            }
            Inline::Footnote(c) => {
                out.push_str(" [");
                out.push_str(plain_inline(c).trim());
                out.push(']');
            }
            Inline::Break => out.push('\n'),
            Inline::Typst(_) => {}
        }
    }
    out
}

/// Stücke ohne führenden und abschließenden Leerraum (in Text-Stücken), leere Text-Stücke fallen weg.
pub fn trim_inline(mut pieces: Vec<Inline>) -> Vec<Inline> {
    while let Some(Inline::Text(t)) = pieces.first_mut() {
        let trimmed = t.trim_start().to_owned();
        if trimmed.is_empty() {
            pieces.remove(0);
        } else {
            *t = trimmed;
            break;
        }
    }
    while let Some(Inline::Text(t)) = pieces.last_mut() {
        let trimmed = t.trim_end().to_owned();
        if trimmed.is_empty() {
            pieces.pop();
        } else {
            *t = trimmed;
            break;
        }
    }
    pieces
}

/// Ist der Inhalt leer (nur Leerraum)?
pub fn inline_is_blank(pieces: &[Inline]) -> bool {
    plain_inline(pieces).trim().is_empty()
        && !pieces.iter().any(|p| matches!(p, Inline::Footnote(_) | Inline::Code(_) | Inline::Typst(_)))
}

/// Würde Typst `after` an einen eingebetteten Ausdruck direkt davor hängen (Aufruf mit `(`, Inhaltsblock `[`,
/// Feldzugriff `.Name`, verschlucktes `;`)? Gegenstück zu `needs_guard` im Markdown-Import.
fn needs_guard(after: &str) -> bool {
    let mut chars = after.chars();
    match chars.next() {
        Some('(' | '[' | ';') => true,
        Some('.') => chars.next().is_some_and(|c| c.is_alphabetic() || c == '_'),
        _ => false,
    }
}

/// Stücke als Typst-Markup. `line_start` = sie beginnen einen Absatz. Hinter einem eingebetteten Ausdruck
/// (`#emph[…]`, `#linebreak()`, `#raw("…")` …) steht ein Schutz-`;`, wenn der folgende Text sonst als Aufruf,
/// Inhaltsblock oder Feldzugriff gelesen würde ([`needs_guard`]).
pub fn render_inline(pieces: &[Inline], line_start: bool) -> String {
    let mut out = String::new();
    let mut guard = false;
    let rendered: Vec<String> = pieces.iter().map(|p| render_piece(p, false)).collect();
    for (i, piece) in pieces.iter().enumerate() {
        let at_start = line_start && out.trim().is_empty();
        let text = match piece {
            Inline::Text(t) => escape_text(t, at_start),
            Inline::Emph(c) | Inline::Strong(c) => {
                let (open, name) = if matches!(piece, Inline::Emph(_)) { ('_', "emph") } else { ('*', "strong") };
                let inner = render_inline(c, false);
                if inner.trim().is_empty() {
                    continue;
                }
                let prev = out.chars().last();
                let next = rendered.get(i + 1).and_then(|r| r.chars().next());
                let first = inner.chars().next();
                let last = inner.chars().last();
                let delimiters_hold = !(wordy(prev) && wordy(first))
                    && !(wordy(last) && wordy(next))
                    && !first.is_some_and(char::is_whitespace)
                    && !last.is_some_and(char::is_whitespace)
                    && last != Some('\\');
                if delimiters_hold && !at_start { format!("{open}{inner}{open}") } else { format!("#{name}[{inner}]") }
            }
            other => render_piece(other, at_start),
        };
        if text.is_empty() {
            continue;
        }
        if guard && needs_guard(&text) {
            out.push(';');
        }
        guard = text.starts_with('#');
        out.push_str(&text);
    }
    out
}

fn render_piece(piece: &Inline, line_start: bool) -> String {
    match piece {
        Inline::Text(t) => escape_text(t, line_start),
        Inline::Emph(c) => format!("#emph[{}]", render_inline(c, false)),
        Inline::Strong(c) => format!("#strong[{}]", render_inline(c, false)),
        Inline::Footnote(c) => format!("#footnote[{}]", render_inline(&trim_inline(c.clone()), false)),
        Inline::Super(c) => format!("#super[{}]", render_inline(c, false)),
        Inline::Sub(c) => format!("#sub[{}]", render_inline(c, false)),
        Inline::Link(url, c) => {
            let label = render_inline(c, false);
            if label.trim().is_empty() {
                format!("#link({})", string_literal(url))
            } else {
                format!("#link({})[{label}]", string_literal(url))
            }
        }
        Inline::Code(code) => format!("#raw({})", string_literal(code)),
        Inline::Break => "#linebreak()".to_owned(),
        Inline::Typst(expr) => expr.clone(),
    }
}

/// Ergebnis von [`assign`]: je bestätigtem Punkt (Index) die Abschnitte (Indizes), deren Text er bekommt.
pub type Assignment = Vec<(usize, Vec<usize>)>;

/// Vom Menschen bestätigte Gliederung auf die vorgeschlagenen Abschnitte abbilden: Liste der (Punkt, Abschnitte,
/// deren Text er bekommt). Regeln: Tragen Punkte `source`, gehören alle Punkte mit `source` zu genau diesem
/// Abschnitt (aufsteigend, jeder höchstens einmal; Punkte ohne `source` sind neue, leere Punkte); ohne jedes `source`
/// müssen es gleich viele Punkte wie Abschnitte sein (Zuordnung nach der Stelle). Der Text eines gelöschten
/// Abschnitts geht an den vorigen behaltenen (am Anfang an den nächsten), nie verloren; dafür gibt es eine Warnung.
pub fn assign(sections: &[Section], confirmed: &[Confirmed]) -> Result<(Assignment, Vec<ImportWarning>), String> {
    if confirmed.is_empty() {
        return Err("Die Gliederung ist leer: mindestens ein Punkt (Kapitel) ist nötig.".into());
    }
    let mut warnings = Vec::new();
    let any_source = confirmed.iter().any(|c| c.source.is_some());
    let sources: Vec<Option<usize>> = if any_source {
        let mut last: Option<usize> = None;
        for c in confirmed {
            if let Some(s) = c.source {
                if s >= sections.len() {
                    return Err(format!(
                        "source {s}: den Abschnitt gibt es im Entwurf nicht (0–{}).",
                        sections.len().saturating_sub(1)
                    ));
                }
                if last.is_some_and(|l| s <= l) {
                    return Err(
                        "Die Reihenfolge der Abschnitte bleibt wie im Dokument: source muss aufsteigen (Punkte \
                         löschen, umbenennen, umstufen geht, verschieben nicht)."
                            .into(),
                    );
                }
                last = Some(s);
            }
        }
        confirmed.iter().map(|c| c.source).collect()
    } else {
        if confirmed.len() != sections.len() {
            return Err(format!(
                "Die Gliederung hat {} Punkte, der Entwurf {} Abschnitte: ohne source müssen es gleich viele sein \
                 (zum Löschen je Punkt source angeben).",
                confirmed.len(),
                sections.len()
            ));
        }
        (0..sections.len()).map(Some).collect()
    };
    let mut owner: Vec<Option<usize>> = vec![None; sections.len()];
    for (item, source) in sources.iter().enumerate() {
        if let Some(s) = source {
            owner[*s] = Some(item);
        }
    }
    let mut merged: Vec<(usize, Vec<usize>)> = (0..confirmed.len()).map(|i| (i, Vec::new())).collect();
    let mut current: Option<usize> = None;
    let mut pending: Vec<usize> = Vec::new();
    for (index, section) in sections.iter().enumerate() {
        match owner[index] {
            Some(item) => {
                current = Some(item);
                if !pending.is_empty() {
                    merged[item].1.append(&mut pending);
                }
                merged[item].1.push(index);
            }
            None => {
                if !section.plain.trim().is_empty() || !section.body.trim().is_empty() {
                    warnings.push(ImportWarning {
                        code: "import-outline-merged".into(),
                        message: format!(
                            "Der Punkt „{}“ ist gestrichen; sein Text steht jetzt im {} Punkt.",
                            section.title,
                            if current.is_some() { "vorigen" } else { "nächsten" }
                        ),
                        line: None,
                    });
                }
                match current {
                    Some(item) => merged[item].1.push(index),
                    None => pending.push(index),
                }
            }
        }
    }
    if !pending.is_empty() {
        // Kein Punkt mit Text-Zuordnung: alles an den ersten Punkt.
        merged[0].1.append(&mut pending);
    }
    Ok((merged, warnings))
}

/// Das Werk als Typst: Werkvorlage (`#set document(title, author)`, ggf. Sprache), dann je bestätigtem Punkt die
/// Überschrift (Teil über die Teil-Markierung von baruch-typst) und der Text der zugeordneten Abschnitte. Bilder
/// stehen schon als `#image("…")` im Text.
pub fn build_typst(
    title: &str,
    author: Option<&str>,
    lang: Option<&str>,
    sections: &[Section],
    confirmed: &[Confirmed],
) -> Result<(String, Vec<ImportWarning>), String> {
    for item in confirmed {
        if item.title.trim().is_empty() {
            return Err("Ein Punkt der Gliederung hat keinen Titel (zum Streichen die Zeile leeren).".into());
        }
        if !(1..=3).contains(&item.level) {
            return Err(format!("„{}“: Ebene {} – erlaubt sind 1 bis 3.", item.title, item.level));
        }
        if item.part && item.level != 1 {
            return Err(format!("„{}“: Ein Teil steht auf der obersten Ebene (Ebene 1).", item.title));
        }
    }
    let (assigned, warnings) = assign(sections, confirmed)?;
    let mut out = String::new();
    out.push_str("#set document(title: ");
    out.push_str(&string_literal(title));
    if let Some(author) = author.map(str::trim).filter(|a| !a.is_empty()) {
        out.push_str(", author: ");
        out.push_str(&string_literal(author));
    }
    out.push_str(")\n");
    if let Some(lang) = lang.filter(|l| l.len() == 2 && l.chars().all(|c| c.is_ascii_lowercase())) {
        out.push_str(&format!("#set text(lang: \"{lang}\")\n"));
    }
    for (item, parts) in assigned {
        let point = &confirmed[item];
        out.push('\n');
        let line = baruch_typst::heading_line(if point.part { 0 } else { point.level }, point.title.trim(), None);
        out.push_str(line.trim_end_matches('\n'));
        out.push('\n');
        let body: Vec<&str> = parts.iter().map(|&i| sections[i].body.trim()).filter(|b| !b.is_empty()).collect();
        if !body.is_empty() {
            out.push('\n');
            out.push_str(&body.join("\n\n"));
            out.push('\n');
        }
    }
    Ok((out, warnings))
}

/// Erste Zeichen des Texts (höchstens `max`, an einer Wortgrenze, mit „…“).
pub fn sample(plain: &str, max: usize) -> String {
    let flat = plain.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.chars().count() <= max {
        return flat;
    }
    let cut: String = flat.chars().take(max).collect();
    let cut = match cut.rfind(' ') {
        Some(i) if i > max / 2 => cut[..i].to_owned(),
        _ => cut,
    };
    format!("{cut} …")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(t: &str) -> Inline {
        Inline::Text(t.into())
    }

    #[test]
    fn plain_text_is_masked_so_typst_sets_it_literally() {
        assert_eq!(
            escape_text("#set *fett* _x_ $a$ @ref <l> [k] ~ `c` \\", false),
            "\\#set \\*fett\\* \\_x\\_ \\$a\\$ \\@ref \\<l\\> \\[k\\] \\~ \\`c\\` \\\\"
        );
        assert_eq!(escape_text("= Kein Titel", true), "\\= Kein Titel", "am Absatzanfang wäre = eine Überschrift");
        assert_eq!(escape_text("- keine Liste", true), "\\- keine Liste", "am Absatzanfang wäre - eine Liste");
        assert_eq!(escape_text("12. Mai", true), "12\\. Mai", "am Absatzanfang wäre 12. eine Aufzählung");
        assert_eq!(escape_text("am 12. Mai", true), "am 12. Mai", "mitten im Satz bleibt der Punkt");
        assert_eq!(escape_text("a = b - c", false), "a = b - c", "mitten im Satz bleiben = und - stehen");
        assert_eq!(
            escape_text("x // y /* z", false),
            "x \\// y \\/\\* z",
            "kein Kommentar (der Stern ist ohnehin maskiert)"
        );
        assert_eq!(
            escape_text("A--B -? C", false),
            "A\\--B \\-? C",
            "kein Halbgeviertstrich, kein weiches Trennzeichen"
        );
        assert_eq!(escape_text("Zeile\neins", false), "Zeile eins", "Zeilenumbrüche werden Leerzeichen");
    }

    #[test]
    fn emphasis_uses_delimiters_only_where_typst_reads_them() {
        let pieces = vec![text("Ein "), Inline::Emph(vec![text("leises")]), text(" Wort.")];
        assert_eq!(render_inline(&pieces, true), "Ein _leises_ Wort.", "an Wortgrenzen _…_");
        let inner = vec![text("Haus"), Inline::Strong(vec![text("tür")]), text("schloss")];
        assert_eq!(render_inline(&inner, true), "Haus#strong[tür]schloss", "mitten im Wort #strong[…]");
        let footnote = vec![text("Satz."), Inline::Footnote(vec![text(" Eine Quelle. ")])];
        assert_eq!(render_inline(&footnote, true), "Satz.#footnote[Eine Quelle.]", "Fußnote als #footnote");
        let first = vec![Inline::Emph(vec![text("Anfang")]), text(" des Absatzes")];
        assert_eq!(render_inline(&first, true), "#emph[Anfang] des Absatzes", "am Absatzanfang sicher als Funktion");
        assert_eq!(
            render_inline(&[Inline::Link("https://x.example/a\"b".into(), vec![text("hier")])], false),
            "#link(\"https://x.example/a\\\"b\")[hier]",
            "Adresse als Zeichenkette maskiert"
        );
    }

    #[test]
    fn embedded_expressions_get_a_guard_before_parentheses_and_fields() {
        // Typst hängt an einen eingebetteten Ausdruck direkt folgendes `(`/`[` (Aufruf) bzw. `.Name` (Feld) an:
        // `#linebreak()(Fortsetzung)` wäre ein Aufruf des Ergebnisses und scheitert beim Satz.
        let pieces = vec![text("Erste Zeile"), Inline::Break, text("(Fortsetzung) hier.")];
        assert_eq!(render_inline(&pieces, true), "Erste Zeile#linebreak();(Fortsetzung) hier.", "Schutz vor (");
        let pieces = vec![
            text("Ein "),
            Inline::Code("x".into()),
            text(".Feld und Haus"),
            Inline::Emph(vec![text("tür")]),
            text("(n)."),
        ];
        assert_eq!(
            render_inline(&pieces, true),
            "Ein #raw(\"x\");.Feld und Haus#emph[tür];(n).",
            "Schutz vor .Name und ( hinter #raw und #emph"
        );
        let pieces = vec![text("Satz"), Inline::Footnote(vec![text("Quelle")]), text(". Weiter; und")];
        assert_eq!(
            render_inline(&pieces, true),
            "Satz#footnote[Quelle]. Weiter; und",
            "Satzende braucht keinen Schutz"
        );
        let pieces = vec![text("A"), Inline::Break, text("; B")];
        assert_eq!(render_inline(&pieces, true), "A#linebreak();; B", "ein ; würde sonst verschluckt");
        let pieces = vec![text("Ein "), Inline::Emph(vec![text("leises")]), text("(Wort)")];
        assert_eq!(render_inline(&pieces, true), "Ein _leises_(Wort)", "_…_ ist Markup, kein Ausdruck: kein Schutz");
    }

    fn section(title: &str, body: &str) -> Section {
        Section { level: 1, title: title.into(), part: false, body: body.into(), plain: body.into() }
    }

    #[test]
    fn the_confirmed_outline_decides_headings_and_no_text_is_lost() {
        let sections =
            vec![section("Eins", "Text eins."), section("Zwei", "Text zwei."), section("Drei", "Text drei.")];
        let confirmed = vec![
            Confirmed { level: 1, title: "Erstes".into(), part: false, source: Some(0) },
            Confirmed { level: 2, title: "Drittes".into(), part: false, source: Some(2) },
        ];
        let (typst, warnings) = build_typst("Buch", Some("Ina"), Some("de"), &sections, &confirmed).unwrap();
        assert!(
            typst.starts_with("#set document(title: \"Buch\", author: \"Ina\")\n#set text(lang: \"de\")\n"),
            "{typst}"
        );
        assert!(
            typst.contains("\n= Erstes\n\nText eins.\n\nText zwei.\n"),
            "gestrichener Punkt: Text beim vorigen: {typst}"
        );
        assert!(typst.contains("\n== Drittes\n\nText drei.\n"), "umbenannt und umgestuft: {typst}");
        assert_eq!(warnings.len(), 1, "eine Warnung für den gestrichenen Punkt: {warnings:?}");
        assert_eq!(warnings[0].code, "import-outline-merged");
        let doc = baruch_typst::parse(&typst).unwrap();
        assert_eq!(doc.headings.len(), 2, "genau die bestätigten Überschriften: {typst}");
        let back = vec![
            Confirmed { level: 1, title: "X".into(), part: false, source: Some(2) },
            Confirmed { level: 1, title: "Y".into(), part: false, source: Some(1) },
        ];
        assert!(
            build_typst("B", None, None, &sections, &back).is_err(),
            "Verschieben ändert die Reihenfolge: abgelehnt"
        );
        let positional = vec![Confirmed { level: 1, title: "A".into(), part: false, source: None }];
        assert!(build_typst("B", None, None, &sections, &positional).is_err(), "ohne source gleich viele Punkte nötig");
        let part = vec![
            Confirmed { level: 1, title: "Erster Teil".into(), part: true, source: None },
            Confirmed { level: 1, title: "A".into(), part: false, source: Some(0) },
            Confirmed { level: 1, title: "B".into(), part: false, source: Some(1) },
            Confirmed { level: 1, title: "C".into(), part: false, source: Some(2) },
        ];
        let (typst, _) = build_typst("B", None, None, &sections, &part).unwrap();
        let doc = baruch_typst::parse(&typst).unwrap();
        assert_eq!(doc.headings[0].level, 0, "ein bestätigter Teil wird die Teil-Markierung: {typst}");
    }
}
