// SPDX-License-Identifier: AGPL-3.0-or-later
//! Satzbild nach der Werkvorlage: Schrift, Größe, Farben, Einzug, Zeilenabstand, Satzbreite und die
//! Kästen von `#szene`/`#arbeit`, gelesen aus `#set text/par/page(…)`, `#show emph/strong/heading:
//! set text(…)` und `#let szene/arbeit(…) = block(…)`. Kommentare und Raw-Text zählen nicht, ebenso
//! Regeln, die nicht auf der obersten Ebene stehen (in `#show …: it => […]`, `#show: …`, in
//! Funktionsrümpfen `#let f() = {…}` oder anderen Ausdrücken): Die gelten nur für einen Teil des
//! Dokuments. Gilt eine Regel mehrmals, gewinnt die letzte (wie in Typst). Werte, die sich nicht sicher
//! nach CSS übersetzen lassen, fehlen; dann gelten die Vorgaben aus `wysiwyg.css`.

use serde::{Deserialize, Serialize};

use crate::parse::split_args;
use crate::scan::{
    Scanner, after_scheme, closed_group, ident_end, is_ident_continue, next_char, skip_escape, skip_line_comment,
};

/// Kasten eines Blocks (`#szene`, `#arbeit`), als CSS-Werte.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoxStyle {
    /// Hintergrund (`fill`).
    pub fill: Option<String>,
    /// Innenabstand (`inset`).
    pub inset: Option<String>,
    /// Rahmen (`stroke`), z. B. `0.5pt solid #aaaaaa`.
    pub stroke: Option<String>,
}

/// Darstellung des Satzbilds, als CSS-Werte; `None` = Vorgabe.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SheetStyle {
    /// Schriftfamilien, CSS-Liste (`"Erfundene Antiqua", serif`).
    pub font: Option<String>,
    pub size: Option<String>,
    /// Textfarbe.
    pub fill: Option<String>,
    pub emph_fill: Option<String>,
    pub strong_fill: Option<String>,
    /// Aus `#show heading: set text(…)`.
    pub heading_font: Option<String>,
    pub heading_fill: Option<String>,
    /// Erstzeileneinzug (`first-line-indent`).
    pub indent: Option<String>,
    /// Zeilenabstand (`leading`, Abstand zwischen den Zeilen wie in Typst).
    pub leading: Option<String>,
    pub justify: Option<bool>,
    /// Seitenbreite (`width` bzw. `paper`).
    pub width: Option<String>,
    /// Blattfarbe. Die Werkvorlage liefert sie nie (das Blatt bleibt hell wie die Vorschau); nur die
    /// Satzbild-Einstellungen des Werks setzen sie (Runde 18c, `--ws-background`, `static/sheet.css`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background: Option<String>,
    pub scene: Option<BoxStyle>,
    pub work: Option<BoxStyle>,
}

/// Darstellung `base` (aus der Vorlage) mit `over` überschrieben: jedes gesetzte Feld von `over` gewinnt.
pub fn sheet_style_over(base: SheetStyle, over: &SheetStyle) -> SheetStyle {
    fn pick<T: Clone>(base: Option<T>, over: &Option<T>) -> Option<T> {
        over.clone().or(base)
    }
    SheetStyle {
        font: pick(base.font, &over.font),
        size: pick(base.size, &over.size),
        fill: pick(base.fill, &over.fill),
        emph_fill: pick(base.emph_fill, &over.emph_fill),
        strong_fill: pick(base.strong_fill, &over.strong_fill),
        heading_font: pick(base.heading_font, &over.heading_font),
        heading_fill: pick(base.heading_fill, &over.heading_fill),
        indent: pick(base.indent, &over.indent),
        leading: pick(base.leading, &over.leading),
        justify: pick(base.justify, &over.justify),
        width: pick(base.width, &over.width),
        background: pick(base.background, &over.background),
        scene: pick(base.scene, &over.scene),
        work: pick(base.work, &over.work),
    }
}

