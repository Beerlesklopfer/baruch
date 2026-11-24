// SPDX-License-Identifier: AGPL-3.0-or-later
//! Prüfung der Antwort im Code ([`check`]) – nicht das Modell entscheidet, was belegt ist:
//!
//! - Ein Zitat („…“, “…”, "…" oder »…«) in derselben Zeile wie ein `[Wn]` gilt nur dann als belegt, wenn es
//!   nach der Normalisierung ([`normalize`]: Leerraum, typografische Anführungszeichen, weiches Trennzeichen)
//!   Teilstring einer dort genannten, abgerufenen Seite ist; Auslassungen („…“, „[…]“, „...“) teilen das Zitat
//!   in Stücke, die in dieser Reihenfolge vorkommen müssen (je an Wortgrenzen; mit Auslassung je mindestens
//!   [`MIN_GAP_PIECE_WORDS`] Wörter und höchstens [`MAX_GAP_CHARS`] Zeichen Lücke auf der Seite). Satzzeichen am
//!   Ende des Zitats zählen nicht.
//!   Sonst folgt [`NOT_BELEGT`].
//! - `[Wn]` ohne abgerufene Seite bekommt [`UNKNOWN_SOURCE`], eine genannte Adresse, die nicht abgerufen
//!   wurde, [`NOT_FETCHED`].
//! - Darunter setzt der Code: bei ausgefallener Websuche [`failure_line`], ohne abrufbaren Beleg (keine
//!   genannte Seite mit belegtem Zitat oder ohne Zitat) [`CHECK_NOTE`] und immer die Zeile „Quellen: [Wn] Titel –
//!   URL (abgerufen am TT.MM.JJJJ)“ ([`sources_line`]).
//! - Jede genannte Seite ergibt Einträge für `web_evidence` ([`Draft`]): je Zitat `belegt` bzw.
//!   `nicht_belegt`, eine Seite ohne Zitat `nicht_zitiert`.

use serde::Serialize;
use unicode_normalization::UnicodeNormalization;

use super::search::url_at;
use super::{Research, WebPage};
use crate::connections::multigpt::clean_line;

/// Zusatz hinter einem Zitat, das auf der genannten Seite nicht steht.
pub const NOT_BELEGT: &str = "(nicht belegt)";
/// Zusatz hinter einem `[Wn]` ohne abgerufene Seite.
pub const UNKNOWN_SOURCE: &str = "(unbekannte Quelle)";
/// Zusatz hinter einer Adresse, die nicht abgerufen wurde.
pub const NOT_FETCHED: &str = "(nicht abgerufen)";
/// Zeile, wenn es keinen abrufbaren Beleg gibt.
pub const CHECK_NOTE: &str = "Angabe prüfen: nicht belegt";

/// Ergebnis der Prüfung eines Zitats bzw. einer genannten Seite.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    Belegt,
    NichtBelegt,
    NichtZitiert,
}

impl Verdict {
    /// Wert in `web_evidence.verdict`.
    pub fn as_str(self) -> &'static str {
        match self {
            Verdict::Belegt => "belegt",
            Verdict::NichtBelegt => "nicht_belegt",
            Verdict::NichtZitiert => "nicht_zitiert",
        }
    }

    /// Anzeige.
    pub fn label(code: &str) -> &'static str {
        match code {
            "belegt" => "belegt (Wortlaut auf der Seite gefunden)",
            "nicht_belegt" => "nicht belegt (Wortlaut steht nicht auf der Seite)",
            _ => "genannt, ohne Zitat",
        }
    }
}

/// Ein Eintrag für `web_evidence`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Draft {
    /// n von `[Wn]`.
    pub label: usize,
    /// Das Zitat, wie es in der Antwort steht (leer bei `nicht_zitiert`).
    pub quote: String,
    pub verdict: Verdict,
}

/// Geprüfte Antwort.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Checked {
    /// Antwort mit Zusätzen und den Zeilen darunter.
    pub text: String,
    pub evidence: Vec<Draft>,
}

/// Zeile, wenn die Websuche ausfiel.
pub fn failure_line(reason: &str) -> String {
    format!("Hinweis: Websuche ausgefallen ({})", clean_line(reason, 300))
}

