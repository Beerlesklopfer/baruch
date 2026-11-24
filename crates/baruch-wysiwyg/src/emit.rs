// SPDX-License-Identifier: AGPL-3.0-or-later
//! Blöcke → kanonisches Typst. Nur für neue oder bearbeitete Blöcke; unberührte übernimmt
//! [`crate::serialize`] wörtlich. Für jede Art außer der Insel gilt `parse(emit(k)) == k` (ohne die
//! `raw` geschachtelter Blöcke), sofern `k` kanonisch ist: Text ohne Zeilenenden, keine zwei Texte
//! nebeneinander, kein Leerraum am Anfang einer Zeile bzw. am Ende eines Blocks, keine zwei
//! Zeilenumbrüche hintereinander, kein Zeilenumbruch in einer Überschrift (oberste Ebene oder in
//! `_…_`/`*…*`). Eine Überschrift mit einem solchen Umbruch (aus dem Satzbild) endet dort; der Rest
//! wird ein Absatz dahinter ([`split_heading`], Runde 7).
//!
//! Regeln: Sonderzeichen im Text werden maskiert (`\#`, `\*`, `\_` …), am Zeilenanfang auch
//! Listen-, Überschrifts- und Aufzählungszeichen; ein geschütztes Leerzeichen wird `~`. `_…_` und
//! `*…*` stehen nur an Wortgrenzen und nicht geschachtelt in sich selbst, sonst `#emph[…]` bzw.
//! `#strong[…]`. Hinter einem `#`-Ausdruck wird `(` und `.name` maskiert, hinter einem
//! Zeilenkommentar beginnt eine neue Zeile.

use crate::parse::MAX_LIST_DEPTH;
use crate::scan::is_label_char;
use crate::{Block, BlockKind, Inline, Term};

/// Kanonisches Typst eines Blocks, ohne Zeilenende. Eine Insel ergibt `""` (sie wird nie neu
/// erzeugt); in einem Rumpf (`#szene[…]`) steht eine Insel mit ihrem `raw`.
pub fn emit(kind: &BlockKind) -> String {
    match kind {
        BlockKind::Heading { level, inlines, label } => {
            // Runde 7 („Absatz wird Titel“): Die Überschrift endet am ersten Zeilenumbruch; was
            // dahinter steht, ist ein eigener Absatz und nie Teil der Überschriftszeile.
            let (head, rest) = split_heading(inlines);
            let mut line = heading_text(*level, &head);
            if let Some(label) = label {
                line.push_str(&label_line(label));
            }
            if let Some(rest) = rest {
                line.push_str("\n\n");
                line.push_str(&emit(&BlockKind::Paragraph { inlines: rest }));
            }
            line
        }
        BlockKind::Paragraph { inlines } => seq(inlines, &Ctx::default(), true),
        BlockKind::List { ordered, items, depths } => {
            let marker = if *ordered { "+" } else { "-" };
            let levels = list_depths(items.len(), depths);
            items
                .iter()
                .zip(&levels)
                .map(|(item, &d)| format!("{}{marker} {}", spaces(d), seq(item, &item_ctx(d), true)))
                .collect::<Vec<_>>()
                .join("\n")
        }
        BlockKind::Enum { start, items, depths } => {
            // Fortlaufende Nummern wie beim Lesen: oben ab `start`, jede geschachtelte Gruppe ab 1.
            let levels = list_depths(items.len(), depths);
            let mut counters: Vec<u64> = Vec::new();
            let mut lines = Vec::with_capacity(items.len());
            for (k, (item, &d)) in items.iter().zip(&levels).enumerate() {
                counters.truncate(d + 1);
                if counters.len() == d {
                    counters.push(if k == 0 { *start } else { 1 });
                }
                let number = counters[d];
                counters[d] = number.saturating_add(1);
                lines.push(format!("{}{number}. {}", spaces(d), seq(item, &item_ctx(d), true)));
            }
            lines.join("\n")
        }
        BlockKind::Terms { items } => items
            .iter()
            .map(|Term { term, desc }| {
                // Der Begriff ist eine Zeile; jeder Doppelpunkt darin wird maskiert (er beendete den Begriff).
                let term_ctx = Ctx { heading: true, term: true, after: Some(':'), ..Ctx::default() };
                let desc = seq(desc, &item_ctx(0), true);
                let sep = if desc.is_empty() { "" } else { " " };
                format!("/ {}:{sep}{desc}", seq(term, &term_ctx, true))
            })
            .collect::<Vec<_>>()
            .join("\n"),
        BlockKind::Quote { block, attribution, body } => {
            let mut args = Vec::new();
            if *block || attribution.is_none() {
                args.push(format!("block: {block}"));
            }
            if let Some(attribution) = attribution {
                args.push(format!(
                    "attribution: [{}]",
                    seq(attribution, &Ctx { after: Some(']'), ..Ctx::default() }, false)
                ));
            }
            format!("#quote({}){}", args.join(", "), content(body, false))
        }
        BlockKind::Scene { args, body } => format!("#szene{}{}", arguments(args.as_deref()), content(body, true)),
        BlockKind::Work { args, body } => format!("#arbeit{}{}", arguments(args.as_deref()), content(body, true)),
        BlockKind::Part { title } => baruch_typst::part_line(title).trim_end_matches('\n').to_owned(),
        BlockKind::Unnumbered { level, title } => {
            baruch_typst::unnumbered_line(*level, title).trim_end_matches('\n').to_owned()
        }
        BlockKind::Island => String::new(),
    }
}

