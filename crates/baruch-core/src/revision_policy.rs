// SPDX-License-Identifier: AGPL-3.0-or-later
//! Fassungsmodell (Runde 22, Vertrag §2.1–2.3): Fortschreiben der aktuellen Fassung oder neue Fassung.
//!
//! Speichert der Mensch im Editor (Modus [`SaveMode::Auto`]) auf der aktuellen Fassung und gilt keine Regel für
//! eine neue Fassung, wird die aktuelle Fassung fortgeschrieben. [`decide`] prüft die Regeln in der festen
//! Reihenfolge aus §2.2; der erste Treffer liefert den Grund ([`NewReason`]). Die Einstellung je Werk ist
//! [`RevisionPolicy`] (Tabelle `work_revision_policy`, fehlt die Zeile → [`RevisionPolicy::default`]).

use similar::TextDiff;

/// Einstellung „Fassungen“ je Werk.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RevisionPolicy {
    /// Fortschreiben eingeschaltet.
    pub amend: bool,
    /// Ab dieser Änderung (Prozent, 1–100) entsteht eine neue Fassung.
    pub max_change_percent: u8,
    /// Nach so vielen Minuten seit dem letzten Schreiben entsteht eine neue Fassung (0 = keine Zeitgrenze).
    pub new_after_minutes: u32,
}

/// Vorgabe seit 2026-11-27 (Nutzer: „Strg+S als normales Speichern“): Speichern schreibt die eigene aktuelle Fassung
/// immer fort, wie in einer Textverarbeitung – keine Größen- und keine Zeitgrenze (100 %, 0 min). Jeder Stand bleibt im
/// Verlauf der Fortschreibungen; eine neue Fassung gibt es auf Wunsch oder nach den festen Regeln (fremde Fassung,
/// andere Basis, Überschrift/Gliederung). Die Grenzen lassen sich je Werk wieder einschalten.
impl Default for RevisionPolicy {
    fn default() -> Self {
        RevisionPolicy { amend: true, max_change_percent: 100, new_after_minutes: 0 }
    }
}

/// Grenzen von [`RevisionPolicy::max_change_percent`] (einschließlich).
pub const MAX_CHANGE_PERCENT: (u8, u8) = (1, 100);
/// Grenzen von [`RevisionPolicy::new_after_minutes`] (einschließlich).
pub const NEW_AFTER_MINUTES: (u32, u32) = (0, 1440);

impl RevisionPolicy {
    /// Prüft die Grenzen beider Zahlen (deutsche Meldung).
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

/// Speichermodus des Editors: `Auto` = fortschreiben, wenn die Regeln es erlauben; `New` = immer neue Fassung.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SaveMode {
    Auto,
    New,
}

impl SaveMode {
    /// Parameter `mode` aus RPC/Formular: fehlt er, gilt `New` (Verhalten wie vor Runde 22).
    pub fn from_param(s: Option<&str>) -> Result<SaveMode, String> {
        match s {
            None | Some("new") => Ok(SaveMode::New),
            Some("auto") => Ok(SaveMode::Auto),
            Some(other) => Err(format!("Unbekannter Speichermodus „{other}“ (erlaubt: „auto“, „new“).")),
        }
    }
}

/// Grund für eine neue Fassung (Reihenfolge der Prüfung wie die Varianten).
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
    /// Kennung (= serde-Wert).
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

    /// Deutsche Bezeichnung für die Oberfläche.
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

/// Tatsachen eines Speicherns, die [`decide`] braucht.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SaveFacts {
    pub has_current: bool,
    pub current_by_human: bool,
    pub base_is_current: bool,
    pub heading_changed: bool,
    pub change_percent: f64,
    pub minutes_since_last_write: u64,
}

/// None = fortschreiben, Some(Grund) = neue Fassung (Reihenfolge §2.2).
///
/// Reihenfolge: ausdrücklich gewünscht (`SaveMode::New`) → keine aktuelle Fassung → Fortschreiben ausgeschaltet →
/// aktuelle Fassung nicht vom Menschen (Persona, MCP, übernommener Vorschlag) → nicht auf der aktuellen Fassung
/// bearbeitet → Überschrift/Ebene/Teilen/Zusammenführen → Änderung über `max_change_percent` → Zeitgrenze
/// (`new_after_minutes > 0` und seit dem letzten Schreiben mindestens so viele Minuten). Keine Regel greift →
/// `None` (fortschreiben).
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

/// Änderung in Prozent: (1 − Wortähnlichkeit) · 100 (`similar::TextDiff::from_words`).
pub fn change_percent(old: &str, new: &str) -> f64 {
    let ratio = TextDiff::from_words(old, new).ratio();
    (1.0 - f64::from(ratio)) * 100.0
}

/// Wort-Unterschied zweier Texte (für den Verlauf der Fortschreibungen): Stücke in Reihenfolge, je gleich, neu oder
/// entfernt; aufeinanderfolgende Stücke derselben Art sind zusammengefasst, Leerraum bleibt erhalten.
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

    /// Die frühere Vorgabe (20 %, 30 min): an ihr prüfen die Tests die Grenzregeln.
    fn limited() -> RevisionPolicy {
        RevisionPolicy { amend: true, max_change_percent: 20, new_after_minutes: 30 }
    }

    /// Tatsachen, unter denen fortgeschrieben wird (eigene aktuelle Fassung, kleine Änderung, eben geschrieben).
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
        // Ganz anderer Text ohne gemeinsame Teile (auch ohne Leerraum): 100 %.
        let p = change_percent("Leuchtturmwärter", "Möwenschwarm");
        assert!(p > 99.0, "alles anders: nahe 100 % (war {p})");
        // Vertragsformel zählt die Leerräume zwischen den Wörtern als gleiche Teile: Ein ganz neuer Satz mit gleich
        // vielen Wörtern liegt darum bei etwa der Hälfte – immer noch weit über der Vorgabe von 20 %.
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
