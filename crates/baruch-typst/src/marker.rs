// SPDX-License-Identifier: AGPL-3.0-or-later
//! Teil und „ohne Nummer“ als Markierung im Typst-Text (Vertrag Runde 14, Abschnitt 1).
//!
//! Zwei Zeilenformen, wörtlich, eine Zeile in Spalte 0:
//!
//! ```text
//! #heading(level: 1, numbering: none)[T] <baruch-part>          Teil
//! #heading(level: N, numbering: none)[T] <baruch-unnumbered>    Kapitel/Abschnitt ohne Nummer, N = 1..=3
//! ```
//!
//! Dahinter darf nach Leerzeichen/Tab ein Kommentar `// …` stehen (er bleibt beim Umbenennen
//! erhalten). `T` ist der Titel maskiert wie [`crate::escape_title`], dazu ein `\` vor einem führenden
//! `=`, `-`, `+`, `/` und vor dem Punkt eines führenden `Ziffern.` (sonst begänne im Inhaltsblock eine
//! Struktur). `T` enthält kein unmaskiertes `[` oder `]`, keinen Kommentar und kein offenes Inline-Raw.
//!
//! Warum diese Form: Typst setzt beide ohne Definition in der Werkvorlage, `numbering: none` zählt in
//! Typst nicht mit (Kapitel laufen über Teile hinweg fort wie in Baruch), und die Labels machen die
//! Zeilen eindeutig als Baruch-Markierung. Erkannt wird nur die kanonische Form; jede Abweichung
//! (andere Abstände, anderes Label, Einzug, Ziffer ≠ 1 beim Teil) bleibt Text bzw. Werkvorlage – so
//! bleibt ein frei geschriebenes `#heading(...)` wie bisher unangetastet.

use crate::{comment_start, escape_title, label_start, read_heading};

/// Label der Teil-Markierung.
pub const PART_LABEL: &str = "baruch-part";
/// Label der Markierung „ohne Nummer“.
pub const UNNUMBERED_LABEL: &str = "baruch-unnumbered";

/// Anfang beider Markierungen bis zur Ebene.
const PREFIX: &str = "#heading(level: ";
/// Zwischen Ebene und Titel.
const MIDDLE: &str = ", numbering: none)[";

/// Eine erkannte Markierungszeile (ohne Rücksicht auf Raw/Kommentar davor).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Marker<'a> {
    /// 0 = Teil, 1–3 = „ohne Nummer“.
    pub level: u8,
    /// `T` wörtlich.
    pub raw_title: &'a str,
    /// Kommentar ab `//` bis vor das Zeilenende (leer, wenn keiner).
    pub comment: &'a str,
    /// `\n`, `\r\n` oder leer (letzte Zeile ohne Zeilenende).
    pub ending: &'a str,
}

/// Zeilenende abtrennen: (Inhalt, Zeilenende).
fn split_ending(line: &str) -> (&str, &str) {
    if let Some(content) = line.strip_suffix("\r\n") {
        (content, "\r\n")
    } else if let Some(content) = line.strip_suffix('\n') {
        (content, "\n")
    } else {
        (line, "")
    }
}

/// Länge von `T` bis vor das schließende `]`, wenn `T` kanonisch ist: Maskierungen `\X` werden
/// übersprungen, Inline-Raw muss in der Zeile schließen, kein unmaskiertes `[`, kein Kommentar.
fn title_end(text: &str) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => {
                // Ein Backslash am Ende maskiert das `]` – dann schließt der Block nicht.
                let next = text[i + 1..].chars().next()?;
                i += 1 + next.len_utf8();
            }
            b'`' => {
                let run = text[i..].bytes().take_while(|&b| b == b'`').count();
                let close = text[i + run..].find(&"`".repeat(run))?;
                i += run + close + run;
            }
            b'/' if matches!(bytes.get(i + 1), Some(b'*')) => return None,
            b'/' if matches!(bytes.get(i + 1), Some(b'/')) && !crate::after_scheme(text, i) => return None,
            b'[' => return None,
            b']' => return Some(i),
            _ => i += 1,
        }
    }
    None
}

/// Liest eine Markierungszeile in kanonischer Form, sonst `None`.
pub(crate) fn read_marker(line: &str) -> Option<Marker<'_>> {
    let (content, ending) = split_ending(line);
    let rest = content.strip_prefix(PREFIX)?;
    let level = match rest.as_bytes().first()? {
        digit @ b'1'..=b'3' => digit - b'0',
        _ => return None,
    };
    let rest = rest[1..].strip_prefix(MIDDLE)?;
    let end = title_end(rest)?;
    let raw_title = &rest[..end];
    let after = &rest[end + 1..];
    let (level, after) = if let Some(after) = after.strip_prefix(" <baruch-part>") {
        if level != 1 {
            return None;
        }
        (0, after)
    } else {
        (level, after.strip_prefix(" <baruch-unnumbered>")?)
    };
    let comment = if after.is_empty() {
        ""
    } else {
        let trimmed = after.trim_start_matches([' ', '\t']);
        if trimmed.len() == after.len() || !trimmed.starts_with("//") {
            return None;
        }
        trimmed
    };
    Some(Marker { level, raw_title, comment, ending })
}