/// Leerzeichen für den Einzug geschachtelter Listenpunkte (zwei je Ebene).
const SPACES: &str = "                                        ";

/// Einzug der Ebene `depth` (zwei Leerzeichen je Ebene, höchstens [`MAX_LIST_DEPTH`] + 1 Ebenen).
fn spaces(depth: usize) -> &'static str {
    &SPACES[..(2 * depth.min(usize::from(MAX_LIST_DEPTH) + 1)).min(SPACES.len())]
}

/// Kontext für den Text eines Listenpunkts der Ebene `depth`: Folgezeilen eine Ebene tiefer eingerückt als
/// sein Zeichen.
fn item_ctx(depth: usize) -> Ctx {
    Ctx { indent: spaces(depth + 1), ..Ctx::default() }
}

/// Tiefe je Punkt, gültig gemacht: der erste Punkt oben, jeder weitere höchstens eine Ebene tiefer als der
/// davor, höchstens [`MAX_LIST_DEPTH`]; fehlende Angaben = 0.
fn list_depths(count: usize, depths: &[u8]) -> Vec<usize> {
    let mut out: Vec<usize> = Vec::with_capacity(count);
    for k in 0..count {
        let wanted = usize::from(depths.get(k).copied().unwrap_or(0).min(MAX_LIST_DEPTH));
        let limit = out.last().map_or(0, |&d| d + 1);
        out.push(wanted.min(limit));
    }
    out
}

/// Überschriftszeile ohne Label und ohne Zeilenende: `=` × Ebene, Leerzeichen, Titel.
pub(crate) fn heading_text(level: u8, inlines: &[Inline]) -> String {
    let ctx = Ctx { heading: true, ..Ctx::default() };
    format!("{} {}", "=".repeat(usize::from(level.clamp(1, 6))), seq(inlines, &ctx, false))
}

/// Inlines einer Überschrift am ersten Zeilenumbruch teilen ([`Inline::Soft`] oder ein Zeilenende
/// im Sinn von Typst, [`baruch_typst::is_newline`], in einem Text), auf oberster Ebene wie in
/// `_…_` bzw. `*…*`: davor der Titel, dahinter (falls etwas übrig bleibt) der Absatz, der unter der
/// Überschrift steht. Ein Umbruch in einer Hervorhebung teilt diese (`*Laub⏎Absatz*` → Titel
/// `*Laub*`, Absatz `*Absatz*`), denn in der Überschriftszeile würde er sonst ein Leerzeichen und
/// zöge den Absatz doch in den Titel. Fußnoten und Zitate (`#footnote[…]`, `#quote[…]`) bleiben
/// ganz; ihr Inhalt ist ein eigener Inhaltsblock. Umbrüche am Anfang zählen nicht, Leerraum an der
/// Schnittstelle entfällt. So kann ein weicher Umbruch (Umschalt+Enter, `<br>` des Browsers, auch in
/// `<b>`/`<i>`) im Satzbild nie Absatztext in die Überschriftszeile ziehen; Typst kennt ohnehin nur
/// einzeilige Überschriften.
pub(crate) fn split_heading(inlines: &[Inline]) -> (Vec<Inline>, Option<Vec<Inline>>) {
    let flat = trim_front(flatten_newlines(inlines));
    let (head, rest) = cut_at_break(flat);
    let Some(rest) = rest else {
        return (head, None);
    };
    let head = trim_back(head);
    let rest = trim_back(trim_front(rest));
    (head, (!rest.is_empty()).then_some(rest))
}

