// SPDX-License-Identifier: AGPL-3.0-or-later
//! Zerlegen einer Typst-Datei in Werkvorlage und Gliederung (Vertrag B1.5, Abschnitt 2).
//!
//! - **Werkvorlage** ist alles vor der ersten Überschrift (Präambel mit `#set`, `#show`, `#let` …).
//! - **Überschrift** ist eine Zeile, die in Spalte 0 mit 1–3 `=` und danach Leerzeichen oder Tab
//!   beginnt. `====` und tiefer bleibt Text. Ein Kommentar dahinter (`== Titel // Notiz`) gehört
//!   nicht zum Titel.
//! - **Nicht als Gliederung** zählen Zeilen in Raw-Blöcken (```` ``` ```` am Zeilenanfang öffnet und
//!   schließt), in mehrzeiligem Inline-Raw (`` `…` `` über Zeilen), in Blockkommentaren `/* … */`, in
//!   mehrzeiligen Strings im Code (`#let x = "…"`) und in mehrzeiligen Inhaltsblöcken aus Code
//!   (`#block[…]`, `#f(x)[…]`, `#let x = [ … ]`, `#show …: it => [ … ]`). Dafür verfolgt der Scanner
//!   Klammern (`(`, `[`, `{`) ab einem `#`-Ausdruck; ein `[` im Fließtext der obersten Ebene bleibt Text.
//! - **Teile und „ohne Nummer“** (Runde 14, [`marker`]): die kanonischen Zeilen
//!   `#heading(level: 1, numbering: none)[T] <baruch-part>` (Teil, [`Heading::level`] 0) und
//!   `#heading(level: N, numbering: none)[T] <baruch-unnumbered>` (Ebene N, [`Heading::numbered`]
//!   `false`) zählen wie Überschriften; eine davon vor der ersten `=` beendet die Werkvorlage.
//! - **Bekannte Grenze:** eingerückte Überschriften und jedes andere `#heading(...)` werden nicht
//!   erkannt; sie bleiben Text (vor der ersten Überschrift also Teil der Werkvorlage). Ein `else [`
//!   am Anfang einer Folgezeile (`] else [`) gilt als Fließtext.
//! - Der **Text** einer Überschrift reicht von ihrer Zeile bis vor die nächste Überschrift, mit allen
//!   Zeilenenden. Darum gilt immer `join(&d.template, bodies) == source`, Byte für Byte.
//! - Titel und Autor stammen aus dem ersten `#set document(title: …, author: …)` außerhalb von
//!   Kommentaren (`//`, `/* … */`) und Raw-Text.
//! - **Maskierungen** (Vertrag Runde 4, Abschnitt 2): [`Heading::title`] ist der aufgelöste Titel
//!   (`\'` → `'`, `\#` → `#` …, [`unescape_title`]), [`Heading::raw_title`] der wörtliche aus der
//!   Zeile. Neue Überschriftszeilen baut [`heading_line`] und maskiert dabei mit [`escape_title`].
//! - **Nummerierung:** [`heading_numbering`] liest die letzte `#set heading(numbering: …)`-Regel der
//!   obersten Ebene der Werkvorlage (wie Typst) und gibt sie in der Mustersprache von [`scheme`]
//!   zurück (`I|1`, `I.;A.;1.`); [`format_number`] wendet ein Muster nach den Regeln von Typst an
//!   (`"I.1"` mit `[2, 3]` → `II.3`). Gliederungsschemata und der Export ihrer Regel: [`scheme`].
//! - Ein **UTF-8-BOM** (U+FEFF) am Dateianfang gehört zur Werkvorlage; eine Überschrift direkt danach
//!   zählt trotzdem (Windows-Editoren schreiben ihn oft). So bleibt die Rundreise byte-gleich.
//!
//! Zeilenenden werden nicht umgeschrieben; CRLF → LF macht der Aufrufer (der Store) vorher.
//! Die Crate kennt weder Datenbank noch Dateisystem.

pub mod cover;
pub mod cover_layout;
pub mod index;
pub mod markdown;
pub mod marker;
pub mod scheme;
pub mod timing;
pub mod typeset;
pub mod xref;

pub use markdown::{markdown_heading_line, markdown_headings};

pub use marker::{
    PART_LABEL, UNNUMBERED_LABEL, part_line, part_page_rule, relevel_line, set_numbered_line, unnumbered_line,
};

/// Ergebnis von [`parse`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Document {
    /// Alles vor der ersten Überschrift (leer, wenn die Datei mit einer Überschrift beginnt).
    pub template: String,
    /// `title` aus `#set document(...)`: String-Literal oder Rohtext eines Inhaltsblocks `[…]`.
    pub title: Option<String>,
    /// `author` aus `#set document(...)`: String-Literal oder Tupel von Strings (mit ", " verbunden).
    pub author: Option<String>,
    pub headings: Vec<Heading>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Heading {
    /// 0 = Teil (Markierung `<baruch-part>`, [`marker`]), 1 (`=`) bis 3 (`===`).
    pub level: u8,
    /// Titel mit aufgelösten Typst-Maskierungen ([`unescape_title`] von `raw_title`).
    pub title: String,
    /// Text nach den `=`, getrimmt, ohne angehängtes `<label>` und ohne Kommentar, wörtlich.
    pub raw_title: String,
    /// Von der Überschriftszeile bis vor die nächste Überschrift, inklusive aller Zeilenenden.
    pub body: String,
    /// `false` nur für die Markierung „ohne Nummer“ (`<baruch-unnumbered>`, [`marker`]); `=`-Zeilen
    /// und Teile sind `true`.
    pub numbered: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("keine Überschrift (= …) gefunden")]
    NoHeadings,
}