/// `T` für den Inhaltsblock aus einem schon maskierten Titel: führende Strukturzeichen bekommen
/// einen Backslash.
fn guard_start(escaped: &str) -> String {
    if escaped.starts_with(['=', '-', '+', '/']) {
        return format!("\\{escaped}");
    }
    let digits = escaped.bytes().take_while(u8::is_ascii_digit).count();
    if digits > 0 && escaped[digits..].starts_with('.') {
        return format!("{}\\{}", &escaped[..digits], &escaped[digits..]);
    }
    escaped.to_owned()
}

/// `T` aus einem Titel (Klartext).
fn content_title(title: &str) -> String {
    guard_start(&escape_title(title))
}

/// `T` aus dem wörtlichen Titel einer `=`-Zeile: unmaskierte `[` und `]` (außerhalb von Inline-Raw)
/// bekommen einen Backslash, führende Strukturzeichen ebenso. `None`, wenn der Titel im Inhaltsblock
/// nicht sicher stünde (offenes Inline-Raw, Kommentar) oder Code enthält (unmaskiertes `#` außerhalb
/// von Inline-Raw, z. B. `#emph[…]`, `#footnote[…]`): Die Klammern eines Aufrufs dürfen keinen
/// Backslash bekommen, sonst setzte Typst den Funktionswert als Raw-Text. Dann greift der Rückfall
/// über die Show-Regel (`snapshot::fallback_rules`), der Text bleibt.
fn content_title_from_raw(raw: &str) -> Option<String> {
    let mut out = String::with_capacity(raw.len() + 4);
    let mut i = 0;
    while i < raw.len() {
        let c = raw[i..].chars().next()?;
        match c {
            '\\' => {
                let next = raw[i + 1..].chars().next().map_or(0, char::len_utf8);
                out.push_str(&raw[i..i + 1 + next]);
                i += 1 + next;
            }
            '`' => {
                let run = raw[i..].bytes().take_while(|&b| b == b'`').count();
                let close = raw[i + run..].find(&"`".repeat(run))?;
                out.push_str(&raw[i..i + run + close + run]);
                i += run + close + run;
            }
            '#' => return None,
            '[' | ']' => {
                out.push('\\');
                out.push(c);
                i += 1;
            }
            _ => {
                out.push(c);
                i += c.len_utf8();
            }
        }
    }
    let out = guard_start(&out);
    title_end(&format!("{out}]")).filter(|&end| end == out.len())?;
    Some(out)
}

fn with_comment(mut line: String, comment: &str) -> String {
    if !comment.is_empty() {
        line.push(' ');
        line.push_str(comment);
    }
    line
}

/// Teil-Markierung mit Zeilenende.
pub fn part_line(title: &str) -> String {
    format!("{PREFIX}1{MIDDLE}{}] <{PART_LABEL}>\n", content_title(title))
}

/// Markierung „ohne Nummer“ (level auf 1..=3 geklemmt) mit Zeilenende.
pub fn unnumbered_line(level: u8, title: &str) -> String {
    format!("{PREFIX}{}{MIDDLE}{}] <{UNNUMBERED_LABEL}>\n", level.clamp(1, 3), content_title(title))
}

/// Kommentar (ab `//` bzw. `/*`) einer `=`-Zeile oder einer Markierung, sonst leer.
pub(crate) fn line_comment(line: &str) -> &str {
    if let Some(marker) = read_marker(line) {
        return marker.comment;
    }
    let content = split_ending(line).0;
    let equals = content.bytes().take_while(|&b| b == b'=').count();
    let rest = &content[equals..];
    if equals == 0 || !rest.starts_with([' ', '\t']) {
        return "";
    }
    let cut = comment_start(rest).unwrap_or(rest.len());
    rest[cut..].trim_end()
}

/// Neue Zeile für [`crate::heading_line`], wenn eine Markierung entsteht: Ebene 0 → Teil;
/// vorige Zeile „ohne Nummer“ und Ebene 1–3 → wieder „ohne Nummer“. Kommentar der vorigen Zeile bleibt.
pub(crate) fn marker_heading_line(level: u8, title: &str, previous: Option<&str>) -> Option<String> {
    let comment = previous.map_or("", line_comment);
    let line = if level == 0 {
        part_line(title)
    } else if previous.and_then(read_marker).is_some_and(|m| m.level > 0) && level <= 3 {
        unnumbered_line(level, title)
    } else {
        return None;
    };
    let line = line.trim_end_matches('\n').to_owned();
    Some(with_comment(line, comment) + "\n")
}

