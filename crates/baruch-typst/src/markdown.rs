// SPDX-License-Identifier: AGPL-3.0-or-later
//! Markdown headings while typing become Typst headings (user decision 2026-11-07).
//!
//! A line that starts in column 0 with 1–6 `#` and a space (`# Titel`, `## Titel` …) becomes the Typst heading `=`
//! × level + space + title; the outline is then reconciled as with a typed `=` line (when saving in the store).
//! This rule is authoritative; the widgets of the editor apply it live while typing (`web/src/markdown.ts`, same
//! cases).
//!
//! - **Level:** Baruch knows three heading levels (chapter, section, subsection). `####` to `######` are limited to
//!   level 3 (`===`); whether the level fits there is checked as always by the outline when saving (otherwise a
//!   message, nothing saved).
//! - **Title:** literal as in a typed `=` line; a closing Markdown sequence (` ##`) and whitespace at the end are
//!   dropped. Without a title (`# ` alone) the line stays.
//! - **Exceptions** (stay as they are): `\#` at the start, the hash in quotation marks (`'# …'`, `"# …"`, „…“),
//!   Typst code (`#heading(…)`, `#set`, `#let` – no space after the hash), indented lines, lines in raw blocks
//!   (```` ``` ````), in comments (`//`, `/* */`), in strings and brackets of code or content blocks – that is,
//!   everywhere a `=` line would not be a heading either ([`crate::parse`]). The work template is never converted
//!   (it does not run through this function).

use std::borrow::Cow;

use crate::Scanner;

/// Deepest heading level of the outline.
pub const MAX_LEVEL: usize = 3;

/// A line (with or without line end) as a Typst heading if it is a Markdown heading – regardless of raw/comment
/// before it (that is checked by [`markdown_headings`]).
pub fn markdown_heading_line(line: &str) -> Option<String> {
    let (content, ending) = match line.strip_suffix("\r\n") {
        Some(content) => (content, "\r\n"),
        None => match line.strip_suffix('\n') {
            Some(content) => (content, "\n"),
            None => (line, ""),
        },
    };
    let hashes = content.bytes().take_while(|&b| b == b'#').count();
    if !(1..=6).contains(&hashes) {
        return None;
    }
    let rest = content[hashes..].strip_prefix(' ')?;
    let title = closing_stripped(rest.trim());
    if title.is_empty() {
        return None;
    }
    Some(format!("{} {title}{ending}", "=".repeat(hashes.min(MAX_LEVEL))))
}

/// Remove the closing sequence ` ###` of an ATX heading (only after whitespace, as in Markdown).
fn closing_stripped(title: &str) -> &str {
    let without = title.trim_end_matches('#');
    if without.len() < title.len() && (without.is_empty() || without.ends_with([' ', '\t'])) {
        without.trim_end()
    } else {
        title
    }
}