/// Darstellung aus der Werkvorlage ableiten.
pub fn sheet_style(template: &str) -> SheetStyle {
    let text = blank(template);
    let top = top_level(&text);
    let text = Rules { text: &text, top: &top };
    let mut style = SheetStyle::default();
    for args in set_rules(&text, "text") {
        for (name, value) in &args {
            match name.as_deref() {
                Some("font") => set(&mut style.font, font(value)),
                Some("size") => set(&mut style.size, length(value)),
                Some("fill") => set(&mut style.fill, color(value)),
                None => {
                    set(&mut style.size, length(value));
                    set(&mut style.fill, color(value));
                }
                _ => {}
            }
        }
    }
    for args in set_rules(&text, "par") {
        for (name, value) in &args {
            match name.as_deref() {
                Some("first-line-indent") => set(&mut style.indent, indent(value)),
                Some("leading") => set(&mut style.leading, length(value)),
                Some("justify") => set(&mut style.justify, boolean(value)),
                _ => {}
            }
        }
    }
    for args in set_rules(&text, "page") {
        for (name, value) in &args {
            match name.as_deref() {
                Some("width") => set(&mut style.width, length(value)),
                Some("paper") => set(&mut style.width, paper(value)),
                _ => {}
            }
        }
    }
    for (selector, fill_slot) in
        [("emph", &mut style.emph_fill), ("strong", &mut style.strong_fill), ("heading", &mut style.heading_fill)]
    {
        for args in show_set_text(&text, selector) {
            for (name, value) in &args {
                if name.as_deref() == Some("fill") {
                    set(fill_slot, color(value));
                }
            }
        }
    }
    for args in show_set_text(&text, "heading") {
        for (name, value) in &args {
            if name.as_deref() == Some("font") {
                set(&mut style.heading_font, font(value));
            }
        }
    }
    style.scene = box_style(&text, "szene");
    style.work = box_style(&text, "arbeit");
    style
}

/// Grundschrift der Werkvorlage für den Umschlag (Runde 27): die erste Schrift (ein String oder das erste Element
/// eines Arrays) aus dem **letzten** `#set text(font: …)` auf oberster Ebene – dieselbe Regel wie [`sheet_style`]
/// und wie Typst selbst (die spätere Regel gewinnt). Kommentare, Raw-Text, `#show …: set text(…)`, Rümpfe von
/// `#show: …` und Funktionsrümpfe zählen nicht. Eine Schrift, die sich nicht lesen lässt (Variable, Maskierung im
/// String), wird übergangen; dann gilt die vorige Regel.
pub fn template_font(template: &str) -> Option<String> {
    let text = blank(template);
    let top = top_level(&text);
    let rules = Rules { text: &text, top: &top };
    let mut found = None;
    for args in set_rules(&rules, "text") {
        for (name, value) in &args {
            if name.as_deref() == Some("font") {
                set(&mut found, first_font(value));
            }
        }
    }
    found
}

/// Erster Schriftname aus `"Name"` oder `("A", "B")` (ohne Anführungszeichen).
fn first_font(value: &str) -> Option<String> {
    let v = value.trim();
    let first = if v.starts_with('(') {
        let (name, value) = args_at(v, 0)?.into_iter().next()?;
        if name.is_some() {
            return None;
        }
        string(&value)?
    } else {
        string(v)?
    };
    let first = first.trim();
    (!first.is_empty()).then(|| first.to_owned())
}

/// Neuen Wert übernehmen, wenn er sich übersetzen ließ (spätere Regeln gewinnen).
fn set<T>(slot: &mut Option<T>, value: Option<T>) {
    if value.is_some() {
        *slot = value;
    }
}

