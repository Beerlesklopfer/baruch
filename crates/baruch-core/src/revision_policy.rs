// SPDX-License-Identifier: AGPL-3.0-or-later
//! Revision model (Round 22, contract §2.1–2.3): amending the current revision or a new revision.
//!
//! If the human saves in the editor (mode [`SaveMode::Auto`]) on the current revision and no rule for a new
//! revision applies, the current revision is amended. [`decide`] checks the rules in the fixed order of §2.2; the
//! first hit yields the reason ([`NewReason`]). The setting per work is [`RevisionPolicy`] (table
//! `work_revision_policy`, if the row is missing → [`RevisionPolicy::default`]).

use similar::TextDiff;

/// Setting „Fassungen“ per work.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RevisionPolicy {
    /// Amending switched on.
    pub amend: bool,
    /// From this change on (percent, 1–100) a new revision is created.
    pub max_change_percent: u8,
    /// After this many minutes since the last write a new revision is created (0 = no time limit).
    pub new_after_minutes: u32,
}

/// Default since 2026-11-27 (user: „Strg+S als normales Speichern“): saving always amends the own current revision,
/// as in a word processor – no size limit and no time limit (100 %, 0 min). Every state stays in the history of the
/// amendments; a new revision exists on request or by the fixed rules (foreign revision, other base,
/// heading/outline). The limits can be switched on again per work.
impl Default for RevisionPolicy {
    fn default() -> Self {
        RevisionPolicy { amend: true, max_change_percent: 100, new_after_minutes: 0 }
    }
}

/// Limits of [`RevisionPolicy::max_change_percent`] (inclusive).
pub const MAX_CHANGE_PERCENT: (u8, u8) = (1, 100);
/// Limits of [`RevisionPolicy::new_after_minutes`] (inclusive).
pub const NEW_AFTER_MINUTES: (u32, u32) = (0, 1440);

impl RevisionPolicy {
    /// Checks the limits of both numbers (German message).
    pub fn validate(&self) -> Result<(), String> {
        let (lo, hi) = MAX_CHANGE_PERCENT;
        if !(lo..=hi).contains(&self.max_change_percent) {
            return Err(format!("Größte Änderung: {lo} bis {hi} Prozent (nicht {}).", self.max_change_percent));
        }
        let (lo, hi) = NEW_AFTER_MINUTES;
        if !(lo..=hi).contains(&self.new_after_minutes) {
            return Err(format!(
                "Neue Fassung nach: {lo} bis {hi} Minuten (0 = keine Zeitgrenze, nicht {}).",
                self.new_after_minutes
            ));
        }
        Ok(())
    }
}

/// Save mode of the editor: `Auto` = amend if the rules allow it; `New` = always a new revision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SaveMode {
    Auto,
    New,
}

impl SaveMode {
    /// Parameter `mode` from RPC/form: if it is missing, `New` applies (behaviour as before Round 22).
    pub fn from_param(s: Option<&str>) -> Result<SaveMode, String> {
        match s {
            None | Some("new") => Ok(SaveMode::New),
            Some("auto") => Ok(SaveMode::Auto),
            Some(other) => Err(format!("Unbekannter Speichermodus „{other}“ (erlaubt: „auto“, „new“).")),
        }
    }
}

/// Reason for a new revision (order of the check like the variants).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NewReason {
    Requested,
    NoCurrent,
    PolicyOff,
    NotOwn,
    NotOnCurrent,
    HeadingChanged,
    LargeChange,
    TimeElapsed,
}

impl NewReason {
    /// Identifier (= serde value).
    pub fn id(self) -> &'static str {
        match self {
            NewReason::Requested => "requested",
            NewReason::NoCurrent => "no_current",
            NewReason::PolicyOff => "policy_off",
            NewReason::NotOwn => "not_own",
            NewReason::NotOnCurrent => "not_on_current",
            NewReason::HeadingChanged => "heading_changed",
            NewReason::LargeChange => "large_change",
            NewReason::TimeElapsed => "time_elapsed",
        }
    }

    /// German label for the user interface.
    pub fn label(self) -> &'static str {
        match self {
            NewReason::Requested => "als neue Fassung gewünscht",
            NewReason::NoCurrent => "keine aktuelle Fassung",
            NewReason::PolicyOff => "Fortschreiben ausgeschaltet",
            NewReason::NotOwn => "aktuelle Fassung stammt nicht vom Menschen",
            NewReason::NotOnCurrent => "nicht auf der aktuellen Fassung bearbeitet",
            NewReason::HeadingChanged => "Überschrift oder Gliederung geändert",
            NewReason::LargeChange => "große Änderung",
            NewReason::TimeElapsed => "Zeitgrenze überschritten",
        }
    }
}

/// Facts of a save that [`decide`] needs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SaveFacts {
    pub has_current: bool,
    pub current_by_human: bool,
    pub base_is_current: bool,
    pub heading_changed: bool,
    pub change_percent: f64,
    pub minutes_since_last_write: u64,
}

