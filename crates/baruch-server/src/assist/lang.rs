// SPDX-License-Identifier: AGPL-3.0-or-later
//! Markers of persona answers in both work languages (Round 38, interface R38-S1).
//!
//! A prompt prescribes the markers of the work language ([`markers`], [`language_rule`]). Every recognition
//! (critical findings, companion lines, "all done", findings parser, chat) accepts both languages, whatever the
//! work language is ([`ALL`]). For German every marker is literally the one used before Round 38.

use baruch_core::WorkLanguage;

use super::prompts;

/// The markers a persona writes at the start of a line, and the notes Baruch adds to an answer, in one language.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Markers {
    /// Mandatory finding of the auditor.
    pub critical: &'static str,
    pub suggestion: &'static str,
    pub note: &'static str,
    pub praise: &'static str,
    /// Opening line of the lector's review.
    pub what_works: &'static str,
    /// RFC: findings of the others that matter now.
    pub important_now: &'static str,
    /// RFC: findings of the others that can wait.
    pub can_wait: &'static str,
    /// Heading of the solutions the persona would apply now.
    pub my_proposal: &'static str,
    /// Feedback on an own finding, lower case.
    pub done: &'static str,
    /// Quoted wording a solution replaces.
    pub instead_of: &'static str,
    /// The explicit sentence when all own findings of the section seem resolved.
    pub all_done_sentence: &'static str,
    /// Note Baruch adds to a critical finding without a quotation from the text.
    pub unbacked_note: &'static str,
    /// Note Baruch adds to a critical finding without a solution.
    pub no_solution_note: &'static str,
}

/// German markers: literally the constants and prompt texts from before Round 38.
pub const DE: Markers = Markers {
    critical: "Kritisch:",
    suggestion: "Vorschlag:",
    note: "Hinweis:",
    praise: "Lob:",
    what_works: "Was trägt:",
    important_now: "Jetzt wichtig:",
    can_wait: "Kann warten:",
    my_proposal: "Mein Vorschlag:",
    done: prompts::DONE_MARK,
    instead_of: "statt:",
    all_done_sentence: prompts::ALL_DONE_SENTENCE,
    unbacked_note: prompts::UNBACKED_NOTE,
    no_solution_note: prompts::NO_SOLUTION_NOTE,
};

/// English markers.
pub const EN: Markers = Markers {
    critical: "Critical:",
    suggestion: "Suggestion:",
    note: "Note:",
    praise: "Praise:",
    what_works: "What works:",
    important_now: "Important now:",
    can_wait: "Can wait:",
    my_proposal: "My proposal:",
    done: "done:",
    instead_of: "instead of:",
    all_done_sentence: "All my findings on this section seem to be resolved.",
    unbacked_note: "(not backed by the text)",
    no_solution_note: "(no solution proposed)",
};

/// Both marker sets; a recognition goes through all of them.
pub const ALL: [&Markers; 2] = [&DE, &EN];

/// Further markers found in Round 38, same pattern as [`Markers`]: the solution lines below a critical finding and
/// the words of a "done" line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MoreMarkers {
    /// New wording of a solution (`- besser: "…"`).
    pub better: &'static str,
    /// Next step of a solution (`- Schritt: …`).
    pub step: &'static str,
    /// Word before the number in a "done" line ("Erledigt: Befund 7 …"), lower case.
    pub finding: &'static str,
    /// Word before the quoted place in a "done" line.
    pub place: &'static str,
    /// Headings of the outline of an answer besides `what_works`, lower case without colon.
    pub what_is_missing: &'static str,
    pub what_disturbs: &'static str,
}

/// Further German markers: literally the words used before Round 38.
pub const MORE_DE: MoreMarkers = MoreMarkers {
    better: "besser:",
    step: "Schritt:",
    finding: "befund",
    place: "Stelle",
    what_is_missing: "was fehlt",
    what_disturbs: "was stört",
};

/// Further English markers.
pub const MORE_EN: MoreMarkers = MoreMarkers {
    better: "better:",
    step: "Step:",
    finding: "finding",
    place: "place",
    what_is_missing: "what is missing",
    what_disturbs: "what disturbs",
};

/// Both sets of further markers; a recognition goes through all of them.
pub const MORE: [&MoreMarkers; 2] = [&MORE_DE, &MORE_EN];

/// Further markers of a work language.
pub fn more(language: WorkLanguage) -> &'static MoreMarkers {
    match language {
        WorkLanguage::De => &MORE_DE,
        WorkLanguage::En => &MORE_EN,
    }
}

/// A prompt text written with the German markers, with the markers of the work language instead. German: unchanged.
/// The prompt texts in [`prompts`] are English prose that names the German markers literally; no English word in
/// them equals a German marker, so replacing the markers is exact.
pub fn localized(text: &str, language: WorkLanguage) -> String {
    if language == WorkLanguage::De {
        return text.to_owned();
    }
    let (m, more) = (markers(language), more(language));
    let done_line = format!("Done: {} n ({}", more.finding, more.place);
    let pairs: [(&str, &str); 16] = [
        (DE.all_done_sentence, m.all_done_sentence),
        (DE.unbacked_note, m.unbacked_note),
        (DE.no_solution_note, m.no_solution_note),
        ("Erledigt: Befund n (Stelle", &done_line),
        (DE.my_proposal, m.my_proposal),
        (DE.critical, m.critical),
        (DE.suggestion, m.suggestion),
        (DE.note, m.note),
        (DE.praise, m.praise),
        (DE.what_works, m.what_works),
        (DE.important_now, m.important_now),
        (DE.can_wait, m.can_wait),
        (DE.instead_of, m.instead_of),
        (MORE_DE.better, more.better),
        (MORE_DE.step, more.step),
        (DE.done, m.done),
    ];
    let mut out = text.to_owned();
    for (german, other) in pairs {
        out = out.replace(german, other);
    }
    out
}