/// Zerlegt `source` in Werkvorlage und Überschriften (Regeln siehe Modulbeschreibung).
pub fn parse(source: &str) -> Result<Document, ParseError> {
    let mut scanner = Scanner::default();
    let mut starts: Vec<(usize, u8, String, bool)> = Vec::new();
    let bom = if source.starts_with(BOM) { BOM.len_utf8() } else { 0 };
    let mut offset = bom;
    for line in source[bom..].split_inclusive('\n') {
        if scanner.in_markup() {
            if let Some((level, title)) = read_heading(line) {
                starts.push((offset, level, title, true));
            } else if let Some(found) = marker::read_marker(line) {
                starts.push((offset, found.level, found.raw_title.to_owned(), found.level == 0));
            }
        }
        scanner.feed(line);
        offset += line.len();
    }
    let Some(&(first, ..)) = starts.first() else {
        return Err(ParseError::NoHeadings);
    };
    let mut headings = Vec::with_capacity(starts.len());
    for (i, (start, level, title, numbered)) in starts.iter().enumerate() {
        let end = starts.get(i + 1).map_or(source.len(), |next| next.0);
        headings.push(Heading {
            level: *level,
            title: unescape_title(title),
            raw_title: title.clone(),
            body: source[*start..end].to_owned(),
            numbered: *numbered,
        });
    }
    let (title, author) = document_meta(source);
    Ok(Document { template: source[..first].to_owned(), title, author, headings })
}

/// Byte-Order-Mark, den manche Editoren an den Dateianfang schreiben.
const BOM: char = '\u{feff}';

/// Werkvorlage und Texte hintereinander; Umkehrung von [`parse`].
pub fn join<'a>(template: &str, bodies: impl IntoIterator<Item = &'a str>) -> String {
    let mut out = template.to_owned();
    for body in bodies {
        out.push_str(body);
    }
    out
}

/// Wie [`join`], dazu je Text sein Bytebereich im Ergebnis (Runde 8, Typst-Vorschau: Meldungen
/// eines Typst-Laufs auf den Abschnitt zurückführen). Das Ergebnis ist byte-gleich mit [`join`].
pub fn join_with_offsets<'a>(
    template: &str,
    bodies: impl IntoIterator<Item = &'a str>,
) -> (String, Vec<std::ops::Range<usize>>) {
    let mut out = template.to_owned();
    let mut ranges = Vec::new();
    for body in bodies {
        let start = out.len();
        out.push_str(body);
        ranges.push(start..out.len());
    }
    (out, ranges)
}

/// Zeile (1-basiert) der Bytestelle `byte` in `text`, gezählt wie Typst (`Lines` in typst-syntax):
/// jedes [`is_newline`]-Zeichen beendet eine Zeile, `\r\n` zählt einmal. Stellen hinter dem Ende
/// zählen wie das Ende.
pub fn line_at(text: &str, byte: usize) -> usize {
    let mut line = 1;
    let mut chars = text.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        if i >= byte {
            break;
        }
        if is_newline(c) {
            if let Some(&(j, '\n')) = chars.peek().filter(|_| c == '\r') {
                if j >= byte {
                    break; // zwischen \r und \n: noch dieselbe Zeile
                }
                chars.next();
            }
            line += 1;
        }
    }
    line
}

/// Ebene und wörtlicher Titel, wenn `line` eine Überschrift ist (ohne Rücksicht auf Raw/Kommentar
/// davor). Ein Kommentar (`//`, `/*`) hinter der Überschrift gehört nicht zum Titel.
fn read_heading(line: &str) -> Option<(u8, String)> {
    let level = line.bytes().take_while(|&b| b == b'=').count();
    if !(1..=3).contains(&level) {
        return None;
    }
    let rest = &line[level..];
    if !rest.starts_with([' ', '\t']) {
        return None;
    }
    let rest = &rest[..comment_start(rest).unwrap_or(rest.len())];
    Some((level as u8, strip_label(rest.trim()).to_owned()))
}

/// Steht vor Stelle `i` von `text` ein Linkanfang (`http:` bzw. `https:`)? Dann ist `//` kein Kommentar.
fn after_scheme(text: &str, i: usize) -> bool {
    let before = &text[..i];
    before.ends_with("http:") || before.ends_with("https:")
}

/// Anfang eines Kommentars (`//` oder `/*`) in einer Markup-Zeile. Escapes (`\/`), Inline-Raw
/// (`` `//` ``) und Links (`https://…`) zählen nicht.
fn comment_start(text: &str) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => i += 2,
            b'`' => {
                let run = backticks(&text[i..]);
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

/// Entfernt ein angehängtes `<label>` am Ende des Titels.
fn strip_label(title: &str) -> &str {
    match label_start(title) {
        Some(open) => title[..open].trim_end(),
        None => title,
    }
}

/// Stelle des angehängten `<label>` am Ende von `title` (Zeichen wie in Typst), falls vorhanden.
fn label_start(title: &str) -> Option<usize> {
    let inner = title.strip_suffix('>')?;
    let open = inner.rfind('<')?;
    let label = &inner[open + 1..];
    let valid = !label.is_empty() && label.chars().all(|c| c.is_alphanumeric() || matches!(c, '_' | '-' | ':' | '.'));
    // Ein maskiertes `\<` öffnet kein Label.
    (valid && !inner[..open].ends_with('\\')).then_some(open)
}

/// Zeichen, deren Maskierung `\X` [`unescape_title`] auflöst (dieselbe Liste wie Migration 0006 von
/// baruch-store; `\\` wird zu `\`).
const UNESCAPED: &[char] = &['\'', '"', '#', '*', '_', '$', '@', '<', '>', '[', ']', '`', '~', '=', '-', '/', '+', '.'];

/// Zeichen, die [`escape_title`] maskiert: Markup-Zeichen von Typst und der Backslash selbst.
const ESCAPED: &[char] = &['\\', '#', '*', '_', '$', '@', '<', '>', '[', ']', '`', '~'];

/// Typst-Maskierungen eines Überschriftstitels auflösen: `\\` → `\`, `\X` → `X` für die Zeichen
/// `' " # * _ $ @ < > [ ] ` ~ = - / + .`; jeder andere Backslash bleibt stehen (`\n`, `\u{…}`).
/// Beispiel: `Anna\'s Laube` → `Anna's Laube`.
pub fn unescape_title(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\'
            && let Some(&next) = chars.peek()
            && (next == '\\' || UNESCAPED.contains(&next))
        {
            out.push(next);
            chars.next();
            continue;
        }
        out.push(c);
    }
    out
}

