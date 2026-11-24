// SPDX-License-Identifier: AGPL-3.0-or-later
//! Kleine Leser über Typst-Quelltext: Wo endet ein Raw-Text, ein Kommentar, eine Formel, ein
//! `#`-Ausdruck, ein Inhaltsblock `[…]`? Alle Positionen sind Byte-Stellen in `&str` auf
//! Zeichengrenzen. Ist etwas nicht geschlossen, reicht es bis zum Ende des Textes, und der
//! [`Scanner`] merkt sich das (`open`); der Aufrufer macht daraus eine Insel. Die Schachtelung ist
//! auf [`MAX_DEPTH`] begrenzt, damit ein bösartiger Text (`[[[[…`) den Stapel nicht sprengt.

/// Tiefste Schachtelung von Klammern und Inhaltsblöcken, die noch gelesen wird.
pub(crate) const MAX_DEPTH: usize = 48;

/// Schlüsselwörter, nach denen ein `#`-Ausdruck Code bis zum Zeilenende ist (`#let`, `#set` …).
const KEYWORDS: &[&str] =
    &["let", "set", "show", "import", "include", "if", "for", "while", "context", "return", "break", "continue"];

pub(crate) fn is_keyword(name: &str) -> bool {
    KEYWORDS.contains(&name)
}

pub(crate) fn is_ident_start(c: char) -> bool {
    c.is_alphabetic() || c == '_'
}

pub(crate) fn is_ident_continue(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '-'
}

/// Zeichen eines Labels `<…>` (wie in Typst).
pub(crate) fn is_label_char(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '_' | '-' | ':' | '.')
}

/// Zeichen an Stelle `i` (falls vorhanden).
pub(crate) fn char_at(s: &str, i: usize) -> Option<char> {
    s.get(i..).and_then(|rest| rest.chars().next())
}

/// Stelle nach dem Zeichen an `i`.
pub(crate) fn next_char(s: &str, i: usize) -> usize {
    i + char_at(s, i).map_or(1, char::len_utf8)
}

/// Steht vor Stelle `i` ein Linkanfang (`http:` bzw. `https:`)? Dann ist `//` kein Kommentar.
pub(crate) fn after_scheme(s: &str, i: usize) -> bool {
    let before = &s[..i];
    before.ends_with("http:") || before.ends_with("https:")
}

/// Bezeichner ab `i`; Ende (exklusiv), `i` wenn dort keiner beginnt.
pub(crate) fn ident_end(s: &str, i: usize) -> usize {
    let mut j = i;
    while let Some(c) = char_at(s, j) {
        if (j == i && !is_ident_start(c)) || (j > i && !is_ident_continue(c)) {
            break;
        }
        j += c.len_utf8();
    }
    j
}

/// Maskierung `\x` ab `i` (`s[i] == '\\'`): Ende. `\u{…}` reicht bis zur schließenden Klammer. Vor Leerraum
/// (auch einem Zeilenende) ist `\` ein erzwungener Umbruch und nur das eine Zeichen (Runde 18b): Das
/// Zeilenende dahinter beendet die Zeile weiterhin.
pub(crate) fn skip_escape(s: &str, i: usize) -> usize {
    let j = i + 1;
    if char_at(s, j).is_some_and(char::is_whitespace) {
        return j;
    }
    if s[j..].starts_with("u{")
        && let Some(close) = s[j..].find('}')
    {
        return j + close + 1;
    }
    if j >= s.len() { s.len() } else { next_char(s, j) }
}

/// Zeilenkommentar ab `i`: Ende (vor dem Zeilenende).
pub(crate) fn skip_line_comment(s: &str, i: usize) -> usize {
    s[i..].find('\n').map_or(s.len(), |n| i + n)
}

/// Leser über einen Text; `open` wird wahr, sobald etwas nicht geschlossen ist.
pub(crate) struct Scanner<'a> {
    pub s: &'a str,
    pub open: bool,
}

impl<'a> Scanner<'a> {
    pub fn new(s: &'a str) -> Self {
        Scanner { s, open: false }
    }

    fn unclosed(&mut self) -> usize {
        self.open = true;
        self.s.len()
    }

    /// Raw-Text ab `i` (Backticks): Ende. Zwei Backticks sind leerer Raw-Text, ab drei schließt
    /// dieselbe Anzahl.
    pub fn raw(&mut self, i: usize) -> usize {
        let s = self.s;
        let run = s[i..].bytes().take_while(|&b| b == b'`').count();
        if run == 2 {
            return i + 2;
        }
        let fence = "`".repeat(run);
        match s[i + run..].find(&fence) {
            Some(close) => i + run + close + run,
            None => self.unclosed(),
        }
    }