/// Zeilenenden in Texten (auch in `_…_`/`*…*`) wie einfache Umbrüche behandeln.
fn flatten_newlines(inlines: &[Inline]) -> Vec<Inline> {
    let mut flat: Vec<Inline> = Vec::with_capacity(inlines.len());
    for inline in inlines {
        match inline {
            Inline::Text { v } if v.chars().any(baruch_typst::is_newline) => {
                for (k, piece) in v.split(baruch_typst::is_newline).enumerate() {
                    if k > 0 {
                        flat.push(Inline::Soft);
                    }
                    if !piece.is_empty() {
                        flat.push(Inline::Text { v: piece.to_owned() });
                    }
                }
            }
            Inline::Emph { c } => flat.push(Inline::Emph { c: flatten_newlines(c) }),
            Inline::Strong { c } => flat.push(Inline::Strong { c: flatten_newlines(c) }),
            other => flat.push(other.clone()),
        }
    }
    flat
}

/// Baut aus einem Inhalt wieder dieselbe Hervorhebung (`Emph` bzw. `Strong`).
type Rewrap = fn(Vec<Inline>) -> Inline;

/// Am ersten Umbruch teilen, der in der Überschriftszeile stünde (oberste Ebene oder in `_…_`/`*…*`,
/// beliebig tief). Eine geteilte Hervorhebung steht mit ihrem vorderen Teil im Titel und mit dem
/// hinteren am Anfang des Rests; leere Teile entfallen.
fn cut_at_break(inlines: Vec<Inline>) -> (Vec<Inline>, Option<Vec<Inline>>) {
    let mut head = Vec::with_capacity(inlines.len());
    let mut iter = inlines.into_iter();
    while let Some(inline) = iter.next() {
        let (c, wrap): (Vec<Inline>, Rewrap) = match inline {
            Inline::Soft => return (head, Some(iter.collect())),
            Inline::Emph { c } => (c, |c| Inline::Emph { c }),
            Inline::Strong { c } => (c, |c| Inline::Strong { c }),
            other => {
                head.push(other);
                continue;
            }
        };
        let (front, back) = cut_at_break(c);
        let Some(back) = back else {
            head.push(wrap(front));
            continue;
        };
        if !front.is_empty() {
            head.push(wrap(front));
        }
        let mut rest = Vec::new();
        if !back.is_empty() {
            rest.push(wrap(back));
        }
        rest.extend(iter);
        return (head, Some(rest));
    }
    (head, None)
}

/// Umbrüche, leere Texte, leere Hervorhebungen und Leerraum am Anfang einer Folge entfernen, auch
/// am Anfang einer vorn stehenden Hervorhebung.
fn trim_front(mut inlines: Vec<Inline>) -> Vec<Inline> {
    loop {
        match inlines.first_mut() {
            Some(Inline::Soft) => {
                inlines.remove(0);
            }
            Some(Inline::Text { v }) => {
                let trimmed = v.trim_start_matches([' ', '\t']);
                if trimmed.is_empty() {
                    inlines.remove(0);
                } else {
                    *v = trimmed.to_owned();
                    return inlines;
                }
            }
            Some(Inline::Emph { c } | Inline::Strong { c }) => {
                *c = trim_front(std::mem::take(c));
                if c.is_empty() {
                    inlines.remove(0);
                } else {
                    return inlines;
                }
            }
            _ => return inlines,
        }
    }
}

/// Wie [`trim_front`], aber am Ende der Folge.
fn trim_back(mut inlines: Vec<Inline>) -> Vec<Inline> {
    loop {
        match inlines.last_mut() {
            Some(Inline::Soft) => {
                inlines.pop();
            }
            Some(Inline::Text { v }) => {
                let trimmed = v.trim_end_matches([' ', '\t']).len();
                if trimmed == 0 {
                    inlines.pop();
                } else {
                    v.truncate(trimmed);
                    return inlines;
                }
            }
            Some(Inline::Emph { c } | Inline::Strong { c }) => {
                *c = trim_back(std::mem::take(c));
                if c.is_empty() {
                    inlines.pop();
                } else {
                    return inlines;
                }
            }
            _ => return inlines,
        }
    }
}

