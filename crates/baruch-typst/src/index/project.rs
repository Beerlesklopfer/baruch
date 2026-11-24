// SPDX-License-Identifier: AGPL-3.0-or-later
//! Projektion eines Abschnittstexts (Runde 31b): automatische Bibelstellen-Marken und Auszeichnung der ersten Erwähnung
//! eines Glossarbegriffs je Kapitel. Nur Export, Karte, Spiegel, Satz, PDF und EPUB – die Texte in SQLite bleiben.
//!
//! **Bibelstellen:** Ist das Register `bibel` in `index_pages.registers`, bekommt jede gültige Stelle, die
//! [`baruch_scripture::scan`] im Fließtext findet (nicht in Code, Kommentar, Raw, Überschriftszeile oder Marke), direkt
//! dahinter eine Marke `register: "bibel"`, `term` = Buch („Johannes“), `sub` = Kapitel und Verse („3,16“), `sort` =
//! [`baruch_scripture::Passage::sort_key`] (biblische Reihenfolge).
//!
//! **Erste Erwähnung** (nur mit `glossary_first_mention == Some(true)` und Glossar): je Kapitel (Überschrift der Ebene 1,
//! Teil oder „ohne Nummer“ Ebene 1 setzt zurück) die erste Fundstelle eines Begriffs oder einer Variante als ganze Wörter
//! (Groß-/Kleinschreibung egal). Form, nur Standard-Typst:
//! - mit Glossarseite: `#link(<baruch-gl-UUID>)[#emph[Wort]]` – ein Link auf den Eintrag der Glossarseite (das Label
//!   steht im Buchende; ohne Glossarseite gäbe es keinen Verweis auf ein fehlendes Label);
//! - ohne Glossarseite: `/*baruch-erste-erwaehnung*/#emph[Wort]` – der Kommentar davor ist unsichtbar und macht die
//!   Auszeichnung beim Import eines Exports wieder erkennbar (Runde 21b, [`super::strip_first_mentions`]); von Hand
//!   geschriebene Hervorhebung bleibt so unberührt.
//!
//! Folgt dem Wort ein Zeichen, das Typst an den Ausdruck hängen würde (`(`, `[`, `.` vor einem Buchstaben, `;`), steht
//! dazwischen ein `;` (beendet den eingebetteten Ausdruck und verschwindet im Satz).

use std::borrow::Cow;
use std::collections::{HashMap, HashSet};

use super::scan::{Scan, scan};
use super::{
    BackMatter, FIRST_MENTION_MARK, GLOSSARY_ANCHOR_PREFIX, IndexMark, SCRIPTURE_REGISTER, mark_source, term_key,
};

/// Muster eines Glossarbegriffs: Wörter (Vergleichsform) und die Trennzeichen dazwischen.
struct Pattern {
    entry: usize,
    words: Vec<String>,
    seps: Vec<String>,
}

/// Vorbereitete Projektion (privat, gehört zu [`super::Projector`]).
pub(super) struct Plan {
    pub active: bool,
    scripture: bool,
    /// Muster nach dem ersten Wort.
    patterns: HashMap<String, Vec<Pattern>>,
    /// Label je Eintrag, wenn verlinkt wird (Glossarseite an und gültiges Label).
    anchors: Vec<Option<String>>,
    /// Einträge, die im laufenden Kapitel schon ausgezeichnet sind.
    seen: HashSet<usize>,
}

/// Darf eine UUID Teil eines Typst-Labels sein?
pub(super) fn label_safe(uuid: &str) -> bool {
    !uuid.is_empty() && uuid.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
}

/// Ist das Bibelstellenregister eingeschaltet (Registerseiten mit `bibel`)?
pub(super) fn scripture_enabled(back: &BackMatter) -> bool {
    back.settings.index_pages.as_ref().is_some_and(|p| p.registers.iter().any(|r| r == SCRIPTURE_REGISTER))
}

