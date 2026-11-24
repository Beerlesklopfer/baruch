// SPDX-License-Identifier: AGPL-3.0-or-later
//! Werkvorlage und Satz-Einstellungen (Runde 29, Vertrag §5.3/§5.4): Konflikte melden, wo die Vorlage dieselbe
//! Sache selbst setzt ([`template_conflicts`]), eine Kapitelanfangs-Regel der Vorlage erkennen
//! ([`detect_chapter_break`]) und entfernen ([`remove_rule`]). Kommentare in der Vorlage zählen nicht.
//!
//! Gelesen wird eine Kopie der Vorlage, in der Zeilen- und Blockkommentare durch Leerzeichen ersetzt sind
//! (Zeichenketten bleiben, `"http://…"` beginnt keinen Kommentar); Bytestellen und Zeilen bleiben dabei gleich.
//! - **Set-Regeln:** `set page|text|par|footnote(…)` mit `#` davor oder im Code (nach `{`, `;`, Zeilenanfang); gezählt
//!   werden die benannten Argumente der obersten Ebene. Gemeldet wird die Zeile des Arguments.
//! - **Kapitelanfang:** eine Show-Regel für Überschriften (`heading` oder `heading.where(level: 1 …)`), deren Rumpf
//!   `pagebreak` enthält. Sie wirkt hinter der Baruch-Regel weiter (Typst wendet die zuerst definierte Show-Regel
//!   zuletzt an); darum ist sie nur dann unschädlich, wenn sie nichts anderes will als die Einstellung (gleicher Wert,
//!   oder die Vorlage bricht auf eine neue Seite und die Einstellung verlangt die rechte Seite).
//! - **Erkennen** ([`detect_chapter_break`]) nur, wenn die Regel ausschließlich aus dem Umbruch und der Überschrift
//!   besteht (`it => { pagebreak(…); it }` bzw. `it => [#pagebreak(…) #it]`, beliebiger Leerraum und Zeilenumbrüche,
//!   mit oder ohne `weak`, `to: "odd"` → rechte Seite, ohne `to` → neue Seite, `to: "even"` → nicht übernehmbar):
//!   Nur dann geht beim Entfernen nichts verloren.

use serde::Serialize;

use super::model::{ChapterStart, PageNumberPosition, PageNumberStart, TypesetSettings, field_label};

/// Wirkung einer Einstellung, die die Werkvorlage ebenfalls setzt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ConflictEffect {
    /// Die Baruch-Regel steht hinter der Vorlage und gilt.
    Overrides,
    /// Eine Show-Regel der Vorlage wirkt trotzdem (z. B. Seitenumbruch bei „fortlaufend“).
    Ineffective,
}

/// Eine Stelle der Werkvorlage, die ein gesetztes Feld selbst setzt.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TemplateConflict {
    /// Feld-id aus [`super::FIELDS`].
    pub field: &'static str,
    /// Zeile in der Werkvorlage (1-basiert).
    pub line: usize,
    /// Text der Zeile ohne Zeilenende.
    pub text: String,
    pub effect: ConflictEffect,
    /// Deutsche Meldung.
    pub message: String,
}

/// Erkannte Kapitelanfangs-Regel der Werkvorlage.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ChapterBreakRule {
    /// Bytebereich `start..end` der ganzen Zeile(n) der Regel inklusive abschließendem '\n'.
    pub start: usize,
    pub end: usize,
    /// Erste Zeile der Regel (1-basiert).
    pub line: usize,
    /// Regeltext ohne Zeilenende.
    pub text: String,
    /// Wert, den die Regel bewirkt.
    pub chapter_start: ChapterStart,
}

/// Die kanonische Kapitelanfangs-Regel (neue rechte Seite).
pub const CANONICAL_CHAPTER_BREAK: &str =
    "#show heading.where(level: 1): it => { pagebreak(weak: true, to: \"odd\"); it }";

// ---- Lesen der Vorlage -------------------------------------------------------------------------