    /// Blockkommentar `/* … */` ab `i` (geschachtelt): Ende.
    pub fn block_comment(&mut self, i: usize) -> usize {
        let bytes = self.s.as_bytes();
        let mut depth = 0usize;
        let mut j = i;
        while j < bytes.len() {
            if bytes[j..].starts_with(b"/*") {
                depth += 1;
                j += 2;
            } else if bytes[j..].starts_with(b"*/") {
                depth -= 1;
                j += 2;
                if depth == 0 {
                    return j;
                }
            } else {
                j += 1;
            }
        }
        self.unclosed()
    }

    /// Formel `$…$` ab `i`: Ende.
    pub fn math(&mut self, i: usize) -> usize {
        let s = self.s;
        let mut j = i + 1;
        while j < s.len() {
            match s.as_bytes()[j] {
                b'\\' => j = skip_escape(s, j),
                b'$' => return j + 1,
                _ => j = next_char(s, j),
            }
        }
        self.unclosed()
    }

    /// String `"…"` ab `i`: Ende.
    pub fn string(&mut self, i: usize) -> usize {
        let s = self.s;
        let mut j = i + 1;
        while j < s.len() {
            match s.as_bytes()[j] {
                b'\\' => j = skip_escape(s, j),
                b'"' => return j + 1,
                _ => j = next_char(s, j),
            }
        }
        self.unclosed()
    }

    /// Code in Klammern `(…)` bzw. `{…}` ab `i`: Ende nach der schließenden Klammer. Eine fremde
    /// schließende Klammer beendet das Lesen vor ihr (offen).
    pub fn code_group(&mut self, i: usize, depth: usize) -> usize {
        if depth > MAX_DEPTH {
            return self.unclosed();
        }
        let s = self.s;
        let close = if s.as_bytes()[i] == b'(' { b')' } else { b'}' };
        let mut j = i + 1;
        while j < s.len() {
            let b = s.as_bytes()[j];
            match b {
                b'"' => j = self.string(j),
                b'`' => j = self.raw(j),
                b'/' if s[j..].starts_with("//") => j = skip_line_comment(s, j),
                b'/' if s[j..].starts_with("/*") => j = self.block_comment(j),
                b'(' | b'{' => j = self.code_group(j, depth + 1),
                b'[' => j = self.content(j, depth + 1),
                _ if b == close => return j + 1,
                b')' | b'}' | b']' => {
                    self.open = true;
                    return j;
                }
                _ => j = next_char(s, j),
            }
        }
        self.unclosed()
    }

    /// Inhaltsblock `[…]` ab `i` (Markup, geschachtelt): Ende nach der schließenden Klammer.
    pub fn content(&mut self, i: usize, depth: usize) -> usize {
        if depth > MAX_DEPTH {
            return self.unclosed();
        }
        let s = self.s;
        let mut j = i + 1;
        while j < s.len() {
            match s.as_bytes()[j] {
                b'\\' => j = skip_escape(s, j),
                b'`' => j = self.raw(j),
                b'$' => j = self.math(j),
                b'/' if s[j..].starts_with("//") && !after_scheme(s, j) => j = skip_line_comment(s, j),
                b'/' if s[j..].starts_with("/*") => j = self.block_comment(j),
                b'#' => j = self.hash(j, depth + 1),
                b'[' => j = self.content(j, depth + 1),
                b']' => return j + 1,
                _ => j = next_char(s, j),
            }
        }
        self.unclosed()
    }

    /// Code nach einem Schlüsselwort (`#let …`, `#show …: it => […]`): bis zum Zeilenende außerhalb
    /// von Klammern, bis `;` oder vor einer fremden schließenden Klammer.
    fn keyword_expr(&mut self, i: usize, depth: usize) -> usize {
        let s = self.s;
        let mut j = i;
        while j < s.len() {
            match s.as_bytes()[j] {
                b'\n' | b';' => return j,
                b'"' => j = self.string(j),
                b'`' => j = self.raw(j),
                b'/' if s[j..].starts_with("//") => return j,
                b'/' if s[j..].starts_with("/*") => j = self.block_comment(j),
                b'(' | b'{' => j = self.code_group(j, depth + 1),
                b'[' => j = self.content(j, depth + 1),
                b')' | b'}' | b']' => return j,
                _ => j = next_char(s, j),
            }
        }
        s.len()
    }

