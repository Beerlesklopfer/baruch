// SPDX-License-Identifier: AGPL-3.0-or-later
//! Recognise and check Bible references (Round 10, task "scripture"): pure, only `std` and `serde`.
//!
//! - [`parse`] reads a reference such as „1. Kön 3,16–28“, „Ps 139,23-24“ or
//!   „Joh 3,16; 4,1“ (the book is carried over), chapter ranges („Joh 3,16–4,2“), verse lists („Joh 3,16.18“,
//!   one [`Found`] per entry), „f“ (one verse more) and „ff“ (up to the end of the chapter) and whole chapters
//!   („Ps 23“, verse 0). Errors are in the [`RefStatus`], never a panic.
//! - [`scan`] finds references in running text („… wie in (1. Kor 13,4) …“) with byte range; only known
//!   books, capitalised book, word boundaries. Ambiguous abbreviations („Am“, „Mal“ …) only with a verse.
//!   Since Round 17b: verse lists with comma („Joh 3,16, 18“, also in [`parse`]); no reference before a
//!   count word („Johannes 3 Mal“, „Ps 23 Jahre“) and no time of day after a weekday abbreviation („Mi 19:30“,
//!   „Fr 9:15 Uhr“); „Psalm 139, … Vers 23“ in the same sentence becomes Ps 139,23.
//! - Book, chapter and verse are checked against the embedded versification of the Schlachter 1951
//!   (`data/versification.tsv`, origin and licence in [`SOURCE`]). **No Bible text**: this crate never confirms
//!   the wording of a quotation itself; [`VerseText`] is the interface for that, [`NoBible`]
//!   (no Bible chosen) always answers [`Wording::Unchecked`]. The text is delivered by a SWORD module through
//!   the crate `baruch-sword`; the comparison happens here ([`wording::compare`]).
//! - [`versification`]: the versifications "German" and "KJV" as pure numbers (for SWORD modules).

mod books;
pub mod versification;
pub mod wording;

use serde::Serialize;

/// Origin of the versification (also in the header of `data/versification.tsv`).
pub const SOURCE: &str = "Verszählung: deutsche Zählung der Schlachter-Bibel 1951 nach der Verszählung \
„German“ des SWORD-Projekts (CrossWire Bible Society, canon_german.h; das SWORD-Modul GerSch der Schlachter \
1951 ist in ihr angelegt), abgerufen am 2026-11-06; übernommen nur Kapitel- und Verszahlen, kein Bibeltext.";

/// A biblical book with the number of verses per chapter (`verses[0]` = chapter 1).
#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct Book {
    /// OSIS code, e.g. `1Kgs`.
    pub id: &'static str,
    /// Name according to Schlachter, e.g. „1. Könige“.
    pub name: &'static str,
    /// Common abbreviation, e.g. „1. Kön“ (that is how [`Found::normalized`] writes).
    pub abbr: &'static str,
    #[serde(skip)]
    pub verses: &'static [u16],
}

impl Book {
    /// Number of chapters.
    pub fn chapters(&self) -> u16 {
        self.verses.len() as u16
    }

    /// Number of verses of a chapter (from 1); `None` if the chapter does not exist.
    pub fn verses_in(&self, chapter: u16) -> Option<u16> {
        chapter.checked_sub(1).and_then(|i| self.verses.get(usize::from(i))).copied()
    }
}

/// All 66 books in canonical order.
pub fn books() -> &'static [Book] {
    &books::BOOKS
}

/// Book for a name or an abbreviation („1. Kön“, „1 Kön“, „1.Kön“, „1. Könige“, „Psalm“, „1Kgs“);
/// case, full stops and whitespace do not count. „Joh“ is John, „1. Joh“ the first letter.
pub fn find_book(name: &str) -> Option<&'static Book> {
    books::lookup(name)
}

/// Position of a book in the canonical order of [`books`] (1 = Genesis … 66 = Revelation; Round 31b for the
/// index of Bible references). A book that does not come from [`books`] → 0.
pub fn book_order(book: &Book) -> u8 {
    books().iter().position(|b| b.id == book.id).map_or(0, |i| (i + 1) as u8)
}

/// A reference: from `start` to `end` (chapter, verse each); verse 0 means "whole chapter".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Passage {
    pub book: &'static Book,
    pub start: (u16, u16),
    pub end: (u16, u16),
}

