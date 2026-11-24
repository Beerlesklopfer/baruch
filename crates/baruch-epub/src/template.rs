// SPDX-License-Identifier: AGPL-3.0-or-later
//! Was das EPUB aus der Werkvorlage liest (Besitz „epub“, Runde 20a): Show-Regeln mit `pagebreak` und die Sprache.
//!
//! Ein EPUB fließt; Typst-Regeln wirken dort nicht. Seitenumbrüche vor Kapiteln und Teilen bildet die
//! Dateiaufteilung ab (jede Teil-Seite und jedes Kapitel eine eigene Datei, dazu `break-before` im CSS). Darum
//! meldet [`pagebreak_rules`] jede Show-Regel der Vorlage, die `pagebreak` enthält:
//! - Regel für Überschriften der Ebene 1 (bzw. für Teile, `<baruch-part>`) → abgebildet
//!   ([`TEMPLATE_PAGEBREAK_MAPPED`], Info); die von [`baruch_typst::typeset::detect_chapter_break`] erkannte Regel
//!   bestimmt dazu `break-before` (neue Seite bzw. rechte Seite).
//! - jede andere Regel mit `pagebreak` → nicht abbildbar ([`TEMPLATE_PAGEBREAK_UNMAPPED`], Warnung).

use baruch_typst::typeset::{ChapterStart, detect_chapter_break};

pub const TEMPLATE_PAGEBREAK_MAPPED: &str = "TEMPLATE_PAGEBREAK_MAPPED";
pub const TEMPLATE_PAGEBREAK_UNMAPPED: &str = "TEMPLATE_PAGEBREAK_UNMAPPED";

/// Eine Show-Regel der Vorlage mit `pagebreak`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PagebreakRule {
    /// Erste Zeile (1-basiert).
    pub line: usize,
    /// Regeltext (Kommentare entfernt, auf eine Zeile verkürzt).
    pub text: String,
    /// `true` = Kapitel bzw. Teil (im EPUB abgebildet).
    pub mapped: bool,
}

/// Kopie ohne Kommentare (durch Leerzeichen ersetzt, Zeilenenden bleiben), gleiche Bytelänge.
fn strip_comments(source: &str) -> String {
    let bytes = source.as_bytes();
    let mut out = bytes.to_vec();
    let mut i = 0;
    let mut in_string = false;
    while i < bytes.len() {
        let b = bytes[i];
        if in_string {
            match b {
                b'\\' => i += 1,
                b'"' | b'\n' => in_string = false,
                _ => {}
            }
            i += 1;
            continue;
        }
        if b == b'"' {
            in_string = true;
            i += 1;
        } else if bytes[i..].starts_with(b"//") && !(i > 0 && bytes[i - 1] == b':') {
            while i < bytes.len() && bytes[i] != b'\n' {
                out[i] = b' ';
                i += 1;
            }
        } else if bytes[i..].starts_with(b"/*") {
            while i < bytes.len() && !bytes[i..].starts_with(b"*/") {
                if bytes[i] != b'\n' {
                    out[i] = b' ';
                }
                i += 1;
            }
            for _ in 0..2 {
                if i < bytes.len() {
                    out[i] = b' ';
                    i += 1;
                }
            }
        } else {
            i += 1;
        }
    }
    // Nur ASCII-Bytes wurden ersetzt, aber mitten in Mehrbyte-Zeichen kann ein Kommentar nicht beginnen; ersetzte
    // Folgebytes von Mehrbyte-Zeichen in Kommentaren sind Leerzeichen geworden – das Ergebnis bleibt gültiges UTF-8,
    // weil ganze Zeichen bis zum Zeilenende ersetzt werden.
    String::from_utf8(out).unwrap_or_else(|e| String::from_utf8_lossy(e.as_bytes()).into_owned())
}

/// Ende eines Ausdrucks ab `start`: bis zum Zeilenende auf Klammertiefe 0.
fn statement_end(clean: &str, start: usize) -> usize {
    let bytes = clean.as_bytes();
    let mut depth = 0i32;
    let mut in_string = false;
    let mut i = start;
    while i < bytes.len() {
        let b = bytes[i];
        if in_string {
            match b {
                b'\\' => i += 1,
                b'"' => in_string = false,
                _ => {}
            }
        } else {
            match b {
                b'"' => in_string = true,
                b'(' | b'[' | b'{' => depth += 1,
                b')' | b']' | b'}' => depth -= 1,
                b'\n' if depth <= 0 => return i,
                _ => {}
            }
        }
        i += 1;
    }
    bytes.len()
}

/// Ist das Wort `show` an Stelle `at` ein eigenes Schlüsselwort (`#show` oder `show` am Zeilenanfang/in Code)?
fn show_at(clean: &str, at: usize) -> bool {
    let before = clean[..at].chars().next_back();
    let after = clean[at + 4..].chars().next();
    let word_before = before.is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '-' || c == '.');
    let word_after = after.is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '-');
    !word_before && !word_after
}