/// Ebene einer Überschriftszeile ändern, Titel wörtlich: `=`-Zeile → Zahl der `=`; Markierung „ohne
/// Nummer“ → `level: N`. Teil-Markierung, level 0 oder keine Überschrift → None. Zeilenende bleibt.
/// Ebenen über 3 gibt es in der Gliederung nicht; auch sie ergeben `None`.
pub fn relevel_line(line: &str, level: u8) -> Option<String> {
    if !(1..=3).contains(&level) {
        return None;
    }
    if let Some(marker) = read_marker(line) {
        if marker.level == 0 {
            return None;
        }
        // Die Ziffer steht an fester Stelle direkt hinter dem Präfix.
        return Some(format!("{PREFIX}{level}{}", &line[PREFIX.len() + 1..]));
    }
    read_heading(line)?;
    let equals = line.bytes().take_while(|&b| b == b'=').count();
    Some(format!("{}{}", "=".repeat(usize::from(level)), &line[equals..]))
}

/// Erste Überschriftszeile von `body` (davor nur Leerraum) umstellen, Rest byte-gleich:
/// numbered=false und `=`-Zeile ohne Label → Markierung „ohne Nummer“ (Titel wörtlich, Kommentar
/// bleibt); numbered=true und Markierung „ohne Nummer“ → `=`-Zeile. Sonst (schon so, Teil, `=`-Zeile
/// mit eigenem Label, keine Überschrift) → None. `None` auch, wenn der wörtliche Titel im
/// Inhaltsblock nicht sicher stünde (offenes Inline-Raw).
pub fn set_numbered_line(body: &str, numbered: bool) -> Option<String> {
    let first = body.len() - body.trim_start().len();
    let start = body[..first].rfind('\n').map_or(0, |at| at + 1);
    if start != first {
        return None; // eingerückt: keine Überschrift
    }
    let end = body[start..].find('\n').map_or(body.len(), |at| start + at + 1);
    let line = &body[start..end];
    let replaced = if let Some(marker) = read_marker(line) {
        if marker.level == 0 || !numbered {
            return None;
        }
        let heading = format!("{} {}", "=".repeat(usize::from(marker.level)), marker.raw_title);
        with_comment(heading, marker.comment) + marker.ending
    } else {
        let (level, raw_title) = read_heading(line)?;
        if numbered {
            return None;
        }
        let (content, ending) = split_ending(line);
        let rest = &content[usize::from(level)..];
        let cut = comment_start(rest).unwrap_or(rest.len());
        if label_start(rest[..cut].trim()).is_some() {
            return None;
        }
        let title = content_title_from_raw(&raw_title)?;
        let marker = format!("{PREFIX}{level}{MIDDLE}{title}] <{UNNUMBERED_LABEL}>");
        with_comment(marker, rest[cut..].trim_end()) + ending
    };
    Some(format!("{}{replaced}{}", &body[..start], &body[end..]))
}

/// Typst-String-Literal.
fn string_literal(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Typst-Regeln für die Teil-Seite im Satz (nur R14a benutzt sie): je Teil-Markierung in
/// Reihenfolge labels[i] als Nummer (leer = ohne), eigene rechte Seite, danach Seitenumbruch;
/// endet mit Zeilenende.
///
/// Die Regel greift über das Label `<baruch-part>` und benutzt nur `it.body`, nicht `it` selbst:
/// So wirken Überschrift-Regeln der Werkvorlage nicht auf die Teil-Seite, die Überschrift bleibt
/// aber für `outline` und `query` erhalten. Sie gehört hinter die Werkvorlage (die zuletzt
/// definierte Regel greift zuerst). Gezählt wird mit dem eigenen Zähler `baruch-part`, nicht mit
/// dem Überschriftenzähler.
pub fn part_page_rule(labels: &[String]) -> String {
    let list: Vec<String> = labels.iter().map(|l| string_literal(l)).collect();
    // Ein Element braucht das Komma, sonst wäre `("I")` kein Array.
    let array = format!("({}{})", list.join(", "), if list.len() == 1 { "," } else { "" });
    format!(
        "#show <{PART_LABEL}>: it => {{ \
pagebreak(weak: true, to: \"odd\"); \
counter(\"baruch-part\").step(); \
v(1fr); \
context {{ let n = counter(\"baruch-part\").get().first(); \
let num = {array}.at(n - 1, default: \"\"); \
align(center, block({{ if num != \"\" {{ text(size: 1.4em, num); parbreak() }}; \
text(size: 2em, weight: \"bold\", it.body) }})) }}; \
v(1fr); \
pagebreak(weak: true) }}\n"
    )
}