/// Zerlegt einen Begriff in Wörter (Buchstaben und Ziffern) und die Trennzeichen dazwischen (Leerraum zu einem
/// Leerzeichen). Andere Zeichen als Leerraum, `-`, `'`, `’` und `.` zwischen den Wörtern oder am Rand → `None`
/// (der Begriff wird nicht gesucht).
pub(super) fn split_term(term: &str) -> Option<(Vec<String>, Vec<String>)> {
    // Läufe aus Wortzeichen bzw. Nicht-Wortzeichen, abwechselnd.
    let mut runs: Vec<(bool, String)> = Vec::new();
    for c in term.trim().chars() {
        let word = c.is_alphanumeric();
        match runs.last_mut() {
            Some((w, run)) if *w == word => run.push(c),
            _ => runs.push((word, c.to_string())),
        }
    }
    if !runs.first().is_some_and(|r| r.0) || !runs.last().is_some_and(|r| r.0) {
        return None;
    }
    let mut words = Vec::new();
    let mut seps = Vec::new();
    for (word, run) in runs {
        if word {
            words.push(term_key(&run));
        } else {
            seps.push(normalize_sep(&run)?);
        }
    }
    Some((words, seps))
}

/// Trennzeichen in Vergleichsform: Leerraum zu einem Leerzeichen; nur Leerraum, `-`, `'`, `’`, `.` erlaubt, keine
/// Leerzeile.
fn normalize_sep(sep: &str) -> Option<String> {
    if sep.matches('\n').count() > 1 {
        return None;
    }
    let mut out = String::new();
    let mut space = false;
    for c in sep.chars() {
        if c.is_whitespace() {
            space = true;
            continue;
        }
        if !matches!(c, '-' | '\'' | '’' | '.') {
            return None;
        }
        if space {
            out.push(' ');
            space = false;
        }
        out.push(c);
    }
    if space {
        out.push(' ');
    }
    (!out.is_empty()).then_some(out)
}

impl Plan {
    pub fn new(back: &BackMatter) -> Plan {
        let active = back.is_active();
        let scripture = active && scripture_enabled(back);
        let mention = active && back.settings.glossary_first_mention == Some(true);
        let link = back.settings.glossary_page.is_some();
        let mut patterns: HashMap<String, Vec<Pattern>> = HashMap::new();
        let mut anchors = Vec::new();
        for (entry, gloss) in back.glossary.iter().enumerate() {
            anchors.push((link && label_safe(&gloss.uuid)).then(|| format!("{GLOSSARY_ANCHOR_PREFIX}{}", gloss.uuid)));
            if !mention {
                continue;
            }
            for form in std::iter::once(&gloss.term).chain(&gloss.variants) {
                if let Some((words, seps)) = split_term(form) {
                    patterns.entry(words[0].clone()).or_default().push(Pattern { entry, words, seps });
                }
            }
        }
        // Längere Muster zuerst („Neuer Bund“ vor „Neuer“).
        for list in patterns.values_mut() {
            list.sort_by_key(|p| std::cmp::Reverse(p.words.len()));
        }
        Plan { active, scripture, patterns, anchors, seen: HashSet::new() }
    }

    pub fn project<'b>(&mut self, body: &'b str) -> Cow<'b, str> {
        if !self.active || (!self.scripture && self.patterns.is_empty()) {
            return Cow::Borrowed(body);
        }
        let scanned = scan(body);
        // (Anfang, Ende, Ersatz); Einfügungen haben Anfang == Ende.
        let mut edits: Vec<(usize, usize, String)> = Vec::new();
        if !self.patterns.is_empty() {
            self.mentions(body, &scanned, &mut edits);
        } else {
            // Kapitelwechsel trotzdem mitzählen (spielt ohne Muster keine Rolle).
            self.seen.clear();
        }
        if self.scripture {
            for (at, mark) in scripture_marks(body, &scanned) {
                let inside = edits.iter().any(|(a, b, _)| *a < at && at < *b);
                if !inside {
                    edits.push((at, at, mark_source(&mark)));
                }
            }
        }
        if edits.is_empty() {
            return Cow::Borrowed(body);
        }
        edits.sort_by_key(|(a, b, _)| (*a, *b));
        let mut out = String::with_capacity(body.len() + edits.iter().map(|e| e.2.len()).sum::<usize>());
        let mut at = 0;
        for (a, b, text) in edits {
            if a < at {
                continue;
            }
            out.push_str(&body[at..a]);
            out.push_str(&text);
            at = b;
        }
        out.push_str(&body[at..]);
        Cow::Owned(out)
    }

    /// Erste Erwähnungen in `body` → Ersetzungen.
    fn mentions(&mut self, body: &str, scanned: &Scan, edits: &mut Vec<(usize, usize, String)>) {
        let words = scanned.words(body);
        let keys: Vec<String> = words.iter().map(|&(a, b)| term_key(&body[a..b])).collect();
        let mut headings = scanned.headings.iter().filter(|h| h.level <= 1).map(|h| h.start).peekable();
        let mut t = 0;
        while t < words.len() {
            let (start, _) = words[t];
            while headings.next_if(|&h| h <= start).is_some() {
                self.seen.clear();
            }
            let Some(found) = self.match_at(body, scanned, &words, &keys, t) else {
                t += 1;
                continue;
            };
            let (entry, n) = found;
            let end = words[t + n - 1].1;
            if self.seen.insert(entry) {
                let slice = &body[start..end];
                let wrapped = match &self.anchors[entry] {
                    Some(label) => format!("#link(<{label}>)[#emph[{slice}]]"),
                    None => format!("{FIRST_MENTION_MARK}#emph[{slice}]"),
                };
                let guard = needs_guard(&body[end..]);
                edits.push((start, end, if guard { format!("{wrapped};") } else { wrapped }));
            }
            t += n;
        }
        for _ in headings {
            self.seen.clear();
        }
    }

    /// Längstes Muster ab Wort `t`: (Eintrag, Zahl der Wörter).
    fn match_at(
        &self,
        body: &str,
        scanned: &Scan,
        words: &[(usize, usize)],
        keys: &[String],
        t: usize,
    ) -> Option<(usize, usize)> {
        let list = self.patterns.get(&keys[t])?;
        'pattern: for pattern in list {
            let n = pattern.words.len();
            if t + n > words.len() {
                continue;
            }
            for k in 1..n {
                if keys[t + k] != pattern.words[k] {
                    continue 'pattern;
                }
                let (a, b) = (words[t + k - 1].1, words[t + k].0);
                if !scanned.prose_range(a, b) || normalize_sep(&body[a..b]).as_deref() != Some(&pattern.seps[k - 1]) {
                    continue 'pattern;
                }
            }
            return Some((pattern.entry, n));
        }
        None
    }
}

