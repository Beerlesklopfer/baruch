// SPDX-License-Identifier: AGPL-3.0-or-later
//! Evidence from SWORD for the personas (R15, item 5c “Daten”): Bible wording, commentaries and
//! lexicon articles on the references of a text. The presentation in the context (fact block, `[Sn]`) belongs
//! to the lector (`assist/bible_facts.rs`); here only the data, switches and limits arise.
//!
//! - **Switch and permission** ([`research`] decides alone): only `@belege` (default on) and the persona
//!   that is lector by the choice (default off), with the switch `persona.bible_set` on
//!   ([`crate::personas::bible_state`]). An MCP order to one of these personas gets the evidence as well
//!   (local data, nothing goes out).
//! - **References:** only what [`baruch_scripture::scan`] recognises in the text; invalid references yield no text,
//!   only a note in `problems`. Per reference (no duplicates) first the wording of the work's Bible, then per
//!   chosen commentary its entries (in the versification of the commentary).
//! - **Lexicons** (last, since Round 17b): first the Strong numbers of the verses of all references if the Bible of
//!   the work has Strong markup (`Bible::has_strongs`, `Bible::verse_strongs`), then Strong numbers in the text
//!   (`G25`, `H430`), last capitalised words from four letters on with a **lemma probe** (the word, then
//!   the stem without a simple ending -ern/-en/-es/-er/-e/-n/-s against the keywords of the lexicon, since Round 19c
//!   with umlaut reversal „Bäume“ → „Baum“ and last the last component of a compound; no grammar). Strong numbers are
//!   looked up as „G25“, „G0025“, „G00025“, the bare number only in a lexicon of the same language (conf
//!   `Feature=GreekDef`/`HebrewDef`). Per chosen lexicon at most [`MAX_LEXICON_NOTES`] articles.
//! - **Limits:** total length of all `text` at most `max_chars` (per piece of evidence at most [`MAX_NOTE_CHARS`]),
//!   at most [`MAX_REFERENCES`] references; what no longer fits is named by a note. Plain text without markup.

use std::sync::Arc;

use baruch_scripture::RefStatus;
use baruch_sword::{Bible, Commentary, Lexicon};

use crate::bibles::{Choice, Source};

/// Maximum length of a piece of evidence (characters).
pub const MAX_NOTE_CHARS: usize = 1500;
/// Maximum number of references that get evidence.
pub const MAX_REFERENCES: usize = 12;
/// Maximum number of lexicon articles per lexicon.
pub const MAX_LEXICON_NOTES: usize = 5;
/// Maximum number of keywords that are looked up.
pub const MAX_KEYWORDS: usize = 200;

/// Kind of a piece of evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum NoteKind {
    Bible,
    Commentary,
    Lexicon,
}

/// A piece of evidence from a SWORD module.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ScriptureNote {
    pub kind: NoteKind,
    /// Module name (conf section).
    pub module: String,
    /// Reference in German spelling („Ps 139,23–24“) or keyword of the lexicon.
    pub reference: String,
    /// Plain text without markup, shortened (then truncated = true).
    pub text: String,
    /// Licence notice of the conf.
    pub notice: String,
    pub truncated: bool,
}

/// Result of [`research`].
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
pub struct Research {
    /// Evidence in the order of the references in the text; per reference Bible, then commentaries; lexicons last.
    pub notes: Vec<ScriptureNote>,
    /// Honest notes (module missing, reference does not exist, limit reached …), one sentence each.
    pub problems: Vec<String>,
}

/// Collect evidence without exceeding `max_chars`.
struct Collector {
    out: Research,
    room: usize,
    full: bool,
}