/// Text für den Vergleich: Unicode-Normalform NFC (Runde 19b: zerlegte Umlaute wie „a“ + U+0308 aus PDFs gleich
/// „ä“), typografische Anführungszeichen vereinheitlicht, weiches Trennzeichen entfernt, Leerraum zu einem Leerzeichen.
pub fn normalize(text: &str) -> String {
    let mapped: String = text
        .nfc()
        .filter(|c| *c != '\u{ad}')
        .map(|c| match c {
            '„' | '“' | '”' | '«' | '»' | '″' => '"',
            '‚' | '‘' | '’' | '‹' | '›' | '′' => '\'',
            other => other,
        })
        .collect();
    mapped.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Mindestens so viele Wörter je Stück eines Zitats mit Auslassung („Gott … ist … nicht“ ist kein Beleg).
pub const MIN_GAP_PIECE_WORDS: usize = 3;
/// Höchstens so viele Zeichen darf eine Auslassung auf der Seite überspringen.
pub const MAX_GAP_CHARS: usize = 400;

/// Steht das Zitat (normalisiert, Auslassungen als Lücken) auf der Seite (normalisiert)? Jedes Stück muss an
/// Wortgrenzen beginnen und enden; mit Auslassungen hat jedes Stück mindestens [`MIN_GAP_PIECE_WORDS`] Wörter,
/// und zwischen zwei Stücken liegen auf der Seite höchstens [`MAX_GAP_CHARS`] Zeichen – sonst ließe sich aus
/// verstreuten Wortfetzen ein sinnentstelltes „Zitat“ bauen.
pub fn quote_found(quote: &str, page: &str) -> bool {
    let quote = normalize(quote);
    let quote = quote.trim().trim_end_matches(['.', ',', ';', ':', '!', '?']).trim();
    if quote.is_empty() {
        return false;
    }
    let gapped = quote.replace("[…]", "…").replace("(…)", "…").replace("...", "…");
    let pieces: Vec<&str> = gapped.split('…').map(str::trim).filter(|p| !p.is_empty()).collect();
    if pieces.is_empty() {
        return false;
    }
    if pieces.len() > 1 && pieces.iter().any(|p| p.split_whitespace().count() < MIN_GAP_PIECE_WORDS) {
        return false;
    }
    chain(page, &pieces, 0, None)
}

/// Kommen `pieces` ab Byte `from` in dieser Reihenfolge an Wortgrenzen vor, jedes folgende Stück höchstens
/// [`MAX_GAP_CHARS`] Zeichen hinter dem vorigen (`window` = es gilt eine Lücke)? Probiert jede passende Stelle.
fn chain(page: &str, pieces: &[&str], from: usize, window: Option<usize>) -> bool {
    let Some((piece, rest)) = pieces.split_first() else { return true };
    let mut search = from;
    while let Some(at) = page[search..].find(piece).map(|i| search + i) {
        if window.is_some_and(|w| page[from..at].chars().count() > w) {
            return false;
        }
        let end = at + piece.len();
        if at_boundary(page, at, end) && chain(page, rest, end, Some(MAX_GAP_CHARS)) {
            return true;
        }
        search = at + page[at..].chars().next().map_or(1, char::len_utf8);
    }
    false
}

/// Liegt `page[start..end]` an Wortgrenzen (kein Buchstabe/keine Ziffer direkt davor bzw. dahinter, wo das
/// Stück selbst mit einem Buchstaben/einer Ziffer beginnt bzw. endet)?
fn at_boundary(page: &str, start: usize, end: usize) -> bool {
    let word = |c: char| c.is_alphanumeric();
    let inner = &page[start..end];
    let before_ok = !inner.chars().next().is_some_and(word) || !page[..start].chars().next_back().is_some_and(word);
    let after_ok = !inner.chars().next_back().is_some_and(word) || !page[end..].chars().next().is_some_and(word);
    before_ok && after_ok
}

/// `[Wn]` einer Zeile: (Ende in Bytes, n).
fn labels_in(line: &str) -> Vec<(usize, usize)> {
    let mut found = Vec::new();
    let mut from = 0;
    while let Some(at) = line[from..].find("[W") {
        let start = from + at;
        let after = &line[start + 2..];
        let digits = after.bytes().take_while(u8::is_ascii_digit).count();
        if digits > 0 && after[digits..].starts_with(']') {
            let n = after[..digits].parse::<usize>().unwrap_or(usize::MAX);
            found.push((start + 3 + digits, n));
            from = start + 3 + digits;
        } else {
            from = start + 2;
        }
    }
    found
}

/// Zitate einer Zeile: (Ende hinter dem schließenden Zeichen in Bytes, Inhalt).
fn quotes_in(line: &str) -> Vec<(usize, String)> {
    let closers = |c: char| -> &'static [char] {
        match c {
            '„' => &['“', '”', '"'],
            '“' => &['”'],
            '"' => &['"'],
            '»' => &['«'],
            '«' => &['»'],
            _ => &[],
        }
    };
    let mut found = Vec::new();
    let mut from = 0;
    while from < line.len() {
        let Some((offset, open)) = line[from..].char_indices().find(|(_, c)| !closers(*c).is_empty()) else { break };
        let start = from + offset + open.len_utf8();
        match line[start..].char_indices().find(|(_, c)| closers(open).contains(c)) {
            Some((len, close)) => {
                let end = start + len + close.len_utf8();
                found.push((end, line[start..start + len].to_owned()));
                from = end;
            }
            None => from = start,
        }
    }
    found
}

