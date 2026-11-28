// SPDX-License-Identifier: AGPL-3.0-or-later
//! Draft as a watermark (Round 22, contract §2.9, task spiegel-entwurf).
//!
//! [`draft_rule`] delivers a Typst rule `#set page(foreground: …)` that the server puts **only into the typesetting
//! source of the outputs** (typesetting preview, book PDF, cue sheet, on request the cover PDF) – never into
//! export, mirror (`main.typ`), export map or the rule block „// Baruch: Satz“ ([`super::rules`] does not know the
//! field, [`super::settings_from_rules`] stays untouched).
//!
//! - **Why `foreground`:** background, header and footer already belong to the work template or the typesetting
//!   rules (the page counting from the first chapter on stands in the page background, see [`super::rules`]); a
//!   second `#set page(background: …)` would replace them. The foreground is free; the watermark lies with low
//!   opacity above the text and does not change the page breaks (same number of pages as without draft). If a work
//!   template sets `foreground` itself, the watermark applies in the outputs with draft (it stands behind it).
//! - **Position:** diagonally across the page (centre, −45°), in the header (top centre, 7 mm from the edge) or in
//!   the footer (bottom centre).
//! - **Text only as a string:** text, addition and date stand as escaped Typst strings in the rule (no markup, no
//!   execution). Spaces become no-break spaces, so that Typst does not break the text. Control characters are
//!   dropped (the check in `validate` refuses them anyway).
//! - **Date only passed in:** the rule determines no date (no `datetime.today()`); the server hands in the local
//!   time as a finished text, otherwise source and state would not be reproducible.
//! - **Size:** calculated from the number of characters (at most 80 pt), so that even 40 characters fit on an A6 page.

use baruch_core::WorkLanguage;
use baruch_core::book_text::book_t;

use super::model::DraftPlacement;
use super::model::{DEFAULT_DRAFT_TEXT, Draft};
use super::rules::literal;

/// First line of the rule (marker in the typesetting source of the outputs).
pub const DRAFT_MARK: &str = "// Baruch: Entwurf (nur in den Ausgaben)";

/// Second line of the watermark: „Stand <Datum>“ (only with `draft.date` and a date passed in) and the addition,
/// joined with „ · “; empty if both are missing. The EPUB note uses this line too.
pub fn draft_line(draft: &Draft, date: Option<&str>) -> String {
    draft_line_in(draft, date, WorkLanguage::De)
}

/// Like [`draft_line`] in the work language `lang` (Round 39b): „Stand <date>“ / „As of <date>“ from the book catalog.
pub fn draft_line_in(draft: &Draft, date: Option<&str>, lang: WorkLanguage) -> String {
    let mut parts: Vec<String> = Vec::new();
    if draft.date
        && let Some(date) = date.map(str::trim).filter(|d| !d.is_empty())
    {
        parts.push(book_t(lang, "book.draft.as_of", &[("date", date)]));
    }
    let note = draft.note.trim();
    if !note.is_empty() {
        parts.push(note.to_owned());
    }
    parts.join(" · ")
}

/// Typst number with at most one decimal place.
fn pt(x: f64) -> String {
    let rounded = (x * 10.0).round() / 10.0;
    format!("{rounded}pt")
}

/// String without a break: space → no-break space, then escaped.
fn unbreakable(text: &str) -> String {
    literal(&text.replace(' ', "\u{a0}"))
}

/// The rule of the watermark (ends with `'\n'`); `date` is today's date in local time as text (e.g. „10.11.2026“)
/// and is only set with `draft.date`.
pub fn draft_rule(draft: &Draft, date: Option<&str>) -> String {
    draft_rule_in(draft, date, WorkLanguage::De)
}

/// Text of the watermark in the work language `lang` (Round 39b): the stored default [`DEFAULT_DRAFT_TEXT`]
/// („ENTWURF“) becomes the catalog word of the work language („DRAFT“ in English); a text of the human stays as it
/// is. German: always the stored text.
pub fn draft_text_in(draft: &Draft, lang: WorkLanguage) -> String {
    let text = draft.text.trim();
    if text == DEFAULT_DRAFT_TEXT { book_t(lang, "book.draft.watermark", &[]) } else { text.to_owned() }
}

/// Like [`draft_rule`] in the work language `lang` (Round 39b): default text and „Stand“ line from the book catalog
/// ([`draft_text_in`], [`draft_line_in`]).
pub fn draft_rule_in(draft: &Draft, date: Option<&str>, lang: WorkLanguage) -> String {
    let text = draft_text_in(draft, lang);
    let text = text.as_str();
    let line = draft_line_in(draft, date, lang);
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
