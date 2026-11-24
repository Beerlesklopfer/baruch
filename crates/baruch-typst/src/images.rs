// SPDX-License-Identifier: AGPL-3.0-or-later
//! Bildpfade in Typst-Text (Vertrag Runde 6, Abschnitt 4b): Welche Dateien nennen Vorlage und Text?
//!
//! [`image_paths`] sucht `image(`-Aufrufe: im Markup als `#image(`, in Code und Argumenten als
//! `image(` (z. B. `#figure(image("…"), …)`, `#let bild = image("…")`). Kommentare und Raw-Text
//! zählen nicht ([`super::blank_comments`]). Erkannt wird nur ein String-Literal als erstes
//! Argument; berechnete Pfade (`"bilder/" + name`, eine Variable) bleiben unerkannt.
//!
//! Runde 11 („images“): [`computed_images`] findet die `image(`-Aufrufe mit **berechnetem** Pfad (z. B.
//! Kapitelbilder in der Werkvorlage) und ermittelt sie, wo das ohne Typst-Auswertung sicher geht:
//! `image(liste.at(i))` mit genau einer Definition `let liste = ("a.png", "b.png", …)` aus lauter
//! String-Literalen und einem Index, der eine Zahl ist ([`ComputedImage::Fixed`]) oder die Nummer des
//! Kapitels ± k (`counter(heading).get().first()`, `….at(0)`, mit `at(here())`, auch über genau eine
//! `let n = …`; [`ComputedImage::PerChapter`]). Alles andere ist [`ComputedImage::Unknown`]: Die
//! Oberfläche sagt dann ehrlich „Bild aus der Vorlage“, statt zu raten.

use super::{Arg, Cursor, blank_comments, ident_char, ident_start};

/// String-Literale des ersten Arguments aller `image(`-Aufrufe außerhalb von Kommentaren und Raw, in
/// Reihenfolge, ohne Doppelte.
pub fn image_paths(source: &str) -> Vec<String> {
    let text = blank_comments(source);
    let bytes = text.as_bytes();
    let mut out: Vec<String> = Vec::new();
    let mut from = 0;
    while let Some(found) = text[from..].find("image(") {
        let at = from + found;
        from = at + "image(".len();
        if !is_call(bytes, at) {
            continue;
        }
        if let Some(path) = first_string_argument(&text[from..])
            && !out.contains(&path)
        {
            out.push(path);
        }
    }
    out
}

/// Steht an `at` ein Aufruf von `image` (und kein Wortteil wie `myimage(` oder Fließtext wie
/// „ein image(…)“)? Direkt davor `#` (Markup), oder davor – über Leerraum hinweg – ein Zeichen, nach
/// dem ein Code-Ausdruck beginnt: `(`, `,`, `:`, `=`, `{` oder `;`. Ein `.` davor (Methode,
/// `x.image(`) zählt nicht.
fn is_call(bytes: &[u8], at: usize) -> bool {
    let Some(&before) = at.checked_sub(1).and_then(|i| bytes.get(i)) else {
        return false;
    };
    if before == b'#' {
        return true;
    }
    if ident_char(before) || before == b'.' {
        return false;
    }
    let mut i = at;
    while i > 0 && bytes[i - 1].is_ascii_whitespace() {
        i -= 1;
    }
    i > 0 && matches!(bytes[i - 1], b'(' | b',' | b':' | b'=' | b'{' | b';')
}

/// Erstes Argument nach der öffnenden Klammer, wenn es ein String-Literal ist, dem `,` oder `)`
/// folgt (sonst ein berechneter Ausdruck).
fn first_string_argument(rest: &str) -> Option<String> {
    let mut cursor = Cursor { s: rest, i: 0 };
    while cursor.peek().is_some_and(char::is_whitespace) {
        cursor.bump();
    }
    if cursor.peek() != Some('"') {
        return None;
    }
    cursor.bump();
    let start = cursor.i;
    let value = cursor.string();
    if !rest[start..cursor.i].ends_with('"') {
        return None; // String schließt nicht
    }
    while cursor.peek().is_some_and(char::is_whitespace) {
        cursor.bump();
    }
    matches!(cursor.peek(), Some(',' | ')')).then_some(value).filter(|v| !v.is_empty())
}

/// Ein `image(`-Aufruf mit berechnetem Pfad ([`computed_images`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComputedImage {
    /// `liste.at(Kapitelnummer + offset)`: im Kapitel `n` (ab 1) das Element `n + offset` (ab 0).
    PerChapter { list: Vec<String>, offset: i64 },
    /// `liste.at(k)` mit fester Zahl: immer dasselbe Bild.
    Fixed { path: String },
    /// Nicht sicher ermittelbar; `expr` ist das erste Argument, wie es im Text steht (getrimmt).
    Unknown { expr: String },
}

