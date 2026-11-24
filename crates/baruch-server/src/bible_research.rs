// SPDX-License-Identifier: AGPL-3.0-or-later
//! Belege aus SWORD für die Personas (R15, Punkt 5c „Daten“): Bibelwortlaut, Kommentare und
//! Lexikon-Artikel zu den Stellen eines Texts. Die Darstellung im Kontext (Faktenblock, `[Sn]`) gehört
//! dem Lektor (`assist/bible_facts.rs`); hier entstehen nur die Daten, Schalter und Grenzen.
//!
//! - **Schalter und Erlaubnis** ([`research`] entscheidet allein): nur `@belege` (Vorgabe an) und die Persona,
//!   die nach der Wahl Lektor ist (Vorgabe aus), mit eingeschaltetem Schalter `persona.bible_set`
//!   ([`crate::personas::bible_state`]). Ein MCP-Auftrag an eine dieser Personas bekommt die Belege ebenso
//!   (lokale Daten, nichts geht hinaus).
//! - **Stellen:** nur, was [`baruch_scripture::scan`] im Text erkennt; ungültige Stellen liefern keinen Text,
//!   nur einen Hinweis in `problems`. Je Stelle (ohne Doppel) erst der Wortlaut der Werk-Bibel, dann je
//!   gewähltem Kommentar dessen Einträge (in der Zählung des Kommentars).
//! - **Lexika** (zuletzt, seit Runde 17b): zuerst die Strong-Nummern der Verse aller Stellen, wenn die Bibel des
//!   Werks Strong-Auszeichnung hat (`Bible::has_strongs`, `Bible::verse_strongs`), dann Strong-Nummern im Text
//!   (`G25`, `H430`), zuletzt großgeschriebene Wörter ab vier Buchstaben mit einer **Lemma-Probe** (das Wort, dann
//!   der Stamm ohne einfache Endung -ern/-en/-es/-er/-e/-n/-s gegen die Stichwörter des Lexikons, seit Runde 19c mit
//!   Umlaut-Rückführung „Bäume“ → „Baum“ und zuletzt dem letzten Bestandteil eines Kompositums; keine Grammatik). Strong-Nummern werden als „G25“, „G0025“, „G00025“ gesucht, die bloße Zahl nur in einem
//!   Lexikon derselben Sprache (conf `Feature=GreekDef`/`HebrewDef`). Je gewähltem Lexikon höchstens
//!   [`MAX_LEXICON_NOTES`] Artikel.
//! - **Grenzen:** Gesamtlänge aller `text` höchstens `max_chars` (je Beleg höchstens [`MAX_NOTE_CHARS`]),
//!   höchstens [`MAX_REFERENCES`] Stellen; was nicht mehr passt, nennt ein Hinweis. Klartext ohne Auszeichnung.

use std::sync::Arc;

use baruch_scripture::RefStatus;
use baruch_sword::{Bible, Commentary, Lexicon};

use crate::bibles::{Choice, Source};

/// Höchstlänge eines Belegs (Zeichen).
pub const MAX_NOTE_CHARS: usize = 1500;
/// Höchstzahl der Stellen, die Belege bekommen.
pub const MAX_REFERENCES: usize = 12;
/// Höchstzahl der Lexikon-Artikel je Lexikon.
pub const MAX_LEXICON_NOTES: usize = 5;
/// Höchstzahl der Stichwörter, die nachgeschlagen werden.
pub const MAX_KEYWORDS: usize = 200;

/// Art eines Belegs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum NoteKind {
    Bible,
    Commentary,
    Lexicon,
}

/// Ein Beleg aus einem SWORD-Modul.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ScriptureNote {
    pub kind: NoteKind,
    /// Modulname (conf-Abschnitt).
    pub module: String,
    /// Stelle in deutscher Schreibweise („Ps 139,23–24“) bzw. Stichwort des Lexikons.
    pub reference: String,
    /// Klartext ohne Markup, gekürzt (dann truncated = true).
    pub text: String,
    /// Lizenzvermerk der conf.
    pub notice: String,
    pub truncated: bool,
}

/// Ergebnis von [`research`].
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
pub struct Research {
    /// Belege in der Reihenfolge der Stellen im Text; je Stelle erst die Bibel, dann die Kommentare; Lexika zuletzt.
    pub notes: Vec<ScriptureNote>,
    /// Ehrliche Hinweise (Modul fehlt, Stelle gibt es nicht, Grenze erreicht …), je ein Satz.
    pub problems: Vec<String>,
}