/// Zeilenende im Sinn von Typst (`is_newline` in typst-syntax): außer `\n` und `\r` auch `\x0B`,
/// `\x0C`, U+0085, U+2028 und U+2029. Dahinter beginnt eine neue Zeile, an deren Anfang `= `, `- `,
/// `+ `, `1. ` und `/ ` Struktur wären.
pub fn is_newline(c: char) -> bool {
    matches!(c, '\n' | '\r' | '\x0B' | '\x0C' | '\u{85}' | '\u{2028}' | '\u{2029}')
}

/// Titel für eine neue Überschriftszeile maskieren (Umkehrung von [`unescape_title`]): Backslash und
/// `# * _ $ @ < > [ ] ` ~` bekommen einen Backslash, ebenso ein `/` vor `/` oder `*` (sonst begänne
/// ein Kommentar). Zeilenenden ([`is_newline`]) werden zu Leerzeichen, die Ränder getrimmt.
pub fn escape_title(title: &str) -> String {
    let flat: String = title.chars().map(|c| if is_newline(c) { ' ' } else { c }).collect();
    let chars: Vec<char> = flat.trim().chars().collect();
    let mut out = String::with_capacity(flat.len() + 8);
    for (i, &c) in chars.iter().enumerate() {
        let comment = c == '/' && matches!(chars.get(i + 1), Some('/' | '*'));
        if ESCAPED.contains(&c) || comment {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// Überschriftszeile `"=" × level + " " + escape_title(title)` mit Zeilenende. Hatte die vorige Zeile
/// (`previous`) ein Label `<…>` oder einen Kommentar `// …`, bleiben beide erhalten.
///
/// Runde 14: `level` 0 ergibt die Teil-Markierung ([`part_line`]); ist `previous` eine Markierung
/// „ohne Nummer“ und `level` 1–3, bleibt es eine ([`unnumbered_line`]). In beiden Fällen bleibt ein
/// Kommentar der vorigen Zeile erhalten.
pub fn heading_line(level: u8, title: &str, previous: Option<&str>) -> String {
    if let Some(line) = marker::marker_heading_line(level, title, previous) {
        return line;
    }
    let mut line = format!("{} {}", "=".repeat(usize::from(level.max(1))), escape_title(title));
    if let Some(previous) = previous {
        let previous = previous.trim_end_matches(['\n', '\r']);
        let equals = previous.bytes().take_while(|&b| b == b'=').count();
        let rest = &previous[equals..];
        if equals > 0 && rest.starts_with([' ', '\t']) {
            let cut = comment_start(rest).unwrap_or(rest.len());
            let content = rest[..cut].trim();
            if let Some(open) = label_start(content) {
                line.push(' ');
                line.push_str(&content[open..]);
            }
            let comment = rest[cut..].trim_end();
            if !comment.is_empty() {
                line.push(' ');
                line.push_str(comment);
            }
        }
    }
    line.push('\n');
    line
}

/// Nummerierung der Werkvorlage in der Mustersprache von [`scheme`] (Vertrag Runde 6, Abschnitt 2b).
///
/// Es zählt nur `#set heading(numbering: …)` auf der **obersten Ebene** (nicht in `#show …`-Regeln,
/// Inhaltsblöcken oder Funktionsrümpfen), außerhalb von Kommentaren und Raw-Text, und davon die
/// **letzte** Regel mit `numbering` (wie in Typst). Ein String → `Some(muster)`, `none` → `Some("")`
/// (keine Nummern). Eine Funktion (`(..n) => …`, direkt oder über den Namen einer `#let`-Funktion)
/// wird je Ebene ausgewertet, wenn sie nur Tiefenbedingungen (`n.pos().len() == k`, `<= k`, `< k`,
/// auch über `let p = n.pos()`), Text und `numbering(…)` enthält; ein leerer Sonst-Zweig (`""`,
/// `none`, `[]` oder kein `else`) heißt „ohne Nummer“: `len() == 1` mit `numbering("I", ..n)` → `I|1`,
/// `<= k` → `P|k`, `< k` → `P|k-1`, `== k` (k > 1) → Liste nur mit Position k (`;1.1`). Sonst zählt
/// wie bisher das erste `numbering("MUSTER", …)` der Funktion für alle Ebenen. Ohne Regel oder bei
/// einem nicht auswertbaren Wert `None`.
pub fn heading_numbering(template: &str) -> Option<String> {
    const NEEDLE: &str = "#set heading(";
    let blanked = blank_comments(template);
    let tops = top_level_hashes(template);
    let mut found = None;
    for &rule in &tops {
        if !blanked[rule..].starts_with(NEEDLE) {
            continue;
        }
        let start = rule + NEEDLE.len();
        let mut cursor = Cursor { s: &blanked[start..], i: 0 };
        for (name, value, at) in cursor.arguments_at() {
            if name != "numbering" {
                continue;
            }
            found = Some(match value {
                Arg::Str(pattern) => Some(pattern),
                Arg::Other(text) if text.trim() == "none" => Some(String::new()),
                _ => {
                    let rest = &blanked[start + at..];
                    let expression = rest[..expression_end(rest)].trim();
                    function_numbering(&blanked, &tops, rule, expression)
                }
            });
        }
    }
    found.flatten()
}

/// Nummerierung aus einer Funktion in der Regel oder dem Namen einer `#let`-Funktion, die vor der
/// Regel (`rule`) auf der obersten Ebene definiert ist (die letzte solche Definition gilt).
fn function_numbering(blanked: &str, tops: &[usize], rule: usize, expression: &str) -> Option<String> {
    if let Some(spec) = scheme::read_function(expression) {
        return Some(spec);
    }
    let is_name = !expression.is_empty()
        && expression.bytes().all(ident_char)
        && expression.bytes().next().is_some_and(ident_start);
    if is_name {
        let needle = format!("#let {expression}");
        let definition = tops.iter().enumerate().rev().find(|&(_, &at)| {
            at < rule
                && blanked[at..].starts_with(&needle)
                && blanked[at + needle.len()..].trim_start_matches([' ', '\t']).starts_with(['(', '='])
        });
        if let Some((index, &at)) = definition {
            let end = tops.get(index + 1).copied().unwrap_or(blanked.len());
            let body = blanked[at + needle.len()..end].trim_start();
            let spec = match body.strip_prefix('=') {
                Some(value) => scheme::read_function(value.trim()),
                None => scheme::read_let_function(body),
            };
            if spec.is_some() {
                return spec;
            }
        }
    }
    numbering_call(expression).or_else(|| let_numbering(blanked, expression))
}

/// Stellen aller `#`, die auf der obersten Ebene im Markup stehen (nicht in Kommentaren, Raw-Text,
/// Strings, Inhaltsblöcken, Argumenten oder Codeblöcken), in Reihenfolge.
fn top_level_hashes(source: &str) -> Vec<usize> {
    let mut scanner = Scanner { record: true, ..Scanner::default() };
    let mut offset = 0;
    for line in source.split_inclusive('\n') {
        scanner.base = offset;
        scanner.feed(line);
        offset += line.len();
    }
    scanner.tops
}

/// Länge eines Ausdrucks bis zum Komma oder zur schließenden Klammer auf Ebene 0 (Strings übersprungen).
fn expression_end(s: &str) -> usize {
    let bytes = s.as_bytes();
    let mut depth = 0usize;
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' if depth == 0 => return i,
            b')' | b']' | b'}' => depth -= 1,
            b',' if depth == 0 => return i,
            b'"' => {
                // Ein String, der in der Zeile nicht schließt, zählt als gewöhnliches Zeichen.
                i += 1 + closing_quote(&bytes[i + 1..]).map_or(0, |len| len + 1);
                continue;
            }
            b'\\' => i += 1,
            _ => {}
        }
        i += 1;
    }
    bytes.len()
}

/// Erstes `numbering("MUSTER"` im Text (z. B. dem Rumpf einer Funktion), sonst `None`.
fn numbering_call(text: &str) -> Option<String> {
    const CALL: &str = "numbering(";
    let mut from = 0;
    while let Some(found) = text[from..].find(CALL) {
        let at = from + found;
        // Nur der Aufruf selbst, nicht `heading-numbering(` oder `my_numbering(`.
        let whole = text[..at].bytes().next_back().is_none_or(|b| !ident_char(b));
        let mut cursor = Cursor { s: &text[at + CALL.len()..], i: 0 };
        while cursor.peek().is_some_and(char::is_whitespace) {
            cursor.bump();
        }
        if whole && cursor.peek() == Some('"') {
            cursor.bump();
            return Some(cursor.string());
        }
        from = at + CALL.len();
    }
    None
}

/// `numbering("MUSTER"` im Rumpf von `#let name(…) = …` bzw. `#let name = …`; der Rumpf reicht bis zur
/// nächsten Zeile, die mit `#` beginnt (nächste Regel oder Definition), oder bis zum Ende.
fn let_numbering(blanked: &str, name: &str) -> Option<String> {
    if name.is_empty() || !name.bytes().all(ident_char) || !name.bytes().next().is_some_and(ident_start) {
        return None;
    }
    let needle = format!("#let {name}");
    let mut from = 0;
    while let Some(found) = blanked[from..].find(&needle) {
        let after = from + found + needle.len();
        let next = blanked[after..].trim_start_matches([' ', '\t']);
        if next.starts_with('(') || next.starts_with('=') {
            let body = &blanked[after..];
            let end = body.find("\n#").map_or(body.len(), |n| n + 1);
            return numbering_call(&body[..end]);
        }
        from = after;
    }
    None
}

/// Zählzeichen eines Nummerierungsmusters (wie in Typst).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Counter {
    Arabic,
    LowerLatin,
    UpperLatin,
    LowerRoman,
    UpperRoman,
    Symbol,
}