impl Passage {
    /// Sort key in biblical order (Round 31b): book ([`book_order`]), start and end with chapter and verse each in
    /// fixed width, e.g. `43.003.016-003.016` for Joh 3,16. A whole chapter (verse 0) comes before its verses;
    /// „ff“ up to the end of the chapter is already the last verse after the check.
    pub fn sort_key(&self) -> String {
        let clamp = |n: u16| n.min(999);
        format!(
            "{:02}.{:03}.{:03}-{:03}.{:03}",
            book_order(self.book),
            clamp(self.start.0),
            clamp(self.start.1),
            clamp(self.end.0),
            clamp(self.end.1)
        )
    }

    /// Chapters and verses without the book, as [`Found::normalized`] writes them („3,16–18“, „23“, „3,16–4,2“).
    pub fn chapter_verse(&self) -> String {
        let normalized = verse_range(self.book, self.start, self.end);
        match normalized.strip_prefix(self.book.abbr) {
            Some(rest) => rest.trim_start().to_owned(),
            None => normalized,
        }
    }
}

/// Result of the check against the versification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", content = "detail", rename_all = "snake_case")]
pub enum RefStatus {
    /// Book, chapter and verses exist.
    Valid,
    /// The book is unknown.
    UnknownBook,
    /// The chapter does not exist; the book has `max` chapters.
    NoSuchChapter { max: u16 },
    /// The verse does not exist; chapter `chapter` has `max` verses.
    NoSuchVerse { chapter: u16, max: u16 },
    /// Not readable as a reference (reason in German).
    Malformed(String),
}

impl RefStatus {
    /// Short name for interfaces: `valid`, `unknown_book`, `no_such_chapter`, `no_such_verse`,
    /// `malformed`.
    pub fn code(&self) -> &'static str {
        match self {
            RefStatus::Valid => "valid",
            RefStatus::UnknownBook => "unknown_book",
            RefStatus::NoSuchChapter { .. } => "no_such_chapter",
            RefStatus::NoSuchVerse { .. } => "no_such_verse",
            RefStatus::Malformed(_) => "malformed",
        }
    }
}

/// A reference that was found.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Found {
    /// The text of the reference as it stood there (`&text[span.0..span.1]`).
    pub input: String,
    /// Byte range in the searched text.
    pub span: (usize, usize),
    /// The reference, if the book is known and the reference readable (also with too large a chapter or verse;
    /// whether it exists is said by `status`).
    pub passage: Option<Passage>,
    pub status: RefStatus,
    /// Uniform notation („1. Kön 3,16–28“), if the book is known and the reference readable.
    pub normalized: Option<String>,
}

impl Found {
    /// Result as a German clause, e.g. „vorhanden“ or „Ps 139 hat nur 24 Verse“.
    pub fn message(&self) -> String {
        match (&self.status, &self.passage) {
            (RefStatus::Valid, _) => "vorhanden".to_owned(),
            (RefStatus::UnknownBook, _) => format!("Buch unbekannt in „{}“", self.input),
            (RefStatus::NoSuchChapter { max }, Some(p)) => {
                format!("{} hat nur {max} Kapitel", p.book.name)
            }
            (RefStatus::NoSuchChapter { max }, None) => format!("das Buch hat nur {max} Kapitel"),
            (RefStatus::NoSuchVerse { chapter, max }, Some(p)) => {
                format!("{} {chapter} hat nur {max} {}", p.book.abbr, if *max == 1 { "Vers" } else { "Verse" })
            }
            (RefStatus::NoSuchVerse { chapter, max }, None) => format!("Kapitel {chapter} hat nur {max} Verse"),
            (RefStatus::Malformed(why), _) => format!("nicht lesbar: {why}"),
        }
    }
}

/// Wording of a reference.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum Wording {
    /// Not checked: Baruch has no Bible text.
    Unchecked,
    /// Wording from a local Bible.
    Text(String),
}

/// Source for the wording of a reference.
pub trait VerseText {
    fn wording(&self, passage: &Passage) -> Wording;
}

/// No Bible chosen: every wording stays unchecked (with a Bible: `baruch-sword`).
#[derive(Debug, Clone, Copy, Default)]
pub struct NoBible;

impl VerseText for NoBible {
    fn wording(&self, _passage: &Passage) -> Wording {
        Wording::Unchecked
    }
}

// ---- Reading --------------------------------------------------------------------------------------

/// "Up to the end of the chapter" (ff), until the check knows the end.
const CHAPTER_END: u16 = u16::MAX;