/// Belege sammeln, ohne `max_chars` zu überschreiten.
struct Collector {
    out: Research,
    room: usize,
    full: bool,
}

impl Collector {
    /// Beleg anhängen (gekürzt auf den Platz); `false`, wenn kein Platz mehr ist.
    fn push(&mut self, kind: NoteKind, module: &str, reference: &str, text: &str, notice: &str) -> bool {
        let text = text.trim();
        if text.is_empty() {
            return true;
        }
        let max = MAX_NOTE_CHARS.min(self.room);
        if max < 40 {
            if !self.full {
                self.full = true;
                self.out.problems.push("Grenze erreicht: weitere Belege aus SWORD weggelassen.".to_owned());
            }
            return false;
        }
        let (text, truncated) = baruch_sword::reference::clip(text, max);
        self.room -= text.chars().count();
        self.out.notes.push(ScriptureNote {
            kind,
            module: module.to_owned(),
            reference: reference.to_owned(),
            text,
            notice: notice.to_owned(),
            truncated,
        });
        true
    }

    fn problem(&mut self, sentence: String) {
        if !self.out.problems.contains(&sentence) {
            self.out.problems.push(sentence);
        }
    }
}

/// Stichwörter für Lexika: Strong-Nummern und großgeschriebene Wörter ab vier Buchstaben, ohne Doppel, in der
/// Reihenfolge im Text.
pub fn keywords(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for word in text.split(|c: char| !c.is_alphanumeric()) {
        if out.len() >= MAX_KEYWORDS {
            break;
        }
        let noun = word.chars().count() >= 4
            && word.chars().next().is_some_and(char::is_uppercase)
            && word.chars().all(char::is_alphabetic);
        if (strong_number(word).is_some() || noun) && !out.iter().any(|w| w.eq_ignore_ascii_case(word)) {
            out.push(word.to_owned());
        }
    }
    out
}

/// Strong-Nummer in Normalform („G25“, „H430“; ohne führende Nullen), wenn `word` eine ist („G0025“ auch).
pub fn strong_number(word: &str) -> Option<String> {
    let prefix = word.chars().next().filter(|c| matches!(c, 'G' | 'H'))?;
    let digits = &word[1..];
    if digits.is_empty() || digits.len() > 5 || !digits.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let number: u32 = digits.parse().ok()?;
    (number > 0).then(|| format!("{prefix}{number}"))
}

/// Schreibweisen, unter denen ein Lexikon eine Strong-Nummer führen kann: „G25“, „G0025“, „G00025“ und –
/// nur bei einem Lexikon derselben Sprache (conf `Feature=GreekDef` bzw. `HebrewDef`) – die bloße Zahl
/// „00025“/„25“. So bekommt ein hebräisches Lexikon nie den Artikel zu einer griechischen Nummer.
pub fn strong_key_forms(strong: &str, features: &[String]) -> Vec<String> {
    let Some(normal) = strong_number(strong) else { return Vec::new() };
    let (prefix, number) = normal.split_at(1);
    let mut forms = vec![normal.clone(), format!("{prefix}{number:0>4}"), format!("{prefix}{number:0>5}")];
    let feature = if prefix == "G" { "GreekDef" } else { "HebrewDef" };
    if features.iter().any(|f| f.eq_ignore_ascii_case(feature)) {
        forms.push(format!("{number:0>5}"));
        forms.push(number.to_owned());
    }
    forms.dedup();
    forms
}

/// Kandidaten der Lemma-Probe: das Wort selbst, dann die Stämme ohne einfache Endung (-ern/-en/-es/-er/-e/-n/-s,
/// mindestens drei Buchstaben, längere zuerst) und seit Runde 19c dieselben Formen mit zurückgeführtem Umlaut
/// („Bäume“ → „Baum“); Komposita probiert [`baruch_sword::Lexicon::lookup_lemma`] zuletzt über den letzten
/// Bestandteil. Eine ehrliche Probe, keine Lemmatisierung ([`baruch_sword::reference::lemma_candidates`]).
pub fn lemma_candidates(word: &str) -> Vec<String> {
    baruch_sword::reference::lemma_candidates(word)
}

/// `Feature=`-Einträge der conf eines Moduls (für die Sprache eines Strong-Lexikons); leer, wenn die conf
/// nicht lesbar ist.
fn conf_features(info: &baruch_sword::BibleInfo) -> Vec<String> {
    let Ok(conf) = std::fs::read_to_string(&info.conf_file) else { return Vec::new() };
    conf.lines()
        .filter_map(|line| line.trim().strip_prefix("Feature="))
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .collect()
}