impl Counter {
    fn of(c: char) -> Option<Counter> {
        Some(match c {
            '1' => Counter::Arabic,
            'a' => Counter::LowerLatin,
            'A' => Counter::UpperLatin,
            'i' => Counter::LowerRoman,
            'I' => Counter::UpperRoman,
            '*' => Counter::Symbol,
            _ => return None,
        })
    }

    fn apply(self, n: usize) -> String {
        match self {
            Counter::Arabic => n.to_string(),
            Counter::LowerLatin => latin(n),
            Counter::UpperLatin => latin(n).to_uppercase(),
            Counter::LowerRoman => roman(n).to_lowercase(),
            Counter::UpperRoman => roman(n),
            Counter::Symbol => {
                const SYMBOLS: [&str; 6] = ["*", "†", "‡", "§", "¶", "‖"];
                if n == 0 {
                    return "-".to_owned();
                }
                SYMBOLS[(n - 1) % SYMBOLS.len()].repeat((n - 1) / SYMBOLS.len() + 1)
            }
        }
    }
}

/// a, b, …, z, aa, ab … (bijektiv zur Basis 26); 0 → "-".
fn latin(mut n: usize) -> String {
    if n == 0 {
        return "-".to_owned();
    }
    let mut letters = Vec::new();
    while n > 0 {
        n -= 1;
        letters.push(char::from(b'a' + (n % 26) as u8));
        n /= 26;
    }
    letters.iter().rev().collect()
}

/// Römische Zahl in Großbuchstaben; 0 → "N" (wie Typst).
fn roman(mut n: usize) -> String {
    const TABLE: [(usize, &str); 13] = [
        (1000, "M"),
        (900, "CM"),
        (500, "D"),
        (400, "CD"),
        (100, "C"),
        (90, "XC"),
        (50, "L"),
        (40, "XL"),
        (10, "X"),
        (9, "IX"),
        (5, "V"),
        (4, "IV"),
        (1, "I"),
    ];
    if n == 0 {
        return "N".to_owned();
    }
    let mut out = String::new();
    for (value, letters) in TABLE {
        while n >= value {
            out.push_str(letters);
            n -= value;
        }
    }
    out
}