/// CSS-Variablen unter `.ws-sheet` (nur gesetzte Werte; der Rest kommt aus `wysiwyg.css`).
pub fn sheet_css(style: &SheetStyle) -> String {
    let mut vars: Vec<(&str, String)> = Vec::new();
    let mut put = |name: &'static str, value: &Option<String>| {
        if let Some(value) = value.as_deref().filter(|v| safe(v)) {
            vars.push((name, value.to_owned()));
        }
    };
    put("--ws-font", &style.font);
    put("--ws-size", &style.size);
    put("--ws-fill", &style.fill);
    put("--ws-emph-fill", &style.emph_fill);
    put("--ws-strong-fill", &style.strong_fill);
    put("--ws-heading-font", &style.heading_font);
    put("--ws-heading-fill", &style.heading_fill);
    put("--ws-indent", &style.indent);
    put("--ws-leading", &style.leading);
    put("--ws-width", &style.width);
    put("--ws-background", &style.background);
    put("--ws-justify", &style.justify.map(|j| if j { "justify".into() } else { "start".into() }));
    for (prefix, boxed) in [("scene", &style.scene), ("work", &style.work)] {
        if let Some(boxed) = boxed {
            let name = |part: &str| -> &'static str {
                match (prefix, part) {
                    ("scene", "fill") => "--ws-scene-fill",
                    ("scene", "inset") => "--ws-scene-inset",
                    ("scene", _) => "--ws-scene-stroke",
                    (_, "fill") => "--ws-work-fill",
                    (_, "inset") => "--ws-work-inset",
                    _ => "--ws-work-stroke",
                }
            };
            put(name("fill"), &boxed.fill);
            put(name("inset"), &boxed.inset);
            put(name("stroke"), &boxed.stroke);
        }
    }
    let body: Vec<String> = vars.into_iter().map(|(name, value)| format!("{name}: {value};")).collect();
    format!(".ws-sheet {{ {} }}", body.join(" "))
}

/// Darf der Wert in CSS stehen? Nur Buchstaben, Ziffern und wenige Zeichen; nichts, was die Regel
/// verlassen könnte (`;`, `{`, `}`, `<`, `\`), und Anführungszeichen nur paarig (eine offene
/// CSS-Zeichenkette reichte bis zum Zeilenende und nähme die übrigen Variablen mit).
fn safe(value: &str) -> bool {
    !value.is_empty()
        && value.chars().all(|c| c.is_alphanumeric() || " #%.,()-+_\"'".contains(c))
        && value.matches('"').count().is_multiple_of(2)
        && value.matches('\'').count().is_multiple_of(2)
}

/// Kommentare und Raw-Text durch Leerzeichen ersetzen (Positionen bleiben).
fn blank(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    let mut scanner = Scanner::new(source);
    let mut i = 0;
    while i < source.len() {
        let end = match source.as_bytes()[i] {
            b'"' => {
                let end = scanner.string(i);
                out.push_str(&source[i..end]);
                i = end;
                continue;
            }
            b'`' => scanner.raw(i),
            b'/' if source[i..].starts_with("//") && !after_scheme(source, i) => skip_line_comment(source, i),
            b'/' if source[i..].starts_with("/*") => scanner.block_comment(i),
            _ => {
                let next = crate::scan::next_char(source, i);
                out.push_str(&source[i..next]);
                i = next;
                continue;
            }
        };
        out.extend(source[i..end].chars().map(|c| if c == '\n' { '\n' } else { ' ' }));
        i = end;
    }
    out
}

type Args = Vec<(Option<String>, String)>;

/// Argumente einer Klammer ab `open` (`text[open] == '('`).
fn args_at(text: &str, open: usize) -> Option<Args> {
    let end = closed_group(text, open)?;
    let inner = &text[open + 1..end - 1];
    Some(
        split_args(inner)
            .into_iter()
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .map(|part| match named(part) {
                Some((name, value)) => (Some(name.to_owned()), value.trim().to_owned()),
                None => (None, part.to_owned()),
            })
            .collect(),
    )
}

/// `name: wert` (Name aus Buchstaben, Ziffern, `-`, `_`).
fn named(part: &str) -> Option<(&str, &str)> {
    let (name, value) = part.split_once(':')?;
    let name = name.trim();
    (!name.is_empty() && name.chars().all(is_ident_continue)).then_some((name, value))
}