/// Zeile mit dem Label einer Überschrift (`\n<name>`); ein Name mit fremden Zeichen ergäbe kein
/// Label und entfällt.
pub(crate) fn label_line(label: &str) -> String {
    if !label.is_empty() && label.chars().all(crate::scan::is_label_char) {
        format!("\n<{label}>")
    } else {
        String::new()
    }
}

/// Argumente vor dem Rumpf (`(ort: [Küche])`) wörtlich; ohne Argumente nichts.
fn arguments(args: Option<&str>) -> String {
    args.map(|a| format!("({a})")).unwrap_or_default()
}

/// Rumpf eines Blocks als Inhaltsblock: ein einzelner einzeiliger Absatz als `[…]`, sonst
/// mehrzeilig mit Leerzeilen zwischen den Blöcken.
fn content(body: &[Block], multiline: bool) -> String {
    let parts: Vec<String> = body
        .iter()
        .map(|block| match block.kind {
            BlockKind::Island => block.raw.trim().to_owned(),
            ref kind => emit(kind),
        })
        .filter(|part| !part.is_empty())
        .collect();
    if parts.is_empty() {
        return "[]".into();
    }
    let single = !multiline
        && parts.len() == 1
        && matches!(body[0].kind, BlockKind::Paragraph { .. })
        && !parts[0].contains('\n')
        && !parts[0].contains("//")
        && !parts[0].ends_with('\\');
    if single { format!("[{}]", parts[0]) } else { format!("[\n{}\n]", parts.join("\n\n")) }
}

#[derive(Clone, Default)]
struct Ctx {
    /// Einzug hinter einem Zeilenumbruch (Listenpunkt: zwei Leerzeichen).
    indent: &'static str,
    /// Oberste Ebene einer Überschrift: Zeilenumbrüche werden Leerzeichen.
    heading: bool,
    /// Innerhalb von `_…_` bzw. `*…*`: dasselbe Zeichen darf nicht noch einmal öffnen.
    in_emph: bool,
    in_strong: bool,
    /// Zeichen, das hinter der ganzen Folge steht (`_`, `*` bzw. `]`); `None` am Blockende.
    after: Option<char>,
    /// Begriff einer Begriffsliste (Runde 18b): `:` wird maskiert, auch in Fußnoten und Links darin.
    term: bool,
}

impl Ctx {
    /// Neuer Inhaltsblock `[…]` (Fußnote, Zitat, Link, `#emph[…]`): Einzug und Begriff bleiben.
    fn fresh(&self) -> Ctx {
        Ctx { indent: self.indent, after: Some(']'), term: self.term, ..Ctx::default() }
    }
}

/// Erstes Zeichen, mit dem ein Inline beginnt (für die Wortgrenze davor).
fn first_char(inline: &Inline) -> Option<char> {
    match inline {
        Inline::Text { v } => v.chars().next().map(|c| if needs_escape(c) { '\\' } else { c }),
        Inline::Soft => Some('\n'),
        Inline::Island { raw } => raw.chars().next(),
        Inline::Label { .. } => Some('<'),
        Inline::Emph { .. } => Some('_'),
        Inline::Strong { .. } => Some('*'),
        Inline::Footnote { .. }
        | Inline::Quote { .. }
        | Inline::Link { .. }
        | Inline::Xref { .. }
        | Inline::IndexMark { .. } => Some('#'),
        Inline::Linebreak => Some('\\'),
        Inline::Ref { .. } => Some('@'),
    }
}