    /// `#`-Ausdruck ab `i` (`s[i] == '#'`): Ende. Ein Name mit angehängten Aufrufen `(…)`,
    /// Inhaltsblöcken `[…]` und Feldern `.name`; nach einem Schlüsselwort Code bis zum Zeilenende;
    /// `#(…)`, `#{…}`, `#"…"`. Folgt dem `#` nichts davon, ist es nur das eine Zeichen.
    pub fn hash(&mut self, i: usize, depth: usize) -> usize {
        if depth > MAX_DEPTH {
            return self.unclosed();
        }
        let s = self.s;
        let start = i + 1;
        let mut j = match char_at(s, start) {
            Some(c) if is_ident_start(c) => {
                let end = ident_end(s, start);
                if is_keyword(&s[start..end]) {
                    return self.keyword_expr(end, depth);
                }
                end
            }
            Some('(') => self.code_group(start, depth + 1),
            Some('{') => return self.code_group(start, depth + 1),
            Some('"') => self.string(start),
            _ => return start,
        };
        loop {
            match char_at(s, j) {
                Some('(') => j = self.code_group(j, depth + 1),
                Some('[') => j = self.content(j, depth + 1),
                Some('.') if char_at(s, j + 1).is_some_and(is_ident_start) => j = ident_end(s, j + 1),
                _ => return j,
            }
        }
    }
}

/// Ende des geschlossenen Inhaltsblocks ab `i` in `s`, `None` wenn er nicht schließt.
pub(crate) fn closed_content(s: &str, i: usize) -> Option<usize> {
    let mut scanner = Scanner::new(s);
    let end = scanner.content(i, 0);
    (!scanner.open).then_some(end)
}

/// Ende der geschlossenen Klammer `(…)` ab `i` in `s`, `None` wenn sie nicht schließt.
pub(crate) fn closed_group(s: &str, i: usize) -> Option<usize> {
    let mut scanner = Scanner::new(s);
    let end = scanner.code_group(i, 0);
    (!scanner.open).then_some(end)
}

/// Stellen der Zeilenenden, die im Markup der obersten Ebene liegen (nicht in Raw-Text, Kommentaren,
/// Formeln oder `#`-Ausdrücken): die „logischen Zeilen“ als Bereiche, jeweils mit Zeilenende.
pub(crate) fn logical_lines(s: &str) -> Vec<(usize, usize)> {
    let mut scanner = Scanner::new(s);
    let mut lines = Vec::new();
    let mut start = 0;
    let mut j = 0;
    while j < s.len() {
        match s.as_bytes()[j] {
            b'\\' => j = skip_escape(s, j),
            b'`' => j = scanner.raw(j),
            b'$' => j = scanner.math(j),
            b'/' if s[j..].starts_with("//") && !after_scheme(s, j) => j = skip_line_comment(s, j),
            b'/' if s[j..].starts_with("/*") => j = scanner.block_comment(j),
            b'#' => j = scanner.hash(j, 0),
            b'\n' => {
                lines.push((start, j + 1));
                j += 1;
                start = j;
            }
            _ => j = next_char(s, j),
        }
    }
    if start < s.len() {
        lines.push((start, s.len()));
    }
    lines
}

/// Anfang eines Kommentars in einer Überschriftszeile, genau wie `baruch_typst::heading_line` ihn
/// sucht: Maskierungen und Inline-Raw zählen nicht, ebenso `//` hinter `http:`/`https:`.
pub(crate) fn heading_comment_start(text: &str) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => i += 2,
            b'`' => {
                let run = text[i..].bytes().take_while(|&b| b == b'`').count();
                let close = text[i + run..].find(&"`".repeat(run))?;
                i += run + close + run;
            }
            b'/' if bytes.get(i + 1) == Some(&b'*') => return Some(i),
            b'/' if bytes.get(i + 1) == Some(&b'/') && !after_scheme(text, i) => return Some(i),
            _ => i += 1,
        }
    }
    None
}

