// SPDX-License-Identifier: AGPL-3.0-or-later
//! Sprechdauer eines Abschnittstexts (Runde 16b „Zeitwächter“, Schnittstelle R16-S1).
//!
//! [`spoken_words`] zählt die gesprochenen Wörter eines Typst-Texts, [`speaking_seconds`] rechnet sie
//! mit Wörtern je Minute in Sekunden um, [`clock`] schreibt Sekunden als Uhrzeit. Zählregeln (so auch in
//! `docs/Bedienung.md`, Abschnitt „Zeitwächter“):
//!
//! 1. CRLF → LF.
//! 2. Kommentare zählen nicht: `/* … */` (auch mehrzeilig, verschachtelt) und `//` bis Zeilenende, wenn
//!    `//` am Zeilenanfang oder nach Leerraum steht (`https://` bleibt Text).
//! 3. Überschriftszeilen (nach Leerraum `=`…`=` + Leerzeichen, auch die Teil-Markierungen
//!    `#heading(…)[…] <baruch-part>`) zählen nicht.
//! 4. Raw `` `…` `` und ```` ```…``` ```` zählen nicht; Mathe `$…$` zählt nicht.
//! 5. Code: `#name`, `#name.feld` samt direkt folgenden Argumentgruppen `(…)` (geklammert, Zeichenketten
//!    beachtet) fällt weg; direkt folgende Inhaltsblöcke `[…]` zählen – außer bei `#footnote` und
//!    `#arbeit` (nicht gesprochen). Zeilen mit `#set`, `#show`, `#let`, `#import`, `#include` fallen ganz
//!    weg (samt Klammern, die über die Zeile hinausreichen).
//! 6. Labels `<…>` und Verweise `@name` fallen weg.
//! 7. Markup-Zeichen `*` `_` fallen weg, `~` wird Leerzeichen, Listenzeichen `- `, `+ `, `/ ` am
//!    Zeilenanfang fallen weg, Maskierung `\x` → `x`, `\` als Zeilenumbruch → Leerzeichen.
//! 8. Wort = Folge von Nicht-Leerraum mit mindestens einem Buchstaben oder einer Ziffer
//!    (`char::is_alphanumeric`); „Joh 3,16“ = 2 Wörter, ein Bindestrichwort = 1.
//!
//! Die Crate rechnet nur; Einstellungen (Wörter je Minute, Zielzeit) hält der Store.

/// Vorgabe: Wörter je Minute beim freien Vortrag.
pub const DEFAULT_WORDS_PER_MINUTE: u32 = 110;
/// Kleinste erlaubte Sprechgeschwindigkeit.
pub const MIN_WORDS_PER_MINUTE: u32 = 60;
/// Größte erlaubte Sprechgeschwindigkeit.
pub const MAX_WORDS_PER_MINUTE: u32 = 250;
/// Längste Zielzeit in Minuten.
pub const MAX_TARGET_MINUTES: u32 = 240;

/// Gesprochene Wörter eines Abschnittstexts (Typst) nach den Zählregeln unten.
///
/// Zählregeln: siehe Moduldokumentation (1)–(8).
pub fn spoken_words(typst: &str) -> usize {
    let text = typst.replace("\r\n", "\n");
    spoken_text(&text).split_whitespace().filter(|w| w.chars().any(char::is_alphanumeric)).count()
}

/// Sekunden für `words` Wörter: aufgerundet ceil(words*60/wpm); wpm auf [MIN, MAX] geklemmt; 0 Wörter → 0.
pub fn speaking_seconds(words: usize, words_per_minute: u32) -> u32 {
    if words == 0 {
        return 0;
    }
    let wpm = u64::from(words_per_minute.clamp(MIN_WORDS_PER_MINUTE, MAX_WORDS_PER_MINUTE));
    let seconds = (words as u64).saturating_mul(60).div_ceil(wpm);
    u32::try_from(seconds).unwrap_or(u32::MAX)
}

/// Uhr-Schreibweise: unter einer Stunde „m:ss“ („0:45“, „12:05“), sonst „h:mm:ss“ („1:02:03“).
pub fn clock(seconds: u32) -> String {
    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;
    let secs = seconds % 60;
    if hours == 0 { format!("{minutes}:{secs:02}") } else { format!("{hours}:{minutes:02}:{secs:02}") }
}

fn ident_start(ch: char) -> bool {
    ch.is_alphabetic() || ch == '_'
}

fn ident_char(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_' || ch == '-'
}

/// Zeichenkette `"…"` ab `i` (auf dem `"`) überspringen; Index hinter dem schließenden `"`.
fn skip_string(c: &[char], mut i: usize) -> usize {
    i += 1;
    while i < c.len() {
        match c[i] {
            '\\' => i += 2,
            '"' => return i + 1,
            _ => i += 1,
        }
    }
    c.len()
}

/// Bis zum nächsten `close` überspringen, ab `i` (hinter dem Öffner); mit `escapes` beendet `\x` nicht.
fn skip_until(c: &[char], mut i: usize, close: &[char], escapes: bool) -> usize {
    while i < c.len() {
        if escapes && c[i] == '\\' {
            i += 2;
            continue;
        }
        if c[i..].starts_with(close) {
            return i + close.len();
        }
        i += 1;
    }
    c.len()
}

