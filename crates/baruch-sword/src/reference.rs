// SPDX-License-Identifier: AGPL-3.0-or-later
//! Kommentare und Lexika als Recherchewerkzeug (Runde 15, Punkt 5a).
//!
//! - [`Commentary`]: Kommentar zu einer Stelle in der **Verszählung des Moduls** (jede, die Baruch kennt), Treiber
//!   RawCom, RawCom4, zCom, zCom4. Stellen, die sich einen gespeicherten Eintrag teilen (ein Kommentar zu
//!   mehreren Versen), werden zu einem [`CommentaryEntry`] mit Bereich zusammengefasst. Lineare Suche.
//! - [`Lexicon`]: Stichwort → Artikel, Stichwortliste (mit Präfix) und Suche in Stichwort und Text, Treiber
//!   RawLD, RawLD4, zLD.
//!
//! Runde 19c: **Lemma-Probe** für Stichwörter ([`Lexicon::lookup_lemma`]): das Wort, Strong-Nummern in ihren
//! Schreibweisen (`G26` ↔ `G0026`, auch bei `StrongsPadding=false`), dann [`lemma_candidates`] (Stamm ohne einfache
//! Endung, Umlaut-Rückführung „Bäume“ → „Baum“) und zuletzt [`compound_tails`] (letzter Bestandteil eines Kompositums,
//! „Feigenbaum“ → „Baum“). Eine ehrliche Probe, keine Lemmatisierung.
//!
//! Text immer als Klartext ohne Auszeichnung ([`crate::text::to_plain`]), je Eintrag höchstens
//! [`MAX_ENTRY_CHARS`] Zeichen (dann `truncated`). HrefCom, RawFiles (persönliche Notizen) und unbekannte
//! Verszählungen meldet [`crate::scan`] mit `supported: false` und Grund. Verschlüsselte Module (R17c) öffnen
//! [`open_commentary_with_key`] und [`open_lexicon_with_key`] mit dem Schlüssel des Nutzers.

use std::time::Instant;

use baruch_scripture::versification::Versification;
use serde::Serialize;
use sword_modules::conf::{ModDrv, ModuleConf};
use sword_modules::{DictionaryModule, Module, VerseKey, VerseModule};

use crate::{
    BibleError, BibleInfo, checked_data_path, common_support, conf_with_key, known_versification, open_error, text,
};

/// Höchstlänge eines Eintrags (Kommentar bzw. Artikel) in Zeichen; Längeres wird gekürzt.
pub const MAX_ENTRY_CHARS: usize = 20_000;
/// Höchstzahl der Verse, die [`Commentary::entries`] durchsieht.
pub const MAX_COMMENTARY_VERSES: usize = 400;
/// Höchstzahl der Stichwörter in einer Liste.
pub const MAX_KEYS: usize = 500;

/// Kann Baruch den Kommentar lesen? Treiber, Kompression, Verschlüsselung, Verszählung, DataPath.
pub(crate) fn support_commentary(conf: &ModuleConf) -> Result<&'static Versification, String> {
    let drv = conf.mod_drv().map_err(|e| e.to_string())?;
    let compressed = match drv {
        ModDrv::RawCom | ModDrv::RawCom4 => false,
        ModDrv::ZCom | ModDrv::ZCom4 => true,
        ModDrv::HrefCom => return Err("Treiber HREFCom (Verweise ins Netz) wird nicht gelesen".to_owned()),
        ModDrv::RawFiles => return Err("Treiber RawFiles (persönliche Notizen) wird nicht gelesen".to_owned()),
        other => return Err(format!("Treiber {other:?} (nur RawCom, RawCom4, zCom und zCom4)")),
    };
    common_support(conf, compressed)?;
    known_versification(conf)
}

/// Kann Baruch das Lexikon lesen? Treiber, Kompression, Verschlüsselung, DataPath.
pub(crate) fn support_lexicon(conf: &ModuleConf) -> Result<(), String> {
    let drv = conf.mod_drv().map_err(|e| e.to_string())?;
    let compressed = match drv {
        ModDrv::RawLD | ModDrv::RawLD4 => false,
        ModDrv::ZLD => true,
        other => return Err(format!("Treiber {other:?} (nur RawLD, RawLD4 und zLD)")),
    };
    common_support(conf, compressed)
}

