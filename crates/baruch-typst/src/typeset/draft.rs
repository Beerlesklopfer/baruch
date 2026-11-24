// SPDX-License-Identifier: AGPL-3.0-or-later
//! Entwurf als Wasserzeichen (Runde 22, Vertrag §2.9, Aufgabe spiegel-entwurf).
//!
//! [`draft_rule`] liefert eine Typst-Regel `#set page(foreground: …)`, die der Server **nur in die Satzquelle der
//! Ausgaben** legt (Satz-Vorschau, Buch-PDF, Stichwortzettel, auf Wunsch das Umschlag-PDF) – nie in Export, Spiegel
//! (`main.typ`), Export-Karte oder den Regelblock „// Baruch: Satz“ ([`super::rules`] kennt das Feld nicht,
//! [`super::settings_from_rules`] bleibt unberührt).
//!
//! - **Warum `foreground`:** Hintergrund, Kopf- und Fußzeile gehören schon der Werkvorlage bzw. den Satz-Regeln (die
//!   Seitenzählung ab dem ersten Kapitel steht im Seitenhintergrund, siehe [`super::rules`]); ein zweites
//!   `#set page(background: …)` würde sie ersetzen. Der Vordergrund ist frei; das Wasserzeichen liegt mit geringer
//!   Deckkraft über dem Text und ändert den Umbruch nicht (gleiche Seitenzahl wie ohne Entwurf). Setzt eine
//!   Werkvorlage selbst `foreground`, gilt in den Ausgaben mit Entwurf das Wasserzeichen (es steht dahinter).
//! - **Lage:** diagonal über die Seite (Mitte, −45°), in der Kopfzeile (oben mittig, 7 mm vom Rand) oder in der
//!   Fußzeile (unten mittig).
//! - **Text nur als Zeichenkette:** Text, Zusatz und Datum stehen als maskierte Typst-Zeichenketten in der Regel
//!   (kein Markup, keine Ausführung). Leerzeichen werden zu geschützten Leerzeichen, damit Typst den Text nicht
//!   umbricht. Steuerzeichen fallen weg (die Prüfung in `validate` lehnt sie ohnehin ab).
//! - **Datum nur übergeben:** Die Regel ermittelt kein Datum (kein `datetime.today()`); der Server reicht die
//!   Ortszeit als fertigen Text herein, sonst wären Quelle und Stand nicht reproduzierbar.
//! - **Größe:** aus der Zeichenzahl gerechnet (höchstens 80 pt), damit auch 40 Zeichen auf eine A6-Seite passen.

use super::model::Draft;
use super::model::DraftPlacement;
use super::rules::literal;

/// Erste Zeile der Regel (Kennung in der Satzquelle der Ausgaben).
pub const DRAFT_MARK: &str = "// Baruch: Entwurf (nur in den Ausgaben)";

/// Zweite Zeile des Wasserzeichens: „Stand <Datum>“ (nur mit `draft.date` und übergebenem Datum) und der Zusatz,
/// verbunden mit „ · “; leer, wenn beides fehlt. Auch der EPUB-Hinweis nutzt diese Zeile.
pub fn draft_line(draft: &Draft, date: Option<&str>) -> String {
    let mut parts: Vec<String> = Vec::new();
    if draft.date
        && let Some(date) = date.map(str::trim).filter(|d| !d.is_empty())
    {
        parts.push(format!("Stand {date}"));
    }
    let note = draft.note.trim();
    if !note.is_empty() {
        parts.push(note.to_owned());
    }
    parts.join(" · ")
}

/// Typst-Zahl mit höchstens einer Nachkommastelle.
fn pt(x: f64) -> String {
    let rounded = (x * 10.0).round() / 10.0;
    format!("{rounded}pt")
}

/// Zeichenkette ohne Umbruch: Leerzeichen → geschütztes Leerzeichen, dann maskiert.
fn unbreakable(text: &str) -> String {
    literal(&text.replace(' ', "\u{a0}"))
}

/// Die Regel des Wasserzeichens (endet mit `'\n'`); `date` ist das heutige Datum in Ortszeit als Text (z. B.
/// „10.11.2026“) und wird nur mit `draft.date` gesetzt.
pub fn draft_rule(draft: &Draft, date: Option<&str>) -> String {
    let text = draft.text.trim();
    let line = draft_line(draft, date);
    let alpha = draft.opacity_percent.clamp(1, 100);
    let fill = format!("rgb(0%, 0%, 0%, {alpha}%)");
    let body = match draft.placement {
        DraftPlacement::Diagonal => {
            let chars = text.chars().count().max(1) as f64;
            let size = (360.0 / (0.7 * chars)).min(80.0);
            let mut stack = format!(
                "align(center, text(size: {}, weight: \"bold\", tracking: 0.05em, fill: {fill}, {}))",
                pt(size),
                unbreakable(text)
            );
            if !line.is_empty() {
                let small = (360.0 / (0.5 * line.chars().count().max(1) as f64)).clamp(5.0, 14.0);
                stack.push_str(&format!(
                    ", align(center, text(size: {}, fill: {fill}, {}))",
                    pt(small),
                    unbreakable(&line)
                ));
            }
            format!("place(center + horizon, rotate(-45deg, stack(dir: ttb, spacing: 3mm, {stack})))")
        }
        DraftPlacement::Header | DraftPlacement::Footer => {
            let full = if line.is_empty() { text.to_owned() } else { format!("{text} · {line}") };
            let (anchor, dy) =
                if draft.placement == DraftPlacement::Header { ("top", "7mm") } else { ("bottom", "-7mm") };
            format!(
                "place({anchor} + center, dy: {dy}, text(size: 9pt, weight: \"bold\", tracking: 0.15em, fill: {fill}, \
                 {}))",
                unbreakable(&full)
            )
        }
    };
    format!("{DRAFT_MARK}\n#set page(foreground: {body})\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_date_appears_only_when_asked_for_and_given() {
        let mut draft = Draft { note: "Fassungen von heute".into(), ..Draft::default() };
        assert_eq!(draft_line(&draft, Some("9.11.2026")), "Fassungen von heute", "ohne draft.date kein Datum");
        draft.date = true;
        assert_eq!(draft_line(&draft, Some("9.11.2026")), "Stand 9.11.2026 · Fassungen von heute", "Datum vorn");
        assert_eq!(
            draft_line(&draft, None),
            "Fassungen von heute",
            "ohne übergebenes Datum keins (nie selbst ermittelt)"
        );
        draft.note.clear();
        assert_eq!(draft_line(&draft, Some("9.11.2026")), "Stand 9.11.2026", "nur das Datum");
    }

    #[test]
    fn the_rule_has_its_mark_and_no_date_function() {
        let rule = draft_rule(&Draft { date: true, ..Draft::default() }, Some("1.2.2026"));
        assert!(rule.starts_with(DRAFT_MARK), "Kennzeile vorn: {rule}");
        assert!(rule.ends_with(")\n"), "eine Regel, mit Zeilenende: {rule}");
        assert!(!rule.contains("datetime"), "Typst ermittelt kein Datum: {rule}");
        assert!(rule.contains("\"Stand\u{a0}1.2.2026\""), "Datum als übergebener Text: {rule}");
        assert!(rule.contains("rgb(0%, 0%, 0%, 12%)"), "Deckkraft 12 % der Vorgabe: {rule}");
    }
}
