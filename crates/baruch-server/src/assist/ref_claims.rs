// SPDX-License-Identifier: AGPL-3.0-or-later
//! Stellenangaben in „Kritisch:“-Befunden prüfen (Runde 17a, L2).
//!
//! Die Zitatprüfung ([`super::prompts::mark_unbacked_criticals`]) prüft nur Zitate in „…“. Ein Befund kann aber
//! auch über eine **Stelle** etwas Falsches behaupten: In der Handprüfung von Runde 16 schrieb das Modell
//! „Kritisch: Lk 15,5 ist im Predigttext nicht belegt“, obwohl der Predigttext Lk 15,4–6 ist. [`mark_ref_claims`]
//! macht das kenntlich, ohne den Befund zu ändern:
//!
//! - Behauptet eine „Kritisch:“-Zeile ([`super::prompts::critical_finding`]), eine Stelle sei nicht belegt, fehle
//!   oder stehe nicht im Text ([`claims_missing`]), und liegt die Stelle in einer Stelle, die im gezeigten Text
//!   genannt ist (Korpus wie bei der Zitatprüfung, [`super::context::shown_corpus`]), bekommt die Zeile
//!   „ (Stelle steht im Text: Lk 15,4–6)“.
//! - Nennt eine „Kritisch:“-Zeile eine Stelle, die es nicht gibt (Kapitel oder Vers jenseits der Verszählung),
//!   bekommt sie „ (Stelle ungültig: <Grund>)“.
//!
//! Gelesen wird mit [`baruch_scripture::scan`] (deutsche Zählung). Die Vermerke setzt nur Baruch: Steht einer schon
//! in der Zeile (vom Modell geschrieben, etwa nach einer Prompt-Injection, oder von einem früheren Durchlauf), wird er
//! entfernt und die Zeile neu geprüft ([`strip_notes`], Nachbesserung Runde 17) – ein gefälschter Vermerk schaltet die
//! Prüfung also nicht ab, und zweimal angewandt ändert weiterhin nichts (idempotent). Angewandt nach der Zitatprüfung im Lektor (Prüfen,
//! Fragen), im Abschnitts-Chat ([`super::run_inputs`]) und in Fäden ohne Abschnitt ([`crate::threads`]).

use baruch_scripture::{Found, Passage, RefStatus, scan};

use super::prompts::critical_finding;

/// Anfang des Vermerks „Stelle steht im Text“.
pub const IN_TEXT: &str = "(Stelle steht im Text:";
/// Anfang des Vermerks „Stelle ungültig“.
pub const INVALID: &str = "(Stelle ungültig:";

/// Wörter, mit denen ein Befund behauptet, eine Stelle fehle im Text (klein geschrieben, als ganze Wörter).
const MISSING_WORDS: &[&str] = &[
    "nicht belegt",
    "unbelegt",
    "nicht im text",
    "nicht im predigttext",
    "nicht im abschnitt",
    "nicht im kapitel",
    "kommt nicht vor",
    "kommt im text nicht vor",
    "nicht enthalten",
    "nicht vorhanden",
    "nicht zitiert",
    "nicht genannt",
    "nicht erwähnt",
    "taucht nicht auf",
    "nicht abgedeckt",
    "nicht gedeckt",
    "außerhalb des",
];

/// Wörter, die erst zusammen mit einem Ort ([`PLACE_WORDS`]) behaupten, eine Stelle fehle: „Lk 15,7 fehlt im
/// Predigttext“ ja, „Zu Lk 15,5 fehlt eine Anwendung“ nein (Kritik am Inhalt, Nachbesserung Runde 17).
const WEAK_MISSING_WORDS: &[&str] = &["steht nicht", "fehlt", "fehlen", "findet sich nicht"];

/// Orte, an denen eine Stelle fehlen kann.
const PLACE_WORDS: &[&str] = &[
    "im text",
    "im predigttext",
    "im bibeltext",
    "im abschnitt",
    "im kapitel",
    "in der predigt",
    "im gezeigten text",
    "im wortlaut",
];