/// Klartext gekürzt auf `max` Zeichen; `true`, wenn gekürzt.
pub fn clip(text: &str, max: usize) -> (String, bool) {
    if text.chars().count() <= max {
        return (text.to_owned(), false);
    }
    let mut out: String = text.chars().take(max.saturating_sub(1)).collect();
    out.push('…');
    (out, true)
}

/// Ein Eintrag eines Kommentars: Bereich (Kapitel, Vers) in der Zählung des Moduls und Klartext.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CommentaryEntry {
    pub start: (u16, u16),
    pub end: (u16, u16),
    pub text: String,
    pub truncated: bool,
}

/// Treffer der Kommentarsuche.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CommentaryHit {
    /// OSIS-Buch.
    pub book: String,
    pub chapter: u16,
    pub verse: u16,
    pub text: String,
    pub truncated: bool,
}

/// Ein geöffneter Kommentar.
pub struct Commentary {
    pub info: BibleInfo,
    pub v11n: &'static Versification,
    module: VerseModule,
}

impl std::fmt::Debug for Commentary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Commentary").field("info", &self.info.name).field("v11n", &self.v11n.name).finish()
    }
}

/// Öffnet den Kommentar `info` (aus [`crate::scan`], `kind` commentary) ohne Schlüssel.
pub fn open_commentary(info: &BibleInfo) -> Result<Commentary, BibleError> {
    open_commentary_with_key(info, None)
}

/// Öffnet den Kommentar `info` mit dem Schlüssel `key` eines verschlüsselten Moduls (nur im Speicher, wie
/// [`crate::open_with_key`]).
pub fn open_commentary_with_key(info: &BibleInfo, key: Option<&str>) -> Result<Commentary, BibleError> {
    let conf = conf_with_key(info, key)?;
    let v11n = support_commentary(&conf).map_err(BibleError::Unsupported)?;
    checked_data_path(&conf).map_err(BibleError::Unsupported)?;
    if conf.is_locked() {
        return Err(BibleError::Locked(info.name.clone()));
    }
    let module = Module::open(&info.dir, conf).and_then(Module::into_verse).map_err(|e| open_error(&info.name, e))?;
    if !module.versification().name().eq_ignore_ascii_case(v11n.name) {
        return Err(BibleError::Unsupported(format!("Verszählung {}", module.versification().name())));
    }
    Ok(Commentary { info: info.clone(), v11n, module })
}

/// Probelesen der ersten Einträge (R17c, wie [`crate::Bible::probe`]).
fn probe_first(
    entries: impl Iterator<Item = Result<sword_modules::Entry, sword_modules::Error>>,
) -> Result<(), BibleError> {
    crate::probe_entries(entries)
}

impl Commentary {
    /// R17c: Probelesen des ersten vorhandenen Eintrags (prüft einen Schlüssel, bevor Baruch ihn speichert).
    pub fn probe(&self) -> Result<(), BibleError> {
        probe_first(self.module.iter().map(|r| r.map(|(_, e)| e)))
    }

    fn raw(&self, book: &str, chapter: u16, verse: u16) -> Result<Option<String>, BibleError> {
        if self.v11n.verses_in(book, chapter).is_none_or(|n| verse == 0 || verse > n) {
            return Ok(None);
        }
        let Some(key) = VerseKey::from_osis(book, u8::try_from(chapter).unwrap_or(0), u8::try_from(verse).unwrap_or(0))
        else {
            return Ok(None);
        };
        let entry = self.module.verse(&key).map_err(|e| BibleError::Read(e.to_string()))?;
        Ok(entry.map(|e| text::to_plain(&e.raw, e.markup)).filter(|t| !t.is_empty()))
    }