/// Adressen einer Zeile: (Ende in Bytes, Adresse).
fn urls_in(line: &str) -> Vec<(usize, String)> {
    let mut found = Vec::new();
    let mut from = 0;
    while let Some(at) = line[from..].find("http") {
        let start = from + at;
        let rest = &line[start..];
        if rest.starts_with("http://") || rest.starts_with("https://") {
            let url = url_at(rest);
            found.push((start + url.len(), url.to_owned()));
            from = start + url.len().max(4);
        } else {
            from = start + 4;
        }
    }
    found
}

/// Adresse für den Vergleich: ohne Anker und ohne `/` am Ende.
fn url_key(url: &str) -> &str {
    url.split('#').next().unwrap_or(url).trim_end_matches('/')
}

/// Die Zeile „Quellen: …“ für die genannten Seiten (in der Reihenfolge der ersten Nennung).
pub fn sources_line(pages: &[&WebPage]) -> String {
    if pages.is_empty() {
        return "Quellen: keine abgerufene Seite genannt".to_owned();
    }
    let items: Vec<String> = pages
        .iter()
        .map(|p| format!("[W{}] {} – {} (abgerufen am {})", p.label, clean_line(&p.title, 200), p.url, p.day))
        .collect();
    format!("Quellen: {}", items.join("; "))
}

