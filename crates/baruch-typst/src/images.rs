// SPDX-License-Identifier: AGPL-3.0-or-later
//! Image paths in Typst text (contract Round 6, section 4b): which files do template and text name?
//!
//! [`image_paths`] looks for `image(` calls: in markup as `#image(`, in code and arguments as `image(` (e.g.
//! `#figure(image("…"), …)`, `#let bild = image("…")`). Comments and raw text do not count
//! ([`super::blank_comments`]). Only a string literal as first argument is recognised; computed paths (`"bilder/" +
//! name`, a variable) stay unrecognised.
//!
//! Round 11 („images“): [`computed_images`] finds the `image(` calls with a **computed** path (e.g. chapter images
//! in the work template) and determines them where that is safely possible without evaluating Typst:
//! `image(liste.at(i))` with exactly one definition `let liste = ("a.png", "b.png", …)` of nothing but string
//! literals and an index that is a number ([`ComputedImage::Fixed`]) or the number of the chapter ± k
//! (`counter(heading).get().first()`, `….at(0)`, with `at(here())`, also through exactly one `let n = …`;
//! [`ComputedImage::PerChapter`]). Everything else is [`ComputedImage::Unknown`]: the user interface then honestly
//! says „Bild aus der Vorlage“ instead of guessing.

use super::{Arg, Cursor, blank_comments, ident_char, ident_start};

/// String literals of the first argument of all `image(` calls outside of comments and raw, in order, without
/// duplicates.
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

/// Does a call of `image` stand at `at` (and not part of a word like `myimage(` or running text like „ein
/// image(…)“)? Directly before it `#` (markup), or before it – across whitespace – a character after which a code
/// expression starts: `(`, `,`, `:`, `=`, `{` or `;`. A `.` before it (method, `x.image(`) does not count.
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

/// First argument after the opening parenthesis if it is a string literal followed by `,` or `)` (otherwise a
/// computed expression).
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
        return None; // string does not close
    }
    while cursor.peek().is_some_and(char::is_whitespace) {
        cursor.bump();
    }
    matches!(cursor.peek(), Some(',' | ')')).then_some(value).filter(|v| !v.is_empty())
}

/// An `image(` call with a computed path ([`computed_images`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComputedImage {
    /// `liste.at(chapter number + offset)`: in chapter `n` (from 1) the element `n + offset` (from 0).
    PerChapter { list: Vec<String>, offset: i64 },
    /// `liste.at(k)` with a fixed number: always the same image.
    Fixed { path: String },
    /// Cannot be determined safely; `expr` is the first argument as it stands in the text (trimmed).
    Unknown { expr: String },
}

impl ComputedImage {
    /// Image in the chapter `chapter` (from 1): `Some(path)` if safely determined; `None` for
    /// [`ComputedImage::Unknown`] or an index outside the list (Typst would report an error there).
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

/// All `image(` calls whose first argument is not a string literal, in order (comments and raw do not count), each
/// with what can be determined safely (see module documentation).
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
            continue; // literal: image_paths knows that
        }
        let expr = raw_argument(rest);
        if expr.is_empty() || expr.starts_with('[') {
            continue; // nothing or content: no path
        }
        out.push(resolve(&text, expr));
    }
    out
}

/// Does `image(` stand as a statement of its own in a code block (`{ … }` or `context { … }`): at the start of a
/// line (only whitespace before it) within an open `{`? This is how templates write chapter images in `show` rules;
/// in markup the same line would be running text.
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

/// First argument raw (trimmed): up to the `,` or `)` at level 0, strings skipped.
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

/// Resolve `liste.at(index)`, otherwise `Unknown`.
fn resolve(text: &str, expr: &str) -> ComputedImage {
    let unknown = || ComputedImage::Unknown { expr: expr.to_owned() };
    let Some((name, index)) = split_at_call(expr) else { return unknown() };
    let Some(list) = literal_list(text, name) else { return unknown() };
    let index = index.trim();
    if let Ok(k) = index.parse::<usize>() {
        return list.get(k).map_or_else(unknown, |path| ComputedImage::Fixed { path: path.clone() });
    }
    let offset = chapter_offset(index).or_else(|| {
        // Index through exactly one variable: `let n = counter(heading).get().first() - 1`.
        let variable = ident_only(index)?;
        let value = single_let(text, variable)?;
        chapter_offset(&value)
    });
    match offset {
        Some(offset) => ComputedImage::PerChapter { list, offset },
        None => unknown(),
    }
}

/// `name.at(index)` → (`name`, `index`); otherwise `None` (also with leftovers behind it).
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

/// Position of the `)` that closes the already opened parenthesis (strings skipped).
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

/// Only an identifier?
fn ident_only(text: &str) -> Option<&str> {
    let bytes = text.as_bytes();
    (bytes.first().is_some_and(|&b| ident_start(b)) && bytes.iter().all(|&b| ident_char(b))).then_some(text)
}

/// Places of `let <name> =` (word boundaries; `==` does not count) with the position after the `=`.
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
            continue; // longer name
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

/// Value of exactly one definition `let <name> = …` up to the end of the line or `;` (trimmed); several or none →
/// `None` (which one applies only Typst would know).
fn single_let(text: &str, name: &str) -> Option<String> {
    let values = let_values(text, name);
    let [value] = values.as_slice() else { return None };
    let end = value.find(['\n', ';']).unwrap_or(value.len());
    Some(value[..end].trim().to_owned())
}

/// List from exactly one definition `let <name> = ("…", "…", …)` of nothing but string literals (a single element
/// only with a comma, otherwise it would be a parenthesised string).
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

/// Index "number of the chapter ± k" → `k`; otherwise `None`. Whitespace and one parenthesis around it do not count.
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