    /// Kommentar zu den Versen `start..=end` (Kapitel, Vers; Vers 0 = ganzes Kapitel) von `book` (OSIS) in
    /// der Zählung des Moduls. Gleiche Einträge benachbarter Verse werden zu einem Bereich; Verse ohne
    /// Kommentar fehlen. `Err(Unsupported)` bei einem Buch, das die Zählung nicht kennt.
    pub fn entries(&self, book: &str, start: (u16, u16), end: (u16, u16)) -> Result<Vec<CommentaryEntry>, BibleError> {
        let chapters = self.v11n.chapters_of(book).ok_or_else(|| BibleError::Unsupported(format!("Buch {book}")))?;
        let mut out: Vec<CommentaryEntry> = Vec::new();
        let mut seen = 0;
        for chapter in start.0..=end.0.max(start.0) {
            let Some(&max) = chapters.get(usize::from(chapter).wrapping_sub(1)) else { break };
            let from = if chapter == start.0 && start.1 > 0 { start.1 } else { 1 };
            let to = if chapter == end.0 && end.1 > 0 { end.1.min(max) } else { max };
            for verse in from..=to {
                seen += 1;
                if seen > MAX_COMMENTARY_VERSES {
                    return Ok(out);
                }
                let Some(plain) = self.raw(book, chapter, verse)? else { continue };
                let (text, truncated) = clip(&plain, MAX_ENTRY_CHARS);
                match out.last_mut() {
                    Some(last) if last.text == text => last.end = (chapter, verse),
                    _ => out.push(CommentaryEntry { start: (chapter, verse), end: (chapter, verse), text, truncated }),
                }
            }
        }
        Ok(out)
    }

    /// Lineare Suche nach einer Wortfolge (normalisiert wie die Bibelsuche); höchstens `limit` Treffer bis
    /// `deadline`. Ein Eintrag, den sich mehrere Verse teilen, zählt einmal (erster Vers).
    pub fn search(
        &self,
        query: &str,
        limit: usize,
        deadline: Instant,
    ) -> Result<(Vec<CommentaryHit>, bool), BibleError> {
        let needle: Vec<String> = baruch_scripture::wording::words(query).into_iter().map(|w| w.key).collect();
        let mut hits: Vec<CommentaryHit> = Vec::new();
        if needle.is_empty() || limit == 0 {
            return Ok((hits, false));
        }
        let mut last_raw = String::new();
        let mut searched = 0usize;
        for item in self.module.iter() {
            let (key, entry) = item.map_err(|e| BibleError::Read(e.to_string()))?;
            if key.chapter == 0 || key.verse == 0 {
                continue;
            }
            searched += 1;
            if searched.is_multiple_of(256) && Instant::now() > deadline {
                return Ok((hits, true));
            }
            if entry.raw == last_raw {
                continue;
            }
            last_raw.clone_from(&entry.raw);
            let plain = text::to_plain(&entry.raw, entry.markup);
            let words: Vec<String> = baruch_scripture::wording::words(&plain).into_iter().map(|w| w.key).collect();
            if words.windows(needle.len()).any(|w| w == needle.as_slice()) {
                let Some(book) = key.osis_book() else { continue };
                let (text, truncated) = clip(&plain, MAX_ENTRY_CHARS);
                hits.push(CommentaryHit {
                    book: book.to_owned(),
                    chapter: key.chapter.into(),
                    verse: key.verse.into(),
                    text,
                    truncated,
                });
                if hits.len() >= limit {
                    return Ok((hits, true));
                }
            }
        }
        Ok((hits, false))
    }
}

/// Einfache deutsche Endungen, die die Lemma-Probe abschneidet (längste zuerst): „Hirten“ → „Hirte“,
/// „Schafe“ → „Schaf“, „Weges“ → „Weg“, „Kindern“ → „Kind“. Keine Grammatik.
pub const LEMMA_ENDINGS: [&str; 7] = ["ern", "en", "es", "er", "e", "n", "s"];
/// Kürzester letzter Bestandteil eines Kompositums (Zeichen), den [`compound_tails`] probiert.
pub const MIN_COMPOUND_TAIL: usize = 4;
/// Kürzester vorderer Teil eines Kompositums (Zeichen).
pub const MIN_COMPOUND_HEAD: usize = 3;