/// Kopie von `template` ohne Kommentare (durch Leerzeichen ersetzt, Zeilenenden bleiben), gleiche Bytelänge.
fn strip_comments(template: &str) -> String {
    let bytes = template.as_bytes();
    let mut out = bytes.to_vec();
    let mut i = 0;
    let mut in_string = false;
    while i < bytes.len() {
        let b = bytes[i];
        if in_string {
            match b {
                b'\\' => i += 2,
                b'"' => {
                    in_string = false;
                    i += 1;
                }
                b'\n' => {
                    in_string = false; // Zeichenkette über das Zeilenende hinaus gibt es nicht; nicht weiterziehen.
                    i += 1;
                }
                _ => i += 1,
            }
            continue;
        }
        match (b, bytes.get(i + 1)) {
            (b'"', _) => {
                in_string = true;
                i += 1;
            }
            (b'/', Some(b'/')) => {
                while i < bytes.len() && bytes[i] != b'\n' {
                    out[i] = b' ';
                    i += 1;
                }
            }
            (b'/', Some(b'*')) => {
                let mut depth = 0usize;
                while i < bytes.len() {
                    if bytes[i] == b'/' && bytes.get(i + 1) == Some(&b'*') {
                        depth += 1;
                        out[i] = b' ';
                        out[i + 1] = b' ';
                        i += 2;
                    } else if bytes[i] == b'*' && bytes.get(i + 1) == Some(&b'/') {
                        depth -= 1;
                        out[i] = b' ';
                        out[i + 1] = b' ';
                        i += 2;
                        if depth == 0 {
                            break;
                        }
                    } else {
                        if bytes[i] != b'\n' {
                            out[i] = b' ';
                        }
                        i += 1;
                    }
                }
            }
            _ => i += 1,
        }
    }
    // Nur ASCII-Bytes wurden ersetzt bzw. ganze Zeichen in Kommentaren byteweise durch Leerzeichen: gültiges UTF-8.
    String::from_utf8(out).unwrap_or_else(|_| template.to_owned())
}

/// Zeile (1-basiert) der Bytestelle `at`.
fn line_of(text: &str, at: usize) -> usize {
    text.as_bytes()[..at.min(text.len())].iter().filter(|&&b| b == b'\n').count() + 1
}

/// Text der Zeile `line` (1-basiert) ohne Zeilenende, getrimmt.
fn line_text(text: &str, line: usize) -> String {
    text.lines().nth(line.saturating_sub(1)).unwrap_or("").trim().to_owned()
}

/// Ende (exklusiv) des geklammerten Ausdrucks, der bei `open` (`(`, `[` oder `{`) beginnt; Zeichenketten zählen nicht.
fn balanced_end(clean: &str, open: usize) -> Option<usize> {
    let bytes = clean.as_bytes();
    let mut depth = 0usize;
    let mut i = open;
    let mut in_string = false;
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
                b')' | b']' | b'}' => {
                    depth = depth.checked_sub(1)?;
                    if depth == 0 {
                        return Some(i + 1);
                    }
                }
                _ => {}
            }
        }
        i += 1;
    }
    None
}

/// Benannte Argumente der obersten Ebene in `args` (ohne die äußeren Klammern), je (Name, Bytestelle in `args`).
fn named_args(args: &str) -> Vec<(String, usize)> {
    let bytes = args.as_bytes();
    let mut out = Vec::new();
    let mut depth = 0usize;
    let mut in_string = false;
    let mut item_start = 0usize;
    let mut i = 0;
    let check = |start: usize, end: usize, out: &mut Vec<(String, usize)>| {
        let item = &args[start..end];
        let lead = item.len() - item.trim_start().len();
        let item = item.trim_start();
        let name: String = item.chars().take_while(|c| c.is_alphanumeric() || *c == '-' || *c == '_').collect();
        if !name.is_empty() && item[name.len()..].trim_start().starts_with(':') {
            out.push((name, start + lead));
        }
    };
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
                b')' | b']' | b'}' => depth = depth.saturating_sub(1),
                b',' if depth == 0 => {
                    check(item_start, i, &mut out);
                    item_start = i + 1;
                }
                _ => {}
            }
        }
        i += 1;
    }
    check(item_start, bytes.len(), &mut out);
    out
}