/// Zählzeichen mit ihrem Präfix und der Rest des Musters (Suffix); `None` ohne Zählzeichen.
type Pieces = (Vec<(String, Counter)>, String);

fn pattern_pieces(pattern: &str) -> Option<Pieces> {
    let mut pieces = Vec::new();
    let mut text = String::new();
    for c in pattern.chars() {
        match Counter::of(c) {
            Some(counter) => pieces.push((std::mem::take(&mut text), counter)),
            None => text.push(c),
        }
    }
    (!pieces.is_empty()).then_some((pieces, text))
}

/// Hat `pattern` mindestens ein Zählzeichen (`1 a A i I *`)? Sonst ist es kein Nummerierungsmuster.
pub fn is_numbering_pattern(pattern: &str) -> bool {
    pattern_pieces(pattern).is_some()
}

/// Nummer nach einem Typst-Muster: Je Zahl in `counts` das nächste Zählzeichen mit seinem Präfix; gibt
/// es mehr Zahlen als Zählzeichen, wiederholt sich das letzte (mit seinem Präfix, bei leerem Präfix
/// mit dem Suffix als Trenner); am Ende steht das Suffix. `format_number("I.1", &[2, 3])` = `II.3`,
/// `format_number("1.", &[2])` = `2.`, `format_number("1.1", &[1, 2, 3])` = `1.2.3`. Ein Muster
/// ohne Zählzeichen oder leere `counts` → leer.
pub fn format_number(pattern: &str, counts: &[usize]) -> String {
    let Some((pieces, suffix)) = pattern_pieces(pattern) else { return String::new() };
    if counts.is_empty() {
        return String::new();
    }
    let mut out = String::new();
    let mut numbers = counts.iter();
    for ((prefix, counter), &n) in pieces.iter().zip(&mut numbers) {
        out.push_str(prefix);
        out.push_str(&counter.apply(n));
    }
    if let Some((last_prefix, last_counter)) = pieces.last() {
        for &n in numbers {
            out.push_str(if last_prefix.is_empty() { &suffix } else { last_prefix });
            out.push_str(&last_counter.apply(n));
        }
    }
    out.push_str(&suffix);
    out
}

/// Offene Klammer, die über Zeilen reichen kann.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Open {
    /// Inhaltsblock `[…]` aus Code (`#block[`, `#f(x)[`, `[` in Argumenten): drinnen ist Markup, aber
    /// keine Gliederung. `call`: direkt danach darf ein weiterer `[…]` folgen (`#grid[a][b]`).
    Content { call: bool },
    /// Eckige Klammer im Markup eines Inhaltsblocks (Typst zählt sie mit, damit `]` richtig schließt).
    Bracket,
    /// Argumentliste bzw. Klammerausdruck im Code; `call` wie oben (`#f(x)[…]`).
    Paren { call: bool },
    /// Codeblock `{…}`.
    Brace,
}

/// Zustand über Zeilen hinweg. Eine Zeile kann nur dann eine Überschrift sein, wenn sie im Markup
/// der obersten Ebene beginnt, also nichts davon offen ist.
#[derive(Default)]
struct Scanner {
    /// Raw-Block mit ```` ``` ```` am Zeilenanfang: Länge des Zauns, geschlossen am Zeilenanfang.
    raw_fence: Option<usize>,
    /// Inline-Raw, das über das Zeilenende reicht: Länge der Backtick-Folge.
    raw_inline: Option<usize>,
    /// Tiefe der Blockkommentare (Typst schachtelt sie).
    comment_depth: u32,
    /// String im Code, der über das Zeilenende reicht.
    in_string: bool,
    /// Offene Klammern, innerste zuletzt.
    stack: Vec<Open>,
    /// Stellen der `#` auf oberster Ebene merken ([`top_level_hashes`]); `base` = Anfang der Zeile.
    record: bool,
    base: usize,
    tops: Vec<usize>,
}

/// Länge der Backtick-Folge am Anfang von `s`.
fn backticks(s: &str) -> usize {
    s.bytes().take_while(|&b| b == b'`').count()
}

/// Schlüsselwörter, nach denen der Rest der Zeile Code ist (`#let x = "…"`, `#show …: it => [`).
const LINE_CODE: &[&str] = &["let", "set", "show", "import", "include", "if", "else", "for", "while", "return"];

fn ident_start(b: u8) -> bool {
    b.is_ascii_alphabetic() || b == b'_' || b >= 0x80
}

fn ident_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'-' || b >= 0x80
}

impl Scanner {
    /// Steht der Anfang der nächsten Zeile im Markup der obersten Ebene?
    fn in_markup(&self) -> bool {
        self.raw_fence.is_none()
            && self.raw_inline.is_none()
            && self.comment_depth == 0
            && !self.in_string
            && self.stack.is_empty()
    }

    /// Ist an der aktuellen Stelle Code (nicht Markup)?
    fn in_code(&self, line_code: bool) -> bool {
        match self.stack.last() {
            Some(Open::Paren { .. } | Open::Brace) => true,
            Some(Open::Content { .. } | Open::Bracket) => false,
            None => line_code,
        }
    }