/// None = amend, Some(reason) = new revision (order §2.2).
///
/// Order: explicitly requested (`SaveMode::New`) → no current revision → amending switched off → current revision
/// not by the human (persona, MCP, accepted proposal) → not edited on the current revision →
/// heading/level/split/merge → change above `max_change_percent` → time limit (`new_after_minutes > 0` and at least
/// that many minutes since the last write). No rule applies → `None` (amend).
pub fn decide(policy: &RevisionPolicy, mode: SaveMode, facts: &SaveFacts) -> Option<NewReason> {
    if mode == SaveMode::New {
        return Some(NewReason::Requested);
    }
    if !facts.has_current {
        return Some(NewReason::NoCurrent);
    }
    if !policy.amend {
        return Some(NewReason::PolicyOff);
    }
    if !facts.current_by_human {
        return Some(NewReason::NotOwn);
    }
    if !facts.base_is_current {
        return Some(NewReason::NotOnCurrent);
    }
    if facts.heading_changed {
        return Some(NewReason::HeadingChanged);
    }
    if facts.change_percent > f64::from(policy.max_change_percent) {
        return Some(NewReason::LargeChange);
    }
    if policy.new_after_minutes > 0 && facts.minutes_since_last_write >= u64::from(policy.new_after_minutes) {
        return Some(NewReason::TimeElapsed);
    }
    None
}

/// Change in percent: (1 − word similarity) · 100 (`similar::TextDiff::from_words`).
pub fn change_percent(old: &str, new: &str) -> f64 {
    let ratio = TextDiff::from_words(old, new).ratio();
    (1.0 - f64::from(ratio)) * 100.0
}