/// Selektor einer Show-Regel: Text bis zum ersten `:` auf Klammertiefe 0 (außerhalb von Zeichenketten).
fn selector_of(rule: &str) -> &str {
    let mut depth = 0i32;
    let mut in_string = false;
    let mut escaped = false;
    for (i, c) in rule.char_indices() {
        if in_string {
            match c {
                _ if escaped => escaped = false,
                '\\' => escaped = true,
                '"' => in_string = false,
                _ => {}
            }
            continue;
        }
        match c {
            '"' => in_string = true,
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth -= 1,
            ':' if depth == 0 => return &rule[..i],
            _ => {}
        }
    }
    rule
}

/// Alle Show-Regeln der Vorlage mit `pagebreak`, in Textreihenfolge.
pub fn pagebreak_rules(template: &str) -> Vec<PagebreakRule> {
    let clean = strip_comments(template);
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(found) = clean[from..].find("show") {
        let at = from + found;
        from = at + 4;
        if !show_at(&clean, at) {
            continue;
        }
        let end = statement_end(&clean, at);
        let text = &clean[at..end];
        from = end.max(from);
        if !text.contains("pagebreak") {
            continue;
        }
        let selector: String = selector_of(&text[4..]).chars().filter(|c| !c.is_whitespace()).collect();
        let level_one = selector.starts_with("heading") && selector.contains("level:1");
        let part = selector.contains("baruch-part");
        let compact: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
        let start = if at > 0 && clean.as_bytes()[at - 1] == b'#' { at - 1 } else { at };
        out.push(PagebreakRule {
            line: clean[..start].matches('\n').count() + 1,
            text: format!("{}{compact}", if start < at { "#" } else { "" }),
            mapped: level_one || part,
        });
    }
    out
}

/// Kapitelanfang, den die Vorlage selbst setzt (erkannte Regel `#show heading.where(level: 1): …pagebreak…`).
pub fn template_chapter_start(template: &str) -> Option<ChapterStart> {
    detect_chapter_break(template).map(|rule| rule.chapter_start)
}

/// Sprache aus `#set text(lang: "…")` der Vorlage (letzte Angabe gilt, Kommentare zählen nicht).
pub fn template_lang(template: &str) -> Option<String> {
    let clean = strip_comments(template);
    let mut found = None;
    let mut from = 0;
    while let Some(at) = clean[from..].find("lang:") {
        let at = from + at;
        from = at + 5;
        let before = clean[..at].chars().next_back();
        if before.is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '-') {
            continue;
        }
        let line_start = clean[..at].rfind('\n').map_or(0, |n| n + 1);
        if !clean[line_start..at].contains("text(") {
            continue;
        }
        let rest = clean[at + 5..].trim_start();
        if let Some(rest) = rest.strip_prefix('"')
            && let Some(close) = rest.find('"')
        {
            let lang = rest[..close].trim();
            if is_language_tag(lang) {
                found = Some(lang.to_owned());
            }
        }
    }
    found
}

/// Grobe Prüfung eines BCP-47-Tags: Teile aus 1–8 Buchstaben/Ziffern, getrennt durch `-`, der erste nur Buchstaben.
pub fn is_language_tag(tag: &str) -> bool {
    let mut parts = tag.split('-');
    let first = parts.next().unwrap_or("");
    (2..=8).contains(&first.len())
        && first.chars().all(|c| c.is_ascii_alphabetic())
        && parts.all(|p| (1..=8).contains(&p.len()) && p.chars().all(|c| c.is_ascii_alphanumeric()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rules_in_comments_do_not_count() {
        let template = "// #show heading: it => { pagebreak(); it }\n/* #show figure: it => pagebreak() */\n";
        assert!(pagebreak_rules(template).is_empty(), "Kommentare sind keine Regeln");
    }

    #[test]
    fn a_multiline_rule_is_one_rule_with_its_first_line() {
        let template = "#set text(lang: \"de\")\n#show heading.where(level: 2): it => {\n  pagebreak()\n  it\n}\n";
        let rules = pagebreak_rules(template);
        assert_eq!(rules.len(), 1, "eine Regel über drei Zeilen: {rules:?}");
        assert_eq!(rules[0].line, 2, "erste Zeile der Regel");
        assert!(!rules[0].mapped, "Ebene 2 lässt sich nicht abbilden");
        assert!(rules[0].text.starts_with("#show heading.where(level: 2)"), "Regeltext: {}", rules[0].text);
    }

    #[test]
    fn the_language_comes_from_set_text() {
        assert_eq!(template_lang("#set text(lang: \"en\", size: 11pt)\n").as_deref(), Some("en"), "lang aus text()");
        assert_eq!(template_lang("#set page(lang: \"fr\")\n"), None, "nur text() setzt die Sprache");
        assert_eq!(template_lang("#set text(lang: \"x y\")\n"), None, "kein gültiges Sprachkürzel");
    }
}