    fn feed(&mut self, line: &str) {
        if let Some(fence) = self.raw_fence {
            if backticks(line.trim_start()) == fence {
                self.raw_fence = None;
            }
            return;
        }
        // Byteweise: Alle Zeichen, auf die es ankommt, sind ASCII; UTF-8-Folgebytes passen auf keines.
        let bytes = line.as_bytes();
        let indent = line.len() - line.trim_start().len();
        // Rest der Zeile ist Code nach `#let`, `#set`, `#show` … (endet mit der Zeile).
        let mut line_code = false;
        let mut i = 0;
        while i < bytes.len() {
            let rest = &bytes[i..];
            if self.comment_depth > 0 {
                if rest.starts_with(b"/*") {
                    self.comment_depth += 1;
                    i += 2;
                } else if rest.starts_with(b"*/") {
                    self.comment_depth -= 1;
                    i += 2;
                } else {
                    i += 1;
                }
                continue;
            }
            if let Some(run) = self.raw_inline {
                let found = backticks(&line[i..]);
                if found == run {
                    self.raw_inline = None;
                }
                i += found.max(1);
                continue;
            }
            if self.in_string {
                match bytes[i] {
                    b'\\' => i += 2,
                    b'"' => {
                        self.in_string = false;
                        i += 1;
                    }
                    _ => i += 1,
                }
                continue;
            }
            let code = self.in_code(line_code);
            match bytes[i] {
                b'`' => {
                    let run = backticks(&line[i..]);
                    let close = rest[run..].windows(run).position(|w| w.iter().all(|&b| b == b'`'));
                    match close {
                        // Inline-Raw oder Raw-Block in einer Zeile: überspringen.
                        Some(close) => i += run + close + run,
                        // Offener Zaun am Zeilenanfang: Raw-Block bis zur schließenden Zeile.
                        None if run >= 3 && i == indent => {
                            self.raw_fence = Some(run);
                            return;
                        }
                        // Sonst Inline-Raw über das Zeilenende hinaus.
                        None => {
                            self.raw_inline = Some(run);
                            i += run;
                        }
                    }
                }
                b'\\' => i += 2,
                b'/' if rest.starts_with(b"//") && !after_scheme(line, i) => break,
                b'/' if rest.starts_with(b"/*") => {
                    self.comment_depth = 1;
                    i += 2;
                }
                b'"' if code => {
                    self.in_string = true;
                    i += 1;
                }
                b'"' => {
                    // Im Markup gilt nur ein in derselben Zeile geschlossener String als String; ein
                    // einzelnes Anführungszeichen im Text bleibt Text. So öffnet "src/*.typ" keinen Kommentar.
                    match closing_quote(&rest[1..]) {
                        Some(len) => i += 1 + len + 1,
                        None => i += 1,
                    }
                }
                b'#' if !code => {
                    if self.record && self.stack.is_empty() {
                        self.tops.push(self.base + i);
                    }
                    i += 1;
                    if self.hash_expression(line, &mut i) {
                        line_code = true;
                    }
                }
                b'[' if code => {
                    self.stack.push(Open::Content { call: false });
                    i += 1;
                }
                // `[` im Markup der obersten Ebene ist Text; in einem Inhaltsblock wird es mitgezählt.
                b'[' if !self.stack.is_empty() => {
                    self.stack.push(Open::Bracket);
                    i += 1;
                }
                b']' if !code => {
                    i += 1;
                    if let Some(Open::Content { call: true }) = self.stack.pop() {
                        self.trailing_content(bytes, &mut i);
                    }
                }
                b'(' if code => {
                    self.stack.push(Open::Paren { call: false });
                    i += 1;
                }
                b'{' if code => {
                    self.stack.push(Open::Brace);
                    i += 1;
                }
                b')' | b'}' if code => {
                    i += 1;
                    if let Some(Open::Paren { call: true }) = self.stack.pop() {
                        self.trailing_content(bytes, &mut i);
                    }
                }
                _ => i += 1,
            }
        }
    }

    /// Nach `#` im Markup (`i` steht dahinter): Ausdruck lesen und offene Klammern merken.
    /// `true`, wenn ein Schlüsselwort den Rest der Zeile zu Code macht.
    fn hash_expression(&mut self, line: &str, i: &mut usize) -> bool {
        let bytes = line.as_bytes();
        match bytes.get(*i) {
            Some(b'(') => {
                self.stack.push(Open::Paren { call: true });
                *i += 1;
                return false;
            }
            Some(b'{') => {
                self.stack.push(Open::Brace);
                *i += 1;
                return false;
            }
            Some(b'[') => {
                self.stack.push(Open::Content { call: true });
                *i += 1;
                return false;
            }
            Some(&b) if ident_start(b) => {}
            _ => return false,
        }
        let start = *i;
        while bytes.get(*i).is_some_and(|&b| ident_char(b)) {
            *i += 1;
        }
        if LINE_CODE.contains(&&line[start..*i]) {
            return true;
        }
        loop {
            match bytes.get(*i) {
                Some(b'.') if bytes.get(*i + 1).is_some_and(|&b| ident_start(b)) => {
                    *i += 1;
                    while bytes.get(*i).is_some_and(|&b| ident_char(b)) {
                        *i += 1;
                    }
                }
                Some(b'(') => {
                    self.stack.push(Open::Paren { call: true });
                    *i += 1;
                    return false;
                }
                Some(b'[') => {
                    self.stack.push(Open::Content { call: true });
                    *i += 1;
                    return false;
                }
                _ => return false,
            }
        }
    }

    /// Nach `)` oder `]` eines Aufrufs: ein direkt folgendes `[` öffnet einen weiteren Inhaltsblock.
    fn trailing_content(&mut self, bytes: &[u8], i: &mut usize) {
        if bytes.get(*i) == Some(&b'[') {
            self.stack.push(Open::Content { call: true });
            *i += 1;
        }
    }
}

/// Länge des String-Inhalts bis zum schließenden `"` (mit `\`-Escapes), falls in `bytes` vorhanden.
fn closing_quote(bytes: &[u8]) -> Option<usize> {
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => i += 2,
            b'"' => return Some(i),
            b'\n' => return None,
            _ => i += 1,
        }
    }
    None
}

/// Wert eines Arguments in `#set document(...)`.
enum Arg {
    Str(String),
    Content(String),
    Tuple(Vec<Arg>),
    /// Anderer Ausdruck, roh (z. B. `none`, eine Variable, ein Aufruf).
    Other(String),
}