impl Collector {
    /// Append evidence (shortened to the room); `false` if no room is left.
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

/// Keywords for lexicons: Strong numbers and capitalised words from four letters on, no duplicates, in the
/// order in the text.
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

/// Strong number in normal form („G25“, „H430“; without leading zeros) if `word` is one („G0025“ too).
pub fn strong_number(word: &str) -> Option<String> {
    let prefix = word.chars().next().filter(|c| matches!(c, 'G' | 'H'))?;
    let digits = &word[1..];
    if digits.is_empty() || digits.len() > 5 || !digits.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let number: u32 = digits.parse().ok()?;
    (number > 0).then(|| format!("{prefix}{number}"))
}

/// Spellings under which a lexicon may list a Strong number: „G25“, „G0025“, „G00025“ and –
/// only for a lexicon of the same language (conf `Feature=GreekDef` or `HebrewDef`) – the bare number
/// „00025“/„25“. So a Hebrew lexicon never gets the article for a Greek number.
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

/// Candidates of the lemma probe: the word itself, then the stems without a simple ending (-ern/-en/-es/-er/-e/-n/-s,
/// at least three letters, longer ones first) and since Round 19c the same forms with reversed umlaut
/// („Bäume“ → „Baum“); compounds are tried last by [`baruch_sword::Lexicon::lookup_lemma`] through the last
/// component. An honest probe, no lemmatisation ([`baruch_sword::reference::lemma_candidates`]).
pub fn lemma_candidates(word: &str) -> Vec<String> {
    baruch_sword::reference::lemma_candidates(word)
}

/// `Feature=` entries of the conf of a module (for the language of a Strong lexicon); empty if the conf
/// is not readable.
fn conf_features(info: &baruch_sword::BibleInfo) -> Vec<String> {
    let Ok(conf) = std::fs::read_to_string(&info.conf_file) else { return Vec::new() };
    conf.lines()
        .filter_map(|line| line.trim().strip_prefix("Feature="))
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .collect()
}

/// A keyword for the lexicons.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Keyword {
    /// Strong number (normal form „G26“), from the markup of the Bible or from the text.
    Strong(String),
    /// Capitalised word from the text (lemma probe).
    Word(String),
}

/// Keywords in the order of the lookup: first the Strong numbers of the verses (`verse_strongs`),
/// then those from the text, last the words; no duplicates, at most [`MAX_KEYWORDS`].
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

/// Articles for a keyword: Strong number in its spellings, word through the lemma probe.
fn lookup_keyword(
    lexicon: &Lexicon,
    features: &[String],
    keyword: &Keyword,
) -> Result<Option<baruch_sword::reference::LexiconArticle>, baruch_sword::BibleError> {
    match keyword {
        Keyword::Strong(strong) => {
            // `Lexicon::lookup` (sword-modules) itself strips the prefix of „G…“/„H…“ and looks up the bare number –
            // in every lexicon. So here only keywords that exist exactly like that (through the keyword list).
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
        // Round 19c: lemma probe from baruch-sword (endings, umlaut, last the last component of a compound).
        Keyword::Word(word) => Ok(lexicon.lookup_lemma(word)?.map(|(article, _)| article)),
    }
}

/// Opened sources for the collection (no waiting time when reading).
struct Opened {
    bible: Option<Arc<Bible>>,
    commentaries: Vec<Arc<Commentary>>,
    lexicons: Vec<Arc<Lexicon>>,
}

/// Read evidence (blocking; in the server through `spawn_blocking`).
fn collect(opened: &Opened, text: &str, mut c: Collector) -> Research {
    let mut seen: Vec<String> = Vec::new();
    // Strong numbers of the verses of all references (only for a Bible with Strong markup), for the lexicons.
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
            // Round 23a (code review quellen-1): the reference comes in German versification from the reference
            // check; it is read as everywhere else through `bibles::read_passage` (mapping into the module's
            // versification). Before, this path read the German numbers unchanged and returned the wrong KJV verse.
            match crate::bibles::read_passage(bible, book, passage.start, passage.end) {
                Ok(read) => {
                    // Strong numbers of the verses read (versification of the module), only with Strong markup.
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
            // Round 23a (code review quellen-2): the commentary too in its versification, through the mapping.
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

/// Evidence from SWORD on the Bible references (and, with a chosen lexicon, keywords) in `text` for `persona` in
/// `work`. None = no block (switch off, not allowed, or no Bible, commentary or lexicon chosen). Total of all `text`
/// at most `max_chars`; never invented references (only what baruch_scripture recognises and the module returns).
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
    tokio::task::spawn_blocking(crate::i18n::carry(move || collect(&opened, &text, collector))).await.ok()
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
