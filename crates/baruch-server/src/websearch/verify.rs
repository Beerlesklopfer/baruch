// SPDX-License-Identifier: AGPL-3.0-or-later
//! Check of the answer in the code ([`check`]) – it is not the model that decides what is evidenced:
//!
//! - A quotation („…“, “…”, "…" or »…«) in the same line as a `[Wn]` counts as evidenced only if,
//!   after normalisation ([`normalize`]: whitespace, typographic quotation marks, soft hyphen), it is a
//!   substring of a retrieved page named there; omissions („…“, „[…]“, „...“) split the quotation
//!   into pieces that must occur in this order (each at word boundaries; with an omission each at least
//!   [`MIN_GAP_PIECE_WORDS`] words and at most [`MAX_GAP_CHARS`] characters of gap on the page). Punctuation at
//!   the end of the quotation does not count.
//!   Otherwise [`NOT_BELEGT`] follows.
//! - `[Wn]` without a retrieved page gets [`UNKNOWN_SOURCE`], a named address that was not
//!   retrieved [`NOT_FETCHED`].
//! - Below that the code sets: for a failed web search [`failure_line`], without retrievable evidence (no
//!   named page with an evidenced quotation or without a quotation) [`CHECK_NOTE`] and always the line
//!   „Quellen: [Wn] Titel – URL (abgerufen am TT.MM.JJJJ)“ ([`sources_line`]).
//! - Every named page results in entries for `web_evidence` ([`Draft`]): per quotation `belegt` or
//!   `nicht_belegt`, a page without a quotation `nicht_zitiert`.

use serde::Serialize;
use unicode_normalization::UnicodeNormalization;

use baruch_core::WorkLanguage;
use baruch_core::book_text::book_t;

use super::search::url_at;
use super::{Research, WebPage};
use crate::connections::multigpt::clean_line;

/// Addition behind a quotation that does not stand on the named page.
pub const NOT_BELEGT: &str = "(nicht belegt)";
/// Addition behind a `[Wn]` without a retrieved page.
pub const UNKNOWN_SOURCE: &str = "(unbekannte Quelle)";
/// Addition behind an address that was not retrieved.
pub const NOT_FETCHED: &str = "(nicht abgerufen)";
/// Line if there is no retrievable evidence.
pub const CHECK_NOTE: &str = "Angabe prüfen: nicht belegt";

/// Result of the check of a quotation or of a named page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    Belegt,
    NichtBelegt,
    NichtZitiert,
}

impl Verdict {
    /// Value in `web_evidence.verdict`.
    pub fn as_str(self) -> &'static str {
        match self {
            Verdict::Belegt => "belegt",
            Verdict::NichtBelegt => "nicht_belegt",
            Verdict::NichtZitiert => "nicht_zitiert",
        }
    }

    /// Display.
    pub fn label(code: &str) -> &'static str {
        match code {
            "belegt" => "belegt (Wortlaut auf der Seite gefunden)",
            "nicht_belegt" => "nicht belegt (Wortlaut steht nicht auf der Seite)",
            _ => "genannt, ohne Zitat",
        }
    }
}

/// An entry for `web_evidence`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Draft {
    /// n of `[Wn]`.
    pub label: usize,
    /// The quotation as it stands in the answer (empty for `nicht_zitiert`).
    pub quote: String,
    pub verdict: Verdict,
}

/// Checked answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Checked {
    /// Answer with additions and the lines below.
    pub text: String,
    pub evidence: Vec<Draft>,
}

/// Line if the web search failed.
pub fn failure_line(reason: &str) -> String {
    format!("Hinweis: Websuche ausgefallen ({})", clean_line(reason, 300))
}

/// Text for the comparison: Unicode normal form NFC (Round 19b: decomposed umlauts such as "a" + U+0308 from PDFs
/// equal to the precomposed letter), typographic quotation marks unified, soft hyphen removed, whitespace to one
/// space.
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

/// At least this many words per piece of a quotation with an omission („Gott … ist … nicht“ is no evidence).
pub const MIN_GAP_PIECE_WORDS: usize = 3;
/// An omission may skip at most this many characters on the page.
pub const MAX_GAP_CHARS: usize = 400;

/// Does the quotation (normalised, omissions as gaps) stand on the page (normalised)? Every piece must begin
/// and end at word boundaries; with omissions every piece has at least [`MIN_GAP_PIECE_WORDS`] words,
/// and between two pieces lie at most [`MAX_GAP_CHARS`] characters on the page – otherwise a "quotation" with a
/// distorted meaning could be built from scattered scraps of words.
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

/// Do `pieces` occur from byte `from` in this order at word boundaries, every following piece at most
/// [`MAX_GAP_CHARS`] characters behind the previous one (`window` = a gap applies)? Tries every matching position.
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