/// Letzter Umlaut zurückgeführt („Bäum“ → „Baum“, „Söhn“ → „Sohn“, „Füß“ → „Fuß“); `None` ohne Umlaut.
fn without_last_umlaut(word: &str) -> Option<String> {
    let (at, c) = word.char_indices().rev().find(|(_, c)| matches!(c, 'ä' | 'ö' | 'ü' | 'Ä' | 'Ö' | 'Ü'))?;
    let plain = match c {
        'ä' => 'a',
        'ö' => 'o',
        'ü' => 'u',
        'Ä' => 'A',
        'Ö' => 'O',
        _ => 'U',
    };
    let mut out = String::with_capacity(word.len());
    out.push_str(&word[..at]);
    out.push(plain);
    out.push_str(&word[at + c.len_utf8()..]);
    Some(out)
}

/// Kandidaten der Lemma-Probe: das Wort selbst, dann die Stämme ohne Endung (mindestens drei Buchstaben, längere
/// zuerst), dann (Runde 19c) dieselben Formen mit zurückgeführtem Umlaut („Bäume“ → „Bäum“ → „Baum“). Ohne Doppel.
pub fn lemma_candidates(word: &str) -> Vec<String> {
    let word = word.trim();
    let mut stems: Vec<String> = Vec::new();
    for ending in LEMMA_ENDINGS {
        if let Some(stem) = word.strip_suffix(ending)
            && stem.chars().count() >= 3
            && stem != word
            && !stems.iter().any(|c| c == stem)
        {
            stems.push(stem.to_owned());
        }
    }
    // Kleinste Änderung zuerst: längere Stämme vor kürzeren („Hirte“ vor „Hirt“).
    stems.sort_by_key(|stem| std::cmp::Reverse(stem.chars().count()));
    let mut out = vec![word.to_owned()];
    out.extend(stems);
    let umlauts: Vec<String> = out.iter().filter_map(|w| without_last_umlaut(w)).collect();
    for form in umlauts {
        if !out.contains(&form) {
            out.push(form);
        }
    }
    out
}

/// Letzte Bestandteile eines Kompositums als Rückfall der Lemma-Probe (Runde 19c): jedes Ende ab
/// [`MIN_COMPOUND_TAIL`] Zeichen hinter einem vorderen Teil ab [`MIN_COMPOUND_HEAD`] Zeichen, längere zuerst,
/// großgeschrieben („Feigenbaum“ → „Eigenbaum“, …, „Baum“). Die Probe nimmt das erste, das es als Stichwort gibt.
pub fn compound_tails(word: &str) -> Vec<String> {
    let chars: Vec<char> = word.trim().chars().collect();
    let mut out = Vec::new();
    for start in MIN_COMPOUND_HEAD..chars.len() {
        if chars.len() - start < MIN_COMPOUND_TAIL {
            break;
        }
        let tail: String = chars[start..].iter().collect();
        let mut letters = tail.chars();
        let Some(first) = letters.next() else { continue };
        if !first.is_alphabetic() {
            continue;
        }
        out.push(first.to_uppercase().chain(letters).collect());
    }
    out
}

/// Sieht `key` aus wie eine Strong-Nummer („G26“, „h430“, „G0026a“)?
pub fn strong_like(key: &str) -> bool {
    let key = key.trim();
    let mut chars = key.chars();
    matches!(chars.next(), Some('G' | 'g' | 'H' | 'h'))
        && chars.as_str().chars().next().is_some_and(|c| c.is_ascii_digit())
        && chars.as_str().chars().all(|c| c.is_ascii_alphanumeric())
}

/// Ein Artikel eines Lexikons.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LexiconArticle {
    /// Stichwort, wie es im Modul steht.
    pub key: String,
    pub text: String,
    pub truncated: bool,
}

/// Treffer der Lexikonsuche: Stichwort und Ausschnitt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LexiconHit {
    pub key: String,
    /// Klartext des Artikels (gekürzt auf 300 Zeichen).
    pub excerpt: String,
    /// `true`, wenn das Stichwort selbst passt (sonst nur der Text).
    pub in_key: bool,
}

/// Stichwortliste.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct KeyList {
    pub keys: Vec<String>,
    /// Es gibt mehr als die gelieferten.
    pub truncated: bool,
    /// Zahl aller Stichwörter im Modul.
    pub total: usize,
}

/// Ein geöffnetes Lexikon.
pub struct Lexicon {
    pub info: BibleInfo,
    module: DictionaryModule,
}