/// Convert all Markdown headings of a section text (rules above). Without a hit the text comes back unchanged
/// (borrowed).
pub fn markdown_headings(source: &str) -> Cow<'_, str> {
    let mut scanner = Scanner::default();
    let mut out: Option<String> = None;
    let mut offset = 0;
    for line in source.split_inclusive('\n') {
        let converted = if scanner.in_markup() { markdown_heading_line(line) } else { None };
        match converted {
            Some(heading) => {
                let out = out.get_or_insert_with(|| source[..offset].to_owned());
                scanner.feed(&heading);
                out.push_str(&heading);
            }
            None => {
                scanner.feed(line);
                if let Some(out) = out.as_mut() {
                    out.push_str(line);
                }
            }
        }
        offset += line.len();
    }
    out.map_or(Cow::Borrowed(source), Cow::Owned)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markdown_heading_lines_follow_the_table_of_the_decision() {
        let cases: &[(&str, Option<&str>, &str)] = &[
            ("# Titel\n", Some("= Titel\n"), "Ebene 1"),
            ("## Titel\n", Some("== Titel\n"), "Ebene 2"),
            ("### Titel\n", Some("=== Titel\n"), "Ebene 3"),
            ("#### Titel\n", Some("=== Titel\n"), "Ebene 4 auf 3 begrenzt"),
            ("##### Titel\n", Some("=== Titel\n"), "Ebene 5 auf 3 begrenzt"),
            ("###### Titel\n", Some("=== Titel\n"), "Ebene 6 auf 3 begrenzt"),
            ("####### Titel\n", None, "sieben Rauten sind keine Markdown-Überschrift"),
            ("# Titel", Some("= Titel"), "ohne Zeilenende"),
            ("# Titel\r\n", Some("= Titel\r\n"), "CRLF bleibt"),
            ("# Titel  \n", Some("= Titel\n"), "Leerraum am Ende entfällt"),
            ("## Titel ##\n", Some("== Titel\n"), "schließende Folge entfällt"),
            ("# C#\n", Some("= C#\n"), "Raute im Wort bleibt Teil des Titels"),
            ("# Der *große* Tag <tag>\n", Some("= Der *große* Tag <tag>\n"), "Titel wörtlich wie eine =-Zeile"),
            ("# \n", None, "ohne Titel"),
            ("#\n", None, "nur eine Raute"),
            ("#Titel\n", None, "ohne Leerzeichen"),
            ("#\tTitel\n", None, "Tab statt Leerzeichen"),
            ("\\# Titel\n", None, "maskierte Raute"),
            ("'# Titel'\n", None, "in einfachen Anführungszeichen"),
            ("\"# Titel\"\n", None, "in doppelten Anführungszeichen"),
            ("„# Titel“\n", None, "in deutschen Anführungszeichen"),
            ("#heading(level: 2)[Titel]\n", None, "Typst-Code #heading"),
            ("#set text(lang: \"de\")\n", None, "Typst-Code #set"),
            ("#let x = 1\n", None, "Typst-Code #let"),
            (" # Titel\n", None, "eingerückt"),
            ("Text # Titel\n", None, "Raute mitten in der Zeile"),
            ("= Titel\n", None, "schon Typst"),
        ];
        for (line, want, why) in cases {
            assert_eq!(markdown_heading_line(line).as_deref(), *want, "{why}: {line:?}");
        }
    }

    #[test]
    fn whole_texts_convert_only_lines_in_top_level_markup() {
        let cases: &[(&str, &str, &str)] = &[
            (
                "# Kapitel\n\nText.\n\n## Abschnitt\nMehr.\n",
                "= Kapitel\n\nText.\n\n== Abschnitt\nMehr.\n",
                "zwei Ebenen",
            ),
            ("= Kapitel\n\n```\n# kein Titel\n```\n", "= Kapitel\n\n```\n# kein Titel\n```\n", "Raw-Block"),
            (
                "= Kapitel\n\n````md\n```\n# kein Titel\n````\n",
                "= Kapitel\n\n````md\n```\n# kein Titel\n````\n",
                "Raw mit langem Zaun",
            ),
            ("= Kapitel\n// # kein Titel\n", "= Kapitel\n// # kein Titel\n", "Zeilenkommentar"),
            ("= Kapitel\n/* Notiz\n# kein Titel\n*/\n", "= Kapitel\n/* Notiz\n# kein Titel\n*/\n", "Blockkommentar"),
            ("= Kapitel\n#quote[\n# kein Titel\n]\n", "= Kapitel\n#quote[\n# kein Titel\n]\n", "im Inhaltsblock"),
            ("= Kapitel\n#let s = \"a\n# kein Titel\"\n", "= Kapitel\n#let s = \"a\n# kein Titel\"\n", "im String"),
            ("```\n# kein Titel\n```\n# Danach\n", "```\n# kein Titel\n```\n= Danach\n", "nach dem Raw-Block wieder"),
        ];
        for (text, want, why) in cases {
            assert_eq!(markdown_headings(text), *want, "{why}");
        }
        assert!(
            matches!(markdown_headings("= Kapitel\n\nText.\n"), Cow::Borrowed(_)),
            "ohne Markdown-Überschrift bleibt der Text unverändert (geliehen)"
        );
        let once = markdown_headings("# A\n## B\n").into_owned();
        assert_eq!(markdown_headings(&once), once, "zweimal angewandt ändert nichts mehr (idempotent)");
    }
}