/// `source` mit Leerzeichen statt Kommentaren (`//` bis Zeilenende, `/* … */` geschachtelt) und
/// Raw-Text (`` `…` ``, ```` ```…``` ````); Zeilenenden und alle Positionen bleiben. Strings, die in
/// derselben Zeile schließen, bleiben unberührt (`"src/*.typ"`, `"https://…"`).
fn blank_comments(source: &str) -> String {
    let bytes = source.as_bytes();
    let mut out = bytes.to_vec();
    let mut blank = |from: usize, to: usize| {
        for b in &mut out[from..to] {
            if *b != b'\n' {
                *b = b' ';
            }
        }
    };
    let mut i = 0;
    while i < bytes.len() {
        let rest = &bytes[i..];
        match bytes[i] {
            b'\\' => i += 2,
            b'"' => match closing_quote(&rest[1..]) {
                Some(len) => i += 1 + len + 1,
                None => i += 1,
            },
            b'`' => {
                let run = backticks(&source[i..]);
                let end = rest[run..]
                    .windows(run)
                    .position(|w| w.iter().all(|&b| b == b'`'))
                    .map_or(bytes.len(), |close| i + run + close + run);
                blank(i, end);
                i = end;
            }
            b'/' if rest.starts_with(b"//") && !after_scheme(source, i) => {
                let end = rest.iter().position(|&b| b == b'\n').map_or(bytes.len(), |n| i + n);
                blank(i, end);
                i = end;
            }
            b'/' if rest.starts_with(b"/*") => {
                let mut depth = 0u32;
                let mut j = i;
                while j < bytes.len() {
                    if bytes[j..].starts_with(b"/*") {
                        depth += 1;
                        j += 2;
                    } else if bytes[j..].starts_with(b"*/") {
                        depth -= 1;
                        j += 2;
                        if depth == 0 {
                            break;
                        }
                    } else {
                        j += 1;
                    }
                }
                let end = j.min(bytes.len());
                blank(i, end);
                i = end;
            }
            _ => i += 1,
        }
    }
    // Ersetzt wurden nur ganze Bereiche zwischen ASCII-Zeichen, das Ergebnis ist also UTF-8.
    String::from_utf8(out).unwrap_or_else(|_| source.to_owned())
}

/// `title` und `author` aus dem ersten `#set document(...)` außerhalb von Kommentaren und Raw-Text.
fn document_meta(source: &str) -> (Option<String>, Option<String>) {
    const NEEDLE: &str = "#set document(";
    let source = blank_comments(source);
    let source = source.as_str();
    if let Some(found) = source.find(NEEDLE) {
        let from = found + NEEDLE.len();
        let mut cursor = Cursor { s: &source[from..], i: 0 };
        let args = cursor.arguments();
        let mut title = None;
        let mut author = None;
        for (name, value) in args {
            match (name.as_str(), value) {
                ("title", Arg::Str(s) | Arg::Content(s)) => title = Some(s),
                ("author", Arg::Str(s)) => author = Some(s),
                ("author", Arg::Tuple(items)) => {
                    let names: Vec<String> =
                        items.into_iter().filter_map(|a| if let Arg::Str(s) = a { Some(s) } else { None }).collect();
                    author = Some(names.join(", "));
                }
                _ => {}
            }
        }
        let title = title.map(|t| t.trim().to_owned()).filter(|t| !t.is_empty());
        let author = author.map(|a| a.trim().to_owned()).filter(|a| !a.is_empty());
        return (title, author);
    }
    (None, None)
}

/// Kleiner Leser für die Argumentliste von `#set document(` (nach der öffnenden Klammer).
struct Cursor<'a> {
    s: &'a str,
    i: usize,
}

