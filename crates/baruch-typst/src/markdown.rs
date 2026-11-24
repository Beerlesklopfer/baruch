// SPDX-License-Identifier: AGPL-3.0-or-later
//! Markdown-Überschriften beim Tippen werden Typst-Überschriften (Nutzerentscheidung 2026-11-07).
//!
//! Eine Zeile, die in Spalte 0 mit 1–6 `#` und einem Leerzeichen beginnt (`# Titel`, `## Titel` …), wird
//! zur Typst-Überschrift `=` × Ebene + Leerzeichen + Titel; die Gliederung gleicht sich danach ab wie bei
//! einer getippten `=`-Zeile (beim Speichern im Store). Diese Regel ist maßgeblich; die Widgets des
//! Editors wenden sie live beim Tippen an (`web/src/markdown.ts`, gleiche Fälle).
//!
//! - **Ebene:** Baruch kennt drei Überschriftsebenen (Kapitel, Abschnitt, Unterabschnitt). `####` bis
//!   `######` werden auf Ebene 3 (`===`) begrenzt; ob die Ebene dort passt, prüft wie immer die Gliederung
//!   beim Speichern (sonst eine Meldung, nichts gespeichert).
//! - **Titel:** wörtlich wie in einer getippten `=`-Zeile; eine schließende Markdown-Folge (` ##`) und
//!   Leerraum am Ende entfallen. Ohne Titel (`# ` allein) bleibt die Zeile.
//! - **Ausnahmen** (bleiben, wie sie sind): `\#` am Anfang, die Raute in Anführungszeichen (`'# …'`,
//!   `"# …"`, „…“), Typst-Code (`#heading(…)`, `#set`, `#let` – nach der Raute kein Leerzeichen),
//!   eingerückte Zeilen, Zeilen in Raw-Blöcken (```` ``` ````), in Kommentaren (`//`, `/* */`), in Strings
//!   und Klammern von Code oder Inhaltsblöcken – also überall dort, wo auch eine `=`-Zeile keine
//!   Überschrift wäre ([`crate::parse`]). Die Werkvorlage wird nie umgewandelt (sie läuft nicht durch
//!   diese Funktion).

use std::borrow::Cow;

use crate::Scanner;

/// Tiefste Überschriftsebene der Gliederung.
pub const MAX_LEVEL: usize = 3;

/// Eine Zeile (mit oder ohne Zeilenende) als Typst-Überschrift, wenn sie eine Markdown-Überschrift ist –
/// ohne Rücksicht auf Raw/Kommentar davor (das prüft [`markdown_headings`]).
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

/// Schließende Folge ` ###` einer ATX-Überschrift entfernen (nur nach Leerraum, wie in Markdown).
fn closing_stripped(title: &str) -> &str {
    let without = title.trim_end_matches('#');
    if without.len() < title.len() && (without.is_empty() || without.ends_with([' ', '\t'])) {
        without.trim_end()
    } else {
        title
    }
}

/// Alle Markdown-Überschriften eines Abschnittstexts umwandeln (Regeln oben). Ohne Treffer kommt der
/// Text unverändert (geliehen) zurück.
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