impl std::fmt::Debug for Lexicon {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Lexicon").field("info", &self.info.name).finish()
    }
}

/// Öffnet das Lexikon `info` (aus [`crate::scan`], `kind` lexicon) ohne Schlüssel.
pub fn open_lexicon(info: &BibleInfo) -> Result<Lexicon, BibleError> {
    open_lexicon_with_key(info, None)
}

/// Öffnet das Lexikon `info` mit dem Schlüssel `key` eines verschlüsselten Moduls (nur im Speicher).
pub fn open_lexicon_with_key(info: &BibleInfo, key: Option<&str>) -> Result<Lexicon, BibleError> {
    let conf = conf_with_key(info, key)?;
    support_lexicon(&conf).map_err(BibleError::Unsupported)?;
    if conf.is_locked() {
        return Err(BibleError::Locked(info.name.clone()));
    }
    let module =
        Module::open(&info.dir, conf).and_then(Module::into_dictionary).map_err(|e| open_error(&info.name, e))?;
    Ok(Lexicon { info: info.clone(), module })
}

fn read_err(e: sword_modules::Error) -> BibleError {
    BibleError::Read(e.to_string())
}

impl Lexicon {
    /// R17c: Probelesen des ersten vorhandenen Artikels (prüft einen Schlüssel, bevor Baruch ihn speichert).
    pub fn probe(&self) -> Result<(), BibleError> {
        probe_first(self.module.iter().map(|r| r.map(|(_, e)| e)))
    }

    /// Zahl der Stichwörter.
    pub fn len(&self) -> usize {
        self.module.len()
    }

    pub fn is_empty(&self) -> bool {
        self.module.is_empty()
    }

    /// Artikel zu einem Stichwort (Groß-/Kleinschreibung nach der conf, `@LINK` gefolgt); `None`, wenn es
    /// das Stichwort nicht gibt.
    pub fn lookup(&self, key: &str) -> Result<Option<LexiconArticle>, BibleError> {
        let key = key.trim();
        if key.is_empty() {
            return Ok(None);
        }
        let Some(entry) = self.module.get(key).map_err(read_err)? else { return Ok(None) };
        let plain = text::to_plain(&entry.raw, entry.markup);
        // Das Stichwort, wie es im Modul steht (nächstes ab der Eingabe ist bei einem Treffer es selbst).
        let stored = self.module.get_nearest(key).map_err(read_err)?.map(|(k, _)| k).unwrap_or_else(|| key.to_owned());
        let (text, truncated) = clip(&plain, MAX_ENTRY_CHARS);
        Ok(Some(LexiconArticle { key: stored, text, truncated }))
    }

    /// Runde 19c: Artikel mit Lemma-Probe. Zuerst das Stichwort selbst ([`Self::lookup`]), bei einer Strong-Nummer
    /// dann ihre gepolsterten Schreibweisen (`G26` → `00026`, `G0026`; über `get_strongs`, unabhängig von
    /// `StrongsPadding`), sonst [`lemma_candidates`] und zuletzt die Bestandteile aus [`compound_tails`] (je mit
    /// ihren Lemma-Kandidaten). Antwort: Artikel und – wenn nicht das Stichwort selbst passte – die probierte Form.
    pub fn lookup_lemma(&self, key: &str) -> Result<Option<(LexiconArticle, Option<String>)>, BibleError> {
        let key = key.trim();
        if key.is_empty() {
            return Ok(None);
        }
        if let Some(article) = self.lookup(key)? {
            return Ok(Some((article, None)));
        }
        if strong_like(key) {
            if let Some(entry) = self.module.get_strongs(key).map_err(read_err)? {
                let plain = text::to_plain(&entry.raw, entry.markup);
                let (text, truncated) = clip(&plain, MAX_ENTRY_CHARS);
                let stored = self.strong_key(key)?.unwrap_or_else(|| key.to_owned());
                return Ok(Some((LexiconArticle { key: stored.clone(), text, truncated }, Some(stored))));
            }
            return Ok(None);
        }
        for candidate in lemma_candidates(key).into_iter().skip(1) {
            if let Some(article) = self.lookup(&candidate)? {
                return Ok(Some((article, Some(candidate))));
            }
        }
        for tail in compound_tails(key) {
            for candidate in lemma_candidates(&tail) {
                if let Some(article) = self.lookup(&candidate)? {
                    return Ok(Some((article, Some(candidate))));
                }
            }
        }
        Ok(None)
    }