impl Cursor<'_> {
    fn peek(&self) -> Option<char> {
        self.s[self.i..].chars().next()
    }

    fn bump(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.i += c.len_utf8();
        Some(c)
    }

    fn skip_space(&mut self) {
        while self.peek().is_some_and(|c| c.is_whitespace() || c == ',') {
            self.bump();
        }
    }

    /// `name: wert, …` bis zur schließenden `)`.
    fn arguments(&mut self) -> Vec<(String, Arg)> {
        self.arguments_at().into_iter().map(|(name, value, _)| (name, value)).collect()
    }

    /// Wie [`Cursor::arguments`], dazu je Wert die Stelle seines Anfangs in `s`.
    fn arguments_at(&mut self) -> Vec<(String, Arg, usize)> {
        let mut args = Vec::new();
        loop {
            self.skip_space();
            match self.peek() {
                None | Some(')') => return args,
                _ => {}
            }
            let start = self.i;
            while self.peek().is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '-') {
                self.bump();
            }
            let name = self.s[start..self.i].to_owned();
            while self.peek().is_some_and(char::is_whitespace) {
                self.bump();
            }
            if name.is_empty() || self.peek() != Some(':') {
                // Positionsargument oder Unlesbares: bis zum nächsten Komma überspringen.
                self.value();
                if self.i == start {
                    self.bump();
                }
                continue;
            }
            self.bump();
            while self.peek().is_some_and(char::is_whitespace) {
                self.bump();
            }
            let at = self.i;
            let value = self.value();
            args.push((name, value, at));
        }
    }

    fn value(&mut self) -> Arg {
        match self.peek() {
            Some('"') => {
                self.bump();
                Arg::Str(self.string())
            }
            Some('[') => {
                self.bump();
                Arg::Content(self.content())
            }
            Some('(') => {
                self.bump();
                let mut items = Vec::new();
                loop {
                    self.skip_space();
                    match self.peek() {
                        None => break,
                        Some(')') => {
                            self.bump();
                            break;
                        }
                        _ => {
                            let before = self.i;
                            items.push(self.value());
                            if self.i == before {
                                self.bump();
                            }
                        }
                    }
                }
                Arg::Tuple(items)
            }
            _ => {
                // Ausdruck (Variable, Aufruf …): bis zum Komma oder zur Klammer auf Ebene 0.
                let start = self.i;
                let mut depth = 0usize;
                while let Some(c) = self.peek() {
                    match c {
                        '(' | '[' | '{' => depth += 1,
                        ')' | ']' | '}' if depth == 0 => break,
                        ')' | ']' | '}' => depth -= 1,
                        ',' if depth == 0 => break,
                        '"' => {
                            self.bump();
                            self.string();
                            continue;
                        }
                        _ => {}
                    }
                    self.bump();
                }
                Arg::Other(self.s[start..self.i].to_owned())
            }
        }
    }

    /// Rest eines String-Literals nach dem öffnenden `"`, Escapes aufgelöst.
    fn string(&mut self) -> String {
        let mut out = String::new();
        while let Some(c) = self.bump() {
            match c {
                '"' => break,
                '\\' => match self.bump() {
                    Some('n') => out.push('\n'),
                    Some('r') => out.push('\r'),
                    Some('t') => out.push('\t'),
                    Some('u') if self.peek() == Some('{') => {
                        self.bump();
                        let start = self.i;
                        while self.peek().is_some_and(|c| c != '}') {
                            self.bump();
                        }
                        let hex = &self.s[start..self.i];
                        self.bump();
                        if let Some(c) = u32::from_str_radix(hex, 16).ok().and_then(char::from_u32) {
                            out.push(c);
                        }
                    }
                    Some(other) => out.push(other),
                    None => break,
                },
                c => out.push(c),
            }
        }
        out
    }

    /// Rohtext eines Inhaltsblocks nach dem öffnenden `[` bis zur passenden `]`.
    fn content(&mut self) -> String {
        let start = self.i;
        let mut depth = 0usize;
        while let Some(c) = self.peek() {
            match c {
                '\\' => {
                    self.bump();
                }
                '[' => depth += 1,
                ']' if depth == 0 => break,
                ']' => depth -= 1,
                _ => {}
            }
            self.bump();
        }
        let text = self.s[start..self.i].to_owned();
        self.bump();
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heading_lines_need_one_to_three_equals_and_a_space() {
        assert_eq!(read_heading("= Titel\n"), Some((1, "Titel".into())), "einfache Überschrift");
        assert_eq!(read_heading("===\tDrei <sec:drei>\n"), Some((3, "Drei".into())), "Tab erlaubt, Label fällt weg");
        assert_eq!(read_heading("==== Vier\n"), None, "vierte Ebene bleibt Text");
        assert_eq!(read_heading("=Ohne\n"), None, "ohne Leerzeichen keine Überschrift");
        assert_eq!(read_heading(" = Eingerückt\n"), None, "eingerückt ist keine Überschrift");
        assert_eq!(read_heading("= a < b\n"), Some((1, "a < b".into())), "kein Label, bleibt im Titel");
    }

    #[test]
    fn a_comment_behind_a_heading_is_not_part_of_the_title() {
        assert_eq!(read_heading("== Titel // Kommentar\n"), Some((2, "Titel".into())), "// beendet den Titel");
        assert_eq!(
            read_heading("= Titel <t> // noch\n"),
            Some((1, "Titel".into())),
            "Label vor dem Kommentar fällt weg"
        );
        assert_eq!(read_heading("= A /* B */\n"), Some((1, "A".into())), "Blockkommentar gehört nicht zum Titel");
        assert_eq!(
            read_heading("= Siehe https://example.org/x\n"),
            Some((1, "Siehe https://example.org/x".into())),
            "ein Link ist kein Kommentar"
        );
        assert_eq!(read_heading("= Der `//`-Strich\n"), Some((1, "Der `//`-Strich".into())), "// in Raw bleibt");
        assert_eq!(read_heading("= a \\// b\n"), Some((1, "a \\// b".into())), "maskiertes / beginnt keinen Kommentar");
    }

    #[test]
    fn document_meta_reads_strings_tuples_and_content_blocks() {
        let (t, a) = document_meta("#set document(title: [Die *Probe*], author: (\"A. Muster\", \"B. Beispiel\"))\n");
        assert_eq!(t.as_deref(), Some("Die *Probe*"), "Titel aus dem Inhaltsblock, roh");
        assert_eq!(a.as_deref(), Some("A. Muster, B. Beispiel"), "Autoren aus dem Tupel");
        let (t, a) = document_meta(
            "// #set document(title: \"alt\")\n#set document(\n  date: auto,\n  title: \"Neu \\\"x\\\"\",\n)\n",
        );
        assert_eq!(t.as_deref(), Some("Neu \"x\""), "auskommentierte Zeile zählt nicht");
        assert_eq!(a, None, "ohne author kein Autor");
    }

    #[test]
    fn document_meta_skips_block_comments_and_trailing_line_comments() {
        let (t, _) = document_meta("/*\n#set document(title: \"alt\")\n*/\n#set document(title: \"neu\")\n");
        assert_eq!(t.as_deref(), Some("neu"), "#set document in /* … */ zählt nicht");
        let (t, _) =
            document_meta("/* a /* b */\n#set document(title: \"innen\")\n*/\n#set document(title: \"außen\")\n");
        assert_eq!(t.as_deref(), Some("außen"), "Blockkommentare schachteln wie in Typst");
        let (t, _) = document_meta("#set text(lang: \"de\") // #set document(title: \"hinten\")\n= A\n");
        assert_eq!(t, None, "ein Zeilenkommentar hinter Code zählt nicht");
        let (t, _) = document_meta("#let p = \"a/*b\"\n#set document(title: \"echt\")\n");
        assert_eq!(t.as_deref(), Some("echt"), "/* in einem String öffnet keinen Kommentar");
        let (t, a) = document_meta("#set document(\n  title: \"T\", // Arbeitstitel\n  author: \"A\",\n)\n");
        assert_eq!((t.as_deref(), a.as_deref()), (Some("T"), Some("A")), "Kommentar in der Argumentliste stört nicht");
    }

    #[test]
    fn blank_comments_keeps_positions_and_line_ends() {
        let source = "a // ä\nb /* ö\n*/ c\n";
        let blanked = blank_comments(source);
        assert_eq!(blanked.len(), source.len(), "gleiche Länge, damit Positionen stimmen");
        assert_eq!(blanked.lines().count(), source.lines().count(), "Zeilenenden bleiben");
        assert!(!blanked.contains('ä') && !blanked.contains('ö'), "Kommentartext ist weg: {blanked:?}");
        assert!(blanked.starts_with("a ") && blanked.ends_with(" c\n"), "Code bleibt: {blanked:?}");
    }

    #[test]
    fn strings_with_slash_star_do_not_open_a_comment() {
        let d = parse("#let p = \"src/*.typ\"\n= Eins\n").unwrap();
        assert_eq!(d.headings.len(), 1, "String darf keinen Kommentar öffnen: {d:?}");
    }
}

pub mod images;