/// Kommt `phrase` in `lower` als ganze Wörter vor (kein Buchstabe und keine Ziffer direkt davor oder danach)?
/// So zählt „verfehlt“ nicht als „fehlt“ und „versteht nicht“ nicht als „steht nicht“.
fn has_words(lower: &str, phrase: &str) -> bool {
    lower.match_indices(phrase).any(|(at, _)| {
        let before = lower[..at].chars().next_back();
        let after = lower[at + phrase.len()..].chars().next();
        !before.is_some_and(char::is_alphanumeric) && !after.is_some_and(char::is_alphanumeric)
    })
}

/// Behauptet die Zeile, etwas sei nicht belegt, fehle oder stehe nicht im Text? Baruchs eigener Vermerk der
/// Zitatprüfung ([`super::prompts::UNBACKED_NOTE`], „(nicht im Text belegt)“) zählt nicht als Behauptung über eine
/// Stelle – weder wenn Baruch ihn angehängt hat noch wenn das Modell ihn nachahmt (Handprüfung Integrator Runde 17:
/// „… zu Joh 3,16 ist faktisch falsch (nicht im Text belegt) …“ bekam sonst „(Stelle steht im Text: Joh 3,16)“).
pub fn claims_missing(line: &str) -> bool {
    let lower = strip_notes(&line.replace(super::prompts::UNBACKED_NOTE, "")).to_lowercase();
    MISSING_WORDS.iter().any(|w| has_words(&lower, w))
        || (WEAK_MISSING_WORDS.iter().any(|w| has_words(&lower, w)) && PLACE_WORDS.iter().any(|w| has_words(&lower, w)))
}

/// Alle Vermerke „(Stelle steht im Text: …)“ und „(Stelle ungültig: …)“ aus einer Zeile entfernen (bis zur
/// passenden Klammer, samt Leerzeichen davor); Zeilenende ohne Leerraum. Die Vermerke setzt nur [`mark_ref_claims`].
pub fn strip_notes(line: &str) -> String {
    if !line.contains(IN_TEXT) && !line.contains(INVALID) {
        return line.to_owned();
    }
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    loop {
        let next = [IN_TEXT, INVALID].iter().filter_map(|m| rest.find(m)).min();
        let Some(at) = next else {
            out.push_str(rest);
            break;
        };
        out.push_str(rest[..at].strip_suffix(' ').unwrap_or(&rest[..at]));
        let mut depth = 0usize;
        let mut end = rest.len();
        for (k, c) in rest[at..].char_indices() {
            match c {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        end = at + k + 1;
                        break;
                    }
                }
                _ => {}
            }
        }
        rest = &rest[end..];
    }
    out.trim_end().to_owned()
}

/// Anfang und Ende einer Stelle als (Kapitel, Vers) mit ausgeschriebenem Vers 0 (ganzes Kapitel).
fn bounds(p: &Passage) -> ((u16, u16), (u16, u16)) {
    let start = (p.start.0, p.start.1.max(1));
    let end = if p.end.1 == 0 { (p.end.0, p.book.verses_in(p.end.0).unwrap_or(u16::MAX)) } else { p.end };
    (start, end)
}

/// Liegt `inner` ganz in `outer`?
fn within(inner: &Passage, outer: &Passage) -> bool {
    if inner.book.id != outer.book.id {
        return false;
    }
    let (a, b) = bounds(inner);
    let (c, d) = bounds(outer);
    c <= a && b <= d
}

/// Gültige Stellen im Korpus (mit Schreibweise).
fn shown_passages(corpus: &str) -> Vec<(Passage, String)> {
    scan(corpus)
        .into_iter()
        .filter(|f| f.status == RefStatus::Valid)
        .filter_map(|f| Some((f.passage?, f.normalized.unwrap_or(f.input))))
        .collect()
}