/// Word difference of two texts (for the history of the amendments): pieces in order, each equal, new or removed;
/// consecutive pieces of the same kind are merged, whitespace is kept.
pub fn word_diff(old: &str, new: &str) -> Vec<(crate::DiffKind, String)> {
    let mut out: Vec<(crate::DiffKind, String)> = Vec::new();
    for change in TextDiff::from_words(old, new).iter_all_changes() {
        let kind = match change.tag() {
            similar::ChangeTag::Equal => crate::DiffKind::Same,
            similar::ChangeTag::Insert => crate::DiffKind::Added,
            similar::ChangeTag::Delete => crate::DiffKind::Removed,
        };
        match out.last_mut() {
            Some((last, text)) if *last == kind => text.push_str(change.value()),
            _ => out.push((kind, change.value().to_owned())),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The former default (20 %, 30 min): the tests check the limit rules against it.
    fn limited() -> RevisionPolicy {
        RevisionPolicy { amend: true, max_change_percent: 20, new_after_minutes: 30 }
    }

    /// Facts under which amending happens (own current revision, small change, just written).
    fn amendable() -> SaveFacts {
        SaveFacts {
            has_current: true,
            current_by_human: true,
            base_is_current: true,
            heading_changed: false,
            change_percent: 5.0,
            minutes_since_last_write: 2,
        }
    }

    #[test]
    fn a_small_own_change_on_the_current_version_is_amended() {
        let policy = limited();
        assert_eq!(decide(&policy, SaveMode::Auto, &amendable()), None, "kleine eigene Änderung: fortschreiben");
        let edge = SaveFacts { change_percent: 20.0, minutes_since_last_write: 29, ..amendable() };
        assert_eq!(decide(&policy, SaveMode::Auto, &edge), None, "genau 20 % und 29 min liegen noch innerhalb");
    }

    #[test]
    fn every_rule_gives_its_own_reason() {
        let policy = limited();
        let cases = [
            (SaveFacts { has_current: false, ..amendable() }, NewReason::NoCurrent, "ohne aktuelle Fassung"),
            (SaveFacts { current_by_human: false, ..amendable() }, NewReason::NotOwn, "Fassung einer Persona"),
            (SaveFacts { base_is_current: false, ..amendable() }, NewReason::NotOnCurrent, "andere Basis"),
            (SaveFacts { heading_changed: true, ..amendable() }, NewReason::HeadingChanged, "Überschrift geändert"),
            (SaveFacts { change_percent: 20.5, ..amendable() }, NewReason::LargeChange, "über 20 %"),
            (SaveFacts { minutes_since_last_write: 30, ..amendable() }, NewReason::TimeElapsed, "30 min vergangen"),
        ];
        for (facts, reason, why) in cases {
            assert_eq!(decide(&policy, SaveMode::Auto, &facts), Some(reason), "{why}: {facts:?}");
        }
        assert_eq!(
            decide(&policy, SaveMode::New, &amendable()),
            Some(NewReason::Requested),
            "Modus „new“ verlangt immer eine neue Fassung"
        );
        let off = RevisionPolicy { amend: false, ..policy };
        assert_eq!(decide(&off, SaveMode::Auto, &amendable()), Some(NewReason::PolicyOff), "ausgeschaltet");
    }

    #[test]
    fn the_first_matching_rule_wins_in_the_agreed_order() {
        let off = RevisionPolicy { amend: false, ..limited() };
        let everything = SaveFacts {
            has_current: true,
            current_by_human: false,
            base_is_current: false,
            heading_changed: true,
            change_percent: 90.0,
            minutes_since_last_write: 999,
        };
        assert_eq!(decide(&off, SaveMode::New, &everything), Some(NewReason::Requested), "requested zuerst");
        let no_current = SaveFacts { has_current: false, ..everything };
        assert_eq!(decide(&off, SaveMode::Auto, &no_current), Some(NewReason::NoCurrent), "dann no_current");
        assert_eq!(decide(&off, SaveMode::Auto, &everything), Some(NewReason::PolicyOff), "dann policy_off");
        let on = limited();
        assert_eq!(decide(&on, SaveMode::Auto, &everything), Some(NewReason::NotOwn), "dann not_own");
        let own = SaveFacts { current_by_human: true, ..everything };
        assert_eq!(decide(&on, SaveMode::Auto, &own), Some(NewReason::NotOnCurrent), "dann not_on_current");
        let on_current = SaveFacts { base_is_current: true, ..own };
        assert_eq!(decide(&on, SaveMode::Auto, &on_current), Some(NewReason::HeadingChanged), "dann Überschrift");
        let same_heading = SaveFacts { heading_changed: false, ..on_current };
        assert_eq!(decide(&on, SaveMode::Auto, &same_heading), Some(NewReason::LargeChange), "dann große Änderung");
    }

    #[test]
    fn the_word_diff_marks_only_the_changed_words() {
        use crate::DiffKind::{Added, Removed, Same};
        let parts = word_diff("Die Möwe ruft laut.", "Die Möwe ruft leise.");
        assert_eq!(
            parts,
            vec![(Same, "Die Möwe ruft ".to_owned()), (Removed, "laut.".to_owned()), (Added, "leise.".to_owned())],
            "gleiche Wörter zusammengefasst, nur das geänderte Wort markiert"
        );
        let joined: String =
            word_diff("a b", "a c d").iter().filter(|(k, _)| *k != Removed).map(|(_, t)| t.as_str()).collect();
        assert_eq!(joined, "a c d", "ohne Entferntes ergibt sich der neue Text");
    }

    #[test]
    fn zero_minutes_means_no_time_limit() {
        let policy = RevisionPolicy { new_after_minutes: 0, ..limited() };
        let late = SaveFacts { minutes_since_last_write: 100_000, ..amendable() };
        assert_eq!(decide(&policy, SaveMode::Auto, &late), None, "0 = keine Zeitgrenze, also fortschreiben");
    }

    #[test]
    fn change_percent_is_zero_for_equal_texts_and_about_hundred_for_different_ones() {
        let text = "Der Leuchtturm am Kap stand schief, doch er leuchtete jede Nacht.";
        assert!(change_percent(text, text).abs() < f64::EPSILON, "gleicher Text: 0 %");
        // Entirely different text without common parts (also without whitespace): 100 %.
        let p = change_percent("Leuchtturmwärter", "Möwenschwarm");
        assert!(p > 99.0, "alles anders: nahe 100 % (war {p})");
        // The contract formula counts the whitespace between the words as equal parts: an entirely new sentence
        // with the same number of words therefore lies at about half – still far above the default of 20 %.
        let other = "Elf Möwen zählten Muscheln unter grünem Himmel bei Ebbe, ganz still und froh.";
        let p = change_percent(text, other);
        assert!(p > 40.0, "neuer Satz: deutlich über 20 % (war {p})");
        let small = "Der Leuchtturm am Kap stand schief, doch er leuchtete jede Nacht hell.";
        let q = change_percent(text, small);
        assert!(q > 0.0 && q < 20.0, "ein Wort mehr: kleine Änderung (war {q})");
    }

    #[test]
    fn mode_parameter_and_policy_limits_are_checked() {
        assert_eq!(SaveMode::from_param(None), Ok(SaveMode::New), "fehlt = new (wie vor Runde 22)");
        assert_eq!(SaveMode::from_param(Some("auto")), Ok(SaveMode::Auto));
        assert!(SaveMode::from_param(Some("immer")).is_err(), "Unbekanntes abgelehnt");
        assert!(limited().validate().is_ok(), "Vorgabe gültig");
        let zero = RevisionPolicy { max_change_percent: 0, ..limited() };
        assert!(zero.validate().unwrap_err().contains("1 bis 100"), "0 % abgelehnt mit Grenzen");
        let long = RevisionPolicy { new_after_minutes: 1441, ..limited() };
        assert!(long.validate().unwrap_err().contains("0 bis 1440"), "über einen Tag abgelehnt");
        assert_eq!(NewReason::LargeChange.id(), "large_change", "id = serde-Wert");
        assert_eq!(NewReason::LargeChange.label(), "große Änderung");
    }
}