/// Ist `at` der Anfang eines Schlüsselworts im Code (mit `#` davor oder nach `{`, `;`, `(`, `[`, Zeilenanfang)?
fn keyword_at(clean: &str, at: usize, word: &str) -> bool {
    if !clean[at..].starts_with(word) {
        return false;
    }
    let after = clean.as_bytes().get(at + word.len()).copied();
    if after.is_some_and(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-') {
        return false;
    }
    let before = clean[..at].trim_end_matches([' ', '\t']);
    matches!(before.as_bytes().last(), None | Some(b'#' | b'{' | b';' | b'(' | b'[' | b'\n'))
}

/// Eine `set`-Regel der Vorlage: Element (`page`, `text` …) und benannte Argumente mit Bytestelle in der Vorlage.
struct SetRule {
    element: String,
    args: Vec<(String, usize)>,
}

fn set_rules(clean: &str) -> Vec<SetRule> {
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(found) = clean[from..].find("set") {
        let at = from + found;
        from = at + 3;
        if !keyword_at(clean, at, "set") {
            continue;
        }
        let rest = &clean[at + 3..];
        let element: String = rest.trim_start().chars().take_while(|c| c.is_ascii_alphabetic()).collect();
        if element.is_empty() {
            continue;
        }
        let after_name = at + 3 + (rest.len() - rest.trim_start().len()) + element.len();
        if clean.as_bytes().get(after_name) != Some(&b'(') {
            continue;
        }
        let Some(end) = balanced_end(clean, after_name) else { continue };
        let inner = &clean[after_name + 1..end - 1];
        let args = named_args(inner).into_iter().map(|(name, pos)| (name, after_name + 1 + pos)).collect();
        out.push(SetRule { element, args });
        from = end;
    }
    out
}

/// Eine Show-Regel der Vorlage: Bytebereich ab `show` bis zum Ende des Ausdrucks (Zeilenende auf Klammertiefe 0),
/// Selektor (ohne Leerraum) und Rumpf (Text nach dem `:`).
struct ShowRule {
    /// Bytestelle von `#` (bzw. `show` ohne `#`).
    start: usize,
    /// Ende des Ausdrucks (exklusiv, vor dem Zeilenende).
    end: usize,
    selector: String,
    body_start: usize,
}

fn show_rules(clean: &str) -> Vec<ShowRule> {
    let bytes = clean.as_bytes();
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(found) = clean[from..].find("show") {
        let at = from + found;
        from = at + 4;
        if !keyword_at(clean, at, "show") {
            continue;
        }
        let start = if at > 0 && bytes[at - 1] == b'#' { at - 1 } else { at };
        // Selektor bis zum ':' auf Klammertiefe 0.
        let mut i = at + 4;
        let mut depth = 0usize;
        let mut in_string = false;
        let mut colon = None;
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
                    b')' | b']' | b'}' => depth = depth.saturating_sub(1),
                    b':' if depth == 0 => {
                        colon = Some(i);
                        break;
                    }
                    b'\n' if depth == 0 => break,
                    _ => {}
                }
            }
            i += 1;
        }
        let Some(colon) = colon else { continue };
        // Rumpf bis zum Zeilenende auf Klammertiefe 0.
        let mut j = colon + 1;
        let mut depth = 0usize;
        let mut in_string = false;
        while j < bytes.len() {
            let b = bytes[j];
            if in_string {
                match b {
                    b'\\' => j += 1,
                    b'"' => in_string = false,
                    _ => {}
                }
            } else {
                match b {
                    b'"' => in_string = true,
                    b'(' | b'[' | b'{' => depth += 1,
                    b')' | b']' | b'}' => depth = depth.saturating_sub(1),
                    b'\n' if depth == 0 => break,
                    _ => {}
                }
            }
            j += 1;
        }
        let end = j.min(bytes.len());
        let selector: String = clean[at + 4..colon].chars().filter(|c| !c.is_whitespace()).collect();
        out.push(ShowRule { start, end, selector, body_start: colon + 1 });
        from = end;
    }
    out
}