/// Work language of a model call: read from the language rule that [`prompts::effective_prompt_in`] puts below
/// every effective prompt. A call without the rule counts as German (every work from before Round 38).
pub fn of_prompt(system: &str) -> WorkLanguage {
    WorkLanguage::ALL
        .into_iter()
        .find(|language| *language != WorkLanguage::De && system.contains(&language_rule(*language)))
        .unwrap_or_default()
}

/// Markers of a work language.
pub fn markers(language: WorkLanguage) -> &'static Markers {
    match language {
        WorkLanguage::De => &DE,
        WorkLanguage::En => &EN,
    }
}

/// The rule placed below every effective prompt (also a stored one): answer in the work language and use its
/// markers literally. The sentence itself is always English.
pub fn language_rule(language: WorkLanguage) -> String {
    let m = markers(language);
    let line_markers = [
        m.critical,
        m.suggestion,
        m.note,
        m.praise,
        m.what_works,
        m.important_now,
        m.can_wait,
        m.my_proposal,
        m.done,
        m.instead_of,
        more(language).better,
        more(language).step,
    ];
    let listed: Vec<String> = line_markers.iter().map(|marker| format!("\"{marker}\"")).collect();
    format!(
        "Always answer in {name} – the language of this work – whatever language this prompt or the question uses. \
         Use these markers literally: {markers}. If all your findings on a section seem resolved, say so with \
         exactly this sentence: \"{all_done}\"",
        name = language.english_name(),
        markers = listed.join(", "),
        all_done = m.all_done_sentence,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn german_markers_are_the_ones_from_before_round_38() {
        assert_eq!(DE.critical, "Kritisch:", "existing answers and tests with German findings must be recognised");
        assert_eq!(DE.done, "erledigt:", "the feedback marker of Round 22 stays lower case");
        assert_eq!(
            DE.all_done_sentence, "Alle meine Befunde zu diesem Abschnitt scheinen erledigt.",
            "the sentence that waives the mandatory finding must stay literally the same for German works"
        );
        assert_eq!(DE.unbacked_note, "(nicht im Text belegt)", "notes already stored in posts keep their wording");
        assert_eq!(DE.no_solution_note, "(ohne Lösungsvorschlag)", "notes already stored in posts keep their wording");
    }

    #[test]
    fn markers_follow_the_work_language() {
        assert_eq!(markers(WorkLanguage::De), &DE, "a German work prescribes the German markers");
        assert_eq!(markers(WorkLanguage::En), &EN, "an English work prescribes the English markers");
        assert_eq!(ALL.len(), WorkLanguage::ALL.len(), "every work language needs a marker set for the recognition");
    }

    #[test]
    fn line_markers_end_with_a_colon_and_differ_between_the_languages() {
        for (de, en) in [
            (DE.critical, EN.critical),
            (DE.suggestion, EN.suggestion),
            (DE.note, EN.note),
            (DE.praise, EN.praise),
            (DE.what_works, EN.what_works),
            (DE.important_now, EN.important_now),
            (DE.can_wait, EN.can_wait),
            (DE.my_proposal, EN.my_proposal),
            (DE.done, EN.done),
            (DE.instead_of, EN.instead_of),
        ] {
            assert!(de.ends_with(':') && en.ends_with(':'), "a marker opens a line and ends with a colon: {de} / {en}");
            assert_ne!(
                de.to_lowercase(),
                en.to_lowercase(),
                "the bilingual recognition must be able to tell which language a marker is from"
            );
        }
    }

    #[test]
    fn the_language_rule_names_the_language_and_its_markers() {
        let en = language_rule(WorkLanguage::En);
        assert!(
            en.starts_with("Always answer in English – the language of this work –"),
            "the model must be told the work language by its English name: {en}"
        );
        for marker in ["\"Critical:\"", "\"What works:\"", "\"My proposal:\"", "\"instead of:\"", "\"done:\""] {
            assert!(en.contains(marker), "an English work prescribes the English marker {marker}: {en}");
        }
        assert!(!en.contains("Kritisch:"), "an English work must not prescribe German markers: {en}");
        assert!(en.contains(EN.all_done_sentence), "the waiving sentence is prescribed in the work language: {en}");

        let de = language_rule(WorkLanguage::De);
        assert!(de.starts_with("Always answer in German –"), "the rule itself stays English, the language is German");
        for marker in ["\"Kritisch:\"", "\"Was trägt:\"", "\"Mein Vorschlag:\"", "\"statt:\"", "\"erledigt:\""] {
            assert!(de.contains(marker), "a German work prescribes the German marker {marker}: {de}");
        }
        assert!(!de.contains("Critical:"), "a German work must not prescribe English markers: {de}");
    }
}