/// Hängt Typst ein direkt folgendes Zeichen an einen eingebetteten Ausdruck (`#emph[…]`)?
fn needs_guard(after: &str) -> bool {
    let mut chars = after.chars();
    match chars.next() {
        Some('(' | '[' | ';') => true,
        Some('.') => chars.next().is_some_and(|c| c.is_alphabetic() || c == '_'),
        _ => false,
    }
}

/// Automatische Marken des Bibelstellenregisters: (Einfügestelle, Marke) für jede gültige Stelle im Fließtext.
pub(super) fn scripture_marks(body: &str, scanned: &Scan) -> Vec<(usize, IndexMark)> {
    let masked = scanned.masked(body);
    let mut out = Vec::new();
    for found in baruch_scripture::scan(&masked) {
        if found.status != baruch_scripture::RefStatus::Valid {
            continue;
        }
        let Some(passage) = found.passage else { continue };
        let (a, b) = found.span;
        if !scanned.prose_range(a, b) || !scanned.insertable(body, b) {
            continue;
        }
        let mark = IndexMark {
            register: SCRIPTURE_REGISTER.to_owned(),
            term: passage.book.name.to_owned(),
            sub: passage.chapter_verse(),
            sort: passage.sort_key(),
            ..IndexMark::default()
        };
        out.push((b, mark));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terms_split_into_words_and_separators() {
        assert_eq!(
            split_term("Neuer  Bund"),
            Some((vec!["neuer".into(), "bund".into()], vec![" ".into()])),
            "Leerraum zu einem Leerzeichen, Wörter klein"
        );
        assert_eq!(
            split_term("Jesus-Christus"),
            Some((vec!["jesus".into(), "christus".into()], vec!["-".into()])),
            "Bindestrich als Trennzeichen"
        );
        assert_eq!(split_term("C++"), None, "Satzzeichen am Rand: nicht gesucht");
        assert_eq!(split_term("a/b"), None, "Schrägstrich ist kein erlaubtes Trennzeichen");
    }

    #[test]
    fn a_guard_follows_only_where_typst_would_continue_the_expression() {
        assert!(needs_guard("(s) weiter"), "Klammer würde Argumente öffnen");
        assert!(needs_guard("[x]"), "eckige Klammer wäre ein weiterer Inhaltsblock");
        assert!(needs_guard(".Name"), "Punkt vor Buchstaben wäre ein Feldzugriff");
        assert!(needs_guard("; und"), "ein Semikolon würde sonst verschluckt");
        assert!(!needs_guard(". Satz"), "Satzende bleibt");
        assert!(!needs_guard(" weiter"), "Leerzeichen bleibt");
    }
}