    /// Das Stichwort, unter dem `get_strongs` eine Strong-Nummer fand (gleiche Reihenfolge der Formen).
    fn strong_key(&self, number: &str) -> Result<Option<String>, BibleError> {
        let digits: String = number.trim()[1..].chars().take_while(char::is_ascii_digit).collect();
        let Ok(value) = digits.parse::<u32>() else { return Ok(None) };
        let letter = number.trim()[..1].to_uppercase();
        for form in [format!("{value:05}"), format!("{letter}{value:04}")] {
            let listed = self.keys(&form, 1)?;
            if let Some(stored) = listed.keys.into_iter().next().filter(|k| k.eq_ignore_ascii_case(&form)) {
                return Ok(Some(stored));
            }
        }
        Ok(None)
    }

    /// Stichwörter (sortiert wie im Modul), die mit `prefix` beginnen (ohne Präfix alle), höchstens `limit`
    /// (≤ [`MAX_KEYS`]).
    pub fn keys(&self, prefix: &str, limit: usize) -> Result<KeyList, BibleError> {
        let limit = limit.clamp(1, MAX_KEYS);
        let prefix_fold = prefix.trim().to_uppercase();
        let mut keys = Vec::new();
        let mut truncated = false;
        let start = if prefix_fold.is_empty() {
            0
        } else {
            // Erster Eintrag ab dem Präfix: über get_nearest das Stichwort, dann seine Stelle linear suchen
            // wäre teuer; die Liste ist sortiert, also binär über key_at.
            let (mut lo, mut hi) = (0, self.module.len());
            while lo < hi {
                let mid = lo + (hi - lo) / 2;
                if self.module.key_at(mid).map_err(read_err)?.to_uppercase().as_bytes() < prefix_fold.as_bytes() {
                    lo = mid + 1;
                } else {
                    hi = mid;
                }
            }
            lo
        };
        for i in start..self.module.len() {
            let key = self.module.key_at(i).map_err(read_err)?;
            if !prefix_fold.is_empty() && !key.to_uppercase().starts_with(&prefix_fold) {
                break;
            }
            if keys.len() == limit {
                truncated = true;
                break;
            }
            keys.push(key);
        }
        Ok(KeyList { keys, truncated, total: self.module.len() })
    }

    /// Suche nach einer Wortfolge in Stichwort und Artikel (normalisiert); Stichworttreffer zuerst,
    /// höchstens `limit` Treffer bis `deadline`.
    pub fn search(&self, query: &str, limit: usize, deadline: Instant) -> Result<(Vec<LexiconHit>, bool), BibleError> {
        let needle: Vec<String> = baruch_scripture::wording::words(query).into_iter().map(|w| w.key).collect();
        if needle.is_empty() || limit == 0 {
            return Ok((Vec::new(), false));
        }
        let contains = |text: &str| {
            let words: Vec<String> = baruch_scripture::wording::words(text).into_iter().map(|w| w.key).collect();
            words.windows(needle.len()).any(|w| w == needle.as_slice())
        };
        let (mut in_key, mut in_text) = (Vec::new(), Vec::new());
        let mut truncated = false;
        for (i, item) in self.module.iter().enumerate() {
            if i % 128 == 127 && Instant::now() > deadline {
                truncated = true;
                break;
            }
            let (key, entry) = item.map_err(read_err)?;
            let plain = text::to_plain(&entry.raw, entry.markup);
            let key_hit = contains(&key);
            if key_hit || contains(&plain) {
                let hit = LexiconHit { key, excerpt: clip(&plain, 300).0, in_key: key_hit };
                if key_hit {
                    in_key.push(hit)
                } else {
                    in_text.push(hit)
                }
                if in_key.len() >= limit {
                    truncated = true;
                    break;
                }
            }
        }
        in_key.extend(in_text);
        if in_key.len() > limit {
            in_key.truncate(limit);
            truncated = true;
        }
        Ok((in_key, truncated))
    }
}