/// Steht an `at` ein eigenständiges Wort (davor kein Bezeichnerzeichen)?
fn word_start(text: &str, at: usize) -> bool {
    text[..at].chars().next_back().is_none_or(|c| !is_ident_continue(c) && c != '.')
}

/// Argumente aller `#set <name>(…)` in Reihenfolge.
fn set_rules(rules: &Rules, name: &str) -> Vec<Args> {
    let needle = format!("#set {name}(");
    rules.find(&needle).filter_map(|at| args_at(rules.text, at + needle.len() - 1)).collect()
}

/// Blankierter Vorlagentext mit den Stellen der `#`-Ausdrücke auf oberster Ebene.
struct Rules<'a> {
    text: &'a str,
    top: &'a [usize],
}

impl Rules<'_> {
    /// Stellen von `needle`, die einen Ausdruck der obersten Ebene beginnen.
    fn find<'n>(&self, needle: &'n str) -> impl Iterator<Item = usize> + use<'_, 'n> {
        self.text.match_indices(needle).map(|(at, _)| at).filter(|at| self.top.binary_search(at).is_ok())
    }
}

/// Anfänge der `#`-Ausdrücke im Markup der obersten Ebene, aufsteigend. Was in einem Ausdruck steht
/// (Argumente, Inhaltsblöcke, Funktionsrümpfe, Show-Regeln), zählt nicht. Hinter `#set name(…)` geht
/// es nach der Klammer weiter (mehrere Regeln in einer Zeile).
fn top_level(text: &str) -> Vec<usize> {
    let mut scanner = Scanner::new(text);
    let mut starts = Vec::new();
    let mut j = 0;
    while j < text.len() {
        match text.as_bytes()[j] {
            b'\\' => j = skip_escape(text, j),
            b'`' => j = scanner.raw(j),
            b'$' => j = scanner.math(j),
            b'#' => {
                starts.push(j);
                let end = set_rule_end(text, j).unwrap_or_else(|| scanner.hash(j, 0));
                j = end.max(j + 1);
            }
            _ => j = next_char(text, j),
        }
    }
    starts
}

/// Ende von `#set name(…)` ab `at` (hinter der Klammer), sonst `None`.
fn set_rule_end(text: &str, at: usize) -> Option<usize> {
    let keyword_end = ident_end(text, at + 1);
    if &text[at + 1..keyword_end] != "set" {
        return None;
    }
    let rest = &text[keyword_end..];
    let name = keyword_end + (rest.len() - rest.trim_start_matches([' ', '\t']).len());
    let name_end = ident_end(text, name);
    (name_end > name && text[name_end..].starts_with('(')).then(|| closed_group(text, name_end)).flatten()
}

/// Argumente aller `#show <selector>: set text(…)`.
fn show_set_text(rules: &Rules, selector: &str) -> Vec<Args> {
    let text = rules.text;
    let mut found = Vec::new();
    for at in rules.find("#show") {
        let rest = text[at + 5..].trim_start();
        let Some(rest) = rest.strip_prefix(selector) else { continue };
        let Some(rest) = rest.trim_start().strip_prefix(':') else { continue };
        let Some(rest) = rest.trim_start().strip_prefix("set") else { continue };
        let Some(rest) = rest.trim_start().strip_prefix("text") else { continue };
        let rest = rest.trim_start();
        if rest.starts_with('(') {
            let open = text.len() - rest.len();
            found.extend(args_at(text, open));
        }
    }
    found
}