/// Verschachtelten Blockkommentar ab `i` (auf `/*`) überspringen; Index dahinter.
fn skip_block_comment(c: &[char], mut i: usize) -> usize {
    let mut depth = 0usize;
    while i < c.len() {
        if c[i..].starts_with(&['/', '*']) {
            depth += 1;
            i += 2;
        } else if c[i..].starts_with(&['*', '/']) {
            depth = depth.saturating_sub(1);
            i += 2;
            if depth == 0 {
                return i;
            }
        } else {
            i += 1;
        }
    }
    c.len()
}

/// Geklammerte Gruppe ab `i` (auf `(`, `[` oder `{`) überspringen; Index hinter der schließenden Klammer.
/// In `(…)` und `{…}` gilt Code (Zeichenketten zählen als Ganzes), in `[…]` Markup (Maskierungen, Raw).
fn skip_group(c: &[char], mut i: usize) -> usize {
    let open = c[i];
    let close = match open {
        '(' => ')',
        '[' => ']',
        _ => '}',
    };
    let code = open != '[';
    i += 1;
    while i < c.len() {
        let ch = c[i];
        if ch == close {
            return i + 1;
        }
        match ch {
            '(' | '{' if code => i = skip_group(c, i),
            '[' => i = skip_group(c, i),
            '"' if code => i = skip_string(c, i),
            '\\' if !code => i += 2,
            '`' if !code => {
                i = if c[i..].starts_with(&['`', '`', '`']) {
                    skip_until(c, i + 3, &['`', '`', '`'], false)
                } else {
                    skip_until(c, i + 1, &['`'], false)
                }
            }
            '#' if !code && c.get(i + 1).is_some_and(|&n| ident_start(n)) => {
                i += 1;
                while i < c.len()
                    && (ident_char(c[i]) || (c[i] == '.' && c.get(i + 1).is_some_and(|&n| ident_start(n))))
                {
                    i += 1;
                }
                if c.get(i).is_some_and(|&n| n == '(' || n == '{') {
                    i = skip_group(c, i);
                }
            }
            '/' if c[i..].starts_with(&['/', '*']) => i = skip_block_comment(c, i),
            _ => i += 1,
        }
    }
    c.len()
}

/// Rest einer Code-Zeile ab `i` überspringen (bis zum Zeilenende außerhalb von Klammern und
/// Zeichenketten); Index auf dem `\n` bzw. am Ende.
fn skip_code_line(c: &[char], mut i: usize) -> usize {
    while i < c.len() && c[i] != '\n' {
        match c[i] {
            '(' | '[' | '{' => i = skip_group(c, i),
            '"' => i = skip_string(c, i),
            '/' if c[i..].starts_with(&['/', '*']) => i = skip_block_comment(c, i),
            _ => i += 1,
        }
    }
    i
}

const LINE_KEYWORDS: [&str; 6] = ["set", "show", "let", "import", "include", "heading"];

/// Beginnt bei `j` ein `#`-Schlüsselwort, dessen Zeile ganz wegfällt?
fn line_keyword_at(c: &[char], j: usize) -> bool {
    if c.get(j) != Some(&'#') {
        return false;
    }
    let mut k = j + 1;
    while k < c.len() && ident_char(c[k]) {
        k += 1;
    }
    let word: String = c[j + 1..k].iter().collect();
    LINE_KEYWORDS.contains(&word.as_str())
}

/// Überschriftszeile ab `j` (erstes Nicht-Leerraumzeichen): `=`…`=` + Leerzeichen/Tab/Zeilenende.
fn heading_at(c: &[char], j: usize) -> bool {
    let mut k = j;
    while k < c.len() && c[k] == '=' {
        k += 1;
    }
    k > j && (k == c.len() || matches!(c[k], ' ' | '\t' | '\n'))
}

/// Offener Block im Fließtext: `true` = Inhaltsblock eines Code-Aufrufs (seine `]` fällt weg).
type OpenBlocks = Vec<bool>;

/// Nach `#` bei `i` (auf dem `#`): Code-Ausdruck verarbeiten; gibt den neuen Index zurück und legt einen
/// gesprochenen Inhaltsblock auf `blocks`, wenn einer folgt.
fn code_at(c: &[char], mut i: usize, blocks: &mut OpenBlocks) -> Option<usize> {
    let next = *c.get(i + 1)?;
    if next == '(' || next == '{' {
        return Some(skip_group(c, i + 1));
    }
    if !ident_start(next) {
        return None;
    }
    i += 1;
    let start = i;
    while i < c.len() && ident_char(c[i]) {
        i += 1;
    }
    let name: String = c[start..i].iter().collect();
    while i < c.len() && c[i] == '.' && c.get(i + 1).is_some_and(|&n| ident_start(n)) {
        i += 1;
        while i < c.len() && ident_char(c[i]) {
            i += 1;
        }
    }
    let silent = name == "footnote" || name == "arbeit";
    Some(arguments(c, i, silent, blocks))
}