#[derive(Debug, Clone)]
enum BookPart {
    Known(&'static Book),
    Unknown,
}

#[derive(Debug)]
struct Item {
    span: (usize, usize),
    start: (u16, u16),
    end: (u16, u16),
}

#[derive(Debug)]
struct Segment {
    book: BookPart,
    /// Spelling of the book as it stood there (for [`AMBIGUOUS`](books::AMBIGUOUS)).
    token: Option<String>,
    items: Vec<Item>,
    end: usize,
    /// Round 23a: the reference names chapter **and** verse explicitly („3,16“, „1,3“) – not only a number that is
    /// read as a verse for a book with one chapter („Jud 3“, „Ob 3“).
    explicit_verse: bool,
}

fn rest(s: &str, i: usize) -> &str {
    &s[i..]
}

fn is_space(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\u{a0}' | '\u{202f}')
}

fn skip_spaces(s: &str, mut i: usize) -> usize {
    while let Some(c) = rest(s, i).chars().next().filter(|c| is_space(*c)) {
        i += c.len_utf8();
    }
    i
}

/// 1 to 3 digits, no fourth.
fn number(s: &str, i: usize) -> Option<(u16, usize)> {
    let digits = rest(s, i).bytes().take_while(u8::is_ascii_digit).count();
    if digits == 0 || digits > 3 {
        return None;
    }
    Some((rest(s, i)[..digits].parse().ok()?, i + digits))
}

/// Hyphen or dash (with whitespace before and after) → position after it.
fn dash(s: &str, i: usize) -> Option<usize> {
    let j = skip_spaces(s, i);
    let c = rest(s, j).chars().next().filter(|c| matches!(c, '-' | '–' | '—' | '‒'))?;
    Some(skip_spaces(s, j + c.len_utf8()))
}

/// Comma or colon between chapter and verse (with whitespace) → position after it. If a numbered book follows
/// after whitespace („Joh 3, 1. Joh 4“, „Ps 23, 2. Sam 7,14“), the comma is a
/// list separator and not the start of a verse: then `None`.
fn verse_mark(s: &str, i: usize) -> Option<usize> {
    let j = skip_spaces(s, i);
    if !rest(s, j).starts_with([',', ':']) {
        return None;
    }
    let m = skip_spaces(s, j + 1);
    let numbered_book = m > j + 1
        && book_token(s, m, false)
            .is_some_and(|(token, _)| token.starts_with(['1', '2', '3']) && books::lookup(token).is_some());
    (!numbered_book).then_some(m)
}

/// Comma or colon without whitespace directly before a number („4,2“ in the range „3,16–4,2“); that way
/// „Ps 139,23-24, 2. Sam …“ stays a verse range.
fn tight_verse_mark(s: &str, i: usize) -> Option<usize> {
    (rest(s, i).starts_with([',', ':']) && rest(s, i + 1).starts_with(|c: char| c.is_ascii_digit())).then_some(i + 1)
}

fn alnum_at(s: &str, i: usize) -> bool {
    rest(s, i).chars().next().is_some_and(char::is_alphanumeric)
}

/// A book name from `i`: optionally „1“–„3“ with full stop and whitespace, then at least two letters and
/// optionally a full stop. `upper`: the first letter must be upper case (running text).
fn book_token(s: &str, i: usize, upper: bool) -> Option<(&str, usize)> {
    let mut j = i;
    if rest(s, j).starts_with(['1', '2', '3', '4', '5']) {
        j += 1;
        if rest(s, j).starts_with('.') {
            j += 1;
        }
        j = skip_spaces(s, j);
    }
    let letters_start = j;
    let mut count = 0;
    for c in rest(s, j).chars().take_while(|c| c.is_alphabetic()) {
        if count == 0 && upper && !c.is_uppercase() {
            return None;
        }
        count += 1;
        j += c.len_utf8();
    }
    if count < 2 || j == letters_start {
        return None;
    }
    if rest(s, j).starts_with('.') {
        j += 1;
    }
    Some((&s[i..j], j))
}

/// „f“ or „ff“ (not the start of a word) or a range after verse `v` in chapter `chapter`.
fn tail(s: &str, k: usize, chapter: u16, v: u16) -> ((u16, u16), usize) {
    let p = skip_spaces(s, k);
    if rest(s, p).starts_with("ff") && !alnum_at(s, p + 2) {
        return ((chapter, CHAPTER_END), p + 2);
    }
    if rest(s, p).starts_with('f') && !alnum_at(s, p + 1) {
        return ((chapter, v.saturating_add(1)), p + 1);
    }
    if let Some(d) = dash(s, k)
        && let Some((n, q)) = number(s, d)
    {
        if let Some(r) = tight_verse_mark(s, q)
            && let Some((w, q2)) = number(s, r)
        {
            return ((n, w), q2);
        }
        return ((chapter, n), q);
    }
    ((chapter, v), k)
}

/// Words after a number that mark it as a count, duration or time of day („Johannes 3 Mal“, „3 x“,
/// „Ps 23 Jahre“): then it is not a reference in running text (Round 17b).
const COUNT_WORDS: [&str; 30] = [
    "Mal",
    "mal",
    "x",
    "×",
    "Stück",
    "Jahr",
    "Jahre",
    "Jahren",
    "Tag",
    "Tage",
    "Tagen",
    "Uhr",
    "Minute",
    "Minuten",
    "Stunde",
    "Stunden",
    "Woche",
    "Wochen",
    "Monat",
    "Monate",
    "Monaten",
    "Sekunden",
    "Euro",
    "Prozent",
    "Meter",
    "Kilometer",
    "km",
    "Personen",
    "Leute",
    "Seiten",
];

/// Is there from `i` (after optional whitespace) a count word from [`COUNT_WORDS`] as a whole word?
///
/// Round 19c: „Mal“ is also the abbreviation of Malachi. If a chapter reference follows the „Mal“ („Er las Joh
/// 3,16 Mal 3,20 vor.“), it is the book, not a count word – otherwise the reference before it would be dropped
/// silently.
fn count_word_follows(s: &str, i: usize) -> bool {
    let j = skip_spaces(s, i);
    let word: String = rest(s, j).chars().take_while(|c| c.is_alphabetic() || *c == '×').collect();
    if word.is_empty() || !COUNT_WORDS.contains(&word.as_str()) || alnum_at(s, j + word.len()) {
        return false;
    }
    // Chapter and verse („Mal 3,20“); a bare number („3 Mal 2 Tage“) stays a count.
    let book_follows = word == "Mal"
        && number(s, skip_spaces(s, j + word.len()))
            .and_then(|(_, k)| verse_mark(s, k))
            .is_some_and(|m| number(s, m).is_some());
    !book_follows
}

/// Further verse of a list with comma from `k` („, 18“ in „Joh 3,16, 18“): `(verse, start, end of the number)`.
/// Not a verse is a number with full stop („, 2. Sam“, „, 3. Tag“), before chapter and verse („, 4,1“), before a
/// book („, 1 Joh 4“), before a count word („, 18 Mal“) or in the middle of a word („, 18a“).
fn comma_verse(s: &str, k: usize) -> Option<(u16, usize, usize)> {
    if !rest(s, k).starts_with(',') {
        return None;
    }
    let m = skip_spaces(s, k + 1);
    let (n, q) = number(s, m)?;
    let ordinal = rest(s, q).starts_with('.') && !rest(s, q + 1).starts_with(|c: char| c.is_ascii_digit());
    let book = book_token(s, m, false).is_some_and(|(token, _)| books::lookup(token).is_some());
    let follows = |tail: &str| rest(s, q).starts_with(tail) && !alnum_at(s, q + tail.len());
    let glued = alnum_at(s, q) && !follows("ff") && !follows("f");
    if n == 0 || ordinal || book || glued || tight_verse_mark(s, q).is_some() || count_word_follows(s, q) {
        return None;
    }
    Some((n, m, q))
}

/// Chapters and verses from `j`. `seg_start`: start of the reference (range of the first entry).
fn items(s: &str, j: usize, seg_start: usize, single: bool) -> Result<(Vec<Item>, usize), String> {
    let (c, k) = number(s, j).ok_or("Kapitel fehlt")?;
    let verse_at = verse_mark(s, k).filter(|&m| number(s, m).is_some());
    let (chapter, first_verse) = match verse_at {
        Some(m) => {
            let (v, after) = number(s, m).expect("eben geprüft");
            (c, Some((v, m, after)))
        }
        // Books with one chapter: „Jud 3“ is verse 3.
        None if single => (1, Some((c, j, k))),
        None => (c, None),
    };
    let Some((mut v, mut v_pos, mut after)) = first_verse else {
        let mut end = (c, 0);
        let mut k = k;
        if let Some(d) = dash(s, k)
            && let Some((n, q)) = number(s, d)
            && tight_verse_mark(s, q).is_none()
        {
            end = (n, 0);
            k = q;
        }
        return Ok((vec![Item { span: (seg_start, k), start: (c, 0), end }], k));
    };
    let mut list = Vec::new();
    let mut chapter = chapter;
    loop {
        if v == 0 {
            return Err("Vers 0 gibt es nicht".to_owned());
        }
        let (end, k) = tail(s, after, chapter, v);
        let from = if list.is_empty() { seg_start } else { v_pos };
        list.push(Item { span: (from, k), start: (chapter, v), end });
        chapter = end.0;
        // Verse list „3,16.18“: full stop directly before a number.
        if rest(s, k).starts_with('.')
            && let Some((next, a)) = number(s, k + 1)
        {
            v = next;
            v_pos = k + 1;
            after = a;
            continue;
        }
        // Verse list with comma „3,16, 18“ (Round 17b).
        if let Some((next, m, a)) = comma_verse(s, k) {
            v = next;
            v_pos = m;
            after = a;
            continue;
        }
        return Ok((list, k));
    }
}

/// A reference from `i` (book or carried-over book, then chapters and verses).
fn segment(s: &str, i: usize, carry: Option<&BookPart>, scan: bool) -> Result<Segment, String> {
    let (book, token, j) = match book_token(s, i, scan) {
        Some((token, e)) => match books::lookup(token) {
            Some(b) => (BookPart::Known(b), Some(token.to_owned()), e),
            None if scan => return Err("kein Buch".to_owned()),
            None => (BookPart::Unknown, Some(token.to_owned()), e),
        },
        None => match carry {
            Some(c) => (c.clone(), None, i),
            None => return Err("Buch fehlt".to_owned()),
        },
    };
    let single = matches!(book, BookPart::Known(b) if b.chapters() == 1);
    let j = skip_spaces(s, j);
    let (items, end) = items(s, j, i, single)?;
    let explicit_verse = number(s, j).and_then(|(_, k)| verse_mark(s, k)).is_some_and(|m| number(s, m).is_some());
    Ok(Segment { book, token, items, end, explicit_verse })
}

fn verse_range(book: &Book, start: (u16, u16), end: (u16, u16)) -> String {
    let show = |v: u16| if v == CHAPTER_END { "ff".to_owned() } else { v.to_string() };
    let text = if start.1 == 0 {
        if end.0 == start.0 { start.0.to_string() } else { format!("{}–{}", start.0, end.0) }
    } else if end == start {
        format!("{},{}", start.0, start.1)
    } else if end.1 == CHAPTER_END && end.0 == start.0 {
        format!("{},{}ff", start.0, start.1)
    } else if end.0 == start.0 {
        format!("{},{}–{}", start.0, start.1, show(end.1))
    } else {
        format!("{},{}–{},{}", start.0, start.1, end.0, show(end.1))
    };
    format!("{} {text}", book.abbr)
}

/// Checks a reference against the versification; `end.1 == CHAPTER_END` becomes the last verse.
fn check(book: &'static Book, start: (u16, u16), end: (u16, u16)) -> (RefStatus, Passage) {
    let max = book.chapters();
    let mut passage = Passage { book, start, end };
    for c in [start.0, end.0] {
        if book.verses_in(c).is_none() {
            if end.1 == CHAPTER_END {
                passage.end = start;
            }
            return (RefStatus::NoSuchChapter { max }, passage);
        }
    }
    let last = |c: u16| book.verses_in(c).unwrap_or(0);
    if end.1 == CHAPTER_END {
        passage.end.1 = last(end.0);
    }
    for (c, v) in [passage.start, passage.end] {
        if v > last(c) {
            return (RefStatus::NoSuchVerse { chapter: c, max: last(c) }, passage);
        }
    }
    let backwards = if start.1 == 0 { passage.end.0 < start.0 } else { passage.end < start };
    if backwards {
        return (RefStatus::Malformed("Bereich endet vor seinem Anfang".to_owned()), passage);
    }
    (RefStatus::Valid, passage)
}

fn found_items(s: &str, seg: &Segment, out: &mut Vec<Found>) {
    for item in &seg.items {
        let input = s[item.span.0..item.span.1].trim_end().to_owned();
        let span = (item.span.0, item.span.0 + input.len());
        match seg.book {
            BookPart::Known(book) => {
                let normalized = verse_range(book, item.start, item.end);
                let (status, passage) = check(book, item.start, item.end);
                out.push(Found { input, span, passage: Some(passage), status, normalized: Some(normalized) });
            }
            BookPart::Unknown => {
                out.push(Found { input, span, passage: None, status: RefStatus::UnknownBook, normalized: None });
            }
        }
    }
}

/// Reads a reference (one or several, separated with `;`). Every entry of a verse list and
/// every reference after `;` is a [`Found`] of its own; ranges (`span`) refer to `reference`.
/// After `;` the previous book continues to apply if no new one is there („Joh 3,16; 4,1“).
pub fn parse(reference: &str) -> Vec<Found> {
    let mut out = Vec::new();
    let mut carry: Option<BookPart> = None;
    let mut start = 0;
    for piece in reference.split(';') {
        let piece_start = start;
        start += piece.len() + 1;
        let lead = piece.len() - piece.trim_start().len();
        let text = piece.trim();
        if text.is_empty() {
            continue;
        }
        let a = piece_start + lead;
        let b = a + text.len();
        let malformed = |why: String| Found {
            input: text.to_owned(),
            span: (a, b),
            passage: None,
            status: RefStatus::Malformed(why),
            normalized: None,
        };
        // Read only up to the end of this piece.
        let bounded = &reference[..b];
        match segment(bounded, a, carry.as_ref(), false) {
            Ok(seg) if skip_spaces(bounded, seg.end) == b => {
                found_items(bounded, &seg, &mut out);
                carry = Some(seg.book);
            }
            Ok(seg) => out.push(malformed(format!("unerwartet „{}“", bounded[seg.end..].trim()))),
            Err(why) => out.push(malformed(why)),
        }
    }
    out
}

/// Finds references in running text (only known books, book capitalised, at word boundaries).
/// After `;` it continues with the carried-over book as long as a reference is there.
pub fn scan(text: &str) -> Vec<Found> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < text.len() {
        let c = rest(text, i).chars().next().expect("innerhalb des Texts");
        let boundary = text[..i].chars().next_back().is_none_or(|p| !p.is_alphanumeric());
        if boundary
            && (c.is_uppercase() || matches!(c, '1'..='5'))
            && let Ok(seg) = segment(text, i, None, true)
            && accept_in_text(&seg)
            // Round 23a: a count word invalidates only a reference without an explicit verse („Johannes 3 Mal“,
            // „Ps 23 Jahre“). After chapter and verse („Joh 3,16 Mal 3“) the number belongs to the reference – nobody
            // counts „3,16 Mal“ –, the reference stays. „Mal 3“ itself stays unrecognised (ambiguous, only with a
            // verse).
            && (seg.explicit_verse || !count_word_follows(text, seg.end))
            && !weekday_clock(text, i)
        {
            found_items(text, &seg, &mut out);
            let mut end = seg.end;
            // „Psalm 139, … Vers 23“: the chapter gets the verse from the same sentence.
            if let Some((found, after)) = verse_later(text, i, &seg) {
                out.pop();
                out.push(found);
                end = after;
            }
            let book = seg.book;
            // „Joh 3,16; 4,1“ and „Joh 3,16; Röm 8,28“
            loop {
                let j = skip_spaces(text, end);
                if !rest(text, j).starts_with(';') {
                    break;
                }
                let k = skip_spaces(text, j + 1);
                match segment(text, k, Some(&book), true) {
                    Ok(next) if accept_in_text(&next) => {
                        found_items(text, &next, &mut out);
                        end = next.end;
                    }
                    _ => break,
                }
            }
            i = end.max(i + c.len_utf8());
            continue;
        }
        i += c.len_utf8();
    }
    out
}