/// Ein Stichwort für die Lexika.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Keyword {
    /// Strong-Nummer (Normalform „G26“), aus der Auszeichnung der Bibel oder aus dem Text.
    Strong(String),
    /// Großgeschriebenes Wort aus dem Text (Lemma-Probe).
    Word(String),
}

/// Stichwörter in der Reihenfolge der Nachfrage: zuerst die Strong-Nummern der Verse (`verse_strongs`),
/// dann die aus dem Text, zuletzt die Wörter; ohne Doppel, höchstens [`MAX_KEYWORDS`].
fn lexicon_keywords(verse_strongs: &[String], text: &str) -> Vec<Keyword> {
    let mut out: Vec<Keyword> = Vec::new();
    let push = |k: Keyword, out: &mut Vec<Keyword>| {
        if out.len() < MAX_KEYWORDS && !out.contains(&k) {
            out.push(k);
        }
    };
    for strong in verse_strongs.iter().filter_map(|s| strong_number(s)) {
        push(Keyword::Strong(strong), &mut out);
    }
    for word in keywords(text) {
        match strong_number(&word) {
            Some(strong) => push(Keyword::Strong(strong), &mut out),
            None => push(Keyword::Word(word), &mut out),
        }
    }
    out
}

/// Artikel zu einem Stichwort: Strong-Nummer in ihren Schreibweisen, Wort über die Lemma-Probe.
fn lookup_keyword(
    lexicon: &Lexicon,
    features: &[String],
    keyword: &Keyword,
) -> Result<Option<baruch_sword::reference::LexiconArticle>, baruch_sword::BibleError> {
    match keyword {
        Keyword::Strong(strong) => {
            // `Lexicon::lookup` (sword-modules) streicht bei „G…“/„H…“ selbst das Präfix und sucht die bloße Zahl –
            // in jedem Lexikon. Darum hier nur Stichwörter, die es genau so gibt (über die Stichwortliste).
            for form in strong_key_forms(strong, features) {
                let listed = lexicon.keys(&form, 1)?;
                let Some(stored) = listed.keys.first().filter(|k| k.eq_ignore_ascii_case(&form)) else { continue };
                if let Some(mut article) = lexicon.lookup(stored)? {
                    article.key = stored.clone();
                    return Ok(Some(article));
                }
            }
            Ok(None)
        }
        // Runde 19c: Lemma-Probe aus baruch-sword (Endungen, Umlaut, zuletzt der letzte Bestandteil eines Kompositums).
        Keyword::Word(word) => Ok(lexicon.lookup_lemma(word)?.map(|(article, _)| article)),
    }
}

/// Geöffnete Quellen für die Sammlung (ohne Wartezeit beim Lesen).
struct Opened {
    bible: Option<Arc<Bible>>,
    commentaries: Vec<Arc<Commentary>>,
    lexicons: Vec<Arc<Lexicon>>,
}