/// Argumentgruppen ab `i` verarbeiten: `(…)` fällt weg, `[…]` ist stumm oder wird gesprochen geöffnet.
fn arguments(c: &[char], mut i: usize, silent: bool, blocks: &mut OpenBlocks) -> usize {
    loop {
        match c.get(i) {
            Some('(') => i = skip_group(c, i),
            Some('[') if silent => i = skip_group(c, i),
            Some('[') => {
                blocks.push(true);
                return i + 1;
            }
            _ => return i,
        }
    }
}

/// Gesprochener Text: alles, was nach den Zählregeln wegfällt, ist entfernt.
fn spoken_text(src: &str) -> String {
    let c: Vec<char> = src.chars().collect();
    let mut out = String::with_capacity(src.len());
    let mut blocks: OpenBlocks = Vec::new();
    let mut i = 0usize;
    let mut line_start = true;
    while i < c.len() {
        if line_start {
            line_start = false;
            let mut j = i;
            while j < c.len() && matches!(c[j], ' ' | '\t') {
                j += 1;
            }
            if heading_at(&c, j) {
                while i < c.len() && c[i] != '\n' {
                    i += 1;
                }
                continue;
            }
            if line_keyword_at(&c, j) {
                i = skip_code_line(&c, j);
                continue;
            }
            if matches!(c.get(j), Some('-' | '+' | '/')) && matches!(c.get(j + 1), Some(' ' | '\t')) {
                i = j + 2;
                continue;
            }
        }
        let ch = c[i];
        match ch {
            '\n' => {
                out.push('\n');
                line_start = true;
                i += 1;
            }
            '/' if c.get(i + 1) == Some(&'*') => {
                let end = skip_block_comment(&c, i);
                out.extend(c[i..end].iter().filter(|&&x| x == '\n').map(|_| ' '));
                i = end;
            }
            '/' if c.get(i + 1) == Some(&'/') && (i == 0 || c[i - 1].is_whitespace()) => {
                while i < c.len() && c[i] != '\n' {
                    i += 1;
                }
            }
            '`' => {
                i = if c[i..].starts_with(&['`', '`', '`']) {
                    skip_until(&c, i + 3, &['`', '`', '`'], false)
                } else {
                    skip_until(&c, i + 1, &['`'], false)
                };
                out.push(' ');
            }
            '$' => {
                i = skip_until(&c, i + 1, &['$'], true);
                out.push(' ');
            }
            '\\' => match c.get(i + 1) {
                None => i += 1,
                Some(n) if n.is_whitespace() => {
                    out.push(' ');
                    i += 1;
                }
                Some('u') if c.get(i + 2) == Some(&'{') => {
                    let end = c[i + 3..].iter().position(|&x| x == '}').map(|p| i + 3 + p);
                    match end {
                        Some(end) => {
                            let hex: String = c[i + 3..end].iter().collect();
                            if let Some(decoded) = u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32) {
                                out.push(decoded);
                            }
                            i = end + 1;
                        }
                        None => {
                            out.push('u');
                            i += 2;
                        }
                    }
                }
                Some(&n) => {
                    out.push(n);
                    i += 2;
                }
            },
            '#' => match code_at(&c, i, &mut blocks) {
                Some(next) => i = next,
                None => {
                    out.push('#');
                    i += 1;
                }
            },
            '[' => {
                blocks.push(false);
                out.push('[');
                i += 1;
            }
            ']' => match blocks.pop() {
                Some(true) => {
                    // Ein weiterer Inhaltsblock desselben Aufrufs (`#grid[a][b]`) ist ein eigenes Stück Text.
                    if c.get(i + 1) == Some(&'[') {
                        out.push(' ');
                    }
                    i = arguments(&c, i + 1, false, &mut blocks);
                }
                Some(false) | None => {
                    out.push(']');
                    i += 1;
                }
            },
            '<' => {
                let mut k = i + 1;
                while k < c.len() && (ident_char(c[k]) || c[k] == '.' || c[k] == ':') {
                    k += 1;
                }
                if k > i + 1 && c.get(k) == Some(&'>') && ident_start(c[i + 1]) {
                    i = k + 1;
                } else {
                    out.push('<');
                    i += 1;
                }
            }
            '@' if (i == 0 || !c[i - 1].is_alphanumeric()) && c.get(i + 1).is_some_and(|&n| ident_start(n)) => {
                let mut k = i + 1;
                while k < c.len()
                    && (ident_char(c[k])
                        || ((c[k] == '.' || c[k] == ':') && c.get(k + 1).is_some_and(|&n| ident_char(n))))
                {
                    k += 1;
                }
                i = k;
            }
            '*' | '_' => i += 1,
            '~' => {
                out.push(' ');
                i += 1;
            }
            _ => {
                out.push(ch);
                i += 1;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_continued_content_block_after_a_call_counts_too() {
        assert_eq!(
            spoken_words("#grid[eins zwei][drei]"),
            3,
            "beide Inhaltsblöcke eines Aufrufs werden gesprochen, die Klammern nicht"
        );
    }
}