/// Bildpfad einer Insel: das String-Literal im ersten Argument des ersten `image(`-Aufrufs, auch
/// geschachtelt (`#figure(image("/bilder/haus.svg"), caption: […])`), außerhalb von Kommentaren,
/// Raw-Text und Formeln. Ein berechneter Pfad (`image("a" + p)`) oder ein anderes erstes Argument
/// ergibt `None`.
pub fn image_path(raw: &str) -> Option<String> {
    let mut found = Vec::new();
    find_image_markup(raw, 0, raw.len(), 0, &mut found, true);
    found.into_iter().next()?
}

/// Runde 18b: Bildpfade **aller** `image(`-Aufrufe einer Insel in Textreihenfolge (wie [`image_path`] je
/// Aufruf; Aufrufe ohne String-Literal fehlen). Ein Block mit mehreren Bildern zeigt so alle.
pub fn image_paths(raw: &str) -> Vec<String> {
    let mut found = Vec::new();
    find_image_markup(raw, 0, raw.len(), 0, &mut found, false);
    found.into_iter().flatten().collect()
}

/// Wert eines String-Literals `"…"` (ganzer Text), Maskierungen aufgelöst; sonst `None`.
pub(crate) fn string_value(literal: &str) -> Option<String> {
    let mut scanner = Scanner::new(literal);
    if !literal.starts_with('"') || scanner.string(0) != literal.len() || scanner.open {
        return None;
    }
    unescape(&literal[1..literal.len() - 1])
}

/// Inhalt eines String-Literals ohne Anführungszeichen auflösen (`\n`, `\t`, `\r`, `\u{…}`, `\x` → `x`).
fn unescape(inner: &str) -> Option<String> {
    let mut value = String::new();
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            value.push(c);
            continue;
        }
        match chars.next()? {
            'n' => value.push('\n'),
            't' => value.push('\t'),
            'r' => value.push('\r'),
            'u' => {
                let rest: String = chars.by_ref().take_while(|&c| c != '}').collect();
                let hex = rest.strip_prefix('{')?;
                value.push(u32::from_str_radix(hex, 16).ok().and_then(char::from_u32)?);
            }
            other => value.push(other),
        }
    }
    Some(value)
}

/// Im Markup `s[from..to]` nach `#…`-Ausdrücken suchen und jeden `image(`-Aufruf in `found` sammeln
/// (`None`: ohne String-Literal); mit `first` endet die Suche beim ersten. `true`, wenn sie enden soll.
fn find_image_markup(
    s: &str,
    from: usize,
    to: usize,
    depth: usize,
    found: &mut Vec<Option<String>>,
    first: bool,
) -> bool {
    if depth > MAX_DEPTH {
        return false;
    }
    let s = &s[..to];
    let mut scanner = Scanner::new(s);
    let mut j = from;
    while j < s.len() {
        match s.as_bytes()[j] {
            b'\\' => j = skip_escape(s, j),
            b'`' => j = scanner.raw(j),
            b'$' => j = scanner.math(j),
            b'/' if s[j..].starts_with("//") && !after_scheme(s, j) => j = skip_line_comment(s, j),
            b'/' if s[j..].starts_with("/*") => j = scanner.block_comment(j),
            b'#' => {
                let end = scanner.hash(j, 0);
                if find_image_code(s, j + 1, end, depth + 1, found, first) {
                    return true;
                }
                j = end.max(j + 1);
            }
            _ => j = next_char(s, j),
        }
    }
    false
}

/// Im Code `s[from..to]` (Argumente, Klammern) nach `image(` suchen; Inhaltsblöcke `[…]` sind wieder
/// Markup. Wie [`find_image_markup`].
fn find_image_code(
    s: &str,
    from: usize,
    to: usize,
    depth: usize,
    found: &mut Vec<Option<String>>,
    first: bool,
) -> bool {
    if depth > MAX_DEPTH {
        return false;
    }
    let s = &s[..to];
    let mut scanner = Scanner::new(s);
    let mut j = from;
    while j < s.len() {
        let Some(c) = char_at(s, j) else { return false };
        match c {
            '"' => j = scanner.string(j),
            '`' => j = scanner.raw(j),
            '/' if s[j..].starts_with("//") => j = skip_line_comment(s, j),
            '/' if s[j..].starts_with("/*") => j = scanner.block_comment(j),
            '[' => {
                let end = scanner.content(j, 0);
                // Ohne die schließende Klammer, falls der Block geschlossen ist.
                let inner_end = if s[..end].ends_with(']') && end > j + 1 { end - 1 } else { end };
                if find_image_markup(s, j + 1, inner_end, depth + 1, found, first) {
                    return true;
                }
                j = end.max(j + 1);
            }
            _ if is_ident_start(c) => {
                let end = ident_end(s, j);
                let method = s[..j].ends_with('.');
                if &s[j..end] == "image" && !method && s[end..].starts_with('(') {
                    found.push(string_argument(s, end + 1));
                    if first {
                        return true;
                    }
                }
                j = end;
            }
            _ => j += c.len_utf8(),
        }
    }
    false
}