/// Belege lesen (blockierend; im Server über `spawn_blocking`).
fn collect(opened: &Opened, text: &str, mut c: Collector) -> Research {
    let mut seen: Vec<String> = Vec::new();
    // Strong-Nummern der Verse aller Stellen (nur bei einer Bibel mit Strong-Auszeichnung), für die Lexika.
    let mut verse_strongs: Vec<String> = Vec::new();
    for found in baruch_scripture::scan(text) {
        let Some(passage) = found.passage.as_ref().filter(|_| matches!(found.status, RefStatus::Valid)) else {
            c.problem(format!("„{}“: {} – kein Text dazu.", found.input, found.message()));
            continue;
        };
        let label = found.normalized.clone().unwrap_or_else(|| found.input.clone());
        if seen.contains(&label) {
            continue;
        }
        if seen.len() >= MAX_REFERENCES {
            c.problem(format!("Mehr als {MAX_REFERENCES} Stellen: Belege nur für die ersten {MAX_REFERENCES}."));
            break;
        }
        seen.push(label.clone());
        let book = passage.book.id;
        if let Some(bible) = &opened.bible {
            // Runde 23a (Codereview quellen-1): Die Stelle kommt in deutscher Zählung aus der Stellenprüfung; gelesen
            // wird wie überall sonst über `bibles::read_passage` (Abbildung in die Zählung des Moduls). Vorher las
            // dieser Weg die deutschen Nummern unverändert im Modul und lieferte bei KJV-Zählung den falschen Vers.
            match crate::bibles::read_passage(bible, book, passage.start, passage.end) {
                Ok(read) => {
                    // Strong-Nummern der gelesenen Verse (Zählung des Moduls), nur bei Strong-Auszeichnung.
                    if bible.has_strongs() {
                        for (v, at) in read.verses.iter().zip(&read.module_refs).filter(|(v, _)| v.text.is_some()) {
                            if verse_strongs.len() >= MAX_KEYWORDS {
                                break;
                            }
                            match bible.verse_strongs(at.book, v.chapter, v.verse) {
                                Ok(numbers) => verse_strongs.extend(numbers),
                                Err(e) => c.problem(format!("{label}: Strong-Nummern nicht lesbar: {e}")),
                            }
                        }
                    }
                    let verses = &read.verses;
                    let missing = verses.iter().filter(|v| v.text.is_none()).count();
                    let words: Vec<&str> = verses.iter().filter_map(|v| v.text.as_deref()).collect();
                    if words.is_empty() {
                        c.problem(format!("{label}: Modul {} hat dort keinen Text.", bible.info.name));
                    } else {
                        if missing > 0 {
                            c.problem(format!("{label}: {missing} Vers(e) fehlen im Modul {}.", bible.info.name));
                        }
                        if !c.push(NoteKind::Bible, &bible.info.name, &label, &words.join(" "), &bible.info.notice) {
                            return c.out;
                        }
                    }
                }
                Err(e) => c.problem(format!("{label}: {e}")),
            }
        }
        for commentary in &opened.commentaries {
            // Runde 23a (Codereview quellen-2): auch der Kommentar in seiner Zählung, über die Abbildung.
            match crate::bibles::read_commentary(commentary, book, passage.start, passage.end) {
                Ok(read) if read.entries.is_empty() => {}
                Ok(read) => {
                    let single = read.entries.len() == 1;
                    let joined: Vec<String> = read
                        .entries
                        .iter()
                        .map(|e| if single { e.text.clone() } else { format!("{}: {}", read.label(e), e.text) })
                        .collect();
                    let info = &commentary.info;
                    if !c.push(NoteKind::Commentary, &info.name, &label, &joined.join(" "), &info.notice) {
                        return c.out;
                    }
                }
                Err(e) => c.problem(format!("{label}: Kommentar {}: {e}", commentary.info.name)),
            }
        }
    }
    let words = lexicon_keywords(&verse_strongs, text);
    for lexicon in &opened.lexicons {
        let features = conf_features(&lexicon.info);
        let mut taken = 0;
        for word in &words {
            if taken >= MAX_LEXICON_NOTES {
                break;
            }
            match lookup_keyword(lexicon, &features, word) {
                Ok(Some(article)) => {
                    taken += 1;
                    let info = &lexicon.info;
                    if !c.push(NoteKind::Lexicon, &info.name, &article.key, &article.text, &info.notice) {
                        return c.out;
                    }
                }
                Ok(None) => {}
                Err(e) => {
                    c.problem(format!("Lexikon {}: {e}", lexicon.info.name));
                    break;
                }
            }
        }
    }
    c.out
}