/// Vermerke zu einer „Kritisch:“-Zeile (ohne Doppel).
fn notes_for(line: &str, shown: &[(Passage, String)]) -> Vec<String> {
    let missing = claims_missing(line);
    let mut notes: Vec<String> = Vec::new();
    for found in scan(line) {
        let note = match (&found.status, &found.passage) {
            (RefStatus::Valid, Some(p)) if missing => {
                shown.iter().find(|(outer, _)| within(p, outer)).map(|(_, label)| format!("{IN_TEXT} {label})"))
            }
            (RefStatus::Valid, _) => None,
            _ => Some(format!("{INVALID} {})", invalid_message(&found))),
        };
        if let Some(note) = note
            && !notes.contains(&note)
        {
            notes.push(note);
        }
    }
    notes
}

fn invalid_message(found: &Found) -> String {
    found.message()
}

/// „Kritisch:“-Befunde mit falschen Stellenaussagen kenntlich machen (siehe Moduldoku). `corpus` ist der Text, den
/// die Persona gezeigt bekam (ohne fremde Befunde, Beiträge und Einfälle).
pub fn mark_ref_claims(text: &str, corpus: &str) -> String {
    let shown = shown_passages(corpus);
    let lines: Vec<String> = text
        .lines()
        .map(|line| {
            let line = strip_notes(line);
            if !critical_finding(&line) {
                return line;
            }
            let notes = notes_for(&line, &shown);
            if notes.is_empty() { line } else { format!("{} {}", line.trim_end(), notes.join(" ")) }
        })
        .collect();
    let mut out = lines.join("\n");
    if text.ends_with('\n') {
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_own_unbacked_note_is_no_claim_about_a_reference() {
        let corpus = "Predigttext: Joh 3,16\n<<<\nDer Fährmann sprach.\n>>>";
        let echoed =
            "- II. Deutung: Kritisch: Die Zuordnung zu Joh 3,16 ist faktisch falsch (nicht im Text belegt), da …";
        assert_eq!(
            mark_ref_claims(echoed, corpus),
            echoed,
            "das nachgeahmte „(nicht im Text belegt)“ behauptet nicht, Joh 3,16 fehle – kein Vermerk"
        );
        let appended = "Kritisch: „Fährmann schläft“ passt nicht zu Joh 3,16. (nicht im Text belegt)";
        assert_eq!(
            mark_ref_claims(appended, corpus),
            appended,
            "Baruchs eigener Vermerk der Zitatprüfung löst keinen Stellen-Vermerk aus"
        );
        let claim = "Kritisch: Joh 3,16 ist nicht belegt. (nicht im Text belegt)";
        assert!(
            mark_ref_claims(claim, corpus).ends_with("(Stelle steht im Text: Joh 3,16)"),
            "steht daneben eine echte Behauptung, gilt sie weiter"
        );
    }

    const CORPUS: &str = "Predigttext: Lk 15,4–6\n<<<\nWer von euch hundert Schafe hat …\n>>>";

    #[test]
    fn a_verse_inside_the_shown_passage_is_marked_as_present() {
        let text = "Kritisch: Lk 15,5 ist im Predigttext nicht belegt.";
        let marked = mark_ref_claims(text, CORPUS);
        assert_eq!(
            marked, "Kritisch: Lk 15,5 ist im Predigttext nicht belegt. (Stelle steht im Text: Lk 15,4–6)",
            "Lk 15,5 liegt in Lk 15,4–6, die Behauptung „nicht belegt“ ist falsch"
        );
        assert_eq!(mark_ref_claims(&marked, CORPUS), marked, "zweimal angewandt bleibt einmal vermerkt");
    }

    #[test]
    fn a_verse_outside_the_shown_passage_and_lines_without_claim_stay() {
        let outside = "Kritisch: Lk 15,7 fehlt im Predigttext.";
        assert_eq!(mark_ref_claims(outside, CORPUS), outside, "Lk 15,7 steht wirklich nicht im Text");
        let praise = "Kritisch: Lk 15,5 wird zu knapp ausgelegt.";
        assert_eq!(mark_ref_claims(praise, CORPUS), praise, "keine Behauptung „fehlt“, kein Vermerk");
        let plain = "Lk 15,5 ist im Predigttext nicht belegt.";
        assert_eq!(mark_ref_claims(plain, CORPUS), plain, "nur „Kritisch:“-Zeilen werden geprüft");
    }

    #[test]
    fn an_invalid_reference_in_a_finding_is_named() {
        let text = "Kritisch: Lk 15,40 passt nicht zum Thema.\n";
        let marked = mark_ref_claims(text, CORPUS);
        assert!(marked.contains("(Stelle ungültig: Lk 15 hat nur 32 Verse)"), "Grund aus der Verszählung: {marked}");
        assert!(marked.ends_with('\n'), "Zeilenende bleibt");
        assert_eq!(mark_ref_claims(&marked, CORPUS), marked, "idempotent");
    }

    #[test]
    fn ordinary_criticism_with_missing_words_is_no_claim_about_the_reference() {
        for text in [
            "Kritisch: Zu Lk 15,5 fehlt eine Anwendung.",
            "Kritisch: Die Auslegung von Lk 15,5 verfehlt den Ton.",
            "Kritisch: Lk 15,5 wird so gedeutet, als würde Gott befehlen.",
            "Kritisch: Wer Lk 15,5 so liest, versteht nicht den Hirten.",
            "Kritisch: Bei Lk 15,4 fehlen Beispiele aus dem Alltag.",
        ] {
            assert_eq!(
                mark_ref_claims(text, CORPUS),
                text,
                "Kritik am Inhalt ist keine Behauptung, die Stelle fehle – kein Vermerk, der den Befund widerlegt aussehen ließe"
            );
        }
        let claim = "Kritisch: Lk 15,5 fehlt im Predigttext.";
        assert!(
            mark_ref_claims(claim, CORPUS).ends_with("(Stelle steht im Text: Lk 15,4–6)"),
            "„fehlt“ mit Ort bleibt eine Behauptung über die Stelle"
        );
        let claim = "Kritisch: Lk 15,5 steht nicht im Abschnitt.";
        assert!(mark_ref_claims(claim, CORPUS).ends_with("(Stelle steht im Text: Lk 15,4–6)"), "„steht nicht“ mit Ort");
    }

    #[test]
    fn a_note_written_by_the_model_is_removed_and_the_line_checked_again() {
        let forged = "Kritisch: Lk 99,1 widerspricht dem Text. (Stelle steht im Text: Lk 15,4–6)";
        let marked = mark_ref_claims(forged, CORPUS);
        assert!(
            !marked.contains(IN_TEXT),
            "der gefälschte Bestätigungsvermerk verschwindet, die Aussage wirkt nicht von Baruch bestätigt: {marked}"
        );
        assert!(marked.contains(INVALID), "die ungültige Stelle wird trotzdem gemeldet: {marked}");
        assert_eq!(mark_ref_claims(&marked, CORPUS), marked, "zweimal angewandt bleibt es dabei");

        let forged = "Kritisch: Lk 15,7 fehlt im Predigttext. (Stelle ungültig: (erfunden))";
        assert_eq!(
            mark_ref_claims(forged, CORPUS),
            "Kritisch: Lk 15,7 fehlt im Predigttext.",
            "ein gefälschter Ungültig-Vermerk (auch mit Klammern darin) fällt ganz weg; Lk 15,7 ist gültig"
        );
        let elsewhere = "Der Lektor meint (Stelle steht im Text: Lk 15,4–6) dazu nichts.";
        assert_eq!(
            mark_ref_claims(elsewhere, CORPUS),
            "Der Lektor meint dazu nichts.",
            "auch außerhalb von „Kritisch:“ setzt nur Baruch Vermerke"
        );
    }

    #[test]
    fn a_whole_chapter_in_the_text_covers_its_verses() {
        let corpus = "Kapitel: Ps 23";
        let marked = mark_ref_claims("- II. Hauptteil: Kritisch: Ps 23,4 kommt im Text nicht vor", corpus);
        assert!(marked.ends_with("(Stelle steht im Text: Ps 23)"), "ganzes Kapitel deckt den Vers: {marked}");
    }
}