impl ComputedImage {
    /// Bild im Kapitel `chapter` (ab 1): `Some(Pfad)`, wenn sicher ermittelt; `None` bei
    /// [`ComputedImage::Unknown`] oder einem Index außerhalb der Liste (Typst meldete dort einen Fehler).
    pub fn for_chapter(&self, chapter: usize) -> Option<&str> {
        match self {
            ComputedImage::PerChapter { list, offset } => {
                let index = i64::try_from(chapter).ok()?.checked_add(*offset)?;
                list.get(usize::try_from(index).ok()?).map(String::as_str)
            }
            ComputedImage::Fixed { path } => Some(path),
            ComputedImage::Unknown { .. } => None,
        }
    }
}

/// Alle `image(`-Aufrufe, deren erstes Argument kein String-Literal ist, in Reihenfolge (Kommentare
/// und Raw zählen nicht), je mit dem, was sich sicher ermitteln lässt (siehe Moduldoku).
pub fn computed_images(source: &str) -> Vec<ComputedImage> {
    let text = blank_comments(source);
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(found) = text[from..].find("image(") {
        let at = from + found;
        from = at + "image(".len();
        if !is_call(bytes, at) && !is_statement(bytes, at) {
            continue;
        }
        let rest = &text[from..];
        if first_string_argument(rest).is_some() {
            continue; // Literal: das kennt image_paths
        }
        let expr = raw_argument(rest);
        if expr.is_empty() || expr.starts_with('[') {
            continue; // nichts oder Inhalt: kein Pfad
        }
        out.push(resolve(&text, expr));
    }
    out
}

/// Steht `image(` als eigene Anweisung in einem Code-Block (`{ … }` bzw. `context { … }`): am
/// Zeilenanfang (nur Leerraum davor) innerhalb einer offenen `{`? So schreiben Vorlagen Kapitelbilder
/// in `show`-Regeln; im Markup wäre dieselbe Zeile Fließtext.
fn is_statement(bytes: &[u8], at: usize) -> bool {
    let mut i = at;
    while i > 0 && matches!(bytes[i - 1], b' ' | b'\t') {
        i -= 1;
    }
    if i > 0 && bytes[i - 1] != b'\n' {
        return false;
    }
    let mut depth = 0i64;
    for &b in &bytes[..at] {
        match b {
            b'{' => depth += 1,
            b'}' => depth -= 1,
            _ => {}
        }
    }
    depth > 0
}

/// Erstes Argument roh (getrimmt): bis zum `,` bzw. `)` auf Ebene 0, Strings übersprungen.
fn raw_argument(rest: &str) -> &str {
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    for (i, c) in rest.char_indices() {
        if in_string {
            match (escaped, c) {
                (true, _) => escaped = false,
                (false, '\\') => escaped = true,
                (false, '"') => in_string = false,
                _ => {}
            }
            continue;
        }
        match c {
            '"' => in_string = true,
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' if depth == 0 => return rest[..i].trim(),
            ')' | ']' | '}' => depth -= 1,
            ',' if depth == 0 => return rest[..i].trim(),
            _ => {}
        }
    }
    rest.trim()
}

/// `liste.at(index)` auflösen, sonst `Unknown`.
fn resolve(text: &str, expr: &str) -> ComputedImage {
    let unknown = || ComputedImage::Unknown { expr: expr.to_owned() };
    let Some((name, index)) = split_at_call(expr) else { return unknown() };
    let Some(list) = literal_list(text, name) else { return unknown() };
    let index = index.trim();
    if let Ok(k) = index.parse::<usize>() {
        return list.get(k).map_or_else(unknown, |path| ComputedImage::Fixed { path: path.clone() });
    }
    let offset = chapter_offset(index).or_else(|| {
        // Index über genau eine Variable: `let n = counter(heading).get().first() - 1`.
        let variable = ident_only(index)?;
        let value = single_let(text, variable)?;
        chapter_offset(&value)
    });
    match offset {
        Some(offset) => ComputedImage::PerChapter { list, offset },
        None => unknown(),
    }
}

/// `name.at(index)` → (`name`, `index`); sonst `None` (auch bei Resten dahinter).
fn split_at_call(expr: &str) -> Option<(&str, &str)> {
    let bytes = expr.as_bytes();
    if !bytes.first().is_some_and(|&b| ident_start(b)) {
        return None;
    }
    let end = bytes.iter().position(|&b| !ident_char(b)).unwrap_or(bytes.len());
    let name = &expr[..end];
    let rest = expr[end..].trim_start().strip_prefix(".at(")?;
    let close = matching_paren(rest)?;
    rest[close + 1..].trim().is_empty().then_some((name, &rest[..close]))
}