/// String-Literal für Typst (`"…"`, Backslash, Anführungszeichen und Zeilenenden maskiert).
fn string_literal(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for c in value.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Kann ein Verweis als `@ziel` stehen? Das Ziel muss wie beim Lesen beginnen (Buchstabe, Ziffer, `_`), darf nicht
/// mit `.` oder `:` enden, und das Zeichen dahinter darf nicht zum Ziel gehören; sonst `#ref(<ziel>)`.
fn plain_ref(target: &str, following: Option<char>) -> bool {
    target.chars().next().is_some_and(|c| c.is_alphanumeric() || c == '_')
        && !target.ends_with(['.', ':'])
        && !following.is_some_and(is_label_char)
}

/// Zeichen, die im Markup immer maskiert werden.
fn needs_escape(c: char) -> bool {
    matches!(c, '\\' | '#' | '*' | '_' | '$' | '@' | '<' | '>' | '[' | ']' | '`' | '~')
}

fn wordy(c: Option<char>) -> bool {
    c.is_some_and(char::is_alphanumeric)
}

/// Folge von Inlines; `line_start`: steht die erste am Zeilenanfang (Absatz, Listenpunkt)?
fn seq(inlines: &[Inline], ctx: &Ctx, line_start: bool) -> String {
    let mut out = String::new();
    let mut at_line_start = line_start;
    let mut after_hash = false;
    for (k, inline) in inlines.iter().enumerate() {
        let next = inlines.get(k + 1);
        // Erstes Zeichen hinter diesem Inline: das nächste oder das, was die Folge schließt.
        let following = next.and_then(first_char).or(ctx.after);
        let was_hash = std::mem::take(&mut after_hash);
        let starting = std::mem::take(&mut at_line_start);
        match inline {
            Inline::Text { v } => text(&mut out, v, starting, was_hash, following, ctx.term),
            Inline::Soft if ctx.heading => out.push(' '),
            Inline::Soft => {
                out.push('\n');
                out.push_str(ctx.indent);
                at_line_start = true;
            }
            Inline::Emph { c } | Inline::Strong { c } => {
                let emph = matches!(inline, Inline::Emph { .. });
                let (delimiter, function, nested) =
                    if emph { ('_', "emph", ctx.in_emph) } else { ('*', "strong", ctx.in_strong) };
                let inner_ctx = Ctx {
                    in_emph: ctx.in_emph || emph,
                    in_strong: ctx.in_strong || !emph,
                    after: Some(delimiter),
                    ..ctx.clone()
                };
                let inner = seq(c, &inner_ctx, false);
                let prev = out.chars().next_back();
                let in_word = (wordy(prev) && wordy(inner.chars().next()))
                    || (wordy(inner.chars().next_back()) && wordy(following));
                // `*/` bzw. `/*` an den Rändern von `*…*` wären Kommentarzeichen.
                let comment = !emph && (inner.starts_with('/') || inner.ends_with('/'));
                if nested || in_word || comment || inner.is_empty() {
                    out.push_str(&format!("#{function}[{}]", seq(c, &ctx.fresh(), false)));
                    after_hash = true;
                } else {
                    out.push(delimiter);
                    out.push_str(&inner);
                    out.push(delimiter);
                }
            }
            Inline::Footnote { c } | Inline::Quote { c } => {
                let function = if matches!(inline, Inline::Footnote { .. }) { "footnote" } else { "quote" };
                out.push_str(&format!("#{function}[{}]", seq(c, &ctx.fresh(), false)));
                after_hash = true;
            }
            Inline::Linebreak => {
                // Ein Leerzeichen trennt den Umbruch vom Folgenden (`\]`, `\_` wären Maskierungen); am
                // Blockende und vor einem Zeilenende steht er allein.
                out.push('\\');
                if !matches!(next, Some(Inline::Soft)) && (next.is_some() || ctx.after.is_some()) {
                    out.push(' ');
                }
            }
            Inline::Link { url, c } => {
                out.push_str("#link(");
                out.push_str(&string_literal(url));
                out.push(')');
                if let Some(c) = c {
                    out.push_str(&format!("[{}]", seq(c, &ctx.fresh(), false)));
                }
                after_hash = true;
            }
            Inline::Xref { key, c, page } => {
                // Runde 30a: Form aus `baruch_typst::xref::xref_snippet` (mit Seitenzahl-Zusatz), `parse` liest sie zurück.
                out.push_str(&baruch_typst::xref::xref_snippet(*key, &seq(c, &ctx.fresh(), false), *page));
                after_hash = true;
            }
            Inline::Ref { target, supplement } => {
                if target.is_empty() || !target.chars().all(is_label_char) {
                    // Kein gültiges Ziel: als Text, wie getippt.
                    text(&mut out, &format!("@{target}"), starting, was_hash, following, ctx.term);
                } else if plain_ref(target, following) {
                    out.push('@');
                    out.push_str(target);
                    if let Some(supplement) = supplement {
                        out.push_str(&format!("[{supplement}]"));
                    }
                } else {
                    out.push_str(&format!("#ref(<{target}>"));
                    if let Some(supplement) = supplement {
                        out.push_str(&format!(", supplement: [{supplement}]"));
                    }
                    out.push(')');
                    after_hash = true;
                }
            }
            Inline::Label { v } => {
                if !v.is_empty() && v.chars().all(crate::scan::is_label_char) {
                    out.push('<');
                    out.push_str(v);
                    out.push('>');
                } else {
                    text(&mut out, &format!("<{v}>"), starting, was_hash, following, ctx.term);
                }
            }
            Inline::IndexMark { mark } => {
                // R31: kanonische Form; endet mit dem Label, dahinter darf Text ohne Maskierung stehen.
                out.push_str(&baruch_typst::index::mark_source(mark));
            }
            Inline::Island { raw } => {
                out.push_str(raw);
                after_hash = raw.starts_with('#');
                // Hinter einem Zeilenkommentar ginge sonst der Rest der Zeile im Kommentar unter.
                if raw.starts_with("//") && !matches!(next, None | Some(Inline::Soft)) {
                    out.push('\n');
                    out.push_str(ctx.indent);
                    at_line_start = true;
                }
            }
        }
    }
    out
}

/// Text maskiert anhängen; `term`: im Begriff einer Begriffsliste auch `:` maskieren.
fn text(out: &mut String, v: &str, line_start: bool, after_hash: bool, after: Option<char>, term: bool) {
    // Alle Zeilenenden im Sinn von Typst, auch U+2028 & Co.: dahinter stünde Text am Zeilenanfang.
    let flat: String = v.chars().map(|c| if baruch_typst::is_newline(c) { ' ' } else { c }).collect();
    let v = if line_start { flat.trim_start_matches([' ', '\t']) } else { flat.as_str() };
    let chars: Vec<char> = v.chars().collect();
    // Aufzählung am Zeilenanfang: „1. …“ → „1\. …“.
    let digits = if line_start { chars.iter().take_while(|c| c.is_ascii_digit()).count() } else { 0 };
    for (k, &c) in chars.iter().enumerate() {
        let following = chars.get(k + 1).copied().or(after);
        let escape = needs_escape(c)
            || (term && c == ':')
            // Am Zeilenanfang Überschrift, Liste, Aufzählung, Begriff verhindern. `--`, `-?` und `-1`
            // sind keine Listenpunkte; maskiert verlören sie ihre Kurzschreibweise (wie `1\..`).
            || (k == 0
                && line_start
                && (matches!(c, '=' | '+' | '/')
                    || (c == '-' && !following.is_some_and(|n| n == '-' || n == '?' || n.is_ascii_digit()))))
            || (digits > 0 && k == digits && c == '.' && following != Some('.'))
            || (k == 0 && after_hash && (c == '(' || (c == '.' && following.is_some_and(crate::scan::is_ident_start))))
            // `//` und `/*` begännen einen Kommentar (auch mit dem Zeichen hinter dem Text).
            || (c == '/'
                && matches!(following, Some('/' | '*'))
                && !(out.ends_with("http:") || out.ends_with("https:")));
        match c {
            '\u{a0}' => out.push('~'),
            _ => {
                if escape {
                    out.push('\\');
                }
                out.push(c);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(v: &str) -> Inline {
        Inline::Text { v: v.into() }
    }

    #[test]
    fn special_characters_are_escaped() {
        let kind = BlockKind::Paragraph { inlines: vec![text("- #1 kostet $5 *heute* @x <y> [z] a_b ~ \\")] };
        assert_eq!(emit(&kind), r"\- \#1 kostet \$5 \*heute\* \@x \<y\> \[z\] a\_b \~ \\", "alles maskiert");
    }

    #[test]
    fn emphasis_inside_a_word_uses_the_function() {
        let kind =
            BlockKind::Paragraph { inlines: vec![text("Haus"), Inline::Emph { c: vec![text("tür")] }, text(" zu")] };
        assert_eq!(emit(&kind), "Haus#emph[tür] zu", "_ zwischen Wortzeichen wäre Text");
    }

    #[test]
    fn a_line_comment_island_is_followed_by_a_new_line() {
        let kind = BlockKind::Paragraph {
            inlines: vec![text("Satz "), Inline::Island { raw: "// Notiz".into() }, text("weiter")],
        };
        assert_eq!(emit(&kind), "Satz // Notiz\nweiter", "der Text danach landet nicht im Kommentar");
    }
}