/// Kasten aus `#let <name>(…) = block|rect|box(…)`; die letzte Definition gilt.
fn box_style(rules: &Rules, name: &str) -> Option<BoxStyle> {
    let text = rules.text;
    let needle = format!("#let {name}(");
    let mut result = None;
    for at in rules.find(&needle) {
        let mut scanner = Scanner::new(text);
        let end = scanner.hash(at, 0);
        let definition = &text[at..end];
        let Some(eq) = definition.find('=') else { continue };
        let body = &definition[eq + 1..];
        let call = ["block(", "rect(", "box("]
            .iter()
            .filter_map(|f| body.match_indices(f).find(|(k, _)| word_start(body, *k)).map(|(k, _)| k + f.len() - 1))
            .min();
        let Some(open) = call else { continue };
        let Some(args) = args_at(body, open) else { continue };
        let mut style = BoxStyle::default();
        for (arg, value) in &args {
            match arg.as_deref() {
                Some("fill") => style.fill = color(value),
                Some("inset") => style.inset = length(value),
                Some("stroke") => style.stroke = stroke(value),
                _ => {}
            }
        }
        result = Some(style);
    }
    result
}

/// Länge in pt, mm, cm, in oder em (auch mit Nachkommastellen).
fn length(value: &str) -> Option<String> {
    let v = value.trim();
    let unit = ["pt", "mm", "cm", "in", "em"].into_iter().find(|u| v.ends_with(u))?;
    let number = &v[..v.len() - unit.len()];
    let valid = !number.is_empty()
        && number.parse::<f64>().is_ok_and(|n| n.is_finite() && n >= 0.0)
        && number.chars().all(|c| c.is_ascii_digit() || c == '.');
    valid.then(|| format!("{number}{unit}"))
}

/// `first-line-indent`: Länge oder `(amount: …, all: …)`.
fn indent(value: &str) -> Option<String> {
    let v = value.trim();
    if v.starts_with('(') {
        let args = args_at(v, 0)?;
        return args.iter().find(|(n, _)| n.as_deref() == Some("amount")).and_then(|(_, a)| length(a));
    }
    length(v)
}