/// Weekday abbreviation before a time of day („Mi 19:30“, „Fr 9:15 Uhr“): abbreviation, whitespace, hour (0–23),
/// `:` and exactly two digits for the minute (00–59). In running text that is not a reference (Round 17b); the
/// strict [`parse`] still reads „Mi 7:18“ as Micah.
fn weekday_clock(s: &str, i: usize) -> bool {
    const WEEKDAYS: [&str; 7] = ["mo", "di", "mi", "do", "fr", "sa", "so"];
    let Some((token, e)) = book_token(s, i, true) else { return false };
    if !WEEKDAYS.contains(&books::key(token).as_str()) {
        return false;
    }
    let j = skip_spaces(s, e);
    let hours = rest(s, j).bytes().take_while(u8::is_ascii_digit).count();
    if hours == 0 || hours > 2 || !rest(s, j + hours).starts_with(':') {
        return false;
    }
    let m = j + hours + 1;
    let minutes = rest(s, m).bytes().take_while(u8::is_ascii_digit).count();
    let (Ok(h), Ok(min)) = (rest(s, j)[..hours].parse::<u16>(), rest(s, m)[..minutes.min(2)].parse::<u16>()) else {
        return false;
    };
    minutes == 2 && h <= 23 && min <= 59
}

/// Maximum distance (characters) between a whole chapter and „Vers …“ in the same sentence.
const VERSE_LATER_CHARS: usize = 120;