/// String-Literal als ganzes erstes Argument ab `i` (hinter der öffnenden Klammer), aufgelöst.
fn string_argument(s: &str, i: usize) -> Option<String> {
    let start = i + (s[i..].len() - s[i..].trim_start().len());
    if !s[start..].starts_with('"') {
        return None;
    }
    let mut scanner = Scanner::new(s);
    let end = scanner.string(start);
    if scanner.open || !matches!(s[end..].trim_start().chars().next(), Some(',' | ')')) {
        return None;
    }
    unescape(&s[start + 1..end - 1])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(s: &str) -> (&str, bool) {
        let mut scanner = Scanner::new(s);
        let end = scanner.hash(0, 0);
        (&s[..end], scanner.open)
    }

    #[test]
    fn a_hash_expression_ends_after_its_arguments_and_fields() {
        assert_eq!(
            hash("#link(\"a]b\")[Text] danach"),
            ("#link(\"a]b\")[Text]", false),
            "String mit ] und Inhaltsblock gehören dazu"
        );
        assert_eq!(hash("#sym.arrow.r x"), ("#sym.arrow.r", false), "Felder gehören dazu");
        assert_eq!(hash("#footnote[x]. Satz"), ("#footnote[x]", false), "ein Punkt vor Leerzeichen ist Text");
        assert_eq!(
            hash("#let x = [a\n\nb]\nText"),
            ("#let x = [a\n\nb]", false),
            "Schlüsselwort: Code bis zum Zeilenende außerhalb der Klammern"
        );
        assert!(hash("#f[offen").1, "nicht geschlossener Inhaltsblock ist offen");
    }

    #[test]
    fn deep_nesting_stops_at_the_limit_instead_of_overflowing() {
        let s = "[".repeat(100_000);
        assert_eq!(closed_content(&s, 0), None, "nicht geschlossen und zu tief: offen, ohne Absturz");
        let s = format!("x{}", "#f[".repeat(50_000));
        assert_eq!(logical_lines(&s).len(), 1, "eine logische Zeile, ohne Absturz");
    }

    #[test]
    fn the_image_path_is_the_literal_of_the_first_image_call() {
        assert_eq!(image_path("#image(\"/bilder/a.png\")").as_deref(), Some("/bilder/a.png"), "Markup");
        assert_eq!(
            image_path("#figure(image(\"/bilder/b.svg\", width: 50%), caption: [Ein #emph[Bild]])").as_deref(),
            Some("/bilder/b.svg"),
            "in einer Abbildung"
        );
        assert_eq!(image_path("// #image(\"/bilder/c.png\")"), None, "im Kommentar nicht");
        assert_eq!(image_path("/* #image(\"/bilder/c.png\") */"), None, "im Blockkommentar nicht");
        assert_eq!(image_path("`#image(\"/x.png\")`"), None, "im Raw-Text nicht");
        assert_eq!(image_path("#image(\"a\" + p)"), None, "berechneter Pfad nicht");
        assert_eq!(image_path("#x.image(\"/m.png\")"), None, "eine Methode heißt nur so");
        assert_eq!(image_path("#images(\"/n.png\")"), None, "anderer Name");
        assert_eq!(
            image_path("#box[Text #image(\"/i\\u{e4}.png\")]").as_deref(),
            Some("/iä.png"),
            "im Inhaltsblock, Maskierung aufgelöst"
        );
        assert_eq!(
            image_path("#figure(image(\"/erst.png\"))\n#image(\"/zweit.png\")").as_deref(),
            Some("/erst.png"),
            "der erste Aufruf zählt"
        );
    }

    #[test]
    fn logical_lines_skip_newlines_inside_constructs() {
        let s = "a #szene[\nb\n\nc\n]\nd\n";
        let lines: Vec<&str> = logical_lines(s).into_iter().map(|(a, b)| &s[a..b]).collect();
        assert_eq!(lines, ["a #szene[\nb\n\nc\n]\n", "d\n"], "Zeilenenden im Inhaltsblock zählen nicht");
    }
}