fn boolean(value: &str) -> Option<bool> {
    match value.trim() {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

/// Satzbreite nach dem Papierformat.
fn paper(value: &str) -> Option<String> {
    let name = string(value)?;
    let width = match name.as_str() {
        "a4" => "21cm",
        "a5" => "14.8cm",
        "a6" => "10.5cm",
        "b5" => "17.6cm",
        "us-letter" => "8.5in",
        _ => return None,
    };
    Some(width.into())
}

/// Inhalt eines String-Literals `"…"` (ohne Maskierungen).
fn string(value: &str) -> Option<String> {
    let inner = value.trim().strip_prefix('"')?.strip_suffix('"')?;
    (!inner.contains(['"', '\\'])).then(|| inner.to_owned())
}

/// Schrift: `"Name"` oder `("A", "B")` → CSS-Liste mit Anführungszeichen.
fn font(value: &str) -> Option<String> {
    let v = value.trim();
    let names: Vec<String> = if v.starts_with('(') {
        args_at(v, 0)?.into_iter().filter(|(n, _)| n.is_none()).map(|(_, s)| string(&s)).collect::<Option<_>>()?
    } else {
        vec![string(v)?]
    };
    let names: Vec<String> = names
        .into_iter()
        .filter(|n| !n.is_empty() && n.chars().all(|c| c.is_alphanumeric() || " -_.".contains(c)))
        .map(|n| format!("\"{n}\""))
        .collect();
    (!names.is_empty()).then(|| names.join(", "))
}

/// Farbe: `rgb("#rrggbb")`, `rgb(r, g, b)`, `luma(n)`, `cmyk(…)` oder ein Farbname von Typst.
fn color(value: &str) -> Option<String> {
    let v = value.trim();
    if let Some(rest) = v.strip_prefix("rgb(").and_then(|r| r.strip_suffix(')')) {
        let parts: Vec<&str> = rest.split(',').map(str::trim).collect();
        if parts.len() == 1 {
            let hex = string(parts[0])?;
            let hex = hex.strip_prefix('#').unwrap_or(&hex).to_owned();
            return (matches!(hex.len(), 3 | 4 | 6 | 8) && hex.chars().all(|c| c.is_ascii_hexdigit()))
                .then(|| format!("#{}", hex.to_ascii_lowercase()));
        }
        if parts.len() == 3 || parts.len() == 4 {
            let channels: Vec<u8> = parts[..3].iter().map(|p| channel(p)).collect::<Option<_>>()?;
            return Some(format!("rgb({}, {}, {})", channels[0], channels[1], channels[2]));
        }
        return None;
    }
    if let Some(rest) = v.strip_prefix("luma(").and_then(|r| r.strip_suffix(')')) {
        let n = channel(rest.split(',').next()?.trim())?;
        return Some(format!("rgb({n}, {n}, {n})"));
    }
    if let Some(rest) = v.strip_prefix("cmyk(").and_then(|r| r.strip_suffix(')')) {
        let parts: Vec<f64> = rest.split(',').map(|p| percent(p.trim())).collect::<Option<_>>()?;
        if parts.len() != 4 {
            return None;
        }
        let k = 1.0 - parts[3];
        let to = |c: f64| ((1.0 - c) * k * 255.0).round() as u8;
        return Some(format!("rgb({}, {}, {})", to(parts[0]), to(parts[1]), to(parts[2])));
    }
    let named = match v {
        "black" => "#000000",
        "gray" => "#aaaaaa",
        "silver" => "#dddddd",
        "white" => "#ffffff",
        "navy" => "#001f3f",
        "blue" => "#0074d9",
        "aqua" => "#7fdbff",
        "teal" => "#39cccc",
        "eastern" => "#239dad",
        "purple" => "#b10dc9",
        "fuchsia" => "#f012be",
        "maroon" => "#85144b",
        "red" => "#ff4136",
        "orange" => "#ff851b",
        "yellow" => "#ffdc00",
        "olive" => "#3d9970",
        "green" => "#2ecc40",
        "lime" => "#01ff70",
        _ => return None,
    };
    Some(named.into())
}

/// Farbkanal: Ganzzahl 0–255 oder Prozent.
fn channel(part: &str) -> Option<u8> {
    if part.ends_with('%') {
        return percent(part).map(|p| (p * 255.0).round() as u8);
    }
    part.parse::<u8>().ok()
}

/// `n%` → n/100 (0–1).
fn percent(part: &str) -> Option<f64> {
    let n: f64 = part.strip_suffix('%')?.trim().parse().ok()?;
    (0.0..=100.0).contains(&n).then_some(n / 100.0)
}

/// Rahmen: `1pt`, `gray`, `0.5pt + gray` → `0.5pt solid #aaaaaa`.
fn stroke(value: &str) -> Option<String> {
    let mut width = None;
    let mut paint = None;
    for part in value.split('+').map(str::trim) {
        match length(part) {
            Some(l) => width = Some(l),
            None => paint = Some(color(part)?),
        }
    }
    let width = width.unwrap_or_else(|| "1pt".into());
    Some(format!("{width} solid {}", paint.unwrap_or_else(|| "currentColor".into())))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colors_lengths_and_strokes_translate_to_css() {
        assert_eq!(color("rgb(\"#1A2B3C\")").as_deref(), Some("#1a2b3c"), "Hexfarbe klein geschrieben");
        assert_eq!(color("luma(240)").as_deref(), Some("rgb(240, 240, 240)"), "Grauwert");
        assert_eq!(color("rgb(10%, 20%, 30%)").as_deref(), Some("rgb(26, 51, 77)"), "Prozentkanäle");
        assert_eq!(color("red.lighten(20%)"), None, "Methoden werden nicht ausgewertet");
        assert_eq!(length("11.5pt").as_deref(), Some("11.5pt"), "Länge bleibt");
        assert_eq!(length("1fr"), None, "Bruchteile sind keine Satzlänge");
        assert_eq!(stroke("0.5pt + gray").as_deref(), Some("0.5pt solid #aaaaaa"), "Stärke und Farbe");
    }

    #[test]
    fn unsafe_values_never_reach_the_css() {
        let style = SheetStyle { fill: Some("red; } body { x".into()), ..SheetStyle::default() };
        assert!(!sheet_css(&style).contains("body"), "ein Wert verlässt die Regel nicht");
    }
}