/// Belege aus SWORD zu den Bibelstellen (und, mit gewähltem Lexikon, Stichwörtern) in `text` für `persona` im Werk `work`.
/// None = die Persona bekommt keinen Block (Schalter aus, nicht erlaubt oder weder Bibel noch Kommentar noch Lexikon gewählt).
/// Gesamtlänge aller `text` höchstens `max_chars`; nie erfundene Stellen (nur, was baruch_scripture erkennt und das Modul liefert).
pub async fn research(
    state: &crate::AppState,
    work: baruch_store::WorkId,
    persona: &str,
    text: &str,
    max_chars: usize,
) -> Option<Research> {
    let (_, allowed, on, _) = crate::personas::bible_state(state, persona).await.ok()?;
    if !allowed || !on {
        return None;
    }
    let choice = crate::bibles::of_work(state, work).await.ok()?;
    let sources = crate::bibles::work_sources(state, work).await.ok()?;
    if matches!(choice, Choice::None) && sources.commentaries.is_empty() && sources.lexicons.is_empty() {
        return None;
    }
    let mut collector = Collector { out: Research::default(), room: max_chars, full: false };
    let bible = match choice {
        Choice::Ready(bible) => Some(bible),
        Choice::None => None,
        Choice::Missing(name) => {
            collector.problem(format!("Bibelmodul {name} in keinem Modulordner gefunden; kein Wortlaut."));
            None
        }
        Choice::Unusable(info, why) => {
            collector.problem(format!("Bibelmodul {}: {why}; kein Wortlaut.", info.name));
            None
        }
    };
    let mut commentaries = Vec::new();
    for name in &sources.commentaries {
        match crate::bibles::commentary_by_name(state, name).await.ok()? {
            Source::Ready(c) => commentaries.push(c),
            Source::Problem { module, problem, .. } => collector.problem(format!("Kommentar {module}: {problem}.")),
        }
    }
    let mut lexicons = Vec::new();
    for name in &sources.lexicons {
        match crate::bibles::lexicon_by_name(state, name).await.ok()? {
            Source::Ready(l) => lexicons.push(l),
            Source::Problem { module, problem, .. } => collector.problem(format!("Lexikon {module}: {problem}.")),
        }
    }
    let opened = Opened { bible, commentaries, lexicons };
    let text = text.to_owned();
    tokio::task::spawn_blocking(move || collect(&opened, &text, collector)).await.ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keywords_are_capitalised_words_and_strong_numbers_without_doubles() {
        let words = keywords("Der Kiesel und die Amsel am Bach; kiesel G25 und H430, Abend. Ab.");
        assert_eq!(
            words,
            vec!["Kiesel", "Amsel", "Bach", "G25", "H430", "Abend"],
            "Substantive ab vier Buchstaben, Strong-Nummern"
        );
    }

    #[test]
    fn the_lemma_probe_tries_the_word_and_simple_stems() {
        assert_eq!(lemma_candidates("Hirten"), ["Hirten", "Hirte", "Hirt"], "-en und -n abgeschnitten, Wort zuerst");
        assert_eq!(lemma_candidates("Schafe"), ["Schafe", "Schaf"], "-e abgeschnitten");
        assert_eq!(lemma_candidates("Kindern"), ["Kindern", "Kinder", "Kind"], "längerer Stamm zuerst");
        assert_eq!(lemma_candidates("Amsel"), ["Amsel"], "ohne passende Endung nur das Wort");
        assert_eq!(lemma_candidates("Ufer"), ["Ufer"], "Stamm unter drei Buchstaben wird nicht probiert");
    }

    #[test]
    fn strong_numbers_are_normalised_and_bare_numbers_only_for_the_right_language() {
        assert_eq!(strong_number("G0025").as_deref(), Some("G25"), "führende Nullen fallen weg");
        assert_eq!(strong_number("H430").as_deref(), Some("H430"), "hebräisch");
        assert_eq!(strong_number("Gott"), None, "ein Wort ist keine Nummer");
        assert_eq!(strong_number("G0"), None, "Null ist keine Strong-Nummer");
        assert_eq!(strong_key_forms("G25", &[]), ["G25", "G0025", "G00025"], "ohne Sprachangabe nur mit Präfix");
        let greek = vec!["GreekDef".to_owned()];
        assert!(
            strong_key_forms("G25", &greek).contains(&"00025".to_owned()),
            "ein griechisches Lexikon (Feature=GreekDef) bekommt auch die bloße Zahl"
        );
        assert!(
            !strong_key_forms("H25", &greek).contains(&"00025".to_owned()),
            "eine hebräische Nummer nie als bloße Zahl in einem griechischen Lexikon (falscher Artikel)"
        );
    }

    #[test]
    fn verse_strongs_come_first_then_text_numbers_then_words() {
        let keys = lexicon_keywords(&["G26".into(), "G0026".into(), "H430".into()], "Der Hirte G26 und Liebe");
        assert_eq!(
            keys,
            vec![
                Keyword::Strong("G26".into()),
                Keyword::Strong("H430".into()),
                Keyword::Word("Hirte".into()),
                Keyword::Word("Liebe".into()),
            ],
            "Strong-Nummern der Verse zuerst (ohne Doppel, auch in anderer Schreibweise), dann die Wörter"
        );
    }

    #[test]
    fn the_collector_keeps_the_total_length_and_says_so() {
        let mut c = Collector { out: Research::default(), room: 100, full: false };
        assert!(c.push(NoteKind::Bible, "M", "Ps 1,1", &"a".repeat(80), "PD"), "erster passt");
        assert!(!c.push(NoteKind::Bible, "M", "Ps 1,2", &"b".repeat(80), "PD"), "zweiter hat keinen Platz mehr");
        let total: usize = c.out.notes.iter().map(|n| n.text.chars().count()).sum();
        assert!(total <= 100, "Gesamtlänge eingehalten: {total}");
        assert!(c.out.problems.iter().any(|p| p.contains("Grenze erreicht")), "Hinweis: {:?}", c.out.problems);
    }
}