/// Abbreviations whose full stop does not end the sentence between chapter and „Vers …“.
const SENTENCE_ABBREVIATIONS: [&str; 16] =
    ["vgl", "bzw", "z", "B", "d", "h", "u", "a", "s", "o", "ca", "Nr", "S", "Kap", "bes", "etc"];

/// „Psalm 139, … Vers 23“ (also „Verse 23–24“, „Versen 3f“, „V. 23“): if after a whole chapter in the same
/// sentence – at most [`VERSE_LATER_CHARS`] characters, no end of sentence, no empty line and no other reference
/// in between – a verse is given, it becomes a reference with verse. Range from the book to the verse number.
fn verse_later(s: &str, start: usize, seg: &Segment) -> Option<(Found, usize)> {
    let BookPart::Known(book) = seg.book else { return None };
    let [item] = seg.items.as_slice() else { return None };
    if item.start.1 != 0 || item.end != item.start {
        return None;
    }
    let chapter = item.start.0;
    let mut p = seg.end;
    let mut chars = 0;
    while p < s.len() && chars <= VERSE_LATER_CHARS {
        let c = rest(s, p).chars().next().expect("innerhalb des Texts");
        let boundary = s[..p].chars().next_back().is_none_or(|b| !b.is_alphanumeric());
        if boundary && c.is_alphabetic() {
            let word: String = rest(s, p).chars().take_while(|c| c.is_alphabetic()).collect();
            let after_word = p + word.len();
            let marker = match word.as_str() {
                "Vers" | "Verse" | "Versen" | "vers" | "verse" => Some(after_word),
                "V" if rest(s, after_word).starts_with('.') => Some(after_word + 1),
                _ => None,
            };
            if let Some(m) = marker {
                let m = skip_spaces(s, m);
                if let Some((v, after)) = number(s, m) {
                    if v == 0 {
                        return None;
                    }
                    let (end, k) = tail(s, after, chapter, v);
                    let input = s[start..k].trim_end().to_owned();
                    let span = (start, start + input.len());
                    let normalized = verse_range(book, (chapter, v), end);
                    let (status, passage) = check(book, (chapter, v), end);
                    return Some((
                        Found { input, span, passage: Some(passage), status, normalized: Some(normalized) },
                        k,
                    ));
                }
                return None;
            }
            // Another reference ends the search („Ps 23 und Joh 3 Vers 16“: the verse belongs to Joh 3).
            if c.is_uppercase() && segment(s, p, None, true).is_ok() {
                return None;
            }
            // Round 23a (code review quellen-3): another chapter without a book name ends it too („Joh 3 und Kapitel
            // 4 Vers 1“ is not Joh 3,1).
            if matches!(word.as_str(), "Kapitel" | "Kapiteln" | "Kapitels" | "Kap" | "kapitel") {
                return None;
            }
            p = after_word;
            chars += word.chars().count();
            continue;
        }
        if boundary && matches!(c, '1'..='3') && segment(s, p, None, true).is_ok() {
            return None;
        }
        // Round 23a (code review quellen-3): a further number between chapter and „Vers“ is another chapter
        // („Ps 22 und 23 Vers 1“) – the verse then does not belong to the first. An ordinal („im 2. Teil“) is none.
        if boundary && c.is_ascii_digit() {
            let digits = rest(s, p).bytes().take_while(u8::is_ascii_digit).count();
            if !rest(s, p + digits).starts_with('.') {
                return None;
            }
        }
        if rest(s, p).starts_with("\n\n") {
            return None;
        }
        if matches!(c, '.' | '!' | '?') {
            let next_is_space = rest(s, p + 1).chars().next().is_none_or(char::is_whitespace);
            let before: String = {
                let head = &s[..p];
                let len: usize = head.chars().rev().take_while(|c| c.is_alphanumeric()).map(char::len_utf8).sum();
                head[head.len() - len..].to_owned()
            };
            // „im 2. teil“ (ordinal before a lower-case word) does not end the sentence, „Psalm 139. Vers 23“ does.
            let next_word = rest(s, skip_spaces(s, p + 1)).chars().next();
            let ordinal = !before.is_empty()
                && before.chars().all(|c| c.is_ascii_digit())
                && next_word.is_some_and(char::is_lowercase);
            let abbreviation = SENTENCE_ABBREVIATIONS.contains(&before.as_str());
            if next_is_space && !(c == '.' && (ordinal || abbreviation)) {
                return None;
            }
        }
        p += c.len_utf8();
        chars += 1;
    }
    None
}