/// Gilt der (leerraumfreie) Selektor für Überschriften der Ebene 1?
fn heading_level_one(selector: &str) -> bool {
    if selector == "heading" {
        return true;
    }
    let Some(args) = selector.strip_prefix("heading.where(").and_then(|s| s.strip_suffix(')')) else { return false };
    !args.contains("level:") || args.split(',').any(|a| a == "level:1")
}

/// Umbruch-Art aus den Argumenten eines `pagebreak(…)` (leerraumfrei): `Some(Some(start))` erkannt, `Some(None)` =
/// `to: "even"`, `None` = unbekannte Argumente.
fn pagebreak_kind(args: &str) -> Option<Option<ChapterStart>> {
    let mut start = Some(ChapterStart::NewPage);
    for arg in args.split(',').filter(|a| !a.is_empty()) {
        match arg {
            "weak:true" | "weak:false" => {}
            "to:\"odd\"" => start = start.map(|_| ChapterStart::RightPage),
            "to:\"even\"" => start = None,
            _ => return None,
        }
    }
    Some(start)
}

/// Erste `pagebreak(…)`-Art im Rumpf (für Konflikte); unbekannte Argumente → neue Seite.
fn break_in(body: &str) -> Option<Option<ChapterStart>> {
    let compact: String = body.chars().filter(|c| !c.is_whitespace()).collect();
    let at = compact.find("pagebreak(")?;
    let open = at + "pagebreak".len();
    let end = balanced_end(&compact, open)?;
    Some(pagebreak_kind(&compact[open + 1..end - 1]).unwrap_or(Some(ChapterStart::NewPage)))
}

/// Rumpf genau aus Umbruch und Überschrift? Liefert die Art (`None` bei `to: "even"` oder anderem Inhalt).
fn exact_break(body: &str) -> Option<Option<ChapterStart>> {
    let compact: String = body.chars().filter(|c| !c.is_whitespace()).collect();
    let arrow = compact.find("=>")?;
    let var = &compact[..arrow];
    if var.is_empty() || !var.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-') {
        return None;
    }
    let rest = &compact[arrow + 2..];
    let (inner, code) = if let Some(inner) = rest.strip_prefix('{').and_then(|r| r.strip_suffix('}')) {
        (inner, true)
    } else {
        let inner = rest.strip_prefix('[').and_then(|r| r.strip_suffix(']'))?;
        (inner, false)
    };
    let call = if code { "pagebreak(" } else { "#pagebreak(" };
    let after_call = inner.strip_prefix(call)?;
    let close = after_call.find(')')?;
    let kind = pagebreak_kind(&after_call[..close])?;
    let tail = &after_call[close + 1..];
    let ok = if code {
        let tail = tail.strip_prefix(';').unwrap_or(tail);
        let tail = tail.strip_suffix(';').unwrap_or(tail);
        tail == var
    } else {
        tail == format!("#{var}")
    };
    ok.then_some(kind)
}

/// Erkennt eine Show-Regel für Überschriften Ebene 1 mit Seitenumbruch.
pub fn detect_chapter_break(template: &str) -> Option<ChapterBreakRule> {
    let clean = strip_comments(template);
    for rule in show_rules(&clean) {
        if !rule.selector.starts_with("heading.where(") || !heading_level_one(&rule.selector) {
            continue;
        }
        // Nur genau `heading.where(level: 1)`, damit beim Entfernen keine andere Bedingung verloren geht.
        if rule.selector != "heading.where(level:1)" {
            continue;
        }
        let Some(Some(chapter_start)) = exact_break(&clean[rule.body_start..rule.end]) else { continue };
        // Die Regel beginnt ihre Zeile (nur Leerraum davor) und nichts außer Leerraum folgt ihr auf der letzten Zeile.
        let line_start = clean[..rule.start].rfind('\n').map_or(0, |n| n + 1);
        if !clean[line_start..rule.start].trim().is_empty() {
            continue;
        }
        let line_end = clean[rule.end..].find('\n').map_or(template.len(), |n| rule.end + n + 1);
        if !clean[rule.end..line_end].trim().is_empty() {
            continue;
        }
        let text = template[line_start..line_end].trim_end_matches(['\n', '\r']).to_owned();
        return Some(ChapterBreakRule {
            start: line_start,
            end: line_end,
            line: line_of(template, line_start),
            text,
            chapter_start,
        });
    }
    None
}