/// Antwort prüfen (siehe Moduldoku).
pub fn check(answer: &str, research: &Research) -> Checked {
    let normalized: Vec<(usize, String)> = research.pages.iter().map(|p| (p.label, normalize(&p.text))).collect();
    let page_text = |n: usize| normalized.iter().find(|(l, _)| *l == n).map(|(_, t)| t.as_str());
    let fetched: Vec<&str> = research.pages.iter().map(|p| url_key(&p.url)).collect();
    let mut cited: Vec<usize> = Vec::new();
    let mut evidence: Vec<Draft> = Vec::new();
    let mut lines: Vec<String> = Vec::new();
    for line in answer.trim_end().lines() {
        let mut inserts: Vec<(usize, String)> = Vec::new();
        let mut known: Vec<usize> = Vec::new();
        for (end, n) in labels_in(line) {
            if page_text(n).is_some() {
                if !known.contains(&n) {
                    known.push(n);
                }
                if !cited.contains(&n) {
                    cited.push(n);
                }
            } else if !line[end..].starts_with(&format!(" {UNKNOWN_SOURCE}")) {
                inserts.push((end, format!(" {UNKNOWN_SOURCE}")));
            }
        }
        if !known.is_empty() {
            for (end, quote) in quotes_in(line) {
                if quote.trim().is_empty() {
                    continue;
                }
                match known.iter().find(|n| page_text(**n).is_some_and(|t| quote_found(&quote, t))) {
                    Some(n) => {
                        evidence.push(Draft { label: *n, quote: quote.trim().to_owned(), verdict: Verdict::Belegt })
                    }
                    None => {
                        evidence.push(Draft {
                            label: known[0],
                            quote: quote.trim().to_owned(),
                            verdict: Verdict::NichtBelegt,
                        });
                        if !line[end..].starts_with(&format!(" {NOT_BELEGT}")) {
                            inserts.push((end, format!(" {NOT_BELEGT}")));
                        }
                    }
                }
            }
        }
        for (end, url) in urls_in(line) {
            if !fetched.contains(&url_key(&url)) && !line[end..].starts_with(&format!(" {NOT_FETCHED}")) {
                inserts.push((end, format!(" {NOT_FETCHED}")));
            }
        }
        let mut out = line.to_owned();
        inserts.sort_by_key(|insert| std::cmp::Reverse(insert.0));
        for (at, text) in inserts {
            out.insert_str(at, &text);
        }
        lines.push(out);
    }
    for n in &cited {
        if !evidence.iter().any(|d| d.label == *n) {
            evidence.push(Draft { label: *n, quote: String::new(), verdict: Verdict::NichtZitiert });
        }
    }
    let mut text = lines.join("\n");
    let mut footer: Vec<String> = Vec::new();
    if let Some(reason) = &research.failure {
        footer.push(failure_line(reason));
    }
    if !evidence.iter().any(|d| d.verdict != Verdict::NichtBelegt) {
        footer.push(CHECK_NOTE.to_owned());
    }
    let pages: Vec<&WebPage> = cited.iter().filter_map(|n| research.page(*n)).collect();
    footer.push(sources_line(&pages));
    text.push_str("\n\n");
    text.push_str(&footer.join("\n"));
    Checked { text, evidence }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page(label: usize, url: &str, text: &str) -> WebPage {
        WebPage {
            label,
            url: url.into(),
            title: format!("Seite {label}"),
            preferred: label == 1,
            text: text.into(),
            accessed: "2026-11-07T09:30:05Z".into(),
            day: "07.11.2026".into(),
        }
    }

    fn research() -> Research {
        Research {
            query: "Licht".into(),
            pages: vec![
                page(1, "https://lichtkunde.example/a", "Das Licht   kreist über dem\nWasser, sagt der ‚Wärter‘."),
                page(2, "https://archiv.example/b/", "Im Jahr 1890 sanken zwölf Schiffe vor der Küste."),
            ],
            ..Research::default()
        }
    }

    #[test]
    fn quotes_on_the_page_stay_and_invented_ones_are_marked() {
        let answer = "Laut [W1] gilt: „Das Licht kreist über dem Wasser.“\n\
                      Und [W2] schreibt \"Im Jahr 1890 sanken dreißig Schiffe\".\n\
                      Auslassung [W2]: „Im Jahr 1890 … vor der Küste“.";
        let checked = check(answer, &research());
        let lines: Vec<&str> = checked.text.lines().collect();
        assert_eq!(
            lines[0], "Laut [W1] gilt: „Das Licht kreist über dem Wasser.“",
            "Wortlaut der Seite (Leerraum egal): ohne Zusatz"
        );
        assert_eq!(
            lines[1], "Und [W2] schreibt \"Im Jahr 1890 sanken dreißig Schiffe\" (nicht belegt).",
            "erfundener Wortlaut: gekennzeichnet direkt hinter dem Zitat"
        );
        assert_eq!(
            lines[2], "Auslassung [W2]: „Im Jahr 1890 … vor der Küste“.",
            "Auslassung … als Lücke in der Reihenfolge"
        );
        assert_eq!(
            checked.evidence.iter().map(|d| (d.label, d.verdict)).collect::<Vec<_>>(),
            vec![(1, Verdict::Belegt), (2, Verdict::NichtBelegt), (2, Verdict::Belegt)],
            "je Zitat ein Eintrag mit Urteil"
        );
        assert!(!checked.text.contains(CHECK_NOTE), "es gibt Belege: kein „Angabe prüfen“: {}", checked.text);
        assert_eq!(
            lines.last().copied(),
            Some(
                "Quellen: [W1] Seite 1 – https://lichtkunde.example/a (abgerufen am 07.11.2026); \
                 [W2] Seite 2 – https://archiv.example/b/ (abgerufen am 07.11.2026)"
            ),
            "die Quellenzeile setzt der Code, in der Reihenfolge der Nennung"
        );
        assert_eq!(
            check(&checked.text, &research()).text.matches(NOT_BELEGT).count(),
            1,
            "nicht doppelt gekennzeichnet"
        );
    }

    #[test]
    fn unknown_labels_unfetched_urls_and_missing_evidence_are_marked() {
        let answer = "Siehe [W9] und https://anders.example/x. Auch https://archiv.example/b passt.";
        let checked = check(answer, &research());
        let first = checked.text.lines().next().unwrap();
        assert_eq!(
            first,
            "Siehe [W9] (unbekannte Quelle) und https://anders.example/x (nicht abgerufen). Auch https://archiv.example/b passt.",
            "[W9] ohne Seite, fremde Adresse nicht abgerufen; abgerufene Adresse (ohne / am Ende) bleibt"
        );
        assert!(checked.text.contains(&format!("\n\n{CHECK_NOTE}\n")), "ohne Beleg: Angabe prüfen: {}", checked.text);
        assert!(checked.text.ends_with("Quellen: keine abgerufene Seite genannt"), "Quellenzeile auch ohne Nennung");
        assert!(checked.evidence.is_empty(), "nichts genannt, nichts zu speichern");
        let named = check("Nur genannt: [W1].", &research());
        assert_eq!(
            named.evidence,
            vec![Draft { label: 1, quote: String::new(), verdict: Verdict::NichtZitiert }],
            "genannt ohne Zitat"
        );
        assert!(!named.text.contains(CHECK_NOTE), "eine genannte, abgerufene Seite ist ein Beleg ohne Zitat");
        let failed = Research { failure: Some("Zeitlimit\nerreicht".into()), ..Research::default() };
        assert!(
            check("Antwort.", &failed).text.contains("Hinweis: Websuche ausgefallen (Zeitlimit erreicht)"),
            "ausgefallene Websuche einzeilig unter der Antwort"
        );
    }

    #[test]
    fn normalization_only_touches_space_and_quotation_marks() {
        assert_eq!(
            normalize(" „a“\u{a0}b ‚c‘ \n d\u{ad}e "),
            "\"a\" b 'c' de",
            "Leerraum, Anführungszeichen, weiches Trennzeichen"
        );
        assert!(quote_found("sagt der 'Wärter'", &normalize("sagt der ‚Wärter‘.")), "typografische einfache Zeichen");
        assert!(!quote_found("das licht kreist", &normalize("Das Licht kreist")), "Groß/klein zählt (Wortlaut)");
        assert!(!quote_found(" … ", "irgendwas"), "nur Auslassung ist kein Zitat");
    }

    #[test]
    fn decomposed_umlauts_count_as_the_same_text() {
        // Runde 19b: Seite mit zerlegtem „ü“ (u + U+0308, typisch aus PDFs), Zitat in NFC – und umgekehrt.
        let page = normalize("Die Bru\u{308}cke trägt.");
        assert!(quote_found("Die Brücke trägt", &page), "NFC-Zitat auf zerlegter Seite ist belegt");
        assert!(quote_found("Die Bru\u{308}cke", &normalize("Die Brücke trägt.")), "zerlegtes Zitat auf NFC-Seite");
        assert!(!quote_found("Die Brucke", &page), "ohne Umlaut bleibt es ein anderer Wortlaut");
    }

    #[test]
    fn gapped_quotes_need_whole_words_real_pieces_and_short_gaps() {
        let page = normalize(
            "Der Wärter sagt, dass das Feuer jede Nacht brennt. Im Hafen liegen Boote. Er glaubt nicht an Geister, \
             aber an das Licht über dem Wasser.",
        );
        // Begründung: der gutartige Fall – zwei Stücke aus je mindestens drei Wörtern, kurze Lücke.
        assert!(quote_found("dass das Feuer … liegen Boote. Er glaubt", &page), "kurze Lücke, ganze Wörter: belegt");
        // Begründung: Wortfetzen aus je einem Wort ergeben einen beliebigen Satz – kein Beleg.
        assert!(!quote_found("Feuer … Boote … nicht", &page), "Stücke unter drei Wörtern: nicht belegt");
        // Begründung: Teilwörter („euer“ in „Feuer“) sind nicht der Wortlaut der Seite.
        assert!(!quote_found("euer jede Nacht", &page), "kein Treffer mitten im Wort");
        assert!(quote_found("das Feuer jede Nacht", &page), "derselbe Wortlaut an Wortgrenzen: belegt");
        // Begründung: Stücke weit auseinander (über MAX_GAP_CHARS) gehören nicht zu einem Satz.
        let far = format!("Am Anfang war das Licht. {} Am Ende kam die Flut.", "Füllwort ".repeat(100));
        assert!(!quote_found("Am Anfang war … Am Ende kam", &normalize(&far)), "Lücke über der Grenze: nicht belegt");
        let near = "Am Anfang war das Licht. Danach Am Ende kam die Flut.";
        assert!(quote_found("Am Anfang war … Am Ende kam", near), "dieselben Stücke nah beieinander: belegt");
        // Begründung: Die erste Stelle des ersten Stücks muss nicht die passende sein (spätere Stelle mit kurzer Lücke).
        let twice =
            format!("Am Anfang war es still. {} Am Anfang war das Wort, Am Ende kam Ruhe.", "Füll ".repeat(120));
        assert!(quote_found("Am Anfang war … Am Ende kam", &normalize(&twice)), "spätere Stelle wird auch probiert");
    }
}