/// Stelle der `)`, die die schon geöffnete Klammer schließt (Strings übersprungen).
fn matching_paren(rest: &str) -> Option<usize> {
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    for (i, c) in rest.char_indices() {
        if in_string {
            match (escaped, c) {
                (true, _) => escaped = false,
                (false, '\\') => escaped = true,
                (false, '"') => in_string = false,
                _ => {}
            }
            continue;
        }
        match c {
            '"' => in_string = true,
            '(' => depth += 1,
            ')' if depth == 0 => return Some(i),
            ')' => depth -= 1,
            _ => {}
        }
    }
    None
}

/// Nur ein Bezeichner?
fn ident_only(text: &str) -> Option<&str> {
    let bytes = text.as_bytes();
    (bytes.first().is_some_and(|&b| ident_start(b)) && bytes.iter().all(|&b| ident_char(b))).then_some(text)
}

/// Stellen von `let <name> =` (Wortgrenzen; `==` zählt nicht) mit der Stelle nach dem `=`.
fn let_values<'t>(text: &'t str, name: &str) -> Vec<&'t str> {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(found) = text[from..].find("let") {
        let at = from + found;
        from = at + 3;
        if at > 0 && ident_char(bytes[at - 1]) {
            continue;
        }
        let rest = &text[at + 3..];
        let Some(rest) = rest.strip_prefix(|c: char| c.is_whitespace()) else { continue };
        let rest = rest.trim_start();
        let Some(after) = rest.strip_prefix(name) else { continue };
        if after.as_bytes().first().is_some_and(|&b| ident_char(b)) {
            continue; // längerer Name
        }
        let after = after.trim_start();
        if let Some(value) = after.strip_prefix('=')
            && !value.starts_with('=')
        {
            out.push(value);
        }
    }
    out
}

/// Wert von genau einer Definition `let <name> = …` bis zum Zeilenende bzw. `;` (getrimmt); mehrere
/// oder keine → `None` (welche gilt, wüsste nur Typst).
fn single_let(text: &str, name: &str) -> Option<String> {
    let values = let_values(text, name);
    let [value] = values.as_slice() else { return None };
    let end = value.find(['\n', ';']).unwrap_or(value.len());
    Some(value[..end].trim().to_owned())
}

/// Liste aus genau einer Definition `let <name> = ("…", "…", …)` aus lauter String-Literalen (ein
/// einzelnes Element nur mit Komma, sonst wäre es ein geklammerter String).
fn literal_list(text: &str, name: &str) -> Option<Vec<String>> {
    let values = let_values(text, name);
    let [value] = values.as_slice() else { return None };
    let value = value.trim_start();
    let inner = value.strip_prefix('(')?;
    let close = matching_paren(inner)?;
    let mut cursor = Cursor { s: value, i: 0 };
    let Arg::Tuple(items) = cursor.value() else { return None };
    let mut list = Vec::with_capacity(items.len());
    for item in items {
        let Arg::Str(path) = item else { return None };
        list.push(path);
    }
    let single_without_comma = list.len() == 1 && !inner[..close].contains(',');
    (!list.is_empty() && !single_without_comma).then_some(list)
}

/// Index „Nummer des Kapitels ± k“ → `k`; sonst `None`. Leerraum und eine Klammer außen herum zählen
/// nicht.
fn chapter_offset(index: &str) -> Option<i64> {
    let compact: String = index.chars().filter(|c| !c.is_whitespace()).collect();
    let compact = compact.strip_prefix('(').and_then(|c| c.strip_suffix(')')).unwrap_or(&compact);
    const CHAPTER: [&str; 4] = [
        "counter(heading).get().first()",
        "counter(heading).get().at(0)",
        "counter(heading).at(here()).first()",
        "counter(heading).at(here()).at(0)",
    ];
    let rest = CHAPTER.iter().find_map(|c| compact.strip_prefix(c))?;
    if rest.is_empty() {
        return Some(0);
    }
    let (sign, digits) = match rest.split_at(1) {
        ("-", d) => (-1, d),
        ("+", d) => (1, d),
        _ => return None,
    };
    digits.parse::<i64>().ok().map(|k| sign * k)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chapter_offsets_are_read_without_spaces_and_outer_parens() {
        assert_eq!(chapter_offset("counter(heading).get().first() - 1"), Some(-1), "minus eins");
        assert_eq!(chapter_offset("( counter(heading).at(here()).at(0) )"), Some(0), "Klammer außen");
        assert_eq!(chapter_offset("counter(heading).get().first() * 2"), None, "Malnehmen kennt nur Typst");
        assert_eq!(chapter_offset("counter(page).get().first()"), None, "Seitenzähler ist kein Kapitel");
    }

    #[test]
    fn a_word_ending_in_image_is_no_call() {
        assert!(image_paths("#myimage(\"a.png\")").is_empty(), "myimage ist ein anderer Name");
        assert!(image_paths("#x.image(\"a.png\")").is_empty(), "Methode zählt nicht");
        assert!(image_paths("Das image(\"a.png\") im Fließtext").is_empty(), "ohne # im Markup kein Aufruf");
    }
}