/// Does `page[start..end]` lie at word boundaries (no letter/no digit directly before or behind where the
/// piece itself begins or ends with a letter/a digit)?
fn at_boundary(page: &str, start: usize, end: usize) -> bool {
    let word = |c: char| c.is_alphanumeric();
    let inner = &page[start..end];
    let before_ok = !inner.chars().next().is_some_and(word) || !page[..start].chars().next_back().is_some_and(word);
    let after_ok = !inner.chars().next_back().is_some_and(word) || !page[end..].chars().next().is_some_and(word);
    before_ok && after_ok
}

/// `[Wn]` of a line: (end in bytes, n).
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

/// Quotations of a line: (end behind the closing character in bytes, content).
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

/// Addresses of a line: (end in bytes, address).
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

/// Address for the comparison: without anchor and without `/` at the end.
fn url_key(url: &str) -> &str {
    url.split('#').next().unwrap_or(url).trim_end_matches('/')
}

/// The line „Quellen: …“ for the named pages (in the order of the first mention).
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

/// Check the answer (see the module documentation) with the German additions ([`check_in`]).
pub fn check(answer: &str, research: &Research) -> Checked {
    check_in(answer, research, WorkLanguage::De)
}

/// The additions Baruch writes into the answer in the work language (Round 39b): (not supported, unknown source,
/// not fetched, check note) from the book catalog; German: [`NOT_BELEGT`], [`UNKNOWN_SOURCE`], [`NOT_FETCHED`],
/// [`CHECK_NOTE`].
pub fn additions(lang: WorkLanguage) -> [String; 4] {
    [
        book_t(lang, "book.websearch.not_supported", &[]),
        book_t(lang, "book.websearch.unknown_source", &[]),
        book_t(lang, "book.websearch.not_fetched", &[]),
        book_t(lang, "book.websearch.check_note", &[]),
    ]
}

/// Like [`check`] with the additions in the work language `lang` (Round 39b). An addition already behind a label,
/// quotation or address is recognised in every work language and not written twice.
pub fn check_in(answer: &str, research: &Research, lang: WorkLanguage) -> Checked {
    let [not_supported, unknown_source, not_fetched, check_note] = additions(lang);
    let already = |rest: &str, index: usize| {
        WorkLanguage::ALL.into_iter().any(|other| rest.starts_with(&format!(" {}", additions(other)[index])))
    };
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
            } else if !already(&line[end..], 1) {
                inserts.push((end, format!(" {unknown_source}")));
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
                        if !already(&line[end..], 0) {
                            inserts.push((end, format!(" {not_supported}")));
                        }
                    }
                }
            }
        }
        for (end, url) in urls_in(line) {
            if !fetched.contains(&url_key(&url)) && !already(&line[end..], 2) {
                inserts.push((end, format!(" {not_fetched}")));
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
        footer.push(check_note.clone());
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
        // Round 19b: page with a decomposed u-umlaut (u + U+0308, typical from PDFs), quotation in NFC – and vice
        // versa.
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
        // Reason: the benign case – two pieces of at least three words each, short gap.
        assert!(quote_found("dass das Feuer … liegen Boote. Er glaubt", &page), "kurze Lücke, ganze Wörter: belegt");
        // Reason: scraps of one word each result in an arbitrary sentence – no evidence.
        assert!(!quote_found("Feuer … Boote … nicht", &page), "Stücke unter drei Wörtern: nicht belegt");
        // Reason: partial words („euer“ in „Feuer“) are not the wording of the page.
        assert!(!quote_found("euer jede Nacht", &page), "kein Treffer mitten im Wort");
        assert!(quote_found("das Feuer jede Nacht", &page), "derselbe Wortlaut an Wortgrenzen: belegt");
        // Reason: pieces far apart (over MAX_GAP_CHARS) do not belong to one sentence.
        let far = format!("Am Anfang war das Licht. {} Am Ende kam die Flut.", "Füllwort ".repeat(100));
        assert!(!quote_found("Am Anfang war … Am Ende kam", &normalize(&far)), "Lücke über der Grenze: nicht belegt");
        let near = "Am Anfang war das Licht. Danach Am Ende kam die Flut.";
        assert!(quote_found("Am Anfang war … Am Ende kam", near), "dieselben Stücke nah beieinander: belegt");
        // Reason: the first position of the first piece need not be the matching one (later position with a short gap).
        let twice =
            format!("Am Anfang war es still. {} Am Anfang war das Wort, Am Ende kam Ruhe.", "Füll ".repeat(120));
        assert!(quote_found("Am Anfang war … Am Ende kam", &normalize(&twice)), "spätere Stelle wird auch probiert");
    }
}