/// In running text: ambiguous abbreviations only with a verse.
///
/// Round 23a (code review quellen-4): for a book with one chapter the parser reads the first number as a verse
/// („Ob 3“ = Obadiah 1,3); the rule "only with a verse" therefore never applied, and „Ob 3 oder 4 Gäste kommen“
/// became a reference. An ambiguous abbreviation counts there only with explicit chapter and verse („Ob 1,3“) or
/// with a range („Ob 3–4“).
fn accept_in_text(seg: &Segment) -> bool {
    let ambiguous = seg.token.as_deref().is_some_and(|t| books::AMBIGUOUS.contains(&books::key(t).as_str()));
    if !ambiguous {
        return true;
    }
    if seg.items.iter().any(|item| item.start.1 == 0) {
        return false;
    }
    let single = matches!(seg.book, BookPart::Known(b) if b.chapters() == 1);
    !single || seg.explicit_verse || seg.items.iter().any(|item| item.end != item.start)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn found(text: &str) -> Vec<String> {
        scan(text).into_iter().filter_map(|f| f.normalized).collect()
    }

    /// Round 23a (backlog „Joh 3,16 Mal 3“): rule – a count word invalidates only a reference without an explicit
    /// verse.
    #[test]
    fn a_count_word_only_voids_a_reference_without_an_explicit_verse() {
        assert_eq!(
            found("Er las Joh 3,16 Mal 3 vor."),
            ["Joh 3,16"],
            "Joh 3,16 bleibt (vorher entfiel sie still); „Mal 3“ allein bleibt unerkannt, weil mehrdeutig"
        );
        assert_eq!(found("Er las Joh 3,16 Mal 3,20 vor."), ["Joh 3,16", "Mal 3,20"], "mit Vers ist „Mal“ Maleachi");
        assert!(found("Er las Johannes 3 Mal am Tag.").is_empty(), "ohne Vers bleibt „3 Mal“ eine Anzahl");
        assert!(found("Mit Ps 23 Jahre gelebt.").is_empty(), "ohne Vers bleibt „23 Jahre“ eine Dauer");
        assert!(
            found("Den Brief Jud 3 Mal gelesen.").is_empty(),
            "bei einem Buch mit einem Kapitel ist „3“ kein ausdrücklicher Vers: weiter eine Anzahl"
        );
    }

    /// Round 23a (code review quellen-3): another chapter without a book name between chapter and „Vers“.
    #[test]
    fn a_later_verse_does_not_attach_across_another_chapter() {
        assert_eq!(
            found("Lies Joh 3 und Kapitel 4 Vers 1."),
            ["Joh 3"],
            "„Kapitel 4 Vers 1“ gehört nicht zu Joh 3: das Kapitel bleibt ganz, kein falsches Joh 3,1"
        );
        assert_eq!(found("In Röm 8 und dann in Kapitel 12 Vers 2 steht es."), ["Röm 8"], "ebenso Röm 8, nicht Röm 8,2");
        assert_eq!(found("Siehe Ps 22 und 23 Vers 1."), ["Ps 22"], "eine weitere Zahl ist ein anderes Kapitel");
        assert_eq!(
            found("In Psalm 139, dort im 2. teil, steht Vers 23."),
            ["Ps 139,23"],
            "eine Ordnungszahl („2. teil“, klein wie im vorhandenen Satzende-Test) ist kein Kapitel: der Vers gehört weiter zu Psalm 139"
        );
        assert_eq!(found("Psalm 139 nennt in Vers 23 die Bitte."), ["Ps 139,23"], "der gewöhnliche Fall bleibt");
    }

    /// Round 23a (code review quellen-4): „Ob“ is a conjunction and the abbreviation of Obadiah (one chapter).
    #[test]
    fn an_ambiguous_one_chapter_book_needs_chapter_and_verse_or_a_range() {
        assert!(found("Ob 3 oder 4 Gäste kommen, ist offen.").is_empty(), "„Ob 3“ im Satz ist keine Stelle");
        assert_eq!(found("Vgl. Ob 1,3 dazu."), ["Ob 1,3"], "mit Kapitel und Vers ist es Obadja");
        assert_eq!(found("Vgl. Ob 3–4 dazu."), ["Ob 1,3–4"], "ein Bereich ist ebenfalls eindeutig");
        assert_eq!(found("Vgl. Obadja 3 dazu."), ["Ob 1,3"], "der ausgeschriebene Name ist nicht mehrdeutig");
    }

    #[test]
    fn numbers_take_at_most_three_digits() {
        assert_eq!(number("176", 0), Some((176, 3)), "drei Ziffern");
        assert_eq!(number("1234", 0), None, "vier Ziffern sind keine Stelle (Jahreszahl)");
        assert_eq!(number("x", 0), None, "keine Ziffer");
    }

    #[test]
    fn book_tokens_need_two_letters() {
        assert_eq!(book_token("1. Kön 3", 0, false).map(|t| t.0), Some("1. Kön"), "mit Nummer");
        assert_eq!(book_token("4,1", 0, false), None, "Kapitel ist kein Buch");
        assert_eq!(book_token("joh 3", 0, true), None, "im Fließtext groß");
    }
}