/// `template` ohne den Bereich `rule.start..rule.end`; ungültige Grenzen → `template` unverändert.
pub fn remove_rule(template: &str, rule: &ChapterBreakRule) -> String {
    let valid = rule.start <= rule.end
        && rule.end <= template.len()
        && template.is_char_boundary(rule.start)
        && template.is_char_boundary(rule.end);
    if !valid {
        return template.to_owned();
    }
    let mut out = String::with_capacity(template.len() - (rule.end - rule.start));
    out.push_str(&template[..rule.start]);
    out.push_str(&template[rule.end..]);
    out
}

// ---- Konflikte ---------------------------------------------------------------------------------

/// Felder, die ein Argument einer `set`-Regel betrifft (nur gesetzte Felder).
fn fields_of(element: &str, arg: &str, s: &TypesetSettings) -> Vec<&'static str> {
    let first_chapter = s
        .page_numbers
        .as_ref()
        .is_some_and(|n| n.start == PageNumberStart::FirstChapter && n.position != PageNumberPosition::Hidden);
    let top_numbers = s.page_numbers.as_ref().is_some_and(|n| n.position == PageNumberPosition::TopOutside);
    let candidates: Vec<(&'static str, bool)> = match (element, arg) {
        ("page", "paper" | "width" | "height" | "flipped") => vec![("page_format", s.page_format.is_some())],
        ("page", "margin") => vec![("margins", s.margins.is_some())],
        ("page", "numbering" | "number-align" | "footer") => vec![("page_numbers", s.page_numbers.is_some())],
        ("page", "header") => vec![("page_numbers", top_numbers), ("running_head", s.running_head.is_some())],
        ("page", "background") => vec![("page_numbers", first_chapter)],
        ("text", "font") => vec![("font", s.font.is_some())],
        ("text", "size") => vec![("font_size_pt", s.font_size_pt.is_some())],
        ("text", "lang" | "region") => vec![("lang", s.lang.is_some())],
        ("text", "hyphenate") => vec![("hyphenate", s.hyphenate.is_some())],
        ("text", "costs") => vec![("widows_orphans", s.widows_orphans.is_some())],
        ("par", "leading") => vec![("leading_em", s.leading_em.is_some())],
        ("par", "justify") => vec![("justify", s.justify.is_some())],
        ("par", "first-line-indent" | "spacing") => vec![("paragraph", s.paragraph.is_some())],
        ("footnote", "numbering") => vec![("footnotes", s.footnotes.is_some())],
        _ => Vec::new(),
    };
    candidates.into_iter().filter(|(_, set)| *set).map(|(field, _)| field).collect()
}

fn message(field: &str, line: usize, effect: ConflictEffect) -> String {
    let label = field_label(field);
    match effect {
        ConflictEffect::Overrides => format!(
            "Die Werkvorlage setzt {label} selbst (Zeile {line}); die Einstellung gilt, weil Baruch sie dahinter setzt."
        ),
        ConflictEffect::Ineffective => format!(
            "Die Werkvorlage setzt {label} selbst (Zeile {line}); die Einstellung wirkt nicht, solange die Zeile in \
             der Vorlage steht."
        ),
    }
}

/// Stellen der Vorlage, die gesetzte Felder selbst setzen (je Feld und Zeile einmal, nach Zeile sortiert, bei
/// gleicher Zeile in [`super::FIELDS`]-Reihenfolge).
pub fn template_conflicts(template: &str, settings: &TypesetSettings) -> Vec<TemplateConflict> {
    let clean = strip_comments(template);
    let mut out: Vec<TemplateConflict> = Vec::new();
    let push = |field: &'static str, at: usize, effect: ConflictEffect, out: &mut Vec<TemplateConflict>| {
        let line = line_of(template, at);
        if out.iter().any(|c| c.field == field && c.line == line) {
            return;
        }
        out.push(TemplateConflict {
            field,
            line,
            text: line_text(template, line),
            effect,
            message: message(field, line, effect),
        });
    };
    for rule in set_rules(&clean) {
        for (arg, at) in &rule.args {
            for field in fields_of(&rule.element, arg, settings) {
                push(field, *at, ConflictEffect::Overrides, &mut out);
            }
        }
    }
    if let Some(wanted) = settings.chapter_start {
        for rule in show_rules(&clean) {
            if !heading_level_one(&rule.selector) {
                continue;
            }
            let Some(kind) = break_in(&clean[rule.body_start..rule.end]) else { continue };
            // Die Vorlage bricht nach der Baruch-Regel noch einmal um; unschädlich nur, wenn sie nichts anderes will.
            let harmless = matches!(
                (kind, wanted),
                (Some(ChapterStart::NewPage), ChapterStart::NewPage | ChapterStart::RightPage)
                    | (Some(ChapterStart::RightPage), ChapterStart::RightPage)
            );
            let effect = if harmless { ConflictEffect::Overrides } else { ConflictEffect::Ineffective };
            push("chapter_start", rule.start, effect, &mut out);
        }
    }
    let order = |field: &str| super::FIELDS.iter().position(|(id, _)| *id == field).unwrap_or(usize::MAX);
    out.sort_by_key(|c| (c.line, order(c.field)));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_line_is_found_and_removed_with_its_line_end() {
        let template = format!("#set text(lang: \"de\")\n  {CANONICAL_CHAPTER_BREAK}\n#set par(justify: true)\n");
        let rule = detect_chapter_break(&template).expect("kanonische Zeile (eingerückt) wird erkannt");
        assert_eq!(rule.line, 2, "zweite Zeile, 1-basiert");
        assert_eq!(rule.chapter_start, ChapterStart::RightPage, "to: \"odd\" = neue rechte Seite");
        assert_eq!(&template[rule.start..rule.end], format!("  {CANONICAL_CHAPTER_BREAK}\n"), "ganze Zeile samt \\n");
        assert_eq!(
            remove_rule(&template, &rule),
            "#set text(lang: \"de\")\n#set par(justify: true)\n",
            "remove_rule nimmt genau diese Zeile heraus"
        );
    }

    #[test]
    fn commented_rule_is_ignored_and_bad_bounds_leave_the_template() {
        let template = format!("// {CANONICAL_CHAPTER_BREAK}\n/*\n{CANONICAL_CHAPTER_BREAK}\n*/\n");
        assert_eq!(detect_chapter_break(&template), None, "Zeilen- und Blockkommentare zählen nicht");
        let bad = ChapterBreakRule {
            start: 5,
            end: 500,
            line: 1,
            text: String::new(),
            chapter_start: ChapterStart::RightPage,
        };
        assert_eq!(remove_rule(&template, &bad), template, "Bereich hinter dem Ende → unverändert");
    }

    #[test]
    fn comments_keep_byte_positions_and_strings_are_not_comments() {
        let template = "#set text(font: \"a//b\") // Notiz\n/* x\ny */ #set par(justify: true)\n";
        let clean = strip_comments(template);
        assert_eq!(clean.len(), template.len(), "gleiche Bytelänge, damit Stellen und Zeilen stimmen");
        assert!(clean.contains("\"a//b\""), "// in einer Zeichenkette beginnt keinen Kommentar: {clean}");
        assert!(
            !clean.contains("Notiz") && !clean.contains("/*") && !clean.contains("*/"),
            "Kommentare sind weg: {clean}"
        );
        assert_eq!(clean.matches('\n').count(), 3, "Zeilenenden bleiben");
    }

    #[test]
    fn named_arguments_ignore_nested_dictionaries() {
        let args = named_args(" margin: (inside: 2cm, outside: 1cm), paper: \"a5\" ");
        let names: Vec<&str> = args.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, ["margin", "paper"], "inside/outside gehören zum Wörterbuch, nicht zu page");
    }
}
